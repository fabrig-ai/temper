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
pub const OLD_STRAY_AGE_MS: u64 = 2000;
pub const REPOSITORY: u32 = 1;
const ACME: u64 = 10;
const UMBRA: u64 = 20;
// The layer retains at most this many git calls; a full ring fails the story.
const GIT_RECORDS: u32 = 128;
const WIRE_BYTES: u32 = 32_768;

// A whole bounded tree, git's JSON/base64 framing and the App's documents
// fit 32 KiB. State::new checks this against the domain's file bounds.
pub(crate) fn protocol_limits() -> skein_fake_forge_protocol::Limits {
    let mut limits = forge::protocol_limits();
    limits.http.body = u64::from(WIRE_BYTES);
    limits.documents.json.string = WIRE_BYTES;
    limits.documents.json.length = WIRE_BYTES;
    limits.documents.writer.length = WIRE_BYTES;
    limits.documents.body_bytes = WIRE_BYTES;
    limits
}

#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "independent switches select the fake GitHub setup scenario")]
pub struct Fixture {
    /// Seeded App records and acme installation; false exercises a person connecting.
    pub connected: bool,
    /// Connect through the browser instead of preparing a pasted App.
    pub manifest: bool,
    /// Seed an unaccepted umbra installation before the person's paste.
    pub old_stray: bool,
    /// Add an unaccepted account once the browser has installed acme.
    pub unaccepted_after_install: bool,
    /// The person's App hook, which paste and reconcile may need to repair.
    pub pasted_hook: &'static [u8],
    /// Repository contents, CI and notifications; the world supplies ids and names.
    pub setup: fn() -> api::Setup,
    /// Fake timings and history capacity, before the domain allocates them.
    pub configure_domain: fn(&mut domain::Config),
    /// Seed repository history before the peer starts; setup itself starts no CI.
    pub prepare: fn(&mut domain::Domain, &skein_lib::Env<domain::Config>),
    pub config: fn() -> layer::Config,
    pub configure: fn(&mut Value, &mut Value, SocketAddr),
}
#[must_use]
pub fn fixture() -> Fixture {
    Fixture {
        connected: true,
        manifest: false,
        old_stray: false,
        unaccepted_after_install: false,
        pasted_hook: b"https://temper-test.invalid/",
        setup,
        configure_domain: |_| {},
        prepare: |_, _| {},
        config,
        configure: |_, _, _| {},
    }
}
fn setup() -> api::Setup {
    let mut setup = (forge::fixture().setup)();
    setup.hooked = false;
    setup
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
    fixtures::turn_bounds::apply(&mut value, fixtures::turn_bounds::turn_body(&agent));
    fixtures::private(&path.join("temper.json"), &serde_json::to_vec(&value)?)?;
    fixtures::private(&path.join("agent.json"), &serde_json::to_vec(&agent)?)
}

#[derive(Debug)]
pub struct Branch {
    pub head: u64,
    pub tree: BTreeMap<Vec<u8>, Vec<u8>>,
}
/// The App returned to the person by the fake's conversion, never a real key.
pub struct MadeApp {
    pub id: u64,
    pub pem: Secret,
}

#[derive(Default)]
pub struct Seen {
    observations: Vec<domain::Observation>,
    pub app: Option<MadeApp>,
    pub git_calls: Vec<layer::GitCall>,
    /// Total successful mints; the pin exposes no per-installation mint counters.
    pub minted: u64,
    pub unaccepted: Option<u64>,
    pub installations: Vec<(u64, u64, Vec<u8>)>,
    /// App request methods and targets from the peer's redacted HTTP heads.
    pub app_calls: Vec<(skein_http::Method, Vec<u8>)>,
    pub installation_created: Vec<(u64, Wall)>,
    /// The git-call count when each recorded ctl call first completed.
    pub git_after_call: Vec<usize>,
    pub branches: BTreeMap<Vec<u8>, Branch>,
    /// Pull creation and merging on acme/app, retained in the layer's order.
    pub pulls: Vec<domain::Observation>,
}
impl Seen {
    /// Domain records in order, including CI reports at each pushed head.
    #[must_use]
    pub fn observations(&self) -> &[domain::Observation] {
        &self.observations
    }

