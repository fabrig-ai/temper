use super::fixtures::{Harness, grouped_steps, long, once};
use crate::host::{LINE_BYTES, io_limits};
use alloc::boxed::Box;
use skein_io::kernel::{Done, Exit, Target};
use skein_lib::{Duration, List, bytes};
use skein_sim::{Event, Summary};

#[test]
fn the_steps_start_in_order_and_a_stop_leaves_no_child() {
    let steps = [
        once(b"prepare", &[b"exit", b"0"]),
        long(b"forgejo", &[b"never"]),
        long(b"runner", &[b"never"]),
        once(b"once-1", &[b"exit", b"0"]),
        once(b"once-2", &[b"exit", b"0"]),
        long(b"serve", &[b"never"]),
    ];
    let mut harness = Harness::new(&steps);
    harness.run_until_idle();
    let mut names = List::with_capacity(6);
    for launch in &harness.launches {
        names.push(launch.name.clone()).expect("six launches");
    }
    let expected: Box<[Box<[u8]>]> = Box::new([
        bytes::copy_of(b"prepare"),
        bytes::copy_of(b"forgejo"),
        bytes::copy_of(b"runner"),
        bytes::copy_of(b"once-1"),
        bytes::copy_of(b"once-2"),
        bytes::copy_of(b"serve"),
    ]);
    assert_eq!(names.as_slice(), expected.as_ref(), "the setup's step order");
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0), "a requested stop succeeds");
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn an_owned_version_probe_stops_or_times_out_without_starting_the_runner() {
    for stop in [false, true] {
        let steps =
            [long(b"forgejo", &[b"never"]).with_ready(([127, 0, 0, 1], 3000).into()), long(b"runner", &[b"never"])];
        let mut harness = Harness::new(&steps);
        harness.run_until_idle();
        assert_eq!(harness.spawn_times(b"forgejo").len(), 1);
        assert_eq!(harness.spawn_times(b"runner").len(), 0, "no version endpoint answers");
        if stop {
            harness.terminate();
        }
        assert_eq!(harness.run_to_end(), Exit::Code(u8::from(!stop)));
        assert_eq!(harness.spawn_times(b"runner").len(), 0, "the barrier never passed");
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}
#[test]
fn a_stop_signals_each_long_step_twice_last_first() {
    let steps = [long(b"forgejo", &[b"never"]), long(b"runner", &[b"never"]), long(b"serve", &[b"never"])];
    let mut harness = Harness::with_stubborn(&steps, true);
    harness.run_until_idle();
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0), "a stop ultimately closes stubborn children");
    let mut children = List::with_capacity(3);
    let mut signals = List::with_capacity(6);
    for entry in harness.sim.trace() {
        if entry.pid == harness.pid {
            if let Event::Complete { result: Ok(Done::Spawned { pidfd, .. }), .. } = &entry.event {
                children.push(*pidfd).expect("three children");
            }
            if let Event::Submit { kind: Summary::Signal { pidfd, signal: 15, to: Target::Child }, .. } = &entry.event {
                signals.push((*pidfd, entry.at)).expect("two termination signals per child");
            }
        }
    }
    assert_eq!(signals.len(), 6, "each stubborn child is signalled twice");
    for (index, child) in children.iter().rev().enumerate() {
        let start = u32::try_from(index).expect("three children").saturating_mul(2);
        let first = signals.get(start).expect("first signal");
        let second = signals.get(start.saturating_add(1)).expect("second signal");
        assert_eq!((first.0, second.0), (*child, *child), "last child first");
        assert!(second.1.saturating_since(first.1) >= Duration::from_secs(10), "a grace between signals");
    }
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn a_long_step_that_keeps_exiting_is_given_up_naming_it() {
    let steps = [once(b"prepare", &[b"exit", b"0"]), long(b"forgejo", &[b"exit", b"1"])];
    let mut harness = Harness::new(&steps);
    assert_eq!(harness.run_to_end(), Exit::Code(1));
    let spawns = harness.spawn_times(b"forgejo");
    assert_eq!(spawns.len(), 6, "the first run and five restarts");
    for (pair, gap) in spawns.windows(2).zip([500, 1_000, 2_000, 4_000, 8_000]) {
        assert!(pair[1].saturating_since(pair[0]) >= Duration::from_millis(gap), "the backoff doubles");
    }
    assert_eq!(harness.spawn_times(b"prepare").len(), 6, "each restart runs prepare again");
    assert!(Harness::contains(&harness.stderr(), b"forgejo exited 1; giving up\n"), "give-up names step");
}
#[test]
fn a_once_step_that_fails_ends_the_box_with_exit_one() {
    let steps = [once(b"prepare", &[b"exit", b"1"]), long(b"forgejo", &[b"never"])];
    let mut harness = Harness::new(&steps);
    assert_eq!(harness.run_to_end(), Exit::Code(1));
    assert_eq!(&*harness.stderr(), b"Logs: /data/box/logs/\nStarting prepare\nprepare exited 1; giving up\n");
    assert_eq!(harness.spawn_times(b"forgejo").len(), 0, "failure never starts the next step");
}
#[test]
fn every_step_keeps_the_environment_except_inherited_activation_variables() {
    let steps = [once(b"prepare", &[b"exit", b"0"]), long(b"serve", &[b"never"])];
    let mut harness = Harness::new(&steps);
    harness.run_until_idle();
    let expected: Box<[Box<[u8]>]> = Box::new([bytes::copy_of(b"HOME=/box"), bytes::copy_of(b"TOKEN=whole=value")]);
    assert_eq!(harness.launches.len(), 2, "both steps ran");
    for launch in &harness.launches {
        assert_eq!(launch.environment, expected, "activation variables are removed and other entries stay whole");
    }
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
}
#[test]
fn the_io_limits_are_usable_for_one_six_and_sixty_four_steps() {
    for count in [1_u32, 6, 64] {
        let limits = io_limits(count);
        assert!(limits.is_usable(), "usable for {count} steps");
        assert_eq!(limits.sockets, count.saturating_mul(4).saturating_add(5));
        assert_eq!((limits.intake, limits.receive, limits.names), (LINE_BYTES, 1, 0));
        assert_eq!((limits.sends, limits.accepts, limits.backlog), (1, 1, 1));
        assert_eq!(limits.close_timeout, Duration::from_secs(5));
        assert_eq!(limits.retry, Duration::from_millis(10));
        let mut steps = List::with_capacity(count);
        steps.push(long(b"serve", &[b"never"])).unwrap();
        if count > 1 {
            steps.push(super::fixtures::link(b"seed", &[b"exit", b"0"])).unwrap();
        }
        for _ in steps.len()..count {
            steps.push(long(b"worker", &[b"never"])).unwrap();
        }
        let mut harness = Harness::with_stubborn(steps.as_slice(), true);
        harness.run_until_idle();
        if count > 1 {
            harness.write_child(harness.serve_index(), 1, b"link\n");
            harness.run_until_idle();
        }
        assert_eq!(harness.launches.len(), count);
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}

#[test]
fn serves_pipe_lines_are_bounded_named_and_answered() {
    let mut too_long = List::with_capacity(257);
    for _ in 0_u32..256 {
        too_long.push(b'x').unwrap();
    }
    too_long.push(b'\n').unwrap();
    for (name, input, expected, diagnostic) in [
        ("ask", b"start 3\n".as_slice(), b"step 3 prepare\nstep 3 run\nstarted 3\n".as_slice(), b"".as_slice()),
        ("unknown forge", b"start 99\n", b"failed 99 not asked\n", b""),
        ("unknown line", b"secret unknown\n", b"", b"box: serve: unknown pipe line\n"),
        ("too long", too_long.as_slice(), b"", b"box: serve: pipe line too long\n"),
    ] {
        let mut harness = Harness::grouped(grouped_steps(), &[], true);
        harness.run_until_idle();
        assert_eq!(harness.launches.len(), 1, "{name}: dormant until asked");
        assert_eq!(
            harness.launches.get(0).unwrap().pipes.as_ref(),
            &[
                skein_io::kernel::Pipe { child: 1, way: skein_io::kernel::Way::Out, parent: None },
                skein_io::kernel::Pipe { child: 2, way: skein_io::kernel::Way::Out, parent: None },
                skein_io::kernel::Pipe { child: 3, way: skein_io::kernel::Way::Out, parent: None },
                skein_io::kernel::Pipe { child: 4, way: skein_io::kernel::Way::In, parent: None },
            ]
        );
        harness.ask(input);
        harness.run_until_idle();
        assert_eq!(&*harness.reply_bytes(), expected, "{name}");
        let lifecycle =
            if name == "ask" { b"Forge 3 (Forgejo): starting\nForge 3 (Forgejo): started\n".as_slice() } else { b"" };
        assert_eq!(
            &*harness.stderr(),
            crate::lines::joined(&[b"Logs: /data/box/logs/\nStarting serve\n", lifecycle, diagnostic]).as_ref(),
            "{name}"
        );
        if !diagnostic.is_empty() {
            harness.ask(b"start 99\n");
            harness.run_until_idle();
            assert_eq!(&*harness.reply_bytes(), b"failed 99 not asked\n", "{name}: next line recovered");
        }
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}
#[test]
fn records_are_probed_before_start_and_written_private_or_reported() {
    use skein_fake_machine::{How, Item};
    let present =
        [Item::directory(b"box"), Item::directory(b"box/started"), Item::file(b"box/started/3", b"").mode(0o600)];
    let unreadable =
        [Item::directory(b"box"), Item::directory(b"box/started"), Item::file(b"box/started/3", b"").mode(0)];
    let blocked = [Item::file(b"box", b"kept")];
    let denied = [Item::directory(b"box"), Item::directory(b"box/started").mode(0o500)];
    for (name, items, recorded, writable, why) in [
        ("absent", &[][..], false, true, b"".as_slice()),
        ("present", present.as_slice(), true, true, b""),
        ("unreadable", unreadable.as_slice(), false, true, b"permission denied"),
        ("directory blocked", blocked.as_slice(), false, false, b"not a directory"),
        ("create denied", denied.as_slice(), false, false, b""),
    ] {
        let mut harness = Harness::grouped(grouped_steps(), items, true);
        harness.run_until_idle();
        assert_eq!(harness.spawn_times(b"prepare 3").len(), usize::from(recorded), "{name}: startup record");
        let diagnostics = harness.stderr();
        if !why.is_empty() {
            assert!(Harness::contains(&diagnostics, b"box/started/3"), "{name}");
            assert!(Harness::contains(&diagnostics, why), "{name}");
        }
        harness.ask(b"start 3\n");
        harness.run_until_idle();
        let replies = harness.reply_bytes();
        if recorded {
            assert_eq!(&*replies, b"started 3\n");
        } else if writable {
            assert_eq!(&*replies, b"step 3 prepare\nstep 3 run\nstarted 3\n");
        } else {
            assert_eq!(&*replies, b"step 3 prepare\nstep 3 run\nfailed 3 record not written\n");
            assert!(Harness::contains(&harness.stderr(), b"box/started/3"));
        }
        if writable && name != "unreadable" {
            let root = harness.machine.reopen_root(0);
            let file = harness.machine.open(root, b"box/started/3", How::Read).unwrap();
            assert_eq!(harness.machine.stat(file).mode & 0o777, 0o600, "{name}: private record");
            assert!(harness.machine.read(file, 0, 1).unwrap().is_empty());
            harness.machine.close(file);
            harness.machine.close(root);
        }
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
        assert_eq!(harness.machine.open_handles(), 0, "{name}: every opened record closed");
    }
}
#[test]
fn an_exited_serve_loses_its_answers_and_gets_fresh_pipes() {
    let mut harness = Harness::grouped(grouped_steps(), &[], true);
    harness.run_until_idle();
    let old = harness.serve_index();
    let child = harness.hosted.get(old).unwrap();
    let old_output = child.output.unwrap();
    let old_input = child.input.unwrap();
    let pid = child.pid;
    harness.hold_spawn = Some(bytes::copy_of(b"prepare 3"));
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    assert!(harness.held_spawn.is_some());
    harness.sim.finish_service(pid, Exit::Code(1));
    harness.run_until_idle();
    harness.replies.clear();
    harness.release_spawn();
    harness.run_until_idle();
    assert!(harness.replies.is_empty(), "answers to the exited serve are dropped");
    harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500)));
    harness.run_until_idle();
    assert_eq!(harness.spawn_times(b"serve").len(), 2);
    let fresh = harness.hosted.get(harness.serve_index()).unwrap();
    assert_ne!(fresh.pid, pid);
    assert!(fresh.output.is_some() && fresh.input.is_some());
    assert_ne!(harness.control_fds.get(0), harness.control_fds.get(1));
    assert!(old_output.raw() > 0_i32 && old_input.raw() > 0_i32);
    assert!(harness.replies.is_empty(), "old pending answers were dropped");
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    assert_eq!(&*harness.reply_bytes(), b"started 3\n");
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn a_serve_that_does_not_read_holds_the_next_ask_until_replies_are_written() {
    for buffer in [8, 16] {
        let mut harness = Harness::buffered(grouped_steps(), &[], true, buffer);
        harness.run_until_idle();
        harness.ask(b"start 3\n");
        harness.run_until_idle();
        harness.ask(b"start 4\n");
        harness.run_until_idle();
        assert!(!Harness::contains(&harness.reply_bytes(), b"failed 4"), "buffer {buffer}: next ask waits");
        let expected = b"step 3 prepare\nstep 3 run\nstarted 3\nfailed 4 not asked\n";
        let mut received = List::with_capacity(u32::try_from(expected.len()).unwrap());
        for _ in 0..expected.len() {
            if usize::try_from(received.len()).unwrap() == expected.len() {
                break;
            }
            let remaining = u32::try_from(expected.len()).unwrap().saturating_sub(received.len());
            for byte in harness.read_reply(remaining) {
                received.push(byte).unwrap();
            }
            harness.run_until_idle();
        }
        assert_eq!(received.as_slice(), expected, "buffer {buffer}: replies stay in order");
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "the link's byte and lifecycle cases share the hosted-child table")]
fn a_link_receives_only_the_first_line_and_its_end() {
    use super::fixtures::link;
    use skein_shell::Host;
    enum Action {
        Write(&'static [u8], usize, Box<[u8]>),
        Read(Box<[u8]>),
        Exit(&'static [u8]),
        WriteExit(&'static [u8], Box<[u8]>),
        BrokenPipe,
        Advance,
        Stdout(Box<[u8]>),
        Stderr(&'static [u8]),
        Spawns(&'static [u8], usize),
    }
    type Case<'a> = (&'a str, &'a [&'a [u8]], Duration, Box<[Action]>);
    use Action::{Advance, Exit as End, Read, Spawns, Stderr, Stdout, Write};
    fn write(name: &'static [u8], fd: usize, data: &[u8]) -> Action {
        Write(name, fd, bytes::copy_of(data))
    }
    fn read(data: &[u8]) -> Action {
        Read(bytes::copy_of(data))
    }
    fn stdout(data: &[u8]) -> Action {
        Stdout(bytes::copy_of(data))
    }
    let mut full = List::with_capacity(LINE_BYTES);
    for _ in 0..LINE_BYTES {
        full.push(b'x').unwrap();
    }
    let full = full.into_boxed();
    let past = crate::lines::joined(&[full.as_ref(), b"tail\nlater\n"]);
    let cases: [Case<'_>; 9] = [
        (
            "whole",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([write(b"serve", 1, b"link\nlater\n"), read(b"link\n"), read(b""), stdout(b"later\n")]),
        ),
        (
            "two chunks",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([
                write(b"serve", 1, b"li"),
                Spawns(b"seed", 0),
                stdout(b""),
                write(b"serve", 1, b"nk\nlater\n"),
                read(b"link\n"),
                read(b""),
                stdout(b"later\n"),
            ]),
        ),
        (
            "at cap",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([write(b"serve", 1, &full), read(&full), read(b""), stdout(b"")]),
        ),
        (
            "past cap",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([write(b"serve", 1, &past), read(&full), read(b""), stdout(b"tail\nlater\n")]),
        ),
        (
            "incomplete exit",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([
                write(b"serve", 1, b"half"),
                End(b"serve"),
                stdout(b"half"),
                Spawns(b"seed", 0),
                Advance,
                write(b"serve", 1, b"restart\nlater\n"),
                read(b"restart\n"),
                read(b""),
                stdout(b"later\n"),
            ]),
        ),
        (
            "seed exits unread",
            &[b"exit".as_slice(), b"0"][..],
            Duration::from_millis(500),
            Box::new([
                write(b"serve", 1, b"link\nlater\n"),
                Spawns(b"seed", 1),
                Action::BrokenPipe,
                stdout(b"later\n"),
            ]),
        ),
        (
            "seed output",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([
                write(b"serve", 1, b"link\nlater\n"),
                read(b"link\n"),
                read(b""),
                write(b"seed", 1, b"out\n"),
                write(b"seed", 2, b"err\n"),
                Stderr(b"seed: out\nseed: err\n"),
                stdout(b"later\n"),
            ]),
        ),
        (
            "restart after seed",
            &[b"never".as_slice()][..],
            Duration::from_millis(500),
            Box::new([
                write(b"serve", 1, b"link\n"),
                read(b"link\n"),
                read(b""),
                End(b"serve"),
                Advance,
                write(b"serve", 1, b"fresh\nlater\n"),
                Spawns(b"seed", 1),
                stdout(b"fresh\nlater\n"),
            ]),
        ),
        (
            "line with immediate restart",
            &[b"never".as_slice()][..],
            Duration::from_millis(0),
            Box::new([
                Action::WriteExit(b"serve", bytes::copy_of(b"link\n")),
                read(b"link\n"),
                read(b""),
                Spawns(b"serve", 2),
                Spawns(b"seed", 1),
                stdout(b""),
            ]),
        ),
    ];
    for (name, seed_args, backoff, actions) in cases {
        let steps = temper_forge_setup::Steps {
            always: Box::new([long(b"serve", &[b"never"]), link(b"seed", seed_args)]),
            asked: Box::new([]),
        };
        let limits = crate::limits::Limits { backoff, ..crate::limits::small() };
        let mut harness = Harness::configured(steps, &[], true, skein_sim::Config::calm().buffer, &limits);
        harness.run_until_idle();
        for action in actions {
            match action {
                Write(child, fd, data) => harness.write_child(harness.child_index(child), fd, &data),
                Read(expected) => {
                    let seed = harness.child_index(b"seed");
                    let mut received = List::with_capacity(LINE_BYTES);
                    // A pipe may return a short read; read again until the expected line is whole.
                    for _ in 0..expected.len().max(1) {
                        let most = u32::try_from(
                            expected.len().saturating_sub(usize::try_from(received.len()).unwrap()).max(1),
                        )
                        .unwrap();
                        let piece = harness.read_stdin(seed, most);
                        for byte in &piece {
                            received.push(*byte).unwrap();
                        }
                        if piece.is_empty() || usize::try_from(received.len()).unwrap() == expected.len() {
                            break;
                        }
                    }
                    assert_eq!(received.as_slice(), expected.as_ref(), "{name}: stdin");
                }
                End(child) => {
                    let pid = harness.hosted.get(harness.child_index(child)).unwrap().pid;
                    harness.sim.finish_service(pid, Exit::Code(0));
                }
                Action::WriteExit(child, data) => {
                    let index = harness.child_index(child);
                    harness.write_child(index, 1, &data);
                    let pid = harness.hosted.get(index).unwrap().pid;
                    harness.sim.finish_service(pid, Exit::Code(0));
                }
                Action::BrokenPipe => {
                    let mut found = false;
                    for entry in harness.sim.trace() {
                        if let Event::Complete { result: Err(skein_io::kernel::Error::BrokenPipe), .. } = &entry.event {
                            found = true;
                        }
                    }
                    assert!(found, "{name}: the stdin write failed");
                }
                Advance => harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500))),
                Stdout(expected) => assert_eq!(harness.stdout().as_ref(), expected.as_ref(), "{name}: stdout"),
                Stderr(expected) => assert!(
                    Harness::contains(&harness.stderr(), expected),
                    "{name}: child stderr remains visible alongside lifecycle and block notices"
                ),
                Spawns(child, expected) => assert_eq!(harness.spawn_times(child).len(), expected, "{name}: launches"),
            }
            harness.run_until_idle();
        }
        assert_eq!(harness.service.exit(), None, "{name}: link stdin alone can fail");
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0), "{name}: stop");
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}

