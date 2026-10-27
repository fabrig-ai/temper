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

/// domain/forge.md, 8.3: the review names its subject before the pull section.
#[test]
fn a_gate_reviews_a_named_pull_with_its_whole_section_at_a_sha1_head() {
    let (world, reviewer, head) = review_world();
    assert_eq!(&head[20..], &[0; 12]);
    let change = change_row(&world, requester(&world, reviewer.task));
    let number = change.pull.expect("opened pull");
    let identity = format!(
        "Pull request: #{number}\nBranch: {}\nBase: main\nHead: {}\n",
        core::str::from_utf8(&change.branch).expect("branch"),
        core::str::from_utf8(&engine::commit_text(&head)).expect("SHA-1"),
    );
    assert!(core::str::from_utf8(task_brief(&reviewer)).expect("task brief").contains(&identity));
    let task_at = reviewer
        .sections
        .iter()
        .position(|part| part.kind == engine::BriefKind::Core(jig_domain_brief::Core::Task))
        .expect("task section");
    let pull_at = reviewer
        .sections
        .iter()
        .position(|part| part.kind == engine::BriefKind::Forge(engine::ForgeBriefKind::Pull))
        .expect("pull section");
    assert!(task_at < pull_at);
    let engine::BriefBody::Text(words) = &reviewer.sections[pull_at].body else {
        panic!("pull section must be readable: {:?}", reviewer.sections[pull_at]);
    };
    let words = core::str::from_utf8(words).expect("pull text");
    for expected in
        ["Title: Small fix", "Body:", "Files and diff:", "File: file", "After:\nreview this diff", "Against base:"]
    {
        assert!(words.contains(expected), "missing {expected}: {words}");
    }
    let rendered = temper_engine_smith::charter(&reviewer);
    assert!(rendered.brief.sections[task_at].text.windows(identity.len()).any(|part| part == identity.as_bytes()));
    assert!(rendered.brief.sections[pull_at].text.windows(CONTENT.len()).any(|part| part == CONTENT));
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
fn a_reviews_retry_says_it_is_asked_again_naming_the_failure() {
    const FAILURE: &[u8] = b"cannot read the pull";
    const OPENING: &[u8] =
        b"This review is asked again at the same head: the first review failed: cannot read the pull\n";
    let (mut world, first, head) = review_world();
    let original = live_task(&world, first.task).spec.words.clone();
    assert!(original.starts_with(b"Review this change at the named head\n"));
    finish_review(&mut world, &first, tasks::TaskResult::Failure { reason: FAILURE.into() });
    let second = next_assignment(&mut world, 3);
    assert_review_head(&world, &second, head);
    let retry = &live_task(&world, second.task).spec.words;
    assert_ne!(retry, &original);
    assert!(retry.starts_with(OPENING), "retry names the first failure: {retry:?}");
    assert_eq!(&retry[OPENING.len()..], original.as_ref());
    assert!(
        second.sections.iter().any(|section| matches!(&section.body,
        engine::BriefBody::Text(words) if words.starts_with(OPENING))),
        "the assigned brief carries the retry words"
    );
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

fn wait_held(world: &mut World, task: u64) {
    for _ in 0..MAX_TICKS {
        world.tick();
        if matches!(live_task(world, task).phase, tasks::Phase::Held { .. }) && world.store.pending.is_empty() {
            return;
        }
    }
    panic!("change did not hold: {:?}", change_row(world, task));
}

fn held_review() -> (World, u64, [u8; 32]) {
    let (mut world, first, head) = review_world();
    let change = requester(&world, first.task);
    finish_review(&mut world, &first, tasks::TaskResult::Failure { reason: b"cannot read the pull".as_slice().into() });
    let second = next_assignment(&mut world, 3);
    finish_review(&mut world, &second, tasks::TaskResult::Failure { reason: b"still cannot read".as_slice().into() });
    wait_held(&mut world, change);
    assert!(matches!(live_task(&world, change).phase, tasks::Phase::Held { .. }));
    (world, change, head)
}

fn release(world: &mut World, task: u64) {
    let revision = live_task(world, task).revisions.hold;
    let mut key = [95; 16];
    key[15] = u8::try_from(revision).expect("small hold revision");
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(950 + revision)),
        sign_in: world.signed_in.expect("owner signed in"),
        key,
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
        .unwrap_or_else(|| {
            panic!(
                "release commit: task {task}, hold {revision}, phase {:?}, change {:?}, pending {:?}",
                live_task(world, task).phase,
                change_row(world, task).change.state,
                world.store.pending
            )
        });
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

fn past_tasks() -> (World, u64, [u8; 32]) {
    let (mut world, task, head) = held_review();
    release(&mut world, task);
    for index in [4, 5] {
        let review = next_assignment(&mut world, index);
        finish_review(
            &mut world,
            &review,
            tasks::TaskResult::Failure { reason: b"review unavailable".as_slice().into() },
        );
    }
    wait_held(&mut world, task);
    release(&mut world, task);
    let sixth = next_assignment(&mut world, 6);
    finish_review(
        &mut world,
        &sixth,
        tasks::TaskResult::Failure { reason: b"review still unavailable".as_slice().into() },
    );
    wait_held(&mut world, task);
    (world, task, head)
}

fn assert_authority_hold(
    world: &World,
    task: u64,
    lack: temper_engine_domain_forge_change::AuthorityLack,
    words: &[u8],
    released: bool,
) {
    use temper_engine_domain_forge_change::{Hold, State};
    let row = change_row(world, task);
    let State::Held { why: Hold::Authority(cause), .. } = row.change.state else { panic!("{:?}", row.change.state) };
    assert_eq!(cause.lack, lack);
    assert_eq!(cause.released, released);
    let expected = if lack == temper_engine_domain_forge_change::AuthorityLack::Spend {
        tasks::Hold::Budget
    } else {
        tasks::Hold::Procedure
    };
    assert!(
        matches!(live_task(world, task).phase, tasks::Phase::Held { why, .. } if why == expected),
        "never left due"
    );
    let parent = requester(world, task);
    assert!(
        live_task(world, parent)
            .inbox
            .iter()
            .any(|word| word.from == tasks::Party::Task(task)
                && word.words.windows(words.len()).any(|part| part == words)),
        "requester heard the precise cause"
    );
    let live = live_task(world, task);
    assert_eq!(cause.recovery.task, task);
    assert_eq!(cause.recovery.spec, live.revisions.spec);
    assert_eq!(cause.recovery.hold, live.revisions.hold);
    let needed = u64::from(live.made);
    assert_eq!(cause.recovery.tasks, needed);
    let way = format!(
        "release it: task release {task} --hold {} (it holds again unless it has more); or cancel it: task cancel {task} REASON",
        live.revisions.hold,
    );
    assert!(temper_engine_domain_forge_change::authority_words(cause).ends_with(way.as_bytes()));
}

#[test]
fn a_change_past_its_tasks_holds_naming_them() {
    let (world, task, _) = past_tasks();
    assert_authority_hold(
        &world,
        task,
        temper_engine_domain_forge_change::AuthorityLack::Tasks,
        b"its 6 tasks are made",
        false,
    );
    assert_eq!(world.assigned.len(), 7, "chat and exactly six delegates");
}

#[test]
fn a_bare_release_of_a_change_past_its_tasks_holds_it_again() {
    let (mut world, task, _) = past_tasks();
    release(&mut world, task);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert_authority_hold(
        &world,
        task,
        temper_engine_domain_forge_change::AuthorityLack::Tasks,
        b"released without more tasks",
        true,
    );
    assert_eq!(world.assigned.len(), 7, "no delegate attempted");
}

fn change_authority(world: &mut World, task: u64, tasks: Option<u32>, depth: Option<u32>, spend: Option<u64>) {
    let row = live_task(world, task);
    let revision = row.revisions.spec;
    let mut authority = row.authority.clone();
    if let Some(tasks) = tasks {
        authority.delegation.tasks = tasks;
    }
    if let Some(depth) = depth {
        authority.delegation.depth = depth;
    }
    if let Some(spend) = spend {
        authority.budget.spend = spend;
    }
    // Its existing funder has the unspent allowance; the person's pool is fully
    // allotted to that requester in this small world.
    let parent = world.assigned[0].clone();
    let event = world.protocol.call(
        CHANNEL,
        parent.task,
        parent.attempt,
        97,
        engine::Call {
            completion: 97,
            position: 1,
            tool: engine::Tool::Amend {
                target: task,
                revision,
                amendment: temper_engine_domain_world::fixtures::task_amendment(
                    None,
                    Some(authority),
                    b"more authority for the change's next step",
                ),
            },
        },
    );
    world.send(event);
    for _ in 0..MAX_TICKS {
        world.tick();
        if live_task(world, task).revisions.spec > revision && world.store.pending.is_empty() {
            return;
        }
    }
    panic!("amendment committed: {:?}", world.answers);
}

// review_world has consumed the producer and review; the chat is also assigned.
const PRODUCER_AND_REVIEW: u32 = 2;
const BEFORE_REPAIR: usize = 3;

fn rejected_review_with_tasks(tasks: u32) -> (World, u64) {
    let (mut world, reviewer, _) = review_world();
    let change = requester(&world, reviewer.task);
    change_authority(&mut world, change, Some(tasks), None, None);
    finish_review(
        &mut world,
        &reviewer,
        tasks::TaskResult::Verdict { code: 2, fields: Box::new([]), words: b"repair the diff".as_slice().into() },
    );
    (world, change)
}

#[test]
fn one_task_left_admits_the_reviews_repair() {
    use temper_engine_domain_forge_change::{Repair, State};
    const WITH_REPAIR: u32 = PRODUCER_AND_REVIEW + 1;
    let (mut world, change) = rejected_review_with_tasks(WITH_REPAIR);
    let repair = next_assignment(&mut world, BEFORE_REPAIR);
    assert_eq!(requester(&world, repair.task), change, "jig admitted the change's repair");
    assert!(matches!(live_task(&world, repair.task).contract, tasks::Contract::Change { .. }));
    assert_eq!(change_row(&world, change).change.state, State::Repairing { requested: true, why: Repair::Gate(1) });
    let row = live_task(&world, change);
    assert_eq!(row.authority.delegation.tasks, WITH_REPAIR);
    assert_eq!(row.made, WITH_REPAIR + 1, "made counts the change itself; its final task was admitted");
    assert!(!matches!(row.phase, tasks::Phase::Held { .. }));
}

#[test]
fn no_tasks_left_holds_before_the_reviews_repair() {
    let (mut world, change) = rejected_review_with_tasks(PRODUCER_AND_REVIEW);
    wait_held(&mut world, change);
    assert_authority_hold(
        &world,
        change,
        temper_engine_domain_forge_change::AuthorityLack::Tasks,
        b"its 2 tasks are made",
        false,
    );
    assert_eq!(live_task(&world, change).made, PRODUCER_AND_REVIEW + 1);
    assert_eq!(world.assigned.len(), BEFORE_REPAIR, "no repair was attempted");
    assert_eq!(change_row(&world, change).change.repairs, 0);
}

#[test]
fn a_release_with_more_tasks_lets_it_go_on() {
    let (mut world, task, head) = past_tasks();
    change_authority(&mut world, task, Some(8), None, None);
    release(&mut world, task);
    let fresh = next_assignment(&mut world, 7);
    assert_review_head(&world, &fresh, head);
    approve(&mut world, &fresh);
    assert_lands(&mut world, task);
}

#[test]
fn a_change_past_its_depth_holds_naming_it() {
    let (mut world, task, _) = held_review();
    change_authority(&mut world, task, None, Some(0), None);
    release(&mut world, task);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert_authority_hold(
        &world,
        task,
        temper_engine_domain_forge_change::AuthorityLack::Depth,
        b"its depth is used",
        false,
    );
}

#[test]
fn a_change_below_a_runs_reserve_holds_naming_it() {
    let (mut world, task, _) = held_review();
    let spent = live_task(&world, task).numbers.spent + live_task(&world, task).numbers.spent_below;
    change_authority(&mut world, task, None, None, Some(spent + 1));
    release(&mut world, task);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert_authority_hold(
        &world,
        task,
        temper_engine_domain_forge_change::AuthorityLack::Spend,
        b"its spend is below a run's reserve",
        false,
    );
}

#[test]
fn a_producers_result_is_a_push_while_gating_and_only_the_change_reports_a_landing() {
    let (mut world, _, producer, branch) = change_world_with_gate(false, 1000, None, true);
    let change = requester(&world, producer.task);
    assert!(matches!(live_task(&world, producer.task).contract, tasks::Contract::Change { kind: 2, .. }));
    produce(&mut world, &producer, &branch);
    let reviewer = next_assignment(&mut world, 2);
    assert!(matches!(change_row(&world, change).change.state, temper_engine_domain_forge_change::State::Gating { .. }));
    assert_eq!(world.merges, 0);
    let push = ended_result(&world, producer.task).clone();
    assert!(matches!(&push, tasks::TaskResult::Change { kind: 2, words, .. }
        if words.as_ref() == b"pushed" && !words.windows(6).any(|word| word.eq_ignore_ascii_case(b"landed"))));
    approve(&mut world, &reviewer);
    assert_lands(&mut world, change);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert_eq!(ended_result(&world, producer.task), &push);
    assert!(matches!(ended_result(&world, change), tasks::TaskResult::Change { kind: 1, words, .. }
        if words.split(|byte| *byte == b'\n').next().is_some_and(|line| line.len() == 32 && line.iter().all(u8::is_ascii_hexdigit))));
}

fn ended_result(world: &World, task: u64) -> &tasks::TaskResult {
    let key = Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)));
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row)))) = world.store.rows.get(&key)
    else {
        panic!("ended task {task}")
    };
    let tasks::Phase::Ended(tasks::Ending::Done(result)) = &row.phase else { panic!("successful task") };
    result
}

