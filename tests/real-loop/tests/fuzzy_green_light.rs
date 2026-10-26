//! The person answers the work's question before its CSV change starts.
use temper_real_loop_world::{green_light, run_forge};

#[test]
fn a_green_light_in_the_inbox_lets_the_change_land() {
    let ran = run_forge(green_light::fixture(), green_light::scripts, green_light::acts);
    green_light::check(&ran);
}
