use super::*;
use std::ffi::OsString;

const PAGE: &[u8] = b"<!doctype html><html><head></head><body>effective fixture</body></html>";
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
fn initialized_source(scratch: &Scratch) -> Directories {
    let source = scratch.dirs();
    let binary = std::env::current_exe().expect("fixture binary");
    // Optional executables are explicitly absent, independent of the test runner's PATH.
    let initialized = crate::init::write(&source, &[], &binary, [0; 5]).expect("source init without PATH");
    crate::close(initialized.listeners.into_iter().map(|(fd, _)| fd).collect());
    for path in initialized.paths {
        if fs::symlink_metadata(&path).expect("fixture metadata").is_file() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture file");
        }
    }
    source
}
fn provision_bundle(path: &Path) -> PathBuf {
    make_private(&path.join("0000000000000001"), skein_shell::effective_user()).expect("private fixture bundle");
    write(&path.join("index.html"), PAGE, false).expect("fixture page");
    write(&path.join("0000000000000001/main.js"), b"// fixture\n", false).expect("fixture script");
    path.to_path_buf()
}
fn provisioned_source(scratch: &Scratch) -> Directories {
    let source = initialized_source(scratch);
    let bundle = provision_bundle(&scratch.0.join("bundle-loopback"));
    let mut temper = read_json(&source.document());
    temper["application"]["clients"]["bundle"] = bundle.to_str().expect("fixture bundle path").into();
    store_json(&source.document(), &temper);
    source
}

#[test]
fn fake_backends_is_an_additive_box_only_switch() {
    let environment = [(Box::<[u8]>::from(b"HOME".as_slice()), Box::<[u8]>::from(b"/fixture/home".as_slice()))];
    for (words, expected) in [
        (vec!["box", "--fake-backends"], true),
        (vec!["box", "--fake-backends", "--config", "/c", "--data", "/d", "--cache", "/k"], true),
        (vec!["box", "--fake-backends", "--fake-backends"], false),
        (vec!["box", "--fake-backends", "extra"], false),
        (vec!["serve", "--fake-backends"], false),
        (vec!["init", "--fake-backends"], false),
    ] {
        let args = words.iter().map(|s| Box::<[u8]>::from(s.as_bytes())).collect::<Vec<_>>();
        let result = crate::configure_with(&args, &environment, b"/fixture");
        assert_eq!(matches!(result, Ok(crate::Command::BoxFakeBackends { .. })), expected, "{words:?}: {result:?}");
    }
    let args = [Box::<[u8]>::from(b"box".as_slice())];
    assert!(matches!(crate::configure_with(&args, &environment, b"/fixture"), Ok(crate::Command::Box { .. })));
}

#[test]
fn fresh_preparation_is_private_and_isolates_imported_resources() {
    let scratch = Scratch::new("fresh");
    let source = provisioned_source(&scratch);
    let prepared = prepare(&source).expect("prepare effective documents");
    assert_ne!(prepared.directories.config, source.config);
    assert_ne!(prepared.directories.data, source.data);
    assert_eq!(prepared.directories.cache, source.cache);
    assert_eq!(prepared.directories.config, bytes(&scratch.0.join("source-data/box/fake-backends/config")));
    let tree = read_json(&prepared.directories.document());
    assert_eq!(
        tree["workers"][0]["agent"]["configuration"],
        crate::path(&prepared.directories.config).join("agent.json").to_str().expect("path")
    );
    assert_eq!(
        tree["application"]["clients"]["bundle"],
        scratch.0.join("bundle-loopback").to_str().expect("provisioned bundle path")
    );
    validate(&prepared.directories, &prepared.bytes).expect("ordinary deployment and agent checks");
    let root = crate::path(&source.data).join("box/fake-backends");
    for name in ["", "config", "data", "config/charters"] {
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
        "data/worker.secret",
        "data/worker.digest",
    ] {
        assert_eq!(fs::symlink_metadata(root.join(name)).expect("private file").mode() & 0o777, 0o600, "{name}");
    }
    assert!(prepare(&source).err().expect("live sandbox lock").contains("another fake-backend box"));
}