#[test]
fn an_unrecorded_group_neither_probes_nor_writes_and_waits_at_every_box_start() {
    use skein_fake_machine::{How, Item};
    // A non-directory would expose either an accidental probe or a write.
    for _restart in 0_u8..2 {
        let mut steps = grouped_steps();
        steps.asked[0].recorded = false;
        let mut harness = Harness::grouped(steps, &[Item::file(b"box", b"kept")], true);
        harness.run_until_idle();
        assert_eq!(harness.launches.len(), 1, "only serve starts");
        assert_eq!(
            harness.stderr().as_ref(),
            b"box: cannot write /data/box/logs/: NotADirectory; the steps' lines follow here\nStarting serve\n",
            "no probe of box/started/3"
        );
        harness.ask(b"start 3\n");
        harness.run_until_idle();
        assert_eq!(&*harness.reply_bytes(), b"step 3 prepare\nstep 3 run\nstarted 3\n");
        assert_eq!(
            harness.stderr().as_ref(),
            b"Forge 3 (Forgejo): starting\nForge 3 (Forgejo): started\n",
            "unrecorded group emits its lifecycle without a record write"
        );
        let root = harness.machine.reopen_root(0);
        let file = harness.machine.open(root, b"box", How::Read).unwrap();
        assert_eq!(harness.machine.read(file, 0, 4).unwrap().as_slice(), b"kept");
        harness.machine.close(file);
        harness.machine.close(root);
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
        assert_eq!(harness.machine.open_handles(), 0);
    }
}

