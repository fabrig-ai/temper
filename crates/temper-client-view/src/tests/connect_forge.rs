#![allow(clippy::disallowed_types, clippy::disallowed_methods, reason = "test assertions inspect the semantic tree")]
use super::fixtures as f;
use skein_lib::List;
use skein_ui::{Act, Role, State};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{self as d, Address, Choice, Step, pages};
use temper_client_protocol_wire::v1 as w;
const HERE: u16 = 3;
const GITHUB: u16 = 2;
pub(super) fn row(
    number: u16,
    kind: w::Kind,
    mode: w::Mode,
    connection: w::Connection,
    offer: w::Offer,
) -> w::ForgeRow {
    w::ForgeRow::new(
        &w::CEILINGS,
        w::ForgeRowParts {
            number,
            kind,
            mode,
            connection,
            offer,
            first: None,
            installations: List::with_capacity(0),
            ended: None,
            app: None,
            web: None,
            repositories: List::with_capacity(0),
            owned: None,
            admin: false,
            read_only: false,
            ended_stop: None,
        },
    )
    .unwrap()
}
pub(super) fn rows(connection: w::Connection, offer: w::Offer) -> List<w::ForgeRow> {
    let mut rows = List::with_capacity(2);
    rows.push(row(GITHUB, w::Kind::Github, w::Mode::Managed, w::Connection::NotConnected, w::Offer::NotOffered))
        .unwrap();
    rows.push(row(HERE, w::Kind::Forgejo, w::Mode::Owned, connection, offer)).unwrap();
    rows
}
fn root(choice: Option<Choice>, connection: w::Connection, offer: w::Offer) -> d::Domain {
    f::forge_root(Address::ConnectForge { forge: choice }, rows(connection, offer))
}
fn here(connection: w::Connection) -> d::Domain {
    root(Some(Choice::Forge(HERE)), connection, w::Offer::Offered)
}
pub(super) fn connected() -> w::Connection {
    w::Connection::Connected(
        w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 42, login: b"temper".as_slice().into() }).unwrap(),
    )
}
fn node(tree: &skein_ui::Tree, role: Role, name: &[u8]) -> skein_ui::NodeId {
    f::find(tree, role, name).unwrap_or_else(|| panic!("missing {role:?}: {}", String::from_utf8_lossy(name)))
}
fn press(root: &d::Domain, role: Role, name: &[u8]) -> d::Action {
    let mut view = f::page_view(root);
    let id = node(view.tree(), role, name);
    crate::act(&mut view, id, if role == Role::Choice { Act::Toggle } else { Act::Press }).expect("bound control")
}
pub(super) fn words_in_order(tree: &skein_ui::Tree, phrases: &[&str]) {
    let mut bytes = Vec::new();
    for (index, node) in tree.nodes().iter().enumerate() {
        if let Some(name) = &node.name
            && f::node_text(tree, index).is_empty()
        {
            bytes.extend_from_slice(name);
            bytes.push(b'\n');
        }
        for run in &node.text {
            bytes.extend_from_slice(&run.text);
        }
    }
    let words = String::from_utf8_lossy(&bytes);
    let mut remaining = words.as_ref();
    for phrase in phrases {
        let index = remaining.find(phrase).unwrap_or_else(|| panic!("missing or out of order: {phrase}\nin {words}"));
        remaining = &remaining[index.checked_add(phrase.len()).expect("phrase end fits")..];
    }
}
#[test]
fn choose_draws_a_card_per_choice_in_order() {
    let root = root(None, w::Connection::NotSetUp, w::Offer::Offered);
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, b"Connect a forge");
    words_in_order(
        view.tree(),
        &[
            "Choose",
            "Details",
            "Setting up",
            "Ready",
            "GitHub",
            "Your repositories on GitHub. temper makes its own App there; you choose the account and the repositories it may use.",
            "In this tab, on GitHub · about two minutes",
            "Connect GitHub",
            "Use an App of your own ›",
            "Forgejo on this machine",
            "A forge of your own, run by temper box. No account needed: temper makes your first repository, and you start in it.",
            "Here · about a minute",
            "Choose",
            "A Forgejo elsewhere",
            "Your own Forgejo server. temper connects as its admin, or with its bot’s tokens.",
            "Needs a change to temper’s configuration and a restart",
            "Show how",
        ],
    );
    let choices =
        view.tree().nodes().iter().find(|n| n.styles.as_slice().contains(&crate::look::FLOW_CHOICES)).unwrap();
    let cards: Vec<_> = view.tree().nodes().iter().filter(|n| parent(view.tree(), n.id) == Some(choices.id)).collect();
    assert_eq!(cards.len(), 3);
    let primary: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY))
        .map(|n| n.name.as_deref().unwrap())
        .collect();
    assert_eq!(primary, [b"Connect GitHub".as_slice()]);
    for (name, style) in [
        (b"Choose".as_slice(), crate::look::FLOW_SECONDARY),
        (b"Show how".as_slice(), crate::look::FLOW_SECONDARY),
        ("Use an App of your own ›".as_bytes(), crate::look::FLOW_QUIET),
    ] {
        let id = node(view.tree(), Role::Link, name);
        assert!(view.tree().nodes().iter().find(|n| n.id == id).unwrap().styles.as_slice().contains(&style));
    }
    assert!(cards.iter().all(|n| n.role == Role::Region));
    let regions: Vec<_> =
        view.tree().nodes().iter().filter(|n| n.role == Role::Region).filter_map(|n| n.name.as_deref()).collect();
    assert_eq!(regions, [b"LLM account".as_slice(), b"GitHub", b"Forgejo on this machine", b"A Forgejo elsewhere"]);
    assert_eq!(
        press(&root, Role::Link, b"Show how"),
        d::Action::Pages(pages::Act::Go { address: Address::ConnectForge { forge: Some(Choice::Elsewhere) } })
    );
}
#[test]
fn github_details_offer_only_the_app_of_ones_own_with_a_masked_key() {
    let root = root(Some(Choice::Forge(GITHUB)), w::Connection::NotSetUp, w::Offer::Offered);
    let view = f::browser_page_view(&root);
    let words = String::from_utf8_lossy(&f::text(view.tree())).into_owned();
    assert!(!words.contains("organization"));
    assert!(!words.contains("Connect GitHub"));
    assert!(!words.contains("not available yet"));
    node(view.tree(), Role::Heading { level: 2 }, b"An App of your own");
    let button = node(view.tree(), Role::Button, b"Connect the App");
    assert!(!view.tree().nodes().iter().find(|n| n.id == button).unwrap().states.as_slice().contains(&State::Disabled));
    node(view.tree(), Role::TextField, b"App id");
    let key = node(view.tree(), Role::TextField, b"Private key");
    assert_eq!(view.tree().nodes().iter().find(|n| n.id == key).unwrap().entry, Some(skein_ui::EntryKind::Secret));
}
#[test]
fn the_terminal_shows_ctl_or_the_web_page_instead_of_the_key_field() {
    let view = f::page_view(&root(Some(Choice::Forge(GITHUB)), w::Connection::NotSetUp, w::Offer::Offered));
    assert!(f::find(view.tree(), Role::TextField, b"App id").is_none());
    assert!(f::find(view.tree(), Role::TextField, b"Private key").is_none());
    assert!(f::find(view.tree(), Role::Button, b"Connect the App").is_none());
    words_in_order(
        view.tree(),
        &[
            "Paste the key on this page in a browser, or run temper ctl DIR forge connect 2 github-app ID FILE, FILE the key's file, readable by you alone.",
        ],
    );
}

