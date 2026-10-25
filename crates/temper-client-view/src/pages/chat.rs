//! The chat follows jig-client's thread in chronological order.
use crate::{cards::P, look};
use crate::{
    frame::Bindings,
    words::{TimeForm, Times, Word, Words},
};
use alloc::boxed::Box;
use jig_client::{
    contract as c,
    thread::{Line, Thread, Words as WordState},
};
use skein_lib::List;
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::{
    Domain,
    pages::{Act, chat::Chat, delegation},
};
fn text(b: &mut Builder, role: Role, value: &[u8]) {
    b.open(role);
    b.name(value);
    b.text(Run::Plain, value);
    b.close();
}
fn control(b: &mut Builder, bindings: &mut Bindings, role: Role, name: &[u8], act: Act) {
    b.open(role);
    b.name(name);
    b.bind(bindings.insert(act));
    b.close();
}
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
    text_limit: u32,
    times: &mut Times,
) {
    b.open(Role::Main);
    b.style(look::MAIN);
    b.style(look::CHAT);
    if let Some(chat) = root.pages().chat()
        && let Some(task) = chat.task(root.client())
    {
        let title = words.say(
            Word::ChatTitle,
            &[Hole::Number(chat.task), Hole::Name(skein_lib::bytes::copy_of(task.chip.title.bytes()))],
        );
        b.open(Role::Region);
        b.name(&title);
        b.style(look::CHATBAR);
        b.open(Role::Slot);
        heading(b, &title, task.chip.title.bytes());
        if let Some(result) = &task.result {
            ended_status(b, task, result, words, times);
        } else if let Some(me) = root.pages().me(root.client()) {
            super::chats::status(
                b,
                &task.chip,
                (&task.waits_on, Some(chat.waiting_delegates(root.client())), task.run_wait.as_ref()),
                (task.project, task.requester),
                Some(&task.wake),
                me,
                words,
            );
        }
        b.close();
        b.open(Role::Slot);
        spend(b, task, words);
        if temper_client_domain::pages::chat::allowed(task, c::ActKind::Stop) {
            control(b, bindings, Role::Button, &words.say(Word::Stop, &[]), Act::Stop);
        }
        if temper_client_domain::pages::chat::allowed(task, c::ActKind::Cancel) {
            control(b, bindings, Role::Button, &words.say(Word::Close, &[]), Act::Close);
        }
        b.close();
        b.close();
        context(b, root, task.project, words);
        b.open(Role::Slot);
        b.style(look::THREAD);
        if chat.offers_older() {
            control(b, bindings, Role::Button, &words.say(Word::Older, &[]), Act::Older);
        }
        let person = speakers(task, root.pages().me(root.client()));
        conversation_with_times(
            b,
            &chat.thread,
            words,
            (rows, text_limit),
            person,
            (Drawn::ChatPage { request: Some(&task.spec), root }, Some(times)),
            bindings,
        );
        let mut unanchored = false;
        for card in &chat.cards {
            if proposal_anchor(chat, card.object, rows).is_none() {
                unanchored = true;
            }
        }
        if unanchored || !chat.departures().is_empty() {
            b.open(Role::List);
            b.style(look::CARDS);
        }
        for card in &chat.cards {
            if proposal_anchor(chat, card.object, rows).is_none() {
                proposal_card(b, root, card, words, bindings, (rows, Some(times)), Role::Item);
            }
        }
        for object in chat.departures() {
            b.open(Role::Item);
            b.style(look::CARD);
            b.key(super::inbox::card_key(*object));
            words.status(b, P::CardLeft, &[]);
            b.close();
        }
        if unanchored || !chat.departures().is_empty() {
            b.close();
        }
        if let Some(result) = &task.result {
            let clock = end_clock(task, result, words, times);
            result_text(b, result, task.requester, words, clock.as_deref());
        }
        act_status(b, root, chat, words);
        b.close();
        composer(b, root, chat, task, words, bindings, times);
    }
    crate::confirm::main(b, root, words, bindings, rows);
    b.close();
}
fn heading(b: &mut Builder, title: &[u8], suffix: &[u8]) {
    b.open(Role::Heading { level: 1 });
    b.name(title);
    let title_at = title.len().checked_sub(suffix.len()).expect("title hole is a suffix");
    let prefix = title.get(..title_at).expect("title suffix is within the phrase");
    let separator_at = skein_lib::bytes::find(prefix, b" ").expect("chat heading separates number and title");
    for part in [
        prefix.get(..separator_at).expect("number precedes the separator"),
        prefix.get(separator_at..).expect("separator is within the prefix"),
        title.get(title_at..).expect("title suffix is within the phrase"),
    ] {
        b.open(Role::Text);
        b.name(part);
        b.run(Run::Plain, part);
        b.close();
    }
    b.close();
}

fn act_status(b: &mut Builder, root: &Domain, chat: &Chat, words: &Words) {
    for (key, kind) in &chat.pending {
        match kind {
            temper_client_domain::pages::chat::Sending::Message | temper_client_domain::pages::chat::Sending::Seen => {}
            temper_client_domain::pages::chat::Sending::Stop | temper_client_domain::pages::chat::Sending::Close => {
                if let Some(pending) = root.client().pending(*key) {
                    crate::confirm::progress_text(b, words, pending.progress);
                }
            }
        }
    }
    if chat.outcome_kind != Some(temper_client_domain::pages::chat::Sending::Message)
        && let Some(outcome) = &chat.outcome
    {
        super::chats::outcome_text(b, words, outcome, 0);
    }
}
/// The signed-in person and whether they requested this chat.
pub(crate) struct Speakers {
    pub(crate) person: Option<u64>,
    pub(crate) requester: bool,
}

