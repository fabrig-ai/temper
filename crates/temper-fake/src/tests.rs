//! One table for each document and each independent behaviour.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary tests edit JSON values with serde, leaving the step crate under the root lints"
)]
#![expect(clippy::default_numeric_fallback, reason = "JSON table integers have no Rust arithmetic unit")]
use super::{
    credential::{self, credential},
    forge, llm, turn,
};
use alloc::vec::Vec;
use jig_protocol_config::Path;
use serde_json::{Value, json};
use skein_fake_llm_domain::api;
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};

fn llm_document() -> Value {
    json!({"listen":{"socket":"127.0.0.1:0"},"key":"/tmp/key","tls":null,
        "models":[{"name":"qa","window":4096,"output":1024}],"cued":[],"asked":[],"otherwise":["fallback"]})
}
fn forge_document() -> Value {
    json!({"listen":{"socket":"127.0.0.1:0"},"admin_token":"/tmp/forge/admin_token","owner":"fake",
        "repositories":[{"name":"widget","landing":"main","ci":null}]})
}
#[test]
#[expect(clippy::too_many_lines, reason = "one table names every document bound and refusal path")]
fn llm_document_checks() {
    let scripts = json!([{"cue":"hello","turns":["answer"]}]);
    let models = json!([{"name":"qa","window":4096,"output":1024}]);
    let object = json!({"lines":[{"text":"hello"}],"finish":"stop","tokens":1});
    let rows = [
        ("/listen", json!({"socket":"192.0.2.1:0"}), "listen: must be a loopback socket address\n"),
        ("/listen", json!({"host":"localhost","port":1}), "listen: must be a loopback socket address\n"),
        ("/key", json!("relative"), "key: must be an absolute path without NUL\n"),
        ("/key", json!("/tmp/\u{0}"), "key: must be an absolute path without NUL\n"),
        ("/tls", json!({"root":"relative"}), "tls.root: must be an absolute path without NUL\n"),
        ("/tls", json!({"root":"/tmp/root","extra":1}), "tls.extra: unknown member\n"),
        ("/tls", json!({}), "tls.root: missing\n"),
        ("/models", json!([]), "models: must be non-empty\n"),
        ("/models", Value::Array((0..9).map(|_| models[0].clone()).collect()), "models: past its bound\n"),
        ("/models/0/name", json!(""), "models[0].name: must be non-empty\n"),
        ("/models/0/name", json!("x".repeat(4097)), "models[0].name: past its bound\n"),
        ("/models/0/window", json!(0), "models[0].window: must be at least 1\n"),
        ("/models/0/window", json!(4_294_967_296_u64), "models[0].window: past its bound\n"),
        ("/models/0/output", json!(0), "models[0].output: must be at least 1\n"),
        ("/models/0/output", json!(4097), "models[0].output: must not exceed window\n"),
        ("/models/0", json!({"name":"qa","window":1,"output":1,"extra":0}), "models[0].extra: unknown member\n"),
        ("/models", Value::Array((0..2).map(|_| models[0].clone()).collect()), "models[1].name: must be unique\n"),
        ("/cued", Value::Array((0..9).map(|_| scripts[0].clone()).collect()), "cued: past its bound\n"),
        ("/asked", Value::Array((0..9).map(|_| scripts[0].clone()).collect()), "asked: past its bound\n"),
        ("/asked", json!([{"cue":"","turns":["x"]}]), "asked[0].cue: must be non-empty\n"),
        ("/cued", json!([{"cue":"x".repeat(257),"turns":["x"]}]), "cued[0].cue: past its bound\n"),
        ("/asked", Value::Array((0..2).map(|_| scripts[0].clone()).collect()), "asked[1].cue: must be unique\n"),
        ("/cued", json!([{"cue":"x","turns":["x"],"extra":1}]), "cued[0].extra: unknown member\n"),
        ("/asked", json!([{"cue":"x","turns":[]}]), "asked[0].turns: must be non-empty\n"),
        ("/otherwise", json!([]), "otherwise: must be non-empty\n"),
        ("/otherwise", json!(["x", "x", "x", "x", "x", "x", "x", "x", "x"]), "otherwise: past its bound\n"),
        ("/otherwise", json!([{"lines":[],"finish":"stop","tokens":1}]), "otherwise[0].lines: must be non-empty\n"),
        (
            "/otherwise",
            json!([{"lines":[{"text":""}],"finish":"stop","tokens":1}]),
            "otherwise[0].lines[0].text: must be non-empty\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"call":"bad.name","arguments":{}}],"finish":"tool_calls","tokens":1}]),
            "otherwise[0].lines[0].call: expected ASCII letters, digits, _ or -\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"call":"x","arguments":[]}],"finish":"tool_calls","tokens":1}]),
            "otherwise[0].lines[0].arguments: expected a JSON object\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x","hold":"x"}],"finish":"stop","tokens":1}]),
            "otherwise[0].lines[0].hold: unknown member\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x"}],"finish":"no","tokens":1}]),
            "otherwise[0].finish: expected stop, tool_calls, length or content_filter\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x"}],"finish":"stop","tokens":0}]),
            "otherwise[0].tokens: must be at least 1\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x"}],"finish":"stop","tokens":1,"extra":0}]),
            "otherwise[0].extra: unknown member\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"fail":"x"}],"finish":"stop","tokens":1}]),
            "otherwise[0].lines[0].fail: wrong value kind\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"opaque":"x"}],"finish":"stop","tokens":1}]),
            "otherwise[0].lines[0].opaque: unknown member\n",
        ),
        ("/key", json!("/".to_owned() + &"x".repeat(4096)), "key: past its bound\n"),
        ("/tls", json!({"root":"/".to_owned()+&"x".repeat(4096)}), "tls.root: past its bound\n"),
        ("/key", Value::Null, "key: wrong value kind\n"),
        ("/models/0/output", json!(4_294_967_296_u64), "models[0].output: past its bound\n"),
        ("/models/0/window", json!(-1), "models[0].window: past its bound\n"),
        ("/asked", json!([{"cue":"x".repeat(257),"turns":["x"]}]), "asked[0].cue: past its bound\n"),
        ("/cued", json!([{"cue":"","turns":["x"]}]), "cued[0].cue: must be non-empty\n"),
        ("/cued", Value::Array((0..2).map(|_| scripts[0].clone()).collect()), "cued[1].cue: must be unique\n"),
        ("/cued", json!([{"cue":"x","turns":[]}]), "cued[0].turns: must be non-empty\n"),
        (
            "/asked",
            json!([{"cue":"x","turns":["a","b","c","d","e","f","g","h","i"]}]),
            "asked[0].turns: past its bound\n",
        ),
        (
            "/otherwise",
            json!([{"lines":(0..9).map(|_|json!({"text":"x"})).collect::<Vec<_>>(),"finish":"stop","tokens":1}]),
            "otherwise[0].lines: past its bound\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"call":"","arguments":{}}],"finish":"tool_calls","tokens":1}]),
            "otherwise[0].lines[0].call: must be non-empty\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"call":"x".repeat(65),"arguments":{}}],"finish":"tool_calls","tokens":1}]),
            "otherwise[0].lines[0].call: past its bound\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"call":"x"}],"finish":"tool_calls","tokens":1}]),
            "otherwise[0].lines[0].arguments: missing\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x","call":"x","arguments":{}}],"finish":"stop","tokens":1}]),
            "otherwise[0].lines[0]: expected text alone, or call and arguments\n",
        ),
        (
            "/otherwise",
            json!([{"lines":[{"text":"x"}],"finish":"stop","tokens":-1}]),
            "otherwise[0].tokens: past its bound\n",
        ),
        ("/otherwise", json!(["x".repeat(65_536)]), "otherwise[0]: answer past its byte bound\n"),
        (
            "/cued",
            Value::Array(
                (0..8)
                    .map(|i| json!({"cue":i.to_string(),"turns":(0..8).map(|_|"x".repeat(4_024)).collect::<Vec<_>>()}))
                    .collect(),
            ),
            "otherwise: scripts past their joint byte bound\n",
        ),
    ];
    for (pointer, value, expected) in rows {
        let mut doc = llm_document();
        *doc.pointer_mut(pointer).unwrap() = value;
        let refused = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err();
        assert_eq!(&*refused.line(), expected.as_bytes(), "{pointer}");
    }
    for tls in [Value::Null, json!({"root":"/tmp/root"})] {
        let mut doc = llm_document();
        doc["tls"] = tls;
        doc["otherwise"] = json!([object]);
        llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    }
    let mut doc = llm_document();
    doc["extra"] = json!(1);
    assert_eq!(
        &*llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line(),
        b"extra: unknown member\n"
    );
    for (bytes, expected) in [
        (b"[]".to_vec(), b"document: wrong value kind\n".as_slice()),
        (b"{".to_vec(), b"document: invalid JSON\n"),
        (b"{}".to_vec(), b"listen: missing\n"),
        (alloc::vec![b' '; usize::try_from(super::DOCUMENT_BYTES).unwrap()+1], b"document: invalid JSON\n"),
        (br#"{"listen":{"socket":"127.0.0.1:0"},"key":"/tmp/first","key":"/tmp/second","tls":null,"models":[],"cued":[],"asked":[],"otherwise":["x"]}"#.to_vec(), b"key: duplicate\n"),
    ] { assert_eq!(llm::Document::decode(&bytes).unwrap_err().line().as_ref(), expected); }
    let mut doc = llm_document();
    let calls = json!({"lines":(0..8).map(|_|json!({"call":"x".repeat(64),"arguments":{}})).collect::<Vec<_>>(),"finish":"tool_calls","tokens":u64::MAX});
    let scripts = |prefix: &str| {
        (0..8).map(|i|json!({"cue":prefix.to_owned()+&i.to_string()+&"x".repeat(254),"turns":(0..8).map(|_|calls.clone()).collect::<Vec<_>>()})).collect::<Vec<_>>()
    };
    doc["models"] = json!(
        (0..8)
            .map(|i| json!({"name":"x".repeat(4095)+&i.to_string(),"window":u32::MAX,"output":u32::MAX}))
            .collect::<Vec<_>>()
    );
    doc["cued"] = json!(scripts("s"));
    doc["asked"] = json!(scripts("u"));
    doc["key"] = json!("/".to_owned() + &"x".repeat(4095));
    doc["tls"] = json!({"root":"/".to_owned()+&"x".repeat(4095)});
    llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table names every document bound and refusal path")]
fn forge_document_checks() {
    let rows = [
        ("/listen", json!({"socket":"0.0.0.0:0"}), "listen: must be a loopback socket address\n"),
        ("/admin_token", json!("relative"), "admin_token: must be an absolute path without NUL\n"),
        ("/owner", json!("temper-admin"), "owner: expected a Forgejo login other than temper-admin\n"),
        ("/owner", json!("bad/name"), "owner: expected a Forgejo login other than temper-admin\n"),
        ("/owner", json!("x".repeat(41)), "owner: past its bound\n"),
        ("/repositories", json!([]), "repositories: must be non-empty\n"),
        ("/repositories/0/name", json!("bad/name"), "repositories[0].name: expected a Forgejo repository name\n"),
        ("/repositories/0/name", json!("x".repeat(101)), "repositories[0].name: past its bound\n"),
        ("/repositories/0/landing", json!("bad..branch"), "repositories[0].landing: expected a branch name\n"),
        ("/repositories/0/landing", json!("x".repeat(257)), "repositories[0].landing: past its bound\n"),
        (
            "/repositories/0/ci",
            json!({"path":"../outside","green":"ok"}),
            "repositories[0].ci.path: expected a relative file path with no . or .. component\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"/outside","green":"ok"}),
            "repositories[0].ci.path: expected a relative file path with no . or .. component\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"README.md","green":"ok"}),
            "repositories[0].ci.path: must not name a seed file\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"src/main.rs","green":"ok"}),
            "repositories[0].ci.path: must not name a seed file\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":".forgejo/workflows/check.yml","green":"ok"}),
            "repositories[0].ci.path: must not name a seed file\n",
        ),
        ("/repositories/0/ci", json!({"path":"result","green":""}), "repositories[0].ci.green: must be non-empty\n"),
        (
            "/repositories/0/ci",
            json!({"path":"x".repeat(257),"green":"ok"}),
            "repositories[0].ci.path: past its bound\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"result","green":"x".repeat(8193)}),
            "repositories[0].ci.green: past its bound\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"result","green":"ok","extra":1}),
            "repositories[0].ci.extra: unknown member\n",
        ),
        (
            "/repositories/0",
            json!({"name":"widget","landing":"main","ci":null,"extra":1}),
            "repositories[0].extra: unknown member\n",
        ),
        ("/owner", json!(""), "owner: expected a Forgejo login other than temper-admin\n"),
        ("/admin_token", json!("/".to_owned() + &"x".repeat(4096)), "admin_token: past its bound\n"),
        ("/repositories/0/name", json!(""), "repositories[0].name: expected a Forgejo repository name\n"),
        ("/repositories/0/landing", json!(""), "repositories[0].landing: expected a branch name\n"),
        (
            "/repositories/0/ci",
            json!({"path":"","green":"ok"}),
            "repositories[0].ci.path: expected a relative file path with no . or .. component\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"ci/./result","green":"ok"}),
            "repositories[0].ci.path: expected a relative file path with no . or .. component\n",
        ),
        (
            "/repositories/0/ci",
            json!({"path":"ci/result","green":"ok","unknown":0}),
            "repositories[0].ci.unknown: unknown member\n",
        ),
    ];
    for (pointer, value, expected) in rows {
        let mut doc = forge_document();
        *doc.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            &*forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line(),
            expected.as_bytes(),
            "{pointer}"
        );
    }
    let repository = json!({"name":"widget","landing":"main","ci":null});
    for (count, expected) in
        [(2_usize, b"repositories[1].name: must be unique\n".as_slice()), (9, b"repositories: past its bound\n")]
    {
        let mut doc = forge_document();
        doc["repositories"] = Value::Array((0..count).map(|_| repository.clone()).collect());
        assert_eq!(&*forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line(), expected);
    }
    let mut doc = forge_document();
    doc["extra"] = json!(1);
    assert_eq!(
        &*forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line(),
        b"extra: unknown member\n"
    );
    forge::Document::decode(&serde_json::to_vec(&forge_document()).unwrap()).unwrap();
    for (bytes, expected) in [
        (b"[]".as_slice(), b"document: wrong value kind\n".as_slice()),
        (b"{", b"document: invalid JSON\n"),
        (b"{}", b"listen: missing\n"),
    ] {
        assert_eq!(forge::Document::decode(bytes).unwrap_err().line().as_ref(), expected);
    }
    let mut doc = forge_document();
    doc["owner"] = json!("x".repeat(40));
    doc["admin_token"] = json!("/".to_owned() + &"x".repeat(4095));
    doc["repositories"]=json!((0..8).map(|i|json!({"name":"x".repeat(99)+&i.to_string(),"landing":"x".repeat(256),"ci":{"path":"x".repeat(256),"green":"x".repeat(8192)}})).collect::<Vec<_>>());
    forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
}
#[test]
fn turns_spellings() {
    for (text, tokens) in [("", 0_u64), ("a", 1), ("abcd", 1), ("abcde", 2)] {
        let bytes = serde_json::to_vec(&text).unwrap();
        let d = super::read::document(&bytes).unwrap();
        let turn = turn::into_skein(turn::Turn::read(&d, 0, &Path::root()).unwrap());
        assert_eq!(
            turn,
            api::Turn {
                lines: Box::new([api::Line::Text { text: text.as_bytes().into() }]),
                finish: api::Finish::Stop,
                tokens
            }
        );
    }
    for (spelling, finish) in [
        ("stop", api::Finish::Stop),
        ("tool_calls", api::Finish::ToolCalls),
        ("length", api::Finish::Length),
        ("content_filter", api::Finish::ContentFilter),
    ] {
        let bytes = serde_json::to_vec(&json!({"lines":[{"text":"hello"},{"call":"tool_1-X","arguments":{"x":[1,true,null]}}],"finish":spelling,"tokens":99})).unwrap();
        let d = super::read::document(&bytes).unwrap();
        let turn = turn::into_skein(turn::Turn::read(&d, 0, &Path::root()).unwrap());
        assert_eq!(turn.finish, finish);
        assert_eq!(turn.tokens, 99);
        assert_eq!(
            turn.lines.as_ref(),
            &[
                api::Line::Text { text: b"hello".as_slice().into() },
                api::Line::Call {
                    name: b"tool_1-X".as_slice().into(),
                    arguments: br#"{"x":[1,true,null]}"#.as_slice().into()
                }
            ]
        );
    }
}
fn query(system: &[u8], request: &[u8]) -> api::Query {
    api::Query {
        credential: Some(0),
        model: b"qa".as_slice().into(),
        system: system.into(),
        messages: Box::new([api::Message {
            role: api::Role::User,
            parts: Box::new([api::Part::Text { text: request.into() }]),
        }]),
        tools: Box::new([]),
        choice: api::ToolChoice::None,
        max_tokens: 100,
        caching: api::Caching::Unscoped,
    }
}
fn answer(doc: &llm::Document, seed: u64, query: api::Query) -> api::Answer {
    let mut domain = doc.domain(seed);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: llm::LIMITS };
    let mut out = Queue::with_capacity(1);
    skein_fake_llm_domain::step(
        &mut domain,
        &env,
        skein_fake_llm_domain::Event::Call { reply_to: ReplyTo::new(Token::new(1)), call: Token::new(2), query },
        &mut out,
    );
    if out.is_empty() {
        skein_fake_llm_domain::fire(&mut domain, &env, &mut out);
    }
    match out.pop().unwrap() {
        skein_fake_llm_domain::Request::Reply { result, .. } => result.unwrap(),
        skein_fake_llm_domain::Request::Piece { .. } | skein_fake_llm_domain::Request::Hold { .. } => {
            panic!("whole answer")
        }
    }
}
#[test]
fn llm_answers() {
    let mut doc = llm_document();
    doc["cued"] = json!([{"cue":"system cue","turns":["system answer"]}]);
    doc["asked"] = json!([{"cue":"asked cue","turns":["asked answer"]}]);
    let doc = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    for (system, request, text) in
        [("system cue", "asked cue", "system answer"), ("", "asked cue", "asked answer"), ("", "other", "fallback")]
    {
        let answer = answer(&doc, 23, query(system.as_bytes(), request.as_bytes()));
        assert_eq!(answer.parts.as_ref(), &[api::Part::Text { text: text.as_bytes().into() }]);
    }
}
#[test]
fn llm_input_rule_bounds() {
    let rows = [
        (json!([]), "inputs: must be non-empty"),
        (json!((0..17).map(|_| json!({"cue":"", "turn":0})).collect::<Vec<_>>()), "inputs: past its bound"),
        (json!([{"cue":"x".repeat(257),"turn":0},{"cue":"","turn":0}]), "inputs[0].cue: past its bound"),
        (json!([{"cue":"é".repeat(129),"turn":0},{"cue":"","turn":0}]), "inputs[0].cue: past its bound"),
        (json!([{"cue":"x","turn":0}]), "inputs[0].cue: only the last rule must have an empty cue"),
        (json!([{"cue":"","turn":0},{"cue":"x","turn":0}]), "inputs[0].cue: only the last rule must have an empty cue"),
        (json!([{"cue":"","turn":0},{"cue":"","turn":0}]), "inputs[0].cue: only the last rule must have an empty cue"),
        (json!([{"cue":"","turn":2}]), "inputs[0].turn: must index this script's turns"),
        (json!([{"cue":"","turn":-1}]), "inputs[0].turn: past its bound"),
        (json!([{"cue":"","turn":0,"extra":1}]), "inputs[0].extra: unknown member"),
        (json!([{"turn":0}]), "inputs[0].cue: missing"),
        (json!([{"cue":""}]), "inputs[0].turn: missing"),
    ];
    for group in ["cued", "asked"] {
        for (inputs, expected) in &rows {
            let mut doc = llm_document();
            doc[group] = json!([{"cue":"script", "turns":["first","second"], "inputs":inputs}]);
            let refused = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err();
            assert_eq!(refused.line().as_ref(), format!("{group}[0].{expected}\n").as_bytes());
        }
    }
    let mut doc = llm_document();
    doc["otherwise"] =
        json!([{"lines":[{"text":"fallback"}],"finish":"stop","tokens":2,"inputs":[{"cue":"","turn":0}]}]);
    assert_eq!(
        llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
        b"otherwise[0].inputs: unknown member\n",
    );
}

