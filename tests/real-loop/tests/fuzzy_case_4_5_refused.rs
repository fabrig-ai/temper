//! Retained first-edition story (domain/testing.md, 5.3): a refused key holds A until the person replaces it.
use temper_real_loop_world::{
    case::{
        self,
        oracle::{self, Observed},
    },
    run_forge,
};

#[test]
fn a_refused_key_holds_its_work_until_it_is_replaced() {
    // Leaves out s2's sign-in and credentials never copied between machines,
    // B's work and the case's five hours (domain/testing.md, 5.3's 4.5 row).
    // One worker and a key replacement stand for sign-in; the failed run ends,
    // and a new attempt resumes the same task's committed conversation.
    let ran = run_forge(case::fixture(), case::scripts::step_4_5_refused, case::acts::step_4_5_refused);
    let observed = Observed::from_ran(&ran, case::acts::READS_4_5_REFUSED);
    oracle::assert_met(&oracle::step_4_5_refused(&observed));
}
