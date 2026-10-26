//! The pre-release golden story (protocol/README.md, 9; draft/upgrades.md, 13).
use crate::{
    deployment::Deployment,
    fixtures,
    referee::Store,
    world::{Proc, Story, World},
};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::{Observation, api};
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use skein_fake_person::{Find, Step};
use skein_ui::Role;

// Allocation order: the first chat, its tracked change, its producing agent,
// then the second chat. The first chat stays paused after its goal lands.
pub const CHAT: u64 = 1;
pub const GOAL: u64 = 2;
pub const PRODUCER: u64 = 3;
pub const MID_RUN: u64 = 4;
pub const PAUSED_WORDS: &[u8] = b"Golden chat paused";
// Two live chats share the existing policy allowance of 100.
pub const CHAT_ALLOWANCE: u64 = 50;
// A chat may reserve 40 for its own run; this leaves room to delegate
// while its first run is still in flight. The producer consumes two tokens.
pub(crate) const PRODUCER_ALLOWANCE: u64 = 8;
// The fixture has two original authentication tokens and three setup tokens.
const AUTHENTICATION_TOKENS: u32 = 5;
pub const ADMIN_TOKEN: &[u8] = b"golden-admin-token";
// The fake setup issuer gives the admin token 1, then makes API 2 and git 3.
pub const API_TOKEN: &[u8] = b"0000000000000000000000000000000000000002";
pub const GIT_TOKEN: &[u8] = b"0000000000000000000000000000000000000003";
pub const MID_RUN_WORDS: &[u8] = b"@midrun Keep working";

/// A completed API call, including refused attempts, without connection-local IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub user: u64,
    pub repository: Box<[u8]>,
    pub op: api::Op,
    pub result: Result<api::Answer, api::Error>,
}
/// The outside forge survives a service restart alongside this accumulated history.
/// `effects` includes git writes and CI; `calls` includes every completed HTTP call.
/// Hand this to `world::over_with_ledger`, then take the appended ledger from
/// `World::into_parts`. The backend is shared with that next phase, never rebuilt.
pub struct Ledger {
    pub effects: Vec<Observation>,
    pub calls: Vec<Call>,
    /// Sanitized HTTP heads also cover setup writes that stay outside the domain.
    pub requests: Vec<skein_http::server::Call>,
    pub(crate) story: Story,
    pub(crate) transport: temper_fake_git_hosted::Typed,
}
impl std::fmt::Debug for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ledger")
            .field("effects", &self.effects)
            .field("calls", &self.calls)
            .field("requests", &self.requests)
            .finish_non_exhaustive()
    }
}
impl Ledger {
    pub(crate) fn new(transport: temper_fake_git_hosted::Typed, story: Story) -> Self {
        Self { effects: Vec::new(), calls: Vec::new(), requests: Vec::new(), transport, story }
    }
    /// Read the retained forge without making another call or recording an effect.
    ///
    /// # Errors
    /// Returns the fake forge's refusal for an unknown repository or unreadable item.
    pub fn inspect(&self, repository: &[u8], read: &api::Read) -> Result<api::Answer, api::Error> {
        let forge = self.transport.forge.borrow();
        forge.domain.inspect(&forge.config, repository, read)
    }
    pub(crate) fn observe(&mut self, procs: &[Proc]) {
        for proc in procs {
            let Proc::Forge(peer) = proc else { continue };
            for record in peer.observations() {
                if let skein_fake_peers::forge::Observation::Head { call, .. } = record {
                    self.requests.push(call.clone());
                }
                if let skein_fake_peers::forge::Observation::Call { user, repository, op, result, .. } = record {
                    self.calls.push(Call {
                        user: *user,
                        repository: repository.clone(),
                        op: op.clone(),
                        result: result.clone(),
                    });
                }
            }
        }
    }
}

/// Run with fixed seeds until the second chat's first turn is durable and its
/// run is still in flight, then perform the same settled drain as `drain_mid_run`.
/// At v0.1.0 the tag keeps this result; later tests read that kept input.
#[must_use]
pub fn golden() -> (Deployment, Ledger) {
    World::run(fixtures::calm(fixtures::SEED), Story::Golden).into_parts()
}

