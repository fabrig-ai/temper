//! Steps, exits and timers in; process orders out (protocol/setup.md, 7.2).
use crate::layout::Placed;
use crate::limits::{Limits, Unfit, check};
use crate::terminal::Fact;
use alloc::boxed::Box;
use skein_io::kernel::Exit;
use skein_lib::{List, Queue, Time};
use temper_forge_setup::Kind;
/// Inputs referring only to a step's index. Withdrawn ends an unsent spawn without an io owner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Started { step: usize },
    Ready { step: usize },
    NotReady { step: usize },
    Line { step: usize },
    Withdrawn { step: usize },
    NotStarted { step: usize },
    Exited { step: usize, exit: Exit },
    Closed { step: usize },
    Terminate,
    Ask { forge: u16 },
    Stop { forge: u16 },
    Written { forge: u16 },
    NotWritten { forge: u16 },
}
/// Effects requested of the host. Withdraw drops a spawn only before its io request exists.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Hold { step: usize },
    Spawn { step: usize },
    Withdraw { step: usize },
    Wait { step: usize },
    Signal { step: usize },
    Close { step: usize },
    Record { forge: u16 },
    Say { said: Said },
    Terminal { fact: Fact },
}
/// Pipe replies name a placed step; formatting belongs to the host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Said {
    Step { step: usize },
    Started { forge: u16 },
    Stopped { forge: u16 },
    Failed { forge: u16, why: Why },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    Busy,
    NotAsked,
    Stopping,
    Exited { step: usize, exit: Exit },
    NotStarted { step: usize },
    NotWritten,
    NotReady { step: usize },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Dormant,
    Starting,
    Recording { next: Option<usize> },
    Started,
    Failing { why: Why },
    Stopping,
}
#[derive(Debug)]
pub(crate) struct GroupState {
    forge: u16,
    recorded: bool,
    first: usize,
    end: usize,
    chained: bool,
    status: Status,
}
/// The result once all children are closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Stopped,
    GaveUp { step: usize, restarts: u32 },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Dormant,
    Waiting { next: Option<usize> },
    Starting { next: Option<usize> },
    Live { next: Option<usize> },
    Closing { next: Option<usize>, success: bool },
    Backoff { at: Time, next: Option<usize> },
    Grace { at: Time, second: bool },
    Stopped,
}
#[derive(Debug)]
pub(crate) struct State {
    kind: Kind,
    ready: bool,
    notice: Option<Fact>,
    waiting: bool,
    withdraw: bool,
    group: Option<usize>,
    phase: Phase,
    why: Option<Why>,
    exits: Queue<Time>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StopNotice {
    Unrequested,
    Requested,
    Reported,
}
/// The box's policy, independent of process handles and step names.
#[derive(Debug)]
pub struct Machine {
    states: Box<[State]>,
    groups: Box<[GroupState]>,
    limits: Limits,
    outcome: Option<Outcome>,
    forced: bool,
    begun: bool,
    stop_notice: StopNotice,
}
impl Machine {
    /// Checks limits and allocates each step's bounded exit history.
    pub fn new(limits: &Limits, steps: &[Placed]) -> Result<Self, Unfit> {
        check(limits)?;
        let mut states = List::with_capacity(u32::try_from(steps.len()).expect("steps fit u32"));
        let mut groups = List::with_capacity(u32::try_from(steps.len()).expect("bounded groups"));
        for (index, step) in steps.iter().enumerate() {
            if step.step.kind() == Kind::Link {
                assert!(step.asked.is_none(), "a link is outside groups");
                let before = index.checked_sub(1).expect("a link is not first");
                let previous = steps.get(before).expect("preceding step");
                assert!(previous.step.kind() == Kind::Long, "a link follows a long step");
                assert!(previous.asked.is_none(), "a link's long step is outside groups");
            }
            let group = match step.asked {
                Some(asked) => {
                    let last = groups.len().checked_sub(1);
                    let mut found = None;
                    if let Some(last) = last {
                        let previous: &mut GroupState = groups.get_mut(last).expect("last group");
                        if previous.forge == asked.forge && previous.end == index {
                            previous.end = index.saturating_add(1);
                            found = Some(usize::try_from(last).expect("group index"));
                        }
                    }
                    match found {
                        Some(index) => Some(index),
                        None => {
                            let group = usize::try_from(groups.len()).expect("group index");
                            groups
                                .push(GroupState {
                                    forge: asked.forge,
                                    recorded: asked.recorded,
                                    first: index,
                                    end: index.saturating_add(1),
                                    chained: false,
                                    status: Status::Dormant,
                                })
                                .expect("bounded groups");
                            Some(group)
                        }
                    }
                }
                None => None,
            };
            states
                .push(State {
                    kind: step.step.kind(),
                    ready: step.step.ready().is_some(),
                    notice: (!step.step.started_notice().is_empty()).then_some(Fact::Started { step: index }),
                    waiting: false,
                    withdraw: false,
                    group,
                    phase: Phase::Dormant,
                    why: None,
                    exits: Queue::with_capacity(limits.restarts.saturating_add(1)),
                })
                .expect("one state per step");
        }
        Ok(Self {
            states: states.into_boxed(),
            groups: groups.into_boxed(),
            limits: *limits,
            outcome: None,
            forced: false,
            begun: false,
            stop_notice: StopNotice::Unrequested,
        })
    }
    /// Conservative reservation for start/input/fire: each of N steps can
    /// contribute a hold, spawn, Starting and progress reply (4N). Closing or
    /// signalling visits each step once instead of spawning it. Teardown also
    /// withdraws each starting step once (at most N); this and the reverse stop
    /// visit fit within 4N, including a failure's closes and a give-up's stop.
    /// A group transition adds at most one fact and one reply (or one record); restart adds one fact;
    /// giving up adds one fact and visits the stop range. These alternatives and
    /// the two stop facts fit the remaining six. Fire visits each state once and
    /// emits at most four orders per state, with no group completion transitions.
    /// Reserve this before consuming any policy input.
    #[must_use]
    pub fn max_out(&self) -> u32 {
        u32::try_from(self.states.len())
            .expect("steps fit u32")
            .checked_mul(4)
            .expect("order bound")
            .checked_add(6)
            .expect("lifecycle facts")
    }
    /// Starts the first step once.
    pub fn start(&mut self, _now: Time, recorded: &[u16], orders: &mut Queue<Order>) {
        if !self.begun && self.outcome.is_none() {
            self.begun = true;
            for group in &mut self.groups {
                if group.recorded && recorded.contains(&group.forge) {
                    group.chained = true;
                    group.status = Status::Started;
                }
            }
            for first in 0..self.states.len() {
                if self.chain(first).is_none() {
                    self.spawn(first, self.successor(first), orders);
                    break;
                }
            }
        }
    }
    fn chain(&self, step: usize) -> Option<usize> {
        let group = self.states.get(step).expect("step index").group?;
        if self.groups.get(group).expect("group index").chained { None } else { Some(group) }
    }
    #[expect(clippy::manual_find, reason = "the programming model uses explicit loops rather than closures")]
    fn successor(&self, step: usize) -> Option<usize> {
        for next in step.saturating_add(1)..self.states.len() {
            if self.chain(step) == self.chain(next) {
                return Some(next);
            }
        }
        None
    }
    fn continuation(&self, step: usize) -> Option<usize> {
        match self.states.get(step)?.phase {
            Phase::Waiting { next } => next,
            Phase::Dormant
            | Phase::Starting { .. }
            | Phase::Live { .. }
            | Phase::Closing { .. }
            | Phase::Backoff { .. }
            | Phase::Grace { .. }
            | Phase::Stopped => self.successor(step),
        }
    }
    fn spawn(&mut self, step: usize, next: Option<usize>, orders: &mut Queue<Order>) {
        if let Some(next) = next
            && self.states.get(next).expect("next step").kind == Kind::Link
            && self.states.get(next).expect("next step").phase == Phase::Dormant
        {
            orders.push(Order::Hold { step });
        }
        if let Some(state) = self.states.get_mut(step) {
            state.phase = Phase::Starting { next };
            state.waiting = false;
            state.withdraw = false;
            state.why = None;
            orders.push(Order::Terminal { fact: Fact::Starting { step } });
            orders.push(Order::Spawn { step });
            if let Some(group) = state.group
                && self.groups.get(group).expect("group index").status == Status::Starting
            {
                orders.push(Order::Say { said: Said::Step { step } });
            }
        }
    }
    /// Takes one lifecycle or termination input.
    pub fn input(&mut self, now: Time, input: Input, orders: &mut Queue<Order>) {
        match input {
            Input::Withdrawn { step } => {
                if let Some(state) = self.states.get(step)
                    && state.withdraw
                {
                    self.ended_start(now, step, orders);
                }
            }
            Input::Terminate => self.stop(now, orders),
            Input::Ask { forge } => self.ask(forge, orders),
            Input::Stop { forge } => self.stop_group(now, forge, orders),
            Input::Written { forge } => self.written(forge, orders),
            Input::NotWritten { forge } => {
                if self.outcome.is_none()
                    && let Some(group) = self.group(forge)
                    && match self.groups.get(group).expect("group").status {
                        Status::Recording { .. } => true,
                        Status::Dormant
                        | Status::Starting
                        | Status::Started
                        | Status::Failing { .. }
                        | Status::Stopping => false,
                    }
                {
                    self.fail(group, Why::NotWritten, orders);
                }
            }
            Input::Line { step } => self.line(step, orders),
            Input::Started { step } => self.started(now, step, orders),
            Input::Ready { step } => {
                if self.waiting(step) {
                    self.states.get_mut(step).expect("step").waiting = false;
                    self.ready(step, orders);
                }
            }
            Input::NotReady { step } => {
                if self.outcome.is_none() && !self.group_stopping(step) && self.waiting(step) {
                    self.states.get_mut(step).expect("step").waiting = false;
                    if let Some(group) = self.starting_group(step) {
                        self.fail(group, Why::NotReady { step }, orders);
                    } else {
                        self.give_up(now, step, 0, Why::NotReady { step }, orders);
                    }
                }
            }
            Input::NotStarted { step } => {
                if self.outcome.is_some() || self.group_stopping(step) || self.group_failing(step) {
                    self.ended_start(now, step, orders);
                } else {
                    if self.outcome.is_none()
                        && let Some(group) = self.starting_group(step)
                    {
                        self.states.get_mut(step).expect("step").phase = Phase::Stopped;
                        self.fail(group, Why::NotStarted { step }, orders);
                        self.failed_if_closed(group, orders);
                        return;
                    }
                    if let Some(state) = self.states.get_mut(step) {
                        let next = match state.phase {
                            Phase::Starting { next } => next,
                            Phase::Dormant
                            | Phase::Waiting { .. }
                            | Phase::Live { .. }
                            | Phase::Closing { .. }
                            | Phase::Backoff { .. }
                            | Phase::Grace { .. }
                            | Phase::Stopped => return,
                        };
                        state.phase = Phase::Closing { next, success: false };
                        state.why = Some(Why::NotStarted { step });
                    }
                    self.closed(now, step, orders);
                }
            }
            Input::Exited { step, exit } => {
                if self.outcome.is_none()
                    && let Some(group) = self.starting_group(step)
                    && (self.states.get(step).expect("step").kind == Kind::Long || exit != Exit::Code(0))
                {
                    self.fail(group, Why::Exited { step, exit }, orders);
                } else {
                    self.exited(step, exit, orders);
                }
            }
            Input::Closed { step } => self.closed(now, step, orders),
        }
        if self.stop_notice == StopNotice::Requested && self.is_idle() {
            self.stop_notice = StopNotice::Reported;
            orders.push(Order::Terminal { fact: Fact::Stopped });
        }
    }
    // A withdrawal or late spawn failure is a teardown end, with no failure cause.
    fn ended_start(&mut self, now: Time, step: usize, orders: &mut Queue<Order>) {
        let Some(state) = self.states.get_mut(step) else { return };
        if let Phase::Starting { .. } = state.phase {
            state.phase = Phase::Closing { next: None, success: false };
            self.closed(now, step, orders);
        }
    }
    fn withdraw_range(&mut self, first: usize, end: usize, orders: &mut Queue<Order>) {
        for step in first..end {
            let state = self.states.get_mut(step).expect("withdraw step");
            if let Phase::Starting { .. } = state.phase
                && !state.withdraw
            {
                state.withdraw = true;
                orders.push(Order::Withdraw { step });
            }
        }
    }
    fn started(&mut self, now: Time, step: usize, orders: &mut Queue<Order>) {
        let Some(state) = self.states.get_mut(step) else {
            return;
        };
        match state.phase {
            Phase::Starting { next } => {
                state.phase = Phase::Live { next };
                if self.outcome.is_some() {
                    if self.forced {
                        state.phase = Phase::Closing { next: None, success: false };
                        orders.push(Order::Close { step });
                    } else {
                        self.stop_next(now, orders);
                    }
                } else if self.group_failing(step) {
                    self.states.get_mut(step).expect("step").phase = Phase::Closing { next: None, success: false };
                    orders.push(Order::Close { step });
                } else if let Some(group) = self.states.get(step).expect("step").group
                    && self.groups.get(group).expect("group").status == Status::Stopping
                {
                    self.stop_group_next(now, group, orders);
                } else if self.states.get(step).expect("step").ready {
                    self.states.get_mut(step).expect("step").waiting = true;
                    orders.push(Order::Wait { step });
                } else {
                    self.ready(step, orders);
                }
            }
            Phase::Dormant
            | Phase::Waiting { .. }
            | Phase::Live { .. }
            | Phase::Closing { .. }
            | Phase::Backoff { .. }
            | Phase::Grace { .. }
            | Phase::Stopped => {}
        }
    }
    fn ready(&mut self, step: usize, orders: &mut Queue<Order>) {
        if self.outcome.is_some() || self.group_stopping(step) {
            return;
        }
        let state = self.states.get(step).expect("step");
        let Phase::Live { next } = state.phase else { return };
        if state.kind != Kind::Long {
            return;
        }
        if let Some(next) = next
            && self.states.get(next).expect("next step").kind == Kind::Link
        {
            return;
        }
        self.states.get_mut(step).expect("step").phase = Phase::Live { next: None };
        let starting_group = self.starting_group(step).is_some();
        let state = self.states.get_mut(step).expect("step");
        if !starting_group && let Some(fact) = state.notice {
            orders.push(Order::Terminal { fact });
        }
        if let Some(group) = self.starting_group(step) {
            if self.group_failing(step) {
                self.states.get_mut(step).expect("step").phase = Phase::Closing { next: None, success: false };
                orders.push(Order::Close { step });
            } else {
                self.advance_group(group, next, orders);
            }
        } else if let Some(next) = next {
            self.spawn(next, self.continuation(next), orders);
        }
    }
    fn waiting(&self, step: usize) -> bool {
        match self.states.get(step) {
            Some(state) => state.waiting,
            None => false,
        }
    }
    fn group_failing(&self, step: usize) -> bool {
        match self.starting_group(step) {
            Some(group) => match self.groups.get(group).expect("group").status {
                Status::Failing { .. } => true,
                Status::Dormant | Status::Starting | Status::Recording { .. } | Status::Started | Status::Stopping => {
                    false
                }
            },
            None => false,
        }
    }
    fn line(&mut self, step: usize, orders: &mut Queue<Order>) {
        if self.outcome.is_some() {
            return;
        }
        let Some(state) = self.states.get(step) else { return };
        if state.kind != Kind::Long {
            return;
        }
        let next = match state.phase {
            Phase::Waiting { next }
            | Phase::Live { next }
            | Phase::Closing { next, .. }
            | Phase::Backoff { next, .. } => next,
            Phase::Dormant | Phase::Starting { .. } | Phase::Grace { .. } | Phase::Stopped => None,
        };
        let Some(link) = next else { return };
        let state = self.states.get(link).expect("next step");
        if state.kind != Kind::Link || state.phase != Phase::Dormant {
            return;
        }
        let state = self.states.get_mut(step).expect("long step");
        match &mut state.phase {
            Phase::Waiting { next }
            | Phase::Live { next }
            | Phase::Closing { next, .. }
            | Phase::Backoff { next, .. } => *next = None,
            Phase::Dormant | Phase::Starting { .. } | Phase::Grace { .. } | Phase::Stopped => {
                unreachable!("checked phase")
            }
        }
        self.spawn(link, self.continuation(link), orders);
    }
    fn exited(&mut self, step: usize, exit: Exit, orders: &mut Queue<Order>) {
        let Some(state) = self.states.get_mut(step) else {
            return;
        };
        match state.phase {
            Phase::Starting { next } | Phase::Live { next } => {
                state.why = Some(Why::Exited { step, exit });
                state.phase = Phase::Closing { next, success: exit == Exit::Code(0) };
                orders.push(Order::Close { step });
            }
            Phase::Grace { .. } => {
                state.why = Some(Why::Exited { step, exit });
                state.phase = Phase::Closing { next: None, success: exit == Exit::Code(0) };
                orders.push(Order::Close { step });
            }
            Phase::Dormant | Phase::Waiting { .. } | Phase::Closing { .. } | Phase::Backoff { .. } | Phase::Stopped => {
            }
        }
    }
    fn closed(&mut self, now: Time, step: usize, orders: &mut Queue<Order>) {
        let Some(state) = self.states.get_mut(step) else {
            return;
        };
        let group = state.group;
        let phase = core::mem::replace(&mut state.phase, Phase::Stopped);
        match phase {
            Phase::Closing { next, success } => {
                if self.outcome.is_some() {
                    self.stop_next(now, orders);
                } else if let Some(group) = group
                    && self.groups.get(group).expect("group").status == Status::Stopping
                {
                    self.stop_group_next(now, group, orders);
                } else if let Some(group) = group
                    && self.group_starting(group)
                {
                    match self.groups.get(group).expect("group").status {
                        Status::Failing { .. } => self.failed_if_closed(group, orders),
                        Status::Starting => self.advance_group(group, next, orders),
                        Status::Dormant | Status::Recording { .. } | Status::Started | Status::Stopping => {}
                    }
                } else if self.states.get(step).expect("step").kind == Kind::Long {
                    self.restart(now, step, next, orders);
                } else if success {
                    if let Some(next) = next {
                        self.spawn(next, self.continuation(next), orders);
                    }
                } else {
                    let why = self.states.get(step).expect("step").why.expect("accepted failure");
                    self.give_up(now, step, 0, why, orders);
                }
            }
            Phase::Dormant
            | Phase::Waiting { .. }
            | Phase::Starting { .. }
            | Phase::Live { .. }
            | Phase::Backoff { .. }
            | Phase::Grace { .. }
            | Phase::Stopped => {
                state.phase = phase;
            }
        }
    }
    fn group(&self, forge: u16) -> Option<usize> {
        for (index, group) in self.groups.iter().enumerate() {
            if group.forge == forge {
                return Some(index);
            }
        }
        None
    }
    fn group_starting(&self, group: usize) -> bool {
        match self.groups.get(group).expect("group").status {
            Status::Starting | Status::Recording { .. } | Status::Failing { .. } => true,
            Status::Dormant | Status::Started | Status::Stopping => false,
        }
    }
    fn starting_group(&self, step: usize) -> Option<usize> {
        let group = self.states.get(step)?.group?;
        if self.group_starting(group) { Some(group) } else { None }
    }
    fn group_stopping(&self, step: usize) -> bool {
        let Some(state) = self.states.get(step) else { return false };
        match state.group {
            Some(group) => self.groups.get(group).expect("group").status == Status::Stopping,
            None => false,
        }
    }
    fn refused(&self, forge: u16, stop: bool, orders: &mut Queue<Order>) -> bool {
        let mut why = None;
        if self.outcome.is_some() {
            why = Some(Why::Stopping);
        } else {
            for group in &self.groups {
                let busy = match group.status {
                    Status::Stopping => true,
                    Status::Failing { .. } => stop || group.forge != forge,
                    Status::Dormant | Status::Starting | Status::Recording { .. } | Status::Started => false,
                };
                if busy {
                    why = Some(Why::Busy);
                    break;
                }
            }
            if why.is_none() && self.group(forge).is_none() {
                why = Some(Why::NotAsked);
            }
        }
        if let Some(why) = why {
            orders.push(Order::Say { said: Said::Failed { forge, why } });
            true
        } else {
            false
        }
    }
    fn ask(&mut self, forge: u16, orders: &mut Queue<Order>) {
        if self.refused(forge, false, orders) {
            return;
        }
        let index = self.group(forge).expect("checked group");
        if self.groups.get(index).expect("group").status == Status::Started {
            orders.push(Order::Say { said: Said::Started { forge } });
            return;
        }
        if self.group_starting(index) {
            return;
        }
        for group in 0..self.groups.len() {
            if self.group_starting(group) {
                orders.push(Order::Say { said: Said::Failed { forge, why: Why::Busy } });
                return;
            }
        }
        let group = self.groups.get_mut(index).expect("group");
        group.status = Status::Starting;
        let first = group.first;
        orders.push(Order::Terminal { fact: Fact::GroupStarting { forge } });
        self.advance_group(index, Some(first), orders);
    }
    fn stop_group(&mut self, now: Time, forge: u16, orders: &mut Queue<Order>) {
        if self.refused(forge, true, orders) {
            return;
        }
        for group in 0..self.groups.len() {
            if self.group_starting(group) {
                orders.push(Order::Say { said: Said::Failed { forge, why: Why::Busy } });
                return;
            }
        }
        let index = self.group(forge).expect("checked group");
        let group = self.groups.get_mut(index).expect("group");
        if group.status == Status::Dormant {
            orders.push(Order::Say { said: Said::Stopped { forge } });
            return;
        }
        group.status = Status::Stopping;
        // A restart's preparation can still be live. Disarm its continuation
        // and backoff along with the long steps (protocol/setup.md, 7.2).
        group.chained = false;
        for step in group.first..group.end {
            let state = self.states.get_mut(step).expect("group step");
            if let Phase::Backoff { .. } = state.phase {
                state.phase = Phase::Stopped;
            }
        }
        let (first, end) = (group.first, group.end);
        self.withdraw_range(first, end, orders);
        self.stop_group_next(now, index, orders);
    }
    fn stop_group_next(&mut self, now: Time, index: usize, orders: &mut Queue<Order>) {
        let group = self.groups.get(index).expect("group");
        let (first, end) = (group.first, group.end);
        if !self.stop_range(now, first, end, orders) {
            return;
        }
        for step in first..end {
            let state = self.states.get_mut(step).expect("group step");
            state.phase = Phase::Dormant;
            state.waiting = false;
            while state.exits.pop().is_some() {}
        }
        let group = self.groups.get_mut(index).expect("group");
        group.status = Status::Dormant;
        orders.push(Order::Terminal { fact: Fact::GroupStopped { forge: group.forge } });
        orders.push(Order::Say { said: Said::Stopped { forge: group.forge } });
    }
    fn advance_group(&mut self, index: usize, next: Option<usize>, orders: &mut Queue<Order>) {
        // The record means the whole group stands (setup.md, 6.6/7.1):
        // Forgejo answered and the remaining group steps completed.
        if let Some(step) = next {
            self.spawn(step, self.successor(step), orders);
        } else {
            let group = self.groups.get_mut(index).expect("group");
            if group.recorded {
                group.status = Status::Recording { next: None };
                orders.push(Order::Record { forge: group.forge });
            } else {
                self.group_started(index, orders);
            }
        }
    }
    fn group_started(&mut self, index: usize, orders: &mut Queue<Order>) {
        let group = self.groups.get_mut(index).expect("group");
        group.status = Status::Started;
        orders.push(Order::Terminal { fact: Fact::GroupStarted { forge: group.forge } });
        orders.push(Order::Say { said: Said::Started { forge: group.forge } });
    }
    fn written(&mut self, forge: u16, orders: &mut Queue<Order>) {
        if self.outcome.is_some() {
            return;
        }
        let Some(index) = self.group(forge) else {
            return;
        };
        let group = self.groups.get_mut(index).expect("group");
        match group.status {
            Status::Recording { next } => {
                group.status = Status::Starting;
                if let Some(step) = next {
                    self.spawn(step, self.successor(step), orders);
                } else {
                    self.group_started(index, orders);
                }
            }
            Status::Dormant | Status::Starting | Status::Started | Status::Failing { .. } | Status::Stopping => {}
        }
    }
    fn fail(&mut self, index: usize, why: Why, orders: &mut Queue<Order>) {
        let group = self.groups.get_mut(index).expect("group");
        if match group.status {
            Status::Failing { .. } => true,
            Status::Dormant | Status::Starting | Status::Recording { .. } | Status::Started | Status::Stopping => false,
        } {
            return;
        }
        group.status = Status::Failing { why };
        for step in group.first..group.end {
            let state = self.states.get_mut(step).expect("group step");
            match state.phase {
                Phase::Live { .. } | Phase::Grace { .. } => {
                    state.phase = Phase::Closing { next: None, success: false };
                    orders.push(Order::Close { step });
                }
                Phase::Dormant
                | Phase::Waiting { .. }
                | Phase::Starting { .. }
                | Phase::Closing { .. }
                | Phase::Backoff { .. }
                | Phase::Stopped => {}
            }
        }
        let (first, end) = (group.first, group.end);
        self.withdraw_range(first, end, orders);
        self.failed_if_closed(index, orders);
    }
    fn failed_if_closed(&mut self, index: usize, orders: &mut Queue<Order>) {
        let group = self.groups.get_mut(index).expect("group");
        let why = match group.status {
            Status::Failing { why } => why,
            Status::Dormant | Status::Starting | Status::Recording { .. } | Status::Started | Status::Stopping => {
                return;
            }
        };
        for step in group.first..group.end {
            match self.states.get(step).expect("group step").phase {
                Phase::Dormant | Phase::Stopped => {}
                Phase::Waiting { .. }
                | Phase::Starting { .. }
                | Phase::Live { .. }
                | Phase::Closing { .. }
                | Phase::Backoff { .. }
                | Phase::Grace { .. } => return,
            }
        }
        for step in group.first..group.end {
            let state = self.states.get_mut(step).expect("group step");
            state.phase = Phase::Dormant;
            for _ in 0..state.exits.len() {
                state.exits.pop();
            }
        }
        group.status = Status::Dormant;
        orders.push(Order::Terminal { fact: Fact::GroupFailed { forge: group.forge, why } });
        orders.push(Order::Say { said: Said::Failed { forge: group.forge, why } });
    }
    fn restart(&mut self, now: Time, step: usize, next: Option<usize>, orders: &mut Queue<Order>) {
        let state = self.states.get_mut(step).expect("known step");
        let count = state.exits.len();
        for _ in 0..count {
            let at = state.exits.pop().expect("counted exit");
            if now.saturating_since(at) <= self.limits.window {
                state.exits.push(at);
            }
        }
        state.exits.push(now);
        let count = state.exits.len();
        let why = state.why.expect("restart follows accepted exit or spawn refusal");
        if count > self.limits.restarts {
            self.give_up(now, step, self.limits.restarts, why, orders);
        } else {
            let mut delay = self.limits.backoff;
            for _ in 1..count {
                delay = delay.saturating_mul(2).min(self.limits.backoff_max);
            }
            self.states.get_mut(step).expect("known step").phase =
                Phase::Backoff { at: now.saturating_add(delay), next };
            orders.push(Order::Terminal { fact: Fact::Restarting { step, why, delay } });
        }
    }
    fn give_up(&mut self, now: Time, step: usize, restarts: u32, why: Why, orders: &mut Queue<Order>) {
        self.outcome = Some(Outcome::GaveUp { step, restarts });
        orders.push(Order::Terminal { fact: Fact::GivingUp { step, why } });
        self.disarm();
        self.withdraw_range(0, self.states.len(), orders);
        self.stop_next(now, orders);
    }
    fn disarm(&mut self) {
        for state in &mut self.states {
            match state.phase {
                Phase::Backoff { .. } => state.phase = Phase::Stopped,
                Phase::Dormant
                | Phase::Waiting { .. }
                | Phase::Starting { .. }
                | Phase::Live { .. }
                | Phase::Closing { .. }
                | Phase::Grace { .. }
                | Phase::Stopped => {}
            }
        }
    }
    fn stop(&mut self, now: Time, orders: &mut Queue<Order>) {
        if self.stop_notice == StopNotice::Unrequested {
            self.stop_notice = StopNotice::Requested;
            orders.push(Order::Terminal { fact: Fact::Stopping });
        }
        if self.outcome.is_some() {
            self.forced = true;
            for (step, state) in self.states.iter_mut().enumerate() {
                match state.phase {
                    Phase::Live { .. } | Phase::Grace { .. } => {
                        state.phase = Phase::Closing { next: None, success: false };
                        orders.push(Order::Close { step });
                    }
                    Phase::Dormant
                    | Phase::Waiting { .. }
                    | Phase::Starting { .. }
                    | Phase::Closing { .. }
                    | Phase::Backoff { .. }
                    | Phase::Stopped => {}
                }
            }
        } else {
            self.outcome = Some(Outcome::Stopped);
            self.disarm();
            self.withdraw_range(0, self.states.len(), orders);
            self.stop_next(now, orders);
        }
    }
    fn stop_next(&mut self, now: Time, orders: &mut Queue<Order>) {
        self.stop_range(now, 0, self.states.len(), orders);
    }
    fn stop_range(&mut self, now: Time, first: usize, end: usize, orders: &mut Queue<Order>) -> bool {
        for step in (first..end).rev() {
            let state = self.states.get_mut(step).expect("stop step");
            match state.phase {
                Phase::Live { .. } => {
                    state.phase = Phase::Grace { at: now.saturating_add(self.limits.grace), second: false };
                    orders.push(Order::Signal { step });
                    return false;
                }
                Phase::Starting { .. } | Phase::Closing { .. } | Phase::Grace { .. } => return false,
                Phase::Dormant | Phase::Waiting { .. } | Phase::Backoff { .. } | Phase::Stopped => {}
            }
        }
        true
    }
    /// Fires all expired waits; the caller reserves `max_out` orders.
    pub fn fire(&mut self, now: Time, orders: &mut Queue<Order>) {
        for step in 0..self.states.len() {
            let phase = self.states.get(step).expect("bounded step").phase;
            match phase {
                Phase::Backoff { at, next: following } if at <= now => {
                    let mut first = step;
                    for before in (0..step).rev() {
                        if self.chain(before) != self.chain(step) {
                            continue;
                        }
                        if self.states.get(before).expect("bounded step").kind != Kind::Once {
                            break;
                        }
                        first = before;
                    }
                    self.states.get_mut(step).expect("bounded step").phase = Phase::Waiting { next: following };
                    let next = if first == step { following } else { self.successor(first) };
                    self.spawn(first, next, orders);
                }
                Phase::Grace { at, second } if at <= now => {
                    let state = self.states.get_mut(step).expect("bounded step");
                    if second {
                        state.phase = Phase::Closing { next: None, success: false };
                        orders.push(Order::Close { step });
                    } else {
                        state.phase = Phase::Grace { at: now.saturating_add(self.limits.grace), second: true };
                        orders.push(Order::Signal { step });
                    }
                }
                Phase::Dormant
                | Phase::Waiting { .. }
                | Phase::Starting { .. }
                | Phase::Live { .. }
                | Phase::Closing { .. }
                | Phase::Backoff { .. }
                | Phase::Grace { .. }
                | Phase::Stopped => {}
            }
        }
    }
    /// Earliest backoff or grace still armed.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for state in &self.states {
            match state.phase {
                Phase::Backoff { at, .. } | Phase::Grace { at, .. } => next = Some(next.unwrap_or(at).min(at)),
                Phase::Dormant
                | Phase::Waiting { .. }
                | Phase::Starting { .. }
                | Phase::Live { .. }
                | Phase::Closing { .. }
                | Phase::Stopped => {}
            }
        }
        next
    }
    /// The chosen outcome; meaningful to the host once idle.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }
    /// No starting or live child remains.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        for state in &self.states {
            match state.phase {
                Phase::Dormant | Phase::Waiting { .. } | Phase::Stopped => {}
                Phase::Starting { .. }
                | Phase::Live { .. }
                | Phase::Closing { .. }
                | Phase::Backoff { .. }
                | Phase::Grace { .. } => return false,
            }
        }
        true
    }
}
