//! Shared settings form and words, independent of either client or engine ownership.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod file;
mod json;
pub mod page;
pub mod render;
mod text;
pub mod values;
pub mod view;
pub mod wire;
use alloc::boxed::Box;
use skein_json::writer::Encoder;
pub use text::{Text, text_end};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub settings: jig_client_protocol_wire::Limits,
    pub ask_bytes: u64,
    pub result_bytes: u64,
    pub text: u32,
    pub rows: u32,
    pub authority: u32,
    pub fields: u32,
}
/// Render with the same structural/text split used by ctl.
#[must_use]
pub fn settings_text(value: &view::ReadValue, requested: Option<u64>, limits: &Limits) -> Box<[u8]> {
    let mut measure = Text::measure();
    render::text(value, requested, limits, &mut measure);
    let structural = measure.structural();
    let mut measure = Text::bounded(limits.text, structural);
    render::text(value, requested, limits, &mut measure);
    let mut out = measure.writing(limits.text, structural);
    render::text(value, requested, limits, &mut out);
    out.finish()
}
pub(crate) fn utc(wall: skein_lib::Wall, out: &mut Text) {
    let minute = wall.as_secs() / 60;
    let hour = (minute / 60) % 24;
    if hour < 10 {
        out.put(b"0");
    }
    out.number(hour);
    out.put(b":");
    if minute % 60 < 10 {
        out.put(b"0");
    }
    out.number(minute % 60);
}
pub(crate) fn hex(bytes: &[u8], out: &mut Encoder) {
    let mut hex = skein_lib::Writer::new(bytes.len().checked_mul(2).expect("document bound fits"));
    for byte in bytes {
        for digit in [byte >> 4u32, byte & 15] {
            hex.put(&[if digit < 10 {
                b'0'.checked_add(digit).expect("digit")
            } else {
                b'a'.checked_add(digit.checked_sub(10).expect("hex letter")).expect("letter")
            }])
            .expect("measured hex");
        }
    }
    out.string(&hex.finish());
}

impl Limits {
    /// Existing ctl settings bounds, also used by the host's neutral adapter.
    #[must_use]
    pub fn standard() -> Self {
        let mut settings = jig_client_protocol_wire::CEILINGS;
        settings.owned_record = 1_048_576;
        settings.document_text_bytes = 32_768;
        settings.owner_part_key_path_value = 9;
        settings.owner_part_key_path_value_item = 32_768;
        settings.owner_pattern_end_exact_value = 32_768;
        settings.owner_pattern_end_open_value = 32_768;
        settings.owner_pattern_segments = 9;
        settings.owner_pattern_segments_item = 32_768;
        settings.owner_delegation_kinds = 12;
        settings.owner_authority_grants = 1_048_576;
        settings.owner_authority_note_resources = 1_048_576;
        settings.field_rule_name = 32_768;
        settings.verdict_choice_fields = 32;
        settings.task_contract_document_approval_state = 32_768;
        settings.task_contract_document_verdict_choices = 1_048_576;
        settings.propose_settings_items = 1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.propose_settings_items);
        settings.settings_review_items = 1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.settings_review_items);
        settings.owner_proposal_action_settings_items =
            jig_client_protocol_wire::CEILINGS.owner_proposal_action_settings_items;
        settings.owner_retained_action_settings_items =
            jig_client_protocol_wire::CEILINGS.owner_retained_action_settings_items;
        // Whole settings share the existing receiving text, rows and message envelopes.
        settings.charter_instructions = 1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.charter_instructions);
        settings.change_settings_items = 1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.change_settings_items);
        settings.revision_document_items =
            1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.revision_document_items);
        settings.setting_model_name = 32_768_u32.min(jig_client_protocol_wire::CEILINGS.setting_model_name);
        settings.setting_model_program_some =
            32_768_u32.min(jig_client_protocol_wire::CEILINGS.setting_model_program_some);
        settings.setting_model_effort_some =
            32_768_u32.min(jig_client_protocol_wire::CEILINGS.setting_model_effort_some);
        settings.setting_model_efforts_item =
            32_768_u32.min(jig_client_protocol_wire::CEILINGS.setting_model_efforts_item);
        settings.charter_alternatives = 32_u32.min(jig_client_protocol_wire::CEILINGS.charter_alternatives);
        settings.setting_model_efforts = 32_u32.min(jig_client_protocol_wire::CEILINGS.setting_model_efforts);
        settings.setting_policy_projections = 32_u32.min(jig_client_protocol_wire::CEILINGS.setting_policy_projections);
        settings.setting_policy_roles = 32_u32.min(jig_client_protocol_wire::CEILINGS.setting_policy_roles);
        settings.setting_policy_requirements =
            32_u32.min(jig_client_protocol_wire::CEILINGS.setting_policy_requirements);
        settings.setting_policy_permissions = 32_u32.min(jig_client_protocol_wire::CEILINGS.setting_policy_permissions);
        settings.setting_policy_templates = 32_u32.min(jig_client_protocol_wire::CEILINGS.setting_policy_templates);
        settings.rules_implies = 32_u32.min(jig_client_protocol_wire::CEILINGS.rules_implies);
        settings.rules_requirements = 32_u32.min(jig_client_protocol_wire::CEILINGS.rules_requirements);
        // Settings are a whole inventory, independent of a page's row limit.
        settings.settings_document_settings =
            1_048_576_u32.min(jig_client_protocol_wire::CEILINGS.settings_document_settings);
        settings.settings_document_held = 32_u32.min(jig_client_protocol_wire::CEILINGS.settings_document_held);
        settings.settings_document_pending = 32_u32.min(jig_client_protocol_wire::CEILINGS.settings_document_pending);
        settings.revision_row_items = 32_u32.min(jig_client_protocol_wire::CEILINGS.revision_row_items);
        settings.revisions_page_rows = 32_u32.min(jig_client_protocol_wire::CEILINGS.revisions_page_rows);
        settings.refusal_full_tasks = 32_u32.min(jig_client_protocol_wire::CEILINGS.refusal_full_tasks);

        Self {
            settings,
            ask_bytes: 1_048_576,
            result_bytes: 6_292_480,
            text: 32_768,
            rows: 32,
            authority: 12,
            fields: 16,
        }
    }
}

/// Settings refusal facts shared with ctl's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Stale(u32),
    Malformed { at: u32, setting: view::Setting, fault: view::SettingFault },
    Unchanged,
    Authority,
    Limit,
    NotOffered,
}
#[must_use]
pub fn refusal_text(why: &Refusal) -> Box<[u8]> {
    let mut measure = Text::measure();
    refusal(why, &mut measure);
    let mut out = Text::writing_exact(measure);
    refusal(why, &mut out);
    out.finish()
}
pub fn refusal(why: &Refusal, out: &mut Text) {
    match why {
        Refusal::Stale(newest) => {
            out.put(b"stale; newest revision ");
            out.number(u64::from(*newest));
            out.put(b"; read settings show again");
        }
        Refusal::Malformed { at, setting, fault } => {
            out.put(b"out of shape; item ");
            out.number(u64::from(*at));
            out.put(b"; ");
            render::setting_text(*setting, out);
            out.put(b": ");
            out.put(render::fault_word(*fault));
        }
        Refusal::Unchanged => out.put(b"settings unchanged"),
        Refusal::Authority => out.put(b"Authority missing"),
        Refusal::Limit => out.put(b"Limit reached"),
        Refusal::NotOffered => out.put(b"This act is not offered"),
    }
}
