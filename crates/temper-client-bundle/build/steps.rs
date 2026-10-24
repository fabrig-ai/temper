//! The build script's logic, plain functions over their inputs, tested on samples by `tests/steps.rs`
//! (client/browser.md, 2.6).
use std::ffi::OsStr;

/// What `TEMPER_BUNDLE` asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Choice {
    Embed,
    None,
}
/// `TEMPER_BUNDLE`: unset or `embed`, the embed; `none`, no bundle; any other value is the build's failure.
pub fn choice(value: Option<&OsStr>) -> Result<Choice, String> {
    match value.map(OsStr::as_encoded_bytes) {
        None | Some(b"embed") => Ok(Choice::Embed),
        Some(b"none") => Ok(Choice::None),
        Some(_) => Err(format!(
            "temper-client-bundle: TEMPER_BUNDLE must be embed or none, not {}",
            value.unwrap_or_default().display()
        )),
    }
}
/// The list of a build without the bundle.
pub const NO_BUNDLE: &str = "pub const FILES: &[(&str, &[u8])] = &[];\npub const DIGEST: Option<&str> = None;\n";

/// Which bundles the build packages, in execution order; an empty plan does no nested work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Ordinary,
    Loopback,
}

pub fn plan(choice: Choice, loopback: bool) -> &'static [Variant] {
    match choice {
        Choice::None => &[],
        Choice::Embed => {
            if loopback {
                &[Variant::Ordinary, Variant::Loopback]
            } else {
                &[Variant::Ordinary]
            }
        }
    }
}

/// Keep the two include lists independent; an absent bundle always exports the empty API.
pub fn sources(ordinary: Option<&Bundle>, loopback: Option<&Bundle>) -> (String, String) {
    (ordinary.map_or_else(|| NO_BUNDLE.into(), source), loopback.map_or_else(|| NO_BUNDLE.into(), source))
}

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fmt::Write,
    fs, io,
    path::{Path, PathBuf},
};

/// The module's cargo profile and its output directory.
pub fn profile(value: &str) -> Result<(&'static str, &'static str), String> {
    match value {
        "debug" => Ok(("dev", "debug")),
        "release" => Ok(("web", "web")),
        _ => Err(format!("temper-client-bundle: unsupported PROFILE {value:?}")),
    }
}

/// Cargo's nearest target directory, including builds with an explicit target triple.
pub fn target_dir(out: &Path) -> Option<PathBuf> {
    out.ancestors().find(|at| at.join("CACHEDIR.TAG").is_file()).map(Path::to_path_buf)
}

pub const NEED_WASM: &str = "temper-client-bundle: the browser's bundle needs the wasm32-unknown-unknown target: add it (rustup target add wasm32-unknown-unknown), or build without the browser (TEMPER_BUNDLE=none)";

/// Decide from rustc's exit status and printed target library directory.
pub fn wasm_target(success: bool, stdout: &[u8]) -> Result<(), &'static str> {
    if success
        && std::str::from_utf8(stdout).is_ok_and(|path| !path.trim().is_empty() && Path::new(path.trim()).is_dir())
    {
        Ok(())
    } else {
        Err(NEED_WASM)
    }
}

/// One child invocation; inherited environment is retained except for the named removals.
#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub remove: Vec<&'static str>,
    pub set: Vec<(&'static str, OsString)>,
    pub directory: PathBuf,
}

/// The nested cargo builds and bundler, in execution order (client/browser.md, 2.6).
pub fn commands(cargo: &OsStr, root: &Path, target: &Path, out: &Path, profile: (&str, &str)) -> [Command; 3] {
    variant_commands(cargo, root, target, out, profile, Variant::Ordinary)
}

/// Reuse the built bundler, rebuilding the module with loopback navigation into a separate bundle.
pub fn loopback_commands(cargo: &OsStr, root: &Path, target: &Path, out: &Path, profile: (&str, &str)) -> [Command; 2] {
    let [module, _, bundler] = variant_commands(cargo, root, target, out, profile, Variant::Loopback);
    [module, bundler]
}

pub fn bundle_directory(out: &Path, variant: Variant) -> PathBuf {
    out.join(match variant {
        Variant::Ordinary => "bundle",
        Variant::Loopback => "bundle-loopback",
    })
}

