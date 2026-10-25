use jig_client::contract;
use jig_client_protocol_link as link;
use skein_lib::{RequestRecord, Token};
use temper_client_domain as client;
pub const NOW: skein_lib::Wall = skein_lib::Wall::EPOCH;
pub const ATTEMPT: Token = Token::new(101);
pub const READ: Token = Token::new(102);
pub const SERVICE: u64 = 7;
pub fn credential() -> skein_lib::Secret {
    skein_lib::Secret::new(Box::from(CREDENTIAL))
}
pub const CREDENTIAL: &[u8] = b"one-time-service-credential";
pub fn table_record() -> RequestRecord<jig_client::Held> {
    RequestRecord {
        key: [9; 16],
        scope: SERVICE,
        first_sent: None,
        request: jig_client::Held {
            key: jig_client::Key { asker: jig_client::Asker::JIG, number: ATTEMPT.raw() },
            request: jig_client::Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 9 })),
        },
    }
}
pub fn link_limits() -> link::Limits {
    let native = crate::limits::native();
    link::Limits::new(native.client.client.domain, native.client.client.exchanges).expect("link bounds")
}
pub fn inbox_read() -> contract::Read {
    contract::Read::Inbox { project: None, group: contract::InboxReadGroup::Needs, kind: None, goal: None, after: None }
}
pub fn wire_limits() -> temper_client_protocol_wire::v1::Limits {
    client::forge::wire_limits(&crate::limits::native().client.forge).expect("forge wire bounds")
}
pub fn adoption() -> client::forge::Adoption {
    client::forge::Adoption {
        project: 1,
        forge: 1,
        owner: Box::from(*b"acme"),
        name: Box::from(*b"shop"),
        prefix: Box::from(*b"temper/"),
        landing: Box::from(*b"main"),
        role: client::forge::Role::Owned,
        home: true,
        ci: true,
        checks: Box::from([1, 2]),
    }
}
pub fn adopted(protection: client::forge::Protection) -> client::forge::Adopted {
    client::forge::Adopted {
        forge: 1,
        repository: 7,
        owner: Box::from(*b"acme"),
        name: Box::from(*b"shop"),
        role: client::forge::Role::Owned,
        home: true,
        kinds: client::forge::Kinds {
            read: true,
            push: false,
            open: true,
            land: false,
            review: true,
            status: false,
            comment: true,
            issue: false,
            branch: true,
        },
        protection,
    }
}
pub fn role_refusal() -> contract::Refusal {
    contract::Refusal::Role { held: None }
}
pub fn core_refusal() -> Box<[u8]> {
    use jig_client_protocol_wire as w;
    let limits = link_limits().wire;
    let value = w::Refusal::Role(w::RefusalRole::new(&limits, w::RefusalRoleParts { held: None }).expect("role fits"));
    let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("measure fits"));
    value.encode(&mut writer).expect("measured room");
    writer.finish()
}

