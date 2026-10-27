//! The reference case's cast, accounts and first kit tree (domain/testing.md, 5.3).
use crate::{fixtures, forge, landing};
use serde_json::{Value, json};
use skein_fake_forge_domain::api;

pub mod acts;
pub mod defaults;
pub mod oracle;
pub mod scripts;

// Fresh deployment numbers; the oracle checks them against the person's documents
// and A's question's tool output and the answer delivered to A.
pub const PROJECT: u64 = 1;
pub const DIRECTOR: u64 = 1;
pub const A: u64 = 2;
pub const DESIGN: u64 = 3;
// The design's current-head CI subscription and news precede this question.
pub const QUESTION: u64 = 8;
// After step_4_1's chat and A's delegation create tasks 1..=5,
// step_4_3's chat creates CHECK; its delegate creates B, which delegates BENCHMARK.
pub const CHECK: u64 = 6;
pub const B: u64 = 7;
pub const BENCHMARK: u64 = 8;
// The change procedure in step_4_1 creates its coder, then its reviewer.
pub const CODER: u64 = 4;
pub const REVIEW: u64 = 5;
pub const CHECK_PERSON: &str = "@case_check Schedule B's benchmark and run the two-hour check.";
pub const B_SPEC: &str = "@case_b Own the benchmark; delegate it and report its ending.";
pub const BENCHMARK_SPEC: &str = "@case_benchmark Run the benchmark and wait in place.";
pub const BENCHMARK_RUNNING: &str = "The benchmark is running.";
pub const CHECK_REASON: &str = "The two-hour check found the benchmark stalled; cancel it.";
pub const B_REPORT: &str = "The benchmark was cancelled; B has nothing left running.";
pub const CHECK_REPORT: &str = "The two-hour check cancelled the stalled benchmark and B reported.";
pub const FIRST_ACCOUNT: u64 = 1;
pub const SECOND_ACCOUNT: u64 = 3;
pub const SECOND_KEY_FILE: &str = "second.key";
pub const DEFAULT_MODEL: &str = "fake";
pub const DEFAULT_MODEL_NUMBER: u8 = 0;
pub const SURVEY: u64 = 3;
pub const PACE_QUESTION_MESSAGE: u64 = 1;
pub const PACE_PERSON: &str = "@case_director Ask about the pace and keep my answer.";
pub const PACE_QUESTION: &str = "Claude spends eight times what the week can sustain. I recommend moving what needs no taste to Codex and switching s2 to the second subscription. Keep the pace?";
pub const PACE_ANSWER: &str = "Keep the pace while the hardware allows; move what needs no taste.";
pub const PACE_NOTE: &str = "Pace";
pub const PACE_PENDING: &str = "The pace question is waiting for your answer.";
pub const PACE_A_SPEC: &str =
    "@case_a Ask the director whether to keep the pace; on its direction, have a survey delegated and report.";
// Distinct from the initial question's cue after smith recaps on a model change.
pub const PACE_A_RESUMED_SPEC: &str = "@case_a resumed Have the survey delegated and report on the pace direction.";
pub const HELD_PERSON: &str = "@case_director Delegate A's report and wait for it.";
pub const HELD_A_SPEC: &str = "@case_a Report when your account allows the work to continue.";
pub const SURVEY_SPEC: &str = "@case_survey Survey the work that needs no taste and report.";
pub const RESUMED_REPORT: &str = "The work continued on its account.";
pub const PACE_REPORT: &str = "The pace is kept and the survey is done.";
pub const SURVEY_REPORT: &str = "Work needing no taste can move.";
// The fake forge's one repository stands as kit, since forge.rs names it,
// until the fixture holds two.
pub const KIT: &str = "acme/app";
pub const OWNER_MODEL: &str = "owner";
pub const OWNER_EFFORT: &str = "xhigh";
// Model and effort numbers are their configuration order, after the default model.
pub const OWNER_MODEL_NUMBER: u8 = 1;
pub const OWNER_EFFORT_NUMBER: u8 = 0;
pub const DRAFT: &str = "docs/drafts/export.md";
pub const DESIGN_DOC: &str = "docs/design/export.md";
pub const OWNERS_DOC: &str = "docs/process/owners.md";
pub const DRAFT_WORDS: &str = "# Export draft\n\nExport the application's data.\n";
pub const OWNERS_WORDS: &str = "# Owners\n\nAn owner designs its draft, reports, then implements it.\n";
pub const DESIGN_WORDS: &str = "# Export\n\nDesigned by A from the draft at its commit.\n";
pub const DESIGN_LINE: &str = "Designed by A from the draft at its commit.";
pub const PERSON: &str = "@case_director Export's draft has landed; schedule it, priority 1.";
pub const A_SPEC_PREFIX: &str = "@case_a Own the draft Export: docs/drafts/export.md at kit's commit ";
pub const A_SPEC_SUFFIX: &str = ". Design it as a change of Markdown alone, send me your design report as a question, and finish on my green light.";
pub const SCHEDULED: &str = "Scheduled: A owns Export at priority 1.";
pub const GREEN_LIGHT: &str = "Green light: go on.";
pub const ANSWERED: &str = "Answered A.";
pub const DIRECTOR_REPORT: &str = "A designed Export and has its green light.";
pub const DESIGN_SPEC: &str = "@case_design Write Export's design at docs/design/export.md, Markdown alone.";
pub const DESIGN_DELEGATED: &str = "Design delegated.";
pub const DESIGN_REPORT: &str =
    "Export's design has landed on kit's main at docs/design/export.md: one Markdown file. Green light to go on?";
pub const ASKED: &str = "Asked the director.";
pub const A_REPORT: &str = "Export's design landed, and the director gave its green light.";
pub const DESIGN_TITLE: &str = "Export's design";
pub const REVIEWED: &str = "Checked";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Director,
    A,
    Design,
    Review,
    Survey,
    Check,
    B,
    Benchmark,
    /// The director whose run crosses a worker restart.
    Hang,
    /// A's next phase, asking that director for its direction.
    Phase,
    Rules,
    Scope,
    Pause,
    Owner,
    Bowner,
    Helper,
    Implementer,
    H1Coder,
    Shape,
    Mover,
    Plan,
    Planner,
    Subowner,
    Misread,
    MisreadHelper,
    Direction,
    StoryOwner,
    OverlapOwner,
    X3Writer,
    PlannedWriter,
    StoryWriter,
    BatchDirector,
    BatchOwner,
    BatchSubowner,
    BatchOne,
    BatchTwo,
    BatchThree,
    BatchStory,
    BlockerDirector,
    BlockerOwner,
    BlockerSubowner,
    BlockerImplementer,
    BlockerCoder,
    // Act 3, step 6.5 (domain/testing.md, 5.3)
    BudgetDirector,
    BudgetOwner,
    BudgetSubowner,
    BudgetInvestigator,
    BudgetAdvisor,
    // 6.6
    WaitingDirector,
    WaitingOwner,
    WaitingKitCoder,
    WaitingAppCoder,
    // 6.4
    RedDirector,
    RedOwner,
    RedCoder,
    RedRepairCoder,
    // Act 3, step 6.7 (domain/testing.md, 5.3)
    CapDirector,
    CapOwner,
    CapSubowner,
    CapImplementer,
    QaDirector,
    QaOwner,
    QaSubowner,
    QaTester,
    QaNext,
    QaCoder,
}
impl Agent {
    #[must_use]
    pub fn cue(self) -> &'static [u8] {
        match self {
            Self::BlockerDirector => b"@case_blocker_director",
            Self::BlockerOwner => b"@case_blocker_owner",
            Self::BlockerSubowner => b"@case_blocker_subowner",
            Self::BlockerImplementer => b"@case_blocker_implementer",
            Self::BlockerCoder => b"@case_blocker_coder",
            // Act 3, step 6.5 (domain/testing.md, 5.3)
            Self::BudgetDirector => b"@case_budget_director",
            Self::BudgetOwner => b"@case_budget_owner",
            Self::BudgetSubowner => b"@case_budget_subowner",
            Self::BudgetInvestigator => b"@case_budget_investigator",
            Self::BudgetAdvisor => b"@case_budget_advisor",
            // 6.6
            Self::WaitingDirector => b"@case_waiting_director",
            Self::WaitingOwner => b"@case_waiting_owner",
            Self::WaitingKitCoder => b"@case_waiting_kit",
            Self::WaitingAppCoder => b"@case_waiting_app",
            // 6.4
            Self::RedDirector => b"@case_red_director",
            Self::RedOwner => b"@case_red_owner",
            Self::RedCoder => b"@case_red_coder",
            Self::RedRepairCoder => b"Repair landing branch CI",
            Self::BatchDirector => b"@case_batch_director",
            Self::BatchOwner => b"@case_batch_owner",
            Self::BatchSubowner => b"@case_batch_subowner",
            Self::BatchOne => b"@case_batch_one",
            Self::BatchTwo => b"@case_batch_two",
            Self::BatchThree => b"@case_batch_three",
            Self::BatchStory => b"@case_batch_story",
            Self::Misread => b"@case_misread_director",
            Self::MisreadHelper => b"@case_misread_helper",
            Self::Direction => b"@case_direction",
            Self::StoryOwner => b"@case_story_owner",
            Self::OverlapOwner => b"@case_overlap_owner",
            Self::X3Writer => b"@case_x3_writer",
            Self::PlannedWriter => b"@case_planned_writer",
            Self::StoryWriter => b"@case_story_writer",
            Self::Plan => b"@case_plan_director",
            Self::Planner => b"@case_planner",
            Self::Subowner => b"@case_subowner",
            Self::Shape => b"@case_shape",
            Self::Mover => b"@case_mover",
            Self::Rules => b"@case_rules",
            Self::Scope => b"@case_scope",
            Self::H1Coder => b"@case_h1",
            Self::Pause => b"@case_pause",
            Self::Owner => b"@case_owner",
            Self::Bowner => b"@case_bowner",
            Self::Helper => b"@case_helper",
            Self::Implementer => b"@case_implementer",
            Self::Hang => b"@case_hang",
            Self::Phase => b"@case_phase",
            Self::Check => b"@case_check",
            Self::B => b"@case_b ",
            Self::Benchmark => b"@case_benchmark",
            Self::Director => b"@case_director",
            Self::A => b"@case_a ",
            Self::Design => b"@case_design",
            Self::Survey => b"@case_survey",
            Self::Review => b"Review this change at the named head",
            // Act 3, step 6.7 (domain/testing.md, 5.3)
            Self::CapDirector => b"@case_cap_director",
            Self::CapOwner => b"@case_cap_owner",
            Self::CapSubowner => b"@case_cap_subowner",
            Self::CapImplementer => b"@case_cap_implementer",
            Self::QaDirector => b"@case_qa_director",
            Self::QaOwner => b"@case_qa_owner",
            Self::QaSubowner => b"@case_qa_subowner",
            Self::QaTester => b"@case_qa_tester",
            Self::QaNext => b"@case_qa_next",
            Self::QaCoder => b"@case_qa_coder",
        }
    }
}

