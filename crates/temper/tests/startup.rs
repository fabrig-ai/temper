pub mod support;
use std::{
    fmt::Write,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

#[test]
fn a_configuration_file_past_its_bound_is_refused() {
    let scratch = support::Scratch::startup("configuration-bound");
    let directory = scratch.path();
    fs::File::create(directory.join("temper.json"))
        .expect("configuration file")
        .set_len(1_048_577)
        .expect("one byte past bound");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused"),
        format!("temper serve: configuration {}: past 1048576 bytes\n", directory.join("temper.json").display())
    );
    scratch.remove();
}
#[test]
fn a_secret_file_readable_by_others_refuses_the_start_naming_it() {
    let scratch = support::Scratch::startup("public-token");
    let directory = scratch.path();
    support::configuration(directory, &support::Forge::Unreached);
    fs::set_permissions(directory.join("secrets/forge-1.git-token"), fs::Permissions::from_mode(0o644))
        .expect("public permissions");
    let refused = temper::prepare_in(&support::configuration::directories(directory)).err().expect("start refused");
    assert_eq!(
        refused,
        format!(
            "temper serve: application.identities[0].token: {}: readable by others or reached through a link\n",
            scratch.path().join("secrets/forge-1.git-token").display()
        )
    );
    assert!(!refused.contains("forge-1-git-token-value"), "no secret in a diagnostic");
    scratch.remove();
}
#[test]
fn a_worker_root_reached_through_a_link_is_refused() {
    let scratch = support::Scratch::startup("linked-root");
    let directory = scratch.path();
    support::configuration(directory, &support::Forge::Unreached);
    fs::create_dir(directory.join("real-root")).expect("root directory");
    symlink(directory.join("real-root"), directory.join("worker-root")).expect("linked root");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused"),
        "temper serve: application.workers[0].root: must be a private directory of this user's, not reached through a link\n"
    );
    scratch.remove();
}
#[test]
fn problems_reach_stderr_one_line_each() {
    let scratch = support::Scratch::startup("problem-lines");
    let directory = scratch.path();
    fs::write(directory.join("temper.json"), b"{}").expect("configuration");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused"),
        "temper serve: engine: missing\ntemper serve: workers: missing\ntemper serve: application: missing\n"
    );
    scratch.remove();
}

#[test]
fn an_address_with_no_token_in_the_directory_is_not_signed_in() {
    for (row, token, expected) in [
        ("absent", None, "temper open: not signed in: give the link temper serve printed\n"),
        ("empty", Some(b"".as_slice()), "temper open: not signed in: give the link temper serve printed\n"),
    ] {
        let scratch = support::Scratch::startup(&format!("open-{row}"));
        if let Some(token) = token {
            fs::write(scratch.path().join("token"), token).expect("token file");
            fs::set_permissions(scratch.path().join("token"), fs::Permissions::from_mode(0o600))
                .expect("private token");
        }
        assert_eq!(
            temper::open::check_token(scratch.path()).expect_err("not signed in").line(scratch.path()),
            expected,
            "{row}"
        );
        scratch.remove();
    }
}

#[test]
fn a_user_file_readable_by_others_refuses_the_start_naming_it() {
    let scratch = support::Scratch::startup("public-user");
    support::configuration(scratch.path(), &support::Forge::Unreached);
    fs::set_permissions(scratch.path().join("secrets/forge-1.user"), fs::Permissions::from_mode(0o644))
        .expect("fixture operation");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("configuration refused"),
        format!(
            "temper serve: application.forges[0].user.file: {}: readable by others or reached through a link\n",
            scratch.path().join("secrets/forge-1.user").display()
        )
    );
    scratch.remove();
}
fn managed(directory: &std::path::Path, move_files: bool) -> serde_json::Value {
    support::configuration(directory, &support::Forge::Unreached);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("configuration bytes"))
            .expect("fixture operation");
    value["application"]["setup"] = serde_json::json!([{
        "forge":1,"directory":"forge-1","bot":{"login":"temper","email":"temper@example.test"},
        "orgs":["acme"],"mode":{"managed":{"admin_token":"secrets/forgejo-admin"}}
    }]);
    if move_files {
        fs::create_dir(directory.join("forge-1")).expect("fixture operation");
        for (from, to) in [
            ("secrets/forge-1.api-token", "api_token"),
            ("secrets/forge-1.git-token", "git_token"),
            ("secrets/forge-1.user", "user"),
        ] {
            fs::rename(directory.join(from), directory.join("forge-1").join(to)).expect("fixture operation");
        }
        value["application"]["forges"][0]["api_token"] = "forge-1/api_token".into();
        value["application"]["forges"][0]["user"] = serde_json::json!({"file":"forge-1/user"});
        value["application"]["identities"][0]["token"] = "forge-1/git_token".into();
    }
    support::write_private(&directory.join("temper.json"), &serde_json::to_vec(&value).expect("configuration JSON"));
    value
}
#[test]
fn a_setup_naming_another_api_token_file_refuses_temper_serve_naming_its_path() {
    let scratch = support::Scratch::startup("setup-token");
    let mut value = managed(scratch.path(), true);
    value["application"]["forges"][0]["api_token"] = "another/api_token".into();
    support::write_private(
        &scratch.path().join("temper.json"),
        &serde_json::to_vec(&value).expect("configuration JSON"),
    );
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("configuration refused"),
        "temper serve: application.forges[0].api_token: invalid form\n"
    );
    scratch.remove();
}
#[test]
fn a_setup_limit_past_its_check_refuses_temper_serve_naming_both_limits() {
    let scratch = support::Scratch::startup("setup-limits");
    let mut value = managed(scratch.path(), true);
    value["application"]["limits"]["overrides"]["setup.backoff_ms"] = 6000.into();
    support::write_private(
        &scratch.path().join("temper.json"),
        &serde_json::to_vec(&value).expect("configuration JSON"),
    );
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("configuration refused"),
        "temper serve: application.limits.overrides.setup.backoff_ms: must agree with application.limits.overrides.setup.backoff_max_ms\n"
    );
    scratch.remove();
}
#[test]
fn a_managed_entrys_absent_admin_token_file_does_not_stop_temper_serve() {
    let scratch = support::Scratch::startup("setup-no-admin");
    managed(scratch.path(), true);
    assert!(!scratch.path().join("secrets/forgejo-admin").exists());
    assert!(temper::prepare_in(&support::configuration::directories(scratch.path())).is_ok());
    scratch.remove();
}
fn box_document(scratch: &support::Scratch) -> serde_json::Value {
    support::configuration(scratch.path(), &support::Forge::Unreached);
    serde_json::from_slice(&fs::read(scratch.path().join("temper.json")).expect("configuration")).expect("JSON")
}
fn refused_box(scratch: &support::Scratch, value: &serde_json::Value) -> String {
    support::write_private(&scratch.path().join("temper.json"), &serde_json::to_vec(value).expect("JSON"));
    temper::run(temper::Command::Box { directories: support::configuration::directories(scratch.path()) })
        .expect_err("box refused")
}
#[test]
fn temper_box_runs_a_fake_entry_to_serves_link() {
    let scratch = support::Scratch::startup("box-fake-entry");
    let mut value = managed(scratch.path(), true);
    let address = support::free_addresses::<1>(&[])[0];
    support::configuration::fake_entry(&mut value["application"]["setup"][0], &address.to_string());
    value["application"]["forges"][0]["api"] = serde_json::json!({"socket": address.to_string()});
    value["application"]["forges"][0]["git"] = format!("http://{address}").into();
    fs::set_permissions(scratch.path().join("forge-1"), fs::Permissions::from_mode(0o700))
        .expect("private forge directory");
    support::write_private(&scratch.path().join("temper.json"), &serde_json::to_vec(&value).expect("JSON"));
    let ended =
        support::boxed::run(scratch.path(), None, support::boxed::Plan::Stop, skein_lib::Duration::from_secs(3));
    let diagnostic = String::from_utf8_lossy(&ended.stderr);
    let link = support::printed_link(&ended.stdout).unwrap_or_else(|| panic!("serve's link: {diagnostic}"));
    assert!(skein_lib::bytes::find(link, b"/jig/v1/sign-in/link?secret=").is_some(), "{diagnostic}");
    assert!(!diagnostic.contains("application.setup"), "{diagnostic}");
    assert_eq!(ended.exit, Some(skein_io::kernel::Exit::Code(0)), "{diagnostic}");
    assert_eq!(ended.left, Some(Vec::new()), "no process outlived the box: {diagnostic}");
    scratch.remove();
}
#[test]
fn a_fake_entrys_handover_is_read_privately_and_checked_before_serve_starts() {
    for case in ["missing", "public", "short", "uppercase", "valid"] {
        let scratch = support::Scratch::startup(&format!("fake-handover-{case}"));
        let mut value = managed(scratch.path(), true);
        support::configuration::fake_entry(&mut value["application"]["setup"][0], "127.0.0.1:9");
        let handover = scratch.path().join("forge-1/admin_token");
        if case != "missing" {
            let bytes = match case {
                "short" => b"0123456789abcdef".as_slice(),
                "uppercase" => b"0123456789abcdef0123456789abcdef0123456A",
                _ => b"0123456789abcdef0123456789abcdef01234567",
            };
            support::write_private(&handover, bytes);
            if case == "public" {
                fs::set_permissions(&handover, fs::Permissions::from_mode(0o644)).expect("public handover");
            }
        }
        support::write_private(&scratch.path().join("temper.json"), &serde_json::to_vec(&value).expect("JSON"));
        let result = temper::envelope_in(
            &support::configuration::directories(scratch.path()),
            &serde_json::to_vec(&value).expect("JSON"),
        );
        match case {
            "valid" => {
                assert!(result.is_ok());
            }
            "missing" | "public" => {
                let why = if case == "missing" { "absent" } else { "readable by others or reached through a link" };
                assert_eq!(
                    result.expect_err("handover refused"),
                    format!("temper serve: application.setup[0].admin_token: {}: {why}\n", handover.display())
                );
            }
            _ => assert_eq!(
                result.expect_err("handover refused"),
                format!(
                    "temper serve: application.setup[0].admin_token: {}: admin_token must be exactly 40 lowercase hexadecimal digits\n",
                    handover.display()
                )
            ),
        }
        scratch.remove();
    }
}

