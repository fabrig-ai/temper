//! The reference case's judgements, played word for word (domain/testing.md, 5.3).
use super::{
    A, A_REPORT, A_SPEC_PREFIX, A_SPEC_SUFFIX, ANSWERED, ASKED, Agent, DESIGN_DELEGATED, DESIGN_DOC, DESIGN_LINE,
    DESIGN_REPORT, DESIGN_SPEC, DESIGN_TITLE, DESIGN_WORDS, DIRECTOR, DIRECTOR_REPORT, GREEN_LIGHT,
    OWNER_EFFORT_NUMBER, OWNER_MODEL_NUMBER, QUESTION, REVIEWED, SCHEDULED, draft_commit,
};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Rule, Script, Turn};
use std::net::SocketAddr;

/// Smith renders its opening in smith-domain-run/src/prompt.rs.
const BEGIN: &str = "Begin the work your brief describes.";
/// Smith returns crossed finish feedback in smith-domain-run/src/run.rs via prompt.rs.
const CROSSED: &str = "Finish was not accepted because messages arrived.";
/// Temper renders smith's message acknowledgement in temper-engine-smith/src/answers.rs.
const MESSAGE: &str = "Message ";
/// Temper renders smith's delegation acknowledgement in temper-engine-smith/src/answers.rs.
const DELEGATED: &str = "Delegated tasks: ";
/// Smith renders labelled inbox deliveries in smith-domain-run/src/inbox.rs.
const DELIVERY: &str = ": ";

