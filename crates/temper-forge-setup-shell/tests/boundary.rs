pub mod support;
use std::{fs, path::Path};
fn forbidden(source: &str) -> bool {
    let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("std::process")
        || compact.contains("process::") && compact.contains("usestd::{")
        || ["CommandExt", "try_lock", "lock_shared", ".lock()"].iter().any(|name| compact.contains(name))
}
fn forbidden_at(path: &Path, source: &str) -> bool {
    let source = if path.ends_with("temper/src/main.rs") {
        source.replace("std::process::ExitCode", "ExitCode").replace("process::ExitCode", "ExitCode")
    } else if path.ends_with("temper/src/program_login.rs") {
        // Terminal sign-in execs smith's command (protocol/credentials.md, 3).
        // Other process types and every lock operation retain the same guard.
        source.replace("std::process::Command", "TerminalCommand").replace("process::CommandExt", "TerminalExec")
    } else if path.ends_with("temper/src/restart_fault.rs") {
        // The process tier's exact-listener fault aborts its own serve only.
        // Process creation, exec and every lock still belong to their boundaries.
        source.replace("std::process::abort", "RestartFaultAbort")
    } else {
        source.into()
    };
    forbidden(&source)
}
fn scan(directory: &Path) {
    for entry in fs::read_dir(directory).expect("source directory") {
        let path = entry.expect("source entry").path();
        if path.file_name().is_some_and(|name| name == "tests" || name == "tests.rs") {
            continue;
        }
        if path.is_dir() {
            scan(&path);
        } else if path.extension().is_some_and(|name| name == "rs") {
            if path.ends_with("temper-forge-setup-shell/src/process.rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("Rust source");
            assert!(!forbidden_at(&path, &source), "process or lock outside its boundary: {}", path.display());
        }
    }
}
#[test]
fn process_and_file_locks_stay_in_their_declared_modules() {
    let crates = skein_scratch::paths::workspace().join("crates");
    for entry in fs::read_dir(crates).expect("crates") {
        let source = entry.expect("crate").path().join("src");
        if source.is_dir() {
            scan(&source);
        }
    }
}
#[test]
fn the_scan_catches_each_form_and_passes_other_process_modules() {
    for (path, source, expected) in [
        ("other.rs", "use std::{fs, process::Child};", true),
        ("other.rs", "std::process::exit(1)", true),
        ("other.rs", "CommandExt", true),
        ("other.rs", "try_lock", true),
        ("other.rs", "smith::process::Event", false),
        ("temper/src/program_login.rs", "std::process::Command", false),
        ("temper/src/program_login.rs", "use std::os::unix::process::CommandExt;", false),
        ("temper/src/program_login.rs", "std::process::Child", true),
        ("temper/src/program_login.rs", "std::process::exit(1)", true),
        ("temper/src/program_login.rs", "try_lock", true),
        ("temper/src/host.rs", "std::process::abort()", true),
        ("temper/src/restart_fault.rs", "std::process::abort()", false),
        ("temper/src/restart_fault.rs", "std::process::Command", true),
        ("temper/src/restart_fault.rs", "try_lock", true),
        ("other.rs", "std::process::Command", true),
    ] {
        assert_eq!(forbidden_at(Path::new(path), source), expected, "{path}: {source}");
    }
}
