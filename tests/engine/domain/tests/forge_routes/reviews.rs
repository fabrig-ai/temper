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

fn next_assignment(world: &mut World, index: usize) -> engine::Assignment {
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

fn review_world() -> (World, engine::Assignment, [u8; 32]) {
    let (mut world, _, producer, branch) = change_world_with_gate(false, 1000, None, true);
    let head = produce(&mut world, &producer, &branch);
    let reviewer = next_assignment(&mut world, 2);
    (world, reviewer, head)
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
            Record::Core(_) | Record::Delivery { .. } => None,
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

fn finish_review(world: &mut World, reviewer: &engine::Assignment, result: tasks::TaskResult) {
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
            Record::Core(_) | Record::Delivery { .. } => None,
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
