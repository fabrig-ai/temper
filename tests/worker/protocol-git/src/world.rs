//! One component, scripted requests above and scripted records below.
use crate::{
    fixtures,
    io::{Below, Input, Invocation, Node, Script},
    referee::{Expected, Referee, Verdict},
};
use skein_lib::{Time, Token};
use std::collections::BTreeMap;
use temper_worker_domain_checkout::git;
use temper_worker_protocol_git::{Component, Event, Request};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    None,
    Deadline(u32),
    Cancel(u32),
    ExitFirst,
}
#[derive(Debug)]
pub struct Scenario {
    pub op: git::Op,
    pub script: Script,
    pub stop: Stop,
    pub expected: Expected,
    pub identity: bool,
    pub files: BTreeMap<Vec<u8>, Node>,
}
#[derive(Debug)]
pub struct Outcome {
    pub verdict: Verdict,
    pub terminals: Vec<(Token, git::Done)>,
    pub spawns: u32,
    pub signals: u32,
    pub tree: BTreeMap<Vec<u8>, Node>,
}
impl Outcome {
    #[must_use]
    pub fn faults(&self) -> Vec<(Token, Option<git::Fault>)> {
        self.terminals
            .iter()
            .map(|(owner, done)| {
                (
                    *owner,
                    match done {
                        git::Done::Failed { fault } | git::Done::FailedWithOutput { fault, .. } => Some(*fault),
                        git::Done::IsAncestor { .. }
                        | git::Done::Merged
                        | git::Done::Conflicted { .. }
                        | git::Done::Succeeded
                        | git::Done::Fetched { .. }
                        | git::Done::Committed { .. }
                        | git::Done::Unchanged
                        | git::Done::Exists
                        | git::Done::Rejected => None,
                    },
                )
            })
            .collect()
    }
}
fn deliver(component: &mut Component, below: &mut Below, now: Time, input: Input) {
    match input {
        Input::Io(event) => {
            assert!(component.accepts(&event), "owned io event");
            component.up(now, event, &mut below.out);
        }
        Input::Kernel(complete) => {
            assert!(component.owns(complete.op));
            component.complete(now, complete, &mut below.out);
        }
    }
    component.reclaim();
}
/// Run to the terminal, then shut down and settle every owned descriptor.
#[must_use]
pub fn run(scenario: &Scenario, seed: u64) -> Outcome {
    let mut below = Below::new(Script { invocations: [Invocation::captured("version")].into(), ..Script::default() });
    let mut component =
        Component::new(&fixtures::limits(), &fixtures::checkout(), fixtures::settings()).expect("valid limits");
    let mut draw = seed;
    component.start(Time::ZERO, &mut below.out);
    let mut started = false;
    for _ in 0..10_000 {
        below.drain();
        if let Some(event) = below.out.events.pop() {
            assert_eq!(event, Event::Started);
            started = true;
        }
        let Some(input) = below.next(&mut draw) else {
            break;
        };
        deliver(&mut component, &mut below, Time::ZERO, input);
    }
    assert!(started);
    below.reset(fixtures::copy_script(&scenario.script));
    below.tree.extend(scenario.files.clone());
    let mut now = Time::ZERO;
    component.down(
        now,
        Request::Io {
            owner: fixtures::OWNER,
            op: scenario.op.clone(),
            deadline: fixtures::DEADLINE,
            identity: scenario.identity.then(fixtures::identity),
        },
        &mut below.out,
    );
    let mut referee = Referee::default();
    let mut stopped = false;
    for _ in 0..100_000 {
        below.drain();
        referee.check(&below);
        if let Some(event) = below.out.events.pop() {
            referee.observe(event, &below, fixtures::OWNER);
        }
        if !stopped
            && match scenario.stop {
                Stop::Deadline(at) | Stop::Cancel(at) => below.spawns == at,
                Stop::None | Stop::ExitFirst => false,
            }
        {
            stopped = true;
            referee.stop(below.spawns);
            match scenario.stop {
                Stop::Deadline(_) => {
                    now = fixtures::DEADLINE;
                    component.fire(now, &mut below.out);
                }
                Stop::Cancel(_) => component.down(now, Request::CancelIo { owner: fixtures::OWNER }, &mut below.out),
                Stop::None | Stop::ExitFirst => unreachable!("stop selected"),
            }
        }
        let Some(input) = below.next(&mut draw) else {
            break;
        };
        if !stopped && scenario.stop == Stop::ExitFirst && below.closed_children == below.spawns && below.spawns > 0 {
            stopped = true;
            referee.stop(below.spawns);
            component.down(now, Request::CancelIo { owner: fixtures::OWNER }, &mut below.out);
        }
        deliver(&mut component, &mut below, now, input);
    }
    below.drain();
    referee.check(&below);
    if let Some(event) = below.out.events.pop() {
        referee.observe(event, &below, fixtures::OWNER);
    }
    let verdict = referee.finish(&scenario.expected);
    component.down(now, Request::Shutdown, &mut below.out);
    for _ in 0..10_000 {
        below.drain();
        let Some(input) = below.next(&mut draw) else {
            break;
        };
        deliver(&mut component, &mut below, now, input);
    }
    assert!(component.is_empty() && below.settled(), "teardown has no live resources");
    Outcome { verdict, terminals: referee.terminals, spawns: below.spawns, signals: below.signals, tree: below.tree }
}
