//! Design-only adversarial QA: client/forge.md 5.4; client/README.md
//! 3.5, 4.6, 4.8, 5.1–5.3, 5.6; client/browser.md 2.3.
use jig_client_world::harness::{ClientId, Fault, Until, World};
use skein_fake_person::{Find, Step as PersonStep};
use skein_lib::Duration;
use skein_ui::{Act, Role, State};
use skein_world::domain::Verdict;
use temper_client_domain::ChatsFilter;
use temper_client_domain::{Address, Choice, Place, Step, address};
use temper_client_view::look;
use temper_client_world::{app::Temper, fixtures as f, front::Joint};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, store::Store, walking};

type Pages = World<Temper, Joint>;
const OWNED: u16 = 1;
const GITHUB: u16 = 2;
const REMOTE: u16 = 3;
const FAKE: u16 = 4;
const UNKNOWN: u16 = 65535;
// walking::config gives each fake writer user 1.
const BOT_ID: u64 = 1;
// Enough for several 1–3 ms shell deliveries; below the 2 s request obligation.
const PENDING_WINDOW: Duration = Duration::from_millis(100);
const BOT: &[u8] = b"qa-bot";
const FAILURE: &[u8] = b"preparing: fake disk full";
// One writing request for one press, even when pressed twice before its answer.
const ONE_SETUP: usize = 1;

fn flow(choice: Option<Choice>) -> Address {
    Address::ConnectForge { forge: choice }
}
fn at(forge: u16) -> Address {
    flow(Some(Choice::Forge(forge)))
}
fn see(role: Role, name: &[u8]) -> PersonStep<Address> {
    PersonStep::See { find: Find::exactly_named(role, name), within: f::WITHIN }
}
fn gone(role: Role, name: &[u8]) -> PersonStep<Address> {
    PersonStep::Gone { find: Find::exactly_named(role, name), within: f::WITHIN }
}
fn follow(world: &mut Pages, client: ClientId, steps: Vec<PersonStep<Address>>) {
    world.person(client, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}
fn owned_details() -> forge::Owned {
    forge::Owned {
        forgejo: b"17.0.0".as_slice().into(),
        runner: Some(b"13.2.0".as_slice().into()),
        password: b"/fake/forge/person_password".as_slice().into(),
    }
}
fn configured(owner: bool, under_box: bool, mode: forge::Mode) -> Pages {
    configured_with(
        owner,
        under_box,
        &[
            forge::Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: (mode == forge::Mode::Owned).then(owned_details),
                git: Box::new([]),
                web: None,
                read_only: false,
                number: OWNED,
                kind: forge::Kind::Forgejo,
                mode,
                asked: true,
            },
            forge::Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                git: Box::new([]),
                web: None,
                read_only: false,
                number: GITHUB,
                kind: forge::Kind::GitHub,
                mode: forge::Mode::Connected,
                asked: false,
            },
        ],
    )
}
fn configured_with(owner: bool, under_box: bool, rows: &[forge::Configured; 2]) -> Pages {
    configured_accounts(owner, under_box, rows, true)
}
fn configured_accounts(owner: bool, under_box: bool, rows: &[forge::Configured; 2], account: bool) -> Pages {
    let mut world = if owner { f::accounts_world(f::SEED) } else { f::world(f::SEED) };
    let front = world.front();
    let mut store = std::mem::replace(&mut front.engine.store, Store::new());
    if !account {
        // The charter keeps its model; the restored deployment account goes.
        let accounts = store
            .rows
            .iter()
            .filter(|(_, row)| matches!(row, engine::Record::Core(jig_domain_core::Record::Accounts(_))))
            .map(|(key, _)| key.clone())
            .collect();
        store.seed(Box::new([]), accounts);
    }
    let mut limits = front.engine.env.limits;
    let configs = std::array::from_fn(|_| {
        let mut config = walking::config(f::SEED);
        config.core.connectors = Box::new([temper_client_protocol_wire::FORGE]);
        config.forge.writers = rows
            .iter()
            .map(|row| {
                let mut writer = config.forge.writers[0];
                writer.forge = row.number;
                writer.author = BOT_ID;
                writer
            })
            .collect();
        config.forges = Box::new((*rows).clone());
        config.under_box = under_box;
        if rows.iter().any(|row| row.admin_connect) {
            temper_client_world::front::first_run_projects(&mut config, &mut limits);
        }
        config
    });
    front.engine = Driver::configured(store, configs, &limits);
    front.engine.settle();
    world
}
fn world() -> Pages {
    configured(true, true, forge::Mode::Owned)
}
fn world_for(number: u16) -> Pages {
    match number {
        REMOTE => configured_with(
            true,
            true,
            &[
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: GITHUB,
                    kind: forge::Kind::GitHub,
                    mode: forge::Mode::Connected,
                    asked: false,
                },
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: REMOTE,
                    kind: forge::Kind::Forgejo,
                    mode: forge::Mode::Managed,
                    asked: false,
                },
            ],
        ),
        FAKE => configured_with(
            true,
            true,
            &[
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: OWNED,
                    kind: forge::Kind::Forgejo,
                    mode: forge::Mode::Owned,
                    asked: true,
                },
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: FAKE,
                    kind: forge::Kind::Forgejo,
                    mode: forge::Mode::Fake,
                    asked: false,
                },
            ],
        ),
        _ => world(),
    }
}
fn connection(world: &mut Pages, number: u16, state: forge::Connection) {
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::Connection { forge: number, state })));
}
fn connected() -> forge::Connection {
    forge::Connection::Connected { id: BOT_ID, login: BOT.into() }
}
// Welcome hides Live. Establish the world's Online observation before entering it.
fn open(world: &mut Pages, address: Address) -> ClientId {
    let client = world.client(f::PARTY, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(world, client, vec![see(Role::Heading { level: 1 }, b"Chats")]);
    follow(world, client, vec![PersonStep::Go { address }]);
    client
}
fn rendered(nodes: &[skein_ui::Node]) -> String {
    nodes
        .iter()
        .flat_map(|node| {
            node.name
                .iter()
                .map(AsRef::as_ref)
                .chain(node.description.iter().map(AsRef::as_ref))
                .chain(node.text.as_slice().iter().map(|run| run.text.as_ref()))
        })
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}
fn text(world: &Pages, client: ClientId) -> String {
    rendered(world.tree(client).nodes())
}
fn current_step(world: &Pages, client: ClientId, expected: &[u8]) {
    let nodes = world.tree(client).nodes();
    assert!(
        nodes.iter().enumerate().any(|(index, node)| node.role == Role::Item
            && node.states.as_slice().contains(&State::Current)
            && rendered(&nodes[index..index + usize::try_from(node.size).expect("tree size")])
                .contains(String::from_utf8_lossy(expected).as_ref())),
        "current step {}: {nodes:?}",
        String::from_utf8_lossy(expected)
    );
}
fn pending(world: &mut Pages) {
    let _ = world.run(Until::Time(world.now().saturating_add(PENDING_WINDOW)));
}
fn press_pending(world: &mut Pages, client: ClientId, button: &[u8]) {
    let button = node(world, client, Role::Button, button).id;
    world.act(client, button, Act::Press);
    pending(world);
}
fn answer_setup(world: &mut Pages, answer: forge::SetUpAnswer) {
    let (number, request) = world.front().set_ups().last().copied().expect("one setup sent");
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::SetUpAnswered {
        forge: number,
        request,
        answer,
    })));
}
fn node<'a>(world: &'a Pages, client: ClientId, role: Role, name: &[u8]) -> &'a skein_ui::Node {
    world
        .tree(client)
        .nodes()
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing {role:?} {}: {}", String::from_utf8_lossy(name), text(world, client)))
}

