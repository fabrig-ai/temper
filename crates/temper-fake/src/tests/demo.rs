//! The deployment member against facts from the real document readers.
use crate::{demo, llm};
use jig_protocol_config::{self as config, Path};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{self, Finish, Message, Part, Role};
use temper_forge_setup::{problem, setup::Setup};

type Row = (Vec<(&'static str, Option<Value>)>, &'static str);

pub(super) fn fixture() -> Value {
    let mut d: Value = serde_json::from_slice(include_bytes!("../../../../docs/examples/owned.json")).unwrap();
    d["engine"]["llm"]["endpoints"].as_array_mut().unwrap().push(json!({
        "name":"demo", "number":3, "dialect":3, "provider":"demo",
        "address":{"socket":"127.0.0.1:8370"}, "transport":"plaintext", "headers":[],
        "connect_ms":10000, "handshake_ms":10000
    }));
    d["engine"]["llm"]["models"].as_array_mut().unwrap().push(json!({
        "name":"demo", "endpoint":"demo", "window":4096, "output":1024,
        "prices":{"input":1,"cached":1,"output":1,"unit":1}
    }));
    d["engine"]["llm"]["providers"].as_array_mut().unwrap().push(json!({
        "name":"demo", "number":3, "models":["demo"], "sources":{"keys":true}
    }));
    d["application"]["setup"][0]["mode"] = json!({"fake":{
        "listen":{"socket":"127.0.0.1:3000"},"git":"demo/bin/git", "owner":"playground",
        "repositories":[{"name":"widget","landing":"main"}]
    }});
    d["application"]["demo"] = json!({"llm":{
        "listen":{"socket":"127.0.0.1:8370"}, "provider":3, "key":"demo/llm.key",
        "asked":[{"cue":"Add CSV export to widget","turns":["Export CSV",{
            "lines":[{"text":"Done"}],"finish":"stop","tokens":1
        }]}],"otherwise":["demo model"]}, "forge":1, "project":1});
    d
}
fn member(d: &skein_json::Document, at: u32, name: &[u8]) -> u32 {
    config::read::Object::new(d, at).unwrap().member(d, name).unwrap()
}
pub(super) fn read(value: &Value) -> Result<Option<demo::Demo>, crate::Refusal> {
    let bytes = serde_json::to_vec(value).unwrap();
    let d = config::document(&bytes, &config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 }).unwrap();
    let directories = config::Directories::given(b"/config", b"/data", b"/cache").unwrap();
    let engine = temper_service::startup::engine(&d, member(&d, 0, b"engine"), &directories).unwrap();
    let app = member(&d, 0, b"application");
    let mut ps = problem::Problems::empty();
    let setup = Setup::read(&d, member(&d, app, b"setup"), &temper_forge_setup::limits::small(), &directories, &mut ps)
        .unwrap();
    assert!(ps.is_empty(), "{:?}", problem::lines(&ps));
    let mut ps = config::Problems::empty();
    let listeners: Vec<_> = [b"clients".as_slice(), b"webhooks"]
        .iter()
        .map(|name| {
            let at = member(&d, member(&d, app, name), b"listener");
            match config::address::read(&d, at, &Path::root(), &mut ps).unwrap() {
                config::Address::Addr(a) => a.socket().unwrap(),
                config::Address::Host { .. } => panic!("fixture socket"),
            }
        })
        .collect();
    assert!(ps.is_empty());
    let projects: Vec<_> = engine.projects.iter().map(|p| p.number).collect();
    demo::read(
        &d,
        member(&d, app, b"demo"),
        &demo::Facts {
            llm: &engine.llm,
            projects: &projects,
            setup: &setup,
            listeners: &listeners,
            directories: &directories,
        },
    )
}
#[test]
#[expect(clippy::too_many_lines, reason = "one refusal table covers the member and its document relationships")]
fn demo_refusals() {
    let owned: Value = serde_json::from_slice(include_bytes!("../../../../docs/examples/owned.json")).unwrap();
    let sign_in = json!({"issuer":"https://issuer.example", "client":"client", "scopes":[],
        "authorization":"https://issuer.example/auth", "token":"https://issuer.example/token",
        "keys":"https://issuer.example/keys", "transport":"tls", "server_name":"issuer.example",
        "must_grant":[], "redirect_path":"callback"});
    let scripts = json!([{"cue":"x","turns":["x"]}]);
    // Each row edits a fresh document before either real parser sees it.
    let mut rows: Vec<Row> = vec![
        (vec![("/application/demo/extra", Some(json!(1)))], "application.demo.extra: unknown member\n"),
        (vec![("/application/demo/llm/extra", Some(json!(1)))], "application.demo.llm.extra: unknown member\n"),
        (vec![("/application/demo/client", Some(json!("demo/client")))], "application.demo.client: unknown member\n"),
        (vec![("/application/demo", Some(json!([])))], "application.demo: wrong value kind\n"),
        (
            vec![("/application/demo/llm/provider", Some(json!("3")))],
            "application.demo.llm.provider: wrong value kind\n",
        ),
        (
            vec![("/application/demo/llm/listen/socket", Some(json!("192.0.2.1:8370")))],
            "application.demo.llm.listen: must be a loopback socket address\n",
        ),
        (
            vec![("/application/demo/llm/listen/socket", Some(json!("127.0.0.1:7100")))],
            "application.demo.llm.listen: must be unique\n",
        ),
        (
            vec![("/application/demo/llm/listen/socket", Some(json!("127.0.0.1:7101")))],
            "application.demo.llm.listen: must be unique\n",
        ),
        (
            vec![("/application/demo/llm/listen/socket", Some(json!("127.0.0.1:3000")))],
            "application.demo.llm.listen: must be unique\n",
        ),
        (
            vec![
                ("/application/setup/0", Some(owned["application"]["setup"][0].clone())),
                ("/application/demo/llm/listen/socket", Some(json!("127.0.0.1:3000"))),
            ],
            "application.demo.llm.listen: must be unique\n",
        ),
        (
            vec![("/application/demo/llm/provider", Some(json!(9)))],
            "application.demo.llm.provider: names no LLM provider\n",
        ),
        (
            vec![
                ("/engine/llm/providers/1/sources/sign_in", Some(sign_in.clone())),
                ("/engine/llm/providers/1/signed_in_plan", Some(json!(false))),
            ],
            "application.demo.llm.provider: must offer a key alone\n",
        ),
        (
            vec![
                ("/engine/llm/providers/1/sources/programs", Some(json!(["claude_code"]))),
                ("/engine/llm/providers/1/manage_usage", Some(json!("https://example.test/usage"))),
            ],
            "application.demo.llm.provider: must offer a key alone\n",
        ),
        (
            vec![("/engine/llm/providers/1/sources/keys", Some(json!(false)))],
            "application.demo.llm.provider: must offer a key alone\n",
        ),
        (
            vec![
                ("/engine/llm/providers/1/models", Some(json!([]))),
                ("/engine/llm/models", Some(json!([owned["engine"]["llm"]["models"][0].clone()]))),
            ],
            "application.demo.llm.provider: names a provider with no models\n",
        ),
        (
            vec![("/engine/llm/endpoints/1/address/socket", Some(json!("127.0.0.1:8371")))],
            "application.demo.llm.provider: names an endpoint not at llm.listen\n",
        ),
        (
            vec![
                ("/engine/llm/endpoints/1/transport", Some(json!("tls"))),
                ("/engine/llm/endpoints/1/server_name", Some(json!("localhost"))),
            ],
            "application.demo.llm.provider: names an endpoint with TLS\n",
        ),
        (
            vec![("/engine/llm/endpoints/1/dialect", Some(json!(1)))],
            "application.demo.llm.provider: names an endpoint not in OpenAI's dialect\n",
        ),
        (vec![("/engine/llm/models/1/window", Some(json!(0)))], "application.demo.llm.provider: must be at least 1\n"),
        (vec![("/engine/llm/models/1/output", Some(json!(0)))], "application.demo.llm.provider: must be at least 1\n"),
        (
            vec![("/engine/llm/models/1/output", Some(json!(4097)))],
            "application.demo.llm.provider: must not exceed window\n",
        ),
        (
            vec![("/application/demo/llm/key", Some(json!("")))],
            "application.demo.llm.key: relative path must be non-empty with no .. component\n",
        ),
        (
            vec![("/application/demo/llm/key", Some(json!("k".repeat(4097))))],
            "application.demo.llm.key: past its bound\n",
        ),
        (
            vec![("/application/demo/llm/asked", Some(json!((0..9).map(|_| scripts[0].clone()).collect::<Vec<_>>())))],
            "application.demo.llm.asked: past its bound\n",
        ),
        (
            vec![("/application/demo/llm/asked", Some(json!([scripts[0].clone(), scripts[0].clone()])))],
            "application.demo.llm.asked[1].cue: must be unique\n",
        ),
        (
            vec![("/application/demo/llm/otherwise", Some(json!([])))],
            "application.demo.llm.otherwise: must be non-empty\n",
        ),
        (
            vec![(
                "/application/demo/llm/asked",
                Some(json!(
                    (0..8)
                        .map(
                            |i| json!({"cue":i.to_string(),"turns":(0..8).map(|_|"x".repeat(4024)).collect::<Vec<_>>()})
                        )
                        .collect::<Vec<_>>()
                )),
            )],
            "application.demo.llm.otherwise: scripts past their joint byte bound\n",
        ),
        (vec![("/application/demo/forge", Some(json!(2)))], "application.demo.forge: names no fake setup entry\n"),
        (
            vec![("/application/setup/0", Some(owned["application"]["setup"][0].clone()))],
            "application.demo.forge: names no fake setup entry\n",
        ),
        (vec![("/application/demo/project", Some(json!(9)))], "application.demo.project: names no project\n"),
    ];
    rows.push((
        vec![
            ("/engine/llm/providers/1/sources/sign_in", Some(sign_in)),
            ("/engine/llm/providers/1/signed_in_plan", Some(json!(true))),
            ("/engine/llm/providers/1/manage_usage", Some(json!("https://example.test/usage"))),
        ],
        "application.demo.llm.provider: must offer a key alone\n",
    ));
    rows.push((
        vec![("/application/demo/llm/key", Some(json!("demo/\u{0}")))],
        "application.demo.llm.key: must be an absolute path without NUL\n",
    ));
    let mut models = vec![owned["engine"]["llm"]["models"][0].clone()];
    models.extend((0..9).map(|i| {
        json!({"name":format!("demo-{i}"), "endpoint":"demo", "window":4096,"output":1024,
        "prices":{"input":1,"cached":1,"output":1,"unit":1}})
    }));
    rows.push((
        vec![
            ("/engine/llm/models", Some(json!(models))),
            ("/engine/llm/providers/1/models", Some(json!((0..9).map(|i| format!("demo-{i}")).collect::<Vec<_>>()))),
        ],
        "application.demo.llm.provider: past its bound\n",
    ));
    for (changes, expected) in rows {
        let mut d = fixture();
        for (path, value) in &changes {
            let (parent, field) = path.rsplit_once('/').unwrap();
            let parent = d.pointer_mut(parent).unwrap();
            match value {
                Some(v) => {
                    if let Some(o) = parent.as_object_mut() {
                        o.insert(field.into(), v.clone());
                    } else {
                        parent[field.parse::<usize>().unwrap()] = v.clone();
                    }
                }
                None => {
                    parent.as_object_mut().unwrap().remove(field);
                }
            }
        }
        assert_eq!(read(&d).unwrap_err().line().as_ref(), expected.as_bytes(), "{changes:?}");
    }
}
#[test]
fn demo_acceptance() {
    for (enabled, cued, window, output) in
        [(false, false, 4096, 1024), (true, false, 4096, 1024), (true, true, 4096, 1024), (true, true, 65536, 32768)]
    {
        let mut value = fixture();
        value["engine"]["llm"]["models"][1]["window"] = json!(window);
        value["engine"]["llm"]["models"][1]["output"] = json!(output);
        if cued {
            value["application"]["demo"]["llm"]["cued"] = json!([
                {"cue":"coding brief","turns":["coding answer"]}
            ]);
        }
        if !enabled {
            value["application"]["demo"] = Value::Null;
        }
        let demo = read(&value).unwrap();
        if !enabled {
            assert!(demo.is_none());
            continue;
        }
        let demo = demo.unwrap();
        let m = &value["application"]["demo"]["llm"];
        let combined: Vec<_> = m["asked"]
            .as_array()
            .unwrap()
            .iter()
            .chain(m.get("cued").and_then(Value::as_array).into_iter().flatten())
            .cloned()
            .collect();
        let standalone = json!({"listen":m["listen"], "key":"/data/demo/llm.key", "tls":null,
            "models":[{"name":"demo","window":window,"output":output}], "cued":combined,
            "asked":[], "otherwise":m["otherwise"]});
        assert_eq!(demo.llm(), &llm::Document::decode(&serde_json::to_vec(&standalone).unwrap()).unwrap());
        assert_eq!(demo.provider(), 3);
        assert_eq!(demo.forge(), 1);
        assert_eq!(demo.project(), 1);
    }
}

