//! A change's agents read the forge at their own repository.
use super::*;
use skein_forge::api::{Answer, Error, Read};

const CHANNEL: Token = Token::new(7);
const MAX_TICKS: u32 = 200;
const FILE: &[u8] = b"file";
const CONTENT: &[u8] = b"review this diff";

fn read(
    world: &mut World,
    run: &engine::Assignment,
    number: u32,
    repository: skein_forge::api::Repository,
    read: Read,
) -> Result<Answer, Error> {
    let before = world.answers.len();
    let event = world.protocol.call(
        CHANNEL,
        run.task,
        run.attempt,
        u64::from(number),
        engine::Call { completion: number, position: 1, tool: engine::Tool::ReadForge { repository, read } },
    );
    world.send(event);
    for _ in 0..MAX_TICKS {
        world.tick();
        if let Some(answer) = world.answers.get(before) {
            let engine::CallAnswer::ForgeRead(result) = answer else { panic!("forge answer: {answer:?}") };
            return *result.clone();
        }
    }
    panic!("forge read was not answered");
}

pub(super) fn next_assignment(world: &mut World, index: usize) -> engine::Assignment {
    for _ in 0..MAX_TICKS {
        world.tick();
        if let Some(run) = world.assigned.get(index) {
            return run.clone();
        }
    }
    panic!("delegate {index} was not assigned: {:?}", world.answers);
}

fn produce(world: &mut World, producer: &engine::Assignment, branch: &[u8]) -> [u8; 32] {
    world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.into(), commit: 1 }));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", branch, FILE, CONTENT, 1)
        .expect("producer pushed review head");
    let event = world.protocol.answer(
        CHANNEL,
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    translate::commit(head)
}

pub(super) fn review_world() -> (World, engine::Assignment, [u8; 32]) {
    let (mut world, _, producer, branch) = change_world_with_gate(false, 1000, None, true);
    let head = produce(&mut world, &producer, &branch);
    let reviewer = next_assignment(&mut world, 2);
    (world, reviewer, head)
}

#[test]
fn a_change_keeps_its_producing_and_review_charters_after_configuration_changes() {
    let world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate { agent: true, ..Gate::default() },
        None,
        StoryLimits { forge_charter: Some(2), ..StoryLimits::default() },
    );
    let (mut world, _, producer, branch) = change_world_in(world, Change::default());
    assert_eq!(producer.charter, 2);
    let change = requester(&world, producer.task);
    assert_eq!(change_row(&world, change).producing_charter, 2);
    assert_eq!(change_row(&world, change).review_charter, 4);
    world.restart_with_charter(true, true, Some(1));
    world.until(Until::Ready);
    let head = produce(&mut world, &producer, &branch);
    let reviewer = next_assignment(&mut world, 2);
    assert_eq!(reviewer.charter, 4);
    assert_review_head(&world, &reviewer, head);
    assert!(!reviewer.run.policy.modify);
    assert!(!reviewer.run.policy.agents);
    let task = live_task(&world, reviewer.task);
    assert_eq!(task.authority.tools, tasks::Tools(512));
    assert!(!task.authority.grants.is_empty());
    assert!(
        task.authority.grants.iter().all(|grant| grant.connector == 0 && forge_top::READ_KINDS.contains(&grant.kind))
    );
    assert!(task.authority.delegation.kinds.is_empty());
    assert!(task.spec.words.ends_with(b"Small fix"));
    finish_review(
        &mut world,
        &reviewer,
        tasks::TaskResult::Verdict { code: 2, fields: Box::new([]), words: b"fix the unsafe edge".as_slice().into() },
    );
    let repair = next_assignment(&mut world, 3);
    assert_eq!(repair.charter, 2, "the next producing step uses the change's snapshot");
    assert_eq!(change_row(&world, change).producing_charter, 2);
    assert_eq!(change_row(&world, change).review_charter, 4);
}

