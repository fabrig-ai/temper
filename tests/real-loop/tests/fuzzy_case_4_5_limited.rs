//! Retained first-edition story (domain/testing.md, 5.3): an account's reset resumes A without the person's act.
//! The provider's shortest reset is 1 s, the story is measured against the provisional 2 s guide.
use temper_real_loop_world::{
    case::{
        self,
        oracle::{self, Observed},
    },
    run_forge,
};

#[test]
fn a_limited_account_holds_its_work_until_its_reset() {
    // Leaves out s2's sign-in, credentials never copied between machines and B
    // (domain/testing.md, 5.3's 4.5 row). One worker uses a key; the case's five
    // hours become the wire's shortest reset, 1 s. The failed activation ends;
    // a new attempt resumes the same task's committed conversation.
    let ran = run_forge(case::fixture(), case::scripts::step_4_5_limited, case::acts::step_4_5_limited);
    let observed =
        Observed::from_ran(&ran, case::acts::READS_4_5_LIMITED).with_commands(&case::acts::step_4_5_limited());
    oracle::assert_met(&oracle::step_4_5_limited(&observed));
}