#[test]
fn forgejo_here_details_say_the_first_repository_and_set_it_up() {
    let mut root = here(w::Connection::NotSetUp);
    let view = f::page_view(&root);
    words_in_order(
        view.tree(),
        &[
            "Forgejo on this machine",
            "temper box downloads Forgejo and its runner, starts them, and makes your first repository, with a README and a CI workflow. No restart.",
            "Its CI jobs run on this machine, as you, with no isolation.",
            "Forgejo stops when temper box stops. Its repositories are kept in temper’s data directory.",
        ],
    );
    assert!(view.tree().nodes().iter().any(|n| n.role == Role::TextField));
    let action = press(&root, Role::Button, b"Set up a Forgejo here");
    assert_eq!(action, d::Action::Forge(d::forge::Act::SetUp { forge: HERE }));
    f::root_event(&mut root, d::Event::Act { action });
    let view = f::page_view(&root);
    let id = node(view.tree(), Role::Button, b"Set up a Forgejo here");
    assert!(view.tree().nodes().iter().find(|n| n.id == id).unwrap().states.as_slice().contains(&State::Disabled));
}
#[test]
fn forgejo_here_without_the_box_lists_its_four_commands() {
    let view = f::page_view(&root(Some(Choice::Forge(HERE)), w::Connection::NotSetUp, w::Offer::NotUnderTheBox));
    node(view.tree(), Role::Heading { level: 2 }, b"Set it up from a terminal");
    words_in_order(
        view.tree(),
        &[
            "temper was not started by temper box, so it does not start a Forgejo itself. Run these, keep the last two running (a service manager is best), then restart temper:",
            "temper forge fetch 3",
            "temper forge prepare 3",
            "temper forge run 3 forgejo",
            "temper forge run 3 runner",
            "Once it is connected, make your first repository here.",
        ],
    );
    assert!(f::find(view.tree(), Role::Button, b"Set up a Forgejo here").is_none());
    assert_eq!(view.tree().nodes().iter().filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_CODE)).count(), 1);
    let index = view.tree().nodes().iter().position(|n| n.styles.as_slice().contains(&crate::look::FLOW_CODE)).unwrap();
    assert_eq!(
        f::node_text(view.tree(), index).as_ref(),
        b"temper forge fetch 3\ntemper forge prepare 3\ntemper forge run 3 forgejo\ntemper forge run 3 runner"
    );
}
#[test]
fn elsewhere_details_give_the_next_unused_number() {
    let view = f::page_view(&root(Some(Choice::Elsewhere), w::Connection::NotSetUp, w::Offer::Offered));
    words_in_order(
        view.tree(),
        &[
            "A Forgejo elsewhere",
            "Add its address to the configuration and restart temper box. Then connect as its admin, and temper makes and keeps its bot, or paste its bot’s tokens.",
            "application.forges",
            "\"number\": 4",
            "\"kind\": \"forgejo\"",
            "\"host\": \"forge.example\"",
            "\"log_origins\": null",
            "application.identities",
            "\"number\": 4, \"forge\": 4",
            "application.setup",
            "\"forge\": 4",
            "\"mode\": \"managed\"",
            "Without it, temper takes its bot’s tokens only.",
            "Restart temper box, then connect it from this page.",
        ],
    );
}
#[test]
fn setting_up_marks_the_rows_step() {
    for (step, done) in
        [(None, 0), (Some(w::SetUpStep::Fetch), 0), (Some(w::SetUpStep::Prepare), 1), (Some(w::SetUpStep::Run), 2)]
    {
        let view = f::page_view(&here(w::Connection::SettingUp(
            w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step }).unwrap(),
        )));
        node(view.tree(), Role::Heading { level: 2 }, b"Setting up Forgejo on this machine");
        assert_eq!(
            view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
            done
        );
        node(view.tree(), Role::Status, b"under way");
        words_in_order(
            view.tree(),
            &[
                if done == 0 { "Fetching Forgejo and its runner" } else { "Fetched Forgejo and its runner" },
                "Preparing: its database, temper’s admin, your account",
                "Starting Forgejo and its runner",
                "Connecting as temper",
                "Making the first repository, and adopting it into its project",
                "You may leave this page: the steps go on, and the banner says where they are.",
            ],
        );
    }
    let view = f::page_view(&here(w::Connection::Connecting));
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        3
    );
}
#[test]
fn set_up_failed_offers_set_up_again() {
    let root = here(w::Connection::SetUpFailed(
        w::SetUpFailed::new(&w::CEILINGS, w::SetUpFailedParts { why: b"fetch failed".as_slice().into(), at: None })
            .unwrap(),
    ));
    let view = f::page_view(&root);
    let reason = node(view.tree(), Role::Alert, b"Set up failed: fetch failed");
    assert!(
        view.tree()
            .nodes()
            .iter()
            .find(|n| n.id == reason)
            .unwrap()
            .styles
            .as_slice()
            .contains(&crate::look::FLOW_FAILED)
    );
    let retry = node(view.tree(), Role::Button, b"Set up again");
    assert!(
        view.tree()
            .nodes()
            .iter()
            .find(|n| n.id == retry)
            .unwrap()
            .styles
            .as_slice()
            .contains(&crate::look::FLOW_SECONDARY)
    );
    assert!(f::find(view.tree(), Role::List, b"Each step").is_some());
    assert_eq!(press(&root, Role::Button, b"Set up again"), d::Action::Forge(d::forge::Act::SetUp { forge: HERE }));
}
#[test]
fn ready_names_the_forge_and_offers_adopt_a_repository() {
    let root = here(connected());
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 2 }, b"Forgejo on this machine is connected");
    node(view.tree(), Role::Paragraph, b"Connected as temper.");
    words_in_order(view.tree(), &["Forgejo on this machine is on the Forges page, with its details."]);
    assert_eq!(
        press(&root, Role::Link, b"Adopt a repository"),
        d::Action::Pages(pages::Act::Go { address: Address::Adopt { project: f::PROJECT } })
    );
    assert_eq!(press(&root, Role::Link, b"Forges"), d::Action::Pages(pages::Act::Go { address: Address::Forges }));
}
#[test]
fn the_welcomes_forge_step_is_choose_and_details() {
    let mut root = f::forge_root(
        Address::Welcome { step: Step::Forge { forge: None } },
        rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    assert_eq!(
        press(&root, Role::Link, b"Show how"),
        d::Action::Pages(pages::Act::Go {
            address: Address::Welcome { step: Step::Forge { forge: Some(Choice::Elsewhere) } }
        })
    );
    f::root_event(
        &mut root,
        d::Event::Went {
            place: d::Place::Address(Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(HERE)) } }),
        },
    );
    let view = f::page_view(&root);
    node(view.tree(), Role::Button, b"Set up a Forgejo here");
    assert_eq!(
        press(&root, Role::Link, "‹ Back".as_bytes()),
        d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Forge { forge: None } } })
    );
}
#[test]
fn a_forge_under_way_is_no_choice_in_the_welcome() {
    let root = f::forge_root(
        Address::Welcome { step: Step::Forge { forge: None } },
        rows(w::Connection::Connecting, w::Offer::Offered),
    );
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Forgejo on this machine · Connecting".as_bytes());
    assert!(f::find(view.tree(), Role::Link, b"Choose").is_none());
    assert_eq!(
        press(&root, Role::Link, b"View"),
        d::Action::Pages(pages::Act::Go { address: Address::ConnectForge { forge: Some(Choice::Forge(HERE)) } })
    );
}
#[test]
fn the_model_step_follows_the_persons_forge() {
    let mut forges = rows(w::Connection::Connecting, w::Offer::Offered);
    // Model follows the person's forge, even with another forge still connecting.
    *forges.get_mut(0).unwrap() = github_rows(true).get(0).unwrap().clone();
    let root = f::forge_root(Address::Welcome { step: Step::Model { way: None } }, forges);
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "GitHub · no repository chosen yet".as_bytes());
    node(view.tree(), Role::Disclosure, b"Each step");
    node(view.tree(), Role::Heading { level: 1 }, b"Connect a model");
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        1
    );
}

#[test]
fn the_welcomes_ready_names_the_forge_and_the_model() {
    let (mut root, accounts) =
        f::forge_root_accounts(Address::Welcome { step: Step::Ready }, rows(connected(), w::Offer::Offered));
    f::welcome_account(&mut root, accounts);
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, b"Almost ready");
    node(view.tree(), Role::Paragraph, b"Forge: Forgejo on this machine, connected as temper");
    let steps = view.tree().nodes().iter().filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_STEP));
    for step in steps {
        assert!(step.styles.as_slice().contains(&crate::look::FLOW_DONE));
        assert!(!step.states.as_slice().contains(&State::Current));
    }
    let account_view = f::page_view(&{
        f::root_event(&mut root, d::Event::Went { place: d::Place::Address(Address::Accounts) });
        root
    });
    let heading = account_view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::Heading { level: 3 })
        .unwrap()
        .name
        .as_deref()
        .unwrap();
    let model = [b"Model: ".as_slice(), heading].concat();
    node(view.tree(), Role::Paragraph, &model);
}
#[test]
fn ready_without_a_forge_or_model_offers_the_two_steps() {
    let root = f::welcome_root(Step::Ready, false);
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, b"Forge: none yet");
    node(view.tree(), Role::Slot, b"Model: none yet");
    node(view.tree(), Role::Link, b"Connect a forge");
    node(view.tree(), Role::Link, b"Connect a model");
}

#[test]
fn the_welcomes_ready_follows_a_forge_under_way() {
    let root =
        f::forge_root(Address::Welcome { step: Step::Ready }, rows(w::Connection::Connecting, w::Offer::Offered));
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Forge: Forgejo on this machine · Connecting".as_bytes());
    node(view.tree(), Role::Disclosure, b"Each step");
    assert!(f::find(view.tree(), Role::Link, b"Adopt a repository").is_none());
}

#[test]
fn the_forge_pages_have_navigation_and_no_playground_banner() {
    for address in [Address::ConnectForge { forge: None }, Address::Forges] {
        let root = f::demo_frame_root(address, f::PROJECT, true, None, None);
        assert!(root.demo().is_some());
        let view = f::page_view(&root);
        node(view.tree(), Role::Navigation, b"Main navigation");
        assert!(f::find(view.tree(), Role::Region, b"Playground").is_none());
    }
}

#[test]
fn a_set_up_refusal_is_said_below_its_button() {
    use jig_client::{Answered, Down, Family};
    use skein_lib::{Env, Queue, RequestEnvelope, Time};
    use temper_client_protocol_wire as wire;
    let mut root = here(w::Connection::NotSetUp);
    let limits = f::domain_limits();
    let mut out = Queue::with_capacity(d::max_out(&limits));
    d::step(
        &mut root,
        &Env { now: Time::ZERO, wall: f::NOW, limits },
        d::Event::Act { action: d::Action::Forge(d::forge::Act::SetUp { forge: HERE }) },
        &mut out,
    );
    let mut sent = None;
    while let Some(request) = out.pop() {
        if let d::Request::Client { request: jig_client::Request::Down { down: Down::Send { attempt, .. } } } = request
        {
            sent = Some(attempt);
        }
    }
    let wire_limits = d::forge::wire_limits(&limits.forge).unwrap();
    let outcome = w::Outcome::SetUpRefused(
        w::SetUpRefused::new(&wire_limits, w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap(),
    );
    f::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: sent.expect("set up sent"),
                    envelope: RequestEnvelope::Final,
                    answer: Some(Answered::Family(Family {
                        family: wire::FORGE,
                        version: wire::CLIENT_VERSION,
                        bytes: wire::answer(&w::Answer::new(&wire_limits, w::AnswerParts { outcome }).unwrap()),
                    })),
                    why: None,
                },
            },
        },
    );
    let view = f::page_view(&root);
    let refusal = d::forge::words::set_up_refusal(d::forge::forges::SetUpRefusal::Busy);
    node(view.tree(), Role::Alert, refusal);
    let button = node(view.tree(), Role::Button, b"Set up a Forgejo here");
    assert!(!view.tree().nodes().iter().find(|n| n.id == button).unwrap().states.as_slice().contains(&State::Disabled));
}

