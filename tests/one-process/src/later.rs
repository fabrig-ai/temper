//! What every later build promises (draft/upgrades.md, 13; protocol/README.md, 9).
use crate::{
    deployment::{Deployment, Held},
    fixtures,
    golden::{self, Ledger},
    referee::Store,
    world,
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::api;
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use skein_fake_person::{Find, Step};
use skein_ui::Role;
use temper_engine_domain_forge as forge;

const REPOSITORY: &[u8] = b"acme/app";
const PERSON: u64 = 1;
const RESUME: &[u8] = b"Continue after the upgrade";
const RESUMED: &[u8] = b"Golden chat resumed";
const CONTINUED: &[u8] = b"Golden task continued";
// The CLI connects forge 1, then adopts acme/app for project 1.
const CLI_ANSWERS: usize = 2;
// The continuation replays only the retained adoption request.
const REPLAY_ANSWERS: usize = 1;

/// Start the candidate on a kept deployment, then exercise its promised history
/// and continued work. At the tag only the caller's input changes to kept files.
pub fn assert_later(kept: &Deployment, ledger: Ledger) {
    let before = Held::read(kept);
    let effects = ledger.effects.clone();
    let calls = ledger.calls.clone();
    let requests = ledger.requests.clone();
    let answers = ledger.answers.clone();
    let main = ledger.inspect(REPOSITORY, &api::Read::Branch { branch: b"main".as_slice().into() });
    let settings = fixtures::calm(fixtures::SEED);
    let mut started = world::over_with_ledger(kept, settings, ledger);
    let deployment = started.deployment();
    let after = Held::read(&deployment);
    assert_identities_and_counters(&before, &after);
    assert_policies(&before, &after);
    assert_kept_outcomes(&before, &after);
    assert_old_turns_read(&before, &after);
    assert_landed_main(&after, &main);
    assert_eq!(deployment.configuration, kept.configuration, "configuration stays as written");
    assert_eq!(deployment.startup, kept.startup, "small startup files stay as written");

    let (continued, ledger) = started.continue_work(settings).into_parts();
    let continued = Held::read(&continued);
    assert_work_goes_on(&before, &continued);
    assert_policies(&before, &continued);
    assert_kept_outcomes(&before, &continued);
    assert_old_turns_read(&before, &continued);
    assert_landed_main(&continued, &main);
    assert_retained_answers(&answers, &ledger);
    assert_no_effect_twice(&effects, &calls, &requests, &main, &ledger);
}

fn assert_identities_and_counters(before: &Held, after: &Held) {
    assert_eq!(numbers(after), numbers(before), "every deployment high-water mark and identity survives startup");
    for (key, row) in &before.jig {
        if matches!(
            row,
            core::Record::People(
                people::Stored::Person { .. } | people::Stored::SignIn { .. } | people::Stored::Roles { .. }
            ) | core::Record::Accounts(_)
        ) {
            assert_eq!(after.jig.get(key), Some(row), "identity/account row {key:?}");
        }
    }
    for (key, row) in &before.application {
        if matches!(row, forge::Stored::Repository(_) | forge::Stored::Writer(_) | forge::Stored::Binding { .. }) {
            assert_eq!(after.application.get(key), Some(row), "forge/repository identity {key:?}");
        }
    }
    assert_eq!(after.jig_secrets, before.jig_secrets, "account values and credential digests survive");
    assert_eq!(after.application_secrets, before.application_secrets, "forge credential numbers and values survive");
}

fn assert_policies(before: &Held, after: &Held) {
    for (key, row) in &before.jig {
        if matches!(
            row,
            core::Record::People(people::Stored::Policy { .. }) | core::Record::Core(core::CoreRecord::Settings(_))
        ) {
            assert_eq!(after.jig.get(key), Some(row), "kept project policy/settings {key:?}");
        }
    }
}

fn assert_kept_outcomes(before: &Held, after: &Held) {
    for number in [golden::GOAL, golden::PRODUCER] {
        assert!(
            after.jig.values().any(|row| matches!(row,
                core::Record::Tasks(tasks::Stored::Ended(task)) if task.number == number
                    && matches!(task.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. })))
            )),
            "kept task {number} still has its successful change result"
        );
    }
    for (key, row) in &before.jig {
        if matches!(
            row,
            core::Record::Tasks(
                tasks::Stored::Ended(_)
                    | tasks::Stored::Result(_)
                    | tasks::Stored::Activity(_)
                    | tasks::Stored::Milestone(_)
                    | tasks::Stored::History(_)
                    | tasks::Stored::ClosedLedger(_)
                    | tasks::Stored::EndedByParty(_)
                    | tasks::Stored::EndedByProject(_)
                    | tasks::Stored::EndedDelegate(_)
            ) | core::Record::People(people::Stored::Answer { .. } | people::Stored::Acted { .. })
                | core::Record::Core(core::CoreRecord::Terminal(_))
        ) {
            assert_eq!(after.jig.get(key), Some(row), "kept result/request answer {key:?}");
        }
    }
    for (key, row) in &before.application {
        if matches!(row, forge::Stored::Landed { .. } | forge::Stored::Change(_) | forge::Stored::Application { .. }) {
            assert_eq!(after.application.get(key), Some(row), "kept landing/request outcome {key:?}");
        }
    }
}

