//! Replays cross the same runner boundary as new calls, after subjects retire.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary fixtures and printed-byte assertions; testing-strategy.md, step tests"
)]
use crate::{
    jig::{call, rows},
    tests::fixtures as f,
};
use jig_client::contract as c;
use skein_lib::{List, Wall};

fn historical(key: c::ObjectKey, value: c::OwnerHistoricalValue) -> c::ReadResult {
    c::ReadResult::Value(c::ReadValue::Historical(Box::new(c::OwnerHistorical { key, value, at: None, by: None })))
}
fn ended() -> c::ReadResult {
    let mut task = f::ended_task(c::Phase::Cancelled);
    task.acts = List::with_capacity(0);
    f::historical_task_result(task, c::Ending::Done)
}
fn retired(call: &call::Call) -> c::ReadResult {
    if call.tool == rows::ANSWER_AT {
        let c::ReadResult::Value(c::ReadValue::Question(question)) = f::act_document(call) else { panic!("question") };
        let mut question = *question;
        question.acts = List::with_capacity(0);
        return historical(
            c::ObjectKey::Question { task: f::TASK, message: 3 },
            c::OwnerHistoricalValue::Question {
                question,
                answer: None,
                ending: Some(Box::new(c::Result {
                    task: f::ended_task(c::Phase::Done).chip,
                    ending: c::Ending::Done,
                    value: None,
                    at: None,
                    details: None,
                    acts: List::with_capacity(0),
                })),
            },
        );
    }
    if call.tool == rows::TASK_DECIDE_AT {
        return historical(
            c::ObjectKey::Escalation(f::TASK),
            c::OwnerHistoricalValue::Escalation {
                task: Box::new(f::ended_task(c::Phase::Cancelled)),
                decision: c::OwnerEscalationDecisionRecord {
                    project: f::PROJECT,
                    requester: f::PARTY,
                    task: f::TASK,
                    revision: f::HOLD,
                    by: f::PARTY,
                    at: Wall::EPOCH,
                    decision: c::OwnerEscalationDecision::Release,
                },
                hold: None,
            },
        );
    }
    if call.tool == rows::DECIDE_AT {
        let Some(call::Value::Proposal(key)) = call::value(call, 0) else { panic!("proposal") };
        let c::Proposer::Task(proposer) = key.proposer else { panic!("task proposal") };
        return historical(
            c::ObjectKey::Proposal(*key),
            c::OwnerHistoricalValue::Proposal(c::OwnerProposalHistory {
                number: key.number,
                proposer,
                as_holder: false,
                state: c::OwnerProposalHistoryState::Accepted { by: c::OwnerRequester::Person(f::PARTY) },
                action: c::OwnerRetainedAction::Release { task: f::TASK },
            }),
        );
    }
    ended()
}
fn serve(root: &mut f::Harness, number: u32, document: c::ReadResult) {
    root.serve(number, document);
    if root.reads.contains_key(&number) {
        root.serve(number, ended());
    }
}
const TASK_HOLD_CALL: &[&str] = &["task", "hold", "41", "--stop", "Review the work", "--key", "place-hold"];
fn replay(argv: &[&str]) {
    for json in [false, true] {
        for fresh in [false, true] {
            let parsed = f::call(argv);
            let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
            let first = root.submit_prepared(parsed.clone(), json, None);
            serve(&mut root, first, f::act_document(&parsed));
            let ask = f::encoded(&root.sends.get(&first).expect("first act sent").2);
            let key = root.sent_name(first);
            let outcome = if parsed.tool == rows::TASK_HOLD_AT {
                c::Outcome::Owner(c::OwnerOutcome::Placed { hold: 12, revision: c::Revision { value: 1 } })
            } else {
                c::Outcome::Changed
            };
            root.done(first, outcome.clone());
            let expected = root.answer(first).expect("first outcome");
            if fresh {
                root = f::chat_root(f::CHAT_BUDGET, f::OTHER_SEED);
            }
            let second = root.submit_prepared(parsed.clone(), json, None);
            serve(&mut root, second, retired(&parsed));
            assert_eq!(f::encoded(&root.sends.get(&second).expect("replay sent").2), ask, "{argv:?}");
            assert_eq!(root.sent_name(second), key);
            root.done(second, outcome);
            let actual = root.answer(second).expect("replayed outcome");
            assert_eq!(
                (actual.status, actual.stdout, actual.stderr),
                (expected.status, expected.stdout, expected.stderr)
            );
        }
    }
}
#[test]
fn task_message_replays_after_end() {
    replay(f::MESSAGE_CALL);
}
#[test]
fn task_answer_replays_its_retained_question() {
    replay(f::ACT_CALLS[2]);
}
#[test]
fn task_cancel_replays_after_end_in_this_or_a_fresh_directory() {
    replay(f::ACT_CALLS[7]);
}
#[test]
fn task_hold_replays_after_end() {
    replay(TASK_HOLD_CALL);
}
#[test]
fn task_stop_replays_after_end() {
    replay(f::ACT_CALLS[6]);
}
#[test]
fn task_release_replays_after_end() {
    replay(f::ACT_CALLS[5]);
}
#[test]
fn task_decide_replays_its_retained_escalation() {
    replay(f::DECIDE_ESCALATION_CALL);
}
#[test]
fn proposal_decide_replays_its_retained_proposal() {
    replay(f::DECIDE_TASK_CALL);
}
#[test]
fn chat_close_replays_after_end() {
    replay(f::CLOSE_CALL);
}
#[test]
fn task_cancel_with_a_new_key_on_an_ended_task_is_the_engines_refusal() {
    for (phase, words) in [(c::Phase::Done, "done"), (c::Phase::Cancelled, "cancelled")] {
        for extra in [&[][..], &["--grace", "0s"][..]] {
            for json in [false, true] {
                let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
                let argv = f::extended(f::ACT_CALLS[7], extra);
                let call = root.submit_prepared(f::call(argv.as_slice()), json, None);
                let mut task = f::ended_task(phase);
                task.acts = List::with_capacity(0);
                root.serve(call, f::historical_task_result(task, c::Ending::Done));
                assert!(root.sent_name(call).is_some());
                root.answered(call, c::Answer::Refused { refusal: c::Refusal::Unknown, at: None });
                let answer = root.answer(call).expect("engine refused");
                assert_eq!(answer.status, 2);
                if json {
                    let ending = format!("\"ended\":\"{words}\"");
                    assert!(answer.stderr.windows(ending.len()).any(|s| s == ending.as_bytes()));
                    assert!(answer.stderr.windows(9).any(|s| s == b"\"task\":41"));
                } else {
                    assert_eq!(answer.stderr.as_ref(), format!("task 41 has ended: {words}\n").as_bytes());
                }
            }
        }
    }
}
#[test]
fn task_amend_replays_equal_replacements_after_its_revision_moves() {
    let argv = ["task", "amend", "41", "--rev", "7", "--priority", "3", "because", "--key", "amend"];
    for json in [false, true] {
        let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
        let first = root.submit_prepared(f::call(&argv), json, None);
        let mut before = f::choice_task(c::Admission::Allowed);
        before.priority = Some(1);
        root.serve(first, f::task_result(before));
        let ask = f::act_ask(&root, first).clone();
        root.done(first, c::Outcome::Changed);
        let expected = root.answer(first).unwrap();
        let mut task = f::ended_task(c::Phase::Done);
        task.priority = Some(3);
        task.spec_revision.value = task.spec_revision.value.saturating_add(1);
        task.acts = List::with_capacity(0);
        let second = root.submit_prepared(f::call(&argv), json, None);
        root.serve(second, f::historical_task_result(task, c::Ending::Done));
        assert_eq!(f::act_ask(&root, second), &ask);
        root.done(second, c::Outcome::Changed);
        let actual = root.answer(second).unwrap();
        assert_eq!((actual.status, actual.stdout, actual.stderr), (expected.status, expected.stdout, expected.stderr));
    }
}

