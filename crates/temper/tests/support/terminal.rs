//! A typist's commands and both binaries on the process tier's real ring.
use super::WITHIN;
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Signal, Submit};
use skein_lib::{Duration, Queue, Time, Token, Wall};
use skein_shell::{Clock, Host};
use skein_world::{
    Referee,
    end_to_end::{Binary, Command, Mode, Streams},
    real,
};
use std::{collections::VecDeque, path::Path};
const READ: Token = Token::new(1);
const WRITE: Token = Token::new(2);
const CLOSE: Token = Token::new(3);

/// The first printed link, with the service's scripted pipes still owned.
pub struct Link {
    bytes: Vec<u8>,
    streams: Streams,
}
impl AsRef<[u8]> for Link {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
pub fn link(serve: &mut Binary, within: Duration) -> Link {
    let streams = serve.take_streams().expect("serve pipes");
    let Streams::Pipes { output, .. } = streams else { panic!("serve uses pipes") };
    Link { bytes: super::read_line(output, within), streams }
}
/// The same piped invocation is also hosted by ctl stories.
pub(super) fn open_arguments(private: &Path, link: &[u8]) -> Vec<Box<[u8]>> {
    vec![b"open".as_slice().into(), private.as_os_str().as_bytes().into(), link.into()]
}
#[must_use]
pub fn open(directory: &Path, private: &Path, link: &[u8]) -> Binary {
    Binary::start(
        Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: open_arguments(private, link)
                .iter()
                .map(|word| std::ffi::OsStr::from_bytes(word).into())
                .collect(),
            directory: directory.into(),
            environment: Vec::new(),
        },
        Mode::Pipes,
        65_536,
    )
    .expect("temper open starts")
}
use std::os::unix::ffi::OsStrExt;
enum Action {
    Wait(Vec<u8>),
    Type(Vec<u8>),
    On { control: Vec<u8>, words: Vec<u8> },
}
fn typed_control(bytes: &[u8], control: &[u8], words: &[u8]) -> Option<Vec<u8>> {
    for line in bytes.split(|byte| *byte == b'\n').rev().skip(1) {
        let line = line.trim_ascii_start();
        let Some(line) = line.strip_prefix(b"[") else { continue };
        let Some(end) = line.iter().position(|byte| *byte == b']') else { continue };
        // A link's outward mark follows its name for show; the person types the name (5.7).
        let label = line.get(end + 2..).map(|label| label.strip_suffix(" ↗".as_bytes()).unwrap_or(label));
        if label != Some(control) || !line[..end].iter().all(u8::is_ascii_digit) {
            continue;
        }
        let mut typed = line[..end].to_vec();
        if !words.is_empty() {
            typed.push(b' ');
            typed.extend_from_slice(words);
        }
        typed.push(b'\n');
        return Some(typed);
    }
    None
}
struct Pipes {
    input: Option<Fd>,
    output: Option<Fd>,
    reading: bool,
    writing: bool,
    closing: u32,
    bytes: Vec<u8>,
    actions: VecDeque<Action>,
    typed: Option<(Vec<u8>, usize)>,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
}
impl Pipes {
    fn new(streams: &Streams, actions: VecDeque<Action>) -> Self {
        let &Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
        Self {
            input: Some(input),
            output: Some(output),
            reading: false,
            writing: false,
            closing: 0,
            bytes: Vec::new(),
            actions,
            typed: None,
            complete: Queue::with_capacity(8),
            submit: Queue::with_capacity(8),
        }
    }
    fn close(&mut self, fd: Fd) {
        self.closing += 1;
        self.submit.push(Submit { op: CLOSE, kind: Op::Close { fd } });
    }
}
impl Host for Pipes {
    fn iterate(&mut self, _now: Time, _wall: Wall) {
        while let Some(complete) = self.complete.pop() {
            if complete.op == READ {
                self.reading = false;
                let Op::PipeRead { buf, .. } = complete.kind else { panic!("stdout read") };
                let Done::Count(count) = complete.result.expect("stdout read succeeds") else { panic!("read count") };
                self.bytes.extend_from_slice(&buf[..usize::try_from(count).expect("read count fits")]);
                assert!(self.bytes.len() <= 262_144, "bounded terminal output");
                if count == 0 {
                    let output = self.output.take().expect("stdout pipe");
                    self.close(output);
                }
            } else if complete.op == WRITE {
                self.writing = false;
                let Done::Count(count) = complete.result.expect("stdin write succeeds") else { panic!("write count") };
                let (bytes, written) = self.typed.as_mut().expect("typed line");
                *written += usize::try_from(count).expect("write count fits");
                if *written == bytes.len() {
                    self.typed = None;
                }
            } else {
                assert_eq!(complete.op, CLOSE, "pipe close token");
                complete.result.expect("pipe closed");
                self.closing -= 1;
            }
        }
        if self.typed.is_none() {
            match self.actions.front() {
                Some(Action::Wait(words)) if skein_lib::bytes::find(&self.bytes, words).is_some() => {
                    self.actions.pop_front();
                }
                Some(Action::Type(_)) => {
                    let Some(Action::Type(bytes)) = self.actions.pop_front() else { panic!("type action") };
                    self.typed = Some((bytes, 0));
                }
                Some(Action::On { control, words }) => {
                    if let Some(bytes) = typed_control(&self.bytes, control, words) {
                        self.actions.pop_front();
                        self.typed = Some((bytes, 0));
                    }
                }
                Some(Action::Wait(_)) | None => {}
            }
        }
        if let Some(Action::Wait(words)) = self.actions.front()
            && words.starts_with(b"Adopted ")
            && let Some(reason) = adoption_refusal(&self.bytes)
        {
            panic!("terminal adoption refused: {reason}");
        }
        if !self.writing
            && let Some((bytes, written)) = &self.typed
        {
            self.writing = true;
            self.submit.push(Submit {
                op: WRITE,
                kind: Op::PipeWrite { fd: self.input.expect("stdin pipe"), bytes: bytes[*written..].into(), from: 0 },
            });
        }
        if self.actions.is_empty()
            && self.typed.is_none()
            && let Some(input) = self.input.take()
        {
            self.close(input);
        }
        if !self.reading
            && let Some(fd) = self.output
        {
            self.reading = true;
            self.submit.push(Submit { op: READ, kind: Op::PipeRead { fd, buf: vec![0; 4096].into_boxed_slice() } });
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submit
    }
    fn work_pending(&self, _now: Time) -> bool {
        !self.complete.is_empty()
            || !self.submit.is_empty()
            || (!self.reading && self.output.is_some())
            || (!self.writing && self.typed.is_some())
            || self.actions.front().is_some_and(|action| match action {
                Action::Wait(words) => skein_lib::bytes::find(&self.bytes, words).is_some(),
                Action::Type(_) => true,
                Action::On { control, words } => typed_control(&self.bytes, control, words).is_some(),
            })
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.closing == 0
            && self.actions.is_empty()
            && self.typed.is_none()
    }
    fn operations(&self) -> u32 {
        4
    }
}
fn adoption_refusal(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let frame = text.rsplit("\n== ").next().expect("at least one frame");
    frame.lines().find_map(|line| line.trim_start().strip_prefix("! ").map(str::to_owned))
}

#[test]
fn a_terminal_adoption_refusal_uses_the_current_frame() {
    let refused = include_bytes!("../../../temper-client-shell-terminal/src/tests/golden/adopt_refused_not_found.txt");
    assert_eq!(
        adoption_refusal(refused).as_deref(),
        Some("No repository acme/shop on forge 1 that temper's forge user can see.")
    );
    let mut frames = refused.to_vec();
    frames.extend_from_slice(b"\n== Adopt a repository == Live\nAdopted acme/shop on forge 1 as owned.\n");
    assert_eq!(adoption_refusal(&frames), None, "an old alert does not decide the next frame");
}
enum Process {
    Binary(Box<Binary>),
    Pipes(Box<Pipes>),
}
impl Process {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Binary(x) => x.as_ref(),
            Self::Pipes(x) => x.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Binary(x) => x.as_mut(),
            Self::Pipes(x) => x.as_mut(),
        }
    }
}
impl Host for Process {
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
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
struct Judge {
    stopped: bool,
    deadline: Time,
    settled: bool,
}
impl Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        if !self.stopped && procs[2].is_empty() && procs[3].is_empty() {
            let Process::Binary(serve) = &mut procs[0] else { panic!("service binary") };
            serve.signal(Signal::Terminate);
            self.stopped = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.settled = self.stopped && procs.iter().all(Host::is_empty);
        let Process::Binary(serve) = &procs[0] else { panic!("service binary") };
        assert!(
            self.stopped || serve.exit_status().is_none(),
            "service ended before the terminal: {}",
            String::from_utf8_lossy(serve.stderr())
        );
        let Process::Binary(open) = &procs[2] else { panic!("terminal binary") };
        let Process::Pipes(typist) = &procs[3] else { panic!("typist") };
        assert!(
            now < self.deadline || self.settled,
            "terminal deadline; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&typist.bytes),
            String::from_utf8_lossy(open.stderr())
        );
        assert!(
            open.exit_status().is_none() || typist.actions.is_empty(),
            "terminal exited before the script; stderr: {}",
            String::from_utf8_lossy(open.stderr())
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "terminal and service did not drain".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
/// The typed script is run by `exit`, with both children and pipes hosted together.
pub struct Typist {
    open: Option<Binary>,
    serve: Option<Binary>,
    link: Option<Link>,
    actions: VecDeque<Action>,
    stderr: Vec<u8>,
}
impl Typist {
    #[must_use]
    pub fn new(open: Binary, serve: Binary, link: Link) -> Self {
        Self { open: Some(open), serve: Some(serve), link: Some(link), actions: VecDeque::new(), stderr: Vec::new() }
    }
    pub fn wait_for(&mut self, words: &[u8], within: Duration) {
        assert_eq!(within, WITHIN, "the script's shared deadline");
        self.actions.push_back(Action::Wait(words.to_vec()));
    }
    pub fn type_line(&mut self, words: &[u8]) {
        let mut bytes = words.to_vec();
        bytes.push(b'\n');
        self.actions.push_back(Action::Type(bytes));
    }
    /// Type the number printed beside the last matching control, then its words.
    pub fn type_on(&mut self, control: &[u8], words: &[u8]) {
        self.actions.push_back(Action::On { control: control.to_vec(), words: words.to_vec() });
    }
    #[must_use]
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
    pub fn exit(&mut self, within: Duration) -> u8 {
        let clock = Clock::new();
        let mut world = real::World::new(Judge {
            stopped: false,
            settled: false,
            deadline: clock.now().now.saturating_add(within),
        });
        let serve = self.serve.take().expect("service binary");
        let link = self.link.take().expect("service streams");
        world.spawn_with_fds(serve.descriptors(), || Process::Binary(Box::new(serve)));
        world.spawn_with_fds(link.streams.descriptors(), || {
            Process::Pipes(Box::new(Pipes::new(&link.streams, VecDeque::new())))
        });
        let mut open = self.open.take().expect("terminal binary");
        let streams = open.take_streams().expect("terminal pipes");
        world.spawn_with_fds(open.descriptors(), || Process::Binary(Box::new(open)));
        world.spawn_with_fds(streams.descriptors(), || {
            Process::Pipes(Box::new(Pipes::new(&streams, std::mem::take(&mut self.actions))))
        });
        let outcome = world.run(&clock, within.saturating_add(Duration::from_secs(10)));
        let Process::Binary(open) = &outcome.procs[2] else { panic!("terminal binary") };
        self.stderr = open.stderr().to_vec();
        let Process::Binary(serve) = &outcome.procs[0] else { panic!("service binary") };
        assert_eq!(
            serve.exit_status(),
            Some(Exit::Code(0)),
            "service drained: {}",
            String::from_utf8_lossy(serve.stderr())
        );
        match open.exit_status().expect("terminal exited") {
            Exit::Code(code) => code,
            Exit::Signal(_) => 1,
        }
    }
}