/// The turn rules for `step_4_1`'s scripts, index-parallel (skein's fake-llm.md, 2.4).
#[must_use]
pub fn step_4_1_rules() -> Box<[Box<[Rule]>]> {
    let director = [
        (A_REPORT, 4),
        (CROSSED, 4),
        (DESIGN_REPORT, 2),
        (MESSAGE, 3),
        (&format!("{DELEGATED}{A}"), 1),
        (DELIVERY, 1),
        (BEGIN, 0),
        ("", 1),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    let a = [
        (GREEN_LIGHT, 4),
        (CROSSED, 4),
        (&format!("Result from task {}:\n", super::DESIGN), 2),
        (MESSAGE, 3),
        (&format!("{DELEGATED}{}", super::DESIGN), 1),
        (DELIVERY, 1),
        (BEGIN, 0),
        ("", 1),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([Box::new(director), Box::new(a), Box::new([]), Box::new([])])
}

/// The opener's rules followed by one empty table per check script.
#[must_use]
pub fn step_4_1_4_3_rules() -> Box<[Box<[Rule]>]> {
    let mut rules = step_4_1_rules().into_vec();
    rules.extend([Box::<[Rule]>::default(), Box::default(), Box::default()]);
    rules.into_boxed_slice()
}

/// Both restart scripts count turns, since opening and retry share their input.
#[must_use]
pub fn step_4_6_rules() -> Box<[Box<[Rule]>]> {
    Box::new([Box::new([]), Box::new([])])
}

fn call(name: &[u8], value: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into_boxed_slice(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn wait(words: &str) -> Turn {
    Turn {
        lines: Box::new([
            Line::Text { text: words.as_bytes().into() },
            Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn then_wait(mut turn: Turn, words: &str) -> Turn {
    let mut lines = turn.lines.into_vec();
    lines.extend(wait(words).lines);
    turn.lines = lines.into_boxed_slice();
    turn
}
fn report(words: &str) -> Turn {
    call(b"finish", &json!({"form":"report","text":words,"fields":{}}))
}
fn grants(forge: SocketAddr, kinds: &[u8]) -> Value {
    let mut grants = Vec::new();
    for kind in kinds {
        grants.push(
            json!({"connector":1,"kind":kind,"segments":["forge",forge.to_string()],"terminal":"open","last":"acme"}),
        );
    }
    Value::Array(grants)
}

#[must_use]
pub fn step_4_1(forge: SocketAddr) -> Box<[Script]> {
    let owner = json!({"batch":[{
        "executor":{"kind":"agent","charter":1},"tracked":{"priority":1},
        "spec":{"words":format!("{A_SPEC_PREFIX}{}{A_SPEC_SUFFIX}", draft_commit()),"parameters":[]},
        "contract":{"kind":"report","words":128},
        "llm":{"main":{"model":OWNER_MODEL_NUMBER,"effort":OWNER_EFFORT_NUMBER}},
        "authority":{"tools":3,"grants":grants(forge, &[2,3,4,7,8,10,11,12,13]),
            "delegation":{"kinds":[{"kind":"procedure","number":2},{"kind":"agent","number":1},{"kind":"agent","number":4}],"tasks":6,"depth":3},
            "budget":{"spend":20},"notes":0}
    }]});
    let design = json!({"batch":[{
        "executor":{"kind":"procedure","connector":1,"code":2},
        "spec":{"words":DESIGN_SPEC,"parameters":[
            {"name":1,"kind":"resource","connector":1,"resource":temper_engine_domain_forge::tag(1, crate::forge::ID)},
            {"name":2,"kind":"bytes","value":"main"}]},
        "contract":{"kind":"change","connector":1,"change_kind":1,"words":128},
        "authority":{"tools":1,"grants":grants(forge, &[2,3,4,10,11,12]),
            "delegation":{"kinds":[{"kind":"agent","number":1},{"kind":"agent","number":4}],"tasks":4,"depth":2},
            "budget":{"spend":10},"notes":0}
    }]});
    Box::new([
        Script {
            cue: Agent::Director.cue().into(),
            turns: Box::new([
                call(b"delegate", &owner),
                wait(SCHEDULED),
                call(b"message", &json!({"target":A,"form":"answer","question":QUESTION,"words":GREEN_LIGHT})),
                wait(ANSWERED),
                report(DIRECTOR_REPORT),
            ]),
        },
        Script {
            cue: Agent::A.cue().into(),
            turns: Box::new([
                call(b"delegate", &design),
                wait(DESIGN_DELEGATED),
                call(b"message", &json!({"target":DIRECTOR,"form":"question","words":DESIGN_REPORT})),
                wait(ASKED),
                report(A_REPORT),
            ]),
        },
        Script {
            cue: Agent::Design.cue().into(),
            turns: Box::new([
                call(b"write", &json!({"path":DESIGN_DOC,"content":DESIGN_WORDS})),
                call(b"finish", &json!({"form":"change","fields":{"title":DESIGN_TITLE,"body":DESIGN_LINE}})),
            ]),
        },
        Script {
            cue: Agent::Review.cue().into(),
            turns: Box::new([call(
                b"finish",
                &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]}),
            )]),
        },
    ])
}

// Account stories use no forge grants and adopt no repository.
fn owner(spec: &str, survey: bool) -> Turn {
    call(
        b"delegate",
        &json!({"batch":[{
            "executor":{"kind":"agent","charter":1},"tracked":{"priority":1},
            "spec":{"words":spec,"parameters":[]},"contract":{"kind":"report","words":128},
            "llm":{"main":{"model":OWNER_MODEL_NUMBER,"effort":OWNER_EFFORT_NUMBER}},
            "authority":{"tools":3,"grants":[],
                "delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":u8::from(survey),"depth":1},
                "budget":{"spend":20},"notes":0}
        }]}),
    )
}

#[must_use]
pub fn step_4_5_refused(_forge: SocketAddr) -> Box<[Script]> {
    account_wait(Line::Fail {
        error: skein_fake_llm_domain::api::Error::Unauthorized,
        report: skein_fake_llm_domain::api::Report::none(),
        credentials: Box::new([0]),
    })
}

#[must_use]
pub fn step_4_5_limited(_forge: SocketAddr) -> Box<[Script]> {
    account_wait(Line::Fail {
        error: skein_fake_llm_domain::api::Error::Exhausted { retry_after: Some(skein_lib::Duration::from_secs(1)) },
        report: skein_fake_llm_domain::api::Report::none(),
        credentials: Box::new([0]),
    })
}

fn account_wait(failure: Line) -> Box<[Script]> {
    use super::{HELD_A_SPEC, RESUMED_REPORT};
    let mut resumed = report(RESUMED_REPORT);
    let mut lines = vec![failure];
    lines.extend(resumed.lines);
    resumed.lines = lines.into();
    Box::new([
        Script {
            cue: Agent::Director.cue().into(),
            turns: Box::new([owner(HELD_A_SPEC, false), wait("A delegated."), report(RESUMED_REPORT)]),
        },
        Script { cue: Agent::A.cue().into(), turns: Box::new([resumed]) },
    ])
}

// When a director can ask its own person, swap this turn and its placement.
fn pace_question() -> Turn {
    call(b"message", &json!({"target":DIRECTOR,"form":"question","words":super::PACE_QUESTION}))
}
fn note() -> Turn {
    call(
        b"note",
        &json!({"scope":{"kind":"project","project":super::PROJECT},
        "description":super::PACE_NOTE,"body":super::PACE_ANSWER}),
    )
}
fn recall() -> Turn {
    call(
        b"recall",
        &json!({"kind":"search","scopes":[{"kind":"project","project":super::PROJECT}],
        "query":super::PACE_NOTE}),
    )
}
fn amend() -> Turn {
    call(
        b"amend",
        &json!({"target":super::A,"revision":0,"amendment":{
        "reason":super::PACE_ANSWER,"spec":{"words":super::PACE_A_RESUMED_SPEC,"parameters":[]},
        "llm":{"main":{"model":super::DEFAULT_MODEL_NUMBER}}}}),
    )
}

#[must_use]
pub fn step_4_4(_forge: SocketAddr) -> Box<[Script]> {
    use super::{DEFAULT_MODEL_NUMBER, PACE_A_SPEC, PACE_REPORT, SURVEY_REPORT, SURVEY_SPEC};
    Box::new([
        Script {
            cue: Agent::Director.cue().into(),
            turns: Box::new([
                owner(PACE_A_SPEC, true),
                wait("A delegated."),
                wait(super::PACE_PENDING),
                note(),
                amend(),
                wait("The answer is kept and A's model amended."),
                recall(),
                report(PACE_REPORT),
            ]),
        },
        // At the same position, the longer cue wins by being listed first.
        // Smith's recap resets the fake's assistant-message turn count.
        Script {
            cue: super::PACE_A_RESUMED_SPEC.as_bytes().into(),
            turns: Box::new([
                then_wait(
                    call(
                        b"delegate",
                        &json!({"batch":[{
                            "executor":{"kind":"agent","charter":1},
                            "spec":{"words":SURVEY_SPEC,"parameters":[]},
                            "contract":{"kind":"report","words":128},
                            "llm":{"main":{"model":DEFAULT_MODEL_NUMBER}},
                            "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
                                // Leave room for A's current run's reservation.
                                // Five units admit the model's four-unit reserve floor.
                                "budget":{"spend":5},"notes":0}
                        }]}),
                    ),
                    "Survey delegated.",
                ),
                report(PACE_REPORT),
                // The old conversation has two assistant turns at the model
                // change; the resumed one reports at assistant position one.
                Turn {
                    lines: Box::new([Line::Text { text: super::PACE_ANSWER.as_bytes().into() }]),
                    finish: Finish::Stop,
                    tokens: 20,
                },
            ]),
        },
        Script { cue: Agent::A.cue().into(), turns: Box::new([pace_question(), wait("Asked about the pace.")]) },
        Script { cue: Agent::Survey.cue().into(), turns: Box::new([report(SURVEY_REPORT)]) },
    ])
}

#[must_use]
pub fn step_4_3(_forge: SocketAddr) -> Box<[Script]> {
    use super::{B_REPORT, B_SPEC, BENCHMARK, BENCHMARK_RUNNING, BENCHMARK_SPEC, CHECK, CHECK_REASON, CHECK_REPORT};
    use skein_fake_llm_domain::api::Piece;
    let delegate = |spec, tools, tasks, depth, spend| {
        call(
            b"delegate",
            &json!({"batch":[{
                "executor":{"kind":"agent","charter":2},
                "spec":{"words":spec,"parameters":[]},"contract":{"kind":"report","words":128},
                "authority":{"tools":tools,"grants":[],
                    "delegation":{"kinds":[{"kind":"agent","number":2}],"tasks":tasks,"depth":depth},
                    "budget":{"spend":spend},"notes":0}
            }]}),
        )
    };
    let mut arm = call(b"subscribe", &json!({"kind":"timer","at":0}));
    let mut lines = vec![Line::Pieces {
        pieces: Box::new([Piece {
            text: b"All workers placed; arm the two-hour check.".as_slice().into(),
            hold: Some(crate::cluster::PLACED.into()),
        }]),
    }];
    lines.extend(arm.lines);
    arm.lines = lines.into_boxed_slice();
    Box::new([
        Script {
            cue: Agent::Check.cue().into(),
            turns: Box::new([
                delegate(B_SPEC, 3, 1, 1, 20),
                wait("B scheduled; awaiting the benchmark on s2."),
                arm,
                wait("The two-hour check is armed."),
                call(b"cancel", &json!({"target":BENCHMARK,"reason":CHECK_REASON})),
                wait("The benchmark's cancellation was requested."),
                report(CHECK_REPORT),
            ]),
        },
        Script {
            cue: Agent::B.cue().into(),
            turns: Box::new([
                delegate(BENCHMARK_SPEC, 2, 0, 0, 5),
                // A grandchild needs an introduction to message the director.
                call(b"introduce", &json!({"left":CHECK,"right":BENCHMARK})),
                wait("B's benchmark is delegated."),
                report(B_REPORT),
            ]),
        },
        Script {
            cue: Agent::Benchmark.cue().into(),
            turns: Box::new([then_wait(
                call(b"message", &json!({"target":CHECK,"form":"words","words":BENCHMARK_RUNNING})),
                BENCHMARK_RUNNING,
            )]),
        },
    ])
}

/// The opener and the check share a forge, with a script for each distinct cue.
#[must_use]
pub fn step_4_1_4_3(forge: SocketAddr) -> Box<[Script]> {
    let mut scripts = step_4_1(forge).into_vec();
    scripts.extend(step_4_3(forge));
    scripts.into_boxed_slice()
}

/// Commit the question read before waiting through a worker's ending.
#[must_use]
pub fn step_4_6(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        HANG, HANG_PERSON, HANG_READ, HANG_REPORT, PHASE, PHASE_ANSWER, PHASE_QUESTION, PHASE_QUESTION_NUMBER,
        PHASE_REPORT, PHASE_SPEC,
    };
    Box::new([
        Script {
            cue: HANG_PERSON.as_bytes().into(),
            turns: Box::new([
                owner(PHASE_SPEC, false),
                wait("A's next phase is delegated."),
                // Consult the project's phase notes before directing A. Unlike
                // wait, the completed tool turn commits the question read.
                {
                    let mut consulted = call(
                        b"recall",
                        &json!({"kind":"search","scopes":[{"kind":"project","project":super::PROJECT}],"query":"Export's next phase"}),
                    );
                    let mut lines = vec![Line::Text { text: HANG_READ.as_bytes().into() }];
                    lines.extend(consulted.lines);
                    consulted.lines = lines.into();
                    consulted
                },
                wait("The committed question waits for its answer."),
                call(
                    b"message",
                    &json!({"target":PHASE,"form":"answer","question":PHASE_QUESTION_NUMBER,"words":PHASE_ANSWER}),
                ),
                wait("The next phase has its answer."),
                report(HANG_REPORT),
            ]),
        },
        Script {
            cue: PHASE_SPEC.as_bytes().into(),
            turns: Box::new([
                call(b"message", &json!({"target":HANG,"form":"question","words":PHASE_QUESTION})),
                wait("The next phase awaits its answer."),
                report(PHASE_REPORT),
            ]),
        },
    ])
}

// Act 2, step 5.1 (domain/testing.md, 5.3)
#[must_use]
pub fn step_5_1(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        RULES, RULES_ANSWERED, RULES_DELEGATED, RULES_REPORT, SCOPE, SCOPE_ANSWER, SCOPE_ASKED, SCOPE_QUESTION,
        SCOPE_QUESTION_HOLD, SCOPE_QUESTION_NUMBER, SCOPE_REPORT, SCOPE_SPEC,
    };
    use skein_fake_llm_domain::api::Piece;
    let mut question = call(b"message", &json!({"target":RULES,"form":"question","words":SCOPE_QUESTION}));
    let mut lines = vec![Line::Pieces {
        pieces: Box::new([Piece {
            text: b"Ask x3's scope after the director is held.".as_slice().into(),
            hold: Some(SCOPE_QUESTION_HOLD.into()),
        }]),
    }];
    lines.extend(question.lines);
    question.lines = lines.into();
    let director: Box<[Turn]> = Box::new([
        owner(SCOPE_SPEC, false),
        wait(RULES_DELEGATED),
        call(
            b"message",
            &json!({"target":SCOPE,"form":"answer","question":SCOPE_QUESTION_NUMBER,"words":SCOPE_ANSWER}),
        ),
        wait(RULES_ANSWERED),
        report(RULES_REPORT),
    ]);
    Box::new([
        // The committed delegation in the fresh brief selects the resumed
        // script before the ordinary director cue. Its Begin waits, instead
        // of repeating the delegation while A's cancelled turn resumes.
        Script { cue: format!("result: Delegated tasks: {SCOPE}").into_bytes().into(), turns: director.clone() },
        Script { cue: super::Agent::Rules.cue().into(), turns: director },
        Script {
            cue: super::Agent::Scope.cue().into(),
            turns: Box::new([question, wait(SCOPE_ASKED), report(SCOPE_REPORT)]),
        },
    ])
}

#[must_use]
pub fn step_5_1_rules() -> Box<[Box<[Rule]>]> {
    // The trial with counted turns delegated again after Release: the fresh
    // conversation resets its assistant count. Select only the director by
    // fresh input; A retains its counted conversation across its parked wait.
    let director = [
        (super::SCOPE_REPORT, 4),
        (CROSSED, 4),
        (super::SCOPE_QUESTION, 2),
        (MESSAGE, 3),
        (DELEGATED, 1),
        (DELIVERY, 1),
        (BEGIN, 0),
        ("", 1),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    let mut resumed = director.clone();
    for rule in &mut resumed {
        if rule.cue.as_ref() == BEGIN.as_bytes() {
            rule.turn = 1;
        }
    }
    Box::new([Box::new(resumed), Box::new(director), Box::new([])])
}

// Act 2, step 5.3 (domain/testing.md, 5.3)
#[must_use]
pub fn step_5_3(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        MOVER, MOVER_AMENDMENT_REVISION, MOVER_ASKED, MOVER_QUESTION, MOVER_REPORT, MOVER_SPEC, NEW_BRIEF, SHAPE,
        SHAPE_AMENDED, SHAPE_DELEGATED, SHAPE_REASON, SHAPE_REPORT,
    };
    Box::new([
        Script {
            cue: Agent::Shape.cue().into(),
            turns: Box::new([
                call(
                    b"delegate",
                    &json!({"batch":[{
                        "executor":{"kind":"agent","charter":2},"tracked":{"priority":1},
                        "spec":{"words":MOVER_SPEC,"parameters":[]},"contract":{"kind":"report","words":128},
                        "authority":{"tools":3,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
                            "budget":{"spend":20},"notes":0}
                    }]}),
                ),
                wait(SHAPE_DELEGATED),
                call(
                    b"amend",
                    &json!({"target":MOVER,"revision":MOVER_AMENDMENT_REVISION - 1,
                    "amendment":{"instructions":NEW_BRIEF,"reason":SHAPE_REASON}}),
                ),
                wait(SHAPE_AMENDED),
                report(SHAPE_REPORT),
            ]),
        },
        Script {
            cue: Agent::Mover.cue().into(),
            turns: Box::new([
                call(b"message", &json!({"target":SHAPE,"form":"question","words":MOVER_QUESTION})),
                wait(MOVER_ASKED),
                report(MOVER_REPORT),
            ]),
        },
    ])
}

// Act 2, step 5.2 (domain/testing.md, 5.3)
#[must_use]
pub fn step_5_2_rules() -> Box<[Box<[Rule]>]> {
    use super::{BOWNER, BOWNER_REPORT, H1, HELPER, IMPLEMENTER, IMPLEMENTER_RESULT, OWNER};
    // Cancelling the helper wakes the chat before B ends. Observed in the
    // cluster run: counting that wake as B's report finishes the chat early.
    let pause = [
        (BOWNER_REPORT, 5),
        (CROSSED, 5),
        (&format!("{DELEGATED}{BOWNER}"), 3),
        (&format!("{DELEGATED}{HELPER}"), 2),
        (&format!("{DELEGATED}{OWNER}"), 1),
        (BEGIN, 0),
        ("", 4),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    // Current-head CI wakes do not replace either delegate's final result.
    let owner = [
        (IMPLEMENTER_RESULT, 3),
        (&format!("Result from task {H1}:"), 3),
        (CROSSED, 3),
        (&format!("{DELEGATED}{IMPLEMENTER}"), 1),
        (&format!("{DELEGATED}{H1}"), 2),
        (BEGIN, 0),
        ("", 2),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([Box::new(pause), Box::new([]), Box::new(owner), Box::new([]), Box::new([]), Box::new([]), Box::new([])])
}

#[must_use]
pub fn step_5_2(forge: SocketAddr) -> Box<[Script]> {
    use super::{
        Agent, BOWNER_REPORT, BOWNER_SPEC, H1_DOC, H1_LINE, H1_SPEC, H1_WORDS, HELPER, HELPER_CHARTER, HELPER_REASON,
        HELPER_SPEC, IMPLEMENTER_HOLD, IMPLEMENTER_RESULT, IMPLEMENTER_SPEC, OWNER_REPORT, OWNER_SPEC, PAUSE_PROPOSAL,
        PAUSE_REPORT, REVIEW_HOLD, REVIEWED,
    };
    use skein_fake_llm_domain::api::Piece;
    let agent = |spec: &str, charter, spend, tools, tasks, depth, kinds: Value, resources: Value| {
        json!({
            "executor":{"kind":"agent","charter":charter},
            "spec":{"words":spec,"parameters":[]},"contract":{"kind":"report","words":128},
            "authority":{"tools":tools,"grants":resources,
                "delegation":{"kinds":kinds,"tasks":tasks,"depth":depth},"budget":{"spend":spend},"notes":0}
        })
    };
    let mut b = agent(
        BOWNER_SPEC,
        1,
        25,
        3,
        4,
        3,
        json!([{"kind":"agent","number":1},{"kind":"agent","number":4},{"kind":"procedure","number":2}]),
        grants(forge, &[2, 3, 4, 7, 8, 10, 11, 12, 13]),
    );
    b["tracked"] = json!({"priority":1});
    let a = agent(OWNER_SPEC, 1, 5, 0, 0, 0, json!([]), json!([]));
    let helper = agent(HELPER_SPEC, HELPER_CHARTER, 5, 0, 0, 0, json!([]), json!([]));
    let implementer = agent(IMPLEMENTER_SPEC, 1, 5, 0, 0, 0, json!([]), json!([]));
    let change = json!({
        "executor":{"kind":"procedure","connector":1,"code":2},
        "spec":{"words":H1_SPEC,"parameters":[
            {"name":1,"kind":"resource","connector":1,"resource":temper_engine_domain_forge::tag(1, crate::forge::ID)},
            {"name":2,"kind":"bytes","value":"main"}]},
        "contract":{"kind":"change","connector":1,"change_kind":1,"words":128},
        "authority":{"tools":1,"grants":grants(forge,&[2,3,4,10,11,12]),
            "delegation":{"kinds":[{"kind":"agent","number":1},{"kind":"agent","number":4}],"tasks":2,"depth":2},
            "budget":{"spend":10},"notes":0}
    });
    let held = |name: &[u8], words: &str, mut turn: Turn| {
        let mut lines =
            vec![Line::Pieces { pieces: Box::new([Piece { text: words.as_bytes().into(), hold: Some(name.into()) }]) }];
        lines.extend(turn.lines);
        turn.lines = lines.into();
        turn
    };
    Box::new([
        Script {
            cue: Agent::Pause.cue().into(),
            turns: Box::new([
                call(b"delegate", &json!({"batch":[a]})),
                call(b"delegate", &json!({"batch":[helper]})),
                call(b"delegate", &json!({"batch":[b]})),
                call(b"cancel", &json!({"target":HELPER,"reason":HELPER_REASON})),
                wait(PAUSE_PROPOSAL),
                report(PAUSE_REPORT),
            ]),
        },
        Script { cue: Agent::Owner.cue().into(), turns: Box::new([report(OWNER_REPORT)]) },
        Script {
            cue: Agent::Bowner.cue().into(),
            turns: Box::new([
                call(b"delegate", &json!({"batch":[implementer]})),
                call(b"delegate", &json!({"batch":[change]})),
                wait("B's implementer and h1 are delegated."),
                report(BOWNER_REPORT),
            ]),
        },
        Script { cue: Agent::Helper.cue().into(), turns: Box::new([wait("The old helper waits.")]) },
        Script {
            cue: Agent::Implementer.cue().into(),
            turns: Box::new([held(IMPLEMENTER_HOLD, "The implementer's run is held.", report(IMPLEMENTER_RESULT))]),
        },
        Script {
            cue: H1_SPEC.as_bytes().into(),
            turns: Box::new([
                call(b"write", &json!({"path":H1_DOC,"content":H1_WORDS})),
                call(b"finish", &json!({"form":"change","fields":{"title":"h1","body":H1_LINE}})),
            ]),
        },
        Script {
            cue: Agent::Review.cue().into(),
            turns: Box::new([held(
                REVIEW_HOLD,
                "h1's review is held.",
                call(b"finish", &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]})),
            )]),
        },
    ])
}