#[test]
fn forge_queries_canonicalize_good_bad_and_repeated_values() {
    for (query, choice, canonical) in [
        ("", None, ""),
        ("?forge=1", Some(Choice::Forge(OWNED)), "?forge=1"),
        ("?forge=65535", Some(Choice::Forge(UNKNOWN)), "?forge=65535"),
        ("?forge=elsewhere", Some(Choice::Elsewhere), "?forge=elsewhere"),
        ("?forge=1&forge=2", Some(Choice::Forge(OWNED)), "?forge=1"),
        ("?forge=elsewhere&forge=1", Some(Choice::Elsewhere), "?forge=elsewhere"),
        ("?forge=01", None, ""),
        ("?forge=65536", None, ""),
        ("?forge=-1", None, ""),
        ("?forge=garbage", None, ""),
        ("?forge=", None, ""),
        ("?forge=1=2", None, ""),
        ("?x=1&forge=3&at=ready", Some(Choice::Forge(REMOTE)), "?forge=3"),
    ] {
        let path = format!("/forges/connect{query}");
        assert_eq!(address::parse(path.as_bytes()), Place::Address(flow(choice)), "{path}");
        assert_eq!(address::print(&flow(choice)).as_ref(), format!("/forges/connect{canonical}").as_bytes());
        let fields = query.trim_start_matches('?');
        let welcome = format!("/welcome?at=forge&{fields}");
        let step = Step::Forge { forge: choice };
        assert_eq!(address::parse(welcome.as_bytes()), Place::Address(Address::Welcome { step }), "{welcome}");
    }
}

#[test]
fn malformed_first_forge_does_not_shadow_a_readable_repeat() {
    for path in ["/forges/connect?forge=garbage&forge=1", "/welcome?at=forge&forge=&forge=1"] {
        let expected = if path.starts_with("/welcome") {
            Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(OWNED)) } }
        } else {
            at(OWNED)
        };
        assert_eq!(address::parse(path.as_bytes()), Place::Address(expected), "browser.md 2.3: {path}");
    }
}

#[test]
fn choose_orders_github_owned_and_elsewhere_and_omits_fakes() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    let client = open(&mut world, flow(None));
    follow(
        &mut world,
        client,
        vec![
            see(Role::Region, b"GitHub"),
            see(Role::Region, b"Forgejo on this machine"),
            see(Role::Region, b"A Forgejo elsewhere"),
            gone(Role::Region, b"Playground forge"),
        ],
    );
    let names = world
        .tree(client)
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Region)
        .filter_map(|node| node.name.as_deref())
        .collect::<Vec<_>>();
    let github = names.iter().position(|name| *name == b"GitHub").expect("GitHub card");
    let owned = names.iter().position(|name| *name == b"Forgejo on this machine").expect("owned card");
    let remote = names.iter().position(|name| *name == b"A Forgejo elsewhere").expect("elsewhere card");
    assert!(github < owned && owned < remote, "forge.md 5.4: {names:?}");
}

#[test]
fn unknown_and_fake_choices_fall_back_to_choose() {
    for number in [UNKNOWN, FAKE] {
        let mut world = world_for(number);
        let client = open(&mut world, at(number));
        follow(
            &mut world,
            client,
            vec![see(Role::Region, b"A Forgejo elsewhere"), gone(Role::Link, "‹ All forges".as_bytes())],
        );
    }
}

