//! GitHub stories share one forge and its authenticated git route (domain/testing.md, 2.4).
use crate::{fixtures, forge, landing};
use serde_json::{Value, json};
use skein_fake_forge_domain::{self as domain, api};
use skein_fake_forge_github::{
    self as layer,
    installations::{Kind, Reach},
};
use skein_fake_peers::forge::{Forge, Peer};
use skein_lib::{Duration, List, Secret, Wall};
use std::{cell::RefCell, collections::BTreeMap, net::SocketAddr, path::Path, rc::Rc};
use temper_github_app::APP;

pub const INSTALLATION: u64 = 1;
pub const REPOSITORY: u32 = 1;
const ACME: u64 = 10;
const UMBRA: u64 = 20;
// The layer retains at most this many git calls; a full ring fails the story.
const GIT_RECORDS: u32 = 128;

#[derive(Clone, Copy)]
pub struct Fixture {
    pub config: fn() -> layer::Config,
    pub configure: fn(&mut Value, &mut Value, SocketAddr),
}
#[must_use]
pub fn fixture() -> Fixture {
    Fixture { config, configure: |_, _, _| {} }
}
fn config() -> layer::Config {
    let mut accounts = List::with_capacity(2);
    let mut repositories = List::with_capacity(2);
    for (login, id, name, repository) in [("acme", ACME, "app", REPOSITORY), ("umbra", UMBRA, "tool", 2)] {
        accounts
            .push(layer::service::Account { login: login.as_bytes().into(), id, organization: true })
            .expect("two accounts");
        repositories
            .push(layer::service::Row {
                id: repository,
                owner: login.as_bytes().into(),
                owner_id: id,
                organization: true,
                name: name.as_bytes().into(),
                private: true,
            })
            .expect("two repositories");
    }
    let mut permissions = List::with_capacity(16);
    for (name, access) in skein_forge::github::setup::PERMISSIONS {
        permissions
            .push(skein_forge::github::document::Permission { name: (*name).into(), access })
            .expect("App permissions");
    }
    let mut events = List::with_capacity(16);
    for event in skein_forge::github::setup::EVENTS {
        events.push((*event).into()).expect("App events");
    }
    layer::Config {
        deliveries: layer::delivery::Config {
            log_min: Duration::from_millis(10),
            log_max: Duration::from_millis(50),
            ..layer::delivery::Config::standard()
        },
        rate_quota: 5000,
        rate_window: Duration::from_secs(3600),
        token_life: layer::installations::TOKEN_LIFE,
        base: b"http://127.0.0.1".as_slice().into(),
        blob: b"http://127.0.0.1".as_slice().into(),
        third_origin: None,
        blob_redirect: None,
        app: layer::app::App {
            id: APP.id,
            slug: APP.slug.as_bytes().into(),
            owner: ACME,
            permissions: permissions.into_boxed(),
            events: events.into_boxed(),
            key: layer::keys::KEYS[APP.key],
            webhook: Secret::new(b"fake-webhook".as_slice().into()),
        },
        app_facts: layer::service::AppFacts::Complete,
        bot: APP.bot,
        shown: layer::service::Shown::Ruleset,
        ruleset_id: 1,
        pull_requests_only: false,
        required_signatures: false,
        hook: b"https://temper-test.invalid/".as_slice().into(),
        repositories,
        accounts,
        person: b"acme".as_slice().into(),
        tokens: List::with_capacity(0),
        faults: layer::Faults { reset: 0, drop: 0, server_error: 0 },
    }
}
pub(crate) fn write(path: &Path, address: SocketAddr, clients: SocketAddr, fixture: Fixture) -> std::io::Result<()> {
    let mut value: Value = serde_json::from_slice(&std::fs::read(path.join("temper.json"))?)?;
    let mut agent: Value = serde_json::from_slice(&std::fs::read(path.join("agent.json"))?)?;
    value["application"]["clients"]["listener"] = json!({"socket":clients.to_string()});
    value["application"]["forges"] = json!([{"number":1,"kind":"github","api":{"socket":address.to_string()},
        "transport":"plaintext","git":format!("http://{address}"),"web":format!("http://{address}")}]);
    value["application"]["identities"] = json!([{"number":1,"forge":1,"login":null,"author":null}]);
    // Use the landing world's procedure and capacities, without a landing rule.
    (landing::fixture().configure)(&mut value, &mut agent, address);
    value["application"]["landing"]["projects"] = json!([]);
    value["application"]["limits"]["overrides"]["forge_component.log_poll_ms"] = 10.into();
    // Adoption must reach the forge for either account, rather than fail authority first.
    let broaden = |grants: &mut Value| {
        for grant in grants.as_array_mut().expect("grants") {
            grant["pattern"]["segments"] = json!(["forge"]);
            grant["pattern"]["value"] = address.to_string().into();
        }
    };
    broaden(&mut value["engine"]["rules"]["ceiling"]["grants"]);
    for project in value["engine"]["projects"].as_array_mut().expect("projects") {
        broaden(&mut project["policy"]["ceiling"]["grants"]);
        broaden(&mut project["policy"]["roles"][0]["authority"]["grants"]);
        broaden(&mut project["policy"]["projections"]);
    }
    (fixture.configure)(&mut value, &mut agent, address);
    fixtures::private(&path.join("temper.json"), &serde_json::to_vec(&value)?)?;
    fixtures::private(&path.join("agent.json"), &serde_json::to_vec(&agent)?)
}

