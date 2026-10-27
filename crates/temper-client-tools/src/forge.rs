//! Checked adoption calls and the forge's shared resource vocabulary.
use crate::{
    jig::{
        self,
        call::{Call, Value, Values, number, value},
        render::{self, Text},
        runner::{Called, Out},
    },
    root::Limits,
};
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{Client, Key, Latest, Outcome, State, Told, contract as c};
use skein_json::writer::Encoder;
use skein_lib::{List, Queue};
use skein_ui::phrases::Hole;
use temper_client_domain::{
    forge::{
        self,
        words::{self, Word},
    },
    owner::Owner,
};
#[derive(Debug)]
pub(crate) struct Adopter {
    waiting: BTreeMap<u32, Called>,
    pending: BTreeMap<Key, Pending>,
    json: bool,
}
#[derive(Debug)]
struct Pending {
    adoption: forge::Adoption,
    called: Option<Called>,
}
impl Adopter {
    pub(crate) fn new() -> Self {
        Self { waiting: BTreeMap::new(), pending: BTreeMap::new(), json: false }
    }
    pub(crate) fn calls(&self) -> usize {
        self.waiting.len().checked_add(self.pending.len()).expect("bounded calls")
    }
    pub(crate) fn kept_json(&mut self, json: bool) {
        self.json = json;
    }
}
pub(crate) fn me(client: &Client) -> Option<&c::Me> {
    let doc = client.document(jig_client::Handle { asker: crate::root::RUNNER.asker(), number: 0 })?;
    if doc.state != State::Live {
        return None;
    }
    match doc.latest? {
        Latest::Jig(c::Delivery::Me(me)) => Some(me),
        Latest::Jig(_) | Latest::Family(_) => None,
    }
}
pub(crate) fn call(adopter: &mut Adopter, client: &Client, limits: &Limits, called: Called, out: &mut Queue<Out>) {
    if adopter.waiting.len().checked_add(adopter.pending.len()).expect("bounded calls")
        >= usize::try_from(limits.tools.rows).expect("rows fit")
    {
        fail(called, b"call limit", out);
    } else if let Some(me) = me(client) {
        send(adopter, client, me, limits, called, out);
    } else {
        adopter.waiting.insert(called.call_number, called);
    }
}
pub(crate) fn ready(adopter: &mut Adopter, client: &Client, limits: &Limits, out: &mut Queue<Out>) {
    let Some(me) = me(client) else { return };
    for _ in 0..adopter.waiting.len() {
        let Some((_, called)) = adopter.waiting.pop_first() else { break };
        send(adopter, client, me, limits, called, out);
    }
}
fn send(adopter: &mut Adopter, client: &Client, me: &c::Me, limits: &Limits, called: Called, out: &mut Queue<Out>) {
    let Some(project) = jig::runner::reads::project(&called.call, me) else {
        fail(called, b"unknown project", out);
        return;
    };
    let form = form(&called.call, project, limits.domain.forge);
    let Some(adoption) = forge::adoption(&form, limits.domain.forge.checks) else {
        let marks = forge::broken(&form, limits.domain.forge.checks);
        let mut measure = Text::measure();
        marks_text(marks, &mut measure);
        let mut text = Text::writing_exact(measure);
        marks_text(marks, &mut text);
        let printed = render::failure(&text.finish(), called.json, 1);
        out.push(Out::Printed { number: called.number, printed });
        return;
    };
    let Some(request) = forge::ask(&adoption, &limits.domain.forge) else {
        fail(called, b"adoption encoding failed", out);
        return;
    };
    let key = Key { asker: Owner::Forge.asker(), number: (u64::from(called.call_number) << 32u32) | 1 };
    if client.pending(key).is_some() {
        fail(called, b"tools' bug; client key collision; report it as a finding", out);
        return;
    }
    let name = Some(crate::key::engine(called.call.key.as_ref().expect("adopt requires a key")));
    adopter.pending.insert(key, Pending { adoption, called: Some(called) });
    out.push(Out::Client { event: jig_client::Event::Send { key, name, request } });
}
fn form(call: &Call, project: u32, limits: forge::Limits) -> forge::Form {
    let mut form = forge::form(project);
    for (at, field) in
        [(2, forge::Text::Owner), (3, forge::Text::Name), (4, forge::Text::Landing), (6, forge::Text::Prefix)]
    {
        if let Some(Value::Text(bytes)) = value(call, at) {
            forge::write(&mut form, field, bytes.clone(), forge::Typed::Bytes, limits.field_bytes, limits.checks);
        }
    }
    if let Some(forge) = number(call, 1) {
        forge::write(
            &mut form,
            forge::Text::Forge,
            decimal(forge),
            forge::Typed::Forge(u16::try_from(forge).ok()),
            limits.field_bytes,
            limits.checks,
        );
    }
    form.role = match value(call, 5) {
        Some(Value::Word(1)) => forge::Role::Fork,
        Some(Value::Word(2)) => forge::Role::Context,
        _ => forge::Role::Owned,
    };
    if form.role == forge::Role::Context || value(call, 7).is_some() {
        form.home.on = false;
    }
    if value(call, 8).is_some() {
        form.ci.on = false;
    }
    if let Some(Some(Values::Many(values))) = call.values.get(9) {
        let mut checks = List::with_capacity(limits.checks);
        let mut valid = true;
        for value in values {
            match value {
                Value::Number(n) => match u32::try_from(*n) {
                    Ok(n) => {
                        if checks.push(n).is_err() {
                            valid = false;
                        }
                    }
                    Err(_) => valid = false,
                },
                Value::Word(_)
                | Value::Text(_)
                | Value::Stdin
                | Value::File(_)
                | Value::Duration(_)
                | Value::Proposal(_)
                | Value::Digest(_)
                | Value::Condition(_)
                | Value::Switch
                | Value::Project(_)
                | Value::Turn(_)
                | Value::Cursor(_)
                | Value::Group(_)
                | Value::Order(_)
                | Value::Tool(_) => valid = false,
            }
        }
        let typed = if valid { Some(checks.into_boxed()) } else { None };
        forge::write(
            &mut form,
            forge::Text::Checks,
            Box::default(),
            forge::Typed::Checks(typed),
            limits.field_bytes,
            limits.checks,
        );
    }
    form
}
fn fail(called: Called, why: &[u8], out: &mut Queue<Out>) {
    out.push(Out::Printed { number: called.number, printed: render::failure(why, called.json, 1) });
}
fn marks_text(marks: forge::Marks, out: &mut Text) {
    let mut first = true;
    for broken in marks.texts.into_iter().chain([marks.home]) {
        if let Some((word, holes)) = words::broken(broken) {
            if !first {
                out.put(b"\n");
            }
            phrase(word, &holes, out);
            first = false;
        }
    }
}
pub(crate) fn told(adopter: &mut Adopter, limits: &Limits, told: Told, out: &mut Queue<Out>) {
    match told {
        Told::Kept { key, request } => {
            if let Some(adoption) = restored(&request, limits.domain.forge) {
                adopter.pending.insert(key, Pending { adoption, called: None });
            }
        }
        Told::Answered { key, outcome } => {
            let Some(pending) = adopter.pending.remove(&key) else { return };
            let json = match &pending.called {
                Some(called) => called.json,
                None => adopter.json,
            };
            let printed = print(&pending.adoption, &outcome, json, pending.called.is_none(), limits);
            match pending.called {
                Some(called) => out.push(Out::Printed { number: called.number, printed }),
                None => out.push(Out::Kept { line: if printed.status == 0 { printed.stdout } else { printed.stderr } }),
            }
        }
        Told::Pending { .. }
        | Told::Changed { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. }
        | Told::Link { .. }
        | Told::Dropped { .. } => {}
    }
}
fn restored(ask: &jig_client::Ask, limits: forge::Limits) -> Option<forge::Adoption> {
    use temper_client_protocol_wire::{self as wire, v1 as w};
    let jig_client::Ask::Family { record, .. } = ask else { return None };
    if record.family != wire::FORGE || record.version != wire::CLIENT_VERSION {
        return None;
    }
    let requesting = wire::decode_requesting(&record.bytes, &forge::wire_limits(&limits)?)?;
    let w::Request::Adopt(adopt) = requesting.into_parts().request else { return None };
    let a = adopt.into_parts();
    Some(forge::Adoption {
        project: a.project,
        home: a.home,
        forge: a.forge,
        owner: a.owner,
        name: a.name,
        prefix: a.prefix,
        role: match a.role {
            w::Role::Owned => forge::Role::Owned,
            w::Role::Adopted => forge::Role::Adopted,
            w::Role::Fork => forge::Role::Fork,
            w::Role::Context => forge::Role::Context,
        },
        landing: a.landing,
        ci: a.ci,
        checks: a.checks.into_boxed(),
    })
}
pub(crate) fn print(
    adoption: &forge::Adoption,
    outcome: &Outcome,
    json: bool,
    kept: bool,
    limits: &Limits,
) -> jig::Printed {
    let answer = match outcome {
        Outcome::Unknown => Some(forge::Answer::Unknown),
        Outcome::Answered(answer) => forge::answer(&limits.domain, answer),
        Outcome::Unsent | Outcome::Lost => unreachable!("temper's requests carry no value"),
        Outcome::Held | Outcome::Limit | Outcome::Unconfirmed | Outcome::NotKept => {
            let operation =
                render::outcome::Operation::of(&forge::ask(adoption, &limits.domain.forge).expect("validated ask"));
            if kept {
                return jig::Printed {
                    file: jig::FileEnd::Keep,
                    status: 1,
                    stdout: Box::default(),
                    stderr: render::outcome::kept(&operation, outcome, json, None, &limits.tools),
                };
            }
            return render::outcome::print(&operation, outcome, json, None, &limits.tools);
        }
    };
    let Some(answer) = answer else { return decode_failure(json, kept) };
    let status = match answer {
        forge::Answer::Adopted(_) => 0,
        forge::Answer::Unknown => 1,
        forge::Answer::CoreRefused(_) | forge::Answer::ForgeRefused(_) | forge::Answer::EntranceRefused(_) => 2,
    };
    let bytes = if json {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        encode(&answer, kept, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut write = Encoder::write(size, &bounds);
        encode(&answer, kept, &mut write);
        let bytes = write.finish();
        if kept { render::outcome::newline(&bytes) } else { bytes }
    } else {
        let mut measure = Text::measure();
        answer_text(adoption, &answer, kept, limits.tools.text, &mut measure);
        let mut write = Text::writing_exact(measure);
        answer_text(adoption, &answer, kept, limits.tools.text, &mut write);
        write.finish()
    };
    if status == 0 {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: bytes, stderr: Box::default() }
    } else {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: Box::default(), stderr: bytes }
    }
}
fn phrase(word: Word, holes: &[Hole], out: &mut Text) {
    for (candidate, literals, _) in words::ROWS {
        if candidate == word {
            literals_text(literals, holes, out);
            return;
        }
    }
    unreachable!("closed words")
}
pub(crate) fn literals_text(literals: &[&[u8]], holes: &[Hole], out: &mut Text) {
    for (index, literal) in literals.iter().enumerate() {
        out.put(literal);
        match holes.get(index) {
            Some(Hole::Number(n)) => out.number(*n),
            Some(Hole::Name(bytes)) => out.put(bytes),
            None => {}
            Some(Hole::Amount(_) | Hole::Span(_) | Hole::Wall(_) | Hole::Reference { .. }) => {
                unreachable!("forge holes are names and numbers")
            }
        }
    }
}
fn decimal(n: u64) -> Box<[u8]> {
    let mut measure = Text::measure();
    measure.number(n);
    let mut out = Text::writing_exact(measure);
    out.number(n);
    out.finish()
}
fn word_bytes(word: Word) -> Box<[u8]> {
    let mut measure = Text::measure();
    phrase(word, &[], &mut measure);
    let mut out = Text::writing_exact(measure);
    phrase(word, &[], &mut out);
    out.finish()
}
fn answer_text(a: &forge::Adoption, answer: &forge::Answer, kept: bool, text: u32, out: &mut Text) {
    if kept {
        out.put(b"kept: adopt ");
    }
    let name = Hole::Name(words::full_name(&a.owner, &a.name));
    let number = Hole::Name(decimal(u64::from(a.forge)));
    match answer {
        forge::Answer::Adopted(a) => {
            phrase(
                Word::Adopted,
                &[
                    Hole::Name(words::full_name(&a.owner, &a.name)),
                    Hole::Number(u64::from(a.forge)),
                    Hole::Name(word_bytes(words::role(a.role))),
                ],
                out,
            );
            out.put(b"\n");
            if a.home {
                phrase(Word::IsHome, &[], out);
                out.put(b"\n");
            }
            let mut measure = Text::measure();
            kinds(a.kinds, &mut measure);
            let mut list = Text::writing_exact(measure);
            kinds(a.kinds, &mut list);
            phrase(Word::May, &[Hole::Name(list.finish())], out);
            if a.protection == forge::Protection::Unknown {
                out.put(b"\n");
                phrase(Word::Protection, &[], out);
            }
        }
        forge::Answer::Unknown => phrase(Word::Unknown, &[name], out),
        forge::Answer::CoreRefused(why) => render::acts::refusal(why, &render::document::phrasebook(text), out),
        forge::Answer::EntranceRefused(why) => {
            render::acts::refusal(why.refusal(), &render::document::phrasebook(text), out);
        }
        forge::Answer::ForgeRefused(why) => match why {
            forge::Reason::NoSuchForge => phrase(Word::NoSuchForge, &[number], out),
            forge::Reason::NotFound => phrase(Word::NotFound, &[name, number], out),
            forge::Reason::NoAccess => phrase(Word::NoAccess, &[name, number], out),
            forge::Reason::Refused => phrase(Word::Refused, &[name, Hole::Name(a.prefix.clone())], out),
            forge::Reason::AnotherProject => phrase(Word::AnotherProject, &[name], out),
            forge::Reason::HomeHeld => {
                phrase(Word::HomeHeld, &[], out);
                adoption_retry(a, true, false, out);
            }
            forge::Reason::PrefixOccupied => {
                phrase(Word::PrefixOccupied, &[Hole::Name(a.prefix.clone())], out);
                adoption_retry(a, !a.home, true, out);
            }
            forge::Reason::InFlight => phrase(Word::InFlight, &[name], out),
            forge::Reason::TooLarge => phrase(Word::TooLarge, &[], out),
            forge::Reason::Unreachable => phrase(Word::Unreachable, &[number], out),
            forge::Reason::InvalidAnswer => phrase(Word::InvalidAnswer, &[number], out),
        },
    }
    out.put(b"\n");
}
fn adoption_retry(a: &forge::Adoption, no_home: bool, new_prefix: bool, out: &mut Text) {
    out.put(b"\n  temper ctl DIR repository adopt --project ");
    out.put(skein_lib::Decimal::of(u64::from(a.project)).as_bytes());
    out.put(b" --forge ");
    out.put(skein_lib::Decimal::of(u64::from(a.forge)).as_bytes());
    for (flag, value) in [
        (b" --owner ".as_slice(), a.owner.as_ref()),
        (b" --name ", a.name.as_ref()),
        (b" --landing ", a.landing.as_ref()),
    ] {
        out.put(flag);
        render::lists::argument(value, out);
    }
    out.put(b" --role ");
    out.put(match a.role {
        forge::Role::Owned => b"owned",
        forge::Role::Adopted => b"adopted",
        forge::Role::Fork => b"fork",
        forge::Role::Context => b"context",
    });
    out.put(b" --prefix ");
    render::lists::argument(if new_prefix { b"UNUSED-PREFIX/" } else { &a.prefix }, out);
    if no_home {
        out.put(b" --no-home");
    }
    if !a.ci {
        out.put(b" --no-ci");
    }
    for check in &a.checks {
        out.put(b" --check ");
        out.put(skein_lib::Decimal::of(u64::from(*check)).as_bytes());
    }
    out.put(b" --key KEY");
}