// Act 2, step 5.5 (domain/testing.md, 5.3)
#[must_use]
pub fn step_5_5(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        A_DEADLINE, A1_DEADLINE, PLAN_ANSWER, PLAN_ANSWER_HOLD, PLAN_ANSWER_PENDING, PLAN_DIRECTOR, PLAN_NOTE,
        PLAN_NOTE_NUMBER, PLAN_QUESTION_NUMBER, PLAN_REPORT, PLAN_WORDS, PLANNER, PLANNER_REPORT, PLANNER_SPEC,
        PROJECT, SUBOWNER_REPORT, SUBOWNER_SPEC,
    };
    use skein_fake_llm_domain::api::Piece;
    let member = |spec: &str, spend, tools, notes, tasks, depth, deadline| {
        json!({
            "executor":{"kind":"agent","charter":2},
            "spec":{"words":spec,"parameters":[]},"contract":{"kind":"report","words":128},
            "authority":{"tools":tools,"grants":[],"notes":notes,
                "delegation":{"kinds":[{"kind":"agent","number":2}],"tasks":tasks,"depth":depth},
                "budget":{"spend":spend,"deadline":deadline}}
        })
    };
    let mut mission = member(PLANNER_SPEC, 20, 0x0183, 2, 2, 2, A_DEADLINE);
    mission["tracked"] = json!({"priority":1});
    let mut answer = call(
        b"message",
        &json!({"target":PLANNER,"form":"answer",
        "question":PLAN_QUESTION_NUMBER,"words":PLAN_ANSWER}),
    );
    let mut lines = vec![Line::Pieces {
        pieces: Box::new([
            Piece { text: PLAN_ANSWER_PENDING.as_bytes().into(), hold: None },
            Piece { text: b"The answer is released.".as_slice().into(), hold: Some(PLAN_ANSWER_HOLD.into()) },
        ]),
    }];
    lines.extend(answer.lines);
    answer.lines = lines.into();
    Box::new([
        Script {
            cue: Agent::Plan.cue().into(),
            turns: Box::new([
                call(b"delegate", &json!({"batch":[mission]})),
                wait("A is planning."),
                answer,
                wait("A has its green light."),
                report(PLAN_REPORT),
            ]),
        },
        Script {
            cue: Agent::Planner.cue().into(),
            turns: Box::new([
                call(
                    b"note",
                    &json!({"scope":{"kind":"project","project":PROJECT},
                "description":PLAN_NOTE,"body":PLAN_WORDS}),
                ),
                call(b"message", &json!({"target":PLAN_DIRECTOR,"form":"question","words":PLAN_WORDS})),
                wait("A's plan awaits its answer."),
                call(b"recall", &json!({"kind":"name","name":PLAN_NOTE_NUMBER})),
                call(b"delegate", &json!({"batch":[member(SUBOWNER_SPEC, 5, 1, 0, 1, 0, A1_DEADLINE)]})),
                wait("M1's sub-owner is delegated."),
                report(PLANNER_REPORT),
            ]),
        },
        Script {
            cue: Agent::Subowner.cue().into(),
            turns: Box::new([
                call(
                    b"delegate",
                    // The helper asks no spend, so depth 0 is the batch's only deficit.
                    &json!({"batch":[member("A helper under M1's sub-owner.", 0, 0, 0, 0, 0, A1_DEADLINE)]}),
                ),
                report(SUBOWNER_REPORT),
            ]),
        },
    ])
}

