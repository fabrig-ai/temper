//! Only current init output, with an explicit bundle and public charter edits, is importable in M1.
use super::{Directories, Json, bytes, public, resolved};
use std::{
    num::NonZeroU16,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) struct Source {
    pub marker: Box<[u8]>,
    pub documents: Option<(Json, Json)>,
    pub charters: Vec<(&'static str, Box<[u8]>)>,
    pub bundle: Option<PathBuf>,
}
const RESOURCES: &[(&str, bool, &str)] = &[
    ("engine/deployment/directory", false, "deployment"),
    ("engine/hosts/workers/0/digest", false, "worker.digest"),
    ("engine/charters/0/instructions", true, "charters/chat.txt"),
    ("engine/charters/1/instructions", true, "charters/coding.txt"),
    ("workers/0/secret", false, "worker.secret"),
    ("workers/0/agent/configuration", true, "agent.json"),
    ("workers/0/agent/arguments/1", true, "agent.json"),
    ("application/setup/0/directory", false, "demo/forge"),
    ("application/setup/0/mode/fake/git", false, "demo/bin/git"),
    ("application/setup/1/directory", false, "forge-3"),
    ("application/demo/llm/key", false, "demo/llm.key"),
    ("application/workers/0/root", false, "workspaces"),
];
impl Source {
    pub(super) fn read(dirs: &Directories) -> Result<Self, String> {
        let config = crate::path(&dirs.config);
        let empty = match std::fs::read_dir(config) {
            Ok(mut entries) => entries.next().transpose().map_err(|why| why.to_string())?.is_none(),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => true,
            Err(why) => return Err(format!("source configuration {}: {why}", config.display())),
        };
        let mut marker =
            format!("temper fake-backends v1\nconfig={:?}\ndata={:?}\n", dirs.config, dirs.data).into_bytes();
        if empty {
            marker.extend_from_slice(b"empty\n");
            return Ok(Self { marker: marker.into(), documents: None, charters: Vec::new(), bundle: None });
        }
        let temper_bytes = public(&dirs.document())?;
        let agent_bytes = public(&config.join("agent.json"))?;
        let mut temper = Json::decode(&temper_bytes)?;
        let mut agent = Json::decode(&agent_bytes)?;
        let binary = std::env::current_exe().map_err(|why| why.to_string())?;
        let git = match temper.at("application/workers/0/git")? {
            Json::Null => None,
            Json::String(path) => Some(current_executable(b"git", path, "application/workers/0/git")?),
            Json::Object(_) | Json::Array(_) | Json::Number(_) | Json::Bool(_) => {
                return Err("application/workers/0/git: expected init's executable or null".into());
            }
        };
        let claude = if agent.at("programs/claude_code").is_ok() {
            Some(current_executable(
                b"claude",
                agent.string("programs/claude_code/path")?,
                "agent.json/programs/claude_code/path",
            )?)
        } else {
            None
        };
        let ports = ports(&temper)?;
        let login = temper.string("application/setup/1/mode/owned/person/login")?;
        let generated = temper_init::files(
            &temper_init::Facts {
                binary: binary.as_os_str().as_bytes(),
                git: git.as_ref().map(|path| path.as_os_str().as_bytes()),
                claude: claude.as_ref().map(|path| path.as_os_str().as_bytes()),
                ports,
                login: Some(login),
            },
            &temper_init::Directories { config: &dirs.config, data: &dirs.data },
            &[0; 32],
            &[0; 32],
        )
        .map_err(|why| format!("source init shape: {why:?}"))?;
        let (mut expected, expected_agent) = documents(generated)?;
        let bundle = match temper.at("application/clients/bundle")? {
            Json::Null => None,
            Json::String(path) => Some(resolved(config, path)?),
            Json::Object(_) | Json::Array(_) | Json::Number(_) | Json::Bool(_) => {
                return Err("application/clients/bundle: expected a directory or null".into());
            }
        };
        // Bundle is an admitted public override. Its directory is checked before sandbox creation.
        expected.set("application/clients/bundle", temper.at("application/clients/bundle")?.clone())?;
        for &(pointer, configuration, name) in RESOURCES {
            equivalent(
                &mut temper,
                &expected,
                pointer,
                crate::path(if configuration { &dirs.config } else { &dirs.data }),
                name,
            )?;
        }
        if claude.is_some() {
            equivalent(
                &mut agent,
                &expected_agent,
                "programs/claude_code/directory",
                crate::path(&dirs.data),
                "programs",
            )?;
        }
        temper.same(&expected, "temper.json")?;
        agent.same(&expected_agent, "agent.json")?;
        let checked = crate::setup::check(&temper.encode()?, &dirs.jig())?;
        let mut charters = Vec::new();
        for name in ["charters/chat.txt", "charters/coding.txt"] {
            let path = config.join(name);
            let file = checked
                .named
                .files
                .iter()
                .find(|file| crate::path(&file.path) == path)
                .ok_or_else(|| format!("{name}: expected a public init charter"))?;
            let value = public(&path)?;
            if value.len() > usize::try_from(file.max).expect("charter bound fits") {
                return Err(format!("{name}: public charter past its byte bound"));
            }
            marker.extend_from_slice(&skein_lib::sha256::digest(&value));
            charters.push((name, value));
        }
        marker.extend_from_slice(&skein_lib::sha256::digest(&temper_bytes));
        marker.extend_from_slice(&skein_lib::sha256::digest(&agent_bytes));
        Ok(Self { marker: marker.into(), documents: Some((temper, agent)), charters, bundle })
    }
    /// Rebase only public paths whose init-equivalence was checked above. Relative data paths
    /// keep their spelling and resolve against D'; absolute spellings are made relative first.
    pub(super) fn rebase(&self, effective: &Directories) -> Result<Option<(Box<[u8]>, Box<[u8]>)>, String> {
        let Some((mut temper, mut agent)) = self.documents.clone() else { return Ok(None) };
        let agent_name = bytes(&crate::path(&effective.config).join("agent.json"));
        temper.set("workers/0/agent/configuration", Json::String(agent_name.clone()))?;
        temper.set("workers/0/agent/arguments/1", Json::String(agent_name))?;
        if let Ok(program) = agent.at_mut("programs/claude_code/directory") {
            *program = Json::String(bytes(&crate::path(&effective.data).join("programs")));
        }
        Ok(Some((temper.encode()?, agent.encode()?)))
    }
}
fn equivalent(value: &mut Json, expected: &Json, pointer: &str, root: &Path, name: &str) -> Result<(), String> {
    if resolved(root, value.string(pointer)?)? != resolved(root, name.as_bytes())? {
        return Err(format!("{pointer}: only init's {name} resource is supported"));
    }
    value.set(pointer, expected.at(pointer)?.clone())
}
fn current_executable(name: &[u8], given: &[u8], pointer: &str) -> Result<PathBuf, String> {
    match executable(name) {
        Some(path) if path.as_os_str().as_bytes() == given => Ok(path),
        Some(_) | None => Err(format!(
            "{pointer}: custom external executables are unsupported; expected current init's PATH selection"
        )),
    }
}
fn executable(name: &[u8]) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    crate::init::candidates(&skein_shell::environment(), name)
        .into_iter()
        .find(|path| std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0))
}
fn port(value: &Json, pointer: &str) -> Result<NonZeroU16, String> {
    let socket = std::str::from_utf8(value.string(pointer)?)
        .ok()
        .and_then(|s| s.parse::<std::net::SocketAddr>().ok())
        .filter(|addr| addr.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) && addr.port() != 0)
        .ok_or_else(|| format!("{pointer}: expected init's IPv4 loopback socket"))?;
    Ok(NonZeroU16::new(socket.port()).expect("nonzero checked"))
}
fn ports(value: &Json) -> Result<temper_init::Ports, String> {
    Ok(temper_init::Ports {
        clients: port(value, "application/clients/listener/socket")?,
        owned: port(value, "application/setup/1/mode/owned/listen/socket")?,
        forge: port(value, "application/setup/0/mode/fake/listen/socket")?,
        llm: port(value, "application/demo/llm/listen/socket")?,
        webhooks: port(value, "application/webhooks/listener/socket")?,
    })
}
fn documents(answer: temper_init::Answer) -> Result<(Json, Json), String> {
    let mut temper = None;
    let mut agent = None;
    for file in answer.files {
        if file.name == b"temper.json" || file.name == b"agent.json" {
            let temper_init::Content::Bytes { bytes, .. } = file.content else {
                unreachable!("init documents are bytes")
            };
            if file.name == b"temper.json" {
                temper = Some(Json::decode(&bytes)?);
            } else {
                agent = Some(Json::decode(&bytes)?);
            }
        }
    }
    Ok((temper.expect("init temper document"), agent.expect("init agent document")))
}
