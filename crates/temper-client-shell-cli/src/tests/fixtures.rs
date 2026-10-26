use crate::{
    arguments,
    host::{Invocation, OneShot, Startup},
};
use jig_client_protocol_wire as w;
use skein_io::kernel::{Complete, Done, Error, Exit, Fd, Kind, Op, OpenHow, ServiceSignal, Signalled, Stat, Submit};
use skein_lib::{Duration, Time, Wall};
use skein_shell::Host;
use std::collections::{BTreeMap, VecDeque};
macro_rules! encode {
    ($value:expr) => {{
        let value = &$value;
        let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("record size"));
        value.encode(&mut writer).expect("record fits");
        writer.finish()
    }};
}
pub mod local;
pub mod moves;
pub mod recovery;
pub mod waits;
pub use recovery::{dropped, kept_chat, other_party, small_store};
pub const LINK: &[u8] = b"http://127.0.0.1:4000/jig/v1/sign-in/link?secret=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const LINK_FILE: Fd = Fd::new(23);
pub const FILE_PARENT: Fd = Fd::new(6);
pub const CHAT: &[&[u8]] = &[b"chat", b"hello", b"--project", b"shop", b"--key", b"start-chat"];
const DIRECTORY: Fd = Fd::new(20);
const ADDRESS: Fd = Fd::new(21);
const TOKEN: Fd = Fd::new(22);
const STDERR: Fd = Fd::new(3);
const USER: u32 = 7;
pub const TOKEN_BYTES: &[u8] = b"0123456789012345678901234567890123456789";
pub const PARTIAL: usize = 3; // More than one short write is needed for every host diagnostic.
#[expect(clippy::struct_excessive_bools, reason = "independent scripted file and network facts")]
pub struct Frame {
    pub move_answers: VecDeque<moves::Answer>,
    pub move_requests: Vec<(w::AccountHolder, w::AccountChange)>,
    pub file_steps: Vec<&'static str>,
    pub record_exists: bool,
    pub record_existing_kind: Option<Kind>,
    pub hold_record_create: bool,
    pub race_record_create: bool,
    pub store_history: Vec<Vec<u8>>,
    pub record_error: Option<Error>,
    pub record_fault: Option<&'static str>,
    pub record_removed: bool,
    pub record_data: Vec<u8>,
    pub record_how: Option<OpenHow>,
    pub record_root: Option<Fd>,
    move_mode: bool,
    record_fd: Option<Fd>,
    pub host: OneShot,
    pub link: &'static [u8],
    pub input: &'static [u8],
    pub link_root: Option<Fd>,
    pub link_leaf: Option<Box<[u8]>>,
    pub input_stat: Option<Stat>,
    pub input_reads: u32,
    pub linked_leaf: Option<Error>,
    pub carried_keys: Vec<skein_lib::Secret>,
    pub refuse_link: Option<()>,
    pub keep_link: Option<()>,
    pub link_bearers: Vec<Option<Box<[u8]>>>,
    pub again_links: u32,
    pub busy_links: u32,
    pub full_link: Option<(u32, Wall, Wall)>,
    pub link_posts: Vec<Box<[u8]>>,
    hold_stores: Option<()>,
    store: Option<Box<[u8]>>,
    offers: Option<()>,
    answer_kept: Option<()>,
    answer_new: Option<()>,
    acts: Vec<[u8; 16]>,
    fail_store: Option<()>,
    store_writes: u32,
    files: BTreeMap<Fd, Vec<u8>>,
    names: BTreeMap<Box<[u8]>, Fd>,
    stored: BTreeMap<Box<[u8]>, Vec<u8>>,
    sockets: BTreeMap<Fd, Socket>,
    held: Vec<Submit>,
    opened: Vec<Box<[u8]>>,
    requests: Vec<Box<[u8]>>,
    deliver: Option<()>,
    watch: Option<u64>,
    wait_task: Option<u64>,
    stopping: Option<()>,
    stream: Option<Fd>,
    stdout: Vec<u8>,
    next_socket: i32,
    now: Time,
    start: Time,
    signal: Option<Submit>,
    pub busy: bool,
    pub absent: bool,
    pub refused: bool,
    pub address: &'static [u8],
    pub token: &'static [u8],
    pub locks: u32,
    locked: Option<()>,
    pub released_after: Option<u32>,
    pub token_reads: u32,
    pub private_opens: u32,
    stderr: Vec<u8>,
    pub write_size: usize,
    pub writes: u32,
    probe: Option<skein_lib::Token>,
    probe_replies: Vec<(u64, Time, Box<[u8]>)>,
}
impl Frame {
    pub fn act(words: &[&[u8]]) -> Self {
        let parsed = arguments::parse(words).expect("fixture call");
        let move_mode = matches!(&parsed, temper_client_tools::jig::call::Parsed::Call { call, .. } if call.tool == temper_client_tools::jig::rows::EXPORT_AT || call.tool == temper_client_tools::jig::rows::IMPORT_AT);
        let file_parent = arguments::file_root(&parsed).map(|_| FILE_PARENT);
        let files = arguments::text_files(&parsed)
            .into_iter()
            .map(|(index, _)| (index, Fd::new(30 + i32::from(index))))
            .collect();
        let host = OneShot::with_text_files(
            Invocation { directory: Some(b"directory".as_slice().into()), parsed },
            Startup {
                parent: Some(Fd::new(1)),
                file_parent,
                stdin: Fd::new(2),
                stdout: Fd::new(4),
                stderr: STDERR,
                signals: Fd::new(5),
                user: USER,
                seed: 1,
            },
            files,
        )
        .expect("native limits");
        Self {
            move_answers: VecDeque::new(),
            move_requests: Vec::new(),
            file_steps: Vec::new(),
            record_exists: false,
            record_existing_kind: None,
            hold_record_create: false,
            race_record_create: false,
            store_history: Vec::new(),
            record_error: None,
            record_fault: None,
            record_removed: false,
            record_data: Vec::new(),
            record_how: None,
            record_root: None,
            move_mode,
            record_fd: None,
            host,
            link: LINK,
            input: b"",
            link_root: None,
            link_leaf: None,
            input_stat: None,
            input_reads: 0,
            linked_leaf: None,
            carried_keys: Vec::new(),
            refuse_link: None,
            keep_link: None,
            link_bearers: Vec::new(),
            again_links: 0,
            busy_links: 0,
            full_link: None,
            link_posts: Vec::new(),
            hold_stores: None,
            store: None,
            offers: None,
            answer_kept: None,
            answer_new: None,
            acts: Vec::new(),
            fail_store: None,
            store_writes: 0,
            files: BTreeMap::new(),
            names: BTreeMap::new(),
            stored: BTreeMap::new(),
            sockets: BTreeMap::new(),
            held: Vec::new(),
            opened: Vec::new(),
            requests: Vec::new(),
            deliver: None,
            watch: None,
            wait_task: None,
            stopping: None,
            stream: None,
            stdout: Vec::new(),
            next_socket: 100,
            now: Time::ZERO,
            start: Time::ZERO,
            signal: None,
            busy: false,
            absent: false,
            refused: false,
            address: b"http://127.0.0.1:4000\n",
            token: TOKEN_BYTES,
            locks: 0,
            locked: None,
            released_after: None,
            token_reads: 0,
            private_opens: 0,
            stderr: Vec::new(),
            write_size: usize::MAX,
            writes: 0,
            probe: None,
            probe_replies: Vec::new(),
        }
    }
    pub fn sign_in(link: &'static [u8]) -> Self {
        let mut frame = Self::act(&[b"sign-in", b"link-file"]);
        frame.link = link;
        frame.token = b"";
        frame.hold_stores = Some(());
        frame
    }
    pub fn answer_sign_in_then_me(&mut self) {
        self.sign_in_then_deliver_me();
    }
    pub fn store_token_with_address(&mut self) {
        self.hold_stores = None;
        self.run();
    }
    pub fn stored(&self, name: &[u8]) -> Option<&[u8]> {
        self.stored.get(name).map(Vec::as_slice)
    }
    pub fn requests(&self) -> usize {
        self.requests.len()
    }
    pub fn read(words: &[&str]) -> Self {
        Self::act(&words.iter().map(|word| word.as_bytes()).collect::<Vec<_>>())
    }
    pub fn sign_in_then_deliver_me(&mut self) {
        self.deliver = Some(());
        if self.watch.is_some() {
            self.deliver_me();
        }
        self.run();
    }
    pub fn drain(&mut self) {
        self.run();
    }
    pub fn opened(&self, path: &[u8]) -> bool {
        self.opened.iter().any(|name| name.as_ref() == path)
    }
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn sent_stream_gone(&self) -> bool {
        self.requests.iter().any(|path| path.as_ref() == b"/jig/v1/stream-gone")
    }
    pub fn started(&self) -> Time {
        self.start
    }
    pub fn next_deadline(&self) -> Option<Time> {
        self.host.next_policy_deadline()
    }
    pub fn lock_busy(&mut self) {
        self.busy = true;
        self.run();
    }
    pub fn advance_past(&mut self, duration: Duration) {
        self.now = self.now.saturating_add(duration);
        self.run();
    }
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    pub fn exit(&self) -> Option<Exit> {
        self.host.exit()
    }
    pub fn signal(&mut self, signal: ServiceSignal) {
        self.run();
        let Submit { op, kind } = self.signal.take().expect("armed signal read");
        self.host.completions().push(Complete {
            op,
            kind,
            result: Ok(Done::Signalled(Signalled::Termination { signal, sender: None })),
        });
        self.run();
    }
    pub fn run(&mut self) {
        for _ in 0..2000 {
            for mut submit in std::mem::take(&mut self.held) {
                if matches!(submit.kind, Op::Sync { .. }) && self.hold_stores.is_none()
                    || matches!(&submit.kind, Op::Recv { fd, .. } if self.sockets.get(fd).is_some_and(|s| !s.reply.is_empty() || s.eof.is_some()))
                {
                    let result = self.answer(&mut submit.kind);
                    self.host.completions().push(Complete { op: submit.op, kind: submit.kind, result });
                } else {
                    self.held.push(submit);
                }
            }
            self.host.iterate(self.now, Wall::EPOCH);
            self.probe_reply();
            let mut answered = false;
            while let Some(mut submit) = self.host.submissions().pop() {
                if self.hold_record_create
                    && matches!(&submit.kind, Op::Open { path, how: OpenHow::CreateNoFollow { .. }, .. } if path.as_ref() == b"record")
                {
                    self.held.push(submit);
                    continue;
                }
                if self.hold_stores.is_some() && matches!(submit.kind, Op::Sync { .. }) {
                    self.held.push(submit);
                    continue;
                }
                if matches!(submit.kind, Op::ReadSignal { .. }) {
                    self.signal = Some(submit);
                    continue;
                }
                if let Op::Recv { fd, .. } = &submit.kind
                    && self.sockets.get(fd).is_some_and(|s| s.reply.is_empty() && s.eof.is_none())
                {
                    self.held.push(submit);
                    continue;
                }
                let result = self.answer(&mut submit.kind);
                self.host.completions().push(Complete { op: submit.op, kind: submit.kind, result });
                answered = true;
            }
            if !answered && !self.host.work_pending(self.now) {
                return;
            }
        }
        panic!("host did not settle");
    }
    fn stat(&self, fd: Fd) -> Done {
        if fd == LINK_FILE
            && let Some(stat) = self.input_stat
        {
            return Done::Stat(stat);
        }
        let (kind, size, mode, links) = if fd == DIRECTORY {
            (Kind::Directory, 0, if self.refused { 0o777 } else { 0o700 }, 2)
        } else {
            (Kind::File, u64::try_from(self.contents(fd).len()).expect("fixture size"), 0o600, 1)
        };
        Done::Stat(Stat { kind, size, mode, owner: USER, links })
    }
    fn contents(&self, fd: Fd) -> &[u8] {
        if fd == ADDRESS {
            self.address
        } else if fd == TOKEN {
            self.token
        } else if fd == LINK_FILE {
            self.link
        } else if fd == Fd::new(24) {
            self.store.as_deref().expect("existing store")
        } else {
            self.files.get(&fd).expect("fixture file")
        }
    }
    fn read_file(&self, fd: Fd, buf: &mut [u8], at: u64) -> Done {
        let bytes = self.contents(fd);
        let start = usize::try_from(at).expect("fixture offset").min(bytes.len());
        let count = buf.len().min(bytes.len() - start);
        buf[..count].copy_from_slice(&bytes[start..start + count]);
        Done::Count(u32::try_from(count).expect("fixture count"))
    }
    fn open_file(&mut self, root: Fd, path: &[u8], how: OpenHow) -> Result<Done, Error> {
        self.opened.push(path.into());
        if path == b"record" && matches!(how, OpenHow::CreateNoFollow { .. }) {
            self.record_how = Some(how);
            self.record_root = Some(root);
            if self.record_exists || self.record_existing_kind.is_some() {
                return Err(Error::Exists);
            }
            if let Some(error) = self.record_error {
                return Err(error);
            }
            self.file_steps.push("create");
            let fd = Fd::new(self.next_socket);
            self.next_socket += 1;
            self.files.insert(fd, Vec::new());
            self.record_fd = Some(fd);
            self.record_exists = true;
            return Ok(Done::Fd(fd));
        }
        if matches!(how, OpenHow::Create { .. } | OpenHow::CreateNoFollow { .. }) {
            let fd = Fd::new(self.next_socket);
            self.next_socket += 1;
            self.files.insert(fd, Vec::new());
            self.names.insert(path.into(), fd);
            return Ok(Done::Fd(fd));
        }
        if path == b"call.json" || path == b"link-file" || path == b"link" || path == b"record" {
            assert_eq!(how, OpenHow::ReadNoFollow, "check the file at its name");
            if let Some(error) = self.linked_leaf {
                return Err(error);
            }
            self.link_root = Some(root);
            self.link_leaf = Some(path.into());
            return Ok(Done::Fd(LINK_FILE));
        }
        if path == b"store" {
            return self.store.as_ref().map_or(Err(Error::NotFound), |_| Ok(Done::Fd(Fd::new(24))));
        }
        if path == b"directory" {
            if how == OpenHow::ReadNoFollow {
                self.private_opens += 1;
            }
            if self.absent {
                return Err(Error::NotFound);
            }
            Ok(Done::Fd(DIRECTORY))
        } else if path == b"address" {
            Ok(Done::Fd(ADDRESS))
        } else if path == b"token" {
            self.token_reads += 1;
            Ok(Done::Fd(TOKEN))
        } else {
            assert_eq!(path, b".");
            Ok(Done::Fd(Fd::new(19)))
        }
    }
    fn lock_directory(&mut self, fd: Fd) -> Result<Done, Error> {
        assert_eq!(fd, DIRECTORY);
        self.locks += 1;
        if self.busy {
            Err(Error::Busy)
        } else {
            self.locked = Some(());
            Ok(Done::Nothing)
        }
    }
    fn answer(&mut self, kind: &mut Op) -> Result<Done, Error> {
        match kind {
            Op::Open { root, path, how } => self.open_file(*root, path, *how),
            Op::Write { fd, bytes, from, at } => {
                self.record_write(*fd, bytes, *from, *at)?;
                let start = usize::try_from(*from).expect("write offset");
                let at = usize::try_from(*at).expect("file offset");
                let bytes = &bytes[start..];
                let file = self.files.get_mut(fd).expect("temporary file");
                file.resize(at + bytes.len(), 0);
                file[at..].copy_from_slice(bytes);
                Ok(Done::Count(u32::try_from(bytes.len()).expect("written size")))
            }
            Op::Rename { from, to, .. } => {
                let fd = self.names.remove(from).expect("temporary name");
                self.stored.insert(to.clone(), self.files.get(&fd).expect("temporary contents").clone());
                self.store_history.push(self.files.get(&fd).expect("stored contents").clone());
                if to.as_ref() == b"store" {
                    self.store_writes += 1;
                }
                Ok(Done::Nothing)
            }
            Op::Stat { fd } => Ok(self.stat(*fd)),
            Op::Read { fd, buf, at } => {
                if *fd == LINK_FILE {
                    self.input_reads += 1;
                }
                Ok(self.read_file(*fd, buf, *at))
            }
            Op::Lock { fd } => self.lock_directory(*fd),
            Op::PipeWrite { fd, bytes, from } => {
                assert!([STDERR, Fd::new(4)].contains(fd));
                let start = usize::try_from(*from).expect("fixture offset");
                let count = (bytes.len() - start).min(self.write_size);
                if *fd == STDERR {
                    self.stderr.extend_from_slice(&bytes[start..start + count]);
                } else {
                    self.stdout.extend_from_slice(&bytes[start..start + count]);
                }
                self.writes += 1;
                Ok(Done::Count(u32::try_from(count).expect("fixture count")))
            }
            Op::Socket { .. } => {
                let fd = Fd::new(self.next_socket);
                self.next_socket += 1;
                self.sockets.insert(fd, Socket::default());
                Ok(Done::Fd(fd))
            }
            Op::Sync { fd } if Some(*fd) == self.record_fd || *fd == FILE_PARENT => self.record_sync(*fd),
            Op::Remove { name, dir, .. } if name.as_ref() == b"record" => self.record_remove(*dir),
            Op::Sync { .. } if self.fail_store.is_some() => Err(Error::Permission),
            Op::Close { fd } if *fd == DIRECTORY && self.locked.is_some() => {
                self.released_after = Some(self.store_count());
                Ok(Done::Nothing)
            }
            Op::Close { fd } if self.sockets.contains_key(fd) => {
                self.sockets.get_mut(fd).expect("closing socket").closed = true;
                Ok(Done::Nothing)
            }
            Op::Sync { .. } | Op::Remove { .. } | Op::Connect { .. } | Op::Shutdown { .. } | Op::Close { .. } => {
                Ok(Done::Nothing)
            }
            Op::Send { fd, bytes, from } => {
                let start = usize::try_from(*from).expect("send offset");
                self.sockets.get_mut(fd).expect("socket").request.extend_from_slice(&bytes[start..]);
                let count = bytes.len() - start;
                self.http_request(*fd);
                Ok(Done::Count(u32::try_from(count).expect("send count")))
            }
            Op::Recv { fd, buf } => {
                let reply = &mut self.sockets.get_mut(fd).expect("socket").reply;
                let count = buf.len().min(reply.len());
                for byte in &mut buf[..count] {
                    *byte = reply.pop_front().expect("response byte");
                }
                Ok(Done::Count(u32::try_from(count).expect("recv count")))
            }
            Op::Cancel { target } => self.cancel(*target),
            Op::PipeRead { buf, .. } => {
                let count = buf.len().min(self.input.len());
                buf[..count].copy_from_slice(&self.input[..count]);
                self.input = &self.input[count..];
                Ok(Done::Count(u32::try_from(count).expect("input count")))
            }
            Op::Bind { .. }
            | Op::Listen { .. }
            | Op::Accept { .. }
            | Op::Append { .. }
            | Op::Truncate { .. }
            | Op::MakeDirectory { .. }
            | Op::List { .. }
            | Op::Spawn { .. }
            | Op::Wait { .. }
            | Op::Signal { .. }
            | Op::Usage
            | Op::Window { .. }
            | Op::ReadSignal { .. } => panic!("unexpected kernel operation"),
        }
    }
    fn cancel(&mut self, target: skein_lib::Token) -> Result<Done, Error> {
        if let Some(index) = self.held.iter().position(|submit| submit.op == target) {
            let Submit { op, kind } = self.held.remove(index);
            let mut kind = kind;
            let result =
                if self.race_record_create && matches!(&kind, Op::Open { path, .. } if path.as_ref() == b"record") {
                    self.answer(&mut kind)
                } else {
                    Err(Error::Cancelled)
                };
            self.host.completions().push(Complete { op, kind, result });
            return Ok(Done::Nothing);
        }
        if self.signal.as_ref().is_none_or(|signal| signal.op != target) {
            return Err(Error::TooLate);
        }
        let Submit { op, kind } = self.signal.take().expect("signal stays in flight");
        self.host.completions().push(Complete { op, kind, result: Err(Error::Cancelled) });
        Ok(Done::Nothing)
    }
}

