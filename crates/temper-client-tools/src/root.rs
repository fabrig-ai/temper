//! The tools' shared client and exhaustive owner routing (client/tools.md, 4).
use crate::jig::{self, runner};
use alloc::boxed::Box;
use jig_client::{Client, Told};
use skein_lib::{Env, Queue, Time};
use temper_client_domain::owner::Owner;
pub const RUNNER: Owner = Owner::Runner;
#[derive(Debug)]
pub struct Limits {
    pub domain: temper_client_domain::Limits,
    pub tools: jig::Limits,
}
#[derive(Debug)]
pub struct Root {
    client: Client,
    runner: runner::Runner,
    adopter: crate::forge::Adopter,
    forges: crate::forges::Forges,
    changes: crate::change::Changes,
    next: Option<u32>,
}
#[derive(Debug)]
pub enum Event {
    Client { event: jig_client::Event },
    Call { number: u32, call: jig::call::Call, json: bool, value: Option<skein_lib::Secret> },
}
#[derive(Debug)]
pub enum Request {
    Client { request: jig_client::Request },
    Ready,
    Record { number: u32, record: skein_lib::Secret },
    Kept { line: Box<[u8]> },
    Progress { line: Box<[u8]> },
    Printed { number: u32, printed: jig::Printed },
    Fact { fact: temper_client_domain::Fact },
}
impl Root {
    /// # Errors
    /// Refuses client limits that cannot be represented.
    pub fn new(limits: &Limits, seed: u64) -> Result<Self, jig_client::Unusable> {
        Ok(Self::with_client(Client::new(&limits.domain.client, seed)?, seed))
    }
    /// # Errors
    /// Refuses client limits that cannot be represented.
    pub fn unlinked(limits: &Limits, seed: u64) -> Result<Self, jig_client::Unusable> {
        Ok(Self::with_client(Client::unlinked(&limits.domain.client, seed)?, seed))
    }
    pub(crate) fn with_client(client: Client, seed: u64) -> Self {
        let mut runner = runner::Runner::new(RUNNER.asker(), crate::key::TOOLS, seed);
        runner.parts(&Parts);
        Self {
            client,
            runner,
            adopter: crate::forge::Adopter::new(),
            forges: crate::forges::Forges::new(),
            changes: crate::change::Changes::new(),
            next: Some(1),
        }
    }
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }
    #[must_use]
    pub fn kept(&self) -> u32 {
        self.runner.kept(&self.client)
    }
    pub fn kept_json(&mut self, json: bool) {
        self.runner.kept_json(json);
        self.adopter.kept_json(json);
        self.forges.kept_json(json);
    }
}
pub fn step(root: &mut Root, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    let mut client = Queue::with_capacity(jig_client::max_out(&env.limits.domain.client));
    let mut child = Queue::with_capacity(env.limits.tools.rows.checked_mul(2).expect("tools outputs fit"));
    match event {
        Event::Client { event } => {
            if let jig_client::Event::Start { .. } | jig_client::Event::StartUnlinked { .. } = &event {
                root.runner.start(&mut child);
            }
            root.client.step(env.now, env.wall, event, &mut client);
        }
        Event::Call { number, call, json, value } => {
            let active = root
                .runner
                .calls()
                .checked_add(root.adopter.calls())
                .expect("bounded calls")
                .checked_add(root.forges.calls())
                .expect("bounded calls")
                .checked_add(root.changes.calls())
                .expect("bounded calls");
            if active >= usize::try_from(env.limits.tools.rows).expect("rows fit") {
                let mut printed = jig::render::failure(b"call limit", json, 1);
                if call.tool == jig::rows::EXPORT_AT {
                    printed.file = jig::FileEnd::Remove;
                }
                out.push(Request::Printed { number, printed });
            } else if let Some(call_number) = root.next {
                root.next = call_number.checked_add(1);
                let forge = crate::table::TOOLS.get(usize::from(call.tool)).expect("parsed tool").name
                    == crate::adopt::ROW.name;
                let called = runner::Called { number, call_number, call, json, value };
                if crate::change::owns(&called.call) {
                    crate::change::call(&mut root.changes, &root.client, env, called, &mut child);
                } else if crate::forges::owns(&called.call) {
                    crate::forges::call(&mut root.forges, &root.client, env, called, &mut child);
                } else if forge {
                    crate::forge::call(&mut root.adopter, &root.client, &env.limits, called, &mut child);
                } else {
                    runner::call(&mut root.runner, &root.client, &tools_env(env), called, &mut child);
                }
            } else {
                let mut printed = jig::render::failure(b"call number exhausted", json, 1);
                if call.tool == jig::rows::EXPORT_AT {
                    printed.file = jig::FileEnd::Remove;
                }
                out.push(Request::Printed { number, printed });
            }
        }
    }
    drain(root, env, &mut client, &mut child, out);
    root.client.reclaim();
}
fn tools_env(env: &Env<Limits>) -> Env<jig::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.tools }
}
fn drain(
    root: &mut Root,
    env: &Env<Limits>,
    client: &mut Queue<jig_client::Request>,
    child: &mut Queue<runner::Out>,
    out: &mut Queue<Request>,
) {
    let bound = jig_client::max_out(&env.limits.domain.client)
        .checked_mul(env.limits.tools.rows.checked_add(1).expect("rows fit"))
        .expect("routing bound fits");
    for _ in 0..bound {
        if let Some(request) = client.pop() {
            match request {
                jig_client::Request::Told { told } => route(root, env, told, child, out),
                // M4's Authorize place answers it
                jig_client::Request::Authorized(_) => {}
                request @ (jig_client::Request::Exchange { .. }
                | jig_client::Request::OpenStream { .. }
                | jig_client::Request::CloseStream
                | jig_client::Request::Save { .. }
                | jig_client::Request::Erase { .. }
                | jig_client::Request::Down { .. }
                | jig_client::Request::SignOut
                | jig_client::Request::ServiceCredential { .. }) => out.push(Request::Client { request }),
            }
        } else if let Some(request) = child.pop() {
            match request {
                runner::Out::Client { event } => root.client.step(env.now, env.wall, event, client),
                runner::Out::Record { number, record } => out.push(Request::Record { number, record }),
                runner::Out::Ready => out.push(Request::Ready),
                runner::Out::Kept { line } => out.push(Request::Kept { line }),
                runner::Out::Progress { line } => out.push(Request::Progress { line }),
                runner::Out::Printed { number, printed } => out.push(Request::Printed { number, printed }),
                runner::Out::DraftAmendment { called, task } => {
                    let prepared = match runner::acts::me(&root.runner, &root.client) {
                        Some(me) => crate::amend::prepare(&called.call, &task, me, &env.limits.tools),
                        None => Err(runner::acts::Failed::Fixed(b"signed out")),
                    };
                    runner::acts::prepared(&mut root.runner, &root.client, &tools_env(env), *called, prepared, child);
                }
            }
        } else {
            return;
        }
    }
    assert!(client.is_empty() && child.is_empty(), "bounded routing completes");
}
fn route(root: &mut Root, env: &Env<Limits>, told: Told, child: &mut Queue<runner::Out>, out: &mut Queue<Request>) {
    if let Told::Kept { key, .. } = &told {
        let prefix = u32::try_from(key.number >> 32u32).expect("high half fits");
        if let Some(next) = root.next
            && prefix >= next
        {
            root.next = prefix.checked_add(1);
        }
    }
    let asker = match &told {
        Told::Link { .. } | Told::Dropped { .. } => RUNNER.asker(),
        Told::Kept { key, .. } | Told::Pending { key } | Told::Answered { key, .. } => key.asker,
        Told::Changed { handle } | Told::Text { handle } | Told::Ended { handle, .. } | Told::Page { handle, .. } => {
            handle.asker
        }
    };
    let me_changed = match &told {
        Told::Changed { handle } => handle.asker == RUNNER.asker() && handle.number == 0,
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => false,
    };
    match Owner::of(asker) {
        Some(Owner::Runner) => runner::told(&mut root.runner, &root.client, &tools_env(env), told, child),
        Some(Owner::Forge) => {
            match &told {
                Told::Kept { key, request } => runner::told(
                    &mut root.runner,
                    &root.client,
                    &tools_env(env),
                    Told::Kept { key: *key, request: request.clone() },
                    child,
                ),
                Told::Answered { key, .. } => root.runner.forget_kept(*key),
                Told::Link { .. }
                | Told::Pending { .. }
                | Told::Dropped { .. }
                | Told::Changed { .. }
                | Told::Text { .. }
                | Told::Ended { .. }
                | Told::Page { .. } => {}
            }
            crate::change::told(&mut root.changes, &root.client, env, &told, child);
            crate::forges::told(&mut root.forges, &root.client, env, &told, child);
            crate::forge::told(&mut root.adopter, &env.limits, told, child);
        }
        Some(Owner::Pages) | None => out.push(Request::Fact { fact: temper_client_domain::Fact::UnknownAsker }),
    }
    if me_changed {
        crate::change::ready(&mut root.changes, &root.client, env, child);
        crate::forge::ready(&mut root.adopter, &root.client, &env.limits, child);
        crate::forges::ready(&mut root.forges, &root.client, env, child);
    }
}
pub fn fire(root: &mut Root, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut client = Queue::with_capacity(jig_client::max_out(&env.limits.domain.client));
    let mut child = Queue::with_capacity(env.limits.tools.rows.checked_mul(2).expect("tools outputs fit"));
    root.client.fire(env.now, env.wall, &mut client);
    drain(root, env, &mut client, &mut child, out);
    jig::wait::fire(&mut root.runner, &root.client, &tools_env(env), &mut child);
    crate::change::fire(&mut root.changes, env, &mut child);
    crate::forges::fire(&mut root.forges, &root.client, env, &mut child);
    drain(root, env, &mut client, &mut child, out);
    root.client.reclaim();
}
#[must_use]
pub fn next_deadline(root: &Root) -> Option<Time> {
    root.client
        .next_deadline()
        .into_iter()
        .chain(jig::wait::next_deadline(&root.runner))
        .chain(root.forges.next_deadline())
        .chain(root.changes.next_deadline())
        .min()
}