#[test]
fn owned_details_offer_one_setup_and_name_its_costs() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    let client = open(&mut world, at(OWNED));
    follow(
        &mut world,
        client,
        vec![see(Role::Button, b"Set up a Forgejo here"), see(Role::Link, "‹ All forges".as_bytes())],
    );
    let words = text(&world, client);
    for expected in [
        "Its CI jobs run on this machine, as you, with no isolation.",
        "Forgejo stops when temper box stops. Its repositories are kept in temper’s data directory.",
    ] {
        assert!(words.contains(expected), "missing {expected}: {words}");
    }
}

#[test]
fn not_under_box_explains_all_four_commands_without_action() {
    let mut world = configured(true, false, forge::Mode::Owned);
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    let client = open(&mut world, at(OWNED));
    follow(
        &mut world,
        client,
        vec![
            see(Role::Heading { level: 2 }, b"Set it up from a terminal"),
            gone(Role::Button, b"Set up a Forgejo here"),
        ],
    );
    let words = text(&world, client);
    for command in
        ["temper forge fetch 1", "temper forge prepare 1", "temper forge run 1 forgejo", "temper forge run 1 runner"]
    {
        assert!(words.contains(command), "missing {command}: {words}");
    }
    follow(
        &mut world,
        client,
        vec![PersonStep::Go { address: flow(None) }, see(Role::Region, b"Forgejo on this machine")],
    );
    assert!(text(&world, client).contains("Needs four commands and a restart"));
}

#[test]
fn unoffered_forge_has_no_owned_choose_card() {
    for (mode, asked, state) in [
        (forge::Mode::Managed, false, forge::Connection::NotConnected),
        (forge::Mode::Owned, false, forge::Connection::NotSetUp),
    ] {
        let mut world = configured_with(
            true,
            true,
            &[
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: OWNED,
                    kind: forge::Kind::Forgejo,
                    mode,
                    asked,
                },
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: GITHUB,
                    kind: forge::Kind::GitHub,
                    mode: forge::Mode::Connected,
                    asked: false,
                },
            ],
        );
        connection(&mut world, OWNED, state);
        let client = open(&mut world, flow(None));
        follow(
            &mut world,
            client,
            vec![see(Role::Region, b"A Forgejo elsewhere"), gone(Role::Region, b"Forgejo on this machine")],
        );
    }
}

#[test]
fn github_details_offer_only_the_app_of_ones_own_with_a_masked_key() {
    let mut world = world();
    let client = world.client_with(
        f::PARTY,
        Address::Chats { filter: ChatsFilter::All, start: false },
        f::domain_limits(),
        browser_client,
    );
    follow(
        &mut world,
        client,
        vec![
            see(Role::Heading { level: 1 }, b"Chats"),
            PersonStep::Go { address: at(GITHUB) },
            see(Role::Button, b"Connect the App"),
        ],
    );
    let words = text(&world, client);
    assert!(!words.contains("organization"), "{words}");
    assert!(!words.contains("Connect GitHub"), "{words}");
    assert!(!words.contains("not available yet"), "{words}");
    node(&world, client, Role::Heading { level: 2 }, b"An App of your own");
    follow(
        &mut world,
        client,
        vec![gone(Role::Heading { level: 2 }, b"GitHub"), gone(Role::Heading { level: 3 }, b"An App of your own")],
    );
    node(&world, client, Role::TextField, b"App id");
    assert!(!node(&world, client, Role::Button, b"Connect the App").states.as_slice().contains(&State::Disabled));
    assert_eq!(node(&world, client, Role::TextField, b"Private key").entry, Some(skein_ui::EntryKind::Secret));
    follow(&mut world, client, vec![PersonStep::Go { address: flow(None) }, see(Role::Button, b"Connect GitHub")]);
    assert!(!node(&world, client, Role::Button, b"Connect GitHub").states.as_slice().contains(&State::Disabled));
    let words = text(&world, client);
    assert!(
        words.contains("Your repositories on GitHub. temper makes its own App there; you choose the account and the repositories it may use."),
        "{words}"
    );
}

