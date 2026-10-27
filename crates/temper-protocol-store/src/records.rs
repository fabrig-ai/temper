//! Total translations between durable domain rows and generated records.
use crate::{Bounds, Undecodable, v10 as w};
use alloc::boxed::Box;
use skein_lib::{Reader, Writer};
use temper_engine_domain_forge as d;
mod answers;
mod api;
mod application;
mod change;
mod client;
mod tiers;
mod verification;
#[cfg(test)]
pub(crate) use api::encode_error;
mod entries;
mod issues;
mod landing;
mod names;
mod topics;
const FITS: &str = "admitted fields fit the schema";

/// Encode every durable family admitted by the domain.
#[must_use]
pub fn encode(value: &d::Stored) -> Box<[u8]> {
    let value = match value {
        d::Stored::Landing(row) => w::Value::Landing(landing::encode(row)),
        d::Stored::Receipt { key, receipt } => w::Value::Receipt(landing::encode_receipt(key, receipt)),
        d::Stored::Application { token, until, outcome } => {
            w::Value::Application(application::encode(*token, *until, outcome))
        }
        d::Stored::Repository(value) => w::Value::Repository(names::encode_adopted(value)),
        d::Stored::Hold(value) => w::Value::Hold(names::encode_hold(value)),
        d::Stored::Names { task, resources } => w::Value::Names(topics::encode_names(*task, resources)),
        d::Stored::Subscription(value) => w::Value::Subscription(topics::encode_subscriber(value)),
        d::Stored::BranchHead(value) => w::Value::BranchHead(topics::encode_branch_head(value)),
        d::Stored::PullState(value) => w::Value::PullState(topics::encode_pull_state(value)),
        d::Stored::Ci(value) => w::Value::Ci(topics::encode_ci_state(value)),
        d::Stored::Landed { commit, task } => w::Value::Landed(topics::encode_landed(*commit, *task)),
        d::Stored::Release(value) => w::Value::Release(topics::encode_release(value)),
        d::Stored::ProposedEffect(value) => w::Value::ProposedEffect(entries::encode_proposed(value)),
        d::Stored::Entry { entry, held } => w::Value::Entry(entries::encode_entry_row(entry, *held)),
        d::Stored::Call { key, answer } => w::Value::Call(entries::encode_call(key, answer)),
        d::Stored::Binding { address, subject } => w::Value::Binding(
            w::Binding::new(&w::CEILINGS, w::BindingParts { address: address.clone(), subject: subject.clone() })
                .expect(FITS),
        ),
        d::Stored::Tier(row) => w::Value::Tier(Box::new(tiers::encode(row))),
        d::Stored::Verification(row) => w::Value::Verification(verification::encode(row)),
        d::Stored::Flight { task, entry } => {
            w::Value::Flight(w::Flight::new(&w::CEILINGS, w::FlightParts { task: *task, entry: *entry }).expect(FITS))
        }
        d::Stored::DemoAct(row) => w::Value::DemoAct(
            w::DemoAct::new(
                &w::CEILINGS,
                w::DemoActParts {
                    key: row.key,
                    act: match row.act {
                        d::DemoActKind::Close { project, task } => w::DemoActKind::Close(
                            w::DemoClose::new(&w::CEILINGS, w::DemoCloseParts { project, task }).expect(FITS),
                        ),
                        d::DemoActKind::RemoveAccount { account } => w::DemoActKind::RemoveAccount(
                            w::DemoRemoveAccount::new(&w::CEILINGS, w::DemoRemoveAccountParts { account }).expect(FITS),
                        ),
                    },
                },
            )
            .expect(FITS),
        ),
        d::Stored::Demo(value) => w::Value::Demo(
            w::Demo::new(
                &w::CEILINGS,
                w::DemoParts {
                    forge: value.forge,
                    state: match value.state {
                        d::DemoState::Running => w::DemoState::Running,
                        d::DemoState::Ended => w::DemoState::Ended,
                    },
                },
            )
            .expect(FITS),
        ),
        d::Stored::Writer(value) => w::Value::Writer(
            w::ForgeWriter::new(&w::CEILINGS, w::ForgeWriterParts { forge: value.forge, author: value.author })
                .expect(FITS),
        ),
        d::Stored::Client(value) => w::Value::Client(client::encode(value)),
        d::Stored::Change(value) => w::Value::Change(Box::new(change::encode_change_row(value))),
        d::Stored::Issue(value) => w::Value::Issue(Box::new(issues::encode(value))),
    };
    let row = w::Row::new(&w::CEILINGS, w::RowParts { value }).expect("admitted row fits the schema");
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("row length fits"));
    row.encode(&mut writer).expect("measured room");
    writer.finish()
}