#[test]
fn llm_input_rules_play_the_selected_turn() {
    // Both group indices retain their own table; absent inputs still count.
    for group in ["cued", "asked"] {
        let mut doc = llm_document();
        let mut inputs = vec![json!({"cue":"é".repeat(128),"turn":1})];
        inputs.extend((0..14).map(|i| json!({"cue":format!("unused {i}"),"turn":0})));
        inputs.push(json!({"cue":"","turn":0}));
        doc[group] = json!([
            {"cue":"counted", "turns":["counted answer"]},
            {"cue":"script", "turns":["default answer","selected answer"], "inputs":inputs},
        ]);
        let doc = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
        for (cue, request, expected) in [
            ("script", "é".repeat(128), "selected answer"),
            ("script", "other".into(), "default answer"),
            ("counted", "other".into(), "counted answer"),
        ] {
            let query = if group == "cued" {
                query(cue.as_bytes(), request.as_bytes())
            } else {
                query(b"", format!("{cue} {request}").as_bytes())
            };
            assert_eq!(answer(&doc, 23, query).parts.as_ref(), &[api::Part::Text { text: expected.as_bytes().into() }]);
        }
    }
}

#[test]
fn seeded_credentials() {
    let doc = llm::Document::decode(&serde_json::to_vec(&llm_document()).unwrap()).unwrap();
    let mut previous = alloc::collections::BTreeSet::new();
    for seed in [0_u64, 23] {
        let before = answer(&doc, seed, query(b"", b"other"));
        let streams = [credential::KEY, credential::ADMIN_TOKEN]
            .into_iter()
            .chain((0..credential::ADDED_KEYS).map(credential::added_key));
        for stream in streams {
            let key = credential(seed, stream);
            assert_eq!(key, credential(seed, stream));
            assert!(key.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)));
            assert!(previous.insert(key), "streams and seeds differ");
        }
        assert_eq!(before, answer(&doc, seed, query(b"", b"other")));
    }
}
#[test]
fn widget_prints_the_three_row_table() {
    let (_, main) = forge::FILES.iter().find(|(path, _)| *path == b"src/main.rs").unwrap();
    assert_eq!(*main, b"const TABLE: [(&str, u32); 3] = [(\"apples\", 3), (\"pears\", 5), (\"plums\", 2)];\n\nfn main() {\n    for (name, count) in TABLE {\n        println!(\"{name:<8}{count}\");\n    }\n}\n");
}
#[test]
fn forge_seed() {
    let mut doc = forge_document();
    doc["repositories"] =
        json!([{"name":"widget","landing":"main","ci":null},{"name":"second","landing":"trunk","ci":null}]);
    let doc = forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    for seed in [0_u64, 23, 0, 23] {
        let (mut forge, config) = doc.seed(seed);
        assert_eq!(config.users.as_slice(), &[(forge::OWNER, b"fake".as_slice().into())]);
        assert!(config.tokens.is_empty());
        for (i, repository) in config.repositories.iter().enumerate() {
            assert_eq!(repository.id, u32::try_from(i).unwrap() + 1);
            let landing = &doc.repositories()[i].landing;
            let head = forge.domain.branch(&repository.full, landing).unwrap();
            assert_eq!(head, u64::from(repository.id));
            let object = forge.domain.object(head).unwrap();
            assert_eq!(object.message.as_ref(), forge::MESSAGE);
            assert!(object.parent.is_none());
            assert!(object.merge_parent.is_none());
            for (path, content) in forge::FILES {
                assert_eq!(object.tree.get(path).unwrap().as_ref(), content);
            }
            assert_eq!(
                forge.domain.permission(&repository.full, 1).unwrap(),
                skein_fake_forge_domain::api::Permission::None
            );
            assert_eq!(
                forge.domain.permission(&repository.full, forge::OWNER).unwrap(),
                skein_fake_forge_domain::api::Permission::None
            );
        }
        assert!(forge.domain.pop_observation().is_none());
        assert_eq!(forge.domain.observations_lost(), 0);
    }
}
#[test]
fn ci_cue() {
    use skein_fake_forge_domain::{self as domain, api as a};
    for (cue, content, expected) in [
        (true, b"green".as_slice(), a::Check::Passed),
        (true, b"red", a::Check::Failed),
        (false, b"green", a::Check::Passed),
        (false, b"red", a::Check::Passed),
    ] {
        let mut doc = forge_document();
        if cue {
            doc["repositories"][0]["ci"] = json!({"path":"ci/result","green":"green"});
        }
        let doc = forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
        let (mut forge, _config) = doc.seed(23);
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: forge::LIMITS };
        let head =
            domain::advance(&mut forge.domain, &env, b"fake/widget", b"main", b"ci/result", content, forge::OWNER)
                .unwrap();
        let mut out = Queue::with_capacity(128);
        for _ in 0..128_u32 {
            if !forge.domain.is_due(Time::ZERO) {
                break;
            }
            domain::fire(&mut forge.domain, &env, &mut out);
            while out.pop().is_some() {}
            forge.domain.reclaim();
        }
        let statuses =
            forge.domain.inspect(&forge::LIMITS, b"fake/widget", &a::Read::Statuses { commit: head }).unwrap();
        let a::Answer::Statuses(statuses) = statuses else {
            panic!("statuses");
        };
        assert_eq!(statuses.len(), 1);
        assert_eq!(&*statuses[0].context, forge::CONTEXT);
        assert_eq!(statuses[0].state, expected);
    }
}

