//! The machine on skein-io, with one bounded chunk per child pipe.
use crate::{
    layout::{self, Placed},
    limits::{Limits, Unfit},
    lines::{self, Lines},
    machine::{Input, Machine, Order, Outcome, Said, Why},
    pipe::{self, Command, Line, Reader},
    ready::Ready,
    records::Records,
    sink::Sink,
};
use alloc::boxed::Box;
use skein_io::{
    self as io, Event, Io, Request,
    kernel::{self, Complete, Exit, Fd, Submit},
};
use skein_lib::{
    Decimal, Duration, Env, List, Queue, Time, Token, Wall,
    stream::{Down, Read, Up},
};
use temper_forge_setup::{Kind, Step, Steps};
/// The descriptors and command inherited from the binary.
#[derive(Debug)]
pub struct Settings {
    pub program: Box<[u8]>,
    pub environment: Box<[Box<[u8]>]>,
    pub serve: Box<[u8]>,
    pub listeners: Box<[(Fd, core::net::SocketAddr)]>,
    pub root: Fd,
    pub signals: Fd,
    pub stdout: Fd,
    pub stderr: Fd,
}
/// A longer line is emitted in pieces, each prefixed.
pub const LINE_BYTES: u32 = 4096;
const ROOT_CLOSE: Token = Token::new(u64::MAX);
#[derive(Debug)]
struct Pipe {
    token: Option<Token>,
    reading: bool,
    lines: Lines,
}
impl Pipe {
    fn new(plain: bool) -> Self {
        Self { token: None, reading: false, lines: Lines::new(plain) }
    }
    fn settled(&self) -> bool {
        self.token.is_none() && self.lines.is_empty()
    }
}
#[derive(Debug)]
struct Child {
    owner: Option<Token>,
    process: Option<Token>,
    pipes: [Pipe; 2],
    spawn: bool,
    hold: bool,
    line: Option<Box<[u8]>>,
    stdin: Option<Sink>,
    ready: Option<Ready>,
}
impl Child {
    #[expect(clippy::manual_map, reason = "programming-model.md: match rather than Option::map")]
    fn new(serve: &[u8], step: &Step) -> Self {
        Self {
            owner: None,
            process: None,
            pipes: [Pipe::new(lines::route(serve, step.name(), 1)), Pipe::new(false)],
            spawn: false,
            hold: false,
            line: None,
            stdin: None,
            ready: match step.ready() {
                Some(address) => Some(Ready::new(address)),
                None => None,
            },
        }
    }
    fn ready_settled(&self) -> bool {
        match &self.ready {
            Some(ready) => ready.settled(),
            None => true,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    New,
    Running,
    Closing,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Root {
    Open,
    Closing,
    Closed,
}
#[derive(Debug)]
struct Control {
    read: Token,
    sink: Sink,
    reader: Reader,
    reading: bool,
    ended: bool,
    line: Option<Line>,
}
/// All io and policy state owned by one box process.
#[derive(Debug)]
pub struct Service {
    io: Io,
    env: Env<io::Limits>,
    machine: Machine,
    steps: Box<[Placed]>,
    records: Records,
    control: Option<Control>,
    closing_control: [Option<Token>; 2],
    line_bytes: u32,
    reply_entries: u32,
    settings: Settings,
    children: Box<[Child]>,
    sinks: [Sink; 2],
    listeners: Box<[(core::net::SocketAddr, Token)]>,
    listeners_closing: usize,
    signals: Token,
    phase: Phase,
    failed: bool,
    serial: u64,
    root: Root,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    told: Queue<Event>,
    requests: Queue<Request>,
    orders: Queue<Order>,
    memory: u64,
}
impl Service {
    /// Checks the policy limits and adopts the binary's descriptors.
    pub fn new(limits: &Limits, steps: Steps, settings: Settings) -> Result<Self, Unfit> {
        let mut longest = 0;
        for group in &steps.asked {
            longest = longest.max(u32::try_from(group.steps.len()).expect("bounded group"));
        }
        let reply_entries = longest.saturating_add(2);
        let steps = layout::flatten(steps);
        let machine = Machine::new(limits, &steps)?;
        let records = Records::new(&steps);
        let count = u32::try_from(steps.len()).expect("steps fit u32");
        let mut bounds = io_limits(count);
        bounds.sockets = bounds
            .sockets
            .checked_add(u32::try_from(settings.listeners.len()).expect("bounded listeners"))
            .expect("listener slots");
        bounds.intake = bounds.intake.max(limits.line_bytes);
        bounds.output = bounds.output.max(limits.line_bytes);
        let mut children = List::with_capacity(count);
        let mut inputs = u64::try_from(settings.serve.len()).expect("serve name");
        for placed in &steps {
            let step = &placed.step;
            bounds.output = bounds.output.max(
                LINE_BYTES
                    .checked_add(u32::try_from(step.name().len()).expect("bounded name"))
                    .expect("bounded line")
                    .checked_add(3)
                    .expect("prefix"),
            );
            children.push(Child::new(&settings.serve, step)).expect("one child per step");
            inputs = inputs.saturating_add(u64::try_from(size_of_val(step.listeners())).expect("listener addresses"));
            inputs = inputs.saturating_add(u64::try_from(step.name().len()).expect("name length"));
            inputs = inputs.saturating_add(u64::try_from(size_of_val(step.arguments())).expect("argument slots"));
            for argument in step.arguments() {
                inputs = inputs.saturating_add(u64::try_from(argument.len()).expect("argument length"));
            }
        }
        inputs = inputs.saturating_add(inherited_bytes(&settings, count));
        let mut io = Io::new(&bounds);
        let signals = io.adopt_signals(settings.signals).expect("three adopted slots");
        let stdout = io.adopt_write_pipe(settings.stdout, None).expect("stdout slot");
        let stderr = io.adopt_write_pipe(settings.stderr, None).expect("stderr slot");
        let listeners = adopt_listeners(&mut io, &settings.listeners);
        let capacity = io::operations(&bounds).expect("bounded operations").saturating_mul(4).max(64);
        let entries = count.saturating_mul(2).saturating_add(1);
        // Each queued record can own a launch or a chunk. Count even the impossible
        // case where every slot holds the largest launch and the largest chunk.
        let payload = inputs.saturating_add(u64::from(bounds.output)).saturating_add(
            u64::try_from(size_of::<kernel::Pipe>())
                .expect("pipe spec")
                .saturating_mul(u64::try_from(settings.listeners.len()).expect("listener count").saturating_add(4)),
        );
        let queues = Queue::<Complete>::worst_case(capacity)
            .expect("completion queue")
            .saturating_add(Queue::<Submit>::worst_case(capacity).expect("submission queue"))
            .saturating_add(Queue::<Event>::worst_case(capacity).expect("event queue"))
            .saturating_add(Queue::<Request>::worst_case(capacity).expect("request queue"))
            .saturating_add(Queue::<Order>::worst_case(capacity).expect("order queue"));
        let memory = io::worst_case(&bounds)
            .expect("io memory")
            .saturating_add(crate::limits::worst_case(limits, count).expect("policy memory"))
            .saturating_add(inputs.saturating_mul(3))
            .saturating_add(listener_memory(&settings.listeners))
            .saturating_add(u64::from(count).saturating_mul(64))
            .saturating_add(queues)
            .saturating_add(payload.saturating_mul(u64::from(capacity).saturating_mul(4)))
            .saturating_add(List::<Child>::worst_case(count).expect("child states"))
            .saturating_add(List::<Placed>::worst_case(count).expect("step states"))
            .saturating_add(u64::from(count).saturating_mul(u64::from(LINE_BYTES).saturating_mul(4)))
            .saturating_add(Sink::worst_case(entries, bounds.output).saturating_mul(2))
            .saturating_add(Sink::worst_case(reply_entries, limits.line_bytes))
            .saturating_add(u64::from(limits.line_bytes).saturating_mul(2))
            .saturating_add(Records::worst_case(count))
            .saturating_add(link_memory(&steps))
            .saturating_add(u64::from(count).saturating_mul(Ready::worst_case()));
        Ok(Self {
            io,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: bounds },
            machine,
            steps,
            records,
            control: None,
            closing_control: [None, None],
            line_bytes: limits.line_bytes,
            reply_entries,
            settings,
            children: children.into_boxed(),
            sinks: [Sink::new(stdout, entries), Sink::new(stderr, entries)],
            listeners: listeners.into_boxed(),
            listeners_closing: 0,
            signals,
            phase: Phase::New,
            failed: false,
            serial: 1_u64 << 63,
            root: Root::Open,
            completions: Queue::with_capacity(capacity),
            submissions: Queue::with_capacity(capacity),
            told: Queue::with_capacity(capacity),
            requests: Queue::with_capacity(capacity),
            orders: Queue::with_capacity(capacity),
            memory,
        })
    }
    fn room(&self, bound: io::MaxOut) -> bool {
        self.told.room() >= bound.events && self.submissions.room() >= bound.submissions
    }
    fn input(&mut self, input: Input) {
        self.machine.input(self.env.now, input, &mut self.orders);
    }
    fn event(&mut self, event: Event) {
        for state in &mut self.children {
            if let Some(ready) = &mut state.ready
                && ready.owns(&event)
            {
                ready.event(event, self.env.now, &mut self.requests);
                return;
            }
        }
        match event {
            Event::Shutdown { .. } => self.input(Input::Terminate),
            Event::Spawned { owner, child, pipes } => {
                self.spawned(owner, child, &pipes);
            }
            Event::Exited { owner, exit } => {
                for (step, state) in self.children.iter().enumerate() {
                    if state.owner == Some(owner) {
                        if self.steps.get(step).expect("step").step.name() == self.settings.serve.as_ref()
                            && let Some(control) = self.control.take()
                        {
                            self.closing_control = [Some(control.read), Some(control.sink.token)];
                            self.requests.push(Request::Close { entity: control.read });
                            self.requests.push(Request::Close { entity: control.sink.token });
                        }
                        self.machine.input(self.env.now, Input::Exited { step, exit }, &mut self.orders);
                        break;
                    }
                }
            }
            Event::Closed { owner } => {
                for token in &mut self.closing_control {
                    if *token == Some(owner) {
                        *token = None;
                    }
                }
                if let Some(control) = &mut self.control {
                    if control.sink.token == owner {
                        control.sink.closed();
                    }
                    if control.read == owner {
                        control.ended = true;
                        control.reading = false;
                    }
                }
                for sink in &mut self.sinks {
                    if sink.token == owner {
                        sink.closed();
                    }
                }
                for (step, state) in self.children.iter_mut().enumerate() {
                    if let Some(stdin) = &state.stdin
                        && stdin.token == owner
                    {
                        state.stdin = None;
                    }
                    if state.owner == Some(owner) {
                        let input = if state.process.take().is_some() {
                            Input::Closed { step }
                        } else {
                            Input::NotStarted { step }
                        };
                        state.owner = None;
                        self.machine.input(self.env.now, input, &mut self.orders);
                    }
                    for pipe in &mut state.pipes {
                        if pipe.token == Some(owner) {
                            pipe.token = None;
                            pipe.reading = false;
                            pipe.lines.end();
                        }
                    }
                }
            }
            Event::Stream { owner, up } => self.stream(owner, up),
            Event::Failed { owner, .. } => {
                if owner == self.signals {
                    self.failed = true;
                    self.input(Input::Terminate);
                }
            }
            Event::Listening { .. }
            | Event::Accepted { .. }
            | Event::Connecting { .. }
            | Event::Connected { .. }
            | Event::Output { .. }
            | Event::Usage { .. }
            | Event::Resized
            | Event::Window { .. } => {}
        }
    }
    fn spawned(&mut self, owner: Token, child: Token, pipes: &[Token]) {
        for (step, state) in self.children.iter_mut().enumerate() {
            if state.owner == Some(owner) {
                state.process = Some(child);
                if self.steps.get(step).expect("step").step.kind() == Kind::Link {
                    let mut sink = Sink::new(*pipes.get(2).expect("link stdin"), 1);
                    sink.push(state.line.take().expect("link's first line")).expect("one entry");
                    state.stdin = Some(sink);
                }
                for (pipe, token) in state.pipes.iter_mut().zip(pipes) {
                    pipe.token = Some(*token);
                }
                if self.steps.get(step).expect("step").step.name() == self.settings.serve.as_ref() {
                    self.control = Some(Control {
                        read: *pipes.get(2).expect("serve requests pipe"),
                        sink: Sink::new(*pipes.get(3).expect("serve answers pipe"), self.reply_entries),
                        reader: Reader::new(self.line_bytes),
                        reading: false,
                        ended: false,
                        line: None,
                    });
                }
                self.machine.input(self.env.now, Input::Started { step }, &mut self.orders);
                break;
            }
        }
    }
    fn stream(&mut self, owner: Token, up: Up) {
        if let Some(control) = &mut self.control {
            if control.sink.token == owner {
                control.sink.up(up, &mut self.requests);
                return;
            }
            if control.read == owner {
                control.reading = false;
                match up {
                    Up::Bytes(bytes) => {
                        assert_eq!(bytes.len(), 1, "one byte demand");
                        assert!(control.line.is_none(), "one pending control line");
                        control.line = control.reader.byte(*bytes.first().expect("one byte"));
                    }
                    Up::End | Up::Failed(_) => {
                        control.ended = true;
                        self.requests.push(Request::Close { entity: owner });
                    }
                    Up::Room => {}
                }
                return;
            }
        }
        for sink in &mut self.sinks {
            if sink.token == owner {
                if sink.up(up, &mut self.requests) {
                    self.failed = true;
                    self.input(Input::Terminate);
                }
                return;
            }
        }
        for (step, state) in self.children.iter_mut().enumerate() {
            if let Some(stdin) = &mut state.stdin
                && stdin.token == owner
            {
                // Only the link's exit decides success, even if it did not read.
                stdin.up(up, &mut self.requests);
                return;
            }
            for (fd, pipe) in state.pipes.iter_mut().enumerate() {
                if pipe.token == Some(owner) {
                    pipe.reading = false;
                    match up {
                        Up::Bytes(bytes) => pipe.lines.push(bytes),
                        Up::End | Up::Failed(_) => {
                            pipe.lines.end();
                            self.requests.push(Request::Close { entity: owner });
                        }
                        Up::Room => {}
                    }
                    if fd == 0
                        && let Some(line) = pipe.lines.first()
                        && self.machine.outcome().is_none()
                    {
                        state.line = Some(line);
                        self.machine.input(self.env.now, Input::Line { step }, &mut self.orders);
                    }
                    return;
                }
            }
        }
    }
    fn order(&mut self, order: Order) {
        match order {
            Order::Record { forge } => self.records.write(forge),
            Order::Say { said } => {
                if let Said::Stopped { forge } = said {
                    let sink = self.sinks.get_mut(1).expect("stderr");
                    if sink.takes() {
                        let digits = Decimal::of(u64::from(forge));
                        sink.push(lines::joined(&[b"box: forge ", digits.as_bytes(), b"'s group stopped\n"]))
                            .expect("sink room checked");
                    }
                }
                if let Some(control) = &mut self.control {
                    let line = match said {
                        Said::Step { step } => {
                            let asked = self.steps.get(step).expect("step").asked.expect("asked step");
                            pipe::step(asked.forge, asked.stage)
                        }
                        Said::Started { forge } => pipe::started(forge),
                        Said::Stopped { forge } => pipe::stopped(forge),
                        Said::Failed { forge, why } => {
                            let why = match why {
                                Why::Busy => skein_lib::bytes::copy_of(b"busy"),
                                Why::NotAsked => skein_lib::bytes::copy_of(b"not asked"),
                                Why::Stopping => skein_lib::bytes::copy_of(b"stopping"),
                                Why::Exited { step, exit } => {
                                    pipe::exited(self.steps.get(step).expect("step").step.name(), exit)
                                }
                                Why::NotStarted { step } => {
                                    pipe::not_started(self.steps.get(step).expect("step").step.name())
                                }
                                Why::NotReady { step } => lines::joined(&[
                                    self.steps.get(step).expect("step").step.name(),
                                    b" version unavailable",
                                ]),
                                Why::NotWritten => skein_lib::bytes::copy_of(pipe::not_written()),
                            };
                            pipe::failed(forge, &why, self.line_bytes)
                        }
                    };
                    if control.sink.room() {
                        control.sink.push(line).expect("reply room checked");
                    }
                }
            }
            Order::Wait { step } => {
                self.children
                    .get_mut(step)
                    .expect("step")
                    .ready
                    .as_mut()
                    .expect("version endpoint")
                    .start(self.env.now);
            }
            Order::Hold { step } => self.children.get_mut(step).expect("machine step").hold = true,
            Order::Spawn { step } => {
                if self.steps.get(step).expect("step").step.kind() == Kind::Link {
                    let before = step.checked_sub(1).expect("link predecessor");
                    let line = self.children.get_mut(before).expect("long step").line.take();
                    self.children.get_mut(step).expect("link step").line = line;
                }
                self.children.get_mut(step).expect("machine step").spawn = true;
            }
            Order::Signal { step } => {
                if let Some(ready) = &mut self.children.get_mut(step).expect("step").ready {
                    ready.cancel(&mut self.requests);
                }
                if let Some(child) = self.children.get(step).expect("machine step").process {
                    self.requests.push(Request::Signal {
                        child,
                        signal: kernel::Signal::Terminate,
                        to: kernel::Target::Child,
                    });
                }
            }
            Order::Close { step } => {
                if let Some(ready) = &mut self.children.get_mut(step).expect("step").ready {
                    ready.cancel(&mut self.requests);
                }
                if self.steps.get(step).expect("step").step.name() == self.settings.serve.as_ref()
                    && let Some(control) = self.control.take()
                {
                    self.closing_control = [Some(control.read), Some(control.sink.token)];
                    self.requests.push(Request::Close { entity: control.read });
                    self.requests.push(Request::Close { entity: control.sink.token });
                }
                if let Some(child) = self.children.get(step).expect("machine step").process {
                    self.requests.push(Request::Close { entity: child });
                }
            }
        }
    }
    fn children(&mut self) {
        for (step, state) in self.children.iter_mut().enumerate() {
            if self.requests.room() < 6 || self.orders.room() < self.machine.max_out() {
                break;
            }
            if let Some(ready) = &mut state.ready {
                if self.machine.outcome().is_some() {
                    ready.cancel(&mut self.requests);
                }
                if let Some(ok) = ready.tick(self.env.now, &mut self.serial, &mut self.requests) {
                    self.machine.input(
                        self.env.now,
                        if ok { Input::Ready { step } } else { Input::NotReady { step } },
                        &mut self.orders,
                    );
                }
            }
            if let Some(stdin) = &mut state.stdin {
                stdin.tick(true, &mut self.requests);
            }
            let description = &self.steps.get(step).expect("step state").step;
            for (fd, pipe) in state.pipes.iter_mut().enumerate() {
                let plain = fd == 0 && lines::route(&self.settings.serve, description.name(), 1);
                let sink = self.sinks.get_mut(usize::from(!plain)).expect("two sinks");
                if sink.takes() {
                    if let Some(line) = pipe.lines.take(description.name()) {
                        sink.push(line).expect("checked sink room");
                    }
                } else if !sink.room() {
                    // Broken sinks discard bounded pending pieces during teardown.
                    drop(pipe.lines.take(description.name()));
                }
                if pipe.lines.wants_read()
                    && !pipe.reading
                    && sink.takes()
                    && let Some(stream) = pipe.token
                {
                    self.requests.push(Request::Stream {
                        stream,
                        down: Down::Demand { read: Read::Arrived { max: LINE_BYTES }, room: 0 },
                    });
                    pipe.reading = true;
                }
            }
            let mut settled = state.stdin.is_none() && state.ready_settled();
            for pipe in &state.pipes {
                settled &= pipe.settled();
            }
            if description.name() == self.settings.serve.as_ref() {
                settled &= self.closing_control == [None, None];
            }
            if state.spawn && settled {
                state.spawn = false;
                self.serial = self.serial.checked_add(1).expect("spawn serial");
                let owner = Token::new(self.serial);
                state.owner = Some(owner);
                if description.kind() != Kind::Link {
                    state.line = None;
                }
                for (fd, pipe) in state.pipes.iter_mut().enumerate() {
                    *pipe = Pipe::new(fd == 0 && lines::route(&self.settings.serve, description.name(), 1));
                }
                if state.hold {
                    state.pipes.get_mut(0).expect("stdout").lines.hold();
                    state.hold = false;
                }
                self.requests.push(Request::Spawn {
                    owner,
                    spawn: kernel::Spawn {
                        program: self.settings.program.clone(),
                        args: Box::from(description.arguments()),
                        env: spawn_environment(&self.settings.environment, description.listeners().len()),
                        root: self.settings.root,
                        dir: Box::new([]),
                        pipes: spawn_pipes(description, &self.settings.serve, &self.listeners),
                    },
                });
            }
        }
    }
    fn record_work(&mut self) {
        if (self.sinks.get(1).expect("stderr").takes() || !self.sinks.get(1).expect("stderr").room())
            && let Some(notice) = self.records.take_notice()
        {
            drop(self.sinks.get_mut(1).expect("stderr").push(notice));
        }
        if !self.records.has_notice()
            && self.orders.room() >= self.machine.max_out()
            && let Some(answer) = self.records.take_answer()
        {
            self.input(answer);
        }
        self.records.tick(self.settings.root, &mut self.submissions);
    }
    fn control_ready(&self) -> bool {
        let Some(control) = &self.control else {
            return false;
        };
        control.sink.drained()
            && self.orders.is_empty()
            && self.orders.room() >= self.machine.max_out()
            && self.sinks.get(1).expect("stderr").takes()
            && self.phase == Phase::Running
    }
    fn take_control_line(&mut self) {
        if !self.control_ready() {
            return;
        }
        let Some(line) = self.control.as_mut().expect("serve").line.take() else {
            return;
        };
        match line {
            Line::Whole(line) => match pipe::command(&line) {
                Command::Start(forge) => self.input(Input::Ask { forge }),
                Command::Stop(forge) => self.input(Input::Stop { forge }),
                Command::Unknown => {
                    self.sinks
                        .get_mut(1)
                        .expect("stderr")
                        .push(skein_lib::bytes::copy_of(b"box: serve: unknown pipe line\n"))
                        .expect("diagnostic room");
                }
            },
            Line::TooLong => {
                self.sinks
                    .get_mut(1)
                    .expect("stderr")
                    .push(skein_lib::bytes::copy_of(b"box: serve: pipe line too long\n"))
                    .expect("diagnostic room");
            }
        }
    }
    fn can_read_control(&self) -> bool {
        let Some(control) = &self.control else {
            return false;
        };
        !control.ended
            && !control.reading
            && control.line.is_none()
            && self.control_ready()
            && self.orders.room() >= self.machine.max_out()
            && self.sinks.get(1).expect("stderr").takes()
            && self.phase == Phase::Running
    }
    fn control_work(&mut self) {
        if self.requests.room() < 3 {
            return;
        }
        self.take_control_line();
        if self.can_read_control() {
            let control = self.control.as_mut().expect("live serve");
            self.requests.push(Request::Stream {
                stream: control.read,
                down: Down::Demand { read: Read::Arrived { max: 1 }, room: 0 },
            });
            control.reading = true;
        }
        if let Some(control) = &mut self.control {
            control.sink.tick(false, &mut self.requests);
            control.sink.flush(self.env.limits.output, &mut self.requests);
        }
    }
    fn finish(&mut self) {
        if !self.machine.is_idle() || self.machine.outcome().is_none() || !self.records.ready() {
            return;
        }
        for state in &self.children {
            if !state.ready_settled() {
                return;
            }
            for pipe in &state.pipes {
                if !pipe.settled() {
                    return;
                }
            }
        }
        if self.phase != Phase::Closing {
            if self.requests.room() == 0 {
                return;
            }
            match self.machine.outcome() {
                Some(Outcome::GaveUp { step, restarts }) => {
                    let sink = self.sinks.get_mut(1).expect("stderr");
                    if sink.room() && !sink.takes() {
                        return;
                    }
                    if sink.takes() {
                        let digits = Decimal::of(u64::from(restarts));
                        sink.push(lines::joined(&[
                            b"box: ",
                            self.steps.get(step).expect("failed step").step.name(),
                            b" gave up after ",
                            digits.as_bytes(),
                            b" restarts\n",
                        ]))
                        .expect("sink room");
                    }
                }
                Some(Outcome::Stopped) | None => {}
            }

            self.requests.push(Request::Close { entity: self.signals });
            self.phase = Phase::Closing;
        }
        while self.listeners_closing < self.listeners.len() && self.requests.room() > 0 {
            let (_, token) = self.listeners.get(self.listeners_closing).expect("held listener");
            self.requests.push(Request::Close { entity: *token });
            self.listeners_closing = self.listeners_closing.saturating_add(1);
        }
        if self.root == Root::Open && self.submissions.room() > 0 {
            self.submissions.push(Submit { op: ROOT_CLOSE, kind: kernel::Op::Close { fd: self.settings.root } });
            self.root = Root::Closing;
        }
    }
}
fn listener_memory(held: &[(Fd, core::net::SocketAddr)]) -> u64 {
    let count = u32::try_from(held.len()).expect("bounded listeners");
    u64::try_from(size_of_val(held))
        .expect("held listener slots")
        .saturating_add(List::<(core::net::SocketAddr, Token)>::worst_case(count).expect("listener tokens"))
}
fn adopt_listeners(io: &mut Io, held: &[(Fd, core::net::SocketAddr)]) -> List<(core::net::SocketAddr, Token)> {
    let mut listeners = List::with_capacity(u32::try_from(held.len()).expect("bounded listeners"));
    for (fd, address) in held {
        let token = io.adopt_listener(*fd, *address).expect("unique listeners and counted slots");
        listeners.push((*address, token)).expect("counted listener tokens");
    }
    listeners
}
fn spawn_environment(environment: &[Box<[u8]>], listeners: usize) -> Box<[Box<[u8]>]> {
    let mut out = List::with_capacity(u32::try_from(environment.len()).expect("bounded environment").saturating_add(1));
    for entry in environment {
        if !entry.starts_with(b"LISTEN_FDS=") && !entry.starts_with(b"LISTEN_PID=") {
            out.push(entry.clone()).expect("environment counted");
        }
    }
    if listeners > 0 {
        let count = Decimal::of(u64::try_from(listeners).expect("bounded listeners"));
        out.push(lines::joined(&[b"LISTEN_FDS=", count.as_bytes()])).expect("count slot");
    }
    out.into_boxed()
}
fn spawn_pipes(step: &Step, serve: &[u8], listeners: &[(core::net::SocketAddr, Token)]) -> Box<[kernel::Pipe]> {
    let n = u32::try_from(step.listeners().len()).expect("bounded listeners");
    let mut pipes = List::with_capacity(n.checked_add(4).expect("bounded spawn table"));
    for child in [1, 2] {
        pipes.push(kernel::Pipe { child, way: kernel::Way::Out, parent: None }).expect("stdio");
    }
    for (i, address) in step.listeners().iter().enumerate() {
        let mut found = None;
        for (addr, token) in listeners {
            if addr == address {
                found = Some(*token);
                break;
            }
        }
        let token = found.expect("binary opened every step listener");
        let child = 3_u32.checked_add(u32::try_from(i).expect("bounded listener index")).expect("bounded child fd");
        pipes.push(kernel::Pipe { child, way: kernel::Way::Given(token), parent: None }).expect("listener slot");
    }
    if step.name() == serve {
        let requests = n.checked_add(3).expect("bounded child fd");
        let answers = n.checked_add(4).expect("bounded child fd");
        pipes.push(kernel::Pipe { child: requests, way: kernel::Way::Out, parent: None }).expect("serve requests");
        pipes.push(kernel::Pipe { child: answers, way: kernel::Way::In, parent: None }).expect("serve answers");
    } else if step.kind() == Kind::Link {
        pipes.push(kernel::Pipe { child: 0, way: kernel::Way::In, parent: None }).expect("link stdin");
    }
    pipes.into_boxed()
}
fn link_memory(steps: &[Placed]) -> u64 {
    let mut memory = 0_u64;
    for step in steps {
        if step.step.kind() == Kind::Link {
            memory = memory.saturating_add(u64::from(LINE_BYTES)).saturating_add(Sink::worst_case(1, LINE_BYTES));
        }
    }
    memory
}
fn inherited_bytes(settings: &Settings, count: u32) -> u64 {
    let mut inputs = 0_u64;
    inputs = inputs.saturating_add(
        u64::try_from(size_of_val(settings.environment.as_ref()))
            .expect("environment slots")
            .saturating_mul(u64::from(count).saturating_add(1)),
    );
    for value in &settings.environment {
        inputs = inputs.saturating_add(
            u64::try_from(value.len()).expect("environment length").saturating_mul(u64::from(count).saturating_add(1)),
        );
    }
    inputs = inputs.saturating_add(
        u64::try_from(settings.program.len())
            .expect("program length")
            .saturating_mul(u64::from(count).saturating_add(1)),
    );
    inputs
}
/// Up to four entities per step, three inherited streams and two serve control pipes.
/// Names and the configured control line bound extend the byte limits at construction.
#[must_use]
pub fn io_limits(steps: u32) -> io::Limits {
    io::Limits {
        sockets: steps.checked_mul(4).expect("bounded step count").checked_add(5).expect("inherited streams"),
        names: 0,
        refusals: steps.max(1),
        intake: LINE_BYTES,
        receive: 1,
        output: LINE_BYTES.saturating_add(3),
        sends: 1,
        accepts: 1,
        backlog: 1,
        close_timeout: Duration::from_secs(5),
        retry: Duration::from_millis(10),
    }
}
impl skein_shell::Host for Service {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        for _ in 0..self.env.limits.sockets.saturating_add(self.env.limits.refusals) {
            if !self.io.is_ready() || !self.room(io::MAX_OUT_RESUME) {
                break;
            }
            io::resume(&mut self.io, &self.env, &mut self.told, &mut self.submissions);
        }
        if !self.io.is_ready() {
            for _ in 0..self.completions.len() {
                if !self.room(io::MAX_OUT_UP) {
                    break;
                }
                let complete = self.completions.pop().expect("counted completion");
                if Records::owns(complete.op) {
                    self.records.complete(complete);
                } else if complete.op == ROOT_CLOSE {
                    self.root = Root::Closed;
                } else {
                    io::up(&mut self.io, &self.env, complete, &mut self.told, &mut self.submissions);
                }
            }
            if self.completions.is_empty() {
                for _ in 0..self.env.limits.sockets.saturating_mul(2) {
                    if !self.io.is_due(now) || !self.room(io::MAX_OUT_FIRE) {
                        break;
                    }
                    io::fire(&mut self.io, &self.env, &mut self.told, &mut self.submissions);
                }
            }
        }
        self.record_work();
        if self.phase == Phase::New && self.records.ready() && self.orders.room() >= self.machine.max_out() {
            self.machine.start(now, self.records.recorded(), &mut self.orders);
            self.phase = Phase::Running;
        }
        for _ in 0..self.told.len() {
            if self.orders.room() < self.machine.max_out() || self.requests.room() < 4 {
                break;
            }
            let event = self.told.pop().expect("counted event");
            self.event(event);
        }
        if self.told.is_empty() && self.orders.room() >= self.machine.max_out() {
            self.machine.fire(now, &mut self.orders);
        }
        for _ in 0..self.orders.len() {
            if self.requests.room() < 3 {
                break;
            }
            if let Some(Order::Say { .. }) = self.orders.iter().next()
                && let Some(control) = &self.control
                && control.sink.room()
                && !control.sink.takes()
            {
                break;
            }
            if let Some(Order::Say { said: Said::Stopped { .. } }) = self.orders.iter().next()
                && self.sinks.get(1).expect("stderr").room()
                && !self.sinks.get(1).expect("stderr").takes()
            {
                break;
            }
            let order = self.orders.pop().expect("counted order");
            self.order(order);
        }
        self.children();
        self.control_work();
        self.record_work();
        self.finish();
        for sink in &mut self.sinks {
            if self.requests.room() > 0 {
                sink.tick(self.phase == Phase::Closing, &mut self.requests);
            }
        }
        for _ in 0..self.requests.len() {
            if !self.io.takes() || self.submissions.room() < io::MAX_OUT_DOWN.submissions {
                break;
            }
            let request = self.requests.pop().expect("counted request");
            io::down(&mut self.io, &self.env, request, &mut self.submissions);
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        if self.phase == Phase::New
            || !self.completions.is_empty()
            || !self.told.is_empty()
            || !self.requests.is_empty()
            || !self.orders.is_empty()
            || self.io.is_ready()
            || self.io.is_due(now)
        {
            return true;
        }
        if let Some(at) = self.next_deadline()
            && at <= now
        {
            return true;
        }
        if self.records.pending() && (!self.records.has_notice() || self.sinks.get(1).expect("stderr").takes()) {
            return true;
        }
        if let Some(control) = &self.control
            && (control.sink.pending()
                || control.sink.needs_flush()
                || self.can_read_control()
                || (control.line.is_some() && self.control_ready()))
        {
            return true;
        }
        for sink in &self.sinks {
            if sink.pending() {
                return true;
            }
        }
        for (step, state) in self.children.iter().enumerate() {
            if let Some(stdin) = &state.stdin
                && (stdin.pending() || stdin.needs_flush())
            {
                return true;
            }
            let description = &self.steps.get(step).expect("step state").step;
            for (fd, pipe) in state.pipes.iter().enumerate() {
                let plain = fd == 0 && lines::route(&self.settings.serve, description.name(), 1);
                let sink = self.sinks.get(usize::from(!plain)).expect("two sinks");
                if (pipe.lines.ready() && (sink.takes() || !sink.room()))
                    || (pipe.lines.wants_read() && !pipe.reading && pipe.token.is_some() && sink.takes())
                {
                    return true;
                }
            }
            let mut settled = state.stdin.is_none() && state.ready_settled();
            for pipe in &state.pipes {
                settled &= pipe.settled();
            }
            if description.name() == self.settings.serve.as_ref() {
                settled &= self.closing_control == [None, None];
            }
            if state.spawn && settled {
                return true;
            }
        }
        false
    }
    fn next_deadline(&self) -> Option<Time> {
        let deadline = self.next_policy_deadline();
        match (self.io.next_deadline(), deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) | (None, a) => a,
        }
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        let mut deadline = self.machine.next_deadline();
        for state in &self.children {
            if let Some(ready) = &state.ready
                && let Some(at) = ready.next_deadline()
            {
                deadline = Some(deadline.unwrap_or(at).min(at));
            }
        }
        deadline
    }
    fn is_empty(&self) -> bool {
        self.phase == Phase::Closing
            && self.root == Root::Closed
            && self.machine.is_idle()
            && self.io.is_empty()
            && self.requests.is_empty()
            && self.orders.is_empty()
            && self.told.is_empty()
            && self.completions.is_empty()
            && self.submissions.is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        if !self.is_empty() {
            return None;
        }
        Some(Exit::Code(u8::from(self.failed || self.machine.outcome() != Some(Outcome::Stopped))))
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        io::operations(&self.env.limits).expect("io operations").saturating_add(2)
    }
}
