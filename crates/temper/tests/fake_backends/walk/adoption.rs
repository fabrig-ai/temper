//! QA pass 20261027-0037: context leaves the writer alone; setup keys replay.
use super::{Call, Walk, boxed, ctl};
use skein_io::kernel::Exit;

fn adopt(project: &str, role: &str, key: &str) -> Call {
    Call::new(&[
        "repository", "adopt",
        "--project",
        project,
        "--forge",
        "3",
        "--owner",
        "playground",
        "--name",
        "widget",
        "--landing",
        "main",
        "--role",
        role,
        "--key",
        key,
        "--json",
    ])
}

#[test]
fn context_preserves_the_writer_and_a_refused_setup_preserves_the_first_key() {
    let walk = Walk::owned("walk-context-and-setup-replay");
    let setup = Call::new(&["forge", "setup", "3", "--playground", "--key", "first", "--json"]);
    let outcome = boxed::run_ctl_with_exits(
        walk.scratch.path(),
        super::super::flags(&walk.paths[0], &walk.paths[1], &walk.paths[2], Some(&walk.bundle)),
        walk.scratch.path(),
        vec![
            ctl::Step::Run(Call::sign_in()),
            ctl::Step::Run(setup.clone()),
            ctl::Step::Run(setup.clone()),
            ctl::Step::Run(adopt("1", "owned", "other")),
            ctl::Step::Run(adopt("1", "context", "reader")),
            ctl::Step::Run(adopt("2", "owned", "writer")),
            ctl::Step::Run(Call::new(&["forge", "list", "--json"])),
            ctl::Step::Run(Call::new(&["forge", "setup", "3", "--playground", "--key", "second", "--json"])),
            ctl::Step::Run(setup),
            ctl::Step::Run(Call::new(&["forge", "list", "--json"])),
        ],
        crate::BOX_DEADLINE,
        || (),
        &[(3, 2), (7, 2)],
    );
    let diagnostic = String::from_utf8_lossy(&outcome.stderr);
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "box drained: {diagnostic}");
    assert_eq!(outcome.left, Some(Vec::new()), "no child outlives its box: {diagnostic}");
    let [signed, first, replay, other, reader, writer, before, refused, replay_after, after] = outcome.calls.as_slice()
    else {
        panic!("all ten ctl calls completed: {diagnostic}")
    };
    for call in [signed, first, replay, reader, writer, before, replay_after, after] {
        assert_eq!(
            call.exit,
            Some(Exit::Code(0)),
            "{} {}; box: {diagnostic}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&other.stderr).expect("refusal JSON"),
        serde_json::json!({"forge_refused":{"reason":"another_project"}})
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&refused.stderr).expect("refusal JSON"),
        serde_json::json!({"refused":{"why":"not_offered"}})
    );
    assert_eq!(other.exit, Some(Exit::Code(2)));
    assert_eq!(refused.exit, Some(Exit::Code(2)));
    let answers = serde_json::Deserializer::from_slice(&first.stdout)
        .into_iter::<serde_json::Value>()
        .collect::<Result<Vec<_>, _>>()
        .expect("setup JSON");
    assert_eq!(answers.len(), 2, "progress and adoption");
    assert_eq!(answers[0], serde_json::json!({"setting_up":{}}));
    assert_eq!(answers[1]["adopted"]["owner"], "playground");
    assert_eq!(answers[1]["adopted"]["name"], "widget");
    assert_eq!(answers[1]["adopted"]["home"], true);
    assert_eq!(first.stdout, replay.stdout, "first key replays before the refusal");
    assert_eq!(first.stdout, replay_after.stdout, "first key replays after the refusal");
    let context = ctl::json(reader);
    assert_eq!(context["adopted"]["role"], "context");
    assert_eq!(context["adopted"]["home"], false);
    let owned = ctl::json(writer);
    assert_eq!(owned["adopted"]["role"], "owned");
    assert_eq!(owned["adopted"]["home"], true);
    assert_eq!(context["adopted"]["repository"], owned["adopted"]["repository"]);
    assert_eq!(ctl::json(before), ctl::json(after), "setup refusal/replay leaves forge and first repository unchanged");
    walk.scratch.remove();
}
