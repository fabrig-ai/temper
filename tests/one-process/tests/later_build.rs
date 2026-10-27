use temper_one_process_world::{golden::golden, later::assert_later};

#[test]
fn a_later_build_keeps_what_the_golden_deployment_holds() {
    let (kept, ledger) = golden();
    assert_later(&kept, ledger);
}
