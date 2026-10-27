//! One private file reaches the credential used by the claimed chat.
use skein_world::domain::heap::Counting;
#[global_allocator]
static HEAP: Counting = Counting;
use temper_real_loop_world::{
    Exit, Scratch, argv,
    calls::{D1, call, file, number, sign_in, task},
    llm, run_key_on, scripts,
};
// The cold providers-only store allocates source 0, then account 1.
const ADDED_ACCOUNT: u64 = 1;
const CHAT: usize = 4; // Sign-in, add, rank and usable wait precede the chat.
const KEY: &[u8] = b"private-file-smoke-llm-key";
#[test]
fn a_private_file_key_reaches_the_chats_llm() {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let scratch = Scratch::new("account-key");
    let mut key = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(scratch.path().join("llm.key"))
        .expect("private file");
    key.write_all(KEY).expect("key bytes");
    key.write_all(b"\n").expect("final newline");
    let ran = run_key_on(
        scratch,
        || {
            let mut recipe = llm::scripted(scripts::chat_scripts(1));
            recipe.config.path = b"/v1/responses".as_slice().into();
            recipe.credential =
                skein_llm::Credential { value: KEY.into(), scheme: skein_llm::Scheme::Key, account_id: Box::default() };
            recipe
        },
        || {
            vec![
                sign_in(D1),
                call(D1, argv!["account", "add-key", "1", file("llm.key"), "--deployment", "--key", "smoke-llm"]),
                call(D1, argv!["account", "rank", number(ADDED_ACCOUNT), "1"]),
                call(D1, argv!["account", "list", "--until", "usable"]),
                call(D1, argv!["chat", "start", "--project", "1", scripts::WORDS, "--key", "key-chat"]),
                call(D1, argv!["task", "show", task(CHAT), "--until", "replied"]),
            ]
        },
    );
    for called in ran.calls() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&called.stderr));
    }
    assert!(
        ran.llm().iter().any(|observation| matches!(
            observation,
            skein_fake_peers::llm::Observation::Credential { scheme: Some(skein_llm::Scheme::Key), index: Some(0), .. }
        )),
        "the peer accepts only the private file's exact key; observations: {:?}; task: {}",
        ran.llm(),
        String::from_utf8_lossy(&ran.calls().last().expect("wait").stdout)
    );
}
