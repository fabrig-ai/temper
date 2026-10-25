use super::fixtures;
use crate::{
    invocation::{self, Command, Source},
    limits::{self, Unfit},
};
use alloc::boxed::Box;
use skein_lib::{Token, bytes};
use temper_worker_domain_checkout::git;

// Literal settings make the expected vector independent of the builder.
const SETTINGS: &[&[u8]] = &[
    b"-c",
    b"core.hooksPath=/dev/null",
    b"-c",
    b"core.fsmonitor=false",
    b"-c",
    b"protocol.version=2",
    b"-c",
    b"protocol.allow=never",
    b"-c",
    b"protocol.https.allow=always",
    b"-c",
    b"protocol.http.allow=always",
    b"-c",
    b"credential.helper=",
    b"-c",
    b"http.sslVerify=true",
    b"-c",
    b"gc.auto=0",
    b"-c",
    b"maintenance.auto=false",
    b"-c",
    b"rerere.enabled=false",
    b"-c",
    b"merge.conflictStyle=merge",
    b"-c",
    b"submodule.recurse=false",
];
const ID: &[u8] = b"abababababababababababababababababababab";
fn rows() -> [(Command, &'static [&'static [u8]]); 17] {
    let remote: Box<[u8]> = Box::from(fixtures::REMOTE);
    let branch: Box<[u8]> = Box::from(&b"temper/7"[..]);
    let commit = fixtures::COMMIT;
    [
        (Command::Version, &[b"version"]),
        (
            Command::Clone { remote: remote.clone(), directory: Box::from(&b"app"[..]) },
            &[b"clone", b"--no-checkout", b"--no-tags", b"--origin", b"origin", b"--", fixtures::REMOTE, b"app"],
        ),
        (
            Command::Fetch { remote: remote.clone(), source: Source::Branch(branch.clone()) },
            &[
                b"fetch",
                b"--porcelain",
                b"--verbose",
                b"--no-tags",
                b"--no-write-fetch-head",
                b"--no-recurse-submodules",
                b"--",
                fixtures::REMOTE,
                b"+refs/heads/temper/7:refs/temper/fetched",
            ],
        ),
        (
            Command::Fetch { remote: remote.clone(), source: Source::Commit(commit) },
            &[
                b"fetch",
                b"--porcelain",
                b"--verbose",
                b"--no-tags",
                b"--no-write-fetch-head",
                b"--no-recurse-submodules",
                b"--",
                fixtures::REMOTE,
                b"+abababababababababababababababababababab:refs/temper/fetched",
            ],
        ),
        (
            Command::Fetch { remote: remote.clone(), source: Source::Default },
            &[
                b"fetch",
                b"--porcelain",
                b"--verbose",
                b"--no-tags",
                b"--no-write-fetch-head",
                b"--no-recurse-submodules",
                b"--",
                fixtures::REMOTE,
                b"+HEAD:refs/temper/fetched",
            ],
        ),
        (
            Command::Create { remote: remote.clone(), branch: branch.clone(), commit },
            &[
                b"push",
                b"--porcelain",
                b"--force-with-lease=refs/heads/temper/7:",
                b"--",
                fixtures::REMOTE,
                b"abababababababababababababababababababab:refs/heads/temper/7",
            ],
        ),
        (Command::CheckOut { commit }, &[b"checkout", b"--force", b"--detach", ID]),
        (Command::Clean, &[b"clean", b"-ffdxq"]),
        (Command::Merge { theirs: commit }, &[b"merge", b"--no-commit", b"--no-ff", b"--", ID]),
        (Command::Unmerged, &[b"diff-files", b"--name-only", b"-z", b"--diff-filter=U"]),
        (Command::Add, &[b"add", b"--all"]),
        (Command::DiffIndex { parent: commit }, &[b"diff-index", b"--cached", b"--quiet", ID, b"--"]),
        (Command::WriteTree, &[b"write-tree"]),
        (
            Command::CommitTree { tree: commit, parent: commit, merging: None },
            &[b"commit-tree", ID, b"-p", ID, b"-F", b"-"],
        ),
        (
            Command::CommitTree { tree: commit, parent: commit, merging: Some(commit) },
            &[b"commit-tree", ID, b"-p", ID, b"-p", ID, b"-F", b"-"],
        ),
        (Command::IsAncestor { expected: commit, commit }, &[b"merge-base", b"--is-ancestor", ID, ID]),
        (
            Command::Push { remote, commit, branch, expected: None },
            &[
                b"push",
                b"--porcelain",
                b"--",
                fixtures::REMOTE,
                b"abababababababababababababababababababab:refs/heads/temper/7",
            ],
        ),
    ]
}
#[test]
fn each_command_s_arguments_are_git_md_s_vector() {
    for (command, tail) in rows().into_iter().chain([(
        Command::Push {
            remote: Box::from(fixtures::REMOTE),
            commit: fixtures::COMMIT,
            branch: Box::from(&b"temper/7"[..]),
            expected: Some(fixtures::COMMIT),
        },
        &[
            &b"push"[..],
            b"--porcelain",
            b"--force-with-lease=refs/heads/temper/7:abababababababababababababababababababab",
            b"--",
            fixtures::REMOTE,
            b"abababababababababababababababababababab:refs/heads/temper/7",
        ][..],
    )]) {
        let actual = invocation::arguments(&command);
        assert_eq!(actual.len(), SETTINGS.len() + tail.len(), "{command:?}");
        for (got, expected) in actual.iter().zip(SETTINGS.iter().chain(tail.iter())) {
            assert_eq!(&**got, *expected, "{command:?}");
        }
    }
    for (workspace, expected) in [
        (0, b"workspaces/0/app".as_slice()),
        (7, b"workspaces/7/app"),
        (u64::MAX, b"workspaces/18446744073709551615/app"),
    ] {
        assert_eq!(
            &*invocation::place(&git::Place {
                git: None,
                workspace: Token::new(workspace),
                repository: Box::from(&b"app"[..])
            }),
            expected
        );
    }
}
#[test]
fn a_remote_operation_carries_the_token_only_in_its_header() {
    let identity = fixtures::identity();
    for kind in [git::Kind::Clone, git::Kind::Fetch, git::Kind::Create, git::Kind::Push] {
        let environment = invocation::environment(fixtures::HOME, Some(kind), Some(&identity));
        assert_eq!(environment.len(), 12);
        for expected in [
            &b"GIT_CONFIG_COUNT=1"[..],
            b"GIT_CONFIG_KEY_0=http.extraHeader",
            b"GIT_CONFIG_VALUE_0=Authorization: Basic dGVtcGVyLWJvdDp0b2stMQ==",
        ] {
            assert!(environment.contains(&Box::from(expected)), "{kind:?}: {expected:?}");
        }
        for variable in &environment {
            assert_eq!(bytes::find(variable, b"tok-1"), None);
            assert!(!variable.starts_with(b"PATH="));
        }
    }
    for (command, _) in rows() {
        for argument in &invocation::arguments(&command) {
            assert_eq!(bytes::find(argument, b"tok-1"), None, "{command:?}");
        }
    }
}
#[test]
fn a_commit_is_authored_and_committed_as_the_identity() {
    let identity = fixtures::identity();
    let environment = invocation::environment(fixtures::HOME, Some(git::Kind::Commit), Some(&identity));
    let expected = [
        b"HOME=/srv/worker/home".as_slice(),
        b"LC_ALL=C",
        b"TZ=UTC",
        b"GIT_CONFIG_NOSYSTEM=1",
        b"GIT_CONFIG_GLOBAL=/dev/null",
        b"GIT_TERMINAL_PROMPT=0",
        b"GIT_ASKPASS=",
        b"GIT_COMMITTER_NAME=Temper Bot",
        b"GIT_COMMITTER_EMAIL=bot@example.test",
        b"GIT_AUTHOR_NAME=Temper Bot",
        b"GIT_AUTHOR_EMAIL=bot@example.test",
    ];
    assert_eq!(environment.len(), expected.len());
    for (actual, expected) in environment.iter().zip(expected) {
        assert_eq!(&**actual, expected);
    }
}
#[test]
fn an_operation_naming_no_identity_commits_as_the_placeholder() {
    let identity = fixtures::identity();
    for kind in
        [None, Some(git::Kind::Make), Some(git::Kind::CheckOut), Some(git::Kind::Merge), Some(git::Kind::IsAncestor)]
    {
        let environment = invocation::environment(fixtures::HOME, kind, None);
        let expected = [
            b"HOME=/srv/worker/home".as_slice(),
            b"LC_ALL=C",
            b"TZ=UTC",
            b"GIT_CONFIG_NOSYSTEM=1",
            b"GIT_CONFIG_GLOBAL=/dev/null",
            b"GIT_TERMINAL_PROMPT=0",
            b"GIT_ASKPASS=",
            b"GIT_COMMITTER_NAME=temper",
            b"GIT_COMMITTER_EMAIL=temper@temper.invalid",
        ];
        assert_eq!(environment.len(), expected.len());
        for (actual, expected) in environment.iter().zip(expected) {
            assert_eq!(&**actual, expected, "{kind:?}");
        }
        assert_eq!(invocation::environment(fixtures::HOME, kind, Some(&identity)), environment);
    }
}
#[test]
fn the_message_is_title_blank_line_body_and_newline() {
    for (body, expected) in [(b"".as_slice(), b"Title\n".as_slice()), (b"Body", b"Title\n\nBody\n")] {
        assert_eq!(&*invocation::message(b"Title", body), expected, "{body:?}");
    }
}
#[test]
fn ids_of_forty_and_sixty_four_digits_convert_both_ways() {
    for (digits, expected) in [
        (ID, Some(fixtures::COMMIT)),
        (b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd", Some(fixtures::COMMIT_256)),
        (b"abababababababababababababababababababa", None),
        (b"ABABABABABABABABABABABABABABABABABABABAB", None),
        (b"abababababababababababababababababababag", None),
    ] {
        assert_eq!(invocation::object(digits), expected, "{digits:?}");
        if let Some(commit) = expected {
            assert_eq!(&*invocation::hex(commit), digits);
        }
    }
}
#[test]
fn limits_breaking_git_md_s_rules_are_refused_naming_the_limit() {
    let valid = fixtures::limits();
    assert_eq!(limits::check(&valid, &fixtures::checkout()), Ok(()));
    for (limits, expected) in [
        (crate::Limits { operations: 1, ..valid }, Unfit::Operations),
        (crate::Limits { stdout_bytes: 0, ..valid }, Unfit::StdoutBytes),
        (crate::Limits { stderr_bytes: 511, ..valid }, Unfit::StderrBytes),
        (crate::Limits { read_bytes: 0, ..valid }, Unfit::ReadBytes),
        (crate::Limits { sweep_entries: 0, ..valid }, Unfit::SweepEntries),
        (crate::Limits { sweep_bytes: 254, ..valid }, Unfit::SweepBytes),
    ] {
        assert_eq!(limits::check(&limits, &fixtures::checkout()), Err(expected), "{expected:?}");
    }
    assert_eq!(limits::largest_output(&fixtures::checkout()), Some(384));
    let mut checkout = fixtures::checkout();
    checkout.conflicts = u32::MAX;
    assert_eq!(limits::largest_output(&checkout), None);
    assert_eq!(limits::check(&valid, &checkout), Err(Unfit::StdoutBytes));
}
#[test]
fn the_identity_s_debug_shows_no_token_byte() {
    let printed = alloc::fmt::format(format_args!("{:?}", fixtures::identity()));
    assert!(printed.contains("Secret { len: 5 }"));
    assert!(!printed.contains("tok-1"));
    assert!(!printed.contains("116, 111, 107, 45, 49"));
}
