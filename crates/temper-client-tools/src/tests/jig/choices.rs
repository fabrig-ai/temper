use crate::tests::fixtures as f;
use jig_client::contract as c;
use skein_lib::List;

#[test]
fn chat_uses_the_named_model() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(f::MODEL_CHAT);
    assert_eq!(f::chat_task(&root, call).llm.as_ref().expect("choice").main.model.bytes(), f::MODEL_NAME);
}
#[test]
fn chat_uses_the_named_effort() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(f::MODEL_CHAT);
    assert_eq!(f::chat_task(&root, call).llm.as_ref().expect("choice").main.effort, Some(f::act_text(f::EFFORT_NAME)));
}
#[test]
fn chat_keeps_the_template_account() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(f::MODEL_CHAT);
    assert_eq!(f::chat_task(&root, call).llm.as_ref().expect("choice").account, f::prior_choice().account);
}
#[test]
fn chat_keeps_the_template_subagents() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(f::MODEL_CHAT);
    assert_eq!(f::chat_task(&root, call).llm.as_ref().expect("choice").subagents, f::prior_choice().subagents);
}
#[test]
fn chat_keeps_the_template_choice() {
    let me = f::choice_me(f::CHAT_BUDGET);
    let expected = jig_client::shapes::chat(&me, f::PROJECT, f::call_words(f::CREATE_CHAT_CALL, 1)).expect("shape");
    let mut root = f::choice_root(me);
    let call = root.submit(f::CREATE_CHAT_CALL);
    assert_eq!(f::act_ask(&root, call), &expected);
}
#[test]
fn chat_keeps_the_engine_default() {
    let me = f::chat_me(f::CHAT_BUDGET);
    let expected = jig_client::shapes::chat(&me, f::PROJECT, f::call_words(f::CREATE_CHAT_CALL, 1)).expect("shape");
    let mut root = f::choice_root(me);
    let call = root.submit(f::CREATE_CHAT_CALL);
    assert_eq!(f::act_ask(&root, call), &expected);
}
#[test]
fn chat_can_name_a_model_without_a_template_choice() {
    let mut me = f::choice_me(f::CHAT_BUDGET);
    me.projects.get_mut(0).expect("project").templates.get_mut(0).expect("template").llm = None;
    let mut root = f::choice_root(me);
    let call = root.submit(f::MODEL_CHAT);
    assert_eq!(
        f::chat_task(&root, call).llm,
        Some(c::LlmChoice {
            account: None,
            main: f::model_effort(f::MODEL_NAME, Some(f::EFFORT_NAME)),
            subagents: List::with_capacity(0),
        })
    );
}
#[test]
fn chat_proposes_the_named_choice() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET + f::OVER_BOUND));
    let call = root.submit(f::MODEL_CHAT);
    let c::Ask::Propose { project, action: c::ProposeAction::Batch { tasks }, reason } = f::act_ask(&root, call) else {
        panic!("proposed chat")
    };
    assert_eq!(*project, f::PROJECT);
    assert_eq!(reason.bytes(), b"");
    assert_eq!(
        tasks.get(0).expect("one chat").llm.as_ref().expect("choice").main,
        f::model_effort(f::MODEL_NAME, Some(f::EFFORT_NAME))
    );
}
#[test]
fn unknown_chat_model_names_the_usage_error() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call =
        root.submit(&["chat", "start", "hello", "--project", "shop", "--model", "unknown-model", "--key", "unknown"]);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"unknown model: unknown-model\n".as_slice()));
}
#[test]
fn unknown_chat_effort_names_the_usage_error() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(&[
        "chat",
        "start",
        "hello",
        "--project",
        "shop",
        "--model",
        "chosen-model",
        "--effort",
        "unknown-effort",
        "--key",
        "unknown",
    ]);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"unknown effort: unknown-effort\n".as_slice()));
}
#[test]
fn chat_model_lookup_is_scoped_to_the_project() {
    let mut me = f::choice_me(f::CHAT_BUDGET);
    let mut other = me.projects.get(0).expect("project").clone();
    other.project = f::PROJECT + 1;
    other.name = f::document_text(b"other");
    other.models.clear();
    let mut projects = List::with_capacity(2);
    projects.push(me.projects.get(0).expect("project").clone()).expect("first");
    projects.push(other).expect("other");
    me.projects = projects;
    let mut root = f::choice_root(me);
    let call =
        root.submit(&["chat", "start", "hello", "--project", "other", "--model", "chosen-model", "--key", "other"]);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"unknown model: chosen-model\n".as_slice()));
}
#[test]
fn chat_effort_requires_a_model() {
    assert_eq!(
        f::parse(&["chat", "start", "hello", "--project", "shop", "--effort", "high", "--key", "effort"]),
        Err(crate::jig::argv::Usage::Missing { argument: 2 })
    );
}
#[test]
fn chat_model_refuses_stdin() {
    assert_eq!(
        f::parse(&["chat", "start", "hello", "--project", "shop", "--model", "-", "--key", "model"]),
        Err(crate::jig::argv::Usage::Kind { argument: 2 })
    );
}
#[test]
fn chat_effort_refuses_stdin() {
    assert_eq!(
        f::parse(&[
            "chat",
            "start",
            "hello",
            "--project",
            "shop",
            "--model",
            "chosen-model",
            "--effort",
            "-",
            "--key",
            "model"
        ]),
        Err(crate::jig::argv::Usage::Kind { argument: 3 })
    );
}
#[test]
fn chat_choice_roundtrips_through_json() {
    let call = f::call(f::MODEL_CHAT);
    let json = crate::jig::json::write_call(crate::table::TOOLS, &call).expect("call JSON");
    assert_eq!(f::from_json(&json), call);
}

