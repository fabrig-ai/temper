use super::*;
use fixtures::{BASE, BROWSER, CHANGE, HEAD, MERGE, OPEN};
use temper_engine_domain_forge_tiers as policy;
const TWO_TIERS: usize = 2; // browser and fuzzy in the fixture.

#[test]
fn a_tiered_main_is_watched_with_no_task_naming_it() {
    let d = fixtures::tiered();
    assert_eq!(crate::tiers::watched(&d).as_slice(), &[fixtures::main_name()]);
}
#[test]
fn the_first_tip_seen_starts_each_tiers_baseline() {
    let mut d = fixtures::tiered();
    let out = fixtures::tier_baseline(&mut d);
    assert_eq!(
        out.into_iter().filter(|r| matches!(r, Request::TierRun { number: 1, tip: BASE, .. })).count(),
        TWO_TIERS
    );
    assert!(fixtures::browser(&d).record.run.as_ref().expect("baseline").landings.is_empty());
}
#[test]
fn a_merge_moving_the_main_starts_the_next_run_over_its_landing() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    let out = fixtures::tier_move(&mut d, MERGE, Some(&[MERGE]));
    assert_eq!(
        out.into_iter().filter(|r| matches!(r, Request::TierRun { number: 2, tip: MERGE, .. })).count(),
        TWO_TIERS
    );
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("next run").landings.as_ref(),
        &[policy::Landing { commit: MERGE, key: MERGE }]
    );
}
#[test]
fn a_move_during_a_run_waits_for_the_next_range() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    outputs(&mut d, Event::TierTask { repository: REPO, tier: fixtures::FUZZY.into(), number: 1, task: OPEN });
    let out = fixtures::tier_move(&mut d, HEAD, None);
    assert!(!out.into_iter().any(|r| matches!(r, Request::TierRun { .. })));
    assert_eq!(fixtures::browser(&d).pending.as_ref(), &[policy::Landing { commit: HEAD, key: HEAD }]);
}
#[test]
fn a_push_temper_did_not_make_is_a_landing_of_its_own() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    fixtures::tier_move(&mut d, HEAD, Some(&[MERGE, HEAD]));
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[policy::Landing { commit: HEAD, key: HEAD }]
    );
}
#[test]
fn a_merge_and_a_push_in_one_move_are_two_landings_oldest_first() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    fixtures::tier_move(&mut d, HEAD, Some(&[MERGE, HEAD]));
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[policy::Landing { commit: MERGE, key: MERGE }, policy::Landing { commit: HEAD, key: HEAD }]
    );
}
#[test]
fn a_move_seen_without_its_commits_is_one_landing_at_the_tip() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    fixtures::tier_move(&mut d, HEAD, None);
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[policy::Landing { commit: HEAD, key: HEAD }]
    );
}
#[test]
fn the_tier_task_first_creates_its_branch_at_the_tip() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    let out = outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    let entry = d.entries.get(&OPEN).expect("branch outbox");
    assert_eq!(
        entry.entry.effect.write,
        skein_forge::api::Write::CreateBranch { branch: b"temper/tier/browser/1".as_slice().into(), commit: BASE }
    );
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Pending { entry: OPEN });
    assert!(out.iter().any(|r| matches!(
        r,
        Request::TierEffect {
            purpose: crate::purpose::EffectPurpose::Tier { tier: 0, run: 1, operation: 1, part: 0, .. },
            ..
        }
    )));
}
#[test]
fn a_tier_row_is_restored_at_restart() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    let row = fixtures::browser(&d).clone();
    let mut restored = fixtures::tiered();
    outputs(&mut restored, Event::Restore { record: Stored::Tier(row.clone()) });
    assert_eq!(fixtures::browser(&restored), &row);
}
#[test]
fn a_veto_clears_the_uncommitted_branch_entry() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    outputs(&mut d, Event::VetoTier { task: CHANGE, entry: OPEN });
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Absent);
    assert!(!d.entries.contains_key(&OPEN));
}

const SMALL_RANGE: usize = 3; // 2 * changes (one live change) + 1.
#[test]
fn a_full_range_merges_its_two_oldest_outside_landings() {
    let (mut d, env) = fixtures::full_tier_range(SMALL_RANGE);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::moved(&mut d, &env, REPO, BASE, HEAD, None, &mut out);
    assert_eq!(
        fixtures::browser(&d).pending.as_ref(),
        &[
            policy::Landing { commit: [11; 32], key: [11; 32] },
            policy::Landing { commit: MERGE, key: MERGE },
            policy::Landing { commit: HEAD, key: HEAD }
        ]
    );
}
#[test]
fn a_range_one_short_of_full_merges_nothing() {
    let (mut d, env) = fixtures::full_tier_range(SMALL_RANGE - 1);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::moved(&mut d, &env, REPO, BASE, HEAD, None, &mut out);
    assert_eq!(
        fixtures::browser(&d).pending.as_ref(),
        &[
            policy::Landing { commit: [10; 32], key: [10; 32] },
            policy::Landing { commit: [11; 32], key: [11; 32] },
            policy::Landing { commit: HEAD, key: HEAD }
        ]
    );
}
#[test]
fn project_tiers_accepts_each_new_members_bound() {
    let mut d = domain();
    assert!(d.project_tiers(
        fixtures::PROJECT,
        fixtures::tier_settings(),
        fixtures::configured_tier(
            WORKFLOW_BYTES as usize,
            LIMITS.ignored_paths as usize,
            LIMITS.name_bytes as usize,
            CHECK_NAME_BYTES as usize
        )
    ));
}
#[test]
fn project_tiers_refuses_each_member_one_past_its_bound() {
    for (workflow, paths, path_bytes, artifact) in [
        (WORKFLOW_BYTES as usize + 1, 0, 1, 1),
        (1, LIMITS.ignored_paths as usize + 1, 1, 1),
        (1, 1, LIMITS.name_bytes as usize + 1, 1),
        (1, 0, 1, CHECK_NAME_BYTES as usize + 1),
    ] {
        let mut d = domain();
        assert!(!d.project_tiers(
            fixtures::PROJECT,
            fixtures::tier_settings(),
            fixtures::configured_tier(workflow, paths, path_bytes, artifact)
        ));
    }
}
#[test]
fn a_made_branch_is_not_emitted_again_after_restore() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    let mut row = fixtures::browser(&d).clone();
    row.task = Some(CHANGE);
    row.branch = TierBranch::Made;
    outputs(&mut d, Event::Restore { record: Stored::Tier(row) });
    let out = outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    assert_eq!(out.as_ref(), &[Request::TierDecision { task: CHANGE, decision: TierDecision::Wait }]);
}
