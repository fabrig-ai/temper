use crate::tests::fixtures as f;
use jig_client::{Watchable, contract as c};
use skein_lib::Time;
#[test]
fn a_wait_until_replied_answers_at_the_parked_delivery_with_its_digest() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_REPLIED);
    let parked = f::parked();
    let expected = f::subject_json(&crate::jig::render::Part::delivery(&parked).expect("task form"));
    root.deliver_subject(&f::task_watch(), parked);
    let answer = root.answer(call).expect("met at the parked delivery");
    assert_eq!(answer.status, 0);
    assert!(f::json_has(&answer, &expected));
    assert!(answer.stdout.ends_with(b",\"met\":true}"));
}
#[test]
fn a_read_matches_a_wait_digest() {
    let mut root = f::signed_in(f::PARTY);
    for row in f::wait_cases(&mut root) {
        let digest = row.read_digest;
        let call = root.wait_since(row.argv, &[], &digest);
        root.deliver_subject(&row.watch, row.delivery.clone());
        assert!(root.answer(call).is_none(), "unchanged {:?}", row.argv);
        let changed = f::changed_delivery(row.delivery);
        root.deliver_subject(&row.watch, changed);
        let answer = root.answer(call).expect("changed subject answers");
        assert_eq!(answer.status, 0, "{:?}", row.argv);
        assert!(!f::json_has(&answer, &digest), "{:?}", row.argv);
    }
}
#[test]
fn since_compares_the_first_delivery() {
    let mut root = f::signed_in(f::PARTY);
    let parked = f::parked();
    let different = f::task_digest(&f::writing_chat()).hex();
    let call = root.wait_since(f::WAIT_TASK, &[], &different);
    root.deliver_subject(&f::task_watch(), parked);
    assert_eq!(root.answer(call).expect("first differs").status, 0);
}
#[test]
fn until_changed_answers_the_first_change() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, &[]);
    root.deliver_subject(&f::task_watch(), f::parked());
    assert!(root.answer(call).is_none());
    root.deliver_subject(&f::task_watch(), f::writing_chat());
    assert_eq!(root.answer(call).expect("first change").status, 0);
}
#[test]
fn both_requirements_must_hold_on_the_same_delivery() {
    let parked = f::parked();
    let mut root = f::signed_in(f::PARTY);
    let call = root.wait_since(f::WAIT_TASK, f::UNTIL_REPLIED, &f::task_digest(&parked).hex());
    for delivery in [parked.clone(), f::writing_chat(), parked.clone()] {
        root.deliver_subject(&f::task_watch(), delivery);
        assert!(root.answer(call).is_none());
    }
    root.deliver_subject(&f::task_watch(), f::changed_delivery(parked));
    assert_eq!(root.answer(call).expect("both hold now").status, 0);
}
#[test]
fn timeout_exits_3_with_the_last_subject() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::SHORT_TIMEOUT);
    let parked = f::parked();
    let expected = f::subject_json(&crate::jig::render::Part::delivery(&parked).expect("task form"));
    root.deliver_subject(&f::task_watch(), parked);
    root.now = Time::ZERO.checked_add(f::WAIT_SPAN).expect("deadline");
    root.fire();
    let answer = root.answer(call).expect("timed out");
    assert_eq!(answer.status, 3);
    assert!(f::json_has(&answer, &expected));
    assert!(f::json_has(&answer, b"\"why\":\"timeout\",\"met\":false"));
}
#[test]
fn a_timeout_does_not_fire_early() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::SHORT_TIMEOUT);
    assert_eq!(crate::root::next_deadline(&root.root), Time::ZERO.checked_add(f::WAIT_SPAN));
    root.fire();
    assert!(root.answer(call).is_none());
}
#[test]
fn a_me_change_rechecks_the_task_without_changing_its_digest() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_WAITING_ON_ME);
    let task = f::goal(
        f::TASK,
        f::party_wait(c::Whom::Role { project: f::PROJECT, role: f::POLICY_ROLE }, c::WaitedFor::Question),
    );
    let delivery = c::Delivery::Task(Box::new(task));
    let digest = f::task_digest(&delivery).hex();
    root.deliver_subject(&f::task_watch(), delivery);
    assert!(root.answer(call).is_none());
    let mut me = f::me(f::PARTY);
    me.projects.get_mut(0).expect("project").role = c::HeldRole::Policy { role: f::POLICY_ROLE };
    root.deliver(c::Delivery::Me(Box::new(me)));
    let answer = root.answer(call).expect("role now holds");
    assert_eq!(answer.status, 0);
    assert!(f::json_has(&answer, &digest));
}
#[test]
fn a_meeting_delivery_wins_over_timeout_in_the_same_iteration() {
    let flags = f::extended(f::SHORT_TIMEOUT, f::UNTIL_REPLIED);
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, flags.as_slice());
    root.now = Time::ZERO.checked_add(f::WAIT_SPAN).expect("deadline");
    root.deliver_subject(&f::task_watch(), f::parked());
    root.fire();
    assert_eq!(root.answer(call).expect("delivery won").status, 0);
}
#[test]
fn a_wait_keeps_its_baseline_while_behind() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, &[]);
    root.deliver_subject(&f::task_watch(), f::parked());
    root.watch_event(&f::task_watch(), c::WatchEvent::Behind);
    root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
    assert!(root.answer(call).is_none());
    root.deliver_subject(&f::task_watch(), f::writing_chat());
    assert_eq!(root.answer(call).expect("new live delivery").status, 0);
}
#[test]
fn coalesced_deliveries_answer_from_the_current_document() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_REPLIED);
    root.deliver_subject(&f::task_watch(), f::writing_chat());
    assert!(root.answer(call).is_none());
    let changed = f::changed_delivery(f::parked());
    let expected = f::subject_json(&crate::jig::render::Part::delivery(&changed).expect("task"));
    root.deliver_subject(&f::task_watch(), changed);
    assert!(f::json_has(&root.answer(call).expect("current parked document"), &expected));
}
#[test]
fn run_text_changes_are_deliveries() {
    let (mut root, call) = f::waiting_call(f::WAIT_RUN, &[]);
    root.deliver_run(f::no_provisional());
    root.deliver_run(f::provisional());
    let answer = root.answer(call).expect("text changed");
    assert_eq!(answer.status, 0);
    assert!(f::json_has(&answer, b"draft"));
    assert!(root.answer(call).is_none());
}
#[test]
fn an_answer_releases_only_the_calls_handle() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_REPLIED);
    let flags = f::extended(f::WAIT_TASK, &["--until", "phase:running", "--json"]);
    let other = root.submit(flags.as_slice());
    root.deliver_subject(&f::task_watch(), f::parked());
    assert_eq!(root.answer(call).expect("first call met").status, 0);
    assert!(root.answer(other).is_none());
    assert_eq!(root.watches.len(), f::TWO_CALLERS);
    root.deliver_subject(&f::task_watch(), f::writing_chat());
    assert_eq!(root.answer(other).expect("second call met").status, 0);
    assert_eq!(root.watches.len(), f::SHARED_WATCHES);
    assert_eq!(crate::root::next_deadline(&root.root), None);
    for subject in root.watches.values() {
        assert_eq!(*subject, Watchable::Jig(c::Subject::Me));
    }
}

