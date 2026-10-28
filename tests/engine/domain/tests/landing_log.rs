//! A landing log is loaded through the real durable store at a cold start.
use jig_domain_core::documents::Window;
use jig_domain_people::ApplicationScope;
use skein_lib::{ReplyTo, Token, Wall};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures, forge_stand::ForgeStand, walking};

const SEED: u64 = 71; // The direct forge fixture's deployment.
const PROJECT: u32 = 1;
const REPOSITORY: skein_forge::api::Repository = skein_forge::api::Repository { forge: 1, repository: 2 };
const WATCHER: Token = Token::new(700);
const KEPT: u8 = 16; // domain/after-landing.md, 10: small's landings_kept.

fn wrapped_log() -> forge::LandingLog {
    let mut landings = Vec::new();
    // The writer's seventeenth insertion drops commit 1: the retained ring is 2..=17.
    for number in 2..=KEPT + 1 {
        let verdict = if number % 2 == 0 { forge::Verdict::Verifying } else { forge::Verdict::Cleared };
        landings.push(forge::Logged {
            commit: [number; 32],
            seen: Wall::from_nanos(u64::from(number)),
            changes: if number % 3 == 0 { Box::new([]) } else { Box::new([u64::from(number)]) },
            verdicts: Box::new([forge::Mark { tier: b"browser".as_slice().into(), verdict }]),
        });
    }
    forge::LandingLog { repository: REPOSITORY, landings: landings.into_boxed_slice() }
}
fn read(driver: &mut Driver, sign_in: u64) -> forge::LandingsView {
    let mut lines = Vec::new();
    let mut after = None;
    let mut last = None;
    // At least one line per page; the fixture's document window is four rows.
    for page in 0..u64::from(KEPT) {
        let watcher = Token::new(WATCHER.raw() + page);
        driver.send(engine::Event::Party(engine::PartyInput::LoggedLandings {
            watcher,
            sign_in,
            scope: ApplicationScope::Project(PROJECT),
            project: PROJECT,
            newer_than: Wall::EPOCH,
            window: Window { rows: 32, after },
        }));
        driver.settle();
        let mut found = None;
        for delivery in &driver.delivered {
            if let engine::Delivery::ForgeDocument { watcher: due, document } = delivery
                && *due == watcher
                && let forge::Document::Landings(view) = &**document
            {
                found = Some(view.clone());
            }
        }
        let view = found.expect("a page of the durable landing log");
        lines.extend(view.logged.iter().cloned());
        after = match view.after {
            Some(number) => Some(jig_domain_core::documents::Cursor::Number { number }),
            None => None,
        };
        last = Some(view);
        if after.is_none() {
            break;
        }
    }
    let mut view = last.expect("at least one page");
    view.logged = lines.into_boxed_slice();
    view
}

fn cold(mut store: temper_engine_domain_world::store::Store) -> Driver {
    store.crash(SEED);
    let mut driver = Driver::cold(store, walking::config(SEED), &fixtures::forge_limits());
    driver.forge = Some(ForgeStand::new());
    driver.settle();
    assert!(
        driver.root.ready() && !driver.stopped,
        "cold admission {:?}; {:?}",
        driver.root.restart_failure(),
        driver.immediate
    );
    driver
}
#[test]
fn a_cold_restart_reads_back_the_landing_log() {
    let mut driver = Driver::forge();
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver.send(engine::Event::Party(engine::PartyInput::Application {
        scope: ApplicationScope::Project(PROJECT),
        reply_to: ReplyTo::new(Token::new(95)),
        sign_in: driver.session(),
        key: [95; 16],
        digest: [5; 32],
        carried: None,
        request: engine::Application::Forge(forge::Ask::Adopt(fixtures::adoption(REPOSITORY.forge))),
    }));
    driver.settle();
    let sign_in = driver.session();
    driver.store.seed(
        Box::new([engine::Record::Forge { row: Box::new(forge::Stored::LandingLog(wrapped_log())) }]),
        Box::new([]),
    );
    let mut driver = cold(driver.store);
    let before = read(&mut driver, sign_in);
    let mut restarted = cold(driver.store);
    let after = read(&mut restarted, sign_in);
    assert!(restarted.root.ready() && !restarted.stopped, "cold admission: {:?}", restarted.root.restart_failure());
    assert_eq!(before.logged.len(), usize::from(KEPT));
    assert_eq!(before.logged[0].commit, [17; 32]);
    for (line, entry) in before.logged.iter().rev().zip(wrapped_log().landings.iter()) {
        assert_eq!(line.repository.as_ref(), b"org/repo");
        assert_eq!(
            (line.commit, line.seen, &line.changes, &line.verdicts),
            (entry.commit, entry.seen, &entry.changes, &entry.verdicts)
        );
    }
    assert_eq!(before, after);
}
