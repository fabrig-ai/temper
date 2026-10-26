//! One loopback GitHub for manifest history and the welcome's installation/adoption chain (client/browser.md, 4.4).
use skein_fake_forge_domain as domain;
use skein_fake_forge_github as layer;
use skein_fake_peers::forge::{Forge, Peer};
use skein_lib::{Duration, List, Secret};
use std::{cell::RefCell, net::SocketAddr, rc::Rc};

// One Chromium keep-alive, temper's forge connection, the setup client's
// next request and its previous connection until that close completes.
// EOF and the next Accept can arrive together, before the old slot is reclaimed.
const CONNECTIONS: u32 = 1 + 1 + 2;
// The forge's Full and Contents tokens, plus the setup client's picker token.
const INSTALLATION_TOKENS: u32 = 3;
// Browser navigation (3), connect/conversion (4), reconcile/accept (5),
// forge token/identity reads (4), picker (2), and adoption's seven read stages.
// Each adoption stage may use two HTTP requests (metadata/protection or pagination).
const OBSERVATIONS: u32 = 3 + 4 + 5 + 4 + 2 + 2 * 7;
const SEED: u64 = 23;
const ACCOUNT: u64 = 10;
const BOT: u64 = 11;
const REPOSITORY: u32 = 1;
const FULL_NAME: &[u8] = b"acme/app";

#[cfg(test)]
mod tests;

pub(super) fn peer(address: SocketAddr) -> Peer {
    let profile = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small);
    let config = domain_config();
    let mut domain = domain::Domain::new(&config, SEED);
    seed_repository(&mut domain, &config);
    let forge = Rc::new(RefCell::new(Forge::new(domain, config)));
    let config = github_config();
    let http = skein_http::server::Limits { body: 32768, response: 32768, ..profile.driver.server };
    // Each retained head owns at most its wire-head bytes and its Header array.
    let observation_bytes = http
        .head
        .checked_add(
            http.headers
                .checked_mul(u32::try_from(core::mem::size_of::<skein_http::Header>()).expect("header size"))
                .expect("header array"),
        )
        .expect("one retained head")
        .checked_mul(OBSERVATIONS)
        .expect("retained request heads");
    let limits = layer::Limits {
        deliveries: layer::delivery::Limits { entries: 1, payload_bytes: 32768 },
        http,
        github: skein_forge::github::limits::Limits {
            json: skein_json::tokenizer::Limits { chunk: 1024, string: 32768, number: 32, depth: 16, length: 32768 },
            writer: skein_json::writer::Limits { depth: 16, length: 32768 },
            string_bytes: 1024,
            body_bytes: 32768,
            inner: 32,
            target_bytes: 4096,
        },
        installations: layer::installations::Limits { installations: 1, tokens: INSTALLATION_TOKENS, listed: 1 },
        pages: layer::pages::Limits { states: 1, manifest_bytes: 8192 },
        page: 1,
        body: 32768,
        response: 32768,
        blob_records: 1,
        git_records: 1,
    };
    Peer::github(
        address,
        ([127, 0, 0, 1], 0).into(),
        skein_fake_peers::Limits {
            io: profile.io,
            connections: CONNECTIONS,
            queue: 64,
            plaintext: 65536,
            ciphertext: 65536,
            observations: OBSERVATIONS,
            observation_bytes,
        },
        config,
        limits,
        forge,
        SEED,
    )
    .expect("loopback GitHub peer")
}

fn github_config() -> layer::Config {
    let mut accounts = List::with_capacity(1);
    accounts
        .push(layer::service::Account { login: b"acme".as_slice().into(), id: ACCOUNT, organization: false })
        .expect("one account");
    let mut repositories = List::with_capacity(1);
    repositories
        .push(layer::service::Row {
            id: REPOSITORY,
            owner: b"acme".as_slice().into(),
            owner_id: ACCOUNT,
            organization: false,
            name: b"app".as_slice().into(),
            private: true,
        })
        .expect("one seeded repository");
    layer::Config {
        deliveries: layer::delivery::Config { log_entries: 1, ..layer::delivery::Config::standard() },
        rate_quota: 5000,
        rate_window: Duration::from_secs(3600),
        token_life: layer::installations::TOKEN_LIFE,
        base: b"http://127.0.0.1".as_slice().into(),
        blob: b"http://127.0.0.1".as_slice().into(),
        third_origin: None,
        blob_redirect: None,
        app: layer::app::App {
            id: 42,
            slug: b"unused".as_slice().into(),
            owner: ACCOUNT,
            permissions: Box::new([]),
            events: Box::new([]),
            key: layer::keys::KEYS[0],
            webhook: Secret::new(b"fake-hook".as_slice().into()),
        },
        app_facts: layer::service::AppFacts::Complete,
        bot: BOT,
        shown: layer::service::Shown::Ruleset,
        ruleset_id: 1,
        pull_requests_only: false,
        required_signatures: false,
        hook: b"https://unused.invalid/".as_slice().into(),
        repositories,
        accounts,
        person: b"acme".as_slice().into(),
        tokens: List::with_capacity(0),
        faults: layer::Faults { reset: 0, drop: 0, server_error: 0 },
    }
}

