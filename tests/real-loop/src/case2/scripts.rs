//! Each member delegates its data's children, then writes and waits in place.
use super::{CAST, Cast, Member, MemberKind};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Piece, Rule, Script, Turn};
use std::net::SocketAddr;

fn delegated(cast: &Cast, member: &Member) -> Value {
    let mut kinds = Vec::with_capacity(cast.members.len());
    // Duplicate kinds are not admitted; one entry per charter, in cast order.
    for (at, candidate) in cast.members.iter().enumerate() {
        let mut earlier = false;
        for previous in &cast.members[..at] {
            earlier |= previous.charter == candidate.charter;
        }
        if !earlier {
            kinds.push(json!({"kind":"agent","number":candidate.charter}));
        }
    }
    let mut value = json!({
        "executor":{"kind":"agent","charter":member.charter},
        "spec":{"words":member.spec,"parameters":[]},
        "contract":{"kind":"report","words":128},
        "llm":{"main":{"model":member.model.number()}},
        "authority":{"tools":3,"grants":[],
            "delegation":{"kinds":kinds,"tasks":cast.descendants(member.name),"depth":cast.depth(member.name)},
            "budget":{"spend":cast.spend(member.name)},"notes":0}
    });
    if let MemberKind::Goal { priority } = member.kind {
        value["tracked"] = json!({"priority":priority});
    }
    value
}

#[must_use]
pub fn scripts(cast: &Cast, _forge: SocketAddr) -> Box<[Script]> {
    let mut scripts = Vec::with_capacity(cast.members.len());
    for member in cast.members {
        // Singleton batches reuse the small fixture's admission and preserve
        // each parent's children in cast order without a wider batch bound.
        let mut turns = Vec::with_capacity(2 + cast.descendants(member.name) as usize);
        turns.push(tool(b"read_docs", &json!({"topic":"hold"}), &checkpoint(member)));
        for child in cast.members {
            if child.under == Some(member.name) {
                turns.push(Turn {
                    lines: Box::new([Line::Call {
                        name: b"delegate".as_slice().into(),
                        arguments: serde_json::to_vec(&json!({"batch":[delegated(cast, child)]}))
                            .expect("delegation JSON")
                            .into(),
                    }]),
                    finish: Finish::ToolCalls,
                    tokens: 20,
                });
            }
        }
        // The peer's held piece leaves this exact attempt writing. Teardown
        // cancels it through the existing harness, as in the first edition.
        turns.push(Turn {
            lines: Box::new([
                Line::Pieces {
                    pieces: Box::new([Piece {
                        text: format!("{} is working at hour 0.", member.name).into_bytes().into(),
                        hold: Some(member.cue.as_bytes().into()),
                    }]),
                },
                Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
            ]),
            finish: Finish::ToolCalls,
            tokens: 20,
        });
        scripts.push(Script { cue: member.cue.as_bytes().into(), turns: turns.into_boxed_slice() });
    }
    scripts.into_boxed_slice()
}
#[must_use]
pub fn hour_0(forge: SocketAddr) -> Box<[Script]> {
    scripts(&CAST, forge)
}
#[must_use]
pub fn rules(cast: &Cast) -> Box<[Box<[Rule]>]> {
    let mut rules = Vec::with_capacity(cast.members.len());
    for _ in cast.members {
        rules.push(Box::<[Rule]>::default());
    }
    rules.into_boxed_slice()
}
#[must_use]
pub fn hour_0_rules() -> Box<[Box<[Rule]>]> {
    rules(&CAST)
}

// This text is committed before the writing checkpoint, so a fresh brief can
// select the continuation without repeating a delegation after Stop.
fn checkpoint(member: &Member) -> String {
    format!("{} committed its hour 0 checkpoint.", member.name)
}
fn tool(name: &[u8], arguments: &Value, words: &str) -> Turn {
    Turn {
        lines: Box::new([
            Line::Text { text: words.as_bytes().into() },
            Line::Call { name: name.into(), arguments: serde_json::to_vec(arguments).expect("tool JSON").into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}

pub const DIRECTION: &str = "Use every free slot.";
pub const RELEASE_REASON: &str = "The person asked to use every free slot.";
// No message precedes Stop; Stop takes 1, the direction 2, the proposal 3.
pub const STOP_HOLD: u64 = 1;
pub const HOLD_REVISION: u64 = 1;
pub const PROPOSAL: &str = "t1/3";

/// Hour 0 followed by the director's refused release and owner proposal.
#[must_use]
pub fn step_8_1(forge: SocketAddr) -> Box<[Script]> {
    let opening = scripts(&CAST, forge);
    let mut result = Vec::with_capacity(2 * CAST.members.len() - 1);
    for (at, mut script) in opening.into_vec().into_iter().enumerate() {
        if at == 0 {
            let mut turns = script.turns.into_vec();
            turns.extend([
                tool(b"release", &json!({"target":CAST.number("P"),"revision":HOLD_REVISION}), "Try resuming P."),
                tool(
                    b"propose",
                    &json!({"action":"release","task":CAST.number("P"),
                    "hold":STOP_HOLD,"revision":HOLD_REVISION,"reason":RELEASE_REASON}),
                    "Ask the hold's owner.",
                ),
                tool(b"wait", &json!({}), "The release waits for the person."),
            ]);
            script.turns = turns.into_boxed_slice();
        } else {
            // The saved transcript distinguishes a resumed run from the initial
            // Begin; both continue at the same held writing turn.
            result.push(Script {
                cue: checkpoint(&CAST.members[at]).into_bytes().into(),
                turns: Box::new([script.turns.last().expect("writing turn").clone()]),
            });
        }
        result.push(script);
    }
    result.into_boxed_slice()
}
#[must_use]
pub fn step_8_1_rules() -> Box<[Box<[Rule]>]> {
    let mut result = Vec::with_capacity(2 * CAST.members.len() - 1);
    for (at, member) in CAST.members.iter().enumerate() {
        let mut writing = 1;
        for child in CAST.members {
            writing += usize::from(child.under == Some(member.name));
        }
        let mut rules = Vec::with_capacity(writing + 5);
        if at == 0 {
            for (cue, turn) in [(DIRECTION, writing + 1), ("Refused: task", writing + 2), ("Proposed as ", writing + 3)]
            {
                rules.push(Rule { cue: cue.as_bytes().into(), turn: u32::try_from(turn).expect("turn count") });
            }
        } else {
            result.push(Box::<[Rule]>::default());
        }
        let mut next = 2;
        for child in CAST.members {
            if child.under == Some(member.name) {
                rules.push(Rule {
                    cue: format!("Delegated tasks: {}", CAST.number(child.name)).into_bytes().into(),
                    turn: next,
                });
                next += 1;
            }
        }
        rules.push(Rule { cue: b"Begin the work your brief describes.".as_slice().into(), turn: 0 });
        rules.push(Rule { cue: Box::new([]), turn: 1 });
        result.push(rules.into_boxed_slice());
    }
    result.into_boxed_slice()
}
