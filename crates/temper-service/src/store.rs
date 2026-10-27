//! The deployment store and its three file drivers (protocol/store.md, 3–5;
//! jig's protocol/store.md, 13).
#[cfg(test)]
mod baseline;
mod ranges;
pub mod route;
use jig_protocol_store as protocol;
use route::Up;
use skein_io::{
    file,
    file_layer::{self, FileIo},
    kernel::{Complete, Fd, Op, Submit},
};
use skein_lib::{Duration, Env, Map, Queue, Time, Token, Wall};
use temper_engine_domain as root;
use temper_engine_domain_forge as forge;
/// The four real owners admitted at every shell open (jig's store.md, 10.3).
pub const PATCH_SETS: protocol::PatchSets = protocol::PatchSets {
    sets: &[
        protocol::PatchSet {
            owner: protocol::Owner::Skein,
            name: skein_oauth::OWNER,
            patches: skein_oauth::patches::PATCHES,
            fresh: None,
            families: &[protocol::KeptFamily {
                name: skein_oauth::FAMILY,
                texts: skein_oauth::patches::kept::skein_oauth::TEXTS,
                map: protocol::KeptMap::Secrets,
                keys: &[],
                frames: None,
                flights: &[],
            }],
        },
        protocol::PatchSet {
            owner: protocol::Owner::Smith,
            name: smith_transcript::OWNER,
            patches: smith_transcript::patches::PATCHES,
            fresh: Some(smith_transcript::FRESH_WINDOWS),
            families: &[protocol::KeptFamily {
                name: smith_transcript::FAMILY,
                texts: smith_transcript::patches::kept::smith_transcript::TEXTS,
                map: protocol::KeptMap::Store,
                keys: &[],
                frames: Some(smith_transcript::ROOT),
                flights: &[],
            }],
        },
        protocol::JIG,
        temper_protocol_store::APPLICATION,
    ],
};

fn with_patch_sets(mut component: protocol::Store) -> protocol::Store {
    component.patch_sets(PATCH_SETS);
    component
}

/// The wires this engine speaks to its workers (jig's protocol/store.md,
/// 5.3; temper's protocol/README.md, 9).
pub const WIRES: [protocol::Wire; 4] = [
    protocol::Wire { name: "jig-channel", version: channel_version(jig_protocol_worker_wire::VERSION) },
    protocol::Wire { name: "smith-charter", version: record_version(smith_charter::Charter::FORMAT_VERSION) },
    protocol::Wire { name: "smith-turn", version: record_version(smith_transcript::Turn::FORMAT_VERSION) },
    protocol::Wire { name: "temper-payload", version: temper_protocol_worker_wire::PAYLOAD_VERSION },
];
// These conversions are checked while the static wire table is evaluated.
const fn channel_version(version: u16) -> u32 {
    let bytes = version.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], 0, 0])
}
const fn record_version(version: u64) -> u32 {
    let bytes = version.to_le_bytes();
    assert!(bytes[4] == 0 && bytes[5] == 0 && bytes[6] == 0 && bytes[7] == 0, "wire version fits u32");
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
const ADOPT: Token = Token::new(u64::MAX - 1);
const CLOSE: Token = Token::new(u64::MAX);
const MASK: u64 = (1_u64 << 60) - 1;

#[derive(Debug)]
struct Driver {
    io: FileIo,
    above: Queue<file::Event>,
    handles: Map<Token, ()>,
}
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "opened, pending generation, descriptor adoption and closing are independent lifecycle facts"
)]
pub struct Store {
    #[cfg(test)]
    pub(crate) forge_loads: skein_lib::List<Option<u64>>,
    #[cfg(test)]
    pub(crate) erases: u32,
    #[cfg(test)]
    pub(crate) hold_mark: bool,
    #[cfg(test)]
    pub(crate) held_mark: Option<root::StoreRequest>,
    #[cfg(test)]
    pub(crate) secret_requests: u32,
    pub(crate) component: protocol::Store,
    env: Env<protocol::Limits>,
    pub(crate) routes: route::Routes,
    pub(crate) above: Queue<protocol::Event>,
    pub(crate) routed: Queue<Up>,
    pub(crate) secrets: Queue<protocol::Event>,
    /// The answers to the accounts' records' loads and writes (jig's store.md 9).
    pub(crate) sources: Queue<protocol::Event>,
    below: protocol::Below,
    drivers: [Driver; 3],
    pub(crate) submitted: Queue<Submit>,
    pub(crate) opened: bool,
    pub(crate) pending_generation: bool,
    /// The installation's identifier: the drawn one until the store opens,
    /// then the one it keeps (jig's store.md 4.4).
    pub(crate) installation: [u8; 16],
    adopted: bool,
    root: Option<Fd>,
    closing: bool,
}

