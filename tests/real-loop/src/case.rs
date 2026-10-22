//! The reference case's cast, accounts and first kit tree (domain/testing.md, 5.3).
use crate::{fixtures, forge, landing};
use serde_json::json;
use skein_fake_forge_domain::{self as domain, api};

pub mod acts;
pub mod oracle;
pub mod scripts;

// Fresh deployment numbers; the oracle checks them against the person's documents
// and A's question's tool output and the answer delivered to A.
pub const PROJECT: u64 = 1;
pub const DIRECTOR: u64 = 1;
pub const A: u64 = 2;
pub const DESIGN: u64 = 3;
pub const QUESTION: u64 = 6;
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
}
impl Agent {
    #[must_use]
    pub fn cue(self) -> &'static [u8] {
        match self {
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

#[must_use]
pub fn draft_commit() -> String {
    // State::new uses this configuration and seed. repository stores the first
    // tree without a timestamp; real-loop wall skew cannot change its identity.
    let config = temper_fake_git_hosted::fixtures::CONFIG;
    let mut domain = domain::Domain::new(&config, fixtures::SEED);
    let first = domain::repository(&mut domain, &config, (fixture().setup)());
    let object = skein_fake_forge_protocol::routes::object(first);
    let mut words = String::new();
    for byte in object.bytes {
        use std::fmt::Write;
        write!(words, "{byte:02x}").expect("commit hex");
    }
    words
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
            // The direct service fixture has no binary-added deployment project.
            // Its owner role in project zero makes deployment accounts readable.
            let mut deployment = value["engine"]["projects"][0].clone();
            deployment["number"] = 0.into();
            value["engine"]["projects"].as_array_mut().expect("projects").push(deployment);
            value["engine"]["limits"]["overrides"]["people.projects"] = 2.into();
            value["engine"]["limits"]["overrides"]["authority.projects"] = 2.into();
            value["engine"]["limits"]["overrides"]["tasks.funders"] = 4.into();
            // A runs on the director's explicit choice, distinct from the default.
            let mut model = value["engine"]["llm"]["models"][0].clone();
            model["name"] = OWNER_MODEL.into();
            model["efforts"] = json!([OWNER_EFFORT]);
            value["engine"]["llm"]["models"].as_array_mut().expect("models").push(model);
            value["engine"]["llm"]["providers"][0]["models"] = json!(["fake", OWNER_MODEL]);
            value["engine"]["charters"][0]["alternatives"] = json!([OWNER_MODEL]);
            let mut model = agent["endpoints"][0]["models"][0].clone();
            model["name"] = OWNER_MODEL.into();
            model["efforts"] = json!([OWNER_EFFORT]);
            agent["endpoints"][0]["models"].as_array_mut().expect("agent models").push(model);
            // The director delegates, messages, amends and keeps project notes.
            value["engine"]["rules"]["ceiling"]["tools"] = 391.into();
            value["engine"]["rules"]["ceiling"]["notes"] = 2.into();
            // The extra owner adds one delegation level before the change procedure.
            value["engine"]["rules"]["ceiling"]["delegation"]["depth"] = 5.into();
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
            value["engine"]["templates"][0]["contract"]["report"]["words"] = 128.into();
            // The brief includes the full draft commit and its instructions.
            value["engine"]["limits"]["overrides"]["documents.text_bytes"] = 512.into();
            landing::admit(value);
        },
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
            let mut director = value["engine"]["charters"][0].clone();
            director["number"] = 2.into();
            director["waiting_ms"] = 60000.into();
            value["engine"]["charters"].as_array_mut().expect("charters").push(director);
            value["engine"]["limits"]["overrides"]["tasks.charters"] = 2.into();
            value["engine"]["templates"][0]["executor"]["charter"] = 2.into();
            // The person must also be offered the new executor for that chat.
            let mut kinds = value["engine"]["rules"]["ceiling"]["delegation"]["kinds"].clone();
            kinds.as_array_mut().expect("executor kinds").push(json!({"kind":"charter","number":2}));
            value["engine"]["rules"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
                project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds.clone();
            }
            landing::admit(value);
        },
        ..fixture()
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
            // Subscribe is family 32; cancel shares control (4), already in 391.
            let tools = 0x0187 | 0x0020;
            value["engine"]["rules"]["ceiling"]["tools"] = tools.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = tools.into();
                project["policy"]["roles"][0]["authority"]["tools"] = tools.into();
            }
            // Announce placement and wait in one completion, with serial tools.
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
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

// The third chat and its new owner follow the opener and the two-hour check.
pub const HANG: u64 = 9;
pub const PHASE: u64 = 10;
pub const HANG_PERSON: &str = "@case_hang Delegate A's next phase and answer it once after the workstation goes away.";
pub const PHASE_SPEC: &str = "@case_phase Own Export's next phase and ask the director for its green light.";
pub const PHASE_QUESTION: &str = "May Export's next phase begin?";
pub const PHASE_ANSWER: &str = "Begin Export's next phase.";
pub const PHASE_REPORT: &str = "Export's next phase has its direction.";
pub const HANG_READ: &str = "A's next-phase question is read; its answer is still waiting.";
pub const HANG_REPORT: &str = "The restarted director answered A and its next phase reported.";
// The core allocates message numbers across the combined story.
pub const PHASE_QUESTION_NUMBER: u64 = 16;

/// The retry clock belongs to the case from its first preparation.
#[must_use]
pub fn fixture_4_6() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (fixture_check().configure)(value, agent, forge);
            // The retry path is the same, only its wait is short; the backoff is not what this story tests.
            value["engine"]["limits"]["overrides"]["tasks.retries.agent.base_ms"] = 1.into();
            landing::admit(value);
        },
        ..fixture_check()
    }
}
