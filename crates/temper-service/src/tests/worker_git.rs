use super::fixtures;
use crate::Worker;
use jig_domain_host as host;
use jig_protocol_config::{Path, Problem, Segment, What};
use skein_io::{self as io, kernel};
use skein_lib::{Time, Token, Wall};
use temper_worker_domain::{
    self as root,
    checkout::{Ending, Fact, git},
};
use temper_worker_protocol_git_world::io::Invocation;

#[test]
fn a_clone_reaches_git_with_its_identity_s_newest_value() {
    let mut stand = fixtures::started_worker();
    stand.worker.link(fixtures::NOW, fixtures::application_value(fixtures::IDENTITY, 1, b"tok-old"));
    stand.worker.link(fixtures::NOW, fixtures::application_value(fixtures::IDENTITY, 2, b"tok-new"));
    stand.worker.link(fixtures::NOW, fixtures::repository_assignment());
    let (_, clone) = stand.answer_until_spawn(b"clone");
    let newest = &b"GIT_CONFIG_VALUE_0=Authorization: Basic dGVtcGVyLWJvdDp0b2stbmV3"[..];
    assert!(clone.env.contains(&Box::from(newest)), "the newest generation reaches git");
    assert!(clone.env.contains(&Box::from(b"HOME=/worker/home".as_slice())));
    for argument in &clone.args {
        assert_eq!(skein_lib::bytes::find(argument, b"tok-"), None, "no token in an argument");
    }
}

#[test]
fn an_operation_whose_identity_has_no_value_fails_broken_without_git() {
    let mut stand = fixtures::started_worker();
    stand.worker.link(fixtures::NOW, fixtures::repository_assignment());
    for _ in 0_u16..1000 {
        if !stand.turn() {
            break;
        }
    }
    assert_eq!(stand.below.spawns, 1, "the version check alone spawned git");
    assert_eq!(ended(&mut stand.worker), Some(Ending::Failed { fault: git::Fault::Broken }));
}

#[test]
fn a_cancel_reaches_the_component_and_its_done_reaches_the_root() {
    let mut stand = fixtures::started_worker();
    stand.worker.link(fixtures::NOW, fixtures::application_value(fixtures::IDENTITY, 1, b"tok-new"));
    stand.worker.link(fixtures::NOW, fixtures::repository_assignment());
    let (owner, spawn) = stand.answer_until_spawn(b"clone");
    stand.below.script.invocations.push_back(Invocation::captured("clone"));
    stand.below.out.io.push(io::Request::Spawn { owner, spawn });
    stand.below.drain();
    stand.input();
    stand.worker.link(
        fixtures::NOW,
        jig_protocol_worker_link::Request::Down {
            down: host::LinkDown::Cancel { run: Token::new(7), attempt: Token::new(2) },
            parts: Box::new([]),
        },
    );
    for _ in 0_u16..1000 {
        if !stand.turn() {
            break;
        }
    }
    assert_eq!(stand.below.signals, 1);
    assert_eq!(ended(&mut stand.worker), Some(Ending::Failed { fault: git::Fault::Cancelled }));
}

#[test]
fn the_link_dials_only_once_the_git_component_has_started() {
    let mut stand = fixtures::WorkerStand::new();
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let Some(io::Request::Spawn { spawn, .. }) = stand.worker.io().iter().next() else { panic!("git starts first") };
    assert_eq!(spawn.args[26].as_ref(), b"version");
    for request in &*stand.worker.io() {
        if let io::Request::Connect { .. } = request {
            panic!("link cannot dial before git starts");
        }
    }
    let mut dials = 0_u32;
    for _ in 0_u16..1000 {
        stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
        for _ in 0..stand.worker.io().len() {
            let request = stand.worker.io().pop().unwrap();
            if let io::Request::Connect { .. } = request {
                assert!(stand.worker.git.ready);
                dials += 1;
            } else {
                stand.below.out.io.push(request);
            }
        }
        stand.drain();
        if !stand.input() {
            break;
        }
    }
    assert_eq!(dials, 1);
}