/// Admit the core, journal and temper families, then copy checked sizes to KV.
#[must_use]
pub fn limits(root: &root::Limits, forge: &forge::Limits, profile: &protocol::Limits) -> Option<protocol::Limits> {
    admit(root, forge, profile, None).ok()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Refusal {
    Map(protocol::sizes::SizeRefusal),
    ReadRoom { read: u64, needs: u64 },
    Transcript { bytes: u32, ceiling: u32 },
    Page { transcript: u32, frame: u32, page: u32, ceiling: u32 },
}

/// Keep the cause for startup before any comparison needs the store's bounds.
pub(crate) fn admit(
    root: &root::Limits,
    forge: &forge::Limits,
    profile: &protocol::Limits,
    turn: Option<u32>,
) -> Result<protocol::Limits, Refusal> {
    let limits = layout(root, forge, profile)?;
    let needs = read_room(&limits, turn).ok_or(Refusal::Map(protocol::sizes::SizeRefusal::Ceiling))?;
    let decoded = decoded_read_room(turn).ok_or(Refusal::Map(protocol::sizes::SizeRefusal::Ceiling))?;
    // The chunk or resume buffer can already cover a page. Only a decoded
    // row must fit the configured read itself; either refusal names one room.
    if check_read_page(&limits).is_none() || root.loads.read_bytes < decoded {
        return Err(Refusal::ReadRoom { read: root.loads.read_bytes, needs });
    }
    match protocol::sizes::whole(max_whole(&limits)) {
        Ok(()) => {}
        Err(why) => return Err(Refusal::Map(why)),
    }
    Ok(limits)
}

/// The room named by either read refusal: the drivers' encoded page or the
/// agent's decoded turn with its read row (protocol/configuration.md, 7).
/// Without agent metadata only the page requirement is known.
pub(crate) fn read_room(limits: &protocol::Limits, turn: Option<u32>) -> Option<u64> {
    Some(u64::from(limits.store.page.bytes).max(decoded_read_room(turn)?))
}

fn decoded_read_room(turn: Option<u32>) -> Option<u64> {
    match turn {
        Some(turn) => u64::from(turn)
            .checked_add(u64::try_from(size_of::<jig_domain_core::reads::Row>()).ok()?)?
            .checked_add(u64::try_from(size_of::<jig_domain_core::Record>()).ok()?),
        None => Some(0),
    }
}

/// Size the records before agent metadata is read. Read admission follows once
/// the page and, where available, the agent's decoded read-row room are known.
pub(crate) fn layout(
    root: &root::Limits,
    forge: &forge::Limits,
    profile: &protocol::Limits,
) -> Result<protocol::Limits, Refusal> {
    let arithmetic = Refusal::Map(protocol::sizes::SizeRefusal::Ceiling);
    let core = root::core_limits(root);
    let mut kv = profile.store;
    let transcript = root.loads.transcript_bytes;
    let ceiling = protocol::core_v6::CEILINGS.turn_record_transcript;
    if transcript > ceiling {
        return Err(Refusal::Transcript { bytes: transcript, ceiling });
    }
    let sizes = match protocol::sizes::check(
        &core,
        &temper_protocol_store::application(forge).ok_or(arithmetic)?,
        &root::journal_limits(&root.journal),
        &kv,
        profile.orphan_erases,
    ) {
        Ok(sizes) => sizes,
        Err(why) => return Err(Refusal::Map(why)),
    };
    kv.key = sizes.key;
    kv.value = sizes.value;
    kv.ops = sizes.ops;
    kv.commit = sizes.commit;
    kv.queued_bytes = u64::from(sizes.commit).checked_mul(u64::from(kv.queued)).ok_or(arithmetic)?;
    kv.segment = u64::from(sizes.commit).checked_mul(2).ok_or(arithmetic)?;
    kv.snapshot_after = u64::from(sizes.commit);
    kv.chunk = sizes.key.checked_add(sizes.value).ok_or(arithmetic)?.checked_add(20).ok_or(arithmetic)?;
    kv.page.bytes = page_bytes(&core, transcript, sizes.key, sizes.value).ok_or(arithmetic)?;
    if kv.page.bytes > profile.store.page.bytes {
        let frame = protocol::sizes::frame(&core, transcript).ok_or(arithmetic)?;
        return Err(Refusal::Page { transcript, frame, page: kv.page.bytes, ceiling: profile.store.page.bytes });
    }
    Ok(protocol::Limits {
        core,
        store: kv,
        secrets: kv,
        held: profile.held,
        journal: root::journal_limits(&root.journal),
        joins: profile.joins,
        queue: kv.queued,
        files: profile.files,
        reads: core.load_slots.checked_add(core.tasks.tasks).ok_or(arithmetic)?,
        orphan_erases: profile.orphan_erases,
    })
}

// protocol/store.md, 4; jig owns current full-frame sizing.
fn page_bytes(core: &jig_domain_core::Limits, transcript: u32, key: u32, value: u32) -> Option<u32> {
    Some(key.checked_add(value)?.max(protocol::sizes::frame(core, transcript)?))
}

impl Store {
    #[cfg(test)]
    pub(crate) fn opening_limits(&self) -> protocol::Limits {
        self.env.limits
    }

    #[must_use]
    pub fn new(
        application: &root::Limits,
        limits: &protocol::Limits,
        models: &[jig_domain_core::Model],
        installation: [u8; 16],
    ) -> Self {
        let room = root::max_out(application).max(protocol::MAX_OUT_DOWN).checked_mul(2).expect("store route room");
        let routes = route::Routes::new(
            application.loads.loads,
            application,
            temper_protocol_store::bounds(&application.forge).expect("checked family bounds"),
        );
        let mut drivers = [driver(limits, room), driver(limits, room), driver(limits, room)];
        let [store, secrets, transcripts] = &mut drivers;
        let seeded = protocol::seed(
            [&mut store.io, &mut secrets.io, &mut transcripts.io],
            u64::from_le_bytes(*installation.first_chunk::<8>().expect("installation seed")),
        );
        let mut component = with_patch_sets(protocol::Store::on_directory(
            limits,
            temper_protocol_store::application(&application.forge).expect("checked application sizes"),
            models,
            seeded,
        ));
        assert!(component.wires(&WIRES), "valid engine wires before open");
        Self {
            #[cfg(test)]
            forge_loads: skein_lib::List::with_capacity(128),
            #[cfg(test)]
            erases: 0,
            #[cfg(test)]
            hold_mark: false,
            #[cfg(test)]
            held_mark: None,
            #[cfg(test)]
            secret_requests: 0,
            component,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
            routes,
            above: Queue::with_capacity(room),
            routed: Queue::with_capacity(room),
            secrets: Queue::with_capacity(room),
            sources: Queue::with_capacity(room),
            below: protocol::Below {
                store: Queue::with_capacity(room),
                secrets: Queue::with_capacity(room),
                transcripts: Queue::with_capacity(room),
            },
            drivers,
            submitted: Queue::with_capacity(room),
            opened: false,
            pending_generation: false,
            installation,
            adopted: false,
            root: None,
            closing: false,
        }
    }
    /// Whether the component has reported that its deployment opened.
    #[must_use]
    pub const fn opened(&self) -> bool {
        self.opened
    }
    pub fn adopt(&mut self, fd: Fd, user: u32) -> Result<(), Fd> {
        if self.adopted {
            return Err(fd);
        }
        self.adopted = true;
        for index in 0..3 {
            let driver = self.drivers.get_mut(index).expect("driver index");
            driver.io = core::mem::replace(&mut driver.io, FileIo::new(1, 1, 1, Duration::from_secs(1)))
                .with_effective_user(user);
            if index == 0 {
                let file = driver.io.adopt_root(fd).expect("deployment root slot");
                driver.handles.insert(file, ()).expect("owned deployment root");
            }
        }
        self.root = Some(fd);
        let clone =
            file::Request::OpenDirectory { owner: ADOPT, root: fd, name: b".".as_slice().into(), no_follow: true };
        self.below.secrets.push(clone);
        self.below.transcripts.push(file::Request::OpenDirectory {
            owner: ADOPT,
            root: fd,
            name: b".".as_slice().into(),
            no_follow: true,
        });
        Ok(())
    }
    pub fn clock(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
    }
    pub fn ask(&mut self, request: protocol::Request) {
        match request {
            protocol::Request::Secrets { owner, request } => {
                if let Some(event) = self.secrets(owner, request) {
                    self.above.push(event);
                }
            }
            request @ (protocol::Request::Open { .. }
            | protocol::Request::Switch { .. }
            | protocol::Request::Commit { .. }
            | protocol::Request::Restore { .. }
            | protocol::Request::Load { .. }
            | protocol::Request::Read { .. }
            | protocol::Request::Get { .. }
            | protocol::Request::Transcript { .. }
            | protocol::Request::Site { .. }) => {
                protocol::down(&mut self.component, &self.env, request, &mut self.above, &mut self.below);
            }
        }
    }
    pub fn down(&mut self, out: root::StoreRequest) -> bool {
        #[cfg(test)]
        if self.hold_mark
            && let root::StoreRequest::Commit { writes, .. } = &out
            && writes.contains(&root::Write::Wires)
        {
            assert!(self.held_mark.is_none(), "one held startup mark");
            self.held_mark = Some(out);
            return true;
        }
        if let Some(request) = route::down(&mut self.routes, out) {
            self.ask(request);
            true
        } else {
            false
        }
    }
    /// Get answers stay in this pass; secret writes' terminals are handed to their owner.
    pub fn secrets(&mut self, owner: Token, request: protocol::Secrets) -> Option<protocol::Event> {
        #[cfg(test)]
        {
            self.secret_requests = self.secret_requests.checked_add(1).expect("test secret requests");
        }
        #[cfg(test)]
        if owner == crate::forge::records::LOAD
            && let protocol::Secrets::Load { after, .. } = &request
        {
            self.forge_loads.push(*after).expect("test page trace");
        }
        #[cfg(test)]
        if let protocol::Secrets::Erase { .. } = &request {
            self.erases = self.erases.checked_add(1).expect("test erases");
        }
        let immediate = match &request {
            protocol::Secrets::Get { .. } | protocol::Secrets::Load { .. } => true,
            protocol::Secrets::AdoptValue { .. }
            | protocol::Secrets::Adopt { .. }
            | protocol::Secrets::Keep { .. }
            | protocol::Secrets::Replace { .. }
            | protocol::Secrets::Erase { .. }
            | protocol::Secrets::Hold { .. }
            | protocol::Secrets::Carry { .. } => false,
        };
        protocol::down(
            &mut self.component,
            &self.env,
            protocol::Request::Secrets { owner, request },
            &mut self.above,
            &mut self.below,
        );
        let mut answer = None;
        if immediate {
            for _ in 0..self.above.len() {
                let event = self.above.pop().expect("store terminals");
                if let protocol::Event::Secret { owner: found, .. }
                | protocol::Event::SecretsLoaded { owner: found, .. } = &event
                    && *found == owner
                {
                    answer = Some(event);
                } else {
                    self.above.push(event);
                }
            }
        }
        answer
    }
    #[must_use]
    pub fn room(&self) -> bool {
        self.secrets.room() >= protocol::MAX_OUT_DOWN
            && self.sources.room() >= protocol::MAX_OUT_DOWN
            && self.above.room() >= protocol::MAX_OUT_DOWN
            && self.routed.room() >= protocol::MAX_OUT_DOWN
            && self.below.store.room() >= protocol::MAX_OUT_DOWN
            && self.below.secrets.room() >= protocol::MAX_OUT_DOWN
            && self.below.transcripts.room() >= protocol::MAX_OUT_DOWN
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next: Option<Time> = None;
        for driver in &self.drivers {
            next = match (next, driver.io.next_deadline()) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, None) => left,
                (None, right) => right,
            };
        }
        next
    }
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        if !self.secrets.is_empty()
            || !self.sources.is_empty()
            || !self.above.is_empty()
            || !self.routed.is_empty()
            || !self.submitted.is_empty()
        {
            return true;
        }
        for index in 0..3 {
            let driver = self.drivers.get(index).expect("driver index");
            if driver.io.is_due(now)
                || !self.drivers.get(index).expect("driver index").above.is_empty()
                || driver.io.takes() && (!self.queue(index).is_empty() || self.can_close(index))
            {
                return true;
            }
        }
        false
    }
    fn can_close(&self, index: usize) -> bool {
        self.closing
            && !self.drivers.get(index).expect("driver index").handles.is_empty()
            && (index != 0
                || self.drivers[1].handles.is_empty()
                    && self.drivers[2].handles.is_empty()
                    && self.drivers[1].io.takes()
                    && self.drivers[2].io.takes()
                    && self.drivers[1].above.is_empty()
                    && self.drivers[2].above.is_empty())
    }
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closing && self.idle() && empty_handles(&self.drivers)
    }
    #[must_use]
    pub fn idle(&self) -> bool {
        (self.closing || self.routes.idle())
            && self.secrets.is_empty()
            && self.sources.is_empty()
            && self.above.is_empty()
            && self.routed.is_empty()
            && self.submitted.is_empty()
            && self.below.store.is_empty()
            && self.below.secrets.is_empty()
            && self.below.transcripts.is_empty()
            && idle_drivers(&self.drivers)
    }
    pub fn close(&mut self) {
        self.closing = true;
        self.root = None;
        for _ in 0..self.above.len() {
            let _terminal = self.above.pop();
        }
        for _ in 0..self.routed.len() {
            let _up = self.routed.pop();
        }
        for _ in 0..self.secrets.len() {
            let _terminal = self.secrets.pop();
        }
        for _ in 0..self.sources.len() {
            let _terminal = self.sources.pop();
        }
        for index in 0..3 {
            for _ in 0..self.queue(index).len() {
                let _request = self.pop(index);
            }
        }
    }
    pub fn complete(&mut self, mut complete: Complete) {
        let index = usize::try_from((complete.op.raw() >> 60_u32).checked_sub(8).expect("driver namespace"))
            .expect("three drivers");
        complete.op = Token::new(complete.op.raw() & MASK);
        if let Op::Cancel { target } = &mut complete.kind {
            *target = Token::new(target.raw() & MASK);
        }
        let mut submitted = Queue::with_capacity(4);
        let driver = self.drivers.get_mut(index).expect("driver index");
        file_layer::up(&mut driver.io, complete, &mut driver.above, &mut submitted);
        self.publish(index, &mut submitted);
    }
    fn publish(&mut self, index: usize, queue: &mut Queue<Submit>) {
        let prefix = *crate::tokens::STORE.get(index).expect("driver prefix");
        for _ in 0..queue.len() {
            let mut submit = queue.pop().expect("file submissions");
            submit.op = Token::new(submit.op.raw() | prefix);
            if let Op::Cancel { target } = &mut submit.kind {
                *target = Token::new(target.raw() | prefix);
            }
            self.submitted.push(submit);
        }
    }
    fn queue(&self, index: usize) -> &Queue<file::Request> {
        if index == 0 {
            &self.below.store
        } else if index == 1 {
            &self.below.secrets
        } else {
            &self.below.transcripts
        }
    }
    fn pop(&mut self, index: usize) -> Option<file::Request> {
        if index == 0 {
            self.below.store.pop()
        } else if index == 1 {
            self.below.secrets.pop()
        } else {
            self.below.transcripts.pop()
        }
    }
    fn descriptor(&mut self, index: usize, event: &file::Event) -> Option<Fd> {
        if let file::Event::Opened { file, .. } = event {
            self.drivers
                .get_mut(index)
                .expect("driver index")
                .handles
                .insert(*file, ())
                .expect("bounded open file handles");
            return self.drivers.get(index).expect("driver index").io.descriptor(*file);
        }
        None
    }
    fn close_request(&mut self, index: usize) -> Option<file::Request> {
        if self.can_close(index) {
            let handles = &mut self.drivers.get_mut(index).expect("driver index").handles;
            if let Some((token, ())) = handles.iter().last() {
                let file = *token;
                handles.remove(&file);
                return Some(close(file));
            }
        }
        None
    }
    pub fn iterate(&mut self) {
        if self.adopted && !self.closing && self.room() {
            protocol::fire(&mut self.component, &self.env, &mut self.above, &mut self.below);
        }
        for index in 0..3 {
            let mut submitted = Queue::with_capacity(4);
            file_layer::expire(
                &mut self.drivers.get_mut(index).expect("driver index").io,
                self.env.now,
                &mut submitted,
            );
            for _ in 0..self.drivers.get(index).expect("driver index").above.len() {
                if !self.room() {
                    break;
                }
                let event = self.drivers.get_mut(index).expect("driver index").above.pop().expect("file terminal");
                if event.owner() == CLOSE {
                    continue;
                }
                let fd = self.descriptor(index, &event);
                if event.owner() == ADOPT {
                    if fd.is_none() {
                        self.root = None;
                        self.above.push(protocol::Event::OpenRefused {
                            owner: Token::new(1),
                            why: protocol::OpenRefusal::Failed,
                        });
                    } else if !self.drivers[1].handles.is_empty()
                        && !self.drivers[2].handles.is_empty()
                        && let Some(fd) = self.root.take()
                    {
                        self.ask(protocol::Request::Open {
                            owner: Token::new(1),
                            root: protocol::Root { token: Token::new(1), fd },
                            installation: self.installation,
                        });
                    }
                    continue;
                }
                let arrived = protocol::Arrived { event, fd };
                let event = if index == 0 {
                    protocol::Io::Store(arrived)
                } else if index == 1 {
                    protocol::Io::Secrets(arrived)
                } else {
                    protocol::Io::Transcripts(arrived)
                };
                if !self.closing {
                    protocol::up(&mut self.component, &self.env, event, &mut self.above, &mut self.below);
                }
            }
            if self.drivers.get(index).expect("driver index").io.takes() {
                let request = self.pop(index);
                if let Some(file::Request::Close { file, .. }) = &request {
                    self.drivers.get_mut(index).expect("driver index").handles.remove(file);
                }
                let request = match request {
                    Some(request) => Some(request),
                    None => self.close_request(index),
                };
                if let Some(request) = request {
                    let driver = self.drivers.get_mut(index).expect("driver index");
                    file_layer::down(&mut driver.io, self.env.now, request, &mut driver.above, &mut submitted);
                }
            }
            self.publish(index, &mut submitted);
        }
    }
}
#[must_use]
pub fn owns(token: Token) -> bool {
    crate::tokens::STORE.contains(&(token.raw() & !MASK))
}

