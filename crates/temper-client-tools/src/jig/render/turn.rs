//! Turn parts retain their byte positions and carry an exact continuation call.
use super::{Text, text_end};
use crate::jig::{Limits, call::Call, call::number};
use alloc::boxed::Box;
use jig_client::contract as c;
pub(super) fn name(name: c::TurnName, out: &mut Text) {
    out.number(name.attempt);
    out.put(b".");
    out.number(u64::from(name.number));
}
pub(super) fn sized(task: u64, turn: c::TurnName, bytes: u64, past: bool, out: &mut Text) {
    out.put(b"turn ");
    name(turn, out);
    out.put(b" \xc2\xb7 ");
    out.number(bytes.div_ceil(1024));
    out.put(b" KB \xc2\xb7 ");
    if past {
        out.put(b"past the limit; report it\n");
    } else {
        out.put(b"read with: task turn ");
        out.number(task);
        out.put(b" ");
        name(turn, out);
        out.put(b"\n");
    }
}
pub(super) fn text(
    task: u64,
    turn_name: c::TurnName,
    turn: &c::TurnRead,
    call: &Call,
    limits: &Limits,
    out: &mut Text,
) {
    match turn {
        c::TurnRead::Undecodable { why: c::TurnProblem::TooLarge { bytes } } => {
            sized(task, turn_name, *bytes, call.tool == 6, out);
        }
        c::TurnRead::Undecodable { why: c::TurnProblem::Version { .. } | c::TurnProblem::Malformed } => {
            out.put(b"turn ");
            name(turn_name, out);
            out.put(b" \xc2\xb7 unreadable; report it\n");
        }
        c::TurnRead::Decoded(turn) => {
            out.put(b"turn ");
            name(turn_name, out);
            out.put(b"\n");
            let mut part = 0u64;
            for message in &turn.messages {
                for block in &message.blocks {
                    match block {
                        c::Block::Text(text) => words(task, turn_name, &mut part, text.bytes(), call, limits, out),
                        c::Block::Call(call_block) => {
                            out.put(b"call ");
                            out.number(call_block.number);
                            out.put(b" \xc2\xb7 ");
                            out.put(call_block.tool.bytes());
                            out.put(b"\n");
                            words(task, turn_name, &mut part, call_block.input.bytes(), call, limits, out);
                            if let Some(outcome) = &call_block.outcome {
                                words(task, turn_name, &mut part, outcome.bytes(), call, limits, out);
                            }
                            if call.tool != 4 {
                                for reference in &call_block.parts {
                                    reference_text(reference, out);
                                }
                            }
                        }
                        c::Block::Result { call: called, summary, failed } => {
                            out.put(b"result ");
                            out.number(*called);
                            if *failed {
                                out.put(b" \xc2\xb7 failed");
                            }
                            out.put(b"\n");
                            words(task, turn_name, &mut part, summary.bytes(), call, limits, out);
                        }
                        c::Block::Opaque { dialect, size } => {
                            out.put(b"opaque ");
                            out.put(dialect.bytes());
                            out.put(b" \xc2\xb7 ");
                            out.number(*size);
                            out.put(b" bytes\n");
                        }
                    }
                }
            }
        }
    }
}
fn words(task: u64, turn: c::TurnName, part: &mut u64, bytes: &[u8], call: &Call, limits: &Limits, out: &mut Text) {
    *part = part.checked_add(1).expect("bounded parts");
    if call.tool == 6
        && let Some(selected) = number(call, 2)
        && selected != *part
    {
        return;
    }
    let from = if call.tool == 6 { number(call, 3).unwrap_or(0) } else { 0 };
    let from = text_end(bytes, usize::try_from(from).unwrap_or(usize::MAX));
    let bytes = bytes.get(from..).expect("offset within part");
    let end = text_end(bytes, out.room().min(usize::try_from(limits.part).expect("part bound fits")));
    out.put(bytes.get(..end).expect("part cut"));
    out.reserve(end);
    out.put(b"\n");
    let left = bytes.len().checked_sub(end).expect("part cut");
    if left > 0 {
        out.number(u64::try_from(left).expect("bounded part"));
        out.put(b" bytes left; read on with: ");
        continuation(
            task,
            turn,
            *part,
            u64::try_from(from.checked_add(end).expect("bounded offset")).expect("bounded offset"),
            out,
        );
        out.put(b"\n");
    }
}

fn continuation(task: u64, turn: c::TurnName, part: u64, from: u64, out: &mut Text) {
    out.put(b"task turn ");
    out.number(task);
    out.put(b" ");
    name(turn, out);
    out.put(b" --part ");
    out.number(part);
    out.put(b" --from ");
    out.number(from);
}
pub(super) fn read_on(task: u64, turn: c::TurnName, part: u64, from: u64) -> Box<[u8]> {
    let mut measure = Text::measure();
    continuation(task, turn, part, from, &mut measure);
    let mut out = Text::writing_exact(measure);
    continuation(task, turn, part, from, &mut out);
    out.finish()
}
pub(super) fn reference_text(part: &c::Part, out: &mut Text) {
    out.put(b"part: ");
    match part {
        c::Part::Resource { connector, path } => {
            out.number(u64::from(*connector));
            out.put(b":");
            for (at, segment) in path.iter().enumerate() {
                if at > 0 {
                    out.put(b"/");
                }
                out.put(segment);
            }
        }
        c::Part::Connector { .. }
        | c::Part::Kind { .. }
        | c::Part::Panel { .. }
        | c::Part::Result { .. }
        | c::Part::Effect { .. }
        | c::Part::OwnerEffect { .. }
        | c::Part::Outbox { .. }
        | c::Part::Hold { .. }
        | c::Part::Project { .. }
        | c::Part::Deployment { .. } => super::document::part(part, out),
    }
    out.put(b"\n");
}
