//! Bounded whole settings input (client/tools.md, 6.4; jig's client/domain/contract.md, 3.2).
use super::{
    Limits,
    call::{Call, Value, number, value},
};
#[path = "settings_file_values.rs"]
mod values;
use alloc::boxed::Box;
use jig_client::contract as c;
use skein_json::{Document, Kind};
use skein_lib::{List, Writer};
type Fields = List<(Box<[u8]>, u32)>;

pub(super) struct Error {
    member: Box<[u8]>,
    rule: &'static [u8],
    bound: Option<u64>,
}
pub(super) fn error(member: &[u8], rule: &'static [u8]) -> Error {
    Error { member: member.into(), rule, bound: None }
}
fn bound(member: &[u8], maximum: u32) -> Error {
    Error { member: member.into(), rule: b"bound", bound: Some(u64::from(maximum)) }
}
fn message(item: Option<u32>, why: Error) -> Box<[u8]> {
    let mut measured = super::render::Text::measure();
    describe(item, &why, &mut measured);
    let mut out = super::render::Text::writing_exact(measured);
    describe(item, &why, &mut out);
    out.finish()
}
fn describe(item: Option<u32>, why: &Error, out: &mut super::render::Text) {
    out.put(b"settings file");
    if let Some(item) = item {
        out.put(b" item ");
        out.number(u64::from(item));
    }
    out.put(b": ");
    out.put(&why.member);
    out.put(b": ");
    out.put(why.rule);
    if let Some(bound) = why.bound {
        out.put(b" ");
        out.number(bound);
    }
}
pub(super) struct Decoder {
    pub doc: Document,
    pub limits: jig_client_protocol_wire::Limits,
}
impl Decoder {
    pub fn kind(&self, at: u32) -> Result<Kind, Error> {
        Ok(self.doc.token(at).ok_or(error(b"JSON", b"missing value"))?.kind)
    }
    pub fn string(&self, at: u32) -> Result<&[u8], Error> {
        let token = self.doc.token(at).ok_or(error(b"JSON", b"missing string"))?;
        if token.kind != Kind::String && token.kind != Kind::Key {
            return Err(error(b"JSON", b"expected string"));
        }
        self.doc.text(token).ok_or(error(b"JSON", b"invalid string"))
    }
    pub fn number(&self, at: u32) -> Result<u64, Error> {
        if self.kind(at)? != Kind::Number {
            return Err(error(b"JSON", b"expected unsigned integer"));
        }
        let bytes = self
            .doc
            .text(self.doc.token(at).ok_or(error(b"JSON", b"missing number"))?)
            .ok_or(error(b"JSON", b"invalid number"))?;
        let mut number = 0_u64;
        if bytes.is_empty() {
            return Err(error(b"JSON", b"expected unsigned integer"));
        }
        for byte in bytes {
            if !byte.is_ascii_digit() {
                return Err(error(b"JSON", b"expected unsigned integer"));
            }
            number = number.checked_mul(10).ok_or(error(b"JSON", b"integer bound"))?;
            number = number
                .checked_add(u64::from(byte.checked_sub(b'0').expect("ASCII digit")))
                .ok_or(error(b"JSON", b"integer bound"))?;
        }
        Ok(number)
    }
    pub fn boolean(&self, at: u32) -> Result<bool, Error> {
        match self.kind(at)? {
            Kind::True => Ok(true),
            Kind::False => Ok(false),
            Kind::ObjectStart
            | Kind::ObjectEnd
            | Kind::ArrayStart
            | Kind::ArrayEnd
            | Kind::Key
            | Kind::String
            | Kind::Number
            | Kind::Null
            | Kind::Long => Err(error(b"JSON", b"expected boolean")),
        }
    }
    pub fn words(&self, at: u32, most: u32, name: &[u8]) -> Result<c::Text, Error> {
        let bytes = self.string(at)?;
        if bytes.len() > usize::try_from(most).expect("u32 fits") {
            return Err(bound(name, most));
        }
        c::Text::new(bytes.into(), most).or(Err(error(name, b"UTF-8 required")))
    }
    pub fn document_text(&self, at: u32) -> Result<c::DocumentText, Error> {
        let fields = self.object(at, &[b"bytes", b"cut"])?;
        if self.boolean(field(&fields, b"cut")?)? {
            return Err(error(b"cut", b"whole value required"));
        }
        Ok(c::DocumentText::complete(self.words(
            field(&fields, b"bytes")?,
            self.limits.document_text_bytes,
            b"document_text_bytes",
        )?))
    }
    pub fn hex(&self, at: u32, most: u32, name: &[u8]) -> Result<Box<[u8]>, Error> {
        let bytes = self.string(at)?;
        if bytes.len() % 2 != 0 {
            return Err(error(name, b"even hexadecimal bytes required"));
        }
        let count = bytes.len() / 2;
        if count > usize::try_from(most).expect("u32 fits") {
            return Err(bound(name, most));
        }
        let mut out = Writer::new(count);
        for pair in bytes.chunks_exact(2) {
            let high = digit(*pair.first().expect("pair"))?;
            let low = digit(*pair.get(1).expect("pair"))?;
            out.put(&[high.checked_mul(16).expect("hex digit").checked_add(low).expect("hex byte")])
                .expect("measured hex");
        }
        Ok(out.finish())
    }
    pub fn object(&self, start: u32, allowed: &[&[u8]]) -> Result<Fields, Error> {
        if self.kind(start)? != Kind::ObjectStart {
            return Err(error(b"JSON", b"expected object"));
        }
        let mut fields = List::with_capacity(u32::try_from(allowed.len()).expect("schema field count"));
        let mut depth = 0_u32;
        for at in start.checked_add(1).ok_or(error(b"JSON", b"position bound"))?..self.doc.len() {
            match self.kind(at)? {
                Kind::Key if depth == 0 => {
                    let key = self.string(at)?;
                    if !allowed.contains(&key) {
                        return Err(error(key, b"unknown member"));
                    }
                    if optional(&fields, key).is_some() {
                        return Err(error(key, b"duplicate member"));
                    }
                    fields
                        .push((key.into(), at.checked_add(1).ok_or(error(key, b"position bound"))?))
                        .or(Err(error(key, b"field count bound")))?;
                }
                Kind::ObjectStart | Kind::ArrayStart => depth = depth.checked_add(1).expect("bounded JSON depth"),
                Kind::ObjectEnd | Kind::ArrayEnd => {
                    if depth == 0 {
                        return Ok(fields);
                    }
                    depth = depth.checked_sub(1).expect("positive depth");
                }
                Kind::Key | Kind::String | Kind::Number | Kind::True | Kind::False | Kind::Null | Kind::Long => {}
            }
        }
        Err(error(b"JSON", b"unfinished object"))
    }
    pub fn array(&self, start: u32, most: u32, name: &[u8]) -> Result<List<u32>, Error> {
        if self.kind(start)? != Kind::ArrayStart {
            return Err(error(name, b"expected array"));
        }
        // Count before allocating; every element costs at least one token.
        let mut count = 0_u32;
        let mut depth = 0_u32;
        for at in start.checked_add(1).ok_or(error(name, b"position bound"))?..self.doc.len() {
            let kind = self.kind(at)?;
            if depth == 0 && kind == Kind::ArrayEnd {
                break;
            }
            if depth == 0 {
                count = count.checked_add(1).ok_or(bound(name, most))?;
                if count > most {
                    return Err(bound(name, most));
                }
            }
            match kind {
                Kind::ArrayStart | Kind::ObjectStart => depth = depth.checked_add(1).expect("bounded JSON depth"),
                Kind::ArrayEnd | Kind::ObjectEnd => depth = depth.checked_sub(1).expect("balanced JSON"),
                Kind::Key | Kind::String | Kind::Number | Kind::True | Kind::False | Kind::Null | Kind::Long => {}
            }
        }
        let mut items = List::with_capacity(count);
        depth = 0;
        for at in start.checked_add(1).ok_or(error(name, b"position bound"))?..self.doc.len() {
            let kind = self.kind(at)?;
            if depth == 0 && kind == Kind::ArrayEnd {
                break;
            }
            if depth == 0 {
                items.push(at).expect("counted elements");
            }
            match kind {
                Kind::ArrayStart | Kind::ObjectStart => depth = depth.checked_add(1).expect("bounded JSON depth"),
                Kind::ArrayEnd | Kind::ObjectEnd => depth = depth.checked_sub(1).expect("balanced JSON"),
                Kind::Key | Kind::String | Kind::Number | Kind::True | Kind::False | Kind::Null | Kind::Long => {}
            }
        }
        Ok(items)
    }
}
fn digit(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte.checked_sub(b'0').expect("ASCII digit")),
        b'a'..=b'f' => Ok(byte.checked_sub(b'a').expect("hex letter").checked_add(10).expect("hex digit")),
        _ => Err(error(b"JSON", b"lower-case hexadecimal required")),
    }
}
pub(super) fn optional(fields: &Fields, name: &[u8]) -> Option<u32> {
    for (key, at) in fields {
        if key.as_ref() == name {
            return Some(*at);
        }
    }
    None
}
pub(super) fn field(fields: &Fields, name: &[u8]) -> Result<u32, Error> {
    optional(fields, name).ok_or(error(name, b"missing member"))
}
pub(crate) fn selector(call: &Call) -> c::Setting {
    if call.form == 1 {
        return c::Setting::Rules;
    }
    let number =
        c::SettingNumber { number: u32::try_from(number(call, 1).expect("setting number")).expect("parsed u32") };
    match value(call, 0) {
        Some(Value::Word(0)) => c::Setting::Charter(number),
        Some(Value::Word(1)) => c::Setting::Template(number),
        Some(Value::Word(2)) => c::Setting::Policy(number),
        Some(_) | None => unreachable!("parsed setting kind"),
    }
}
fn setting(d: &Decoder, at: u32) -> Result<c::Setting, Error> {
    let fields = d.object(at, &[b"kind", b"number"])?;
    let kind = d.string(field(&fields, b"kind")?)?;
    if kind == b"rules" {
        if fields.len() != 1 {
            return Err(error(b"number", b"rules has no number"));
        }
        return Ok(c::Setting::Rules);
    }
    let number = c::SettingNumber {
        number: u32::try_from(d.number(field(&fields, b"number")?)?).or(Err(error(b"number", b"u32 bound")))?,
    };
    match kind {
        b"charter" => Ok(c::Setting::Charter(number)),
        b"template" => Ok(c::Setting::Template(number)),
        b"policy" => Ok(c::Setting::Policy(number)),
        _ => Err(error(b"kind", b"unknown setting kind")),
    }
}
fn item(d: &Decoder, at: u32) -> Result<c::SettingItem, Error> {
    let fields = d.object(at, &[b"setting", b"set", b"edit"])?;
    let setting = setting(d, field(&fields, b"setting")?)?;
    let edit = match (optional(&fields, b"set"), optional(&fields, b"edit")) {
        (Some(at), None) => {
            let value = match setting {
                c::Setting::Charter(_) => c::SettingValue::Charter(Box::new(values::charter(d, at)?)),
                c::Setting::Template(_) => c::SettingValue::Template(Box::new(values::template(d, at)?)),
                c::Setting::Policy(_) => c::SettingValue::Policy(Box::new(values::setting_policy(d, at)?)),
                c::Setting::Rules => c::SettingValue::Rules(Box::new(values::rules(d, at)?)),
            };
            c::SettingEdit::Set(c::SettingSet { value })
        }
        (None, Some(at)) => match d.string(at)? {
            b"removed" => c::SettingEdit::Removed,
            b"reset" => c::SettingEdit::Reset,
            _ => return Err(error(b"edit", b"removed or reset required")),
        },
        _ => return Err(error(b"item", b"exactly one of set or edit required")),
    };
    Ok(c::SettingItem { setting, edit })
}
pub(crate) fn decode(bytes: &[u8], limits: &Limits) -> Result<List<c::SettingItem>, Box<[u8]>> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.ask_bytes {
        return Err(message(None, bound(b"file bytes", u32::try_from(limits.ask_bytes).unwrap_or(u32::MAX))));
    }
    let doc = super::json::document_depth(bytes, 32)
        .or(Err(message(None, error(b"JSON", b"invalid JSON or depth bound 32"))))?;
    let d = Decoder { doc, limits: limits.settings };
    let fields = match d.object(0, &[b"items"]) {
        Ok(fields) => fields,
        Err(why) => return Err(message(None, why)),
    };
    // The wire carries length-prefixed items in an ask-sized blob. Even an
    // empty item occupies bytes; the exact aggregate is checked below too.
    let at = match field(&fields, b"items") {
        Ok(at) => at,
        Err(why) => return Err(message(None, why)),
    };
    let positions = match d.array(at, 1024, b"items") {
        Ok(positions) => positions,
        Err(why) => return Err(message(None, why)),
    };
    let mut items: List<c::SettingItem> = List::with_capacity(positions.len());
    let mut encoded = 0_u32;
    for (index, at) in positions.iter().enumerate() {
        let index = u32::try_from(index).expect("bounded items").checked_add(1).expect("item count fits");
        let next = match item(&d, *at) {
            Ok(item) => item,
            Err(why) => return Err(message(Some(index), why)),
        };
        for previous in &items {
            if previous.setting == next.setting {
                return Err(message(Some(index), error(b"setting", b"duplicate setting")));
            }
        }
        let row = match jig_client_protocol_link::translate::settings::wire_setting_item(&next, &limits.settings) {
            Ok(row) => row,
            Err(problem) => return Err(message(Some(index), wire_error(problem, &limits.settings))),
        };
        encoded = encoded
            .checked_add(row.measure())
            .ok_or(message(Some(index), bound(b"encoded items bytes", limits.settings.change_settings_items)))?;
        encoded = encoded
            .checked_add(4)
            .ok_or(message(Some(index), bound(b"encoded items bytes", limits.settings.change_settings_items)))?;
        if encoded > limits.settings.change_settings_items {
            return Err(message(Some(index), bound(b"encoded items bytes", limits.settings.change_settings_items)));
        }
        items.push(next).expect("counted items");
    }
    let ask = c::Ask::ChangeSettings(c::ChangeSettings { against: 0, items, running: c::Running::Stays });
    let wire = jig_client_protocol_link::translate::asks::ask(&ask, &limits.settings)
        .or(Err(message(None, bound(b"encoded items bytes", limits.settings.change_settings_items))))?;
    if u64::from(wire.measure()) > limits.ask_bytes {
        return Err(message(
            None,
            bound(b"encoded request bytes", u32::try_from(limits.ask_bytes).unwrap_or(u32::MAX)),
        ));
    }
    let c::Ask::ChangeSettings(ask) = ask else {
        unreachable!("settings request");
    };
    Ok(ask.items)
}
pub(crate) fn request(call: &Call, limits: &Limits) -> Result<c::ChangeSettings, Box<[u8]>> {
    let Some(Value::Text(bytes)) = value(call, 0) else {
        return Err(message(None, error(b"FILE", b"input not loaded")));
    };
    Ok(c::ChangeSettings {
        against: u32::try_from(
            number(call, if call.tool == crate::jig::rows::PROPOSE_SETTINGS_AT { 2 } else { 1 }).expect("against"),
        )
        .expect("parsed u32"),
        items: decode(bytes, limits)?,
        running: match value(call, 2) {
            Some(Value::Word(1)) => c::Running::Moves,
            Some(_) | None => c::Running::Stays,
        },
    })
}

