// Fixture parsing without a deployment keeps written paths.
#![expect(clippy::disallowed_methods, reason = "ordinary tests edit UTF-8 configuration fixtures")]
#![expect(clippy::disallowed_types, clippy::disallowed_macros, reason = "ordinary step tests build diagnostic tables")]
use super::fixtures;
use crate::startup::{Problems, What, checks, lines};
use jig_protocol_config::{self as config, Path};
use temper_protocol_worker_wire as payload;

fn started(input: &[u8], files: &crate::settings::Files, agent: &crate::settings::Agent) -> Problems {
    match crate::startup(
        input,
        &jig_protocol_config::Directories::working(),
        files,
        agent,
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    ) {
        Ok(service) => {
            drop(service);
            Problems::empty()
        }
        Err(problems) => problems,
    }
}
#[test]
fn standalone_shape_is_required_of_jigs_parts() {
    let mut engine = fixtures::engine().jig;
    let mut clients = fixtures::engine().clients;
    engine.hosts.workers = Box::new([engine.hosts.workers.into_iter().next().unwrap()]);
    let mut workers = Box::new([fixtures::worker_settings().jig, fixtures::worker_settings().jig]);
    for (change, path, what) in standalone_shapes() {
        let count = apply_shape(change, &mut engine, &mut clients, &mut *workers);
        let mut found = Problems::empty();
        checks::standalone(&engine, &clients, &workers[..count], &mut found);
        assert_eq!(found.problems.len(), 1, "row {path:?}");
        assert_eq!(found.problems.get(0).unwrap().what, what, "row {path:?}");
        assert_eq!(found.problems.get(0).unwrap().path, path, "row {change:?}");
    }
}
#[test]
fn a_remote_past_the_checkouts_names_refuses_the_start() {
    let document = fixtures::with(
        &fixtures::document(),
        b"application.limits.overrides",
        br#"{"checkout.name_bytes":300,"checkout.repositories":4,"forge.name_bytes":256,"forge.resources_per_task":4}"#,
    );
    let found = crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err();
    assert_eq!(
        &*lines(&found),
        b"application.limits.overrides.checkout.name_bytes: must agree with the longest remote\n".as_slice()
    );
}
#[test]
fn the_engines_items_past_the_workers_refuse_the_start() {
    let document = fixtures::with(
        &fixtures::document(),
        b"application.limits.overrides",
        br#"{"checkout.name_bytes":600,"checkout.repositories":1,"git_component.stdout_bytes":2048}"#,
    );
    let found = started(&document, &fixtures::files(), &fixtures::startup_agent());
    assert_eq!(&*lines(&found), b"application.limits.overrides.forge.resources_per_task: must agree with checkout.repositories and checkout.name_bytes\n".as_slice());
}
#[test]
fn an_identity_past_the_grant_bound_is_refused_naming_it() {
    let mut files = fixtures::files();
    files.entries = fixtures::without(files.entries, b"secrets/forge-1.git-token");
    files
        .entries
        .push((b"secrets/forge-1.git-token".as_slice().into(), skein_lib::Secret::new(Box::new([b'x'; 8192]))))
        .unwrap();
    let found = started(&fixtures::document(), &files, &fixtures::startup_agent());
    assert_eq!(&*lines(&found), b"application.identities[0]: past its bound\n".as_slice());
}
#[test]
fn default_engine_and_worker_grants_fit_the_channel_and_link() {
    let own = crate::limits::profile(crate::limits::Profile::Small);
    for limits in [crate::translate::limits(&own).unwrap(), crate::worker_settings::payload_limits(&own).unwrap()] {
        assert_eq!(payload::largest(&limits).unwrap().grant, 7057);
        assert_eq!(jig_protocol_worker_wire::CEILINGS.application_value_secret, 8192);
        let worker = fixtures::worker_settings();
        let link = crate::worker_settings::link_limits(&worker.jig, &limits).unwrap();
        assert!(jig_protocol_worker_link::worst_case(&link).is_some());
    }
    assert!(started(&fixtures::document(), &fixtures::files(), &fixtures::startup_agent()).is_empty());
    let worker =
        crate::startup::worker_part(&fixtures::worker_alone_document(), &config::Directories::working()).unwrap();
    assert_eq!(worker.limits.forges.installations, crate::limits::FORGE_INSTALLATIONS);
    assert_eq!(worker.limits.forge_component.token_bytes, crate::limits::FORGE_TOKEN_BYTES);
}