#[test]
fn a_pipe_stop_closes_only_its_group_keeps_the_record_and_can_be_started_again() {
    use skein_fake_machine::How;
    let mut harness = Harness::grouped(grouped_steps(), &[], true);
    harness.run_until_idle();
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    assert_eq!(&*harness.reply_bytes(), b"step 3 prepare\nstep 3 run\nstarted 3\n");
    harness.replies.clear();
    harness.ask(b"stop 3\n");
    harness.run_until_idle();
    assert!(harness.reply_bytes().is_empty(), "stubborn group has not closed yet");
    for _ in 0_u32..2 {
        harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_secs(10)));
        harness.run_until_idle();
    }
    assert_eq!(&*harness.reply_bytes(), b"stopped 3\n");
    assert!(Harness::contains(&harness.stderr(), b"Forge 3 (Forgejo): stopped\n"));
    assert_eq!(harness.spawn_times(b"serve").len(), 1);
    assert_eq!(harness.spawn_times(b"forgejo 3").len(), 1);
    let root = harness.machine.reopen_root(0);
    let record = harness.machine.open(root, b"box/started/3", How::Read).expect("stop keeps the record");
    harness.machine.close(record);
    harness.machine.close(root);
    harness.replies.clear();
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    assert_eq!(&*harness.reply_bytes(), b"step 3 prepare\nstep 3 run\nstarted 3\n");
    assert_eq!(harness.spawn_times(b"forgejo 3").len(), 2);
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
    assert_eq!(harness.machine.open_handles(), 0);
}

