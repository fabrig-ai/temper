use temper_client_world::{
    app::{Event, Root},
    fixtures,
    front::{Joint, account_acts as accounts},
};
fn call(front: &mut Joint, root: &mut Root, number: u32, words: &[&[u8]]) -> String {
    let printed = invoke(front, root, number, words);
    assert_eq!(
        printed.status,
        0,
        "call {number}: {words:?}: {printed:?}; refusal {:?}",
        String::from_utf8_lossy(&printed.stderr)
    );
    String::from_utf8(printed.stdout.to_vec()).expect("UTF-8 output")
}
fn invoke<'a>(
    front: &mut Joint,
    root: &'a mut Root,
    number: u32,
    words: &[&[u8]],
) -> &'a temper_client_tools::jig::Printed {
    let Event::Call { call, json, .. } = fixtures::call(words) else { unreachable!() };
    accounts::event(front, root, Event::Call { number, call, json, value: None });
    root.agent().printed(number).expect("call answered")
}
#[test]
fn a_person_raises_a_spent_project_and_its_next_chat_runs() {
    let (mut front, mut root) = accounts::tools_fixture_with_spend(fixtures::SEED, 100);
    let chat = call(&mut front, &mut root, 1, &[b"chat", b"start", b"--project", b"1", b"--key", b"first", b"Hello"]);
    assert!(chat.starts_with("done: task "), "{chat}");
    let first = fixtures::done_task(root.agent().printed(1).expect("first chat created"));
    finish_chat(&mut front, &mut root, first, 100);
    let refused = invoke(
        &mut front,
        &mut root,
        3,
        &[b"chat", b"start", b"--project", b"1", b"--key", b"empty", b"Again", b"--json"],
    );
    assert_eq!(refused.status, 2, "{refused:?}");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("budget"), "{refused:?}");
    let refused =
        invoke(&mut front, &mut root, 10, &[b"chat", b"start", b"--project", b"1", b"--key", b"empty", b"Again"]);
    assert_eq!(refused.status, 2);
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr),
        "chat start: refused: your pool in project 1 has 0 left; budget 100\nread it: project show 1\nraise it: project fund 1 AMOUNT --role 0, and first the project's if its period has no room: project fund 1 AMOUNT\n"
    );
    let before = call(&mut front, &mut root, 4, &[b"project", b"show", b"1"]);
    assert!(before.contains("pool, party 1 "), "{before}");
    assert!(before.contains("budget 100, reserved 0, spent 100, left 0"), "{before}");
    let period = call(&mut front, &mut root, 5, &[b"project", b"fund", b"1", b"2000", b"--key", b"period"]);
    assert_eq!(period, "project 1: spend per period 2000\nnow: period 1: budget 2000, left 1900\n");
    let pool = call(&mut front, &mut root, 6, &[b"project", b"fund", b"1", b"200", b"--role", b"0", b"--key", b"pool"]);
    assert!(
        pool.starts_with("role 0: spend per period 200\nnow: period 1: budget 2000, left 1800\nnow: pool, party 1 "),
        "{pool}"
    );
    assert!(pool.ends_with(": budget 200, left 100\n"), "{pool}");
    call(&mut front, &mut root, 7, &[b"chat", b"start", b"--project", b"1", b"--key", b"funded", b"Now continue"]);
    let second = fixtures::done_task(root.agent().printed(7).expect("funded chat created"));
    assert_ne!(first, second);
    finish_chat(&mut front, &mut root, second, 100);
    let after = call(&mut front, &mut root, 9, &[b"project", b"show", b"1"]);
    assert!(after.contains("spend per period: 2000"), "{after}");
    assert!(after.contains("budget 200, reserved 0, spent 200, left 0"), "{after}");
}

fn finish_chat(front: &mut Joint, root: &mut Root, task: u64, spent: u64) {
    fixtures::finish_chat(front, task, spent);
    accounts::settle(front, root);
}

#[test]
fn fresh_ctl_replays_the_first_fund_edit_and_reads_todays_balances() {
    let mut world = fixtures::world(fixtures::SEED);
    let first = fixtures::agent(&mut world);
    for words in [
        &[b"project".as_slice(), b"fund", b"1", b"400", b"--role", b"0", b"--key", b"role-first"][..],
        &[b"project".as_slice(), b"fund", b"1", b"900", b"--key", b"period-first"][..],
        &[b"project".as_slice(), b"fund", b"1", b"300", b"--role", b"0", b"--key", b"role-next"][..],
        &[b"project".as_slice(), b"fund", b"1", b"800", b"--key", b"period-next"][..],
    ] {
        let number = fixtures::give(&mut world, first, words);
        let answer = fixtures::answered(&mut world, first, number);
        assert_eq!(answer.status, 0, "{answer:?}");
    }
    // The shipped ctl starts a new client for each invocation. This client has
    // not seen either key; the engine must answer with their retained edits.
    let replay = fixtures::agent(&mut world);
    for (words, edit) in [
        (
            &[b"project".as_slice(), b"fund", b"1", b"400", b"--role", b"0", b"--key", b"role-first"][..],
            "role 0: spend per period 400\n",
        ),
        (
            &[b"project".as_slice(), b"fund", b"1", b"900", b"--key", b"period-first"][..],
            "project 1: spend per period 900\n",
        ),
    ] {
        let number = fixtures::give(&mut world, replay, words);
        let answer = fixtures::answered(&mut world, replay, number);
        assert_eq!(answer.status, 0, "{answer:?}");
        let text = String::from_utf8(answer.stdout.into_vec()).expect("UTF-8 output");
        assert!(text.starts_with(edit), "{text}");
        assert!(text.contains("now: period 1: budget 800,"), "{text}");
    }
}

#[test]
fn a_role_cut_to_zero_refuses_chat_with_the_pool_remedy() {
    let (mut front, mut root) = accounts::tools_fixture_with_spend(fixtures::SEED, 100);
    call(&mut front, &mut root, 1, &[b"project", b"fund", b"1", b"0", b"--role", b"0", b"--key", b"cut"]);
    let refused =
        invoke(&mut front, &mut root, 2, &[b"chat", b"start", b"--project", b"1", b"Hello", b"--key", b"empty"]);
    assert_eq!(refused.status, 2, "{refused:?}");
    let text = String::from_utf8_lossy(&refused.stderr);
    assert!(text.starts_with("chat start: refused: your pool in project 1"), "{text}");
    assert!(text.contains("\nread it: project show 1\n"), "{text}");
    assert!(text.contains("raise it: project fund 1 AMOUNT --role 0"), "{text}");
    assert!(text.ends_with("project fund 1 AMOUNT\n"), "{text}");
}
