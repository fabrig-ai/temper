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
// A unit test binary has no fake-claude sibling: each scratch provisions an adapter beside its source directories.
fn start(source: &Directories, bundle: Option<&Path>) -> Result<Prepared, String> {
    let adapter = crate::path(&source.config).parent().expect("scratch root").join("fake-claude");
    if !adapter.exists() {
        write(&adapter, b"#!/bin/sh\nexit 1\n", false).expect("fixture adapter");
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).expect("executable fixture adapter");
    }
    prepare(source, bundle, &adapter)
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
fn program_login_line_names_the_model_steps_placeholder_and_quotes_effective_directories() {
    let dirs = Directories {
        config: b"/effective owner's/config".as_slice().into(),
        data: b"/effective data".as_slice().into(),
        cache: b"/shared cache".as_slice().into(),
        origins: [skein_shell::Origin::Flag; 3],
    };
    assert_eq!(
        program_login_line(&dirs, Path::new("/bin owner's/temper")),
        "'/bin owner'\\''s/temper' program-login --config '/effective owner'\\''s/config' --data '/effective data' --cache '/shared cache' NAME (the name the Model step shows)"
    );
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

fn configure(words: &[&[u8]]) -> Result<crate::Command, String> {
    let environment = [(Box::<[u8]>::from(b"HOME".as_slice()), Box::<[u8]>::from(b"/fixture/home".as_slice()))];
    let args = words.iter().map(|word| Box::<[u8]>::from(*word)).collect::<Vec<_>>();
    crate::configure_with(&args, &environment, b"/fixture")
}

#[test]
fn bundle_paths_resolve_against_the_invocation_directory() {
    for (given, expected) in
        [(b"assets".as_slice(), "/fixture/assets"), (b"/bundles/loopback".as_slice(), "/bundles/loopback")]
    {
        let command = configure(&[b"box", b"--fake-backends", b"--bundle", given]).expect("bundle arguments");
        let crate::Command::BoxFakeBackends { bundle, .. } = command else { panic!("fake box") };
        assert_eq!(bundle, Some(PathBuf::from(expected)));
    }
}

#[test]
fn bundle_and_fake_flags_allow_interleaved_directory_settings() {
    let cases: &[&[&[u8]]] = &[
        &[b"box", b"--bundle", b"assets", b"--config", b"/c", b"--fake-backends", b"--data", b"/d", b"--cache", b"/k"],
        &[b"box", b"--data", b"/d", b"--fake-backends", b"--cache", b"/k", b"--bundle", b"assets", b"--config", b"/c"],
    ];
    for words in cases {
        let command = configure(words).expect("interleaved bundle and directory settings");
        let crate::Command::BoxFakeBackends { directories, bundle, .. } = command else { panic!("fake box") };
        assert_eq!(bundle, Some(PathBuf::from("/fixture/assets")));
        assert_eq!(directories.config.as_ref(), b"/c");
        assert_eq!(directories.data.as_ref(), b"/d");
        assert_eq!(directories.cache.as_ref(), b"/k");
    }
}

#[test]
fn malformed_bundle_grammar_is_refused() {
    let cases: &[&[&[u8]]] = &[
        &[b"box", b"--bundle", b"assets"],
        &[b"box", b"--fake-backends", b"--bundle"],
        &[b"box", b"--bundle", b"--fake-backends"],
        &[b"box", b"--fake-backends", b"--bundle", b"assets", b"extra"],
        &[b"box", b"--fake-backends", b"--bundle", b"assets", b"--bundle", b"other"],
        &[b"box", b"--fake-backends", b"--bundle", b"assets", b"--fake-backends"],
        &[b"serve", b"--fake-backends", b"--bundle", b"assets"],
        &[b"init", b"--bundle", b"assets"],
    ];
    for words in cases {
        assert!(configure(words).is_err(), "{words:?}");
    }
}

#[test]
fn bundle_paths_check_both_sides_of_the_absolute_bound() {
    // configuration.md, 12: the resolved absolute path is bounded at 4096 bytes.
    const PATH_BYTES: usize = 4096;
    const WORKING: &[u8] = b"/fixture/";
    let mut absolute = vec![b'a'; PATH_BYTES];
    absolute[0] = b'/';
    let relative = vec![b'a'; PATH_BYTES - WORKING.len()];
    for (given, accepted) in [
        (absolute.clone(), true),
        ([absolute, b"a".to_vec()].concat(), false),
        (relative.clone(), true),
        ([relative, b"a".to_vec()].concat(), false),
        (Vec::new(), false),
        (b"assets\0other".to_vec(), false),
    ] {
        assert_eq!(configure(&[b"box", b"--fake-backends", b"--bundle", &given]).is_ok(), accepted);
    }
}

fn prepare_empty(scratch: &Scratch, source: &Directories) -> Prepared {
    // Init holds five ordinary listeners; the four welcome peers each add one.
    const INIT_LISTENERS: usize = 9;
    let supplied = provision_bundle(&scratch.0.join("explicit-bundle"));
    let prepared = start(source, Some(&supplied)).expect("init directly into effective directories");
    let tree = read_json(&prepared.directories.document());
    assert_eq!(tree["application"]["clients"]["bundle"], supplied.to_str().expect("fixture bundle path"));
    assert!(crate::path(&prepared.directories.data).join("worker.secret").is_file());
    assert!(!source.document().exists());
    assert_eq!(prepared.listeners.0.len(), INIT_LISTENERS);
    for (_, port) in &prepared.listeners.0 {
        assert!(TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.get())).is_err(), "listener stays held");
    }
    validate(&prepared.directories, &prepared.bytes, &scratch.0.join("fake-claude"))
        .expect("ordinary effective config and agent checks");
    prepared
}