const ADMIN_TOKEN: &[u8] = b"admin-token-placeholder-0123";
fn admin_world() -> Pages {
    // The original deployment's durable store is cold-restored with the new
    // configuration, as after changing temper.json and restarting.
    let mut world = configured_accounts(
        true,
        false,
        &[
            forge::Configured {
                owned: None,
                admin_connect: true,
                bot: Box::new([]),
                git: Box::new([]),
                web: Some(b"https://git.example.org".as_slice().into()),
                read_only: false,
                number: OWNED,
                kind: forge::Kind::Forgejo,
                mode: forge::Mode::Managed,
                asked: false,
            },
            forge::Configured {
                owned: None,
                admin_connect: false,
                bot: Box::new([]),
                git: Box::new([]),
                web: None,
                read_only: false,
                number: GITHUB,
                kind: forge::Kind::GitHub,
                mode: forge::Mode::Connected,
                asked: false,
            },
        ],
        false,
    );
    world.front().engine.forge =
        Some(temper_engine_domain_world::forge_stand::ForgeStand::with(&[(b"work", b"hello", 42)]));
    world
}
fn admin_input(world: &mut Pages, client: ClientId, field: &[u8], value: &[u8]) {
    let id = node(world, client, Role::TextField, field).id;
    world.act(client, id, Act::Input { value: value.into() });
    pending(world);
}
fn admin_private(world: &mut Pages, client: ClientId) {
    let tree = world.tree(client);
    for node in tree.nodes() {
        for value in [node.name.as_deref(), node.description.as_deref(), node.value.as_ref().map(|v| v.text.as_ref())]
            .into_iter()
            .flatten()
        {
            assert!(skein_lib::bytes::find(value, ADMIN_TOKEN).is_none());
        }
        for run in &node.text {
            assert!(skein_lib::bytes::find(&run.text, ADMIN_TOKEN).is_none());
        }
    }
    let engine = &world.front().engine;
    // Includes durable notes and all engine answers and output logs retained
    // by the fake driver; no real token, forge or process participates.
    for captured in [
        format!("{:?}", engine.store.rows),
        format!("{:?}", engine.transactions),
        format!("{:?}", engine.delivered),
        format!("{:?}", engine.immediate),
    ] {
        assert!(skein_lib::bytes::find(captured.as_bytes(), ADMIN_TOKEN).is_none());
    }
}
#[test]
fn elsewhere_welcome_after_restart_connects_retries_and_opens_its_new_project() {
    let mut world = admin_world();
    let client = open(&mut world, Address::Welcome { step: Step::Forge { forge: None } });
    follow(&mut world, client, vec![see(Role::Region, b"GitHub"), see(Role::Region, b"A Forgejo elsewhere")]);
    let names = world
        .tree(client)
        .nodes()
        .iter()
        .filter(|n| n.role == Role::Region)
        .filter_map(|n| n.name.as_deref())
        .collect::<Vec<_>>();
    assert!(
        names.iter().position(|n| *n == b"GitHub").expect("GitHub card")
            < names.iter().position(|n| *n == b"A Forgejo elsewhere").expect("add another forge card")
    );
    assert!(!names.contains(&b"Forgejo at https://git.example.org".as_slice()));
    // Enter by address: the welcome draws no admin card (client/forge.md, 5.4).
    follow(
        &mut world,
        client,
        vec![
            PersonStep::Go { address: Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(OWNED)) } } },
            see(Role::TextField, "The site admin’s token".as_bytes()),
        ],
    );
    admin_input(&mut world, client, b"Runner label", b"linux-arm");
    admin_input(&mut world, client, "The site admin’s token".as_bytes(), ADMIN_TOKEN);
    admin_private(&mut world, client);
    press_pending(&mut world, client, b"Connect as its admin");
    follow(&mut world, client, vec![see(Role::Heading { level: 2 }, b"Connecting forge 1 as its admin.")]);
    admin_private(&mut world, client);
    let (_, request) = *world.front().admin_connects.last().expect("admin Connect sent");
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::ConnectEnded {
        forge: OWNED,
        request,
        connected: false,
        why: Some(b"NotAdmin".as_slice().into()),
        stop: Some(forge::AdminStop::NotAdmin),
    })));
    follow(
        &mut world,
        client,
        vec![see(Role::TextField, "The site admin’s token".as_bytes()), see(Role::Button, b"Connect as its admin")],
    );
    assert!(text(&world, client).contains("temper kept nothing of the token."));
    assert_eq!(
        node(&world, client, Role::TextField, b"Runner label").value.as_ref().expect("field value").text.as_ref(),
        b"linux-arm"
    );
    assert_eq!(
        node(&world, client, Role::TextField, "The site admin’s token".as_bytes())
            .value
            .as_ref()
            .expect("field value")
            .text
            .as_ref(),
        b""
    );
    admin_private(&mut world, client);
    admin_input(&mut world, client, "The site admin’s token".as_bytes(), ADMIN_TOKEN);
    press_pending(&mut world, client, b"Connect as its admin");
    follow(&mut world, client, vec![see(Role::Heading { level: 2 }, b"Connecting forge 1 as its admin.")]);
    let (_, request) = *world.front().admin_connects.last().expect("admin Connect sent");
    assert_eq!(world.front().admin_connects.len(), 2);
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::FirstMade { forge: OWNED, id: 42 })));
    connection(&mut world, OWNED, connected());
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::ConnectEnded {
        forge: OWNED,
        request,
        connected: true,
        why: None,
        stop: None,
    })));
    // Adoption advances welcome to Model (client/README.md, 3.5); the durable
    // runner label belongs to this forge's Ready (client/forge.md, 5.4).
    follow(
        &mut world,
        client,
        vec![
            see(Role::Heading { level: 1 }, b"Connect a model"),
            PersonStep::Go { address: at(OWNED) },
            see(Role::Heading { level: 2 }, b"Connected as qa-bot (id 1), managed."),
            see(
                Role::Paragraph,
                "Made work/hello, and its project hello. Its CI runs on your forge’s runners labelled linux-arm; with none, a change’s checks wait.".as_bytes(),
            ),
            PersonStep::Go { address: Address::Welcome { step: Step::Ready } },
            see(Role::Heading { level: 1 }, b"work/hello is ready"),
            see(Role::Paragraph, b"Project hello, with work/hello"),
            see(Role::Link, b"Open hello"),
        ],
    );
    admin_private(&mut world, client);
    let open = node(&world, client, Role::Link, b"Open hello").id;
    world.act(client, open, Act::Press);
    follow(&mut world, client, vec![see(Role::Heading { level: 1 }, b"Chats")]);
    admin_private(&mut world, client);
    assert!(world.front().engine_adopted(OWNED, 42));
}