#[test]
fn installations_past_the_grant_bound_refuse_the_start_naming_the_limit() {
    // Nine entries fit; ten entries are the first count past jig's ceiling.
    let document = fixtures::with(&fixtures::document(), b"application.limits.overrides", br#"{"checkout.name_bytes":600,"checkout.repositories":4,"git_component.stdout_bytes":2048,"forge.name_bytes":256,"forge.resources_per_task":4,"forges.installations":10,"forge_component.installations":10}"#);
    let found = started(&document, &fixtures::files(), &fixtures::startup_agent());
    assert_eq!(&*lines(&found), b"engine.limits.application_value_secret: 8625 bytes, past its bound of 8192\nworkers.limits.application_value_secret: 8625 bytes, past its bound of 8192\n".as_slice());
    let document = fixtures::with(
        &fixtures::worker_alone_document(),
        b"application.limits.overrides",
        br#"{"forges.installations":10,"forge_component.installations":10}"#,
    );
    let found = crate::startup::worker_part(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(
        &*lines(&found),
        b"workers.limits.application_value_secret: 8625 bytes, past its bound of 8192\n".as_slice()
    );
}

#[test]
fn a_worker_alone_takes_installation_and_token_bounds_from_its_configuration() {
    let document = fixtures::with(
        &fixtures::worker_alone_document(),
        b"application.limits.overrides",
        br#"{"forges.installations":3,"forge_component.token_bytes":64}"#,
    );
    let part = crate::startup::worker_part(&document, &config::Directories::working()).unwrap();
    let limits = crate::worker_settings::payload_limits(&part.limits).unwrap();
    assert_eq!(limits.entries_entries, 3);
    assert_eq!(limits.entry_token, 64);
    let engine_limits = crate::translate::limits(&part.limits).unwrap();
    assert_eq!(engine_limits.entries_entries, limits.entries_entries);
    assert_eq!(engine_limits.entry_token, limits.entry_token);
}

#[test]
fn an_endpoint_or_model_missing_from_the_agent_is_named() {
    let document = fixtures::document();
    for (row, endpoints, models, expected) in [
        (
            "endpoint",
            false,
            true,
            b"engine.llm.endpoints[0]: not in the agent's configuration under the same name\n".as_slice(),
        ),
        ("model", true, false, b"charter 1.model: not in the agent's configuration under the same name\ncharter 2.model: not in the agent's configuration under the same name\ncharter 3.model: not in the agent's configuration under the same name\ncharter 4.model: not in the agent's configuration under the same name\n"),
    ] {
        let mut agent = fixtures::startup_agent();
        if !endpoints {
            agent.endpoints = smith_protocol_channel::Endpoints::new(skein_lib::List::with_capacity(0));
        }
        if !models {
            agent.bounds.models = Box::new([]);
        }
        let found = started(&document, &fixtures::files(), &agent);
        assert_eq!(&*lines(&found), expected, "row {row}");
    }
}
#[test]
fn git_component_limits_against_the_checkout_name_both() {
    let mut own = crate::limits::profile(crate::limits::Profile::Small);
    own.checkout.name_bytes = 600;
    let git = temper_worker_protocol_git::Limits { stdout_bytes: 2048, ..own.git_component };
    for (git, expected) in [
        (temper_worker_protocol_git::Limits { operations: 2, ..git }, b"application.limits.overrides.git_component.operations: must agree with checkout.workspaces\n".as_slice()),
        (temper_worker_protocol_git::Limits { stdout_bytes: 1024, ..git }, b"application.limits.overrides.git_component.stdout_bytes: must agree with checkout.name_bytes and checkout.conflicts * (checkout.path_bytes + 1)\n"),
        (temper_worker_protocol_git::Limits { stderr_bytes: 1, ..git }, b"application.limits.overrides.git_component.stderr_bytes: must agree with checkout.push_diagnostic\n"),
    ] {
        let mut found = Problems::empty();
        checks::limits(&own, &git, &mut found);
        assert_eq!(&*lines(&found), expected, "row {git:?}");
    }
}
#[test]
fn a_worst_case_past_memory_bytes_says_by_how_much() {
    let mut found = Problems::empty();
    checks::memory(4097, 4096, 513, 512, &mut found);
    assert_eq!(&*lines(&found), b"engine.memory_bytes: the worst case passes memory_bytes by 1 bytes\nworkers[0].memory_bytes: the worst case passes memory_bytes by 1 bytes\n".as_slice());
    let document = fixtures::with(&fixtures::document(), b"engine.memory_bytes", b"1");
    let document = fixtures::with(&document, b"workers[0].memory_bytes", b"1");
    let found = started(&document, &fixtures::files(), &fixtures::startup_agent());
    assert_eq!(found.problems.len(), 2, "both services checked before construction");
    for (index, path) in [
        (0, Path::root().member(b"engine").member(b"memory_bytes")),
        (1, Path::root().member(b"workers").child(config::Segment::Index(0)).member(b"memory_bytes")),
    ] {
        assert_eq!(found.problems.get(index).unwrap().path, path, "row {index}");
        let What::Jig(config::What::Memory { over }) = found.problems.get(index).unwrap().what else {
            panic!("memory refusal row {index}");
        };
        assert!(over > 0, "row {index}");
    }
}
#[test]
fn a_channel_limit_past_a_ceiling_names_the_limit() {
    let mut own = crate::limits::profile(crate::limits::Profile::Small);
    own.checkout.name_bytes = 600;
    let normal = payload::Bounds {
        repositories: 1,
        name_bytes: 32,
        remote_bytes: 32,
        conflicts: 0,
        path_bytes: 0,
        token_bytes: 4096,
        entries: 8,
        entry_token_bytes: 512,
        text_bytes: 256,
    };
    let mut found = Problems::empty();
    let too_many = payload::Bounds { repositories: u32::MAX, ..normal };
    checks::channel(Some(&too_many), Some(&normal), &[], &own, &mut found);
    assert_eq!(found.problems.len(), 1);
    assert_eq!(
        found.problems.get(0).unwrap().path,
        Path::root().member(b"application").member(b"limits").member(b"overrides").member(b"forge.resources_per_task")
    );
    let What::Past { bytes, .. } = found.problems.get(0).unwrap().what else {
        panic!("repository ceiling named");
    };
    assert_eq!(bytes, 4_294_967_295);
}
#[test]
fn one_ios_entities_hold_both_services_and_their_children() {
    let document = fixtures::with(&fixtures::document(), b"application.limits.overrides", br#"{"checkout.name_bytes":600,"checkout.repositories":4,"git_component.stdout_bytes":2048,"git_component.operations":13}"#);
    let found = crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err();
    assert_eq!(&*lines(&found), b"engine.limits.io.sockets: 68 bytes, past its bound of 64\n".as_slice());
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    TwoWorkers,
    WorkerSocket,
    HostSocket,
    ClientProcess,
    ClientRemote,
    Connectors,
    Program,
    Configuration,
}
fn apply_shape(
    shape: Shape,
    engine: &mut config::Engine,
    clients: &mut crate::startup::application::Clients,
    workers: &mut [config::Worker],
) -> usize {
    use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    use skein_io::{Addr, Name};
    let socket = config::Address::Addr(Addr::Ip(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7100))));
    let local = config::Address::Addr(Addr::InProcess(Name::new(b"workers").unwrap()));
    engine.hosts.listener = Box::new([local.clone()]);
    clients.listener = socket.clone();
    engine.connectors = Box::new([1]);
    for worker in workers.iter_mut() {
        worker.engine = local.clone();
        worker.agent.program = b"/opt/temper".as_slice().into();
        worker.agent.configuration = b"/opt/agent.json".as_slice().into();
    }
    match shape {
        Shape::TwoWorkers => return 2,
        Shape::WorkerSocket => workers[0].engine = socket,
        Shape::HostSocket => engine.hosts.listener = Box::new([socket]),
        Shape::ClientProcess => clients.listener = local,
        Shape::ClientRemote => {
            clients.listener =
                config::Address::Addr(Addr::Ip(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 7100))));
        }
        Shape::Connectors => engine.connectors = Box::new([1, 2]),
        Shape::Program => workers[0].agent.program = b"temper".as_slice().into(),
        Shape::Configuration => workers[0].agent.configuration = b"agent.json".as_slice().into(),
    }
    1
}

