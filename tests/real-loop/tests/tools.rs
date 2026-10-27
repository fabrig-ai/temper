//! The tools' real pipes meet the standalone service and hosted agent.
use skein_world::domain::heap::Counting;
#[global_allocator]
static HEAP: Counting = Counting;
// Each story signs in first; a chat, when present, is the next call.
const SIGN_IN: usize = 0;
const CHAT: usize = 1;
// The replied wait follows the chat; the running ended wait precedes close.
const WAIT: usize = 3;
const CLOSE: usize = 4;
// Each story needs at most one chat's scripted model turn.
const CHATS: usize = 1;
#[test]
fn a_signed_in_chat_shows_its_reply_only_once_committed() {
    use temper_real_loop_world::{
        Exit, argv,
        calls::{D1, occurrences, task, until},
        fixtures, run, scripts,
    };
    let ran = run(
        || scripts::chat_scripts(CHATS),
        || {
            let mut steps = fixtures::signed_chat("first");
            steps.push(until(D1, argv!["chat", "show", task(CHAT)], scripts::REPLY, scripts::EARLY));
            steps
        },
    );
    let [signed, chat, read] = ran.calls() else { panic!("three calls") };
    assert_eq!(
        [signed.exit, chat.exit, read.exit],
        [Some(Exit::Code(0)); 3],
        "{}",
        String::from_utf8_lossy(&chat.stderr)
    );
    assert_eq!((read.early, occurrences(&read.stdout, scripts::REPLY)), (0, 1), "{} reads", read.tries);
}
#[test]
fn a_signed_in_directory_is_private_to_its_user() {
    use std::os::unix::fs::MetadataExt;
    use temper_real_loop_world::{
        Exit,
        calls::{D1, sign_in},
        fixtures, run, scripts,
    };
    let ran = run(|| scripts::chat_scripts(CHATS), || vec![sign_in(D1)]);
    assert_eq!(
        ran.calls()[SIGN_IN].exit,
        Some(Exit::Code(0)),
        "{}",
        String::from_utf8_lossy(&ran.calls()[SIGN_IN].stderr)
    );
    let directory = fixtures::directory(ran.path(), D1);
    for (path, mode) in
        [(directory.clone(), 0o700), (directory.join("token"), 0o600), (directory.join("address"), 0o600)]
    {
        let metadata = std::fs::metadata(&path).expect("signed-in metadata");
        assert_eq!(
            (metadata.mode() & 0o777, metadata.uid()),
            (mode, skein_shell::effective_user()),
            "{}",
            path.display()
        );
    }
}

#[test]
fn another_clients_close_meets_a_running_wait() {
    use temper_real_loop_world::{
        Exit, argv,
        calls::{D1, call, join, start, task, watching},
        fixtures, run, scripts,
    };
    let ran = run(
        || scripts::chat_scripts(CHATS),
        || {
            let mut steps = fixtures::signed_chat("waiting");
            steps.extend([
                call(D1, argv!["task", "show", task(CHAT), "--until", "replied"]),
                start(D1, argv!["task", "show", task(CHAT), "--until", "ended"]),
                watching(WAIT),
                call(D1, argv!["chat", "close", task(CHAT), "--key", "close-waiting"]),
                join(WAIT),
            ]);
            steps
        },
    );
    let (wait, close) = (&ran.calls()[WAIT], &ran.calls()[CLOSE]);
    assert_eq!([wait.exit, close.exit], [Some(Exit::Code(0)); 2], "{}", String::from_utf8_lossy(&wait.stderr));
    assert!(wait.ended > close.spawned, "the wait ended at {:?}, before the close", wait.ended);
}
