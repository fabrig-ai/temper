//! T5: the init profile's answer retention can stop all new deciding requests.
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{Duration, ReplyTo, Token};
use temper_engine_domain::{Delivery, Event, Key, PartyInput};
use temper_engine_domain_world::{direct::Driver, store::Store, walking};

const SEED: u64 = 9765;
// temper-init/template/temper.json selects Small without people overrides.
const ANSWERS: u8 = 32;
const RETENTION: Duration = Duration::from_secs(86_400);

fn fund(driver: &mut Driver, key: u8, role: bool) -> people::Reply {
    let change = if role {
        people::PolicyChange::RoleSpend { role: 0, period_spend: 500 }
    } else {
        people::PolicyChange::ProjectSpend { period_spend: 1000 }
    };
    let to = Token::new(8000 + u64::from(key));
    driver.send(Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(to),
        sign_in: driver.session(),
        key: [key; 16],
        ask: people::Ask::ChangePolicy { project: 1, change },
    }));
    driver.settle();
    driver
        .delivered
        .iter()
        .rev()
        .find_map(|delivery| {
            if let Delivery::WebReply { to: found, reply } = delivery
                && *found == ReplyTo::new(to)
            {
                Some(reply.clone())
            } else {
                None
            }
        })
        .expect("every act receives a terminal transport answer")
}

fn full() -> Driver {
    let mut limits = walking::limits();
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../../../../crates/temper-init/template/temper.json"))
            .expect("init template is JSON");
    let configured = &document["engine"]["limits"];
    assert_eq!(configured["profile"], "small");
    let small = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    limits.people.requests = configured["overrides"]["people.requests"]
        .as_u64()
        .map_or(small.core.people.requests, |value| u32::try_from(value).expect("request count fits"));
    limits.people.request_retention = configured["overrides"]["people.request_retention_ms"]
        .as_u64()
        .map_or(small.core.people.request_retention, Duration::from_millis);
    assert_eq!(limits.people.request_retention, RETENTION);
    limits.journal.writes = walking::writes(&limits);
    let mut driver =
        Driver::configured(Store::with_limits(&limits), [walking::config(SEED), walking::config(SEED)], &limits);
    driver.settle();
    driver.sign_in();
    driver.settle();
    for key in 1..=ANSWERS {
        assert_eq!(
            fund(&mut driver, key, false),
            people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })
        );
    }
    let retained = driver
        .store
        .rows
        .keys()
        .filter(|key| matches!(key, Key::Core(core::Key::People(people::Key::Answer(_)))))
        .count();
    assert_eq!(retained, usize::from(ANSWERS));
    driver
}

#[test]
fn full_answers_refuse_new_keys_but_replay_and_restart_preserve_them_until_expiry() {
    let mut driver = full();
    let busy = people::Reply::Again(people::Again::Busy);
    assert_eq!(fund(&mut driver, 33, true), busy);
    assert_eq!(fund(&mut driver, 34, false), busy);
    assert_eq!(fund(&mut driver, 1, false), people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }));
    let limits = driver.env.limits;
    let mut driver = Driver::cold(driver.store, walking::config(SEED), &limits);
    driver.settle();
    driver.sign_in();
    driver.settle();
    assert_eq!(fund(&mut driver, 33, true), busy, "restart restores the full retained table");
    driver.fire_at(RETENTION);
    driver.settle();
    driver.delivered.clear();
    driver.sign_in();
    driver.settle();
    assert_eq!(fund(&mut driver, 33, true), people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }));
}

#[test]
fn the_init_answer_budget_allows_a_role_restore_after_thirty_two_decisions() {
    let mut driver = full();
    assert_eq!(
        fund(&mut driver, 33, true),
        people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 }),
        "T5 defect: a modest session exhausts init's entire day of deciding requests"
    );
}
