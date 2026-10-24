//! A task's header, specification, supplied subtree and latest committed activity.
use crate::{
    frame::Bindings,
    look,
    words::{Times, Word, Words},
};
use alloc::boxed::Box;
use jig_client::{Latest, contract as c};
use skein_lib::{List, bytes::copy_of};
use skein_ui::{Builder, Role, Run, phrases::Hole};
use temper_client_domain::PlanFilter;
use temper_client_domain::{
    Address, Domain,
    pages::{
        Act,
        delegation::{self, Kind, State as ResultState},
        task::{self, Shape, State},
    },
};
fn text(b: &mut Builder, role: Role, value: &[u8]) {
    b.open(role);
    b.name(value);
    b.text(Run::Plain, value);
    b.close();
}
fn region(b: &mut Builder, word: Word, words: &Words) {
    b.open(Role::Region);
    b.name(&words.say(word, &[]));
    text(b, Role::Heading { level: 2 }, &words.say(word, &[]));
}
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
    times: &mut Times,
) {
    if let Some(page) = root.pages().task()
        && page.shape == Shape::Agent
        && page.document(root.client()).is_some()
    {
        super::agent::main(b, root, words, bindings, rows, times);
        return;
    }
    b.open(Role::Main);
    b.style(look::MAIN);
    b.style(look::PAGE);
    if let Some(page) = root.pages().task()
        && page.document(root.client()).is_none()
    {
        reading(b, page, words);
    }
    if let Some(page) = root.pages().task()
        && let Some(task) = page.document(root.client())
    {
        b.open(Role::Slot);
        b.style(look::PAGEBAR);
        if let c::OwnerRequester::Task(number) = task.requester {
            task_link(b, number, words, bindings);
        }
        text(b, Role::Paragraph, &words.say(Word::Task, &[Hole::Number(task.chip.task)]));
        b.close();
        let panel = temper_client_domain::forge::change::for_task(root.forge().panel(), task.chip.task);
        let change = super::change::shown(panel);
        header(b, task, root.pages().me(root.client()), root.forge().demo(), words, bindings, panel);
        if let Some(change) = change {
            // Word capacity prices the full pull address and label; Limits::of
            // guarantees at least this text bound for every view preset.
            let text_bound = words.capacity().expect("checked text capacity");
            super::change::toolbar(b, change, words, text_bound);
        }
        if page.shape == Shape::Tracked {
            if root.pages().confirmation().is_none()
                && match &page.amending {
                    Some(open) => open.done,
                    None => true,
                }
                && (jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Amend).is_some()
                    || jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Stop).is_some()
                    || jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Cancel).is_some())
            {
                b.open(Role::Slot);
                b.style(look::TASK_ACTS);
                act(b, task, c::ActKind::Amend, Word::Amend, Act::Amend, words, bindings);
                act(b, task, c::ActKind::Stop, Word::Stop, Act::Stop, words, bindings);
                act(b, task, c::ActKind::Cancel, Word::Cancel, Act::Close, words, bindings);
                b.close();
            }
            amendment(b, root, words, bindings);
            outcome(b, root, words);
        }
        controls(b, &task.chip, root, words, bindings, rows);
        crate::choice::composer(b, root, Some(task), words, bindings, times);
        if page.shape == Shape::Tracked {
            asked(b, task, words);
            result(b, task, words, look::TASK_CARD);
        }
        if let Some(change) = change {
            super::change::start(b, change, words, bindings);
        }
        let part = Plan::of(page, task);
        let tree = if let Some(document) = root.client().document(page.tree)
            && let Some(Latest::Jig(tree)) = document.latest
        {
            Some(tree)
        } else {
            None
        };
        if tree.is_some() || part.ended.is_some() {
            plan(b, tree, root, words, bindings, rows, part);
        }
        if let Some(change) = change {
            super::change::finish(b, change, words);
        }
        if page.shape == Shape::Tracked {
            latest(b, page.latest.as_deref(), words, bindings);
            budget(b, task, root.pages().me(root.client()), words, bindings);
        }
    }
    crate::confirm::main(b, root, words, bindings, rows);
    b.close();
}
pub(crate) fn reading(b: &mut Builder, page: &task::Task, words: &Words) {
    use task::{DocumentRead, ReadEnd};
    let word = match page.document_read {
        DocumentRead::Ended { why: ReadEnd::Unknown, .. } => Word::TaskUnknown,
        DocumentRead::Ended { why: ReadEnd::NotAllowed, .. } => Word::TaskNotAllowed,
        DocumentRead::Ended { why: ReadEnd::SignedOut, .. } => Word::TaskSignedOut,
        DocumentRead::Ended { why: ReadEnd::NotServed, .. } => Word::TaskNotServed,
        DocumentRead::Ended { why: ReadEnd::Gone, .. } => Word::TaskGone,
        DocumentRead::Ended { why: ReadEnd::TooLarge, .. } => Word::TaskTooLarge,
        DocumentRead::Ended { why: ReadEnd::Malformed, .. } => Word::TaskMalformed,
        DocumentRead::Reading
        | DocumentRead::Ready
        | DocumentRead::Ended {
            why: ReadEnd::Busy | ReadEnd::Unreachable | ReadEnd::Limit | ReadEnd::SignIn, ..
        } => Word::ReadingTask,
    };
    let holes = if word == Word::ReadingTask { &[Hole::Number(page.number)][..] } else { &[] };
    text(b, Role::Status, &words.say(word, holes));
}
/// An offered act by its admission (client/README.md, 5.11).
pub(crate) fn act(
    b: &mut Builder,
    task: &c::Task,
    kind: c::ActKind,
    word: Word,
    act: Act,
    words: &Words,
    bindings: &mut Bindings,
) {
    let Some(admission) = jig_client::facts::offer(task.acts.as_slice(), kind) else { return };
    match admission {
        c::Admission::Allowed | c::Admission::Bounded { .. } => {
            b.open(Role::Button);
            if kind == c::ActKind::Cancel {
                b.style(look::TASK_DANGER);
            }
            b.name(&words.say(word, &[]));
            b.bind(bindings.insert(act));
            b.close();
        }
        c::Admission::Proposed { to } => {
            text(b, Role::Paragraph, &words.say(word, &[]));
            words.card_text(b, crate::cards::P::ProposeInstead, &[]);
            crate::cards::recipient(b, words, to);
        }
        c::Admission::Refused(refusal) => {
            text(b, Role::Paragraph, &words.say(word, &[]));
            words.refusal(b, refusal, 0);
        }
    }
}
pub(crate) fn outcome(b: &mut Builder, root: &Domain, words: &Words) {
    let Some(page) = root.pages().task() else { return };
    for (key, kind) in &page.pending {
        if *kind != task::Sending::Amend
            && let Some(pending) = root.client().pending(*key)
        {
            crate::confirm::progress_text(b, words, pending.progress);
        }
    }
    if let Some(outcome) = &page.outcome {
        super::chats::outcome_text(b, words, outcome, 0);
    }
}
fn amendment_outcome(b: &mut Builder, root: &Domain, words: &Words, bindings: &mut Bindings) {
    let Some(page) = root.pages().task() else { return };
    let Some(open) = &page.amending else { return };
    for (key, kind) in &page.pending {
        if *kind == task::Sending::Amend
            && let Some(pending) = root.client().pending(*key)
        {
            crate::confirm::progress_text(b, words, pending.progress);
        }
    }
    if let Some(outcome) = &open.outcome {
        super::chats::outcome_text(b, words, outcome, 0);
        if let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::Moved { .. },
            ..
        })) = outcome
        {
            button(b, Word::LookAgain, Act::AmendAgain, words, bindings);
        }
    }
}

