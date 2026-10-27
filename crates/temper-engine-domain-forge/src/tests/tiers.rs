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
    assert_eq!(tier_runs(&out).as_slice(), &[(1, BASE); TWO_TIERS]);
    assert!(fixtures::browser(&d).record.run.as_ref().expect("baseline").landings.is_empty());
}
#[test]
fn a_merge_moving_the_main_starts_the_next_run_over_its_landing() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    let out = fixtures::tier_move(&mut d, MERGE, Some(&[MERGE]));
    assert_eq!(tier_runs(&out).as_slice(), &[(2, MERGE); TWO_TIERS]);
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
    assert!(tier_runs(&out).is_empty());
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
    // Tag 8, repository 1/2 (eight bytes each), tier index 0, run 1, branch operation 1, part 0.
    const PURPOSE_BYTES: [u8; 30] =
        [8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0];
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    let out = outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    let entry = d.entries.get(&OPEN).expect("branch outbox");
    assert_eq!(
        entry.entry.effect.write,
        skein_forge::api::Write::CreateBranch { branch: b"temper/tier/browser.1".as_slice().into(), commit: BASE }
    );
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Pending { entry: OPEN });
    let mut identity = None;
    for request in &out {
        if let Request::TierEffect { purpose, .. } = request {
            identity = purpose.identity();
        }
    }
    assert_eq!(identity.as_deref(), Some(PURPOSE_BYTES.as_slice()));
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
            WORKFLOW_BYTES,
            usize::try_from(LIMITS.ignored_paths).expect("fixture bound fits usize"),
            usize::try_from(LIMITS.name_bytes).expect("fixture bound fits usize"),
            CHECK_NAME_BYTES
        )
    ));
}
#[test]
fn project_tiers_refuses_each_member_one_past_its_bound() {
    for (workflow, paths, path_bytes, artifact) in [
        (WORKFLOW_BYTES + 1, 0, 1, 1),
        (1, usize::try_from(LIMITS.ignored_paths).expect("fixture bound fits usize") + 1, 1, 1),
        (1, 1, usize::try_from(LIMITS.name_bytes).expect("fixture bound fits usize") + 1, 1),
        (1, 0, 1, CHECK_NAME_BYTES + 1),
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

#[test]
fn project_tiers_refuses_invalid_workflows_artifacts_and_ignored_paths() {
    let rows: &[(&[u8], &[u8], &[u8])] = &[
        (b"", b"report", b"docs"),
        (b"a/b", b"report", b"docs"),
        (b"tier.yml", b"", b"docs"),
        (b"tier.yml", b"a/b", b"docs"),
        (b"tier.yml", b"report", b""),
        (b"tier.yml", b"report", b"/"),
        (b"tier.yml", b"report", b"/docs"),
        (b"tier.yml", b"report", b"docs/"),
        (b"tier.yml", b"report", b"a//b"),
        (b"tier.yml", b"report", b"."),
        (b"tier.yml", b"report", b".."),
        (b"tier.yml", b"report", b"a/../b"),
    ];
    for (workflow, artifact, path) in rows {
        let mut d = domain();
        let mut configured = fixtures::repository_tiers(&[BROWSER]);
        configured[0].tiers[0].workflow = Box::from(*workflow);
        configured[0].tiers[0].artifact = Box::from(*artifact);
        configured[0].tiers[0].ignored = Box::new([Box::from(*path)]);
        assert!(!d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), configured));
    }
}

fn tier_runs(out: &Queue<Request>) -> skein_lib::List<(u64, [u8; 32])> {
    let mut runs = skein_lib::List::with_capacity(out.len());
    for request in out {
        if let Request::TierRun { number, tip, .. } = request {
            runs.push((*number, *tip)).expect("output bounds tier requests");
        }
    }
    runs
}
