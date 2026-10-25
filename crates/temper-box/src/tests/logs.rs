use super::fixtures::{Harness, line_cap, long};
use crate::{
    host::Echo,
    limits::{self, RECORD_BYTES},
};
use skein_fake_machine::{How, Is, Item};
use skein_io::kernel::Exit;
use skein_lib::{List, Wall, bytes};
use temper_forge_setup::Steps;

fn harness(echo: Echo, items: &[Item], forge: bool, limits: &limits::Limits) -> Harness {
    let steps: Box<[_]> = if forge {
        Box::new([long(b"serve", &[b"never"]), long(b"forge 1", &[b"never"])])
    } else {
        Box::new([long(b"serve", &[b"never"])])
    };
    Harness::configured_echo(Steps { always: steps, asked: Box::new([]) }, items, true, 16_384, limits, echo)
}
fn read(h: &mut Harness, path: &[u8]) -> (Box<[u8]>, u32) {
    let root = h.machine.reopen_root(0);
    let fd = h.machine.open(root, path, How::ReadNoFollow).unwrap();
    let stat = h.machine.stat(fd);
    let got =
        if stat.is == Is::File { h.machine.read(fd, 0, 65_536).unwrap().into_boxed_slice() } else { Box::new([]) };
    h.machine.close(fd);
    h.machine.close(root);
    (got, stat.mode)
}
fn stop(h: &mut Harness) {
    h.terminate();
    assert_eq!(h.run_to_end(), Exit::Code(0));
    h.sim.assert_quiescent(h.pid);
    h.sim.assert_no_open_fds(h.pid);
    assert_eq!(h.machine.open_handles(), 0);
}
fn snapshot(h: &mut Harness) -> Box<[Item]> {
    let mut items = List::with_capacity(128);
    items.push(Item::directory(b"box").mode(0o700)).unwrap();
    for path in [b"box/logs".as_slice(), b"box/logs.previous"] {
        let root = h.machine.reopen_root(0);
        if let Ok(fd) = h.machine.open(root, path, How::DirectoryNoFollow) {
            items.push(Item::directory(path).mode(h.machine.stat(fd).mode)).unwrap();
            for (kind, name) in h.machine.list(fd, 32, 8192).unwrap() {
                assert_eq!(kind, Is::File);
                let full = crate::lines::joined(&[path, b"/", &name]);
                let (data, mode) = read(h, &full);
                items.push(Item::file(&full, &data).mode(mode)).unwrap();
            }
            h.machine.close(fd);
        }
        h.machine.close(root);
    }
    items.into_boxed()
}
#[test]
fn quiet_logs_are_private_timestamped_and_rotated_across_three_starts() {
    let mut h = harness(Echo::None, &[], true, &limits::small());
    h.run_until_idle();
    h.wall = Wall::from_nanos(1_792_764_200_000_000_000);
    let forge = h.child_index(b"forge 1");
    h.write_child(forge, 2, b"first\nsecond\n");
    h.run_until_idle();
    h.write_child(forge, 1, b"third\n");
    h.write_child(h.serve_index(), 2, b"serve diagnostic\n");
    h.write_child(h.serve_index(), 1, b"http://local/sign-in/link?secret=private\n");
    h.run_until_idle();
    let stderr = h.stderr();
    assert!(Harness::contains(&stderr, b"Logs: /data/box/logs/\nStarting serve\n"));
    assert!(Harness::contains(&stderr, b"Ctrl-C stops temper."));
    assert!(!Harness::contains(&stderr, b"forge 1: "));
    let (logged, mode) = read(&mut h, b"box/logs/forge-1.log");
    assert_eq!(mode, 0o600);
    assert_eq!(
        logged.as_ref(),
        b"2026-10-23T14:03:20Z first\n2026-10-23T14:03:20Z second\n2026-10-23T14:03:20Z third\n"
    );
    assert_eq!(read(&mut h, b"box/logs").1, 0o700);
    assert_eq!(read(&mut h, b"box/logs/serve.log").0.as_ref(), b"2026-10-23T14:03:20Z serve diagnostic\n");
    stop(&mut h);
    let items = snapshot(&mut h);
    let mut second = harness(Echo::None, &items, false, &limits::small());
    second.run_until_idle();
    assert_eq!(read(&mut second, b"box/logs.previous/forge-1.log").0, logged);
    second.write_child(second.serve_index(), 2, b"second run\n");
    second.run_until_idle();
    stop(&mut second);
    let items = snapshot(&mut second);
    let mut third = harness(Echo::None, &items, true, &limits::small());
    third.run_until_idle();
    assert_eq!(read(&mut third, b"box/logs.previous/serve.log").0.as_ref(), b"1970-01-01T00:00:00Z second run\n");
    assert!(read(&mut third, b"box/logs/serve.log").0.is_empty());
    assert!(read(&mut third, b"box/logs/forge-1.log").0.is_empty());
    stop(&mut third);
    let items = snapshot(&mut third);
    for item in &items {
        assert_ne!(item.path, b"box/logs.previous/forge-1.log");
        if let skein_fake_machine::Made::File(data) = &item.made {
            assert!(!Harness::contains(data, b"/sign-in/link"));
        }
    }
}
#[test]
fn rotation_preserves_whole_records_at_both_minimum_bounds() {
    for bound in [RECORD_BYTES, RECORD_BYTES + 1] {
        let mut h = harness(Echo::None, &[], true, &limits::Limits { log_bytes: bound, ..limits::small() });
        h.run_until_idle();
        let forge = h.child_index(b"forge 1");
        let cap = line_cap();
        for _ in 0_u32..2 {
            h.write_child(forge, 2, &cap);
            h.run_until_idle();
        }
        let record = crate::lines::joined(&[b"1970-01-01T00:00:00Z ", &cap, b"\n"]);
        assert_eq!(read(&mut h, b"box/logs/forge-1.log").0.as_ref(), record.as_ref());
        assert_eq!(read(&mut h, b"box/logs/forge-1.log.1").0.as_ref(), record.as_ref());
        h.write_child(forge, 2, b"short\nlast\n");
        h.run_until_idle();
        assert_eq!(
            read(&mut h, b"box/logs/forge-1.log").0.as_ref(),
            b"1970-01-01T00:00:00Z short\n1970-01-01T00:00:00Z last\n"
        );
        assert_eq!(read(&mut h, b"box/logs/forge-1.log.1").0.as_ref(), record.as_ref());
        stop(&mut h);
    }
}
#[test]
fn echo_writes_the_same_pieces_to_terminal_and_log() {
    let mut h = harness(Echo::Steps, &[], true, &limits::small());
    h.run_until_idle();
    h.write_child(h.child_index(b"forge 1"), 2, b"one\ntwo\n");
    h.run_until_idle();
    assert!(Harness::contains(&h.stderr(), b"forge 1: one\nforge 1: two\n"));
    assert_eq!(
        read(&mut h, b"box/logs/forge-1.log").0.as_ref(),
        b"1970-01-01T00:00:00Z one\n1970-01-01T00:00:00Z two\n"
    );
    stop(&mut h);
}
#[test]
fn a_refused_create_names_only_its_log_once_and_echoes_its_lines() {
    let mut h = harness(Echo::None, &[], true, &limits::small());
    h.refuse_log = Some(bytes::copy_of(b"forge-1.log"));
    h.run_until_idle();
    h.write_child(h.child_index(b"forge 1"), 2, b"first\nsecond\n");
    h.write_child(h.serve_index(), 2, b"private\n");
    h.run_until_idle();
    let output = h.stderr();
    assert_eq!(bytes::count(&output, b"box: cannot write", 2), 1);
    assert!(Harness::contains(
        &output,
        b"box: cannot write /data/box/logs/forge-1.log: NoSpace; the step's lines follow here\n"
    ));
    assert!(Harness::contains(&output, b"forge 1: first\nforge 1: second\n"));
    assert!(!Harness::contains(&output, b"serve: private"));
    assert_eq!(read(&mut h, b"box/logs/serve.log").0.as_ref(), b"1970-01-01T00:00:00Z private\n");
    stop(&mut h);
}
#[test]
fn an_inner_directory_is_untouched_and_disables_logging_once() {
    let items = [
        Item::directory(b"box"),
        Item::directory(b"box/logs"),
        Item::directory(b"box/logs.previous"),
        Item::directory(b"box/logs.previous/inner"),
    ];
    let mut h = harness(Echo::None, &items, true, &limits::small());
    h.run_until_idle();
    h.write_child(h.child_index(b"forge 1"), 2, b"forge diagnostic\n");
    h.write_child(h.serve_index(), 2, b"serve diagnostic\n");
    h.run_until_idle();
    let output = h.stderr();
    assert_eq!(bytes::count(&output, b"box: cannot write", 2), 1);
    assert!(Harness::contains(
        &output,
        b"box: cannot write /data/box/logs/: IsADirectory; the steps' lines follow here\n"
    ));
    assert!(Harness::contains(&output, b"forge 1: forge diagnostic\n"));
    assert!(Harness::contains(&output, b"serve: serve diagnostic\n"));
    assert_eq!(read(&mut h, b"box/logs.previous/inner").1, 0o755);
    stop(&mut h);
}
#[test]
fn a_start_cut_short_keeps_previous() {
    let items = [
        Item::directory(b"box"),
        Item::directory(b"box/logs.previous"),
        Item::file(b"box/logs.previous/kept.log", b"kept\n").mode(0o600),
    ];
    let mut h = harness(Echo::None, &items, false, &limits::small());
    h.run_until_idle();
    assert_eq!(read(&mut h, b"box/logs.previous/kept.log").0.as_ref(), b"kept\n");
    stop(&mut h);
}

