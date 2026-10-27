use skein_fake_peers::forge::Delivery;
use skein_world::domain::Verdict;
use temper_one_process_world::referee::{restart, secrets};
use temper_one_process_world::{fixtures, world};
#[test]
fn a_signal_mid_run_drains_and_the_next_start_settles_the_claims() {
    let outcome = world::drain_mid_run(fixtures::calm(fixtures::SEED), true);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.drained);
    assert_eq!(outcome.restarts, 1);
    assert_eq!(outcome.claims_settled_at_restart, 1);
}
#[test]
fn a_secret_in_a_root_fails_the_referee_naming_it() {
    let root = format!("root {:?}", fixtures::LLM_TOKEN);
    for (name, bytes, expected) in [
        ("formatted root", root.into_bytes(), Some("LLM access token")),
        ("binary frame", [b"\xff".as_slice(), fixtures::LLM_TOKEN, b"\x80"].concat(), Some("LLM access token")),
        ("interrupted token", [b"test-\xfftoken".as_slice()].concat(), None),
    ] {
        assert_eq!(secrets::in_bytes(&bytes), expected, "{name}");
    }
}
#[test]
fn a_restart_showing_part_of_a_commit_fails_the_referee() {
    let records = fixtures::two_forge_records();
    let uncertain: restart::Records =
        records.iter().map(|row| (temper_engine_domain_forge::stored_key(row), row.clone())).collect();
    let partial: restart::Records =
        records.iter().take(1).map(|row| (temper_engine_domain_forge::stored_key(row), row.clone())).collect();
    assert_eq!(
        restart::check(&restart::Records::new(), Some(&uncertain), &partial),
        Err(temper_engine_domain_forge::Key::Names(2))
    );
}
#[test]
fn a_person_signs_in_by_the_printed_link_and_a_chat_is_answered() {
    let outcome = world::run(fixtures::calm(fixtures::SEED), world::Story::Chat { words: b"Hello" });
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.signed_in, 1);
    assert_eq!(outcome.chats_answered, 1);
}
#[test]
fn a_person_keeps_the_sign_in_held_by_a_fresh_printed_link() {
    let starts = world::sign_in_again(fixtures::calm(fixtures::SEED));
    for outcome in starts {
        assert_eq!(outcome.verdict, Verdict::Passed);
        assert_eq!(outcome.signed_in, 1);
    }
}
#[test]
fn an_adopted_repository_gets_its_change_pushed_through_git() {
    let outcome = world::run(fixtures::calm(fixtures::SEED), world::Story::Adopt);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert_eq!(outcome.adopted, 1);
    assert_eq!(outcome.pushed.len(), 1);
    assert_eq!(outcome.deliveries, [Delivery::NoAddress, Delivery::Written]);
}

#[test]
fn a_second_run_pushes_its_own_agents_edit() {
    let outcome = world::run(fixtures::calm(fixtures::SEED), world::Story::TwoChanges);
    assert_eq!(outcome.verdict, Verdict::Passed);
    let [first, second] = outcome.pushed.as_slice() else { panic!("two pushes: {:?}", outcome.pushed) };
    for (push, content) in [(first, b"first\n".as_slice()), (second, b"second\n".as_slice())] {
        assert_eq!(push.tree, std::collections::BTreeMap::from([(b"conflict.txt".to_vec(), content.to_vec())]));
    }
    assert_ne!(first.branch, second.branch);
}