pub fn tree_limits() -> skein_ui::Limits {
    skein_ui::Limits {
        nodes: 256,
        depth: 32,
        text_per_node: 1024,
        text_bytes: 65_536,
        drawing_bytes: 16,
        scene_marks: 0,
        scene_bytes: 0,
        styles_per_node: 2,
        patches: 1280,
        bindings: 256,
        slots: 64,
        markdown_depth: 3,
    }
}
pub(crate) fn print(tree: &skein_ui::Tree) -> skein_ui_text::Print {
    let mut print = skein_ui_text::Print::new(&tree_limits()).expect("fixture print bounds");
    skein_ui_text::render(tree, b"temper", Some(temper_client_view::frame::link_region()), &mut print);
    print
}
pub fn golden(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/golden").join(format!("{name}.txt"))
}
pub fn adoption_form(filled: bool) -> client::forge::Form {
    let mut form = client::forge::form(1);
    form.confirmed = true;
    form.served = client::forge::Served::Yes;
    if filled {
        for (field, text) in [
            (client::forge::Text::Owner, b"acme".as_slice()),
            (client::forge::Text::Name, b"shop"),
            (client::forge::Text::Landing, b"main"),
        ] {
            form.texts[field.index()].text = text.into();
        }
    }
    form
}
pub fn adoption_page(form: &client::forge::Form, words: &temper_client_view::words::Words) -> skein_ui::Tree {
    let limits = tree_limits();
    let forge = words;
    let mut builder = skein_ui::Builder::new(&limits);
    synthetic_header(&mut builder, None, false);
    let mut bindings = temper_client_view::forge::bindings::Bindings::new(limits.bindings, 8, 1);
    builder.open(skein_ui::Role::Main);
    temper_client_view::forge::page::page(
        &mut builder,
        form,
        Some(b"shop"),
        forge,
        &mut bindings,
        limits.text_per_node,
    );
    builder.close();
    builder.close();
    visible(builder.finish())
}
pub fn visible(mut tree: skein_ui::Tree) -> skein_ui::Tree {
    let limits = tree_limits();
    skein_ui::diff(
        &skein_ui::Tree::new(limits.nodes),
        &mut tree,
        &limits,
        &mut 0,
        &mut skein_lib::Queue::with_capacity(limits.patches),
    );
    tree
}
pub fn copy_tree(source: &skein_ui::Tree) -> skein_ui::Tree {
    let mut builder = skein_ui::Builder::new(&tree_limits());
    let mut open = Vec::new();
    for (index, node) in source.nodes().iter().enumerate() {
        while open.last().is_some_and(|end| *end == index) {
            builder.close();
            open.pop();
        }
        builder.open(node.role);
        if let Some(region) = node.region {
            builder.region(region);
        }
        if let Some(name) = &node.name {
            builder.name(name);
        }
        if let Some(description) = &node.description {
            builder.description(description);
        }
        for state in &node.states {
            builder.state(*state);
        }
        for run in &node.text {
            builder.run(run.run, &run.text);
        }
        if let Some(value) = &node.value {
            builder.value(&value.text, value.written);
        }
        if let Some(binding) = node.binding {
            builder.bind(binding);
        }
        if let Some(link) = node.link {
            builder.link(link);
        }
        if let Some(external) = &node.external {
            builder.external(external);
        }
        open.push(index + usize::try_from(node.size).expect("subtree fits"));
    }
    for _ in open {
        builder.close();
    }
    visible(builder.finish())
}
pub fn page(address: &temper_client_domain::Address, signed_out: bool) -> skein_ui::Tree {
    use skein_lib::{Env, Queue};
    use temper_client_domain::Event;
    let mut limits = crate::limits::native().client;
    limits.client.domain.text = 32;
    limits.client.domain.turn_text = 32;
    limits.client.domain.rows = 8;
    limits.client.domain.document_bytes = 16_384;
    limits.client.domain.requests.bytes = 4096;
    let env = Env { now: skein_lib::Time::ZERO, wall: NOW, limits };
    let forge = page_words();
    let mut view =
        temper_client_view::View::new(&temper_client_view::Limits { tree: tree_limits() }, &limits, forge, 1);
    let mut root = client::unlinked(&limits, 16);
    let mut out = Queue::with_capacity(client::max_out(&limits));
    client::step(
        &mut root,
        &env,
        Event::Start { address: address.clone(), kept: skein_lib::List::with_capacity(0), with: None },
        &mut out,
    );
    for event in [
        jig_client::Event::StartUnlinked { kept: skein_lib::List::with_capacity(0) },
        jig_client::Event::SignIn { with: jig_client::With::Transport },
    ] {
        client::step(&mut root, &env, Event::Client { event }, &mut out);
    }
    if signed_out {
        client::step(&mut root, &env, Event::SignedOut, &mut out);
    } else {
        client::step(
            &mut root,
            &env,
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } },
            },
            &mut out,
        );
        populate_page(&mut root, &env, address, &mut out);
        assert!(out.is_empty(), "fixture settles within its step budget: {out:?}");
    }
    let settled = Env { now: env.now.saturating_add(skein_lib::Duration::ZERO), ..env };
    client::fire(&mut root, &settled, &mut out);
    temper_client_view::build(&mut view, &root, NOW, &mut Queue::with_capacity(tree_limits().patches));
    copy_tree(view.tree())
}
fn populate_page(
    root: &mut client::Domain,
    env: &skein_lib::Env<client::Limits>,
    address: &temper_client_domain::Address,
    out: &mut skein_lib::Queue<client::Request>,
) {
    use jig_client::contract::{Delivery, ReadResult, Subject, WatchEvent};
    use temper_client_domain::{Event, Request};
    for _ in 0..128 {
        let Some(request) = out.pop() else {
            break;
        };
        match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(what) },
                    },
            } => {
                let value = read_value(&what);
                if let Some(value) = value {
                    let result = ReadResult::Value(value);
                    client::step(
                        root,
                        env,
                        Event::Client {
                            event: jig_client::Event::Up {
                                up: jig_client::Up::ReadDone { read, result: jig_client::Readout::Jig(result) },
                            },
                        },
                        out,
                    );
                }
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(subject) }
                            | jig_client::Down::Continue { watch, subject },
                    },
            } => {
                client::step(
                    root,
                    env,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::Watched { watch, event: WatchEvent::Opened },
                        },
                    },
                    out,
                );
                let delivery = match subject {
                    Subject::Settings => unreachable!("temper does not watch settings"),
                    Subject::Me => Some(Delivery::Me(Box::new(jig_client_world::stories::frame(1)))),
                    Subject::Task { .. } => Some(Delivery::Task(Box::new(jig_client_world::stories::task()))),
                    Subject::Deployment => {
                        Some(Delivery::Deployment(Box::new(jig_client_world::stories::deployment())))
                    }
                    Subject::Tree { .. }
                    | Subject::Chips { .. }
                    | Subject::Inbox { .. }
                    | Subject::Tasks { .. }
                    | Subject::Run { .. }
                    | Subject::Accounts => None,
                };
                if let Some(delivery) = delivery {
                    client::step(
                        root,
                        env,
                        Event::Client {
                            event: jig_client::Event::Up {
                                up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(delivery) },
                            },
                        },
                        out,
                    );
                }
            }
            Request::Show { address: next, .. } => {
                if &next != address {
                    client::step(root, env, Event::Went { place: client::Place::Address(next) }, out);
                }
            }
            Request::Client { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Fact { .. }
            | Request::Dropped { .. } => {}
        }
    }
}