fn kinds(kinds: forge::Kinds, out: &mut Text) {
    let mut first = true;
    for (on, word) in words::kinds(kinds) {
        if on {
            if !first {
                out.put(b"; ");
            }
            phrase(word, &[], out);
            first = false;
        }
    }
}
fn encode(answer: &forge::Answer, kept: bool, out: &mut Encoder) {
    if kept {
        out.object_start();
        out.key(b"operation");
        out.string(b"adopt");
        out.key(b"outcome");
    }
    out.object_start();
    match answer {
        forge::Answer::Adopted(a) => {
            out.key(b"adopted");
            adopted_json(a, out);
        }
        forge::Answer::Unknown => {
            out.key(b"unknown");
            out.object_start();
            out.object_end();
        }
        forge::Answer::ForgeRefused(why) => {
            out.key(b"forge_refused");
            out.object_start();
            out.key(b"reason");
            out.string(reason(*why));
            out.object_end();
        }
        forge::Answer::CoreRefused(why) => {
            out.key(b"core_refused");
            out.object_start();
            out.key(b"refusal");
            render::document::refusal_json(why, out);
            out.object_end();
        }
        forge::Answer::EntranceRefused(why) => {
            out.key(b"entrance_refused");
            out.object_start();
            out.key(b"refusal");
            render::document::refusal_json(why.refusal(), out);
            out.object_end();
        }
    }
    out.object_end();
    if kept {
        out.object_end();
    }
}
fn reason(why: forge::Reason) -> &'static [u8] {
    match why {
        forge::Reason::NoSuchForge => b"no_such_forge",
        forge::Reason::NotFound => b"not_found",
        forge::Reason::NoAccess => b"no_access",
        forge::Reason::Refused => b"refused",
        forge::Reason::AnotherProject => b"another_project",
        forge::Reason::HomeHeld => b"home_held",
        forge::Reason::PrefixOccupied => b"prefix_occupied",
        forge::Reason::InFlight => b"in_flight",
        forge::Reason::TooLarge => b"too_large",
        forge::Reason::Unreachable => b"unreachable",
        forge::Reason::InvalidAnswer => b"invalid_answer",
    }
}
fn adopted_json(a: &forge::Adopted, out: &mut Encoder) {
    out.object_start();
    for (key, n) in [(b"forge".as_slice(), u64::from(a.forge)), (b"repository", u64::from(a.repository))] {
        out.key(key);
        out.unsigned(n);
    }
    for (key, bytes) in [
        (b"owner".as_slice(), a.owner.as_ref()),
        (b"name", a.name.as_ref()),
        (b"role", word_bytes(words::role(a.role)).as_ref()),
    ] {
        out.key(key);
        out.string(bytes);
    }
    out.key(b"home");
    out.boolean(a.home);
    out.key(b"kinds");
    out.object_start();
    for (key, on) in [
        (b"read".as_slice(), a.kinds.read),
        (b"push", a.kinds.push),
        (b"open", a.kinds.open),
        (b"land", a.kinds.land),
        (b"review", a.kinds.review),
        (b"status", a.kinds.status),
        (b"comment", a.kinds.comment),
        (b"issue", a.kinds.issue),
        (b"branch", a.kinds.branch),
    ] {
        out.key(key);
        out.boolean(on);
    }
    out.object_end();
    out.key(b"protection");
    out.object_start();
    match &a.protection {
        forge::Protection::Unknown => {
            out.key(b"unknown");
            out.object_start();
            out.object_end();
        }
        forge::Protection::Absent => {
            out.key(b"absent");
            out.object_start();
            out.object_end();
        }
        forge::Protection::Rule(r) => {
            out.key(b"rule");
            out.object_start();
            out.key(b"branch");
            out.string(&r.branch);
            out.key(b"contexts");
            out.array_start();
            for context in &r.contexts {
                out.string(context);
            }
            out.array_end();
            out.key(b"approvals");
            out.unsigned(u64::from(r.approvals));
            out.key(b"dismiss_stale");
            out.boolean(r.dismiss_stale);
            out.object_end();
        }
    }
    out.object_end();
    out.object_end();
}
pub(crate) fn part(part: &c::OwnerPart, out: &mut Text) {
    let (word, holes): (_, Box<[Hole]>) = match (&part.kind, &part.key) {
        (c::OwnerPartKind::Resource, c::OwnerPartKey::Path(path)) => {
            let mut bound = path.len();
            for segment in path {
                bound = bound
                    .checked_add(u32::try_from(segment.len()).expect("bounded segment"))
                    .expect("bounded whole path");
            }
            words::path(path.as_slice(), bound)
        }
        _ => (words::ResourceWord::Shown, Box::new([])),
    };
    let (literals, _) = words::resource_english(word);
    literals_text(literals, &holes, out);
}

