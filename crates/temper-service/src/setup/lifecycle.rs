//! Connection sequencing and serialized installation rewrites (protocol/forge.md, 2.2–2.6).
use super::{Write, answer, ended, failure_text};
use alloc::boxed::Box;
use core::num::NonZeroU64;
use jig_protocol_store::{SecretOwner, Secrets};
use skein_forge_connection as connection;
use skein_forge_setup::{host, reconcile};
use skein_lib::List;
use skein_lib::{Map, Secret, Token};
use temper_engine_domain as root;
use temper_engine_domain_forge as domain;
use temper_protocol_records::{self as records, Item};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Manifest,
    Convert,
    Paste,
    Install,
    Accept,
    Reconcile,
    LocalTakeOver,
    LocalRemoval,
    LocalReconcile,
    LocalCreate,
    AdminReconcile,
    AdminCreate,
    LocalConnect,
    Adopt,
    Follow,
    Reachable,
}
#[derive(Debug)]
pub(super) struct Run {
    pub(super) first: Option<Intent>,
    pub(super) previous: domain::Connection,
    pub(super) forge: u16,
    pub(super) kind: Kind,
    pub(super) internal: bool,
}
#[derive(Debug)]
pub(super) struct Adoption {
    pub(super) next: NonZeroU64,
    pub(super) repositories: u64,
    pub(super) files: Option<AdoptionFiles>,
}
#[derive(Debug)]
pub(super) struct AdoptionFiles {
    pub(super) admin_token: Secret,
    pub(super) access: connection::Access,
}
#[derive(Debug)]
pub(crate) struct Intent {
    pub(crate) label: Box<[u8]>,
    pub(crate) org: Box<[u8]>,
    pub(crate) repository: Box<[u8]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Source {
    Prepared,
    Managed,
}
#[derive(Debug)]
pub(super) struct Local {
    pub(super) source: Source,
    pub(crate) first: Option<Intent>,
    pub(super) repositories: u64,
    pub(super) pending: bool,
    pub(super) restored: bool,
    pub(super) limits: skein_forge_setup::limits::Limits,
    pub(super) webhooks: bool,
    pub(super) bot_login: Box<[u8]>,
    pub(super) identity: crate::settings::Identity,
    pub(super) handover: Option<Secret>,
    pub(super) path: Box<[u8]>,
    pub(super) file: FileStep,
    pub(super) trigger: Trigger,
}
#[derive(PartialEq, Eq, Debug)]
pub(super) enum Trigger {
    Start,
    StartAsked,
    StartDemo,
    Asked,
}
#[derive(Debug)]
pub(super) enum FileStep {
    Idle,
    Reading { run: Token, sent: bool },
    Read(Result<Option<Secret>, crate::HandoverUnread>),
    Removing { run: Token, sent: bool },
    Removed { run: Token, result: Result<(), i32> },
}
impl FileStep {
    fn completed(&self) -> bool {
        match self {
            Self::Read(_) | Self::Removed { .. } => true,
            Self::Idle | Self::Reading { .. } | Self::Removing { .. } => false,
        }
    }
    fn idle(&self) -> bool {
        match self {
            Self::Idle => true,
            Self::Read(_) | Self::Removed { .. } | Self::Reading { .. } | Self::Removing { .. } => false,
        }
    }
}
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "local state lives inline in the bounded forge map")]
pub(super) enum Reconcile {
    GitHub,
    Local(Local),
}
pub(super) struct Lifecycle {
    pub(super) adoptions: Map<u16, Adoption>,
    pub(super) reconcile: Map<u16, Reconcile>,
    deleted: Map<(u16, u64), ()>,
    pub(super) deleting: Map<u16, ()>,
    pub(super) installations: Map<(u16, u64), domain::Installation>,
    next_run: u64,
    restored: bool,
}
impl Lifecycle {
    pub(super) fn local(&self, forge: u16) -> Option<&Local> {
        match self.reconcile.get(&forge) {
            Some(Reconcile::Local(local)) => Some(local),
            Some(Reconcile::GitHub) | None => None,
        }
    }
    pub(super) fn local_mut(&mut self, forge: u16) -> Option<&mut Local> {
        match self.reconcile.get_mut(&forge) {
            Some(Reconcile::Local(local)) => Some(local),
            Some(Reconcile::GitHub) | None => None,
        }
    }
    pub(super) fn ready(&self, runs: &Map<Token, Run>, limit: u32, domain: &root::Domain) -> bool {
        for (forge, pending) in &self.reconcile {
            if domain.forge_ended(*forge) {
                continue;
            }
            if let Reconcile::Local(local) = pending
                && (local.pending && local.restored || local.file.completed())
            {
                return true;
            }
        }
        for (forge, _) in &self.adoptions {
            if domain.forge_ended(*forge) {
                continue;
            }
            if runs.len() < limit && !running(runs, *forge) {
                return true;
            }
        }
        for (forge, pending) in &self.reconcile {
            if domain.forge_ended(*forge) {
                continue;
            }
            match pending {
                Reconcile::Local(_) => continue,
                Reconcile::GitHub => {}
            }
            if runs.len() < limit
                && connected_in_root(domain, *forge)
                && !self.deleting.contains_key(forge)
                && !running(runs, *forge)
            {
                return true;
            }
        }
        for ((forge, _), ()) in &self.deleted {
            if domain.forge_ended(*forge) {
                continue;
            }
            if !self.deleting.contains_key(forge) && !running(runs, *forge) {
                return true;
            }
        }
        false
    }
    pub(super) fn new(forges: u32, installations: u32) -> Self {
        let count = forges.checked_mul(installations).expect("installation tables");
        Self {
            adoptions: Map::with_capacity(forges),
            reconcile: Map::with_capacity(forges),
            deleted: Map::with_capacity(count),
            deleting: Map::with_capacity(forges),
            installations: Map::with_capacity(count),
            next_run: 1,
            restored: false,
        }
    }
}
fn connected_in_root(root: &root::Domain, forge: u16) -> bool {
    match root.forge_connection(forge) {
        domain::Connection::Connected { .. } => true,
        domain::Connection::NotConnected
        | domain::Connection::NotSetUp
        | domain::Connection::SettingUp { .. }
        | domain::Connection::SetUpFailed { .. }
        | domain::Connection::Connecting
        | domain::Connection::Stopped { .. } => false,
    }
}
pub(crate) fn set_up_refused(
    engine: &mut crate::engine::Engine,
    forge: u16,
    request: Token,
    why: domain::SetUpRefusal,
) {
    // The link already named this fault. Ordinary refusals stay in the connector.
    engine.events.push(temper_engine_domain::Event::Forge(Box::new(domain::Event::SetUpAnswered {
        forge,
        request,
        answer: domain::SetUpAnswer::Refused(why),
    })));
}
pub(crate) fn stopped_connection(engine: &mut crate::engine::Engine, forge: u16) {
    if engine.setup.lifecycle.local(forge).is_none() {
        engine.setup.lifecycle.reconcile.remove(&forge);
    }
}
pub(super) fn kind(plan: &host::Plan) -> Kind {
    match plan {
        host::Plan::Manifest { .. } => Kind::Manifest,
        host::Plan::Convert { .. } => Kind::Convert,
        host::Plan::Install { .. } => Kind::Install,
        host::Plan::Accept { .. } => Kind::Accept,
        host::Plan::Reconcile { .. } => Kind::Reconcile,
        host::Plan::Adopt { .. } => Kind::Adopt,
        host::Plan::Create { .. } => Kind::LocalCreate,
        host::Plan::Paste { .. } => Kind::Paste,
        host::Plan::TakeOver { .. } => Kind::LocalTakeOver,
        host::Plan::Check { .. } => unreachable!("Check is not staged"),
        host::Plan::Reachable { .. } => Kind::Reachable,
    }
}
pub(super) fn busy(engine: &crate::engine::Engine, forge: u16) -> bool {
    engine.setup.busy(forge)
}
fn running(runs: &Map<Token, Run>, forge: u16) -> bool {
    for (_, run) in runs {
        if run.forge == forge && run.kind != Kind::Reachable {
            return true;
        }
    }
    false
}