#[test]
fn empty_configuration_refuses_before_creating_data_or_cache() {
    let scratch = Scratch::new("empty");
    let source = scratch.dirs();
    // Both an absent C and a present empty C need ordinary init before provisioning.
    missing_bundle_refuses(&source);
    make_private(crate::path(&source.config), skein_shell::effective_user()).expect("empty configuration");
    missing_bundle_refuses(&source);
}

fn missing_bundle_refuses(source: &Directories) {
    let error = prepare(source).err().expect("no bundle");
    assert!(error.contains("scripts/bundle.sh --loopback OUT"), "{error}");
    assert!(error.contains("application.clients.bundle"), "{error}");
    assert!(error.contains("temper init"), "{error}");
    assert!(!crate::path(&source.data).exists(), "refuse before data mutation");
    assert!(!crate::path(&source.cache).exists(), "refuse before cache mutation");
}

#[test]
fn initialized_source_without_bundle_refuses_before_sandbox_mutation() {
    let scratch = Scratch::new("unprovisioned");
    let source = initialized_source(&scratch);
    // Configuration import must not need any resource under source D.
    fs::remove_dir_all(crate::path(&source.data)).expect("remove source fixture data");
    fs::remove_dir_all(crate::path(&source.cache)).expect("remove source fixture cache");
    missing_bundle_refuses(&source);
}

#[test]
fn partial_sandbox_refuses_with_explicit_reset_instructions() {
    let scratch = Scratch::new("partial");
    let source = provisioned_source(&scratch);
    let (root, _) = directories(&source).expect("sandbox paths");
    make_private(&root, skein_shell::effective_user()).expect("partial fixture");
    let error = prepare(&source).err().expect("partial refused");
    assert!(
        error.contains("partial preparation") && error.contains("remove this sandbox directory explicitly"),
        "{error}"
    );
}

#[test]
fn reuse_keeps_effective_edits_peer_addresses_and_data() {
    let scratch = Scratch::new("reuse");
    let source = provisioned_source(&scratch);
    let first = prepare(&source).expect("fresh sandbox");
    let peer = fs::read(crate::path(&first.directories.config).join("oauth.json")).expect("peer document");
    let data = crate::path(&first.directories.data).join("retained-fixture");
    write(&data, b"keep this data", false).expect("fixture data");
    let mut edited = read_json(&first.directories.document());
    edited["engine"]["deployment"]["seed"] = 42.into();
    let retained_bundle = provision_bundle(&scratch.0.join("retained-bundle"));
    edited["application"]["clients"]["bundle"] = retained_bundle.to_str().expect("retained bundle path").into();
    store_json(&first.directories.document(), &edited);
    let secret = fs::read(crate::path(&first.directories.data).join("worker.secret")).expect("generated test secret");
    drop(first);
    // Source documents stay unchanged, but the retained bundle no longer needs the source directory.
    fs::remove_dir_all(scratch.0.join("bundle-loopback")).expect("remove original provisioned bundle");
    let second = prepare(&source).expect("reuse the retained configuration's bundle");
    assert_eq!(read_json(&second.directories.document()), edited);
    assert_eq!(fs::read(crate::path(&second.directories.config).join("oauth.json")).expect("peer document"), peer);
    assert_eq!(fs::read(crate::path(&second.directories.data).join("worker.secret")).expect("test secret"), secret);
    assert_eq!(fs::read(data).expect("retained data"), b"keep this data");
    assert!(second.listeners.0.is_empty(), "reuse reopens its own listeners");
}

#[test]
fn reuse_refuses_a_missing_effective_bundle_and_preserves_edits() {
    let scratch = Scratch::new("retained-missing-bundle");
    let source = provisioned_source(&scratch);
    let first = prepare(&source).expect("fresh sandbox");
    let document = first.directories.document();
    let mut edited = read_json(&document);
    let absent = scratch.0.join("absent-retained-bundle");
    edited["application"]["clients"]["bundle"] = absent.to_str().expect("absent bundle path").into();
    store_json(&document, &edited);
    drop(first);
    let error = prepare(&source).err().expect("missing retained bundle refused");
    assert!(error.contains("absent-retained-bundle"), "{error}");
    assert_eq!(read_json(&document), edited);
}

