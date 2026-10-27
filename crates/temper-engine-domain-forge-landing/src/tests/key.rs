use crate::{Entry, EntryKind, Gate, Key, KeyLimits, Keyed, Repository, Tree, definition, key};
use alloc::boxed::Box;
use skein_lib::{List, Writer, bytes::copy_of, sha256::digest};

const ROOT: [u8; 32] = [1; 32];
const REPOSITORY: Repository = Repository { forge: 1, repository: 2 };
const LIMITS: KeyLimits = KeyLimits { entries: 8, path_bytes: 256 };
fn gate() -> Gate {
    Gate {
        name: copy_of(b"gate"),
        workflow: copy_of(b"gate.yml"),
        ignored: Box::new([copy_of(b"docs")]),
        blocking: true,
    }
}
fn tree(object: u8, pairs: &[(&[u8], u8)]) -> Tree {
    let mut entries = List::with_capacity(u32::try_from(pairs.len()).expect("fixture count fits"));
    for (path, id) in pairs {
        entries
            .push(Entry { name: copy_of(path), mode: 0o100_644, kind: EntryKind::Blob, object: [*id; 32] })
            .expect("fixture sized");
    }
    Tree { object: [object; 32], entries: entries.into_boxed() }
}
fn directory(root: &mut Tree, index: usize) {
    root.entries[index].mode = 0o040_000;
    root.entries[index].kind = EntryKind::Tree;
}
fn keyed(trees: &[Tree], ignored: &[Box<[u8]>]) -> Keyed {
    key(REPOSITORY, b"gate", b"definition", ignored, ROOT, trees, &LIMITS)
}
fn complete(value: Keyed) -> Key {
    match value {
        Keyed::Key(key) => key,
        Keyed::Need(_) | Keyed::TooLarge => unreachable!("complete fixture"),
    }
}
#[test]
fn a_key_with_nothing_ignored_reads_the_root_alone() {
    assert_eq!(keyed(&[], &[]), Keyed::Need(ROOT));
    let mut root = tree(1, &[(b"src", 2)]);
    directory(&mut root, 0);
    let result = keyed(&[root], &[]);
    assert_eq!(result, Keyed::Key(complete(result)));
}
#[test]
fn an_ignored_directory_is_left_out_without_reading_it() {
    let mut root = tree(1, &[(b"docs", 2)]);
    directory(&mut root, 0);
    assert_eq!(keyed(&[root], &[copy_of(b"docs")]), keyed(&[tree(1, &[])], &[]));
}
#[test]
fn an_ignored_file_reads_each_directory_above_it() {
    let ignored = [copy_of(b"crates/x/README.md")];
    let mut root = tree(1, &[(b"crates", 2)]);
    directory(&mut root, 0);
    assert_eq!(keyed(&[root.clone()], &ignored), Keyed::Need([2; 32]));
    let mut crates = tree(2, &[(b"x", 3)]);
    directory(&mut crates, 0);
    assert_eq!(keyed(&[root.clone(), crates.clone()], &ignored), Keyed::Need([3; 32]));
    let x = tree(3, &[(b"README.md", 4), (b"lib.rs", 5)]);
    let result = keyed(&[root, crates, x], &ignored);
    assert_eq!(result, Keyed::Key(complete(result)));
}
#[test]
fn a_change_inside_an_ignored_path_keeps_the_key() {
    let mut before = tree(1, &[(b"docs", 2), (b"lib.rs", 3)]);
    directory(&mut before, 0);
    let mut after = before.clone();
    after.entries[0].object = [9; 32];
    assert_eq!(keyed(&[before], &[copy_of(b"docs")]), keyed(&[after], &[copy_of(b"docs")]));
}
#[test]
fn a_change_outside_the_ignored_paths_changes_the_key() {
    let before = tree(1, &[(b"docs", 2), (b"lib.rs", 3)]);
    let mut after = before.clone();
    after.entries[1].object = [9; 32];
    assert_ne!(complete(keyed(&[before], &[copy_of(b"docs")])), complete(keyed(&[after], &[copy_of(b"docs")])));
}
#[test]
fn keys_differ_by_repository_gate_and_definition() {
    let trees = [tree(1, &[(b"a", 2)])];
    let original = complete(key(REPOSITORY, b"gate", b"definition", &[], ROOT, &trees, &LIMITS));
    for repository in [Repository { forge: 3, ..REPOSITORY }, Repository { repository: 3, ..REPOSITORY }] {
        assert_ne!(original, complete(key(repository, b"gate", b"definition", &[], ROOT, &trees, &LIMITS)));
    }
    assert_ne!(original, complete(key(REPOSITORY, b"other", b"definition", &[], ROOT, &trees, &LIMITS)));
    assert_ne!(original, complete(key(REPOSITORY, b"gate", b"changed", &[], ROOT, &trees, &LIMITS)));
}
#[test]
fn entries_are_ordered_by_their_full_paths() {
    let mut root = tree(1, &[(b"crates", 2), (b"crates.md", 3)]);
    directory(&mut root, 0);
    let child = tree(2, &[(b"ignored", 4), (b"a", 5)]);
    let ignored = [copy_of(b"crates/ignored")];
    let original = keyed(&[root.clone(), child.clone()], &ignored);
    root.entries.reverse();
    let mut reversed = child.clone();
    reversed.entries.reverse();
    assert_eq!(original, keyed(&[root, reversed], &ignored));
    // Independent flattened paths enforce crates.md before crates/a.
    assert_eq!(original, keyed(&[tree(1, &[(b"crates.md", 3), (b"crates/a", 5)])], &[]));
}
#[test]
fn the_encoding_is_landing_6_1s() {
    let trees = [tree(1, &[(b"b", 4), (b"a", 3)])];
    let mut bytes = Writer::new(107);
    bytes
        .put(&[
            1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 4, b'g', b'a', b't', b'e', 0, 4, b'd', b'e', b'f',
            b'n',
        ])
        .expect("header");
    bytes.put(&[0, 1, b'a', 0, 0, 0x81, 0xa4]).expect("first entry");
    bytes.put(&[3; 32]).expect("first object");
    bytes.put(&[0, 1, b'b', 0, 0, 0x81, 0xa4]).expect("second entry");
    bytes.put(&[4; 32]).expect("second object");
    assert_eq!(key(REPOSITORY, b"gate", b"defn", &[], ROOT, &trees, &LIMITS), Keyed::Key(digest(&bytes.finish())));
}
#[test]
fn a_filtered_tree_past_its_limits_is_too_large() {
    let trees = [tree(1, &[(b"a", 2), (b"bb", 3)])];
    for limits in [
        KeyLimits { entries: 2, path_bytes: 3 },
        KeyLimits { entries: 1, path_bytes: 3 },
        KeyLimits { entries: 2, path_bytes: 2 },
    ] {
        let result = key(REPOSITORY, b"gate", b"definition", &[], ROOT, &trees, &limits);
        if limits.entries == 2 && limits.path_bytes == 3 {
            assert_eq!(result, Keyed::Key(complete(result)));
        } else {
            assert_eq!(result, Keyed::TooLarge);
        }
    }
}
#[test]
fn a_gates_definition_has_workflow_paths_and_blocking() {
    let g = gate();
    assert_eq!(&*definition(&g), b"\0\x08gate.yml\0\x01\0\x04docs\x01");
    let mut changed = g.clone();
    changed.blocking = false;
    assert_ne!(definition(&g), definition(&changed));
    changed = g.clone();
    changed.workflow = copy_of(b"other.yml");
    assert_ne!(definition(&g), definition(&changed));
    changed = g.clone();
    changed.ignored = Box::new([copy_of(b"other")]);
    assert_ne!(definition(&g), definition(&changed));
}
#[test]
fn ignored_paths_match_whole_segments() {
    let trees = [tree(1, &[(b"docs", 2), (b"docs.md", 3), (b"docs2", 4)])];
    assert_eq!(keyed(&trees, &[copy_of(b"docs")]), keyed(&[tree(1, &[(b"docs.md", 3), (b"docs2", 4)])], &[]));
}
#[test]
fn an_unrequested_tree_is_rejected() {
    assert_eq!(keyed(&[tree(9, &[])], &[]), Keyed::TooLarge);
    assert_eq!(keyed(&[tree(1, &[]), tree(9, &[])], &[]), Keyed::TooLarge);
    let mut root = tree(1, &[(b"crates", 2)]);
    directory(&mut root, 0);
    assert_eq!(keyed(&[root, tree(9, &[])], &[copy_of(b"crates/a")]), Keyed::TooLarge);
}

