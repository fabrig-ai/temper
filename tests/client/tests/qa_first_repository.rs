//! First-repository promises exercised through the person and ctl on the real engine.
use jig_client_world::harness::{ClientId, Until, World};
use skein_fake_person::{Find, Step};
use skein_lib::Token;
use skein_ui::Role;
use temper_client_domain::ChatsFilter;
use temper_client_domain::{Address, Choice};
use temper_client_world::{app::Temper, fixtures, front::Joint};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, walking};

type ClientWorld = World<Temper, Joint>;
const FORGE: u16 = 1;
const REPOSITORY: u32 = 7;

fn configured(under_box: bool) -> ClientWorld {
    let mut world = fixtures::world(fixtures::SEED);
    world.randomness(0, 0, 0);
    let store = world.front().engine.store.fork();
    let mut limits = world.front().engine.env.limits;
    let configs = std::array::from_fn(|_| {
        let mut config = walking::config(fixtures::SEED);
        config.forges = Box::new([forge::Configured {
            admin_connect: false,
            bot: Box::new([]),
            owned: None,
            git: Box::new([]),
            web: None,
            read_only: false,
            number: FORGE,
            kind: forge::Kind::Forgejo,
            mode: forge::Mode::Owned,
            asked: true,
        }]);
        config.under_box = under_box;
        config.core.connectors = Box::new([temper_client_protocol_wire::FORGE]);
        temper_client_world::front::first_run_projects(&mut config, &mut limits);
        config
    });
    let mut driver = Driver::with_repositories(store, configs, &limits, fixtures::REPOSITORIES);
    driver.hello(Token::new(7));
    driver.settle();
    world.front().engine = driver;
    connection(&mut world, forge::Connection::NotSetUp);
    world
}
fn connection(world: &mut ClientWorld, state: forge::Connection) {
    send(world, forge::Event::Connection { forge: FORGE, state });
}
fn send(world: &mut ClientWorld, event: forge::Event) {
    world.front().engine.send(engine::Event::Forge(Box::new(event)));
}
fn flow() -> Address {
    Address::ConnectForge { forge: Some(Choice::Forge(FORGE)) }
}
fn script(world: &mut ClientWorld, person: ClientId, steps: Vec<Step<Address>>) {
    world.person(person, steps);
    let outcome = world.run(Until::Steps(256));
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
}
fn see(name: &'static [u8]) -> Step<Address> {
    Step::See { find: Find::text(name), within: fixtures::WITHIN }
}
fn press(name: &'static [u8]) -> Step<Address> {
    Step::Press { find: Find::named(Role::Button, name) }
}
fn fill(name: &'static [u8], words: &[u8]) -> Step<Address> {
    Step::Type { find: Find::named(Role::TextField, name), words: words.into() }
}
fn setup(world: &mut ClientWorld, person: ClientId) {
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::TextField, b"Organization"), within: fixtures::WITHIN },
            fill(b"Organization", b"acme"),
            fill(b"Repository", b"shop"),
            press(b"Set up a Forgejo here"),
        ],
    );
    wait_for_setup(world);
    acknowledge(world);
}
fn wait_for_setup(world: &mut ClientWorld) {
    for _ in 0..256 {
        if !world.front().set_ups().is_empty() {
            return;
        }
        let _ = world.run(Until::Steps(2));
    }
    panic!("the form did not send SetUp");
}
fn acknowledge(world: &mut ClientWorld) {
    let (forge, request) = *world.front().set_ups().last().expect("admitted setup");
    send(world, forge::Event::SetUpAnswered { forge, request, answer: forge::SetUpAnswer::SettingUp });
    connection(world, forge::Connection::SettingUp { step: None });
}
fn text(world: &ClientWorld, person: ClientId) -> String {
    world
        .tree(person)
        .nodes()
        .iter()
        .flat_map(|node| node.text.iter())
        .map(|run| String::from_utf8_lossy(&run.text))
        .collect::<Vec<_>>()
        .join("\n")
}
fn made(world: &mut ClientWorld) {
    let (forge, request) = *world.front().set_ups().last().expect("admitted setup");
    send(world, forge::Event::SetUpAnswered { forge, request, answer: forge::SetUpAnswer::SettingUp });
    send(world, forge::Event::FirstMade { forge, id: u64::from(REPOSITORY) });
}
fn connected(world: &mut ClientWorld) {
    connection(world, forge::Connection::Connected { id: 1, login: b"temper".as_slice().into() });
}
fn ctl(world: &mut ClientWorld, agent: ClientId, words: &[&[u8]]) -> temper_client_tools::jig::Printed {
    let number = fixtures::give(world, agent, words);
    let outcome = world.run(Until::Steps(256));
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "all competing requests settled: {outcome:?}");
    fixtures::answered(world, agent, number)
}

