//! Funding periods from the root's calendar, over the real store.
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token, Wall};
use temper_engine_domain::{self as engine, Key, Record, Write};
use temper_engine_domain_world::{
    direct::Driver,
    store::Store,
    walking::{PERIOD_SPAN, config, limits},
};

fn ledger(driver: &Driver, project: u32, period: u64) -> tasks::FundingRecord {
    let key = Key::Core(core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period { project, period })));
    let Some(Record::Core(core::Record::Tasks(tasks::Stored::Ledger(row)))) = driver.store.rows.get(&key) else {
        panic!("period {period} of project {project} is stored");
    };
    *row
}
fn periods(driver: &Driver, project: u32) -> Vec<u64> {
    driver
        .store
        .rows
        .values()
        .filter_map(|row| {
            if let Record::Core(core::Record::Tasks(tasks::Stored::Ledger(tasks::FundingRecord {
                funder: tasks::Funder::Period { project: owner, period },
                ..
            }))) = row
                && *owner == project
            {
                Some(*period)
            } else {
                None
            }
        })
        .collect()
}

fn next(driver: &Driver, project: u32, period: u64) -> Wall {
    ledger(driver, project, period).period.expect("period times").next.expect("calendar supplied next start")
}
fn driver(seed: u64) -> Driver {
    let mut driver = Driver::configured(Store::new(), [config(seed), config(seed)], &limits());
    driver.settle();
    fund(&mut driver, 1000);
    driver
}

fn fund(driver: &mut Driver, budget: u64) {
    driver.period_sent = true;
    driver.send(engine::Event::Party(engine::PartyInput::Period { project: 1, period: 1, budget, next: None }));
    driver.settle();
}

fn reserve_in_a_chat(d: &mut Driver) {
    d.sign_in();
    d.settle();
    let people::Ask::Create { project, mut batch } = d.chat_ask(1, b"keep the old period's reservation") else {
        unreachable!("chat fixture creates a task");
    };
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 50;
    d.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(10400)),
        sign_in: d.session(),
        key: [103; 16],
        ask: people::Ask::Create { project, batch },
    }));
    d.pump();
    assert!(ledger(d, 1, 1).numbers.reserved > 0);
}

#[test]
fn a_period_is_renewed_at_its_next_start() {
    let mut d = driver(10401);
    assert_eq!(d.root.next_deadline(&d.env), Some(skein_lib::Time::from_nanos(next(&d, 1, 1).as_nanos())));
    reserve_in_a_chat(&mut d);
    let first = ledger(&d, 1, 1);
    let times = first.period.expect("period times");
    assert_eq!(times.next, Some(Wall::from_nanos(times.began.as_nanos() + PERIOD_SPAN.as_nanos())));
    d.fire_at(PERIOD_SPAN);
    d.pump();
    let second = ledger(&d, 1, 2);
    assert_eq!(second.numbers, tasks::Numbers { budget: 1000, reserved: 0, spent: 0, spent_below: 0 });
    assert_eq!(
        second.period,
        Some(tasks::PeriodTimes {
            began: Wall::from_nanos(PERIOD_SPAN.as_nanos()),
            next: Some(Wall::from_nanos(2 * PERIOD_SPAN.as_nanos()))
        })
    );
    assert_eq!(ledger(&d, 1, 1), first);
}

#[test]
fn a_start_past_a_next_start_opens_one_period_from_the_starts_time() {
    let mut d = driver(10402);
    reserve_in_a_chat(&mut d);
    let first = ledger(&d, 1, 1);
    let now = 4 * PERIOD_SPAN.as_nanos();
    let mut d = Driver::from_store_at(d.store, config(10402), &limits(), Wall::from_nanos(now));
    d.pump();
    assert_eq!(periods(&d, 1), vec![1, 2]);
    assert_eq!(ledger(&d, 1, 1), first);
    assert_eq!(
        ledger(&d, 1, 2).period,
        Some(tasks::PeriodTimes {
            began: Wall::from_nanos(now),
            next: Some(Wall::from_nanos(now + PERIOD_SPAN.as_nanos()))
        })
    );
}