pub(crate) fn person() -> Vec<Step<temper_client_domain::Address>> {
    let mut steps = crate::stories::person();
    steps.extend([
        Step::See { find: Find::text(PAUSED_WORDS), within: fixtures::PERSON_WAIT },
        Step::See { find: Find::named(Role::Status, b"waiting on your words"), within: fixtures::PERSON_WAIT },
        Step::Go {
            address: temper_client_domain::Address::Chats {
                filter: temper_client_domain::ChatsFilter::All,
                start: false,
            },
        },
        Step::See { find: Find::named(Role::TextArea, b"Start a chat"), within: fixtures::PERSON_WAIT },
        Step::Type { find: Find::named(Role::TextArea, b"Start a chat"), words: MID_RUN_WORDS.into() },
        Step::Press { find: Find::named(Role::Button, b"Start") },
        Step::See { find: Find::text(MID_RUN_WORDS), within: fixtures::PERSON_WAIT },
    ]);
    steps
}

pub(crate) fn scripts(scripts: &mut Vec<Script>) {
    let planner = scripts.iter_mut().find(|script| script.cue.as_ref() == b"@plan").expect("planner script");
    // Speak and wait, leaving the chat's turns intact while the person starts independent work.
    planner.turns[2] = Turn {
        lines: Box::new([
            Line::Text { text: PAUSED_WORDS.into() },
            Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: fixtures::CALL_TOKENS,
    };
    scripts.push(Script {
        cue: b"@midrun".as_slice().into(),
        turns: Box::new([
            Turn {
                lines: Box::new([Line::Call {
                    name: b"read".as_slice().into(),
                    arguments: br#"{"path":"conflict.txt"}"#.as_slice().into(),
                }]),
                finish: Finish::ToolCalls,
                tokens: fixtures::CALL_TOKENS,
            },
            Turn {
                lines: Box::new([Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() }]),
                finish: Finish::ToolCalls,
                tokens: fixtures::CALL_TOKENS,
            },
        ]),
    });
}

pub(crate) fn complete(store: &Store) -> bool {
    let paused = store.rows.values().any(|row| {
        matches!(row,
            temper_engine_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
                if task.number == CHAT && task.phase == tasks::Phase::Active(tasks::Active::Idle)
        )
    });
    let running = store.rows.values().any(|row| {
        matches!(row,
            temper_engine_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
                if task.number == MID_RUN && matches!(task.phase, tasks::Phase::Active(tasks::Active::Running { .. }))
        )
    });
    let committed = store.rows.values().any(|row| {
        matches!(row,
            temper_engine_domain::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn)))
                if turn.task == MID_RUN && turn.turn == 1
        )
    });
    let landed = store.rows.values().any(|row| {
        matches!(row,
            temper_engine_domain::Record::Forge { row }
                if matches!(row.as_ref(), temper_engine_domain_forge::Stored::Landed { task: GOAL, .. })
        )
    });
    paused && running && committed && landed
}

pub(crate) fn configure(value: &mut serde_json::Value) {
    let forge = value["application"]["forges"][0].as_object_mut().expect("forge");
    for name in ["api_token", "user", "webhook_secret", "identity"] {
        forge.remove(name);
    }
    let identity = value["application"]["identities"][0].as_object_mut().expect("identity");
    identity.remove("token");
    identity.insert("forge".into(), 1.into());
    value["application"]["setup"] = serde_json::json!([
        {"forge":1,"bot":{"login":"worker","email":"worker@example.invalid"},"mode":"managed"}
    ]);
}
pub(crate) fn table() -> skein_fake_forge_protocol::Config {
    let mut table = temper_fake_git_hosted::fixtures::table(b"acme/app", None);
    let mut tokens = skein_lib::List::with_capacity(AUTHENTICATION_TOKENS);
    for token in table.tokens.into_boxed() {
        tokens.push(token).expect("fixture tokens");
    }
    for (token, user) in [(ADMIN_TOKEN, 2), (API_TOKEN, 1), (GIT_TOKEN, 1)] {
        tokens.push((skein_lib::Secret::new(token.into()), user)).expect("golden tokens");
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
    let admin = setup.account(b"stranger", true, b"Golden site admin");
    setup.token(
        admin,
        b"admin",
        &[b"write:admin", b"read:user", b"write:organization", b"write:repository"],
        skein_lib::Secret::new(ADMIN_TOKEN.into()),
    );
}

// The shell normally hands this checked setup to startup::with_setup. The
// earlier stories use transitional file credentials and need no setup.
pub(crate) fn checked_setup(bytes: &[u8]) -> temper_forge_setup::check::Checked {
    use jig_protocol_config as config;
    use temper_forge_setup::check::{self, Members};
    use temper_service::startup::{Transport, User};
    let directories = config::Directories::working();
    let value: serde_json::Value = serde_json::from_slice(bytes).expect("configuration JSON");
    let given = crate::stories::given(&value);
    let named = temper_service::startup::named_with(bytes, &directories, &given).expect("named golden deployment");
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
    .expect("checked golden setup")
}