#[test]
fn setup_steps_show_the_reported_step_and_current_flow_step() {
    for (state, current) in [
        (forge::Connection::SettingUp { step: None }, None),
        (forge::Connection::SettingUp { step: Some(forge::SetUpStep::Fetch) }, Some(0)),
        (forge::Connection::SettingUp { step: Some(forge::SetUpStep::Prepare) }, Some(1)),
        (forge::Connection::SettingUp { step: Some(forge::SetUpStep::Run) }, Some(2)),
        (forge::Connection::Connecting, Some(3)),
    ] {
        let mut world = world();
        connection(&mut world, OWNED, state);
        let client = open(&mut world, at(OWNED));
        follow(&mut world, client, vec![see(Role::Heading { level: 2 }, b"Setting up Forgejo on this machine")]);
        let words = text(&world, client);
        let nodes = world.tree(client).nodes();
        let progress = nodes
            .iter()
            .position(|node| node.role == Role::List && node.name.as_deref() == Some(b"Each step"))
            .expect("setup progress list");
        let progress = &nodes[progress..progress + usize::try_from(nodes[progress].size).expect("tree size")];
        let items = progress.iter().enumerate().filter(|(_, node)| node.role == Role::Item).collect::<Vec<_>>();
        let labels = [
            if current > Some(0) {
                "Fetched Forgejo 17.0.0 and its runner 13.2.0"
            } else {
                "Fetching Forgejo 17.0.0 and its runner 13.2.0"
            },
            "Preparing: its database, temper’s admin, your account",
            "Starting Forgejo and its runner",
            "Connecting as temper",
            "Making the first repository, and adopting it into its project",
        ];
        assert_eq!(items.len(), labels.len(), "{words}");
        for (stage, ((index, item), label)) in items.iter().zip(labels).enumerate() {
            let subtree = &progress[*index..*index + usize::try_from(item.size).expect("tree size")];
            assert!(rendered(subtree).contains(label), "stage {stage}: {words}");
            let under_way = current == Some(stage);
            let done = current.is_some_and(|current| stage < current);
            assert_eq!(item.states.as_slice().contains(&State::Current), under_way, "stage {stage}: {words}");
            for (status, expected) in [(b"under way".as_slice(), under_way), (b"done".as_slice(), done)] {
                assert_eq!(
                    subtree.iter().any(|node| node.role == Role::Status && node.name.as_deref() == Some(status)),
                    expected,
                    "stage {stage}: {words}"
                );
            }
        }
        if current.is_none() {
            node(&world, client, Role::Status, b"under way");
        }
        current_step(&world, client, b"Setting up");
        assert!(world.front().set_ups().is_empty(), "opening a running setup must not ask again");
    }
}

#[test]
fn connecting_and_connected_rows_select_their_flow_steps() {
    for (number, state, step, level, heading) in [
        (
            OWNED,
            forge::Connection::Connecting,
            b"Setting up".as_slice(),
            2,
            b"Setting up Forgejo on this machine".as_slice(),
        ),
        (GITHUB, forge::Connection::Connecting, b"Connecting", 1, b"Connect a forge"),
        (REMOTE, forge::Connection::Connecting, b"Connecting", 1, b"Connect a forge"),
        (OWNED, connected(), b"Ready", 2, b"Forgejo on this machine is connected"),
        (GITHUB, connected(), b"Connecting", 2, "Install temper’s App".as_bytes()),
        (REMOTE, connected(), b"Ready", 2, b"Forgejo is connected"),
    ] {
        let mut world = world_for(number);
        connection(&mut world, number, state);
        let client = open(&mut world, at(number));
        follow(&mut world, client, vec![see(Role::Heading { level }, heading)]);
        current_step(&world, client, step);
    }
}

#[test]
fn forges_omit_unstarted_and_fake_rows_and_offer_add_link() {
    let mut world = world_for(FAKE);
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    connection(&mut world, FAKE, connected());
    let client = open(&mut world, Address::Forges);
    follow(
        &mut world,
        client,
        vec![
            see(Role::Link, b"+ Add a forge"),
            see(Role::Paragraph, b"No forge of yours yet."),
            gone(Role::Disclosure, b"Forgejo on this machine"),
            gone(Role::Disclosure, b"GitHub"),
            gone(Role::Disclosure, b"Playground forge"),
        ],
    );
}

#[test]
fn forges_rows_are_folded_and_keep_state_account_and_actions() {
    for (state, words, action) in [
        (forge::Connection::SettingUp { step: None }, "Setting up", "Open its set-up ›"),
        (
            forge::Connection::SettingUp { step: Some(forge::SetUpStep::Fetch) },
            "Setting up: fetching",
            "Open its set-up ›",
        ),
        (
            forge::Connection::SettingUp { step: Some(forge::SetUpStep::Prepare) },
            "Setting up: preparing",
            "Open its set-up ›",
        ),
        (
            forge::Connection::SettingUp { step: Some(forge::SetUpStep::Run) },
            "Setting up: starting",
            "Open its set-up ›",
        ),
        (
            forge::Connection::SetUpFailed { why: FAILURE.into() },
            "Set up failed: preparing: fake disk full",
            "Set up again",
        ),
        (forge::Connection::Connecting, "Connecting", "Open its set-up ›"),
        (connected(), "Connected as qa-bot", "Adopt a repository"),
        (
            forge::Connection::Stopped { reason: b"token refused".as_slice().into() },
            "Stopped: token refused",
            "Restart temper box: its preparation mends it",
        ),
    ] {
        let mut world = world();
        let prepared = matches!(
            state,
            forge::Connection::Connecting | forge::Connection::Connected { .. } | forge::Connection::Stopped { .. }
        );
        connection(&mut world, OWNED, state);
        let client = open(&mut world, Address::Forges);
        let row = node(&world, client, Role::Disclosure, b"Forgejo on this machine");
        assert!(!row.states.as_slice().contains(&State::Expanded), "rows start folded");
        let drawing = text(&world, client);
        node(&world, client, Role::Term, b"Access");
        node(&world, client, Role::Description, b"Site admin, with temper's own token");
        if prepared {
            node(&world, client, Role::Term, b"Runs");
            node(
                &world,
                client,
                Role::Description,
                b"Forgejo 17.0.0 and its runner 13.2.0, on this machine. CI jobs run here, as you.",
            );
            node(&world, client, Role::Term, b"First password");
            node(&world, client, Role::Description, b"Your first password is in /fake/forge/person_password. Forgejo asks you to change it when you sign in.");
        } else {
            assert!(!drawing.contains("First password"));
            assert!(!drawing.contains("Your first password is in"));
            assert!(!drawing.contains("Runs"));
            assert!(!drawing.contains("Forgejo 17.0.0"));
        }
        for expected in ["forge 1 · Forgejo", words, action] {
            assert!(drawing.contains(expected), "missing {expected}: {drawing}");
        }
        follow(
            &mut world,
            client,
            vec![PersonStep::Go { address: flow(None) }, gone(Role::Region, b"Forgejo on this machine")],
        );
    }
}