#[test]
fn a_changes_reviewer_reads_its_own_pull() {
    let (mut world, reviewer, head) = review_world();
    let Answer::Pull(pull) =
        read(&mut world, &reviewer, 10, forge_world::REPO, Read::Pull { number: 1 }).expect("pull read")
    else {
        panic!("pull")
    };
    assert_eq!(pull.number, 1);
    assert_eq!(pull.commit, head);
    let Answer::PullFiles { head: observed, files, more } =
        read(&mut world, &reviewer, 11, forge_world::REPO, Read::PullFiles { number: 1, head, page: 1 })
            .expect("files read")
    else {
        panic!("files")
    };
    assert_eq!(observed, head);
    assert!(!more);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path.as_ref(), FILE);
    assert_eq!(files[0].after.as_deref(), Some(CONTENT));
    let Answer::Items { items, .. } =
        read(&mut world, &reviewer, 12, forge_world::REPO, Read::Items { since: Time::ZERO, page: 1, kind: None })
            .expect("items read")
    else {
        panic!("items")
    };
    assert!(items.iter().any(|item| item.number == pull.number && item.title.as_ref() == b"Small fix"));
}

#[test]
fn a_changes_producer_reads_the_forge() {
    let (mut world, _, producer, _) = change_world_with_gate(false, 1000, None, true);
    let Answer::Commit(head) =
        read(&mut world, &producer, 10, forge_world::REPO, Read::Branch { branch: b"main".as_slice().into() })
            .expect("producer read")
    else {
        panic!("branch")
    };
    assert_eq!(head, translate::commit(1));
}

