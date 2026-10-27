//! Settings cross the root's journal and real store, including cold restoration.
use jig_domain_authority as authority;
use jig_domain_core::{self as core, settings as s};
use jig_domain_people as people;
use skein_lib::{Duration, ReplyTo, Token};
use temper_engine_domain::{self as engine, Delivery, Event, Key, PartyInput, Record, Write};
use temper_engine_domain_world::{direct::Driver, store::Store, walking};

const SEED: u64 = 9761;
// Restoration keeps two archived revisions; pruning keeps one (jig's domain/engine.md, 17.7).
const HISTORY: u32 = 2;
const PRUNED_HISTORY: u32 = 1;
const OWNER_PERSON: u64 = 1;
const OTHER: u64 = 8;
const POLICY_CAP: u64 = 16_384;
const PRIOR_POLICY_CAP: u64 = POLICY_CAP + 1;
const POLICY_NUMBER: u32 = 1;
const MADE_PROJECT: u32 = 2;
const POLICY_ROLES: u32 = 2;
// Production Small's journal/page byte authority; two >16 KiB policy rows
// still fit a loaded page. Counts stay the direct world's, except the stated
// policy shape capacities and room for the second project below.
const POLICY_PAGE_BYTES: u32 = 65_536;

fn policy_limits(policy_bytes: u64) -> engine::Limits {
    let mut limits = walking::limits();
    limits.policy_bytes = policy_bytes;
    limits.settings.history = HISTORY;
    limits.journal.transcript_bytes = POLICY_PAGE_BYTES;
    limits.loads.bytes = POLICY_PAGE_BYTES;
    limits.loads.reply_bytes = POLICY_PAGE_BYTES;
    limits.loads.transcript_bytes = POLICY_PAGE_BYTES;
    limits.loads.read_bytes = u64::from(POLICY_PAGE_BYTES);
    // Existing widest policy shapes allow a byte-heavy policy without
    // crossing an independent count, pattern or per-segment limit.
    let widest = temper_engine_domain_world::store::widest();
    limits.authority.grants = widest.authority.grants;
    limits.authority.roles = POLICY_ROLES;
    limits.authority.segments = widest.authority.segments;
    limits.authority.segment_bytes = widest.authority.segment_bytes;
    // Served policy documents must retain those same grant/role shapes.
    limits.documents.grants = limits.documents.grants.max(limits.authority.grants);
    limits.documents.roles = limits.documents.roles.max(limits.authority.roles);
    // Core admission requires the three scalar note scopes plus each grant.
    limits.notes.scopes = 3 + limits.authority.grants;
    limits.authority.projects = MADE_PROJECT;
    limits.people.projects = MADE_PROJECT + 1; // Includes the deployment roster.
    limits.journal.writes = walking::writes(&limits);
    limits
}

fn all_resources() -> authority::Grant {
    authority::Grant {
        connector: 0,
        kind: 1,
        pattern: authority::Pattern { segments: Box::new([]), last: authority::Last::Open(Box::new([])) },
    }
}

fn policy_config() -> engine::Config {
    let mut config = walking::config(SEED);
    // A broad existing connector grant lets the test vary only owned bytes
    // in narrower project grants, rather than fail the rules' authority check.
    config.core.defaults.rules.ceiling.grants = Box::new([all_resources()]);
    config
}

