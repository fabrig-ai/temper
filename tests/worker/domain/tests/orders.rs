//! The worker keeps these orders whatever its queues hold
//! (programming-model.md, 7.2): a capability's requests are routed before
//! the hub asks a capability anything more (W1), and the payload
//! hand-outs come back in the order they went out, an agent's end
//! following its last one (W2).

use skein_lib::{Duration, Time};
use temper_worker_domain::{Request, agent, host};
use temper_worker_domain_world::{protocol, runtime::orders as runtime, whole::orders as whole};

// One byte is enough to distinguish the turn bodies while keeping the
// pressure in outstanding entries rather than payload size.
const TURN_TEXT_BYTES: usize = 1;

/// The outer cleanup order a peer sees, with three prepares' terminals
/// held and released in waves. W1's inner capability/hub priority is checked
/// by the domain unit test `a_capabilitys_callback_is_routed_before_the_hubs_next_request`.
#[test]
fn a_failed_prepare_releases_its_hold_before_the_hub_answers() {
    let report = whole::failed_prepares();
    assert_eq!(report.peak_held, usize::try_from(whole::RUNS).expect("three runs"));
    let mut released = Vec::new();
    let mut answered = Vec::new();
    for crossing in report.crossed {
        match crossing {
            whole::Crossing::Released { workspace } => {
                assert!(!released.contains(&workspace), "each workspace releases once");
                released.push(workspace);
            }
            whole::Crossing::Io { workspace } => {
                assert!(!released.contains(&workspace), "no operation names a released workspace");
            }
            whole::Crossing::Answer { run, idle } => {
                answered.push(run);
                assert_eq!(released.len(), answered.len(), "release precedes each answer");
                assert_eq!(
                    idle,
                    u32::try_from(answered.len()).expect("bounded answers"),
                    "release precedes the answer crossing"
                );
            }
        }
    }
    assert_eq!(answered.len(), usize::try_from(whole::RUNS).expect("three runs"));
}

/// Both agents have two turns, a withdrawn call's name and a result out.
fn hold_several_hand_outs() -> runtime::Driver {
    let mut driver = runtime::Driver::new();
    for number in 1..=runtime::TURNS {
        for index in 0..runtime::RUNS.len() {
            driver.say(index, agent::Up::Turn { turn: protocol::turn(number, u64::from(number), TURN_TEXT_BYTES) });
        }
    }
    for index in 0..runtime::RUNS.len() {
        driver.say(
            index,
            agent::Up::Call {
                call: runtime::CALL,
                name: agent::CallName { activation: runtime::ATTEMPT.raw(), completion: runtime::TURNS, position: 0 },
                deadline: Time::ZERO.saturating_add(Duration::from_secs(60)),
                ask: agent::Ask::Host {
                    tool: Box::from(&b"recall"[..]),
                    effect: agent::Effect::Read,
                    body: Box::from(&b"{}"[..]),
                },
            },
        );
        driver.say(index, agent::Up::Withdraw { call: runtime::CALL });
        driver.say(
            index,
            agent::Up::Answer {
                answer: agent::Answer {
                    read: None,
                    turns: runtime::TURNS,
                    spent: u64::from(runtime::TURNS),
                    result: protocol::report(b"done"),
                },
            },
        );
    }
    driver
}

#[test]
fn hand_outs_come_back_in_the_order_they_went_out() {
    let mut driver = hold_several_hand_outs();
    assert_eq!(driver.owed.len(), runtime::HAND_OUTS, "both agents' payloads remain out together");
    while !driver.owed.is_empty() {
        driver.release_one();
    }
    assert_eq!(driver.returned, driver.handed, "global FIFO also preserves each agent's FIFO");
    for run in runtime::RUNS {
        let turns: Vec<_> = driver
            .requests
            .iter()
            .filter_map(|request| {
                if let Request::Link(host::LinkUp::Turn { run: from, turn, .. }) = request {
                    (*from == run).then_some(turn.turn)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(turns, [1, 2], "each agent's turns reach the engine in sequence");
        let call_order: Vec<_> = driver
            .requests
            .iter()
            .filter_map(|request| {
                if let Request::Link(host::LinkUp::Call { run: from, .. }) = request {
                    (*from == run).then_some("Called")
                } else if let Request::CancelRelay { run: from, .. } = request {
                    (*from == run).then_some("Withdrawn")
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(call_order, ["Called", "Withdrawn"]);
    }
}

#[test]
fn an_agents_end_waits_for_its_last_hand_out() {
    let mut driver = hold_several_hand_outs();
    for index in 0..runtime::RUNS.len() {
        driver.gone(index);
    }
    assert_eq!(driver.owed.len(), runtime::HAND_OUTS);
    assert_eq!(answers(&driver), [], "neither end overtakes its outstanding hand-outs");
    while !driver.owed.is_empty() {
        driver.release_one();
        let expected: Vec<_> = driver
            .owners
            .iter()
            .zip(runtime::RUNS)
            .filter_map(|(owner, run)| {
                let pending = driver.owed.iter().any(|event| {
                    matches!(event,
                temper_worker_domain::Event::Encoded { owner: from, .. } if from == owner)
                });
                (!pending).then_some(run)
            })
            .collect();
        assert_eq!(answers(&driver), expected, "an end follows its own last hand-out, exactly once");
    }
    assert_eq!(answers(&driver), runtime::RUNS, "each end answers exactly once after its last return");
}

fn answers(driver: &runtime::Driver) -> Vec<skein_lib::Token> {
    driver
        .requests
        .iter()
        .filter_map(|request| if let Request::Answer { run, .. } = request { Some(*run) } else { None })
        .collect()
}
