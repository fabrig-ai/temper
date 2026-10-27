//! A closed chat's retained reads and keyed close, through the shipped box and ctl.
use super::{bundle, flags, private, support};
use skein_io::kernel::Exit;
use support::ctl::{self, Call, Step};

#[test]
fn a_closed_chat_keeps_its_turns_and_replays_its_first_close() {
    let scratch = support::Scratch::new("fake-backends-closed-chat");
    let paths = ["config", "data", "cache"].map(|name| scratch.path().join(name));
    for path in &paths {
        private(path);
    }
    let supplied = scratch.path().join("bundle");
    bundle(&supplied, super::SOURCE_PAGE);
    let key = paths[1].join("box/fake-backends/data/llm.key");
    let outcome = support::boxed::run_ctl_with_exits(
        scratch.path(),
        flags(&paths[0], &paths[1], &paths[2], Some(&supplied)),
        scratch.path(),
        vec![
            Step::Run(Call::sign_in()),
            Step::Run(Call::new(&[
                "account",
                "add-key",
                "1",
                key.to_str().expect("fake key path"),
                "--deployment",
                "--key",
                "model",
            ])),
            Step::Run(Call::new(&["account", "list", "--until", "usable", "--timeout", "20s"])),
            Step::Run(Call::new(&["chat", "start", "--project", "1", "hello retained chat", "--key", "chat"])),
            Step::Run(Call::new(&["task", "show", "1", "--until", "replied", "--timeout", "20s"])),
            Step::Run(Call::new(&["chat", "show", "1", "--json"])),
            Step::Run(Call::new(&["chat", "close", "1", "--reason", "finished", "--key", "close"])),
            Step::Run(Call::new(&["task", "show", "1", "--until", "ended", "--timeout", "20s"])),
            Step::Run(Call::new(&["chat", "show", "1"])),
            Step::Run(Call::new(&["chat", "show", "1", "--json"])),
            Step::Run(Call::new(&["task", "show", "1", "--run", "--json"])),
            Step::Run(Call::new(&["chat", "close", "1", "--reason", "finished", "--key", "close"])),
            Step::Run(Call::new(&["chat", "close", "1", "--reason", "finished", "--key", "close-2"])),
        ],
        crate::BOX_DEADLINE,
        || (),
        &[(12, 2)],
    );
    let diagnostic = String::from_utf8_lossy(&outcome.stderr);
    for (index, call) in outcome.calls.iter().enumerate() {
        assert_eq!(
            call.exit,
            Some(Exit::Code(if index == 12 { 2 } else { 0 })),
            "ctl {index}: {}; box: {diagnostic}",
            String::from_utf8_lossy(&call.stderr)
        );
    }
    assert_eq!(outcome.calls.len(), 13, "whole story completed; box: {diagnostic}");
    let before = ctl::json(&outcome.calls[5]);
    let after = ctl::json(&outcome.calls[9]);
    let turns = |document: &serde_json::Value| -> Vec<serde_json::Value> {
        document["activity"]["rows"]
            .as_array()
            .expect("conversation activity rows")
            .iter()
            .filter(|row| !row["event"]["turn"].is_null() || !row["event"]["owner"]["decoded"].is_null())
            .cloned()
            .collect()
    };
    let kept = turns(&before);
    assert!(!kept.is_empty(), "live conversation contains committed turns: {before}");
    assert!(
        turns(&after).starts_with(&kept),
        "closed conversation retains the committed turns before its final wind-down reply"
    );
    for index in [8, 9] {
        let text = String::from_utf8_lossy(&outcome.calls[index].stdout);
        assert!(text.contains("Hello from the local fake model."), "kept reply: {text}");
    }
    assert!(ctl::json(&outcome.calls[10]).get("run").is_none(), "ended task has no provisional run");
    assert_eq!(outcome.calls[11].stdout, outcome.calls[6].stdout, "same-key replay gives first answer");
    let refusal = String::from_utf8_lossy(&outcome.calls[12].stderr);
    assert!(refusal.contains("chat 1 has ended"), "new-key close refusal: {refusal}");
    assert!(outcome.checked.is_some(), "story reached final checkpoint");
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "box drains: {diagnostic}");
    assert_eq!(outcome.left, Some(Vec::new()), "no process outlives box");
    scratch.remove();
}
