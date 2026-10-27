//! Offered acts are read once, then kept by the shared client's request table.
pub(crate) mod choice;
mod subject;
use super::{Called, Out, Runner, finish};
use crate::jig::{
    Limits,
    call::{Call, Value, Values, number, value},
    render::{self, outcome::Operation},
    rows,
};
use alloc::boxed::Box;
use jig_client::{Client, Event, Handle, Key, Latest, Readable, Readout, contract as c, shapes};
use skein_lib::{Env, List, Queue};
#[derive(Debug)]
pub(super) struct Reading {
    called: Called,
    document: Option<c::ReadValue>,
}
#[derive(Debug)]
pub(super) struct Sent {
    pub(super) called: Called,
    pub(super) operation: Operation,
    pub(super) holder: Option<c::AccountHolder>,
    pub(super) dispatched: bool,
    alternate: Option<c::Ask>,
}
pub(crate) fn me<'a>(runner: &Runner, client: &'a Client) -> Option<&'a c::Me> {
    match client.document(runner.me())?.latest? {
        Latest::Jig(c::Delivery::Me(me)) => Some(me),
        Latest::Jig(_) | Latest::Family(_) => None,
    }
}
pub(super) fn start(runner: &mut Runner, client: &Client, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    if called.call.tool == rows::PROJECT_FUND_AT {
        let Some(me) = me(runner, client) else { return };
        let request = project_fund(&called.call, me);
        prepared(runner, client, env, called, request, out);
        return;
    }
    if called.call.tool == rows::LIFT_AT {
        let ask = c::Ask::Lift {
            hold: number(&called.call, 0).expect("hold argument"),
            revision: c::Revision { value: number(&called.call, 1).expect("revision argument") },
            with_tasks: value(&called.call, 2).is_some(),
        };
        prepared(runner, client, env, called, Ok((Request::Ask(ask), None)), out);
        return;
    }
    if called.call.tool == rows::MOVE_TO_NEWEST_AT {
        let revision = u32::try_from(number(&called.call, 0).expect("revision argument")).expect("revision fits u32");
        prepared(
            runner,
            client,
            env,
            called,
            Ok((Request::Ask(c::Ask::MoveToNewest(c::MoveToNewest { revision })), None)),
            out,
        );
        return;
    }
    if called.call.tool == rows::CHANGE_SETTINGS_AT || called.call.tool == rows::PROPOSE_SETTINGS_AT {
        let request = match crate::jig::settings_file::request(&called.call, &env.limits) {
            Ok(ask) => {
                let ask = if called.call.tool == rows::PROPOSE_SETTINGS_AT {
                    c::Ask::Propose {
                        project: 0,
                        reason: match words(&called.call, 1, &env.limits) {
                            Ok(reason) => reason,
                            Err(why) => {
                                prepared(runner, client, env, called, Err(why), out);
                                return;
                            }
                        },
                        action: c::ProposeAction::Settings(c::ProposeSettings {
                            against: ask.against,
                            items: ask.items,
                        }),
                    }
                } else {
                    c::Ask::ChangeSettings(ask)
                };
                Ok((Request::Ask(ask), None))
            }
            Err(why) => Err(Failed::Detail(why)),
        };
        prepared(runner, client, env, called, request, out);
        return;
    }
    if called.call.tool == rows::CHAT_CREATE_AT {
        let Some(me) = me(runner, client) else { return };
        let ask = chat(&called.call, me, &env.limits);
        prepared(runner, client, env, called, ask, out);
        return;
    }
    let call = &called.call;
    let task = number(call, 0).unwrap_or(0);
    let read = match call.tool {
        rows::DECIDE_AT => {
            let Some(Value::Proposal(proposal)) = value(call, 0) else { unreachable!("proposal form") };
            c::Read::Proposal { proposal: *proposal }
        }
        rows::TASK_DECIDE_AT => c::Read::Escalation { task },
        rows::ANSWER_AT => c::Read::Question { task, message: number(call, 1).expect("question") },
        _ => c::Read::Task { task },
    };
    out.push(Out::Client {
        event: Event::Read {
            handle: Handle { asker: runner.asker, number: (u64::from(called.call_number) << 32u32) | 1 },
            read: Readable::Jig(read),
        },
    });
    runner.acts.insert(called.call_number, Reading { called, document: None });
}
pub(crate) enum Failed {
    Fixed(&'static [u8]),
    Unknown { what: &'static [u8], name: Box<[u8]> },
    EffortRequired { name: Box<[u8]> },
    Detail(Box<[u8]>),
}
pub(crate) type Prepared = Result<(Request, Option<c::Recipient>), Failed>;
fn project_fund(call: &Call, me: &c::Me) -> Prepared {
    match super::reads::project(call, me) {
        Some(project) => {
            let period_spend = number(call, 1).expect("amount argument");
            let edit = match number(call, 2) {
                Some(role) => c::PolicyEdit::RoleSpend { role: u32::try_from(role).expect("role fits"), period_spend },
                None => c::PolicyEdit::ProjectSpend { period_spend },
            };
            Ok((Request::Ask(c::Ask::ChangePolicy { project, edit }), None))
        }
        None => Err(Failed::Fixed(b"unknown project")),
    }
}
fn chat(call: &Call, me: &c::Me, limits: &Limits) -> Prepared {
    let project = super::reads::project(call, me).ok_or(Failed::Fixed(b"unknown project"))?;
    let Ok(mut ask) = shapes::chat(me, project, words(call, 1, limits)?) else {
        return Err(Failed::Fixed(b"chat not offered"));
    };
    let c::Ask::Create { tasks, .. } = &mut ask else { unreachable!("chat creates") };
    let task = tasks.get_mut(0).expect("chat creates one task");
    task.llm = choice::llm(call, 2, 3, me, project, task.llm.as_ref())?;
    let mut creating = None;
    for membership in &me.projects {
        if membership.project == project {
            creating = membership.creating.as_ref();
        }
    }
    let creating = creating.ok_or(Failed::Fixed(b"chat not offered"))?;
    let draft = shapes::GoalDraft {
        words: task.spec.words.clone(),
        executor: task.executor,
        budget: task.authority.spend,
        deadline: task.authority.deadline,
        priority: task.tracked,
        llm: task.llm.clone(),
    };
    match shapes::fall(creating, &draft, task.deadline) {
        shapes::Fall::Within => Ok((Request::Ask(ask), None)),
        shapes::Fall::Proposable(to) => {
            let c::Ask::Create { tasks, .. } = ask else { unreachable!("chat creates") };
            Ok((
                Request::Ask(c::Ask::Propose {
                    project,
                    action: c::ProposeAction::Batch { tasks },
                    reason: empty(limits),
                }),
                Some(to),
            ))
        }
        shapes::Fall::Refused(_) | shapes::Fall::NotOffered(_) => Err(Failed::Fixed(b"chat exceeds offered bounds")),
    }
}
pub(crate) fn words(call: &Call, index: usize, limits: &Limits) -> Result<c::Text, Failed> {
    let bytes = match value(call, index) {
        Some(Value::Text(bytes)) => bytes.as_ref(),
        None => b"",
        _ => return Err(Failed::Fixed(b"text not loaded")),
    };
    match c::Text::new(Box::from(bytes), limits.text) {
        Ok(text) => Ok(text),
        Err(_) => Err(Failed::Fixed(b"text limit")),
    }
}
fn empty(limits: &Limits) -> c::Text {
    c::Text::new(Box::default(), limits.text).expect("empty words fit")
}
pub(super) fn page(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    handle: Handle,
    page: Readout,
    out: &mut Queue<Out>,
) {
    let prefix = u32::try_from(handle.number >> 32u32).expect("high half");
    let Some(mut reading) = runner.acts.remove(&prefix) else { return };
    if let Some(document) = reading.document.take() {
        let called = reading.called;
        let ask = match &page {
            Readout::Jig(c::ReadResult::Value(value)) => match render::read_task(value) {
                Some((task, _)) if subject::routes_through(&called.call, &document, task.chip.task) => {
                    let Some(me) = me(runner, client) else { return };
                    build(&called.call, &document, me, &env.limits, Some(task.project))
                }
                Some(_) | None => Err(Failed::Fixed(b"task project unavailable")),
            },
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                Err(Failed::Fixed(b"task project unavailable"))
            }
        };
        prepared(runner, client, env, called, ask, out);
        return;
    }
    if let Readout::Jig(c::ReadResult::Value(document)) = &page
        && let Ok(subject::Scope::Task(task)) = subject::scope(&reading.called.call, document)
    {
        out.push(Out::Client { event: Event::Read { handle, read: Readable::Jig(c::Read::Task { task }) } });
        reading.document = Some(document.clone());
        runner.acts.insert(prefix, reading);
        return;
    }
    let called = reading.called;
    let Some(me) = me(runner, client) else {
        let printed = render::failure(b"signed out", called.json, 1);
        finish(called, printed, out);
        return;
    };
    if called.call.tool == rows::AMEND_AT
        && let Readout::Jig(c::ReadResult::Value(document)) = &page
        && let Some((task, _)) = render::read_task(document)
        && task.chip.task == number(&called.call, 0).expect("task")
    {
        out.push(Out::DraftAmendment { called: Box::new(called), task: Box::new(task.clone()) });
        return;
    }
    let ask = match &page {
        Readout::Jig(c::ReadResult::Value(document)) => build(&called.call, document, me, &env.limits, None),
        Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
            let printed = render::read(
                &page,
                client,
                runner.me(),
                called.json,
                &env.limits,
                runner.parts.expect("host vocabulary"),
            );
            finish(called, printed, out);
            return;
        }
    };
    prepared(runner, client, env, called, ask, out);
}
fn settings_history(call: &Call, document: &c::ReadValue) -> bool {
    if call.tool == rows::DECIDE_AT
        && let c::ReadValue::Historical(history) = document
        && let Some(Value::Proposal(key)) = value(call, 0)
        && render::historical_proposal(history) == Some(*key)
        && let c::OwnerHistoricalValue::Proposal(proposal) = &history.value
        && match proposal.action {
            c::OwnerRetainedAction::Settings(_) => true,
            c::OwnerRetainedAction::Batch(_)
            | c::OwnerRetainedAction::Effect { .. }
            | c::OwnerRetainedAction::ProcedureEffect { .. }
            | c::OwnerRetainedAction::Amend { .. }
            | c::OwnerRetainedAction::Widen { .. }
            | c::OwnerRetainedAction::Release { .. } => false,
        }
    {
        return true;
    }
    false
}

