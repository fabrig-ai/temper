//! The deployment's demo, read whole before any files (protocol/setup.md, 8.4).
use crate::{Refusal, limits, llm, read};
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::{self as config, Directory, Path};
use skein_fake_llm_domain::api;
use skein_json::Document as Json;
use skein_lib::{List, bytes};
use temper_forge_setup::setup::{Mode, Setup};

/// The key file's read bound: the largest account key and its newline.
/// jig's client-v11 Keyed.value ceiling is 4096 bytes; account keys are raw bytes.
pub const KEY_FILE_BYTES: u32 = 4097;

/// Why the demo key cannot be added as an account's key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRefused {
    Empty,
    TooLong,
}
impl KeyRefused {
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Empty => "holds no key",
            Self::TooLong => "key past 4096 bytes",
        }
    }
}
/// A demo key file's key: its bytes less a final newline (protocol/setup.md, 8.4).
pub fn key(file: &[u8]) -> Result<&[u8], KeyRefused> {
    let key = match file.strip_suffix(b"\n") {
        Some(key) => key,
        None => file,
    };
    if key.is_empty() {
        return Err(KeyRefused::Empty);
    }
    if key.len() > usize::try_from(KEY_FILE_BYTES - 1).expect("key bound") {
        return Err(KeyRefused::TooLong);
    }
    Ok(key)
}

