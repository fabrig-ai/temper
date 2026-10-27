//! Landing spans chat, coding and review, then the chat's reply after the merge.
//! Landing and added-dependency stories host four or five agent lifetimes with git
//! calls, exceeding the 1 s guide; their behavior is covered in the root worlds.
//! The shared-forge harness does not meter each host, so use the system allocator.
use serde_json::Value;
use temper_real_loop_world::landing;

#[test]
fn a_chat_lands_its_change_on_the_forge() {
    use temper_real_loop_world::{
        Exit, argv,
        calls::{D1, call, delegate, digest, sign_in, task},
        forge, landing, run_forge,
    };
    // Sign-in and adoption precede the chat; its replied wait precedes the tree read.
    const CHAT: usize = 2;
    const TREE: usize = 4;
    // The reply follows landing; the final watch also joins the chat's close.
    const REPLIED: usize = 6;
    const CHAT_ENDED: usize = 7;
    let ran = run_forge(
        landing::fixture(),
        |at| landing::scripts(at, u64::from(forge::ID), landing::README),
        || {
            vec![
                sign_in(D1),
                call(
                    D1,
                    argv![
                        "repository",
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
                call(D1, argv!["chat", "start", "--project", "1", "--key", "land", "@forge_chat Land a small change"]),
                call(D1, argv!["task", "show", task(CHAT), "--until", "replied"]),
                call(D1, argv!["task", "show", task(CHAT), "--tree", "--json"]),
                call(D1, argv!["task", "show", delegate(TREE), "--until", "ended"]),
                call(D1, argv!["task", "show", task(CHAT), "--since", digest(TREE), "--until", "replied"]),
                // A replied watch accepts closing before the run has ended.
                // Finish the requesting chat before service teardown.
                call(D1, argv!["task", "show", task(CHAT), "--until", "ended", "--json"]),
            ]
        },
    );
    assert_eq!(ran.calls()[REPLIED].exit, Some(Exit::Code(0)), "{:?}", ran.calls()[REPLIED]);
    assert_eq!(ran.calls()[CHAT_ENDED].exit, Some(Exit::Code(0)), "{:?}", ran.calls()[CHAT_ENDED]);
    let chat: Value = serde_json::from_slice(&ran.calls()[CHAT_ENDED].stdout).expect("chat ended");
    assert_eq!(chat["task"]["result"]["value"]["report"]["bytes"], "The change landed.", "{chat}");
    assert!(ran.forge().merged_green() && ran.forge().main_file(b"README.md").ends_with(landing::EDIT));
}

#[test]
fn a_chat_keeps_its_live_delegate_until_the_change_lands() {
    use landing::kept::{A3, E1};
    use temper_real_loop_world::{
        Exit, argv,
        calls::{D1, call, number, sign_in, task},
        forge, run_forge,
    };
    const ONE_RUN: usize = 1;
    const CHAT: usize = 2;
    const E1_DOCUMENT: usize = 4;
    const A3_DOCUMENT: usize = 5;
    const CHAT_ENDED: usize = 6;
    let ran = run_forge(
        landing::kept::fixture(),
        |at| landing::kept::scripts(at, u64::from(forge::ID)),
        || {
            vec![
                sign_in(D1),
                call(
                    D1,
                    argv![
                        "repository",
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
                call(D1, argv!["chat", "start", "--project", "1", "--key", "kept", "@kept_chat Add a landing wait"]),
                call(D1, argv!["task", "show", task(CHAT), "--until", "replied"]),
                call(D1, argv!["task", "show", number(E1), "--until", "ended", "--json"]),
                call(D1, argv!["task", "show", number(A3), "--until", "ended", "--json"]),
                call(D1, argv!["task", "show", task(CHAT), "--until", "ended", "--json"]),
            ]
        },
    );
    for document in [E1_DOCUMENT, A3_DOCUMENT, CHAT_ENDED] {
        assert_eq!(ran.calls()[document].exit, Some(Exit::Code(0)), "{:?}", ran.calls()[document]);
    }
    assert!(ran.forge().merged_green() && ran.forge().main_file(b"README.md").ends_with(landing::EDIT));
    let e1: Value =
        serde_json::from_slice::<Value>(&ran.calls()[E1_DOCUMENT].stdout).expect("e1 document")["task"].take();
    let a3: Value =
        serde_json::from_slice::<Value>(&ran.calls()[A3_DOCUMENT].stdout).expect("a3 document")["task"].take();
    assert_eq!(e1["chip"]["task"], E1, "{e1}");
    assert_eq!(a3["chip"]["task"], A3, "{a3}");
    let ended = |value: &Value| value["chip"]["details"]["owner"]["details"]["ended_at"].as_u64().expect("ended wall");
    assert!(ended(&a3) >= ended(&e1), "a3 {a3}; e1 {e1}");
    let started = a3["chip"]["details"]["owner"]["details"]["run_since"].as_u64().expect("a3 run start");
    assert!(started >= ended(&e1), "a3 started after e1 ended");
    assert_eq!(a3["result"]["value"]["report"]["bytes"], landing::kept::AFTER_MET, "{a3}");
    let chat: Value = serde_json::from_slice(&ran.calls()[CHAT_ENDED].stdout).expect("chat ended");
    assert_eq!(chat["task"]["result"]["value"]["report"]["bytes"], "Both delegates ended.");
    let mut a3_runs = 0;
    let mut revised = false;
    // a3's script has one finish call per run, hence one query per run.
    for seen in ran.llm() {
        let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { continue };
        if query.system.windows(b"@kept_a3".len()).any(|word| word == b"@kept_a3") {
            a3_runs += 1;
        }
        for part in query.messages.iter().flat_map(|message| message.parts.iter()) {
            if let skein_fake_llm_domain::api::Part::ToolOutput { output, is_error: false, .. } = part {
                revised |= output
                    .windows(b"Done: task 3 is at spec revision 1".len())
                    .any(|word| word == b"Done: task 3 is at spec revision 1");
            }
        }
    }
    assert_eq!(a3_runs, ONE_RUN);
    assert!(revised, "the chat heard Revised");
}