#[must_use]
pub fn kit_tree() -> Box<[api::File]> {
    let mut tree = (forge::fixture().setup)().tree.into_vec();
    for (path, words) in [(DRAFT, DRAFT_WORDS), (OWNERS_DOC, OWNERS_WORDS)] {
        tree.push(api::File { path: path.as_bytes().into(), content: words.as_bytes().into() });
    }
    tree.into_boxed_slice()
}

// The seeded first repository's object spelling; derived again in the test below.
const DRAFT_COMMIT: &str = "910a2dec89025cc1000000000000000100000000000000000000000000000000";

#[must_use]
pub fn draft_commit() -> String {
    DRAFT_COMMIT.into()
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use skein_fake_forge_domain as domain;

    #[test]
    fn draft_identity_matches_the_seeded_repository() {
        let config = temper_fake_git_hosted::fixtures::CONFIG;
        let mut domain = domain::Domain::new(&config, fixtures::SEED);
        let first = domain::repository(&mut domain, &config, (fixture().setup)());
        let object = skein_fake_forge_protocol::routes::object(first);
        let mut words = String::new();
        for byte in object.bytes {
            use std::fmt::Write;
            write!(words, "{byte:02x}").expect("commit hex");
        }
        assert_eq!(draft_commit(), words);
    }
}

#[must_use]
pub fn fixture() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (landing::fixture().setup)();
            setup.tree = kit_tree();
            setup.checks.cue =
                Some(api::Cue::File { path: DESIGN_DOC.as_bytes().into(), green: DESIGN_LINE.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (landing::fixture().configure)(value, agent, forge);
            // The seed retains its authority; the service receives these defaults directly.
            for member in ["charters", "templates", "policies", "rules"] {
                value["engine"].as_object_mut().expect("engine").remove(member);
            }
            fixtures::own_policy(value);
            value["world_settings"]["case"] = true.into();
            value["engine"]["limits"]["overrides"]["people.projects"] = 2.into();
            value["engine"]["limits"]["overrides"]["authority.projects"] = 2.into();
            value["engine"]["limits"]["overrides"]["tasks.funders"] = 4.into();
            // A runs on the director's explicit choice, distinct from the default.
            let mut model = value["engine"]["llm"]["models"][0].clone();
            model["name"] = OWNER_MODEL.into();
            model["efforts"] = json!([OWNER_EFFORT]);
            value["engine"]["llm"]["models"].as_array_mut().expect("models").push(model);
            value["engine"]["llm"]["providers"][0]["models"] = json!(["fake", OWNER_MODEL]);
            let mut model = agent["endpoints"][0]["models"][0].clone();
            model["name"] = OWNER_MODEL.into();
            model["efforts"] = json!([OWNER_EFFORT]);
            agent["endpoints"][0]["models"].as_array_mut().expect("agent models").push(model);
            // The director delegates, messages, amends and keeps project notes.
            // The extra owner adds one delegation level before the change procedure.
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = 391.into();
                project["policy"]["ceiling"]["notes"] = 2.into();
                project["policy"]["roles"][0]["authority"]["notes"] = 2.into();
                project["policy"]["ceiling"]["delegation"]["depth"] = 5.into();
                project["policy"]["roles"][0]["authority"]["delegation"]["depth"] = 5.into();
                project["policy"]["roles"][0]["authority"]["tools"] = 391.into();
            }
            // A tracked owner reserves core news, landing news, an answer and its
            // delegate's result together; the landing fixture's inbox holds 4 KiB.
            value["engine"]["limits"]["overrides"]["tasks.inbox_bytes"] = 16384.into();
            // The director's prescribed final report exceeds the chat's 32 bytes.
            value["world_settings"]["report_words"] = 128.into();
            // The brief includes the full draft commit and its instructions.
            value["engine"]["limits"]["overrides"]["documents.text_bytes"] = 512.into();
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

/// Keep only the director on its worker; delegates retain the usual parking waits.
#[must_use]
pub fn fixture_placed() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // The chat and A both use charter 1 in the base fixture. A waiting
            // run keeps its one slot until a message resumes it, so give only
            // the director a charter whose wait exceeds PATIENCE (10 s).
            value["world_settings"]["second_wait_ms"] = 60000.into();
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 2.into();
            value["world_settings"]["executor"] = 2.into();
            // The person must also be offered the new executor for that chat.
            let mut kinds = value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("executor kinds").push(json!({"kind":"charter","number":2}));
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
            }
            landing::admit(value);
        },
        ..fixture()
    }
}

// Keep two calls, payload 4,096 and output 8,192. Window 262,144 gives
// final A = 1,320,403 and F(A) = 1,320,467, within Small's 1,590,110-byte
// page ceiling; the old 1,048,576-token window required a 1,792,326-byte frame.
fn small_two_call_window(value: &mut Value, agent: &mut Value) {
    for model in value["engine"]["llm"]["models"].as_array_mut().expect("engine models") {
        model["window"] = 262_144.into();
    }
    for model in agent["endpoints"][0]["models"].as_array_mut().expect("agent models") {
        model["window"] = 262_144.into();
    }
}

#[must_use]
pub fn fixture_4_4() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // The survey's delegate and wait share one completion. Its
            // resumed script then cannot collide with the model-change recap.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            small_two_call_window(value, agent);
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
        },
        ..fixture()
    }
}

