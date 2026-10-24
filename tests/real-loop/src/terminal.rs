//! The person's terminal client in the loop, and the typist at it (domain/testing.md, 2.4).
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Pipe, Spawn, Submit, Way};
use skein_lib::{Duration, Queue, Token};
use skein_shell::Host;
use std::{os::unix::ffi::OsStrExt, path::Path};
use temper_client_shell_terminal::{
    client::{NativeClient, Settings, Start},
    limits,
    terminal::{Loop, Terminal},
};

/// The terminal's program, hosted once in a world, as `temper open`.
pub const OPEN: &[u8] = b"/real-loop/temper-open";
/// The tier's print interval, as short as its engine's paces.
pub const PRINT_INTERVAL: Duration = Duration::from_millis(20);
const SEED: u64 = 0x7465_726d_696e_616c;
const TAG: u64 = 1 << 60;
const PAGE_BYTES: usize = 4096;
// A story's transcript is bounded independently of its pipes' read buffers.
const TRANSCRIPT_BYTES: usize = 1_048_576;

/// A person's act at the terminal, each one a story's step.
pub enum Act {
    /// Open on its DIRECTORY by the link the service printed last.
    Open,
    /// Type the last page's numbered control with this label, then the words.
    TypeOn { label: &'static [u8], words: &'static [u8] },
    /// Read a page after the last command that holds `holds` and lacks `lacks`.
    See { holds: &'static [u8], lacks: Option<&'static [u8]> },
    /// Type `q` and wait for the terminal's exit.
    Close,
}
#[must_use]
pub fn open() -> crate::calls::Step {
    crate::calls::Step::Terminal(Act::Open)
}
#[must_use]
pub fn type_on(label: &'static [u8], words: &'static [u8]) -> crate::calls::Step {
    crate::calls::Step::Terminal(Act::TypeOn { label, words })
}
#[must_use]
pub fn see(holds: &'static [u8]) -> crate::calls::Step {
    crate::calls::Step::Terminal(Act::See { holds, lacks: None })
}
#[must_use]
pub fn see_without(holds: &'static [u8], lacks: &'static [u8]) -> crate::calls::Step {
    crate::calls::Step::Terminal(Act::See { holds, lacks: Some(lacks) })
}
#[must_use]
pub fn close() -> crate::calls::Step {
    crate::calls::Step::Terminal(Act::Close)
}