fn drive(root: &mut d::Domain, event: d::Event) -> skein_lib::Queue<d::Request> {
    let limits = f::domain_limits();
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    d::step(root, &skein_lib::Env { now: skein_lib::Time::ZERO, wall: f::NOW, limits }, event, &mut out);
    out
}
fn act(root: &mut d::Domain, act: d::forge::Act) -> skein_lib::Queue<d::Request> {
    drive(root, d::Event::Act { action: d::Action::Forge(act) })
}
fn attempt(mut out: skein_lib::Queue<d::Request>) -> skein_lib::Token {
    while let Some(request) = out.pop() {
        if let d::Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } },
        } = request
        {
            return attempt;
        }
    }
    panic!("request sent")
}
fn answer(root: &mut d::Domain, attempt: skein_lib::Token, outcome: w::Outcome) {
    f::root_event(root, answer_event(attempt, outcome));
}
fn answer_event(attempt: skein_lib::Token, outcome: w::Outcome) -> d::Event {
    d::Event::Client {
        event: jig_client::Event::Up {
            up: jig_client::Up::Answer {
                attempt,
                envelope: skein_lib::RequestEnvelope::Final,
                why: None,
                answer: Some(jig_client::Answered::Family(jig_client::Family {
                    family: temper_client_protocol_wire::FORGE,
                    version: temper_client_protocol_wire::CLIENT_VERSION,
                    bytes: temper_client_protocol_wire::answer(
                        &w::Answer::new(&w::CEILINGS, w::AnswerParts { outcome }).unwrap(),
                    ),
                })),
            },
        },
    }
}
pub(super) fn first_rows(connection: w::Connection, state: w::FirstState) -> List<w::ForgeRow> {
    let first = w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::Existing(
                w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: f::PROJECT }).unwrap(),
            ),
            state,
            label: Box::from(b"temper-host".as_slice()),
        },
    )
    .unwrap();
    let mut rows = List::with_capacity(1);
    rows.push(
        w::ForgeRow::new(
            &w::CEILINGS,
            w::ForgeRowParts {
                number: HERE,
                kind: w::Kind::Forgejo,
                mode: w::Mode::Owned,
                connection,
                offer: w::Offer::Offered,
                first: Some(first),
                installations: List::with_capacity(0),
                ended: None,
                app: None,
                web: None,
                repositories: List::with_capacity(0),
                owned: None,
                admin: false,
                read_only: false,
                ended_stop: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    rows
}
pub(super) fn deliver(
    root: &mut d::Domain,
    watch: skein_lib::Token,
    rows: List<w::ForgeRow>,
) -> skein_lib::Queue<d::Request> {
    drive(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::FamilyDelivered {
                    watch,
                    record: jig_client::Family {
                        family: temper_client_protocol_wire::FORGE,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes: temper_client_protocol_wire::forges(
                            &w::Forges::new(&w::CEILINGS, w::ForgesParts { forges: rows }).unwrap(),
                        ),
                    },
                },
            },
        },
    )
}
pub(super) fn made() -> w::FirstState {
    w::FirstState::Made(w::Made::new(&w::CEILINGS, w::MadeParts { id: 42 }).unwrap())
}
fn has_state(tree: &skein_ui::Tree, role: Role, name: &[u8], state: State) -> bool {
    let id = node(tree, role, name);
    tree.nodes().iter().find(|n| n.id == id).unwrap().states.as_slice().contains(&state)
}
#[test]
fn details_show_the_set_ups_fields_with_their_defaults() {
    let mut root = here(w::Connection::NotSetUp);
    let view = f::page_view(&root);
    for (name, value) in [
        (b"Organization".as_slice(), b"work".as_slice()),
        (b"Repository", b"hello"),
        (b"Into a new project, named", b"hello"),
    ] {
        let id = node(view.tree(), Role::TextField, name);
        assert_eq!(
            view.tree().nodes().iter().find(|n| n.id == id).unwrap().value.as_ref().unwrap().text.as_ref(),
            value
        );
    }
    let nodes = view.tree().nodes();
    let org = nodes.iter().find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Organization")).unwrap();
    let repository =
        nodes.iter().find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Repository")).unwrap();
    let project = nodes
        .iter()
        .find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Into a new project, named"))
        .unwrap();
    let grandparent = |id| parent(view.tree(), parent(view.tree(), id).unwrap());
    assert_eq!(grandparent(org.id), grandparent(repository.id));
    let pair = grandparent(org.id).unwrap();
    assert!(nodes.iter().find(|n| n.id == pair).unwrap().styles.as_slice().contains(&crate::look::FLOW_PAIR));
    let primary: Vec<_> = nodes
        .iter()
        .filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY))
        .map(|n| n.name.as_deref().unwrap())
        .collect();
    assert_eq!(primary, [b"Set up a Forgejo here".as_slice()]);
    assert_ne!(grandparent(org.id), grandparent(project.id));
    let pair = grandparent(project.id).unwrap();
    assert!(nodes.iter().find(|n| n.id == pair).unwrap().styles.as_slice().contains(&crate::look::FLOW_PAIR));
    for field in [org, repository, project] {
        let wrapper = parent(view.tree(), field.id).unwrap();
        let wrapper = nodes.iter().find(|n| n.id == wrapper).unwrap();
        assert!(wrapper.styles.as_slice().contains(&crate::look::FLOW_FIELD));
    }
    let facts = nodes.iter().find(|n| n.styles.as_slice().contains(&crate::look::FLOW_FACTS)).unwrap();
    assert_eq!(facts.role, Role::List);
    let items: Vec<_> = nodes.iter().filter(|n| parent(view.tree(), n.id) == Some(facts.id)).collect();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|n| n.role == Role::Item));
    assert!(!view.tree().nodes().iter().any(|node| node.role == Role::ChoiceGroup));
    let mut view = f::page_view(&root);
    let id = node(view.tree(), Role::TextField, b"Into a new project, named");
    let action = crate::act(&mut view, id, Act::Input { value: Box::default() }).unwrap();
    f::root_event(&mut root, d::Event::Act { action });
    act(&mut root, d::forge::Act::SetUp { forge: HERE });
    let view = f::page_view(&root);
    assert!(has_state(view.tree(), Role::TextField, b"Into a new project, named", State::Invalid));
    node(view.tree(), Role::Alert, b"Required.");
}
#[test]
fn an_invalid_refusal_marks_the_organization_and_repository() {
    let mut root = here(w::Connection::NotSetUp);
    let token = attempt(act(&mut root, d::forge::Act::SetUp { forge: HERE }));
    answer(
        &mut root,
        token,
        w::Outcome::SetUpRefused(
            w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why: w::SetUpRefusal::Invalid }).unwrap(),
        ),
    );
    let view = f::page_view(&root);
    for name in [b"Organization".as_slice(), b"Repository"] {
        assert!(has_state(view.tree(), Role::TextField, name, State::Invalid));
    }
    node(view.tree(), Role::Alert, b"invalid: the organization, repository or project name is empty or too long");
    assert_eq!(root.forge().setup_form().text(d::forge::forges::SetupField::Repository), b"hello");
}
#[test]
fn setting_up_names_its_first_repository_and_project() {
    let root = f::forge_root(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        first_rows(w::Connection::Connecting, w::FirstState::Making),
    );
    let view = f::page_view(&root);
    words_in_order(
        view.tree(),
        &[
            "Setting up Forgejo on this machine",
            "work/hello, into Shop",
            "Making work/hello, and adopting it into Shop",
        ],
    );
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        3_usize
    );
    node(view.tree(), Role::Status, b"under way");
}
#[test]
fn setting_up_names_the_demo_project_playground() {
    let root = f::root_at_with_forges(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        &f::domain_limits(),
        b"Project 1",
        Some((f::PROJECT, true, w::DemoState::Running)),
        None,
        Some(first_rows(w::Connection::Connecting, w::FirstState::Making)),
    )
    .0;
    for words in [f::words(), f::browser_words()] {
        let mut view = f::view_of(words);
        f::build(&mut view, &root);
        words_in_order(
            view.tree(),
            &["work/hello, into Playground", "Making work/hello, and adopting it into Playground"],
        );
    }
}
#[test]
fn setting_up_uses_the_sent_fields_until_the_row_carries_them() {
    let mut root = here(w::Connection::NotSetUp);
    act(&mut root, d::forge::Act::SetUp { forge: HERE });
    // The flow keeps the submitted fields even when it is showing Details.
    assert_eq!(root.forge().set_up().unwrap().org.as_ref(), b"work");
    let (mut root, watch) = f::forge_root_watch(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    act(&mut root, d::forge::Act::SetUp { forge: HERE });
    deliver(&mut root, watch, rows(w::Connection::Connecting, w::Offer::Offered));
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, b"work/hello, into hello");
    words_in_order(view.tree(), &["Starting Forgejo and its runner", "Connecting"]);
    assert!(!f::text(view.tree()).windows(b"Connecting, and making".len()).any(|w| w == b"Connecting, and making"));
}
#[test]
fn ready_says_the_repository_is_ready_and_opens_its_project() {
    let address = Address::ConnectForge { forge: Some(Choice::Forge(HERE)) };
    let mut root = f::forge_root(address, first_rows(connected(), made()));
    f::root_event(
        &mut root,
        d::Event::Act { action: d::Action::Pages(pages::Act::Project { project: f::SECOND_PROJECT }) },
    );
    assert_eq!(root.pages().project(), Some(f::SECOND_PROJECT));
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 2 }, b"work/hello is ready");
    node(view.tree(), Role::Paragraph, b"In Shop, on Forgejo on this machine.");
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        4_usize
    );
    let action = press(&root, Role::Link, b"Open Shop");
    assert_eq!(action, d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT }));
    let mut out = drive(&mut root, d::Event::Act { action });
    assert_eq!(root.pages().project(), Some(f::PROJECT));
    assert_eq!(
        out.pop(),
        Some(d::Request::Show { address: Address::Chats { filter: ChatsFilter::All, start: false }, replace: false })
    );
}
fn pending_adoption() -> (d::Domain, skein_lib::Token) {
    let (root, token, _) = pending_adoption_with_watch();
    (root, token)
}
fn pending_adoption_with_watch() -> (d::Domain, skein_lib::Token, skein_lib::Token) {
    let (mut root, watch) = f::forge_root_watch(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    let token = attempt(act(&mut root, d::forge::Act::SetUp { forge: HERE }));
    answer(&mut root, token, w::Outcome::SettingUp);
    let token = attempt(deliver(&mut root, watch, new_first_rows()));
    let token = attempt(jig_answer(
        &mut root,
        token,
        jig_client::contract::Answer::Done(jig_client::contract::Outcome::Owner(
            jig_client::contract::OwnerOutcome::ProjectMade { project: f::PROJECT },
        )),
    ));
    (root, token, watch)
}
#[test]
fn open_project_waits_on_the_adoption() {
    let (root, _) = pending_adoption();
    let view = f::page_view(&root);
    assert!(has_state(view.tree(), Role::Button, b"Open Shop", State::Disabled));
    assert!(f::find(view.tree(), Role::Link, b"Open Shop").is_none());
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        4
    );
    node(view.tree(), Role::Status, b"under way");
}
#[test]
fn a_refused_adoption_says_its_outcome() {
    let (mut root, token) = pending_adoption();
    answer(
        &mut root,
        token,
        w::Outcome::ForgeRefused(
            w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason: w::ForgeReason::Unreachable }).unwrap(),
        ),
    );
    let view = f::page_view(&root);
    words_in_order(view.tree(), &["Forge 3 did not answer; nothing was adopted. Try again."]);
    assert!(f::find(view.tree(), Role::Link, b"Open Shop").is_none());
}
#[test]
fn a_failed_first_repository_says_its_step_with_set_up_again() {
    for (step, expected) in [
        (w::CreateStep::Admin, b"Making work/hello failed: checking the admin's token".as_slice()),
        (w::CreateStep::Org, b"Making work/hello failed: making the organization"),
        (w::CreateStep::Repository, b"Making work/hello failed: making the repository"),
        (w::CreateStep::File, b"Making work/hello failed: adding the starter workflow"),
    ] {
        for address in
            [Address::ConnectForge { forge: Some(Choice::Forge(HERE)) }, Address::Welcome { step: Step::Ready }]
        {
            let root = f::forge_root(
                address,
                first_rows(
                    connected(),
                    w::FirstState::Failed(
                        w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: step.clone() }).unwrap(),
                    ),
                ),
            );
            let view = f::page_view(&root);
            node(view.tree(), Role::Alert, expected);
            node(view.tree(), Role::Button, b"Set up again");
            if let Address::ConnectForge { .. } = root.pages().address() {
                assert!(f::find(view.tree(), Role::List, b"Each step").is_some());
            } else {
                node(view.tree(), Role::Disclosure, b"Each step");
            }
        }
    }
}
#[test]
fn the_forges_row_names_the_first_repository_and_project() {
    for (state, expected, first) in [
        (w::FirstState::Making, b"work/hello, into Shop".as_slice(), b"Making work/hello".as_slice()),
        (made(), "work/hello · project Shop".as_bytes(), "work/hello · project Shop".as_bytes()),
    ] {
        let root = f::forge_root(Address::Forges, first_rows(connected(), state.clone()));
        let view = f::page_view(&root);
        let id = node(view.tree(), Role::Disclosure, b"Forgejo on this machine");
        let row = view.tree().nodes().iter().find(|n| n.id == id).unwrap();
        let description = row.description.as_deref().unwrap();
        assert!(description.ends_with(expected));
        assert!(!row.states.as_slice().contains(&State::Expanded));
        node(view.tree(), if state == made() { Role::Description } else { Role::Paragraph }, first);
    }
}

