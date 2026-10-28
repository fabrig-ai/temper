//! protocol/git.md, section 9: the program, its version, and none at all.
use super::fixtures::{self, Started};
use crate::{Component, Event, Request};
use alloc::boxed::Box;
use temper_worker_domain_checkout::git;

const FIRST: &[u8] = b"/fake/first";
const SECOND: &[u8] = b"/fake/second";
const CURRENT: &[u8] = b"git version 2.41.0\n";
const REMOTE: &[u8] = b"https://forge.example.test:8443/acme/app.git";

struct Row {
    name: &'static str,
    machine: bool,
    previous: &'static [&'static [u8]],
    item: Option<&'static [u8]>,
    check: bool,
    version: &'static [u8],
    status: u8,
    remote: bool,
    diagnostic: Option<&'static [u8]>,
}

#[test]
#[expect(clippy::too_many_lines, reason = "the program contract stays in one table, with data-only rows")]
fn the_repository_program_is_checked_reused_or_absent() {
    let rows = [
        Row {
            name: "item matching startup reuses its answer",
            machine: true,
            previous: &[],
            item: Some(b"/bin/git"),
            check: false,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "item replaces settings at first use",
            machine: true,
            previous: &[],
            item: Some(FIRST),
            check: true,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "same item reuses answer",
            machine: true,
            previous: &[FIRST],
            item: Some(FIRST),
            check: false,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "different item checked afresh",
            machine: true,
            previous: &[FIRST],
            item: Some(SECOND),
            check: true,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "only last answer retained",
            machine: true,
            previous: &[FIRST, SECOND],
            item: Some(FIRST),
            check: true,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "old item fails operation unrun",
            machine: true,
            previous: &[],
            item: Some(FIRST),
            check: true,
            version: b"git version 2.40.9\n",
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "failed version fails operation unrun",
            machine: true,
            previous: &[],
            item: Some(FIRST),
            check: true,
            version: CURRENT,
            status: 128,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "settings supplies program",
            machine: true,
            previous: &[],
            item: None,
            check: false,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "no settings version at start, item still works",
            machine: false,
            previous: &[],
            item: Some(FIRST),
            check: true,
            version: CURRENT,
            status: 0,
            remote: false,
            diagnostic: None,
        },
        Row {
            name: "no program remote invocation fails 128",
            machine: false,
            previous: &[],
            item: None,
            check: false,
            version: CURRENT,
            status: 128,
            remote: true,
            diagnostic: Some(b"fatal: no git on this machine for forge.example.test:8443"),
        },
        Row {
            name: "no program local invocation fails 128",
            machine: false,
            previous: &[],
            item: None,
            check: false,
            version: CURRENT,
            status: 128,
            remote: false,
            diagnostic: Some(b"fatal: no git on this machine"),
        },
    ];
    for row in rows {
        let mut world = start(row.machine);
        for program in row.previous {
            world.down(request(Some(program), false));
            invocation(&mut world, program, true, CURRENT, 0);
            checkout(&mut world, program);
            assert_eq!(world.done(), Some((fixtures::OWNER, git::Done::Succeeded)), "{} prior", row.name);
            world.component.reclaim();
        }
        world.down(request(row.item, row.remote));
        if let Some(diagnostic) = row.diagnostic {
            assert_eq!(
                world.done(),
                Some((
                    fixtures::OWNER,
                    git::Done::FailedWithOutput {
                        fault: git::Fault::Broken,
                        diagnostic: git::PushDiagnostic::new(diagnostic, 0),
                    }
                )),
                "{}",
                row.name
            );
            // The same classifier result as an actual fatal exit, status 128.
            let Request::Io { op, .. } = request(None, row.remote) else { unreachable!("operation fixture") };
            let crate::sequence::Next::Run(command) = crate::sequence::first(&op) else { unreachable!() };
            assert_eq!(
                crate::sequence::after(&op, &command, 128, b"", diagnostic, 0, &fixtures::checkout()),
                crate::sequence::Next::End(git::Done::FailedWithOutput {
                    fault: git::Fault::Broken,
                    diagnostic: git::PushDiagnostic::new(diagnostic, 0)
                })
            );
        } else {
            let program = row.item.unwrap_or(b"/bin/git");
            if row.check {
                invocation(&mut world, program, true, row.version, row.status);
            }
            if row.version == CURRENT && row.status == 0 {
                checkout(&mut world, program);
                assert_eq!(world.done(), Some((fixtures::OWNER, git::Done::Succeeded)), "{}", row.name);
            } else {
                assert_eq!(
                    world.done(),
                    Some((fixtures::OWNER, git::Done::Failed { fault: git::Fault::Broken })),
                    "{}",
                    row.name
                );
                // Cache an unusable answer as well as a usable one.
                world.component.reclaim();
                world.down(request(row.item, false));
                assert_eq!(world.done(), Some((fixtures::OWNER, git::Done::Failed { fault: git::Fault::Broken })));
            }
        }
        assert!(world.below.out.io.is_empty(), "{}: no extra spawn", row.name);
    }
}

fn start(machine: bool) -> Started {
    if machine {
        return Started::new();
    }
    let mut world = Started::unstarted();
    world.component = Component::new(
        &fixtures::limits(),
        &fixtures::checkout(),
        crate::Settings { git: None, ..fixtures::settings() },
    )
    .expect("valid");
    world.component.start(world.now, &mut world.below.out);
    assert!(world.below.out.io.is_empty(), "no start version spawn");
    for _ in 0_u8..7 {
        world.file_operation();
        world.complete_ok();
    }
    assert_eq!(world.below.out.events.pop(), Some(Event::Started));
    world.empty_sweep();
    world
}
#[expect(clippy::manual_map, reason = "explicit option match follows the step subset")]
fn request(program: Option<&[u8]>, remote: bool) -> Request {
    let mut at = fixtures::at();
    at.git = match program {
        Some(program) => Some(Box::from(program)),
        None => None,
    };
    Request::Io {
        owner: fixtures::OWNER,
        op: if remote {
            git::Op::Clone { at, remote: Box::from(REMOTE), identity: 1, account: None }
        } else {
            git::Op::CheckOut { at, commit: fixtures::COMMIT, clean: true }
        },
        deadline: fixtures::DEADLINE,
        identity: Some(fixtures::identity()),
    }
}
fn checkout(world: &mut Started, program: &[u8]) {
    for _ in 0_u8..2 {
        invocation(world, program, false, b"", 0);
    }
}
fn invocation(world: &mut Started, program: &[u8], version: bool, stdout: &[u8], status: u8) {
    for request in &world.below.out.io {
        if let skein_io::Request::Spawn { spawn, .. } = request {
            let actual = match spawn.args.last() {
                Some(arg) => arg.as_ref() == b"version",
                None => false,
            };
            assert_eq!(actual, version);
        }
    }
    let (child, pipes) = world.below.spawned_program(&mut world.component, world.now, program);
    world.stdout(pipes[0], stdout);
    world.end(pipes[0]);
    world.end(pipes[1]);
    world.exited(child, status);
    world.closed(child, &pipes);
}