fn button(b: &mut Builder, word: Word, act: Act, words: &Words, bindings: &mut Bindings) {
    b.open(Role::Button);
    b.name(&words.say(word, &[]));
    b.bind(bindings.insert(act));
    b.close();
}
/// The page's two amendment controls (client/README.md, 5.11).
pub(crate) fn amendment(b: &mut Builder, root: &Domain, words: &Words, bindings: &mut Bindings) {
    let Some(page) = root.pages().task() else { return };
    let Some(open) = &page.amending else { return };
    let Some(task) = page.document(root.client()) else { return };
    b.open(Role::Region);
    b.style(look::TASK_AMEND);
    let title = words.say(Word::AmendTask, &[Hole::Number(page.number)]);
    b.name(&title);
    text(b, Role::Heading { level: 2 }, &title);
    if open.done {
        amendment_outcome(b, root, words, bindings);
        b.close();
        return;
    }
    if open.cut {
        text(b, Role::Paragraph, &words.say(Word::SpecTooLong, &[]));
    } else {
        // The text area's name is its label too, drawn where a person sees it.
        text(b, Role::Paragraph, &words.say(Word::Specification, &[]));
        b.open(Role::TextArea);
        b.name(&words.say(Word::Specification, &[]));
        b.value(&open.words, 0);
        b.bind(bindings.insert(Act::AmendWords { words: Box::default() }));
        b.close();
    }
    if let Some(tracking) = open.tracking {
        b.open(Role::Check);
        b.name(&words.say(Word::Tracked, &[]));
        b.bind(bindings.insert(Act::AmendTracked));
        if let c::Tracking::Tracked { .. } = tracking {
            b.state(skein_ui::State::Pressed);
        }
        b.close();
        b.open(Role::TextField);
        b.name(&words.say(Word::AmendPriority, &[]));
        let priority = match tracking {
            c::Tracking::Tracked { priority } => priority,
            c::Tracking::Untracked => 0,
        };
        b.value(skein_lib::Decimal::of(u64::from(priority)).as_bytes(), 0);
        b.bind(bindings.insert(Act::AmendPriority { priority }));
        if tracking == c::Tracking::Untracked {
            b.state(skein_ui::State::Disabled);
        }
        b.close();
    }
    if open.no_change {
        text(b, Role::Status, &words.say(Word::NoChange, &[]));
    }
    if open.draft.moved(task) {
        text(b, Role::Status, &words.say(Word::TaskChanged, &[]));
    } else {
        act(b, task, c::ActKind::Amend, Word::SendAmendment, Act::SendAmendment, words, bindings);
    }
    button(b, Word::Discard, Act::DiscardAmendment, words, bindings);
    amendment_outcome(b, root, words, bindings);
    b.close();
}
pub(crate) fn header(
    b: &mut Builder,
    task: &c::Task,
    me: Option<&c::Me>,
    demo: Option<&temper_client_domain::forge::Demo>,
    words: &Words,
    bindings: &mut Bindings,
    panel: Option<&temper_client_domain::forge::panel::Panel>,
) {
    let panel = temper_client_domain::forge::change::for_task(panel, task.chip.task);
    let change = super::change::shown(panel);
    region(b, Word::TaskHeader, words);
    b.style(look::TASK_HEADING);
    b.open(Role::Slot);
    b.style(look::META);
    if change.is_none()
        && let Some(priority) = task.priority
    {
        text(b, Role::Paragraph, &words.say(Word::GoalPriority, &[Hole::Number(u64::from(priority))]));
    }
    if change.is_some() {
        b.open(Role::Paragraph);
        b.style(look::CHANGE_TAG);
        b.text(
            Run::Plain,
            &super::change::say(
                words,
                temper_client_domain::forge::words::ChangeWord::Tag,
                &[Hole::Number(task.chip.task)],
            ),
        );
        b.close();
    } else {
        b.open(Role::Slot);
        text(b, Role::Paragraph, &words.say(Word::Task, &[Hole::Number(task.chip.task)]));
        executor(b, task.chip.executor, words);
        b.close();
    }
    let project = match super::project_name(me, demo, task.project, words) {
        Some(name) => name,
        None => words.say(Word::Project, &[Hole::Number(u64::from(task.project))]),
    };
    text(b, Role::Paragraph, &project);
    if let Some(panel) = panel {
        if let Some(word) = temper_client_domain::forge::change::unavailable(&panel.state) {
            text(b, Role::Paragraph, &super::change::say(words, word, &[]));
        }
    } else if let c::ChipPhase::Owner { details, .. } = task.chip.details.as_ref()
        && let Some(part) = &details.procedure
    {
        crate::forge::owner_part(b, part, words);
    }
    if let c::Executor::Procedure { .. } = task.chip.executor
        && let Some(goal) = task.goal
        && goal != task.chip.task
    {
        b.open(Role::Slot);
        text(b, Role::Paragraph, &words.say(Word::Goal, &[]));
        task_link(b, goal, words, bindings);
        b.close();
    }
    b.close();
    text(b, Role::Heading { level: 1 }, task.chip.title.bytes());
    b.open(Role::Slot);
    b.style(look::META);
    state(b, task::header_state(task, me), words);
    requester(b, task.requester, me, words);
    b.close();
    if let Some(change) = change {
        super::change::pull(b, change, words);
    }
    b.close();
}

