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
    let before = call(&mut front, &mut root, 4, &[b"project", b"show", b"1"]);
    assert!(before.contains("pool, party 1 "), "{before}");
    assert!(before.contains("budget 100, reserved 0, spent 100, left 0"), "{before}");
    let period = call(&mut front, &mut root, 5, &[b"project", b"fund", b"1", b"2000", b"--key", b"period"]);
    assert_eq!(period, "period 1: budget 2000, left 1900\n");
    let pool = call(&mut front, &mut root, 6, &[b"project", b"fund", b"1", b"200", b"--role", b"0", b"--key", b"pool"]);
    assert!(pool.starts_with("period 1: budget 2000, left 1800\npool, party 1 "), "{pool}");
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
