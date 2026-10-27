//! Landing views (domain/landing.md, 14; client/tools.md, 5.1).
use crate::{
    jig::{
        self,
        call::{Value, number, value},
        render::{self, Text},
        runner::{Called, Out},
        table::{
            Keyed, Kind,
            Occurs::{Optional, Required},
            Tool, flag, positional, switch,
        },
    },
    root::Limits,
};
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{Client, Event, Handle, Latest, Readable, Readout, State, Told, Watchable, Why, contract as c};
use skein_json::writer::Encoder;
use skein_lib::{Env, Queue};
use temper_client_domain::{forge, owner::Owner};
use temper_client_protocol_wire::{self as wire, v1 as w};

pub const READ: Tool = Tool {
    name: b"landing show",
    line: b"Read a landing: its members, their candidates, each gate's proof and the turn",
    prints: b"`landing TASK`, its project and phase, each member's change, pull request, candidate and gates, and the turn",
    result: b"landing",
    forms: &[&[
        positional(b"task", b"TASK", Kind::NumberRange { min: 1, max: u64::MAX }, Required, b"The landing task's number"),
        flag(b"project", b"PROJECT", Kind::Project { empty: false }, Optional, b"Its project; the task's when absent"),
    ]],
    keyed: Keyed::Never,
    area: 6,
    notes: b"A landing lands its changes by turn: each gate is proved by a receipt\nof its key, then main is fast-forwarded to each candidate. A task with\nno landing fails, `no landing for task TASK` (exit 2).\n",
};
pub const LIST: Tool = Tool {
    name: b"landing list",
    line: b"List a project's landings, the turn's holder first, then newest first",
    prints:
        b"`landings of project PROJECT`, the turn, and a line per landing: its task, phase, changes and repositories",
    result: b"landings",
    forms: &[&[
        flag(b"project", b"PROJECT", Kind::Project { empty: false }, Required, b"The project"),
        switch(b"ended", b"Also the landings that ended"),
    ]],
    keyed: Keyed::Never,
    area: 6,
    notes: b"Without --ended, the live landings alone. `landing show TASK` reads one.\n",
};

