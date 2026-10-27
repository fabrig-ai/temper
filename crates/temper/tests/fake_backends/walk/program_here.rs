//! The Model step declares a program, then the person copies the box's login command.
use super::{Act, Call, Walk, ctl, here_key};

// Init's link signs in as "Owner"; the Model step's default is claude-owner.
const NAME: &str = "claude-owner";

fn program_account(call: &super::Ended) -> u64 {
    let accounts = ctl::json(call);
    let account = accounts["accounts"]["party"]
        .as_array()
        .expect("accounts")
        .iter()
        .find(|account| {
            account["sources"]
                .as_array()
                .expect("sources")
                .iter()
                .any(|source| source["name"] == NAME && source["kind"] == "program")
        })
        .expect("declared program account");
    assert_eq!(account["provider"], 2, "Claude subscription belongs to Anthropic");
    account["number"].as_u64().expect("account number")
}

fn assert_program_usable(call: &super::Ended) {
    let accounts = ctl::json(call);
    let host = accounts["accounts"]["program_hosts"]
        .as_array()
        .expect("program hosts")
        .iter()
        .find(|host| host["name"] == "worker")
        .expect("worker program facts");
    assert_eq!(host["connected"], true);
    let program = host["programs"]
        .as_array()
        .expect("host programs")
        .iter()
        .find(|program| program["name"] == NAME)
        .expect("login checked on worker");
    assert_eq!(program["state"], "usable");
}

#[test]
fn a_program_and_forgejo_here_reach_ready_on_the_fake_box() {
    let walk = Walk::owned("walk-program-here");
    let seen = walk
        .run(vec![
            Act::Now(Call::new(&["account", "list", "--json"])),
            Act::Now(Call::new(&["forge", "list", "--json"])),
            Act::Mark("welcome preconditions"),
            Act::After(Box::new(|seen| {
                here_key::assert_offered(seen);
                Call::new(&["forge", "setup", "3", "--key", "walk-here", "--json"])
            })),
            Act::Mark("owned forge adopted"),
            Act::After(Box::new(|seen| {
                here_key::assert_adopted(seen.last().expect("setup adoption"));
                Call::new(&["account", "declare", "2", "claude_code", NAME, "--key", "walk-program"])
            })),
            Act::Now(Call::new(&["account", "list", "--json"])),
            Act::After(Box::new(|seen| {
                let [.., declared, accounts] = seen else { panic!("program declaration and account read") };
                assert!(String::from_utf8_lossy(&declared.stdout).contains("temper program-login claude-owner"));
                program_account(accounts);
                // A read after these assertions lets the printed-command step follow the same act sequence.
                Call::new(&["forge", "list", "--json"])
            })),
            Act::ProgramLogin(NAME),
            Act::After(Box::new(|seen| {
                assert_eq!(seen.last().expect("separate program-login process").stdout, b"Signed in.\n");
                let [.., accounts, _forges, _login] = seen else { panic!("declared account before login") };
                let account = program_account(accounts).to_string();
                // tools.md 5.5: verify asks the host to check a newly completed terminal login.
                Call::new(&["account", "verify", &account, "--source", "program", "--json"])
            })),
            Act::Now(Call::new(&["account", "list", "--until", "usable", "--json"])),
            Act::Now(Call::new(&["forge", "list", "--json"])),
            Act::Now(Call::new(&["account", "list", "--json"])),
            Act::Mark("program ready"),
        ])
        .answers;
    here_key::assert_ready(&seen);
    assert_program_usable(seen.last().expect("accounts after login"));
    // End at Ready: programs cannot take a first chat today (jig's
    // crates/jig-domain-core/src/run_choice.rs:202–223), and ctl has no goal tool
    // yet (tools.md, area 2; temper-client-tools/src/guide.rs:79).
    walk.scratch.remove();
}