#[test]
fn unkeyed_cancel_on_an_ended_task_still_asks_the_engine() {
    let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
    let call = root.submit(&f::ACT_CALLS[7][..4]);
    root.serve(call, ended());
    root.answered(call, c::Answer::Refused { refusal: c::Refusal::Unknown, at: None });
    assert_eq!(root.answer(call).unwrap().status, 2);
}
#[test]
fn a_mismatched_historical_subject_cannot_route_an_act() {
    let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
    let call = root.submit(f::ACT_CALLS[7]);
    let mut wrong = f::ended_task(c::Phase::Cancelled);
    wrong.chip.task = wrong.chip.task.saturating_add(1);
    root.serve(call, f::historical_task_result(wrong, c::Ending::Done));
    assert!(root.sends.is_empty());
    assert_eq!(root.answer(call).unwrap().status, 1);
}

// Verbs without a retiring subject still send the same key and request, and
// print its first refusal in both formats. This also guards future table rows.
const DIRECT_CALLS: &[&[&str]] = &[
    f::CREATE_CHAT_CALL,
    &["project", "hold", "shop", "--stop", "Review the project", "--key", "project-hold"],
    f::ADOPTION,
    &["account", "connect", "9", "--key", "connect"],
    &["account", "disconnect", "43", "--key", "disconnect"],
    &["account", "add-key", "9", "file", "--key", "add"],
    &["account", "replace-key", "43", "file", "--key", "replace"],
    &["account", "forget", "43", "--key", "forget"],
    &["account", "rank", "43", "2", "--key", "rank"],
    &["account", "retry", "43", "--key", "retry"],
    &["account", "declare", "9", "helper", "work-login", "--key", "declare"],
    &["account", "verify", "43", "--key", "verify"],
    &["account", "import", "file", "--key", "import"],
    &["settings", "apply", "FILE", "--against", "5", "--key", "settings"],
    &["settings", "propose", "FILE", "--against", "5", "--key", "settings"],
    &["revision", "advance", "5", "--key", "advance"],
    &["project", "fund", "1", "200", "--key", "fund"],
    &["forge", "setup", "3", "--key", "setup"],
    &["forge", "connect", "3", "github", "--key", "connect"],
    &["forge", "install", "3", "--key", "install"],
    &["forge", "accept", "3", "22", "--key", "accept"],
    &["playground", "start", "--key", "start"],
];
fn direct(argv: &[&str]) {
    for json in [false, true] {
        let mut root = f::chat_root(f::CHAT_BUDGET, f::SEED);
        let mut me = f::chat_me(f::CHAT_BUDGET);
        me.providers = f::provider_offers();
        root.deliver(c::Delivery::Me(Box::new(me)));
        let mut parsed = f::call(argv);
        if parsed.tool == rows::CHANGE_SETTINGS_AT || parsed.tool == rows::PROPOSE_SETTINGS_AT {
            *parsed.values.get_mut(0).unwrap() = Some(call::Values::One(call::Value::Text(
                br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#.as_slice().into(),
            )));
        }
        let mut expected = None;
        let mut request = None;
        let mut key = None;
        for _ in 0_u32..2 {
            let value = if parsed.tool == rows::ADD_KEY_AT
                || parsed.tool == rows::REPLACE_KEY_AT
                || parsed.tool == rows::IMPORT_AT
            {
                Some(skein_lib::Secret::new(b"fake-record-for-step-tests".as_slice().into()))
            } else {
                None
            };
            let number = root.submit_prepared(parsed.clone(), json, value);
            let mut accounts = f::accounts();
            if parsed.tool == rows::DISCONNECT_AT {
                accounts.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().kind = c::OwnerAccountKind::SignIn;
            } else if parsed.tool == rows::VERIFY_AT {
                accounts.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().kind = c::OwnerAccountKind::Program;
            }
            f::deliver_accounts(&mut root, accounts);
            let sent = root.sends.get(&number).unwrap_or_else(|| panic!("act sent: {argv:?}"));
            if let Some(request) = &request {
                assert_eq!(&sent.2, request, "{argv:?}");
            }
            if let Some(key) = key {
                assert_eq!(root.sent_name(number), Some(key), "{argv:?}");
            }
            request = Some(sent.2.clone());
            key = root.sent_name(number);
            root.answered(number, c::Answer::Refused { refusal: c::Refusal::NotOffered, at: None });
            let actual = root.answer(number).expect("engine refusal rendered");
            assert_eq!(actual.status, 2, "{argv:?}");
            let actual = (actual.status, actual.stdout, actual.stderr);
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "{argv:?}");
            }
            expected = Some(actual);
        }
    }
}
#[test]
fn every_other_act_replays_its_first_outcome_in_text_and_json() {
    for argv in DIRECT_CALLS {
        direct(argv);
    }
}
#[test]
fn replay_cases_cover_every_keyed_verb_in_the_tools_table() {
    let mut covered = Vec::new();
    for argv in DIRECT_CALLS.iter().chain(f::ACT_CALLS.iter()) {
        covered.push(f::call(argv).tool);
    }
    covered.push(rows::AMEND_AT);
    covered.push(f::call(TASK_HOLD_CALL).tool);
    for (index, tool) in crate::table::TOOLS.iter().enumerate() {
        if tool.keyed != crate::jig::table::Keyed::Never {
            assert!(covered.contains(&u8::try_from(index).unwrap()), "missing replay for {:?}", tool.name);
        }
    }
}

