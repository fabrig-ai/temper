//! Whole command lists from deployment documents, including dormant groups.
use super::demo;
use crate::steps;
use jig_protocol_config as config;
use serde_json::{Value, json};
use temper_forge_setup::{Group, Kind, Stage, Step, Steps, problem, setup::Setup};

fn document(entries: Value, with_demo: bool) -> Value {
    let mut value = demo::fixture();
    value["application"]["setup"] = entries;
    if !with_demo {
        value["application"]["demo"] = Value::Null;
    }
    value
}

fn command(name: &str, arguments: &[&str], kind: Kind) -> Step {
    Step::new(name.as_bytes().into(), arguments.iter().map(|word| Box::from(word.as_bytes())).collect(), kind)
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table compares every command, lifetime and group for whole documents")]
fn documents_give_the_box_its_whole_ordered_steps() {
    let fake = demo::fixture()["application"]["setup"][0].clone();
    let owned: Value = serde_json::from_slice(include_bytes!("../../../../docs/examples/owned.json")).unwrap();
    let mut owned = owned["application"]["setup"][0].clone();
    owned["forge"] = json!(2);
    owned["directory"] = json!("forge-2");
    owned["mode"]["owned"]["listen"]["socket"] = json!("127.0.0.1:3002");
    let mut without_runner = owned.clone();
    without_runner["mode"]["owned"]["runner"] = Value::Null;
    let mut asked = owned.clone();
    asked["start"] = json!("asked");
    let mut second_fake = fake.clone();
    second_fake["forge"] = json!(2);
    second_fake["directory"] = json!("forge-2");
    second_fake["mode"]["fake"]["listen"]["socket"] = json!("127.0.0.1:3002");
    let mut managed = fake.clone();
    managed["orgs"] = json!(["acme"]);
    managed["mode"] = json!({"managed":{"admin_token":"admin_token"}});

    let llm = command("fake llm", &["fake", "llm", "--config", "C", "--data", "D", "--cache", "K"], Kind::Long);
    let llm = llm.listening(Box::new([demo::fixture()["application"]["demo"]["llm"]["listen"]["socket"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()]));
    let prepare1 =
        command("prepare 1", &["forge", "prepare", "--config", "C", "--data", "D", "--cache", "K", "1"], Kind::Once);
    let fake1 =
        command("fake forge 1", &["fake", "forge", "--config", "C", "--data", "D", "--cache", "K", "1"], Kind::Long);
    let fake1 =
        fake1.listening(Box::new([fake["mode"]["fake"]["listen"]["socket"].as_str().unwrap().parse().unwrap()]));
    let prepare2 =
        command("prepare 2", &["forge", "prepare", "--config", "C", "--data", "D", "--cache", "K", "2"], Kind::Once);
    let fake2 =
        command("fake forge 2", &["fake", "forge", "--config", "C", "--data", "D", "--cache", "K", "2"], Kind::Long);
    let fake2 = fake2.listening(Box::new(["127.0.0.1:3002".parse().unwrap()]));
    let forgejo2 = command(
        "forgejo 2",
        &["forge", "run", "--config", "C", "--data", "D", "--cache", "K", "2", "forgejo"],
        Kind::Long,
    )
    .with_ready("127.0.0.1:3002".parse().unwrap());
    let runner2 = command(
        "runner 2",
        &["forge", "run", "--config", "C", "--data", "D", "--cache", "K", "2", "runner"],
        Kind::Long,
    );
    let reconcile1 = command(
        "reconcile 1",
        &["forge", "reconcile", "--config", "C", "--data", "D", "--cache", "K", "1"],
        Kind::Once,
    );
    let serve = command("serve", &["serve", "--config", "C", "--data", "D", "--cache", "K", "--box"], Kind::Long);
    let serve_listeners = ["127.0.0.1:8367".parse().unwrap(), "127.0.0.1:8371".parse().unwrap()];
    let serve = serve.listening(Box::new(serve_listeners));
    let demo_group = Group {
        forge: 1,
        recorded: false,
        before: 0,
        steps: Box::new([(Stage::Run, llm), (Stage::Prepare, prepare1.clone()), (Stage::Run, fake1.clone())]),
    };
    let rows = [
        ("empty", document(json!([]), false), Steps { always: Box::new([serve.clone()]), asked: Box::new([]) }),
        (
            "fake",
            document(json!([fake]), false),
            Steps { always: Box::new([prepare1.clone(), fake1.clone(), serve.clone()]), asked: Box::new([]) },
        ),
        (
            "demo",
            document(json!([fake]), true),
            Steps { always: Box::new([serve.clone()]), asked: Box::new([demo_group.clone()]) },
        ),
        (
            "owned with runner",
            document(json!([fake, owned]), true),
            Steps {
                always: Box::new([prepare2.clone(), forgejo2.clone(), runner2.clone(), serve.clone()]),
                asked: Box::new([demo_group.clone()]),
            },
        ),
        (
            "owned without runner",
            document(json!([fake, without_runner]), true),
            Steps {
                always: Box::new([prepare2.clone(), forgejo2.clone(), serve.clone()]),
                asked: Box::new([demo_group.clone()]),
            },
        ),
        (
            "asked",
            document(json!([fake, asked]), true),
            Steps {
                always: Box::new([serve.clone()]),
                asked: Box::new([
                    demo_group.clone(),
                    Group {
                        forge: 2,
                        recorded: true,
                        before: 0,
                        steps: Box::new([
                            (Stage::Prepare, prepare2.clone()),
                            (Stage::Run, forgejo2.clone()),
                            (Stage::Run, runner2.clone()),
                        ]),
                    },
                ]),
            },
        ),
        (
            "asked after an unasked fake",
            document(json!([fake, asked]), false),
            Steps {
                always: Box::new([prepare1.clone(), fake1.clone(), serve.clone()]),
                asked: Box::new([Group {
                    forge: 2,
                    recorded: true,
                    before: 2,
                    steps: Box::new([
                        (Stage::Prepare, prepare2.clone()),
                        (Stage::Run, forgejo2),
                        (Stage::Run, runner2),
                    ]),
                }]),
            },
        ),
        (
            "two fakes in document order",
            document(json!([second_fake, fake]), false),
            Steps {
                always: Box::new([prepare2.clone(), fake2.clone(), prepare1, fake1, serve.clone()]),
                asked: Box::new([]),
            },
        ),
        (
            "another fake with the demo",
            document(json!([second_fake, fake]), true),
            Steps { always: Box::new([prepare2, fake2, serve.clone()]), asked: Box::new([demo_group]) },
        ),
        (
            "managed unchanged",
            document(json!([managed]), false),
            Steps { always: Box::new([reconcile1, serve]), asked: Box::new([]) },
        ),
    ];
    let flags: Vec<Box<[u8]>> =
        ["--config", "C", "--data", "D", "--cache", "K"].iter().map(|flag| Box::from(flag.as_bytes())).collect();
    for (name, value, expected) in rows {
        let bytes = serde_json::to_vec(&value).unwrap();
        let d =
            config::document(&bytes, &config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 }).unwrap();
        let app = config::read::Object::new(&d, 0).unwrap().member(&d, b"application").unwrap();
        let at = config::read::Object::new(&d, app).unwrap().member(&d, b"setup").unwrap();
        let directories = config::Directories::given(b"/config", b"/data", b"/cache").unwrap();
        let mut ps = problem::Problems::empty();
        let setup = Setup::read(&d, at, &temper_forge_setup::limits::small(), &directories, &mut ps)
            .unwrap_or_else(|| panic!("{name}: {}", String::from_utf8_lossy(&problem::lines(&ps))));
        let demo = demo::read(&value).unwrap();
        assert_eq!(steps(&setup, demo.as_ref(), &flags, Box::new(serve_listeners)), expected, "{name}");
    }
}