#[test]
fn an_unusable_git_refuses_the_worker_naming_its_program() {
    let mut stand = fixtures::WorkerStand::new();
    stand.below.script.invocations.front_mut().unwrap().transcript.stdout = b"git version 2.40.0\n".as_slice().into();
    for _ in 0_u16..1000 {
        if !stand.turn() {
            break;
        }
    }
    assert_eq!(
        stand.worker.refused().unwrap().problems.get(0),
        Some(&Problem {
            path: Path::root().member(b"application").member(b"workers").child(Segment::Index(0)).member(b"git"),
            what: What::Form,
        })
    );
    assert!(!stand.worker.git.ready);
}

#[test]
fn the_component_s_file_operations_leave_through_the_worker() {
    let mut stand = fixtures::WorkerStand::new();
    for _ in 0_u16..1000 {
        stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
        if let Some(submit) = stand.worker.submitted().pop() {
            assert_eq!(crate::tokens::prefix(submit.op), 6);
            let kernel::Op::MakeDirectory { dir: root, name: path, mode } = submit.kind else {
                panic!("layout starts with a directory")
            };
            assert_eq!(root, kernel::Fd::new(9));
            assert_eq!(path.as_ref(), b"trash");
            assert_eq!(mode, 0o700);
            return;
        }
        stand.drain();
        stand.input();
    }
    panic!("version success starts the layout");
}

fn ended(worker: &mut Worker) -> Option<Ending> {
    let mut last = None;
    while let Some(fact) = worker.root.pop_fact() {
        if let root::Fact::Checkout { fact: Fact::Ended { done, .. } } = fact {
            last = Some(done);
        }
    }
    last
}

