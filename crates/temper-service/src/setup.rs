//! Durable setup needs and held manifest pages (protocol/forge.md, 2.6).
pub(crate) mod diagnostics;
pub(crate) mod lifecycle;
pub(crate) mod public;
mod routes;
pub(crate) use routes::{MANIFEST_SCRIPT_HASH, MANIFEST_STYLE_HASH, PAGE_STYLE_HASH, busy};
mod settings;
use alloc::boxed::Box;
use jig_protocol_store::Event as StoreEvent;
use lifecycle::{Kind, Purpose, Run};
pub(crate) use settings::prepare;
use skein_forge_setup::{host, reconcile};
use skein_lib::{CryptoRng, Duration, Env, Map, Queue, Secret, Time, Token, Wall};
use temper_engine_domain as root;
use temper_protocol_records::Item;

/// Plans are staged in the protocol layer: the root names them, never their credentials.
pub(crate) struct Layer {
    component: Option<host::Host>,
    pending: Option<(host::Settings, CryptoRng)>,
    waiting_for_webhooks: bool,
    pub(crate) env: Env<host::Limits>,
    pub(crate) above: Queue<host::Event>,
    pub(crate) io: Queue<skein_io::Request>,
    pub(crate) closed: bool,
    closing: bool,
    served: Map<u16, ()>,
    unserved: Map<u16, &'static [u8]>,
    pub(crate) staged: Map<Token, host::Plan>,
    runs: Map<Token, Run>,
    public: Map<Token, public::Caller>,
    backs: Queue<public::ReturnTo>,
    pastes: Map<u64, u16>,
    file_api: Map<u16, Secret>,
    erases: Map<Token, u16>,
    lifecycle: lifecycle::Lifecycle,
    writes: Map<Token, Write>,
    next_write: u64,
    pages: Queue<Page>,
    waiting: Queue<Waiting>,
    held: Queue<Held>,
    listener: Box<[u8]>,
    life: Duration,
    value_bytes: u32,
    #[cfg(test)]
    pub(crate) answers: Queue<(Token, host::Answer)>,
}
struct Write {
    purpose: Purpose,
    number: u64,
    value: Secret,
}
struct Page {
    forge: u16,
    state: Box<[u8]>,
    bytes: Box<[u8]>,
    until: Time,
}
/// GitHub's setup visit for a forge busy with its one run, held until that run ends (client/forge.md, 9.1).
struct Waiting {
    forge: u16,
    call: Token,
    installation: core::num::NonZeroU64,
    state: Box<[u8]>,
    back: Option<Box<[u8]>>,
    until: Time,
}
/// A person's Install that met only the engine's own run on its forge, held until that run ends
/// (protocol/setup.md, 5.1): the forge's one person's request, so at most one per forge.
struct Held {
    run: Token,
    forge: u16,
    until: Time,
}
// skein host/states.rs:53 spells its 16 random bytes as 32 hex bytes.
pub(crate) const STATE_TEXT_BYTES: u64 = 32;
/// Whole host plus the allocations retained by this owner (protocol/forge.md, 2.6).
#[expect(clippy::too_many_lines, reason = "sum each retained setup owner and transient allocation")]
pub(crate) fn worst_case(
    config: &crate::settings::Engine,
    forges: &skein_forge_connection::Settings,
    batch: u32,
) -> Option<u64> {
    use temper_engine_domain_forge::{BOT_LOGIN_SUFFIX, STOPPED_TEXT_BYTES};
    let (mut settings, l) = settings::fixed(config, forges).ok()?;
    // Port 0 may bind any port: price the longest hook URL before admission.
    for index in 0..settings.forges.len() {
        if let host::Kind::Forgejo(given) = &mut settings.forges.get_mut(index)?.kind
            && let Some(listener) = &mut given.listener
            && listener.port() == 0
        {
            listener.set_port(u16::MAX);
        }
    }
    let mut listener_bytes = 0_u64;
    let mut web_bytes = 0_u64;
    for forge in &settings.forges {
        if let host::Kind::GitHub(given) = &forge.kind {
            listener_bytes = listener_bytes.max(u64::try_from(given.url.len()).ok()?);
            web_bytes = web_bytes.max(u64::try_from(given.web.len()).ok()?);
        }
    }
    let host = host::worst_case(&l, &settings)?.checked_add(listener_bytes)?; // Includes pending fixed Settings, moved into Host::new.
    let installations = config.limits.forge_component.installations;
    let count = l.forges.checked_mul(installations)?;
    let above = batch.checked_mul(host::MAX_OUT.above)?;
    let below = batch.checked_mul(host::MAX_OUT.below)?;
    let writes = l.runs.checked_add(l.forges)?;
    let name = u64::from(config.limits.forge.name_bytes);
    let page = u64::from(l.body_bytes).checked_add(STATE_TEXT_BYTES)?;
    // The kit's Manifest outcome: its form target and manifest, each at most body_bytes (5.1).
    let form = u64::from(l.body_bytes).checked_mul(2)?.checked_add(STATE_TEXT_BYTES)?;
    let wanted = skein_forge::github::setup::PERMISSIONS.len().checked_add(skein_forge::github::setup::EVENTS.len())?;
    let lacking = skein_lib::List::<skein_forge_setup::github::Lacking>::worst_case(u32::try_from(wanted).ok()?)?
        .checked_add(diagnostics::worst_case()?)?;
    // A host page can hold decoder-sized names before the routing checks the wire's bounds.
    let reachable = u64::from(config.limits.picker.page).checked_mul(
        u64::try_from(size_of::<skein_forge_setup::reachable::Reached>())
            .ok()?
            .checked_add(u64::from(l.string_bytes).checked_mul(2)?)?,
    )?;
    // Handed-up items, pages/links with state, failure subjects, or missing access.
    let payload = u64::from(l.body_bytes_in)
        .max(form)
        .max(u64::from(config.limits.records.value_bytes))
        .max(name)
        .max(lacking)
        .max(reachable);
    let mut local_bytes = 0_u64;
    for identity in &config.identities {
        if !config.records_only.contains(&identity.forge) {
            continue;
        }
        let mut bot = 0_usize;
        for setup in &config.setup {
            if setup.entry.forge == identity.forge {
                bot = setup.entry.bot.login.len();
            }
        }
        let login = if identity.login.is_empty() { bot } else { identity.login.len() };
        local_bytes = local_bytes.checked_add(
            u64::try_from(
                bot.checked_add(login)?
                    .checked_add(identity.author.name.len())?
                    .checked_add(identity.author.email.len())?,
            )
            .ok()?,
        )?;
    }
    let mut handovers = 0_u64;
    for setup in &config.setup {
        if config.records_only.contains(&setup.entry.forge) {
            // Checked input, Files, Local, staged plan and shell read scratch overlap.
            // The path is retained locally and cloned for a correlated read/removal.
            let path = u64::try_from(setup.entry.directory.len()).ok()?.checked_add(12)?;
            handovers = handovers
                .checked_add(u64::try_from(size_of::<crate::HandoverRead>()).ok()?)?
                .checked_add(path.checked_mul(3)?)?
                .checked_add(u64::from(config.limits.records.value_bytes).checked_mul(5)?)?;
        }
    }
    // Retained strings, codec parts and their encoded copy can coexist.
    // Each HTTP call may retain its own served manifest page. The policy's
    // measured buffer and header copy overlap; 256 covers its fixed words.
    let responses = if listener_bytes == 0 {
        0
    } else {
        u64::from(crate::limits::server(&config.jig.limits).connections)
            .checked_mul(u64::from(l.body_bytes).checked_add(web_bytes.checked_add(256)?.checked_mul(2)?)?)?
    };
    host.checked_add(responses)?
        .checked_add(handovers)?
        // Pending/active intent names and the Create plan clone coexist.
        .checked_add(u64::from(l.forges).checked_mul(name.checked_mul(4)?.checked_add(64)?)?)?
        .checked_add(local_bytes.checked_mul(3)?)?
        // Token, encoded identity, and copied parts overlap during the records handoff.
        .checked_add(u64::from(config.limits.records.value_bytes).checked_mul(4)?)?
        .checked_add(Map::<u16, ()>::worst_case(l.forges)?)? // served
        .checked_add(Map::<u16, lifecycle::Reconcile>::worst_case(l.forges)?)? // lifecycle.reconcile
        .checked_add(Map::<u16, ()>::worst_case(l.forges)?)? // lifecycle.deleting
        .checked_add(Map::<u16, &'static [u8]>::worst_case(l.forges)?)? // unserved
        .checked_add(Map::<u16, lifecycle::Adoption>::worst_case(l.forges)?)?
        // temper-15: an old-form fake holds its handover and file access until adoption ends.
        .checked_add(
            u64::from(
                40_u32
                    .checked_add(config.limits.forge_component.token_bytes)?
                    .checked_add(config.limits.forge_component.hook_secret_bytes)?,
            )
            .checked_mul(u64::from(l.forges))?,
        )?
        // Each staged start retains its restored ids. check_adopted's sorted
        // (id,index) tuples coexist with the snapshot; host::worst_case owns running plans.
        .checked_add(
            u64::from(config.limits.forge.client.repositories)
                .checked_mul(u64::try_from(size_of::<u64>().checked_add(size_of::<(u64, u32)>())?).ok()?)?,
        )?
        .checked_add(
            u64::from(config.limits.forge.client.repositories).checked_mul(8)?.checked_mul(u64::from(l.runs))?,
        )?
        .checked_add(Map::<Token, host::Plan>::worst_case(l.runs)?)?
        .checked_add(Map::<Token, Run>::worst_case(l.runs)?)?
        // One Get bundle is transient while its decoded PEM takes the final run slot.
        .checked_add(4096)?
        .checked_add(Map::<Token, public::Caller>::worst_case(l.runs)?)?
        .checked_add(Map::<u64, u16>::worst_case(l.forges)?)?
        .checked_add(Map::<u16, Secret>::worst_case(l.forges)?)?
        .checked_add(u64::from(l.forges).checked_mul(u64::from(l.token_bytes))?)?
        .checked_add(Map::<Token, u16>::worst_case(l.forges)?)?
        .checked_add(u64::from(l.runs).checked_mul(4096 + 512)?)?
        .checked_add(Queue::<public::ReturnTo>::worst_case(l.states)?)?
        .checked_add(u64::from(l.states).checked_mul(STATE_TEXT_BYTES + 512)?)?
        .checked_add(
            name.checked_add(u64::try_from(BOT_LOGIN_SUFFIX.len()).ok()?)?
                .max(STOPPED_TEXT_BYTES)
                .checked_mul(u64::from(l.runs))?,
        )?
        .checked_add(Map::<(u16, u64), ()>::worst_case(count)?)? // lifecycle.deleted
        .checked_add(Map::<(u16, u64), temper_engine_domain_forge::Installation>::worst_case(count)?)?
        .checked_add(name.checked_mul(u64::from(count))?)? // installation account logins
        .checked_add(Map::<Token, Write>::worst_case(writes)?)?
        // Encoded record, including VERSION; the store's own copy stays in store::worst_case.
        .checked_add(u64::from(config.limits.records.value_bytes).checked_mul(u64::from(writes))?)?
        .checked_add(Queue::<Page>::worst_case(l.states)?)?
        // Held setup visits: each its state and a back as long as the backs' own.
        .checked_add(Queue::<Waiting>::worst_case(l.states)?)?
        .checked_add(Queue::<Held>::worst_case(l.forges)?)?
        .checked_add(u64::from(l.states).checked_mul(STATE_TEXT_BYTES + 512)?)?
        .checked_add(page.checked_mul(u64::from(l.states))?)?
        .checked_add(Queue::<host::Event>::worst_case(above)?)?
        .checked_add(payload.checked_mul(u64::from(above))?)?
        .checked_add(Queue::<skein_io::Request>::worst_case(below)?)?
        .checked_add(u64::from(l.io.output).checked_mul(u64::from(below))?)?
        .checked_add(skein_lib::List::<u16>::worst_case(l.forges)?)? // advance's forge-number scratch
        .checked_add(skein_lib::List::<u64>::worst_case(installations)?)? // accepted()
        .checked_add(skein_lib::List::<u64>::worst_case(installations)?)? // deleted
        .checked_add(skein_lib::List::<u64>::worst_case(installations)?)? // remaining
        // accepted_item's raw little-endian ids coexist with the encoded Write above.
        .checked_add(u64::try_from(size_of::<u64>()).ok()?.checked_mul(u64::from(installations))?)
}

impl Layer {
    #[cfg(test)]
    pub(crate) fn startup_restored(&self) -> bool {
        self.lifecycle.startup_restored()
    }
    /// Consume the listener's typed bind result before any setup plan can copy Given.
    pub(crate) fn webhooks_listening(&mut self, addr: core::net::SocketAddr) {
        if let Some((settings, _)) = &mut self.pending {
            for index in 0..settings.forges.len() {
                if let host::Kind::Forgejo(given) = &mut settings.forges.get_mut(index).expect("configured forge").kind
                    && given.listener.is_some()
                {
                    given.listener = Some(addr);
                }
            }
        }
        self.waiting_for_webhooks = false;
    }
    pub(crate) fn records_only(&self, forge: u16) -> bool {
        self.lifecycle.local(forge).is_some()
    }
    pub(crate) fn new(
        settings: host::Settings,
        limits: host::Limits,
        crypto: CryptoRng,
        life: Duration,
        value_bytes: u32,
        batch: u32,
        installations: u32,
    ) -> Self {
        let mut listener: Box<[u8]> = Box::new([]);
        let mut served = Map::with_capacity(limits.forges);
        for forge in &settings.forges {
            served.insert(forge.number(), ()).expect("configured host forges");
            if let host::Kind::GitHub(given) = &forge.kind {
                listener.clone_from(&given.url);
            }
        }
        Self {
            listener,
            component: None,
            pending: Some((settings, crypto)),
            waiting_for_webhooks: false,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            above: Queue::with_capacity(host::MAX_OUT.above.checked_mul(batch).expect("host output room")),
            io: Queue::with_capacity(host::MAX_OUT.below.checked_mul(batch).expect("host IO room")),
            closed: false,
            closing: false,
            served,
            unserved: Map::with_capacity(limits.forges),
            staged: Map::with_capacity(limits.runs),
            runs: Map::with_capacity(limits.runs),
            public: Map::with_capacity(limits.runs),
            backs: Queue::with_capacity(limits.states),
            pastes: Map::with_capacity(limits.forges),
            file_api: Map::with_capacity(limits.forges),
            erases: Map::with_capacity(limits.forges),
            lifecycle: lifecycle::Lifecycle::new(limits.forges, installations),
            writes: Map::with_capacity(limits.runs.checked_add(limits.forges).expect("setup writes")),
            next_write: 1,
            pages: Queue::with_capacity(limits.states),
            waiting: Queue::with_capacity(limits.states),
            held: Queue::with_capacity(limits.forges),
            life,
            value_bytes,
            #[cfg(test)]
            answers: Queue::with_capacity(64),
        }
    }
    pub(crate) fn room(&self) -> bool {
        self.batch_room(1)
    }
    pub(crate) fn batch_room(&self, batch: u32) -> bool {
        self.above.room() >= host::MAX_OUT.above.checked_mul(batch).expect("host batch above")
            && self.io.room() >= host::MAX_OUT.below.checked_mul(batch).expect("host batch IO")
    }
    pub(crate) fn down(&mut self, request: host::Request) {
        #[cfg(test)]
        if let host::Request::Answer { run, answer } = &request {
            self.answers.push((*run, answer.clone()));
        }
        self.component.as_mut().expect("host started after records load").down(
            &self.env,
            request,
            &mut self.above,
            &mut self.io,
        );
    }
    pub(crate) fn up(&mut self, event: skein_io::Event) {
        self.component.as_mut().expect("host owns IO").up(
            &self.env,
            crate::accounts::untag(event),
            &mut self.above,
            &mut self.io,
        );
    }
    pub(crate) fn close(&mut self) {
        if self.closing || self.closed || !self.room() {
            return;
        }
        self.closing = true;
        self.lifecycle.adoptions = Map::with_capacity(self.env.limits.forges);
        self.lifecycle.reconcile = Map::with_capacity(self.env.limits.forges);
        self.pages = Queue::with_capacity(self.env.limits.states);
        self.waiting = Queue::with_capacity(self.env.limits.states);
        self.held = Queue::with_capacity(self.env.limits.forges);
        self.backs = Queue::with_capacity(self.env.limits.states);
        self.public = Map::with_capacity(self.env.limits.runs);
        self.file_api = Map::with_capacity(self.env.limits.forges);
        self.staged = Map::with_capacity(self.env.limits.runs);
        if self.component.is_some() {
            self.down(host::Request::Close);
        } else {
            self.pending = None;
            self.closed = true;
        }
    }
    #[expect(clippy::manual_map, reason = "the step subset uses explicit matches")]
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        let host = match &self.component {
            Some(host) => host.next_deadline(),
            None => None,
        };
        let mut next = crate::accounts::earliest(
            host,
            match self.pages.iter().next() {
                Some(page) => Some(page.until),
                None => None,
            },
        );
        for waiting in &self.waiting {
            next = crate::accounts::earliest(next, Some(waiting.until));
        }
        for held in &self.held {
            next = crate::accounts::earliest(next, Some(held.until));
        }
        next
    }
    pub(crate) fn work_pending(&self, now: Time) -> bool {
        !self.above.is_empty() || !self.io.is_empty() || due(self.next_deadline(), now) || self.waiting_ready()
    }
    fn waiting_ready(&self) -> bool {
        for waiting in &self.waiting {
            if !self.busy(waiting.forge) {
                return true;
            }
        }
        for held in &self.held {
            if !self.busy(held.forge) {
                return true;
            }
        }
        false
    }
    /// The forge's one run: a live run of it, or its removal (protocol/setup.md, 6.5).
    pub(crate) fn busy(&self, forge: u16) -> bool {
        if self.lifecycle.deleting.contains_key(&forge) {
            return true;
        }
        for (_, run) in &self.runs {
            if run.forge == forge && run.kind != Kind::Reachable {
                return true;
            }
        }
        false
    }
    pub(super) fn reading(&self, forge: u16) -> bool {
        for (_, run) in &self.runs {
            if run.forge == forge && run.kind == Kind::Reachable {
                return true;
            }
        }
        false
    }
    /// Busy with the engine's own runs alone: the reconcile after a connect, at a start or on a hint.
    pub(super) fn busy_internally(&self, forge: u16) -> bool {
        if self.lifecycle.deleting.contains_key(&forge) {
            return false;
        }
        let mut any = false;
        for (_, run) in &self.runs {
            if run.forge == forge && run.kind != Kind::Reachable {
                if !run.internal {
                    return false;
                }
                any = true;
            }
        }
        any
    }
    pub(crate) fn lifecycle_ready(&self, engine: &crate::engine::Engine) -> bool {
        !self.closing
            && self.component.is_some()
            && self.lifecycle.ready(&self.runs, self.env.limits.runs, &engine.domain)
    }
    /// A served page stays available for reload, until its own clock/count evicts it.
    pub(crate) fn page(&self, forge: u16, state: &[u8], now: Time) -> Option<&[u8]> {
        for page in &self.pages {
            if page.forge == forge && page.state.as_ref() == state && now < page.until {
                return Some(&page.bytes);
            }
        }
        None
    }
    fn take_page(&mut self, forge: u16, state: &[u8], now: Time) -> bool {
        let mut found = false;
        for _ in 0..self.pages.len() {
            let page = self.pages.pop().expect("page count");
            if page.until <= now {
                continue;
            }
            if page.forge == forge && page.state.as_ref() == state {
                found = true;
            } else {
                self.pages.push(page);
            }
        }
        found
    }
    fn hold(&mut self, forge: u16, state: Box<[u8]>, bytes: Box<[u8]>) {
        if self.pages.room() == 0 {
            let _oldest = self.pages.pop();
        }
        self.pages.push(Page { forge, state, bytes, until: self.env.now.saturating_add(self.life) });
    }
}
/// Name the forge before gating, including plans whose connected path is staged later.
fn forge(plan: &host::Plan) -> u16 {
    match plan {
        host::Plan::Manifest { forge, .. }
        | host::Plan::Convert { forge, .. }
        | host::Plan::Install { forge }
        | host::Plan::Accept { forge, .. }
        | host::Plan::Reconcile { forge, .. }
        | host::Plan::Adopt { forge, .. }
        | host::Plan::Create { forge, .. }
        | host::Plan::Paste { forge, .. }
        | host::Plan::Check { forge }
        | host::Plan::TakeOver { forge, .. }
        | host::Plan::Reachable { forge, .. } => *forge,
    }
}
pub(crate) fn request(engine: &mut crate::engine::Engine, run: Token) {
    let plan = engine.setup.staged.remove(&run).expect("root names a staged setup plan");
    let number = forge(&plan);
    if crate::forge::ended(engine, number) {
        ended(engine, run, root::SetupEnd::Stopped { forge: number, reason: Box::from(b"forge ended".as_slice()) });
    } else if engine.drain.is_some() || engine.setup.closing || engine.setup.component.is_none() {
        ended(
            engine,
            run,
            root::SetupEnd::Stopped { forge: number, reason: Box::from(b"setup host is not running".as_slice()) },
        );
    } else if let Some(host) = engine.forge.unreachable(number) {
        ended(engine, run, root::SetupEnd::Stopped { forge: number, reason: crate::forge::unreachable_text(&host) });
    } else if !engine.setup.served.contains_key(&number) {
        let reason = match engine.setup.unserved.get(&number) {
            Some(reason) => *reason,
            None => b"setup host has no entry for this forge",
        };
        ended(engine, run, root::SetupEnd::Stopped { forge: number, reason: Box::from(reason) });
    } else if matches_app_plan(&plan) && lifecycle::has_app(engine, number) {
        ended(
            engine,
            run,
            root::SetupEnd::Stopped {
                forge: number,
                reason: Box::from(b"forge holds App records; Forget before making a new App".as_slice()),
            },
        );
    } else if is_install(&plan) && engine.setup.busy_internally(number) && engine.setup.held.room() > 0 {
        // An Install's link writes nothing: it waits for the engine's own run, never a person's.
        let until = engine.env.now.saturating_add(engine.setup.life);
        engine.setup.held.push(Held { run, forge: number, until });
    } else if (if let host::Plan::Reachable { .. } = &plan {
        engine.setup.reading(number)
    } else {
        lifecycle::busy(engine, number)
    }) || engine.setup.runs.len() == engine.setup.env.limits.runs
    {
        ended(
            engine,
            run,
            root::SetupEnd::Stopped { forge: number, reason: Box::from(b"setup host: busy".as_slice()) },
        );
    } else {
        let kind = lifecycle::kind(&plan);
        engine
            .setup
            .runs
            .insert(
                run,
                Run {
                    first: None,
                    forge: number,
                    kind,
                    internal: false,
                    previous: engine.domain.forge_connection(number),
                },
            )
            .expect("admitted host run");
        if kind == Kind::Convert {
            lifecycle::state(engine, number, temper_engine_domain_forge::Connection::Connecting);
        }
        run_plan(engine, run, plan);
    }
}
/// Run each held Install whose forge is free, or end it busy past the state's life.
fn release(engine: &mut crate::engine::Engine) {
    for _ in 0..engine.setup.held.len() {
        if engine.events.room() < 4 || !engine.setup.room() {
            break;
        }
        let held = engine.setup.held.pop().expect("held count");
        if !engine.setup.busy(held.forge) {
            engine
                .setup
                .staged
                .insert(held.run, host::Plan::Install { forge: held.forge })
                .expect("held run was staged");
            request(engine, held.run);
        } else if engine.env.now < held.until && engine.setup.busy_internally(held.forge) {
            engine.setup.held.push(held);
        } else {
            let reason = Box::from(b"setup host: busy".as_slice());
            ended(engine, held.run, root::SetupEnd::Stopped { forge: held.forge, reason });
        }
    }
}
fn is_install(plan: &host::Plan) -> bool {
    match plan {
        host::Plan::Install { .. } => true,
        host::Plan::Manifest { .. }
        | host::Plan::Convert { .. }
        | host::Plan::Accept { .. }
        | host::Plan::Reconcile { .. }
        | host::Plan::Paste { .. }
        | host::Plan::Check { .. }
        | host::Plan::TakeOver { .. }
        | host::Plan::Adopt { .. }
        | host::Plan::Create { .. }
        | host::Plan::Reachable { .. } => false,
    }
}
fn matches_app_plan(plan: &host::Plan) -> bool {
    match plan {
        host::Plan::Manifest { .. } | host::Plan::Convert { .. } => true,
        host::Plan::Install { .. }
        | host::Plan::Accept { .. }
        | host::Plan::Reconcile { .. }
        | host::Plan::Paste { .. }
        | host::Plan::Check { .. }
        | host::Plan::TakeOver { .. }
        | host::Plan::Adopt { .. }
        | host::Plan::Create { .. }
        | host::Plan::Reachable { .. } => false,
    }
}
fn ended(engine: &mut crate::engine::Engine, run: Token, end: root::SetupEnd) {
    if public::ended(engine, run, &end) {
        return;
    }
    engine.events.push(root::Event::Setup { run, end });
}
fn begin(engine: &mut crate::engine::Engine) {
    if !engine.forge_records.loading()
        && !engine.setup.waiting_for_webhooks
        && engine.end.is_none()
        && engine.drain.is_none()
        && let Some((settings, crypto)) = engine.setup.pending.take()
    {
        match host::Host::new(&engine.setup.env.limits, settings, crypto) {
            Ok(host) => engine.setup.component = Some(host),
            Err(why) => {
                let mut found = jig_protocol_config::Problems::empty();
                settings::problems(why, &mut found);
                engine.end = Some(crate::End::Stopped(crate::Stop::Start(found)));
            }
        }
    }
}
#[expect(clippy::manual_map, reason = "the step subset uses explicit matches")]
pub(crate) fn advance(engine: &mut crate::engine::Engine) {
    engine.setup.env.now = engine.env.now;
    engine.setup.env.wall = engine.env.wall;
    begin(engine);
    release(engine);
    routes::advance(engine);
    for _ in 0..engine.setup.pages.len() {
        if !due(
            match engine.setup.pages.iter().next() {
                Some(page) => Some(page.until),
                None => None,
            },
            engine.env.now,
        ) {
            break;
        }
        let _expired = engine.setup.pages.pop();
    }
    if engine.setup.room()
        && let Some(host) = &mut engine.setup.component
        && due(host.next_deadline(), engine.env.now)
    {
        host.fire(&engine.setup.env, &mut engine.setup.above, &mut engine.setup.io);
    }
    for _ in 0..engine.setup.above.len() {
        if !engine.setup.room()
            || !engine.store.room()
            || !engine.forge.room(2)
            || engine.events.room()
                < engine.forge_records.bounds.installations.checked_add(4).expect("setup event room")
            || engine.notes.room() == 0
        {
            break;
        }
        let event = engine.setup.above.pop().expect("host output count");
        if (engine.setup.closing || engine.end.is_some()) && event != host::Event::Closed {
            continue;
        }
        match event {
            host::Event::Need { run, need } => {
                let forge = engine.setup.runs.get(&run).expect("live setup run").forge;
                match need {
                    host::Need::Read(item) => {
                        let value = read_item(engine, run, forge, item);
                        engine.setup.down(host::Request::Answer { run, answer: host::Answer::Read(value) });
                    }
                    host::Need::Write { item, bytes } => {
                        lifecycle::keep(
                            engine,
                            forge,
                            item_record(item).expect("host never writes Pasted"),
                            bytes.expose(),
                            Purpose::Host(run),
                        );
                    }
                }
            }
            host::Event::Done { run, outcome } => lifecycle::done(engine, run, outcome),
            host::Event::Stop { run, failure } => lifecycle::stopped(engine, run, failure),
            host::Event::Closed => engine.setup.closed = true,
        }
        if engine.end.is_some() {
            break;
        }
    }
    lifecycle::advance(engine);
}
pub(crate) fn answer(engine: &mut crate::engine::Engine, event: StoreEvent) {
    if let StoreEvent::Erased { owner } = event {
        if let Some(forge) = engine.setup.erases.remove(&owner)
            && !engine.setup.closing
            && engine.drain.is_none()
            && engine.end.is_none()
        {
            lifecycle::connect(engine, forge);
        }
        return;
    }
    if let StoreEvent::Kept { owner } = event {
        let write = engine.setup.writes.remove(&owner).expect("setup keep owner");
        engine.forge_records.rows.push(write.number, write.value, engine.setup.value_bytes);
        if !engine.setup.closing {
            lifecycle::kept(engine, write.purpose);
        }
    } else {
        lifecycle::writes_failed(engine);
        crate::stop::unavailable(engine, b"forge records");
    }
}
fn item_record(item: skein_forge_setup::item::Item) -> Option<Item> {
    use skein_forge_setup::item::Item as I;
    Some(match item {
        I::AdminToken => Item::AdminToken,
        I::ApiToken => Item::ApiToken,
        I::GitToken => Item::GitToken,
        I::User => Item::User,
        I::WebhookSecret => Item::WebhookSecret,
        I::AppId => Item::AppId,
        I::AppSlug => Item::AppSlug,
        I::AppKey => Item::AppKey,
        I::Accepted => Item::Accepted,
        I::Pasted => return None,
    })
}
#[expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "failure has no credential; this is the diagnostic boundary"
)]
fn failure_text(failure: &reconcile::Failure) -> Box<[u8]> {
    alloc::format!(
        "setup {:?} {}: {:?}",
        failure.step,
        core::str::from_utf8(&failure.subject).unwrap_or("subject"),
        failure.why
    )
    .into_bytes()
    .into_boxed_slice()
}
fn due(at: Option<Time>, now: Time) -> bool {
    match at {
        Some(at) => at <= now,
        None => false,
    }
}
pub(crate) fn tag(mut request: skein_io::Request) -> skein_io::Request {
    if let skein_io::Request::Connect { owner, .. } = &mut request {
        assert!(owner.raw() < (1_u64 << 60_u32), "setup token fits prefix");
        *owner = Token::new(owner.raw() | crate::tokens::SETUP);
    }
    request
}
#[cfg(test)]
#[path = "tests/setup_scopes.rs"]
mod scopes_tests;
#[cfg(test)]
#[path = "tests/setup.rs"]
mod tests;

