use temper_fake_git_hosted::{Refusal, check, fixtures};
use temper_worker_protocol_git::invocation::{self, Command, Source};
#[test]
fn an_argument_vector_the_builders_did_not_make_exits_129() {
    let mut spawn = fixtures::spawn_of(&Command::WriteTree);
    let mut args = invocation::arguments(&Command::WriteTree).into_vec();
    args.push(Box::from(&b"--missing-ok"[..]));
    spawn.args = args.into_boxed_slice();
    assert_eq!(check(&spawn).err(), Some(Refusal::Usage));
}
#[test]
fn a_token_outside_the_header_or_a_path_variable_is_refused() {
    for (name, extra) in [("PATH", b"PATH=/usr/bin".as_slice()), ("token", b"HOME=/fake-git-identity-token")] {
        let mut spawn = fixtures::spawn_of(&Command::Fetch {
            remote: b"https://forge.example/acme/app".as_slice().into(),
            source: Source::Default,
        });
        let mut env = spawn.env.into_vec();
        env.retain(|entry| !entry.starts_with(b"HOME=") || !extra.starts_with(b"HOME="));
        env.push(extra.into());
        spawn.env = env.into_boxed_slice();
        assert!(matches!(check(&spawn), Err(Refusal::Broken(_))), "{name}");
    }
}
#[test]
fn a_remote_over_plaintext_off_loopback_is_refused() {
    for (url, expected) in [
        (b"http://forge.example/acme/app".as_slice(), Some(Refusal::Broken("plaintext remote off loopback"))),
        (b"http://127.0.0.1:3000/acme/app", None),
        (b"http://[::1]:3000/acme/app", None),
        (b"https://forge.example/acme/app", None),
    ] {
        let spawn = fixtures::spawn_of(&Command::Fetch { remote: url.into(), source: Source::Default });
        assert_eq!(check(&spawn).err(), expected, "{url:?}");
    }
}
#[test]
fn each_builder_vector_is_admitted_with_its_whole_environment() {
    let branch = b"main".as_slice();
    let remote = b"https://forge.example/acme/app".as_slice();
    let one = invocation::object(&temper_fake_git_hosted::id::id(1)).expect("counted id");
    let two = invocation::object(&temper_fake_git_hosted::id::id(2)).expect("counted id");
    let rows = [
        ("version", Command::Version),
        ("clone", Command::Clone { remote: remote.into(), directory: b"app".as_slice().into() }),
        ("fetch default", Command::Fetch { remote: remote.into(), source: Source::Default }),
        ("fetch branch", Command::Fetch { remote: remote.into(), source: Source::Branch(b"main".as_slice().into()) }),
        ("fetch commit", Command::Fetch { remote: remote.into(), source: Source::Commit(one) }),
        ("create", Command::Create { remote: remote.into(), branch: b"topic".as_slice().into(), commit: one }),
        ("checkout", Command::CheckOut { commit: one }),
        ("clean", Command::Clean),
        ("merge", Command::Merge { theirs: two }),
        ("unmerged", Command::Unmerged),
        ("add", Command::Add),
        ("diff", Command::DiffIndex { parent: one }),
        ("tree", Command::WriteTree),
        ("commit one parent", Command::CommitTree { tree: one, parent: one, merging: None }),
        ("commit two parents", Command::CommitTree { tree: one, parent: one, merging: Some(two) }),
        ("ancestor", Command::IsAncestor { expected: one, commit: two }),
        ("push", Command::Push { remote: remote.into(), commit: two, branch: branch.into(), expected: None }),
        (
            "push lease",
            Command::Push { remote: remote.into(), commit: two, branch: branch.into(), expected: Some(one) },
        ),
    ];
    for (name, command) in rows {
        let checked = check(&fixtures::spawn_of(&command)).expect(name);
        assert_eq!(checked.command, command, "{name}");
    }
}
#[test]
fn a_missing_or_reordered_setting_names_the_broken_contract() {
    for (name, change) in [("missing", false), ("reordered", true)] {
        let mut spawn = fixtures::spawn_of(&Command::WriteTree);
        let mut args = spawn.args.into_vec();
        if change {
            args.swap(1, 3);
        } else {
            args.drain(..2);
        }
        spawn.args = args.into_boxed_slice();
        assert_eq!(check(&spawn).err(), Some(Refusal::Broken("git settings")), "{name}");
    }
}
#[test]
fn an_admitted_credential_is_absent_from_debug_output() {
    let remote = b"https://forge.example/acme/app".as_slice();
    let spawn = fixtures::spawn_of(&Command::Fetch { remote: remote.into(), source: Source::Default });
    let checked = check(&spawn).expect("valid invocation");
    let text = format!("{checked:?}");
    assert!(!text.as_bytes().windows(fixtures::TOKEN.len()).any(|part| part == fixtures::TOKEN));
    assert!(!text.contains("ZmFrZS1naXQtaWRlbnRpdHktdG9rZW4="));
}

