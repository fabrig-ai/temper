//! The person's acts, with documents identified by the moment they were read.
use super::{A, DESIGN, DIRECTOR, PERSON, PROJECT};
use crate::{
    argv,
    calls::{self, D1, call, join, number, sign_in, start},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Read {
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
    HangEnded,
    HangQuestionRead,
    HangWaiting,
    PhaseEnded,
    HangActivity,
    HangActivityOlder,
    HangActivityFirst,
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

// The two held slices share their reads; the refused one adds the person's act.
const HELD_A_ENDED: usize = 3;
const A_WAITING: usize = 5;
const HELD_ACCOUNTS: usize = 6;
const REFUSED_DIRECTOR_ENDED: usize = 8;
const LIMITED_DIRECTOR_ENDED: usize = 7;
pub const READS_4_5_REFUSED: &[(Read, usize)] = &[
    (Read::DirectorEnded, REFUSED_DIRECTOR_ENDED),
    (Read::AWaiting, A_WAITING),
    (Read::Refused, HELD_ACCOUNTS),
    (Read::AEnded, HELD_A_ENDED),
];
pub const READS_4_5_LIMITED: &[(Read, usize)] = &[
    (Read::DirectorEnded, LIMITED_DIRECTOR_ENDED),
    (Read::AWaiting, A_WAITING),
    (Read::Limited, HELD_ACCOUNTS),
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
        call(D1, argv!["wait", "task", number(A), "--until", "phase:due"]),
        call(D1, argv!["task", number(A), "--json"]),
        call(D1, argv!["accounts", "--json"]),
    ];
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

const HANG_ENDED: usize = 1;
const HANG_QUESTION_READ: usize = 2;
const HANG_WAITING: usize = 3;
const HANG_A_ENDED: usize = 4;
const HANG_ACTIVITY: usize = 5;
const HANG_ACTIVITY_OLDER: usize = 6;
const HANG_ACTIVITY_FIRST: usize = 7;
pub const READS_4_6: &[(Read, usize)] = &[
    (Read::HangEnded, HANG_ENDED),
    (Read::HangQuestionRead, HANG_QUESTION_READ),
    (Read::HangWaiting, HANG_WAITING),
    (Read::PhaseEnded, HANG_A_ENDED),
    (Read::HangActivity, HANG_ACTIVITY),
    (Read::HangActivityOlder, HANG_ACTIVITY_OLDER),
    (Read::HangActivityFirst, HANG_ACTIVITY_FIRST),
];

#[must_use]
pub fn step_4_6(base: usize) -> Vec<calls::Step> {
    use super::{HANG, HANG_PERSON, HANG_READ, PHASE};
    vec![
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

#[must_use]
pub fn step_4_1_4_3_4_6() -> Vec<calls::Step> {
    let mut steps = step_4_1_4_3();
    let base = reads_4_1_4_3().iter().map(|(_, call)| call).max().expect("check calls") + 1;
    steps.extend(step_4_6(base));
    steps
}

#[must_use]
pub fn reads_4_1_4_3_4_6() -> Vec<(Read, usize)> {
    let mut reads = reads_4_1_4_3();
    let base = reads.iter().map(|(_, call)| call).max().expect("check calls") + 1;
    reads.extend(READS_4_6.iter().map(|&(read, call)| (read, base + call)));
    reads
}