fn settings_action(action: &c::OwnerProposalAction) -> bool {
    match action {
        c::OwnerProposalAction::Settings(_) => true,
        c::OwnerProposalAction::Batch(_)
        | c::OwnerProposalAction::Effect { .. }
        | c::OwnerProposalAction::ProcedureEffect { .. }
        | c::OwnerProposalAction::Amend { .. }
        | c::OwnerProposalAction::Widen { .. }
        | c::OwnerProposalAction::Release { .. } => false,
    }
}

fn build(call: &Call, document: &c::ReadValue, me: &c::Me, limits: &Limits, project: Option<u32>) -> Prepared {
    if call.tool == rows::AMEND_AT {
        return Err(Failed::Fixed(b"unexpected amendment document"));
    }
    if call.tool == rows::CLOSE_AT {
        return close_request(call, document, me, limits);
    }
    if settings_history(call, document) {
        return Ok((Request::Ask(c::Ask::Decide { project: 0, on: decide(call, 0, limits, true)? }), None));
    }
    let project = match subject::scope(call, document)? {
        subject::Scope::Project(project) => project,
        subject::Scope::Task(_) => project.ok_or(Failed::Fixed(b"project unavailable"))?,
    };
    let offers = subject::offers(document);
    let act = kind(call);
    let mut admission = None;
    if let Some(offers) = offers {
        for offer in offers {
            if offer.act == act {
                admission = Some(&offer.admission);
            }
        }
    }
    let to = match admission {
        Some(c::Admission::Proposed { to }) => Some(to.clone()),
        Some(c::Admission::Bounded { .. }) => return Err(Failed::Fixed(b"act has no bounded draft")),
        // A replay must reach the engine even when its subject no longer
        // offers this act. The engine alone refuses a fresh key.
        Some(c::Admission::Allowed | c::Admission::Refused(_)) | None => None,
    };
    let settings =
        if let c::ReadValue::OwnerProposal(proposal) = document { settings_action(&proposal.action) } else { false };
    let task = number(call, 0).unwrap_or(0);
    let ask = match call.tool {
        rows::MESSAGE_AT => c::Ask::Message { project, task, words: words(call, 1, limits)? },
        rows::ANSWER_AT => c::Ask::Answer {
            project,
            on: c::AnswerOn::Question {
                task,
                message: number(call, 1).expect("question"),
                words: words(call, 2, limits)?,
            },
        },
        rows::DECIDE_AT | rows::TASK_DECIDE_AT => c::Ask::Decide { project, on: decide(call, task, limits, settings)? },
        rows::RELEASE_AT => c::Ask::Release {
            project,
            task,
            revision: c::Revision { value: number(call, 1).expect("hold") },
            again: if value(call, 2).is_some() { Some(true) } else { None },
        },
        rows::STOP_AT => c::Ask::Stop { project, task },
        rows::CANCEL_AT => c::Ask::Cancel {
            project,
            task,
            reason: words(call, 2, limits)?,
            grace: match value(call, 1) {
                Some(Value::Duration(duration)) => Some(*duration),
                _ => None,
            },
        },
        _ => unreachable!("act tool"),
    };
    let phase = subject::ended(document);
    Ok((Request::Act { ask, phase }, to))
}
fn decide(call: &Call, task: u64, limits: &Limits, settings: bool) -> Result<c::DecideOn, Failed> {
    Ok(if call.tool == rows::DECIDE_AT {
        let Some(Value::Proposal(proposal)) = value(call, 0) else { unreachable!("proposal form") };
        let decision = decision(call, limits)?;
        let running = match value(call, 3) {
            Some(Value::Word(0)) => Some(c::Running::Stays),
            Some(Value::Word(1)) => Some(c::Running::Moves),
            None if settings && decision == c::Decision::Accepted => Some(c::Running::Stays),
            Some(_) | None => None,
        };
        c::DecideOn::Proposal { proposal: *proposal, decision, running }
    } else {
        c::DecideOn::Escalation {
            task,
            revision: c::Revision { value: number(call, 1).expect("hold") },
            decision: decision(call, limits)?,
        }
    })
}