#[derive(Default)]
struct Socket {
    request: Vec<u8>,
    reply: VecDeque<u8>,
    answered: bool,
    eof: Option<()>,
    closed: bool,
}
pub type Engine = Frame;
pub type Disk = Frame;
impl Frame {
    #[expect(clippy::too_many_lines, reason = "one HTTP fake keeps the native routes and their answers together")]
    fn http_request(&mut self, fd: Fd) {
        let socket = self.sockets.get_mut(&fd).expect("socket");
        if socket.answered {
            return;
        }
        let Some(end) = socket.request.windows(4).position(|b| b == b"\r\n\r\n") else { return };
        let head = std::str::from_utf8(&socket.request[..end]).expect("HTTP head");
        let length = head
            .lines()
            .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
            .map_or(0, |number| number.parse::<usize>().expect("content length"));
        if socket.request.len() < end + 4 + length {
            return;
        }
        let path = head.split_whitespace().nth(1).expect("request path").as_bytes().to_vec();
        self.requests.push(path.clone().into_boxed_slice());
        if self.stopping.is_some() {
            return;
        }
        socket.answered = true;
        match path.as_slice() {
            b"/jig/v1/sign-in/link" => {
                self.link_posts.push(socket.request[end + 4..].into());
                self.link_bearers.push(head.lines().find_map(|line| {
                    line.strip_prefix("Authorization: Bearer ").map(|value| {
                        skein_lib::base64::decode(skein_lib::base64::Alphabet::Url, value.as_bytes(), 40)
                            .expect("native bearer encoding")
                    })
                }));
                let answer = if self.again_links > 0 {
                    self.again_links -= 1;
                    w::SignInAnswer::NotReady
                } else if self.busy_links > 0 {
                    self.busy_links -= 1;
                    w::SignInAnswer::Busy
                } else if let Some((sign_ins, earliest, link_until)) = self.full_link {
                    w::SignInAnswer::Full(
                        w::SignInsFull::new(
                            &w::CEILINGS,
                            w::SignInsFullParts {
                                sign_ins,
                                earliest: earliest.as_nanos(),
                                link_until: link_until.as_nanos(),
                            },
                        )
                        .expect("full answer"),
                    )
                } else if self.refuse_link.is_some() {
                    w::SignInAnswer::Refused
                } else {
                    w::SignInAnswer::SignedIn(
                        w::SignedIn::new(
                            &w::CEILINGS,
                            w::SignedInParts {
                                token: self.keep_link.is_none().then(|| skein_lib::Secret::new(TOKEN_BYTES.into())),
                                party: 1,
                                expires: u64::MAX,
                            },
                        )
                        .expect("signed-in answer"),
                    )
                };
                let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer size"));
                answer.encode(&mut writer).expect("answer fits");
                let body = writer.finish();
                socket.reply.extend(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).bytes());
                socket.reply.extend(body.iter().copied());
            }
            b"/jig/v1/stream" => self.hello(fd),
            b"/jig/v1/watch" => {
                let open = w::Open::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&socket.request[end + 4..]))
                    .expect("watch record");
                let subject = w::Subject::decode(&w::CEILINGS, &mut skein_lib::Reader::new(open.subject().record()))
                    .expect("watch subject");
                if subject == w::Subject::Me {
                    self.watch = Some(open.watch());
                } else {
                    self.wait_task = Some(open.watch());
                }
                let answer = w::OpenAnswer::Admitted;
                let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer size"));
                answer.encode(&mut writer).expect("answer fits");
                let body = writer.finish();
                socket.reply.extend(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).bytes());
                socket.reply.extend(body.iter().copied());
                if self.deliver.is_some() {
                    if subject == w::Subject::Me {
                        self.deliver_me();
                    } else if self.move_mode {
                        self.deliver_move_accounts(open.watch(), &subject);
                    } else {
                        self.deliver_task(false);
                    }
                }
            }
            b"/jig/v1/request" => {
                let body = socket.request[end + 4..].to_vec();
                self.act_reply(fd, &body);
            }
            b"/jig/v1/read" => self.read_task(fd),
            b"/jig/v1/sign-out" => {
                self.hold_stores = Some(());
                self.reply(fd, &encode!(w::SignOutAnswer::SignedOut));
            }
            b"/jig/v1/stream-gone" | b"/jig/v1/unwatch" => self.reply(fd, &[]),
            _ => panic!("unexpected HTTP route"),
        }
    }
    fn deliver_me(&mut self) {
        let value = w::DeliveryValue::Me(if self.offers.is_some() || self.move_mode {
            recovery::chat_me()
        } else {
            jig_client_test_records::frame_record()
        });
        let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("delivery size"));
        value.encode(&mut writer).expect("delivery fits");
        let document = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: writer.finish() },
        )
        .expect("owned delivery");
        let event = w::StreamEvent::Delivery(
            w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch: self.watch.expect("opened watch"), document })
                .expect("me delivery"),
        );
        self.sockets.get_mut(&self.stream.expect("open stream")).expect("stream socket").reply.extend(sse(&event));
    }
}
fn sse(event: &w::StreamEvent) -> Vec<u8> {
    use skein_lib::base64::{self, Alphabet};
    let mut writer = skein_lib::Writer::new(usize::try_from(event.measure()).expect("event size"));
    event.encode(&mut writer).expect("event fits");
    let bytes = writer.finish();
    let length =
        base64::encoded_len(Alphabet::Standard, u32::try_from(bytes.len()).expect("event size")).expect("base64 size");
    let mut writer = skein_lib::Writer::new(usize::try_from(length).expect("base64 size"));
    base64::encode(Alphabet::Standard, &bytes, &mut writer).expect("base64 fits");
    let mut frame = b"data: ".to_vec();
    frame.extend_from_slice(&writer.finish());
    frame.extend_from_slice(b"\n\n");
    frame
}