/// What the typist read, retained for a story's checks.
#[derive(Default)]
pub struct Read {
    /// Every page printed, whole, in order.
    pub pages: Vec<Page>,
    /// temper's own lines (`temper: ...`), in order.
    pub lines: Vec<Box<[u8]>>,
    /// The last open's exit, once it ended.
    pub exit: Option<Exit>,
    /// How many times the harness opened it (two when the first link was spent).
    pub opens: u32,
}
pub struct Page {
    pub bytes: Box<[u8]>,
    /// The typist's commands typed before this page was read.
    pub commands: u32,
}
/// The terminal's DIRECTORY, its own rather than any tools client's.
#[must_use]
pub fn directory(scratch: &Path) -> std::path::PathBuf {
    scratch.join("clients").join("terminal")
}
pub(crate) fn roots(spawn: &Spawn) -> Vec<skein_world::StartupRoot> {
    let directory = Path::new(std::ffi::OsStr::from_bytes(&spawn.args[0]));
    vec![skein_world::StartupRoot {
        name: b"parent".as_slice().into(),
        path: directory.parent().expect("terminal parent").as_os_str().as_bytes().into(),
    }]
}
pub(crate) fn host(spawn: &Spawn, inherited: &skein_world::Inherited) -> crate::world::Proc {
    let directory = Path::new(std::ffi::OsStr::from_bytes(&spawn.args[0]));
    let target = temper::open::target(&spawn.args[1]).expect("printed terminal link");
    let pipe = |child| inherited.pipes.iter().find(|(number, _)| *number == child).expect("terminal pipe").1;
    crate::world::Proc::Terminal(Box::new(compose(
        directory,
        target,
        inherited.roots[0].1,
        pipe(0),
        pipe(1),
        inherited.signal,
    )))
}
fn compose(directory: &Path, target: temper::open::Target, parent: Fd, stdin: Fd, stdout: Fd, signals: Fd) -> Terminal {
    let (engine, link, start) = match target {
        temper::open::Target::Link { engine, link } => {
            let start = Start::Link(link.expose().into());
            (engine, Some(link), start)
        }
        temper::open::Target::Address { engine } => (engine, None, Start::Token),
    };
    let bounds = limits::native();
    let (view, _) = limits::view(&bounds).expect("native view bounds");
    let client = NativeClient::new(
        Settings {
            engine: skein_io::Addr::Ip(engine),
            directory: directory.file_name().expect("terminal directory leaf").as_bytes().into(),
            limits: bounds,
        },
        parent,
        skein_shell::effective_user(),
        SEED,
        start,
    )
    .expect("native terminal client");
    let settings = limits::Terminal { interval: PRINT_INTERVAL, ..limits::terminal() };
    let state = Loop::new(
        &settings,
        &view.tree,
        directory.as_os_str().as_bytes().into(),
        link,
        bounds.client.client.domain.requests.requests,
    );
    Terminal::new(client, state, stdin, stdout, signals).expect("terminal descriptors")
}
/// Admission uses the composition's own operation count. Construction is sans io:
/// these placeholder descriptors are never driven or submitted to the kernel.
pub(crate) fn operations() -> u32 {
    compose(
        Path::new("terminal"),
        temper::open::Target::Address { engine: "127.0.0.1:1".parse().expect("operation-count address") },
        Fd::new(3),
        Fd::new(0),
        Fd::new(1),
        Fd::new(2),
    )
    .operations()
}

