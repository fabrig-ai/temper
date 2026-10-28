//! A bounded browser on the caller's loop submissions; only local fake HTTP.
use skein_io::kernel::{Complete, Done, Family, Op};
use std::{fmt::Write as _, net::SocketAddr};

const RESPONSE: usize = 65_536;
const HOPS: usize = 5;

#[derive(Debug)]
pub struct Browsed {
    pub call: usize,
    pub hops: Vec<u16>,
    pub body: Vec<u8>,
    /// The converted App's slug, taken from the redirect's installation URL.
    pub app_slug: Option<String>,
}

#[derive(Clone, Copy)]
enum Page {
    Installation,
    Manifest,
    Create,
    Convert,
    Install,
    Setup,
}

pub(super) struct Browser {
    pub(super) seen: Browsed,
    address: SocketAddr,
    target: String,
    bytes: Vec<u8>,
    next: Option<String>,
    form: Option<String>,
    page: Page,
    pub(super) done: bool,
}
impl Browser {
    pub(super) fn new(call: usize, stdout: &[u8]) -> Self {
        let output = std::str::from_utf8(stdout).expect("browser stdout");
        let mut lines = output.lines();
        lines
            .find(|line| line.ends_with("Open this address in a browser on the engine's machine:"))
            .expect("ctl's browser instruction");
        let url = lines.next().expect("ctl's browser address");
        let (address, target) = local(url);
        let page = if target.starts_with("/forges/") && target.contains("/github/manifest?") {
            Page::Manifest
        } else {
            Page::Installation
        };
        Self {
            seen: Browsed { call, hops: Vec::with_capacity(HOPS), body: Vec::new(), app_slug: None },
            address,
            target,
            bytes: Vec::with_capacity(RESPONSE),
            next: None,
            form: None,
            page,
            done: false,
        }
    }
    pub(super) fn start(&self) -> Op {
        Op::Socket { family: Family::of(&self.address) }
    }
    pub(super) fn complete(&mut self, complete: Complete) -> Option<Op> {
        match (complete.kind, complete.result.expect("browser operation")) {
            (Op::Socket { .. }, Done::Fd(fd)) => Some(Op::Connect { fd, addr: self.address }),
            (Op::Connect { fd, .. }, Done::Nothing) => {
                let request = if let Some(form) = self.form.take() {
                    format!(
                        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{form}",
                        self.target,
                        self.address,
                        form.len()
                    )
                } else {
                    format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", self.target, self.address)
                };
                Some(Op::Send { fd, bytes: request.into_bytes().into_boxed_slice(), from: 0 })
            }
            (Op::Send { fd, bytes, from }, Done::Count(count)) => {
                assert!(count > 0, "browser send progresses");
                let from = from + count;
                Some(if from as usize == bytes.len() {
                    Op::Recv { fd, buf: vec![0; 4096].into_boxed_slice() }
                } else {
                    Op::Send { fd, bytes, from }
                })
            }
            (Op::Recv { fd, buf }, Done::Count(count)) => {
                self.bytes.extend_from_slice(&buf[..count as usize]);
                assert!(self.bytes.len() <= RESPONSE, "bounded browser response");
                if self.response() {
                    Some(Op::Close { fd })
                } else {
                    assert!(count > 0, "browser response ended before its Content-Length");
                    Some(Op::Recv { fd, buf })
                }
            }
            (Op::Close { .. }, Done::Nothing) => {
                if let Some(url) = self.next.take() {
                    (self.address, self.target) = local(&url);
                    self.bytes.clear();
                    Some(self.start())
                } else {
                    self.done = true;
                    None
                }
            }
            (op, done) => panic!("browser completion {op:?}: {done:?}"),
        }
    }
    fn response(&mut self) -> bool {
        let Some(end) = self.bytes.windows(4).position(|part| part == b"\r\n\r\n") else { return false };
        let header = std::str::from_utf8(&self.bytes[..end]).expect("HTTP headers");
        let mut lines = header.split("\r\n");
        let status: u16 = lines
            .next()
            .expect("HTTP status")
            .split_whitespace()
            .nth(1)
            .expect("status code")
            .parse()
            .expect("numeric status");
        let mut length = None;
        let mut location = None;
        for line in lines {
            let (name, value) = line.split_once(':').expect("HTTP header");
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<usize>().expect("Content-Length"));
            } else if name.eq_ignore_ascii_case("location") {
                location = Some(value.trim().to_owned());
            }
        }
        let length = length.expect("browser peers frame with Content-Length");
        assert!(end + 4 + length <= RESPONSE, "bounded framed response");
        if self.bytes.len() < end + 4 + length {
            return false;
        }
        assert!(self.seen.hops.len() < HOPS, "bounded browser hops");
        self.seen.hops.push(status);
        let body = &self.bytes[end + 4..end + 4 + length];
        let html = std::str::from_utf8(body).expect("browser page UTF-8");
        match self.page {
            Page::Manifest => {
                assert_eq!(status, 200, "manifest page");
                let mut forms = html.split("<form ").skip(1);
                let form = forms.next().expect("one manifest form");
                assert!(forms.next().is_none(), "only one manifest form");
                self.next = Some(attribute(form.split_once('>').expect("form tag").0, "action"));
                let input = html
                    .split("<input ")
                    .skip(1)
                    .map(|part| part.split_once('>').expect("input tag").0)
                    .find(|tag| tag.contains("name=\"manifest\""))
                    .expect("manifest input");
                assert_eq!(attribute(input, "type"), "hidden");
                let manifest = attribute(input, "value");
                let mut form = String::from("manifest=");
                for byte in manifest.bytes() {
                    write!(form, "%{byte:02X}").expect("form byte");
                }
                assert!(form.len() <= RESPONSE, "bounded posted manifest");
                self.form = Some(form);
                self.page = Page::Create;
            }
            Page::Create => {
                assert_eq!(status, 200, "fake manifest form page");
                let link = html
                    .split("<a ")
                    .skip(1)
                    .map(|part| part.split_once('>').expect("link tag").0)
                    .find(|tag| tag.contains("id=\"create\""))
                    .expect("fake's create link");
                self.next = Some(attribute(link, "href"));
                self.page = Page::Convert;
            }
            Page::Convert => {
                assert_eq!(status, 302, "converted App's installation redirect");
                let mut url = location.expect("installation Location");
                let (_, target) = local(&url);
                let slug = target
                    .strip_prefix("/apps/")
                    .expect("App installation")
                    .split_once("/installations/new?")
                    .expect("installation state")
                    .0;
                self.seen.app_slug = Some(slug.to_owned());
                url.push_str("&account=acme&repositories=all");
                self.next = Some(url);
                self.page = Page::Install;
            }
            Page::Install => {
                assert_eq!(status, 302, "fake installation redirects to setup");
                self.next = location;
                assert!(self.next.is_some(), "setup Location");
                self.page = Page::Setup;
            }
            Page::Installation if matches!(status, 302 | 303) => {
                let location = location.expect("redirect Location");
                self.next = Some(if location.starts_with('/') {
                    format!("http://{}{location}", self.address)
                } else {
                    location
                });
            }
            Page::Installation | Page::Setup => {
                self.seen.body = body.to_vec();
            }
        }
        true
    }
}
fn attribute(tag: &str, name: &str) -> String {
    let marker = format!("{name}=\"");
    let value = tag.split_once(&marker).expect("HTML attribute").1.split_once('"').expect("attribute end").0;
    // Ampersand comes last, so an escaped entity's text is not decoded twice.
    value.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}
fn local(url: &str) -> (SocketAddr, String) {
    let rest = url.strip_prefix("http://").expect("fake browser uses plaintext HTTP");
    let (authority, target) = rest.split_once('/').expect("browser URL path");
    let address: SocketAddr = authority.parse().expect("browser's numeric address");
    assert!(address.ip().is_loopback(), "browser stays on the local fake and engine");
    (address, format!("/{target}"))
}
