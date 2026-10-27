use super::*;
fn args(values: &[&str]) -> Vec<Argument> {
    values.iter().map(|s| Box::from(s.as_bytes())).collect()
}
#[test]
fn settings_resolve_with_their_origins_and_keep_the_remaining_arguments() {
    for (name, arguments, environment, config, origin, rest) in [
        ("flag", vec!["--config", "/flag"], vec![("TEMPER_CONFIG", "/variable")], "/flag", Origin::Flag, vec![]),
        ("variable", vec![], vec![("TEMPER_CONFIG", "/variable")], "/variable", Origin::Variable, vec![]),
        ("base", vec![], vec![("XDG_CONFIG_HOME", "/xdg")], "/xdg/temper", Origin::Base, vec![]),
        ("home", vec![], vec![], "/home/test/.config/temper", Origin::Home, vec![]),
        ("empty", vec![], vec![("TEMPER_CONFIG", "")], "/home/test/.config/temper", Origin::Home, vec![]),
        ("relative", vec!["--config", "../cfg"], vec![], "/work/../cfg", Origin::Flag, vec![]),
        (
            "positionals",
            vec!["one", "--config", "/cfg", "two", "--data", "/d", "three", "--cache", "/k"],
            vec![],
            "/cfg",
            Origin::Flag,
            vec!["one", "two", "three"],
        ),
        (
            "separator",
            vec!["--", "--config", "/cfg"],
            vec![],
            "/home/test/.config/temper",
            Origin::Home,
            vec!["--", "--config", "/cfg"],
        ),
    ] {
        let mut env: Vec<EnvironmentEntry> =
            environment.iter().map(|(k, v)| (Box::from(k.as_bytes()), Box::from(v.as_bytes()))).collect();
        env.push((Box::from(b"HOME".as_slice()), Box::from(b"/home/test".as_slice())));
        let (found, remaining) = resolve(&args(&arguments), &env, b"/work").expect(name);
        assert_eq!(found.config.as_ref(), config.as_bytes(), "{name}");
        assert_eq!(found.origins[0], origin, "{name}");
        assert_eq!(remaining.as_ref(), args(&rest), "{name}");
    }
}
#[test]
fn settings_refusals_name_the_flag_and_its_reason() {
    let long = "/".to_owned() + &"a".repeat(4096);
    for (arguments, expected) in [
        (vec!["--config", "/a", "--config", "/b"], "--config is given twice"),
        (vec!["--config"], "--config takes a value: CONFIG"),
        (
            vec![],
            "--config CONFIG is not given, TEMPER_CONFIG is not set, and its default $XDG_CONFIG_HOME/temper needs XDG_CONFIG_HOME or HOME as an absolute path",
        ),
        (vec!["--config", &long, "--data", "/d", "--cache", "/c"], "--config DIR: absolute path is past 4096 bytes"),
    ] {
        assert_eq!(resolve(&args(&arguments), &[], b"/work").expect_err("refused").to_string(), expected);
    }
}
pub(crate) fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("temper-dirs-{}-{name}", std::process::id()));
    std::fs::create_dir(&path).expect("scratch");
    path
}
pub(crate) fn given(root: &Path) -> Directories {
    resolve(
        &args(&[
            "--config",
            root.to_str().expect("scratch text"),
            "--data",
            root.join("data").to_str().expect("data text"),
            "--cache",
            root.join("cache").to_str().expect("cache text"),
        ]),
        &[],
        b"/",
    )
    .expect("given")
    .0
}
#[test]
fn roots_are_private_and_checked_at_their_own_names() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    for row in ["absent", "private", "public", "link", "file", "parent-link"] {
        let root = scratch(row);
        let target = root.join("parent/root");
        match row {
            "absent" => {}
            "parent-link" => {
                std::fs::create_dir(root.join("real")).expect("parent");
                symlink(root.join("real"), root.join("parent")).expect("parent link");
            }
            _ => {
                std::fs::create_dir(root.join("parent")).expect("parent");
                match row {
                    "link" => symlink(&root, &target).expect("link"),
                    "file" => std::fs::write(&target, b"").expect("file"),
                    _ => {
                        std::fs::create_dir(&target).expect("directory");
                        std::fs::set_permissions(
                            &target,
                            std::fs::Permissions::from_mode(if row == "public" { 0o755 } else { 0o700 }),
                        )
                        .expect("mode");
                    }
                }
            }
        }
        let result = make_private(&target, skein_shell::effective_user());
        if ["public", "link", "file"].contains(&row) {
            assert_eq!(
                result.expect_err(row),
                format!("{}: must be a private directory of this user's, not reached through a link", target.display())
            );
        } else {
            result.expect(row);
            assert_eq!(std::fs::metadata(&target).expect("made").mode() & 0o777, 0o700);
        }
        if row == "absent" {
            assert_eq!(std::fs::metadata(root.join("parent")).expect("made parent").mode() & 0o777, 0o700);
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
#[test]
fn a_missing_document_refuses_before_making_data_or_cache() {
    let root = scratch("missing");
    let dirs = given(&root);
    assert_eq!(
        dirs.read().expect_err("missing"),
        format!("configuration {}: absent; temper init writes one", dirs.document().display())
    );
    assert!(!super::super::path(&dirs.data).exists());
    assert!(!super::super::path(&dirs.cache).exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}