pub(super) fn has_app(engine: &crate::engine::Engine, forge: u16) -> bool {
    match engine.forge_records.rows.forge(forge) {
        Some(rows) => rows.has(Item::AppId) || rows.has(Item::AppSlug) || rows.has(Item::AppKey),
        None => false,
    }
}
pub(crate) fn state(engine: &mut crate::engine::Engine, forge: u16, state: domain::Connection) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    engine.events.push(root::Event::Forge(Box::new(domain::Event::Connection { forge, state })));
}
pub(super) fn keep(engine: &mut crate::engine::Engine, forge: u16, item: Item, bytes: &[u8], purpose: Purpose) {
    let value = Secret::new(records::encode(bytes).into_boxed_slice());
    let bound = usize::try_from(engine.setup.value_bytes).expect("record bound");
    assert!(value.expose().len() <= bound, "startup::checks::records proves every encoded setup write fits");
    assert!(engine.setup.next_write < (1_u64 << 60_u32), "setup write names fit prefix");
    let owner = Token::new(crate::tokens::SETUP | engine.setup.next_write);
    engine.setup.next_write = engine.setup.next_write.checked_add(1).expect("setup write names");
    let number = records::number(forge, item);
    engine
        .setup
        .writes
        .insert(owner, Write { purpose, number, value: value.clone() })
        .ok()
        .expect("bounded setup writes");
    if let Some(terminal) = engine.store.secrets(
        owner,
        Secrets::Keep {
            owner: SecretOwner::Application,
            number,
            value: jig_protocol_store::SecretValue::Application(value),
        },
    ) {
        answer(engine, terminal);
    }
}
pub(super) enum Purpose {
    Host(Token),
    Origin(Token),
    PasteOrigin(Token),
    LocalOrigin(Token),
    AdminOrigin(Token),
    Admin(Token),
    Deleted(u16),
}
/// The connect's `back`, held with its manifest run, for the manifest page's Back.
fn manifest_back(engine: &crate::engine::Engine, token: Token) -> Option<&[u8]> {
    match engine.setup.public.get(&token) {
        Some(super::public::Caller::Manifest { back, .. }) => back.as_deref(),
        _ => None,
    }
}
pub(super) fn done(engine: &mut crate::engine::Engine, token: Token, outcome: reconcile::Outcome) {
    let run = engine.setup.runs.get(&token).expect("ended setup run");
    let (forge, kind) = (run.forge, run.kind);
    if crate::forge::ended(engine, forge) {
        refused(engine, token, forge, Box::from(b"forge ended".as_slice()));
        return;
    }
    if kind == Kind::AdminReconcile || kind == Kind::AdminCreate {
        admin_done(engine, token, outcome);
        return;
    }
    if kind == Kind::Reachable {
        reached(engine, token, outcome);
        return;
    }
    if kind == Kind::LocalTakeOver {
        engine.setup.runs.get_mut(&token).expect("takeover done").kind = Kind::LocalRemoval;
        engine.setup.lifecycle.local_mut(forge).expect("local takeover").file =
            FileStep::Removing { run: token, sent: false };
        return;
    }
    if kind == Kind::LocalReconcile {
        engine.setup.runs.get_mut(&token).expect("reconcile done").kind = Kind::LocalConnect;
        let origin = records::origin(connection::settings::Kind::Forgejo, &configured(engine, forge).api).encode();
        keep(engine, forge, Item::Origin, &origin, Purpose::LocalOrigin(token));
        return;
    }
    if kind == Kind::LocalCreate {
        let reconcile::Outcome::Created { id } = outcome else { unreachable!("Create returns its repository") };
        engine.setup.runs.remove(&token).expect("created run");
        engine.events.push(root::Event::Forge(Box::new(domain::Event::FirstMade { forge, id: id.get() })));
        adopt_local(engine, forge);
        return;
    }
    if kind == Kind::Adopt {
        let adoption = engine.setup.lifecycle.adoptions.get_mut(&forge).expect("live start-time adoption");
        if adoption.next.get() < adoption.repositories {
            adoption.next = NonZeroU64::new(adoption.next.get().checked_add(1).expect("bounded repositories"))
                .expect("next repository");
        } else {
            let adoption = engine.setup.lifecycle.adoptions.remove(&forge).expect("last adoption");
            match adoption.files {
                Some(files) => crate::forge::connect(engine, forge, Ok(crate::forge::Start::Adopted(files.access))),
                None => connect_local(engine, forge),
            }
        }
    }
    if kind == Kind::Paste {
        if let reconcile::Outcome::Lacking { lacking } = &outcome
            && !lacking.is_empty()
        {
            engine.notes.push(crate::Note::ForgeLacking { number: forge, names: super::diagnostics::names(lacking) });
        }
        let origin = records::origin(connection::settings::Kind::GitHub, &configured(engine, forge).api).encode();
        keep(engine, forge, Item::Origin, &origin, Purpose::PasteOrigin(token));
        return;
    }
    if kind == Kind::Convert {
        let config = configured(engine, forge);
        let origin = records::origin(connection::settings::Kind::GitHub, &config.api).encode();
        keep(engine, forge, Item::Origin, &origin, Purpose::Origin(token));
        return;
    }
    if kind == Kind::Accept || kind == Kind::Reconcile {
        installations(engine, forge);
    }
    let outcome = match outcome {
        reconcile::Outcome::Manifest { state, target, manifest } => {
            // The kit answers the form's parts; the page is temper's (forge-setup.md 5.1).
            let back = manifest_back(engine, token);
            let Some(page) = super::routes::form(&target, &manifest, back, forge, engine.setup.env.limits.body_bytes)
            else {
                let failure = reconcile::Failure {
                    forge,
                    step: reconcile::Stage::App,
                    subject: Box::from(b"manifest".as_slice()),
                    why: reconcile::Why::Unexpected,
                };
                stopped(engine, token, failure);
                return;
            };
            engine.setup.hold(forge, state.clone(), page);
            root::SetupOutcome::Manifest { state }
        }
        reconcile::Outcome::Install { state, url } => root::SetupOutcome::Install { state, url },
        reconcile::Outcome::Lacking { lacking } => {
            if !lacking.is_empty() {
                engine
                    .notes
                    .push(crate::Note::ForgeLacking { number: forge, names: super::diagnostics::names(&lacking) });
            }
            root::SetupOutcome::Done
        }
        reconcile::Outcome::Nothing => root::SetupOutcome::Done,
        reconcile::Outcome::Identity { .. }
        | reconcile::Outcome::Reachable { .. }
        | reconcile::Outcome::Created { .. } => {
            unreachable!("later plans are not staged")
        }
    };
    finish(engine, token, root::SetupEnd::Done { forge, outcome });
}
pub(super) fn stopped(engine: &mut crate::engine::Engine, token: Token, failure: reconcile::Failure) {
    super::public::stopped(engine, token, &failure);
    let run = engine.setup.runs.get(&token).expect("stopped setup run");
    if crate::forge::ended(engine, run.forge) {
        let forge = run.forge;
        refused(engine, token, forge, Box::from(b"forge ended".as_slice()));
        return;
    }
    if run.kind == Kind::Reachable {
        let why = reachable_failure(failure.why);
        engine.setup.runs.remove(&token);
        super::public::reached(engine, token, why);
        return;
    }
    if run.kind == Kind::LocalCreate || run.kind == Kind::AdminCreate {
        create_stopped(engine, token, failure);
        return;
    }
    if run.kind == Kind::Adopt {
        engine.setup.lifecycle.adoptions.remove(&run.forge);
    }
    if run.kind == Kind::LocalReconcile
        || run.kind == Kind::LocalTakeOver
        || run.kind == Kind::Adopt && engine.setup.lifecycle.local(run.forge).is_some()
    {
        let forge = run.forge;
        let plan = if run.kind == Kind::LocalTakeOver {
            crate::SetupPlan::TakeOver
        } else if run.kind == Kind::Adopt {
            crate::SetupPlan::Adopt
        } else {
            crate::SetupPlan::Reconcile
        };
        engine.setup.runs.remove(&token);
        local_stopped(
            engine,
            forge,
            crate::SetupStopped::Stopped { plan, step: crate::SetupStep { stage: failure.step, why: failure.why } },
        );
        return;
    }
    let mut reason = failure_text(&failure);
    if run.kind == Kind::Convert && failure.why == reconcile::Why::Refused(404) {
        reason = Box::from(
            b"setup conversion refused (404): the App may exist on GitHub without temper holding its key".as_slice(),
        );
    }
    if run.kind == Kind::Paste {
        match failure.why {
            reconcile::Why::KeyForm => {
                reason = Box::from(b"The key isn't a GitHub App private key: paste its .pem file whole.".as_slice());
            }
            reconcile::Why::Refused(401) => {
                reason =
                    Box::from(b"GitHub refused the App's id and key (401): check both are the same App's.".as_slice());
            }
            reconcile::Why::Missing
            | reconcile::Why::Refused(_)
            | reconcile::Why::NotAdmin
            | reconcile::Why::BotIsAdmin
            | reconcile::Why::Unmarked
            | reconcile::Why::OtherUser
            | reconcile::Why::OtherApp
            | reconcile::Why::Unknown
            | reconcile::Why::Busy
            | reconcile::Why::Crowded
            | reconcile::Why::Down
            | reconcile::Why::Unexpected => {}
        }
    }
    finish(engine, token, root::SetupEnd::Stopped { forge: failure.forge, reason });
}
fn finish(engine: &mut crate::engine::Engine, token: Token, end: root::SetupEnd) {
    let run = engine.setup.runs.remove(&token).expect("terminal setup run");
    let follow = run.kind == Kind::Follow;
    if run.kind == Kind::Convert {
        state(engine, run.forge, run.previous);
    }
    if run.internal {
        match end {
            root::SetupEnd::Stopped { forge, reason } => {
                ended(engine, token, root::SetupEnd::Stopped { forge, reason });
            }
            root::SetupEnd::Done { .. } => {}
        }
    } else {
        ended(engine, token, end);
    }
    if follow {
        engine.events.push(root::Event::Forge(Box::new(domain::Event::AppRecorded {
            forge: run.forge,
            app: domain::AppWay::Manifest,
        })));
        connect(engine, run.forge);
    }
}
pub(super) fn kept(engine: &mut crate::engine::Engine, purpose: Purpose) {
    match purpose {
        Purpose::AdminOrigin(run) => admin_create(engine, run),
        Purpose::Admin(run) => {
            let forge = engine.setup.runs.get(&run).expect("admin waits on Kept").forge;
            if crate::forge::ended(engine, forge) {
                refused(engine, run, forge, Box::from(b"forge ended".as_slice()));
                return;
            }
            connect_local(engine, forge);
            finish(engine, run, root::SetupEnd::Done { forge, outcome: root::SetupOutcome::Done });
        }
        Purpose::PasteOrigin(run) => {
            let forge = engine.setup.runs.get(&run).expect("paste waits on Origin").forge;
            finish(engine, run, root::SetupEnd::Done { forge, outcome: root::SetupOutcome::Done });
        }
        Purpose::LocalOrigin(run) => {
            let active = engine.setup.runs.remove(&run).expect("reconcile waits for Origin Kept");
            if crate::forge::ended(engine, active.forge) {
                return;
            }
            if !create_local(engine, active.forge) {
                adopt_local(engine, active.forge);
            }
        }
        Purpose::Host(run) => engine.setup.down(host::Request::Answer { run, answer: host::Answer::Written }),
        Purpose::Origin(run) => {
            let active = engine.setup.runs.get_mut(&run).expect("conversion waits on Origin");
            active.kind = Kind::Follow;
            let forge = active.forge;
            super::run_plan(engine, run, host::Plan::Install { forge });
        }
        Purpose::Deleted(forge) => {
            engine.setup.lifecycle.deleting.remove(&forge);
            installations(engine, forge);
        }
    }
}
fn configured(engine: &crate::engine::Engine, number: u16) -> &crate::settings::Forge {
    for forge in &engine.workers.forges {
        if forge.number == number {
            return forge;
        }
    }
    unreachable!("configured setup forge")
}
pub(super) fn connect(engine: &mut crate::engine::Engine, forge: u16) {
    let expected = records::origin(connection::settings::Kind::GitHub, &configured(engine, forge).api);
    let access = match engine.forge_records.rows.forge(forge) {
        Some(rows) => {
            records::access(connection::settings::Kind::GitHub, &expected, rows, &engine.forge_records.bounds, false)
        }
        None => Err(records::NotConnected::NoRecords),
    };
    let start = match access {
        Ok(start) => Ok(crate::forge::Start::Restored(start)),
        Err(why) => Err(why),
    };
    crate::forge::connect(engine, forge, start);
}
pub(super) fn accepted(engine: &crate::engine::Engine, forge: u16) -> Box<[u64]> {
    if let Some(rows) = engine.forge_records.rows.forge(forge)
        && let Ok(bytes) = rows.get(Item::Accepted)
    {
        return skein_forge_setup::item::accepted(bytes, engine.forge_records.bounds.installations)
            .expect("kept accepted set");
    }
    Box::new([])
}
pub(super) fn installations(engine: &mut crate::engine::Engine, forge: u16) {
    let accepted = accepted(engine, forge);
    for id in &accepted {
        if let Some(info) = engine.setup.lifecycle.installations.get_mut(&(forge, *id))
            && info.stray
        {
            info.stray = false;
            let info = info.clone();
            installation_event(engine, forge, *id, Some(info));
        }
    }
    crate::forge::tokens::accepted(engine, forge, &accepted);
    engine.forge.down(connection::Request::Installations { forge, accepted });
}
pub(crate) fn connected(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    let config = configured(engine, forge);
    let github = config.kind == crate::settings::Kind::GitHub;
    let (id, login) = if github {
        let rows = engine.forge_records.rows.forge(forge).expect("Connected has records");
        let id =
            skein_forge_setup::item::user(rows.get(Item::User).expect("connected User")).expect("valid User").get();
        let slug = rows.get(Item::AppSlug).expect("connected slug");
        let mut login =
            skein_lib::Writer::new(slug.len().checked_add(domain::BOT_LOGIN_SUFFIX.len()).expect("bot login"));
        login.put(slug).expect("slug room");
        login.put(domain::BOT_LOGIN_SUFFIX).expect("suffix room");
        (id, login.finish())
    } else if let Some(local) = engine.setup.lifecycle.local(forge) {
        let rows = engine.forge_records.rows.forge(forge).expect("Connected has records");
        let id =
            skein_forge_setup::item::user(rows.get(Item::User).expect("connected User")).expect("valid User").get();
        (id, local.bot_login.clone())
    } else {
        // Forgejo still starts from private files (temper-15); its login is the git identity's.
        let mut login = Box::from([]);
        if let Some((_, value)) = engine.values.identities.newest(config.identity) {
            let mut reader = skein_lib::Reader::new(value.expose());
            let identity = temper_protocol_worker_wire::v1::Identity::decode(
                &temper_protocol_worker_wire::v1::CEILINGS,
                &mut reader,
            )
            .expect("configured identity");
            login = Box::from(identity.login());
        }
        (config.user, login)
    };
    state(engine, forge, domain::Connection::Connected { id, login });
    if github {
        crate::forge::tokens::connected(engine, forge);
        engine.setup.lifecycle.reconcile.insert(forge, Reconcile::GitHub).expect("one reconcile per forge");
    }
}
fn installation_event(
    engine: &mut crate::engine::Engine,
    forge: u16,
    installation: u64,
    state: Option<domain::Installation>,
) {
    engine.events.push(root::Event::Forge(Box::new(domain::Event::Installation { forge, installation, state })));
}
/// A minted token fills the public account without changing suspension or acceptance.
pub(crate) fn installation_account(
    engine: &mut crate::engine::Engine,
    forge: u16,
    installation: u64,
    account: &connection::Account,
) {
    let Some(info) = engine.setup.lifecycle.installations.get_mut(&(forge, installation)) else { return };
    let heard = (account.id, account.login.clone());
    if info.account.as_ref() != Some(&heard) {
        info.account = Some(heard);
        let info = info.clone();
        installation_event(engine, forge, installation, Some(info));
    }
}
pub(crate) fn installation(
    engine: &mut crate::engine::Engine,
    forge: u16,
    installation: u64,
    account: Option<connection::Account>,
    change: connection::Change,
) {
    if change == connection::Change::Deleted || change == connection::Change::Suspended {
        crate::forge::tokens::removed(engine, forge, installation);
    }
    let key = (forge, installation);
    if change == connection::Change::Deleted {
        let known = engine.setup.lifecycle.installations.remove(&key).is_some();
        let accepted = accepted(engine, forge).contains(&installation);
        if known || accepted && !engine.setup.lifecycle.deleted.contains_key(&key) {
            installation_event(engine, forge, installation, None);
        }
        if accepted {
            engine.setup.lifecycle.deleted.insert(key, ()).expect("bounded accepted deletions");
        }
        return;
    }
    let previous = engine.setup.lifecycle.installations.get(&key);
    let suspended = match change {
        connection::Change::Suspended => true,
        connection::Change::Unsuspended => false,
        // A changed repository selection leaves the suspension as it was.
        connection::Change::Created | connection::Change::Renamed | connection::Change::Repositories => {
            match previous {
                Some(info) => info.suspended,
                None => false,
            }
        }
        connection::Change::Deleted => unreachable!("deleted above"),
    };
    let info = domain::Installation {
        account: match account {
            Some(account) => Some((account.id, account.login)),
            None => None,
        },
        stray: !accepted(engine, forge).contains(&installation),
        suspended,
    };
    if previous != Some(&info) {
        engine.setup.lifecycle.installations.insert(key, info.clone()).expect("component's installation bound");
        installation_event(engine, forge, installation, Some(info));
    }
}
pub(super) fn next_run(lifecycle: &mut Lifecycle) -> Token {
    assert!(lifecycle.next_run < (1_u64 << 60_u32), "internal setup names");
    let run = Token::new(crate::tokens::SETUP | lifecycle.next_run);
    lifecycle.next_run = lifecycle.next_run.checked_add(1).expect("setup run names");
    run
}
#[expect(clippy::too_many_lines, reason = "one ordered pass over setup continuations")]
pub(super) fn advance(engine: &mut crate::engine::Engine) {
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() || engine.setup.component.is_none() {
        return;
    }
    let mut forges = List::with_capacity(engine.setup.env.limits.forges);
    for forge in &engine.workers.forges {
        forges.push(forge.number).expect("configured forges");
    }
    for forge in &forges {
        if crate::forge::ended(engine, *forge) {
            continue;
        }
        if !engine.setup.room()
            || !engine.store.room()
            || !engine.forge.room(2)
            || engine.events.room() < 4
            || engine.notes.room() == 0
        {
            continue;
        }
        if file_answer(engine, *forge) || start_pending(engine, *forge) {
            continue;
        }
        if busy(engine, *forge) {
            continue;
        }
        if engine.setup.lifecycle.adoptions.contains_key(forge) {
            let Some(repository) = adoption_repository(engine, *forge) else {
                continue;
            };
            let run = next_run(&mut engine.setup.lifecycle);
            if !engine.setup.served.contains_key(forge) {
                let reason = *engine.setup.unserved.get(forge).expect("configured setup exclusion");
                engine.setup.lifecycle.adoptions.remove(forge);
                ended(engine, run, root::SetupEnd::Stopped { forge: *forge, reason: Box::from(reason) });
                continue;
            }
            engine
                .setup
                .runs
                .insert(
                    run,
                    Run {
                        first: None,
                        forge: *forge,
                        kind: Kind::Adopt,
                        internal: true,
                        previous: domain::Connection::NotConnected,
                    },
                )
                .expect("admitted adoption");
            super::run_plan(engine, run, host::Plan::Adopt { forge: *forge, repository });
            continue;
        }
        let mut deleted = List::with_capacity(engine.forge_records.bounds.installations);
        for ((number, id), ()) in &engine.setup.lifecycle.deleted {
            if number == forge {
                deleted.push(*id).expect("accepted deletion bound");
            }
        }
        if !deleted.is_empty() {
            let mut remaining = List::with_capacity(engine.forge_records.bounds.installations);
            for id in &accepted(engine, *forge) {
                if !deleted.as_slice().contains(id) {
                    remaining.push(*id).expect("accepted bound");
                }
            }
            for id in &deleted {
                engine.setup.lifecycle.deleted.remove(&(*forge, *id));
            }
            if remaining.as_slice() == accepted(engine, *forge).as_ref() {
                continue;
            }
            engine.setup.lifecycle.deleting.insert(*forge, ()).expect("one rewrite per forge");
            let bytes = skein_forge_setup::item::accepted_item(remaining.as_slice());
            keep(engine, *forge, Item::Accepted, bytes.expose(), Purpose::Deleted(*forge));
        } else if engine.setup.lifecycle.local(*forge).is_none()
            && engine.setup.lifecycle.reconcile.contains_key(forge)
            && connected_in_root(&engine.domain, *forge)
            && engine.setup.runs.len() < engine.setup.env.limits.runs
        {
            engine.setup.lifecycle.reconcile.remove(forge);
            let run = next_run(&mut engine.setup.lifecycle);
            if !engine.setup.served.contains_key(forge) {
                let reason = *engine.setup.unserved.get(forge).expect("configured setup exclusion");
                ended(engine, run, root::SetupEnd::Stopped { forge: *forge, reason: Box::from(reason) });
                continue;
            }
            engine
                .setup
                .runs
                .insert(
                    run,
                    Run {
                        first: None,
                        forge: *forge,
                        kind: Kind::Reconcile,
                        internal: true,
                        previous: domain::Connection::NotConnected,
                    },
                )
                .expect("admitted reconcile");
            super::run_plan(engine, run, host::Plan::Reconcile { forge: *forge, adopted: Box::new([]) });
        }
    }
}