pub(crate) fn speakers(task: &c::Task, me: Option<&c::Me>) -> Speakers {
    #[expect(clippy::manual_map, reason = "view code spells out the signed-in person")]
    let person = match me {
        Some(me) => Some(me.party.number),
        None => None,
    };
    let requester = match task.requester {
        c::OwnerRequester::Person(number) => Some(number) == person,
        c::OwnerRequester::Task(_) | c::OwnerRequester::Deployment { .. } => false,
    };
    Speakers { person, requester }
}

pub(crate) fn context(b: &mut Builder, root: &Domain, project: u32, words: &Words) {
    if !root.forge().demo_running() {
        return;
    }
    let Some(demo) = root.forge().demo() else { return };
    if project != demo.project {
        return;
    }
    let Some(me) = root.pages().me(root.client()) else { return };
    let project = match super::project_name(Some(me), root.forge().demo(), project, words) {
        Some(name) => name,
        None => words.say(Word::Project, &[Hole::Number(u64::from(project))]),
    };
    b.open(Role::Slot);
    b.style(look::CONTEXT);
    let repository = words.say(Word::DemoRepository, &[Hole::Name(demo.owner.clone()), Hole::Name(demo.name.clone())]);
    b.open(Role::Paragraph);
    b.text(Run::Plain, &words.say(Word::DemoContextProject, &[Hole::Name(project)]));
    b.open(Role::Text);
    b.style(look::REPOSITORY);
    b.run(Run::Plain, &repository);
    b.close();
    words.text(b, Word::DemoRepositoryForge, &[]);
    b.close();
    let mut owner = false;
    for project in &me.projects {
        if project.project == demo.project && project.role == c::HeldRole::Owner {
            owner = true;
            break;
        }
    }
    let signed = if me.deployment_role == Some(c::HeldRole::Owner) {
        words.say(Word::SignedDeploymentOwner, &[])
    } else if owner {
        words.say(Word::SignedProjectOwner, &[])
    } else {
        words.say(Word::SignedName, &[Hole::Name(skein_lib::bytes::copy_of(me.party.name.bytes()))])
    };
    text(b, Role::Paragraph, &signed);
    b.close();
}

// A proposal keeps one keyed card at its first visible call, or in the tail.
fn proposal_card(
    b: &mut Builder,
    root: &Domain,
    card: &temper_client_domain::pages::inbox::Card,
    words: &Words,
    bindings: &mut Bindings,
    (rows, times): (u32, Option<&mut Times>),
    role: Role,
) {
    b.open(role);
    b.name(&super::inbox::card_name(root, card.object, Word::Proposal, words));
    b.style(look::CARD);
    b.style(look::KIND_PROPOSAL);
    if role == Role::Item {
        b.key(super::inbox::card_key(card.object));
    }
    if let Some(page) = &card.page
        && let Some(since) = super::inbox::waiting_since(page)
        && let Some(times) = times
    {
        proposal_wait(b, words, times, since);
    }
    crate::confirm::card(b, root, card, words, bindings, rows);
    b.close();
}

// Only assistant calls inside the displayed row bounds can stand for a card.
fn proposal_anchor(chat: &Chat, object: c::ObjectKey, rows: u32) -> Option<(c::TurnName, usize, usize)> {
    let bound = usize::try_from(rows).expect("row bound fits");
    for line in chat.thread.lines().into_boxed() {
        if let Line::Turn { name, turn, .. } = line {
            for (message_at, message) in turn.messages.iter().take(bound).enumerate() {
                if message.speaker != c::Speaker::Assistant {
                    continue;
                }
                for (block_at, block) in message.blocks.iter().take(bound).enumerate() {
                    if let c::Block::Call(call) = block
                        && call.objects.as_slice().contains(&object)
                    {
                        return Some((name, message_at, block_at));
                    }
                }
            }
        }
    }
    None
}

