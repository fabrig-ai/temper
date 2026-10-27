//! Retained first-edition story (domain/testing.md, 5.3): the person answers the pace question and the director keeps it.
use temper_real_loop_world::{
    case::{
        self,
        oracle::{self, Observed},
    },
    run_forge,
};

#[test]
fn the_pace_question_reaches_the_person_and_its_answer_is_kept() {
    // Leaves out the director as asker (only a delegate can ask), plan
    // windows, resets and sustainable pace as account facts, B's model, and s2's
    // sources. The holder's account order stands for s2, and recalling the
    // note stands for not asking again (domain/testing.md, 5.3's 4.4 row).
    let ran = run_forge(case::fixture_4_4(), case::scripts::step_4_4, case::acts::step_4_4);
    let observed = Observed::from_ran(&ran, case::acts::READS_4_4);
    oracle::assert_met(&oracle::step_4_4(&observed));
}
