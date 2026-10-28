//! Mapping the configured setup forges to the pinned host (protocol/setup.md, 5.1).
use alloc::boxed::Box;
use jig_protocol_config::{Path, Problems, What};
use skein_forge_setup::{github, host};
use skein_lib::{CryptoRng, List, Writer};
use temper_forge_setup::setup::Mode;

pub(super) fn fixed(
    config: &crate::settings::Engine,
    forges: &skein_forge_connection::Settings,
) -> Result<(host::Settings, host::Limits), Problems> {
    let own = &config.limits;
    let pure = own.setup.plan();
    check(&own.setup)?;
    let mut configured = List::with_capacity(own.forge_component.forges);
    for endpoint in &forges.forges {
        let source = source_forge(config, endpoint.number);
        let transport = match &endpoint.transport {
            skein_forge_connection::settings::Transport::Plaintext { name } => {
                host::Transport::Plaintext { name: name.clone() }
            }
            skein_forge_connection::settings::Transport::Tls { server_name, trust } => {
                host::Transport::Tls { server_name: server_name.clone(), trust: trust.clone() }
            }
        };
        let skein_io::Addr::Ip(address) = endpoint.address else {
            continue;
        };
        let plaintext = match &transport {
            host::Transport::Plaintext { .. } => true,
            host::Transport::Tls { .. } => false,
        };
        if plaintext && !address.ip().is_loopback() {
            continue;
        }
        // Every kind serves picker reads; writing plans use each configured setup entry.
        if source.kind == crate::settings::Kind::Forgejo {
            let given = forgejo(config, source)?;
            configured
                .push(host::Forge { kind: host::Kind::Forgejo(given), address: endpoint.address, transport })
                .expect("configured host forges");
            continue;
        }
        let url = match &config.clients.public {
            Some(origin) => origin.base(),
            None => listener(&config.clients.listener)?,
        };
        let number = skein_lib::Decimal::of(u64::from(source.number));
        let given = github::given::Given {
            forge: source.number,
            // Another owner's App: its hook and installations are read, never changed (setup.md 17.7).
            read_only: source.app_read_only,
            word: Box::from(b"temper".as_slice()),
            web: match &source.web {
                Some(web) => web.clone(),
                None => Box::from(b"https://github.com".as_slice()),
            },
            url: url.clone(),
            redirect_url: joined(&[&url, b"forges/", number.as_bytes(), b"/github/redirect"]),
            setup_url: joined(&[&url, b"forges/", number.as_bytes(), b"/github/setup"]),
            limits: github::limits::Limits {
                key_bytes: own.forge_component.key_bytes,
                name_bytes: own.forge.name_bytes,
                page_bytes: own.forge_component.body_bytes,
                installations: own.forges.installations,
                reachable: own.picker.page,
                // At most one installation per page, plus the terminal empty page.
                pages: own.forges.installations.checked_add(1).ok_or(problem(b"forges.installations"))?,
                state_life: own.setup.state_life,
                deadline: pure.deadline,
                backoff: pure.backoff,
                backoff_max: pure.backoff_max,
            },
        };
        if let Err(refused) = github::given::check(&given) {
            let mut found = Problems::empty();
            problems(host::Unusable::GitHubGiven { forge: source.number, refused }, &mut found);
            return Err(found);
        }
        configured
            .push(host::Forge { kind: host::Kind::GitHub(given), address: endpoint.address, transport })
            .expect("configured host forges");
    }
    let f = own.forge_component;
    let limits = host::Limits {
        forges: f.forges,
        runs: own.setup.runs,
        states: own.setup.states,
        token_bytes: f.token_bytes,
        head_bytes: f.head_bytes,
        headers: f.headers,
        body_bytes_in: f.body_bytes_in,
        depth: f.depth,
        chunk: f.chunk,
        string_bytes: f.string_bytes.max(f.key_bytes.checked_mul(2).ok_or(problem(b"forge_component.key_bytes"))?),
        body_bytes: f.body_bytes,
        inner: f.inner.max(own.picker.page),
        target_bytes: f.target_bytes,
        io: config.jig.limits.io,
        tls: f.tls,
    };
    Ok((host::Settings { forges: configured, user_agent: Box::from(crate::forge::USER_AGENT.as_bytes()) }, limits))
}
fn forgejo(
    config: &crate::settings::Engine,
    source: &crate::settings::Forge,
) -> Result<skein_forge_setup::given::Given, Problems> {
    for setup in &config.setup {
        if setup.entry.forge != source.number {
            continue;
        }
        let listener = if !admin_connect(&setup.entry.mode)
            && (config.records_only.contains(&source.number) || source.webhook_secret.is_some())
        {
            match &config.webhooks {
                Some(hooks) => match hooks.listener {
                    skein_io::Addr::Ip(address) => Some(address),
                    skein_io::Addr::InProcess(_) => unreachable!("checked webhooks listener"),
                },
                None => None,
            }
        } else {
            None
        };
        // The configured fallback is replaced by Listening before the host starts.
        let mut limits = config.limits.setup.plan();
        limits.repositories = config.limits.forge.client.repositories;
        limits.reachable = config.limits.picker.page;
        let given = temper_forge_setup::given::values(&setup.entry, listener, &limits);
        if let Err(refused) = skein_forge_setup::given::check(&given) {
            let mut found = Problems::empty();
            problems(host::Unusable::Given { forge: source.number, refused }, &mut found);
            return Err(found);
        }
        return Ok(given);
    }
    // A connected Forgejo runs only read plans here. The kit requires the writing
    // plan's names too; use temper's valid constants, never the connected user's login.
    let mut limits = config.limits.setup.plan();
    limits.reachable = config.limits.picker.page;
    limits.repositories = config.limits.forge.client.repositories;
    let given = skein_forge_setup::given::Given {
        forge: source.number,
        bot_login: b"temper".as_slice().into(),
        bot_email: b"temper@localhost".as_slice().into(),
        admin_name: b"temper-admin".as_slice().into(),
        api_name: b"temper-api".as_slice().into(),
        git_name: b"temper-git".as_slice().into(),
        orgs: Box::new([]),
        org_owner: None,
        adopted: Box::new([]),
        listener: None,
        word: b"temper".as_slice().into(),
        limits,
    };
    if let Err(refused) = skein_forge_setup::given::check(&given) {
        let mut found = Problems::empty();
        problems(host::Unusable::Given { forge: source.number, refused }, &mut found);
        return Err(found);
    }
    Ok(given)
}
pub(crate) fn prepare(
    config: &crate::settings::Engine,
    forges: &mut skein_forge_connection::Settings,
    seed: &skein_lib::Secret,
    under_box: bool,
) -> Result<super::Layer, Problems> {
    let (settings, limits) = fixed(config, forges)?;
    let own = &config.limits;
    let crypto = CryptoRng::new(seed, 5).ok_or(problem(b"setup"))?;
    let mut layer = super::Layer::new(
        settings,
        limits,
        crypto,
        own.setup.state_life,
        own.records.value_bytes,
        own.forge_component.installations,
    );
    for source in &config.forges {
        if source.kind == crate::settings::Kind::Forgejo
            && let Some(token) = &source.api_token
        {
            layer.file_api.insert(source.number, token.clone()).expect("configured Forgejo tokens");
        }
    }
    // Only Forgejo entries that can make hooks need the bind before starting.
    if let Some((settings, _)) = &layer.pending {
        for forge in &settings.forges {
            if let host::Kind::Forgejo(given) = &forge.kind
                && given.listener.is_some()
            {
                layer.waiting_for_webhooks = true;
                break;
            }
        }
    }
    for forge in &config.forges {
        if !layer.served.contains_key(&forge.number) {
            let reason: &[u8] = match forge.kind {
                crate::settings::Kind::GitHub => b"setup host serves plaintext only on loopback",
                crate::settings::Kind::Forgejo => {
                    let mut reason = b"Forgejo setup has no owned or fake entry".as_slice();
                    for setup in &config.setup {
                        if setup.entry.forge == forge.number {
                            reason = match &setup.entry.mode {
                                Mode::Managed { .. } => b"managed Forgejo setup is not staged yet",
                                Mode::Owned(_) | Mode::Fake(_) => b"setup host serves plaintext only on loopback",
                            };
                            break;
                        }
                    }
                    reason
                }
            };
            layer.unserved.insert(forge.number, reason).expect("configured setup exclusions");
        }
    }
    local_starts(config, &mut layer, under_box)?;
    // temper-15 removes the old file access and fake handover/adoption branch.
    for setup in &config.setup {
        if !config.records_only.contains(&setup.entry.forge)
            && let Mode::Fake(fake) = &setup.entry.mode
        {
            for index in 0..forges.forges.len() {
                let forge = forges.forges.get_mut(index).expect("configured forge");
                if forge.number == setup.entry.forge {
                    if forge.access.is_none() {
                        break;
                    }
                    layer
                        .lifecycle
                        .adoptions
                        .insert(
                            forge.number,
                            super::lifecycle::Adoption {
                                next: core::num::NonZeroU64::new(1).expect("first repository"),
                                repositories: u64::try_from(fake.repositories.len()).expect("checked repositories"),
                                files: Some(super::lifecycle::AdoptionFiles {
                                    admin_token: setup.handover.as_ref().expect("checked fake handover").clone(),
                                    access: forge.access.take().expect("fake access from files"),
                                }),
                            },
                        )
                        .expect("one adoption per fake forge");
                    break;
                }
            }
        }
    }
    Ok(layer)
}
fn local_starts(config: &crate::settings::Engine, layer: &mut super::Layer, under_box: bool) -> Result<(), Problems> {
    let own = &config.limits;
    let demo_forge = match &config.demo {
        Some(demo) if under_box => Some(demo.forge),
        Some(_) | None => None,
    };
    for setup in &config.setup {
        if !config.records_only.contains(&setup.entry.forge) {
            continue; // temper-15 removes old-form file startup and its fake adoption.
        }
        if let Some(value) = &setup.handover
            && value.expose().len() > usize::try_from(own.records.value_bytes).expect("record bound")
        {
            return Err(problem(b"records.value_bytes"));
        }
        let source = source_forge(config, setup.entry.forge);
        assert!(source.api_token.is_none(), "records-only starts without access");
        let mut joined = None;
        for identity in &config.identities {
            if identity.number == source.identity {
                joined = Some(identity);
            }
        }
        let identity = joined.expect("checked identity");
        let login = &setup.entry.bot.login;
        let local = super::lifecycle::Local {
            first: None,
            source: if admin_connect(&setup.entry.mode) {
                super::lifecycle::Source::Managed
            } else {
                super::lifecycle::Source::Prepared
            },
            repositories: match &setup.entry.mode {
                Mode::Fake(fake) => u64::try_from(fake.repositories.len()).expect("checked repositories"),
                Mode::Owned(_) | Mode::Managed { .. } => 0,
            },
            pending: true,
            handover: setup.handover.clone(),
            path: temper_forge_setup::place::Place { directory: setup.entry.directory.clone(), user: Box::new([]) }
                .path(temper_forge_setup::place::File::AdminToken),
            file: super::lifecycle::FileStep::Idle,
            trigger: if demo_forge == Some(setup.entry.forge) {
                super::lifecycle::Trigger::StartDemo
            } else if setup.entry.asked {
                super::lifecycle::Trigger::StartAsked
            } else {
                super::lifecycle::Trigger::Start
            },
            restored: false,
            limits: {
                let mut limits = own.setup.plan();
                limits.repositories = own.forge.client.repositories;
                limits
            },
            webhooks: config.webhooks.is_some() && !admin_connect(&setup.entry.mode),
            bot_login: login.clone(),
            identity: crate::settings::Identity {
                number: identity.number,
                forge: source.number,
                token: skein_lib::Secret::new(Box::new([])),
                login: if identity.login.is_empty() { login.clone() } else { identity.login.clone() },
                author: identity.author.clone(),
            },
        };
        let wire = crate::translate::limits(&config.limits).ok_or(problem(b"checkout.name_bytes"))?;
        if crate::translate::identity(&local.identity, &wire).is_err() {
            return Err(problem(b"checkout.name_bytes"));
        }
        layer
            .lifecycle
            .reconcile
            .insert(source.number, super::lifecycle::Reconcile::Local(local))
            .expect("one local start per forge");
    }
    Ok(())
}
fn source_forge(config: &crate::settings::Engine, number: u16) -> &crate::settings::Forge {
    for forge in &config.forges {
        if forge.number == number {
            return forge;
        }
    }
    unreachable!("resolved configured forge")
}
fn check(own: &temper_forge_setup::limits::Host) -> Result<(), Problems> {
    let pure = own.plan();
    if pure.orgs == 0
        || own.runs == 0
        || own.states == 0
        || own.state_life == skein_lib::Duration::ZERO
        || own.state_life > github::limits::STATE_LIFE_MAX
    {
        return Err(problem(b"setup"));
    }
    if let Err(why) = skein_forge_setup::limits::check(&pure) {
        let mut found = Problems::empty();
        for place in [Some(why.place), why.other].into_iter().flatten() {
            use skein_forge_setup::given::Place;
            let name = match place {
                Place::Backoff => b"setup.backoff_ms".as_slice(),
                Place::BackoffMax => b"setup.backoff_max_ms",
                Place::Deadline => b"setup.deadline_ms",
                Place::FilesLimit
                | Place::FileBytesLimit
                | Place::CreateOrg
                | Place::CreateRepository
                | Place::Files
                | Place::FilePath(_)
                | Place::FileContent(_)
                | Place::Plan => unreachable!("Create inputs are admitted by the domain and fixed starter profile"),
                Place::Word
                | Place::BotLogin
                | Place::BotEmail
                | Place::AdminName
                | Place::ApiName
                | Place::GitName
                | Place::Orgs
                | Place::Org(_)
                | Place::OrgOwner
                | Place::Repositories
                | Place::Repository(_)
                | Place::OrgsLimit
                | Place::RepositoriesLimit => b"setup.orgs",
                Place::ReachableLimit | Place::Page => b"picker.page",
            };
            found.push(limit_path(name), What::Bound);
        }
        return Err(found);
    }
    Ok(())
}
fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut len = 0_usize;
    for part in parts {
        len = len.checked_add(part.len()).expect("bounded setup URL");
    }
    let mut writer = Writer::new(len);
    for part in parts {
        writer.put(part).expect("measured setup URL");
    }
    writer.finish()
}
#[expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "configuration protocol renders a checked listener address"
)]
fn listener(address: &jig_protocol_config::Address) -> Result<Box<[u8]>, Problems> {
    let value = match address {
        jig_protocol_config::Address::Addr(skein_io::Addr::Ip(address)) => alloc::format!("http://{address}/"),
        jig_protocol_config::Address::Host { name, port } => {
            alloc::format!("http://{}:{port}/", core::str::from_utf8(name).expect("checked listener name"))
        }
        jig_protocol_config::Address::Addr(skein_io::Addr::InProcess(_)) => return Err(problem(b"setup")),
    };
    Ok(value.into_bytes().into_boxed_slice())
}
pub(super) fn limit_path(name: &[u8]) -> Path {
    Path::root().member(b"application").member(b"limits").member(b"overrides").member(name)
}
fn problem(name: &[u8]) -> Problems {
    let mut found = Problems::empty();
    found.push(limit_path(name), What::Bound);
    found
}
/// All pinned host refusals name the members which feed that bound.
pub(crate) fn problems(why: host::Unusable, found: &mut Problems) {
    use host::{Stacked, Unusable};
    let names: &[&[u8]] = match why {
        Unusable::Zero => &[b"setup.runs", b"setup.states", b"forge_component", b"io"],
        Unusable::TokenBytes { .. } => &[b"forge_component.token_bytes"],
        Unusable::Documents(_) | Unusable::GitHubDocuments(_) => {
            &[b"forge_component.depth", b"forge_component.body_bytes_in", b"forge_component.chunk"]
        }
        Unusable::GitHubMetadata { .. } => &[b"forge_component.inner", b"forge_component.string_bytes"],
        Unusable::Stack { stacked, .. } => match stacked {
            Stacked::HttpOverIo => &[b"forge_component.chunk", b"io.intake", b"io.output"],
            Stacked::HttpOverTls => {
                &[b"forge_component.chunk", b"forge_component.tls.read", b"forge_component.tls.send"]
            }
            Stacked::TlsOverIo => {
                &[b"forge_component.tls.read", b"forge_component.tls.send", b"io.intake", b"io.output"]
            }
            Stacked::JsonOverHttp => &[b"forge_component.chunk"],
        },
        Unusable::Forges { .. } => &[b"forge_component.forges"],
        Unusable::Duplicate { forge }
        | Unusable::Address { forge }
        | Unusable::PlaintextName { forge }
        | Unusable::Request { forge }
        | Unusable::Given { forge, .. }
        | Unusable::GitHubGiven { forge, .. }
        | Unusable::Reachable { forge, .. } => {
            // Forge numbers, not array indices, are retained in the host diagnostic.
            let number = skein_lib::Decimal::of(u64::from(forge));
            found.push(Path::root().member(b"application").member(b"forges").member(number.as_bytes()), What::Bound);
            match why {
                Unusable::GitHubGiven { refused, .. } => {
                    for place in [Some(refused.place), refused.other].into_iter().flatten() {
                        found.push(limit_path(github_member(place)), What::Bound);
                    }
                    return;
                }
                Unusable::Reachable { .. } => &[b"picker.page", b"forge_component.inner"],
                Unusable::Zero
                | Unusable::TokenBytes { .. }
                | Unusable::Documents(_)
                | Unusable::GitHubDocuments(_)
                | Unusable::GitHubMetadata { .. }
                | Unusable::Stack { .. }
                | Unusable::Forges { .. }
                | Unusable::Duplicate { .. }
                | Unusable::Address { .. }
                | Unusable::PlaintextName { .. }
                | Unusable::Request { .. }
                | Unusable::Given { .. } => &[b"forge_component", b"setup"],
            }
        }
    };
    for name in names {
        found.push(limit_path(name), What::Bound);
    }
}
fn github_member(place: github::given::Place) -> &'static [u8] {
    use github::given::Place;
    match place {
        Place::KeyBytes => b"forge_component.key_bytes",
        Place::NameBytes => b"forge.name_bytes",
        Place::PageBytes => b"forge_component.body_bytes",
        Place::Installations | Place::Pages => b"forges.installations",
        Place::StateLife => b"setup.state_life_ms",
        Place::Deadline => b"setup.deadline_ms",
        Place::Backoff => b"setup.backoff_ms",
        Place::BackoffMax => b"setup.backoff_max_ms",
        Place::Reachable | Place::Page => b"picker.page",
        Place::Word
        | Place::Target
        | Place::State
        | Place::Web
        | Place::Url
        | Place::RedirectUrl
        | Place::SetupUrl
        | Place::Code => b"setup",
    }
}

fn admin_connect(mode: &Mode) -> bool {
    match mode {
        Mode::Managed { admin_token: None } => true,
        Mode::Managed { admin_token: Some(_) } | Mode::Owned(_) | Mode::Fake(_) => false,
    }
}