#[test]
fn peer_reservations_transfer_once_and_close_with_preparation() {
    let scratch = Scratch::new("peer-ownership");
    let source = scratch.dirs();
    let prepared = prepare_empty(&scratch, &source);
    let peers = prepared.peers;
    for address in [peers.github, peers.oauth, peers.llm, peers.forgejo] {
        assert_eq!(prepared.listeners.0.iter().filter(|(_, port)| port.get() == address.port()).count(), 1);
        assert!(TcpListener::bind(address).is_err(), "peer reservation remains owned");
    }
    drop(prepared);
    // Hold the released addresses so reuse would fail if it tried to bind any peer.
    let held: Vec<TcpListener> = [peers.github, peers.oauth, peers.llm, peers.forgejo]
        .into_iter()
        .map(|address| TcpListener::bind(address).expect("owned peer fd closes on preparation drop"))
        .collect();
    let reused = start(&source, None).expect("retained configuration");
    assert_eq!(reused.peers, peers);
    assert!(reused.listeners.0.is_empty(), "reuse leaves binding to the ordinary box");
    drop(held);
}

#[test]
fn retained_peer_edits_are_checked_without_replacement() {
    for kind in ["edited", "zero", "collision", "ordinary", "outside", "oauth"] {
        let scratch = Scratch::new(&format!("peer-validation-{kind}"));
        let source = provisioned_source(&scratch);
        let first = start(&source, None).expect("fresh sandbox");
        let file = if kind == "oauth" { "oauth.json" } else { "llm.json" };
        let path = crate::path(&first.directories.config).join(file);
        let mut edited = read_json(&path);
        let mut reservation = None;
        match kind {
            "edited" => {
                let socket =
                    reserve(&[1455, first.peers.github.port(), first.peers.oauth.port()]).expect("edited peer port");
                edited["listen"]["socket"] = addr(&socket).expect("socket").to_string().into();
                reservation = Some(socket);
            }
            "zero" => edited["listen"]["socket"] = "127.0.0.1:0".into(),
            "collision" => edited["listen"]["socket"] = first.peers.github.to_string().into(),
            "ordinary" => {
                edited["listen"]["socket"] =
                    read_json(&first.directories.document())["application"]["clients"]["listener"]["socket"].clone();
            }
            "outside" => edited["key"] = "/outside/llm.key".into(),
            "oauth" => edited["redirect_port"] = 0.into(),
            _ => unreachable!("fixture kind"),
        }
        store_json(&path, &edited);
        drop(first);
        let result = start(&source, None);
        if kind == "edited" {
            assert_eq!(
                result.expect("safe effective edit retained").peers.llm.to_string(),
                edited["listen"]["socket"].as_str().expect("edited socket")
            );
        } else {
            let error = result.err().expect("unsafe retained peer refused");
            assert!(error.contains(file), "{kind}: {error}");
        }
        assert_eq!(read_json(&path), edited, "retained edits never silently replaced");
        drop(reservation);
    }
}

