//! A change's way to landing, using the browser's panel (client/tools.md, 5.4).
use crate::{
    jig::{
        self,
        call::{Value, number, value},
        digest::Digest,
        render::{self, Text},
        runner::{Called, Out},
        table::{
            Keyed, Kind,
            Occurs::{Optional, Required},
            Tool, flag, positional,
        },
    },
    root::Limits,
};
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{Client, Event, Handle, Latest, Readable, Readout, State, Told, Watchable, Why, contract as c};
use skein_lib::{Env, Queue, Time};
use temper_client_domain::{
    forge::{self, change as words},
    owner::Owner,
};
use temper_client_protocol_wire::{self as wire, v1 as w};

pub const READ: Tool = Tool {
    name: b"change show",
    line: b"Read a change's way to landing: its pull request, CI and step",
    prints: b"`change N`, its repository, branch and landing branch, its pull request, step and CI at its head, and a digest",
    result: b"change",
    forms: &[&[
        positional(b"task", b"TASK", Kind::NumberRange { min: 1, max: u64::MAX }, Required, b"The change task's number"),
        flag(b"project", b"PROJECT", Kind::Project { empty: false }, Optional, b"Its project; the task's when absent"),
        flag(b"since", b"DIGEST", Kind::Digest, Optional, b"Wait until it differs from the read that printed DIGEST"),
        flag(b"until", b"CONDITION", Kind::Word { set: &[b"pull", b"ci:pending", b"ci:passed", b"ci:failed", b"held", b"landed", b"changed"] }, Optional, b"Wait until it holds: pull, ci:pending, ci:passed, ci:failed, held, landed, or changed"),
        flag(b"timeout", b"DURATION", Kind::Duration, Optional, b"How long a wait lasts; 300 seconds when absent, at most 3600"),
    ]],
    keyed: Keyed::Never,
    area: 6,
    notes: b"Until it lands, it prints `next: temper ctl DIR change show TASK\n--until landed`; landed, its merge commit. A task with no change\nfails, `no change panel for task N` (exit 2). The forge holds its\ndiff, reviews and logs: read them there.\n",
};
#[derive(Debug)]
pub(crate) struct Changes {
    calls: BTreeMap<u32, Watching>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelState {
    Expected,
    Missing,
}
#[derive(Debug)]
struct Watching {
    called: Called,
    task: u64,
    project: Option<u32>,
    waiting: bool,
    since: Option<Digest>,
    first: Option<Digest>,
    deadline: Option<Time>,
    panel: Option<Handle>,
    read: Option<Handle>,
    task_watch: Option<Handle>,
    activity: Option<u64>,
    final_panel: bool,
    ended: Option<&'static [u8]>,
    panel_state: PanelState,
    run_read: Option<u64>,
    run_watch: Option<(Handle, u64)>,
    run_hold: Option<Box<[u8]>>,
    last: Option<Box<w::ChangePanel>>,
    serial: u32,
    started: bool,
}
impl Changes {
    pub(crate) fn new() -> Self {
        Self { calls: BTreeMap::new() }
    }
    pub(crate) fn calls(&self) -> usize {
        self.calls.len()
    }
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for call in self.calls.values() {
            if let Some(deadline) = call.deadline {
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
}
pub(crate) fn call(state: &mut Changes, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    if called.value.is_some() || jig::argv::check(crate::table::TOOLS, &called.call, &env.limits.tools).is_err() {
        let printed = render::failure(b"invalid change call", called.json, 1);
        out.push(Out::Printed { number: called.number, printed });
        return;
    }
    let waiting = jig::call::waiting(&READ, &called.call);
    let task = number(&called.call, 0).expect("checked task");
    let since = match value(&called.call, 2) {
        Some(Value::Digest(bytes)) if waiting => Some(Digest::from_bytes(*bytes)),
        _ => None,
    };
    let deadline = match value(&called.call, 4) {
        Some(Value::Duration(duration)) if waiting => {
            Some(env.now.checked_add(*duration).unwrap_or(Time::from_nanos(u64::MAX)))
        }
        _ => None,
    };
    state.calls.insert(
        called.call_number,
        Watching {
            called,
            task,
            project: None,
            waiting,
            since,
            first: None,
            deadline,
            panel: None,
            read: None,
            task_watch: None,
            activity: None,
            final_panel: false,
            ended: None,
            panel_state: PanelState::Expected,
            run_read: None,
            run_watch: None,
            run_hold: None,
            last: None,
            serial: 16,
            started: false,
        },
    );
    ready(state, client, env, out);
}
fn handle(watch: &mut Watching) -> Handle {
    watch.serial = watch.serial.checked_add(1).expect("bounded wait life");
    Handle {
        asker: Owner::Forge.asker(),
        number: (u64::from(watch.called.call_number) << 32u32) | u64::from(watch.serial),
    }
}
pub(crate) fn ready(state: &mut Changes, client: &Client, env: &Env<Limits>, out: &mut Queue<Out>) {
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
        let watch = state.calls.get_mut(&number).expect("active change");
        watch.started = true;
        let project = value(&watch.called.call, 1);
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
            let json = watch.called.json;
            complete(state, number, render::failure(b"unknown project", json, 1), out);
            continue;
        }
        if watch.project.is_some() {
            open_panel(watch, env, out);
        } else {
            read_task(watch, out);
        }
    }
}

fn open_panel(watch: &mut Watching, env: &Env<Limits>, out: &mut Queue<Out>) {
    if watch.waiting && watch.task_watch.is_none() {
        watch_task(watch, out);
    }
    let handle = handle(watch);
    watch.panel = Some(handle);
    out.push(Out::Client {
        event: Event::Watch { handle, subject: subject(watch.task, watch.project.expect("task project"), &env.limits) },
    });
}
pub(crate) fn subject(task: u64, project: u32, limits: &Limits) -> Watchable {
    let limits = forge::wire_limits(&limits.domain.forge).expect("checked forge limits");
    let change = w::ChangeSubject::new(&limits, w::ChangeSubjectParts { task }).expect("task fits");
    let record = w::Watching::new(&limits, w::WatchingParts { watch: w::Watch::Change(change) }).expect("watch fits");
    Watchable::Family {
        scope: c::Scope::Project(project),
        window: c::Window { rows: 0, after: None },
        record: jig_client::Family {
            family: wire::FORGE,
            version: wire::CLIENT_VERSION,
            bytes: wire::watching(&record),
        },
    }
}
fn read_task(watch: &mut Watching, out: &mut Queue<Out>) {
    watch.run_read = None;
    let handle = handle(watch);
    watch.read = Some(handle);
    out.push(Out::Client { event: Event::Read { handle, read: Readable::Jig(c::Read::Task { task: watch.task }) } });
}
fn read_run(watch: &mut Watching, run: u64, out: &mut Queue<Out>) {
    if watch.read.is_some() {
        return;
    }
    let handle = handle(watch);
    watch.read = Some(handle);
    watch.run_read = Some(run);
    out.push(Out::Client { event: Event::Read { handle, read: Readable::Jig(c::Read::Task { task: run }) } });
}
fn close(watch: &Watching, out: &mut Queue<Out>) {
    for handle in [watch.panel, watch.task_watch, run_handle(watch)].into_iter().flatten() {
        out.push(Out::Client { event: Event::Unwatch { handle } });
    }
}
fn complete(state: &mut Changes, number: u32, printed: jig::Printed, out: &mut Queue<Out>) {
    let watch = state.calls.remove(&number).expect("active change");
    close(&watch, out);
    out.push(Out::Printed { number: watch.called.number, printed });
}
fn missing(task: u64, json: bool) -> jig::Printed {
    let mut measure = Text::measure();
    measure.put(b"no change panel for task ");
    measure.number(task);
    let mut out = Text::writing_exact(measure);
    out.put(b"no change panel for task ");
    out.number(task);
    render::failure(&out.finish(), json, 2)
}
fn panel_ended(watch: &mut Watching, out: &mut Queue<Out>) {
    watch.panel_state = PanelState::Missing;
    if let Some(handle) = watch.panel.take() {
        out.push(Out::Client { event: Event::Unwatch { handle } });
    }
    read_task(watch, out);
}
fn has_change(task: &c::Task, result: Option<&c::Result>) -> bool {
    let result = match result {
        Some(result) => Some(result),
        None => match &task.result {
            Some(c::TaskResult::Document(result)) => Some(result.as_ref()),
            _ => None,
        },
    };
    let Some(result) = result else { return false };
    let value = match &result.ending {
        c::Ending::Task(ending) => match ending.as_ref() {
            c::OwnerChipEnding::Done(value) => Some(value),
            c::OwnerChipEnding::Failed(_) | c::OwnerChipEnding::Cancelled { .. } => None,
        },
        c::Ending::Done => result.value.as_ref(),
        c::Ending::Failed { .. }
        | c::Ending::Cancelled { .. }
        | c::Ending::Withdrawn { .. }
        | c::Ending::Decided { .. }
        | c::Ending::Deleted => None,
    };
    let Some(c::ResultValue::Change { part, .. }) = value else { return false };
    part.connector == wire::FORGE
}
fn terminal(task: &c::Task) -> bool {
    match task.chip.phase {
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => true,
        c::Phase::Waiting
        | c::Phase::Idle
        | c::Phase::Due
        | c::Phase::Preparing
        | c::Phase::Claimed
        | c::Phase::Running
        | c::Phase::Active
        | c::Phase::BackingOff
        | c::Phase::Closing
        | c::Phase::Held => false,
    }
}
fn task_read(
    watch: &mut Watching,
    page: &Readout,
    client: &Client,
    env: &Env<Limits>,
    out: &mut Queue<Out>,
) -> Option<jig::Printed> {
    let Readout::Jig(c::ReadResult::Value(value)) = page else {
        return Some(render::read(
            page,
            client,
            Handle { asker: crate::root::RUNNER.asker(), number: 0 },
            watch.called.json,
            &env.limits.tools,
            &crate::root::Parts,
        ));
    };
    let Some((task, result)) = render::read_task(value) else {
        return Some(render::failure(b"unexpected task document", watch.called.json, 1));
    };
    if task.chip.task != watch.task {
        return Some(render::failure(b"unexpected task document", watch.called.json, 1));
    }
    if watch.panel_state == PanelState::Missing {
        if task.chip.phase == c::Phase::Held
            && let c::TaskContractDocument::Change { connector: wire::FORGE, kind: 1, .. } = task.result_contract
        {
            let reason = render::holds::summary(task, &crate::root::Parts);
            let mut measure = Text::measure();
            measure.put(b"held before registration: ");
            measure.put(&reason);
            let mut out = Text::writing_exact(measure);
            out.put(b"held before registration: ");
            out.put(&reason);
            return Some(render::failure(&out.finish(), watch.called.json, 2));
        }
        if !watch.waiting {
            return Some(missing(watch.task, watch.called.json));
        }
    }
    // The first task read supplies the project; a read remains a read once.
    if watch.project.is_none() {
        watch.project = Some(task.project);
        open_panel(watch, env, out);
    }
    if watch.waiting && terminal(task) {
        if !has_change(task, result) {
            let why = ending(task, result);
            watch.ended = Some(why);
            if watch.last.is_some() {
                return Some(print(
                    watch.last.as_deref(),
                    watch.called.json,
                    Some(false),
                    why,
                    &env.limits,
                    watch.run_hold.as_deref(),
                ));
            }
            if watch.panel.is_none() {
                return Some(missing(watch.task, watch.called.json));
            }
        } else if watch.final_panel {
            return Some(match watch.last.as_deref() {
                Some(panel) => {
                    print(Some(panel), watch.called.json, Some(false), b"ended", &env.limits, watch.run_hold.as_deref())
                }
                None => missing(watch.task, watch.called.json),
            });
        } else {
            watch.final_panel = true;
            if watch.panel.is_none() {
                open_panel(watch, env, out);
            }
        }
    } else if watch.waiting {
        watch.activity = Some(task.activity);
        if watch.task_watch.is_none() {
            watch_task(watch, out);
        }
    }
    None
}
fn watch_task(watch: &mut Watching, out: &mut Queue<Out>) {
    let handle = handle(watch);
    watch.task_watch = Some(handle);
    out.push(Out::Client {
        event: Event::Watch { handle, subject: Watchable::Jig(c::Subject::Task { task: watch.task }) },
    });
}
fn ending(task: &c::Task, result: Option<&c::Result>) -> &'static [u8] {
    let result = result.or(match &task.result {
        Some(c::TaskResult::Document(result)) => Some(result.as_ref()),
        _ => None,
    });
    let phase = if task.chip.phase == c::Phase::Cancelled {
        b"ended: cancelled".as_slice()
    } else if task.chip.phase == c::Phase::Failed {
        b"ended: failed".as_slice()
    } else {
        b"ended".as_slice()
    };
    let Some(result) = result else { return phase };
    match &result.ending {
        c::Ending::Cancelled { .. } => b"ended: cancelled",
        c::Ending::Failed { .. } => b"ended: failed",
        c::Ending::Task(ending) => match ending.as_ref() {
            c::OwnerChipEnding::Cancelled { .. } => b"ended: cancelled",
            c::OwnerChipEnding::Failed(_) => b"ended: failed",
            c::OwnerChipEnding::Done(_) => b"ended",
        },
        c::Ending::Done | c::Ending::Withdrawn { .. } | c::Ending::Decided { .. } | c::Ending::Deleted => phase,
    }
}

