//! One shared terminal on one real ring (protocol/setup.md, 7.2 and 13.1).
//! The synthetic children expose write completions; Service is used unchanged.
#![cfg(test)]
#![forbid(unsafe_code)]
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "the real-world harness is ordinary Rust (programming-model.md, 10.2), with explicitly bounded vectors"
)]

use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Pipe, ServiceSignal, Spawn, Submit, Way};
use skein_lib::{Duration, Queue, Time, Token, Wall};
use skein_shell::{Clock, Host, HostedPipes, close_keeper_fd, hosted_pipes, open_root};
use skein_world::{
    HostedProgram, Inherited, Referee,
    real::{Controls, World},
};
use temper_box::host::{Service, Settings};
use temper_forge_setup::{Kind, Step, Steps};

// The only output is two small blocks, two diagnostics and lifecycle notices.
const CAPTURE_BYTES: usize = 4096;
const READ_BYTES: usize = 128;
// At most four unused inherited pipes close alongside a signal read and write.
const OPERATIONS: u32 = 8;
const QUEUE_ENTRIES: u32 = OPERATIONS;
// The Service is admitted first, before the capture root and hosted children.
const BOX_HOST: usize = 0;
const PATIENCE: Duration = Duration::from_secs(2);
const PROGRAM: &[u8] = b"synthetic-box-child";
const FIRST: &[u8] = b"http://127.0.0.1:8367/jig/v1/sign-in/link?secret=test-first\n";
const FRESH: &[u8] = b"http://127.0.0.1:8367/jig/v1/sign-in/link?secret=test-fresh\n";
const PRELUDE: &[u8] = b"\nOpen this link in a browser on this machine. It signs you in as this\ntemper's owner, once and for a short while, so keep it private.\nOpened again once spent, it prints a fresh one here.\n\n";
const FOOTER: &[u8] = b"\nCtrl-C stops temper.\n\n";
const ANNOUNCEMENT: &[u8] = b"A fresh sign-in link:\n";
const FIRST_DIAGNOSTIC: &[u8] = b"diagnostic: between first pieces\n";
const FRESH_DIAGNOSTIC: &[u8] = b"diagnostic: between fresh pieces\n";
const SERVE_WRITES: [&[u8]; 4] = [
    b"http://127.0.0.1:8367/jig/v1/sign-in/link?secret=",
    b"test-first\n",
    b"http://127.0.0.1:8367/jig/v1/sign-in/link?secret=",
    b"test-fresh\n",
];
const DIAGNOSTIC_WRITES: [&[u8]; 4] = [b"between first ", b"pieces\n", b"between fresh ", b"pieces\n"];

#[derive(Debug)]
enum Proc {
    Box(Box<Service>),
    Serve(Synthetic),
    Diagnostic(Synthetic),
    Capture(Synthetic),
}

impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Box(host) => host.as_ref(),
            Self::Serve(host) | Self::Diagnostic(host) | Self::Capture(host) => host,
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Box(host) => host.as_mut(),
            Self::Serve(host) | Self::Diagnostic(host) | Self::Capture(host) => host,
        }
    }
}

// This wrapper only delegates the public Host interface, including policy time.
impl Host for Proc {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.host().exit()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}

/// A small raw-operation host, driven by the world's scheduler like Service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Eof,
    Signal,
}

#[derive(Debug)]
struct Synthetic {
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    owned: Vec<Fd>,
    writer: Option<Fd>,
    chunks: &'static [&'static [u8]],
    released: usize,
    written: usize,
    writing: bool,
    received: Vec<u8>,
    end: Option<End>,
    pending: u32,
    serial: u64,
}

