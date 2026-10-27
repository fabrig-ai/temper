//! A chat's change goes through the same named delegation admission as a raw batch.
use super::*;

fn chat_world(adopt: bool) -> (World, engine::Assignment) {
    let mut world = World::configured(true, true);
    if adopt {
        world.adopt();
    } else {
        world.sign_in();
    }
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    (world, chat)
}

pub(super) fn change(
    world: &mut World,
    chat: &engine::Assignment,
    name: u64,
    repository: Option<&[u8]>,
    words: &[u8],
) -> engine::CallAnswer {
    let before = world.answers.len();
    let event = world.protocol.call(
        Token::new(7),
        chat.task,
        chat.attempt,
        name,
        engine::Call {
            completion: u32::try_from(name).expect("bounded call name"),
            position: 1,
            tool: engine::Tool::Change { words: words.into(), repository: repository.map(Box::from) },
        },
    );
    world.send(event);
    for _ in 0..200 {
        world.tick();
        if world.answers.len() > before {
            return world.answers[before].clone();
        }
    }
    panic!("change was not answered");
}

fn live(world: &World, task: u64) -> &tasks::TaskRecord {
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(task))))
    else {
        panic!("live task {task}")
    };
    row
}

#[test]
fn change_expands_the_only_repository_field_by_field() {
    let (mut world, chat) = chat_world(true);
    let parent = live(&world, chat.task).clone();
    let answer = change(&mut world, &chat, 93, None, b"Small fix");
    let engine::CallAnswer::Delegated(made) = answer else { panic!("delegated: {answer:?}") };
    let row = live(&world, made[0].task);
    assert_eq!(row.project, parent.project);
    assert_eq!(row.executor, tasks::Executor::Procedure { connector: 0, code: 2 });
    assert_eq!(
        row.spec,
        tasks::Spec {
            words: Box::from(&b"Small fix"[..]),
            parameters: Box::new([
                tasks::Parameter::Resource {
                    name: 1,
                    connector: 0,
                    resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository)
                },
                tasks::Parameter::Bytes { name: 2, value: Box::from(&b"main"[..]) },
            ]),
            inputs: Box::new([]),
        }
    );
    assert_eq!(
        row.contract,
        tasks::Contract::Change { connector: 0, kind: 1, words: 512.min(world.env.limits.tasks.result_bytes) }
    );
    assert_eq!(row.authority.tools, parent.authority.tools);
    assert_eq!(
        row.authority.grants,
        [1, 2, 3, 4, 10, 11, 12]
            .into_iter()
            .flat_map(|kind| {
                let prefix = [b"forge".as_slice(), b"forge.example", b"org"];
                [
                    (kind == 1).then(|| tasks::Grant {
                        connector: 0,
                        kind,
                        pattern: tasks::Pattern {
                            segments: prefix.map(Box::from).into(),
                            last: tasks::Last::Exact(Box::from(b"repo".as_slice())),
                        },
                    }),
                    Some(tasks::Grant {
                        connector: 0,
                        kind,
                        pattern: tasks::Pattern {
                            segments: [b"forge".as_slice(), b"forge.example", b"org", b"repo"].map(Box::from).into(),
                            last: tasks::Last::Open(Box::new([])),
                        },
                    }),
                ]
                .into_iter()
                .flatten()
            })
            .collect::<Box<[_]>>()
    );
    assert_eq!(row.authority.delegation.kinds.as_ref(), &[tasks::AuthorityExecutor::Charter(1)]);
    assert_eq!(row.authority.delegation.tasks, 4);
    assert_eq!(row.authority.delegation.depth, 2.min(parent.authority.delegation.depth - 1));
    assert_eq!(
        row.authority.budget.spend,
        (parent.numbers.budget - parent.numbers.spent - parent.numbers.spent_below - parent.numbers.reserved) / 2
    );
    assert_eq!(row.authority.budget.deadline, parent.authority.budget.deadline);
    assert_eq!(row.authority.notes, tasks::Scopes(0));
    assert!(row.authority.note_resources.is_empty());
    assert!(row.dependencies.is_empty());
    assert_eq!(row.wake, tasks::WakePolicy::DEFAULT);
    assert_eq!(row.label, 0);
    assert_eq!(row.flags, tasks::Flags::PLAIN);
    assert!(row.llm.is_none());
    assert_eq!(made[0].given, None, "named funding");
    world.until(Until::SecondAssignment);
    assert!(
        world.assigned[1].run.contract
            == tasks::Contract::Change { connector: 0, kind: 2, words: world.env.limits.tasks.result_bytes },
        "the same code-2 procedure starts its producer"
    );
}