#[derive(Debug)]
pub struct Branch {
    pub head: u64,
    pub tree: BTreeMap<Vec<u8>, Vec<u8>>,
}
#[derive(Default)]
pub struct Seen {
    pub git_calls: Vec<layer::GitCall>,
    /// Total successful mints; the pin exposes no per-installation mint counters.
    pub minted: u64,
    pub installations: Vec<(u64, u64, Vec<u8>)>,
    /// The git-call count when each recorded ctl call first completed.
    pub git_after_call: Vec<usize>,
    pub branches: BTreeMap<Vec<u8>, Branch>,
}
impl Seen {
    pub(crate) fn checkpoint(&mut self, peer: &Peer, calls: &[crate::calls::Called]) {
        let count = peer.git_calls().expect("GitHub git records").len();
        assert!(count < GIT_RECORDS, "git record ring never filled");
        while calls.get(self.git_after_call.len()).is_some_and(|call| call.exit.is_some()) {
            self.git_after_call.push(usize::try_from(count).expect("git count"));
        }
    }
    pub(crate) fn collect(&mut self, peer: &Peer, handle: &Rc<RefCell<Forge>>) {
        let layer = peer.github_layer().expect("GitHub layer");
        self.git_calls = layer.git_calls().iter().copied().collect();
        self.minted = layer.installations().minted();
        self.installations =
            layer.installations().iter().map(|row| (row.id, row.account, row.login.to_vec())).collect();
        let mut forge = handle.borrow_mut();
        while let Some(observation) = forge.domain.pop_observation() {
            if let domain::Observation::Moved { repository, branch, .. } = observation
                && repository.as_ref() == b"acme/app"
            {
                let api::Answer::Commit(head) = forge
                    .domain
                    .inspect(&forge.config, &repository, &api::Read::Branch { branch: branch.clone() })
                    .expect("pushed branch")
                else {
                    panic!("branch head")
                };
                let object = forge.domain.object(head).expect("pushed object");
                self.branches.insert(
                    branch.to_vec(),
                    Branch {
                        head,
                        tree: object.tree.iter().map(|(path, bytes)| (path.to_vec(), bytes.to_vec())).collect(),
                    },
                );
            }
        }
    }
}
pub(crate) struct State {
    pub(crate) handle: Rc<RefCell<Forge>>,
}
impl State {
    pub(crate) fn new(fixture: Fixture) -> (Self, Peer) {
        let mut config = temper_fake_git_hosted::fixtures::CONFIG;
        config.limits.calls = forge::protocol_limits().calls;
        config.limits.title_bytes = 512;
        config.limits.body_bytes = 2048;
        let clock = skein_shell::Clock::new().now();
        config.skew = domain::Skew::Ahead(Duration::from_nanos(
            clock.wall.as_nanos().checked_sub(clock.now.as_nanos()).expect("wall after monotonic"),
        ));
        let mut domain = domain::Domain::new(&config, fixtures::SEED);
        for (id, name) in [(REPOSITORY, b"acme/app".as_slice()), (2, b"umbra/tool".as_slice())] {
            let mut setup = (forge::fixture().setup)();
            setup.id = id;
            setup.name = name.into();
            setup.hooked = false;
            domain::repository(&mut domain, &config, setup);
        }
        domain::grant(&mut domain, b"acme/app", APP.bot, api::Permission::Admin);
        let handle = Rc::new(RefCell::new(Forge::new(domain, config)));
        let wire = forge::protocol_limits();
        let limits = layer::Limits {
            deliveries: layer::delivery::Limits { entries: 64, payload_bytes: wire.documents.body_bytes },
            http: wire.http,
            github: skein_forge::github::limits::Limits {
                json: wire.documents.json,
                writer: wire.documents.writer,
                string_bytes: wire.documents.string_bytes,
                body_bytes: wire.documents.body_bytes,
                inner: wire.documents.inner,
                target_bytes: wire.documents.target_bytes,
            },
            installations: layer::installations::Limits { installations: 8, tokens: 32, listed: 8 },
            pages: layer::pages::Limits { states: 8, manifest_bytes: 8192 },
            page: 32,
            body: wire.documents.body_bytes,
            response: wire.documents.body_bytes,
            blob_records: 16,
            git_records: GIT_RECORDS,
        };
        let mut peer = Peer::github(
            ([127, 0, 0, 1], 0).into(),
            ([127, 0, 0, 1], 0).into(),
            forge::peer_limits(),
            (fixture.config)(),
            limits,
            handle.clone(),
            fixtures::SEED,
        )
        .expect("GitHub peer");
        let id = peer
            .github_layer_mut()
            .expect("GitHub layer")
            .installations_mut()
            .install(ACME, b"acme", Kind::Organization, Reach::All, Wall::EPOCH)
            .expect("acme installed");
        assert_eq!(id, INSTALLATION, "layer's first installation");
        (Self { handle }, peer)
    }
}
