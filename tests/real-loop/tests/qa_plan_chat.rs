//! Chat turn ordering and committed replies (jig's domain/tasks.md, 5.5, 5.8; domain/testing.md, 2.1, 2.6; client/tools.md, 5.2); a QA pass run by `--profile qa`.
use skein_fake_llm_domain::api;
use temper_real_loop_world::{
    calls::{self, Argv, Dir},
    fixtures,
};

#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

const EXPECTED_REQUESTS: usize = 2;
const IN_FLIGHT_AND_ONE_CLOSING: usize = 2;
const PERSON: Dir = Dir(1);
const CHAT: usize = 1;
const LATEST_ACTIVITY_CALL: usize = 6;
const OLDER_CHAT_CALL: usize = 7;
const FINAL_CHAT_CALL: usize = 8;
const PREFIX: &[u8] = b"The first ";
const FIRST: &[u8] = b"The first QA reply is complete.";
const SECOND: &[u8] = b"The queued QA words were answered.";
const QUEUED: &[u8] = b"These are the queued QA words.";
const HOLD: &[u8] = b"qa-first-turn";

// This pass keeps the stock requester-ended chat in the shared settings setup.
fn chat_fixture() -> temper_real_loop_world::forge::Fixture {
    fn configure(document: &mut serde_json::Value, agent: &mut serde_json::Value, forge: std::net::SocketAddr) {
        (temper_real_loop_world::forge::fixture().configure)(document, agent, forge);
        fixtures::requester(document);
    }
    let mut fixture = temper_real_loop_world::forge::fixture();
    fixture.configure = configure;
    fixture
}

fn scripts() -> Box<[api::Script]> {
    Box::from([api::Script {
        cue: Box::default(),
        turns: Box::from([
            api::Turn {
                lines: Box::from([api::Line::Pieces {
                    pieces: Box::from([
                        api::Piece { text: Box::from(PREFIX), hold: None },
                        api::Piece { text: Box::from(&FIRST[PREFIX.len()..]), hold: Some(Box::from(HOLD)) },
                    ]),
                }]),
                finish: api::Finish::Stop,
                tokens: 20,
            },
            api::Turn {
                lines: Box::from([api::Line::Text { text: Box::from(SECOND) }]),
                finish: api::Finish::Stop,
                tokens: 20,
            },
        ]),
    }])
}

#[test]
fn words_during_a_turn_are_answered_once() {
    let ran = temper_real_loop_world::run_forge(chat_fixture(), |_| scripts(), queued_steps);
    calls_succeed(&ran);
    let conversation =
        [ran.calls()[OLDER_CHAT_CALL].stdout.as_slice(), ran.calls()[FINAL_CHAT_CALL].stdout.as_slice()].concat();
    assert_eq!(calls::occurrences(&conversation, FIRST), 1, "first reply committed once");
    assert_eq!(calls::occurrences(&conversation, SECOND), 1, "queued words answered once");
    let queries = queries(&ran);
    assert_eq!(queries.len(), EXPECTED_REQUESTS, "two messages need two calls, with no nudge or retry");
    let deliveries: usize = queries
        .last()
        .expect("second call")
        .messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .map(|part| if let api::Part::Text { text } = part { calls::occurrences(text, QUEUED) } else { 0 })
        .sum();
    assert_eq!(deliveries, 1, "queued words delivered once to the second call");
    let answers: Vec<_> = ran
        .llm()
        .iter()
        .filter_map(|item| {
            if let skein_fake_peers::llm::Observation::Answered { result, .. } = item { Some(result) } else { None }
        })
        .collect();
    assert_eq!(answers.len(), EXPECTED_REQUESTS, "each model call answered once");
    assert!(answers.iter().all(|answer| answer.is_ok()), "both replies complete");
    let task: serde_json::Value =
        serde_json::from_slice(&ran.calls().last().expect("task read").stdout).expect("task JSON");
    assert_eq!(task["chip"]["phase"], "idle", "Open chat parks");
    assert!(task.get("hold").expect("hold field").is_null(), "chat is not held");
    assert!(task.get("result").expect("result field").is_null(), "text alone supplies no result");
    assert!(
        task["chip"]["details"]["owner"]["details"].get("retry").expect("retry field").is_null(),
        "no retry is spent"
    );
}