fn handles(limits: &protocol::Limits) -> u32 {
    limits.files.checked_add(4).expect("store file handles fit")
}
fn max_read(limits: &protocol::Limits) -> u32 {
    limits
        .store
        .chunk
        .max(limits.core.resume_bytes)
        .max(u32::try_from(limits.core.load_bytes.min(u64::from(u32::MAX))).expect("file read length"))
}
fn max_whole(limits: &protocol::Limits) -> u32 {
    max_read(limits).max(protocol::sizes::MANIFEST_BYTES)
}
fn check_read_page(limits: &protocol::Limits) -> Option<()> {
    if max_read(limits) < limits.store.page.bytes {
        return None;
    }
    Some(())
}
fn driver(limits: &protocol::Limits, capacity: u32) -> Driver {
    Driver {
        io: FileIo::with_whole_limit(handles(limits), max_read(limits), 32, max_whole(limits), limits.store.deadline),
        above: Queue::with_capacity(capacity),
        handles: Map::with_capacity(handles(limits)),
    }
}
fn close(file: Token) -> file::Request {
    file::Request::Close { owner: CLOSE, file }
}

fn empty_handles(drivers: &[Driver; 3]) -> bool {
    for driver in drivers {
        if !driver.handles.is_empty() {
            return false;
        }
    }
    true
}
fn idle_drivers(drivers: &[Driver; 3]) -> bool {
    for driver in drivers {
        if !driver.io.takes() || !driver.above.is_empty() {
            return false;
        }
    }
    true
}

