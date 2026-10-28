//! Rung B1's first passing walk, replayed from its six model transcripts.
#[test]
fn a_change_failing_ci_is_repaired_and_lands() {
    let ran = temper_real_loop_world::repair::run();
    temper_real_loop_world::repair::check(&ran);
}
