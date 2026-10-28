//! The real launcher's traps, with a synthetic record and process stand-ins.
use crate::support;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
};

fn executable(path: &Path, body: &[u8]) {
    support::write_private(path, body);
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("fixture executable");
}

fn fixture_commands(root: &Path) {
    let bin = root.join("bin");
    fs::create_dir(&bin).expect("fixture commands");
    fs::create_dir(root.join("debug")).expect("fixture target");
    executable(
        &root.join("debug/temper"),
        br#"#!/bin/sh
set -eu
test "$1" = ctl && test "$3" = account && test "$4" = export && test "$5" = 7
IFS= read -r box < "$FIXTURE_ROOT/box-pid"
kill -0 "$box"
: > "$FIXTURE_ROOT/export-started"
sleep 0.05
kill -0 "$box"
test "$FIXTURE_FAIL_EXPORT" = false
mv "$TEMPER_LIVE_STORE/held" "$6"
: > "$FIXTURE_ROOT/exported"
"#,
    );
    executable(
        &bin.join("systemctl"),
        br#"#!/bin/sh
set -eu
test -f "$FIXTURE_ROOT/exported"
test "$(stat -c %a "$TEMPER_LIVE_ACCOUNT")" = 600
IFS= read -r box < "$FIXTURE_ROOT/box-pid"
kill "$box"
: > "$FIXTURE_ROOT/stopped"
"#,
    );
    executable(
        &bin.join("heavy"),
        br#"#!/bin/sh
set -eu
case " $* " in *' --no-run '*) exit 0 ;; esac
mkdir -m 700 "$TEMPER_LIVE_STORE" "$TEMPER_LIVE_STORE/client"
cp "$FIXTURE_ROOT/hook" "$TEMPER_LIVE_STORE/give-back"
test -x "$TEMPER_LIVE_STORE/give-back"
(cd "$TEMPER_LIVE_STORE"; exec sleep 30) </dev/null >/dev/null 2>&1 &
printf '%s\n' "$!" > "$FIXTURE_ROOT/box-pid"
printf '7\n' > "$TEMPER_LIVE_RETURN_NUMBER"
mv "$TEMPER_LIVE_ACCOUNT" "$TEMPER_LIVE_STORE/held"
if [ "$FIXTURE_FAIL_HOOK" = true ]; then
    printf '#!/bin/sh\nexit 1\n' > "$TEMPER_LIVE_STORE/give-back"
else
    rm "$TEMPER_LIVE_STORE/give-back"
fi
: > "$FIXTURE_ROOT/walking"
if [ "$FIXTURE_SIGNAL" = true ]; then
    while :; do sleep 0.05; done
fi
exit 1
"#,
    );
}

fn launcher_case(rung: &str, signal: Option<&str>, failed_hook: bool, failed_export: bool) {
    let case = format!("rung-launcher-{}-{failed_hook}-{failed_export}", signal.unwrap_or("exit"));
    let scratch = support::Scratch::startup(&case);
    let root = scratch.path();
    let account = root.join("synthetic-record");
    support::write_private(&account, b"synthetic fixture only");
    fixture_commands(root);
    support::forge_land::rung::return_hook(&root.join("debug/temper"), root, root, &account);
    fs::rename(root.join("give-back"), root.join("hook")).expect("hook installed before synthetic import");
    let mut child = Command::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/proof-live.sh"))
        .arg(rung)
        .env("PATH", format!("{}:/usr/bin:/bin", root.join("bin").display()))
        .env("CARGO_TARGET_DIR", root)
        .env("TMPDIR", root)
        .env("TEMPER_LIVE_ACCOUNT", &account)
        .env("TEMPER_LIVE_STORE", root.join("store"))
        .env("TEMPER_LIVE_KEEP", root.join("evidence"))
        .env("FIXTURE_ROOT", root)
        .env("FIXTURE_FAIL_HOOK", failed_hook.to_string())
        .env("FIXTURE_FAIL_EXPORT", failed_export.to_string())
        .env("FIXTURE_SIGNAL", signal.is_some().to_string())
        .stdin(Stdio::null())
        .stdout(fs::File::create(root.join("launcher.out")).expect("launcher output"))
        .stderr(fs::File::create(root.join("launcher.err")).expect("launcher errors"))
        .spawn()
        .expect("real launcher with synthetic processes");
    if let Some(signal) = signal {
        let clock = skein_shell::Clock::new();
        let until = clock.now().now.saturating_add(skein_lib::Duration::from_secs(3));
        while !root.join("walking").exists() {
            assert!(clock.now().now < until, "synthetic walk started");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            Command::new("kill").args([signal, &child.id().to_string()]).status().expect("signal launcher").success()
        );
    }
    let status = child.wait().expect("launcher finished");
    assert!(!status.success(), "the synthetic walk fails or is interrupted");
    assert!(
        root.join("export-started").exists(),
        "launcher exported without its hook: {}",
        fs::read_to_string(root.join("launcher.err")).expect("errors")
    );
    if failed_export {
        assert!(!account.exists() && !root.join("stopped").exists(), "failed export never stops the box");
        let pid = fs::read_to_string(root.join("box-pid")).expect("synthetic box pid");
        assert!(Command::new("kill").args(["-0", pid.trim()]).status().expect("box alive").success());
        assert!(Command::new("kill").args(["-TERM", pid.trim()]).status().expect("fixture cleanup").success());
    } else {
        let returned = fs::symlink_metadata(&account).expect("synthetic account returned");
        assert!(returned.is_file() && returned.len() > 0);
        assert_eq!(returned.permissions().mode() & 0o777, 0o600);
        assert!(root.join("stopped").exists(), "stop follows native export and stat");
    }
    scratch.remove();
}

#[test]
fn a_rungs_launcher_returns_after_its_hook_is_removed_mid_walk_before_stopping_the_box() {
    launcher_case("rung-b1", None, false, false);
}

#[test]
fn a_rungs_launcher_returns_after_a_failed_hook_on_int_term_and_hup_before_stopping_the_box() {
    for signal in ["-INT", "-TERM", "-HUP"] {
        launcher_case("rung-b0", Some(signal), true, false);
    }
}

#[test]
fn a_rungs_launcher_leaves_the_box_alive_when_native_return_fails() {
    launcher_case("rung-b1", None, false, true);
}

#[test]
fn a_rungs_return_marks_cancellation_even_before_the_store_exists() {
    let scratch = support::Scratch::startup("rung-return-before-store");
    let root = scratch.path();
    let account = root.join("synthetic-record");
    support::write_private(&account, b"synthetic fixture only");
    let body = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/proof-live-return.sh"));
    let output = Command::new("sh")
        .args(["-c", &format!("{body}\nrung_return \"$@\""), "rung-return"])
        .arg("/must-not-export")
        .arg(root.join("not-created"))
        .arg(root.join("not-created/client"))
        .arg(account)
        .arg(root.join("account-number"))
        .output()
        .expect("return before setup");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(root.join("account-number.returning").is_file(), "a later import or unit launch must refuse");
    assert!(!root.join("not-created").exists());
    scratch.remove();
}