pub(crate) fn worst_case(application: &root::Limits, limits: &protocol::Limits) -> Option<u64> {
    let room = root::max_out(application).max(protocol::MAX_OUT_DOWN).checked_mul(2)?;
    // Queues and load flights price the same immutable payload envelope.
    let payload = root::payload_worst_case(application)?;
    protocol::worst_case_with_patches(limits, &PATCH_SETS)?
        .checked_add(u64::try_from(size_of::<Store>()).ok()?)?
        .checked_add(FileIo::worst_case(handles(limits), max_read(limits), 32, max_whole(limits))?.checked_mul(3)?)?
        .checked_add(Queue::<file::Request>::worst_case(room)?.checked_mul(3)?)?
        .checked_add(Queue::<file::Event>::worst_case(room)?.checked_mul(3)?)?
        .checked_add(Queue::<protocol::Event>::worst_case(room)?.checked_mul(3)?)?
        .checked_add(Queue::<Up>::worst_case(room)?)?
        .checked_add(Queue::<Submit>::worst_case(room)?)?
        .checked_add(Map::<Token, ()>::worst_case(handles(limits))?.checked_mul(3)?)?
        .checked_add(route::worst_case(application, limits.store.key, payload)?)?
        .checked_add(u64::from(max_read(limits)).checked_mul(u64::from(room))?.checked_mul(6)?)?
        .checked_add(payload.checked_mul(u64::from(room))?)
}