#[cfg(test)]
#[path = "tests/root.rs"]
mod tests;

/// Forge parts use the domain's shared words and whole resource paths.
#[derive(Debug)]
pub struct Parts;
impl jig::render::Parts for Parts {
    fn phrasebook(&self, text: u32) -> skein_ui::phrases::Phrasebook {
        temper_client_domain::words::phrasebook(text)
    }
    fn task_next(&self, task: &jig_client::contract::Task, out: &mut jig::render::Text) {
        if let jig_client::contract::TaskContractDocument::Change { connector, kind: 1, .. } = task.result_contract
            && connector == temper_client_protocol_wire::FORGE
        {
            out.put(b"next: temper ctl DIRECTORY change ");
            out.number(task.chip.task);
            out.put(b"\n");
        }
    }
    fn owner_part(&self, part: &jig_client::contract::OwnerPart, out: &mut jig::render::Text) {
        crate::forge::part(part, out);
    }
    fn part(&self, part: &jig_client::contract::Part, out: &mut jig::render::Text) {
        crate::forge::result_part(part, out);
    }
    fn change_kind(&self, connector: u16, kind: u16, out: &mut jig::render::Text) {
        crate::forge::change_kind(connector, kind, out);
    }
    fn change(
        &self,
        kind: u16,
        part: &jig_client::contract::OwnerPart,
        words: &jig_client::contract::DocumentText,
        out: &mut jig::render::Text,
    ) {
        crate::forge::change(kind, part, words, out);
    }
}

#[cfg(test)]
#[path = "tests/fixtures/root_acts.rs"]
pub(crate) mod act_fixture;

impl Root {
    #[must_use]
    pub fn file_end(&self, number: u32) -> jig::FileEnd {
        runner::file_end(&self.runner, number)
    }
    pub fn recorded(
        &mut self,
        env: &Env<Limits>,
        number: u32,
        result: Result<(), Box<[u8]>>,
        out: &mut Queue<Request>,
    ) {
        let mut client = Queue::with_capacity(jig_client::max_out(&env.limits.domain.client));
        let mut child = Queue::with_capacity(env.limits.tools.rows.checked_mul(2).expect("tools outputs fit"));
        runner::recorded(&mut self.runner, &self.client, &tools_env(env), number, result, &mut child);
        drain(self, env, &mut client, &mut child, out);
        self.client.reclaim();
    }
}
