//! Argument refusals never start a browser or a box (client/journeys.md, 6).
use std::process::Command;

#[test]
fn again_without_a_name_is_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_temper-client-journeys"))
        .arg("again")
        .output()
        .expect("run the journeys program");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("usage is text"),
        "usage: temper-client-journeys [add NAME FILE | again NAME]\n"
    );
}
