//! Complete worker stories with real child domains and outside observations.
use temper_worker_domain::{host, wire};
use temper_worker_domain_world::whole::{Case, run, saved, unprepared, workstreams};
#[test]
fn a_prepared_run_delivers_the_complete_tree_and_answers() {
    assert_eq!(run(1, Case::Delivered).cancellations, 0);
}
#[test]
fn cancelling_mid_delivery_waits_for_the_landing_before_answering() {
    assert_eq!(run(2, Case::CancelledDelivery).cancellations, 1);
}
#[test]
fn contact_restored_within_the_grace_keeps_the_agent_running() {
    assert_eq!(run(3, Case::ContactWithin).cancellations, 0);
}
#[test]
fn contact_lost_past_the_grace_saves_before_the_cancellation_answer() {
    assert_eq!(run(4, Case::ContactPast).saves, 1);
}
#[test]
fn the_hello_carries_the_configured_stop_bound_whole() {
    assert_eq!(run(71, Case::Delivered).stop_bound, temper_worker_domain_world::STOP_BOUND);
}
#[test]
fn stories_replay() {
    for case in [Case::Delivered, Case::CancelledDelivery, Case::ContactWithin, Case::ContactPast] {
        assert_eq!(run(71, case), run(71, case));
    }
}
#[test]
fn a_branch_the_forge_lacks_fails_the_prepare_naming_the_repository() {
    let (ending, preparation) = unprepared(5);
    let host::Ending::Failed { failure: host::Failure::Unprepared(host::Preparation::Permanent { .. }), .. } = ending
    else {
        panic!("a permanent preparation failure: {ending:?}")
    };
    assert_eq!(preparation, Some(wire::Preparation::Missing { tag: 1, missing: wire::Missing::Branch }));
}
#[test]
fn the_hello_names_the_workstreams_the_checkout_holds() {
    assert_eq!(&*workstreams(6), [81]);
}

#[test]
fn a_save_says_whether_each_repository_started_from_saved_work() {
    let rows = [
        (wire::Start::Base { branch: Box::from(&b"main"[..]) }, false),
        (wire::Start::Saved { branch: Box::from(&b"saved"[..]) }, true),
    ];
    for (start, expected) in rows {
        let name = format!("{start:?}");
        let work = saved(8, start);
        let saved = work.saved.expect("saved work");
        let [wire::SavedRepository { tag, from_saved, landing: wire::Landing::Landed { .. } }] = saved.as_ref() else {
            panic!("{name}: one repository's save landed: {saved:?}");
        };
        assert_eq!((*tag, *from_saved), (1, expected), "{name}");
    }
}

#[test]
fn a_host_answer_at_the_agents_reply_bound_reaches_it_whole() {
    use temper_worker_domain_world::whole::{REPLY_BYTES, host_answer};
    let smith_channel::Reply::Host(reply) = host_answer(91, REPLY_BYTES, false) else {
        panic!("the agent receives the whole host answer");
    };
    assert!(!reply.error());
    assert_eq!(reply.text(), vec![b'x'; usize::try_from(REPLY_BYTES).expect("reply length")]);
}

#[test]
fn a_host_answer_past_the_agents_reply_bound_reaches_it_as_too_large() {
    use temper_worker_domain_world::whole::{REPLY_BYTES, host_answer};
    for error in [false, true] {
        assert_eq!(host_answer(92, REPLY_BYTES + 1, error), smith_channel::Reply::TooLarge);
    }
}

#[test]
fn a_saved_retry_delivers_the_file_written_before_its_run_ended() {
    let report = temper_worker_domain_world::whole::retry(109, true);
    let expected =
        [(b"code".to_vec(), b"original\n".to_vec()), (b"changes/3.md".to_vec(), b"repair kept\n".to_vec())].into();
    assert_eq!(report.tree, expected);
    assert_eq!(report.saves, 1);
}

#[test]
fn a_run_starting_fresh_from_its_brief_starts_clean() {
    let report = temper_worker_domain_world::whole::retry(110, false);
    assert_eq!(report.tree, [(b"code".to_vec(), b"original\n".to_vec())].into());
    assert_eq!(report.saves, 0);
}

#[test]
fn a_landed_change_saves_nothing() {
    assert_eq!(run(111, Case::Delivered).saves, 0);
}
