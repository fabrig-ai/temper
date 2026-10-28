//! A git process reads and changes the fake machine through kernel operations.
use crate::answer::path;
use crate::{Answer, Checked, Refusal, Transport, answer, check};
use skein_fake_checkout::{Checkout, Kind};
use skein_io::kernel::{self as k, Complete, Done, Exit, Fd, Op, OpenHow, Submit};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use temper_worker_protocol_git::invocation::Command;

#[derive(Debug)]
enum Job {
    Scan(Vec<u8>),
    List(Vec<u8>, Fd),
    ReadOpen(Vec<u8>),
    Read(Vec<u8>, Fd, Vec<u8>),
    Input(Fd),
    Make(Vec<u8>),
    Remove(Vec<u8>, bool),
    WriteOpen(Vec<u8>, Vec<u8>, bool),
    Truncate(Fd, Vec<u8>),
    Write(Fd, Box<[u8]>, u32),
    Pipe(Fd, Box<[u8]>, u32),
    Close(Fd),
    Socket(Op),
}
#[derive(Debug)]
enum Pending {
    Socket,
    Scan(Vec<u8>),
    List(Vec<u8>, Fd),
    ReadOpen(Vec<u8>),
    Read(Vec<u8>, Fd, Vec<u8>),
    Input(Fd),
    Parent(Box<Job>),
    ParentAction(Fd),
    WriteOpen(Vec<u8>),
    Truncate(Fd, Vec<u8>),
    Write(Fd),
    Pipe(Fd),
    Close(Fd),
}
/// Descriptors lent to one invocation; the program closes all of them.
#[derive(Debug)]
pub struct Start {
    /// The root beneath which working-tree paths are resolved.
    pub root: Fd,
    /// Child pipe numbers paired with the program's descriptors.
    pub pipes: Box<[(u32, Fd)]>,
    /// Every descriptor the program owns, including roots, appends and signals.
    pub owned: Box<[Fd]>,
}
/// One hosted invocation; every descriptor closes before its terminal exit.
#[derive(Debug)]
pub struct Git<T: Transport> {
    checked: Option<Checked>,
    checkout: Checkout,
    transport: T,
    connection: Option<crate::connection::Connection>,
    exchange: Option<crate::exchange::Exchange>,
    runs: u32,
    root: Fd,
    pipes: BTreeMap<u32, Fd>,
    owned: BTreeSet<Fd>,
    before: BTreeMap<Vec<u8>, k::Kind>,
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    jobs: VecDeque<Job>,
    pending: Option<Pending>,
    answer: Option<Answer>,
    closing: bool,
    finished: bool,
    serial: u64,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl<T: Transport> Git<T> {
    #[must_use]
    pub fn new(spawn: &k::Spawn, start: Start, transport: T) -> Self {
        let mut broken = None;
        let checked = match check(spawn) {
            Ok(value) => Some(value),
            Err(Refusal::Usage) => None,
            Err(Refusal::Broken(rule)) => {
                broken = Some(Answer {
                    exit: 125,
                    stdout: Vec::new(),
                    stderr: format!("temper fake git: broken: {rule}\n").into_bytes(),
                });
                None
            }
        };
        let root = start.root;
        let pipes: BTreeMap<_, _> = start.pipes.into_vec().into_iter().collect();
        let owned = start.owned.into_vec().into_iter().collect();
        let mut jobs = VecDeque::new();
        if let Some(checked) = &checked {
            if matches!(checked.command, Command::CommitTree { .. }) {
                jobs.push_back(Job::Input(*pipes.get(&0).expect("commit stdin")));
            }
            match &checked.command {
                Command::Version => {}
                Command::Clone { directory, .. } => jobs.push_back(Job::Scan(path(&checked.at, directory))),
                Command::Fetch { .. }
                | Command::Create { .. }
                | Command::CheckOut { .. }
                | Command::Clean
                | Command::Merge { .. }
                | Command::Unmerged
                | Command::Add
                | Command::DiffIndex { .. }
                | Command::WriteTree
                | Command::CommitTree { .. }
                | Command::IsAncestor { .. }
                | Command::Push { .. } => jobs.push_back(Job::Scan(checked.at.to_vec())),
            }
        }
        let mut checkout = Checkout::new();
        checkout.mkdir(b"");
        let connection = transport.connection();
        Self {
            connection,
            exchange: None,
            runs: 0,
            checked,
            checkout,
            transport,
            root,
            pipes,
            owned,
            before: BTreeMap::new(),
            files: BTreeMap::new(),
            jobs,
            pending: None,
            answer: broken,
            closing: false,
            finished: false,
            serial: 0,
            completions: Queue::with_capacity(1),
            submissions: Queue::with_capacity(1),
        }
    }
    fn emit(&mut self, op: Op, pending: Pending) {
        self.serial += 1;
        self.pending = Some(pending);
        self.submissions.push(Submit { op: Token::new(self.serial), kind: op });
    }
    fn next(&mut self, job: Job) {
        match job {
            Job::Socket(op) => self.emit(op, Pending::Socket),
            Job::Scan(path) => self.emit(
                Op::Open { root: self.root, path: path.clone().into(), how: OpenHow::DirectoryNoFollow },
                Pending::Scan(path),
            ),
            Job::List(path, fd) => self.emit(
                Op::List { fd, entries: vec![k::Entry::BLANK; 8].into(), names: vec![0; 2040].into() },
                Pending::List(path, fd),
            ),
            Job::ReadOpen(path) => self.emit(
                Op::Open { root: self.root, path: path.clone().into(), how: OpenHow::ReadNoFollow },
                Pending::ReadOpen(path),
            ),
            Job::Read(path, fd, bytes) => self.emit(
                Op::Read {
                    fd,
                    buf: vec![0; 4096].into(),
                    at: u64::try_from(bytes.len()).expect("fixture file length"),
                },
                Pending::Read(path, fd, bytes),
            ),
            Job::Input(fd) => self.emit(Op::PipeRead { fd, buf: vec![0; 4096].into() }, Pending::Input(fd)),
            job @ (Job::Make(_) | Job::Remove(_, _)) => {
                let (Job::Make(name) | Job::Remove(name, _)) = &job else {
                    panic!("parent operation");
                };
                let parent = name.rsplitn(2, |b| *b == b'/').nth(1).unwrap_or(b".");
                self.emit(
                    Op::Open { root: self.root, path: parent.into(), how: OpenHow::DirectoryNoFollow },
                    Pending::Parent(Box::new(job)),
                );
            }
            Job::WriteOpen(path, bytes, exists) => {
                self.files.insert(path.clone(), bytes);
                self.emit(
                    Op::Open {
                        root: self.root,
                        path: path.clone().into(),
                        how: if exists {
                            OpenHow::WriteNoFollow
                        } else {
                            OpenHow::CreateNoFollow { mode: Some(0o600) }
                        },
                    },
                    Pending::WriteOpen(path),
                );
            }
            Job::Truncate(fd, bytes) => self.emit(Op::Truncate { fd, length: 0 }, Pending::Truncate(fd, bytes)),
            Job::Write(fd, bytes, from) => {
                self.emit(Op::Write { fd, bytes, from, at: u64::from(from) }, Pending::Write(fd));
            }
            Job::Pipe(fd, bytes, from) => self.emit(Op::PipeWrite { fd, bytes, from }, Pending::Pipe(fd)),
            Job::Close(fd) => self.emit(Op::Close { fd }, Pending::Close(fd)),
        }
    }
    fn complete(&mut self, complete: Complete) {
        let pending = self.pending.take().expect("one requested kernel operation");
        if matches!(pending, Pending::Socket) {
            self.connection.as_mut().expect("socket job's connection").complete(complete.kind, complete.result);
        } else {
            self.file_complete(pending, complete.kind, complete.result);
        }
    }
    fn file_complete(&mut self, pending: Pending, kind: Op, result: Result<Done, k::Error>) {
        match (pending, kind, result) {
            (Pending::Scan(path), _, Ok(Done::Fd(fd))) => {
                self.owned.insert(fd);
                self.before.insert(path.clone(), k::Kind::Directory);
                self.checkout.mkdir(&path);
                self.jobs.push_front(Job::List(path, fd));
            }
            (Pending::Scan(_), _, Err(k::Error::NotFound)) => {}
            (Pending::List(path, fd), Op::List { entries, names, .. }, Ok(Done::Count(count))) => {
                if count == 0 {
                    self.jobs.push_front(Job::Close(fd));
                } else {
                    self.jobs.push_front(Job::List(path.clone(), fd));
                    for entry in entries.iter().take(usize::try_from(count).expect("eight entries")) {
                        let full = path_of_entry(&path, *entry, &names);
                        self.before.insert(full.clone(), entry.kind);
                        match entry.kind {
                            k::Kind::Directory => self.jobs.push_back(Job::Scan(full)),
                            k::Kind::File => self.jobs.push_back(Job::ReadOpen(full)),
                            k::Kind::Symlink | k::Kind::Other => self.checkout.special(&full),
                        }
                    }
                }
            }
            (Pending::ReadOpen(path), _, Ok(Done::Fd(fd))) => {
                self.owned.insert(fd);
                self.jobs.push_front(Job::Read(path, fd, Vec::new()));
            }
            (Pending::Read(path, fd, mut bytes), Op::Read { buf, .. }, Ok(Done::Count(count))) => {
                if count == 0 {
                    self.checkout.write(&path, &bytes);
                    self.files.insert(path, bytes);
                    self.jobs.push_front(Job::Close(fd));
                } else {
                    bytes.extend(&buf[..usize::try_from(count).expect("read buffer")]);
                    self.jobs.push_front(Job::Read(path, fd, bytes));
                }
            }
            (Pending::Input(fd), Op::PipeRead { buf, .. }, Ok(Done::Count(count))) => {
                if count != 0 {
                    self.checked
                        .as_mut()
                        .expect("commit invocation")
                        .message
                        .extend(&buf[..usize::try_from(count).expect("read buffer")]);
                    self.jobs.push_front(Job::Input(fd));
                }
            }
            (Pending::Parent(job), _, Ok(Done::Fd(fd))) => {
                self.owned.insert(fd);
                let op = match *job {
                    Job::Make(path) => Op::MakeDirectory { dir: fd, name: basename(&path), mode: 0o700 },
                    Job::Remove(path, directory) => Op::Remove { dir: fd, name: basename(&path), directory },
                    Job::Scan(_)
                    | Job::List(..)
                    | Job::ReadOpen(_)
                    | Job::Read(..)
                    | Job::Input(_)
                    | Job::WriteOpen(..)
                    | Job::Truncate(..)
                    | Job::Write(..)
                    | Job::Pipe(..)
                    | Job::Socket(_)
                    | Job::Close(_) => panic!("parent operation"),
                };
                self.emit(op, Pending::ParentAction(fd));
            }
            (Pending::ParentAction(fd), _, Ok(Done::Nothing) | Err(k::Error::Exists)) => {
                self.jobs.push_front(Job::Close(fd));
            }
            (Pending::WriteOpen(path), kind, Ok(Done::Fd(fd))) => {
                self.write_open(path, fd, matches!(kind, Op::Open { how: OpenHow::CreateNoFollow { .. }, .. }));
            }
            (Pending::Truncate(fd, bytes), _, Ok(Done::Nothing)) => {
                self.jobs.push_front(if bytes.is_empty() { Job::Close(fd) } else { Job::Write(fd, bytes.into(), 0) });
            }
            (Pending::Write(fd), Op::Write { bytes, from, .. }, Ok(Done::Count(count))) => {
                assert!(count > 0, "a file write progresses");
                let from = from + count;
                self.jobs.push_front(if usize::try_from(from).expect("buffer position") == bytes.len() {
                    Job::Close(fd)
                } else {
                    Job::Write(fd, bytes, from)
                });
            }
            (Pending::Pipe(fd), Op::PipeWrite { bytes, from, .. }, Ok(Done::Count(count))) => {
                assert!(count > 0, "a pipe write progresses");
                let from = from + count;
                if usize::try_from(from).expect("buffer position") != bytes.len() {
                    self.jobs.push_front(Job::Pipe(fd, bytes, from));
                }
            }
            (Pending::Close(fd), _, Ok(Done::Nothing)) => {
                self.owned.remove(&fd);
            }
            (_, _, Err(error)) => self.failure(error),
            (_, _, Ok(_)) => panic!("kernel completion has the operation's shape"),
        }
    }
    fn write_open(&mut self, path: Vec<u8>, fd: Fd, created: bool) {
        self.owned.insert(fd);
        let bytes = self.files.remove(&path).expect("pending write bytes");
        if created {
            self.jobs.push_front(Job::WriteOpen(path, bytes, true));
            self.jobs.push_front(Job::Close(fd));
        } else {
            self.jobs.push_front(Job::Truncate(fd, bytes));
        }
    }
    fn failure(&mut self, error: k::Error) {
        self.jobs.clear();
        self.answer = Some(Answer {
            exit: 128,
            stdout: Vec::new(),
            stderr: format!("fatal: cannot access working tree: {error:?}\n").into_bytes(),
        });
        if self.closing {
            self.close_owned();
        }
    }
    fn prepare(&mut self) {
        let answer = if let Some(answer) = self.answer.take() {
            answer
        } else if self.checked.is_some() {
            if self.runs >= crate::MAX_RUNS {
                crate::exchange::broken("runs")
            } else {
                self.runs += 1;
                self.transport.begin();
                let result =
                    answer(self.checked.as_ref().expect("checked invocation"), &mut self.checkout, &mut self.transport);
                match self.transport.missed() {
                    crate::Missing::None => result,
                    crate::Missing::Call(call) => {
                        self.restore();
                        self.request(call);
                        return;
                    }
                    crate::Missing::Unsendable => {
                        self.restore();
                        crate::exchange::broken("request")
                    }
                }
            }
        } else {
            Answer::usage()
        };
        if self.checked.is_some() && answer.exit <= 1 {
            self.changes();
        }
        if !answer.stdout.is_empty() {
            self.jobs.push_back(Job::Pipe(*self.pipes.get(&1).expect("stdout"), answer.stdout.clone().into(), 0));
        }
        if !answer.stderr.is_empty() {
            self.jobs.push_back(Job::Pipe(*self.pipes.get(&2).expect("stderr"), answer.stderr.clone().into(), 0));
        }
        self.answer = Some(answer);
        self.closing = true;
    }
    fn restore(&mut self) {
        let mut checkout = Checkout::new();
        checkout.mkdir(b"");
        for (path, kind) in &self.before {
            match kind {
                k::Kind::Directory => checkout.mkdir(path),
                k::Kind::File => {
                    checkout.write(path, self.files.get(path).expect("scanned file"));
                }
                k::Kind::Symlink | k::Kind::Other => checkout.special(path),
            }
        }
        self.checkout = checkout;
    }
    fn request(&mut self, call: skein_fake_forge_protocol::git::Call) {
        let checked = self.checked.as_ref().expect("admitted route invocation");
        let remote = crate::check::remote(&checked.command)
            .map(<[u8]>::to_vec)
            .or_else(|| self.files.get(&path(&checked.at, b".git/temper-remote")).cloned())
            .unwrap_or_default();
        let result = crate::exchange::Exchange::new(call, remote).and_then(|exchange| {
            let limits = self.transport.route_limits().expect("missing call's route limits");
            let (call, body) = exchange.request(checked.authorization.as_ref(), &limits)?;
            self.connection.as_mut().expect("missing call's connection").call(exchange.address, call, body);
            Ok(exchange)
        });
        match result {
            Ok(exchange) => self.exchange = Some(exchange),
            Err(answer) => self.answer = Some(answer),
        }
    }
    fn received(&mut self, reply: Result<crate::connection::Reply, crate::connection::Failed>) {
        let exchange = self.exchange.take().expect("one missing call in flight");
        let limits = self.transport.route_limits().expect("route limits");
        match exchange.finish(reply, &limits) {
            Ok((call, answer, authenticated)) => self.transport.receive(call, answer, authenticated),
            Err(answer) => self.answer = Some(answer),
        }
    }
    fn changes(&mut self) {
        let mut after = nodes(&mut self.checkout);
        for (path, kind) in &self.before {
            if after.get(path) == Some(&k::Kind::Other) && *kind == k::Kind::Symlink {
                after.insert(path.clone(), *kind);
            }
        }
        let mut remove: Vec<_> = self
            .before
            .iter()
            .filter(|(path, kind)| after.get(*path) != Some(*kind))
            .map(|(path, kind)| (path.clone(), *kind == k::Kind::Directory))
            .collect();
        remove.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
        for (path, directory) in remove {
            self.jobs.push_back(Job::Remove(path, directory));
        }
        for (path, kind) in &after {
            if *kind == k::Kind::Directory && self.before.get(path) != Some(&k::Kind::Directory) && !path.is_empty() {
                self.jobs.push_back(Job::Make(path.clone()));
            }
        }
        for (path, bytes) in self.checkout.files() {
            if self.files.get(path).map(Vec::as_slice) != Some(bytes) {
                self.jobs.push_back(Job::WriteOpen(
                    path.to_vec(),
                    bytes.to_vec(),
                    self.before.get(path) == Some(&k::Kind::File),
                ));
            }
        }
    }
    fn close_owned(&mut self) {
        for fd in &self.owned {
            self.jobs.push_back(Job::Close(*fd));
        }
    }
}
impl<T: Transport> Host for Git<T> {
    fn iterate(&mut self, now: Time, _wall: Wall) {
        self.transport.set_time(now);
        while let Some(complete) = self.completions.pop() {
            self.complete(complete);
        }
        if self.pending.is_some() {
            return;
        }
        if let Some(connection) = &mut self.connection {
            if self.closing {
                connection.close();
            }
            let op = connection.next_job();
            let result = connection.take();
            if let Some(op) = op {
                self.next(Job::Socket(op));
                return;
            }
            if let Some(result) = result {
                self.received(result);
            }
            if self.exchange.is_some() {
                return;
            }
        }
        if self.jobs.is_empty() && !self.closing {
            self.prepare();
        }
        if self.jobs.is_empty() && self.closing {
            if self.owned.is_empty() {
                self.finished = true;
            } else {
                self.close_owned();
            }
        }
        if let Some(job) = self.jobs.pop_front() {
            self.next(job);
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: Time) -> bool {
        self.pending.is_none() && !self.finished
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.finished && self.completions.is_empty() && self.submissions.is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.is_empty().then(|| Exit::Code(self.answer.as_ref().expect("answered invocation").exit))
    }
    fn operations(&self) -> u32 {
        1
    }
}
fn path_of_entry(at: &[u8], entry: k::Entry, names: &[u8]) -> Vec<u8> {
    path(at, entry.name(names).expect("listed name"))
}
fn basename(path: &[u8]) -> Box<[u8]> {
    path.rsplit(|b| *b == b'/').next().expect("path component").into()
}
fn nodes(checkout: &mut Checkout) -> BTreeMap<Vec<u8>, k::Kind> {
    let root = checkout.root(b"");
    let mut out = BTreeMap::new();
    let mut dirs = vec![Vec::new()];
    while let Some(at) = dirs.pop() {
        out.insert(at.clone(), k::Kind::Directory);
        for (name, kind) in checkout.scan(root, &at, usize::MAX).expect("model directory").0 {
            let path = path(&at, &name);
            let kind = match kind {
                Kind::File => k::Kind::File,
                Kind::Directory => k::Kind::Directory,
                Kind::Link | Kind::Special => k::Kind::Other,
            };
            if kind == k::Kind::Directory {
                dirs.push(path.clone());
            }
            out.insert(path, kind);
        }
    }
    out
}
