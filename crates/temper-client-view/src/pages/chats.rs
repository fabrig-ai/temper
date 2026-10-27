//! The person's chats: a composer and one filtered list of live and closed lines.
use crate::{
    frame::Bindings,
    words::{TimeForm, Times, Word, Words},
};
use alloc::boxed::Box;
use jig_client::contract::{EndedGroup, TaskLine};
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::{Address, ChatsFilter, Domain, pages::Act};
fn text(b: &mut Builder, role: Role, value: &[u8]) {
    b.open(role);
    b.name(value);
    b.text(Run::Plain, value);
    b.close();
}
fn control(b: &mut Builder, bindings: &mut Bindings, role: Role, name: &[u8], act: Act) {
    if role == Role::Link {
        crate::link(b, name, bindings.insert(act));
        b.text(Run::Plain, name);
    } else {
        b.open(role);
        b.name(name);
        b.bind(bindings.insert(act));
    }
    b.close();
}
/// Enter and Start share the composer's send readiness (client/README.md, 3.5 and 5.6):
/// the model is usable, no playground start is pending or failed, and no chat start is in flight.
fn ready_to_start(root: &Domain) -> bool {
    let Some(chats) = root.pages().chats() else { return false };
    crate::choice::can_send(root)
        && chats.start.is_none()
        && !root.forge().demo_starting()
        && root.forge().demo_failure().is_none()
}
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    text_limit: u32,
    times: &mut Times,
) {
    b.open(Role::Main);
    b.style(crate::look::MAIN);
    let demo = demo_start(root);
    if let Some(demo) = demo {
        start(b, root, demo, words, bindings);
    } else {
        b.style(crate::look::PAGE);
        b.open(Role::Slot);
        b.style(crate::look::PAGEBAR);
        text(b, Role::Heading { level: 1 }, &words.say(Word::Chats, &[]));
        b.close();
    }
    if let Some(chats) = root.pages().chats() {
        if root.pages().me(root.client()).is_some() && root.pages().project().is_some() {
            if demo.is_none() {
                text(b, Role::Heading { level: 2 }, &words.say(Word::StartChat, &[]));
            }
            crate::choice::composer(b, root, None, words, bindings, times);
            let ready = ready_to_start(root);
            if !ready {
                bindings.block_submissions();
            }
            b.open(Role::Slot);
            b.style(crate::look::ENTRY);
            b.open(Role::TextArea);
            b.name(&words.say(Word::StartChat, &[]));
            if let Address::Chats { start: true, .. } = root.pages().address() {
                b.focus(0);
            }
            if ready {
                b.state(State::SubmitsOnEnter);
            }
            b.style(crate::look::COMPOSER);
            b.value(&chats.words, temper_client_domain::pages::shown(root.pages()));
            b.bind(bindings.insert(Act::ChatWords { words: Box::default() }));
            b.close();
            b.open(Role::Paragraph);
            b.style(crate::look::HINT);
            words.text(b, Word::ChatHint, &[]);
            b.close();
            if let Some(me) = root.pages().me(root.client())
                && root.pages().choice().valid(me)
            {
                b.open(Role::Button);
                b.name(&words.say(Word::Start, &[]));
                if !ready {
                    b.state(State::Disabled);
                }
                b.bind(bindings.insert(Act::StartChat));
                b.close();
            }
            if chats.refusal.is_some() {
                words.refusal(b, &jig_client::contract::Refusal::NotOffered, text_limit);
            }
            if let Some(outcome) = &chats.outcome
                && !accepted(outcome)
            {
                outcome_text(b, words, outcome, text_limit);
            }
            b.close();
        }
        list(b, root, words, bindings, times);
    }
    b.close();
}
fn list(b: &mut Builder, root: &Domain, words: &Words, bindings: &mut Bindings, times: &mut Times) {
    let Some(chats) = root.pages().chats() else { return };
    b.open(Role::Slot);
    b.style(crate::look::HEADING);
    text(b, Role::Heading { level: 2 }, &words.say(Word::YourChats, &[]));
    b.open(Role::Slot);
    b.style(crate::look::FILTERS);
    for (filter, word) in [
        (ChatsFilter::All, Word::AllChats),
        (ChatsFilter::Live, Word::LiveChats),
        (ChatsFilter::Closed, Word::ClosedChats),
    ] {
        b.open(Role::Button);
        b.name(&words.say(word, &[]));
        if chats.filter == filter {
            b.state(State::Pressed);
        }
        b.bind(bindings.insert(Act::ChatFilter { filter }));
        b.close();
    }
    b.close();
    b.close();
    let live = match chats.live(root.client()) {
        Some(live) if chats.filter != ChatsFilter::Closed => live.rows.as_slice(),
        _ => &[],
    };
    let (closed, more) = match &chats.page {
        Some(page) if chats.filter != ChatsFilter::Live => match &page.ended {
            EndedGroup::Rows { rows, after, .. } => (rows.as_slice(), after.is_some()),
            EndedGroup::None => (&[][..], false),
        },
        _ => (&[][..], false),
    };
    if live.is_empty() && closed.is_empty() {
        let word = match chats.filter {
            ChatsFilter::All => Word::NoChats,
            ChatsFilter::Live => Word::NoLiveChats,
            ChatsFilter::Closed => Word::NoClosedChats,
        };
        let empty = words.say(word, &[]);
        b.open(Role::Paragraph);
        b.name(&empty);
        b.style(crate::look::EMPTY);
        b.text(Run::Plain, &empty);
        b.close();
    } else {
        b.open(Role::List);
        b.name(&words.say(Word::YourChats, &[]));
        b.style(crate::look::ROWS);
        // Select from the two bounded windows without allocating scratch or
        // cloning documents. Only the previous key survives each selection.
        let mut previous = None;
        for _ in 0..live.len().checked_add(closed.len()).expect("two bounded windows fit") {
            let mut next: Option<&TaskLine> = None;
            for rows in [live, closed] {
                for line in rows {
                    let key = (line.ended_at.unwrap_or(line.last_activity), line.chip.task);
                    if let Some(previous) = previous
                        && !before(previous, key)
                    {
                        continue;
                    }
                    if let Some(chosen) = next
                        && !before(key, (chosen.ended_at.unwrap_or(chosen.last_activity), chosen.chip.task))
                    {
                        continue;
                    }
                    next = Some(line);
                }
            }
            let Some(chosen) = next else { break };
            row(b, root, chosen, words, bindings, times);
            previous = Some((chosen.ended_at.unwrap_or(chosen.last_activity), chosen.chip.task));
        }
        b.close();
    }
    if more {
        control(b, bindings, Role::Button, &words.say(Word::MoreClosedChats, &[]), Act::ClosedChats);
    }
}