#[expect(clippy::disallowed_macros, reason = "Stopped has no credential; diagnostic boundary")]
pub(crate) fn stopped_text(why: connection::Stopped) -> Box<[u8]> {
    alloc::format!("{why:?}").into_bytes().into_boxed_slice()
}

pub(super) fn refused(engine: &mut crate::engine::Engine, run: Token, forge: u16, reason: Box<[u8]>) {
    if engine.setup.runs.contains_key(&run) {
        engine.setup.lifecycle.adoptions.remove(&forge);
        finish(engine, run, root::SetupEnd::Stopped { forge, reason });
    } else {
        ended(engine, run, root::SetupEnd::Stopped { forge, reason });
    }
}

/// Called after connector rows restore, before fresh reads need their connections.
pub(crate) fn restored(engine: &mut crate::engine::Engine) {
    if engine.setup.lifecycle.restored {
        return;
    }
    for forge in &engine.workers.forges {
        if let Some(local) = engine.setup.lifecycle.local_mut(forge.number) {
            local.restored = true;
        }
    }
    engine.setup.lifecycle.restored = true;
}
fn local_stopped(engine: &mut crate::engine::Engine, forge: u16, why: crate::SetupStopped) {
    let local = engine.setup.lifecycle.local_mut(forge).expect("local failure");
    local.handover = None;
    let connection = match local.trigger {
        Trigger::Start => domain::Connection::NotConnected,
        Trigger::StartAsked if why == crate::SetupStopped::NotPrepared => domain::Connection::NotSetUp,
        Trigger::StartDemo | Trigger::StartAsked | Trigger::Asked => {
            domain::Connection::SetUpFailed { why: local_failure(forge, &why) }
        }
    };
    engine.notes.push(crate::Note::ForgeSetupStopped { number: forge, why });
    state(engine, forge, connection);
}
fn start_pending(engine: &mut crate::engine::Engine, forge: u16) -> bool {
    if let Some(local) = engine.setup.lifecycle.local(forge)
        && local.restored
        && local.pending
    {
        start_local(engine, forge);
        return true;
    }
    false
}
fn start_local(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    let local = engine.setup.lifecycle.local_mut(forge).expect("local start");
    assert!(local.source != Source::Managed || local.handover.is_none(), "managed string form never stages TakeOver");
    local.pending = false;
    if local.trigger == Trigger::StartDemo {
        state(engine, forge, domain::Connection::NotSetUp);
        return;
    }
    if busy(engine, forge) || engine.setup.runs.len() == engine.setup.env.limits.runs {
        local_stopped(engine, forge, crate::SetupStopped::Busy);
        return;
    }
    if !engine.setup.served.contains_key(&forge) || engine.forge.unreachable(forge).is_some() {
        local_stopped(engine, forge, crate::SetupStopped::Missing);
        return;
    }
    if let Some(admin_token) = engine.setup.lifecycle.local_mut(forge).expect("local start").handover.take() {
        let run = next_run(&mut engine.setup.lifecycle);
        engine
            .setup
            .runs
            .insert(
                run,
                Run {
                    first: None,
                    forge,
                    kind: Kind::LocalTakeOver,
                    internal: true,
                    previous: domain::Connection::NotConnected,
                },
            )
            .expect("admitted local takeover");
        state(engine, forge, domain::Connection::Connecting);
        super::run_plan(engine, run, host::Plan::TakeOver { forge, admin_token });
        return;
    }
    reconcile_local(engine, forge);
}
fn reconcile_local(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    let Some(rows) = engine.forge_records.rows.forge(forge) else {
        missing_admin(engine, forge);
        return;
    };
    if !rows.has(Item::AdminToken) {
        missing_admin(engine, forge);
        return;
    }
    let valid = match rows.get(Item::AdminToken) {
        Ok(bytes) => skein_forge_setup::item::fits(
            skein_forge_setup::item::Item::AdminToken,
            bytes,
            &engine.forge_records.bounds,
        ),
        Err(_) => false,
    };
    if !valid {
        records_failed(engine, forge, records::NotConnected::Unreadable);
        return;
    }
    let expected = records::origin(connection::settings::Kind::Forgejo, &configured(engine, forge).api);
    if rows.has(Item::Origin) {
        let origin = match rows.get(Item::Origin) {
            Ok(bytes) => records::Origin::decode(bytes).ok(),
            Err(_) => None,
        };
        let why = match origin {
            Some(found) if found != expected => {
                Some(records::NotConnected::OtherForge { kind: found.kind, api: found.api })
            }
            None => Some(records::NotConnected::Unreadable),
            Some(_) => None,
        };
        if let Some(why) = why {
            records_failed(engine, forge, why);
            return;
        }
    }
    let adopted = engine.domain.adopted_ids(forge);
    let local = engine.setup.lifecycle.local(forge).expect("local start");
    let limits = &local.limits;
    assert!(
        skein_forge_setup::given::check_adopted(&adopted, limits).is_ok(),
        "restored adopted ids are checked at their entrance"
    );
    let run = next_run(&mut engine.setup.lifecycle);
    engine
        .setup
        .runs
        .insert(
            run,
            Run {
                first: None,
                forge,
                kind: Kind::LocalReconcile,
                internal: true,
                previous: domain::Connection::NotConnected,
            },
        )
        .expect("admitted local reconcile");
    state(engine, forge, domain::Connection::Connecting);
    super::run_plan(engine, run, host::Plan::Reconcile { forge, adopted });
}
#[expect(clippy::disallowed_macros, reason = "record refusal contains only public origin information")]
fn records_failed(engine: &mut crate::engine::Engine, forge: u16, why: records::NotConnected) {
    if engine.setup.lifecycle.local(forge).expect("local failure").trigger != Trigger::Start {
        state(
            engine,
            forge,
            domain::Connection::SetUpFailed {
                why: bounded_reason(alloc::format!("reconciling: {why}").into_bytes().into_boxed_slice()),
            },
        );
    }
    engine.notes.push(crate::Note::ForgeNotConnected { number: forge, why });
}
fn adoption_repository(engine: &mut crate::engine::Engine, forge: u16) -> Option<NonZeroU64> {
    if engine.setup.runs.len() == engine.setup.env.limits.runs {
        return None;
    }
    let adoption = engine.setup.lifecycle.adoptions.get(&forge).expect("pending adoption");
    if adoption.files.is_some() {
        return Some(adoption.next);
    }
    let adopted = engine.domain.adopted_ids(forge);
    for id in adoption.next.get()..=adoption.repositories {
        if !adopted.contains(&id) {
            let next = NonZeroU64::new(id).expect("seeded repository");
            engine.setup.lifecycle.adoptions.get_mut(&forge).expect("local adoption").next = next;
            return Some(next);
        }
    }
    engine.setup.lifecycle.adoptions.remove(&forge);
    connect_local(engine, forge);
    None
}
/// Reconcile's Origin is durable before the first repository can be made.
fn create_local(engine: &mut crate::engine::Engine, forge: u16) -> bool {
    let local = engine.setup.lifecycle.local_mut(forge).expect("reconciled local");
    let Some(first) = local.first.take() else { return false };
    let plan = host::Plan::Create {
        forge,
        org: first.org.clone(),
        repository: first.repository.clone(),
        files: Box::new([host::File {
            path: temper_forge_setup::STARTER_PATH.into(),
            content: temper_forge_setup::starter_workflow(b"temper-host").expect("owned runner label"),
        }]),
    };
    assert!(
        skein_forge_setup::given::check_create(&local.limits, engine.setup.env.limits.string_bytes, &plan).is_ok(),
        "SetUp admission and the starter profile prove Create's inputs"
    );
    let run = next_run(&mut engine.setup.lifecycle);
    engine
        .setup
        .runs
        .insert(
            run,
            Run {
                first: Some(first),
                forge,
                kind: Kind::LocalCreate,
                internal: true,
                previous: domain::Connection::NotConnected,
            },
        )
        .expect("reconcile released its run slot");
    super::run_plan(engine, run, plan);
    true
}
#[expect(clippy::disallowed_macros, reason = "closed credential-free Create diagnostic")]
fn create_stopped(engine: &mut crate::engine::Engine, token: Token, failure: reconcile::Failure) {
    let run = engine.setup.runs.get_mut(&token).expect("stopped Create");
    let admin = run.kind == Kind::AdminCreate;
    let forge = run.forge;
    let first = run.first.take().expect("Create retains the admitted names");
    let step = match failure.step {
        reconcile::Stage::Admin => domain::CreateStep::Admin,
        reconcile::Stage::Org => domain::CreateStep::Org,
        reconcile::Stage::Repository => domain::CreateStep::Repository,
        reconcile::Stage::File => domain::CreateStep::File,
        reconcile::Stage::Bot
        | reconcile::Stage::Token
        | reconcile::Stage::App
        | reconcile::Stage::Installation
        | reconcile::Stage::State => unreachable!("host Create stopped outside its stages"),
    };
    let suffix = alloc::format!(": {:?} ({:?})", failure.step, failure.why);
    let length = first
        .org
        .len()
        .checked_add(first.repository.len())
        .expect("bounded Create names")
        .checked_add(8)
        .expect("bounded Create prefix")
        .checked_add(suffix.len())
        .expect("bounded Create diagnostic");
    let mut why = skein_lib::Writer::new(length);
    for part in [b"making ".as_slice(), &first.org, b"/", &first.repository, suffix.as_bytes()] {
        why.put(part).expect("measured Create diagnostic");
    }
    let why = bounded_reason(why.finish());
    engine.notes.push(crate::Note::ForgeSetupStopped {
        number: forge,
        why: crate::SetupStopped::Stopped {
            plan: crate::SetupPlan::Create,
            step: crate::SetupStep { stage: failure.step, why: failure.why },
        },
    });
    engine.events.push(root::Event::Forge(Box::new(domain::Event::FirstFailed { forge, step })));
    if admin {
        finish(engine, token, root::SetupEnd::Stopped { forge, reason: why });
    } else {
        engine.setup.runs.remove(&token).expect("stopped local Create");
        state(engine, forge, domain::Connection::SetUpFailed { why });
    }
}
fn adopt_local(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    let repositories = engine.setup.lifecycle.local(forge).expect("reconciled local").repositories;
    if repositories == 0 {
        connect_local(engine, forge);
        return;
    }
    engine
        .setup
        .lifecycle
        .adoptions
        .insert(forge, Adoption { next: NonZeroU64::new(1).expect("first repository"), repositories, files: None })
        .expect("one adoption per local fake");
}
pub(super) fn connect_local(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    let expected = records::origin(connection::settings::Kind::Forgejo, &configured(engine, forge).api);
    let rows = engine.forge_records.rows.forge(forge).expect("reconciled records");
    let local = engine.setup.lifecycle.local_mut(forge).expect("local connection");
    let start = records::access(
        connection::settings::Kind::Forgejo,
        &expected,
        rows,
        &engine.forge_records.bounds,
        local.webhooks,
    );
    let start = match start {
        Ok(start) => start,
        Err(why) => {
            crate::forge::connect(engine, forge, Err(why));
            return;
        }
    };
    let token = rows.get(Item::GitToken).expect("reconciled GitToken");
    assert!(
        skein_forge_setup::item::fits(skein_forge_setup::item::Item::GitToken, token, &engine.forge_records.bounds),
        "kept GitToken validated"
    );
    {
        local.identity.token = Secret::new(Box::from(token));
        let value = crate::translate::identity(&local.identity, &engine.workers.encoding.limits)
            .expect("checked identity bounds");
        local.identity.token = Secret::new(Box::new([]));
        let number = local.identity.number;
        let generation = engine.values.identities.put(number, value);
        engine.events.push(root::Event::Credential {
            number,
            generation,
            valid: skein_lib::Duration::from_nanos(u64::MAX),
        });
    }
    crate::forge::connect(engine, forge, Ok(crate::forge::Start::Restored(start)));
}