impl Synthetic {
    fn new(owned: Vec<Fd>) -> Self {
        Self {
            completions: Queue::with_capacity(QUEUE_ENTRIES),
            submissions: Queue::with_capacity(QUEUE_ENTRIES),
            owned,
            writer: None,
            chunks: &[],
            released: 0,
            written: 0,
            writing: false,
            received: Vec::with_capacity(CAPTURE_BYTES),
            end: None,
            pending: 0,
            serial: 0,
        }
    }
    fn submit(&mut self, kind: Op) {
        self.serial = self.serial.checked_add(1).expect("fixed story operations");
        self.pending = self.pending.checked_add(1).expect("bounded pending operations");
        assert!(self.pending <= OPERATIONS, "synthetic operation bound");
        self.submissions.push(Submit { op: Token::new(self.serial), kind });
    }
    fn read(&mut self, fd: Fd) {
        self.submit(Op::PipeRead { fd, buf: vec![0; READ_BYTES].into_boxed_slice() });
    }
    fn close_all(&mut self, end: End) {
        self.end = Some(end);
        while let Some(fd) = self.owned.pop() {
            self.submit(Op::Close { fd });
        }
    }
    fn capture(fd: Fd) -> Self {
        let mut host = Self::new(vec![fd]);
        host.read(fd);
        host
    }
    fn child(inherited: &Inherited, number: u32, chunks: &'static [&'static [u8]]) -> Self {
        let writer = descriptor(&inherited.pipes, number);
        let mut host = Self::new(vec![writer, inherited.signal]);
        host.writer = Some(writer);
        host.chunks = chunks;
        // The other stream, stdin and serve's control pair have no role here.
        // Close them through the ring, rather than retaining EOF keepers.
        for (_, fd) in &inherited.pipes {
            if *fd != writer {
                host.submit(Op::Close { fd: *fd });
            }
        }
        assert!(inherited.roots.is_empty() && inherited.appends.is_empty(), "no synthetic startup files");
        host.submit(Op::ReadSignal { fd: inherited.signal });
        host
    }
    fn complete(&mut self, complete: Complete) {
        self.pending = self.pending.checked_sub(1).expect("one completion per operation");
        let done = complete.result.expect("synthetic kernel operation succeeds");
        match (complete.kind, done) {
            (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => {
                assert!(count > 0, "a write makes progress");
                let from = from.checked_add(count).expect("small fixed write");
                if usize::try_from(from).expect("write offset") == bytes.len() {
                    self.written = self.written.checked_add(1).expect("four separate writes");
                    self.writing = false;
                } else {
                    self.submit(Op::PipeWrite { fd, bytes, from });
                }
            }
            (Op::PipeRead { fd, buf }, Done::Count(count)) => {
                if count == 0 {
                    self.close_all(End::Eof);
                } else {
                    let count = usize::try_from(count).expect("fixed read count");
                    let length = self.received.len().checked_add(count).expect("bounded capture length");
                    assert!(length <= CAPTURE_BYTES, "capture is at most 4096 bytes");
                    self.received.extend_from_slice(buf.get(..count).expect("kernel read bound"));
                    self.read(fd);
                }
            }
            (Op::ReadSignal { .. }, Done::Signalled(_)) => {
                assert!(!self.writing, "shutdown follows every separate write completion");
                self.close_all(End::Signal);
            }
            (Op::Close { .. }, Done::Nothing) => {}
            _ => unreachable!("only fixed synthetic operations are submitted"),
        }
    }
}

impl Host for Synthetic {
    fn iterate(&mut self, _now: Time, _wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            self.complete(complete);
        }
        if self.end.is_none() && !self.writing && self.written < self.released {
            let bytes = Box::from(*self.chunks.get(self.written).expect("released script chunk"));
            self.writing = true;
            self.submit(Op::PipeWrite { fd: self.writer.expect("child writer"), bytes, from: 0 });
        }
    }
    fn drain(&mut self) {}
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: Time) -> bool {
        !self.completions.is_empty()
            || !self.submissions.is_empty()
            || (self.end.is_none() && !self.writing && self.written < self.released)
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.end.is_some()
            && self.owned.is_empty()
            && self.pending == 0
            && self.completions.is_empty()
            && self.submissions.is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        if self.is_empty() { Some(Exit::Code(0)) } else { None }
    }
    fn worst_case(&self) -> u64 {
        // Unchecked world; nevertheless count queues and their fixed buffers.
        let queues = Queue::<Complete>::worst_case(QUEUE_ENTRIES)
            .expect("completion storage")
            .checked_add(Queue::<Submit>::worst_case(QUEUE_ENTRIES).expect("submission storage"))
            .expect("queue storage");
        // Capture plus at most one fixed read/write buffer per operation and
        // eight descriptor slots. The remaining host fields live inline.
        let buffers = CAPTURE_BYTES
            .checked_add(
                READ_BYTES.checked_mul(usize::try_from(OPERATIONS).expect("operation count")).expect("fixed buffers"),
            )
            .expect("capture and buffers");
        let descriptors = size_of::<Fd>()
            .checked_mul(usize::try_from(OPERATIONS).expect("descriptor count"))
            .expect("descriptor storage");
        queues
            .checked_add(
                u64::try_from(buffers.checked_add(descriptors).expect("synthetic storage")).expect("storage fits u64"),
            )
            .expect("host storage")
    }
    fn operations(&self) -> u32 {
        OPERATIONS
    }
}

