//! The setup owner checks its document before commands read any files.
use jig_protocol_config as config;
use temper_forge_setup::{
    check::{self, Checked, Members},
    problem,
};
use temper_service::startup::{Named, Transport, User};

#[derive(Debug)]
pub(crate) struct Deployment {
    pub named: Named,
    pub checked: Checked,
    pub limits: temper_box::limits::Limits,
    pub demo: Option<temper_fake::demo::Demo>,
}

pub(crate) fn check_with(
    bytes: &[u8],
    directories: &config::Directories,
    given: &temper_service::stock::Given,
) -> Result<Deployment, String> {
    let named = temper_service::startup::named_with(bytes, directories, given)
        .map_err(|why| String::from_utf8_lossy(&temper_service::startup::lines(&why)).into_owned())?;
    let document = config::document(
        bytes,
        &config::DocumentLimits { bytes: super::CONFIG_BYTES, tokens: super::CONFIG_BYTES, depth: 32 },
    )
    .expect("named checked document");
    let mut forges = Vec::with_capacity(named.forges.len());
    for forge in &named.forges {
        let identity =
            named.identities.iter().find(|identity| identity.number == forge.identity).expect("named checked identity");
        forges.push(check::Forge {
            number: forge.number,
            api: match &forge.api {
                config::Address::Addr(a) => a.socket(),
                config::Address::Host { .. } => None,
            },
            plaintext: matches!(forge.transport, Transport::Plaintext),
            git: forge.git.clone(),
            user: match &forge.user {
                User::Id(value) => check::User::Id(*value),
                User::File(path) => check::User::File(path.clone()),
            },
            api_token: forge.api_token.clone(),
            webhook_secret: forge.webhook_secret.clone(),
            identity_token: identity.token.clone(),
            identity_login: identity.login.clone(),
        });
    }
    let mut listeners = match &named.clients {
        config::Address::Addr(a) => a.socket().into_iter().collect::<Vec<_>>(),
        config::Address::Host { .. } => Vec::new(),
    };
    if let Some(webhooks) = &named.webhooks
        && let config::Address::Addr(a) = &webhooks.listener
        && let Some(socket) = a.socket()
    {
        listeners.push(socket);
    }
    let members = Members { forges: forges.into_boxed_slice(), listeners: listeners.into_boxed_slice() };
    let application = super::member(&document, 0, b"application");
    let at = super::member(&document, application, b"setup");
    let setup_overrides: Vec<_> =
        named.overrides.iter().filter(|(name, _)| !name.starts_with(b"box.")).cloned().collect();
    let listener = named.webhooks.as_ref().map(|webhooks| super::address(&webhooks.listener)).transpose()?;
    let checked = check::checked(&document, at, directories, &members, listener, &setup_overrides)
        .map_err(|why| String::from_utf8_lossy(&problem::lines(&why)).into_owned())?;
    let mut problems = temper_service::startup::Problems::empty();
    for (index, entry) in checked.setup.entries.iter().enumerate() {
        use temper_forge_setup::setup::Mode;
        use temper_service::startup::Listener;
        let index = u32::try_from(index).expect("bounded setup");
        let local = match &entry.mode {
            Mode::Managed { .. } => None,
            Mode::Owned(owned) => Some((owned.listen, Listener::Owned(index))),
            Mode::Fake(fake) => Some((fake.listen, Listener::Fake(index))),
        };
        if let Some((listen, listener)) = local {
            temper_service::startup::redirect_ports(&named.redirects, Some(listen.port()), listener, &mut problems);
        }
    }
    let projects: Vec<_> = named.projects.iter().map(|project| project.number).collect();
    let facts = temper_fake::demo::Facts {
        llm: &named.llm,
        projects: &projects,
        setup: &checked.setup,
        listeners: &members.listeners,
        directories,
    };
    let demo = temper_fake::demo::read(&document, super::member(&document, application, b"demo"), &facts)
        .map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
    if let Some(demo) = &demo {
        temper_service::startup::redirect_ports(
            &named.redirects,
            Some(demo.llm().listen().port()),
            temper_service::startup::Listener::Demo,
            &mut problems,
        );
    }
    if !problems.is_empty() {
        return Err(String::from_utf8_lossy(&temper_service::startup::lines(&problems)).into_owned());
    }
    let limits = box_limits(&named.overrides)?;
    Ok(Deployment { named, checked, limits, demo })
}
pub(crate) fn check(bytes: &[u8], directories: &config::Directories) -> Result<Deployment, String> {
    check_with(bytes, directories, &temper_service::stock::Given::Stock)
}

fn box_limits(overrides: &[(Box<[u8]>, u64)]) -> Result<temper_box::limits::Limits, String> {
    use temper_box::limits::{self, Refused};
    let mut limits = limits::small();
    let mut problems = Vec::new();
    let prefix = "application.limits.overrides.";
    for (name, value) in overrides.iter().filter(|(name, _)| name.starts_with(b"box.")) {
        if let Err(why) = limits::apply(&mut limits, name, *value) {
            let reason = match why {
                Refused::Unknown => "no limit of temper's has this name",
                Refused::Range => "outside its type",
            };
            problems.push(format!("{prefix}{}: {reason}", String::from_utf8_lossy(name)));
        }
    }
    if let Err(why) = limits::check(&limits) {
        let names = why.names();
        let name = String::from_utf8_lossy(names[0]);
        let reason = match names.get(1) {
            Some(other) => format!("must agree with {}", String::from_utf8_lossy(other)),
            None => "past its bound".into(),
        };
        problems.push(format!("{prefix}{name}: {reason}"));
    }
    if problems.is_empty() { Ok(limits) } else { Err(problems.join("\n")) }
}
