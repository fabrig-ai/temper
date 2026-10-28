use super::{Choice, Default, FIELDS, FORMS, Field, Form, Presence, Value, Width, Word, most};
use crate::{Limits, Text};
use alloc::boxed::Box;
use skein_lib::Decimal;

pub const INTRO: &[u8] = b"A settings file is a `file`, below; each section is one shape of JSON object, and each bullet one of its members, with its type, its bound and what it does. Read a value with `settings show`, given its kind and number, edit it, and set it whole. A bound here is what ctl sends; the deployment may still refuse a value, naming its fault: `shape`, `count`, `reference`, `rules`, `budget`, `grace` or `owner`. Spend is in the deployment's units, those of its models' prices. Hex is bytes in lower-case hexadecimal: `666f726765` is `forge`.";

#[must_use]
pub fn page(limits: &Limits) -> Box<[u8]> {
    page_of(&FORMS, &FIELDS, limits)
}

#[must_use]
pub fn page_of(forms: &[Form], fields: &[Field], limits: &Limits) -> Box<[u8]> {
    let mut measure = Text::measure();
    write(forms, fields, limits, &mut measure);
    let mut out = Text::writing_exact(measure);
    write(forms, fields, limits, &mut out);
    let bytes = out.finish();
    for byte in &bytes {
        assert!(*byte == b'\n' || (b' '..=b'~').contains(byte), "page is printable ASCII and line ends");
    }
    bytes
}

fn write(forms: &[Form], fields: &[Field], limits: &Limits, out: &mut Text) {
    out.put(b"# Settings fields\n\nKind: reference\n\nEvery member of a settings value: its type, its bound and what it does to work, as a settings file sets it and `settings show` prints it.\n");
    out.put(b"\n");
    out.put(INTRO);
    out.put(b"\n");
    for form in forms {
        out.put(b"\n## ");
        out.put(form.name);
        out.put(b"\n\n");
        if !form.line.is_empty() {
            out.put(form.line);
            out.put(b"\n\n");
        }
        match form.choice {
            Choice::Every => {}
            Choice::One { words } => {
                for word in words {
                    out.put(b"- ");
                    quoted(word.word, out);
                    out.put(b":");
                    line(word.line, out);
                    out.put(b"\n");
                }
            }
        }
        for field in fields {
            if field.shape != form.shape {
                continue;
            }
            out.put(b"- `");
            out.put(field.name);
            out.put(b"`: ");
            if let Some(bound) = field.list {
                out.put(b"list of at most ");
                number(most(bound, limits), out);
                out.put(b", each ");
            }
            value(&field.value, forms, limits, out);
            if field.null {
                out.put(b", or null");
            }
            match field.presence {
                Presence::Required => out.put(b", required"),
                Presence::Optional(default) => {
                    out.put(b", optional, default ");
                    out.put(match default {
                        Default::Null => b"null",
                        Default::EmptyList => b"[]",
                    });
                }
            }
            out.put(b".");
            line(field.line, out);
            if let Value::Words(words) = field.value {
                for word in words {
                    if !word.line.is_empty() {
                        out.put(b" ");
                        quoted(word.word, out);
                        out.put(b": ");
                        out.put(word.line);
                    }
                }
            }
            out.put(b"\n");
        }
    }
}

fn line(bytes: &[u8], out: &mut Text) {
    if !bytes.is_empty() {
        out.put(b" ");
        out.put(bytes);
    }
}
fn quoted(word: &[u8], out: &mut Text) {
    out.put(b"`\"");
    out.put(word);
    out.put(b"\"`");
}
fn words(values: &[Word], out: &mut Text) {
    for (index, word) in values.iter().enumerate() {
        if index > 0 {
            if index.checked_add(1).expect("word count fits") == values.len() {
                out.put(b" or ");
            } else {
                out.put(b", ");
            }
        }
        quoted(word.word, out);
    }
}
fn value(value: &Value, forms: &[Form], limits: &Limits, out: &mut Text) {
    match value {
        Value::Integer(width) => out.put(match width {
            Width::U8 => b"integer, 0 to 255",
            Width::U16 => b"integer, 0 to 65,535",
            Width::U32 => b"integer, 0 to 4,294,967,295",
            Width::U64 => b"integer",
        }),
        Value::Nanoseconds => out.put(b"nanoseconds"),
        Value::Time => out.put(b"nanoseconds since 1970 UTC"),
        Value::Boolean => out.put(b"true or false"),
        Value::Text(bound) | Value::Hex(bound) => {
            out.put(if let Value::Text(_) = value { b"text" } else { b"hex" });
            out.put(b", at most ");
            number(most(*bound, limits), out);
            out.put(b" bytes");
        }
        Value::Words(values) => words(values, out),
        Value::Shape(shape) => {
            for form in forms {
                if form.shape == *shape {
                    out.put(b"`");
                    out.put(form.name);
                    out.put(b"`");
                    return;
                }
            }
            unreachable!("nested shape has a form")
        }
        Value::Setting => out.put(b"the setting's kind's value: `charter`, `template`, `policy` or `rules`"),
    }
}
fn number(value: u64, out: &mut Text) {
    let decimal = Decimal::of(value);
    let digits = decimal.as_bytes();
    for (index, digit) in digits.iter().enumerate() {
        if index > 0 && digits.len().checked_sub(index).expect("digit offset") % 3 == 0 {
            out.put(b",");
        }
        out.put(&[*digit]);
    }
}
