//! A live run's twin (domain/testing.md, 4.1), read after a clean stop.
mod document;
mod frames;

use std::path::PathBuf;

#[derive(Debug)]
pub struct Input {
    pub data: PathBuf,
    pub base: Box<[u8]>,
    pub takes: Box<[Take]>,
    pub replaces: Box<[Replace]>,
}
#[derive(Debug)]
pub struct Take {
    pub task: u64,
    pub cue: Box<[u8]>,
}
#[derive(Debug)]
pub struct Replace {
    pub old: Box<[u8]>,
    pub new: Box<[u8]>,
}
#[derive(Debug)]
pub struct Twin {
    pub document: Box<[u8]>,
    pub report: Report,
}
#[derive(Debug)]
pub struct Report {
    pub turns: Box<[(u64, u32)]>,
    pub replaced: Box<[u32]>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Read { path: PathBuf },
    Frame { task: u64, at: u64 },
    Version { task: u64, version: u16 },
    Turn { task: u64, turn: u32, why: Unheld },
    SameCue { cue: Box<[u8]> },
    Bounds { what: &'static str },
    Base,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Unheld {
    Opaque,
    CutShort,
    Compaction,
}

/// Convert selected tasks' completions to system-cued scripts, preserving
/// the base's other members. The caller guarantees a cleanly stopped input.
///
/// # Errors
/// Refuses unreadable or invalid frames, unsupported turns, duplicate cues,
/// invalid bases and documents beyond the fake's bounds.
pub fn twin(input: &Input) -> Result<Twin, Refusal> {
    let mut base = document::base(&input.base)?;
    if input.takes.len() > temper_fake::ENTRIES as usize {
        return Err(Refusal::Bounds { what: "scripts" });
    }
    for (i, take) in input.takes.iter().enumerate() {
        if take.cue.is_empty() || take.cue.len() > 256 || std::str::from_utf8(&take.cue).is_err() {
            return Err(Refusal::Bounds { what: "cue" });
        }
        for previous in &input.takes[..i] {
            if previous.cue == take.cue {
                return Err(Refusal::SameCue { cue: take.cue.clone() });
            }
        }
    }
    for replace in &input.replaces {
        if replace.old.is_empty() {
            return Err(Refusal::Bounds { what: "empty replace" });
        }
    }
    let mut turns = Vec::new();
    let mut replaced = vec![0; input.replaces.len()];
    let mut scripts = Vec::new();
    for take in &input.takes {
        let completions = frames::read(&input.data, take.task)?;
        let count = u32::try_from(completions.len()).expect("eight turns");
        scripts.push(document::script(take, &completions, &input.replaces, &mut replaced)?);
        turns.push((take.task, count));
    }
    let document = document::finish(&mut base, &scripts)?;
    Ok(Twin { document, report: Report { turns: turns.into_boxed_slice(), replaced: replaced.into_boxed_slice() } })
}