#[test]
fn cued_scripts_follow_asked_and_play_from_the_brief() {
    let mut value = fixture();
    value["application"]["demo"]["llm"]["cued"] = json!([
        {"cue":"coding brief","turns":["coding answer"]}
    ]);
    let demo = read(&value).unwrap().unwrap();
    let m = &value["application"]["demo"]["llm"];
    let standalone = json!({"listen":m["listen"], "key":"/data/demo/llm.key", "tls":null,
        "models":[{"name":"demo","window":4096,"output":1024}],
        "cued":[m["asked"][0],m["cued"][0]],"asked":[],"otherwise":m["otherwise"]});
    assert_eq!(demo.llm(), &llm::Document::decode(&serde_json::to_vec(&standalone).unwrap()).unwrap());
    let answer = super::answer(demo.llm(), 23, super::query(b"coding brief", b"Begin"));
    assert_eq!(answer.parts.as_ref(), &[Part::Text { text: b"coding answer".as_slice().into() }]);
}

#[test]
fn cued_refusals_and_shared_bounds() {
    let scripts: Vec<_> = (0..8).map(|i| json!({"cue":i.to_string(),"turns":["x"]})).collect();
    for (cued, expected) in [
        (json!(null), "application.demo.llm.cued: wrong value kind\n"),
        (json!({}), "application.demo.llm.cued: wrong value kind\n"),
        (json!([{"cue":"","turns":["x"]}]), "application.demo.llm.cued[0].cue: must be non-empty\n"),
        (json!([{"cue":"x".repeat(257),"turns":["x"]}]), "application.demo.llm.cued[0].cue: past its bound\n"),
        (json!([{"cue":"x","turns":[]}]), "application.demo.llm.cued[0].turns: must be non-empty\n"),
        (json!([{"cue":"x","turns":vec!["x";9]}]), "application.demo.llm.cued[0].turns: past its bound\n"),
        (
            json!([{"cue":"Add CSV export to widget","turns":["x"]}]),
            "application.demo.llm.cued[0].cue: must be unique\n",
        ),
        (json!([scripts[0], scripts[0]]), "application.demo.llm.cued[1].cue: must be unique\n"),
        (
            json!((0..7).map(|i| json!({"cue":i.to_string(),"turns":vec!["x".repeat(5000);8]})).collect::<Vec<_>>()),
            "application.demo.llm.otherwise: scripts past their joint byte bound\n",
        ),
        (json!(scripts), "application.demo.llm.cued: past its bound\n"),
        (json!(vec![scripts[0].clone(); 9]), "application.demo.llm.cued: past its bound\n"),
    ] {
        let mut value = fixture();
        value["application"]["demo"]["llm"]["cued"] = cued;
        assert_eq!(read(&value).unwrap_err().line().as_ref(), expected.as_bytes());
    }
    for cued in [json!([]), json!(&scripts[..7]), json!([{"cue":"x".repeat(256),"turns":vec!["x";8]}])] {
        let mut value = fixture();
        value["application"]["demo"]["llm"]["cued"] = cued;
        assert!(read(&value).unwrap().is_some());
    }
    let mut value = fixture();
    value["application"]["demo"]["llm"]["asked"] = json!([]);
    value["application"]["demo"]["llm"]["cued"] = json!(scripts);
    assert!(read(&value).unwrap().is_some());
}