#[test]
fn current_init_import_copies_only_public_assets_and_rebases_paths() {
    let scratch = Scratch::new("import");
    let source = provisioned_source(&scratch);
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
    let prepared = prepare(&source).expect("current init-shaped import");
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
    assert_eq!(
        fs::read(crate::path(&source.data).join("worker.secret")).expect("unchanged source test secret"),
        source_secret
    );
    assert_eq!(fs::read(scratch.0.join("bundle-loopback/index.html")).expect("source page unchanged"), PAGE);
    assert_eq!(
        fs::read(scratch.0.join("bundle-loopback/0000000000000001/main.js")).expect("source script unchanged"),
        b"// fixture\n"
    );
    drop(prepared);
    write(&crate::path(&source.config).join("charters/chat.txt"), b"Changed source charter.\n", true)
        .expect("changed public source");
    let error = prepare(&source).err().expect("changed source refuses reuse");
    assert!(error.contains("changed public source documents") && error.contains("reset"), "{error}");
}

#[test]
fn unsupported_imports_refuse_with_the_public_member_named() {
    let scratch = Scratch::new("unsupported");
    let source = provisioned_source(&scratch);
    let mut temper = read_json(&source.document());
    temper["workers"][0]["agent"]["arguments"] = serde_json::json!(["agent", "/elsewhere/agent.json", "extra"]);
    store_json(&source.document(), &temper);
    let error = prepare(&source).err().expect("custom argv refused");
    assert!(error.contains("workers/0/agent/arguments"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn effective_commands_reopen_the_retained_directories() {
    let scratch = Scratch::new("commands");
    let source = provisioned_source(&scratch);
    let prepared = prepare(&source).expect("prepare effective document");
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
fn missing_provisioned_directory_refuses_before_data_or_cache_mutation() {
    let scratch = Scratch::new("missing-bundle");
    let source = provisioned_source(&scratch);
    fs::remove_dir_all(scratch.0.join("bundle-loopback")).expect("remove provisioned fixture");
    fs::remove_dir_all(crate::path(&source.data)).expect("remove source fixture data");
    fs::remove_dir_all(crate::path(&source.cache)).expect("remove source fixture cache");
    missing_bundle_refuses(&source);
}

#[test]
fn unsupported_provisioned_file_refuses_before_sandbox_creation() {
    let scratch = Scratch::new("bundle-type");
    let source = provisioned_source(&scratch);
    write(&scratch.0.join("bundle-loopback/0000000000000001/unsupported.exe"), b"unsupported", false)
        .expect("invalid bundle fixture");
    let error = prepare(&source).err().expect("unsupported bundle file refused");
    assert!(error.contains("application.clients.bundle") && error.contains("unsupported.exe"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn linked_provisioned_page_refuses_before_reading_its_target() {
    let scratch = Scratch::new("bundle-link");
    let source = provisioned_source(&scratch);
    let page = scratch.0.join("bundle-loopback/index.html");
    fs::remove_file(&page).expect("remove fixture page");
    std::os::unix::fs::symlink(scratch.0.join("unread-target"), &page).expect("linked fixture page");
    let error = prepare(&source).err().expect("linked bundle page refused");
    assert!(error.contains("index.html") && error.contains("links"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn source_public_asset_links_are_refused_before_reading_them() {
    let scratch = Scratch::new("source-links");
    let source = provisioned_source(&scratch);
    let charter = crate::path(&source.config).join("charters/chat.txt");
    fs::remove_file(&charter).expect("remove public fixture");
    // The target is absent, so a refusal about links proves it was never opened or copied.
    std::os::unix::fs::symlink(scratch.0.join("unread-target"), &charter).expect("linked public fixture");
    let error = prepare(&source).err().expect("source public link refused");
    assert!(error.contains("links"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn import_preserves_explicitly_disabled_optional_programs() {
    let scratch = Scratch::new("disabled-programs");
    let source = provisioned_source(&scratch);
    let prepared = prepare(&source).expect("import optional choices independently of today's PATH");
    let temper = read_json(&prepared.directories.document());
    let agent = read_json(&crate::path(&prepared.directories.config).join("agent.json"));
    assert!(temper["application"]["workers"][0]["git"].is_null());
    assert_eq!(agent["programs"], serde_json::json!({}));
}