#[test]
fn a_wait_text_uses_the_subjects_read_form() {
    let expected = f::read_answer(f::TASK_CALL, f::task_result(f::parked_task()));
    let (mut root, call) = f::plain_wait(f::UNTIL_REPLIED);
    root.deliver_subject(&f::task_watch(), f::parked());
    assert_eq!(root.answer(call).expect("text answer").stdout, f::appended(&expected.stdout, b"met\n"));
}
#[test]
fn a_timeout_text_names_why_it_was_not_met() {
    let expected = f::read_answer(f::TASK_CALL, f::task_result(f::parked_task()));
    let (mut root, call) = f::plain_wait(f::SHORT_TIMEOUT);
    root.deliver_subject(&f::task_watch(), f::parked());
    root.now = Time::ZERO.checked_add(f::WAIT_SPAN).expect("deadline");
    root.fire();
    assert_eq!(root.answer(call).expect("timeout text").stdout, f::appended(&expected.stdout, b"not met: timeout\n"));
}

#[test]
fn a_task_watch_ending_reads_how_the_task_ended_before_answering() {
    let mut root = f::signed_in(f::PARTY);
    for phase in f::ENDED_PHASES {
        let argv = f::extended(f::WAIT_TASK, f::UNTIL_ENDED);
        let argv = f::extended(
            argv.as_slice(),
            if argv.as_slice().contains(&"--until") || argv.as_slice().contains(&"--since") {
                &["--json"]
            } else {
                &["--until", "changed", "--json"]
            },
        );
        let call = root.submit(argv.as_slice());
        root.deliver_subject(&f::task_watch(), f::writing_chat());
        root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
        assert!(root.answer(call).is_none(), "{phase:?}: final read is pending");
        assert_eq!(root.reads.get(&call).expect("final read").1, c::Read::Task { task: f::TASK });
        let task = f::ended_task(*phase);
        let read = root.submit(f::TASK_JSON_CALL);
        root.serve(read, f::task_result(task.clone()));
        let expected = root.answer(read).expect("ended task read");
        root.serve(call, f::task_result(task));
        let answer = root.answer(call).expect("final task read");
        assert_eq!(answer.status, 0, "{phase:?}");
        assert_eq!(answer.stdout, f::met_json(&expected.stdout), "{phase:?}");
    }
}
// Both a refused opening and an admitted watch's end use the same final read.
// The task read has a live form and a historical document/result form.
const FINAL_TASK_FORMS: usize = 2;
// Any application family is unexpected on the jig task read.
const UNEXPECTED_FAMILY: u16 = 1;
const TASK_ENDS: &[c::WatchEnd] = &[c::WatchEnd::SubjectEnded, c::WatchEnd::Unknown];

