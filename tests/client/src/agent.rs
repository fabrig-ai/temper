//! The tools' root as a scripted client beside the person.
use crate::app::{Event, Root, View};
use skein_lib::{Env, Queue, Time};
use std::collections::{BTreeMap, BTreeSet};
use temper_client_domain::{self as domain, Request};
use temper_client_tools::{jig::Printed, root};

pub struct Agent {
    root: root::Root,
    ready: bool,
    kept_lines: Vec<Box<[u8]>>,
    progress: Vec<Box<[u8]>>,
    printed: BTreeMap<u32, Printed>,
    calls: u32,
    release_tasks: BTreeSet<u64>,
    shown_revision: u64,
}
impl Agent {
    pub fn build(&self, render: &mut skein_ui::View, out: &mut Queue<skein_ui::Patch>) {
        use jig_client::Link;
        use skein_ui::Role;
        let (count, name) = match self.root.client().link().state {
            Link::Live => (0, Some(crate::app::LIVE)),
            Link::Offline => (1, Some(crate::app::OFFLINE)),
            Link::Starting => (2, None),
            Link::SignedOut => (3, None),
            Link::OutOfDate => (4, None),
        };
        let count = count + self.shown_revision * 5;
        if !render.stale(count) {
            return;
        }
        let mut builder = render.builder();
        builder.open(Role::Page);
        builder.open(Role::Banner);
        builder.open(Role::Region);
        builder.region(temper_client_view::frame::link_region());
        if let Some(name) = name {
            builder.open(Role::Status);
            builder.name(name);
            builder.close();
        }
        builder.close();
        builder.close();
        builder.open(Role::Main);
        for task in &self.release_tasks {
            builder.open(Role::Item);
            builder.key(skein_ui::Key { owner: temper_client_view::frame::owner(), kind: 2, a: *task, b: 0 });
            builder.close();
        }
        builder.close();
        builder.close();
        render.finish(builder, count, out);
    }
    #[must_use]
    pub fn ready(&self) -> bool {
        self.ready
    }
    #[must_use]
    pub fn kept(&self) -> u32 {
        self.root.kept()
    }
    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls
    }
    #[must_use]
    pub fn kept_lines(&self) -> &[Box<[u8]>] {
        &self.kept_lines
    }
    #[must_use]
    pub fn printed(&self, number: u32) -> Option<&Printed> {
        self.printed.get(&number)
    }
}
#[must_use]
pub fn limits() -> root::Limits {
    root::Limits::new(temper_client_tools::limits::client(), temper_client_tools::limits::tools())
}
#[must_use]
pub fn new(seed: u64) -> (Root, View) {
    let limits = limits();
    let presentation = crate::fixtures::domain_limits();
    let words = crate::fixtures::phrasebooks(&presentation);
    let tree_limits = crate::fixtures::view_bounds(&presentation, &words).tree;
    (
        Root::Agent(Box::new(Agent {
            root: root::Root::unlinked(&limits, seed).expect("tools preset"),
            ready: false,
            kept_lines: Vec::new(),
            progress: Vec::new(),
            printed: BTreeMap::new(),
            calls: 0,
            release_tasks: BTreeSet::new(),
            shown_revision: 0,
        })),
        View::Agent { render: Box::new(skein_ui::View::new(&tree_limits)), limits: tree_limits },
    )
}
fn environment(env: &Env<domain::Limits>) -> Env<root::Limits> {
    Env { now: env.now, wall: env.wall, limits: root::Limits::new(env.limits, temper_client_tools::limits::tools()) }
}
pub fn step(agent: &mut Agent, env: &Env<domain::Limits>, event: Event, out: &mut Queue<Request>) {
    let env = environment(env);
    let mut requests =
        Queue::with_capacity(<crate::app::Temper as jig_client_world::harness::App>::max_out(&env.limits.domain));
    match event {
        Event::Start { kept, .. } => {
            root::step(
                &mut agent.root,
                &env,
                root::Event::Client { event: jig_client::Event::StartUnlinked { kept } },
                &mut requests,
            );
            root::step(
                &mut agent.root,
                &env,
                root::Event::Client { event: jig_client::Event::SignIn { with: jig_client::With::Transport } },
                &mut requests,
            );
        }
        Event::Root(domain::Event::Client { event }) => {
            root::step(&mut agent.root, &env, root::Event::Client { event }, &mut requests);
        }
        Event::Root(domain::Event::Went { .. }) => {}
        Event::Root(event) => panic!("agent received person event: {event:?}"),
        Event::Call { number, call, json, value } => {
            agent.calls = number;
            root::step(&mut agent.root, &env, root::Event::Call { number, call, json, value }, &mut requests);
        }
    }
    drain(agent, &mut requests, out);
}
fn drain(agent: &mut Agent, requests: &mut Queue<root::Request>, out: &mut Queue<Request>) {
    while let Some(request) = requests.pop() {
        match request {
            temper_client_tools::root::Request::Record { .. } => unreachable!("world agent does not move accounts"),
            root::Request::Client { request } => out.push(Request::Client { request }),
            root::Request::Fact { fact } => out.push(Request::Fact { fact }),
            root::Request::Ready => agent.ready = true,
            root::Request::Kept { line } => agent.kept_lines.push(line),
            root::Request::Progress { line } => agent.progress.push(line),
            root::Request::Printed { number, printed } => {
                for task in released_tasks(&printed) {
                    if agent.release_tasks.insert(task) {
                        agent.shown_revision += 1;
                    }
                }
                agent.printed.insert(number, printed);
            }
        }
    }
}
fn released_tasks(printed: &Printed) -> Vec<u64> {
    let stdout = std::str::from_utf8(&printed.stdout).expect("printed document UTF-8");
    stdout
        .lines()
        .skip_while(|line| *line != "acts:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.strip_prefix("  task release "))
        .map(|line| line.split_ascii_whitespace().next().expect("release task").parse().expect("task number"))
        .collect()
}
pub fn fire(agent: &mut Agent, env: &Env<domain::Limits>, out: &mut Queue<Request>) {
    let mut requests =
        Queue::with_capacity(<crate::app::Temper as jig_client_world::harness::App>::max_out(&env.limits));
    root::fire(&mut agent.root, &environment(env), &mut requests);
    drain(agent, &mut requests, out);
}
#[must_use]
pub fn next_deadline(agent: &Agent) -> Option<Time> {
    root::next_deadline(&agent.root)
}