fn descriptor(pipes: &[(u32, Fd)], number: u32) -> Fd {
    for (child, fd) in pipes {
        if *child == number {
            return *fd;
        }
    }
    unreachable!("requested inherited descriptor exists")
}

fn child(spawn: &Spawn, inherited: &Inherited) -> Proc {
    match spawn.args.first().expect("synthetic role").as_ref() {
        b"serve" => Proc::Serve(Synthetic::child(inherited, 1, &SERVE_WRITES)),
        b"diagnostic" => Proc::Diagnostic(Synthetic::child(inherited, 2, &DIAGNOSTIC_WRITES)),
        _ => unreachable!("only the story's two children are hosted"),
    }
}

#[derive(Debug)]
struct Judge {
    controls: Controls,
    deadline: Time,
    wake: Option<Time>,
    stage: u8,
    settled: bool,
}

fn capture(procs: &[Proc]) -> &Synthetic {
    for proc in procs {
        if let Proc::Capture(host) = proc {
            return host;
        }
    }
    unreachable!("one capture root")
}
fn writes(procs: &[Proc], serve: bool) -> usize {
    for proc in procs {
        match proc {
            Proc::Serve(host) if serve => return host.written,
            Proc::Diagnostic(host) if !serve => return host.written,
            Proc::Box(_) | Proc::Serve(_) | Proc::Diagnostic(_) | Proc::Capture(_) => {}
        }
    }
    0
}
fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    position(bytes, needle).is_some()
}
fn position(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    for (at, piece) in bytes.windows(needle.len()).enumerate() {
        if piece == needle {
            return Some(at);
        }
    }
    None
}
fn occurrences(bytes: &[u8], needle: &[u8]) -> usize {
    let mut count: usize = 0;
    for piece in bytes.windows(needle.len()) {
        if piece == needle {
            count = count.checked_add(1).expect("bounded capture occurrences");
        }
    }
    count
}