#[cfg(test)]
mod sizing_tests {
    use super::{FileIo, check_read_page, handles, max_read, max_whole, page_bytes, protocol};

    #[test]
    fn drivers_read_a_whole_page_in_every_admitted_store() {
        let (application, _, profile) = crate::tests::fixtures::store_limits();
        let limits = super::limits(&application, &application.forge, &profile).unwrap();
        assert!(max_read(&limits) >= limits.store.page.bytes);
    }

    #[test]
    fn a_store_whose_reads_are_below_its_page_is_refused() {
        let (_, _, mut limits) = crate::tests::fixtures::store_limits();
        let page = limits.store.page.bytes;
        let below = page.checked_sub(1).unwrap();
        // Synthetic fields check admission without constructing an invalid KV store.
        limits.store.chunk = below;
        limits.core.resume_bytes = below;
        limits.core.load_bytes = u64::from(below);
        assert_eq!(max_read(&limits), below);
        assert_eq!(check_read_page(&limits), None);
        for (chunk, resume, load) in [(page, below, below), (below, page, below), (below, below, page)] {
            limits.store.chunk = chunk;
            limits.core.resume_bytes = resume;
            limits.core.load_bytes = u64::from(load);
            assert_eq!(max_read(&limits), page);
            assert_eq!(check_read_page(&limits), Some(()));
        }
    }

