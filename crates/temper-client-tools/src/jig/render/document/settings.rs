//! Client settings views adapt to the neutral settings renderer.
use super::Text;
use crate::jig::{Limits, settings_adapter as a};
use jig_client::contract as c;
use skein_json::writer::Encoder;
pub(crate) fn setting_text(setting: c::Setting, out: &mut Text) {
    settings_form::render::setting_text(a::form_setting(setting), out);
}
pub(super) fn encode_setting(setting: c::Setting, out: &mut Encoder) {
    settings_form::render::encode_setting(a::form_setting(setting), out);
}
pub(super) fn encode_value(value: &c::SettingValue, out: &mut Encoder) {
    settings_form::render::encode_value(&a::form_setting_value(value), out);
}
pub(super) fn value_inline(value: &c::SettingValue, limits: &Limits, out: &mut Text) {
    settings_form::render::value_inline(&a::form_setting_value(value), &a::limits(limits), out);
}
pub(crate) fn proposal_text(proposal: c::ProposalKey, out: &mut Text) {
    settings_form::render::proposal_text(a::form_proposal_key(&proposal), out);
}
pub(crate) fn text(value: &c::ReadValue, requested: Option<u64>, limits: &Limits, out: &mut Text) {
    settings_form::render::text(&a::form_read_value(value), requested, &a::limits(limits), out);
}
pub(super) fn edit_word(edit: &c::RevisionItemEdit) -> &'static [u8] {
    settings_form::render::edit_word(&a::form_revision_item_edit(edit))
}
pub(super) fn encode(value: &c::ReadValue, out: &mut Encoder) {
    settings_form::render::encode(&a::form_read_value(value), out);
}

pub(crate) fn fault_word(fault: c::SettingFault) -> &'static [u8] {
    settings_form::render::fault_word(a::form_setting_fault(fault))
}