#[test]
fn a_nonempty_box_configuration_without_a_document_is_refused_naming_it() {
    let scratch = support::Scratch::startup("box-absent-document");
    fs::write(scratch.path().join("other"), b"kept").expect("nonempty configuration");
    assert_eq!(
        temper::run(temper::Command::Box { directories: support::configuration::directories(scratch.path()) }),
        Err(format!(
            "temper box: configuration {}: absent; temper init writes one\n",
            scratch.path().join("temper.json").display()
        ))
    );
    scratch.remove();
}
#[test]
fn a_box_configuration_missing_members_is_refused_one_line_each() {
    let scratch = support::Scratch::startup("box-members");
    assert_eq!(
        refused_box(&scratch, &serde_json::json!({})),
        "temper box: engine: missing\ntemper box: workers: missing\ntemper box: application: missing\n"
    );
    scratch.remove();
}
#[test]
fn a_setup_naming_no_forge_refuses_the_box_before_it_starts() {
    let scratch = support::Scratch::startup("box-forge");
    let mut value = managed(scratch.path(), false);
    value["application"]["setup"][0]["forge"] = 7.into();
    assert_eq!(refused_box(&scratch, &value), "temper box: application.setup[0].forge: unknown reference\n");
    assert!(!scratch.path().join("forge-7").exists());
    scratch.remove();
}
#[test]
fn a_box_maximum_backoff_below_its_base_is_refused() {
    let scratch = support::Scratch::startup("box-backoff");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.backoff_max_ms"] = 100.into();
    assert_eq!(
        refused_box(&scratch, &value),
        "temper box: application.limits.overrides.box.backoff_ms: must agree with box.backoff_max_ms\n"
    );
    scratch.remove();
}
#[test]
fn a_box_maximum_backoff_reaching_its_window_is_refused() {
    let scratch = support::Scratch::startup("box-window");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.backoff_max_ms"] = 300_000.into();
    assert_eq!(
        refused_box(&scratch, &value),
        "temper box: application.limits.overrides.box.backoff_max_ms: must agree with box.window_ms\n"
    );
    scratch.remove();
}
#[test]
fn a_box_restart_count_past_its_bound_is_refused() {
    let scratch = support::Scratch::startup("box-restarts");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.restarts"] = 65.into();
    assert_eq!(
        refused_box(&scratch, &value),
        "temper box: application.limits.overrides.box.restarts: past its bound\n"
    );
    scratch.remove();
}
#[test]
fn a_box_restart_count_outside_its_type_is_refused() {
    let scratch = support::Scratch::startup("box-restarts-type");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.restarts"] = 4_294_967_296_u64.into();
    assert_eq!(
        refused_box(&scratch, &value),
        "temper box: application.limits.overrides.box.restarts: outside its type\n"
    );
    scratch.remove();
}
#[test]
fn an_unknown_box_limit_is_refused() {
    let scratch = support::Scratch::startup("box-unknown");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.nope"] = 1.into();
    assert_eq!(
        refused_box(&scratch, &value),
        "temper box: application.limits.overrides.box.nope: no limit of temper's has this name\n"
    );
    scratch.remove();
}
#[test]
fn a_box_limit_past_its_check_refuses_temper_serve_too() {
    let scratch = support::Scratch::startup("box-serve-refused");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.backoff_max_ms"] = 100.into();
    support::write_private(&scratch.path().join("temper.json"), &serde_json::to_vec(&value).expect("JSON"));
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("serve refused"),
        "temper serve: application.limits.overrides.box.backoff_ms: must agree with box.backoff_max_ms\n"
    );
    scratch.remove();
}
#[test]
fn a_box_limit_within_its_check_is_accepted_by_temper_serve() {
    let scratch = support::Scratch::startup("box-serve-accepted");
    let mut value = box_document(&scratch);
    value["application"]["limits"]["overrides"]["box.restarts"] = 3.into();
    support::write_private(&scratch.path().join("temper.json"), &serde_json::to_vec(&value).expect("JSON"));
    assert!(temper::prepare_in(&support::configuration::directories(scratch.path())).is_ok());
    scratch.remove();
}

#[test]
fn a_webhook_secret_readable_by_others_refuses_startup() {
    let scratch = support::Scratch::startup("public-hook-secret");
    let _ = support::webhooks::configuration(scratch.path());
    fs::set_permissions(scratch.path().join("secrets/forge-1.webhook-secret"), fs::Permissions::from_mode(0o644))
        .expect("public permissions");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("configuration refused"),
        format!(
            "temper serve: application.forges[0].webhook_secret: {}: readable by others or reached through a link\n",
            scratch.path().join("secrets/forge-1.webhook-secret").display()
        )
    );
    scratch.remove();
}