#[expect(clippy::disallowed_macros, reason = "closed credential-free setup diagnostic")]
fn local_failure(forge: u16, why: &crate::SetupStopped) -> Box<[u8]> {
    let text = match why {
        crate::SetupStopped::NotPrepared => Box::from(b"the box's preparation left no handover".as_slice()),
        crate::SetupStopped::Missing => Box::from(b"taking the handover: setup host missing".as_slice()),
        crate::SetupStopped::Busy => Box::from(b"taking the handover: setup host busy".as_slice()),
        crate::SetupStopped::Stopped { plan, step } => {
            let action = match plan {
                crate::SetupPlan::TakeOver => "taking the handover",
                crate::SetupPlan::Reconcile => "reconciling",
                crate::SetupPlan::Adopt => "adopting",
                crate::SetupPlan::Create => "making",
            };
            alloc::format!("{action}: {:?}: {:?}", step.stage, step.why).into_bytes().into_boxed_slice()
        }
        crate::SetupStopped::Removal { errno, .. } => {
            alloc::format!("removing the handover: forge {forge}'s admin_token: errno {errno}")
                .into_bytes()
                .into_boxed_slice()
        }
        crate::SetupStopped::Unreadable { why, .. } => {
            alloc::format!("the handover is unreadable: forge {forge}'s admin_token: {why:?}")
                .into_bytes()
                .into_boxed_slice()
        }
    };
    bounded_reason(text)
}
/// The client's setup reason is at most 256 bytes, including UTF-8 boundaries.
#[expect(clippy::disallowed_methods, reason = "the diagnostic boundary cuts text at a UTF-8 character boundary")]
pub(super) fn bounded_reason(text: Box<[u8]>) -> Box<[u8]> {
    let text = core::str::from_utf8(&text).expect("diagnostic is UTF-8");
    let mut end = text.len().min(256);
    while !text.is_char_boundary(end) {
        end = end.checked_sub(1).expect("a character starts before the bound");
    }
    Box::from(text.as_bytes().get(..end).expect("bounded character boundary"))
}
/// The box's preparation may create a fresh token on every ask.
pub(crate) fn started(engine: &mut crate::engine::Engine, forge: u16) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() {
        return;
    }
    if engine.setup.lifecycle.local(forge).is_none() {
        // temper-15 retains old-form access; 14a2 moves the template to records and removes the box's reconcile step.
        state(engine, forge, domain::Connection::NotConnected);
        return;
    }
    let local = engine.setup.lifecycle.local_mut(forge).expect("local start");
    local.trigger = Trigger::Asked;
    local.pending = false;
    if !local.file.idle() || busy(engine, forge) {
        local_stopped(engine, forge, crate::SetupStopped::Busy);
        return;
    }
    let run = next_run(&mut engine.setup.lifecycle);
    engine.setup.lifecycle.local_mut(forge).expect("local start").file = FileStep::Reading { run, sent: false };
    state(engine, forge, domain::Connection::Connecting);
}
pub(crate) fn take_read(engine: &mut crate::engine::Engine) -> Option<crate::HandoverRead> {
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() {
        return None;
    }
    for configured in &engine.workers.forges {
        let forge = configured.number;
        if crate::forge::ended(engine, forge) {
            continue;
        }
        if let Some(local) = engine.setup.lifecycle.local_mut(forge)
            && let FileStep::Reading { run, sent } = &mut local.file
            && !*sent
        {
            *sent = true;
            return Some(crate::HandoverRead {
                forge,
                run: *run,
                path: local.path.clone(),
                max: engine.setup.value_bytes,
            });
        }
    }
    None
}
pub(crate) fn read(
    engine: &mut crate::engine::Engine,
    forge: u16,
    run: Token,
    answer: Result<Option<Secret>, crate::HandoverUnread>,
) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() {
        return;
    }
    let Some(local) = engine.setup.lifecycle.local_mut(forge) else { return };
    let expected = match &local.file {
        FileStep::Reading { run: active, sent } => *sent && *active == run,
        FileStep::Idle | FileStep::Read(_) | FileStep::Removing { .. } | FileStep::Removed { .. } => false,
    };
    if !expected {
        return;
    }
    local.file = FileStep::Read(match answer {
        Ok(value) => {
            // Shell inputs are bounded again at the service boundary before retention.
            if let Some(value) = &value
                && value.expose().len() > usize::try_from(engine.setup.value_bytes).expect("record bound")
            {
                Err(crate::HandoverUnread::TooLarge)
            } else {
                Ok(value)
            }
        }
        Err(why) => Err(why),
    });
}
pub(crate) fn take_removal(engine: &mut crate::engine::Engine) -> Option<crate::HandoverRemoval> {
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() {
        return None;
    }
    for configured in &engine.workers.forges {
        let forge = configured.number;
        if crate::forge::ended(engine, forge) {
            continue;
        }
        if let Some(local) = engine.setup.lifecycle.local_mut(forge)
            && let FileStep::Removing { run, sent } = &mut local.file
            && !*sent
        {
            *sent = true;
            return Some(crate::HandoverRemoval { forge, run: *run, path: local.path.clone() });
        }
    }
    None
}
pub(crate) fn removed(engine: &mut crate::engine::Engine, forge: u16, run: Token, result: Result<(), i32>) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    if engine.setup.closing || engine.drain.is_some() || engine.end.is_some() {
        return;
    }
    let Some(local) = engine.setup.lifecycle.local_mut(forge) else { return };
    let expected = match &local.file {
        FileStep::Removing { run: active, sent } => *sent && *active == run,
        FileStep::Idle | FileStep::Read(_) | FileStep::Reading { .. } | FileStep::Removed { .. } => false,
    };
    if !expected {
        return;
    }
    local.file = FileStep::Removed { run, result };
}
/// Shell completions wait for the ordinary step's output room before doing work.
fn file_answer(engine: &mut crate::engine::Engine, forge: u16) -> bool {
    let Some(local) = engine.setup.lifecycle.local_mut(forge) else { return false };
    if !local.file.completed() {
        return false;
    }
    match core::mem::replace(&mut local.file, FileStep::Idle) {
        FileStep::Read(Ok(value)) => {
            local.handover = value;
            start_local(engine, forge);
        }
        FileStep::Read(Err(why)) => {
            let path = local.path.clone();
            local_stopped(engine, forge, crate::SetupStopped::Unreadable { path, why });
        }
        FileStep::Removed { run, result } => {
            let path = local.path.clone();
            engine.setup.runs.remove(&run).expect("removal waits after TakeOver Done");
            match result {
                Ok(()) => reconcile_local(engine, forge),
                Err(errno) => local_stopped(engine, forge, crate::SetupStopped::Removal { path, errno }),
            }
        }
        FileStep::Idle | FileStep::Reading { .. } | FileStep::Removing { .. } => unreachable!("completed file IO"),
    }
    true
}

