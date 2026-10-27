//! Canonical filtered-tree keys (domain/landing.md, 6.1).
use crate::{Commit, Gate, Key, Repository};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
};
use skein_lib::{Writer, sha256::Sha256};

/// One forge tree entry.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Entry {
    pub name: Box<[u8]>,
    pub mode: u32,
    pub kind: EntryKind,
    pub object: Commit,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntryKind {
    Blob,
    Tree,
    Commit,
}
/// A whole tree: root named by candidate, directories by their own ids.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Tree {
    pub object: Commit,
    pub entries: Box<[Entry]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Keyed {
    Need(Commit),
    Key(Key),
    TooLarge,
}
/// Bounds of the filtered entries and their total full-path bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyLimits {
    pub entries: u32,
    pub path_bytes: u32,
}

/// Workflow, ignored-path count and paths, each length-prefixed by a
/// big-endian u16; then one byte for blocking (domain/landing.md, 6.1).
#[must_use]
pub fn definition(gate: &Gate) -> Box<[u8]> {
    let mut length = gate.workflow.len().checked_add(5).expect("definition header fits");
    for path in &gate.ignored {
        length = length.checked_add(2).expect("path length prefix fits");
        length = length.checked_add(path.len()).expect("definition fits");
    }
    assert!(u16::try_from(length).is_ok(), "definition fits its encoded length");
    let mut out = Writer::new(length);
    put_word(&mut out, gate.workflow.len());
    out.put(&gate.workflow).expect("sized workflow");
    put_word(&mut out, gate.ignored.len());
    for path in &gate.ignored {
        put_word(&mut out, path.len());
        out.put(path).expect("sized ignored path");
    }
    out.put(&[u8::from(gate.blocking)]).expect("sized blocking flag");
    out.finish()
}

fn put_word(out: &mut Writer, value: usize) {
    out.put(&u16::try_from(value).expect("definition length fits u16").to_be_bytes()).expect("sized word");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pending {
    Directory(Commit),
    Entry { mode: u32, object: Commit },
}

/// Hash a filtered tree, requesting the first missing tree in path order.
#[must_use]
pub fn key(
    repository: Repository,
    name: &[u8],
    definition: &[u8],
    ignored: &[Box<[u8]>],
    candidate: Commit,
    trees: &[Tree],
    limits: &KeyLimits,
) -> Keyed {
    let (Ok(name_len), Ok(definition_len)) = (u16::try_from(name.len()), u16::try_from(definition.len())) else {
        return Keyed::TooLarge;
    };
    let mut hash = Sha256::new();
    hash.update(&[1]);
    hash.update(&u64::from(repository.forge).to_be_bytes());
    hash.update(&u64::from(repository.repository).to_be_bytes());
    hash.update(&name_len.to_be_bytes());
    hash.update(name);
    hash.update(&definition_len.to_be_bytes());
    hash.update(definition);
    walk(hash, ignored, candidate, trees, *limits)
}

fn walk(mut hash: Sha256, ignored: &[Box<[u8]>], candidate: Commit, trees: &[Tree], limits: KeyLimits) -> Keyed {
    let mut pending = BTreeMap::new();
    let mut read = BTreeSet::new();
    let replaced = pending.insert(Box::<[u8]>::from([]), Pending::Directory(candidate));
    assert!(replaced.is_none(), "root is the first entry");
    let mut entries = 0_u32;
    let mut path_bytes = 0_u32;
    // Each expanded directory consumes a distinct prefix of an ignored path.
    let mut budget = u64::from(limits.entries).saturating_add(1);
    for path in ignored {
        budget = budget.saturating_add(u64::try_from(path.len()).expect("path length fits u64"));
    }
    for _ in 0..budget {
        let Some((path, item)) = pending.pop_first() else { return finished(hash, &read, trees) };
        match item {
            Pending::Directory(object) => {
                let mut tree = None;
                for supplied in trees {
                    if supplied.object == object {
                        if tree.is_some() {
                            return Keyed::TooLarge;
                        }
                        tree = Some(supplied);
                    }
                }
                let Some(tree) = tree else {
                    for supplied in trees {
                        if !read.contains(&supplied.object) {
                            return Keyed::TooLarge;
                        }
                    }
                    return Keyed::Need(object);
                };
                let _inserted = read.insert(object);
                if !expand(&path, tree, ignored, limits, &mut pending, &mut entries, &mut path_bytes) {
                    return Keyed::TooLarge;
                }
            }
            Pending::Entry { mode, object } => {
                hash.update(&u16::try_from(path.len()).expect("admitted path length").to_be_bytes());
                hash.update(&path);
                hash.update(&mode.to_be_bytes());
                hash.update(&object);
            }
        }
    }
    if pending.is_empty() { finished(hash, &read, trees) } else { Keyed::TooLarge }
}

fn finished(hash: Sha256, read: &BTreeSet<Commit>, trees: &[Tree]) -> Keyed {
    for tree in trees {
        if !read.contains(&tree.object) {
            return Keyed::TooLarge;
        }
    }
    Keyed::Key(hash.finish())
}

fn expand(
    parent: &[u8],
    tree: &Tree,
    ignored: &[Box<[u8]>],
    limits: KeyLimits,
    pending: &mut BTreeMap<Box<[u8]>, Pending>,
    entries: &mut u32,
    path_bytes: &mut u32,
) -> bool {
    for entry in &tree.entries {
        let length = parent.len().saturating_add(usize::from(!parent.is_empty())).saturating_add(entry.name.len());
        if entry.name.is_empty() {
            return false;
        }
        let mut path = Writer::new(length);
        path.put(parent).expect("sized parent");
        if !parent.is_empty() {
            path.put(b"/").expect("sized separator");
        }
        path.put(&entry.name).expect("sized name");
        let path = path.finish();
        let mut excluded = false;
        let mut below = false;
        for ignore in ignored {
            if prefix(ignore, &path) {
                excluded = true;
            }
            if path.len() < ignore.len() && prefix(&path, ignore) {
                below = true;
            }
        }
        if excluded {
            continue;
        }
        let item = match entry.kind {
            EntryKind::Tree if below => Pending::Directory(entry.object),
            EntryKind::Tree | EntryKind::Blob | EntryKind::Commit => {
                if *entries >= limits.entries || length > usize::from(u16::MAX) {
                    return false;
                }
                *entries = entries.checked_add(1).expect("entry below limit");
                let Some(total) = path_bytes.checked_add(u32::try_from(path.len()).expect("path fits u16")) else {
                    return false;
                };
                if total > limits.path_bytes {
                    return false;
                }
                *path_bytes = total;
                Pending::Entry { mode: entry.mode, object: entry.object }
            }
        };
        if pending.insert(path, item).is_some() {
            return false;
        }
    }
    true
}

fn prefix(prefix: &[u8], path: &[u8]) -> bool {
    path == prefix || (path.starts_with(prefix) && path.get(prefix.len()) == Some(&b'/'))
}