#[test]
fn a_first_period_with_no_next_start_after_a_crash_is_given_one() {
    let mut d = driver(10403);
    let mut first = ledger(&d, 1, 1);
    first.period.as_mut().expect("period times").next = None;
    d.store.seed(Box::new([Record::Core(core::Record::Tasks(tasks::Stored::Ledger(first)))]), Box::new([]));
    let mut d = Driver::from_store_at(d.store, config(10403), &limits(), Wall::from_nanos(100));
    d.settle();
    first.period.as_mut().expect("period times").next = Some(Wall::from_nanos(PERIOD_SPAN.as_nanos()));
    assert_eq!(ledger(&d, 1, 1), first);
    assert_eq!(periods(&d, 1), vec![1]);
}

#[test]
fn two_projects_due_at_once_are_renewed_in_one_decision() {
    let mut limits = limits();
    limits.authority.projects = 2;
    limits.people.projects = 3;
    limits.journal.writes = temper_engine_domain_world::walking::writes(&limits);
    let configs = [config(10404), config(10404)].map(|mut config| {
        let second = core::SeedProject {
            number: 2,
            name: Box::from("Second"),
            policy: config.core.seed_projects[0].policy.clone(),
        };
        config.core.seed_projects = vec![config.core.seed_projects.into_vec().remove(0), second].into_boxed_slice();
        config
    });
    let mut d = Driver::configured(Store::with_limits(&limits), configs, &limits);
    d.settle();
    d.send(engine::Event::Party(engine::PartyInput::Period { project: 2, period: 1, budget: 1000, next: None }));
    d.settle();
    assert_eq!(next(&d, 1, 1), next(&d, 2, 1));
    let before = d.transactions.len();
    d.fire_at(PERIOD_SPAN);
    d.settle();
    assert_eq!(ledger(&d, 1, 2).numbers.budget, 1000);
    assert_eq!(ledger(&d, 2, 2).numbers.budget, 1000);
    let commits: Vec<_> = d.transactions[before..].iter().filter(|writes| writes.iter().any(|write| matches!(write, Write::Save(Record::Core(core::Record::Tasks(tasks::Stored::Ledger(row)))) if matches!(row.funder, tasks::Funder::Period { period: 2, .. })))).collect();
    assert_eq!(commits.len(), 1, "both new periods commit together");
    for project in [1, 2] {
        assert!(commits[0].iter().any(|write| matches!(write, Write::Save(Record::Core(core::Record::Tasks(tasks::Stored::Ledger(row)))) if row.funder == tasks::Funder::Period { project, period: 2 })));
    }
}

#[test]
fn nothing_is_armed_while_stopping() {
    let mut d = driver(10405);
    assert!(d.root.next_deadline(&d.env).is_some());
    let calendar_deadline = d.root.next_deadline(&d.env).expect("calendar deadline");
    d.send(engine::Event::Stop);
    // Accounts may keep their own drain timer; the calendar contributes none.
    assert!(d.root.next_deadline(&d.env).is_none_or(|next| next > calendar_deadline));
    d.fire_at(PERIOD_SPAN);
    d.pump();
    assert_eq!(periods(&d, 1), vec![1]);
}

#[test]
fn a_raised_spend_is_the_next_periods_budget() {
    let configs = [config(10406), config(10406)].map(|mut config| {
        let policy = &mut config.core.seed_projects[0].policy.policy;
        policy.period_spend = 500;
        policy.roles[0].requests = authority::Requests(1 | 4 | 256);
        config
    });
    let mut d = Driver::configured(Store::new(), configs, &limits());
    d.settle();
    fund(&mut d, 500);
    assert_eq!(ledger(&d, 1, 1).numbers.budget, 500);
    d.sign_in();
    d.settle();
    d.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(10406)),
        sign_in: d.session(),
        key: [104; 16],
        ask: people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::ProjectSpend { period_spend: 800 } },
    }));
    d.settle();
    let policy = d.store.rows.get(&Key::Core(core::Key::People(people::Key::Policy(1)))).expect("saved policy");
    assert!(
        matches!(policy, Record::Core(core::Record::People(people::Stored::Policy { value, .. })) if value.period_spend == 800)
    );
    d.fire_at(PERIOD_SPAN);
    d.settle();
    assert_eq!(ledger(&d, 1, 2).numbers.budget, 800);
}