#[test]
fn empty_organization_is_required_without_starting_the_box() {
    // client/forge.md, 5.4: an empty organization is refused locally, fields kept.
    let mut world = configured(true);
    let person = world.client(fixtures::PARTY, flow());
    script(
        &mut world,
        person,
        vec![
            Step::See { find: Find::named(Role::TextField, b"Organization"), within: fixtures::WITHIN },
            fill(b"Organization", b""),
            press(b"Set up a Forgejo here"),
            see(b"Required."),
        ],
    );
    assert!(world.front().set_ups().is_empty(), "an empty organization must not start the box");
    assert!(
        world.tree(person).nodes().iter().any(|node| node.role == Role::TextField
            && node.name.as_deref() == Some(b"Into a new project, named".as_slice())),
        "setup form must expose its new project name"
    );
}

#[test]
fn empty_repository_is_required_without_starting_the_box() {
    // client/forge.md, 5.4: an empty repository is refused locally.
    let mut world = configured(true);
    let person = world.client(fixtures::PARTY, flow());
    script(
        &mut world,
        person,
        vec![
            Step::See { find: Find::named(Role::TextField, b"Repository"), within: fixtures::WITHIN },
            fill(b"Repository", b""),
            press(b"Set up a Forgejo here"),
            see(b"Required."),
        ],
    );
    assert!(world.front().set_ups().is_empty(), "an empty repository must not start the box");
}

#[test]
fn without_the_box_the_flow_names_the_commands_and_offers_no_setup() {
    // protocol/setup.md, 6.5: without the box the page names four commands.
    let mut world = configured(false);
    let person = world.client(fixtures::PARTY, flow());
    script(&mut world, person, vec![see(b"Set it up from a terminal")]);
    let page = text(&world, person);
    for command in
        ["temper forge fetch 1", "temper forge prepare 1", "temper forge run 1 forgejo", "temper forge run 1 runner"]
    {
        assert!(page.contains(command), "without-box instructions must name {command}: {page}");
    }
}

#[test]
fn made_without_connected_must_not_adopt_early() {
    // client/tools.md, 5.5; client/forge.md, 5.4: wait for made AND connected.
    let mut world = configured(true);
    let person = world.client(fixtures::PARTY, flow());
    setup(&mut world, person);
    made(&mut world);
    connection(&mut world, forge::Connection::Connecting);
    let outcome = world.run(Until::Steps(256));
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(!world.front().engine_adopted(FORGE, REPOSITORY), "made alone must not adopt before the forge connects");
    connected(&mut world);
    script(&mut world, person, vec![see(b"acme/shop is ready")]);
    assert!(world.front().engine_adopted(FORGE, REPOSITORY), "the asking client must adopt after connection");
}

#[test]
fn a_second_tab_that_never_asked_does_not_adopt_the_made_repository() {
    // client/forge.md, 5.4: only the client that sent SetUp sends its one Adopt.
    let mut world = configured(true);
    let agent = fixtures::agent(&mut world);
    let person = world.client(fixtures::PARTY, flow());
    let number =
        fixtures::give(&mut world, agent, &[b"forge-setup", b"1", b"--org", b"acme", b"--repository", b"shop"]);
    wait_for_setup(&mut world);
    made(&mut world);
    connected(&mut world);
    let answer = fixtures::answered(&mut world, agent, number);
    assert_eq!(answer.status, 0, "{answer:?}");
    script(&mut world, person, vec![see(b"acme/shop is ready")]);
    assert_eq!(
        world.front().applications_admitted(),
        2,
        "one SetUp and ctl's one Adopt; the observer tab must not adopt"
    );
}

#[test]
fn leaving_the_flow_still_adopts_exactly_once() {
    // client/forge.md, 5.4: the asking client adopts on whatever page the person is.
    let mut world = configured(true);
    let person = world.client(fixtures::PARTY, flow());
    setup(&mut world, person);
    script(&mut world, person, vec![Step::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } }]);
    made(&mut world);
    connected(&mut world);
    let _ = world.run(Until::Steps(256));
    assert!(world.front().engine_adopted(FORGE, REPOSITORY), "leaving the flow must not lose adoption");
    let count = world.front().applications_admitted();
    send(&mut world, forge::Event::FirstMade { forge: FORGE, id: u64::from(REPOSITORY) });
    script(&mut world, person, vec![Step::Go { address: flow() }, see(b"acme/shop is ready")]);
    assert_eq!(
        world.front().applications_admitted(),
        count,
        "duplicate row updates and returning to Ready must not send another Adopt"
    );
}

