//! Layout of the forge's change facts (client/README.md, 5.10).
use crate::{
    frame::Bindings,
    look,
    words::{Word, Words},
};
use alloc::boxed::Box;
use skein_ui::{Builder, Role, Run, phrases::Hole};
use temper_client_domain::forge::{
    change::{Change, Said},
    words::ChangeWord as W,
};

pub(crate) fn say(words: &Words, word: W, holes: &[Hole]) -> Box<[u8]> {
    words.book.say(words.language, word.id(words.connector), holes).expect("checked change phrase").text
}
fn phrase(b: &mut Builder, words: &Words, said: Said) {
    b.run(Run::Plain, &say(words, said.0, &said.1));
}
fn text(b: &mut Builder, words: &Words, word: W, holes: &[Hole]) {
    b.open(Role::Paragraph);
    b.text(Run::Plain, &say(words, word, holes));
    b.close();
}
pub(crate) fn pull(b: &mut Builder, change: Change<'_>, words: &Words) {
    b.open(Role::Paragraph);
    b.style(look::META);
    // Both phrases share one META text, keeping the three-hole book bound.
    b.open(Role::Text);
    let prefix = change.pull();
    b.run(Run::Plain, &say(words, prefix.0, &prefix.1));
    let repository = change.repository();
    b.run(Run::Plain, &say(words, repository.0, &repository.1));
    b.close();
    b.close();
}
pub(crate) fn toolbar(b: &mut Builder, change: Change<'_>, words: &Words, text_bound: u32, serves_pages: bool) {
    let Some(address) = change.address() else { return };
    if serves_pages && !skein_ui::markdown::safe_url(address) {
        return;
    }
    let label = say(words, W::View, &[]);
    let bytes = label.len().checked_add(address.len()).expect("bounded address and label fit");
    if u64::try_from(bytes).expect("length fits") > u64::from(text_bound) {
        return;
    }
    b.open(Role::Slot);
    b.style(look::TASK_ACTS);
    if serves_pages {
        b.open(Role::Link);
        b.text(Run::Plain, &label);
        b.external(address);
        b.close();
    } else {
        b.open(Role::Paragraph);
        // Separate texts keep the separator within the existing per-text bound.
        b.open(Role::Text);
        b.run(Run::Plain, &label);
        b.run(Run::Plain, b" ");
        b.close();
        b.text(Run::Plain, address);
        b.close();
    }
    b.close();
}
pub(crate) fn start(b: &mut Builder, change: Change<'_>, words: &Words, bindings: &mut Bindings) {
    b.open(Role::Slot);
    b.style(look::CHANGE);
    b.open(Role::List);
    b.name(&words.say(Word::Steps, &[]));
    b.style(look::STEPS);
    let slots = change.slots();
    for (index, word) in [W::Producing, W::Opening, W::Checking, W::Gating, W::Queued, W::Landed].iter().enumerate() {
        b.open(Role::Item);
        b.style(look::STEP);
        if index < usize::from(slots.done) {
            b.style(look::STEP_DONE);
            b.state(skein_ui::State::Completed);
        }
        if slots.current == Some(u8::try_from(index).expect("six slots")) {
            b.style(look::STEP_CURRENT);
            b.state(skein_ui::State::Current);
        }
        if *word == W::Queued
            && let Some(queue) = change.queue()
        {
            let word = temper_client_domain::forge::change::ordinal_word(queue.place, true);
            b.text(Run::Plain, &say(words, word, &[Hole::Number(u64::from(queue.place))]));
        } else {
            b.text(Run::Plain, &say(words, *word, &[]));
        }
        b.close();
    }
    b.close();
    b.open(Role::Slot);
    b.style(look::CHANGE);
    let line = change.line();
    let line_word = line.0;
    b.open(Role::Text);
    phrase(b, words, line);
    if let Some(queue) = change.queue() {
        // First has its own complete sentence; Queued's prefix needs its tail.
        // The domain marks First through its line word, not a step inferred here.
        if line_word != W::FirstLine {
            if !queue.ahead.is_empty() || queue.unnamed != 0 {
                b.run(Run::Plain, &say(words, W::After, &[]));
                for (index, task) in queue.ahead.iter().enumerate() {
                    if index != 0 {
                        let word = if index.checked_add(1) == Some(queue.ahead.len()) && queue.unnamed == 0 {
                            W::And
                        } else {
                            W::Comma
                        };
                        b.run(Run::Plain, &say(words, word, &[]));
                    }
                    b.close();
                    super::task::task_link(b, *task, words, bindings);
                    b.open(Role::Text);
                }
                if queue.unnamed != 0 {
                    let word = if queue.ahead.is_empty() {
                        W::Tasks
                    } else {
                        b.run(Run::Plain, &say(words, W::And, &[]));
                        W::More
                    };
                    b.run(Run::Plain, &say(words, word, &[Hole::Number(u64::from(queue.unnamed))]));
                }
            }
            b.run(Run::Plain, &say(words, W::Period, &[]));
        }
    }
    b.close();
    b.close();
    if let Some(task) = change.runs() {
        super::task::task_link(b, task, words, bindings);
    }
    if let Some((step, reason)) = change.hold() {
        b.open(Role::Paragraph);
        b.style(look::CHANGE_HOLD);
        b.text(
            Run::Plain,
            &say(words, W::Held, &[Hole::Name(say(words, step, &[])), Hole::Name(say(words, reason, &[]))]),
        );
        b.close();
    }
}
fn gate(b: &mut Builder, words: &Words, name: W, holes: &[Hole], state: Option<W>) {
    b.open(Role::Slot);
    b.style(look::GATE);
    text(b, words, name, holes);
    if let Some(state) = state {
        text(b, words, state, &[]);
    }
    b.close();
}
pub(crate) fn finish(b: &mut Builder, change: Change<'_>, words: &Words) {
    b.open(Role::Region);
    b.name(&say(words, W::Gates, &[]));
    b.open(Role::Heading { level: 2 });
    b.text(Run::Plain, &say(words, W::Gates, &[]));
    b.close();
    gate(b, words, W::Ci, &[], Some(change.ci()));
    for index in 0..change.gates() {
        let row = change.gate(index).expect("bounded gate index");
        gate(b, words, row.name, &[Hole::Number(row.number)], Some(row.verdict));
    }
    if change.gates() == 0 {
        gate(b, words, W::NoGate, &[], None);
    }
    b.close();
    b.open(Role::Paragraph);
    b.style(look::META);
    b.open(Role::Text);
    for (index, (word, holes)) in change.bounds().iter().enumerate() {
        if index != 0 {
            b.run(Run::Plain, &say(words, W::Separator, &[]));
        }
        b.run(Run::Plain, &say(words, *word, holes));
    }
    b.close();
    b.close();
    b.close();
}

pub(crate) fn shown(panel: Option<&temper_client_domain::forge::panel::Panel>) -> Option<Change<'_>> {
    temper_client_domain::forge::change::shown(&panel?.state)
}
