//! Shared spawn fixtures, also used by the hosted world's program tests.
use skein_io::kernel::{Fd, Pipe, Spawn, Way};
use skein_lib::Secret;
use temper_worker_domain_checkout::git;
use temper_worker_protocol_git::{
    Identity,
    invocation::{self, Command, Source},
};
/// Only the header may hold this token's bytes.
pub const TOKEN: &[u8] = b"fake-git-identity-token";
/// The scenario's git identity.
#[must_use]
pub fn identity() -> Identity {
    Identity {
        token: Secret::new(TOKEN.into()),
        login: b"worker".as_slice().into(),
        name: b"Worker".as_slice().into(),
        email: b"worker@example.invalid".as_slice().into(),
    }
}
/// The spawn the component makes for the command's operation.
#[must_use]
pub fn spawn_of(command: &Command) -> Spawn {
    let kind = match command {
        Command::Clone { .. } => Some(git::Kind::Clone),
        Command::Fetch { .. } => Some(git::Kind::Fetch),
        Command::Create { .. } => Some(git::Kind::Create),
        Command::Push { .. } | Command::IsAncestor { .. } => Some(git::Kind::Push),
        Command::Unmerged
        | Command::Add
        | Command::DiffIndex { .. }
        | Command::WriteTree
        | Command::CommitTree { .. } => Some(git::Kind::Commit),
        Command::CheckOut { .. } | Command::Clean => Some(git::Kind::CheckOut),
        Command::Merge { .. } => Some(git::Kind::Merge),
        Command::Version => None,
    };
    let mut pipes =
        vec![Pipe { child: 1, way: Way::Out, parent: None }, Pipe { child: 2, way: Way::Out, parent: None }];
    if matches!(command, Command::CommitTree { .. }) {
        pipes.insert(0, Pipe { child: 0, way: Way::In, parent: None });
    }
    Spawn {
        program: b"/usr/bin/git".as_slice().into(),
        args: invocation::arguments(command),
        env: invocation::environment(b"/worker/home", kind, Some(&identity())),
        root: Fd::new(1),
        dir: b"workspaces/1/app".as_slice().into(),
        pipes: pipes.into_boxed_slice(),
    }
}

