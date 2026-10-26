//! The restricted shell of the machine (domain/testing.md, 4.3).
/// The scripted shell's echo redirection acts on the inherited machine files.
pub struct Shell {
    jobs: std::collections::VecDeque<skein_io::kernel::Op>,
    owned: std::collections::BTreeSet<skein_io::kernel::Fd>,
    content: Box<[u8]>,
    target: Vec<u8>,
    written: Vec<u8>,
    code: u8,
    completions: skein_lib::Queue<skein_io::kernel::Complete>,
    submissions: skein_lib::Queue<skein_io::kernel::Submit>,
    pending: bool,
    finished: bool,
}
impl Shell {
    /// The file named by this shell's redirection.
    #[must_use]
    pub fn target(&self) -> &[u8] {
        &self.target
    }
    /// Bytes whose writes have completed, for an outside observer.
    #[must_use]
    pub fn written(&self) -> &[u8] {
        &self.written
    }

    /// Starts the restricted command over its inherited workspace and pipes.
    #[must_use]
    pub fn new(spawn: &skein_io::kernel::Spawn, inherited: &skein_sim_world::Inherited) -> Self {
        use skein_io::kernel::{Op, OpenHow};
        assert_eq!(spawn.args[0].as_ref(), b"-c", "shell command invocation");
        let command = std::str::from_utf8(&spawn.args[1]).ok();
        let edit = command.and_then(|command| command.split_once('>')).and_then(|(echo, target)| {
            let text = echo.trim().strip_prefix("echo ")?;
            let target = target.trim().as_bytes();
            (!target.is_empty() && !target.contains(&b'/') && target != b"..").then_some((text, target))
        });
        let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("shell worker root").1;
        let mut jobs = std::collections::VecDeque::new();
        let (content, target, code) = if let Some((text, target)) = edit {
            let path = [spawn.dir.as_ref(), b"/", target].concat();
            jobs.push_back(Op::Open { root, path: path.into(), how: OpenHow::WriteNoFollow });
            ([text.as_bytes(), b"\n"].concat().into(), target.to_vec(), 0)
        } else {
            let error = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("shell stderr").1;
            jobs.push_back(Op::PipeWrite {
                fd: error,
                bytes: b"/bin/sh: command not found\n".as_slice().into(),
                from: 0,
            });
            (Box::new([]) as Box<[u8]>, Vec::new(), 127)
        };
        let mut owned: std::collections::BTreeSet<_> = inherited.roots.iter().map(|(_, fd)| *fd).collect();
        owned.extend(inherited.pipes.iter().map(|(_, fd)| *fd));
        owned.insert(inherited.signal);
        Self {
            jobs,
            owned,
            content,
            target,
            written: Vec::new(),
            code,
            completions: skein_lib::Queue::with_capacity(1),
            submissions: skein_lib::Queue::with_capacity(1),
            pending: false,
            finished: false,
        }
    }
}
impl skein_shell::Host for Shell {
    fn iterate(&mut self, _now: skein_lib::Time, _wall: skein_lib::Wall) {
        use skein_io::kernel::{Done, Op};
        while let Some(complete) = self.completions.pop() {
            self.pending = false;
            match (complete.kind, complete.result.expect("shell file operation")) {
                (Op::Open { .. }, Done::Fd(fd)) => {
                    self.owned.insert(fd);
                    self.jobs.push_back(Op::Truncate { fd, length: 0 });
                }
                (Op::Truncate { fd, .. }, Done::Nothing) => {
                    self.written.clear();
                    self.jobs.push_back(Op::Write { fd, bytes: self.content.clone(), from: 0, at: 0 });
                }
                (Op::Write { fd, bytes, from, .. }, Done::Count(count)) => {
                    self.written.extend_from_slice(&bytes[from as usize..(from + count) as usize]);
                    let from = from + count;
                    if (from as usize) < bytes.len() {
                        self.jobs.push_back(Op::Write { fd, bytes, from, at: u64::from(from) });
                    }
                }
                (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => {
                    if (from + count) as usize != bytes.len() {
                        self.jobs.push_back(Op::PipeWrite { fd, bytes, from: from + count });
                    }
                }
                (Op::Close { fd }, Done::Nothing) => {
                    self.owned.remove(&fd);
                }
                (op, done) => panic!("shell terminal {op:?}: {done:?}"),
            }
        }
        if !self.pending {
            if let Some(kind) = self.jobs.pop_front().or_else(|| self.owned.first().map(|fd| Op::Close { fd: *fd })) {
                self.pending = true;
                self.submissions.push(skein_io::kernel::Submit { op: skein_lib::Token::new(1), kind });
            } else {
                self.finished = true;
            }
        }
    }
    fn completions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: skein_lib::Time) -> bool {
        !self.completions.is_empty() || (!self.finished && !self.pending)
    }
    fn next_deadline(&self) -> Option<skein_lib::Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<skein_lib::Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.finished
    }
    fn exit(&self) -> Option<skein_io::kernel::Exit> {
        self.finished.then_some(skein_io::kernel::Exit::Code(self.code))
    }
    fn worst_case(&self) -> u64 {
        8192
    }
    fn operations(&self) -> u32 {
        1
    }
}
