//! A core procedure document can precede the forge child's change record.
use jig_domain_core::documents::{Watched, Window};
use jig_domain_people::ApplicationScope;
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as root;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures, forge_stand::ForgeStand, walking};

const TASK: u64 = 51;
const EARLY: Token = Token::new(700);
const LATE: Token = Token::new(701);

fn before_change() -> Driver {
    // Make and park the core's procedure independently of the forge child:
    // this is the boundary between the task document and its change record.
    let mut core = jig_tasks_world::World::new(71, jig_tasks_world::LIMITS);
    let mut new = jig_tasks_world::task(TASK, &[]);
    new.executor = tasks::Executor::Procedure { connector: 0, code: 2 };
    new.contract = tasks::Contract::Change { connector: 0, kind: 1, words: 32 };
    new.spec.parameters = Box::new([tasks::Parameter::Resource { name: 1, connector: 0, resource: forge::tag(1, 2) }]);
    new.authority = fixtures::report_delegate(b"change", Box::new([])).authority;
    new.numbers.budget = new.authority.budget.spend;
    assert_eq!(core.make(tasks::Party::Person(1), vec![new]), jig_tasks_world::Reply::Made(vec![TASK]));
    core.send(tasks::Event::Procedure {
        reply_to: ReplyTo::new(Token::new(99)),
        task: TASK,
        step: 1,
        read: None,
        decision: tasks::ProcedureDecision::new(tasks::ProcedureEnd::Wait),
    });
    assert_eq!(core.record(TASK).phase, tasks::Phase::Active(tasks::Active::Idle));

    let limits = fixtures::forge_limits();
    let mut store = fixtures::restored_store(walking::config(71), &limits);
    let mut header = store.header();
    header.tasks = TASK;
    let mut records: Vec<_> =
        core.records.into_values().map(|record| root::Record::Core(jig_domain_core::Record::Tasks(record))).collect();
    records.push(root::Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(header))));
    store.seed(records.into_boxed_slice(), Box::new([]));
    let mut driver = Driver::configured(store, [walking::config(71), walking::config(71)], &limits);
    driver.forge = Some(ForgeStand::new());
    driver.settle();
    assert!(driver.root.ready() && !driver.stopped, "restored root: {:?}", driver.root.restart_failure());
    driver.sign_in();
    driver.settle();
    driver.send(root::Event::Party(root::PartyInput::Application {
        scope: ApplicationScope::Project(1),
        reply_to: ReplyTo::new(Token::new(95)),
        sign_in: driver.session(),
        key: [95; 16],
        digest: [5; 32],
        carried: None,
        request: root::Application::Forge(forge::Ask::Adopt(fixtures::adoption(1))),
    }));
    driver.settle();
    assert_eq!(
        driver.task(TASK).expect("core task exists").executor,
        tasks::Executor::Procedure { connector: 0, code: 2 }
    );
    driver
}

fn watch(driver: &mut Driver, watcher: Token) {
    driver.send(root::Event::Party(root::PartyInput::Watch {
        watcher,
        sign_in: driver.session(),
        subject: Watched::Application { scope: ApplicationScope::Project(1), connector: 0, subject: Token::new(TASK) },
        window: Window { rows: 0, after: None },
    }));
    driver.settle();
}

#[test]
fn a_panel_before_its_change_ends_gone_and_a_fresh_watch_reads_the_change() {
    let mut driver = before_change();
    watch(&mut driver, EARLY);
    assert!(
        driver.delivered.iter().any(|delivery| matches!(delivery,
            root::Delivery::WatchEnded { watcher, why: jig_domain_views::End::Gone } if *watcher == EARLY
        )),
        "the pre-record watch ended Gone: {:?}",
        driver.delivered
    );
    driver.send(root::Event::Forge(Box::new(forge::Event::Change {
        row: temper_engine_forge_world::changes::change_row(),
    })));
    driver.settle();
    watch(&mut driver, LATE);
    assert!(driver.delivered.iter().any(|delivery| matches!(delivery,
        root::Delivery::Panel { watcher, panel } if *watcher == LATE && panel.task == TASK && panel.step == forge::PanelStep::Producing
    )), "a fresh watch can read the newly made change: {:?}", driver.delivered);
    let original: Vec<_> = driver
        .delivered
        .iter()
        .filter(|delivery| {
            matches!(delivery,
                root::Delivery::Panel { watcher, .. } | root::Delivery::WatchEnded { watcher, .. } if *watcher == EARLY
            )
        })
        .collect();
    assert!(
        original.len() == 1
            && matches!(original[0], root::Delivery::WatchEnded { why: jig_domain_views::End::Gone, .. }),
        "the original watch stays ended Gone: {original:?}"
    );
}
