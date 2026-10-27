//! The browser's bundle, read whole at start, and its closed list of types (client/browser.md, 2.2; skein's
//! ui-web.md, 4.3 and 4.4). Copied from jig's testing/jig-test-service/src/server/bundle.rs.
use super::Clients;
use alloc::boxed::Box;
use skein_http::{Header, Method, server as http};
use skein_lib::{
    List, Map, Token, bytes,
    stream::{Down, Read, Up},
};

/// Temper's fixed explanation when configuration names no browser bundle (client/browser.md, 2.2).
#[expect(clippy::disallowed_macros, reason = "a fixed page compiled in, not data read at run time")]
pub const NO_BUNDLE_PAGE: &[u8] = include_bytes!("no-bundle.html");

/// Digest-directory files are cached permanently (client/browser.md, 2.2).
pub(crate) const FILE_CACHE_CONTROL: &[u8] = b"public, max-age=31536000, immutable";
/// The page is revalidated on each visit (client/browser.md, 2.2).
pub(crate) const PAGE_CACHE_CONTROL: &[u8] = b"no-cache";

/// A bundle path's bound (jig's testing/jig-test/src/bundle.rs).
pub const NAME_BYTES: u32 = 1024;
/// The digest directory's name: sixteen lowercase hex digits.
const DIGEST_BYTES: u64 = 16;
/// The page's `ETag`: a quoted FNV-1a of sixteen hex digits.
const ETAG_BYTES: u64 = 18;

/// One digest-directory file, typed by the closed list (skein's ui-web.md, 4.4).
#[derive(Debug)]
pub struct File {
    pub kind: &'static [u8],
    pub bytes: Box<[u8]>,
}
/// The bundle read whole at start (client/browser.md, 2.2).
#[derive(Debug)]
pub struct Bundle {
    pub page: Box<[u8]>,
    pub etag: Box<[u8]>,
    pub digest: Box<[u8]>,
    pub files: Map<Box<[u8]>, File>,
}
/// A path's type by its extension, from the closed list; `None` off it.
#[must_use]
pub fn kind(path: &[u8]) -> Option<&'static [u8]> {
    let mut dot = None;
    for (at, byte) in path.iter().enumerate() {
        if *byte == b'.' {
            dot = Some(at);
        }
        if *byte == b'/' {
            dot = None;
        }
    }
    let dot = dot?;
    match path.get(dot.checked_add(1)?..)? {
        b"html" => Some(b"text/html; charset=utf-8"),
        b"js" => Some(b"text/javascript"),
        b"wasm" => Some(b"application/wasm"),
        b"css" => Some(b"text/css"),
        b"svg" => Some(b"image/svg+xml"),
        b"png" => Some(b"image/png"),
        b"webp" => Some(b"image/webp"),
        b"ico" => Some(b"image/x-icon"),
        b"woff2" => Some(b"font/woff2"),
        b"txt" => Some(b"text/plain; charset=utf-8"),
        b"webmanifest" => Some(b"application/manifest+json"),
        _ => None,
    }
}
/// The bundle's bytes at its limits: its files (the page among them), their names, the digest and the `ETag`.
#[must_use]
pub fn worst_case(limits: &crate::limits::Clients) -> Option<u64> {
    u64::from(limits.bundle_bytes)
        // Filling retains the old page, measured forge metas, and the new page.
        .checked_mul(3)?
        .checked_add(64)?
        .checked_add(Map::<Box<[u8]>, File>::worst_case(limits.bundle_files)?)?
        .checked_add(u64::from(limits.bundle_files).checked_mul(u64::from(NAME_BYTES))?)?
        .checked_add(DIGEST_BYTES)?
        .checked_add(ETAG_BYTES)?
        .checked_add(u64::try_from(size_of::<Bundle>()).ok()?)
}

