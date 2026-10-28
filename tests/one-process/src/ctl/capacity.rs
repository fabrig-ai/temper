//! Concurrent shipped ctl calls, with the clock and kernel owned by this world.
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Pipe, Signal, Spawn, Submit, Target, Way};
use skein_lib::{Duration, Queue, Time, Token, Wall};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
const TAG: u64 = 0xfffd_0000_0000_0000;
pub(crate) const DEADLINE: Duration = Duration::from_millis(250);
#[derive(Clone, Copy, Debug)]
pub enum Profile {
    Small,
    Init,
    FullMap,
}
#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub streams: u32,
    pub profile: Profile,
    pub json: bool,
    pub export: bool,
}
impl Case {
    pub(crate) fn init(self) -> bool {
        matches!(self.profile, Profile::Init | Profile::FullMap)
    }
    pub(crate) fn exchange_full(self) -> bool {
        matches!(self.profile, Profile::FullMap)
    }
}
#[derive(Default)]
struct Observed {
    live: BTreeSet<usize>,
    finished: bool,
}
thread_local! { static OBSERVED: RefCell<Observed> = RefCell::new(Observed::default()); }
pub(crate) fn reset() {
    OBSERVED.with(|o| *o.borrow_mut() = Observed::default());
}
pub(crate) fn ready(slot: usize) {
    OBSERVED.with(|o| {
        o.borrow_mut().live.insert(slot);
    });
}
pub(crate) fn finished() -> bool {
    OBSERVED.with(|o| o.borrow().finished)
}
#[derive(Default)]
struct Child {
    pidfd: Option<Fd>,
    exit: Option<Exit>,
    pending: u32,
    stdout: Option<Fd>,
    output: Vec<u8>,
    error: Vec<u8>,
}
#[derive(Clone, Copy)]
enum Stage {
    SignIn,
    Watches,
    Refused,
    Release,
    Recovered,
    Drain,
    Closing,
    Done,
}
pub(crate) struct Caller {
    root: Fd,
    link: Option<Box<[u8]>>,
    case: Case,
    stage: Stage,
    children: Vec<Child>,
    owners: BTreeMap<Token, usize>,
    serial: u64,
    started: Option<Time>,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl Caller {
    pub(crate) fn owns(token: Token) -> bool {
        token.raw() & TAG == TAG
    }
    pub(crate) fn new(root: Fd, link: &[u8], case: Case) -> Self {
        reset();
        Self {
            root,
            link: Some(link.into()),
            case,
            stage: Stage::SignIn,
            children: vec![],
            owners: BTreeMap::new(),
            serial: 0,
            started: None,
            completions: Queue::with_capacity(128),
            submissions: Queue::with_capacity(128),
        }
    }
    fn submit(&mut self, child: usize, kind: Op) {
        self.serial += 1;
        let op = Token::new(TAG | self.serial);
        assert!(self.owners.insert(op, child).is_none());
        self.children[child].pending += 1;
        self.submissions.push(Submit { op, kind });
    }
    fn spawn(&mut self, words: &[&[u8]]) {
        let slot = self.children.len();
        self.children.push(Child::default());
        self.submit(
            slot,
            Op::Spawn {
                spawn: Box::new(Spawn {
                    program: super::PROGRAM.into(),
                    args: words.iter().map(|w| (*w).into()).collect(),
                    env: Box::new([format!("capacity-slot={slot}").into_bytes().into()]),
                    root: self.root,
                    dir: Box::new([]),
                    pipes: [(0, Way::In), (1, Way::Out), (2, Way::Out)]
                        .map(|(child, way)| Pipe { child, way, parent: None })
                        .into(),
                }),
            },
        );
    }
    fn read(&mut self, child: usize, fd: Fd) {
        self.submit(child, Op::PipeRead { fd, buf: vec![0; 4096].into() });
    }
    fn write(&mut self, child: usize, fd: Fd, bytes: Box<[u8]>, from: u32) {
        if from as usize == bytes.len() {
            self.submit(child, Op::Close { fd });
        } else {
            self.submit(child, Op::PipeWrite { fd, bytes, from });
        }
    }
    fn complete(&self, child: usize) -> bool {
        self.children[child].exit.is_some() && self.children[child].pending == 0
    }
    fn stop(&mut self, child: usize) {
        let pidfd = self.children[child].pidfd.expect("live watch");
        self.submit(child, Op::Signal { pidfd, signal: Signal::Interrupt, to: Target::Child });
    }
    fn progress(&mut self, now: Time) {
        let count = self.case.streams as usize;
        match self.stage {
            Stage::SignIn if self.children.is_empty() => self.spawn(&[b"/client/private", b"sign-in", b"-"]),
            Stage::SignIn if self.complete(0) => {
                assert_eq!(self.children[0].exit, Some(Exit::Code(0)));
                self.stage = Stage::Watches;
            }
            Stage::Watches => {
                let opened = self.children.len() - 1;
                if self.case.exchange_full() && opened == count && self.complete(count) {
                    let refused = &self.children[count];
                    assert_eq!(refused.exit, Some(Exit::Code(2)), "{}", String::from_utf8_lossy(&refused.error));
                    assert!(refused.output.is_empty());
                    assert!(now < self.started.expect("last watch start").saturating_add(Duration::from_secs(1)));
                    assert_eq!(refused.error, b"the engine's 8 client exchanges are in use (clients.exchanges): end a waiting ctl call, or wait\n");
                    for child in 1..count {
                        self.stop(child);
                    }
                    self.stage = Stage::Drain;
                    return;
                }
                let ready = OBSERVED.with(|o| o.borrow().live.range(1..=count).count());
                if opened == ready {
                    if opened < count {
                        if self.case.exchange_full() && opened + 1 == count {
                            self.started = Some(now);
                        }
                        self.spawn(&[b"/client/private", b"inbox", b"--until", b"waiting", b"--timeout", b"90"]);
                    } else {
                        self.started = Some(now);
                        self.stage = Stage::Refused;
                        if self.case.export {
                            self.spawn(&[b"/client/private", b"account", b"export", b"1", b"/setup/return"]);
                        } else if self.case.json {
                            self.spawn(&[b"/client/private", b"me", b"--json"]);
                        } else {
                            self.spawn(&[b"/client/private", b"me"]);
                        }
                    }
                }
            }
            Stage::Refused if self.complete(count + 1) => {
                for watch in &self.children[1..=count] {
                    assert_eq!(watch.exit, None, "all watches held during refusal");
                }
                let refused = &self.children[count + 1];
                assert_eq!(refused.exit, Some(Exit::Code(2)), "{}", String::from_utf8_lossy(&refused.error));
                assert!(refused.output.is_empty());
                let expected = format!(
                    "the engine's {} event streams are in use (clients_http.streams): end a waiting ctl call, or wait",
                    self.case.streams
                );
                if self.case.json {
                    let value: serde_json::Value = serde_json::from_slice(&refused.error).expect("JSON refusal");
                    assert_eq!(value["error"], expected);
                } else {
                    assert_eq!(refused.error, format!("{expected}\n").as_bytes());
                }
                assert!(
                    now < self.started.expect("refused call start").saturating_add(Duration::from_secs(1)),
                    "bounded short deadline"
                );
                self.stop(count);
                self.stage = Stage::Release;
            }
            Stage::Release if self.complete(count) => {
                assert_eq!(self.children[count].exit, Some(Exit::Code(130)));
                self.spawn(&[b"/client/private", b"me"]);
                self.stage = Stage::Recovered;
            }
            Stage::Recovered if self.complete(count + 2) => {
                let recovered = &self.children[count + 2];
                assert_eq!(recovered.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&recovered.error));
                assert!(!recovered.output.is_empty());
                for child in 1..count {
                    self.stop(child);
                }
                self.stage = Stage::Drain;
            }
            Stage::Drain if self.children.iter().all(|c| c.exit.is_some() && c.pending == 0) => {
                self.submit(0, Op::Close { fd: self.root });
                self.stage = Stage::Closing;
            }
            Stage::Closing if self.children.iter().all(|c| c.pending == 0) => {
                self.stage = Stage::Done;
                OBSERVED.with(|o| o.borrow_mut().finished = true);
            }
            Stage::SignIn
            | Stage::Refused
            | Stage::Release
            | Stage::Recovered
            | Stage::Drain
            | Stage::Closing
            | Stage::Done => {}
        }
    }
}
impl Caller {
    pub(crate) fn iterate(&mut self, now: Time, _wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            let child = self.owners.remove(&complete.op).expect("owned operation");
            self.children[child].pending -= 1;
            match (complete.kind, complete.result.expect("capacity operation")) {
                (Op::Spawn { spawn }, Done::Spawned { pidfd, .. }) => {
                    self.children[child].pidfd = Some(pidfd);
                    self.submit(child, Op::Wait { pidfd, reap: false });
                    for pipe in spawn.pipes {
                        let fd = pipe.parent.expect("parent pipe");
                        match pipe.child {
                            0 => {
                                let bytes =
                                    if child == 0 { self.link.take().unwrap_or_default() } else { Box::default() };
                                self.write(child, fd, bytes, 0);
                            }
                            1 => {
                                self.children[child].stdout = Some(fd);
                                self.read(child, fd);
                            }
                            2 => self.read(child, fd),
                            _ => panic!("standard pipe"),
                        }
                    }
                }
                (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => self.write(child, fd, bytes, from + count),
                (Op::PipeRead { fd, buf }, Done::Count(count)) => {
                    if count == 0 {
                        self.submit(child, Op::Close { fd });
                    } else {
                        let c = &mut self.children[child];
                        let bytes = if c.stdout == Some(fd) { &mut c.output } else { &mut c.error };
                        assert!(bytes.len() + count as usize <= super::OUTPUT);
                        bytes.extend_from_slice(&buf[..count as usize]);
                        self.read(child, fd);
                    }
                }
                (Op::Wait { pidfd, reap: false }, Done::Exit(_)) => self.submit(child, Op::Wait { pidfd, reap: true }),
                (Op::Wait { pidfd, reap: true }, Done::Exit(exit)) => {
                    self.children[child].exit = Some(exit);
                    self.children[child].pidfd = None;
                    self.submit(child, Op::Close { fd: pidfd });
                }
                (Op::Close { .. } | Op::Signal { .. }, Done::Nothing) => {}
                (op, done) => panic!("capacity completion {op:?}: {done:?}"),
            }
        }
        self.progress(now);
    }
    pub(crate) fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    pub(crate) fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    pub(crate) fn work_pending(&self, _now: Time) -> bool {
        !self.completions.is_empty()
            || matches!(self.stage, Stage::SignIn) && (self.children.is_empty() || self.complete(0))
            || matches!(self.stage, Stage::Watches)
                && OBSERVED.with(|o| o.borrow().live.range(1..=self.case.streams as usize).count())
                    == self.children.len() - 1
            || matches!(self.stage, Stage::Drain) && self.children.iter().all(|c| c.exit.is_some() && c.pending == 0)
    }
    pub(crate) fn is_empty(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }
}
