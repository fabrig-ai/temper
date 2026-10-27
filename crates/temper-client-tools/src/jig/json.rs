//! Calls and tool records derived from the table (tools contract, 6.2, 8.1).
use crate::jig::{
    argv::{self, Usage},
    call::{Call, Source, Value, Values},
    key::Name,
    table::{Argument, Bound, Keyed, Kind, Limits, Occurs, Place, Tool, bound},
};
use alloc::boxed::Box;
use jig_client::contract::QueryOrder;
use skein_json::{
    Document, Kind as JsonKind, tokenizer,
    writer::{self, Encoder},
};
use skein_lib::{
    Env, Intake, List, Queue, Time, Wall, Writer,
    stream::{Down, Up},
};
/// An encoded record exceeds its bound or contains non-UTF-8 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    Long,
    Text,
}
/// Decode and check one call; stdin may be used only once.
pub fn call(table: &[&Tool], bytes: &[u8], from: Source, limits: &Limits) -> Result<Call, Usage> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.ask_bytes {
        return Err(Usage::Json);
    }
    let doc = document(bytes)?;
    let fields = members(&doc, 0)?;
    let mut name = None;
    let mut arguments = None;
    for (key, at) in &fields {
        match text(&doc, *key)? {
            b"tool" => {
                require(&doc, *at, JsonKind::String)?;
                name = Some(text(&doc, *at)?);
            }
            b"arguments" => arguments = Some(members(&doc, *at)?),
            _ => return Err(Usage::Json),
        }
    }
    let name = name.ok_or(Usage::Json)?;
    let arguments = arguments.ok_or(Usage::Json)?;
    let mut error = Usage::Tool;
    for (index, tool) in table.iter().enumerate() {
        if tool.name != name {
            continue;
        }
        for (form, specs) in tool.forms.iter().enumerate() {
            match decode(table, &doc, &arguments, specs, limits) {
                Ok(Decoded { values, key }) => {
                    let call = Call {
                        tool: u8::try_from(index).or(Err(Usage::Tool))?,
                        form: u8::try_from(form).or(Err(Usage::Tool))?,
                        values,
                        key,
                    };
                    match argv::check(table, &call, limits) {
                        Ok(()) => {
                            if from == Source::Stdin {
                                for slot in &call.values {
                                    if *slot == Some(Values::One(Value::Stdin)) {
                                        return Err(Usage::Stdin);
                                    }
                                }
                            }
                            return Ok(call);
                        }
                        Err(failed) => error = failed,
                    }
                }
                Err(failed) => error = failed,
            }
        }
    }
    Err(error)
}
fn document(bytes: &[u8]) -> Result<Document, Usage> {
    document_depth(bytes, 3)
}
pub(crate) fn document_depth(bytes: &[u8], depth: u32) -> Result<Document, Usage> {
    let length = u32::try_from(bytes.len()).or(Err(Usage::Json))?;
    if length == u32::MAX {
        return Err(Usage::Json);
    }
    let limits = tokenizer::Limits { depth, string: length, number: 20, chunk: length.max(1), length };
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut machine = tokenizer::Tokenizer::new(&limits);
    let mut intake = Intake::with_capacity(length.max(tokenizer::largest_demand(&limits)));
    intake.append(bytes).or(Err(Usage::Json))?;
    let mut above = Queue::with_capacity(1);
    let mut below = Queue::with_capacity(1);
    let mut tokens = List::with_capacity(length);
    let mut demanded = None;
    let steps = bytes.len().checked_mul(4).ok_or(Usage::Json)?.checked_add(8).ok_or(Usage::Json)?;
    for _ in 0..steps {
        match demanded.take() {
            Some(read) => match intake.meet(read) {
                Some(bytes) => tokenizer::up(&mut machine, &env, Up::Bytes(bytes), &mut above, &mut below),
                None => tokenizer::up(&mut machine, &env, Up::End, &mut above, &mut below),
            },
            None => tokenizer::down(&mut machine, &env, tokenizer::Request::Next, &mut above, &mut below),
        }
        match below.pop() {
            Some(Down::Demand { read, .. }) => demanded = Some(read),
            Some(Down::Send(_) | Down::Finish) => return Err(Usage::Json),
            None => {}
        }
        match above.pop() {
            Some(tokenizer::Event::Token(token)) => tokens.push(token).or(Err(Usage::Json))?,
            Some(tokenizer::Event::Done) => {
                return Document::from_tokens(
                    tokens.as_slice(),
                    &skein_json::document::Limits { tokens: length, text: length },
                )
                .or(Err(Usage::Json));
            }
            Some(
                tokenizer::Event::Failed(_)
                | tokenizer::Event::Closed
                | tokenizer::Event::Long(_)
                | tokenizer::Event::Skipped(_),
            ) => return Err(Usage::Json),
            None => {}
        }
    }
    Err(Usage::Json)
}
fn require(doc: &Document, at: u32, kind: JsonKind) -> Result<(), Usage> {
    if doc.token(at).ok_or(Usage::Json)?.kind == kind { Ok(()) } else { Err(Usage::Json) }
}
fn text(doc: &Document, at: u32) -> Result<&[u8], Usage> {
    doc.text(doc.token(at).ok_or(Usage::Json)?).ok_or(Usage::Json)
}
/// Direct member positions; structural depth, never Compact's text end.
fn members(doc: &Document, start: u32) -> Result<List<(u32, u32)>, Usage> {
    require(doc, start, JsonKind::ObjectStart)?;
    let mut fields = List::with_capacity(doc.len());
    let mut depth = 0_u32;
    for at in start.checked_add(1).ok_or(Usage::Json)?..doc.len() {
        match doc.token(at).ok_or(Usage::Json)?.kind {
            JsonKind::Key if depth == 0 => {
                for (key, _) in &fields {
                    if text(doc, *key)? == text(doc, at)? {
                        return Err(Usage::Json);
                    }
                }
                fields.push((at, at.checked_add(1).ok_or(Usage::Json)?)).or(Err(Usage::Json))?;
            }
            JsonKind::ObjectStart | JsonKind::ArrayStart => depth = depth.checked_add(1).ok_or(Usage::Json)?,
            JsonKind::ObjectEnd | JsonKind::ArrayEnd => {
                if depth == 0 {
                    return Ok(fields);
                }
                depth = depth.checked_sub(1).ok_or(Usage::Json)?;
            }
            JsonKind::Key
            | JsonKind::String
            | JsonKind::Number
            | JsonKind::True
            | JsonKind::False
            | JsonKind::Null
            | JsonKind::Long => {}
        }
    }
    Err(Usage::Json)
}
struct Decoded {
    values: Box<[Option<Values>]>,
    key: Option<Name>,
}
fn decode(
    table: &[&Tool],
    doc: &Document,
    fields: &List<(u32, u32)>,
    specs: &[Argument],
    limits: &Limits,
) -> Result<Decoded, Usage> {
    let mut values = List::with_capacity(u32::try_from(specs.len()).or(Err(Usage::Json))?);
    let mut key = None;
    for (name, at) in fields {
        let name = text(doc, *name)?;
        let mut known = name == b"key";
        if name == b"key" {
            require(doc, *at, JsonKind::String)?;
            key = Some(Name::new(text(doc, *at)?).ok_or(Usage::Key)?);
        }
        for spec in specs {
            known |= argument_name(spec.name).as_ref() == name;
        }
        if !known {
            return Err(Usage::Json);
        }
    }
    for spec in specs {
        let mut found = None;
        for (key, at) in fields {
            if text(doc, *key)? == argument_name(spec.name).as_ref() {
                found = Some(*at);
            }
        }
        let value = match found {
            Some(at) => Some(decode_values(table, doc, at, spec, limits)?),
            None if spec.kind == Kind::Order => Some(Values::One(Value::Order(QueryOrder::Number))),
            None if spec.name == b"timeout" => Some(Values::One(Value::Duration(limits.wait))),
            None => None,
        };
        values.push(value).or(Err(Usage::Json))?;
    }
    Ok(Decoded { values: values.into_boxed(), key })
}
fn decode_values(table: &[&Tool], doc: &Document, at: u32, spec: &Argument, limits: &Limits) -> Result<Values, Usage> {
    let Occurs::Repeated { most } = spec.occurs else {
        return Ok(Values::One(decode_value(table, doc, at, spec, limits)?));
    };
    require(doc, at, JsonKind::ArrayStart)?;
    let mut values = List::with_capacity(most);
    for index in at.checked_add(1).ok_or(Usage::Json)?..doc.len() {
        if doc.token(index).ok_or(Usage::Json)?.kind == JsonKind::ArrayEnd {
            return Ok(Values::Many(values.into_boxed()));
        }
        let value = decode_value(table, doc, index, spec, limits)?;
        values.push(value).or(Err(Usage::Json))?;
    }
    Err(Usage::Json)
}
fn decode_value(table: &[&Tool], doc: &Document, at: u32, spec: &Argument, limits: &Limits) -> Result<Value, Usage> {
    if spec.place == Place::Switch {
        require(doc, at, JsonKind::True)?;
        return Ok(Value::Switch);
    }
    let kind = doc.token(at).ok_or(Usage::Json)?.kind;
    match spec.kind {
        Kind::Number | Kind::NumberRange { .. } | Kind::Count | Kind::Duration => require(doc, at, JsonKind::Number)?,
        Kind::Project { .. } if kind == JsonKind::Number => {}
        Kind::Word { .. }
        | Kind::Text
        | Kind::NamedNumber { .. }
        | Kind::Literal
        | Kind::File
        | Kind::Proposal
        | Kind::Digest
        | Kind::Condition
        | Kind::Subject
        | Kind::Project { .. }
        | Kind::Turn
        | Kind::Cursor
        | Kind::Group
        | Kind::Order
        | Kind::Tool => require(doc, at, JsonKind::String)?,
    }
    let bytes = text(doc, at)?;
    // Amendment FILE arguments carry their loaded words in a JSON call.
    if spec.kind == Kind::File && (spec.name == b"instructions" || spec.name == b"spec" || spec.name == b"items") {
        return Ok(Value::Text(Box::from(bytes)));
    }
    if spec.kind == Kind::Subject {
        let mut words = List::with_capacity(u32::try_from(bytes.len()).or(Err(Usage::Json))?);
        let mut start = 0_usize;
        for at in 0..=bytes.len() {
            let boundary = match bytes.get(at) {
                Some(byte) => byte.is_ascii_whitespace(),
                None => true,
            };
            if boundary {
                if start < at {
                    words.push(bytes.get(start..at).ok_or(Usage::Json)?).or(Err(Usage::Json))?;
                }
                start = at.checked_add(1).ok_or(Usage::Json)?;
            }
        }
        let (subject, outer) = argv::watch(table, words.as_slice(), limits)?;
        if outer.len() != 1 {
            return Err(Usage::Json);
        }
        return Ok(Value::Subject(subject));
    }
    // JSON durations are integer seconds, never argv's suffixed words.
    if spec.kind == Kind::Duration || kind == JsonKind::Number {
        for byte in bytes {
            if !byte.is_ascii_digit() {
                return Err(Usage::Json);
            }
        }
    }
    argv::value::decode(table, spec.kind, bound(spec, limits), bytes).ok_or(Usage::Json)
}
/// Measure then encode a checked call, with its row and form unchanged.
pub fn write_call(table: &[&Tool], call: &Call) -> Result<Box<[u8]>, Refusal> {
    let limits = writer::Limits { depth: 3, length: u32::MAX };
    let mut measured = Encoder::measure(&limits);
    encode_call(table, call, &mut measured)?;
    let length = measured_length(measured)?;
    let mut output = Encoder::write(length, &limits);
    encode_call(table, call, &mut output)?;
    Ok(output.finish())
}
fn measured_length(encoder: Encoder) -> Result<u32, Refusal> {
    match encoder.measured() {
        Ok(length) => Ok(length),
        Err(writer::Refusal::TooLong | writer::Refusal::TooDeep) => Err(Refusal::Long),
        Err(writer::Refusal::Text | writer::Refusal::Number) => Err(Refusal::Text),
    }
}
fn encode_call(table: &[&Tool], call: &Call, out: &mut Encoder) -> Result<(), Refusal> {
    let row = table.get(usize::from(call.tool)).ok_or(Refusal::Text)?;
    let specs = row.forms.get(usize::from(call.form)).ok_or(Refusal::Text)?;
    out.object_start();
    out.key(b"tool");
    out.string(row.name);
    out.key(b"arguments");
    out.object_start();
    for (spec, values) in specs.iter().zip(&call.values) {
        match values {
            Some(Values::One(Value::Order(QueryOrder::Number))) | None => {}
            Some(Values::One(value)) => {
                out.key(&argument_name(spec.name));
                encode_value(table, spec, value, out)?;
            }
            Some(Values::Many(values)) => {
                out.key(&argument_name(spec.name));
                out.array_start();
                for value in values {
                    encode_value(table, spec, value, out)?;
                }
                out.array_end();
            }
        }
    }
    if let Some(key) = &call.key {
        out.key(b"key");
        out.string(key.bytes());
    }
    out.object_end();
    out.object_end();
    Ok(())
}
fn encode_value(table: &[&Tool], spec: &Argument, value: &Value, out: &mut Encoder) -> Result<(), Refusal> {
    match value {
        Value::Switch => out.boolean(true),
        Value::Number(n) => out.unsigned(*n),
        Value::Project(crate::jig::call::Project::Number(n)) => out.unsigned(u64::from(*n)),
        Value::Duration(d) => out.unsigned(d.as_nanos().checked_div(1_000_000_000).ok_or(Refusal::Text)?),
        Value::Subject(subject) => {
            let mut words = List::with_capacity(256);
            argv::write_subject(table, subject, &mut words).or(Err(Refusal::Text))?;
            let mut size = 0_usize;
            for word in &words {
                size = size.checked_add(word.len()).ok_or(Refusal::Long)?;
            }
            let mut text = Writer::new(
                size.checked_add(usize::try_from(words.len()).or(Err(Refusal::Long))?.saturating_sub(1))
                    .ok_or(Refusal::Long)?,
            );
            for (index, word) in words.iter().enumerate() {
                if index != 0 {
                    text.put(b" ").or(Err(Refusal::Long))?;
                }
                text.put(word).or(Err(Refusal::Long))?;
            }
            out.string(&text.finish());
        }
        Value::Word(_)
        | Value::Text(_)
        | Value::Stdin
        | Value::File(_)
        | Value::Proposal(_)
        | Value::Digest(_)
        | Value::Condition(_)
        | Value::Project(_)
        | Value::Turn(_)
        | Value::Cursor(_)
        | Value::Group(_)
        | Value::Order(_)
        | Value::Tool(_) => {
            let bytes = argv::value::encode(table, spec.kind, value).or(Err(Refusal::Text))?;
            out.string(&bytes);
        }
    }
    Ok(())
}
/// All table fields, including every form, for help TOOL --json.
pub fn write_tool(tool: &Tool, limits: &Limits) -> Result<Box<[u8]>, Refusal> {
    let encoding = writer::Limits { depth: 5, length: u32::MAX };
    let mut measured = Encoder::measure(&encoding);
    encode_tool(tool, limits, &mut measured);
    let length = measured_length(measured)?;
    let mut output = Encoder::write(length, &encoding);
    encode_tool(tool, limits, &mut output);
    Ok(output.finish())
}
fn encode_tool(tool: &Tool, limits: &Limits, out: &mut Encoder) {
    out.object_start();
    for (key, text) in [
        (b"name".as_slice(), tool.name),
        (b"line", tool.line),
        (b"prints", tool.prints),
        (b"notes", tool.notes),
        (b"result", tool.result),
    ] {
        out.key(key);
        out.string(text);
    }
    out.key(b"area");
    out.unsigned(u64::from(tool.area));
    out.key(b"keyed");
    out.string(match tool.keyed {
        Keyed::Never => b"never",
        Keyed::Optional => b"optional",
        Keyed::Required => b"required",
    });
    out.key(b"forms");
    out.array_start();
    for form in tool.forms {
        out.array_start();
        for spec in *form {
            out.object_start();
            let name = argument_name(spec.name);
            for (key, text) in [(b"name".as_slice(), name.as_ref()), (b"shown", spec.shown), (b"line", spec.line)] {
                out.key(key);
                out.string(text);
            }
            out.key(b"place");
            out.string(match spec.place {
                Place::Positional => b"positional",
                Place::Flag => b"flag",
                Place::Switch => b"switch",
            });
            out.key(b"kind");
            out.string(kind_word(spec.kind));
            out.key(b"bound");
            encode_bound(bound(spec, limits), out);
            match spec.kind {
                Kind::Word { set } => {
                    out.key(b"set");
                    out.array_start();
                    for word in set {
                        out.string(word);
                    }
                    out.array_end();
                }
                Kind::NumberRange { min, max } => {
                    out.key(b"min");
                    out.unsigned(min);
                    out.key(b"max");
                    out.unsigned(max);
                }
                Kind::NamedNumber { words, prefixes, min, max } => {
                    out.key(b"set");
                    out.array_start();
                    for word in words {
                        out.string(word);
                    }
                    out.array_end();
                    prefixes_json(prefixes, out);
                    out.key(b"min");
                    out.unsigned(min);
                    out.key(b"max");
                    out.unsigned(max);
                }
                Kind::Number
                | Kind::Count
                | Kind::Text
                | Kind::Literal
                | Kind::File
                | Kind::Duration
                | Kind::Proposal
                | Kind::Digest
                | Kind::Condition
                | Kind::Subject
                | Kind::Project { .. }
                | Kind::Turn
                | Kind::Cursor
                | Kind::Group
                | Kind::Order
                | Kind::Tool => {}
            }
            out.key(b"occurs");
            out.string(match spec.occurs {
                Occurs::Required => b"required",
                Occurs::Optional => b"optional",
                Occurs::Repeated { .. } => b"repeated",
            });
            match spec.occurs {
                Occurs::Repeated { most } => {
                    out.key(b"most");
                    out.unsigned(u64::from(most));
                }
                Occurs::Required | Occurs::Optional => {}
            }
            out.object_end();
        }
        out.array_end();
    }
    out.array_end();
    out.object_end();
}
fn kind_word(kind: Kind) -> &'static [u8] {
    match kind {
        Kind::Number | Kind::NumberRange { .. } => b"number",
        Kind::Count => b"count",
        Kind::Word { .. } => b"word",
        Kind::Text => b"text",
        Kind::Literal => b"literal",
        Kind::File => b"file",
        Kind::Duration => b"duration",
        Kind::Proposal => b"proposal",
        Kind::Digest => b"digest",
        Kind::NamedNumber { .. } => b"string",
        Kind::Condition => b"condition",
        Kind::Subject => b"subject",
        Kind::Project { .. } => b"project",
        Kind::Turn => b"turn",
        Kind::Cursor => b"cursor",
        Kind::Group => b"group",
        Kind::Order => b"order",
        Kind::Tool => b"tool",
    }
}

/// JSON member names use snake case; argv keeps its flag spelling.
fn argument_name(name: &[u8]) -> Box<[u8]> {
    let mut out = Writer::new(name.len());
    for byte in name {
        out.put(&[if *byte == b'-' { b'_' } else { *byte }]).expect("one byte per name byte");
    }
    out.finish()
}

fn prefixes_json(prefixes: &[&[u8]], out: &mut Encoder) {
    out.key(b"prefixes");
    out.array_start();
    for prefix in prefixes {
        out.string(prefix);
    }
    out.array_end();
}

fn encode_bound(bound: Bound, out: &mut Encoder) {
    match bound {
        Bound::None => out.null(),
        Bound::Numbers { least, most } => {
            out.object_start();
            out.key(b"least");
            out.unsigned(least);
            out.key(b"most");
            out.unsigned(most);
            out.object_end();
        }
        Bound::Bytes { most } => {
            out.object_start();
            out.key(b"bytes");
            out.unsigned(most);
            out.object_end();
        }
        Bound::Seconds { most } => {
            out.object_start();
            out.key(b"seconds");
            out.unsigned(most);
            out.object_end();
        }
    }
}