fn final_task_reads(task: c::Task, ending: c::Ending) -> [c::ReadResult; FINAL_TASK_FORMS] {
    [f::task_result(task.clone()), f::historical_task_result(task, ending)]
}

fn final_read(end: c::WatchEnd, json: bool, until: &[&str]) -> (f::Harness, u32, skein_lib::Token, skein_lib::Token) {
    let flags = f::extended(until, f::SHORT_TIMEOUT);
    let (mut root, call) =
        if json { f::waiting_call(f::WAIT_TASK, flags.as_slice()) } else { f::plain_wait(flags.as_slice()) };
    let watch = root.end(call, &f::task_watch(), end);
    assert!(root.answer(call).is_none(), "{end:?}: final read pending without any delivery");
    let (read, what) = root.reads.get(&call).expect("final read");
    assert_eq!(*what, c::Read::Task { task: f::TASK });
    let read = *read;
    assert_eq!(crate::root::next_deadline(&root.root), None);
    assert_eq!(root.watches.len(), f::SHARED_WATCHES);
    (root, call, watch, read)
}

fn settled_final_read(root: &mut f::Harness, call: u32, watch: skein_lib::Token, read: skein_lib::Token) {
    assert!(root.reads.is_empty());
    assert_eq!(root.watches.len(), f::SHARED_WATCHES);
    let handle = jig_client::Handle { asker: crate::root::RUNNER.asker(), number: u64::from(call) << 32 };
    assert!(root.root.client().document(handle).is_none(), "call's watch handle released");
    assert_eq!(crate::root::next_deadline(&root.root), None);
    root.client(jig_client::Event::Up { up: jig_client::Up::ReadDone { read, result: jig_client::Readout::Limit } });
    root.client(jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: c::WatchEnd::SubjectEnded } });
    root.now = Time::ZERO.checked_add(f::WAIT_SPAN).expect("old wait deadline");
    root.fire();
    root.client(jig_client::Event::Up { up: jig_client::Up::SignedOut });
    assert!(root.answer(call).is_none(), "no second answer after late terminals, timer or shared sign-out");
    assert_eq!(crate::root::next_deadline(&root.root), None);
}

