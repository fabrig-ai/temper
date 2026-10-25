//! Named scenarios and deterministic fault choices shared by both test files.
use crate::{
    io::{Fault, Invocation, Node, Script},
    referee::Expected,
    world::{Scenario, Stop},
};
use skein_io::kernel;
use skein_lib::{Duration, Secret, Time, Token};
use std::collections::BTreeMap;
use temper_worker_domain_checkout::{self as checkout, git};
use temper_worker_protocol_git::{Identity, Limits, Settings};

pub const OWNER: Token = Token::new(91);
pub const DEADLINE: Time = Time::from_nanos(1000);
pub const SEED: u64 = 71;
pub const ONE: git::Commit =
    git::Commit::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
pub const TWO: git::Commit =
    git::Commit::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
pub const SIX: git::Commit =
    git::Commit::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
pub const SEVEN: git::Commit =
    git::Commit::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
#[must_use]
pub fn checkout() -> checkout::Limits {
    checkout::Limits {
        workspaces: 2,
        repositories: 2,
        name_bytes: 64,
        message_bytes: 256,
        conflicts: 4,
        path_bytes: 64,
        remote_timeout: Duration::from_secs(30),
        local_timeout: Duration::from_secs(5),
        facts: 8,
    }
}
#[must_use]
pub fn limits() -> Limits {
    Limits { operations: 2, stdout_bytes: 512, stderr_bytes: 512, read_bytes: 8, sweep_entries: 2, sweep_bytes: 255 }
}
#[must_use]
pub fn settings() -> Settings {
    Settings {
        git: Some(Box::from(&b"/bin/git"[..])),
        home: Box::from(&b"/worker/home"[..]),
        root: kernel::Fd::new(3),
        prefix: (6_u64 << 60) + (1_u64 << 48),
        serial: 1000,
        largest_read: 64,
        largest_room: 8,
    }
}
#[must_use]
pub fn identity() -> Identity {
    Identity {
        token: Secret::new(Box::from(&b"secret-git!"[..])),
        login: Box::from(&b"temper"[..]),
        name: Box::from(&b"Temper Bot"[..]),
        email: Box::from(&b"bot@example.test"[..]),
    }
}
fn at() -> git::Place {
    git::Place { git: None, workspace: Token::new(7), repository: Box::from(&b"specimen"[..]) }
}
fn remote() -> Box<[u8]> {
    Box::from(&b"https://forge.example.test/fixture/specimen.git"[..])
}
#[must_use]
pub fn copy_script(script: &Script) -> Script {
    Script {
        invocations: script
            .invocations
            .iter()
            .map(|invocation| Invocation {
                transcript: crate::transcripts::Transcript {
                    argv: invocation.transcript.argv.clone(),
                    exit: invocation.transcript.exit,
                    stdout: invocation.transcript.stdout.clone(),
                    stderr: invocation.transcript.stderr.clone(),
                },
                fault: invocation.fault,
            })
            .collect(),
        exit_first: script.exit_first,
        file_failure: script.file_failure,
    }
}
fn scenario(op: git::Op, cases: &[&str], expected: Expected) -> Scenario {
    Scenario {
        op,
        script: Script {
            invocations: cases.iter().map(|case| Invocation::captured(case)).collect(),
            ..Script::default()
        },
        expected,
        stop: Stop::None,
        identity: true,
        files: BTreeMap::new(),
    }
}
#[must_use]
pub fn operations() -> Vec<(&'static str, Scenario)> {
    vec![
        ("make", scenario(git::Op::Make { workspace: Token::new(7) }, &[], Expected::done(git::Done::Succeeded))),
        (
            "clone",
            scenario(
                git::Op::Clone { at: at(), remote: remote(), identity: 1, account: None },
                &["clone"],
                Expected::done(git::Done::Succeeded),
            ),
        ),
        ("fetch", fetch()),
        (
            "create",
            scenario(
                git::Op::Create {
                    at: at(),
                    remote: remote(),
                    branch: Box::from(&b"created"[..]),
                    commit: ONE,
                    identity: 1,
                    account: None,
                },
                &["create-new-branch"],
                Expected::done(git::Done::Succeeded),
            ),
        ),
        (
            "checkout",
            scenario(
                git::Op::CheckOut { at: at(), commit: ONE },
                &["checkout-1", "checkout-2"],
                Expected::done(git::Done::Succeeded),
            ),
        ),
        (
            "merge",
            scenario(git::Op::Merge { at: at(), theirs: TWO }, &["merge-clean"], Expected::done(git::Done::Merged)),
        ),
        (
            "merge conflict",
            scenario(
                git::Op::Merge { at: at(), theirs: TWO },
                &["merge-conflict", "merge-conflict-2"],
                Expected::done(git::Done::Conflicted { files: Box::new([Box::from(&b"conflict.txt"[..])]) }),
            ),
        ),
        ("commit", commit(false)),
        ("merging commit", commit(true)),
        ("push", push(false)),
        ("leased push", push(true)),
        ("standalone-ancestry", {
            let mut query = scenario(
                git::Op::IsAncestor { at: at(), ancestor: TWO, commit: SIX },
                &["push-ancestor-yes"],
                Expected::done(git::Done::IsAncestor { contains: true }),
            );
            query.identity = false;
            query
        }),
    ]
}
#[must_use]
pub fn fetch() -> Scenario {
    scenario(
        git::Op::Fetch { at: at(), remote: remote(), want: git::Want::Default, identity: 1, account: None },
        &["fetch-default"],
        Expected::done(git::Done::Fetched { commit: TWO }),
    )
}
#[must_use]
pub fn commit(merging: bool) -> Scenario {
    scenario(
        git::Op::Commit {
            at: at(),
            parent: ONE,
            merging: merging.then_some(TWO),
            title: Box::from(&b"Title"[..]),
            body: Box::from(&b"Body"[..]),
            identity: 1,
        },
        if merging {
            &["commit-unmerged", "commit-add", "commit-write-tree", "commit-tree-two-parents"]
        } else {
            &["commit-add", "commit-changed", "commit-write-tree", "commit-tree-one-parent"]
        },
        Expected::done(git::Done::Committed { commit: if merging { SEVEN } else { SIX } }),
    )
}
#[must_use]
pub fn push(lease: bool) -> Scenario {
    scenario(
        git::Op::Push {
            at: at(),
            remote: remote(),
            commit: TWO,
            branch: Box::from(&b"main"[..]),
            expected: lease.then_some(ONE),
            identity: 1,
            account: None,
        },
        if lease { &["push-ancestor-yes", "push-fast-forward"] } else { &["push-fast-forward"] },
        Expected::done(git::Done::Succeeded),
    )
}
#[derive(Clone, Copy, Debug)]
pub enum Race {
    ExitFirst,
    CancelFirst,
}
#[must_use]
pub fn cancel_race(race: Race) -> Scenario {
    let mut scenario = fetch();
    match race {
        Race::ExitFirst => scenario.stop = Stop::ExitFirst,
        Race::CancelFirst => {
            scenario.stop = Stop::Cancel(1);
            scenario.expected = Expected::fault(git::Fault::Cancelled);
        }
    }
    scenario
}
#[must_use]
pub fn sweep() -> Scenario {
    let mut scenario = operations().remove(0).1;
    for path in ["trash/old", "trash/old/a", "trash/old/a/b", "trash/wide"] {
        scenario.files.insert(path.as_bytes().to_vec(), Node::Directory);
    }
    for path in ["trash/old/file", "trash/old/a/b/file", "trash/wide/a", "trash/wide/b", "trash/wide/c", "trash/wide/d"]
    {
        scenario.files.insert(path.as_bytes().to_vec(), Node::File(Box::from(&b"payload"[..])));
    }
    scenario.files.insert(b"trash/wide/link".to_vec(), Node::Link);
    scenario
}
#[must_use]
pub fn drawn(seed: u64) -> Scenario {
    let mut choices = operations();
    let index = usize::try_from(seed % u64::try_from(choices.len()).expect("cases")).expect("index");
    let mut scenario = choices.remove(index).1;
    scenario.script.exit_first = seed & 1 != 0;
    let count = u32::try_from(scenario.script.invocations.len()).expect("sequence");
    match seed % 6 {
        0 if count > 0 => {
            scenario.stop = Stop::Deadline(1 + u32::try_from(seed % u64::from(count)).expect("index"));
            scenario.expected = Expected::fault(git::Fault::TimedOut);
        }
        1 if count > 0 => {
            scenario.stop = Stop::Cancel(1 + u32::try_from(seed % u64::from(count)).expect("index"));
            scenario.expected = Expected::fault(git::Fault::Cancelled);
        }
        2 if count > 0 => {
            scenario.script.invocations.front_mut().expect("first").fault = Fault::Spawn;
            scenario.expected = Expected::fault(git::Fault::Broken);
        }
        3 if count > 0 => {
            scenario.script.invocations.front_mut().expect("first").fault = Fault::Signal;
            scenario.expected = Expected::fault(git::Fault::Broken);
        }
        4 => {
            scenario.identity = false;
            if scenario.op.is_remote() || scenario.op.kind() == git::Kind::Commit {
                scenario.expected = Expected::fault(git::Fault::Broken);
            }
        }
        _ => {}
    }
    scenario
}

