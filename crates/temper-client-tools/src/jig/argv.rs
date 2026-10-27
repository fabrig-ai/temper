//! One table-driven argv parser and checker.
pub(crate) mod value;
use super::{
    Limits,
    call::{Call, Parsed, Source, Value, Values},
    key::Name,
    subject,
    table::{Argument, Keyed, Kind, Occurs, Place, Tool, bound},
};
use alloc::boxed::Box;
use jig_client::contract::QueryOrder;
use skein_lib::{List, Overflow, Writer};
use value::{decode, encode, valid};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    Tool,
    NoGroup { at: u16 },
    NeedsVerb { at: u16 },
    NoTool { group: u16, verb: u16 },
    Unknown { at: u16 },
    Extra { at: u16 },
    Missing { argument: u8 },
    Twice { argument: u8 },
    Kind { argument: u8 },
    Rows { argument: u8 },
    Long { argument: u8 },
    Many { argument: u8 },
    Stdin,
    Key,
    Condition,
    TerminalPhase { phase: jig_client::contract::Phase },
    Timeout,
    WaitPart { tool: u8, argument: u8 },
    Json,
    AmendmentRevision,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unwritten {
    Overflow(Overflow),
    NoForm,
}
struct Form {
    values: Box<[Option<Values>]>,
    key: Option<Name>,
    json: bool,
}
/// Parse argv after the host's directory.
pub fn parse(table: &[&Tool], words: &[&[u8]], limits: &Limits) -> Result<Parsed, Usage> {
    u16::try_from(words.len()).or(Err(Usage::Tool))?;
    let (first, rest) = words.split_first().ok_or(Usage::Tool)?;
    let (name, words, json) = if *first == b"--json" {
        let (name, words) = rest.split_first().ok_or(Usage::Tool)?;
        (*name, words, true)
    } else {
        (*first, rest, false)
    };
    if name == b"--call" {
        let (file, tail) = words.split_first().ok_or(Usage::Json)?;
        let json = match tail {
            [] => json,
            [b"--json"] if !json => true,
            _ => return Err(Usage::Json),
        };
        let file = if *file == b"-" { Source::Stdin } else { Source::Path(Box::from(*file)) };
        return Ok(Parsed::File { file, json });
    }
    let offset = u16::from(json);
    let (selected, words) = named_tool(table, name, words, offset)?;
    if name == super::rows::HELP.name
        && let Some((group, tail)) = words.split_first()
        && !group.starts_with(b"--")
    {
        let at = offset.checked_add(1).ok_or(Usage::Tool)?;
        let known = first_word(table, group).ok_or(Usage::NoGroup { at })?;
        if let Some(verb) = tail.first()
            && !verb.starts_with(b"--")
            && (known.name == *group || named_index(table, group, verb).is_none())
        {
            return Err(Usage::NoTool { group: at, verb: at.checked_add(1).ok_or(Usage::Tool)? });
        }
    }
    let mut error = Usage::Tool;
    for (tool, row) in table.iter().enumerate() {
        if row.name != selected.name {
            continue;
        }
        let tool = u8::try_from(tool).or(Err(Usage::Tool))?;
        for (form, arguments) in row.forms.iter().enumerate() {
            let form = u8::try_from(form).or(Err(Usage::Tool))?;
            check_words(row, tool, arguments, words)?;
            let parsed = form_values(table, arguments, words, limits, row.keyed, json);
            match parsed {
                Ok(parsed) => {
                    let call = Call { tool, form, values: parsed.values, key: parsed.key };
                    match check(table, &call, limits) {
                        Ok(()) => return Ok(Parsed::Call { call, json: parsed.json }),
                        Err(failed) => error = failed,
                    }
                }
                Err(failed) => {
                    if error == Usage::Tool {
                        error = failed;
                    }
                }
            }
        }
    }
    Err(error)
}
#[expect(clippy::manual_find, reason = "the parser searches the bounded table without closures")]
fn first_word<'a>(table: &[&'a Tool], word: &[u8]) -> Option<&'a Tool> {
    for row in table {
        if super::table::name_words(row.name).0 == word {
            return Some(row);
        }
    }
    None
}
fn named_index(table: &[&Tool], group: &[u8], verb: &[u8]) -> Option<u8> {
    for (index, row) in table.iter().enumerate() {
        if let Some(rest) = row.name.strip_prefix(group)
            && rest.strip_prefix(b" ") == Some(verb)
        {
            return u8::try_from(index).ok();
        }
    }
    None
}
fn named_tool<'a, 'b>(
    table: &[&'a Tool],
    group: &[u8],
    words: &'b [&'b [u8]],
    at: u16,
) -> Result<(&'a Tool, &'b [&'b [u8]]), Usage> {
    let row = first_word(table, group).ok_or(Usage::NoGroup { at })?;
    if row.name == group {
        return Ok((row, words));
    }
    let (verb, tail) = words.split_first().ok_or(Usage::NeedsVerb { at })?;
    let index = named_index(table, group, verb)
        .ok_or(Usage::NoTool { group: at, verb: at.checked_add(1).ok_or(Usage::Tool)? })?;
    Ok((*table.get(usize::from(index)).ok_or(Usage::Tool)?, tail))
}
fn tool_index(table: &[&Tool], word: &[u8]) -> Option<u8> {
    for (index, tool) in table.iter().enumerate() {
        if tool.name == word {
            return u8::try_from(index).ok();
        }
    }
    None
}
fn form_values(
    table: &[&Tool],
    arguments: &[Argument],
    words: &[&[u8]],
    limits: &Limits,
    keyed: Keyed,
    json: bool,
) -> Result<Form, Usage> {
    let count = u8::try_from(arguments.len()).or(Err(Usage::Tool))?;
    let mut values = List::with_capacity(u32::from(count));
    for _ in arguments {
        values.push(None).or(Err(Usage::Tool))?;
    }
    let mut result = Form { values: Box::from([]), key: None, json };
    let mut skip = None;
    let mut positional = false;
    for (index, word) in words.iter().enumerate() {
        if skip == Some(index) {
            continue;
        }
        if !positional && *word == b"--" {
            positional = true;
            continue;
        }
        let at = u16::try_from(index).or(Err(Usage::Tool))?;
        let flag = !positional && word.starts_with(b"--");
        if flag && *word == b"--json" {
            if result.json {
                return Err(Usage::Json);
            }
            result.json = true;
            continue;
        }
        if flag && *word == b"--key" && keyed != Keyed::Never {
            if result.key.is_some() {
                return Err(Usage::Key);
            }
            let next = index.checked_add(1).ok_or(Usage::Tool)?;
            result.key = Some(Name::new(words.get(next).ok_or(Usage::Key)?).ok_or(Usage::Key)?);
            skip = Some(next);
            continue;
        }
        let mut found = None;
        for (slot, argument) in arguments.iter().enumerate() {
            let id = u8::try_from(slot).or(Err(Usage::Tool))?;
            if flag {
                if argument.place != Place::Positional && word.get(2..) == Some(argument.name) {
                    found = Some(id);
                    break;
                }
            } else if argument.place == Place::Positional && values.get(u32::from(id)) == Some(&None) {
                found = Some(id);
                break;
            }
        }
        let argument = found.ok_or(if flag { Usage::Unknown { at } } else { Usage::Extra { at } })?;
        let spec = arguments.get(usize::from(argument)).ok_or(Usage::Tool)?;
        let slot = values.get_mut(u32::from(argument)).ok_or(Usage::Tool)?;
        if slot.is_some() && !repeated(spec.occurs) {
            return Err(Usage::Twice { argument });
        }
        let value = if spec.place == Place::Switch {
            Value::Switch
        } else {
            let text = if flag {
                let next = index.checked_add(1).ok_or(Usage::Tool)?;
                skip = Some(next);
                *words.get(next).ok_or(Usage::Missing { argument })?
            } else {
                *word
            };
            decode(table, spec.kind, bound(spec, limits), text).ok_or(if spec.kind == Kind::Condition {
                Usage::Condition
            } else {
                Usage::Kind { argument }
            })?
        };
        valid(table, spec, &value, argument, limits)?;
        store(slot, spec.occurs, value, argument)?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        let index = u32::try_from(index).or(Err(Usage::Tool))?;
        let slot = values.get_mut(index).ok_or(Usage::Tool)?;
        if slot.is_none() {
            if argument.occurs == Occurs::Required {
                return Err(Usage::Missing { argument: u8::try_from(index).or(Err(Usage::Tool))? });
            }
            if argument.kind == Kind::Order {
                *slot = Some(Values::One(Value::Order(QueryOrder::Number)));
            }
        }
    }
    result.values = values.into_boxed();
    defaults(arguments, &mut result.values, limits);
    Ok(result)
}
fn repeated(occurs: Occurs) -> bool {
    match occurs {
        Occurs::Repeated { .. } => true,
        Occurs::Required | Occurs::Optional => false,
    }
}
/// The same validation is used for typed JSON calls.
pub fn check(table: &[&Tool], call: &Call, limits: &Limits) -> Result<(), Usage> {
    let tool = table.get(usize::from(call.tool)).ok_or(Usage::Tool)?;
    let args = *tool.forms.get(usize::from(call.form)).ok_or(Usage::Tool)?;
    match tool.keyed {
        Keyed::Never if call.key.is_some() => return Err(Usage::Key),
        Keyed::Required if call.key.is_none() => return Err(Usage::Key),
        Keyed::Never | Keyed::Optional | Keyed::Required => {}
    }
    check_values(table, args, &call.values, limits)?;
    if tool.name == super::rows::HELP.name && !help_valid(table, call) {
        return Err(Usage::Tool);
    }
    // The creation row owns this rule; reading a chat has no keyed choice form.
    if tool.name == super::rows::CHAT_CREATE.name && tool.keyed == Keyed::Required {
        check_choice(call, 2, 3, limits)?;
    }
    if tool.name == b"task amend" {
        check_amend(call, limits)?;
    }
    if tool.name == b"forge connect" && call.form == 0 && call.values.contains(&Some(Values::One(Value::Stdin))) {
        return Err(Usage::Stdin);
    }
    if tool.area == 5 && (tool.keyed != Keyed::Never || tool.name == b"account export") {
        for (index, (arg, slot)) in args.iter().zip(&call.values).enumerate() {
            if let Some(Values::One(Value::Number(n))) = slot
                && ((*n == 0 && arg.name == b"position") || u32::try_from(*n).is_err())
            {
                return Err(Usage::Kind { argument: u8::try_from(index).expect("argument") });
            }
            if arg.kind == Kind::File && *slot == Some(Values::One(Value::Stdin)) {
                return Err(Usage::Stdin);
            }
        }
    }
    if tool.name == b"account declare" {
        for (index, bound) in [(1, crate::jig::call::PROGRAM_ADAPTER), (2, crate::jig::call::PROGRAM_NAME)] {
            let argument = u8::try_from(index).expect("argument");
            let Some(Value::Text(bytes)) = crate::jig::call::value(call, index) else {
                return Err(Usage::Kind { argument });
            };
            if bytes.len() > usize::try_from(bound).expect("name bound fits") {
                return Err(Usage::Long { argument });
            }
            if index == 2 && !jig_client::contract::valid_program_name(bytes) {
                return Err(Usage::Kind { argument });
            }
        }
    }
    if tool.name == b"inbox" {
        check_inbox(call)?;
    }
    check_watch(tool, call)?;
    Ok(())
}
/// Defaults are added only to waiting reads; a bare read has no timer.
pub(crate) fn defaults(args: &[Argument], values: &mut [Option<Values>], limits: &Limits) {
    let mut waiting = false;
    for (arg, value) in args.iter().zip(values.iter()) {
        waiting |= (arg.name == b"since" || arg.name == b"until") && value.is_some();
    }
    if waiting {
        for (arg, value) in args.iter().zip(values) {
            if arg.name == b"timeout" && value.is_none() {
                *value = Some(Values::One(Value::Duration(limits.wait)));
            }
        }
    }
}
fn incompatible(row: &Tool, name: &[u8]) -> bool {
    match row.name {
        b"inbox" => name == b"project" || name == b"kind" || name == b"root" || name == b"after",
        b"task show" => name == b"tree" || name == b"activity" || name == b"run",
        b"task list" => name == b"after" || name == b"ended",
        _ => false,
    }
}
// Check raw flags before decoding values: even an invalid part's value must
// name the first incompatible flag in the order the caller supplied it.
fn check_words(row: &Tool, tool: u8, args: &[Argument], words: &[&[u8]]) -> Result<(), Usage> {
    let mut watched = false;
    for arg in args {
        watched |= arg.name == b"since";
    }
    if !watched {
        return Ok(());
    }
    let mut since = false;
    let mut until = false;
    let mut timeout = false;
    let mut part = None;
    let mut skip = false;
    for word in words {
        if skip {
            skip = false;
            continue;
        }
        if *word == b"--" {
            break;
        }
        let Some(name) = word.strip_prefix(b"--") else { continue };
        since |= name == b"since";
        until |= name == b"until";
        timeout |= name == b"timeout";
        for (index, arg) in args.iter().enumerate() {
            if arg.name == name {
                skip = arg.place == Place::Flag;
                if part.is_none() && incompatible(row, name) {
                    part = Some(u8::try_from(index).expect("bounded arguments"));
                }
                break;
            }
        }
    }
    if !since && !until {
        if timeout {
            return Err(Usage::Timeout);
        }
    } else if let Some(argument) = part {
        return Err(Usage::WaitPart { tool, argument });
    }
    Ok(())
}
fn check_watch(row: &Tool, call: &Call) -> Result<(), Usage> {
    let args = row.forms.get(usize::from(call.form)).ok_or(Usage::Tool)?;
    let mut watched = false;
    for arg in *args {
        watched |= arg.name == b"since";
    }
    if !watched {
        return Ok(());
    }
    if !super::call::waiting(row, call) {
        return if super::call::argument(row, call, b"timeout").is_some() { Err(Usage::Timeout) } else { Ok(()) };
    }
    for (index, (arg, value)) in args.iter().zip(&call.values).enumerate() {
        if value.is_some() && incompatible(row, arg.name) {
            return Err(Usage::WaitPart { tool: call.tool, argument: u8::try_from(index).expect("bounded arguments") });
        }
    }
    if let Some(Value::Condition(condition)) = super::call::argument(row, call, b"until") {
        let subject = subject::of(call).ok_or(Usage::Condition)?;
        subject::check(&subject, *condition)?;
    }
    Ok(())
}
fn check_values(table: &[&Tool], args: &[Argument], values: &[Option<Values>], limits: &Limits) -> Result<(), Usage> {
    if args.len() != values.len() {
        return Err(Usage::Tool);
    }
    let mut stdin = false;
    for (index, (argument, values)) in args.iter().zip(values).enumerate() {
        let id = u8::try_from(index).or(Err(Usage::Tool))?;
        match values {
            None if argument.occurs == Occurs::Required => return Err(Usage::Missing { argument: id }),
            None => {}
            Some(Values::One(value)) => {
                if repeated(argument.occurs) {
                    return Err(Usage::Many { argument: id });
                }
                checked_value(table, argument, value, id, limits, &mut stdin)?;
            }
            Some(Values::Many(values)) => {
                let Occurs::Repeated { most } = argument.occurs else {
                    return Err(Usage::Many { argument: id });
                };
                if values.is_empty() || u32::try_from(values.len()).unwrap_or(u32::MAX) > most {
                    return Err(Usage::Many { argument: id });
                }
                for value in values {
                    checked_value(table, argument, value, id, limits, &mut stdin)?;
                }
            }
        }
    }
    Ok(())
}
fn checked_value(
    table: &[&Tool],
    argument: &Argument,
    value: &Value,
    id: u8,
    limits: &Limits,
    stdin: &mut bool,
) -> Result<(), Usage> {
    valid(table, argument, value, id, limits)?;
    if *value == Value::Stdin {
        if *stdin {
            return Err(Usage::Stdin);
        }
        *stdin = true;
    }
    Ok(())
}
/// Write positionals followed by flags, in the row's order.
pub fn back(table: &[&Tool], call: &Call, out: &mut List<Box<[u8]>>) -> Result<(), Unwritten> {
    let row = table.get(usize::from(call.tool)).ok_or(Unwritten::NoForm)?;
    let args = row.forms.get(usize::from(call.form)).ok_or(Unwritten::NoForm)?;
    if args.len() != call.values.len() {
        return Err(Unwritten::NoForm);
    }
    let (group, verb) = super::table::name_words(row.name);
    push(out, group)?;
    if let Some(verb) = verb {
        push(out, verb)?;
    }
    let delayed = escaped_position(args, &call.values, table)?;
    for positional in [true, false] {
        for (index, (argument, slot)) in args.iter().zip(&call.values).enumerate() {
            if positional && delayed <= index {
                continue;
            }
            if (argument.place == Place::Positional) != positional {
                continue;
            }
            match slot {
                Some(Values::One(value)) => emit(table, argument, value, out)?,
                Some(Values::Many(values)) => {
                    for value in values {
                        emit(table, argument, value, out)?;
                    }
                }
                None => {}
            }
        }
    }
    if let Some(name) = &call.key {
        push(out, b"--key")?;
        push(out, name.bytes())?;
    }
    if delayed < args.len() {
        push(out, b"--")?;
        for (argument, slot) in args.iter().zip(&call.values).skip(delayed) {
            if argument.place != Place::Positional {
                continue;
            }
            match slot {
                Some(Values::One(value)) => push(out, &encode(table, argument.kind, value)?)?,
                None => {}
                Some(Values::Many(_)) => return Err(Unwritten::NoForm),
            }
        }
    }
    Ok(())
}
fn escaped_position(args: &[Argument], values: &[Option<Values>], table: &[&Tool]) -> Result<usize, Unwritten> {
    for (index, (argument, slot)) in args.iter().zip(values).enumerate() {
        if argument.place != Place::Positional {
            continue;
        }
        let Some(values) = slot else {
            continue;
        };
        let Values::One(value) = values else {
            return Err(Unwritten::NoForm);
        };
        if encode(table, argument.kind, value)?.starts_with(b"--") {
            return Ok(index);
        }
    }
    Ok(args.len())
}
fn emit(table: &[&Tool], argument: &Argument, value: &Value, out: &mut List<Box<[u8]>>) -> Result<(), Unwritten> {
    if *value == Value::Order(QueryOrder::Number) {
        return Ok(());
    }
    if *value == Value::Order(QueryOrder::Changed) || *value == Value::Order(QueryOrder::Ended) {
        return Err(Unwritten::NoForm);
    }
    if argument.place != Place::Positional {
        let mut flag = Writer::new(argument.name.len().checked_add(2).ok_or(Unwritten::Overflow(Overflow))?);
        written(flag.put(b"--"))?;
        written(flag.put(argument.name))?;
        push(out, &flag.finish())?;
    }
    if argument.place != Place::Switch {
        let word = encode(table, argument.kind, value)?;
        if argument.place == Place::Positional && word.starts_with(b"--") {
            push(out, b"--")?;
        }
        push(out, &word)?;
    }
    Ok(())
}
fn push(out: &mut List<Box<[u8]>>, bytes: &[u8]) -> Result<(), Unwritten> {
    out.push(Box::from(bytes)).or(Err(Unwritten::Overflow(Overflow)))
}
fn written(result: Result<(), Overflow>) -> Result<(), Unwritten> {
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(Unwritten::Overflow(error)),
    }
}

