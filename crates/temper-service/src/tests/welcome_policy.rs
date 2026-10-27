//! Real init bytes through startup parsing and the root's project/adoption decisions.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "ordinary regression code retains whole snapshots and prints its offline measurement"
)]
use super::{application, config, parse};
use crate::{settings, tests::fixtures};
use ::core::{num::NonZeroU16, str};
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{List, ReplyTo, Secret, Token};
use temper_engine_domain::{self as engine, Delivery, Event, Key, PartyInput, Record};
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, forge_stand::ForgeStand, store::Store};

fn generated() -> (engine::Config, engine::Limits) {
    let port = |value| NonZeroU16::new(value).expect("nonzero synthetic port");
    let answer = temper_init::files(
        &temper_init::Facts {
            binary: b"/bin/temper",
            git: Some(b"/bin/git"),
            claude: None,
            login: Some(b"ada"),
            ports: temper_init::Ports {
                clients: port(8367),
                owned: port(8368),
                forge: port(8369),
                llm: port(8370),
                webhooks: port(8371),
            },
        },
        &temper_init::Directories { config: b"/config", data: b"/data" },
        &[0; 32],
        &[1; 32],
    )
    .expect("actual init producer");
    let mut files = settings::Files { entries: List::with_capacity(8) };
    let mut document = None;
    for file in answer.files {
        if let temper_init::Content::Bytes { bytes, .. } = file.content {
            if file.name == b"temper.json" {
                document = Some(bytes.clone());
            }
            let mut path = match file.directory {
                temper_init::Directory::Config => b"/config/".to_vec(),
                temper_init::Directory::Data => b"/data/".to_vec(),
            };
            path.extend_from_slice(file.name);
            files.entries.push((path.into_boxed_slice(), Secret::new(bytes))).expect("generated files");
        }
    }
    let directories = config::Directories::given(b"/config", b"/data", b"/cache").expect("absolute directories");
    let parsed = parse(&document.expect("generated temper.json"), &directories).expect("real welcome preparation");
    let mut problems = super::Problems::empty();
    let resolved = application::resolve(&parsed.application, &files, &mut problems).expect("generated file values");
    assert!(problems.is_empty());
    let mut settings = settings::Engine {
        jig: parsed.engine,
        clients: parsed.application.clients,
        sign_in: resolved.sign_in,
        webhooks: resolved.webhooks,
        files,
        forges: resolved.forges,
        demo: parsed.application.demo,
        setup: Box::new([]),
        records_only: Box::new([]),
        identities: resolved.identities,
        landing: parsed.application.landing,
        charter: parsed.application.charter,
        limits: parsed.application.limits,
    };
    let limits = super::engine_limits(&mut settings.jig.limits, &settings.limits).expect("unchanged effective limits");
    let seeds = fixtures::seeds();
    let core = config::core(&settings.jig, seeds.deployment, seeds.seed);
    let (forge, identities) = crate::forge::config(&settings, seeds.deployment);
    let forges = application::configured(&settings);
    let demo = settings.demo.as_ref().map(|demo| demo.domain(&settings.forges, settings.limits.setup.deadline));
    (
        engine::Config {
            period_span: crate::stock::PERIOD_SPAN,
            core,
            forge,
            identities,
            forges,
            demo,
            landing: settings.landing,
            forge_charter: settings.charter,
            under_box: false,
        },
        limits,
    )
}

fn policies(driver: &Driver) -> Vec<(u32, people::PolicyValue)> {
    driver
        .store
        .rows
        .values()
        .filter_map(|record| match record {
            Record::Core(core::Record::People(people::Stored::Policy { project, value, .. })) => {
                Some((*project, value.clone()))
            }
            Record::Core(_) | Record::Forge { .. } => None,
        })
        .collect()
}

fn ask(driver: &mut Driver, key: u8, ask: people::Ask) {
    driver.send(Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(u64::from(key))),
        sign_in: driver.session(),
        key: [key; 16],
        ask,
    }));
    driver.settle();
}

fn reply(driver: &Driver, request: u8) -> &people::Reply {
    driver
        .delivered
        .iter()
        .find_map(|delivery| {
            if let Delivery::WebReply { to, reply } = delivery
                && *to == ReplyTo::new(Token::new(u64::from(request)))
            {
                Some(reply)
            } else {
                None
            }
        })
        .expect("one completed people request")
}