#[test]
fn json_chat_effort_requires_a_model() {
    assert_eq!(
        f::json(
            br#"{"tool":"chat start","arguments":{"project":"shop","text":"hello","effort":"high","key":"effort"}}"#
        ),
        Err(crate::jig::argv::Usage::Missing { argument: 2 })
    );
}

#[test]
fn a_long_unknown_model_prints_a_json_usage_error() {
    let name = "x".repeat(4096);
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call =
        root.submit(&["chat", "start", "hello", "--project", "shop", "--model", &name, "--key", "long", "--json"]);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!(answer.status, 1);
    let name = answer
        .stderr
        .strip_prefix(b"{\"error\":\"unknown model: ")
        .expect("prefix")
        .strip_suffix(b"...\"}")
        .expect("cut marker");
    assert_eq!(name, "x".repeat(256).as_bytes());
}

#[test]
fn a_model_name_requires_utf8() {
    assert_eq!(
        crate::table::parse(&[
            b"chat",
            b"start",
            b"hello",
            b"--project",
            b"shop",
            b"--model",
            b"\xff",
            b"--key",
            b"invalid"
        ]),
        Err(crate::jig::argv::Usage::Kind { argument: 2 })
    );
}

#[test]
fn model_names_follow_the_text_bound() {
    let mut limits = crate::limits::tools();
    limits.text = 4;
    for name in [b"name".as_slice(), b"names".as_slice()] {
        let parsed = crate::jig::argv::parse(
            crate::table::TOOLS,
            &[b"chat", b"start", b"hi", b"--project", b"shop", b"--key", b"bound", b"--model", name],
            &limits,
        );
        if name.len() == 4 {
            parsed.expect("model at bound");
        } else {
            assert_eq!(parsed, Err(crate::jig::argv::Usage::Long { argument: 2 }));
        }
    }
}

#[test]
fn effort_names_follow_the_text_bound() {
    let mut limits = crate::limits::tools();
    limits.text = 4;
    for name in [b"name".as_slice(), b"names".as_slice()] {
        let parsed = crate::jig::argv::parse(
            crate::table::TOOLS,
            &[
                b"chat",
                b"start",
                b"hi",
                b"--project",
                b"shop",
                b"--key",
                b"bound",
                b"--model",
                b"ok",
                b"--effort",
                name,
            ],
            &limits,
        );
        if name.len() == 4 {
            parsed.expect("effort at bound");
        } else {
            assert_eq!(parsed, Err(crate::jig::argv::Usage::Long { argument: 3 }));
        }
    }
}
#[test]
fn chat_choice_roundtrips_through_printing() {
    let call = f::call(f::EFFORTLESS_CHAT);
    assert_eq!(f::printed(&call), crate::jig::call::Parsed::Call { call, json: false });
}
#[test]
fn chat_sends_no_effort_for_an_effortless_model() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit(f::EFFORTLESS_CHAT);
    assert_eq!(
        f::chat_task(&root, call).llm.as_ref().expect("choice").main,
        f::model_effort(f::EFFORTLESS_MODEL_NAME, None)
    );
}
#[test]
fn chat_requires_an_effort_for_a_model_with_efforts() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call =
        root.submit(&["chat", "start", "hello", "--project", "shop", "--model", "chosen-model", "--key", "effort"]);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("usage error");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"effort required: chosen-model\n".as_slice()));
}
#[test]
fn unknown_model_control_bytes_are_cut() {
    for name in ["safe\u{1b}unsafe", "safe\u{7f}unsafe", "safe\u{80}unsafe"] {
        let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
        let call = root.submit(&["chat", "start", "hello", "--project", "shop", "--model", name, "--key", "control"]);
        let answer = root.answer(call).expect("usage error");
        assert_eq!(answer.stderr.as_ref(), b"unknown model: safe...\n");
    }
}
#[test]
fn unknown_model_echo_keeps_a_utf8_boundary() {
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let mut name = "x".repeat(255);
    name.push('é');
    let call = root.submit(&["chat", "start", "hello", "--project", "shop", "--model", &name, "--key", "unicode"]);
    let answer = root.answer(call).expect("usage error");
    let echo =
        answer.stderr.strip_prefix(b"unknown model: ").expect("prefix").strip_suffix(b"...\n").expect("cut marker");
    assert_eq!(echo, "x".repeat(255).as_bytes());
}
