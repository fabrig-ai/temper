//! Jig's frame decoder and Smith's turn codec; never opens the store.
use crate::{Refusal, Unheld};
use jig_protocol_store::{self as store, codec, transcripts::frame};
use smith_transcript::{self as smith, Block, CallProblem, Returned, Role, TurnKind};
use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::Path,
};

fn limits() -> codec::Limits {
    codec::Limits {
        settings: store::settings_v1::CEILINGS,
        core: store::core_v6::CEILINGS,
        tasks: store::tasks_v9::CEILINGS,
        people: store::people_v13::CEILINGS,
        notes: store::notes_v1::CEILINGS,
    }
}

pub(crate) fn read(data: &Path, task: u64) -> Result<Vec<smith::Turn>, Refusal> {
    // Jig's protocol/store.md, 8.1; transcripts::path is private to the store.
    let path = data.join("transcripts").join(format!("{:016x}", task >> 12)).join(format!("{task:016x}"));
    let Ok(mut file) = File::open(&path) else {
        return Err(Refusal::Read { path });
    };
    let mut at = 0;
    let mut turns = Vec::new();
    let limits = limits();
    let maximum = store::core_v6::TurnRecord::worst_case_bytes(&limits.core).expect("schema ceilings") + 12;
    loop {
        let mut head = [0; 4];
        // A whole EOF ends the file; an incomplete header is an unheld frame.
        match file.read(&mut head[..1]) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => return Err(Refusal::Read { path }),
        }
        if let Err(error) = file.read_exact(&mut head[1..]) {
            return Err(if error.kind() == ErrorKind::UnexpectedEof {
                Refusal::Frame { task, at }
            } else {
                Refusal::Read { path }
            });
        }
        let length = u32::from_be_bytes(head);
        if length < 14 || u64::from(length) > maximum {
            return Err(Refusal::Frame { task, at });
        }
        let mut bytes = vec![0; length as usize];
        bytes[..4].copy_from_slice(&head);
        if let Err(error) = file.read_exact(&mut bytes[4..]) {
            return Err(if error.kind() == ErrorKind::UnexpectedEof {
                Refusal::Frame { task, at }
            } else {
                Refusal::Read { path }
            });
        }
        let turn = decode(&bytes, &limits, task, at)?;
        if turns.len() == temper_fake::ENTRIES as usize {
            return Err(Refusal::Bounds { what: "turns" });
        }
        turns.push(turn);
        at = at.checked_add(u64::from(length)).ok_or(Refusal::Bounds { what: "frame offset" })?;
    }
    if turns.is_empty() {
        return Err(Refusal::Frame { task, at: 0 });
    }
    Ok(turns)
}

fn decode(bytes: &[u8], limits: &codec::Limits, task: u64, at: u64) -> Result<smith::Turn, Refusal> {
    let end = bytes.len();
    let checksum = u32::from_be_bytes(bytes[end - 8..end - 4].try_into().expect("four checksum bytes"));
    if bytes[end - 4..] != bytes[..4] || frame::crc32c(&bytes[4..end - 8]) != checksum {
        return Err(Refusal::Frame { task, at });
    }
    // The generated codec owns version admission; inspect its version error
    // only after the frame's wrapper and checksum have passed.
    let Ok(record) = frame::decode(bytes, limits) else {
        let version = u16::from_be_bytes([bytes[4], bytes[5]]);
        let mut reader = skein_lib::Reader::new(&bytes[4..end - 8]);
        if let Err(problem) = store::core_v6::TurnRecord::decode(&limits.core, &mut reader)
            && problem.path == store::core_v6::Path::TurnRecordVersion
        {
            return Err(Refusal::Version { task, version });
        }
        return Err(Refusal::Frame { task, at });
    };
    if record.task != task {
        return Err(Refusal::Frame { task, at });
    }
    let mut reader = skein_lib::Reader::new(&record.transcript);
    let turn = match smith::Turn::decode(&smith::CEILINGS, &mut reader) {
        Ok(turn) => turn,
        Err(problem) => {
            if problem.path == smith::v6::Path::TurnVersion && record.transcript.len() >= 2 {
                return Err(Refusal::Version {
                    task,
                    version: u16::from_be_bytes([record.transcript[0], record.transcript[1]]),
                });
            }
            return Err(Refusal::Frame { task, at });
        }
    };
    check(&turn, task, record.turn)?;
    Ok(turn)
}

fn check(turn: &smith::Turn, task: u64, number: u32) -> Result<(), Refusal> {
    if *turn.kind() == TurnKind::Window {
        return Err(Refusal::Turn { task, turn: number, why: Unheld::Compaction });
    }
    let mut completions = 0;
    for message in turn.messages() {
        if *message.role() == Role::Assistant {
            completions += 1;
        }
        for block in message.blocks() {
            match block {
                Block::Opaque(_) => return Err(Refusal::Turn { task, turn: number, why: Unheld::Opaque }),
                Block::Result(result) => {
                    if let Returned::Invalid(invalid) = result.returned()
                        && matches!(invalid.problem(), CallProblem::CutOff(_))
                    {
                        return Err(Refusal::Turn { task, turn: number, why: Unheld::CutShort });
                    }
                }
                Block::Call(call) => {
                    if !matches!(
                        serde_json::from_slice::<serde_json::Value>(call.input()),
                        Ok(serde_json::Value::Object(_))
                    ) {
                        return Err(Refusal::Turn { task, turn: number, why: Unheld::CutShort });
                    }
                }
                Block::Text(_) | Block::Refusal(_) => {}
            }
        }
    }
    if completions != 1 {
        return Err(Refusal::Turn { task, turn: number, why: Unheld::CutShort });
    }
    Ok(())
}
