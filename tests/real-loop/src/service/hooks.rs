//! Port-0 listeners and completed HTTP status lines on hooks connections.
use skein_io::kernel::{Complete, Done, Fd, Op};
use std::{collections::BTreeMap, net::SocketAddr};
// Listeners, with room for each of the peer's four concurrent connections.
const SOCKETS: usize = 8;
const LINE: usize = 4096;
#[derive(Default)]
pub(super) struct Hooks {
    bound: BTreeMap<Fd, SocketAddr>,
    listening: Vec<(Fd, SocketAddr)>,
    connections: BTreeMap<Fd, Vec<u8>>,
}
impl Hooks {
    pub(super) fn complete(&mut self, complete: &Complete, hooks: Option<SocketAddr>, responses: &mut Vec<u16>) {
        match (&complete.kind, &complete.result) {
            (Op::Bind { fd, addr }, Ok(Done::Bound(bound))) if addr.port() == 0 => {
                assert!(self.bound.len() < SOCKETS, "bounded listeners");
                self.bound.insert(*fd, *bound);
            }
            (Op::Listen { fd, .. }, Ok(Done::Nothing)) => {
                if let Some(address) = self.bound.remove(fd) {
                    assert!(self.listening.len() < SOCKETS, "bounded listening sockets");
                    self.listening.push((*fd, address));
                }
            }
            (Op::Accept { fd: listener }, Ok(Done::Accepted { fd, .. }))
                if self.listening.iter().any(|(at, address)| at == listener && Some(*address) == hooks) =>
            {
                assert!(self.connections.len() < SOCKETS, "bounded hooks connections");
                self.connections.insert(*fd, Vec::new());
            }
            (Op::Send { fd, bytes, from }, Ok(Done::Count(count))) => {
                if let Some(line) = self.connections.get_mut(fd) {
                    line.extend_from_slice(&bytes[*from as usize..(*from + *count) as usize]);
                    assert!(line.len() <= LINE, "bounded HTTP status line");
                    if line.windows(2).any(|part| part == b"\r\n") {
                        let code = std::str::from_utf8(line)
                            .expect("HTTP status line")
                            .split_whitespace()
                            .nth(1)
                            .expect("status code")
                            .parse()
                            .expect("numeric status");
                        assert!(responses.len() < 1024, "bounded response observations");
                        responses.push(code);
                        self.connections.remove(fd);
                    }
                }
            }
            (Op::Close { fd }, Ok(Done::Nothing)) => {
                self.connections.remove(fd);
            }
            _ => {}
        }
    }
}