#[test]
fn amendment_replays_its_original_direct_or_proposed_form_after_the_offer_changes() {
    for proposed in [false, true] {
        for json in [false, true] {
            let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
            let admission = if proposed {
                c::Admission::Proposed { to: c::Recipient::Party(f::RECIPIENT) }
            } else {
                c::Admission::Allowed
            };
            let first = root.submit_prepared(f::call(f::MODEL_AMEND), json, None);
            root.serve(first, f::task_result(f::choice_task(admission)));
            let original = f::act_ask(&root, first).clone();
            let outcome = if proposed {
                c::Outcome::Owner(c::OwnerOutcome::Proposed { proposal: f::PROPOSAL, to: 9 })
            } else {
                c::Outcome::Owner(c::OwnerOutcome::Amended { task: f::TASK })
            };
            root.done(first, outcome.clone());
            let expected = root.answer(first).unwrap();
            let mut task = f::choice_task(if proposed {
                c::Admission::Allowed
            } else {
                c::Admission::Proposed { to: c::Recipient::Party(f::RECIPIENT) }
            });
            task.llm = f::model_change(Some(f::EFFORT_NAME)).llm.clone();
            task.spec_revision.value = task.spec_revision.value.saturating_add(1);
            let second = root.submit_prepared(f::call(f::MODEL_AMEND), json, None);
            let page = if proposed {
                task.chip.phase = c::Phase::Done;
                task.acts = List::with_capacity(0);
                f::historical_task_result(task, c::Ending::Done)
            } else {
                f::task_result(task)
            };
            root.serve(second, page);
            assert_ne!(f::act_ask(&root, second), &original, "changed offer selects the other form");
            root.answered(second, c::Answer::Refused { refusal: c::Refusal::KeyConflict, at: None });
            let unexpected = root.answer(second);
            assert!(unexpected.is_none(), "a conflict may identify the other retained representation: {unexpected:?}");
            assert_eq!(f::act_ask(&root, second), &original);
            root.done(second, outcome);
            let actual = root.answer(second).unwrap();
            assert_eq!(
                (actual.status, actual.stdout, actual.stderr),
                (expected.status, expected.stdout, expected.stderr)
            );
        }
    }
}
#[test]
fn an_amendment_key_conflict_tries_the_other_form_only_once() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    for _ in 0_u32..2 {
        root.answered(call, c::Answer::Refused { refusal: c::Refusal::KeyConflict, at: None });
    }
    assert_eq!(root.answer(call).unwrap().status, 2);
}

