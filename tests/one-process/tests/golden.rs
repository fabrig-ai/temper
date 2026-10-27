use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_protocol_store::secrets_v1::SourceValue;
use skein_fake_forge_domain::{Observation, api};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change::State;
use temper_one_process_world::{
    deployment::{Held, JigSecret},
    fixtures,
    golden::{self, golden},
    world,
};

// The configured project, printed owner's first sign-in, and the one added key.
const PROJECT: u32 = 1;
const PERSON: u64 = 1;
const SIGN_IN: u64 = 1;
const ACCOUNT: u32 = 1;
const SOURCE: u32 = 0;
const FORGE: u16 = 1;
const REPOSITORY: u32 = 2;
const REPOSITORY_NAME: &[u8] = b"acme/app";
// Base 1 and topic 2 are seeded; the producer makes 3, and the merge makes 4.
const MERGE: u64 = 4;
const MERGES: usize = 1;
// The configured project and role each retain the existing policy's allowance.
const POLICY_ALLOWANCE: u64 = 100;
// One account and source consume the fixture's numbers 0 and 1.
const NEXT_ACCOUNT: u32 = 2;
const ACCOUNT_BINDINGS: usize = 1;
// Sign-in reserves candidate 2 before matching the configured owner 1.
const PERSON_CANDIDATE: u64 = 2;

#[test]
fn the_golden_story_holds_each_part() {
    let (deployment, ledger) = golden();
    let held = Held::read(&deployment);
    assert_people_and_account(&held);
    assert_landing(&held, &ledger);
    assert_paused_chat(&held);
    assert_mid_run(&held);
    assert_forge_secrets(&held);
}

fn assert_people_and_account(held: &Held) {
    let core::Record::People(people::Stored::Policy { project, value, .. }) =
        &held.jig[&core::Key::People(people::Key::Policy(PROJECT))]
    else {
        panic!("project policy")
    };
    assert_eq!(*project, PROJECT);
    assert_eq!(value.period_spend, POLICY_ALLOWANCE);
    let core::Record::People(people::Stored::Person { number, identity }) =
        &held.jig[&core::Key::People(people::Key::Person(PERSON))]
    else {
        panic!("signed-in person")
    };
    assert_eq!(*number, PERSON);
    assert_eq!(identity.key.provider, 1);
    assert_eq!(identity.key.subject.as_ref(), b"owner-a");
    let core::Record::People(people::Stored::Roles { holdings, .. }) =
        &held.jig[&core::Key::People(people::Key::Roles(PROJECT))]
    else {
        panic!("project membership")
    };
    assert_eq!(holdings.as_ref(), &[people::Holding { person: PERSON, role: people::Role::Owner }]);
    assert!(matches!(
        held.jig[&core::Key::People(people::Key::SignIn(SIGN_IN))],
        core::Record::People(people::Stored::SignIn { number: SIGN_IN, person: PERSON, .. })
    ));
    assert!(matches!(
        held.jig[&core::Key::Accounts(accounts::Key::Account { number: ACCOUNT })],
        core::Record::Accounts(accounts::Stored::Account {
            number: ACCOUNT,
            provider: 1,
            holder: accounts::Holder::Deployment,
            ..
        })
    ));
    assert!(matches!(
        held.jig[&core::Key::Accounts(accounts::Key::Source { account: ACCOUNT, number: SOURCE })],
        core::Record::Accounts(accounts::Stored::Source {
            account: ACCOUNT,
            number: SOURCE,
            kind: accounts::Kind::Key,
            held: None,
            ..
        })
    ));
    let mut key = skein_kv::KeyWriter::new(u32::MAX);
    key.tag(3).tag(0).u64(u64::from(SOURCE));
    let JigSecret::Source(source) = &held.jig_secrets[&key.finish().expect("account key")] else {
        panic!("account source envelope")
    };
    let SourceValue::Key(key) = source.value() else { panic!("account key source") };
    assert_eq!(key.key().expose(), fixtures::LLM_TOKEN);
    assert_eq!(
        held.jig[&core::Key::Accounts(accounts::Key::Next)],
        core::Record::Accounts(accounts::Stored::Next { number: NEXT_ACCOUNT })
    );
    let core::Record::Core(core::CoreRecord::Deployment(numbers)) =
        &held.jig[&core::Key::Core(core::CoreKey::Deployment)]
    else {
        panic!("deployment counters")
    };
    assert_eq!(numbers.tasks, golden::MID_RUN);
    assert_eq!(numbers.projects, u64::from(PROJECT));
    assert_eq!(numbers.people, PERSON_CANDIDATE);
    assert_eq!(numbers.sign_ins, SIGN_IN);
}

