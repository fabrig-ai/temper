//! The fake's hosted file operations against the real kernel.
use skein_world::domain::heap::Counting;
use temper_fake_git_fixtures as fixtures;
use temper_real_loop_world::git::Disk;
use temper_worker_domain_checkout::git;
use temper_worker_protocol_git::invocation::{self, Command};
#[global_allocator]
static HEAP: Counting = Counting;

// The transcript's base and topic commits are counted objects 1 and 2.
const BASE: u64 = 1;
const TOPIC: u64 = 2;
// The forge assigns the next two object numbers after the base and topic.
const FIRST_COMMIT: u64 = 3;
const SECOND_COMMIT: u64 = 4;
// The base tree contains only conflict.txt.
const BASE_TREE_FILES: u32 = 1;
// Fit the large staged file in a single forge object.
const BODY_BYTES: u32 = 8192;
// Cross a read chunk, an input chunk and a directory-list page respectively.
const FILE_BYTES: usize = 5000;
const MESSAGE_BYTES: usize = 4100;
const NEW_FILES: usize = 12;
// One original file and the twelve additions in the transcript.
const TREE_FILES: u32 = 13;

fn commit(number: u64) -> git::Commit {
    invocation::object(&temper_fake_git_hosted::id::id(number)).expect("counted id")
}

#[test]
fn hosted_clone_creates_only_the_destination() {
    let scenario = fixtures::scenario();
    let command = Command::Clone { remote: scenario.remote.into(), directory: b"copy".as_slice().into() };
    let mut disk = Disk::new(fixtures::scenario);
    let result = disk.run(&command, b"");
    assert_eq!((result.exit, result.stdout, result.stderr), (0, vec![], vec![]));
    assert_eq!(disk.read(b"copy/.git/objects/1"), b"");
    assert_eq!(disk.read(b"copy/.git/objects/2"), b"");
    assert_eq!(disk.read(b"conflict.txt"), b"base\n");
    assert!(!disk.exists(b"copy/conflict.txt"));
}

fn large_body() -> fixtures::Scenario {
    let scenario = fixtures::scenario();
    scenario.transport.forge.borrow_mut().config.limits.body_bytes = BODY_BYTES;
    scenario
}

#[test]
fn hosted_git_commits_the_staged_real_files() {
    let mut disk = Disk::new(large_body);
    disk.write(b"conflict.txt", &vec![b'x'; FILE_BYTES]);
    disk.write(b".git/MERGE_HEAD", &TOPIC.to_le_bytes());
    for number in 0..NEW_FILES {
        disk.write(format!("new-{number}").as_bytes(), b"new\n");
    }
    disk.run(&Command::Add, b"");
    let tree = disk.run(&Command::WriteTree, b"");
    let tree_id = invocation::object(tree.stdout.strip_suffix(b"\n").expect("one id")).expect("tree id");
    disk.write(b"conflict.txt", b"unstaged\n");
    let message = vec![b'm'; MESSAGE_BYTES];
    let result =
        disk.run(&Command::CommitTree { tree: tree_id, parent: commit(BASE), merging: Some(commit(TOPIC)) }, &message);
    assert_eq!(result.exit, 0);
    assert_eq!(result.stdout, b"1d0b14e4db018fed000000000000000300000000\n");
    assert!(result.stderr.is_empty());
    let forge = disk.forge();
    let object = forge.domain.object(FIRST_COMMIT).expect("new commit");
    assert_eq!((object.parent, object.merge_parent), (Some(BASE), Some(TOPIC)));
    assert_eq!(object.tree.get(b"conflict.txt".as_slice()).expect("staged file").as_ref(), vec![b'x'; FILE_BYTES]);
    assert_eq!(object.tree.len(), TREE_FILES);
    assert_eq!(object.message.as_ref(), message);
    assert_eq!(disk.read(b".git/temper-head"), BASE.to_le_bytes());
    assert_eq!(disk.read(b".git/MERGE_HEAD"), TOPIC.to_le_bytes());
    assert_eq!(disk.read(b".git/objects/3"), b"");
}

#[test]
fn hosted_clean_after_checkout_leaves_only_the_commits_files() {
    let mut disk = Disk::new(fixtures::scenario);
    disk.write(b"conflict.txt", &vec![b'x'; FILE_BYTES]);
    disk.write(b"untracked/deep/file", b"remove\n");
    disk.write(b".git/keep", b"metadata\n");
    disk.run(&Command::CheckOut { commit: commit(TOPIC) }, b"");
    let cleaned = disk.run(&Command::Clean, b"");
    assert_eq!(cleaned.exit, 0);
    assert_eq!(disk.read(b"conflict.txt"), b"topic\n");
    assert_eq!(disk.read(b".git/keep"), b"metadata\n");
    assert!(!disk.exists(b"untracked"), "a nested untracked directory is removed");
}

#[test]
fn hosted_commit_tree_keeps_exact_parents() {
    let mut disk = Disk::new(fixtures::scenario);
    let tree = disk.run(&Command::WriteTree, b"");
    let tree = invocation::object(tree.stdout.strip_suffix(b"\n").expect("one tree id")).expect("tree id");
    let rows = [
        (None, b"1d0b14e4db018fed000000000000000300000000\n".as_slice(), FIRST_COMMIT, None),
        (Some(commit(TOPIC)), b"6e73e372e2338aca000000000000000400000000\n".as_slice(), SECOND_COMMIT, Some(TOPIC)),
    ];
    for (merging, expected, number, second) in rows {
        let result = disk.run(&Command::CommitTree { tree, parent: commit(BASE), merging }, b"unchanged\n");
        assert_eq!(result.exit, 0, "commit {number}: exit");
        assert_eq!(result.stdout, expected, "commit {number}: id");
        let forge = disk.forge();
        let object = forge.domain.object(number).expect("new unchanged commit");
        assert_eq!((object.parent, object.merge_parent), (Some(BASE), second), "commit {number}: parents");
        assert_eq!(object.tree.len(), BASE_TREE_FILES, "commit {number}: tree files");
        assert_eq!(
            object.tree.get(b"conflict.txt".as_slice()).expect("base file").as_ref(),
            b"base\n",
            "commit {number}: tree"
        );
        assert_eq!(disk.read(b".git/temper-head"), BASE.to_le_bytes(), "commit {number}: HEAD");
    }
}