/// The check keeps all three runs in place, with timer and cancellation authority.
#[must_use]
pub fn fixture_check() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture_placed().configure)(value, agent, forge);
            // One adopted repository contains one pull. Its initial scan and
            // the scripted forge hooks supply the listing changes; repeating
            // the listing every 10 ms adds traffic throughout the timer story.
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            // Branch watches keep their 100 ms slow reconciliation: at most
            // 2 * (PATIENCE / 100 ms + 1) steady reads for the two branches.
            // Subscribe is family 32; cancel shares control (4), already in 391.
            let tools = 0x0187 | 0x0020;
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = tools.into();
                project["policy"]["roles"][0]["authority"]["tools"] = tools.into();
            }
            // This placement story needs ten-unit runs: the smaller model's
            // stock 4C cap is four, winding waiting agents down and freeing a
            // worker before the late benchmark worker is added.
            value["world_settings"]["maximum_run_spend"] = 10.into();
            // Announce placement and wait in one completion, with serial tools.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            small_two_call_window(value, agent);
            agent["profile"]["policy"]["parallel_tools"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            // The opener's checkout needs an LLM grant and an application git
            // grant. One refuses its coder; two fit the three-worker ring.
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            landing::admit(value);
        },
        ..fixture_placed()
    }
}

/// The third-chat scenario starts a fresh deployment; its director is first.
pub const HANG: u64 = 1;
/// A's next phase is this deployment's first delegate.
pub const PHASE: u64 = 2;
/// The person requests the restart scenario.
pub const HANG_PERSON: &str = "@case_hang Delegate A's next phase and answer it once after the workstation goes away.";
/// The brief for A's next phase.
pub const PHASE_SPEC: &str = "@case_phase Own Export's next phase and ask the director for its green light.";
/// A's question retained across the director's retry.
pub const PHASE_QUESTION: &str = "May Export's next phase begin?";
/// The director's single answer.
pub const PHASE_ANSWER: &str = "Begin Export's next phase.";
/// A's completed next-phase report.
pub const PHASE_REPORT: &str = "Export's next phase has its direction.";
/// The committed read barrier before the workstation stops.
pub const HANG_READ: &str = "A's next-phase question is read; its answer is still waiting.";
/// The restarted director's final report.
pub const HANG_REPORT: &str = "The restarted director answered A and its next phase reported.";
/// The first question in this fresh deployment, checked in the read and answer.
pub const PHASE_QUESTION_NUMBER: u64 = 1;

/// The base case's one-slot workers, owner model, notes and placed director.
/// No repository adoption, landing policy, protected tree or check scripts are needed.
#[must_use]
pub fn fixture_4_6() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture_placed().configure)(value, agent, forge);
            value["application"]["landing"]["projects"] = json!([]);
            // The retry path is the same, only its wait is short; the backoff is not what this story tests.
            value["engine"]["limits"]["overrides"]["tasks.retries.agent.base_ms"] = 1.into();
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 2, step 5.1 (domain/testing.md, 5.3)
pub const RULES: u64 = 1;
pub const SCOPE: u64 = 2;
pub const SCOPE_QUESTION_NUMBER: u64 = 2;
pub const RULES_HOLD_REVISION: u64 = 1;
pub const RULES_PERSON: &str = "@case_rules Delegate A to ask about x3's scope and wait for its report.";
pub const SCOPE_SPEC: &str = "@case_scope Own x3's scope; ask the director and report on its answer.";
pub const SCOPE_QUESTION: &str = "What is x3's scope in the next milestone?";
pub const SCOPE_ANSWER: &str = "x3 owns the next milestone's design and its defined end.";
pub const RULES_DELEGATED: &str = "A owns the scope question.";
pub const RULES_ANSWERED: &str = "A has its scope answer.";
pub const SCOPE_ASKED: &str = "The scope question awaits the director.";
pub const SCOPE_REPORT: &str = "x3's scope has its direction.";
pub const RULES_REPORT: &str = "A reported x3's scope under the new process.";
pub const SCOPE_QUESTION_HOLD: &[u8] = b"case-scope-question";
pub const RULES_FILE: &str = "AGENTS.md";
pub const DROPPED_RULE: &str = "Owners check shared version numbers before they land.";
pub const OLD_RULES: &str = "# Process\n\nOwners check shared version numbers before they land.\n";
pub const NEW_RULES: &str = "# Process\n\nAn owner takes one mission, at most four milestones, then stops.\nMilestones take 2-3 hours under a 4-hour cap, with a sub-owner each.\nThe plan comes first. Start a fresh session at each milestone's end.\nRun a QA pass beside the next milestone.\n";

#[must_use]
pub fn fixture_5_1() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (forge::fixture().setup)();
            let mut tree = setup.tree.into_vec();
            tree.push(api::File { path: RULES_FILE.as_bytes().into(), content: OLD_RULES.as_bytes().into() });
            setup.tree = tree.into_boxed_slice();
            setup
        },
        configure: |value, agent, forge| {
            (fixture_placed().configure)(value, agent, forge);
            value["world_settings"]["second_resume"] = false.into();
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 2, step 5.3 (domain/testing.md, 5.3)
pub const SHAPE: u64 = 1;
pub const MOVER: u64 = 2;
pub const MOVER_QUESTION_NUMBER: u64 = 1;
pub const MOVER_AMENDMENT_REVISION: u64 = 1;
pub const SHAPE_PERSON: &str = "@case_shape Help A move Export to the new shape; amend its instructions when it asks.";
pub const MOVER_SPEC: &str = "@case_mover Own Export; ask the director how to go on under the new shape and report.";
pub const OLD_BRIEF: &str = "Implement Export in one long session.";
pub const MOVER_QUESTION: &str = "How do I go on under the new shape?";
pub const NEW_BRIEF: &str = "x3 finishes the old way, outside any milestone, under the person's two-hour cap; if it doesn't finish, we evaluate together. Then make the milestone plan first.";
pub const SHAPE_REASON: &str = "Continue x3 under the cap, then plan the milestones.";
pub const SHAPE_DELEGATED: &str = "A owns Export under its old brief.";
pub const SHAPE_AMENDED: &str = "A's instructions now answer its question.";
pub const MOVER_ASKED: &str = "The new-shape question awaits the director.";
pub const MOVER_REPORT: &str = "x3 will finish under the cap; the milestone plan comes next.";
pub const SHAPE_REPORT: &str = "A reported under its amended instructions.";

#[must_use]
pub fn fixture_5_3() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            value["world_settings"]["owner_brief"] = true.into();
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 2.into();
            let mut kinds = value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("executor kinds").push(json!({"kind":"charter","number":2}));
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
            }
            // Neither task mounts a repository or delegates a change.
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 2, step 5.2 (domain/testing.md, 5.3)
// The chat delegates A, the helper, then B; only B creates descendants.
// B delegates its implementer, then h1, which creates its coder and reviewer.
pub const PAUSE_WORKERS: &[&str] = &["ws", "s1", "s2"];
pub const PAUSE: u64 = 1;
pub const OWNER: u64 = 2;
pub const HELPER: u64 = 3;
pub const HELPER_CHARTER: u64 = 3;
pub const BOWNER: u64 = 4;
pub const IMPLEMENTER: u64 = 5;
pub const H1: u64 = 6;
pub const H1_CODER: u64 = 7;
pub const H1_REVIEWER: u64 = 8;
pub const B_HOLD: u64 = 1;
pub const PAUSE_PERSON: &str = "@case_pause Pause every owner but A; I'll go through each one's work and say what resumes. Stop the old shape's leftovers.";
pub const OWNER_SPEC: &str = "@case_owner A remains free to report.";
pub const BOWNER_SPEC: &str = "@case_bowner Delegate your implementer and h1, then await their endings.";
pub const HELPER_SPEC: &str = "@case_helper Wait on the old shape's charter.";
pub const IMPLEMENTER_SPEC: &str = "@case_implementer Finish your running work when released.";
pub const H1_SPEC: &str = "@case_h1 Write docs/design/h1.md, Markdown alone.";
pub const PAUSE_PROPOSAL: &str = "I propose stopping B; the person must stop it.";
pub const HELPER_REASON: &str = "The person has stopped the old shape's leftovers; cancel this helper.";
pub const IMPLEMENTER_RESULT: &str = "B's implementer finished its running work.";
pub const OWNER_REPORT: &str = "A remained free and finished.";
pub const BOWNER_REPORT: &str = "B received its implementer's result and h1's landing after release.";
pub const PAUSE_REPORT: &str = "B reported after release; A finished and the old helper was cancelled.";
pub const H1_DOC: &str = "docs/design/h1.md";
pub const H1_WORDS: &str = "# h1\n\nB's change went on while B was stopped.\n";
pub const H1_LINE: &str = "B's change went on while B was stopped.";
pub const IMPLEMENTER_HOLD: &[u8] = b"case-5-2-implementer";
pub const REVIEW_HOLD: &[u8] = b"case-5-2-review";