fn assert_landing(held: &Held, ledger: &golden::Ledger) {
    let forge::Stored::Repository(repository) = &held.application
        [&forge::Key::Repository(skein_forge::api::Repository { forge: FORGE, repository: REPOSITORY })]
    else {
        panic!("adopted repository")
    };
    assert_eq!(repository.project, PROJECT);
    assert_eq!(repository.owner.as_ref(), b"acme");
    assert_eq!(repository.name.as_ref(), b"app");
    assert_eq!(repository.role, forge::Role::Owned);
    assert!(repository.ci);
    let forge::Stored::Change(change) = &held.application[&forge::Key::Change(golden::GOAL)] else {
        panic!("goal change")
    };
    let State::Landed { merge } = change.change.state else { panic!("goal's change landed") };
    assert_eq!(change.task, golden::GOAL);
    assert_eq!(change.pending, None);
    assert_eq!(change.base.as_ref(), b"main");
    assert!(matches!(held.application[&forge::Key::Landed(merge)], forge::Stored::Landed { task: golden::GOAL, .. }));
    let producer = task(held, golden::PRODUCER);
    assert_eq!(producer.requester, tasks::Party::Task(golden::GOAL));
    assert!(matches!(producer.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))));
    let goal = task(held, golden::GOAL);
    assert_eq!(goal.tracked, Some(1));
    assert_eq!(goal.requester, tasks::Party::Task(golden::CHAT));
    assert!(matches!(goal.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))));
    let merges: Vec<_> = ledger
        .effects
        .iter()
        .filter_map(|effect| {
            if let Observation::Merged { repository, base, commit, .. } = effect {
                Some((repository.as_ref(), base.as_ref(), *commit))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(merges, [(REPOSITORY_NAME, b"main".as_slice(), MERGE)]);
    assert_eq!(
        ledger.inspect(REPOSITORY_NAME, &api::Read::Branch { branch: b"main".as_slice().into() }),
        Ok(api::Answer::Commit(MERGE))
    );
    let mut decoded_merge = [0; 32];
    decoded_merge[..20].copy_from_slice(&skein_fake_forge_domain::object_bytes(MERGE));
    assert_eq!(merge, decoded_merge, "store and outside forge name the same merge");
}

fn assert_paused_chat(held: &Held) {
    let chat = task(held, golden::CHAT);
    assert_eq!(chat.project, PROJECT);
    assert_eq!(chat.requester, tasks::Party::Person(PERSON));
    assert!(chat.flags.conversation);
    assert_eq!(chat.numbers.budget, golden::CHAT_ALLOWANCE);
    assert_eq!(chat.phase, tasks::Phase::Active(tasks::Active::Idle));
    let turns = &held.turns[&golden::CHAT];
    assert!(turns.iter().any(|turn| turn.turn == 1 && turn.opens_window), "the chat's opening turn is kept");
    let proof = proof(held, golden::CHAT);
    assert!(matches!(proof.terminal.as_ref().map(|terminal| &terminal.end), Some(tasks::End::Parked)));
    assert!(turns.iter().any(|turn| turn.attempt == proof.attempt), "the paused attempt's turns are kept");
}

fn assert_mid_run(held: &Held) {
    let running = task(held, golden::MID_RUN);
    assert_eq!(running.spec.words.as_ref(), golden::MID_RUN_WORDS);
    assert_eq!(running.requester, tasks::Party::Person(PERSON));
    let tasks::Phase::Active(tasks::Active::Running { attempt }) = running.phase else { panic!("task mid-run") };
    let proof = proof(held, golden::MID_RUN);
    assert_eq!(proof.attempt, attempt);
    assert_eq!(proof.terminal, None, "the drained claim still awaits its terminal");
    assert_eq!(proof.accounts.len(), ACCOUNT_BINDINGS, "the claim keeps its account binding");
    assert_eq!(proof.accounts[0].account, ACCOUNT);
    assert_eq!(proof.accounts[0].source, SOURCE);
    assert!(held.turns[&golden::MID_RUN].iter().any(|turn| turn.attempt == attempt && turn.turn == 1));
    assert!(proof.turn.as_ref().is_some_and(|turn| turn.turn >= 1));
}

fn assert_forge_secrets(held: &Held) {
    use temper_protocol_records::Item;
    // The managed connect keeps admin, API, git, user and origin, without a hook.
    const SECRETS: usize = 5;
    assert_eq!(held.application_secrets.len(), SECRETS);
    for (item, expected) in [
        (Item::AdminToken, golden::ADMIN_TOKEN),
        (Item::ApiToken, golden::API_TOKEN),
        (Item::GitToken, golden::GIT_TOKEN),
        (Item::User, &1_u64.to_le_bytes()),
    ] {
        let number = temper_protocol_records::number(FORGE, item);
        assert_eq!(held.application_secrets[&number].as_ref(), expected, "{item:?}");
    }
    let origin = temper_protocol_records::Origin::decode(
        &held.application_secrets[&temper_protocol_records::number(FORGE, Item::Origin)],
    )
    .expect("origin");
    assert_eq!(origin.kind, skein_forge_connection::settings::Kind::Forgejo);
    assert_eq!(origin.api.as_ref(), "127.0.0.1:3000");
}

#[expect(clippy::wildcard_enum_match_arm, reason = "the fixture selects live or ended task rows only")]
fn task(held: &Held, number: u64) -> &tasks::TaskRecord {
    held.jig
        .values()
        .find_map(|record| match record {
            core::Record::Tasks(tasks::Stored::Live(task) | tasks::Stored::Ended(task)) if task.number == number => {
                Some(task)
            }
            _ => None,
        })
        .expect("kept task")
}
fn proof(held: &Held, number: u64) -> &core::RunProof {
    let core::Record::Core(core::CoreRecord::RunProof(proof)) =
        &held.jig[&core::Key::Core(core::CoreKey::RunProof(number))]
    else {
        panic!("kept claim")
    };
    proof
}

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
