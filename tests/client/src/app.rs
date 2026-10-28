//! The shell adapter observes requests and trees, never a domain's decisions.
use jig_client_world::{
    harness::{App, CardKey, Duty, Receipt, Seen, Sent, StoreWrite},
    observe,
};
use skein_lib::{Env, List, Queue, RequestEnvelope, Time};
use skein_ui::{Act, NodeId, Patch, Tree};
use temper_client_domain::{self as domain, Action, Address, Request};
use temper_client_view as view;

use crate::fixtures;
pub const LIVE: &[u8] = b"Live";
pub const OFFLINE: &[u8] = b"Offline";

/// temper's root and view on jig's common client world kit.
pub struct Temper;
/// Either the person's pages or the tools' scripted root.
pub enum Root {
    Person(Box<domain::Domain>),
    Agent(Box<crate::agent::Agent>),
}
impl Root {
    #[must_use]
    pub fn person(&self) -> &domain::Domain {
        match self {
            Self::Person(root) => root,
            Self::Agent(_) => panic!("agent has no person root"),
        }
    }
    #[must_use]
    pub fn agent(&self) -> &crate::agent::Agent {
        match self {
            Self::Agent(root) => root,
            Self::Person(_) => panic!("person has no agent root"),
        }
    }
}
pub enum View {
    Person(Box<view::View>),
    Agent { render: Box<skein_ui::View>, limits: skein_ui::Limits },
}
/// The shell world's Start carries its unlinked client's own typed records.
#[derive(Debug)]
pub enum Event {
    Root(domain::Event),
    Call { number: u32, call: temper_client_tools::jig::call::Call, json: bool, value: Option<skein_lib::Secret> },
    Start { address: Address, kept: List<skein_lib::RequestRecord<jig_client::Held>> },
}
impl From<domain::Event> for Event {
    fn from(event: domain::Event) -> Self {
        Self::Root(event)
    }
}
impl Event {
    #[must_use]
    pub fn root(&self) -> Option<&domain::Event> {
        match self {
            Self::Root(event) => Some(event),
            Self::Start { .. } | Self::Call { .. } => None,
        }
    }
}

