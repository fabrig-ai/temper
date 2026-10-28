use super::{
    Bound, Choice, Default, FIELDS, FORMS, Field, Form, Limit, Presence, Shape, Value, Width, Word, form, members,
    resolve,
};
use crate::Limits;
use alloc::boxed::Box;
use skein_json::writer::{Encoder, Limits as JsonLimits};
use skein_lib::List;

// T1 names all 41 decoder shapes, including the alternative-only objects.
const SHAPES: [Shape; 41] = [
    Shape::File,
    Shape::Item,
    Shape::Setting,
    Shape::Charter,
    Shape::Model,
    Shape::Template,
    Shape::Text,
    Shape::Executor,
    Shape::Contract,
    Shape::Approval,
    Shape::Part,
    Shape::Key,
    Shape::Report,
    Shape::Verdict,
    Shape::Choice,
    Shape::ChoiceField,
    Shape::Change,
    Shape::Budget,
    Shape::AsAllowed,
    Shape::Deadline,
    Shape::Flags,
    Shape::Llm,
    Shape::ModelEffort,
    Shape::Policy,
    Shape::Role,
    Shape::TemplateOverride,
    Shape::Permission,
    Shape::Rules,
    Shape::Implication,
    Shape::Authority,
    Shape::Models,
    Shape::Grant,
    Shape::Pattern,
    Shape::Last,
    Shape::Delegation,
    Shape::Delegate,
    Shape::Resource,
    Shape::Requirement,
    Shape::Judge,
    Shape::Guard,
    Shape::Observed,
];
fn kinds() -> &'static [Word] {
    for field in members(Shape::Setting) {
        if field.name == b"kind" {
            let Value::Words(words) = field.value else {
                unreachable!("kind words");
            };
            return words;
        }
    }
    unreachable!("setting has kind")
}