/// How the clients' listener answers a call the mapping does not claim (client/browser.md, 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    File,
    Page,
    NoBundle,
    NotModified,
    Root,
    NotFound,
    NotAllowed,
    Busy,
}
/// An answer on its way: what it is, the path it serves and how much of its body is sent.
#[derive(Debug)]
pub(crate) struct Reply {
    answer: Answer,
    path: Box<[u8]>,
    offset: usize,
}
fn target(head: &http::Call) -> &[u8] {
    match bytes::find(&head.target, b"?") {
        Some(at) => head.target.get(..at).expect("query position"),
        None => &head.target,
    }
}
impl Bundle {
    fn file(&self, path: &[u8]) -> Option<&File> {
        let name = if path == b"/robots.txt" {
            b"files/robots.txt".as_slice()
        } else {
            let rest = path.strip_prefix(b"/bundle/")?;
            let slash = bytes::find(rest, b"/")?;
            if rest.get(..slash)? != self.digest.as_ref() {
                return None;
            }
            rest.get(slash.checked_add(1)?..)?
        };
        for (path, file) in &self.files {
            if path.as_ref() == name {
                return Some(file);
            }
        }
        None
    }
    fn content(&self, reply: &Reply) -> &[u8] {
        match reply.answer {
            Answer::File | Answer::Root => &self.file(&reply.path).expect("routed file").bytes,
            Answer::Page => &self.page,
            Answer::NoBundle => NO_BUNDLE_PAGE,
            Answer::NotModified | Answer::NotFound | Answer::NotAllowed | Answer::Busy => b"",
        }
    }
}
/// An entity tag without its weakness mark and the blanks around it.
fn strong(tag: &[u8]) -> &[u8] {
    let tag = tag.trim_ascii();
    tag.strip_prefix(b"W/").unwrap_or(tag)
}
/// Whether an `If-None-Match` field holds the page's tag: `*`, or a comma-separated list with a member equal to it,
/// compared weakly (RFC 9110, 13.1.2).
fn revalidates(field: &[u8], etag: &[u8]) -> bool {
    if field.trim_ascii() == b"*" {
        return true;
    }
    let mut rest = field;
    loop {
        let (member, next) = match bytes::find(rest, b",") {
            Some(at) => (rest.get(..at).expect("comma position"), rest.get(at.saturating_add(1)..)),
            None => (rest, None),
        };
        if strong(member) == strong(etag) {
            return true;
        }
        match next {
            Some(next) => rest = next,
            None => return false,
        }
    }
}
/// The answer to a call the mapping does not claim, in the design's order.
pub(crate) fn route(bundle: Option<&Bundle>, head: &http::Call) -> Answer {
    let Some(bundle) = bundle else {
        return if head.method == Method::Get && !target(head).starts_with(b"/bundle/") && target(head) != b"/robots.txt"
        {
            Answer::NoBundle
        } else {
            Answer::NotFound
        };
    };
    if head.method != Method::Get {
        return Answer::NotAllowed;
    }
    let path = target(head);
    if path.starts_with(b"/bundle/") {
        return if bundle.file(path).is_some() { Answer::File } else { Answer::NotFound };
    }
    if path == b"/robots.txt" && bundle.file(path).is_none() {
        return Answer::NotFound;
    }
    if let Some(field) = head.header(b"If-None-Match")
        && revalidates(field, &bundle.etag)
    {
        return Answer::NotModified;
    }
    if path == b"/robots.txt" { Answer::Root } else { Answer::Page }
}
fn field(headers: &mut List<Header>, name: &[u8], value: &[u8]) {
    headers.push(Header { name: bytes::copy_of(name), value: bytes::copy_of(value) }).expect("answer fields");
}
impl Clients {
    /// Answers a call the mapping does not claim, and holds the answer until it is sent.
    pub(super) fn serve_call(&mut self, call: Token, head: &http::Call, busy: bool) {
        let answer = if busy { Answer::Busy } else { route(self.bundle.as_ref(), head) };
        let reply = Reply { answer, path: bytes::copy_of(target(head)), offset: 0 };
        let mut headers = List::with_capacity(3);
        let mut length = 0;
        let status = match answer {
            Answer::Page | Answer::File | Answer::Root => {
                let bundle = self.bundle.as_ref().expect("routed bundle");
                let kind = match answer {
                    Answer::File | Answer::Root => bundle.file(&reply.path).expect("routed file").kind,
                    Answer::Page => b"text/html; charset=utf-8",
                    Answer::NoBundle => unreachable!("handled without a bundle"),
                    Answer::NotModified | Answer::NotFound | Answer::NotAllowed | Answer::Busy => {
                        unreachable!("content answer")
                    }
                };
                field(&mut headers, b"Content-Type", kind);
                length = bundle.content(&reply).len();
                200
            }
            Answer::NoBundle => {
                field(&mut headers, b"Content-Type", b"text/html; charset=utf-8");
                length = NO_BUNDLE_PAGE.len();
                200
            }
            Answer::NotModified => 304,
            Answer::NotFound => 404,
            Answer::NotAllowed => 405,
            Answer::Busy => 503,
        };
        match answer {
            Answer::File => field(&mut headers, b"Cache-Control", FILE_CACHE_CONTROL),
            Answer::Page | Answer::Root | Answer::NotModified => {
                field(&mut headers, b"Cache-Control", PAGE_CACHE_CONTROL);
                field(&mut headers, b"ETag", &self.bundle.as_ref().expect("revalidated bundle").etag);
            }
            Answer::NoBundle => field(&mut headers, b"Cache-Control", PAGE_CACHE_CONTROL),
            Answer::NotFound | Answer::NotAllowed | Answer::Busy => {}
        }
        let response = http::Response {
            status,
            headers: headers.into_boxed(),
            body: if status == 304 || busy {
                http::Body::None
            } else {
                http::Body::Length(u64::try_from(length).expect("bounded file"))
            },
            close: busy,
        };
        self.server.answer(call, http::Request::Discard, &mut self.server_out);
        self.server.answer(call, http::Request::Respond(response), &mut self.server_out);
        if length != 0 {
            self.demand(call, length);
        }
        self.replies.insert(call, reply).expect("bounded server calls");
    }
    /// Takes one event of a call being answered: room for a chunk, or the answer's end.
    pub(super) fn serve_event(&mut self, call: Token, event: http::Event) {
        if self.replies.get(&call).expect("own reply").answer == Answer::Busy {
            match event {
                http::Event::Done(_) | http::Event::Failed(_) | http::Event::Closed => {
                    self.replies.remove(&call);
                    self.server.done(call, &mut self.server_out);
                }
                http::Event::Ended | http::Event::Refused(_) | http::Event::Reply(_) | http::Event::Body(_) => {}
                http::Event::Call(_) => unreachable!("a head is routed, not an event"),
            }
            return;
        }
        match event {
            http::Event::Reply(Up::Room) => self.chunk(call),
            http::Event::Done(_)
            | http::Event::Ended
            | http::Event::Refused(_)
            | http::Event::Failed(_)
            | http::Event::Closed
            | http::Event::Reply(Up::Failed(_))
            | http::Event::Body(Up::Failed(_)) => {
                self.replies.remove(&call);
                self.server.done(call, &mut self.server_out);
            }
            http::Event::Body(_) | http::Event::Reply(_) => {}
            http::Event::Call(_) => unreachable!("a head is routed, not an event"),
        }
    }
    fn demand(&mut self, call: Token, left: usize) {
        let room = self.send.min(u32::try_from(left).expect("bounded file"));
        let demand = http::Request::Reply(Down::Demand { read: Read::Nothing, room });
        self.server.answer(call, demand, &mut self.server_out);
    }
    fn chunk(&mut self, call: Token) {
        let reply = self.replies.get_mut(&call).expect("own reply");
        let body = match reply.answer {
            Answer::NoBundle => NO_BUNDLE_PAGE,
            Answer::File
            | Answer::Page
            | Answer::Root
            | Answer::NotModified
            | Answer::NotFound
            | Answer::NotAllowed
            | Answer::Busy => self.bundle.as_ref().expect("body's bundle").content(reply),
        };
        let end = reply.offset.saturating_add(usize::try_from(self.send).expect("send fits")).min(body.len());
        let bytes = bytes::copy_of(body.get(reply.offset..end).expect("reply slice"));
        let left = body.len().checked_sub(end).expect("reply cursor");
        reply.offset = end;
        self.server.answer(call, http::Request::Reply(Down::Send(bytes)), &mut self.server_out);
        if left == 0 {
            self.server.answer(call, http::Request::Reply(Down::Finish), &mut self.server_out);
        } else {
            self.demand(call, left);
        }
    }
}

