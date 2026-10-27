use crate::tests::fixtures as f;
use jig_client::contract as c;
use skein_lib::List;

#[test]
fn amend_sends_only_the_main_choice_change() {
    let (root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    assert_eq!(
        f::act_ask(&root, call),
        &c::Ask::Amend {
            project: f::PROJECT,
            task: f::TASK,
            revision: Some(c::Revision { value: f::CHOICE_SPEC }),
            change: f::model_change(Some(f::EFFORT_NAME)),
            reason: f::act_text(b"because"),
        }
    );
}
#[test]
fn amend_sends_the_named_effort() {
    let (root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    let c::Ask::Amend { change, .. } = f::act_ask(&root, call) else { panic!("amend") };
    assert_eq!(change.llm.as_ref().expect("choice").main.effort, Some(f::act_text(f::EFFORT_NAME)));
}
#[test]
fn amend_can_start_a_task_choice() {
    let mut task = f::choice_task(c::Admission::Allowed);
    task.llm = None;
    // A previously claimed account is a separate fact, never copied into the preference.
    task.kept_account = Some(99);
    let (root, call) = f::send_choice_amend(f::MODEL_AMEND, task);
    let c::Ask::Amend { change, .. } = f::act_ask(&root, call) else { panic!("amend") };
    assert_eq!(
        change.llm,
        Some(c::LlmChoice {
            account: None,
            main: f::model_effort(f::MODEL_NAME, Some(f::EFFORT_NAME)),
            subagents: List::with_capacity(0),
        })
    );
}
#[test]
fn amend_proposes_the_exact_change() {
    let (root, call) = f::send_choice_amend(
        f::MODEL_AMEND,
        f::choice_task(c::Admission::Proposed { to: c::Recipient::Party(f::RECIPIENT) }),
    );
    assert_eq!(
        f::act_ask(&root, call),
        &c::Ask::Propose {
            project: f::PROJECT,
            action: c::ProposeAction::Amendment {
                task: f::TASK,
                revision: c::Revision { value: f::CHOICE_SPEC },
                change: f::model_change(Some(f::EFFORT_NAME)),
            },
            reason: f::act_text(b"because"),
        }
    );
}
#[test]
fn amend_sends_within_a_bounded_offer() {
    let mut task = f::choice_task(c::Admission::Allowed);
    task.acts.get_mut(0).expect("amend offer").admission = c::Admission::Bounded {
        within: c::Bounds::Amend {
            budget_left: 0,
            grants: List::with_capacity(0),
            implications: List::with_capacity(0),
            authority: Box::new(task.authority.clone()),
            ceiling: Box::new(task.authority.clone()),
        },
        beyond: c::Beyond::Refused(c::Refusal::NotOffered),
    };
    let (root, call) = f::send_choice_amend(f::MODEL_AMEND, task);
    assert_eq!(
        f::act_ask(&root, call),
        &c::Ask::Amend {
            project: f::PROJECT,
            task: f::TASK,
            revision: Some(c::Revision { value: f::CHOICE_SPEC }),
            change: f::model_change(Some(f::EFFORT_NAME)),
            reason: f::act_text(b"because"),
        }
    );
}
#[test]
fn amend_refuses_a_denied_offer_locally() {
    let (mut root, call) =
        f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Refused(c::Refusal::NotOffered)));
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("not offered");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"amendment not offered\n".as_slice()));
}
#[test]
fn amend_requires_an_offer() {
    let mut task = f::choice_task(c::Admission::Allowed);
    task.acts.clear();
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, task);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("not offered");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"amendment not offered\n".as_slice()));
}
#[test]
fn amend_requires_a_change() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--rev", "7", "because"]),
        Err(crate::jig::argv::Usage::Missing { argument: 2 })
    );
}
#[test]
fn amend_requires_a_revision_for_a_model_change() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--model", "chosen-model", "because"]),
        Err(crate::jig::argv::Usage::AmendmentRevision)
    );
}
#[test]
fn amend_requires_a_reason() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--rev", "7", "--model", "chosen-model"]),
        Err(crate::jig::argv::Usage::Missing { argument: 12 })
    );
}
#[test]
fn amend_refuses_an_unknown_model() {
    let (mut root, call) = f::send_choice_amend(
        &["task", "amend", "41", "--rev", "7", "--model", "unknown", "because"],
        f::choice_task(c::Admission::Allowed),
    );
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("unknown model");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"unknown model: unknown\n".as_slice()));
}
#[test]
fn amend_refuses_an_unknown_effort() {
    let (mut root, call) = f::send_choice_amend(
        &["task", "amend", "41", "--rev", "7", "--model", "chosen-model", "--effort", "unknown", "because"],
        f::choice_task(c::Admission::Allowed),
    );
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("unknown effort");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"unknown effort: unknown\n".as_slice()));
}
#[test]
fn amend_refuses_an_account_flag() {
    assert_eq!(
        f::parse(f::extended(f::MODEL_AMEND, &["--account", "43"]).as_slice()),
        Err(crate::jig::argv::Usage::Unknown { at: 10 })
    );
}
#[test]
fn amend_choice_roundtrips_through_json() {
    let call = f::call(f::MODEL_AMEND);
    let json = crate::jig::json::write_call(crate::table::TOOLS, &call).expect("call JSON");
    assert_eq!(f::from_json(&json), call);
}
#[test]
fn amend_choice_roundtrips_through_printing() {
    let call = f::call(f::EFFORTLESS_AMEND);
    assert_eq!(f::printed(&call), crate::jig::call::Parsed::Call { call, json: false });
}
#[test]
fn a_named_amend_sends_the_callers_key() {
    let (root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    assert_eq!(root.sent_name(call), Some(f::named("model-amend")));
}
#[test]
fn an_unnamed_amend_draws_a_key() {
    let (root, call) = f::send_choice_amend(
        &["task", "amend", "41", "--rev", "7", "--model", "chosen-model", "--effort", "high", "because"],
        f::choice_task(c::Admission::Allowed),
    );
    assert_eq!(root.sent_name(call), Some(*root.saved.get(&call).expect("drawn key saved")));
}
#[test]
fn amend_prints_done() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    root.done(call, c::Outcome::Changed);
    let answer = root.answer(call).expect("done");
    assert_eq!((answer.status, answer.stdout.as_ref()), (0, b"done: task 41, amend\n".as_slice()));
}
#[test]
fn amend_renders_an_engine_llm_refusal() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Llm { model: f::act_text(f::MODEL_NAME), why: c::LlmWhy::Authority },
            at: None,
        },
    );
    let answer = root.answer(call).expect("refusal");
    assert_eq!(
        (answer.status, answer.stderr.as_ref()),
        (2, b"amend: This model cannot be chosen: chosen-model\n".as_slice())
    );
}
#[test]
fn amend_renders_a_json_llm_refusal() {
    let argv = f::extended(f::MODEL_AMEND, f::JSON_FLAG);
    let (mut root, call) = f::send_choice_amend(argv.as_slice(), f::choice_task(c::Admission::Allowed));
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Llm { model: f::act_text(f::MODEL_NAME), why: c::LlmWhy::Effort },
            at: None,
        },
    );
    let answer = root.answer(call).expect("refusal");
    assert_eq!((answer.status, answer.stderr.as_ref()),
        (2, br#"{"operation":"amend","outcome":{"refused":{"refusal":{"llm":{"model":"chosen-model","why":"effort"}},"at":null}},"task":41,"to":null}"#.as_slice()));
}
#[test]
fn amend_renders_an_engine_account_refusal() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    root.answered(
        call,
        c::Answer::Refused { refusal: c::Refusal::Account { account: 43, why: c::AccountWhy::Limited }, at: None },
    );
    let answer = root.answer(call).expect("refusal");
    assert_eq!(
        (answer.status, answer.stderr.as_ref()),
        (2, b"amend: Kept account is limited; cannot switch to account 43\n".as_slice())
    );
}
#[test]
fn amend_renders_a_moved_spec_refusal() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Moved { now: c::Revision { value: f::CHOICE_SPEC + 1 }, what: c::Moved::Spec },
            at: None,
        },
    );
    let answer = root.answer(call).expect("refusal");
    assert_eq!(
        (answer.status, answer.stderr.as_ref()),
        (2, b"amend: The object changed\nread again: task 41\n".as_slice())
    );
}
#[test]
fn task_offers_print_the_general_amend_form() {
    let (mut root, call, _) = f::task_read();
    let mut task = f::choice_task(c::Admission::Allowed);
    task.spec_revision = c::Revision { value: 8 };
    root.serve(call, f::task_result(task));
    let answer = root.answer(call).expect("task read");
    assert!(skein_lib::bytes::find(&answer.stdout, b"amend 41 --rev 8 CHANGE... REASON --key NAME").is_some());
}

