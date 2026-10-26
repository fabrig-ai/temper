//! Factories close unused startup signal descriptors before running their host.
use skein_io::kernel::{Complete, Fd, Op, Submit};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use std::ops::{Deref, DerefMut};
const CLOSE: Token = Token::new(u64::MAX);
pub(crate) struct SignalHost<H> {
    host: H,
    signal: Option<Fd>,
    closing: bool,
}
impl<H> SignalHost<H> {
    pub fn new(host: H, signal: Option<Fd>) -> Self {
        Self { host, signal, closing: false }
    }
}
impl<H> Deref for SignalHost<H> {
    type Target = H;
    fn deref(&self) -> &H {
        &self.host
    }
}
impl<H> DerefMut for SignalHost<H> {
    fn deref_mut(&mut self) -> &mut H {
        &mut self.host
    }
}
impl<H: Host> Host for SignalHost<H> {
    fn drain(&mut self) {
        self.host.drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        for _ in 0..self.host.completions().len() {
            let complete = self.host.completions().pop().expect("completion count");
            if complete.op == CLOSE {
                complete.result.expect("startup signal close");
                self.signal = None;
            } else {
                self.host.completions().push(complete);
            }
        }
        if !self.closing
            && self.host.submissions().room() > 0
            && let Some(fd) = self.signal
        {
            self.closing = true;
            self.host.submissions().push(Submit { op: CLOSE, kind: Op::Close { fd } });
        }
        self.host.iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host.work_pending(now) || (self.signal.is_some() && !self.closing)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.signal.is_none() && self.host.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.host.worst_case()
    }
    fn exit(&self) -> Option<skein_io::kernel::Exit> {
        self.host.exit()
    }
    fn operations(&self) -> u32 {
        self.host.operations() + 1
    }
}

pub(crate) struct Machine(pub skein_fake_machine::Machine);
impl skein_world::Machine for Machine {
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, skein_io::kernel::Error> {
        if path == b"agent" {
            let worker = self.0.reopen_root(1);
            // The parent has no workspace; repository runs adopt its prepared directory.
            let root =
                match self.0.open(worker, crate::fixtures::PREPARED_REPOSITORY, skein_fake_machine::How::Directory) {
                    Ok(root) => {
                        self.0.close(worker);
                        root
                    }
                    Err(skein_fake_machine::Refusal::NotFound) => worker,
                    Err(why) => panic!("prepared agent directory: {why:?}"),
                };
            return Ok(skein_sim::Handle::new(root.raw()));
        }
        let index = match path {
            b"state" => 0,
            b"worker" => 1,
            b"client" => 2,
            b"/setup" => 3,
            _ => return Err(skein_io::kernel::Error::NotFound),
        };
        Ok(skein_sim::Handle::new(self.0.reopen_root(index).raw()))
    }
    fn close_root(&mut self, root: skein_sim::Handle) {
        self.0.close(skein_fake_machine::Opened::new(root.raw()));
    }
    fn cut(&mut self, cut: skein_world::Cut, held: &[skein_sim::Handle], seed: u64) {
        match cut {
            skein_world::Cut::Kill => {
                for handle in held {
                    self.0.close(skein_fake_machine::Opened::new(handle.raw()));
                }
            }
            skein_world::Cut::PowerLoss => self.0.crash(seed),
        }
    }
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}
pub(crate) fn fresh() -> skein_fake_machine::Machine {
    let mut machine = skein_fake_machine::Machine::for_user(1000);
    for items in [
        vec![],
        vec![
            skein_fake_machine::Item::directory(b"workspaces"),
            skein_fake_machine::Item::directory(b"trash"),
            skein_fake_machine::Item::directory(b"home"),
        ],
        vec![],
    ] {
        let root = machine.lay(&items);
        machine.close(root);
    }
    machine
}
