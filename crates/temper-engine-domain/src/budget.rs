//! Count owned transcript bodies and completed brief text (domain/root.md, 4.1).
use crate::{Assignment, BriefBody, BriefSection, Payload, Record, Write};

pub(crate) fn turn_bytes(write: &Write) -> u64 {
    if let Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(turn)))) = write {
        u64::try_from(turn.transcript.len()).expect("turn body length fits u64")
    } else {
        0
    }
}

pub(crate) fn payload_bytes(payload: &Payload) -> u64 {
    if let Payload::Turn { transcript, .. } = payload {
        u64::try_from(transcript.len()).expect("turn body length fits u64")
    } else {
        0
    }
}

pub(crate) fn section_bytes(sections: &[BriefSection]) -> u64 {
    let mut bytes = 0_u64;
    for section in sections {
        if let BriefBody::Text(text) = &section.body {
            bytes = bytes
                .checked_add(u64::try_from(text.len()).expect("section length fits u64"))
                .expect("completed brief text fits u64");
        }
    }
    bytes
}

pub(crate) fn assignment_bytes(assignment: &Assignment) -> u64 {
    let mut bytes = section_bytes(&assignment.sections);
    for body in &assignment.transcript {
        bytes = bytes
            .checked_add(u64::try_from(body.len()).expect("transcript body length fits u64"))
            .expect("assignment transcript and section bytes fit u64");
    }
    bytes
}
