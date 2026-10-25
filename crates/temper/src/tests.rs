#[test]
fn directory_commands_take_flags_after_their_command_words() {
    for (words, valid) in [
        (vec!["init"], true),
        (vec!["init", "extra"], false),
        (vec!["serve"], true),
        (vec!["box"], true),
        (vec!["serve", "old.json"], false),
        (vec!["program-login", "name"], true),
        (vec!["program-login"], false),
        (vec!["forge", "prepare", "1"], true),
        (vec!["forge", "fetch", "1"], true),
        (vec!["forge", "run", "1", "runner"], true),
        (vec!["forge", "reconcile", "1"], true),
        (vec!["forge", "sign-in", "1"], true),
        (vec!["unknown"], false),
    ] {
        let mut words: Vec<Box<[u8]>> = words.iter().map(|v| Box::from(v.as_bytes())).collect();
        words
            .extend([b"--config".as_slice(), b"/c", b"--data", b"/d", b"--cache", b"/k"].iter().map(|v| Box::from(*v)));
        assert_eq!(super::configure_with(&words, &[], b"/").is_ok(), valid);
    }
}
const OPEN_LINK: &[u8] =
    b"http://127.0.0.1:7000/jig/v1/sign-in/link?secret=AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM".as_slice();
type OpenRow = (&'static str, &'static [u8], Result<(bool, &'static str), &'static str>);
const OPEN_ARGUMENTS: &[OpenRow] = &[
    ("link", OPEN_LINK, Ok((true, "127.0.0.1:7000"))),
    ("address", b"http://127.0.0.1:7000".as_slice(), Ok((false, "127.0.0.1:7000"))),
    ("slash", b"http://127.0.0.1:7000/".as_slice(), Ok((false, "127.0.0.1:7000"))),
    ("IPv6", b"http://[::1]:7000/".as_slice(), Ok((false, "[::1]:7000"))),
    ("loopback range", b"http://127.1.2.3:7000".as_slice(), Ok((false, "127.1.2.3:7000"))),
    ("scheme", b"https://127.0.0.1:7000".as_slice(), Err("not a link or an address: https://127.0.0.1:7000")),
    ("hostname", b"http://localhost:7000".as_slice(), Err("not a link or an address: http://localhost:7000")),
    ("port", b"http://127.0.0.1".as_slice(), Err("not a link or an address: http://127.0.0.1")),
    ("remote", b"http://10.0.0.1:7000".as_slice(), Err("the engine's address must be on loopback")),
    ("path", b"http://127.0.0.1:7000/other".as_slice(), Err("not a link or an address: http://127.0.0.1:7000/other")),
    (
        "short secret",
        b"http://127.0.0.1:7000/jig/v1/sign-in/link?secret=AA".as_slice(),
        Err("not a link or an address: http://127.0.0.1:7000/jig/v1/sign-in/link?secret=AA"),
    ),
];

#[test]
fn each_open_argument_reads_as_a_link_an_address_or_a_refusal() {
    use super::open::{Refused, Target, target};
    for &(row, argument, expected) in OPEN_ARGUMENTS {
        let found = target(argument)
            .map(|target| match target {
                Target::Link { engine, link } => {
                    assert_eq!(link.expose(), argument, "{row}: link bytes");
                    (true, engine.to_string())
                }
                Target::Address { engine } => (false, engine.to_string()),
            })
            .map_err(|why: Refused| why.line(std::path::Path::new("client")));
        assert_eq!(
            found.as_ref().map(|(link, engine)| (*link, engine.as_str())).map_err(String::as_str),
            expected
                .map_err(|why| format!("temper open: {why}\n"))
                .as_ref()
                .map(|(link, engine)| (*link, *engine))
                .map_err(String::as_str),
            "{row}"
        );
    }
}
#[test]
fn open_without_two_arguments_prints_its_usage() {
    for (row, arguments, expected) in [
        ("none", vec![], "temper open: usage: temper open DIRECTORY LINK_OR_ADDRESS\n"),
        ("one", vec!["client".into()], "temper open: usage: temper open DIRECTORY LINK_OR_ADDRESS\n"),
        (
            "three",
            vec!["client".into(), "address".into(), "extra".into()],
            "temper open: usage: temper open DIRECTORY LINK_OR_ADDRESS\n",
        ),
    ] {
        let refused = super::open::start(&arguments, &skein_shell::Clock::new()).expect_err("usage refused");
        assert_eq!(refused.line(std::path::Path::new("client")), expected, "{row}");
    }
}

#[path = "../tests/support/configuration.rs"]
pub(crate) mod configuration;

#[test]
fn the_standalone_envelope_retains_no_descriptors() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let directory = std::env::temp_dir().join(format!("temper-envelope-{}", std::process::id()));
    fs::create_dir(&directory).expect("envelope scratch");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("private scratch");
    for name in ["accounts", "secrets", "charters", "state"] {
        fs::create_dir_all(directory.join(name)).expect("fixture parents");
    }
    let write = |name: &str, bytes: &[u8]| {
        let path = directory.join(name);
        fs::write(&path, bytes).expect("fixture bytes");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture");
    };
    for (name, bytes) in jig_channel_world::fixtures::files(false).entries.into_boxed() {
        write(std::str::from_utf8(&name).expect("fixture path"), bytes.expose());
    }
    for (name, bytes) in [
        ("worker.secret", b"worker-secret".as_slice()),
        ("secrets/forge-1.user", b"42\n".as_slice()),
        ("secrets/forge-1.api-token", b"forge-1-api-token-value".as_slice()),
        ("secrets/forge-1.git-token", b"forge-1-git-token-value".as_slice()),
        ("root.der", skein_tls_world::pki::ROOT),
    ] {
        write(name, bytes);
    }
    let (agent, document) = configuration::documents(&directory, "/fixture/temper");
    write("agent.json", &agent);
    assert_eq!(configuration::flags(&directory).len(), 6);
    let envelope = super::envelope_in(&configuration::directories(&directory), &document).expect("standalone envelope");
    assert_no_descriptors_in(&directory);
    let mut owned = Vec::new();
    let service = super::prepare(&configuration::directories(&directory), &document, &mut owned)
        .expect("same standalone preparation");
    assert_eq!(envelope, (service.worst_case(), service.operations()));
    drop(service);
    super::close(owned);
    assert_no_descriptors_in(&directory);
    fs::remove_dir_all(directory).expect("remove envelope scratch");
}
fn assert_no_descriptors_in(directory: &std::path::Path) {
    for entry in std::fs::read_dir("/proc/self/fd").expect("open descriptor list") {
        let path = entry.expect("descriptor entry").path();
        if let Ok(target) = std::fs::read_link(&path) {
            assert!(!target.starts_with(directory), "descriptor {path:?} still holds {target:?}");
        }
    }
}