fn read_page(
    watch: &mut Watching,
    page: &Readout,
    client: &Client,
    env: &Env<Limits>,
    out: &mut Queue<Out>,
) -> Option<jig::Printed> {
    let Some(run) = watch.run_read.take() else { return task_read(watch, page, client, env, out) };
    let Readout::Jig(c::ReadResult::Value(value)) = page else {
        return Some(render::failure(b"could not read the change's task", watch.called.json, 2));
    };
    let Some((task, _)) = render::read_task(value) else {
        return Some(render::failure(b"unexpected task document", watch.called.json, 1));
    };
    if task.chip.task != run {
        return Some(render::failure(b"unexpected task document", watch.called.json, 1));
    }
    let latest = match &watch.last {
        Some(panel) => *panel.runs(),
        None => None,
    };
    if let Some(latest) = latest
        && latest != run
    {
        read_run(watch, latest, out);
        return None;
    }
    watch.run_hold = if latest.is_some() && task.chip.phase == c::Phase::Held {
        Some(render::holds::summary(task, &crate::root::Parts))
    } else {
        None
    };
    if watch.waiting && latest.is_some() && !terminal(task) && watch.run_watch.is_none() {
        let handle = handle(watch);
        watch.run_watch = Some((handle, run));
        out.push(Out::Client {
            event: Event::Watch { handle, subject: Watchable::Jig(c::Subject::Task { task: run }) },
        });
    }
    delivered(watch, env)
}
fn panel_changed(
    watch: &mut Watching,
    client: &Client,
    env: &Env<Limits>,
    handle: Handle,
    out: &mut Queue<Out>,
) -> Option<jig::Printed> {
    let doc = client.document(handle)?;
    if (doc.state == State::Opening || doc.state == State::Behind) && doc.latest.is_none() {
        return None;
    }
    let panel = match doc.latest {
        Some(Latest::Family(record)) if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION => {
            wire::decode_change_panel(
                &record.bytes,
                &forge::wire_limits(&env.limits.domain.forge).expect("checked limits"),
            )
        }
        _ => None,
    };
    match panel {
        Some(panel) if panel.task() == watch.task => {
            watch.last = Some(Box::new(panel));
            watch.panel_state = PanelState::Expected;
            watch.run_hold = None;
            if let Some((_, task)) = watch.run_watch
                && Some(task) != *watch.last.as_ref().expect("delivered panel").runs()
            {
                let (handle, _) = watch.run_watch.take().expect("old run watch");
                out.push(Out::Client { event: Event::Unwatch { handle } });
            }
            if let Some(run) = watch.last.as_ref().expect("delivered panel").runs() {
                let run = *run;
                read_run(watch, run, out);
            } else {
                return delivered(watch, env);
            }
        }
        _ => panel_ended(watch, out),
    }
    None
}
fn run_handle(watch: &Watching) -> Option<Handle> {
    let (handle, _) = watch.run_watch?;
    Some(handle)
}
fn watch_failure(why: Why, json: bool) -> jig::Printed {
    let (reason, status): (&[u8], u8) = match why {
        Why::NotAllowed => (b"not_allowed", 2),
        Why::SignedOut => (b"signed out", 1),
        Why::NotServed => (b"version not served", 1),
        Why::Limit => (b"watch table limit", 1),
        Why::Unknown | Why::SubjectEnded => unreachable!("handled"),
    };
    render::failure(reason, json, status)
}
pub(crate) fn told(state: &mut Changes, client: &Client, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) {
    let handle = match told {
        Told::Changed { handle } | Told::Ended { handle, .. } | Told::Page { handle, .. } => *handle,
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Text { .. } => return,
    };
    let number = u32::try_from(handle.number >> 32u32).expect("high half");
    let Some(watch) = state.calls.get_mut(&number) else { return };
    let mut printed = None;
    match told {
        Told::Page { page, .. } if watch.read == Some(handle) => {
            watch.read = None;
            printed = read_page(watch, page, client, env, out);
        }
        Told::Changed { .. } if watch.panel == Some(handle) => {
            printed = panel_changed(watch, client, env, handle, out);
        }
        Told::Changed { .. } if run_handle(watch) == Some(handle) => {
            let Some(doc) = client.document(handle) else { return };
            if let Some(Latest::Jig(c::Delivery::Task(task))) = doc.latest {
                let (_, run) = watch.run_watch.expect("run watch");
                if task.chip.task == run {
                    watch.run_hold = if task.chip.phase == c::Phase::Held {
                        Some(render::holds::summary(task, &crate::root::Parts))
                    } else {
                        None
                    };
                    printed = delivered(watch, env);
                } else {
                    printed = Some(render::failure(b"unexpected task document", watch.called.json, 1));
                }
            }
        }
        Told::Ended { why, .. } if run_handle(watch) == Some(handle) => {
            let (_, run) = watch.run_watch.take().expect("run watch");
            if *why == Why::Unknown || *why == Why::SubjectEnded {
                read_run(watch, run, out);
            } else {
                printed = Some(watch_failure(*why, watch.called.json));
            }
        }
        Told::Changed { .. } if watch.task_watch == Some(handle) => {
            let Some(doc) = client.document(handle) else { return };
            if let Some(Latest::Jig(c::Delivery::Task(task))) = doc.latest {
                if task.chip.task != watch.task {
                    printed = Some(render::failure(b"unexpected task document", watch.called.json, 1));
                } else if terminal(task) {
                    if let Some(handle) = watch.task_watch.take() {
                        out.push(Out::Client { event: Event::Unwatch { handle } });
                    }
                    read_task(watch, out);
                } else if watch.panel.is_none() && watch.activity != Some(task.activity) {
                    watch.activity = Some(task.activity);
                    if let Some(handle) = watch.task_watch.take() {
                        out.push(Out::Client { event: Event::Unwatch { handle } });
                    }
                    open_panel(watch, env, out);
                }
            }
        }
        Told::Ended { why, .. } if watch.panel == Some(handle) || watch.task_watch == Some(handle) => {
            if *why == Why::Unknown || *why == Why::SubjectEnded {
                if watch.panel == Some(handle) {
                    panel_ended(watch, out);
                } else {
                    watch.task_watch = None;
                    read_task(watch, out);
                }
            } else {
                printed = Some(watch_failure(*why, watch.called.json));
            }
        }
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Changed { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => {}
    }
    if let Some(printed) = printed {
        complete(state, number, printed, out);
    }
}
fn delivered(watch: &mut Watching, env: &Env<Limits>) -> Option<jig::Printed> {
    let panel = watch.last.as_deref().expect("delivered panel");
    if !watch.waiting {
        return Some(print(Some(panel), watch.called.json, None, b"", &env.limits, watch.run_hold.as_deref()));
    }
    let Some(digest) = digest(panel, &env.limits) else {
        return Some(render::failure(b"result limit", watch.called.json, 1));
    };
    let condition = match value(&watch.called.call, 3) {
        Some(Value::Word(word)) => {
            let Kind::Word { set } = READ.forms.first().expect("read form").get(3).expect("condition argument").kind
            else {
                unreachable!("condition words")
            };
            Some(*set.get(usize::from(*word)).expect("checked condition"))
        }
        _ => None,
    };
    let holds = match condition {
        Some(b"changed") => Some(watch.first.is_some() && jig::wait::met(watch.since, watch.first, digest, None)),
        Some(b"pull") => Some(panel.pull().is_some()),
        Some(b"ci:pending") => Some(*panel.ci() == w::Ci::Pending),
        Some(b"ci:passed") => Some(*panel.ci() == w::Ci::Passed),
        Some(b"ci:failed") => Some(*panel.ci() == w::Ci::Failed),
        Some(b"held") => Some(panel.hold().is_some() || watch.run_hold.is_some()),
        Some(b"landed") => Some(landed(panel.step()).is_some()),
        None => None,
        _ => unreachable!("checked condition"),
    };
    if let Some(why) = watch.ended {
        return Some(print(Some(panel), watch.called.json, Some(false), why, &env.limits, watch.run_hold.as_deref()));
    }
    let met = jig::wait::met(watch.since, watch.first, digest, holds);
    watch.first = Some(watch.first.unwrap_or(digest));
    if met || watch.final_panel || landed(panel.step()).is_some() {
        Some(print(Some(panel), watch.called.json, Some(met), b"ended", &env.limits, watch.run_hold.as_deref()))
    } else {
        None
    }
}
pub(crate) fn fire(state: &mut Changes, env: &Env<Limits>, out: &mut Queue<Out>) {
    for _ in 0..state.calls.len() {
        let mut due = None;
        for (&number, watch) in &state.calls {
            if let Some(deadline) = watch.deadline
                && deadline <= env.now
            {
                due = Some(number);
                break;
            }
        }
        let Some(number) = due else { break };
        let watch = state.calls.get(&number).expect("active change");
        let printed = print(
            watch.last.as_deref(),
            watch.called.json,
            Some(false),
            b"timeout",
            &env.limits,
            watch.run_hold.as_deref(),
        );
        complete(state, number, printed, out);
    }
}

use skein_json::writer::Encoder;
fn landed(step: &w::Step) -> Option<&w::Landed> {
    match step {
        w::Step::Landed(landed) => Some(landed),
        w::Step::Ready(_)
        | w::Step::Preparing(_)
        | w::Step::Producing
        | w::Step::Opening
        | w::Step::Recreating
        | w::Step::Reopening
        | w::Step::Checking
        | w::Step::Gating
        | w::Step::Queued(_)
        | w::Step::First
        | w::Step::Updating
        | w::Step::Resolving
        | w::Step::Repairing(_)
        | w::Step::Landing
        | w::Step::Ended => None,
    }
}
fn step(step: &w::Step) -> &'static [u8] {
    // The browser's held-step vocabulary is also the step's lower-case name.
    match step {
        w::Step::First => b"first",
        w::Step::Ended => b"ended",
        w::Step::Ready(_)
        | w::Step::Preparing(_)
        | w::Step::Producing
        | w::Step::Opening
        | w::Step::Recreating
        | w::Step::Reopening
        | w::Step::Checking
        | w::Step::Gating
        | w::Step::Queued(_)
        | w::Step::Updating
        | w::Step::Resolving
        | w::Step::Repairing(_)
        | w::Step::Landing
        | w::Step::Landed(_) => phrase(words::held_step(step)),
    }
}
fn phrase(word: forge::words::ChangeWord) -> &'static [u8] {
    for (candidate, literals, _) in &forge::words::CHANGES {
        if *candidate == word {
            return literals.first().expect("phrase literal");
        }
    }
    unreachable!("closed change vocabulary")
}
fn ci(ci: &w::Ci) -> &'static [u8] {
    match ci {
        w::Ci::Unknown => b"unknown",
        w::Ci::Pending => b"pending",
        w::Ci::Passed => b"passed",
        w::Ci::Failed => b"failed",
    }
}
fn hold(hold: &w::Hold) -> &[u8] {
    match hold {
        w::Hold::Stalled => b"stalled",
        w::Hold::Repairs => b"repairs",
        w::Hold::Resolutions => b"resolutions",
        w::Hold::Updates => b"updates",
        w::Hold::Moved => b"moved",
        w::Hold::Missing => b"missing",
        w::Hold::Closed => b"closed",
        w::Hold::Retargeted => b"retargeted",
        w::Hold::Failed => b"failed",
        w::Hold::Rejected => b"rejected",
        w::Hold::Authority(value) => value.reason(),
    }
}
fn repair(step: &w::Step) -> Option<&'static [u8]> {
    match step {
        w::Step::Repairing(repair) => Some(match repair.reason() {
            w::Reason::Ci => b"ci",
            w::Reason::Gate(_) => b"gate",
            w::Reason::Semantic => b"semantic",
        }),
        w::Step::Ready(_)
        | w::Step::Preparing(_)
        | w::Step::Producing
        | w::Step::Opening
        | w::Step::Recreating
        | w::Step::Reopening
        | w::Step::Checking
        | w::Step::Gating
        | w::Step::Queued(_)
        | w::Step::First
        | w::Step::Updating
        | w::Step::Resolving
        | w::Step::Landing
        | w::Step::Landed(_)
        | w::Step::Ended => None,
    }
}
fn gate_kind(kind: &w::GateKind) -> &'static [u8] {
    match kind {
        w::GateKind::Review => b"review",
        w::GateKind::Approval => b"approval",
        w::GateKind::Check => b"check",
    }
}
fn verdict(verdict: &w::Verdict) -> &'static [u8] {
    match verdict {
        w::Verdict::None => b"none",
        w::Verdict::Pending => b"pending",
        w::Verdict::Approved => b"approved",
        w::Verdict::Changes => b"changes",
    }
}
fn hex(bytes: &[u8; 32]) -> [u8; 40] {
    let mut digits = [0; 40];
    for (i, digit) in digits.iter_mut().enumerate() {
        let byte = *bytes.get(i / 2).expect("forty digits in 32 bytes");
        let nibble = if i % 2 == 0 { byte >> 4_u8 } else { byte & 15 };
        *digit = *b"0123456789abcdef".get(usize::from(nibble)).expect("hex nibble");
    }
    digits
}
fn used_json(used: &w::Used, out: &mut Encoder) {
    out.object_start();
    out.key(b"used");
    out.unsigned(u64::from(used.used()));
    out.key(b"limit");
    out.unsigned(u64::from(used.limit()));
    out.object_end();
}
fn encode(panel: &w::ChangePanel, digest: Option<Digest>, out: &mut Encoder) {
    out.object_start();
    out.key(b"task");
    out.unsigned(panel.task());
    out.key(b"forge");
    out.unsigned(u64::from(panel.forge()));
    for (key, value) in [
        (b"owner".as_slice(), panel.owner()),
        (b"name", panel.name()),
        (b"branch", panel.branch()),
        (b"landing", panel.landing()),
    ] {
        out.key(key);
        out.string(value);
    }
    out.key(b"head");
    match panel.head() {
        Some(head) => out.string(&hex(head)),
        None => out.null(),
    }
    out.key(b"pull");
    match panel.pull() {
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
    out.key(b"step");
    out.string(step(panel.step()));
    out.key(b"merge");
    match landed(panel.step()) {
        Some(landed) => out.string(&hex(landed.merge())),
        None => out.null(),
    }
    out.key(b"queue");
    match words::queue(panel.step()) {
        Some(queue) => {
            out.object_start();
            out.key(b"place");
            out.unsigned(u64::from(queue.place));
            out.key(b"ahead");
            out.array_start();
            for task in queue.ahead {
                out.unsigned(*task);
            }
            out.array_end();
            out.object_end();
        }
        None => out.null(),
    }
    out.key(b"repair");
    match repair(panel.step()) {
        Some(reason) => out.string(reason),
        None => out.null(),
    }
    out.key(b"hold");
    match panel.hold() {
        Some(why) => out.string(hold(why)),
        None => out.null(),
    }
    out.key(b"runs");
    match panel.runs() {
        Some(task) => out.unsigned(*task),
        None => out.null(),
    }
    out.key(b"ci");
    out.string(ci(panel.ci()));
    out.key(b"gates");
    out.array_start();
    for gate in panel.gates() {
        out.object_start();
        out.key(b"number");
        out.unsigned(gate.number());
        out.key(b"kind");
        out.string(gate_kind(gate.kind()));
        out.key(b"blocks");
        out.boolean(gate.blocks());
        out.key(b"verdict");
        out.string(verdict(gate.verdict()));
        out.object_end();
    }
    out.array_end();
    for (key, used) in
        [(b"repairs".as_slice(), panel.repairs()), (b"resolutions", panel.resolutions()), (b"updates", panel.updates())]
    {
        out.key(key);
        used_json(used, out);
    }
    if let Some(digest) = digest {
        out.key(b"digest");
        out.string(&digest.hex());
    }
    out.object_end();
}
fn json(panel: Option<&w::ChangePanel>, digest: Option<Digest>, met: Option<bool>, why: &[u8], out: &mut Encoder) {
    if let Some(met) = met {
        out.object_start();
        out.key(b"change");
        match panel {
            Some(panel) => encode(panel, digest, out),
            None => out.null(),
        }
        out.key(b"met");
        out.boolean(met);
        if !met {
            out.key(b"why");
            out.string(why);
        }
        out.object_end();
    } else {
        encode(panel.expect("read panel"), digest, out);
    }
}
fn text(
    panel: Option<&w::ChangePanel>,
    digest: Option<Digest>,
    met: Option<bool>,
    why: &[u8],
    run_hold: Option<&[u8]>,
    out: &mut Text,
) {
    if let Some(panel) = panel {
        out.put(b"change ");
        out.number(panel.task());
        out.put(b" \xc2\xb7 ");
        out.put(panel.owner());
        out.put(b"/");
        out.put(panel.name());
        out.put(b" on forge ");
        out.number(u64::from(panel.forge()));
        out.put(b" \xc2\xb7 ");
        out.put(panel.branch());
        out.put(b" into ");
        out.put(panel.landing());
        out.put(b"\n");
        if let Some(pull) = panel.pull() {
            out.put(b"pull request #");
            out.number(pull.number());
            if !pull.address().is_empty() {
                out.put(b" \xc2\xb7 ");
                out.put(pull.address());
            }
            out.put(b"\n");
        }
        out.put(b"step: ");
        out.put(step(panel.step()));
        if let w::Step::Landed(landed) = panel.step() {
            out.put(b" \xc2\xb7 merge commit ");
            out.put(&hex(landed.merge()));
        }
        out.put(b"\n");
        text_way(panel, out);
        if let Some(hold) = panel.hold() {
            out.put(b"held: ");
            if let w::Hold::Authority(value) = hold {
                out.put(value.reason());
            } else {
                out.put(phrase(words::hold(hold)));
            }
            out.put(b" at ");
            out.put(step(panel.step()));
            out.put(b"\n");
        }
        if let Some(task) = panel.runs() {
            out.put(if run_hold.is_some() { b"held: task " } else { b"running: task " });
            out.number(*task);
            if let Some(reason) = run_hold {
                out.put(b" (");
                out.put(reason);
                out.put(b")");
            }
            out.put(b"\n");
        }
        if let Some(queue) = words::queue(panel.step()) {
            out.put(b"queued: place ");
            out.number(u64::from(queue.place));
            for (i, task) in queue.ahead.iter().enumerate() {
                out.put(if i == 0 { b", behind task " } else { b", task " });
                out.number(*task);
            }
            out.put(b"\n");
        }
        if let w::Step::Repairing(_) = panel.step() {
            let (word, holes) = words::line(panel);
            out.put(b"repair: ");
            for (candidate, literals, _) in &forge::words::CHANGES {
                if *candidate == word {
                    crate::forge::literals_text(literals, &holes, out);
                    break;
                }
            }
            out.put(b"\n");
        }
        text_checks(panel, out);
        if let Some(digest) = digest {
            out.put(b"digest: ");
            out.put(&digest.hex());
            out.put(b"\n");
        }
        if landed(panel.step()).is_none() {
            out.put(b"next: temper ctl DIR change show ");
            out.number(panel.task());
            out.put(b" --until landed\n");
        }
    }
    match met {
        Some(true) => out.put(b"met\n"),
        Some(false) => {
            out.put(b"not met: ");
            out.put(why);
            out.put(b"\n");
        }
        None => {}
    }
}
fn text_way(panel: &w::ChangePanel, out: &mut Text) {
    match panel.way() {
        w::Way::Turn => {
            out.put(b"way: turn");
            if let Some(task) = panel.in_landing() {
                out.put(b" \xc2\xb7 landing task ");
                out.number(*task);
            }
            out.put(b"\n");
        }
        w::Way::Queue => {}
    }
}
fn text_checks(panel: &w::ChangePanel, out: &mut Text) {
    out.put(b"head ");
    match panel.head() {
        Some(head) => out.put(&hex(head)),
        None => out.put(b"unknown"),
    }
    out.put(b" \xc2\xb7 CI ");
    out.put(ci(panel.ci()));
    out.put(b"\n");
    if panel.gates().is_empty() {
        out.put(b"gates: none\n");
    }
    for gate in panel.gates() {
        out.put(b"gate ");
        out.number(gate.number());
        out.put(b" \xc2\xb7 ");
        out.put(gate_kind(gate.kind()));
        out.put(if gate.blocks() { b" \xc2\xb7 blocking" } else { b" \xc2\xb7 advisory" });
        out.put(b" \xc2\xb7 ");
        out.put(verdict(gate.verdict()));
        out.put(b"\n");
    }
    for (i, (label, used)) in [
        (b"repairs ".as_slice(), panel.repairs()),
        (b"resolutions ", panel.resolutions()),
        (b"updates ", panel.updates()),
    ]
    .iter()
    .enumerate()
    {
        if i > 0 {
            out.put(b" \xc2\xb7 ");
        }
        out.put(label);
        out.number(u64::from(used.used()));
        out.put(b" of ");
        out.number(u64::from(used.limit()));
    }
    out.put(b"\n");
}
fn digest(panel: &w::ChangePanel, limits: &Limits) -> Option<Digest> {
    let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
    let mut measure = Encoder::measure(&bounds);
    encode(panel, None, &mut measure);
    let mut out = Encoder::write(measure.measured().ok()?, &bounds);
    encode(panel, None, &mut out);
    Some(Digest::of(&out.finish()))
}
pub(crate) fn print(
    panel: Option<&w::ChangePanel>,
    json_output: bool,
    met: Option<bool>,
    why: &[u8],
    limits: &Limits,
    run_hold: Option<&[u8]>,
) -> jig::Printed {
    let digest = match panel {
        Some(panel) => match digest(panel, limits) {
            Some(digest) => Some(digest),
            None => return render::failure(b"result limit", json_output, 1),
        },
        None => None,
    };
    let bytes = if json_output {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        json(panel, digest, met, why, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut out = Encoder::write(size, &bounds);
        json(panel, digest, met, why, &mut out);
        out.finish()
    } else {
        let mut measure = Text::measure();
        text(panel, digest, met, why, run_hold, &mut measure);
        let mut out = Text::writing_exact(measure);
        text(panel, digest, met, why, run_hold, &mut out);
        out.finish()
    };
    if bytes.len() > usize::try_from(limits.tools.text).expect("text fits") {
        return render::failure(b"result limit", json_output, 1);
    }
    jig::Printed {
        file: jig::FileEnd::Keep,
        status: if met == Some(false) { 3 } else { 0 },
        stdout: bytes,
        stderr: Box::default(),
    }
}
