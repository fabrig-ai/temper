use crate::terminal::{Fact, Formatter, Item, Target};
use skein_lib::{Wall, bytes};
fn wall(seconds: u64) -> Wall {
    Wall::from_nanos(seconds.checked_mul(1_000_000_000).unwrap())
}
fn item(target: Target, words: &[u8]) -> Item {
    Item { target, bytes: bytes::copy_of(words) }
}
#[test]
fn the_first_block_has_exact_words_and_three_targets() {
    let formatter = Formatter::new();
    assert_eq!(formatter.block(bytes::copy_of(b"http://link\n"), wall(50)).as_slice(), &[
        item(Target::Stderr, b"\nOpen this link in a browser on this machine. It signs you in as this\ntemper's owner, once and for a short while, so keep it private.\nOpened again once spent, it prints a fresh one here.\n\n"),
        item(Target::Stdout, b"http://link\n"),
        item(Target::Stderr, b"\nCtrl-C stops temper.\n\n"),
    ]);
}
#[test]
fn fresh_blocks_keep_state_across_restart_and_use_supplied_utc() {
    let mut formatter = Formatter::new();
    formatter.admitted_block();
    for (seconds, expected) in [
        (86_399, b"23:59:59Z A fresh sign-in link:\n".as_slice()),
        (86_400, b"00:00:00Z A fresh sign-in link:\n"),
        (60, b"00:01:00Z A fresh sign-in link:\n"),
        (172_861, b"00:01:01Z A fresh sign-in link:\n"),
    ] {
        assert_eq!(
            formatter.block(bytes::copy_of(b"http://fresh\n"), wall(seconds)).as_slice(),
            &[item(Target::Stderr, expected), item(Target::Stdout, b"http://fresh\n"),]
        );
    }
}
#[test]
fn lifecycle_facts_are_untimed_until_a_complete_block_is_admitted() {
    let steps = crate::layout::flatten(temper_forge_setup::Steps {
        always: Box::new([super::fixtures::step(b"serve", temper_forge_setup::Kind::Long)]),
        asked: Box::new([]),
    });
    let mut formatter = Formatter::new();
    for (fact, words, timed) in [
        (Fact::Starting { step: 0 }, b"Starting serve\n".as_slice(), b"01:02:03Z Starting serve\n".as_slice()),
        (Fact::Stopping, b"Stopping\n", b"01:02:03Z Stopping\n"),
        (Fact::Stopped, b"Stopped\n", b"01:02:03Z Stopped\n"),
    ] {
        assert_eq!(formatter.fact(fact, &steps, wall(3_723)), item(Target::Stderr, words));
        let mut after = Formatter::new();
        after.admitted_block();
        assert_eq!(after.fact(fact, &steps, wall(3_723)), item(Target::Stderr, timed));
    }
    let refused = formatter.notice(crate::terminal::REFUSAL, wall(3_723));
    assert_eq!(refused, item(Target::Stderr, b"box: serve: sign-in link too long\n"));
    // Neither formatting a refusal nor constructing a block marks admission.
    drop(formatter.block(bytes::copy_of(b"held\n"), wall(3_723)));
    assert_eq!(formatter.block_entries(), 3);
    formatter.admitted_block();
    assert_eq!(formatter.notice(b"box: failure\n", wall(3_723)), item(Target::Stderr, b"01:02:03Z box: failure\n"));
}