#[must_use]
pub fn step_5_5_rules() -> Box<[Box<[Rule]>]> {
    use super::{PLAN_ANSWER, PLAN_NOTE_NUMBER, PLAN_QUESTION_NUMBER, SUBOWNER, SUBOWNER_REPORT};
    // The counted stress trial did not settle: a fresh conversation resets
    // the assistant count, as in 5.1. Follow A's new input across its parks;
    // the director and A1 keep counted turns.
    let planner = [
        (SUBOWNER_REPORT, 6),
        (CROSSED, 6),
        (&format!("{DELEGATED}{SUBOWNER}"), 5),
        (&format!("Note {PLAN_NOTE_NUMBER} revision 1: "), 4),
        (PLAN_ANSWER, 3),
        (&format!("Message {PLAN_QUESTION_NUMBER} sent"), 2),
        (&format!("Note {PLAN_NOTE_NUMBER} revision 1 written"), 1),
        (BEGIN, 0),
        ("", 2),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([Box::new([]), Box::new(planner), Box::new([])])
}

// Act 2, step 5.6 (domain/testing.md, 5.3)
#[must_use]
pub fn step_5_6(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        MISREAD_HELPER, MISREAD_HELPER_HOLD, MISREAD_HELPER_PENDING, MISREAD_HELPER_SPEC, MISREAD_NOTE, MISREAD_REASON,
        MISREAD_RECALL_HOLD, MISREAD_REPORT, MISREAD_RULE, MISREAD_RULE_PENDING, MISREAD_TIMER_AT, PROJECT,
    };
    use skein_fake_llm_domain::api::Piece;
    let held = |marker: &str, hold: &[u8], mut turn: Turn| {
        let mut lines = vec![Line::Pieces {
            pieces: Box::new([
                Piece { text: marker.as_bytes().into(), hold: None },
                Piece { text: b"Continue.".as_slice().into(), hold: Some(hold.into()) },
            ]),
        }];
        lines.extend(turn.lines);
        turn.lines = lines.into();
        turn
    };
    Box::new([
        Script {
            cue: Agent::Misread.cue().into(),
            turns: Box::new([
                call(b"subscribe", &json!({"kind":"timer","at":MISREAD_TIMER_AT})),
                call(
                    b"delegate",
                    &json!({"batch":[{
                        "executor":{"kind":"agent","charter":1},"tracked":{"priority":1},
                        "spec":{"words":MISREAD_HELPER_SPEC,"parameters":[]},"contract":{"kind":"report","words":128},
                        "authority":{"tools":0,"grants":[],"notes":0,
                            "delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":5}}
                    }]}),
                ),
                wait("A helper is acting on the priorities; the timer remains armed."),
                call(b"cancel", &json!({"target":MISREAD_HELPER,"reason":MISREAD_REASON})),
                call(
                    b"note",
                    &json!({"scope":{"kind":"project","project":PROJECT},
                "description":MISREAD_NOTE,"body":MISREAD_RULE}),
                ),
                held(
                    MISREAD_RULE_PENDING,
                    MISREAD_RECALL_HOLD,
                    call(
                        b"recall",
                        &json!({"kind":"search","scopes":[{"kind":"project","project":PROJECT}],
                    "query":MISREAD_NOTE}),
                    ),
                ),
                call(b"read_branch", &json!({"repository":"acme/app","branch":"main"})),
                report(MISREAD_REPORT),
            ]),
        },
        Script {
            cue: Agent::MisreadHelper.cue().into(),
            turns: Box::new([held(
                MISREAD_HELPER_PENDING,
                MISREAD_HELPER_HOLD,
                report("The helper has landed nothing."),
            )]),
        },
    ])
}

#[must_use]
pub fn step_5_6_rules() -> Box<[Box<[Rule]>]> {
    use super::{MISREAD_CORRECTION, MISREAD_HELPER, MISREAD_NOTE, MISREAD_RULE, MISREAD_TIMER};
    // The counted stress trial restarted the opening on the person's later
    // message: fresh runs reset the assistant count. Only the director needs
    // input rules; the helper has one held turn, then ends with cancellation.
    let director = [
        (CROSSED, 7),
        (&format!(": {MISREAD_NOTE}\n{MISREAD_RULE}\n"), 6),
        (&super::draft_commit(), 7),
        (" revision 1 written", 5),
        ("Done", 4),
        (MISREAD_CORRECTION, 3),
        (&format!("Subscription {MISREAD_TIMER} added"), 1),
        (&format!("{DELEGATED}{MISREAD_HELPER}"), 2),
        (BEGIN, 0),
        ("", 2),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([Box::new(director), Box::new([])])
}

// Act 2, step 5.4 (domain/testing.md, 5.3)
fn direction_owner(forge: SocketAddr, spec: &str, spend: u64, notes: u32, tasks: u32) -> Value {
    json!({
        "executor":{"kind":"agent","charter":1},"tracked":{"priority":1},
        "spec":{"words":spec,"parameters":[]},"contract":{"kind":"report","words":128},
        "authority":{"tools":391,"notes":notes,"grants":grants(forge,&[2,3,4,10,11,12]),
            "delegation":{"kinds":[{"kind":"agent","number":1},{"kind":"agent","number":4},{"kind":"procedure","number":2}],"tasks":tasks,"depth":3},
            "budget":{"spend":spend}}
    })
}
fn direction_change(forge: SocketAddr, spec: &str, path: &str, tasks: u32) -> Value {
    repository_change(forge, crate::forge::ID, spec, path, tasks)
}
fn repository_change(forge: SocketAddr, repository: u32, spec: &str, path: &str, tasks: u32) -> Value {
    json!({
        "executor":{"kind":"procedure","connector":1,"code":2},
        "spec":{"words":spec,"parameters":[
            {"name":1,"kind":"resource","connector":1,"resource":temper_engine_domain_forge::tag(1, repository)},
            {"name":2,"kind":"bytes","value":"main"},
            {"name":6,"kind":"bytes","value":path}]},
        "contract":{"kind":"change","connector":1,"change_kind":1,"words":128},
        "authority":{"tools":1,"notes":0,"grants":grants(forge,&[2,3,4,10,11,12]),
            "delegation":{"kinds":[{"kind":"agent","number":1},{"kind":"agent","number":4}],"tasks":tasks,"depth":2},"budget":{"spend":10}}
    })
}
fn direction_held(name: &[u8], mut turn: Turn) -> Turn {
    use skein_fake_llm_domain::api::Piece;
    let mut lines = vec![Line::Pieces {
        pieces: Box::new([Piece { text: super::DIRECTION_HELD_WORDS.as_bytes().into(), hold: Some(name.into()) }]),
    }];
    lines.extend(turn.lines);
    turn.lines = lines.into();
    turn
}

#[must_use]
pub fn step_5_4(forge: SocketAddr) -> Box<[Script]> {
    use super::{
        B_DELEGATE_HOLD, B_FILE, B_WORDS, DEFERRED_NOTE, DEFERRED_NOTE_NUMBER, DEFERRED_WORDS, DIRECTION_BRIEF,
        DIRECTION_REASON, DIRECTION_REPORT, DIRECTION_REVISION, OVERLAP_OWNER_REPORT, OVERLAP_OWNER_SPEC, PLANNED,
        PLANNED_CANCEL_REASON, PLANNED_CODING_HOLD, PLANNED_SPEC, PROJECT, STORY_CHANGE, STORY_GREEN, STORY_OWNER,
        STORY_OWNER_REPORT, STORY_OWNER_SPEC, STORY_RECALL_HOLD, STORY_SPEC, STORY_WORDS, X3, X3_CANCEL_REASON,
        X3_FILE, X3_REVIEW_HOLD, X3_SPEC, X3_WORDS,
    };
    let verdict = || call(b"finish", &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]}));
    Box::new([
        Script {
            cue: Agent::Direction.cue().into(),
            turns: Box::new([
                then_wait(
                    call(
                        b"delegate",
                        &json!({"batch":[direction_owner(forge, STORY_OWNER_SPEC,30,2,6),direction_owner(forge, OVERLAP_OWNER_SPEC,20,0,2)]}),
                    ),
                    "A and B own kit's work.",
                ),
                then_wait(
                    call(
                        b"amend",
                        &json!({"target":STORY_OWNER,"revision":DIRECTION_REVISION-1,"amendment":{"instructions":DIRECTION_BRIEF,"reason":DIRECTION_REASON}}),
                    ),
                    "A has the new direction.",
                ),
                report(DIRECTION_REPORT),
                wait("The director is waiting for direction or the owners' reports."),
            ]),
        },
        Script {
            cue: Agent::StoryOwner.cue().into(),
            turns: Box::new([
                call(b"delegate", &json!({"batch":[direction_change(forge, X3_SPEC,X3_FILE,2)]})),
                wait("x3's review waits."),
                call(b"cancel", &json!({"target":X3,"reason":X3_CANCEL_REASON})),
                call(
                    b"note",
                    &json!({"scope":{"kind":"project","project":PROJECT},"description":DEFERRED_NOTE,"body":DEFERRED_WORDS,"tags":["deferred"]}),
                ),
                wait("x3's cancellation is settling before a1 starts."),
                call(b"delegate", &json!({"batch":[direction_change(forge, STORY_SPEC,B_FILE,2)]})),
                wait("a1 is delegated."),
                direction_held(STORY_RECALL_HOLD, call(b"recall", &json!({"kind":"name","name":DEFERRED_NOTE_NUMBER}))),
                report(STORY_OWNER_REPORT),
            ]),
        },
        Script {
            cue: Agent::OverlapOwner.cue().into(),
            turns: Box::new([
                direction_held(
                    B_DELEGATE_HOLD,
                    then_wait(
                        call(b"delegate", &json!({"batch":[direction_change(forge, PLANNED_SPEC,B_FILE,1)]})),
                        "h1's planned path waits.",
                    ),
                ),
                then_wait(
                    call(b"cancel", &json!({"target":PLANNED,"reason":PLANNED_CANCEL_REASON})),
                    "h1's cancellation is settling.",
                ),
                report(OVERLAP_OWNER_REPORT),
            ]),
        },
        Script {
            cue: Agent::X3Writer.cue().into(),
            turns: Box::new([
                call(b"write", &json!({"path":X3_FILE,"content":X3_WORDS})),
                call(b"finish", &json!({"form":"change","fields":{"title":"x3","body":"The deferred plan."}})),
            ]),
        },
        Script {
            cue: Agent::PlannedWriter.cue().into(),
            turns: Box::new([direction_held(PLANNED_CODING_HOLD, report("h1 did not produce a change."))]),
        },
        Script {
            cue: Agent::StoryWriter.cue().into(),
            turns: Box::new([
                call(b"edit", &json!({"path":B_FILE,"edits":[{"old":B_WORDS,"new":STORY_WORDS}]})),
                call(b"finish", &json!({"form":"change","fields":{"title":"a1","body":STORY_GREEN}})),
            ]),
        },
        // Each review starts with a fresh transcript. Select x3's by its
        // requester in the system brief; a1's review needs no hold.
        Script {
            cue: format!("[Requested by task {X3}]").into_bytes().into(),
            turns: Box::new([direction_held(X3_REVIEW_HOLD, verdict())]),
        },
        Script { cue: format!("[Requested by task {STORY_CHANGE}]").into_bytes().into(), turns: Box::new([verdict()]) },
    ])
}

