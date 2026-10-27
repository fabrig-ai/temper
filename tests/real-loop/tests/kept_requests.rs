//! A retained adoption at capacity cannot hold another act in its ctl directory.
use skein_fake_llm_domain::api::{Finish, Line, Piece, Script, Turn};
use temper_real_loop_world::{
    self as world, Exit, argv,
    calls::{self, D1},
    forge, landing,
};

fn fixture() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, at| {
            (landing::fixture().configure)(value, agent, at);
            // This story owns the old capacity regardless of the shipped preset.
            value["application"]["limits"]["overrides"]["forge.applications"] = 4.into();
        },
        ..landing::fixture()
    }
}
fn adopt(repository: &'static str, key: &'static str) -> calls::Step {
    calls::call(
        D1,
        argv!["adopt", "--project", "1", "--owner", "acme", "--name", repository, "--landing", "main", "--key", key],
    )
}
fn parked() -> Turn {
    Turn {
        lines: Box::new([Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn scripts(at: std::net::SocketAddr) -> Box<[Script]> {
    let mut scripts = landing::scripts(at, u64::from(forge::ID), landing::README);
    let mut turns = scripts[0].turns.to_vec();
    let finish = turns.pop().expect("chat finish");
    turns.push(parked());
    turns.push(finish);
    scripts[0].turns = turns.into_boxed_slice();
    scripts
}
#[test]
fn an_unanswered_fifth_adoption_holds_no_close_or_export_after_landing() {
    const SIGNED_IN: usize = 0;
    const ADOPTED: usize = 1;
    const REFUSED: usize = 2;
    const ADOPTED_AGAIN: usize = 3;
    const CHAT_CREATED: usize = 4;
    const CHAT_REPLIED: usize = 5;
    const LANDED: usize = 6;
    const REFUSED_AGAIN: usize = 8;
    const KEPT_ADOPTION: usize = 9;
    const PENDING_BEFORE: usize = 10;
    const CLOSED: usize = 11;
    const EXPORTED: usize = 12;
    const PENDING_AFTER: usize = 13;
    const ENDED: usize = 14;
    let ran = world::run_forge(fixture(), scripts, || {
        vec![
            calls::sign_in(D1),          // 0
            adopt("app", "setup-adopt"), // 1
            adopt("absent", "refused"),  // 2
            adopt("app", "successful"),  // 3
            calls::call(D1, argv!["chat", "--project", "1", "--key", "land", "@forge_chat Land a small change"]), // 4
            calls::call(D1, argv!["wait", "task", "1", "--until", "replied"]), // 5
            calls::call(D1, argv!["wait", "change", "2", "--until", "landed"]), // 6
            calls::call(D1, argv!["wait", "task", "1", "--until", "replied"]), // 7
            adopt("absent", "refused-again"), // 8
            calls::start(
                D1,
                argv![
                    "adopt",
                    "--project",
                    "1",
                    "--owner",
                    "acme",
                    "--name",
                    "app",
                    "--landing",
                    "main",
                    "--key",
                    "repeat"
                ],
            ), // 9
            // Observe the durable request before ending the invocation; no 30 s timeout is needed.
            calls::until(D1, argv!["pending"], b"adopt, party 1, first sent ", b"not sent"), // 10
            calls::kill(KEPT_ADOPTION),
            calls::join(KEPT_ADOPTION),
            calls::call(D1, argv!["close", "1", "--key", "close"]), // 11
            // The fixture's account is a key: a prompt 'not signed in' refusal proves admission.
            calls::call(D1, argv!["export", "1", calls::file("returned")]), // 12
            calls::call(D1, argv!["pending"]),                              // 13
            calls::call(D1, argv!["wait", "task", "1", "--until", "ended"]), // 14
        ]
    });
    for (index, status) in [
        (SIGNED_IN, 0),
        (ADOPTED, 0),
        (REFUSED, 2),
        (ADOPTED_AGAIN, 0),
        (CHAT_CREATED, 0),
        (CHAT_REPLIED, 0),
        (LANDED, 0),
        (REFUSED_AGAIN, 2),
        (CLOSED, 0),
        (EXPORTED, 2),
        (ENDED, 0),
    ] {
        assert_eq!(ran.calls()[index].exit, Some(Exit::Code(status)), "call {index}: {:?}", ran.calls()[index]);
    }
    assert!(ran.forge().merged_green());
    assert!(ran.forge().main_file(b"README.md").ends_with(landing::EDIT));
    assert_eq!(
        ran.calls()[PENDING_BEFORE].stdout,
        ran.calls()[PENDING_AFTER].stdout,
        "close and export preserve the kept adoption"
    );
    for index in [CLOSED, EXPORTED] {
        let call = &ran.calls()[index];
        assert!(call.ended.as_nanos() - call.spawned.as_nanos() < 2_000_000_000, "prompt answer: {call:?}");
        assert!(calls::occurrences(&call.stderr, b"kept: adopt, first sent ") == 1, "{call:?}");
        assert!(
            calls::occurrences(
                &call.stderr,
                b"unanswered: it clears when the engine answers it, or past its retention; pending lists it"
            ) == 1
        );
    }
    assert!(calls::occurrences(&ran.calls()[EXPORTED].stderr, b"not signed in") == 1);
}

fn cancelling_scripts(at: std::net::SocketAddr) -> Box<[Script]> {
    let mut scripts = landing::scripts(at, u64::from(forge::ID), landing::README);
    scripts[1].turns[0] = Turn {
        lines: Box::new([Line::Pieces {
            pieces: Box::new([
                Piece { text: b"Producing".as_slice().into(), hold: None },
                Piece { text: b" held".as_slice().into(), hold: Some(b"producing".as_slice().into()) },
            ]),
        }]),
        finish: Finish::Stop,
        tokens: 20,
    };
    scripts
}
#[test]
fn cancelling_a_change_answers_its_landing_wait_with_the_last_panel() {
    const LANDING_WAIT: usize = 5;
    let ran = world::run_forge(fixture(), cancelling_scripts, || {
        vec![
            calls::sign_in(D1),
            adopt("app", "adopt"),
            calls::call(D1, argv!["chat", "--project", "1", "--key", "change", "@forge_chat Make a change"]),
            calls::call(D1, argv!["wait", "task", "1", "--until", "replied"]), // 3
            calls::call(D1, argv!["change", "2", "--json"]),                   // 4
            calls::start(D1, argv!["wait", "change", "2", "--until", "landed", "--timeout", "10s", "--json"]), // 5
            calls::watching(LANDING_WAIT),
            calls::call(D1, argv!["close", "1", "--key", "close"]), // 6
            calls::release_hold(b"producing"),
            calls::call(D1, argv!["wait", "task", "2", "--until", "ended"]), // 7
            calls::join(LANDING_WAIT),
            calls::call(D1, argv!["wait", "task", "1", "--until", "ended"]),
        ]
    });
    assert_eq!(ran.calls()[LANDING_WAIT].exit, Some(Exit::Code(3)));
    let answer: serde_json::Value = serde_json::from_slice(&ran.calls()[LANDING_WAIT].stdout).expect("wait JSON");
    assert_eq!(answer["met"], false);
    assert_eq!(answer["why"], "ended: cancelled");
    assert_eq!(answer["change"]["step"], "producing");
    assert!(answer["change"]["digest"].is_string());
    let wait = &ran.calls()[LANDING_WAIT];
    assert!(wait.ended.as_nanos() - wait.spawned.as_nanos() < 2_000_000_000, "{wait:?}");
}
