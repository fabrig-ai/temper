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
