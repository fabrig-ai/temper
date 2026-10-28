use temper_engine_domain_world::tier_runs::{BoundCut, bound_story};
#[test]
fn a_runner_down_past_the_bound_ends_a_change_uncleared_and_its_late_red_is_kept() {
    bound_story(None);
}
#[test]
fn cold_restarts_keep_the_bound_uncleared_pulls_held_release_and_late_red() {
    for cut in [
        BoundCut::Waiting,
        BoundCut::Uncleared,
        BoundCut::LogPulls,
        BoundCut::HeldStatus,
        BoundCut::Released,
        BoundCut::Red,
    ] {
        bound_story(Some(cut));
    }
}