fn owned_example(case: &str) -> support::Scratch {
    let scratch = support::Scratch::startup(case);
    let owned = support::owned::Owned::example();
    support::owned::lay(scratch.path(), &owned);
    fs::write(scratch.path().join("temper.json"), support::owned::example_document(scratch.path(), &owned))
        .expect("example configuration");
    scratch
}
#[test]
fn the_owned_example_asks_for_reconciles_files() {
    let scratch = owned_example("owned-example-refused");
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("start refused"),
        "temper serve: application.forges[0].api_token: forge-1/api_token: absent\n\
         temper serve: application.forges[0].user.file: forge-1/user: absent\n\
         temper serve: application.forges[0].webhook_secret: forge-1/webhook_secret: absent\n\
         temper serve: application.identities[0].token: forge-1/git_token: absent\n"
            .replace(": forge-1/", &format!(": {}/forge-1/", scratch.path().display()))
    );
    scratch.remove();
}
fn owned_example_with_forge_files(case: &str) -> support::Scratch {
    const TOKEN_BYTES: usize = 512;
    const BOT_USER: &[u8] = b"42";
    let scratch = support::Scratch::startup(case);
    let owned = support::owned::Owned::example();
    support::owned::lay(scratch.path(), &owned);
    support::write_private(
        &scratch.path().join("temper.json"),
        &support::owned::example_document(scratch.path(), &owned),
    );
    let forge = scratch.path().join("forge-1");
    fs::create_dir(&forge).expect("forge directory");
    fs::set_permissions(&forge, fs::Permissions::from_mode(0o700)).expect("private forge");
    for (name, bytes) in [
        ("api_token", [b't'; TOKEN_BYTES].as_slice()),
        ("user", BOT_USER),
        ("webhook_secret", b"webhook-secret"),
        ("git_token", b"git-token"),
    ] {
        support::write_private(&forge.join(name), bytes);
    }
    scratch
}
#[test]
fn serve_prepares_once_on_the_owned_example() {
    let scratch = owned_example_with_forge_files("owned-example-prepared");
    let prepared = temper::prepare_in(&support::configuration::directories(scratch.path()));
    assert!(prepared.is_ok(), "{}", prepared.err().unwrap_or_default());
    scratch.remove();
}
#[test]
fn serve_checks_the_owned_agents_admission_at_each_bound() {
    let scratch = owned_example_with_forge_files("owned-admission-bounds");
    let path = scratch.path().join("owned/agent.json");
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("agent fixture")).expect("agent JSON");
    for (member, value, expected) in [
        (
            "max_turns",
            7,
            Some("engine.charters[0].turns: needs 8 turns; the agent's profile.policy.max_turns allows 7\n"),
        ),
        ("max_turns", 8, None),
        (
            "check_timeout",
            60001,
            Some(
                "engine.charters[0].reserve_time_ms: needs checks within 60000 ms; the agent's profile.policy.check_timeout is 60001 ms\n",
            ),
        ),
        ("check_timeout", 60000, None),
        (
            "check_timeout",
            300_000,
            Some(
                "engine.charters[0].reserve_time_ms: needs checks within 60000 ms; the agent's profile.policy.check_timeout is 300000 ms\n",
            ),
        ),
        (
            "tool_payload",
            4097,
            Some(
                "engine.limits: needs 4096-byte messages (tasks.message_bytes); the agent offers 4095 (tool_payload less 2)\n",
            ),
        ),
        ("tool_payload", 4098, None),
        (
            "tool_payload",
            600,
            Some(
                "engine.limits: needs 4096-byte messages (tasks.message_bytes); the agent offers 598 (tool_payload less 2)\n",
            ),
        ),
    ] {
        let mut agent = original.clone();
        let section = if member == "tool_payload" { "declared" } else { "policy" };
        agent["profile"][section][member] = value.into();
        support::write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
        let result = temper::prepare_in(&support::configuration::directories(scratch.path()));
        match expected {
            Some(line) => assert_eq!(
                result.err().expect("admission refused"),
                format!("temper serve: {line}"),
                "{member}={value}"
            ),
            None => assert!(result.is_ok(), "{member}={value}: {}", result.err().unwrap_or_default()),
        }
    }
    let mut agent = original.clone();
    agent["profile"]["policy"]["max_turns"] = 5.into();
    agent["profile"]["policy"]["check_timeout"] = 300_000.into();
    agent["profile"]["declared"]["tool_payload"] = 600.into();
    support::write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("old tuple refused"),
        "temper serve: engine.charters[0].turns: needs 8 turns; the agent's profile.policy.max_turns allows 5\n"
    );
    // The selected charter's number identifies its path, even when it is not its index.
    let config_path = scratch.path().join("temper.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).expect("configuration")).expect("JSON");
    assert_eq!(document["engine"]["charters"][1]["number"].as_u64(), Some(2));
    document["engine"]["charters"][0]["turns"] = 4.into();
    support::write_private(&config_path, &serde_json::to_vec(&document).expect("configuration JSON"));
    agent = original;
    agent["profile"]["policy"]["max_turns"] = 7.into();
    support::write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
    assert_eq!(
        temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("second charter refused"),
        "temper serve: engine.charters[1].turns: needs 8 turns; the agent's profile.policy.max_turns allows 7\n"
    );
    scratch.remove();
}

fn configuration_document(value: &serde_json::Value) -> skein_json::Document {
    jig_protocol_config::document(
        &serde_json::to_vec(value).expect("configuration JSON"),
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 65_536, depth: 32 },
    )
    .expect("configuration document")
}

#[test]
fn shipped_bounds_fit_the_configured_charters_and_worker() {
    use jig_protocol_config as config;
    use temper_service as service;

    let scratch = support::Scratch::startup("owned-admission");
    let bounded = support::owned::bounded(scratch.path());
    let value: serde_json::Value = serde_json::from_slice(&bounded.document).expect("deployment JSON");
    let engine = config::engine(&configuration_document(&value["engine"]), 0, &config::Directories::working())
        .expect("engine configuration");
    let agent = &bounded.agent;
    let core = config::core(&engine, [0; 16], 1);
    let needs = jig_domain_core::fleet_needs::configured(&core, &engine.limits.core);
    assert_eq!(
        jig_domain_fleet::shortfall(
            &needs,
            &temper_engine_domain::hello_bounds(agent.bounds.clone()),
            jig_domain_fleet::HostKind::Worker,
        ),
        None,
        "the shipped worker is admitted by the canonical fleet check"
    );
    // The minimum Smith payload is the engine's message requirement plus
    // Smith's rendered separator; its advertised message excludes that separator.
    let payload = u64::from(engine.limits.core.tasks.message_bytes)
        .checked_add(temper_worker_domain::SEPARATOR)
        .expect("rendered message");
    assert!(agent.host_limits.message_bytes >= payload);
    let named = service::named(&bounded.document, &config::Directories::working()).expect("owned configuration");
    let worker = &service::settings::Worker {
        jig: config::worker(&configuration_document(&value["workers"][0]), 0, &config::Directories::working())
            .expect("worker configuration"),
        root: named.worker.expect("standalone worker").root,
        git: Some(b"/usr/bin/git".as_slice().into()),
    };
    let mut own = service::limits::profile(service::limits::Profile::Small);
    for (name, number) in value["application"]["limits"]["overrides"].as_object().expect("overrides") {
        service::limits::apply(&mut own, name.as_bytes(), number.as_u64().expect("limit number"))
            .expect("application override");
    }
    let own = &own;
    assert!(worker.jig.limits.hub.event_bytes >= u64::from(agent.bounds.message_bytes));
    assert!(u64::from(own.checkout.message_bytes) >= agent.host_limits.call_bytes);
    let prepared = service::worker_settings::prepare(worker, agent, own).expect("complete worker bounds");
    // Each receiver one byte below its actual sender's bound fails its own rule.
    let mut short = prepared.root.clone();
    short.host.event_bytes = u64::from(agent.bounds.message_bytes).checked_sub(1).expect("nonempty message");
    assert_eq!(temper_worker_domain::unfit(&short).as_slice(), &[temper_worker_domain::Unfit::BoundsMessage]);
    let mut short = prepared.root.clone();
    short.checkout.message_bytes = u32::try_from(agent.host_limits.call_bytes)
        .expect("call fits checkout bound")
        .checked_sub(1)
        .expect("nonempty call");
    assert_eq!(temper_worker_domain::unfit(&short).as_slice(), &[temper_worker_domain::Unfit::CallBytes]);
    let required = service::worker_worst_case(worker, agent, own).expect("worker memory expression");
    assert!(required <= worker.jig.memory, "worker needs {required}, declares {}", worker.jig.memory);
    scratch.remove();
}

#[test]
fn the_owned_example_is_named_without_a_problem() {
    // Only naming a fixture document; no deployment or files are opened.
    let named = temper_service::named(support::owned::EXAMPLE, &jig_protocol_config::Directories::working())
        .expect("owned example named");
    assert_eq!(named.worker.as_ref().expect("standalone worker").agent.as_ref(), b"/home/me/temper/owned/agent.json");
    assert_eq!(named.worker.as_ref().expect("standalone worker").root.as_ref(), b"workspaces");
    assert_eq!(named.deployment.as_ref(), b"state");
    for path in [b"forge-1/api_token".as_slice(), b"forge-1/user", b"forge-1/webhook_secret", b"forge-1/git_token"] {
        assert_eq!(
            named.files.iter().find(|file| file.path.as_ref() == path).map(|file| file.private),
            Some(true),
            "{path:?}"
        );
    }
}

#[test]
fn every_command_checks_setup_before_reading_files() {
    let scratch = owned_example("every-command-setup");
    let path = scratch.path().join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    value["application"]["limits"]["overrides"]["setup.backoff_ms"] = 6000.into();
    let bytes = serde_json::to_vec(&value).expect("configuration JSON");
    support::write_private(&path, &bytes);
    let line = "application.limits.overrides.setup.backoff_ms: must agree with application.limits.overrides.setup.backoff_max_ms\n";
    for (command, refused) in [
        (
            "serve",
            temper::prepare_in(&support::configuration::directories(scratch.path())).err().expect("serve refused"),
        ),
        (
            "box",
            temper::run(temper::Command::Box { directories: support::configuration::directories(scratch.path()) })
                .expect_err("box refused"),
        ),
    ] {
        assert_eq!(refused, format!("temper {command}: {line}"), "{command}");
    }
    for verb in ["prepare", "reconcile", "run"] {
        support::owned::refused(scratch.path(), verb, format!("temper forge {verb}: forge 1: {line}").as_bytes());
    }
    let runner = support::owned::command(scratch.path(), "runner").output().expect("runner refuses");
    assert_eq!(runner.status.code(), Some(1));
    assert_eq!(runner.stderr, format!("temper forge run: forge 1: {line}").as_bytes());
    scratch.remove();
}