#[test]
fn explicit_bundle_initializes_without_creating_absent_source_configuration() {
    let scratch = Scratch::new("absent-explicit");
    let source = scratch.dirs();
    let _prepared = prepare_empty(&scratch, &source);
    assert!(!crate::path(&source.config).exists());
}

#[test]
fn explicit_bundle_leaves_existing_empty_source_configuration_untouched() {
    let scratch = Scratch::new("empty-explicit");
    let source = scratch.dirs();
    make_private(crate::path(&source.config), skein_shell::effective_user()).expect("empty source configuration");
    let before = fs::symlink_metadata(crate::path(&source.config)).expect("source directory metadata");
    let _prepared = prepare_empty(&scratch, &source);
    let after = fs::symlink_metadata(crate::path(&source.config)).expect("unchanged source directory");
    assert!(fs::read_dir(crate::path(&source.config)).expect("source remains empty").next().is_none());
    assert_eq!(after.ino(), before.ino());
    assert_eq!(after.mode(), before.mode());
    assert_eq!(after.mtime(), before.mtime());
    assert_eq!(after.mtime_nsec(), before.mtime_nsec());
}

#[test]
fn explicit_bundle_overrides_an_unavailable_source_bundle_without_rewriting_source() {
    let scratch = Scratch::new("source-override");
    let source = provisioned_source(&scratch);
    let original = fs::read(source.document()).expect("source public bytes");
    fs::remove_dir_all(scratch.0.join("bundle-loopback")).expect("source bundle unavailable");
    let supplied = provision_bundle(&scratch.0.join("override"));
    let first = start(&source, Some(&supplied)).expect("explicit selection overrides source bundle");
    assert_eq!(
        read_json(&first.directories.document())["application"]["clients"]["bundle"],
        supplied.to_str().expect("supplied path")
    );
    let retained = first.bytes.clone();
    drop(first);
    let second = start(&source, None).expect("restart without repeating the selection");
    assert_eq!(second.bytes, retained);
    assert_eq!(fs::read(source.document()).expect("unchanged source bytes"), original);
}

#[test]
fn explicit_bundle_imports_init_configuration_without_a_source_bundle() {
    let scratch = Scratch::new("init-explicit");
    let source = initialized_source(&scratch);
    let original = fs::read(source.document()).expect("source document");
    let supplied = provision_bundle(&scratch.0.join("supplied"));
    let prepared = start(&source, Some(&supplied)).expect("init-shaped source plus explicit bundle");
    assert_eq!(
        read_json(&prepared.directories.document())["application"]["clients"]["bundle"],
        supplied.to_str().expect("supplied path")
    );
    assert_eq!(fs::read(source.document()).expect("source unchanged"), original);
}

#[test]
fn fresh_preparation_is_private_and_isolates_imported_resources() {
    let scratch = Scratch::new("fresh");
    let source = provisioned_source(&scratch);
    let prepared = start(&source, None).expect("prepare effective documents");
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
    validate(&prepared.directories, &prepared.bytes, &scratch.0.join("fake-claude"))
        .expect("ordinary deployment and agent checks");
    let root = crate::path(&source.data).join("box/fake-backends");
    for name in ["", "config", "data"] {
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
        "data/worker.secret",
        "data/worker.digest",
    ] {
        assert_eq!(fs::symlink_metadata(root.join(name)).expect("private file").mode() & 0o777, 0o600, "{name}");
    }
    assert!(start(&source, None).err().expect("live sandbox lock").contains("another fake-backend box"));
}

#[test]
fn empty_configuration_refuses_before_creating_data_or_cache() {
    let scratch = Scratch::new("empty");
    let source = scratch.dirs();
    // Missing provisioning refuses for both absent C and present empty C.
    missing_bundle_refuses(&source);
    make_private(crate::path(&source.config), skein_shell::effective_user()).expect("empty configuration");
    missing_bundle_refuses(&source);
}