// A proposal's wait, muted beside its first line as the inbox's.
fn proposal_wait(b: &mut Builder, words: &Words, times: &mut Times, since: skein_lib::Wall) {
    let said = words.say(Word::TimeWaitingChat, &[Hole::Name(times.say(words, TimeForm::Span, since))]);
    b.open(Role::Paragraph);
    b.name(&said);
    b.style(look::META);
    b.text(Run::Plain, &said);
    b.close();
}
#[expect(clippy::manual_map, reason = "step code spells out the supplied end clock")]
fn end_clock(task: &c::Task, result: &c::TaskResult, words: &Words, times: &mut Times) -> Option<Box<[u8]>> {
    let at = match task.chip.details.as_ref() {
        c::ChipPhase::Owner { details, .. } => details.ended_at,
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
        | c::ChipPhase::Ended { .. } => match result {
            c::TaskResult::Document(result) => result.at,
            c::TaskResult::Ending(_) => None,
        },
    };
    match at {
        Some(at) => Some(times.say(words, TimeForm::Clock, at)),
        None => None,
    }
}
fn ended_status(b: &mut Builder, task: &c::Task, result: &c::TaskResult, words: &Words, times: &mut Times) {
    let ending = match result {
        c::TaskResult::Ending(ending) => Some(ending.as_ref()),
        c::TaskResult::Document(result) => match &result.ending {
            c::Ending::Task(ending) => Some(ending.as_ref()),
            c::Ending::Done
            | c::Ending::Failed { .. }
            | c::Ending::Cancelled { .. }
            | c::Ending::Withdrawn { .. }
            | c::Ending::Decided { .. }
            | c::Ending::Deleted => None,
        },
    };
    let phase = match ending {
        Some(c::OwnerChipEnding::Failed(_)) => c::Phase::Failed,
        Some(c::OwnerChipEnding::Cancelled { by, .. }) if !closed_by_requester(task.requester, *by) => {
            c::Phase::Cancelled
        }
        Some(_) => c::Phase::Ended,
        None => match result {
            c::TaskResult::Document(result) => match &result.ending {
                c::Ending::Failed { .. } => c::Phase::Failed,
                c::Ending::Cancelled { by, .. } => {
                    if actor_requester(*by) == Some(task.requester) {
                        c::Phase::Ended
                    } else {
                        c::Phase::Cancelled
                    }
                }
                c::Ending::Withdrawn { .. } => c::Phase::Cancelled,
                c::Ending::Task(_) | c::Ending::Done | c::Ending::Decided { .. } | c::Ending::Deleted => {
                    c::Phase::Ended
                }
            },
            c::TaskResult::Ending(_) => unreachable!("owner ending handled above"),
        },
    };
    let status = if phase == c::Phase::Ended {
        words.say(Word::ChatClosed, &[])
    } else {
        words.say_word(jig_client::words::Word::Phase(phase), &[])
    };
    let name = match end_clock(task, result, words, times) {
        Some(clock) => crate::accounts::words::join(&[&status, b" ", &clock]),
        None => status,
    };
    b.open(Role::Status);
    b.style(look::STATUS_QUIET);
    b.name(&name);
    b.text(Run::Plain, &name);
    b.close();
}
fn reason_bytes(reason: Option<&c::Text>) -> Option<&[u8]> {
    match reason {
        Some(reason) => Some(reason.bytes()),
        None => None,
    }
}
// A chat owns one end line; goal/result cards keep their independent drawing.
fn result_text(
    b: &mut Builder,
    result: &c::TaskResult,
    requester: c::OwnerRequester,
    words: &Words,
    clock: Option<&[u8]>,
) {
    match result {
        c::TaskResult::Ending(ending) => owner_end(b, ending, requester, words, clock),
        c::TaskResult::Document(result) => match &result.ending {
            c::Ending::Task(ending) => owner_end(b, ending, requester, words, clock),
            c::Ending::Failed { reason } => {
                end_reason(b, Word::ChatFailed, reason_bytes(reason.as_ref()), words, clock);
            }
            c::Ending::Cancelled { reason, by, .. } => {
                let by = actor_requester(*by);
                if let Some(by) = by
                    && closed_by_requester(requester, by)
                {
                    end_closed(b, words, clock);
                    return;
                }
                end_reason(b, Word::ChatCancelled, reason_bytes(reason.as_ref()), words, clock);
            }
            c::Ending::Withdrawn { reason } => end_reason(b, Word::ChatCancelled, Some(reason.bytes()), words, clock),
            c::Ending::Done | c::Ending::Deleted | c::Ending::Decided { .. } => {
                end_report(b, result.value.as_ref(), words, clock);
            }
        },
    }
}
fn owner_end(
    b: &mut Builder,
    ending: &c::OwnerChipEnding,
    requester: c::OwnerRequester,
    words: &Words,
    clock: Option<&[u8]>,
) {
    match ending {
        c::OwnerChipEnding::Done(value) => end_report(b, Some(value), words, clock),
        c::OwnerChipEnding::Failed(reason) => {
            end_reason(b, Word::ChatFailed, Some(reason.bytes()), words, clock);
        }
        c::OwnerChipEnding::Cancelled { reason, by, .. } => {
            if closed_by_requester(requester, *by) {
                end_closed(b, words, clock);
            } else {
                end_reason(b, Word::ChatCancelled, Some(reason.bytes()), words, clock);
            }
        }
    }
}
fn actor_requester(by: c::Actor) -> Option<c::OwnerRequester> {
    match by {
        c::Actor::Party(number) => Some(c::OwnerRequester::Person(number)),
        c::Actor::Task(number) => Some(c::OwnerRequester::Task(number)),
        c::Actor::ProjectDeployment { project } => Some(c::OwnerRequester::Deployment { project }),
        c::Actor::Deployment => None,
    }
}
pub(crate) fn closed_by_requester(requester: c::OwnerRequester, by: c::OwnerRequester) -> bool {
    requester == by
}
fn end_closed(b: &mut Builder, words: &Words, clock: Option<&[u8]>) {
    let said = match clock {
        Some(clock) => words.say(Word::TimeClosed, &[crate::cards::name(clock)]),
        None => words.say(Word::ChatClosed, &[]),
    };
    end_line(b, &said);
}
fn end_reason(b: &mut Builder, word: Word, reason: Option<&[u8]>, words: &Words, clock: Option<&[u8]>) {
    let reason = match reason {
        Some(reason) => reason,
        None => b"",
    };
    let said = match clock {
        Some(clock) => words.say(
            if word == Word::ChatFailed { Word::TimeFailed } else { Word::TimeCancelled },
            &[crate::cards::name(clock), crate::cards::name(reason)],
        ),
        None => words.say(word, &[crate::cards::name(reason)]),
    };
    end_line(b, &said);
}
fn end_report(b: &mut Builder, value: Option<&c::ResultValue>, words: &Words, clock: Option<&[u8]>) {
    let said = match value {
        Some(c::ResultValue::Report(report)) if !report.bytes().is_empty() => match clock {
            Some(clock) => {
                words.say(Word::TimeEndedReport, &[crate::cards::name(clock), crate::cards::name(report.bytes())])
            }
            None => words.say(Word::ChatEndedReport, &[crate::cards::name(report.bytes())]),
        },
        _ => match clock {
            Some(clock) => words.say(Word::TimeEnded, &[crate::cards::name(clock)]),
            None => words.say(Word::ChatEnded, &[]),
        },
    };
    end_line(b, &said);
}
fn end_line(b: &mut Builder, value: &[u8]) {
    b.open(Role::Paragraph);
    b.style(look::ENDED);
    b.name(value);
    b.text(Run::Plain, value);
    b.close();
}
fn spend(b: &mut Builder, task: &c::Task, words: &Words) {
    text(
        b,
        Role::Paragraph,
        &words.say(
            Word::ChatbarSpend,
            &[Hole::Number(task.budget.numbers.spent), Hole::Number(task.budget.numbers.budget)],
        ),
    );
}
fn composer(
    b: &mut Builder,
    root: &Domain,
    chat: &Chat,
    task: &c::Task,
    words: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    if task.result.is_some() {
        return;
    }
    if let c::ChipPhase::Owner { phase: c::OwnerChipPhase::Ended, .. } = task.chip.details.as_ref() {
        return;
    }
    if temper_client_domain::pages::chat::allowed(task, c::ActKind::Message) {
        b.open(Role::Slot);
        b.style(look::COMPOSER_AREA);
        crate::choice::composer(b, root, Some(task), words, bindings, times);
        b.open(Role::Slot);
        b.style(look::ENTRY);
        b.open(Role::TextArea);
        b.name(&words.say(Word::Message, &[]));
        b.placeholder(&words.say(Word::MessagePlaceholder, &[Hole::Number(task.chip.task)]));
        if crate::choice::model_usable(root) {
            b.state(State::SubmitsOnEnter);
        }
        b.style(look::COMPOSER);
        b.value(&chat.words, temper_client_domain::pages::shown(root.pages()));
        b.bind(bindings.insert(Act::MessageWords { words: Box::default() }));
        b.close();
        b.open(Role::Paragraph);
        b.style(look::HINT);
        words.text(b, Word::ChatSendHint, &[]);
        b.close();
        b.open(Role::Button);
        b.name(&words.say(Word::Send, &[]));
        if !crate::choice::model_usable(root) {
            b.state(State::Disabled);
        }
        b.bind(bindings.insert(Act::Send));
        b.close();
        if chat.outcome_kind == Some(temper_client_domain::pages::chat::Sending::Message)
            && let Some(outcome) = &chat.outcome
            && !super::chats::accepted(outcome)
        {
            super::chats::outcome_text(b, words, outcome, 0);
        }
        b.close();
        b.close();
    } else {
        crate::choice::composer(b, root, Some(task), words, bindings, times);
    }
}
/// The chat projects a request and assistant messages; an agent keeps every work line.
pub(crate) enum Drawn<'a> {
    ChatPage {
        request: Option<&'a c::SpecificationDocument>,
        root: &'a Domain,
    },
    #[cfg(test)]
    Chat {
        request: Option<&'a c::SpecificationDocument>,
        delegations: Option<(&'a Chat, &'a jig_client::Client)>,
    },
    Lines,
}

