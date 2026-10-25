//! The process-tier GitHub document (domain/testing.md, 4.2).
use super::{Document, Repository};
use crate::{Refusal, read};
use alloc::boxed::Box;
use jig_protocol_config::Path;
use skein_fake_forge_github::{
    self as layer,
    installations::{Kind, Reach},
};
use skein_json::Document as Json;
use skein_lib::{Duration, List, Secret, Wall, bytes};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Github {
    id: u64,
    slug: Box<[u8]>,
    bot: u64,
    key: usize,
    setup_url: Option<Box<[u8]>>,
    accounts: Box<[Account]>,
    token_life: Duration,
}
#[derive(Clone, PartialEq, Eq, Debug)]
struct Account {
    login: Box<[u8]>,
    id: u64,
    kind: Kind,
    installation: Option<Installation>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
struct Installation {
    id: u64,
    repositories: Option<Box<[Box<[u8]>]>>,
}
fn id(o: &mut jig_protocol_config::read::Object, d: &Json, name: &[u8], p: &Path) -> Result<u64, Refusal> {
    let at = read::required(o, d, name, p)?;
    let value = read::integer(d, at, u64::MAX, &p.member(name))?;
    if value == 0 {
        return Err(read::why(&p.member(name), b"must be nonzero"));
    }
    Ok(value)
}
pub(super) fn decode(d: &Json, at: u32, p: &Path) -> Result<Github, Refusal> {
    let mut o = read::object(d, at, p)?;
    let app = read::required(&mut o, d, b"app", p)?;
    let accounts = read::required(&mut o, d, b"accounts", p)?;
    let token_life = match o.member(d, b"token_life_seconds") {
        Some(at) => {
            let value = read::integer(d, at, u64::MAX / 1_000_000_000, &p.member(b"token_life_seconds"))?;
            if value == 0 {
                return Err(read::why(&p.member(b"token_life_seconds"), b"must be nonzero"));
            }
            Duration::from_secs(value)
        }
        None => Duration::from_secs(3600),
    };
    read::finish(o, d, p)?;
    let app_path = p.member(b"app");
    let mut o = read::object(d, app, &app_path)?;
    let app_id = id(&mut o, d, b"id", &app_path)?;
    let slug = read::field(&mut o, d, b"slug", 39, &app_path)?;
    if !skein_forge_setup::given::login(&slug) {
        return Err(read::why(&app_path.member(b"slug"), b"expected an App slug"));
    }
    let bot = id(&mut o, d, b"bot", &app_path)?;
    if bot == app_id {
        return Err(read::why(&app_path.member(b"bot"), b"must differ from App id"));
    }
    let key_at = read::required(&mut o, d, b"key", &app_path)?;
    let key = read::integer(d, key_at, 2, &app_path.member(b"key"))?;
    let key = usize::try_from(key).expect("three fixed keys");
    let setup_url = match o.member(d, b"setup_url") {
        Some(at) if !read::is_null(d, at) => Some(read::text(d, at, 1024, &app_path.member(b"setup_url"))?),
        Some(_) | None => None,
    };
    read::finish(o, d, &app_path)?;
    let p = p.member(b"accounts");
    let items = read::list(d, accounts, 1, &p)?;
    let mut accounts: List<Account> = List::with_capacity(items.len());
    let mut installed = 0_u64;
    for (index, at) in items.iter().enumerate() {
        let p = read::index(&p, index);
        let account = account(d, *at, &p)?;
        for prior in &accounts {
            if prior.id == account.id || prior.login == account.login {
                return Err(read::why(&p, b"account login and id must be unique"));
            }
            if let (Some(a), Some(b)) = (&prior.installation, &account.installation)
                && a.id == b.id
            {
                return Err(read::why(&p.member(b"installation").member(b"id"), b"must be unique"));
            }
        }
        if let Some(installation) = &account.installation {
            installed = installed.checked_add(1).expect("eight accounts");
            if installation.id != installed {
                return Err(read::why(
                    &p.member(b"installation").member(b"id"),
                    b"expected sequential ids from 1 in installed account order",
                ));
            }
        }
        accounts.push(account).expect("eight accounts");
    }
    Ok(Github { id: app_id, slug, bot, key, setup_url, accounts: accounts.into_boxed(), token_life })
}
fn account(d: &Json, at: u32, p: &Path) -> Result<Account, Refusal> {
    let mut o = read::object(d, at, p)?;
    let login = read::field(&mut o, d, b"login", 39, p)?;
    if !skein_forge_setup::given::login(&login) {
        return Err(read::why(&p.member(b"login"), b"expected a GitHub login"));
    }
    let id = id(&mut o, d, b"id", p)?;
    let kind = read::field(&mut o, d, b"kind", 12, p)?;
    let kind = match kind.as_ref() {
        b"user" => Kind::User,
        b"organization" => Kind::Organization,
        _ => return Err(read::why(&p.member(b"kind"), b"expected user or organization")),
    };
    let at = read::required(&mut o, d, b"installation", p)?;
    read::finish(o, d, p)?;
    let installation = if read::is_null(d, at) {
        None
    } else {
        let p = p.member(b"installation");
        let mut o = read::object(d, at, &p)?;
        let id = self::id(&mut o, d, b"id", &p)?;
        let at = read::required(&mut o, d, b"repositories", &p)?;
        read::finish(o, d, &p)?;
        let p = p.member(b"repositories");
        let repositories = match d.token(at) {
            Some(token) if token.kind == skein_json::document::Kind::String => {
                if read::text(d, at, 3, &p)?.as_ref() != b"all" {
                    return Err(read::why(&p, b"expected all or repository names"));
                }
                None
            }
            Some(_) | None => {
                let items = read::list(d, at, 0, &p)?;
                let mut names = List::with_capacity(items.len());
                for (i, at) in items.iter().enumerate() {
                    let name = read::text(d, *at, 100, &read::index(&p, i))?;
                    if names.as_slice().contains(&name) {
                        return Err(read::why(&p, b"must be unique"));
                    }
                    names.push(name).expect("eight repository names");
                }
                Some(names.into_boxed())
            }
        };
        Some(Installation { id, repositories })
    };
    Ok(Account { login, id, kind, installation })
}
impl Github {
    pub(super) const fn bot(&self) -> u64 {
        self.bot
    }
    pub(super) fn check(&self, repositories: &[Repository]) -> Result<(), Refusal> {
        for (i, repository) in repositories.iter().enumerate() {
            if self.account(&repository.owner).is_none() {
                return Err(read::why(
                    &read::index(&Path::root().member(b"repositories"), i).member(b"owner"),
                    b"must name an account",
                ));
            }
        }
        for account in &self.accounts {
            if let Some(Installation { repositories: Some(names), .. }) = &account.installation {
                for name in names {
                    let mut found = false;
                    for repo in repositories {
                        found |= repo.owner == account.login && repo.name == *name;
                    }
                    if !found {
                        return Err(read::why(
                            &Path::root().member(b"github").member(b"accounts"),
                            b"installation repository must belong to its account",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    #[expect(clippy::manual_find, reason = "step code uses explicit loops, never closures")]
    fn account(&self, login: &[u8]) -> Option<&Account> {
        for account in &self.accounts {
            if account.login.as_ref() == login {
                return Some(account);
            }
        }
        None
    }
    fn config(&self, document: &Document) -> layer::Config {
        let mut accounts = List::with_capacity(crate::ENTRIES);
        for account in &self.accounts {
            accounts
                .push(layer::service::Account {
                    login: account.login.clone(),
                    id: account.id,
                    organization: account.kind == Kind::Organization,
                })
                .expect("eight accounts");
        }
        let mut repositories = List::with_capacity(crate::ENTRIES);
        for (i, repository) in document.repositories.iter().enumerate() {
            let account = self.account(&repository.owner).expect("checked owner");
            repositories
                .push(layer::service::Row {
                    id: u32::try_from(i).expect("eight repositories").checked_add(1).expect("repository id"),
                    owner: account.login.clone(),
                    owner_id: account.id,
                    organization: account.kind == Kind::Organization,
                    name: repository.name.clone(),
                    private: true,
                })
                .expect("eight repositories");
        }
        let first = self.accounts.first().expect("nonempty accounts");
        let mut permissions = List::with_capacity(16);
        for (name, access) in skein_forge::github::setup::PERMISSIONS {
            permissions
                .push(skein_forge::github::document::Permission { name: bytes::copy_of(name), access })
                .expect("wanted permissions");
        }
        let mut events = List::with_capacity(16);
        for event in skein_forge::github::setup::EVENTS {
            events.push(bytes::copy_of(event)).expect("wanted events");
        }
        layer::Config {
            deliveries: layer::delivery::Config::standard(),
            rate_quota: 5000,
            rate_window: Duration::from_secs(3600),
            token_life: self.token_life,
            base: bytes::copy_of(b"http://127.0.0.1"),
            blob: bytes::copy_of(b"http://127.0.0.1"),
            third_origin: None,
            blob_redirect: None,
            app: layer::app::App {
                id: self.id,
                slug: self.slug.clone(),
                owner: first.id,
                permissions: permissions.into_boxed(),
                events: events.into_boxed(),
                key: layer::keys::KEYS.get(self.key).expect("checked fixed key"),
                webhook: Secret::new(bytes::copy_of(b"fake-webhook")),
            },
            app_facts: layer::service::AppFacts::Complete,
            bot: self.bot,
            shown: layer::service::Shown::Ruleset,
            ruleset_id: 1,
            pull_requests_only: false,
            required_signatures: false,
            hook: bytes::copy_of(b"https://temper-test.invalid/"),
            repositories,
            accounts,
            person: first.login.clone(),
            tokens: List::with_capacity(0),
            faults: layer::Faults { reset: 0, drop: 0, server_error: 0 },
        }
    }
    #[expect(clippy::disallowed_types, reason = "skein's peer shares its Forge with the observation host")]
    pub(super) fn peer(
        &self,
        document: &Document,
        forge: alloc::rc::Rc<core::cell::RefCell<skein_fake_peers::forge::Forge>>,
        seed: u64,
    ) -> Result<skein_fake_peers::forge::Peer, skein_fake_peers::Error> {
        let mut peer_limits = crate::limits::peer();
        peer_limits.io.sockets = peer_limits.io.sockets.checked_add(1).expect("second GitHub listener");
        let config = self.config(document);
        let mut blob = document.listen;
        blob.set_port(0);
        let mut peer =
            skein_fake_peers::forge::Peer::github(document.listen, blob, peer_limits, config, limits(), forge, seed)?
                .without_records();
        let layer = peer.github_layer_mut().expect("GitHub peer");
        layer.set_setup_url(self.setup_url.clone()).or(Err(skein_fake_peers::Error::Limits))?;
        let installations = layer.installations_mut();
        // The pinned layer allocates from 1; accounts without an installation consume no id.
        for account in &self.accounts {
            let Some(installation) = &account.installation else { continue };
            let reach = match &installation.repositories {
                None => Reach::All,
                Some(names) => {
                    let mut ids = List::with_capacity(crate::ENTRIES);
                    for (i, repository) in document.repositories.iter().enumerate() {
                        if repository.owner == account.login && names.contains(&repository.name) {
                            ids.push(
                                u32::try_from(i).expect("eight repositories").checked_add(1).expect("repository id"),
                            )
                            .expect("eight ids");
                        }
                    }
                    Reach::Listed(ids)
                }
            };
            let id = installations
                .install(account.id, &account.login, account.kind, reach, Wall::EPOCH)
                .or(Err(skein_fake_peers::Error::Limits))?;
            if id != installation.id {
                return Err(skein_fake_peers::Error::Limits);
            }
        }
        Ok(peer)
    }
}
/// Bounds the layer's retained git-call ring and thus the process's observation stream.
#[must_use]
pub fn limits() -> layer::Limits {
    let mut wire = super::git_limits();
    wire.http.response = crate::limits::BUFFER;
    layer::Limits {
        deliveries: layer::delivery::Limits { entries: 64, payload_bytes: crate::limits::FORGE_BODY },
        http: wire.http,
        github: skein_forge::github::limits::Limits {
            json: wire.documents.json,
            writer: wire.documents.writer,
            string_bytes: wire.documents.string_bytes,
            body_bytes: wire.documents.body_bytes,
            inner: wire.documents.inner,
            target_bytes: wire.documents.target_bytes,
        },
        installations: layer::installations::Limits {
            installations: crate::ENTRIES,
            tokens: 32,
            listed: crate::ENTRIES,
        },
        pages: layer::pages::Limits { states: crate::ENTRIES, manifest_bytes: 8192 },
        page: 32,
        body: crate::limits::FORGE_BODY,
        response: crate::limits::FORGE_BODY,
        blob_records: 16,
        git_records: 128,
    }
}

impl Github {
    pub(super) fn pem(&self) -> Box<[u8]> {
        layer::keys::pem(layer::keys::KEYS.get(self.key).expect("checked key index")).expect("fixed fake App key")
    }
}