#[test]
fn a_changes_grants_name_its_repository_alone() {
    let mut world = World::configured_policy(true, true, false, 1000, Gate { agent: true, ..Gate::default() }, None);
    world.adopt();
    world.send(protocol::hello_slots(CHANNEL, 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    let forge = authority::Name {
        segments: [b"forge".as_slice(), b"forge.example", b"another-owner", b"another-repo"].map(Box::from).into(),
    };
    assert!(
        live_task(&world, chat.task)
            .authority
            .grants
            .iter()
            .all(|grant| authority::pattern_covers(&pattern(&grant.pattern), &forge)),
        "chat grants cover the forge"
    );
    let answer = change_tool::change(&mut world, &chat, 93, None, b"Small fix");
    let engine::CallAnswer::Delegated(made) = answer else {
        panic!("change delegated: {answer:?}; limits {:?}", world.env.limits.tasks)
    };
    let producer = next_assignment(&mut world, 1);
    let branch = live_task(&world, producer.task)
        .spec
        .parameters
        .iter()
        .find_map(|parameter| match parameter {
            tasks::Parameter::Bytes { name: 3, value } => Some(value.clone()),
            _ => None,
        })
        .expect("producer branch");
    let mut authorities =
        [made[0].task, producer.task].map(|task| (task, live_task(&world, task).authority.clone())).to_vec();
    let head = produce(&mut world, &producer, &branch);
    let reviewer = next_assignment(&mut world, 2);
    authorities.push((reviewer.task, live_task(&world, reviewer.task).authority.clone()));
    for (task, authority) in authorities {
        assert_repository_grants(task, &authority);
    }
    assert!(
        matches!(
            read(&mut world, &reviewer, 10, forge_world::REPO, Read::PullFiles { number: 1, head, page: 1 }),
            Ok(Answer::PullFiles { .. })
        ),
        "chat change passes its reads to the reviewer"
    );
    // A second adopted repository in the same project makes this an authority refusal,
    // rather than a missing-repository or cross-project refusal.
    let mut other = world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            Record::Forge { row } => match row.as_ref() {
                forge_top::Stored::Repository(repository) if repository.provider == forge_world::REPO => {
                    Some(repository.clone())
                }
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("adopted repository");
    other.provider.repository = 3;
    other.name = b"repo-extra".as_slice().into();
    other.home = false;
    world.send(engine::Event::Forge(Box::new(forge_top::Event::Restore {
        record: forge_top::Stored::Repository(other),
    })));
    assert_eq!(
        read(
            &mut world,
            &reviewer,
            11,
            skein_forge::api::Repository { repository: 3, ..forge_world::REPO },
            Read::Pull { number: 1 }
        ),
        Err(Error::Forbidden)
    );
}

fn pattern(value: &tasks::Pattern) -> authority::Pattern {
    authority::Pattern {
        segments: value.segments.clone(),
        last: match &value.last {
            tasks::Last::Exact(bytes) => authority::Last::Exact(bytes.clone()),
            tasks::Last::Open(bytes) => authority::Last::Open(bytes.clone()),
        },
    }
}

fn assert_repository_grants(task: u64, authority: &tasks::Authority) {
    let root = authority::Pattern {
        segments: [b"forge".as_slice(), b"forge.example", b"org", b"repo"].map(Box::from).into(),
        last: authority::Last::Open(Box::new([])),
    };
    for grant in &authority.grants {
        assert_eq!(grant.connector, 0);
        assert!(
            authority::pattern_at_most(&pattern(&grant.pattern), &root)
                || grant.pattern
                    == tasks::Pattern {
                        segments: [b"forge".as_slice(), b"forge.example", b"org"].map(Box::from).into(),
                        last: tasks::Last::Exact(b"repo".as_slice().into())
                    },
            "task {task} grant outside repository: {grant:?}"
        );
        let sibling = authority::Name {
            segments: [b"forge".as_slice(), b"forge.example", b"org", b"repo-extra", b"pull", b"1"]
                .map(Box::from)
                .into(),
        };
        assert!(!authority::pattern_covers(&pattern(&grant.pattern), &sibling));
    }
}

pub(super) fn finish_review(world: &mut World, reviewer: &engine::Assignment, result: tasks::TaskResult) {
    let event = world.protocol.answer(
        CHANNEL,
        reviewer.task,
        reviewer.attempt,
        5,
        tasks::End::Finished { result, cancel_delegates: false },
        engine::Parts::none(),
    );
    world.send(event);
}

fn requester(world: &World, task: u64) -> u64 {
    let tasks::Party::Task(parent) = live_task(world, task).requester else { panic!("task requester") };
    parent
}

fn change_row(world: &World, task: u64) -> &forge_top::ChangeRow {
    world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            Record::Forge { row } => match row.as_ref() {
                forge_top::Stored::Change(row) if row.task == task => Some(row),
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("durable change")
}

fn assert_review_head(world: &World, reviewer: &engine::Assignment, head: [u8; 32]) {
    let row = live_task(world, reviewer.task);
    assert!(matches!(row.contract, tasks::Contract::Verdict { .. }), "fresh review, not a repair");
    assert!(
        row.spec.parameters.iter().any(|parameter| matches!(parameter, tasks::Parameter::Number { name: 7, value: 1 })),
        "the same gate"
    );
    let mut digits = skein_lib::List::with_capacity(40);
    for byte in &head[..20] {
        for digit in forge_top::hex(*byte) {
            digits.push(digit).expect("SHA-1 digits");
        }
    }
    assert!(
        row.spec.parameters.iter().any(|parameter| matches!(parameter,
        tasks::Parameter::Bytes { name: 8, value } if value.as_ref() == digits.as_slice())),
        "the same pinned head"
    );
}

#[test]
fn a_failed_review_is_asked_again_at_the_same_head() {
    let (mut world, first, head) = review_world();
    let change = requester(&world, first.task);
    finish_review(&mut world, &first, tasks::TaskResult::Failure { reason: b"cannot read the pull".as_slice().into() });
    let second = next_assignment(&mut world, 3);
    assert_ne!(second.task, first.task);
    assert_review_head(&world, &second, head);
    let row = change_row(&world, change);
    assert_eq!(row.change.repairs, 0);
    assert_eq!(
        row.change.gate_failures.as_ref(),
        &[temper_engine_domain_forge_change::GateFailure { number: 1, count: 1 }]
    );
    assert!(row.verdicts.is_empty(), "failure is no verdict");
    assert_eq!(world.merges, 0);
}

#[test]
fn a_second_failed_review_holds_the_change_naming_it() {
    use temper_engine_domain_forge_change::{GateFailure, Hold, State};
    const SECOND_FAILURE: &[u8] = b"cannot judge the second review";
    let (mut world, first, head) = review_world();
    let change = requester(&world, first.task);
    let parent = requester(&world, change);
    finish_review(&mut world, &first, tasks::TaskResult::Failure { reason: b"cannot read the pull".as_slice().into() });
    let second = next_assignment(&mut world, 3);
    assert_review_head(&world, &second, head);
    finish_review(&mut world, &second, tasks::TaskResult::Failure { reason: SECOND_FAILURE.into() });
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    let row = change_row(&world, change);
    assert!(matches!(row.change.state, State::Held { why: Hold::Failed, .. }), "{:?}", row.change.state);
    assert_eq!(row.change.repairs, 0);
    assert_eq!(row.change.gate_failures.as_ref(), &[GateFailure { number: 1, count: 2 }]);
    assert!(row.verdicts.is_empty());
    assert_eq!(world.merges, 0);
    assert_eq!(world.assigned.len(), 4, "no repair or third review");
    assert!(
        matches!(live_task(&world, change).phase, tasks::Phase::Held { why: tasks::Hold::EffectFailed, .. }),
        "jig holds the change"
    );
    assert!(
        live_task(&world, parent).inbox.iter().any(|word| word.from == tasks::Party::Task(change)
            && word.words.windows(SECOND_FAILURE.len()).any(|part| part == SECOND_FAILURE)),
        "hold names the second review's failure"
    );
}

#[test]
fn a_request_for_changes_is_still_a_repair() {
    let (mut world, first, _) = review_world();
    let change = requester(&world, first.task);
    finish_review(
        &mut world,
        &first,
        tasks::TaskResult::Verdict { code: 2, fields: Box::new([]), words: b"fix the unsafe edge".as_slice().into() },
    );
    let repair = next_assignment(&mut world, 3);
    assert!(matches!(live_task(&world, repair.task).contract, tasks::Contract::Change { .. }));
    let row = change_row(&world, change);
    assert_eq!(row.change.repairs, 1);
    assert!(row.change.gate_failures.is_empty());
    assert_eq!(row.verdicts[0].status, temper_engine_domain_forge_change::Status::Failed);
}

// domain/forge.md, 8.5: the procedure retains the precise hold while its review waits.
#[test]
fn an_external_close_holds_the_change_during_review() {
    use temper_engine_domain_forge_change::{Hold, State};
    const OUTSIDE_AUTHOR: u64 = 2; // The forge world's other collaborator.
    const REVIEW_GATE: u64 = 1; // review_world's configured agent gate.
    let (mut world, reviewer, head) = review_world();
    let change = requester(&world, reviewer.task);
    let pull = change_row(&world, change).pull.expect("reviewed pull");
    world.external(OUTSIDE_AUTHOR, raw::Op::Write(raw::Write::Close { number: pull }));
    // A keep may have backed off to poll_max; allow its close read and the
    // procedure's fresh-read chain one queue window to finish.
    let until = world
        .env
        .now
        .saturating_add(world.env.limits.forge.client.poll_max)
        .saturating_add(world.env.limits.forge.queue_window);
    while world.env.now < until {
        world.tick();
    }
    let row = change_row(&world, change);
    assert_eq!(
        row.change.state,
        State::Held { was: Box::new(State::Gating { head, asked: Box::new([REVIEW_GATE]) }), why: Hold::PullClosed },
        "the procedure records the close before jig holds its task"
    );
    assert!(matches!(live_task(&world, change).phase, tasks::Phase::Held { why: tasks::Hold::Drift, .. }));
    assert_eq!(
        row.delegate,
        Some((reviewer.task, temper_engine_domain_forge_change::Delegate::Gate { number: REVIEW_GATE, head }))
    );
    assert_eq!(world.merges, 0);
}

fn held_review() -> (World, u64, [u8; 32]) {
    let (mut world, first, head) = review_world();
    let change = requester(&world, first.task);
    finish_review(&mut world, &first, tasks::TaskResult::Failure { reason: b"cannot read the pull".as_slice().into() });
    let second = next_assignment(&mut world, 3);
    finish_review(&mut world, &second, tasks::TaskResult::Failure { reason: b"still cannot read".as_slice().into() });
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert!(matches!(live_task(&world, change).phase, tasks::Phase::Held { .. }));
    (world, change, head)
}

fn release(world: &mut World, task: u64) {
    let revision = live_task(world, task).revisions.hold;
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(950)),
        sign_in: world.signed_in.expect("owner signed in"),
        key: [95; 16],
        ask: people::Ask::Release { project: 1, task, revision, again: None },
    }));
    // Inspect the decision before either row is durable or any forge read answers.
    let writes = world
        .store
        .pending
        .iter()
        .find_map(|(_, writes)| {
            writes
                .iter()
                .any(|write| {
                    matches!(write,
                        engine::Write::Save(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))))
                            if row.number == task && !matches!(row.phase, tasks::Phase::Held { .. })
                    )
                })
                .then_some(writes)
        })
        .expect("the task's release commit");
    assert!(
        writes.iter().any(|write| matches!(write,
            engine::Write::Save(Record::Forge { row }) if matches!(row.as_ref(),
                forge_top::Stored::Change(row) if row.task == task
                    && !matches!(row.change.state, temper_engine_domain_forge_change::State::Held { .. })
                    && row.change.gate_failures.is_empty()
                    && row.change.released.is_some())
        )),
        "the same commit clears the change's hold and review tries"
    );
}

