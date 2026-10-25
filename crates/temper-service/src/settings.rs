//! Typed engine and worker settings; startup supplies the values.
use alloc::boxed::Box;
use skein_lib::{List, Secret};

/// The deployment's demo as the clients read it (client/README.md, 8; protocol/setup.md, 8.4).
#[derive(Debug)]
pub struct Demo {
    pub project: u32,
    pub forge: u16,
    pub provider: u32,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub cues: Box<[Box<[u8]>]>,
    pub landing: Box<[u8]>,
    pub key_file: Box<[u8]>,
    pub state: Option<temper_engine_domain_forge::DemoState>,
}

impl Demo {
    pub(crate) fn domain(
        &self,
        forges: &[Forge],
        deadline: skein_lib::Duration,
    ) -> temper_engine_domain_forge::DemoConfig {
        use temper_engine_domain_forge as forge;
        let mut host = Box::default();
        for forge in forges {
            if forge.number == self.forge {
                host = crate::family::host(&forge.git);
                break;
            }
        }
        forge::DemoConfig {
            provider: self.provider,
            deadline,
            adoption: forge::Adoption {
                project: self.project,
                home: true,
                forge: self.forge,
                host,
                owner: self.owner.clone(),
                name: self.name.clone(),
                prefix: b"temper/".as_slice().into(),
                role: forge::Role::Owned,
                landing: self.landing.clone(),
                ci: true,
                checks: Box::new([]),
            },
        }
    }
}

/// A forge temper works on (protocol/configuration.md, section 3, `forges`).
#[derive(Debug)]
pub struct Forge {
    pub number: u16,
    pub kind: Kind,
    /// Where the forge component connects, never an in-process name.
    pub api: jig_protocol_config::Address,
    pub transport: Transport,
    /// The base of each remote and the host of the forge's resource paths.
    pub git: Box<[u8]>,
    /// The fake forge's resolved git program, in place of the worker's git.
    pub git_program: Option<Box<[u8]>>,
    pub web: Option<Box<[u8]>>,
    /// The pasted GitHub App, its hook and installations belong to another owner.
    pub app_read_only: bool,
    /// The API token, read from its private file: the forge component's alone.
    pub api_token: Option<Secret>,
    pub webhook_secret: Option<Secret>,
    /// The forge user the API token belongs to: the forge writer.
    pub user: u64,
    /// The number of this forge's git identity.
    pub identity: u32,
}

/// The documents a forge speaks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Forgejo,
    GitHub,
}

/// A forge component's connections.
#[derive(Debug)]
pub enum Transport {
    /// Only to a socket address on loopback.
    Plaintext,
    /// The trust file's bytes, or none for the machine's roots.
    Tls { server_name: Box<[u8]>, trust: Option<Box<[u8]>> },
}

/// A git identity (protocol/configuration.md, section 3, `identities`).
#[derive(Debug)]
pub struct Identity {
    pub number: u32,
    pub forge: u16,
    pub token: Secret,
    pub login: Box<[u8]>,
    pub author: Author,
}

/// The author and committer of the commits made with an identity.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Author {
    pub name: Box<[u8]>,
    pub email: Box<[u8]>,
}

/// The private files jig's parts name, by path, as jig's reference takes them.
#[derive(Debug)]
pub struct Files {
    pub entries: List<(Box<[u8]>, Secret)>,
}

/// The seeds the shell draws at start, as jig's reference takes them.
#[derive(Debug)]
pub struct Seeds {
    pub deployment: [u8; 16],
    pub seed: u64,
    pub crypto: Secret,
    /// The installation's identifier drawn for a first start; a later start
    /// sends the one its store keeps (jig's configuration.md, 8).
    pub installation: [u8; 16],
}

/// One checked setup entry and its start-time handover (protocol/setup.md, 8.4).
#[derive(Debug)]
pub struct Setup {
    pub entry: temper_forge_setup::setup::Entry,
    /// The private handover for `TakeOver`; temper-15 removes the old fake-adoption use.
    pub handover: Option<Secret>,
}

/// The engine's settings: jig's part, typed by jig-protocol-config, and
/// temper's members (protocol/configuration.md, sections 2 and 3).
#[derive(Debug)]
pub struct Engine {
    pub jig: jig_protocol_config::Engine,
    pub clients: crate::startup::application::Clients,
    pub sign_in: Option<SignIn>,
    pub webhooks: Option<skein_forge_connection::hooks::Webhooks>,
    pub files: Files,
    pub forges: Box<[Forge]>,
    pub demo: Option<Demo>,
    pub setup: Box<[Setup]>,
    /// Checked local entries whose access comes from records.
    pub records_only: Box<[u16]>,
    pub identities: Box<[Identity]>,
    pub landing: temper_engine_domain::LandingPolicy,
    pub charter: u32,
    pub limits: super::limits::Limits,
}

/// The adapters recognized by smith, separate from a program source's name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Adapter {
    ClaudeCode,
}
impl Adapter {
    #[must_use]
    pub fn name(self) -> &'static [u8] {
        match self {
            Self::ClaudeCode => b"claude_code",
        }
    }
}
/// Smith's checked bound for each program path.
pub const PROGRAM_PATH_BYTES: usize = 4096;
/// Secret-free declaration, retained once by the worker; never a login.
#[derive(Clone, Debug)]
pub struct Program {
    pub adapter: Adapter,
    pub path: Box<[u8]>,
    pub directory: Box<[u8]>,
}
/// The worker's agent profile, which the binary reads from smith's
/// agent configuration, as jig's reference service's `Agent`
/// (`lib.rs:36-41`); 09 builds the worker's host half from it.
#[derive(Debug)]
pub struct Agent {
    pub tool_payload: u32,
    pub programs: Box<[Program]>,
    pub endpoints: smith_protocol_channel::Endpoints,
    pub limits: smith_host_protocol::Limits,
    pub bounds: jig_domain_host::Bounds,
    pub host_limits: smith_host_domain::Limits,
}

/// A worker's settings: jig's part and temper's (protocol/configuration.md,
/// section 4); 09 builds the worker from it.
#[derive(Debug)]
pub struct Worker {
    pub jig: jig_protocol_config::Worker,
    /// The worker's root directory: workspaces, trash and home under it.
    pub root: Box<[u8]>,
    /// The absolute path of the git program, or none on this machine.
    pub git: Option<Box<[u8]>>,
}

/// Checked Google registration; the client secret is private startup input.
#[derive(Debug)]
pub struct SignIn {
    pub provider: u16,
    pub client: Box<[u8]>,
    pub secret: Secret,
    pub issuer: Box<[u8]>,
    pub authorization: Box<[u8]>,
    pub token: Box<[u8]>,
    pub keys: Box<[u8]>,
    pub transport: Option<Transport>,
    pub members: Box<[crate::startup::application::sign_in::Member]>,
    pub forges: Box<[crate::startup::application::sign_in::ForgeSignIn]>,
}