#[test]
#[expect(clippy::too_many_lines, reason = "the funding story spans cancellation, repair, fresh review and landing")]
fn a_review_at_a_head_whose_ci_fails_is_cancelled_and_the_repair_is_funded() {
    use temper_engine_domain_forge_change::{State, Status};
    const H1: u64 = 2;
    let world = World::configured_policy(true, true, false, 1000, Gate { agent: true, ..Gate::default() }, None);
    let (mut world, _, producer, branch) =
        temper_engine_domain_world::forge_routes::change_world_funded(world, Change::default(), 10);
    world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    let first_head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, FILE, CONTENT, 1)
        .expect("producer pushed H1");
    let event = world.protocol.answer(
        CHANNEL,
        producer.task,
        producer.attempt,
        3,
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
    let first_review = next_assignment(&mut world, 2);
    let change = requester(&world, first_review.task);
    assert_eq!(first_head, H1);
    assert_review_head(&world, &first_review, translate::commit(first_head));
    assert_eq!(live_task(&world, change).numbers, tasks::Numbers { budget: 10, spent: 0, spent_below: 3, reserved: 7 });
    assert_eq!(live_task(&world, first_review.task).numbers.budget, 7);
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: first_head,
            context: b"build".as_slice().into(),
            state: raw::Check::Failed,
        }),
    );
    for _ in 0..MAX_TICKS {
        world.tick();
        if change_row(&world, change).change.state == (State::CancellingGates { head: translate::commit(first_head) }) {
            break;
        }
    }
    let row = change_row(&world, change);
    assert_eq!(row.change.state, State::CancellingGates { head: translate::commit(first_head) });
    assert_eq!(row.change.repairs, 0);
    assert_eq!(world.assigned.len(), 3, "no repair while the review holds its funds");
    let tasks::Phase::Closing(closing) = &live_task(&world, first_review.task).phase else {
        panic!("review was not cancelled: {:?}", live_task(&world, first_review.task).phase);
    };
    let tasks::Ending::Cancelled { reason, .. } = &closing.ending else { panic!("cancelled review") };
    assert!(reason.windows(b"CI failed at head".len()).any(|part| part == b"CI failed at head"));
    // The worker drains only now. Even a racing approval is discarded on this failed head.
    let event = world.protocol.answer(
        CHANNEL,
        first_review.task,
        first_review.attempt,
        0,
        tasks::End::Finished {
            result: tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    let repair = next_assignment(&mut world, 3);
    assert!(matches!(live_task(&world, repair.task).contract, tasks::Contract::Change { .. }));
    assert_eq!(live_task(&world, repair.task).numbers.budget, 7, "cancelled reservation funds the repair");
    let row = change_row(&world, change);
    assert_eq!(row.change.repairs, 1);
    assert!(row.verdicts.is_empty());
    assert!(row.change.gate_failures.is_empty(), "cancellation uses no failed-review retry");
    assert_eq!(live_task(&world, change).made, 4, "producer, cancelled review and repair each count as made");
    let second_head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, FILE, b"repaired", 1)
        .expect("repair pushed H2");
    assert_ne!(second_head, first_head);
    let event = world.protocol.answer(
        CHANNEL,
        repair.task,
        repair.attempt,
        0,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"repaired".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    let second_review = next_assignment(&mut world, 4);
    assert_review_head(&world, &second_review, translate::commit(second_head));
    assert_ne!(first_review.task, second_review.task);
    let event = world.protocol.answer(
        CHANNEL,
        second_review.task,
        second_review.attempt,
        0,
        tasks::End::Finished {
            result: tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    for _ in 0..MAX_TICKS {
        world.tick();
        if world.merges == 1 {
            break;
        }
    }
    assert_eq!(world.merges, 1, "H2 lands after its fresh review");
    let row = change_row(&world, change);
    assert_eq!(row.change.repairs, 1);
    assert!(row.change.gate_failures.is_empty());
    assert!(
        row.verdicts
            .iter()
            .all(|report| report.head == translate::commit(second_head) && report.status == Status::Passed)
    );
}
