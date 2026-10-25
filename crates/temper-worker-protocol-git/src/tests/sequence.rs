use super::fixtures;
use crate::{
    invocation::Command,
    sequence::{self, Next},
};
use alloc::boxed::Box;
use temper_worker_domain_checkout::git;
#[test]
fn each_operation_s_next_invocation_is_git_md_s() {
    standard_rows();
    let create = git::Op::Create {
        at: fixtures::at(),
        remote: Box::from(fixtures::REMOTE),
        branch: Box::from(&b"main"[..]),
        commit: fixtures::COMMIT,
        identity: 1,
        account: None,
    };
    let ran = Command::Create {
        remote: Box::from(fixtures::REMOTE),
        branch: Box::from(&b"main"[..]),
        commit: fixtures::COMMIT,
    };
    for (stdout, expected) in [
        (
            b"*\tabababababababababababababababababababab:refs/heads/main\t[new branch]\n".as_slice(),
            git::Done::Succeeded,
        ),
        (b"=\tabababababababababababababababababababab:refs/heads/main\t[up to date]\n", git::Done::Exists),
        (b"!\tabababababababababababababababababababab:refs/heads/main\t[rejected] (stale info)\n", git::Done::Exists),
        (
            b"!\tabababababababababababababababababababab:refs/heads/main\t[remote rejected] (hook declined)\n",
            git::Done::FailedWithOutput { fault: git::Fault::Refused, diagnostic: git::PushDiagnostic::empty() },
        ),
    ] {
        assert_eq!(sequence::after(&create, &ran, 1, stdout, b"", 0, &fixtures::checkout()), Next::End(expected));
    }
}

fn standard_rows() {
    let commit = fixtures::commit_op(None);
    let push = fixtures::push_op(Some(fixtures::EXPECTED));
    let merge = git::Op::Merge { at: fixtures::at(), theirs: fixtures::COMMIT };
    let fetch = git::Op::Fetch {
        at: fixtures::at(),
        remote: Box::from(fixtures::REMOTE),
        want: git::Want::Default,
        identity: 1,
        account: None,
    };
    let rows = [
        (&commit,Command::DiffIndex { parent:fixtures::PARENT },0,b"".as_slice(),Next::End(git::Done::Unchanged)),
        (&commit,Command::DiffIndex { parent:fixtures::PARENT },1,b"",Next::Run(Command::WriteTree)),
        (&push,Command::IsAncestor { expected:fixtures::EXPECTED,commit:fixtures::COMMIT },1,b"",Next::End(git::Done::Rejected)),
        (&merge,Command::Merge { theirs:fixtures::COMMIT },0,b"",Next::End(git::Done::Merged)),
        (&merge,Command::Merge { theirs:fixtures::COMMIT },1,b"",Next::Run(Command::Unmerged)),
        (&merge,Command::Unmerged,0,b"conflict\0",Next::End(git::Done::Conflicted { files:Box::from([Box::from(&b"conflict"[..])]) })),
        (&merge,Command::Unmerged,0,b"",Next::End(git::Done::FailedWithOutput { fault:git::Fault::Broken,diagnostic:git::PushDiagnostic::empty() })),
        (&fetch,Command::Fetch { remote:Box::from(fixtures::REMOTE),source:crate::invocation::Source::Default },0,b"* 0000000000000000000000000000000000000000 abababababababababababababababababababab refs/temper/fetched\n",Next::End(git::Done::Fetched { commit:fixtures::COMMIT })),
    ];
    for (op, ran, code, stdout, next) in rows {
        assert_eq!(sequence::after(op, &ran, code, stdout, b"", 0, &fixtures::checkout()), next, "{ran:?}: {code}");
    }
}

#[test]
fn standalone_ancestry_exits_are_booleans_and_faults_keep_their_diagnostic() {
    let op = git::Op::IsAncestor { at: fixtures::at(), ancestor: fixtures::PARENT, commit: fixtures::COMMIT };
    let command = Command::IsAncestor { expected: fixtures::PARENT, commit: fixtures::COMMIT };
    assert_eq!(sequence::first(&op), Next::Run(command.clone()));
    for (code, done) in [
        (0, git::Done::IsAncestor { contains: true }),
        (1, git::Done::IsAncestor { contains: false }),
        (
            128,
            git::Done::FailedWithOutput {
                fault: git::Fault::Broken,
                diagnostic: git::PushDiagnostic::new(b"bad object", 3),
            },
        ),
    ] {
        assert_eq!(sequence::after(&op, &command, code, b"", b"bad object", 3, &fixtures::checkout()), Next::End(done));
    }
}