/// B's hold can wake the director before the person sends the new direction.
/// Follow delivered input so notices cannot consume the amendment or report.
/// A's subtree also installs its current-head CI topics; those wakes must wait
/// for the amendment instead of advancing its counted cancellation script.
#[must_use]
pub fn step_5_4_rules() -> Box<[Box<[Rule]>]> {
    use super::{
        DEFERRED_NOTE_NUMBER, DIRECTION_BRIEF, DIRECTION_MESSAGE, DIRECTION_REVISION, OVERLAP_OWNER, STORY_CHANGE,
        STORY_OWNER, STORY_OWNER_REPORT, X3,
    };
    let director = [
        (STORY_OWNER_REPORT, 2),
        (CROSSED, 2),
        (&format!("Done: task {STORY_OWNER} is at spec revision {DIRECTION_REVISION}"), 3),
        (DIRECTION_MESSAGE, 1),
        ("Held task needs a decision", 3),
        (&format!("{DELEGATED}{STORY_OWNER}, {OVERLAP_OWNER}"), 3),
        (BEGIN, 0),
        ("", 3),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    let owner = [
        (&format!("Note {DEFERRED_NOTE_NUMBER} revision 1: ") as &str, 8),
        // Landing news may cross the report after the note has been recalled.
        (CROSSED, 8),
        (&format!("Result from task {STORY_CHANGE}:"), 7),
        (&format!("Result from task {X3}:"), 5),
        (&format!("Note {DEFERRED_NOTE_NUMBER} revision 1 written"), 4),
        ("Done", 3),
        (DIRECTION_BRIEF, 2),
        (&format!("{DELEGATED}{STORY_CHANGE}"), 6),
        (&format!("{DELEGATED}{X3}"), 1),
        ("forge ci ", 1),
        (BEGIN, 0),
        ("", 1),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([
        Box::new(director),
        Box::new(owner),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    ])
}

// Act 3, step 6.1 (domain/testing.md, 5.3)
fn batch_member(agent: Agent, spend: u64, tasks: u32, depth: u32, deadline: u64) -> Value {
    json!({
        "executor":{"kind":"agent","charter":1},
        "spec":{"words":String::from_utf8_lossy(agent.cue()),"parameters":[]},
        "contract":{"kind":"report","words":128},
        "authority":{"tools":if depth == 0 {0} else {3},"grants":[],"notes":0,
            "delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":tasks,"depth":depth},
            "budget":{"spend":spend,"deadline":deadline}}
    })
}
fn batch_held(hold: &[u8], mut turn: Turn) -> Turn {
    use skein_fake_llm_domain::api::Piece;
    let mut lines = vec![Line::Pieces {
        pieces: Box::new([
            Piece { text: super::BATCH_WRITING.as_bytes().into(), hold: None },
            Piece { text: b" Report follows.".as_slice().into(), hold: Some(hold.into()) },
        ]),
    }];
    lines.extend(turn.lines);
    turn.lines = lines.into();
    turn
}

#[must_use]
pub fn step_6_1(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        A_DEADLINE, A1_DEADLINE, BATCH_ANSWER, BATCH_DIRECTOR_REPORT, BATCH_HOLDS, BATCH_OWNER, BATCH_OWNER_REPORT,
        BATCH_QUESTION, BATCH_QUESTION_NUMBER, BATCH_REPORTS, BATCH_SUBOWNER, BATCH_SUBOWNER_REPORT,
    };
    let mut owner = batch_member(Agent::BatchOwner, 61, 5, 2, A_DEADLINE);
    owner["tracked"] = json!({"priority":1});
    let agents = [Agent::BatchOne, Agent::BatchTwo, Agent::BatchThree, Agent::BatchStory];
    let mut batch: Vec<_> = agents.iter().map(|agent| batch_member(*agent, 10, 0, 0, A1_DEADLINE)).collect();
    batch[3]["dependencies"] = json!([{"kind":"batch","number":0},{"kind":"batch","number":1}]);
    let mut scripts = vec![
        Script {
            cue: Agent::BatchDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A owns M1."),
                report(BATCH_DIRECTOR_REPORT),
                wait("A still owns M1."),
            ]),
        },
        Script {
            cue: Agent::BatchOwner.cue().into(),
            turns: Box::new([
                then_wait(
                    call(b"delegate", &json!({"batch":[batch_member(Agent::BatchSubowner,51,4,1,A1_DEADLINE)]})),
                    "A1 owns M1's design.",
                ),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BATCH_SUBOWNER,"form":"answer","question":BATCH_QUESTION_NUMBER,"words":BATCH_ANSWER}),
                    ),
                    "The design has its green light.",
                ),
                report(BATCH_OWNER_REPORT),
                wait("A1 is still working."),
            ]),
        },
        Script {
            cue: Agent::BatchSubowner.cue().into(),
            turns: Box::new([
                then_wait(
                    call(b"message", &json!({"target":BATCH_OWNER,"form":"question","words":BATCH_QUESTION})),
                    "The design awaits A's answer.",
                ),
                then_wait(call(b"delegate", &json!({"batch":batch})), "The batch is delegated."),
                report(BATCH_SUBOWNER_REPORT),
                wait("The remaining batch delegates are still working."),
            ]),
        },
    ];
    for ((agent, hold), report_words) in agents.into_iter().zip(BATCH_HOLDS).zip(BATCH_REPORTS) {
        scripts.push(Script { cue: agent.cue().into(), turns: Box::new([batch_held(hold, report(report_words))]) });
    }
    scripts.into()
}

#[must_use]
pub fn step_6_1_rules() -> Box<[Box<[Rule]>]> {
    use super::{BATCH_ANSWER, BATCH_OWNER_REPORT, BATCH_QUESTION, BATCH_REPORTS, BATCH_SUBOWNER_REPORT};
    // Results can wake A1 separately or together; held-task notices can
    // reach a requester between its expected inputs (5.4's director).
    let director =
        [(BATCH_OWNER_REPORT, 1), (CROSSED, 1), (DELEGATED, 2), ("Held task needs a decision", 2), (BEGIN, 0), ("", 2)]
            .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    let owner = [
        (BATCH_SUBOWNER_REPORT, 2),
        (CROSSED, 2),
        (BATCH_QUESTION, 1),
        (MESSAGE, 3),
        (DELEGATED, 3),
        ("Held task needs a decision", 3),
        (BEGIN, 0),
        ("", 3),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    let subowner = [
        (BATCH_REPORTS[3], 2),
        (CROSSED, 2),
        (BATCH_ANSWER, 1),
        (MESSAGE, 3),
        (DELEGATED, 3),
        ("Result from task", 3),
        ("Held task needs a decision", 3),
        (BEGIN, 0),
        ("", 3),
    ]
    .map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn });
    Box::new([
        Box::new(director),
        Box::new(owner),
        Box::new(subowner),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    ])
}

// Act 3, step 6.2 (domain/testing.md, 5.3)
fn blocker_owner(forge: SocketAddr, agent: Agent, spend: u64, tasks: u32, depth: u32, deadline: u64) -> Value {
    let mut member = batch_member(agent, spend, tasks, depth, deadline);
    member["authority"]["tools"] = 7.into();
    member["authority"]["grants"] = grants(forge, &[2, 3, 4, 10, 11, 12]);
    member["authority"]["delegation"]["kinds"] = json!([{"kind":"agent","number":1},{"kind":"agent","number":2},{"kind":"agent","number":4},{"kind":"procedure","number":2}]);
    member
}

#[must_use]
pub fn step_6_2(forge: SocketAddr) -> Box<[Script]> {
    use super::{
        Agent, BLOCKER_A3, BLOCKER_A3_REPORT, BLOCKER_ANSWER, BLOCKER_DIRECTOR, BLOCKER_DIRECTOR_REPORT, BLOCKER_E1,
        BLOCKER_FINDING, BLOCKER_KEPT_REPORT, BLOCKER_OWNER, BLOCKER_OWNER_REPORT, BLOCKER_QUESTION,
        BLOCKER_QUESTION_NUMBER, BLOCKER_REASON, BLOCKER_REVIEW_HOLD, BLOCKER_REVISION, BLOCKER_SUBOWNER,
        BLOCKER_SUBOWNER_REPORT, BLOCKER_UP_ANSWER, BLOCKER_UP_QUESTION, BLOCKER_UP_QUESTION_NUMBER, REVIEWED,
    };
    let [owner, subowner, a3, e1] = blocker_members(forge);
    Box::new([
        Script {
            cue: Agent::BlockerDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A owns the blocker."),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BLOCKER_OWNER,"form":"answer",
                "question":BLOCKER_UP_QUESTION_NUMBER,"words":BLOCKER_UP_ANSWER}),
                    ),
                    "A has the director's answer.",
                ),
                report(BLOCKER_DIRECTOR_REPORT),
                wait("A is still working."),
            ]),
        },
        Script {
            cue: Agent::BlockerOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[subowner]})), "A1 owns the blocker."),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BLOCKER_DIRECTOR,"form":"question",
                "words":BLOCKER_UP_QUESTION}),
                    ),
                    "The director's decision waits.",
                ),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BLOCKER_SUBOWNER,"form":"answer",
                "question":BLOCKER_QUESTION_NUMBER,"words":BLOCKER_ANSWER}),
                    ),
                    "A1 has the answer.",
                ),
                report(BLOCKER_OWNER_REPORT),
                wait("A1 is still working."),
            ]),
        },
        Script {
            cue: Agent::BlockerSubowner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[a3]})), "a3 implements."),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BLOCKER_OWNER,"form":"question",
                "words":BLOCKER_QUESTION}),
                    ),
                    "The stopped owner's API needs direction.",
                ),
                call(b"delegate", &json!({"batch":[e1]})),
                then_wait(
                    call(
                        b"amend",
                        &json!({"target":BLOCKER_A3,"revision":BLOCKER_REVISION-1,
                "amendment":{"dependencies":[BLOCKER_E1],"reason":BLOCKER_REASON}}),
                    ),
                    "e1 keeps a3 pending.",
                ),
                report(BLOCKER_SUBOWNER_REPORT),
                wait("a3 is still working."),
            ]),
        },
        Script {
            cue: Agent::BlockerImplementer.cue().into(),
            turns: Box::new([
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BLOCKER_SUBOWNER,"form":"words",
                "words":BLOCKER_FINDING}),
                    ),
                    "a3 awaits the blocker decision.",
                ),
                report(BLOCKER_KEPT_REPORT),
                report(BLOCKER_A3_REPORT),
                wait("a3 awaits e1."),
            ]),
        },
        blocker_coder(),
        Script {
            cue: format!("[Requested by task {BLOCKER_E1}]").into_bytes().into(),
            turns: Box::new([direction_held(
                BLOCKER_REVIEW_HOLD,
                call(b"finish", &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]})),
            )]),
        },
    ])
}

