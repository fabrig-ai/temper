//! R12: an authorized person's unread words on a delegate must restore
//! (jig's domain/people.md, 5.1; domain/engine.md, 6).
use super::*;

const CHANNEL: Token = Token::new(7);
const PROJECT: u32 = 1;
const OWNER: u64 = 1;
const MODEL: &[u8] = b"gpt-6.1-sol";
const EFFORT: &[u8] = b"high";
const WORDS: &[u8] = b"please complete the task";
const WORKERS: u32 = 2; // The chat and its one delegate run together.
const DEPTH: u32 = 2; // The existing batch fixture's two-level tree.
const POOL: u64 = 500; // The existing batch fixture's owner's pool.
const DELEGATE_CALL: u64 = 912;
const MESSAGE_KEY: u8 = 40;
const FIRST_RELEASE: u8 = 41;
const SECOND_RELEASE: u8 = 42;
const AMENDMENT_KEY: u8 = 43;
// Walking's retry policy allows one retry after the first failure.
const FAILED_RUNS: usize = 2;
const THIRD_HOLD: u64 = 3; // Initial hold plus one after each release.

fn settings() -> ([engine::Config; 2], engine::Limits) {
    let (mut configurations, bounds) = batch_settings(WORKERS, DEPTH, POOL, false, false, false);
    for configuration in &mut configurations {
        let mut model = configuration.core.settings.models[0].clone();
        model.name = MODEL.into();
        model.efforts = Box::new([EFFORT.into()]);
        model.effort = Some(EFFORT.into());
        configuration.core.defaults.charters[0].policy.model = model.clone();
        configuration.core.settings.models = Box::new([model]);
        configuration.core.settings.llm_providers.providers[0].models = Box::new([MODEL.into()]);
    }
    (configurations, bounds)
}

fn delegate() -> (Driver, u64) {
    let (configurations, bounds) = settings();
    let mut driver = Driver::configured(Store::new(), configurations, &bounds);
    let mut hello = hello_with(CHANNEL, WORKERS, Duration::from_secs(1), Box::new([]));
    if let engine::Event::Worker(engine::WorkerInput::Link { link: engine::Link::Hello { hello, .. }, .. }) = &mut hello
    {
        hello.bounds.models[0].name = MODEL.into();
    }
    driver.send(hello);
    driver.settle();
    driver.sign_in();
    driver.settle();
    chat(&mut driver, 12);
    driver.settle();
    let parent = driver.last_assignment();
    let children =
        call_batch(&mut driver, &parent, DELEGATE_CALL, Box::new([report_delegate(b"restart proof", Box::new([]))]));
    (driver, children[0])
}

fn hold(driver: &mut Driver, task: u64) {
    for _ in 0..FAILED_RUNS {
        let attempt = live(driver, task).attempt;
        driver.answer(CHANNEL, task, attempt, 0, tasks::End::Failed(tasks::Class::Invalid), engine::Parts::none());
        driver.settle();
        driver.fire_at(Duration::from_nanos(driver.env.now.as_nanos() + Duration::from_secs(2).as_nanos()));
        driver.settle();
    }
}

fn ask(driver: &mut Driver, key: u8, ask: people::Ask) {
    let sign_in = driver
        .delivered
        .iter()
        .rev()
        .find_map(|delivery| {
            if let Delivery::WebReply { reply: people::Reply::SignedIn { sign_in, .. }, .. } = delivery {
                Some(*sign_in)
            } else {
                None
            }
        })
        .expect("latest owner session");
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(u64::from(key))),
        sign_in,
        key: [key; 16],
        ask,
    }));
    driver.settle();
}

fn release(driver: &mut Driver, task: u64, key: u8) {
    let revision = live(driver, task).revisions.hold;
    ask(driver, key, people::Ask::Release { project: PROJECT, task, revision, again: None });
}

fn amend_model(driver: &mut Driver, task: u64) {
    let revision = live(driver, task).revisions.spec;
    let mut amendment = person_amendment(None, None, b"explicit model and effort");
    amendment.llm = Some(people::LlmChoice {
        account: None,
        main: people::ModelEffort { model: 0, effort: Some(0) },
        subagents: Box::new([]),
    });
    ask(driver, AMENDMENT_KEY, people::Ask::Amend { project: PROJECT, task, revision, amendment, release: None });
}

fn twice_released(words: Option<&[u8]>) -> (Driver, u64) {
    let (mut driver, task) = delegate();
    hold(&mut driver, task);
    let tasks::Party::Task(parent) = live(&driver, task).requester else { panic!("delegate requester") };
    hold(&mut driver, parent);
    // Keep the fixture's four-answer cap: expire the chat's creation key
    // while both tasks are held, then renew the fixture's shorter sign-in.
    driver.fire_at(Duration::from_nanos(
        driver.env.now.as_nanos() + driver.env.limits.people.request_retention.as_nanos() + 1,
    ));
    driver.settle();
    driver.sign_in();
    driver.settle();
    if let Some(words) = words {
        ask(&mut driver, MESSAGE_KEY, people::Ask::Say { project: PROJECT, task, words: words.into() });
    }
    release(&mut driver, task, FIRST_RELEASE);
    hold(&mut driver, task);
    release(&mut driver, task, SECOND_RELEASE);
    hold(&mut driver, task);
    (driver, task)
}

fn cold(driver: Driver) -> Driver {
    let ([_, configuration], bounds) = settings();
    let mut restarted = Driver::cold(driver.store, configuration, &bounds);
    restarted.pump();
    restarted
}

#[test]
fn messaged_released_amended_delegate_survives_cold_restart() {
    let (mut driver, task) = twice_released(Some(WORDS));
    amend_model(&mut driver, task);
    let restarted = cold(driver);
    assert!(restarted.root.ready(), "cold restore refused: {:?}", restarted.root.restore_failure());
}

#[test]
fn person_messaged_delegate_survives_cold_restart() {
    let (mut driver, task) = delegate();
    let message = driver.say(task, MESSAGE_KEY);
    let requester = live(&driver, task).requester;
    let restarted = cold(driver);
    assert!(
        restarted.root.ready(),
        "Live({task}) inbox word {message} from Person({OWNER}), requester {requester:?}: {:?}",
        restarted.root.restore_failure()
    );
}

#[test]
fn unmessaged_released_amended_delegate_survives_cold_restart() {
    let (mut driver, task) = twice_released(None);
    amend_model(&mut driver, task);
    let restarted = cold(driver);
    assert!(restarted.root.ready(), "cold restore refused: {:?}", restarted.root.restore_failure());
}

#[test]
fn unmessaged_held_delegate_survives_cold_restart() {
    let (mut driver, task) = delegate();
    hold(&mut driver, task);
    let restarted = cold(driver);
    assert!(restarted.root.ready(), "cold restore refused: {:?}", restarted.root.restore_failure());
}

#[test]
fn release_and_amend_fixture_commits_each_requested_operation() {
    let (mut driver, task) = twice_released(Some(WORDS));
    amend_model(&mut driver, task);
    let row = live(&driver, task);
    assert_eq!(row.revisions.hold, THIRD_HOLD);
    assert!(matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Failures(tasks::Class::Invalid), .. }));
    assert!(replied(&driver, &people::Outcome::Amended { task, released: false }));
    assert!(
        row.inbox.iter().any(|word| word.kind == tasks::MessageKind::Words && word.from == tasks::Party::Person(OWNER))
    );
    assert!(driver.store.rows.contains_key(&Key::Core(jig_domain_core::Key::People(people::Key::Person(OWNER)))));
}