const PAGE: &[u8] = b"<!doctype html><title>t</title>";
const DIGEST: &str = "bundle/0123456789abcdef";

/// A refusal's row: its name, what spoils the bundle, and the start's refusal.
type Spoiled<'a> = (&'a str, fn(&std::path::Path), &'a str);
/// A limit's row: its name, the configuration's change, and the start's refusal.
type Limited<'a> = (&'a str, fn(&mut serde_json::Value), &'a str);

fn fixture_bundle(directory: &std::path::Path) {
    fs::create_dir_all(directory.join(DIGEST).join("files")).expect("bundle directory");
    fs::write(directory.join("bundle/index.html"), PAGE).expect("bundle page");
    fs::write(directory.join(DIGEST).join("boot.js"), b"boot();").expect("bundle script");
    fs::write(directory.join(DIGEST).join("files/robots.txt"), b"User-agent: *\nDisallow:\n")
        .expect("bundle root file");
}
fn bundle_configuration(directory: &std::path::Path) {
    support::configuration(directory, &support::Forge::Unreached);
    let document = directory.join("temper.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&document).expect("configuration")).expect("JSON");
    value["application"]["clients"]["bundle"] = directory.join("bundle").to_str().expect("bundle path").into();
    support::write_private(&document, &serde_json::to_vec(&value).expect("JSON"));
}
#[test]
fn a_bundle_directory_is_read_with_its_one_digest_directory() {
    let scratch = support::Scratch::startup("bundle-read");
    let directory = scratch.path();
    fixture_bundle(directory);
    let limits = temper_service::limits::Clients { bundle_files: 128, bundle_bytes: 16 << 20 };
    let bundle =
        temper::bundle::read(directory.join("bundle").as_os_str().as_encoded_bytes(), limits).expect("bundle read");
    assert_eq!(bundle.page.as_ref(), PAGE);
    assert_eq!(bundle.etag.as_ref(), b"\"edaa28e315690734\"");
    assert_eq!(bundle.digest.as_ref(), b"0123456789abcdef");
    assert_eq!(bundle.files.len(), 2);
    let script = bundle.files.get(b"boot.js".as_slice()).expect("script");
    assert_eq!((script.kind, script.bytes.as_ref()), (b"text/javascript".as_slice(), b"boot();".as_slice()));
    let robots = bundle.files.get(b"files/robots.txt".as_slice()).expect("root file");
    assert_eq!(
        (robots.kind, robots.bytes.as_ref()),
        (b"text/plain; charset=utf-8".as_slice(), b"User-agent: *\nDisallow:\n".as_slice())
    );
    scratch.remove();
}
#[test]
fn a_directory_that_is_no_bundle_refuses_the_start() {
    let rows: [Spoiled<'_>; 4] = [
        (
            "no-page",
            |directory| fs::remove_file(directory.join("bundle/index.html")).expect("page removed"),
            "temper serve: application.clients.bundle: bundle/index.html: absent\n",
        ),
        (
            "no-digest",
            |directory| fs::remove_dir_all(directory.join(DIGEST)).expect("digest removed"),
            "temper serve: application.clients.bundle: bundle: no digest directory\n",
        ),
        (
            "two-digests",
            |directory| fs::create_dir(directory.join("bundle/fedcba9876543210")).expect("second digest"),
            "temper serve: application.clients.bundle: bundle/fedcba9876543210: more than one digest directory\n",
        ),
        (
            "off-the-list",
            |directory| fs::write(directory.join(DIGEST).join("files/data.bin"), b"x").expect("odd file"),
            "temper serve: application.clients.bundle: bundle/0123456789abcdef/files/data.bin: file type is not on the closed list\n",
        ),
    ];
    for (name, spoil, expected) in rows {
        let scratch = support::Scratch::startup(&format!("bundle-{name}"));
        let directory = scratch.path();
        bundle_configuration(directory);
        fixture_bundle(directory);
        spoil(directory);
        assert_eq!(
            temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused"),
            expected.replace(": bundle", &format!(": {}/bundle", directory.display())),
            "row {name}"
        );
        scratch.remove();
    }
}
#[test]
fn a_bundle_past_a_limit_refuses_the_start() {
    let rows: [Limited<'_>; 2] = [
        (
            "files",
            |value| value["application"]["limits"]["overrides"]["clients.bundle_files"] = 2.into(),
            "temper serve: application.clients.bundle: bundle/0123456789abcdef/files/robots.txt: past clients.bundle_files\n",
        ),
        (
            "bytes",
            |value| value["application"]["limits"]["overrides"]["clients.bundle_bytes"] = 61.into(),
            "temper serve: application.clients.bundle: bundle/0123456789abcdef/files/robots.txt: past clients.bundle_bytes\n",
        ),
    ];
    for (name, limit, expected) in rows {
        let scratch = support::Scratch::startup(&format!("bundle-limit-{name}"));
        let directory = scratch.path();
        bundle_configuration(directory);
        fixture_bundle(directory);
        support::owned::configuration_value(directory, limit);
        assert_eq!(
            temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused"),
            expected.replace(": bundle", &format!(": {}/bundle", directory.display())),
            "row {name}"
        );
        scratch.remove();
    }
}

#[test]
fn programs_directories_are_checked_at_the_name_before_startup_effects() {
    for row in ["missing", "private", "link", "linked-parent", "mode", "file"] {
        let scratch = support::Scratch::startup(&format!("programs-{row}"));
        let directory = scratch.path();
        support::configuration(directory, &support::Forge::Unreached);
        let target = directory.join("programs");
        if row != "missing" {
            if row == "file" {
                fs::write(&target, b"not a directory").expect("file");
            } else {
                fs::create_dir(&target).expect("programs directory");
                fs::set_permissions(&target, fs::Permissions::from_mode(if row == "mode" { 0o750 } else { 0o700 }))
                    .expect("directory mode");
            }
        }
        let path = match row {
            "link" => {
                let link = directory.join("linked");
                symlink(&target, &link).expect("leaf link");
                link
            }
            "linked-parent" => {
                let parent = directory.join("parent");
                symlink(directory, &parent).expect("parent link");
                parent.join("programs")
            }
            _ => target.clone(),
        };
        let agent_path = directory.join("agent.json");
        let mut agent: serde_json::Value =
            serde_json::from_slice(&fs::read(&agent_path).expect("agent")).expect("JSON");
        agent["programs"] = serde_json::json!({"claude_code":{"path":"/nonexistent/claude", "directory":path}});
        fs::write(agent_path, serde_json::to_vec(&agent).expect("agent JSON")).expect("agent");
        // A later missing secret proves that the preflight neither creates nor inspects a login.
        fs::remove_file(directory.join("secrets/forge-1.api-token")).expect("absent peer");
        let refused = temper::prepare_in(&support::configuration::directories(directory)).err().expect("refused");
        if ["missing", "private", "linked-parent"].contains(&row) {
            assert!(refused.contains("application.forges[0].api_token"), "{row}: {refused}");
            assert!(!refused.contains("programs.claude_code.directory"), "{row}: {refused}");
        } else {
            let reason = match row {
                "link" => "symbolic link",
                "mode" => "nonprivate mode bits 0o50",
                "file" => "not a directory",
                _ => unreachable!(),
            };
            assert_eq!(
                refused,
                format!(
                    "temper serve: workers[0].agent.configuration: programs.claude_code.directory: {}: {reason}\n",
                    path.display()
                )
            );
        }
        assert!(!directory.join("worker-root").exists(), "{row}: no worker root");
        assert!(!target.exists() || row != "missing", "missing programs stay missing");
        scratch.remove();
    }
}

// The engine now retains two encoded values per identity: one extra 7,057-B
// value here. Its map value grows Secret (16 B) to generation + newest +
// previous (40 B), across 11 slots in the single reserved B-tree node.
// Tokens adds a 32-B map inline to Service; this Forgejo-only document
// reserves no GitHub entry maps or announcement scratch.
const GIT_GENERATIONS_GROWTH: u64 = (1 + 4 + 8 * (8 + 4 + 256 + 4 + 512) + 3 * (4 + 256)) + 11 * (40 - 16) + 32;

// Each of three worker credential slots keeps two generations, each gaining
// an 8-B Time. Option<Value> uses Value's existing enum niche for the empty
// marker, so neither it nor the outer Option<Grant> adds a tag.
const GIT_VALIDITY_GROWTH: u64 = 3 * 2 * 8;

fn github_identity_growth() -> (u64, u64) {
    // Worker-wire v3: Identity's old token plus three text fields was
    // 4 + 4096 + 3*(4 + 256) = 4,880 B. GitHub's kind has a one-byte tag,
    // a four-byte list length and eight entries (u64 account, length-prefixed
    // 256-B login and 512-B token), then the same three texts: 7,057 B.
    let github_grant_growth = (1 + 4 + 8 * (8 + 4 + 256 + 4 + 512) + 3 * (4 + 256)) - (4 + 4_096 + 3 * (4 + 256));
    // Each of two repositories adds an option tag and u64 account on the wire,
    // and 16 B inline in decoded Items. The engine holds one encoded identity;
    // its six channels (two workers + four opening) reserve two assignments
    // each. Translation holds two decoded/converted envelopes for each of
    // 32 requests plus one transient. Items remains the largest payload heap.
    // Three extra u32 limits in both Encoding and Worker add 24 B to Service.
    // Jig's 7,934-B credential bound still exceeds the new identity, so grant
    // envelopes keep their size. Only the application identity table grows.
    let repository_account_growth = 2 * (1 + 8);
    let github_identity_growth =
        github_grant_growth + (6 * 2 + 2 * (32 + 1)) * repository_account_growth + 2 * 16 + 2 * 3 * 4;
    // Two identity generations for each of three credential slots; one incoming
    // assignment body and two transient body/framed copies in the worker link.
    // Service's inline limits are charged to the engine above, so this worker
    // grows 13,116 B. A worker alone also owns Service's 24 B (13,140 B total).
    let github_worker_growth = 2 * 3 * github_grant_growth + 3 * repository_account_growth;
    (github_identity_growth, github_worker_growth)
}

fn start_growth() -> u64 {
    // Held DNS names: 32 B inline map in Service and its 312 B node at four forge slots.
    // This owned fixture uses socket addresses, so it retains no host-name bytes.
    let held_forges_growth = 32 + 312;
    // The fixed page served when no browser bundle is configured.
    held_forges_growth + u64::try_from(temper_service::clients::bundle::NO_BUNDLE_PAGE.len()).expect("fixed page size")
}

fn github_worker_pick_growth() -> u64 {
    // Value grows 64 -> 80 B; each of three credential slots keeps two generations.
    let identities = 3 * 2 * 16;
    // Repository.account adds 16 B to both repository types. The checkout has
    // two holds per workspace, three workspaces and two repositories per hold;
    // the root stages items for its one run slot plus one refused assignment.
    let repositories = (2 * 3 * 2 + (1 + 1) * 2) * 16;
    // Op grows 144 -> 160 B, checkout::Request 160 -> 176 B. The root keeps
    // two MAX_OUT (2) checkout requests; root::Request remains 224 B.
    let checkout_requests = 2 * 2 * 16;
    // Operation's inline Op grows 16 B in each of 2*(operations+1) slab slots.
    // worker_git::worst_case counts the component and operations+1 carriers.
    let git_operations = (1 + (3 + 1)) * (2 * (3 + 1)) * 16;
    identities + repositories + checkout_requests + git_operations
}

#[test]
#[expect(clippy::too_many_lines, reason = "keep the cause-by-cause census of the owned startup envelopes together")]
fn owned_memory_figures_match_startup_requirements_at_bounded_strings() {
    let scratch = support::Scratch::startup("owned-memory-figures");
    let bounded = support::owned::bounded(scratch.path());
    let required = bounded.requirements();
    // skein 9d34791e -> fc3e61ea changes terminal Job refusal only: zero owned-byte drift.
    // Both JSONs retain 85,508,807,222,521 B engine / 3,283,176,427,421 B worker.
    // Against main fcfd657f7 (79,659,997,210,050 B engine; 2,343,551,746,598 B worker).
    // Compared with the prior base, configured-forge retention and the setup-host/store-v3
    // changes lower both engine sums by 2,708,534,096 B. Their bounds cancel in this
    // repin delta; setup unions fit the existing carriers. Worker inputs are unchanged.
    // Root envelopes: 1 actual root + 2,817 service handoffs + 256 store queue slots
    // + 2 restored-page flights = 3,076 (the previous 3,074 omitted those last two).
    // Main's root/envelope was 19,829,474,639 B; now the root is 19,850,945,039 B
    // and a handoff is 19,837,786,639 B. Only the root owns the 50 independently
    // retained contexts (32 completed asks + 2*8 pending asks + 2 core transits).
    // Each handoff can own one incoming context; multiplying all retained contexts
    // by the queue capacities would charge names where they cannot be held.
    // Per-envelope growth, by owner/cause (all figures bytes):
    //   one incoming names array: 263,168; people ask headers: 9,008;
    //   fleet program-source state: 157,120; task state: 1,744; authority: 152;
    //   core routed payload/document bounds: 7,549,224;
    //   four new root callbacks: 4*(576 B Work + 65,536 B row envelope);
    //   retained root read/document and typed carrier envelopes: 67,136. Sum: 8,312,000 B.
    let root_growth = 263_168 + 9_008 + 157_120 + 1_744 + 152 + 7_549_224 + 4 * (576 + 65_536) + 67_136;
    // Stored task/people LLM fields enlarge the encoded row ceiling by 230,037 B
    // (885,491 -> 1,115,528). Every extra encoded byte adds 21,379,237 B across
    // KV buffers, chunks and decoded-page carrier bounds. Decoded row heaps grow
    // 929,808 B = 2*(263,192 task + 201,704 people + 8 TaskRecord header);
    // 886,803 prior page carriers + 4 joins hold them, plus 16 B fixed decoder state.
    // Beside those: 3 FileIo layers * 5 buffers grow by 230,037 B each; 6*256 raw read buffers each
    // grow 230,037 B; Store inline state +24 B; Service inline state +7,416 B;
    // worker-channel state +11,560 B; client documents +3,154,016 B;
    // accounts driver drops 1,009,024 B; empty D2/runtime state adds 1,832 B.
    // skein 64e28b02 adds job-log origin state: configured Forge 136 -> 168 B
    // across 4 list slots; Connection 1,384 -> 1,392 B and Waiting 528 -> 536 B
    // (its origin route) across 2 slab slots each; the slots also grow 8 B each.
    // Inline Service state grows 32,616 -> 32,648 B: skein 29f187c3's delivery-log
    // limits grow its forge Env 160 -> 184 B; skein 90ed6c4e's setup TLS limits
    // grow its host Env 128 -> 136 B. No GitHub forge is configured here, so
    // the origin and puller heaps add nothing. Sum: 128 + 16 + 16 + 24 + 8 = 192 B.
    let engine_growth = 3_076 * root_growth
        + 50 * 263_168
        + 230_037 * 21_379_237
        + (886_803 + 4) * 929_808
        + 16
        + 3 * 5 * 230_037
        + 6 * 256 * 230_037
        + 24
        + 7_416
        + 11_560
        + 3_154_016
        - 1_009_024
        + 1_832
        + 4 * (168 - 136)
        + 2 * (1_392 - 1_384)
        + 2 * (536 - 528)
        + (184 - 160)
        + (136 - 128);
    // The jig repin adds 5,768,515,608,452 B: main totals 85,428,512,818,502 B;
    // skein 94495819's 192 B above bring it to 85,428,512,818,694 B.
    // 05b adds 2,536 B of volatile root connection/installation tables to each
    // of those 3,076 root/payload envelopes, 14,204 B of protocol-sized
    // connections, 514 B of wanted diagnostic names for this one configured
    // forge, and 200 B of inline service growth. Records retain main's term.
    let lifecycle_growth = 3_076 * 2_536 + 14_204 + 514 + 200;
    // 05b delta +7,815,654 B; total 85,428,520,634,348 B.
    // JP10c-c: each bounded deployment gains two offers, their program names,
    // display names and manage targets, plus the header and two readiness rows.
    let document_growth = 2 * (96 + 2 * 16 + 2 * 32 + 128 + 2_048) + 56 + 2 * 24;
    // Core: one construction, sixteen watches, four act scratch documents,
    // two load slots and one five-document answer. Root: two more such answers.
    let document_copies = 1 + 16 + 4 + 2 + (2 + 3) + 2 * (2 + 3);
    // Pending/roles/coverage/candidate numbers coexist with the largest temporary.
    // Hosts + one model need and its name (4,520 B) exceed ranking (4*32 B)
    // and source facts ((4*3)*64 B).
    let readiness_scratch = 2 * 32 + 2 * 12 + 4 * 8 + 4 * 4 + (2 + 8) * 40 + 24 + 4_096;
    // Seven u32 wire bounds, 7*4 B plus 4 B alignment = 32 B.
    let wire_bounds_growth = 7 * 4 + 4;
    let catalogue_growth = 3_076 * (document_copies * document_growth + readiness_scratch) + wire_bounds_growth;
    // Catalogue delta +580,059,808 B; total 85,429,100,694,156 B.
    // y1b's original lifecycle: issued names, pending operations, removal unions;
    // fleet channel/pending/check/result state and detail bytes; scratch/cleanup
    // names, target snapshot and configured names. Then history/host views and
    // one validating answer copy. These bounds match jig's small profile.
    // Review adds typed Issued/Pending entries, joined fleet pending replies,
    // four scratch queues, 32 core routes and 32 milestones per projection cache.
    // Temper has one connector (three caches/task), and its routed payload is
    // 616,192 B. Its two-line document grows by two hosts' names/login lists and
    // four source names: 2*(4096 + 2*(88 + 32 + 2*128)) + 2*2*(16 + 32) = 9,888 B.
    // Document copies: construction + watches + acts + loads + one read answer
    // + two root read answers. Sixteen root routes add Work/row envelopes.
    let envelope_growth = (3 * 600 + 12 * 32)
        + (3 * 512 + 12 * 9 * 8 + 9 * 8)
        + (4 * 352 + 11 * 32)
        + (2 * 72 + 2 * 3 * 176 + 2 * 3 * 480 + 2 * 3 * 744 + 2 * 12 * 4_096)
        + 2 * 12 * 33
        + 9 * 4
        + 8 * (16 + 32)
        + 2 * 11 * 8
        + 3 * (2 + 8) * 8
        + 9 * (24 + 32)
        + (3 * 11 * 16 - 12 * 32)
        + 3 * 11 * 24
        + 2 * 3 * 11 * 8
        + 4 * 4 * 88
        + 32 * (536 + 240 + 616_192)
        + 8 * (1 + 2) * 32 * (168 + 4_096 + 2 * 512)
        + (1 + 16 + 4 + 2 + (2 + 3) + 2 * (2 + 3)) * 9_888
        + 16 * (576 + 65_536);
    // Sum 25,355,964 B. Against y1b's review-only +25,870,392 B: one fewer
    // connector saves 1,353,728 B of caches; smaller payloads save 709,632 B.
    // Original lifecycle +115,396, documents +375,744 and root routes +1,057,792
    // offset those savings, leaving temper 514,428 B below that review delta.
    // Fixed carriers: accounts' two 32 B maps + fleet's 16 B cleanup Box
    // and 16 B Membership;
    // root and engine-env copies of three +8 B child limits; three kit Store
    // carriers each add those limits + five u32 codec bounds + 4 B alignment;
    // two store-env limits copies; fourteen u32 client wire bounds. Sum 392 B.
    let carriers = 2 * 32 + 2 * 16 + 2 * (8 + 8 + 8) + 3 * (24 + 5 * 4 + 4) + 2 * 24 + 14 * 4;
    let programs_growth = 3_076 * envelope_growth + carriers;
    // Programs delta +77,994,945,656 B; total 85,507,095,639,812 B.
    // workers::worst_case retains one endpoint name: codex (5 B) becomes openai (6 B).
    let endpoint_growth = 6 - 5;
    // 20b: Host, owner tables/names, encoded writes/handoff, pages, above, IO, deletion scratch.
    let setup_growth = 96_188 + 14_768 + 49_216 + 131_712 + 1_610_617_344 + 100_673_280 + 200;
    // Jig 4353edb9 -> 4be07c1f: Program forms add 32 B per retained
    // grant and source. Core/root grow 8,576 B per envelope;
    // the accounts queue holds 264 FromComponent answers. Two client components retain
    // 64 model descriptions (512 B) and 64 B of prices/header per model.
    // Service's aligned codec bounds grow 8 B. No admission accounting changes.
    let repin_growth = 3_076 * 8_576 + 264 * 32 + 2 * 64 * (512 + 64) + 8;
    // e4b1a8855 -> jig 031f7267, smith cfc6a6b0, skein f77e4990:
    // measured core 12,221,072,683 -> 12,221,076,907 B (+4,224).
    // Model grows 112 -> 128 B; each of 64 bounded models also reserves a
    // 32 B backend name. Four charter carriers grow 16 B each. Placement
    // and catalogue each retain 4 accounts * 3 sources * 8 B HostSource.
    // HostRule grows the winning deployment document by 2 hosts * 16 B.
    // Core holds 28 copies (1 + 16 + 4 + 2 + 5); the root's two five-document
    // read answers add 10 more. Thus the root and payload each grow 4,544 B.
    // Assignment remains 176 B: its RunCharter is boxed, so its map adds 0.
    // Program forms/grants were already priced on e4b1a8855; no new 32 B
    // grant growth belongs to this interval. Measured system terms grow by
    // 1 root * 4,544 + 2,817 handoffs * 4,544 + (256 store slots + 2 read
    // flights) * 4,544 = 13,977,344 B; every other system term stays fixed.
    let endpoint_binding_growth = 3_076 * (64 * (16 + 32) + 4 * 16 + 2 * 4 * 3 * 8 + document_copies * (2 * 16));
    // One configured forge retains an optional repository git program (16 B).
    let git_program_growth = 16;
    // Jig 031f7267 -> 2a62db26: bounded task-record additions (added at/by,
    // circle witnesses and kept pending dependencies) enlarge retained records
    // and their store/page carriers by 42,615,522,708 B; worker inputs stay fixed.
    let added_dependencies_growth = 42_615_522_708_u64;
    // Jig 2a62db26 -> f2e7e8a2: the core retains preparing dependency-result
    // reads and their bounded IDs/results; temper drops its own reader.
    // Net engine growth is 6,680,579,880 B; worker inputs stay fixed.
    let preparing_dependency_reads_growth = 6_680_579_880_u64;
    // Jig f2e7e8a2 -> bd80e9be: core amendment reads retain dependency places,
    // attempted IDs and the original replay bytes while loading ended rows.
    // Each of the 3,076 root/envelope copies grows 366,256 B; workers stay fixed.
    let amendment_dependency_reads_growth = 3_076 * 366_256;
    // The root's 32 observation Facts grow 120 B inline for StepRefused's problem.
    // Its 3,076 owned envelope copies add 11,811,840 B; worker inputs stay fixed.
    let procedure_refusal_growth = 3_076 * 32 * 120;
    // Skein 4760d83c: the forge connection's JSON body intake adds 8,256 B
    // per connection at the 4,096-B chunk, across two connection slots.
    let forge_body_intake_growth = 2 * 8_256;
    // The public demo option adds 64 B inline; this deployment has no demo heap.
    let demo_growth = 64;
    // Listener notes: a separate 40 B queue holds three 40 B Note slots;
    // the existing forge slot grows 32 -> 40 B. Two pending bound addresses
    // add 32 B inline each, for 232 B total engine growth.
    let listening_growth = 40 + 3 * 40 + (40 - 32) + 2 * 32;
    // A host flight keeps its decoded amendment's changed controls (12 B),
    // growing Flight 80 -> 96 B: the 32-call host flight map grows 1,232 B
    // in each of 3,076 owned envelope copies; worker inputs stay fixed.
    let amendment_changes_growth = 3_076 * 1_232;
    // Start-time fake adoptions add a 32 B inline map and one 1,016 B tree node
    // (four forge slots, 80 B Adoption values). Held bytes reserve each forge's
    // 40 B handover, 512 B API token and 128 B hook secret: +3,768 B in total.
    let fake_adoption_growth = 32 + 1_016 + 4 * (40 + 512 + 128);
    // The bounded fixture now supplies the checked owned entry, as serve does:
    // its Forgejo Given, host protocol buffers and bounded plans add 1,194,285,681 B.
    let forgejo_setup_growth = 1_194_285_681;
    // Jig ed4141db -> 197305b5: bounded open-question facts and their census
    // reads grow the engine and its owned envelope copies by 886,277,472 B.
    let open_questions_growth = 886_277_472;
    // The root's settled Option<(Time, Wall)> adds 24 B inline through Engine
    // in Service. Payload envelopes and worker-owned state are unchanged.
    let settled_root_growth = 24;
    // Jig finishing: the small profile indexes eight sign-ins, replaces one
    // 8-B token per retained run with a 24-B reconciliation action, and
    // replaces each 88-B watch snapshot with a 16-B ending. Core also grows
    // 40 B inline (32-B expiry set header plus 8-B immutable room cache).
    let expiry_index = skein_lib::Set::<(skein_lib::Wall, u64)>::worst_case(8).expect("small expiry index");
    let reconciliation_growth = u64::from(temper_engine_domain::VIEWS.runs) * (24 - 8);
    let ending_release = u64::from(temper_engine_domain::VIEWS.watchers) * (88 - 16);
    // The same heap delta propagates through the existing 3,076 root and
    // payload envelopes counted above; only Service owns Core's inline bytes.
    let finishing_growth = 3_076 * (expiry_index + reconciliation_growth) + 40;
    let ending_release = 3_076 * ending_release;
    let (github_identity_growth, github_worker_growth) = github_identity_growth();
    assert_eq!(
        required.engine_bytes,
        79_659_997_210_050_u64
            + engine_growth
            + lifecycle_growth
            + catalogue_growth
            + programs_growth
            + endpoint_growth
            + setup_growth
            + repin_growth
            + endpoint_binding_growth
            + git_program_growth
            + added_dependencies_growth
            + preparing_dependency_reads_growth
            + amendment_dependency_reads_growth
            + procedure_refusal_growth
            + forge_body_intake_growth
            + demo_growth
            + listening_growth
            + amendment_changes_growth
            + fake_adoption_growth
            + forgejo_setup_growth
            + open_questions_growth
            + settled_root_growth
            + finishing_growth
            - ending_release
            + github_identity_growth
            + start_growth()
            + GIT_GENERATIONS_GROWTH
    );
    // Worker host payload stays 107,206,624 B. Program-source hub outputs grow
    // room 7,286 -> 10,198: 2,912 additional slots, three host payload queues.
    // Other causes: root program-source state +117,552 B; serialized source
    // command/resources +230,981 B; larger source-inventory link frames +199,784 B;
    // extra root Request/Event/ProcessEvent slots 224/544/32 B and lower IO
    // cohorts 9,984 B per room slot (the link's 48 outputs * 208 B IO request).
    let worker_growth = 8_736 * 107_206_624_u64 + 117_552 + 230_981 + 199_784 + 2_912 * (224 + 544 + 32 + 9_984);
    // Delta +936,589,018,589 B (31,951,325 B outside host copies); total 3,280,140,765,187 B.
    // Admission: payload 600 -> 4098, hub event 598 -> 4096 and checkout
    // message 4096 -> 4098. The unchanged worker_worst_case calculation at
    // these same bounded strings adds exactly 3,035,662,234 B (no headroom).
    let admission_growth = 3_035_662_234_u64;
    // Four held grants add 4 * 32 B in the host; the worker link's
    // transient list holds accounts + credentials = 4 + 3 grants (224 B).
    // Include v3's identity generations and assignment carriers; the original
    // admission cause stays separate.
    let grant_wire_growth = 4 * 32 + (4 + 3) * 32 + github_worker_growth + github_worker_pick_growth();
    assert_eq!(
        required.worker_bytes,
        2_343_551_746_598_u64 + worker_growth + admission_growth + grant_wire_growth + GIT_VALIDITY_GROWTH
    );

    support::owned::check_memory("../../docs/examples/owned.json", required);
    eprintln!("owned requirements: engine {}, worker {} bytes", required.engine_bytes, required.worker_bytes);
    check_bounded_startup(&bounded, required);
    scratch.remove();
}

fn check_bounded_startup(bounded: &support::owned::Bounded, required: temper_service::startup::Requirements) {
    let mut value: serde_json::Value = serde_json::from_slice(&bounded.document).expect("bounded JSON");
    value["engine"]["memory_bytes"] = required.engine_bytes.into();
    value["workers"][0]["memory_bytes"] = required.worker_bytes.into();
    let document = serde_json::to_vec(&value).expect("exact envelopes JSON");
    let started = bounded.startup(&document);
    assert!(
        started.is_ok(),
        "{}",
        String::from_utf8_lossy(&temper_service::startup::lines(&started.err().expect("startup refusal")))
    );
    for (engine, path) in [(true, "engine.memory_bytes"), (false, "workers[0].memory_bytes")] {
        let mut short = value.clone();
        if engine {
            short["engine"]["memory_bytes"] = (required.engine_bytes - 1).into();
        } else {
            short["workers"][0]["memory_bytes"] = (required.worker_bytes - 1).into();
        }
        let refused = bounded
            .startup(&serde_json::to_vec(&short).expect("one-byte-short JSON"))
            .err()
            .expect("refused before construction");
        assert_eq!(
            String::from_utf8_lossy(&temper_service::startup::lines(&refused)),
            format!("{path}: the worst case passes memory_bytes by 1 bytes\n")
        );
    }
}

#[test]
fn owned_memory_figures_for_init_match_its_own_document() {
    let scratch = support::Scratch::startup("init-memory-figures");
    let bounded = support::owned::bounded_template(scratch.path());
    let required = bounded.requirements();
    // The same listener growth is 240 B here: init has two existing forge
    // Note slots growing 32 -> 40 B, rather than the owned example's one.
    // The fake-adoption map and held credentials add the same 3,768 B as above.
    // Supplying init's checked fake entry also counts its host and git program;
    // its GitHub host was already priced, so this engine term adds 26,262,672 B.
    // Worker-wire v3 adds the same channel/Items/Service growth as above, but
    // init holds two encoded identities: 3,637 + 2,177 = 5,814 B engine growth.
    // Its worker has the same three credential slots and grows 13,116 B.
    // The decoded worker pick adds github_worker_pick_growth() = 1,056 B to
    // both examples and the worker-alone figure; the engine does not grow.
    // Grant validity adds GIT_VALIDITY_GROWTH = 48 B to each worker figure.
    support::owned::check_memory("../temper-init/template/temper.json", required);
    eprintln!("init requirements: engine {}, worker {} bytes", required.engine_bytes, required.worker_bytes);
    check_bounded_startup(&bounded, required);
    scratch.remove();
}

#[test]
fn program_login_refuses_invalid_input_before_creation() {
    use support::program_login::{configuration, no_deployment_resources, refused};
    for row in ["no-program", "malformed", "missing-members", "oversized", "bad-agent", "bad-name"] {
        let scratch = support::Scratch::startup(&format!("login-{row}"));
        let directory = scratch.path();
        let programs = directory.join("programs");
        configuration(
            directory,
            (row != "no-program").then_some(programs.as_path()),
            std::path::Path::new("/nonexistent/claude"),
        );
        match row {
            "malformed" => fs::write(directory.join("temper.json"), b"{").expect("malformed JSON"),
            "missing-members" => fs::write(directory.join("temper.json"), b"{}").expect("missing members"),
            "oversized" => fs::File::create(directory.join("temper.json"))
                .expect("configuration")
                .set_len(1_048_577)
                .expect("past bound"),
            "bad-agent" => fs::write(directory.join("agent.json"), b"{}").expect("bad agent"),
            _ => {}
        }
        let why = refused(directory, if row == "bad-name" { "../escape" } else { "source" }.as_ref());
        assert!(why.starts_with("temper program-login: "), "{row}: {why}");
        assert_eq!(why.lines().count(), 1, "{row}: {why}");
        assert!(!why.contains("exec:"), "{row}: {why}");
        if row == "no-program" {
            assert!(why.contains("programs.claude_code: not configured"), "{why}");
        }
        if row == "oversized" {
            assert!(why.contains("past 1048576 bytes"), "{why}");
        }
        if row == "bad-name" {
            assert!(why.contains("source name must be"), "{why}");
        }
        assert!(!programs.exists());
        no_deployment_resources(directory);
        scratch.remove();
    }
}

#[test]
fn program_login_checks_both_directory_names_before_creating_anything() {
    use support::program_login::{configuration, no_deployment_resources, refused};
    for row in
        ["program-link", "program-mode", "program-file", "source-link", "source-mode", "source-file", "source-dangling"]
    {
        let scratch = support::Scratch::startup(&format!("login-{row}"));
        let directory = scratch.path();
        let programs = directory.join("programs");
        let source = programs.join("source");
        if row.starts_with("source-") {
            fs::create_dir(&programs).expect("programs");
            fs::set_permissions(&programs, fs::Permissions::from_mode(0o700)).expect("private");
        }
        let target = if row.starts_with("source-") { &source } else { &programs };
        let expected = if row.ends_with("link") || row.ends_with("dangling") {
            symlink(directory.join("missing"), target).expect("linked leaf");
            "symbolic link"
        } else if row.ends_with("mode") {
            fs::create_dir(target).expect("directory");
            fs::set_permissions(target, fs::Permissions::from_mode(0o750)).expect("unsafe mode");
            "nonprivate mode bits 0o50"
        } else {
            fs::write(target, b"file").expect("wrong kind");
            "not a directory"
        };
        configuration(directory, Some(&programs), std::path::Path::new("/nonexistent/claude"));
        let why = refused(directory, "source".as_ref());
        assert!(why.contains(expected), "{row}: {why}");
        assert!(why.contains(&target.display().to_string()), "{row}: {why}");
        assert!(!directory.join("missing").exists(), "linked peer stays absent");
        if row.starts_with("program-") {
            assert!(fs::symlink_metadata(&source).is_err(), "missing source stays absent");
        }
        no_deployment_resources(directory);
        scratch.remove();
    }
}

#[test]
fn program_login_prepares_private_directories_through_each_supported_parent() {
    use support::program_login::{configuration, no_deployment_resources, refused};
    for row in ["missing", "private", "linked-parent"] {
        let scratch = support::Scratch::startup(&format!("login-create-{row}"));
        let directory = scratch.path();
        let programs = directory.join("programs");
        if row == "private" {
            fs::create_dir(&programs).expect("programs");
            fs::set_permissions(&programs, fs::Permissions::from_mode(0o700)).expect("private");
        }
        let configured = if row == "linked-parent" {
            symlink(directory, directory.join("parent")).expect("linked parent");
            directory.join("parent/programs")
        } else {
            programs.clone()
        };
        configuration(directory, Some(&configured), std::path::Path::new("/nonexistent/claude"));
        let why = refused(directory, "source".as_ref());
        assert!(why.starts_with("temper program-login: exec: "), "{row}: {why}");
        assert_eq!(why.lines().count(), 1);
        for path in [&programs, &programs.join("source")] {
            assert_eq!(fs::metadata(path).expect("created directory").permissions().mode() & 0o777, 0o700);
        }
        no_deployment_resources(directory);
        scratch.remove();
    }
}

#[test]
fn init_prints_written_files_account_guidance_and_explicit_start_flags_on_stderr() {
    const NO_GIT: &str =
        "init: no git on PATH: a real forge's repositories need git 2.41 or later, set as workers[].git";
    for has_git in [true, false] {
        let scratch = support::Scratch::startup(&format!("init-command-{has_git}"));
        let bin = scratch.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        let git = bin.join("git");
        if has_git {
            fs::write(&git, b"#!/bin/sh\nexit 0\n").expect("git");
            fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).expect("executable");
        }
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_temper"))
            .arg("init")
            .args(support::configuration::flags(scratch.path()))
            .env_clear()
            .env("PATH", &bin)
            .output()
            .expect("init command");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.is_empty());
        let lines = String::from_utf8(output.stderr).expect("init lines");
        let mut expected = String::new();
        for name in [
            "worker.secret",
            "worker.digest",
            "demo/llm.key",
            "demo/bin/git",
            "charters/chat.txt",
            "charters/coding.txt",
            "agent.json",
            "temper.json",
        ] {
            writeln!(expected, "init: wrote {}", scratch.path().join(name).display()).expect("string");
        }
        assert!(lines.starts_with(&expected), "files are reported in write order");
        assert!(lines.contains("init: after starting, add an OpenAI API key from the accounts page\n"));
        let path = scratch.path().display();
        assert!(
            lines.ends_with(&format!("init: start it with temper box --config {path} --data {path} --cache {path}\n"))
        );

        let diagnostics: Vec<_> = lines.lines().filter(|line| line.starts_with("init: no git")).collect();
        assert_eq!(diagnostics, if has_git { vec![] } else { vec![NO_GIT] });
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(scratch.path().join("temper.json")).expect("configuration"))
                .expect("JSON");
        assert_eq!(document["application"]["workers"][0]["git"].is_null(), !has_git);
        scratch.remove();
    }
}

