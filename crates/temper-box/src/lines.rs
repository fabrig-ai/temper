//! Bounded line pieces, held until a sink takes them.
use crate::host::LINE_BYTES;
use alloc::boxed::Box;
use skein_lib::{List, Wall, Writer};
/// Whether a child's pipe goes to stdout unchanged.
#[must_use]
pub fn route(plain: &[u8], step: &[u8], child_fd: u32) -> bool {
    child_fd == 1 && plain == step
}
/// A completed terminal line or a whole refused serve line, with the wall time
/// of its LF/EOF (or diagnostic cap), even when admission delays extraction.
#[derive(PartialEq, Eq, Debug)]
pub enum Output {
    /// The raw diagnostic piece, before the terminal prefix and added LF.
    Piece {
        bytes: Box<[u8]>,
        wall: Wall,
    },
    Line {
        bytes: Box<[u8]>,
        wall: Wall,
    },
    Refused {
        wall: Wall,
    },
    /// An incomplete held legacy first piece remains raw terminal output.
    Legacy {
        bytes: Box<[u8]>,
    },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handoff {
    None,
    Waiting,
    Incomplete,
}
#[derive(Debug)]
pub struct Lines {
    plain: bool,
    handoff: Handoff,
    chunk: Box<[u8]>,
    position: usize,
    piece: List<u8>,
    ended: bool,
    discarding: bool,
    chunk_wall: Wall,
    end_wall: Wall,
}
impl Lines {
    #[must_use]
    pub fn new(plain: bool) -> Self {
        Self {
            plain,
            handoff: Handoff::None,
            chunk: Box::new([]),
            position: 0,
            piece: List::with_capacity(LINE_BYTES),
            ended: false,
            discarding: false,
            chunk_wall: Wall::EPOCH,
            end_wall: Wall::EPOCH,
        }
    }
    /// Keep the next complete first line apart from ordinary output.
    pub fn hold(&mut self) {
        self.handoff = Handoff::Waiting;
    }
    /// Consumes at most one bounded chunk; EOF releases an incomplete line.
    pub fn first(&mut self) -> Option<Box<[u8]>> {
        if self.handoff != Handoff::Waiting {
            return None;
        }
        if self.fill_piece() {
            self.handoff = Handoff::None;
            let first = skein_lib::bytes::copy_of(self.piece.as_slice());
            self.piece.clear();
            return Some(first);
        }
        if self.ended {
            self.handoff = Handoff::None;
            if self.plain {
                self.handoff = Handoff::Incomplete;
            }
        }
        None
    }
    pub fn push(&mut self, bytes: Box<[u8]>, wall: Wall) {
        assert!(self.chunk.is_empty(), "one chunk per pipe");
        assert!(bytes.len() <= usize::try_from(LINE_BYTES).expect("line cap fits"), "bounded child chunk");
        self.chunk = bytes;
        self.chunk_wall = wall;
    }
    pub fn end(&mut self, wall: Wall) {
        if !self.ended {
            self.end_wall = wall;
        }
        self.ended = true;
    }
    #[must_use]
    pub fn wants_read(&self) -> bool {
        self.chunk.is_empty() && !self.ended
    }
    #[must_use]
    pub fn ready(&self) -> bool {
        !self.chunk.is_empty() || self.ended && (!self.piece.is_empty() || self.discarding)
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunk.is_empty() && self.piece.is_empty() && !self.discarding
    }
    /// Reserve terminal room before calling: even an incomplete result advances
    /// bounded assembly/discard state. No result waits in an unpriced queue.
    pub fn take(&mut self, _name: &[u8]) -> Option<Output> {
        if self.handoff == Handoff::Waiting {
            return None;
        }
        if self.handoff == Handoff::Incomplete && !self.piece.is_empty() {
            let bytes = skein_lib::bytes::copy_of(self.piece.as_slice());
            self.piece.clear();
            return Some(Output::Legacy { bytes });
        }
        if self.plain {
            return self.url();
        }
        let wall = self.chunk_wall;
        let complete = self.fill_piece();
        if !complete && (!self.ended || self.piece.is_empty()) {
            return None;
        }
        let wall = if complete { wall } else { self.end_wall };
        let bytes = skein_lib::bytes::copy_of(self.piece.as_slice());
        self.piece.clear();
        Some(Output::Piece { bytes, wall })
    }
    fn url(&mut self) -> Option<Output> {
        let wall = self.chunk_wall;
        for _ in 0..self.chunk.len() {
            let Some(byte) = self.chunk.get(self.position).copied() else { break };
            self.position = self.position.checked_add(1).expect("chunk position");
            if byte == b'\n' {
                let refused = self.discarding;
                self.discarding = false;
                let result = if refused {
                    Output::Refused { wall }
                } else {
                    Output::Line { bytes: joined(&[self.piece.as_slice(), b"\n"]), wall }
                };
                self.piece.clear();
                self.release_chunk();
                return Some(result);
            }
            if !self.discarding {
                if self.piece.room() == 0 {
                    self.discarding = true;
                    self.piece.clear();
                } else {
                    self.piece.push(byte).expect("payload cap checked");
                }
            }
        }
        self.release_chunk();
        if self.ended {
            if self.discarding {
                self.discarding = false;
                return Some(Output::Refused { wall: self.end_wall });
            }
            if !self.piece.is_empty() {
                let bytes = joined(&[self.piece.as_slice(), b"\n"]);
                self.piece.clear();
                return Some(Output::Line { bytes, wall: self.end_wall });
            }
        }
        None
    }
    fn release_chunk(&mut self) {
        if self.position == self.chunk.len() {
            self.chunk = Box::new([]);
            self.position = 0;
        }
    }
    #[cfg(test)]
    pub(crate) fn bounded(&self) -> bool {
        self.chunk.len() <= usize::try_from(LINE_BYTES).unwrap()
            && self.piece.len() <= LINE_BYTES
            && self.position <= self.chunk.len()
    }
    fn fill_piece(&mut self) -> bool {
        let mut complete = false;
        for byte in self.chunk.iter().skip(self.position) {
            self.piece.push(*byte).expect("line cap checked after each byte");
            self.position = self.position.saturating_add(1);
            if *byte == b'\n' || self.piece.room() == 0 {
                complete = true;
                break;
            }
        }
        if self.position == self.chunk.len() {
            self.chunk = Box::new([]);
            self.position = 0;
        }
        complete
    }
}
pub(crate) fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).expect("bounded output");
    }
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).expect("computed output length");
    }
    writer.finish()
}
