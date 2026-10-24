use crate::{
    limits,
    machine::{Input, Machine, Order},
};
use alloc::boxed::Box;
use skein_io::kernel::Exit;
use skein_lib::{Duration, List, Queue, Time, bytes};
use temper_forge_setup::{Kind, Step};
pub const PREPARE: usize = 0;
pub const FORGEJO: usize = 1;
pub const RUNNER: usize = 2;
pub const ONCE_1: usize = 3;
pub const ONCE_2: usize = 4;
pub const SERVE: usize = 5;
pub fn step(name: &[u8], kind: Kind) -> Step {
    command(name, &[], kind)
}
pub fn owned_steps() -> Box<[Step]> {
    Box::new([
        step(b"prepare", Kind::Once),
        step(b"forgejo", Kind::Long),
        step(b"runner", Kind::Long),
        step(b"once-1", Kind::Once),
        step(b"once-2", Kind::Once),
        step(b"serve", Kind::Long),
    ])
}
#[derive(Debug)]
pub struct Rig {
    pub machine: Machine,
    pub out: Queue<Order>,
    pub now: Time,
}
impl Rig {
    pub fn new(limits: &limits::Limits) -> Self {
        let machine = Machine::new(
            limits,
            &crate::layout::flatten(temper_forge_setup::Steps { always: owned_steps(), asked: Box::new([]) }),
        )
        .expect("valid limits");
        let out = Queue::with_capacity(machine.max_out().saturating_mul(4));
        let mut rig = Self { machine, out, now: Time::ZERO };
        rig.machine.start(rig.now, &[], &mut rig.out);
        rig
    }
    pub fn running() -> Self {
        let mut rig = Self::new(&limits::small());
        rig.completed(PREPARE);
        rig.started(FORGEJO);
        rig.started(RUNNER);
        rig.completed(ONCE_1);
        rig.completed(ONCE_2);
        rig.started(SERVE);
        rig.orders();
        rig
    }
    pub fn input(&mut self, input: Input) {
        self.machine.input(self.now, input, &mut self.out);
    }
    pub fn started(&mut self, step: usize) {
        self.input(Input::Started { step });
    }
    pub fn exited(&mut self, step: usize, exit: Exit) {
        self.input(Input::Exited { step, exit });
    }
    pub fn closed(&mut self, step: usize) {
        self.input(Input::Closed { step });
    }
    pub fn completed(&mut self, step: usize) {
        self.started(step);
        self.exited(step, Exit::Code(0));
        self.closed(step);
    }
    pub fn advance(&mut self, by: Duration) {
        self.now = self.now.saturating_add(by);
        self.machine.fire(self.now, &mut self.out);
    }
    pub fn terminate(&mut self) {
        self.input(Input::Terminate);
    }
    pub fn orders(&mut self) -> Box<[Order]> {
        let mut orders = List::with_capacity(self.out.len());
        for _ in 0..self.out.len() {
            orders.push(self.out.pop().expect("counted order")).expect("counted capacity");
        }
        orders.into_boxed()
    }
    pub fn failed_long(&mut self, step: usize) {
        self.exited(step, Exit::Code(1));
        self.closed(step);
        self.orders();
    }
    pub fn restart_forgejo(&mut self) {
        self.completed(PREPARE);
        self.started(FORGEJO);
        self.orders();
    }
}

use crate::host::{Service, Settings};
use skein_fake_machine::Machine as FakeMachine;
use skein_io::kernel::{Complete, Done, Fd, Op, ServiceSignal, Submit};
use skein_lib::{Token, Wall};
use skein_shell::Host;
use skein_sim::{Answer, Ask, Call, Config, Handle, Pid, Program, Reply, Sim};