#[test]
fn a_replied_wait_meets_the_task_ending_before_delivery() {
    for result in final_task_reads(f::ended_task(c::Phase::Done), c::Ending::Done) {
        let expected = f::read_answer(f::TASK_JSON_CALL, result.clone());
        let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_REPLIED);
        root.deliver_subject(&f::task_watch(), f::writing_chat());
        assert!(root.answer(call).is_none(), "running has not replied");
        let watch = root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
        assert!(root.answer(call).is_none(), "terminal read pending");
        let (read, what) = root.reads.get(&call).expect("final read");
        assert_eq!(*what, c::Read::Task { task: f::TASK });
        let read = *read;
        root.serve(call, result);
        let answer = root.answer(call).expect("terminal task has replied");
        assert_eq!(answer.status, 0);
        assert_eq!(answer.stdout, f::met_json(&expected.stdout));
        settled_final_read(&mut root, call, watch, read);
    }
}

#[test]
fn a_replied_wait_meets_an_initially_unknown_terminal_task() {
    for result in final_task_reads(f::ended_task(c::Phase::Done), c::Ending::Done) {
        let expected = f::read_answer(f::TASK_JSON_CALL, result.clone());
        let (mut root, call, watch, read) = final_read(c::WatchEnd::Unknown, true, f::UNTIL_REPLIED);
        root.serve(call, result);
        let answer = root.answer(call).expect("already ended task has replied");
        assert_eq!(answer.status, 0);
        assert_eq!(answer.stdout, f::met_json(&expected.stdout));
        settled_final_read(&mut root, call, watch, read);
    }
}

#[test]
fn a_task_ending_before_delivery_reads_its_terminal_result() {
    for end in TASK_ENDS {
        for (phase, ending) in [
            (c::Phase::Ended, c::Ending::Deleted),
            (c::Phase::Done, c::Ending::Done),
            (c::Phase::Failed, c::Ending::Failed { reason: None }),
            (c::Phase::Cancelled, c::Ending::Cancelled { by: c::Actor::Task(f::OTHER_TASK), reason: None, at: None }),
        ] {
            for result in final_task_reads(f::ended_task(phase), ending) {
                let expected = f::read_answer(f::TASK_JSON_CALL, result.clone());
                let (mut root, call, watch, read) = final_read(*end, true, f::UNTIL_ENDED);
                root.serve(call, result);
                let answer = root.answer(call).expect("matching terminal read");
                assert_eq!(answer.status, 0, "{end:?}, {phase:?}");
                assert_eq!(answer.stdout, f::met_json(&expected.stdout), "{end:?}, {phase:?}");
                settled_final_read(&mut root, call, watch, read);
            }
        }
    }
}

