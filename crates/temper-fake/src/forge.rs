//! The QA forge's document, fixed seed and setup acts (protocol/setup.md, 8.4).
pub mod github;
pub mod observations;
use crate::{Refusal, read};
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::Path;
use skein_fake_forge_domain::{self as domain, api};
use skein_fake_forge_protocol as forgejo;
use skein_fake_peers::forge::Forge;
use skein_forge_setup::given::login;
use skein_lib::{Duration, List, Secret, bytes};
use temper_forge_setup::name::{branch, relative, repository};

/// Setup increments its account number before insertion, so it cannot issue MAX.
pub const OWNER: u64 = u64::MAX;
/// The one named check, with no delay, silence or rerun.
pub const CONTEXT: &[u8] = b"check";
/// A first commit has an empty message in skein's repository constructor.
pub const MESSAGE: &[u8] = b"";
/// The fixed tree describes the check; the fake never runs its workflow.
pub const FILES: [(&[u8], &[u8]); 3] = [
    (b"README.md", b"# Widget\n\nA small repository for temper's QA loop.\n"),
    (b"src/main.rs", b"const TABLE: [(&str, u32); 3] = [(\"apples\", 3), (\"pears\", 5), (\"plums\", 2)];\n\nfn main() {\n    for (name, count) in TABLE {\n        println!(\"{name:<8}{count}\");\n    }\n}\n"),
    (b".forgejo/workflows/check.yml", b"name: check\non: [push, pull_request]\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo 'The fake judges the configured CI cue; this workflow is not run.'\n"),
];
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ci {
    pub path: Box<[u8]>,
    pub green: Box<[u8]>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Repository {
    pub id: Option<u32>,
    pub tree: Option<Box<[api::File]>>,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub landing: Box<[u8]>,
    pub ci: Option<Ci>,
}
/// The checked standalone forge document; it reads no deployment or bot identity.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Document {
    listen: SocketAddr,
    admin_token: Box<[u8]>,
    owner: Box<[u8]>,
    repositories: Box<[Repository]>,
    github: Option<github::Github>,
}
impl Document {
    /// The fake forge of a checked setup entry, its token at `admin_token` (protocol/setup.md, 8.4).
    #[must_use]
    pub fn fake(fake: &temper_forge_setup::setup::Fake, admin_token: &[u8]) -> Self {
        let mut repositories =
            List::with_capacity(u32::try_from(fake.repositories.len()).expect("checked repositories"));
        for repository in &fake.repositories {
            repositories
                .push(Repository {
                    id: None,
                    tree: None,
                    owner: fake.owner.clone(),
                    name: repository.name.clone(),
                    landing: repository.landing.clone(),
                    ci: None,
                })
                .expect("checked repositories");
        }
        Self {
            listen: fake.listen,
            admin_token: bytes::copy_of(admin_token),
            owner: fake.owner.clone(),
            repositories: repositories.into_boxed(),
            github: None,
        }
    }
    /// The seeded App's public test private key, for the fake box's own-App path.
    #[must_use]
    pub fn github_app_key(&self) -> Option<Box<[u8]>> {
        let github = self.github.as_ref()?;
        Some(github.pem())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        let d = read::document(bytes)?;
        let p = Path::root();
        let mut o = read::object(&d, 0, &p)?;
        let listen_at = read::required(&mut o, &d, b"listen", &p)?;
        let admin_token = read::field(&mut o, &d, b"admin_token", crate::limits::PATH_BYTES, &p)?;
        let owner = read::field(&mut o, &d, b"owner", 40, &p)?;
        let repositories_at = read::required(&mut o, &d, b"repositories", &p)?;
        let github = match o.member(&d, b"github") {
            Some(at) => Some(github::decode(&d, at, &p.member(b"github"))?),
            None => None,
        };
        read::finish(o, &d, &p)?;
        let listen = read::listen(&d, listen_at, &p.member(b"listen"))?;
        read::absolute(&admin_token, &p.member(b"admin_token"))?;
        if !login(&owner) || owner.as_ref() == b"temper-admin" {
            return Err(read::why(&p.member(b"owner"), b"expected a Forgejo login other than temper-admin"));
        }
        let p = p.member(b"repositories");
        let items = read::list(&d, repositories_at, 1, &p)?;
        let mut repositories = List::<Repository>::with_capacity(items.len());
        for (i, at) in items.iter().enumerate() {
            let p = read::index(&p, i);
            let mut o = read::object(&d, *at, &p)?;
            let id = match o.member(&d, b"id") {
                Some(at) => {
                    let id = read::integer(&d, at, u64::from(u32::MAX), &p.member(b"id"))?;
                    if id == 0 {
                        return Err(read::why(&p.member(b"id"), b"must be positive"));
                    }
                    Some(u32::try_from(id).expect("bounded id"))
                }
                None => None,
            };
            let tree = match o.member(&d, b"tree") {
                Some(at) => Some(tree(&d, at, &p.member(b"tree"))?),
                None => None,
            };
            let name = read::field(&mut o, &d, b"name", 100, &p)?;
            let landing = read::field(&mut o, &d, b"landing", LIMITS.limits.name_bytes, &p)?;
            let repository_owner = if github.is_some() {
                match o.member(&d, b"owner") {
                    Some(at) => read::text(&d, at, 39, &p.member(b"owner"))?,
                    None => owner.clone(),
                }
            } else {
                owner.clone()
            };
            let ci_at = read::required(&mut o, &d, b"ci", &p)?;
            read::finish(o, &d, &p)?;
            if !repository(&name) {
                return Err(read::why(&p.member(b"name"), b"expected a Forgejo repository name"));
            }
            if !branch(&landing) {
                return Err(read::why(&p.member(b"landing"), b"expected a branch name"));
            }
            for prev in &repositories {
                if prev.id.is_some() != id.is_some() {
                    return Err(read::why(&p.member(b"id"), b"every repository must give an id, or none"));
                }
                if id.is_some() && prev.id == id {
                    return Err(read::why(&p.member(b"id"), b"must be unique"));
                }
                if prev.name == name && prev.owner == repository_owner {
                    return Err(read::why(&p.member(b"name"), b"must be unique"));
                }
            }
            let ci = if read::is_null(&d, ci_at) {
                None
            } else {
                let p = p.member(b"ci");
                let mut o = read::object(&d, ci_at, &p)?;
                let path = read::field(&mut o, &d, b"path", LIMITS.limits.name_bytes, &p)?;
                let green = read::field(&mut o, &d, b"green", LIMITS.limits.content_bytes, &p)?;
                read::finish(o, &d, &p)?;
                if !relative(&path) {
                    return Err(read::why(
                        &p.member(b"path"),
                        b"expected a relative file path with no . or .. component",
                    ));
                }
                for (seed, _) in FILES {
                    if tree.is_none() && path.as_ref() == seed {
                        return Err(read::why(&p.member(b"path"), b"must not name a seed file"));
                    }
                }
                read::nonempty(&green, &p.member(b"green"))?;
                Some(Ci { path, green })
            };
            repositories
                .push(Repository { id, tree, owner: repository_owner, name, landing, ci })
                .expect("eight repositories");
        }
        if let Some(github) = &github {
            github.check(repositories.as_slice())?;
        }
        Ok(Self { listen, admin_token, owner, repositories: repositories.into_boxed(), github })
    }
    #[must_use]
    pub fn listen(&self) -> SocketAddr {
        self.listen
    }
    #[must_use]
    pub fn admin_token(&self) -> &[u8] {
        &self.admin_token
    }
    #[must_use]
    pub fn owner(&self) -> &[u8] {
        &self.owner
    }
    #[must_use]
    pub fn repositories(&self) -> &[Repository] {
        &self.repositories
    }
    /// The same seed order gives the same commit at every start: skein stores
    /// tree, message and parents, with no author or time; its id is a count.
    /// No grant is seeded. Seed observations are drained, so a file holds the run alone.
    #[must_use]
    pub fn seed(&self, seed: u64) -> (Forge, forgejo::Config) {
        let mut domain = domain::Domain::new(&LIMITS, seed);
        let mut repositories = List::with_capacity(crate::ENTRIES);
        for (i, repository) in self.repositories.iter().enumerate() {
            let id = repository
                .id
                .unwrap_or(u32::try_from(i).expect("eight repositories").checked_add(1).expect("numbered from one"));
            let full = read::joined(&[&repository.owner, b"/", &repository.name]);
            let tree = match &repository.tree {
                Some(tree) => tree.clone(),
                None => {
                    let mut tree = List::with_capacity(3);
                    for (path, content) in FILES {
                        tree.push(api::File { path: bytes::copy_of(path), content: bytes::copy_of(content) })
                            .expect("three seed files");
                    }
                    tree.into_boxed()
                }
            };
            domain::repository(
                &mut domain,
                &LIMITS,
                api::Setup {
                    id,
                    name: full.clone(),
                    default: repository.landing.clone(),
                    tree,
                    labels: Box::new([]),
                    checks: checks(repository.ci.as_ref()),
                    protection: None,
                    hooked: true,
                },
            );
            if let Some(github) = &self.github {
                domain::grant(&mut domain, &full, github.bot(), api::Permission::Admin);
            }
            repositories.push(forgejo::Repository { id, full, protected: None }).expect("eight repositories");
        }
        for _ in 0..LIMITS.limits.observations {
            if domain.pop_observation().is_none() {
                break;
            }
        }
        let mut users = List::with_capacity(1);
        users.push((OWNER, self.owner.clone())).expect("one domain owner");
        let config = forgejo::Config {
            base: bytes::copy_of(b"http://127.0.0.1"),
            tokens: List::with_capacity(0),
            users,
            repositories,
            max_response_items: 32,
            default_paging_num: 30,
            hooks: None,
            faults: forgejo::Faults { reset: 0, drop: 0, server_error: 0 },
        };
        (Forge::new(domain, LIMITS), config)
    }
}
fn tree(d: &skein_json::Document, at: u32, p: &Path) -> Result<Box<[api::File]>, Refusal> {
    let items = match jig_protocol_config::read::list(d, at, LIMITS.limits.files) {
        Ok(items) => items,
        Err(why) => return Err(read::kind(p, why)),
    };
    let mut files = List::<api::File>::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let mut o = read::object(d, *at, &p)?;
        let path = read::field(&mut o, d, b"path", LIMITS.limits.name_bytes, &p)?;
        let content = read::field(&mut o, d, b"content", LIMITS.limits.content_bytes, &p)?;
        read::finish(o, d, &p)?;
        if !relative(&path) {
            return Err(read::why(&p.member(b"path"), b"expected a relative file path with no . or .. component"));
        }
        for prev in &files {
            if prev.path == path {
                return Err(read::why(&p.member(b"path"), b"must be unique"));
            }
        }
        files.push(api::File { path, content }).expect("bounded files");
    }
    Ok(files.into_boxed())
}
/// Makes the setup's one site admin and one token; `all` is Forgejo's every scope.
pub fn setup(store: &mut forgejo::setup::Store, admin_token: &[u8; 40]) {
    let admin = store.account(b"temper-admin", true, b"temper admin");
    store.token(admin, b"temper-admin", &[b"all"], Secret::new(bytes::copy_of(admin_token)));
}
#[must_use]
#[expect(clippy::manual_map, reason = "step code uses explicit matches, never closures")]
pub fn checks(ci: Option<&Ci>) -> api::Checks {
    api::Checks {
        contexts: Box::new([bytes::copy_of(CONTEXT)]),
        latency_min: Duration::ZERO,
        latency_max: Duration::ZERO,
        silent: 0,
        passes: 1000,
        reruns: 0,
        cue: match ci {
            Some(ci) => Some(api::Cue::File { path: ci.path.clone(), green: ci.green.clone() }),
            None => None,
        },
    }
}
/// Bounded store for eight repositories, with small mutable QA histories.
pub const LIMITS: domain::Config = domain::Config {
    limits: domain::Limits {
        repositories: 8,
        users: 32,
        bypassing_users: 8,
        labels: 8,
        items: 32,
        comments: 8,
        dependencies: 8,
        reviews: 8,
        branches: 32,
        commits: 128,
        files: 16,
        statuses: 32,
        contexts: 8,
        pages: 8,
        name_bytes: 256,
        title_bytes: 256,
        body_bytes: 4096,
        content_bytes: 8192,
        page_size: 32,
        calls: 8,
        hooks: 32,
        observations: 128,
    },
    latency_min: Duration::ZERO,
    latency_max: Duration::ZERO,
    settle_min: Duration::ZERO,
    settle_max: Duration::ZERO,
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
    ci: 0,
    hook_min: Duration::ZERO,
    hook_max: Duration::ZERO,
    hooks_late: 0,
    hooks_lost: 0,
    resolution: Duration::from_secs(1),
    skew: domain::Skew::None,
    status_updates: false,
    edit_updates: false,
};