fn sized_policy(bytes: u64) -> core::ConfiguredPolicy {
    let limits = policy_limits(PRIOR_POLICY_CAP);
    let mut configured = policy_config().core.seed_projects[0].policy.clone();
    configured.permissions = Box::new([]);
    configured.policy.ceiling.grants = Box::new([all_resources()]);
    let mut role = configured.policy.roles[0].clone();
    role.number = 1;
    configured.policy.roles = Box::new([configured.policy.roles[0].clone(), role]);
    let empty_grant = authority::Grant {
        connector: 0,
        kind: 1,
        pattern: authority::Pattern {
            segments: vec![
                Box::<[u8]>::default();
                usize::try_from(limits.authority.segments).expect("fixture segment count fits usize")
            ]
            .into_boxed_slice(),
            last: authority::Last::Exact(Box::new([])),
        },
    };
    let grants = vec![empty_grant; usize::try_from(limits.authority.grants).expect("fixture grant count fits usize")]
        .into_boxed_slice();
    configured.policy.projections.clone_from(&grants);
    for role in &mut configured.policy.roles {
        role.authority.grants.clone_from(&grants);
    }
    let snapshot = core::policy_snapshot(&configured.policy, &configured.permissions, &configured.templates)
        .expect("representable policy");
    let base = people::policy_bytes(&snapshot).expect("policy carrier bytes");
    let mut remaining = usize::try_from(bytes.checked_sub(base).expect("carrier is below the byte fixture"))
        .expect("fixture bytes fit usize");
    // Fill existing bounded segments, then their terminal words. The last
    // segment can contribute one byte, so cap and cap+1 are both representable.
    for grant in configured
        .policy
        .projections
        .iter_mut()
        .chain(configured.policy.roles.iter_mut().flat_map(|role| role.authority.grants.iter_mut()))
    {
        let authority::Last::Exact(last) = &mut grant.pattern.last else { unreachable!("exact fixture") };
        for word in grant.pattern.segments.iter_mut().chain(std::iter::once(last)) {
            let count = remaining
                .min(usize::try_from(limits.authority.segment_bytes).expect("fixture segment bytes fit usize"));
            *word = vec![b'x'; count].into_boxed_slice();
            remaining -= count;
        }
    }
    assert_eq!(remaining, 0, "existing shape capacities reach the named policy byte bound");
    let snapshot = core::policy_snapshot(&configured.policy, &configured.permissions, &configured.templates)
        .expect("bounded policy snapshot");
    assert_eq!(people::policy_bytes(&snapshot), Some(bytes));
    configured
}

fn saved_policy(bytes: u64, project: bool) -> Driver {
    let limits = policy_limits(bytes);
    let mut driver = Driver::cold(Store::with_limits(&limits), policy_config(), &limits);
    driver.settle();
    driver.sign_in();
    driver.settle();
    change(
        &mut driver,
        1,
        20,
        s::Item {
            setting: s::Setting::Policy(POLICY_NUMBER),
            edit: s::Edit::Set(s::Value::Policy(Box::new(sized_policy(bytes)))),
        },
    );
    assert!(
        driver.delivered.iter().any(|delivery| matches!(
            delivery,
            Delivery::WebReply { reply: people::Reply::Outcome(people::Outcome::SettingsChanged { revision: 2 }), .. }
        )),
        "the byte-heavy setting was committed through ChangeSettings"
    );
    if project {
        driver.send(Event::Party(PartyInput::Ask {
            reply_to: ReplyTo::new(Token::new(21)),
            sign_in: driver.session(),
            key: [21; 16],
            models: Box::new([]),
            ask: people::Ask::MakeProject { name: "Saved policy".into(), policy: POLICY_NUMBER },
        }));
        driver.settle();
        assert!(
            driver.delivered.iter().any(|delivery| matches!(
                delivery,
                Delivery::WebReply {
                    reply: people::Reply::Outcome(people::Outcome::ProjectMade { project: MADE_PROJECT }),
                    ..
                }
            )),
            "MakeProject committed its copy of the changed policy"
        );
        // Remove the source setting through another real decision: only the
        // project's retained copy must trigger the named narrowed restore.
        change(&mut driver, 2, 22, s::Item { setting: s::Setting::Policy(POLICY_NUMBER), edit: s::Edit::Removed });
        assert_eq!(head(&driver).number, 3);
    }
    driver
}

fn replay_policy(driver: Driver, limits: &engine::Limits) -> Driver {
    let mut store = Store::with_limits(limits);
    for (index, writes) in driver.transactions.into_iter().enumerate() {
        let number = u64::try_from(index).expect("transaction index fits u64") + 1;
        // Complete transactions include deployment counters, settings history,
        // keyed answers and project rows, just as the real root committed them.
        store.submit(number, writes.into_boxed_slice(), 0);
        assert_eq!(store.tick(false), Ok(Some(number)), "the narrower real codec admits the committed transaction");
    }
    store.crash(SEED);
    Driver::cold(store, policy_config(), limits)
}