#[must_use]
pub fn fixture_5_2() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (fixture().setup)();
            setup.checks.cue =
                Some(api::Cue::File { path: H1_DOC.as_bytes().into(), green: H1_LINE.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (fixture_placed().configure)(value, agent, forge);
            // One host call for each of the three one-slot workers.
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            // Initial adoption and forge hooks supply the listing changes,
            // as in fixture_check; repeating the listing adds no evidence.
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            value["world_settings"]["helper"] = true.into();
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 3.into();
            let mut kinds = value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("kinds").push(json!({"kind":"charter","number":HELPER_CHARTER}));
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
                // The chat's run, A, the helper and B reserve 10 + 5 + 5 + 25.
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 50.into();
            }
            // The coder's checkout needs both its LLM and application git grants,
            // as in fixture_check. The cluster supplies one slot per worker.
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 2, step 5.5 (domain/testing.md, 5.3)
pub const PLAN_DIRECTOR: u64 = 1;
pub const PLANNER: u64 = 2;
pub const SUBOWNER: u64 = 3;
pub const PLAN_QUESTION_NUMBER: u64 = 2;
pub const PLAN_ANSWER_NUMBER: u64 = 3;
pub const PLAN_NOTE_NUMBER: u64 = 1;
pub const A_DEADLINE: u64 = 4_102_444_800_000_000_000;
pub const A1_DEADLINE: u64 = A_DEADLINE - 14_400_000_000_000;
pub const PLAN_ANSWER_PENDING: &str = "Wait for A's first run to park before answering.";
pub const PLAN_ANSWER_HOLD: &[u8] = b"case-5-5-answer";
pub const PLAN_PERSON: &str = "@case_plan_director Give A the mission's deadline and answer its plan.";
pub const PLANNER_SPEC: &str =
    "@case_planner Plan the nearer mission, ask the director, and delegate M1's sub-owner after its green light.";
pub const SUBOWNER_SPEC: &str =
    "@case_subowner Own M1 under its deadline; try to delegate a helper and report the refusal.";
pub const PLAN_NOTE: &str = "Export's nearer mission";
pub const PLAN_WORDS: &str = "A nearer end: this mission ends at the end-to-end story; the rest of Export is a later mission. M1: the path through app and kit's fake. M2: the rest of the story. Each takes 2 to 3 hours. May I use B's files?";
pub const PLAN_ANSWER: &str =
    "Green light: the nearer end, M1 and M2 approved; every ask granted, including B's files.";
pub const SUBOWNER_REFUSAL: &str = "Authority needs proposal: authority ceiling; ";
/// What the refusal adds when the batch also asks more spend than A1 has left.
pub const SPEND_DEFICIT: &str = "spend allowance; ";
pub const SUBOWNER_REPORT: &str = "M1's sub-owner could not delegate its own helper.";
pub const PLANNER_REPORT: &str = "The recalled plan started M1's sub-owner under its deadline; it could not delegate.";
pub const PLAN_REPORT: &str = "A's answered plan and M1's sub-owner have reported.";

#[must_use]
pub fn fixture_5_5() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            value["world_settings"]["second_resume"] = false.into();
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 2.into();
            let mut kinds = value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("kinds").push(json!({"kind":"charter","number":2}));
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
            }
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 2, step 5.6 (domain/testing.md, 5.3)
pub const MISREAD_DIRECTOR: u64 = 1;
pub const MISREAD_HELPER: u64 = 2;
pub const MISREAD_TIMER: u64 = 1;
pub const MISREAD_CORRECTION_MESSAGE: u64 = 2;
pub const MISREAD_TIMER_AT: u64 = 4_102_444_800_000_000_000;
pub const MISREAD_PERSON: &str = "@case_misread_director Priorities for information: 1, the end-to-end story on fakes; 2, real backends; s1 and s2 preferred for priority 1.";
pub const MISREAD_HELPER_SPEC: &str = "@case_misread_helper Act on priority 1: the end-to-end story on fakes.";
pub const MISREAD_CORRECTION: &str = "that was just informative; don't act on it";
pub const MISREAD_REASON: &str =
    "The person's priorities were for information, not instructions to act; cancel this helper.";
pub const MISREAD_NOTE: &str = "Information is not an instruction";
pub const MISREAD_RULE: &str =
    "Priorities supplied for information are not instructions. Do not act on them unless the person asks.";
pub const MISREAD_REPORT: &str =
    "The misread helper was cancelled before landing anything, and the recalled rule is kept in the project's notes.";
pub const MISREAD_HELPER_HOLD: &[u8] = b"case-5-6-helper";
pub const MISREAD_RECALL_HOLD: &[u8] = b"case-5-6-recall";
pub const MISREAD_HELPER_PENDING: &str = "The helper's first turn is held.";
pub const MISREAD_RULE_PENDING: &str = "The rule is written; recall waits for the helper's cancellation.";

#[must_use]
pub fn fixture_5_6() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            value["world_settings"]["resume"] = false.into();
            // Subscribe is family 32; read is 512 for the unchanged-main check.
            let tools = 0x0187 | 0x0020 | 0x0200;
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = tools.into();
                project["policy"]["roles"][0]["authority"]["tools"] = tools.into();
            }
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 2, step 5.4 (domain/testing.md, 5.3)
pub const DIRECTION: u64 = 1;
pub const STORY_OWNER: u64 = 2;
pub const OVERLAP_OWNER: u64 = 3;
pub const X3: u64 = 4;
pub const X3_PULL_NUMBER: u64 = 1;
pub const STORY_PULL_NUMBER: u64 = 2;
pub const X3_CODER: u64 = 5;
pub const X3_REVIEWER: u64 = 6;
pub const PLANNED: u64 = 7;
pub const PLANNED_CODER: u64 = 8;
pub const STORY_CHANGE: u64 = 9;
pub const STORY_CODER: u64 = 10;
pub const STORY_REVIEWER: u64 = 11;
pub const DIRECTION_REVISION: u64 = 1;
pub const OVERLAP_HOLD_REVISION: u64 = 1;
// Current-head CI subscriptions and news also use fresh message identities.
pub const DEFERRED_NOTE_NUMBER: u64 = 9;
pub const DIRECTION_PERSON_MESSAGE: u64 = 7;
pub const DIRECTION_AMENDMENT_MESSAGE: u64 = 8;
pub const OVERLAP_SUBSCRIPTION: u64 = 5;
pub const OVERLAP_RECHECK_ATTEMPT: u64 = 3;
pub const DIRECTION_PERSON: &str = "@case_direction Delegate A and B as owners on kit and wait for my direction.";
pub const DIRECTION_MESSAGE: &str = "The end-to-end story first; cut it to one path; placeholders are fine. Stop x3. B's files are A's where the story needs them.";
pub const DIRECTION_BRIEF: &str = "The end-to-end story first, one path with placeholders. Stop x3 and defer its plan in a project note. B's files are A's where the story needs them.";
pub const DIRECTION_REASON: &str =
    "The person wants the end-to-end story first; stop x3 and use B's files where needed.";
pub const STORY_OWNER_SPEC: &str =
    "@case_story_owner Own A's work: delegate x3, then await the director's new instructions.";
pub const OVERLAP_OWNER_SPEC: &str =
    "@case_overlap_owner Own B's work: delegate planned h1 on B's file, then wait for news.";
pub const X3_SPEC: &str = "@case_x3_writer Write docs/design/x3.md, Markdown alone.";
pub const PLANNED_SPEC: &str = "@case_planned_writer Plan a change to B's file; wait before coding it.";
pub const STORY_SPEC: &str = "@case_story_writer Write the end-to-end story in docs/design/b.md, Markdown alone.";
pub const X3_FILE: &str = "docs/design/x3.md";
pub const X3_WORDS: &str = "# x3\n\nThe old plan, deferred.\n";
pub const B_FILE: &str = "docs/design/b.md";
pub const B_WORDS: &str = "# B\n\nA placeholder before the story.\n";
pub const STORY_WORDS: &str = "# B\n\nThe end-to-end story takes one path through B's file.\n";
pub const STORY_GREEN: &str = "The end-to-end story takes one path through B's file.";
pub const X3_CANCEL_REASON: &str = "Defer x3: the person's end-to-end story comes first.";
pub const PLANNED_CANCEL_REASON: &str = "A's landing overlaps h1; cancel the plan to go again from the new main.";
pub const DEFERRED_NOTE: &str = "x3's deferred plan";
pub const DEFERRED_WORDS: &str =
    "Task 4 (x3) is deferred under its closed pull; return to its old plan after the end-to-end story.";