    #[test]
    fn drivers_hold_the_whole_manifest_when_reads_are_smaller() {
        let (_, _, mut limits) = crate::tests::fixtures::store_limits();
        limits.core.resume_bytes = 16;
        limits.core.load_bytes = 8;
        // Synthetic read fields exercise the helper without constructing a KV store.
        for chunk in [32, protocol::sizes::MANIFEST_BYTES.checked_sub(1).unwrap()] {
            limits.store.chunk = chunk;
            assert_eq!(max_read(&limits), chunk);
            assert_eq!(max_whole(&limits), protocol::sizes::MANIFEST_BYTES);
        }
    }

    #[test]
    fn drivers_take_reads_whole_when_reads_are_larger() {
        let (_, _, mut limits) = crate::tests::fixtures::store_limits();
        let above = protocol::sizes::MANIFEST_BYTES.checked_add(1).unwrap();
        for (chunk, resume, load) in [(above, 16, 8), (32, above, 8), (32, 16, u64::from(above))] {
            limits.store.chunk = chunk;
            limits.core.resume_bytes = resume;
            limits.core.load_bytes = load;
            assert_eq!(max_read(&limits), above);
            assert_eq!(max_whole(&limits), max_read(&limits));
        }
    }

    #[test]
    fn an_admitted_store_already_reads_the_whole_manifest() {
        let (application, _, profile) = crate::tests::fixtures::store_limits();
        let limits = super::limits(&application, &application.forge, &profile).unwrap();
        assert!(max_read(&limits) >= protocol::sizes::MANIFEST_BYTES);
        assert_eq!(max_whole(&limits), max_read(&limits));
        let old_driver = FileIo::worst_case(handles(&limits), max_read(&limits), 32, max_read(&limits)).unwrap();
        let whole_driver = FileIo::worst_case(handles(&limits), max_read(&limits), 32, max_whole(&limits)).unwrap();
        assert_eq!(whole_driver, old_driver);
    }