/// A chatbar's own spend and allotted budget.
pub fn spend(spent: u64, budget: u64, _below: u64) -> Box<[u8]> {
    use skein_ui::phrases::Hole::Number;
    page_words().say(temper_client_view::words::Word::ChatbarSpend, &[Number(spent), Number(budget)])
}
pub fn page_words() -> temper_client_view::words::Words {
    static WORDS: std::sync::OnceLock<temper_client_view::words::Words> = std::sync::OnceLock::new();
    WORDS
        .get_or_init(|| {
            temper_client_view::words::Words::english(
                temper_client_protocol_wire::FORGE,
                32,
                temper_client_view::words::Shell::Terminal,
            )
            .expect("temper words fit")
        })
        .clone()
}

pub fn inbox_document(group: contract::InboxReadGroup) -> contract::InboxPage {
    use contract::{InboxEntry, InboxGroup, ObjectKey};
    let mut rows = skein_lib::List::with_capacity(6);
    for (kind, object) in [
        (InboxGroup::Questions, ObjectKey::Question { task: 9, message: 1 }),
        (
            InboxGroup::Proposals,
            ObjectKey::Proposal(contract::ProposalKey { proposer: contract::Proposer::Task(9), number: 1 }),
        ),
        (InboxGroup::Escalations, ObjectKey::Escalation(9)),
        (InboxGroup::PersonTasks, ObjectKey::PersonTask(9)),
        (InboxGroup::Results, ObjectKey::Result(9)),
        (InboxGroup::Replies, ObjectKey::Task(10)),
    ] {
        if contract::InboxReadGroup::of(kind) == group {
            rows.push(InboxEntry {
                reply: None,
                lineage: skein_lib::List::with_capacity(0),
                goal: None,
                recipient: None,
                stall: None,
                object,
                project: 1,
                group: kind,
                position: 1,
                at: skein_lib::Wall::EPOCH,
            })
            .expect("six inbox groups");
        }
    }
    contract::InboxPage { cut: false, rows, before: None, after: None }
}
pub fn inbox_page() -> skein_ui::Tree {
    use skein_ui::{Role, Run};
    let mut builder = skein_ui::Builder::new(&tree_limits());
    synthetic_frame(&mut builder, false);
    builder.open(Role::Main);
    builder.open(Role::Heading { level: 1 });
    builder.text(Run::Plain, b"Inbox");
    builder.close();
    builder.open(Role::Link);
    builder.name(&page_words().say(temper_client_view::words::Word::NewChatLink, &[]));
    builder.link(binding());
    builder.close();
    for name in [b"All projects".as_slice(), b"Shop", b"Second", b"All goals"] {
        builder.open(Role::Button);
        builder.name(name);
        builder.bind(binding());
        builder.close();
    }
    builder.open(Role::Disclosure);
    builder.name(b"Kind");
    for name in [b"All kinds".as_slice(), b"Proposal", b"Question", b"Held", b"Person task"] {
        builder.open(Role::Button);
        builder.name(name);
        builder.bind(binding());
        builder.close();
    }
    builder.close();
    builder.open(Role::TextField);
    builder.name(b"Goal");
    builder.value(b"", 0);
    builder.bind(binding());
    builder.close();
    for (heading, cards) in [(b"Needs you".as_slice(), &INBOX_CARDS[..4]), (b"Updates".as_slice(), &INBOX_CARDS[4..])] {
        builder.open(Role::Heading { level: 2 });
        builder.text(Run::Plain, heading);
        builder.close();
        builder.open(Role::Paragraph);
        builder.text(Run::Plain, if heading == b"Needs you" { b"4" } else { b"2 new" });
        builder.close();
        for (kind, body) in cards {
            builder.open(Role::Item);
            if heading == b"Needs you" {
                builder.open(Role::Paragraph);
                builder.text(Run::Plain, kind);
                builder.close();
            }
            builder.open(Role::Paragraph);
            builder.text(Run::Plain, b"Project 1");
            builder.close();
            builder.open(Role::Link);
            builder.name(b"Task 9");
            builder.link(binding());
            builder.close();
            // These synthetic records all carry the fixture's epoch time.
            let (time, _) = page_words().time(
                if heading == b"Needs you" {
                    temper_client_view::words::TimeForm::Span
                } else {
                    temper_client_view::words::TimeForm::Ago
                },
                NOW,
                NOW,
            );
            let time = if heading == b"Needs you" {
                page_words().say(temper_client_view::words::Word::TimeWaiting, &[skein_ui::phrases::Hole::Name(time)])
            } else {
                time
            };
            if heading == b"Needs you" {
                builder.open(Role::Paragraph);
                builder.text(Run::Plain, &time);
                builder.close();
            }
            builder.open(Role::Heading { level: 3 });
            builder.text(Run::Plain, body);
            builder.close();
            if *kind == b"Proposal" {
                golden_cards::proposal(&mut builder);
            }
            if heading == b"Updates" {
                builder.open(Role::Paragraph);
                builder.text(Run::Plain, &time);
                builder.close();
            }
            if *kind == b"Held" {
                golden_cards::held(&mut builder);
            }
            builder.close();
        }
    }
    builder.close();
    builder.close();
    visible(builder.finish())
}
const INBOX_CARDS: [(&[u8], &[u8]); 6] = [
    (b"Proposal", b"Add OAuth login"),
    (b"Question", b"Which provider first?"),
    (b"Held", b"Review needs attention"),
    (b"Person task", b"Choose a provider"),
    (b"Updates", b"T9 Accessibility pass is done"),
    (b"Reply", b"T9 OAuth login? replied"),
];
pub fn task_page() -> skein_ui::Tree {
    task::page()
}
pub fn agent_task_page() -> skein_ui::Tree {
    task::agent_task_page()
}
pub fn signed_out_page() -> skein_ui::Tree {
    page(&temper_client_domain::Address::Inbox { filter: temper_client_domain::address::Filter::ALL }, true)
}