fn producer(words: &[u8]) -> (World, tasks::TaskRecord) {
    let (mut world, chat) = chat_world(true);
    let engine::CallAnswer::Delegated(_) = change(&mut world, &chat, 93, None, words) else { panic!("delegated") };
    world.until(Until::SecondAssignment);
    let row = live(&world, world.assigned[1].task).clone();
    (world, row)
}

#[test]
fn the_producer_hears_the_change_task_lands_what_it_delivers_when_its_bound_has_room() {
    let (world, row) = producer(b"Small fix");
    assert_eq!(
        row.spec.words.as_ref(),
        b"Small fix\n\nThe host pushes what you deliver; the change task opens the pull request, \
waits for CI and merges it. Finish with your change once it is checked."
            .as_slice(),
        "the change's words, then the producer's scope"
    );
    // Words that leave no room for the scope beside the branches go alone.
    let bound = usize::try_from(world.env.limits.tasks.spec_bytes).expect("bound fits");
    let words = vec![b'x'; bound - b"main".len() - 64];
    let (_, row) = producer(&words);
    let branch = row
        .spec
        .parameters
        .iter()
        .find_map(|parameter| match parameter {
            tasks::Parameter::Bytes { name: 3, value } => Some(value.len()),
            _ => None,
        })
        .expect("the producer's branch");
    assert!(branch <= 64, "the branch fits beside the words: {branch}");
    assert_eq!(row.spec.words.as_ref(), words.as_slice(), "the words alone");
}

#[test]
fn change_with_no_repository_is_refused_once() {
    let (mut world, chat) = chat_world(false);
    let first = change(&mut world, &chat, 93, None, b"Small fix");
    assert!(matches!(first, engine::CallAnswer::ChangeRefused { ref repositories, .. } if repositories.is_empty()));
    let task_rows = world
        .store
        .rows
        .keys()
        .filter(|key| matches!(key, Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(_)))))
        .count();
    let again = change(&mut world, &chat, 93, None, b"Small fix");
    assert_eq!(again, first);
    assert_eq!(
        world
            .store
            .rows
            .keys()
            .filter(|key| matches!(key, Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(_)))))
            .count(),
        task_rows
    );
    let key = jig_domain_core::CallKey { task: chat.task, attempt: chat.attempt, completion: 93, position: 1 };
    assert!(world.store.rows.contains_key(&Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Call(key)))));
}

#[test]
fn change_with_an_unknown_repository_names_the_available_one() {
    let (mut world, chat) = chat_world(true);
    let first = change(&mut world, &chat, 93, Some(b"org/missing"), b"Small fix");
    assert!(
        matches!(&first, engine::CallAnswer::ChangeRefused { named: Some(named), repositories, .. } if named.as_ref() == b"org/missing" && repositories.as_ref() == [Box::from(&b"org/repo"[..])])
    );
    assert_eq!(change(&mut world, &chat, 93, Some(b"org/missing"), b"Small fix"), first);
    assert_eq!(world.assigned.len(), 1);
}

