//! Safety checks observe the component's boundaries only.
use crate::io::Below;
use skein_lib::Token;
use temper_worker_domain_checkout::git;
use temper_worker_protocol_git::Event;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Passed,
}
#[derive(Debug)]
pub struct Expected {
    done: Option<git::Done>,
    fault: Option<git::Fault>,
}
impl Expected {
    #[must_use]
    pub fn done(done: git::Done) -> Self {
        Self { done: Some(done), fault: None }
    }
    #[must_use]
    pub fn fault(fault: git::Fault) -> Self {
        Self { done: None, fault: Some(fault) }
    }
}
#[derive(Debug, Default)]
pub struct Referee {
    pub terminals: Vec<(Token, git::Done)>,
    stopped_at: Option<u32>,
}
impl Referee {
    pub fn stop(&mut self, spawns: u32) {
        self.stopped_at = Some(spawns);
    }
    pub fn observe(&mut self, event: Event, below: &Below, owner: Token) {
        let Event::Done { owner: got, done } = event else { panic!("operation returned a startup event") };
        assert_eq!(got, owner);
        assert!(self.terminals.is_empty(), "one terminal per operation");
        assert_eq!(below.closed_children, below.spawns, "all children closed before Done");
        self.check(below);
        self.terminals.push((got, done));
    }
    pub fn check(&self, below: &Below) {
        if let Some(at) = self.stopped_at {
            assert_eq!(below.spawns, at, "no invocation starts after stop");
        }
        for arguments in &below.arguments {
            for argument in arguments {
                assert!(!argument.windows(11).any(|part| part == b"secret-git!"));
            }
        }
    }
    #[must_use]
    pub fn finish(&self, expected: &Expected) -> Verdict {
        assert_eq!(self.terminals.len(), 1, "one terminal reached");
        if let Some(done) = &expected.done {
            assert_eq!(&self.terminals[0].1, done);
        }
        if let Some(fault) = &expected.fault {
            match &self.terminals[0].1 {
                git::Done::Failed { fault: got } | git::Done::FailedWithOutput { fault: got, .. } => {
                    assert_eq!(got, fault);
                }
                other @ (git::Done::IsAncestor { .. }
                | git::Done::Merged
                | git::Done::Conflicted { .. }
                | git::Done::Succeeded
                | git::Done::Fetched { .. }
                | git::Done::Committed { .. }
                | git::Done::Unchanged
                | git::Done::Exists
                | git::Done::Rejected) => panic!("expected {fault:?}, got {other:?}"),
            }
        }
        Verdict::Passed
    }
}