use crate::{Answer, Typed, answer, check, id::id};
use skein_fake_forge_domain::{self as domain, api};
use skein_fake_peers::forge::Forge;
use skein_lib::{Duration, List};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use temper_fake_checkout::{
    Checkout,
    git::{self as local, Remote, Tree},
};
/// The protocol world's synchronous forge configuration, shared by the worlds.
pub const CONFIG: domain::Config = temper_worker_checkout_world::forge::CONFIG;
/// Each protocol or refusal case has its own equivalent checkout fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    MissingRepository,
    MissingBranch,
    MissingCommit,
    WrongToken,
    ProtectedBranch,
    Unreachable,
    NoPermission,
}
/// The forge and local files for one invocation or a hosted-world story.
#[derive(Debug)]
pub struct Scenario {
    pub transport: Typed,
    pub checkout: Checkout,
    pub base: u64,
    pub topic: u64,
    pub at: Vec<u8>,
    pub remote: Vec<u8>,
}
/// One repository, acme/app, with a base and a topic commit.
#[must_use]
pub fn scenario() -> Scenario {
    named(b"acme/app", None)
}
/// The API table the HTTP and git processes share.
#[must_use]
pub fn table(full: &[u8], protected: Option<&[u8]>) -> skein_fake_forge_protocol::Config {
    let mut tokens = List::with_capacity(2);
    tokens.push((Secret::new(TOKEN.into()), 1)).expect("two tokens");
    tokens.push((Secret::new(b"fake-git-stranger-token".as_slice().into()), 2)).expect("two tokens");
    let mut users = List::with_capacity(2);
    users.push((1, b"worker".as_slice().into())).expect("two users");
    users.push((2, b"stranger".as_slice().into())).expect("two users");
    let mut repositories = List::with_capacity(1);
    repositories
        .push(skein_fake_forge_protocol::Repository { id: 2, full: full.into(), protected: protected.map(Into::into) })
        .expect("one repository");
    skein_fake_forge_protocol::Config {
        base: b"http://127.0.0.1:3000".as_slice().into(),
        tokens,
        users,
        repositories,
        max_response_items: 8,
        default_paging_num: 30,
        hooks: None,
        faults: skein_fake_forge_protocol::Faults { reset: 0, drop: 0, server_error: 0 },
    }
}
/// The transcript's repository name, with an unborn fixture for fixture/empty.
#[must_use]
pub fn named(full: &[u8], protected: Option<&[u8]>) -> Scenario {
    let mut domain = domain::Domain::new(&CONFIG, 1);
    let setup = api::Setup {
        workflows: Box::new([]),
        id: 2,
        name: full.into(),
        default: b"main".as_slice().into(),
        tree: files(tree(b"base\n")),
        labels: Box::new([]),
        checks: api::Checks {
            contexts: Box::new([]),
            latency_min: Duration::ZERO,
            latency_max: Duration::ZERO,
            silent: 0,
            passes: 1000,
            reruns: 0,
            cue: None,
        },
        protection: protected.map(|branch| api::Protection {
            branch: branch.into(),
            contexts: Box::new([]),
            approvals: 0,
            dismiss_stale: false,
            queue: false,
            linear: false,
            code_owners: false,
            bypassing_users: Box::new([]),
        }),
        hooked: false,
    };
    let base = if full == b"fixture/empty" {
        domain::unborn_repository(&mut domain, &CONFIG, setup);
        0
    } else {
        domain::repository(&mut domain, &CONFIG, setup)
    };
    domain::grant(&mut domain, full, 1, api::Permission::Write);
    domain::grant(&mut domain, full, 2, api::Permission::Read);
    let forge = Rc::new(RefCell::new(Forge::new(domain, CONFIG)));
    let mut transport = Typed::new(forge, &table(full, protected));
    let identity = identity();
    transport.authenticate(Some(&(identity.login, identity.token)));
    let topic =
        if base == 0 { 0 } else { transport.store(base, None, tree(b"topic\n"), b"topic").expect("changed tree") };
    if base != 0 {
        transport.push(&remote(full), b"topic", topic, None).expect("new topic");
    }
    let at = b"workspaces/1/specimen".to_vec();
    let mut checkout = Checkout::new();
    if base != 0 {
        local::clone_repository(&mut transport, &mut checkout, &remote(full), &at).expect("fixture clone");
        local::check_out(&transport, &mut checkout, &at, base).expect("fixture checkout");
    }
    let mut scenario = Scenario { transport, checkout, base, topic, at, remote: remote(full) };
    if base != 0 {
        apply(&mut scenario, &Command::Add);
    }
    scenario
}
fn remote(full: &[u8]) -> Vec<u8> {
    [b"http://127.0.0.1:3000/".as_slice(), full, b".git"].concat()
}
fn tree(content: &[u8]) -> Tree {
    BTreeMap::from([(b"conflict.txt".to_vec(), content.to_vec())])
}
fn files(tree: Tree) -> Box<[api::File]> {
    tree.into_iter().map(|(path, content)| api::File { path: path.into(), content: content.into() }).collect()
}
fn commit(number: u64) -> git::Commit {
    invocation::object(&id(number)).expect("counted id")
}
fn apply(scenario: &mut Scenario, command: &Command) -> Answer {
    let mut spawn = spawn_of(command);
    spawn.dir = scenario.at.clone().into();
    let checked = check(&spawn).expect("fixture spawn");
    answer(&checked, &mut scenario.checkout, &mut scenario.transport)
}
fn ours(scenario: &mut Scenario) -> u64 {
    let number = scenario.transport.store(scenario.base, None, tree(b"ours\n"), b"ours").expect("changed tree");
    scenario.checkout.write(&[scenario.at.as_slice(), format!("/.git/objects/{number}").as_bytes()].concat(), b"");
    number
}
fn conflicted(scenario: &mut Scenario) {
    let ours = ours(scenario);
    apply(scenario, &Command::CheckOut { commit: commit(ours) });
    apply(scenario, &Command::Merge { theirs: commit(scenario.topic) });
}
fn stage_ours(scenario: &mut Scenario) -> git::Commit {
    scenario.checkout.write(&[scenario.at.as_slice(), b"/conflict.txt"].concat(), b"ours\n");
    apply(scenario, &Command::Add);
    let result = apply(scenario, &Command::WriteTree);
    invocation::object(result.stdout.strip_suffix(b"\n").expect("tree terminator")).expect("tree id")
}
/// Prepare independently of the transcript files, then run the actual fake.
#[must_use]
pub fn answer_case(case: &str, fault: Fault) -> Answer {
    let protected = match case {
        "create-protected" => Some(b"blocked".as_slice()),
        "push-protected" => Some(b"main".as_slice()),
        _ => None,
    };
    let full = match case {
        "create-protected" | "push-protected" => b"fixture/guarded".as_slice(),
        "clone-stranger-private" => b"fixture/private",
        "fetch-sha256-branch" => b"fixture/hashed",
        "clone-empty" => b"fixture/empty",
        _ => b"fixture/specimen",
    };
    let mut scenario = named(full, protected);
    let command = command_case(case, &mut scenario);
    let mut spawn = spawn_of(&command);
    spawn.dir = scenario.at.clone().into();
    match fault {
        Fault::None | Fault::MissingBranch | Fault::MissingCommit | Fault::ProtectedBranch => {}
        Fault::MissingRepository => {
            if let Command::Clone { directory, .. } = &command {
                spawn.args = invocation::arguments(&Command::Clone {
                    remote: remote(b"fixture/missing").into(),
                    directory: directory.clone(),
                });
            }
        }
        Fault::WrongToken => {
            let mut identity = identity();
            identity.token = Secret::new(b"wrong-token".as_slice().into());
            spawn.env = invocation::environment(
                b"/worker/home",
                Some(if case.starts_with("clone") { git::Kind::Clone } else { git::Kind::Fetch }),
                Some(&identity),
            );
        }
        Fault::Unreachable => {
            let mut forge = scenario.transport.forge.borrow_mut();
            domain::set_reachable(&mut forge.domain, full, false);
        }
        Fault::NoPermission => {
            let mut identity = identity();
            identity.login = b"stranger".as_slice().into();
            identity.token = Secret::new(b"fake-git-stranger-token".as_slice().into());
            spawn.env = invocation::environment(b"/worker/home", Some(git::Kind::Push), Some(&identity));
        }
    }
    let mut checked = check(&spawn).expect("case builder spawn");
    checked.message = if case == "commit-tree-two-parents" {
        b"Merge\n\nTwo parents.\n".to_vec()
    } else {
        b"Fixture\n\nOne parent.\n".to_vec()
    };
    answer(&checked, &mut scenario.checkout, &mut scenario.transport)
}
fn command_case(case: &str, scenario: &mut Scenario) -> Command {
    let identity = identity();
    scenario.transport.authenticate(Some(&(identity.login, identity.token)));
    let remote: Box<[u8]> = scenario.remote.clone().into();
    let base = commit(scenario.base);
    match case {
        "version" => Command::Version,
        "clone"
        | "clone-empty"
        | "clone-missing-repository"
        | "clone-stranger-private"
        | "clone-wrong-token"
        | "clone-refused-connection" => clone_case(case, scenario, remote),
        "fetch-branch" => Command::Fetch { remote, source: Source::Branch(b"topic".as_slice().into()) },
        "fetch-sha256-branch" => Command::Fetch { remote, source: Source::Branch(b"main".as_slice().into()) },
        "fetch-missing-branch" => Command::Fetch { remote, source: Source::Branch(b"missing".as_slice().into()) },
        "fetch-missing-commit" => Command::Fetch { remote, source: Source::Commit(commit(u64::MAX)) },
        "fetch-wrong-token" => Command::Fetch { remote, source: Source::Default },
        "fetch-commit" => {
            apply(
                scenario,
                &Command::Fetch { remote: remote.clone(), source: Source::Branch(b"topic".as_slice().into()) },
            );
            Command::Fetch { remote, source: Source::Commit(base) }
        }
        "fetch-default" | "fetch-up-to-date" => {
            apply(scenario, &Command::Fetch { remote: remote.clone(), source: Source::Default });
            Command::Fetch { remote, source: Source::Default }
        }
        "checkout-1" => Command::CheckOut { commit: base },
        "checkout-2" => {
            scenario.checkout.write(&[scenario.at.as_slice(), b"/untracked"].concat(), b"discard\n");
            Command::Clean
        }
        "merge-clean" => Command::Merge { theirs: commit(scenario.topic) },
        "merge-up-to-date" => Command::Merge { theirs: base },
        "merge-conflict" => {
            let ours = ours(scenario);
            apply(scenario, &Command::CheckOut { commit: commit(ours) });
            Command::Merge { theirs: commit(scenario.topic) }
        }
        "merge-conflict-2" | "commit-unmerged" => {
            conflicted(scenario);
            Command::Unmerged
        }
        "commit-add" => Command::Add,
        "commit-unchanged" => Command::DiffIndex { parent: base },
        "commit-changed" => {
            stage_ours(scenario);
            Command::DiffIndex { parent: base }
        }
        "commit-write-tree" => {
            stage_ours(scenario);
            Command::WriteTree
        }
        "commit-tree-one-parent" | "commit-tree-two-parents" => {
            let tree = stage_ours(scenario);
            Command::CommitTree {
                tree,
                parent: base,
                merging: (case == "commit-tree-two-parents").then(|| commit(scenario.topic)),
            }
        }
        "create-new-branch" | "create-exists-at-commit" | "create-exists-elsewhere" => {
            if case != "create-new-branch" {
                scenario.transport.create(&remote, b"created", scenario.base).expect("fixture branch");
            }
            Command::Create {
                remote,
                branch: b"created".as_slice().into(),
                commit: if case == "create-exists-elsewhere" { commit(scenario.topic) } else { base },
            }
        }
        "create-protected" => Command::Create { remote, branch: b"blocked".as_slice().into(), commit: base },
        "push-ancestor-yes" | "push-ancestor-no" => {
            let ours = commit(ours(scenario));
            if case == "push-ancestor-yes" {
                Command::IsAncestor { expected: base, commit: ours }
            } else {
                Command::IsAncestor { expected: ours, commit: base }
            }
        }
        "push-fast-forward"
        | "push-new-branch"
        | "push-up-to-date"
        | "push-non-fast-forward"
        | "push-lease-stale"
        | "push-no-permission"
        | "push-protected" => push_case(case, scenario),
        _ => panic!("unknown captured case {case}"),
    }
}
fn push_case(case: &str, scenario: &mut Scenario) -> Command {
    let mut ours = ours(scenario);
    let remote = scenario.remote.clone();
    if matches!(case, "push-up-to-date" | "push-non-fast-forward" | "push-lease-stale") {
        scenario.transport.push(&remote, b"main", ours, None).expect("fixture push");
    }
    if case == "push-lease-stale" {
        ours = scenario.transport.commit_tree(ours, None, tree(b"ours\n"), b"Stale lease\n");
        scenario.checkout.write(&[scenario.at.as_slice(), format!("/.git/objects/{ours}").as_bytes()].concat(), b"");
    }
    Command::Push {
        remote: remote.into(),
        commit: commit(if case == "push-non-fast-forward" { scenario.base } else { ours }),
        branch: Box::from(match case {
            "push-new-branch" => b"pushed".as_slice(),
            "push-no-permission" => b"forbidden",
            _ => b"main",
        }),
        expected: (case == "push-lease-stale").then(|| commit(scenario.base)),
    }
}
/// Ignore an id's width while preserving identity equality and zero ids.
/// Real captures contain SHA-256 and abbreviated SHA-1; the fake counts SHA-1.
#[must_use]
pub fn masked(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut ids = BTreeMap::new();
    let mut at = 0;
    while at < bytes.len() {
        let length = bytes[at..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        if (7..=64).contains(&length)
            && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric())
            && bytes.get(at + length).is_none_or(|b| !b.is_ascii_alphanumeric())
        {
            let digits = &bytes[at..at + length];
            let counted = length == 40
                && u64::from_str_radix(std::str::from_utf8(&digits[16..32]).expect("ASCII count"), 16)
                    .is_ok_and(|count| id(count).as_slice() == digits);
            let digits = if counted { &digits[..16] } else { digits };
            let number =
                u64::from_str_radix(std::str::from_utf8(digits).expect("ASCII id"), 16).expect("masked or counted id");
            if number == 0 {
                out.extend(b"<zero>");
            } else {
                let next = ids.len() + 1;
                let value = *ids.entry(number).or_insert(next);
                out.extend(format!("<id:{value}>").as_bytes());
            }
            at += length;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

fn clone_case(case: &str, scenario: &mut Scenario, remote: Box<[u8]>) -> Command {
    scenario.checkout.remove(&scenario.at);
    scenario.at = b"workspaces/1".to_vec();
    let directory = match case {
        "clone-empty" => b"empty".as_slice(),
        "clone-missing-repository" => b"missing",
        "clone-stranger-private" => b"private",
        "clone-wrong-token" => b"wrong",
        "clone-refused-connection" => b"refused",
        _ => b"specimen",
    };
    Command::Clone { remote, directory: directory.into() }
}
