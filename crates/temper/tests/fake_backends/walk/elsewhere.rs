use super::*;

fn adoption(call: &Ended) -> Value {
    // Connect prints progress before the terminal project/adoption answer.
    let answer = call.stdout.split(|byte| *byte == b'\n').rfind(|line| !line.is_empty()).expect("adoption answer");
    serde_json::from_slice(answer).expect("adoption JSON")
}

fn adopted_project(seen: &[Ended]) -> String {
    let [_, _, connected, ..] = seen else { panic!("connection answered") };
    adoption(connected)["project"].as_u64().expect("adopted project number").to_string()
}

fn chat_task(seen: &[Ended]) -> String {
    let [_, _, _, _, _, _, _, started, ..] = seen else { panic!("first chat started") };
    ctl::json(started)["outcome"]["done"]["owner"]["created"]["first"].as_u64().expect("chat's task number").to_string()
}

fn assert_ready(seen: &[Ended]) {
    let [before_model, before_forge, connected, _, _, ready_model, ready_forge] = seen else {
        panic!("welcome ready reads completed")
    };
    assert!(!usable(before_model), "no model before the key: the welcome must offer its Model step");
    let offered = forge_number(before_forge, 4);
    assert_eq!(offered["mode"], "managed");
    assert_eq!(offered["state"], "not_connected");
    assert!(offered["first"].is_null(), "no first repository adopted before Connect");
    let connected = adoption(connected);
    let adopted = &connected["outcome"]["adopted"];
    assert_eq!(adopted["forge"], 4);
    assert_eq!(adopted["owner"], "fake-person");
    assert_eq!(adopted["name"], "hello");
    assert_eq!(adopted["role"], "owned");
    assert_eq!(adopted["home"], true);
    assert!(connected["project"].as_u64().expect("adopted project") > 0, "repository adopted into a project");
    let ready = forge_number(ready_forge, 4);
    assert_eq!(ready["state"], "connected");
    assert_eq!(ready["first"]["state"], "made");
    assert_eq!(ready["first"]["org"], "fake-person");
    assert_eq!(ready["first"]["repository"], "hello");
    assert_eq!(ready["first"]["id"], adopted["repository"]);
    assert!(usable(ready_model), "key account usable at Ready");
}

pub(super) fn run() {
    let walk = Walk::new("walk-elsewhere-key");
    let admin = walk.sandbox().join("data/forgejo-admin.token");
    let seen = walk.run(vec![
        Act::Now(Call::new(&["account", "list", "--json"])),
        Act::Now(Call::new(&["forge", "list", "--json"])),
        Act::Now(Call::new(&[
            "forge",
            "connect",
            "4",
            "forgejo-admin",
            path(&admin),
            "--first",
            "fake-person/hello",
            "--key",
            "walk-forge",
            "--json",
        ])),
        Act::Now(Call::new(&["forge", "list", "--until", "connected:4", "--timeout", "30s"])),
        walk.add_key(),
        Act::Now(Call::new(&["account", "list", "--until", "usable", "--timeout", "10s", "--json"])),
        Act::Now(Call::new(&["forge", "list", "--json"])),
        Act::After(Box::new(|seen| {
            assert_ready(seen);
            Call::new(&[
                "chat",
                "start",
                "--project",
                &adopted_project(seen),
                "--key",
                "walk-chat",
                "Say hello",
                "--json",
            ])
        })),
        Act::After(Box::new(|seen| {
            Call::new(&["task", "show", &chat_task(seen), "--until", "replied", "--timeout", "10s", "--json"])
        })),
        Act::After(Box::new(|seen| Call::new(&["chat", "show", &chat_task(seen), "--json"]))),
    ]);
    assert_reply(&seen);
    walk.scratch.remove();
}
