// Fixture parsing without a deployment keeps written paths.
//! What the root's construction asserts and the worker's rules against the
//! agent, each refused at start naming its path (protocol/configuration.md, 7).
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "ordinary test code edits UTF-8 configuration fixtures and reads their diagnostics"
)]
use super::fixtures;
use crate::startup::{Problems, checks, lines};

const ENGINE_OVERRIDES: &str = r#""tasks.result_bytes":32,"tasks.run_inbox":2,"hub.turns":1,"people.amendment_bytes":64,"core.policy_bytes":4096,"tasks.spec_bytes":128,"tasks.message_bytes":512,"tasks.hold_bytes":64,"tasks.authority_bytes":64,"people.words":512,"core.call_answer_bytes":2048,"notes.recalled":1"#;
const WORKER_OVERRIDES: &str = r#""hub.slots":1,"hub.run_calls":1,"tasks.run_inbox":2,"hub.turns":1,"tasks.message_bytes":512,"people.words":512,"core.call_answer_bytes":2048"#;

fn started(document: &[u8]) -> Box<[u8]> {
    let found = match crate::startup(
        document,
        &jig_protocol_config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
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
    };
    lines(&found)
}
fn overridden(document: &[u8], path: &[u8], shipped: &str, extra: &str) -> Box<[u8]> {
    fixtures::with(document, path, format!("{{{shipped},{extra}}}").as_bytes())
}
fn engine_with(extra: &str) -> Box<[u8]> {
    overridden(&fixtures::document(), b"engine.limits.overrides", ENGINE_OVERRIDES, extra)
}
/// The fixture engine's root as the start makes it, held to `limits`.
fn root(limits: &temper_engine_domain::Limits) -> Box<[u8]> {
    let engine = fixtures::guard_engine();
    let mut found = Problems::empty();
    let seeds = fixtures::seeds();
    let core = jig_protocol_config::core(&engine.jig, seeds.deployment, seeds.seed);
    checks::root(core, &engine, &seeds, &[], limits, &mut found);
    lines(&found)
}
fn root_limits() -> temper_engine_domain::Limits {
    let engine = fixtures::guard_engine();
    crate::limits::engine(&engine.jig.limits, &engine.limits).unwrap()
}