fn before(a: (skein_lib::Wall, u64), b: (skein_lib::Wall, u64)) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 < b.1)
}

fn start(
    b: &mut Builder,
    root: &Domain,
    demo: &temper_client_domain::forge::Demo,
    words: &Words,
    bindings: &mut Bindings,
) {
    b.open(Role::Slot);
    b.style(crate::look::CHATBAR);
    b.open(Role::Slot);
    text(b, Role::Paragraph, &words.say(Word::NewChat, &[]));
    b.open(Role::Status);
    b.style(crate::look::STATUS_QUIET);
    let status = if root.forge().demo_starting() {
        Word::DemoStarting
    } else if root.forge().demo_failure().is_some() {
        Word::DemoNotStarted
    } else {
        Word::Ready
    };
    let ready = words.say(status, &[]);
    b.name(&ready);
    b.text(Run::Plain, &ready);
    b.close();
    b.close();
    b.open(Role::Slot);
    text(b, Role::Paragraph, &words.say(Word::DemoModelName, &[]));
    b.close();
    b.close();
    if let Some(project) = root.pages().project() {
        super::chat::context(b, root, project, words);
    }
    suggestions(b, root, demo, words, bindings);
}

pub(crate) fn ended(chip: &jig_client::contract::Chip) -> bool {
    use jig_client::contract::{ChipPhase, OwnerChipPhase, Phase};
    if let Phase::Done | Phase::Ended | Phase::Failed | Phase::Cancelled = chip.phase {
        return true;
    }
    if let ChipPhase::Owner { phase: OwnerChipPhase::Ended, .. } | ChipPhase::Ended { .. } = chip.details.as_ref() {
        return true;
    }
    false
}