#[test]
fn key_file() {
    let hex = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let bound = usize::try_from(demo::KEY_FILE_BYTES - 1).unwrap();
    // The engine's Keyed.value is a secret, not text: every byte is admitted.
    let bytes: Vec<u8> = (0..=255).collect();
    for (file, expected) in [
        ([hex.as_slice(), b"\n"].concat(), Ok(hex.as_slice().to_vec())),
        (hex.to_vec(), Ok(hex.to_vec())),
        ([hex.as_slice(), b"\n\n"].concat(), Ok([hex.as_slice(), b"\n"].concat())),
        (b"\n".to_vec(), Err("holds no key")),
        (vec![], Err("holds no key")),
        (vec![b'k'; bound + 1], Err("key past 4096 bytes")),
        ([vec![b'k'; bound + 1], vec![b'\n']].concat(), Err("key past 4096 bytes")),
        (vec![b'k'; bound], Ok(vec![b'k'; bound])),
        ([vec![b'k'; bound], vec![b'\n']].concat(), Ok(vec![b'k'; bound])),
        (bytes.clone(), Ok(bytes)),
    ] {
        assert_eq!(demo::key(&file).map(<[u8]>::to_vec).map_err(demo::KeyRefused::reason), expected);
    }
}

#[test]
fn demo_answers_the_brief() {
    let mut value = fixture();
    // These fixtures are the fake's own and need not be the template's.
    value["application"]["demo"]["llm"]["asked"] = json!([
        {"cue":"Add CSV export to widget","turns":[{
            "lines":[{"text":"Export CSV"},{"call":"finish","arguments":{
                "form":"report","text":"The playground changes nothing.","fields":{}
            }}],"finish":"tool_calls","tokens":3
        }]},
        {"cue":"Add a dark theme to widget","turns":[{
            "lines":[{"text":"Use dark colours"},{"call":"finish","arguments":{
                "form":"report","text":"The playground changes nothing.","fields":{}
            }}],"finish":"tool_calls","tokens":4
        }]}
    ]);
    let demo = read(&value).unwrap().unwrap();
    let csv = "Help the person plan their work.\n\nWhen a task, not a person, requested you, you are a goal doing work your requester agreed to plan first: before anything is changed, send your requester your plan with message, as a question asking whether to go ahead, and wait for the answer. Then call change with what was agreed, wait for its result, and report what landed as its result says it, naming no check or test it doesn't.\n\nOtherwise you are the person's chat. When they ask for a change to a repository, call change with what to change, then tell them what landed. When they want to agree a plan before anything is changed, never call change yourself: call goal with their words, and tell them a question will wait in their inbox. When they answer it, send their answer to the goal with message, as the answer to that question's number, and tell them you did. When the goal reports, tell them what landed. Explain each result by what it says: never say a check or test ran that it doesn't name.\n\n## Task\n\nAdd CSV export to widget\n[Report: at most 32 bytes]\n[Requested by person 1]\n\n";
    let dark = "Help the person plan their work.\n\nWhen a task, not a person, requested you, you are a goal doing work your requester agreed to plan first: before anything is changed, send your requester your plan with message, as a question asking whether to go ahead, and wait for the answer. Then call change with what was agreed, wait for its result, and report what landed as its result says it, naming no check or test it doesn't.\n\nOtherwise you are the person's chat. When they ask for a change to a repository, call change with what to change, then tell them what landed. When they want to agree a plan before anything is changed, never call change yourself: call goal with their words, and tell them a question will wait in their inbox. When they answer it, send their answer to the goal with message, as the answer to that question's number, and tell them you did. When the goal reports, tell them what landed. Explain each result by what it says: never say a check or test ran that it doesn't name.\n\n## Task\n\nAdd a dark theme to widget\n[Report: at most 32 bytes]\n[Requested by person 1]\n\n";
    let opening = "Begin the work your brief describes.";
    for (name, system, user, said, text, tokens, finish) in [
        ("CSV brief", csv, opening, false, "Export CSV", 3, Finish::ToolCalls),
        ("dark brief", dark, opening, false, "Use dark colours", 4, Finish::ToolCalls),
        ("user cue only", "No suggested request", "Add CSV export to widget", false, "demo model", 3, Finish::Stop),
        ("past last turn", csv, opening, true, "done", 1, Finish::Stop),
        (
            "earliest cue",
            "Add a dark theme to widget; Add CSV export to widget",
            opening,
            false,
            "Use dark colours",
            4,
            Finish::ToolCalls,
        ),
    ] {
        let mut query = super::query(system.as_bytes(), user.as_bytes());
        if said {
            query.messages = Box::new([
                Message { role: Role::User, parts: Box::new([Part::Text { text: opening.as_bytes().into() }]) },
                Message {
                    role: Role::Assistant,
                    parts: Box::new([Part::Text { text: b"Earlier answer".as_slice().into() }]),
                },
                Message { role: Role::User, parts: Box::new([Part::Text { text: user.as_bytes().into() }]) },
            ]);
        }
        let answer = super::answer(demo.llm(), 23, query);
        let mut parts = vec![Part::Text { text: text.as_bytes().into() }];
        if finish == Finish::ToolCalls {
            parts.push(Part::ToolCall {
                id: b"call_0000000000000001".as_slice().into(),
                name: b"finish".as_slice().into(),
                arguments:
                    br#"{"fields":{},"form":"report","text":"The playground changes nothing."}"#.as_slice().into(),
            });
        }
        assert_eq!(answer.parts.as_ref(), parts.as_slice(), "{name}");
        assert_eq!(answer.finish, finish, "{name}");
        assert_eq!(answer.usage.output, Some(tokens), "{name}");
    }
}

