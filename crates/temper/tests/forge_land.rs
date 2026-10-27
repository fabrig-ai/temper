pub mod support;

use serde_json::{Value, json};
use std::collections::BTreeSet;
use support::forge_land::{last_readme_line, live_roots, require_chat_progress, require_closed_roots};

fn roots_read(tasks: &[u64]) -> Value {
    json!({"tasks": {"rows": tasks.iter().map(|task| json!({"chip": {"task": task}})).collect::<Vec<_>>(), "more": false}})
}

#[test]
fn a_fresh_sittings_close_leaves_no_live_root() {
    let before = live_roots(&roots_read(&[])).expect("whole fresh roots");
    assert_eq!(before, BTreeSet::new());
    assert_eq!(require_closed_roots(&roots_read(&[]), &before), Ok(()));
    assert_eq!(
        require_closed_roots(&roots_read(&[7]), &before),
        Err("live root tasks this sitting left: [7]; earlier live root tasks missing: []".to_owned())
    );
}

#[test]
fn a_kept_store_keeps_an_earlier_sittings_live_chat() {
    let before = live_roots(&roots_read(&[1])).expect("whole kept roots");
    assert_eq!(before, BTreeSet::from([1]));
    assert_eq!(require_closed_roots(&roots_read(&[1]), &before), Ok(()));
    assert_eq!(
        require_closed_roots(&roots_read(&[1, 4]), &before),
        Err("live root tasks this sitting left: [4]; earlier live root tasks missing: []".to_owned())
    );
}

#[test]
fn a_sitting_never_closes_an_earlier_root() {
    let before = BTreeSet::from([1]);
    assert_eq!(
        require_closed_roots(&roots_read(&[]), &before),
        Err("live root tasks this sitting left: []; earlier live root tasks missing: [1]".to_owned())
    );
}

#[test]
fn a_roots_read_must_be_whole() {
    for read in [
        json!({"tasks": {"rows": [], "more": true}}),
        json!({"tasks": {"more": false}}),
        json!({"tasks": {"rows": [{"chip": {}}], "more": false}}),
    ] {
        assert!(live_roots(&read).is_err(), "{read}");
    }
}

#[test]
fn a_readme_ignores_trailing_empty_lines() {
    for (readme, expected) in [
        ("starter\ntag\n\n", Some("tag")),
        ("tag", Some("tag")),
        ("tag\n", Some("tag")),
        ("tag\n\n\n", Some("tag")),
        ("tag\r\n", Some("tag")),
        ("tag\r\n\r\n", Some("tag")),
        ("", None),
        ("\n", None),
        ("\n\n", None),
        ("\r\n", None),
    ] {
        assert_eq!(last_readme_line(readme.as_bytes()), expected, "README {readme:?}");
    }
}

#[test]
fn a_readme_keeps_its_last_nonempty_line_exactly() {
    for (readme, expected) in [
        ("tag\nother\n\n", "other"),
        ("tag \n\n", "tag "),
        (" tag\n", " tag"),
        ("tag\n  \n", "  "),
        ("tag\n\t\n\n", "\t"),
    ] {
        assert_eq!(last_readme_line(readme.as_bytes()), Some(expected), "README {readme:?}");
    }
}

// The facts read by discovery, in task show --json's owner shape. The encoders live
// in temper-client-tools/src/jig/render/document/{chips,owner_chips,
// owner_authority,tasks,parts,llm}.rs.
fn empty_park() -> Value {
    json!({
        "chip": {"details": {"owner": {
            "phase": {"active": "idle"},
            "details": {"attempt": 1, "live_delegates": 0}
        }}},
        "waits_on": "news",
        "run_wait": null,
        "subscriptions": {"entries": []},
        "cut": false,
        "wake": {
            "words": "immediate", "notices": "immediate", "news": "immediate",
            "results": "last_or_failure", "questions": true, "answers": true, "timers": true
        }
    })
}

#[test]
fn an_empty_park_stops_the_story() {
    assert_eq!(require_chat_progress(&empty_park()), Err("chat parked without delegating a change".to_owned()));
}

fn continues_with(pointer: &str, replacement: Value) {
    let mut task = empty_park();
    *task.pointer_mut(pointer).expect("fixture field") = replacement;
    assert_eq!(require_chat_progress(&task), Ok(()), "{pointer}: {task}");
}

