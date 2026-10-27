use jig_client_world::harness::{ClientId, World};
use temper_client_world::{app::Temper, fixtures, front::Joint};

fn large_chat() -> (World<Temper, Joint>, u64, (u64, u32)) {
    let mut world = fixtures::world_with_large_turns(fixtures::SEED);
    let task = fixtures::chat(world.front(), b"Hello");
    let turn = fixtures::large_turn(world.front(), task);
    (world, task, turn)
}
fn agent_chat() -> (World<Temper, Joint>, ClientId, u64, (u64, u32)) {
    let (mut world, task, turn) = large_chat();
    let agent = fixtures::agent(&mut world);
    (world, agent, task, turn)
}

#[test]
fn a_large_turns_rows_overlap_the_page_bound() {
    let (mut world, task, _) = large_chat();
    let (single, activity) = fixtures::turn_row_costs(world.front(), task);
    let bound = world.front().engine.env.limits.loads.read_bytes;
    assert!(single <= bound, "{single}");
    assert!(bound < activity, "{activity}");
}

#[test]
fn a_large_turn_in_a_chat_prints_as_its_size_line() {
    let (mut world, agent, task, (attempt, number)) = agent_chat();
    let call = fixtures::give(&mut world, agent, &[b"chat", b"show", task.to_string().as_bytes()]);
    let printed = fixtures::answered(&mut world, agent, call);
    assert_eq!(printed.status, 0, "{printed:?}");
    let stdout = std::str::from_utf8(&printed.stdout).expect("chat UTF-8");
    assert!(stdout.contains(&format!("read with: task turn {task} {attempt}.{number}\n")), "{stdout}");
}

// The 24 KiB text crosses PART=16 KiB, needing exactly two printed reads.
const TURN_READS: u32 = 2;
#[test]
fn a_large_turn_is_read_whole_by_its_parts() {
    let (mut world, agent, task, (attempt, number)) = agent_chat();
    let turn = format!("{attempt}.{number}");
    let (text, reads) =
        fixtures::read_whole(&mut world, agent, &[b"task", b"turn", task.to_string().as_bytes(), turn.as_bytes()]);
    assert_eq!(reads, TURN_READS);
    // session_turn sequence 1 emits b' ' + 1, one byte for each text position.
    assert_eq!(text, vec![b'!'; fixtures::LARGE_TURN_TEXT]);
}

#[test]
fn a_turn_past_64_kib_commits_and_is_read_whole_with_the_configured_allowance() {
    let mut world = fixtures::world_with_long_turns(fixtures::SEED);
    let task = fixtures::chat(world.front(), b"Hello");
    let (attempt, number) = fixtures::large_turn_with_opaque(world.front(), task, 65_536);
    let (single, _) = fixtures::turn_row_costs(world.front(), task);
    let bound = world.front().engine.env.limits.loads.read_bytes;
    assert!(single > 65_536, "the stored read crosses the old cap");
    assert!(single <= bound, "the configured read holds the complete row");
    let agent = fixtures::agent(&mut world);
    let (text, reads) = fixtures::read_whole(
        &mut world,
        agent,
        &[b"task", b"turn", task.to_string().as_bytes(), format!("{attempt}.{number}").as_bytes()],
    );
    assert_eq!(reads, TURN_READS);
    assert_eq!(text, vec![b'!'; fixtures::LARGE_TURN_TEXT]);
}
