//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_owner_authority_document(
    value: &jig_client::contract::OwnerAuthorityDocument,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"authority");
    super::encode_owner_authority(&value.authority, out, budget);
    out.key(b"cut");
    out.boolean(value.cut);
    out.object_end();
}

pub(super) fn encode_owner_authority_executor(
    value: jig_client::contract::OwnerAuthorityExecutor,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    let value = &value;
    match value {
        jig_client::contract::OwnerAuthorityExecutor::Charter(field_0) => {
            out.object_start();
            out.key(b"charter");
            out.unsigned(u64::from(*(field_0)));
            out.object_end();
        }
        jig_client::contract::OwnerAuthorityExecutor::Procedure(field_0) => {
            out.object_start();
            out.key(b"procedure");
            out.unsigned(u64::from(*(field_0)));
            out.object_end();
        }
        jig_client::contract::OwnerAuthorityExecutor::Role(field_0) => {
            out.object_start();
            out.key(b"role");
            out.unsigned(u64::from(*(field_0)));
            out.object_end();
        }
    }
}

pub(super) fn encode_owner_budget(
    value: &jig_client::contract::OwnerBudget,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"numbers");
    super::encode_owner_numbers(&value.numbers, out, budget);
    out.key(b"left");
    out.unsigned(value.left);
    out.key(b"funder");
    super::encode_funder(&value.funder, out, budget);
    out.key(b"deadline");
    match &value.deadline {
        Some(item_1) => {
            out.unsigned((item_1).as_nanos());
        }
        None => out.null(),
    }
    out.object_end();
}

pub(super) fn encode_owner_chip_details(
    value: &jig_client::contract::OwnerChipDetails,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    encode_owner_chip_details_from_project(value, out, budget);
    encode_owner_chip_details_from_resources_observed(value, out, budget);
    out.object_end();
}

fn encode_owner_chip_details_from_project(
    value: &jig_client::contract::OwnerChipDetails,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.key(b"project");
    out.unsigned(u64::from(value.project));
    out.key(b"executor");
    super::encode_owner_executor(&value.executor, out, budget);
    out.key(b"phase_since");
    out.unsigned(value.phase_since.as_nanos());
    out.key(b"run_since");
    match &value.run_since {
        Some(item_1) => {
            out.unsigned((item_1).as_nanos());
        }
        None => out.null(),
    }
    out.key(b"attempt");
    match &value.attempt {
        Some(item_2) => {
            out.unsigned(*(item_2));
        }
        None => out.null(),
    }
    out.key(b"retry");
    match &value.retry {
        Some(item_3) => {
            super::encode_retry_notice(item_3, out, budget);
        }
        None => out.null(),
    }
    out.key(b"wake");
    super::encode_owner_wake_policy(&value.wake, out, budget);
    out.key(b"person_wait");
    match &value.person_wait {
        Some(item_4) => {
            super::encode_owner_person_wait(item_4, out, budget);
        }
        None => out.null(),
    }
    out.key(b"hold");
    match &value.hold {
        Some(item_5) => {
            encode_owner_chip_hold(item_5, out, budget);
        }
        None => out.null(),
    }
    out.key(b"budget");
    encode_owner_budget(&value.budget, out, budget);
    out.key(b"activity");
    out.unsigned(value.activity);
    out.key(b"resources");
    out.array_start();
    for item_6 in &value.resources {
        super::encode_owner_resource_line(item_6, out, budget);
    }
    out.array_end();
    out.key(b"resources_cut");
    out.boolean(value.resources_cut);
}
fn encode_owner_chip_details_from_resources_observed(
    value: &jig_client::contract::OwnerChipDetails,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.key(b"resources_observed");
    out.boolean(value.resources_observed);
    out.key(b"procedure");
    match &value.procedure {
        Some(item_7) => {
            super::encode_owner_part(item_7, out, budget);
        }
        None => out.null(),
    }
    out.key(b"live_delegates");
    out.unsigned(u64::from(value.live_delegates));
    out.key(b"closing_connectors");
    match &value.closing_connectors {
        Some(item_8) => {
            out.array_start();
            for item_9 in item_8 {
                out.unsigned(u64::from(*(item_9)));
            }
            out.array_end();
        }
        None => out.null(),
    }
    out.key(b"ending");
    match &value.ending {
        Some(item_10) => {
            encode_owner_chip_ending(item_10, out, budget);
        }
        None => out.null(),
    }
    out.key(b"ended_at");
    match &value.ended_at {
        Some(item_11) => {
            out.unsigned((item_11).as_nanos());
        }
        None => out.null(),
    }
    out.key(b"observation");
    match &value.observation {
        Some(item_12) => {
            encode_owner_chip_observation(item_12, out, budget);
        }
        None => out.null(),
    }
}

pub(super) fn encode_owner_chip_ending(
    value: &jig_client::contract::OwnerChipEnding,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::OwnerChipEnding::Done(field_0) => {
            out.object_start();
            out.key(b"done");
            super::encode_result_value(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::OwnerChipEnding::Failed(field_0) => {
            out.object_start();
            out.key(b"failed");
            super::encode_document_text(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::OwnerChipEnding::Cancelled { reason, result, by, at } => {
            out.object_start();
            out.key(b"cancelled");
            out.object_start();
            out.key(b"reason");
            super::encode_document_text(reason, out, budget);
            out.key(b"result");
            match result {
                Some(item_1) => {
                    super::encode_result_value(item_1, out, budget);
                }
                None => out.null(),
            }
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
            out.key(b"at");
            out.unsigned((at).as_nanos());
            out.object_end();
            out.object_end();
        }
    }
}

pub(super) fn encode_owner_chip_hold(
    value: &jig_client::contract::OwnerChipHold,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    if let Some(placed) = &value.placed {
        out.key(b"placed");
        super::placed_hold::encode(placed, out, budget);
    }
    out.key(b"account");
    match &value.account {
        Some(item_1) => {
            super::encode_owner_part(item_1, out, budget);
        }
        None => out.null(),
    }
    out.key(b"source");
    match &value.source {
        Some(item_2) => {
            super::encode_owner_part(item_2, out, budget);
        }
        None => out.null(),
    }
    out.key(b"reason");
    super::encode_hold_category(&value.reason, out, budget);
    out.key(b"revision");
    out.unsigned(value.revision);
    out.key(b"before");
    super::encode_owner_prior_phase(&value.before, out, budget);
    out.key(b"escalation");
    match &value.escalation {
        Some(item_3) => {
            super::encode_owner_hold_escalation(item_3, out, budget);
        }
        None => out.null(),
    }
    out.key(b"tries");
    super::encode_owner_failure_tries(&value.tries, out, budget);
    out.key(b"hold_wait_since");
    match &value.hold_wait_since {
        Some(item_4) => {
            out.unsigned((item_4).as_nanos());
        }
        None => out.null(),
    }
    out.object_end();
}

pub(super) fn encode_owner_chip_observation(
    value: &jig_client::contract::OwnerChipObservation,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"channel");
    out.unsigned(value.channel.raw());
    out.key(b"work");
    super::encode_owner_chip_work(&value.work, out, budget);
    out.key(b"since");
    out.unsigned(value.since.as_nanos());
    out.key(b"reported_at");
    out.unsigned(value.reported_at.as_nanos());
    out.key(b"started");
    match &value.started {
        Some(item_1) => {
            out.unsigned((item_1).as_nanos());
        }
        None => out.null(),
    }
    out.object_end();
}
