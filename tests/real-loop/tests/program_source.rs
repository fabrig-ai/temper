//! A declared program source is checked on the worker through smith's fake Claude Code.
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{
    Exit, Scratch, argv,
    calls::{D1, call, number, sign_in},
    claude, llm, run_on,
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

#[test]
fn a_program_source_is_declared_verified_and_usable() {
    let scratch = Scratch::new("program-source");
    claude::source(&scratch, SOURCE, &smith_fake_claude::Story::default()).expect("signed-in fake source");
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

    let bytes = std::fs::read(ran.path().join("programs").join(SOURCE).join(smith_fake_claude::OBSERVATIONS_FILE))
        .expect("fake observations");
    let observations = smith_fake_claude::observations(&bytes);
    assert!(!observations.is_empty(), "worker invoked the fake");
    for observation in observations {
        assert_eq!(observation.arguments, ["auth", "status"]);
        assert!(observation.environment.iter().any(|name| name == "CLAUDE_CONFIG_DIR"));
        for forbidden in ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"] {
            assert!(!observation.environment.iter().any(|name| name == forbidden), "filtered {forbidden}");
        }
    }
}