fn approve(world: &mut World, reviewer: &engine::Assignment) {
    finish_review(
        world,
        reviewer,
        tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
    );
}

fn assert_lands(world: &mut World, task: u64) {
    for _ in 0..MAX_TICKS {
        world.tick();
        if matches!(change_row(world, task).change.state, temper_engine_domain_forge_change::State::Landed { .. }) {
            assert_eq!(world.merges, 1);
            return;
        }
    }
    panic!("released change did not land: {:?}", change_row(world, task));
}

#[test]
fn a_released_change_asks_its_review_afresh() {
    let (mut world, change, head) = held_review();
    release(&mut world, change);
    let fresh = next_assignment(&mut world, 4);
    assert_review_head(&world, &fresh, head);
    assert!(change_row(&world, change).change.gate_failures.is_empty());
    approve(&mut world, &fresh);
    assert_lands(&mut world, change);
}

#[test]
fn a_released_change_counts_its_review_tries_anew() {
    let (mut world, change, head) = held_review();
    release(&mut world, change);
    let fresh = next_assignment(&mut world, 4);
    finish_review(&mut world, &fresh, tasks::TaskResult::Failure { reason: b"cannot read again".as_slice().into() });
    let retry = next_assignment(&mut world, 5);
    assert_review_head(&world, &retry, head);
    assert_eq!(
        change_row(&world, change).change.gate_failures.as_ref(),
        &[temper_engine_domain_forge_change::GateFailure { number: 1, count: 1 }]
    );
    assert!(!matches!(live_task(&world, change).phase, tasks::Phase::Held { .. }));
    approve(&mut world, &retry);
    assert_lands(&mut world, change);
}

