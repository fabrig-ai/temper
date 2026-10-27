//! One task's borrowed documents, first activity page and live run handle.
use super::{Act, Request, inbox::Card};
use crate::address::PlanFilter;
use crate::{Address, Limits, owner::Owner};
use alloc::boxed::Box;
use jig_client::{Client, Handle, Key, Latest, Readable, Readout, Told, Watchable, contract as c};
use skein_lib::{Duration, Env, List, Queue, Time, Wall};
/// A task's state as a goal's page says it (client/README.md, 5.8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// jig's word for its phase.
    Phase(c::Phase),
    /// Waiting on one listed dependency: this task.
    WaitingFor(u64),
    /// Waiting on `count` listed dependencies, and on more when the list was cut.
    WaitingForTasks { count: u32, more: bool },
    /// Waiting on no dependency, for its holdings.
    WaitingForHoldings,
    /// Waiting for the supplied number of live delegates.
    WaitingForDelegates(u32),
    /// Waiting for an answer from the reader.
    WaitingForYou,
    /// Waiting for an answer from another person or role.
    WaitingForParty(c::Whom),
    /// Waiting for news.
    WaitingForNews,
    /// Running without more specific observed work.
    Running,
    /// Observed writing.
    Writing,
    /// Observed using this tool.
    Tool(Box<[u8]>),
    /// Ended, with its owner's ending.
    Ended(c::OwnerTaskStatus),
}
/// A task's state from its chip.
#[must_use]
pub fn row_state(chip: &c::Chip) -> State {
    if chip.phase == c::Phase::Running {
        if let c::ChipPhase::Owner { details, .. } = chip.details.as_ref()
            && let Some(observation) = &details.observation
        {
            match &observation.work {
                c::OwnerChipWork::Writing => return State::Writing,
                c::OwnerChipWork::Tool { tool, .. } => return State::Tool(tool.clone()),
                c::OwnerChipWork::Preparing
                | c::OwnerChipWork::Starting
                | c::OwnerChipWork::Active
                | c::OwnerChipWork::Waiting
                | c::OwnerChipWork::Ending => {}
            }
        }
        return State::Running;
    }
    match chip.details.as_ref() {
        c::ChipPhase::Owner { phase: c::OwnerChipPhase::Waiting { dependencies, more, holds }, .. } => {
            if dependencies.len() == 1 && !more {
                State::WaitingFor(*dependencies.as_slice().first().expect("one dependency"))
            } else if !dependencies.is_empty() || *more {
                State::WaitingForTasks { count: dependencies.len(), more: *more }
            } else if *holds {
                State::WaitingForHoldings
            } else {
                State::Phase(chip.phase)
            }
        }
        c::ChipPhase::Owner { phase: c::OwnerChipPhase::Ended, details } => match &details.ending {
            Some(c::OwnerChipEnding::Done(_)) => State::Ended(c::OwnerTaskStatus::Done),
            Some(c::OwnerChipEnding::Failed(_)) => State::Ended(c::OwnerTaskStatus::Failed),
            Some(c::OwnerChipEnding::Cancelled { .. }) => State::Ended(c::OwnerTaskStatus::Cancelled),
            None => State::Phase(chip.phase),
        },
        c::ChipPhase::Owner {
            phase:
                c::OwnerChipPhase::Active(_)
                | c::OwnerChipPhase::Person { .. }
                | c::OwnerChipPhase::Closing(_)
                | c::OwnerChipPhase::Held { .. },
            ..
        }
        | c::ChipPhase::Active(_)
        | c::ChipPhase::Waiting { .. }
        | c::ChipPhase::Idle { .. }
        | c::ChipPhase::Starting
        | c::ChipPhase::Running { .. }
        | c::ChipPhase::WaitingForWords { .. }
        | c::ChipPhase::BackingOff { .. }
        | c::ChipPhase::Procedure { .. }
        | c::ChipPhase::Person { .. }
        | c::ChipPhase::Closing(_)
        | c::ChipPhase::Held { .. }
        | c::ChipPhase::Ended { .. } => State::Phase(chip.phase),
    }
}
/// A goal's header state from its exact wait, falling back to its chip.
#[must_use]
pub fn header_state(task: &c::Task, me: Option<&c::Me>) -> State {
    match task.chip.phase {
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => return row_state(&task.chip),
        c::Phase::Waiting
        | c::Phase::Idle
        | c::Phase::Due
        | c::Phase::Preparing
        | c::Phase::Claimed
        | c::Phase::Running
        | c::Phase::Active
        | c::Phase::BackingOff
        | c::Phase::Closing
        | c::Phase::Held => {}
    }
    match &task.waits_on {
        c::WaitsOn::Dependencies { pending, count } => {
            if *count == 1 && pending.len() == 1 {
                State::WaitingFor(*pending.as_slice().first().expect("one pending task"))
            } else if *count != 0 {
                State::WaitingForTasks { count: *count, more: false }
            } else {
                row_state(&task.chip)
            }
        }
        c::WaitsOn::Delegates => match task.chip.details.as_ref() {
            c::ChipPhase::Owner { details, .. } => State::WaitingForDelegates(details.live_delegates),
            c::ChipPhase::Active(_)
            | c::ChipPhase::Waiting { .. }
            | c::ChipPhase::Idle { .. }
            | c::ChipPhase::Starting
            | c::ChipPhase::Running { .. }
            | c::ChipPhase::WaitingForWords { .. }
            | c::ChipPhase::BackingOff { .. }
            | c::ChipPhase::Procedure { .. }
            | c::ChipPhase::Person { .. }
            | c::ChipPhase::Closing(_)
            | c::ChipPhase::Held { .. }
            | c::ChipPhase::Ended { .. } => row_state(&task.chip),
        },
        c::WaitsOn::Party { whom, .. } => {
            if let Some(me) = me
                && reader(*whom, task.project, me)
            {
                State::WaitingForYou
            } else {
                State::WaitingForParty(*whom)
            }
        }
        c::WaitsOn::News => State::WaitingForNews,
        c::WaitsOn::Nothing => row_state(&task.chip),
    }
}
/// A task's run heading (client/README.md, 5.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// A run is out: claimed, running, or its run closing.
    Live { attempt: u64, since: Option<Wall> },
    /// The task lives and no run is out: its last run's attempt.
    Waiting { attempt: u64, since: Option<Wall> },
    /// The task has ended: its last run's attempt.
    Ended { attempt: u64 },
    /// The task lives and has had no run.
    NoRun,
}
/// None when the task ended without a run, or its chip is not an owner's.
#[must_use]
#[expect(clippy::manual_map, reason = "the client's domain matches without a closure")]
pub fn run(task: &c::Task) -> Option<Run> {
    let c::ChipPhase::Owner { phase, details } = task.chip.details.as_ref() else { return None };
    match phase {
        c::OwnerChipPhase::Active(c::RunStage::Claimed { attempt } | c::RunStage::Running { attempt })
        | c::OwnerChipPhase::Closing(c::ClosingStage::Run { attempt }) => {
            Some(Run::Live { attempt: *attempt, since: details.run_since })
        }
        c::OwnerChipPhase::Ended => match details.attempt {
            Some(attempt) => Some(Run::Ended { attempt }),
            None => None,
        },
        c::OwnerChipPhase::Active(_)
        | c::OwnerChipPhase::Waiting { .. }
        | c::OwnerChipPhase::Person { .. }
        | c::OwnerChipPhase::Closing(_)
        | c::OwnerChipPhase::Held { .. } => Some(match details.attempt {
            Some(attempt) => Run::Waiting { attempt, since: details.run_since },
            None => Run::NoRun,
        }),
    }
}
// Jig's facts::reader is private; keep its person and project-role identity rule.
fn reader(whom: c::Whom, project: u32, me: &c::Me) -> bool {
    match whom {
        c::Whom::Person(person) => person == me.party.number,
        c::Whom::Role { project: held_project, role } => {
            if held_project != project {
                return false;
            }
            for held in &me.projects {
                let number = match held.role {
                    c::HeldRole::Owner => 0,
                    c::HeldRole::Maintainer => 1,
                    c::HeldRole::Member => 2,
                    c::HeldRole::Observer => 3,
                    c::HeldRole::Policy { role } => role,
                };
                if held.project == project && number == role {
                    return true;
                }
            }
            false
        }
    }
}
/// A dependency's state from its link; none when a stored link has no status.
#[must_use]
pub fn link_state(link: &c::OwnerTaskLink) -> Option<State> {
    match link {
        c::OwnerTaskLink::Live(chip) | c::OwnerTaskLink::Retained { chip, .. } => Some(row_state(chip)),
        c::OwnerTaskLink::Stored { status: Some(status), .. } => Some(State::Ended(*status)),
        c::OwnerTaskLink::Stored { status: None, .. } => None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Unknown,
    Tracked,
    /// An agent's task with no priority, not the chat label (client/README.md, 5.9).
    Agent,
    HeaderAndPlan,
}
/// A task page's request kinds, one in flight per kind (client/README.md, 5.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sending {
    Message,
    Stop,
    Cancel,
    Amend,
}
/// The page's open amendment; its draft is 3.7's (client/README.md, 5.11).
#[derive(Debug)]
pub struct Amending {
    pub draft: crate::amend::Draft,
    pub words: Box<[u8]>,
    pub cut: bool,
    pub tracking: Option<c::Tracking>,
    pub no_change: bool,
    pub done: bool,
    pub outcome: Option<jig_client::Outcome>,
}
impl Amending {
    fn of(task: &c::Task, shape: Shape) -> Self {
        let mut draft = crate::amend::Draft::of(task);
        let words = task.spec.specification.words.bytes.clone();
        let cut = draft.words(words.clone()).is_err();
        Self {
            draft,
            words: if cut { Box::default() } else { words.into_bytes() },
            cut,
            tracking: if shape == Shape::Tracked {
                Some(match task.priority {
                    Some(priority) => c::Tracking::Tracked { priority },
                    None => c::Tracking::Untracked,
                })
            } else {
                None
            },
            no_change: false,
            done: false,
            outcome: None,
        }
    }
}
/// A goal's or procedure's ended delegates, one read out at a time (client/README.md, 5.8, 5.10).
#[derive(Debug)]
pub struct Ended {
    pub handle: Handle,
    pub page: Option<Box<c::TaskPage>>,
    pub reading: bool,
    pub stale: bool,
}