fn assert_landed_main(held: &Held, main: &Result<api::Answer, api::Error>) {
    let api::Answer::Commit(commit) = main.as_ref().expect("kept forge main") else { panic!("main commit") };
    let object = skein_fake_forge_domain::object_bytes(*commit);
    let mut merge = [0; 32];
    merge[..object.len()].copy_from_slice(&object);
    assert!(
        matches!(
            held.application.get(&forge::Key::Landed(merge)),
            Some(forge::Stored::Landed { task: golden::GOAL, .. })
        ),
        "the durable landing and the outside main name the same merge"
    );
}

fn assert_old_turns_read(before: &Held, after: &Held) {
    for task in [golden::CHAT, golden::MID_RUN] {
        let kept = &before.turns[&task];
        assert!(!kept.is_empty(), "golden task {task} has committed turns");
        assert!(after.turns[&task].starts_with(kept), "current frame reader preserves every old turn of {task}");
        for turn in kept {
            let key = core::Key::Core(core::CoreKey::Turn { task, attempt: turn.attempt, turn: turn.turn });
            assert_eq!(after.jig.get(&key), before.jig.get(&key), "old committed turn row {key:?}");
        }
    }
}

fn assert_work_goes_on(before: &Held, after: &Held) {
    for task in [golden::CHAT, golden::MID_RUN] {
        let old = proof(before, task);
        let new = proof(after, task);
        assert!(new.attempt > old.attempt, "task {task} gets a new activation");
        assert_eq!(new.conversation, old.conversation, "task {task} resumes its kept conversation");
        assert!(after.turns[&task].len() > before.turns[&task].len(), "task {task} commits more turns");
        assert!(after.turns[&task].iter().any(|turn| turn.attempt == new.attempt), "new activation commits work");
        assert!(new.attempt > numbers(before).runs, "new activation is past all kept activations");
    }
    let old = numbers(before);
    let new = numbers(after);
    assert_eq!(new.id, old.id, "deployment identity never changes");
    assert!(new.sign_ins > old.sign_ins, "the real client allocates a sign-in past every kept sign-in");
    let sign_in = core::Key::People(people::Key::SignIn(new.sign_ins));
    assert!(
        matches!(after.jig.get(&sign_in), Some(core::Record::People(people::Stored::SignIn { person: PERSON, .. }))),
        "new sign-in still names the kept person"
    );
}

fn assert_retained_answers(answers: &[([u8; 16], jig_client::Family)], ledger: &Ledger) {
    assert_eq!(answers.len(), CLI_ANSWERS, "the kept CLI has the connection and adoption answers");
    assert!(ledger.answers.starts_with(answers), "the first answer remains in the ledger");
    let replayed = &ledger.answers[answers.len()..];
    assert_eq!(replayed.len(), REPLAY_ANSWERS, "one retained adoption is replayed");
    for answer in replayed {
        assert!(answers.contains(answer), "same request key, family, version and exact answer bytes");
    }
}

