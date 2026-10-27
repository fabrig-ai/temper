//! Adversarial stories for domain/testing.md, 5.3 and draft/reference-case.md, 4.2 and 7.2.
//! Only scripts, fixture inputs and the person's reads are changed. A story whose
//! mutation stalls it (A untracked, authority beyond the director's, the answer before
//! the question or none, CI red) fails before the oracle, on the settle deadline or the
//! forge's record cap: loud, but not the oracle's to report, so none is kept here.
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use std::net::SocketAddr;
use temper_real_loop_world::{
    Ran, argv,
    calls::{self, D1},
    case::{
        self, Agent, acts,
        oracle::{self, Observed, Promise},
    },
};

// One delegate call per director/owner script, and one record per tool turn.
const ONE_EDIT: usize = 1;
const FIRST_DELEGATE: usize = 0;
// The fresh case has at most eight tasks; this number names none of them.
const ABSENT_TASK: u64 = 999;
const PAST_CALLS: usize = usize::MAX;
const OTHER_PRIORITY: u8 = 2;
// Keep the case's scripted per-turn token charge when adding a tool turn.
const TOKENS_PER_CALL: u64 = 20;
const NON_MARKDOWN: &str = "export.rs";
const NON_MARKDOWN_WORDS: &str = "pub fn export() {}\n";
const CHANGED_DRAFT: &str = "# Export draft\n\nChanged by the designer.\n";

fn script(scripts: &mut [Script], agent: Agent) -> &mut Script {
    scripts.iter_mut().find(|script| script.cue.as_ref() == agent.cue()).expect("case agent script")
}

fn edit_call(scripts: &mut [Script], agent: Agent, tool: &[u8], edit: impl Fn(&mut Value)) {
    let mut edited = 0;
    for turn in &mut script(scripts, agent).turns {
        for line in &mut turn.lines {
            if let Line::Call { name, arguments } = line
                && name.as_ref() == tool
            {
                let mut value: Value = serde_json::from_slice(arguments).expect("script call JSON");
                edit(&mut value);
                *arguments = serde_json::to_vec(&value).expect("mutated call JSON").into();
                edited += 1;
            }
        }
    }
    assert_eq!(edited, ONE_EDIT, "edit exactly the intended tool call");
}

fn tool_turn(name: &[u8], arguments: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(arguments).expect("tool JSON").into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: TOKENS_PER_CALL,
    }
}

fn extra_design_turn(scripts: &mut [Script], tool: &[u8], arguments: &Value) {
    let design = script(scripts, Agent::Design);
    let mut turns = design.turns.to_vec();
    let finish = turns.pop().expect("design finish");
    turns.extend([tool_turn(tool, arguments), finish]);
    design.turns = turns.into();
}

fn story(scripts: fn(SocketAddr) -> Box<[Script]>) -> Ran {
    temper_real_loop_world::run_forge_by_input(case::fixture(), scripts, case::scripts::step_4_1_rules, acts::step_4_1)
}

fn misses(ran: &Ran, expected: &[(Promise, &str)]) {
    assert_report(&Observed::from_ran(ran, acts::READS_4_1), expected);
}

fn assert_report(observed: &Observed, expected: &[(Promise, &str)]) {
    let misses = oracle::step_4_1(observed);
    for miss in &misses {
        println!("{miss}");
    }
    for &(promise, words) in expected {
        assert!(
            misses.iter().any(|miss| miss.step == "4.1" && miss.promise == promise && miss.what.contains(words)),
            "expected {promise:?}: {words}; see printed misses"
        );
    }
    // Account for the entire list: another checkpoint's miss cannot hide behind this one.
    assert_eq!(misses.len(), expected.len(), "the mutation has exactly its intended misses");
}

const DELEGATION: Promise =
    Promise::Must("delegate a goal with instructions, input, priority, host kind, pool and models (exact)");
const AUTHORITY: Promise = Promise::Add("authority bounds A's subtree");
const MARKDOWN: Promise = Promise::Must("the fast path for Markdown alone (exact)");

#[test]
fn another_priority_reports_the_delegation_must() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        edit_call(&mut scripts, Agent::Director, b"delegate", |call| {
            call["batch"][FIRST_DELEGATE]["tracked"]["priority"] = OTHER_PRIORITY.into();
        });
        scripts
    });
    misses(&ran, &[(DELEGATION, "priority")]);
}

#[test]
fn the_default_model_reports_the_delegation_must() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        edit_call(&mut scripts, Agent::Director, b"delegate", |call| {
            call["batch"][FIRST_DELEGATE].as_object_mut().expect("delegate").remove("llm");
        });
        scripts
    });
    misses(&ran, &[(DELEGATION, "A.llm.main.model must be owner"), (DELEGATION, "A.llm.main.effort must be xhigh")]);
}