#[test]
fn a_stored_settings_policy_one_byte_over_the_new_bound_refuses_by_setting_and_source() {
    let driver = saved_policy(PRIOR_POLICY_CAP, false);
    let mut restarted = replay_policy(driver, &policy_limits(POLICY_CAP));
    restarted.pump();
    assert!(restarted.stopped);
    assert!(!restarted.root.ready());
    assert_eq!(
        restarted.root.restore_failure(),
        Some(core::RestartRequest::Settings {
            failure: s::Failure { setting: s::Setting::Policy(POLICY_NUMBER), fault: s::Fault::Shape },
            changed: true,
        })
    );
    assert!(restarted.transactions.is_empty(), "a refused start commits no release revision");
}

#[test]
fn a_stored_project_policy_one_byte_over_the_new_bound_refuses_by_project() {
    let driver = saved_policy(PRIOR_POLICY_CAP, true);
    let mut restarted = replay_policy(driver, &policy_limits(POLICY_CAP));
    restarted.pump();
    assert!(restarted.stopped);
    assert!(!restarted.root.ready());
    assert_eq!(restarted.root.restore_failure(), Some(core::RestartRequest::ProjectPolicy { project: MADE_PROJECT }));
    assert!(restarted.transactions.is_empty(), "a refused start commits no release revision");
}

#[test]
fn a_stored_settings_policy_at_the_byte_bound_is_admitted() {
    let driver = saved_policy(POLICY_CAP, false);
    let mut restarted = replay_policy(driver, &policy_limits(POLICY_CAP));
    restarted.settle();
    assert!(restarted.root.ready());
    assert!(!restarted.stopped);
    assert_eq!(restarted.root.restore_failure(), None);
}

#[test]
fn a_stored_project_policy_at_the_byte_bound_is_admitted() {
    let driver = saved_policy(POLICY_CAP, true);
    let mut restarted = replay_policy(driver, &policy_limits(POLICY_CAP));
    restarted.settle();
    assert!(restarted.root.ready());
    assert!(!restarted.stopped);
    assert_eq!(restarted.root.restore_failure(), None);
}

fn stock_config() -> engine::Config {
    let mut config = walking::config(SEED);
    let llm = jig_protocol_config::Llm {
        endpoints: Box::new([]),
        models: walking::models(),
        descriptions: Box::new([None]),
        providers: Box::new([]),
    };
    config.core.defaults = temper_service::stock::defaults(&llm);
    config.core.connectors = Box::new([1]);
    config.core.seed_projects = Box::new([]);
    config.core.owners[0].project = people::DEPLOYMENT;
    config
}

fn stock_limits() -> engine::Limits {
    let mut limits = walking::limits();
    // Stock policy 1 holds ten grants; room for the design's five executor kinds.
    limits.authority.grants = 16;
    limits.authority.executors = 8;
    limits.documents.grants = 16;
    limits.documents.executors = 8;
    limits.notes.scopes = 32;
    limits.tasks.charters = 4;
    limits.journal.writes = walking::writes(&limits);
    limits
}

fn deployment_without_projects() -> Driver {
    let limits = stock_limits();
    let mut driver = Driver::cold(Store::with_limits(&limits), stock_config(), &limits);
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver
}