impl Bundle {
    /// Fill the page's sign-in ways before computing its entity tag (protocol/sign-in.md, 7).
    pub fn fill_sign_in(&mut self, google: bool) -> Option<()> {
        let meta = if google {
            b"<meta name=\"temper-sign-in\" content=\"google\">".as_slice()
        } else {
            b"<meta name=\"temper-sign-in\" content=\"\">".as_slice()
        };
        self.insert_meta(meta)
    }
    fn insert_meta(&mut self, meta: &[u8]) -> Option<()> {
        const HEX: &[u8] = b"0123456789abcdef";
        let at = bytes::find(&self.page, b"</head>")?;
        let mut out = skein_lib::Writer::new(self.page.len().checked_add(meta.len())?);
        out.put(self.page.get(..at)?).ok()?;
        out.put(meta).ok()?;
        out.put(self.page.get(at..)?).ok()?;
        self.page = out.finish();
        // The bundle reader uses FNV-1a, quoted as sixteen lower-case hex digits.
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in &self.page {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        let mut tag = skein_lib::Writer::new(18);
        tag.put(b"\"").ok()?;
        for shift in (0_u32..16).rev() {
            let nibble = usize::try_from((hash >> shift.checked_mul(4)?) & 15).ok()?;
            tag.put(core::slice::from_ref(HEX.get(nibble)?)).ok()?;
        }
        tag.put(b"\"").ok()?;
        self.etag = tag.finish();
        Some(())
    }
}

fn attribute(byte: &u8) -> &[u8] {
    match byte {
        b'&' => b"&amp;",
        b'"' => b"&quot;",
        b'\'' => b"&#39;",
        b'<' => b"&lt;",
        b'>' => b"&gt;",
        _ => core::slice::from_ref(byte),
    }
}
fn meta_put(writer: &mut skein_lib::Writer, length: &mut usize, measuring: bool, value: &[u8]) -> Option<()> {
    *length = length.checked_add(value.len())?;
    if !measuring {
        writer.put(value).ok()?;
    }
    Some(())
}
fn forge_metas(
    sign_in: &crate::startup::SignIn,
    forges: &[crate::startup::Forge],
    setup: &temper_forge_setup::setup::Setup,
    writer: &mut skein_lib::Writer,
    measuring: bool,
) -> Option<usize> {
    let mut length = 0_usize;
    for row in &sign_in.forges {
        let mut forge = None;
        for configured in forges {
            if configured.number == row.forge {
                forge = Some(configured);
                break;
            }
        }
        let forge = forge?;
        let mut mode = b"other".as_slice();
        let mut public = None;
        for entry in &setup.entries {
            if entry.forge == row.forge
                && let temper_forge_setup::setup::Mode::Owned(owned) = &entry.mode
            {
                mode = b"owned";
                public = owned.public.as_ref();
                break;
            }
        }
        meta_put(writer, &mut length, measuring, b"<meta name=\"temper-forge-sign-in\" content=\"")?;
        let number = skein_lib::Decimal::of(u64::from(row.forge));
        meta_put(writer, &mut length, measuring, number.as_bytes())?;
        let origin = crate::startup::application::forge_origin(&forge.git, public);
        for value in [&row.client, &origin] {
            meta_put(writer, &mut length, measuring, b" ")?;
            for byte in value {
                meta_put(writer, &mut length, measuring, attribute(byte))?;
            }
        }
        meta_put(writer, &mut length, measuring, b" ")?;
        meta_put(writer, &mut length, measuring, mode)?;
        meta_put(writer, &mut length, measuring, b"\">")?;
    }
    Some(length)
}
impl Bundle {
    /// Publish checked forge sign-in facts beside the page's sign-in ways.
    pub fn fill_forge_sign_in(
        &mut self,
        sign_in: Option<&crate::startup::SignIn>,
        forges: &[crate::startup::Forge],
        setup: &temper_forge_setup::setup::Setup,
        limit: u32,
    ) -> Option<()> {
        let Some(sign_in) = sign_in else { return Some(()) };
        let length = forge_metas(sign_in, forges, setup, &mut skein_lib::Writer::new(0), true)?;
        if length == 0 {
            return Some(());
        }
        let mut total = self.page.len().checked_add(length)?;
        for (_, file) in &self.files {
            total = total.checked_add(file.bytes.len())?;
        }
        if total > usize::try_from(limit).ok()? {
            return None;
        }
        let mut out = skein_lib::Writer::new(length);
        assert_eq!(forge_metas(sign_in, forges, setup, &mut out, false), Some(length), "measured forge metas");
        self.insert_meta(&out.finish())
    }
}
