//! Change delegates spell their pinned commits as task text parameters.
use super::*;

#[path = "delegates/fixtures.rs"]
pub(super) mod fixtures;

use fixtures::{BASE_COMMIT, INITIAL_COMMIT, OWNER, PUSHER, answer_producer, assert_commit, next_delegate};

const REVIEW_PASSES: u32 = 1000;
const RESOLVE_COMMIT: u32 = 6;
const GATE_COMMIT: u32 = 8;
const BASE_DIGITS: &[u8] = b"1d0b14e4db018fed000000000000000300000000";
const HEAD_DIGITS: &[u8] = b"975835de1c9756ce000000000000000200000000";

#[test]
fn a_resolution_delegate_carries_its_base_as_hexadecimal_text() {
    let (mut world, _chat, producer, branch) = change_world(false, REVIEW_PASSES, None);
    assert_eq!(
        world.external(PUSHER, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: INITIAL_COMMIT })),
        raw::Answer::Branch(raw::Created::Created)
    );
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"branch change", OWNER)
        .expect("producer pushed branch");
    assert_eq!(
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"file", b"base change", OWNER),
        Ok(BASE_COMMIT)
    );
    answer_producer(&mut world, &producer);
    let resolver = next_delegate(&mut world);
    assert_commit(&world, resolver.task, RESOLVE_COMMIT, BASE_DIGITS);
}

#[test]
fn a_gate_delegate_carries_its_head_as_hexadecimal_text() {
    let (mut world, _chat, producer, branch) = change_world_with_gate(false, REVIEW_PASSES, None, true);
    assert_eq!(
        world.external(PUSHER, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: INITIAL_COMMIT })),
        raw::Answer::Branch(raw::Created::Created)
    );
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"review this diff", OWNER)
        .expect("producer pushed review head");
    answer_producer(&mut world, &producer);
    let gate = next_delegate(&mut world);
    assert_commit(&world, gate.task, GATE_COMMIT, HEAD_DIGITS);
}