#[test]
fn managed_and_github_connected_rows_keep_their_names_and_accounts() {
    let mut world = world_for(REMOTE);
    connection(&mut world, REMOTE, connected());
    connection(&mut world, GITHUB, connected());
    let client = open(&mut world, Address::Forges);
    follow(&mut world, client, vec![see(Role::Disclosure, b"Forgejo"), see(Role::Disclosure, b"GitHub")]);
    let words = text(&world, client);
    assert!(words.contains("qa-bot · user 1"), "Account line: {words}");
}

#[test]
fn navigation_dot_names_and_colours_follow_real_rows_only() {
    for (number, state, label, style) in [
        (OWNED, forge::Connection::SettingUp { step: None }, b"a forge is setting up".as_slice(), look::NAV_LIMITED),
        (GITHUB, forge::Connection::Connecting, b"a forge is setting up", look::NAV_LIMITED),
        (REMOTE, forge::Connection::Connecting, b"a forge is setting up", look::NAV_LIMITED),
        (OWNED, forge::Connection::SetUpFailed { why: FAILURE.into() }, b"a forge needs you", look::NAV_NEEDS),
        (OWNED, forge::Connection::Stopped { reason: FAILURE.into() }, b"a forge needs you", look::NAV_NEEDS),
    ] {
        let mut world = world_for(number);
        connection(&mut world, number, state);
        let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
        let dot = node(&world, client, Role::Text, label);
        assert!(dot.styles.as_slice().contains(&style), "wrong colour: {dot:?}");
    }
}

#[test]
fn failure_dot_wins_over_progress_then_clears_on_connect() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::SetUpFailed { why: FAILURE.into() });
    connection(&mut world, GITHUB, forge::Connection::Connecting);
    let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(&mut world, client, vec![see(Role::Text, b"a forge needs you"), gone(Role::Text, b"a forge is setting up")]);
    connection(&mut world, OWNED, connected());
    connection(&mut world, GITHUB, connected());
    world.front().engine.send(engine::Event::Forge(Box::new(forge::Event::Installation {
        forge: GITHUB,
        installation: 1,
        state: Some(forge::Installation {
            account: Some((1, b"qa-account".as_slice().into())),
            suspended: false,
            stray: false,
        }),
    })));
    follow(
        &mut world,
        client,
        vec![gone(Role::Text, b"a forge needs you"), gone(Role::Text, b"a forge is setting up")],
    );
}

#[test]
fn fake_failure_never_sets_navigation_dot() {
    let mut world = world_for(FAKE);
    connection(&mut world, FAKE, forge::Connection::SetUpFailed { why: FAILURE.into() });
    let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(
        &mut world,
        client,
        vec![gone(Role::Text, b"a forge needs you"), gone(Role::Text, b"a forge is setting up")],
    );
}

#[test]
fn setup_press_is_single_and_disabled_until_box_answers() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    let client = open(&mut world, at(OWNED));
    let button = node(&world, client, Role::Button, b"Set up a Forgejo here").id;
    world.fault(Fault::DoublePress { client, node: button });
    pending(&mut world);
    assert_eq!(world.front().set_ups().len(), ONE_SETUP);
    assert!(node(&world, client, Role::Button, b"Set up a Forgejo here").states.as_slice().contains(&State::Disabled));
    answer_setup(&mut world, forge::SetUpAnswer::Refused(forge::SetUpRefusal::Busy));
    follow(&mut world, client, vec![see(Role::Button, b"Set up a Forgejo here")]);
}

#[test]
fn retry_press_is_single_and_disabled_until_box_answers() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::SetUpFailed { why: FAILURE.into() });
    let client = open(&mut world, Address::Forges);
    let button = node(&world, client, Role::Button, b"Set up again").id;
    world.fault(Fault::DoublePress { client, node: button });
    pending(&mut world);
    assert_eq!(world.front().set_ups().len(), ONE_SETUP);
    assert!(node(&world, client, Role::Button, b"Set up again").states.as_slice().contains(&State::Disabled));
    answer_setup(&mut world, forge::SetUpAnswer::Refused(forge::SetUpRefusal::Busy));
    follow(&mut world, client, vec![see(Role::Button, b"Set up again")]);
}

