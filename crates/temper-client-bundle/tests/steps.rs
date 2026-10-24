//! The build script's logic on samples (client/browser.md, 2.6), in both builds.
#[path = "../build/steps.rs"]
mod steps;

use std::ffi::OsStr;
use steps::Choice;

#[test]
fn temper_bundle_is_embed_unset_or_embed_none_for_none_and_fails_otherwise() {
    assert_eq!(steps::choice(None), Ok(Choice::Embed));
    assert_eq!(steps::choice(Some(OsStr::new("embed"))), Ok(Choice::Embed));
    assert_eq!(steps::choice(Some(OsStr::new("none"))), Ok(Choice::None));
    for value in ["", "None", "dir", "/tmp/bundle"] {
        assert_eq!(
            steps::choice(Some(OsStr::new(value))),
            Err(format!("temper-client-bundle: TEMPER_BUNDLE must be embed or none, not {value}")),
            "{value:?}"
        );
    }
}

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "temper-bundle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).expect("sample operation succeeds");
    path
}

#[test]
fn profiles_map_to_module_profile_and_output_directory() {
    for (input, expected) in [("debug", ("dev", "debug")), ("release", ("web", "web"))] {
        assert_eq!(steps::profile(input), Ok(expected));
    }
}

#[test]
fn unknown_profile_is_refused_by_name() {
    assert_eq!(steps::profile("custom"), Err("temper-client-bundle: unsupported PROFILE \"custom\"".into()));
}

