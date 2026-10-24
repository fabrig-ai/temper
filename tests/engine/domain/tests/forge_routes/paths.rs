//! The forge's deepest path fits both shipped document profiles.
use super::*;
use jig_domain_core::{
    acts::{Act, Admission},
    documents, reads,
};

const READ: Token = Token::new(901);
const SIGN_IN_READER: Token = Token::new(902);
const SET_READERS: Token = Token::new(903);
const SET_READERS_KEY: [u8; 16] = [93; 16];
const OWNER: u64 = 1;
const READER_SUBJECT: u64 = 8;
// crates/temper/tests/support/configuration.rs: the process tier's short lists.
const PROCESS_LINES: u32 = 2;
const LINES: &[(&str, u32)] = &[("jig's small profile", 4), ("the process tier's override", PROCESS_LINES)];

fn sign_in_reader(world: &mut World) {
    let owner = world.signed_in.take().expect("owner session");
    world.send(engine::Event::Party(engine::PartyInput::SignedIn {
        held: None,
        reply_to: ReplyTo::new(SIGN_IN_READER),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: READER_SUBJECT.to_be_bytes().into() },
            login: Box::from(&b"reader"[..]),
            name: Box::from(&b"Reader"[..]),
        },
    }));
    world.until(Until::SignedIn);
    let sign_in = world.signed_in.expect("reader session");
    let Some(Record::Core(jig_domain_core::Record::People(people::Stored::SignIn { person, .. }))) =
        world.store.rows.get(&Key::Core(jig_domain_core::Key::People(people::Key::SignIn(sign_in))))
    else {
        panic!("the reader's durable sign-in");
    };
    let holdings = Box::new([
        people::Holding { person: OWNER, role: people::Role::Owner },
        people::Holding { person: *person, role: people::Role::Observer },
    ]);
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(SET_READERS),
        sign_in: owner,
        key: SET_READERS_KEY,
        ask: people::Ask::SetRoles { project: 1, holdings: holdings.clone() },
    }));
    world.until(Until::Ready);
    assert_eq!(
        world.store.rows.get(&Key::Core(jig_domain_core::Key::People(people::Key::Roles(1)))),
        Some(&Record::Core(jig_domain_core::Record::People(people::Stored::Roles { project: 1, holdings }))),
        "the reader is a party of the producer's project"
    );
}

fn change_world_with_lines(lines: u32) -> (World, engine::Assignment, engine::Assignment, Box<[u8]>) {
    let mut documents = jig_protocol_config::profile(jig_protocol_config::Profile::Small).core.documents;
    documents.lines = lines;
    let world = World::configured_policy(true, true, true, 1000, Gate::default(), Some(documents));
    change_world_in(world, Change::default())
}

fn read(world: &mut World, read: reads::Read) -> Option<reads::Answer> {
    world.reads.clear();
    world.send(engine::Event::Party(engine::PartyInput::Read {
        reply_to: ReplyTo::new(READ),
        sign_in: world.signed_in.expect("reader session"),
        read,
    }));
    for _ in 0..40 {
        if let Some(index) = world.reads.iter().position(|(reply, _)| *reply == ReplyTo::new(READ)) {
            return Some(world.reads.remove(index).1);
        }
        world.tick();
    }
    None
}

fn task_line(world: &mut World, project: u32, task: u64) -> Option<documents::TaskLine> {
    let query = documents::TaskQuery { project: Some(project), requester: None, ..jig_core_world::fixtures::OWN_TASKS };
    let Some(reads::Answer::Tasks(page)) = read(world, reads::Read::Tasks { query, live: None, ended: None }) else {
        return None;
    };
    page.live.iter().find(|line| line.chip.number == task).cloned()
}

#[test]
fn a_change_on_the_forges_deepest_branch_is_listed_in_a_task_query() {
    for &(source, lines) in LINES {
        let (mut world, _chat, producer, _branch) = change_world_with_lines(lines);
        let row = task_line(&mut world, 1, producer.task);
        assert_eq!(row.map(|line| line.chip.number), Some(producer.task), "{source}: listed, not refused as too large");
    }
}

#[test]
fn a_change_on_the_forges_deepest_branch_is_served_in_its_task_document() {
    for &(source, lines) in LINES {
        let (mut world, _chat, producer, _branch) = change_world_with_lines(lines);
        let answer = read(&mut world, reads::Read::Task { task: producer.task });
        let Some(reads::Answer::Document(document)) = answer else { panic!("{source}: owner task: {answer:?}") };
        let documents::Document::Task(task) = *document else { panic!("{source}: owner task document") };
        let offer = task.acts.iter().find(|offer| offer.act == Act::Amend).expect("Amend offer");
        assert!(matches!(offer.admission, Admission::Bounded { .. }), "{source}: bounded Amend offer");
        sign_in_reader(&mut world);
        let answer = read(&mut world, reads::Read::Task { task: producer.task });
        let Some(reads::Answer::Document(document)) = answer else { panic!("{source}: task document: {answer:?}") };
        let documents::Document::Task(task) = *document else { panic!("{source}: task document") };
        assert_eq!(task.chip.number, producer.task, "{source}: served, not refused as too large");
    }
}
