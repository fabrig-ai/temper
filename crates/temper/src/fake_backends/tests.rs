use super::*;
use std::ffi::OsString;

const PAGE: &[u8] = b"<!doctype html><html><head></head><body>effective fixture</body></html>";
const FILES: &[(&str, &[u8])] = &[("index.html", PAGE), ("0000000000000001/main.js", b"// fixture\n")];
struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("temper-fake-backends-{}-{name}", std::process::id()));
        fs::create_dir(&path).expect("new private scratch");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private scratch");
        Self(path)
    }
    fn dirs(&self) -> Directories {
        Directories {
            config: bytes(&self.0.join("source-config")),
            data: bytes(&self.0.join("source-data")),
            cache: bytes(&self.0.join("cache")),
            origins: [skein_shell::Origin::Flag; 3],
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove private fixture");
    }
}
fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).expect("fixture bytes")).expect("fixture JSON")
}
fn store_json(path: &Path, value: &serde_json::Value) {
    write(path, &serde_json::to_vec(value).expect("fixture JSON"), true).expect("write fixture JSON");
}

#[test]
fn fake_backends_is_an_additive_box_only_switch() {
    for (words, expected) in [
        (vec!["box", "--fake-backends"], true),
        (vec!["box", "--fake-backends", "--config", "/c", "--data", "/d", "--cache", "/k"], true),
        (vec!["box", "--fake-backends", "--fake-backends"], false),
        (vec!["box", "--fake-backends", "extra"], false),
        (vec!["serve", "--fake-backends"], false),
        (vec!["init", "--fake-backends"], false),
    ] {
        let args = words.iter().map(|s| Box::<[u8]>::from(s.as_bytes())).collect::<Vec<_>>();
        let result = crate::configure_with(&args, &[], b"/fixture");
        assert_eq!(matches!(result, Ok(crate::Command::BoxFakeBackends { .. })), expected, "{words:?}");
    }
    let args = [Box::<[u8]>::from(b"box".as_slice())];
    assert!(matches!(crate::configure_with(&args, &[], b"/fixture"), Ok(crate::Command::Box { .. })));
}

#[test]
fn fresh_preparation_is_private_isolated_and_holds_init_listeners() {
    let scratch = Scratch::new("fresh");
    let source = scratch.dirs();
    let prepared = prepare(&source, FILES).expect("prepare effective documents");
    assert!(!crate::path(&source.config).exists(), "source C is not initialized");
    assert!(!crate::path(&source.data).join("worker.secret").exists(), "source D receives no deployment");
    assert_eq!(prepared.directories.cache, source.cache);
    assert_eq!(prepared.listeners.0.len(), 5, "the five init listeners stay owned");
    assert_eq!(prepared.directories.config, bytes(&scratch.0.join("source-data/box/fake-backends/config")));
    let tree = read_json(&prepared.directories.document());
    assert_eq!(
        tree["workers"][0]["agent"]["configuration"],
        crate::path(&prepared.directories.config).join("agent.json").to_str().expect("path")
    );
    validate(&prepared.directories, &prepared.bytes).expect("ordinary deployment and agent checks");
    let root = crate::path(&source.data).join("box/fake-backends");
    for name in
        ["", "config", "data", "config/charters", "config/bundle-loopback", "config/bundle-loopback/0000000000000001"]
    {
        assert_eq!(fs::symlink_metadata(root.join(name)).expect("private directory").mode() & 0o777, 0o700, "{name}");
    }
    for name in [
        "prepared",
        "lock",
        "config/temper.json",
        "config/agent.json",
        "config/github.json",
        "config/oauth.json",
        "config/llm.json",
        "config/charters/chat.txt",
        "config/charters/coding.txt",
        "config/bundle-loopback/index.html",
        "data/worker.secret",
        "data/worker.digest",
    ] {
        assert_eq!(fs::symlink_metadata(root.join(name)).expect("private file").mode() & 0o777, 0o600, "{name}");
    }
    assert!(prepare(&source, FILES).err().expect("live sandbox lock").contains("another fake-backend box"));
}

