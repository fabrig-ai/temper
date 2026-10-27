//! Hosts the pure parent machine while retaining the lock (protocol/setup.md, 6.4).
use crate::run::{Failed, checked};
use skein_io::{
    Event, Io, Limits, Request,
    kernel::{Complete, Exit, Fd, Pipe, ServiceSignal, Signal, Spawn, Submit, Target, Way},
};
use skein_lib::{
    Env, Queue, Time, Token, Wall,
    stream::{Down, Read, Up},
};
use skein_shell::{Clock, Config, Host, Kernel};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use temper_forge_setup::{
    parent::{Act, Event as ParentEvent, Parent, Status, Stop},
    run::Launch,
};
const OWNER: Token = Token::new(1);
/// Admits the launch before spawning, keeping its lock until the loop settles.
pub fn run(limits: &Limits, launch: &Launch, address: std::net::SocketAddr) -> Result<u8, Failed> {
    let _lock = checked(limits, launch)?;
    let program = std::env::current_exe().map_err(Failed::Exec)?;
    let spec = &launch.exec;
    let signals = skein_shell::open_termination_signals()
        .map_err(|n| Failed::Exec(io::Error::other(format!("signals: errno {n}"))))?;
    let operations = skein_io::operations(limits).expect("usable io limits");
    let mut kernel = match Kernel::open(Config { operations }) {
        Ok(kernel) => kernel,
        Err(why) => {
            skein_shell::close_keeper_fd(signals);
            return Err(Failed::Exec(io::Error::other(why.to_string())));
        }
    };
    let root = match skein_shell::open_root(std::path::Path::new("/")) {
        Ok(root) => root,
        Err(n) => {
            skein_shell::close_keeper_fd(signals);
            return Err(Failed::Exec(io::Error::other(format!("root: errno {n}"))));
        }
    };
    let mut io = Io::new(limits);
    let listener = io.adopt_listener(Fd::new(3), address).expect("room for listener");
    let mut args: Vec<Box<[u8]>> =
        // Kill: a Forgejo left shutting down gracefully would meet a restarted
        // forge run's on the box's socket and its database (protocol/setup.md, 6.4).
        [
            b"internal".as_slice(),
            b"forge-child",
            b"--parent",
            crate::process::id().to_string().as_bytes(),
            b"--signal",
            b"kill",
            b"--",
            &spec.program,
        ]
            .into_iter()
            .map(Into::into)
            .collect();
    args.extend(spec.arguments.iter().cloned());
    let spawn = Spawn {
        program: program.as_os_str().as_bytes().into(),
        args: args.into_boxed_slice(),
        env: spec.environment.clone(),
        root,
        dir: spec.directory.strip_prefix(b"/").expect("absolute command directory").into(),
        pipes: Box::new([
            Pipe { child: 1, way: Way::Out, parent: None },
            Pipe { child: 2, way: Way::Out, parent: None },
            Pipe { child: 3, way: Way::Given(listener), parent: None },
        ]),
    };
    let mut host = Running {
        io,
        limits: *limits,
        operations,
        signal: None,
        pending_signals: Some(signals),
        spawn: Some(spawn),
        child: None,
        pipes: [None, None],
        listener: Some(listener),
        parent: Parent::running(),
        exit: None,
        failure: None,
        complete: Queue::with_capacity(operations),
        subs: Queue::with_capacity(operations),
        events: Queue::with_capacity(
            skein_io::MAX_OUT_UP.events.max(skein_io::MAX_OUT_RESUME.events).max(skein_io::MAX_OUT_FIRE.events),
        ),
    };
    skein_shell::drive(&mut kernel, &Clock::new(), &mut host);
    skein_shell::close_keeper_fd(root);
    if let Some(signals) = host.pending_signals.take() {
        skein_shell::close_keeper_fd(signals);
    }
    if let Some(why) = host.failure {
        return Err(why);
    }
    Ok(host.exit.expect("a completed child's exit"))
}

