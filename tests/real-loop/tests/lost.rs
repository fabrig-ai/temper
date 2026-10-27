//! A request held durably survives the loss of its caller's answer.
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{
    Exit, argv,
    calls::{D2, call, listed, same_party, task, until},
    fixtures, run, scripts,
};
#[global_allocator]
static HEAP: Counting = Counting;
// D2's same-party sign-in is call two; its named retry is call three.
const AGAIN: usize = 3;
// Each story gives one scripted response to each of its distinct named chats.
const ONE_CHAT: usize = 1;

#[test]
fn a_lost_answer_reaches_its_name_from_another_directory() {
    let ran = run(
        || scripts::chat_scripts(ONE_CHAT),
        || {
            let mut steps = fixtures::lost_chat();
            steps.extend([
                same_party(D2),
                call(
                    D2,
                    argv![
                        "chat",
                        "start",
                        "--project",
                        fixtures::PROJECT,
                        "--key",
                        fixtures::LOST_NAME,
                        scripts::WORDS
                    ],
                ),
                until(D2, argv!["chat", "show", task(AGAIN)], scripts::REPLY, scripts::EARLY),
                call(D2, argv!["task", "list", "--project", fixtures::PROJECT, "--label", "chat"]),
            ]);
            steps
        },
    );
    let [_, lost, signed, again, _, tasks] = ran.calls() else { panic!("six calls") };
    assert_eq!(
        (lost.exit, lost.stdout.as_slice()),
        (Some(Exit::Signal(9)), b"".as_slice()),
        "killed before its answer"
    );
    assert_eq!(signed.tries, 2, "same party signed in through a fresh link");
    assert_eq!(again.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&again.stderr));
    assert_eq!(listed(&tasks.stdout), [ran.number(AGAIN)], "the party has one chat");
}
