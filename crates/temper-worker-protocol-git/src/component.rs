//! The git component: counted operations, explicit lifecycle, parent queues.
use crate::{
    Event, Identity, Limits, Outputs, Request, Settings, Unfit, Unusable, Version,
    files::{Action, Directories, Task},
    invocation::Command,
    limits,
    operation::Operation,
    sequence::{self, Next},
};
use alloc::boxed::Box;
use skein_io::{self as io, kernel};
use skein_lib::{Id, Map, Slab, Time, Token};
use temper_worker_domain_checkout::{self as checkout, git};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Start {
    Idle,
    Checking,
    Layout,
    Started,
    Unusable,
    Aborted,
}
/// One worker's git component; the service drives it and drains its outputs.
#[derive(Debug)]
pub struct Component {
    pub(crate) limits: Limits,
    pub(crate) checkout: checkout::Limits,
    pub(crate) settings: Settings,
    slots: Slab<Operation>,
    index: Map<Token, Id<Operation>>,
    serial: u64,
    start: Start,
    shutting_down: bool,
    directories: Directories,
    background: Option<Task>,
    checked: Option<(Box<[u8]>, Option<Version>)>,
}
fn usable(version: Option<Version>) -> bool {
    version >= Some(Version { major: 2, minor: 41, patch: 0 })
}
fn event(out: &mut Outputs, event: Event) {
    out.events.push(event);
}
impl Component {
    /// Refuse the limit that breaks protocol/git.md section 8 before
    /// allocating state.
    pub fn new(limits: &Limits, checkout: &checkout::Limits, settings: Settings) -> Result<Component, Unfit> {
        limits::check(limits, checkout)?;
        let capacity = limits.operations.checked_add(1).ok_or(Unfit::Memory)?;
        let slots = capacity.checked_mul(2).ok_or(Unfit::Memory)?;
        assert!(settings.largest_read > 0 && settings.largest_room > 0, "io validated usable limits");
        let directories = Directories::new(settings.serial);
        Ok(Component {
            limits: *limits,
            checkout: *checkout,
            settings,
            slots: Slab::with_capacity(slots),
            index: Map::with_capacity(capacity),
            serial: 0,
            start: Start::Idle,
            shutting_down: false,
            directories,
            background: None,
            checked: None,
        })
    }
    fn token(&mut self) -> Option<Token> {
        self.serial = self.serial.checked_add(1)?;
        if self.serial >= (1_u64 << 48_u32) {
            return None;
        }
        Some(Token::new(self.settings.prefix.checked_add(self.serial)?))
    }
    fn insert(&mut self, operation: Operation) -> Id<Operation> {
        let key = operation.io_owner;
        let id = self.slots.insert(operation).expect("live plus retired operations fit the slab");
        assert!(self.index.insert(key, id).expect("admitted index fits").is_none(), "unique invocation token");
        id
    }
    /// Start once with git's version check followed by a private, fresh layout.
    pub fn start(&mut self, _now: Time, out: &mut Outputs) {
        assert_eq!(self.start, Start::Idle, "start once");
        if self.settings.git.is_none() {
            self.start = Start::Layout;
            self.background(Task::layout(), out);
            return;
        }
        self.start = Start::Checking;
        let token = self.token().expect("first invocation token");
        let mut operation = Operation::new(token, token, Time::from_nanos(u64::MAX), None, None, Command::Version);
        operation.spawn(token, Command::Version, &self.settings, out);
        self.insert(operation);
    }
    fn owned(&self, owner: Token) -> Option<Id<Operation>> {
        for (_, id) in &self.index {
            if self.slots.get(*id).expect("live index").owner == owner {
                return Some(*id);
            }
        }
        None
    }
    /// Run an operation or cancel the operation named by its echoed owner.
    pub fn down(&mut self, now: Time, request: Request, out: &mut Outputs) {
        match request {
            Request::Io { owner, op, deadline, identity } => self.begin(now, owner, op, deadline, identity, out),
            Request::CancelIo { owner } => {
                if let Some(id) = self.owned(owner) {
                    self.slots.get_mut(id).expect("live index").cancel(git::Fault::Cancelled, &self.checkout, out);
                }
            }
            Request::Shutdown => {
                self.shutting_down = true;
                self.maintain(false, out);
            }
        }
    }
    fn begin(
        &mut self,
        now: Time,
        owner: Token,
        op: git::Op,
        deadline: Time,
        identity: Option<Identity>,
        out: &mut Outputs,
    ) {
        let needs_identity = match op {
            git::Op::Clone { .. }
            | git::Op::Fetch { .. }
            | git::Op::Create { .. }
            | git::Op::Commit { .. }
            | git::Op::Push { .. } => true,
            git::Op::Make { .. } | git::Op::CheckOut { .. } | git::Op::IsAncestor { .. } | git::Op::Merge { .. } => {
                false
            }
        };
        if self.start != Start::Started
            || self.shutting_down
            || self.index.len() >= self.limits.operations
            || self.slots.is_full()
            || self.owned(owner).is_some()
            || (needs_identity && identity.is_none())
        {
            event(out, Event::Done { owner, done: git::Done::Failed { fault: git::Fault::Broken } });
            return;
        }
        if now >= deadline {
            event(out, Event::Done { owner, done: git::Done::Failed { fault: git::Fault::TimedOut } });
            return;
        }
        let Some(token) = self.token() else {
            event(out, Event::Done { owner, done: git::Done::Failed { fault: git::Fault::Broken } });
            return;
        };
        let mut next = sequence::first(&op);
        // Concurrent first uses check independently; only completed answers
        // are reused, and the last completed check replaces the cache.
        if let Some(program) = crate::invocation::item_program(&op) {
            next = match &self.checked {
                Some((checked, answer)) if checked.as_ref() == program => {
                    if usable(*answer) {
                        next
                    } else {
                        Next::End(git::Done::Failed { fault: git::Fault::Broken })
                    }
                }
                Some(_) | None => Next::Run(Command::Version),
            };
        }
        let identity = if needs_identity { identity } else { None };
        let operation = Operation::new(owner, token, deadline, Some(op), identity, Command::Version);
        let id = self.insert(operation);
        self.advance(id, next, out);
    }
    /// Route only events accepted by the invocation or one of its pipe tokens.
    pub fn up(&mut self, now: Time, input: io::Event, out: &mut Outputs) {
        let Some(owner) = io_owner(&input) else {
            return;
        };
        let mut found = None;
        for (_, id) in &self.index {
            if self.slots.get(*id).expect("live index").accepts(owner) {
                found = Some(*id);
                break;
            }
        }
        let Some(id) = found else {
            return;
        };
        let operation = self.slots.get_mut(id).expect("live index");
        let startup = operation.op.is_none();
        let checking = operation.command == Command::Version;
        let version = operation.found_version();
        if !startup && now >= operation.deadline {
            operation.cancel(git::Fault::TimedOut, &self.checkout, out);
        }
        let next = operation.up(input, &self.settings, &self.limits, &self.checkout, out);
        if let Some(next) = next {
            if startup {
                let aborted = operation.aborted;
                self.retire(id);
                if aborted {
                    self.start = Start::Aborted;
                    self.maintain(false, out);
                    return;
                }
                let program = self.settings.git.as_ref().expect("startup checked a configured program");
                self.checked = Some((program.clone(), version));
                if usable(version) {
                    self.start = Start::Layout;
                    self.background(Task::layout(), out);
                } else {
                    self.start = Start::Unusable;
                    event(out, Event::Unusable { why: Unusable::Version { found: version } });
                }
            } else {
                let next = if checking {
                    if let Some(fault) = operation.stop {
                        Next::End(git::Done::Failed { fault })
                    } else {
                        let op = operation.op.as_ref().expect("item operation");
                        let program = crate::invocation::item_program(op).expect("item version check");
                        self.checked = Some((Box::from(program), version));
                        operation.ran = false;
                        if usable(version) {
                            sequence::first(op)
                        } else {
                            Next::End(git::Done::Failed { fault: git::Fault::Broken })
                        }
                    }
                } else {
                    next
                };
                self.advance(id, next, out);
                self.maintain(false, out);
            }
        }
    }
    fn retire(&mut self, id: Id<Operation>) {
        let operation = self.slots.get_mut(id).expect("live index");
        operation.identity = None;
        operation.op = None;
        operation.command = Command::Version;
        operation.file = None;
        let key = operation.io_owner;
        self.index.remove(&key).expect("live index");
        self.slots.retire(id);
    }
    fn advance(&mut self, id: Id<Operation>, next: Next, out: &mut Outputs) {
        match next {
            Next::Run(command) => {
                let operation = self.slots.get(id).expect("live index");
                if operation.program(&self.settings).is_none() {
                    let op = operation.op.as_ref().expect("no program is an operation failure");
                    let diagnostic = crate::invocation::no_program(op, &command);
                    let next = sequence::after(op, &command, 128, b"", &diagnostic, 0, &self.checkout);
                    let Next::End(done) = next else { unreachable!("a fatal exit ends the operation") };
                    self.finish(id, done, out);
                    return;
                }
                let Some(token) = self.token() else {
                    self.finish(
                        id,
                        git::Done::FailedWithOutput {
                            fault: git::Fault::Broken,
                            diagnostic: git::PushDiagnostic::empty(),
                        },
                        out,
                    );
                    return;
                };
                let operation = self.slots.get_mut(id).expect("live index");
                let old = operation.io_owner;
                self.index.remove(&old).expect("live index");
                operation.spawn(token, command, &self.settings, out);
                assert!(self.index.insert(token, id).expect("same operation").is_none(), "unique token");
            }
            Next::End(done) => self.finish(id, done, out),
            Next::Make => {
                let op = self.slots.get(id).expect("live index").op.as_ref().expect("Make op");
                let workspace = match op {
                    git::Op::Make { workspace } => *workspace,
                    git::Op::Clone { .. }
                    | git::Op::Fetch { .. }
                    | git::Op::Create { .. }
                    | git::Op::CheckOut { .. }
                    | git::Op::IsAncestor { .. }
                    | git::Op::Merge { .. }
                    | git::Op::Commit { .. }
                    | git::Op::Push { .. } => unreachable!("Make sequence"),
                };
                self.file_start(id, Task::make(workspace), out);
            }
            Next::Scan { files } => {
                let op = self.slots.get(id).expect("live index").op.as_ref().expect("Commit op");
                let at = match op {
                    git::Op::Commit { at, .. } => at,
                    git::Op::Make { .. }
                    | git::Op::Clone { .. }
                    | git::Op::Fetch { .. }
                    | git::Op::Create { .. }
                    | git::Op::CheckOut { .. }
                    | git::Op::IsAncestor { .. }
                    | git::Op::Merge { .. }
                    | git::Op::Push { .. } => unreachable!("scan sequence"),
                };
                self.file_start(id, Task::scan(files, crate::invocation::place(at)), out);
            }
        }
    }
    fn finish(&mut self, id: Id<Operation>, done: git::Done, out: &mut Outputs) {
        let operation = self.slots.get(id).expect("live index");
        if !operation.aborted {
            event(out, Event::Done { owner: operation.owner, done });
        }
        self.retire(id);
    }
    fn submit(&mut self, task: &mut Task, kind: kernel::Op, out: &mut Outputs) {
        task.pending = self.token().expect("file token lifetime");
        out.kernel.push(kernel::Submit { op: task.pending, kind });
    }
    fn background(&mut self, mut task: Task, out: &mut Outputs) {
        let action = task.first(&mut self.directories, &self.settings);
        self.background_action(task, action, out);
    }
    fn background_action(&mut self, mut task: Task, action: Action, out: &mut Outputs) {
        match action {
            Action::Run(kind) => {
                self.submit(&mut task, kind, out);
                self.background = Some(task);
            }
            Action::Layout(result) => {
                if self.start != Start::Aborted {
                    match result {
                        Ok(()) => {
                            self.start = Start::Started;
                            event(out, Event::Started);
                        }
                        Err(error) => {
                            self.start = Start::Unusable;
                            event(out, Event::Unusable { why: Unusable::Layout { error } });
                        }
                    }
                }
                self.maintain(result.is_ok(), out);
            }
            Action::Swept(failed) => {
                if failed {
                    self.directories.failures = self.directories.failures.saturating_add(1);
                }
                self.maintain(false, out);
            }
            Action::Closed => {}
            Action::Operation(_) => unreachable!("background file task"),
        }
    }
    fn maintain(&mut self, sweep: bool, out: &mut Outputs) {
        if self.background.is_some() {
            return;
        }
        if (self.shutting_down || self.start == Start::Unusable) && self.index.is_empty() {
            self.background(Task::closing(), out);
        } else if sweep && !self.shutting_down && self.start == Start::Started {
            self.background(Task::sweep(), out);
        }
    }
    fn file_start(&mut self, id: Id<Operation>, mut task: Task, out: &mut Outputs) {
        let action = task.first(&mut self.directories, &self.settings);
        self.file_action(id, task, action, out);
    }
    fn file_action(&mut self, id: Id<Operation>, mut task: Task, action: Action, out: &mut Outputs) {
        match action {
            Action::Run(kind) => {
                self.submit(&mut task, kind, out);
                let operation = self.slots.get_mut(id).expect("live index");
                self.index.remove(&operation.io_owner).expect("live index");
                operation.io_owner = task.pending;
                assert!(self.index.insert(task.pending, id).expect("same operation").is_none(), "unique token");
                operation.file = Some(task);
            }
            Action::Operation(mut next) => {
                let operation = self.slots.get(id).expect("live index");
                if let Some(fault) = operation.stop {
                    next = if operation.ran {
                        sequence::failed(fault, b"", 0)
                    } else {
                        Next::End(git::Done::Failed { fault })
                    };
                }
                let make = operation.op.as_ref().expect("file op").kind() == git::Kind::Make;
                self.advance(id, next, out);
                self.maintain(make, out);
            }
            Action::Layout(_) | Action::Swept(_) | Action::Closed => unreachable!("operation file task"),
        }
    }
    /// Settle owned kernel work before advancing or answering its caller.
    pub fn complete(&mut self, now: Time, complete: kernel::Complete, out: &mut Outputs) {
        let background = match &self.background {
            Some(task) => task.pending == complete.op,
            None => false,
        };
        if background {
            let mut task = self.background.take().expect("pending task");
            let action =
                task.complete(complete, &mut self.directories, &self.settings, &self.limits, self.shutting_down);
            self.background_action(task, action, out);
            return;
        }
        let Some(id) = self.index.get(&complete.op).copied() else {
            return;
        };
        let operation = self.slots.get_mut(id).expect("live index");
        let Some(mut task) = operation.file.take() else {
            return;
        };
        if now >= operation.deadline && operation.stop.is_none() {
            operation.stop = Some(git::Fault::TimedOut);
        }
        let action =
            task.complete(complete, &mut self.directories, &self.settings, &self.limits, operation.stop.is_some());
        self.file_action(id, task, action, out);
    }
    /// Kill and settle at most one operation whose one deadline has elapsed.
    pub fn fire(&mut self, now: Time, out: &mut Outputs) {
        for (_, id) in &self.index {
            let operation = self.slots.get_mut(*id).expect("live index");
            if operation.op.is_some() && operation.stop.is_none() && operation.deadline <= now {
                operation.cancel(git::Fault::TimedOut, &self.checkout, out);
                break;
            }
        }
    }
    /// Abort each child and pipe; retain a pending spawn until it can settle.
    pub fn abort(&mut self, out: &mut Outputs) {
        self.start = Start::Aborted;
        self.shutting_down = true;
        for (_, id) in &self.index {
            let operation = self.slots.get_mut(*id).expect("live index");
            operation.aborted = true;
            operation.identity = None;
            operation.cancel(git::Fault::Cancelled, &self.checkout, out);
        }
        self.maintain(false, out);
    }
    #[must_use]
    pub fn accepts(&self, event: &io::Event) -> bool {
        let Some(owner) = io_owner(event) else {
            return false;
        };
        for (_, id) in &self.index {
            if self.slots.get(*id).expect("live index").accepts(owner) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn owns(&self, operation: Token) -> bool {
        operation.raw().checked_sub(self.settings.prefix).unwrap_or(u64::MAX) < (1_u64 << 48_u32)
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut earliest = None;
        for (_, id) in &self.index {
            let operation = self.slots.get(*id).expect("live index");
            if operation.op.is_some()
                && operation.stop.is_none()
                && (earliest.is_none() || Some(operation.deadline) < earliest)
            {
                earliest = Some(operation.deadline);
            }
        }
        earliest
    }
    #[must_use]
    pub fn is_due(&self, now: Time) -> bool {
        match self.next_deadline() {
            Some(deadline) => deadline <= now,
            None => false,
        }
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
            && self.background.is_none()
            && self.directories.trash.is_none()
            && self.directories.workspaces.is_none()
    }
    #[must_use]
    pub fn operations(&self) -> u32 {
        limits::operations(&self.limits).expect("validated operation count")
    }
    /// The configured component bounds used to price its parent layer.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Reclaim retired state after the loop's down pass.
    pub fn reclaim(&mut self) {
        self.slots.reclaim();
    }
}
fn io_owner(event: &io::Event) -> Option<Token> {
    match event {
        io::Event::Listening { owner, .. }
        | io::Event::Accepted { owner, .. }
        | io::Event::Connecting { owner, .. }
        | io::Event::Connected { owner, .. }
        | io::Event::Stream { owner, .. }
        | io::Event::Output { owner, .. }
        | io::Event::Spawned { owner, .. }
        | io::Event::Exited { owner, .. }
        | io::Event::Usage { owner, .. }
        | io::Event::Window { owner, .. }
        | io::Event::Failed { owner, .. }
        | io::Event::Closed { owner } => Some(*owner),
        io::Event::Resized | io::Event::Shutdown { .. } => None,
    }
}