#[expect(clippy::wildcard_enum_match_arm, reason = "other wire paths are not settings value bounds")]
fn wire_error(problem: jig_client_protocol_wire::Problem, limits: &jig_client_protocol_wire::Limits) -> Error {
    use jig_client_protocol_wire::Path;
    match problem.path {
        Path::CharterAlternatives => bound(b"charter_alternatives", limits.charter_alternatives),
        Path::CharterInstructions => bound(b"charter_instructions", limits.charter_instructions),
        Path::FieldRuleName => bound(b"field_rule_name", limits.field_rule_name),
        Path::LlmChoiceSubagents => bound(b"llm_choice_subagents", limits.llm_choice_subagents),
        Path::ModelEffortEffort => bound(b"model_effort_effort_some", limits.model_effort_effort_some),
        Path::ModelEffortModel => bound(b"model_effort_model", limits.model_effort_model),
        Path::ModelSetNamesNames => bound(b"model_set_names_names", limits.model_set_names_names),
        Path::OwnedRecord => bound(b"owned_record", limits.owned_record),
        Path::OwnerAuthorityGrants => bound(b"owner_authority_grants", limits.owner_authority_grants),
        Path::OwnerAuthorityNoteResources => {
            bound(b"owner_authority_note_resources", limits.owner_authority_note_resources)
        }
        Path::OwnerDelegationKinds => bound(b"owner_delegation_kinds", limits.owner_delegation_kinds),
        Path::OwnerPartKeyPathValue => bound(b"owner_part_key_path_value", limits.owner_part_key_path_value),
        Path::OwnerPatternEndExactValue => {
            bound(b"owner_pattern_end_exact_value", limits.owner_pattern_end_exact_value)
        }
        Path::OwnerPatternEndOpenValue => bound(b"owner_pattern_end_open_value", limits.owner_pattern_end_open_value),
        Path::OwnerPatternSegments => bound(b"owner_pattern_segments", limits.owner_pattern_segments),
        Path::RulesImplies => bound(b"rules_implies", limits.rules_implies),
        Path::RulesRequirements => bound(b"rules_requirements", limits.rules_requirements),
        Path::SettingModelEffort => bound(b"setting_model_effort_some", limits.setting_model_effort_some),
        Path::SettingModelEfforts => bound(b"setting_model_efforts", limits.setting_model_efforts),
        Path::SettingModelName => bound(b"setting_model_name", limits.setting_model_name),
        Path::SettingModelProgram => bound(b"setting_model_program_some", limits.setting_model_program_some),
        Path::SettingPolicyPermissions => bound(b"setting_policy_permissions", limits.setting_policy_permissions),
        Path::SettingPolicyProjections => bound(b"setting_policy_projections", limits.setting_policy_projections),
        Path::SettingPolicyRequirements => bound(b"setting_policy_requirements", limits.setting_policy_requirements),
        Path::SettingPolicyRoles => bound(b"setting_policy_roles", limits.setting_policy_roles),
        Path::SettingPolicyTemplates => bound(b"setting_policy_templates", limits.setting_policy_templates),
        Path::TaskContractDocumentApprovalState => {
            bound(b"task_contract_document_approval_state", limits.task_contract_document_approval_state)
        }
        Path::TaskContractDocumentVerdictChoices => {
            bound(b"task_contract_document_verdict_choices", limits.task_contract_document_verdict_choices)
        }
        Path::VerdictChoiceFields => bound(b"verdict_choice_fields", limits.verdict_choice_fields),
        _ => error(b"value", b"wire value bound"),
    }
}