#[test]
fn change_accepts_a_second_change_within_the_chats_task_allotment() {
    let (mut world, chat) =
        configured_chat(100, Box::new([people::Executor::Charter(1), people::Executor::Procedure(2)]), 7);
    assert!(matches!(change(&mut world, &chat, 93, None, b"First fix"), engine::CallAnswer::Delegated(_)));
    assert!(matches!(change(&mut world, &chat, 94, None, b"Second fix"), engine::CallAnswer::Delegated(_)));
}

#[test]
fn change_refuses_oversized_words_as_a_delegate_spec() {
    let (mut world, chat) = chat_world(true);
    let words = vec![b'x'; usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize") + 1];
    assert!(
        matches!(change(&mut world, &chat, 93, None, &words), engine::CallAnswer::DelegationRefused(problem) if problem.why == tasks::Refusal::Spec)
    );
    assert_eq!(world.assigned.len(), 1);
}

fn configured_chat(cap: u64, kinds: Box<[people::Executor]>, tasks: u32) -> (World, engine::Assignment) {
    let mut world = World::configured(true, true);
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let people::Ask::Create { project, mut batch } = adoption_chat(&world) else { panic!("create") };
    batch[0].budget = people::Budget::AsAllowed { cap: Some(cap) };
    batch[0].authority.delegation.kinds = kinds;
    batch[0].authority.delegation.tasks = tasks;
    let chat = assigned_chat(&mut world, 92, people::Ask::Create { project, batch });
    (world, chat)
}

#[test]
fn change_without_procedure_permission_gets_delegates_executor_denial() {
    let (mut world, chat) = configured_chat(100, Box::new([people::Executor::Charter(1)]), 11);
    assert!(
        matches!(change(&mut world, &chat, 93, None, b"Small fix"), engine::CallAnswer::DelegationDenied { findings, .. } if findings.contains(&authority::Finding::Executor { source: authority::Source::Task }))
    );
    assert_eq!(world.assigned.len(), 1);
}

#[test]
fn change_without_task_capacity_gets_delegates_tasks_denial() {
    let (mut world, chat) =
        configured_chat(100, Box::new([people::Executor::Charter(1), people::Executor::Procedure(2)]), 0);
    assert!(
        matches!(change(&mut world, &chat, 93, None, b"Small fix"), engine::CallAnswer::DelegationDenied { findings, .. } if findings.contains(&authority::Finding::Tasks { source: authority::Source::Task }))
    );
}

#[test]
fn change_with_one_task_left_is_refused_as_the_tree_limit() {
    let (mut world, chat) =
        configured_chat(100, Box::new([people::Executor::Charter(1), people::Executor::Procedure(2)]), 1);
    assert!(
        matches!(change(&mut world, &chat, 93, None, b"Small fix"), engine::CallAnswer::DelegationRefused(problem) if problem.why == tasks::Refusal::Tree)
    );
    assert_eq!(world.assigned.len(), 1);
}

#[test]
fn change_with_one_level_left_is_refused_as_the_depth_limit() {
    let mut world = World::configured(true, true);
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let people::Ask::Create { project, mut batch } = adoption_chat(&world) else { panic!("create") };
    batch[0].authority.delegation.depth = 1;
    let chat = assigned_chat(&mut world, 92, people::Ask::Create { project, batch });
    assert!(
        matches!(change(&mut world, &chat, 93, None, b"Small fix"), engine::CallAnswer::DelegationRefused(problem) if problem.why == tasks::Refusal::Depth)
    );
    assert_eq!(world.assigned.len(), 1);
}

#[test]
fn change_with_half_the_remainder_at_minimum_is_refused_as_funding() {
    let (mut world, chat) =
        configured_chat(62, Box::new([people::Executor::Charter(1), people::Executor::Procedure(2)]), 11);
    let row = live(&world, chat.task);
    assert_eq!(
        (row.numbers.budget - row.numbers.reserved - row.numbers.spent - row.numbers.spent_below) / 2,
        1,
        "fixture leaves half at the minimum"
    );
    assert!(
        matches!(change(&mut world, &chat, 93, None, b"Small fix"), engine::CallAnswer::DelegationRefused(problem) if problem.why == tasks::Refusal::Funding)
    );
}

fn several_repositories() -> (World, engine::Assignment) {
    let mut world = World::configured(true, true);
    world.adopt();
    world.until(Until::Ready);
    fake::repository(
        &mut world.fake,
        &world.fake_env.limits,
        raw::Setup {
            workflows: Box::new([]),
            id: 3,
            name: Box::from(&b"org/extra"[..]),
            default: Box::from(&b"main"[..]),
            tree: Box::new([]),
            labels: Box::new([]),
            checks: raw::Checks {
                contexts: Box::new([]),
                latency_min: Duration::ZERO,
                latency_max: Duration::ZERO,
                silent: 0,
                passes: 1000,
                reruns: 0,
                cue: None,
            },
            protection: None,
            hooked: false,
        },
    );
    fake::grant(&mut world.fake, b"org/extra", 1, raw::Permission::Admin);
    world.send(engine::Event::Party(engine::PartyInput::Application {
        scope: people::ApplicationScope::Project(1),
        reply_to: ReplyTo::new(Token::new(95)),
        sign_in: world.signed_in.expect("owner session"),
        key: [95; 16],
        digest: [6; 32],
        carried: None,
        request: engine::Application::Forge(forge_top::Ask::Adopt(forge_top::Adoption {
            project: 1,
            home: false,
            forge: forge_world::REPO.forge,
            host: Box::from(&b"forge.example"[..]),
            owner: Box::from(&b"org"[..]),
            name: Box::from(&b"extra"[..]),
            prefix: Box::from(&b"temper/"[..]),
            role: forge_top::Role::Owned,
            landing: Box::from(&b"main"[..]),
            ci: true,
            checks: Box::new([]),
        })),
    }));
    for _ in 0..200 {
        world.tick();
        if world.adopted.len() == 2 {
            break;
        }
    }
    assert_eq!(world.adopted.len(), 2, "second adoption: {:?}", world.adopted);
    assert!(matches!(world.adopted[1], forge_top::ApplicationOutcome::Adopted(_)), "{:?}", world.adopted);
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    (world, chat)
}

#[test]
fn change_with_several_unnamed_repositories_is_refused_once() {
    let (mut world, chat) = several_repositories();
    let first = change(&mut world, &chat, 93, None, b"Small fix");
    assert!(
        matches!(&first, engine::CallAnswer::ChangeRefused { named: None, repositories, .. } if repositories.len() == 2),
        "{first:?}"
    );
    assert_eq!(change(&mut world, &chat, 93, None, b"Small fix"), first);
    assert_eq!(world.assigned.len(), 1);
}

#[test]
fn change_selects_the_named_repository_among_several() {
    let (mut world, chat) = several_repositories();
    let engine::CallAnswer::Delegated(made) = change(&mut world, &chat, 93, Some(b"org/extra"), b"Small fix") else {
        panic!("delegated")
    };
    assert_eq!(
        live(&world, made[0].task).spec.parameters[0],
        tasks::Parameter::Resource { name: 1, connector: 0, resource: forge_top::tag(forge_world::REPO.forge, 3) }
    );
}

fn named_read(
    world: &mut World,
    chat: &engine::Assignment,
    completion: u32,
    tool: &[u8],
    input: &[u8],
) -> engine::CallAnswer {
    let before = world.answers.len();
    let input = smith_domain_run::HostInput::attested(input.into()).expect("bounded input");
    let temper_engine_smith::Decoded::Call(body) = temper_engine_smith::call(
        smith_domain_run::CallName { activation: chat.attempt, completion, position: 1 },
        tool,
        &input,
    )
    .expect("flat read") else {
        panic!("root read");
    };
    let event = world.protocol.call(Token::new(7), chat.task, chat.attempt, u64::from(completion), body);
    world.send(event);
    for _ in 0..200 {
        world.tick();
        if world.answers.len() > before {
            return world.answers[before].clone();
        }
    }
    panic!("read was not answered");
}

#[test]
fn a_chat_reads_its_only_repository_by_branch_without_a_repository_name() {
    let (mut world, chat) = chat_world(true);
    let answer = named_read(&mut world, &chat, 93, b"read_branch", br#"{"branch":"main"}"#);
    assert!(matches!(answer, engine::CallAnswer::ForgeRead(ref result) if result.is_ok()), "{answer:?}");
    assert!(world.store.pending.is_empty(), "a read needs no commit");
}

#[test]
fn an_unnamed_read_with_two_repositories_names_both() {
    let (mut world, chat) = several_repositories();
    let answer = named_read(&mut world, &chat, 93, b"read_branch", br#"{"branch":"main"}"#);
    assert!(
        matches!(answer, engine::CallAnswer::ReadRepositoryRefused { named: None, ref repositories }
        if repositories.as_ref() == [Box::from(b"org/repo".as_slice()), Box::from(b"org/extra".as_slice())]),
        "{answer:?}"
    );
}

#[test]
fn a_read_with_an_unknown_name_names_the_project_repositories() {
    let (mut world, chat) = chat_world(true);
    let answer = named_read(&mut world, &chat, 93, b"read_branch", br#"{"repository":"org/missing","branch":"main"}"#);
    assert!(
        matches!(answer, engine::CallAnswer::ReadRepositoryRefused { named: Some(ref named), ref repositories }
        if named.as_ref() == b"org/missing" && repositories.as_ref() == [Box::from(b"org/repo".as_slice())]),
        "{answer:?}"
    );
}

#[test]
fn a_named_context_repository_is_readable() {
    let (mut world, chat) = several_repositories();
    world.send(engine::Event::Party(engine::PartyInput::Application {
        scope: people::ApplicationScope::Project(1),
        reply_to: ReplyTo::new(Token::new(96)),
        sign_in: world.signed_in.expect("owner session"),
        key: [96; 16],
        digest: [6; 32],
        carried: None,
        request: engine::Application::Forge(forge_top::Ask::Adopt(forge_top::Adoption {
            project: 1,
            home: true,
            forge: forge_world::REPO.forge,
            host: b"forge.example".as_slice().into(),
            owner: b"org".as_slice().into(),
            name: b"repo".as_slice().into(),
            prefix: b"temper/".as_slice().into(),
            role: forge_top::Role::Context,
            landing: b"main".as_slice().into(),
            ci: true,
            checks: Box::new([]),
        })),
    }));
    for _ in 0..200 {
        world.tick();
        if world.adopted.len() == 3 {
            break;
        }
    }
    assert_eq!(world.adopted.len(), 3);
    let answer = named_read(&mut world, &chat, 93, b"read_branch", br#"{"repository":"org/repo","branch":"main"}"#);
    assert!(matches!(answer, engine::CallAnswer::ForgeRead(ref result) if result.is_ok()), "{answer:?}");
}

#[test]
fn a_chat_read_pull_for_reaches_the_fake_forge_through_named_routing() {
    let (mut world, chat) = chat_world(true);
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: b"work".as_slice().into(), commit: 1 })),
        raw::Answer::Branch(_)
    ));
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"work", b"file", b"change", 1).expect("head");
    let raw::Answer::Created(_) = world.external(
        2,
        raw::Op::Write(raw::Write::OpenPull {
            opens_empty: false,
            title: b"Named read".as_slice().into(),
            body: Box::new([]),
            head: b"work".as_slice().into(),
            base: b"main".as_slice().into(),
        }),
    ) else {
        panic!("pull opened");
    };
    let before = world.forge_calls;
    let answer = named_read(
        &mut world,
        &chat,
        93,
        b"read_pull_for",
        br#"{"repository":"org/repo","head":"work","base":"main"}"#,
    );
    assert!(matches!(answer, engine::CallAnswer::ForgeRead(ref result) if result.is_ok()), "{answer:?}");
    assert!(world.forge_calls > before, "the read reached the fake forge");
    let rendered = temper_engine_smith::answer(&answer);
    assert!(!rendered.error());
    assert!(rendered.text().starts_with(b"Pull 1 head work commit "), "{:?}", rendered.text());
}