#[test]
fn box_initializes_only_absent_or_empty_configuration_directories() {
    for row in ["absent", "empty", "nonempty"] {
        let scratch = support::Scratch::startup(&format!("box-directory-{row}"));
        let config = scratch.path().join("config");
        let data = scratch.path().join("data");
        let cache = scratch.path().join("cache");
        let arguments: Vec<Box<[u8]>> = [
            b"--config".as_slice(),
            config.as_os_str().as_encoded_bytes(),
            b"--data",
            data.as_os_str().as_encoded_bytes(),
            b"--cache",
            cache.as_os_str().as_encoded_bytes(),
        ]
        .iter()
        .map(|v| Box::from(*v))
        .collect();
        let (dirs, _) = temper::directories::resolve(&arguments, &[], b"/").expect("scratch directories");
        if row != "absent" {
            fs::create_dir(&config).expect("configuration");
        }
        if row == "nonempty" {
            fs::write(config.join("other"), b"kept").expect("entry");
        }
        let result = temper::box_command::configuration(&dirs);
        if row == "nonempty" {
            assert_eq!(
                result.expect_err("missing document"),
                format!("configuration {}: absent; temper init writes one", dirs.document().display())
            );
            assert_eq!(fs::read(config.join("other")).expect("kept"), b"kept");
            assert!(!data.exists());
            assert!(!cache.exists());
        } else {
            let first = result.expect("initialization");
            assert!(data.join("worker.secret").exists());
            assert!(cache.is_dir());
            assert_eq!(
                temper::box_command::configuration(&dirs).expect("second read"),
                first,
                "existing configuration is kept"
            );
        }
        scratch.remove();
    }
}

