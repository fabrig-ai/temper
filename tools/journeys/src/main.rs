use std::{
    env, fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{self, Command},
};
use temper_client_journeys::again::{self, Paths};
use temper_client_journeys::{JOURNEYS, Refusal, add, reprint};
use temper_client_shell_web::journey;

fn main() {
    skein_fail_stop::install();
    let mut args = env::args_os();
    let _program = args.next();
    let arguments: Vec<_> = args.collect();
    let result = match arguments.as_slice() {
        [] => reprint_all(),
        [command, name, file] if command == "add" => {
            let Some(name) = name.to_str() else {
                fail(&format!("invalid journey name {}", name.display()));
            };
            match fs::read(file) {
                Ok(recording) => add(Path::new(JOURNEYS), name, &recording).map(|()| false).map_err(refusal),
                Err(_) => Err(format!("cannot read {}", Path::new(file).display())),
            }
        }
        [command, name] if command == "again" => {
            let Some(name) = name.to_str() else {
                fail(&format!("invalid journey name {}", name.display()));
            };
            record_again(name).map(|()| false)
        }
        _ => {
            eprintln!("usage: temper-client-journeys [add NAME FILE | again NAME]");
            process::exit(2);
        }
    };
    match result {
        Ok(false) => {}
        Ok(true) => process::exit(1),
        Err(why) => fail(&why),
    }
}

fn record_again(name: &str) -> Result<(), String> {
    // Unwalkable journeys are refused before starting the builds or either child.
    let _steps = again::starting(Path::new(JOURNEYS), name).map_err(refusal)?;
    let chromium = env::var_os("SKEIN_TEST_CHROMIUM")
        .ok_or_else(|| format!("{name}: set SKEIN_TEST_CHROMIUM to the Chromium binary"))?;
    let chromium = fs::canonicalize(&chromium)
        .map_err(|error| format!("{name}: cannot open Chromium {}: {error}", Path::new(&chromium).display()))?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = scratch().map_err(|error| format!("{name}: cannot make a fresh scratch directory: {error}"))?;
    let result = build(&root, &scratch, chromium)
        .and_then(|paths| again::again(Path::new(JOURNEYS), name, &paths).map_err(refusal));
    match result {
        Ok(()) => fs::remove_dir_all(&scratch)
            .map_err(|error| format!("{name}: cannot remove scratch directory {}: {error}", scratch.display())),
        Err(why) => Err(format!("{name}: {why}\nscratch directory kept at {}", scratch.display())),
    }
}

fn scratch() -> io::Result<PathBuf> {
    let base = fs::canonicalize(env::temp_dir())?;
    for attempt in 0..1000 {
        let path = base.join(format!("temper-journey-{}-{attempt}", process::id()));
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no fresh journey directory name"))
}

fn build(root: &Path, scratch: &Path, chromium: PathBuf) -> Result<Paths, String> {
    let status = Command::new("cargo")
        .current_dir(root)
        .env("TEMPER_BUNDLE", "none")
        .args(["build", "-p", "temper", "-p", "temper-fake-claude"])
        .status()
        .map_err(|error| format!("cannot start cargo build: {error}"))?;
    if !status.success() {
        return Err(format!("cargo build failed: {status}"));
    }
    let bundle = scratch.join("bundle");
    let status = Command::new(root.join("scripts/bundle.sh"))
        .current_dir(root)
        .env("TEMPER_BUNDLE", "none")
        .args(["--loopback", "--record"])
        .arg(&bundle)
        .status()
        .map_err(|error| format!("cannot start scripts/bundle.sh: {error}"))?;
    if !status.success() {
        return Err(format!("scripts/bundle.sh failed: {status}"));
    }
    let target = env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |target| root.join(target));
    Ok(Paths { temper: target.join("debug/temper"), bundle, chromium, scratch: scratch.to_owned() })
}

fn reprint_all() -> Result<bool, String> {
    let stale = reprint(Path::new(JOURNEYS), Path::new(JOURNEYS)).map_err(refusal)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(JOURNEYS).map_err(|_| format!("cannot read {JOURNEYS}"))? {
        let entry = entry.map_err(|_| format!("cannot read {JOURNEYS}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|_| format!("cannot read {}", path.display()))?.is_dir()
            && path.join("recording").is_file()
        {
            names.push(entry.file_name());
        }
    }
    names.sort();
    for name in names {
        println!("{}", name.to_string_lossy());
    }
    for (name, stopped) in &stale {
        eprintln!("temper-client-journeys: {}", stale_message(name, stopped));
    }
    Ok(!stale.is_empty())
}

fn refusal(why: Refusal) -> String {
    match why {
        Refusal::Name(name) => format!("invalid journey name {name:?}; expected [a-z][a-z0-9-]*"),
        Refusal::Proof { name, refusal } => match refusal {
            journey::Refusal::Unreadable(unreadable) => unreadable_message(&name, &unreadable),
            journey::Refusal::Stale(stale) => stale_message(&name, &stale),
            journey::Refusal::Unproved { mark } => format!("{name} has no live print at mark {mark}"),
            journey::Refusal::Differs { mark, byte } => format!("{name} differs at mark {mark}, byte {byte}"),
        },
        Refusal::Unreadable { name, unreadable } => unreadable_message(&name, &unreadable),
        Refusal::Io { path } => format!("cannot read or write {}", path.display()),
        Refusal::Unwalkable { name, unwalkable } => match unwalkable {
            journey::Unwalkable::Unreadable(unreadable) => unreadable_message(&name, &unreadable),
            journey::Unwalkable::Drawing { step } => {
                format!("{name} is unwalkable at step {step}: a drawing needs a new walk")
            }
            journey::Unwalkable::Moving { step } => {
                format!("{name} is unwalkable at step {step}: a key mark caught a moving page")
            }
        },
        Refusal::NotStill { name, step } => format!("{name} at step {step}: page not still within 30 s"),
        Refusal::NoControl { name, step } => format!(
            "{name} at step {step}: no control gives the recorded event within 10 s of the page being still; walk it anew"
        ),
        Refusal::WalkDiffers { name, step } => format!("{name} at step {step}: the new walk differs; walk it anew"),
        Refusal::Save { name, step, refusal: why } => {
            format!("at step {step}: {}", refusal(Refusal::Proof { name, refusal: why }))
        }
        Refusal::Harness { name, step, report } => format!("{name} at step {step}: browser or box failed: {report}"),
    }
}

fn unreadable_message(name: &str, unreadable: &journey::Unreadable) -> String {
    format!("{name} is unreadable at line {}: {}", unreadable.line, unreadable.why)
}

fn stale_message(name: &str, stale: &journey::Stale) -> String {
    format!(
        "{name} is stale at turn {}, line {}: recorded {}, asked {}; record it again (client/journeys.md, 8)",
        stale.turn, stale.line, stale.recorded, stale.asked
    )
}

fn fail(why: &str) -> ! {
    eprintln!("temper-client-journeys: {why}");
    process::exit(1);
}
