//! Effect-free setup preflight, as temper serve performs (protocol/setup.md, 8.4).
use jig_protocol_config as config;
use temper_forge_setup::check::{self, Checked, Members};
use temper_service::startup::{Transport, User};

pub(crate) fn member(document: &skein_json::Document, at: u32, name: &[u8]) -> u32 {
    config::read::Object::new(document, at).expect("document object").member(document, name).expect("document member")
}
pub(crate) fn setup(bytes: &[u8], directories: &config::Directories) -> Checked {
    let named = temper_service::named(bytes, directories).expect("deployment names");
    let document = config::document(bytes, &config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 })
        .expect("deployment JSON");
    let forges = named
        .forges
        .iter()
        .map(|forge| {
            let identity =
                named.identities.iter().find(|identity| identity.number == forge.identity).expect("forge identity");
            check::Forge {
                number: forge.number,
                api: match &forge.api {
                    config::Address::Addr(a) => a.socket(),
                    config::Address::Host { .. } => None,
                },
                plaintext: matches!(forge.transport, Transport::Plaintext),
                git: forge.git.clone(),
                user: match &forge.user {
                    User::Id(id) => check::User::Id(*id),
                    User::File(path) => check::User::File(path.clone()),
                },
                api_token: forge.api_token.clone(),
                webhook_secret: forge.webhook_secret.clone(),
                identity_token: identity.token.clone(),
                identity_login: identity.login.clone(),
            }
        })
        .collect();
    let clients = match &named.clients {
        config::Address::Addr(a) => a.socket().expect("clients socket"),
        config::Address::Host { .. } => panic!("world clients listener is a socket"),
    };
    let webhooks = named.webhooks.as_ref().map(|hooks| match &hooks.listener {
        config::Address::Addr(a) => a.socket().expect("webhooks socket"),
        config::Address::Host { .. } => panic!("world webhooks listener is a socket"),
    });
    let mut listeners = vec![clients];
    listeners.extend(webhooks);
    let members = Members { forges, listeners: listeners.into_boxed_slice() };
    let application = member(&document, 0, b"application");
    let overrides: Vec<_> = named.overrides.iter().filter(|(name, _)| !name.starts_with(b"box.")).cloned().collect();
    let checked = check::checked(
        &document,
        member(&document, application, b"setup"),
        directories,
        &members,
        webhooks,
        &overrides,
    )
    .expect("deployment setup");
    let projects: Vec<_> = named.projects.iter().map(|project| project.number).collect();
    temper_fake::demo::read(
        &document,
        member(&document, application, b"demo"),
        &temper_fake::demo::Facts {
            llm: &named.llm,
            projects: &projects,
            setup: &checked.setup,
            listeners: &members.listeners,
            directories,
        },
    )
    .expect("deployment demo");
    checked
}
