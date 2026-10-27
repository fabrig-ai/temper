//! The held-review control's owned forge and fixture-only handover.
use crate::fixtures;
pub(crate) const ADMIN_TOKEN: &[u8] = b"restart-admin-token";
const API_TOKEN: &[u8] = b"0000000000000000000000000000000000000002";
const GIT_TOKEN: &[u8] = b"0000000000000000000000000000000000000003";
pub(crate) fn table() -> skein_fake_forge_protocol::Config {
    let mut table = temper_fake_git_hosted::fixtures::table(b"acme/app", None);
    let mut tokens = skein_lib::List::with_capacity(5);
    for token in table.tokens.into_boxed() {
        tokens.push(token).expect("fixture tokens");
    }
    for (token, user) in [(ADMIN_TOKEN, 2), (API_TOKEN, 1), (GIT_TOKEN, 1)] {
        tokens.push((skein_lib::Secret::new(token.into()), user)).expect("restart tokens");
    }
    table.tokens = tokens;
    table
}
pub(crate) fn setup_peer(peer: &mut skein_fake_peers::forge::Peer) {
    let setup = peer.setup_mut();
    let worker = setup.account(b"worker", false, b"temper:bot");
    assert_eq!(worker, fixtures::WORKER_USER);
    // Match stories::forge's existing domain grant at the setup protocol face.
    setup
        .grant(fixtures::REPOSITORY, b"worker", skein_forge::forgejo::request::Grant::Write)
        .expect("fixture collaborator");
    let admin = setup.account(b"stranger", true, b"Restart site admin");
    setup.token(
        admin,
        b"admin",
        &[b"write:admin", b"read:user", b"write:organization", b"write:repository"],
        skein_lib::Secret::new(ADMIN_TOKEN.into()),
    );
}

pub(crate) fn checked_setup(bytes: &[u8]) -> temper_forge_setup::check::Checked {
    use jig_protocol_config as config;
    use temper_forge_setup::check::{self, Members};
    use temper_service::startup::{Transport, User};
    let directories = config::Directories::working();
    let given = crate::stories::given(&serde_json::from_slice(bytes).expect("deployment JSON"));
    let named = temper_service::startup::named_with(bytes, &directories, &given).expect("named restart deployment");
    let forges = named
        .forges
        .iter()
        .map(|forge| {
            let identity =
                named.identities.iter().find(|identity| identity.number == forge.identity).expect("identity");
            check::Forge {
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
            }
        })
        .collect();
    let value: serde_json::Value = serde_json::from_slice(bytes).expect("configuration JSON");
    let setup = serde_json::to_vec(&value["application"]["setup"]).expect("setup JSON");
    let size = u32::try_from(setup.len()).expect("setup size");
    let document = config::document(&setup, &config::DocumentLimits { bytes: size, tokens: size, depth: 32 })
        .expect("setup document");
    check::checked(
        &document,
        0,
        &directories,
        &Members { forges, listeners: Box::new([fixtures::CLIENTS]) },
        named.webhooks.map(|hooks| match hooks.listener {
            config::Address::Addr(a) => a.socket().expect("webhooks socket"),
            config::Address::Host { .. } => panic!("loopback fixture"),
        }),
        &named.overrides,
    )
    .expect("checked restart setup")
}