#[test]
fn owned_memory_figures_for_a_worker_alone_include_its_process_and_no_engine() {
    let scratch = support::Scratch::startup("worker-alone-memory-figures");
    let bounded = support::owned::bounded(scratch.path());
    let mut document: serde_json::Value = serde_json::from_slice(&bounded.document).expect("bounded document");
    document.as_object_mut().expect("worker-alone fixture").remove("engine");
    document["workers"][0]["engine"] = serde_json::json!({"socket":"127.0.0.1:7100"});
    document["application"] =
        serde_json::json!({"workers":document["application"]["workers"], "limits":document["application"]["limits"]});
    let input = serde_json::to_vec(&document).expect("worker-alone fixture");
    let required = temper_service::startup::memory_requirements(
        &input,
        &jig_protocol_config::Directories::working(),
        &bounded.files,
        &bounded.agent,
        &bounded.seeds,
        None,
        skein_lib::List::with_capacity(0),
        None,
    )
    .expect("worker-alone fixture");
    assert_eq!(required.engine_bytes, 0);
    let figure = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/figures/worker-alone-memory.txt");
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
        fs::write(&figure, format!("{}\n", required.worker_bytes)).expect("worker-alone fixture");
    }
    let kept: u64 =
        fs::read_to_string(figure).expect("worker-alone fixture").trim().parse().expect("worker-alone fixture");
    assert_eq!(
        required.worker_bytes, kept,
        "regenerate with SKEIN_CODEGEN_WRITE=1 heavy cargo nextest run -p temper --test startup -E 'test(owned_memory_figures)'"
    );
    document["workers"][0]["memory_bytes"] = required.worker_bytes.into();
    let part = temper_service::startup::worker_part(
        &serde_json::to_vec(&document).expect("worker-alone fixture"),
        &jig_protocol_config::Directories::working(),
    )
    .expect("worker-alone fixture");
    let service = temper_service::startup::worker_alone(
        part,
        &bounded.files,
        &bounded.agent,
        temper_service::WorkerRuntime { root: skein_io::kernel::Fd::new(9), path: b"/worker".as_slice().into() },
        7,
    )
    .expect("worker-alone fixture");
    assert_eq!(service.worst_case(), kept);
    assert!(service.setup_page(1, b"state", skein_lib::Time::ZERO).is_none());
    scratch.remove();
}