#[test]
fn a_final_task_read_must_match_the_number_and_terminal_phase() {
    let phases = f::LIVE_PHASES.len().checked_add(f::ENDED_PHASES.len()).expect("fixture phase count");
    let mut tasks = skein_lib::List::with_capacity(u32::try_from(phases).expect("fixture phases fit"));
    for phase in f::LIVE_PHASES {
        tasks.push(f::ended_task(*phase)).expect("one row per live phase");
    }
    for phase in f::ENDED_PHASES {
        let mut wrong = f::ended_task(*phase);
        wrong.chip.task = f::OTHER_TASK;
        tasks.push(wrong).expect("one wrong task per terminal phase");
    }
    for (end, until) in [
        (c::WatchEnd::SubjectEnded, f::UNTIL_ENDED),
        (c::WatchEnd::Unknown, f::UNTIL_ENDED),
        (c::WatchEnd::SubjectEnded, f::UNTIL_REPLIED),
        (c::WatchEnd::Unknown, f::UNTIL_REPLIED),
    ] {
        for task in &tasks {
            // Historical keys are self-consistent even for the wrong task number.
            for result in final_task_reads(task.clone(), c::Ending::Done) {
                let (mut root, call, watch, read) = final_read(end, true, until);
                root.serve(call, result);
                let answer = root.answer(call).expect("invalid final task fails visibly");
                assert_eq!(answer.status, 1, "{end:?}: {task:?}");
                assert!(answer.stdout.is_empty());
                assert!(!answer.stderr.is_empty());
                settled_final_read(&mut root, call, watch, read);
            }
        }
    }
}