fn template() -> Value {
    let template: Value = serde_json::from_slice(include_bytes!("../../../temper-init/template/temper.json")).unwrap();
    let mut value = fixture();
    value["engine"]["llm"]["models"][1] = template["engine"]["llm"]["models"][1].clone();
    value["application"]["demo"] = template["application"]["demo"].clone();
    value["application"]["demo"]["llm"]["listen"]["socket"] = json!("127.0.0.1:8370");
    value
}

const CHAT: &str = "Help the person plan their work.\n\nWhen a task, not a person, requested you, you are a goal doing work your requester agreed to plan first: before anything is changed, send your requester your plan with message, as a question asking whether to go ahead, and wait for the answer. Then call change with what was agreed, wait for its result, and report what landed as its result says it, naming no check or test it doesn't.\n\nOtherwise you are the person's chat. When they ask for a change to a repository, call change with what to change, then tell them what landed. When they want to agree a plan before anything is changed, never call change yourself: call goal with their words, and tell them a question will wait in their inbox. When they answer it, send their answer to the goal with message, as the answer to that question's number, and tell them you did. When the goal reports, tell them what landed. Explain each result by what it says: never say a check or test ran that it doesn't name.\n\n## Task\n\nAdd CSV export to widget\n[Report: at most 32 bytes]\n[Requested by person 1]\n\n";
const RESUMED_CHAT: &str = "Help the person plan their work.\n\nWhen a task, not a person, requested you, you are a goal doing work your requester agreed to plan first: before anything is changed, send your requester your plan with message, as a question asking whether to go ahead, and wait for the answer. Then call change with what was agreed, wait for its result, and report what landed as its result says it, naming no check or test it doesn't.\n\nOtherwise you are the person's chat. When they ask for a change to a repository, call change with what to change, then tell them what landed. When they want to agree a plan before anything is changed, never call change yourself: call goal with their words, and tell them a question will wait in their inbox. When they answer it, send their answer to the goal with message, as the answer to that question's number, and tell them you did. When the goal reports, tell them what landed. Explain each result by what it says: never say a check or test ran that it doesn't name.\n\n## Task\n\nAdd CSV export to widget\n[Report: at most 32 bytes]\n[Requested by person 1]\n\n## Plan\n\ndelegate 2: ended, spec revision 1\n";
const CODING: &str = "Read the repository and make the requested change in its files, then read back what you wrote and finish with a clear change summary. You can run nothing, and the change has no pull request yet: once you finish, your work is committed, its pull request opened and its checks run; what fails there, or needs the pull request's number, comes back as a repair that names it.\n\n## Task\n\nExport widget's table as CSV\n[Change connector 1, kind 2, at most 4096 bytes]\n[Requested by task 2]\n\n";

