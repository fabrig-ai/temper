//! GitHub story 6: the worker's clone and push authenticate on the layer's git route.
use serde_json::Value;
use temper_real_loop_world::{
    Exit, Ran, argv,
    calls::{D1, call, file, number, sign_in, task},
    github, landing, run_github,
};

// The fresh deployment allocates chat 1, procedure 2, and its coder 3.
const CODER: u64 = 3;
const UMBRA: usize = 3;
const CHAT: usize = 5;
const PROCEDURE: u64 = 2;
const DELIVERED: usize = 8;

#[test]
fn a_chats_clone_and_push_carry_its_installations_token() {
    let ran = run_github(github::fixture(), scripts, || {
        vec![
            sign_in(D1),
            call(D1, argv!["add-key", "1", file("fixture.key"), "--deployment", "--key", "github-llm"]),
            call(D1, argv!["wait", "accounts", "--until", "usable"]),
            call(
                D1,
                argv![
                    "adopt",
                    "--project",
                    "1",
                    "--forge",
                    "1",
                    "--owner",
                    "umbra",
                    "--name",
                    "tool",
                    "--landing",
                    "main",
                    "--key",
                    "adopt-umbra",
                    "--json"
                ],
            ),
            call(
                D1,
                argv![
                    "adopt",
                    "--project",
                    "1",
                    "--forge",
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
            call(D1, argv!["chat", "--project", "1", "--key", "github-change", "@forge_chat Update README"]),
            call(D1, argv!["wait", "task", task(CHAT), "--until", "replied"]),
            // An idle procedure has durably made its coder before we watch that task.
            call(D1, argv!["wait", "task", number(PROCEDURE), "--until", "phase:idle"]),
            call(D1, argv!["wait", "task", number(CODER), "--until", "ended", "--json"]),
        ]
    });
    for (index, result) in ran.calls().iter().enumerate() {
        assert_eq!(result.exit, Some(Exit::Code(if index == UMBRA { 2 } else { 0 })), "call {index}: {result:?}");
    }
    let refused: Value = serde_json::from_slice(&ran.calls()[UMBRA].stderr).expect("refused adoption JSON");
    assert_eq!(refused["forge_refused"]["reason"], "no_access", "{refused}");
    let seen = ran.github();
    assert_eq!(seen.git_after_call[UMBRA], 0, "no git call before the uninstalled adoption completed");
    assert_eq!(seen.installations, [(github::INSTALLATION, 10, b"acme".to_vec())], "only acme has an installation");
    assert!(seen.minted >= 1, "the secrets scan accompanies real token minting");
    assert!(seen.git_calls.iter().any(|call| !call.writes && call.status == 200), "successful clone/read");
    assert_eq!(seen.git_calls.iter().filter(|call| call.writes).count(), 1, "exactly one push writes");
    for call in &seen.git_calls {
        let minted = call.minted.expect("every git call authenticated by a minted token");
        assert_eq!(minted.installation, github::INSTALLATION);
        assert!(minted.mint > 0 && minted.mint <= seen.minted);
        // git reads the new branch before pushing it: its expected absence is 404.
        assert!(call.status == 200 || (!call.writes && call.status == 404), "{call:?}");
    }
    let delivered: Value = serde_json::from_slice(&ran.calls()[DELIVERED].stdout).expect("coder's ended task");
    let change = &delivered["task"]["chip"]["details"]["owner"]["details"]["ending"]["done"]["change"];
    assert!(change.is_object(), "coder delivered a change: {delivered}");
    let branches: Vec<_> = seen.branches.iter().filter(|(name, _)| name.as_slice() != b"main").collect();
    assert_eq!(branches.len(), 1, "one pushed branch");
    let (_, branch) = branches[0];
    assert_eq!(branch.tree.get(b"README.md".as_slice()).expect("README"), &[landing::README, landing::EDIT].concat());
    assert_eq!(delivered["task"]["chip"]["task"], CODER);
    assert_eq!(change["kind"], 2, "the coder's produced change");
    assert_eq!(change["part"]["key"]["number"], github::REPOSITORY);
    let local = no_installation_token_pattern_or_app_key_outside_the_secrets_store(&ran);
    assert_eq!(local, [branch.head], "the pushed head is the coder's one newly created local commit");
}

fn scripts(at: std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]> {
    let mut scripts = landing::scripts(at, u64::from(github::REPOSITORY), landing::README).into_vec();
    scripts.truncate(2); // Chat delegates and waits; coder reads, edits and finishes. No review.
    scripts[0].turns = std::mem::take(&mut scripts[0].turns).into_vec().into_iter().take(2).collect();
    scripts.into_boxed_slice()
}

// Also read the fake checkout's local object names: the coder is the only
// repository-bearing agent, and creates one object beyond its checked-out base.
fn no_installation_token_pattern_or_app_key_outside_the_secrets_store(ran: &Ran) -> Vec<u64> {
    use skein_lib::{
        Writer,
        base64::{self, Alphabet},
    };
    let key = skein_fake_forge_github::keys::KEYS[temper_github_app::APP.key];
    let size = base64::encoded_len(Alphabet::Standard, u32::try_from(key.len()).expect("test key length"))
        .expect("base64 length");
    let mut encoded = Writer::new(usize::try_from(size).expect("base64 size"));
    base64::encode(Alphabet::Standard, key, &mut encoded).expect("base64 key");
    let encoded = encoded.finish();
    let clean = |bytes: &[u8]| {
        assert!(
            !bytes.windows(40).any(|part| part.starts_with(b"ghs_") && part[4..].iter().all(u8::is_ascii_alphanumeric)),
            "minted-token pattern outside jig's secrets store"
        );
        assert!(skein_lib::bytes::find(bytes, key).is_none(), "App DER outside jig's secrets store");
        assert!(skein_lib::bytes::find(bytes, &encoded).is_none(), "App base64 outside jig's secrets store");
    };
    let secrets = ran.path().join("state/secrets");
    assert!(secrets.is_dir(), "the excluded directory is jig's secrets store");
    let mut created = Vec::new();
    let mut pending = vec![ran.path().to_path_buf()];
    while let Some(path) = pending.pop() {
        if path == secrets {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path).expect("scratch metadata");
        if metadata.is_dir() {
            pending.extend(
                std::fs::read_dir(path).expect("scratch directory").map(|entry| entry.expect("scratch entry").path()),
            );
        } else if metadata.is_file() {
            let bytes = std::fs::read(&path).expect("scratch file");
            clean(&bytes);
            if path.ends_with(".git/temper-head") {
                let base = u64::from_le_bytes(bytes.try_into().expect("fake checkout head"));
                let objects = path.parent().expect("git directory").join("objects");
                for object in std::fs::read_dir(objects).expect("local objects") {
                    let name = object.expect("local object").file_name();
                    let commit: u64 = name.to_str().expect("object name").parse().expect("counted object");
                    if commit != base {
                        created.push(commit);
                    }
                }
            }
        }
    }
    for call in ran.calls() {
        clean(&call.stdout);
        clean(&call.stderr);
    }
    created
}