#[test]
fn an_owner_of_a_deployment_with_no_project_makes_one_from_policy_1() {
    let mut driver = deployment_without_projects();
    let llm = jig_protocol_config::Llm {
        endpoints: Box::new([]),
        models: walking::models(),
        descriptions: Box::new([None]),
        providers: Box::new([]),
    };
    let defaults = temper_service::stock::defaults(&llm);
    let mut rules = defaults.rules;
    let mut policy = defaults.policies.get(&POLICY_NUMBER).expect("stock policy 1").clone();
    let widened = authority::Tools(1023);
    assert_ne!(policy.policy.ceiling.tools, widened);
    rules.ceiling.tools = widened;
    policy.policy.ceiling.tools = widened;
    policy.policy.roles[0].authority.tools = widened;
    driver.send(Event::Party(PartyInput::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(30)),
        sign_in: driver.session(),
        key: [30; 16],
        digest: [30; 32],
        change: Box::new(s::Change {
            against: 1,
            items: Box::new([
                s::Item { setting: s::Setting::Rules, edit: s::Edit::Set(s::Value::Rules(Box::new(rules))) },
                s::Item {
                    setting: s::Setting::Policy(POLICY_NUMBER),
                    edit: s::Edit::Set(s::Value::Policy(Box::new(policy))),
                },
            ]),
            running: s::Running::Stays,
        }),
    }));
    driver.settle();
    assert_eq!(head(&driver).number, 2, "the deployment owner changed both settings");
    driver.send(Event::Party(PartyInput::Ask {
        reply_to: ReplyTo::new(Token::new(31)),
        sign_in: driver.session(),
        key: [31; 16],
        models: Box::new([]),
        ask: people::Ask::MakeProject { name: "First project".into(), policy: POLICY_NUMBER },
    }));
    driver.settle();
    assert!(driver.delivered.iter().any(|delivery| matches!(
        delivery,
        Delivery::WebReply { reply: people::Reply::Outcome(people::Outcome::ProjectMade { project: 1 }), .. }
    )));
    let Some(Record::Core(core::Record::People(people::Stored::Policy { value, .. }))) =
        driver.store.rows.get(&Key::Core(core::Key::People(people::Key::Policy(1))))
    else {
        panic!("the first project's policy is committed")
    };
    assert_eq!(value.ceiling.tools, widened.0);
    let ask = walking::chat(1, b"Use the widened tools");
    driver.send(Event::Party(PartyInput::Ask {
        reply_to: ReplyTo::new(Token::new(32)),
        sign_in: driver.session(),
        key: [32; 16],
        models: Box::new([]),
        ask,
    }));
    driver.pump(); // An admitted agent task awaits a worker's capacity.
    let task = driver.task(1).expect("the first project admits its owner's task");
    assert_eq!(task.project, 1);
    assert_eq!(task.authority.tools.0, widened.0);
}

#[path = "settings/b2.rs"]
mod b2;

#[test]
fn a_stock_chat_funds_two_dependent_changes_with_two_repairs_each() {
    b2::story();
}

