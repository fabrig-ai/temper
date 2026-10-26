//! Client receipts decoded from the kernel's HTTP and SSE bytes.
pub(crate) mod cache;
use jig_client::contract as client;
use jig_client_protocol_link::translate;
use jig_client_protocol_wire::v19 as wire;
use skein_io::kernel::{Complete, Done, Op};
use skein_lib::{
    Reader,
    base64::{self, Alphabet},
};
use std::collections::BTreeMap;

pub(super) fn native_link_limits() -> jig_client_protocol_link::Limits {
    jig_client_protocol_link::Limits::new(
        temper_client_shell_terminal::limits::native().client.client.domain,
        temper_client_shell_terminal::limits::native().client.client.exchanges,
    )
    .expect("native bounds")
}

pub(crate) enum Received {
    SignedIn(u64),
    Answer { key: [u8; 16], answer: client::Answer },
    Delivery(client::Delivery),
    Read(client::ReadValue),
    ApplicationSent { key: [u8; 16] },
    Application { key: [u8; 16], answer: temper_client_domain::forge::Answer },
}
#[derive(Default)]
struct Connection {
    sent: Vec<u8>,
    incoming: Vec<u8>,
    remaining: Option<usize>,
    chunked: bool,
    header: bool,
    body: Vec<u8>,
    answered: bool,
    sent_application: Option<()>,
}
#[derive(Default)]
pub(crate) struct Wire {
    connections: BTreeMap<i32, Connection>,
    pub(crate) received: Vec<Received>,
}
fn mark(bytes: &[u8], marker: &[u8]) -> Option<usize> {
    bytes.windows(marker.len()).position(|window| window == marker)
}
impl Wire {
    pub(crate) fn complete(&mut self, complete: &Complete) {
        match (&complete.kind, &complete.result) {
            (Op::Send { fd, bytes, from }, Ok(Done::Count(count))) => {
                let connection = self.connections.entry(fd.raw()).or_default();
                connection.sent.extend_from_slice(&bytes[*from as usize..(*from + *count) as usize]);
                connection.request(&mut self.received);
            }
            (Op::Recv { fd, buf }, Ok(Done::Count(count))) => {
                let connection = self.connections.entry(fd.raw()).or_default();
                connection.incoming.extend_from_slice(&buf[..*count as usize]);
                connection.progress(&mut self.received);
            }
            (Op::Connect { .. }, Ok(Done::Fd(fd))) => {
                self.connections.insert(fd.raw(), Connection::default());
            }
            _ => {}
        }
    }
}
impl Connection {
    fn request(&mut self, received: &mut Vec<Received>) {
        if self.sent_application.is_some() {
            return;
        }
        let Some(end) = mark(&self.sent, b"\r\n\r\n") else { return };
        if self.sent.split(|byte| *byte == b' ').nth(1) != Some(b"/jig/v1/request".as_slice()) {
            return;
        }
        let header = std::str::from_utf8(&self.sent[..end]).expect("request header");
        let Some(length) = header
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim().parse::<usize>().expect("request length"))
        else {
            return;
        };
        if self.sent.len() < end + 4 + length {
            return;
        }
        let limits = native_link_limits();
        let keyed =
            wire::Keyed::decode(&limits.wire, &mut Reader::new(&self.sent[end + 4..])).expect("sent keyed request");
        self.sent_application = Some(());
        if matches!(keyed.owner(), wire::Owner::Family(_)) {
            received.push(Received::ApplicationSent { key: *keyed.key() });
        }
    }
    fn progress(&mut self, received: &mut Vec<Received>) {
        if !self.header {
            let Some(end) = mark(&self.incoming, b"\r\n\r\n") else { return };
            let header = std::str::from_utf8(&self.incoming[..end]).expect("HTTP response header");
            assert!(header.starts_with("HTTP/1.1 200 "), "client response status");
            for line in header.lines().skip(1) {
                let (name, value) = line.split_once(':').expect("HTTP header field");
                if name.eq_ignore_ascii_case("content-length") {
                    self.remaining = Some(value.trim().parse().expect("HTTP body length"));
                } else if name.eq_ignore_ascii_case("transfer-encoding") {
                    assert_eq!(value.trim(), "chunked", "HTTP streaming framing");
                    self.chunked = true;
                }
            }
            self.incoming.drain(..end + 4);
            self.header = true;
        }
        if self.chunked {
            while let Some(end) = mark(&self.incoming, b"\r\n") {
                let size = usize::from_str_radix(std::str::from_utf8(&self.incoming[..end]).expect("chunk size"), 16)
                    .expect("hex chunk size");
                if self.incoming.len() < end + size + 4 {
                    break;
                }
                self.body.extend_from_slice(&self.incoming[end + 2..end + 2 + size]);
                assert_eq!(&self.incoming[end + 2 + size..end + 4 + size], b"\r\n", "chunk ending");
                self.incoming.drain(..end + size + 4);
            }
            self.stream(received);
        } else if !self.answered && self.remaining.is_some_and(|size| self.incoming.len() >= size) {
            let size = self.remaining.expect("length-framed reply");
            self.answer(&self.incoming[..size], received);
            self.answered = true;
        }
    }
    fn answer(&self, bytes: &[u8], received: &mut Vec<Received>) {
        let end = mark(&self.sent, b"\r\n\r\n").expect("sent request header");
        let path = self.sent.split(|byte| *byte == b' ').nth(1).expect("request target");
        let limits = native_link_limits();
        match path {
            b"/jig/v1/sign-in/link" => {
                let answer =
                    wire::SignInAnswer::decode(&limits.wire, &mut Reader::new(bytes)).expect("sign-in reply bytes");
                // NotReady asks the native caller to retry after startup. Only an
                // actual SignedIn answer releases the referee's sign-in receipt.
                if matches!(answer, wire::SignInAnswer::NotReady) {
                    return;
                }
                let wire::SignInAnswer::SignedIn(value) = answer else { panic!("sign-in accepted: {answer:?}") };
                received.push(Received::SignedIn(value.party()));
            }
            b"/jig/v1/request" => {
                let keyed =
                    wire::Keyed::decode(&limits.wire, &mut Reader::new(&self.sent[end + 4..])).expect("request bytes");
                let value = wire::KeyedAnswer::decode(&limits.wire, &mut Reader::new(bytes)).expect("answer bytes");
                if matches!(&value, wire::KeyedAnswer::FinalAnswer(document) if document.owner() == &wire::Owner::Jig) {
                    let answer = translate::answers::answer(value, &limits).expect("jig answer document");
                    received.push(Received::Answer { key: *keyed.key(), answer });
                } else if let wire::KeyedAnswer::FinalAnswer(document) = value {
                    let wire::Owner::Family(family) = document.owner() else { panic!("application family owner") };
                    let record = jig_client::Family {
                        family: family.number(),
                        version: document.version(),
                        bytes: document.record().into(),
                    };
                    let bounds = temper_client_domain::forge::wire_limits(
                        &temper_client_shell_terminal::limits::native().client.forge,
                    )
                    .expect("family bounds");
                    let answer = temper_client_domain::forge::family::answer(&record, &bounds, &limits)
                        .expect("adoption answer");
                    received.push(Received::Application { key: *keyed.key(), answer });
                }
            }
            b"/jig/v1/read" => {
                if let wire::ReadAnswer::Value(value) =
                    wire::ReadAnswer::decode(&limits.wire, &mut Reader::new(bytes)).expect("read reply bytes")
                    && value.owner() == &wire::Owner::Jig
                {
                    received
                        .push(Received::Read(translate::reads::value(value.record(), &limits).expect("read document")));
                }
            }
            b"/jig/v1/watch" | b"/jig/v1/unwatch" | b"/jig/v1/stream-gone" => {}
            _ => panic!("unobserved client request target"),
        }
    }
    fn stream(&mut self, received: &mut Vec<Received>) {
        let limits = native_link_limits();
        while let Some(end) = mark(&self.body, b"\n\n") {
            let event: Vec<_> = self.body.drain(..end + 2).collect();
            let data: Vec<_> = event
                .split(|byte| *byte == b'\n')
                .filter_map(|line| line.strip_prefix(b"data: ").or_else(|| line.strip_prefix(b"data:")))
                .flatten()
                .copied()
                .collect();
            if data.is_empty() {
                continue;
            }
            let decoded = base64::decode(Alphabet::Standard, &data, 16_777_216).expect("SSE record encoding");
            if let wire::StreamEvent::Delivery(value) =
                wire::StreamEvent::decode(&wire::CEILINGS, &mut Reader::new(&decoded)).expect("SSE record bytes")
                && value.document().owner() == &wire::Owner::Jig
            {
                received.push(Received::Delivery(
                    translate::watches::delivery(value.document().record(), &limits).expect("delivery document"),
                ));
            }
        }
    }
}