#[test]
fn the_fixture_document_starts() {
    assert_eq!(&*started(&fixtures::document()), b"");
    assert_eq!(&*root(&root_limits()), b"");
}
#[test]
fn a_document_seeding_no_project_starts() {
    let document = fixtures::with(&fixtures::document(), b"engine.projects", b"[]");
    let document = fixtures::with(&document, b"application.landing.projects", b"[]");
    assert_eq!(&*started(&document), b"");
}
#[test]
fn engine_slots_above_0_refuse_the_start() {
    assert_eq!(
        &*started(&engine_with(r#""fleet.engine_slots":1"#)),
        b"engine.limits.overrides.fleet.engine_slots: the root has no hub, so the fleet must place every run on a worker\n"
    );
}
#[test]
fn the_hub_check_refuses_a_charter_the_engine_may_host() {
    for hosts in [jig_domain_fleet::Kinds::Engine, jig_domain_fleet::Kinds::Both] {
        let mut engine = fixtures::guard_engine().jig;
        engine.charters[0].hosts = hosts;
        let mut found = Problems::empty();
        checks::hub(&engine, &mut found);
        assert_eq!(
            lines(&found).as_ref(),
            b"charter 1.hosts: the root has no hub, so the fleet must place every run on a worker\n"
        );
    }
}

#[test]
fn an_identity_past_the_accounts_credentials_refuses_the_start() {
    let document = fixtures::with(&fixtures::document(), b"application.identities[0].number", b"4");
    let document = fixtures::with(&document, b"application.forges[0].identity", b"4");
    assert_eq!(
        &*started(&document),
        b"application.identities[0].number: must be from 1 to accounts.credentials, in engine.limits and in the worker's limits\n"
    );
    let document = overridden(
        &fixtures::document(),
        b"workers[0].limits.overrides",
        WORKER_OVERRIDES,
        r#""accounts.credentials":1"#,
    );
    let document = fixtures::with(&document, b"application.identities[0].number", b"2");
    let document = fixtures::with(&document, b"application.forges[0].identity", b"2");
    assert_eq!(
        &*started(&document),
        b"application.identities[0].number: must be from 1 to accounts.credentials, in engine.limits and in the worker's limits\n"
    );
}
#[test]
fn the_engines_repositories_below_the_checkouts_refuse_the_start() {
    assert_eq!(
        &*started(&fixtures::with(
            &fixtures::document(),
            b"application.limits.overrides",
            br#"{"checkout.name_bytes":600,"checkout.repositories":4,"git_component.stdout_bytes":2048,"forge.name_bytes":256,"forge.resources_per_task":2}"#,
        )),
        b"application.limits.overrides.forge.resources_per_task: must agree with checkout.repositories and checkout.name_bytes\n"
    );
}
#[test]
fn the_hubs_accounts_past_the_agents_name_both_limits() {
    let document =
        overridden(&fixtures::document(), b"workers[0].limits.overrides", WORKER_OVERRIDES, r#""hub.accounts":4096"#);
    assert_eq!(
        &*started(&document),
        b"workers[0].limits.overrides.hub.accounts: must agree with the agent's accounts\n"
    );
}
#[test]
fn the_checkouts_repositories_past_the_agents_directories_name_both_limits() {
    let mut agent = fixtures::startup_agent();
    agent.host_limits.directories = 1;
    let worker = fixtures::worker_settings();
    let own = crate::limits::profile(crate::limits::Profile::Small);
    let mut found = Problems::empty();
    checks::worker(&crate::worker_settings::root_limits(&worker, &agent, &own), &mut found);
    assert_eq!(
        &*lines(&found),
        b"application.limits.overrides.checkout.repositories: must agree with the agent's directories\n"
    );
}
#[test]
fn a_resume_limit_past_a_rows_bytes_refuses_the_start() {
    assert_eq!(
        &*started(&fixtures::with(
            &engine_with(r#""core.resume_bytes":65541,"hub.transcript_bytes":262161"#),
            b"workers[0].limits.overrides",
            format!("{{{WORKER_OVERRIDES},\"hub.transcript_bytes\":262161}}").as_bytes(),
        )),
        b"application.limits.overrides.root.journal.transcript_bytes: 65540 bytes, short of core.resume_bytes, 65541 bytes\n"
    );
}
#[test]
fn judges_tables_smaller_than_the_authoritys_refuse_the_start() {
    let mut limits = root_limits();
    limits.forge.judge_projects = limits.authority.projects.checked_sub(1).unwrap();
    assert_eq!(
        &*root(&limits),
        b"application.limits.overrides.forge.judge_projects: must agree with authority.projects in engine.limits\n"
    );
    let mut limits = root_limits();
    limits.forge.judge_criteria = limits.authority.requirements.checked_sub(1).unwrap();
    assert_eq!(
        &*root(&limits),
        b"application.limits.overrides.forge.judge_criteria: must agree with authority.requirements in engine.limits\n"
    );
}
#[test]
fn an_owners_project_without_an_escalation_role_refuses_the_start() {
    let mut engine = fixtures::guard_engine();
    engine.jig.projects[0].policy.escalation_role = None;
    let seeds = fixtures::seeds();
    let core = jig_protocol_config::core(&engine.jig, seeds.deployment, seeds.seed);
    let limits = crate::limits::engine(&engine.jig.limits, &engine.limits).unwrap();
    let mut found = Problems::empty();
    checks::root(core, &engine, &seeds, &[], &limits, &mut found);
    assert!(String::from_utf8_lossy(&lines(&found)).lines().any(|line| line
        == "engine.projects[0].policy.escalation_role: an owner's project must name an escalation role from 0 to 3"));
}
#[test]
fn a_charters_run_past_its_bytes_refuses_the_start() {
    let mut limits = root_limits();
    limits.journal.run_bytes = 16;
    let found = root(&limits);
    assert!(found.starts_with(b"charter 1: "), "{}", String::from_utf8_lossy(&found));
    assert!(found.ends_with(b" bytes, past root.journal.run_bytes of 16\n"), "{}", String::from_utf8_lossy(&found));
}
#[test]
fn permission_mappings_past_a_rows_bytes_refuse_the_start() {
    let mut limits = root_limits();
    limits.journal.transcript_bytes = 1;
    limits.forge.judge_projects = limits.authority.projects;
    let found = root(&limits);
    assert!(
        String::from_utf8_lossy(&found).lines().any(|line| line.starts_with("engine.projects[0].roles: ")
            && line.ends_with(" bytes, past root.journal.transcript_bytes of 1")),
        "{}",
        String::from_utf8_lossy(&found)
    );
}
#[test]
fn landing_rules_past_the_authoritys_requirements_refuse_the_start() {
    let rule = r#"{"connector":1,"kind":4,"pattern":{"segments":["forge","127.0.0.1:9","acme","app"],"last":"open","value":"main"},"enforce":true,"ci":true,"up_to_date":true,"gates":[{"number":3,"blocking":true,"freshness":"exact"}],"approvals":[]}"#;
    let document = engine_with(r#""authority.requirements":2"#);
    let deployment = fixtures::with(&document, b"application.landing.deployment", format!("[{rule}]").as_bytes());
    let deployment = fixtures::with(&deployment, b"application.landing.projects", b"[]");
    assert_eq!(
        &*started(&deployment),
        b"application.landing.deployment: must agree with root.journal.transcript_bytes, authority.requirements, forge.judge_criteria and forge.change_policy.gates\n"
    );
    let project = fixtures::with(&document, b"application.landing.projects[0].rules", format!("[{rule}]").as_bytes());
    assert_eq!(
        &*started(&project),
        b"application.landing.projects[0].rules: must agree with root.journal.transcript_bytes, authority.requirements, forge.judge_criteria and forge.change_policy.gates\n"
    );
}
#[test]
fn forges_past_the_clients_op_bytes_refuse_the_start() {
    let mut limits = root_limits();
    limits.forge.client.op_bytes = 1;
    assert_eq!(&*root(&limits), b"application.forges: must agree with forge.repositories and forge.client.op_bytes\n");
}
#[test]
fn a_deployment_provider_0_refuses_the_start() {
    let document = fixtures::with(&fixtures::document(), b"engine.providers[1].number", b"0");
    assert_eq!(
        &*started(&document),
        b"engine.providers[1].number: the deployment's provider must not be 0, the forge's sign-in provider\n"
    );
}

#[test]
fn configured_worker_count_past_membership_refuses_startup() {
    let mut limits = root_limits();
    limits.fleet.configured_workers = 1;
    assert_eq!(&*root(&limits), b"engine.hosts.workers: past its bound\n");
}
#[test]
fn configured_worker_name_past_membership_names_its_startup_path() {
    let mut limits = root_limits();
    limits.fleet.worker_name_bytes = 1;
    assert_eq!(&*root(&limits), b"engine.hosts.workers[0].name: past its bound\n");
}

#[test]
fn a_joined_policy_past_its_bytes_returns_problems_before_construction() {
    // Project 7 is document entry 0. The parsed policy (272 bytes) still fits
    // 512; joining all three enforced requirements is the admission failure.
    let document =
        fixtures::with(&fixtures::document(), b"engine.limits.overrides", format!("{{{ENGINE_OVERRIDES}}}").as_bytes());
    let document = fixtures::with(&document, b"engine.projects[0].number", b"7");
    let document = fixtures::with(&document, b"application.landing.projects[0].project", b"7");
    let document = fixtures::with(
        &document,
        b"application.landing.projects[0].rules[0].gates",
        br#"[{"number":3,"blocking":true,"freshness":"exact"}]"#,
    );
    let document = fixtures::with(
        &document,
        b"application.landing.projects[0].rules[0].approvals",
        br#"[{"role":0,"people":1,"freshness":"clean"}]"#,
    );
    // The old small inline form is jig's input only; this guard takes typed values.
    let mut engine = fixtures::guard_engine();
    engine.jig = fixtures::parse_engine(&fixtures::with(fixtures::ENGINE, b"projects", fixtures::INLINE_PROJECTS));
    engine.jig.projects[0].number = 7;
    engine.jig.owners[0].project = 7;
    let parsed = jig_protocol_config::document(
        &document,
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 },
    )
    .unwrap();
    let at = jig_protocol_config::read::Object::new(&parsed, 0).unwrap().member(&parsed, b"application").unwrap();
    let mut found = Problems::empty();
    let application = crate::startup::application::parse(
        &parsed,
        at,
        &engine.jig,
        &jig_protocol_config::Directories::working(),
        &mut found,
    )
    .unwrap();
    engine.landing = application.landing;
    let seeds = fixtures::seeds();
    let core = jig_protocol_config::core(&engine.jig, seeds.deployment, seeds.seed);
    let mut limits = crate::limits::engine(&engine.jig.limits, &engine.limits).unwrap();
    limits.policy_bytes = 512;
    checks::root(core, &engine, &seeds, &application.landing_projects, &limits, &mut found);
    assert_eq!(lines(&found).as_ref(), b"engine.projects[0].policy.bytes: 761 bytes, past core.policy_bytes of 512\n");
}

#[test]
fn unaccountable_policy_bytes_use_the_existing_bound_formatter() {
    let mut found = Problems::empty();
    found.push(
        jig_protocol_config::Path::root()
            .member(b"engine")
            .member(b"projects")
            .child(jig_protocol_config::Segment::Index(0))
            .member(b"policy")
            .member(b"bytes"),
        crate::startup::What::Jig(jig_protocol_config::What::Bound),
    );
    assert_eq!(lines(&found).as_ref(), b"engine.projects[0].policy.bytes: past its bound\n");
}