#[test]
fn active_work_and_intentional_waits_continue() {
    continues_with("/chip/details/owner/details/attempt", Value::Null);
    continues_with("/chip/details/owner/details/live_delegates", json!(1));
    for stage in [
        json!("due"),
        json!("preparing"),
        json!({"claimed": {"attempt": 1}}),
        json!({"running": {"attempt": 1}}),
        json!({"backing_off": {"until": 10}}),
    ] {
        continues_with("/chip/details/owner/phase/active", stage);
    }
    let mut running = empty_park();
    running["chip"]["details"]["owner"]["phase"] = json!({"active": {"running": {"attempt": 1}}});
    for work in [
        json!("preparing"),
        json!("starting"),
        json!("active"),
        json!("writing"),
        json!({"tool": {"call": "01", "tool": "02"}}),
        json!("waiting"),
        json!("ending"),
    ] {
        running["chip"]["details"]["owner"]["details"]["observation"] =
            json!({"channel": 1, "work": work, "since": 1, "reported_at": 2, "started": 1});
        assert_eq!(require_chat_progress(&running), Ok(()), "{running}");
    }
    continues_with(
        "/chip/details/owner/phase",
        json!({"waiting": {"dependencies": [2], "more": false, "holds": false}}),
    );
    continues_with(
        "/chip/details/owner/phase",
        json!({"person": {"address": {"party": 1}, "taken_by": null, "since": null}}),
    );
    for waits in [
        json!("nothing"),
        json!("delegates"),
        json!({"dependencies": {"pending": [2], "count": 1}}),
        json!({"party": {"whom": {"person": 1}, "entries": 1, "task": 2, "kind": "question"}}),
    ] {
        continues_with("/waits_on", waits);
    }
    for wait in
        [json!("credential"), json!("host"), json!({"account": {"account": 1, "source": null, "why": "refreshing"}})]
    {
        continues_with("/run_wait", wait);
    }
    for kind in [
        json!({"timer": {"at": 10, "period": null}}),
        json!({"task": {"target": 2, "held": true, "result": true}}),
        json!({"topic": {"connector": 1, "topic": 2}}),
    ] {
        continues_with("/subscriptions/entries", json!([{"number": 1, "kind": kind}]));
    }
    continues_with("/cut", json!(true));
    for key in ["words", "notices", "news"] {
        for rule in [json!("never"), json!({"batch": {"count": 2, "age": 10}}), json!("unknown")] {
            continues_with(&format!("/wake/{key}"), rule);
        }
    }
    for rule in ["never", "each", "unknown"] {
        continues_with("/wake/results", json!(rule));
    }
    for key in ["questions", "answers", "timers"] {
        continues_with(&format!("/wake/{key}"), json!(false));
    }
    for key in ["words", "notices", "news", "results", "questions", "answers", "timers"] {
        let mut task = empty_park();
        task["wake"].as_object_mut().expect("wake object").remove(key);
        assert_eq!(require_chat_progress(&task), Ok(()), "missing wake {key}");
    }
    let mut task = empty_park();
    task["wake"]["future"] = json!(true);
    assert_eq!(require_chat_progress(&task), Ok(()), "unknown wake field");
    continues_with("/wake", Value::Null);
}

#[test]
fn a_terminal_chat_is_not_an_empty_park() {
    for phase in [
        json!({"held": {"reason": "drift", "revision": 1}}),
        json!({"closing": {"run": {"attempt": 1}}}),
        json!({"closing": "delegates"}),
        json!({"closing": "effects"}),
        json!({"closing": "releases"}),
        json!({"closing": "settled"}),
        json!("ended"),
    ] {
        continues_with("/chip/details/owner/phase", phase);
    }
}