fn labelled(label: &[u8], name: &[u8]) -> Box<[crate::layout::Placed]> {
    use temper_forge_setup::{Group, Kind, Stage, Steps};
    crate::layout::flatten(Steps {
        always: Box::new([]),
        asked: Box::new([Group {
            forge: 37,
            before: 0,
            recorded: false,
            label: bytes::copy_of(label),
            steps: Box::new([(Stage::Run, super::fixtures::step(name, Kind::Long))]),
        }]),
    })
}
#[test]
fn group_and_failure_facts_use_supplied_labels_and_exact_words() {
    use crate::machine::Why;
    use skein_io::kernel::Exit;
    use skein_lib::Duration;
    let steps = labelled(b"\xffother\0label", b"a step");
    let mut formatter = Formatter::new();
    formatter.admitted_block();
    for (fact, expected) in [
        (Fact::GroupStarting { forge: 37 }, b"\xffother\0label: starting\n".as_slice()),
        (Fact::GroupStarted { forge: 37 }, b"\xffother\0label: started\n"),
        (Fact::GroupStopped { forge: 37 }, b"\xffother\0label: stopped\n"),
        (
            Fact::GroupFailed { forge: 37, why: Why::Exited { step: 0, exit: Exit::Code(7) } },
            b"\xffother\0label: failed, a step exited 7\n",
        ),
        (
            Fact::GroupFailed { forge: 37, why: Why::NotStarted { step: 0 } },
            b"\xffother\0label: failed, a step did not start\n",
        ),
        (
            Fact::GroupFailed { forge: 37, why: Why::NotReady { step: 0 } },
            b"\xffother\0label: failed, a step version unavailable\n",
        ),
        (Fact::GroupFailed { forge: 37, why: Why::NotWritten }, b"\xffother\0label: failed, record not written\n"),
        (
            Fact::Restarting {
                step: 0,
                why: Why::Exited { step: 0, exit: Exit::Code(0) },
                delay: Duration::from_millis(500),
            },
            b"a step exited 0; restarting in 0.5 s\n",
        ),
        (
            Fact::Restarting { step: 0, why: Why::NotStarted { step: 0 }, delay: Duration::from_millis(1000) },
            b"a step did not start; restarting in 1 s\n",
        ),
        (
            Fact::Restarting { step: 0, why: Why::Busy, delay: Duration::from_nanos(1_000_000_001) },
            b"busy; restarting in 1.000000001 s\n",
        ),
        (
            Fact::GivingUp { step: 0, why: Why::Exited { step: 0, exit: Exit::Signal(15) } },
            b"a step killed by signal 15; giving up\n",
        ),
    ] {
        let expected = crate::lines::joined(&[b"23:59:59Z ", expected]);
        assert_eq!(formatter.fact(fact, &steps, wall(86_399)), item(Target::Stderr, &expected));
    }
    for (why, expected) in
        [(Why::Busy, b"busy".as_slice()), (Why::NotAsked, b"not asked"), (Why::Stopping, b"stopping")]
    {
        assert_eq!(crate::terminal::why(why, &steps).as_ref(), expected);
    }
}
#[test]
fn lifecycle_output_bound_covers_long_labels_and_signal_numbers() {
    use crate::{machine::Why, terminal::max_bytes};
    use skein_io::kernel::Exit;
    use skein_lib::Duration;
    let label: Box<[u8]> = Box::new([b'L'; 8192]);
    let name: Box<[u8]> = Box::new([b'N'; 4096]);
    let steps = labelled(&label, &name);
    let why = Why::Exited { step: 0, exit: Exit::Signal(u32::MAX) };
    let mut formatter = Formatter::new();
    formatter.admitted_block();
    for fact in [
        Fact::GroupFailed { forge: 37, why },
        Fact::Restarting { step: 0, why, delay: Duration::from_nanos(u64::MAX) },
        Fact::GivingUp { step: 0, why },
    ] {
        let item = formatter.fact(fact, &steps, wall(86_399));
        assert!(item.bytes.len() <= usize::try_from(max_bytes(&steps)).unwrap());
        assert!(item.bytes.ends_with(b"\n"));
        assert!(super::fixtures::Harness::contains(&item.bytes, b"killed by signal 4294967295"));
    }
    assert!(max_bytes(&[]) >= crate::terminal::FIXED_BYTES);
}

#[test]
fn the_largest_tail_fits_one_output_item_and_reset_releases_every_line() {
    use crate::limits::MAX_TAIL_LINES;
    use crate::terminal::{Tail, failure_bytes};
    let steps = labelled(b"Forge with a long label", b"last step");
    let name = steps.first().unwrap().step.name();
    let piece = super::fixtures::line_cap();
    let line = crate::lines::joined(&[name, b": ", &piece, b"\n"]);
    let mut tail = Tail::new(MAX_TAIL_LINES);
    for _ in 0..MAX_TAIL_LINES {
        tail.push(line.clone());
    }
    let fact = Fact::GroupFailed {
        forge: 37,
        why: crate::machine::Why::Exited { step: 0, exit: skein_io::kernel::Exit::Code(1) },
    };
    let formatter = Formatter::new();
    let mut block = formatter.fact(fact, &steps, Wall::EPOCH);
    tail.append(&mut block, b"/data/box/logs/last-step.log");
    assert!(block.bytes.len() > 409_600);
    assert!(block.bytes.len() <= usize::try_from(failure_bytes(&steps, b"/data")).unwrap());
    tail.reset();
    let mut empty = formatter.fact(fact, &steps, Wall::EPOCH);
    tail.append(&mut empty, b"/data/box/logs/last-step.log");
    assert_eq!(empty, formatter.fact(fact, &steps, Wall::EPOCH));
}