pub(super) fn github_rows(accepted: bool) -> List<w::ForgeRow> {
    let mut row = row(
        GITHUB,
        w::Kind::Github,
        w::Mode::Connected,
        w::Connection::Connected(
            w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 9, login: b"my-app[bot]".as_slice().into() })
                .unwrap(),
        ),
        w::Offer::NotOffered,
    )
    .into_parts();
    row.ended = Some(
        w::ConnectStopped::new(&w::CEILINGS, w::ConnectStoppedParts { why: b"fixture stopped".as_slice().into() })
            .unwrap(),
    );
    row.installations = List::with_capacity(3);
    if accepted {
        row.installations
            .push(
                w::Installation::new(
                    &w::CEILINGS,
                    w::InstallationParts {
                        id: 1,
                        account: Some(
                            w::Account::new(&w::CEILINGS, w::AccountParts { id: 10, login: b"acme".as_slice().into() })
                                .unwrap(),
                        ),
                        suspended: false,
                        stray: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    for (id, suspended, stray) in [(2, true, !accepted), (3, false, true)] {
        row.installations
            .push(
                w::Installation::new(&w::CEILINGS, w::InstallationParts { id, account: None, suspended, stray })
                    .unwrap(),
            )
            .unwrap();
    }
    let mut rows = List::with_capacity(1);
    rows.push(w::ForgeRow::new(&w::CEILINGS, row).unwrap()).unwrap();
    rows
}
#[test]
fn connecting_on_github_offers_install_with_its_way_back() {
    for address in [
        Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) },
        Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(GITHUB)) } },
    ] {
        let root = f::forge_root(address.clone(), github_rows(false));
        let view = f::browser_page_view(&root);
        words_in_order(
            view.tree(),
            &["Install temper’s App", "my-app is connected to temper, and installed nowhere yet.", "Install on GitHub"],
        );
        let mut view = f::browser_page_view(&root);
        let button = node(view.tree(), Role::Button, b"Install on GitHub");
        assert_eq!(
            crate::act(&mut view, button, Act::Press),
            Some(d::Action::Forge(d::forge::Act::Install { forge: GITHUB, back: d::address::print(&address) }))
        );
    }
}
#[test]
fn an_accepted_installation_makes_github_ready_and_reads_its_picker() {
    let root = f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) }, github_rows(true));
    let view = f::browser_page_view(&root);
    words_in_order(
        view.tree(),
        &["Choose a repository to start in", "my-app is installed on acme.", "Reading GitHub's repositories…"],
    );
    assert!(f::find(view.tree(), Role::ChoiceGroup, b"Account").is_none());
}
#[test]
#[expect(clippy::disallowed_macros, reason = "the test captures Debug to verify secret redaction")]
fn the_private_key_moves_as_a_secret_and_never_enters_the_view() {
    let mut root = root(Some(Choice::Forge(GITHUB)), w::Connection::NotSetUp, w::Offer::Offered);
    let view = f::browser_page_view(&root);
    let id = node(view.tree(), Role::TextField, b"App id");
    let event = crate::event(&view, id, Act::Input { value: b"7".as_slice().into() }).unwrap();
    f::root_event(&mut root, event);
    let key = node(view.tree(), Role::TextField, b"Private key");
    let event = crate::event(&view, key, Act::Input { value: b"private-app-fixture\nsecond line\n".as_slice().into() })
        .unwrap();
    assert!(!format!("{event:?}").contains("private-app-fixture"));
    let d::Event::Forge(d::forge::Event::AppKey { .. }) = &event else {
        panic!("the input moves straight into a secret key event");
    };
    f::root_event(&mut root, event);
    let view = f::browser_page_view(&root);
    assert!(!format!("{view:?}").contains("private-app-fixture"));
    let field = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Private key"))
        .unwrap();
    assert!(field.value.as_ref().unwrap().text.is_empty());
    let written = field.value.as_ref().unwrap().written;
    f::root_event(&mut root, d::Event::Act { action: d::Action::Forge(d::forge::Act::ConnectApp { forge: GITHUB }) });
    let view = f::browser_page_view(&root);
    let field = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::TextField && n.name.as_deref() == Some(b"Private key"))
        .unwrap();
    assert!(field.value.as_ref().unwrap().written > written);
    assert!(!root.forge().app_form().has_key());
}
#[test]
fn a_connect_refusal_is_said_below_its_button() {
    let mut root = root(Some(Choice::Forge(GITHUB)), w::Connection::NotSetUp, w::Offer::Offered);
    f::root_event(
        &mut root,
        d::Event::Act { action: d::Action::Forge(d::forge::Act::AppWrite { text: b"7".as_slice().into() }) },
    );
    f::root_event(
        &mut root,
        d::Event::Forge(d::forge::Event::AppKey { value: skein_lib::Secret::new(b"fake-key".as_slice().into()) }),
    );
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&f::limits()));
    d::step(
        &mut root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: f::NOW, limits: f::limits() },
        d::Event::Act { action: d::Action::Forge(d::forge::Act::ConnectApp { forge: GITHUB }) },
        &mut out,
    );
    let mut attempt = None;
    while let Some(request) = out.pop() {
        if let d::Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt: value, .. } },
        } = request
        {
            attempt = Some(value);
        }
    }
    f::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: attempt.unwrap(),
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Family(jig_client::Family {
                        family: temper_client_protocol_wire::FORGE,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes: temper_client_protocol_wire::answer(
                            &w::Answer::new(
                                &w::CEILINGS,
                                w::AnswerParts {
                                    outcome: w::Outcome::ConnectRefused(
                                        w::ConnectRefused::new(
                                            &w::CEILINGS,
                                            w::ConnectRefusedParts { why: w::ForgeRefusal::NoKey },
                                        )
                                        .unwrap(),
                                    ),
                                },
                            )
                            .unwrap(),
                        ),
                    })),
                    why: None,
                },
            },
        },
    );
    let view = f::browser_page_view(&root);
    words_in_order(view.tree(), &["Connect the App", "The key did not reach temper: paste it again."]);
}

fn manifest_started(address: Address) -> (d::Domain, skein_lib::Queue<d::Request>) {
    let mut root = f::forge_root(address, rows(w::Connection::NotSetUp, w::Offer::Offered));
    let mut view = f::browser_page_view(&root);
    let button = node(view.tree(), Role::Button, b"Connect GitHub");
    let action = crate::act(&mut view, button, Act::Press).expect("live manifest button");
    let out = drive(&mut root, d::Event::Act { action });
    (root, out)
}
#[test]
fn connect_github_sends_the_flow_address_at_its_forge() {
    for (address, back) in [
        (Address::ConnectForge { forge: None }, b"/forges/connect?forge=2".as_slice()),
        (Address::Welcome { step: Step::Forge { forge: None } }, b"/welcome?at=forge&forge=2".as_slice()),
    ] {
        let (_, mut out) = manifest_started(address);
        let mut sent = None;
        while let Some(request) = out.pop() {
            if let d::Request::Client {
                request: jig_client::Request::Down { down: jig_client::Down::Send { request, value, .. } },
            } = request
            {
                assert!(value.is_none());
                let jig_client::Ask::Family { record, .. } = request else { panic!("family") };
                sent = Some(
                    temper_client_protocol_wire::decode_requesting(&record.bytes, &w::CEILINGS)
                        .unwrap()
                        .into_parts()
                        .request,
                );
            }
        }
        let w::Request::Connect(connect) = sent.expect("manifest Connect") else { panic!("Connect") };
        assert_eq!(connect.forge(), GITHUB);
        let w::How::GithubManifest(manifest) = connect.how() else { panic!("manifest") };
        assert_eq!(manifest.back().as_deref(), Some(back));
    }
}
#[test]
fn connect_github_is_disabled_until_the_answer() {
    let (root, _) = manifest_started(Address::ConnectForge { forge: None });
    let view = f::browser_page_view(&root);
    let button = node(view.tree(), Role::Button, b"Connect GitHub");
    assert!(view.tree().nodes().iter().find(|n| n.id == button).unwrap().states.as_slice().contains(&State::Disabled));
}
#[test]
fn connect_github_refusal_shows_under_its_button() {
    let (mut root, out) = manifest_started(Address::ConnectForge { forge: None });
    answer(
        &mut root,
        attempt(out),
        w::Outcome::ConnectRefused(
            w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::Busy }).unwrap(),
        ),
    );
    let view = f::browser_page_view(&root);
    words_in_order(
        view.tree(),
        &["Connect GitHub", "temper is busy with forge 2: try again when it ends.", "Use an App of your own"],
    );
}
#[test]
fn connect_github_link_leaves_the_tab_once() {
    let (mut root, out) = manifest_started(Address::ConnectForge { forge: None });
    let attempt = attempt(out);
    let outcome = w::Outcome::Link(
        w::Link::new(&w::CEILINGS, w::LinkParts { address: b"/forges/2/github/manifest?state=fake".as_slice().into() })
            .unwrap(),
    );
    let mut out = drive(&mut root, answer_event(attempt, outcome.clone()));
    let mut leaves = Vec::new();
    while let Some(request) = out.pop() {
        if let d::Request::Leave { to, .. } = request {
            leaves.push(to);
        }
    }
    assert_eq!(leaves, [Box::<[u8]>::from(b"/forges/2/github/manifest?state=fake".as_slice())]);
    let mut out = drive(&mut root, answer_event(attempt, outcome));
    while let Some(request) = out.pop() {
        if let d::Request::Leave { .. } = request {
            panic!("only the first answer may leave the page");
        }
    }
    let view = f::browser_page_view(&root);
    // The tab leaves by the path; no external link is drawn for it.
    assert!(view.tree().nodes().iter().all(|n| n.external.is_none()));
}
#[test]
fn a_whole_address_link_keeps_its_external_link_meanwhile() {
    let (mut root, out) = manifest_started(Address::ConnectForge { forge: None });
    let attempt = attempt(out);
    let address = b"https://github.com/apps/temper-fake/installations/new?state=fake";
    let outcome =
        w::Outcome::Link(w::Link::new(&w::CEILINGS, w::LinkParts { address: address.as_slice().into() }).unwrap());
    drive(&mut root, answer_event(attempt, outcome));
    let view = f::browser_page_view(&root);
    let link = node(view.tree(), Role::Link, b"Connect GitHub");
    assert_eq!(
        view.tree().nodes().iter().find(|n| n.id == link).unwrap().external.as_deref(),
        Some(address.as_slice())
    );
}
#[test]
fn connecting_uses_the_apps_way_for_its_words() {
    for (app, words) in [
        (
            Some(w::AppWay::Manifest),
            "my-app is made on GitHub, under your account, and installed nowhere yet. Install it, and choose the repositories temper may use.",
        ),
        (
            Some(w::AppWay::Pasted),
            "my-app is connected to temper, and installed nowhere yet. Install it, and choose the repositories temper may use.",
        ),
        (
            None,
            "my-app is connected to temper, and installed nowhere yet. Install it, and choose the repositories temper may use.",
        ),
    ] {
        let mut parts = github_rows(false).get(0).unwrap().clone().into_parts();
        parts.app = app;
        let mut rows = List::with_capacity(1);
        rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
        let root = f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) }, rows);
        words_in_order(f::browser_page_view(&root).tree(), &["Install temper’s App", words, "Install on GitHub"]);
    }
}

