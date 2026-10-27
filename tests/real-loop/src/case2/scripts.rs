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
        let mut turns = Vec::with_capacity(1 + cast.descendants(member.name) as usize);
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