fn cancel_and_assert_panel_ended(world: &mut World, task: u64) {
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(951)),
        sign_in: world.signed_in.expect("owner signed in"),
        key: [96; 16],
        ask: people::Ask::Cancel { project: 1, task, reason: b"cancel held change".as_slice().into(), grace: None },
    }));
    for _ in 0..MAX_TICKS {
        world.tick();
        if world.store.rows.contains_key(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)))) {
            assert_eq!(change_row(world, task).change.state, temper_engine_domain_forge_change::State::Ended);
            world.send(engine::Event::Party(engine::PartyInput::Watch {
                watcher: Token::new(952),
                sign_in: world.signed_in.expect("owner signed in"),
                subject: jig_domain_core::documents::Watched::Application {
                    scope: people::ApplicationScope::Project(1),
                    connector: 0,
                    subject: Token::new(task),
                },
                window: jig_domain_core::documents::Window { rows: 0, after: None },
            }));
            for _ in 0..MAX_TICKS {
                world.tick();
                if let Some((_, panel)) = world.panels.iter().find(|(watcher, _)| *watcher == Token::new(952)) {
                    assert_eq!(panel.step, forge_top::PanelStep::Ended);
                    assert_eq!(panel.hold, None);
                    assert_eq!(panel.runs, None);
                    assert_eq!(world.merges, 0);
                    return;
                }
            }
            panic!("ended change's retained panel was not delivered");
        }
    }
    panic!("cancel did not end the held task");
}

#[test]
fn a_cancelled_held_change_ends_its_panel() {
    let (mut world, change, _) = held_review();
    cancel_and_assert_panel_ended(&mut world, change);
}

/// Restore the held snapshot for each cause, then exercise the person's real release/cancel route.
fn held_for(why: temper_engine_domain_forge_change::Hold) -> (World, u64, [u8; 32]) {
    use temper_engine_domain_forge_change::{GateKind, GateReport, Hold, State, Status};
    let (mut world, task, head) = held_review();
    let mut row = change_row(&world, task).clone();
    row.change.state = State::Held { was: Box::new(State::Gating { head, asked: Box::new([]) }), why };
    row.change.gate_failures = Box::new([]);
    match why {
        Hold::Repairs => {
            row.change.repairs = world.env.limits.forge.change_policy.repairs;
            row.verdicts = Box::new([GateReport { number: 1, head, status: Status::Failed }]);
        }
        Hold::Rejected => {
            row.change.gates[0].kind = GateKind::Person;
            row.verdicts = Box::new([GateReport { number: 1, head, status: Status::Failed }]);
        }
        Hold::BranchMoved => row.drift = Some(Hold::BranchMoved),
        _ => panic!("not a held fixture for {why:?}"),
    }
    world.send(engine::Event::Forge(Box::new(forge_top::Event::Restore { record: forge_top::Stored::Change(row) })));
    (world, task, head)
}