#[must_use]
pub fn step_6_2_rules() -> Box<[Box<[Rule]>]> {
    use super::{
        BLOCKER_A3_REPORT, BLOCKER_ANSWER, BLOCKER_E1, BLOCKER_FINDING, BLOCKER_OWNER_REPORT, BLOCKER_QUESTION,
        BLOCKER_REASON, BLOCKER_SUBOWNER_REPORT, BLOCKER_UP_ANSWER, BLOCKER_UP_QUESTION,
    };
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[(BLOCKER_OWNER_REPORT, 2), (CROSSED, 2), (BLOCKER_UP_QUESTION, 1), (BEGIN, 0), ("", 3)]),
        table(&[
            (BLOCKER_SUBOWNER_REPORT, 3),
            (CROSSED, 3),
            (BLOCKER_UP_ANSWER, 2),
            (BLOCKER_QUESTION, 1),
            (BEGIN, 0),
            ("", 4),
        ]),
        table(&[
            (BLOCKER_A3_REPORT, 4),
            (CROSSED, 4),
            (&format!("{DELEGATED}{BLOCKER_E1}"), 3),
            (BLOCKER_ANSWER, 2),
            (BLOCKER_FINDING, 1),
            (BEGIN, 0),
            ("", 5),
        ]),
        table(&[
            (&format!("T{BLOCKER_E1} is done; the task can go on"), 2),
            (BLOCKER_REASON, 1),
            (CROSSED, 2),
            (BEGIN, 0),
            ("", 3),
        ]),
        Box::new([]),
        Box::new([]),
    ])
}

fn blocker_coder() -> Script {
    use super::{BLOCKER_FILE, BLOCKER_FIXED, BLOCKER_WRONG};
    Script {
        cue: Agent::BlockerCoder.cue().into(),
        turns: Box::new([
            call(b"read", &json!({"path":BLOCKER_FILE})),
            call(b"edit", &json!({"path":BLOCKER_FILE,"edits":[{"old":BLOCKER_WRONG,"new":BLOCKER_FIXED}]})),
            call(b"finish", &json!({"form":"change","fields":{"title":"e1","body":"Fix B's six public signatures."}})),
        ]),
    }
}

fn blocker_members(forge: SocketAddr) -> [Value; 4] {
    use super::{A_DEADLINE, A1_DEADLINE, BLOCKER_E1_SPEC, BLOCKER_FILE};
    let mut owner = blocker_owner(forge, Agent::BlockerOwner, 42, 5, 4, A_DEADLINE);
    owner["tracked"] = json!({"priority":1});
    let subowner = blocker_owner(forge, Agent::BlockerSubowner, 32, 4, 3, A1_DEADLINE);
    let mut a3 = batch_member(Agent::BlockerImplementer, 10, 0, 0, A1_DEADLINE);
    a3["authority"]["tools"] = 2.into();
    a3["executor"]["charter"] = 2.into();
    let mut e1 = direction_change(forge, BLOCKER_E1_SPEC, BLOCKER_FILE, 2);
    e1["authority"]["budget"]["deadline"] = A1_DEADLINE.into();
    [owner, subowner, a3, e1]
}

// Act 3, step 6.5 (domain/testing.md, 5.3)
#[must_use]
pub fn step_6_5(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        A_DEADLINE, A1_DEADLINE, BUDGET_A5, BUDGET_A5_REPORT, BUDGET_ADVISOR_REPORT, BUDGET_ADVISOR_SPEC,
        BUDGET_AMEND_HOLD, BUDGET_ANSWER, BUDGET_DIRECTOR, BUDGET_DIRECTOR_REPORT, BUDGET_FINDING, BUDGET_HIGH_HOLD,
        BUDGET_HIGH_NUMBER, BUDGET_MEDIUM_NUMBER, BUDGET_OWNER, BUDGET_OWNER_REPORT, BUDGET_PERSON_ANSWER,
        BUDGET_QUESTION, BUDGET_QUESTION_NUMBER, BUDGET_REASON, BUDGET_REVISION, BUDGET_SUBOWNER,
        BUDGET_SUBOWNER_REPORT, BUDGET_UP_QUESTION, BUDGET_UP_QUESTION_NUMBER, DEFAULT_MODEL_NUMBER,
        OWNER_EFFORT_NUMBER, OWNER_MODEL_NUMBER,
    };
    let mut owner = batch_member(Agent::BudgetOwner, 35, 3, 2, A_DEADLINE);
    owner["authority"]["tools"] = 7.into();
    owner["tracked"] = json!({"priority":1});
    let mut subowner = batch_member(Agent::BudgetSubowner, 25, 2, 1, A1_DEADLINE);
    subowner["authority"]["tools"] = 7.into();
    let mut a5 = batch_member(Agent::BudgetInvestigator, 10, 0, 0, A1_DEADLINE);
    a5["authority"]["tools"] = 2.into();
    a5["llm"] = json!({"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_MEDIUM_NUMBER}});
    let mut advisor = batch_member(Agent::BudgetAdvisor, 10, 0, 0, A1_DEADLINE);
    advisor["spec"]["words"] = BUDGET_ADVISOR_SPEC.into();
    advisor["llm"] = json!({"main":{"model":OWNER_MODEL_NUMBER,"effort":OWNER_EFFORT_NUMBER}});
    Box::new([
        Script {
            cue: Agent::BudgetDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A owns the budget question."),
                report(BUDGET_DIRECTOR_REPORT),
                wait("The person's inbox holds A's question; A is still working."),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BUDGET_OWNER,"form":"answer","question":BUDGET_UP_QUESTION_NUMBER,"words":BUDGET_PERSON_ANSWER}),
                    ),
                    "A has the person's answer.",
                ),
            ]),
        },
        Script {
            cue: Agent::BudgetOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[subowner]})), "A1 owns the investigation."),
                then_wait(
                    call(b"message", &json!({"target":BUDGET_DIRECTOR,"form":"question","words":BUDGET_UP_QUESTION})),
                    "The budget awaits the person's answer.",
                ),
                then_wait(
                    call(
                        b"message",
                        &json!({"target":BUDGET_SUBOWNER,"form":"answer","question":BUDGET_QUESTION_NUMBER,"words":BUDGET_ANSWER}),
                    ),
                    "A1 has the person's direction.",
                ),
                report(BUDGET_OWNER_REPORT),
                wait("A1 is still working on the cost."),
            ]),
        },
        Script {
            cue: Agent::BudgetSubowner.cue().into(),
            turns: Box::new([
                then_wait(
                    call(b"message", &json!({"target":BUDGET_OWNER,"form":"question","words":BUDGET_QUESTION})),
                    "A's budget decision waits.",
                ),
                then_wait(call(b"delegate", &json!({"batch":[a5]})), "a5 investigates at medium."),
                batch_held(
                    BUDGET_AMEND_HOLD,
                    then_wait(
                        call(
                            b"amend",
                            &json!({"target":BUDGET_A5,"revision":BUDGET_REVISION-1,"amendment":{"llm":{"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_HIGH_NUMBER}},"reason":BUDGET_REASON}}),
                        ),
                        "a5 will try again at high.",
                    ),
                ),
                then_wait(
                    call(b"delegate", &json!({"batch":[advisor]})),
                    "The advisor reads a5's report and evidence.",
                ),
                report(BUDGET_SUBOWNER_REPORT),
                wait("The investigation or advisor is still working."),
            ]),
        },
        Script {
            cue: Agent::BudgetInvestigator.cue().into(),
            turns: Box::new([
                then_wait(
                    call(b"message", &json!({"target":BUDGET_SUBOWNER,"form":"words","words":BUDGET_FINDING})),
                    "a5 awaits a next try.",
                ),
                batch_held(BUDGET_HIGH_HOLD, report(BUDGET_A5_REPORT)),
                wait("a5 awaits the amendment's reason."),
            ]),
        },
        Script { cue: Agent::BudgetAdvisor.cue().into(), turns: Box::new([report(BUDGET_ADVISOR_REPORT)]) },
    ])
}

#[must_use]
pub fn step_6_5_rules() -> Box<[Box<[Rule]>]> {
    use super::{
        BUDGET_A5_REPORT, BUDGET_ADVISOR_REPORT, BUDGET_ANSWER, BUDGET_FINDING, BUDGET_OWNER_REPORT,
        BUDGET_PERSON_ANSWER, BUDGET_QUESTION, BUDGET_REASON, BUDGET_SUBOWNER_REPORT, BUDGET_UP_QUESTION,
    };
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[
            (BUDGET_OWNER_REPORT, 1),
            (CROSSED, 1),
            (BUDGET_PERSON_ANSWER, 3),
            (BUDGET_UP_QUESTION, 2),
            (MESSAGE, 2),
            (DELEGATED, 2),
            ("Held task needs a decision", 2),
            (BEGIN, 0),
            ("", 2),
        ]),
        table(&[
            (BUDGET_SUBOWNER_REPORT, 3),
            (CROSSED, 3),
            (BUDGET_PERSON_ANSWER, 2),
            (BUDGET_QUESTION, 1),
            (MESSAGE, 4),
            (DELEGATED, 4),
            ("Held task needs a decision", 4),
            (BEGIN, 0),
            ("", 4),
        ]),
        table(&[
            (BUDGET_ADVISOR_REPORT, 4),
            (CROSSED, 4),
            (BUDGET_A5_REPORT, 3),
            (BUDGET_FINDING, 2),
            (BUDGET_ANSWER, 1),
            (MESSAGE, 5),
            (DELEGATED, 5),
            ("Held task needs a decision", 5),
            (BEGIN, 0),
            ("", 5),
        ]),
        table(&[
            (BUDGET_REASON, 1),
            (CROSSED, 1),
            (MESSAGE, 2),
            ("Held task needs a decision", 2),
            (BEGIN, 0),
            ("", 2),
        ]),
        Box::new([]),
    ])
}