struct Drawing<'a> {
    projected: bool,
    request: Option<&'a c::SpecificationDocument>,
    delegations: Option<(&'a Chat, &'a jig_client::Client)>,
    proposals: Option<&'a Domain>,
}
fn drawing(drawn: Drawn<'_>) -> Drawing<'_> {
    match drawn {
        Drawn::ChatPage { request, root } => Drawing {
            projected: true,
            request,
            delegations: Some((root.pages().chat().expect("chat page"), root.client())),
            proposals: Some(root),
        },
        #[cfg(test)]
        Drawn::Chat { request, delegations } => Drawing { projected: true, request, delegations, proposals: None },
        Drawn::Lines => Drawing { projected: false, request: None, delegations: None, proposals: None },
    }
}

pub(crate) fn conversation(
    b: &mut Builder,
    thread: &Thread,
    words: &Words,
    limits: (u32, u32),
    speakers: Speakers,
    drawn: Drawn<'_>,
    bindings: &mut Bindings,
) {
    conversation_with_times(b, thread, words, limits, speakers, (drawn, None), bindings);
}

// Agent task pages keep their own treatment of moments (client/README.md, 5.12).
pub(crate) fn conversation_with_times(
    b: &mut Builder,
    thread: &Thread,
    words: &Words,
    limits: (u32, u32),
    speakers: Speakers,
    (drawn, mut times): (Drawn<'_>, Option<&mut Times>),
    bindings: &mut Bindings,
) {
    let (rows, text_limit) = limits;
    let Drawing { projected, request, delegations, proposals } = drawing(drawn);
    let bound = match delegations {
        Some((chat, _)) => chat.delegations.capacity(),
        None => 0,
    };
    let mut tasks = List::with_capacity(bound);
    let mut shown = List::with_capacity(bound);
    b.open(Role::Log);
    b.name(&words.say_card(P::ConversationLog, &[]));
    let lines = thread.lines().into_boxed();
    let mut day = None;
    // A chat started with no words opens on its first words line (3.5).
    if thread.lines.edges.after == jig_client::thread::window::Edge::End
        && let Some(request) = request
        && !request.specification.words.bytes().is_empty()
    {
        let clock = turn_time(b, first_time(&lines), &mut times, &mut day, words);
        request_message(b, request, speakers.requester, words, clock.as_deref());
    }
    for line in lines {
        if hidden_line(&line, projected, delegations) {
            heard_card(b, &line, delegations, (&mut tasks, &mut shown), words, bindings);
            continue;
        }
        let clock = turn_time(b, line_time(&line), &mut times, &mut day, words);
        b.open(Role::Region);
        b.style(look::TURN);
        match line {
            Line::Words { by, words: message, state, .. } => {
                if projected {
                    person_words(b, by, message, state, speakers.person, words, clock.as_deref());
                } else {
                    person_lines(b, by, message, state, speakers.person, words, clock.as_deref());
                }
            }
            Line::Turn { name, turn, .. } => {
                if projected {
                    committed(
                        b,
                        turn,
                        words,
                        (rows, clock.as_deref()),
                        Committed {
                            delegations,
                            proposals: match proposals {
                                Some(root) => Some((root, name)),
                                None => None,
                            },
                            times: times.as_deref_mut(),
                        },
                        (&mut tasks, &mut shown),
                        bindings,
                    );
                } else {
                    committed_lines(
                        b,
                        turn,
                        words,
                        (rows, speakers.requester, clock.as_deref()),
                        delegations,
                        (&mut tasks, &mut shown),
                        bindings,
                    );
                }
            }
            Line::Provisional { text, cut, .. } => provisional(b, text, cut, words, text_limit),
            Line::Undecoded { why, .. } => undecoded(b, why, words, clock.as_deref()),
            Line::RunBegan { attempt } => {
                b.style(look::MARKER);
                text(b, Role::Paragraph, &words.say(Word::RunBegan, &[Hole::Number(attempt)]));
            }
            Line::RunEnded { attempt } => {
                b.style(look::MARKER);
                text(b, Role::Paragraph, &words.say(Word::RunEnded, &[Hole::Number(attempt)]));
            }
            Line::Left { lines } => {
                b.style(look::MARKER);
                text(b, Role::Status, &words.say(Word::LinesLeft, &[Hole::Number(u64::from(lines))]));
            }
            Line::Activity { line } => match &line.event {
                c::Activity::Owner { source, .. } => owner_line(b, &source.line, projected, speakers.person, words),
                c::Activity::Result { result } => {
                    crate::cards::result(b, result, words);
                }
                c::Activity::News { summary, .. } => text(b, Role::Paragraph, summary.bytes()),
                c::Activity::Decision { .. } => {
                    words.text(b, Word::DecisionMade, &[]);
                }
                c::Activity::Words { .. }
                | c::Activity::RunBegan { .. }
                | c::Activity::RunEnded { .. }
                | c::Activity::Turn { .. } => {}
            },
        }
        b.close();
    }
    // The live delegates anchor follows the window's last line (client/README.md, 5.7).
    live_delegates(b, &mut tasks, delegations, &mut shown, words, bindings);
    b.close();
}
fn undecoded(b: &mut Builder, why: c::TurnProblem, words: &Words, clock: Option<&[u8]>) {
    match why {
        c::TurnProblem::Version { found } => {
            words.status(b, P::TurnUnreadVersion, &[Hole::Number(u64::from(found))]);
        }
        c::TurnProblem::Malformed => words.status(b, P::TurnUnreadMalformed, &[]),
        c::TurnProblem::TooLarge { .. } => words.status(b, P::TurnUnreadTooLarge, &[]),
    }
    if clock.is_some() {
        b.open(Role::Slot);
        b.style(look::WHO);
        turn_clock(b, clock);
        b.close();
    }
}
fn request_message(
    b: &mut Builder,
    request: &c::SpecificationDocument,
    requester: bool,
    words: &Words,
    clock: Option<&[u8]>,
) {
    b.open(Role::Region);
    b.style(look::TURN);
    let label = if requester { words.say_card(P::You, &[]) } else { words.say_card(P::Person, &[]) };
    person_message(b, &label, request.specification.words.bytes(), WordState::Read { message: 0 }, words, clock);
    if request.specification.words.cut {
        words.status(b, P::TurnCut, &[]);
    }
    b.close();
}
fn first_time(lines: &[Line<'_>]) -> Option<skein_lib::Wall> {
    for line in lines {
        if let Some(at) = line_time(line) {
            return Some(at);
        }
    }
    None
}
fn line_time(line: &Line<'_>) -> Option<skein_lib::Wall> {
    match line {
        Line::Words { at, .. } => *at,
        Line::Turn { at, .. } | Line::Undecoded { at, .. } => Some(*at),
        Line::Activity { .. }
        | Line::Provisional { .. }
        | Line::RunBegan { .. }
        | Line::RunEnded { .. }
        | Line::Left { .. } => None,
    }
}
fn turn_time(
    b: &mut Builder,
    at: Option<skein_lib::Wall>,
    times: &mut Option<&mut Times>,
    day: &mut Option<u64>,
    words: &Words,
) -> Option<Box<[u8]>> {
    let at = at?;
    let times = times.as_mut()?;
    let date = at.as_nanos() / 86_400_000_000_000;
    if *day != Some(date) {
        let said = times.say(words, TimeForm::Day, at);
        b.open(Role::Paragraph);
        b.style(look::DAY);
        b.name(&said);
        b.text(Run::Plain, &said);
        b.close();
        *day = Some(date);
    }
    Some(words.turn_clock(at))
}
fn turn_clock(b: &mut Builder, clock: Option<&[u8]>) {
    if let Some(clock) = clock {
        text(b, Role::Time, clock);
    }
}
/// A delegate's heard result anchors its card, as a result block does (client/README.md, 5.7).
fn heard_card(
    b: &mut Builder,
    line: &Line<'_>,
    delegations: Option<(&Chat, &jig_client::Client)>,
    (tasks, shown): (&mut List<u64>, &mut List<u64>),
    words: &Words,
    bindings: &mut Bindings,
) {
    if let Line::Activity { line } = line
        && let c::Activity::Owner { source, .. } = &line.event
        && let Some(task) = delegation::heard(&source.line)
        && !shown.as_slice().contains(&task)
    {
        tasks.clear();
        tasks.push(task).expect("one heard delegate fits");
        b.open(Role::Region);
        b.style(look::TURN);
        cards(b, tasks, delegations, shown, words, bindings);
        b.close();
    }
}
fn provisional(b: &mut Builder, message: &[u8], cut: bool, words: &Words, text_limit: u32) {
    b.open(Role::Slot);
    b.style(look::MESSAGE);
    b.style(look::BY_AGENT);
    b.open(Role::Slot);
    b.style(look::WHO);
    words.status(b, P::Provisional, &[]);
    b.close();
    b.open(Role::Slot);
    b.style(look::SAYS);
    markdown(b, message, words, text_limit);
    if cut {
        words.status(b, P::TurnCut, &[]);
    }
    b.close();
    b.close();
}
fn owner_line(b: &mut Builder, line: &c::OwnerActivity, projected: bool, person: Option<u64>, words: &Words) {
    if projected && let c::OwnerActivity::Words { by, words: message, .. } = line {
        let (label, side) = match by {
            c::OwnerRequester::Person(by) => (
                if Some(*by) == person { words.say_card(P::You, &[]) } else { words.say_card(P::Person, &[]) },
                look::BY_PERSON,
            ),
            c::OwnerRequester::Task(task) => (words.say(Word::TaskWho, &[Hole::Number(*task)]), look::BY_AGENT),
            c::OwnerRequester::Deployment { .. } => {
                super::task::owner_activity(b, line, words);
                return;
            }
        };
        words_message(b, &label, message.bytes(), WordState::Read { message: 0 }, side, words, None);
        return;
    }
    super::task::owner_activity(b, line, words);
}
fn hidden_line(line: &Line<'_>, projected: bool, delegations: Option<(&Chat, &jig_client::Client)>) -> bool {
    match line {
        Line::RunBegan { .. } | Line::RunEnded { .. } => projected,
        Line::Activity { line } => hidden(&line.event, projected, delegations),
        Line::Words { .. }
        | Line::Turn { .. }
        | Line::Undecoded { .. }
        | Line::Provisional { .. }
        | Line::Left { .. } => false,
    }
}

fn live_delegates(
    b: &mut Builder,
    tasks: &mut List<u64>,
    delegations: Option<(&Chat, &jig_client::Client)>,
    shown: &mut List<u64>,
    words: &Words,
    bindings: &mut Bindings,
) {
    tasks.clear();
    if let Some((chat, client)) = delegations
        && let Some(document) = chat.task(client)
    {
        for delegate in &document.delegates {
            let task = delegate.chip.task;
            if !shown.as_slice().contains(&task) && chat.delegation(client, task).is_some() && tasks.push(task).is_err()
            {
                break;
            }
        }
    }
    if !tasks.is_empty() {
        b.open(Role::Region);
        b.style(look::TURN);
        cards(b, tasks, delegations, shown, words, bindings);
        b.close();
    }
}
fn person_words(
    b: &mut Builder,
    by: Option<&c::Party>,
    message: &[u8],
    state: WordState,
    person: Option<u64>,
    words: &Words,
    clock: Option<&[u8]>,
) {
    let label = match by {
        Some(by) if Some(by.number) != person => skein_lib::bytes::copy_of(by.name.bytes()),
        Some(_) | None => words.say_card(P::You, &[]),
    };
    person_message(b, &label, message, state, words, clock);
}
fn person_message(
    b: &mut Builder,
    label: &[u8],
    message: &[u8],
    state: WordState,
    words: &Words,
    clock: Option<&[u8]>,
) {
    words_message(b, label, message, state, look::BY_PERSON, words, clock);
}
fn words_message(
    b: &mut Builder,
    label: &[u8],
    message: &[u8],
    state: WordState,
    side: skein_ui::Style,
    words: &Words,
    clock: Option<&[u8]>,
) {
    b.open(Role::Slot);
    b.style(look::MESSAGE);
    b.style(side);
    b.open(Role::Slot);
    b.style(look::WHO);
    text(b, Role::Paragraph, label);
    turn_clock(b, clock);
    match state {
        WordState::BeingSent => words.status(b, P::WordsBeingSent, &[]),
        WordState::Sent { .. } => words.status(b, P::WordsSent, &[]),
        WordState::Read { .. } => {}
    }
    b.close();
    b.open(Role::Slot);
    b.style(look::SAYS);
    let mut start = 0;
    for (end, byte) in message.iter().enumerate() {
        if *byte == b'\n' {
            text(b, Role::Paragraph, message.get(start..end).expect("paragraph range"));
            start = end.checked_add(1).expect("next paragraph within message");
        }
    }
    text(b, Role::Paragraph, message.get(start..).expect("last paragraph"));
    b.close();
    b.close();
}

fn person_lines(
    b: &mut Builder,
    by: Option<&c::Party>,
    message: &[u8],
    state: WordState,
    person: Option<u64>,
    words: &Words,
    clock: Option<&[u8]>,
) {
    b.open(Role::Slot);
    b.style(look::MESSAGE);
    b.style(look::BY_PERSON);
    b.open(Role::Slot);
    b.style(look::WHO);
    match by {
        Some(by) if Some(by.number) != person => text(b, Role::Paragraph, by.name.bytes()),
        Some(_) | None => text(b, Role::Paragraph, &words.say_card(P::You, &[])),
    }
    turn_clock(b, clock);
    words.status(
        b,
        match state {
            WordState::BeingSent => P::WordsBeingSent,
            WordState::Sent { .. } => P::WordsSent,
            WordState::Read { .. } => P::WordsRead,
        },
        &[],
    );
    b.close();
    b.open(Role::Slot);
    b.style(look::SAYS);
    text(b, Role::Paragraph, message);
    b.close();
    b.close();
}

// Decoded turns already share the receiving turn_text budget; both drawings
// retain their text whole, independently of provisional/field text.
struct Committed<'a> {
    delegations: Option<(&'a Chat, &'a jig_client::Client)>,
    proposals: Option<(&'a Domain, c::TurnName)>,
    times: Option<&'a mut Times>,
}
fn committed(
    b: &mut Builder,
    turn: &c::Turn,
    words: &Words,
    (rows, clock): (u32, Option<&[u8]>),
    Committed { delegations, proposals, mut times }: Committed<'_>,
    lists: (&mut List<u64>, &mut List<u64>),
    bindings: &mut Bindings,
) {
    let (tasks, shown) = lists;
    for (message_at, message) in prefix(b, turn.messages.as_slice(), rows, words).iter().enumerate() {
        let blocks = prefix(b, message.blocks.as_slice(), rows, words);
        if message.speaker == c::Speaker::Assistant {
            let mut has_content = false;
            for block in blocks {
                match block {
                    c::Block::Text(value) => {
                        if !value.bytes().is_empty() {
                            has_content = true;
                        }
                    }
                    c::Block::Opaque { .. } => has_content = true,
                    c::Block::Call(_) | c::Block::Result { .. } => {}
                }
            }
            if has_content {
                b.open(Role::Slot);
                b.style(look::MESSAGE);
                b.style(look::BY_AGENT);
                b.open(Role::Slot);
                b.style(look::WHO);
                words.card_text(b, P::Temper, &[]);
                turn_clock(b, clock);
                b.close();
                b.open(Role::Slot);
                b.style(look::SAYS);
                for block in blocks {
                    match block {
                        c::Block::Text(value) => skein_ui::markdown::read(value.bytes(), 2, b),
                        c::Block::Opaque { dialect, size } => {
                            words.card_text(b, P::OpaqueDialect, &[crate::cards::name(dialect.bytes())]);
                            words.card_text(b, P::OpaqueBytes, &[Hole::Number(*size)]);
                        }
                        c::Block::Call(_) | c::Block::Result { .. } => {}
                    }
                }
                b.close();
                b.close();
            }
            for block in blocks {
                if let c::Block::Call(call) = block {
                    tasks.clear();
                    delegation::delegated(call, tasks);
                    cards(b, tasks, delegations, shown, words, bindings);
                }
            }
            folded_calls(b, blocks, tasks, delegations, words);
            for (block_at, block) in blocks.iter().enumerate() {
                if let c::Block::Call(call) = block
                    && let Some((root, name)) = proposals
                {
                    let chat = root.pages().chat().expect("chat page");
                    for card in &chat.cards {
                        if call.objects.as_slice().contains(&card.object)
                            && proposal_anchor(chat, card.object, rows) == Some((name, message_at, block_at))
                        {
                            proposal_card(b, root, card, words, bindings, (rows, times.as_deref_mut()), Role::Region);
                        }
                    }
                }
            }
        }
        // Input messages have no shell. Their results can still be the first
        // retained anchor of a delegated task (client/README.md, 3.5 and 5.7).
        for block in blocks {
            if let c::Block::Result { summary, .. } = block {
                tasks.clear();
                delegation::results(summary.bytes(), tasks);
                if requested(tasks, delegations) {
                    cards(b, tasks, delegations, shown, words, bindings);
                }
            }
        }
    }
    if turn.cut {
        words.status(b, P::TurnCut, &[]);
    }
}
fn committed_lines(
    b: &mut Builder,
    turn: &c::Turn,
    words: &Words,
    limits: (u32, bool, Option<&[u8]>),
    delegations: Option<(&Chat, &jig_client::Client)>,
    lists: (&mut List<u64>, &mut List<u64>),
    bindings: &mut Bindings,
) {
    let (rows, requester, clock) = limits;
    let (tasks, shown) = lists;
    for message in prefix(b, turn.messages.as_slice(), rows, words) {
        b.open(Role::Slot);
        b.style(look::MESSAGE);
        b.style(match message.speaker {
            c::Speaker::Person => look::BY_PERSON,
            c::Speaker::Assistant => look::BY_AGENT,
            c::Speaker::Tool => look::BY_TOOL,
            c::Speaker::System => look::BY_SYSTEM,
        });
        b.open(Role::Slot);
        b.style(look::WHO);
        words.card_text(
            b,
            match message.speaker {
                c::Speaker::Person => {
                    if requester {
                        P::You
                    } else {
                        P::Person
                    }
                }
                c::Speaker::Assistant => P::Temper,
                c::Speaker::Tool => P::CallCompleted,
                c::Speaker::System => P::TurnInstructions,
            },
            &[],
        );
        turn_clock(b, clock);
        b.close();
        b.open(Role::Slot);
        b.style(look::SAYS);
        let blocks = prefix(b, message.blocks.as_slice(), rows, words);
        for block in blocks {
            match block {
                c::Block::Text(value) => skein_ui::markdown::read(value.bytes(), 2, b),
                c::Block::Call(call) => {
                    tasks.clear();
                    delegation::delegated(call, tasks);
                    cards(b, tasks, delegations, shown, words, bindings);
                }
                c::Block::Result { call, summary, failed } => {
                    tasks.clear();
                    for message in &turn.messages {
                        for block in &message.blocks {
                            if let c::Block::Call(source) = block
                                && source.number == *call
                            {
                                delegation::delegated(source, tasks);
                            }
                        }
                    }
                    if carded(tasks, delegations) {
                        continue;
                    }
                    tasks.clear();
                    delegation::results(summary.bytes(), tasks);
                    if requested(tasks, delegations) {
                        cards(b, tasks, delegations, shown, words, bindings);
                        continue;
                    }
                    b.open(Role::Slot);
                    b.style(look::WORK);
                    words.card_text(b, if *failed { P::CallFailed } else { P::CallCompleted }, &[]);
                    text(b, Role::Paragraph, summary.bytes());
                    b.close();
                }
                c::Block::Opaque { dialect, size } => {
                    words.card_text(b, P::OpaqueDialect, &[crate::cards::name(dialect.bytes())]);
                    words.card_text(b, P::OpaqueBytes, &[Hole::Number(*size)]);
                }
            }
        }
        folded_calls(b, blocks, tasks, delegations, words);
        b.close();
        b.close();
    }
    if turn.cut {
        words.status(b, P::TurnCut, &[]);
    }
}

// Both conversation drawings keep a call's name and outcome as one work line.
fn call_line(b: &mut Builder, call: &c::Call) {
    b.open(Role::Slot);
    b.style(look::WORK);
    text(b, Role::Paragraph, call.tool.bytes());
    if let Some(summary) = &call.outcome {
        text(b, Role::Paragraph, summary.bytes());
    }
    b.close();
}

fn other_call(block: &c::Block, tasks: &mut List<u64>, delegations: Option<(&Chat, &jig_client::Client)>) -> bool {
    let c::Block::Call(call) = block else { return false };
    if let Some((chat, _)) = delegations {
        for card in &chat.cards {
            if call.objects.as_slice().contains(&card.object) {
                return false;
            }
        }
    }
    tasks.clear();
    delegation::delegated(call, tasks);
    !carded(tasks, delegations)
}
fn folded_calls(
    b: &mut Builder,
    blocks: &[c::Block],
    tasks: &mut List<u64>,
    delegations: Option<(&Chat, &jig_client::Client)>,
    words: &Words,
) {
    let mut count = 0_u64;
    for block in blocks {
        if other_call(block, tasks, delegations) {
            count = count.checked_add(1).expect("bounded call count");
        }
    }
    if count == 0 {
        return;
    }
    let count = words.say(Word::ToolCalls, &[Hole::Number(count)]);
    let separator = words.say(Word::ToolSeparator, &[]);
    let mut names = List::with_capacity(u32::try_from(blocks.len()).expect("bounded blocks"));
    let mut length = 0_usize;
    for (at, block) in blocks.iter().enumerate() {
        let c::Block::Call(call) = block else { continue };
        if !other_call(block, tasks, delegations) {
            continue;
        }
        let mut repeated = 0_u64;
        let mut earlier = false;
        for (index, candidate) in blocks.iter().enumerate() {
            if let c::Block::Call(other) = candidate
                && other.tool == call.tool
                && other_call(candidate, tasks, delegations)
            {
                repeated = repeated.checked_add(1).expect("bounded repeated calls");
                if index < at {
                    earlier = true;
                }
            }
        }
        if earlier {
            continue;
        }
        let name = if repeated > 1 {
            words.say(Word::ToolRepeated, &[crate::cards::name(call.tool.bytes()), Hole::Number(repeated)])
        } else {
            skein_lib::bytes::copy_of(call.tool.bytes())
        };
        if !names.is_empty() {
            length = length.checked_add(separator.len()).expect("bounded separator");
        }
        length = length.checked_add(name.len()).expect("bounded fold line");
        names.push(name).expect("one name per block at most");
    }
    let mut description = skein_lib::Writer::new(length);
    for (at, part) in names.iter().enumerate() {
        if at != 0 {
            description.put(&separator).expect("separator fits fold line");
        }
        description.put(part).expect("tool fits fold line");
    }
    b.open(Role::Disclosure);
    b.style(look::WORK);
    b.name(&count);
    b.description(&description.finish());
    for block in blocks {
        if other_call(block, tasks, delegations)
            && let c::Block::Call(call) = block
        {
            call_line(b, call);
        }
    }
    b.close();
}

fn cards(
    b: &mut Builder,
    tasks: &List<u64>,
    delegations: Option<(&Chat, &jig_client::Client)>,
    shown: &mut List<u64>,
    words: &Words,
    bindings: &mut Bindings,
) {
    let Some((chat, client)) = delegations else { return };
    for task in tasks {
        if !shown.as_slice().contains(task)
            && let Some(document) = chat.delegation(client, *task)
        {
            crate::cards::delegation::card(b, document, words, bindings);
            shown.push(*task).expect("shown delegations fit their bound");
        }
    }
}
fn requested(tasks: &List<u64>, delegations: Option<(&Chat, &jig_client::Client)>) -> bool {
    let Some((chat, client)) = delegations else { return false };
    if tasks.is_empty() {
        return false;
    }
    for task in tasks {
        if chat.requested(client, *task).is_none() {
            return false;
        }
    }
    true
}
fn carded(tasks: &List<u64>, delegations: Option<(&Chat, &jig_client::Client)>) -> bool {
    let Some((chat, client)) = delegations else { return false };
    if tasks.is_empty() {
        return false;
    }
    for task in tasks {
        if chat.delegation(client, *task).is_none() {
            return false;
        }
    }
    true
}
fn hidden(event: &c::Activity, projected: bool, delegations: Option<(&Chat, &jig_client::Client)>) -> bool {
    if let c::Activity::Owner { source, .. } = event {
        match &source.line {
            c::OwnerActivity::Milestone { .. } => return true,
            c::OwnerActivity::RunBegan { .. } | c::OwnerActivity::RunEnded { .. } => {
                if projected {
                    return true;
                }
            }
            c::OwnerActivity::Words { .. }
            | c::OwnerActivity::Heard(_)
            | c::OwnerActivity::Notified { .. }
            | c::OwnerActivity::Turn { .. }
            | c::OwnerActivity::Decided { .. }
            | c::OwnerActivity::Kept { .. } => {}
        }
    }
    let Some((chat, client)) = delegations else { return false };
    let task = match event {
        c::Activity::Owner { source, .. } => delegation::heard(&source.line),
        c::Activity::Result { result } => Some(result.task.task),
        c::Activity::Words { .. }
        | c::Activity::RunBegan { .. }
        | c::Activity::RunEnded { .. }
        | c::Activity::Turn { .. }
        | c::Activity::News { .. }
        | c::Activity::Decision { .. } => None,
    };
    match task {
        Some(task) => chat.delegation(client, task).is_some(),
        None => false,
    }
}
fn prefix<'a, T>(b: &mut Builder, values: &'a [T], most: u32, words: &Words) -> &'a [T] {
    let end = values.len().min(usize::try_from(most).expect("window fits"));
    if end < values.len() {
        words.status(b, P::TurnCut, &[]);
    }
    values.get(..end).expect("bounded prefix")
}
fn markdown(b: &mut Builder, bytes: &[u8], words: &Words, most: u32) {
    let most = bytes.len().min(usize::try_from(most).expect("text bound fits"));
    let mut end = most;
    if end < bytes.len() {
        // The protocol validated UTF-8; align a display window to the preceding character.
        for index in most.saturating_sub(3)..=most {
            if let Some(byte) = bytes.get(index)
                && byte & 0xc0 != 0x80
            {
                end = index;
            }
        }
    }
    skein_ui::markdown::read(bytes.get(..end).expect("UTF-8 prefix"), 2, b);
    if end < bytes.len() {
        words.status(b, P::TurnCut, &[]);
    }
}
