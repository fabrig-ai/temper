//! Machine output and failure tails (protocol/git.md, sections 4 and 6).
use crate::{Version, invocation};
use alloc::boxed::Box;
use skein_lib::{List, bytes};
use temper_worker_domain_checkout::{self as checkout, git};

/// A push's porcelain status for the requested branch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    FastForward,
    NewBranch,
    UpToDate,
    NonFastForward,
    FetchFirst,
    StaleInfo,
    Declined,
    Other,
}

fn split(input: &[u8], delimiter: u8) -> Option<(&[u8], &[u8])> {
    let at = bytes::find(input, &[delimiter])?;
    Some((input.get(..at)?, input.get(at.checked_add(1)?..)?))
}

/// Exactly one complete porcelain fetch line, with valid old and new ids.
#[must_use]
pub fn fetched(stdout: &[u8]) -> Option<git::Commit> {
    let line = stdout.strip_suffix(b"\n")?;
    let (flag, rest) = split(line, b' ')?;
    if flag != b" " && flag != b"+" && flag != b"*" && flag != b"=" && !flag.is_empty() {
        return None;
    }
    // A space flag occupies the first byte; splitting there leaves it empty.
    let rest = if flag.is_empty() { rest.strip_prefix(b" ")? } else { rest };
    let (old, rest) = split(rest, b' ')?;
    let (new, target) = split(rest, b' ')?;
    invocation::object(old)?;
    if old.len() != new.len() || target != b"refs/temper/fetched" {
        return None;
    }
    invocation::object(new)
}

/// The requested branch's unique status line; other refs do not decide it.
#[must_use]
pub fn status(stdout: &[u8], branch: &[u8]) -> Option<Status> {
    let target = invocation::joined(&[b"refs/heads/", branch]);
    let mut found = None;
    let mut start = 0_usize;
    if !stdout.ends_with(b"\n") {
        return None;
    }
    for (end, byte) in stdout.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let line = stdout.get(start..end)?;
        start = end.checked_add(1)?;
        let Some((flag, rest)) = split(line, b'\t') else {
            continue;
        };
        if flag.len() != 1 {
            return None;
        }
        let (spec, summary) = split(rest, b'\t')?;
        let (from, to) = split(spec, b':')?;
        if to != &*target {
            continue;
        }
        if found.is_some() {
            return None;
        }
        invocation::object(from)?;
        found = Some(match flag {
            b" " if !summary.is_empty() => Status::FastForward,
            b"*" if summary == b"[new branch]" => Status::NewBranch,
            b"=" if summary == b"[up to date]" => Status::UpToDate,
            b"!" => match summary {
                b"[rejected] (non-fast-forward)" => Status::NonFastForward,
                b"[rejected] (fetch first)" => Status::FetchFirst,
                b"[rejected] (stale info)" => Status::StaleInfo,
                other if other.starts_with(b"[remote rejected] (") && other.ends_with(b"declined)") => Status::Declined,
                _ => Status::Other,
            },
            _ => Status::Other,
        });
    }
    found
}

/// NUL-terminated paths, admitted before any owned path is allocated.
#[must_use]
pub fn paths(stdout: &[u8], checkout: &checkout::Limits) -> Option<Box<[Box<[u8]>]>> {
    let mut count = 0_u32;
    let mut start = 0_usize;
    for (end, byte) in stdout.iter().enumerate() {
        if *byte != 0 {
            continue;
        }
        let length = u32::try_from(end.checked_sub(start)?).ok()?;
        if length == 0 || length > checkout.path_bytes {
            return None;
        }
        count = count.checked_add(1)?;
        if count > checkout.conflicts {
            return None;
        }
        start = end.checked_add(1)?;
    }
    if start != stdout.len() {
        return None;
    }
    let mut out = List::with_capacity(count);
    start = 0;
    for (end, byte) in stdout.iter().enumerate() {
        if *byte == 0 {
            out.push(Box::from(stdout.get(start..end)?)).expect("validated path count");
            start = end.checked_add(1)?;
        }
    }
    Some(out.into_boxed())
}