#[test]
fn missing_chat_facts_report_the_shape_error() {
    for (parent, key) in [
        ("", "chip"),
        ("/chip", "details"),
        ("/chip/details", "owner"),
        ("/chip/details/owner", "phase"),
        ("/chip/details/owner", "details"),
        ("/chip/details/owner/details", "attempt"),
        ("/chip/details/owner/details", "live_delegates"),
        ("", "waits_on"),
        ("", "run_wait"),
        ("", "subscriptions"),
        ("/subscriptions", "entries"),
        ("", "cut"),
        ("", "wake"),
    ] {
        let mut task = empty_park();
        task.pointer_mut(parent).expect("fixture parent").as_object_mut().expect("object").remove(key);
        assert_eq!(require_chat_progress(&task), Err("chat progress facts missing".to_owned()), "{parent}/{key}");
    }
    for (pointer, malformed) in [
        ("/chip/details/owner/details/attempt", json!("1")),
        ("/chip/details/owner/details/attempt", json!(-1)),
        ("/chip/details/owner/details/live_delegates", Value::Null),
        ("/chip/details/owner/details/live_delegates", json!("0")),
        ("/subscriptions/entries", Value::Null),
        ("/cut", Value::Null),
    ] {
        let mut task = empty_park();
        *task.pointer_mut(pointer).expect("fixture field") = malformed;
        assert_eq!(require_chat_progress(&task), Err("chat progress facts missing".to_owned()), "{pointer}");
    }
}

fn launcher(directory: &std::path::Path, argument: &str, login: Option<&std::path::Path>) -> std::process::Command {
    let mut command = std::process::Command::new("/bin/sh");
    command
        .arg(skein_scratch::paths::workspace().join("scripts/proof-live.sh"))
        .arg(argument)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", directory)
        .env("TMPDIR", directory)
        .env("TEMPER_LIVE_STORE", directory.join("store"))
        .env("TEMPER_LIVE_KEEP", directory.join("evidence"));
    if let Some(login) = login {
        command.env("TEMPER_LIVE_CLAUDE", login);
    }
    command
}

#[test]
fn the_claude_launcher_refuses_invalid_held_directories() {
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    let scratch = support::Scratch::new("live-claude-refusals");
    let root = scratch.path();
    fs::write(root.join("file"), b"opaque").expect("file fixture");
    for (name, mode) in
        [("public", 0o755), ("empty", 0o700), ("Bad-name", 0o700), ("abcdefghijklmnopqrstuvwxyz1234567", 0o700)]
    {
        fs::create_dir(root.join(name)).expect("directory fixture");
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(mode)).expect("mode");
    }
    symlink(root.join("empty"), root.join("link")).expect("link fixture");
    for (argument, login, message) in [
        ("forge-land-claude", None, "TEMPER_LIVE_CLAUDE is required for forge-land-claude"),
        ("forge-land-claude", Some("missing"), "TEMPER_LIVE_CLAUDE names no directory"),
        ("forge-land-claude", Some("file"), "TEMPER_LIVE_CLAUDE is not a directory or is a link"),
        ("forge-land-claude", Some("link"), "TEMPER_LIVE_CLAUDE is not a directory or is a link"),
        ("forge-land-claude", Some("link/"), "TEMPER_LIVE_CLAUDE is not a directory or is a link"),
        ("forge-land-claude", Some("public"), "TEMPER_LIVE_CLAUDE is not 0700"),
        ("forge-land-claude", Some("empty"), "TEMPER_LIVE_CLAUDE is empty"),
        (
            "forge-land-claude",
            Some("Bad-name"),
            "TEMPER_LIVE_CLAUDE has no valid source name (1 to 32 lowercase letters, digits or hyphens)",
        ),
        (
            "forge-land-claude",
            Some("abcdefghijklmnopqrstuvwxyz1234567"),
            "TEMPER_LIVE_CLAUDE has no valid source name (1 to 32 lowercase letters, digits or hyphens)",
        ),
        ("forge-land", Some("empty"), "TEMPER_LIVE_CLAUDE is for forge-land-claude"),
        ("", Some("empty"), "TEMPER_LIVE_CLAUDE is for forge-land-claude"),
    ] {
        let path = login.map(|name| root.join(name));
        let output = launcher(root, argument, path.as_deref()).output().expect("offline launcher checks");
        assert_eq!(output.status.code(), Some(2), "{argument}: {login:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), message, "{argument}: {login:?}");
    }
    scratch.remove();
}

