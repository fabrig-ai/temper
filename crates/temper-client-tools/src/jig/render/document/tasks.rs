//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_subscription_kind(
    value: &jig_client::contract::SubscriptionKind,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::SubscriptionKind::Task { target, held, result } => {
            out.object_start();
            out.key(b"task");
            out.object_start();
            out.key(b"target");
            out.unsigned(*(target));
            out.key(b"held");
            out.boolean(*(held));
            out.key(b"result");
            out.boolean(*(result));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::SubscriptionKind::Timer { at, period } => {
            out.object_start();
            out.key(b"timer");
            out.object_start();
            out.key(b"at");
            out.unsigned((at).as_nanos());
            out.key(b"period");
            match period {
                Some(item_1) => {
                    out.unsigned((item_1).as_nanos());
                }
                None => out.null(),
            }
            out.object_end();
            out.object_end();
        }
        jig_client::contract::SubscriptionKind::Topic { connector, topic } => {
            out.object_start();
            out.key(b"topic");
            out.object_start();
            out.key(b"connector");
            out.unsigned(u64::from(*(connector)));
            out.key(b"topic");
            out.unsigned(*(topic));
            out.object_end();
            out.object_end();
        }
    }
}

pub(super) fn encode_subscriptions(
    value: &jig_client::contract::Subscriptions,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"entries");
    out.array_start();
    for item_1 in &value.entries {
        super::encode_subscription(item_1, out, budget);
    }
    out.array_end();
    out.object_end();
}

pub(super) fn encode_taken_by(value: &jig_client::contract::TakenBy, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    out.key(b"number");
    out.unsigned(value.number);
    out.key(b"name");
    match &value.name {
        Some(item_1) => {
            budget.words((item_1).bytes(), out);
        }
        None => out.null(),
    }
    out.object_end();
}

pub(super) fn encode_task(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    let digest = budget.digest.take();
    out.object_start();
    encode_task_from_label(value, out, budget);
    encode_task_from_person_revision(value, out, budget);
    encode_task_from_hold(value, out, budget);
    encode_task_result_member(value, out, budget);
    encode_task_from_delegates(value, out, budget);
    super::subject_end(digest, out);
}

pub(super) fn encode_historical_task(
    value: &jig_client::contract::Task,
    result: &jig_client::contract::Result,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    let digest = budget.digest.take();
    out.object_start();
    encode_task_from_label(value, out, budget);
    encode_task_from_person_revision(value, out, budget);
    encode_task_from_hold(value, out, budget);
    encode_task_from_delegates(value, out, budget);
    out.key(b"result");
    super::encode_result(result, out, budget);
    super::subject_end(digest, out);
}

