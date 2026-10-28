//! Observe the hosted shell's kernel lock without changing its requests.
use skein_io::kernel::{Complete, Done, Error, Exit, Fd, Op, Submit};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use std::{cell::RefCell, rc::Rc};

/// Only lock-state changes cross from hosted calls to this story's referee.
#[derive(Clone, Copy)]
pub(crate) struct LockChange {
    setup: bool,
    call: usize,
    locked: Option<Time>,
    released: Option<Time>,
    busy: u32,
}
impl LockChange {
    fn differs(self, call: &super::Called) -> bool {
        (self.locked, self.released, self.busy) != (call.locked, call.released, call.busy)
    }
    fn apply(self, call: &mut super::Called) {
        call.locked = self.locked;
        call.released = self.released;
        call.busy = self.busy;
    }
}

pub(crate) struct LockChanges {
    pending: Vec<LockChange>,
    // Call groups whose records or applied caller state may have changed.
    pub(crate) invalidated: Vec<(usize, bool)>,
}
impl Default for LockChanges {
    fn default() -> Self {
        // Each live call iterates at most once between referee acts.
        let live = crate::fixtures::CALLS_AT_ONCE as usize;
        Self { pending: Vec::with_capacity(live), invalidated: Vec::with_capacity(live) }
    }
}
impl LockChanges {
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.invalidated.clear();
    }
    pub(crate) fn invalidate(&mut self, call: usize, setup: bool) {
        if !self.invalidated.contains(&(call, setup)) {
            self.invalidated.push((call, setup));
        }
    }
    fn push(&mut self, change: LockChange) {
        assert!(self.pending.len() < self.pending.capacity(), "bounded live lock changes");
        self.invalidate(change.call, change.setup);
        self.pending.push(change);
    }
    pub(crate) fn changed(&self, caller: &super::Caller) -> bool {
        self.pending
            .iter()
            .any(|change| (!change.setup || caller.setup()) && change.differs(&caller.calls[change.call]))
    }
    pub(crate) fn apply(&mut self, caller: &mut super::Caller) {
        for index in 0..self.pending.len() {
            let change = self.pending[index];
            if !change.setup || caller.setup() {
                change.apply(&mut caller.calls[change.call]);
                // Applying one attempt can leave another attempt disagreeing.
                self.invalidate(change.call, change.setup);
            }
        }
        self.pending.clear();
    }
}

pub(crate) struct Recorded {
    pub(crate) setup: bool,
    pub(crate) host: temper_client_shell_cli::host::OneShot,
    pub(crate) call: usize,
    pub(crate) locked: Option<Time>,
    pub(crate) released: Option<Time>,
    pub(crate) busy: u32,
    changes: RefCell<Option<Rc<RefCell<LockChanges>>>>,
    lock: Option<Token>,
    descriptor: Option<Fd>,
}
impl Recorded {
    pub(crate) fn new(host: temper_client_shell_cli::host::OneShot, call: usize, setup: bool) -> Self {
        Self {
            setup,
            host,
            call,
            locked: None,
            released: None,
            busy: 0,
            changes: RefCell::new(None),
            lock: None,
            descriptor: None,
        }
    }
    pub(crate) fn differs(&self, caller: &super::Caller) -> bool {
        (!self.setup || caller.setup()) && self.change().differs(&caller.calls[self.call])
    }
    pub(crate) fn apply(&self, caller: &mut super::Caller) {
        if !self.setup || caller.setup() {
            self.change().apply(&mut caller.calls[self.call]);
        }
    }
    pub(crate) fn record_change(&self) {
        if let Some(changes) = self.changes.borrow().as_ref() {
            changes.borrow_mut().push(self.change());
        }
    }
    pub(crate) fn stop_observing(&self) {
        self.changes.borrow_mut().take();
    }
    fn change(&self) -> LockChange {
        LockChange { setup: self.setup, call: self.call, locked: self.locked, released: self.released, busy: self.busy }
    }
    pub(crate) fn observe_with(&self, changes: &Rc<RefCell<LockChanges>>) {
        let mut observer = self.changes.borrow_mut();
        if observer.is_none() {
            *observer = Some(changes.clone());
            // A retry reuses its call number: its initial empty state matters too.
            changes.borrow_mut().push(self.change());
        }
    }
}
impl Host for Recorded {
    fn iterate(&mut self, now: Time, wall: Wall) {
        let before = (self.locked, self.released, self.busy);
        for complete in self.host.completions().iter() {
            if let Op::Lock { fd } = complete.kind {
                assert_eq!(self.lock.take(), Some(complete.op), "submitted lock");
                match complete.result {
                    Ok(Done::Nothing) => {
                        self.locked = Some(now);
                        self.descriptor = Some(fd);
                    }
                    Err(Error::Busy) => self.busy += 1,
                    _ => panic!("lock completion {:?}", complete.result),
                }
            } else if matches!(complete.kind, Op::Close { fd } if Some(fd) == self.descriptor) {
                assert_eq!(complete.result, Ok(Done::Nothing), "locked descriptor closed");
                self.released = Some(now);
                self.descriptor = None;
            }
        }
        self.host.iterate(now, wall);
        if before != (self.locked, self.released, self.busy) {
            self.record_change();
        }
    }
    fn drain(&mut self) {
        self.host.drain();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        for submit in self.host.submissions().iter() {
            if matches!(submit.kind, Op::Lock { .. }) {
                self.lock = Some(submit.op);
            }
        }
        self.host.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host.is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.host.exit()
    }
    fn operations(&self) -> u32 {
        self.host.operations()
    }
}

#[cfg(test)]
mod tests {
    use super::{LockChange, LockChanges};
    use crate::calls::{Called, Caller};
    use skein_io::kernel::Fd;
    use skein_lib::Time;

    #[test]
    fn a_retry_clears_old_locks_and_finished_setup_cannot_touch_its_reused_call() {
        let mut caller = Caller::new(vec![], std::path::PathBuf::new(), Fd::new(0));
        caller.calls.push(Called {
            locked: Some(Time::from_nanos(1)),
            released: Some(Time::from_nanos(2)),
            busy: 1,
            exit: None,
            stdout: vec![],
            stderr: vec![],
            spawned: Time::ZERO,
            ended: Time::ZERO,
            observed_at: None,
            tries: 0,
            early: 0,
        });
        let mut changes = LockChanges::default();
        let retry = LockChange { setup: false, call: 0, locked: None, released: None, busy: 0 };
        changes.push(retry);
        assert!(changes.changed(&caller), "the new host's empty state replaces its old attempt");
        changes.apply(&mut caller);
        assert_eq!((caller.calls[0].locked, caller.calls[0].released, caller.calls[0].busy), (None, None, 0));
        for next in [
            LockChange { busy: 1, ..retry },
            LockChange { locked: Some(Time::from_nanos(3)), busy: 1, ..retry },
            LockChange { locked: Some(Time::from_nanos(3)), released: Some(Time::from_nanos(4)), busy: 1, ..retry },
        ] {
            changes.push(next);
            assert!(changes.changed(&caller));
            changes.apply(&mut caller);
            assert!(!changes.changed(&caller));
            assert!(!next.differs(&caller.calls[0]));
        }
        // A fixture call has been popped; its index may now be absent or reused.
        changes.push(LockChange { setup: true, call: 1, ..retry });
        changes.push(LockChange { setup: true, call: 0, ..retry });
        assert!(!changes.changed(&caller));
        changes.apply(&mut caller);
        assert_eq!(caller.calls[0].released, Some(Time::from_nanos(4)));
    }
}
