//! Builds the browser's bundle and writes the list `src/lib.rs` includes (client/browser.md, 2.6), unless
//! `TEMPER_BUNDLE` is `none`.
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

mod steps;

fn embed(out: &std::path::Path, variants: &[steps::Variant]) -> Result<(String, String), String> {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|why| why.to_string())?;
    let profile = env::var("PROFILE").expect("cargo sets PROFILE");
    let profile = steps::profile(&profile)?;
    let target = steps::target_dir(out).ok_or("temper-client-bundle: OUT_DIR has no cargo target ancestor")?;
    let rustc = env::var_os("RUSTC").expect("cargo sets RUSTC");
    let check = Command::new(rustc)
        .args(["--print", "target-libdir", "--target", "wasm32-unknown-unknown"])
        .stderr(Stdio::null())
        .output();
    match check {
        Ok(check) => steps::wasm_target(check.status.success(), &check.stdout)?,
        Err(_) => return Err(steps::NEED_WASM.into()),
    }
    let cargo = env::var_os("CARGO").expect("cargo sets CARGO");
    let mut ordinary = None;
    let mut loopback = None;
    for &variant in variants {
        let directory = steps::bundle_directory(out, variant);
        let commands: Vec<_> = match variant {
            steps::Variant::Ordinary => ["browser module", "bundler build", "bundling"]
                .into_iter()
                .zip(steps::commands(&cargo, &root, &target, out, profile))
                .collect(),
            steps::Variant::Loopback => ["loopback browser module", "bundling"]
                .into_iter()
                .zip(steps::loopback_commands(&cargo, &root, &target, out, profile))
                .collect(),
        };
        for (step, description) in commands {
            if step == "bundling" && directory.exists() {
                fs::remove_dir_all(&directory)
                    .map_err(|why| format!("temper-client-bundle: removing previous bundle: {why}"))?;
            }
            let mut command = Command::new(description.program);
            command.args(description.args).current_dir(description.directory).stdout(Stdio::from(std::io::stderr()));
            for name in description.remove {
                command.env_remove(name);
            }
            for (name, value) in description.set {
                command.env(name, value);
            }
            let status = command.status().map_err(|why| format!("temper-client-bundle: {step} failed: {why}"))?;
            if !status.success() {
                return Err(format!("temper-client-bundle: {step} failed: {status}"));
            }
        }
        // The second module build overwrites the wasm and dep-info. Bundle and record the inputs
        // before that build; include_bytes! points only into the distinct bundled directories.
        let dep = target.join("bundle/wasm32-unknown-unknown").join(profile.1).join("temper_client_shell_web.d");
        let dep =
            fs::read_to_string(dep).map_err(|why| format!("temper-client-bundle: reading module dep-info: {why}"))?;
        for path in steps::rerun_paths(&root, &steps::dep_info(&dep)?) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        let bundle = steps::bundle(&directory)?;
        match variant {
            steps::Variant::Ordinary => ordinary = Some(bundle),
            steps::Variant::Loopback => loopback = Some(bundle),
        }
    }
    Ok(steps::sources(ordinary.as_ref(), loopback.as_ref()))
}

fn main() -> ExitCode {
    println!("cargo:rerun-if-env-changed=TEMPER_BUNDLE");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let list = match steps::choice(env::var_os("TEMPER_BUNDLE").as_deref()) {
        Ok(choice) => {
            let variants = steps::plan(choice, cfg!(feature = "loopback-navigation"));
            if variants.is_empty() { Ok(steps::sources(None, None)) } else { embed(&out, variants) }
        }
        Err(why) => Err(why),
    };
    match list.and_then(|(ordinary, loopback)| {
        for (name, source) in [("bundle.rs", ordinary), ("loopback.rs", loopback)] {
            fs::write(out.join(name), source)
                .map_err(|why| format!("temper-client-bundle: writing {name} list: {why}"))?;
        }
        Ok(())
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
