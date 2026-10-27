use super::fixtures::{self, DIRECTORIES, cases, content, parse, text};
use crate::{files, template};
use serde_json::{Value, json};
use skein_lib::{Stack, bytes};

fn set(value: &mut Value, path: &str, replacement: Value) {
    *value.pointer_mut(path).expect("the member exists in the owned example") = replacement;
}

fn every_forge() -> Value {
    json!({"segments": ["forge"], "last": "open", "value": ""})
}

fn replace_org_patterns(document: &mut Value) -> u32 {
    let org = json!({"segments": ["forge", "127.0.0.1:3000"], "last": "open", "value": "acme"});
    let mut pending = Stack::with_capacity(4096);
    pending.push(document).expect("the root fits");
    let mut replaced = 0_u32;
    for _ in 0..4096_u32 {
        let Some(value) = pending.pop() else { break };
        if *value == org {
            *value = every_forge();
            replaced = replaced.checked_add(1).expect("the owned example's pattern count fits");
            continue;
        }
        match value {
            Value::Array(items) => {
                for item in items {
                    pending.push(item).expect("the owned example fits the traversal bound");
                }
            }
            Value::Object(members) => {
                for member in members.values_mut() {
                    pending.push(member).expect("the owned example fits the traversal bound");
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    assert!(pending.is_empty(), "every owned example member was visited");
    replaced
}

#[test]
fn the_owned_example_offers_openai_keys_and_chatgpts_sign_in() {
    let owned = parse(fixtures::OWNED);
    assert_eq!(
        owned["engine"]["llm"]["providers"],
        json!([{
            "name": "openai", "number": 1_u32, "signed_in_plan": true,
            "manage_usage": "https://chatgpt.com/settings/usage",
            "models": ["gpt-6-luna"], "sources": {"keys": true, "sign_in": {
                "issuer": "https://auth.openai.com", "client": "dynamic_agent_client", "name_hint": "temper",
                "authorization": "https://auth.openai.com/api/accounts/authorize",
                "token": "https://auth.openai.com/api/accounts/oauth/token",
                "revocation": "https://auth.openai.com/api/accounts/oauth/revoke",
                "keys": "https://auth.openai.com/.well-known/jwks.json",
                "transport": "tls", "server_name": "auth.openai.com",
                "scopes": ["openid", "profile", "email", "offline_access", "resource.invoke", "chatgpt.tokens.use.direct"],
                "must_grant": ["chatgpt.tokens.use.direct"], "resource": "https://api.openai.com/v1",
                "redirect_path": "auth/callback", "redirect_port": 1_455_u16
            }}
        }])
    );
    let endpoint = &owned["engine"]["llm"]["endpoints"][0];
    assert_eq!(endpoint["name"], "openai");
    assert_eq!(endpoint["provider"], "openai");
    assert_eq!(endpoint["dialect"], 3_u32);
    assert_eq!(endpoint["address"], json!({"host": "api.openai.com", "port": 443_u16}));
    assert_eq!(endpoint["transport"], "tls");
    assert_eq!(endpoint["server_name"], "api.openai.com");
    assert_eq!(endpoint["headers"], parse(b"[]"));
    assert_eq!(owned["engine"]["llm"]["models"][0]["endpoint"], "openai");
    let agent = parse(fixtures::AGENT);
    let endpoint = &agent["endpoints"][0];
    assert_eq!(endpoint["name"], "openai");
    assert_eq!(endpoint["provider"], "openai");
    assert_eq!(endpoint["dialect"], 3_u32);
    assert_eq!(endpoint["address"], "api.openai.com:443");
    assert_eq!(endpoint["transport"], "tls");
    assert_eq!(endpoint["server_name"], "api.openai.com");
    assert_eq!(endpoint["headers"], parse(b"[]"));
    assert_eq!(endpoint["models"][0]["reasoning_item"], 65_536_u32);
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table checks the whole default document against the owned example")]
fn the_default_is_the_owned_example_so_edited() {
    let owned = parse(fixtures::OWNED);
    for (row, facts) in cases() {
        let answered = files(&facts, &DIRECTORIES, &[0; 32], &[0xab; 32]).expect("valid facts");
        let actual = parse(content(&answered, b"temper.json"));
        let members = actual["application"]["demo"].as_object().unwrap();
        assert_eq!(members.len(), 3, "{row}");
        for name in ["llm", "forge", "project"] {
            assert!(members.contains_key(name), "{row}: {name}");
        }
        let ports = [
            "/application/clients/listener/socket",
            "/application/forges/0/api/socket",
            "/application/forges/2/api/socket",
            "/application/demo/llm/listen/socket",
            "/application/webhooks/listener/socket",
        ]
        .map(|path| actual.pointer(path).expect("written listener").as_str().expect("socket"));
        for (index, port) in ports.iter().enumerate() {
            assert!(!ports[..index].contains(port), "{row}: five distinct ports");
        }
        assert_eq!(
            actual["application"]["forges"][2]["api"]["socket"],
            format!("127.0.0.1:{}", facts.ports.owned),
            "{row}"
        );
        let mut expected = owned.clone();
        // A review names repository, base, branch, gate and head, then offers two verdicts.
        expected["engine"]["limits"]["overrides"]["tasks.parameters"] = json!(5_u32);
        expected["engine"]["limits"]["overrides"]["tasks.contract_choices"] = json!(2_u32);
        expected["engine"]["acts"] = json!([{ "connector": 1_u16, "may": ["close", "remove_account"] }]);
        assert!(!actual["engine"]["owners"].as_array().unwrap().is_empty());
        // The startup turn_bounds writer holds these to each document's own
        // agent; init adds a demo endpoint, so smith derives a different T.
        let template = parse(include_bytes!("../../template/temper.json"));
        for pointer in ["/engine/limits/overrides", "/workers/0/limits/overrides", "/application/limits/overrides"] {
            for (name, bytes) in template.pointer(pointer).expect("template overrides").as_object().expect("limit map")
            {
                if name.starts_with("hub.turn")
                    || name == "hub.transcript_bytes"
                    || name == "core.resume_bytes"
                    || name.starts_with("root.loads.")
                    || name == "root.journal.transcript_bytes"
                {
                    expected.pointer_mut(pointer).expect("owned overrides")[name] = bytes.clone();
                }
            }
        }
        assert_eq!(replace_org_patterns(&mut expected), 30, "{row}");
        expected["engine"]["projects"][0]["policy"]["ceiling"]["tools"] = json!(515_u64);
        expected["engine"]["projects"][0]["policy"]["roles"][0]["authority"]["tools"] = json!(515_u64);
        expected["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["tasks"] = json!(14_u32);
        expected["engine"]["projects"][0]["policy"]["roles"][0]["authority"]["delegation"]["tasks"] = json!(14_u32);
        expected["engine"]["projects"][0]["policy"]["ceiling"]["budget"]["spend"] = json!(202_944_u64);
        expected["engine"]["projects"][0]["policy"]["period_spend"] = json!(202_944_u64);
        expected["engine"]["projects"][0]["policy"]["roles"][0]["period_spend"] = json!(202_944_u64);
        expected["engine"]["projects"][0]["policy"]["roles"][0]["authority"]["budget"]["spend"] = json!(101_472_u64);
        for authority in ["/engine/projects/0/policy/ceiling", "/engine/projects/0/policy/roles/0/authority"] {
            expected.pointer_mut(authority).expect("seed authority")["delegation"]["kinds"]
                .as_array_mut()
                .expect("executor kinds")
                .insert(2, json!({"kind":"charter", "number":4_u32}));
        }
        let policy = &mut expected["engine"]["projects"][0]["policy"];
        for path in ["/ceiling/grants", "/projections", "/roles/0/authority/grants"] {
            let grants = policy.pointer_mut(path).unwrap().as_array_mut().unwrap();
            for kind in [6_u32, 9, 16, 17] {
                grants.push(
                    json!({"connector":1_u16,"kind":kind,"pattern":{"segments":["forge"],"last":"open","value":""}}),
                );
            }
            let mut ordered = alloc::collections::BTreeMap::new();
            for grant in &*grants {
                ordered.insert(grant["kind"].as_u64().unwrap(), grant.clone());
            }
            grants.clear();
            for grant in ordered.into_values() {
                grants.push(grant);
            }
        }
        for path in ["/ceiling/delegation/kinds", "/roles/0/authority/delegation/kinds"] {
            for number in [3_u32, 4] {
                policy
                    .pointer_mut(path)
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"kind":"procedure","number":number}));
            }
        }
        expected["engine"]["policies"] = expected["engine"]["projects"].clone();
        expected["engine"]["limits"]["overrides"]["tasks.inbox_bytes"] = json!(12_288_u32);
        expected["application"]["limits"]["overrides"]["forge.client.repositories"] = json!(3_u32);
        expected["application"]["limits"]["overrides"]["forge.repositories"] = json!(3_u32);
        expected["application"]["limits"]["overrides"]["forge.output"] = json!(82_u32);
        for (path, value) in [
            (
                "/engine/llm/providers",
                json!([
                    owned["engine"]["llm"]["providers"][0],
                    {"name": "demo", "number": 3_u64, "signed_in_plan": false,
                     "models": ["demo"], "sources": {"keys": true}}
                ]),
            ),
            (
                "/engine/llm/endpoints",
                json!([
                    owned["engine"]["llm"]["endpoints"][0],
                    {"name": "demo", "number": 3_u64, "dialect": 3_u64, "provider": "demo",
                     "address": {"socket": format!("127.0.0.1:{}", facts.ports.llm)},
                     "transport": "plaintext", "headers": parse(b"[]"), "connect_ms": 10_000_u64, "handshake_ms": 10_000_u64}
                ]),
            ),
            ("/engine/llm/models", json!([owned["engine"]["llm"]["models"][0], demo_model()])),
            // The template's own figures, which startup's owned_memory_figures_for_init drift check writes and checks.
            ("/engine/memory_bytes", actual["engine"]["memory_bytes"].clone()),
            ("/workers/0/memory_bytes", actual["workers"][0]["memory_bytes"].clone()),
            (
                "/application/forges",
                json!([
                    {"number": 1_u16, "kind": "forgejo", "api": {"socket": format!("127.0.0.1:{}", facts.ports.forge)},
                     "transport": "plaintext", "git": format!("http://127.0.0.1:{}", facts.ports.forge), "web": null},
                    {"number": 2_u16, "kind": "github", "api": null, "transport": null, "git": null, "web": null},
                    {"number": 3_u16, "kind": "forgejo", "api": {"socket": format!("127.0.0.1:{}", facts.ports.owned)},
                     "transport": "plaintext", "git": format!("http://127.0.0.1:{}", facts.ports.owned), "web": null}
                ]),
            ),
            (
                "/application/identities",
                json!([
                    {"number": 1_u32, "forge": 2_u16, "login": null, "author": null},
                    {"number": 2_u32, "forge": 1_u16, "login": "temper",
                     "author": {"name": "Temper", "email": "temper@localhost"}},
                    {"number": 3_u32, "forge": 3_u16, "login": "temper",
                     "author": {"name": "Temper", "email": "temper@localhost"}}
                ]),
            ),
            (
                "/application/setup",
                json!([{
                    "forge": 1_u16, "directory": "demo/forge", "bot": {"login": "temper", "email": "temper@localhost"},
                    "mode": {"fake": {"listen": {"socket": format!("127.0.0.1:{}", facts.ports.forge)},
                        "git": "demo/bin/git", "owner": "playground", "repositories": [{"name": "widget", "landing": "main"}]}}
                }, {
                    "forge": 3_u16, "directory": "forge-3", "bot": {"login": "temper", "email": "temper@localhost"},
                    "start": "asked",
                    "mode": {"owned": {"forgejo": "fetched", "runner": {
                        "binary": "fetched", "labels": ["temper-host:host"], "capacity": 1_u32,
                        "path": "/usr/local/bin:/usr/bin:/bin"},
                        "listen": {"socket": format!("127.0.0.1:{}", facts.ports.owned)},
                        "person": {"login": "ada", "email": "ada@localhost"}}}
                }]),
            ),
            ("/application/demo", demo(facts.ports.llm.get())),
            ("/application/webhooks", json!({"listener": {"socket": format!("127.0.0.1:{}", facts.ports.webhooks)}})),
            ("/application/clients/listener/socket", json!(format!("127.0.0.1:{}", facts.ports.clients))),
            ("/application/clients/bundle", Value::Null),
            (
                "/application/workers/0/git",
                match facts.git {
                    Some(git) => json!(text(git)),
                    None => Value::Null,
                },
            ),
            ("/application/landing/projects/0/rules/0/pattern", every_forge()),
            ("/engine/hosts/workers/0/digest", json!("worker.digest")),
            ("/engine/deployment/directory", json!("deployment")),
            ("/workers/0/secret", json!("worker.secret")),
            ("/workers/0/agent/program", json!(text(facts.binary))),
            ("/workers/0/agent/configuration", json!("/c/temper/agent.json")),
            ("/workers/0/agent/arguments/2", json!("/c/temper/agent.json")),
        ] {
            set(&mut expected, path, value);
        }
        expected["application"]["landing"]["deployment"] =
            expected["application"]["landing"]["projects"][0]["rules"].clone();
        expected["application"]["landing"]["projects"] = parse(b"[]");
        for member in ["charters", "templates", "policies", "rules"] {
            expected["engine"].as_object_mut().unwrap().remove(member);
            assert!(actual["engine"].get(member).is_none(), "{row}: {member}");
        }
        expected["engine"]["llm"]["models"][0]["prices"]["unit"] = json!(1000_u64);
        assert_eq!(actual, expected, "{row}");
    }
}

fn demo(port: u16) -> Value {
    json!({
        "llm": {
            "listen": {
                "socket": format!("127.0.0.1:{port}")
            },
            "provider": 3,
            "key": "demo/llm.key",
            "asked": [
                {
                    "cue": "Add CSV export to widget",
                    "turns": csv_chat()
                },
                {
                    "cue": "Add a dark theme to widget",
                    "turns": [
                        demo_turn("I would give widget a dark theme that follows the system's preference, with its colours as variables in one place so both themes stay in step. I am the scripted model and play this from a script, so nothing is changed: connect a model from Accounts to have the work done.")
                    ]
                },
                {
                    "cue": "Fix widget's empty search state",
                    "turns": [
                        demo_turn("I would have widget say when a search finds nothing, name the words searched for and offer to clear them, in place of an empty table that looks broken. I am the scripted model and play this from a script, so nothing is changed: connect a model from Accounts to have the work done.")
                    ]
                }
            ],
            "cued": [
                {
                    "cue": "Export widget's table as CSV",
                    "turns": csv_coding()
                },
                {
                    "cue": "Review this change at the named head",
                    "turns": [scripted_turn(json!([{"call": "finish", "arguments": {
                        "form": "verdict", "label": "1", "text": "Checked widget CSV export.", "fields": {}, "items": parse(b"[]")
                    }}]))]
                }
            ],
            "otherwise": [
                demo_turn("I am the scripted model, the playground’s: I answer only the suggested requests, from a script. To ask for other work, connect a model from Accounts: a ChatGPT plan, a Claude subscription or an API key.")
            ]
        },
        "forge": 1,
        "project": 1
    })
}

// The template delegates playground/widget: forge 1, repository 1, tag 4294967297.
fn csv_chat() -> Value {
    json!([
        scripted_turn(json!([
            {"text": "I’ll delegate this change in playground/widget. It will push a branch, open a pull request, check CI and land into main on Playground forge."},
            {"call": "delegate", "arguments": {"batch": [{
                "executor": {"kind": "procedure", "connector": 1, "code": 2},
                "spec": {"words": "Export widget's table as CSV", "parameters": [
                    {"name": 1, "kind": "resource", "connector": 1, "resource": 4_294_967_297_u64},
                    {"name": 2, "kind": "bytes", "value": "main"}
                ]},
                "contract": {"kind": "change", "connector": 1, "change_kind": 1, "words": 128},
                "authority": {"tools": 1, "grants": [demo_grant(2), demo_grant(3), demo_grant(4), demo_grant(10), demo_grant(11), demo_grant(12)],
                    "delegation": {"kinds": [{"kind": "agent", "number": 2}], "tasks": 4, "depth": 2},
                    "budget": {"spend": 21_140}, "notes": 0}
            }]}}
        ])),
        scripted_turn(json!([{"call": "wait", "arguments": {}}])),
        scripted_turn(json!([
            {"text": "CSV export is available from widget’s table. The change landed on Playground forge. This was a scripted change."},
            {"call": "wait", "arguments": {}}
        ]))
    ])
}

fn demo_grant(kind: u16) -> Value {
    json!({"connector": 1_u16, "kind": kind, "segments": ["forge"], "terminal": "open", "last": ""})
}

fn csv_coding() -> Value {
    let old = "fn main() {\n    for (name, count) in TABLE {\n        println!(\"{name:<8}{count}\");\n    }\n}\n";
    let new = concat!(
        "fn main() {\n",
        "    let csv = std::env::args().any(|arg| arg == \"--csv\");\n",
        "    if csv {\n",
        "        println!(\"name,count\");\n",
        "    }\n",
        "    for (name, count) in TABLE {\n",
        "        if csv {\n",
        "            println!(\"{name},{count}\");\n",
        "        } else {\n",
        "            println!(\"{name:<8}{count}\");\n",
        "        }\n",
        "    }\n",
        "}\n"
    );
    json!([
        scripted_turn(json!([{"call": "read", "arguments": {"path": "src/main.rs"}}])),
        scripted_turn(json!([{"call": "edit", "arguments": {
            "path": "src/main.rs", "edits": [{"old": old, "new": new}]
        }}])),
        scripted_turn(json!([{"call": "finish", "arguments": {"form": "change", "fields": {
            "title": "Add CSV export to widget",
            "body": "widget --csv prints the table as CSV: a header line, then a line for each row. This was a scripted change."
        }}}]))
    ])
}

fn scripted_turn(lines: Value) -> Value {
    let mut bytes = 0_usize;
    for line in lines.as_array().unwrap() {
        let length = match line["text"].as_str() {
            Some(text) => text.len(),
            None => line["call"]
                .as_str()
                .unwrap()
                .len()
                .checked_add(serde_json::to_vec(&line["arguments"]).unwrap().len())
                .unwrap(),
        };
        bytes = bytes.checked_add(length).unwrap();
    }
    json!({"lines": lines, "finish": "tool_calls", "tokens": bytes.div_ceil(4)})
}

fn demo_turn(text: &str) -> Value {
    json!({"lines": [{"text": text}], "finish": "stop", "tokens": text.len().div_ceil(4)})
}

fn demo_model() -> Value {
    json!({"name": "demo", "endpoint": "demo", "window": 131_072_u64, "output": 8_192_u64,
           "prices": {"input": 1_u64, "cached": 1_u64, "output": 1_u64, "unit": 1_000_u64}})
}

fn demo_agent(port: u16) -> Value {
    json!({"name": "demo", "number": 3_u64, "dialect": 3_u64, "provider": "openai",
           "address": format!("127.0.0.1:{port}"), "transport": "plaintext",
           "identity": "plain", "connect": 10_000_u64, "handshake": 10_000_u64, "headers": parse(b"[]"),
           "models": [{"name": "demo", "window": 131_072_u64, "output": 8_192_u64,
                       "prices": {"input": 1_u64, "cached": 1_u64, "output": 1_u64, "unit": 1_000_u64},
                       "reasoning_item": 2_048_u64, "head": 60_000_u64, "idle": 30_000_u64}]})
}

#[test]
fn the_agent_configuration_is_the_owned_examples_but_its_programs() {
    let owned = parse(fixtures::AGENT);
    for (row, facts) in cases() {
        let answered = files(&facts, &DIRECTORIES, &[0; 32], &[0xab; 32]).expect("valid facts");
        let mut expected = owned.clone();
        let programs = match facts.claude {
            Some(claude) => json!({"claude_code": {"path": text(claude), "directory": "/d/temper/programs"}}),
            None => json!({}),
        };
        set(&mut expected, "/programs", programs);
        set(&mut expected, "/endpoints", json!([owned["endpoints"][0], demo_agent(facts.ports.llm.get())]));
        expected["endpoints"][0]["models"][0]["prices"]["unit"] = json!(1000_u64);
        assert_eq!(parse(content(&answered, b"agent.json")), expected, "{row}");
    }
}

#[test]
fn facts_are_written_as_json_text() {
    for (row, binary, escaped) in [
        (
            "quote and backslash",
            b"/opt/tem\"per\\bin/temper".as_slice(),
            b"\"/opt/tem\\\"per\\\\bin/temper\"".as_slice(),
        ),
        (
            "control bytes and UTF-8",
            b"/opt/\n\t\x01/\xc3\xa9/temper".as_slice(),
            b"\"/opt/\\n\\t\\u0001/\xc3\xa9/temper\"".as_slice(),
        ),
    ] {
        let facts = crate::Facts { binary, ..fixtures::facts() };
        let answered = files(&facts, &DIRECTORIES, &[0; 32], &[0xab; 32]).expect("valid text paths");
        let document = content(&answered, b"temper.json");
        assert!(bytes::find(document, escaped).is_some(), "{row}: JSON escaped the path");
        let parsed = parse(document);
        assert_eq!(parsed["engine"]["llm"]["endpoints"][1]["address"], json!({"socket": "127.0.0.1:8370"}), "{row}");
        assert_eq!(parse(content(&answered, b"agent.json"))["endpoints"][1], demo_agent(8370), "{row}");
        assert_eq!(
            parsed
                .pointer("/workers/0/agent/program")
                .expect("the worker has an agent program")
                .as_str()
                .expect("a JSON path is text")
                .as_bytes(),
            binary,
            "{row}"
        );
    }
}

#[test]
fn no_placeholder_is_left() {
    for (row, facts) in cases() {
        let answered = files(&facts, &DIRECTORIES, &[0; 32], &[0xab; 32]).expect("valid facts");
        for name in [b"temper.json".as_slice(), b"agent.json".as_slice()] {
            assert_eq!(bytes::find(content(&answered, name), b"\"$"), None, "{row}: {name:?}");
        }
    }
    for (row, source, expected) in [
        (
            "temper",
            template::TEMPER,
            [
                (b"\"$binary\"".as_slice(), 1_u32),
                (b"\"$agent\"".as_slice(), 2),
                (b"\"$git\"".as_slice(), 1),
                (b"\"$listener\"".as_slice(), 1),
                (b"\"$forge\"".as_slice(), 2),
                (b"\"$forge_git\"".as_slice(), 1),
                (b"\"$owned\"".as_slice(), 2),
                (b"\"$owned_git\"".as_slice(), 1),
                (b"\"$login\"".as_slice(), 1),
                (b"\"$email\"".as_slice(), 1),
                (b"\"$llm\"".as_slice(), 2),
                (b"\"$webhooks\"".as_slice(), 1),
            ]
            .as_slice(),
        ),
        ("agent", template::AGENT, [(b"\"$programs\"".as_slice(), 1_u32), (b"\"$llm\"".as_slice(), 1)].as_slice()),
    ] {
        // Exact token counts catch missing, extra and misspelled placeholders.
        let mut count = 0_u32;
        for (token, wanted) in expected {
            assert_eq!(bytes::count(source, token, u32::MAX), *wanted, "{row}: {token:?}");
            count = count.checked_add(*wanted).expect("the table's token count fits");
        }
        assert_eq!(bytes::count(source, b"\"$", u32::MAX), count, "{row}: only known tokens");
        parse(source);
    }
}

#[test]
fn the_person_keeps_a_forgejo_login_or_falls_back() {
    use crate::Person::{FellBack, Found};
    for (row, login, written, person) in [
        ("valid", Some(b"ada".as_slice()), "ada", Found),
        ("case kept", Some(b"Ada".as_slice()), "Ada", Found),
        ("reserved", Some(b"admin".as_slice()), "person", FellBack),
        ("reserved case", Some(b"Admin".as_slice()), "person", FellBack),
        ("reserved pattern", Some(b"x.keys".as_slice()), "person", FellBack),
        ("pattern case", Some(b"X.GPG".as_slice()), "person", FellBack),
        ("pattern is suffix", Some(b"x.keys.other".as_slice()), "x.keys.other", Found),
        ("invalid start", Some(b"-ada".as_slice()), "person", FellBack),
        (
            "at bound",
            Some(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".as_slice()),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Found,
        ),
        ("past bound", Some(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".as_slice()), "person", FellBack),
        ("bot", Some(b"temper".as_slice()), "person", FellBack),
        ("admin account", Some(b"temper-admin".as_slice()), "person", FellBack),
        ("org", Some(b"work".as_slice()), "person", FellBack),
        ("bot case", Some(b"Temper".as_slice()), "person", FellBack),
        ("empty", Some(b"".as_slice()), "person", FellBack),
        ("nontext", Some(b"\xff".as_slice()), "person", FellBack),
        ("absent", None, "person", FellBack),
    ] {
        let facts = crate::Facts { login, ..fixtures::facts() };
        let answer = files(&facts, &DIRECTORIES, &[0; 32], &[0xab; 32]).expect("login never refuses init");
        assert_eq!(answer.person, person, "{row}");
        let document = parse(content(&answer, b"temper.json"));
        assert_eq!(
            document["application"]["setup"][1]["mode"]["owned"]["person"],
            json!({"login": written, "email": format!("{written}@localhost")}),
            "{row}"
        );
    }
}

#[test]
fn the_template_keeps_project_one_without_settings_members() {
    let document = parse(template::TEMPER);
    assert_eq!(document["engine"]["projects"].as_array().unwrap().len(), 1);
    assert_eq!(document["engine"]["projects"][0]["number"], 1_u32);
    for member in ["charters", "templates", "policies", "rules"] {
        assert!(document["engine"].get(member).is_none());
    }
    let landing = &document["application"]["landing"];
    assert_eq!(landing["projects"], parse(b"[]"));
    assert_eq!(
        landing["deployment"],
        json!([{
            "connector": 1_u16, "kind": 4_u16, "pattern": every_forge(),
            "enforce": true, "ci": true, "up_to_date": true,
            "gates": [{"number": 1_u32, "blocking": true, "freshness": "clean"}], "approvals": parse(b"[]")
        }])
    );
}
