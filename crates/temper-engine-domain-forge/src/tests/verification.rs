use super::*;
use fixtures::{BROWSER, CHANGE, FUZZY, MERGE, PROJECT};
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_tiers::{Blame, Failures};

#[test]
fn a_merge_into_a_repository_with_tiers_saves_its_verification() {
    let (_, requests) = fixtures::merged(true);
    assert!(requests.contains(&Request::Save {
        read_afresh: None,
        release: None,
        record: Stored::Landed { commit: MERGE, task: CHANGE }
    }));
    let expected = Verification {
        repository: REPO,
        commit: MERGE,
        changes: Box::new([CHANGE]),
        tiers: Box::new([
            TierVerification { tier: Box::from(BROWSER), verdict: None, repaired: None },
            TierVerification { tier: Box::from(FUZZY), verdict: None, repaired: None },
        ]),
    };
    assert!(requests.contains(&Request::Save {
        read_afresh: None,
        release: None,
        record: Stored::Verification(expected)
    }));
}

#[test]
fn a_merge_into_a_repository_with_no_tier_saves_no_verification() {
    let (d, requests) = fixtures::merged(false);
    assert!(requests.contains(&Request::Save {
        read_afresh: None,
        release: None,
        record: Stored::Landed { commit: MERGE, task: CHANGE }
    }));
    assert!(d.verifications.is_empty());
}

#[test]
fn a_landed_change_steps_with_no_read() {
    let mut d = fixtures::verifying();
    let requests = fixtures::verification_step(&mut d);
    assert!(requests.contains(&Request::ChangeDecision {
        task: CHANGE,
        decision: change::Decision::None,
        entry: None,
        evidence: None
    }));
    assert!(ready(&mut d).is_empty());
}

#[test]
fn one_tier_cleared_leaves_the_change_verifying() {
    let mut d = fixtures::verifying();
    fixtures::judge(&mut d, BROWSER, Blame::Cleared);
    assert_eq!(crate::verification::verification(&d, REPO, MERGE), change::Verification::Verifying);
    assert!(d.procedure_wakes.is_empty());
}

#[test]
fn the_second_tier_cleared_wakes_the_change_and_it_finishes() {
    let mut d = fixtures::verifying();
    fixtures::judge(&mut d, BROWSER, Blame::Cleared);
    fixtures::judge(&mut d, FUZZY, Blame::Cleared);
    assert!(ready(&mut d).contains(&Request::WakeProcedure { task: CHANGE }));
    assert!(fixtures::verification_step(&mut d).contains(&Request::ChangeDecision {
        task: CHANGE,
        decision: change::Decision::Finish { merge: MERGE },
        entry: None,
        evidence: None
    }));
}

#[test]
fn a_tier_blaming_the_landing_turns_the_change_red() {
    for blame in [Blame::Owns(Failures::Many), Blame::Suspect(Failures::Many)] {
        let mut d = fixtures::verifying();
        fixtures::judge(&mut d, BROWSER, blame);
        assert!(d.procedure_wakes.contains_key(&CHANGE));
        fixtures::verification_step(&mut d);
        assert_eq!(d.change(CHANGE).expect("change").change.state, change::State::Red { merge: MERGE });
    }
}

#[test]
fn a_repaired_red_tier_counts_as_cleared() {
    let mut d = fixtures::verifying();
    fixtures::judge(&mut d, BROWSER, Blame::Owns(Failures::Many));
    fixtures::judge(&mut d, FUZZY, Blame::Cleared);
    let row = d.verifications.get_mut(&(REPO, MERGE)).expect("verification");
    row.tiers[0].repaired = Some(fixtures::HEAD);
    assert_eq!(crate::verification::verification(&d, REPO, MERGE), change::Verification::Cleared);
}

#[test]
fn the_row_goes_when_its_change_is_released() {
    let mut d = fixtures::verifying();
    let requests = outputs(
        &mut d,
        Event::Release { task: CHANGE, root: CHANGE, ending: ReleaseEnding::Done, reason: None, entry: fixtures::OPEN },
    );
    assert!(requests.contains(&Request::Erase { key: Key::Verification { repository: REPO, commit: MERGE } }));
    assert!(d.verifications.is_empty());
}

#[test]
fn the_row_is_restored_at_restart() {
    let mut before = fixtures::verifying();
    fixtures::judge(&mut before, BROWSER, Blame::Suspect(Failures::Many));
    let row = before.verifications.get(&(REPO, MERGE)).expect("verification").clone();
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Verification(row.clone()) });
    assert_eq!(d.verifications.get(&(REPO, MERGE)), Some(&row));
    assert_eq!(crate::verification::verification(&d, REPO, MERGE), change::Verification::Red);
}

#[test]
fn project_tiers_accepts_both_name_and_count_bounds() {
    // CHECK_NAME_BYTES, the branch-safe name bound in domain/landing.md, 12.
    const NAME_BYTES: usize = 32;
    for names in [&[BROWSER, FUZZY][..], &[[b'a'; NAME_BYTES].as_slice()][..], &[b"a-Z_09".as_slice()][..]] {
        let mut d = domain();
        assert!(d.project_tiers(PROJECT, fixtures::tier_settings(), fixtures::repository_tiers(names)));
    }
}

#[test]
fn project_tiers_refuses_invalid_configuration_without_replacing_it() {
    // One byte past CHECK_NAME_BYTES (domain/landing.md, 12).
    const PAST_NAME_BYTES: usize = 33;
    for names in [
        &[BROWSER, FUZZY, b"third".as_slice()][..],
        &[[b'a'; PAST_NAME_BYTES].as_slice()][..],
        &[b"".as_slice()][..],
        &[b"a/b".as_slice()][..],
        &[b"a b".as_slice()][..],
        &[BROWSER, BROWSER][..],
    ] {
        let mut d = fixtures::verifying();
        let before = d.tiers.get(&PROJECT).expect("configured").clone();
        assert!(!d.project_tiers(PROJECT, fixtures::tier_settings(), fixtures::repository_tiers(names)));
        assert_eq!(d.tiers.get(&PROJECT), Some(&before));
    }
}

#[test]
fn project_tiers_refuses_a_repeated_repository() {
    let mut d = domain();
    let repeated = RepositoryTiers { repository: REPO, tiers: Box::new([]) };
    assert!(!d.project_tiers(PROJECT, fixtures::tier_settings(), Box::new([repeated.clone(), repeated])));
    assert!(d.tiers.is_empty());
}