#[test]
fn missing_bundle_and_partial_sandboxes_are_named_refusals() {
    let scratch = Scratch::new("refusals");
    let source = scratch.dirs();
    let error = prepare(&source, &[]).err().expect("no bundle");
    assert!(error.contains("scripts/bundle.sh --loopback"));
    assert!(error.contains("application.clients.bundle"));
    assert!(!crate::path(&source.data).exists(), "refuse before sandbox mutation");
    let (root, _) = directories(&source).expect("sandbox paths");
    make_private(&root, skein_shell::effective_user()).expect("partial fixture");
    let error = prepare(&source, FILES).err().expect("partial refused");
    assert!(error.contains("reset"), "{error}");
}

#[test]
fn reuse_keeps_effective_edits_peer_addresses_and_data() {
    let scratch = Scratch::new("reuse");
    let source = scratch.dirs();
    let first = prepare(&source, FILES).expect("fresh sandbox");
    let peer = fs::read(crate::path(&first.directories.config).join("oauth.json")).expect("peer document");
    let data = crate::path(&first.directories.data).join("retained-fixture");
    write(&data, b"keep this data", false).expect("fixture data");
    let mut edited = read_json(&first.directories.document());
    edited["engine"]["deployment"]["seed"] = 42.into();
    store_json(&first.directories.document(), &edited);
    let secret = fs::read(crate::path(&first.directories.data).join("worker.secret")).expect("generated test secret");
    drop(first);
    // Retained configuration, rather than this invocation's package, is authoritative.
    let second = prepare(&source, &[]).expect("retained sandbox without a current packaged bundle");
    assert_eq!(read_json(&second.directories.document()), edited);
    assert_eq!(fs::read(crate::path(&second.directories.config).join("oauth.json")).expect("peer document"), peer);
    assert_eq!(fs::read(crate::path(&second.directories.data).join("worker.secret")).expect("test secret"), secret);
    assert_eq!(fs::read(data).expect("retained data"), b"keep this data");
    assert!(second.listeners.0.is_empty(), "reuse reopens its own listeners");
}

#[test]
fn current_init_import_copies_only_public_assets_and_rebases_paths() {
    let scratch = Scratch::new("import");
    let source = scratch.dirs();
    let initialized = crate::init::core(&source).expect("source fixture init");
    crate::close(initialized.listeners.into_iter().map(|(fd, _)| fd).collect());
    let mut temper = read_json(&source.document());
    // Equivalent absolute resource spelling is admitted without following source D's resources.
    temper["workers"][0]["secret"] = crate::path(&source.data).join("worker.secret").to_str().expect("path").into();
    temper["engine"]["charters"][0]["instructions"] =
        crate::path(&source.config).join("charters/chat.txt").to_str().expect("path").into();
    store_json(&source.document(), &temper);
    write(&crate::path(&source.config).join("charters/chat.txt"), b"A custom public charter.\n", true)
        .expect("public charter");
    let before = ["temper.json", "agent.json", "charters/chat.txt", "charters/coding.txt"].map(|name| {
        let path = crate::path(&source.config).join(name);
        (name, fs::read(&path).expect("source public fixture"), fs::metadata(path).expect("mode").mode())
    });
    let source_secret = fs::read(crate::path(&source.data).join("worker.secret")).expect("generated fixture secret");
    let prepared = prepare(&source, FILES).expect("current init-shaped import");
    assert_ne!(
        fs::read(crate::path(&prepared.directories.data).join("worker.secret")).expect("fresh test secret"),
        source_secret
    );
    assert_eq!(
        fs::read(crate::path(&prepared.directories.config).join("charters/chat.txt")).expect("effective charter"),
        b"A custom public charter.\n"
    );
    for (name, contents, mode) in &before {
        let path = crate::path(&source.config).join(name);
        assert_eq!(fs::read(&path).expect("source unchanged"), *contents, "{name}");
        assert_eq!(fs::metadata(path).expect("source mode").mode(), *mode, "{name}");
    }
    drop(prepared);
    write(&crate::path(&source.config).join("charters/chat.txt"), b"Changed source charter.\n", true)
        .expect("changed public source");
    let error = prepare(&source, FILES).err().expect("changed source refuses reuse");
    assert!(error.contains("changed public source documents") && error.contains("reset"), "{error}");
}