fn standalone_shapes() -> [(Shape, Path, What); 8] {
    [
        (Shape::TwoWorkers, Path::root().member(b"workers"), What::Standalone),
        (
            Shape::WorkerSocket,
            Path::root().member(b"workers").child(config::Segment::Index(0)).member(b"engine"),
            What::Standalone,
        ),
        (Shape::HostSocket, Path::root().member(b"workers"), What::EngineAlone),
        (
            Shape::ClientProcess,
            Path::root().member(b"application").member(b"clients").member(b"listener"),
            What::Loopback,
        ),
        (
            Shape::ClientRemote,
            Path::root().member(b"application").member(b"clients").member(b"listener"),
            What::Loopback,
        ),
        (Shape::Connectors, Path::root().member(b"engine").member(b"connectors"), What::Connectors),
        (
            Shape::Program,
            Path::root().member(b"workers").child(config::Segment::Index(0)).member(b"agent").member(b"program"),
            What::Absolute,
        ),
        (
            Shape::Configuration,
            Path::root().member(b"workers").child(config::Segment::Index(0)).member(b"agent").member(b"configuration"),
            What::Absolute,
        ),
    ]
}

#[test]
fn a_webhook_secret_without_a_listener_is_refused() {
    let document = fixtures::with(&fixtures::webhook_document(), b"application.webhooks", b"null");
    assert_eq!(
        &*lines(&crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"application.forges[0].webhook_secret: needs application.webhooks\n"
    );
}
#[test]
fn a_webhook_listener_off_loopback_is_refused() {
    let document = fixtures::with(
        &fixtures::webhook_document(),
        b"application.webhooks.listener",
        br#"{"socket":"192.0.2.1:7200"}"#,
    );
    assert_eq!(
        &*lines(&crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"application.webhooks.listener: must be a socket address on loopback\n"
    );
}
#[test]
fn a_webhook_listener_equal_to_clients_is_refused() {
    let document = fixtures::with(
        &fixtures::webhook_document(),
        b"application.webhooks.listener",
        br#"{"socket":"127.0.0.1:7100"}"#,
    );
    assert_eq!(
        &*lines(&crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"application.webhooks.listener: must be unique\n"
    );
}
#[test]
fn webhook_listeners_on_port_zero_do_not_clash() {
    let document =
        fixtures::with(&fixtures::webhook_document(), b"application.webhooks.listener", br#"{"socket":"127.0.0.1:0"}"#);
    let document = fixtures::with(&document, b"application.clients.listener", br#"{"socket":"127.0.0.1:0"}"#);
    let named = crate::named(&document, &jig_protocol_config::Directories::working()).unwrap();
    assert_eq!(named.webhooks.unwrap().listener, named.clients);
}
#[test]
fn a_webhook_secret_past_its_bound_is_refused() {
    let mut files = fixtures::files();
    files.entries = fixtures::without(files.entries, b"secrets/forge-1.webhook-secret");
    files
        .entries
        .push((b"secrets/forge-1.webhook-secret".as_slice().into(), skein_lib::Secret::new(Box::new([b'x'; 129]))))
        .unwrap();
    let found = started(&fixtures::webhook_document(), &files, &fixtures::startup_agent());
    assert_eq!(&*lines(&found), b"application.forges[0].webhook_secret: past its bound\n");
}
#[test]
fn hook_connections_past_the_process_sockets_are_refused() {
    let document =
        fixtures::with(&fixtures::webhook_document(), b"application.limits.overrides", br#"{"checkout.name_bytes":600,"checkout.repositories":4,"git_component.stdout_bytes":2048,"forge_component.hooks":56}"#);
    assert_eq!(
        &*lines(&crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"engine.limits.io.sockets: 85 bytes, past its bound of 64\n"
    );
}
#[test]
fn the_shipped_profile_admits_a_webhook_listener() {
    let found = started(&fixtures::webhook_document(), &fixtures::files(), &fixtures::startup_agent());
    assert_eq!(&*lines(&found), b"");
}
#[test]
fn a_bundle_limit_of_zero_refuses_the_start() {
    for (overrides, expected) in [
        (
            br#"{"clients.bundle_files":0}"#.as_slice(),
            "application.limits.overrides.clients.bundle_files: must agree with a nonempty bundle\n",
        ),
        (
            br#"{"clients.bundle_bytes":0}"#.as_slice(),
            "application.limits.overrides.clients.bundle_bytes: must agree with a nonempty bundle\n",
        ),
    ] {
        let document = fixtures::with(&fixtures::document(), b"application.limits.overrides", overrides);
        let found = crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err();
        assert_eq!(&*lines(&found), expected.as_bytes(), "row {overrides:?}");
    }
}

#[test]
fn fixed_redirect_ports_reject_listeners_on_the_reserved_number() {
    use crate::startup::Listener;
    for (port, listener, expected) in [
        (7100, Listener::Clients, "redirect_port: fixed port conflicts with application.clients.listener\n"),
        (7101, Listener::Webhooks, "redirect_port: fixed port conflicts with application.webhooks.listener\n"),
        (7100, Listener::Owned(2), "redirect_port: fixed port conflicts with application.setup[2].mode.owned.listen\n"),
        (7100, Listener::Demo, "redirect_port: fixed port conflicts with application.demo.llm.listen\n"),
        (7100, Listener::Fake(2), "redirect_port: fixed port conflicts with application.setup[2].mode.fake.listen\n"),
    ] {
        let mut problems = Problems::empty();
        checks::redirect_ports(&fixed_redirects(), Some(port), listener, &mut problems);
        assert_eq!(problems.problems.len(), 1, "{listener:?} {port:?}");
        assert_eq!(problems.problems.get(0).unwrap().what, What::Redirect(listener));
        assert_eq!(lines(&problems).as_ref(), expected.as_bytes());
    }
}
#[test]
fn fixed_redirect_ports_accept_listeners_without_a_reserved_number() {
    use crate::startup::Listener;
    for (port, listener) in [(Some(7102), Listener::Clients), (Some(0), Listener::Clients), (None, Listener::Webhooks)]
    {
        let mut problems = Problems::empty();
        checks::redirect_ports(&fixed_redirects(), port, listener, &mut problems);
        assert_eq!(problems.problems.len(), 0, "{listener:?} {port:?}");
    }
}
#[test]
fn provider_redirect_ports_are_checked_before_files_are_read() {
    const PROVIDERS: &[u8] = br#"[{"name":"Fake","number":1,"signed_in_plan":false,"models":["small"],"sources":{"sign_in":{"issuer":"https://issuer.example","client":"client","scopes":[],"authorization":"https://issuer.example/auth","token":"https://issuer.example/token","keys":"https://issuer.example/keys","transport":"tls","server_name":"issuer.example","must_grant":[],"redirect_path":"callback","redirect_port":7100}}}]"#;
    let input = fixtures::with(&fixtures::document(), b"engine.llm.providers", PROVIDERS);
    let input = fixtures::with(&input, b"application.limits.overrides", br#"{"checkout.name_bytes":600,"checkout.repositories":4,"git_component.stdout_bytes":2048,"git_component.operations":2,"checkout.workspaces":2,"forge_component.hooks":1}"#);
    let input = core::str::from_utf8(&input).unwrap();
    for (listener, expected) in [
        (r#"{"socket":"127.0.0.1:7100"}"#, true),
        (r#"{"socket":"[::1]:7100"}"#, true),
        (r#"{"socket":"127.0.0.1:7102"}"#, false),
        (r#"{"socket":"127.0.0.1:0"}"#, false),
    ] {
        let document = fixtures::with(input.as_bytes(), b"application.clients.listener", listener.as_bytes());
        let result = crate::named(&document, &jig_protocol_config::Directories::working());
        if expected {
            assert_eq!(&*lines(&result.unwrap_err()), b"engine.llm.providers[0].sources.sign_in.redirect_port: fixed port conflicts with application.clients.listener\n");
        } else {
            let named = result.unwrap();
            assert_eq!(named.redirects.len(), 1);
            assert_eq!(&*named.redirects[0].redirect_path, b"callback");
        }
    }
    let hooks = input.replace("\"webhooks\":null", r#""webhooks":{"listener":{"socket":"[::1]:7100"}}"#);
    let hooks = fixtures::with(hooks.as_bytes(), b"application.clients.listener", br#"{"socket":"127.0.0.1:7102"}"#);
    assert_eq!(&*lines(&crate::named(&hooks, &jig_protocol_config::Directories::working()).unwrap_err()), b"engine.llm.providers[0].sources.sign_in.redirect_port: fixed port conflicts with application.webhooks.listener\n");
    let ephemeral = input.replace(",\"redirect_port\":7100", "");
    assert!(
        crate::named(ephemeral.as_bytes(), &jig_protocol_config::Directories::working()).unwrap().redirects.is_empty()
    );
    let input = input.replace("\"redirect_port\":7100", "\"redirect_port\":0");
    assert_eq!(
        &*lines(&crate::named(input.as_bytes(), &jig_protocol_config::Directories::working()).unwrap_err()),
        b"engine.llm.providers[0].sources.sign_in.redirect_port: invalid form\n"
    );
}

#[test]
fn every_offered_adapter_must_be_declared_by_the_agent() {
    let document = fixtures::with(
        &fixtures::document(),
        b"engine.llm.providers",
        br#"[{"name":"Fake","number":1,"manage_usage":"https://claude.ai/settings/usage","models":["small"],"sources":{"programs":["claude_code"]}}]"#,
    );
    let document = jig_protocol_config::document(
        &document,
        &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 },
    )
    .unwrap();
    let engine_at = config::read::Object::new(&document, 0).unwrap().member(&document, b"engine").unwrap();
    let mut engine =
        config::engine(&document, engine_at, &jig_protocol_config::Directories::working(), config::Defaults::Passed)
            .unwrap();
    let defaults = crate::stock::defaults(&engine.llm);
    config::pass(&mut engine, defaults).unwrap();
    for declared in [false, true] {
        let mut agent = fixtures::startup_agent();
        if declared {
            agent.programs = Box::new([crate::settings::Program {
                adapter: crate::settings::Adapter::ClaudeCode,
                path: b"/bin/claude".as_slice().into(),
                directory: b"/programs".as_slice().into(),
            }]);
        }
        let mut problems = Problems::empty();
        checks::agent(&engine, &agent, &mut problems);
        assert_eq!(
            &*lines(&problems),
            if declared {
                b"".as_slice()
            } else {
                b"engine.llm.providers[0].sources.programs[0]: program adapter not declared in the agent's programs\n"
                    .as_slice()
            }
        );
        let mut no_offers = fixtures::engine().jig;
        no_offers.llm.providers = Box::new([]);
        let mut problems = Problems::empty();
        checks::agent(&no_offers, &agent, &mut problems);
        assert!(problems.is_empty(), "no offers need no declaration");
    }
}
#[test]
fn agent_model_bounds_are_checked_under_the_configured_name() {
    for window in [true, false] {
        let engine = fixtures::engine().jig;
        let mut agent = fixtures::startup_agent();
        if window {
            agent.bounds.models[0].window = 1;
        } else {
            agent.bounds.models[0].output = 1;
        }
        let mut problems = Problems::empty();
        checks::agent(&engine, &agent, &mut problems);
        assert_eq!(
            &*lines(&problems),
            b"charter 1.model: not in the agent's configuration under the same name\ncharter 2.model: not in the agent's configuration under the same name\ncharter 3.model: not in the agent's configuration under the same name\ncharter 4.model: not in the agent's configuration under the same name\n"
        );
    }
}

#[test]
fn credential_capacity_is_checked_on_both_sides_and_against_smith() {
    let mut engine = fixtures::engine().jig;
    let mut worker = fixtures::worker_settings().jig.limits;
    let agent = fixtures::worker_agent();
    let bound = agent.limits.bodies.credential_value_bytes;
    for (sent, received, accepted) in
        [(bound, bound, true), (bound - 1, bound, true), (bound, bound - 1, false), (bound + 1, bound + 1, false)]
    {
        engine.limits.token = sent;
        worker.token = received;
        let mut problems = Problems::empty();
        checks::grants(&engine, &worker, &agent, &mut problems);
        assert_eq!(problems.is_empty(), accepted, "{sent} -> {received}");
        if !accepted {
            assert!(core::str::from_utf8(&lines(&problems)).unwrap().contains("accounts.token"));
        }
    }
}

#[test]
fn configured_keys_use_the_token_bound_instead_of_the_moved_file_ceiling() {
    let input = fixtures::with(
        fixtures::ENGINE,
        b"llm.providers",
        br#"[{"name":"Fake","number":1,"models":["small"],"sources":{"keys":true}}]"#,
    );
    let document =
        config::document(&input, &config::DocumentLimits { bytes: 65_536, tokens: 4096, depth: 32 }).unwrap();
    let mut engine =
        config::engine(&document, 0, &jig_protocol_config::Directories::working(), config::Defaults::Passed).unwrap();
    let defaults = crate::stock::defaults(&engine.llm);
    config::pass(&mut engine, defaults).unwrap();
    engine.limits.token = 1;
    let key = engine.limits.token;
    assert_eq!(crate::values::credential_bytes(&engine), key);
    let mut worker = fixtures::worker_settings().jig.limits;
    worker.token = 1;
    let mut agent = fixtures::worker_agent();
    for bytes in [key - 1, key, key + 1] {
        agent.limits.bodies.credential_value_bytes = bytes;
        let mut problems = Problems::empty();
        checks::grants(&engine, &worker, &agent, &mut problems);
        assert_eq!(problems.is_empty(), bytes >= key);
    }
    agent.limits.bodies.program_value_name = u32::try_from(smith_host_domain::SourceName::BYTES).unwrap() - 1;
    let mut problems = Problems::empty();
    checks::grants(&engine, &worker, &agent, &mut problems);
    assert!(core::str::from_utf8(&lines(&problems)).unwrap().contains("program_value_name"));
}

fn fixed_redirects() -> [crate::startup::RedirectPort; 2] {
    [7100, 7101].map(|port| crate::startup::RedirectPort {
        port,
        path: Path::root().member(b"redirect_port"),
        redirect_path: b"callback".as_slice().into(),
    })
}

#[test]
fn admission_names_global_and_time_requirements_with_their_units() {
    use skein_lib::Duration;
    for (time, waiting, spend, inbox, unacknowledged, expected) in [
        (
            119_999,
            60_000,
            100,
            8,
            8,
            "charter 1.time_ms: needs 120000 ms of run time; the agent's profile.policy.max_time allows 119999 ms\n",
        ),
        (
            300_000,
            9_999,
            100,
            8,
            8,
            "charter 1.waiting_ms: needs 10000 ms of waiting; the agent's profile.policy.max_waiting allows 9999 ms\n",
        ),
        (
            300_000,
            60_000,
            9,
            8,
            8,
            "rules.maximum_run_spend: needs a spend allowance of 40; the agent's profile.policy.max_spend allows 9\n",
        ),
        (
            300_000,
            60_000,
            100,
            1,
            1,
            "engine.limits: needs an inbox of 2 (tasks.run_inbox); the agent's profile.policy.inbox allows 1\n",
        ),
        (
            300_000,
            60_000,
            100,
            2,
            0,
            "engine.limits: needs an unacknowledged-turn allowance of 1 (hub.turns); the agent's profile.policy.unacknowledged allows 0\n",
        ),
    ] {
        let mut agent = fixtures::startup_agent();
        agent.bounds.time = Duration::from_millis(time);
        agent.bounds.waiting = Duration::from_millis(waiting);
        agent.bounds.spend = spend;
        agent.bounds.inbox = inbox;
        agent.bounds.unacknowledged = unacknowledged;
        let engine = fixtures::guard_engine().jig;
        let core = config::core(&engine, [7; 16], 7);
        let mut found = Problems::empty();
        checks::admission(&core, &engine, &agent, &mut found);
        assert_eq!(&*lines(&found), expected.as_bytes());
    }
}

#[test]
fn each_document_shape_is_read_from_its_members() {
    use crate::startup::{Shape, shape};
    for (document, expected) in [
        (fixtures::document(), Shape::Standalone),
        (fixtures::engine_alone_document(), Shape::EngineAlone),
        (fixtures::worker_alone_document(), Shape::WorkerAlone),
    ] {
        assert_eq!(shape(&document, &config::Directories::working()), Ok(expected));
    }
}

type ShapeRow = (&'static [u8], &'static [u8], Vec<(Path, What)>);

fn shape_problems(input: &[u8]) -> Vec<(Path, What)> {
    crate::startup::shape(input, &config::Directories::working())
        .unwrap_err()
        .problems
        .iter()
        .map(|problem| (problem.path.clone(), problem.what))
        .collect()
}

#[test]
fn an_engine_alone_names_each_shape_departure() {
    use config::Segment::Index;
    let root = Path::root();
    let hosts = root.member(b"engine").member(b"hosts");
    let listener = hosts.member(b"listener");
    let base = fixtures::engine_alone_document();
    let rows: &[ShapeRow] = &[
        (b"workers", b"[{}]", vec![(root.member(b"workers"), What::EngineAlone)]),
        (
            b"application.workers",
            br#"[{"root":"worker-root","git":null}]"#,
            vec![(root.member(b"application").member(b"workers"), What::EngineAlone)],
        ),
        (
            b"engine.hosts.listener",
            br#"[{"socket":"127.0.0.1:0"},{"in_process":"workers"}]"#,
            vec![(listener.clone(), What::EngineAlone), (listener.child(Index(1)), What::EngineAlone)],
        ),
        (
            b"engine.hosts.listener",
            br#"[{"socket":"127.0.0.1:0"},{"socket":"[::1]:0"}]"#,
            vec![(listener.clone(), What::EngineAlone)],
        ),
        (
            b"engine.hosts.listener",
            br#"[{"socket":"192.0.2.1:7000"}]"#,
            vec![(listener.child(Index(0)), What::Jig(config::What::Plaintext))],
        ),
        (
            b"engine.hosts.listener",
            br#"[{"socket":"127.0.0.1:0","transport":"tls"}]"#,
            vec![(listener.child(Index(0)).member(b"transport"), What::Jig(config::What::Unknown))],
        ),
        (b"engine.hosts.workers", b"[]", vec![(hosts.member(b"workers"), What::EngineAlone)]),
        (
            b"engine.hosts.workers",
            br#"[{"name":"worker-a","digest":"same"},{"name":"worker-b","digest":"same"}]"#,
            vec![(hosts.member(b"workers").child(Index(1)).member(b"digest"), What::EngineAlone)],
        ),
    ];
    for (member, value, expected) in rows {
        let input = fixtures::with(&base, member, value);
        assert_eq!(&shape_problems(&input), expected, "{}", core::str::from_utf8(member).unwrap());
    }
}

#[test]
fn the_socket_listener_in_the_old_hostsocket_case_now_names_both_local_worker_lists() {
    let input = fixtures::with(&fixtures::document(), b"engine.hosts.listener", br#"[{"socket":"127.0.0.1:7100"}]"#);
    assert_eq!(
        shape_problems(&input),
        vec![
            (Path::root().member(b"workers"), What::EngineAlone),
            (Path::root().member(b"application").member(b"workers"), What::EngineAlone),
        ]
    );
}

#[test]
fn a_worker_alone_names_each_shape_departure() {
    use config::Segment::Index;
    let root = Path::root();
    let workers = root.member(b"workers");
    let engine = workers.child(Index(0)).member(b"engine");
    let base = fixtures::worker_alone_document();
    let rows: &[ShapeRow] = &[
        (b"workers", b"[]", vec![(workers.clone(), What::WorkerAlone)]),
        (b"workers[0].engine", br#"{"in_process":"workers"}"#, vec![(engine.clone(), What::WorkerAlone)]),
        (
            b"workers[0].engine",
            br#"{"socket":"192.0.2.1:7000"}"#,
            vec![(engine.clone(), What::Jig(config::What::Plaintext))],
        ),
        (
            b"workers[0].engine",
            br#"{"socket":"127.0.0.1:7000","transport":"tls"}"#,
            vec![(engine.member(b"transport"), What::Jig(config::What::Unknown))],
        ),
        (b"application.workers", b"[]", vec![(root.member(b"application").member(b"workers"), What::WorkerAlone)]),
        (
            b"application.workers",
            br#"[{"root":"one","git":null},{"root":"two","git":null}]"#,
            vec![(root.member(b"application").member(b"workers"), What::WorkerAlone)],
        ),
        (
            b"application",
            br#"{"workers":[{"root":"worker","git":null}],"limits":{"profile":"small","overrides":{}},"forges":[]}"#,
            vec![(root.member(b"application").member(b"forges"), What::WorkerAlone)],
        ),
    ];
    for (member, value, expected) in rows {
        let input = fixtures::with(&base, member, value);
        assert_eq!(&shape_problems(&input), expected, "{}", core::str::from_utf8(member).unwrap());
    }
    let stray = [b"{\"stray\":null,".as_slice(), &base[1..]].concat();
    assert_eq!(shape_problems(&stray), vec![(root.member(b"stray"), What::WorkerAlone)]);
    // Duplicate whole worker documents by their existing raw list body.
    let text = core::str::from_utf8(&base).unwrap();
    let (_, body) = text.split_once("\"workers\":[").unwrap();
    let (body, _) = body.split_once("],\"application\":").unwrap();
    let two = [b"[".as_slice(), body.as_bytes(), b",", body.as_bytes(), b"]"].concat();
    assert_eq!(shape_problems(&fixtures::with(&base, b"workers", &two)), vec![(workers, What::WorkerAlone)]);
}

#[test]
fn the_cluster_shapes_accept_both_loopback_families_and_the_whole_v4_range() {
    for socket in [br#"{"socket":"127.42.3.4:7100"}"#.as_slice(), br#"{"socket":"[::1]:7100"}"#] {
        let worker = fixtures::with(&fixtures::worker_alone_document(), b"workers[0].engine", socket);
        assert_eq!(
            crate::startup::shape(&worker, &config::Directories::working()),
            Ok(crate::startup::Shape::WorkerAlone)
        );
        let listener = [b"[".as_slice(), socket, b"]"].concat();
        let engine = fixtures::with(&fixtures::engine_alone_document(), b"engine.hosts.listener", &listener);
        assert_eq!(
            crate::startup::shape(&engine, &config::Directories::working()),
            Ok(crate::startup::Shape::EngineAlone)
        );
    }
}

#[test]
fn each_turn_bound_accepts_equality_and_names_one_byte_short() {
    const TURN: u32 = 65_536;
    let mut engine = config::profile(config::Profile::Small);
    let mut worker = fixtures::worker_settings().jig;
    let mut own = crate::limits::profile(crate::limits::Profile::Small);
    let mut agent = fixtures::startup_agent();
    agent.limits.bodies.turn_body = TURN;
    let loaded = TURN + u32::try_from(size_of::<temper_engine_domain::Record>()).unwrap();
    let window = u64::from(TURN + 4) * 4;
    let rows = [
        ("engine.limits.overrides.hub.turn_bytes", u64::from(TURN), "the agent's largest turn"),
        ("workers[0].limits.overrides.hub.turn_bytes", u64::from(TURN), "the agent's largest turn"),
        ("engine.limits.overrides.core.resume_bytes", u64::from(TURN + 4), "the agent's largest turn with its length"),
        ("application.limits.overrides.root.loads.transcript_bytes", u64::from(TURN), "the agent's largest turn"),
        (
            "application.limits.overrides.root.loads.bytes",
            u64::from(loaded),
            "the agent's largest turn with a loaded row",
        ),
        ("engine.limits.overrides.hub.transcript_bytes", window, "the engine's resume window"),
        ("workers[0].limits.overrides.hub.transcript_bytes", window, "the engine's resume window"),
    ];
    for (index, (path, needs, of)) in rows.into_iter().enumerate() {
        engine.core.resume_bytes = TURN + 4;
        engine.hub.turn_bytes = u64::from(TURN);
        worker.limits.hub.turn_bytes = u64::from(TURN);
        engine.hub.transcript_bytes = window;
        worker.limits.hub.transcript_bytes = window;
        own.root.loads.transcript_bytes = TURN;
        own.root.loads.bytes = loaded;
        own.root.loads.reply_bytes = loaded;
        let mut found = Problems::empty();
        checks::turns(Some(&engine), core::slice::from_ref(&worker), &own, Some(&agent), &mut found);
        assert_eq!(&*lines(&found), b"", "{path} at equality");
        match index {
            0 => engine.hub.turn_bytes -= 1,
            1 => worker.limits.hub.turn_bytes -= 1,
            2 => engine.core.resume_bytes -= 1,
            3 => own.root.loads.transcript_bytes -= 1,
            4 => own.root.loads.bytes -= 1,
            5 => engine.hub.transcript_bytes -= 1,
            6 => worker.limits.hub.transcript_bytes -= 1,
            _ => unreachable!("seven turn-bound relations"),
        }
        checks::turns(Some(&engine), core::slice::from_ref(&worker), &own, Some(&agent), &mut found);
        assert_eq!(&*lines(&found), format!("{path}: {} bytes, short of {of}, {needs} bytes\n", needs - 1).as_bytes(),);
    }
}

#[test]
fn turn_checks_use_only_what_each_process_knows() {
    let engine = config::profile(config::Profile::Small);
    let own = crate::limits::profile(crate::limits::Profile::Small);
    let worker = fixtures::worker_settings().jig;
    let mut agent = fixtures::startup_agent();
    // The small hub's 262144 bytes hold its 65536-byte resume window exactly.
    let mut found = Problems::empty();
    checks::turns(Some(&engine), &[], &own, None, &mut found);
    assert_eq!(&*lines(&found), b"");
    let mut short_engine = engine;
    short_engine.hub.transcript_bytes -= 1;
    checks::turns(Some(&short_engine), &[], &own, None, &mut found);
    assert_eq!(&*lines(&found), b"engine.limits.overrides.hub.transcript_bytes: 262143 bytes, short of the engine's resume window, 262144 bytes\n");
    let mut found = Problems::empty();
    agent.limits.bodies.turn_body = u32::try_from(worker.limits.hub.turn_bytes).unwrap();
    checks::turns(None, core::slice::from_ref(&worker), &own, Some(&agent), &mut found);
    assert_eq!(&*lines(&found), b"");
    agent.limits.bodies.turn_body += 1;
    checks::turns(None, core::slice::from_ref(&worker), &own, Some(&agent), &mut found);
    assert_eq!(
        &*lines(&found),
        b"workers[0].limits.overrides.hub.turn_bytes: 65536 bytes, short of the agent's largest turn, 65537 bytes\n"
    );
}
