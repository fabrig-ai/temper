use std::{path::PathBuf, process::ExitCode};
use temper_twin::{Input, Refusal, Replace, Take};

fn main() -> ExitCode {
    skein_fail_stop::install();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(line) => {
            eprintln!("{line}");
            ExitCode::from(2)
        }
    }
}
fn run() -> Result<(), String> {
    let mut data = None;
    let mut base = None;
    let mut out = None;
    let mut takes = Vec::new();
    let mut replaces = Vec::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let Some(value) = args.next() else {
            return Err(String::from(
                "expected --data DIR --base FILE --out FILE [--cue TASK=PHRASE] [--replace OLD=NEW]",
            ));
        };
        match arg.to_str() {
            Some("--data") if data.is_none() => data = Some(PathBuf::from(value)),
            Some("--base") if base.is_none() => base = Some(PathBuf::from(value)),
            Some("--out") if out.is_none() => out = Some(PathBuf::from(value)),
            Some("--cue") => {
                let text = value.to_str().ok_or("cue must be UTF-8")?;
                let (task, cue) = text.split_once('=').ok_or("expected --cue TASK=PHRASE")?;
                let Ok(task) = task.parse() else {
                    return Err(String::from("cue task must be a u64"));
                };
                takes.push(Take { task, cue: cue.as_bytes().into() });
            }
            Some("--replace") => {
                let text = value.to_str().ok_or("replace must be UTF-8")?;
                let (old, new) = text.split_once('=').ok_or("expected --replace OLD=NEW")?;
                replaces.push(Replace { old: old.as_bytes().into(), new: new.as_bytes().into() });
            }
            _ => return Err(format!("unknown or repeated option: {}", arg.to_string_lossy().escape_debug())),
        }
    }
    let data = data.ok_or("--data required")?;
    let base = base.ok_or("--base required")?;
    let out = out.ok_or("--out required")?;
    let Ok(bytes) = std::fs::read(&base) else {
        return Err(format!("read: {}", base.to_string_lossy().escape_debug()));
    };
    let input = Input {
        data,
        base: bytes.into_boxed_slice(),
        takes: takes.into_boxed_slice(),
        replaces: replaces.into_boxed_slice(),
    };
    let twin = match temper_twin::twin(&input) {
        Ok(twin) => twin,
        Err(refusal) => return Err(line(&refusal)),
    };
    if std::fs::write(&out, &twin.document).is_err() {
        return Err(format!("write: {}", out.to_string_lossy().escape_debug()));
    }
    for (task, turns) in &twin.report.turns {
        println!("task {task}: {turns} turns");
    }
    for (replace, count) in input.replaces.iter().zip(&twin.report.replaced) {
        println!("{}: {count}", String::from_utf8_lossy(&replace.old).escape_debug());
    }
    Ok(())
}
fn line(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Read { path } => format!("read transcript: {}", path.to_string_lossy().escape_debug()),
        Refusal::Frame { task, at } => format!("task {task}: bad or uncommitted frame at {at}"),
        Refusal::Version { task, version } => format!("task {task}: unsupported version {version}"),
        Refusal::Turn { task, turn, why } => format!("task {task} turn {turn}: {why:?}"),
        Refusal::SameCue { cue } => format!("same cue: {:?}", String::from_utf8_lossy(cue)),
        Refusal::Bounds { what } => format!("past bounds: {what}"),
        Refusal::Base => String::from("base: invalid fake LLM document"),
    }
}