#[test]
fn every_box_refusal_is_shown_and_reenables_setup() {
    for (refusal, words) in [
        (forge::SetUpRefusal::NotOffered, "not offered"),
        (forge::SetUpRefusal::NotUnderTheBox, "not under the box"),
        (forge::SetUpRefusal::Busy, "busy"),
    ] {
        let mut world = world();
        connection(&mut world, OWNED, forge::Connection::NotSetUp);
        let client = open(&mut world, at(OWNED));
        press_pending(&mut world, client, b"Set up a Forgejo here");
        answer_setup(&mut world, forge::SetUpAnswer::Refused(refusal));
        follow(&mut world, client, vec![see(Role::Button, b"Set up a Forgejo here")]);
        let drawing = text(&world, client);
        assert!(drawing.contains(words), "{refusal:?}: expected {words}: {drawing}");
        assert!(
            !node(&world, client, Role::Button, b"Set up a Forgejo here").states.as_slice().contains(&State::Disabled)
        );
    }
}

#[test]
fn welcome_accepted_setup_moves_to_model_and_follows_its_forge() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::NotSetUp);
    let client = open(&mut world, Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(OWNED)) } });
    press_pending(&mut world, client, b"Set up a Forgejo here");
    answer_setup(&mut world, forge::SetUpAnswer::SettingUp);
    connection(&mut world, OWNED, forge::Connection::SettingUp { step: Some(forge::SetUpStep::Prepare) });
    follow(
        &mut world,
        client,
        vec![see(Role::Heading { level: 1 }, b"Connect a model"), see(Role::Disclosure, b"Each step")],
    );
    connection(&mut world, GITHUB, connected());
    follow(&mut world, client, vec![see(Role::Disclosure, b"Each step")]);
    assert!(text(&world, client).contains("Forgejo on this machine · Setting up: preparing"));
    connection(&mut world, OWNED, connected());
    follow(
        &mut world,
        client,
        vec![
            PersonStep::Go { address: Address::Welcome { step: Step::Ready } },
            see(Role::Heading { level: 1 }, b"Almost ready"),
            see(Role::Paragraph, b"Project hello, with work/hello"),
            see(Role::Disclosure, b"Each step"),
            gone(Role::Heading { level: 2 }, b"Setting up Forgejo on this machine"),
            gone(Role::Link, b"Later"),
        ],
    );
    node(&world, client, Role::Paragraph, "Forge: Forgejo on this machine · Connected as qa-bot (1)".as_bytes());
    assert!(!node(&world, client, Role::Disclosure, b"Each step").states.as_slice().contains(&State::Expanded));
    assert!(node(&world, client, Role::Button, b"Open hello").states.as_slice().contains(&State::Disabled));
    assert_eq!(world.front().set_ups().len(), ONE_SETUP);
}

#[test]
fn running_setup_opened_in_second_tab_is_not_resent() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::SettingUp { step: Some(forge::SetUpStep::Run) });
    let _first = open(&mut world, at(OWNED));
    let second = world.client(f::PARTY, at(OWNED));
    follow(&mut world, second, vec![see(Role::Heading { level: 2 }, b"Setting up Forgejo on this machine")]);
    assert!(world.front().set_ups().is_empty());
}

#[test]
fn setup_strip_tracks_progress_and_changes_its_continue_step() {
    let mut world = world();
    let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(&mut world, client, vec![see(Role::Region, b"Setup")]);
    follow(
        &mut world,
        client,
        vec![
            PersonStep::Press { find: Find::exactly_named(Role::Link, b"Continue setting up") },
            see(Role::Heading { level: 1 }, b"Connect a forge"),
            PersonStep::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        ],
    );
    connection(&mut world, OWNED, forge::Connection::SettingUp { step: Some(forge::SetUpStep::Fetch) });
    follow(&mut world, client, vec![see(Role::Link, b"View")]);
    assert!(text(&world, client).contains("Forgejo on this machine · Setting up: fetching"));
    follow(
        &mut world,
        client,
        vec![
            PersonStep::Press { find: Find::exactly_named(Role::Link, b"Continue setting up") },
            see(Role::Heading { level: 1 }, b"Almost ready"),
        ],
    );
}

#[test]
fn strip_exclusions_and_forges_navigation_current_match_pages() {
    for page in [
        Address::Forges,
        flow(None),
        Address::Accounts,
        Address::Welcome { step: Step::Forge { forge: None } },
        Address::Welcome { step: Step::Model { way: None } },
        Address::Welcome { step: Step::Ready },
    ] {
        let mut world = world();
        let client = open(&mut world, page);
        follow(&mut world, client, vec![gone(Role::Region, b"Setup"), gone(Role::Region, b"Playground")]);
    }
    for page in [Address::Forges, flow(None)] {
        let mut world = world();
        let client = open(&mut world, page);
        assert!(node(&world, client, Role::Link, b"Forges").states.as_slice().contains(&State::Current));
    }
}

#[test]
#[ignore = "needs: a non-owner identity; `configured(false, ..)` still signs in the deployment's owner, so the forges watch is served"]
fn non_owner_forges_page_explains_the_watch_refusal() {
    let mut world = configured(false, true, forge::Mode::Owned);
    let client = open(&mut world, Address::Forges);
    follow(&mut world, client, vec![see(Role::Link, b"Forges"), gone(Role::Button, b"Set up a Forgejo here")]);
    assert!(text(&world, client).contains("role"), "forges refusal: {}", text(&world, client));
}

#[test]
#[ignore = "needs: a non-owner identity; `configured(false, ..)` still signs in the deployment's owner"]
fn non_owner_setup_strip_only_names_the_missing_model() {
    let mut world = configured(false, true, forge::Mode::Owned);
    let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(&mut world, client, vec![see(Role::Region, b"Setup")]);
    let words = text(&world, client);
    assert!(words.contains("temper has no model yet."), "{words}");
    assert!(!words.contains("temper has no forge"), "forges are not this person's to set up: {words}");
}

