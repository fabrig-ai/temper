//! The person's acts, with documents identified by the moment they were read.
use super::{A, DESIGN, DIRECTOR, PERSON, PROJECT};
use crate::{
    argv,
    calls::{self, D1, call, join, number, sign_in, start},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Read {
    BEndedEarlier,
    ImplementerRunning,
    H1Running,
    HelperEnded,
    BHeld,
    BStillHeld,
    ImplementerEnded,
    H1Ended,
    OwnerEnded,
    PauseEnded,
    DirectorEnded,
    AEnded,
    DesignEnded,
    AWaiting,
    Refused,
    Limited,
    PaceAsked,
    Ranked,
    SurveyEnded,
    CheckEnded,
    DirectorActivity,
    DirectorTimer,
    BenchmarkWaiting,
    BenchmarkEnded,
    BEnded,
    /// The retried director reports.
    HangEnded,
    /// The director commits its question read before stopping.
    HangQuestionRead,
    /// The director waits in its first attempt.
    HangWaiting,
    /// A's next phase reports.
    PhaseEnded,
    /// The newest director activity page.
    HangActivity,
    /// The middle director activity page.
    HangActivityOlder,
    /// The oldest director activity page.
    HangActivityFirst,
    RulesWaiting,
    RulesHeld,
    RulesQuestionWaiting,
    ScopeEnded,
    RulesEnded,
    /// The director's whole activity once ended: its runs' endings.
    RulesActivity,
    /// The first fresh director brief after Release, read from the scripted model's received system text.
    RulesFreshBrief,
    /// A's whole activity once ended: its runs' endings.
    ScopeActivity,
    ShapeEnded,
    MoverEnded,
    PlanAnswerPending,
    PlannerParked,
    PlannerEnded,
    PlannerEarlier,
    PlannerFirst,
    PlanEnded,
    SubownerEnded,
    MisreadParked,
    MisreadHelperRunning,
    MisreadRulePending,
    MisreadFresh,
    MisreadHelperEnded,
    MisreadEnded,
    X3Open,
    X3Produced,
    X3Ended,
    PlannedRunning,
    PlannedEnded,
    StoryLanded,
    DirectionBHeld,
    DirectionBPending,
    DirectionBEnded,
    DirectionBFirst,
    StoryOwnerEnded,
    DirectionEnded,
    StoryOwnerEarlier,
    StoryOwnerFirst,
    DirectionNoteWritten,
    DirectionFirst,
    BatchOneWriting,
    BatchTwoWriting,
    BatchThreeWriting,
    BatchStoryWaiting,
    BatchStoryAfterOne,
    BatchOneBeforeStory,
    BatchTwoBeforeStory,
    BatchStoryWriting,
    BatchThreeStillWriting,
    BatchOneEnded,
    BatchTwoEnded,
    BatchThreeEnded,
    BatchStoryEnded,
    BatchSubownerEnded,
    BatchOwnerEnded,
    BatchDirectorEnded,
    BlockerKept,
    BlockerPending,
    BlockerReviewWriting,
    BlockerA3Ended,
    BlockerE1Ended,
    BlockerCoderEnded,
    BlockerReviewerEnded,
    BlockerSubownerEnded,
    BlockerOwnerEnded,
    BlockerDirectorEnded,
    // Act 3, step 6.5 (domain/testing.md, 5.3)
    BudgetAsked,
    BudgetInboxAnswered,
    BudgetMedium,
    BudgetHigh,
    BudgetA5Ended,
    BudgetAdvisorEnded,
    BudgetSubownerEnded,
    BudgetOwnerEnded,
    BudgetDirectorEnded,
    BudgetSubownerFirst,
    BudgetOwnerFirst,
    BudgetDirectorFirst,
    // 6.6
    WaitingE1Pending,
    WaitingRepinPending,
    WaitingReviewWriting,
    WaitingDirectorEnded,
    WaitingOwnerEnded,
    WaitingE1Ended,
    WaitingRepinEnded,
    WaitingE1CoderEnded,
    WaitingE1ReviewerEnded,
    WaitingRepinCoderEnded,
    WaitingRepinReviewerEnded,
    WaitingRootsEnded,
    // 6.4
    RedReviewWriting,
    RedChangeOpen,
    RedRepairWriting,
    RedRepairPending,
    RedTimerArmed,
    RedChangePaused,
    RedDirectorEnded,
    RedOwnerEnded,
    RedChangeEnded,
    RedCoderEnded,
    RedReviewEnded,
    RedRepairEnded,
    RedRepairCoderEnded,
    RedRepairReviewEnded,
    RedRootsEnded,
    RedRootsPending,
    RedUpdatedReviewEnded,
    // Act 3, step 6.7 (domain/testing.md, 5.3)
    CapHeldWait,
    CapHeld,
    CapA5Writing,
    CapOwnerWaiting,
    CapInboxHeld,
    CapInboxHeldAfter,
    CapPassed,
    CapReleased,
    CapA5Ended,
    CapSubownerEnded,
    CapOwnerEnded,
    CapDirectorEnded,
    CapInboxEnded,
    CapInboxEndedAfter,
    CapSubownerFirst,
    CapOwnerFirst,
    CapDirectorFirst,
    QaSubownerBefore,
    QaNextWriting,
    QaSubownerLive,
    QaTesterLive,
    QaDirectorEnded,
    QaOwnerEnded,
    QaSubownerEnded,
    QaTesterEnded,
    QaNextEnded,
    QaChangeEnded,
    QaCoderEnded,
    QaReviewerEnded,
    QaInboxEnded,
    QaInboxAfter,
    QaRootsEnded,
}
// Provisioning's key call does not count as a story act. The director's
// ended wait starts first; its delegates' ended documents are read before
// their requesters end, then the director's wait is joined.
const DIRECTOR_ENDED: usize = 3;
const A_ENDED: usize = 7;
const DESIGN_ENDED: usize = 6;
pub const READS_4_1: &[(Read, usize)] =
    &[(Read::DirectorEnded, DIRECTOR_ENDED), (Read::AEnded, A_ENDED), (Read::DesignEnded, DESIGN_ENDED)];

#[must_use]
pub fn step_4_1() -> Vec<calls::Step> {
    vec![
        sign_in(D1),
        // Leave out the goal's issue: recovery of its edit holds a closing goal
        // (M1's finding; forge_route.rs project_goal projects only in a home).
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "adopt-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "schedule", PERSON]),
        // Keep these watches live: a fresh wait on an ended delegate exits
        // unknown. jig's crates/jig-domain-core/src/routing/views.rs:304 maps
        // documents/small.rs:23-28's missing/ended live row to that refusal.
        // 2071ea76 fixes historical reads only; it cannot admit these watches.
        start(D1, argv!["wait", "task", number(DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(A), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(DESIGN), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(A), "--until", "ended", "--json"]),
        join(DIRECTOR_ENDED),
    ]
}

// Refusal keeps its phase wait and fresh read; the limit retains its matching read.
const HELD_A_ENDED: usize = 3;
const A_WAITING: usize = 5;
const HELD_ACCOUNTS: usize = 6;
const REFUSED_DIRECTOR_ENDED: usize = 8;
const LIMITED_A_WAITING: usize = 4;
const LIMITED_ACCOUNTS: usize = 5;
const LIMITED_DIRECTOR_ENDED: usize = 6;
pub const READS_4_5_REFUSED: &[(Read, usize)] = &[
    (Read::DirectorEnded, REFUSED_DIRECTOR_ENDED),
    (Read::AWaiting, A_WAITING),
    (Read::Refused, HELD_ACCOUNTS),
    (Read::AEnded, HELD_A_ENDED),
];
pub const READS_4_5_LIMITED: &[(Read, usize)] = &[
    (Read::DirectorEnded, LIMITED_DIRECTOR_ENDED),
    (Read::AWaiting, LIMITED_A_WAITING),
    (Read::Limited, LIMITED_ACCOUNTS),
    (Read::AEnded, HELD_A_ENDED),
];

#[must_use]
pub fn step_4_5_refused() -> Vec<calls::Step> {
    use super::{FIRST_ACCOUNT, SECOND_KEY_FILE};
    held("refused", Some(call(D1, argv!["replace-key", number(FIRST_ACCOUNT), calls::file(SECOND_KEY_FILE)])))
}

#[must_use]
pub fn step_4_5_limited() -> Vec<calls::Step> {
    // Only reads and waits follow the chat: the engine lifts the limit.
    held("limited", None)
}

fn held(key: &'static str, act: Option<calls::Step>) -> Vec<calls::Step> {
    let mut steps = vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", key, super::HELD_PERSON]),
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "replied"]),
        // A fresh wait after the requester ends exits unknown: jig's
        // routing/views.rs admission and documents/small.rs require live rows.
        // Keep A's watch across the act until jig supports historical waits.
        start(D1, argv!["wait", "task", number(A), "--until", "ended", "--json"]),
    ];
    if act.is_none() {
        steps.push(calls::until_matching(
            D1,
            argv!["task", number(A), "--json"],
            calls::ReadCondition::LimitedTask { task: A, account: super::FIRST_ACCOUNT, source: 0 },
            b"\"phase\":\"ended\"",
        ));
    } else {
        steps.push(call(D1, argv!["wait", "task", number(A), "--until", "phase:due"]));
        steps.push(call(D1, argv!["task", number(A), "--json"]));
    }
    steps.push(call(D1, argv!["accounts", "--json"]));
    steps.extend(act);
    steps.push(join(HELD_A_ENDED));
    // The director can already be ended too: a fresh wait exits unknown.
    steps.push(calls::until(
        D1,
        argv!["task", number(DIRECTOR), "--json"],
        b"\"phase\":\"ended\"",
        b"\"phase\":\"running\"",
    ));
    steps
}