pub(crate) fn first(engine: &mut crate::engine::Engine, forge: u16, org: Box<[u8]>, repository: Box<[u8]>) {
    if crate::forge::ended(engine, forge) {
        return;
    }
    if let Some(local) = engine.setup.lifecycle.local_mut(forge) {
        local.first = Some(Intent { org, repository, label: b"temper-host".as_slice().into() });
    }
}

fn reachable_failure(why: reconcile::Why) -> domain::ReachableAnswer {
    let reason = match why {
        reconcile::Why::Busy => return domain::ReachableAnswer::Refused(domain::ForgeRefusal::Busy),
        reconcile::Why::Missing | reconcile::Why::Refused(404) => domain::AdoptionRefusal::NotFound,
        reconcile::Why::Refused(401 | 403) | reconcile::Why::NotAdmin => domain::AdoptionRefusal::NoAccess,
        reconcile::Why::Refused(_) => domain::AdoptionRefusal::Refused,
        reconcile::Why::Down => domain::AdoptionRefusal::Unreachable,
        reconcile::Why::BotIsAdmin
        | reconcile::Why::Unmarked
        | reconcile::Why::OtherUser
        | reconcile::Why::OtherApp
        | reconcile::Why::KeyForm
        | reconcile::Why::Unknown
        | reconcile::Why::Crowded
        | reconcile::Why::Unexpected => domain::AdoptionRefusal::InvalidAnswer,
    };
    domain::ReachableAnswer::ForgeRefused(reason)
}

