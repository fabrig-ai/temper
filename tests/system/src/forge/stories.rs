//! Smith runs whose forge changes land through the root and the fake forge.
use super::*;
use crate::world as smith_world;
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use smith_agent_world::{Job, World as Agent};
use smith_domain_run as run;
use temper_engine_smith::ChangeResource;

const DELEGATE: &[u8] = br#"{"batch":[{"executor":{"kind":"procedure","connector":0,"code":2},"spec":{"words":"@coding Small fix","parameters":[{"name":1,"kind":"resource","connector":0,"resource":4294967298},{"name":2,"kind":"bytes","value":"main"}]},"contract":{"kind":"change","connector":0,"change_kind":1,"words":32},"authority":{"tools":1,"grants":[{"connector":0,"kind":2,"segments":["forge","forge.example","org"],"terminal":"open","last":"repo"},{"connector":0,"kind":3,"segments":["forge","forge.example","org"],"terminal":"open","last":"repo"},{"connector":0,"kind":4,"segments":["forge","forge.example","org"],"terminal":"open","last":"repo"}],"delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":2,"depth":1},"budget":{"spend":20},"notes":0}}]}"#;

fn chat_script() -> Script {
    let call = |name: &[u8], arguments: &[u8]| Turn {
        lines: Box::new([Line::Call { name: name.into(), arguments: arguments.into() }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    };
    Script {
        cue: b"@forge_chat".as_slice().into(),
        turns: Box::new([
            call(b"delegate", DELEGATE),
            call(b"wait", b"{}"),
            call(b"wait", b"{}"),
            call(b"wait", b"{}"),
        ]),
    }
}

fn repair_script() -> Script {
    Script {
        cue: b"Repair the failed CI".as_slice().into(),
        turns: Box::new([
            Turn {
                lines: Box::new([Line::Call {
                    name: b"read".as_slice().into(),
                    arguments: br#"{"path":"src/lib.rs"}"#.as_slice().into(),
                }]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
            Turn {
                lines: Box::new([Line::Call {
                    name: b"edit".as_slice().into(),
                    // smith's edit takes an ordered list (smith 495230d4).
                    arguments: br#"{"path":"src/lib.rs","edits":[{"old":"42","new":"43"}]}"#.as_slice().into(),
                }]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
            Turn {
                lines: Box::new([Line::Call {
                    name: b"finish".as_slice().into(),
                    arguments:
                        br#"{"title":"Repair the failed check","body":"The answer is 43 now."}"#.as_slice().into(),
                }]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
        ]),
    }
}

fn start_change_world(passes: u32) -> (World, engine::Assignment) {
    let mut world = World::configured_policy(true, true, false, passes, None);
    world.adopt();
    world.send(smith_world::hello(2));
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(92)),
        sign_in: world.signed_in.expect("owner session"),
        key: [92; 16],
        ask: world.chat(1, b"@forge_chat"),
    }));
    world.until(Until::Assigned);
    let chat = world.assigned[0].clone();
    (world, chat)
}

fn run_smith(
    world: &mut World,
    assignment: &engine::Assignment,
    mut agent: Agent,
    change: Option<ChangeResource>,
) -> Agent {
    agent.enable_parent_host_calls();
    loop {
        if agent.drive(1000) {
            break;
        }
        let pending: Vec<_> = agent.pending_host_calls().into_iter().cloned().collect();
        assert!(!pending.is_empty(), "Smith either settles or yields a host call");
        for submission in pending {
            let temper_engine_smith::Decoded::Call(body) =
                temper_engine_smith::call(submission.name, &submission.tool, &submission.input).unwrap_or_else(
                    |problem| {
                        panic!(
                            "declared Smith tool input: {problem:?} tool={} input={}",
                            String::from_utf8_lossy(&submission.tool),
                            String::from_utf8_lossy(submission.input.bytes())
                        )
                    },
                )
            else {
                panic!("script tool answered at decode")
            };
            let before = world.answers.len();
            let call = world.protocol.call(
                smith_world::CHANNEL,
                assignment.task,
                assignment.attempt,
                submission.relay.owner.raw(),
                body,
            );
            world.send(call);
            for _ in 0..200 {
                world.tick();
                if world.answers.len() > before {
                    break;
                }
            }
            let answer = world.answers.get(before).expect("root committed host answer");
            agent
                .return_host_reply(submission.relay, run::HostReply::Answered(temper_engine_smith::answer(answer)))
                .expect("one pending Smith relay");
        }
    }
    for (number, read, spent) in agent.turn_metadata() {
        let record = &agent.turns()[usize::try_from(*number - 1).expect("positive turn")];
        let turn = smith_world::turn(assignment, *number, *read, *spent, record);
        let (run, attempt) = (Token::new(assignment.task), Token::new(assignment.attempt));
        world.send(protocol::up(
            smith_world::CHANNEL,
            host::LinkUp::Turn { run, attempt, turn },
            engine::Parts::none(),
        ));
        for _ in 0..8 {
            world.tick();
        }
    }
    let terminal =
        temper_engine_smith::result(smith_world::copy_answer(agent.answer()), change).expect("typed Smith result");
    let answer = world.protocol.answer(
        smith_world::CHANNEL,
        assignment.task,
        assignment.attempt,
        terminal.cumulative,
        terminal.end,
        engine::Parts::none(),
    );
    world.send(answer);
    world.until(Until::Ready);
    agent
}

#[test]
fn the_smith_delegate_input_creates_the_producer() {
    let (mut world, chat) = start_change_world(1000);
    let input = run::HostInput::attested(DELEGATE.into()).expect("bounded Smith host input");
    let temper_engine_smith::Decoded::Call(body) = temper_engine_smith::call(
        run::CallName { activation: chat.attempt, completion: 1, position: 1 },
        b"delegate",
        &input,
    )
    .expect("declared delegate shape") else {
        panic!("delegate answered at decode")
    };
    let call = world.protocol.call(smith_world::CHANNEL, chat.task, chat.attempt, 93, body);
    world.send(call);
    world.until(Until::Delegated);
    world.until(Until::SecondAssignment);
    let producer = world.assigned[1].clone();
    let mut agent = smith_world::agent_for(&producer, None, Job::Coding);
    agent.run(1000);
    assert!(
        matches!(agent.answer(), run::Answer::Accepted { outcome: run::outcome::Declared::Change(_), .. }),
        "{:?}",
        agent.answer()
    );
}

#[test]
fn a_small_fix_made_in_a_smith_chat_lands_through_the_fake_forge() {
    let (mut world, chat) = start_change_world(1000);
    let agent = smith_world::scripted_agent_for(&chat, chat_script());
    let parent = run_smith(&mut world, &chat, agent, None);
    assert!(matches!(parent.answer(), run::Answer::Parked { .. }));
    world.until(Until::SecondAssignment);
    let producer = world.assigned[1].clone();
    let branch = producer.workspace.repositories[0].push.clone().expect("writable change branch");
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    let agent = smith_world::agent_for(&producer, None, Job::Coding);
    let pushed = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"fixed", 1)
        .expect("worker pushed its branch");
    assert!(pushed > 1);
    let producer_agent = run_smith(
        &mut world,
        &producer,
        agent,
        Some(ChangeResource {
            connector: 0,
            kind: 2,
            resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
        }),
    );
    assert!(matches!(
        producer_agent.answer(),
        run::Answer::Accepted { outcome: run::outcome::Declared::Change(_), .. }
    ));
    for _ in 0..200 {
        world.tick();
        if world.store.rows.values().any(|stored| {
            matches!(stored,
                Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(change)
                    if landed(&change.change.state))
            )
        }) {
            break;
        }
    }
    assert!(world.store.rows.values().any(|stored| matches!(stored,
        Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(change)
            if landed(&change.change.state))
    )));
}

#[test]
#[expect(clippy::too_many_lines, reason = "the scenario follows the Smith repair and forge landing sequence")]
fn a_smith_change_with_failed_ci_is_repaired_reviewed_and_landed() {
    let (mut world, chat) = start_change_world(0);
    let agent = smith_world::scripted_agent_for(&chat, chat_script());
    let parent = run_smith(&mut world, &chat, agent, None);
    assert!(matches!(parent.answer(), run::Answer::Parked { .. }));
    world.until(Until::SecondAssignment);
    let producer = world.assigned[1].clone();
    let branch = producer.workspace.repositories[0].push.clone().expect("writable change branch");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: 1,
            context: b"build".as_slice().into(),
            state: raw::Check::Passed,
            description: Box::new([]),
            link: Box::new([]),
        }),
    );
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    let failed_head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"first", 1)
        .expect("producer pushed first head");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: failed_head,
            context: b"build".as_slice().into(),
            state: raw::Check::Failed,
            description: Box::new([]),
            link: Box::new([]),
        }),
    );
    let producer_agent = run_smith(
        &mut world,
        &producer,
        smith_world::agent_for(&producer, None, Job::Coding),
        Some(ChangeResource {
            connector: 0,
            kind: 2,
            resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
        }),
    );
    assert!(matches!(
        producer_agent.answer(),
        run::Answer::Accepted { outcome: run::outcome::Declared::Change(_), .. }
    ));
    for _ in 0..200 {
        world.tick();
        if world.assigned.len() >= 3 {
            break;
        }
    }
    let repair = world.assigned.get(2).expect("failed CI assigned a repair").clone();
    let failed_step = b"Failed step: 1 Run build\nEnd of its log:\n[log truncated before this]\n";
    assert!(
        repair.sections.iter().any(|section| section.kind == engine::BriefKind::Forge(engine::ForgeBriefKind::Ci)
            && matches!(&section.body, engine::BriefBody::Text(words)
            if words.windows(failed_step.len()).any(|part| part == failed_step)
                && words.windows(b"failing check build 3\n".len()).any(|part| part == b"failing check build 3\n"))),
        "CI sections: {:?}",
        repair.sections
    );
    let repaired_head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"repaired", 1)
        .expect("repair pushed a new head");
    for _ in 0..5 {
        world.tick();
    }
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: repaired_head,
            context: b"build".as_slice().into(),
            state: raw::Check::Passed,
            description: Box::new([]),
            link: Box::new([]),
        }),
    );
    let pull = world
        .store
        .rows
        .values()
        .find_map(|stored| match stored {
            Record::Forge { row, .. } => match row.as_ref() {
                forge_top::Stored::Change(change) => change.pull,
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("opened pull request");
    assert!(matches!(
        world.external(
            2,
            raw::Op::Write(raw::Write::Review {
                number: pull,
                verdict: Some(raw::Verdict::Approve),
                body: b"Reviewed repaired head".as_slice().into(),
            })
        ),
        raw::Answer::Reviewed(_)
    ));
    let repair_agent = run_smith(
        &mut world,
        &repair,
        smith_world::scripted_coding_agent_for(&repair, repair_script()),
        Some(ChangeResource {
            connector: 0,
            kind: 2,
            resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
        }),
    );
    assert!(
        matches!(repair_agent.answer(), run::Answer::Accepted { outcome: run::outcome::Declared::Change(_), .. }),
        "{:?}",
        repair_agent.answer()
    );
    for _ in 0..200 {
        world.tick();
        if world.store.rows.values().any(|stored| {
            matches!(stored,
                Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(change)
                    if landed(&change.change.state))
            )
        }) {
            break;
        }
    }
    assert!(world.store.rows.values().any(|stored| matches!(stored,
        Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(change)
            if landed(&change.change.state)
                && change.change.repairs == 1)
    )));
}
