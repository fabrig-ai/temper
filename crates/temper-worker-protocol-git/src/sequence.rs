//! Fixed sequences, never retries (protocol/git.md, section 4).
use crate::{
    invocation::{self, Command, Source},
    output::{self, Status},
};
use alloc::boxed::Box;
use temper_worker_domain_checkout::{self as checkout, git};

/// The next step after an invocation has settled and closed.
#[derive(Clone, PartialEq, Eq, Debug)]
#[expect(clippy::large_enum_variant, reason = "bounded diagnostics stay inline as in git::Done")]
pub enum Next {
    Run(Command),
    Make,
    Scan { files: Box<[Box<[u8]>]> },
    End(git::Done),
}
/// The first step depends only on the operation.
#[must_use]
pub fn first(op: &git::Op) -> Next {
    Next::Run(match op {
        git::Op::Make { .. } => return Next::Make,
        git::Op::Clone { at, remote, .. } => {
            Command::Clone { remote: remote.clone(), directory: at.repository.clone() }
        }
        git::Op::Fetch { remote, want, .. } => Command::Fetch {
            remote: remote.clone(),
            source: match want {
                git::Want::Branch { branch } => Source::Branch(branch.clone()),
                git::Want::Commit { commit } => Source::Commit(*commit),
                git::Want::Default => Source::Default,
            },
        },
        git::Op::Create { remote, branch, commit, .. } => {
            Command::Create { remote: remote.clone(), branch: branch.clone(), commit: *commit }
        }
        git::Op::CheckOut { commit, .. } => Command::CheckOut { commit: *commit },
        git::Op::IsAncestor { ancestor, commit, .. } => Command::IsAncestor { expected: *ancestor, commit: *commit },
        git::Op::Merge { theirs, .. } => Command::Merge { theirs: *theirs },
        git::Op::Commit { merging, .. } => {
            if merging.is_some() {
                Command::Unmerged
            } else {
                Command::Add
            }
        }
        git::Op::Push { remote, commit, branch, expected, .. } => match expected {
            Some(expected) => Command::IsAncestor { expected: *expected, commit: *commit },
            None => Command::Push { remote: remote.clone(), commit: *commit, branch: branch.clone(), expected: None },
        },
    })
}
/// Fault and diagnostic from the invocation that failed.
#[must_use]
pub fn failed(fault: git::Fault, tail: &[u8], cut: u64) -> Next {
    Next::End(git::Done::FailedWithOutput { fault, diagnostic: git::PushDiagnostic::new(tail, cut) })
}
/// Decide by exit status and stdout first, otherwise by the failure tail.
#[must_use]
pub fn after(
    op: &git::Op,
    ran: &Command,
    code: u8,
    stdout: &[u8],
    tail: &[u8],
    cut: u64,
    checkout: &checkout::Limits,
) -> Next {
    match ran {
        Command::Clone { .. } | Command::Clean if code == 0 => return Next::End(git::Done::Succeeded),
        Command::Fetch { .. } if code == 0 => {
            if let Some(commit) = output::fetched(stdout) {
                return Next::End(git::Done::Fetched { commit });
            }
        }
        Command::Create { branch, .. } => {
            if let Some(next) = pushed(stdout, branch, true, tail, cut) {
                return next;
            }
        }
        Command::Push { branch, .. } => {
            if let Some(next) = pushed(stdout, branch, false, tail, cut) {
                return next;
            }
        }
        Command::CheckOut { .. } if code == 0 => return Next::Run(Command::Clean),
        Command::Merge { .. } => match code {
            0 => return Next::End(git::Done::Merged),
            1 => return Next::Run(Command::Unmerged),
            _ => {}
        },
        Command::Unmerged if code == 0 => {
            if let Some(next) = unmerged(op, stdout, checkout) {
                return next;
            }
        }
        Command::Add if code == 0 => match op {
            git::Op::Commit { parent, merging, .. } => {
                return Next::Run(if merging.is_some() {
                    Command::WriteTree
                } else {
                    Command::DiffIndex { parent: *parent }
                });
            }
            git::Op::Make { .. }
            | git::Op::Clone { .. }
            | git::Op::Fetch { .. }
            | git::Op::Create { .. }
            | git::Op::CheckOut { .. }
            | git::Op::IsAncestor { .. }
            | git::Op::Merge { .. }
            | git::Op::Push { .. } => {}
        },
        Command::DiffIndex { .. } => match code {
            0 => return Next::End(git::Done::Unchanged),
            1 => return Next::Run(Command::WriteTree),
            _ => {}
        },
        Command::WriteTree if code == 0 => {
            if let Some(command) = commit_tree(op, stdout) {
                return Next::Run(command);
            }
        }
        Command::CommitTree { .. } if code == 0 => {
            if let Some(commit) = id(stdout) {
                return Next::End(git::Done::Committed { commit });
            }
        }
        Command::IsAncestor { .. } => match op {
            git::Op::IsAncestor { .. } => {
                if code <= 1 {
                    return Next::End(git::Done::IsAncestor { contains: code == 0 });
                }
                return failed(git::Fault::Broken, tail, cut);
            }
            git::Op::Push { remote, commit, branch, expected, .. } => match code {
                1 => return Next::End(git::Done::Rejected),
                0 => {
                    return Next::Run(Command::Push {
                        remote: remote.clone(),
                        commit: *commit,
                        branch: branch.clone(),
                        expected: *expected,
                    });
                }
                _ => {}
            },
            git::Op::Make { .. }
            | git::Op::Clone { .. }
            | git::Op::Fetch { .. }
            | git::Op::Create { .. }
            | git::Op::CheckOut { .. }
            | git::Op::Merge { .. }
            | git::Op::Commit { .. } => {}
        },
        Command::Version
        | Command::Clone { .. }
        | Command::Fetch { .. }
        | Command::CheckOut { .. }
        | Command::Clean
        | Command::Unmerged
        | Command::Add
        | Command::WriteTree
        | Command::CommitTree { .. } => {}
    }
    let fault = if code == 0 { git::Fault::Broken } else { output::classify(tail, op.kind()) };
    failed(fault, tail, cut)
}
fn id(stdout: &[u8]) -> Option<git::Commit> {
    invocation::object(stdout.strip_suffix(b"\n")?)
}
fn pushed(stdout: &[u8], branch: &[u8], create: bool, tail: &[u8], cut: u64) -> Option<Next> {
    let status = output::status(stdout, branch)?;
    let done = match status {
        Status::NewBranch => git::Done::Succeeded,
        Status::UpToDate => {
            if create {
                git::Done::Exists
            } else {
                git::Done::Succeeded
            }
        }
        Status::StaleInfo => {
            if create {
                git::Done::Exists
            } else {
                git::Done::Rejected
            }
        }
        Status::FastForward if !create => git::Done::Succeeded,
        Status::NonFastForward | Status::FetchFirst if !create => git::Done::Rejected,
        Status::Declined => return Some(failed(git::Fault::Refused, tail, cut)),
        Status::FastForward | Status::NonFastForward | Status::FetchFirst | Status::Other => return None,
    };
    Some(Next::End(done))
}
fn unmerged(op: &git::Op, stdout: &[u8], checkout: &checkout::Limits) -> Option<Next> {
    let files = output::paths(stdout, checkout)?;
    match op {
        git::Op::Merge { .. } if !files.is_empty() => Some(Next::End(git::Done::Conflicted { files })),
        git::Op::Commit { .. } => Some(if files.is_empty() { Next::Run(Command::Add) } else { Next::Scan { files } }),
        git::Op::Make { .. }
        | git::Op::Clone { .. }
        | git::Op::Fetch { .. }
        | git::Op::Create { .. }
        | git::Op::CheckOut { .. }
        | git::Op::IsAncestor { .. }
        | git::Op::Merge { .. }
        | git::Op::Push { .. } => None,
    }
}
fn commit_tree(op: &git::Op, stdout: &[u8]) -> Option<Command> {
    let tree = id(stdout)?;
    match op {
        git::Op::Commit { parent, merging, .. } => {
            Some(Command::CommitTree { tree, parent: *parent, merging: *merging })
        }
        git::Op::Make { .. }
        | git::Op::Clone { .. }
        | git::Op::Fetch { .. }
        | git::Op::Create { .. }
        | git::Op::CheckOut { .. }
        | git::Op::IsAncestor { .. }
        | git::Op::Merge { .. }
        | git::Op::Push { .. } => None,
    }
}
