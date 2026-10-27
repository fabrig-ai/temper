//! The box's held webhook listener, prepared before the service can restore.
use skein_io::kernel::{Complete, Done, Fd, Op, Submit};
use skein_lib::{Queue, Token};
use std::net::SocketAddr;

// Separate from the service's kernel routing tokens; only one setup op is issued at a time.
pub(crate) const OWNER: Token = Token::new(u64::MAX - 3);
pub(crate) const ADDRESS: SocketAddr =
    SocketAddr::V4(std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 41000));
// The held socket accepts nothing until claimed, so this backlog is never used by the story.
const BACKLOG: u32 = 1;
pub(crate) struct Listener {
    next: Option<Op>,
    socket: Option<Fd>,
}
impl Listener {
    pub(crate) fn new() -> Self {
        Self { next: Some(Op::Socket { family: skein_io::kernel::Family::of(&ADDRESS) }), socket: None }
    }
    pub(crate) fn ready(&self) -> bool {
        self.next.is_some()
    }
    pub(crate) fn submit(&mut self, into: &mut Queue<Submit>) {
        if let Some(kind) = self.next.take() {
            into.push(Submit { op: OWNER, kind });
        }
    }
    pub(crate) fn complete(&mut self, complete: Complete) -> Option<Fd> {
        match (complete.kind, complete.result.expect("the fake box opens its held listener")) {
            (Op::Socket { .. }, Done::Fd(fd)) => {
                self.socket = Some(fd);
                self.next = Some(Op::Bind { fd, addr: ADDRESS });
            }
            (Op::Bind { fd, .. }, Done::Bound(_)) => self.next = Some(Op::Listen { fd, backlog: BACKLOG }),
            (Op::Listen { .. }, Done::Nothing) => return self.socket.take(),
            other => panic!("held listener setup terminal: {other:?}"),
        }
        None
    }
}