pub(crate) fn change_kind(connector: u16, kind: u16, out: &mut Text) {
    out.put(b"change");
    if connector == temper_client_protocol_wire::number(temper_client_protocol_wire::Owner::Forge) {
        if kind != 1 {
            out.put(b", ");
            match words::kind(kind) {
                Some(word) => literals_text(words::resource_english(word).0, &[], out),
                None => literals_text(
                    words::resource_english(words::ResourceWord::OtherKind).0,
                    &[Hole::Number(u64::from(kind))],
                    out,
                ),
            }
        }
    } else {
        out.put(b", connector ");
        out.number(u64::from(connector));
        out.put(b" kind ");
        out.number(u64::from(kind));
    }
}
pub(crate) fn change(kind: u16, resource: &c::OwnerPart, words: &c::DocumentText, out: &mut Text) {
    out.put(b"Landed");
    if let Some(commit) = temper_client_domain::pages::delegation::merge_commit(words.bytes()) {
        out.put(b" \xc2\xb7 ");
        // A core Change result names its settled pull by number. Resource paths,
        // when supplied, additionally name its repository and host (client/README.md, 4.1).
        if kind == 1
            && let c::OwnerPartKey::Number(number) = resource.key
        {
            out.put(b"pull request #");
            out.number(number);
        } else {
            part(resource, out);
        }
        out.put(b" \xc2\xb7 merge commit ");
        out.words(commit);
        if !words.cut
            && let Some(start) = skein_lib::bytes::find(words.bytes(), b"\n")
        {
            let line =
                words.bytes().get(start.checked_add(1).expect("newline separator")..).expect("newline within words");
            if let Some(separator) = skein_lib::bytes::find(line, b": ") {
                out.put(b"\npull request: ");
                out.words(
                    line.get(separator.checked_add(2).expect("address separator")..).expect("separator within line"),
                );
            }
        }
    } else {
        out.result_words(words.bytes());
    }
}
pub(crate) fn result_part(value: &c::Part, out: &mut Text) {
    match value {
        c::Part::Resource { connector, path } => part(
            &c::OwnerPart {
                connector: *connector,
                kind: c::OwnerPartKind::Resource,
                key: c::OwnerPartKey::Path(path.clone()),
            },
            out,
        ),
        c::Part::Connector { .. } => literals_text(words::resource_english(words::ResourceWord::Connector).0, &[], out),
        c::Part::Kind { kind, .. } => match words::kind(*kind) {
            Some(word) => literals_text(words::resource_english(word).0, &[], out),
            None => literals_text(
                words::resource_english(words::ResourceWord::OtherKind).0,
                &[Hole::Number(u64::from(*kind))],
                out,
            ),
        },
        c::Part::Panel { .. }
        | c::Part::Result { .. }
        | c::Part::Effect { .. }
        | c::Part::OwnerEffect { .. }
        | c::Part::Outbox { .. }
        | c::Part::Hold { .. }
        | c::Part::Project { .. }
        | c::Part::Deployment { .. } => literals_text(words::resource_english(words::ResourceWord::Shown).0, &[], out),
    }
}

fn decode_failure(json: bool, kept: bool) -> jig::Printed {
    let reason = b"adoption answer decoding failed";
    if !kept {
        return render::failure(reason, json, 1);
    }
    let stderr = if json {
        let limits = skein_json::writer::Limits { depth: 2, length: 128 };
        let mut measure = Encoder::measure(&limits);
        error_json(reason, &mut measure);
        let mut out = Encoder::write(measure.measured().expect("fixed error fits"), &limits);
        error_json(reason, &mut out);
        render::outcome::newline(&out.finish())
    } else {
        let mut measure = Text::measure();
        measure.put(b"kept: adopt ");
        measure.put(reason);
        measure.put(b"\n");
        let mut out = Text::writing_exact(measure);
        out.put(b"kept: adopt ");
        out.put(reason);
        out.put(b"\n");
        out.finish()
    };
    jig::Printed { file: jig::FileEnd::Keep, status: 1, stdout: Box::default(), stderr }
}
fn error_json(reason: &[u8], out: &mut Encoder) {
    out.object_start();
    out.key(b"operation");
    out.string(b"adopt");
    out.key(b"outcome");
    out.object_start();
    out.key(b"error");
    out.string(reason);
    out.object_end();
    out.object_end();
}
