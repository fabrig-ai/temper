//! One queued line is one write grant; bytes from different pipes stay apart.
use crate::terminal::{Item, Target};
use alloc::boxed::Box;
use skein_io::Request;
use skein_lib::{
    Queue, Token,
    stream::{Down, Read, Up},
};

/// One FIFO and one acknowledged item across both terminal descriptors. The
/// active item remains counted after its payload moves into io.
#[derive(Debug)]
pub struct Terminal {
    tokens: [Token; 2],
    queued: Queue<Item>,
    active: Option<Target>,
    phase: Phase,
    closing: [bool; 2],
    closed: [bool; 2],
}
impl Terminal {
    #[must_use]
    pub fn new(stdout: Token, stderr: Token, entries: u32) -> Self {
        Self {
            tokens: [stdout, stderr],
            queued: Queue::with_capacity(entries),
            active: None,
            phase: Phase::Idle,
            closing: [false; 2],
            closed: [false; 2],
        }
    }
    #[must_use]
    pub fn owns(&self, token: Token) -> bool {
        self.tokens.contains(&token)
    }
    #[must_use]
    pub fn failed(&self) -> bool {
        self.phase == Phase::Failed
    }
    #[must_use]
    pub fn takes(&self, entries: u32) -> bool {
        !self.failed()
            && self.closing == [false; 2]
            && self.queued.room()
                >= entries
                    .checked_add(u32::from(self.active.is_some() && self.phase == Phase::Flushing))
                    .expect("active slot")
    }
    pub fn push(&mut self, item: Item) {
        assert!(self.takes(1), "terminal admission precedes construction");
        self.queued.push(item);
    }
    /// Atomic block admission; failure leaves the caller owning every item.
    pub fn block(&mut self, items: skein_lib::List<Item>) -> Result<(), skein_lib::List<Item>> {
        if !self.takes(items.len()) {
            return Err(items);
        }
        for item in items.into_boxed() {
            self.queued.push(item);
        }
        Ok(())
    }
    /// A length grant sends once, then asks for the full configured output
    /// budget. Only that second Room releases the active slot.
    pub fn up(&mut self, token: Token, up: Up, output: u32, out: &mut Queue<Request>) -> bool {
        match up {
            Up::Room => {
                if self.active.is_some() && token == self.token(self.active.expect("active target")) {
                    if self.phase == Phase::Waiting {
                        let item = self.queued.pop().expect("demanded front item");
                        self.active = Some(item.target);
                        out.push(Request::Stream { stream: token, down: Down::Send(item.bytes) });
                        out.push(Request::Stream {
                            stream: token,
                            down: Down::Demand { read: Read::Nothing, room: output },
                        });
                        self.phase = Phase::Flushing;
                    } else if self.phase == Phase::Flushing {
                        self.active = None;
                        self.phase = Phase::Idle;
                    }
                }
            }
            Up::Failed(_) => return self.fail(),
            Up::Bytes(_) | Up::End => {}
        }
        false
    }
    fn fail(&mut self) -> bool {
        if self.failed() {
            return false;
        }
        self.phase = Phase::Failed;
        self.active = None;
        // Explicit failed-output teardown is the sole dropping path.
        for _ in 0..self.queued.len() {
            drop(self.queued.pop());
        }
        true
    }
    pub fn closed(&mut self, token: Token) -> bool {
        let mut unexpected = false;
        for (i, target) in self.tokens.iter().enumerate() {
            if *target == token {
                unexpected = !*self.closing.get(i).expect("two descriptors");
                *self.closed.get_mut(i).expect("two descriptors") = true;
            }
        }
        unexpected && self.fail()
    }
    pub fn tick(&mut self, finish: bool, out: &mut Queue<Request>) {
        if self.phase == Phase::Idle
            && self.active.is_none()
            && let Some(item) = self.queued.iter().next()
        {
            let target = item.target;
            let room = u32::try_from(item.bytes.len()).expect("bounded terminal item");
            out.push(Request::Stream { stream: self.token(target), down: Down::Demand { read: Read::Nothing, room } });
            self.active = Some(target);
            self.phase = Phase::Waiting;
            return;
        }
        if self.failed() || finish && self.active.is_none() && self.queued.is_empty() {
            for (i, token) in self.tokens.iter().enumerate() {
                if !*self.closing.get(i).expect("two descriptors") && !*self.closed.get(i).expect("two descriptors") {
                    out.push(Request::Close { entity: *token });
                    *self.closing.get_mut(i).expect("two descriptors") = true;
                }
            }
        }
    }
    #[must_use]
    pub fn pending(&self, finish: bool) -> bool {
        if self.phase == Phase::Idle && self.active.is_none() && !self.queued.is_empty() {
            return true;
        }
        if self.failed() || finish && self.active.is_none() && self.queued.is_empty() {
            for (closing, closed) in self.closing.iter().zip(self.closed) {
                if !*closing && !closed {
                    return true;
                }
            }
        }
        false
    }
    fn token(&self, target: Target) -> Token {
        *self
            .tokens
            .get(match target {
                Target::Stdout => 0,
                Target::Stderr => 1,
            })
            .expect("two targets")
    }
    #[cfg(test)]
    pub(crate) fn bounded(&self, bytes: u32) -> bool {
        let active = u32::from(self.active.is_some() && self.phase == Phase::Flushing);
        let entries = self.queued.len().checked_add(active).expect("bounded entries");
        if entries > self.queued.capacity() {
            return false;
        }
        for item in &self.queued {
            if item.bytes.len() > usize::try_from(bytes).unwrap() {
                return false;
            }
        }
        true
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    Waiting,
    Flushing,
    Failed,
    Closing,
    Closed,
}
#[derive(Debug)]
pub struct Sink {
    pub(crate) token: Token,
    queued: Queue<Box<[u8]>>,
    phase: Phase,
    dirty: bool,
}
impl Sink {
    #[must_use]
    pub fn new(token: Token, entries: u32) -> Self {
        Self { token, queued: Queue::with_capacity(entries), phase: Phase::Idle, dirty: false }
    }
    #[must_use]
    pub fn room(&self) -> bool {
        self.phase == Phase::Idle || self.phase == Phase::Waiting || self.phase == Phase::Flushing
    }
    #[must_use]
    pub fn takes(&self) -> bool {
        self.room() && self.queued.room() > 0
    }
    pub fn push(&mut self, bytes: Box<[u8]>) -> Result<(), Box<[u8]>> {
        if !self.room() {
            return Err(bytes);
        }
        self.queued.try_push(bytes)
    }
    /// Reports a broken output once.
    pub fn up(&mut self, up: Up, out: &mut Queue<Request>) -> bool {
        match up {
            Up::Room => {
                if self.phase == Phase::Waiting {
                    let bytes = self.queued.pop().expect("demanded front line");
                    out.push(Request::Stream { stream: self.token, down: Down::Send(bytes) });
                    self.dirty = true;
                    self.phase = Phase::Idle;
                } else if self.phase == Phase::Flushing {
                    self.dirty = false;
                    self.phase = Phase::Idle;
                }
            }
            Up::Failed(_) => {
                if self.phase != Phase::Failed && self.phase != Phase::Closing && self.phase != Phase::Closed {
                    self.phase = Phase::Failed;
                    for _ in 0..self.queued.len() {
                        drop(self.queued.pop());
                    }
                    return true;
                }
            }
            Up::Bytes(_) | Up::End => {}
        }
        false
    }
    pub fn tick(&mut self, finish: bool, out: &mut Queue<Request>) {
        if self.phase == Phase::Idle {
            if let Some(front) = self.queued.iter().next() {
                let room = u32::try_from(front.len()).expect("bounded line");
                out.push(Request::Stream { stream: self.token, down: Down::Demand { read: Read::Nothing, room } });
                self.phase = Phase::Waiting;
            } else if finish {
                out.push(Request::Close { entity: self.token });
                self.phase = Phase::Closing;
            }
        } else if self.phase == Phase::Failed {
            out.push(Request::Close { entity: self.token });
            self.phase = Phase::Closing;
        }
    }
    pub fn closed(&mut self) {
        self.phase = Phase::Closed;
    }
    /// Whether io's terminal Closed has been consumed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.phase == Phase::Closed
    }
    /// Demand the entire io output budget, so Room proves previous sends finished.
    pub fn flush(&mut self, output: u32, out: &mut Queue<Request>) {
        if self.needs_flush() {
            out.push(Request::Stream { stream: self.token, down: Down::Demand { read: Read::Nothing, room: output } });
            self.phase = Phase::Flushing;
        }
    }
    #[must_use]
    pub fn needs_flush(&self) -> bool {
        self.phase == Phase::Idle && self.queued.is_empty() && self.dirty
    }
    #[must_use]
    pub fn drained(&self) -> bool {
        self.phase == Phase::Idle && self.queued.is_empty() && !self.dirty
    }
    #[must_use]
    pub fn pending(&self) -> bool {
        self.phase == Phase::Idle && !self.queued.is_empty() || self.phase == Phase::Failed
    }
}