/// The same small story capacities as ops's jig profile.
#[must_use]
pub fn limits() -> domain::Limits {
    fixtures::domain_limits()
}
impl App for Temper {
    type Root = Root;
    type View = View;
    type Limits = domain::Limits;
    type Event = Event;
    type Request = Request;
    type Record = skein_lib::RequestRecord<jig_client::Held>;
    type Key = [u8; 16];
    type Address = Address;
    fn limits() -> Self::Limits {
        limits()
    }
    fn new(seed: u64) -> (Self::Root, Self::View) {
        let limits = limits();
        let forge = fixtures::phrasebooks(&limits);
        let bounds = fixtures::view_bounds(&limits, &forge);
        let root = domain::unlinked(&limits, seed);
        (
            Root::Person(Box::new(root)),
            View::Person(Box::new(view::View::new(&bounds, &limits, forge, fixtures::FORGE))),
        )
    }
    fn view_limits(view: &Self::View) -> skein_ui::Limits {
        match view {
            View::Person(view) => view.limits().tree,
            View::Agent { limits, .. } => *limits,
        }
    }
    fn start(address: Address, kept: Vec<Self::Record>, _offset: i32) -> Event {
        let mut records = List::with_capacity(u32::try_from(kept.len()).expect("small store count"));
        for record in kept {
            records.push(record).expect("kept count measured");
        }
        Event::Start { address, kept: records }
    }
    fn went(address: Address) -> Event {
        domain::Event::Went { place: domain::Place::Address(address) }.into()
    }
    fn step(root: &mut Self::Root, env: &Env<Self::Limits>, event: Event, out: &mut Queue<Request>) {
        let Root::Person(root) = root else {
            let Root::Agent(agent) = root else { unreachable!() };
            crate::agent::step(agent, env, event, out);
            return;
        };
        match event {
            Event::Call { .. } => panic!("person received agent call"),
            Event::Root(event) => domain::step(root, env, event, out),
            Event::Start { address, kept } => {
                domain::step(
                    root,
                    env,
                    domain::Event::Client { event: jig_client::Event::StartUnlinked { kept } },
                    out,
                );
                domain::step(
                    root,
                    env,
                    domain::Event::Client { event: jig_client::Event::SignIn { with: jig_client::With::Transport } },
                    out,
                );
                domain::step(
                    root,
                    env,
                    domain::Event::Start { address, kept: List::with_capacity(0), with: None },
                    out,
                );
            }
        }
    }
    fn fire(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Request>) {
        match root {
            Root::Person(root) => domain::fire(root, env, out),
            Root::Agent(agent) => crate::agent::fire(agent, env, out),
        }
    }
    fn take(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Request>) {
        if let Root::Person(root) = root {
            domain::take(root, env, out);
        }
    }
    fn next_deadline(root: &Self::Root) -> Option<Time> {
        match root {
            Root::Person(root) => domain::next_deadline(root),
            Root::Agent(agent) => crate::agent::next_deadline(agent),
        }
    }
    fn build(view: &mut Self::View, root: &Self::Root, patches: &mut Queue<Patch>) {
        match (view, root) {
            (View::Person(view), Root::Person(root)) => view::build(view, root, fixtures::NOW, patches),
            (View::Agent { render, .. }, Root::Agent(agent)) => agent.build(render, patches),
            _ => panic!("root and view must belong to the same client"),
        }
    }
    fn tree(view: &Self::View) -> &Tree {
        match view {
            View::Person(view) => view.tree(),
            View::Agent { render, .. } => render.tree(),
        }
    }
    fn act(view: &mut Self::View, node: NodeId, act: Act) -> Option<Event> {
        match view {
            View::Person(view) => view::event(view, node, act).map(Into::into),
            View::Agent { .. } => None,
        }
    }
    fn duty(request: Request) -> Duty<Self> {
        match request {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Save { record } } } => {
                Duty::Store(StoreWrite::Save(record))
            }
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Erase { key } } } => {
                Duty::Store(StoreWrite::Erase(key))
            }
            Request::Client { request: jig_client::Request::SignOut } | Request::SignOut => Duty::SignOut,
            Request::Client { .. } => Duty::Wire(request),
            Request::Show { address, replace } => Duty::Show { address, replace },
            Request::Leave { .. } | Request::Fact { .. } | Request::Dropped { .. } => Duty::Fact,
        }
    }
    fn record_key(record: &Self::Record) -> Self::Key {
        record.key
    }
    fn unsent(request: Request) -> Option<Event> {
        let event = match request {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } } } => {
                jig_client::Event::Up {
                    up: jig_client::Up::Answer { attempt, envelope: RequestEnvelope::Lost, answer: None, why: None },
                }
            }
            Request::Client { request: jig_client::Request::Exchange { exchange, .. } } => {
                jig_client::Event::Lost { exchange }
            }
            Request::Client { request: jig_client::Request::OpenStream { .. } } => jig_client::Event::StreamGone,
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open { watch, .. }
                            | jig_client::Down::Close { watch }
                            | jig_client::Down::Continue { watch, .. },
                    },
            } => jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Dropped },
            },
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Read { read, .. } } } => {
                jig_client::Event::Up { up: jig_client::Up::ReadDone { read, result: jig_client::Readout::Limit } }
            }
            Request::Client {
                request:
                    jig_client::Request::Authorized(_)
                    | jig_client::Request::Save { .. }
                    | jig_client::Request::Erase { .. }
                    | jig_client::Request::CloseStream
                    | jig_client::Request::Told { .. }
                    | jig_client::Request::SignOut
                    | jig_client::Request::ServiceCredential { .. }
                    | jig_client::Request::Down { down: jig_client::Down::Save { .. } | jig_client::Down::Erase { .. } },
            }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Fact { .. }
            | Request::Dropped { .. } => return None,
        };
        Some(domain::Event::Client { event }.into())
    }
    fn max_out(limits: &Self::Limits) -> u32 {
        domain::max_out(limits) + 2 * temper_client_tools::limits::tools().rows
    }
    fn patch_limit(view: &Self::View) -> u32 {
        Self::view_limits(view).patches
    }
    fn worst_case(_limits: &Self::Limits) -> u64 {
        // No bound of temper's: the harness's heap_bytes_per_client bounds a client (programming-model.md 6.3); jig's provided method replaces this at the set's repin.
        u64::MAX
    }
    fn window_changed(event: &Event) -> bool {
        match event.root() {
            Some(domain::Event::Act { action: Action::Forge(_) }) => true,
            Some(domain::Event::Act { action: Action::Pages(act) }) => matches!(
                act,
                domain::pages::Act::Project { .. } | domain::pages::Act::Filter { .. } | domain::pages::Act::Go { .. }
            ),
            Some(
                domain::Event::Start { .. }
                | domain::Event::Went { .. }
                | domain::Event::Client { .. }
                | domain::Event::Forge(_)
                | domain::Event::SignedOut
                | domain::Event::AccountKey { .. }
                | domain::Event::SignInAddress { .. },
            )
            | None => false,
        }
    }
    fn seen(tree: &Tree) -> Vec<Seen> {
        let mut seen = crate::cards::seen(tree);
        if let Some((index, region)) =
            tree.nodes().iter().enumerate().find(|(_, node)| node.region == Some(view::frame::link_region()))
        {
            let end =
                index.checked_add(usize::try_from(region.size).expect("tree region fits")).expect("tree indices fit");
            for node in &tree.nodes()[index..end] {
                if node.role == skein_ui::Role::Status {
                    match node.name.as_deref() {
                        Some(LIVE) => seen.push(Seen::Link { live: true }),
                        Some(OFFLINE) => seen.push(Seen::Link { live: false }),
                        _ => {}
                    }
                }
            }
        }
        if adoption_page(tree) {
            let mut state = jig_client_world::harness::CardSeen::Waiting;
            for node in tree.nodes() {
                if node.role == skein_ui::Role::Button
                    && node.name.as_deref() == Some(b"Adopt")
                    && node.states.as_slice().contains(&skein_ui::State::Disabled)
                {
                    state = jig_client_world::harness::CardSeen::Deciding;
                }
                if node.role == skein_ui::Role::Alert
                    || node.role == skein_ui::Role::Status
                        && node.name.as_deref().is_some_and(|name| {
                            name.starts_with(b"Adopted ") || name.starts_with(b"It is not known whether ")
                        })
                {
                    state = jig_client_world::harness::CardSeen::Done;
                }
            }
            seen.push(Seen::Card { key: adoption_card(), state, revision: None });
        }
        seen
    }
    fn sent(request: &Request) -> Option<Sent> {
        let Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, key, request, .. } },
        } = request
        else {
            return None;
        };
        // Seen has no kept answer; match jig's client observer.
        Some(match request {
            jig_client::Ask::Jig(ask) if matches!(ask.as_ref(), jig_client::contract::Ask::Seen { .. }) => return None,
            jig_client::Ask::Family { .. } => {
                Sent { attempt: *attempt, key: *key, card: Some(adoption_card()), revision: None, words: None }
            }
            jig_client::Ask::Jig(ask) => {
                let mut sent = observe::jig_sent(*attempt, *key, ask);
                // Both inbox and plan releases use the held task's deciding card.
                if let jig_client::contract::Ask::Release { task, .. } = ask.as_ref() {
                    sent.card = Some(observe::key(jig_client::contract::ObjectKey::Escalation(*task)));
                }
                sent
            }
        })
    }
    fn decision(request: &Request) -> Option<&jig_client::contract::Ask> {
        match request {
            Request::Client {
                request:
                    jig_client::Request::Down { down: jig_client::Down::Send { request: jig_client::Ask::Jig(ask), .. } },
            } => Some(ask),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Fact { .. }
            | Request::Dropped { .. } => None,
        }
    }
    fn received(event: &Event) -> Vec<Receipt> {
        let Some(event) = event.root() else {
            return match event {
                Event::Start { .. } => vec![Receipt::Started],
                Event::Call { .. } => vec![],
                Event::Root(_) => unreachable!(),
            };
        };
        match event {
            domain::Event::Client { event: jig_client::Event::Up { up } } => {
                let mut receipts = observe::up_received(up);
                if let jig_client::Up::ReadDone {
                    result:
                        jig_client::Readout::Jig(jig_client::contract::ReadResult::Value(
                            jig_client::contract::ReadValue::Task(task),
                        )),
                    ..
                } = up
                    && let Some(hold) = &task.hold
                {
                    receipts.push(Receipt::Card {
                        key: observe::key(jig_client::contract::ObjectKey::Escalation(task.chip.task)),
                        revisions: vec![(2, hold.revision().value)],
                    });
                }
                receipts
            }
            domain::Event::SignedOut => vec![Receipt::Parked],
            domain::Event::Start { .. }
            | domain::Event::Went { .. }
            | domain::Event::Act { .. }
            | domain::Event::Forge(_)
            | domain::Event::AccountKey { .. }
            | domain::Event::SignInAddress { .. }
            | domain::Event::Client { .. } => vec![],
        }
    }
    fn card(tree: &Tree, node: NodeId) -> Option<CardKey> {
        if adoption_page(tree)
            && tree.nodes().iter().any(|value| {
                value.id == node
                    && value.binding.is_some_and(|binding| {
                        binding.owner == temper_client_domain::forge::words::owner(fixtures::FORGE)
                    })
            })
        {
            Some(adoption_card())
        } else {
            crate::cards::enclosing(tree, node)
        }
    }
}
fn adoption_card() -> CardKey {
    CardKey(b"forge/adoption".to_vec())
}
fn adoption_page(tree: &Tree) -> bool {
    tree.nodes().iter().any(|node| {
        node.role == skein_ui::Role::Heading { level: 1 }
            && node.text.as_slice().iter().any(|run| run.text.starts_with(b"Adopt a repository into "))
    })
}