    #[test]
    fn typed_secret_rows_fit_the_small_profiles_page() {
        // Secrets keys are the two owner bytes followed by a u64 number.
        const KEY_BYTES: u64 = 2 + 8;
        let (application, _, profile) = crate::tests::fixtures::store_limits();
        let limits = super::limits(&application, &application.forge, &profile).unwrap();
        let row = protocol::secrets_v1::worst_case_bytes(&protocol::secrets_v1::CEILINGS).unwrap();
        assert_eq!(row, 274_504);
        assert!(row <= u64::from(limits.secrets.value));
        assert!(row.checked_add(KEY_BYTES).unwrap() < u64::from(limits.secrets.page.bytes));
        assert_eq!(profile.store.page.bytes, 1_590_110);
    }

    #[test]
    fn page_covers_map_or_full_current_frame() {
        let (_, core, _) = crate::tests::fixtures::store_limits();
        // Versioned TurnRecord: 52 bytes including Some(read), plus payload;
        // two lengths and CRC add 12. No turn or frame buffer is allocated.
        assert_eq!(page_bytes(&core, 8, 3, 4), Some(72));
        assert_eq!(page_bytes(&core, 8, 80, 4), Some(84));
    }

    #[test]
    fn page_refuses_codec_excess_and_checked_overflow() {
        let (_, core, _) = crate::tests::fixtures::store_limits();
        let ceiling = jig_protocol_store::core_v6::CEILINGS.turn_record_transcript;
        assert_eq!(page_bytes(&core, ceiling, 3, 4), ceiling.checked_add(64));
        assert_eq!(page_bytes(&core, ceiling.checked_add(1).unwrap(), 3, 4), None);
        assert_eq!(page_bytes(&core, u32::MAX, 3, 4), None);
        assert_eq!(page_bytes(&core, 8, u32::MAX, 1), None);
    }
}