struct Running {
    io: Io,
    limits: Limits,
    operations: u32,
    signal: Option<Token>,
    pending_signals: Option<Fd>,
    spawn: Option<Spawn>,
    child: Option<Token>,
    pipes: [Option<Token>; 2],
    listener: Option<Token>,
    parent: Parent,
    exit: Option<u8>,
    failure: Option<Failed>,
    complete: Queue<Complete>,
    subs: Queue<Submit>,
    events: Queue<Event>,
}
impl Running {
    fn down(&mut self, env: &Env<Limits>, request: Request) {
        skein_io::down(&mut self.io, env, request, &mut self.subs);
    }
    fn demand(&mut self, env: &Env<Limits>, pipe: Token) {
        self.down(
            env,
            Request::Stream {
                stream: pipe,
                down: Down::Demand { read: Read::Arrived { max: self.limits.receive }, room: 0 },
            },
        );
    }
    fn events(&mut self, env: &Env<Limits>) {
        while let Some(event) = self.events.pop() {
            if let Event::Spawned { child, pipes, .. } = event {
                self.child = Some(child);
                // Signals stay blocked until io can attribute them to the spawned child.
                // This also keeps an early external shutdown from being lost before Spawned.
                if let Some(signals) = self.pending_signals.take() {
                    self.signal = Some(self.io.adopt_signals(signals).expect("room for the signal descriptor"));
                }
                for (i, pipe) in pipes.iter().take(2).enumerate() {
                    self.pipes[i] = Some(*pipe);
                    self.demand(env, *pipe);
                }
                if self.failure.is_some() {
                    self.down(env, Request::Close { entity: child });
                }
            } else if let Event::Stream { owner, up } = event {
                if let Some(i) = self.pipes.iter().position(|pipe| *pipe == Some(owner)) {
                    if let Up::Bytes(bytes) = up {
                        let written = if i == 0 {
                            std::io::stdout().write_all(&bytes)
                        } else {
                            std::io::stderr().write_all(&bytes)
                        };
                        if let Err(why) = written {
                            self.failure = Some(Failed::Exec(why));
                            if let Some(child) = self.child {
                                self.down(env, Request::Close { entity: child });
                            }
                        }
                        self.demand(env, owner);
                    } else if matches!(up, Up::End | Up::Failed(_)) {
                        self.down(env, Request::Close { entity: owner });
                    }
                }
            } else if let Event::Exited { exit, .. } = event {
                let status = match exit {
                    Exit::Code(code) => Status::Code(code),
                    Exit::Signal(signal) => Status::Signal(signal),
                };
                self.act(env, ParentEvent::Exited(status));
            } else if let Event::Shutdown { signal, from } = event {
                let stop = match signal {
                    ServiceSignal::Terminate => Stop::Terminate,
                    ServiceSignal::Interrupt => Stop::Interrupt,
                };
                self.act(env, ParentEvent::Shutdown { stop, from_forgejo: from.is_some() && from == self.child });
            } else if let Event::Failed { owner, error } = event {
                self.failure = Some(Failed::Exec(io::Error::other(format!("spawn: {error:?}"))));
                if let Some(child) = self.child {
                    self.down(env, Request::Close { entity: child });
                }
                if owner != OWNER {
                    self.down(env, Request::Close { entity: owner });
                }
            } else if let Event::Closed { owner } = event {
                if owner == OWNER {
                    self.child = None;
                }
                if self.signal == Some(owner) {
                    self.signal = None;
                }
            }
        }
        if self.spawn.is_none() && self.child.is_none() && (self.exit.is_some() || self.failure.is_some()) {
            if let Some(signal) = self.signal.take() {
                self.down(env, Request::Close { entity: signal });
            }
            if let Some(listener) = self.listener.take() {
                self.down(env, Request::Close { entity: listener });
            }
        }
    }
    fn act(&mut self, env: &Env<Limits>, event: ParentEvent) {
        match self.parent.step(event) {
            Act::Nothing => {}
            Act::Forward(stop) => {
                if let Some(child) = self.child {
                    let signal = match stop {
                        Stop::Terminate => Signal::Terminate,
                        Stop::Interrupt => Signal::Interrupt,
                    };
                    self.down(env, Request::Signal { child, signal, to: Target::Child });
                }
            }
            Act::End(code) => self.exit = Some(code),
        }
    }
}
impl Host for Running {
    fn iterate(&mut self, now: Time, wall: Wall) {
        let env = Env { now, wall, limits: self.limits };
        while self.io.is_ready() {
            skein_io::resume(&mut self.io, &env, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        while let Some(complete) = self.complete.pop() {
            skein_io::up(&mut self.io, &env, complete, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        while self.io.is_due(now) {
            skein_io::fire(&mut self.io, &env, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        if let Some(spawn) = self.spawn.take() {
            self.down(&env, Request::Spawn { owner: OWNER, spawn });
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.subs
    }
    fn work_pending(&self, now: Time) -> bool {
        self.io.is_ready() || self.io.is_due(now) || !self.complete.is_empty()
    }
    fn next_deadline(&self) -> Option<Time> {
        self.io.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.spawn.is_none() && self.io.is_empty()
    }
    fn worst_case(&self) -> u64 {
        skein_io::worst_case(&self.limits).expect("io memory fits")
            + Queue::<Complete>::worst_case(self.operations).expect("completions fit")
            + Queue::<Submit>::worst_case(self.operations).expect("submissions fit")
            + Queue::<Event>::worst_case(self.events.capacity()).expect("events fit")
    }
    fn operations(&self) -> u32 {
        self.operations
    }
}
