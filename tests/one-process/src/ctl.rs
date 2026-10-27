//! The shipped ctl runs setup on the same simulated kernel as the native person.
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Pipe, Spawn, Submit, Way};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use skein_world::{Inherited, StartupRoot};
use temper_client_shell_cli::{
    arguments,
    host::{Invocation, OneShot, Startup},
};
pub const PROGRAM: &[u8] = b"/world/temper-ctl";
const OUTPUT: usize = 65_536;
const TAG: u64 = 0xfffe_0000_0000_0000;

pub fn roots(spawn: &Spawn) -> Vec<StartupRoot> {
    let parsed =
        arguments::parse(&spawn.args[1..].iter().map(AsRef::as_ref).collect::<Vec<_>>()).expect("setup arguments");
    let mut roots = vec![StartupRoot { name: b"parent".as_slice().into(), path: b"client".as_slice().into() }];
    if let Some(path) = arguments::file_root(&parsed) {
        roots.push(StartupRoot { name: b"file_parent".as_slice().into(), path });
    }
    for (number, path) in arguments::text_files(&parsed) {
        roots.push(StartupRoot { name: format!("text-{number}").into_bytes().into(), path });
    }
    roots
}
pub fn host(spawn: &Spawn, inherited: &Inherited) -> Recorded {
    let parsed =
        arguments::parse(&spawn.args[1..].iter().map(AsRef::as_ref).collect::<Vec<_>>()).expect("setup arguments");
    let pipe = |number| inherited.pipes.iter().find(|(child, _)| *child == number).expect("standard pipe").1;
    let files = inherited
        .roots
        .iter()
        .filter_map(|(name, fd)| {
            name.strip_prefix(b"text-").map(|number| {
                (std::str::from_utf8(number).expect("text argument").parse().expect("text argument"), *fd)
            })
        })
        .collect();
    let host = OneShot::with_text_files(
        Invocation { directory: Some(b"private".as_slice().into()), parsed },
        Startup {
            parent: Some(inherited.roots[0].1),
            file_parent: inherited.roots.iter().find(|(name, _)| name.as_ref() == b"file_parent").map(|(_, fd)| *fd),
            stdin: pipe(0),
            stdout: pipe(1),
            stderr: pipe(2),
            signals: inherited.signal,
            user: 1000,
            seed: crate::fixtures::SEED,
        },
        files,
    )
    .expect("shipped ctl limits");
    Recorded { host, wire: crate::referee::client::Wire::default(), seen: 0 }
}