fn chip_project(chip: &c::Chip) -> Option<u32> {
    match chip.details.as_ref() {
        c::ChipPhase::Owner { details, .. } => Some(details.project),
        c::ChipPhase::Active(_)
        | c::ChipPhase::Waiting { .. }
        | c::ChipPhase::Idle { .. }
        | c::ChipPhase::Starting
        | c::ChipPhase::Running { .. }
        | c::ChipPhase::WaitingForWords { .. }
        | c::ChipPhase::BackingOff { .. }
        | c::ChipPhase::Procedure { .. }
        | c::ChipPhase::Person { .. }
        | c::ChipPhase::Closing(_)
        | c::ChipPhase::Held { .. }
        | c::ChipPhase::Ended { .. } => None,
    }
}
fn kind(call: &Call) -> c::ActKind {
    match call.tool {
        rows::MESSAGE_AT => c::ActKind::Message,
        rows::CLOSE_AT | rows::CANCEL_AT => c::ActKind::Cancel,
        rows::ANSWER_AT => c::ActKind::Answer,
        rows::DECIDE_AT | rows::TASK_DECIDE_AT => match value(call, if call.tool == rows::DECIDE_AT { 1 } else { 2 }) {
            Some(Value::Word(0)) => {
                if call.tool == rows::DECIDE_AT {
                    c::ActKind::Accept
                } else {
                    c::ActKind::Release
                }
            }
            Some(Value::Word(1)) => {
                if call.tool == rows::DECIDE_AT {
                    c::ActKind::Reject
                } else {
                    c::ActKind::LeaveHeld
                }
            }
            _ => c::ActKind::PassUp,
        },
        rows::RELEASE_AT => c::ActKind::Release,
        rows::STOP_AT => c::ActKind::Stop,
        _ => unreachable!("act tool"),
    }
}
fn decision(call: &Call, limits: &Limits) -> Result<c::Decision, Failed> {
    let index = if call.tool == rows::DECIDE_AT { 1 } else { 2 };
    let reason = words(call, if call.tool == rows::DECIDE_AT { 2 } else { 3 }, limits)?;
    Ok(match value(call, index) {
        Some(Value::Word(0)) => {
            if call.tool == rows::DECIDE_AT {
                c::Decision::Accepted
            } else {
                c::Decision::Released { again: None }
            }
        }
        Some(Value::Word(1)) => {
            if call.tool == rows::DECIDE_AT {
                c::Decision::Rejected { reason }
            } else {
                c::Decision::LeftHeld { reason }
            }
        }
        _ => c::Decision::PassedUp,
    })
}
pub(crate) enum Request {
    MoveOut { holder: c::AccountHolder, account: u32 },
    Written { holder: c::AccountHolder, account: u32, number: u32 },
    MoveIn,
    Ask(c::Ask),
    Amend { ask: c::Ask, alternate: Option<c::Ask> },
    Act { ask: c::Ask, phase: Option<c::Phase> },
    Account { ask: c::Ask, context: render::outcome::Account },
    AddKey { holder: c::AccountHolder, provider: u32, join: Option<u32> },
    ReplaceKey { holder: c::AccountHolder, source: u32 },
    DeclareProgram { holder: c::AccountHolder, provider: u32, join: Option<u32>, name: Box<[u8]> },
    CheckProgram { holder: c::AccountHolder, source: u32 },
}
pub(crate) fn prepared(
    runner: &mut Runner,
    client: &Client,
    _env: &Env<Limits>,
    called: Called,
    request: Prepared,
    out: &mut Queue<Out>,
) {
    let (request, to) = match request {
        Ok(request) => request,
        Err(why) => {
            let printed = match why {
                Failed::Fixed(why) => render::failure(why, called.json, 1),
                Failed::Detail(why) => render::failure(&why, called.json, 1),
                Failed::Unknown { what, name } => choice::named(b"unknown ", what, &name, called.json),
                Failed::EffortRequired { name } => choice::named(b"effort required", b"", &name, called.json),
            };
            finish(called, printed, out);
            return;
        }
    };
    send(runner, client, called, request, to, out);
}
fn context(request: Request) -> (Request, Option<render::outcome::Account>, Option<c::Phase>) {
    match request {
        Request::Act { ask, phase } => (Request::Ask(ask), None, phase),
        Request::Account { ask, context } => (Request::Ask(ask), Some(context), None),
        request @ (Request::MoveOut { .. }
        | Request::Written { .. }
        | Request::MoveIn
        | Request::Ask(_)
        | Request::Amend { .. }
        | Request::AddKey { .. }
        | Request::ReplaceKey { .. }
        | Request::DeclareProgram { .. }
        | Request::CheckProgram { .. }) => (request, None, None),
    }
}
fn amendment_request(request: Request) -> (Request, Option<c::Ask>, bool) {
    match request {
        Request::Amend { ask, alternate } => {
            let retry = alternate.is_none();
            (Request::Ask(ask), alternate, retry)
        }
        request @ (Request::MoveOut { .. }
        | Request::Written { .. }
        | Request::MoveIn
        | Request::Ask(_)
        | Request::Act { .. }
        | Request::Account { .. }
        | Request::AddKey { .. }
        | Request::ReplaceKey { .. }
        | Request::DeclareProgram { .. }
        | Request::CheckProgram { .. }) => (request, None, false),
    }
}

