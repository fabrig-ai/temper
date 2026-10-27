//! A new standalone preparation reads the work the drained service kept.
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{
    Exit, argv,
    calls::{D1, call, number, occurrences, sign_in, task, until},
    fixtures, llm, run_on, scripts,
};
#[global_allocator]
static HEAP: Counting = Counting;
// One chat is created after sign-in; its last read follows the restarted sign-in.
const CHAT: usize = 2;
const READ: usize = 1;
const SETTINGS: usize = 2;
const CHARTER: usize = 3;
// Only that chat's first run consumes a scripted turn; the restart consumes none.
const CHATS: usize = 1;
#[test]
fn a_restarted_service_keeps_the_chat_with_its_reply() {
    let scratch = temper_real_loop_world::Scratch::new("settings-restart");
    let path = scratch.path().to_path_buf();
    let first = run_on(
        scratch,
        || llm::scripted(scripts::chat_scripts(CHATS)),
        || {
            let mut steps = fixtures::signed_chat("kept");
            let chat = steps.pop().expect("chat opener");
            let mut items = fixtures::setup_items(&path).expect("settings setup");
            items[0]["set"]["waiting"] = 2_000_000.into();
            steps.extend(fixtures::change_settings(D1, "restart-settings", items));
            steps.push(chat);
            steps.push(until(D1, argv!["chat", "show", task(CHAT)], scripts::REPLY, scripts::EARLY));
            steps
        },
    );
    let chat = first.number(CHAT);
    let second = run_on(
        first.into_scratch(),
        || llm::scripted(scripts::chat_scripts(CHATS)),
        move || {
            vec![
                sign_in(D1),
                call(D1, argv!["chat", "show", number(chat)]),
                call(D1, argv!["settings", "show", "--json"]),
                call(D1, argv!["settings", "show", "charter", "1", "--json"]),
            ]
        },
    );
    let read = &second.calls()[READ];
    assert_eq!(
        (read.exit, occurrences(&read.stdout, scripts::REPLY)),
        (Some(Exit::Code(0)), 1),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let asked = second.llm().iter().any(|seen| matches!(seen, skein_fake_peers::llm::Observation::Query { .. }));
    assert!(!asked, "the restarted service asked the model again");
    let inventory: serde_json::Value =
        serde_json::from_slice(&second.calls()[SETTINGS].stdout).expect("settings inventory");
    assert_eq!(inventory["newest"], 2, "the ctl change survives restart");
    let charter: serde_json::Value = serde_json::from_slice(&second.calls()[CHARTER].stdout).expect("charter setting");
    assert_eq!(charter["value"]["waiting"], 2_000_000, "the saved value overrides the world's 1 ms default");
}