pub(crate) struct Caller {
    root: Fd,
    link: Option<Box<[u8]>>,
    at: u8,
    started: bool,
    pidfd: Option<Fd>,
    pending: u32,
    serial: u64,
    stdout: Option<Fd>,
    exit: Option<Exit>,
    output: Vec<u8>,
    error: Vec<u8>,
    finished: bool,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl Caller {
    pub(crate) fn owns(token: Token) -> bool {
        token.raw() & TAG == TAG
    }
    pub(crate) fn new(root: Fd, link: &[u8]) -> Self {
        Self {
            root,
            link: Some(link.into()),
            at: 0,
            started: false,
            pidfd: None,
            pending: 0,
            serial: 0,
            stdout: None,
            exit: None,
            output: Vec::with_capacity(OUTPUT),
            error: Vec::with_capacity(OUTPUT),
            finished: false,
            completions: Queue::with_capacity(32),
            submissions: Queue::with_capacity(32),
        }
    }
    fn submit(&mut self, kind: Op) {
        self.serial += 1;
        self.pending += 1;
        self.submissions.push(Submit { op: Token::new(TAG | self.serial), kind });
    }
    fn read(&mut self, fd: Fd) {
        self.submit(Op::PipeRead { fd, buf: vec![0; 4096].into_boxed_slice() });
    }
    fn write(&mut self, fd: Fd, bytes: Box<[u8]>, from: u32) {
        if from as usize == bytes.len() {
            self.submit(Op::Close { fd });
        } else {
            self.submit(Op::PipeWrite { fd, bytes, from });
        }
    }
}
impl Host for Caller {
    fn iterate(&mut self, _now: Time, _wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            self.pending -= 1;
            match (complete.kind, complete.result.expect("ctl setup operation")) {
                (Op::Spawn { spawn }, Done::Spawned { pidfd, .. }) => {
                    self.pidfd = Some(pidfd);
                    for pipe in spawn.pipes {
                        let fd = pipe.parent.expect("parent pipe");
                        match pipe.child {
                            0 => {
                                let bytes = self.link.take().unwrap_or_default();
                                self.write(fd, bytes, 0);
                            }
                            1 => {
                                self.stdout = Some(fd);
                                self.read(fd);
                            }
                            2 => self.read(fd),
                            _ => panic!("standard setup pipe"),
                        }
                    }
                }
                (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => self.write(fd, bytes, from + count),
                (Op::PipeRead { fd, buf }, Done::Count(count)) => {
                    if count == 0 {
                        self.submit(Op::Close { fd });
                    } else {
                        let bytes = if self.stdout == Some(fd) { &mut self.output } else { &mut self.error };
                        assert!(bytes.len() + count as usize <= OUTPUT, "bounded ctl setup output");
                        bytes.extend_from_slice(&buf[..count as usize]);
                        self.read(fd);
                    }
                }
                (Op::Wait { pidfd, reap: false }, Done::Exit(_)) => self.submit(Op::Wait { pidfd, reap: true }),
                (Op::Wait { pidfd, reap: true }, Done::Exit(exit)) => {
                    self.exit = Some(exit);
                    self.submit(Op::Close { fd: pidfd });
                }
                (Op::Close { .. }, Done::Nothing) => {}
                (op, done) => panic!("setup completion {op:?}: {done:?}"),
            }
        }
        if self.pending != 0 || self.finished {
            return;
        }
        if let Some(pidfd) = self.pidfd.take() {
            self.submit(Op::Wait { pidfd, reap: false });
            return;
        }
        if self.started {
            assert_eq!(
                self.exit,
                Some(Exit::Code(0)),
                "ctl setup {} refused: {} {}",
                self.at,
                String::from_utf8_lossy(&self.output),
                String::from_utf8_lossy(&self.error)
            );
            self.at += 1;
            self.started = false;
            self.exit = None;
            self.output.clear();
            self.error.clear();
        }
        if self.at >= 2 {
            self.finished = true;
            return;
        }
        let words: &[&[u8]] = if self.at == 0 {
            &[b"/client/private", b"sign-in", b"-"]
        } else {
            &[
                b"/client/private",
                b"settings",
                b"apply",
                b"/setup/settings.json",
                b"--against",
                b"1",
                b"--key",
                b"world-settings",
            ]
        };
        self.started = true;
        self.submit(Op::Spawn {
            spawn: Box::new(Spawn {
                program: PROGRAM.into(),
                args: words.iter().map(|word| (*word).into()).collect(),
                env: Box::new([]),
                root: self.root,
                dir: Box::new([]),
                pipes: [(0, Way::In), (1, Way::Out), (2, Way::Out)]
                    .map(|(child, way)| Pipe { child, way, parent: None })
                    .into(),
            }),
        });
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: Time) -> bool {
        !self.completions.is_empty() || (!self.finished && self.pending == 0)
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.finished && self.submissions.is_empty() && self.completions.is_empty()
    }
    fn operations(&self) -> u32 {
        32
    }
    fn worst_case(&self) -> u64 {
        262_144
    }
}

/// Observe actual sign-in bytes before starting the native person on its saved token.
pub(crate) struct Recorded {
    host: OneShot,
    wire: crate::referee::client::Wire,
    seen: usize,
}
impl Host for Recorded {
    fn iterate(&mut self, now: Time, wall: Wall) {
        for complete in self.host.completions().iter() {
            self.wire.complete(complete);
        }
        for event in &self.wire.received[self.seen..] {
            if let crate::referee::client::Received::SignedIn(party) = event {
                crate::world::setup_signed(*party);
            }
        }
        self.seen = self.wire.received.len();
        self.host.iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host.is_empty()
    }
    fn operations(&self) -> u32 {
        self.host.operations()
    }
    fn worst_case(&self) -> u64 {
        self.host.worst_case() + 1_048_576
    }
    fn exit(&self) -> Option<Exit> {
        self.host.exit()
    }
}
