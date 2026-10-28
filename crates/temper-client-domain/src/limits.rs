//! The pages and forge share jig's receiving and request bounds.
use crate::forge;
use temper_client_protocol_wire::v1 as w;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub forge: forge::Limits,
    pub client: jig_client::Limits,
    pub accounts: AccountLimits,
}
/// Interim view counts until jig owns receiving counts (owner, 17:51Z; x1g).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountLimits {
    pub accounts: u32,
    pub sources: u32,
    pub uses: u32,
    pub sign_ins: u32,
}
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    jig_client::max_out(&limits.client)
        .checked_mul(4)
        .expect("root output bound fits")
        .checked_add(1)
        .expect("welcome output bound fits")
}

/// Refuses unusable client limits, unrepresentable page counts and forge wire bounds.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    // jig's valid() replaces this at the set's repin
    if jig_client::worst_case(&limits.client).is_none() || limits.forge.field_bytes < 7 {
        return false;
    }
    let Some(wire) = forge::wire_limits(&limits.forge) else { return false };
    let Some(lines) = limits.client.domain.rows.checked_mul(4) else { return false };
    let Ok(document) = u32::try_from(limits.client.domain.document_bytes) else { return false };
    lines.checked_add(1).is_some()
        && limits.client.domain.rows.checked_mul(3).is_some()
        && document.checked_add(1).is_some()
        && w::Answer::worst_case_heap(&wire).is_some()
        && w::DemoAnswer::worst_case_heap(&wire).is_some()
        && w::ChangePanel::worst_case_heap(&wire).is_some()
        && w::Page::worst_case_heap(&wire).is_some()
        && w::Forges::worst_case_heap(&wire).is_some()
}