fn store(slot: &mut Option<Values>, occurs: Occurs, value: Value, argument: u8) -> Result<(), Usage> {
    match occurs {
        Occurs::Repeated { most } => {
            let mut many = List::with_capacity(most);
            match slot {
                Some(Values::Many(old)) => {
                    for value in old.iter() {
                        many.push(value.clone()).or(Err(Usage::Many { argument }))?;
                    }
                }
                Some(Values::One(_)) => return Err(Usage::Many { argument }),
                None => {}
            }
            many.push(value).or(Err(Usage::Many { argument }))?;
            *slot = Some(Values::Many(many.into_boxed()));
        }
        Occurs::Required | Occurs::Optional => *slot = Some(Values::One(value)),
    }
    Ok(())
}

fn check_inbox(call: &Call) -> Result<(), Usage> {
    use jig_client::contract::InboxReadGroup;
    let Some(Value::Cursor(bytes)) = crate::jig::call::value(call, 3) else { return Ok(()) };
    let group = match bytes.first() {
        Some(0) => InboxReadGroup::Needs,
        Some(1) => InboxReadGroup::Updates,
        _ => return Err(Usage::Kind { argument: 3 }),
    };
    if let Some(Value::Group(kind)) = crate::jig::call::value(call, 1)
        && InboxReadGroup::of(*kind) != group
    {
        return Err(Usage::Kind { argument: 3 });
    }
    Ok(())
}