fn missing_bundle_refuses(source: &Directories) {
    let error = start(source, None).err().expect("no bundle");
    assert!(error.contains("scripts/bundle.sh --loopback DIR"), "{error}");
    assert!(error.contains("application.clients.bundle"), "{error}");
    assert!(error.contains("temper box --fake-backends --bundle BUNDLE"), "{error}");
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
    let error = start(&source, None).err().expect("partial refused");
    assert!(
        error.contains("partial preparation") && error.contains("remove this sandbox directory explicitly"),
        "{error}"
    );
}

#[test]
fn reuse_keeps_effective_edits_peer_addresses_and_data() {
    let scratch = Scratch::new("reuse");
    let source = provisioned_source(&scratch);
    let first = start(&source, None).expect("fresh sandbox");
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
    let second = start(&source, None).expect("reuse the retained configuration's bundle");
    assert_eq!(read_json(&second.directories.document()), edited);
    assert_eq!(fs::read(crate::path(&second.directories.config).join("oauth.json")).expect("peer document"), peer);
    assert_eq!(fs::read(crate::path(&second.directories.data).join("worker.secret")).expect("test secret"), secret);
    assert_eq!(fs::read(data).expect("retained data"), b"keep this data");
    assert!(second.listeners.0.is_empty(), "reuse reopens its own listeners");
}

fn edit_retained(prepared: &Prepared) -> serde_json::Value {
    let mut edited = read_json(&prepared.directories.document());
    edited["engine"]["deployment"]["seed"] = 42.into();
    store_json(&prepared.directories.document(), &edited);
    edited
}

fn prepared_source(scratch: &Scratch) -> (Directories, Prepared) {
    let source = provisioned_source(scratch);
    let prepared = start(&source, None).expect("fresh sandbox");
    (source, prepared)
}

#[test]
fn reuse_atomically_replaces_bundle_and_preserves_every_other_edit() {
    use std::io::Read;
    let scratch = Scratch::new("reuse-override");
    let (source, first) = prepared_source(&scratch);
    let mut edited = edit_retained(&first);
    let document = first.directories.document();
    let old_bytes = fs::read(&document).expect("retained document bytes");
    let old_inode = fs::metadata(&document).expect("retained inode").ino();
    let mut old_file = fs::File::open(&document).expect("hold old retained document open");
    let root = crate::path(&source.data).join("box/fake-backends");
    let resources = [
        "prepared",
        "config/agent.json",
        "config/github.json",
        "config/oauth.json",
        "config/llm.json",
        "data/worker.secret",
    ]
    .map(|name| (name, fs::read(root.join(name)).expect("retained fixture resource")));
    let data = root.join("data/retained-fixture");
    write(&data, b"keep this data", false).expect("retained data");
    drop(first);
    fs::remove_dir_all(scratch.0.join("bundle-loopback")).expect("old bundle unavailable");
    let replacement = provision_bundle(&scratch.0.join("replacement"));
    let second = start(&source, Some(&replacement)).expect("replace the retained selection");
    edited["application"]["clients"]["bundle"] = replacement.to_str().expect("replacement path").into();
    assert_eq!(read_json(&document), edited);
    assert_eq!(second.bytes.as_ref(), fs::read(&document).expect("published configuration"));
    let mut held_bytes = Vec::new();
    old_file.read_to_end(&mut held_bytes).expect("read old inode after publication");
    assert_eq!(held_bytes, old_bytes, "the old inode was never truncated");
    assert_ne!(fs::metadata(&document).expect("new inode").ino(), old_inode);
    assert_eq!(fs::metadata(&document).expect("private replacement").mode() & 0o777, 0o600);
    assert!(!document.with_file_name("temper.json.bundle-staging").exists());
    for (name, content) in resources {
        assert_eq!(fs::read(root.join(name)).expect("unchanged retained resource"), content, "{name}");
    }
    assert_eq!(fs::read(data).expect("retained data"), b"keep this data");
    let retained = second.bytes.clone();
    drop(second);
    let third = start(&source, None).expect("replacement remains selected without a flag");
    assert_eq!(third.bytes, retained);
}