fn number(input: &[u8]) -> Option<(u32, &[u8])> {
    let mut value = 0_u32;
    let mut end = 0_usize;
    for byte in input {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(byte.checked_sub(b'0')?))?;
        end = end.checked_add(1)?;
    }
    if end == 0 {
        return None;
    }
    Some((value, input.get(end..)?))
}
/// Version numbers from a complete git-version line, vendor suffix allowed.
#[must_use]
pub fn version(stdout: &[u8]) -> Option<Version> {
    let line = stdout.strip_prefix(b"git version ")?.strip_suffix(b"\n")?;
    let (major, rest) = number(line)?;
    let (minor, rest) = number(rest.strip_prefix(b".")?)?;
    let (patch, suffix) = number(rest.strip_prefix(b".")?)?;
    if bytes::find(suffix, b"\n").is_some() {
        return None;
    }
    if !suffix.is_empty() && !suffix.starts_with(b".") && !suffix.starts_with(b"-") && !suffix.starts_with(b" ") {
        return None;
    }
    Some(Version { major, minor, patch })
}

/// Last matching fatal/error line wins; local failures always are Broken.
#[must_use]
pub fn classify(tail: &[u8], kind: git::Kind) -> git::Fault {
    match kind {
        git::Kind::IsAncestor | git::Kind::Make | git::Kind::CheckOut | git::Kind::Merge | git::Kind::Commit => {
            return git::Fault::Broken;
        }
        git::Kind::Clone | git::Kind::Fetch | git::Kind::Create | git::Kind::Push => {}
    }
    let mut end = tail.len();
    for (at, byte) in tail.iter().enumerate().rev() {
        if *byte == b'\n' {
            let start = at.checked_add(1).expect("index within tail");
            if let Some(fault) = line_fault(tail.get(start..end).expect("line within tail"), kind) {
                return fault;
            }
            end = at;
        }
    }
    line_fault(tail.get(..end).expect("first line within tail"), kind).unwrap_or(git::Fault::Broken)
}
fn line_fault(line: &[u8], kind: git::Kind) -> Option<git::Fault> {
    if line.starts_with(b"fatal: repository '") && line.ends_with(b"' not found") {
        return Some(git::Fault::Missing { missing: git::Missing::Repository });
    }
    if kind == git::Kind::Fetch {
        if line.starts_with(b"fatal: couldn't find remote ref ") {
            return Some(git::Fault::Missing { missing: git::Missing::Branch });
        }
        if line.starts_with(b"fatal: remote error: upload-pack: not our ref ") {
            return Some(git::Fault::Missing { missing: git::Missing::Commit });
        }
    }
    if line.starts_with(b"fatal: could not read Username for ")
        || line.starts_with(b"fatal: Authentication failed for ")
    {
        return Some(git::Fault::Refused);
    }
    if let Some(rest) = line.strip_prefix(b"fatal: unable to access '")
        && let Some(at) = bytes::find(rest, b"': ")
    {
        let reason = rest.get(at.checked_add(3)?..)?;
        return Some(match reason {
            b"The requested URL returned error: 401" | b"The requested URL returned error: 403" => git::Fault::Refused,
            b"The requested URL returned error: 404" => git::Fault::Missing { missing: git::Missing::Repository },
            _ => git::Fault::Unreachable,
        });
    }
    if line.starts_with(b"fatal: the remote end hung up unexpectedly")
        || line.starts_with(b"fatal: early EOF")
        || line.starts_with(b"error: RPC failed")
    {
        return Some(git::Fault::Unreachable);
    }
    None
}

/// A bounded streaming check for conflict marker lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Markers {
    head: [u8; 8],
    seen: u32,
    last: u8,
    found: bool,
}
impl Markers {
    /// Before the first read.
    pub const START: Markers = Markers { head: [0; 8], seen: 0, last: 0, found: false };
    /// Feed any-sized read, preserving a line across reads.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.end_line();
            } else {
                if let Some(slot) = self.head.get_mut(usize::try_from(self.seen).unwrap_or(usize::MAX)) {
                    *slot = byte;
                }
                self.seen = self.seen.saturating_add(1);
                self.last = byte;
            }
        }
    }
    /// Finish also checks the last line without a newline.
    #[must_use]
    pub fn finish(mut self) -> bool {
        if self.seen > 0 {
            self.end_line();
        }
        self.found
    }
    fn end_line(&mut self) {
        let length = if self.last == b'\r' { self.seen.saturating_sub(1) } else { self.seen };
        let kept = self.head.get(..usize::try_from(length.min(8)).expect("eight fits")).expect("within head");
        self.found = self.found
            || kept.starts_with(b"<<<<<<< ")
            || kept.starts_with(b">>>>>>> ")
            || (length == 7 && kept == b"=======");
        self.seen = 0;
        self.last = 0;
    }
}