/// argv is protocol input: validate names without allocating another copy.
#[expect(clippy::disallowed_methods, reason = "the model/effort argv rule validates UTF-8 at the input boundary")]
fn check_choice(call: &Call, model: usize, effort: usize, limits: &Limits) -> Result<(), Usage> {
    for index in [model, effort] {
        let argument = u8::try_from(index).or(Err(Usage::Tool))?;
        if let Some(value) = crate::jig::call::value(call, index) {
            let Value::Text(text) = value else { return Err(Usage::Kind { argument }) };
            if text.len() > usize::try_from(limits.text).expect("text bound fits") {
                return Err(Usage::Long { argument });
            }
            if core::str::from_utf8(text).is_err() {
                return Err(Usage::Kind { argument });
            }
        }
    }
    if crate::jig::call::value(call, effort).is_some() && crate::jig::call::value(call, model).is_none() {
        let argument = u8::try_from(model).or(Err(Usage::Tool))?;
        return Err(Usage::Missing { argument });
    }
    Ok(())
}

fn present(call: &Call, index: usize) -> bool {
    match call.values.get(index) {
        Some(Some(_)) => true,
        Some(None) | None => false,
    }
}
fn check_amend(call: &Call, limits: &Limits) -> Result<(), Usage> {
    check_choice(call, 10, 11, limits)?;
    let mut change = false;
    let mut needs_revision = false;
    for index in 2..12 {
        if present(call, index) {
            change = true;
            needs_revision |= index != 2;
        }
    }
    if needs_revision && crate::jig::call::number(call, 1).is_none() {
        return Err(Usage::AmendmentRevision);
    }
    if !change {
        return Err(Usage::Missing { argument: 2 });
    }
    for (first, second) in [(2, 3), (4, 5), (8, 9)] {
        if present(call, first) && present(call, second) {
            return Err(Usage::Twice { argument: u8::try_from(second).expect("amend argument") });
        }
    }
    if let Some(Value::Text(text)) = crate::jig::call::value(call, 4) {
        deadline(text).ok_or(Usage::Kind { argument: 4 })?;
    }
    if let Some(priority) = crate::jig::call::number(call, 2) {
        u32::try_from(priority).or(Err(Usage::Kind { argument: 2 }))?;
    }
    Ok(())
}
/// UTC Gregorian date to the minute, representable by Wall's nanoseconds.
#[must_use]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "four-digit years after 1970 and validated calendar fields bound every calendar intermediate"
)]
pub fn deadline(text: &[u8]) -> Option<skein_lib::Wall> {
    if text.len() != 17
        || text.get(4) != Some(&b'-')
        || text.get(7) != Some(&b'-')
        || text.get(10) != Some(&b'T')
        || text.get(13) != Some(&b':')
        || text.get(16) != Some(&b'Z')
    {
        return None;
    }
    let year = value::number(text.get(..4)?)?;
    let month = value::number(text.get(5..7)?)?;
    let day = value::number(text.get(8..10)?)?;
    let hour = value::number(text.get(11..13)?)?;
    let minute = value::number(text.get(14..16)?)?;
    if year < 1970 || !(1..=12).contains(&month) || hour > 23 || minute > 59 {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let lengths = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day == 0 || day > *lengths.get(usize::try_from(month.checked_sub(1)?).ok()?)? {
        return None;
    }
    let before = year.checked_sub(1)?;
    let mut days = (year - 1970).checked_mul(365)? + (before / 4 - 1969 / 4) - (before / 100 - 1969 / 100)
        + (before / 400 - 1969 / 400);
    for length in lengths.iter().take(usize::try_from(month.checked_sub(1)?).ok()?) {
        days = days.checked_add(*length)?;
    }
    let seconds = days
        .checked_add(day.checked_sub(1)?)?
        .checked_mul(86400)?
        .checked_add(hour.checked_mul(3600)?)?
        .checked_add(minute.checked_mul(60)?)?;
    Some(skein_lib::Wall::from_nanos(seconds.checked_mul(1_000_000_000)?))
}

fn help_valid(table: &[&Tool], call: &Call) -> bool {
    let group = match super::call::value(call, 0) {
        Some(Value::Text(group)) => group,
        None => return super::call::value(call, 1).is_none(),
        Some(_) => return false,
    };
    let Some(row) = first_word(table, group) else { return false };
    match super::call::value(call, 1) {
        Some(Value::Text(verb)) => row.name != group.as_ref() && named_index(table, group, verb).is_some(),
        None => true,
        Some(_) => false,
    }
}
