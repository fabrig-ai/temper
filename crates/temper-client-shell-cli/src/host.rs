//! One call owns its files, pipes and signal subscription (client/tools.md, 8).
use crate::{
    arguments::{self, Class},
    directory::Directory,
    limits,
    output::Output,
    recovery::Recovery,
    signing::{Credential, Signing},
    transport::Memory,
};
use jig_client_shell_native::Shell;
use skein_io::{
    self as io,
    file_layer::FileIo,
    kernel::{Complete, Exit, Fd, Op, ServiceSignal, Submit},
};
use skein_lib::{
    Env, Queue, Time, Token, Wall,
    stream::{Down, Read, Up},
};
use skein_shell::Host;
use temper_client_tools::{jig::call::Parsed, root};
mod input;
mod sign_in;
const TAG: u64 = 1 << 61;
const FILES: u64 = 1 << 62;
const ROOM: u32 = (limits::EXCHANGES + 1) * 128; // Two bounded socket bursts plus pipe and file completions.
/// Descriptors opened by the entry point; parent belongs to this host.
#[derive(Clone, Copy)]
pub struct Startup {
    pub parent: Option<Fd>,
    pub file_parent: Option<Fd>,
    pub stdin: Fd,
    pub stdout: Fd,
    pub stderr: Fd,
    pub signals: Fd,
    pub user: u32,
    pub seed: u64,
}
/// One parsed call, with DIRECTORY's leaf, absent for help.
pub struct Invocation {
    pub directory: Option<Box<[u8]>>,
    pub parsed: Parsed,
}
/// Whether the invocation awaits its act's answer or a condition on its watch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Waiting {
    Answer,
    Condition,
}
/// The shell's one-shot host.
pub struct OneShot {
    directory: Directory,
    call: Option<Parsed>,
    class: Class,
    waiting: Waiting,
    pub(crate) root: root::Root,
    root_env: Env<root::Limits>,
    connections: jig_client_shell_native::Limits,
    requests: Queue<root::Request>,
    pub(crate) transport: Option<Memory>,
    #[cfg(test)]
    pub(crate) test_connections: Option<jig_client_shell_native::Limits>,
    signing: Option<Signing>,
    recovery: Recovery,
    startup: Startup,
    until: Option<Time>,
    link: jig_client::Link,
    io: io::Io,
    env: Env<io::Limits>,
    output: Output,
    signals: Option<Token>,
    stdin: Option<Token>,
    input: Option<Vec<u8>>,
    value: Option<skein_lib::Secret>,
    preparing: bool,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
    raw: Queue<Submit>,
    told: Queue<io::Event>,
    status: Option<u8>,
    closed: bool,
    memory: u64,
    move_file: Option<Box<[u8]>>,
    exporting: bool,
    file_end: Option<temper_client_tools::jig::FileEnd>,
    recording: Option<u32>,
}
impl OneShot {
    /// # Errors
    /// Returns unusable client limits; the shipped preset is representable.
    pub fn new(call: Invocation, startup: Startup) -> Result<Self, jig_client::Unusable> {
        Self::with_text_files(call, startup, Vec::new())
    }
    /// Construct with each amendment text file's parent opened before the loop.
    /// # Errors
    /// Returns unusable client limits.
    pub fn with_text_files(
        call: Invocation,
        startup: Startup,
        files: Vec<(u8, Fd)>,
    ) -> Result<Self, jig_client::Unusable> {
        let limits = limits::native();
        let class = match &call.parsed {
            Parsed::Call { call, .. } => arguments::class(call),
            Parsed::File { .. } => Class::Help,
        };
        let root = root::Root::new(&limits.root, startup.seed)?;
        let request_room = root::max_out(&limits.root);
        let mut io = io::Io::new(&limits.io);
        let output = Output::new(&mut io, startup.stdout, startup.stderr, limits.io.output);
        let stdin = Some(io.adopt_read_pipe(startup.stdin).expect("stdin slot"));
        let signals = Some(io.adopt_signals(startup.signals).expect("signal slot"));
        let memory = io::worst_case(&limits.io).expect("io envelope")
            + FileIo::worst_case(crate::directory::FILE_SLOTS, u32::try_from(limits.root.tools.ask_bytes).expect("call bound"), 1, u32::try_from(limits.root.tools.ask_bytes).expect("call bound")).expect("file envelope")
            + Queue::<Complete>::worst_case(ROOM).expect("completions")
            + Queue::<Submit>::worst_case(ROOM).expect("submissions") * 3
            + Queue::<io::Event>::worst_case(ROOM).expect("io events")
            + Queue::<io::file::Event>::worst_case(16).expect("file events")
            + u64::from(limits.io.output) * 128
            + limits::transport(&limits).expect("native envelope")
            + FileIo::worst_case(4, limits.connections.body_bytes, 1, limits.connections.body_bytes).expect("native file envelope")
            + u64::from(limits.connections.body_bytes) * 3 // Loaded store, credential and routing copies.
            + jig_client::worst_case(&limits.root.domain.client).expect("client envelope") * 3 // Client, restored asks and diagnostic copies.
            + Queue::<root::Request>::worst_case(request_room).expect("root outputs")
            + Queue::<jig_client::Request>::worst_case(request_room).expect("ordered store writes")
            + u64::from(request_room) * u64::from(limits.connections.body_bytes) // Queued store records.
            + limits.root.tools.ask_bytes * 256 // Input and the JSON parser's per-byte token/document scratch.
            + u64::from(limits.root.tools.rows) * (u64::from(temper_client_tools::limits::MOVED_BYTES) + u64::try_from(size_of::<Option<skein_lib::Secret>>()).expect("secret holder"))
            + 2 * (u64::from(temper_client_tools::limits::MOVED_BYTES) + 2) // Key or moved-record file buffer and its bounded read.
            + 8192; // Output lines and DIRECTORY's bounded path.
        let (exporting, move_file) = match &call.parsed {
            Parsed::Call { call, .. }
                if call.tool == temper_client_tools::jig::rows::EXPORT_AT
                    || call.tool == temper_client_tools::jig::rows::IMPORT_AT =>
            {
                let index = usize::from(call.tool == temper_client_tools::jig::rows::EXPORT_AT);
                let Some(Some(temper_client_tools::jig::call::Values::One(
                    temper_client_tools::jig::call::Value::File(file),
                ))) = call.values.get(index)
                else {
                    unreachable!("move file")
                };
                (call.tool == temper_client_tools::jig::rows::EXPORT_AT, Some(file.clone()))
            }
            Parsed::Call { .. } | Parsed::File { .. } => (false, None),
        };
        let mut host = Self {
            exporting,
            move_file,
            file_end: None,
            recording: None,
            directory: Directory::new(
                startup,
                call.directory.unwrap_or_default(),
                class,
                files,
                u32::try_from(limits.root.tools.ask_bytes).expect("call bound"),
            ),
            call: Some(call.parsed),
            class,
            waiting: if class == Class::Wait { Waiting::Condition } else { Waiting::Answer },
            root,
            connections: limits.connections,
            root_env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.root },
            requests: Queue::with_capacity(request_room),
            transport: None,
            #[cfg(test)]
            test_connections: None,
            signing: None,
            recovery: Recovery::new(request_room),
            startup,
            until: None,
            link: jig_client::Link::SignedOut,
            io,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.io },
            output,
            signals,
            stdin,
            input: None,
            value: None,
            preparing: true,
            complete: Queue::with_capacity(ROOM),
            submit: Queue::with_capacity(ROOM),
            raw: Queue::with_capacity(ROOM),
            told: Queue::with_capacity(ROOM),
            status: None,
            closed: false,
            memory,
        };
        host.prepare_start();
        Ok(host)
    }
    fn request(&mut self, request: io::Request) {
        assert!(self.io.takes() && self.raw.room() >= io::MAX_OUT_DOWN.submissions, "host request room");
        io::down(&mut self.io, &self.env, request, &mut self.raw);
    }
    fn fail(&mut self, text: &[u8]) {
        if self.status.is_some() {
            return;
        }
        let mut line = b"temper ctl: ".to_vec();
        line.extend_from_slice(text);
        line.push(b'\n');
        self.output.push(true, line.into_boxed_slice());
        self.finish(1, false);
    }
    fn print(&mut self, printed: temper_client_tools::jig::Printed) {
        self.file_end = Some(printed.file);
        self.output.push(false, printed.stdout);
        if printed.status == 1 && !printed.stderr.is_empty() {
            self.fail(printed.stderr.strip_suffix(b"\n").unwrap_or(&printed.stderr));
        } else {
            self.output.push(true, printed.stderr);
            self.finish(printed.status, false);
        }
    }
    fn finish(&mut self, status: u8, signal: bool) {
        if self.status.is_some() {
            return;
        }
        if self.move_file.is_some() {
            let end = self.file_end.unwrap_or_else(|| {
                if self.exporting { self.root.file_end(1) } else { temper_client_tools::jig::FileEnd::Keep }
            });
            self.directory.end_record(end);
        }
        self.output.push(true, self.root.unanswered());
        self.status = Some(status);
        self.value = None;
        self.until = None;
        if let Some(transport) = &mut self.transport {
            transport.close(self.env.now);
        }
        if let Some(signing) = &mut self.signing {
            signing.close();
        }
        if self.signing.is_none() {
            self.directory.close();
        }
        if signal {
            for request in self.output.close(true) {
                self.request(request);
            }
            self.closed = true;
        }
        for entity in [self.stdin.take(), self.signals.take()].into_iter().flatten() {
            self.request(io::Request::Abort { entity });
        }
    }
    fn root_event(&mut self, event: jig_client::Event) {
        root::step(&mut self.root, &self.root_env, root::Event::Client { event }, &mut self.requests);
    }

    fn local_start(&mut self) {
        if !matches!(self.class, Class::Pending | Class::SignOut) || !self.directory.ready() || self.recovery.loaded {
            return;
        }
        if self.signing.is_none() {
            let engine =
                self.directory.engine.unwrap_or_else(|| skein_io::Addr::Ip("127.0.0.1:0".parse().expect("loopback")));
            self.signing = Some(Signing::held(
                engine,
                self.directory.parent(),
                self.directory.leaf(),
                self.startup,
                &mut self.submit,
                self.connections,
                self.env.limits,
            ));
            return;
        }
        let signing = self.signing.as_mut().expect("local shell");
        let Some(shell) = &mut signing.shell else {
            self.fail(b"DIRECTORY's store cannot be read");
            return;
        };
        if let Some((token, records)) = shell.loaded() {
            self.recovery.loaded = true;
            if self.class == Class::Pending {
                let json = matches!(self.call, Some(Parsed::Call { json: true, .. }));
                let printed = crate::pending::print(records, json);
                self.output.push(false, printed.stdout);
                self.finish(printed.status, false);
            } else if let Some(token) = token {
                self.root_event(jig_client::Event::Start { kept: skein_lib::List::with_capacity(0) });
                self.root_event(jig_client::Event::SignIn { with: jig_client::With::Held(token) });
                self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
            } else {
                self.fail(b"DIRECTORY is not signed in");
            }
        } else if self.recovery.unreadable(signing.opening, shell, self.env.now) {
            self.fail(b"DIRECTORY's store cannot be read");
        }
    }
    fn sign_out_keep(&mut self) {
        if self.class != Class::SignOut || self.call.is_some() {
            return;
        }
        let signing = self.signing.as_mut().expect("sign-out shell");
        let shell = signing.shell.as_mut().expect("loaded shell");
        if signing.credential == Credential::New && self.root.client().link().state == jig_client::Link::SignedOut {
            shell.keep(None);
            signing.credential = Credential::Writing;
        }
        if let Some(kept) = shell.kept() {
            if kept {
                self.finish(0, false);
            } else {
                self.fail(b"the sign-out token could not be kept");
            }
        }
    }
    fn client_events(&mut self) {
        loop {
            let event = if let Some(transport) = &mut self.transport {
                transport.events.pop()
            } else {
                self.signing.as_mut().and_then(|s| s.shell.as_mut()).and_then(|s| s.events().pop())
            };
            let Some(event) = event else { break };
            if self.class == Class::Act && !self.recovery.event(&event) {
                self.fail(b"not kept (DIRECTORY's store not written)");
                return;
            }
            let signed = self.signing.as_mut().is_some_and(|s| {
                if matches!(&event, jig_client::Event::Reply { exchange, .. } | jig_client::Event::Lost { exchange } if Some(*exchange) == s.exchange) {
                    s.exchange = None;
                    true
                } else { false }
            });
            self.root_event(event);
            if signed && self.root.client().link().credential.is_none() {
                use jig_client::NotSignedIn;
                match self.root.client().link().not_signed_in {
                    Some(reason @ (NotSignedIn::NotReady | NotSignedIn::Busy)) => {
                        let signing = self.signing.as_mut().expect("link sign-in");
                        signing.last = Some(reason);
                        signing.again(self.env.now, self.until.expect("sign-in deadline"));
                    }
                    Some(NotSignedIn::Full { sign_ins, earliest, link_until }) => {
                        self.output.push(
                            true,
                            temper_client_domain::sign_in::full_line("temper ctl", sign_ins, earliest, link_until),
                        );
                        self.finish(1, false);
                        return;
                    }
                    None => {
                        self.fail(b"the sign-in link was refused");
                        return;
                    }
                }
            }
        }
    }
    fn client_progress(&mut self) {
        if self.status.is_some() {
            return;
        }
        if self.transport.is_none()
            && matches!(self.class, Class::Read | Class::Wait)
            && let Some(token) = self.directory.token.take()
        {
            let connections = self.connections;
            #[cfg(test)]
            let connections = self.test_connections.unwrap_or(connections);
            self.transport = Some(Memory::new(self.directory.engine.expect("loaded engine"), &connections));
            self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
            self.root_event(jig_client::Event::Start { kept: skein_lib::List::with_capacity(0) });
            self.root_event(jig_client::Event::SignIn { with: jig_client::With::Held(skein_lib::Secret::new(token)) });
        }
        self.signing_start();
        self.recovery_start();
        self.local_start();
        if self.class == Class::Pending {
            return;
        }
        if self.status.is_some() {
            return;
        }
        if self.signing.as_ref().is_some_and(|s| s.with.is_some()) {
            return;
        }
        if self.class == Class::Act && !self.recovery.loaded {
            return;
        }
        if self.transport.is_none() && self.signing.is_none() {
            return;
        }
        self.client_events();
        if self.status.is_some() {
            return;
        }
        if root::next_deadline(&self.root).is_some_and(|at| self.env.now >= at) {
            root::fire(&mut self.root, &self.root_env, &mut self.requests);
        }
        self.signing_retry();
        self.root_requests();
        if self.status.is_some() {
            return;
        }
        if self.class == Class::Act {
            if let Some(shell) = self.signing.as_mut().and_then(|s| s.shell.as_mut()) {
                self.recovery.pump(shell);
            }
            if self.recovery.admits()
                && self.root.replayed()
                && self.recovery.sending == 0
                && self
                    .signing
                    .as_ref()
                    .and_then(|s| s.shell.as_ref())
                    .is_some_and(|shell| !shell.work_pending(self.env.now))
                && let Some(Parsed::Call { call, json }) = self.call.take()
            {
                self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
                root::step(
                    &mut self.root,
                    &self.root_env,
                    root::Event::Call { number: 1, call, json, value: self.value.take() },
                    &mut self.requests,
                );
            }
            if self.recovery.admits()
                && let Some(printed) = self.recovery.printed.take()
            {
                self.print(printed);
            }
        }
        self.sign_out_keep();
        self.signing_keep();
        if self.status.is_some() {
            return;
        }
        self.client_link();
    }
    fn client_link(&mut self) {
        let link = self.root.client().link().state;
        let reading = self.root.change_read_pending();
        if self.waiting == Waiting::Condition && self.call.is_none() {
            if reading && self.until.is_none() {
                self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
            } else if !reading && link == jig_client::Link::Live {
                self.until = None;
            }
        }
        if link != self.link {
            if self.waiting == Waiting::Condition && self.call.is_none() && link == jig_client::Link::Live && !reading {
                self.until = None;
            } else if self.link == jig_client::Link::Live && self.until.is_none() {
                self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
            }
            self.link = link;
        }
        match link {
            jig_client::Link::SignedOut
                if self.class == Class::SignOut
                    || self.signing.as_ref().is_some_and(|s| s.credential == Credential::New) => {}
            jig_client::Link::SignedOut => self.fail(b"signed out"),
            jig_client::Link::OutOfDate => self.fail(b"client is out of date"),
            jig_client::Link::Starting | jig_client::Link::Live | jig_client::Link::Offline => {}
        }
    }
    fn root_requests(&mut self) {
        while let Some(request) = self.requests.pop() {
            match request {
                root::Request::Client { request } => {
                    if let Some(signing) = &mut self.signing
                        && let jig_client::Request::Exchange {
                            exchange,
                            message: jig_client::Message::SignInLink(_),
                            ..
                        } = &request
                    {
                        signing.exchange = Some(*exchange);
                    }
                    if let Some(transport) = &mut self.transport {
                        transport.down(self.env.now, request);
                    } else if let Some(shell) = self.signing.as_mut().and_then(|s| s.shell.as_mut()) {
                        if self.class == Class::Act {
                            self.recovery.down(shell, request);
                        } else {
                            shell.down(request);
                        }
                    }
                }
                root::Request::Ready => {
                    self.recovery.ready = true;
                    if self.class == Class::Act {
                        continue;
                    }
                    if self.class == Class::SignOut {
                        self.call = None;
                        self.signing.as_mut().expect("sign-out shell").credential = Credential::New;
                        self.root_event(jig_client::Event::SignOut);
                        continue;
                    }
                    if let Some(Parsed::Call { call, json }) = self.call.take() {
                        self.until = (self.class == Class::Read).then(|| self.env.now.saturating_add(limits::DEADLINE));
                        root::step(
                            &mut self.root,
                            &self.root_env,
                            root::Event::Call { number: 1, call, json, value: self.value.take() },
                            &mut self.requests,
                        );
                    }
                }
                root::Request::Record { number, record } => {
                    self.recording = Some(number);
                    self.directory.write_record(&record);
                }
                root::Request::Printed { printed, .. } => {
                    self.file_end = Some(printed.file);
                    if self.class == Class::Act {
                        self.recovery.printed = Some(printed);
                        continue;
                    }
                    if self.class == Class::SignIn
                        && let Some(signing) = &mut self.signing
                        && printed.status == 0
                    {
                        signing.printed = Some(printed.stdout);
                        continue;
                    }
                    self.print(printed);
                }
                root::Request::Kept { line } => self.output.push(true, line),
                root::Request::Progress { line } => {
                    // The first-repository act has answered and now owns a bounded
                    // watch. Keep its allowance while the engine link is live.
                    self.waiting = Waiting::Condition;
                    if self.root.client().link().state == jig_client::Link::Live {
                        self.until = None;
                    }
                    self.output.push(false, line);
                }
                root::Request::Fact { .. } => {}
            }
        }
    }
    fn recovery_start(&mut self) {
        if self.class != Class::Act || !self.directory.ready() || self.recovery.loaded {
            return;
        }
        if self.signing.is_none() {
            self.signing = Some(Signing::held(
                self.directory.engine.expect("loaded engine"),
                self.directory.parent(),
                self.directory.leaf(),
                self.startup,
                &mut self.submit,
                self.connections,
                self.env.limits,
            ));
            self.until = Some(self.env.now.saturating_add(limits::DEADLINE));
            return;
        }
        let signing = self.signing.as_mut().expect("act shell");
        let Some(shell) = &mut signing.shell else {
            self.fail(b"DIRECTORY's store cannot be read");
            return;
        };
        if let Some((token, records)) = shell.loaded() {
            let Some(token) = token else {
                self.fail(b"DIRECTORY is not signed in");
                return;
            };
            self.recovery.loaded = true;
            self.root.kept_json(matches!(self.call, Some(Parsed::Call { json: true, .. })));
            let mut kept = skein_lib::List::with_capacity(u32::try_from(records.len()).expect("bounded store"));
            for record in records {
                kept.push(record).expect("loaded record room");
            }
            self.root_event(jig_client::Event::Start { kept });
            self.root_event(jig_client::Event::SignIn { with: jig_client::With::Held(token) });
        } else if self.recovery.unreadable(signing.opening, shell, self.env.now) {
            self.fail(b"DIRECTORY's store cannot be read");
        }
    }
    fn link_progress(&mut self, now: Time, wall: Wall) {
        if let Some(shell) = self.signing.as_mut().and_then(|s| s.shell.as_mut()) {
            shell.iterate(now, wall);
            while self.submit.room() > 0 {
                let Some(submit) = shell.submissions().pop() else { break };
                if matches!(submit.kind, Op::Connect { .. } | Op::Send { .. }) {
                    self.recovery.sending += 1;
                }
                self.submit.push(submit);
            }
        }
        self.client_progress();
        if self.status.is_some() && self.signing.as_ref().is_some_and(Signing::is_empty) {
            self.directory.close();
        }
        if self.status.is_none() && self.until.is_some_and(|at| now >= at) {
            if self.signing.as_ref().is_some_and(|signing| signing.last == Some(jig_client::NotSignedIn::Busy)) {
                self.fail(b"temper is busy; this link stays usable until it expires");
                return;
            }
            let address = std::str::from_utf8(self.directory.address.as_deref().expect("loaded address"))
                .expect("numeric address");
            self.fail(format!("the engine at {address} did not answer within 30 s").as_bytes());
        }
    }
    fn events(&mut self, now: Time) {
        while let Some(event) = self.told.pop() {
            let owner = match &event {
                io::Event::Connecting { owner, .. }
                | io::Event::Connected { owner }
                | io::Event::Stream { owner, .. }
                | io::Event::Closed { owner }
                | io::Event::Failed { owner, .. } => Some(*owner),
                io::Event::Output { .. }
                | io::Event::Listening { .. }
                | io::Event::Accepted { .. }
                | io::Event::Spawned { .. }
                | io::Event::Exited { .. }
                | io::Event::Usage { .. }
                | io::Event::Resized
                | io::Event::Window { .. }
                | io::Event::Shutdown { .. } => None,
            };
            if owner.is_some_and(|owner| self.transport.as_ref().is_some_and(|memory| memory.owns(owner))) {
                self.transport.as_mut().expect("memory socket").event(now, event);
                continue;
            }
            match event {
                io::Event::Shutdown { signal, .. } => self.finish(
                    match signal {
                        ServiceSignal::Interrupt => limits::INTERRUPTED,
                        ServiceSignal::Terminate => limits::TERMINATED,
                    },
                    true,
                ),
                io::Event::Stream { owner, up } => {
                    if Some(owner) == self.stdin && self.input.is_some() {
                        self.input_event(up);
                        continue;
                    }
                    if !self.closed
                        && let Some(request) = self.output.event(owner, &up)
                    {
                        self.request(request);
                    }
                }
                io::Event::Closed { .. } | io::Event::Resized => {}
                io::Event::Failed { .. } => self.fail(b"output failed"),
                io::Event::Output { .. }
                | io::Event::Listening { .. }
                | io::Event::Accepted { .. }
                | io::Event::Connecting { .. }
                | io::Event::Connected { .. }
                | io::Event::Spawned { .. }
                | io::Event::Exited { .. }
                | io::Event::Usage { .. }
                | io::Event::Window { .. } => panic!("host owns only pipes and signals"),
            }
        }
    }
    fn complete(&mut self, mut complete: Complete) {
        if complete.op.raw() & (TAG | FILES) == 0 {
            if matches!(complete.kind, Op::Connect { .. } | Op::Send { .. }) {
                self.recovery.sending -= 1;
            }
            self.signing.as_mut().expect("native completion").complete(complete);
            return;
        }
        let tag = if complete.op.raw() & FILES != 0 { FILES } else { TAG };
        complete.op = Token::new(complete.op.raw() & !tag);
        if let Op::Cancel { target } = &mut complete.kind {
            *target = Token::new(target.raw() & !tag);
        }
        if tag == FILES {
            self.directory.complete(complete);
        } else {
            io::up(&mut self.io, &self.env, complete, &mut self.told, &mut self.raw);
        }
    }
    fn publish(queue: &mut Queue<Submit>, tag: u64, out: &mut Queue<Submit>) {
        while out.room() > 0 {
            let Some(mut submit) = queue.pop() else {
                break;
            };
            assert!(submit.op.raw() & (TAG | FILES) == 0, "host token namespace");
            submit.op = Token::new(submit.op.raw() | tag);
            if let Op::Cancel { target } = &mut submit.kind {
                *target = Token::new(target.raw() | tag);
            }
            out.push(submit);
        }
    }
}
impl Host for OneShot {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        self.root_env.now = now;
        self.root_env.wall = wall;
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
        self.events(now);
        self.prepare_progress();
        if !self.preparing || self.status.is_some() {
            self.directory.progress(now);
        }
        if let Some(result) = self.directory.recorded.take() {
            let number = self.recording.take().expect("record acknowledgment");
            if self.status.is_none() {
                self.root.recorded(&self.root_env, number, result, &mut self.requests);
            }
        }
        if let Some(why) = self.directory.not_removed.take()
            && !self.exporting
        {
            let mut line = b"temper ctl: ".to_vec();
            line.extend_from_slice(self.move_file.as_deref().expect("import file"));
            line.extend_from_slice(b" not removed: ");
            line.extend_from_slice(&why);
            line.extend_from_slice(b"; its record is spent\n");
            self.output.push(true, line.into_boxed_slice());
            self.status = Some(1);
        }
        if let Some(error) = self.directory.error.take() {
            if let Some(Parsed::Call { call, json }) = self.call.take() {
                if arguments::key_file(&call).is_some() {
                    self.key_file_fail(&call, &error);
                } else {
                    self.fail(&error);
                }
                self.call = Some(Parsed::Call { call, json });
            } else {
                self.fail(&error);
            }
        }
        if !self.preparing || self.status.is_some() {
            self.link_progress(now, wall);
        }
        if let Some(transport) = &mut self.transport {
            transport.progress(now);
        }
        while self.io.takes() && self.raw.room() >= io::MAX_OUT_DOWN.submissions {
            let Some(request) = self.transport.as_mut().and_then(Memory::request) else { break };
            self.request(request);
        }
        for request in self.output.requests() {
            self.request(request);
        }
        if self.status.is_some() && self.output.is_empty() && self.directory.is_empty() && !self.closed {
            for request in self.output.close(false) {
                self.request(request);
            }
            self.closed = true;
        }
        Self::publish(&mut self.raw, TAG, &mut self.submit);
        Self::publish(&mut self.directory.submissions, FILES, &mut self.submit);
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
            || !self.submit.is_empty()
            || self.io.is_ready()
            || self.io.is_due(now)
            || self.preparation_pending(now)
            || (!self.preparing || self.status.is_some()) && self.directory.pending(now)
            || self.transport.as_ref().is_some_and(|memory| memory.pending(now))
            || self.signing.as_ref().and_then(|s| s.shell.as_ref()).is_some_and(|s| s.work_pending(now))
            || self.status.is_none()
                && (!self.requests.is_empty()
                    || self.signing.as_ref().and_then(|s| s.retry).is_some_and(|at| now >= at)
                    || self.until.is_some_and(|at| now >= at)
                    || root::next_deadline(&self.root).is_some_and(|at| now >= at))
    }
    fn next_deadline(&self) -> Option<Time> {
        [
            self.io.next_deadline(),
            self.directory.deadline(),
            self.transport.as_ref().and_then(Memory::next_deadline),
            self.signing.as_ref().and_then(|s| s.shell.as_ref()).and_then(Shell::next_deadline),
            self.until,
            self.signing.as_ref().and_then(|s| s.retry),
            self.status.is_none().then(|| root::next_deadline(&self.root)).flatten(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        [
            self.directory.retry(),
            self.until,
            self.signing.as_ref().and_then(|s| s.retry),
            self.status.is_none().then(|| root::next_deadline(&self.root)).flatten(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn is_empty(&self) -> bool {
        self.status.is_some()
            && self.closed
            && self.io.is_empty()
            && self.directory.is_empty()
            && self.transport.as_ref().is_none_or(Memory::is_empty)
            && self.signing.as_ref().is_none_or(Signing::is_empty)
            && self.complete.is_empty()
            && self.raw.is_empty()
            && self.submit.is_empty()
            && self.told.is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.is_empty().then(|| Exit::Code(self.status.expect("terminal status")))
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        io::operations(&self.env.limits).expect("io operations") * 2 + 5
    }
}