impl Judge {
    fn ready(&self, procs: &[Proc]) -> bool {
        let bytes = &capture(procs).received;
        match self.stage {
            0 => writes(procs, true) == 1,
            1 => writes(procs, false) == 2 && contains(bytes, FIRST_DIAGNOSTIC),
            // Suffix cues let even a broken layout drain before its assertion.
            2 => writes(procs, true) == 2 && contains(bytes, b"test-first\n"),
            3 => writes(procs, true) == 3,
            4 => writes(procs, false) == 4 && contains(bytes, FRESH_DIAGNOSTIC),
            5 => writes(procs, true) == 4 && contains(bytes, b"test-fresh\n"),
            6 => {
                for proc in procs {
                    if !proc.is_empty() {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }
}
impl Referee<Proc> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Proc]) {
        self.wake = None;
        // Only the first prefix is initially runnable, regardless of spawn order.
        for proc in &mut *procs {
            if let Proc::Serve(host) = proc {
                host.released = host.released.max(1);
            }
        }
        if !self.ready(procs) {
            return;
        }
        for proc in &mut *procs {
            match proc {
                Proc::Serve(host) => match self.stage {
                    1 => host.released = 2,
                    2 => host.released = 3,
                    4 => host.released = 4,
                    _ => {}
                },
                Proc::Diagnostic(host) => match self.stage {
                    0 => host.released = 2,
                    3 => host.released = 4,
                    _ => {}
                },
                Proc::Box(_) | Proc::Capture(_) => {}
            }
        }
        if self.stage == 5 {
            self.controls.signal(BOX_HOST, ServiceSignal::Terminate);
        }
        if self.stage == 6 {
            self.settled = true;
        }
        self.stage = self.stage.checked_add(1).expect("seven causal stages");
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        if self.ready(procs) {
            self.wake = Some(now);
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        if self.settled { None } else { Some(self.wake.unwrap_or(self.deadline).min(self.deadline)) }
    }
    fn overdue(&self, now: Time) -> Option<String> {
        if now >= self.deadline && !self.settled {
            Some(format!("shared-terminal stage {} did not settle", self.stage))
        } else {
            None
        }
    }
    fn passed(&self) -> bool {
        self.settled
    }
}

fn startup(root: Fd, pipes: Box<[Pipe]>) -> Spawn {
    Spawn { program: PROGRAM.into(), args: Box::new([]), env: Box::new([]), root, dir: Box::new([]), pipes }
}
fn close_placeholders(pipes: &HostedPipes) {
    for fd in [pipes.pidfd, pipes.signal, pipes.signal_writer] {
        close_keeper_fd(fd);
    }
}

fn shared_terminal(root: Fd) -> (Fd, Fd, Fd) {
    let mut pipe = startup(root, Box::new([Pipe { child: 1, way: Way::Out, parent: None }]));
    let opened = hosted_pipes(&mut pipe).expect("shared output pipe");
    let reader = pipe.pipes.first().expect("one output pipe").parent.expect("capture reader");
    let writer = descriptor(&opened.pipes, 1);
    close_placeholders(&opened);
    let mut terminal = startup(
        root,
        Box::new([
            Pipe { child: 1, way: Way::Given(Token::new(1)), parent: Some(writer) },
            Pipe { child: 2, way: Way::Given(Token::new(2)), parent: Some(writer) },
        ]),
    );
    let writers = hosted_pipes(&mut terminal).expect("independently owned terminal writers");
    close_placeholders(&writers);
    close_keeper_fd(writer);
    let stdout = descriptor(&writers.pipes, 1);
    let stderr = descriptor(&writers.pipes, 2);
    assert_ne!(stdout, stderr, "two owners never share a descriptor number");
    (reader, stdout, stderr)
}

fn timestamp_before(bytes: &[u8], at: usize) {
    let start = at.checked_sub(10).expect("HH:MM:SSZ precedes terminal words");
    let stamp = bytes.get(start..at).expect("ten timestamp bytes");
    assert_eq!(stamp.get(2), Some(&b':'), "UTC hour separator");
    assert_eq!(stamp.get(5), Some(&b':'), "UTC minute separator");
    assert_eq!(stamp.get(8..), Some(b"Z ".as_slice()), "UTC suffix");
    for at in [0, 1, 3, 4, 6, 7] {
        assert!(stamp.get(at).expect("timestamp digit").is_ascii_digit(), "decimal UTC time");
    }
    assert!(
        start == 0 || bytes.get(start.checked_sub(1).expect("nonzero start")) == Some(&b'\n'),
        "timestamp starts its line"
    );
}

#[test]
fn a_split_sign_in_link_stays_whole_on_the_shared_terminal() {
    let scratch = skein_scratch::Scratch::new("box-terminal");
    let root = open_root(scratch.path()).expect("box startup root");
    let (reader, stdout, stderr) = shared_terminal(root);
    let clock = Clock::new();
    let deadline = clock.now().now.saturating_add(PATIENCE);
    let mut world =
        World::new_controlled(|controls| Judge { controls, deadline, wake: None, stage: 0, settled: false });
    world.host(HostedProgram { program: PROGRAM.into(), make: child, instances: 2, operations: OPERATIONS });
    world.spawn_signals(|signals| {
        let steps = Steps {
            always: Box::new([
                Step::new(b"diagnostic".as_slice().into(), Box::new([b"diagnostic".as_slice().into()]), Kind::Long),
                Step::new(b"serve".as_slice().into(), Box::new([b"serve".as_slice().into()]), Kind::Long),
            ]),
            asked: Box::new([]),
        };
        Proc::Box(Box::new(
            Service::new(
                &temper_box::limits::small(),
                steps,
                Settings {
                    echo: temper_box::host::Echo::Steps,
                    data: Box::from(b"/data".as_slice()),
                    program: PROGRAM.into(),
                    environment: Box::new([]),
                    serve: b"serve".as_slice().into(),
                    listeners: Box::new([]),
                    root,
                    signals,
                    stdout,
                    stderr,
                },
            )
            .expect("real Service settings"),
        ))
    });
    world.spawn_with_fds(vec![reader], || Proc::Capture(Synthetic::capture(reader)));
    let outcome = world.run(&clock, PATIENCE);
    assert!(outcome.killed.is_empty(), "children close voluntarily on their signals");
    assert_eq!(outcome.procs.len(), 4, "one Service, one capture and two hosted children");
    for proc in &outcome.procs {
        assert!(proc.is_empty(), "all hosts drain their owned resources");
        assert_eq!(proc.exit(), Some(Exit::Code(0)), "every host exits successfully");
        if let Proc::Serve(host) | Proc::Diagnostic(host) = proc {
            assert_eq!(host.written, 4, "each child completes four separate PipeWrite operations");
            assert_eq!(host.end, Some(End::Signal), "each child closes on its inherited signal");
        }
    }
    let captured = capture(&outcome.procs);
    assert_eq!(captured.end, Some(End::Eof), "capture reaches EOF after both Service writers close");
    let bytes = &captured.received;
    let text = String::from_utf8_lossy(bytes);
    let first = position(bytes, FIRST);
    assert!(first.is_some(), "first URL must occupy one intact line; capture:\n{text}");
    let first = first.expect("intact first URL");
    let prelude = first.checked_sub(PRELUDE.len()).expect("first URL follows prelude");
    assert_eq!(bytes.get(prelude..first), Some(PRELUDE), "exact first prelude immediately precedes URL");
    let first_end = first.checked_add(FIRST.len()).expect("first line end");
    let footer_end = first_end.checked_add(FOOTER.len()).expect("footer end");
    assert_eq!(bytes.get(first_end..footer_end), Some(FOOTER), "exact footer immediately follows URL");
    let fresh = position(bytes, FRESH).expect("fresh URL occupies one intact line");
    let announcement = fresh.checked_sub(ANNOUNCEMENT.len()).expect("fresh URL follows announcement");
    assert_eq!(bytes.get(announcement..fresh), Some(ANNOUNCEMENT), "fresh announcement immediately precedes URL");
    timestamp_before(bytes, announcement);
    assert_eq!(occurrences(bytes, FIRST), 1, "exactly one intact first URL line");
    assert_eq!(occurrences(bytes, FRESH), 1, "exactly one intact fresh URL line");
    assert_eq!(occurrences(bytes, SERVE_WRITES.first().expect("URL prefix")), 2, "no stray or duplicate URL prefix");
    assert!(
        position(bytes, FIRST_DIAGNOSTIC).expect("first diagnostic") < prelude,
        "first diagnostic precedes whole first block"
    );
    let diagnostic = position(bytes, FRESH_DIAGNOSTIC).expect("fresh diagnostic");
    assert!(footer_end <= diagnostic && diagnostic < announcement, "fresh diagnostic is between the blocks");
    let stopping = position(bytes, b"Stopping\n").expect("Stopping lifecycle notice");
    let stopped = position(bytes, b"Stopped\n").expect("Stopped lifecycle notice");
    assert!(fresh < stopping && stopping < stopped, "fresh URL precedes ordered shutdown notices");
    timestamp_before(bytes, stopping);
    timestamp_before(bytes, stopped);
}