fn synthetic_frame(builder: &mut skein_ui::Builder, behind: bool) {
    use skein_ui::{Role, Run};
    builder.open(Role::Page);
    builder.open(Role::Banner);
    builder.open(Role::Paragraph);
    builder.text(Run::Plain, b"temper");
    builder.close();
    builder.open(Role::Disclosure);
    builder.name(b"Menu");
    builder.open(Role::Button);
    builder.name(b"Shop");
    builder.state(skein_ui::State::Pressed);
    builder.bind(binding());
    builder.close();
    builder.open(Role::Navigation);
    for name in [b"Inbox".as_slice(), b"Chats", b"Adopt a repository"] {
        builder.open(Role::Link);
        builder.name(name);
        builder.link(binding());
        builder.close();
    }
    builder.close();
    builder.open(Role::Region);
    builder.region(temper_client_view::frame::link_region());
    builder.open(Role::Status);
    builder.text(Run::Plain, b"Live");
    builder.close();
    if behind {
        builder.open(Role::Status);
        builder.text(Run::Plain, b"Behind: waiting for current facts");
        builder.close();
    }
    builder.close();
    builder.open(Role::Status);
    builder.text(Run::Plain, b"Pat");
    builder.close();
    builder.open(Role::Button);
    builder.name(b"Sign out");
    builder.bind(binding());
    builder.close();
    builder.close();
    builder.close();
}
pub fn synthetic_header(builder: &mut skein_ui::Builder, title: Option<&[u8]>, behind: bool) {
    use skein_ui::{Role, Run};
    synthetic_frame(builder, behind);
    builder.open(Role::Main);
    if let Some(title) = title {
        builder.open(Role::Heading { level: 1 });
        builder.text(Run::Plain, title);
        builder.close();
    }
    builder.close();
}
pub fn header_page(title: Option<&[u8]>, behind: bool) -> skein_ui::Tree {
    let mut builder = skein_ui::Builder::new(&tree_limits());
    synthetic_header(&mut builder, title, behind);
    builder.close();
    visible(builder.finish())
}
pub fn binding() -> skein_ui::Binding {
    skein_ui::Binding { owner: temper_client_view::frame::owner(), token: Token::new(1) }
}
pub fn every_role() -> skein_ui::Tree {
    use skein_ui::{Role, Run};
    let mut builder = skein_ui::Builder::new(&tree_limits());
    synthetic_header(&mut builder, Some(b"Every role"), false);
    for (role, label) in [
        (Role::Region, b"Region".as_slice()),
        (Role::Figure, b"Figure"),
        (Role::Dialog, b"Dialog"),
        (Role::List, b"List"),
        (Role::Tree, b"Tree"),
        (Role::ChoiceGroup, b"Choices"),
        (Role::Tabs, b"Tabs"),
        (Role::Log, b"Log"),
    ] {
        builder.open(role);
        builder.name(label);
        builder.text(Run::Plain, b"child");
        builder.close();
    }
    for (role, text) in [
        (Role::Heading { level: 2 }, b"Heading".as_slice()),
        (Role::Paragraph, b"Paragraph"),
        (Role::Time, b"Time"),
        (Role::Cell, b"Cell"),
        (Role::Term, b"Term"),
        (Role::Description, b"Description"),
        (Role::Alert, b"Alert"),
        (Role::Code, b"code\nsecond"),
        (Role::Quote, b"quote\nsecond"),
        (Role::Status, b"Status"),
    ] {
        builder.open(role);
        builder.text(Run::Plain, text);
        builder.close();
    }
    builder.text(Run::Plain, b"Text");
    builder.open(Role::Text);
    builder.run(Run::Code, b"run");
    builder.run(Run::Strong, b" strong");
    builder.close();
    for role in [Role::Item, Role::TreeItem] {
        builder.open(role);
        builder.open(Role::Paragraph);
        builder.text(Run::Plain, b"item\ncontinued");
        builder.close();
        builder.close();
    }
    builder.open(Role::Table);
    builder.open(Role::Row);
    for text in [b"one".as_slice(), b"two"] {
        builder.open(Role::Cell);
        builder.text(Run::Plain, text);
        builder.close();
    }
    builder.close();
    builder.close();
    role_controls(&mut builder);
    builder.close();
    visible(builder.finish())
}
fn role_controls(builder: &mut skein_ui::Builder) {
    use skein_ui::{Role, Run, State};
    for (role, name) in [
        (Role::Button, b"Button".as_slice()),
        (Role::Tab, b"Tab"),
        (Role::Link, b"Link"),
        (Role::TextField, b"Field"),
        (Role::TextArea, b"Area"),
        (Role::Choice, b"Choice"),
        (Role::Check, b"Check"),
    ] {
        builder.open(role);
        builder.name(name);
        builder.bind(binding());
        if role == Role::TextField {
            builder.value(b"value", 1);
            builder.state(State::SubmitsOnEnter);
        }
        if role == Role::TextArea {
            builder.value(b"", 1);
        }
        builder.close();
    }
    for (role, name, state) in
        [(Role::Choice, b"Chosen".as_slice(), State::Selected), (Role::Check, b"Checked", State::Pressed)]
    {
        builder.open(role);
        builder.name(name);
        builder.state(state);
        builder.bind(binding());
        builder.close();
    }
    builder.open(Role::Button);
    builder.name(b"All states");
    builder.bind(binding());
    for state in
        [State::Selected, State::Pressed, State::Invalid, State::Busy, State::Current, State::Expanded, State::Disabled]
    {
        builder.state(state);
    }
    builder.close();
    builder.open(Role::Button);
    builder.name(b"Unbound");
    builder.close();
    builder.open(Role::Link);
    builder.text(Run::Plain, b"Outside");
    builder.external(b"https://example.test/");
    builder.close();
    builder.open(Role::Status);
    builder.name(b"Loading");
    builder.close();
    builder.open(Role::Status);
    builder.close();
    builder.open(Role::Progress);
    builder.name(b"Progress");
    builder.text(Run::Plain, b"half");
    builder.close();
    builder.open(Role::Separator);
    builder.close();
    builder.open(Role::Slot);
    builder.text(Run::Plain, b"Slot");
    builder.close();
    builder.drawing(
        skein_ui::DrawingKind { owner: temper_client_view::frame::owner(), kind: 1 },
        b"Drawing",
        b"image",
        16,
    );
    builder.text(Run::Plain, b"fallback");
    builder.close();
    builder.open(Role::Paragraph);
    builder.text(Run::Plain, b"before");
    builder.open(Role::Link);
    builder.text(Run::Plain, b"Nested");
    builder.bind(binding());
    builder.close();
    builder.text(Run::Plain, b"after");
    builder.close();
}
pub fn control_text() -> skein_ui::Tree {
    use skein_ui::{Role, Run};
    let mut builder = skein_ui::Builder::new(&tree_limits());
    synthetic_header(&mut builder, Some(b"Text\tchecked"), false);
    builder.open(Role::Button);
    builder.name(b"Label");
    builder.text(Run::Plain, b"Label");
    builder.bind(binding());
    builder.close();
    builder.open(Role::Paragraph);
    builder.text(Run::Plain, b"a\t b\nnext\x1b\0\x7f\xc2\x80\xff\xc0\xaf\xe2\x82");
    builder.close();
    builder.open(Role::Text);
    builder.run(Run::Emphasis, "café ".as_bytes());
    builder.run(Run::Code, b"x\ty\nz");
    builder.close();
    builder.close();
    visible(builder.finish())
}
fn read_value(what: &contract::Read) -> Option<contract::ReadValue> {
    use contract::{Read, ReadValue};
    match what {
        Read::Settings | Read::Setting(_) | Read::Revision(_) | Read::Revisions(_) => {
            unreachable!("temper does not read settings")
        }
        Read::Me => Some(ReadValue::Me(Box::new(jig_client_world::stories::frame(1)))),
        Read::Task { .. } => Some(ReadValue::Task(Box::new(jig_client_world::stories::task()))),
        Read::Project { .. } => Some(ReadValue::Project(Box::new(jig_client_world::stories::project()))),
        Read::Deployment => Some(ReadValue::Deployment(Box::new(jig_client_world::stories::deployment()))),
        Read::Proposal { .. } => Some(ReadValue::Proposal(Box::new(jig_client_world::stories::proposal()))),
        Read::Inbox { group, .. } => Some(ReadValue::Inbox(Box::new(inbox_document(*group)))),
        Read::Activity { .. } => Some(ReadValue::Activity(Box::new(activity_document()))),
        Read::Turn { .. }
        | Read::Escalation { .. }
        | Read::Question { .. }
        | Read::PersonTask { .. }
        | Read::Result { .. }
        | Read::Ended { .. }
        | Read::History { .. }
        | Read::Spend { .. }
        | Read::Tasks { .. }
        | Read::Notes { .. } => None,
    }
}
fn activity_document() -> contract::ActivityPage {
    let mut rows = skein_lib::List::with_capacity(1);
    rows.push(contract::ActivityLine {
        position: 1,
        at: skein_lib::Wall::EPOCH,
        event: contract::Activity::Words {
            by: jig_client_world::stories::party(1),
            message: 1,
            words: jig_client_world::stories::text(b"A fixture conversation"),
        },
    })
    .expect("one conversation entry");
    contract::ActivityPage { cut: false, rows, before: None, after: None }
}

