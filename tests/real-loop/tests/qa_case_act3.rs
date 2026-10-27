//! Adversarial real-loop runs of the retired first edition,
//! retained in domain/testing.md, 5.3; draft/reference-case.md is the second edition.
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Line, Script};
use std::net::SocketAddr;
use temper_real_loop_world::{
    argv,
    calls::{self, D1},
    case::{
        self, Agent, acts,
        oracle::{self, Miss, Observed, Promise},
    },
    run_cluster_forge,
};

const WORKERS: &[&str] = &["ws", "s1", "s2"];
// Each selected script call is a single structured mutation.
const ONE_EDIT: usize = 1;

fn edit_call(
    scripts: &mut [Script],
    agent: Agent,
    tool: &[u8],
    select: impl Fn(&Value) -> bool,
    edit: impl Fn(&mut Value),
) {
    let script = scripts.iter_mut().find(|s| s.cue.as_ref() == agent.cue()).expect("case agent");
    let mut edited = 0;
    for turn in &mut script.turns {
        for line in &mut turn.lines {
            if let Line::Call { name, arguments } = line
                && name.as_ref() == tool
            {
                let mut call: Value = serde_json::from_slice(arguments).expect("tool JSON");
                if select(&call) {
                    println!("editing {}: {call}", String::from_utf8_lossy(tool));
                    let original = call.clone();
                    edit(&mut call);
                    assert_ne!(call, original, "the selected call must actually change");
                    *arguments = serde_json::to_vec(&call).expect("edited tool JSON").into();
                    edited += 1;
                }
            }
        }
    }
    assert_eq!(edited, ONE_EDIT, "one intended call changed");
}

fn run(
    fixture: temper_real_loop_world::forge::Fixture,
    scripts: fn(SocketAddr) -> Box<[Script]>,
    rules: fn() -> Box<[Box<[skein_fake_llm_domain::api::Rule]>]>,
    steps: fn() -> Vec<temper_real_loop_world::calls::Step>,
    reads: &[(acts::Read, usize)],
) -> Observed {
    let ran = run_cluster_forge(WORKERS, &[], fixture, scripts, rules, steps);
    Observed::from_ran(&ran, reads).with_commands(&steps())
}
fn batch(scripts: fn(SocketAddr) -> Box<[Script]>) -> Observed {
    run(case::fixture_6_1(), scripts, case::scripts::step_6_1_rules, acts::step_6_1, acts::READS_6_1)
}
fn blocker(scripts: fn(SocketAddr) -> Box<[Script]>) -> Observed {
    run(case::fixture_6_2(), scripts, case::scripts::step_6_2_rules, acts::step_6_2, acts::READS_6_2)
}
fn budget(scripts: fn(SocketAddr) -> Box<[Script]>) -> Observed {
    run(case::fixture_6_5(), scripts, case::scripts::step_6_5_rules, acts::step_6_5, acts::READS_6_5)
}

fn task(document: &Value) -> &Value {
    document.get("task").filter(|value| value.is_object()).unwrap_or(document)
}

fn only_miss(misses: &[Miss], step: &'static str, promise: Promise) {
    for miss in misses {
        println!("{miss}");
    }
    assert_eq!(misses.iter().map(|miss| (miss.step, miss.promise)).collect::<Vec<_>>(), [(step, promise)]);
}

#[test]
fn both_prerequisites_gate_the_story_while_a_third_runs() {
    // 6.1: a4 waits after a1 alone, then runs before a3 ends.
    oracle::assert_met(&oracle::step_6_1(&batch(case::scripts::step_6_1)));
}