const PACE_DIRECTOR_ENDED: usize = 3;
const PACE_ASKED: usize = 6;
const RANKED: usize = 8;
const SURVEY_ENDED: usize = 10;
const PACE_A_ENDED: usize = 11;
pub const READS_4_4: &[(Read, usize)] = &[
    (Read::DirectorEnded, PACE_DIRECTOR_ENDED),
    (Read::PaceAsked, PACE_ASKED),
    (Read::Ranked, RANKED),
    (Read::SurveyEnded, SURVEY_ENDED),
    (Read::AEnded, PACE_A_ENDED),
];

#[must_use]
pub fn step_4_4() -> Vec<calls::Step> {
    use super::{PACE_ANSWER, PACE_PERSON, PACE_QUESTION_MESSAGE, SECOND_ACCOUNT, SECOND_KEY_FILE, SURVEY};
    vec![
        sign_in(D1),
        call(D1, argv!["add-key", "1", calls::file(SECOND_KEY_FILE), "--deployment", "--key", "second-llm"]),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "pace", PACE_PERSON]),
        start(D1, argv!["wait", "task", number(DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "inbox", "--until", "waiting"]),
        // A's question makes the director due. Wait for its acknowledgement
        // run to park before the person reads the question: it must outlive
        // that acknowledgement (jig's domain/tasks.md, 8.2). Polling chat
        // instead races its live run watch with that run's end and can retry.
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "phase:idle"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(D1, argv!["rank", number(SECOND_ACCOUNT), "1"]),
        call(D1, argv!["accounts", "--json"]),
        call(D1, argv!["answer", number(DIRECTOR), number(PACE_QUESTION_MESSAGE), PACE_ANSWER, "--key", "pace-answer"]),
        // The director reports after A, which reports after the survey. Its
        // existing live watch is the completion barrier; read their historical
        // documents once afterwards, without polling or opening ended watches.
        join(PACE_DIRECTOR_ENDED),
        call(D1, argv!["task", number(SURVEY), "--json"]),
        call(D1, argv!["task", number(A), "--json"]),
    ]
}

// Call positions within the signed-in check suffix; non-call steps consume none.
const CHECK_DIRECTOR_ENDED: usize = 1;
const CHECK_BENCHMARK_WAITING: usize = 4;
const CHECK_BENCHMARK_ENDED: usize = 5;
const CHECK_B_ENDED: usize = 6;
const CHECK_DIRECTOR_ACTIVITY: usize = 7;
const CHECK_DIRECTOR_TIMER: usize = 8;
pub const READS_4_3: &[(Read, usize)] = &[
    (Read::CheckEnded, CHECK_DIRECTOR_ENDED),
    (Read::BenchmarkWaiting, CHECK_BENCHMARK_WAITING),
    (Read::BenchmarkEnded, CHECK_BENCHMARK_ENDED),
    (Read::BEnded, CHECK_B_ENDED),
    (Read::DirectorActivity, CHECK_DIRECTOR_ACTIVITY),
    (Read::DirectorTimer, CHECK_DIRECTOR_TIMER),
];

/// The check follows the opener's sign-in; `base` is its first recorded call.
#[must_use]
pub fn step_4_3(base: usize) -> Vec<calls::Step> {
    use super::{B, BENCHMARK, CHECK, CHECK_PERSON};
    vec![
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "check", CHECK_PERSON]),
        start(D1, argv!["wait", "task", number(CHECK), "--until", "ended", "--json"]),
        calls::watching(base + CHECK_DIRECTOR_ENDED),
        // Each replied wait follows a completed delegation, admitting the
        // next task's watch without racing a not-yet-created task number.
        call(D1, argv!["wait", "task", number(CHECK), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(B), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(BENCHMARK), "--until", "phase:due", "--json"]),
        calls::add_worker("s2"),
        join(base + CHECK_DIRECTOR_ENDED),
        call(D1, argv!["task", number(BENCHMARK), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(B), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(CHECK), "--activity", "32", "--json"]),
        // The latest page starts after the timer: follow its supplied cursor.
        call(
            D1,
            argv![
                "activity",
                number(CHECK),
                "--before",
                calls::activity_before(base + CHECK_DIRECTOR_ACTIVITY),
                "--json"
            ],
        ),
    ]
}

/// Preserve every opener act, then run the check in the signed-in directory.
#[must_use]
pub fn step_4_1_4_3() -> Vec<calls::Step> {
    let mut steps = step_4_1();
    let base = READS_4_1.iter().map(|(_, call)| call).max().expect("opener calls") + 1;
    steps.extend(step_4_3(base));
    steps
}

#[must_use]
pub fn reads_4_1_4_3() -> Vec<(Read, usize)> {
    let base = READS_4_1.iter().map(|(_, call)| call).max().expect("opener calls") + 1;
    READS_4_1.iter().copied().chain(READS_4_3.iter().map(|&(read, call)| (read, base + call))).collect()
}