fn started(history: u32) -> Driver {
    let mut limits = walking::limits();
    limits.settings.history = history;
    let mut driver = Driver::cold(Store::new(), walking::config(SEED), &limits);
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver
}
fn rule_value(minimum: u64) -> s::Value {
    let mut rules = walking::config(SEED).core.defaults.rules;
    rules.minimum_run_spend = minimum;
    s::Value::Rules(Box::new(rules))
}
fn rules(minimum: u64) -> s::Item {
    s::Item { setting: s::Setting::Rules, edit: s::Edit::Set(rule_value(minimum)) }
}
fn change(driver: &mut Driver, against: u32, key: u8, item: s::Item) {
    driver.send(Event::Party(PartyInput::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(u64::from(key))),
        sign_in: driver.session(),
        key: [key; 16],
        digest: [key; 32],
        change: Box::new(s::Change { against, items: Box::new([item]), running: s::Running::Stays }),
    }));
    driver.settle();
}
fn head(driver: &Driver) -> &s::Head {
    driver
        .store
        .rows
        .values()
        .find_map(|row| {
            if let Record::Core(core::Record::Core(core::CoreRecord::Settings(s::Record::Revision {
                history: false,
                revision,
            }))) = row
            {
                Some(&revision.head)
            } else {
                None
            }
        })
        .expect("newest held revision")
}
fn read_rules(driver: &mut Driver, revision: u32) -> core::reads::SettingDocument {
    driver.send(Event::Party(PartyInput::Read {
        reply_to: ReplyTo::new(Token::new(990)),
        sign_in: driver.session(),
        read: core::reads::Read::Setting { setting: s::Setting::Rules, revision, proposal: None },
    }));
    driver.settle();
    driver
        .delivered
        .iter()
        .rev()
        .find_map(|delivery| {
            if let Delivery::Read { answer: core::reads::Answer::Setting(document), .. } = delivery {
                Some(*document.clone())
            } else {
                None
            }
        })
        .expect("current setting read")
}
#[test]
fn an_owner_change_commits_and_the_next_read_serves_it() {
    let mut driver = started(HISTORY);
    change(&mut driver, 1, 1, rules(2));
    assert!(driver.delivered.iter().any(|delivery| matches!(
        delivery,
        Delivery::WebReply { reply: people::Reply::Outcome(people::Outcome::SettingsChanged { revision: 2 }), .. }
    )));
    assert_eq!(head(&driver).by, s::Decider::Party(OWNER_PERSON)); // First issued person, independent of their identity subject.
    assert_eq!(read_rules(&mut driver, 2).value, rule_value(2));
}
#[test]
fn a_nonowner_cannot_change_the_deployments_settings() {
    let mut driver = started(HISTORY);
    driver.delivered.clear();
    driver.send(Event::Party(PartyInput::SignedIn {
        reply_to: ReplyTo::new(Token::new(991)),
        held: None,
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: OTHER.to_be_bytes().into() },
            login: Box::new([]),
            name: Box::new([]),
        },
    }));
    driver.settle();
    change(&mut driver, 1, 2, rules(2));
    assert_eq!(head(&driver).number, 1);
    assert!(driver.delivered.iter().any(|delivery| matches!(
        delivery,
        Delivery::WebReply {
            reply: people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { .. }))
                | people::Reply::Refused(people::Refusal::Role { .. }),
            ..
        }
    )));
}
#[test]
fn a_stale_owner_change_names_the_newest_revision() {
    let mut driver = started(HISTORY);
    change(&mut driver, 1, 3, rules(2));
    change(&mut driver, 1, 4, rules(3));
    assert!(driver.delivered.iter().any(|delivery| matches!(
        delivery,
        Delivery::WebReply {
            reply: people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Stale { newest: 2 })),
            ..
        }
    )));
    assert_eq!(head(&driver).number, 2);
}
#[test]
fn changed_settings_restore_before_accounts_and_survive_a_restart() {
    let mut driver = started(HISTORY);
    change(&mut driver, 1, 5, rules(2));
    let mut restarted = Driver::cold(driver.store, walking::config(SEED), &walking::limits());
    restarted.settle();
    restarted.sign_in();
    restarted.settle();
    let mut ranges: Vec<_> = restarted.loads.iter().map(|(_, range)| range.clone()).collect();
    ranges.dedup(); // Settings can span several pages before the next range begins.
    assert_eq!(
        ranges.into_iter().take(3).collect::<Vec<_>>(),
        vec![
            engine::Range::Core(engine::CoreRange::Deployment),
            engine::Range::Core(engine::CoreRange::Settings),
            engine::Range::Core(engine::CoreRange::Accounts)
        ]
    );
    assert_eq!(read_rules(&mut restarted, 2).value, rule_value(2));
}
#[test]
fn changed_release_defaults_record_a_revision_once() {
    let driver = started(HISTORY);
    let mut config = walking::config(SEED);
    config.core.defaults.rules.minimum_run_spend = 2;
    let mut released = Driver::cold(driver.store, config, &walking::limits());
    released.settle();
    assert_eq!((head(&released).number, head(&released).by), (2, s::Decider::Release));
    let mut config = walking::config(SEED);
    config.core.defaults.rules.minimum_run_spend = 2;
    let mut unchanged = Driver::cold(released.store, config, &walking::limits());
    unchanged.settle();
    assert_eq!((head(&unchanged).number, head(&unchanged).by), (2, s::Decider::Release));
    assert!(
        !unchanged.transactions.iter().flatten().any(|write| matches!(
            write,
            Write::Save(Record::Core(core::Record::Core(core::CoreRecord::Settings(s::Record::Revision { .. }))))
        )),
        "an unchanged start records no revision"
    );
}
#[test]
fn a_revision_copy_precedes_its_erase_in_the_same_commit() {
    let mut driver = started(PRUNED_HISTORY);
    change(&mut driver, 1, 6, rules(2));
    change(&mut driver, 2, 7, rules(3));
    let commit = driver
        .transactions
        .iter()
        .find(|writes| writes.contains(&Write::CopySettingsRevision(2)))
        .expect("second revision archive");
    let copy =
        commit.iter().position(|write| *write == Write::CopySettingsRevision(2)).expect("copy in the same commit");
    let erase = commit
        .iter()
        .position(|write| *write == Write::Erase(Key::Core(core::Key::Core(core::CoreKey::Settings(s::Key::Held(2))))))
        .expect("held erase in the same commit");
    assert!(copy < erase);
    assert_eq!(
        Write::CopySettingsRevision(2).key(),
        Key::Core(core::Key::Core(core::CoreKey::Settings(s::Key::History(2))))
    );
    assert!(driver.store.rows.contains_key(&Key::Core(core::Key::Core(core::CoreKey::Settings(s::Key::History(2))))));
    assert!(!driver.store.rows.contains_key(&Key::Core(core::Key::Core(core::CoreKey::Settings(s::Key::History(1))))));
}
#[test]
fn a_stored_change_that_no_longer_checks_refuses_the_start() {
    let mut driver = started(HISTORY);
    let mut template = walking::config(SEED).core.defaults.templates[0].clone();
    template.grace = Some(Duration::from_secs(4));
    change(
        &mut driver,
        1,
        8,
        s::Item { setting: s::Setting::Template(1), edit: s::Edit::Set(s::Value::Template(Box::new(template))) },
    );
    let mut limits = walking::limits();
    limits.tasks.grace = Duration::from_secs(3);
    let mut restarted = Driver::cold(driver.store, walking::config(SEED), &limits);
    restarted.pump();
    assert!(restarted.stopped);
    assert_eq!(
        restarted.root.restore_failure(),
        Some(core::RestartRequest::Settings {
            failure: s::Failure { setting: s::Setting::Template(1), fault: s::Fault::Grace },
            changed: true
        })
    );
}