#[test]
fn amend_refusal_keeps_the_named_retry() {
    let (mut root, call) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Authority { lacking: c::Lacking::Approver, proposable: Some(true) },
            at: None,
        },
    );
    assert_eq!(root.answer(call).expect("refusal").stderr.as_ref(),
        b"amend: Authority missing\npropose instead: amend 41 because --rev 7 --model chosen-model --effort high --key model-amend\n");
}
#[test]
fn a_named_amend_is_stable_across_clocks() {
    let (first, a) = f::send_choice_amend(f::MODEL_AMEND, f::choice_task(c::Admission::Allowed));
    let mut second = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    second.now = skein_lib::Time::from_nanos(f::LATER);
    second.wall = skein_lib::Wall::from_nanos(f::LATER);
    let b = second.submit(f::MODEL_AMEND);
    second.serve(b, f::task_result(f::choice_task(c::Admission::Allowed)));
    assert_eq!(
        f::encoded(&first.sends.get(&a).expect("first ask").2),
        f::encoded(&second.sends.get(&b).expect("second ask").2)
    );
}

#[test]
fn amend_requires_an_effort_for_a_model_with_efforts() {
    let mut task = f::choice_task(c::Admission::Allowed);
    task.llm.as_mut().expect("prior choice").main = f::model_effort(f::MODEL_NAME, Some(f::EFFORT_NAME));
    let (mut root, call) =
        f::send_choice_amend(&["task", "amend", "41", "--rev", "7", "--model", "chosen-model", "because"], task);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"effort required: chosen-model\n".as_slice()));
}
#[test]
fn amend_sends_no_effort_for_an_effortless_model() {
    let (root, call) = f::send_choice_amend(f::EFFORTLESS_AMEND, f::choice_task(c::Admission::Allowed));
    let c::Ask::Amend { change, .. } = f::act_ask(&root, call) else { panic!("amend") };
    let mut expected = f::prior_choice();
    expected.main = f::model_effort(f::EFFORTLESS_MODEL_NAME, None);
    assert_eq!(change.llm, Some(expected));
}
#[test]
fn task_without_llm_offers_the_general_amend_form() {
    let (mut root, call, _) = f::task_read();
    let mut task = f::choice_task(c::Admission::Allowed);
    task.spec_revision = c::Revision { value: 8 };
    task.llm = None;
    root.serve(call, f::task_result(task));
    let answer = root.answer(call).expect("task read");
    assert!(skein_lib::bytes::find(&answer.stdout, b"amend 41 --rev 8 CHANGE... REASON --key NAME").is_some());
}