#[test]
fn directory_links_are_refused_without_touching_their_targets() {
    let layouts: [Box<[Item]>; 3] = [
        Box::new([Item::directory(b"elsewhere"), Item::link(b"box", b"elsewhere")]),
        Box::new([Item::directory(b"box"), Item::directory(b"elsewhere"), Item::link(b"box/logs", b"../elsewhere")]),
        Box::new([
            Item::directory(b"box"),
            Item::directory(b"box/logs"),
            Item::directory(b"elsewhere"),
            Item::link(b"box/logs.previous", b"../elsewhere"),
        ]),
    ];
    for items in layouts {
        let mut h = harness(Echo::None, &items, false, &limits::small());
        h.run_until_idle();
        let output = h.stderr();
        assert_eq!(bytes::count(&output, b"box: cannot write /data/box/logs/:", 2), 1);
        stop(&mut h);
        let root = h.machine.reopen_root(0);
        let fd = h.machine.open(root, b"elsewhere", How::DirectoryNoFollow).unwrap();
        assert!(h.machine.list(fd, 32, 8192).unwrap().is_empty());
        h.machine.close(fd);
        h.machine.close(root);
        assert_eq!(h.machine.open_handles(), 0);
    }
}

#[test]
fn a_step_name_that_is_not_a_plain_file_name_falls_back_to_echo() {
    let mut h = Harness::configured_echo(
        Steps { always: Box::new([long(b"bad/name", &[b"never"])]), asked: Box::new([]) },
        &[],
        true,
        16_384,
        &limits::small(),
        Echo::None,
    );
    h.run_until_idle();
    h.write_child(h.child_index(b"bad/name"), 2, b"diagnostic\n");
    h.run_until_idle();
    let output = h.stderr();
    assert_eq!(bytes::count(&output, b"box: cannot write", 2), 1);
    assert!(Harness::contains(
        &output,
        b"/data/box/logs/bad/name.log: InvalidArgument; the step's lines follow here\n"
    ));
    assert!(Harness::contains(&output, b"bad/name: diagnostic\n"));
    stop(&mut h);
}