fn encode_task_from_label(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    out.key(b"label");
    out.unsigned(u64::from(value.label));
    out.key(b"flags");
    super::encode_task_flags(value.flags, out, budget);
    out.key(b"last_activity");
    out.unsigned(value.last_activity.as_nanos());
    out.key(b"waits_on");
    super::encode_waits_on(&value.waits_on, out, budget);
    out.key(b"chip");
    super::encode_chip(&value.chip, out, budget);
    out.key(b"goal");
    match &value.goal {
        Some(item_1) => {
            out.unsigned(*(item_1));
        }
        None => out.null(),
    }
    out.key(b"priority");
    match &value.priority {
        Some(item_2) => {
            out.unsigned(u64::from(*(item_2)));
        }
        None => out.null(),
    }
    out.key(b"lineage_chips");
    out.array_start();
    for item_3 in &value.lineage_chips {
        super::encode_chip(item_3, out, budget);
    }
    out.array_end();
    out.key(b"inputs");
    out.array_start();
    for item_4 in &value.inputs {
        super::encode_owner_task_link(item_4, out, budget);
    }
    out.array_end();
    out.key(b"project");
    out.unsigned(u64::from(value.project));
    out.key(b"requester");
    super::encode_owner_requester(&value.requester, out, budget);
    out.key(b"lineage");
    out.array_start();
    for item_5 in &value.lineage {
        out.unsigned(*(item_5));
    }
    out.array_end();
    out.key(b"spec");
    super::encode_specification_document(&value.spec, out, budget);
    super::llm::encode_task(value, out, budget);
    out.key(b"spec_revision");
    super::encode_revision(value.spec_revision, out, budget);
}
fn encode_task_from_person_revision(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    out.key(b"person_revision");
    match &value.person_revision {
        Some(item_6) => {
            super::encode_revision(*(item_6), out, budget);
        }
        None => out.null(),
    }
    out.key(b"taken_by");
    match &value.taken_by {
        Some(item_7) => {
            out.unsigned(*(item_7));
        }
        None => out.null(),
    }
    out.key(b"instructions");
    match &value.instructions {
        Some(item_8) => {
            super::encode_document_text(item_8, out, budget);
        }
        None => out.null(),
    }
    super::task_settings::encode(value.settings.as_ref(), out);
    out.key(b"wake");
    super::encode_owner_wake_policy(&value.wake, out, budget);
    out.key(b"subscriptions");
    encode_subscriptions(&value.subscriptions, out, budget);
    out.key(b"result_contract");
    super::encode_task_contract_document(&value.result_contract, out, budget);
    out.key(b"dependencies");
    out.array_start();
    for item_9 in &value.dependencies {
        super::encode_owner_task_link(item_9, out, budget);
    }
    out.array_end();
    out.key(b"dependency_facts");
    out.array_start();
    for fact in &value.dependency_facts {
        super::dependencies::encode(fact, out, budget);
    }
    out.array_end();
    out.key(b"references");
    out.array_start();
    for item_10 in &value.references {
        super::encode_owner_task_link(item_10, out, budget);
    }
    out.array_end();
    out.key(b"cut");
    out.boolean(value.cut);
    out.key(b"resources");
    out.array_start();
    for item_11 in &value.resources {
        super::encode_task_resource(item_11, out, budget);
    }
    out.array_end();
    out.key(b"budget");
    super::encode_owner_budget(&value.budget, out, budget);
    out.key(b"authority");
    super::encode_owner_authority_document(&value.authority, out, budget);
}
fn encode_task_from_hold(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    out.key(b"hold");
    match &value.hold {
        Some(item_12) => {
            super::encode_task_hold(item_12, out, budget);
        }
        None => out.null(),
    }
    out.key(b"proposals");
    out.array_start();
    for item_13 in &value.proposals {
        super::encode_proposal_key(item_13, out, budget);
    }
    out.array_end();
    out.key(b"activity");
    out.unsigned(value.activity);
    out.key(b"changed");
    out.unsigned(value.changed.as_nanos());
    out.key(b"acts");
    out.array_start();
    for item_14 in &value.acts {
        super::encode_offered(item_14, out, budget);
    }
    out.array_end();
}

fn encode_task_result_member(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    out.key(b"result");
    match &value.result {
        Some(item_15) => {
            super::encode_task_result(item_15, out, budget);
        }
        None => out.null(),
    }
}

fn encode_task_from_delegates(value: &jig_client::contract::Task, out: &mut Encoder, budget: &mut super::Cut) {
    out.key(b"delegates");
    out.array_start();
    for item_16 in &value.delegates {
        super::encode_live_delegate(item_16, out, budget);
    }
    out.array_end();
    out.key(b"context");
    match &value.context {
        Some(item_17) => {
            encode_task_context(item_17, out, budget);
        }
        None => out.null(),
    }
    out.key(b"bytes_left");
    out.unsigned(budget.left);
}

pub(super) fn encode_task_context(
    value: &jig_client::contract::TaskContext,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"escalation");
    match &value.escalation {
        Some(item_1) => {
            super::encode_escalation(item_1, out, budget);
        }
        None => out.null(),
    }
    out.key(b"parts");
    out.array_start();
    for item_2 in &value.parts {
        super::encode_part(item_2, out, budget);
    }
    out.array_end();
    out.key(b"closing");
    match &value.closing {
        Some(item_3) => {
            super::encode_closing(item_3, out, budget);
        }
        None => out.null(),
    }
    out.key(b"spend");
    out.array_start();
    for item_4 in &value.spend {
        super::encode_spend_line(item_4, out, budget);
    }
    out.array_end();
    out.key(b"runtime");
    super::encode_runtime(&value.runtime, out, budget);
    out.key(b"impact");
    super::encode_impact(&value.impact, out, budget);
    out.key(b"writer_slots");
    out.array_start();
    for item_5 in &value.writer_slots {
        super::encode_writer_slot(item_5, out, budget);
    }
    out.array_end();
    out.object_end();
}