#[test]
fn repeating_the_same_bundle_preserves_document_bytes_and_inode() {
    let scratch = Scratch::new("reuse-same-bundle");
    let (source, first) = prepared_source(&scratch);
    let document = first.directories.document();
    let retained = [b"\n ".as_slice(), first.bytes.as_ref(), b" \n"].concat();
    write(&document, &retained, true).expect("retain custom formatting");
    let inode = fs::metadata(&document).expect("retained inode").ino();
    drop(first);
    let same = scratch.0.join("bundle-loopback");
    let second = start(&source, Some(&same)).expect("same bundle is already selected");
    assert_eq!(second.bytes.as_ref(), retained);
    assert_eq!(fs::metadata(&document).expect("unchanged inode").ino(), inode);
}

fn refused_replacement_preserves_document(source: &Directories, document: &Path, replacement: &Path, member: &str) {
    let before = fs::read(document).expect("retained bytes before refusal");
    let inode = fs::metadata(document).expect("retained inode").ino();
    let error = start(source, Some(replacement)).err().expect("replacement refused");
    assert!(error.contains(member), "{error}");
    assert_eq!(fs::read(document).expect("retained bytes after refusal"), before);
    assert_eq!(fs::metadata(document).expect("unchanged inode").ino(), inode);
    assert!(!document.with_file_name("temper.json.bundle-staging").exists());
}

#[test]
fn invalid_replacement_bundle_preserves_retained_document() {
    let scratch = Scratch::new("reuse-invalid-bundle");
    let (source, first) = prepared_source(&scratch);
    let _edited = edit_retained(&first);
    let document = first.directories.document();
    drop(first);
    refused_replacement_preserves_document(&source, &document, &scratch.0.join("absent"), "index.html");
}

#[test]
fn invalid_retained_configuration_refuses_a_valid_replacement_without_mutation() {
    let scratch = Scratch::new("reuse-invalid-config");
    let (source, first) = prepared_source(&scratch);
    let document = first.directories.document();
    let mut edited = read_json(&document);
    edited["workers"][0]["agent"]["arguments"] = serde_json::json!(["internal", "agent", "/outside/agent.json"]);
    store_json(&document, &edited);
    drop(first);
    let replacement = provision_bundle(&scratch.0.join("valid-replacement"));
    refused_replacement_preserves_document(&source, &document, &replacement, "workers/0/agent/arguments");
}

#[test]
fn staging_file_collision_leaves_the_retained_document_untouched() {
    let scratch = Scratch::new("reuse-staging-collision");
    let (source, first) = prepared_source(&scratch);
    let document = first.directories.document();
    let before = fs::read(&document).expect("retained bytes");
    let staging = document.with_file_name("temper.json.bundle-staging");
    write(&staging, b"existing staging file", false).expect("staging collision fixture");
    drop(first);
    let replacement = provision_bundle(&scratch.0.join("replacement"));
    let error = start(&source, Some(&replacement)).err().expect("staging collision refused");
    assert!(error.contains("temper.json.bundle-staging"), "{error}");
    assert_eq!(fs::read(&document).expect("retained document unchanged"), before);
    assert_eq!(fs::read(staging).expect("staging file unchanged"), b"existing staging file");
}

#[test]
fn reuse_refuses_a_missing_effective_bundle_and_preserves_edits() {
    let scratch = Scratch::new("retained-missing-bundle");
    let source = provisioned_source(&scratch);
    let first = start(&source, None).expect("fresh sandbox");
    let document = first.directories.document();
    let mut edited = read_json(&document);
    let absent = scratch.0.join("absent-retained-bundle");
    edited["application"]["clients"]["bundle"] = absent.to_str().expect("absent bundle path").into();
    store_json(&document, &edited);
    drop(first);
    let error = start(&source, None).err().expect("missing retained bundle refused");
    assert!(error.contains("absent-retained-bundle"), "{error}");
    assert_eq!(read_json(&document), edited);
}