#[test]
fn a_group_failure_carries_its_final_lines_as_one_block_only_when_quiet() {
    use super::fixtures::{grouped_steps, once};
    for echo in [Echo::None, Echo::Steps] {
        let mut steps = grouped_steps();
        steps.asked[0].steps[0].1 = once(b"prepare 3", &[b"never"]);
        let mut h = Harness::configured_echo(steps, &[], true, 16_384, &limits::small(), echo);
        h.run_until_idle();
        let serve = h.hosted.get(h.serve_index()).unwrap().pid;
        h.ask(b"start 3\n");
        h.run_until_idle();
        let child = h.child_index(b"prepare 3");
        h.write_child(child, 2, b"one\ntwo\nthree\n");
        let pid = h.hosted.get(child).unwrap().pid;
        h.sim.finish_service(pid, Exit::Code(1));
        h.run_until_idle();
        let output = h.stderr();
        if echo == Echo::None {
            assert!(Harness::contains(&output, b"Forge 3 (Forgejo): failed, prepare 3 exited 1\n    prepare 3: one\n    prepare 3: two\n    prepare 3: three\n    Log: /data/box/logs/prepare-3.log\n"));
        } else {
            assert!(Harness::contains(&output, b"prepare 3: one\nprepare 3: two\nprepare 3: three\n"));
            assert!(!Harness::contains(&output, b"    prepare 3:"));
            assert!(!Harness::contains(&output, b"    Log:"));
        }
        assert!(h.sim.service_running(serve), "a failed group leaves serve running");
        stop(&mut h);
    }
}