pub(crate) fn executor(b: &mut Builder, executor: c::Executor, words: &Words) {
    let word = match executor {
        c::Executor::Agent { .. } => Word::Agent,
        c::Executor::Procedure { .. } => Word::Procedure,
        c::Executor::Party { .. } | c::Executor::Role { .. } => Word::PersonTask,
    };
    text(b, Role::Paragraph, &words.say(word, &[]));
}
pub(crate) fn requester(b: &mut Builder, by: c::OwnerRequester, me: Option<&c::Me>, words: &Words) {
    let (word, number) = match by {
        c::OwnerRequester::Person(number) => {
            if let Some(me) = me
                && me.party.number == number
            {
                text(b, Role::Paragraph, &words.say(Word::AskedBy, &[Hole::Name(copy_of(me.party.name.bytes()))]));
                return;
            }
            (Word::AskedByPerson, number)
        }
        c::OwnerRequester::Task(number) => (Word::AskedByTask, number),
        c::OwnerRequester::Deployment { project } => (Word::AskedByDeployment, u64::from(project)),
    };
    text(b, Role::Paragraph, &words.say(word, &[Hole::Number(number)]));
}
pub(crate) fn state(b: &mut Builder, state: State, words: &Words) {
    let (name, token) = match state {
        State::Phase(phase) => (phase_word(phase, words), phase_token(phase)),
        State::WaitingFor(number) => (words.say(Word::WaitingForTask, &[Hole::Number(number)]), look::STATUS_WAITING),
        State::WaitingForTasks { count, more } => (
            words.say(
                if more { Word::WaitingForMoreTasks } else { Word::WaitingForTasks },
                &[Hole::Number(u64::from(count))],
            ),
            look::STATUS_WAITING,
        ),
        State::WaitingForHoldings => (words.say(Word::WaitingForHoldings, &[]), look::STATUS_WAITING),
        State::WaitingForDelegates(count) => {
            (words.say(Word::WaitingForDelegates, &[Hole::Number(u64::from(count))]), look::STATUS_WAITING)
        }
        State::WaitingForYou => (words.say(Word::WaitingForYou, &[]), look::STATUS_WAITING),
        State::WaitingForParty(whom) => {
            let (word, number) = match whom {
                c::Whom::Person(person) => (Word::WaitingForPerson, person),
                c::Whom::Role { role, .. } => (Word::WaitingForRole, u64::from(role)),
            };
            (words.say(word, &[Hole::Number(number)]), look::STATUS_WAITING)
        }
        State::WaitingForNews => (words.say(Word::WaitingForNews, &[]), look::STATUS_WAITING),
        State::Running => (words.say(Word::Running, &[]), look::STATUS_ACTIVE),
        State::Writing => (words.say(Word::Writing, &[]), look::STATUS_ACTIVE),
        State::Tool(tool) => (words.say(Word::RunningTool, &[Hole::Name(tool)]), look::STATUS_ACTIVE),
        State::Ended(status) => {
            let phase = match status {
                c::OwnerTaskStatus::Done => c::Phase::Done,
                c::OwnerTaskStatus::Failed => c::Phase::Failed,
                c::OwnerTaskStatus::Cancelled => c::Phase::Cancelled,
            };
            (words.say_word(jig_client::words::Word::Phase(phase), &[]), phase_token(phase))
        }
    };
    b.open(Role::Status);
    b.style(token);
    b.name(&name);
    b.text(Run::Plain, &name);
    b.close();
}
// jig's word for a live phase in lower case (5.8); an ended one keeps 5.7's.
fn phase_word(phase: c::Phase, words: &Words) -> Box<[u8]> {
    let word = match phase {
        c::Phase::Waiting => Word::PhaseWaiting,
        c::Phase::Idle => Word::PhaseIdle,
        c::Phase::Due => Word::PhaseDue,
        c::Phase::Preparing => Word::PhasePreparing,
        c::Phase::Claimed => Word::PhaseClaimed,
        c::Phase::Running => Word::Running,
        c::Phase::Active => Word::PhaseActive,
        c::Phase::BackingOff => Word::PhaseRetrying,
        c::Phase::Closing => Word::PhaseClosing,
        c::Phase::Held => Word::PhaseHeld,
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => {
            return words.say_word(jig_client::words::Word::Phase(phase), &[]);
        }
    };
    words.say(word, &[])
}
fn phase_token(phase: c::Phase) -> skein_ui::Style {
    match phase {
        c::Phase::Due | c::Phase::Preparing | c::Phase::Claimed | c::Phase::Running | c::Phase::Active => {
            look::STATUS_ACTIVE
        }
        c::Phase::Done => look::STATUS_DONE,
        c::Phase::Held | c::Phase::Failed => look::STATUS_HELD,
        c::Phase::BackingOff => look::STATUS_RETRYING,
        c::Phase::Waiting | c::Phase::Idle => look::STATUS_WAITING,
        c::Phase::Closing | c::Phase::Ended | c::Phase::Cancelled => look::STATUS_QUIET,
    }
}
pub(crate) fn asked(b: &mut Builder, task: &c::Task, words: &Words) {
    region(b, Word::Asked, words);
    b.style(look::TASK_CARD);
    b.open(Role::Paragraph);
    b.style(look::TASK_CONTRACT);
    let kind = match delegation::facts(task).kind {
        Kind::Report => Word::GoalReport,
        Kind::Change => Word::GoalChange,
        Kind::Verdict => Word::GoalVerdict,
        Kind::Approval => Word::GoalApproval,
    };
    let name = words.say(kind, &[]);
    b.name(&name);
    b.text(Run::Plain, &name);
    b.close();
    skein_ui::markdown::read(task.spec.specification.words.bytes(), 3, b);
    let parts = !task.resources.is_empty()
        || match &task.context {
            Some(context) => !context.parts.is_empty(),
            None => false,
        };
    if parts {
        b.open(Role::Slot);
        b.style(look::META);
    }
    for resource in &task.resources {
        match resource {
            c::TaskResource::Part(part) => crate::forge::drawn(b, part, words),
            c::TaskResource::Owner(resource) => crate::forge::owner_part(b, &resource.part, words),
        }
    }
    if let Some(context) = &task.context {
        for part in &context.parts {
            crate::forge::drawn(b, part, words);
        }
    }
    if parts {
        b.close();
    }
    b.close();
}
pub(crate) fn result(b: &mut Builder, task: &c::Task, words: &Words, style: skein_ui::Style) {
    let facts = delegation::facts(task);
    match facts.state {
        ResultState::Running | ResultState::Held => return,
        ResultState::Done | ResultState::Landed | ResultState::Failed | ResultState::Cancelled | ResultState::Ended => {
        }
    }
    region(b, Word::GoalResult, words);
    b.style(style);
    crate::cards::delegation::status(b, facts.state, words);
    if let Some(said) = facts.said {
        crate::cards::delegation::said(b, said, words);
    }
    b.close();
}
pub(crate) fn budget(b: &mut Builder, task: &c::Task, me: Option<&c::Me>, words: &Words, bindings: &mut Bindings) {
    region(b, Word::BudgetAuthority, words);
    b.style(look::TASK_CARD);
    b.style(look::TASK_BUDGET);
    let budget = &task.budget;
    let authority = &task.authority.authority;
    text(
        b,
        Role::Paragraph,
        &words.say(
            Word::BudgetLeft,
            &[Hole::Number(budget.numbers.spent), Hole::Number(budget.left), Hole::Number(budget.numbers.budget)],
        ),
    );
    for (term, amount) in
        [(Word::DelegatesSpent, budget.numbers.spent_below), (Word::ReservedDelegates, budget.numbers.reserved)]
    {
        text(b, Role::Term, &words.say(term, &[]));
        text(b, Role::Description, &words.say(Word::Amount, &[Hole::Number(amount)]));
    }
    text(b, Role::Term, &words.say(Word::FundedBy, &[]));
    match budget.funder {
        c::Funder::Task(number) => {
            b.open(Role::Description);
            task_link(b, number, words, bindings);
            b.close();
        }
        c::Funder::Pool { person, .. } => {
            let mine = match me {
                Some(me) => me.party.number == person,
                None => false,
            };
            let name = if mine {
                words.say(Word::YourPool, &[])
            } else {
                words.say(Word::PersonPool, &[Hole::Number(person)])
            };
            text(b, Role::Description, &name);
        }
        c::Funder::Recurring { task, .. } => {
            text(b, Role::Description, &words.say(Word::RecurringBudget, &[Hole::Number(task)]));
        }
        c::Funder::Period { project, .. } => {
            text(b, Role::Description, &words.say(Word::ProjectBudget, &[Hole::Number(u64::from(project))]));
        }
    }
    text(b, Role::Term, &words.say(Word::BudgetDeadline, &[]));
    let deadline = match budget.deadline {
        Some(at) => words.deadline(at),
        None => words.say(Word::NoDeadline, &[]),
    };
    text(b, Role::Description, &deadline);
    text(b, Role::Term, &words.say(Word::MaySpend, &[]));
    text(b, Role::Description, &words.say(Word::Amount, &[Hole::Number(authority.spend)]));
    text(b, Role::Term, &words.say(Word::MayDelegate, &[]));
    let delegation = &authority.delegation;
    let bounds = if delegation.tasks == 1 {
        words.say(Word::DelegateOne, &[Hole::Number(u64::from(delegation.depth))])
    } else {
        words.say(
            Word::DelegateBounds,
            &[Hole::Number(u64::from(delegation.tasks)), Hole::Number(u64::from(delegation.depth))],
        )
    };
    text(b, Role::Description, &bounds);
    if let Some(models) = &authority.models {
        text(b, Role::Term, &words.say(Word::Models, &[]));
        match models {
            c::AuthorityModels::All => text(b, Role::Description, &words.say(Word::AllModels, &[])),
            c::AuthorityModels::Named(names) if names.is_empty() => {
                text(b, Role::Description, &words.say(Word::NoModels, &[]));
            }
            c::AuthorityModels::Named(names) => {
                for name in names {
                    text(b, Role::Description, &words.model_name(name));
                }
            }
        }
    }
    if task.authority.cut {
        text(b, Role::Paragraph, &words.say(Word::AuthorityCut, &[]));
    }
    b.close();
}
fn chip(b: &mut Builder, chip: &c::Chip, words: &Words, bindings: &mut Bindings) {
    b.open(Role::TreeItem);
    b.name(&words.say(Word::Task, &[Hole::Number(chip.task)]));
    b.open(Role::Slot);
    b.style(look::PLAN_ROW);
    b.open(Role::Slot);
    task_link(b, chip.task, words, bindings);
    text(b, Role::Paragraph, chip.title.bytes());
    b.close();
    executor(b, chip.executor, words);
    state(b, task::row_state(chip), words);
    if let c::ChipPhase::Owner { details, .. } = chip.details.as_ref() {
        b.open(Role::Paragraph);
        b.style(look::PLAN_SPEND);
        let name = words.say(Word::RowSpent, &[Hole::Number(details.budget.numbers.spent)]);
        b.name(&name);
        b.text(Run::Plain, &name);
        b.close();
    }
    if let c::ChipPhase::Owner { details, .. } = chip.details.as_ref()
        && let Some(part) = &details.procedure
    {
        crate::forge::owner_part(b, part, words);
    }
    b.close();
}
pub(crate) fn task_link(b: &mut Builder, task: u64, words: &Words, bindings: &mut Bindings) {
    let name = words.say(Word::Task, &[Hole::Number(task)]);
    crate::link(b, &name, bindings.insert(Act::Go { address: Address::Task { task, filter: PlanFilter::All } }));
    b.text(Run::Plain, &name);
    b.close();
}
fn parent(b: &mut Builder, stack: &mut List<u64>, parent: Option<u64>) {
    let mut keep = 0;
    for (index, number) in stack.as_slice().iter().enumerate() {
        if Some(*number) == parent {
            keep = index.checked_add(1).expect("tree stack fits");
        }
    }
    for _ in keep..stack.as_slice().len() {
        b.close();
    }
    stack.truncate(u32::try_from(keep).expect("tree stack fits"));
}
/// client/README.md, 5.8: keep each live root's delegates under it, in supplied
/// sibling order. Task lookup is O(log n); linking and the explicit walk are O(n).
fn tree_order(root: u64, rows: &[(u64, Option<u64>)]) -> List<u32> {
    let bound = u32::try_from(rows.len()).expect("tree rows fit");
    // `bound` is outside the window, and serves as the absent link.
    let mut index = List::with_capacity(bound);
    let mut links = List::with_capacity(bound);
    let mut order = List::with_capacity(bound);
    for (at, (task, _)) in rows.iter().enumerate() {
        let at = u32::try_from(at).expect("tree index fits");
        index.push((*task, at)).expect("tree index fits");
        links.push((bound, bound)).expect("tree links fit");
        if *task == root {
            // Legacy deliveries include the undrawn root and its folds.
            order.push(at).expect("tree order fits");
        }
    }
    let mut index = index.into_boxed();
    index.sort_unstable();
    let mut first = bound;
    // Prepending in reverse preserves the window's order at every parent.
    for (at, (task, parent)) in rows.iter().enumerate().rev() {
        if *task == root {
            continue;
        }
        let at = u32::try_from(at).expect("tree index fits");
        let mut parent_index = bound;
        if let Some(parent) = parent
            && *parent != root
        {
            let mut low = 0;
            let mut high = index.len();
            // Each step halves the index, as in EndedRows::mark.
            for _ in 0..usize::BITS {
                if low == high {
                    break;
                }
                let half = high.checked_sub(low).expect("ordered search bounds") / 2;
                let middle = low.checked_add(half).expect("tree index fits");
                let (number, position) = index.get(middle).expect("search index in tree");
                match number.cmp(parent) {
                    core::cmp::Ordering::Less => low = middle.checked_add(1).expect("tree index fits"),
                    core::cmp::Ordering::Greater => high = middle,
                    core::cmp::Ordering::Equal => {
                        parent_index = *position;
                        break;
                    }
                }
            }
        }
        let next = if parent_index == bound {
            core::mem::replace(&mut first, at)
        } else {
            let parent = links.get_mut(parent_index).expect("parent in tree");
            core::mem::replace(&mut parent.0, at)
        };
        links.get_mut(at).expect("row in tree").1 = next;
    }
    let mut stack = List::with_capacity(bound);
    if first != bound {
        stack.push(first).expect("tree walk fits");
    }
    // Each row is visited once; a sibling is deferred while its subtree is drawn.
    for _ in 0..bound {
        let Some(at) = stack.last().copied() else { break };
        stack.truncate(stack.len().checked_sub(1).expect("nonempty tree stack"));
        order.push(at).expect("tree order fits");
        let (child, sibling) = *links.get(at).expect("row in tree");
        if sibling != bound {
            stack.push(sibling).expect("tree walk fits");
        }
        if child != bound {
            stack.push(child).expect("tree walk fits");
        }
    }
    order
}
fn folded(b: &mut Builder, counts: &[(c::Phase, u32)], held_below: u32, words: &Words) -> bool {
    let mut shown = false;
    for (phase, count) in counts {
        if *count != 0 {
            shown = true;
            text(
                b,
                Role::Paragraph,
                &words.say(
                    Word::Folded,
                    &[
                        Hole::Number(u64::from(*count)),
                        Hole::Name(words.say_word(jig_client::words::Word::Phase(*phase), &[])),
                    ],
                ),
            );
        }
    }
    if held_below != 0 {
        text(b, Role::Status, &words.say(Word::HeldBelow, &[Hole::Number(u64::from(held_below))]));
        shown = true;
    }
    shown
}
pub(crate) struct Plan<'a> {
    pub word: Word,
    pub ended: Option<(&'a c::TaskPage, bool)>,
    pub goal: Option<(PlanFilter, u32)>,
}
impl<'a> Plan<'a> {
    fn of(page: &'a task::Task, task: &c::Task) -> Self {
        let word = match task.chip.executor {
            c::Executor::Procedure { .. } => Word::ItsTasks,
            c::Executor::Agent { .. } | c::Executor::Party { .. } | c::Executor::Role { .. } => Word::Plan,
        };
        let ended = match &page.ended {
            Some(task::Ended { page: Some(page), .. }) => Some((
                page.as_ref(),
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
                },
            )),
            Some(_) | None => None,
        };
        let goal = if word == Word::Plan && page.ended.is_some() {
            let mut live = 0;
            if let c::ChipPhase::Owner { details, .. } = task.chip.details.as_ref() {
                live = details.live_delegates;
            }
            Some((page.filter, live))
        } else {
            None
        };
        Self { word, ended, goal }
    }
    fn heading(&self, b: &mut Builder, words: &Words, bindings: &mut Bindings, ended_rows: &EndedRows<'_>) {
        region(b, self.word, words);
        b.style(look::TASK_CARD);
        if let Some((filter, live)) = self.goal {
            if self.ended.is_some() {
                text(
                    b,
                    Role::Paragraph,
                    &words.say(
                        if ended_rows.more { Word::PlanMoreCounts } else { Word::PlanCounts },
                        &[
                            Hole::Number(u64::from(live)),
                            Hole::Number(u64::try_from(ended_rows.lines.len()).expect("page fits")),
                        ],
                    ),
                );
            }
            b.open(Role::Slot);
            b.style(look::FILTERS);
            for (value, word) in [(PlanFilter::All, Word::AllPlan), (PlanFilter::Live, Word::LivePlan)] {
                b.open(Role::Button);
                b.name(&words.say(word, &[]));
                if filter == value {
                    b.state(skein_ui::State::Pressed);
                }
                b.bind(bindings.insert(Act::PlanFilter { filter: value }));
                b.close();
            }
            b.close();
        }
    }
}
fn owner_plan_row(
    b: &mut Builder,
    row: &c::OwnerTreeRow,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
) {
    chip(b, &row.chip, words, bindings);
    let facts = if let c::ChipPhase::Owner { details, .. } = row.chip.details.as_ref() {
        details.dependencies.as_slice()
    } else {
        &[]
    };
    dependencies(b, row.chip.task, &row.dependencies, facts, row.cut, root.pages().me(root.client()), words, bindings);
    controls(b, &row.chip, root, words, bindings, rows);
}
pub(crate) fn plan(
    b: &mut Builder,
    tree: Option<&c::Delivery>,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
    part: Plan<'_>,
) {
    let mut ended_rows = EndedRows::new(part.ended, tree);
    part.heading(b, words, bindings, &ended_rows);
    let Plan { word, ended, goal } = part;
    if let Some((PlanFilter::Live, _)) = goal {
        ended_rows.lines = &[];
        ended_rows.more = false;
    }
    b.open(Role::Tree);
    b.style(look::PLAN);
    b.name(&words.say(word, &[]));
    let mut has_tree_content = false;
    match tree {
        Some(c::Delivery::OwnerTree(tree)) => {
            has_tree_content = !tree.rows.is_empty();
            let mut stack = List::with_capacity(tree.rows.len());
            has_tree_content |= owner_folds(b, tree, tree.root.task, words);
            let mut rows_by_parent = List::with_capacity(tree.rows.len());
            for row in &tree.rows {
                rows_by_parent.push((row.chip.task, Some(row.parent))).expect("tree rows fit");
            }
            let order = tree_order(tree.root.task, rows_by_parent.as_slice());
            for index in &order {
                let row = tree.rows.get(*index).expect("ordered tree row");
                if row.parent == tree.root.task && ended_rows.precedes(row.chip.task) {
                    parent(b, &mut stack, None);
                    has_tree_content |= ended_rows.before(b, Some(row.chip.task), root, words, bindings, rows);
                }
                parent(b, &mut stack, Some(row.parent));
                owner_plan_row(b, row, root, words, bindings, rows);
                stack.push(row.chip.task).expect("tree stack fits");
                has_tree_content |= owner_folds(b, tree, row.chip.task, words);
            }
            parent(b, &mut stack, None);
            has_tree_content |= folded(b, &[], tree.held_below, words);
        }
        Some(c::Delivery::Tree(tree)) => {
            let mut stack = List::with_capacity(tree.nodes.len());
            let mut rows_by_parent = List::with_capacity(tree.nodes.len());
            for node in &tree.nodes {
                rows_by_parent.push((node.chip.task, node.parent)).expect("tree rows fit");
            }
            let order = tree_order(tree.task, rows_by_parent.as_slice());
            for index in &order {
                let node = tree.nodes.get(*index).expect("ordered tree node");
                if node.parent == Some(tree.task) && ended_rows.precedes(node.chip.task) {
                    parent(b, &mut stack, None);
                    has_tree_content |= ended_rows.before(b, Some(node.chip.task), root, words, bindings, rows);
                }
                parent(b, &mut stack, node.parent);
                if node.chip.task != tree.task {
                    has_tree_content = true;
                    chip(b, &node.chip, words, bindings);
                    controls(b, &node.chip, root, words, bindings, rows);
                    stack.push(node.chip.task).expect("tree stack fits");
                }
                has_tree_content |= counts(b, node.folded, node.held_below, words);
            }
            parent(b, &mut stack, None);
            has_tree_content |= counts(b, tree.folded, tree.held_below, words);
        }
        Some(
            c::Delivery::OwnerProject(_)
            | c::Delivery::Me(_)
            | c::Delivery::Task(_)
            | c::Delivery::OwnerProposal(_)
            | c::Delivery::Chips(_)
            | c::Delivery::Inbox(_)
            | c::Delivery::OwnerInbox(_)
            | c::Delivery::Tasks(_)
            | c::Delivery::Run(_)
            | c::Delivery::RunState(_)
            | c::Delivery::Settings(_)
            | c::Delivery::Deployment(_)
            | c::Delivery::OwnerDeployment(_)
            | c::Delivery::OwnerAccounts(_),
        )
        | None => {}
    }
    has_tree_content |= ended_rows.before(b, None, root, words, bindings, rows);
    b.close();
    if !has_tree_content {
        let word = match (ended, goal) {
            (_, Some((PlanFilter::Live, _))) | (None, _) => Word::NoLiveTasks,
            (Some((_, true)), _) => Word::ItsTasksNone,
            (Some((_, false)), _) => Word::ItsTasksNoneYet,
        };
        text(b, Role::Paragraph, &words.say(word, &[]));
    }
    if ended_rows.more {
        text(b, Role::Paragraph, &words.say(Word::ItsTasksMoreEnded, &[]));
    }
    b.close();
}
/// One cursor through the number-ordered page, with tree matches marked first.
/// Marking before drawing also handles legacy trees whose roots arrive out of order.
struct EndedRows<'a> {
    lines: &'a [c::TaskLine],
    shown: List<bool>,
    next: usize,
    more: bool,
}
impl<'a> EndedRows<'a> {
    fn new(ended: Option<(&'a c::TaskPage, bool)>, tree: Option<&c::Delivery>) -> Self {
        let (lines, more) = match ended {
            Some((c::TaskPage { ended: c::EndedGroup::Rows { rows, after, cut }, .. }, _)) => {
                (rows.as_slice(), after.is_some() || *cut)
            }
            Some((c::TaskPage { ended: c::EndedGroup::None, .. }, _)) | None => (&[][..], false),
        };
        let mut result =
            Self { lines, shown: List::with_capacity(u32::try_from(lines.len()).expect("page fits")), next: 0, more };
        for _ in lines {
            result.shown.push(false).expect("page matches fit");
        }
        match tree {
            Some(c::Delivery::OwnerTree(tree)) => {
                for row in &tree.rows {
                    result.mark(row.chip.task);
                }
            }
            Some(c::Delivery::Tree(tree)) => {
                for node in &tree.nodes {
                    if node.chip.task != tree.task {
                        result.mark(node.chip.task);
                    }
                }
            }
            Some(
                c::Delivery::OwnerProject(_)
                | c::Delivery::Me(_)
                | c::Delivery::Task(_)
                | c::Delivery::OwnerProposal(_)
                | c::Delivery::Chips(_)
                | c::Delivery::Inbox(_)
                | c::Delivery::OwnerInbox(_)
                | c::Delivery::Tasks(_)
                | c::Delivery::Run(_)
                | c::Delivery::RunState(_)
                | c::Delivery::Settings(_)
                | c::Delivery::Deployment(_)
                | c::Delivery::OwnerDeployment(_)
                | c::Delivery::OwnerAccounts(_),
            )
            | None => {}
        }
        result
    }
    fn mark(&mut self, number: u64) {
        let mut low = 0;
        let mut high = self.lines.len();
        // Each step halves the page; even the largest slice fits in usize::BITS steps.
        for _ in 0..usize::BITS {
            if low == high {
                break;
            }
            let half = high.checked_sub(low).expect("ordered search bounds") / 2;
            let middle = low.checked_add(half).expect("page index fits");
            match self.lines.get(middle).expect("search index in page").chip.task.cmp(&number) {
                core::cmp::Ordering::Less => low = middle.checked_add(1).expect("page index fits"),
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => {
                    *self.shown.get_mut(u32::try_from(middle).expect("page index fits")).expect("page match") = true;
                    break;
                }
            }
        }
    }
    fn precedes(&self, number: u64) -> bool {
        match self.lines.get(self.next) {
            Some(line) => line.chip.task < number,
            None => false,
        }
    }
    fn before(
        &mut self,
        b: &mut Builder,
        number: Option<u64>,
        root: &Domain,
        words: &Words,
        bindings: &mut Bindings,
        rows: u32,
    ) -> bool {
        let mut drew = false;
        for index in self.next..self.lines.len() {
            let line = self.lines.get(index).expect("page index");
            if let Some(number) = number
                && line.chip.task >= number
            {
                break;
            }
            self.next = index.checked_add(1).expect("page index fits");
            if !self.shown.as_slice().get(index).expect("page match index") {
                chip(b, &line.chip, words, bindings);
                b.style(look::PLAN_DONE);
                controls(b, &line.chip, root, words, bindings, rows);
                b.close();
                drew = true;
            }
        }
        drew
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the supplied dependency line and its facts share the viewer and rendering context"
)]
pub(crate) fn dependencies(
    b: &mut Builder,
    task: u64,
    links: &List<c::OwnerTaskLink>,
    facts: &[c::Dependency],
    cut: bool,
    me: Option<&c::Me>,
    words: &Words,
    bindings: &mut Bindings,
) {
    if links.is_empty() && !cut {
        return;
    }
    b.open(Role::List);
    b.name(&words.say(Word::Dependencies, &[Hole::Number(task)]));
    b.style(look::DEPENDENCIES);
    for link in links {
        b.open(Role::Item);
        b.key(super::inbox::card_key(c::ObjectKey::Task(link.number())));
        task_link(b, link.number(), words, bindings);
        if let Some(value) = task::link_state(link) {
            state(b, value, words);
        }
        for fact in facts {
            if fact.number == link.number() {
                dependency_mark(b, task, fact, me, words);
                break;
            }
        }
        b.close();
    }
    if cut {
        b.open(Role::Item);
        text(b, Role::Paragraph, &words.say(Word::DependenciesCut, &[]));
        b.close();
    }
    b.close();
}
fn dependency_mark(b: &mut Builder, task: u64, fact: &c::Dependency, me: Option<&c::Me>, words: &Words) {
    let Some(added) = fact.added else { return };
    let name = match added.by {
        c::OwnerRequester::Person(number) => {
            if let Some(me) = me
                && me.party.number == number
            {
                words.say(Word::DependencyAddedByYou, &[])
            } else {
                words.say(Word::DependencyAddedByPerson, &[Hole::Number(number)])
            }
        }
        c::OwnerRequester::Task(number) => words.say(Word::DependencyAddedByTask, &[Hole::Number(number)]),
        c::OwnerRequester::Deployment { .. } => words.say(Word::DependencyAddedByDeployment, &[]),
    };
    let name =
        if fact.keeps { words.say(Word::DependencyKeeps, &[Hole::Name(name), Hole::Number(task)]) } else { name };
    text(b, Role::Paragraph, &name);
}
pub(crate) fn controls(
    b: &mut Builder,
    chip: &c::Chip,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
) {
    if let Some(page) = root.pages().task() {
        for card in &page.cards {
            if card.object == c::ObjectKey::Task(chip.task) && (chip.phase == c::Phase::Held || card.key.is_some()) {
                b.open(Role::List);
                b.style(look::PLAN_HOLD);
                b.open(Role::Item);
                b.key(super::inbox::card_key(c::ObjectKey::Escalation(chip.task)));
                crate::confirm::card(b, root, card, words, bindings, rows);
                b.close();
                b.close();
            }
        }
    }
}
fn owner_folds(b: &mut Builder, tree: &c::OwnerTree, parent: u64, words: &Words) -> bool {
    use c::Phase as P;
    let mut shown = false;
    for fold in &tree.folded {
        if fold.parent == parent {
            let f = fold.phases;
            shown |= folded(
                b,
                &[
                    (P::Waiting, f.waiting),
                    (P::Idle, f.idle),
                    (P::Due, f.starting),
                    (P::Running, f.running),
                    (P::BackingOff, f.backing_off),
                    (P::Closing, f.closing),
                    (P::Held, f.held),
                    (P::Done, f.done),
                    (P::Failed, f.failed),
                    (P::Cancelled, f.cancelled),
                ],
                fold.held_below,
                words,
            );
        }
    }
    shown
}
fn counts(b: &mut Builder, f: c::PhaseCounts, held: u32, words: &Words) -> bool {
    use c::Phase as P;
    folded(
        b,
        &[
            (P::Ended, f.ended),
            (P::Waiting, f.waiting),
            (P::Running, f.running),
            (P::Held, f.held),
            (P::Closing, f.closing),
            (P::Done, f.done),
            (P::Failed, f.failed),
            (P::Cancelled, f.cancelled),
        ],
        held,
        words,
    )
}
pub(crate) fn latest(b: &mut Builder, page: Option<&c::ActivityPage>, words: &Words, bindings: &mut Bindings) {
    region(b, Word::Latest, words);
    b.style(look::TASK_CARD);
    if let Some(page) = page
        && !page.rows.is_empty()
    {
        b.open(Role::List);
        b.style(look::EVENTS);
        for row in &page.rows {
            b.open(Role::Item);
            text(b, Role::Paragraph, &words.say(Word::ActivityAt, &[Hole::Wall(row.at)]));
            b.open(Role::Slot);
            activity(b, &row.event, words, bindings);
            b.close();
            b.close();
        }
        b.close();
    }
    b.close();
}
fn activity(b: &mut Builder, event: &c::Activity, words: &Words, bindings: &mut Bindings) {
    if let Some((_, text)) = event.word_message() {
        text_node(b, text.bytes());
        return;
    }
    if let Some((_, turn)) = event.decoded_turn() {
        for message in &turn.messages {
            for block in &message.blocks {
                match block {
                    c::Block::Text(text) => text_node(b, text.bytes()),
                    c::Block::Call(call) => {
                        text_node(b, call.tool.bytes());
                        if let Some(text) = &call.outcome {
                            text_node(b, text.bytes());
                        }
                    }
                    c::Block::Result { summary, .. } => text_node(b, summary.bytes()),
                    c::Block::Opaque { size, .. } => {
                        text(b, Role::Paragraph, &words.say(Word::Opaque, &[Hole::Number(*size)]));
                    }
                }
            }
        }
        return;
    }
    match event {
        c::Activity::RunBegan { attempt, .. } => {
            text(b, Role::Paragraph, &words.say(Word::RunBegan, &[Hole::Number(*attempt)]));
        }
        c::Activity::RunEnded { attempt, .. } => {
            text(b, Role::Paragraph, &words.say(Word::RunEnded, &[Hole::Number(*attempt)]));
        }
        c::Activity::Owner { source, .. } => {
            if let c::OwnerActivity::Heard(c::OwnerWord {
                from: c::OwnerRequester::Task(task),
                kind: c::OwnerMessageKind::Result(_),
                ..
            }) = &source.line
            {
                task_link(b, *task, words, bindings);
            }
            owner_activity(b, &source.line, words);
        }
        c::Activity::News { summary, part, .. } => {
            text_node(b, summary.bytes());
            if let Some(part) = part {
                crate::forge::drawn(b, part, words);
            }
        }
        c::Activity::Result { result } => {
            task_link(b, result.task.task, words, bindings);
            text_node(b, result.task.title.bytes());
        }
        c::Activity::Decision { .. } => words.text(b, Word::DecisionMade, &[]),
        c::Activity::Turn { .. } => words.text(b, Word::UnreadTurn, &[]),
        c::Activity::Words { .. } => {}
    }
}
fn text_node(b: &mut Builder, value: &[u8]) {
    skein_ui::markdown::read(value, 3, b);
}
pub(crate) fn owner_activity(b: &mut Builder, event: &c::OwnerActivity, words: &Words) {
    match event {
        c::OwnerActivity::Words { words, .. } => text_node(b, words.bytes()),
        c::OwnerActivity::Heard(word) | c::OwnerActivity::Notified { word, .. } => text_node(b, word.words.bytes()),
        c::OwnerActivity::RunBegan { attempt, .. } => words.text(b, Word::RunBegan, &[Hole::Number(*attempt)]),
        c::OwnerActivity::RunEnded { attempt, .. } => words.text(b, Word::RunEnded, &[Hole::Number(*attempt)]),
        c::OwnerActivity::Decided { .. } => words.text(b, Word::DecisionMade, &[]),
        c::OwnerActivity::Turn { .. } => words.text(b, Word::UnreadTurn, &[]),
        c::OwnerActivity::Kept { .. } => words.text(b, Word::DependencyParked, &[]),
        c::OwnerActivity::Milestone { position } => words.text(b, Word::Milestone, &[Hole::Number(*position)]),
    }
}