pub const STORY_OWNER_REPORT: &str = "The story landed; x3's deferred plan was recalled.";
pub const OVERLAP_OWNER_REPORT: &str = "B heard A's overlapping landing after release and cancelled h1.";
pub const DIRECTION_REPORT: &str = "A's story landed and B reported after release.";
pub const DIRECTION_HELD_WORDS: &str = "Waiting at the story's event.";
pub const B_DELEGATE_HOLD: &[u8] = b"case-5-4-b-delegate";
pub const X3_REVIEW_HOLD: &[u8] = b"case-5-4-x3-review";
pub const PLANNED_CODING_HOLD: &[u8] = b"case-5-4-h1-code";
pub const STORY_RECALL_HOLD: &[u8] = b"case-5-4-recall";

#[must_use]
pub fn fixture_5_4() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (fixture().setup)();
            let mut tree = setup.tree.into_vec();
            tree.push(api::File { path: B_FILE.as_bytes().into(), content: B_WORDS.as_bytes().into() });
            setup.tree = tree.into();
            setup.checks.cue = Some(api::Cue::File { path: B_FILE.as_bytes().into(), green: b"# B".as_slice().into() });
            setup
        },
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // Delegate/amend/cancel and their wait share one LLM completion.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            small_two_call_window(value, agent);
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            // Read all three spec parameters, including the planned path (name 6).
            value["engine"]["limits"]["overrides"]["documents.lines"] = 3.into();
            // The director's two-owner batch was refused: "batch limit".
            value["engine"]["limits"]["overrides"]["tasks.batch"] = 2.into();
            // Then "task 2 goal limit": both owners must be tracked.
            value["engine"]["limits"]["overrides"]["tasks.project_goals"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            // "Authority refused: task allowance": the tree has eleven tasks,
            // two owners plus A's six descendants and B's two, under the chat.
            value["engine"]["limits"]["overrides"]["tasks.tree_tasks"] = 11.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["tasks"] = 11.into();
                project["policy"]["roles"][0]["authority"]["delegation"]["tasks"] = 11.into();
            }
            // The director was told "spend allowance": its run reserves ten,
            // and A and B reserve thirty and twenty for their subtrees.
            value["world_settings"]["cap"] = 60.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 60.into();
            }
            // Adoption and the fake forge's hooks supply listing changes (5.2).
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 3, step 6.1 (domain/testing.md, 5.3)
pub const BATCH_DIRECTOR: u64 = 1;
pub const BATCH_OWNER: u64 = 2;
pub const BATCH_SUBOWNER: u64 = 3;
pub const BATCH_ONE: u64 = 4;
pub const BATCH_TWO: u64 = 5;
pub const BATCH_THREE: u64 = 6;
pub const BATCH_STORY: u64 = 7;
pub const BATCH_QUESTION_NUMBER: u64 = 1;
pub const BATCH_PERSON: &str = "@case_batch_director Delegate A to own M1 and wait for its report.";
pub const BATCH_ANSWER_NUMBER: u64 = 2;
pub const BATCH_QUESTION: &str = "M1's design reports five departures. May I send the implementers and the story?";
pub const BATCH_ANSWER: &str =
    "Green light with three amendments: keep one path; name the dependencies; improve the explanation if cheap.";
pub const BATCH_WRITING: &str = "The batch delegate is writing.";
pub const BATCH_HOLDS: [&[u8]; 4] = [b"case-6-1-one", b"case-6-1-two", b"case-6-1-three", b"case-6-1-story"];
pub const BATCH_REPORTS: [&str; 4] = ["a1 implemented.", "a2 implemented.", "a3 implemented.", "a4's story passed."];
pub const BATCH_SUBOWNER_REPORT: &str = "All four batch delegates reported.";
pub const BATCH_OWNER_REPORT: &str = "M1's sub-owner reported the batch.";
pub const BATCH_DIRECTOR_REPORT: &str = "A reported M1's batch.";

#[must_use]
pub fn fixture_6_1() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // A four-entry batch in one task record needs 3,693,669 B,
            // past small's 1,567,829 B store value; size one profile up.
            value["engine"]["limits"]["profile"] = "standard".into();
            value["workers"][0]["limits"]["profile"] = "standard".into();
            // The standard engine needed for a four-entry batch also needs
            // matching application room: 128 retained calls; 8 workers *
            // fleet.max_out (137) + 1 deliveries; three held per delivery;
            // and route_bound (14,162) + 128 journal writes. Held-revision
            // writes and the 32 retained forge answers contribute to this bound.
            // A1's task read needs documents.lines = 4 for its complete live delegates.
            value["application"]["limits"]["overrides"]["forge.named_calls"] = 128.into();
            value["application"]["limits"]["overrides"]["root.journal.deliveries"] = 1097.into();
            value["application"]["limits"]["overrides"]["root.journal.held"] = 3291.into();
            value["application"]["limits"]["overrides"]["root.journal.writes"] = 14290.into();
            // All owners, including the chat, park on the ordinary charter's
            // wait. That releases the three slots for the three implementers.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            // A1 sends four entries in one batch, within the seven-task tree.
            for name in ["tasks.batch", "authority.batch"] {
                value["engine"]["limits"]["overrides"][name] = 4.into();
            }
            value["engine"]["limits"]["overrides"]["tasks.dependencies"] = 2.into();
            value["engine"]["limits"]["overrides"]["documents.lines"] = 4.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            // Each admitted run reserves ten; A1 also reserves forty for its
            // delegates after spending one on its question. A and the director
            // each reserve ten more.
            value["world_settings"]["cap"] = 71.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 71.into();
            }
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 3, step 6.2 (domain/testing.md, 5.3)
// Fresh deployment: director, A, A1, a3, e1, coder, reviewer. The oracle
// checks these numbers, message numbers and revisions against documents.
pub const BLOCKER_DIRECTOR: u64 = 1;
pub const BLOCKER_OWNER: u64 = 2;
pub const BLOCKER_SUBOWNER: u64 = 3;
pub const BLOCKER_A3: u64 = 4;
pub const BLOCKER_E1: u64 = 5;
pub const BLOCKER_CODER: u64 = 6;
pub const BLOCKER_REVIEWER: u64 = 7;
pub const BLOCKER_FINDING_NUMBER: u64 = 1;
pub const BLOCKER_QUESTION_NUMBER: u64 = 2;
pub const BLOCKER_UP_QUESTION_NUMBER: u64 = 3;
pub const BLOCKER_UP_ANSWER_NUMBER: u64 = 4;
pub const BLOCKER_ANSWER_NUMBER: u64 = 5;
pub const BLOCKER_AMENDMENT_NUMBER: u64 = 7;
pub const BLOCKER_REVISION: u64 = 1;
pub const BLOCKER_PULL: u64 = 1;
pub const BLOCKER_PERSON: &str = "@case_blocker_director Delegate A to own M1's blocker and wait for its report.";
pub const BLOCKER_FINDING: &str = "Kit's fake answers wrongly; a3 needs B's fake fixed.";
pub const BLOCKER_QUESTION: &str =
    "May A1 change the six public signatures of B's fake? Nothing outside the crate calls them; B is stopped.";
pub const BLOCKER_UP_QUESTION: &str =
    "A asks the director: may A1 change B's stopped public API, six signatures with no callers outside the crate?";
pub const BLOCKER_UP_ANSWER: &str = "Director's answer: allow A1's six-signature fix on B's stopped API.";
pub const BLOCKER_ANSWER: &str = "A's answer: the director allows the six-signature fix; delegate e1 now.";
pub const BLOCKER_REASON: &str = "a3 now depends on e1's landing to fix B's fake.";
pub const BLOCKER_E1_SPEC: &str =
    "@case_blocker_coder Fix B's fake in src/fake.rs; change six public signatures, with no callers outside the crate.";
