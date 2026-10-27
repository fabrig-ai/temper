//! The shipped box, first repository and first landed task across a restart.
#[expect(
    dead_code,
    clippy::unused_self,
    clippy::struct_field_names,
    reason = "shared process support includes unused fixtures for other stories"
)]
mod support;

use serde_json::{Value, json};
use skein_io::kernel::Exit;
use skein_lib::Duration;
use std::{fs, num::NonZeroU16, os::unix::ffi::OsStrExt, path::Path};
use support::{
    Scratch, boxed,
    ctl::{self, Call, Step},
    first_repository,
};

fn flags(root: &Path) -> Vec<String> {
    support::configuration::flags(root).into_iter().map(|flag| flag.into_string().expect("scratch UTF-8")).collect()
}
fn generated(root: &Path) {
    use temper_init::{Content, Directories, Facts, Ports};
    let addresses = support::free_addresses::<5>(&[]);
    let ports = addresses.map(|address| NonZeroU16::new(address.port()).expect("ephemeral port"));
    let answer = temper_init::files(
        &Facts {
            binary: skein_scratch::paths::binary("temper").as_os_str().as_bytes(),
            git: Some(b"/usr/bin/git"),
            claude: None,
            login: Some(b"qa-person"),
            ports: Ports { clients: ports[0], owned: ports[1], forge: ports[2], llm: ports[3], webhooks: ports[4] },
        },
        &Directories { config: root.as_os_str().as_bytes(), data: root.as_os_str().as_bytes() },
        &[7; 32],
        &[8; 32],
    )
    .expect("generated deployment");
    for file in answer.files {
        if let Content::Bytes { bytes, .. } = file.content {
            let path = root.join(std::ffi::OsStr::from_bytes(file.name));
            fs::create_dir_all(path.parent().expect("file parent")).expect("fixture directories");
            support::write_private(&path, &bytes);
        }
    }
    let path = root.join("temper.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&path).expect("template")).expect("JSON");
    // Keep the owned forge only: this story never connects an external service.
    for member in ["forges", "identities", "setup"] {
        document["application"][member]
            .as_array_mut()
            .expect("template list")
            .retain(|row| row[if member == "forges" { "number" } else { "forge" }] == 3);
    }
    document["application"]["demo"] = Value::Null;
    support::write_private(&path, &serde_json::to_vec(&document).expect("JSON"));
}
fn stage_binaries(root: &Path) {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    let (forgejo, runner) = support::owned::binaries();
    for (source, relative) in [
        (forgejo, format!("forgejo/{VERSION}/forgejo")),
        (runner, format!("forgejo-runner/{RUNNER_VERSION}/forgejo-runner")),
    ] {
        let target = root.join(relative);
        fs::create_dir_all(target.parent().expect("cache parent")).expect("cache directory");
        fs::copy(source, target).expect("stage pinned binary without network");
    }
}
fn assert_box<T>(outcome: &boxed::CtlOutcome<T>) {
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "box must drain: {}", String::from_utf8_lossy(&outcome.stderr));
    assert!(outcome.left.as_ref().is_none_or(Vec::is_empty), "no process may outlive the box");
}
fn rejects_orgs(member: &str) {
    let scratch = Scratch::new(&format!("qa-first-repository-{member}"));
    let root = scratch.path();
    generated(root);
    let path = root.join("temper.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&path).expect("template")).expect("JSON");
    if member == "person.orgs" {
        document["application"]["setup"][0]["mode"]["owned"]["person"]["orgs"] = json!(["work"]);
    } else {
        document["application"]["setup"][0]["orgs"] = json!(["work"]);
    }
    support::write_private(&path, &serde_json::to_vec(&document).expect("JSON"));
    let ended = support::owned::prepare(root, 3);
    let error = String::from_utf8_lossy(&ended.stderr);
    assert_eq!(ended.status.code(), Some(1), "an obsolete owned org list must refuse command startup: {error}");
    assert!(error.contains("orgs") && error.contains("setup"), "refusal must name the obsolete member's path: {error}");
    scratch.remove();
}

#[test]
fn owned_person_orgs_are_refused_at_start() {
    // protocol/setup.md, 8: an owned entry's person.orgs is an unknown member.
    rejects_orgs("person.orgs");
}
#[test]
fn owned_entry_orgs_are_refused_at_start() {
    // protocol/setup.md, 8: the older entry-level orgs is also an unknown member.
    rejects_orgs("orgs");
}

#[test]
fn first_landed_task_and_adoption_survive_a_box_restart_without_setup_replay() {
    // protocol/setup.md, 6.7 and 13.4; domain/forge.md, 8.8: first intent is volatile, adopted work and its landed task survive.
    let scratch = Scratch::new("qa-first-repository-restart");
    let root = scratch.path();
    generated(root);
    stage_binaries(root);
    let mut steps = vec![
        Step::Run(Call::sign_in()),
        Step::Run(Call::new(&["forge", "setup", "3"])),
        Step::Run(Call::new(&["forge", "list", "--json"])),
    ];
    steps.extend(first_repository::task_steps(root));
    let first = boxed::run_ctl_with_llm(
        root,
        flags(root),
        root,
        steps,
        Duration::from_secs(180),
        || first_repository::live(root, root, root),
        first_repository::llm(root),
    );
    assert_box(&first);
    let stderr = String::from_utf8_lossy(&first.stderr);
    let evidence = first.checked.as_ref().expect("first task evidence");
    first_repository::assert_task("first", &first.calls[3..], &stderr, evidence);
    let adoption = String::from_utf8_lossy(&first.calls[1].stdout);
    assert!(
        adoption.contains("work/hello") && adoption.contains("owned"),
        "first setup must adopt work/hello: {adoption}"
    );
    let rows = ctl::json(&first.calls[2]);
    assert!(rows.to_string().contains("made"), "first row must expose made before restart: {rows}");
    assert!(!root.join("forge-3/admin_token").exists(), "the handover must be consumed");
    for member in ["api_token", "git_token", "user", "webhook_secret"] {
        assert!(
            !root.join("forge-3").join(member).exists(),
            "owned credentials must remain records, never {member} files"
        );
    }
    let mut steps = vec![
        Step::Run(Call::new(&["forge", "list", "--until", "settled", "--timeout", "60s"])),
        Step::Run(Call::new(&["forge", "list", "--json"])),
        Step::Run(Call::new(&["forge", "setup", "3"])),
    ];
    steps.extend(first_repository::found_steps());
    let restarted = boxed::run_ctl_with_exits(
        root,
        flags(root),
        root,
        steps,
        Duration::from_secs(120),
        || first_repository::live(root, root, root),
        &[(2, 2)],
    );
    assert_box(&restarted);
    let stderr = String::from_utf8_lossy(&restarted.stderr);
    first_repository::assert_task(
        "restarted",
        &restarted.calls[3..],
        &stderr,
        restarted.checked.as_ref().expect("restored evidence"),
    );
    let rows = ctl::json(&restarted.calls[1]);
    let rows = rows["forges"].as_array().unwrap_or_else(|| panic!("forges rows: {rows}"));
    let row = rows.iter().find(|row| row["number"] == 3 || row["forge"] == 3).expect("owned row");
    assert!(row["first"].is_null(), "a restart must forget the first repository row: {row}");
    assert!(
        String::from_utf8_lossy(&restarted.calls[2].stdout).contains("not offered")
            || String::from_utf8_lossy(&restarted.calls[2].stderr).contains("not offered"),
        "setup after restart must not create or adopt again"
    );
    scratch.remove();
}