/// The welcome creates its first adopted repository's project under policy 1.
pub(super) fn project_creation(value: &mut serde_json::Value, address: SocketAddr) {
    // Deployment 0, configured project 1 and one new project; two funders per working project.
    // A forge branch needs eight authority segments; adoption has two ids and seven options.
    const PROJECTS: u32 = 3;
    const FUNDERS: u32 = 4;
    const ADOPTION_PARTS: usize = 2 + 7;
    const BRANCH_SEGMENTS: u32 = 8;

    value["engine"]["limits"]["overrides"]["people.projects"] = PROJECTS.into();
    value["engine"]["limits"]["overrides"]["authority.projects"] = PROJECTS.into();
    value["application"]["limits"]["overrides"]["forge.judge_projects"] = PROJECTS.into();
    value["engine"]["limits"]["overrides"]["tasks.funders"] = FUNDERS.into();
    // Two resource ids and seven adoption options, as adopt_repository_ask builds them.
    // The options own host, owner, repository, branch prefix, landing, and two flags.
    let fixed = core::mem::size_of::<u16>()
        + core::mem::size_of::<u32>()
        + b"acme".len()
        + b"app".len()
        + b"temper/".len()
        + b"main".len()
        + 2;
    let amendment_bytes =
        ADOPTION_PARTS * core::mem::size_of::<Box<[u8]>>() + fixed + format!("http://{address}").len();
    value["engine"]["limits"]["overrides"]["people.amendment_bytes"] =
        u32::try_from(amendment_bytes).expect("one adoption").into();
    // forge/host/owner/repository/branch, plus temper/<root>/<task>.
    value["engine"]["limits"]["overrides"]["authority.segments"] = BRANCH_SEGMENTS.into();
}

// The picker and adoption both read this repository through the GitHub layer.
fn seed_repository(domain: &mut domain::Domain, config: &domain::Config) {
    domain::repository(
        domain,
        config,
        domain::api::Setup {
            id: REPOSITORY,
            name: FULL_NAME.into(),
            default: b"main".as_slice().into(),
            tree: Box::new([]),
            labels: Box::new([]),
            checks: domain::api::Checks {
                contexts: Box::new([]),
                latency_min: Duration::ZERO,
                latency_max: Duration::ZERO,
                silent: 0,
                passes: 1000,
                reruns: 0,
                cue: None,
            },
            protection: None,
            hooked: false,
        },
    );
    domain::grant(domain, FULL_NAME, BOT, domain::api::Permission::Admin);
}

// Adoption reads metadata and protection, without changing repository contents.
fn domain_config() -> domain::Config {
    domain::Config {
        limits: domain::Limits {
            repositories: 1,
            users: 4,
            labels: 1,
            items: 1,
            comments: 1,
            dependencies: 1,
            reviews: 1,
            branches: 1,
            commits: 1,
            files: 1,
            statuses: 1,
            contexts: 1,
            bypassing_users: 0,
            pages: 1,
            name_bytes: 256,
            title_bytes: 16,
            body_bytes: 32,
            content_bytes: 32,
            page_size: 1,
            calls: 1,
            hooks: 1,
            observations: 16,
        },
        latency_min: Duration::ZERO,
        latency_max: Duration::ZERO,
        settle_min: Duration::from_secs(2),
        settle_max: Duration::from_secs(2),
        late: 0,
        late_min: Duration::ZERO,
        late_max: Duration::ZERO,
        unavailable: 0,
        timeouts: 0,
        landing: 0,
        land_min: Duration::ZERO,
        land_max: Duration::ZERO,
        rate_limit: 0,
        rate_window: Duration::ZERO,
        ci: 12,
        hook_min: Duration::ZERO,
        hook_max: Duration::ZERO,
        hooks_late: 0,
        hooks_lost: 0,
        resolution: Duration::ZERO,
        skew: domain::Skew::None,
        status_updates: false,
        edit_updates: false,
    }
}