/// The deployment's demo, checked against the document (protocol/setup.md, 8.4).
#[derive(Debug)]
pub struct Demo {
    llm: llm::Document,
    provider: u32,
    forge: u16,
    project: u32,
}
impl Demo {
    #[must_use]
    pub fn llm(&self) -> &llm::Document {
        &self.llm
    }
    #[must_use]
    pub fn provider(&self) -> u32 {
        self.provider
    }
    #[must_use]
    pub fn forge(&self) -> u16 {
        self.forge
    }
    #[must_use]
    pub fn project(&self) -> u32 {
        self.project
    }
}
/// What the demo is checked against, all from the checked document.
/// A borrowed view used only during this read; no service state retains it.
#[derive(Debug)]
pub struct Facts<'a> {
    pub llm: &'a config::Llm,
    pub projects: &'a [u32],
    pub setup: &'a Setup,
    pub listeners: &'a [SocketAddr],
    pub directories: &'a config::Directories,
}
/// `null` is no demo. Every refusal's path is rooted at `application.demo`.
pub fn read(document: &Json, at: u32, facts: &Facts<'_>) -> Result<Option<Demo>, Refusal> {
    if read::is_null(document, at) {
        return Ok(None);
    }
    let p = Path::root().member(b"application").member(b"demo");
    let mut o = read::object(document, at, &p)?;
    let llm_at = read::required(&mut o, document, b"llm", &p)?;
    let forge_at = read::required(&mut o, document, b"forge", &p)?;
    let project_at = read::required(&mut o, document, b"project", &p)?;
    read::finish(o, document, &p)?;
    let lp = p.member(b"llm");
    let mut o = read::object(document, llm_at, &lp)?;
    let listen_at = read::required(&mut o, document, b"listen", &lp)?;
    let provider_at = read::required(&mut o, document, b"provider", &lp)?;
    let key_at = read::required(&mut o, document, b"key", &lp)?;
    let asked_at = read::required(&mut o, document, b"asked", &lp)?;
    let cued_at = o.member(document, b"cued");
    let otherwise_at = read::required(&mut o, document, b"otherwise", &lp)?;
    read::finish(o, document, &lp)?;
    let listen = read::listen(document, listen_at, &lp.member(b"listen"))?;
    unique_listen(listen, facts, &lp.member(b"listen"))?;
    let pp = lp.member(b"provider");
    let provider = read::integer(document, provider_at, u64::from(u32::MAX), &pp)?;
    let provider = u32::try_from(provider).expect("u32 bound");
    let models = models(provider, listen, facts.llm, &pp)?;
    let key = data_path(document, key_at, facts.directories, &lp.member(b"key"))?;
    read::absolute(&key, &lp.member(b"key"))?;
    let asked = llm::scripts(document, asked_at, &lp.member(b"asked"))?;
    let cp = lp.member(b"cued");
    let cued = match cued_at {
        Some(at) => llm::scripts(document, at, &cp)?,
        None => Box::new([]),
    };
    let count = asked.len().checked_add(cued.len()).expect("two bounded script lists");
    if count > usize::try_from(crate::ENTRIES).expect("sixteen scripts") {
        return Err(read::why(&cp, b"past its bound"));
    }
    for (index, script) in cued.iter().enumerate() {
        for previous in &asked {
            if previous.cue == script.cue {
                return Err(read::why(&read::index(&cp, index).member(b"cue"), b"must be unique"));
            }
        }
    }
    let scripts =
        api::Scripts { cued, asked, otherwise: llm::turns(document, otherwise_at, &lp.member(b"otherwise"))? };
    let llm = llm::Document::checked(listen, key, None, models, scripts, &lp)?;
    let forge = read::integer(document, forge_at, u64::from(u16::MAX), &p.member(b"forge"))?;
    let forge = u16::try_from(forge).expect("u16 bound");
    let fake = match facts.setup.entry(forge) {
        Some(entry) => match &entry.mode {
            Mode::Fake(_) => true,
            Mode::Owned(_) | Mode::Managed { .. } => false,
        },
        None => false,
    };
    if !fake {
        return Err(read::why(&p.member(b"forge"), b"names no fake setup entry"));
    }
    let project = read::integer(document, project_at, u64::from(u32::MAX), &p.member(b"project"))?;
    let project = u32::try_from(project).expect("u32 bound");
    if !facts.projects.contains(&project) {
        return Err(read::why(&p.member(b"project"), b"names no project"));
    }
    Ok(Some(Demo { llm, provider, forge, project }))
}
fn data_path(d: &Json, at: u32, directories: &config::Directories, p: &Path) -> Result<Box<[u8]>, Refusal> {
    let text = read::text(d, at, limits::PATH_BYTES, p)?;
    match directories.join(Directory::Data, &text) {
        Ok(path) => Ok(path),
        Err(what) => Err(read::kind(p, what)),
    }
}
fn unique_listen(listen: SocketAddr, facts: &Facts<'_>, p: &Path) -> Result<(), Refusal> {
    if facts.listeners.contains(&listen) {
        return Err(read::why(p, b"must be unique"));
    }
    for entry in &facts.setup.entries {
        let local = match &entry.mode {
            Mode::Managed { .. } => None,
            Mode::Owned(owned) => Some(owned.listen),
            Mode::Fake(fake) => Some(fake.listen),
        };
        if local == Some(listen) {
            return Err(read::why(p, b"must be unique"));
        }
    }
    Ok(())
}
fn models(number: u32, listen: SocketAddr, llm: &config::Llm, p: &Path) -> Result<Box<[llm::Model]>, Refusal> {
    let mut found = None;
    for provider in &llm.providers {
        if provider.number == number {
            found = Some(provider);
        }
    }
    let Some(provider) = found else {
        return Err(read::why(p, b"names no LLM provider"));
    };
    if !provider.keys || provider.sign_in.is_some() || !provider.programs.is_empty() || provider.signed_in_plan {
        return Err(read::why(p, b"must offer a key alone"));
    }
    if provider.models.is_empty() {
        return Err(read::why(p, b"names a provider with no models"));
    }
    let mut models = List::with_capacity(u32::try_from(provider.models.len()).expect("checked provider models"));
    for name in &provider.models {
        let mut found = None;
        for model in &llm.models {
            if model.name == *name {
                found = Some(model);
            }
        }
        let model = found.expect("checked provider's model");
        let mut found = None;
        for endpoint in &llm.endpoints {
            if endpoint.number == model.endpoint {
                found = Some(endpoint);
            }
        }
        let endpoint = found.expect("checked model's endpoint");
        let socket = match &endpoint.address {
            config::Address::Addr(a) => a.socket(),
            config::Address::Host { .. } => None,
        };
        if socket != Some(listen) {
            return Err(read::why(p, b"names an endpoint not at llm.listen"));
        }
        match &endpoint.transport {
            config::Transport::Tls { .. } => return Err(read::why(p, b"names an endpoint with TLS")),
            config::Transport::Plaintext => {}
        }
        // Dialect 3 is OpenAI's public Responses route (configuration.md, section 2).
        if endpoint.dialect != 3 {
            return Err(read::why(p, b"names an endpoint not in OpenAI's dialect"));
        }
        models
            .push(llm::Model { name: bytes::copy_of(&model.name), window: model.window, output: model.output })
            .expect("provider model count");
    }
    llm::check_models(models.as_slice(), p, false)?;
    Ok(models.into_boxed())
}