#[test]
#[ignore = "needs: Joint per-subject forges watch ledger, hold/release and refused/ended injection; verify one session watch, no retry after terminal and headings before delivery"]
fn session_watch_survives_pages_and_stays_terminal_without_retry() {
    let mut world = world();
    let client = open(&mut world, Address::Forges);
    follow(
        &mut world,
        client,
        vec![
            PersonStep::Go { address: at(OWNED) },
            PersonStep::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        ],
    );
    // Hold first delivery: Forges must draw headings only. Refuse/end instead:
    // no rows or stale dot/banner; navigating must not open another watch.
}

#[test]
#[ignore = "needs: Joint non-null Demo read fixture and demo lifecycle controls; fake playground row plus independently usable real account"]
fn playground_forge_disappears_when_real_forge_connects() {
    let mut world = world_for(FAKE);
    connection(&mut world, FAKE, connected());
    let client = open(&mut world, Address::Forges);
    follow(&mut world, client, vec![see(Role::Disclosure, b"Playground forge")]);
    connection(&mut world, OWNED, connected());
    follow(&mut world, client, vec![gone(Role::Disclosure, b"Playground forge")]);
}

#[test]
#[ignore = "needs: non-null Demo read fixture to exercise both halves, banner order, Connect a forge href, replacement by progress, exclusions and demo-to-setup/accounts seam"]
fn playground_banner_replaces_connect_link_with_running_setup() {
    let mut world = world();
    let client = open(&mut world, Address::Chats { filter: ChatsFilter::All, start: false });
    follow(&mut world, client, vec![see(Role::Region, b"Playground"), see(Role::Link, b"Connect a forge")]);
    connection(&mut world, OWNED, forge::Connection::SettingUp { step: None });
    follow(&mut world, client, vec![see(Role::Link, b"View"), gone(Role::Link, b"Connect a forge")]);
}

#[test]
#[ignore = "needs: forges row wire fields carrying unreadable/mismatched record reasons; today's Connection::NotConnected has no reason"]
fn unreadable_not_connected_row_is_listed_and_sets_red_dot() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::NotConnected);
    let client = open(&mut world, Address::Forges);
    follow(
        &mut world,
        client,
        vec![see(Role::Disclosure, b"Forgejo on this machine"), see(Role::Text, b"a forge needs you")],
    );
}

#[test]
#[ignore = "needs: Joint browser reload retaining the address; its existing reload explicitly resets to Inbox as a terminal restart"]
fn browser_reload_preserves_running_setup_without_resending() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::SettingUp { step: None });
    let client = open(&mut world, at(OWNED));
    world.reload(client);
    follow(&mut world, client, vec![see(Role::Heading { level: 2 }, b"Setting up Forgejo on this machine")]);
    assert!(world.front().set_ups().is_empty());
}

#[test]
fn accounts_banner_stays_above_setup_strip_on_eligible_pages() {
    for (page, has_strip) in [
        (Address::Chats { filter: ChatsFilter::All, start: false }, true),
        (f::inbox(), true),
        (Address::Adopt { project: 1 }, true),
        (Address::Forges, false),
        (flow(None), false),
    ] {
        // No usable account: the accounts document is not usable.
        let mut world = configured_accounts(
            true,
            true,
            &[
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: OWNED,
                    kind: forge::Kind::Forgejo,
                    mode: forge::Mode::Owned,
                    asked: true,
                },
                forge::Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number: GITHUB,
                    kind: forge::Kind::GitHub,
                    mode: forge::Mode::Connected,
                    asked: false,
                },
            ],
            false,
        );
        let client = open(&mut world, page);
        let nodes = world.tree(client).nodes();
        let banner = nodes.iter().position(|node| {
            node.text.as_slice().iter().any(|run| run.text.as_ref() == b"Agents need an LLM account to run.")
        });
        assert!(banner.is_some(), "README.md 5.1 requires the accounts banner: {}", text(&world, client));
        let strip = nodes.iter().position(|node| node.role == Role::Region && node.name.as_deref() == Some(b"Setup"));
        assert_eq!(strip.is_some(), has_strip, "only eligible pages have a setup strip");
        if let Some(strip) = strip {
            assert!(banner.expect("banner exists") < strip, "accounts banner precedes setup strip");
        }
    }
}

#[test]
fn failed_setup_flow_keeps_reason_and_offers_one_retry() {
    let mut world = world();
    connection(&mut world, OWNED, forge::Connection::SetUpFailed { why: FAILURE.into() });
    let client = open(&mut world, at(OWNED));
    current_step(&world, client, b"Setting up");
    assert!(text(&world, client).contains("Set up failed: preparing: fake disk full"));
    press_pending(&mut world, client, b"Set up again");
    assert_eq!(world.front().set_ups().len(), ONE_SETUP);
    assert!(node(&world, client, Role::Button, b"Set up again").states.as_slice().contains(&State::Disabled));
    answer_setup(&mut world, forge::SetUpAnswer::Refused(forge::SetUpRefusal::Busy));
    follow(&mut world, client, vec![see(Role::Button, b"Set up again")]);
}

fn browser_client(seed: u64) -> (temper_client_world::app::Root, temper_client_world::app::View) {
    use temper_client_world::app::{Root, View};
    let limits = f::domain_limits();
    let words = temper_client_view::words::Words::english(
        f::FORGE,
        limits.client.domain.text,
        temper_client_view::words::Shell::Browser,
    )
    .expect("browser words fit the client bounds");
    let bounds = f::view_bounds(&limits, &words);
    (
        Root::Person(Box::new(temper_client_domain::unlinked(&limits, seed))),
        View::Person(Box::new(temper_client_view::View::new(&bounds, &limits, words, f::FORGE))),
    )
}
