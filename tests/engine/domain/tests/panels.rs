//! The connector's panel follows the core's watch admission and commit barrier.
use jig_domain_core::documents::{Watched, Window};
use jig_domain_people::ApplicationScope;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as root;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures};

const WATCHER: Token = Token::new(700);
const CHANGE: u64 = 51;

fn driver() -> Driver {
    let mut driver = Driver::forge();
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver.send(root::Event::Party(root::PartyInput::Application {
        scope: ApplicationScope::Project(1),
        reply_to: ReplyTo::new(Token::new(95)),
        sign_in: driver.session(),
        key: [95; 16],
        digest: [5; 32],
        carried: None,
        request: temper_engine_domain::Application::Forge(forge::Ask::Adopt(fixtures::adoption(1))),
    }));
    driver.settle();
    driver.send(root::Event::Forge(Box::new(forge::Event::Change {
        row: temper_engine_forge_world::changes::change_row(),
    })));
    driver.settle();
    driver.immediate.clear();
    driver
}
fn watch(driver: &mut Driver, project: u32, task: u64) {
    driver.send(root::Event::Party(root::PartyInput::Watch {
        watcher: WATCHER,
        sign_in: driver.session(),
        subject: Watched::Application {
            scope: ApplicationScope::Project(project),
            connector: 0,
            subject: Token::new(task),
        },
        window: Window { rows: 1, after: None },
    }));
}
fn panels(driver: &Driver) -> Vec<&forge::Panel> {
    driver
        .delivered
        .iter()
        .filter_map(|delivery| {
            if let root::Delivery::Panel { watcher, panel } = delivery {
                assert_eq!(*watcher, WATCHER);
                Some(panel.as_ref())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn a_project_role_admits_the_watch_and_delivers_the_whole_change_panel() {
    let mut driver = driver();
    watch(&mut driver, 1, CHANGE);
    driver.settle();
    let got = panels(&driver);
    assert_eq!(got.len(), 1);
    let panel = got[0];
    assert_eq!(panel.task, CHANGE);
    assert_eq!(panel.forge, 1);
    assert_eq!(panel.owner.as_ref(), b"org");
    assert_eq!(panel.name.as_ref(), b"repo");
    assert_eq!(panel.branch.as_ref(), b"temper/51");
    assert_eq!(panel.landing.as_ref(), b"main");
    assert_eq!(panel.step, forge::PanelStep::Producing);
    assert_eq!(panel.head, None);
    assert_eq!(panel.pull, None);
    assert_eq!(panel.hold, None);
    assert_eq!(panel.runs, None);
    assert_eq!(panel.ci, temper_engine_domain_forge_change::Status::Unknown);
    assert!(panel.gates.is_empty());
    assert_eq!(panel.repairs, forge::PanelUsed { used: 0, limit: driver.env.limits.forge.change_policy.repairs });
    assert_eq!(
        panel.resolutions,
        forge::PanelUsed { used: 0, limit: driver.env.limits.forge.change_policy.resolutions }
    );
    assert_eq!(panel.updates, forge::PanelUsed { used: 0, limit: driver.env.limits.forge.change_policy.updates });
    assert!(
        !driver
            .immediate
            .iter()
            .any(|request| matches!(request, root::Request::Party(root::PartyRequest::WatchRefused { .. })))
    );
}
#[test]
fn a_party_without_a_role_in_the_scope_is_refused_before_the_connector() {
    let mut driver = driver();
    watch(&mut driver, 2, CHANGE);
    driver.settle();
    assert!(panels(&driver).is_empty());
    assert!(driver.immediate.iter().any(|request| matches!(request, root::Request::Party(root::PartyRequest::WatchRefused { watcher, why: jig_domain_core::WatchRefusal::NotAllowed }) if *watcher == WATCHER)));
}
#[test]
fn the_panel_stays_held_until_the_asking_decision_is_durable() {
    let mut driver = driver();
    watch(&mut driver, 1, CHANGE);
    driver.settle();
    assert_eq!(panels(&driver).len(), 1);
    driver.send(root::Event::Party(root::PartyInput::Delivered { watcher: WATCHER }));
    driver.settle();
    assert!(driver.store.pending.is_empty());
    // This save asks for the marked document in the same decision.
    driver.send(root::Event::Forge(Box::new(forge::Event::Delegated {
        task: CHANGE,
        child: 73,
        kind: temper_engine_domain_forge_change::Delegate::Produce,
    })));
    for _ in 0..8 {
        driver.advance(false);
    }
    assert_eq!(panels(&driver).len(), 1, "the marked panel is still held");
    assert!(!driver.store.pending.is_empty(), "the asking decision has an unacknowledged commit");
    driver.settle();
    let got = panels(&driver);
    assert_eq!(got.len(), 2);
    assert_eq!(got[1].runs, Some(73));
}
#[test]
fn a_saved_change_marks_the_forges_subject_and_delivers_the_panel_again() {
    let mut driver = driver();
    watch(&mut driver, 1, CHANGE);
    driver.settle();
    assert_eq!(panels(&driver).len(), 1);
    driver.send(root::Event::Party(root::PartyInput::Delivered { watcher: WATCHER }));
    driver.settle();
    driver.send(root::Event::Forge(Box::new(forge::Event::Delegated {
        task: CHANGE,
        child: 73,
        kind: temper_engine_domain_forge_change::Delegate::Produce,
    })));
    driver.settle();
    let got = panels(&driver);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].runs, None);
    assert_eq!(got[1].runs, Some(73));
}
#[test]
fn a_task_that_is_no_change_ends_the_watch_as_gone() {
    let mut driver = driver();
    watch(&mut driver, 1, 999);
    driver.settle();
    assert!(panels(&driver).is_empty());
    assert!(driver.delivered.iter().any(|delivery| matches!(delivery, root::Delivery::WatchEnded { watcher, why: jig_domain_views::End::Gone } if *watcher == WATCHER)));
}

#[test]
fn a_project_landings_watch_delivers_its_list_after_the_commit_barrier() {
    for ended in [false, true] {
        let mut driver = driver();
        // A preceding save supplies the durability barrier for this read-only watch.
        driver.send(root::Event::Forge(Box::new(forge::Event::Delegated {
            task: CHANGE,
            child: 73,
            kind: temper_engine_domain_forge_change::Delegate::Produce,
        })));
        watch(&mut driver, 1, forge::landings_subject(1, ended));
        for _ in 0..8 {
            driver.advance(false);
        }
        assert!(
            !driver.delivered.iter().any(|delivery| matches!(delivery, root::Delivery::ForgeDocument { .. })),
            "the whole list remains held until durability"
        );
        driver.settle();
        let documents: Vec<_> = driver
            .delivered
            .iter()
            .filter_map(|delivery| {
                if let root::Delivery::ForgeDocument { watcher, document } = delivery {
                    assert_eq!(*watcher, WATCHER);
                    Some(document.as_ref())
                } else {
                    None
                }
            })
            .collect();
        let [forge::Document::Landings(list)] = documents.as_slice() else { panic!("one whole list: {documents:?}") };
        assert_eq!(list.project, 1);
        assert_eq!(list.turn, None);
        assert!(list.landings.is_empty());
    }
}