#[test]
fn the_templates_demo_fits_the_fakes_bounds() {
    assert!(read(&template()).unwrap().is_some());
}

fn answers(system: &str, resumed: &str, count: usize) -> Vec<api::Answer> {
    let demo = read(&template()).unwrap().unwrap();
    let mut query = super::query(system.as_bytes(), b"Begin the work your brief describes.");
    query.model = b"demo".as_slice().into();
    query.max_tokens = 8192;
    let mut answers = Vec::new();
    for index in 0..count {
        if index == 2 {
            query.system = resumed.as_bytes().into();
        }
        let answer = super::answer(demo.llm(), 23, query.clone());
        let mut messages = query.messages.into_vec();
        messages.push(Message { role: Role::Assistant, parts: answer.parts.clone() });
        for part in &answer.parts {
            if let Part::ToolCall { id, .. } = part {
                messages.push(Message {
                    role: Role::User,
                    parts: Box::new([Part::ToolOutput {
                        id: id.clone(),
                        output: b"Completed".as_slice().into(),
                        is_error: false,
                    }]),
                });
            }
        }
        query.messages = messages.into_boxed_slice();
        answers.push(answer);
    }
    answers
}

fn call(answer: &api::Answer, name: &[u8]) -> Value {
    assert_eq!(answer.finish, Finish::ToolCalls);
    let Some(Part::ToolCall { id, name: called, arguments }) = answer.parts.last() else { panic!("tool call") };
    assert_eq!(id.as_ref(), b"call_0000000000000001");
    assert_eq!(called.as_ref(), name);
    serde_json::from_slice(arguments).unwrap()
}

