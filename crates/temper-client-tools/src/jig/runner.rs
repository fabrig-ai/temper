//! Calls share an asker and partition their local numbers (tools contract, 4).
use crate::jig::{call::Call, render, rows, wait};
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{
    Asker, Client, Event, Handle, Latest, State, Told, Watchable,
    contract::{Delivery, Subject},
};
use skein_lib::{Deadlines, Env, Queue};
mod accounts;
pub(crate) mod acts;
mod moves;
pub(crate) mod reads;
mod recovery;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileEnd {
    Keep,
    Remove,
}
#[derive(Debug)]
pub struct Printed {
    pub file: FileEnd,
    pub status: u8,
    pub stdout: Box<[u8]>,
    pub stderr: Box<[u8]>,
}
#[derive(Debug)]
pub struct Runner {
    pub(crate) asker: Asker,
    waiting: BTreeMap<u32, Called>,
    active: BTreeMap<u32, reads::Reading>,
    pub(crate) waits: BTreeMap<u32, wait::Wait>,
    pub(crate) wait_timers: Option<Deadlines<u32>>,
    salt: &'static [u8],
    names: skein_lib::Rng,
    acts: BTreeMap<u32, acts::Reading>,
    sent: BTreeMap<jig_client::Key, acts::Sent>,
    recording: BTreeMap<u32, moves::Recording>,
    kept: BTreeMap<jig_client::Key, render::outcome::Operation>,
    kept_json: bool,
    ready: bool,
    watching: bool,
    pub(crate) parts: Option<&'static dyn render::Parts>,
}
#[derive(Debug)]
pub struct Called {
    pub number: u32,
    pub call_number: u32,
    pub call: Call,
    pub json: bool,
    pub value: Option<skein_lib::Secret>,
}
#[derive(Debug)]
pub enum Out {
    Client { event: Event },
    Ready,
    Record { number: u32, record: skein_lib::Secret },
    Kept { line: Box<[u8]> },
    Progress { line: Box<[u8]> },
    Printed { number: u32, printed: Printed },
    DraftAmendment { called: Box<Called>, task: Box<jig_client::contract::Task> },
}
impl Runner {
    #[must_use]
    pub fn new(asker: Asker, salt: &'static [u8], seed: u64) -> Self {
        Self {
            asker,
            waiting: BTreeMap::new(),
            active: BTreeMap::new(),
            waits: BTreeMap::new(),
            wait_timers: None,
            salt,
            names: skein_lib::Rng::new(seed ^ 0x4143_434F_554E_5453),
            acts: BTreeMap::new(),
            sent: BTreeMap::new(),
            recording: BTreeMap::new(),
            kept: BTreeMap::new(),
            kept_json: false,
            ready: false,
            watching: false,
            parts: None,
        }
    }
    pub fn parts(&mut self, parts: &'static dyn render::Parts) {
        self.parts = Some(parts);
    }
    pub(crate) fn calls(&self) -> usize {
        self.waiting
            .len()
            .checked_add(self.recording.len())
            .expect("bounded calls")
            .checked_add(self.active.len())
            .expect("bounded calls")
            .checked_add(self.acts.len())
            .expect("bounded calls")
            .checked_add(self.waits.len())
            .expect("bounded calls")
            .checked_add(self.sent.len())
            .expect("bounded calls")
    }
    pub(crate) fn forget_kept(&mut self, key: jig_client::Key) {
        self.kept.remove(&key);
    }
    pub(crate) fn kept_json(&mut self, json: bool) {
        self.kept_json = json;
    }
    pub(crate) fn kept(&self, client: &Client) -> u32 {
        recovery::count(self, client)
    }
    pub(crate) fn me(&self) -> Handle {
        Handle { asker: self.asker, number: 0 }
    }
    pub(crate) fn start(&mut self, out: &mut Queue<Out>) {
        if !self.watching {
            self.watching = true;
            out.push(Out::Client { event: Event::Watch { handle: self.me(), subject: Watchable::Jig(Subject::Me) } });
        }
    }
}
pub fn call(runner: &mut Runner, client: &Client, env: &Env<crate::jig::Limits>, called: Called, out: &mut Queue<Out>) {
    let importing = called.call.tool == rows::IMPORT_AT;
    let bound = if importing { crate::jig::call::MOVED_BYTES } else { crate::jig::call::KEY_BYTES };
    if let Some(value) = &called.value
        && (value.is_empty() || value.len() > usize::try_from(bound).expect("value bound"))
    {
        let printed = render::failure(
            if importing {
                b"record file must contain 1 to 131072 bytes"
            } else {
                b"key file must contain 1 to 4096 bytes"
            },
            called.json,
            1,
        );
        finish(called, printed, out);
        return;
    }
    if called.value.is_some() != (called.call.tool == 26 || called.call.tool == 27 || importing) {
        let printed = render::failure(b"key file must be supplied only for add-key or replace-key", called.json, 1);
        finish(called, printed, out);
        return;
    }
    let count = runner.calls();
    if count >= usize::try_from(env.limits.rows).expect("rows fit") {
        let printed = render::failure(b"call limit", called.json, 1);
        finish(called, printed, out);
    } else if !live(runner, client) {
        runner.waiting.insert(called.call_number, called);
    } else {
        run(runner, client, env, called, out);
    }
}
fn live(runner: &Runner, client: &Client) -> bool {
    let Some(document) = client.document(runner.me()) else { return false };
    document.state == State::Live
}
fn run(runner: &mut Runner, client: &Client, env: &Env<crate::jig::Limits>, called: Called, out: &mut Queue<Out>) {
    match called.call.tool {
        0 => {
            let Some(document) = client.document(runner.me()) else { return };
            let Some(Latest::Jig(Delivery::Me(me))) = document.latest else { return };
            let printed =
                render::me(me, called.json, &env.limits, runner.parts.expect("host supplies part vocabulary"));
            finish(called, printed, out);
        }
        1..=7
        | 22
        | 23
        | 25
        | 27..=30
        | 33
        | rows::EXPORT_AT
        | rows::SETTINGS_AT..=rows::REVISIONS_AT
        | rows::PROPOSAL_AT => {
            reads::start(runner, client, env, called, out);
        }
        24 | 26 | 32 | rows::IMPORT_AT => accounts::start(runner, client, env, called, out),
        8..=15 | 31 | rows::CHANGE_SETTINGS_AT | rows::PROPOSE_SETTINGS_AT => {
            acts::start(runner, client, env, called, out);
        }
        16 => wait::start(runner, client, env, called, out),
        _ => {
            let printed = render::failure(b"call not supported", called.json, 1);
            finish(called, printed, out);
        }
    }
}
pub fn told(runner: &mut Runner, client: &Client, env: &Env<crate::jig::Limits>, told: Told, out: &mut Queue<Out>) {
    if wait::told(runner, client, env, &told, out) {
        return;
    }
    match told {
        Told::Changed { handle } if handle == runner.me() => {
            if live(runner, client) {
                if !runner.ready {
                    runner.ready = true;
                    recovery::parked(runner, client, out);
                    out.push(Out::Ready);
                }
                for _ in 0..runner.waiting.len() {
                    let Some((_, called)) = runner.waiting.pop_first() else { break };
                    run(runner, client, env, called, out);
                }
            }
        }
        Told::Page { handle, page } => {
            let prefix = u32::try_from(handle.number >> 32u32).expect("high half");
            if runner.acts.contains_key(&prefix) {
                acts::page(runner, client, env, handle, page, out);
            } else {
                reads::page(runner, client, env, handle, page, out);
            }
        }
        Told::Changed { handle } | Told::Text { handle } => reads::changed(runner, client, env, handle, out),
        Told::Ended { handle, why: jig_client::Why::SignedOut } if handle == runner.me() => {
            runner.ready = false;
            runner.watching = false;
            runner.start(out);
        }
        Told::Ended { handle, why } => reads::ended(runner, handle, why, out),
        Told::Kept { key, request } => recovery::kept(runner, key, &request, client),
        Told::Dropped { count } => recovery::dropped(runner, count, out),
        Told::Answered { key, outcome } => acts::answered(runner, client, env, key, &outcome, out),
        Told::Pending { key } => {
            if let Some(sent) = runner.sent.get_mut(&key)
                && sent.operation.exported.is_some()
                && let Some(pending) = client.pending(key)
            {
                match pending.progress {
                    jig_client::Progress::InFlight | jig_client::Progress::Retrying { .. } => sent.dispatched = true,
                    jig_client::Progress::Parked | jig_client::Progress::Unknown | jig_client::Progress::NotServed => {}
                }
            }
        }
        Told::Link { .. } => {}
    }
}
pub(crate) fn finish(called: Called, mut printed: Printed, out: &mut Queue<Out>) {
    if called.call.tool == rows::EXPORT_AT {
        printed.file = FileEnd::Remove;
    }
    out.push(Out::Printed { number: called.number, printed });
}

/// The runner decides cleanup on signals and shell failures too.
#[must_use]
pub fn file_end(runner: &Runner, number: u32) -> FileEnd {
    for sent in runner.sent.values() {
        if sent.called.number == number && sent.dispatched {
            return FileEnd::Keep;
        }
    }
    FileEnd::Remove
}
pub fn recorded(
    runner: &mut Runner,
    client: &Client,
    env: &Env<crate::jig::Limits>,
    number: u32,
    result: Result<(), Box<[u8]>>,
    out: &mut Queue<Out>,
) {
    moves::recorded(runner, client, env, number, result, out);
}