/// Every setup path, including internal follow-ups, crosses the same unresolved-host gate.
fn run_plan(engine: &mut crate::engine::Engine, run: Token, plan: host::Plan) {
    let number = forge(&plan);
    if crate::forge::ended(engine, number) {
        lifecycle::refused(engine, run, number, Box::from(b"forge ended".as_slice()));
    } else if let Some(host) = engine.forge.unreachable(number) {
        lifecycle::refused(engine, run, number, crate::forge::unreachable_text(&host));
    } else {
        engine.setup.down(host::Request::Run { run, plan });
    }
}

/// Connected Forgejo's legacy file input remains a host-local credential.
fn file_api_token(engine: &crate::engine::Engine, forge: u16) -> Option<&[u8]> {
    match engine.setup.file_api.get(&forge) {
        Some(value) => Some(value.expose()),
        None => None,
    }
}

#[expect(clippy::manual_map, reason = "the step subset uses explicit matches")]
fn read_item(
    engine: &crate::engine::Engine,
    run: Token,
    forge: u16,
    item: skein_forge_setup::item::Item,
) -> Option<Secret> {
    if let Some(public::Caller::Connect { pasted, .. }) = engine.setup.public.get(&run) {
        match pasted {
            Some(public::Pasted::App(value)) if item == skein_forge_setup::item::Item::Pasted => {
                return Some(value.clone());
            }
            Some(public::Pasted::Admin(value)) if item == skein_forge_setup::item::Item::AdminToken => {
                return Some(value.clone());
            }
            Some(public::Pasted::App(_) | public::Pasted::Admin(_)) | None => {}
        }
        if item == skein_forge_setup::item::Item::AdminToken {
            return None; // A caller's admin read never falls back to records or adoptions.
        }
    }
    if item == skein_forge_setup::item::Item::Pasted {
        return None;
    }
    let item = item_record(item).expect("record item");
    let files = match engine.setup.lifecycle.adoptions.get(&forge) {
        Some(adoption) => adoption.files.as_ref(),
        None => None,
    };
    let value = match (files, item) {
        (Some(adoption), Item::AdminToken) => Some(adoption.admin_token.expose()),
        (Some(adoption), Item::WebhookSecret) => match &adoption.access.webhook_secret {
            Some(secret) => Some(secret.expose()),
            None => None,
        },
        _ => match engine.forge_records.rows.forge(forge) {
            Some(rows) => rows.get(item).ok(),
            None => None,
        },
    };
    let value = match value {
        Some(value) => Some(value),
        None if item == Item::ApiToken => file_api_token(engine, forge),
        None => None,
    };
    match value {
        Some(bytes) => Some(Secret::new(Box::from(bytes))),
        None => None,
    }
}