#[test]
fn a_brief_without_the_commit_reports_the_delegation_must() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        edit_call(&mut scripts, Agent::Director, b"delegate", |call| {
            call["batch"][FIRST_DELEGATE]["spec"]["words"] =
                format!("{}{}", case::A_SPEC_PREFIX, case::A_SPEC_SUFFIX).into();
        });
        scripts
    });
    misses(&ran, &[(DELEGATION, "commit")]);
}

#[test]
fn a_non_markdown_file_reports_the_markdown_must() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        extra_design_turn(&mut scripts, b"write", &json!({"path":NON_MARKDOWN,"content":NON_MARKDOWN_WORDS}));
        scripts
    });
    misses(&ran, &[(MARKDOWN, "Markdown")]);
}

#[test]
fn a_changed_draft_reports_the_markdown_must() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        extra_design_turn(
            &mut scripts,
            b"edit",
            &json!({"path":case::DRAFT,"edits":[{"old":case::DRAFT_WORDS,"new":CHANGED_DRAFT}]}),
        );
        scripts
    });
    assert_eq!(
        ran.forge().main_file(case::DRAFT.as_bytes()),
        CHANGED_DRAFT.as_bytes(),
        "the draft mutation must actually land"
    );
    misses(&ran, &[(MARKDOWN, "draft")]);
}

#[test]
fn a_failed_document_act_cannot_silently_pass() {
    let design_read = acts::READS_4_1
        .iter()
        .find_map(|&(read, call)| (read == acts::Read::DesignEnded).then_some(call))
        .expect("design read position");
    let ran = temper_real_loop_world::run_forge_by_input(
        case::fixture(),
        case::scripts::step_4_1,
        case::scripts::step_4_1_rules,
        || {
            let mut steps = acts::step_4_1();
            steps[design_read] = calls::call(D1, argv!["task", "show", calls::number(ABSENT_TASK), "--json"]);
            steps
        },
    );
    let failed = &ran.calls()[design_read];
    println!(
        "document act: exit {:?}, stdout {}, stderr {}",
        failed.exit,
        String::from_utf8_lossy(&failed.stdout),
        String::from_utf8_lossy(&failed.stderr)
    );
    missing_document_cannot_pass(&ran, acts::READS_4_1);
}

#[test]
fn a_read_past_calls_cannot_silently_pass() {
    let ran = story(case::scripts::step_4_1);
    let reads: Vec<_> = acts::READS_4_1
        .iter()
        .map(|&(read, call)| (read, if read == acts::Read::DesignEnded { PAST_CALLS } else { call }))
        .collect();
    missing_document_cannot_pass(&ran, &reads);
}

fn missing_document_cannot_pass(ran: &Ran, reads: &[(acts::Read, usize)]) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Observed::from_ran(ran, reads)));
    if let Ok(observed) = result {
        println!("from_ran returned documents {:?}", observed.documents.keys().collect::<Vec<_>>());
        assert!(!observed.documents.contains_key(&acts::Read::DesignEnded), "the failed read has no document");
        assert_report(
            &observed,
            &[
                (DELEGATION, "DesignEnded: task number must be 3"),
                (DELEGATION, "DesignEnded: project must be 1"),
                (AUTHORITY, "Design.authority/spend exceeds its parent or is absent"),
                (AUTHORITY, "Design.authority/delegation/tasks exceeds its parent or is absent"),
                (AUTHORITY, "Design.authority/delegation/depth exceeds its parent or is absent"),
                (AUTHORITY, "Design.authority.tools must be a subset"),
                (AUTHORITY, "Design.authority.delegation.kinds is absent"),
                (AUTHORITY, "Design.authority.grants is absent"),
                (
                    Promise::Add("spend rolls up to the goal"),
                    "A.budget.spent_below must cover its child's spent + spent_below",
                ),
            ],
        );
    }
}

#[test]
fn several_misses_are_reported_together_without_hiding_checkpoints() {
    let ran = story(|forge| {
        let mut scripts = case::scripts::step_4_1(forge);
        edit_call(&mut scripts, Agent::Director, b"delegate", |call| {
            let owner = &mut call["batch"][FIRST_DELEGATE];
            owner["tracked"]["priority"] = OTHER_PRIORITY.into();
            owner["spec"]["words"] = format!("{}{}", case::A_SPEC_PREFIX, case::A_SPEC_SUFFIX).into();
        });
        extra_design_turn(&mut scripts, b"write", &json!({"path":NON_MARKDOWN,"content":NON_MARKDOWN_WORDS}));
        scripts
    });
    misses(
        &ran,
        &[
            (DELEGATION, "A.priority must be 1"),
            (DELEGATION, "A.spec must name the draft and its first commit"),
            (MARKDOWN, "changed path \"export.rs\" is not Markdown"),
        ],
    );
}
