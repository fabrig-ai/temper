//! Level-triggered waits over the client's current documents (tools contract, 5.4).
use crate::jig::{
    Limits,
    call::{Value, argument},
    conditions::{self, Facts},
    digest::{self, Digest},
    render::{self, Part},
    runner::{self, Called, Out, Runner},
    subject::{Condition, Subject},
};
use alloc::boxed::Box;
use jig_client::{Client, Event, Handle, Latest, Readable, Readout, State, Told, Watchable, Why, contract as c};
use skein_lib::{Deadlines, Env, Queue, Time};
#[derive(Debug)]
pub(crate) struct Wait {
    pub(crate) called: Called,
    pub(crate) handle: Handle,
    pub(crate) until: Option<Condition>,
    since: Option<Digest>,
    first: Option<Digest>,
    me: bool,
    stage: Stage,
    name: &'static [u8],
}
#[derive(Debug)]
enum Stage {
    Watching,
    Finishing { read: Handle },
}
impl Wait {
    #[expect(clippy::manual_map, reason = "the domain matches without closures")]
    fn answers(&self, digest: Digest, holds: Option<bool>) -> bool {
        let verdict = match self.until {
            Some(_) => Some(holds == Some(true)),
            None => None,
        };
        met(self.since, self.first, digest, verdict)
    }
}
pub(crate) fn start(runner: &mut Runner, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    let subject = crate::jig::subject::of(&called.call).expect("watched read");
    let row = crate::jig::rows::watched(called.call.tool).expect("watched read");
    let me = live_me(client, runner.me()).expect("signed-in call");
    let Some((subject, name)) = watch(&subject, &called.call, me, env.limits.rows) else {
        let printed = render::failure(b"unknown project", called.json, 1);
        runner::finish(called, printed, out);
        return;
    };
    let handle = if subject == c::Subject::Me {
        runner.me()
    } else {
        Handle { asker: runner.asker, number: u64::from(called.call_number) << 32u32 }
    };
    let until = match argument(row, &called.call, b"until") {
        Some(Value::Condition(until)) => Some(*until),
        _ => None,
    };
    let since = match argument(row, &called.call, b"since") {
        Some(Value::Digest(bytes)) => Some(Digest::from_bytes(*bytes)),
        _ => None,
    };
    let Some(Value::Duration(timeout)) = argument(row, &called.call, b"timeout") else {
        unreachable!("checked timeout")
    };
    let deadline = env.now.checked_add(*timeout).unwrap_or(Time::from_nanos(u64::MAX));
    let wait = Wait {
        called,
        handle,
        until,
        since,
        first: None,
        me: until == Some(Condition::WaitingOnMe)
            || until == Some(Condition::Replied)
            || (subject == c::Subject::Accounts && me.deployment_role == Some(c::HeldRole::Owner)),
        stage: Stage::Watching,
        name,
    };
    let number = wait.called.call_number;
    if runner.wait_timers.is_none() {
        runner.wait_timers = Some(Deadlines::with_capacity(env.limits.rows));
    }
    // One timer per admitted call: bounded and priced by Deadlines::worst_case(rows).
    runner.wait_timers.as_mut().expect("wait timers").arm(number, deadline).expect("bounded calls");
    if let Some(handle) = deployment_handle(&wait) {
        out.push(Out::Client { event: Event::Watch { handle, subject: Watchable::Jig(c::Subject::Deployment) } });
    }
    runner.waits.insert(number, wait);
    if handle == runner.me() {
        changed(runner, client, env, number, out);
    } else {
        out.push(Out::Client { event: Event::Watch { handle, subject: Watchable::Jig(subject) } });
    }
}
fn watch(
    subject: &Subject,
    call: &crate::jig::call::Call,
    me: &c::Me,
    window: u32,
) -> Option<(c::Subject, &'static [u8])> {
    Some(match subject {
        Subject::Me => (c::Subject::Me, b"me"),
        Subject::Accounts => (c::Subject::Accounts, b"accounts"),
        Subject::Inbox => (c::Subject::Inbox { window }, b"inbox"),
        Subject::Task(task) => (c::Subject::Task { task: *task }, b"task"),
        Subject::Tree(task) => (c::Subject::Tree { task: *task, window }, b"tree"),
        Subject::Run(task) => (c::Subject::Run { task: *task, turn: None, offset: 0 }, b"run"),
        Subject::Tasks => (c::Subject::Tasks { query: runner::reads::task_query(call, me)?, window }, b"tasks"),
    })
}
fn live_me(client: &Client, handle: Handle) -> Option<&c::Me> {
    let document = client.document(handle)?;
    if document.state != State::Live {
        return None;
    }
    match document.latest {
        Some(Latest::Jig(c::Delivery::Me(me))) => Some(me),
        _ => None,
    }
}
fn current_run(client: &Client, handle: Handle) -> Option<Latest> {
    let document = client.document(handle)?;
    let Some(Latest::Jig(c::Delivery::RunState(run))) = document.latest else { return None };
    let mut run = run.clone();
    run.writing = client.text(handle).cloned();
    Some(Latest::Jig(c::Delivery::RunState(run)))
}
pub(crate) fn told(runner: &mut Runner, client: &Client, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) -> bool {
    match told {
        Told::Ended { handle, why } => return ended(runner, client, env, *handle, *why, out),
        Told::Page { handle, page: result } => return page(runner, client, env, *handle, result, out),
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Changed { .. }
        | Told::Text { .. } => {}
    }
    let handle = match told {
        Told::Changed { handle } | Told::Text { handle } => *handle,
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => return false,
    };
    let mut next = None;
    for _ in 0..runner.waits.len() {
        let entry = match next {
            None => runner.waits.first_key_value(),
            Some(number) => {
                runner.waits.range((core::ops::Bound::Excluded(number), core::ops::Bound::Unbounded)).next()
            }
        };
        let Some((&number, wait)) = entry else { break };
        next = Some(number);
        if wait.handle == handle || deployment_handle(wait) == Some(handle) || (handle == runner.me() && wait.me) {
            changed(runner, client, env, number, out);
        }
    }
    false
}
fn changed(runner: &mut Runner, client: &Client, env: &Env<Limits>, number: u32, out: &mut Queue<Out>) {
    let Some(wait) = runner.waits.get_mut(&number) else { return };
    if let Stage::Finishing { .. } = wait.stage {
        return;
    }
    let Some(document) = client.document(wait.handle) else { return };
    if document.state != State::Live && document.state != State::Ended {
        return;
    }
    let run = current_run(client, wait.handle);
    let Some(latest) = run.as_ref().or(document.latest) else { return };
    let Latest::Jig(delivery) = latest else { return };
    let digest = if wait.name == b"accounts" {
        let accounts = accounts(wait, client);
        if !accounts.current {
            return;
        }
        accounts.digest(&env.limits)
    } else {
        let Some(part) = Part::delivery(delivery) else { return };
        digest::part(&part, &env.limits)
    };
    let holds = match wait.until {
        Some(Condition::Changed) => Some(wait.first.is_some() && met(wait.since, wait.first, digest, None)),
        Some(until) => conditions::holds(
            &until,
            &Facts { subject: latest, me: live_me(client, Handle { asker: runner.asker, number: 0 }) },
        ),
        None => None,
    };
    if wait.answers(digest, holds) {
        if wait.until == Some(Condition::Ended) && wait.name == b"task" {
            finishing(runner, number, out);
        } else {
            complete(runner, client, env, number, Ok(Some(digest)), out);
        }
    } else if wait.first.is_none() {
        wait.first = Some(digest);
    }
}
fn complete(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    number: u32,
    result: Result<Option<Digest>, &[u8]>,
    out: &mut Queue<Out>,
) {
    let Some(wait) = runner.waits.remove(&number) else { return };
    runner.wait_timers.as_mut().expect("wait timers").cancel(number);
    let run = current_run(client, wait.handle);
    let latest = match client.document(wait.handle) {
        Some(document) => run.as_ref().or(document.latest),
        None => None,
    };
    let delivery = match latest {
        Some(Latest::Jig(delivery)) => Some(delivery),
        _ => None,
    };
    let (met, why, digest) = match result {
        Ok(digest) => (true, b"".as_slice(), digest),
        Err(why) => (false, why, None),
    };
    let printed = if wait.name == b"accounts" {
        let mut value = accounts(&wait, client);
        value.outcome = Some((met, why));
        let mut printed = render::document::print(
            render::document::Document::Accounts(value),
            wait.called.json,
            &env.limits,
            runner.parts.expect("host vocabulary"),
        );
        if printed.status == 0 && !met {
            printed.status = 3;
        }
        printed
    } else {
        render::waiting(
            render::document::waiting::Waiting {
                result: None,
                delivery,
                me: live_me(client, runner.me()),
                name: wait.name,
                met,
                why,
                digest,
            },
            wait.called.json,
            &env.limits,
            runner.parts.expect("host vocabulary"),
        )
    };
    close(&wait, runner.me(), out);
    runner::finish(wait.called, printed, out);
}
pub(crate) fn fire(runner: &mut Runner, client: &Client, env: &Env<Limits>, out: &mut Queue<Out>) {
    let count = match &runner.wait_timers {
        Some(timers) => timers.len(),
        None => 0,
    };
    for _ in 0..count {
        let Some(number) = runner.wait_timers.as_mut().expect("wait timers").expire(env.now) else { break };
        complete(runner, client, env, number, Err(b"timeout"), out);
    }
}
#[must_use]
pub(crate) fn next_deadline(runner: &Runner) -> Option<Time> {
    runner.wait_timers.as_ref()?.next()
}

