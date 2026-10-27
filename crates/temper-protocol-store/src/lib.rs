//! Temper's ordered families under jig's application tag (protocol/store.md).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
pub mod keys;
pub mod records;
#[rustfmt::skip]
#[path = "generated/v10.rs"]
pub mod v10;
#[cfg(test)]
mod tests;

/// One version for every durable family (protocol/store.md, section 4).
pub const STORE_VERSION: u32 = 10;

/// Largest encodings under the forge's checked bounds, before jig's key tag.
#[must_use]
pub fn application(limits: &temper_engine_domain_forge::Limits) -> Option<jig_protocol_store::sizes::Application> {
    let bounds = bounds(limits)?;
    Some(jig_protocol_store::sizes::Application {
        version: STORE_VERSION,
        key: keys::largest(limits)?,
        value: u32::try_from(v10::Row::worst_case_bytes(bounds.limits())?).ok()?,
    })
}

/// A malformed row, which the routing reports before stopping the engine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Undecodable;

/// Schema limits checked once alongside the forge's key bounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bounds {
    limits: v10::Limits,
    forge: temper_engine_domain_forge::Limits,
}
impl Bounds {
    /// The generated decoder's limits.
    #[must_use]
    pub const fn limits(&self) -> &v10::Limits {
        &self.limits
    }
    /// The domain limits for names and key decoding.
    #[must_use]
    pub const fn forge(&self) -> &temper_engine_domain_forge::Limits {
        &self.forge
    }
}

/// Make schema bounds from the domain, refusing any ceiling overflow.
#[must_use]
pub fn bounds(forge: &temper_engine_domain_forge::Limits) -> Option<Bounds> {
    let limits = v10::Limits {
        dispatch_input_name: 255,
        dispatch_input_value: forge.client.op_bytes,
        dispatch_write_workflow: 255,
        dispatch_write_branch: forge.name_bytes,
        dispatch_write_inputs: u32::try_from(skein_forge::api::INPUTS).ok()?,
        set_status_description: 140,
        set_status_link: 2048,
        create_issue_key: forge.client.op_bytes,
        create_issue_title: forge.client.op_bytes,
        create_issue_body: forge.client.op_bytes,
        open_pull_title: forge.client.op_bytes,
        open_pull_body: forge.client.op_bytes,
        open_pull_head: forge.client.op_bytes,
        open_pull_base: forge.client.op_bytes,
        release_row_comment_some: forge.client.op_bytes,
        post_key: forge.client.op_bytes,
        post_body: forge.client.op_bytes,
        post_review_key: forge.client.op_bytes,
        post_review_body: forge.client.op_bytes,
        edit_title_some: forge.client.op_bytes,
        edit_body_some: forge.client.op_bytes,
        set_reviewers_reviewers: forge.client.rows.min(forge.client.op_bytes.checked_div(8)?),
        set_status_context: forge.client.op_bytes,
        create_branch_branch: forge.client.op_bytes,
        delete_branch_branch: forge.client.op_bytes,
        merge_condition_base: forge.client.op_bytes,
        segment_name: forge.name_bytes,
        segments_segments: forge.name_bytes.checked_mul(8)?,
        protection_branch: forge.client.answer_bytes,
        protection_contexts: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(8)?)?,
        settings_default_branch: forge.client.answer_bytes,
        adopted_host: forge.name_bytes,
        adopted_owner: forge.name_bytes,
        adopted_name: forge.name_bytes,
        adopted_prefix: forge.name_bytes,
        adopted_checks: forge.change_policy.gates,
        retargeted_before: forge.name_bytes,
        retargeted_after: forge.name_bytes,
        names_resources: forge.resources_per_task,
        landings_topic_branch: forge.name_bytes,
        subscriber_goal_tasks_some: forge.issue_policy.plan_items,
        subscriber_paths: forge.paths_per_subscription,
        subscriber_paths_item: forge.name_bytes,
        ..records::limits(forge)?
    };
    v10::check_limits(&limits).ok()?;
    Some(Bounds { limits, forge: *forge })
}

/// Every application key, relative to jig's application tag.
#[must_use]
pub fn interval() -> skein_kv::Range {
    skein_kv::Range { start: Box::new([]), end: None }
}
