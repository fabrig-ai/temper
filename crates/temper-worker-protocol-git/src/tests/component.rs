use super::fixtures;
use crate::{Event, Request, Unusable, Version};
use skein_io::kernel;
use temper_worker_domain_checkout::git;
const FETCHED: &[u8] =
    b"* 0000000000000000000000000000000000000000 abababababababababababababababababababab refs/temper/fetched\n";
#[test]
fn a_fetch_runs_one_invocation_and_ends_fetched() {
    let mut component = fixtures::Started::new();
    component.down(fixtures::fetch(fixtures::OWNER, fixtures::DEADLINE));
    let (child, pipes) = component.spawned();
    component.stdout(pipes[0], FETCHED);
    component.exited(child, 0);
    component.end(pipes[0]);
    component.end(pipes[1]);
    assert_eq!(component.done(), None);
    component.closed(child, &pipes);
    assert_eq!(component.done(), Some((fixtures::OWNER, git::Done::Fetched { commit: fixtures::COMMIT })));
    assert!(component.below.out.io.is_empty());
    component.shutdown();
    assert!(component.component.is_empty());
}
#[test]
fn a_deadline_kills_the_group_and_ends_timed_out_after_the_close() {
    let mut component = fixtures::Started::new();
    component.down(fixtures::fetch(fixtures::OWNER, fixtures::DEADLINE));
    let (child, pipes) = component.spawned();
    component.fire(fixtures::DEADLINE);
    assert_eq!(&*component.signals(), &[(child, kernel::Signal::Kill, kernel::Target::Group)]);
    assert_eq!(component.done(), None);
    component.below.out.io = skein_lib::Queue::with_capacity(crate::MAX_OUT.io);
    component.component.up(
        component.now,
        skein_io::Event::Stream { owner: pipes[0], up: skein_lib::stream::Up::Failed(skein_lib::stream::Fault::Other) },
        &mut component.below.out,
    );
    assert!(component.below.out.io.is_empty(), "a pipe failure after the deadline issues no second close");
    component.closed(child, &pipes);
    assert_eq!(
        component.done(),
        Some((
            fixtures::OWNER,
            git::Done::FailedWithOutput { fault: git::Fault::TimedOut, diagnostic: git::PushDiagnostic::empty() }
        ))
    );
}
#[test]
fn a_cancel_after_the_last_exit_ends_with_the_operation_s_outcome() {
    let mut component = fixtures::Started::new();
    component.down(fixtures::fetch(fixtures::OWNER, fixtures::DEADLINE));
    let (child, pipes) = component.spawned();
    component.stdout(pipes[0], FETCHED);
    component.end(pipes[0]);
    component.end(pipes[1]);
    component.exited(child, 0);
    component.down(Request::CancelIo { owner: fixtures::OWNER });
    assert!(component.signals().is_empty());
    component.closed(child, &pipes);
    assert_eq!(component.done(), Some((fixtures::OWNER, git::Done::Fetched { commit: fixtures::COMMIT })));
}
#[test]
fn a_cancel_between_invocations_starts_no_further_one() {
    let mut component = fixtures::Started::new();
    component.down(Request::Io {
        owner: fixtures::OWNER,
        op: git::Op::CheckOut { at: fixtures::at(), commit: fixtures::COMMIT },
        deadline: fixtures::DEADLINE,
        identity: None,
    });
    let (child, pipes) = component.spawned();
    component.end(pipes[0]);
    component.end(pipes[1]);
    component.exited(child, 0);
    component.down(Request::CancelIo { owner: fixtures::OWNER });
    component.closed(child, &pipes);
    assert!(component.below.out.io.is_empty());
    assert_eq!(
        component.done(),
        Some((
            fixtures::OWNER,
            git::Done::FailedWithOutput { fault: git::Fault::Cancelled, diagnostic: git::PushDiagnostic::empty() }
        ))
    );
}
#[test]
fn stdout_past_its_cap_is_read_dropped_and_ends_broken() {
    let mut component = fixtures::Started::new();
    component.down(fixtures::fetch(fixtures::OWNER, fixtures::DEADLINE));
    let (child, pipes) = component.spawned();
    component.stdout(pipes[0], &[b'x'; 513]);
    assert!(!component.below.out.io.is_empty(), "reading continues past the cap");
    component.end(pipes[0]);
    component.end(pipes[1]);
    component.exited(child, 0);
    component.closed(child, &pipes);
    assert_eq!(
        component.done(),
        Some((
            fixtures::OWNER,
            git::Done::FailedWithOutput { fault: git::Fault::Broken, diagnostic: git::PushDiagnostic::empty() }
        ))
    );
}
#[test]
fn an_operation_whose_identity_has_no_value_fails_broken_unspawned() {
    let mut component = fixtures::Started::new();
    component.down(Request::Io {
        owner: fixtures::OWNER,
        op: fixtures::push_op(None),
        deadline: fixtures::DEADLINE,
        identity: None,
    });
    assert!(component.below.out.io.is_empty());
    assert_eq!(component.done(), Some((fixtures::OWNER, git::Done::Failed { fault: git::Fault::Broken })));
}
#[test]
fn a_git_older_than_two_forty_one_makes_the_start_unusable() {
    let mut component = fixtures::Started::unstarted();
    component.component.start(component.now, &mut component.below.out);
    let (child, pipes) = component.spawned();
    component.stdout(pipes[0], b"git version 2.40.9\n");
    component.end(pipes[0]);
    component.end(pipes[1]);
    component.exited(child, 0);
    component.closed(child, &pipes);
    assert_eq!(
        component.below.out.events.pop(),
        Some(Event::Unusable { why: Unusable::Version { found: Some(Version { major: 2, minor: 40, patch: 9 }) } })
    );
    assert!(component.component.is_empty());
}
#[test]
fn an_abort_during_commit_tree_spawn_settles_without_a_done() {
    let mut component = fixtures::Started::new();
    component.down(Request::Io {
        owner: fixtures::OWNER,
        op: fixtures::commit_op(None),
        deadline: fixtures::DEADLINE,
        identity: Some(fixtures::identity()),
    });
    for (code, stdout) in [(0, b"".as_slice()), (1, b""), (0, b"abababababababababababababababababababab\n")] {
        let (child, pipes) = component.spawned();
        component.stdout(pipes[0], stdout);
        component.end(pipes[0]);
        component.end(pipes[1]);
        component.exited(child, code);
        component.closed(child, &pipes);
    }
    component.component.abort(&mut component.below.out);
    let (child, pipes) = component.spawned();
    assert_eq!(pipes.len(), 3, "commit-tree's pending stdin pipe is still known");
    assert_eq!(component.below.out.io.len(), 4, "abort child and all three pipes");
    component.closed(child, &pipes);
    assert_eq!(component.done(), None);
    for _ in 0_u8..2 {
        component.file_operation();
        component.complete_ok();
    }
    assert!(component.component.is_empty());
}

#[test]
fn standalone_ancestry_needs_no_identity_and_cancels_after_its_child_closes() {
    let mut component = fixtures::Started::new();
    component.down(Request::Io {
        owner: fixtures::OWNER,
        op: git::Op::IsAncestor { at: fixtures::at(), ancestor: fixtures::PARENT, commit: fixtures::COMMIT },
        deadline: fixtures::DEADLINE,
        identity: None,
    });
    let (child, pipes) = component.spawned();
    component.down(Request::CancelIo { owner: fixtures::OWNER });
    assert_eq!(&*component.signals(), &[(child, kernel::Signal::Kill, kernel::Target::Group)]);
    assert_eq!(component.done(), None);
    component.closed(child, &pipes);
    assert_eq!(
        component.done(),
        Some((
            fixtures::OWNER,
            git::Done::FailedWithOutput { fault: git::Fault::Cancelled, diagnostic: git::PushDiagnostic::empty() }
        ))
    );
}