#[test]
fn a_claude_configuration_starts_on_inits_limits() {
    use skein_io::kernel::Exit;
    use skein_lib::Duration;
    use skein_world::end_to_end::Command;
    use support::{boxed, forge_land};
    let (scratch, paths) = initialized_deployment("live-claude-startup");
    configure_claude(&paths);
    // Stop at the sign-in line: no person signs in or starts the playground.
    // There is no login and the configured executable does not exist.
    let outcome = boxed::run_command(
        Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("serve".into())
                .chain(forge_land::deployment_flags(&paths).into_iter().map(Into::into))
                .collect(),
            directory: scratch.path().into(),
            environment: Vec::new(),
        },
        None,
        boxed::Plan::Stop,
        Duration::from_secs(10), // Startup and pipe drain only, within the focused suite's 30 s.
    );
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    assert!(support::printed_link(&outcome.stdout).is_some(), "serve reaches its sign-in line: {stderr}");
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "serve drains: {stderr}");
    assert_eq!(outcome.left, Some(vec![]), "no surviving children: {stderr}");
    assert!(std::fs::read_dir(paths[1].join("programs")).expect("private programs").next().is_none());
    scratch.remove();
}

fn initialized_deployment(case: &str) -> (support::Scratch, [std::path::PathBuf; 3]) {
    let scratch = support::Scratch::new(case);
    let paths = ["config", "data", "cache"].map(|name| scratch.path().join(name));
    support::forge_land::initialize(&paths);
    support::own_listener_files(&paths[0]);
    (scratch, paths)
}

fn configure_claude(paths: &[std::path::PathBuf; 3]) {
    use std::{fs, os::unix::fs::DirBuilderExt, path::Path};
    fs::DirBuilder::new().mode(0o700).create(paths[1].join("programs")).expect("fresh private programs");
    support::forge_land::configure_live_model(
        &paths[0],
        support::forge_land::LiveModel::ClaudeCode { login: &paths[1].join("programs/claude-login") },
        Path::new("/machine/bin/claude"),
    );
}

#[test]
fn a_claude_configuration_replaces_openai_and_keeps_the_demo() {
    use std::fs;
    // docs/help/box.md, step 1: Claude's fields over init's endpoint and model sizes.
    let (scratch, paths) = initialized_deployment("live-claude-configuration");
    let config = &paths[0];
    let agent_before: Value =
        serde_json::from_slice(&fs::read(config.join("agent.json")).expect("agent")).expect("JSON");
    let before: Value =
        serde_json::from_slice(&fs::read(config.join("temper.json")).expect("deployment")).expect("JSON");
    configure_claude(&paths);
    let agent: Value = serde_json::from_slice(&fs::read(config.join("agent.json")).expect("agent")).expect("JSON");
    let value: Value =
        serde_json::from_slice(&fs::read(config.join("temper.json")).expect("deployment")).expect("JSON");
    assert_eq!(
        agent["programs"],
        json!({"claude_code":{
            "path":"/machine/bin/claude", "directory":paths[1].join("programs")
        }})
    );
    assert_eq!(
        agent["endpoints"],
        json!([
            {"name":"claude", "number":2, "dialect":2, "provider":"anthropic", "program":"claude_code",
                "address":"api.anthropic.com:443", "transport":"tls", "server_name":"api.anthropic.com",
                "identity":"plain", "connect":10000, "handshake":10000, "headers":[],
                "models":[{"name":"sonnet", "prices":{"input":1, "cached":1, "output":1, "unit":1000},
                    "window":1_048_576, "output":8192, "reasoning_item":65536, "head":60000, "idle":30000}]},
            agent_before["endpoints"][1]
        ])
    );
    assert_eq!(agent["profile"], agent_before["profile"]);
    assert_eq!(agent["environment"], agent_before["environment"]);
    assert_eq!(
        value["engine"]["llm"],
        json!({
            "providers":[{"name":"anthropic", "number":2, "signed_in_plan":false,
                "manage_usage":"https://claude.ai/settings/usage", "models":["sonnet"],
                "sources":{"programs":["claude_code"]}}, before["engine"]["llm"]["providers"][1]],
            "endpoints":[{"name":"claude", "number":2, "dialect":2, "provider":"anthropic", "program":"claude_code",
                "address":{"host":"api.anthropic.com", "port":443}, "transport":"tls", "server_name":"api.anthropic.com",
                "headers":[], "connect_ms":10000, "handshake_ms":10000}, before["engine"]["llm"]["endpoints"][1]],
            "models":[{"name":"sonnet", "endpoint":"claude", "window":1_048_576, "output":8192,
                "prices":{"input":1, "cached":1, "output":1, "unit":1000}}, before["engine"]["llm"]["models"][1]]
        })
    );
    assert_eq!(value["application"], before["application"], "including init's demo");
    assert_eq!(value["engine"]["limits"], before["engine"]["limits"]);
    assert_eq!(value["workers"], before["workers"]);
    let parsed = smith_agent_shell::config::read(&config.join("agent.json")).expect("ordinary agent schema");
    assert_eq!(
        parsed.endpoint_programs.into_iter().collect::<Vec<_>>(),
        [(2, smith_agent_shell::config::ProgramName::ClaudeCode)]
    );
    eprintln!("Claude agent largest turn: {} bytes", parsed.service.limits.channel.bodies.turn_body);
    scratch.remove();
}