pub(crate) fn row(
    b: &mut Builder,
    root: &Domain,
    line: &TaskLine,
    words: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    b.open(Role::Item);
    b.style(crate::look::ROW);
    b.key(skein_ui::Key { owner: crate::frame::owner(), kind: 10, a: line.chip.task, b: 0 });
    b.open(Role::Slot);
    b.style(crate::look::META);
    if let Some(me) = root.pages().me(root.client()) {
        status(b, &line.chip, (&line.waits_on, None, None), (line.project, line.requester), None, me, words);
    }
    // Keep the number's link in a container: a paragraph is flattened to text
    // by the terminal, hiding the command promised by client/README.md, 3.5.
    b.open(Role::Slot);
    if let Some(project) = super::project_name(root.pages().me(root.client()), root.forge().demo(), line.project, words)
    {
        b.text(Run::Plain, &project);
        words.text(b, Word::ToolSeparator, &[]);
    }
    control(
        b,
        bindings,
        Role::Link,
        &words.say(Word::Chat, &[Hole::Number(line.chip.task)]),
        Act::Go { address: Address::Chat { task: line.chip.task } },
    );
    b.close();
    b.close();
    text(b, Role::Paragraph, line.chip.title.bytes());
    if (line.ended_at.is_some() || ended(&line.chip)) && line.flags.retention == jig_client::contract::Retention::Kept {
        text(b, Role::Paragraph, &words.say(Word::ChatKept, &[]));
    }
    b.open(Role::Paragraph);
    b.style(crate::look::DETAILS);
    let spend = words.say(Word::ChatRowSpend, &[Hole::Number(line.budget.numbers.spent)]);
    b.name(&spend);
    b.text(Run::Plain, &spend);
    if line.ended_at.is_none()
        && !ended(&line.chip)
        && let jig_client::contract::ChipPhase::Owner { details, .. } = line.chip.details.as_ref()
        && details.live_delegates > 0
    {
        words.text(b, Word::ToolSeparator, &[]);
        words.text(b, Word::ChatDelegates, &[Hole::Number(u64::from(details.live_delegates))]);
    }
    b.close();
    b.open(Role::Paragraph);
    b.style(crate::look::TIME);
    b.text(Run::Plain, &times.say(words, TimeForm::Ago, line.ended_at.unwrap_or(line.last_activity)));
    b.close();
    b.close();
}

/// A successful start/message is already represented by the conversation.
pub(crate) fn accepted(outcome: &jig_client::Outcome) -> bool {
    let jig_client::Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Done(_))) = outcome
    else {
        return false;
    };
    true
}

pub(crate) fn outcome_text(b: &mut Builder, words: &Words, outcome: &jig_client::Outcome, text_limit: u32) {
    use crate::cards::P;
    use jig_client::{Answered, Outcome};
    match outcome {
        Outcome::Answered(Answered::Jig(jig_client::contract::Answer::Done(value))) => words.outcome(b, value),
        Outcome::Answered(Answered::Jig(jig_client::contract::Answer::Refused { refusal, .. })) => {
            words.refusal(b, refusal, text_limit);
        }
        Outcome::NotKept | Outcome::Held | Outcome::Full { .. } => words.status(b, P::RequestNotKept, &[]),
        Outcome::Unknown => words.status(b, P::Unknown, &[]),
        Outcome::Limit => words.status(b, P::RequestsInFlight, &[]),
        Outcome::Unconfirmed => words.status(b, P::RequestUnconfirmed, &[]),
        Outcome::Answered(Answered::Family(_)) => {}
        Outcome::Unsent | Outcome::Lost => unreachable!("temper's requests carry no value"),
    }
}