/// Decode one supported row with the checked startup bounds.
pub fn decode(bytes: &[u8], bounds: &Bounds) -> Result<d::Stored, Undecodable> {
    let row = w::Row::decode(bounds.limits(), &mut Reader::new(bytes)).or(Err(Undecodable))?;
    Ok(match row.into_parts().value {
        w::Value::Landing(row) => d::Stored::Landing(landing::decode(row)),
        w::Value::Receipt(row) => landing::decode_receipt(row)?,
        w::Value::Application(value) => application::decode(value, bounds)?,
        w::Value::Issue(value) => d::Stored::Issue(issues::decode(*value)),
        w::Value::Change(value) => d::Stored::Change(change::decode_change_row(*value)),
        w::Value::Binding(value) => {
            let w::BindingParts { address, subject } = value.into_parts();
            if !d::binding_address(&address) {
                return Err(Undecodable);
            }
            d::Stored::Binding { address, subject }
        }
        w::Value::Tier(row) => d::Stored::Tier(tiers::decode(*row)?),
        w::Value::Verification(row) => d::Stored::Verification(verification::decode(row)?),
        w::Value::Flight(row) => {
            let w::FlightParts { task, entry } = row.into_parts();
            d::Stored::Flight { task, entry }
        }
        w::Value::DemoAct(row) => {
            let w::DemoActParts { key, act } = row.into_parts();
            let act = match act {
                w::DemoActKind::Close(row) => {
                    let w::DemoCloseParts { project, task } = row.into_parts();
                    d::DemoActKind::Close { project, task }
                }
                w::DemoActKind::RemoveAccount(row) => {
                    let w::DemoRemoveAccountParts { account } = row.into_parts();
                    d::DemoActKind::RemoveAccount { account }
                }
            };
            d::Stored::DemoAct(d::DemoAct { key, act })
        }
        w::Value::Demo(value) => {
            let w::DemoParts { forge, state } = value.into_parts();
            d::Stored::Demo(d::Demo {
                forge,
                state: match state {
                    w::DemoState::Running => d::DemoState::Running,
                    w::DemoState::Ended => d::DemoState::Ended,
                },
            })
        }
        w::Value::Writer(value) => {
            let w::ForgeWriterParts { forge, author } = value.into_parts();
            if author == 0 {
                return Err(Undecodable);
            }
            d::Stored::Writer(skein_forge_client::Writer { forge, author })
        }
        w::Value::Client(value) => d::Stored::Client(client::decode(value, bounds)?),
        w::Value::Call(value) => entries::decode_call(value, bounds)?,
        w::Value::ProposedEffect(value) => d::Stored::ProposedEffect(entries::decode_proposed(value, bounds)?),
        w::Value::Entry(value) => entries::decode_entry_row(value),
        w::Value::Repository(value) => d::Stored::Repository(names::decode_adopted(value, bounds)?),
        w::Value::Hold(value) => d::Stored::Hold(names::decode_hold(value, bounds)?),
        w::Value::Names(value) => {
            let w::NamesParts { task, resources } = value.into_parts();
            d::Stored::Names { task, resources: topics::decode_names(resources, bounds)? }
        }
        w::Value::Subscription(value) => d::Stored::Subscription(topics::decode_subscriber(value)),
        w::Value::BranchHead(value) => d::Stored::BranchHead(topics::decode_branch_head(value, bounds)?),
        w::Value::PullState(value) => d::Stored::PullState(topics::decode_pull_state(value, bounds)?),
        w::Value::Ci(value) => d::Stored::Ci(topics::decode_ci_state(value)),
        w::Value::Landed(value) => {
            let w::LandedParts { commit, task } = value.into_parts();
            d::Stored::Landed { commit, task }
        }
        w::Value::Release(value) => d::Stored::Release(topics::decode_release(value, bounds)?),
    })
}

/// Restore a decoded record only when its durable key agrees with the row.
pub fn row(key: &[u8], value: &[u8], bounds: &Bounds) -> Result<d::Stored, Undecodable> {
    let row = decode(value, bounds)?;
    let encoded = crate::keys::encode(&d::stored_key(&row), u32::MAX).ok_or(Undecodable)?;
    if encoded.as_ref() != key {
        return Err(Undecodable);
    }
    Ok(row)
}