pub fn once(name: &[u8], arguments: &[&[u8]]) -> Step {
    command(name, arguments, Kind::Once)
}
pub fn long(name: &[u8], arguments: &[&[u8]]) -> Step {
    command(name, arguments, Kind::Long)
}
pub fn link(name: &[u8], arguments: &[&[u8]]) -> Step {
    command(name, arguments, Kind::Link)
}
fn command(name: &[u8], arguments: &[&[u8]], kind: Kind) -> Step {
    let capacity = u32::try_from(arguments.len()).expect("small fixture").saturating_add(1);
    let mut args = List::with_capacity(capacity);
    for argument in arguments {
        args.push(bytes::copy_of(argument)).expect("argument slots");
    }
    args.push(bytes::copy_of(name)).expect("name last");
    Step::new(bytes::copy_of(name), args.into_boxed(), kind)
}
#[derive(Debug)]
pub struct Launch {
    pub name: Box<[u8]>,
    pub environment: Box<[Box<[u8]>]>,
    pub pipes: Box<[skein_io::kernel::Pipe]>,
    pub at: Time,
}
#[derive(Debug)]
pub struct Hosted {
    pub name: Box<[u8]>,
    pub stdio: [Option<Fd>; 3],
    pub pid: Pid,
    pub descriptors: Box<[Fd]>,
    pub listeners: Box<[Fd]>,
    pub closed: bool,
    pub input: Option<Fd>,
    pub output: Option<Fd>,
}
#[derive(Debug)]
pub struct Harness {
    pub sim: Sim,
    pub pid: Pid,
    pub service: Service,
    pub launches: List<Launch>,
    pub machine: FakeMachine,
    pub signals: Fd,
    pub stderr: Fd,
    pub stdout: Fd,
    pub calls: Queue<Call>,
    pub answers: Queue<Answer>,
    pub stubborn: bool,
    pub hosted: List<Hosted>,
    pub pending: List<Token>,
    pub control_fds: List<Fd>,
    pub replies: List<Box<[u8]>>,
    pub hold_prepare: bool,
    pub held_prepare: Option<Call>,
}
impl Harness {
    pub fn new(steps: &[Step]) -> Self {
        Self::with_stubborn(steps, false)
    }
    pub fn with_stubborn(steps: &[Step], stubborn: bool) -> Self {
        Self::grouped(temper_forge_setup::Steps { always: Box::from(steps), asked: Box::new([]) }, &[], stubborn)
    }
    pub fn grouped(steps: temper_forge_setup::Steps, items: &[skein_fake_machine::Item], stubborn: bool) -> Self {
        Self::buffered(steps, items, stubborn, Config::calm().buffer)
    }
    pub fn buffered(
        steps: temper_forge_setup::Steps,
        items: &[skein_fake_machine::Item],
        stubborn: bool,
        buffer: u32,
    ) -> Self {
        Self::configured(steps, items, stubborn, buffer, &limits::small())
    }
    pub fn configured(
        steps: temper_forge_setup::Steps,
        items: &[skein_fake_machine::Item],
        stubborn: bool,
        buffer: u32,
        limits: &limits::Limits,
    ) -> Self {
        let mut config = Config::calm();
        config.buffer = buffer;
        let mut sim = Sim::new(0x12_02, config);
        let pid = sim.spawn_process();
        let signals = sim.open_signal_source(pid);
        let stdout = sim.open_inherited_write(pid);
        let stderr = sim.open_inherited_write(pid);
        let mut machine = FakeMachine::new();
        let opened = machine.lay(items);
        let root = sim.root(pid, Handle::new(opened.raw()));
        let mut listeners = List::with_capacity(32);
        for step in &steps.always {
            open_step_listeners(&mut sim, pid, step, &mut listeners);
        }
        for group in &steps.asked {
            for (_, step) in &group.steps {
                open_step_listeners(&mut sim, pid, step, &mut listeners);
            }
        }
        let settings = Settings {
            program: bytes::copy_of(b"process_fixture"),
            environment: Box::new([
                bytes::copy_of(b"HOME=/box"),
                bytes::copy_of(b"TOKEN=whole=value"),
                bytes::copy_of(b"LISTEN_FDS=99"),
                bytes::copy_of(b"LISTEN_PID=42"),
            ]),
            serve: bytes::copy_of(b"serve"),
            listeners: listeners.into_boxed(),
            root,
            signals,
            stdout,
            stderr,
        };
        let service = Service::new(limits, steps, settings).expect("valid limits");
        Self {
            sim,
            pid,
            service,
            launches: List::with_capacity(128),
            machine,
            signals,
            stderr,
            stdout,
            calls: Queue::with_capacity(64),
            answers: Queue::with_capacity(64),
            stubborn,
            hosted: List::with_capacity(64),
            pending: List::with_capacity(64),
            control_fds: List::with_capacity(64),
            replies: List::with_capacity(128),
            hold_prepare: false,
            held_prepare: None,
        }
    }
    fn answer(&mut self) {
        self.sim.calls(&mut self.calls);
        for _ in 0..self.calls.len() {
            let call = self.calls.pop().expect("counted call");
            if let Ask::Spawn { args, env, pipes, .. } = &call.ask {
                self.launches
                    .push(Launch {
                        name: args.last().expect("step name last").clone(),
                        environment: env.clone(),
                        pipes: pipes.clone(),
                        at: self.sim.now(),
                    })
                    .expect("bounded launches");
                if self.stubborn && args.first().expect("program").as_ref() == b"never" {
                    let (_, token, _) = self.sim.hosted_spawn(call.ticket);
                    self.pending.push(token).expect("bounded hosted launches");
                }
            }
            if self.hold_prepare
                && let Ask::Spawn { args, .. } = &call.ask
                && args.last().expect("step name").as_ref() == b"prepare 3"
            {
                assert!(self.held_prepare.is_none());
                self.held_prepare = Some(call);
                continue;
            }
            skein_fake_machine::step(&mut self.machine, call, &mut self.answers);
            if self.stubborn {
                let mut answer = self.answers.pop().expect("one answer");
                if let Ok(Reply::Program(Program::Never)) = &answer.result {
                    answer.result = Ok(Reply::Program(Program::Service));
                }
                self.answers.push(answer);
            }
            self.sim.answer(&mut self.answers);
        }
    }
    fn bind(&mut self) {
        for complete in self.service.completions().iter() {
            if self.pending.as_slice().contains(&complete.op)
                && let Ok(Done::Spawned { pidfd }) = complete.result
            {
                let (pid, inherited) = self.sim.bind_service(self.pid, pidfd);
                let Op::Spawn { spawn } = &complete.kind else { panic!("hosted spawn") };
                let mut n = 0_u32;
                for pipe in &spawn.pipes {
                    match pipe.way {
                        skein_io::kernel::Way::Given(_) => n = n.checked_add(1).unwrap(),
                        skein_io::kernel::Way::In | skein_io::kernel::Way::Out => {}
                    }
                }
                let control = n.checked_add(3).unwrap();
                let answer = control.checked_add(1).unwrap();
                let serve = spawn.args.last().unwrap().as_ref() == b"serve";
                let mut descriptors =
                    List::with_capacity(u32::try_from(inherited.len()).unwrap().checked_add(1).unwrap());
                let mut listeners = List::with_capacity(n);
                let mut input = None;
                let mut output = None;
                let mut stdio = [None; 3];
                for (number, fd) in inherited {
                    if let Some(slot) = stdio.get_mut(usize::try_from(number).unwrap()) {
                        *slot = Some(fd);
                    }
                    if number >= 3 && number < control {
                        listeners.push(fd).unwrap();
                    }
                    if serve && number == control {
                        output = Some(fd);
                    }
                    if serve && number == answer {
                        input = Some(fd);
                    }
                    descriptors.push(fd).expect("four inherited pipes");
                }
                if let Op::Spawn { spawn } = &complete.kind {
                    for pipe in &spawn.pipes {
                        if serve && pipe.child == answer {
                            self.control_fds.push(pipe.parent.expect("parent pipe")).unwrap();
                        }
                    }
                }
                descriptors.push(self.sim.open_signal_source(pid)).expect("child signal source");
                let Op::Spawn { spawn } = &complete.kind else { panic!("hosted spawn") };
                self.hosted
                    .push(Hosted {
                        name: spawn.args.last().unwrap().clone(),
                        stdio,
                        pid,
                        descriptors: descriptors.into_boxed(),
                        listeners: listeners.into_boxed(),
                        closed: false,
                        input,
                        output,
                    })
                    .expect("bounded hosted children");
            }
        }
    }
    fn settle_hosted(&mut self) {
        for index in 0..self.hosted.len() {
            let child = self.hosted.get_mut(index).expect("hosted index");
            if !child.closed && !self.sim.service_running(child.pid) {
                let mut submissions = Queue::with_capacity(u32::try_from(child.descriptors.len()).unwrap());
                for (index, fd) in child.descriptors.iter().enumerate() {
                    submissions.push(Submit {
                        op: Token::new(u64::try_from(index).expect("bounded descriptors")),
                        kind: Op::Close { fd: *fd },
                    });
                }
                self.sim.submit(child.pid, &mut submissions);
                let mut completions = Queue::<Complete>::with_capacity(u32::try_from(child.descriptors.len()).unwrap());
                self.sim.reap(child.pid, &mut completions);
                self.sim.assert_hosted_settled(child.pid);
                child.closed = true;
            }
        }
    }
    pub fn turn(&mut self) {
        self.sim.reap(self.pid, self.service.completions());
        self.bind();
        self.service.iterate(self.sim.now(), Wall::EPOCH);
        self.service.drain();
        for submission in self.service.submissions().iter() {
            if let Op::PipeWrite { fd, bytes, from } = &submission.kind
                && self.control_fds.as_slice().contains(fd)
            {
                self.replies.push(Box::from(bytes.get(usize::try_from(*from).unwrap()..).unwrap())).unwrap();
            }
        }
        self.sim.submit(self.pid, self.service.submissions());
        self.answer();
        self.settle_hosted();
    }
    pub fn run_until_idle(&mut self) {
        for _ in 0..10_000_u32 {
            self.turn();
            if !self.service.work_pending(self.sim.now())
                && self.sim.next_due().is_none()
                && self.sim.ready(self.pid) == 0
                && !self.sim.deferred(self.pid)
            {
                return;
            }
        }
        unreachable!("box reaches its steady state");
    }
    pub fn run_to_end(&mut self) -> Exit {
        for _ in 0..10_000_u32 {
            self.turn();
            if let Some(exit) = self.service.exit() {
                return exit;
            }
            if !self.service.work_pending(self.sim.now()) {
                let due = match (self.sim.next_due(), self.service.next_deadline()) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, None) | (None, a) => a,
                };
                if let Some(at) = due {
                    self.sim.advance_to(at);
                }
            }
        }
        unreachable!("box settles within bounded turns");
    }
    pub fn serve_index(&self) -> u32 {
        let mut found = None;
        for index in 0..self.hosted.len() {
            let child = self.hosted.get(index).unwrap();
            if child.output.is_some() && !child.closed {
                found = Some(index);
            }
        }
        found.expect("live hosted serve")
    }
    pub fn release_prepare(&mut self) {
        self.hold_prepare = false;
        skein_fake_machine::step(&mut self.machine, self.held_prepare.take().expect("held prepare"), &mut self.answers);
        self.sim.answer(&mut self.answers);
    }
    pub fn ask(&mut self, line: &[u8]) {
        let child = self.hosted.get(self.serve_index()).expect("live serve");
        let mut submits = Queue::with_capacity(1);
        submits.push(Submit {
            op: Token::new(100),
            kind: Op::PipeWrite {
                fd: child.output.expect("serve requests pipe"),
                bytes: bytes::copy_of(line),
                from: 0,
            },
        });
        self.sim.submit(child.pid, &mut submits);
        let mut completes = Queue::with_capacity(1);
        self.sim.reap(child.pid, &mut completes);
        assert_eq!(completes.pop().unwrap().result, Ok(Done::Count(u32::try_from(line.len()).unwrap())));
    }
    pub fn read_reply(&mut self, most: u32) -> Box<[u8]> {
        let child = self.hosted.get(self.serve_index()).unwrap();
        self.read_pipe(child.pid, child.input.unwrap(), most)
    }
    pub fn child_index(&self, name: &[u8]) -> u32 {
        for index in (0..self.hosted.len()).rev() {
            let child = self.hosted.get(index).unwrap();
            if child.name.as_ref() == name && !child.closed {
                return index;
            }
        }
        panic!("live hosted child")
    }
    pub fn write_child(&mut self, index: u32, number: usize, data: &[u8]) {
        let child = self.hosted.get(index).unwrap();
        let mut submissions = Queue::with_capacity(1);
        submissions.push(Submit {
            op: Token::new(102),
            kind: Op::PipeWrite { fd: child.stdio[number].unwrap(), bytes: bytes::copy_of(data), from: 0 },
        });
        self.sim.submit(child.pid, &mut submissions);
        let mut completions = Queue::with_capacity(1);
        self.sim.reap(child.pid, &mut completions);
        assert_eq!(completions.pop().unwrap().result, Ok(Done::Count(u32::try_from(data.len()).unwrap())));
    }
    pub fn read_stdin(&mut self, index: u32, most: u32) -> Box<[u8]> {
        let child = self.hosted.get(index).unwrap();
        self.read_pipe(child.pid, child.stdio[0].unwrap(), most)
    }
    fn read_pipe(&mut self, pid: Pid, fd: Fd, most: u32) -> Box<[u8]> {
        let mut submissions = Queue::with_capacity(1);
        submissions.push(Submit {
            op: Token::new(101),
            kind: Op::PipeRead { fd, buf: bytes::zeroed(usize::try_from(most).unwrap()) },
        });
        self.sim.submit(pid, &mut submissions);
        let mut completions = Queue::with_capacity(1);
        for _ in 0..10_000_u32 {
            self.sim.reap(pid, &mut completions);
            if let Some(complete) = completions.pop() {
                let Ok(Done::Count(count)) = complete.result else { panic!("reply read") };
                let Op::PipeRead { buf, .. } = complete.kind else { panic!("reply buffer") };
                return Box::from(buf.get(..usize::try_from(count).unwrap()).unwrap());
            }
            self.turn();
        }
        unreachable!("reply available within bounded turns");
    }
    pub fn reply_bytes(&self) -> Box<[u8]> {
        let mut got = List::with_capacity(65_536);
        for line in &self.replies {
            for byte in line {
                got.push(*byte).unwrap();
            }
        }
        got.into_boxed()
    }
    pub fn terminate(&mut self) {
        self.sim.deliver_service_signal(self.pid, self.signals, ServiceSignal::Terminate);
    }
    pub fn spawn_times(&self, name: &[u8]) -> Box<[Time]> {
        let mut times = List::with_capacity(128);
        for launch in &self.launches {
            if launch.name.as_ref() == name {
                times.push(launch.at).expect("bounded launches");
            }
        }
        times.into_boxed()
    }
    pub fn stdout(&mut self) -> Box<[u8]> {
        self.sim.peer_drain(self.pid, self.stdout, 65_536).into_boxed_slice()
    }
    pub fn stderr(&mut self) -> Box<[u8]> {
        self.sim.peer_drain(self.pid, self.stderr, 65_536).into_boxed_slice()
    }
    pub fn contains(bytes: &[u8], needle: &[u8]) -> bool {
        for part in bytes.windows(needle.len()) {
            if part == needle {
                return true;
            }
        }
        false
    }
}

