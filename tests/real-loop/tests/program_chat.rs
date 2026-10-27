//! An Open chat parks and resumes through smith's fake Claude Code (protocol/llm.md, 6).
use skein_world::domain::heap::Counting;
use smith_fake_claude::{Script, Seen, Selection, Step, Story, Turn};
use temper_real_loop_world::{
    Exit, Scratch, argv,
    calls::{D1, call, number, occurrences, sign_in, task, until},
    claude, fixtures, llm, run_on,
};
#[global_allocator]
static HEAP: Counting = Counting;

const SOURCE: &str = "live";
// The real loop retains jig's standalone worker fixture name.
const WORKER: &str = "worker-a";
// The harness provisions the direct key's account 1 before the story declares its program.
const PROGRAM_ACCOUNT: u64 = 2;
// Sign-in and declaration precede the first accounts read; verify and wait precede the last.
const DECLARED: usize = 2;
const USABLE: usize = 6;
// The chat follows the accounts reads; each reply is joined at idle before the next act.
const CHAT: usize = 7;
const FIRST_REPLY: usize = 9;
const SECOND_PARKED: usize = 14;
const CONVERSATION: usize = 15;
const FIRST: &str = "First.";
const SECOND: &str = "Second.";
const ONE: &str = "One.";
const TWO: &str = "Two.";
// One program process and one user message for each of the person's two requests.
const REPLIES: usize = 2;

#[test]
fn a_chat_on_a_program_source_replies_parks_and_resumes() {
    let scratch = Scratch::new("program-chat");
    let story = Story {
        cued: vec![Script {
            cue: FIRST.into(),
            turns: [ONE, TWO]
                .into_iter()
                .map(|reply| Turn { steps: vec![Step::Text(reply.into())], ..Turn::default() })
                .collect(),
        }],
        ..Story::default()
    };
    claude::source(&scratch, SOURCE, &story).expect("signed-in fake source");
    let ran = run_on(
        scratch,
        || llm::scripted(Box::new([])),
        || {
            vec![
                sign_in(D1),
                call(D1, argv!["declare", "2", "claude_code", SOURCE, "--key", "declare-program"]),
                call(D1, argv!["accounts", "--json"]),
                call(D1, argv!["verify", number(PROGRAM_ACCOUNT), "--source", "program", "--json"]),
                call(D1, argv!["wait", "accounts", "--until", "settled", "--json"]),
                call(D1, argv!["wait", "accounts", "--until", "usable", "--json"]),
                call(D1, argv!["accounts", "--json"]),
                call(D1, argv!["chat", "--project", fixtures::PROJECT, "--key", "program-chat", FIRST]),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "replied"]),
                until(D1, argv!["chat", task(CHAT)], ONE.as_bytes(), b"One"),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "phase:idle"]),
                call(D1, argv!["message", task(CHAT), SECOND, "--key", "program-second"]),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "replied"]),
                until(D1, argv!["chat", task(CHAT)], TWO.as_bytes(), b"Two"),
                call(D1, argv!["wait", "task", task(CHAT), "--until", "phase:idle", "--json"]),
                call(D1, argv!["chat", task(CHAT)]),
            ]
        },
    );
    for called in ran.calls() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&called.stderr));
    }
    let declared: serde_json::Value = serde_json::from_slice(&ran.calls()[DECLARED].stdout).expect("accounts JSON");
    let account = declared["accounts"]["party"]
        .as_array()
        .expect("party accounts")
        .iter()
        .find(|account| account["number"] == PROGRAM_ACCOUNT)
        .expect("declared program account");
    assert_eq!(account["provider"], 2);
    assert!(
        account["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .any(|source| source["name"] == SOURCE && source["kind"] == "program")
    );

    let usable: serde_json::Value = serde_json::from_slice(&ran.calls()[USABLE].stdout).expect("accounts JSON");
    let worker = usable["accounts"]["program_hosts"]
        .as_array()
        .expect("program hosts")
        .iter()
        .find(|host| host["name"] == WORKER)
        .unwrap_or_else(|| panic!("worker program facts: {usable}"));
    assert_eq!(worker["connected"], true);
    let program = worker["programs"]
        .as_array()
        .expect("host programs")
        .iter()
        .find(|program| program["name"] == SOURCE)
        .expect("source checked on worker");
    assert_eq!(program["state"], "usable");

    // The two-line activity pages keep the replies in separate reads. Smith puts
    // the opening request in the brief, rather than a person transcript message.
    let mut conversation = ran.calls()[FIRST_REPLY].stdout.clone();
    conversation.extend_from_slice(&ran.calls()[CONVERSATION].stdout);
    let text = std::str::from_utf8(&conversation).expect("chat text");
    let positions =
        [ONE, SECOND, TWO].map(|words| text.find(words).unwrap_or_else(|| panic!("chat lacks {words}: {text}")));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    for reply in [ONE, TWO] {
        assert_eq!(occurrences(&conversation, reply.as_bytes()), 1, "{text}");
    }
    let parked: serde_json::Value =
        serde_json::from_slice(&ran.calls()[SECOND_PARKED].stdout).expect("parked task JSON");
    assert_eq!(parked["task"]["kept_account"], PROGRAM_ACCOUNT, "{parked}");
    assert_eq!(parked["task"]["spec"]["specification"]["words"]["bytes"], FIRST, "{parked}");
    assert_eq!(parked["task"]["llm"]["main"]["model"], "fake-claude", "{parked}");
    assert!(ran.llm().is_empty(), "no direct-key request: {:?}", ran.llm());

    let bytes = std::fs::read(ran.path().join("programs").join(SOURCE).join(smith_fake_claude::OBSERVATIONS_FILE))
        .expect("fake observations");
    check_program(&bytes);
}

