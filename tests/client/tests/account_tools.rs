use skein_lib::Secret;
use temper_client_world::{
    app::{Event, Root},
    fixtures,
    front::{Joint, account_acts as accounts},
};
const KEY: &[u8] = b"test-private-key";
const ADD: &[&[u8]] = &[b"account", b"add-key", b"17", b"file", b"--key", b"tools-key"];
const SECOND: &[&[u8]] = &[b"account", b"add-key", b"17", b"file", b"--key", b"second-key"];
fn call(front: &mut Joint, root: &mut Root, number: u32, words: &[&[u8]], value: Option<Secret>) {
    let Event::Call { call, json, .. } = fixtures::call(words) else { unreachable!() };
    accounts::event(front, root, Event::Call { number, call, json, value });
}
#[test]
fn a_lost_tools_key_answer_recovers_one_account() {
    // The first owner also sees the fixture's preseeded deployment account.
    const ADDED_PARTY_ACCOUNTS: usize = 1;
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    accounts::lose_next(&mut front);
    call(&mut front, &mut root, 1, ADD, Some(Secret::new(KEY.into())));
    accounts::kept(&mut front);
    accounts::settle(&mut front, &mut root);
    accounts::lost(&mut front, &mut root);
    assert_eq!(root.agent().printed(1).expect("lost answer").status, 1);
    call(&mut front, &mut root, 2, ADD, Some(Secret::new(KEY.into())));
    assert_eq!(root.agent().printed(2).expect("original answer").status, 0);
    call(&mut front, &mut root, 4, &[b"account", b"list", b"--json"], None);
    let printed = root.agent().printed(4).expect("one account projection");
    let text = std::str::from_utf8(&printed.stdout).expect("JSON");
    assert_eq!(text.matches("\"holder\":{\"party\":").count(), ADDED_PARTY_ACCOUNTS, "{text}");
    accounts::assert_no_carried(&front);
}
#[test]
fn a_tools_connect_wait_settles_at_the_sign_in_end() {
    use jig_domain_accounts as a;
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    call(&mut front, &mut root, 1, &[b"account", b"connect", b"17", b"--key", b"sign-in"], None);
    let source = accounts::signing(&front);
    front.engine.send(temper_engine_domain::Event::Account(a::Event::Visited {
        source,
        address: b"http://engine/visit".as_slice().into(),
    }));
    accounts::settle(&mut front, &mut root);
    assert_eq!(root.agent().printed(1).expect("durable visit").status, 0);
    call(&mut front, &mut root, 2, &[b"account", b"list", b"--until", b"settled"], None);
    assert!(root.agent().printed(2).is_none());
    front.engine.send(temper_engine_domain::Event::Account(a::Event::SignedIn {
        source,
        subject: b"subject".as_slice().into(),
        display: b"Test".as_slice().into(),
        declined: false,
    }));
    accounts::settle(&mut front, &mut root);
    assert_eq!(root.agent().printed(2).expect("settled watch").status, 0);
}
#[test]
fn tools_rank_reaches_the_account_holder() {
    let (mut front, mut root, account) = key_fixture();
    call(&mut front, &mut root, 3, SECOND, Some(Secret::new(KEY.into())));
    accounts::kept(&mut front);
    accounts::settle(&mut front, &mut root);
    assert_eq!(root.agent().printed(3).expect("second key kept").status, 0);
    call(&mut front, &mut root, 4, &[b"account", b"rank", account.as_bytes(), b"2"], None);
    assert_eq!(root.agent().printed(4).expect("ranked").status, 0);
    call(&mut front, &mut root, 5, &[b"account", b"list", b"--json"], None);
    let after = std::str::from_utf8(&root.agent().printed(5).expect("ranked order").stdout).expect("JSON");
    let first = after.find("\"place\":0,\"provider\":").expect("first account");
    let moved = after.find(&format!("{{\"number\":{account},\"place\":1,\"provider\":")).expect("moved account");
    assert!(first < moved, "{after}");
}
#[test]
fn tools_retry_preserves_the_engines_not_limited_refusal() {
    let (mut front, mut root, account) = key_fixture();
    call(&mut front, &mut root, 3, &[b"account", b"retry", account.as_bytes()], None);
    assert_eq!(root.agent().printed(3).expect("not limited").status, 2);
}
#[test]
fn tools_forget_removes_the_account() {
    let (mut front, mut root, account) = key_fixture();
    call(&mut front, &mut root, 3, &[b"account", b"forget", account.as_bytes()], None);
    assert_eq!(root.agent().printed(3).expect("forgotten").status, 0);
    call(&mut front, &mut root, 4, &[b"account", b"list", b"--json"], None);
    let printed = root.agent().printed(4).expect("account removed");
    assert!(skein_lib::bytes::find(&printed.stdout, b"\"party\":[]").is_some());
}
fn key_fixture() -> (Joint, Root, String) {
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    call(&mut front, &mut root, 1, ADD, Some(Secret::new(KEY.into())));
    let source = accounts::kept(&mut front);
    accounts::settle(&mut front, &mut root);
    assert_eq!(root.agent().printed(1).expect("key kept").status, 0);
    assert_ne!(source, 0);
    call(&mut front, &mut root, 2, &[b"account", b"list", b"--json"], None);
    let text = std::str::from_utf8(&root.agent().printed(2).expect("account number").stdout).expect("JSON");
    let account =
        text.split("\"number\":").nth(1).expect("party account").split(',').next().expect("number end").to_owned();
    (front, root, account)
}

