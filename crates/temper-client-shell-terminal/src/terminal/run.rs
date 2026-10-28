//! One host owns the native client, terminal pipes and termination signals.
use super::Loop;
use crate::{client::NativeClient, limits};
use skein_io::{
    self as io,
    kernel::{Complete, Exit, Fd, Op, Submit},
};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use skein_shell::Host;
const TAG: u64 = 1 << 61;
const ROOM: u32 = 64;
/// Native transport and terminal io share the caller's kernel and clock.
pub struct Terminal {
    client: NativeClient,
    state: Loop,
    io: io::Io,
    env: Env<io::Limits>,
    signals: Option<Token>,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
    raw: Queue<Submit>,
    told: Queue<io::Event>,
    operations: u32,
}
impl Terminal {
    /// # Errors
    /// Returns the descriptor io could not adopt; fixed capacities fit all three.
    pub fn new(client: NativeClient, mut terminal: Loop, stdin: Fd, stdout: Fd, signals: Fd) -> Result<Self, Fd> {
        let limits = io::Limits {
            sockets: 1,
            names: 0,
            refusals: 1,
            intake: limits::terminal().read,
            receive: limits::terminal().read,
            output: limits::terminal().read,
            sends: 1,
            accepts: 1,
            backlog: 1,
            close_timeout: Duration::from_secs(5),
            retry: Duration::from_millis(1),
        };
        let mut io = io::Io::new(&limits);
        terminal.terminal.adopt(stdin, stdout)?;
        let signals = io.adopt_signals(signals)?;
        let operations = client
            .operations()
            .checked_add(io::operations(&limits).expect("signal operations"))
            .and_then(|n| n.checked_add(terminal.terminal_operations))
            .expect("combined operations");
        let room = operations.checked_mul(2).expect("completion and submission room");
        Ok(Self {
            client,
            state: terminal,
            io,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            signals: Some(signals),
            complete: Queue::with_capacity(room),
            submit: Queue::with_capacity(room),
            raw: Queue::with_capacity(ROOM),
            told: Queue::with_capacity(ROOM),
            operations,
        })
    }
    /// Explicitly requested metadata; never progresses the native client.
    #[must_use]
    pub fn pending(&self) -> crate::diagnostics::Pending {
        self.client.pending()
    }
    /// The binary prints this stderr line after all resources have drained.
    #[must_use]
    pub fn exit_line(&self) -> Option<Box<[u8]>> {
        self.state.exit().and_then(|(_, line)| line)
    }
    fn request(&mut self, request: io::Request) {
        assert!(self.io.takes() && self.raw.room() >= io::MAX_OUT_DOWN.submissions, "terminal request room");
        io::down(&mut self.io, &self.env, request, &mut self.raw);
    }
    fn complete(&mut self, mut complete: Complete) {
        if skein_ui_text::Terminal::owns(complete.op) {
            self.state.terminal.completions().push(complete);
            return;
        }
        if complete.op.raw() & TAG == 0 {
            self.client.completions().push(complete);
            return;
        }
        complete.op = Token::new(complete.op.raw() & !TAG);
        if let Op::Cancel { target } = &mut complete.kind {
            *target = Token::new(target.raw() & !TAG);
        }
        io::up(&mut self.io, &self.env, complete, &mut self.told, &mut self.raw);
    }
    fn event(&mut self, event: &io::Event) {
        match event {
            io::Event::Shutdown { .. } | io::Event::Failed { .. } => self.state.close(),
            io::Event::Closed { owner } => {
                if self.signals == Some(*owner) {
                    self.signals = None;
                }
            }
            io::Event::Resized => {}
            io::Event::Stream { .. }
            | io::Event::Output { .. }
            | io::Event::Listening { .. }
            | io::Event::Accepted { .. }
            | io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Window { .. } => panic!("terminal owns pipes and signals only"),
        }
    }
    fn pump(&mut self) {
        self.state.iterate(self.env.now, &mut self.client);
        if self.state.exit().is_some()
            && let Some(entity) = self.signals.take()
        {
            self.request(io::Request::Abort { entity });
        }
    }
    fn publish(&mut self) {
        while self.submit.room() > 0 {
            let Some(mut submit) = self.raw.pop() else {
                break;
            };
            assert!(submit.op.raw() & TAG == 0, "terminal token namespace");
            submit.op = Token::new(submit.op.raw() | TAG);
            if let Op::Cancel { target } = &mut submit.kind {
                *target = Token::new(target.raw() | TAG);
            }
            self.submit.push(submit);
        }
        while self.submit.room() > 0 {
            let Some(submit) = self.state.terminal.submissions().pop() else { break };
            self.submit.push(submit);
        }
        while self.submit.room() > 0 {
            let Some(submit) = self.client.submissions().pop() else {
                break;
            };
            self.submit.push(submit);
        }
    }
}
impl Host for Terminal {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        while self.io.is_ready()
            && self.raw.room() >= io::MAX_OUT_RESUME.submissions
            && self.told.room() >= io::MAX_OUT_RESUME.events
        {
            io::resume(&mut self.io, &self.env, &mut self.told, &mut self.raw);
        }
        while !self.io.is_ready()
            && self.raw.room() >= io::MAX_OUT_UP.submissions
            && self.told.room() >= io::MAX_OUT_UP.events
        {
            let Some(complete) = self.complete.pop() else {
                break;
            };
            self.complete(complete);
        }
        while self.io.is_due(now)
            && self.raw.room() >= io::MAX_OUT_FIRE.submissions
            && self.told.room() >= io::MAX_OUT_FIRE.events
        {
            io::fire(&mut self.io, &self.env, &mut self.told, &mut self.raw);
        }
        self.client.iterate(now, wall);
        while let Some(event) = self.told.pop() {
            self.event(&event);
        }
        self.pump();
        self.publish();
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submit
    }
    fn work_pending(&self, now: Time) -> bool {
        !self.complete.is_empty()
            || !self.raw.is_empty()
            || self.io.is_ready()
            || self.io.is_due(now)
            || self.client.work_pending(now)
            || !self.submit.is_empty()
            || self.state.terminal.work_pending(now)
            || self.state.next_deadline().is_some_and(|deadline| now >= deadline)
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.client.next_deadline(), self.io.next_deadline(), self.state.next_deadline()].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        [self.client.next_policy_deadline(), self.state.next_deadline()].into_iter().flatten().min()
    }
    fn is_empty(&self) -> bool {
        self.client.is_empty()
            && self.io.is_empty()
            && self.complete.is_empty()
            && self.submit.is_empty()
            && self.raw.is_empty()
            && self.told.is_empty()
            && self.state.exit().is_some()
    }
    fn exit(&self) -> Option<Exit> {
        self.is_empty().then(|| Exit::Code(self.state.exit().expect("drained loop").0))
    }
    fn operations(&self) -> u32 {
        self.operations
    }
}