fn assert_no_effect_twice(
    effects: &[skein_fake_forge_domain::Observation],
    calls: &[golden::Call],
    requests: &[skein_http::server::Call],
    main: &Result<api::Answer, api::Error>,
    ledger: &Ledger,
) {
    let api::Answer::Commit(commit) = main.as_ref().expect("kept forge main") else { panic!("main commit") };
    assert!(
        ledger.effects.iter().any(|effect| matches!(effect,
            skein_fake_forge_domain::Observation::Merged { commit: landed, .. } if landed == commit
        )),
        "the ledger holds the landing on main"
    );
    assert_eq!(ledger.effects, effects, "every kept effect occurs once; no repeated push, merge, or other effect");
    assert!(ledger.calls.starts_with(calls), "the kept API call history is unchanged");
    assert!(ledger.requests.starts_with(requests), "the kept HTTP history is unchanged");
    assert!(
        ledger.calls[calls.len()..].iter().all(|call| matches!(call.op, api::Op::Read(_))),
        "continued work and replay make no forge writes"
    );
    assert!(
        ledger.requests[requests.len()..].iter().all(|call| call.method == skein_http::Method::Get),
        "setup is never made twice"
    );
    assert_eq!(
        &ledger.inspect(REPOSITORY, &api::Read::Branch { branch: b"main".as_slice().into() }),
        main,
        "the landed forge main is unchanged"
    );
}

fn numbers(held: &Held) -> &core::Deployment {
    let core::Record::Core(core::CoreRecord::Deployment(numbers)) =
        &held.jig[&core::Key::Core(core::CoreKey::Deployment)]
    else {
        panic!("deployment counters")
    };
    numbers
}
fn proof(held: &Held, task: u64) -> &core::RunProof {
    let core::Record::Core(core::CoreRecord::RunProof(proof)) =
        &held.jig[&core::Key::Core(core::CoreKey::RunProof(task))]
    else {
        panic!("task claim")
    };
    proof
}

pub(crate) fn person() -> Vec<Step<temper_client_domain::Address>> {
    vec![
        Step::Go { address: temper_client_domain::Address::Chat { task: golden::CHAT } },
        Step::See { find: Find::text(golden::PAUSED_WORDS), within: fixtures::PERSON_WAIT },
        Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::PERSON_WAIT },
        Step::Type { find: Find::named(Role::TextArea, b"Message"), words: RESUME.into() },
        Step::Press { find: Find::named(Role::Button, b"Send") },
        Step::See { find: Find::text(RESUMED), within: fixtures::PERSON_WAIT },
    ]
}

pub(crate) fn scripts(scripts: &mut [Script]) {
    let planner = scripts.iter_mut().find(|script| script.cue.as_ref() == b"@plan").expect("golden planner");
    // The fake selects system cues, counting the retained assistant messages.
    // Every possible resumed position answers without delegating the change again.
    planner.turns =
        vec![continued_turn(RESUMED); usize::try_from(fixtures::CHARTER_TURNS).expect("turn limit")].into_boxed_slice();
    let mid_run = scripts.iter_mut().find(|script| script.cue.as_ref() == b"@midrun").expect("golden mid-run task");
    mid_run.turns[1] = continued_turn(CONTINUED);
}
fn continued_turn(words: &[u8]) -> Turn {
    Turn {
        lines: Box::new([
            Line::Text { text: words.into() },
            Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: fixtures::CALL_TOKENS,
    }
}

pub(crate) fn complete(store: &Store) -> bool {
    [golden::CHAT, golden::MID_RUN].iter().all(|task| {
        let turns = store.rows.values().filter(|row| matches!(row,
            temper_engine_domain::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) if turn.task == *task
        ));
        // Golden's chat has three turns and mid-run task one; each must commit
        // a turn of a later activation before the continuation may stop.
        turns
            .map(|row| {
                let temper_engine_domain::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) = row else {
                    unreachable!()
                };
                turn.attempt
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1
    })
}