#[test]
fn a_group_stop_is_reported_on_stderr_after_the_control_pipe_closes() {
    let mut steps = grouped_steps();
    steps.asked.get_mut(0).unwrap().forge = 1;
    let mut harness = Harness::grouped(steps, &[], true);
    harness.run_until_idle();
    harness.ask(b"start 1\n");
    harness.run_until_idle();
    harness.replies.clear();
    harness.ask(b"stop 1\n");
    harness.run_until_idle();
    assert_eq!(
        harness.stderr().as_ref(),
        b"Logs: /data/box/logs/\nStarting serve\nForge 3 (Forgejo): starting\nForge 3 (Forgejo): started\n",
        "the group has not closed"
    );
    let serve = harness.hosted.get(harness.serve_index()).unwrap().pid;
    let forge = harness.hosted.get(harness.child_index(b"forgejo 3")).unwrap().pid;
    harness.sim.finish_service(serve, Exit::Code(1));
    harness.sim.finish_service(forge, Exit::Code(0));
    harness.run_until_idle();
    assert!(harness.reply_bytes().is_empty(), "the old control pipe has closed");
    assert_eq!(&*harness.stderr(), b"serve exited 1; restarting in 0.5 s\nForge 3 (Forgejo): stopped\n");
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn each_child_gets_only_its_listeners_and_serve_pipes_follow_them() {
    use skein_io::kernel::Way;
    let forge = ([127, 0, 0, 1], 8100).into();
    let clients = ([127, 0, 0, 1], 8101).into();
    let webhooks = ([127, 0, 0, 1], 8102).into();
    let steps = [
        once(b"prepare", &[b"exit", b"0"]),
        long(b"fake forge", &[b"never"]).listening(Box::new([forge])),
        long(b"serve", &[b"never"]).listening(Box::new([([127, 0, 0, 1], 0).into(), clients, webhooks])),
    ];
    let mut harness = Harness::with_stubborn(&steps, true);
    harness.run_until_idle();
    assert_eq!(harness.launches.len(), 3);
    for (launch, count) in harness.launches.iter().zip([0, 1, 2]) {
        let mut given = 0_u32;
        for pipe in &launch.pipes {
            match pipe.way {
                Way::Given(_) => {
                    assert_eq!(pipe.child, given.checked_add(3).unwrap());
                    given = given.checked_add(1).unwrap();
                }
                Way::In | Way::Out => {}
            }
        }
        assert_eq!(given, count);
        let expected = match count {
            0 => None,
            1 => Some(b"LISTEN_FDS=1".as_slice()),
            _ => Some(b"LISTEN_FDS=2".as_slice()),
        };
        let mut activation = None;
        for entry in &launch.environment {
            if entry.starts_with(b"LISTEN_") {
                assert!(activation.is_none(), "at most one activation variable");
                activation = Some(entry.as_ref());
            }
        }
        assert_eq!(activation, expected);
    }
    let serve = harness.launches.get(2).unwrap();
    assert_eq!((serve.pipes[4].child, serve.pipes[4].way), (5, Way::Out));
    assert_eq!((serve.pipes[5].child, serve.pipes[5].way), (6, Way::In));
    // Receiving a control answer also proves Spawned.pipes still indexes real pipes only.
    harness.ask(b"start 99\n");
    harness.run_until_idle();
    assert_eq!(harness.reply_bytes().as_ref(), b"failed 99 not asked\n");
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn a_document_port_zero_step_is_given_no_listener_or_activation_environment() {
    use skein_io::kernel::Way;
    let steps = [long(b"serve", &[b"never"]).listening(Box::new([([127, 0, 0, 1], 0).into()]))];
    let mut harness = Harness::with_stubborn(&steps, true);
    harness.run_until_idle();
    let launch = harness.launches.get(0).unwrap();
    for pipe in &launch.pipes {
        match pipe.way {
            Way::Given(_) => panic!("port zero is not held or given"),
            Way::In | Way::Out => {}
        }
    }
    for entry in &launch.environment {
        assert!(!entry.starts_with(b"LISTEN_"));
    }
    assert_eq!((launch.pipes[2].child, launch.pipes[3].child), (3, 4));
    assert!(harness.hosted.get(0).unwrap().listeners.is_empty());
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn a_dormant_listener_queues_connections_and_is_given_again_after_restart() {
    use super::fixtures::{call, socket};
    use skein_io::kernel::{Fd, Op, Way};
    let address = ([127, 0, 0, 1], 8200).into();
    let mut steps = grouped_steps();
    steps.asked[0].steps[1].1 = long(b"fake forge 3", &[b"never"]).listening(Box::new([address]));
    let mut harness = Harness::grouped(steps, &[], true);
    harness.run_until_idle();
    assert_eq!(harness.launches.len(), 1, "forge remains dormant");
    let client = harness.sim.spawn_process();
    let connection = socket(&mut harness.sim, client);
    assert_eq!(call(&mut harness.sim, client, Op::Connect { fd: connection, addr: address }).result, Ok(Done::Nothing));
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    let index = harness.child_index(b"fake forge 3");
    let child = harness.hosted.get(index).unwrap();
    let pid = child.pid;
    assert_eq!(child.listeners.as_ref(), &[Fd::new(3)]);
    let accepted = call(&mut harness.sim, pid, Op::Accept { fd: Fd::new(3) });
    let Ok(Done::Accepted { fd, .. }) = accepted.result else { panic!("connection queued before ask is accepted") };
    assert_eq!(call(&mut harness.sim, pid, Op::Close { fd }).result, Ok(Done::Nothing));
    assert_eq!(call(&mut harness.sim, client, Op::Close { fd: connection }).result, Ok(Done::Nothing));
    let mut first = None;
    for launch in &harness.launches {
        if launch.name.as_ref() == b"fake forge 3" {
            assert!(first.is_none(), "one launch before restart");
            first = Some(launch.pipes[2]);
        }
    }
    let first = first.expect("forge was launched");
    let Way::Given(_) = first.way else { panic!("forge is given its listener") };
    harness.sim.finish_service(pid, Exit::Code(1));
    harness.run_until_idle();
    // The parent still holds the socket between children, so this new connection queues too.
    let connection = socket(&mut harness.sim, client);
    assert_eq!(call(&mut harness.sim, client, Op::Connect { fd: connection, addr: address }).result, Ok(Done::Nothing));
    harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500)));
    harness.run_until_idle();
    let mut launches = List::with_capacity(2);
    for launch in &harness.launches {
        if launch.name.as_ref() == b"fake forge 3" {
            launches.push(launch).unwrap();
        }
    }
    assert_eq!(launches.len(), 2);
    assert_eq!(launches.get(1).unwrap().pipes[2], first, "restart gets the parent's same held listener");
    let child = harness.hosted.get(harness.child_index(b"fake forge 3")).unwrap();
    assert_ne!(child.pid, pid);
    let pid = child.pid;
    let Ok(Done::Accepted { fd, .. }) = call(&mut harness.sim, pid, Op::Accept { fd: Fd::new(3) }).result else {
        panic!("restart accepts the connection queued between children")
    };
    call(&mut harness.sim, pid, Op::Close { fd });
    call(&mut harness.sim, client, Op::Close { fd: connection });
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
    harness.sim.assert_no_open_fds(client);
}