#[derive(Debug)]
pub(crate) struct Landings {
    calls: BTreeMap<u32, Watching>,
}
#[derive(Debug)]
struct Watching {
    called: Called,
    task: Option<u64>,
    project: Option<u32>,
    started: bool,
    panel: Option<Handle>,
    read: Option<Handle>,
}
impl Landings {
    pub(crate) fn new() -> Self {
        Self { calls: BTreeMap::new() }
    }
    pub(crate) fn calls(&self) -> usize {
        self.calls.len()
    }
}
pub(crate) fn owns(call: &jig::call::Call) -> bool {
    let name = crate::table::TOOLS.get(usize::from(call.tool)).expect("parsed tool").name;
    name == READ.name || name == LIST.name
}
pub(crate) fn call(state: &mut Landings, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    if called.value.is_some() || jig::argv::check(crate::table::TOOLS, &called.call, &env.limits.tools).is_err() {
        out.push(Out::Printed {
            number: called.number,
            printed: render::failure(b"invalid landing call", called.json, 1),
        });
        return;
    }
    let task = if crate::table::TOOLS.get(usize::from(called.call.tool)).expect("parsed tool").name == READ.name {
        Some(number(&called.call, 0).expect("checked task"))
    } else {
        None
    };
    state
        .calls
        .insert(called.call_number, Watching { called, task, project: None, started: false, panel: None, read: None });
    ready(state, client, env, out);
}
fn handle(watch: &Watching, serial: u64) -> Handle {
    Handle { asker: Owner::Forge.asker(), number: (u64::from(watch.called.call_number) << 32_u32) | serial }
}
pub(crate) fn ready(state: &mut Landings, client: &Client, env: &Env<Limits>, out: &mut Queue<Out>) {
    let Some(me) = crate::forge::me(client) else { return };
    for _ in 0..state.calls.len() {
        let mut next = None;
        for (&number, watch) in &state.calls {
            if !watch.started {
                next = Some(number);
                break;
            }
        }
        let Some(number) = next else { break };
        let watch = state.calls.get_mut(&number).expect("active landing read");
        watch.started = true;
        let project = value(&watch.called.call, usize::from(watch.task.is_some()));
        watch.project = match project {
            Some(Value::Project(jig::call::Project::Number(n))) => Some(*n),
            Some(Value::Project(jig::call::Project::Name(name))) => {
                let mut found = None;
                for project in &me.projects {
                    if project.name.bytes() == name.as_ref() {
                        found = Some(project.project);
                        break;
                    }
                }
                found
            }
            _ => None,
        };
        if project.is_some() && watch.project.is_none() {
            let printed = render::failure(b"unknown project", watch.called.json, 1);
            complete(state, number, printed, out);
        } else if watch.project.is_some() {
            open(watch, &env.limits, out);
        } else {
            let handle = handle(watch, 1);
            watch.read = Some(handle);
            out.push(Out::Client {
                event: Event::Read {
                    handle,
                    read: Readable::Jig(c::Read::Task { task: watch.task.expect("landing task") }),
                },
            });
        }
    }
}
fn open(watch: &mut Watching, limits: &Limits, out: &mut Queue<Out>) {
    let handle = handle(watch, 2);
    watch.panel = Some(handle);
    out.push(Out::Client {
        event: Event::Watch {
            handle,
            subject: subject(
                watch.task,
                watch.project.expect("resolved project"),
                value(&watch.called.call, 1).is_some() && watch.task.is_none(),
                limits,
            ),
        },
    });
}
pub(crate) fn subject(task: Option<u64>, project: u32, ended: bool, limits: &Limits) -> Watchable {
    let limits = forge::wire_limits(&limits.domain.forge).expect("checked limits");
    let watch = match task {
        Some(task) => {
            w::Watch::Landing(w::LandingSubject::new(&limits, w::LandingSubjectParts { task }).expect("subject"))
        }
        None => w::Watch::Landings(
            w::LandingsSubject::new(&limits, w::LandingsSubjectParts { project, ended }).expect("subject"),
        ),
    };
    let watching = w::Watching::new(&limits, w::WatchingParts { watch }).expect("watch");
    Watchable::Family {
        scope: c::Scope::Project(project),
        window: c::Window { rows: 0, after: None },
        record: jig_client::Family {
            family: wire::FORGE,
            version: wire::CLIENT_VERSION,
            bytes: wire::watching(&watching),
        },
    }
}
fn complete(state: &mut Landings, number: u32, printed: jig::Printed, out: &mut Queue<Out>) {
    let watch = state.calls.remove(&number).expect("active landing read");
    if let Some(handle) = watch.panel {
        out.push(Out::Client { event: Event::Unwatch { handle } });
    }
    out.push(Out::Printed { number: watch.called.number, printed });
}
fn missing(task: u64, json: bool) -> jig::Printed {
    let mut measure = Text::measure();
    measure.put(b"no landing for task ");
    measure.number(task);
    let mut text = Text::writing_exact(measure);
    text.put(b"no landing for task ");
    text.number(task);
    render::failure(&text.finish(), json, 2)
}
fn gone(watch: &Watching, limits: &Limits) -> jig::Printed {
    match watch.task {
        Some(task) => missing(task, watch.called.json),
        None => print_list(
            &w::LandingsPanel::new(
                &w::CEILINGS,
                w::LandingsPanelParts {
                    project: watch.project.expect("resolved project"),
                    turn: None,
                    landings: skein_lib::List::with_capacity(0),
                },
            )
            .expect("empty list"),
            watch.called.json,
            limits,
        ),
    }
}
pub(crate) fn told(state: &mut Landings, client: &Client, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) {
    let handle = match told {
        Told::Changed { handle } | Told::Ended { handle, .. } | Told::Page { handle, .. } => *handle,
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Text { .. } => return,
    };
    let number = u32::try_from(handle.number >> 32_u32).expect("call number");
    let Some(watch) = state.calls.get_mut(&number) else { return };
    let printed = match told {
        Told::Page { page, .. } if watch.read == Some(handle) => {
            watch.read = None;
            let task = match page {
                Readout::Jig(c::ReadResult::Value(value)) => render::read_task(value),
                Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => None,
            };
            if let Some((task, _)) = task {
                if Some(task.chip.task) != watch.task {
                    let printed = render::failure(b"unexpected task document", watch.called.json, 1);
                    complete(state, number, printed, out);
                    return;
                }
                watch.project = Some(task.project);
                open(watch, &env.limits, out);
                return;
            }
            match page {
                Readout::Jig(c::ReadResult::None(c::ReadNone::Unknown | c::ReadNone::Gone)) => {
                    missing(watch.task.expect("landing task"), watch.called.json)
                }
                Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => render::read(
                    page,
                    client,
                    Handle { asker: crate::root::RUNNER.asker(), number: 0 },
                    watch.called.json,
                    &env.limits.tools,
                    &crate::root::Parts,
                ),
            }
        }
        Told::Changed { .. } if watch.panel == Some(handle) => {
            let Some(doc) = client.document(handle) else { return };
            if (doc.state == State::Opening || doc.state == State::Behind) && doc.latest.is_none() {
                return;
            }
            match doc.latest {
                Some(Latest::Family(record))
                    if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION =>
                {
                    let limits = forge::wire_limits(&env.limits.domain.forge).expect("checked limits");
                    match watch.task {
                        Some(task) => match wire::decode_landing_panel(&record.bytes, &limits) {
                            Some(panel) if panel.task() == task && Some(panel.project()) == watch.project => {
                                print(&panel, watch.called.json, &env.limits)
                            }
                            _ => render::failure(b"unexpected landing document", watch.called.json, 1),
                        },
                        None => match wire::decode_landings_panel(&record.bytes, &limits) {
                            Some(panel) if Some(panel.project()) == watch.project => {
                                print_list(&panel, watch.called.json, &env.limits)
                            }
                            _ => render::failure(b"unexpected landings document", watch.called.json, 1),
                        },
                    }
                }
                _ => gone(watch, &env.limits),
            }
        }
        Told::Ended { why, .. } if watch.panel == Some(handle) => match why {
            Why::Unknown | Why::SubjectEnded => gone(watch, &env.limits),
            Why::NotAllowed => render::failure(b"not_allowed", watch.called.json, 2),
            Why::SignedOut => render::failure(b"signed out", watch.called.json, 1),
            Why::NotServed => render::failure(b"version not served", watch.called.json, 1),
            Why::Limit => render::failure(b"watch table limit", watch.called.json, 1),
        },
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Changed { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => return,
    };
    complete(state, number, printed, out);
}

fn phase(phase: &w::LandingPhase) -> &'static [u8] {
    match phase {
        w::LandingPhase::Waiting => b"waiting",
        w::LandingPhase::Preparing => b"preparing",
        w::LandingPhase::Proving => b"proving",
        w::LandingPhase::Proved => b"proved",
        w::LandingPhase::Turn => b"in its turn",
        w::LandingPhase::Landed => b"landed",
        w::LandingPhase::Held => b"held",
        w::LandingPhase::Cancelled => b"cancelled",
    }
}
fn hex(bytes: &[u8; 32]) -> [u8; 64] {
    let mut digits = [0; 64];
    for (i, digit) in digits.iter_mut().enumerate() {
        let byte = *bytes.get(i / 2).expect("64 digits in 32 bytes");
        let nibble = if i % 2 == 0 { byte >> 4_u8 } else { byte & 15 };
        *digit = *b"0123456789abcdef".get(usize::from(nibble)).expect("hex nibble");
    }
    digits
}
fn receipt_text(receipt: &w::Receipted, out: &mut Text) {
    out.number(receipt.run());
    out.put(b" \xc2\xb7 ");
    out.put(receipt.address());
}
fn gate_text(gate: &w::GateProof, out: &mut Text) {
    out.put(b"  gate ");
    out.put(gate.name());
    out.put(if gate.blocking() { b", blocking: " } else { b", advisory: " });
    if let Some(key) = gate.key() {
        out.put(b"key ");
        out.put(hex(key).get(..16).expect("key prefix"));
        out.put(b", ");
    }
    match gate.proof() {
        w::Proof::Unkeyed => out.put(b"no key yet"),
        w::Proof::Keyed => out.put(b"keyed"),
        w::Proof::Dispatched(dispatched) => {
            out.put(b"dispatched (try ");
            out.number(u64::from(dispatched.attempt()));
            out.put(b")");
            if let Some(run) = dispatched.run() {
                out.put(b", run ");
                out.number(*run);
            }
        }
        w::Proof::Waiting => out.put(b"waiting on another landing's run"),
        w::Proof::Passed(receipt) => {
            out.put(b"passed by run ");
            receipt_text(receipt, out);
        }
        w::Proof::Failed(receipt) => {
            out.put(b"failed by run ");
            receipt_text(receipt, out);
        }
    }
    if gate.faults() > 0 {
        out.put(b", ");
        out.number(u64::from(gate.faults()));
        out.put(b" runner faults");
    }
    out.put(b"\n");
}
fn text(panel: &w::LandingPanel, out: &mut Text) {
    out.put(b"landing ");
    out.number(panel.task());
    out.put(b" \xc2\xb7 project ");
    out.number(u64::from(panel.project()));
    out.put(b" \xc2\xb7 ");
    out.put(phase(panel.phase()));
    out.put(b"\n");
    for member in panel.members() {
        out.put(b"member: change ");
        out.number(member.change());
        out.put(b" \xc2\xb7 ");
        out.put(member.owner());
        out.put(b"/");
        out.put(member.name());
        out.put(b" on forge ");
        out.number(u64::from(member.forge()));
        out.put(b" \xc2\xb7 ");
        out.put(member.branch());
        out.put(b" into ");
        out.put(member.landing());
        out.put(b"\n");
        if let Some(pull) = member.pull() {
            out.put(b"  pull request #");
            out.number(pull.number());
            out.put(b" \xc2\xb7 ");
            out.put(pull.address());
            out.put(b"\n");
        }
        if let Some(landed) = member.landed() {
            out.put(b"  landed: main at ");
            out.put(&hex(landed));
        } else if let (Some(candidate), Some(base)) = (member.candidate(), member.base()) {
            out.put(b"  candidate ");
            out.put(&hex(candidate));
            out.put(b" on main ");
            out.put(&hex(base));
        } else {
            out.put(b"  candidate: not prepared");
        }
        out.put(b"\n");
        for gate in member.gates() {
            gate_text(gate, out);
        }
    }
    if let Some(w::LandingHold::Faults(faults)) = panel.hold()
        && let Some(member) = panel.members().get(u32::from(faults.member()))
        && let Some(gate) = member.gates().get(u32::from(faults.gate()))
    {
        out.put(b"held: gate ");
        out.put(gate.name());
        out.put(b" of ");
        out.put(member.owner());
        out.put(b"/");
        out.put(member.name());
        out.put(b" failed on its runner past its retries\n");
    }
    match panel.turn() {
        None => out.put(b"turn: free\n"),
        Some(turn) if *turn == panel.task() => out.put(b"turn: this landing's\n"),
        Some(turn) => {
            out.put(b"turn: landing ");
            out.number(*turn);
            out.put(b"'s\n");
        }
    }
    out.put(b"preparations: ");
    out.number(u64::from(panel.preparation()));
    out.put(b" \xc2\xb7 stale proofs: ");
    out.number(u64::from(panel.stale_proofs()));
    out.put(b"\n");
}
fn list_text(panel: &w::LandingsPanel, out: &mut Text) {
    out.put(b"landings of project ");
    out.number(u64::from(panel.project()));
    out.put(b" \xc2\xb7 turn: ");
    match panel.turn() {
        Some(turn) => {
            out.put(b"landing ");
            out.number(*turn);
        }
        None => out.put(b"free"),
    }
    out.put(b"\n");
    if panel.landings().is_empty() {
        out.put(b"no landings\n");
    }
    // The wire is bounded to 64 rows; order without allocating another panel.
    let mut previous = None;
    for _ in 0..panel.landings().len() {
        let mut next: Option<&w::LandingLine> = None;
        for line in panel.landings() {
            let rank = (*panel.turn() == Some(line.task()), line.task());
            if let Some(previous) = previous
                && rank >= previous
            {
                continue;
            }
            if let Some(old) = next
                && rank <= (*panel.turn() == Some(old.task()), old.task())
            {
                continue;
            }
            next = Some(line);
        }
        let Some(line) = next else { break };
        previous = Some((*panel.turn() == Some(line.task()), line.task()));
        out.put(b"landing ");
        out.number(line.task());
        out.put(b" \xc2\xb7 ");
        out.put(phase(line.phase()));
        out.put(b" \xc2\xb7 change ");
        for (i, change) in line.changes().iter().enumerate() {
            if i > 0 {
                out.put(b", ");
            }
            out.number(*change);
        }
        out.put(b" \xc2\xb7 ");
        for (i, repository) in line.repositories().iter().enumerate() {
            if i > 0 {
                out.put(b", ");
            }
            out.put(repository);
        }
        for landed in line.landed() {
            out.put(b" \xc2\xb7 main at ");
            out.put(hex(landed).get(..12).expect("commit prefix"));
        }
        out.put(b"\n");
    }
}
fn optional_number(value: Option<&u64>, out: &mut Encoder) {
    match value {
        Some(value) => out.unsigned(*value),
        None => out.null(),
    }
}
fn optional_hex(value: Option<&[u8; 32]>, out: &mut Encoder) {
    match value {
        Some(value) => out.string(&hex(value)),
        None => out.null(),
    }
}
fn phase_json(value: &w::LandingPhase, out: &mut Encoder) {
    out.string(if *value == w::LandingPhase::Turn { b"turn" } else { phase(value) });
}
fn receipt_json(key: &[u8], value: &w::Receipted, out: &mut Encoder) {
    out.object_start();
    out.key(key);
    out.object_start();
    out.key(b"run");
    out.unsigned(value.run());
    out.key(b"number");
    out.unsigned(value.number());
    out.key(b"address");
    out.string(value.address());
    out.object_end();
    out.object_end();
}
fn proof_json(proof: &w::Proof, out: &mut Encoder) {
    match proof {
        w::Proof::Unkeyed => out.string(b"unkeyed"),
        w::Proof::Keyed => out.string(b"keyed"),
        w::Proof::Waiting => out.string(b"waiting"),
        w::Proof::Dispatched(value) => {
            out.object_start();
            out.key(b"dispatched");
            out.object_start();
            out.key(b"attempt");
            out.unsigned(u64::from(value.attempt()));
            out.key(b"run");
            optional_number(value.run().as_ref(), out);
            out.object_end();
            out.object_end();
        }
        w::Proof::Passed(value) => receipt_json(b"passed", value, out),
        w::Proof::Failed(value) => receipt_json(b"failed", value, out),
    }
}
fn encode(panel: &w::LandingPanel, out: &mut Encoder) {
    out.object_start();
    out.key(b"task");
    out.unsigned(panel.task());
    out.key(b"project");
    out.unsigned(u64::from(panel.project()));
    out.key(b"phase");
    phase_json(panel.phase(), out);
    out.key(b"hold");
    match panel.hold() {
        Some(w::LandingHold::Faults(value)) => {
            out.object_start();
            out.key(b"faults");
            out.object_start();
            out.key(b"member");
            out.unsigned(u64::from(value.member()));
            out.key(b"gate");
            out.unsigned(u64::from(value.gate()));
            out.object_end();
            out.object_end();
        }
        None => out.null(),
    }
    out.key(b"members");
    out.array_start();
    for member in panel.members() {
        out.object_start();
        out.key(b"change");
        out.unsigned(member.change());
        out.key(b"forge");
        out.unsigned(u64::from(member.forge()));
        for (key, value) in [
            (b"owner".as_slice(), member.owner()),
            (b"name", member.name()),
            (b"branch", member.branch()),
            (b"landing", member.landing()),
        ] {
            out.key(key);
            out.string(value);
        }
        out.key(b"pull");
        match member.pull() {
            Some(pull) => {
                out.object_start();
                out.key(b"number");
                out.unsigned(pull.number());
                out.key(b"address");
                out.string(pull.address());
                out.object_end();
            }
            None => out.null(),
        }
        for (key, value) in
            [(b"base".as_slice(), member.base()), (b"candidate", member.candidate()), (b"landed", member.landed())]
        {
            out.key(key);
            optional_hex(value.as_ref(), out);
        }
        out.key(b"gates");
        out.array_start();
        for gate in member.gates() {
            out.object_start();
            out.key(b"name");
            out.string(gate.name());
            out.key(b"blocking");
            out.boolean(gate.blocking());
            out.key(b"key");
            optional_hex(gate.key().as_ref(), out);
            out.key(b"proof");
            proof_json(gate.proof(), out);
            out.key(b"faults");
            out.unsigned(u64::from(gate.faults()));
            out.object_end();
        }
        out.array_end();
        out.object_end();
    }
    out.array_end();
    out.key(b"preparation");
    out.unsigned(u64::from(panel.preparation()));
    out.key(b"stale_proofs");
    out.unsigned(u64::from(panel.stale_proofs()));
    out.key(b"turn");
    optional_number(panel.turn().as_ref(), out);
    out.object_end();
}
fn encode_list(panel: &w::LandingsPanel, out: &mut Encoder) {
    out.object_start();
    out.key(b"project");
    out.unsigned(u64::from(panel.project()));
    out.key(b"turn");
    optional_number(panel.turn().as_ref(), out);
    out.key(b"landings");
    out.array_start();
    for line in panel.landings() {
        out.object_start();
        out.key(b"task");
        out.unsigned(line.task());
        out.key(b"phase");
        phase_json(line.phase(), out);
        out.key(b"changes");
        out.array_start();
        for change in line.changes() {
            out.unsigned(*change);
        }
        out.array_end();
        out.key(b"repositories");
        out.array_start();
        for repository in line.repositories() {
            out.string(repository);
        }
        out.array_end();
        out.key(b"landed");
        out.array_start();
        for landed in line.landed() {
            out.string(&hex(landed));
        }
        out.array_end();
        out.object_end();
    }
    out.array_end();
    out.object_end();
}
fn render_panel(
    one: Option<&w::LandingPanel>,
    list: Option<&w::LandingsPanel>,
    json: bool,
    limits: &Limits,
) -> jig::Printed {
    let bytes = if json {
        let bounds = skein_json::writer::Limits { depth: 10, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        match (one, list) {
            (Some(panel), None) => encode(panel, &mut measure),
            (None, Some(panel)) => encode_list(panel, &mut measure),
            _ => unreachable!("one panel"),
        }
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", json, 1) };
        let mut out = Encoder::write(size, &bounds);
        match (one, list) {
            (Some(panel), None) => encode(panel, &mut out),
            (None, Some(panel)) => encode_list(panel, &mut out),
            _ => unreachable!("one panel"),
        }
        out.finish()
    } else {
        let mut measure = Text::measure();
        match (one, list) {
            (Some(panel), None) => text(panel, &mut measure),
            (None, Some(panel)) => list_text(panel, &mut measure),
            _ => unreachable!("one panel"),
        }
        let mut out = Text::writing_exact(measure);
        match (one, list) {
            (Some(panel), None) => text(panel, &mut out),
            (None, Some(panel)) => list_text(panel, &mut out),
            _ => unreachable!("one panel"),
        }
        out.finish()
    };
    if bytes.len() > usize::try_from(limits.tools.text).expect("text fits") {
        return render::failure(b"result limit", json, 1);
    }
    jig::Printed { file: jig::FileEnd::Keep, status: 0, stdout: bytes, stderr: Box::default() }
}
pub(crate) fn print(panel: &w::LandingPanel, json: bool, limits: &Limits) -> jig::Printed {
    render_panel(Some(panel), None, json, limits)
}
pub(crate) fn print_list(panel: &w::LandingsPanel, json: bool, limits: &Limits) -> jig::Printed {
    render_panel(None, Some(panel), json, limits)
}