#[test]
fn a_chat_reads_its_landed_pulls_state_and_the_merge_the_fake_made() {
    let (mut world, chat, producer, branch) = change_world(false, 1000, None);
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"fixed", 1)
        .expect("producer pushed its branch");
    let event = world.protocol.answer(
        Token::new(7),
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
    for _ in 0..200 {
        world.tick();
        if world.merges == 1 {
            break;
        }
    }
    assert_eq!(world.merges, 1, "the chat's change landed");
    let raw::Answer::Pull(pull) = world.external(1, raw::Op::Read(raw::Read::Pull { number: 1 })) else {
        panic!("the fake's pull");
    };
    let merge = pull.merged.expect("the fake merged the pull");
    assert_ne!(merge, head, "the merge differs from the producer's head");
    let answer = named_read(&mut world, &chat, 94, b"read_pull", br#"{"repository":"org/repo","number":1}"#);
    let rendered = temper_engine_smith::answer(&answer);
    assert!(!rendered.error(), "{answer:?}");
    let mut expected = String::from("state closed merged commit ");
    for byte in fake::object_bytes(merge) {
        write!(&mut expected, "{byte:02x}").expect("merge digits");
    }
    assert!(rendered.text().windows(expected.len()).any(|part| part == expected.as_bytes()), "{:?}", rendered.text());
}

#[test]
fn an_omitted_file_limit_uses_the_engine_read_bound() {
    let (mut world, chat) = chat_world(true);
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"file", b"contents", 1)
        .expect("file head");
    let mut digits = String::new();
    for byte in translate::commit(head) {
        write!(&mut digits, "{byte:02x}").expect("commit hex");
    }
    let input = format!(r#"{{"head":"{digits}","path":"file"}}"#);
    let answer = named_read(&mut world, &chat, 93, b"read_file", input.as_bytes());
    assert!(matches!(answer, engine::CallAnswer::ForgeRead(ref result) if result.is_ok()), "{answer:?}");
    let rendered = temper_engine_smith::answer(&answer);
    assert!(rendered.text().windows(b"contents".len()).any(|part| part == b"contents"), "{:?}", rendered.text());
}

#[test]
fn a_decoded_job_zero_read_reaches_the_fake_and_is_answered() {
    let (mut world, chat) = chat_world(true);
    let input = smith_domain_run::HostInput::attested(br#"{"head":"0000000000000000000000000000000000000000","run":1,"job":0,"attempt":0,"max_bytes":32}"#.as_slice().into()).expect("bounded input");
    let temper_engine_smith::Decoded::Call(call) = temper_engine_smith::call(
        smith_domain_run::CallName { activation: chat.attempt, completion: 1, position: 1 },
        b"read_job",
        &input,
    )
    .expect("job zero decodes") else {
        panic!("read call")
    };
    assert!(
        matches!(&call.tool, engine::Tool::ReadNamed { read: skein_forge::api::Read::Job { attempt, .. }, .. } if attempt.job == 0 && attempt.run == 1 && attempt.attempt == 0)
    );
    let before = world.forge_calls;
    let event = world.protocol.call(Token::new(7), chat.task, chat.attempt, 93, call);
    world.send(event);
    world.until(Until::Read);
    assert!(world.forge_calls > before, "read reaches the independent fake");
    // The pinned fake assigns API job ids from 1; its UI's index 0 is not
    // an API id. It must answer MissingJob, never silently read job 1.
    assert!(
        matches!(world.answers.as_slice(), [engine::CallAnswer::ForgeRead(result)] if result.as_ref() == &Err(skein_forge::api::Error::MissingJob))
    );
}
