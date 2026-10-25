//! The welcome's paths to a first chat on `box --fake-backends`, sent as the acts its buttons send.
use crate::support::{
    self, Ended, boxed,
    ctl::{self, Call, Step},
};
use serde_json::Value;
use skein_io::kernel::Exit;
use std::path::{Path, PathBuf};

/// One fake box on an empty configuration, as a person starts it.
struct Walk {
    scratch: support::Scratch,
    paths: [PathBuf; 3],
    bundle: PathBuf,
}

enum Act {
    Now(Call),
    After(ctl::NextCall),
    /// Follow the preceding act's printed address through the box's supervised peers.
    Visit,
}

impl Walk {
    fn new(case: &str) -> Self {
        let scratch = support::Scratch::new(case);
        let paths = ["config", "data", "cache"].map(|name| scratch.path().join(name));
        for path in &paths {
            super::private(path);
        }
        let bundle = scratch.path().join("loopback-bundle");
        // This process story sends acts through ctl; only the bundle's file shape is needed.
        super::bundle(&bundle, super::SOURCE_PAGE);
        Self { scratch, paths, bundle }
    }

    fn sandbox(&self) -> PathBuf {
        self.paths[1].join("box/fake-backends")
    }

    fn key(&self) -> PathBuf {
        self.scratch.path().join("model-key-paste")
    }

    /// The Model step's key way: copy the key the box leaves for the person, and add it.
    fn add_key(&self) -> Act {
        let source = self.sandbox().join("data/llm.key");
        let paste = self.key();
        Act::After(Box::new(move |_| {
            super::write(&paste, &std::fs::read(&source).expect("box's fake model key"));
            Call::new(&["add-key", "1", path(&paste), "--deployment", "--key", "walk-model", "--json"])
        }))
    }

    fn run(&self, acts: Vec<Act>) -> Seen {
        let count = acts.len();
        let mut steps = vec![Step::Run(Call::sign_in())];
        steps.extend(acts.into_iter().map(|act| match act {
            Act::Now(call) => Step::Run(call),
            Act::Visit => Step::Visit,
            Act::After(next) => Step::Then(Box::new(move |seen| {
                let (_, answers) = seen.split_first().expect("signed in before the welcome's acts");
                next(answers)
            })),
        }));
        let outcome = boxed::run_ctl_with_environment(
            self.scratch.path(),
            super::flags(&self.paths[0], &self.paths[1], &self.paths[2], Some(&self.bundle)),
            self.scratch.path(),
            steps,
            super::DEADLINE,
            || (),
            vec![("PATH".into(), "/usr/bin:/bin".into())],
        );
        let diagnostic = String::from_utf8_lossy(&outcome.stderr);
        let last = outcome.calls.iter().rev().find(|call| call.exit == Some(Exit::Code(0)));
        let last = last.map_or_else(String::new, |call| String::from_utf8_lossy(&call.stdout).into_owned());
        for (index, call) in outcome.calls.iter().enumerate() {
            assert_eq!(
                call.exit,
                Some(Exit::Code(0)),
                "call {index}: stdout {}; stderr {}; last successful answer: {last}; box: {diagnostic}",
                String::from_utf8_lossy(&call.stdout),
                String::from_utf8_lossy(&call.stderr)
            );
        }
        assert_eq!(outcome.calls.len(), count + 1, "all welcome acts completed; box: {diagnostic}");
        assert!(outcome.checked.is_some(), "live checkpoint reached; box: {diagnostic}");
        assert_eq!(outcome.exit, Some(Exit::Code(0)), "healthy box stop: {diagnostic}");
        assert_eq!(outcome.left, Some(Vec::new()), "no child outlives its box: {diagnostic}");
        Seen { answers: outcome.calls.into_iter().skip(1).collect(), visits: outcome.visits }
    }
}

fn path(path: &Path) -> &str {
    path.to_str().expect("fixture path")
}

fn usable(call: &Ended) -> bool {
    let document = ctl::json(call);
    document["accounts"]["usable"].as_bool().expect("account usability")
}