pub fn native() -> crate::client::NativeClient {
    crate::client::NativeClient::new(
        crate::client::Settings {
            engine: skein_io::Addr::Ip("127.0.0.1:7100".parse().expect("loopback")),
            directory: Box::from(*b"client"),
            limits: crate::limits::native(),
        },
        skein_io::kernel::Fd::new(100),
        1000,
        7,
        crate::client::Start::Token,
    )
    .expect("native client")
}
pub fn service_answer() -> Box<[u8]> {
    use jig_client_protocol_wire as w;
    let limits = link_limits().wire;
    let service = w::MadeService::new(&limits, w::MadeServiceParts { party: SERVICE, credential: credential() })
        .expect("service fits");
    let made =
        w::OutcomeMade::new(&limits, w::OutcomeMadeParts { what: w::Made::Service(service) }).expect("made fits");
    let done =
        w::Final::Done(w::Done::new(&limits, w::DoneParts { outcome: w::Outcome::Made(made) }).expect("done fits"));
    let mut bytes = skein_lib::Writer::new(usize::try_from(done.measure()).expect("record fits"));
    done.encode(&mut bytes).expect("measured final");
    let owned =
        w::Owned::new(&limits, w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: bytes.finish() })
            .expect("owned fits");
    let answer = w::KeyedAnswer::FinalAnswer(owned);
    let mut bytes = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer fits"));
    answer.encode(&mut bytes).expect("measured answer");
    bytes.finish()
}