#[test]
fn ctl_is_configured() {
    for arguments in [vec!["help".into()], vec!["directory".into(), "me".into()]] {
        let words = std::iter::once("ctl".into()).chain(arguments.clone());
        assert_eq!(
            super::configure_with(
                &words.map(|v: std::ffi::OsString| Box::from(v.as_encoded_bytes())).collect::<Vec<_>>(),
                &[],
                b"/"
            ),
            Ok(super::Command::Ctl { arguments })
        );
    }
}

#[test]
fn fixed_sign_in_ports_are_checked_against_owned_and_fake_listeners() {
    // Resolve the fixture's data paths as a deployment does.
    let example = include_bytes!("../../../docs/examples/owned.json");
    let mut value: serde_json::Value = serde_json::from_slice(example).expect("owned JSON");
    value["engine"]["llm"]["providers"] = serde_json::json!([{
        "name":"OpenAI", "number":1, "signed_in_plan":false, "models":["gpt-6-luna"], "sources":{
            "sign_in": {"issuer":"https://issuer.example", "client":"client", "scopes":[],
                "authorization":"https://issuer.example/auth", "token":"https://issuer.example/token",
                "keys":"https://issuer.example/keys", "transport":"tls", "server_name":"issuer.example",
                "must_grant":[], "redirect_path":"callback", "redirect_port":3000}
        }
    }]);
    // Keep git concurrency at one so the small profile's 48 sockets reach the
    // redirect-port check after this fixture adds an OAuth listener.
    value["application"]["limits"]["overrides"]["git_component.operations"] = 1.into();
    value["application"]["limits"]["overrides"]["checkout.workspaces"] = 1.into();
    let mut fake = value.clone();
    configuration::fake_entry(&mut fake["application"]["setup"][0], "127.0.0.1:3000");
    let mut demo = fake.clone();
    demo["engine"]["llm"]["providers"][0]["sources"]["sign_in"]["redirect_port"] = 8370.into();
    let fixture = configuration::demo_document(std::path::Path::new("/config"), "127.0.0.1:8370");
    for field in ["providers", "endpoints", "models"] {
        let member = fixture["engine"]["llm"][field].as_array().expect("LLM table").last().expect("demo member");
        demo["engine"]["llm"][field].as_array_mut().expect("LLM table").push(member.clone());
    }
    demo["application"]["demo"] = fixture["application"]["demo"].clone();
    let mut bad_demo = demo.clone();
    bad_demo["application"]["demo"]["project"] = 9.into();
    for (document, expected) in [
        (&bad_demo, "application.demo.project: names no project\n"),
        (
            &demo,
            "engine.llm.providers[0].sources.sign_in.redirect_port: fixed port conflicts with application.demo.llm.listen\n",
        ),
        (
            &value,
            "engine.llm.providers[0].sources.sign_in.redirect_port: fixed port conflicts with application.setup[0].mode.owned.listen\n",
        ),
        (
            &fake,
            "engine.llm.providers[0].sources.sign_in.redirect_port: fixed port conflicts with application.setup[0].mode.fake.listen\n",
        ),
    ] {
        assert_eq!(
            super::setup::check(
                &serde_json::to_vec(document).expect("local JSON"),
                &jig_protocol_config::Directories::given(b"/config", b"/data", b"/cache").expect("fixture directories")
            )
            .expect_err("local listen conflict"),
            expected
        );
    }
    // The same API on a connected forge belongs to the remote peer, not this process.
    value["application"]["setup"] = serde_json::json!([]);
    let deployment = super::setup::check(
        &serde_json::to_vec(&value).expect("connected JSON"),
        &jig_protocol_config::Directories::given(b"/config", b"/data", b"/cache").expect("fixture directories"),
    )
    .expect("connected forge");
    assert!(deployment.demo.is_none());
}