#[test]
fn a_restored_proposed_amendment_is_joined_after_the_task_ends() {
    for json in [false, true] {
        let (original, call) = f::send_choice_amend(
            f::MODEL_AMEND,
            f::choice_task(c::Admission::Proposed { to: c::Recipient::Party(f::RECIPIENT) }),
        );
        let ask = f::act_ask(&original, call).clone();
        let mut records = f::restored_message(f::PARTY);
        let record = records.get_mut(0).unwrap();
        record.key = f::named("model-amend");
        record.request.request = jig_client::Ask::Jig(Box::new(ask));
        let mut root = f::started_with(records, f::OTHER_SEED, json);
        root.deliver(c::Delivery::Me(Box::new(f::choice_me(f::CHAT_BUDGET))));
        let number = root.submit_prepared(f::call(f::MODEL_AMEND), json, None);
        let mut task = f::choice_task(c::Admission::Allowed);
        task.chip.phase = c::Phase::Done;
        task.acts = List::with_capacity(0);
        root.serve(number, f::historical_task_result(task, c::Ending::Done));
        assert!(root.answer(number).is_none(), "join the kept ask after trying its other shape");
        root.done(f::EMPTY, c::Outcome::Owner(c::OwnerOutcome::Proposed { proposal: f::PROPOSAL, to: 9 }));
        assert_eq!(root.answer(number).unwrap().status, 0);
    }
}