pub fn chats_page() -> skein_ui::Tree {
    use skein_ui::{Role, Run};
    let mut b = skein_ui::Builder::new(&tree_limits());
    synthetic_frame(&mut b, false);
    b.open(Role::Main);
    b.open(Role::Heading { level: 1 });
    b.text(Run::Plain, b"Chats");
    b.close();
    b.open(Role::TextArea);
    b.name(b"Start a chat");
    b.bind(binding());
    b.close();
    b.open(Role::Button);
    b.name(b"Start chat");
    b.bind(binding());
    b.close();
    b.open(Role::Heading { level: 2 });
    b.text(Run::Plain, b"Your chats");
    b.close();
    for label in [b"All".as_slice(), b"Live", b"Closed"] {
        b.open(Role::Button);
        b.name(label);
        if label == b"All" {
            b.state(skein_ui::State::Pressed);
        }
        b.bind(binding());
        b.close();
    }
    b.open(Role::List);
    b.name(b"Your chats");
    for (number, title, status, spent, delegates) in [
        (
            b"T9".as_slice(),
            b"Hello".as_slice(),
            b"Writing".as_slice(),
            page_words().say(temper_client_view::words::Word::RowSpent, &[skein_ui::phrases::Hole::Number(7)]),
            Some(b"1 delegate".as_slice()),
        ),
        (
            b"T8".as_slice(),
            b"Earlier".as_slice(),
            b"Closed".as_slice(),
            page_words().say(temper_client_view::words::Word::RowSpent, &[skein_ui::phrases::Hole::Number(4)]),
            None,
        ),
    ] {
        b.open(Role::Item);
        b.open(Role::Slot);
        b.text(Run::Plain, "Shop · ".as_bytes());
        b.open(Role::Link);
        b.name(number);
        b.bind(binding());
        b.close();
        b.close();
        for (role, value) in [(Role::Paragraph, title), (Role::Status, status)] {
            b.open(role);
            b.text(Run::Plain, value);
            b.close();
        }
        b.open(Role::Paragraph);
        b.text(Run::Plain, &spent);
        if let Some(delegates) = delegates {
            b.text(Run::Plain, " · ".as_bytes());
            b.text(Run::Plain, delegates);
        }
        b.close();
        b.open(Role::Paragraph);
        b.text(Run::Plain, &page_words().time(temper_client_view::words::TimeForm::Ago, NOW, NOW).0);
        b.close();
        b.close();
    }
    b.close();
    b.close();
    b.close();
    b.finish()
}

pub mod chat;

mod task;

pub mod golden_cards;
