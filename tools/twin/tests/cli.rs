//! The binary writes only validated twins and prints one-line refusals.
use serde_json::json;
use skein_scratch::Scratch;
use std::{path::Path, process::Command};

fn base(scratch: &Scratch) -> std::path::PathBuf {
    let file = scratch.path().join("base.json");
    std::fs::write(
        &file,
        serde_json::to_vec(&json!({
            "listen":{"socket":"127.0.0.1:0"}, "key":"/twin/key", "tls":null,
            "models":[{"name":"fake","window":65536,"output":16384}],
            "cued":[],"asked":[],"otherwise":["off script"]
        }))
        .expect("base JSON"),
    )
    .expect("base file");
    file
}
#[test]
fn a_conversion_refusal_exits_two_without_opening_the_output() {
    let scratch = Scratch::new("twin-cli-refusal");
    let base = base(&scratch);
    let out = scratch.path().join("out.json");
    let result = Command::new(env!("CARGO_BIN_EXE_twin"))
        .arg("--data")
        .arg(scratch.path())
        .arg("--base")
        .arg(base)
        .arg("--out")
        .arg(&out)
        .args(["--cue", "9=missing task"])
        .output()
        .expect("run twin");
    assert_eq!(result.status.code(), Some(2));
    assert!(!out.exists());
    assert!(result.stderr.starts_with(b"read transcript:"));
    assert_eq!(String::from_utf8(result.stderr).expect("stderr").lines().count(), 1);
    assert!(result.stdout.is_empty());
}
#[test]
fn the_cli_reports_tasks_and_replacement_counts() {
    let scratch = Scratch::new("twin-cli-report");
    let base = base(&scratch);
    let directory = scratch.path().join("transcripts/0000000000000000");
    std::fs::create_dir_all(&directory).expect("task directory");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/0000000000000005"),
        directory.join("0000000000000005"),
    )
    .expect("reviewer frames");
    let out = scratch.path().join("out.json");
    let result = Command::new(env!("CARGO_BIN_EXE_twin"))
        .arg("--data")
        .arg(scratch.path())
        .arg("--base")
        .arg(base)
        .arg("--out")
        .arg(&out)
        .args(["--cue", "5=Review this change at the named head", "--replace", "Checked=Reviewed"])
        .output()
        .expect("run twin");
    assert_eq!(result.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, b"task 5: 1 turns\nChecked: 1\n");
    assert!(temper_fake::llm::Document::decode(&std::fs::read(out).expect("twin document")).is_ok());
}