#[test]
fn constants_fit() {
    let models = (0..8).map(|i| json!({"name":format!("qa-{i}"),"window":4096,"output":1024})).collect::<Vec<_>>();
    let scripts = |prefix: &str| {
        (0..8)
            .map(|i| json!({"cue":format!("{prefix}-{i}"),"turns":["a","b","c","d","e","f","g","h"]}))
            .collect::<Vec<_>>()
    };
    let bytes=serde_json::to_vec(&json!({"listen":{"socket":"127.0.0.1:0"},"key":"/tmp/key","tls":null,
            "models":models,"cued":scripts("system"),"asked":scripts("request"),"otherwise":["a","b","c","d","e","f","g","h"]})).expect("valid fixture");
    let document = llm::Document::decode(&bytes).expect("valid fixture");
    let key = credential(0, credential::KEY);
    let plain = crate::wire::llm(llm::configuration(&document, &key[..], 0), false).expect("valid fixture");
    assert!(skein_fake_llm_domain::worst_case(&llm::LIMITS).is_some());
    let mut tls: Value = serde_json::from_slice(&bytes).expect("valid fixture");
    tls["tls"] = json!({"root":"/tmp/root"});
    let document = llm::Document::decode(&serde_json::to_vec(&tls).expect("valid fixture")).expect("valid fixture");
    crate::wire::llm(llm::configuration(&document, &key[..], 0), false).expect("valid fixture");
    let document=forge::Document::decode(&serde_json::to_vec(&json!({"listen":{"socket":"127.0.0.1:0"},"admin_token":"/tmp/admin_token","owner":"fake",
            "repositories":(0..8).map(|i|json!({"name":format!("repo-{i}"),"landing":"main","ci":null})).collect::<Vec<_>>()})).expect("valid fixture")).expect("valid fixture");
    let token = credential(0, credential::ADMIN_TOKEN);
    let (mut peer, _shared) = forge::peer(&document, &token, 0).expect("valid fixture");
    let setup = peer.setup_mut();
    let admin = setup.find(b"temper-admin").expect("valid fixture");
    assert!(admin.admin);
    assert_ne!(admin.id, forge::OWNER);
    assert_eq!(setup.issued().len(), 1);
    let issued = &setup.issued()[0];
    assert_eq!(issued.user, admin.id);
    assert_eq!(issued.secret.expose(), &token);
    assert_eq!(issued.scopes.as_ref(), &[Box::<[u8]>::from(b"all".as_slice())]);
    assert!(skein_fake_forge_protocol::git::fits(&crate::wire::forge_limits(), &forge::LIMITS.limits));
    assert!(skein_fake_forge_domain::worst_case(&forge::LIMITS.limits).is_some());
    assert!(plain.observations().is_empty());
    assert!(peer.observations().is_empty());
}