#[test]
fn a_final_task_read_rejects_an_unexpected_successful_document() {
    for end in TASK_ENDS {
        for result in [
            jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Me(Box::new(f::me(f::PARTY))))),
            jig_client::Readout::Family {
                family: UNEXPECTED_FAMILY,
                record: Some(Box::from(b"unexpected".as_slice())),
            },
        ] {
            let (mut root, call, watch, read) = final_read(*end, true, f::UNTIL_ENDED);
            root.serve_readout(call, result);
            let answer = root.answer(call).expect("unexpected document fails visibly");
            assert_eq!(answer.status, 1);
            assert!(answer.stdout.is_empty());
            assert!(!answer.stderr.is_empty());
            settled_final_read(&mut root, call, watch, read);
        }
    }
}
#[test]
fn an_already_ended_delivery_reads_the_final_task() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_ENDED);
    let task = f::ended_task(c::Phase::Done);
    root.final_delivery(call, task.clone());
    assert!(root.answer(call).is_none());
    assert_eq!(crate::root::next_deadline(&root.root), None);
    root.serve(call, f::task_result(task));
    assert_eq!(root.answer(call).expect("final read").status, 0);
}
#[test]
fn a_task_ended_text_uses_the_final_reads_part() {
    for end in TASK_ENDS {
        for result in final_task_reads(f::ended_task(c::Phase::Done), c::Ending::Done) {
            let expected = f::read_answer(f::TASK_CALL, result.clone());
            let (mut root, call, watch, read) = final_read(*end, false, f::UNTIL_ENDED);
            root.serve(call, result);
            let answer = root.answer(call).expect("final text");
            assert_eq!(answer.status, 0);
            assert_eq!(answer.stdout, f::appended(&expected.stdout, b"met\n"));
            settled_final_read(&mut root, call, watch, read);
        }
    }
}
#[test]
fn a_run_or_tree_ending_meets_ended() {
    let mut root = f::signed_in(f::PARTY);
    for (argv, watch, delivery) in [
        (f::WAIT_RUN, f::run_watch(), f::no_provisional()),
        (f::WAIT_TREE, f::tree_watch(), c::Delivery::Tree(Box::new(f::composite_tree()))),
    ] {
        for initial in [None, Some(delivery)] {
            let call = root.wait_since(argv, f::UNTIL_ENDED, &f::task_digest(&f::parked()).hex());
            if let Some(delivery) = initial {
                root.deliver_subject(&watch, delivery);
            }
            assert!(root.answer(call).is_none());
            root.end(call, &watch, c::WatchEnd::SubjectEnded);
            let answer = root.answer(call).expect("ended meets despite since");
            assert_eq!(answer.status, 0, "{argv:?}");
            assert!(answer.stdout.ends_with(b",\"met\":true}"));
            assert!(root.reads.is_empty());
            assert_eq!(root.watches.len(), f::SHARED_WATCHES);
        }
    }
}
#[test]
fn a_subject_ending_unmet_keeps_its_last_digest() {
    let mut root = f::signed_in(f::PARTY);
    for flags in [&[][..], f::UNTIL_RUNNING, &["--until", "held"][..]] {
        let argv = f::extended(f::WAIT_TASK, flags);
        let argv = f::extended(
            argv.as_slice(),
            if argv.as_slice().contains(&"--until") || argv.as_slice().contains(&"--since") {
                &["--json"]
            } else {
                &["--until", "changed", "--json"]
            },
        );
        let call = root.submit(argv.as_slice());
        let parked = f::parked();
        let expected = f::subject_json(&crate::jig::render::Part::delivery(&parked).expect("task"));
        root.deliver_subject(&f::task_watch(), parked);
        root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
        let answer = root.answer(call).expect("ended unmet");
        assert_eq!(answer.status, 3);
        assert!(f::json_has(&answer, &expected));
        assert!(answer.stdout.ends_with(b",\"why\":\"ended\",\"met\":false}"));
    }
}
#[test]
fn a_held_wait_ending_before_delivery_invents_no_document() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, &["--until", "held"]);
    root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
    let answer = root.answer(call).expect("ended without delivery");
    assert_eq!(answer.status, 3);
    assert_eq!(answer.stdout.as_ref(), b"{\"task\":null,\"why\":\"ended\",\"met\":false}");
}
#[test]
fn a_subject_no_longer_visible_exits_2() {
    let mut root = f::signed_in(f::PARTY);
    for (end, reason) in [(c::WatchEnd::Unknown, b"unknown".as_slice()), (c::WatchEnd::NotAllowed, b"not_allowed")] {
        let argv = f::extended(f::WAIT_TASK, &["--until", "changed", "--json"]);
        let call = root.submit(argv.as_slice());
        root.end(call, &f::task_watch(), end);
        let answer = root.answer(call).expect("refused");
        assert_eq!(answer.status, 2);
        assert!(skein_lib::bytes::find(&answer.stderr, reason).is_some());
        assert_eq!(crate::root::next_deadline(&root.root), None);
    }
}
#[test]
fn client_watch_failures_exit_1() {
    let mut root = f::signed_in(f::PARTY);
    for end in [c::WatchEnd::SignedOut, c::WatchEnd::NotServed] {
        let argv = f::extended(f::WAIT_TASK, &["--until", "changed", "--json"]);
        let call = root.submit(argv.as_slice());
        root.end(call, &f::task_watch(), end);
        assert_eq!(root.answer(call).expect("client failure").status, 1);
    }
}
#[test]
fn final_read_failures_keep_the_reads_status() {
    for end in TASK_ENDS {
        for (result, status) in [
            (jig_client::Readout::Limit, 1),
            (jig_client::Readout::NotServed { family: None, served: None }, 1),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Busy)), 1),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::SignedOut)), 1),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Unreachable)), 1),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Unknown)), 2),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Refused)), 2),
        ] {
            let (mut root, call, watch, read) = final_read(*end, true, f::UNTIL_ENDED);
            root.serve_readout(call, result);
            let answer = root.answer(call).expect("final read failed");
            assert_eq!(answer.status, status);
            assert!(answer.stdout.is_empty());
            assert!(!answer.stderr.is_empty());
            settled_final_read(&mut root, call, watch, read);
        }
    }
}
#[test]
fn sign_out_of_required_me_ends_the_wait() {
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, f::UNTIL_WAITING_ON_ME);
    root.end(call, &Watchable::Jig(c::Subject::Me), c::WatchEnd::SignedOut);
    assert_eq!(root.answer(call).expect("me signed out").status, 1);
    assert_eq!(crate::root::next_deadline(&root.root), None);
}
#[test]
fn a_stopping_engine_reopens_before_answering() {
    let mut root = f::signed_in(f::PARTY);
    for end in [c::WatchEnd::Stopping, c::WatchEnd::Dropped, c::WatchEnd::Busy] {
        let argv = f::extended(f::WAIT_TASK, f::UNTIL_REPLIED);
        let call = root.submit(argv.as_slice());
        root.deliver_subject(&f::task_watch(), f::writing_chat());
        root.end(call, &f::task_watch(), end);
        assert!(root.answer(call).is_none());
        root.now = crate::root::next_deadline(&root.root).expect("reopen deadline");
        root.fire();
        root.deliver_subject(&f::task_watch(), f::parked());
        assert_eq!(root.answer(call).expect("reopened parked task").status, 0);
    }
}
#[test]
fn a_finishing_read_releases_the_waits_timer() {
    let flags = f::extended(f::UNTIL_ENDED, f::SHORT_TIMEOUT);
    let (mut root, call) = f::waiting_call(f::WAIT_TASK, flags.as_slice());
    root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
    root.now = Time::ZERO.checked_add(f::WAIT_SPAN).expect("old deadline");
    root.fire();
    assert!(root.answer(call).is_none());
    assert_eq!(crate::root::next_deadline(&root.root), None);
    root.serve(call, f::task_result(f::ended_task(c::Phase::Done)));
    assert_eq!(root.answer(call).expect("read after old timeout").status, 0);
}
#[test]
fn an_answered_wait_ignores_late_terminals() {
    let (mut root, call) = f::finishing_call();
    let token = root.reads.get(&call).expect("final read").0;
    root.serve(call, f::task_result(f::ended_task(c::Phase::Done)));
    assert_eq!(root.answer(call).expect("single answer").status, 0);
    root.client(jig_client::Event::Up {
        up: jig_client::Up::ReadDone { read: token, result: jig_client::Readout::Limit },
    });
    root.fire();
    assert!(root.answer(call).is_none());
    assert_eq!(root.watches.len(), f::SHARED_WATCHES);
    assert_eq!(crate::root::next_deadline(&root.root), None);
}

