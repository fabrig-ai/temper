//! The person's bounded browser: redirects, the manifest form and its Create
//! link, on a fresh loopback connection per hop. The landing page needs no cookie.
// Adapted from jig's protocol-accounts world's private browser at the pinned revision.
const ROOM: u32 = 64;
fn limits() -> skein_oauth_accounts::Limits {
    jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small).driver
}
use skein_http::{Header, Method, client as http};
use skein_io::{self as io, kernel};
use skein_lib::stream::{Down, Read, Up};
use skein_lib::{Env, Queue, Time, Token, Wall, bytes};
use skein_world::Host;
use std::{fmt::Write as _, net::SocketAddr};

const WEB: Token = Token::new(1);
/// Room for a response body; the issuer's redirect has none.
const BODY: u32 = 4096;
const PAGE_BYTES: usize = 65_536;
// Manifest, create, conversion, installation and setup; OAuth needs three.
const HOPS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hop {
    pub method: String,
    pub path: String,
    pub status: u16,
}
/// Room for the URL handed over, the driver's `url_bytes`.
const URL: usize = 1024;

pub struct Visitor {
    io: io::Io,
    http: http::Client,
    events: Queue<io::Event>,
    requests: Queue<io::Request>,
    submissions: Queue<kernel::Submit>,
    completions: Queue<kernel::Complete>,
    answers: Queue<http::Event>,
    calls: Queue<http::Request>,
    below: Queue<Down>,
    socket: Option<Token>,
    connected: bool,
    /// The URL handed over, held in place until the visitor's own turn
    /// copies it, so its heap is the visitor's.
    inbox: [u8; URL],
    inbox_len: usize,
    status: Option<u16>,
    account_redirect: Option<Box<[u8]>>,
    last_redirect: Option<(u16, Box<[u8]>)>,
    hops: Vec<Hop>,
    target: String,
    authority: String,
    body: Vec<u8>,
    form: Option<Box<[u8]>>,
    posting: bool,
}
impl std::fmt::Debug for Visitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visitor").field("status", &self.status).finish_non_exhaustive()
    }
}
impl Visitor {
    #[must_use]
    pub fn new() -> Self {
        let limits = limits();
        Self {
            io: io::Io::new(&limits.io),
            http: http::Client::new(&limits.http),
            events: Queue::with_capacity(ROOM),
            requests: Queue::with_capacity(ROOM),
            submissions: Queue::with_capacity(ROOM),
            completions: Queue::with_capacity(ROOM),
            answers: Queue::with_capacity(ROOM),
            calls: Queue::with_capacity(ROOM),
            below: Queue::with_capacity(ROOM),
            socket: None,
            connected: false,
            inbox: [0; URL],
            inbox_len: 0,
            status: None,
            account_redirect: None,
            last_redirect: None,
            hops: Vec::new(),
            target: String::new(),
            authority: String::new(),
            body: Vec::new(),
            form: None,
            posting: false,
        }
    }
    /// Hands over `url` to visit, once: a plain `http://` URL with a
    /// literal address.
    pub(crate) fn hand(&mut self, url: &[u8]) {
        assert!(self.is_empty(), "previous visit drained");
        self.http = http::Client::new(&limits().http);
        self.last_redirect = None;
        self.status = None;
        self.hops.clear();
        self.inbox.get_mut(..url.len()).expect("the URL fits the inbox").copy_from_slice(url);
        self.inbox_len = url.len();
    }
    fn visit(&mut self, url: &[u8]) {
        let rest = url.strip_prefix(b"http://").expect("a plain HTTP URL");
        let at = rest.iter().position(|byte| *byte == b'/').expect("a path");
        let (authority, target) = rest.split_at(at);
        let address: SocketAddr = std::str::from_utf8(authority).expect("ascii").parse().expect("a literal address");
        assert!(address.ip().is_loopback(), "visitor stays on the box's loopback peers");
        std::str::from_utf8(target).expect("target text").clone_into(&mut self.target);
        std::str::from_utf8(authority).expect("authority text").clone_into(&mut self.authority);
        self.body.clear();
        self.posting = self.form.is_some();
        let mut headers = vec![Header { name: bytes::copy_of(b"host"), value: bytes::copy_of(authority) }];
        if self.posting {
            headers.push(Header {
                name: bytes::copy_of(b"content-type"),
                value: bytes::copy_of(b"application/x-www-form-urlencoded"),
            });
        }
        self.calls.push(http::Request::Call(http::Call {
            method: if self.posting { Method::Post } else { Method::Get },
            target: bytes::copy_of(target),
            headers: headers.into_boxed_slice(),
            body: self
                .form
                .as_ref()
                .map_or(http::Body::None, |form| http::Body::Length(u64::try_from(form.len()).expect("bounded form"))),
            close: true,
        }));
        if let Some(form) = &self.form {
            self.calls.push(http::Request::Upload(Down::Demand {
                read: Read::Nothing,
                room: u32::try_from(form.len()).expect("bounded form"),
            }));
        }
        self.requests.push(io::Request::Connect { owner: WEB, addr: io::Addr::Ip(address) });
    }
    /// The first response's status, once its exchange is done.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.hops.first().map(|hop| hop.status)
    }
    /// The last redirect's status and Location, retained after following it.
    #[must_use]
    pub fn last_redirect(&self) -> Option<(u16, &[u8])> {
        self.last_redirect.as_ref().map(|(status, location)| (*status, location.as_ref()))
    }
    #[must_use]
    pub fn hops(&self) -> &[Hop] {
        &self.hops
    }
    fn page(&mut self) {
        if self.account_redirect.is_some() || self.status != Some(200) {
            return;
        }
        let html = std::str::from_utf8(&self.body).expect("page UTF-8");
        if self.target.contains("/github/manifest?") {
            let form = html.split_once("<form ").expect("manifest form").1;
            let action = attribute(form.split_once('>').expect("form tag").0, "action");
            let input =
                html.split("<input ").skip(1).find(|tag| tag.contains("name=\"manifest\"")).expect("manifest input");
            let manifest = attribute(input.split_once('>').expect("input tag").0, "value");
            let mut form = String::from("manifest=");
            for byte in manifest.bytes() {
                write!(form, "%{byte:02X}").expect("form byte");
            }
            assert!(form.len() <= PAGE_BYTES, "bounded manifest form");
            self.form = Some(form.into_bytes().into_boxed_slice());
            self.account_redirect = Some(action.into_bytes().into_boxed_slice());
        } else if self.posting {
            // The fake's Create link stands in for GitHub's confirmation button.
            let link = html.split("<a ").skip(1).find(|tag| tag.contains("id=\"create\"")).expect("Create link");
            self.account_redirect =
                Some(attribute(link.split_once('>').expect("link tag").0, "href").into_bytes().into_boxed_slice());
        }
    }
    fn progress(&mut self, now: Time, wall: Wall) {
        let env = Env { now, wall, limits: limits().http };
        let Some(socket) = self.socket.filter(|_| self.connected) else { return };
        while let Some(event) = self.answers.pop() {
            match event {
                http::Event::Response(response) => {
                    let location = response
                        .headers
                        .iter()
                        .find(|header| header.name.eq_ignore_ascii_case(b"location"))
                        .map(|header| header.value.clone());
                    self.status = Some(response.status);
                    assert!(self.hops.len() < HOPS, "bounded visit chain");
                    self.hops.push(Hop {
                        method: if self.posting { "POST" } else { "GET" }.into(),
                        path: self.target.clone(),
                        status: response.status,
                    });
                    eprintln!("visit {} {} -> {}", self.hops.last().expect("hop").method, self.target, response.status);
                    if matches!(response.status, 302 | 303) {
                        self.account_redirect = location.map(|url| {
                            if url.starts_with(b"/") {
                                format!(
                                    "http://{}{}",
                                    self.authority,
                                    std::str::from_utf8(&url).expect("Location text")
                                )
                                .into_bytes()
                                .into_boxed_slice()
                            } else {
                                url
                            }
                        });
                        assert!(self.account_redirect.is_some(), "account sign-in redirect");
                        self.last_redirect =
                            Some((response.status, self.account_redirect.as_ref().expect("redirect Location").clone()));
                    }
                    self.calls.push(http::Request::Body(Down::Demand { read: Read::Arrived { max: BODY }, room: 0 }));
                }
                http::Event::Body(Up::Bytes(bytes)) => {
                    self.body.extend_from_slice(&bytes);
                    assert!(self.body.len() <= PAGE_BYTES, "bounded page");
                    self.calls.push(http::Request::Body(Down::Demand { read: Read::Arrived { max: BODY }, room: 0 }));
                }
                http::Event::Body(Up::End) | http::Event::Closed => {}
                http::Event::Upload(Up::Room) => {
                    self.calls.push(http::Request::Upload(Down::Send(self.form.take().expect("posted form"))));
                    self.calls.push(http::Request::Upload(Down::Finish));
                }
                http::Event::Done(_) => {
                    self.page();
                    self.calls.push(http::Request::Close);
                    self.requests.push(io::Request::Close { entity: socket });
                }
                http::Event::Failed(_)
                | http::Event::Upload(Up::Bytes(_) | Up::End | Up::Failed(_))
                | http::Event::Body(Up::Room | Up::Failed(_)) => panic!("the visit failed: {event:?}"),
            }
        }
        while self.answers.room() >= 2
            && self.below.room() >= 2
            && let Some(call) = self.calls.pop()
        {
            http::down(&mut self.http, &env, call, &mut self.answers, &mut self.below);
        }
        while let Some(down) = self.below.pop() {
            self.requests.push(io::Request::Stream { stream: socket, down });
        }
    }
}
impl Default for Visitor {
    fn default() -> Self {
        Self::new()
    }
}
impl Host for Visitor {
    fn iterate(&mut self, now: Time, wall: Wall) {
        let limits = limits();
        let env = Env { now, wall, limits: limits.io };
        if self.inbox_len > 0 {
            let url = bytes::copy_of(&self.inbox[..self.inbox_len]);
            self.inbox_len = 0;
            self.visit(&url);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 || !self.io.is_ready() {
                break;
            }
            io::resume(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            let Some(complete) = self.completions.pop() else { break };
            io::up(&mut self.io, &env, complete, &mut self.events, &mut self.submissions);
        }
        for _ in 0..ROOM {
            if self.events.room() < 3 || self.submissions.room() < 2 || !self.io.is_due(now) {
                break;
            }
            io::fire(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        while let Some(event) = self.events.pop() {
            match event {
                io::Event::Connecting { socket, .. } => self.socket = Some(socket),
                io::Event::Connected { .. } => self.connected = true,
                io::Event::Stream { up, .. } => http::up(
                    &mut self.http,
                    &Env { now, wall, limits: limits.http },
                    up,
                    &mut self.answers,
                    &mut self.below,
                ),
                io::Event::Closed { .. } => {
                    self.socket = None;
                    self.connected = false;
                    if let Some(url) = self.account_redirect.take() {
                        self.http = http::Client::new(&limits.http);
                        self.visit(&url);
                    }
                }
                other @ (io::Event::Listening { .. }
                | io::Event::Accepted { .. }
                | io::Event::Output { .. }
                | io::Event::Spawned { .. }
                | io::Event::Exited { .. }
                | io::Event::Usage { .. }
                | io::Event::Resized
                | io::Event::Window { .. }
                | io::Event::Shutdown { .. }
                | io::Event::Failed { .. }) => panic!("visitor event {other:?}"),
            }
        }
        self.progress(now, wall);
        for _ in 0..ROOM {
            if !self.io.takes() || self.submissions.room() < 2 {
                break;
            }
            let Some(request) = self.requests.pop() else { break };
            io::down(&mut self.io, &env, request, &mut self.submissions);
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<kernel::Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<kernel::Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _: Time) -> bool {
        self.io.is_ready()
            || self.inbox_len > 0
            || !self.events.is_empty()
            || !self.requests.is_empty()
            || !self.completions.is_empty()
            || (self.connected && (!self.answers.is_empty() || !self.calls.is_empty() || !self.below.is_empty()))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.io.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.io.is_empty()
            && self.inbox_len == 0
            && self.events.is_empty()
            && self.requests.is_empty()
            && self.submissions.is_empty()
            && self.completions.is_empty()
            && self.answers.is_empty()
            && self.calls.is_empty()
            && self.below.is_empty()
            && self.socket.is_none()
    }
    fn worst_case(&self) -> u64 {
        let limits = limits();
        io::worst_case(&limits.io).expect("io bound") + 512 * 1024
    }
    fn operations(&self) -> u32 {
        io::operations(&limits().io).expect("io operation bound")
    }
}

fn attribute(tag: &str, name: &str) -> String {
    let marker = format!("{name}=\"");
    let value = tag.split_once(&marker).expect("HTML attribute").1.split_once('"').expect("attribute end").0;
    value.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}
