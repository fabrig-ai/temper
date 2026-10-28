use jig_client_world::harness::Until;
use skein_fake_person::{Find, Step};
use skein_ui::Role;
use skein_world::domain::Verdict;
use temper_client_world::fixtures;
#[test]
fn a_message_in_a_chat_is_answered_in_its_conversation() {
    let mut world = fixtures::world(fixtures::SEED);
    // This result-ended chat stays running until the person sends the message;
    // its initial task watch need not beat an already offered completion.
    world.front().reply_in_pieces(&[b"Working"]);
    let task = fixtures::chat(world.front(), b"Hello");
    let person = world.client(fixtures::PARTY, fixtures::chat_page(task));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN },
            Step::Type { find: Find::named(Role::TextArea, b"Message"), words: Box::from(*b"And then?") },
            Step::Press { find: Find::named(Role::Button, b"Send") },
            Step::See { find: Find::text(b"And then?"), within: fixtures::WITHIN },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    assert!(world.front().release_reply(), "finish the held run after the person's message");
    world.person(person, vec![Step::See { find: Find::text(b"Answered by the run"), within: fixtures::WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
}
#[test]
fn a_task_with_the_chat_label_opens_as_a_chat() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::chat(world.front(), b"Hello");
    let person = world.client(
        fixtures::PARTY,
        temper_client_domain::Address::Task { task, filter: temper_client_domain::PlanFilter::All },
    );
    world.person(person, vec![Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
}

const DIAGNOSTIC_NOW: skein_lib::Time = skein_lib::Time::ZERO;
const DIAGNOSTIC_WALL: skein_lib::Wall = skein_lib::Wall::EPOCH;
const POLL_ROUNDS: usize = 12;
const START_OFFSET: i32 = 0;
const DIAGNOSTIC_CLIENT: jig_client_world::harness::ClientId = jig_client_world::harness::ClientId(0);
#[test]
fn prepared_chat_answer_reaches_the_pages_task_watch() {
    use jig_client_world::harness::{App, Duty, Front};
    use temper_client_world::{app::Temper, front::Joint};
    let limits = fixtures::domain_limits();
    let mut front = Joint::new(fixtures::SEED, limits);
    front.answer_runs = true;
    let task = fixtures::chat(&mut front, b"Hello");
    front.bind(DIAGNOSTIC_CLIENT, fixtures::PARTY);
    let (mut root, _) = Temper::new(fixtures::SEED);
    let env = skein_lib::Env { now: DIAGNOSTIC_NOW, wall: DIAGNOSTIC_WALL, limits };
    let mut out = skein_lib::Queue::uncapped();
    Temper::step(&mut root, &env, Temper::start(fixtures::chat_page(task), Vec::new(), START_OFFSET), &mut out);
    for _ in 0..POLL_ROUNDS {
        Temper::take(&mut root, &env, &mut out);
        while let Some(request) = out.pop() {
            if let Duty::Wire(request) = Temper::duty(request) {
                front.wire(DIAGNOSTIC_CLIENT, env.now, env.wall, request);
            }
        }
        for (_, event) in front.poll(env.now, env.wall) {
            front.received(DIAGNOSTIC_CLIENT, &event);
            Temper::step(&mut root, &env, event, &mut out);
        }
    }
    let chat = root.person().pages().chat().expect("chat page");
    assert!(front.engine.task(task).is_none(), "the answer ends the real run");
    assert_eq!(
        chat.task(root.person().client()).expect("task document").chip.phase,
        jig_client::contract::Phase::Ended
    );
}
const PAST_CLOSE_GRACE: skein_lib::Duration = skein_lib::Duration::from_secs(6);
#[test]
fn a_closed_chat_shows_in_your_chats() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::chat(world.front(), b"A chat to close");
    let person = world.client(fixtures::PARTY, fixtures::chat_page(task));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Button, b"Close"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Close") },
            Step::See { find: Find::named(Role::Dialog, b"Close"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::Pause { span: PAST_CLOSE_GRACE },
            Step::Press { find: Find::exactly_named(Role::Link, b"Chats") },
            Step::See { find: Find::named(Role::Heading { level: 2 }, b"Your chats"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"A chat to close"), within: fixtures::WITHIN },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    assert!(world.front().engine.task(task).is_none(), "the closed chat left live work");
}
const CLOSE_REQUEST: skein_lib::Token = skein_lib::Token::new(1107);
const CLOSE_KEY: [u8; 16] = [37; 16];
const CLOSE_GRACE: skein_lib::Duration = skein_lib::Duration::from_secs(5);
const AFTER_CANCEL_ANSWER: skein_lib::Time = skein_lib::Time::from_nanos(6_100_000_000);
const AFTER_GRACE: skein_lib::Time = skein_lib::Time::from_nanos(6_000_000_000);
#[test]
fn a_prepared_close_expires_through_the_fronts_clock() {
    use jig_client_world::harness::Front;
    use temper_engine_domain::{self as engine, PartyInput};
    let mut front = temper_client_world::front::Joint::new(fixtures::SEED, fixtures::domain_limits());
    let task = fixtures::chat(&mut front, b"A chat to close");
    front.engine.send(engine::Event::Party(PartyInput::Ask {
        models: Box::new([]),
        reply_to: skein_lib::ReplyTo::new(CLOSE_REQUEST),
        sign_in: front.engine.session(),
        key: CLOSE_KEY,
        ask: jig_domain_people::Ask::Cancel {
            project: 1,
            task,
            reason: b"Chat closed".as_slice().into(),
            grace: Some(CLOSE_GRACE),
        },
    }));
    front.engine.settle();
    let _events = front.poll(AFTER_GRACE, skein_lib::Wall::from_nanos(AFTER_GRACE.as_nanos()));
    let _events = front.poll(AFTER_CANCEL_ANSWER, skein_lib::Wall::from_nanos(AFTER_CANCEL_ANSWER.as_nanos()));
    assert!(
        front.engine.task(task).is_none(),
        "grace timer ends live work: task={:?}; deadline={:?}; host={:?}; deliveries={:?}",
        front.engine.task(task),
        front.engine.root.next_deadline(&front.engine.env),
        front.engine.host_requests,
        front.engine.delivered
    );
}

const PROPOSE_CALL: u64 = 1108;
const PROPOSE_COMPLETION: u32 = 1;
const PROPOSE_POSITION: u32 = 1;
const PROPOSE_WORKER: skein_lib::Token = skein_lib::Token::new(7);
const PROPOSE_BUDGET: u64 = 150;
const GOAL_PRIORITY: u32 = 1;
const BEFORE_INBOX_DECISION: skein_lib::Duration = skein_lib::Duration::from_millis(100);
const AFTER_INBOX_DECISION: skein_lib::Duration = skein_lib::Duration::from_millis(500);
fn proposed_goal(front: &mut temper_client_world::front::Joint) -> u64 {
    use temper_engine_domain as engine;
    *front = temper_client_world::front::Joint::with_goal_proposals(fixtures::SEED, fixtures::domain_limits());
    let task = fixtures::chat(front, b"Propose a goal");
    let run = front.engine.last_assignment();
    let mut goal = temper_engine_domain_world::fixtures::report_delegate(b"A lasting goal", Box::new([]));
    goal.authority.budget.spend = PROPOSE_BUDGET;
    goal.tracked = Some(GOAL_PRIORITY);
    front.engine.call(
        PROPOSE_WORKER,
        task,
        run.attempt,
        PROPOSE_CALL,
        engine::Call {
            completion: PROPOSE_COMPLETION,
            position: PROPOSE_POSITION,
            tool: engine::Tool::Propose {
                action: engine::ProposedAction::Batch(Box::new([goal])),
                reason: b"A lasting goal".as_slice().into(),
                as_holder: true,
            },
        },
    );
    front.engine.settle();
    assert!(
        matches!(front.engine.answered(PROPOSE_CALL), Some(engine::CallAnswer::Proposed { .. })),
        "run proposed a goal: {:?}",
        front.engine.answered(PROPOSE_CALL)
    );
    task
}
#[test]
fn the_goal_proposal_fixture_waits_for_the_chats_person() {
    let mut front = temper_client_world::front::Joint::new(fixtures::SEED, fixtures::domain_limits());
    let task = proposed_goal(&mut front);
    assert!(
        matches!(&front.engine.task(task).expect("chat").proposal,
        Some(proposal) if matches!(proposal.state, jig_domain_tasks::ProposalState::Pending {
            holder: jig_domain_tasks::ProposalHolder::Person(fixtures::PARTY), ..
        })),
        "pending: {:?}",
        front.engine.task(task).expect("chat").proposal
    );
}
#[test]
fn a_goal_proposed_in_a_chat_accepted_in_the_inbox_leaves_the_chat() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = proposed_goal(world.front());
    let chat = world.client(fixtures::PARTY, fixtures::chat_page(task));
    let inbox = world.client(fixtures::PARTY, fixtures::inbox());
    world.person(
        chat,
        vec![
            Step::See { find: Find::named(Role::Button, b"Accept"), within: fixtures::WITHIN },
            Step::Pause { span: AFTER_INBOX_DECISION },
        ],
    );
    world.person(
        inbox,
        vec![
            Step::Pause { span: BEFORE_INBOX_DECISION },
            Step::See { find: Find::named(Role::Button, b"Accept"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Accept") },
            Step::See { find: Find::named(Role::Dialog, b"Accept"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Accepted"), within: fixtures::WITHIN },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    assert!(
        world.root(chat).person().pages().chat().expect("chat").cards.is_empty(),
        "decided card left the live chat"
    );
    assert!(
        world.front().engine.store.rows.values().any(|record| matches!(record,
            temper_engine_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row)))
            if row.requester == jig_domain_tasks::Party::Person(fixtures::PARTY)
                && row.tracked == Some(GOAL_PRIORITY) && row.spec.words.as_ref() == b"A lasting goal"
        )),
        "engine holds the accepted goal"
    );
}

const INBOX_CLIENT: jig_client_world::harness::ClientId = jig_client_world::harness::ClientId(1);
#[test]
fn the_proposal_fixture_admits_both_chat_and_inbox_clients() {
    use jig_client_world::harness::Front;
    let mut front = temper_client_world::front::Joint::new(fixtures::SEED, fixtures::domain_limits());
    proposed_goal(&mut front);
    for client in [DIAGNOSTIC_CLIENT, INBOX_CLIENT] {
        front.bind(client, fixtures::PARTY);
        front.poll(DIAGNOSTIC_NOW, DIAGNOSTIC_WALL);
    }
}

type Probe = (temper_client_world::app::Root, skein_lib::Queue<temper_client_domain::Request>);
const PROBE_ROUNDS: usize = 24;
fn probe(seed: u64, address: temper_client_domain::Address) -> Probe {
    use jig_client_world::harness::App;
    use temper_client_world::app::Temper;
    let limits = fixtures::domain_limits();
    let (mut root, _) = Temper::new(seed);
    let mut out = skein_lib::Queue::uncapped();
    Temper::step(
        &mut root,
        &skein_lib::Env { now: DIAGNOSTIC_NOW, wall: DIAGNOSTIC_WALL, limits },
        Temper::start(address, Vec::new(), START_OFFSET),
        &mut out,
    );
    (root, out)
}
fn pump(front: &mut temper_client_world::front::Joint, probes: &mut [Probe]) {
    use jig_client_world::harness::{App, ClientId, Duty, Front};
    use temper_client_world::app::Temper;
    let env = skein_lib::Env { now: DIAGNOSTIC_NOW, wall: DIAGNOSTIC_WALL, limits: fixtures::domain_limits() };
    for _ in 0..PROBE_ROUNDS {
        for (index, (root, out)) in probes.iter_mut().enumerate() {
            Temper::take(root, &env, out);
            while let Some(request) = out.pop() {
                if let Duty::Wire(request) = Temper::duty(request) {
                    front.wire(ClientId(index), env.now, env.wall, request);
                }
            }
        }
        for (who, event) in front.poll(env.now, env.wall) {
            front.received(who, &event);
            let (root, out) = &mut probes[who.0];
            Temper::step(root, &env, event, out);
        }
    }
}
#[test]
fn accepting_the_goal_with_chat_and_inbox_watches_settles_the_engine() {
    use jig_client_world::harness::{App, Front};
    use temper_client_domain::{Action, Event, pages::Act};
    let mut front = temper_client_world::front::Joint::new(fixtures::SEED, fixtures::domain_limits());
    let task = proposed_goal(&mut front);
    let mut probes =
        vec![probe(fixtures::SEED, fixtures::chat_page(task)), probe(fixtures::SEED + 1, fixtures::inbox())]
            .into_boxed_slice();
    for client in [DIAGNOSTIC_CLIENT, INBOX_CLIENT] {
        front.bind(client, fixtures::PARTY);
    }
    pump(&mut front, &mut probes);
    let object = probes[1]
        .0
        .person()
        .pages()
        .inbox()
        .expect("inbox")
        .cards
        .as_slice()
        .iter()
        .find(|card| matches!(card.object, jig_client::contract::ObjectKey::Proposal(_)))
        .expect("proposal")
        .object;
    let env = skein_lib::Env { now: DIAGNOSTIC_NOW, wall: DIAGNOSTIC_WALL, limits: fixtures::domain_limits() };
    for act in [Act::Decide { object, revision: None, act: jig_client::contract::ActKind::Accept }, Act::Confirm] {
        temper_client_world::app::Temper::step(
            &mut probes[1].0,
            &env,
            Event::Act { action: Action::Pages(act) }.into(),
            &mut probes[1].1,
        );
        pump(&mut front, &mut probes);
    }
    assert!(probes[0].0.person().pages().chat().expect("chat").cards.is_empty());
}

#[test]
fn opening_a_chat_clears_its_reply_from_the_engines_inbox() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::replied_chat(world.front(), b"Hello");
    let person = world.client(fixtures::PARTY, fixtures::inbox());
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Heading { level: 3 }, b"replied"), within: fixtures::WITHIN },
            Step::Go { address: fixtures::chat_page(task) },
            Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN },
            Step::Press { find: Find::exactly_named(Role::Link, b"Inbox") },
            Step::See { find: Find::named(Role::Heading { level: 1 }, b"Inbox"), within: fixtures::WITHIN },
            Step::Gone { find: Find::named(Role::Heading { level: 3 }, b"replied"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}