fn index(shape: Shape) -> usize {
    for (index, candidate) in SHAPES.iter().enumerate() {
        if *candidate == shape {
            return index;
        }
    }
    unreachable!("all shapes named")
}
fn reaches(value: &Value, seen: &[bool; SHAPES.len()]) -> bool {
    match value {
        Value::Shape(shape) => seen[index(*shape)],
        Value::Setting => {
            for kind in kinds() {
                if seen[index(super::shape_of_kind(kind.word).expect("kind"))] {
                    return true;
                }
            }
            false
        }
        Value::Integer(_)
        | Value::Nanoseconds
        | Value::Time
        | Value::Boolean
        | Value::Text(_)
        | Value::Hex(_)
        | Value::Words(_) => false,
    }
}
fn routes(target: Shape) -> [bool; SHAPES.len()] {
    let mut seen = [false; SHAPES.len()];
    seen[index(target)] = true;
    for _ in &SHAPES {
        let mut changed = false;
        for field in &FIELDS {
            if !seen[index(field.shape)] && reaches(&field.value, &seen) {
                seen[index(field.shape)] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    seen
}

#[test]
fn every_shape_has_one_form_and_is_reached_from_the_file() {
    let mut counts = [0_u32; SHAPES.len()];
    let mut next = 0;
    for form in &FORMS {
        counts[index(form.shape)] += 1;
        let fields = members(form.shape);
        for (at, field) in fields.iter().enumerate() {
            assert!(same(field, FIELDS.get(next).expect("field in table")), "FIELDS follows FORMS");
            next += 1;
            for earlier in &fields[..at] {
                assert_ne!(earlier.name, field.name, "unique member");
            }
        }
        if let Choice::One { words } = form.choice {
            assert!(words.len() + fields.len() >= 2, "a choice has alternatives");
        }
        assert!(routes(form.shape)[index(Shape::File)], "{:?} is reachable", form.shape);
    }
    assert_eq!(counts, [1; SHAPES.len()], "exactly one form per shape");
    assert_eq!(next, FIELDS.len(), "every field grouped under its form");
}

#[derive(Clone, Copy, Debug)]
enum Edit {
    Base,
    Alternative(Shape, usize),
    Word(&'static Field, usize),
    Null(&'static Field),
    Omit(&'static Field),
}
fn target(edit: Edit) -> Shape {
    match edit {
        Edit::Base => Shape::File,
        Edit::Alternative(shape, _) => shape,
        Edit::Word(field, _) | Edit::Null(field) | Edit::Omit(field) => field.shape,
    }
}
fn same(a: &Field, b: &Field) -> bool {
    core::ptr::eq(a, b)
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Shape(Shape),
    Field(&'static Field),
    Value(&'static Value),
    EndObject,
    EndArray,
}

fn alternative(shape: Shape, edit: Edit, route: &[bool; SHAPES.len()]) -> usize {
    if let Edit::Alternative(chosen, index) = edit
        && chosen == shape
    {
        return index;
    }
    for (index, field) in members(shape).iter().enumerate() {
        if reaches(&field.value, route) {
            return index;
        }
    }
    0
}
fn omitted(field: &Field, edit: Edit, changed: &mut bool) -> bool {
    if let Edit::Omit(chosen) = edit
        && same(field, chosen)
        && !*changed
    {
        *changed = true;
        return true;
    }
    false
}
fn emit(kind: &[u8], edit: Edit, route: &[bool; SHAPES.len()], changed: &mut bool, out: &mut Encoder) {
    // Each pending object contributes at most its members and two closing
    // markers. The decoder's JSON depth bound is 32.
    const STACK: usize = FIELDS.len() + 2 * 32;
    // A shape can recur through Every objects, but no path is recursive.
    // At most a full member expansion at each of the decoder's 32 levels.
    const STEPS: usize = FIELDS.len() * FIELDS.len() * 32;
    let mut stack = List::with_capacity(u32::try_from(STACK).expect("schema bound"));
    stack.push(Action::Shape(Shape::Item)).expect("stack room");
    for _ in 0..STEPS {
        let Some(action) = stack.last().copied() else {
            return;
        };
        stack.truncate(stack.len().checked_sub(1).expect("stack not empty"));
        match action {
            Action::EndObject => out.object_end(),
            Action::EndArray => out.array_end(),
            Action::Shape(shape) => {
                let fields = members(shape);
                let selected = match form(shape).choice {
                    Choice::Every => None,
                    Choice::One { words } => {
                        let at = alternative(shape, edit, route);
                        if shape == target(edit) {
                            *changed = true;
                        }
                        if at >= fields.len()
                            && let Some(word) = words.get(at.saturating_sub(fields.len()))
                        {
                            out.string(word.word);
                            continue;
                        }
                        Some(at)
                    }
                };
                out.object_start();
                stack.push(Action::EndObject).expect("schema stack bound");
                for (at, field) in fields.iter().enumerate().rev() {
                    if let Some(selected) = selected
                        && at != selected
                    {
                        continue;
                    }
                    if shape == Shape::Item {
                        let edit_only = match edit {
                            Edit::Word(chosen, _) => chosen.shape == Shape::Item && chosen.name == b"edit",
                            Edit::Base | Edit::Alternative(..) | Edit::Null(_) | Edit::Omit(_) => false,
                        };
                        if field.name == if edit_only { b"set".as_slice() } else { b"edit".as_slice() } {
                            continue;
                        }
                    }
                    if shape == Shape::Setting && field.name == b"number" && kind == b"rules" {
                        continue;
                    }
                    stack.push(Action::Field(field)).expect("schema stack bound");
                }
            }
            Action::Field(field) => {
                if omitted(field, edit, changed) {
                    continue;
                }
                out.key(field.name);
                if field.shape == Shape::Setting && field.name == b"kind" {
                    out.string(kind);
                    continue;
                }
                match edit {
                    Edit::Null(chosen) if same(field, chosen) => {
                        *changed = true;
                        out.null();
                        continue;
                    }
                    Edit::Word(chosen, at) if same(field, chosen) => {
                        let Value::Words(words) = field.value else {
                            unreachable!("word field");
                        };
                        *changed = true;
                        out.string(words[at].word);
                        continue;
                    }
                    Edit::Base | Edit::Alternative(..) | Edit::Null(_) | Edit::Word(..) | Edit::Omit(_) => {}
                }
                if field.list.is_some() {
                    out.array_start();
                    stack.push(Action::EndArray).expect("schema stack bound");
                }
                stack.push(Action::Value(&field.value)).expect("schema stack bound");
            }
            Action::Value(value) => match value {
                Value::Integer(_) | Value::Nanoseconds | Value::Time => out.unsigned(1),
                Value::Boolean => out.boolean(false),
                Value::Text(_) => out.string(b"x"),
                Value::Hex(_) => out.string(b"00"),
                Value::Words(words) => out.string(words[0].word),
                Value::Shape(shape) => stack.push(Action::Shape(*shape)).expect("schema stack bound"),
                Value::Setting => {
                    stack.push(Action::Shape(super::shape_of_kind(kind).expect("kind"))).expect("schema stack bound");
                }
            },
        }
    }
    unreachable!("schema expansion finishes within its bound")
}
fn file(edit: Edit, out: &mut Encoder) -> bool {
    let mut changed = false;
    out.object_start();
    let items = &members(Shape::File)[0];
    if !omitted(items, edit, &mut changed) {
        out.key(items.name);
        out.array_start();
        let route = routes(target(edit));
        for kind in kinds() {
            emit(kind.word, edit, &route, &mut changed, out);
        }
        out.array_end();
    }
    out.object_end();
    changed
}
fn generated(edit: Edit) -> (Box<[u8]>, bool) {
    let limits = JsonLimits { depth: 32, length: u32::try_from(Limits::standard().ask_bytes).expect("file bound") };
    let mut measure = Encoder::measure(&limits);
    file(edit, &mut measure);
    let mut out = Encoder::write(measure.measured().expect("generated file fits"), &limits);
    let changed = file(edit, &mut out);
    (out.finish(), changed)
}
fn decodes(edit: Edit) {
    let (bytes, changed) = generated(edit);
    if !matches_base(edit) {
        assert!(changed, "generated file reaches {edit:?}");
    }
    let result = crate::file::decode(&bytes, &Limits::standard());
    assert!(result.is_ok(), "{edit:?}: {result:?}");
}
fn matches_base(edit: Edit) -> bool {
    match edit {
        Edit::Base => true,
        Edit::Alternative(..) | Edit::Word(..) | Edit::Null(_) | Edit::Omit(_) => false,
    }
}

#[test]
fn every_generated_file_decodes() {
    decodes(Edit::Base);
    for form in &FORMS {
        if let Choice::One { words } = form.choice {
            for at in 0..members(form.shape).len() + words.len() {
                decodes(Edit::Alternative(form.shape, at));
            }
        }
    }
    for field in &FIELDS {
        if field.null {
            decodes(Edit::Null(field));
        }
        if let Value::Words(words) = field.value {
            // The four kinds are already generated together; each set's shape
            // must agree with its selector. Edits select the edit-only form.
            if field.shape != Shape::Setting {
                for at in 0..words.len() {
                    decodes(Edit::Word(field, at));
                }
            }
        }
    }
}

#[test]
fn absent_members_take_their_default_or_are_refused_by_path() {
    for field in &FIELDS {
        if field.shape == Shape::Item {
            continue;
        }
        if let Choice::One { .. } = form(field.shape).choice {
            continue;
        }
        let (bytes, changed) = generated(Edit::Omit(field));
        assert!(changed, "generated file reaches {field:?}");
        if let Presence::Optional(_) = field.presence {
            crate::file::decode(&bytes, &Limits::standard()).expect("optional member takes its default");
            continue;
        }
        let why = crate::file::decode(&bytes, &Limits::standard()).expect_err("required member absent");
        let shape = form(field.shape).name;
        let mut path = skein_lib::Writer::new(shape.len() + 1 + field.name.len());
        path.put(shape).expect("measured path");
        path.put(b".").expect("measured path");
        path.put(field.name).expect("measured path");
        let path = path.finish();
        let mut named = false;
        for word in why.windows(path.len()) {
            if word == path.as_ref() {
                named = true;
                break;
            }
        }
        assert!(named, "{field:?}: {why:?}");
    }
}

#[test]
fn a_path_resolves_through_its_shapes() {
    for (path, shape, name) in [
        (b"charter.model.name".as_slice(), Shape::Model, b"name".as_slice()),
        (b"model.name", Shape::Model, b"name"),
        (b"policy.roles.authority.spend", Shape::Authority, b"spend"),
        (b"policy.roles[].authority.spend", Shape::Authority, b"spend"),
    ] {
        let field = resolve(path).expect("path resolves");
        assert_eq!((field.shape, field.name), (shape, name));
    }
    for path in [
        b"charter.nothing".as_slice(),
        b"nothing.name",
        b"charter.number.more",
        b"charter.model[].name",
        b"model",
        b"model.name.",
    ] {
        assert!(resolve(path).is_none(), "invalid path {path:?}");
    }
}

const PAGE_FORMS: [Form; 2] = [
    Form { shape: Shape::File, name: b"file", choice: Choice::Every, line: b"LINE" },
    Form {
        shape: Shape::Model,
        name: b"model",
        choice: Choice::One { words: &[Word { word: b"chosen", line: b"LINE" }] },
        line: b"",
    },
];
const PAGE_FIELDS: [Field; 14] = [
    Field {
        shape: Shape::File,
        name: b"number",
        value: Value::Integer(Width::U8),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"connector",
        value: Value::Integer(Width::U16),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"turns",
        value: Value::Integer(Width::U32),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"spend",
        value: Value::Integer(Width::U64),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"LINE",
    },
    Field {
        shape: Shape::File,
        name: b"waiting",
        value: Value::Nanoseconds,
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"deadline",
        value: Value::Time,
        list: None,
        null: true,
        presence: Presence::Optional(Default::Null),
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"cut",
        value: Value::Boolean,
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"instructions",
        value: Value::Text(Bound::Limit(Limit::DocumentTextBytes)),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"state",
        value: Value::Hex(Bound::Most(1_000_000)),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"kind",
        value: Value::Words(&[
            Word { word: b"a", line: b"LINE" },
            Word { word: b"b", line: b"" },
            Word { word: b"c", line: b"LINE" },
        ]),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"LINE",
    },
    Field {
        shape: Shape::File,
        name: b"edit",
        value: Value::Words(&[Word { word: b"removed", line: b"" }, Word { word: b"reset", line: b"" }]),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"hosts",
        value: Value::Words(&[Word { word: b"workers", line: b"" }]),
        list: None,
        null: false,
        presence: Presence::Required,
        line: b"",
    },
    Field {
        shape: Shape::File,
        name: b"alternatives",
        value: Value::Shape(Shape::Model),
        list: Some(Bound::Most(32)),
        null: false,
        presence: Presence::Optional(Default::EmptyList),
        line: b"",
    },
    Field {
        shape: Shape::Model,
        name: b"set",
        value: Value::Setting,
        list: Some(Bound::Limit(Limit::CharterAlternatives)),
        null: true,
        presence: Presence::Required,
        line: b"",
    },
];

#[test]
fn the_page_writes_each_type_as_the_table_says() {
    let page = super::render::page_of(&PAGE_FORMS, &PAGE_FIELDS, &Limits::standard());
    let expected = br#"# Settings fields

Kind: reference

Every member of a settings value: its type, its bound and what it does to work, as a settings file sets it and `settings show` prints it.

A settings file is a `file`, below; each section is one shape of JSON object, and each bullet one of its members, with its type, its bound and what it does. Read a value with `settings show`, given its kind and number, edit it, and set it whole. A bound here is what ctl sends; the deployment may still refuse a value, naming its fault: `shape`, `count`, `reference`, `rules`, `budget`, `grace` or `owner`. Spend is in the deployment's units, those of its models' prices. Hex is bytes in lower-case hexadecimal: `666f726765` is `forge`.

## file

LINE

- `number`: integer, 0 to 255, required.
- `connector`: integer, 0 to 65,535, required.
- `turns`: integer, 0 to 4,294,967,295, required.
- `spend`: integer, required. LINE
- `waiting`: nanoseconds, required.
- `deadline`: nanoseconds since 1970 UTC, or null, optional, default null.
- `cut`: true or false, required.
- `instructions`: text, at most 32,768 bytes, required.
- `state`: hex, at most 1,000,000 bytes, required.
- `kind`: `"a"`, `"b"` or `"c"`, required. LINE `"a"`: LINE `"c"`: LINE
- `edit`: `"removed"` or `"reset"`, required.
- `hosts`: `"workers"`, required.
- `alternatives`: list of at most 32, each `model`, optional, default [].

## model

- `"chosen"`: LINE
- `set`: list of at most 16, each the setting's kind's value: `charter`, `template`, `policy` or `rules`, or null, required.
"#;
    assert_eq!(page.as_ref(), expected);
}

// T2, section 3: row descriptions are at most 320 bytes; INTRO, 1,024.
const LINE_BYTES: usize = 320;
const INTRO_BYTES: usize = 1024;

#[derive(Debug)]
struct Line {
    bytes: &'static [u8],
    required: bool,
    most: usize,
}

fn lines() -> List<Line> {
    let mut count = FORMS.len().checked_add(FIELDS.len()).expect("table count").checked_add(1).expect("INTRO");
    for form in &FORMS {
        if let Choice::One { words } = form.choice {
            count = count.checked_add(words.len()).expect("choice words");
        }
    }
    for field in &FIELDS {
        if let Value::Words(words) = field.value {
            count = count.checked_add(words.len()).expect("member words");
        }
    }
    let mut lines = List::with_capacity(u32::try_from(count).expect("line count fits"));
    lines.push(Line { bytes: super::render::INTRO, required: true, most: INTRO_BYTES }).expect("counted lines");
    for form in &FORMS {
        lines.push(Line { bytes: form.line, required: true, most: LINE_BYTES }).expect("counted lines");
        if let Choice::One { words } = form.choice {
            for word in words {
                lines.push(Line { bytes: word.line, required: true, most: LINE_BYTES }).expect("counted lines");
            }
        }
    }
    for field in &FIELDS {
        lines.push(Line { bytes: field.line, required: true, most: LINE_BYTES }).expect("counted lines");
        if let Value::Words(words) = field.value {
            for word in words {
                lines.push(Line { bytes: word.line, required: false, most: LINE_BYTES }).expect("counted lines");
            }
        }
    }
    lines
}

#[test]
fn every_line_is_written_and_within_its_bounds() {
    for line in &lines() {
        if line.required {
            assert!(!line.bytes.is_empty(), "required line {line:?}");
        }
        assert!(line.bytes.len() <= line.most, "line within its bound: {line:?}");
        for byte in line.bytes {
            assert!((b' '..=b'~').contains(byte), "printable ASCII in one paragraph: {line:?}");
        }
        for forbidden in [b"jig".as_slice(), b"smith", b"skein", b".md", b"temper-"] {
            for word in line.bytes.windows(forbidden.len()) {
                assert!(!word.eq_ignore_ascii_case(forbidden), "forbidden word {forbidden:?}: {line:?}");
            }
        }
    }
}

fn line_paths_resolve(line: &[u8]) {
    let mut start = None;
    for (at, byte) in line.iter().enumerate() {
        if *byte != b'`' {
            continue;
        }
        match start.take() {
            None => start = Some(at.checked_add(1).expect("span start")),
            Some(start) => {
                let path = line.get(start..at).expect("code span");
                if !path.contains(&b'.') || path.contains(&b' ') {
                    continue;
                }
                for form in &FORMS {
                    if path.starts_with(form.name) && path.get(form.name.len()) == Some(&b'.') {
                        assert!(resolve(path).is_some(), "line names an unknown field {path:?}: {line:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn every_path_a_line_names_resolves() {
    for line in &lines() {
        line_paths_resolve(line.bytes);
    }
}

#[test]
fn bound_and_null_errors_name_the_file_members() {
    let (bytes, _) = generated(Edit::Base);
    let mut limits = Limits::standard();
    limits.settings.charter_alternatives = 0;
    assert_eq!(
        crate::file::decode(&bytes, &limits).expect_err("alternatives bound").as_ref(),
        b"settings file item 1: charter.alternatives: at most 0"
    );
    for (path, expected) in [
        (b"charter.model".as_slice(), b"settings file item 1: charter.model: value required".as_slice()),
        (b"authority.models", b"settings file item 3: authority.models: value required"),
    ] {
        let field = resolve(path).expect("model member");
        let (bytes, reached) = generated(Edit::Null(field));
        assert!(reached);
        assert_eq!(
            crate::file::decode(&bytes, &Limits::standard()).expect_err("models take a value").as_ref(),
            expected
        );
    }
}