#[test]
#[expect(
    clippy::too_many_lines,
    clippy::disallowed_macros,
    clippy::print_stderr,
    reason = "one offline proof checks selected outcomes and reports its exact snapshot measurement"
)]
fn real_init_policy_fits_with_two_long_github_repositories() {
    let (config, limits) = generated();
    // Init inherits today's Small profile; also prove the stricter product question.
    assert_eq!(limits.policy_bytes, 16_384);
    assert_eq!(config.core.seed_projects.len(), 1);
    assert_eq!(config.landing.deployment.len(), 1);
    assert!(config.landing.projects.is_empty());
    let seed = &config.core.seed_projects[0].policy;
    let snapshot =
        core::policy_snapshot(&seed.policy, &seed.permissions, &seed.templates).expect("whole configured seed");
    assert_eq!(config.core.defaults.policies.get(&1), Some(seed), "welcome copies this exact configured policy");
    assert!(engine::refused(generated().0, &limits).is_empty());
    let mut strict = limits;
    strict.policy_bytes = 4096;
    assert!(engine::refused(generated().0, &strict).is_empty(), "the actual landing join also admits at 4096");
    let mut driver = Driver::cold(Store::with_limits(&limits), config, &limits);
    // Chosen GitHub-shaped owner: 39 ASCII bytes; repositories reach the
    // unchanged adoption field bound (64), not GitHub's external name limit.
    // Admission below proves acceptance by the current code, without live API claims.
    let owner = vec![b'o'; 39];
    assert_eq!(limits.forge.name_bytes, 64);
    let first = vec![b'a'; 64];
    let second = vec![b'b'; 64];
    driver.forge = Some(ForgeStand::with(&[(&owner, &first, 7), (&owner, &second, 8)]).at_forge(2));
    driver.settle();
    let before = policies(&driver);
    assert_eq!(before, vec![(1, snapshot.clone())]);
    driver.send(Event::Party(PartyInput::SignedIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(900)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 1, subject: b"owner-a".as_slice().into() },
            login: b"ada".as_slice().into(),
            name: b"Ada".as_slice().into(),
        },
    }));
    driver.send(Event::Forge(Box::new(forge::Event::Connection {
        forge: 2,
        state: forge::Connection::Connected { id: 1, login: b"app[bot]".as_slice().into() },
    })));
    driver.settle();
    assert_eq!(policies(&driver), before, "connection and sign-in do not rewrite the seed policy");
    for (key, name, repository) in [(1, &first, 7), (2, &second, 8)] {
        ask(
            &mut driver,
            key,
            people::Ask::MakeProject {
                // Welcome permits editing the project name independently; jig caps it at 64.
                name: str::from_utf8(&name[..people::NAME_BYTES]).expect("ASCII project name").into(),
                policy: 1,
            },
        );
        let project = if key == 1 {
            assert_eq!(reply(&driver, key), &people::Reply::Outcome(people::Outcome::ProjectMade { project: 2 }));
            2
        } else {
            assert_eq!(
                reply(&driver, key),
                &people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Limit {
                    which: people::LimitKind::Projects
                })),
                "second welcome selection exceeds the unchanged two-project capacity"
            );
            // Independently price two real adoptions using the already generated
            // project. This is the ordinary adoption boundary, not a welcome fallback.
            1
        };
        let adoption = forge::Adoption {
            project,
            forge: 2,
            host: b"github.com".as_slice().into(),
            owner: owner.clone().into_boxed_slice(),
            name: name.clone().into_boxed_slice(),
            prefix: b"temper/".as_slice().into(),
            role: forge::Role::Owned,
            landing: b"main".as_slice().into(),
            ci: true,
            checks: Box::new([]),
            home: true,
        };
        // Welcome sends a typed application; its real route locates the
        // repository before producing the generic, core-admitted Adopt.
        driver.send(Event::Party(PartyInput::Application {
            scope: people::ApplicationScope::Project(project),
            reply_to: ReplyTo::new(Token::new(u64::from(key + 10))),
            sign_in: driver.session(),
            key: [key + 10; 16],
            digest: [key; 32],
            carried: None,
            request: engine::Application::Forge(forge::Ask::Adopt(adoption)),
        }));
        driver.settle();
        let adopted = driver
            .delivered
            .iter()
            .find_map(|delivery| {
                if let Delivery::Application { to, outcome } = delivery
                    && *to == ReplyTo::new(Token::new(u64::from(key + 10)))
                {
                    Some(outcome)
                } else {
                    None
                }
            })
            .expect("completed adoption");
        let Some(engine::ApplicationAnswer::Adopt(forge::ApplicationOutcome::Adopted(adopted))) = adopted else {
            panic!("long-name repository adopted: {adopted:?}")
        };
        assert_eq!(adopted.repository, skein_forge::api::Repository { forge: 2, repository });
        let Some(Record::Forge { row }) =
            driver.store.rows.get(&Key::Forge(forge::Key::Repository(adopted.repository)))
        else {
            panic!("committed adopted repository")
        };
        let forge::Stored::Repository(row) = row.as_ref() else { panic!("repository key names its row") };
        assert_eq!((row.project, row.home), (project, true));
        let after = policies(&driver);
        assert_eq!(after.len(), 2);
        for (_, value) in &after {
            assert_eq!(value, &snapshot, "adoption adds no policy grants, requirements or metadata");
        }
    }
    let bytes = people::policy_bytes(&snapshot).expect("exact joined snapshot bytes");
    assert_eq!(bytes, 3234);
    std::eprintln!(
        "real init joined policy: {bytes}/4096 requested bytes, configured bound {}; projects 1 and 2; adopted=2; owner=39, repositories=64 bytes each; second welcome MakeProject refuses Projects",
        limits.policy_bytes
    );
    assert!(bytes <= 4096);
}