fn finishing(runner: &mut Runner, number: u32, out: &mut Queue<Out>) {
    let wait = runner.waits.get_mut(&number).expect("active wait");
    let task = crate::jig::call::number(&wait.called.call, 0).expect("task wait");
    let read = Handle { asker: runner.asker, number: (u64::from(number) << 32u32) | 1 };
    wait.stage = Stage::Finishing { read };
    runner.wait_timers.as_mut().expect("wait timers").cancel(number);
    out.push(Out::Client { event: Event::Unwatch { handle: wait.handle } });
    out.push(Out::Client { event: Event::Read { handle: read, read: Readable::Jig(c::Read::Task { task }) } });
}
fn ended(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    handle: Handle,
    why: Why,
    out: &mut Queue<Out>,
) -> bool {
    let shared = handle == runner.me();
    let mut next = None;
    let mut consumed = false;
    for _ in 0..runner.waits.len() {
        let entry = match next {
            None => runner.waits.first_key_value(),
            Some(number) => {
                runner.waits.range((core::ops::Bound::Excluded(number), core::ops::Bound::Unbounded)).next()
            }
        };
        let Some((&number, wait)) = entry else { break };
        next = Some(number);
        if wait.handle != handle
            && deployment_handle(wait) != Some(handle)
            && !(shared && (wait.me || why == Why::SignedOut))
        {
            continue;
        }
        consumed = true;
        if let Stage::Finishing { .. } = wait.stage
            && !shared
        {
            continue;
        }
        if (wait.until == Some(Condition::Ended) || wait.until == Some(Condition::Replied))
            && wait.name == b"task"
            && (why == Why::Unknown || why == Why::SubjectEnded)
        {
            finishing(runner, number, out);
        } else if why == Why::SubjectEnded {
            if wait.until == Some(Condition::Ended) {
                // The renderer digests the last document, including current run text.
                complete(runner, client, env, number, Ok(None), out);
            } else {
                complete(runner, client, env, number, Err(b"ended"), out);
            }
        } else {
            let (reason, status): (&[u8], u8) = match why {
                Why::NotAllowed => (b"not_allowed", 2),
                Why::Unknown => (b"unknown", 2),
                Why::SignedOut => (b"signed out", 1),
                Why::NotServed => (b"version not served", 1),
                Why::Limit => (b"watch table limit", 1),
                Why::SubjectEnded => unreachable!("handled above"),
            };
            let wait = runner.waits.remove(&number).expect("active wait");
            runner.wait_timers.as_mut().expect("wait timers").cancel(number);
            close(&wait, runner.me(), out);
            let printed = render::failure(reason, wait.called.json, status);
            runner::finish(wait.called, printed, out);
        }
    }
    consumed
}
fn page(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    handle: Handle,
    page: &Readout,
    out: &mut Queue<Out>,
) -> bool {
    let number = u32::try_from(handle.number >> 32u32).expect("high half");
    let Some(wait) = runner.waits.get(&number) else { return false };
    let Stage::Finishing { read } = wait.stage else { return false };
    if read != handle {
        return false;
    }
    let wait = runner.waits.remove(&number).expect("finishing wait");
    let expected = crate::jig::call::number(&wait.called.call, 0).expect("finishing task wait");
    let printed = match page {
        Readout::Jig(c::ReadResult::Value(value)) if final_task(expected, value).is_some() => {
            let (task, result) = final_task(expected, value).expect("matching terminal task read");
            let latest = Latest::Jig(c::Delivery::Task(Box::new(task.clone())));
            let me = live_me(client, runner.me());
            let met = conditions::holds(&wait.until.expect("finishing condition"), &Facts { subject: &latest, me })
                == Some(true);
            let Latest::Jig(delivery) = latest else { unreachable!("jig task read") };
            render::waiting(
                render::document::waiting::Waiting {
                    result,
                    delivery: Some(&delivery),
                    me,
                    name: wait.name,
                    met,
                    why: if met { b"" } else { b"ended" },
                    digest: None,
                },
                wait.called.json,
                &env.limits,
                runner.parts.expect("host vocabulary"),
            )
        }
        Readout::Jig(c::ReadResult::Value(_)) => {
            render::failure(b"tools' bug; unexpected final task; report it as a finding", wait.called.json, 1)
        }
        Readout::Limit | Readout::NotServed { .. } | Readout::Jig(c::ReadResult::None(_)) | Readout::Family { .. } => {
            render::read(
                page,
                client,
                runner.me(),
                wait.called.json,
                &env.limits,
                runner.parts.expect("host vocabulary"),
            )
        }
    };
    runner::finish(wait.called, printed, out);
    true
}

