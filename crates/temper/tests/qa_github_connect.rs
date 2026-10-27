//! Executable private-file guards: guide/github.md 10 and client/forge.md 5.4.
pub mod support;

use std::{fs, os::unix::fs::PermissionsExt};
use support::ctl::{self, Call};

fn refuses_file(case: &str, mode: u32, link: bool) {
    let scratch = support::Scratch::new(case);
    let key = scratch.path().join("qa-paste.pem");
    // Intentionally invalid text, not a real key. The file guard must run before parsing.
    let sentinel = b"qa-invalid-private-input-must-not-be-echoed";
    fs::write(&key, sentinel).expect("private input");
    fs::set_permissions(&key, fs::Permissions::from_mode(mode)).expect("test permissions");
    let path = if link {
        let alias = scratch.path().join("qa-link.pem");
        std::os::unix::fs::symlink(&key, &alias).expect("input symlink");
        alias
    } else {
        key
    };
    let ended = ctl::alone(
        scratch.path(),
        Call::new(&[
            "forge",
            "connect",
            "1",
            "github-app",
            "4242",
            path.to_str().expect("path"),
            "--key",
            "qa-private-file",
        ]),
    );
    assert_ne!(ended.exit, Some(skein_io::kernel::Exit::Code(0)), "unsafe file refused");
    let output = [ended.stdout, ended.stderr].concat();
    assert!(!output.windows(sentinel.len()).any(|part| part == sentinel), "no input echo");
    assert!(
        String::from_utf8_lossy(&output).to_lowercase().contains(if link {
            "link"
        } else {
            "permits group or other access"
        }),
        "refusal identifies the private-file requirement: {}",
        String::from_utf8_lossy(&output)
    );
    scratch.remove();
}

#[test]
fn ctl_refuses_a_group_readable_key_file_without_echoing_it() {
    refuses_file("qa-github-group", 0o640, false);
}

#[test]
fn ctl_refuses_an_other_readable_key_file_without_echoing_it() {
    refuses_file("qa-github-other", 0o604, false);
}

#[test]
fn ctl_refuses_a_symlink_even_when_its_target_is_private() {
    refuses_file("qa-github-symlink", 0o600, true);
}

#[test]
#[ignore = "gap: support::github exposes only restart(), no public App/browser helper or setup-plan hold"]
fn a_restart_mid_install_ends_the_old_state_and_erases_the_carried_key() {
    // client/forge.md 9.1 and protocol/credentials.md 2.2.
    let scratch = support::Scratch::new("qa-github-interrupted");
    let directory = skein_scratch::paths::package().join("tests/support/github");
    let mut cluster = support::cluster::Cluster::read_from(scratch.path(), &directory);
    let [clients] = support::free_addresses(&[]);
    cluster.bind("story.clients", &clients.to_string());
    cluster.prepare("forge");
    // Members supports killing and restarting the engine. A public App/browser
    // helper is missing for driving its old setup link after that; a setup-plan
    // hold is also needed to interrupt Paste/Accept before its durable write.
    scratch.remove();
}