#[test]
fn a_stock_reviewer_reads_and_gives_a_verdict_but_cannot_delegate() {
    b2::reviewer();
}

// The host boundary is real here: JSON goes to smith's decoder and the core's
// document goes back through smith's ctl-shared renderer, over the root's fleet.
fn settings_agent_config(allowed: bool) -> engine::Config {
    let mut config = walking::config(SEED);
    if allowed {
        config.core.defaults.rules.ceiling.tools.0 |= core::ToolFamilies::JIG.settings;
        for project in &mut config.core.seed_projects {
            project.policy.policy.ceiling.tools.0 |= core::ToolFamilies::JIG.settings;
            for role in &mut project.policy.policy.roles {
                role.authority.tools.0 |= core::ToolFamilies::JIG.settings;
            }
        }
    }
    // The deployment owner creates the project through the root, acquiring
    // its project role within the fixture's existing owner and holding bounds.
    config.core.owners[0].project = people::DEPLOYMENT;
    let mut policy = config.core.seed_projects[0].policy.clone();
    policy.permissions = Box::new([]);
    config.core.seed_projects = Box::new([]);
    config.core.defaults.policies.insert(1, policy).expect("one default policy");
    config
}

fn settings_agent(allowed: bool) -> (Driver, engine::Assignment) {
    let mut driver = Driver::configured(
        Store::new(),
        [settings_agent_config(allowed), settings_agent_config(allowed)],
        &walking::limits(),
    );
    driver.hello(Token::new(7));
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver.send(Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(991)),
        sign_in: driver.session(),
        key: [91; 16],
        ask: people::Ask::MakeProject { name: "Settings project".into(), policy: 1 },
    }));
    driver.settle();
    driver.period();
    driver.settle();
    driver.send(Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(992)),
        sign_in: driver.session(),
        key: [92; 16],
        ask: walking::configured_chat(
            &jig_core_world::fixtures::Creating::new(&walking::config(SEED).core, walking::limits().tasks.tree_tasks),
            &driver.store.rows,
            driver.env.limits.tasks.tree_tasks,
            1,
            b"Read settings",
        ),
    }));
    driver.settle();
    let run = driver.last_assignment();
    (driver, run)
}