#[test]
fn reversing_dependency_order_preserves_the_same_waits() {
    let observed = batch(|forge| {
        let mut scripts = case::scripts::step_6_1(forge);
        edit_call(
            &mut scripts,
            Agent::BatchSubowner,
            b"delegate",
            |_| true,
            |call| {
                let entries = call["batch"].as_array_mut().expect("batch");
                let story = entries.last_mut().expect("story delegate");
                story["dependencies"].as_array_mut().expect("story dependencies").reverse();
            },
        );
        scripts
    });
    // A batch's dependencies name its prerequisites; their order grants no
    // permission to start after only one has ended (jig's domain/tasks.md, 4 and 7).
    for read in [acts::Read::BatchStoryWaiting, acts::Read::BatchStoryAfterOne] {
        let story = task(&observed.documents[&read]);
        println!("{read:?}: dependencies {}, phase {}", story["dependency_facts"], story["chip"]["phase"]);
        assert_eq!(story["chip"]["phase"], "waiting");
        assert!(story["chip"]["details"]["owner"]["details"]["attempt"].is_null());
    }
    let story = task(&observed.documents[&acts::Read::BatchStoryEnded]);
    let dependencies: std::collections::BTreeSet<_> = story["dependency_facts"]
        .as_array()
        .expect("dependencies")
        .iter()
        .map(|dependency| dependency["number"].as_u64().expect("dependency number"))
        .collect();
    assert_eq!(dependencies, [case::BATCH_ONE, case::BATCH_TWO].into());
    assert_eq!(
        task(&observed.documents[&acts::Read::BatchThreeStillWriting])["chip"]["details"]["owner"]["details"]["observation"]
            ["work"],
        "writing"
    );
    oracle::assert_met(&oracle::step_6_1(&observed));
}

#[test]
fn a_live_dependency_keeps_the_finished_run_until_landing() {
    // 6.2: a3 stops going on; e1's merge, not its coder's report, releases it.
    oracle::assert_met(&oracle::step_6_2(&blocker(case::scripts::step_6_2)));
}

#[test]
fn cancelling_the_pending_fix_holds_its_live_dependent() {
    let ran = run_cluster_forge(
        WORKERS,
        &[],
        case::fixture_6_2(),
        |forge| {
            let mut scripts = case::scripts::step_6_2(forge);
            let owner = scripts
                .iter_mut()
                .find(|script| script.cue.as_ref() == Agent::BlockerSubowner.cue())
                .expect("blocker sub-owner");
            let mut changed = 0;
            // Keep A1 live rather than let its ordinary success report close
            // the held dependent after the exceptional cancellation.
            for turn in &mut owner.turns {
                for line in &mut turn.lines {
                    if let Line::Call { name, arguments } = line
                        && name.as_ref() == b"finish"
                    {
                        *name = b"wait".as_slice().into();
                        *arguments = b"{}".as_slice().into();
                        changed += 1;
                    }
                }
            }
            assert_eq!(changed, ONE_EDIT, "keep one owner report pending");
            scripts
        },
        case::scripts::step_6_2_rules,
        || {
            let mut steps = acts::step_6_2();
            let release = steps
                .iter()
                .position(|step| {
                    matches!(step,
                        calls::Step::ReleaseHold { name } if *name == case::BLOCKER_REVIEW_HOLD
                    )
                })
                .expect("fix review release");
            steps.truncate(release);
            steps.extend([
                calls::call(D1, argv!["task", "cancel", calls::number(case::BLOCKER_E1), "QA cancels the blocker fix"]),
                calls::call(D1, argv!["task", "show", calls::number(case::BLOCKER_E1), "--until", "ended"]),
                calls::call(D1, argv!["task", "show", calls::number(case::BLOCKER_A3), "--until", "held"]),
                calls::call(D1, argv!["task", "show", calls::number(case::BLOCKER_A3), "--json"]),
                calls::release_hold(case::BLOCKER_REVIEW_HOLD),
            ]);
            steps
        },
    );
    let read = ran.calls().last().expect("held dependent read");
    let document: Value = serde_json::from_slice(&read.stdout).expect("task JSON");
    let dependent = task(&document);
    println!("dependent phase {}, hold {}", dependent["chip"]["phase"], dependent["hold"]);
    // jig's domain/tasks.md, 7: a dependency cancelled later holds its dependent.
    assert_eq!(dependent["chip"]["phase"], "held");
    assert_eq!(dependent["hold"]["owner"]["reason"]["dependency"]["task"], case::BLOCKER_E1);
    assert!(dependent["result"].is_null(), "a3 must not finish successfully after its fix is cancelled");
}