// A closed watch alone proves neither the requested identity nor completion.
fn final_task(expected: u64, value: &c::ReadValue) -> Option<(&c::Task, Option<&c::Result>)> {
    let (task, result) = render::read_task(value)?;
    if task.chip.task != expected {
        return None;
    }
    match task.chip.phase {
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => Some((task, result)),
        c::Phase::Waiting
        | c::Phase::Idle
        | c::Phase::Due
        | c::Phase::Preparing
        | c::Phase::Claimed
        | c::Phase::Running
        | c::Phase::BackingOff
        | c::Phase::Active
        | c::Phase::Closing
        | c::Phase::Held => None,
    }
}

// The existing me dependency flag records the owner join for Accounts; no new
// per-call retained document or allocation. Both watches live in jig-client.
fn deployment_handle(wait: &Wait) -> Option<Handle> {
    if wait.name == b"accounts" && wait.me {
        Some(Handle { asker: wait.handle.asker, number: wait.handle.number | 1 })
    } else {
        None
    }
}
fn close(wait: &Wait, me: Handle, out: &mut Queue<Out>) {
    if let Some(handle) = deployment_handle(wait) {
        out.push(Out::Client { event: Event::Unwatch { handle } });
    }
    if wait.handle != me {
        out.push(Out::Client { event: Event::Unwatch { handle: wait.handle } });
    }
}
fn accounts<'a>(wait: &Wait, client: &'a Client) -> render::document::accounts::Accounts<'a> {
    let mut value = render::document::accounts::Accounts {
        party: None,
        deployment: None,
        models: None,
        outcome: None,
        current: false,
    };
    if let Some(document) = client.document(wait.handle)
        && let Some(Latest::Jig(c::Delivery::OwnerAccounts(party))) = document.latest
    {
        value.party = Some(party);
        value.current = document.state == State::Live;
    }
    if let Some(handle) = deployment_handle(wait) {
        value.current &= match live_me(client, Handle { asker: wait.handle.asker, number: 0 }) {
            Some(me) => me.deployment_role == Some(c::HeldRole::Owner),
            None => false,
        };
        match client.document(handle) {
            Some(document) => {
                value.current &= document.state == State::Live;
                match document.latest {
                    Some(Latest::Jig(c::Delivery::OwnerDeployment(deployment))) => value.deployment = Some(deployment),
                    _ => value.current = false,
                }
            }
            None => value.current = false,
        }
    }
    value
}

/// A family owner supplies its document digest and its condition verdict. None
/// means no condition; a condition awaiting facts supplies Some(false).
#[must_use]
pub fn met(since: Option<Digest>, first: Option<Digest>, digest: Digest, holds: Option<bool>) -> bool {
    let differs = match since.or(first) {
        Some(old) => old != digest,
        None => false,
    };
    match (holds, since) {
        (Some(holds), Some(_)) => differs && holds,
        (Some(holds), None) => holds,
        (None, _) => differs,
    }
}