fn jig_answer(
    root: &mut d::Domain,
    token: skein_lib::Token,
    answer: jig_client::contract::Answer,
) -> skein_lib::Queue<d::Request> {
    drive(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: token,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(answer)),
                    why: None,
                },
            },
        },
    )
}
#[test]
fn ready_keeps_make_project_refusal_and_the_name_with_a_bound_retry_button() {
    for address in [Address::ConnectForge { forge: Some(Choice::Forge(HERE)) }, Address::Welcome { step: Step::Ready }]
    {
        let (mut root, watch) = f::forge_root_watch(address, rows(w::Connection::NotSetUp, w::Offer::Offered));
        let token = attempt(act(&mut root, d::forge::Act::SetUp { forge: HERE }));
        answer(&mut root, token, w::Outcome::SettingUp);
        let token = attempt(deliver(&mut root, watch, new_first_rows()));
        jig_answer(
            &mut root,
            token,
            jig_client::contract::Answer::Refused { refusal: jig_client::contract::Refusal::NameTaken, at: None },
        );
        let mut view = f::page_view(&root);
        let id = node(view.tree(), Role::TextField, b"Into a new project, named");
        assert_eq!(
            view.tree().nodes().iter().find(|n| n.id == id).unwrap().value.as_ref().map(|value| value.text.as_ref()),
            Some(b"hello".as_slice())
        );
        words_in_order(view.tree(), &["Another project has that name"]);
        let id = node(view.tree(), Role::Button, b"Adopt work/hello");
        assert_eq!(
            crate::act(&mut view, id, Act::Press),
            Some(d::Action::Forge(d::forge::Act::StartIn { forge: HERE }))
        );
        assert!(root.forge().set_up().unwrap().adoption.is_none());
    }
}
#[test]
fn ready_opens_the_made_project_and_uses_its_banner_name() {
    let (mut root, token, watch) = pending_adoption_with_watch();
    answer(
        &mut root,
        token,
        w::Outcome::Adopted(
            w::Adopted::new(
                &w::CEILINGS,
                w::AdoptedParts {
                    forge: HERE,
                    repository: 7,
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    role: w::Role::Owned,
                    home: true,
                    kinds: w::Kinds::new(&w::CEILINGS, w::KindsParts { skein_bools: [true; 9] }).unwrap(),
                    protection: w::Protection::Unknown,
                },
            )
            .unwrap(),
        ),
    );
    publish_first_adoption(&mut root, watch);
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, b"In Shop, on Forgejo on this machine.");
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        5
    );
    assert_eq!(
        press(&root, Role::Link, b"Open Shop"),
        d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT })
    );
}

fn new_first_rows() -> List<w::ForgeRow> {
    let mut rows = first_rows(connected(), made());
    let row = rows.get_mut(0).unwrap();
    let mut parts = row.clone().into_parts();
    let mut first = parts.first.take().unwrap().into_parts();
    first.project = w::SetUpProject::New(
        w::NewProject::new(&w::CEILINGS, w::NewProjectParts { name: b"hello".as_slice().into() }).unwrap(),
    );
    parts.first = Some(w::FirstRepository::new(&w::CEILINGS, first).unwrap());
    *row = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
    rows
}

pub(super) fn parent(tree: &skein_ui::Tree, id: skein_ui::NodeId) -> Option<skein_ui::NodeId> {
    let index = tree.nodes().iter().position(|n| n.id == id).unwrap();
    tree.nodes()[..index]
        .iter()
        .enumerate()
        .rev()
        .find(|(start, n)| start.checked_add(usize::try_from(n.size).unwrap()).unwrap() > index)
        .map(|(_, n)| n.id)
}

#[test]
fn delivered_steps_keep_making_current_until_adopt_succeeds() {
    let (mut root, watch) = f::forge_root_watch(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    let token = attempt(act(&mut root, d::forge::Act::SetUp { forge: HERE }));
    answer(&mut root, token, w::Outcome::SettingUp);
    for (connection, done) in [
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(w::SetUpStep::Fetch) }).unwrap(),
            ),
            0,
        ),
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(w::SetUpStep::Prepare) }).unwrap(),
            ),
            1,
        ),
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(w::SetUpStep::Run) }).unwrap(),
            ),
            2,
        ),
        (w::Connection::Connecting, 3),
        (connected(), 4),
    ] {
        deliver(&mut root, watch, first_rows(connection, w::FirstState::Making));
        let view = f::page_view(&root);
        let list = view
            .tree()
            .nodes()
            .iter()
            .position(|n| n.role == Role::List && n.name.as_deref() == Some(b"Each step"))
            .unwrap();
        let nodes = &view.tree().nodes()
            [list..list.checked_add(usize::try_from(view.tree().nodes()[list].size).unwrap()).unwrap()];
        assert_eq!(nodes.iter().filter(|n| n.role == Role::Item).count(), 5);
        assert_eq!(nodes.iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(), done);
        assert_eq!(
            nodes.iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"under way")).count(),
            1
        );
    }
    let (mut root, token, watch) = pending_adoption_with_watch();
    let view = f::page_view(&root);
    words_in_order(
        view.tree(),
        &[
            "Fetched Forgejo and its runner",
            "Connecting as temper",
            "Making work/hello, and adopting it into Shop",
            "under way",
        ],
    );
    answer(
        &mut root,
        token,
        w::Outcome::Adopted(
            w::Adopted::new(
                &w::CEILINGS,
                w::AdoptedParts {
                    forge: HERE,
                    repository: 7,
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    role: w::Role::Owned,
                    home: true,
                    kinds: w::Kinds::new(&w::CEILINGS, w::KindsParts { skein_bools: [true; 9] }).unwrap(),
                    protection: w::Protection::Unknown,
                },
            )
            .unwrap(),
        ),
    );
    publish_first_adoption(&mut root, watch);
    let view = f::page_view(&root);
    assert_eq!(
        view.tree().nodes().iter().filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"done")).count(),
        5
    );
}

#[test]
fn an_unknown_setup_stage_claims_no_specific_current_step() {
    let view = f::page_view(&here(w::Connection::SettingUp(
        w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: None }).unwrap(),
    )));
    let list = view
        .tree()
        .nodes()
        .iter()
        .position(|n| n.role == Role::List && n.name.as_deref() == Some(b"Each step"))
        .unwrap();
    let nodes =
        &view.tree().nodes()[list..list.checked_add(usize::try_from(view.tree().nodes()[list].size).unwrap()).unwrap()];
    assert!(!nodes.iter().any(|n| n.states.as_slice().contains(&State::Current)));
    node(view.tree(), Role::Status, b"under way");
}

#[test]
fn welcome_ready_waits_through_made_and_adopt_then_opens_the_adopted_project() {
    let (mut root, token) = pending_adoption();
    f::root_event(&mut root, d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Ready }) });
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, b"Almost ready");
    assert!(has_state(view.tree(), Role::Button, b"Open Shop", State::Disabled));
    node(view.tree(), Role::Disclosure, b"Each step");
    answer(
        &mut root,
        token,
        w::Outcome::Adopted(
            w::Adopted::new(
                &w::CEILINGS,
                w::AdoptedParts {
                    forge: HERE,
                    repository: 7,
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    role: w::Role::Owned,
                    home: true,
                    kinds: w::Kinds::new(&w::CEILINGS, w::KindsParts { skein_bools: [true; 9] }).unwrap(),
                    protection: w::Protection::Unknown,
                },
            )
            .unwrap(),
        ),
    );
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, b"work/hello is ready");
    assert!(f::find(view.tree(), Role::Heading { level: 2 }, b"work/hello is ready").is_none());
    assert!(f::find(view.tree(), Role::Disclosure, b"Each step").is_none());
    words_in_order(
        view.tree(),
        &["Forge: Forgejo on this machine, connected as temper", "Project Shop, with work/hello", "Open Shop"],
    );
    assert_eq!(
        press(&root, Role::Link, b"Open Shop"),
        d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT })
    );
}