const HANG_ENDED: usize = 2;
const HANG_QUESTION_READ: usize = 3;
const HANG_WAITING: usize = 4;
const HANG_A_ENDED: usize = 5;
const HANG_ACTIVITY: usize = 6;
const HANG_ACTIVITY_OLDER: usize = 7;
const HANG_ACTIVITY_FIRST: usize = 8;
/// Documents within the standalone restart story, relative to its first call.
pub const READS_4_6: &[(Read, usize)] = &[
    (Read::HangEnded, HANG_ENDED),
    (Read::HangQuestionRead, HANG_QUESTION_READ),
    (Read::HangWaiting, HANG_WAITING),
    (Read::PhaseEnded, HANG_A_ENDED),
    (Read::HangActivity, HANG_ACTIVITY),
    (Read::HangActivityOlder, HANG_ACTIVITY_OLDER),
    (Read::HangActivityFirst, HANG_ACTIVITY_FIRST),
];

/// Sign in and run the restart story; `base` names its first recorded call.
#[must_use]
pub fn step_4_6(base: usize) -> Vec<calls::Step> {
    use super::{HANG, HANG_PERSON, HANG_READ, PHASE};
    vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "hang", HANG_PERSON]),
        start(D1, argv!["wait", "task", number(HANG), "--until", "ended", "--json"]),
        calls::watching(base + HANG_ENDED),
        // ctl's predicates distinguish phase/replied, but cannot select a
        // committed turn or its text. Activity contains only committed turns;
        // chat/run can also contain the live writing, so cannot be this barrier.
        calls::until(D1, argv!["activity", number(HANG), "--json"], HANG_READ.as_bytes(), b"run_began"),
        // Once the read is committed, await the following in-run wait.
        // Its withheld turn is sealed when the worker stops, as A's parked waits are.
        call(D1, argv!["wait", "task", number(HANG), "--until", "replied", "--json"]),
        calls::stop_worker("ws"),
        join(base + HANG_ENDED),
        call(D1, argv!["task", number(PHASE), "--activity", "32", "--json"]),
        call(D1, argv!["activity", number(HANG), "--json"]),
        call(D1, argv!["activity", number(HANG), "--before", calls::activity_before(base + HANG_ACTIVITY), "--json"]),
        call(
            D1,
            argv!["activity", number(HANG), "--before", calls::activity_before(base + HANG_ACTIVITY_OLDER), "--json"],
        ),
        // The ended chat is the cheapest event proving its retry cannot use ws.
        calls::start_worker("ws"),
    ]
}

// Act 2, step 5.1 (domain/testing.md, 5.3)
const RULES_ENDED: usize = 3;
pub const READS_5_1: &[(Read, usize)] = &[
    (Read::RulesWaiting, 5),
    (Read::RulesHeld, 7),
    (Read::RulesQuestionWaiting, 8),
    (Read::RulesEnded, RULES_ENDED),
    (Read::ScopeEnded, 11),
    (Read::RulesActivity, 12),
    (Read::ScopeActivity, 13),
];

#[must_use]
pub fn step_5_1() -> Vec<calls::Step> {
    use super::{
        NEW_RULES, RULES, RULES_FILE, RULES_HOLD_REVISION, RULES_PERSON, SCOPE, SCOPE_QUESTION, SCOPE_QUESTION_HOLD,
    };
    vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--role",
                "context",
                "--no-home",
                "--key",
                "rules-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "rules", RULES_PERSON]),
        // Keep the ended watch across Stop and Release instead of polling the
        // full task document through the director's answer and both reports.
        start(D1, argv!["wait", "task", number(RULES), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(RULES), "--until", "replied"]),
        calls::until(D1, argv!["task", number(RULES), "--json"], b"\"work\":\"waiting\"", b"\"phase\":\"running\""),
        call(D1, argv!["stop", number(RULES), "--key", "rules-stop"]),
        calls::until(D1, argv!["task", number(RULES), "--json"], b"\"stopped_by\"", b"\"phase\":\"running\""),
        calls::release_hold(SCOPE_QUESTION_HOLD),
        calls::until(
            D1,
            argv!["task", number(RULES), "--activity", "32", "--json"],
            SCOPE_QUESTION.as_bytes(),
            b"\"stopped_by\"",
        ),
        // A must park before the answer: its next run then fetches main's guide.
        call(D1, argv!["wait", "task", number(SCOPE), "--until", "phase:idle"]),
        calls::push(RULES_FILE.as_bytes(), NEW_RULES.as_bytes()),
        call(D1, argv!["release", number(RULES), "--hold", number(RULES_HOLD_REVISION), "--key", "rules-release"]),
        join(RULES_ENDED),
        call(D1, argv!["task", number(SCOPE), "--json"]),
        // Every run's ending: a fresh start refused and retried would pass unseen.
        call(D1, argv!["task", number(RULES), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(SCOPE), "--activity", "32", "--json"]),
    ]
}

// Act 2, step 5.3 (domain/testing.md, 5.3)
const SHAPE_ENDED: usize = 2;
pub const READS_5_3: &[(Read, usize)] = &[(Read::MoverEnded, 3), (Read::ShapeEnded, SHAPE_ENDED)];

#[must_use]
pub fn step_5_3() -> Vec<calls::Step> {
    use super::{MOVER, SHAPE, SHAPE_PERSON};
    vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "shape", SHAPE_PERSON]),
        start(D1, argv!["wait", "task", number(SHAPE), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(MOVER), "--until", "ended", "--json"]),
        join(SHAPE_ENDED),
    ]
}

// Act 2, step 5.2 (domain/testing.md, 5.3)
const B_ENDED_5_2: usize = 16;
pub const READS_5_2: &[(Read, usize)] = &[
    (Read::ImplementerRunning, 5),
    (Read::H1Running, 7),
    (Read::HelperEnded, 8),
    (Read::BHeld, 11),
    (Read::ImplementerEnded, 12),
    (Read::H1Ended, 13),
    (Read::BStillHeld, 14),
    (Read::BEnded, B_ENDED_5_2),
    (Read::BEndedEarlier, 17),
    (Read::OwnerEnded, 18),
    (Read::PauseEnded, 19),
];