// Act 3, step 6.4 (domain/testing.md, 5.3)
#[must_use]
pub fn step_6_4(forge: SocketAddr) -> Box<[Script]> {
    use super::{
        Agent, RED_A4, RED_A4_FILE, RED_BASE, RED_BROKEN, RED_CHANGE_SPEC, RED_CHANGE_SPEND, RED_FILE, RED_FIRST_HEAD,
        RED_OWNER_REPORT, RED_OWNER_SPEC, RED_OWNER_SPEND, RED_REPAIR, RED_REPAIR_HOLD, RED_REPORT, RED_REVIEW_HOLD,
        RED_RULE, RED_TIMER, RED_TIMER_AT, RED_UPDATED_HEAD, REVIEWED,
    };
    let owner = direction_owner(forge, RED_OWNER_SPEC, RED_OWNER_SPEND, 0, 4);
    let mut change = direction_change(forge, RED_CHANGE_SPEC, RED_A4_FILE, 3);
    change["authority"]["budget"]["spend"] = RED_CHANGE_SPEND.into();
    let verdict = || call(b"finish", &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]}));
    let mut interim = call(b"subscribe", &json!({"kind":"timer","at":RED_TIMER_AT}));
    let mut lines = vec![Line::Text { text: RED_RULE.as_bytes().into() }];
    lines.extend(interim.lines);
    interim.lines = lines.into();
    Box::new([
        Script {
            cue: Agent::RedDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A1 owns a4."),
                then_wait(interim, "The interim timer is armed."),
                call(b"unsubscribe", &json!({"subscription":RED_TIMER})),
                report(RED_REPORT),
                wait("A1 is still working."),
            ]),
        },
        Script {
            cue: Agent::RedOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[change]})), "a4 is delegated."),
                report(RED_OWNER_REPORT),
                wait("a4 waits for the landing branch repair."),
            ]),
        },
        Script {
            cue: Agent::RedCoder.cue().into(),
            turns: Box::new([finish_change(
                call(b"write", &json!({"path":RED_A4_FILE,"content":RED_A4})),
                "a4",
                "Preserve green CI.",
            )]),
        },
        Script { cue: review_cue(RED_FIRST_HEAD), turns: Box::new([direction_held(RED_REVIEW_HOLD, verdict())]) },
        Script {
            cue: Agent::RedRepairCoder.cue().into(),
            turns: Box::new([direction_held(
                RED_REPAIR_HOLD,
                finish_change(
                    call(b"edit", &json!({"path":RED_FILE,"edits":[{"old":RED_BROKEN,"new":RED_BASE}]})),
                    "Repair kit CI",
                    "Restore the green cue.",
                ),
            )]),
        },
        Script { cue: format!("[Requested by task {RED_REPAIR}]").into_bytes().into(), turns: Box::new([verdict()]) },
        Script { cue: review_cue(RED_UPDATED_HEAD), turns: Box::new([verdict()]) },
    ])
}

#[must_use]
pub fn step_6_4_rules() -> Box<[Box<[Rule]>]> {
    use super::{RED_CHANGE, RED_MESSAGE, RED_OWNER_REPORT, RED_TIMER};
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[
            // The final report can cross the interim timer or A1's news.
            (CROSSED, 3),
            ("Subscription removed", 3),
            (RED_OWNER_REPORT, 2),
            (&format!("Subscription {RED_TIMER} added"), 4),
            (RED_MESSAGE, 1),
            (DELEGATED, 4),
            (BEGIN, 0),
            ("", 4),
        ]),
        table(&[(&format!("Result from task {RED_CHANGE}:"), 1), (CROSSED, 1), (DELEGATED, 2), (BEGIN, 0), ("", 2)]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    ])
}

fn review_cue(head: u64) -> Box<[u8]> {
    use std::fmt::Write;
    let mut hex = String::new();
    for byte in skein_fake_forge_domain::object_bytes(head) {
        write!(hex, "{byte:02x}").expect("head hex");
    }
    format!("Pull request at head {hex}").into_bytes().into()
}

fn finish_change(mut work: Turn, title: &str, body: &str) -> Turn {
    let mut lines = work.lines.into_vec();
    lines.extend(call(b"finish", &json!({"form":"change","fields":{"title":title,"body":body}})).lines);
    work.lines = lines.into();
    work
}

// Act 3, step 6.7 (domain/testing.md, 5.3)
#[must_use]
pub fn step_6_7(_forge: SocketAddr) -> Box<[Script]> {
    use super::{
        A_DEADLINE, A1_DEADLINE, Agent, CAP_A5_HOLD, CAP_A5_REPORT, CAP_DIRECTOR_REPORT, CAP_HOLD_REVISION,
        CAP_OWNER_REPORT, CAP_PASS_HOLD, CAP_REASON, CAP_REVISION, CAP_SECONDS, CAP_SPLIT, CAP_SPLIT_HOLD,
        CAP_SUBOWNER, CAP_SUBOWNER_REPORT, json,
    };
    let mut owner = batch_member(Agent::CapOwner, 33, 2, 2, A_DEADLINE);
    // A delegates, controls and decides; A1 delegates, then parks on jig's rail.
    owner["authority"]["tools"] = 45.into();
    owner["tracked"] = json!({"priority":1});
    let mut subowner = batch_member(Agent::CapSubowner, 22, 1, 1, A1_DEADLINE);
    subowner["authority"]["budget"].as_object_mut().expect("budget").remove("deadline");
    subowner["deadline_seconds"] = CAP_SECONDS.into();
    subowner["authority"]["tools"] = 33.into();
    let mut later = subowner["authority"].clone();
    later["budget"]["deadline"] = A1_DEADLINE.into();
    let mut a5 = batch_member(Agent::CapImplementer, 10, 0, 0, A1_DEADLINE);
    a5["authority"]["budget"].as_object_mut().expect("budget").remove("deadline");
    Box::new([
        Script {
            cue: Agent::CapDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A owns M1."),
                batch_held(
                    CAP_SPLIT_HOLD,
                    call(
                        b"amend",
                        &json!({"target":CAP_SUBOWNER,"revision":CAP_REVISION-1,
                "amendment":{"authority":later,"instructions":CAP_SPLIT,"reason":CAP_REASON}}),
                    ),
                ),
                then_wait(
                    call(b"release", &json!({"target":CAP_SUBOWNER,"revision":CAP_HOLD_REVISION})),
                    "A1 is released after the split.",
                ),
                report(CAP_DIRECTOR_REPORT),
                wait("A is still working."),
            ]),
        },
        Script {
            cue: Agent::CapOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[subowner]})), "A1 owns M1 until its cap."),
                batch_held(
                    CAP_PASS_HOLD,
                    then_wait(
                        call(
                            b"decide_escalation",
                            &json!({"task":CAP_SUBOWNER,"revision":CAP_HOLD_REVISION,"decision":"pass"}),
                        ),
                        "The director must decide M1's cap.",
                    ),
                ),
                report(CAP_OWNER_REPORT),
                wait("A1 is still working."),
            ]),
        },
        Script {
            cue: Agent::CapSubowner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[a5]})), "A1 parks until its cap or a5's result."),
                report(CAP_SUBOWNER_REPORT),
                wait("A1 awaits a5's ending after the split."),
            ]),
        },
        Script {
            cue: Agent::CapImplementer.cue().into(),
            turns: Box::new([batch_held(CAP_A5_HOLD, report(CAP_A5_REPORT))]),
        },
    ])
}

#[must_use]
pub fn step_6_7_rules() -> Box<[Box<[Rule]>]> {
    use super::{CAP_A5_REPORT, CAP_OWNER_REPORT, CAP_SPLIT, CAP_SUBOWNER_REPORT};
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[
            (CAP_OWNER_REPORT, 3),
            (CROSSED, 3),
            ("changed: authority, instructions", 2),
            ("Done: task", 4),
            ("Held task needs a decision", 1),
            (DELEGATED, 4),
            (BEGIN, 0),
            ("", 4),
        ]),
        table(&[
            (CAP_SUBOWNER_REPORT, 2),
            (CROSSED, 2),
            ("Held task needs a decision", 1),
            (DELEGATED, 3),
            (BEGIN, 0),
            ("", 3),
        ]),
        table(&[(CAP_A5_REPORT, 1), (CROSSED, 1), (CAP_SPLIT, 2), (DELEGATED, 2), (BEGIN, 0), ("", 2)]),
        Box::new([]),
    ])
}

// Act 3, step 6.6 (domain/testing.md, 5.3)
#[must_use]
pub fn step_6_6(forge: SocketAddr) -> Box<[Script]> {
    use super::waiting::{
        APP_ID, APP_REVIEW_HEAD, DIRECTOR_REPORT, E1_SPEC, FILE, FIXED, KIT_ID, KIT_REVIEW_HEAD, OWNER_REPORT,
        OWNER_SPEC, PIN, REPIN_SPEC, REVIEW_HOLD, pin_input,
    };
    let mut owner = direction_owner(forge, OWNER_SPEC, 30, 0, 6);
    owner["authority"]["tools"] = 1.into();
    let e1 = repository_change(forge, KIT_ID, E1_SPEC, FILE, 2);
    let mut repin = repository_change(forge, APP_ID, REPIN_SPEC, PIN, 2);
    repin["dependencies"] = json!([{"kind":"batch","number":0}]);
    let verdict = || call(b"finish", &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]}));
    Box::new([
        Script {
            cue: Agent::WaitingDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A1 owns the two changes."),
                report(DIRECTOR_REPORT),
                wait("A1 is still working."),
            ]),
        },
        Script {
            cue: Agent::WaitingOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[e1,repin]})), "e1 and the waiting repin are delegated."),
                report(OWNER_REPORT),
                wait("Waiting for both landing results; no relay."),
            ]),
        },
        Script {
            cue: Agent::WaitingKitCoder.cue().into(),
            turns: Box::new([finish_change(
                call(b"write", &json!({"path":FILE,"content":FIXED})),
                "e1",
                "Fix B's fake.",
            )]),
        },
        Script { cue: review_cue(KIT_REVIEW_HEAD), turns: Box::new([direction_held(REVIEW_HOLD, verdict())]) },
        Script {
            cue: Agent::WaitingAppCoder.cue().into(),
            turns: Box::new([finish_change(
                call(b"write", &json!({"path":PIN,"content":pin_input()})),
                "Repin kit",
                "Use e1's landed kit commit.",
            )]),
        },
        Script { cue: review_cue(APP_REVIEW_HEAD), turns: Box::new([verdict()]) },
    ])
}