fn closing_scripts() -> Box<[api::Script]> {
    let mut scripts = scripts();
    scripts[0].turns[1] = api::Turn {
        lines: Box::from([api::Line::Call {
            name: Box::from(&b"finish"[..]),
            arguments: Box::from(&b"{\"fields\":{},\"form\":\"report\",\"text\":\"The QA chat is closed.\"}"[..]),
        }]),
        finish: api::Finish::ToolCalls,
        tokens: 20,
    };
    scripts
}

#[test]
#[ignore = "backlog: ctl reads a chat through its watch, which an ended chat ends (jig client contract 6)"]
fn close_during_a_turn_keeps_the_cancelled_conversation_readable() {
    let ran = temper_real_loop_world::run_forge(chat_fixture(), |_| closing_scripts(), close_steps);
    let task: serde_json::Value =
        serde_json::from_slice(&ran.calls().last().expect("task read").stdout).expect("task JSON");
    let cancelled =
        task["chip"]["details"]["owner"]["details"]["ending"].get("cancelled").expect("requester cancellation");
    assert_eq!(cancelled["by"]["person"], 1);
    assert_eq!(cancelled["reason"]["bytes"], "Close this QA chat.");
    assert!(cancelled.get("result").expect("partial result").is_null());
    assert!(task["result"].get("value").expect("result value").is_null());
    assert!(queries(&ran).len() <= IN_FLIGHT_AND_ONE_CLOSING, "Close permits at most one further turn");
    calls_succeed(&ran);
}

fn queries(ran: &temper_real_loop_world::Ran) -> Vec<&api::Query> {
    ran.llm()
        .iter()
        .filter_map(
            |item| {
                if let skein_fake_peers::llm::Observation::Query { query, .. } = item { Some(query) } else { None }
            },
        )
        .collect()
}

fn calls_succeed(ran: &temper_real_loop_world::Ran) {
    for (position, call) in ran.calls().iter().enumerate() {
        assert_eq!(
            call.exit,
            Some(temper_real_loop_world::Exit::Code(0)),
            "call {position}: {}\n{}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
}

fn queued_steps() -> Vec<calls::Step> {
    let mut steps = fixtures::signed_chat("qa-plan-chat-queued");
    steps.extend([
        calls::until(PERSON, Argv(vec!["run".into(), calls::task(CHAT)]), PREFIX, b"not expected"),
        calls::call(
            PERSON,
            Argv(vec![
                "message".into(),
                calls::task(CHAT),
                "These are the queued QA words.".into(),
                "--key".into(),
                "qa-queued-words".into(),
            ]),
        ),
        calls::release_hold(HOLD),
        calls::until(PERSON, Argv(vec!["chat".into(), calls::task(CHAT)]), SECOND, b"not expected"),
        calls::call(
            PERSON,
            Argv(vec![
                "wait".into(),
                "task".into(),
                calls::task(CHAT),
                "--until".into(),
                "replied".into(),
                "--json".into(),
            ]),
        ),
        calls::call(PERSON, Argv(vec!["activity".into(), calls::task(CHAT), "--json".into()])),
        calls::call(
            PERSON,
            Argv(vec![
                "chat".into(),
                calls::task(CHAT),
                "--before".into(),
                calls::activity_before(LATEST_ACTIVITY_CALL),
            ]),
        ),
        calls::call(PERSON, Argv(vec!["chat".into(), calls::task(CHAT)])),
        calls::call(PERSON, Argv(vec!["task".into(), calls::task(CHAT), "--json".into()])),
    ]);
    steps
}

fn close_steps() -> Vec<calls::Step> {
    let mut steps = fixtures::signed_chat("qa-plan-chat-close-running");
    steps.extend([
        calls::until(PERSON, Argv(vec!["run".into(), calls::task(CHAT)]), PREFIX, b"not expected"),
        calls::call(
            PERSON,
            Argv(vec![
                "close".into(),
                calls::task(CHAT),
                "--reason".into(),
                "Close this QA chat.".into(),
                "--key".into(),
                "qa-close-running".into(),
            ]),
        ),
        calls::release_hold(HOLD),
        calls::call(
            PERSON,
            Argv(vec![
                "wait".into(),
                "task".into(),
                calls::task(CHAT),
                "--until".into(),
                "ended".into(),
                "--json".into(),
            ]),
        ),
        calls::call(PERSON, Argv(vec!["chat".into(), calls::task(CHAT)])),
        calls::call(PERSON, Argv(vec!["task".into(), calls::task(CHAT), "--json".into()])),
    ]);
    steps
}