#[test]
fn stock_charters_choose_claude_before_the_demo_model() {
    let (scratch, paths) = initialized_deployment("live-claude-charters");
    configure_claude(&paths);
    let deployment = std::fs::read(paths[0].join("temper.json")).expect("deployment");
    let document = jig_protocol_config::document(
        &deployment,
        &jig_protocol_config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 },
    )
    .expect("document");
    let at = jig_protocol_config::read::Object::new(&document, 0)
        .expect("object")
        .member(&document, b"engine")
        .expect("engine");
    let engine = temper_service::startup::engine(&document, at, &support::configuration::directories(&paths[0]).jig())
        .expect("engine schema");
    let defaults = temper_service::stock::defaults(&engine.llm);
    for charter in &defaults.charters {
        assert_eq!((charter.policy.model.name.as_ref(), charter.policy.model.endpoint), (b"sonnet".as_slice(), 2));
        let alternatives: Vec<_> =
            charter.policy.alternatives.iter().map(|model| (model.name.as_ref(), model.endpoint)).collect();
        assert_eq!(alternatives, [(b"demo".as_slice(), 3)]);
    }
    scratch.remove();
}

#[test]
fn a_claude_plan_declares_without_import_or_sign_in() {
    use std::path::Path;
    use support::forge_land::{LiveModel, live_commands};
    let (commands, account_at) = live_commands(true, LiveModel::ClaudeCode { login: Path::new("/held/claude-login") });
    assert_eq!(
        &commands[account_at - 1],
        &["account", "declare", "2", "claude_code", "claude-login", "--key", "live-program"]
    );
    assert!(commands.iter().all(|args| args[0] != "program-login"
        && (args[0] != "account" || !matches!(args[1].as_str(), "connect" | "import"))));
    let (restart, _) = live_commands(false, LiveModel::ClaudeCode { login: Path::new("/held/claude-login") });
    assert!(
        restart.iter().all(
            |args| args[0] != "account" || !matches!(args[1].as_str(), "declare" | "connect" | "import" | "verify")
        )
    );
}

fn stubbed_launcher(case: &str, body: &str, source: &str) -> (support::Scratch, std::process::Output) {
    use std::{fs, os::unix::fs::PermissionsExt};
    let scratch = support::Scratch::new(case);
    let login = scratch.path().join(source);
    fs::create_dir(&login).expect("synthetic held directory");
    fs::set_permissions(&login, fs::Permissions::from_mode(0o700)).expect("private login fixture");
    fs::create_dir(login.join("opaque-entry")).expect("listing fixture only");
    let heavy = scratch.path().join("heavy");
    fs::write(&heavy, format!("#!/bin/sh\n{body}\n")).expect("offline queue stub");
    fs::set_permissions(&heavy, fs::Permissions::from_mode(0o700)).expect("executable stub");
    let output = launcher(scratch.path(), "forge-land-claude", Some(&login))
        .env("PATH", format!("{}:/usr/bin:/bin", scratch.path().display()))
        .output()
        .expect("stubbed launcher");
    (scratch, output)
}

#[test]
fn the_launcher_returns_the_login_after_build_failure() {
    // SourceName's inclusive lower and upper lengths (credentials.md, 3).
    for source in ["a", "abcdefghijklmnopqrstuvwxyz123456"] {
        let (scratch, output) = stubbed_launcher("live-claude-build-failure", "exit 17", source);
        assert_eq!(output.status.code(), Some(17));
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("CLAUDE LOGIN BACK: box stopped; same private directory, inode and owner; used in place")
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Approve the printed ChatGPT"));
        scratch.remove();
    }
}