fn send(
    runner: &mut Runner,
    client: &Client,
    mut called: Called,
    request: Request,
    to: Option<c::Recipient>,
    out: &mut Queue<Out>,
) {
    let (request, alternate, retry) = amendment_request(request);
    let mut key = crate::jig::key::local(runner.asker, called.call_number);
    if retry {
        key.number = (u64::from(called.call_number) << 32u32) | 2;
    }
    if client.pending(key).is_some() {
        let printed = render::failure(b"tools' bug; client key collision; report it as a finding", called.json, 1);
        finish(called, printed, out);
        return;
    }
    draw_name(runner, &mut called);
    let name = match &called.call.key {
        Some(name) => Some(crate::jig::key::named(runner.salt, name)),
        None => None,
    };
    let (request, account, ended_task) = context(request);
    let mut holder = None;
    let mut exported = None;
    let event = match request {
        Request::MoveOut { holder: h, account } => {
            holder = Some(h);
            jig_client::accounts::move_out(key, name.expect("move name"), h, account)
        }
        Request::Written { holder, account, number } => {
            exported = Some((account, number));
            jig_client::accounts::written(key, name.expect("written name"), holder, number)
        }
        Request::MoveIn => jig_client::accounts::move_in(
            key,
            name.expect("import name"),
            c::AccountHolder::Own,
            called.value.take().expect("record supplied"),
        ),
        Request::Account { .. } | Request::Act { .. } | Request::Amend { .. } => {
            unreachable!("request context extracted")
        }
        Request::DeclareProgram { holder, provider, join, name: program } => {
            jig_client::accounts::declare_program(key, name.expect("declaration name"), holder, provider, join, program)
        }
        Request::CheckProgram { holder, source } => {
            jig_client::accounts::check_program(key, name.expect("check name"), holder, source)
        }
        Request::Ask(ask) => Event::Send { key, name, request: jig_client::Ask::Jig(Box::new(ask)) },
        Request::AddKey { holder, provider, join } => jig_client::accounts::add_key(
            key,
            name.expect("key request name"),
            holder,
            provider,
            join,
            called.value.take().expect("admitted value"),
        ),
        Request::ReplaceKey { holder, source } => jig_client::accounts::replace_key(
            key,
            name.expect("key request name"),
            holder,
            source,
            called.value.take().expect("admitted value"),
        ),
    };
    let (Event::Send { request, .. } | Event::Carry { request, .. }) = &event else { unreachable!("act event") };
    let mut operation = Operation::for_call(request, render::outcome::act_tool(called.call.tool));
    operation.to = to;
    operation.exported = exported;
    if called.call.tool == rows::EXPORT_AT {
        let Some(Value::File(file)) = value(&called.call, 1) else { unreachable!("export file") };
        operation.file = Some(file.clone());
    }
    operation.account = account;
    operation.ended_task = ended_task;
    if called.call.tool != rows::EXPORT_AT
        && called.call.tool != rows::CHANGE_SETTINGS_AT
        && called.call.tool != rows::PROPOSE_SETTINGS_AT
    {
        operation.retry = Some(render::outcome::retry(&called.call));
    }
    if called.call.tool == rows::ADD_KEY_AT
        || called.call.tool == rows::REPLACE_KEY_AT
        || called.call.tool == rows::IMPORT_AT
    {
        operation.value_name.clone_from(&called.call.key);
    }
    if let Some(me) = me(runner, client) {
        operation.party = Some(me.party.number);
    }
    // The compound's public operation is close, rather than its final cancellation.
    if called.call.tool == rows::CLOSE_AT {
        operation.word = b"close";
    }
    if (rows::CONNECT_AT..=rows::RETRY_AT).contains(&called.call.tool)
        || called.call.tool == rows::DECLARE_AT
        || called.call.tool == rows::VERIFY_AT
        || called.call.tool == rows::EXPORT_AT
        || called.call.tool == rows::IMPORT_AT
    {
        // Printed operation words stay unchanged until the calls-rendering task.
        operation.word = render::outcome::account_word(called.call.tool);
    }
    runner.sent.insert(key, Sent { called, operation, holder, dispatched: false, alternate });
    out.push(Out::Client { event });
}
pub(super) fn answered(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    key: Key,
    outcome: &jig_client::Outcome,
    out: &mut Queue<Out>,
) {
    if let Some(mut sent) = runner.sent.remove(&key) {
        if amendment_conflict(outcome)
            && let Some(alternate) = sent.alternate.take()
        {
            send(runner, client, sent.called, Request::Amend { ask: alternate, alternate: None }, None, out);
            return;
        }
        if let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
            c::OwnerOutcome::Proposed { to, .. },
        )))) = outcome
        {
            let Some(project) = sent.operation.proposal_project else {
                let printed = render::failure(
                    b"tools' bug; proposal answer lacks request scope; report it as a finding",
                    sent.called.json,
                    1,
                );
                finish(sent.called, printed, out);
                return;
            };
            sent.operation.to = Some(c::Recipient::Role { project, role: *to });
        }
        if sent.called.call.tool == rows::EXPORT_AT {
            super::moves::answered(runner, client, env, sent, outcome, out);
            return;
        }
        if sent.called.call.tool == rows::PROJECT_FUND_AT
            && let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::PolicyChanged { project },
            )))) = outcome
        {
            // Read the committed project once, resolving names to the answered identity.
            *sent.called.call.values.get_mut(0).expect("project argument") =
                Some(Values::One(Value::Project(crate::jig::call::Project::Number(*project))));
            super::reads::start(runner, client, env, sent.called, out);
            return;
        }
        let mut printed = render::outcome::print(
            &sent.operation,
            outcome,
            sent.called.json,
            me(runner, client),
            &env.limits,
            runner.parts.expect("host vocabulary").diagnostic_prefix(),
        );
        if sent.called.call.tool == rows::IMPORT_AT
            && let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountMovedIn { .. },
            )))) = outcome
        {
            printed.file = super::FileEnd::Remove;
        }
        finish(sent.called, printed, out);
    } else if let Some(operation) = runner.kept.remove(&key) {
        let line = render::outcome::kept(
            &operation,
            outcome,
            runner.kept_json,
            me(runner, client),
            &env.limits,
            runner.parts.expect("host vocabulary").diagnostic_prefix(),
        );
        out.push(Out::Kept { line });
    }
}

