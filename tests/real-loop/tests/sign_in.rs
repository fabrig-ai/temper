//! The person's sign-in ends by the real listener or by the pasted address.
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{
    Exit, argv,
    calls::{D1, Step, call, sign_in},
    run_sign_in, terminal,
};
#[global_allocator]
static HEAP: Counting = Counting;

/// The ctl calls each story reads: `me` after the sign-in, the accounts at its end.
const ME: usize = 1;
const ACCOUNTS: usize = 2;
/// The person ends with one account, of one signed-in source.
const ONE_ACCOUNT: usize = 1;
const ONE_SOURCE: usize = 1;

#[test]
fn a_terminal_sign_in_returns_to_the_real_listener() {
    let ran = run_sign_in(|| {
        vec![
            sign_in(D1),
            call(D1, argv!["me", "--json"]),
            terminal::open(),
            terminal::type_on(b"Accounts", b""),
            terminal::see(b"Connect a model"),
            terminal::type_on(b"Connect a model", b""),
            terminal::see(b"Continue with ChatGPT"),
            terminal::type_on(b"Continue with ChatGPT", b""),
            terminal::see(b"Open the sign-in page <http://"),
            Step::Authorize,
            Step::Return,
            terminal::see(b"Signed in to ChatGPT"),
            call(D1, argv!["account", "list", "--json"]),
            terminal::close(),
        ]
    });
    assert!(ran.calls().iter().all(|call| call.exit == Some(Exit::Code(0))), "ctl calls succeeded");
    assert_eq!(ran.terminal().exit, Some(Exit::Code(0)), "terminal drained");
    let [answer] = ran.returned() else { panic!("one native listener answer") };
    assert!(answer.error.is_none(), "listener answered over a real connection");
    assert_eq!(answer.status, Some(200), "listener success page");
    assert!(answer.body.as_ref() == b"Sign-in received. You may close this page.\n", "fixed success page");
    assert_eq!(ran.code_exchanges(), 1, "exactly one code exchange");
    let me: serde_json::Value = serde_json::from_slice(&ran.calls()[ME].stdout).expect("me JSON");
    let person = me["party"]["number"].as_u64().expect("person number");
    let accounts: serde_json::Value = serde_json::from_slice(&ran.calls()[ACCOUNTS].stdout).expect("accounts JSON");
    let rows = accounts["accounts"]["party"].as_array().expect("person's accounts");
    assert_eq!(rows.len(), ONE_ACCOUNT, "one account for the person");
    let account = &rows[0];
    assert_eq!(account["provider"], 1);
    assert_eq!(account["plan"], true);
    assert_eq!(account["holder"]["party"], person);
    let sources = account["sources"].as_array().expect("account's sources");
    assert_eq!(sources.len(), ONE_SOURCE, "one signed-in source");
    assert_eq!(sources[0]["kind"], "sign_in");
    assert_eq!(sources[0]["usable"], true);
    assert_eq!(sources[0]["state"]["kind"], "granted");
}

#[test]
fn a_terminal_sign_in_finishes_by_the_pasted_address() {
    let ran = run_sign_in(|| {
        vec![
            sign_in(D1),
            call(D1, argv!["me", "--json"]),
            terminal::open(),
            terminal::type_on(b"Accounts", b""),
            terminal::see(b"Connect a model"),
            terminal::type_on(b"Connect a model", b""),
            terminal::see(b"Continue with ChatGPT"),
            terminal::type_on(b"Continue with ChatGPT", b""),
            terminal::see(b"Open the sign-in page <http://"),
            Step::Authorize,
            // The text shell prints disclosures unfolded (skein's ui-text.md, 5).
            terminal::see(b"Address of the page that couldn't be reached"),
            Step::Terminal(terminal::Act::TypeKept { label: b"Address of the page that couldn't be reached" }),
            terminal::type_on(b"Finish connecting", b""),
            terminal::see(b"Signed in to ChatGPT"),
            // The browser returns only after the paste has ended this sign-in.
            Step::Return,
            call(D1, argv!["account", "list", "--json"]),
            terminal::close(),
        ]
    });
    assert!(ran.calls().iter().all(|call| call.exit == Some(Exit::Code(0))), "ctl calls succeeded");
    assert_eq!(ran.terminal().exit, Some(Exit::Code(0)), "terminal drained");
    let [answer] = ran.returned() else { panic!("one late native listener answer") };
    // The sign-in's terminal closes its listener (skein's oauth.md, 6.5).
    assert_eq!(answer.error, Some(skein_io::kernel::Error::Refused), "ended listener refuses the late connection");
    assert_eq!(answer.status, None, "closed listener sends no HTTP status");
    assert!(answer.body.is_empty(), "closed listener sends no page");
    assert_eq!(ran.code_exchanges(), 1, "exactly one code exchange after the late return");
    let me: serde_json::Value = serde_json::from_slice(&ran.calls()[ME].stdout).expect("me JSON");
    let person = me["party"]["number"].as_u64().expect("person number");
    let accounts: serde_json::Value = serde_json::from_slice(&ran.calls()[ACCOUNTS].stdout).expect("accounts JSON");
    let rows = accounts["accounts"]["party"].as_array().expect("person's accounts");
    assert_eq!(rows.len(), ONE_ACCOUNT, "one account for the person after the late return");
    let account = &rows[0];
    assert_eq!(account["provider"], 1);
    assert_eq!(account["plan"], true);
    assert_eq!(account["holder"]["party"], person);
    let sources = account["sources"].as_array().expect("account's sources");
    assert_eq!(sources.len(), ONE_SOURCE, "one signed-in source");
    assert_eq!(sources[0]["kind"], "sign_in");
    assert_eq!(sources[0]["usable"], true);
    assert_eq!(sources[0]["state"]["kind"], "granted");
}
