//! Retained isolated startup for `box --fake-backends` (configuration.md, 12.1).
mod import;
mod json;
use crate::directories::{Directories, make_private, private_root};
use import::Source;
use json::Json;
use skein_io::kernel::{Exit, Fd};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpListener},
    num::NonZeroU16,
    os::fd::IntoRawFd,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
};
use temper_fake::backends::{self, Paths, Peers};
use temper_forge_setup_shell::process as locks;

const REQUIRED: &[&str] = &["temper.json", "agent.json", "github.json", "oauth.json", "llm.json", "forgejo.json"];

struct Listeners(Vec<(Fd, NonZeroU16)>);
impl Drop for Listeners {
    fn drop(&mut self) {
        crate::close(std::mem::take(&mut self.0).into_iter().map(|(fd, _)| fd).collect());
    }
}
struct Prepared {
    directories: Directories,
    bytes: Box<[u8]>,
    listeners: Listeners,
    peers: Peers,
    // Held through drive; closing the descriptor releases the OS lock even after a crash.
    _lock: locks::Lock,
}
pub(crate) fn configure(rest: &[std::ffi::OsString], working: &[u8]) -> Result<Option<PathBuf>, String> {
    let mut fake = false;
    let mut bundle = None;
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match word.as_bytes() {
            b"--fake-backends" if !fake => fake = true,
            b"--bundle" if bundle.is_none() => {
                let value = words.next().ok_or("--bundle DIR: missing directory")?;
                let value = value.as_bytes();
                if value.is_empty() || value.contains(&0) || value.starts_with(b"--") {
                    return Err("--bundle DIR: expected a nonempty path without NUL".into());
                }
                let path = crate::path(working).join(crate::path(value));
                let name = path.as_os_str().as_bytes();
                if !path.is_absolute() || name.contains(&0) || name.len() > 4096 {
                    return Err("--bundle DIR: expected an absolute path of at most 4096 bytes without NUL".into());
                }
                bundle = Some(path);
            }
            _ => return Err("invalid box arguments: expected --fake-backends [--bundle DIR]".into()),
        }
    }
    if !fake {
        return Err("--bundle DIR requires --fake-backends".into());
    }
    Ok(bundle)
}
pub(crate) fn run(source: &Directories, bundle: Option<&Path>) -> Result<Exit, String> {
    let adapter = sibling(&std::env::current_exe().map_err(|why| why.to_string())?)?;
    let Prepared { directories, bytes, mut listeners, peers, _lock: lock } = prepare(source, bundle, &adapter)?;
    let first = backends::steps(&directories.config, &peers);
    let result = crate::box_command::run_configuration(&directories, &bytes, std::mem::take(&mut listeners.0), first);
    drop(lock);
    result
}
fn bytes(path: &Path) -> Box<[u8]> {
    path.as_os_str().as_bytes().into()
}
fn public_path(path: &Path) -> Result<(), String> {
    // Public imports must not turn a link into an accidental source-secret copy.
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&prefix).map_err(|why| format!("{}: {why}", prefix.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{}: public documents must not be reached through links", prefix.display()));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|why| format!("{}: {why}", path.display()))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(format!("{}: expected a regular public document with one link", path.display()));
    }
    Ok(())
}
fn public(path: &Path) -> Result<Box<[u8]>, String> {
    public_path(path)?;
    skein_shell::read_file(path, crate::CONFIG_BYTES)
        .map_err(|why| format!("{}: {}", path.display(), crate::unread(why)))
}
fn private_document(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|why| format!("{}: {why}", path.display()))?;
    if metadata.uid() != skein_shell::effective_user() || metadata.mode() & 0o077 != 0 {
        return Err(format!("{}: retained documents must be private files of this user's", path.display()));
    }
    Ok(())
}
fn resolved(root: &Path, name: &[u8]) -> Result<PathBuf, String> {
    if name.contains(&0) || name.len() > 4096 {
        return Err("public path: NUL or past 4096 bytes".into());
    }
    let path = root.join(crate::path(name));
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => return Err(format!("{}: parent components are unsupported", path.display())),
            Component::CurDir => {}
            Component::RootDir | Component::Normal(_) => clean.push(part.as_os_str()),
            Component::Prefix(_) => return Err("public path: unsupported prefix".into()),
        }
    }
    if clean.as_os_str().len() > 4096 {
        return Err("effective path: past 4096 bytes".into());
    }
    Ok(clean)
}
fn write(path: &Path, content: &[u8], replace: bool) -> Result<(), String> {
    if replace {
        let metadata = fs::symlink_metadata(path).map_err(|why| format!("{}: {why}", path.display()))?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != skein_shell::effective_user()
            || metadata.nlink() != 1
        {
            return Err(format!("{}: expected this user's regular file", path.display()));
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).mode(0o600);
    if replace {
        options.truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(path).map_err(|why| format!("{}: {why}", path.display()))?;
    file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|why| why.to_string())?;
    file.write_all(content).map_err(|why| format!("{}: {why}", path.display()))?;
    file.sync_all().map_err(|why| format!("{}: {why}", path.display()))
}
fn replace_bundle(
    dirs: &Directories,
    content: Box<[u8]>,
    bundle: Option<&Path>,
    adapter: &Path,
) -> Result<Box<[u8]>, String> {
    let mut updated = content.clone();
    if let Some(bundle) = bundle {
        let mut document = Json::decode(&content)?;
        let selection = Json::String(bytes(bundle));
        if document.at("application/clients/bundle")? != &selection {
            document.set("application/clients/bundle", selection)?;
            updated = document.encode()?;
        }
    }
    // Check the entire candidate, including its bundle, before touching retained bytes.
    validate(dirs, &updated, adapter)?;
    if updated == content {
        return Ok(content);
    }
    let path = dirs.document();
    let staging = crate::path(&dirs.config).join("temper.json.bundle-staging");
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&staging).map_err(|why| {
        format!(
            "{}: {why}; an existing staging file must be removed explicitly before another bundle replacement",
            staging.display()
        )
    })?;
    let result = (|| {
        file.write_all(&updated)?;
        file.sync_all()?;
        fs::rename(&staging, &path)
    })();
    if let Err(why) = result {
        let _removed = fs::remove_file(&staging);
        return Err(format!("{}: bundle replacement: {why}", path.display()));
    }
    Ok(updated)
}
fn lock(root: &Path) -> Result<locks::Lock, String> {
    let path = root.join("lock");
    let user = skein_shell::effective_user();
    match fs::symlink_metadata(&path) {
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.uid() != user
                || metadata.mode() & 0o077 != 0 =>
        {
            return Err(reset(root, "lock must be this user's private regular file"));
        }
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("{}: {why}", path.display())),
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|why| format!("{}: {why}", path.display()))?;
    let named = fs::symlink_metadata(&path).map_err(|why| why.to_string())?;
    let opened = file.metadata().map_err(|why| why.to_string())?;
    if named.file_type().is_symlink() || named.ino() != opened.ino() || named.dev() != opened.dev() {
        return Err(format!("{}: lock changed while opening", path.display()));
    }
    locks::lock_file(file)
        .map_err(|why| format!("{}: another fake-backend box is using this sandbox: {why:?}", path.display()))
}
fn reset(root: &Path, reason: &str) -> String {
    format!("{}: {reason}; remove this sandbox directory explicitly to reset it", root.display())
}
fn directories(source: &Directories) -> Result<(PathBuf, Directories), String> {
    let root = resolved(crate::path(&source.data), b"box/fake-backends")?;
    let config = bytes(&root.join("config"));
    let data = bytes(&root.join("data"));
    // Joined agent/program paths must fit the same bound as ordinary init.
    for path in [&config, &data] {
        if path.len().checked_add(32).is_none_or(|n| n > 4096) {
            return Err("fake-backends: effective directory past its path bound".into());
        }
    }
    Ok((root, Directories { config, data, cache: source.cache.clone(), origins: [skein_shell::Origin::Flag; 3] }))
}
fn reserve(excluded: &[u16]) -> Result<TcpListener, String> {
    for _ in 0..32 {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|why| format!("fake peer reservation: {why}"))?;
        if !excluded.contains(&listener.local_addr().map_err(|why| why.to_string())?.port()) {
            return Ok(listener);
        }
    }
    Err("fake peer reservation: could not choose a port distinct from the deployment and callback".into())
}
fn addr(listener: &TcpListener) -> Result<SocketAddr, String> {
    listener.local_addr().map_err(|why| why.to_string())
}
fn contained(root: &[u8], path: &[u8]) -> bool {
    crate::path(path)
        .strip_prefix(crate::path(root))
        .is_ok_and(|rest| !rest.as_os_str().is_empty() && rest.components().all(|p| matches!(p, Component::Normal(_))))
}
fn peer_document(dirs: &Directories, name: &str, bound: u32) -> Result<Box<[u8]>, String> {
    let path = crate::path(&dirs.config).join(name);
    public_path(&path)?;
    private_document(&path)?;
    skein_shell::read_private_file(&path, bound, skein_shell::effective_user())
        .map(|bytes| bytes.expose().into())
        .map_err(|why| format!("{}: {}", path.display(), crate::unread(why)))
}
fn checked_peers(dirs: &Directories, bytes: &[u8]) -> Result<Peers, String> {
    let refuse = |name: &str, why: temper_fake::Refusal| {
        format!("{}: {}", crate::path(&dirs.config).join(name).display(), String::from_utf8_lossy(&why.line()))
    };
    let github =
        temper_fake::forge::Document::decode(&peer_document(dirs, "github.json", temper_fake::DOCUMENT_BYTES)?)
            .map_err(|why| refuse("github.json", why))?;
    let oauth =
        temper_fake::oauth::Document::decode(&peer_document(dirs, "oauth.json", temper_fake::oauth::DOCUMENT_BYTES)?)
            .map_err(|why| refuse("oauth.json", why))?;
    let llm = temper_fake::llm::Document::decode(&peer_document(dirs, "llm.json", temper_fake::DOCUMENT_BYTES)?)
        .map_err(|why| refuse("llm.json", why))?;
    let forgejo =
        temper_fake::forge::Document::decode(&peer_document(dirs, "forgejo.json", temper_fake::DOCUMENT_BYTES)?)
            .map_err(|why| refuse("forgejo.json", why))?;
    let deployment = crate::setup::check(bytes, &dirs.jig())?;
    let mut serve = vec![crate::address(&deployment.named.clients)?];
    if let Some(webhooks) = &deployment.named.webhooks {
        serve.push(crate::address(&webhooks.listener)?);
    }
    let ordinary = temper_fake::steps(
        &deployment.checked.setup,
        deployment.demo.as_ref(),
        &dirs.flags(),
        serve.into_boxed_slice(),
        true,
        Box::new([]),
    );
    let mut ports = vec![1455];
    for step in
        ordinary.always.iter().chain(ordinary.asked.iter().flat_map(|group| group.steps.iter().map(|(_, step)| step)))
    {
        for listen in step.listeners() {
            ports.push(listen.port());
        }
        if let Some(ready) = step.ready() {
            ports.push(ready.port());
        }
    }
    let peers = Peers { github: github.listen(), oauth: oauth.listen(), llm: llm.listen(), forgejo: forgejo.listen() };
    for (name, listen) in [
        ("github.json", peers.github),
        ("oauth.json", peers.oauth),
        ("llm.json", peers.llm),
        ("forgejo.json", peers.forgejo),
    ] {
        if !listen.ip().is_loopback() || listen.port() == 0 || ports.contains(&listen.port()) {
            return Err(format!(
                "{}: peer listen must be nonzero loopback with a distinct, unreserved port",
                crate::path(&dirs.config).join(name).display()
            ));
        }
        ports.push(listen.port());
    }
    for (name, output) in
        [("github.json", github.admin_token()), ("forgejo.json", forgejo.admin_token()), ("llm.json", llm.key())]
            .into_iter()
            .chain(llm.added_keys().iter().map(|path| ("llm.json", path.as_ref())))
            .chain(llm.root().into_iter().map(|path| ("llm.json", path)))
    {
        if !contained(&dirs.data, output) {
            return Err(format!(
                "{}: output {} must stay under effective data",
                crate::path(&dirs.config).join(name).display(),
                crate::path(output).display()
            ));
        }
    }
    Ok(peers)
}
fn validate(dirs: &Directories, bytes: &[u8], expected: &Path) -> Result<(), String> {
    let deployment = crate::setup::check(bytes, &dirs.jig())?;
    let named = &deployment.named;
    for file in named.files.iter().chain(&named.asked_files) {
        if !(contained(&dirs.config, &file.path) && !file.private || contained(&dirs.data, &file.path)) {
            return Err(format!(
                "{}: effective resource must stay under its sandbox config/data",
                crate::file_path(file.at.clone())
            ));
        }
    }
    if !contained(&dirs.data, &named.deployment)
        || named.worker.as_ref().is_none_or(|worker| {
            !contained(&dirs.data, &worker.root)
                || worker.agent.as_ref() != crate::path(&dirs.config).join("agent.json").as_os_str().as_bytes()
        })
    {
        return Err("effective deployment, worker root and agent configuration must stay in the sandbox".into());
    }
    for entry in &deployment.checked.setup.entries {
        let directoryless =
            matches!(entry.mode, temper_forge_setup::setup::Mode::Managed { .. }) && entry.directory.is_empty();
        if !directoryless && !contained(&dirs.data, &entry.directory) {
            return Err("application.setup.directory: must stay under effective data".into());
        }
    }
    let tree = Json::decode(bytes)?;
    let configuration = tree.string("workers/0/agent/configuration")?;
    let argv = tree.at("workers/0/agent/arguments")?;
    let wanted = Json::Array(vec![Json::String(b"agent".as_slice().into()), Json::String(configuration.into())]);
    argv.same(&wanted, "workers/0/agent/arguments")?;
    let agent_document = public(&crate::path(&dirs.config).join("agent.json"))?;
    let agent_tree = Json::decode(&agent_document)?;
    let adapter = agent_tree.string("programs/claude_code/path")
        .map_err(|_| "agent.json/programs/claude_code/path: unsupported adapter; remove this sandbox directory explicitly to reset it".to_string())?;
    adapter_identity(adapter, expected)?;
    provisioned_program(expected)?;
    if tree.string("workers/0/agent/program")?
        != std::env::current_exe().map_err(|why| why.to_string())?.as_os_str().as_bytes()
    {
        return Err("workers/0/agent/program: expected this temper binary".into());
    }
    for file in &named.files {
        if !file.private {
            let asset = public(crate::path(&file.path))?;
            if asset.len() > usize::try_from(file.max).expect("file bound fits") {
                return Err(format!("{}: public asset past its byte bound", crate::file_path(file.at.clone())));
            }
        }
    }
    let agent = crate::agent::preflight(named)?;
    for program in agent.programs {
        if !contained(&dirs.data, &program.directory) {
            return Err("agent.programs.directory: must stay under effective data".into());
        }
    }
    match &named.bundle {
        Some(bundle) => {
            let _checked = provisioned_bundle(Some(crate::path(bundle)), named.bundle_limits)?;
        }
        None => return Err("application.clients.bundle: effective document must name its loopback bundle".into()),
    }
    Ok(())
}
fn provisioned_bundle(path: Option<&Path>, limits: temper_service::limits::Clients) -> Result<PathBuf, String> {
    let instructions =
        "run scripts/bundle.sh --loopback DIR && temper box --fake-backends --bundle DIR (directory flags as usual)";
    let path =
        path.ok_or_else(|| format!("application.clients.bundle: no provisioned loopback bundle; {instructions}"))?;
    let page = path.join("index.html");
    public_path(&page)
        .map_err(|why| format!("application.clients.bundle: {}: {why}; {instructions}", page.display()))?;
    let _checked =
        crate::bundle::read(path.as_os_str().as_bytes(), limits).map_err(|why| format!("{why}; {instructions}"))?;
    Ok(path.to_path_buf())
}
fn print(dirs: &Directories, peers: &Peers, binary: &Path) {
    use std::fmt::Write;
    eprintln!("box: effective configuration {}", crate::path(&dirs.config).display());
    eprintln!("box: effective data {}", crate::path(&dirs.data).display());
    eprintln!("box: effective cache {}", crate::path(&dirs.cache).display());
    for (name, address) in
        [("github", peers.github), ("issuer", peers.oauth), ("model", peers.llm), ("forgejo", peers.forgejo)]
    {
        eprintln!("box: fake {name} http://{address}");
    }
    // Retained peer documents may select another key file under effective data.
    let model = temper_fake::llm::Document::decode(
        &peer_document(dirs, "llm.json", temper_fake::DOCUMENT_BYTES).expect("validated model document"),
    )
    .expect("validated model document");
    eprintln!("box: model key {}", crate::path(model.key()).display());
    eprintln!(
        "box: Forgejo elsewhere admin token file {}",
        crate::path(&dirs.data).join("forgejo-admin.token").display()
    );
    let mut command = format!("'{}' program-login", binary.display().to_string().replace('\'', "'\\''"));
    for (flag, path) in [("config", &dirs.config), ("data", &dirs.data), ("cache", &dirs.cache)] {
        write!(command, " --{flag} '{}'", crate::path(path).display().to_string().replace('\'', "'\\''"))
            .expect("string");
    }
    command.push_str(" source");
    eprintln!("box: program login: {command}");
}
fn sibling(binary: &Path) -> Result<PathBuf, String> {
    if !binary.is_absolute() {
        return Err("temper binary: expected an absolute path".into());
    }
    Ok(binary.parent().ok_or("temper binary: missing parent")?.join("fake-claude"))
}
fn adapter_identity(given: &[u8], expected: &Path) -> Result<(), String> {
    if given != expected.as_os_str().as_bytes() {
        return Err("agent.json/programs/claude_code/path: expected this temper's fake-claude sibling; remove this sandbox directory explicitly to reset it".into());
    }
    Ok(())
}
fn provisioned_program(path: &Path) -> Result<(), String> {
    let executable = fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.mode() & 0o111 != 0);
    if !executable {
        return Err(format!(
            "{}: missing or nonexecutable adapter; provision it with cargo build -p temper-fake-claude",
            path.display()
        ));
    }
    Ok(())
}
// The adapter is this temper's fake-claude sibling; it is a parameter so that tests can provision one.
#[expect(
    clippy::too_many_lines,
    reason = "startup owns its lock and listeners through validation and publishes the prepared marker last"
)]
fn prepare(source_dirs: &Directories, bundle: Option<&Path>, adapter: &Path) -> Result<Prepared, String> {
    let (root, effective) = directories(source_dirs)?;
    let source_config = resolved(Path::new("/"), &source_dirs.config)?;
    if source_config.starts_with(&root) {
        return Err(
            "source configuration overlaps the fake-backend sandbox; choose configuration outside D/box/fake-backends"
                .into(),
        );
    }
    let binary = std::env::current_exe().map_err(|why| why.to_string())?;
    let marker = root.join("prepared");
    let exists = match fs::symlink_metadata(&root) {
        Ok(_) => true,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => false,
        Err(why) => return Err(format!("{}: {why}", root.display())),
    };
    let source = Source::read(source_dirs).map_err(|why| if exists { reset(&root, &why) } else { why })?;
    // Reuse checks the retained effective bundle, which may differ from the source's directory.
    let provisioned = if exists {
        None
    } else {
        let limits = if let Some((document, _)) = &source.documents {
            crate::setup::check(&document.encode()?, &source_dirs.jig())?.named.bundle_limits
        } else {
            temper_service::limits::profile(temper_service::limits::Profile::Small).clients
        };
        Some(provisioned_bundle(bundle.or(source.bundle.as_deref()), limits)?)
    };
    if !exists {
        provisioned_program(adapter)?;
    }
    let user = skein_shell::effective_user();
    make_private(crate::path(&source_dirs.data), user)?;
    make_private(root.parent().expect("box parent"), user)?;
    make_private(&root, user).map_err(|why| reset(&root, &why))?;
    let lock = lock(&root)?;
    if exists {
        let prepared = public(&marker)
            .map_err(|_| reset(&root, "partial preparation: prepared marker is absent or unreadable"))?;
        if prepared != source.marker {
            return Err(reset(&root, "incompatible source directories or changed public source documents"));
        }
        private_root(crate::path(&effective.config), user)
            .map_err(|why| reset(&root, &format!("unsafe effective config: {why}")))?;
        private_root(crate::path(&effective.data), user)
            .map_err(|why| reset(&root, &format!("unsafe effective data: {why}")))?;
        private_document(&marker)?;
        for name in REQUIRED {
            let path = crate::path(&effective.config).join(name);
            if !path.exists() {
                return Err(reset(&root, &format!("missing required {name}")));
            }
            public_path(&path).and_then(|()| private_document(&path)).map_err(|why| reset(&root, &why))?;
        }
        let content = public(&effective.document())?;
        let peers = checked_peers(&effective, &content)?;
        let bytes = replace_bundle(&effective, content, bundle, adapter)?;
        print(&effective, &peers, &binary);
        return Ok(Prepared { directories: effective, bytes, listeners: Listeners(Vec::new()), peers, _lock: lock });
    }
    let written = crate::init::write_with_program(
        &effective,
        &skein_shell::environment(),
        &binary,
        [8367, 8368, 8369, 8370, 8371],
        Some(adapter),
    )?;
    let mut held = Listeners(written.listeners);
    // Init deliberately writes public documents 0644; every retained sandbox document is private.
    for path in written.paths {
        if fs::symlink_metadata(&path).map_err(|why| why.to_string())?.is_file() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|why| why.to_string())?;
        }
    }
    if let Some((temper, agent)) = source.rebase(&effective)? {
        write(&effective.document(), &temper, true)?;
        write(&crate::path(&effective.config).join("agent.json"), &agent, true)?;
        // Imported listeners retain their source values; init's new reservations do not represent them.
        crate::close(std::mem::take(&mut held.0).into_iter().map(|(fd, _)| fd).collect());
    }
    for (name, value) in &source.charters {
        write(&crate::path(&effective.config).join(name), value, true)?;
    }
    let initial = public(&effective.document())?;
    let deployment = crate::setup::check(&initial, &effective.jig())?;
    let mut excluded = vec![1455, crate::address(&deployment.named.clients)?.port()];
    if let Some(webhooks) = &deployment.named.webhooks {
        excluded.push(crate::address(&webhooks.listener)?.port());
    }
    for entry in &deployment.checked.setup.entries {
        match &entry.mode {
            temper_forge_setup::setup::Mode::Owned(owned) => excluded.push(owned.listen.port()),
            temper_forge_setup::setup::Mode::Fake(fake) => excluded.push(fake.listen.port()),
            temper_forge_setup::setup::Mode::Managed { .. } => {}
        }
    }
    if let Some(demo) = &deployment.demo {
        excluded.push(demo.llm().listen().port());
    }
    let github = reserve(&excluded)?;
    excluded.push(addr(&github)?.port());
    let oauth = reserve(&excluded)?;
    excluded.push(addr(&oauth)?.port());
    let llm = reserve(&excluded)?;
    excluded.push(addr(&llm)?.port());
    let forgejo = reserve(&excluded)?;
    let peers = Peers { github: addr(&github)?, oauth: addr(&oauth)?, llm: addr(&llm)?, forgejo: addr(&forgejo)? };
    let bundle = provisioned.expect("new preparation checked the bundle");
    let paths = Paths {
        config: effective.config.clone(),
        data: effective.data.clone(),
        cache: effective.cache.clone(),
        bundle: bytes(&bundle),
        claude: bytes(adapter),
    };
    let documents =
        backends::rewrite(&initial, &public(&crate::path(&effective.config).join("agent.json"))?, &paths, &peers)
            .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    // Existing peer decoders check their generated documents before any marker is published.
    let _forge = temper_fake::forge::Document::decode(&documents.github)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    let _forgejo = temper_fake::forge::Document::decode(&documents.forgejo)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    let _model = temper_fake::llm::Document::decode(&documents.llm)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    for (name, document) in [
        ("temper.json", &documents.temper),
        ("agent.json", &documents.agent),
        ("github.json", &documents.github),
        ("oauth.json", &documents.oauth),
        ("llm.json", &documents.llm),
        ("forgejo.json", &documents.forgejo),
    ] {
        write(&crate::path(&effective.config).join(name), document, name == "temper.json" || name == "agent.json")?;
    }
    validate(&effective, &documents.temper, adapter)?;
    let peers = checked_peers(&effective, &documents.temper)?;
    // Last: a crash or refusal earlier is visibly partial and is never repaired by importing records.
    write(&marker, &source.marker, false)?;
    for (listener, address) in
        [(github, peers.github), (oauth, peers.oauth), (llm, peers.llm), (forgejo, peers.forgejo)]
    {
        held.0.push((Fd::new(listener.into_raw_fd()), NonZeroU16::new(address.port()).expect("checked peer port")));
    }
    print(&effective, &peers, &binary);
    Ok(Prepared { directories: effective, bytes: documents.temper, listeners: held, peers, _lock: lock })
}
#[cfg(test)]
mod tests;
