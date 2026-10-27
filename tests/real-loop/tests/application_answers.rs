//! Retained adoption answers fill the table without trapping ctl in Busy retries.
use temper_real_loop_world::{
    Exit, argv,
    calls::{self, D1},
    forge, landing, run_forge,
};

fn fixture() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (landing::fixture().configure)(value, agent, forge);
            value["application"]["limits"]["overrides"]["forge.applications"] = 4.into();
        },
        ..landing::fixture()
    }
}
fn adopt(name: &'static str, key: &'static str, home: bool) -> calls::Step {
    let mut args =
        argv!["adopt", "--project", "1", "--owner", "acme", "--name", name, "--landing", "main", "--key", key];
    if !home {
        args.0.push("--no-home".into());
    }
    calls::call(D1, args)
}

#[test]
fn a_fifth_adoption_is_refused_at_once_and_its_key_replays_without_a_pending_request() {
    let ran = run_forge(
        fixture(),
        |at| landing::scripts(at, u64::from(forge::ID), landing::README),
        || {
            vec![
                calls::sign_in(D1),
                adopt("app", "setup-adopt", true),
                adopt("absent", "first-refused", false),
                adopt("app", "successful", false),
                calls::call(D1, argv!["chat", "--project", "1", "--key", "chat", "@forge_chat Land a small change"]),
                calls::call(D1, argv!["wait", "task", "1", "--until", "replied"]),
                calls::call(D1, argv!["wait", "task", "2", "--until", "ended"]),
                calls::call(D1, argv!["wait", "task", "1", "--until", "ended"]),
                adopt("absent", "second-refused", false),
                adopt("app", "full", false),
                adopt("app", "full", false),
                calls::call(D1, argv!["pending", "--json"]),
                calls::call(D1, argv!["task", "2", "--json"]),
            ]
        },
    );
    for (index, call) in ran.calls().iter().enumerate() {
        let code = if [2, 8, 9, 10].contains(&index) { 2 } else { 0 };
        assert_eq!(
            call.exit,
            Some(Exit::Code(code)),
            "call {index}: {} {}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    let words = b"temper keeps at most 4 answers to requests like this within a day: ask again later, under a new key";
    for index in [9, 10] {
        let call = &ran.calls()[index];
        assert!(
            call.ended.as_nanos().saturating_sub(call.spawned.as_nanos())
                < skein_lib::Duration::from_secs(5).as_nanos(),
            "capacity is refused promptly, without waiting for retry retention"
        );
        assert!(
            call.stderr.windows(words.len()).any(|value| value == words),
            "call {index}: {}",
            String::from_utf8_lossy(&call.stderr)
        );
    }
    let pending: serde_json::Value = serde_json::from_slice(&ran.calls()[11].stdout).expect("pending JSON");
    assert_eq!(pending["pending"].as_array().expect("pending rows").len(), 0);
    assert!(ran.forge().merged_green(), "the same story landed before its fifth adoption");
    assert!(ran.forge().main_file(b"README.md").ends_with(landing::EDIT));
}