#[test]
fn restart_tails_are_bounded_include_the_eof_piece_and_reset_before_spawn() {
    use skein_lib::Duration;
    let limits = limits::Limits { tail_lines: 2, ..limits::small() };
    let mut h = harness(Echo::None, &[], true, &limits);
    h.run_until_idle();
    let child = h.child_index(b"forge 1");
    h.write_child(child, 2, b"one\ntwo\nthree\nlast");
    let pid = h.hosted.get(child).unwrap().pid;
    h.sim.finish_service(pid, Exit::Code(1));
    h.run_until_idle();
    let output = h.stderr();
    assert!(Harness::contains(&output, b"forge 1 exited 1; restarting in 0.5 s\n    forge 1: three\n    forge 1: last\n    Log: /data/box/logs/forge-1.log\n"));
    assert!(!Harness::contains(&output, b"    forge 1: two\n"));
    h.sim.advance_to(h.sim.now().saturating_add(Duration::from_millis(500)));
    h.run_until_idle();
    let child = h.child_index(b"forge 1");
    h.write_child(child, 1, b"fresh\n");
    let pid = h.hosted.get(child).unwrap().pid;
    h.sim.finish_service(pid, Exit::Code(1));
    h.run_until_idle();
    let output = h.stderr();
    assert!(Harness::contains(
        &output,
        b"forge 1 exited 1; restarting in 1 s\n    forge 1: fresh\n    Log: /data/box/logs/forge-1.log\n"
    ));
    assert!(!Harness::contains(&output, b"    forge 1: last\n"));
    stop(&mut h);
}

#[test]
fn a_waiting_give_up_drains_its_log_without_holding_teardown_orders_or_spinning() {
    use skein_shell::Host;
    let limits = limits::Limits { restarts: 0, ..limits::small() };
    let mut h = harness(Echo::None, &[], true, &limits);
    h.run_until_idle();
    h.hold_log_write = true;
    let child = h.child_index(b"forge 1");
    h.write_child(child, 2, b"held final line\n");
    h.run_until_idle();
    assert!(h.held_log_write.is_some());
    let pid = h.hosted.get(child).unwrap().pid;
    h.sim.finish_service(pid, Exit::Code(1));
    h.run_until_idle();
    assert!(!h.service.work_pending(h.sim.now()), "waiting on the logger is not runnable");
    assert!(!Harness::contains(&h.stderr(), b"giving up"));
    assert!(
        !h.sim.service_running(h.hosted.get(h.serve_index()).unwrap().pid) || h.service.next_deadline().is_some(),
        "give-up's teardown progresses while its block waits"
    );
    h.release_log_write();
    assert_eq!(h.run_to_end(), Exit::Code(1));
    let output = h.stderr();
    assert!(Harness::contains(
        &output,
        b"forge 1 exited 1; giving up\n    forge 1: held final line\n    Log: /data/box/logs/forge-1.log\n"
    ));
    h.sim.assert_quiescent(h.pid);
    h.sim.assert_no_open_fds(h.pid);
    assert_eq!(h.machine.open_handles(), 0);
}

#[test]
fn a_failed_log_or_an_empty_run_has_no_failure_tail() {
    for refused in [true, false] {
        let mut h = harness(Echo::None, &[], true, &limits::small());
        if refused {
            h.refuse_log = Some(bytes::copy_of(b"forge-1.log"));
        }
        h.run_until_idle();
        let child = h.child_index(b"forge 1");
        if refused {
            h.write_child(child, 2, b"visible\n");
            h.run_until_idle();
        }
        let pid = h.hosted.get(child).unwrap().pid;
        h.sim.finish_service(pid, Exit::Code(1));
        h.run_until_idle();
        let output = h.stderr();
        assert!(Harness::contains(&output, b"forge 1 exited 1; restarting"));
        assert!(!Harness::contains(&output, b"    Log:"));
        stop(&mut h);
    }
}

#[test]
fn another_logs_refusal_waits_behind_the_failure_without_blocking_its_tail() {
    let mut h = harness(Echo::None, &[], true, &limits::small());
    h.run_until_idle();
    let forge = h.child_index(b"forge 1");
    h.write_child(forge, 2, b"final\n");
    h.run_until_idle();
    h.hold_terminal_ack = true;
    h.write_child(h.serve_index(), 1, b"http://first\n");
    h.run_until_idle();
    h.write_child(h.serve_index(), 1, b"http://fresh\n");
    h.run_until_idle();
    h.hold_log_write = true;
    h.write_child(h.serve_index(), 2, b"fallback\n");
    h.run_until_idle();
    assert!(h.held_log_write.is_some());
    let pid = h.hosted.get(forge).unwrap().pid;
    h.sim.finish_service(pid, Exit::Code(1));
    h.run_until_idle();
    h.fail_log_write();
    h.run_until_idle();
    h.release_terminal_ack();
    h.run_until_idle();
    let output = h.stderr();
    let block = b"forge 1 exited 1; restarting in 0.5 s\n    forge 1: final\n    Log: /data/box/logs/forge-1.log\n";
    let block_at = bytes::find(&output, block).expect("whole failure block");
    let refusal_at =
        bytes::find(&output, b"box: cannot write /data/box/logs/serve.log: NoSpace;").expect("one refusal");
    assert!(block_at < refusal_at, "refusal follows the waiting block");
    assert!(Harness::contains(&output, b"serve: fallback\n"));
    stop(&mut h);
}
