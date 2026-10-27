//! Retained first-edition story (domain/testing.md, 5.3): held chat, retained question, fresh conventions.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn a_held_director_keeps_the_owners_question_and_its_fresh_run_reads_the_new_rules() {
    // Leaves out the director's compaction budget and fresh context every third
    // check: its charter starts every run fresh. The four hours are a person's
    // release, never a timer; the new shape's rules are words checked, never
    // judged. Kit is context, mounted read-only, and the person's edit is a push
    // to main, not a change through temper (domain/testing.md, 5.3's 5.1 row).
    let ran = run_cluster_forge(
        &["ws", "s1"],
        &[],
        case::fixture_5_1(),
        case::scripts::step_5_1,
        case::scripts::step_5_1_rules,
        case::acts::step_5_1,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_5_1);
    oracle::assert_met(&oracle::step_5_1(&observed));
}