#[test]
fn ignored_paths_do_not_consume_the_filtered_paths_budget() {
    let mut root = tree(1, &[(b"docs", 2)]);
    directory(&mut root, 0);
    let empty = tree(2, &[(b"README.md", 3)]);
    let limits = KeyLimits { entries: 0, path_bytes: 0 };
    let expected = key(REPOSITORY, b"gate", b"definition", &[], ROOT, &[tree(1, &[])], &limits);
    assert_eq!(key(REPOSITORY, b"gate", b"definition", &[copy_of(b"docs")], ROOT, &[root.clone()], &limits), expected);
    assert_eq!(
        key(REPOSITORY, b"gate", b"definition", &[copy_of(b"docs/README.md")], ROOT, &[root, empty], &limits),
        expected
    );
}
#[test]
fn modes_and_whole_subtree_ids_are_part_of_the_key() {
    let mut root = tree(1, &[(b"src", 2)]);
    directory(&mut root, 0);
    let original = complete(keyed(&[root.clone()], &[]));
    root.entries[0].object = [3; 32];
    assert_ne!(original, complete(keyed(&[root.clone()], &[])));
    root.entries[0].object = [2; 32];
    root.entries[0].mode = 0o100_755;
    assert_ne!(original, complete(keyed(&[root], &[])));
}