// A document attempt belongs to this page, including its next ask (client/README.md, 3.5).
const READ_FIRST: Duration = Duration::from_secs(1);
const READ_MOST: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadEnd {
    Busy,
    Unreachable,
    Limit,
    /// Jig ends old reads when a successful sign-in installs its credential.
    SignIn,
    SignedOut,
    Unknown,
    NotAllowed,
    NotServed,
    Gone,
    TooLarge,
    TooLinked {
        joins: u32,
    },
    Malformed,
}
impl ReadEnd {
    #[must_use]
    pub const fn transient(self) -> bool {
        match self {
            Self::Busy | Self::Unreachable | Self::Limit | Self::SignIn => true,
            Self::SignedOut
            | Self::Unknown
            | Self::NotAllowed
            | Self::NotServed
            | Self::Gone
            | Self::TooLarge
            | Self::TooLinked { .. }
            | Self::Malformed => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentRead {
    Reading,
    Ready,
    Ended { why: ReadEnd, retry_at: Option<Time> },
}

#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads document state while backoff stays private")]
pub struct Task {
    pub number: u64,
    pub document: Handle,
    pub document_read: DocumentRead,
    read_wait: Duration,
    /// The document that already permitted an ending-triggered panel reopen (client/forge.md, 9.2).
    pub(crate) panel_reopened_at: Option<u64>,
    pub tree: Handle,
    pub activity: Handle,
    pub run: Option<Handle>,
    pub ended: Option<Ended>,
    pub filter: PlanFilter,
    pub shape: Shape,
    pub latest: Option<Box<c::ActivityPage>>,
    pub thread: jig_client::thread::Thread,
    pub final_task: Option<Box<c::Task>>,
    pub next: u64,
    pub through: Option<u64>,
    pub cards: List<Card>,
    pub words: Box<[u8]>,
    pub pending: List<(Key, Sending)>,
    pub outcome: Option<jig_client::Outcome>,
    pub amending: Option<Amending>,
}
impl Task {
    pub(crate) fn new(number: u64, first: u64, limits: &Limits, out: &mut Queue<Request>) -> Self {
        let document = handle(first);
        let tree = handle(first.checked_add(1).expect("task handles fit"));
        let activity = handle(first.checked_add(2).expect("task handles fit"));
        for (handle, subject) in [
            (document, c::Subject::Task { task: number }),
            (tree, c::Subject::Tree { task: number, window: limits.client.domain.rows }),
        ] {
            out.push(Request::Client(jig_client::Event::Watch { handle, subject: Watchable::Jig(subject) }));
        }
        let mut thread = jig_client::thread::Thread::new(number, &super::chat::thread_limits(limits));
        thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
        read(activity, c::Read::Activity { task: number, before: None }, out);
        Self {
            number,
            document,
            document_read: DocumentRead::Reading,
            read_wait: READ_FIRST,
            panel_reopened_at: None,
            tree,
            activity,
            run: None,
            ended: None,
            filter: PlanFilter::All,
            shape: Shape::Unknown,
            latest: None,
            thread,
            final_task: None,
            next: first.checked_add(3).expect("task handles fit"),
            through: None,
            cards: List::with_capacity(limits.client.domain.reads),
            words: Box::default(),
            pending: List::with_capacity(4),
            outcome: None,
            amending: None,
        }
    }
    #[must_use]
    pub fn document<'a>(&'a self, client: &'a Client) -> Option<&'a c::Task> {
        if let Some(task) = &self.final_task {
            return Some(task);
        }
        match client.document(self.document)?.latest? {
            Latest::Jig(c::Delivery::Task(task)) => Some(task),
            Latest::Jig(_) | Latest::Family(_) => None,
        }
    }
    /// A procedure's connector panel (jig's client/domain/pages.md, 5.2).
    #[must_use]
    pub fn panel(&self, client: &Client, forge: u16) -> Option<crate::forge::panel::Subject> {
        let task = self.document(client)?;
        match task.chip.executor {
            c::Executor::Procedure { connector, .. } if connector == forge => {
                Some(crate::forge::panel::Subject { task: task.chip.task, project: task.project })
            }
            c::Executor::Procedure { .. }
            | c::Executor::Agent { .. }
            | c::Executor::Party { .. }
            | c::Executor::Role { .. } => None,
        }
    }
    pub(crate) fn act(
        &mut self,
        env: &Env<Limits>,
        client: &Client,
        act: Act,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        if self.shape != Shape::Agent {
            return;
        }
        match act {
            Act::MessageWords { words } => {
                if words.len() <= usize::try_from(env.limits.client.domain.text).expect("text bound fits") {
                    self.words = words;
                }
            }
            Act::Send => self.send(env, client, Sending::Message, serial, out),
            Act::Older => {
                if let Some(older) = self.thread.older() {
                    read(self.activity, older, out);
                }
            }
            Act::Amend
            | Act::AmendWords { .. }
            | Act::AmendTracked
            | Act::AmendPriority { .. }
            | Act::DiscardAmendment
            | Act::SendAmendment
            | Act::AmendAgain
            | Act::OpenProject { .. }
            | Act::Project { .. }
            | Act::Choice(_)
            | Act::SignOut
            | Act::Accounts(_)
            | Act::ConnectModel(_)
            | Act::ChatWords { .. }
            | Act::StartChat
            | Act::RetryDemo
            | Act::Playground
            | Act::Suggest { .. }
            | Act::Stop
            | Act::Close
            | Act::DelegateFate { .. }
            | Act::ClosedChats
            | Act::ChatFilter { .. }
            | Act::PlanFilter { .. }
            | Act::Filter { .. }
            | Act::OpenResult { .. }
            | Act::Decide { .. }
            | Act::CardWords { .. }
            | Act::Again { .. }
            | Act::Confirm
            | Act::Cancel
            | Act::Go { .. } => unreachable!("only composer and older acts routed here"),
        }
    }
    /// The task page's amendment acts (client/README.md, 5.11).
    pub(crate) fn amend(
        &mut self,
        env: &Env<Limits>,
        client: &Client,
        act: Act,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        match act {
            Act::Amend | Act::AmendAgain => {
                if let Some(open) = &self.amending
                    && !open.done
                    && act == Act::Amend
                {
                    return;
                }
                if let Some(task) = self.document(client)
                    && (self.shape == Shape::Tracked || self.shape == Shape::Agent)
                    && super::chat::allowed(task, c::ActKind::Amend)
                {
                    self.amending = Some(Amending::of(task, self.shape));
                }
            }
            Act::DiscardAmendment => {
                if let Some(open) = &self.amending
                    && open.done
                {
                    return;
                }
                self.amending = None;
            }
            Act::AmendWords { words } => {
                if let Some(open) = &mut self.amending
                    && !open.done
                    && !open.cut
                    && let Ok(text) = c::Text::new(words, env.limits.client.domain.text)
                {
                    open.words = text.clone().into_bytes();
                    open.draft.words(text).expect("whole specification");
                    open.no_change = false;
                }
            }
            Act::AmendTracked => {
                if let Some(open) = &mut self.amending
                    && !open.done
                    && let Some(tracking) = open.tracking
                {
                    let tracking = match tracking {
                        c::Tracking::Tracked { .. } => c::Tracking::Untracked,
                        c::Tracking::Untracked => c::Tracking::Tracked { priority: 0 },
                    };
                    open.tracking = Some(tracking);
                    open.draft.track(tracking);
                    open.no_change = false;
                }
            }
            Act::AmendPriority { priority } => {
                if let Some(open) = &mut self.amending
                    && !open.done
                    && let Some(c::Tracking::Tracked { .. }) = open.tracking
                {
                    let tracking = c::Tracking::Tracked { priority };
                    open.tracking = Some(tracking);
                    open.draft.track(tracking);
                    open.no_change = false;
                }
            }
            Act::SendAmendment => self.send(env, client, Sending::Amend, serial, out),
            Act::OpenProject { .. }
            | Act::Project { .. }
            | Act::Choice(_)
            | Act::SignOut
            | Act::Accounts(_)
            | Act::ConnectModel(_)
            | Act::ChatWords { .. }
            | Act::StartChat
            | Act::RetryDemo
            | Act::Playground
            | Act::Suggest { .. }
            | Act::MessageWords { .. }
            | Act::Send
            | Act::Stop
            | Act::Close
            | Act::DelegateFate { .. }
            | Act::Older
            | Act::ClosedChats
            | Act::ChatFilter { .. }
            | Act::PlanFilter { .. }
            | Act::Filter { .. }
            | Act::OpenResult { .. }
            | Act::Decide { .. }
            | Act::CardWords { .. }
            | Act::Again { .. }
            | Act::Confirm
            | Act::Cancel
            | Act::Go { .. } => unreachable!("only amendment acts routed here"),
        }
    }
    pub(crate) fn send(
        &mut self,
        env: &Env<Limits>,
        client: &Client,
        kind: Sending,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        let Some(task) = self.document(client) else { return };
        let ask = match kind {
            Sending::Message
                if self.shape == Shape::Agent
                    && !self.words.is_empty()
                    && super::chat::allowed(task, c::ActKind::Message) =>
            {
                c::Ask::Message {
                    project: task.project,
                    task: self.number,
                    words: c::Text::new(self.words.clone(), env.limits.client.domain.text).expect("composer bounded"),
                }
            }
            Sending::Stop if super::chat::allowed(task, c::ActKind::Stop) => {
                c::Ask::Stop { project: task.project, task: self.number }
            }
            Sending::Cancel if super::chat::allowed(task, c::ActKind::Cancel) => c::Ask::Cancel {
                project: task.project,
                task: self.number,
                reason: c::Text::new(Box::from(*b"Cancelled from its page"), u32::MAX)
                    .expect("page cancellation reason fits"),
                grace: None,
            },
            Sending::Amend if super::chat::allowed(task, c::ActKind::Amend) => {
                let Some(open) = &self.amending else { return };
                if open.done || open.draft.moved(task) {
                    return;
                }
                let Some(drafted) = open.draft.amendment() else {
                    self.amending.as_mut().expect("open amendment").no_change = true;
                    return;
                };
                c::Ask::Amend {
                    project: task.project,
                    task: self.number,
                    revision: drafted.revision,
                    change: Box::new(drafted.change),
                    reason: c::Text::new(Box::from(*b"Amended from its page"), u32::MAX).expect("page reason fits"),
                }
            }
            Sending::Message | Sending::Stop | Sending::Cancel | Sending::Amend => return,
        };
        for (_, pending) in &self.pending {
            if *pending == kind {
                return;
            }
        }
        *serial = serial.checked_add(1).expect("task keys fit");
        let key = Key { asker: Owner::Pages.asker(), number: *serial };
        if let c::Ask::Message { words, .. } = &ask {
            let Some(Latest::Jig(c::Delivery::Me(me))) = latest(client, super::me_handle()) else { return };
            if !self.thread.sending(key, me.party.number, words.clone(), 0) {
                return;
            }
            self.words = Box::default();
        }
        self.pending.push((key, kind)).expect("one request per task act fits");
        if kind == Sending::Amend {
            self.amending.as_mut().expect("open amendment").outcome = None;
        } else {
            self.outcome = None;
        }
        out.push(Request::Client(jig_client::Event::Send {
            key,
            name: None,
            request: jig_client::Ask::Jig(Box::new(ask)),
        }));
    }
    fn pending_kind(&self, key: Key) -> Option<Sending> {
        for (pending, kind) in &self.pending {
            if *pending == key {
                return Some(*kind);
            }
        }
        None
    }
    pub(crate) fn leave(&self, out: &mut Queue<Request>) -> u64 {
        for handle in [Some(self.document), Some(self.tree), Some(self.activity), self.run].into_iter().flatten() {
            unwatch(handle, out);
        }
        if let Some(ended) = &self.ended {
            unwatch(ended.handle, out);
        }
        for card in &self.cards {
            unwatch(card.handle, out);
        }
        self.next
    }
    fn answered(&mut self, key: Key, outcome: jig_client::Outcome) {
        if self.pending_kind(key) == Some(Sending::Message) {
            if let jig_client::Outcome::Answered(jig_client::Answered::Jig(answer)) = &outcome {
                self.thread.answered(key, answer);
            } else {
                self.thread.words.refused(key);
            }
        }
        let kind = self.pending_kind(key);
        // A message delivered is said by its words, "Sent", as a chat's
        // (client/README.md, 5.11); a refused or unknown one by the outcome.
        let mut said = false;
        if kind == Some(Sending::Message)
            && let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(_))) = &outcome
        {
            said = true;
        }
        if kind == Some(Sending::Amend) {
            if let Some(open) = &mut self.amending {
                if let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(_))) = &outcome {
                    open.done = true;
                }
                open.outcome = Some(outcome);
            }
        } else if !said {
            self.outcome = Some(outcome);
        }
        let mut kept = List::with_capacity(self.pending.capacity());
        for row in &self.pending {
            if row.0 != key {
                kept.push(*row).expect("remaining pending fits");
            }
        }
        self.pending = kept;
    }
    pub(crate) fn told(&mut self, now: Time, client: &Client, told: Told, out: &mut Queue<Request>) -> bool {
        if let Told::Page { handle, page: Readout::Jig(c::ReadResult::Value(value)) } = &told {
            for index in 0..self.cards.len() {
                let card = self.cards.get_mut(index).expect("bounded held card");
                if card.handle == *handle {
                    if let c::ReadValue::Task(task) = value {
                        card.project = Some(task.project);
                    }
                    card.page = Some(value.clone());
                    if self.shape == Shape::Agent && self.run.is_none() && self.thread.loaded >= self.thread.committed {
                        self.thread.end_run();
                    }
                    return true;
                }
            }
        }
        match told {
            Told::Answered { key, outcome } if self.pending_kind(key).is_some() => {
                self.answered(key, outcome);
            }
            Told::Pending { key } if self.pending_kind(key).is_some() => {}

            Told::Changed { handle } if handle == self.document => {
                if let Some(Latest::Jig(c::Delivery::Task(task))) = latest(client, handle) {
                    self.document_read = DocumentRead::Ready;
                    self.changed(task, out);
                }
            }
            Told::Ended { handle, why } if handle == self.document => {
                self.document_ended(now, client, why, out);
            }
            Told::Page { handle, page } if handle == self.document => {
                self.document_page(now, client, page, out);
            }
            Told::Page { handle, page } if Some(handle) == self.ended_handle() => {
                let ended = self.ended.as_mut().expect("ended children handle");
                ended.reading = false;
                if let Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(page))) = page {
                    ended.page = Some(page);
                }
                if ended.stale {
                    self.read_ended(out);
                }
            }
            Told::Page { handle, page } if handle == self.activity => {
                self.activity_page(client, page, out);
            }
            Told::Changed { handle } if handle == self.tree => {
                self.read_ended(out);
                match latest(client, handle) {
                    Some(Latest::Jig(c::Delivery::OwnerTree(tree))) => {
                        self.held(&tree.root, out);
                        for row in &tree.rows {
                            self.held(&row.chip, out);
                        }
                    }
                    Some(Latest::Jig(c::Delivery::Tree(tree))) => {
                        for node in &tree.nodes {
                            self.held(&node.chip, out);
                        }
                    }
                    Some(Latest::Jig(_) | Latest::Family(_)) | None => {}
                }
            }
            Told::Text { handle } if handle == self.tree => {}
            Told::Changed { handle } | Told::Text { handle } if Some(handle) == self.run => {
                if self.shape == Shape::Agent
                    && let Some(Latest::Jig(c::Delivery::RunState(run))) = latest(client, handle)
                {
                    self.thread.run(run, client.text(handle).or(run.writing.as_ref()));
                }
            }
            Told::Ended { handle, .. } if handle == self.tree || Some(handle) == self.run => {}
            Told::Link { .. }
            | Told::Pending { .. }
            | Told::Answered { .. }
            | Told::Kept { .. }
            | Told::Dropped { .. }
            | Told::Changed { .. }
            | Told::Text { .. }
            | Told::Ended { .. }
            | Told::Page { .. } => return false,
        }
        if self.shape == Shape::Agent && self.run.is_none() && self.thread.loaded >= self.thread.committed {
            self.thread.end_run();
        }
        true
    }
    fn document_ended(&mut self, now: Time, client: &Client, why: jig_client::Why, out: &mut Queue<Request>) {
        let why = match why {
            jig_client::Why::SubjectEnded | jig_client::Why::Unknown => {
                self.document_read = DocumentRead::Reading;
                read(self.document, c::Read::Task { task: self.number }, out);
                return;
            }
            jig_client::Why::NotAllowed => ReadEnd::NotAllowed,
            jig_client::Why::SignedOut => signed_out(client),
            jig_client::Why::NotServed => ReadEnd::NotServed,
            jig_client::Why::Limit => ReadEnd::Limit,
        };
        self.read_ended_at(now, why);
    }
    fn document_page(&mut self, now: Time, client: &Client, page: Readout, out: &mut Queue<Request>) {
        let why = match &page {
            Readout::Limit => ReadEnd::Limit,
            Readout::NotServed { .. } => ReadEnd::NotServed,
            Readout::Jig(c::ReadResult::None(why)) => match why {
                c::ReadNone::Busy => ReadEnd::Busy,
                c::ReadNone::Unreachable => ReadEnd::Unreachable,
                c::ReadNone::SignedOut => signed_out(client),
                c::ReadNone::Unknown => ReadEnd::Unknown,
                c::ReadNone::Refused => ReadEnd::NotAllowed,
                c::ReadNone::Gone => ReadEnd::Gone,
                c::ReadNone::TooLinked { joins } => ReadEnd::TooLinked { joins: *joins },
                c::ReadNone::TooLarge { .. } => ReadEnd::TooLarge,
                c::ReadNone::Malformed => ReadEnd::Malformed,
            },
            Readout::Jig(c::ReadResult::Value(_)) | Readout::Family { .. } => ReadEnd::Malformed,
        };
        if let Some(task) = super::chat::page_task(page, self.number) {
            self.document_read = DocumentRead::Ready;
            self.changed(&task, out);
            self.final_task = Some(task);
        } else {
            self.read_ended_at(now, why);
        }
    }
    fn read_ended_at(&mut self, now: Time, why: ReadEnd) {
        let retry_at = if why.transient() { Some(now.saturating_add(self.read_wait)) } else { None };
        self.document_read = DocumentRead::Ended { why, retry_at };
        if retry_at.is_some() {
            self.read_wait = self.read_wait.saturating_mul(2).min(READ_MOST);
        }
    }
    pub(crate) fn link(&mut self, link: jig_client::Link) {
        let why = match link {
            jig_client::Link::SignedOut => ReadEnd::SignedOut,
            jig_client::Link::OutOfDate => ReadEnd::NotServed,
            jig_client::Link::Starting | jig_client::Link::Live | jig_client::Link::Offline => return,
        };
        self.document_read = DocumentRead::Ended { why, retry_at: None };
    }
    pub(crate) fn fire(&mut self, now: Time, out: &mut Queue<Request>) {
        if let Some(at) = self.next_deadline()
            && at <= now
        {
            self.document_read = DocumentRead::Reading;
            read(self.document, c::Read::Task { task: self.number }, out);
        }
    }
    #[must_use]
    pub(crate) const fn next_deadline(&self) -> Option<Time> {
        match self.document_read {
            DocumentRead::Ended { retry_at, .. } => retry_at,
            DocumentRead::Reading | DocumentRead::Ready => None,
        }
    }
    fn activity_page(&mut self, client: &Client, page: Readout, out: &mut Queue<Request>) {
        if let Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(page))) = page {
            if self.shape == Shape::Agent {
                if let Some(reading) = self.thread.page(*page) {
                    read(self.activity, reading, out);
                }
            } else {
                self.latest = Some(page);
            }
        } else if let Readout::Jig(c::ReadResult::None(c::ReadNone::SignedOut)) = page
            && client.link().state == jig_client::Link::Starting
            && super::awaits_first_read(&self.thread)
            && self.latest.is_none()
        {
            read(self.activity, c::Read::Activity { task: self.number, before: None }, out);
        } else {
            self.thread.failed();
        }
    }
    pub(crate) fn held(&mut self, chip: &c::Chip, out: &mut Queue<Request>) {
        if chip.phase != c::Phase::Held {
            return;
        }
        let object = c::ObjectKey::Task(chip.task);
        for index in 0..self.cards.len() {
            let card = self.cards.get_mut(index).expect("bounded held card");
            if card.object != object {
                continue;
            }
            // A card read for an earlier hold is read again for this one, so a
            // task held anew offers Release anew (client/README.md, 5.11).
            if card.key.is_none()
                && let (Some(was), Some(now)) = (hold_revision(card.page.as_ref()), chip_hold(chip))
                && was < now
            {
                unwatch(card.handle, out);
                card.handle = handle(self.next);
                self.next = self.next.checked_add(1).expect("task handles fit");
                card.page = None;
                card.words = None;
                card.outcome = None;
                read(card.handle, c::Read::Task { task: chip.task }, out);
            }
            return;
        }
        if self.cards.len() == self.cards.capacity() {
            return;
        }
        let handle = handle(self.next);
        self.next = self.next.checked_add(1).expect("task handles fit");
        self.cards
            .push(Card {
                object,
                group: c::InboxGroup::Escalations,
                project: None,
                position: None,
                unread: None,
                handle,
                page: None,
                words: None,
                key: None,
                outcome: None,
            })
            .expect("held cards bounded");
        read(handle, c::Read::Task { task: chip.task }, out);
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "the ended-handle accessor uses a stateless projection of its optional state"
    )]
    fn ended_handle(&self) -> Option<Handle> {
        self.ended.as_ref().map(|ended| ended.handle)
    }
    fn read_ended(&mut self, out: &mut Queue<Request>) {
        let Some(ended) = &mut self.ended else { return };
        if ended.reading {
            ended.stale = true;
            return;
        }
        ended.reading = true;
        ended.stale = false;
        read(
            ended.handle,
            c::Read::Tasks {
                query: c::TaskQuery {
                    project: None,
                    requester: None,
                    root: None,
                    parent: Some(self.number),
                    label: None,
                    phase: Some(c::PhaseClass::Ended),
                    executor: None,
                    tracked: None,
                    order: c::QueryOrder::Ended,
                },
                live: None,
                ended: None,
            },
            out,
        );
    }
    fn changed(&mut self, task: &c::Task, out: &mut Queue<Request>) {
        let reads_ended = match task.chip.executor {
            c::Executor::Procedure { .. } => true,
            c::Executor::Agent { .. } => task.priority.is_some(),
            c::Executor::Party { .. } | c::Executor::Role { .. } => false,
        };
        if reads_ended {
            if self.ended.is_none() {
                let handle = handle(self.next);
                self.next = self.next.checked_add(1).expect("task handles fit");
                self.ended = Some(Ended { handle, page: None, reading: false, stale: false });
            }
            self.read_ended(out);
        }
        self.shape = match task.chip.executor {
            c::Executor::Agent { .. } if task.priority.is_some() => Shape::Tracked,
            c::Executor::Agent { .. } => Shape::Agent,
            c::Executor::Procedure { .. } | c::Executor::Party { .. } | c::Executor::Role { .. } => {
                Shape::HeaderAndPlan
            }
        };
        if self.shape == Shape::Agent {
            if let Some(page) = self.latest.take()
                && let Some(reading) = self.thread.page(*page)
            {
                read(self.activity, reading, out);
            }
            if let Some(reading) = self.thread.document(task) {
                read(self.activity, reading, out);
            }
        } else {
            if let Some(through) = self.through
                && through != task.activity
            {
                read(self.activity, c::Read::Activity { task: self.number, before: None }, out);
            }
            self.through = Some(task.activity);
        }
        let running = match task.chip.details.as_ref() {
            c::ChipPhase::Owner {
                phase:
                    c::OwnerChipPhase::Active(c::RunStage::Claimed { .. } | c::RunStage::Running { .. })
                    | c::OwnerChipPhase::Closing(c::ClosingStage::Run { .. }),
                ..
            }
            | c::ChipPhase::Running { .. }
            | c::ChipPhase::WaitingForWords { .. } => true,
            c::ChipPhase::Owner { .. }
            | c::ChipPhase::Active(_)
            | c::ChipPhase::Waiting { .. }
            | c::ChipPhase::Idle { .. }
            | c::ChipPhase::Starting
            | c::ChipPhase::BackingOff { .. }
            | c::ChipPhase::Procedure { .. }
            | c::ChipPhase::Person { .. }
            | c::ChipPhase::Closing(_)
            | c::ChipPhase::Held { .. }
            | c::ChipPhase::Ended { .. } => false,
        };
        if running && self.run.is_none() {
            let handle = handle(self.next);
            self.next = self.next.checked_add(1).expect("task handles fit");
            self.run = Some(handle);
            out.push(Request::Client(jig_client::Event::Watch {
                handle,
                subject: Watchable::Jig(c::Subject::Run { task: self.number, turn: None, offset: 0 }),
            }));
        } else if !running && let Some(handle) = self.run.take() {
            unwatch(handle, out);
        }
    }
    pub(crate) fn chat(&self, client: &Client, me: Option<&c::Me>) -> Option<Address> {
        let task = self.document(client)?;
        let template = jig_client::shapes::template(me?, task.project, jig_client::shapes::CHAT)?;
        if task.label == template.label { Some(Address::Chat { task: self.number }) } else { None }
    }
}
fn signed_out(client: &Client) -> ReadEnd {
    match client.link().state {
        jig_client::Link::SignedOut => ReadEnd::SignedOut,
        jig_client::Link::Starting | jig_client::Link::Live | jig_client::Link::Offline => ReadEnd::SignIn,
        jig_client::Link::OutOfDate => ReadEnd::NotServed,
    }
}
fn handle(number: u64) -> Handle {
    Handle { asker: Owner::Pages.asker(), number }
}
fn unwatch(handle: Handle, out: &mut Queue<Request>) {
    out.push(Request::Client(jig_client::Event::Unwatch { handle }));
}
fn read(handle: Handle, read: c::Read, out: &mut Queue<Request>) {
    out.push(Request::Client(jig_client::Event::Read { handle, read: Readable::Jig(read) }));
}
/// The hold's revision a chip says, if it is held.
fn chip_hold(chip: &c::Chip) -> Option<u64> {
    match chip.details.as_ref() {
        c::ChipPhase::Held { revision, .. } => Some(revision.value),
        c::ChipPhase::Owner { phase: c::OwnerChipPhase::Held { revision, .. }, .. } => Some(*revision),
        c::ChipPhase::Owner { .. }
        | c::ChipPhase::Active(_)
        | c::ChipPhase::Waiting { .. }
        | c::ChipPhase::Idle { .. }
        | c::ChipPhase::Starting
        | c::ChipPhase::Running { .. }
        | c::ChipPhase::WaitingForWords { .. }
        | c::ChipPhase::BackingOff { .. }
        | c::ChipPhase::Procedure { .. }
        | c::ChipPhase::Person { .. }
        | c::ChipPhase::Closing(_)
        | c::ChipPhase::Ended { .. } => None,
    }
}
/// The hold's revision a held card's read task carries, if any.
fn hold_revision(page: Option<&c::ReadValue>) -> Option<u64> {
    if let Some(c::ReadValue::Task(task)) = page {
        match &task.hold {
            Some(c::TaskHold::Owner(hold)) => Some(hold.revision),
            Some(c::TaskHold::Document(hold)) => Some(hold.revision.value),
            None => None,
        }
    } else {
        None
    }
}
fn latest(client: &Client, handle: Handle) -> Option<&Latest> {
    client.document(handle)?.latest
}
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    // Activity, the final task, the last outcome, and bounded held-task cards and their words.
    // Three more document bounds cover the draft base (spec, authority, instructions,
    // dependencies and model), its replacement spec, and its retained outcome.
    // The editor also keeps one bounded specification field and its inline state.
    // A goal or procedure also keeps one page of ended delegates and its read state.
    // An agent's task also retains a chat's thread and its borrowed view lines.
    limits
        .client
        .domain
        .document_bytes
        .checked_mul(u64::from(limits.client.domain.reads).checked_add(7)?)?
        .checked_add(List::<Card>::worst_case(limits.client.domain.reads)?)?
        .checked_add(List::<(Key, Sending)>::worst_case(4)?)?
        .checked_add(u64::try_from(size_of::<Amending>()).ok()?)?
        .checked_add(u64::try_from(size_of::<Ended>()).ok()?)?
        .checked_add(u64::from(limits.client.domain.text).checked_mul(2)?)?
        .checked_add(u64::from(limits.client.domain.reads).checked_mul(u64::from(limits.client.domain.text))?)?
        .checked_add(jig_client::thread::worst_case(&super::chat::thread_limits(limits))?)?
        .checked_add(List::<jig_client::thread::Line<'_>>::worst_case(
            limits.client.domain.rows.checked_mul(4)?.checked_add(1)?,
        )?)
}