#[test]
fn an_end_without_delivery_names_the_text_subject() {
    let mut root = f::signed_in(f::PARTY);
    let argv = f::extended(f::WAIT_RUN, f::UNTIL_ENDED);
    let call = root.submit(argv.as_slice());
    root.end(call, &f::run_watch(), c::WatchEnd::SubjectEnded);
    assert_eq!(root.answer(call).expect("ended run text").stdout.as_ref(), b"run ended\nmet\n");
}
#[test]
fn an_unchanged_ended_delivery_still_waits_for_the_terminal() {
    let task = f::ended_task(c::Phase::Done);
    let delivery = c::Delivery::Task(Box::new(task.clone()));
    let mut root = f::signed_in(f::PARTY);
    let call = root.wait_since(f::WAIT_TASK, f::UNTIL_ENDED, &f::task_digest(&delivery).hex());
    root.deliver_subject(&f::task_watch(), delivery);
    assert!(root.answer(call).is_none());
    assert!(root.reads.is_empty());
    root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
    root.serve(call, f::task_result(task));
    assert_eq!(root.answer(call).expect("terminal overrides since").status, 0);
}
#[test]
fn a_client_handle_limit_exits_1() {
    let mut root = f::signed_in_with_handle_limit();
    let call = root.submit(f::extended(f::WAIT_TASK, &["--until", "changed"]).as_slice());
    assert_eq!(root.answer(call).expect("client handle limit").status, 1);
    assert_eq!(crate::root::next_deadline(&root.root), None);
}
#[test]
fn sign_out_during_the_final_read_exits_1() {
    for end in TASK_ENDS {
        let (mut root, call, watch, read) = final_read(*end, true, f::UNTIL_ENDED);
        root.client(jig_client::Event::Up { up: jig_client::Up::SignedOut });
        assert_eq!(root.answer(call).expect("signed out final read").status, 1);
        root.serve(call, f::historical_task_result(f::ended_task(c::Phase::Done), c::Ending::Done));
        settled_final_read(&mut root, call, watch, read);
    }
}