#[test]
fn a_programs_owner_refusal_identifies_the_directory_ownership_conflict() {
    use smith_agent_shell::program::{Refusal, Unsafe};
    let other = skein_shell::effective_user().wrapping_add(1);
    let why = super::agent::program_directory_refusal(&Refusal::Unsafe {
        path: "/programs".into(),
        found: Unsafe::Owner { found: other },
    });
    assert_eq!(
        why,
        format!("workers[0].agent.configuration: programs.claude_code.directory: /programs: owned by user {other}")
    );
}

#[test]
fn program_login_refuses_arguments_outside_the_required_source_name() {
    for arguments in [
        vec!["program-login"],
        vec!["program-login", "source", "extra"],
        vec!["program-login", "config.json", "source", "extra"],
        vec!["sign-in", "source"],
    ] {
        assert_eq!(configure_program_login(&arguments), Err(format!("temper: invalid arguments\n{}", super::usage())));
    }
}
#[test]
fn program_login_accepts_exactly_the_required_source_name() {
    let directories =
        super::directories::resolve(&[], &[(Box::from(b"HOME".as_slice()), Box::from(b"/home/test".as_slice()))], b"/")
            .expect("default directories")
            .0;
    assert_eq!(
        configure_program_login(&["program-login", "source"]),
        Ok(super::Command::ProgramLogin { directories, name: "source".into() })
    );
}

#[test]
fn forge_child_names_its_death_signal_or_prints_its_usage() {
    use skein_io::kernel::Signal;
    for (word, signal) in [("kill", Signal::Kill), ("terminate", Signal::Terminate)] {
        assert_eq!(
            configure_program_login(&[
                "forge",
                "child",
                "--parent",
                "7",
                "--signal",
                word,
                "--",
                "/bin/forgejo",
                "web"
            ]),
            Ok(super::Command::ForgeChild {
                parent: 7,
                signal,
                program: "/bin/forgejo".into(),
                arguments: vec!["web".into()],
            })
        );
    }
    let usage = "temper forge child: usage: temper forge child --parent PID --signal kill|terminate -- PROGRAM ARGS…\n";
    for arguments in [
        vec!["forge", "child", "--parent", "7", "--", "/bin/forgejo"],
        vec!["forge", "child", "--parent", "7", "--signal", "interrupt", "--", "/bin/forgejo"],
        vec!["forge", "child", "--signal", "kill", "--parent", "7", "--", "/bin/forgejo"],
    ] {
        assert_eq!(configure_program_login(&arguments), Err(usage.into()), "{arguments:?}");
    }
}