#[test]
fn standalone_setup_names_the_requested_target_before_the_row_has_it() {
    let (mut root, watch) = f::forge_root_watch(
        Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
        rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    let token = attempt(act(&mut root, d::forge::Act::SetUp { forge: HERE }));
    answer(&mut root, token, w::Outcome::SettingUp);
    deliver(&mut root, watch, rows(w::Connection::Connecting, w::Offer::Offered));
    let view = f::page_view(&root);
    let text = f::text(view.tree());
    let text = core::str::from_utf8(&text).unwrap();
    assert!(text.contains("Making work/hello, and adopting it into hello"));
    assert!(!text.contains("Making the first repository"));
}

fn picker_rows(two: bool) -> List<w::ForgeRow> {
    let mut parts = github_rows(true).as_slice()[0].clone().into_parts();
    let mut rows = List::with_capacity(1);
    if two {
        parts.installations = List::with_capacity(2);
        parts.installations.push(github_rows(true).as_slice()[0].installations().as_slice()[0].clone()).unwrap();
        parts
            .installations
            .push(
                w::Installation::new(
                    &w::CEILINGS,
                    w::InstallationParts {
                        id: 4,
                        account: Some(
                            w::Account::new(
                                &w::CEILINGS,
                                w::AccountParts { id: 40, login: b"other".as_slice().into() },
                            )
                            .unwrap(),
                        ),
                        suspended: false,
                        stray: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    rows
}
fn picker_page(empty: bool, more: bool) -> w::Outcome {
    let mut rows = List::with_capacity(2);
    if !empty {
        for (id, name, private) in [(1, b"first".as_slice(), false), (2, b"second".as_slice(), true)] {
            rows.push(
                w::Reached::new(
                    &w::CEILINGS,
                    w::ReachedParts {
                        id,
                        owner: w::Account::new(
                            &w::CEILINGS,
                            w::AccountParts { id: 10, login: b"acme".as_slice().into() },
                        )
                        .unwrap(),
                        name: name.into(),
                        private,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        }
    }
    w::Outcome::Page(w::Page::new(&w::CEILINGS, w::PageParts { rows, more }).unwrap())
}
fn picker_started(two: bool) -> (d::Domain, skein_lib::Token, skein_lib::Token) {
    let (mut root, watch) =
        f::forge_root_watch(Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) }, List::with_capacity(0));
    let token = attempt(deliver(&mut root, watch, picker_rows(two)));
    (root, watch, token)
}
#[test]
fn github_bar_has_choose_connecting_and_ready_with_own_app_under_choose() {
    for rows in [rows(w::Connection::NotSetUp, w::Offer::Offered), github_rows(false), github_rows(true)] {
        let root = f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) }, rows);
        let view = f::browser_page_view(&root);
        let tree = view.tree();
        let steps = node(tree, Role::List, b"Steps");
        let index = tree.nodes().iter().position(|n| n.id == steps).unwrap();
        let size = usize::try_from(tree.nodes()[index].size).unwrap();
        let labels: Vec<_> = tree.nodes()[index..][..size]
            .iter()
            .enumerate()
            .filter(|(_, n)| n.role == Role::Item)
            .map(|(offset, _)| f::node_text(tree, index + offset))
            .collect();
        assert_eq!(
            labels.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            [b"Choose".as_slice(), b"Connecting", b"Ready"]
        );
    }
}
#[test]
fn github_picker_accounts_rows_pages_and_name_are_bound() {
    let (mut root, _, token) = picker_started(true);
    answer(&mut root, token, picker_page(false, true));
    let mut view = f::browser_page_view(&root);
    node(view.tree(), Role::ChoiceGroup, b"Account");
    assert!(has_state(view.tree(), Role::Choice, b"acme", State::Selected));
    node(view.tree(), Role::Choice, b"other");
    assert!(has_state(view.tree(), Role::Choice, b"acme/first", State::Selected));
    let first = node(view.tree(), Role::Choice, b"acme/first");
    let first_row = view.tree().nodes().iter().find(|n| Some(n.id) == parent(view.tree(), first)).unwrap();
    assert!(first_row.styles.as_slice().contains(&crate::look::REPOSITORY_CHOICE));
    assert!(first_row.styles.as_slice().contains(&crate::look::REPOSITORY_SELECTED));
    let second = node(view.tree(), Role::Choice, b"acme/second");
    let at = view.tree().nodes().iter().position(|n| n.id == second).unwrap() - 1;
    assert!(view.tree().nodes()[at].styles.as_slice().contains(&crate::look::REPOSITORY_CHOICE));
    assert!(!view.tree().nodes()[at].styles.as_slice().contains(&crate::look::REPOSITORY_SELECTED));
    let subtree = &view.tree().nodes()[at..at + usize::try_from(view.tree().nodes()[at].size).unwrap()];
    assert!(
        subtree.iter().any(|n| n.role == Role::Text && n.styles.as_slice().contains(&crate::look::REPOSITORY_PRIVATE))
    );
    assert_eq!(f::node_text(view.tree(), at).as_ref(), b"private");
    words_in_order(
        view.tree(),
        &["Not listed? ", "Choose its repositories on GitHub", ", and GitHub sends you back here."],
    );

    node(view.tree(), Role::Button, b"Next");
    let action = crate::act(&mut view, second, Act::Toggle).unwrap();
    drive(&mut root, d::Event::Act { action });
    let view = f::browser_page_view(&root);
    let selected = node(view.tree(), Role::Choice, b"acme/second");
    let selected_row = view.tree().nodes().iter().find(|n| Some(n.id) == parent(view.tree(), selected)).unwrap();
    assert!(selected_row.styles.as_slice().contains(&crate::look::REPOSITORY_SELECTED));
    let name = node(view.tree(), Role::TextField, b"Into a new project, named");
    assert_eq!(
        view.tree().nodes().iter().find(|n| n.id == name).unwrap().value.as_ref().unwrap().text.as_ref(),
        b"second"
    );
    let id = node(view.tree(), Role::Button, b"Adopt acme/second");
    assert!(
        view.tree().nodes().iter().find(|n| n.id == id).unwrap().styles.as_slice().contains(&crate::look::FLOW_PRIMARY)
    );
    let action = press(&root, Role::Button, b"Next");
    let token = attempt(drive(&mut root, d::Event::Act { action }));
    answer(&mut root, token, picker_page(false, false));
    node(f::browser_page_view(&root).tree(), Role::Button, b"First page");
    let back = d::address::print(root.pages().address());
    let mut view = f::browser_page_view(&root);
    let id = node(view.tree(), Role::Link, b"Choose its repositories on GitHub");
    assert_eq!(
        crate::act(&mut view, id, Act::Press),
        Some(d::Action::Forge(d::forge::Act::Install { forge: GITHUB, back }))
    );
}
#[test]
fn github_picker_keeps_empty_pending_and_refused_states() {
    let (mut root, _, token) = picker_started(false);
    node(f::browser_page_view(&root).tree(), Role::Status, "Reading GitHub's repositories…".as_bytes());
    answer(&mut root, token, picker_page(true, false));
    node(f::browser_page_view(&root).tree(), Role::Status, b"None reachable.");
    let action = d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Account(1) };
    let token = attempt(act(&mut root, action));
    answer(
        &mut root,
        token,
        w::Outcome::ConnectRefused(
            w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::Busy }).unwrap(),
        ),
    );
    words_in_order(
        f::browser_page_view(&root).tree(),
        &["Choose a repository to start in", "temper is busy with forge 2: try again when it ends."],
    );
    let token =
        attempt(act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Account(1) }));
    answer(&mut root, token, picker_page(false, false));
    act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Write(Box::default()) });
    act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Adopt });
    node(f::browser_page_view(&root).tree(), Role::Alert, b"Required.");
    act(
        &mut root,
        d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Write(b"my project".as_slice().into()) },
    );
    let token = attempt(act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Adopt }));
    assert!(has_state(f::browser_page_view(&root).tree(), Role::Button, b"Adopt acme/first", State::Disabled));
    jig_answer(
        &mut root,
        token,
        jig_client::contract::Answer::Refused { refusal: jig_client::contract::Refusal::NameTaken, at: None },
    );
    words_in_order(
        f::browser_page_view(&root).tree(),
        &["Choose a repository to start in", "Another project has that name"],
    );
}

#[test]
fn github_picker_read_refusals_show_their_words_and_keep_install_available() {
    for (reason, expected) in [
        (
            w::ForgeReason::NoAccess,
            b"GitHub refused temper's read of the App's repositories: the App may be suspended. See Forges.".as_slice(),
        ),
        (w::ForgeReason::NotFound, b"GitHub no longer has this installation: install the App again.".as_slice()),
    ] {
        let (mut root, _, token) = picker_started(false);
        answer(
            &mut root,
            token,
            w::Outcome::ForgeRefused(w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason }).unwrap()),
        );
        for mut view in [f::page_view(&root), f::browser_page_view(&root)] {
            node(view.tree(), Role::Alert, expected);
            assert!(f::find(view.tree(), Role::ChoiceGroup, b"Repositories").is_none());
            let install = node(view.tree(), Role::Link, b"Choose its repositories on GitHub");
            assert_eq!(
                crate::act(&mut view, install, Act::Press),
                Some(d::Action::Forge(d::forge::Act::Install {
                    forge: GITHUB,
                    back: d::address::print(root.pages().address()),
                }))
            );
        }
    }
}

#[test]
fn github_picker_exhausted_busy_rereads_offer_reload_with_the_busy_words() {
    let (mut root, _, mut token) = picker_started(false);
    let mut env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: f::NOW, limits: f::domain_limits() };
    for reread in 0_u8..=3 {
        let mut out = skein_lib::Queue::with_capacity(d::max_out(&env.limits));
        d::step(
            &mut root,
            &env,
            answer_event(
                token,
                w::Outcome::ConnectRefused(
                    w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::Busy })
                        .unwrap(),
                ),
            ),
            &mut out,
        );
        let view = f::browser_page_view(&root);
        node(view.tree(), Role::Alert, b"temper is busy with forge 2: try again when it ends.");
        assert_eq!(f::find(view.tree(), Role::Paragraph, b"Reload this page to try again.").is_some(), reread == 3);
        if reread != 3 {
            env.now = env.now.checked_add(skein_lib::Duration::from_secs(2)).unwrap();
            d::fire(&mut root, &env, &mut out);
            token = attempt(out);
        }
    }
}
fn adopted_github_rows(web: bool) -> List<w::ForgeRow> {
    let mut parts = github_rows(true).as_slice()[0].clone().into_parts();
    let mut rows = List::with_capacity(1);
    parts.web = if web { Some(b"https://github.test/base/".as_slice().into()) } else { None };
    parts.repositories = List::with_capacity(1);
    parts
        .repositories
        .push(
            w::Adoption::new(
                &w::CEILINGS,
                w::AdoptionParts {
                    owner: b"owner /?".as_slice().into(),
                    name: b"repo/#%".as_slice().into(),
                    project: f::PROJECT,
                },
            )
            .unwrap(),
        )
        .unwrap();
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    rows
}
#[test]
fn github_adopted_card_and_completion_follow_the_row_after_reload_and_encode_the_external_address() {
    for web in [false, true] {
        let root =
            f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(GITHUB)) }, adopted_github_rows(web));
        let view = f::browser_page_view(&root);
        node(view.tree(), Role::Heading { level: 2 }, b"owner /?/repo/#% is ready");
        node(view.tree(), Role::Paragraph, b"In the new project Shop, on GitHub.");
        let open = node(view.tree(), Role::Link, b"Open Shop");
        assert!(
            view.tree()
                .nodes()
                .iter()
                .find(|n| n.id == open)
                .unwrap()
                .styles
                .as_slice()
                .contains(&crate::look::FLOW_PRIMARY)
        );
        assert_eq!(
            press(&root, Role::Link, b"Open Shop"),
            d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT })
        );
        let external = f::find(view.tree(), Role::Link, "Open on GitHub ↗".as_bytes());
        assert_eq!(external.is_some(), web);
        if let Some(id) = external {
            assert_eq!(
                view.tree().nodes().iter().find(|n| n.id == id).unwrap().external.as_deref(),
                Some(b"https://github.test/base/owner%20%2F%3F/repo%2F%23%25".as_slice())
            );
        }
        let d::forge::forges::State::Rows(rows) = root.forges() else { panic!("rows") };
        assert_eq!(crate::forge::connect::adopted_project(&root, &rows[0]), Some(f::PROJECT));
        assert!(f::find(view.tree(), Role::Heading { level: 2 }, b"Choose a repository to start in").is_none());
    }
}

#[test]
fn forgejo_completion_matches_the_made_repository_in_the_durable_row() {
    let mut rows = first_rows(connected(), made());
    let mut parts = rows.as_slice()[0].clone().into_parts();
    parts.repositories = List::with_capacity(2);
    for (owner, name, project) in
        [(b"other".as_slice(), b"hello".as_slice(), 99), (b"work".as_slice(), b"hello".as_slice(), f::PROJECT)]
    {
        parts
            .repositories
            .push(
                w::Adoption::new(&w::CEILINGS, w::AdoptionParts { owner: owner.into(), name: name.into(), project })
                    .unwrap(),
            )
            .unwrap();
    }
    rows.clear();
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    let root = f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(HERE)) }, rows);
    let d::forge::forges::State::Rows(rows) = root.forges() else { panic!("rows") };
    assert!(root.forge().set_up().is_none());
    assert_eq!(crate::forge::connect::adopted_project(&root, &rows[0]), Some(f::PROJECT));
}
#[test]
fn github_adopt_refusals_keep_the_picker_and_success_keeps_the_findings_until_reload() {
    for success in [false, true] {
        let (mut root, watch, token) = picker_started(false);
        answer(&mut root, token, picker_page(false, false));
        act(
            &mut root,
            d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Write(b"Shop".as_slice().into()) },
        );
        let token = attempt(act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Adopt }));
        let token = attempt(jig_answer(
            &mut root,
            token,
            jig_client::contract::Answer::Done(jig_client::contract::Outcome::Owner(
                jig_client::contract::OwnerOutcome::ProjectMade { project: f::PROJECT },
            )),
        ));
        if success {
            answer(
                &mut root,
                token,
                w::Outcome::Adopted(
                    w::Adopted::new(
                        &w::CEILINGS,
                        w::AdoptedParts {
                            forge: GITHUB,
                            repository: 1,
                            owner: b"acme".as_slice().into(),
                            name: b"first".as_slice().into(),
                            role: w::Role::Owned,
                            home: true,
                            kinds: w::Kinds::new(&w::CEILINGS, w::KindsParts { skein_bools: [true; 9] }).unwrap(),
                            protection: w::Protection::Absent,
                        },
                    )
                    .unwrap(),
                ),
            );
            let mut parts = picker_rows(false).as_slice()[0].clone().into_parts();
            parts.repositories = List::with_capacity(1);
            parts
                .repositories
                .push(
                    w::Adoption::new(
                        &w::CEILINGS,
                        w::AdoptionParts {
                            owner: b"acme".as_slice().into(),
                            name: b"first".as_slice().into(),
                            project: f::PROJECT,
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
            let mut rows = List::with_capacity(1);
            rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
            deliver(&mut root, watch, rows);
            words_in_order(
                f::browser_page_view(&root).tree(),
                &[
                    "acme/first is ready",
                    "In the new project Shop, on GitHub.",
                    "temper may: read; push; open pull requests; land; review; comment; create issues; create and delete branches.",
                    "Commit statuses stay temper's own: GitHub's rules cannot require them.",
                    "The branch has no rules.",
                ],
            );
        } else {
            answer(
                &mut root,
                token,
                w::Outcome::ForgeRefused(
                    w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason: w::ForgeReason::NoAccess })
                        .unwrap(),
                ),
            );
            words_in_order(
                f::browser_page_view(&root).tree(),
                &["Choose a repository to start in", "temper's forge user may not read acme/first on forge 2."],
            );
        }
    }
}

