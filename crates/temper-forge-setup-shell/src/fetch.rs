//! The pinned fetch's host: one TLS connection and one body chunk at a time.
use skein_http::client as http;
use skein_io::{
    Io,
    kernel::{Complete, Submit},
};
use skein_lib::{
    Env, Queue, Token,
    sha256::Sha256,
    stream::{Down, Read, Up},
};
use skein_shell::{Clock, Config, Kernel, Trust, Wait};
use skein_tls::client as tls;
use std::{
    fs,
    io::{self, Read as _, Write as _},
    net::ToSocketAddrs,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
};
use temper_forge_setup::{
    fetch::{self, Event, Failure, Fetch, Request},
    setup::Origin,
};

const CHUNK: u32 = 16_384;
const HTTP: http::Limits = http::Limits { request: 16_384, head: 16_384, headers: 64, read: CHUNK, send: CHUNK };
const TLS: tls::Limits = tls::Limits { read: 16_384, send: 16_384, records: 131_072 };
const OWNER: Token = Token::new(1);
/// A machine refusal, a file error, or a transport failure naming the whole URL.
#[derive(Debug)]
pub enum Failed {
    Refused(Failure),
    File { file: Box<[u8]>, why: io::ErrorKind },
    Roots(skein_shell::RootsRefusal),
    Trust(skein_tls::Refusal),
    Transport { url: Box<[u8]>, why: String },
}
impl Failed {
    /// Diagnostic words following the command's forge prefix.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Self::Refused(why) => String::from_utf8_lossy(&why.words()).into_owned(),
            Self::File { file, why } => format!("{}: {why}", String::from_utf8_lossy(file)),
            Self::Roots(why) => match why {
                skein_shell::RootsRefusal::Unread { path, why } => format!("{}: roots: {why:?}", path.display()),
                other @ (skein_shell::RootsRefusal::NoBundle
                | skein_shell::RootsRefusal::TooMany { .. }
                | skein_shell::RootsRefusal::TooLong { .. }) => format!("roots: {other:?}"),
            },
            Self::Trust(why) => format!("roots: {why:?}"),
            Self::Transport { url, why } => format!("{}: {why}", String::from_utf8_lossy(url)),
        }
    }
}
fn file_error(file: &[u8], why: &io::Error) -> Failed {
    Failed::File { file: file.into(), why: why.kind() }
}
fn hash(file: &[u8]) -> Result<Option<[u8; 32]>, Failed> {
    let mut input = match fs::File::open(crate::path(file)) {
        Ok(file) => file,
        Err(why) if why.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(why) => return Err(file_error(file, &why)),
    };
    let mut hash = Sha256::new();
    let mut buffer = [0; CHUNK as usize];
    loop {
        let count = input.read(&mut buffer).map_err(|why| file_error(file, &why))?;
        if count == 0 {
            return Ok(Some(hash.finish()));
        }
        hash.update(&buffer[..count]);
    }
}
fn open(part: &[u8]) -> Result<fs::File, Failed> {
    let path = crate::path(part);
    let parent = path.parent().expect("cache path has a parent");
    fs::DirBuilder::new().recursive(true).mode(0o700).create(parent).map_err(|why| file_error(part, &why))?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|why| file_error(part, &why))?;
    file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|why| file_error(part, &why))?;
    Ok(file)
}
fn finish(handle: &fs::File, part: &[u8], file: &[u8]) -> Result<(), Failed> {
    handle.set_permissions(fs::Permissions::from_mode(0o755)).map_err(|why| file_error(part, &why))?;
    handle.sync_all().map_err(|why| file_error(part, &why))?;
    fs::rename(crate::path(part), crate::path(file)).map_err(|why| file_error(file, &why))?;
    fs::File::open(crate::path(file).parent().expect("cache parent"))
        .and_then(|dir| dir.sync_all())
        .map_err(|why| file_error(file, &why))
}
/// Runs the machine; an interrupted body leaves its part to be truncated next time.
pub fn run(limits: &skein_io::Limits, trust: &Trust, mut machine: Fetch) -> Result<(), Failed> {
    let roots = skein_shell::read_trust_roots(
        trust,
        &skein_shell::RootBounds { bundle_bytes: 1 << 20, certificates: 1024, certificate_bytes: 65_536 },
    )
    .map_err(Failed::Roots)?;
    let (config, _) = skein_tls::Config::from_der(&roots.certificates, &[b"http/1.1"]).map_err(Failed::Trust)?;
    let mut connection: Option<Connection> = None;
    let mut part_file: Option<fs::File> = None;
    let mut part_path: Box<[u8]> = Box::new([]);
    let mut event = Event::Begin;
    let mut out = Queue::with_capacity(fetch::MAX_OUT);
    let result = loop {
        fetch::step(&mut machine, event, &mut out);
        let answer = match out.pop().expect("one fetch request") {
            Request::Hash { file } => hash(&file).map(Event::Hashed),
            Request::Get { origin, path } => {
                if let Some(mut old) = connection.take() {
                    old.close();
                }
                match Connection::new(limits, &config, &origin, &path) {
                    Ok(next) => {
                        connection = Some(next);
                        connection.as_mut().expect("new connection").answer()
                    }
                    Err(why) => Err(why),
                }
            }
            Request::Open { part } => open(&part).map(|file| {
                part_file = Some(file);
                part_path = part;
                Event::Done
            }),
            Request::Read => {
                let connection = connection.as_mut().expect("body follows Get");
                connection.demand();
                connection.answer()
            }
            Request::Write { offset, bytes } => {
                use std::io::{Seek, SeekFrom};
                let file = part_file.as_mut().expect("write follows Open");
                file.seek(SeekFrom::Start(offset))
                    .and_then(|_| file.write_all(&bytes))
                    .map(|()| Event::Done)
                    .map_err(|why| file_error(&part_path, &why))
            }
            Request::Finish { part, file } => {
                let handle = part_file.take().expect("finish follows Open");
                finish(&handle, &part, &file).map(|()| Event::Done)
            }
            Request::Remove { part } => {
                part_file = None;
                fs::remove_file(crate::path(&part)).map(|()| Event::Done).map_err(|why| file_error(&part, &why))
            }
            Request::End(result) => break result.map_err(Failed::Refused),
        };
        match answer {
            Ok(next) => event = next,
            Err(why) => break Err(why),
        }
    };
    if let Some(mut connection) = connection {
        connection.close();
    }
    result
}
struct Connection {
    io: Io,
    limits: skein_io::Limits,
    kernel: Kernel,
    clock: Clock,
    socket: Option<Token>,
    closed: bool,
    tls: tls::Client,
    http: http::Client,
    tls_up: Queue<tls::Event>,
    cipher: Queue<Down>,
    http_up: Queue<http::Event>,
    plain: Queue<Down>,
    events: Queue<skein_io::Event>,
    complete: Queue<Complete>,
    subs: Queue<Submit>,
    call: Option<http::Call>,
    result: Option<Result<Event, String>>,
    url: Box<[u8]>,
}
impl Connection {
    fn new(
        limits: &skein_io::Limits,
        config: &skein_tls::Config,
        origin: &Origin,
        path: &[u8],
    ) -> Result<Self, Failed> {
        let url = fetch::url(origin, path);
        let fail = |why| Failed::Transport { url: url.clone(), why };
        if limits.intake < tls::LARGEST_READ || limits.output < tls::largest_room(&TLS) {
            return Err(fail("io capacity below TLS's largest demand".into()));
        }
        let host = std::str::from_utf8(&origin.host).map_err(|why| fail(why.to_string()))?;
        let host = host.strip_prefix('[').and_then(|host| host.strip_suffix(']')).unwrap_or(host);
        let name = skein_tls::Name::new(host).ok_or_else(|| fail("invalid TLS server name".into()))?;
        let address = (host, origin.port)
            .to_socket_addrs()
            .map_err(|why| fail(format!("connect: {why}")))?
            .next()
            .ok_or_else(|| fail("connect: no address".into()))?;
        let operations = skein_io::operations(limits).expect("usable io limits");
        let kernel = Kernel::open(Config { operations }).map_err(|why| fail(format!("connect kernel: {why:?}")))?;
        let mut authority = origin.host.to_vec();
        if origin.port != 443 {
            authority.extend_from_slice(format!(":{}", origin.port).as_bytes());
        }
        let mut connection = Self {
            io: Io::new(limits),
            limits: *limits,
            kernel,
            clock: Clock::new(),
            socket: None,
            closed: false,
            tls: tls::Client::new(config, name, &TLS),
            http: http::Client::new(&HTTP),
            tls_up: Queue::with_capacity(8),
            cipher: Queue::with_capacity(8),
            http_up: Queue::with_capacity(8),
            plain: Queue::with_capacity(8),
            events: Queue::uncapped(),
            complete: Queue::with_capacity(operations),
            subs: Queue::with_capacity(operations),
            call: Some(http::Call {
                method: skein_http::Method::Get,
                target: path.into(),
                headers: Box::new([
                    skein_http::Header { name: b"Host".as_slice().into(), value: authority.into_boxed_slice() },
                    skein_http::Header { name: b"User-Agent".as_slice().into(), value: b"temper".as_slice().into() },
                ]),
                body: http::Body::None,
                close: true,
            }),
            result: None,
            url,
        };
        connection.io_down(skein_io::Request::Connect { owner: OWNER, addr: skein_io::Addr::Ip(address) });
        Ok(connection)
    }
    fn io_down(&mut self, request: skein_io::Request) {
        let now = self.clock.now();
        skein_io::down(
            &mut self.io,
            &Env { now: now.now, wall: now.wall, limits: self.limits },
            request,
            &mut self.subs,
        );
    }
    fn demand(&mut self) {
        let now = self.clock.now();
        http::down(
            &mut self.http,
            &Env { now: now.now, wall: now.wall, limits: HTTP },
            http::Request::Body(Down::Demand { read: Read::Arrived { max: CHUNK }, room: 0 }),
            &mut self.http_up,
            &mut self.plain,
        );
    }
    fn answer(&mut self) -> Result<Event, Failed> {
        loop {
            self.poll_io(false);
            while self.result.is_none() && self.pump() {}
            self.io.reclaim();
            if let Some(result) = self.result.take() {
                if matches!(result, Ok(Event::Ended)) {
                    self.close();
                }
                return result.map_err(|why| Failed::Transport { url: self.url.clone(), why });
            }
            self.wait();
        }
    }
    fn poll_io(&mut self, closing: bool) {
        self.kernel.reap(&mut self.complete);
        let now = self.clock.now();
        let env = Env { now: now.now, wall: now.wall, limits: self.limits };
        loop {
            if self.io.is_ready() {
                skein_io::resume(&mut self.io, &env, &mut self.events, &mut self.subs);
            } else if let Some(complete) = self.complete.pop() {
                skein_io::up(&mut self.io, &env, complete, &mut self.events, &mut self.subs);
            } else if self.io.is_due(now.now) {
                skein_io::fire(&mut self.io, &env, &mut self.events, &mut self.subs);
            } else {
                break;
            }
            while let Some(event) = self.events.pop() {
                match event {
                    skein_io::Event::Connecting { socket, .. } => self.socket = Some(socket),
                    skein_io::Event::Connected { .. } if !closing => tls::down(
                        &mut self.tls,
                        &Env { now: now.now, wall: now.wall, limits: TLS },
                        tls::Request::Handshake,
                        &mut self.tls_up,
                        &mut self.cipher,
                    ),
                    skein_io::Event::Stream { up, .. } if !closing => tls::up(
                        &mut self.tls,
                        &Env { now: now.now, wall: now.wall, limits: TLS },
                        up,
                        &mut self.tls_up,
                        &mut self.cipher,
                    ),
                    skein_io::Event::Closed { .. } => {
                        self.closed = true;
                        self.socket = None;
                        if !closing && self.result.is_none() {
                            self.result = Some(Err("connection closed before the HTTP answer".into()));
                        }
                    }
                    skein_io::Event::Failed { error: why, .. } if !closing => {
                        self.result = Some(Err(format!("connect: {why:?}")));
                    }
                    skein_io::Event::Connected { .. }
                    | skein_io::Event::Stream { .. }
                    | skein_io::Event::Failed { .. } => {}
                    skein_io::Event::Listening { .. }
                    | skein_io::Event::Accepted { .. }
                    | skein_io::Event::Output { .. }
                    | skein_io::Event::Spawned { .. }
                    | skein_io::Event::Exited { .. }
                    | skein_io::Event::Usage { .. }
                    | skein_io::Event::Resized
                    | skein_io::Event::Window { .. }
                    | skein_io::Event::Shutdown { .. } => unreachable!("socket operations only"),
                }
            }
        }
    }
    fn pump(&mut self) -> bool {
        let now = self.clock.now();
        let http_env = Env { now: now.now, wall: now.wall, limits: HTTP };
        let tls_env = Env { now: now.now, wall: now.wall, limits: TLS };
        if let Some(event) = self.http_up.pop() {
            match event {
                http::Event::Response(response) => {
                    self.result = Some(Ok(Event::Answered {
                        status: response.status,
                        length: match response.framing {
                            http::Framing::Length(n) => Some(n),
                            http::Framing::Empty => Some(0),
                            http::Framing::Chunked | http::Framing::UntilEnd => None,
                        },
                    }));
                }
                http::Event::Body(Up::Bytes(bytes)) => self.result = Some(Ok(Event::Chunk(bytes))),
                http::Event::Done(_) => self.result = Some(Ok(Event::Ended)),
                http::Event::Body(Up::End) | http::Event::Closed => {}
                http::Event::Failed(why) => self.result = Some(Err(format!("HTTP: {why:?}"))),
                http::Event::Body(Up::Failed(why)) => self.result = Some(Err(format!("HTTP body: {why:?}"))),
                http::Event::Upload(_) | http::Event::Body(Up::Room) => unreachable!("GET has no upload"),
            }
        } else if let Some(event) = self.tls_up.pop() {
            match event {
                tls::Event::Ready(_) => {
                    let call = self.call.take().expect("one HTTP call");
                    http::down(
                        &mut self.http,
                        &http_env,
                        http::Request::Call(call),
                        &mut self.http_up,
                        &mut self.plain,
                    );
                }
                tls::Event::Stream(up) => http::up(&mut self.http, &http_env, up, &mut self.http_up, &mut self.plain),
                tls::Event::Failed(why) => self.result = Some(Err(format!("TLS: {why:?}"))),
                tls::Event::Closed => self.result = Some(Err("TLS closed before the HTTP answer".into())),
            }
        } else if let Some(down) = self.cipher.pop() {
            self.io_down(skein_io::Request::Stream { stream: self.socket.expect("connected socket"), down });
        } else if let Some(down) = self.plain.pop() {
            tls::down(&mut self.tls, &tls_env, tls::Request::Stream(down), &mut self.tls_up, &mut self.cipher);
        } else {
            return false;
        }
        true
    }
    fn wait(&mut self) {
        let wait = if self.io.is_ready() || !self.complete.is_empty() {
            Wait::No
        } else {
            self.io.next_deadline().map_or(Wait::Forever, Wait::Until)
        };
        self.kernel.submit(&mut self.subs, wait);
    }
    fn close(&mut self) {
        if let Some(socket) = self.socket {
            self.io_down(skein_io::Request::Abort { entity: socket });
        }
        loop {
            self.poll_io(true);
            self.io.reclaim();
            if self.io.is_empty() && self.subs.is_empty() && self.kernel.in_flight() == 0 {
                break;
            }
            self.wait();
        }
    }
}