fn forge_number(call: &Ended, number: u16) -> Value {
    let document = ctl::json(call);
    let document = if document.get("met").is_some() { &document["forges"] } else { &document };
    document["forges"]
        .as_array()
        .expect("forge rows")
        .iter()
        .find(|row| row["number"] == number)
        .cloned()
        .expect("configured forge offered")
}

#[test]
fn forgejo_elsewhere_and_a_key_reach_a_first_chat_on_the_fake_box() {
    elsewhere::run();
}

struct Seen {
    answers: Vec<Ended>,
    visits: Vec<Vec<support::sign_in_browser::Hop>>,
}

mod chatgpt;
mod elsewhere;
mod github;

#[test]
fn chatgpt_sign_in_and_github_one_click_reach_a_first_chat_on_the_fake_box() {
    chatgpt::run();
}

#[test]
fn github_one_click_and_a_key_reach_a_first_chat_on_the_fake_box() {
    github::one_click();
}

#[test]
fn githubs_own_app_and_a_key_reach_a_first_chat_on_the_fake_box() {
    github::own_app();
}

fn accounts() -> Act {
    Act::Now(Call::new(&["accounts", "--json"]))
}
fn forges() -> Act {
    Act::Now(Call::new(&["forges", "--json"]))
}
fn wait_forges() -> Act {
    Act::Now(Call::new(&["wait", "forges", "--until", "settled", "--json"]))
}
fn adopt_github() -> Act {
    Act::After(Box::new(|seen| {
        let connected = forge_number(seen.last().expect("settled forges"), 2);
        assert_eq!(connected["state"], "connected", "{connected}");
        let [installation] = connected["installations"].as_array().expect("installations").as_slice() else {
            panic!("one installation: {connected}")
        };
        assert_eq!(installation["account"]["login"], "fake-person", "{installation}");
        assert_eq!(installation["stray"], false, "the come-back accepted the installation");
        Call::new(&[
            "adopt",
            "--project",
            "1",
            "--forge",
            "2",
            "--owner",
            "fake-person",
            "--name",
            "widget",
            "--landing",
            "main",
            "--key",
            "walk-adopt",
            "--json",
        ])
    }))
}
fn first_chat() -> Vec<Act> {
    vec![
        Act::Now(Call::new(&["wait", "accounts", "--until", "usable", "--json"])),
        Act::After(Box::new(|seen| {
            assert!(usable(seen.last().expect("usable account read")), "Ready has a usable account");
            Call::new(&["chat", "--project", "1", "--model", "gpt-6-luna", "--key", "walk-chat", "Hello", "--json"])
        })),
        Act::Now(Call::new(&["task", "1", "--json"])),
        Act::Now(Call::new(&["wait", "task", "1", "--until", "replied", "--json"])),
        Act::Now(Call::new(&["chat", "1", "--json"])),
    ]
}
fn assert_reply(seen: &Seen) {
    let chat = ctl::json(seen.answers.last().expect("committed chat"));
    let replies: Vec<_> = chat["activity"]["rows"]
        .as_array()
        .expect("conversation rows")
        .iter()
        .filter_map(|row| row["event"]["owner"]["decoded"]["turn"]["decoded"]["messages"].as_array())
        .flatten()
        .filter(|message| message["speaker"] == "assistant")
        .flat_map(|message| message["blocks"].as_array().expect("assistant blocks"))
        .filter_map(|block| block["text"]["text"].as_str())
        .collect();
    assert_eq!(replies, ["Hello from the local fake model."]);
}

fn assert_adopted(seen: &Seen, number: u16, name: &str) {
    let adopted = seen
        .answers
        .iter()
        .filter_map(|answer| serde_json::from_slice::<Value>(&answer.stdout).ok())
        .find_map(|value| value.get("adopted").cloned())
        .expect("first repository durably adopted");
    assert_eq!(adopted["forge"], number);
    assert_eq!(adopted["owner"], "fake-person");
    assert_eq!(adopted["name"], name);
    assert_eq!(adopted["home"], true);
}
