//! The reference case's judgements, played word for word (domain/testing.md, 5.3).
use super::{
    A, A_REPORT, A_SPEC_PREFIX, A_SPEC_SUFFIX, ANSWERED, ASKED, Agent, DESIGN_DELEGATED, DESIGN_DOC, DESIGN_LINE,
    DESIGN_REPORT, DESIGN_SPEC, DESIGN_TITLE, DESIGN_WORDS, DIRECTOR, DIRECTOR_REPORT, GREEN_LIGHT,
    OWNER_EFFORT_NUMBER, OWNER_MODEL_NUMBER, QUESTION, REVIEWED, SCHEDULED, draft_commit,
};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use std::net::SocketAddr;

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
            "delegation":{"kinds":[{"kind":"procedure","number":2},{"kind":"agent","number":1}],"tasks":6,"depth":3},
            "budget":{"spend":20},"notes":0}
    }]});
    let design = json!({"batch":[{
        "executor":{"kind":"procedure","connector":1,"code":2},
        "spec":{"words":DESIGN_SPEC,"parameters":[
            {"name":1,"kind":"resource","connector":1,"resource":crate::forge::ID},
            {"name":2,"kind":"bytes","value":"main"}]},
        "contract":{"kind":"change","connector":1,"change_kind":1,"words":128},
        "authority":{"tools":1,"grants":grants(forge, &[2,3,4,10,11,12]),
            "delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":4,"depth":2},
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
                // A wait call withholds its own turn. Read the project notes
                // first: its completed tool turn durably records A's question.
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

#[must_use]
pub fn step_4_1_4_3_4_6(forge: SocketAddr) -> Box<[Script]> {
    let mut scripts = step_4_1_4_3(forge).into_vec();
    scripts.extend(step_4_6(forge));
    scripts.into_boxed_slice()
}