/// Verdict choices, like authority grants and settings items, are sized wire
/// pieces rather than a counted wire list. Check their byte envelope too.
pub(super) fn verdict_bound(
    choices: &List<c::VerdictChoice>,
    limits: &jig_client_protocol_wire::Limits,
) -> Result<(), Error> {
    use jig_client_protocol_wire as w;
    let mut size = 0_u32;
    for choice in choices {
        let mut fields = List::with_capacity(choice.fields.len());
        for field in &choice.fields {
            let field = match w::FieldRule::new(limits, w::FieldRuleParts { name: field.name.clone(), max: field.max })
            {
                Ok(field) => field,
                Err(problem) => return Err(wire_error(problem, limits)),
            };
            fields.push(field).expect("same field count");
        }
        let row = match w::VerdictChoice::new(
            limits,
            w::VerdictChoiceParts { code: choice.code, words: choice.words, followups: choice.followups, fields },
        ) {
            Ok(row) => row,
            Err(problem) => return Err(wire_error(problem, limits)),
        };
        size = size
            .checked_add(row.measure())
            .ok_or(bound(b"task_contract_document_verdict_choices", limits.task_contract_document_verdict_choices))?;
        size = size
            .checked_add(4)
            .ok_or(bound(b"task_contract_document_verdict_choices", limits.task_contract_document_verdict_choices))?;
        if size > limits.task_contract_document_verdict_choices {
            return Err(bound(
                b"task_contract_document_verdict_choices",
                limits.task_contract_document_verdict_choices,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{message, wire_error};
    use alloc::boxed::Box;
    use jig_client_protocol_wire as wire;

    #[test]
    fn an_unlisted_wire_path_keeps_the_generic_diagnostic_words() {
        let mut limits = wire::CEILINGS;
        limits.keyed_request = 0;
        let problem = wire::Keyed::new(
            &limits,
            wire::KeyedParts {
                key: [0; 16],
                owner: wire::Owner::Jig,
                version: 1,
                request: Box::from(b"x".as_slice()),
                value: None,
            },
        )
        .expect_err("one request byte is beyond the zero-byte fixture limit");
        assert_eq!(problem.path, wire::Path::KeyedRequest);
        assert_eq!(message(None, wire_error(problem, &limits)).as_ref(), b"settings file: value: wire value bound");
    }
}