#[test]
fn the_launcher_returns_the_login_after_a_signal() {
    let (scratch, output) = stubbed_launcher("live-claude-signal", "kill -TERM \"$PPID\"", "claude-login");
    assert_eq!(output.status.code(), Some(143));
    assert!(String::from_utf8_lossy(&output.stdout).contains("CLAUDE LOGIN BACK:"));
    scratch.remove();
}

#[test]
fn the_launcher_refuses_changed_login_metadata_on_exit() {
    let (scratch, output) =
        stubbed_launcher("live-claude-changed", "chmod 755 \"$TEMPER_LIVE_CLAUDE\"\nexit 17", "claude-login");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("CLAUDE LOGIN NOT BACK:"));
    scratch.remove();
}

#[test]
fn the_launcher_runs_only_the_claude_story_then_returns() {
    let body = "printf '%s\\n' \"$@\" >> \"$HOME/arguments\"\nexit 0";
    let (scratch, output) = stubbed_launcher("live-claude-pass", body, "claude-login");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("CLAUDE LOGIN BACK:"));
    let arguments = std::fs::read_to_string(scratch.path().join("arguments")).expect("stub arguments");
    assert!(arguments.contains("test(=a_person_asks_claude_code_to_land_a_change_on_the_owned_forge)"));
    scratch.remove();
}

fn program_accounts() -> Value {
    json!({"accounts":{
        "party":[{"number":7, "provider":2, "sources":[{"kind":"program", "name":"claude-login", "usable":true}]}],
        "program_hosts":[{"name":"worker", "connected":true, "programs":[{"name":"claude-login", "state":"usable"}]}]
    }})
}

#[test]
fn a_claude_source_is_verified_by_its_declared_account() {
    use support::forge_land::program_verification;
    assert_eq!(
        program_verification(std::path::Path::new("/held/claude-login"), &program_accounts()),
        Ok(vec![
            vec!["account", "verify", "7", "--source", "program", "--json"].into_iter().map(str::to_owned).collect(),
            vec!["account", "list", "--until", "usable", "--timeout", "600s", "--json"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            vec!["account", "list", "--json"].into_iter().map(str::to_owned).collect(),
        ])
    );
}

#[test]
fn a_claude_account_needs_its_login_usable_on_worker() {
    use support::forge_land::LiveModel;
    let model = LiveModel::ClaudeCode { login: std::path::Path::new("/held/claude-login") };
    assert_eq!(model.ready(&program_accounts()), Ok(()));
}

#[test]
fn a_claude_plan_refuses_missing_or_unusable_accounts() {
    use support::forge_land::LiveModel;
    let model = LiveModel::ClaudeCode { login: std::path::Path::new("/held/claude-login") };
    for (pointer, value, message) in [
        ("/accounts/party/0/provider", json!(1), "held Claude Code account missing"),
        ("/accounts/party/0/sources/0/name", json!("other-login"), "held Claude Code account missing"),
        ("/accounts/party/0/sources/0/usable", json!(false), "held Claude Code source is not usable"),
        ("/accounts/program_hosts/0/connected", json!(false), "held Claude Code login is not usable on the worker"),
        (
            "/accounts/program_hosts/0/programs/0/state",
            json!("needs_sign_in"),
            "held Claude Code login is not usable on the worker",
        ),
    ] {
        let mut accounts = program_accounts();
        *accounts.pointer_mut(pointer).expect("fixture field") = value;
        assert_eq!(model.ready(&accounts), Err(message.to_owned()), "{pointer}");
    }
}

#[test]
fn a_claude_plan_selects_provider_two_beside_demo_accounts() {
    use support::forge_land::{LiveModel, program_verification};
    let login = std::path::Path::new("/held/claude-login");
    let mut accounts = program_accounts();
    // The playground's key account, if listed, does not select the held program account.
    accounts["accounts"]["party"]
        .as_array_mut()
        .expect("party accounts")
        .insert(0, json!({"number":8, "provider":3, "sources":[{"kind":"key", "usable":true}]}));
    assert_eq!(LiveModel::ClaudeCode { login }.ready(&accounts), Ok(()));
    assert_eq!(
        program_verification(login, &accounts).expect("held account verification")[0],
        ["account", "verify", "7", "--source", "program", "--json"]
    );
}
