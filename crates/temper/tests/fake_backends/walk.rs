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

    fn sandbox(&self) -> PathBuf {
        self.paths[1].join("box/fake-backends")
    }

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
        let outcome = boxed::run_ctl(
            self.scratch.path(),
            super::flags(&self.paths[0], &self.paths[1], &self.paths[2], Some(&self.bundle)),
            self.scratch.path(),
            steps,
            super::DEADLINE,
            || (),
        );
        let diagnostic = String::from_utf8_lossy(&outcome.stderr);
        for (index, call) in outcome.calls.iter().enumerate() {
            assert_eq!(
                call.exit,
                Some(Exit::Code(0)),
                "call {index}: stdout {}; stderr {}; box: {diagnostic}",
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

#[test]
fn forgejo_elsewhere_and_a_key_reach_a_first_chat_on_the_fake_box() {
    let walk = Walk::new("walk-elsewhere-key");
    let admin = walk.sandbox().join("data/forgejo-admin.token");
    // Reduced to the first failing welcome act: Connect's adoption returns no_access.
    let _seen = walk.run(vec![
        Act::Now(Call::new(&["accounts", "--json"])),
        Act::Now(Call::new(&["forges", "--json"])),
        Act::After(Box::new(move |seen| {
            let [before_model, before_forge] = seen else { panic!("welcome preconditions read") };
            assert!(!usable(before_model), "no model before the key: the welcome must offer its Model step");
            let offered = forge(before_forge);
            assert_eq!(offered["mode"], "managed");
            assert_eq!(offered["state"], "not_connected");
            assert!(offered["first"].is_null(), "no first repository adopted before Connect");
            Call::new(&[
                "forge-connect",
                "4",
                "forgejo-admin",
                path(&admin),
                "--first",
                "fake-person/widget",
                "--key",
                "walk-forge",
                "--json",
            ])
        })),
    ]);
    walk.scratch.remove();
}