#[test]
fn current_init_import_ignores_charter_files_and_rebases_paths() {
    let scratch = Scratch::new("import");
    let source = provisioned_source(&scratch);
    let mut temper = read_json(&source.document());
    // Equivalent absolute resource spelling is admitted without following source D's resources.
    temper["workers"][0]["secret"] = crate::path(&source.data).join("worker.secret").to_str().expect("path").into();
    store_json(&source.document(), &temper);
    fs::create_dir_all(crate::path(&source.config).join("charters")).expect("stray public directory");
    write(&crate::path(&source.config).join("charters/chat.txt"), b"A custom public charter.\n", false)
        .expect("public charter");
    let before = ["temper.json", "agent.json", "charters/chat.txt"].map(|name| {
        let path = crate::path(&source.config).join(name);
        (name, fs::read(&path).expect("source public fixture"), fs::metadata(path).expect("mode").mode())
    });
    let source_secret = fs::read(crate::path(&source.data).join("worker.secret")).expect("generated fixture secret");
    let prepared = start(&source, None).expect("current init-shaped import");
    assert_ne!(
        fs::read(crate::path(&prepared.directories.data).join("worker.secret")).expect("fresh test secret"),
        source_secret
    );
    assert!(!crate::path(&prepared.directories.config).join("charters").exists());
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
    let reused = start(&source, None).expect("stray charter edits do not change the imported defaults");
    assert!(!crate::path(&reused.directories.config).join("charters").exists());
}