fn host_tool(
    driver: &mut Driver,
    run: &engine::Assignment,
    completion: u32,
    tool: &[u8],
    input: &[u8],
) -> (Box<[u8]>, bool) {
    use jig_domain_host as host;
    use smith_domain_run as smith;
    use temper_engine_domain_world::protocol;
    let mut name = skein_lib::Writer::new(16);
    name.put(&run.attempt.to_be_bytes()).expect("activation");
    name.put(&completion.to_be_bytes()).expect("completion");
    name.put(&0_u32.to_be_bytes()).expect("position");
    let name = name.finish();
    driver.protocol.defer_render = true;
    driver.send(protocol::up(
        Token::new(7),
        host::LinkUp::Call {
            run: Token::new(run.task),
            attempt: Token::new(run.attempt),
            call: name.clone(),
            tool: tool.into(),
            writes: tool != b"settings",
            input: input.into(),
            deadline: Duration::from_secs(10),
        },
        engine::Parts::none(),
    ));
    driver.pump();
    let request = driver.host_requests.pop().expect("host decoder called");
    let engine::HostRequest::Decode { to, call, .. } = *request else { panic!("decode") };
    assert_eq!(call.name, name);
    let input = smith::HostInput::attested(call.input).expect("host input");
    let decoded = temper_engine_smith::call_configured(
        smith::CallName { activation: run.attempt, completion, position: 0 },
        &call.tool,
        &input,
        driver.root.models(),
    )
    .expect("valid host call");
    let temper_engine_smith::Decoded::Call(body) = decoded else { panic!("valid settings tool enters the root") };
    driver.send(Event::Worker(engine::WorkerInput::DecodedHostCall { to, body }));
    driver.pump();
    let mut settings_delivery = None;
    for (index, delivery) in driver.delivered.iter().enumerate() {
        if let Delivery::Host(delivery) = delivery
            && matches!(delivery.as_ref(), engine::HostDelivery::Settings { .. })
        {
            settings_delivery = Some(index);
            break;
        }
    }
    let (to, answer) = if let Some(index) = settings_delivery {
        let Delivery::Host(delivery) = driver.delivered.remove(index) else { panic!("settings delivery") };
        let engine::HostDelivery::Settings { to, answer, from, requested } = *delivery else { panic!("settings read") };
        (to, temper_engine_smith::settings_answer(&answer, from, requested, driver.root.models()))
    } else {
        let (to, rendered_name) = driver.protocol.pending_renders.pop().expect("host answer to render");
        assert_eq!(rendered_name, name);
        let (_, answer) = driver.protocol.answers.last().expect("typed answer");
        (to, temper_engine_smith::answer(answer))
    };
    let (body, error) = answer.into_parts();
    driver.send(Event::Worker(engine::WorkerInput::RenderedHostCall {
        to,
        answer: core::SettledAnswer::Host { error, body },
    }));
    driver.pump();
    for delivery in driver.delivered.iter().rev() {
        if let Delivery::Link {
            down: host::LinkDown::Relayed { call, answer: host::Relayed::Answer { text, error }, .. },
            ..
        } = delivery
            && *call == name
        {
            return (text.clone(), *error);
        }
    }
    panic!("settings tool answer reached the run");
}

fn owner_reviews_run_proposal(driver: &mut Driver, task: u64, proposal: u64) {
    driver.send(Event::Party(PartyInput::Read {
        reply_to: ReplyTo::new(Token::new(995)),
        sign_in: driver.session(),
        read: core::reads::Read::Proposal { proposer: people::Proposer::Task(task), proposal },
    }));
    driver.settle();
    let mut reviewed = false;
    for delivery in &driver.delivered {
        if let Delivery::Read { answer: core::reads::Answer::Document(document), .. } = delivery
            && let core::documents::Document::Proposal(document) = document.as_ref()
            && let core::documents::ProposalAction::Settings { against, review: Some(review), .. } = &document.action
        {
            assert_eq!((*against, review.newest), (1, 1));
            let item = review.items.get(0).expect("reviewed charter");
            let Some(s::Value::Charter(charter)) = &item.would_be else { panic!("proposed charter") };
            assert_eq!(charter.policy.instructions.as_ref(), b"Read settings and report.");
            reviewed = true;
        }
    }
    assert!(reviewed, "the owner reads the real settings review");
    driver.send(Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(996)),
        sign_in: driver.session(),
        key: [96; 16],
        ask: people::Ask::DecideProposal {
            project: people::DEPLOYMENT,
            proposer: people::Proposer::Task(task),
            proposal,
            decision: people::ProposalDecision::Accept,
            running: Some(people::Running::Stays),
        },
    }));
    driver.settle();
    let mut accepted = false;
    for delivery in &driver.delivered {
        if let Delivery::WebReply {
            reply:
                people::Reply::Outcome(people::Outcome::ProposalDecided {
                    choice: people::ProposalChoice::Accepted, ..
                }),
            ..
        } = delivery
        {
            accepted = true;
        }
    }
    assert!(accepted, "the owner's acceptance passes through the root");
}