fn amendment_conflict(outcome: &jig_client::Outcome) -> bool {
    if let jig_client::Outcome::Held = outcome {
        return true;
    }
    if let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
        refusal: c::Refusal::KeyConflict,
        ..
    })) = outcome
    {
        return true;
    }
    false
}

fn close_request(call: &Call, document: &c::ReadValue, me: &c::Me, limits: &Limits) -> Prepared {
    let (task, _) = render::read_task(document).ok_or(Failed::Fixed(b"unexpected close document"))?;
    if task.chip.task != number(call, 0).expect("chat argument") {
        return Err(Failed::Fixed(b"unexpected close task"));
    }
    // Only immutable routing scope enters the ask. Terminal history is kept
    // as answer context: an archived task can be Unknown to a fresh act.
    let phase = subject::ended(document);
    Ok((Request::Act { ask: close(call, me, task.project, limits)?, phase }, None))
}

fn close(call: &Call, me: &c::Me, project: u32, limits: &Limits) -> Result<c::Ask, Failed> {
    let moves = match call.values.get(1) {
        Some(Some(Values::Many(moves))) => Some(moves),
        _ => None,
    };
    let count = match moves {
        Some(moves) => u32::try_from(moves.len()).expect("bounded moves"),
        None => 0,
    };
    let mut delegates = List::with_capacity(count);
    if let Some(moves) = moves {
        for moved in moves {
            let Value::Number(task) = moved else { unreachable!("move argument") };
            delegates.push(shapes::Delegate { task: *task, fate: shapes::Fate::Move }).expect("one per move");
        }
    }
    // Cancelling the chat cancels its remaining tree in the engine. Only
    // explicit moves precede it, so changing delegates cannot change the ask.
    match shapes::close_chat(
        me,
        project,
        number(call, 0).expect("chat argument"),
        delegates.as_slice(),
        words(call, 2, limits)?,
    ) {
        Ok(ask) => Ok(ask),
        Err(_) => Err(Failed::Fixed(b"close not offered")),
    }
}

fn draw_name(runner: &mut Runner, called: &mut Called) {
    // Tool 33 draws a key because check_program requires one even when verify omits --key.
    if (called.value.is_some() || called.call.tool == rows::VERIFY_AT || called.call.tool == rows::EXPORT_AT)
        && called.call.key.is_none()
    {
        let mut bytes = [0; 32];
        for chunk in bytes.chunks_mut(16) {
            let hex = crate::jig::digest::Digest::from_bytes(runner.names.next_u64().to_le_bytes()).hex();
            for (slot, byte) in chunk.iter_mut().zip(hex) {
                *slot = byte;
            }
        }
        called.call.key = Some(crate::jig::key::Name::new(&bytes).expect("drawn hexadecimal name"));
    }
}