fn configure_program_login(arguments: &[&str]) -> Result<super::Command, String> {
    super::configure_with(
        &arguments.iter().map(|value| Box::from(value.as_bytes())).collect::<Vec<_>>(),
        &[(Box::from(b"HOME".as_slice()), Box::from(b"/home/test".as_slice()))],
        b"/",
    )
}

#[test]
fn forge_commands_refuse_managed_and_fake_entries_by_name() {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join(format!("temper-not-owned-{}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&directory).expect("command scratch");
    let (_, document) = configuration::documents(&directory, "/fixture/temper");
    let mut managed: serde_json::Value = serde_json::from_slice(&document).expect("document");
    managed["application"]["setup"] = serde_json::json!([{
        "forge":1, "directory":"forge-1", "bot":{"login":"temper","email":"temper@example.test"},
        "orgs":["acme"], "mode":{"managed":{"admin_token":"admin_token"}}
    }]);
    managed["application"]["forges"][0]["api_token"] = "forge-1/api_token".into();
    managed["application"]["forges"][0]["user"] = serde_json::json!({"file":"forge-1/user"});
    managed["application"]["identities"][0]["token"] = "forge-1/git_token".into();
    let mut fake = managed.clone();
    configuration::fake_entry(&mut fake["application"]["setup"][0], "127.0.0.1:9");
    for (document, arguments, expected) in [
        (&managed, vec!["prepare", "1"], "temper forge prepare: forge 1: it is managed, not owned\n"),
        (&managed, vec!["run", "1", "forgejo"], "temper forge run: forge 1: it is managed, not owned\n"),
        (&fake, vec!["run", "1", "forgejo"], "temper forge run: forge 1: it is fake, not owned\n"),
    ] {
        std::fs::write(directory.join("temper.json"), serde_json::to_vec(document).expect("document"))
            .expect("write document");
        let refused = super::forge::start(
            &configuration::directories(&directory),
            &arguments.iter().map(std::ffi::OsString::from).collect::<Vec<_>>(),
            None,
        )
        .expect_err("not owned");
        assert_eq!(refused.line(), expected);
    }
    assert!(!directory.join("forge-1").exists());
    std::fs::remove_dir_all(directory).expect("remove command scratch");
}

pub(crate) fn worker_document(directory: &std::path::Path) -> (Vec<u8>, serde_json::Value) {
    let (agent, input) = configuration::documents(directory, "/missing/temper");
    let mut input: serde_json::Value = serde_json::from_slice(&input).expect("fixture document");
    input.as_object_mut().expect("document object").remove("engine");
    input["workers"][0]["engine"] = serde_json::json!({"socket":"127.0.0.1:7100"});
    input["application"] =
        serde_json::json!({"workers":input["application"]["workers"],"limits":input["application"]["limits"]});
    (agent, input)
}
#[test]
fn serve_prepares_a_worker_alone_without_a_deployment_or_setup_host() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let directory = std::env::temp_dir().join(format!("temper-worker-alone-{}", std::process::id()));
    fs::create_dir(&directory).expect("worker-alone fixture");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("worker-alone fixture");
    let (agent, input) = worker_document(&directory);
    fs::write(directory.join("agent.json"), agent).expect("worker-alone fixture");
    fs::write(directory.join("root.der"), skein_tls_world::pki::ROOT).expect("worker-alone fixture");
    let secret = directory.join("worker.secret");
    fs::write(&secret, b"worker-secret").expect("worker-alone fixture");
    fs::set_permissions(secret, fs::Permissions::from_mode(0o600)).expect("worker-alone fixture");
    let mut owned = Vec::new();
    let (service, store) = super::prepare_with_store(
        &configuration::directories(&directory),
        &serde_json::to_vec(&input).expect("worker-alone fixture"),
        &mut owned,
    )
    .expect("worker-alone fixture");
    assert!(store.is_empty(), "no deployment root");
    assert_eq!(owned.len(), 1, "only the worker root");
    assert!(service.setup_page(1, b"state", skein_lib::Time::ZERO).is_none());
    assert!(!directory.join("state").exists(), "no store directory created");
    super::close(owned);
    fs::remove_dir_all(directory).expect("worker-alone fixture");
}

#[test]
fn unresolved_forge_names_prepare_without_refusing_or_resolving_socket_forges() {
    fn reject_names(configured: &jig_protocol_config::Address) -> Result<std::net::SocketAddr, String> {
        match configured {
            jig_protocol_config::Address::Host { .. } => Err("no resolver in this test".into()),
            jig_protocol_config::Address::Addr(address) => Ok(address.socket().expect("socket fixture")),
        }
    }
    let (_, bytes) = configuration::documents(std::path::Path::new("/fixture"), "/fixture/temper");
    let mut document: serde_json::Value = serde_json::from_slice(&bytes).expect("fixture JSON");
    let mut host = document["application"]["forges"][0].clone();
    host["api"] = serde_json::json!({"host":"forge.example", "port":443});
    let scratch = skein_scratch::paths::workspace().join("target/reject-forge-names");
    std::fs::create_dir_all(&scratch).expect("test directory");
    let trust = scratch.join("root.der");
    std::fs::write(&trust, skein_tls_world::pki::ROOT).expect("fixture trust");
    host["transport"] = serde_json::json!({"tls":{"server_name":"forge.example", "trust":trust}});
    let mut socket = host.clone();
    socket["number"] = 2.into();
    socket["identity"] = 2.into();
    socket["transport"] = "plaintext".into();
    socket["api"] = serde_json::json!({"socket":"127.0.0.1:3001"});
    document["application"]["forges"] = serde_json::json!([host, socket]);
    let mut identity = document["application"]["identities"][0].clone();
    identity["number"] = 2.into();
    document["application"]["identities"].as_array_mut().expect("identities").push(identity);
    document["engine"]["limits"]["overrides"]["accounts.credentials"] = 2.into();
    let bytes = serde_json::to_vec(&document).expect("JSON");
    let named = temper_service::named(&bytes, &jig_protocol_config::Directories::working()).expect("names");
    let endpoints = super::forges(&named.forges, reject_names).expect("DNS never refuses a start");
    assert_eq!(endpoints.len(), 2);
    assert_eq!(endpoints.get(0).expect("host endpoint").address, temper_service::forge::HELD_ADDRESS);
    assert!(
        matches!(&named.forges[0].api, jig_protocol_config::Address::Host { name, .. } if name.as_ref() == b"forge.example")
    );
    assert_eq!(endpoints.get(1).expect("socket endpoint").address, ([127, 0, 0, 1], 3001).into());
    std::fs::remove_dir_all(scratch).expect("remove fixture");
}

#[test]
fn box_echo_resolves_its_default_variable_and_flag_without_consuming_other_arguments() {
    use temper_box::host::Echo;
    for (flags, variable, expected) in [
        (vec![], None, Echo::None),
        (vec![], Some("steps"), Echo::Steps),
        (vec![], Some("none"), Echo::None),
        (vec!["--echo", "steps"], Some("none"), Echo::Steps),
        (vec!["--echo", "none"], Some("steps"), Echo::None),
    ] {
        let words = ["box", "--config", "/c", "--data", "/d", "--cache", "/k"]
            .into_iter()
            .chain(flags)
            .map(|word| Box::from(word.as_bytes()))
            .collect::<Vec<_>>();
        let environment = variable
            .into_iter()
            .map(|value| (Box::from(b"TEMPER_ECHO".as_slice()), Box::from(value.as_bytes())))
            .collect::<Vec<_>>();
        let super::Command::Box { echo, .. } = super::configure_with(&words, &environment, b"/").expect("box settings")
        else {
            panic!("ordinary box")
        };
        assert_eq!(echo, expected);
    }
    for (flags, variable, origin) in [(vec!["--echo", "all"], "steps", "--echo"), (vec![], "all", "TEMPER_ECHO")] {
        let words = ["box", "--config", "/c", "--data", "/d", "--cache", "/k"]
            .into_iter()
            .chain(flags)
            .map(|word| Box::from(word.as_bytes()))
            .collect::<Vec<_>>();
        let environment = [(Box::from(b"TEMPER_ECHO".as_slice()), Box::from(variable.as_bytes()))];
        let why = super::configure_with(&words, &environment, b"/").expect_err("echo refused");
        assert!(why.starts_with(&format!("temper: {origin}: expected none or steps")), "{why}");
    }
    for flags in [
        vec!["--echo", "steps", "extra"],
        vec!["--echo", "none", "--unknown"],
        vec!["--echo", "steps", "--echo", "steps"],
    ] {
        let words = ["box", "--config", "/c", "--data", "/d", "--cache", "/k"]
            .into_iter()
            .chain(flags)
            .map(|word| Box::from(word.as_bytes()))
            .collect::<Vec<_>>();
        assert!(super::configure_with(&words, &[], b"/").is_err(), "leftover flags stay refused");
    }
}
