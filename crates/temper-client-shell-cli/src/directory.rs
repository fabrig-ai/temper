//! DIRECTORY's private files and its engine address (client/tools.md, 8.1).
use crate::{arguments::Class, limits};
use skein_io::{
    Addr, file,
    file_layer::{self, FileIo},
    kernel::{Complete, Error, Fd, Submit},
};
use skein_lib::{Duration, Queue, Time, Token};
use std::net::SocketAddr;
use temper_client_tools::jig::FileEnd;
const OWNER: Token = Token::new(1);
// The existing five slots plus the two amendment text parents.
pub(crate) const FILE_SLOTS: u32 = 7;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Start,
    Exists,
    Checked,
    Private,
    Lock,
    Address,
    Token,
    Link,
    RecordCreate,
    RecordWrite,
    RecordSync,
    RecordDirectory,
    RecordRemove,
    KeyOpen,
    KeyStat,
    KeyRead,
    KeyClose,
    Store,
    Ready,
    Closing,
}
pub(crate) struct Directory {
    io: FileIo,
    root: Option<Token>,
    file_root: Option<Token>,
    text_roots: Vec<(u8, Token)>,
    text_root: Option<Token>,
    private: Option<Token>,
    check: Option<Token>,
    leaf: Box<[u8]>,
    class: Class,
    stage: Stage,
    started: Option<Time>,
    retry: Option<Time>,
    closing: bool,
    link_path: Option<Box<[u8]>>,
    link_max: u32,
    settings_input: Option<()>,
    key: Option<Token>,
    record: Option<Token>,
    record_path: Option<Box<[u8]>>,
    record_given: Option<Box<[u8]>>,
    record_bytes: Option<Box<[u8]>>,
    pub(crate) created: bool,
    file_end: FileEnd,
    remove_only_created: Option<()>,
    pub(crate) recorded: Option<Result<(), Box<[u8]>>>,
    pub(crate) not_removed: Option<Box<[u8]>>,
    key_bytes: Vec<u8>,
    user: u32,
    pub(crate) link: Option<Box<[u8]>>,
    pub(crate) stored: bool,
    pub(crate) engine: Option<Addr>,
    pub(crate) address: Option<Box<[u8]>>,
    pub(crate) token: Option<Box<[u8]>>,
    pub(crate) error: Option<Box<[u8]>>,
    pub(crate) events: Queue<file::Event>,
    pub(crate) submissions: Queue<Submit>,
}
impl Directory {
    pub(crate) fn new(
        parent: Option<Fd>,
        file_parent: Option<Fd>,
        leaf: Box<[u8]>,
        class: Class,
        user: u32,
        seed: u64,
        files: Vec<(u8, Fd)>,
    ) -> Self {
        let mut io = FileIo::new(
            FILE_SLOTS,
            u32::try_from(limits::native().root.tools.ask_bytes).expect("call bound"),
            1,
            Duration::from_secs(30),
        )
        .with_effective_user(user);
        io.seed_randomness(seed);
        let root = parent.map(|fd| io.adopt_root(fd).expect("parent slot"));
        let file_root = file_parent.map(|fd| io.adopt_root(fd).expect("file parent slot"));
        let mut text_roots = Vec::new();
        for (index, fd) in files {
            text_roots.push((index, io.adopt_root(fd).expect("text file parent slot")));
        }
        Self {
            io,
            text_roots,
            text_root: None,
            root,
            file_root,
            private: None,
            check: None,
            leaf,
            class,
            stage: Stage::Start,
            started: None,
            retry: None,
            closing: false,
            link_path: None,
            link_max: limits::LINK,
            settings_input: None,
            key: None,
            record: None,
            record_path: None,
            record_given: None,
            record_bytes: None,
            created: false,
            file_end: FileEnd::Keep,
            remove_only_created: None,
            recorded: None,
            not_removed: None,
            key_bytes: Vec::new(),
            user,
            link: None,
            stored: false,
            engine: None,
            address: None,
            token: None,
            error: None,
            events: Queue::with_capacity(16),
            submissions: Queue::with_capacity(16),
        }
    }
    pub(crate) fn complete(&mut self, complete: Complete) {
        file_layer::up(&mut self.io, complete, &mut self.events, &mut self.submissions);
    }
    pub(crate) fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.retry = None;
        if !self.io.takes() {
            file_layer::cancel(&mut self.io, OWNER, &mut self.submissions);
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.io.open_files() == 0 && self.io.takes() && self.submissions.is_empty()
    }
    pub(crate) fn deadline(&self) -> Option<Time> {
        [self.retry, self.io.next_deadline()].into_iter().flatten().min()
    }
    pub(crate) fn pending(&self, now: Time) -> bool {
        !self.events.is_empty()
            || self.io.is_due(now)
            || (self.io.takes()
                && (self.closing && self.io.open_files() > 0
                    || !self.closing
                        && self.root.is_some()
                        && self.stage != Stage::Ready
                        && self.retry.is_none_or(|at| now >= at)))
    }
    pub(crate) fn retry(&self) -> Option<Time> {
        self.retry
    }
    fn fail(&mut self, text: &[u8]) {
        self.error = Some(text.into());
        self.close();
    }
    fn answer(&mut self, now: Time, event: file::Event) {
        if let file::Event::Opened { file, .. } = &event {
            if self.stage == Stage::RecordCreate {
                self.record = Some(*file);
                self.created = true;
            } else if self.stage == Stage::KeyOpen {
                self.key = Some(*file);
            } else if self.stage == Stage::Exists {
                self.check = Some(*file);
            } else {
                self.private = Some(*file);
            }
        }
        if self.stage == Stage::RecordRemove {
            self.file_end = FileEnd::Keep;
            if let file::Event::Failed { error, .. } = event {
                self.not_removed = Some(format!("{error:?}").into_bytes().into_boxed_slice());
            }
            self.stage = Stage::Ready;
            return;
        }
        if self.closing {
            return;
        }
        if matches!(self.stage, Stage::RecordCreate | Stage::RecordWrite | Stage::RecordSync | Stage::RecordDirectory) {
            self.answer_record(event);
            return;
        }
        if let file::Event::Refused { found, .. } = event {
            let part = match self.stage {
                Stage::Address | Stage::Store => "address",
                Stage::Token => "token",
                Stage::KeyOpen | Stage::KeyStat | Stage::KeyRead | Stage::KeyClose => "key file",
                Stage::Link => "link",
                Stage::RecordCreate
                | Stage::RecordWrite
                | Stage::RecordSync
                | Stage::RecordDirectory
                | Stage::RecordRemove
                | Stage::Start
                | Stage::Exists
                | Stage::Checked
                | Stage::Private
                | Stage::Lock
                | Stage::Ready
                | Stage::Closing => "DIRECTORY",
            };
            self.fail(format!("{part} is not private: {found:?}").as_bytes());
            return;
        }
        self.answer_file(now, event);
    }
    fn answer_file(&mut self, now: Time, event: file::Event) {
        match (self.stage, event) {
            (Stage::KeyOpen | Stage::KeyStat | Stage::KeyRead | Stage::KeyClose, event) => self.answer_key(event),
            (Stage::Link, file::Event::Loaded { bytes, .. }) => {
                self.link = Some(bytes);
                self.stage = Stage::Ready;
            }
            (Stage::Link, file::Event::TooLarge { .. }) if self.settings_input.is_some() => {
                self.fail(format!("settings file: file bytes: bound {}", self.link_max).as_bytes());
            }
            (Stage::Store, file::Event::Stored { .. }) => {
                self.stored = true;
                self.stage = Stage::Ready;
            }
            (Stage::Exists, file::Event::Opened { .. }) => self.stage = Stage::Checked,
            (Stage::Checked, file::Event::Closed { .. }) => {
                self.check = None;
                self.stage = Stage::Private;
            }
            (Stage::Private, file::Event::Opened { .. }) => {
                self.stage = if self.class.locks() { Stage::Lock } else { Stage::Address }
            }
            (Stage::Lock, file::Event::Locked { .. }) => {
                self.retry = None;
                self.stage = Stage::Address;
            }
            (Stage::Lock, file::Event::Busy { .. }) => self.retry = Some(now.saturating_add(limits::LOCK_TRY)),
            (Stage::Address, file::Event::Loaded { bytes, .. }) => match address(&bytes) {
                Ok(engine) => {
                    self.engine = Some(engine);
                    self.address = Some(bytes[..bytes.len() - 1].into());
                    self.stage = Stage::Token;
                }
                Err(error) => self.fail(error),
            },
            (Stage::Token, file::Event::Loaded { bytes, .. }) if bytes.is_empty() => {
                self.fail(b"DIRECTORY is not signed in");
            }
            (Stage::Token, file::Event::Loaded { bytes, .. }) => {
                self.token = Some(bytes);
                self.stage = Stage::Ready;
            }
            (Stage::Exists, file::Event::Failed { error: Error::NotFound, .. }) => {
                self.fail(b"DIRECTORY does not exist");
            }
            (Stage::Token, file::Event::Failed { error: Error::NotFound, .. }) => {
                self.fail(b"DIRECTORY is not signed in");
            }
            (stage, event) => match stage {
                Stage::KeyOpen | Stage::KeyStat | Stage::KeyRead | Stage::KeyClose => self.answer_key(event),
                Stage::RecordCreate
                | Stage::RecordWrite
                | Stage::RecordSync
                | Stage::RecordDirectory
                | Stage::RecordRemove
                | Stage::Start
                | Stage::Exists
                | Stage::Checked
                | Stage::Private
                | Stage::Lock
                | Stage::Address
                | Stage::Token
                | Stage::Link
                | Stage::Store
                | Stage::Ready
                | Stage::Closing => self.fail(b"DIRECTORY could not be read"),
            },
        }
    }
    fn answer_key(&mut self, event: file::Event) {
        match event {
            file::Event::Failed { error, .. } => {
                self.fail(if error == Error::NotFound {
                    b"key file not found"
                } else if error == Error::TooManyLinks {
                    b"key file must not be a link"
                } else {
                    b"key file could not be read"
                });
            }
            file::Event::Opened { .. } => self.stage = Stage::KeyStat,
            file::Event::Stated { stat, .. } => {
                if stat.mode & 0o077 != 0 {
                    self.fail(format!("key file mode {:#o} permits group or other access", stat.mode).as_bytes());
                } else if stat.kind != skein_io::kernel::Kind::File || stat.owner != self.user {
                    self.fail(b"key file must be a private regular file owned by this user");
                } else if stat.size > u64::from(self.link_max) {
                    self.fail(b"key file is too large");
                } else {
                    self.key_bytes = Vec::with_capacity(usize::try_from(self.link_max).expect("key bound"));
                    self.stage = Stage::KeyRead;
                }
            }
            file::Event::Read { bytes, .. } => {
                if self.key_bytes.len() + bytes.len() > usize::try_from(self.link_max).expect("key bound") {
                    self.fail(b"key file is too large");
                } else if bytes.is_empty() {
                    self.stage = Stage::KeyClose;
                } else {
                    self.key_bytes.extend_from_slice(&bytes);
                }
            }
            file::Event::Closed { .. } => {
                self.key = None;
                self.link = Some(std::mem::take(&mut self.key_bytes).into_boxed_slice());
                self.stage = Stage::Ready;
            }
            file::Event::Locked { .. }
            | file::Event::Busy { .. }
            | file::Event::Made { .. }
            | file::Event::Refused { .. }
            | file::Event::Loaded { .. }
            | file::Event::Scanned { .. }
            | file::Event::Stored { .. }
            | file::Event::Conflict { .. }
            | file::Event::TooLarge { .. }
            | file::Event::Written { .. }
            | file::Event::Cut { .. }
            | file::Event::Synced { .. }
            | file::Event::Renamed { .. }
            | file::Event::Removed { .. }
            | file::Event::Listed { .. }
            | file::Event::Cancelled { .. } => self.fail(b"key file could not be read"),
        }
    }
    pub(crate) fn progress(&mut self, now: Time) {
        if self.io.is_due(now) {
            file_layer::expire(&mut self.io, now, &mut self.submissions);
        }
        while let Some(event) = self.events.pop() {
            self.answer(now, event);
        }
        if !self.io.takes() {
            return;
        }
        if self.closing {
            self.progress_close(now);
            return;
        }
        let Some(root) = self.root else {
            return;
        };
        if self.stage == Stage::Start {
            self.stage = if self.class == Class::SignIn { Stage::Private } else { Stage::Exists };
        }
        let request = match self.stage {
            Stage::Exists => file::Request::OpenDirectory {
                owner: OWNER,
                root: self.io.descriptor(root).expect("parent"),
                name: self.leaf.clone(),
                no_follow: true,
            },
            Stage::Checked => file::Request::Close { owner: OWNER, file: self.check.expect("checked directory") },
            Stage::Private => file::Request::OpenPrivate { owner: OWNER, root, path: self.leaf.clone() },
            Stage::Lock => {
                let started = *self.started.get_or_insert(now);
                if now >= started.saturating_add(limits::LOCK) {
                    self.fail(b"DIRECTORY is busy with another call");
                    return;
                }
                if self.retry.is_some_and(|at| now < at) {
                    return;
                }
                self.retry = None;
                file::Request::Lock { owner: OWNER, root: self.private.expect("private directory") }
            }
            Stage::Address if matches!(self.class, Class::SignIn | Class::Pending) => {
                self.stage = Stage::Ready;
                return;
            }
            Stage::RecordCreate => file::Request::CreateNoFollow {
                owner: OWNER,
                root: self.io.descriptor(self.file_root.expect("record parent")).expect("record descriptor"),
                name: self.record_path.as_ref().expect("record leaf").clone(),
                mode: 0o600,
            },
            Stage::RecordWrite => file::Request::WriteAt {
                owner: OWNER,
                file: self.record.expect("record file"),
                offset: 0,
                bytes: self.record_bytes.take().expect("record bytes"),
            },
            Stage::RecordSync => file::Request::Sync { owner: OWNER, file: self.record.expect("record file") },
            Stage::RecordDirectory => file::Request::SyncDirectory {
                owner: OWNER,
                root: self.io.descriptor(self.file_root.expect("record parent")).expect("record descriptor"),
            },
            Stage::KeyOpen => file::Request::OpenReadNoFollow {
                owner: OWNER,
                root: self.io.descriptor(self.file_root.expect("key parent")).expect("file parent"),
                name: self.link_path.take().expect("key leaf"),
            },
            Stage::KeyStat => file::Request::Stat { owner: OWNER, file: self.key.expect("key file") },
            Stage::KeyRead => file::Request::ReadAt {
                owner: OWNER,
                file: self.key.expect("key file"),
                offset: u64::try_from(self.key_bytes.len()).expect("key offset"),
                max: self.link_max + 1 - u32::try_from(self.key_bytes.len()).expect("key length"),
            },
            Stage::KeyClose => file::Request::Close { owner: OWNER, file: self.key.expect("key file") },
            Stage::Link => file::Request::Load {
                owner: OWNER,
                root: self.text_root.or(self.file_root).expect("FILE's parent"),
                path: self.link_path.take().expect("link file"),
                max: self.link_max,
                no_follow: true,
            },
            Stage::Address | Stage::Token => {
                let (path, max) = if self.stage == Stage::Address {
                    (b"address".as_slice(), limits::ADDRESS)
                } else {
                    (b"token".as_slice(), 40)
                };
                file::Request::Load {
                    owner: OWNER,
                    root: self.private.expect("private directory"),
                    path: path.into(),
                    max,
                    no_follow: true,
                }
            }
            Stage::RecordRemove | Stage::Start | Stage::Ready | Stage::Store | Stage::Closing => return,
        };
        self.request(now, request);
    }
    fn progress_close(&mut self, now: Time) {
        if self.file_end == FileEnd::Remove && (self.remove_only_created.is_none() || self.created) {
            self.stage = Stage::RecordRemove;
            self.request(
                now,
                file::Request::Remove {
                    owner: OWNER,
                    root: self.io.descriptor(self.file_root.expect("record parent")).expect("record descriptor"),
                    name: self.record_path.as_ref().expect("record leaf").clone(),
                    directory: false,
                },
            );
            return;
        }
        if let Some(file) = self
            .record
            .take()
            .or_else(|| self.key.take())
            .or_else(|| self.check.take())
            .or_else(|| self.private.take())
            .or_else(|| self.close_text_root())
            .or_else(|| self.file_root.take())
            .or_else(|| self.root.take())
        {
            self.stage = Stage::Closing;
            self.request(now, file::Request::Close { owner: OWNER, file });
        }
    }
    pub(crate) fn ready(&self) -> bool {
        self.stage == Stage::Ready && self.io.takes()
    }
    pub(crate) fn parent(&self) -> Fd {
        self.io.descriptor(self.root.expect("parent")).expect("parent descriptor")
    }
    pub(crate) fn leaf(&self) -> Box<[u8]> {
        self.leaf.clone()
    }
    pub(crate) fn start(&mut self, class: Class) {
        self.class = class;
        self.stage = Stage::Start;
    }
    pub(crate) fn load_key(&mut self, path: &[u8], max: u32) {
        if self.file_root.is_none() {
            self.fail(b"key file could not be read: file has no parent");
            return;
        }
        self.load_link(path);
        if self.closing {
            return;
        }
        self.link_max = max + 1;
        self.stage = Stage::KeyOpen;
    }
    pub(crate) fn create_record(&mut self, path: &[u8]) {
        if self.file_root.is_none() {
            self.fail(b"record file could not be created: file has no parent");
            return;
        }
        self.record_path = Some(Self::file_leaf(path));
        self.record_given = Some(path.into());
        self.remove_only_created = Some(());
        self.stage = Stage::RecordCreate;
    }
    pub(crate) fn record_path(&mut self, path: &[u8]) {
        self.record_path = Some(Self::file_leaf(path));
    }
    fn file_leaf(path: &[u8]) -> Box<[u8]> {
        let start = path.iter().rposition(|byte| *byte == b'/').map_or(0, |at| at + 1);
        path[start..].into()
    }
    pub(crate) fn write_record(&mut self, record: &skein_lib::Secret) {
        self.record_bytes = Some(record.expose().into());
        self.stage = Stage::RecordWrite;
    }
    pub(crate) fn end_record(&mut self, end: FileEnd) {
        if self.record_path.is_some() {
            self.file_end = end;
        }
    }
    fn answer_record(&mut self, event: file::Event) {
        match (self.stage, event) {
            (Stage::RecordCreate, file::Event::Opened { .. }) => self.stage = Stage::Ready,
            (Stage::RecordCreate, file::Event::Failed { error, .. }) => {
                let mut message = self.record_given.as_deref().expect("record path").to_vec();
                message.extend_from_slice(if matches!(error, Error::Exists | Error::TooManyLinks) {
                    b" exists"
                } else {
                    b" could not be created"
                });
                self.fail(&message);
            }
            (Stage::RecordWrite, file::Event::Written { .. }) => self.stage = Stage::RecordSync,
            (Stage::RecordSync, file::Event::Synced { .. }) => self.stage = Stage::RecordDirectory,
            (Stage::RecordDirectory, file::Event::Synced { .. }) => {
                self.stage = Stage::Ready;
                self.recorded = Some(Ok(()));
            }
            (_, file::Event::Failed { error, .. }) => {
                self.stage = Stage::Ready;
                self.recorded = Some(Err(format!("{error:?}").into_bytes().into_boxed_slice()));
            }
            _ => {
                self.stage = Stage::Ready;
                self.recorded = Some(Err(b"record file operation failed".as_slice().into()));
            }
        }
    }
    fn close_text_root(&mut self) -> Option<Token> {
        let (_, root) = self.text_roots.pop()?;
        Some(root)
    }
    pub(crate) fn load_text(&mut self, index: u8, path: &[u8], settings_input: bool) {
        self.settings_input = if settings_input { Some(()) } else { None };
        self.text_root = None;
        for (argument, root) in &self.text_roots {
            if *argument == index {
                self.text_root = Some(*root);
            }
        }
        if self.text_root.is_none() {
            self.fail(b"text file could not be read: file has no parent");
            return;
        }
        let start = path.iter().rposition(|byte| *byte == b'/').map_or(0, |at| at + 1);
        self.link_path = Some(path[start..].into());
        self.link_max = u32::try_from(limits::native().root.tools.ask_bytes).expect("text bound");
        self.stage = Stage::Link;
    }
    pub(crate) fn load_call(&mut self, path: &[u8]) {
        self.load_link(path);
        self.link_max = u32::try_from(limits::native().root.tools.ask_bytes).expect("call bound");
    }
    pub(crate) fn load_link(&mut self, path: &[u8]) {
        if self.file_root.is_none() {
            self.fail(b"DIRECTORY could not be read: link file has no parent");
            return;
        }
        self.link_max = limits::LINK;
        let start = path.iter().rposition(|byte| *byte == b'/').map_or(0, |at| at + 1);
        self.link_path = Some(path[start..].into());
        self.stage = Stage::Link;
    }
    pub(crate) fn store_address(&mut self, now: Time, bytes: Box<[u8]>) {
        self.stage = Stage::Store;
        self.request(
            now,
            file::Request::Store {
                owner: OWNER,
                root: self.private.expect("locked directory"),
                path: b"address".as_slice().into(),
                bytes,
                expected: file::Expect::Any,
                no_follow: true,
            },
        );
    }
    fn request(&mut self, now: Time, request: file::Request) {
        file_layer::down(&mut self.io, now, request, &mut self.events, &mut self.submissions);
    }
}
/// The address is numeric HTTP, one newline-terminated line, on loopback.
pub(crate) fn address(bytes: &[u8]) -> Result<Addr, &'static [u8]> {
    let bad = b"the engine's address must be on loopback".as_slice();
    let line = bytes.strip_suffix(b"\n").ok_or(bad)?;
    let host = line.strip_prefix(b"http://").ok_or(bad)?;
    let socket: SocketAddr = std::str::from_utf8(host).map_err(|_| bad)?.parse().map_err(|_| bad)?;
    if !socket.ip().is_loopback() {
        return Err(bad);
    }
    Ok(Addr::Ip(socket))
}