#[test]
fn a_github_operation_takes_the_entry_of_its_items_account() {
    let mut stand = fixtures::started_worker();
    grant_github(&mut stand, 1, b"tok-second");
    stand.worker.link(fixtures::NOW, fixtures::github_assignment(Some(22)));
    let (owner, clone) = stand.answer_until_spawn(b"clone");
    assert_header(&clone, b"tok-second");
    assert_eq!(stand.finish_git(owner, clone, "clone"), git::Done::Succeeded);
    stand.route_git(Token::new(92), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-second");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
}

#[test]
fn an_account_without_an_entry_is_refused_as_not_installed_without_git() {
    for account in [Some(99), None] {
        let mut stand = fixtures::started_worker();
        grant_github(&mut stand, 1, b"tok-second");
        let mut op = fixtures::github_push();
        if let git::Op::Push { account: field, .. } = &mut op {
            *field = account;
        }
        let owner = Token::new(92);
        stand.route_git(owner, op);
        let Some(root::Event::Done { owner: echoed, done: git::Done::FailedWithOutput { fault, diagnostic } }) =
            stand.worker.events.pop()
        else {
            panic!("routing returns a git refusal to the root");
        };
        assert_eq!(echoed, owner);
        assert_eq!(fault, git::Fault::Refused);
        assert_eq!(diagnostic.output(), b"temper's App is not installed on the account owning app");
        assert_eq!(diagnostic.cut(), 0);
        stand.worker.link(fixtures::NOW, fixtures::github_assignment(account));
        for _ in 0_u16..1000 {
            if !stand.turn() {
                break;
            }
        }
        assert_eq!(stand.below.spawns, 1, "only the version check, {account:?}");
        assert_eq!(ended(&mut stand.worker), Some(Ending::Failed { fault: git::Fault::Refused }));
    }
}

#[test]
fn an_undecodable_value_is_dropped_and_its_operation_fails_broken() {
    use skein_lib::Secret;
    let valid = temper_protocol_worker_wire::identity(&fixtures::github_record(b"x-access-token", b"tok-second"));
    let truncated = Secret::new(valid.expose()[..valid.expose().len() - 1].into());
    let invalid = temper_protocol_worker_wire::identity(&fixtures::github_record(b"bad:login", b"tok-second"));
    for secret in [truncated, invalid] {
        let mut stand = fixtures::started_worker();
        stand.worker.link(fixtures::NOW, fixtures::github_value(1, secret));
        assert_eq!(stand.worker.undecodable_values, 1);
        assert_eq!(stand.worker.identities.newest(fixtures::IDENTITY, fixtures::NOW), None);
        stand.worker.link(fixtures::NOW, fixtures::github_assignment(Some(22)));
        for _ in 0_u16..1000 {
            if !stand.turn() {
                break;
            }
        }
        assert_eq!(stand.below.spawns, 1, "only the version check");
        assert_eq!(ended(&mut stand.worker), Some(Ending::Failed { fault: git::Fault::Broken }));
    }
}

#[test]
fn a_lapsed_git_value_fails_broken_without_git() {
    let mut stand = fixtures::started_worker();
    // The older, unbounded value must never return when generation 2 lapses.
    grant_github(&mut stand, 1, b"tok-old");
    let value = temper_protocol_worker_wire::identity(&fixtures::github_record(b"x-access-token", b"tok-new"));
    let mut grant = fixtures::github_value(2, value);
    let jig_protocol_worker_link::Request::Value { expires, .. } = &mut grant else { panic!("grant") };
    *expires = Time::from_nanos(10);
    stand.worker.link(fixtures::NOW, grant);
    stand.worker.iterate(Time::from_nanos(9), Wall::EPOCH);
    stand.route_git(Token::new(92), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-new");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);

    let spawns = stand.below.spawns;
    for now in [10, 11] {
        stand.worker.iterate(Time::from_nanos(now), Wall::EPOCH);
        // Even a late older grant cannot restore the unbounded value.
        grant_github(&mut stand, 1, b"tok-old");
        assert_broken_without_git(&mut stand);
        assert_eq!(stand.below.spawns, spawns);
    }
    stand.worker.link(fixtures::NOW, fixtures::application_value(fixtures::IDENTITY, 3, b"tok-forgejo"));
    stand.worker.iterate(Time::from_nanos(u64::MAX - 1), Wall::EPOCH);
    stand.route_git(Token::new(93), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    let header = b"GIT_CONFIG_VALUE_0=Authorization: Basic dGVtcGVyLWJvdDp0b2stZm9yZ2Vqbw==";
    assert!(push.env.contains(&Box::from(header.as_slice())), "Forgejo's unbounded grant stays valid");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
}

#[test]
fn a_newer_undecodable_value_pushes_out_the_older() {
    let mut stand = fixtures::started_worker();
    grant_github(&mut stand, 1, b"tok-old");
    stand.route_git(Token::new(92), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-old");
    // The component keeps its own copy across the undecodable replacement.
    let value = temper_protocol_worker_wire::identity(&fixtures::github_record(b"x-access-token", b"tok-new"));
    let truncated = skein_lib::Secret::new(value.expose()[..value.expose().len() - 1].into());
    stand.worker.link(fixtures::NOW, fixtures::github_value(2, truncated));
    assert_eq!(stand.worker.undecodable_values, 1);
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
    grant_github(&mut stand, 1, b"late-old");
    assert_broken_without_git(&mut stand);

    grant_github(&mut stand, 3, b"tok-new");
    stand.route_git(Token::new(93), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-new");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
    assert_eq!(stand.worker.undecodable_values, 1);
}

fn assert_broken_without_git(stand: &mut fixtures::WorkerStand) {
    let owner = Token::new(94);
    let spawns = stand.below.spawns;
    stand.route_git(owner, fixtures::github_push());
    let Some(root::Event::Done { owner: echoed, done }) = stand.worker.events.pop() else {
        panic!("the operation answers without IO")
    };
    assert_eq!(echoed, owner);
    assert_eq!(done, git::Done::Failed { fault: git::Fault::Broken });
    stand.drain();
    assert_eq!(stand.below.spawns, spawns, "no git invocation for a missing value");
}

#[test]
fn a_push_in_flight_across_a_new_generation_ends_on_the_value_it_began_with() {
    let mut stand = fixtures::started_worker();
    grant_github(&mut stand, 1, b"tok-old");
    let mut op = fixtures::github_push();
    // The ancestry check is one invocation of this same push, before its network invocation.
    if let git::Op::Push { expected, .. } = &mut op {
        *expected = Some(git::Commit::new([1; 32]));
    }
    stand.route_git(Token::new(92), op);
    let (owner, ancestry) = stand.answer_until_spawn(b"merge-base");
    assert_header(&ancestry, b"tok-old");
    grant_github(&mut stand, 2, b"tok-new");
    stand.below.script.invocations.push_back(Invocation::captured("push-ancestor-yes"));
    stand.below.out.io.push(io::Request::Spawn { owner, spawn: ancestry });
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-old");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
    stand.route_git(Token::new(93), fixtures::github_push());
    let (owner, push) = stand.answer_until_spawn(b"push");
    assert_header(&push, b"tok-new");
    assert_eq!(stand.finish_git(owner, push, "push-fast-forward"), git::Done::Succeeded);
}

#[test]
fn a_commit_on_github_takes_the_values_author() {
    use temper_protocol_worker_wire::v1;
    let mut stand = fixtures::started_worker();
    // An empty entry list still supplies a commit's author.
    let limits = fixtures::wire_limits();
    let mut record = fixtures::github_record(b"x-access-token", b"tok-second").into_parts();
    record.kind = v1::Kind::Github(
        v1::Entries::new(&limits, v1::EntriesParts { entries: skein_lib::List::with_capacity(0) }).unwrap(),
    );
    let value = temper_protocol_worker_wire::identity(&v1::Identity::new(&limits, record).unwrap());
    stand.worker.link(fixtures::NOW, fixtures::github_value(1, value));
    stand.route_git(
        Token::new(92),
        git::Op::Commit {
            at: fixtures::git_place(),
            parent: git::Commit::new([1; 32]),
            merging: None,
            title: b"Change".as_slice().into(),
            body: Box::default(),
            identity: fixtures::IDENTITY,
        },
    );
    for (command, transcript) in [
        (b"add".as_slice(), "commit-add"),
        (b"diff-index", "commit-changed"),
        (b"write-tree", "commit-write-tree"),
        (b"commit-tree", "commit-tree-one-parent"),
    ] {
        let (owner, spawn) = stand.answer_until_spawn(command);
        assert!(spawn.env.contains(&Box::from(b"GIT_AUTHOR_NAME=Temper App".as_slice())));
        assert!(spawn.env.contains(&Box::from(b"GIT_AUTHOR_EMAIL=app@example.test".as_slice())));
        assert!(spawn.env.contains(&Box::from(b"GIT_COMMITTER_NAME=Temper App".as_slice())));
        assert!(spawn.env.contains(&Box::from(b"GIT_COMMITTER_EMAIL=app@example.test".as_slice())));
        for value in &spawn.env {
            assert!(!value.starts_with(b"GIT_CONFIG_VALUE_0="));
        }
        if command == b"commit-tree" {
            let git::Done::Committed { .. } = stand.finish_git(owner, spawn, transcript) else {
                panic!("commit finishes")
            };
        } else {
            stand.below.script.invocations.push_back(Invocation::captured(transcript));
            stand.below.out.io.push(io::Request::Spawn { owner, spawn });
        }
    }
}

fn grant_github(stand: &mut fixtures::WorkerStand, generation: u64, token: &[u8]) {
    let record = fixtures::github_record(b"x-access-token", token);
    stand
        .worker
        .link(fixtures::NOW, fixtures::github_value(generation, temper_protocol_worker_wire::identity(&record)));
}
fn assert_header(spawn: &kernel::Spawn, token: &[u8]) {
    use skein_lib::Writer;
    let login = b"x-access-token:";
    let mut raw = Writer::new(login.len().checked_add(token.len()).unwrap());
    raw.put(login).unwrap();
    raw.put(token).unwrap();
    let raw = raw.finish();
    let length =
        skein_lib::base64::encoded_len(skein_lib::base64::Alphabet::Standard, u32::try_from(raw.len()).unwrap())
            .unwrap();
    let mut encoded = Writer::new(usize::try_from(length).unwrap());
    skein_lib::base64::encode(skein_lib::base64::Alphabet::Standard, &raw, &mut encoded).unwrap();
    let prefix = b"GIT_CONFIG_VALUE_0=Authorization: Basic ";
    let encoded = encoded.finish();
    let mut expected = Writer::new(prefix.len().checked_add(encoded.len()).unwrap());
    expected.put(prefix).unwrap();
    expected.put(&encoded).unwrap();
    let expected = expected.finish();
    assert!(spawn.env.contains(&expected), "the selected entry supplies the Basic header");
    let mut headers = 0_u32;
    for value in &spawn.env {
        if value.starts_with(b"GIT_CONFIG_VALUE_0=") {
            headers = headers.checked_add(1).unwrap();
        }
    }
    assert_eq!(headers, 1);
    for argument in &spawn.args {
        assert_eq!(skein_lib::bytes::find(argument, token), None, "no token in an argument");
    }
}
