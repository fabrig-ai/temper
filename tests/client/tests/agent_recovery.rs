use jig_client_world::harness::FrontFault;
use skein_lib::Duration;
use temper_client_world::{app::Temper, fixtures, front::Joint};

const CHAT: &[&[u8]] = &[b"chat", b"--project", b"1", b"--key", b"hello", b"Hello"];
const AGAIN: &[&[u8]] = &[b"chat", b"--project", b"1", b"--key", b"second", b"Again"];
const ADOPT: &[&[u8]] =
    &[b"adopt", b"--project", b"1", b"--owner", b"acme", b"--name", b"shop", b"--landing", b"main", b"--key", b"shop"];
// A restart is recovered inside the world's one-second story bound.
const OFFLINE: Duration = Duration::from_millis(30);
// Exactly one application is admitted even if its answer is cut.
const ADOPTIONS: usize = 1;
type World = jig_client_world::harness::World<Temper, Joint>;
type Client = jig_client_world::harness::ClientId;
fn world() -> World {
    let mut world = fixtures::world(fixtures::SEED);
    // The proposal fixture leaves authority for a second chat in the same period.
    *world.front() = Joint::with_goal_proposals(fixtures::SEED, fixtures::domain_limits());
    world
}
fn cut_chat() -> (World, Client, u64) {
    let mut world = world();
    let agent = fixtures::agent(&mut world);
    fixtures::cut(&mut world, agent, CHAT);
    let task = world
        .front()
        .engine
        .store
        .rows
        .keys()
        .find_map(|key| match key {
            temper_engine_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Live(task))) => {
                Some(*task)
            }
            temper_engine_domain::Key::Core(_)
            | temper_engine_domain::Key::Forge(_)
            | temper_engine_domain::Key::Wires => None,
        })
        .expect("committed chat task");
    (world, agent, task)
}
fn call(world: &mut World, agent: Client, words: &[&[u8]]) -> temper_client_tools::jig::Printed {
    let number = fixtures::give(world, agent, words);
    fixtures::answered(world, agent, number)
}

#[test]
fn a_chat_cut_by_a_lost_answer_is_resent_with_its_kept_line() {
    let (mut world, agent, task) = cut_chat();
    world.reload(agent);
    let printed = call(&mut world, agent, AGAIN);
    let expected = format!("kept: chat, done: task {task}, chat\n");
    assert_eq!(printed.status, 0);
    assert_eq!(world.root(agent).agent().kept_lines(), [expected.into_bytes().into_boxed_slice()]);
    assert_eq!(fixtures::chats(&world.front().engine), 2);
}

#[test]
fn a_chat_cut_by_a_lost_answer_asked_again_under_its_name_is_made_once() {
    let (mut world, agent, task) = cut_chat();
    world.reload(agent);
    let printed = call(&mut world, agent, CHAT);
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, format!("done: task {task}, chat\n").as_bytes()));
    assert_eq!(fixtures::chats(&world.front().engine), 1);
}

#[test]
fn a_chat_cut_by_an_engine_restart_asked_again_from_another_directory_is_made_once() {
    let (mut world, original_agent, task) = cut_chat();
    world.fault(FrontFault::Restart { offline: OFFLINE });
    world.reload(original_agent);
    let agent = fixtures::agent(&mut world);
    let printed = call(&mut world, agent, CHAT);
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, format!("done: task {task}, chat\n").as_bytes()));
    assert_eq!(fixtures::chats(&world.front().engine), 1);
}

#[test]
fn an_adoption_cut_by_a_lost_answer_asked_again_under_its_name_is_made_once() {
    let mut world = world();
    let agent = fixtures::agent(&mut world);
    fixtures::cut(&mut world, agent, ADOPT);
    world.reload(agent);
    let printed = call(&mut world, agent, ADOPT);
    assert_eq!(printed.status, 0);
    assert_eq!(world.front().applications_admitted(), ADOPTIONS);
}

#[test]
fn an_unkeyed_stop_cut_before_its_answer_is_resent_with_its_kept_line() {
    let mut world = world();
    let agent = fixtures::agent(&mut world);
    let task = fixtures::done_task(&call(&mut world, agent, CHAT));
    fixtures::cut(&mut world, agent, &[b"stop", task.to_string().as_bytes()]);
    world.reload(agent);
    let printed = call(&mut world, agent, AGAIN);
    let expected = format!("kept: stop, done: task {task}, stop\n");
    assert_eq!(printed.status, 0);
    assert_eq!(world.root(agent).agent().kept_lines(), [expected.into_bytes().into_boxed_slice()]);
}
