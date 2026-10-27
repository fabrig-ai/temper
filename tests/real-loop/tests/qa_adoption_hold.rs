//! Home adoption's task boundary, domain/forge.md, 11. The real-loop
//! public harness has Forgejo only; this rule is independent of the forge.
use serde_json::json;
use skein_fake_forge_domain::api;
use skein_fake_llm_domain::api::{Finish, Line, Part, Script, Turn};
use skein_fake_peers::llm::Observation;
use std::cell::RefCell;
use temper_real_loop_world::{
    self as world, argv,
    calls::{self, D1},
    forge, landing,
};

thread_local! {
    static FORGE: RefCell<Option<temper_fake_git_hosted::Typed>> = const { RefCell::new(None) };
}
fn capture_forge() {
    FORGE.with(|slot| *slot.borrow_mut() = Some(world::git::transport()));
}
fn branch_file() -> Option<Vec<u8>> {
    let transport = FORGE.with(|slot| slot.borrow_mut().take().expect("captured public git transport"));
    let forge = transport.forge.borrow();
    assert!(
        forge
            .domain
            .inspect(&forge.config, b"acme/app", &api::Read::Branch { branch: b"main".as_slice().into() })
            .is_ok()
    );
    match forge.domain.inspect(
        &forge.config,
        b"acme/app",
        &api::Read::Branch { branch: b"temper/1/r1".as_slice().into() },
    ) {
        Ok(api::Answer::Commit(head)) => Some(
            forge
                .domain
                .object(head)
                .expect("branch object")
                .tree
                .get(b"README.md".as_slice())
                .expect("README in branch")
                .to_vec(),
        ),
        Ok(other) => panic!("branch read: {other:?}"),
        Err(_) => None,
    }
}

fn fixture() -> forge::Fixture {
    forge::Fixture { configure: landing::configure, ..forge::fixture() }
}
fn tool(name: &[u8], arguments: &serde_json::Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(arguments).expect("tool JSON").into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn parked(text: &[u8]) -> Turn {
    Turn {
        lines: Box::new([
            Line::Text { text: text.into() },
            Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn adopt() -> calls::Step {
    calls::call(
        D1,
        argv![
            "repository",
            "adopt",
            "--project",
            "1",
            "--forge",
            "1",
            "--owner",
            "acme",
            "--name",
            "app",
            "--landing",
            "main",
            "--key",
            "adopt-home"
        ],
    )
}
fn read_was_answered(ran: &world::Ran) {
    for call in ran.calls() {
        assert_eq!(
            call.exit,
            Some(world::Exit::Code(0)),
            "ctl: {} {}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    let mut read = false;
    for observed in ran.llm() {
        if let Observation::Query { query, .. } = observed {
            for message in &query.messages {
                for part in &message.parts {
                    if let Part::ToolOutput { output, is_error, .. } = part {
                        assert!(
                            !is_error && !output.starts_with(b"Error:"),
                            "tool refused: {}",
                            String::from_utf8_lossy(output)
                        );
                        read |= output.windows(b"# app".len()).any(|bytes| bytes == b"# app");
                    }
                }
            }
        }
    }
    assert!(read, "the model received README content from the adopted home");
}

fn old_chat() -> world::Ran {
    world::run_forge(
        fixture(),
        |_| {
            capture_forge();
            Box::new([Script {
                cue: b"@old".as_slice().into(),
                turns: Box::new([
                    parked(b"Parked before adoption"),
                    tool(b"read", &json!({"path":"README.md"})),
                    parked(b"Read the newly adopted home"),
                ]),
            }])
        },
        || {
            vec![
                calls::sign_in(D1),
                calls::call(D1, argv!["chat", "start", "--project", "1", "--key", "old-chat", "@old Wait for a home"]),
                calls::call(D1, argv!["task", "show", "1", "--until", "replied"]),
                adopt(),
                calls::call(D1, argv!["task", "message", "1", "Read the home now", "--key", "wake-old"]),
                calls::call(D1, argv!["task", "show", "1", "--until", "replied"]),
            ]
        },
    )
}

#[test]
fn a_chat_created_before_home_adoption_reads_without_a_run_branch() {
    let ran = old_chat();
    read_was_answered(&ran);
    assert!(branch_file().is_none(), "an older task must not push a run branch");
}

#[test]
fn an_older_tasks_brief_explains_its_home_adoption_hold() {
    let ran = old_chat();
    read_was_answered(&ran);
    let brief = ran
        .llm()
        .iter()
        .rev()
        .find_map(|observed| {
            if let Observation::Query { query, .. } = observed {
                Some(String::from_utf8_lossy(&query.system).to_ascii_lowercase())
            } else {
                None
            }
        })
        .expect("resumed chat's brief");
    // The design specifies the reason, not its exact wording. Accept either
    // adoption named directly or the home and the relative task/home ordering.
    let explains = brief.contains("adopt")
        || (brief.contains("home")
            && ["before", "after", "earlier", "older", "predat"].iter().any(|word| brief.contains(word)));
    assert!(
        explains,
        "older task's brief must explain why the later-adopted home has no writable run branch; only a read-only checkout is described"
    );
}

#[test]
#[ignore = "a chat's deliver before its end is not built: STATUS.md, domain/agent.md, 4.4"]
fn a_chat_created_after_home_adoption_gets_a_writable_run_branch() {
    let ran = world::run_forge(
        fixture(),
        |_| {
            capture_forge();
            Box::new([Script {
                cue: b"@new".as_slice().into(),
                turns: Box::new([
                    tool(b"read", &json!({"path":"README.md"})),
                    tool(b"edit", &json!({"path":"README.md","edits":[{"old":"# app\n","new":"# app\nQA edit\n"}]})),
                    tool(b"deliver", &json!({"title":"QA home edit","body":"Explicit mid-run delivery"})),
                    parked(b"Delivered the adopted home"),
                ]),
            }])
        },
        || {
            vec![
                calls::sign_in(D1),
                adopt(),
                calls::call(D1, argv!["chat", "start", "--project", "1", "--key", "new-chat", "@new Read the home"]),
                calls::call(D1, argv!["task", "show", "1", "--until", "replied"]),
            ]
        },
    );
    read_was_answered(&ran);
    assert_eq!(
        branch_file().as_deref(),
        Some(b"# app\nQA edit\n".as_slice()),
        "a new task pushes its edited home to its run branch"
    );
}