#[test]
fn the_csv_chat_delegates_waits_and_reports_landing_before_waiting_again() {
    let answers = answers(CHAT, RESUMED_CHAT, 3);
    assert_eq!(answers[0].parts.len(), 2);
    assert_eq!(answers[0].parts[0], Part::Text { text: "I’ll delegate this change in playground/widget. It will push a branch, open a pull request, check CI and land into main on Playground forge.".as_bytes().into() });
    let delegate = call(&answers[0], b"delegate");
    assert_eq!(delegate["batch"][0]["executor"], json!({"kind":"procedure","connector":1,"code":2}));
    assert_eq!(
        delegate["batch"][0]["authority"]["delegation"]["kinds"],
        json!([{"kind":"agent","number":2}, {"kind":"agent","number":4}])
    );
    assert_eq!(delegate["batch"][0]["spec"]["words"], "Export widget's table as CSV");
    assert_eq!(answers[1].parts.len(), 1);
    assert_eq!(call(&answers[1], b"wait"), json!({}));
    assert_eq!(answers[2].parts.len(), 2);
    assert_eq!(answers[2].parts[0], Part::Text { text: "CSV export is available from widget’s table. The change landed on Playground forge. This was a scripted change.".as_bytes().into() });
    assert_eq!(call(&answers[2], b"wait"), json!({}));
    assert_eq!(answers.iter().map(|a| a.usage.output).collect::<Vec<_>>(), [Some(269), Some(2), Some(30)]);
}