#[derive(Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent pipe, command and terminal lifecycles")]
pub(crate) struct Typist {
    pub(crate) read: Read,
    pub(crate) refused: std::cell::Cell<bool>,
    stdin: Option<Fd>,
    active: bool,
    pending: u32,
    serial: u64,
    writing: bool,
    commands: u32,
    cursor: usize,
    opening: bool,
    acted: bool,
    attempts: u8,
    link_count: u32,
    partial: Vec<u8>,
    page: Vec<u8>,
    bytes: usize,
    unclosed: bool,
}
impl Typist {
    pub(crate) fn owns(token: Token) -> bool {
        token.raw() & TAG != 0
    }
    pub(crate) fn worst_case() -> u64 {
        // Transcript payload, per-line/page ownership and allocator capacity growth,
        // the partial page/line, retained link and at most one read/write in flight.
        u64::try_from(TRANSCRIPT_BYTES * (2 + std::mem::size_of::<Page>()) + PAGE_BYTES * 8)
            .expect("typist transcript memory")
    }
    fn submit(&mut self, kind: Op, out: &mut Queue<Submit>) {
        self.serial += 1;
        self.pending += 1;
        out.push(Submit { op: Token::new(TAG | self.serial), kind });
    }
    fn read_pipe(&mut self, fd: Fd, out: &mut Queue<Submit>) {
        self.submit(Op::PipeRead { fd, buf: vec![0; PAGE_BYTES].into_boxed_slice() }, out);
    }
    fn launch(&mut self, scratch: &Path, root: Fd, links: u32, out: &mut Queue<Submit>) {
        let link = std::fs::read(scratch.join("link")).expect("last printed link").trim_ascii().to_vec();
        self.link_count = links;
        self.read.opens += 1;
        self.attempts += 1;
        self.read.exit = None;
        self.refused.set(false);
        self.active = true;
        self.submit(
            Op::Spawn {
                spawn: Box::new(Spawn {
                    program: OPEN.into(),
                    args: vec![directory(scratch).as_os_str().as_bytes().into(), link.into()].into(),
                    env: Box::new([]),
                    root,
                    dir: Box::new([]),
                    pipes: [(0, Way::In), (1, Way::Out)].map(|(child, way)| Pipe { child, way, parent: None }).into(),
                }),
            },
            out,
        );
    }
    pub(crate) fn complete(&mut self, complete: Complete, out: &mut Queue<Submit>) {
        self.pending -= 1;
        match (complete.kind, complete.result.expect("typist operation")) {
            (Op::Spawn { spawn }, Done::Spawned { pidfd, .. }) => {
                for pipe in spawn.pipes {
                    let fd = pipe.parent.expect("terminal parent pipe");
                    match pipe.child {
                        0 => self.stdin = Some(fd),
                        1 => self.read_pipe(fd, out),
                        _ => panic!("terminal stdin and stdout"),
                    }
                }
                self.submit(Op::Wait { pidfd, reap: false }, out);
            }
            (Op::PipeRead { fd, buf }, Done::Count(count)) => {
                if count == 0 {
                    assert!(self.partial.is_empty(), "terminal ended within a line");
                    self.finish_page();
                    self.submit(Op::Close { fd }, out);
                } else {
                    self.printed(&buf[..usize::try_from(count).expect("terminal read size")]);
                    self.read_pipe(fd, out);
                }
            }
            (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => {
                assert_ne!(count, 0, "terminal command write made progress");
                self.write(fd, bytes, from + count, out);
            }
            (Op::Wait { pidfd, reap: false }, Done::Exit(_)) => self.submit(Op::Wait { pidfd, reap: true }, out),
            (Op::Wait { pidfd, reap: true }, Done::Exit(exit)) => {
                self.read.exit = Some(exit);
                self.submit(Op::Close { fd: pidfd }, out);
                if let Some(fd) = self.stdin.take() {
                    self.submit(Op::Close { fd }, out);
                }
            }
            (Op::Close { .. }, Done::Nothing) => {}
            (kind, done) => panic!("typist completion {kind:?}: {done:?}"),
        }
        if self.pending == 0 {
            assert!(self.read.exit.is_some(), "terminal ended before typist settled");
            self.active = false;
        }
    }
    fn finish_page(&mut self) {
        if !self.page.is_empty() {
            assert!(
                self.page.len() < PAGE_BYTES,
                "terminal page exceeds pipe's atomic write bound: {}",
                String::from_utf8_lossy(&self.page)
            );
            self.read.pages.push(Page { bytes: std::mem::take(&mut self.page).into(), commands: self.commands });
        }
    }
    fn printed(&mut self, bytes: &[u8]) {
        self.bytes += bytes.len();
        assert!(self.bytes <= TRANSCRIPT_BYTES, "terminal transcript bound");
        self.partial.extend_from_slice(bytes);
        let mut from = 0;
        while let Some(end) = self.partial[from..].iter().position(|byte| *byte == b'\n') {
            let end = from + end + 1;
            let starts_page = self.partial[from..end].starts_with(b"== ");
            let notice = self.partial[from..end].starts_with(b"temper: ");
            if starts_page || notice {
                self.finish_page();
            }
            if notice {
                self.read.lines.push(self.partial[from..end].into());
            } else {
                assert!(!self.page.is_empty() || starts_page, "page starts with ==");
                self.page.extend_from_slice(&self.partial[from..end]);
            }
            from = end;
        }
        // Compact once per read, rather than shifting the whole page for every line.
        self.partial.drain(..from);
        if self.partial.is_empty() {
            self.finish_page();
        }
    }
    fn write(&mut self, fd: Fd, bytes: Box<[u8]>, from: u32, out: &mut Queue<Submit>) {
        if usize::try_from(from).expect("typed offset") == bytes.len() {
            self.writing = false;
        } else {
            self.submit(Op::PipeWrite { fd, bytes, from }, out);
        }
    }
    fn type_line(&mut self, mut bytes: Vec<u8>, out: &mut Queue<Submit>) {
        assert!(!self.writing, "one command at a time");
        assert!(!bytes.contains(&b'\n'), "one whole command line");
        assert!(bytes.len() <= PAGE_BYTES, "command fits the terminal line bound");
        bytes.push(b'\n');
        self.commands += 1;
        self.cursor = self.read.pages.len();
        self.writing = true;
        self.write(self.stdin.expect("open terminal input"), bytes.into(), 0, out);
    }
    fn fresh(&self) -> impl Iterator<Item = &Page> {
        self.read.pages[self.cursor..].iter().filter(|page| page.commands == self.commands)
    }
    fn live(&self) -> bool {
        self.fresh().any(|page| page.bytes.split(|byte| *byte == b'\n').next().is_some_and(|s| s.ends_with(b"== Live")))
    }
    pub(crate) fn ready(&self, act: &Act, links: u32) -> bool {
        match act {
            Act::Open => !self.opening || self.live() || !self.active && links > self.link_count,
            Act::TypeOn { .. } => !self.acted || !self.writing && self.fresh().next().is_some(),
            Act::See { holds, lacks } => self
                .fresh()
                .any(|page| contains(&page.bytes, holds) && lacks.is_none_or(|lacks| !contains(&page.bytes, lacks))),
            Act::Close => !self.acted || !self.active,
        }
    }
    pub(crate) fn act(&mut self, act: &Act, scratch: &Path, root: Fd, links: u32, out: &mut Queue<Submit>) -> bool {
        match act {
            Act::Open => {
                if !self.opening {
                    assert!(!self.active, "one open terminal");
                    self.opening = true;
                    self.attempts = 0;
                    self.cursor = self.read.pages.len();
                    self.launch(scratch, root, links, out);
                } else if self.live() {
                    self.opening = false;
                    return true;
                } else if !self.active {
                    assert_eq!(self.read.exit, Some(Exit::Code(1)), "terminal sign-in exit");
                    assert!(self.refused.get(), "only a spent link permits another open");
                    assert_eq!(self.attempts, 1, "retry a spent link only once");
                    if links > self.link_count {
                        self.launch(scratch, root, links, out);
                    }
                }
                false
            }
            Act::TypeOn { label, words } => {
                if !self.acted {
                    let page = self.read.pages.last().expect("last printed terminal page");
                    let number = control(&page.bytes, label).unwrap_or_else(|| {
                        panic!(
                            "missing terminal control {} on page:\n{}",
                            String::from_utf8_lossy(label),
                            String::from_utf8_lossy(&page.bytes)
                        )
                    });
                    let mut bytes = number.to_vec();
                    if !words.is_empty() {
                        bytes.push(b' ');
                        bytes.extend_from_slice(words);
                    }
                    self.acted = true;
                    self.type_line(bytes, out);
                    false
                } else if !self.writing && self.fresh().next().is_some() {
                    self.acted = false;
                    true
                } else {
                    false
                }
            }
            Act::See { .. } => self.ready(act, links),
            Act::Close => {
                if !self.acted {
                    assert!(self.active, "close an open terminal");
                    self.acted = true;
                    self.type_line(b"q".to_vec(), out);
                }
                if self.active {
                    false
                } else {
                    self.acted = false;
                    true
                }
            }
        }
    }
    pub(crate) fn finish(&mut self, out: &mut Queue<Submit>) -> bool {
        if self.active && !self.unclosed {
            self.unclosed = true;
            self.type_line(b"q".to_vec(), out);
        }
        if self.active {
            return false;
        }
        assert!(!self.unclosed, "terminal was still running at the end of the story; harness closed it");
        true
    }
    pub(crate) fn finish_ready(&self) -> bool {
        !self.active || !self.unclosed
    }
    pub(crate) fn is_empty(&self) -> bool {
        !self.active
    }
}
fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || bytes.windows(needle.len()).any(|part| part == needle)
}
fn control<'a>(page: &'a [u8], label: &[u8]) -> Option<&'a [u8]> {
    page.split(|byte| *byte == b'\n').rev().find_map(|line| {
        let line = line.trim_ascii_start();
        let line = line.strip_prefix(b"- ").unwrap_or(line).strip_prefix(b"[")?;
        let end = line.iter().position(|byte| *byte == b']')?;
        let number = &line[..end];
        if number.is_empty() || !number.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let text = line.get(end + 2..)?;
        let matches = text == label
            || text.strip_prefix(label).is_some_and(|rest| rest.starts_with(b":") || rest.starts_with(b" ("));
        matches.then_some(number)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_use_the_last_matching_label_and_ignore_disabled_numbers() {
        let page = b"== Chats == Live\n  [1] Chats\n[2] Chats (current)\n[3] Chats elsewhere\n[-] Chats\n[4] Start a chat: (empty) (enter sends)\n  - [14] T1\n  - [15] T12\n";
        assert_eq!(control(page, b"Chats"), Some(b"2".as_slice()));
        assert_eq!(control(page, b"Start a chat"), Some(b"4".as_slice()));
        assert_eq!(control(page, b"Chat"), None);
        assert_eq!(control(page, b"T1"), Some(b"14".as_slice()));
    }

    #[test]
    fn pages_and_notices_are_whole_and_see_requires_a_fresh_page() {
        let mut typist = Typist::default();
        typist.printed(b"temper: signing in\n== Old == Live\nConversation:\n  answer\n");
        typist.commands = 1;
        typist.cursor = typist.read.pages.len();
        let see = Act::See { holds: b"answer", lacks: Some(b"Writing...") };
        assert!(!typist.ready(&see, 1), "the old page is not a new observation");
        typist.printed(b"== Chat == Live\nWriting...\nanswer\n== Chat == Live\nConversation:\n");
        assert!(!typist.ready(&see, 1), "absence and presence belong to the same page");
        typist.printed(b"== Chat == Li");
        typist.printed(b"ve\nConversation:\n  answer\ntemper: closed\n");
        assert!(typist.ready(&see, 1));
        assert_eq!(typist.read.pages.len(), 4);
        assert_eq!(typist.read.pages[0].commands, 0);
        assert!(typist.read.pages[1..].iter().all(|page| page.commands == 1));
        assert_eq!(typist.read.pages[3].bytes.as_ref(), b"== Chat == Live\nConversation:\n  answer\n");
        assert_eq!(
            typist.read.lines,
            [b"temper: signing in\n".as_slice().into(), b"temper: closed\n".as_slice().into()]
        );
    }

    #[test]
    fn a_partial_write_finishes_the_same_whole_command() {
        let mut typist = Typist {
            stdin: Some(Fd::new(0)),
            active: true,
            pending: 1, // The continuously outstanding stdout read.
            ..Typist::default()
        };
        let mut out = Queue::with_capacity(8);
        typist.type_line(b"12 hello".to_vec(), &mut out);
        let submit = out.pop().expect("first write");
        typist.complete(Complete { op: submit.op, kind: submit.kind, result: Ok(Done::Count(2)) }, &mut out);
        let submit = out.pop().expect("remaining write");
        let Op::PipeWrite { ref bytes, from, .. } = submit.kind else { panic!("command write") };
        assert_eq!(bytes.as_ref(), b"12 hello\n");
        assert_eq!(from, 2);
        typist.complete(Complete { op: submit.op, kind: submit.kind, result: Ok(Done::Count(7)) }, &mut out);
        assert!(!typist.writing);
        assert_eq!(typist.commands, 1);
        assert!(out.is_empty());
    }
}