#[test]
fn page_and_ctl_replayed_adoptions_share_the_first_repository_key() {
    // client/forge.md, 5.4; client/tools.md, 5.5: page and ctl share first-repository-F.
    let mut world = configured(true);
    let agent = fixtures::agent(&mut world);
    let _ = ctl(&mut world, agent, &[b"forges"]);
    let person = world.client(fixtures::PARTY, flow());
    setup(&mut world, person);
    made(&mut world);
    connected(&mut world);
    script(&mut world, person, vec![see(b"acme/shop is ready")]);
    let adopt: &[&[u8]] = &[
        b"adopt",
        b"--project",
        b"2",
        b"--forge",
        b"1",
        b"--owner",
        b"acme",
        b"--name",
        b"shop",
        b"--landing",
        b"main",
        b"--key",
        b"first-repository-1",
    ];
    // ctl can name project 2 only once the page has made it, so it replays the page's key rather
    // than racing it. A second replay from the same ctl client goes unanswered: a finding, not this
    // story's.
    let answer = ctl(&mut world, agent, adopt);
    assert_eq!(answer.status, 0, "another client must receive the shared adoption's outcome: {answer:?}");
    script(&mut world, person, vec![see(b"acme/shop is ready")]);
    assert!(world.front().engine_adopted(FORGE, REPOSITORY));
    assert_eq!(world.front().applications_admitted(), 2, "one SetUp and one Adopt across the page and ctl replays");
}

#[test]
fn setup_keeps_an_existing_projects_home_and_gives_the_new_project_its_own() {
    // client/tools.md, 5.5; client/forge.md, 5.4: home only when the project has none.
    let mut world = configured(true);
    connected(&mut world);
    let agent = fixtures::agent(&mut world);
    let answer = ctl(
        &mut world,
        agent,
        &[
            b"adopt",
            b"--project",
            b"1",
            b"--forge",
            b"1",
            b"--owner",
            b"acme",
            b"--name",
            b"second",
            b"--landing",
            b"main",
            b"--key",
            b"existing-home",
        ],
    );
    assert_eq!(answer.status, 0, "seed the project's existing home: {answer:?}");
    connection(&mut world, forge::Connection::NotSetUp);
    let person = world.client(fixtures::PARTY, flow());
    setup(&mut world, person);
    made(&mut world);
    connected(&mut world);
    script(&mut world, person, vec![see(b"acme/shop is ready")]);
    assert!(
        world.front().engine_adopted(FORGE, REPOSITORY),
        "the first repository must be adopted beside the existing home"
    );
    let rows = &world.front().engine.store.rows;
    let homes: Vec<_> = rows
        .values()
        .filter_map(|record| {
            if let engine::Record::Forge { row } = record
                && let forge::Stored::Repository(repository) = row.as_ref()
                && repository.home
            {
                return Some(repository.name.as_ref());
            }
            None
        })
        .collect();
    assert_eq!(homes, vec![b"shop".as_slice(), b"second".as_slice()], "both projects keep their home");
}

#[test]
fn every_create_failure_offers_setup_again_with_the_same_names() {
    // protocol/setup.md, 6.7; client/forge.md, 5.4: failed(step) retries the last fields.
    for (step, why) in [
        (forge::CreateStep::Admin, b"checking the admin's token".as_slice()),
        (forge::CreateStep::Org, b"making the organization".as_slice()),
        (forge::CreateStep::Repository, b"making the repository".as_slice()),
        (forge::CreateStep::File, b"adding the starter workflow".as_slice()),
    ] {
        let mut world = configured(true);
        let person = world.client(fixtures::PARTY, flow());
        setup(&mut world, person);
        send(&mut world, forge::Event::FirstFailed { forge: FORGE, step });
        connection(&mut world, forge::Connection::SetUpFailed { why: why.into() });
        script(&mut world, person, vec![see(b"Making acme/shop failed")]);
        let page = text(&world, person);
        assert!(
            page.contains(std::str::from_utf8(why).expect("failure words")),
            "flow must name the failed step: {page}"
        );
        world.person(person, vec![press(b"Set up again")]);
        for _ in 0..256 {
            if world.front().set_ups().len() == 2 {
                break;
            }
            let _ = world.run(Until::Steps(2));
        }
        assert_eq!(world.front().set_ups().len(), 2, "retry sends exactly one new setup at {step:?}");
        acknowledge(&mut world);
        script(&mut world, person, vec![see(b"acme/shop")]);
    }
}
