use super::*;
use fixtures::{BROWSER, CHANGE, FUZZY, MERGE};
use temper_engine_domain_forge_tiers::Blame;

fn history(d: &Domain) -> &[Logged] {
    &d.landing_logs.get(&REPO).expect("repository log").landings
}
fn push(d: &mut Domain, commit: [u8; 32], seen: Wall) {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    log::moved(d, REPO, commit, seen, &mut out);
}
fn judge(d: &mut Domain, landings: &[([u8; 32], Blame)]) {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    log::judged(d, REPO, BROWSER, landings, &mut out);
}
#[test]
fn a_queue_merge_is_logged_with_its_change() {
    let (d, _) = fixtures::merged(true);
    assert_eq!(
        history(&d),
        &[Logged {
            commit: MERGE,
            seen: env().wall,
            changes: Box::new([CHANGE]),
            verdicts: Box::new([
                Mark { tier: BROWSER.into(), verdict: Verdict::Verifying },
                Mark { tier: FUZZY.into(), verdict: Verdict::Verifying },
            ]),
        }]
    );
}
#[test]
fn a_push_from_outside_temper_is_logged_with_no_change() {
    let mut d = fixtures::tiered();
    push(&mut d, MERGE, env().wall);
    assert_eq!(history(&d)[0].changes.as_ref(), &[]);
}
#[test]
fn the_seen_time_is_the_engines_when_the_move_was_seen() {
    let mut d = fixtures::tiered();
    let seen = Wall::from_nanos(123);
    push(&mut d, MERGE, seen);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    log::landed(&mut d, REPO, MERGE, CHANGE, Wall::from_nanos(456), &mut out);
    assert_eq!(history(&d)[0].seen, seen);
    assert_eq!(history(&d).len(), 1);
}
#[test]
fn the_ring_keeps_the_newest_landings_kept() {
    let mut d = fixtures::tiered();
    // The small fixture's ring has sixteen entries; seventeen wraps it once.
    const LAST: u8 = 17;
    for byte in 1..=LAST {
        push(&mut d, [byte; 32], env().wall);
    }
    assert_eq!(history(&d).len(), 16);
    assert_eq!((history(&d)[0].commit, history(&d)[15].commit), ([2; 32], [17; 32]));
}
#[test]
fn a_green_run_clears_its_ranges_entries_for_its_tier() {
    let mut d = fixtures::tiered();
    push(&mut d, fixtures::HEAD, env().wall);
    push(&mut d, MERGE, env().wall);
    judge(&mut d, &[(fixtures::HEAD, Blame::Cleared), (MERGE, Blame::Cleared)]);
    for entry in history(&d) {
        assert_eq!(
            entry.verdicts.as_ref(),
            &[
                Mark { tier: BROWSER.into(), verdict: Verdict::Cleared },
                Mark { tier: FUZZY.into(), verdict: Verdict::Verifying },
            ]
        );
    }
}
#[test]
fn an_entry_merged_into_a_later_landing_takes_its_verdict() {
    let mut d = fixtures::tiered();
    push(&mut d, fixtures::HEAD, env().wall);
    push(&mut d, MERGE, env().wall);
    judge(&mut d, &[(MERGE, Blame::Suspect(temper_engine_domain_forge_tiers::Failures::Many))]);
    for entry in history(&d) {
        assert_eq!(entry.verdicts[0].verdict, Verdict::Red { tests: None });
    }
}
#[test]
fn a_repository_with_no_tier_logs_no_marks() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    push(&mut d, MERGE, env().wall);
    assert_eq!(history(&d)[0].verdicts.as_ref(), &[]);
}
#[test]
fn a_turn_landing_is_logged_with_its_members_change() {
    const LANDING_TASK: u64 = 61;
    let mut row = views::row(LANDING_TASK, LandingPhase::Turn { at: env().wall });
    row.landing.members[0].gates = Box::new([]);
    row.entries = Box::new([LandingEntry { entry: fixtures::OPEN, member: 0, gate: 0, operation: Operation::Merge }]);
    let (d, _) = fixtures::merged_with_landing(true, Some(row));
    assert_eq!(history(&d)[0].changes.as_ref(), &[CHANGE]);
}