#[must_use]
pub fn step_6_6_rules() -> Box<[Box<[Rule]>]> {
    use super::waiting::{A1, OWNER_REPORT, REPIN};
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[(OWNER_REPORT, 1), (CROSSED, 1), (&format!("{DELEGATED}{A1}"), 2), (BEGIN, 0), ("", 2)]),
        // e1's ending selects only a wait; repin starts by its dependency rail.
        table(&[(&format!("Result from task {REPIN}:"), 1), (CROSSED, 1), (DELEGATED, 2), (BEGIN, 0), ("", 2)]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    ])
}

// Act 3, step 6.8 (domain/testing.md, 5.3).
#[must_use]
pub fn step_6_8(forge: SocketAddr) -> Box<[Script]> {
    use super::qa::{
        A, CHANGE_SPEC, DIRECTOR, DIRECTOR_REPORT, M1_REPORT, NEXT_HOLD, NEXT_REPORT, NEXT_SPEC, OWNER_HOLD,
        OWNER_REPORT, OWNER_SPEC, QA_TEST, QA_TEST_PATH, STATUS_CONTEXT, SUBOWNER_REPORT, SUBOWNER_SPEC, T,
        TEST_REPORT, TESTER_HOLD, TESTER_SPEC,
    };
    let mut owner = direction_owner(forge, OWNER_SPEC, 80, 0, 6);
    owner["authority"]["tools"] = 3.into();
    owner["authority"]["delegation"]["depth"] = 4.into();
    let mut subowner = direction_owner(forge, SUBOWNER_SPEC, 50, 0, 4);
    subowner.as_object_mut().expect("subowner").remove("tracked");
    subowner["authority"]["tools"] = 3.into();
    let mut tester = batch_member(Agent::QaTester, 10, 0, 0, super::A1_DEADLINE);
    tester["spec"]["words"] = TESTER_SPEC.into();
    let mut next = batch_member(Agent::QaNext, 10, 0, 0, super::A1_DEADLINE);
    next["spec"]["words"] = NEXT_SPEC.into();
    let mut change = direction_change(forge, CHANGE_SPEC, QA_TEST_PATH, 2);
    change["spec"]["inputs"] = json!([T]);
    let on_message = call(b"delegate", &json!({"batch":[next]}));
    let relay = then_wait(
        call(b"message", &json!({"target":DIRECTOR,"form":"words","words":format!("A1's M1 report:\n{M1_REPORT}")})),
        "M2 admitted; M1 QA continues.",
    );
    Box::new([
        Script {
            cue: Agent::QaDirector.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[owner]})), "A owns M1 and M2."),
                report(DIRECTOR_REPORT),
                wait("A is still working."),
            ]),
        },
        Script {
            cue: Agent::QaOwner.cue().into(),
            turns: Box::new([
                then_wait(call(b"delegate", &json!({"batch":[subowner]})), "A1 owns M1."),
                batch_held(OWNER_HOLD, on_message),
                report(OWNER_REPORT),
                wait("A1's QA continues."),
                relay,
            ]),
        },
        Script {
            cue: Agent::QaSubowner.cue().into(),
            turns: Box::new([
                call(b"message", &json!({"target":A,"form":"words","words":M1_REPORT})),
                then_wait(call(b"delegate", &json!({"batch":[tester]})), "T tests M1."),
                then_wait(call(b"delegate", &json!({"batch":[change]})), "c lands T's test."),
                report(SUBOWNER_REPORT),
                wait("QA's change is still working."),
            ]),
        },
        Script {
            cue: Agent::QaTester.cue().into(),
            turns: Box::new([
                batch_held(
                    TESTER_HOLD,
                    call(
                        b"effect_forge",
                        &json!({
                            "repository":{"forge":1,"repository":crate::forge::ID},"resource":{"kind":"repository"},
                            "write":{"kind":"status","commit":super::draft_commit(),"context":STATUS_CONTEXT,"check":"passed"}
                        }),
                    ),
                ),
                report(TEST_REPORT),
            ]),
        },
        Script { cue: Agent::QaNext.cue().into(), turns: Box::new([batch_held(NEXT_HOLD, report(NEXT_REPORT))]) },
        Script {
            cue: Agent::QaCoder.cue().into(),
            turns: Box::new([finish_change(
                call(b"write", &json!({"path":QA_TEST_PATH,"content":QA_TEST})),
                "M1 QA",
                "Land T's regression test.",
            )]),
        },
        Script {
            cue: Agent::Review.cue().into(),
            turns: Box::new([call(
                b"finish",
                &json!({"form":"verdict","label":"1","text":REVIEWED,"fields":{},"items":[]}),
            )]),
        },
    ])
}

#[must_use]
pub fn step_6_8_rules() -> Box<[Box<[Rule]>]> {
    use super::qa::{A2, C, M1_REPORT, OWNER_REPORT, SUBOWNER_REPORT, T};
    let table = |entries: &[(&str, u32)]| -> Box<[Rule]> {
        entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
    };
    Box::new([
        table(&[(OWNER_REPORT, 1), (CROSSED, 1), (DELEGATED, 2), (BEGIN, 0), ("", 2)]),
        table(&[
            (SUBOWNER_REPORT, 2),
            (CROSSED, 2),
            ("forge ci ", 3),
            (M1_REPORT, 1),
            (&format!("{DELEGATED}{A2}"), 4),
            (DELEGATED, 3),
            (BEGIN, 0),
            ("", 3),
        ]),
        table(&[
            (&format!("Result from task {C}:"), 3),
            (CROSSED, 3),
            (&format!("Result from task {T}:"), 2),
            ("Message 1 sent", 1),
            (DELEGATED, 4),
            (BEGIN, 0),
            ("", 4),
        ]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_fake_llm_domain::{self as fake, api};
    use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};

    fn answer(scripts: Box<[Script]>, rules: Box<[Box<[Rule]>]>, agent: Agent, fresh: &str) -> api::Answer {
        let mut recipe = crate::llm::played_by_input(scripts, rules);
        let message =
            |role, words: &[u8]| api::Message { role, parts: Box::new([api::Part::Text { text: words.into() }]) };
        let query = api::Query {
            credential: None,
            caching: api::Caching::Unscoped,
            model: b"fake".as_slice().into(),
            system: agent.cue().into(),
            tools: ["finish", "wait"]
                .map(|name| api::ToolSpec {
                    name: name.as_bytes().into(),
                    description: Box::new([]),
                    parameters: b"{}".as_slice().into(),
                })
                .into(),
            choice: api::ToolChoice::Auto,
            messages: Box::new([
                message(api::Role::User, CROSSED.as_bytes()),
                message(api::Role::Assistant, b"The preceding feedback has been read."),
                message(api::Role::User, fresh.as_bytes()),
            ]),
            max_tokens: 4096,
        };
        let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: recipe.domain_limits };
        let mut out = Queue::with_capacity(fake::MAX_OUT);
        fake::step(
            &mut recipe.domain,
            &env,
            fake::Event::Call { reply_to: ReplyTo::new(Token::new(1)), call: Token::new(1), query },
            &mut out,
        );
        loop {
            if let Some(request) = out.pop() {
                let fake::Request::Reply { result, .. } = request else { panic!("unheld script reply") };
                return result.expect("script accepted the query");
            }
            env.now = recipe.domain.next_deadline().expect("a reply is scheduled");
            fake::fire(&mut recipe.domain, &env, &mut out);
        }
    }

    fn check_finish_feedback(scripts: &[Script], rules: &[Box<[Rule]>], agent: Agent, report: &str) {
        let crossed = format!("{CROSSED} Read them before finishing.\n2: forge landing 910a2dec -> 6e73e372");
        for (fresh, finishes) in [("2: forge ci passed at 975835de", false), (crossed.as_str(), true)] {
            let reply = answer(scripts.into(), rules.into(), agent, fresh);
            assert_eq!(reply.finish, Finish::ToolCalls);
            let calls: Vec<_> = reply
                .parts
                .iter()
                .filter_map(|part| match part {
                    api::Part::ToolCall { name, arguments, .. } => Some((name.as_ref(), arguments.as_ref())),
                    api::Part::Opaque { .. } | api::Part::Text { .. } | api::Part::ToolOutput { .. } => None,
                })
                .collect();
            assert_eq!(calls.len(), 1, "one scripted judgement");
            let (name, arguments) = calls[0];
            let arguments: Value = serde_json::from_slice(arguments).expect("tool arguments");
            if finishes {
                assert_eq!(name, b"finish", "new feedback must complete the final report");
                assert_eq!(arguments, json!({"form":"report","text":report,"fields":{}}));
            } else {
                assert_eq!(name, b"wait", "ordinary fresh CI must not replay an older crossed finish");
                assert_eq!(arguments, json!({}));
            }
        }
    }

    #[test]
    fn case_5_4_owner_finishes_after_crossed_report_and_waits_on_fresh_ci() {
        check_finish_feedback(
            &step_5_4(([127, 0, 0, 1], 3000).into()),
            &step_5_4_rules(),
            Agent::StoryOwner,
            super::super::STORY_OWNER_REPORT,
        );
    }

    #[test]
    fn case_6_4_director_finishes_after_crossed_report_and_waits_on_fresh_ci() {
        check_finish_feedback(
            &step_6_4(([127, 0, 0, 1], 3000).into()),
            &step_6_4_rules(),
            Agent::RedDirector,
            super::super::RED_REPORT,
        );
    }
}