#[test]
fn a_released_change_counts_its_repairs_anew() {
    let (mut world, task, _) = held_for(temper_engine_domain_forge_change::Hold::Repairs);
    release(&mut world, task);
    let repair = next_assignment(&mut world, 4);
    assert!(matches!(live_task(&world, repair.task).contract, tasks::Contract::Change { .. }));
    assert_eq!(change_row(&world, task).change.repairs, 1);
    assert!(!matches!(live_task(&world, task).phase, tasks::Phase::Held { .. }));
}

#[test]
fn a_released_change_with_a_declined_approval_is_held_again() {
    use temper_engine_domain_forge_change::{Hold, State};
    let (mut world, task, _) = held_for(Hold::Rejected);
    let revision = live_task(&world, task).revisions.hold;
    release(&mut world, task);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert!(matches!(change_row(&world, task).change.state, State::Held { why: Hold::Rejected, .. }));
    assert!(matches!(live_task(&world, task).phase, tasks::Phase::Held { .. }));
    assert!(live_task(&world, task).revisions.hold > revision, "a fresh task hold matches the unchanged verdict");
    assert_eq!(world.assigned.len(), 4, "a declined approval is not asked again at the same head");
}

#[test]
fn a_released_change_checks_its_moved_branch_afresh() {
    let (mut world, task, old) = held_for(temper_engine_domain_forge_change::Hold::BranchMoved);
    let branch = change_row(&world, task).branch.clone();
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, FILE, b"a person's new diff", 2)
        .expect("person moved the held branch");
    let head = translate::commit(head);
    assert_ne!(head, old);
    release(&mut world, task);
    let fresh = next_assignment(&mut world, 4);
    assert_review_head(&world, &fresh, head);
    assert_eq!(change_row(&world, task).change.last_head, Some(head));
    assert_eq!(change_row(&world, task).drift, None);
    approve(&mut world, &fresh);
    assert_lands(&mut world, task);
}

#[test]
fn a_cancelled_change_held_for_repairs_ends_its_panel() {
    let (mut world, task, _) = held_for(temper_engine_domain_forge_change::Hold::Repairs);
    cancel_and_assert_panel_ended(&mut world, task);
}

#[test]
fn a_cancelled_change_with_a_declined_approval_ends_its_panel() {
    let (mut world, task, _) = held_for(temper_engine_domain_forge_change::Hold::Rejected);
    cancel_and_assert_panel_ended(&mut world, task);
}

#[test]
fn a_cancelled_change_with_a_moved_branch_ends_its_panel() {
    let (mut world, task, _) = held_for(temper_engine_domain_forge_change::Hold::BranchMoved);
    cancel_and_assert_panel_ended(&mut world, task);
}

fn observed_move(world: &World, task: u64, head: [u8; 32]) -> bool {
    world.store.rows.values().any(|record| {
        matches!(record,
        Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Hold(hold)
            if hold.task == task && matches!(&hold.drift,
                Some(forge_top::Drift { change: forge_top::DriftChange::Branch { observed: Some(observed), .. }, .. })
                    if *observed == head)))
    })
}

#[test]
fn a_released_moved_change_is_not_held_again_while_its_fresh_read_is_slow() {
    const SLOW_READ_TICKS: u32 = 600;
    let (mut world, task, _) = held_for(temper_engine_domain_forge_change::Hold::BranchMoved);
    let branch = change_row(&world, task).branch.clone();
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, FILE, b"new diff", 2)
        .expect("person moved the branch");
    let head = translate::commit(head);
    for _ in 0..SLOW_READ_TICKS {
        world.tick();
        if observed_move(&world, task, head) {
            break;
        }
    }
    assert!(observed_move(&world, task, head), "the branch's drift is observed before release");
    world.slow_branch = Some(branch);
    release(&mut world, task);
    for _ in 0..SLOW_READ_TICKS {
        world.tick();
        if let Some(fresh) = world.assigned.get(4) {
            assert_review_head(&world, fresh, head);
            assert!(!matches!(live_task(&world, task).phase, tasks::Phase::Held { .. }));
            return;
        }
    }
    panic!("the released change was held again: {:?}", live_task(&world, task).phase);
}