#[test]
fn an_unknown_watch_without_a_terminal_condition_keeps_status_2() {
    for (argv, flags, watch) in [
        (f::WAIT_TASK, &["--until", "held"][..], f::task_watch()),
        (f::WAIT_TASK, f::UNTIL_RUNNING, f::task_watch()),
        (f::WAIT_TASK, &[][..], f::task_watch()),
        (f::WAIT_RUN, f::UNTIL_ENDED, f::run_watch()),
        (f::WAIT_TREE, f::UNTIL_ENDED, f::tree_watch()),
    ] {
        let (mut root, call) = f::waiting_call(argv, flags);
        root.end(call, &watch, c::WatchEnd::Unknown);
        let answer = root.answer(call).expect("unknown subject");
        assert_eq!(answer.status, 2);
        assert!(answer.stdout.is_empty());
        assert!(skein_lib::bytes::find(&answer.stderr, b"unknown").is_some());
        assert!(root.reads.is_empty());
        assert_eq!(root.watches.len(), f::SHARED_WATCHES);
        assert_eq!(crate::root::next_deadline(&root.root), None);
    }
}

#[test]
fn changed_with_since_needs_a_change_after_the_baseline() {
    let mut root = f::signed_in(f::PARTY);
    let first = f::writing_chat();
    let different = f::task_digest(&f::parked()).hex();
    let call = root.wait_since(f::WAIT_TASK, &["--until", "changed"], &different);
    root.deliver_subject(&f::task_watch(), first.clone());
    assert!(root.answer(call).is_none());
    root.deliver_subject(&f::task_watch(), f::parked());
    assert!(root.answer(call).is_none(), "changed but equals since");
    root.deliver_subject(&f::task_watch(), first);
    assert_eq!(root.answer(call).expect("later delivery differs from since").status, 0);
}
#[test]
fn task_tree_reads_the_tree_alone() {
    let jig_client::Latest::Jig(c::Delivery::OwnerTree(mut owner)) = f::owner_tree_at(f::parked_task().chip) else {
        panic!("owner tree")
    };
    owner.folded = skein_lib::List::with_capacity(1);
    owner
        .folded
        .push(c::OwnerTreeFold {
            parent: f::TASK,
            phases: c::OwnerPhaseCounts { waiting: 1, ..c::OwnerPhaseCounts::ZERO },
            held_below: 0,
        })
        .expect("fold");
    for tree in [c::Delivery::Tree(Box::new(f::composite_tree())), c::Delivery::OwnerTree(owner)] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let flags = if json { &["--tree", "--json"][..] } else { &["--tree"][..] };
            let words = f::extended(f::TASK_CALL, flags);
            let compound = root.submit(words.as_slice());
            root.serve(compound, f::task_result(f::goal(f::TASK, f::waiting_on_me())));
            root.deliver_subject(&f::tree_watch(), tree.clone());
            let compound = root.answer(compound).expect("task and tree");
            let words = f::extended(f::WAIT_TREE, if json { &["--json"] } else { &[] });
            let single = root.submit(words.as_slice());
            assert!(root.reads.is_empty());
            root.deliver_subject(&f::tree_watch(), tree.clone());
            let single = root.answer(single).expect("tree alone");
            assert_eq!(single.status, 0);
            if json {
                assert_eq!(single.stdout, f::subject_json(&crate::jig::render::Part::delivery(&tree).expect("tree")));
                assert!(f::has(&compound, &single.stdout));
            } else {
                assert!(single.stdout.starts_with(b"tree:"));
                assert!(f::has(&single, b"digest: tree "));
                assert!(compound.stdout.ends_with(&single.stdout));
                assert!(!f::has(&single, b"met\n"));
            }
            assert_eq!(crate::root::next_deadline(&root.root), None);
        }
    }
}
