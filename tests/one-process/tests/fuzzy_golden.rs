use skein_fake_forge_domain::{Observation, api};
use temper_one_process_world::{fixtures, golden::golden, world};

const REPOSITORY_NAME: &[u8] = b"acme/app";
// Base 1 and topic 2 are seeded; the producer makes 3, and the merge makes 4.
const MERGE: u64 = 4;
const MERGES: usize = 1;

#[test]
fn the_golden_story_is_deterministic() {
    let (first, ledger) = golden();
    let (second, again) = golden();
    assert_eq!(first, second, "all deployment entries and startup inputs are byte identical; no fields excluded");
    assert_eq!(ledger.effects, again.effects);
    assert_eq!(ledger.calls, again.calls);
    assert_eq!(ledger.requests, again.requests);
    assert_eq!(ledger.answers, again.answers);
}

#[test]
fn the_forge_ledger_survives_the_next_start_without_duplicate_effects() {
    let (deployment, ledger) = golden();
    let effects = ledger.effects.clone();
    let calls = ledger.calls.clone();
    let requests = ledger.requests.clone();
    let (_, after) = world::over_with_ledger(&deployment, fixtures::calm(fixtures::SEED), ledger).into_parts();
    assert_eq!(after.effects, effects, "a start adds no forge effects");
    assert_eq!(after.effects.iter().filter(|effect| matches!(effect, Observation::Merged { .. })).count(), MERGES);
    assert!(after.calls.starts_with(&calls), "old API calls stay in the ledger");
    assert!(after.requests.starts_with(&requests), "old HTTP heads stay in the ledger");
    assert!(
        after.calls[calls.len()..].iter().all(|call| matches!(call.op, api::Op::Read(_))),
        "a restart does not repeat API writes"
    );
    assert!(
        after.requests[requests.len()..].iter().all(|call| call.method == skein_http::Method::Get),
        "a restart does not repeat setup writes"
    );
    assert_eq!(
        after.inspect(REPOSITORY_NAME, &api::Read::Branch { branch: b"main".as_slice().into() }),
        Ok(api::Answer::Commit(MERGE)),
        "the next phase keeps the same outside forge"
    );
}
