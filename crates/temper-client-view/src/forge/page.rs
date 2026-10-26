//! The ordered adoption form in temper's frame (client/README.md, 4.4 and 5.2).
use super::{
    bindings::{Bindings, Meaning},
    words,
};
use crate::words::Words;
use alloc::boxed::Box;
use skein_lib::{RequestProgress, Writer, bytes::copy_of};
use skein_ui::{Builder, Role, State, phrases::Hole};
use temper_client_domain::forge::{self, words::Word};
pub fn page(
    builder: &mut Builder,
    form: &forge::Form,
    project_name: Option<&[u8]>,
    words: &Words,
    bindings: &mut Bindings,
    text_limit: u32,
) {
    builder.open(Role::Slot);
    builder.style(crate::look::PAGEBAR);
    builder.open(Role::Heading { level: 1 });
    let title = match project_name {
        Some(name) => words::say(words, Word::Title, &[Hole::Name(copy_of(name))]),
        None => words::say(words, Word::TitleNumber, &[Hole::Number(u64::from(form.project))]),
    };
    builder.name(&title);
    builder.text(skein_ui::Run::Plain, &title);
    builder.close();
    builder.close();
    builder.open(Role::Region);
    builder.name(&title);
    builder.open(Role::Slot);
    builder.style(crate::look::CARD);
    for (field, word) in
        [(forge::Text::Forge, Word::Forge), (forge::Text::Owner, Word::Owner), (forge::Text::Name, Word::Name)]
    {
        text_field(builder, form, field, word, words, bindings);
    }
    builder.close();
    builder.open(Role::ChoiceGroup);
    builder.name(&words::say(words, Word::Role, &[]));
    for (role, word) in [
        (forge::Role::Owned, Word::OwnedChoice),
        (forge::Role::Fork, Word::ForkChoice),
        (forge::Role::Context, Word::ContextChoice),
    ] {
        builder.open(Role::Choice);
        builder.name(&words::say(words, word, &[]));
        if form.role == role {
            builder.state(State::Selected);
        }
        builder.bind(bindings.insert(Meaning::Role(role)));
        builder.close();
    }
    builder.close();
    check(builder, form.home, Word::Home, Meaning::Home, words, bindings);
    builder.open(Role::Slot);
    builder.style(crate::look::CARD);
    text_field(builder, form, forge::Text::Prefix, Word::Prefix, words, bindings);
    text_field(builder, form, forge::Text::Landing, Word::Landing, words, bindings);
    builder.close();
    check(builder, form.ci, Word::Ci, Meaning::Ci, words, bindings);
    text_field(builder, form, forge::Text::Checks, Word::Checks, words, bindings);
    builder.open(Role::Button);
    builder.name(&words::say(words, Word::Adopt, &[]));
    builder.bind(bindings.insert(Meaning::Adopt));
    if !form.confirmed || form.pending.is_some() || form.served == forge::Served::OutOfDate {
        builder.state(State::Disabled);
    }
    builder.close();
    if form.served == forge::Served::OutOfDate {
        status(builder, Word::OutOfDate, &[], words);
    }
    if let Some((_, progress)) = form.pending {
        crate::cards::progress(
            builder,
            words,
            match progress {
                RequestProgress::InFlight => jig_client::Progress::InFlight,
                RequestProgress::Retrying => jig_client::Progress::Retrying { why: jig_client::Retry::Lost },
                RequestProgress::Parked => jig_client::Progress::Parked,
                RequestProgress::Unknown => jig_client::Progress::Unknown,
            },
        );
    }
    if let Some(answer) = &form.outcome {
        outcome(builder, form, answer, words, text_limit);
    }
    builder.close();
}
fn text_field(
    builder: &mut Builder,
    form: &forge::Form,
    field: forge::Text,
    word: Word,
    words: &Words,
    bindings: &mut Bindings,
) {
    let field_state = form.texts.get(field.index()).expect("six fields");
    builder.open(Role::Slot);
    builder.style(crate::look::CARD);
    builder.open(Role::TextField);
    builder.name(&words::say(words, word, &[]));
    builder.value(&field_state.text, field_state.written);
    builder.bind(bindings.insert(Meaning::Field(field)));
    if field_state.broken.is_some() {
        builder.state(State::Invalid);
    }
    builder.close();
    broken(builder, field_state.broken, words);
    builder.close();
}
fn check(
    builder: &mut Builder,
    check: forge::Check,
    word: Word,
    meaning: Meaning,
    words: &Words,
    bindings: &mut Bindings,
) {
    builder.open(Role::Check);
    builder.name(&words::say(words, word, &[]));
    builder.bind(bindings.insert(meaning));
    if check.on {
        builder.state(State::Pressed);
    }
    if check.broken.is_some() {
        builder.state(State::Invalid);
    }
    builder.close();
    broken(builder, check.broken, words);
}
fn status(builder: &mut Builder, word: Word, holes: &[Hole], words: &Words) {
    builder.open(Role::Status);
    builder.name(&words::say(words, word, holes));
    words::text(words, builder, word, holes);
    builder.close();
}
fn broken(builder: &mut Builder, broken: Option<forge::Broken>, words: &Words) {
    if let Some((word, holes)) = forge::words::broken(broken) {
        status(builder, word, &holes, words);
    }
}

