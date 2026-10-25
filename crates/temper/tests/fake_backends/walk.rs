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

    /// The fake box's private sandbox, DATA/box/fake-backends.
    fn sandbox(&self) -> PathBuf {
        self.paths[1].join("box/fake-backends")
    }

    /// The private paste of the generated model key, populated when the walk starts.
    fn key(&self) -> PathBuf {
        self.scratch.path().join("model-key-paste")
    }

    fn run(&self, acts: Vec<Act>) -> Vec<Ended> {
        let count = acts.len();
        let source = self.sandbox().join("data/llm.key");
        let paste = self.key();
        let mut steps = vec![Step::Run(Call::sign_in())];
        steps.extend(acts.into_iter().enumerate().map(|(index, act)| {
            let source = source.clone();
            let paste = paste.clone();
            Step::Then(Box::new(move |seen| {
                if index == 0 {
                    super::write(&paste, &std::fs::read(&source).expect("box's fake model key"));
                }
                let (_, answers) = seen.split_first().expect("signed in before the welcome's acts");
                match act {
                    Act::Now(call) => call,
                    Act::After(next) => next(answers),
                }
            }))
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
        outcome.calls.into_iter().skip(1).collect()
    }
}

fn path(path: &Path) -> &str {
    path.to_str().expect("fixture path")
}

fn usable(call: &Ended) -> bool {
    let document = ctl::json(call);
    document["accounts"]["usable"].as_bool().expect("account usability")
}

fn forge(call: &Ended) -> Value {
    ctl::json(call)["forges"]
        .as_array()
        .expect("forge rows")
        .iter()
        .find(|row| row["number"] == 4)
        .cloned()
        .expect("managed fake Forgejo offered")
}

fn adoption(call: &Ended) -> Value {
    // Connect prints progress before the terminal project/adoption answer.
    let answer = call.stdout.split(|byte| *byte == b'\n').rfind(|line| !line.is_empty()).expect("adoption answer");
    serde_json::from_slice(answer).expect("adoption JSON")
}

fn adopted_project(seen: &[Ended]) -> String {
    let [_, _, connected, ..] = seen else { panic!("connection answered") };
    adoption(connected)["project"].as_u64().expect("adopted project number").to_string()
}

fn chat_task(seen: &[Ended]) -> String {
    let [_, _, _, _, _, _, _, started, ..] = seen else { panic!("first chat started") };
    ctl::json(started)["outcome"]["done"]["owner"]["created"]["first"].as_u64().expect("chat's task number").to_string()
}

fn assert_ready(seen: &[Ended]) {
    let [before_model, before_forge, connected, _, _, ready_model, ready_forge] = seen else {
        panic!("welcome ready reads completed")
    };
    assert!(!usable(before_model), "no model before the key: the welcome must offer its Model step");
    let offered = forge(before_forge);
    assert_eq!(offered["mode"], "managed");
    assert_eq!(offered["state"], "not_connected");
    assert!(offered["first"].is_null(), "no first repository adopted before Connect");
    let connected = adoption(connected);
    let adopted = &connected["outcome"]["adopted"];
    assert_eq!(adopted["forge"], 4);
    assert_eq!(adopted["owner"], "fake-person");
    assert_eq!(adopted["name"], "hello");
    assert_eq!(adopted["role"], "owned");
    assert_eq!(adopted["home"], true);
    assert!(connected["project"].as_u64().expect("adopted project") > 0, "repository adopted into a project");
    let ready = forge(ready_forge);
    assert_eq!(ready["state"], "connected");
    assert_eq!(ready["first"]["state"], "made");
    assert_eq!(ready["first"]["org"], "fake-person");
    assert_eq!(ready["first"]["repository"], "hello");
    assert_eq!(ready["first"]["id"], adopted["repository"]);
    assert!(usable(ready_model), "key account usable at Ready");
}

#[test]
fn forgejo_elsewhere_and_a_key_reach_a_first_chat_on_the_fake_box() {
    let walk = Walk::new("walk-elsewhere-key");
    let key = walk.key();
    let admin = walk.sandbox().join("data/forgejo-admin.token");
    // The remaining failure is the worker cloning the fake forge with real git.
    let _seen = walk.run(vec![
        Act::Now(Call::new(&["accounts", "--json"])),
        Act::Now(Call::new(&["forges", "--json"])),
        Act::Now(Call::new(&[
            "forge-connect",
            "4",
            "forgejo-admin",
            path(&admin),
            "--first",
            "fake-person/hello",
            "--key",
            "walk-forge",
            "--json",
        ])),
        Act::Now(Call::new(&["wait", "forges", "--until", "connected:4", "--timeout", "30s"])),
        Act::Now(Call::new(&["add-key", "1", path(&key), "--key", "walk-key"])),
        Act::Now(Call::new(&["wait", "accounts", "--until", "usable", "--timeout", "10s", "--json"])),
        Act::Now(Call::new(&["forges", "--json"])),
        Act::After(Box::new(|seen| {
            assert_ready(seen);
            Call::new(&["chat", "--project", &adopted_project(seen), "--key", "walk-chat", "Say hello", "--json"])
        })),
        Act::After(Box::new(|seen| Call::new(&["task", &chat_task(seen), "--json"]))),
        Act::After(Box::new(|seen| {
            Call::new(&["wait", "task", &chat_task(seen), "--until", "replied", "--timeout", "10s", "--json"])
        })),
    ]);
    walk.scratch.remove();
}