#[test]
fn the_csv_chat_answers_done_past_its_last_turn() {
    let answers = answers(CHAT, RESUMED_CHAT, 4);
    assert_eq!(answers[3].parts.as_ref(), &[Part::Text { text: b"done".as_slice().into() }]);
    assert_eq!(answers[3].finish, Finish::Stop);
    assert_eq!(answers[3].usage.output, Some(1));
}

#[test]
fn the_csv_coding_agent_reads_edits_and_finishes_the_change() {
    let answers = answers(CODING, CODING, 3);
    for answer in &answers {
        assert_eq!(answer.parts.len(), 1);
    }
    assert_eq!(call(&answers[0], b"read"), json!({"path":"src/main.rs"}));
    let edit = call(&answers[1], b"edit");
    assert_eq!(edit["path"], "src/main.rs");
    assert_eq!(edit["edits"].as_array().unwrap().len(), 1);
    assert!(edit["edits"][0]["old"].as_str().unwrap().ends_with("}\n"));
    assert!(edit["edits"][0]["new"].as_str().unwrap().contains("println!(\"name,count\");"));
    assert_eq!(
        call(&answers[2], b"finish"),
        json!({"form":"change","fields":{
            "title":"Add CSV export to widget",
            "body":"widget --csv prints the table as CSV: a header line, then a line for each row. This was a scripted change."
        }})
    );
    assert_eq!(answers.iter().map(|a| a.usage.output).collect::<Vec<_>>(), [Some(7), Some(116), Some(47)]);
}

