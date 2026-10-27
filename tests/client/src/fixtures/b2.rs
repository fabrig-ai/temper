//! Roadmap B2 (wave 2), retaining the first-edition fixture (domain/testing.md, 5.3).
//! B2 maps to draft/reference-case.md, 4.1 and 4.2 (domain/testing.md, 2.7).
use super::{
    DELEGATE_COMPLETION, Joint, QUESTION_WORKER, SEED, delegate_turn, domain_limits, engine, make_goal_with_delegation,
    report_task, tasks,
};
use temper_engine_domain_world::fixtures::report_delegate;

pub const ADVISOR_BRIEF: &[u8] = b"Survey the export's crates";
pub const ADVISOR_REPORT: &[u8] = b"Two crates hold the export";
pub const FIRST_BRIEF: &[u8] = b"Implement the export";
pub const FIRST_REPORT: &[u8] = b"The export is implemented";
pub const SECOND_BRIEF: &[u8] = b"Write the export's story";
pub const SECOND_REPORT: &[u8] = b"The story passes";
pub const GOAL_REPORT: &[u8] = b"The export's first milestone landed";
const GOAL_PROGRESS: &[u8] = b"Plan the export's first milestone";
// B2: one survey followed by two implementation tasks.
const DELEGATES: u32 = 3;
const ADVISOR_CALL: u64 = 1301;
const FIRST_CALL: u64 = 1302;
const SECOND_CALL: u64 = 1303;
const FIRST_POSITION: u32 = 1;
const SECOND_POSITION: u32 = 2;

#[derive(Clone, Copy)]
pub struct Advised {
    pub goal: u64,
    pub advisor: u64,
}
pub struct Planned {
    pub goal: u64,
    pub advisor: u64,
    pub first: u64,
    pub second: u64,
}

/// B2's survey and report to its requester: a tracked goal
/// with authority for three delegates delegates one report task, which runs a
/// turn and reports. The goal's next run is out, with no call yet.
pub fn advisor_reported(front: &mut Joint) -> Advised {
    *front = Joint::with_delegates(SEED, &domain_limits(), DELEGATES);
    let goal = make_goal_with_delegation(front, false, Some(DELEGATES));
    delegate_turn(front, goal, None, GOAL_PROGRESS);
    let advisor = delegate(front, goal, ADVISOR_CALL, FIRST_POSITION, ADVISOR_BRIEF, Box::new([]));
    park(front, goal);
    delegate_turn(front, advisor, None, ADVISOR_BRIEF);
    report_task(front, advisor, ADVISOR_REPORT);
    Advised { goal, advisor }
}

/// B2's tasks with a dependency: in that run the goal delegates
/// first, then, in a later call, second depending on it (`Dependency::Existing`),
/// and parks. First runs a turn; second waits for it.
pub fn tasks_declared(front: &mut Joint, advised: Advised) -> Planned {
    goal_turn(front, advised.goal);
    let first = delegate(front, advised.goal, FIRST_CALL, FIRST_POSITION, FIRST_BRIEF, Box::new([]));
    let second = delegate(
        front,
        advised.goal,
        SECOND_CALL,
        SECOND_POSITION,
        SECOND_BRIEF,
        Box::new([engine::Dependency::Existing(first)]),
    );
    park(front, advised.goal);
    delegate_turn(front, first, None, FIRST_BRIEF);
    Planned { goal: advised.goal, advisor: advised.advisor, first, second }
}

/// B2's dependency is met: first reports; second is assigned
/// and runs a turn on the same joint world.
pub fn first_reported(front: &mut Joint, planned: &Planned) {
    report_task(front, planned.first, FIRST_REPORT);
    delegate_turn(front, planned.second, None, SECOND_BRIEF);
}

/// B2's end: second reports, the goal's next run finishes with
/// its report (returned), and the engine settles the goal's spend.
pub fn goal_reported(front: &mut Joint, planned: &Planned) -> Box<[u8]> {
    report_task(front, planned.second, SECOND_REPORT);
    goal_turn(front, planned.goal);
    report_task(front, planned.goal, GOAL_REPORT);
    GOAL_REPORT.into()
}

fn delegate(
    front: &mut Joint,
    goal: u64,
    call: u64,
    position: u32,
    brief: &[u8],
    dependencies: Box<[engine::Dependency]>,
) -> u64 {
    let driver = &mut front.engine;
    let attempt = driver.task(goal).expect("goal's run").attempt;
    driver.call(
        QUESTION_WORKER,
        goal,
        attempt,
        call,
        engine::Call {
            completion: DELEGATE_COMPLETION,
            position,
            tool: engine::Tool::Delegate { batch: Box::new([report_delegate(brief, dependencies)]) },
        },
    );
    driver.settle();
    let Some(engine::CallAnswer::Delegated(children)) = driver.answered(call) else {
        panic!("B2 delegate admitted: {:?}", driver.answered(call));
    };
    children[0].task
}
fn park(front: &mut Joint, goal: u64) {
    let driver = &mut front.engine;
    let run = driver.task(goal).expect("goal's run");
    driver.answer(QUESTION_WORKER, goal, run.attempt, run.run_spent, tasks::End::Parked, engine::Parts::none());
    driver.settle();
}
fn goal_turn(front: &mut Joint, goal: u64) {
    let read = front.engine.task(goal).expect("goal's run").inbox.last().map(|message| message.number);
    delegate_turn(front, goal, read, GOAL_PROGRESS);
}
