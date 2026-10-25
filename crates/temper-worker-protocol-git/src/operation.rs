//! One operation owns its identity until its terminal, and one child at a time.
use crate::{
    Identity, Limits, Outputs, Settings, Version,
    capped::Capped,
    invocation::{self, Command},
    output,
    ring::ByteRing,
    sequence::{self, Next},
};
use alloc::boxed::Box;
use skein_io::{self as io, kernel};
use skein_lib::{Time, Token, stream};
use temper_worker_domain_checkout::{self as checkout, git};

#[derive(Debug)]
pub(crate) struct Operation {
    pub(crate) owner: Token,
    pub(crate) io_owner: Token,
    pub(crate) deadline: Time,
    pub(crate) op: Option<git::Op>,
    pub(crate) identity: Option<Identity>,
    pub(crate) command: Command,
    pub(crate) stop: Option<git::Fault>,
    pub(crate) aborted: bool,
    pub(crate) ran: bool,
    pub(crate) file: Option<crate::files::Task>,
    phase: Phase,
}
#[derive(Debug)]
enum Phase {
    Spawning { failed: bool },
    Active(Child),
    Settled,
}
#[derive(Debug)]
struct Child {
    token: Token,
    pipes: [Option<Token>; 3],
    ended: [bool; 3],
    closed: [bool; 3],
    exited: Option<kernel::Exit>,
    closing: bool,
    stdout: Capped,
    stderr: ByteRing,
    dropped: u64,
    message: Box<[u8]>,
    sent: usize,
}
fn emit(out: &mut Outputs, request: io::Request) {
    out.io.push(request);
}
fn demand(token: Token, read: stream::Read, room: u32, out: &mut Outputs) {
    emit(out, io::Request::Stream { stream: token, down: stream::Down::Demand { read, room } });
}
impl Operation {
    pub fn new(
        owner: Token,
        io_owner: Token,
        deadline: Time,
        op: Option<git::Op>,
        identity: Option<Identity>,
        command: Command,
    ) -> Operation {
        Operation {
            owner,
            io_owner,
            deadline,
            op,
            identity,
            command,
            stop: None,
            aborted: false,
            ran: false,
            file: None,
            phase: Phase::Spawning { failed: false },
        }
    }
    pub fn spawn(&mut self, io_owner: Token, command: Command, settings: &Settings, out: &mut Outputs) {
        self.io_owner = io_owner;
        self.command = command;
        self.phase = Phase::Spawning { failed: false };
        let (directory, kind) = if self.command == Command::Version {
            (Box::from(&b""[..]), None)
        } else {
            match &self.op {
                Some(op) => match op {
                    git::Op::Clone { at, .. } => (
                        invocation::joined(&[b"workspaces/", skein_lib::Decimal::of(at.workspace.raw()).as_bytes()]),
                        Some(op.kind()),
                    ),
                    git::Op::Make { .. } => (Box::from(&b""[..]), Some(op.kind())),
                    git::Op::Fetch { at, .. }
                    | git::Op::Create { at, .. }
                    | git::Op::CheckOut { at, .. }
                    | git::Op::IsAncestor { at, .. }
                    | git::Op::Merge { at, .. }
                    | git::Op::Commit { at, .. }
                    | git::Op::Push { at, .. } => (invocation::place(at), Some(op.kind())),
                },
                None => (Box::from(&b""[..]), None),
            }
        };
        let mut pipes = skein_lib::List::with_capacity(3);
        if self.is_commit_tree() {
            pipes.push(kernel::Pipe { child: 0, way: kernel::Way::In, parent: None }).expect("three pipes");
        }
        for child in [1, 2] {
            pipes.push(kernel::Pipe { child, way: kernel::Way::Out, parent: None }).expect("three pipes");
        }
        emit(
            out,
            io::Request::Spawn {
                owner: io_owner,
                spawn: kernel::Spawn {
                    program: Box::from(self.program(settings).expect("program checked before spawning")),
                    args: invocation::arguments(&self.command),
                    env: invocation::environment(&settings.home, kind, self.identity.as_ref()),
                    root: settings.root,
                    dir: directory,
                    pipes: pipes.into_boxed(),
                },
            },
        );
    }
    pub fn program<'a>(&'a self, settings: &'a Settings) -> Option<&'a [u8]> {
        if let Some(op) = &self.op
            && let Some(program) = invocation::item_program(op)
        {
            return Some(program);
        }
        settings.git.as_deref()
    }
    fn is_commit_tree(&self) -> bool {
        match self.command {
            Command::CommitTree { .. } => true,
            Command::Version
            | Command::Clone { .. }
            | Command::Fetch { .. }
            | Command::Create { .. }
            | Command::CheckOut { .. }
            | Command::Clean
            | Command::Merge { .. }
            | Command::Unmerged
            | Command::Add
            | Command::DiffIndex { .. }
            | Command::WriteTree
            | Command::IsAncestor { .. }
            | Command::Push { .. } => false,
        }
    }
    pub fn accepts(&self, owner: Token) -> bool {
        if self.io_owner == owner {
            return true;
        }
        match &self.phase {
            Phase::Active(child) => child.token == owner || child.pipes.contains(&Some(owner)),
            Phase::Spawning { .. } | Phase::Settled => false,
        }
    }
    pub fn cancel(&mut self, fault: git::Fault, checkout: &checkout::Limits, out: &mut Outputs) {
        if self.stop.is_some() && !self.aborted {
            return;
        }
        if fault == git::Fault::Cancelled
            && !self.aborted
            && self.command != Command::Version
            && let Some(next) = self.ready(checkout)
        {
            match next {
                Next::End(_) => return,
                Next::Run(_) | Next::Make | Next::Scan { .. } => {}
            }
        }
        if self.stop.is_none() {
            self.stop = Some(fault);
        }
        match &mut self.phase {
            Phase::Active(child) => child.close(self.aborted, out),
            Phase::Spawning { .. } | Phase::Settled => {}
        }
    }
    pub fn ready(&self, checkout: &checkout::Limits) -> Option<Next> {
        match &self.phase {
            Phase::Active(child) if child.exited.is_some() && child.outputs_ended() => {
                Some(self.result(child, checkout))
            }
            Phase::Active(_) | Phase::Spawning { .. } | Phase::Settled => None,
        }
    }
    fn result(&self, child: &Child, checkout: &checkout::Limits) -> Next {
        let tail = child.stderr.tail();
        if let Some(fault) = self.stop {
            return sequence::failed(fault, &tail, child.stderr.dropped());
        }
        let stdout = child.stdout.as_bytes();
        if child.dropped != 0 || (!stdout.is_empty() && stdout.last().copied() != Some(self.delimiter())) {
            return sequence::failed(git::Fault::Broken, &tail, child.stderr.dropped());
        }
        match child.exited {
            Some(kernel::Exit::Code(code)) => match &self.op {
                Some(op) => sequence::after(op, &self.command, code, stdout, &tail, child.stderr.dropped(), checkout),
                None => sequence::failed(git::Fault::Broken, &tail, child.stderr.dropped()),
            },
            Some(kernel::Exit::Signal(_)) | None => sequence::failed(git::Fault::Broken, &tail, child.stderr.dropped()),
        }
    }
    fn delimiter(&self) -> u8 {
        match self.command {
            Command::Unmerged => 0,
            Command::Version
            | Command::Clone { .. }
            | Command::Fetch { .. }
            | Command::Create { .. }
            | Command::CheckOut { .. }
            | Command::Clean
            | Command::Merge { .. }
            | Command::Add
            | Command::DiffIndex { .. }
            | Command::WriteTree
            | Command::CommitTree { .. }
            | Command::IsAncestor { .. }
            | Command::Push { .. } => b'\n',
        }
    }
    pub fn up(
        &mut self,
        event: io::Event,
        settings: &Settings,
        limits: &Limits,
        checkout: &checkout::Limits,
        out: &mut Outputs,
    ) -> Option<Next> {
        let phase = core::mem::replace(&mut self.phase, Phase::Settled);
        match phase {
            Phase::Spawning { failed } => self.spawning(event, failed, settings, limits, out),
            Phase::Active(child) => self.active(event, child, settings, checkout, out),
            Phase::Settled => None,
        }
    }
    fn spawning(
        &mut self,
        event: io::Event,
        mut failed: bool,
        settings: &Settings,
        limits: &Limits,
        out: &mut Outputs,
    ) -> Option<Next> {
        let delimiter = self.delimiter();
        let commit = self.is_commit_tree();
        self.phase = match event {
            io::Event::Spawned { owner, child, pipes } if owner == self.io_owner => {
                let message = match &self.op {
                    Some(git::Op::Commit { title, body, .. }) if commit => invocation::message(title, body),
                    Some(
                        git::Op::Make { .. }
                        | git::Op::Clone { .. }
                        | git::Op::Fetch { .. }
                        | git::Op::Create { .. }
                        | git::Op::CheckOut { .. }
                        | git::Op::IsAncestor { .. }
                        | git::Op::Merge { .. }
                        | git::Op::Commit { .. }
                        | git::Op::Push { .. },
                    )
                    | None => Box::from(&b""[..]),
                };
                assert_eq!(pipes.len(), if commit { 3 } else { 2 }, "requested standard pipes");
                let mut child = Child {
                    token: child,
                    pipes: if commit {
                        [pipes.first().copied(), pipes.get(1).copied(), pipes.get(2).copied()]
                    } else {
                        [None, pipes.first().copied(), pipes.get(1).copied()]
                    },
                    ended: [!commit, false, false],
                    closed: [!commit, false, false],
                    exited: None,
                    closing: false,
                    stdout: Capped::new(limits.stdout_bytes),
                    stderr: ByteRing::new(limits.stderr_bytes),
                    dropped: 0,
                    message,
                    sent: 0,
                };
                self.ran = true;
                if self.stop.is_some() || self.aborted {
                    child.close(self.aborted, out);
                } else {
                    child.read(1, delimiter, out);
                    child.read(2, delimiter, out);
                    if let Some(input) = child.pipes.first().copied().flatten() {
                        demand(
                            input,
                            stream::Read::Nothing,
                            settings.largest_room.min(u32::try_from(child.message.len()).expect("bounded message")),
                            out,
                        );
                    }
                }
                Phase::Active(child)
            }
            io::Event::Failed { owner, .. } if owner == self.io_owner => {
                failed = true;
                Phase::Spawning { failed }
            }
            io::Event::Closed { owner } if owner == self.io_owner && failed => {
                return Some(Next::End(if self.ran {
                    git::Done::FailedWithOutput {
                        fault: self.stop.unwrap_or(git::Fault::Broken),
                        diagnostic: git::PushDiagnostic::empty(),
                    }
                } else {
                    git::Done::Failed { fault: self.stop.unwrap_or(git::Fault::Broken) }
                }));
            }
            io::Event::Listening { .. }
            | io::Event::Accepted { .. }
            | io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Stream { .. }
            | io::Event::Output { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. }
            | io::Event::Failed { .. }
            | io::Event::Closed { .. } => Phase::Spawning { failed },
        };
        None
    }
    fn active(
        &mut self,
        event: io::Event,
        mut child: Child,
        settings: &Settings,
        checkout: &checkout::Limits,
        out: &mut Outputs,
    ) -> Option<Next> {
        let delimiter = self.delimiter();
        match event {
            io::Event::Exited { owner, exit } if owner == self.io_owner => child.exited = Some(exit),
            io::Event::Stream { owner, up } => {
                if let Some(i) = child.pipe(owner) {
                    match up {
                        stream::Up::Bytes(bytes) if i != 0 => {
                            if i == 1 {
                                if child.dropped != 0 || child.stdout.write(&bytes).is_err() {
                                    child.dropped = child
                                        .dropped
                                        .saturating_add(u64::try_from(bytes.len()).expect("bounded bytes"));
                                }
                            } else {
                                child.stderr.write(&bytes);
                            }
                            if !child.closing {
                                child.read(i, delimiter, out);
                            }
                        }
                        stream::Up::Room if i == 0 && !child.closing => child.send(settings.largest_room, out),
                        stream::Up::End => {
                            *child.ended.get_mut(i).expect("standard pipe") = true;
                            if !child.closing {
                                emit(out, io::Request::Close { entity: owner });
                            }
                        }
                        stream::Up::Failed(_) => {
                            *child.ended.get_mut(i).expect("standard pipe") = true;
                            if self.stop.is_none() {
                                self.stop = Some(git::Fault::Broken);
                                child.close(self.aborted, out);
                            }
                        }
                        stream::Up::Bytes(_) | stream::Up::Room => {}
                    }
                }
            }
            io::Event::Closed { owner } => {
                if owner == self.io_owner || owner == child.token {
                    // io's child Closed guarantees its pipes are closed too.
                    let next = self.result(&child, checkout);
                    return Some(next);
                }
                if let Some(i) = child.pipe(owner) {
                    *child.closed.get_mut(i).expect("standard pipe") = true;
                }
            }
            io::Event::Failed { owner, .. } if owner == self.io_owner => {
                if self.stop.is_none() {
                    self.stop = Some(git::Fault::Broken);
                    child.close(self.aborted, out);
                }
            }
            io::Event::Listening { .. }
            | io::Event::Accepted { .. }
            | io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Output { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. }
            | io::Event::Failed { .. } => {}
        }
        if !child.closing && child.exited.is_some() && child.outputs_ended() {
            child.closing = true;
            emit(out, io::Request::Close { entity: child.token });
        }
        self.phase = Phase::Active(child);
        None
    }
    pub fn found_version(&self) -> Option<Version> {
        match &self.phase {
            Phase::Active(child)
                if child.exited == Some(kernel::Exit::Code(0)) && self.stop.is_none() && child.dropped == 0 =>
            {
                output::version(child.stdout.as_bytes())
            }
            Phase::Active(_) | Phase::Spawning { .. } | Phase::Settled => None,
        }
    }
}
impl Child {
    fn outputs_ended(&self) -> bool {
        self.ended.get(1) == Some(&true) && self.ended.get(2) == Some(&true)
    }
    fn pipe(&self, token: Token) -> Option<usize> {
        for (i, pipe) in self.pipes.iter().enumerate() {
            if *pipe == Some(token) {
                return Some(i);
            }
        }
        None
    }
    fn read(&self, i: usize, delimiter: u8, out: &mut Outputs) {
        if let Some(token) = self.pipes.get(i).copied().flatten() {
            // max=1 delivers an unterminated final fragment before End, which
            // a larger scan can hide under skein's lib.md section 7's contract.
            let read = if i == 1 {
                stream::Read::Scan { until: stream::Delimiter::new(&[delimiter]).expect("one-byte delimiter"), max: 1 }
            } else {
                stream::Read::Line { max: 1 }
            };
            demand(token, read, 0, out);
        }
    }
    fn send(&mut self, room: u32, out: &mut Outputs) {
        let token = self.pipes.first().copied().flatten().expect("commit stdin");
        let remaining = self.message.len().checked_sub(self.sent).expect("sent within message");
        let amount = remaining.min(usize::try_from(room).expect("u32 fits usize"));
        let end = self.sent.checked_add(amount).expect("bounded message");
        let bytes = Box::from(self.message.get(self.sent..end).expect("message piece"));
        emit(out, io::Request::Stream { stream: token, down: stream::Down::Send(bytes) });
        self.sent = end;
        if end == self.message.len() {
            *self.ended.first_mut().expect("stdin") = true;
            emit(out, io::Request::Stream { stream: token, down: stream::Down::Finish });
            emit(out, io::Request::Close { entity: token });
        } else {
            demand(
                token,
                stream::Read::Nothing,
                u32::try_from(remaining.checked_sub(amount).expect("remaining message"))
                    .expect("bounded message")
                    .min(room),
                out,
            );
        }
    }
    fn close(&mut self, aborted: bool, out: &mut Outputs) {
        if !aborted {
            emit(
                out,
                io::Request::Signal { child: self.token, signal: kernel::Signal::Kill, to: kernel::Target::Group },
            );
        }
        emit(
            out,
            if aborted { io::Request::Abort { entity: self.token } } else { io::Request::Close { entity: self.token } },
        );
        for token in self.pipes.iter().flatten() {
            emit(
                out,
                if aborted { io::Request::Abort { entity: *token } } else { io::Request::Close { entity: *token } },
            );
        }
        self.closing = true;
    }
}
