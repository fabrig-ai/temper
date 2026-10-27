//! The landing path on Claude Code, past the focused guide (domain/testing.md, 2.4).
//! As `fuzzy_landing` does, use the system allocator for the shared-forge hosts.
use smith_fake_claude::{Seen, Selection};
use temper_real_loop_world::{
    Exit, Scratch, argv,
    calls::{D1, call, delegate, digest, occurrences, task, until},
    claude, forge, landing, run_forge_on,
};

// Seven source/account calls, then adoption, precede the chat.
const CHAT: usize = 8;
const FIRST_REPLY: usize = 10;
const TREE: usize = 11;
const LAST_REPLY: usize = 14;
const ENDED: usize = 17;
// Two chat activations, one coder and one reviewer; tools stay in their activation.
const RUNS: usize = 4;

#[test]
fn a_chat_on_claude_code_lands_its_change_on_the_forge() {
    let scratch = Scratch::new("program-landing");
    let repository = temper_engine_domain_forge::tag(1, forge::ID);
    claude::source(&scratch, claude::SOURCE, &landing::program_story(repository, landing::README))
        .expect("signed-in fake source");
    let ran = run_forge_on(
        scratch,
        landing::fixture(),
        |_| Box::new([]),
        || {
            let mut steps = claude::sign_in();
            steps.extend([
                call(
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
                        "adopt-app"
                    ],
                ),
                call(D1, argv!["chat", "--project", "1", "--key", "program-land", landing::PROGRAM_WORDS]),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "replied"]),
                until(D1, argv!["chat", task(CHAT)], landing::REQUESTED.as_bytes(), b"Change"),
                call(D1, argv!["task", task(CHAT), "--tree", "--json"]),
                call(D1, argv!["wait", "task", delegate(TREE), "--until", "ended"]),
                call(D1, argv!["wait", "task", task(CHAT), "--since", digest(TREE), "--until", "replied"]),
                until(D1, argv!["chat", task(CHAT)], landing::LANDED.as_bytes(), b"The change"),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "phase:idle"]),
                // The ordinary Open chat belongs to its person, who closes it after the reply.
                call(D1, argv!["cancel", task(CHAT), "Change landed"]),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "ended", "--json"]),
            ]);
            steps
        },
    );
    for called in ran.calls() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "{called:?}");
    }
    assert_eq!(ran.calls()[ENDED].exit, Some(Exit::Code(0)));
    assert!(ran.forge().merged_green());
    assert!(ran.forge().main_file(b"README.md").ends_with(landing::EDIT));
    let first = &ran.calls()[FIRST_REPLY].stdout;
    assert_eq!(occurrences(first, landing::REQUESTED.as_bytes()), 1, "{}", String::from_utf8_lossy(first));
    // The resumed chat read includes its retained earlier turns too.
    let conversation = &ran.calls()[LAST_REPLY].stdout;
    let text = std::str::from_utf8(conversation).expect("chat text");
    assert!(
        text.find(landing::REQUESTED).expect("first reply") < text.find(landing::LANDED).expect("last reply"),
        "{text}"
    );
    for reply in [landing::REQUESTED, landing::LANDED] {
        assert_eq!(occurrences(conversation, reply.as_bytes()), 1, "{text}");
    }
    assert!(ran.llm().is_empty(), "no direct-key request: {:?}", ran.llm());
    let bytes =
        std::fs::read(ran.path().join("programs").join(claude::SOURCE).join(smith_fake_claude::OBSERVATIONS_FILE))
            .expect("fake observations");
    let observations = smith_fake_claude::observations(&bytes);
    let prints: Vec<_> =
        observations.iter().filter(|seen| seen.arguments.first().is_some_and(|arg| arg == "-p")).collect();
    assert_eq!(prints.len(), RUNS, "{observations:?}");
    let argument = |index: usize, flag: &str| {
        prints[index].arguments.windows(2).find(|pair| pair[0] == flag).expect("program flag")[1].as_str()
    };
    assert_eq!(argument(3, "--resume"), argument(0, "--session-id"));
    for index in [0, 1, 2] {
        assert!(!prints[index].arguments.iter().any(|arg| arg == "--resume"));
    }
    assert_eq!(
        smith_fake_claude::selections(&bytes),
        [
            Seen::Selected { script: Selection::Cued(0), turn: 0 },
            Seen::Selected { script: Selection::Cued(1), turn: 0 },
            Seen::Selected { script: Selection::Cued(2), turn: 0 },
            Seen::Selected { script: Selection::Cued(0), turn: 1 },
        ]
    );
    let acts: Vec<_> =
        smith_fake_claude::seen(&bytes).into_iter().filter(|seen| matches!(seen, Seen::Acted { .. })).collect();
    assert_eq!(
        acts,
        [
            Seen::Acted { tool: "Read".into(), path: "README.md".into(), error: false },
            Seen::Acted { tool: "Edit".into(), path: "README.md".into(), error: false },
        ]
    );
}