fn reached(engine: &mut crate::engine::Engine, token: Token, outcome: reconcile::Outcome) {
    let reconcile::Outcome::Reachable { rows, more } = outcome else { unreachable!("Reachable returns a page") };
    engine.setup.runs.remove(&token);
    let bound = usize::try_from(engine.env.limits.forge.name_bytes).expect("name bound");
    let mut reached = List::with_capacity(u32::try_from(rows.len()).expect("bounded page"));
    for row in rows {
        if row.owner.len() > bound || row.name.len() > bound {
            super::public::reached(
                engine,
                token,
                domain::ReachableAnswer::ForgeRefused(domain::AdoptionRefusal::TooLarge),
            );
            return;
        }
        reached
            .push(domain::Reached {
                id: row.id,
                owner_id: row.owner_id,
                owner: row.owner,
                name: row.name,
                private: row.private,
            })
            .expect("bounded page");
    }
    super::public::reached(
        engine,
        token,
        domain::ReachableAnswer::Page(domain::Page { rows: reached.into_boxed(), more }),
    );
}

fn missing_admin(engine: &mut crate::engine::Engine, forge: u16) {
    if engine.setup.lifecycle.local(forge).expect("local start").source == Source::Managed {
        state(engine, forge, domain::Connection::NotConnected);
    } else {
        local_stopped(engine, forge, crate::SetupStopped::NotPrepared);
    }
}
fn keep_admin(engine: &mut crate::engine::Engine, run: Token) {
    let forge = engine.setup.runs.get(&run).expect("admin run").forge;
    if crate::forge::ended(engine, forge) {
        refused(engine, run, forge, Box::from(b"forge ended".as_slice()));
        return;
    }
    let Some(super::public::Caller::Connect { pasted: Some(super::public::Pasted::Admin(value)), .. }) =
        engine.setup.public.get(&run)
    else {
        unreachable!("admin run holds its paste")
    };
    let value = value.clone();
    keep(engine, forge, Item::AdminToken, value.expose(), Purpose::Admin(run));
}
fn admin_create(engine: &mut crate::engine::Engine, token: Token) {
    let run = engine.setup.runs.get_mut(&token).expect("admin reconcile waits for Origin");
    let forge = run.forge;
    let Some(first) = &run.first else {
        keep_admin(engine, token);
        return;
    };
    let Some(content) = temper_forge_setup::starter_workflow(&first.label) else {
        refused(engine, token, forge, Box::from(b"the runner label is invalid".as_slice()));
        return;
    };
    let plan = host::Plan::Create {
        forge,
        org: first.org.clone(),
        repository: first.repository.clone(),
        files: Box::new([host::File { path: temper_forge_setup::STARTER_PATH.into(), content }]),
    };
    run.kind = Kind::AdminCreate;
    let local = engine.setup.lifecycle.local(forge).expect("managed configuration");
    assert!(
        skein_forge_setup::given::check_create(&local.limits, engine.setup.env.limits.string_bytes, &plan).is_ok(),
        "Connect admission and starter profile prove Create inputs"
    );
    super::run_plan(engine, token, plan);
}
/// A failed store completion ends held callers before the service stops.
pub(super) fn writes_failed(engine: &mut crate::engine::Engine) {
    let mut runs = List::with_capacity(engine.setup.runs.len());
    for (token, run) in &engine.setup.runs {
        if run.kind == Kind::AdminReconcile || run.kind == Kind::AdminCreate {
            runs.push(*token).expect("active admin runs");
        }
    }
    for token in runs.into_boxed() {
        let forge = engine.setup.runs.get(&token).expect("active run").forge;
        // A failed store stops iteration, so the root cannot route this end's erase.
        if let Some(super::public::Caller::Connect { number, .. }) = engine.setup.public.get(&token) {
            let number = *number;
            super::public::erase(engine, number);
        }
        refused(engine, token, forge, Box::from(b"forge records could not be kept".as_slice()));
    }
    engine.setup.writes = Map::with_capacity(
        engine.setup.env.limits.runs.checked_add(engine.setup.env.limits.forges).expect("setup writes"),
    );
}

fn admin_done(engine: &mut crate::engine::Engine, token: Token, outcome: reconcile::Outcome) {
    let run = engine.setup.runs.get(&token).expect("ended admin run");
    let (forge, kind) = (run.forge, run.kind);
    if kind == Kind::AdminReconcile {
        let origin = records::origin(connection::settings::Kind::Forgejo, &configured(engine, forge).api).encode();
        keep(engine, forge, Item::Origin, &origin, Purpose::AdminOrigin(token));
        return;
    }
    if kind == Kind::AdminCreate {
        let reconcile::Outcome::Created { id } = outcome else { unreachable!("Create returns its repository") };
        engine.events.push(root::Event::Forge(Box::new(domain::Event::FirstMade { forge, id: id.get() })));
        keep_admin(engine, token);
        return;
    }
    unreachable!("admin Done belongs to Reconcile or Create");
}