/// Fill every operation slot and both output caps under the counting allocator.
/// The bound also covers this small driver's allocations, a stronger check.
#[must_use]
pub fn memory_peak() -> u64 {
    use crate::io::{Below, Input};
    use skein_world::domain::heap::Meter;
    use temper_worker_protocol_git::{Component, Event, Request};
    let meter = Meter::new();
    meter.start();
    let mut below = Below::new(Script { invocations: [Invocation::captured("version")].into(), ..Script::default() });
    let mut component = Component::new(&limits(), &checkout(), settings()).expect("valid");
    let mut seed = SEED;
    component.start(Time::ZERO, &mut below.out);
    for _ in 0..10_000 {
        below.drain();
        if let Some(event) = below.out.events.pop() {
            assert_eq!(event, Event::Started);
        }
        let Some(input) = below.next(&mut seed) else {
            break;
        };
        match input {
            Input::Io(event) => component.up(Time::ZERO, event, &mut below.out),
            Input::Kernel(complete) => component.complete(Time::ZERO, complete, &mut below.out),
        }
        component.reclaim();
    }
    let mut script = Script::default();
    for _ in 0..limits().operations {
        let mut invocation = Invocation::captured("fetch-default");
        invocation.transcript.stdout = vec![b'x'; 512].into_boxed_slice();
        invocation.transcript.stderr = vec![b'y'; 512].into_boxed_slice();
        script.invocations.push_back(invocation);
    }
    below.reset(script);
    for offset in 0..limits().operations {
        component.down(
            Time::ZERO,
            Request::Io {
                owner: Token::new(91 + u64::from(offset)),
                op: fetch().op,
                deadline: DEADLINE,
                identity: Some(identity()),
            },
            &mut below.out,
        );
        below.drain();
    }
    for _ in 0..10_000 {
        if below.out.events.pop().is_some() {
            component.reclaim();
        }
        below.drain();
        let Some(input) = below.next(&mut seed) else {
            break;
        };
        match input {
            Input::Io(event) => component.up(Time::ZERO, event, &mut below.out),
            Input::Kernel(complete) => component.complete(Time::ZERO, complete, &mut below.out),
        }
    }
    let measured = meter.end();
    meter.check(
        measured,
        temper_worker_protocol_git::worst_case(&limits(), &checkout()).expect("bound"),
        &"git and its driver",
    )
}