pub const BLOCKER_FILE: &str = "src/fake.rs";
pub const BLOCKER_WRONG: &str = "// B's fake\n// Six public signatures answer wrongly.\n";
pub const BLOCKER_FIXED: &str = "// B's fake\n// Six public signatures answer correctly.\n";
pub const BLOCKER_GREEN: &str = "// B's fake";
pub const BLOCKER_KEPT_REPORT: &str = "a3's turn is done, awaiting the fix's landing.";
pub const BLOCKER_A3_REPORT: &str = "a3 continued after e1 landed and reports.";
pub const BLOCKER_SUBOWNER_REPORT: &str = "A1 reports a3 continued after the blocker fix landed.";
pub const BLOCKER_OWNER_REPORT: &str = "A reports A1 resolved the blocker.";
pub const BLOCKER_DIRECTOR_REPORT: &str = "The director reports the blocker resolved.";
pub const BLOCKER_REVIEW_HOLD: &[u8] = b"case-6-2-review";

#[must_use]
pub fn fixture_6_2() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (fixture().setup)();
            let mut tree = setup.tree.into_vec();
            tree.push(api::File { path: BLOCKER_FILE.as_bytes().into(), content: BLOCKER_WRONG.as_bytes().into() });
            setup.tree = tree.into();
            setup.checks.cue =
                Some(api::Cue::File { path: BLOCKER_FILE.as_bytes().into(), green: BLOCKER_GREEN.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // Small's four store joins cannot read a3's three ancestors,
            // dependency and terminal result, nor the coder/reviewer's four
            // ancestors and terminal result. Joins are profile-only: standard
            // gives sixteen, and matching worker channel terms. Its 128
            // retained calls need forge.named_calls 128; eight workers *
            // fleet.max_out 137 + one gives 1097 deliveries, three held each.
            value["engine"]["limits"]["profile"] = "standard".into();
            value["workers"][0]["limits"]["profile"] = "standard".into();
            value["application"]["limits"]["overrides"]["forge.named_calls"] = 128.into();
            value["application"]["limits"]["overrides"]["root.journal.deliveries"] = 1097.into();
            value["application"]["limits"]["overrides"]["root.journal.held"] = 3291.into();
            // Only a3 keeps its waiting run, on charter 2, so the live
            // amendment reaches it before its done turn is saved as kept.
            value["world_settings"]["second_wait_ms"] = 60000.into();
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 2.into();
            let mut kinds = value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("kinds").push(json!({"kind":"charter","number":2}));
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
            }
            // Singleton batches. A depth 4, A1 depth 3 and
            // e1 depth 2 admit the coder/reviewer tree; a3 has depth 0.
            // Two calls let a delegate/message and its wait park together.
            // Three fleet calls admit the three one-slot PAUSE_WORKERS.
            // Three document lines expose e1's resource, branch and path.
            // The seven-task tree fits the base eight-task allowance.
            // Spend reserves ten per admitted run: A1 32 (a3/e1 plus its
            // own reserve after two turns), A 42, director 52.
            // Deployment depth 6 leaves the director 5, A 4 and A1 3.
            // hub.accounts 2 carries the LLM and checkout git account.
            // CI adoption/hooks supply listing changes; poll_ms and
            // poll_max_ms 20001 avoid redundant polls (as 5.4).
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            value["engine"]["limits"]["overrides"]["documents.lines"] = 3.into();
            value["world_settings"]["cap"] = 52.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 52.into();
            }
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["depth"] = 6.into();
                project["policy"]["roles"][0]["authority"]["delegation"]["depth"] = 6.into();
            }
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 3, step 6.5 (domain/testing.md, 5.3)
pub const BUDGET_DIRECTOR: u64 = 1;
pub const BUDGET_OWNER: u64 = 2;
pub const BUDGET_SUBOWNER: u64 = 3;
pub const BUDGET_A5: u64 = 4;
pub const BUDGET_ADVISOR: u64 = 5;
pub const BUDGET_QUESTION_NUMBER: u64 = 1;
pub const BUDGET_UP_QUESTION_NUMBER: u64 = 2;
pub const BUDGET_PERSON_ANSWER_NUMBER: u64 = 3;
pub const BUDGET_RELAY_NUMBER: u64 = 4;
pub const BUDGET_ANSWER_NUMBER: u64 = 5;
pub const BUDGET_FINDING_NUMBER: u64 = 6;
pub const BUDGET_AMENDMENT_NUMBER: u64 = 7;
pub const BUDGET_A5_REPORT_NUMBER: u64 = 8;
pub const BUDGET_ADVISOR_REPORT_NUMBER: u64 = 9;
pub const BUDGET_REVISION: u64 = 1;
pub const BUDGET_MEDIUM_NUMBER: u8 = 0;
pub const BUDGET_HIGH_NUMBER: u8 = 1;
pub const BUDGET_MEDIUM: &str = "medium";
pub const BUDGET_HIGH: &str = "high";
pub const BUDGET_PERSON: &str = "@case_budget_director Delegate A to own M1's budget question and wait for its report.";
pub const BUDGET_QUESTION: &str =
    "A1 asks A: a4's story takes 70 s against its suite's 60 s. May the budget be raised?";
pub const BUDGET_UP_QUESTION: &str =
    "A asks the director: may a4's story take 70 s against its suite's 60 s? Recommend no raise, make it cheaper.";
pub const BUDGET_PERSON_ANSWER: &str = "The person agrees: no raise, make it cheaper.";
pub const BUDGET_ANSWER: &str = "A answers A1: the person allows no raise; make the story cheaper.";
pub const BUDGET_FINDING: &str = "a5 found no cause at medium; evidence: the story's repeated document reads.";
pub const BUDGET_REASON: &str = "Try the investigation again at high effort, using the repeated-read evidence.";
pub const BUDGET_A5_REPORT: &str = "a5 found none again at high; evidence: repeated document reads dominate the story.";
pub const BUDGET_ADVISOR_SPEC: &str = "@case_budget_advisor Read a5's report and evidence in this brief, name the cause, and report once: a5 found none again at high; evidence: repeated document reads dominate the story.";
pub const BUDGET_ADVISOR_REPORT: &str = "The cause is redundant document polling; await the event and read once.";
pub const BUDGET_SUBOWNER_REPORT: &str = "A1 reports the advisor named the cause of a4's cost.";
pub const BUDGET_OWNER_REPORT: &str = "A reports M1's budget question answered and its investigation advised.";
pub const BUDGET_DIRECTOR_REPORT: &str = "The director reports A resolved the budget question.";
pub const BUDGET_AMEND_HOLD: &[u8] = b"case-6-5-amend";
pub const BUDGET_HIGH_HOLD: &[u8] = b"case-6-5-high";

#[must_use]
pub fn fixture_6_5() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            value["engine"]["llm"]["models"][0]["efforts"] = json!([BUDGET_MEDIUM, BUDGET_HIGH]);
            agent["endpoints"][0]["models"][0]["efforts"] = json!([BUDGET_MEDIUM, BUDGET_HIGH]);
            // Small profile; default engine/agent model efforts [medium, high].
            // Singleton delegations, five tasks, parked owners and investigator.
            // Two calls carry a tool and wait in one completion; three fleet
            // calls admit PAUSE_WORKERS. Spend: a5/advisor 10 each; A1 25
            // (3 spent itself + a5's 2 + 10 advisor + 10 run reserve),
            // A 35 (25 + 10 run reserve), director 45 (35 + 10 reserve).
            // The base eight-task allowance holds this five-task tree.
            // landing.projects is empty: no repository or change is adopted.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            small_two_call_window(value, agent);
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            value["world_settings"]["cap"] = 45.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 45.into();
            }
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 3, step 6.4 (domain/testing.md, 5.3)
pub const RED_DIRECTOR: u64 = 1;
pub const RED_OWNER: u64 = 2;
pub const RED_CHANGE: u64 = 3;
pub const RED_CODER: u64 = 4;
pub const RED_REVIEW: u64 = 5;
pub const RED_REPAIR: u64 = 6;
pub const RED_REPAIR_CODER: u64 = 7;
pub const RED_REPAIR_REVIEW: u64 = 8;
pub const RED_UPDATED_REVIEW: u64 = 9;
pub const RED_FIRST_HEAD: u64 = 2;
pub const RED_UPDATED_HEAD: u64 = 6;
pub const RED_A4_PULL: u64 = 1;
pub const RED_REPAIR_PULL: u64 = 2;
// a4's current-head CI subscription and news precede the person's direction.
pub const RED_TIMER: u64 = 8;
pub const RED_PERSON_MESSAGE: u64 = 7;
pub const RED_REVISION: u64 = 0;
pub const RED_TIMER_AT: u64 = 4_102_444_800_000_000_000;
pub const RED_PERSON: &str = "@case_red_director Delegate A1 to own a4 and report its landing.";
pub const RED_OWNER_SPEC: &str = "@case_red_owner Own a4 as a goal; delegate its change and report.";
pub const RED_CHANGE_SPEC: &str = "@case_red_coder Write a4 in docs/a4.md, leaving the CI cue in docs/story.md green.";
pub const RED_MESSAGE: &str = "Kit's tip is red. Rule that a4 fixes it or an interim fix lands within ninety minutes.";
pub const RED_RULE: &str = "a4 fixes it or an interim fix lands within ninety minutes.";
pub const RED_OWNER_REPORT: &str = "A1 reports a4 landed after the deployment repair.";
pub const RED_REPORT: &str = "The interim repair and a4 landed; the timer is removed.";
pub const RED_FILE: &str = "docs/story.md";
pub const RED_GREEN: &str = "# Story green";
pub const RED_BASE: &str = "# Story green\n\nBase story.\n";
pub const RED_BROKEN: &str = "# Story red\n\nBase story.\n";
pub const RED_A4_FILE: &str = "docs/a4.md";
pub const RED_A4: &str = "# A4\n\nA4's story.\n";
pub const RED_REVIEW_HOLD: &[u8] = b"case-6-4-review";
pub const RED_REPAIR_HOLD: &[u8] = b"case-6-4-repair";

