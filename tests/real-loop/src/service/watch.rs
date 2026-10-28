//! A task delivery sent on a connection accepted after the watch cue.
use jig_client_protocol_wire::v19 as wire;
use skein_io::kernel::{Complete, Done, Op};
use skein_lib::{
    Reader,
    base64::{self, Alphabet},
};
use std::collections::BTreeMap;
// The cue observes one small-profile task, including its HTTP and SSE framing.
const BYTES: usize = 65_536;
const CONNECTIONS: usize = 8;
#[derive(Default)]
pub(super) struct Watch {
    pub(super) call: Option<usize>,
    pub(super) armed: bool,
    pub(super) delivered: bool,
    connections: BTreeMap<i32, Connection>,
}
impl Watch {
    pub(super) fn complete(&mut self, complete: &Complete) {
        if !self.armed || self.delivered {
            return;
        }
        match (&complete.kind, &complete.result) {
            (Op::Accept { .. }, Ok(Done::Accepted { fd, .. })) => {
                assert!(self.connections.len() < CONNECTIONS, "bounded watched connections");
                self.connections.insert(fd.raw(), Connection::default());
            }
            (Op::Send { fd, bytes, from }, Ok(Done::Count(count))) => {
                if let Some(connection) = self.connections.get_mut(&fd.raw()) {
                    connection.incoming.extend_from_slice(&bytes[*from as usize..(*from + *count) as usize]);
                    assert!(connection.incoming.len() <= BYTES, "bounded watched send");
                    self.delivered = connection.progress();
                    if self.delivered {
                        self.connections.clear();
                    }
                }
            }
            (Op::Close { fd }, Ok(Done::Nothing)) => {
                self.connections.remove(&fd.raw());
            }
            _ => {}
        }
    }
}
#[derive(Default)]
struct Connection {
    incoming: Vec<u8>,
    header: bool,
    chunked: bool,
    body: Vec<u8>,
}
fn mark(bytes: &[u8], marker: &[u8]) -> Option<usize> {
    bytes.windows(marker.len()).position(|window| window == marker)
}
impl Connection {
    fn progress(&mut self) -> bool {
        if !self.header {
            let Some(end) = mark(&self.incoming, b"\r\n\r\n") else { return false };
            let header = std::str::from_utf8(&self.incoming[..end]).expect("service response header");
            self.chunked = header
                .lines()
                .filter_map(|line| line.split_once(':'))
                .any(|(name, value)| name.eq_ignore_ascii_case("transfer-encoding") && value.trim() == "chunked");
            self.incoming.drain(..end + 4);
            self.header = true;
        }
        if !self.chunked {
            self.incoming.clear();
            return false;
        }
        while let Some(end) = mark(&self.incoming, b"\r\n") {
            let size = usize::from_str_radix(std::str::from_utf8(&self.incoming[..end]).expect("chunk size"), 16)
                .expect("hex chunk size");
            if self.incoming.len() < end + size + 4 {
                break;
            }
            self.body.extend_from_slice(&self.incoming[end + 2..end + 2 + size]);
            assert!(self.body.len() <= BYTES, "bounded watched event");
            assert_eq!(&self.incoming[end + 2 + size..end + 4 + size], b"\r\n", "chunk ending");
            self.incoming.drain(..end + size + 4);
        }
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
            let decoded = base64::decode(Alphabet::Standard, &data, u32::try_from(BYTES).expect("event bound"))
                .expect("SSE record encoding");
            if let wire::StreamEvent::Delivery(value) =
                wire::StreamEvent::decode(&wire::CEILINGS, &mut Reader::new(&decoded)).expect("SSE record")
                && value.document().owner() == &wire::Owner::Jig
                && matches!(
                    wire::DeliveryValue::decode(&wire::CEILINGS, &mut Reader::new(value.document().record()))
                        .expect("delivery document"),
                    wire::DeliveryValue::Task(_)
                )
            {
                return true;
            }
        }
        false
    }
}