pub(crate) fn limits(forge: &d::Limits) -> Option<w::Limits> {
    Some(w::Limits {
        tier_flight_pending: forge.client.rows,
        tier_row_tier: u32::try_from(d::CHECK_NAME_BYTES).ok()?,
        tier_row_pending: forge.changes.checked_mul(2)?.checked_add(1)?,
        tier_run_landings: forge.changes.checked_mul(2)?.checked_add(1)?,
        tier_finding_blamed: forge.changes.checked_mul(2)?.checked_add(1)?,
        tier_finding_flakes: forge.failing_tests,
        tier_record_flakes: forge.failing_tests,
        tier_flake_test: forge.test_id_bytes,
        tier_tests_tests: forge.failing_tests,
        verification_changes: forge.changes,
        verification_tiers: forge.tiers,
        tier_verification_tier: u32::try_from(d::CHECK_NAME_BYTES).ok()?,
        failed_tests_tests: forge.failing_tests,
        failing_test_test_id: forge.test_id_bytes,
        landing_state_members: forge.landing_members,
        landing_member_gates: forge.gates,
        landing_row_asks: forge.landing_members,
        landing_row_entries: forge.entries,
        receipt_name_name: 32,
        receipt_run_url: 2048,
        answer_name_name: forge.client.answer_bytes,
        binding_address: 254,
        binding_subject: 255,
        application_adopted_owner: forge.name_bytes,
        application_adopted_name: forge.name_bytes,
        plan_item_text: forge.issue_policy.comment_bytes,
        milestone_text: forge.issue_policy.comment_bytes,
        goal_view_title: forge.issue_policy.title_bytes,
        goal_view_goal_text: forge.issue_policy.body_bytes,
        goal_view_plan: forge.issue_policy.plan_items,
        goal_view_milestones: forge.issue_policy.milestones,
        goal_view_finished_some: forge.issue_policy.comment_bytes,
        projected_milestones: forge.issue_policy.milestones,
        gating_asked: forge.change_policy.gates,
        change_gates: forge.change_policy.gates,
        change_clean: forge.change_policy.clean_heads,
        change_gate_failures: forge.change_policy.gates,
        change_row_branch: forge.name_bytes,
        change_row_base: forge.name_bytes,
        change_row_title: forge.client.op_bytes,
        change_row_body: forge.client.op_bytes,
        change_row_verdicts: forge.change_policy.gates,
        change_row_gate_remarks: forge.change_policy.gates,
        gate_remark_words: forge.client.answer_bytes,
        live_branch_branch: forge.name_bytes,
        live_record_comments: forge.client.inbox,
        live_record_reviews: forge.client.inbox,
        live_record_echoes: forge.client.rows,
        status_context: forge.client.answer_bytes,
        status_description: forge.client.answer_bytes,
        status_url: forge.client.answer_bytes,
        remark_path: forge.client.answer_bytes,
        remark_body: forge.client.answer_bytes,
        file_path: forge.client.answer_bytes,
        file_before_some: forge.client.answer_bytes,
        file_after_some: forge.client.answer_bytes,
        statuses_answer_statuses: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        remarks_answer_remarks: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        pull_files_answer_files: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        compare_answer_files: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        compare_answer_commits: forge.client.rows,
        checks_answer_statuses: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        collaborators_answer_collaborators: forge.client.rows,
        summary_labels: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(8)?)?,
        summary_key_some: forge.client.answer_bytes,
        summary_title: forge.client.answer_bytes,
        summary_body: forge.client.answer_bytes,
        comment_key_some: forge.client.answer_bytes,
        comment_body: forge.client.answer_bytes,
        review_key_some: forge.client.answer_bytes,
        review_body: forge.client.answer_bytes,
        items_answer_items: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        item_answer_comments: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        reviews_answer_reviews: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(4)?)?,
        pull_reviewers: forge.client.rows,
        pull_head: forge.client.answer_bytes,
        pull_base: forge.client.answer_bytes,
        step_name: forge.client.answer_bytes,
        job_answer_log: forge.client.answer_bytes,
        file_answer_path: forge.client.answer_bytes,
        file_answer_bytes: forge.client.answer_bytes,
        branches_answer_branches: forge.client.answer_bytes.checked_add(forge.client.rows.checked_mul(8)?)?,
        ..w::CEILINGS
    })
}

#[cfg(test)]
#[path = "tests/holds.rs"]
mod holds;