fn publish_first_adoption(root: &mut d::Domain, watch: skein_lib::Token) {
    // The engine publishes the adoption on the forge row as well as answering it.
    let adoption = root.forge().set_up().unwrap().adoption.as_ref().unwrap().value.clone();
    let mut parts = first_rows(connected(), made()).as_slice()[0].clone().into_parts();
    parts.repositories = List::with_capacity(1);
    parts
        .repositories
        .push(
            w::Adoption::new(
                &w::CEILINGS,
                w::AdoptionParts { owner: adoption.owner, name: adoption.name, project: adoption.project },
            )
            .unwrap(),
        )
        .unwrap();
    let mut rows = List::with_capacity(1);
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    deliver(root, watch, rows);
}

// The mockup's failed frame, and each other step of the same five-step flow.
pub(super) fn failure_cases() -> [(List<w::ForgeRow>, Option<usize>, &'static str); 6] {
    let rows = |at| {
        first_rows(
            w::Connection::SetUpFailed(
                w::SetUpFailed::new(
                    &w::CEILINGS,
                    w::SetUpFailedParts {
                        why: "its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest."
                            .as_bytes()
                            .into(),
                        at,
                    },
                )
                .unwrap(),
            ),
            w::FirstState::Making,
        )
    };
    [
        (
            rows(Some(w::FailedStep::Fetch)),
            Some(0),
            "Fetching failed: its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest.",
        ),
        (
            rows(Some(w::FailedStep::Prepare)),
            Some(1),
            "Preparing failed: its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest.",
        ),
        (
            rows(Some(w::FailedStep::Run)),
            Some(2),
            "Starting failed: its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest.",
        ),
        (
            rows(Some(w::FailedStep::Connect)),
            Some(3),
            "Connecting failed: its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest.",
        ),
        (
            first_rows(
                connected(),
                w::FirstState::Failed(
                    w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: w::CreateStep::File }).unwrap(),
                ),
            ),
            Some(4),
            "Making work/hello failed: adding the starter workflow",
        ),
        (
            rows(None),
            None,
            "Set up failed: its port, 127.0.0.1:8368, is taken by another program. temper box’s output has the rest.",
        ),
    ]
}
pub(super) fn failed_progress(tree: &skein_ui::Tree, failed: Option<usize>, why: &str) {
    let list = node(tree, Role::List, b"Each step");
    let start = tree.nodes().iter().position(|n| n.id == list).unwrap();
    let end = start.checked_add(usize::try_from(tree.nodes()[start].size).unwrap()).unwrap();
    let items: Vec<_> = tree.nodes()[start..end].iter().filter(|n| n.role == Role::Item).collect();
    assert_eq!(items.len(), 5);
    let labels = [
        if failed > Some(0) { "Fetched Forgejo and its runner" } else { "Fetching Forgejo and its runner" },
        "Preparing: its database, temper’s admin, your account",
        "Starting Forgejo and its runner",
        "Connecting as temper",
        "Making work/hello, and adopting it into Shop",
    ];
    for (index, (item, label)) in items.iter().zip(labels).enumerate() {
        let item_index = tree.nodes().iter().position(|n| n.id == item.id).unwrap();
        assert!(f::node_text(tree, item_index).starts_with(label.as_bytes()), "step {index}: {label}");
        assert_eq!(item.styles.as_slice().contains(&crate::look::FLOW_FAILED), failed == Some(index));
        assert_eq!(item.styles.as_slice().contains(&crate::look::FLOW_DONE), failed.is_some_and(|at| index < at));
        assert!(!item.states.as_slice().contains(&State::Current));
    }
    let alert = node(tree, Role::Alert, why.as_bytes());
    assert!(tree.nodes().iter().position(|n| n.id == alert).unwrap() >= end);
    assert_eq!(
        tree.nodes()[start..end]
            .iter()
            .filter(|n| n.role == Role::Status && n.name.as_deref() == Some(b"failed"))
            .count(),
        usize::from(failed.is_some())
    );
}
#[test]
fn failed_setup_keeps_all_steps_and_marks_only_its_failure() {
    for (rows, failed, why) in failure_cases() {
        let root = f::forge_root(Address::ConnectForge { forge: Some(Choice::Forge(HERE)) }, rows);
        let view = f::page_view(&root);
        failed_progress(view.tree(), failed, why);
    }
}

#[test]
fn setup_failure_words_keep_the_full_bounded_reason() {
    let words = f::words();
    let bound = usize::try_from(temper_client_protocol_wire::SET_UP_WHY_BYTES).unwrap();
    for (bytes, reason) in [([b'x'].repeat(bound), "x".repeat(bound)), ([0xff].repeat(bound), "�".repeat(bound))] {
        let shown = d::forge::words::lossy(&bytes);
        let text = words.say(crate::words::Word::ForgePrepareFailed, &[skein_ui::phrases::Hole::Name(shown)]);
        let mut expected = String::from("Preparing failed: ");
        expected.push_str(&reason);
        assert_eq!(text.as_ref(), expected.as_bytes());
    }
}

const ADMIN_TOKEN: &[u8] = b"admin-token-placeholder-0123";
fn admin_row(
    connection: w::Connection,
    stop: Option<w::AdminStop>,
    first: Option<w::FirstRepository>,
    web: bool,
) -> w::ForgeRow {
    let mut parts = row(HERE, w::Kind::Forgejo, w::Mode::Managed, connection, w::Offer::NotOffered).into_parts();
    parts.admin = true;
    parts.ended_stop = stop;
    parts.first = first;
    parts.ended = Some(
        w::ConnectStopped::new(&w::CEILINGS, w::ConnectStoppedParts { why: b"Forgejo answered 500".as_slice().into() })
            .unwrap(),
    );
    if web {
        parts.web = Some(b"https://git.example.org".as_slice().into());
    }
    w::ForgeRow::new(&w::CEILINGS, parts).unwrap()
}
fn admin_root(choice: Option<Choice>, row: w::ForgeRow) -> d::Domain {
    let mut rows = List::with_capacity(1);
    rows.push(row).unwrap();
    f::forge_root(Address::ConnectForge { forge: choice }, rows)
}
fn admin_input(root: &mut d::Domain, name: &[u8], value: &[u8]) {
    let view = f::page_view(root);
    let id = node(view.tree(), Role::TextField, name);
    let event = crate::event(&view, id, Act::Input { value: value.into() }).unwrap();
    f::root_event(root, event);
}
fn admin_send(root: &mut d::Domain) {
    let action = press(root, Role::Button, b"Connect as its admin");
    f::root_event(root, d::Event::Act { action });
}
fn no_admin_token(tree: &skein_ui::Tree) {
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
}
#[test]
fn the_admin_card_names_the_configured_host_or_forge_number() {
    for (web, heading) in [(true, b"Forgejo at https://git.example.org".as_slice()), (false, b"Forgejo at forge 3")] {
        let root = admin_root(None, admin_row(w::Connection::NotConnected, None, None, web));
        let view = f::page_view(&root);
        words_in_order(
            view.tree(),
            &[
                core::str::from_utf8(heading).unwrap(),
                "temper connects as its site admin, makes its bot and your first repository.",
                "Connect as its admin",
                "A Forgejo elsewhere",
            ],
        );
    }
}
#[test]
fn admin_details_keep_the_token_out_of_actions_and_rendered_pages() {
    let mut root = admin_root(Some(Choice::Forge(HERE)), admin_row(w::Connection::NotConnected, None, None, true));
    let mut before = f::page_view(&root);
    let group = node(before.tree(), Role::Region, b"Your first repository");
    let legend = node(before.tree(), Role::Heading { level: 3 }, b"Your first repository");
    let nodes = before.tree().nodes();
    assert!(nodes.iter().find(|n| n.id == group).unwrap().styles.as_slice().contains(&crate::look::ADMIN_FIRST));
    let start = nodes.iter().position(|n| n.id == group).unwrap();
    let end = start + usize::try_from(nodes[start].size).unwrap();
    assert_eq!(nodes[start + 1].id, legend);
    for name in [b"Organization".as_slice(), b"Repository", b"Into a new project, named", b"Runner label"] {
        let field = node(before.tree(), Role::TextField, name);
        assert!(nodes[start + 1..end].iter().any(|n| n.id == field));
    }
    let field = node(before.tree(), Role::TextField, "The site admin’s token".as_bytes());
    assert!(crate::act(&mut before, field, Act::Input { value: ADMIN_TOKEN.into() }).is_none());
    admin_input(&mut root, "The site admin’s token".as_bytes(), ADMIN_TOKEN);
    let view = f::page_view(&root);
    let field = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.id == node(view.tree(), Role::TextField, "The site admin’s token".as_bytes()))
        .unwrap();
    assert_eq!(field.entry, Some(skein_ui::EntryKind::Secret));
    no_admin_token(view.tree());
    for name in [b"Organization".as_slice(), b"Repository", b"Into a new project, named", b"Runner label"] {
        node(view.tree(), Role::TextField, name);
    }
    admin_send(&mut root);
    assert!(!root.forge().admin_form().has_token());
    no_admin_token(f::page_view(&root).tree());
}
#[test]
fn admin_local_refusals_mark_the_field_and_say_why() {
    for (field, value, refusal) in [
        ("The site admin’s token", b"".as_slice(), "Required."),
        ("The site admin’s token", &[b'x'; 4091], "The pasted values are past 4 KiB, the most a request carries."),
        ("Organization", b"", "Required."),
        ("Repository", b"", "Required."),
        ("Into a new project, named", b"", "Required."),
        ("Runner label", b"a b", "Letters, digits, dots, dashes and underscores, at most 64."),
    ] {
        let mut root = admin_root(Some(Choice::Forge(HERE)), admin_row(w::Connection::NotConnected, None, None, true));
        admin_input(&mut root, field.as_bytes(), value);
        admin_send(&mut root);
        let view = f::page_view(&root);
        words_in_order(view.tree(), &[refusal]);
        assert!(
            view.tree()
                .nodes()
                .iter()
                .find(|n| n.id == node(view.tree(), Role::TextField, field.as_bytes()))
                .unwrap()
                .states
                .as_slice()
                .contains(&State::Invalid)
        );
        no_admin_token(view.tree());
    }
}
#[test]
fn a_first_organization_named_as_the_bot_marks_the_organization_invalid() {
    let mut root = admin_root(Some(Choice::Forge(HERE)), admin_row(w::Connection::NotConnected, None, None, true));
    admin_input(&mut root, "The site admin’s token".as_bytes(), ADMIN_TOKEN);
    admin_input(&mut root, b"Organization", b"temper-bot");
    let action = press(&root, Role::Button, b"Connect as its admin");
    let token = attempt(drive(&mut root, d::Event::Act { action }));
    answer(
        &mut root,
        token,
        w::Outcome::ConnectRefused(
            w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::FirstIsBot }).unwrap(),
        ),
    );
    let view = f::page_view(&root);
    words_in_order(
        view.tree(),
        &[
            "That organization has the name of temper's bot on this forge, and on Forgejo users and organizations share names: name another organization.",
        ],
    );
    assert!(has_state(view.tree(), Role::TextField, b"Organization", State::Invalid));
    for name in [b"Repository".as_slice(), b"Into a new project, named", b"Runner label"] {
        assert!(!has_state(view.tree(), Role::TextField, name, State::Invalid));
    }
    assert!(!root.forge().admin_form().has_token());
    no_admin_token(view.tree());
}
#[test]
fn admin_connecting_names_the_forge_and_first_repository() {
    let root = admin_root(Some(Choice::Forge(HERE)), admin_row(w::Connection::Connecting, None, None, true));
    let view = f::page_view(&root);
    words_in_order(view.tree(), &["Connecting forge 3 as its admin.", "Making its bot, then work/hello."]);
    assert!(f::find(view.tree(), Role::Button, b"Connect as its admin").is_none());
}
#[test]
fn admin_stops_name_the_reason_and_keep_an_empty_token_field() {
    let refused = |status, stage| {
        w::AdminStop::Refused(w::AdminRefused::new(&w::CEILINGS, w::AdminRefusedParts { status, stage }).unwrap())
    };
    for (stop, line) in [
        (w::AdminStop::NotAdmin, "The token you gave is not a site admin’s. Paste one a site admin made."),
        (
            refused(401, w::SetupStage::Admin),
            "Forgejo refused the token. Check that it was copied whole, and that it hasn’t been deleted.",
        ),
        (refused(403, w::SetupStage::Bot), "The token lacks a scope: making the bot was refused."),
        (refused(403, w::SetupStage::Org), "The token lacks a scope: making the organization was refused."),
        (w::AdminStop::Unmarked, "A user temper didn’t make holds the bot’s login"),
        (w::AdminStop::Other, "Forgejo answered 500"),
    ] {
        let root =
            admin_root(Some(Choice::Forge(HERE)), admin_row(w::Connection::NotConnected, Some(stop), None, true));
        let view = f::page_view(&root);
        words_in_order(view.tree(), &[line, "temper kept nothing of the token."]);
        node(view.tree(), Role::TextField, "The site admin’s token".as_bytes());
        no_admin_token(view.tree());
    }
}
fn admin_first(state: w::FirstState) -> w::FirstRepository {
    w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::New(
                w::NewProject::new(&w::CEILINGS, w::NewProjectParts { name: b"hello".as_slice().into() }).unwrap(),
            ),
            state,
            label: b"linux-arm".as_slice().into(),
        },
    )
    .unwrap()
}
#[test]
fn each_admin_create_step_returns_to_details_with_its_repository() {
    for (step, words) in [
        (w::CreateStep::Admin, "checking the admin's token"),
        (w::CreateStep::Org, "its organization"),
        (w::CreateStep::Repository, "its repository"),
        (w::CreateStep::File, "its starter workflow"),
    ] {
        let first = admin_first(w::FirstState::Failed(
            w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step }).unwrap(),
        ));
        let root = admin_root(Some(Choice::Forge(HERE)), admin_row(connected(), None, Some(first), true));
        let view = f::page_view(&root);
        words_in_order(
            view.tree(),
            &[
                "temper made its bot, but making work/hello stopped at",
                words,
                "Forgejo answered 500. Paste the token again to finish it.",
                "temper kept nothing of the token.",
            ],
        );
        node(view.tree(), Role::Button, b"Connect as its admin");
        no_admin_token(view.tree());
    }
}
#[test]
fn admin_ready_uses_the_durable_label_and_repository_link() {
    let mut parts = admin_row(connected(), None, Some(admin_first(made())), true).into_parts();
    parts.repositories = List::with_capacity(1);
    parts
        .repositories
        .push(
            w::Adoption::new(
                &w::CEILINGS,
                w::AdoptionParts { owner: b"work".as_slice().into(), name: b"hello".as_slice().into(), project: 7 },
            )
            .unwrap(),
        )
        .unwrap();
    let root = admin_root(Some(Choice::Forge(HERE)), w::ForgeRow::new(&w::CEILINGS, parts).unwrap());
    let view = f::page_view(&root);
    words_in_order(
        view.tree(),
        &[
            "Connected as temper (id 42), managed.",
            "Made work/hello, and its project hello. Its CI runs on your forge’s runners labelled linux-arm; with none, a change’s checks wait.",
            "Open hello",
            "Open work/hello on Forgejo",
        ],
    );
    let link = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.id == node(view.tree(), Role::Link, "Open work/hello on Forgejo ↗".as_bytes()))
        .unwrap();
    assert_eq!(link.external.as_deref(), Some(b"https://git.example.org/work/hello".as_slice()));
    assert_eq!(press(&root, Role::Link, b"Open hello"), d::Action::Pages(pages::Act::OpenProject { project: 7 }));
}

