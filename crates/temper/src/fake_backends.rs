//! Retained isolated startup for `box --fake-backends` (configuration.md, 12.1).
mod import;
mod json;
use crate::directories::{Directories, make_private, private_root};
use import::Source;
use json::Json;
use skein_io::kernel::{Exit, Fd};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpListener},
    num::NonZeroU16,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
};
use temper_fake::backends::{self, Paths, Peers};

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
    // Held through drive; closing the descriptor releases the OS lock even after a crash.
    _lock: File,
}
pub(crate) fn run(source: &Directories) -> Result<Exit, String> {
    let Prepared { directories, bytes, mut listeners, _lock: lock } =
        prepare(source, temper_client_bundle::loopback::FILES)?;
    let result = crate::box_command::run_configuration(&directories, bytes, std::mem::take(&mut listeners.0));
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
fn lock(root: &Path) -> Result<File, String> {
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
    file.try_lock()
        .map_err(|why| format!("{}: another fake-backend box is using this sandbox: {why}", path.display()))?;
    Ok(file)
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
fn validate(dirs: &Directories, bytes: &[u8]) -> Result<(), String> {
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
        if !contained(&dirs.data, &entry.directory) {
            return Err("application.setup.directory: must stay under effective data".into());
        }
    }
    let tree = Json::decode(bytes)?;
    let configuration = tree.string("workers/0/agent/configuration")?;
    let argv = tree.at("workers/0/agent/arguments")?;
    let wanted = Json::Array(vec![Json::String(b"agent".as_slice().into()), Json::String(configuration.into())]);
    argv.same(&wanted, "workers/0/agent/arguments")?;
    if tree.string("workers/0/agent/program")?
        != std::env::current_exe().map_err(|why| why.to_string())?.as_os_str().as_bytes()
    {
        return Err("workers/0/agent/program: expected this temper binary".into());
    }
    let _agent_document = public(&crate::path(&dirs.config).join("agent.json"))?;
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
            public_path(&crate::path(bundle).join("index.html"))?;
            let _checked = crate::bundle::read(bundle, named.bundle_limits)?;
        }
        None => return Err("application.clients.bundle: effective document must name its loopback bundle".into()),
    }
    Ok(())
}
fn bundle(
    source: &Source,
    files: &[(&str, &[u8])],
    limits: temper_service::limits::Clients,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = &source.bundle {
        public_path(&path.join("index.html"))?;
        let _checked = crate::bundle::read(path.as_os_str().as_bytes(), limits)?;
        Ok(Some(path.clone()))
    } else if files.is_empty() {
        Err("application.clients.bundle: no loopback bundle; build with scripts/bundle.sh --loopback and set application.clients.bundle, or use a temper that packages the loopback bundle".into())
    } else {
        if files.len() > usize::try_from(limits.bundle_files).expect("bundle count fits") {
            return Err("packaged loopback bundle: past clients.bundle_files".into());
        }
        for (index, (name, _)) in files.iter().enumerate() {
            if (*name != "index.html" && !name.contains('/')) || files[..index].iter().any(|(prior, _)| prior == name) {
                return Err("packaged loopback bundle: duplicate or unexpected root file".into());
            }
        }
        // The ordinary bundle checker validates digest, file types, byte and count bounds first.
        let _checked = crate::bundle::embedded(files, limits)?;
        for (name, _) in files {
            let path = Path::new(name);
            if path.is_absolute()
                || path.components().any(|part| !matches!(part, Component::Normal(_)))
                || name.contains('\0')
            {
                return Err("packaged loopback bundle: paths must be plain relative names".into());
            }
        }
        Ok(None)
    }
}
fn print(dirs: &Directories) {
    eprintln!("box: effective configuration {}", crate::path(&dirs.config).display());
    eprintln!("box: effective data {}", crate::path(&dirs.data).display());
    eprintln!("box: effective cache {}", crate::path(&dirs.cache).display());
    for (kind, name) in [("forge", "github.json"), ("oauth", "oauth.json"), ("llm", "llm.json")] {
        let document = crate::path(&dirs.config).join(name);
        let quoted = format!("'{}'", document.to_string_lossy().replace('\'', "'\\''"));
        eprintln!("box: manually start temper fake {kind} --document {quoted} --seed 7");
    }
    eprintln!("box: manual peer reservations released; peers bind their own listeners");
}
#[expect(
    clippy::too_many_lines,
    reason = "startup owns its lock and listeners through validation and publishes the prepared marker last"
)]
fn prepare(source_dirs: &Directories, files: &[(&str, &[u8])]) -> Result<Prepared, String> {
    let (root, effective) = directories(source_dirs)?;
    let source_config = resolved(Path::new("/"), &source_dirs.config)?;
    if source_config.starts_with(&root) {
        return Err(
            "source configuration overlaps the fake-backend sandbox; choose configuration outside D/box/fake-backends"
                .into(),
        );
    }
    let marker = root.join("prepared");
    let exists = match fs::symlink_metadata(&root) {
        Ok(_) => true,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => false,
        Err(why) => return Err(format!("{}: {why}", root.display())),
    };
    let source = Source::read(source_dirs).map_err(|why| if exists { reset(&root, &why) } else { why })?;
    // Reuse is independent of the current binary's packaged bundle: retained effective edits win.
    let provisioned = if exists {
        None
    } else {
        let limits = if let Some((document, _)) = &source.documents {
            crate::setup::check(&document.encode()?, &source_dirs.jig())?.named.bundle_limits
        } else {
            temper_service::limits::profile(temper_service::limits::Profile::Small).clients
        };
        Some(bundle(&source, files, limits)?)
    };
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
        for name in ["temper.json", "agent.json", "github.json", "oauth.json", "llm.json"] {
            private_document(&crate::path(&effective.config).join(name))?;
        }
        let _github =
            temper_fake::forge::Document::decode(&public(&crate::path(&effective.config).join("github.json"))?)
                .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
        let _model = temper_fake::llm::Document::decode(&public(&crate::path(&effective.config).join("llm.json"))?)
            .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
        let _oauth = Json::decode(&public(&crate::path(&effective.config).join("oauth.json"))?)?;
        let bytes = public(&effective.document())?;
        validate(&effective, &bytes)?;
        print(&effective);
        return Ok(Prepared { directories: effective, bytes, listeners: Listeners(Vec::new()), _lock: lock });
    }
    let written = crate::init::core(&effective)?;
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
    let peers = Peers { github: addr(&github)?, oauth: addr(&oauth)?, llm: addr(&llm)? };
    let bundle = match provisioned.expect("new preparation checked the bundle") {
        Some(path) => path,
        None => {
            let directory = crate::path(&effective.config).join("bundle-loopback");
            make_private(&directory, user)?;
            for (name, content) in files {
                let destination = directory.join(name);
                make_private(destination.parent().expect("packaged path parent"), user)?;
                write(&destination, content, false)?;
            }
            directory
        }
    };
    let paths = Paths {
        config: effective.config.clone(),
        data: effective.data.clone(),
        cache: effective.cache.clone(),
        bundle: bytes(&bundle),
    };
    let documents =
        backends::rewrite(&initial, &public(&crate::path(&effective.config).join("agent.json"))?, &paths, &peers)
            .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    // Existing peer decoders check their generated documents before any marker is published.
    let _forge = temper_fake::forge::Document::decode(&documents.github)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    let _model = temper_fake::llm::Document::decode(&documents.llm)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    for (name, document) in [
        ("temper.json", &documents.temper),
        ("agent.json", &documents.agent),
        ("github.json", &documents.github),
        ("oauth.json", &documents.oauth),
        ("llm.json", &documents.llm),
    ] {
        write(&crate::path(&effective.config).join(name), document, name == "temper.json" || name == "agent.json")?;
    }
    validate(&effective, &documents.temper)?;
    // Last: a crash or refusal earlier is visibly partial and is never repaired by importing records.
    write(&marker, &source.marker, false)?;
    drop((github, oauth, llm));
    print(&effective);
    Ok(Prepared { directories: effective, bytes: documents.temper, listeners: held, _lock: lock })
}
#[cfg(test)]
mod tests;
