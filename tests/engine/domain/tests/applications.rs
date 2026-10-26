//! The forge's keyed adoption through the root (domain/forge.md, 4).
use jig_domain_people as people;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as engine;
use temper_engine_domain::Delivery;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures};

#[test]
fn an_adoption_of_a_repository_the_forge_lacks_is_refused_as_not_found() {
    let mut driver = driver();
    let mut request = fixtures::adoption(1);
    request.name = Box::from(&b"none"[..]);
    adopt(&mut driver, request, 95);
    assert_eq!(
        outcomes(&driver),
        [&Some(temper_engine_domain::ApplicationAnswer::Adopt(forge::ApplicationOutcome::ForgeRefused(
            forge::AdoptionRefusal::NotFound
        )))]
    );
    assert_eq!(driver.forge.as_ref().expect("forge stand").calls, 1);
}

#[test]
fn a_partys_own_adopt_ask_is_refused_as_not_offered() {
    let mut driver = driver();
    let ask = engine::adopt_repository_ask(
        fixtures::adoption(1),
        skein_forge::api::Repository { forge: 1, repository: 2 },
        0,
    )
    .expect("adoption");
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(95)),
        sign_in: driver.session(),
        key: [95; 16],
        ask,
    }));
    driver.settle();
    assert!(driver.delivered.iter().any(|delivery| matches!(
        delivery,
        Delivery::WebReply { reply: people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered)), .. }
    )));
    assert_eq!(driver.forge.as_ref().expect("forge stand").calls, 0);
}

fn seeded_roles(driver: &Driver) -> (Box<[people::Holding]>, std::collections::BTreeMap<u64, u64>) {
    let mut identities = std::collections::BTreeMap::new();
    let mut roles = None;
    for row in driver.store.rows.values() {
        if let engine::Record::Core(jig_domain_core::Record::People(people::Stored::Person { number, identity })) = row
        {
            let subject: [u8; 8] = identity.key.subject.as_ref().try_into().expect("forge subject is a user number");
            identities.insert(u64::from_be_bytes(subject), *number);
        }
        if let engine::Record::Core(jig_domain_core::Record::People(people::Stored::Roles { project: 1, holdings })) =
            row
        {
            roles = Some(holdings.clone());
        }
    }
    (roles.expect("durable project roles"), identities)
}

#[test]
fn a_repository_adopted_seeds_its_collaborators_into_roles() {
    let mut driver = driver();
    adopt(&mut driver, fixtures::adoption(1), 95);
    let (roles, people) = seeded_roles(&driver);
    assert!(roles.contains(&people::Holding { person: people[&7], role: people::Role::Owner }));
    assert!(roles.contains(&people::Holding { person: people[&1], role: people::Role::Maintainer }));
    assert!(roles.contains(&people::Holding { person: people[&2], role: people::Role::Member }));
}

#[test]
fn policy_maps_connector_permissions_to_seeded_roles_after_restart() {
    let mut driver = driver();
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(94)),
        sign_in: driver.session(),
        key: [94; 16],
        ask: people::Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::Permissions {
                mappings: Box::new([
                    people::PermissionRole { connector: 0, permission: 3, role: 3 },
                    people::PermissionRole { connector: 0, permission: 2, role: 1 },
                ]),
            },
        },
    }));
    driver.settle();
    assert!(
        driver.store.rows.contains_key(&engine::Key::Core(jig_domain_core::Key::People(people::Key::Policy(1)))),
        "permission policy committed"
    );
    driver.root = engine::Domain::new(temper_engine_domain_world::walking::config(71), &driver.env.limits)
        .expect("valid engine configuration");
    driver.delivered.clear();
    driver.send(engine::Event::Restart { pending: false });
    driver.settle();
    driver.sign_in();
    driver.settle();
    adopt(&mut driver, fixtures::adoption(1), 95);
    let (roles, people) = seeded_roles(&driver);
    assert!(roles.contains(&people::Holding { person: people[&7], role: people::Role::Owner }));
    assert!(roles.contains(&people::Holding { person: people[&1], role: people::Role::Observer }));
    assert!(roles.contains(&people::Holding { person: people[&2], role: people::Role::Maintainer }));
}

fn driver() -> Driver {
    let mut driver = Driver::forge();
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver
}
fn adopt(driver: &mut Driver, request: forge::Adoption, reply: u64) {
    driver.send(engine::Event::Party(engine::PartyInput::Application {
        scope: jig_domain_people::ApplicationScope::Project(request.project),
        reply_to: ReplyTo::new(Token::new(reply)),
        sign_in: driver.session(),
        key: [95; 16],
        digest: [5; 32],
        carried: None,
        request: temper_engine_domain::Application::Forge(forge::Ask::Adopt(request)),
    }));
    driver.settle();
}
fn outcomes(driver: &Driver) -> Vec<&Option<temper_engine_domain::ApplicationAnswer>> {
    driver
        .delivered
        .iter()
        .filter_map(|delivery| if let Delivery::Application { outcome, .. } = delivery { Some(outcome) } else { None })
        .collect()
}

#[test]
fn an_adoption_naming_a_forge_not_configured_is_refused_as_no_such_forge() {
    let mut driver = driver();
    adopt(&mut driver, fixtures::adoption(2), 95);
    assert_eq!(
        outcomes(&driver),
        [&Some(temper_engine_domain::ApplicationAnswer::Adopt(forge::ApplicationOutcome::ForgeRefused(
            forge::AdoptionRefusal::NoSuchForge
        )))]
    );
    assert_eq!(driver.forge.as_ref().expect("a forge stand").calls, 0);
}