pub fn call(sim: &mut Sim, pid: Pid, op: Op) -> Complete {
    let mut submissions = Queue::with_capacity(1);
    submissions.push(Submit { op: Token::new(100), kind: op });
    sim.submit(pid, &mut submissions);
    let mut completions = Queue::with_capacity(1);
    sim.reap(pid, &mut completions);
    completions.pop().expect("calm operation completes")
}
pub fn socket(sim: &mut Sim, pid: Pid) -> Fd {
    let Ok(Done::Fd(fd)) = call(sim, pid, Op::Socket { family: skein_io::kernel::Family::Ipv4 }).result else {
        panic!("calm socket succeeds")
    };
    fd
}
fn open_step_listeners(sim: &mut Sim, pid: Pid, step: &Step, held: &mut List<(Fd, core::net::SocketAddr)>) {
    for address in step.listeners() {
        let fd = open_listener(sim, pid, *address);
        held.push((fd, *address)).unwrap();
    }
}
fn open_listener(sim: &mut Sim, pid: Pid, address: core::net::SocketAddr) -> Fd {
    let fd = socket(sim, pid);
    assert_eq!(call(sim, pid, Op::Bind { fd, addr: address }).result, Ok(Done::Bound(address)));
    assert_eq!(call(sim, pid, Op::Listen { fd, backlog: 4 }).result, Ok(Done::Nothing));
    fd
}