#[test]
fn the_admin_card_keeps_the_whole_configured_web_address() {
    let mut parts = admin_row(w::Connection::NotConnected, None, None, false).into_parts();
    parts.web = Some(Box::from([b'a'; 2048].as_slice()));
    let root = admin_root(None, w::ForgeRow::new(&w::CEILINGS, parts).unwrap());
    let domain = f::limits();
    let words = f::words();
    let limits = crate::Limits::of(&domain, &words).unwrap();
    let mut view = crate::View::new(&limits, &domain, words, f::FORGE);
    f::build(&mut view, &root);
    let mut heading = b"Forgejo at ".to_vec();
    heading.extend_from_slice(&[b'a'; 2048]);
    node(view.tree(), Role::Heading { level: 2 }, &heading);
}

pub(super) fn pinned_rows(connection: w::Connection, pins: bool, runner: bool) -> List<w::ForgeRow> {
    let mut parts = row(HERE, w::Kind::Forgejo, w::Mode::Owned, connection, w::Offer::Offered).into_parts();
    if pins {
        parts.owned = Some(
            w::OwnedForge::new(
                &w::CEILINGS,
                w::OwnedForgeParts {
                    forgejo: b"17.0.0".as_slice().into(),
                    runner: runner.then(|| b"11.2.1".as_slice().into()),
                    password: b"/fake/first-password".as_slice().into(),
                },
            )
            .unwrap(),
        );
    }
    let mut rows = List::with_capacity(1);
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    rows
}

#[test]
fn owned_details_draw_the_forgejo_pin_and_keep_the_unpinned_words() {
    for (pins, runner) in [(true, true), (true, false), (false, false)] {
        let root = f::forge_root(
            Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
            pinned_rows(w::Connection::NotSetUp, pins, runner),
        );
        for view in [f::page_view(&root), f::browser_page_view(&root)] {
            let expected = if pins {
                "temper box downloads Forgejo 17.0.0 and its runner"
            } else {
                "temper box downloads Forgejo and its runner"
            };
            words_in_order(view.tree(), &[expected, "starts them, and makes your first repository"]);
        }
    }
}

#[test]
fn owned_fetch_steps_draw_both_pins_or_keep_the_unpinned_words() {
    for (pins, runner, fetching, fetched) in [
        (true, true, "Fetching Forgejo 17.0.0 and its runner 11.2.1", "Fetched Forgejo 17.0.0 and its runner 11.2.1"),
        (true, false, "Fetching Forgejo and its runner", "Fetched Forgejo and its runner"),
        (false, false, "Fetching Forgejo and its runner", "Fetched Forgejo and its runner"),
    ] {
        for (step, expected) in [(w::SetUpStep::Fetch, fetching), (w::SetUpStep::Prepare, fetched)] {
            let connection = w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(step) }).unwrap(),
            );
            let root = f::forge_root(
                Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
                pinned_rows(connection, pins, runner),
            );
            for view in [f::page_view(&root), f::browser_page_view(&root)] {
                words_in_order(
                    view.tree(),
                    &[expected, "Preparing", "Starting Forgejo and its runner", "Connecting", "Making"],
                );
            }
        }
    }
}

#[test]
fn owned_ready_offers_the_forgejo_link_after_the_project() {
    for facts_present in [true, false] {
        let root = f::forge_root(
            Address::ConnectForge { forge: Some(Choice::Forge(HERE)) },
            super::welcome::owned_adopted_rows(),
        );
        let mut view = f::view_of(f::browser_words());
        if facts_present {
            let mut facts = crate::forge_sign_in::Forges::new();
            facts.read("3 forgejo http://127.0.0.1:3000 owned");
            view.forge_sign_in(facts);
        }
        f::build(&mut view, &root);
        let link = f::find(view.tree(), Role::Link, "Open Forgejo ↗".as_bytes());
        assert_eq!(link.is_some(), facts_present);
        if let Some(link) = link {
            assert_eq!(
                crate::href(&mut view, link).as_deref(),
                Some(b"http://127.0.0.1:3000/user/oauth2/temper".as_slice())
            );
            words_in_order(view.tree(), &["Open Shop", "Open Forgejo ↗"]);
        }
    }
}

#[test]
fn the_connected_forge_and_repository_picker_offer_their_links_as_terminal_commands() {
    let mut view = f::page_view(&here(connected()));
    let control = view
        .tree()
        .nodes()
        .iter()
        .rev()
        .find(|node| node.role == Role::Link && node.name.as_deref() == Some(b"Forges"))
        .unwrap()
        .id;
    let (id, first) = f::terminal_command(
        view.tree(),
        &view.limits().tree,
        control,
        b"Forges",
        b"Forgejo on this machine is on the ",
    );
    assert_eq!(crate::act(&mut view, id, first), Some(d::Action::Pages(pages::Act::Go { address: Address::Forges })));

    let (mut root, _, token) = picker_started(false);
    answer(&mut root, token, picker_page(false, false));
    act(&mut root, d::forge::Act::Picker { forge: GITHUB, act: d::forge::picker::Act::Adopt });
    let view = f::page_view(&root);
    let label = b"Choose its repositories on GitHub";
    let disabled = node(view.tree(), Role::Button, label);
    assert!(view.tree().disabled(disabled));
    let mut print = skein_ui_text::Print::new(&view.limits().tree).unwrap();
    skein_ui_text::render(view.tree(), b"temper", None, &mut print);
    assert!(skein_lib::bytes::find(print.text.as_slice(), b"Choose its repositories on GitHub (disabled)\n").is_some());
    assert!(!print.numbers.as_slice().contains(&disabled));

    for empty in [false, true] {
        let (mut root, _, token) = picker_started(false);
        answer(&mut root, token, picker_page(empty, false));
        let mut view = f::page_view(&root);
        let label = b"Choose its repositories on GitHub";
        let control = node(view.tree(), Role::Link, label);
        let (id, first) = f::terminal_command(view.tree(), &view.limits().tree, control, label, b"Not listed? ");
        assert_eq!(
            crate::act(&mut view, id, first),
            Some(d::Action::Forge(d::forge::Act::Install {
                forge: GITHUB,
                back: d::address::print(root.pages().address()),
            }))
        );
    }
}