    pub(crate) fn seed_unaccepted(&mut self, peer: &mut Peer) {
        if self.unaccepted.is_some() {
            return;
        }
        let layer = peer.github_layer_mut().expect("GitHub layer");
        if !layer.installations().iter().any(|row| row.account == ACME) {
            return;
        }
        let id = layer
            .installations_mut()
            .install(UMBRA, b"umbra", Kind::Organization, Reach::All, Wall::EPOCH)
            .expect("unaccepted installation beside acme");
        self.unaccepted = Some(id);
    }

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
        self.installation_created = layer.installations().iter().map(|row| (row.id, row.created)).collect();
        self.app_calls = peer
            .observations()
            .iter()
            .filter_map(|record| {
                let skein_fake_peers::forge::Observation::Head { call, .. } = record else { return None };
                (call.target.as_ref() == b"/app" || call.target.starts_with(b"/app/"))
                    .then(|| (call.method, call.target.to_vec()))
            })
            .collect();
        let mut forge = handle.borrow_mut();
        while let Some(observation) = forge.domain.pop_observation() {
            self.observations.push(observation.clone());
            if matches!(&observation,
                domain::Observation::Opened { repository, .. } | domain::Observation::Merged { repository, .. }
                if repository.as_ref() == b"acme/app")
            {
                self.pulls.push(observation);
                continue;
            }
            if let domain::Observation::Moved { repository, branch, to: head, .. } = observation
                && repository.as_ref() == b"acme/app"
            {
                // A landed topic branch may already be deleted. Its move records
                // the pushed object, which remains available after that cleanup.
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
        (fixture.configure_domain)(&mut config);
        // These smoke repositories have two small files each. The GitHub
        // layer pads every API response to its admitted wire capacity, so
        // do not inherit the checkout sweep's 256 files of 64 KiB each.
        config.limits.files = 4;
        config.limits.content_bytes = 1024;
        config.limits.calls = protocol_limits().calls;
        config.limits.title_bytes = 512;
        config.limits.body_bytes = 2048;
        assert!(skein_fake_forge_protocol::git::fits(&protocol_limits(), &config.limits), "bounded git trees fit");
        let clock = skein_shell::Clock::new().now();
        config.skew = domain::Skew::Ahead(Duration::from_nanos(
            clock.wall.as_nanos().checked_sub(clock.now.as_nanos()).expect("wall after monotonic"),
        ));
        let mut domain = domain::Domain::new(&config, fixtures::SEED);
        for (id, name) in [(REPOSITORY, b"acme/app".as_slice()), (2, b"umbra/tool".as_slice())] {
            let mut setup = (fixture.setup)();
            setup.id = id;
            setup.name = name.into();
            domain::repository(&mut domain, &config, setup);
        }
        domain::grant(&mut domain, b"acme/app", APP.bot, api::Permission::Admin);
        let env = skein_lib::Env { now: clock.now, wall: clock.wall, limits: config };
        (fixture.prepare)(&mut domain, &env);
        let handle = Rc::new(RefCell::new(Forge::new(domain, env.limits)));
        let wire = protocol_limits();
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
        let mut hosted = forge::peer_limits();
        // The four call connections share this peer with the component's
        // reserved App/delivery connection, setup's client and hosted git.
        // A call racing a delivery poll must not be refused by the fixture.
        hosted.connections = wire.calls + 3;
        let mut peer = Peer::github(
            ([127, 0, 0, 1], 0).into(),
            ([127, 0, 0, 1], 0).into(),
            hosted,
            (fixture.config)(),
            limits,
            handle.clone(),
            fixtures::SEED,
        )
        .expect("GitHub peer");
        if fixture.connected {
            let id = peer
                .github_layer_mut()
                .expect("GitHub layer")
                .installations_mut()
                .install(ACME, b"acme", Kind::Organization, Reach::All, Wall::EPOCH)
                .expect("acme installed");
            assert_eq!(id, INSTALLATION, "layer's first installation");
        }
        (Self { handle }, peer)
    }
}

/// Stand in for the person's developer-settings work, before any ctl call.
pub(crate) fn make_app(peer: &mut Peer, clients: SocketAddr, path: &Path, fixture: Fixture) -> MadeApp {
    use skein_forge::github::{pin, request::Method, setup};
    use skein_lib::{ReplyTo, Token};
    use std::fmt::Write as _;
    let origin = format!("http://{clients}");
    let setup_url = format!("{origin}/forges/1/github/setup");
    let redirect = format!("{origin}/forges/1/github/redirect");
    let manifest = setup::manifest(
        &setup::Values {
            name: b"temper-person",
            url: origin.as_bytes(),
            hook_url: fixture.pasted_hook,
            redirect_url: redirect.as_bytes(),
            setup_url: setup_url.as_bytes(),
        },
        &protocol_limits().documents.writer,
    )
    .expect("temper manifest");
    // Percent-encode the form value, including JSON punctuation and URL queries.
    let mut form = String::from("manifest=");
    for byte in &manifest {
        write!(form, "%{byte:02X}").expect("form byte");
    }
    let layer = peer.github_layer_mut().expect("GitHub layer");
    assert_eq!(layer.installations().iter().count(), 0, "person starts with no installation");
    let now = skein_shell::Clock::new().now().wall;
    let page = layer.page(
        &layer::pages::Page { method: Method::Post, target: b"/settings/apps/new?state=person", body: form.as_bytes() },
        now,
    );
    assert_eq!(page.status, 200, "developer settings accepts temper's manifest");
    let html = std::str::from_utf8(&page.body).expect("creation page");
    let code = html.split_once("?code=").expect("conversion link").1.split('&').next().expect("code");
    let target = format!("/app-manifests/{code}/conversions");
    let layer::service::Outcome::Reply(reply) = layer.request(
        &layer::service::Request {
            validator: None,
            method: Method::Post,
            target: target.as_bytes(),
            authorization: None,
            version: Some(pin::API_VERSION.as_bytes()),
            agent: true,
            body: b"",
        },
        now,
        ReplyTo::new(Token::new(1)),
    ) else {
        panic!("conversion replies without domain I/O")
    };
    assert_eq!(reply.status, 201, "fake converts the person's App");
    let value: Value = serde_json::from_slice(&reply.body).expect("conversion JSON");
    let app = MadeApp {
        id: value["id"].as_u64().expect("converted App id"),
        pem: Secret::new(value["pem"].as_str().expect("converted PEM").as_bytes().into()),
    };
    fixtures::private(&path.join("github-app.pem"), app.pem.expose()).expect("private downloaded key");
    if fixture.old_stray {
        let created = Wall::from_nanos(
            now.as_nanos().checked_sub(Duration::from_millis(OLD_STRAY_AGE_MS).as_nanos()).expect("old stray time"),
        );
        let id = layer
            .installations_mut()
            .install(UMBRA, b"umbra", Kind::Organization, Reach::All, created)
            .expect("old stray installed");
        assert_eq!(id, INSTALLATION, "stray precedes the person's accepted installation");
    }
    app
}