#[test]
fn unsupported_imports_refuse_with_the_public_member_named() {
    let scratch = Scratch::new("unsupported");
    let source = provisioned_source(&scratch);
    let mut temper = read_json(&source.document());
    temper["workers"][0]["agent"]["arguments"] =
        serde_json::json!(["internal", "agent", "/elsewhere/agent.json", "extra"]);
    store_json(&source.document(), &temper);
    let error = start(&source, None).err().expect("custom argv refused");
    assert!(error.contains("workers/0/agent/arguments"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn effective_commands_reopen_the_retained_directories() {
    let scratch = Scratch::new("commands");
    let source = provisioned_source(&scratch);
    let prepared = start(&source, None).expect("prepare effective document");
    let deployment =
        crate::setup::check(&prepared.bytes, &prepared.directories.jig()).expect("checked effective setup");
    let steps = temper_fake::steps(
        &deployment.checked.setup,
        deployment.demo.as_ref(),
        &prepared.directories.flags(),
        Box::new([]),
        true,
        backends::steps(&prepared.directories.config, &prepared.peers),
    );
    let prefix = backends::steps(&prepared.directories.config, &prepared.peers);
    assert_eq!(&steps.always[..prefix.len()], prefix.as_ref());
    for step in steps.always[prefix.len()..]
        .iter()
        .chain(steps.asked.iter().flat_map(|group| group.steps.iter().map(|(_, step)| step)))
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
fn oversized_explicit_bundle_refuses_before_creating_data_or_cache() {
    let scratch = Scratch::new("explicit-bundle-bytes");
    let source = scratch.dirs();
    let supplied = provision_bundle(&scratch.0.join("supplied"));
    let limits = temper_service::limits::profile(temper_service::limits::Profile::Small).clients;
    let page = OpenOptions::new().write(true).open(supplied.join("index.html")).expect("fixture page");
    page.set_len(u64::from(limits.bundle_bytes) + 1).expect("one byte past the profile's bundle bound");
    let error = start(&source, Some(&supplied)).err().expect("oversized explicit bundle refused");
    assert!(error.contains("clients.bundle_bytes"), "{error}");
    assert!(!crate::path(&source.config).exists());
    assert!(!crate::path(&source.data).exists());
    assert!(!crate::path(&source.cache).exists());
}

#[test]
fn unsupported_provisioned_file_refuses_before_sandbox_creation() {
    let scratch = Scratch::new("bundle-type");
    let source = provisioned_source(&scratch);
    write(&scratch.0.join("bundle-loopback/0000000000000001/unsupported.exe"), b"unsupported", false)
        .expect("invalid bundle fixture");
    let error = start(&source, None).err().expect("unsupported bundle file refused");
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
    let error = start(&source, None).err().expect("linked bundle page refused");
    assert!(error.contains("index.html") && error.contains("links"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn source_public_asset_links_are_refused_before_reading_them() {
    let scratch = Scratch::new("source-links");
    let source = provisioned_source(&scratch);
    let document = crate::path(&source.config).join("agent.json");
    fs::remove_file(&document).expect("remove public fixture");
    // The target is absent, so a refusal about links proves it was never opened or copied.
    std::os::unix::fs::symlink(scratch.0.join("unread-target"), &document).expect("linked public fixture");
    let error = start(&source, None).err().expect("source public link refused");
    assert!(error.contains("links"), "{error}");
    assert!(!crate::path(&source.data).join("box/fake-backends").exists());
}

#[test]
fn import_preserves_disabled_git_and_adds_the_fake_program() {
    let scratch = Scratch::new("disabled-programs");
    let source = provisioned_source(&scratch);
    let prepared = start(&source, None).expect("import optional choices independently of today's PATH");
    let temper = read_json(&prepared.directories.document());
    let agent = read_json(&crate::path(&prepared.directories.config).join("agent.json"));
    assert!(temper["application"]["workers"][0]["git"].is_null());
    assert_eq!(agent["programs"]["claude_code"]["path"], scratch.0.join("fake-claude").to_str().expect("path"));
}

#[test]
fn four_forges_pass_ordinary_admission_with_a_directoryless_managed_entry() {
    let scratch = Scratch::new("four-forge-admission-r2");
    let (_source, first) = prepared_source(&scratch);
    let checked = crate::setup::check(&first.bytes, &first.directories.jig()).expect("ordinary admission");
    assert_eq!(checked.named.forges.len(), 4);
    assert_eq!(checked.named.identities.len(), 4);
    let entry = checked.checked.setup.entry(4).expect("managed four");
    assert!(entry.directory.is_empty());
    assert_eq!(temper_forge_setup::given::values(entry, None, &checked.checked.limits).admin_name.as_ref(), b"admin");
    validate(&first.directories, &first.bytes, &scratch.0.join("fake-claude")).expect("sandbox validation");
    let (memory, operations) =
        crate::envelope_in(&first.directories, &first.bytes).expect("ordinary computed startup sizing");
    assert!(memory > 0 && operations > 0);
    eprintln!("four-forge ordinary sizing: {memory} bytes, {operations} operations");
}

#[test]
fn missing_required_retained_documents_refuse_without_writes() {
    for (index, name) in REQUIRED.iter().enumerate() {
        let scratch = Scratch::new(&format!("required-{index}"));
        let (source, first) = prepared_source(&scratch);
        let config = crate::path(&first.directories.config).to_path_buf();
        let marker = fs::read(config.parent().expect("sandbox").join("prepared")).expect("marker");
        fs::remove_file(config.join(name)).expect("remove required fixture document");
        let before = fs::read_dir(&config).expect("config listing").count();
        drop(first);
        let error = start(&source, None).err().expect("missing required document refused");
        assert!(error.contains(name) && error.contains("reset"), "{error}");
        assert_eq!(fs::read_dir(&config).expect("unchanged listing").count(), before);
        assert!(fs::read(config.parent().expect("sandbox").join("prepared")).expect("marker") == marker);
        assert!(!config.join(name).exists());
    }
}

#[test]
fn foreign_retained_adapter_refuses_without_resetting_or_rewriting_it() {
    let scratch = Scratch::new("foreign-adapter");
    let (source, first) = prepared_source(&scratch);
    let path = crate::path(&first.directories.config).join("agent.json");
    let mut agent = read_json(&path);
    agent["programs"]["claude_code"]["path"] = "/foreign/claude".into();
    store_json(&path, &agent);
    // Moving temper makes both retained executable paths foreign; the adapter reset must win.
    let mut deployment = read_json(&first.directories.document());
    deployment["workers"][0]["agent"]["program"] = "/foreign/temper".into();
    store_json(&first.directories.document(), &deployment);
    let before = fs::read(&path).expect("fixture bytes");
    drop(first);
    let error = start(&source, None).err().expect("foreign adapter refused");
    assert!(error.contains("agent.json/programs/claude_code/path") && error.contains("reset"), "{error}");
    assert!(fs::read(path).expect("unchanged") == before);
}

#[test]
fn exact_sibling_and_provisioning_refusals_use_explicit_paths() {
    let expected = sibling(Path::new("/fixture/temper")).expect("absolute sibling");
    assert_eq!(expected, Path::new("/fixture/fake-claude"));
    adapter_identity(b"/fixture/fake-claude", &expected).expect("correct identity");
    assert!(adapter_identity(b"/foreign/claude", &expected).expect_err("foreign").contains("reset"));
    let scratch = Scratch::new("program-provisioning");
    let path = scratch.0.join("fake-claude");
    for exists in [false, true] {
        if exists {
            write(&path, b"not executable", false).expect("nonexecutable fixture");
        }
        let error = provisioned_program(&path).expect_err("provision outside startup");
        assert!(
            error.contains(path.to_str().expect("path")) && error.contains("cargo build -p temper-fake-claude"),
            "{error}"
        );
        assert!(!error.contains("reset"), "{error}");
    }
}

#[test]
fn source_adapter_is_a_bounded_fact_without_path_discovery() {
    let scratch = Scratch::new("source-program-fact");
    let source = provisioned_source(&scratch);
    let path = crate::path(&source.config).join("agent.json");
    let mut agent = read_json(&path);
    agent["programs"] = serde_json::json!({"claude_code":{"path":"/absent/source/claude","directory":crate::path(&source.data).join("programs")}});
    store_json(&path, &agent);
    let first = start(&source, None).expect("source adapter does not need to exist or match PATH");
    let effective = read_json(&crate::path(&first.directories.config).join("agent.json"));
    assert_ne!(effective["programs"]["claude_code"]["path"], agent["programs"]["claude_code"]["path"]);
}

#[test]
fn fake_backends_echo_takes_the_variable_and_refuses_the_flag() {
    use temper_box::host::Echo;
    let args = ["box", "--fake-backends", "--config", "/c", "--data", "/d", "--cache", "/k"]
        .map(|word| Box::from(word.as_bytes()));
    for (value, expected) in [("steps", Echo::Steps), ("none", Echo::None)] {
        let environment = [(Box::from(b"TEMPER_ECHO".as_slice()), Box::from(value.as_bytes()))];
        let crate::Command::BoxFakeBackends { echo, .. } =
            crate::configure_with(&args, &environment, b"/").expect("valid fake-backend echo")
        else {
            panic!("fake box")
        };
        assert_eq!(echo, expected);
    }
    let crate::Command::BoxFakeBackends { echo, .. } =
        crate::configure_with(&args, &[], b"/").expect("valid fake-backend echo")
    else {
        panic!("fake box")
    };
    assert_eq!(echo, Echo::None);
    let mut flagged = args.to_vec();
    flagged.extend([Box::from(b"--echo".as_slice()), Box::from(b"steps".as_slice())]);
    assert!(
        crate::configure_with(&flagged, &[], b"/")
            .expect_err("invalid fake-backend echo")
            .contains("invalid box arguments")
    );
    let environment = [(Box::from(b"TEMPER_ECHO".as_slice()), Box::from(b"bad".as_slice()))];
    assert!(
        crate::configure_with(&args, &environment, b"/")
            .expect_err("invalid fake-backend echo")
            .contains("TEMPER_ECHO: expected none or steps")
    );
}

#[test]
fn own_app_key_is_the_seeded_apps_private_pem() {
    let scratch = Scratch::new("own-app-key");
    let (source, prepared) = prepared_source(&scratch);
    let path = crate::path(&prepared.directories.data).join("github-app.pem");
    let key = fs::read(&path).expect("own-App key left for the person");
    let forge = temper_fake::forge::Document::decode(
        &fs::read(crate::path(&prepared.directories.config).join("github.json")).expect("forge document"),
    )
    .expect("generated forge");
    assert_eq!(key, forge.github_app_key().expect("seeded App").as_ref());
    assert!(key.starts_with(b"-----BEGIN RSA PRIVATE KEY-----"));
    assert_eq!(fs::metadata(&path).expect("key metadata").mode() & 0o777, 0o600);
    drop(prepared);
    let _again = start(&source, None).expect("reuse keeps the key");
    assert_eq!(fs::read(&path).expect("retained key"), key);
}