pub(crate) fn status(
    b: &mut Builder,
    chip: &jig_client::contract::Chip,
    (waits_on, delegates, run_wait): (
        &jig_client::contract::WaitsOn,
        Option<u32>,
        Option<&jig_client::contract::TaskRunWait>,
    ),
    (project, requester): (u32, jig_client::contract::OwnerRequester),
    wake: Option<&jig_client::contract::OwnerWakePolicy>,
    me: &jig_client::contract::Me,
    words: &Words,
) {
    use jig_client::contract::{ChipPhase, HoldCategory, OwnerRequester, WaitsOn, WakeRule};
    use jig_client::facts::ChatStatus;
    if ended(chip) {
        closed_status(b, words);
        return;
    }
    let live = delegates.unwrap_or(if let ChipPhase::Owner { details, .. } = chip.details.as_ref() {
        details.live_delegates
    } else {
        0
    });
    let waits_on = if live == 0 && *waits_on == WaitsOn::Delegates { &WaitsOn::News } else { waits_on };
    let wake = match wake {
        Some(wake) => Some(wake),
        None => match chip.details.as_ref() {
            ChipPhase::Owner { details, .. } => Some(&details.wake),
            ChipPhase::Active(_)
            | ChipPhase::Waiting { .. }
            | ChipPhase::Idle { .. }
            | ChipPhase::Starting
            | ChipPhase::Running { .. }
            | ChipPhase::WaitingForWords { .. }
            | ChipPhase::BackingOff { .. }
            | ChipPhase::Procedure { .. }
            | ChipPhase::Person { .. }
            | ChipPhase::Closing(_)
            | ChipPhase::Held { .. }
            | ChipPhase::Ended { .. } => None,
        },
    };
    let narrowed = if (*waits_on == WaitsOn::Delegates || *waits_on == WaitsOn::News) && live > 0 {
        Some(words.say(Word::WaitingForDelegates, &[Hole::Number(u64::from(live))]))
    } else if *waits_on == WaitsOn::News
        && requester == OwnerRequester::Person(me.party.number)
        && let Some(wake) = wake
        && match wake.words {
            WakeRule::Never => false,
            WakeRule::Immediate | WakeRule::Batch { .. } => true,
        }
    {
        Some(words.say(Word::WaitingForYourWords, &[]))
    } else {
        None
    };
    if let Some(name) = narrowed {
        b.open(Role::Status);
        b.style(crate::look::STATUS_WAITING);
        b.name(&name);
        b.text(Run::Plain, &name);
        b.close();
        return;
    }
    let status = jig_client::facts::chat_status(chip, waits_on, run_wait, project, me);
    let holes = match &status {
        ChatStatus::Tool { tool, .. } => Some(Hole::Name(tool.clone())),
        ChatStatus::Starting
        | ChatStatus::Writing
        | ChatStatus::Working { .. }
        | ChatStatus::WaitingForYou { .. }
        | ChatStatus::WaitingFor(_)
        | ChatStatus::Parked
        | ChatStatus::Retrying { .. }
        | ChatStatus::HostRefused(_)
        | ChatStatus::Held(_)
        | ChatStatus::Closing
        | ChatStatus::Ended { .. } => None,
    };
    let token = match status {
        ChatStatus::WaitingForYou { .. } => crate::look::STATUS_YOURS,
        ChatStatus::HostRefused(_) | ChatStatus::Held(_) => crate::look::STATUS_HELD,
        ChatStatus::Retrying { .. } => crate::look::STATUS_RETRYING,
        ChatStatus::Starting | ChatStatus::Writing | ChatStatus::Tool { .. } | ChatStatus::Working { .. } => {
            crate::look::STATUS_ACTIVE
        }
        ChatStatus::WaitingFor(_) => crate::look::STATUS_WAITING,
        ChatStatus::Parked | ChatStatus::Closing | ChatStatus::Ended { .. } => crate::look::STATUS_QUIET,
    };
    b.open(Role::Status);
    b.style(token);
    let name = match status {
        ChatStatus::Held(ref reason @ HoldCategory::Exceeded(_)) => crate::cards::hold_phrase(reason, words),
        ChatStatus::WaitingForYou { .. } => words.say(Word::WaitingForYou, &[]),
        ChatStatus::Ended { .. } => words.say(Word::ChatClosed, &[]),
        ChatStatus::Starting
        | ChatStatus::Writing
        | ChatStatus::Tool { .. }
        | ChatStatus::Working { .. }
        | ChatStatus::WaitingFor(_)
        | ChatStatus::Parked
        | ChatStatus::Retrying { .. }
        | ChatStatus::HostRefused(_)
        | ChatStatus::Held(_)
        | ChatStatus::Closing => words.say_word(jig_client::words::Word::status(&status), holes.as_slice()),
    };
    b.name(&name);
    b.text(Run::Plain, &name);
    b.close();
}