fn kinds(kinds: forge::Kinds, words: &Words) -> Box<[u8]> {
    let rows = forge::words::kinds(kinds);
    let mut bytes = 0_usize;
    for (on, word) in rows {
        if on {
            if bytes != 0 {
                bytes = bytes.checked_add(2).expect("separator fits");
            }
            bytes = bytes.checked_add(words::say(words, word, &[]).len()).expect("nine kinds fit");
        }
    }
    let mut writer = Writer::new(bytes);
    let mut first = true;
    for (on, word) in rows {
        if on {
            if !first {
                writer.put(b"; ").expect("nine kinds fit");
            }
            writer.put(&words::say(words, word, &[])).expect("nine kinds fit");
            first = false;
        }
    }
    writer.finish()
}
pub(super) fn outcome(
    builder: &mut Builder,
    form: &forge::Form,
    answer: &forge::Answer,
    words: &Words,
    text_limit: u32,
) {
    match answer {
        forge::Answer::Adopted(adopted) => {
            let role = forge::words::role(adopted.role);
            status(
                builder,
                Word::Adopted,
                &[
                    Hole::Name(forge::words::full_name(&adopted.owner, &adopted.name)),
                    Hole::Number(u64::from(adopted.forge)),
                    Hole::Name(words::say(words, role, &[])),
                ],
                words,
            );
            if adopted.home {
                status(builder, Word::IsHome, &[], words);
            }
            status(builder, Word::May, &[Hole::Name(kinds(adopted.kinds, words))], words);
            match adopted.protection {
                forge::Protection::Unknown => status(builder, Word::Protection, &[], words),
                forge::Protection::Absent | forge::Protection::Rule(_) => {}
            }
        }
        forge::Answer::CoreRefused(refusal) => {
            builder.open(Role::Alert);
            words.refusal(builder, refusal, text_limit);
            builder.close();
        }
        forge::Answer::EntranceRefused(refusal) => {
            builder.open(Role::Alert);
            words.refusal(builder, refusal.refusal(), text_limit);
            builder.close();
        }
        forge::Answer::ForgeRefused(reason) => {
            builder.open(Role::Alert);
            let [forge, owner, name, prefix, _, _] = &form.texts;
            let name = Hole::Name(forge::words::full_name(&owner.text, &name.text));
            let number = Hole::Name(forge.text.clone());
            match reason {
                forge::Reason::NoSuchForge => words::text(words, builder, Word::NoSuchForge, &[number]),
                forge::Reason::NotFound => words::text(words, builder, Word::NotFound, &[name, number]),
                forge::Reason::NoAccess => words::text(words, builder, Word::NoAccess, &[name, number]),
                forge::Reason::Refused => {
                    words::text(words, builder, Word::Refused, &[name, Hole::Name(prefix.text.clone())]);
                }
                forge::Reason::AnotherProject => words::text(words, builder, Word::AnotherProject, &[name]),
                forge::Reason::HomeHeld => words::text(words, builder, Word::HomeHeld, &[]),
                forge::Reason::PrefixOccupied => {
                    words::text(words, builder, Word::PrefixOccupied, &[Hole::Name(prefix.text.clone())]);
                }
                forge::Reason::InFlight => words::text(words, builder, Word::InFlight, &[name]),
                forge::Reason::TooLarge => words::text(words, builder, Word::TooLarge, &[]),
                forge::Reason::Unreachable => words::text(words, builder, Word::Unreachable, &[number]),
                forge::Reason::InvalidAnswer => words::text(words, builder, Word::InvalidAnswer, &[number]),
            }
            builder.close();
        }
        forge::Answer::Unknown => {
            let [_, owner, name, _, _, _] = &form.texts;
            status(builder, Word::Unknown, &[Hole::Name(forge::words::full_name(&owner.text, &name.text))], words);
        }
    }
}