mod core {
    use super::fixtures;
    use skein_io::kernel::{self as k, Complete, Done, Exit, Fd, Op};
    use skein_lib::{Time, Wall};
    use skein_shell::Host;
    use std::collections::BTreeSet;
    use temper_fake_git::{Git, Start};
    use temper_worker_protocol_git::invocation::{self, Command};

    #[test]
    fn broken_spawns_report_the_rule_and_close_every_descriptor() {
        // Root, stdout, stderr, signal and an append inherited by this invocation.
        const DESCRIPTORS: [i32; 5] = [1, 2, 3, 4, 5];
        let rows = [
            (Box::new([]) as Box<[Box<[u8]>]>, invocation::environment(b"/home", None, None), "git settings"),
            (invocation::arguments(&Command::Version), Box::new([]), "fixed environment value"),
        ];
        for (args, env, rule) in rows {
            let spawn = k::Spawn {
                program: b"/git".as_slice().into(),
                args,
                env,
                root: Fd::new(1),
                dir: Box::new([]),
                pipes: Box::new([]),
            };
            let mut held: BTreeSet<_> = DESCRIPTORS.map(Fd::new).into_iter().collect();
            let start = Start {
                root: Fd::new(1),
                pipes: Box::new([(1, Fd::new(2)), (2, Fd::new(3))]),
                owned: held.iter().copied().collect(),
            };
            let mut git = Git::new(&spawn, start, fixtures::scenario().transport);
            let mut stderr: Vec<u8> = Vec::new();
            for _ in 0..32 {
                git.iterate(Time::ZERO, Wall::EPOCH);
                if let Some(exit) = git.exit() {
                    assert_eq!(exit, Exit::Code(125), "{rule}");
                    break;
                }
                let submit = git.submissions().pop().expect("broken invocation progresses");
                let result = match &submit.kind {
                    Op::PipeWrite { fd, bytes, from } => {
                        assert_eq!(*fd, Fd::new(3), "{rule}: stderr only");
                        stderr.extend(&bytes[usize::try_from(*from).expect("offset")..]);
                        Ok(Done::Count(u32::try_from(bytes.len()).expect("short diagnostic") - from))
                    }
                    Op::Close { fd } => {
                        assert!(held.remove(fd), "{rule}: closes once");
                        Ok(Done::Nothing)
                    }
                    other @ (Op::Socket { .. }
                    | Op::Bind { .. }
                    | Op::Listen { .. }
                    | Op::Accept { .. }
                    | Op::Connect { .. }
                    | Op::Recv { .. }
                    | Op::Send { .. }
                    | Op::Shutdown { .. }
                    | Op::Open { .. }
                    | Op::Read { .. }
                    | Op::Write { .. }
                    | Op::Append { .. }
                    | Op::Truncate { .. }
                    | Op::Sync { .. }
                    | Op::Lock { .. }
                    | Op::Stat { .. }
                    | Op::Rename { .. }
                    | Op::Remove { .. }
                    | Op::MakeDirectory { .. }
                    | Op::List { .. }
                    | Op::Spawn { .. }
                    | Op::Wait { .. }
                    | Op::Signal { .. }
                    | Op::Usage
                    | Op::Window { .. }
                    | Op::ReadSignal { .. }
                    | Op::PipeRead { .. }
                    | Op::Cancel { .. }) => panic!("broken invocation requested {other:?}"),
                };
                git.completions().push(Complete { op: submit.op, kind: submit.kind, result });
            }
            assert!(git.is_empty(), "{rule}: settled");
            assert!(held.is_empty(), "{rule}: all inherited descriptors closed");
            assert_eq!(stderr, format!("temper fake git: broken: {rule}\n").as_bytes());
        }
    }
}

#[test]
fn standalone_ancestry_admits_the_local_environment_without_a_credential() {
    let command = Command::IsAncestor {
        expected: temper_worker_domain_checkout::git::Commit::new([1; 32]),
        commit: temper_worker_domain_checkout::git::Commit::new([2; 32]),
    };
    let mut spawn = fixtures::spawn_of(&command);
    spawn.env = temper_worker_protocol_git::invocation::environment(
        b"/worker/home",
        Some(temper_worker_domain_checkout::git::Kind::IsAncestor),
        None,
    );
    let checked = check(&spawn).expect("local ancestry needs no credential");
    assert_eq!(checked.command, command);
    assert!(checked.credential.is_none());
}