#[test]
fn nearest_target_ancestor_works_with_and_without_target_triple() {
    let root = scratch();
    fs::write(root.join("CACHEDIR.TAG"), "").expect("sample operation succeeds");
    for relative in ["debug/build/pkg/out", "wasm32-unknown-unknown/debug/build/pkg/out"] {
        assert_eq!(steps::target_dir(&root.join(relative)), Some(root.clone()));
    }
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn missing_target_tag_returns_none() {
    let root = scratch();
    assert_eq!(steps::target_dir(&root), None);
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn installed_wasm_directory_is_accepted() {
    let root = scratch();
    assert_eq!(steps::wasm_target(true, format!("{}\n", root.display()).as_bytes()), Ok(()));
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn absent_wasm_target_explains_both_ways_out() {
    let root = scratch();
    let absent = root.join("absent");
    for (success, output) in [
        (false, root.to_str().expect("sample path is UTF-8")),
        (true, absent.to_str().expect("sample path is UTF-8")),
        (true, ""),
    ] {
        assert_eq!(
            steps::wasm_target(success, output.as_bytes()),
            Err(
                "temper-client-bundle: the browser's bundle needs the wasm32-unknown-unknown target: add it (rustup target add wasm32-unknown-unknown), or build without the browser (TEMPER_BUNDLE=none)"
            )
        );
    }
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn commands_keep_jobserver_and_remove_outer_flags() {
    let commands = steps::commands(
        OsStr::new("/tool/cargo"),
        Path::new("/work"),
        Path::new("/target"),
        Path::new("/out"),
        ("web", "web"),
    );
    let expected = [
        (
            "/tool/cargo",
            vec![
                "build",
                "--locked",
                "--target",
                "wasm32-unknown-unknown",
                "--profile",
                "web",
                "-p",
                "temper-client-shell-web",
            ],
        ),
        ("/tool/cargo", vec!["build", "--locked", "--release", "-p", "skein-ui-bundle"]),
        (
            "/target/bundle/release/skein-ui-bundle",
            vec![
                "--module",
                "/target/bundle/wasm32-unknown-unknown/web/temper_client_shell_web.wasm",
                "--name",
                "temper_client_shell_web",
                "--prefix",
                "/bundle/",
                "--stylesheet",
                "/work/crates/temper-client-shell-web/temper.css",
                "--stylesheet",
                "/work/crates/temper-client-shell-web/task-pages.css",
                "--files",
                "/work/crates/temper-client-shell-web/files",
                "--noscript",
                "temper needs WebAssembly and scripts.",
                "--out",
                "/out/bundle",
            ],
        ),
    ];
    for (command, (program, args)) in commands.iter().zip(expected) {
        assert_eq!(command.program, program);
        assert_eq!(command.args, args);
        assert_eq!(command.directory, Path::new("/work"));
    }
    for command in &commands[..2] {
        assert_eq!(command.remove, ["CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTC_WORKSPACE_WRAPPER", "CLIPPY_ARGS"]);
        assert_eq!(command.set, [("CARGO_TARGET_DIR", "/target/bundle".into())]);
    }
    assert!(commands[2].remove.is_empty());
    assert!(commands[2].set.is_empty());
}

#[test]
fn dep_info_skips_target_and_unescapes_source_paths() {
    assert_eq!(
        steps::dep_info("/target/module.wasm: /work/main.rs /work/a\\ b.rs \\\n /work/lib.rs\n\n/work/main.rs:\n")
            .expect("sample operation succeeds"),
        vec![PathBuf::from("/work/main.rs"), PathBuf::from("/work/a b.rs"), PathBuf::from("/work/lib.rs")]
    );
}

#[test]
fn malformed_dep_info_is_refused() {
    for source in ["", "no separator", "module: unfinished\\"] {
        assert!(steps::dep_info(source).is_err());
    }
}

#[test]
fn reruns_track_nearest_manifests_and_bundle_inputs() {
    let root = scratch();
    fs::create_dir_all(root.join("package/src")).expect("sample operation succeeds");
    fs::write(root.join("Cargo.toml"), "").expect("sample operation succeeds");
    fs::write(root.join("package/Cargo.toml"), "").expect("sample operation succeeds");
    let expected = [
        "Cargo.toml",
        "Cargo.lock",
        "package/Cargo.toml",
        "package/src/lib.rs",
        "crates/temper-client-shell-web/temper.css",
        "crates/temper-client-shell-web/task-pages.css",
        "crates/temper-client-shell-web/files",
    ]
    .map(|path| root.join(path))
    .into_iter()
    .collect();
    assert_eq!(steps::rerun_paths(&root, &["package/src/lib.rs".into()]), expected);
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn bundle_reads_page_then_files_before_sorted_subdirectories() {
    let root = scratch();
    for path in ["0123456789abcdef/a", "0123456789abcdef/z"] {
        fs::create_dir_all(root.join(path)).expect("sample operation succeeds");
    }
    for path in [
        "index.html",
        "0123456789abcdef/z/z.js",
        "0123456789abcdef/b.js",
        "0123456789abcdef/a/a.js",
        "0123456789abcdef/c.js",
    ] {
        fs::write(root.join(path), "").expect("sample operation succeeds");
    }
    let expected = [
        "index.html",
        "0123456789abcdef/b.js",
        "0123456789abcdef/c.js",
        "0123456789abcdef/a/a.js",
        "0123456789abcdef/z/z.js",
    ]
    .map(|path| (path.to_owned(), root.join(path)))
    .into();
    assert_eq!(
        steps::bundle(&root).expect("sample operation succeeds"),
        steps::Bundle { digest: "0123456789abcdef".into(), files: expected }
    );
    fs::remove_dir_all(root).expect("sample operation succeeds");
}

#[test]
fn bundle_requires_one_lowercase_hex_digest_directory() {
    for directories in [vec![], vec!["ABCDEF0123456789"], vec!["short"], vec!["0123456789abcdef", "fedcba9876543210"]] {
        let root = scratch();
        fs::write(root.join("index.html"), "").expect("sample operation succeeds");
        for directory in directories {
            fs::create_dir(root.join(directory)).expect("sample operation succeeds");
        }
        assert!(steps::bundle(&root).is_err());
        fs::remove_dir_all(root).expect("sample operation succeeds");
    }
}

#[test]
fn source_quotes_paths_and_embeds_two_sample_files() {
    let bundle = steps::Bundle {
        digest: "0123456789abcdef".into(),
        files: vec![
            ("index.html".into(), "/sample/index.html".into()),
            ("0123456789abcdef/boot.js".into(), "/sample/quote\"/boot.js".into()),
        ],
    };
    assert_eq!(
        steps::source(&bundle),
        "pub const FILES: &[(&str, &[u8])] = &[\n    (\"index.html\", include_bytes!(\"/sample/index.html\")),\n    (\"0123456789abcdef/boot.js\", include_bytes!(\"/sample/quote\\\"/boot.js\")),\n];\npub const DIGEST: Option<&str> = Some(\"0123456789abcdef\");\n"
    );
}

#[test]
fn none_source_has_no_files_or_digest() {
    assert_eq!(steps::NO_BUNDLE, "pub const FILES: &[(&str, &[u8])] = &[];\npub const DIGEST: Option<&str> = None;\n");
}