pub const RED_OWNER_SPEND: u64 = 16;
pub const RED_CHANGE_SPEND: u64 = 6;

#[must_use]
pub fn fixture_6_4() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (fixture().setup)();
            let mut tree = setup.tree.into_vec();
            tree.push(api::File { path: RED_FILE.as_bytes().into(), content: RED_BASE.as_bytes().into() });
            setup.tree = tree.into();
            setup.checks.cue =
                Some(api::Cue::File { path: RED_FILE.as_bytes().into(), green: RED_GREEN.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // Small, with each capacity/allowance below checked by a passing
            // whole story and a failing one-less probe in a fresh process.
            // calls_per_response 1 admits the aggregate turn; hub.run_calls
            // 1 serializes its paired calls. hub.accounts 2 carries LLM/git;
            // fleet.calls 1 services worker requests serially; documents.lines
            // 1 still carries every field this oracle reads. Live tasks and
            // project tasks need 5 and 5; tree tasks 6 retains the root's descendants.
            // a4's allowance 6 funds its coder and two exact-head reviews;
            // A1's 16 adds its ten-unit run reservation, and the director's
            // 26 adds ten again. Role spend 26, role period spend 26, and
            // project period spend 31 leave 5 for the deployment's repair.
            // A1 allows 4 descendants and a4 3; one repair suffices (repairs
            // 1). Tools 423 = 391 | 32 adds timer subscription tools, with
            // 5.4's six forge grants unchanged. Forge statuses 3 (State::new)
            // retain a4's head, the red tip and repair head; 2 omits repair CI.
            // Listing polls stay PATIENCE + 1 ms (20001): adoption/hooks supply
            // changes. This timing bound and every oracle bound stay unchanged.
            agent["profile"]["declared"]["calls_per_response"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = 1.into();
            value["engine"]["limits"]["overrides"]["documents.lines"] = 1.into();
            value["world_settings"]["cap"] = 26.into();
            let tools = 0x0187 | 0x0020;
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = tools.into();
                project["policy"]["roles"][0]["authority"]["tools"] = tools.into();
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 26.into();
                project["policy"]["roles"][0]["period_spend"] = 26.into();
                project["policy"]["period_spend"] = 31.into();
            }
            for (name, size) in [("tasks.tasks", 5), ("tasks.project_tasks", 5), ("tasks.tree_tasks", 6)] {
                value["engine"]["limits"]["overrides"][name] = size.into();
            }
            value["application"]["limits"]["overrides"]["forge.change_policy.repairs"] = 1.into();
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

// Act 3, step 6.7 (domain/testing.md, 5.3)
pub const CAP_DIRECTOR: u64 = 1;
pub const CAP_OWNER: u64 = 2;
pub const CAP_SUBOWNER: u64 = 3;
pub const CAP_A5: u64 = 4;
pub const CAP_REVISION: u64 = 1;
pub const CAP_HOLD_REVISION: u64 = 1;
pub const CAP_RELEASE_DECISION: u64 = 2;
// A1 making to a5 begin: 10 isolated trials max 99,528,732 ns;
// 3 concurrent fuzzy suites max 113,037,615 ns (3 serial suites: 99,921,661 ns).
// Twice the largest, 226,075,230 ns, rounded up to whole seconds (at least 1): 1 s.
pub const CAP_SECONDS: u64 = 1;
pub const CAP_PERSON: &str = "@case_cap_director Delegate A to own M1 and decide its cap without asking me.";
pub const CAP_SPLIT: &str = "What has landed is M1; a5's fix opens M2. Report the split once a5 has ended.";
pub const CAP_REASON: &str = "Split M1 at its cap instead of granting a grace to its delegates.";
pub const CAP_A5_REPORT: &str = "a5's fix finished within its original running attempt.";
pub const CAP_SUBOWNER_REPORT: &str = "A1 reports the split: what has landed is M1; a5's fix opens M2.";
pub const CAP_OWNER_REPORT: &str = "A reports A1's milestone split and a5's ending.";
pub const CAP_DIRECTOR_REPORT: &str = "The director split M1 by amendment and release without asking the person.";
pub const CAP_A5_HOLD: &[u8] = b"case-6-7-a5";
pub const CAP_PASS_HOLD: &[u8] = b"case-6-7-pass";
pub const CAP_SPLIT_HOLD: &[u8] = b"case-6-7-split";

#[must_use]
pub fn fixture_6_7() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // Small profile, four tasks in singleton batches (base allowance 8).
            // Two calls admit tool + parked wait, three fleet calls admit the
            // cluster's workers. Budget 64 admits the widening: 1 director completion + A's
            // 33 reserved + director run reserve 10 + A1's 20 unspent
            // funded anew by the director. A needs 33 = its delegation's
            // 1 spent + A1's 22 + run reserve 10. A1 needs 22 = a5's 10
            // + one opening completion + next run reserve 10, with one spare. No landing projects are used.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            small_two_call_window(value, agent);
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            value["world_settings"]["cap"] = 64.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 64.into();
            }
            // Base 391 lacks decide (8) and subscribe (32).
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = 431.into();
                project["policy"]["roles"][0]["authority"]["tools"] = 431.into();
            }
            value["application"]["landing"]["projects"] = json!([]);
            landing::admit(value);
        },
        ..fixture()
    }
}

// Act 3, step 6.6 (domain/testing.md, 5.3). These identities belong only
// to this two-repository case; earlier steps retain KIT = acme/app.
pub mod waiting {
    pub const DIRECTOR: u64 = 1;
    pub const A1: u64 = 2;
    pub const E1: u64 = 3;
    pub const REPIN: u64 = 4;
    pub const E1_CODER: u64 = 5;
    pub const E1_REVIEWER: u64 = 6;
    pub const REPIN_CODER: u64 = 7;
    pub const REPIN_REVIEWER: u64 = 8;
    pub const KIT_ID: u32 = 3;
    pub const APP_ID: u32 = 2;
    pub const KIT: &str = "acme/kit";
    pub const APP: &str = "acme/app";
    pub const FILE: &str = "src/fake.rs";
    pub const FIXED: &str = "// B's fake answers correctly.\n";
    pub const PIN: &str = "kit.pin";
    pub const CI_FILE: &str = "ci.txt";
    pub const GREEN: &str = "green\n";
    pub const PERSON: &str = "@case_waiting_director Yes: let e1 land in kit, then app's waiting repin; report both.";
    pub const OWNER_SPEC: &str = "@case_waiting_owner Delegate e1 in kit and app's repin together; repin depends on batch member zero. Wait for both results; send no relay.";
    pub const E1_SPEC: &str = "@case_waiting_kit Fix B's fake in src/fake.rs and land e1 in kit.";
    pub const REPIN_SPEC: &str = "@case_waiting_app Repin kit.pin to e1's landed kit commit and land in app.";
    pub const OWNER_REPORT: &str = "A1 reports e1 and app's waiting repin landed without relay.";
    pub const DIRECTOR_REPORT: &str = "The director reports kit e1 and app's dependent repin landed.";
    pub const REVIEW_HOLD: &[u8] = b"case-6-6-e1-review";
    // Script inputs only: app root 1, kit root 2, e1 coder 3, kit merge 4,
    // repin coder 5, app merge 6. Initial Waiting serializes the coders.
    // The oracle instead renders the unique actual kit Merged.commit.
    pub const KIT_REVIEW_HEAD: u64 = 3;
    pub const PIN_INPUT: u64 = 4;
    pub const APP_REVIEW_HEAD: u64 = 5;