#[test]
fn the_csv_cues_select_only_their_own_agents_briefs() {
    let value = template();
    for (list, own, other, tool) in
        [("asked", CHAT, CODING, b"delegate".as_slice()), ("cued", CODING, RESUMED_CHAT, b"read")]
    {
        let cue = value["application"]["demo"]["llm"][list][0]["cue"].as_str().unwrap();
        assert!(own.contains(cue));
        assert!(!other.contains(cue));
        let mut only = value.clone();
        only["application"]["demo"]["llm"]["asked"] = json!([]);
        only["application"]["demo"]["llm"]["cued"] = json!([]);
        only["application"]["demo"]["llm"][list] = json!([value["application"]["demo"]["llm"][list][0]]);
        let demo = read(&only).unwrap().unwrap();
        for (system, selected) in [(own, true), (other, false)] {
            let mut query = super::query(system.as_bytes(), b"Begin the work your brief describes.");
            query.model = b"demo".as_slice().into();
            query.max_tokens = 8192;
            let answer = super::answer(demo.llm(), 23, query);
            if selected {
                call(&answer, tool);
            } else {
                // The fallback replies in text, as an Open chat's run does.
                let [Part::Text { text }] = answer.parts.as_ref() else { panic!("the fallback's text alone") };
                assert!(text.starts_with("I am the scripted model".as_bytes()));
                assert_eq!(answer.finish, Finish::Stop);
            }
        }
    }
}

#[test]
fn the_csv_review_uses_its_verdict_script_with_the_changes_words_quoted() {
    let review = "Review the change with read-only authority.\n\n## Task\n\nReview this change at the named head\nChange words:\nExport widget's table as CSV\n[Requested by task 2]\n";
    let answers = answers(review, review, 1);
    let verdict = call(&answers[0], b"finish");
    assert_eq!(verdict["form"], "verdict");
    assert_eq!(verdict["label"], "1");
}