#[test]
fn forge_setup_document_equals_the_standalone_form() {
    use temper_forge_setup::setup::{Fake, Repository};
    for (owner, repositories) in
        [("fake", vec![("widget", "main")]), ("demo", vec![("widget", "trunk"), ("kit", "release")])]
    {
        let fake = Fake {
            listen: "127.0.0.1:3311".parse().expect("listen"),
            git: Box::from(b"/demo/bin/git".as_slice()),
            owner: owner.as_bytes().into(),
            repositories: repositories
                .iter()
                .map(|(name, landing)| Repository { name: name.as_bytes().into(), landing: landing.as_bytes().into() })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        let doc = json!({"listen":{"socket":"127.0.0.1:3311"},"admin_token":"/forge/admin_token","owner":owner,
            "repositories":repositories.iter().map(|(name, landing)| json!({"name":name,"landing":landing,"ci":null})).collect::<Vec<_>>()});
        assert_eq!(
            forge::Document::fake(&fake, b"/forge/admin_token"),
            forge::Document::decode(&serde_json::to_vec(&doc).expect("document")).expect("checked")
        );
    }
}

mod check;

mod demo;

mod steps;

mod github;

mod observations;

fn piece_turn(pieces: Value) -> Value {
    json!({"lines":[{"pieces":pieces}],"finish":"stop","tokens":1})
}
#[test]
fn standalone_pieces_and_control() {
    for hold in [Value::Null, json!("x"), json!("x".repeat(64))] {
        let mut doc = llm_document();
        doc["control"] = json!({"socket":"[::1]:0"});
        doc["otherwise"] =
            json!([piece_turn(json!((0..8).map(|_| json!({"text":"delta","hold":hold})).collect::<Vec<_>>()))]);
        let checked = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert_eq!(checked.control(), Some("[::1]:0".parse().unwrap()));
        let d = crate::read::document(&serde_json::to_vec(&doc["otherwise"][0]).unwrap()).unwrap();
        let turn = turn::into_skein(turn::Turn::read_standalone(&d, 0, &Path::root(), Some(1)).unwrap());
        let api::Line::Pieces { pieces } = &turn.lines[0] else { panic!("pieces preserved") };
        assert_eq!(pieces.len(), 8);
        assert_eq!(pieces[0].text.as_ref(), b"delta");
        assert_eq!(pieces[0].hold.as_deref(), hold.as_str().map(str::as_bytes));
    }
    let mut doc = llm_document();
    doc["otherwise"] = json!([piece_turn(json!([{"text":"delta","hold":null}]))]);
    llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    doc["control"] = json!({"socket":"127.0.0.1:0"});
    doc["otherwise"] = json!(["no holds"]);
    llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
}
#[test]
fn pieces_refuse_each_bound() {
    for (pieces, expected) in [
        (json!([]), "otherwise[0].lines[0].pieces: must be non-empty\n"),
        (
            json!((0..9).map(|_| json!({"text":"x","hold":null})).collect::<Vec<_>>()),
            "otherwise[0].lines[0].pieces: past its bound\n",
        ),
        (json!([{"text":"","hold":null}]), "otherwise[0].lines[0].pieces[0].text: must be non-empty\n"),
        (json!([{"text":"x","hold":""}]), "otherwise[0].lines[0].pieces[0].hold: must be non-empty\n"),
        (json!([{"text":"x","hold":"h".repeat(65)}]), "otherwise[0].lines[0].pieces[0].hold: past its bound\n"),
    ] {
        let mut doc = llm_document();
        doc["otherwise"] = json!([piece_turn(pieces)]);
        assert_eq!(
            llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
            expected.as_bytes()
        );
    }
}
#[test]
fn holds_require_loopback_control() {
    for member in ["otherwise", "cued", "asked"] {
        let mut doc = llm_document();
        let turns = json!([piece_turn(json!([{"text":"held","hold":"gate"}]))]);
        doc[member] = if member == "otherwise" { turns } else { json!([{"cue":"cue","turns":turns}]) };
        assert_eq!(
            llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
            b"control: required when a piece has a hold\n"
        );
    }
    let mut doc = llm_document();
    doc["control"] = json!({"socket":"192.0.2.1:0"});
    assert_eq!(
        llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
        b"control: must be a loopback socket address\n"
    );
}
#[test]
fn deployment_refuses_pieces_with_a_hold() {
    let mut doc = demo::fixture();
    doc["application"]["demo"]["llm"]["otherwise"] = json!([piece_turn(json!([{"text":"held","hold":"gate"}]))]);
    assert_eq!(
        demo::read(&doc).unwrap_err().line().as_ref(),
        b"application.demo.llm.otherwise[0].lines[0].pieces: unknown member\n"
    );
}

// A byte-side driver honours the reused HTTP machine's stream demands; no io or clock.
fn control_request(request: &[u8], release_queued: bool) -> (Vec<u8>, Option<Vec<u8>>) {
    use skein_lib::stream::{Down, Read, Up};
    let mut exchange = llm::control::Exchange::new();
    let mut offset = 0_usize;
    let mut response = Vec::new();
    let mut release = None;
    for _ in 0..256 {
        exchange.advance(Time::ZERO, Wall::EPOCH);
        if let Some(name) = exchange.release() {
            release = Some(name.to_vec());
            exchange.released(release_queued);
        }
        while let Some(down) = exchange.pop() {
            match down {
                Down::Demand { room, .. } if room > 0 => exchange.up(Time::ZERO, Wall::EPOCH, Up::Room),
                Down::Demand { read: Read::Scan { max, .. }, .. } => {
                    let rest = &request[offset..];
                    if rest.is_empty() {
                        exchange.up(Time::ZERO, Wall::EPOCH, Up::End);
                    } else {
                        let count = rest
                            .iter()
                            .position(|b| *b == b'\n')
                            .map_or(rest.len(), |i| i.checked_add(1).expect("request line bound"))
                            .min(usize::try_from(max).unwrap());
                        exchange.up(Time::ZERO, Wall::EPOCH, Up::Bytes(rest[..count].into()));
                        offset = offset.checked_add(count).expect("bounded request offset");
                    }
                }
                Down::Demand { read: Read::Nothing, .. } => {}
                Down::Send(bytes) => response.extend_from_slice(&bytes),
                other @ (Down::Demand { .. } | Down::Finish) => panic!("bodyless request: {other:?}"),
            }
        }
        if exchange.done() {
            return (response, release);
        }
    }
    panic!("control exchange did not settle");
}
#[test]
fn control_routes_releases_and_refusals() {
    for (method, target, queued, status, name) in [
        ("POST", "/release/gate", true, 204, Some(b"gate".as_slice())),
        ("POST", "/release/gate", false, 400, Some(b"gate".as_slice())),
        ("POST", "/release/", true, 400, None),
        ("GET", "/", true, 404, None),
        ("WHATEVER", "/", true, 404, None),
        ("POST", "/elsewhere", true, 404, None),
    ] {
        let (response, release) =
            control_request(format!("{method} {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(), queued);
        assert!(response.starts_with(format!("HTTP/1.1 {status} ").as_bytes()), "{response:?}");
        assert_eq!(release.as_deref(), name);
    }
    for size in [64, 65] {
        let (response, release) = control_request(
            format!("POST /release/{} HTTP/1.1\r\nHost: localhost\r\n\r\n", "h".repeat(size)).as_bytes(),
            true,
        );
        let status = if size == 64 { 204 } else { 400 };
        assert!(response.starts_with(format!("HTTP/1.1 {status} ").as_bytes()));
        assert_eq!(release.is_some(), size == 64);
    }
}
#[test]
fn control_refuses_malformed_heads_and_bodies() {
    for request in [
        b"POST /release/gate HTTP/1.1\r\n\r\n".to_vec(),
        b"POST /release/gate HTTP/1.1\r\nHost: localhost\r\nBad header\r\n\r\n".to_vec(),
        b"POST /release/gate HTTP/1.0\r\n\r\n".to_vec(),
        b"POST /release/gate HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\nx".to_vec(),
        b"POST /release/gate HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec(),
        b"POST /release/gate HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nExpect: 100-continue\r\n\r\n"
            .to_vec(),
        b"POST /release/gate HTTP/1.1\r\n".to_vec(),
        format!("POST /release/gate HTTP/1.1\r\nHost: localhost\r\nX: {}\r\n\r\n", "x".repeat(1024)).into_bytes(),
    ] {
        let (response, release) = control_request(&request, true);
        assert!(response.starts_with(b"HTTP/1.1 400 ") || response.starts_with(b"HTTP/1.0 400 "), "{response:?}");
        assert_eq!(release, None);
        assert!(response.windows(b"Connection: close".len()).any(|part| part == b"Connection: close"));
    }
}

#[test]
fn control_head_bound() {
    let head = "POST /release/gate HTTP/1.1\r\nHost: localhost\r\nX: ";
    let ending = "\r\n\r\n";
    let padding = 1024 - head.len() - ending.len();
    for (extra, status) in [(0, 204), (1, 400)] {
        let request = format!("{head}{}{ending}", "x".repeat(padding + extra));
        let (response, name) = control_request(request.as_bytes(), true);
        assert!(response.starts_with(format!("HTTP/1.1 {status} ").as_bytes()), "{response:?}");
        assert_eq!(name.is_some(), extra == 0);
    }
}

#[test]
fn control_shutdown_with_a_partial_head() {
    use skein_lib::stream::{Down, Read, Up};
    let mut exchange = llm::control::Exchange::new();
    exchange.advance(Time::ZERO, Wall::EPOCH);
    assert_eq!(exchange.pop(), Some(Down::Demand { read: Read::Nothing, room: 256 }));
    exchange.up(Time::ZERO, Wall::EPOCH, Up::Room);
    let Some(Down::Demand { read: Read::Scan { .. }, room: 0 }) = exchange.pop() else {
        panic!("reading the request head");
    };
    exchange.up(Time::ZERO, Wall::EPOCH, Up::Bytes(b"POST /release/gate HTTP/1.1\r\n".as_slice().into()));
    let Some(Down::Demand { read: Read::Scan { .. }, room: 0 }) = exchange.pop() else {
        panic!("reading the request head");
    };
    exchange.shutdown();
    exchange.advance(Time::ZERO, Wall::EPOCH);
    assert_eq!(exchange.pop(), Some(Down::Demand { read: Read::Nothing, room: 0 }));
    assert!(exchange.pop().is_none());
    assert!(exchange.release().is_none());
    assert!(exchange.done());
}

fn failing_turn(error: Value, credentials: Value) -> Value {
    json!({"lines":[{"fail":{"error":error,"credentials":credentials}}],"finish":"stop","tokens":1})
}

#[test]
fn llm_failing_lines_decode_each_error() {
    use api::Error;
    use skein_lib::Duration;
    for (encoded, error) in [
        (json!("overloaded"), Error::Overloaded),
        (json!("unavailable"), Error::Unavailable),
        (json!("context_too_long"), Error::ContextTooLong),
        (json!("unauthorized"), Error::Unauthorized),
        (json!("refused"), Error::Refused),
        (json!("unsupported"), Error::Unsupported),
        (json!("invalid_request"), Error::InvalidRequest),
        (json!({"rate_limited":{"retry_after_ms":1}}), Error::RateLimited { retry_after: Duration::from_millis(1) }),
        (
            json!({"rate_limited":{"retry_after_ms":u64::MAX}}),
            Error::RateLimited { retry_after: Duration::from_millis(u64::MAX) },
        ),
        (json!({"exhausted":{"retry_after_ms":1000}}), Error::Exhausted { retry_after: Some(Duration::from_secs(1)) }),
        (json!({"exhausted":{"retry_after_ms":null}}), Error::Exhausted { retry_after: None }),
    ] {
        for credentials in [json!([]), json!([0, 15])] {
            let mut doc = llm_document();
            doc["added_keys"] = json!((0..15).map(|i| format!("/tmp/added-{i}")).collect::<Vec<_>>());
            doc["cued"] =
                json!([{"cue":"script","turns":["first",failing_turn(encoded.clone(), credentials.clone())]}]);
            let doc = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
            let domain = doc.domain(23);
            assert_eq!(
                domain.scripts()[0].turns[1].lines.as_ref(),
                &[api::Line::Fail {
                    error,
                    report: api::Report::none(),
                    credentials: if credentials.as_array().unwrap().is_empty() {
                        Box::new([])
                    } else {
                        Box::new([0, 15])
                    },
                }]
            );
        }
    }
}

#[test]
fn llm_failing_lines_refuse_invalid_members() {
    for (error, credentials, suffix) in [
        (json!("mystery"), json!([]), "error: unknown error\n"),
        (json!({"mystery":{}}), json!([]), "error.mystery: unknown member\n"),
        (
            json!({"rate_limited":{"retry_after_ms":0}}),
            json!([]),
            "error.rate_limited.retry_after_ms: must be at least 1\n",
        ),
        (json!({"exhausted":{"retry_after_ms":0}}), json!([]), "error.exhausted.retry_after_ms: must be at least 1\n"),
        (
            json!({"rate_limited":{"retry_after_ms":null}}),
            json!([]),
            "error.rate_limited.retry_after_ms: wrong value kind\n",
        ),
        (json!({"exhausted":{"retry_after_ms":-1}}), json!([]), "error.exhausted.retry_after_ms: past its bound\n"),
        (json!({"exhausted":{"retry_after_ms":1,"extra":0}}), json!([]), "error.exhausted.extra: unknown member\n"),
        (json!({"exhausted":{}}), json!([]), "error.exhausted.retry_after_ms: missing\n"),
        (json!({}), json!([]), "error: expected one error\n"),
        (
            json!({"rate_limited":{"retry_after_ms":1},"exhausted":{"retry_after_ms":null}}),
            json!([]),
            "error: expected one error\n",
        ),
        (json!("unauthorized"), json!([0, 0]), "credentials[1]: must be unique\n"),
        (json!("unauthorized"), json!([1]), "credentials[0]: must index an accepted credential\n"),
        (json!("unauthorized"), json!([4_294_967_296_u64]), "credentials[0]: past its bound\n"),
        (json!("unauthorized"), json!((0..17).collect::<Vec<_>>()), "credentials: past its bound\n"),
    ] {
        let mut doc = llm_document();
        doc["cued"] = json!([{"cue":"script","turns":["first",failing_turn(error, credentials)]}]);
        let expected = format!("cued[0].turns[1].lines[0].fail.{suffix}");
        assert_eq!(
            llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
            expected.as_bytes()
        );
    }
}

#[test]
fn llm_added_keys_checks() {
    let mut doc = llm_document();
    assert!(llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap().added_keys().is_empty());
    doc["added_keys"] = json!([]);
    assert!(llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap().added_keys().is_empty());
    doc["added_keys"] = json!([format!("/{}", "x".repeat(4095))]);
    llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    let keys = (0..15).map(|i| format!("/tmp/added-{i}")).collect::<Vec<_>>();
    doc["added_keys"] = json!(keys);
    doc["otherwise"] = json!([failing_turn(json!("unauthorized"), json!((0..16).collect::<Vec<_>>()))]);
    let checked = llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(checked.added_keys().iter().map(|key| core::str::from_utf8(key).unwrap()).collect::<Vec<_>>(), keys);
    let key = credential(23, credential::KEY);
    llm::peer(llm::configuration(&checked, &key, 23)).unwrap();
    for (paths, expected) in [
        (json!(["/tmp/other", "/tmp/other"]), "added_keys[1]: must be unique and other than key\n"),
        (json!(["/tmp/key"]), "added_keys[0]: must be unique and other than key\n"),
        (json!((0..16).map(|i| format!("/tmp/added-{i}")).collect::<Vec<_>>()), "added_keys: past its bound\n"),
        (json!(["relative"]), "added_keys[0]: must be an absolute path without NUL\n"),
        (json!(["/tmp/\u{0}"]), "added_keys[0]: must be an absolute path without NUL\n"),
        (json!([format!("/{}", "x".repeat(4096))]), "added_keys[0]: past its bound\n"),
    ] {
        let mut doc = llm_document();
        doc["added_keys"] = paths;
        assert_eq!(
            llm::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err().line().as_ref(),
            expected.as_bytes()
        );
    }
}

#[test]
fn llm_failure_positions_are_checked_by_skein() {
    for group in ["cued", "asked", "otherwise"] {
        for (credentials, tail, valid) in [
            (json!([]), json!({"text":"ok"}), false),
            (json!([0]), json!({"text":"ok"}), true),
            (json!([0]), json!({"pieces":[{"text":"ok","hold":null}]}), false),
        ] {
            let mut turn = failing_turn(json!("unauthorized"), credentials);
            turn["lines"].as_array_mut().unwrap().push(tail);
            let mut doc = llm_document();
            let turns = json!(["first", turn]);
            doc[group] = if group == "otherwise" { turns } else { json!([{"cue":"script","turns":turns}]) };
            let decoded = llm::Document::decode(&serde_json::to_vec(&doc).unwrap());
            if valid {
                decoded.unwrap();
            } else {
                let path = if group == "otherwise" { "otherwise[1]".into() } else { format!("{group}[0].turns[1]") };
                assert_eq!(decoded.unwrap_err().line().as_ref(), format!("{path}: invalid turn\n").as_bytes());
            }
        }
    }
}

#[test]
fn deployment_refuses_failing_lines() {
    let mut doc = demo::fixture();
    doc["application"]["demo"]["llm"]["otherwise"] = json!([failing_turn(json!("unauthorized"), json!([]))]);
    assert_eq!(
        demo::read(&doc).unwrap_err().line().as_ref(),
        b"application.demo.llm.otherwise[0].lines[0].fail: unknown member\n"
    );
}