#[test]
fn a_fix_to_another_file_cannot_satisfy_the_blocker() {
    let observed = blocker(|forge| {
        let mut scripts = case::scripts::step_6_2(forge);
        edit_call(
            &mut scripts,
            Agent::BlockerCoder,
            b"edit",
            |_| true,
            |call| {
                assert_eq!(call["path"], case::BLOCKER_FILE);
                *call = json!({
                    "path": case::DRAFT,
                    "edits": [{"old": case::DRAFT_WORDS, "new": "# An unrelated draft change\n"}],
                });
            },
        );
        scripts
    });
    // CI's cue passes on B's unchanged file. The 6.2 row requires e1's
    // landing to change B's fake, not just another file on a green head.
    assert_eq!(
        observed.kit.main.get(case::BLOCKER_FILE.as_bytes()).expect("B's fake").as_slice(),
        case::BLOCKER_WRONG.as_bytes()
    );
    assert_eq!(
        observed.kit.main.get(case::DRAFT.as_bytes()).expect("changed draft").as_slice(),
        b"# An unrelated draft change\n"
    );
    let misses = oracle::step_6_2(&observed);
    only_miss(&misses, "6.2", Promise::Must("e1 lands green changing B's fake"));
}

#[test]
fn the_persons_answer_precedes_investigation_and_advice() {
    oracle::assert_met(&oracle::step_6_5(&budget(case::scripts::step_6_5)));
}

#[test]
fn an_unraised_effort_is_reported_at_the_next_revision() {
    let observed = budget(|forge| {
        let mut scripts = case::scripts::step_6_5(forge);
        edit_call(
            &mut scripts,
            Agent::BudgetSubowner,
            b"amend",
            |_| true,
            |call| {
                *call.pointer_mut("/amendment/llm/main/effort").expect("amended effort") =
                    case::BUDGET_MEDIUM_NUMBER.into();
            },
        );
        scripts
    });
    let misses = oracle::step_6_5(&observed);
    only_miss(&misses, "6.5", Promise::Must("a5's effort is raised for its next try"));
}

fn advisor(call: &Value) -> bool {
    call.pointer("/batch/0/spec/words").and_then(Value::as_str).is_some_and(|words| words == case::BUDGET_ADVISOR_SPEC)
}

#[test]
fn a_default_model_advisor_is_reported_despite_its_answer() {
    let observed = budget(|forge| {
        let mut scripts = case::scripts::step_6_5(forge);
        edit_call(&mut scripts, Agent::BudgetSubowner, b"delegate", advisor, |call| {
            assert!(call["batch"][0].as_object_mut().expect("advisor").remove("llm").is_some());
        });
        scripts
    });
    let misses = oracle::step_6_5(&observed);
    only_miss(&misses, "6.5", Promise::Must("one advisor reads a5's report on the owner's model"));
}

#[test]
fn correct_advice_without_the_evidence_does_not_pass() {
    let observed = budget(|forge| {
        let mut scripts = case::scripts::step_6_5(forge);
        edit_call(&mut scripts, Agent::BudgetSubowner, b"delegate", advisor, |call| {
            *call.pointer_mut("/batch/0/spec/words").expect("advisor brief") =
                "@case_budget_advisor Name the cause and report once.".into();
        });
        scripts
    });
    // Retained first edition (domain/testing.md, 5.3): the advisor reads the report and its evidence.
    // Its scripted correct answer alone cannot prove it received either.
    let misses = oracle::step_6_5(&observed);
    only_miss(&misses, "6.5", Promise::Must("one advisor reads a5's report on the owner's model"));
}