#[test]
fn a_tools_chat_choice_reaches_its_task_document() {
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    call(
        &mut front,
        &mut root,
        1,
        &[b"chat", b"start", b"--project", b"1", b"--model", b"scripted", b"--key", b"choice", b"Hello"],
        None,
    );
    let task = fixtures::done_task(root.agent().printed(1).expect("created chat"));
    call(&mut front, &mut root, 2, &[b"task", b"show", task.to_string().as_bytes(), b"--json"], None);
    let printed = root.agent().printed(2).expect("task document");
    let text = std::str::from_utf8(&printed.stdout).expect("JSON");
    assert!(text.contains("\"main\":{\"model\":\"scripted\",\"effort\":null}"), "{text}");
}

#[test]
fn a_tools_chat_effort_reaches_its_task_document() {
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    call(
        &mut front,
        &mut root,
        1,
        &[
            b"chat",
            b"start",
            b"--project",
            b"1",
            b"--model",
            b"effort-model",
            b"--effort",
            b"high",
            b"--key",
            b"effort-choice",
            b"Hello",
        ],
        None,
    );
    let task = fixtures::done_task(root.agent().printed(1).expect("created chat"));
    call(&mut front, &mut root, 2, &[b"task", b"show", task.to_string().as_bytes(), b"--json"], None);
    let printed = root.agent().printed(2).expect("task document");
    let text = std::str::from_utf8(&printed.stdout).expect("JSON");
    assert!(text.contains("\"main\":{\"model\":\"effort-model\",\"effort\":\"high\"}"), "{text}");
}

#[test]
fn a_tools_declaration_is_visible_with_its_host_in_accounts() {
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    call(
        &mut front,
        &mut root,
        1,
        &[b"account", b"declare", b"17", b"claude_code", b"claude-work", b"--key", b"declare-login"],
        None,
    );
    let declared = root.agent().printed(1).expect("durable declaration");
    assert_eq!(declared.status, 0, "{declared:?}");
    assert!(skein_lib::bytes::find(&declared.stdout, b"temper program-login claude-work").is_some());
    call(&mut front, &mut root, 2, &[b"account", b"list", b"--json"], None);
    let printed = root.agent().printed(2).expect("declared source and hosts");
    assert_eq!(printed.status, 0);
    let text = std::str::from_utf8(&printed.stdout).expect("accounts JSON");
    for fact in [
        "Scripted",
        "\"kind\":\"program\"",
        "\"name\":\"claude-work\"",
        "\"program_hosts\":[{\"name\":\"worker\"",
        "\"connected\":true",
        "\"programs\":[{\"name\":\"claude-work\",\"state\":null",
    ] {
        assert!(text.contains(fact), "missing {fact}: {text}");
    }
}