fn check_program(bytes: &[u8]) {
    let observations = smith_fake_claude::observations(bytes);
    assert!(!observations.is_empty(), "worker invoked the fake");
    let prints: Vec<_> = observations
        .iter()
        .filter(|observation| observation.arguments.first().is_some_and(|argument| argument == "-p"))
        .collect();
    assert_eq!(prints.len(), REPLIES, "{observations:?}");
    let argument = |index: usize, flag: &str| {
        prints[index].arguments.windows(2).find(|pair| pair[0] == flag).expect("program flag")[1].as_str()
    };
    let session = argument(0, "--session-id");
    assert!(!session.is_empty());
    assert_eq!(argument(1, "--resume"), session);
    assert!(!prints[0].arguments.iter().any(|argument| argument == "--resume"));
    assert!(!prints[1].arguments.iter().any(|argument| argument == "--session-id"));
    for observation in &observations {
        if observation.arguments.first().is_some_and(|argument| argument == "-p") {
            assert!(!observation.arguments.iter().any(|argument| argument == "--json-schema"));
        } else {
            assert_eq!(observation.arguments, ["auth", "status"]);
        }
        assert!(observation.environment.iter().any(|name| name == "CLAUDE_CONFIG_DIR"));
        for forbidden in ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"] {
            assert!(!observation.environment.iter().any(|name| name == forbidden), "filtered {forbidden}");
        }
    }
    let received = smith_fake_claude::received(bytes);
    assert_eq!(received.len(), REPLIES, "{received:?}");
    let mut texts = Vec::new();
    for message in &received {
        assert_eq!(message["type"], "user");
        assert_eq!(message["message"]["role"], "user");
        let content = &message["message"]["content"];
        let text = match content {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(parts) => {
                parts.iter().filter_map(|part| part["text"].as_str()).collect::<Vec<_>>().join("\n")
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Object(_) => panic!("user content: {content}"),
        };
        texts.push(text);
    }
    // Smith opens from its brief with BEGIN; the resumed input carries the person's next words.
    assert!(!texts[0].contains(FIRST) && !texts[0].contains(SECOND), "{}", texts[0]);
    assert!(texts[1].contains(SECOND), "{}", texts[1]);
    assert_eq!(
        smith_fake_claude::selections(bytes),
        [
            Seen::Selected { script: Selection::Cued(0), turn: 0 },
            Seen::Selected { script: Selection::Cued(0), turn: 1 }
        ]
    );
}