#[test]
fn an_adoption_on_a_configured_forge_is_adopted_with_what_temper_may_do() {
    let mut driver = driver();
    adopt(&mut driver, fixtures::adoption(1), 95);
    assert_eq!(
        outcomes(&driver),
        [&Some(temper_engine_domain::ApplicationAnswer::Adopt(forge::ApplicationOutcome::Adopted(
            forge::AdoptedRepository {
                repository: skein_forge::api::Repository { forge: 1, repository: 2 },
                owner: Box::from(&b"org"[..]),
                name: Box::from(&b"repo"[..]),
                role: forge::Role::Owned,
                home: true,
                kinds: forge::Kinds {
                    read: true,
                    push: true,
                    open: true,
                    land: true,
                    review: true,
                    status: true,
                    comment: true,
                    issue: true,
                    branch: true
                },
                protection: forge::Protection::Absent,
            }
        )))]
    );
}

#[test]
fn an_adoption_sent_again_under_its_key_is_answered_its_kept_outcome() {
    let mut driver = driver();
    adopt(&mut driver, fixtures::adoption(1), 95);
    let calls = driver.forge.as_ref().expect("forge stand").calls;
    let first = outcomes(&driver)[0].clone();
    adopt(&mut driver, fixtures::adoption(1), 96);
    assert_eq!(outcomes(&driver), [&first, &first]);
    assert_eq!(driver.forge.as_ref().expect("forge stand").calls, calls);
}

#[test]
fn an_adoption_the_forge_user_may_not_read_is_refused_as_no_access() {
    let mut driver = driver();
    let mut request = fixtures::adoption(1);
    request.name = b"private".as_slice().into();
    adopt(&mut driver, request, 95);
    assert_eq!(
        outcomes(&driver),
        [&Some(temper_engine_domain::ApplicationAnswer::Adopt(forge::ApplicationOutcome::ForgeRefused(
            forge::AdoptionRefusal::NoAccess
        )))]
    );
}

#[test]
fn an_adoption_sent_again_after_a_restart_is_answered_its_kept_outcome() {
    let mut first = driver();
    adopt(&mut first, fixtures::adoption(1), 95);
    let outcome = outcomes(&first)[0].clone();
    let calls = first.forge.as_ref().expect("forge stand").calls;
    let mut restarted = Driver::configured(
        first.store,
        [temper_engine_domain_world::walking::config(71), temper_engine_domain_world::walking::config(71)],
        &fixtures::forge_limits(),
    );
    restarted.forge = first.forge;
    restarted.settle();
    restarted.sign_in();
    restarted.settle();
    let calls_after_restart = restarted.forge.as_ref().expect("forge stand").calls;
    assert!(calls_after_restart >= calls);
    adopt(&mut restarted, fixtures::adoption(1), 96);
    assert_eq!(outcomes(&restarted), [&outcome]);
    assert_eq!(restarted.forge.as_ref().expect("forge stand").calls, calls_after_restart);
}

#[test]
fn a_person_without_an_owner_role_returns_once_and_replays_the_answer() {
    let mut driver = Driver::forge();
    driver.settle();
    driver.send(engine::Event::Party(engine::PartyInput::SignedIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(93)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 99_u64.to_be_bytes().into() },
            login: b"visitor".as_slice().into(),
            name: b"Visitor".as_slice().into(),
        },
    }));
    driver.settle();
    let sign_in = driver.session();
    driver.delivered.clear();
    for replay in [false, true] {
        driver.send(engine::Event::Party(engine::PartyInput::Application {
            // Return always uses Own admission, even when the envelope names a project.
            scope: people::ApplicationScope::Project(u32::MAX),
            reply_to: ReplyTo::new(Token::new(94)),
            sign_in,
            key: [94; 16],
            digest: [94; 32],
            carried: None,
            request: engine::Application::Return {
                source: 7,
                path: b"/accounts/connect?provider=1&way=sign-in&source=7".as_slice().into(),
            },
        }));
        driver.settle();
        assert_eq!(outcomes(&driver), [&Some(engine::ApplicationAnswer::Returning)], "replay {replay}");
        assert_eq!(
            driver.protocol.returns,
            [(7, Box::from(b"/accounts/connect?provider=1&way=sign-in&source=7".as_slice()))]
        );
        driver.delivered.clear();
    }
    assert_eq!(driver.forge.as_ref().expect("forge stand").calls, 0);
}

fn return_ask(sign_in: u64) -> engine::Event {
    engine::Event::Party(engine::PartyInput::Application {
        scope: people::ApplicationScope::Own,
        reply_to: ReplyTo::new(Token::new(94)),
        sign_in,
        key: [94; 16],
        digest: [94; 32],
        carried: None,
        request: engine::Application::Return { source: 7, path: b"/welcome?at=ready".as_slice().into() },
    })
}
#[test]
fn a_return_replayed_after_restart_is_unknown_without_a_second_output() {
    let mut first = driver();
    first.send(return_ask(first.session()));
    first.settle();
    assert_eq!(outcomes(&first), [&Some(engine::ApplicationAnswer::Returning)]);
    let mut restarted = Driver::configured(
        first.store,
        [temper_engine_domain_world::walking::config(71), temper_engine_domain_world::walking::config(71)],
        &fixtures::forge_limits(),
    );
    restarted.forge = first.forge;
    restarted.settle();
    restarted.sign_in();
    restarted.settle();
    restarted.send(return_ask(restarted.session()));
    restarted.settle();
    assert_eq!(outcomes(&restarted), [&None]);
    assert!(restarted.protocol.returns.is_empty());
}
