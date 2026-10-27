//! Retained first-edition story (domain/testing.md, 5.3): the person's budget answer, a raised investigation and one advisor.
use temper_real_loop_world::{
    Exit,
    case::{self, oracle::Observed},
    run_cluster_forge,
};

#[test]
fn a_budget_question_climbs_two_owners_to_the_persons_inbox_and_a_raised_investigation_hands_its_report_to_an_advisor_without_the_director_asking_the_person()
 {
    // A's question stands for the director's own question to the person:
    // a task cannot ask its own person. The answer recommends no budget raise.
    // The default/owner models stand for Codex/Opus; the query carries no effort.
    // The 70 s, 60 s, findings and cause are words, checked and never judged.
    // Keeping the agreement as a standing rule is 4.4/5.6's part.
    // Singleton batches keep the investigation and advisor sequential. A1's
    // amendment is held until a5 parks and its medium document is read, so
    // the high choice belongs to its next run. Input cue rules tolerate news.
    // The high report is held across its task/activity read, then released.
    // ctl answer wakes the chat; the director relays it as an answer to A,
    // and A answers A1. The chat waits on A's question instead of delegating
    // again when a resumed input also carries the opening words.
    // The effort-only amendment's delivered reason selects a5's next turn;
    // cue rules suffice without also amending its spec. Activity names
    // neither effort nor spec revision per run: the high task document and
    // the next run after decided revision 1 observe the next try.
    // A alone is tracked, at priority one; A1 has depth one and A1_DEADLINE.
    // A and A1 carry tool families 7 to delegate, message and amend; a5 has
    // messages alone, the advisor none. Small admits the singleton tree.
    // Source journal pages follow their supplied cursors for all three
    // owners, keeping every parked attempt.
    // Earlier steps stay unchanged; batch_member, held turns, delivery/tool
    // helpers and 6.1's attempt rule are reused by calls within the library.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_5(),
        case::scripts::step_6_5,
        case::scripts::step_6_5_rules,
        case::acts::step_6_5,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(
            called.exit,
            Some(Exit::Code(0)),
            "call {at}: {}\n{}",
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&called.stdout)
        );
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_6_5).with_commands(&case::acts::step_6_5());
    case::oracle::assert_met(&case::oracle::step_6_5(&observed));
}