/// The git route's wire bounds, shared by the peer and its process client.
#[must_use]
pub fn git_limits() -> skein_fake_forge_protocol::Limits {
    crate::wire::forge_limits()
}

/// Constructs the checked document's peer without io (protocol/setup.md, 8.4).
/// The peer's shared Forge is lent back to the host for observation drains.
/// This follows skein's fake-forge.md, 5, `Rc<RefCell<Forge>>` boundary;
/// the departure from programming-model.md's ownership rule is confined here.
#[expect(
    clippy::disallowed_types,
    reason = "skein-fake-peers requires shared Forge ownership for host observation drains"
)]
pub fn peer(
    document: &Document,
    token: &[u8; 40],
    seed: u64,
) -> Result<(skein_fake_peers::forge::Peer, alloc::rc::Rc<core::cell::RefCell<Forge>>), skein_fake_peers::Error> {
    let (forge, config) = document.seed(seed);
    let shared = alloc::rc::Rc::new(core::cell::RefCell::new(forge));
    if let Some(github) = &document.github {
        return Ok((github.peer(document, shared.clone(), seed)?, shared));
    }
    let mut peer = skein_fake_peers::forge::Peer::new(
        document.listen(),
        crate::limits::peer(),
        config,
        git_limits(),
        shared.clone(),
        seed,
    )?
    .without_records();
    setup(peer.setup_mut(), token);
    Ok((peer, shared))
}
