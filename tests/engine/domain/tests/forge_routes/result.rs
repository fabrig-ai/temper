//! The requester reads all the evidence of a CSV change's repair and landing.
use super::*;

const CSV: &[u8] = b"fn main() { println!(\"fruit,count\\napples,3\"); }\n";
fn finish(world: &mut World, run: &engine::Assignment, result: tasks::TaskResult) {
    let event = world.protocol.answer(
        Token::new(7),
        run.task,
        run.attempt,
        5,
        tasks::End::Finished { result, cancel_delegates: false },
        engine::Parts::none(),
    );
    world.send(event);
}
fn change_result() -> tasks::TaskResult {
    tasks::TaskResult::Change {
        connector: 0,
        kind: 2,
        resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
        words: b"pushed".as_slice().into(),
    }
}
fn next(world: &mut World, index: usize) -> engine::Assignment {
    for _ in 0..200 {
        world.tick();
        if let Some(run) = world.assigned.get(index) {
            return run.clone();
        }
    }
    panic!("delegate {index} was not assigned");
}
#[test]
fn the_csv_changes_requester_reads_the_failed_context_repair_and_approved_review_whole() {
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate { agent: true, ..Gate::default() },
        None,
        StoryLimits { result_bytes: Some(512), ..StoryLimits::default() },
    );
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    let answer = super::change_tool::change(&mut world, &chat, 93, None, b"Export the widget table as CSV");
    let engine::CallAnswer::Delegated(made) = answer else { panic!("change delegated") };
    let change = made[0].task;
    let producer = next(&mut world, 1);
    let branch = producer.workspace.repositories[0].push.clone().expect("change branch");
    // Park the requester so its next assignment must carry the whole result.
    let event =
        world.protocol.answer(Token::new(7), chat.task, chat.attempt, 5, tasks::End::Parked, engine::Parts::none());
    world.send(event);
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    let h1 =
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"src/main.rs", b"fn main() {}\n", 1)
            .expect("producer pushed its first source");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: h1,
            context: b"csv header".as_slice().into(),
            state: raw::Check::Failed,
        }),
    );
    finish(&mut world, &producer, change_result());
    let repair = next(&mut world, 2);
    assert!(contains(ci_words(&repair), b"Failed check: csv header\n"));
    let h2 = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"src/main.rs", CSV, 1)
        .expect("repair pushed the CSV source");
    finish(&mut world, &repair, change_result());
    let review = next(&mut world, 3);
    finish(
        &mut world,
        &review,
        tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
    );
    let requester = next(&mut world, 4);
    assert_eq!(requester.task, chat.task);
    let Some(Record::Forge { row, .. }) = world.store.rows.get(&Key::Forge(forge_top::Key::Change(change))) else {
        panic!("change row")
    };
    let forge_top::Stored::Change(row) = row.as_ref() else { panic!("change row") };
    let temper_engine_domain_forge_change::State::Landed { merge } = row.change.state else { panic!("landed") };
    let merge = String::from_utf8(forge_top::commit_text(&merge).into_vec()).expect("hexadecimal merge id");
    let mut head = [0; 32];
    head[..20].copy_from_slice(&fake::object_bytes(h2));
    let head = String::from_utf8(forge_top::commit_text(&head).into_vec()).expect("hexadecimal head id");
    let result = requester
        .messages
        .iter()
        .find(|word| {
            word.from == tasks::Party::Task(change)
                && matches!(word.kind, tasks::MessageKind::Result(tasks::ResultKind::Change { .. }))
        })
        .expect("requester's result");
    let words = core::str::from_utf8(&result.words).expect("UTF-8 result");
    let lines: Vec<_> = words.lines().collect();
    assert_eq!(lines.len(), 5, "{words}");
    assert_eq!(lines[0], merge);
    assert!(lines[1].starts_with("pull request #"), "{words}");
    assert_eq!(lines[2], format!("checks at {head}: build passed"));
    assert_eq!(lines[3], "repairs: 1: CI failed: csv header");
    assert_eq!(lines[4], format!("reviews: approved at {head}"));
    assert!(!words.contains("bytes cut"), "the requester reads it whole");
}