#[test]
fn unsupported_imports_refuse_with_the_public_member_named() {
    let scratch = Scratch::new("unsupported");
    let source = scratch.dirs();
    let initialized = crate::init::core(&source).expect("source fixture init");
    crate::close(initialized.listeners.into_iter().map(|(fd, _)| fd).collect());
    let mut temper = read_json(&source.document());
    temper["workers"][0]["agent"]["arguments"] = serde_json::json!(["agent", "/elsewhere/agent.json", "extra"]);
    store_json(&source.document(), &temper);
    let error = prepare(&source, FILES).err().expect("custom argv refused");
    assert!(error.contains("workers/0/agent/arguments"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn effective_commands_reopen_the_retained_directories() {
    let scratch = Scratch::new("commands");
    let source = scratch.dirs();
    let prepared = prepare(&source, FILES).expect("prepare effective document");
    let deployment =
        crate::setup::check(&prepared.bytes, &prepared.directories.jig()).expect("checked effective setup");
    let steps = temper_fake::steps(
        &deployment.checked.setup,
        deployment.demo.as_ref(),
        &prepared.directories.flags(),
        Box::new([]),
        true,
    );
    for step in steps.always.iter().chain(steps.asked.iter().flat_map(|group| group.steps.iter().map(|(_, step)| step)))
    {
        // All setup/serve commands receive the effective directory flags, including the retained cache.
        let args =
            step.arguments().iter().map(|arg| OsString::from(std::ffi::OsStr::from_bytes(arg))).collect::<Vec<_>>();
        for (flag, value) in [
            ("--config", &prepared.directories.config),
            ("--data", &prepared.directories.data),
            ("--cache", &prepared.directories.cache),
        ] {
            assert!(
                args.windows(2).any(|pair| pair[0] == flag && pair[1].as_os_str().as_bytes() == value.as_ref()),
                "{flag}: {args:?}"
            );
        }
    }
}

#[test]
fn package_paths_and_file_types_refuse_before_sandbox_mutation() {
    let scratch = Scratch::new("bundle-paths");
    let source = scratch.dirs();
    for files in [
        vec![("index.html", PAGE), ("/escape/main.js", b"script".as_slice())],
        vec![("index.html", PAGE), ("0000000000000001/../escape.js", b"script".as_slice())],
        vec![("index.html", PAGE), ("0000000000000001/secret.txt", b"not a bundle type".as_slice())],
        vec![("index.html", PAGE), ("index.html", PAGE), ("0000000000000001/main.js", b"script".as_slice())],
    ] {
        let error = prepare(&source, &files).err().expect("bad package refused");
        assert!(error.contains("bundle"), "{error}");
        assert!(!crate::path(&source.data).exists(), "bad package must refuse before sandbox creation");
    }
}

#[test]
fn source_public_asset_links_are_refused_before_reading_them() {
    let scratch = Scratch::new("source-links");
    let source = scratch.dirs();
    let initialized = crate::init::core(&source).expect("source fixture init");
    crate::close(initialized.listeners.into_iter().map(|(fd, _)| fd).collect());
    let charter = crate::path(&source.config).join("charters/chat.txt");
    fs::remove_file(&charter).expect("remove public fixture");
    // The target is absent, so a refusal about links proves it was never opened or copied.
    std::os::unix::fs::symlink(scratch.0.join("unread-target"), &charter).expect("linked public fixture");
    let error = prepare(&source, FILES).err().expect("source public link refused");
    assert!(error.contains("links"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn import_keeps_optional_programs_disabled_when_init_found_none() {
    let scratch = Scratch::new("disabled-programs");
    let source = scratch.dirs();
    let binary = std::env::current_exe().expect("fixture binary");
    let initialized = crate::init::write(&source, &[], &binary, [0; 5]).expect("source init without PATH");
    crate::close(initialized.listeners.into_iter().map(|(fd, _)| fd).collect());
    let prepared = prepare(&source, FILES).expect("import optional choices independently of today's PATH");
    let temper = read_json(&prepared.directories.document());
    let agent = read_json(&crate::path(&prepared.directories.config).join("agent.json"));
    assert!(temper["application"]["workers"][0]["git"].is_null());
    assert_eq!(agent["programs"], serde_json::json!({}));
}
