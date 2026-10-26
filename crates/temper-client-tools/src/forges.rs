//! The deployment's forge tools (client/tools.md, 5.4 and 5.5, area 6).
use crate::jig::table::{
    Keyed, Kind,
    Occurs::{Optional, Required},
    Tool, flag, positional, switch,
};
pub const READ: Tool = Tool {
    name: b"forges",
    line: b"Read the deployment's forges",
    result: b"forges",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 6,
};
pub const SET_UP: Tool = Tool {
    name: b"forge-setup",
    line: b"Set up an owned Forgejo here",
    result: b"forge-setup",
    forms: &[&[
        positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
        flag(b"org", b"O", Kind::Literal, Optional, b"Organization; default work"),
        flag(b"repository", b"R", Kind::Literal, Optional, b"Repository; default hello"),
        flag(
            b"project",
            b"P",
            Kind::Project { empty: true },
            Optional,
            b"Project name or number; default a new project named R",
        ),
        switch(b"playground", b"Copy playground/widget; takes no --org or --repository"),
    ]],
    keyed: Keyed::Optional,
    area: 6,
};
pub const CONNECT: Tool = Tool {
    name: b"forge-connect",
    line: b"Connect GitHub or Forgejo as its admin",
    result: b"forge-connect",
    forms: &[
        &[
            positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
            positional(
                b"way",
                b"github-app",
                Kind::Word { set: &[b"github-app"] },
                Required,
                b"An existing GitHub App",
            ),
            positional(b"id", b"ID", Kind::NumberRange { min: 1, max: u64::MAX }, Required, b"GitHub App id"),
            positional(b"file", b"FILE", Kind::File, Required, b"Private PEM key file; at most 4090 bytes"),
        ],
        &[
            positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
            positional(
                b"way",
                b"github",
                Kind::Word { set: &[b"github"] },
                Required,
                b"Create a GitHub App from temper's manifest",
            ),
        ],
        &[
            positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
            positional(b"way", b"forgejo", Kind::Word { set: &[b"forgejo"] }, Required, b"Not offered yet"),
            positional(b"file", b"FILE", Kind::Literal, Required, b"Not offered yet; no file is read"),
        ],
        &[
            positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
            positional(
                b"way",
                b"forgejo-admin",
                Kind::Word { set: &[b"forgejo-admin"] },
                Required,
                b"A Forgejo site admin",
            ),
            positional(b"file", b"FILE", Kind::File, Required, b"Private admin token file; at most 4090 bytes"),
            flag(b"first", b"ORG/NAME", Kind::Literal, Optional, b"Make the first repository"),
            flag(
                b"project",
                b"P",
                Kind::Project { empty: false },
                Optional,
                b"Project name or number; default a new project named NAME",
            ),
            flag(b"label", b"L", Kind::Literal, Optional, b"Runner label; default docker"),
        ],
    ],
    keyed: Keyed::Required,
    area: 6,
};
pub const INSTALL: Tool = Tool {
    name: b"forge-install",
    line: b"Print a fresh GitHub App installation link",
    result: b"forge-install",
    forms: &[&[positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number")]],
    keyed: Keyed::Optional,
    area: 6,
};
pub const ACCEPT: Tool = Tool {
    name: b"forge-accept",
    line: b"Accept an existing GitHub App installation",
    result: b"forge-accept",
    forms: &[&[
        positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
        positional(
            b"installation",
            b"I",
            Kind::NumberRange { min: 1, max: u64::MAX },
            Required,
            b"Installation id, as forges prints it",
        ),
    ]],
    keyed: Keyed::Optional,
    area: 6,
};
pub const REPOSITORIES: Tool = Tool {
    name: b"repositories",
    line: b"Read one page of reachable repositories",
    result: b"repositories",
    forms: &[&[
        positional(b"forge", b"F", Kind::NumberRange { min: 1, max: 65535 }, Required, b"Forge number"),
        flag(
            b"installation",
            b"I",
            Kind::NumberRange { min: 1, max: u64::MAX },
            Optional,
            b"Accepted GitHub installation id",
        ),
        flag(b"page", b"N", Kind::NumberRange { min: 1, max: 4_294_967_295 }, Optional, b"Page number; default 1"),
    ]],
    keyed: Keyed::Never,
    area: 6,
};
mod connection;
mod repositories;
pub const DEMO_START: Tool = Tool {
    name: b"demo-start",
    line: b"Start the playground; run again to finish what is not done",
    result: b"demo-start",
    forms: &[&[]],
    keyed: Keyed::Optional,
    area: 6,
};
pub const WAIT: Tool = Tool {
    name: b"wait",
    line: b"Wait for a subject's change or condition",
    result: b"wait",
    forms: &[
        *jig::rows::WAIT.forms.first().expect("base wait form"),
        &[
            positional(b"subject", b"forges", Kind::Word { set: &[b"forges"] }, Required, b"The deployment's forges"),
            flag(b"since", b"DIGEST", Kind::Digest, Optional, b"Wait past this digest"),
            flag(
                b"until",
                b"settled|connected:F|made:F",
                Kind::NamedNumber { words: &[b"settled"], prefixes: &[b"connected:", b"made:"], min: 1, max: 65535 },
                Optional,
                b"settled: none setting up or connecting; connected:F: forge F connected; made:F: first repository made",
            ),
            flag(b"timeout", b"DURATION", Kind::Duration, Optional, b"At most 3600 seconds; default 300 seconds"),
        ],
        crate::change::WAIT_FORM,
    ],
    keyed: Keyed::Never,
    area: 1,
};

use crate::{
    jig::{
        self,
        call::{Value, number, value},
        digest::Digest,
        render::{self, Text},
        runner::{Called, Out},
    },
    root::Limits,
};
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{Client, Event, Handle, Key, Latest, Outcome, State, Told, Why};
use skein_json::writer::Encoder;
use skein_lib::{Env, Queue, Time};
use temper_client_domain::{
    forge::{
        forges::{
            self as domain, Connection, First, FirstState, Offer, Row, SetUpAnswer, SetUpOutcome, SetUpProject,
            SetUpRefusal, Step,
        },
        words,
    },
    owner::Owner,
};

#[derive(Debug)]
pub(crate) struct Forges {
    waiting: BTreeMap<u32, Called>,
    pending: BTreeMap<Key, SettingUp>,
    watches: BTreeMap<Handle, Watching>,
    adopting: BTreeMap<Key, Adopting>,
    starting: BTreeMap<Key, Starting>,
    json: bool,
    connections: BTreeMap<Key, connection::Pending>,
    pages: BTreeMap<Key, repositories::Pending>,
}
#[derive(Debug)]
struct Intent {
    playground: bool,
    forge: u16,
    org: Box<[u8]>,
    repository: Box<[u8]>,
    project: SetUpProject,
}
#[derive(Debug)]
struct SettingUp {
    intent: Option<Intent>,
    called: Option<Called>,
}
#[derive(Debug)]
struct Adopting {
    called: Called,
    adoption: temper_client_domain::forge::Adoption,
}
#[derive(Debug)]
struct Starting {
    called: Called,
    value: temper_client_domain::forge::start::StartIn,
}
#[derive(Debug)]
struct Watching {
    called: Called,
    setup: Option<Intent>,
    since: Option<Digest>,
    first: Option<Digest>,
    deadline: Option<Time>,
}
impl Forges {
    pub(crate) fn new() -> Self {
        Self {
            waiting: BTreeMap::new(),
            pending: BTreeMap::new(),
            watches: BTreeMap::new(),
            adopting: BTreeMap::new(),
            starting: BTreeMap::new(),
            json: false,
            connections: BTreeMap::new(),
            pages: BTreeMap::new(),
        }
    }
    pub(crate) fn calls(&self) -> usize {
        self.connections
            .len()
            .checked_add(self.pages.len())
            .expect("bounded calls")
            .checked_add(self.waiting.len())
            .expect("bounded calls")
            .checked_add(self.pending.len())
            .expect("bounded calls")
            .checked_add(self.starting.len())
            .expect("bounded calls")
            .checked_add(self.adopting.len())
            .expect("bounded calls")
            .checked_add(self.watches.len())
            .expect("bounded calls")
    }
    pub(crate) fn kept_json(&mut self, json: bool) {
        self.json = json;
    }
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for watch in self.watches.values() {
            if let Some(deadline) = watch.deadline {
                next = Some(match next {
                    Some(old) => deadline.min(old),
                    None => deadline,
                });
            }
        }
        next
    }
}
pub(crate) fn owns(call: &jig::call::Call) -> bool {
    let name = crate::table::TOOLS.get(usize::from(call.tool)).expect("parsed tool").name;
    name == READ.name
        || name == SET_UP.name
        || name == DEMO_START.name
        || name == CONNECT.name
        || name == INSTALL.name
        || name == ACCEPT.name
        || name == REPOSITORIES.name
        || (name == WAIT.name && call.form == 1)
}
fn live(client: &Client) -> bool {
    let Some(doc) = client.document(Handle { asker: crate::root::RUNNER.asker(), number: 0 }) else { return false };
    doc.state == State::Live
}
pub(crate) fn call(state: &mut Forges, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    let count = state.calls();
    if count >= usize::try_from(env.limits.tools.rows).expect("rows fit") {
        let printed = render::failure(b"call limit", called.json, 1);
        finish(called, printed, out);
    } else if live(client) {
        send(state, client, env, called, out);
    } else {
        state.waiting.insert(called.call_number, called);
    }
}
pub(crate) fn ready(state: &mut Forges, client: &Client, env: &Env<Limits>, out: &mut Queue<Out>) {
    if !live(client) {
        return;
    }
    for _ in 0..state.waiting.len() {
        let Some((_, called)) = state.waiting.pop_first() else { break };
        send(state, client, env, called, out);
    }
}
#[expect(clippy::manual_map, reason = "the domain matches without closures")]
fn send(state: &mut Forges, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    if repositories::owns(&called.call) {
        repositories::send(state, env, called, out);
        return;
    }
    if connection::owns(&called.call) {
        connection::send(state, client, env, called, out);
        return;
    }
    if called.value.is_some() || jig::argv::check(crate::table::TOOLS, &called.call, &env.limits.tools).is_err() {
        let printed = render::failure(b"invalid forge call", called.json, 1);
        finish(called, printed, out);
        return;
    }
    let number = (u64::from(called.call_number) << 32u32) | 2;
    let name = crate::table::TOOLS.get(usize::from(called.call.tool)).expect("parsed tool").name;
    if name == SET_UP.name
        && value(&called.call, 4).is_some()
        && (value(&called.call, 1).is_some() || value(&called.call, 2).is_some())
    {
        let printed = render::failure(b"--playground takes no --org or --repository", called.json, 1);
        finish(called, printed, out);
        return;
    }
    if name == SET_UP.name || name == DEMO_START.name {
        let intent = if name == SET_UP.name {
            let forge = u16::try_from(number_arg(&called.call)).expect("checked forge number");
            Some(intent(forge, &called.call, client))
        } else {
            None
        };
        let request = match &intent {
            Some(intent) => domain::ask(
                intent.forge,
                intent.org.clone(),
                intent.repository.clone(),
                intent.project.clone(),
                intent.playground,
                &env.limits.domain.forge,
            ),
            None => temper_client_domain::forge::demo::ask(&env.limits.domain.forge),
        };
        let Some(request) = request else {
            let printed = render::failure(b"set up encoding failed", called.json, 1);
            finish(called, printed, out);
            return;
        };
        let key = Key { asker: Owner::Forge.asker(), number };
        let name = match &called.call.key {
            Some(name) => Some(crate::key::engine(name)),
            None => None,
        };
        state.pending.insert(key, SettingUp { intent, called: Some(called) });
        out.push(Out::Client { event: Event::Send { key, name, request } });
    } else {
        let waiting = crate::table::TOOLS.get(usize::from(called.call.tool)).expect("parsed tool").name == WAIT.name;
        let since = match value(&called.call, 1) {
            Some(Value::Digest(bytes)) => Some(Digest::from_bytes(*bytes)),
            _ => None,
        };
        let deadline = if waiting {
            let Some(Value::Duration(timeout)) = value(&called.call, 3) else { unreachable!("checked timeout") };
            Some(env.now.checked_add(*timeout).unwrap_or(Time::from_nanos(u64::MAX)))
        } else {
            None
        };
        let handle = Handle { asker: Owner::Forge.asker(), number };
        state.watches.insert(handle, Watching { called, setup: None, since, first: None, deadline });
        out.push(Out::Client {
            event: Event::Watch {
                handle,
                subject: domain::watch(&env.limits.domain.forge).expect("checked forge limits"),
            },
        });
    }
}
fn number_arg(call: &jig::call::Call) -> u64 {
    number(call, 0).expect("checked number")
}
fn finish(called: Called, printed: jig::Printed, out: &mut Queue<Out>) {
    out.push(Out::Printed { number: called.number, printed });
}
pub(crate) fn told(state: &mut Forges, client: &Client, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) {
    if repositories::told(state, env, told, out) {
        return;
    }
    if connection::told(state, env, told, out) {
        return;
    }
    match told {
        Told::Kept { key, request } => restore(state, env, *key, request),
        Told::Answered { key, outcome } => {
            if let Some(mut starting) = state.starting.remove(key) {
                if let Some(adoption) = starting.value.answered(outcome) {
                    send_adoption(state, env, starting.called, adoption, out);
                } else {
                    let mut operation = render::outcome::Operation::of(
                        &starting.value.ask(&env.limits.domain.forge).expect("admitted project name"),
                    );
                    operation.word =
                        if connection::owns(&starting.called.call) { b"forge-connect" } else { b"forge-setup" };
                    let printed =
                        render::outcome::print(&operation, outcome, starting.called.json, None, &env.limits.tools);
                    finish(starting.called, printed, out);
                }
                return;
            }
            if let Some(adopting) = state.adopting.remove(key) {
                adopted(state, env, adopting, outcome, out);
                return;
            }
            let Some(pending) = state.pending.remove(key) else { return };
            let json = match &pending.called {
                Some(called) => called.json,
                None => state.json,
            };
            let printed = match &pending.intent {
                Some(intent) => setup_print(intent.forge, outcome, json, pending.called.is_none(), &env.limits),
                None => crate::demo::print(outcome, json, pending.called.is_none(), &env.limits),
            };
            match pending.called {
                Some(called) if printed.status == 0 && pending.intent.is_some() => {
                    out.push(Out::Progress {
                        line: if called.json { render::outcome::newline(&printed.stdout) } else { printed.stdout },
                    });
                    watch_first(state, env, *key, called, pending.intent.expect("setup intent"), out);
                }
                Some(called) => finish(called, printed, out),
                None => out.push(Out::Kept { line: if printed.status == 0 { printed.stdout } else { printed.stderr } }),
            }
        }
        Told::Changed { handle } if state.watches.contains_key(handle) => changed(state, client, env, *handle, out),
        Told::Ended { handle, why } if state.watches.contains_key(handle) => {
            let watch = state.watches.remove(handle).expect("active watch");
            let (why, status): (&[u8], u8) = match why {
                Why::Unknown => (b"unknown", 2),
                Why::NotAllowed => (b"not_allowed", 2),
                Why::SignedOut => (b"signed out", 1),
                Why::NotServed => (b"version not served", 1),
                Why::Limit => (b"watch table limit", 1),
                Why::SubjectEnded => (b"ended", 3),
            };
            let printed = render::failure(why, watch.called.json, status);
            finish(watch.called, printed, out);
        }
        Told::Link { .. }
        | Told::Dropped { .. }
        | Told::Pending { .. }
        | Told::Changed { .. }
        | Told::Ended { .. }
        | Told::Text { .. }
        | Told::Page { .. } => {}
    }
}
fn restore(state: &mut Forges, env: &Env<Limits>, key: Key, request: &jig_client::Ask) {
    let jig_client::Ask::Family { scope: jig_client::contract::Scope::Deployment, record } = request else {
        return;
    };
    if record.family != temper_client_protocol_wire::FORGE
        || record.version != temper_client_protocol_wire::CLIENT_VERSION
    {
        return;
    }
    let Some(limits) = temper_client_domain::forge::wire_limits(&env.limits.domain.forge) else { return };
    let Some(request) = temper_client_protocol_wire::decode_requesting(&record.bytes, &limits) else { return };
    let intent = match request.into_parts().request {
        temper_client_protocol_wire::v1::Request::SetUp(request) => {
            let request = request.into_parts();
            Some(Intent {
                playground: request.playground,
                forge: request.forge,
                org: request.org,
                repository: request.repository,
                project: match request.project {
                    temper_client_protocol_wire::v1::SetUpProject::Existing(project) => {
                        SetUpProject::Existing(project.project())
                    }
                    temper_client_protocol_wire::v1::SetUpProject::New(project) => {
                        SetUpProject::New(project.into_parts().name)
                    }
                },
            })
        }
        temper_client_protocol_wire::v1::Request::StartDemo => None,
        temper_client_protocol_wire::v1::Request::Reachable(_)
        | temper_client_protocol_wire::v1::Request::Adopt(_)
        | temper_client_protocol_wire::v1::Request::Return(_)
        | temper_client_protocol_wire::v1::Request::Connect(_)
        | temper_client_protocol_wire::v1::Request::Accept(_)
        | temper_client_protocol_wire::v1::Request::Install(_) => return,
    };
    state.pending.insert(key, SettingUp { intent, called: None });
}
// MeProject does not expose a home, and forge-setup reads only me and forges.
// A taken home can therefore be discovered only by the adoption's refusal.
fn adopted(state: &mut Forges, env: &Env<Limits>, mut adopting: Adopting, outcome: &Outcome, out: &mut Queue<Out>) {
    if adopting.adoption.home && adoption_refused(outcome, &env.limits) {
        adopting.adoption.home = false;
        let request = temper_client_domain::forge::ask(&adopting.adoption, &env.limits.domain.forge)
            .expect("the admitted adoption with only home cleared still encodes");
        let name = temper_client_domain::forge::first_repository_key(adopting.adoption.forge, true);
        let key = Key { asker: Owner::Forge.asker(), number: (u64::from(adopting.called.call_number) << 32u32) | 4 };
        state.adopting.insert(key, adopting);
        out.push(Out::Client { event: Event::Send { key, name: Some(name), request } });
    } else {
        let mut printed = crate::forge::print(&adopting.adoption, outcome, adopting.called.json, false, &env.limits);
        if printed.status == 0 && connection::owns(&adopting.called.call) {
            let project = u64::from(adopting.adoption.project);
            let mut measure = Text::measure();
            project_answer(&mut measure, adopting.called.json, project, &printed.stdout);
            let mut text = Text::writing_exact(measure);
            project_answer(&mut text, adopting.called.json, project, &printed.stdout);
            printed.stdout = text.finish();
            if printed.stdout.len() > usize::try_from(env.limits.tools.text).expect("text fits") {
                printed = render::failure(b"result limit", adopting.called.json, 1);
            }
        }
        finish(adopting.called, printed, out);
    }
}
fn project_answer(out: &mut Text, json: bool, project: u64, bytes: &[u8]) {
    out.put(if json { b"{\"project\":" } else { b"project " });
    out.number(project);
    out.put(if json { b",\"outcome\":" } else { b"\n" });
    out.put(bytes);
    if json {
        out.put(b"}");
    }
}
fn adoption_refused(outcome: &Outcome, limits: &Limits) -> bool {
    let Outcome::Answered(answer) = outcome else { return false };
    temper_client_domain::forge::answer(&limits.domain, answer)
        == Some(temper_client_domain::forge::Answer::ForgeRefused(temper_client_domain::forge::Reason::Refused))
}
fn rows(client: &Client, handle: Handle, limits: &Limits) -> Option<Box<[Row]>> {
    let Latest::Family(record) = client.document(handle)?.latest? else { return None };
    domain::rows(record, &limits.domain.forge)
}
#[expect(clippy::manual_map, reason = "the domain matches without closures")]
fn changed(state: &mut Forges, client: &Client, env: &Env<Limits>, handle: Handle, out: &mut Queue<Out>) {
    let Some(doc) = client.document(handle) else { return };
    if doc.state != State::Live {
        return;
    }
    let Some(rows) = rows(client, handle, &env.limits) else {
        let watch = state.watches.remove(&handle).expect("active watch");
        out.push(Out::Client { event: Event::Unwatch { handle } });
        let printed = render::failure(b"forges document decoding failed", watch.called.json, 1);
        finish(watch.called, printed, out);
        return;
    };
    if state.watches.get(&handle).expect("active watch").setup.is_some() {
        setup_changed(state, env, handle, &rows, out);
        return;
    }
    let watch = state.watches.get_mut(&handle).expect("active watch");
    let Some(digest) = digest(&rows, &env.limits) else {
        let printed = render::failure(b"result limit", watch.called.json, 1);
        complete(state, handle, printed, out);
        return;
    };
    let condition = match value(&watch.called.call, 2) {
        Some(Value::Text(bytes)) => Some(bytes.as_ref()),
        _ => None,
    };
    let verdict = match condition {
        Some(condition) => Some(holds(&rows, condition)),
        None => None,
    };
    let met = jig::wait::met(watch.since, watch.first, digest, verdict);
    if watch.deadline.is_none() || met {
        let printed = print_rows(
            Some(&rows),
            watch.called.json,
            match watch.deadline {
                Some(_) => Some(true),
                None => None,
            },
            &env.limits,
        );
        complete(state, handle, printed, out);
    } else if watch.first.is_none() {
        watch.first = Some(digest);
    }
}
fn complete(state: &mut Forges, handle: Handle, printed: jig::Printed, out: &mut Queue<Out>) {
    let watch = state.watches.remove(&handle).expect("active watch");
    out.push(Out::Client { event: Event::Unwatch { handle } });
    finish(watch.called, printed, out);
}
pub(crate) fn fire(state: &mut Forges, client: &Client, env: &Env<Limits>, out: &mut Queue<Out>) {
    let mut next = None;
    for _ in 0..state.watches.len() {
        let entry = match next {
            None => state.watches.first_key_value(),
            Some(handle) => {
                state.watches.range((core::ops::Bound::Excluded(handle), core::ops::Bound::Unbounded)).next()
            }
        };
        let Some((&handle, watch)) = entry else { break };
        next = Some(handle);
        if let Some(deadline) = watch.deadline
            && deadline <= env.now
        {
            let rows = rows(client, handle, &env.limits);
            let printed = print_rows(rows.as_deref(), watch.called.json, Some(false), &env.limits);
            complete(state, handle, printed, out);
        }
    }
}
#[must_use]
pub(crate) fn holds(rows: &[Row], condition: &[u8]) -> bool {
    if condition == b"settled" {
        for row in rows {
            if !settled(&row.connection) {
                return false;
            }
        }
        return true;
    }
    let (digits, made) = match condition.strip_prefix(b"made:") {
        Some(digits) => (digits, true),
        None => match condition.strip_prefix(b"connected:") {
            Some(digits) => (digits, false),
            None => return false,
        },
    };
    let mut number = 0u16;
    if digits.is_empty() {
        return false;
    }
    for byte in digits {
        if !byte.is_ascii_digit() {
            return false;
        }
        let Some(next) = number.checked_mul(10) else {
            return false;
        };
        let Some(next) = next.checked_add(u16::from(byte.checked_sub(b'0').expect("ASCII digit"))) else {
            return false;
        };
        number = next;
    }
    for row in rows {
        if row.number == number {
            if made {
                return match &row.first {
                    Some(first) => match first.state {
                        FirstState::Made { .. } => true,
                        FirstState::Making | FirstState::Failed { .. } => false,
                    },
                    None => false,
                };
            }
            return match &row.connection {
                Connection::Connected { .. } => true,
                Connection::NotConnected
                | Connection::NotSetUp
                | Connection::SettingUp { .. }
                | Connection::SetUpFailed { .. }
                | Connection::Connecting
                | Connection::Stopped { .. } => false,
            };
        }
    }
    false
}

// Seven containers at most: wait result, row document, forges array, row,
// installations array, installation, account. Reads and digests omit the wait
// result, but share its bound so every encoding of the rows stays in step.
const ROWS_JSON_DEPTH: u32 = 7;

fn encode_rows(rows: &[Row], digest: Option<Digest>, out: &mut Encoder) {
    out.object_start();
    out.key(b"forges");
    out.array_start();
    for row in rows {
        out.object_start();
        out.key(b"number");
        out.unsigned(u64::from(row.number));
        out.key(b"kind");
        out.string(match row.kind {
            domain::Kind::Forgejo => b"forgejo",
            domain::Kind::GitHub => b"github",
        });
        out.key(b"read_only");
        out.boolean(row.read_only);
        out.key(b"mode");
        out.string(words::forge_mode(row.mode));
        out.key(b"offer");
        out.string(match row.offer {
            Offer::NotOffered => b"not_offered",
            Offer::NotUnderTheBox => b"not_under_the_box",
            Offer::Offered => b"offered",
        });
        out.key(b"state");
        match &row.connection {
            Connection::NotConnected => out.string(b"not_connected"),
            Connection::NotSetUp => out.string(b"not_set_up"),
            Connection::SettingUp { step } => {
                out.string(b"setting_up");
                if let Some(step) = step {
                    out.key(b"step");
                    out.string(match step {
                        Step::Fetch => b"fetch",
                        Step::Prepare => b"prepare",
                        Step::Run => b"run",
                    });
                }
            }
            Connection::SetUpFailed { why, .. } => {
                out.string(b"set_up_failed");
                out.key(b"why");
                out.string(why);
            }
            Connection::Connecting => out.string(b"connecting"),
            Connection::Connected { id, login } => {
                out.string(b"connected");
                out.key(b"id");
                out.unsigned(*id);
                out.key(b"login");
                out.string(login);
            }
            Connection::Stopped { reason } => {
                out.string(b"stopped");
                out.key(b"reason");
                out.string(reason);
            }
        }
        out.key(b"installations");
        out.array_start();
        for installation in &row.installations {
            out.object_start();
            out.key(b"id");
            out.unsigned(installation.id);
            out.key(b"account");
            match &installation.account {
                Some(account) => {
                    out.object_start();
                    out.key(b"id");
                    out.unsigned(account.id);
                    out.key(b"login");
                    out.string(&account.login);
                    out.object_end();
                }
                None => out.null(),
            }
            out.key(b"suspended");
            out.boolean(installation.suspended);
            out.key(b"stray");
            out.boolean(installation.stray);
            out.object_end();
        }
        out.array_end();
        out.key(b"ended");
        match &row.ended {
            Some(why) => out.string(why),
            None => out.null(),
        }
        out.key(b"first");
        match &row.first {
            Some(first) => first_json(first, out),
            None => out.null(),
        }
        out.object_end();
    }
    out.array_end();
    if let Some(digest) = digest {
        out.key(b"digest");
        out.string(&digest.hex());
    }
    out.object_end();
}
fn json_rows(rows: Option<&[Row]>, digest: Option<Digest>, met: Option<bool>, out: &mut Encoder) {
    if let Some(met) = met {
        out.object_start();
        out.key(b"forges");
        match rows {
            Some(rows) => encode_rows(rows, digest, out),
            None => out.null(),
        }
        out.key(b"met");
        out.boolean(met);
        if !met {
            out.key(b"why");
            out.string(b"timeout");
        }
        out.object_end();
    } else {
        encode_rows(rows.expect("read has rows"), digest, out);
    }
}
fn text_rows(rows: Option<&[Row]>, digest: Option<Digest>, met: Option<bool>, out: &mut Text) {
    if let Some(rows) = rows {
        for row in rows {
            out.put(b"forge ");
            out.number(u64::from(row.number));
            out.put(b" \xc2\xb7 ");
            out.put(words::forge_kind(row.kind));
            out.put(b" \xc2\xb7 ");
            out.put(words::forge_mode(row.mode));
            out.put(b" \xc2\xb7 ");
            out.put(&words::forge_connection(row));
            if let Some(slug) = domain::slug(row) {
                out.put(b" \xc2\xb7 ");
                if row.installations.is_empty() {
                    out.put(&words::app_line(row).expect("GitHub App slug"));
                } else {
                    out.put(&words::lossy(slug));
                }
            }
            if let Some(last) = words::last_connect(row) {
                out.put(b" \xc2\xb7 ");
                out.put(&last);
            }
            if let Some(first) = &row.first {
                out.put(b" \xc2\xb7 ");
                out.put(&words::first_repository(first));
            }
            for installation in &row.installations {
                out.put(b"\n  ");
                if installation.account.is_some() {
                    out.put(b"installation ");
                    out.number(installation.id);
                    out.put(b" \xc2\xb7 ");
                }
                out.put(&words::installation_words(installation, row.read_only));
                if installation.stray {
                    out.put(b" \xc2\xb7 forge-accept ");
                    out.number(u64::from(row.number));
                    out.put(b" ");
                    out.number(installation.id);
                }
            }
            out.put(b"\n");
        }
        if let Some(digest) = digest {
            out.put(b"digest: forges ");
            out.put(&digest.hex());
            out.put(b"\n");
        }
    }
    match met {
        Some(true) => out.put(b"met\n"),
        Some(false) => out.put(b"not met: timeout\n"),
        None => {}
    }
}
fn digest(rows: &[Row], limits: &Limits) -> Option<Digest> {
    let bounds = skein_json::writer::Limits { depth: ROWS_JSON_DEPTH, length: limits.tools.text };
    let mut measure = Encoder::measure(&bounds);
    encode_rows(rows, None, &mut measure);
    let mut out = Encoder::write(measure.measured().ok()?, &bounds);
    encode_rows(rows, None, &mut out);
    Some(Digest::of(&out.finish()))
}
pub(crate) fn print_rows(rows: Option<&[Row]>, json: bool, met: Option<bool>, limits: &Limits) -> jig::Printed {
    let digest = match rows {
        Some(rows) => match digest(rows, limits) {
            Some(digest) => Some(digest),
            None => return render::failure(b"result limit", json, 1),
        },
        None => None,
    };
    let bytes = if json {
        let bounds = skein_json::writer::Limits { depth: ROWS_JSON_DEPTH, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        json_rows(rows, digest, met, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", json, 1) };
        let mut out = Encoder::write(size, &bounds);
        json_rows(rows, digest, met, &mut out);
        out.finish()
    } else {
        let mut measure = Text::measure();
        text_rows(rows, digest, met, &mut measure);
        let mut out = Text::writing_exact(measure);
        text_rows(rows, digest, met, &mut out);
        out.finish()
    };
    if bytes.len() > usize::try_from(limits.tools.text).expect("text fits") {
        return render::failure(b"result limit", json, 1);
    }
    jig::Printed {
        file: jig::FileEnd::Keep,
        status: if met == Some(false) { 3 } else { 0 },
        stdout: bytes,
        stderr: Box::default(),
    }
}
fn setup_text(forge: u16, answer: &SetUpAnswer, kept: bool, limits: &Limits, out: &mut Text) {
    if kept {
        out.put(b"kept: forge-setup ");
    }
    match answer {
        SetUpAnswer::Outcome(SetUpOutcome::SettingUp) => out.put(b"setting up\n"),
        SetUpAnswer::Outcome(SetUpOutcome::Refused(why)) => {
            out.put(b"refused: ");
            out.put(words::set_up_refusal(*why));
            out.put(b"\n");
            if *why == SetUpRefusal::NotUnderTheBox {
                out.put(&words::set_up_commands(forge));
                out.put(b"\n");
            }
        }
        SetUpAnswer::Outcome(SetUpOutcome::Unknown) => out.put(b"unknown\n"),
        SetUpAnswer::EntranceRefused(why) => {
            render::acts::refusal(why.refusal(), &render::document::phrasebook(limits.tools.text), out);
            out.put(b"\n");
        }
    }
}
fn setup_json(forge: u16, answer: &SetUpAnswer, kept: bool, out: &mut Encoder) {
    if kept {
        out.object_start();
        out.key(b"operation");
        out.string(b"forge-setup");
        out.key(b"outcome");
    }
    out.object_start();
    match answer {
        SetUpAnswer::Outcome(SetUpOutcome::SettingUp) => {
            out.key(b"setting_up");
            out.object_start();
            out.object_end();
        }
        SetUpAnswer::Outcome(SetUpOutcome::Refused(why)) => {
            out.key(b"refused");
            out.object_start();
            out.key(b"why");
            out.string(match why {
                SetUpRefusal::NotOffered => b"not_offered",
                SetUpRefusal::NotUnderTheBox => b"not_under_the_box",
                SetUpRefusal::Busy => b"busy",
                SetUpRefusal::NewProject => b"new_project",
                SetUpRefusal::Invalid => b"invalid",
            });
            out.object_end();
            if *why == SetUpRefusal::NotUnderTheBox {
                out.key(b"next");
                out.string(&words::set_up_commands(forge));
            }
        }
        SetUpAnswer::Outcome(SetUpOutcome::Unknown) => {
            out.key(b"unknown");
            out.object_start();
            out.object_end();
        }
        SetUpAnswer::EntranceRefused(why) => {
            out.key(b"entrance_refused");
            out.object_start();
            out.key(b"refusal");
            render::document::refusal_json(why.refusal(), out);
            out.object_end();
        }
    }
    out.object_end();
    if kept {
        out.object_end();
    }
}
pub(crate) fn setup_print(forge: u16, outcome: &Outcome, json: bool, kept: bool, limits: &Limits) -> jig::Printed {
    let answer = match outcome {
        Outcome::Answered(answer) => domain::answer(answer, &limits.domain.forge),
        Outcome::Unknown => Some(SetUpAnswer::Outcome(SetUpOutcome::Unknown)),
        Outcome::Held | Outcome::Limit | Outcome::Unconfirmed | Outcome::NotKept | Outcome::Unsent | Outcome::Lost => {
            let mut operation = render::outcome::Operation::of(
                &domain::ask(
                    forge,
                    b"work".as_slice().into(),
                    b"hello".as_slice().into(),
                    SetUpProject::New(b"hello".as_slice().into()),
                    false,
                    &limits.domain.forge,
                )
                .expect("checked limits"),
            );
            operation.word = b"forge-setup";
            if kept {
                return jig::Printed {
                    file: jig::FileEnd::Keep,
                    status: 1,
                    stdout: Box::default(),
                    stderr: render::outcome::kept(&operation, outcome, json, None, &limits.tools),
                };
            }
            return render::outcome::print(&operation, outcome, json, None, &limits.tools);
        }
    };
    let Some(answer) = answer else { return render::failure(b"set up answer decoding failed", json, 1) };
    let status = match answer {
        SetUpAnswer::Outcome(SetUpOutcome::SettingUp) => 0,
        SetUpAnswer::Outcome(SetUpOutcome::Unknown | SetUpOutcome::Refused(_)) | SetUpAnswer::EntranceRefused(_) => 2,
    };
    let bytes = if json {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        setup_json(forge, &answer, kept, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut out = Encoder::write(size, &bounds);
        setup_json(forge, &answer, kept, &mut out);
        let bytes = out.finish();
        if kept { render::outcome::newline(&bytes) } else { bytes }
    } else {
        let mut measure = Text::measure();
        setup_text(forge, &answer, kept, limits, &mut measure);
        let mut out = Text::writing_exact(measure);
        setup_text(forge, &answer, kept, limits, &mut out);
        out.finish()
    };
    if status == 0 {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: bytes, stderr: Box::default() }
    } else {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: Box::default(), stderr: bytes }
    }
}

fn settled(connection: &Connection) -> bool {
    match connection {
        Connection::SettingUp { .. } | Connection::Connecting => false,
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SetUpFailed { .. }
        | Connection::Connected { .. }
        | Connection::Stopped { .. } => true,
    }
}

fn intent(forge: u16, call: &jig::call::Call, client: &Client) -> Intent {
    use jig::call::Project;
    let playground = value(call, 4).is_some();
    let org = match value(call, 1) {
        Some(Value::Text(bytes)) => bytes.clone(),
        _ => {
            if playground {
                b"playground".as_slice().into()
            } else {
                b"work".as_slice().into()
            }
        }
    };
    let repository = match value(call, 2) {
        Some(Value::Text(bytes)) => bytes.clone(),
        _ => {
            if playground {
                b"widget".as_slice().into()
            } else {
                b"hello".as_slice().into()
            }
        }
    };
    let project = match value(call, 3) {
        Some(Value::Project(Project::Number(number))) => SetUpProject::Existing(*number),
        Some(Value::Project(Project::Name(name))) => {
            let mut found = None;
            if let Some(me) = crate::forge::me(client) {
                for project in &me.projects {
                    if project.name.bytes() == name.as_ref() {
                        found = Some(project.project);
                        break;
                    }
                }
            }
            match found {
                Some(number) => SetUpProject::Existing(number),
                None => SetUpProject::New(name.clone()),
            }
        }
        _ => SetUpProject::New(repository.clone()),
    };
    Intent { playground, forge, org, repository, project }
}
// Both ways of making a first repository finish through the same watch and Adopt.
fn watch_first(state: &mut Forges, env: &Env<Limits>, key: Key, called: Called, intent: Intent, out: &mut Queue<Out>) {
    let handle = Handle { asker: Owner::Forge.asker(), number: key.number };
    state.watches.insert(
        handle,
        Watching {
            called,
            setup: Some(intent),
            since: None,
            first: None,
            deadline: Some(env.now.checked_add(env.limits.tools.wait).unwrap_or(Time::from_nanos(u64::MAX))),
        },
    );
    out.push(Out::Client {
        event: Event::Watch { handle, subject: domain::watch(&env.limits.domain.forge).expect("checked forge limits") },
    });
}
fn admin_stop(stop: domain::AdminStop, out: &mut Text) {
    use domain::{AdminStop, SetupStage};
    match stop {
        AdminStop::NotAdmin => out.put(b"not a site admin"),
        AdminStop::Unmarked => out.put(b"unmarked bot login"),
        AdminStop::Other => out.put(b"admin connection stopped"),
        AdminStop::Refused { status, stage } => {
            out.put(b"admin request refused: ");
            out.number(u64::from(status));
            out.put(b" at ");
            out.put(match stage {
                SetupStage::Admin => b"admin",
                SetupStage::Bot => b"bot",
                SetupStage::Token => b"token",
                SetupStage::Org => b"organization",
                SetupStage::Repository => b"repository",
                SetupStage::File => b"starter workflow",
                SetupStage::App => b"app",
                SetupStage::Installation => b"installation",
                SetupStage::State => b"state",
            });
        }
    }
}
fn setup_changed(state: &mut Forges, env: &Env<Limits>, handle: Handle, rows: &[Row], out: &mut Queue<Out>) {
    let watch = state.watches.get(&handle).expect("active setup watch");
    let intent = watch.setup.as_ref().expect("setup intent");
    for row in rows {
        if row.number != intent.forge {
            continue;
        }
        let mut failed = match &row.first {
            Some(first) => match first.state {
                FirstState::Failed { .. } => Some(words::first_repository(first)),
                FirstState::Making | FirstState::Made { .. } => None,
            },
            None => None,
        };
        if failed.is_none() {
            failed = match &row.connection {
                Connection::SetUpFailed { .. } | Connection::Stopped { .. } => Some(words::forge_connection(row)),
                Connection::NotConnected
                | Connection::NotSetUp
                | Connection::SettingUp { .. }
                | Connection::Connecting
                | Connection::Connected { .. } => None,
            };
        }
        if failed.is_none()
            && let Some(stop) = row.ended_stop
        {
            let mut measure = Text::measure();
            admin_stop(stop, &mut measure);
            let mut text = Text::writing_exact(measure);
            admin_stop(stop, &mut text);
            failed = Some(text.finish());
        }
        if let Some(why) = failed {
            let printed = render::failure(&why, watch.called.json, 1);
            complete(state, handle, printed, out);
            return;
        }
        // Create completes before Connect; Adopt needs the forge able to answer.
        if let Some(First { state: FirstState::Made { .. }, .. }) = &row.first
            && let Connection::Connected { .. } = &row.connection
        {
            let watch = state.watches.remove(&handle).expect("active setup watch");
            out.push(Out::Client { event: Event::Unwatch { handle } });
            let intent = watch.setup.expect("setup intent");
            let project = match intent.project {
                SetUpProject::Existing(project) => project,
                SetUpProject::New(_) => 0,
            };
            let adoption = temper_client_domain::forge::Adoption {
                project,
                forge: intent.forge,
                owner: intent.org,
                name: intent.repository,
                landing: b"main".as_slice().into(),
                role: temper_client_domain::forge::Role::Owned,
                home: true,
                ci: true,
                prefix: b"temper/".as_slice().into(),
                checks: Box::default(),
            };
            match intent.project {
                SetUpProject::Existing(_) => send_adoption(state, env, watch.called, adoption, out),
                SetUpProject::New(name) => {
                    let value = temper_client_domain::forge::start::StartIn::new(adoption, name);
                    let Some(request) = value.ask(&env.limits.domain.forge) else {
                        let printed = render::failure(b"project encoding failed", watch.called.json, 1);
                        finish(watch.called, printed, out);
                        return;
                    };
                    let name = temper_client_domain::forge::start_project_key(intent.forge, 0);
                    // Slot 5 belongs to the pasted Connect; project creation gets its own key.
                    let key =
                        Key { asker: Owner::Forge.asker(), number: (u64::from(watch.called.call_number) << 32u32) | 6 };
                    state.starting.insert(key, Starting { called: watch.called, value });
                    out.push(Out::Client { event: Event::Send { key, name: Some(name), request } });
                }
            }
            return;
        }
    }
}
fn send_adoption(
    state: &mut Forges,
    env: &Env<Limits>,
    called: Called,
    adoption: temper_client_domain::forge::Adoption,
    out: &mut Queue<Out>,
) {
    let Some(request) = temper_client_domain::forge::ask(&adoption, &env.limits.domain.forge) else {
        let printed = render::failure(b"adoption encoding failed", called.json, 1);
        finish(called, printed, out);
        return;
    };
    let name = temper_client_domain::forge::first_repository_key(adoption.forge, false);
    let key = Key { asker: Owner::Forge.asker(), number: (u64::from(called.call_number) << 32u32) | 3 };
    state.adopting.insert(key, Adopting { called, adoption });
    out.push(Out::Client { event: Event::Send { key, name: Some(name), request } });
}
fn first_json(first: &First, out: &mut Encoder) {
    out.object_start();
    out.key(b"org");
    out.string(&first.org);
    out.key(b"repository");
    out.string(&first.repository);
    out.key(b"project");
    out.object_start();
    match &first.project {
        SetUpProject::Existing(project) => {
            out.key(b"existing");
            out.unsigned(u64::from(*project));
        }
        SetUpProject::New(name) => {
            out.key(b"new");
            out.string(name);
        }
    }
    out.object_end();
    out.key(b"playground");
    out.boolean(first.playground);
    out.key(b"state");
    match first.state {
        FirstState::Making => out.string(b"making"),
        FirstState::Made { id } => {
            out.string(b"made");
            out.key(b"id");
            out.unsigned(id);
        }
        FirstState::Failed { step } => {
            out.string(b"failed");
            out.key(b"step");
            out.string(match step {
                domain::CreateStep::Admin => b"admin",
                domain::CreateStep::Org => b"org",
                domain::CreateStep::Repository => b"repository",
                domain::CreateStep::File => b"file",
            });
        }
    }
    out.object_end();
}