#[test]
fn a_run_reads_the_settings_and_proposes_a_change_an_owner_accepts() {
    use jig_domain_tasks as tasks;
    let (mut driver, run) = settings_agent(true);
    let original = driver.store.rows.clone();
    let (inventory, error) = host_tool(&mut driver, &run, 1, b"settings", br"{}");
    assert!(!error);
    assert!(inventory.starts_with(b"newest revision 1\n"));
    let (charter, error) =
        host_tool(&mut driver, &run, 2, b"settings", br#"{"setting":{"kind":"charter","number":1},"revision":1}"#);
    assert!(!error);
    assert!(charter.starts_with(b"charter 1, revision 1\n"));
    assert_eq!(driver.store.rows, original, "settings reads save no call record or other row");
    let charter = std::str::from_utf8(&charter).expect("charter text");
    let (_, value) = charter.split_once('\n').expect("value line");
    let value = value.strip_suffix("end\n").expect("whole charter fits this page").trim();
    assert!(value.contains("Complete the task within its authority."));
    let value = value.replace("Complete the task within its authority.", "Read settings and report.");
    let proposal_json = format!(
        "{{\"against\":1,\"reason\":\"clarify the task\",\"items\":[{{\"setting\":{{\"kind\":\"charter\",\"number\":1}},\"set\":{value}}}]}}"
    );
    let (proposed, error) = host_tool(&mut driver, &run, 3, b"propose_settings", proposal_json.as_bytes());
    assert!(!error, "{}", String::from_utf8_lossy(&proposed));
    let text = std::str::from_utf8(&proposed).expect("proposal text");
    let proposal: u64 = text
        .strip_prefix("proposed as proposal ")
        .expect("proposal number")
        .strip_suffix(" to the deployment's owners")
        .expect("owners")
        .parse()
        .expect("number");
    owner_reviews_run_proposal(&mut driver, run.task, proposal);
    let before = driver.store.rows.clone();
    // A retry under the same read name is fresh even after a write changed settings.
    let (inventory, error) = host_tool(&mut driver, &run, 1, b"settings", br"{}");
    assert!(!error);
    assert!(inventory.starts_with(b"newest revision 2\n"));
    assert_eq!(driver.store.rows, before);
    driver.answer(Token::new(7), run.task, run.attempt, 0, tasks::End::Parked, engine::Parts::none());
    driver.settle();
    let next = driver.last_assignment();
    assert_ne!(next.attempt, run.attempt);
    let smith_domain::Event::Start { messages, .. } = temper_engine_smith::start(
        &next,
        None,
        None,
        ReplyTo::new(Token::new(997)),
        Token::new(run.task),
        smith_domain::Window { turns: 1, bytes: 100_000 },
    ) else {
        panic!("smith start")
    };
    let mut heard = false;
    for message in &messages {
        heard |= message.text.as_ref() == b"accepted: revision 2 made";
    }
    assert!(heard, "the next run hears the accepted revision in its starting inbox");
}

#[test]
fn a_run_without_the_settings_family_is_denied() {
    let (mut driver, run) = settings_agent(false);
    let original = driver.store.rows.clone();
    let (answer, error) = host_tool(&mut driver, &run, 1, b"settings", br"{}");
    assert!(error, "settings family is required: {}", String::from_utf8_lossy(&answer));
    assert_eq!(driver.store.rows, original, "a denied read also saves no call record");
    let (answer, error) = host_tool(
        &mut driver,
        &run,
        2,
        b"propose_settings",
        br#"{"against":1,"items":[{"setting":{"kind":"charter","number":1},"edit":"reset"}],"reason":"restore"}"#,
    );
    assert!(error, "settings proposal is denied: {}", String::from_utf8_lossy(&answer));
    let (_, part) = driver.protocol.answers.last().expect("denial");
    assert!(matches!(part, engine::CallAnswer::ToolDenied { .. }));
}