#[must_use]
pub fn step_5_2() -> Vec<calls::Step> {
    use super::{
        Agent, B_HOLD, BOWNER, H1, H1_REVIEWER, HELPER, IMPLEMENTER, IMPLEMENTER_HOLD, OWNER, PAUSE, PAUSE_PERSON,
        PROJECT, REVIEW_HOLD,
    };
    vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "adopt-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "pause", PAUSE_PERSON]),
        call(D1, argv!["wait", "task", number(PAUSE), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(BOWNER), "--until", "replied"]),
        // The documents establish the case's in-flight work before Stop.
        // The fake Pieces keep each run writing until its release below.
        calls::until(D1, argv!["task", number(IMPLEMENTER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        // h1 exists after B's reply; its review is created asynchronously.
        // Wait for its live delegate card before reading the review's document.
        calls::until(D1, argv!["task", number(H1), "--json"], Agent::Review.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(H1_REVIEWER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        calls::until(
            D1,
            argv!["task", number(HELPER), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
        // replied alone can still be writing; read the parked owner's phase.
        call(D1, argv!["wait", "task", number(BOWNER), "--until", "phase:idle"]),
        call(D1, argv!["stop", number(BOWNER), "--key", "stop-b"]),
        calls::until(
            D1,
            argv!["task", number(BOWNER), "--activity", "32", "--json"],
            b"\"phase\":\"held\"",
            b"\"task\"",
        ),
        calls::release_hold(IMPLEMENTER_HOLD),
        calls::release_hold(REVIEW_HOLD),
        calls::until(
            D1,
            argv!["task", number(IMPLEMENTER), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
        calls::until(D1, argv!["task", number(H1), "--activity", "32", "--json"], b"\"phase\":\"ended\"", b"\"task\""),
        call(D1, argv!["task", number(BOWNER), "--activity", "32", "--json"]),
        call(D1, argv!["release", number(BOWNER), "--hold", number(B_HOLD), "--key", "release-b"]),
        calls::until(
            D1,
            argv!["task", number(BOWNER), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
        // B's two attempts span two activity pages; retain the supplied cursor.
        call(D1, argv!["activity", number(BOWNER), "--before", calls::activity_before(B_ENDED_5_2), "--json"]),
        call(D1, argv!["task", number(OWNER), "--activity", "32", "--json"]),
        calls::until(
            D1,
            argv!["task", number(PAUSE), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
    ]
}

// Act 2, step 5.5 (domain/testing.md, 5.3)
const PLAN_ENDED: usize = 2;
pub const READS_5_5: &[(Read, usize)] = &[
    (Read::PlanEnded, PLAN_ENDED),
    (Read::PlannerParked, 3),
    (Read::PlanAnswerPending, 4),
    (Read::PlannerEnded, 5),
    (Read::PlannerEarlier, 6),
    (Read::PlannerFirst, 7),
    (Read::SubownerEnded, 8),
];

#[must_use]
pub fn step_5_5() -> Vec<calls::Step> {
    use super::{PLAN_ANSWER_HOLD, PLAN_ANSWER_PENDING, PLAN_DIRECTOR, PLAN_PERSON, PLANNER, SUBOWNER};
    vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "plan", PLAN_PERSON]),
        start(D1, argv!["wait", "task", number(PLAN_DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(PLANNER), "--until", "phase:idle", "--json"]),
        // Step::Held gates ctl writes. The LLM's emitted piece, read through
        // the live run document, proves its answer is at the fake hold.
        calls::until(
            D1,
            argv!["run", number(PLAN_DIRECTOR), "--json"],
            PLAN_ANSWER_PENDING.as_bytes(),
            b"\"writing\":",
        ),
        calls::release_hold(PLAN_ANSWER_HOLD),
        join(PLAN_ENDED),
        call(D1, argv!["task", number(PLANNER), "--activity", "32", "--json"]),
        call(D1, argv!["activity", number(PLANNER), "--before", calls::activity_before(5), "--json"]),
        call(D1, argv!["activity", number(PLANNER), "--before", calls::activity_before(6), "--json"]),
        call(D1, argv!["task", number(SUBOWNER), "--json"]),
    ]
}

// Act 2, step 5.6 (domain/testing.md, 5.3)
const MISREAD_ENDED: usize = 2;
pub const READS_5_6: &[(Read, usize)] = &[
    (Read::MisreadEnded, MISREAD_ENDED),
    (Read::MisreadParked, 3),
    (Read::MisreadHelperRunning, 4),
    (Read::MisreadRulePending, 6),
    (Read::MisreadFresh, 7),
    (Read::MisreadHelperEnded, 9),
];

#[must_use]
pub fn step_5_6() -> Vec<calls::Step> {
    use super::{
        MISREAD_CORRECTION, MISREAD_DIRECTOR, MISREAD_HELPER, MISREAD_HELPER_HOLD, MISREAD_PERSON, MISREAD_RECALL_HOLD,
        MISREAD_RULE_PENDING,
    };
    vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "misread", MISREAD_PERSON]),
        start(D1, argv!["wait", "task", number(MISREAD_DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(MISREAD_DIRECTOR), "--until", "phase:idle", "--json"]),
        // As in 5.2, observe the helper's open writing run before cancelling it.
        calls::until(D1, argv!["task", number(MISREAD_HELPER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["message", number(MISREAD_DIRECTOR), "--key", "informative", MISREAD_CORRECTION]),
        calls::until(
            D1,
            argv!["run", number(MISREAD_DIRECTOR), "--json"],
            MISREAD_RULE_PENDING.as_bytes(),
            b"\"writing\":",
        ),
        call(D1, argv!["task", number(MISREAD_DIRECTOR), "--activity", "32", "--json"]),
        calls::release_hold(MISREAD_HELPER_HOLD),
        call(D1, argv!["wait", "task", number(MISREAD_HELPER), "--until", "ended"]),
        call(D1, argv!["task", number(MISREAD_HELPER), "--activity", "32", "--json"]),
        // Adopt only when the read needs it: no repeated forge polling while
        // the two scripted runs establish the cancellation and fresh context.
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "misread-kit"
            ],
        ),
        calls::release_hold(MISREAD_RECALL_HOLD),
        join(MISREAD_ENDED),
    ]
}

// Act 2, step 5.4 (domain/testing.md, 5.3)
const DIRECTION_B_ENDED: usize = 30;
const STORY_OWNER_ENDED: usize = 34;
const DIRECTION_ENDED: usize = 36;
const STORY_OWNER_EARLIER: usize = 37;
pub const READS_5_4: &[(Read, usize)] = &[
    (Read::X3Open, 7),
    (Read::X3Produced, 8),
    (Read::PlannedRunning, 12),
    (Read::DirectionBHeld, 15),
    (Read::DirectionNoteWritten, 18),
    (Read::X3Ended, 20),
    (Read::StoryLanded, 23),
    (Read::DirectionBPending, 24),
    (Read::PlannedEnded, 28),
    (Read::DirectionBEnded, DIRECTION_B_ENDED),
    (Read::StoryOwnerEnded, STORY_OWNER_ENDED),
    (Read::DirectionEnded, DIRECTION_ENDED),
    (Read::StoryOwnerEarlier, STORY_OWNER_EARLIER),
    (Read::StoryOwnerFirst, 38),
    (Read::DirectionFirst, 39),
];
#[must_use]
pub fn step_5_4() -> Vec<calls::Step> {
    use super::{
        Agent, B_DELEGATE_HOLD, DIRECTION, DIRECTION_MESSAGE, DIRECTION_PERSON, OVERLAP_HOLD_REVISION, OVERLAP_OWNER,
        PLANNED, PLANNED_CODER, PLANNED_CODING_HOLD, STORY_CHANGE, STORY_OWNER, STORY_RECALL_HOLD, X3, X3_CODER,
        X3_REVIEW_HOLD, X3_REVIEWER,
    };
    vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "adopt-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "direction", DIRECTION_PERSON]),
        call(D1, argv!["wait", "task", number(DIRECTION), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(STORY_OWNER), "--until", "replied"]),
        // The delegate cue selects x3's review before its writing observation.
        // Fake Pieces keep it writing until ReleaseHold (i2's addendum 2).
        calls::until(D1, argv!["task", number(X3), "--json"], Agent::Review.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(X3_REVIEWER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(X3), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(X3_CODER), "--activity", "32", "--json"]),
        calls::release_hold(B_DELEGATE_HOLD),
        calls::until(D1, argv!["task", number(OVERLAP_OWNER), "--json"], Agent::PlannedWriter.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(PLANNED), "--json"], b"\"live_delegates\":1", b"\"task\""),
        calls::until(D1, argv!["task", number(PLANNED_CODER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(PLANNED), "--activity", "32", "--json"]),
        call(D1, argv!["wait", "task", number(OVERLAP_OWNER), "--until", "phase:idle"]),
        call(D1, argv!["stop", number(OVERLAP_OWNER), "--key", "stop-b"]),
        calls::until(
            D1,
            argv!["task", number(OVERLAP_OWNER), "--activity", "32", "--json"],
            b"\"phase\":\"held\"",
            b"\"task\"",
        ),
        call(D1, argv!["message", number(DIRECTION), "--key", "new-direction", DIRECTION_MESSAGE]),
        call(D1, argv!["wait", "task", number(X3), "--until", "phase:closing"]),
        // Commit the note before releasing the review: otherwise the cancel's
        // result can take the note's fresh number between A's two tool calls.
        calls::until(
            D1,
            argv!["task", number(STORY_OWNER), "--activity", "32", "--json"],
            b"\"tool\":\"note\"",
            b"\"task\"",
        ),
        calls::release_hold(X3_REVIEW_HOLD),
        call(D1, argv!["wait", "task", number(X3), "--until", "ended"]),
        call(D1, argv!["task", number(X3), "--activity", "32", "--json"]),
        calls::until(D1, argv!["task", number(STORY_OWNER), "--json"], Agent::StoryWriter.cue(), b"\"task\""),
        // a1's review has no hold. Watch endings once, then read their journals,
        // avoiding repeated reads of the same large activity documents.
        call(D1, argv!["wait", "task", number(STORY_CHANGE), "--until", "ended"]),
        call(D1, argv!["task", number(STORY_CHANGE), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(OVERLAP_OWNER), "--activity", "32", "--json"]),
        call(
            D1,
            argv!["release", number(OVERLAP_OWNER), "--hold", number(OVERLAP_HOLD_REVISION), "--key", "release-b"],
        ),
        call(D1, argv!["wait", "task", number(PLANNED), "--until", "phase:closing"]),
        calls::release_hold(PLANNED_CODING_HOLD),
        call(D1, argv!["wait", "task", number(PLANNED), "--until", "ended"]),
        call(D1, argv!["task", number(PLANNED), "--activity", "32", "--json"]),
        call(D1, argv!["wait", "task", number(OVERLAP_OWNER), "--until", "ended"]),
        call(D1, argv!["task", number(OVERLAP_OWNER), "--activity", "32", "--json"]),
        // B's batched turns fit its complete journal in this one page.
        // Select the run woken by a1's change result, then await its held
        // writing turn (i2's addendum 2). x3's result was a cancellation.
        calls::until(
            D1,
            argv!["task", number(STORY_OWNER), "--activity", "32", "--json"],
            b"\"kind\":{\"result\":{\"change\":",
            b"\"task\"",
        ),
        calls::until(D1, argv!["task", number(STORY_OWNER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        calls::release_hold(STORY_RECALL_HOLD),
        call(D1, argv!["wait", "task", number(STORY_OWNER), "--until", "ended"]),
        call(D1, argv!["task", number(STORY_OWNER), "--activity", "32", "--json"]),
        call(D1, argv!["wait", "task", number(DIRECTION), "--until", "ended"]),
        call(D1, argv!["task", number(DIRECTION), "--activity", "32", "--json"]),
        call(
            D1,
            argv!["activity", number(STORY_OWNER), "--before", calls::activity_before(STORY_OWNER_ENDED), "--json"],
        ),
        call(
            D1,
            argv!["activity", number(STORY_OWNER), "--before", calls::activity_before(STORY_OWNER_EARLIER), "--json"],
        ),
        // B's hold can give the director an extra parked run before the
        // person's direction. Keep that run and the initial delegation too.
        call(D1, argv!["activity", number(DIRECTION), "--before", calls::activity_before(DIRECTION_ENDED), "--json"]),
    ]
}

// Act 3, step 6.1 (domain/testing.md, 5.3)
pub const READS_6_1: &[(Read, usize)] = &[
    (Read::BatchOneWriting, 8),
    (Read::BatchTwoWriting, 9),
    (Read::BatchThreeWriting, 10),
    (Read::BatchStoryWaiting, 11),
    (Read::BatchOneBeforeStory, 12),
    (Read::BatchStoryAfterOne, 13),
    (Read::BatchStoryWriting, 14),
    (Read::BatchTwoBeforeStory, 15),
    (Read::BatchThreeStillWriting, 16),
    (Read::BatchThreeEnded, 17),
    (Read::BatchOneEnded, 19),
    (Read::BatchTwoEnded, 20),
    (Read::BatchStoryEnded, 21),
    (Read::BatchSubownerEnded, 22),
    (Read::BatchOwnerEnded, 23),
    (Read::BatchDirectorEnded, 24),
];

#[must_use]
pub fn step_6_1() -> Vec<calls::Step> {
    use super::{
        BATCH_DIRECTOR, BATCH_HOLDS, BATCH_ONE, BATCH_OWNER, BATCH_PERSON, BATCH_STORY, BATCH_SUBOWNER, BATCH_THREE,
        BATCH_TWO,
    };
    let mut steps =
        vec![sign_in(D1), call(D1, argv!["chat", "--project", number(PROJECT), "--key", "batch", BATCH_PERSON])];
    // Each requester has committed its delegation before its delegate is
    // read. An Until cannot retry an unknown task.
    steps.extend([
        call(D1, argv!["wait", "task", number(BATCH_DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(BATCH_OWNER), "--until", "replied"]),
        calls::until(D1, argv!["task", number(BATCH_SUBOWNER), "--json"], b"\"live_delegates\":4", b"\"task\""),
    ]);
    // All four fake holds stay unreleased throughout these task reads.
    for task in [BATCH_ONE, BATCH_TWO, BATCH_THREE] {
        steps.push(calls::until(D1, argv!["task", number(task), "--json"], b"\"work\":\"writing\"", b"\"task\""));
    }
    for task in [BATCH_ONE, BATCH_TWO, BATCH_THREE] {
        steps.push(call(D1, argv!["task", number(task), "--json"]));
    }
    steps.extend([
        call(D1, argv!["task", number(BATCH_STORY), "--activity", "32", "--json"]),
        calls::release_hold(BATCH_HOLDS[0]),
        calls::until(
            D1,
            argv!["task", number(BATCH_ONE), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
        call(D1, argv!["task", number(BATCH_STORY), "--activity", "32", "--json"]),
        calls::release_hold(BATCH_HOLDS[1]),
        calls::until(D1, argv!["task", number(BATCH_STORY), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(BATCH_TWO), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(BATCH_THREE), "--json"]),
        calls::release_hold(BATCH_HOLDS[2]),
        // a4's result is the final cue for A1, after a3 too has ended.
        calls::until(
            D1,
            argv!["task", number(BATCH_THREE), "--activity", "32", "--json"],
            b"\"phase\":\"ended\"",
            b"\"task\"",
        ),
        calls::release_hold(BATCH_HOLDS[3]),
        call(D1, argv!["wait", "task", number(BATCH_DIRECTOR), "--until", "ended"]),
    ]);
    for task in [BATCH_ONE, BATCH_TWO, BATCH_STORY, BATCH_SUBOWNER, BATCH_OWNER, BATCH_DIRECTOR] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps
}

// Act 3, step 6.2 (domain/testing.md, 5.3)
pub const READS_6_2: &[(Read, usize)] = &[
    (Read::BlockerKept, 7),
    (Read::BlockerReviewWriting, 11),
    (Read::BlockerPending, 12),
    (Read::BlockerA3Ended, 14),
    (Read::BlockerE1Ended, 15),
    (Read::BlockerCoderEnded, 16),
    (Read::BlockerReviewerEnded, 17),
    (Read::BlockerSubownerEnded, 18),
    (Read::BlockerOwnerEnded, 19),
    (Read::BlockerDirectorEnded, 20),
];

#[must_use]
pub fn step_6_2() -> Vec<calls::Step> {
    use super::{
        Agent, BLOCKER_A3, BLOCKER_CODER, BLOCKER_DIRECTOR, BLOCKER_E1, BLOCKER_OWNER, BLOCKER_PERSON,
        BLOCKER_REVIEW_HOLD, BLOCKER_REVIEWER, BLOCKER_SUBOWNER, PROJECT,
    };
    let mut steps = vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "adopt-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "blocker", BLOCKER_PERSON]),
        call(D1, argv!["wait", "task", number(BLOCKER_DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(BLOCKER_OWNER), "--until", "replied"]),
        calls::until(
            D1,
            argv!["task", number(BLOCKER_SUBOWNER), "--json"],
            Agent::BlockerImplementer.cue(),
            b"\"task\"",
        ),
        // Task and activity are separate reads. First wait for the task
        // snapshot to show its dependency wait, then for its saved run's
        // kept row. A kept row alone can follow a still-running snapshot;
        // the dependency wait alone can precede the saved activity.
        calls::until(D1, argv!["task", number(BLOCKER_A3), "--json"], b"\"waits_on\":{\"dependencies\":", b"\"task\""),
        calls::until(
            D1,
            argv!["task", number(BLOCKER_A3), "--activity", "32", "--json"],
            b"\"kept\":{\"attempt\":",
            b"\"task\"",
        ),
        // A nonempty delegate list proves the coder exists. If coding
        // already ended, the held review keeps that list nonempty. Wait
        // on the coder's ending instead of polling e1 throughout coding.
        calls::until(D1, argv!["task", number(BLOCKER_E1), "--json"], b"\"delegates\":[{", b"\"task\""),
        call(D1, argv!["wait", "task", number(BLOCKER_CODER), "--until", "ended"]),
        calls::until(D1, argv!["task", number(BLOCKER_E1), "--json"], Agent::Review.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(BLOCKER_REVIEWER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(BLOCKER_E1), "--json"]),
        calls::release_hold(BLOCKER_REVIEW_HOLD),
        call(D1, argv!["wait", "task", number(BLOCKER_DIRECTOR), "--until", "ended"]),
    ];
    for task in
        [BLOCKER_A3, BLOCKER_E1, BLOCKER_CODER, BLOCKER_REVIEWER, BLOCKER_SUBOWNER, BLOCKER_OWNER, BLOCKER_DIRECTOR]
    {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps
}

// Act 3, step 6.5 (domain/testing.md, 5.3)
pub const READS_6_5: &[(Read, usize)] = &[
    (Read::BudgetAsked, 5),
    (Read::BudgetMedium, 10),
    (Read::BudgetHigh, 11),
    (Read::BudgetA5Ended, 12),
    (Read::BudgetAdvisorEnded, 13),
    (Read::BudgetSubownerEnded, 14),
    (Read::BudgetOwnerEnded, 15),
    (Read::BudgetDirectorEnded, 16),
    (Read::BudgetInboxAnswered, 17),
    (Read::BudgetSubownerFirst, 18),
    (Read::BudgetOwnerFirst, 19),
    (Read::BudgetDirectorFirst, 20),
];

#[must_use]
pub fn step_6_5() -> Vec<calls::Step> {
    use super::{
        Agent, BUDGET_A5, BUDGET_ADVISOR, BUDGET_AMEND_HOLD, BUDGET_DIRECTOR, BUDGET_HIGH_HOLD, BUDGET_OWNER,
        BUDGET_PERSON, BUDGET_PERSON_ANSWER, BUDGET_SUBOWNER, BUDGET_UP_QUESTION_NUMBER, PROJECT,
    };
    let mut steps = vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "budget", BUDGET_PERSON]),
        start(D1, argv!["wait", "task", number(BUDGET_DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "inbox", "--until", "waiting"]),
        call(D1, argv!["wait", "task", number(BUDGET_DIRECTOR), "--until", "phase:idle"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(
            D1,
            argv![
                "answer",
                number(BUDGET_DIRECTOR),
                number(BUDGET_UP_QUESTION_NUMBER),
                BUDGET_PERSON_ANSWER,
                "--key",
                "budget-answer"
            ],
        ),
        calls::until(
            D1,
            argv!["task", number(BUDGET_SUBOWNER), "--json"],
            Agent::BudgetInvestigator.cue(),
            b"\"task\"",
        ),
        calls::until(D1, argv!["task", number(BUDGET_SUBOWNER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["wait", "task", number(BUDGET_A5), "--until", "phase:idle"]),
        call(D1, argv!["task", number(BUDGET_A5), "--activity", "32", "--json"]),
        calls::release_hold(BUDGET_AMEND_HOLD),
        calls::until(
            D1,
            argv!["task", number(BUDGET_A5), "--activity", "32", "--json"],
            b"\"work\":\"writing\"",
            b"\"task\"",
        ),
        calls::release_hold(BUDGET_HIGH_HOLD),
        join(2),
    ];
    for task in [BUDGET_A5, BUDGET_ADVISOR, BUDGET_SUBOWNER, BUDGET_OWNER, BUDGET_DIRECTOR] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps.push(call(D1, argv!["inbox", "--kind", "questions", "--json"]));
    // Follow each supplied cursor; never infer a complete journal from a
    // latest page whose first attempts have fallen outside the window.
    for (task, latest) in [(BUDGET_SUBOWNER, 14), (BUDGET_OWNER, 15), (BUDGET_DIRECTOR, 16)] {
        steps.push(call(D1, argv!["activity", number(task), "--before", calls::activity_before(latest), "--json"]));
    }
    steps
}

// Act 3, step 6.4 (domain/testing.md, 5.3)
pub const READS_6_4: &[(Read, usize)] = &[
    (Read::RedReviewWriting, 8),
    (Read::RedChangeOpen, 9),
    (Read::RedRootsPending, 10),
    (Read::RedRepairWriting, 12),
    // Reuse the creation read; the held coder has its separate document.
    (Read::RedRepairPending, 11),
    (Read::RedTimerArmed, 14),
    (Read::RedChangePaused, 15),
    (Read::RedDirectorEnded, 18),
    (Read::RedOwnerEnded, 19),
    (Read::RedChangeEnded, 20),
    (Read::RedCoderEnded, 21),
    (Read::RedReviewEnded, 22),
    // The ended wait already returns the complete procedure document.
    (Read::RedRepairEnded, 16),
    (Read::RedRepairCoderEnded, 23),
    (Read::RedRepairReviewEnded, 24),
    (Read::RedUpdatedReviewEnded, 25),
    (Read::RedRootsEnded, 26),
];
#[must_use]
pub fn step_6_4() -> Vec<calls::Step> {
    use super::{
        Agent, PROJECT, RED_BROKEN, RED_CHANGE, RED_CODER, RED_DIRECTOR, RED_FILE, RED_MESSAGE, RED_OWNER, RED_PERSON,
        RED_REPAIR, RED_REPAIR_CODER, RED_REPAIR_HOLD, RED_REPAIR_REVIEW, RED_REVIEW, RED_REVIEW_HOLD,
        RED_UPDATED_REVIEW,
    };
    let mut steps = vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "red-kit"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "red", RED_PERSON]),
        call(D1, argv!["wait", "task", number(RED_DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(RED_OWNER), "--until", "replied"]),
        // The producing procedure parks after making its coder; watch that ending.
        call(D1, argv!["wait", "task", number(RED_CHANGE), "--until", "phase:idle"]),
        call(D1, argv!["wait", "task", number(RED_CODER), "--until", "ended"]),
        // No ctl predicate selects review creation: poll only this later transition.
        calls::until(D1, argv!["task", number(RED_CHANGE), "--json"], Agent::Review.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(RED_REVIEW), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        // News and forge hints wake a4's procedure for a step that finds nothing to do: a plain read can catch it
        // due. Read it settled: an ended a4 answers with its ended read, which the oracle names.
        call(D1, argv!["wait", "task", number(RED_CHANGE), "--until", "phase:idle", "--json"]),
        calls::push(RED_FILE.as_bytes(), RED_BROKEN.as_bytes()),
        // A ready change, after its gate passes, discovers the red base.
        calls::release_hold(RED_REVIEW_HOLD),
        // Until cannot retry an unknown task: first observe the deployment
        // repair among project roots, then its coder's live delegate card.
        calls::until(
            D1,
            argv!["tasks", "--project", number(PROJECT), "--json"],
            Agent::RedRepairCoder.cue(),
            b"\"tasks\"",
        ),
        calls::until(D1, argv!["task", number(RED_REPAIR), "--json"], Agent::RedRepairCoder.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(RED_REPAIR_CODER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["message", number(RED_DIRECTOR), "--key", "red-tip", RED_MESSAGE]),
        calls::until(
            D1,
            argv!["task", number(RED_DIRECTOR), "--activity", "32", "--json"],
            b"\"kind\":{\"timer\":{",
            b"\"task\"",
        ),
        call(D1, argv!["wait", "task", number(RED_CHANGE), "--until", "phase:idle", "--json"]),
        calls::release_hold(RED_REPAIR_HOLD),
        call(D1, argv!["wait", "task", number(RED_REPAIR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(RED_DIRECTOR), "--until", "ended"]),
    ];
    for task in [
        RED_DIRECTOR,
        RED_OWNER,
        RED_CHANGE,
        RED_CODER,
        RED_REVIEW,
        RED_REPAIR_CODER,
        RED_REPAIR_REVIEW,
        RED_UPDATED_REVIEW,
    ] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps.push(call(D1, argv!["tasks", "--project", number(PROJECT), "--ended", "--json"]));
    steps
}

// Act 3, step 6.7 (domain/testing.md, 5.3)
pub const READS_6_7: &[(Read, usize)] = &[
    (Read::CapHeldWait, 7),
    (Read::CapHeld, 8),
    (Read::CapA5Writing, 9),
    (Read::CapOwnerWaiting, 10),
    (Read::CapInboxHeld, 11),
    (Read::CapInboxHeldAfter, 12),
    (Read::CapPassed, 15),
    (Read::CapReleased, 17),
    (Read::CapA5Ended, 18),
    (Read::CapSubownerEnded, 19),
    (Read::CapOwnerEnded, 20),
    (Read::CapDirectorEnded, 21),
    (Read::CapInboxEnded, 22),
    (Read::CapInboxEndedAfter, 23),
    (Read::CapSubownerFirst, 24),
    (Read::CapDirectorFirst, 25),
];

#[must_use]
pub fn step_6_7() -> Vec<calls::Step> {
    use super::{
        Agent, CAP_A5, CAP_A5_HOLD, CAP_DIRECTOR, CAP_OWNER, CAP_PASS_HOLD, CAP_PERSON, CAP_SPLIT_HOLD, CAP_SUBOWNER,
        PROJECT,
    };
    let mut steps = vec![
        sign_in(D1),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "cap", CAP_PERSON]),
        start(D1, argv!["wait", "task", number(CAP_DIRECTOR), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "task", number(CAP_DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(CAP_OWNER), "--until", "replied"]),
        calls::until(D1, argv!["task", number(CAP_SUBOWNER), "--json"], Agent::CapImplementer.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(CAP_A5), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        // The deadline hold lasts until the director is explicitly released below.
        call(D1, argv!["wait", "task", number(CAP_SUBOWNER), "--until", "held", "--json"]),
        calls::until(
            D1,
            argv!["task", number(CAP_SUBOWNER), "--activity", "32", "--json"],
            b"\"reason\":\"deadline\"",
            b"\"task\"",
        ),
        call(D1, argv!["task", number(CAP_A5), "--activity", "32", "--json"]),
        calls::until(
            D1,
            argv!["task", number(CAP_OWNER), "--activity", "32", "--json"],
            b"\"work\":\"writing\"",
            b"\"task\"",
        ),
        // The needs-group tag starts an unfiltered page, then its stored continuation.
        call(D1, argv!["inbox", "--after", "00", "--json"]),
        call(D1, argv!["inbox", "--after", calls::inbox_after(11), "--json"]),
        calls::release_hold(CAP_PASS_HOLD),
        // The director's document includes A's observation too. Once A parks
        // after its pass, only the director can satisfy the writing checkpoint.
        call(D1, argv!["wait", "task", number(CAP_OWNER), "--until", "phase:idle"]),
        calls::until(D1, argv!["task", number(CAP_DIRECTOR), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(CAP_SUBOWNER), "--activity", "32", "--json"]),
        calls::release_hold(CAP_SPLIT_HOLD),
        call(D1, argv!["wait", "task", number(CAP_SUBOWNER), "--until", "phase:idle"]),
        call(D1, argv!["task", number(CAP_SUBOWNER), "--activity", "32", "--json"]),
        calls::release_hold(CAP_A5_HOLD),
        join(2),
    ];
    for task in [CAP_A5, CAP_SUBOWNER, CAP_OWNER, CAP_DIRECTOR] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps.push(call(D1, argv!["inbox", "--after", "00", "--json"]));
    steps.push(call(D1, argv!["inbox", "--after", calls::inbox_after(22), "--json"]));

    // The latest activity windows omit the starts of A1 and the director.
    for (task, latest) in [(CAP_SUBOWNER, 19), (CAP_DIRECTOR, 21)] {
        steps.push(call(D1, argv!["activity", number(task), "--before", calls::activity_before(latest), "--json"]));
    }
    steps
}

// Act 3, step 6.6. Name every call position: holds and releases make no call.
const WAITING_SIGN_IN: usize = 0;
const WAITING_ADOPT_APP: usize = WAITING_SIGN_IN + 1;
const WAITING_ADOPT_KIT: usize = WAITING_ADOPT_APP + 1;
const WAITING_CHAT: usize = WAITING_ADOPT_KIT + 1;
const WAITING_DIRECTOR_REPLIED: usize = WAITING_CHAT + 1;
const WAITING_OWNER_REPLIED: usize = WAITING_DIRECTOR_REPLIED + 1;
const WAITING_REVIEW_CREATED: usize = WAITING_OWNER_REPLIED + 1;
const WAITING_REVIEW_WRITING: usize = WAITING_REVIEW_CREATED + 1;
const WAITING_E1_PENDING: usize = WAITING_REVIEW_WRITING + 1;
const WAITING_REPIN_PENDING: usize = WAITING_E1_PENDING + 1;
const WAITING_DIRECTOR_ENDED: usize = WAITING_REPIN_PENDING + 1;
const WAITING_ENDED_FIRST: usize = WAITING_DIRECTOR_ENDED + 1;
pub const READS_6_6: &[(Read, usize)] = &[
    (Read::WaitingReviewWriting, WAITING_REVIEW_WRITING),
    (Read::WaitingE1Pending, WAITING_E1_PENDING),
    (Read::WaitingRepinPending, WAITING_REPIN_PENDING),
    (Read::WaitingDirectorEnded, WAITING_ENDED_FIRST),
    (Read::WaitingOwnerEnded, WAITING_ENDED_FIRST + 1),
    (Read::WaitingE1Ended, WAITING_ENDED_FIRST + 2),
    (Read::WaitingRepinEnded, WAITING_ENDED_FIRST + 3),
    (Read::WaitingE1CoderEnded, WAITING_ENDED_FIRST + 4),
    (Read::WaitingE1ReviewerEnded, WAITING_ENDED_FIRST + 5),
    (Read::WaitingRepinCoderEnded, WAITING_ENDED_FIRST + 6),
    (Read::WaitingRepinReviewerEnded, WAITING_ENDED_FIRST + 7),
    (Read::WaitingRootsEnded, WAITING_ENDED_FIRST + 8),
];

#[must_use]
pub fn step_6_6() -> Vec<calls::Step> {
    use super::{
        Agent,
        waiting::{A1, DIRECTOR, E1, E1_CODER, E1_REVIEWER, PERSON, REPIN, REPIN_CODER, REPIN_REVIEWER, REVIEW_HOLD},
    };
    let mut steps = vec![sign_in(D1)];
    for name in ["app", "kit"] {
        steps.push(call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                name,
                "--landing",
                "main",
                "--no-home",
                "--key",
                name
            ],
        ));
    }
    steps.extend([
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "waiting", PERSON]),
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(A1), "--until", "replied"]),
        calls::until(D1, argv!["task", number(E1), "--json"], Agent::Review.cue(), b"\"task\""),
        calls::until(D1, argv!["task", number(E1_REVIEWER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(E1), "--json"]),
        call(D1, argv!["task", number(REPIN), "--activity", "32", "--json"]),
        calls::release_hold(REVIEW_HOLD),
        // After this release, the person only waits and reads. No targeted
        // release/amend/message starts repin; the initial dependency does.
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "ended"]),
    ]);
    for task in [DIRECTOR, A1, E1, REPIN, E1_CODER, E1_REVIEWER, REPIN_CODER, REPIN_REVIEWER] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps.push(call(D1, argv!["tasks", "--project", number(PROJECT), "--ended", "--json"]));
    steps
}

// 6.8: release steps do not occupy ctl call positions.
const QA_SIGN_IN: usize = 0;
const QA_ADOPT: usize = QA_SIGN_IN + 1;
const QA_CHAT: usize = QA_ADOPT + 1;
const QA_DIRECTOR_REPLIED: usize = QA_CHAT + 1;
const QA_OWNER_REPLIED: usize = QA_DIRECTOR_REPLIED + 1;
const QA_TESTER_WRITING: usize = QA_OWNER_REPLIED + 1;
const QA_OWNER_WRITING: usize = QA_TESTER_WRITING + 1;
const QA_SUBOWNER_BEFORE: usize = QA_OWNER_WRITING + 1;
const QA_NEXT_CREATED: usize = QA_SUBOWNER_BEFORE + 1;
const QA_NEXT_WRITING: usize = QA_NEXT_CREATED + 1;
const QA_SUBOWNER_LIVE: usize = QA_NEXT_WRITING + 1;
const QA_TESTER_LIVE: usize = QA_SUBOWNER_LIVE + 1;
const QA_NEXT_ENDED: usize = QA_TESTER_LIVE + 1;
const QA_DIRECTOR_ENDED: usize = QA_NEXT_ENDED + 1;
const QA_FINAL: usize = QA_DIRECTOR_ENDED + 1;
const QA_INBOX: usize = QA_FINAL + 8;
const QA_INBOX_AFTER: usize = QA_INBOX + 1;
const QA_ROOTS: usize = QA_INBOX_AFTER + 1;
pub const READS_6_8: &[(Read, usize)] = &[
    (Read::QaSubownerBefore, QA_SUBOWNER_BEFORE),
    (Read::QaNextWriting, QA_NEXT_WRITING),
    (Read::QaSubownerLive, QA_SUBOWNER_LIVE),
    (Read::QaTesterLive, QA_TESTER_LIVE),
    (Read::QaDirectorEnded, QA_FINAL),
    (Read::QaOwnerEnded, QA_FINAL + 1),
    (Read::QaSubownerEnded, QA_FINAL + 2),
    (Read::QaTesterEnded, QA_FINAL + 3),
    (Read::QaNextEnded, QA_FINAL + 4),
    (Read::QaChangeEnded, QA_FINAL + 5),
    (Read::QaCoderEnded, QA_FINAL + 6),
    (Read::QaReviewerEnded, QA_FINAL + 7),
    (Read::QaInboxEnded, QA_INBOX),
    (Read::QaInboxAfter, QA_INBOX_AFTER),
    (Read::QaRootsEnded, QA_ROOTS),
];

#[must_use]
pub fn step_6_8() -> Vec<calls::Step> {
    use super::qa::{A, A1, A2, C, CODER, DIRECTOR, NEXT_HOLD, OWNER_HOLD, PERSON, REVIEWER, T, TESTER_HOLD};
    let mut steps = vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                "acme",
                "--name",
                "app",
                "--landing",
                "main",
                "--no-home",
                "--key",
                "qa-adopt"
            ],
        ),
        call(D1, argv!["chat", "--project", number(PROJECT), "--key", "qa", PERSON]),
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "replied"]),
        call(D1, argv!["wait", "task", number(A1), "--until", "replied"]),
        calls::until(D1, argv!["task", number(T), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        calls::until(D1, argv!["task", number(A), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(A1), "--activity", "32", "--json"]),
        calls::release_hold(OWNER_HOLD),
        call(D1, argv!["wait", "task", number(A), "--until", "phase:idle"]),
        calls::until(D1, argv!["task", number(A2), "--json"], b"\"work\":\"writing\"", b"\"task\""),
        call(D1, argv!["task", number(A1), "--activity", "32", "--json"]),
        call(D1, argv!["task", number(T), "--activity", "32", "--json"]),
        calls::release_hold(NEXT_HOLD),
        call(D1, argv!["wait", "task", number(A2), "--until", "ended"]),
        calls::release_hold(TESTER_HOLD),
        call(D1, argv!["wait", "task", number(DIRECTOR), "--until", "ended"]),
    ];
    for task in [DIRECTOR, A, A1, T, A2, C, CODER, REVIEWER] {
        steps.push(call(D1, argv!["task", number(task), "--activity", "32", "--json"]));
    }
    steps.push(call(D1, argv!["inbox", "--after", "00", "--json"]));
    steps.push(call(D1, argv!["inbox", "--after", calls::inbox_after(QA_INBOX), "--json"]));
    steps.push(call(D1, argv!["tasks", "--project", number(PROJECT), "--ended", "--json"]));
    steps
}