fn terminal_harness() -> Harness {
    Harness::with_stubborn(&[long(b"serve", &[b"never"]), long(b"diagnostic", &[b"never"])], true)
}
#[test]
fn split_links_and_diagnostics_form_ordered_first_fresh_and_restarted_blocks() {
    let mut harness = terminal_harness();
    harness.run_until_idle();
    let serve = harness.child_index(b"serve");
    let diagnostic = harness.child_index(b"diagnostic");
    harness.write_child(serve, 1, b"http://first");
    harness.run_until_idle();
    harness.write_child(diagnostic, 2, b"interposed\n");
    harness.run_until_idle();
    assert_eq!(harness.stdout().as_ref(), b"", "split URL stays in assembly");
    harness.write_child(serve, 1, b"/link\n");
    harness.run_until_idle();
    harness.wall = skein_lib::Wall::from_nanos(86_399_000_000_000);
    harness.write_child(serve, 1, b"http://fre");
    harness.run_until_idle();
    harness.write_child(diagnostic, 1, b"between refresh pieces\n");
    harness.run_until_idle();
    harness.write_child(serve, 1, b"sh\n");
    harness.run_until_idle();
    let pid = harness.hosted.get(serve).unwrap().pid;
    harness.sim.finish_service(pid, Exit::Code(0));
    harness.run_until_idle();
    harness.wall = skein_lib::Wall::from_nanos(86_400_000_000_000);
    harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500)));
    harness.run_until_idle();
    harness.write_child(harness.child_index(b"serve"), 1, b"http://restart\n");
    harness.run_until_idle();
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert_eq!(
        harness.terminal_bytes().as_ref(),
        concat!(
            "Logs: /data/box/logs/\nStarting serve\nStarting diagnostic\ndiagnostic: interposed\n",
            "\nOpen this link in a browser on this machine. It signs you in as this\n",
            "temper's owner, once and for a short while, so keep it private.\n",
            "Opened again once spent, it prints a fresh one here.\n\n",
            "http://first/link\n\nCtrl-C stops temper.\n\n",
            "diagnostic: between refresh pieces\n23:59:59Z A fresh sign-in link:\nhttp://fresh\n",
            "23:59:59Z serve exited 0; restarting in 0.5 s\n",
            "00:00:00Z Starting serve\n00:00:00Z A fresh sign-in link:\nhttp://restart\n",
            "00:00:00Z Stopping\n00:00:00Z Stopped\n",
        )
        .as_bytes()
    );
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn held_output_preserves_completion_wall_and_blocks_both_descriptors() {
    use skein_shell::Host;
    let mut harness = terminal_harness();
    harness.run_until_idle();
    harness.write_child(harness.child_index(b"serve"), 1, b"first\n");
    harness.run_until_idle();
    harness.hold_terminal_ack = true;
    harness.write_child(harness.child_index(b"diagnostic"), 2, b"held diagnostic\n");
    harness.run_until_idle();
    let before = harness.terminal_writes.len();
    harness.wall = skein_lib::Wall::from_nanos(86_399_000_000_000);
    // The existing io profile reads one byte at a time. Assemble the prefix
    // while room exists, then retain the final LF after diagnostics use that
    // room. Its completion is old even though extraction must wait.
    harness.write_child(harness.child_index(b"serve"), 1, b"fresh");
    harness.run_until_idle();
    for words in [b"queued one\n".as_slice(), b"queued two\n", b"later diagnostic\n"] {
        harness.write_child(harness.child_index(b"diagnostic"), 1, words);
        harness.run_until_idle();
    }
    harness.write_child(harness.child_index(b"serve"), 1, b"\n");
    harness.run_until_idle();
    assert_eq!(harness.terminal_writes.len(), before, "small Room and Send never release the active item");
    assert!(!harness.service.work_pending(harness.sim.now()), "blocked output lets the loop sleep");
    harness.wall = skein_lib::Wall::from_nanos(172_800_000_000_000);
    harness.release_terminal_ack();
    harness.run_until_idle();
    let all = harness.terminal_bytes();
    assert!(
        Harness::contains(&all, b"23:59:59Z A fresh sign-in link:\nfresh\n"),
        "completion time survives admission pressure: {all:?}"
    );
    assert!(Harness::contains(&all, b"diagnostic: later diagnostic\n"));
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn serve_overflow_is_refused_without_bytes_then_eof_link_and_diagnostic_drain() {
    let mut harness = terminal_harness();
    harness.run_until_idle();
    let serve = harness.child_index(b"serve");
    let cap = super::fixtures::line_cap();
    harness.write_child(serve, 1, &cap);
    harness.run_until_idle();
    harness.write_child(serve, 1, b"x\ngood\nlast");
    harness.run_until_idle();
    harness.write_child(harness.child_index(b"diagnostic"), 2, b"EOF fragment");
    harness.run_until_idle();
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert_eq!(harness.stdout().as_ref(), b"good\nlast\n");
    let output = harness.terminal_bytes();
    assert!(!Harness::contains(&output, b"xxxx"), "no refused URL prefix or tail is written");
    assert!(Harness::contains(&output, b"box: serve: sign-in link too long\n\nOpen this link"));
    assert!(Harness::contains(&output, b"diagnostic: EOF fragment\n"));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn output_ack_bypasses_blocked_ordinary_events_without_spinning_or_losing_items() {
    use skein_shell::Host;
    let mut harness = Harness::with_stubborn(&[long(b"serve", &[b"never"])], true);
    harness.hold_terminal_ack = true;
    harness.run_until_idle();
    let (queued, orders) = crate::host::pressure::saturate(&mut harness.service);
    harness.run_until_idle();
    assert!(crate::host::pressure::blocked(&harness.service));
    assert!(!harness.service.work_pending(harness.sim.now()), "full queues without a credit are not runnable work");
    assert!(crate::host::pressure::bounded(&harness.service));
    harness.release_terminal_ack(); // Its full-budget Room arrives behind both retained ordinary events.
    harness.run_until_idle();
    let output = harness.terminal_bytes();
    assert_eq!(bytes::count(&output, b"queued\n", queued.saturating_add(1)), queued);
    assert_eq!(bytes::count(&output, b"Stopping\n", orders.saturating_add(1)), orders);
    assert!(crate::host::pressure::bounded(&harness.service));
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn failed_terminal_output_still_drains_retained_child_reads_and_closes() {
    use skein_io::kernel::Error;
    let mut harness = Harness::with_stubborn(&[long(b"serve", &[b"never"])], true);
    harness.hold_terminal_ack = true;
    harness.run_until_idle();
    // A first block cannot fit beside the active Starting notice. This leaves
    // admitted bytes in the pipe and io intake when the output breaks.
    harness.write_child(harness.child_index(b"serve"), 1, b"retained link\n");
    harness.run_until_idle();
    let mut failed = harness.held_terminal_ack.pop().expect("active terminal write");
    failed.result = Err(Error::BrokenPipe);
    harness.held_terminal_ack.push(failed);
    harness.release_terminal_ack();
    assert_eq!(harness.run_to_end(), Exit::Code(1));
    assert!(crate::host::pressure::bounded(&harness.service));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn a_legacy_handoff_does_not_print_or_mark_the_first_block() {
    use super::fixtures::link;
    let mut harness = Harness::with_stubborn(&[long(b"serve", &[b"never"]), link(b"seed", &[b"never"])], true);
    harness.run_until_idle();
    harness.write_child(harness.child_index(b"serve"), 1, b"held\n");
    harness.run_until_idle();
    assert_eq!(harness.terminal_bytes().as_ref(), b"Logs: /data/box/logs/\nStarting serve\nStarting seed\n");
    assert_eq!(harness.read_stdin(harness.child_index(b"seed"), LINE_BYTES).as_ref(), b"held\n");
    harness.write_child(harness.child_index(b"serve"), 1, b"visible\n");
    harness.run_until_idle();
    assert!(Harness::contains(&harness.terminal_bytes(), b"\nOpen this link in a browser on this machine."));
    assert_eq!(harness.stdout().as_ref(), b"visible\n");
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn labelled_group_failure_keeps_serve_usable_and_wire_replies_numeric() {
    let mut steps = grouped_steps();
    steps.asked[0].label = bytes::copy_of(b"other label\xff");
    steps.asked[0].steps[0].1 = once(b"prepare 3", &[b"exit", b"7"]);
    let mut harness = Harness::grouped(steps, &[], true);
    harness.run_until_idle();
    harness.write_child(harness.serve_index(), 1, b"http://first\n");
    harness.run_until_idle();
    harness.stderr();
    for _attempt in 0_u32..2 {
        harness.ask(b"start 3\n");
        harness.run_until_idle();
        assert_eq!(harness.reply_bytes().as_ref(), b"step 3 prepare\nfailed 3 prepare 3 exited 7\n");
        assert_eq!(
            harness.stderr().as_ref(),
            b"00:00:00Z other label\xff: starting\n00:00:00Z other label\xff: failed, prepare 3 exited 7\n"
        );
        harness.read_reply(256);
        harness.replies.clear();
        harness.ask(b"start 99\n");
        harness.run_until_idle();
        assert_eq!(harness.reply_bytes().as_ref(), b"failed 99 not asked\n");
        assert!(harness.stderr().is_empty(), "a refusal is a wire reply only");
        harness.read_reply(256);
        harness.replies.clear();
    }
    assert_eq!(harness.spawn_times(b"serve").len(), 1, "failure leaves the same serve usable");
    assert_eq!(harness.spawn_times(b"prepare 3").len(), 2);
    assert_eq!(harness.spawn_times(b"forgejo 3").len(), 0);
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert!(!Harness::contains(&harness.stderr(), b"giving up"), "group failure never becomes box failure");
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
    assert_eq!(harness.machine.open_handles(), 0);
}

#[test]
fn lifecycle_facts_keep_source_wall_and_order_under_terminal_backpressure() {
    use skein_lib::Wall;
    use skein_shell::Host;
    let mut limits = crate::limits::small();
    limits.restarts = 1;
    let mut harness = Harness::configured(
        temper_forge_setup::Steps {
            always: Box::new([long(b"serve", &[b"never"]), long(b"other", &[b"never"])]),
            asked: Box::new([]),
        },
        &[],
        true,
        skein_sim::Config::calm().buffer,
        &limits,
    );
    harness.run_until_idle();
    harness.write_child(harness.serve_index(), 1, b"http://first\n");
    harness.run_until_idle();
    harness.hold_terminal_ack = true;
    harness.write_child(harness.child_index(b"other"), 2, b"held\n");
    harness.run_until_idle();
    for words in [b"queued one\n".as_slice(), b"queued two\n"] {
        harness.write_child(harness.child_index(b"other"), 2, words);
        harness.run_until_idle();
    }
    harness.wall = Wall::from_nanos(86_399_000_000_000);
    let serve = harness.hosted.get(harness.serve_index()).unwrap().pid;
    harness.sim.finish_service(serve, Exit::Signal(9));
    harness.run_until_idle();
    // Both the restart decision and its monotonic timer progress while output
    // waits for its ACK. The Starting fact then waits in the same ordered FIFO.
    harness.wall = Wall::from_nanos(86_400_000_000_000);
    harness.sim.advance_to(harness.sim.now().saturating_add(limits.backoff));
    harness.run_until_idle();
    assert_eq!(harness.spawn_times(b"serve").len(), 2);
    let serve = harness.hosted.get(harness.serve_index()).unwrap().pid;
    harness.sim.finish_service(serve, Exit::Code(0));
    harness.run_until_idle();
    assert_eq!(
        crate::host::pressure::outcome(&harness.service),
        Some(crate::machine::Outcome::GaveUp { step: 0, restarts: 1 })
    );
    assert_eq!(harness.service.exit(), None, "another child and terminal ACK remain outstanding");
    assert!(!Harness::contains(&harness.terminal_bytes(), b"giving up"));
    assert!(
        crate::host::pressure::fact_waits(
            &harness.service,
            crate::terminal::Fact::GivingUp {
                step: 0,
                why: crate::machine::Why::Exited { step: 0, exit: Exit::Code(0) }
            },
            Wall::from_nanos(86_400_000_000_000),
        ),
        "the give-up fact must wait unformatted in the policy queue"
    );
    assert!(crate::host::pressure::bounded(&harness.service));
    harness.wall = Wall::from_nanos(172_861_000_000_000);
    harness.release_terminal_ack();
    harness.run_until_idle();
    let output = harness.terminal_bytes();
    assert!(Harness::contains(&output, b"other: held\nother: queued one\nother: queued two\n23:59:59Z serve killed by signal 9; restarting in 0.5 s\n00:00:00Z Starting serve\n00:00:00Z serve exited 0; giving up\n"), "source walls and decision order survive output delay: {output:?}");
    assert_eq!(bytes::count(&output, b"giving up\n", 2), 1);
    assert_eq!(harness.service.exit(), None, "giving up is visible before the stubborn other child closes");
    assert_eq!(harness.run_to_end(), Exit::Code(1));
    assert_eq!(bytes::count(&harness.terminal_bytes(), b"giving up\n", 2), 1, "no final duplicate");
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn long_labels_pass_actual_output_admission_with_tiny_control_and_write_caps() {
    use skein_lib::Wall;
    for buffer in [8, 16] {
        let label: Box<[u8]> = Box::new([b'L'; 4352]);
        let name: Box<[u8]> = Box::new([b'N'; 80]);
        let mut steps = grouped_steps();
        steps.asked[0].label = label.clone();
        steps.asked[0].steps[0].1 = once(&name, &[b"exit", b"255"]);
        let limits = crate::limits::Limits { line_bytes: 64, ..crate::limits::small() };
        let mut harness = Harness::configured(steps, &[], true, buffer, &limits);
        harness.run_until_idle();
        harness.write_child(harness.serve_index(), 1, b"link\n");
        harness.run_until_idle();
        harness.wall = Wall::from_nanos(86_399_000_000_000);
        harness.ask(b"start 3\n");
        harness.run_until_idle();
        let expected_reply = crate::lines::joined(&[b"step 3 prepare\nfailed 3 ", name.get(..54).unwrap(), b"\n"]);
        let mut received = List::with_capacity(u32::try_from(expected_reply.len()).unwrap());
        for _ in 0..expected_reply.len() {
            if received.len() == u32::try_from(expected_reply.len()).unwrap() {
                break;
            }
            for byte in harness.read_reply(u32::try_from(expected_reply.len()).unwrap().saturating_sub(received.len()))
            {
                received.push(byte).unwrap();
            }
            harness.run_until_idle();
        }
        assert_eq!(received.as_slice(), expected_reply.as_ref(), "wire WHY is still truncated to the 64-byte line cap");
        let expected = crate::lines::joined(&[
            b"23:59:59Z ",
            &label,
            b": starting\n23:59:59Z ",
            &label,
            b": failed, ",
            &name,
            b" exited 255\n",
        ]);
        assert!(
            Harness::contains(&harness.terminal_bytes(), &expected),
            "long lifecycle lines must actually write with buffer {buffer}"
        );
        assert!(crate::host::pressure::bounded(&harness.service));
        harness.terminate();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
        assert_eq!(harness.machine.open_handles(), 0);
    }
}

#[test]
fn an_unsent_restart_is_withdrawn_while_old_output_waits_on_the_terminal() {
    let mut harness = Harness::with_stubborn(&[long(b"serve", &[b"never"])], true);
    harness.hold_terminal_ack = true;
    harness.run_until_idle();
    let serve = harness.child_index(b"serve");
    harness.write_child(serve, 1, b"x");
    harness.run_until_idle();
    let pid = harness.hosted.get(serve).unwrap().pid;
    harness.sim.finish_service(pid, Exit::Code(0));
    harness.run_until_idle();
    harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500)));
    harness.run_until_idle();
    assert_eq!(harness.spawn_times(b"serve").len(), 1, "old EOF fragment holds the restart unsent");
    harness.terminate();
    harness.run_until_idle();
    harness.release_terminal_ack();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert_eq!(harness.spawn_times(b"serve").len(), 1, "withdrawn restart never launches");
    assert_eq!(harness.stdout().as_ref(), b"x\n", "old output is preserved");
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn a_held_spawn_starts_late_and_is_stopped_or_closed_after_second_termination() {
    for forced in [false, true] {
        let mut harness = Harness::with_stubborn(
            &[long(b"forgejo", &[b"never"]).with_ready(([127, 0, 0, 1], 3000).into()), long(b"runner", &[b"never"])],
            true,
        );
        harness.hold_spawn = Some(bytes::copy_of(b"forgejo"));
        harness.run_until_idle();
        assert!(harness.held_spawn.is_some());
        harness.terminate();
        harness.run_until_idle();
        if forced {
            harness.terminate();
            harness.run_until_idle();
        }
        harness.release_spawn();
        assert_eq!(harness.run_to_end(), Exit::Code(0));
        let mut signals = 0_u32;
        for entry in harness.sim.trace() {
            if entry.pid == harness.pid
                && let Event::Submit { kind: Summary::Signal { signal: 15, to: Target::Child, .. }, .. } = entry.event
            {
                signals += 1;
            }
        }
        assert_eq!(signals, if forced { 0 } else { 2 });
        assert_eq!(harness.spawn_times(b"forgejo").len(), 1);
        assert_eq!(harness.spawn_times(b"runner").len(), 0, "late child is never readied or continued");
        harness.sim.assert_quiescent(harness.pid);
        harness.sim.assert_no_open_fds(harness.pid);
    }
}
#[test]
fn a_held_spawn_failing_during_stop_ends_without_retry_or_failure_notice() {
    let mut harness = Harness::with_stubborn(&[long(b"forgejo", &[b"never"]), long(b"runner", &[b"never"])], true);
    harness.hold_spawn = Some(bytes::copy_of(b"forgejo"));
    harness.run_until_idle();
    assert!(harness.held_spawn.is_some());
    harness.terminate();
    harness.run_until_idle();
    harness.fail_spawn();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert_eq!(harness.spawn_times(b"forgejo").len(), 1, "counts the failed call too");
    assert_eq!(harness.spawn_times(b"runner").len(), 0);
    assert!(!Harness::contains(&harness.stderr(), b"restarting"));
    assert!(!Harness::contains(&harness.stderr(), b"giving up"));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
#[test]
fn a_groups_late_restart_preparation_is_stopped_then_the_group_can_start_again() {
    let mut harness = Harness::grouped(grouped_steps(), &[], true);
    harness.run_until_idle();
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    harness.read_reply(256);
    harness.replies.clear();
    harness.hold_spawn = Some(bytes::copy_of(b"prepare 3"));
    let forge = harness.child_index(b"forgejo 3");
    let pid = harness.hosted.get(forge).unwrap().pid;
    harness.sim.finish_service(pid, Exit::Code(1));
    harness.run_until_idle();
    harness.sim.advance_to(harness.sim.now().saturating_add(Duration::from_millis(500)));
    harness.run_until_idle();
    assert!(harness.held_spawn.is_some());
    harness.ask(b"stop 3\n");
    harness.run_until_idle();
    assert!(harness.reply_bytes().is_empty(), "stop awaits the admitted preparation");
    harness.release_spawn();
    harness.run_until_idle();
    assert_eq!(harness.reply_bytes().as_ref(), b"stopped 3\n");
    assert_eq!(harness.spawn_times(b"forgejo 3").len(), 1, "late once step never continues");
    harness.read_reply(256);
    harness.replies.clear();
    harness.ask(b"start 3\n");
    harness.run_until_idle();
    assert_eq!(harness.reply_bytes().as_ref(), b"step 3 prepare\nstep 3 run\nstarted 3\n");
    assert_eq!(harness.spawn_times(b"forgejo 3").len(), 2);
    harness.terminate();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}

#[test]
fn a_spawn_order_ahead_of_a_blocked_stop_notice_still_never_launches() {
    let mut harness = Harness::with_stubborn(&[once(b"prepare", &[b"never"]), long(b"serve", &[b"never"])], true);
    harness.hold_terminal_ack = true;
    harness.run_until_idle();
    let prepare = harness.child_index(b"prepare");
    for line in [b"one\n".as_slice(), b"two\n", b"three\n", b"four\n"] {
        harness.write_child(prepare, 1, line);
        harness.run_until_idle();
    }
    let pid = harness.hosted.get(prepare).unwrap().pid;
    harness.sim.finish_service(pid, Exit::Code(0));
    harness.run_until_idle();
    assert_eq!(harness.spawn_times(b"serve").len(), 0);
    harness.terminate();
    harness.run_until_idle();
    harness.release_terminal_ack();
    assert_eq!(harness.run_to_end(), Exit::Code(0));
    assert_eq!(harness.spawn_times(b"serve").len(), 0, "queued withdrawal prevents admission behind a full terminal");
    harness.sim.assert_quiescent(harness.pid);
    harness.sim.assert_no_open_fds(harness.pid);
}
