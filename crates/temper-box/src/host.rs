//! The machine on skein-io, with one bounded chunk per child pipe.
use crate::{
    layout::{self, Placed},
    limits::{Limits, Unfit},
    lines::{self, Lines, Output},
    logs::{Logs, Piece},
    machine::{Input, Machine, Order, Outcome, Said},
    pipe::{self, Command, Line, Reader},
    ready::Ready,
    records::Records,
    sink::{Sink, Terminal},
    terminal::{self, Fact, Formatter, Item, Tail, Target},
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
/// Which diagnostic lines also reach the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Echo {
    None,
    Steps,
}
/// The descriptors and command inherited from the binary.
#[derive(Debug)]
pub struct Settings {
    pub program: Box<[u8]>,
    pub environment: Box<[Box<[u8]>]>,
    pub serve: Box<[u8]>,
    pub listeners: Box<[(Fd, core::net::SocketAddr)]>,
    pub root: Fd,
    pub echo: Echo,
    pub data: Box<[u8]>,
    pub signals: Fd,
    pub stdout: Fd,
    pub stderr: Fd,
}
/// Serve URL payload cap (LF excluded); diagnostic pieces share the read cap.
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
    withdrawn: Option<Wall>,
    hold: bool,
    line: Option<Box<[u8]>>,
    stdin: Option<Sink>,
    ready: Option<Ready>,
    ready_wall: Wall,
    tail: Tail,
}
impl Child {
    #[expect(clippy::manual_map, reason = "programming-model.md: match rather than Option::map")]
    fn new(serve: &[u8], step: &Step, tail_lines: u32) -> Self {
        Self {
            owner: None,
            process: None,
            pipes: [Pipe::new(lines::route(serve, step.name(), 1)), Pipe::new(false)],
            spawn: false,
            withdrawn: None,
            hold: false,
            line: None,
            stdin: None,
            ready_wall: Wall::EPOCH,
            tail: Tail::new(tail_lines),
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
    line_wall: Wall,
}
/// All io and policy state owned by one box process.
#[derive(Debug)]
struct Told {
    event: Event,
    wall: Wall,
}

#[derive(Debug)]
pub struct Service {
    io: Io,
    env: Env<io::Limits>,
    machine: Machine,
    steps: Box<[Placed]>,
    records: Records,
    logs: Logs,
    record_wall: Wall,
    control: Option<Control>,
    closing_control: [Option<Token>; 2],
    line_bytes: u32,
    reply_entries: u32,
    settings: Settings,
    children: Box<[Child]>,
    terminal: Terminal,
    formatter: Formatter,
    terminate_pending: bool,
    failure: Option<(Fact, Wall)>,
    deferred: Queue<Item>,
    listeners: Box<[(core::net::SocketAddr, Token)]>,
    listeners_closing: usize,
    signals: Token,
    phase: Phase,
    failed: bool,
    serial: u64,
    root: Root,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    events: Queue<Event>,
    told: Queue<Told>,
    requests: Queue<Request>,
    orders: Queue<Order>,
    order_walls: Queue<Wall>,
    memory: u64,
}
impl Service {
    /// Checks the policy limits and adopts the binary's descriptors.
    pub fn new(limits: &Limits, steps: Steps, settings: Settings) -> Result<Self, Unfit> {
        let reply_entries = reply_entries(&steps);
        let steps = layout::flatten(steps);
        let machine = Machine::new(limits, &steps)?;
        let records = Records::new(&steps);
        let logs = Logs::new(&steps, &settings.data, limits.log_bytes);
        let count = u32::try_from(steps.len()).expect("steps fit u32");
        let mut bounds = io_limits(count);
        bounds.sockets = bounds
            .sockets
            .checked_add(u32::try_from(settings.listeners.len()).expect("bounded listeners"))
            .expect("listener slots");
        bounds.intake = bounds.intake.max(limits.line_bytes);
        bounds.output = bounds
            .output
            .max(limits.line_bytes.checked_add(10).expect("notice timestamp"))
            .max(terminal::max_bytes(&steps))
            .max(Logs::max_notice(&steps, &settings.data))
            .max(terminal::failure_bytes(&steps, &settings.data))
            .max(LINE_BYTES.checked_add(1).expect("URL LF"));
        let mut children = List::with_capacity(count);
        let mut inputs = u64::try_from(settings.serve.len()).expect("serve name");
        for placed in &steps {
            if let Some(label) = &placed.label {
                inputs = inputs
                    .checked_add(u64::try_from(label.len()).expect("label length"))
                    .expect("bounded startup memory");
            }
            let step = &placed.step;
            bounds.output = bounds.output.max(
                LINE_BYTES
                    .checked_add(u32::try_from(step.name().len()).expect("bounded name"))
                    .expect("bounded line")
                    .checked_add(3)
                    .expect("prefix"),
            );
            children.push(Child::new(&settings.serve, step, limits.tail_lines)).expect("one child per step");
            inputs = inputs.checked_add(step_bytes(step)).expect("bounded startup memory");
        }
        inputs = inputs.checked_add(inherited_bytes(&settings, count)).expect("bounded startup memory");
        let mut io = Io::new(&bounds);
        let signals = io.adopt_signals(settings.signals).expect("three adopted slots");
        let stdout = io.adopt_write_pipe(settings.stdout, None).expect("stdout slot");
        let stderr = io.adopt_write_pipe(settings.stderr, None).expect("stderr slot");
        let listeners = adopt_listeners(&mut io, &settings.listeners);
        let capacity = io::operations(&bounds)
            .expect("bounded operations")
            .checked_mul(4)
            .expect("queue capacity")
            .max(64)
            .max(machine.max_out().checked_mul(2).expect("policy queue progress"));
        let entries = count.checked_mul(2).expect("terminal entries").checked_add(1).expect("terminal entries").max(3);
        let memory = footprint(limits, &settings, &steps, &bounds, capacity, reply_entries, inputs);
        Ok(Self {
            io,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: bounds },
            machine,
            steps,
            records,
            logs,
            record_wall: Wall::EPOCH,
            control: None,
            closing_control: [None, None],
            line_bytes: limits.line_bytes,
            reply_entries,
            settings,
            children: children.into_boxed(),
            terminal: Terminal::new(stdout, stderr, entries),
            formatter: Formatter::new(),
            terminate_pending: false,
            failure: None,
            deferred: Queue::with_capacity(entries),
            listeners: listeners.into_boxed(),
            listeners_closing: 0,
            signals,
            phase: Phase::New,
            failed: false,
            serial: 1_u64 << 63,
            root: Root::Open,
            completions: Queue::with_capacity(capacity),
            submissions: Queue::with_capacity(capacity),
            events: Queue::with_capacity(capacity),
            told: Queue::with_capacity(capacity),
            requests: Queue::with_capacity(capacity),
            orders: Queue::with_capacity(capacity),
            order_walls: Queue::with_capacity(capacity),
            memory,
        })
    }
    fn room(&self, bound: io::MaxOut) -> bool {
        self.events.room() >= bound.events
            && self.told.room() >= self.events.len().checked_add(bound.events).expect("retained event admission")
            && self.submissions.room() >= bound.submissions
    }
    fn progress_event(&self, event: &Event) -> bool {
        match event {
            Event::Stream { owner, .. } | Event::Closed { owner } => self.terminal.owns(*owner),
            Event::Shutdown { .. }
            | Event::Spawned { .. }
            | Event::Exited { .. }
            | Event::Failed { .. }
            | Event::Listening { .. }
            | Event::Accepted { .. }
            | Event::Connecting { .. }
            | Event::Connected { .. }
            | Event::Output { .. }
            | Event::Usage { .. }
            | Event::Resized
            | Event::Window { .. } => false,
        }
    }
    fn events_runnable(&self) -> bool {
        if self.requests.room() >= 6 && self.orders.room() >= self.machine.max_out() && !self.told.is_empty() {
            return true;
        }
        if self.requests.room() >= 2 {
            for told in &self.told {
                if self.progress_event(&told.event) {
                    return true;
                }
            }
        }
        false
    }
    fn order_ready(&self) -> bool {
        match self.orders.iter().next() {
            Some(Order::Terminal { fact: Fact::Starting { step } })
                if self.steps.get(*step).expect("step").asked.is_some() =>
            {
                true
            }
            Some(Order::Terminal { fact }) => {
                self.failure.is_none()
                    && self.deferred.is_empty()
                    && (self.tail_candidate(*fact).is_some() || self.terminal.takes(1) || self.terminal.failed())
            }
            Some(Order::Say { .. }) => self.reply_ready(),
            Some(
                Order::Spawn { .. }
                | Order::Withdraw { .. }
                | Order::Hold { .. }
                | Order::Wait { .. }
                | Order::Signal { .. }
                | Order::Close { .. }
                | Order::Record { .. },
            ) => true,
            None => false,
        }
    }
    fn reply_ready(&self) -> bool {
        match &self.control {
            Some(control) => !control.sink.room() || control.sink.takes(),
            None => true,
        }
    }
    fn input(&mut self, input: Input) {
        policy_input(&mut self.machine, &self.env, input, &mut self.orders, &mut self.order_walls);
    }
    fn event(&mut self, event: Event) {
        for state in &mut self.children {
            if let Some(ready) = &mut state.ready
                && ready.owns(&event)
            {
                state.ready_wall = self.env.wall;
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
                        policy_input(
                            &mut self.machine,
                            &self.env,
                            Input::Exited { step, exit },
                            &mut self.orders,
                            &mut self.order_walls,
                        );
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
                if self.terminal.owns(owner) && self.terminal.closed(owner) {
                    self.failed = true;
                    self.terminate_pending = true;
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
                        policy_input(&mut self.machine, &self.env, input, &mut self.orders, &mut self.order_walls);
                    }
                    for pipe in &mut state.pipes {
                        if pipe.token == Some(owner) {
                            pipe.token = None;
                            pipe.reading = false;
                            pipe.lines.end(self.env.wall);
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
                        line_wall: Wall::EPOCH,
                    });
                }
                policy_input(
                    &mut self.machine,
                    &self.env,
                    Input::Started { step },
                    &mut self.orders,
                    &mut self.order_walls,
                );
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
                        if control.line.is_some() {
                            control.line_wall = self.env.wall;
                        }
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
        if self.terminal.owns(owner) {
            if self.terminal.up(owner, up, self.env.limits.output, &mut self.requests) {
                self.failed = true;
                self.terminate_pending = true;
            }
            return;
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
                        Up::Bytes(bytes) => pipe.lines.push(bytes, self.env.wall),
                        Up::End | Up::Failed(_) => {
                            pipe.lines.end(self.env.wall);
                            self.requests.push(Request::Close { entity: owner });
                        }
                        Up::Room => {}
                    }
                    if fd == 0
                        && let Some(line) = pipe.lines.first()
                        && self.machine.outcome().is_none()
                    {
                        state.line = Some(line);
                        policy_input(
                            &mut self.machine,
                            &self.env,
                            Input::Line { step },
                            &mut self.orders,
                            &mut self.order_walls,
                        );
                    }
                    return;
                }
            }
        }
    }
    fn terminal_fact(&mut self, fact: Fact, wall: Wall) {
        if let Fact::Starting { step } = fact
            && self.steps.get(step).expect("step fact").asked.is_some()
        {
            return;
        }
        if self.tail_candidate(fact).is_some() && !self.terminal.failed() {
            assert!(self.failure.is_none(), "one waiting failure block");
            self.failure = Some((fact, wall));
        } else if !self.terminal.failed() {
            self.terminal.push(self.formatter.fact(fact, &self.steps, wall));
        }
    }
    fn tail_candidate(&self, fact: Fact) -> Option<usize> {
        let step = terminal::tail_step(fact)?;
        if self.settings.echo == Echo::None && !self.logs.failed(step) { Some(step) } else { None }
    }
    fn failure_ready(&self) -> bool {
        let Some((fact, _)) = self.failure else {
            return false;
        };
        if !self.terminal.takes(1) && !self.terminal.failed() {
            return false;
        }
        if let Some(step) = self.tail_candidate(fact) {
            if self.logs.step() == Some(step) {
                return false;
            }
            for pipe in &self.children.get(step).expect("failure step").pipes {
                if !pipe.settled() {
                    return false;
                }
            }
        }
        true
    }
    fn failure_work(&mut self) {
        if !self.failure_ready() {
            return;
        }
        let (fact, wall) = self.failure.take().expect("ready failure");
        if self.terminal.failed() {
            return;
        }
        let mut item = self.formatter.fact(fact, &self.steps, wall);
        if let Some(step) = self.tail_candidate(fact) {
            self.children.get(step).expect("failure step").tail.append(&mut item, &self.logs.path(step));
        }
        self.terminal.push(item);
    }
    fn terminal_available(&self, step: usize, plain: bool) -> bool {
        match self.failure {
            None => self.deferred.is_empty(),
            // Output retained from the failed step's old stdout precedes its
            // failure. Drain it before the block, including legacy fragments.
            Some((fact, _)) => plain && terminal::tail_step(fact) == Some(step),
        }
    }
    fn order(&mut self, order: Order, wall: Wall) {
        match order {
            Order::Withdraw { step } => {
                let state = self.children.get_mut(step).expect("machine step");
                if state.spawn && state.owner.is_none() {
                    state.spawn = false;
                    state.hold = false;
                    state.line = None;
                    state.withdrawn = Some(wall);
                }
            }
            Order::Terminal { fact } => self.terminal_fact(fact, wall),
            Order::Record { forge } => self.records.write(forge),
            Order::Say { said } => {
                if let Some(control) = &mut self.control {
                    let line = match said {
                        Said::Step { step } => {
                            let asked = self.steps.get(step).expect("step").asked.expect("asked step");
                            pipe::step(asked.forge, asked.stage)
                        }
                        Said::Started { forge } => pipe::started(forge),
                        Said::Stopped { forge } => pipe::stopped(forge),
                        Said::Failed { forge, why } => {
                            let why = terminal::why(why, &self.steps);
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
    fn order_work(&mut self) {
        let mut blocked = false;
        for _ in 0..self.orders.len() {
            if self.requests.room() < 6 {
                break;
            }
            if self.failure.is_some()
                && let Some(Order::Terminal { .. }) = self.orders.iter().next()
            {
                let order = self.orders.pop().expect("counted fact");
                let wall = self.order_walls.pop().expect("fact wall");
                self.orders.push(order);
                self.order_walls.push(wall);
                continue;
            }
            if blocked || !self.order_ready() {
                if self.failure.is_none() {
                    break;
                }
                blocked = true;
                let order = self.orders.pop().expect("counted order");
                let wall = self.order_walls.pop().expect("order wall");
                self.orders.push(order);
                self.order_walls.push(wall);
                continue;
            }
            let order = self.orders.pop().expect("counted order");
            let wall = self.order_walls.pop().expect("order wall");
            self.order(order, wall);
        }
    }
    fn orders_runnable(&self) -> bool {
        if self.failure.is_none() {
            return self.order_ready();
        }
        for order in &self.orders {
            match order {
                Order::Terminal { .. } => {}
                Order::Say { .. } => return self.reply_ready(),
                Order::Spawn { .. }
                | Order::Withdraw { .. }
                | Order::Hold { .. }
                | Order::Wait { .. }
                | Order::Signal { .. }
                | Order::Close { .. }
                | Order::Record { .. } => return true,
            }
        }
        false
    }
    fn withdrawn(&mut self) {
        for (step, state) in self.children.iter_mut().enumerate() {
            if self.orders.room() < self.machine.max_out() {
                break;
            }
            if let Some(wall) = state.withdrawn.take() {
                policy_input_at(
                    &mut self.machine,
                    self.env.now,
                    wall,
                    Input::Withdrawn { step },
                    &mut self.orders,
                    &mut self.order_walls,
                );
            }
        }
    }
    fn child_ready(&mut self, step: usize) {
        let state = self.children.get_mut(step).expect("child step");
        if let Some(ready) = &mut state.ready {
            if self.machine.outcome().is_some() {
                ready.cancel(&mut self.requests);
            }
            if let Some(ok) = ready.tick(self.env.now, &mut self.serial, &mut self.requests) {
                let wall = if ok { state.ready_wall } else { self.env.wall };
                policy_input_at(
                    &mut self.machine,
                    self.env.now,
                    wall,
                    if ok { Input::Ready { step } } else { Input::NotReady { step } },
                    &mut self.orders,
                    &mut self.order_walls,
                );
            }
        }
        if let Some(stdin) = &mut state.stdin {
            stdin.tick(true, &mut self.requests);
        }
    }
    fn children(&mut self) {
        self.withdrawn();
        let failure = self.failure;
        let deferred = !self.deferred.is_empty();
        for step in 0..self.children.len() {
            if self.requests.room() < 8 || self.orders.room() < self.machine.max_out() {
                break;
            }
            self.child_ready(step);
            let state = self.children.get_mut(step).expect("child step");
            let description = &self.steps.get(step).expect("step state").step;
            for (fd, pipe) in state.pipes.iter_mut().enumerate() {
                let plain = fd == 0 && lines::route(&self.settings.serve, description.name(), 1);
                let entries = if plain { self.formatter.block_entries() } else { 1 };
                let terminal_available = match failure {
                    None => !deferred,
                    Some((fact, _)) => plain && terminal::tail_step(fact) == Some(step),
                };
                let quiet = !plain && self.settings.echo == Echo::None && !self.logs.failed(step);
                let diagnostic_ready = plain || self.logs.takes();
                if diagnostic_ready
                    && (quiet || terminal_available && self.terminal.takes(entries) || self.terminal.failed())
                    && let Some(output) = pipe.lines.take(description.name())
                {
                    match output {
                        Output::Piece { bytes, wall } => self.logs.put(Piece { step, bytes, wall }),
                        Output::Line { bytes, wall } if plain && !self.terminal.failed() => {
                            self.terminal.block(self.formatter.block(bytes, wall)).expect("atomic block room");
                            self.formatter.admitted_block();
                        }
                        Output::Legacy { bytes } if !self.terminal.failed() => {
                            self.terminal.push(Item { target: Target::Stdout, bytes });
                        }
                        Output::Line { bytes, .. } if !self.terminal.failed() => {
                            self.terminal.push(Item { target: Target::Stderr, bytes });
                        }
                        Output::Refused { wall } if !self.terminal.failed() => {
                            self.terminal.push(self.formatter.notice(terminal::REFUSAL, wall));
                        }
                        Output::Line { .. } | Output::Legacy { .. } | Output::Refused { .. } => {}
                    }
                }
                if (plain || self.logs.takes())
                    && pipe.lines.wants_read()
                    && !pipe.reading
                    && (quiet || terminal_available && self.terminal.takes(1) || self.terminal.failed())
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
            if state.spawn && settled && self.logs.takes() && failure.is_none() && !withdraw_queued(&self.orders, step)
            {
                state.spawn = false;
                state.tail.reset();
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
    fn notice_room(&self) -> bool {
        self.terminal.failed()
            || if self.failure.is_some() || !self.deferred.is_empty() {
                self.deferred.room() > 0
            } else {
                self.terminal.takes(1)
            }
    }
    fn notice(&mut self, item: Item) {
        if self.terminal.failed() {
            return;
        }
        if self.failure.is_some() || !self.deferred.is_empty() {
            self.deferred.push(item);
        } else {
            self.terminal.push(item);
        }
    }
    fn deferred_work(&mut self) {
        if self.failure.is_some() {
            return;
        }
        for _ in 0..self.deferred.len() {
            if !self.terminal.takes(1) && !self.terminal.failed() {
                break;
            }
            let item = self.deferred.pop().expect("deferred notice");
            if !self.terminal.failed() {
                self.terminal.push(item);
            }
        }
    }
    fn log_work(&mut self) {
        if self.notice_room()
            && let Some(notice) = self.logs.take_notice()
        {
            self.notice(self.formatter.notice(&notice, self.env.wall));
        }
        if self.logs.completed() {
            let echo = self.settings.echo == Echo::Steps || self.logs.failed(self.logs_step());
            if (!echo || self.notice_room())
                && let Some(piece) = self.logs.take()
            {
                let name = self.steps.get(piece.step).expect("log step").step.name();
                if echo {
                    self.notice(Item { target: Target::Stderr, bytes: piece.echo(name) });
                } else if !self.logs.failed(piece.step) {
                    self.children.get_mut(piece.step).expect("log step").tail.push(piece.echo(name));
                }
            }
        }
        self.logs.tick(self.settings.root, &mut self.submissions);
    }
    fn logs_step(&self) -> usize {
        self.logs.step().expect("completed piece")
    }
    fn record_work(&mut self) {
        if self.notice_room()
            && let Some(notice) = self.records.take_notice()
        {
            self.notice(self.formatter.notice(&notice, self.env.wall));
        }
        if !self.records.has_notice()
            && self.orders.room() >= self.machine.max_out()
            && let Some(answer) = self.records.take_answer()
        {
            policy_input_at(
                &mut self.machine,
                self.env.now,
                self.record_wall,
                answer,
                &mut self.orders,
                &mut self.order_walls,
            );
        }
        self.records.tick(self.settings.root, &mut self.submissions);
    }
    fn control_ready(&self) -> bool {
        let Some(control) = &self.control else {
            return false;
        };
        control.sink.drained()
            && self.orders.is_empty()
            && self.failure.is_none()
            && self.deferred.is_empty()
            && self.orders.room() >= self.machine.max_out()
            && (self.terminal.takes(1) || self.terminal.failed())
            && self.phase == Phase::Running
    }
    fn take_control_line(&mut self) {
        if !self.control_ready() {
            return;
        }
        let Some(line) = self.control.as_mut().expect("serve").line.take() else {
            return;
        };
        let wall = self.control.as_ref().expect("serve").line_wall;
        let current_wall = self.env.wall;
        self.env.wall = wall;
        match line {
            Line::Whole(line) => match pipe::command(&line) {
                Command::Start(forge) => self.input(Input::Ask { forge }),
                Command::Stop(forge) => self.input(Input::Stop { forge }),
                Command::Unknown => {
                    if !self.terminal.failed() {
                        self.terminal.push(self.formatter.notice(b"box: serve: unknown pipe line\n", self.env.wall));
                    }
                }
            },
            Line::TooLong => {
                if !self.terminal.failed() {
                    self.terminal.push(self.formatter.notice(b"box: serve: pipe line too long\n", self.env.wall));
                }
            }
        }
        self.env.wall = current_wall;
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
            && (self.terminal.takes(1) || self.terminal.failed())
            && self.phase == Phase::Running
    }
    fn control_work(&mut self) {
        if self.requests.room() < 5 {
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
        if !self.machine.is_idle()
            || self.machine.outcome().is_none()
            || !self.records.ready()
            || !self.orders.is_empty()
            || self.failure.is_some()
            || !self.deferred.is_empty()
        {
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
        if !self.logs.closed() {
            if self.logs.settled() {
                self.logs.close();
            }
            return;
        }
        if self.phase != Phase::Closing {
            if self.requests.room() == 0 {
                return;
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
fn reply_entries(steps: &Steps) -> u32 {
    let mut longest = 0_u32;
    for group in &steps.asked {
        longest = longest.max(u32::try_from(group.steps.len()).expect("bounded group"));
    }
    longest.checked_add(2).expect("bounded reply queue")
}
// A terminal fact or pipe reply can hold a Withdraw in the order queue after
// its Spawn has been consumed. Admission must already respect that withdrawal.
fn withdraw_queued(orders: &Queue<Order>, step: usize) -> bool {
    for order in orders {
        if *order == (Order::Withdraw { step }) {
            return true;
        }
    }
    false
}
// The companion queue has exactly the order queue's capacity. Every policy
// entry stamps ALL newly emitted orders before another entry or consumption;
// admission to the order queue therefore reserves its walls as well.
fn stamp_orders(orders: &Queue<Order>, walls: &mut Queue<Wall>, wall: Wall) {
    let added = orders.len().checked_sub(walls.len()).expect("paired policy orders");
    for _ in 0..added {
        walls.push(wall);
    }
    assert_eq!(orders.len(), walls.len(), "paired policy orders");
}
fn policy_input(
    machine: &mut Machine,
    env: &Env<io::Limits>,
    input: Input,
    orders: &mut Queue<Order>,
    walls: &mut Queue<Wall>,
) {
    policy_input_at(machine, env.now, env.wall, input, orders, walls);
}
fn policy_input_at(
    machine: &mut Machine,
    now: Time,
    wall: Wall,
    input: Input,
    orders: &mut Queue<Order>,
    walls: &mut Queue<Wall>,
) {
    machine.input(now, input, orders);
    stamp_orders(orders, walls, wall);
}
/// All owned heap, including impossible simultaneous launch/chunk payloads.
fn footprint(
    limits: &Limits,
    settings: &Settings,
    steps: &[Placed],
    bounds: &io::Limits,
    capacity: u32,
    reply_entries: u32,
    inputs: u64,
) -> u64 {
    let count = u32::try_from(steps.len()).expect("bounded steps");
    let entries = count.checked_mul(2).expect("terminal entries").checked_add(1).expect("terminal entries").max(3);
    // Each queued record can own a launch or a chunk. Count even the impossible
    // case where every slot holds the largest launch and the largest chunk.
    let payload = inputs
        .checked_add(u64::from(bounds.output))
        .expect("bounded startup memory")
        .checked_add(
            u64::try_from(size_of::<kernel::Pipe>())
                .expect("pipe spec")
                .checked_mul(
                    u64::try_from(settings.listeners.len())
                        .expect("listener count")
                        .checked_add(4)
                        .expect("bounded startup memory"),
                )
                .expect("bounded startup memory"),
        )
        .expect("bounded startup memory");
    let queues = Queue::<Complete>::worst_case(capacity)
        .expect("completion queue")
        .checked_add(Queue::<Submit>::worst_case(capacity).expect("submission queue"))
        .expect("bounded startup memory")
        .checked_add(Queue::<Event>::worst_case(capacity).expect("event queue"))
        .expect("raw events")
        .checked_add(Queue::<Told>::worst_case(capacity).expect("retained events"))
        .expect("captured walls")
        .checked_add(Queue::<Request>::worst_case(capacity).expect("request queue"))
        .expect("bounded startup memory")
        .checked_add(Queue::<Order>::worst_case(capacity).expect("order queue"))
        .expect("bounded startup memory")
        .checked_add(Queue::<Wall>::worst_case(capacity).expect("order source walls"))
        .expect("bounded startup memory");
    io::worst_case(bounds)
        .expect("io memory")
        .checked_add(crate::limits::worst_case(limits, count).expect("policy memory"))
        .expect("bounded startup memory")
        .checked_add(inputs.checked_mul(3).expect("bounded startup memory"))
        .expect("bounded startup memory")
        .checked_add(listener_memory(&settings.listeners))
        .expect("bounded startup memory")
        .checked_add(u64::from(count).checked_mul(64).expect("bounded startup memory"))
        .expect("bounded startup memory")
        .checked_add(queues)
        .expect("bounded startup memory")
        .checked_add(
            payload.checked_mul(u64::from(capacity).checked_mul(5).expect("payload queues")).expect("queued payloads"),
        )
        .expect("bounded startup memory")
        .checked_add(List::<Child>::worst_case(count).expect("child states"))
        .expect("bounded startup memory")
        .checked_add(List::<Placed>::worst_case(count).expect("step states"))
        .expect("bounded startup memory")
        .checked_add(
            u64::from(count)
                .checked_mul(u64::from(LINE_BYTES).checked_mul(4).expect("bounded startup memory"))
                .expect("bounded startup memory"),
        )
        .expect("bounded startup memory")
        .checked_add(Terminal::worst_case(entries, bounds.output).checked_mul(2).expect("terminal and deferred FIFO"))
        .expect("terminal FIFO")
        // Construction: extracted line plus the three block items can overlap
        // assembly and the FIFO. Notices may also copy words to add a time.
        .checked_add(u64::from(bounds.output).checked_mul(4).expect("formatter overlap"))
        .expect("formatter memory")
        .checked_add(List::<Item>::worst_case(6).expect("formatter item tables"))
        .expect("formatter tables")
        .checked_add(Sink::worst_case(reply_entries, limits.line_bytes))
        .expect("bounded startup memory")
        .checked_add(u64::from(limits.line_bytes).checked_mul(2).expect("bounded startup memory"))
        .expect("bounded startup memory")
        .checked_add(Logs::worst_case(steps, &settings.data))
        .expect("logger memory")
        .checked_add(Records::worst_case(count))
        .expect("bounded startup memory")
        .checked_add(tail_memory(limits, steps))
        .expect("retained tails")
        .checked_add(link_memory(steps))
        .expect("bounded startup memory")
        .checked_add(u64::from(count).checked_mul(Ready::worst_case()).expect("bounded startup memory"))
        .expect("bounded startup memory")
}
fn listener_memory(held: &[(Fd, core::net::SocketAddr)]) -> u64 {
    let count = u32::try_from(held.len()).expect("bounded listeners");
    u64::try_from(size_of_val(held))
        .expect("held listener slots")
        .checked_add(List::<(core::net::SocketAddr, Token)>::worst_case(count).expect("listener tokens"))
        .expect("bounded startup memory")
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
    let mut out = List::with_capacity(
        u32::try_from(environment.len()).expect("bounded environment").checked_add(1).expect("bounded startup memory"),
    );
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
fn tail_memory(limits: &Limits, steps: &[Placed]) -> u64 {
    let mut memory = 0_u64;
    for step in steps {
        memory = memory.checked_add(Tail::worst_case(limits.tail_lines, step.step.name().len())).expect("tail memory");
    }
    memory
        .checked_add(u64::try_from(size_of::<Option<(Fact, Wall)>>()).expect("waiting failure state"))
        .expect("failure memory")
}
fn link_memory(steps: &[Placed]) -> u64 {
    let mut memory = 0_u64;
    for step in steps {
        if step.step.kind() == Kind::Link {
            memory = memory
                .checked_add(u64::from(LINE_BYTES))
                .expect("bounded startup memory")
                .checked_add(Sink::worst_case(1, LINE_BYTES))
                .expect("bounded startup memory");
        }
    }
    memory
}
fn step_bytes(step: &Step) -> u64 {
    let mut bytes = u64::try_from(size_of_val(step.listeners())).expect("listener addresses");
    for part in [step.name(), step.started_notice()] {
        bytes = bytes.checked_add(u64::try_from(part.len()).expect("step bytes")).expect("bounded startup memory");
    }
    bytes = bytes
        .checked_add(u64::try_from(size_of_val(step.arguments())).expect("argument slots"))
        .expect("bounded startup memory");
    for argument in step.arguments() {
        bytes =
            bytes.checked_add(u64::try_from(argument.len()).expect("argument length")).expect("bounded startup memory");
    }
    bytes
}
fn inherited_bytes(settings: &Settings, count: u32) -> u64 {
    let mut inputs = 0_u64;
    inputs = inputs
        .checked_add(
            u64::try_from(size_of_val(settings.environment.as_ref()))
                .expect("environment slots")
                .checked_mul(u64::from(count).checked_add(1).expect("bounded startup memory"))
                .expect("bounded startup memory"),
        )
        .expect("bounded startup memory");
    for value in &settings.environment {
        inputs = inputs
            .checked_add(
                u64::try_from(value.len())
                    .expect("environment length")
                    .checked_mul(u64::from(count).checked_add(1).expect("bounded startup memory"))
                    .expect("bounded startup memory"),
            )
            .expect("bounded startup memory");
    }
    inputs = inputs
        .checked_add(
            u64::try_from(settings.program.len())
                .expect("program length")
                .checked_mul(u64::from(count).checked_add(1).expect("bounded startup memory"))
                .expect("bounded startup memory"),
        )
        .expect("bounded startup memory");
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
            io::resume(&mut self.io, &self.env, &mut self.events, &mut self.submissions);
        }
        if !self.io.is_ready() {
            for _ in 0..self.completions.len() {
                if !self.room(io::MAX_OUT_UP) {
                    break;
                }
                let complete = self.completions.pop().expect("counted completion");
                if Logs::owns(complete.op) {
                    self.logs.complete(complete);
                } else if Records::owns(complete.op) {
                    self.record_wall = wall;
                    self.records.complete(complete);
                } else if complete.op == ROOT_CLOSE {
                    self.root = Root::Closed;
                } else {
                    io::up(&mut self.io, &self.env, complete, &mut self.events, &mut self.submissions);
                }
            }
            if self.completions.is_empty() {
                for _ in 0..self.env.limits.sockets.saturating_mul(2) {
                    if !self.io.is_due(now) || !self.room(io::MAX_OUT_FIRE) {
                        break;
                    }
                    io::fire(&mut self.io, &self.env, &mut self.events, &mut self.submissions);
                }
            }
        }
        self.record_work();
        self.log_work();
        if self.phase == Phase::New
            && self.logs.ready()
            && self.records.ready()
            && self.orders.room() >= self.machine.max_out()
        {
            self.machine.start(now, self.records.recorded(), &mut self.orders);
            stamp_orders(&self.orders, &mut self.order_walls, wall);
            self.phase = Phase::Running;
        }
        // Capture wall time on intake, before an ordinary event can wait behind
        // policy pressure. Scan once: progress bypasses a blocked ordinary front,
        // while all ordinary events keep their relative order.
        for _ in 0..self.events.len() {
            let event = self.events.pop().expect("captured event");
            self.told.push(Told { event, wall });
        }
        // An ordinary event can issue four requests. Leave two slots for the
        // active terminal's Send and full-budget Demand, even if policy stalls.
        let mut blocked = false;
        for _ in 0..self.told.len() {
            let told = self.told.pop().expect("counted event");
            let progress = self.progress_event(&told.event);
            if self.requests.room() < if progress { 2 } else { 6 }
                || !progress && (blocked || self.orders.room() < self.machine.max_out())
            {
                if !progress {
                    blocked = true;
                }
                self.told.push(told);
                continue;
            }
            self.env.wall = told.wall;
            self.event(told.event);
            self.env.wall = wall;
        }
        if self.terminate_pending && self.orders.room() >= self.machine.max_out() {
            self.terminate_pending = false;
            self.input(Input::Terminate);
        }
        if self.told.is_empty() && self.orders.room() >= self.machine.max_out() {
            self.machine.fire(now, &mut self.orders);
            stamp_orders(&self.orders, &mut self.order_walls, wall);
        }
        self.failure_work();
        self.deferred_work();
        self.order_work();
        self.children();
        self.control_work();
        self.record_work();
        self.log_work();
        self.failure_work();
        self.deferred_work();
        self.finish();
        if self.requests.room() >= 2 {
            self.terminal.tick(self.phase == Phase::Closing, &mut self.requests);
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
        if self.phase == Phase::New && self.logs.ready() && self.records.ready()
            || !self.completions.is_empty() && self.room(io::MAX_OUT_UP)
            || !self.events.is_empty()
            || self.events_runnable()
            || !self.requests.is_empty() && self.io.takes() && self.submissions.room() >= io::MAX_OUT_DOWN.submissions
            || !self.orders.is_empty() && self.orders_runnable() && self.requests.room() >= 6
            || self.terminate_pending && self.orders.room() >= self.machine.max_out()
            || self.io.is_ready() && self.room(io::MAX_OUT_RESUME)
            || self.io.is_due(now) && self.room(io::MAX_OUT_FIRE)
        {
            return true;
        }
        if let Some(at) = self.next_deadline()
            && at <= now
        {
            return true;
        }
        if self.failure_ready()
            || self.logs.pending()
            || self.logs.has_notice() && self.notice_room()
            || !self.deferred.is_empty() && self.failure.is_none() && (self.terminal.takes(1) || self.terminal.failed())
            || self.logs.completed()
                && (self.settings.echo == Echo::None && !self.logs.failed(self.logs_step()) || self.notice_room())
        {
            return true;
        }
        if self.records.pending() && (!self.records.has_notice() || self.notice_room()) {
            return true;
        }
        if let Some(control) = &self.control
            && self.requests.room() >= 5
            && (control.sink.pending()
                || control.sink.needs_flush()
                || self.can_read_control()
                || (control.line.is_some() && self.control_ready()))
        {
            return true;
        }
        if self.terminal.pending(self.phase == Phase::Closing) && self.requests.room() >= 2 {
            return true;
        }
        if self.orders.room() >= self.machine.max_out() {
            for state in &self.children {
                if state.withdrawn.is_some() {
                    return true;
                }
            }
        }
        if self.orders.room() < self.machine.max_out() || self.requests.room() < 8 {
            return false;
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
                let entries = if plain { self.formatter.block_entries() } else { 1 };
                let quiet = !plain && self.settings.echo == Echo::None && !self.logs.failed(step);
                let visible = self.terminal_available(step, plain);
                if self.orders.room() >= self.machine.max_out()
                    && self.requests.room() >= 8
                    && (plain || self.logs.takes())
                    && ((pipe.lines.ready()
                        && (quiet || visible && self.terminal.takes(entries) || self.terminal.failed()))
                        || (pipe.lines.wants_read()
                            && !pipe.reading
                            && pipe.token.is_some()
                            && (quiet || visible && self.terminal.takes(1) || self.terminal.failed())))
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
            if state.spawn
                && settled
                && self.logs.takes()
                && self.failure.is_none()
                && !withdraw_queued(&self.orders, step)
            {
                return true;
            }
        }
        false
    }
    fn next_deadline(&self) -> Option<Time> {
        let deadline = self.next_policy_deadline();
        let io_deadline = if self.room(io::MAX_OUT_FIRE) { self.io.next_deadline() } else { None };
        match (io_deadline, deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) | (None, a) => a,
        }
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        if self.orders.room() < self.machine.max_out() || !self.told.is_empty() {
            return None;
        }
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
            && self.logs.closed()
            && self.failure.is_none()
            && self.deferred.is_empty()
            && self.machine.is_idle()
            && self.io.is_empty()
            && self.requests.is_empty()
            && self.orders.is_empty()
            && self.order_walls.is_empty()
            && self.events.is_empty()
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
        io::operations(&self.env.limits).expect("io operations").checked_add(3).expect("root, records, logger")
    }
}

/// Test-only saturation of the real host's destination queues. The simulator
/// still owns every completion and ACK; no service or io behaviour is replaced.
#[cfg(test)]
pub(crate) mod pressure {
    use super::{Event, Fact, Item, Order, Service, Target, Told};
    use skein_lib::{Wall, bytes};
    pub fn saturate(service: &mut Service) -> (u32, u32) {
        let mut queued = 0_u32;
        while service.terminal.takes(1) {
            service.terminal.push(Item { target: Target::Stderr, bytes: bytes::copy_of(b"queued\n") });
            queued = queued.checked_add(1).unwrap();
        }
        let orders = service.orders.room();
        for _ in 0..orders {
            service.orders.push(Order::Terminal { fact: Fact::Stopping });
            service.order_walls.push(service.env.wall);
        }
        // These ordinary events must stay in this order while ACKs bypass them.
        for wall in [Wall::EPOCH, Wall::from_nanos(1)] {
            service.told.push(Told { event: Event::Resized, wall });
        }
        (queued, orders)
    }
    pub fn fact_waits(service: &Service, fact: Fact, wall: Wall) -> bool {
        service.orders.iter().next() == Some(&Order::Terminal { fact })
            && service.order_walls.iter().next() == Some(&wall)
    }
    pub fn outcome(service: &Service) -> Option<super::Outcome> {
        service.machine.outcome()
    }
    pub fn blocked(service: &Service) -> bool {
        service.orders.room() == 0
            && !service.terminal.takes(1)
            && service.told.len() == 2
            && service.told.iter().next().expect("ordinary front").wall == Wall::EPOCH
            && service.told.iter().nth(1).expect("ordinary second").wall == Wall::from_nanos(1)
    }
    fn children_bounded(service: &Service) -> bool {
        for child in &service.children {
            for pipe in &child.pipes {
                if !pipe.lines.bounded() {
                    return false;
                }
            }
        }
        true
    }
    pub fn bounded(service: &Service) -> bool {
        service.orders.len() <= service.orders.capacity()
            && service.orders.len() == service.order_walls.len()
            && service.order_walls.capacity() == service.orders.capacity()
            && service.told.len() <= service.told.capacity()
            && service.events.len() <= service.events.capacity()
            && service.requests.len() <= service.requests.capacity()
            && service.submissions.len() <= service.submissions.capacity()
            && service.completions.len() <= service.completions.capacity()
            && service.terminal.bounded(service.env.limits.output)
            && children_bounded(service)
    }
}