fn variant_commands(
    cargo: &OsStr,
    root: &Path,
    target: &Path,
    out: &Path,
    profile: (&str, &str),
    variant: Variant,
) -> [Command; 3] {
    let nested = target.join("bundle");
    let build = |args: &[&str]| Command {
        program: cargo.to_owned(),
        args: args.iter().map(OsString::from).collect(),
        remove: vec!["CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTC_WORKSPACE_WRAPPER", "CLIPPY_ARGS"],
        set: vec![("CARGO_TARGET_DIR", nested.clone().into_os_string())],
        directory: root.to_path_buf(),
    };
    let shell = root.join("crates/temper-client-shell-web");
    let module = nested.join("wasm32-unknown-unknown").join(profile.1).join("temper_client_shell_web.wasm");
    let mut module_build = build(&[
        "build",
        "--locked",
        "--target",
        "wasm32-unknown-unknown",
        "--profile",
        profile.0,
        "-p",
        "temper-client-shell-web",
    ]);
    match variant {
        Variant::Ordinary => {}
        Variant::Loopback => module_build.args.extend(["--features".into(), "loopback-navigation".into()]),
    }
    [
        module_build,
        build(&["build", "--locked", "--release", "-p", "skein-ui-bundle"]),
        Command {
            program: nested.join("release/skein-ui-bundle").into_os_string(),
            args: vec![
                "--module".into(),
                module.into_os_string(),
                "--name".into(),
                "temper_client_shell_web".into(),
                "--prefix".into(),
                "/bundle/".into(),
                "--stylesheet".into(),
                shell.join("temper.css").into_os_string(),
                "--stylesheet".into(),
                shell.join("task-pages.css").into_os_string(),
                "--files".into(),
                shell.join("files").into_os_string(),
                "--noscript".into(),
                "temper needs WebAssembly and scripts.".into(),
                "--out".into(),
                bundle_directory(out, variant).into_os_string(),
            ],
            remove: vec![],
            set: vec![],
            directory: root.to_path_buf(),
        },
    ]
}

/// Read the first make rule's prerequisites, honoring escaped spaces and line continuations.
pub fn dep_info(source: &str) -> Result<Vec<PathBuf>, String> {
    let joined = source.replace("\\\n", "");
    let rule = joined.lines().next().ok_or("temper-client-bundle: empty dep-info")?;
    let (_, prerequisites) = rule.split_once(':').ok_or("temper-client-bundle: dep-info has no target separator")?;
    let mut files = Vec::new();
    let mut word = String::new();
    let mut escaped = false;
    for ch in prerequisites.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch.is_whitespace() {
            if !word.is_empty() {
                files.push(PathBuf::from(std::mem::take(&mut word)));
            }
        } else {
            word.push(ch);
        }
    }
    if escaped {
        return Err("temper-client-bundle: unfinished dep-info escape".into());
    }
    if !word.is_empty() {
        files.push(PathBuf::from(word));
    }
    Ok(files)
}

/// Source files, their nearest package manifests, and the bundle's other inputs.
pub fn rerun_paths(root: &Path, files: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::from([root.join("Cargo.toml"), root.join("Cargo.lock")]);
    for file in files {
        let file = root.join(file);
        if let Some(manifest) = file.ancestors().skip(1).map(|at| at.join("Cargo.toml")).find(|at| at.is_file()) {
            paths.insert(manifest);
        }
        paths.insert(file);
    }
    for relative in ["temper.css", "task-pages.css", "files"] {
        paths.insert(root.join("crates/temper-client-shell-web").join(relative));
    }
    paths
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bundle {
    pub digest: String,
    pub files: Vec<(String, PathBuf)>,
}

fn entries(directory: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

/// Read index first, then files at each directory in name order, descending after its files.
pub fn bundle(root: &Path) -> Result<Bundle, String> {
    let read = || -> io::Result<Bundle> {
        let root = root.canonicalize()?;
        let mut digest = None;
        for entry in entries(&root)? {
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().into_string().map_err(|_| io::Error::other("digest is not UTF-8"))?;
                if digest.is_some()
                    || name.len() != 16
                    || !name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(io::Error::other("expected one digest directory of 16 lowercase hex digits"));
                }
                digest = Some(name);
            }
        }
        let digest = digest.ok_or_else(|| io::Error::other("no digest directory"))?;
        if !root.join("index.html").is_file() {
            return Err(io::Error::other("no index.html"));
        }
        let mut files = vec![("index.html".into(), root.join("index.html"))];
        let mut pending = vec![PathBuf::from(&digest)];
        while let Some(relative) = pending.pop() {
            let mut directories = Vec::new();
            for entry in entries(&root.join(&relative))? {
                let path = relative.join(entry.file_name());
                let kind = entry.file_type()?;
                if kind.is_dir() {
                    directories.push(path);
                } else if kind.is_file() {
                    files.push((
                        path.to_str().ok_or_else(|| io::Error::other("path is not UTF-8"))?.into(),
                        root.join(path),
                    ));
                } else {
                    return Err(io::Error::other("entry must be a regular file or directory"));
                }
            }
            pending.extend(directories.into_iter().rev());
        }
        Ok(Bundle { digest, files })
    };
    read().map_err(|why| format!("temper-client-bundle: {}: {why}", root.display()))
}

/// The exact source included by the library, with Rust string escaping for paths.
pub fn source(bundle: &Bundle) -> String {
    let mut source = String::from("pub const FILES: &[(&str, &[u8])] = &[\n");
    for (name, path) in &bundle.files {
        #[expect(clippy::unnecessary_debug_formatting, reason = "Rust source needs quoted and escaped paths")]
        writeln!(source, "    ({name:?}, include_bytes!({path:?})),").expect("write to String");
    }
    writeln!(source, "];\npub const DIGEST: Option<&str> = Some({:?});", bundle.digest).expect("write to String");
    source
}
