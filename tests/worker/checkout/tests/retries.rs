use temper_worker_checkout_world::merges;

#[test]
fn a_second_hold_delivers_the_existing_readme_diff_without_an_extra_commit() {
    for (movement, ambiguous) in [(None, false), (None, true), (Some(true), false), (Some(false), false)] {
        merges::retry_existing_change(movement, ambiguous);
    }
}