fn closed_status(b: &mut Builder, words: &Words) {
    b.open(Role::Status);
    b.style(crate::look::STATUS_QUIET);
    let name = words.say(Word::ChatClosed, &[]);
    b.name(&name);
    b.text(Run::Plain, &name);
    b.close();
}

/// The demo whose start the Chats page shows, when it shows one.
pub(crate) fn demo_start(root: &Domain) -> Option<&temper_client_domain::forge::Demo> {
    let Address::Chats { .. } = root.pages().address() else { return None };
    match root.demo() {
        Some((demo, halves)) if halves.model && root.pages().project() == Some(demo.project) => Some(demo),
        _ => None,
    }
}

fn suggestions(
    b: &mut Builder,
    root: &Domain,
    demo: &temper_client_domain::forge::Demo,
    words: &Words,
    bindings: &mut Bindings,
) {
    let Some(chats) = root.pages().chats() else { return };
    let heading = words.say(Word::DemoHeading, &[]);
    b.open(Role::Region);
    b.name(&heading);
    b.style(crate::look::INVITATION);
    text(b, Role::Heading { level: 1 }, &heading);
    text(
        b,
        Role::Paragraph,
        &words.say(Word::DemoRequest, &[Hole::Name(demo.owner.clone()), Hole::Name(demo.name.clone())]),
    );
    if let Some(failure) = root.forge().demo_failure() {
        use temper_client_domain::forge::demo::StartFailure;
        let message = match failure {
            StartFailure::Group(why) => {
                words.say(Word::DemoGroupFailed, &[Hole::Name(temper_client_domain::forge::words::lossy(why))])
            }
            StartFailure::Account => words.say(Word::DemoAccountFailed, &[]),
            StartFailure::Adoption => {
                words.say(Word::DemoAdoptionFailed, &[Hole::Name(demo.owner.clone()), Hole::Name(demo.name.clone())])
            }
            StartFailure::NotConnected => words.say(Word::DemoNotConnected, &[]),
        };
        text(b, Role::Alert, &message);
        b.open(Role::Button);
        b.name(&words.say(Word::DemoTryAgain, &[]));
        b.bind(bindings.insert(Act::RetryDemo));
        b.close();
    }
    b.open(Role::List);
    b.name(&words.say(Word::SuggestedRequests, &[]));
    for (cue, value) in demo.cues.iter().enumerate() {
        b.open(Role::Item);
        b.open(Role::Button);
        b.name(value);
        if chats.start.is_some() || root.forge().demo_starting() || root.forge().demo_failure().is_some() {
            b.state(State::Disabled);
        }
        b.bind(bindings.insert(Act::Suggest { cue: u8::try_from(cue).expect("eight cue indices fit") }));
        b.close();
        b.close();
    }
    b.close();
    text(b, Role::Paragraph, &words.say(Word::DemoScript, &[]));
    b.close();
}