    #[must_use]
    pub fn pin_input() -> String {
        use std::fmt::Write;
        let mut words = String::new();
        for byte in skein_fake_forge_domain::object_bytes(PIN_INPUT) {
            write!(words, "{byte:02x}").expect("commit words");
        }
        words
    }
}

fn waiting_setup(id: u32, name: &str) -> api::Setup {
    let mut setup = (landing::fixture().setup)();
    setup.id = id;
    setup.name = name.as_bytes().into();
    let mut tree = setup.tree.into_vec();
    tree.push(api::File { path: waiting::CI_FILE.as_bytes().into(), content: waiting::GREEN.as_bytes().into() });
    setup.tree = tree.into();
    // The same stable CI file is present at the base and changed tree.
    setup.checks.cue =
        Some(api::Cue::File { path: waiting::CI_FILE.as_bytes().into(), green: waiting::GREEN.as_bytes().into() });
    setup
}

#[must_use]
pub fn fixture_6_6() -> forge::Fixture {
    forge::Fixture {
        setup: || waiting_setup(waiting::APP_ID, waiting::APP),
        extra_setups: || Box::new([waiting_setup(waiting::KIT_ID, waiting::KIT)]),
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            let mut kit_rule = value["application"]["landing"]["projects"][0]["rules"][0].clone();
            kit_rule["pattern"]["segments"][3] = "kit".into();
            value["application"]["landing"]["projects"][0]["rules"].as_array_mut().expect("rules").push(kit_rule);
            // Two enforced rules each contribute CI, up-to-date and exact-head
            // blocking review: 2 * 3 = 6 requirements. Both repos are hooked.
            value["engine"]["limits"]["overrides"]["authority.requirements"] = 6.into();
            // Configuring-3's new admission check measures this two-repository
            // project policy at 4398 bytes, above the landing fixture's 4096.
            value["engine"]["limits"]["overrides"]["core.policy_bytes"] = 8192.into();
            // Eight tasks, two batch members, one dependency. The deepest child
            // has three requester ancestors + its terminal result = four joins;
            // repin has two ancestors + dependency + result = four as well.
            for name in ["tasks.batch", "authority.batch"] {
                value["engine"]["limits"]["overrides"][name] = 2.into();
            }
            // Three lines carry a change's repository, branch and path; they
            // also cover the maximum requester ancestry. Two adopted repositories
            // and two checkout repositories fit the existing small capacities.
            value["engine"]["limits"]["overrides"]["documents.lines"] = 3.into();
            // The coder's write + finish and each owner's delegate + wait share
            // one completion, serialized by the existing single call slot.
            agent["profile"]["declared"]["calls_per_response"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = 1.into();
            // F10: the atomic A1 batch exceeds the inherited 2048-byte link
            // input. Until F10 admission/drop handling is fixed, this fixture
            // gives both link sides the agent's existing tool_payload bound.
            value["engine"]["limits"]["overrides"]["core.call_answer_bytes"] = 4096.into();
            value["workers"][0]["limits"]["overrides"]["core.call_answer_bytes"] = 4096.into();
            // Each procedure reserves ten, A1 adds its ten-unit run reserve,
            // and the director adds ten: spends 20, 30 and 40 respectively.
            value["world_settings"]["cap"] = 40.into();
            // Hooks/adoption supply changes; retain the case's PATIENCE+1 polls.
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            landing::admit(value);
        },
    }
}

// Act 3, step 6.8 (domain/testing.md, 5.3).
pub mod qa {
    pub const DIRECTOR: u64 = 1;
    pub const A: u64 = 2;
    pub const A1: u64 = 3;
    pub const T: u64 = 4;
    pub const A2: u64 = 5;
    pub const C: u64 = 6;
    pub const CODER: u64 = 7;
    pub const REVIEWER: u64 = 8;
    pub const PERSON: &str = "@case_qa_director Delegate A for M1's end, M2's start and QA; report when done.";
    pub const OWNER_SPEC: &str =
        "@case_qa_owner Delegate A1, start M2's sub-owner on its M1 message, and pass that report to the director.";
    pub const SUBOWNER_SPEC: &str = "@case_qa_subowner Message A at M1's end, stay for QA, delegate a tester then land its test through your change.";
    pub const NEXT_SPEC: &str = "@case_qa_next M2's sub-owner: report admission only; do no work here.";
    pub const TESTER_SPEC: &str =
        "@case_qa_tester Report one adversarial test as path and content; try one forge status without authority.";
    pub const CHANGE_SPEC: &str = "@case_qa_coder Land the test at tests/qa.rs from T's result, your named spec input.";
    pub const M1_REPORT: &str = "M1 ended.\nDesign followed.\nChanges landed.\nCI green.\nReviews passed.\nSTATUS updated.\nNo blocker.\nM2 may start.\nQA follows.\nA1 stays live.";
    pub const QA_TEST_PATH: &str = "tests/qa.rs";
    pub const QA_TEST: &str = "#[test]\nfn regression() { assert_eq!(2 + 2, 4); }\n";
    pub const TEST_REPORT: &str = "tests/qa.rs\n#[test]\nfn regression() { assert_eq!(2 + 2, 4); }\n";
    pub const SUBOWNER_REPORT: &str = "A1 closes M1 after its change lands T's test.";
    pub const NEXT_REPORT: &str = "A2 admitted for M2; no work performed.";
    pub const OWNER_REPORT: &str = "A reports M1 closed after QA and M2's sub-owner admitted.";
    pub const DIRECTOR_REPORT: &str = "The director reports M1's end, M2's start and QA's test landed through A1.";
    pub const OWNER_HOLD: &[u8] = b"case-6-8-owner";
    pub const TESTER_HOLD: &[u8] = b"case-6-8-tester";
    pub const NEXT_HOLD: &[u8] = b"case-6-8-next";
    pub const STATUS_CONTEXT: &str = "tester-without-authority";
}

#[must_use]
pub fn fixture_6_8() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (fixture().setup)();
            setup.checks.cue =
                Some(api::Cue::File { path: qa::QA_TEST_PATH.as_bytes().into(), green: qa::QA_TEST.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (fixture().configure)(value, agent, forge);
            // c adds its input result to three ancestors and its own result;
            // coder/reviewer have four ancestors and a result. Small's four
            // store joins cannot read these; use standard's sixteen, as 6.2.
            value["engine"]["limits"]["profile"] = "standard".into();
            value["workers"][0]["limits"]["profile"] = "standard".into();
            value["application"]["limits"]["overrides"]["forge.named_calls"] = 128.into();
            value["application"]["limits"]["overrides"]["root.journal.deliveries"] = 1097.into();
            value["application"]["limits"]["overrides"]["root.journal.held"] = 3291.into();
            // Role -> director -> A -> A1 -> c -> coder: the role's
            // depth six yields the director's five, as 6.2's fixture does.
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["depth"] = 6.into();
                project["policy"]["roles"][0]["authority"]["delegation"]["depth"] = 6.into();
            }
            // Eight tasks, with A adding one ancestor over 6.6's smoke.
            // Keep each batch singleton and serialize each response's calls.
            value["engine"]["limits"]["overrides"]["documents.lines"] = 4.into();
            agent["profile"]["declared"]["calls_per_response"] = 1.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 1.into();
            value["engine"]["limits"]["overrides"]["fleet.calls"] = PAUSE_WORKERS.len().into();
            value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 2.into();
            // A reserves A1 (50), A2 (10), and its run's ten; the director
            // reserves A (80) and its run. All remain below the base ceiling 100.
            value["world_settings"]["cap"] = 100.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = 100.into();
            }
            let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
            for name in ["poll_ms", "poll_max_ms"] {
                value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
            }
            landing::admit(value);
        },
        ..fixture()
    }
}
