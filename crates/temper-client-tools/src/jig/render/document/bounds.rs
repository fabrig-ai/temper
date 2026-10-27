//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_admission_limit(
    value: jig_client::contract::AdmissionLimit,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    let value = &value;
    match value {
        jig_client::contract::AdmissionLimit::PlacedHolds => out.string(b"placed_holds"),
        jig_client::contract::AdmissionLimit::ProjectPlacedHolds => out.string(b"project_placed_holds"),
        jig_client::contract::AdmissionLimit::PartyPlacedHolds => out.string(b"party_placed_holds"),
        jig_client::contract::AdmissionLimit::Exemptions => out.string(b"exemptions"),
        jig_client::contract::AdmissionLimit::SettingsProposals => out.string(b"settings_proposals"),
        jig_client::contract::AdmissionLimit::Accounts => out.string(b"accounts"),
        jig_client::contract::AdmissionLimit::HolderAccounts => out.string(b"holder_accounts"),
        jig_client::contract::AdmissionLimit::Sources => out.string(b"sources"),
        jig_client::contract::AdmissionLimit::SignIns => out.string(b"sign_ins"),
        jig_client::contract::AdmissionLimit::Goals => out.string(b"goals"),
        jig_client::contract::AdmissionLimit::Operations => out.string(b"operations"),
        jig_client::contract::AdmissionLimit::Grace => out.string(b"grace"),
        jig_client::contract::AdmissionLimit::Words => out.string(b"words"),
        jig_client::contract::AdmissionLimit::Identity => out.string(b"identity"),
        jig_client::contract::AdmissionLimit::Roles => out.string(b"roles"),
        jig_client::contract::AdmissionLimit::Arithmetic => out.string(b"arithmetic"),
        jig_client::contract::AdmissionLimit::Duplicate => out.string(b"duplicate"),
        jig_client::contract::AdmissionLimit::Empty => out.string(b"empty"),
        jig_client::contract::AdmissionLimit::Batch => out.string(b"batch"),
        jig_client::contract::AdmissionLimit::ProjectLiveTasks => out.string(b"project_live_tasks"),
        jig_client::contract::AdmissionLimit::LiveTasks => out.string(b"live_tasks"),
        jig_client::contract::AdmissionLimit::Tree => out.string(b"tree"),
        jig_client::contract::AdmissionLimit::Depth => out.string(b"depth"),
        jig_client::contract::AdmissionLimit::Delegates => out.string(b"delegates"),
        jig_client::contract::AdmissionLimit::Subscription => out.string(b"subscription"),
        jig_client::contract::AdmissionLimit::Dependencies => out.string(b"dependencies"),
        jig_client::contract::AdmissionLimit::Cycle => out.string(b"cycle"),
        jig_client::contract::AdmissionLimit::Spec => out.string(b"spec"),
        jig_client::contract::AdmissionLimit::Contract => out.string(b"contract"),
        jig_client::contract::AdmissionLimit::Authority => out.string(b"authority"),
        jig_client::contract::AdmissionLimit::Inputs => out.string(b"inputs"),
        jig_client::contract::AdmissionLimit::Attempts => out.string(b"attempts"),
        jig_client::contract::AdmissionLimit::LiveDelegates => out.string(b"live_delegates"),
        jig_client::contract::AdmissionLimit::Restore => out.string(b"restore"),
        jig_client::contract::AdmissionLimit::Read => out.string(b"read"),
        jig_client::contract::AdmissionLimit::Turn => out.string(b"turn"),
        jig_client::contract::AdmissionLimit::HoldKind => out.string(b"hold_kind"),
        jig_client::contract::AdmissionLimit::HoldTaken => out.string(b"hold_taken"),
        jig_client::contract::AdmissionLimit::Holds => out.string(b"holds"),
        jig_client::contract::AdmissionLimit::InboxFull => out.string(b"inbox_full"),
        jig_client::contract::AdmissionLimit::Notes => out.string(b"notes"),
        jig_client::contract::AdmissionLimit::Revision => out.string(b"revision"),
        jig_client::contract::AdmissionLimit::Question => out.string(b"question"),
        jig_client::contract::AdmissionLimit::Priority => out.string(b"priority"),
        jig_client::contract::AdmissionLimit::Policy => out.string(b"policy"),
        jig_client::contract::AdmissionLimit::Resource => out.string(b"resource"),
        jig_client::contract::AdmissionLimit::Projects => out.string(b"projects"),
        jig_client::contract::AdmissionLimit::Service => out.string(b"service"),
    }
}

pub(super) fn encode_attempt_failure(
    value: &jig_client::contract::AttemptFailure,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"attempt");
    out.unsigned(value.attempt);
    out.key(b"class");
    super::encode_failure_class(value.class, out, budget);
    out.key(b"reason");
    budget.words(value.reason.bytes(), out);
    out.key(b"at");
    out.unsigned(value.at.as_nanos());
    out.object_end();
}

pub(super) fn encode_beyond(value: &jig_client::contract::Beyond, out: &mut Encoder, budget: &mut super::Cut) {
    match value {
        jig_client::contract::Beyond::Proposed { to } => {
            out.object_start();
            out.key(b"proposed");
            out.object_start();
            out.key(b"to");
            super::encode_recipient(to, out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Beyond::Refused(field_0) => {
            out.object_start();
            out.key(b"refused");
            super::encode_refusal(field_0, out, budget);
            out.object_end();
        }
    }
}

pub(super) fn encode_bounds(value: &jig_client::contract::Bounds, out: &mut Encoder, budget: &mut super::Cut) {
    match value {
        jig_client::contract::Bounds::Create(field_0) => {
            out.object_start();
            out.key(b"create");
            super::encode_create_bounds(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::Bounds::Amend { budget_left, grants, implications, authority, ceiling } => {
            out.object_start();
            out.key(b"amend");
            out.object_start();
            out.key(b"budget_left");
            out.unsigned(*(budget_left));
            out.key(b"grants");
            out.array_start();
            for item_1 in grants {
                super::encode_owner_grant(item_1, out, budget);
            }
            out.array_end();
            out.key(b"implications");
            out.array_start();
            for item_2 in implications {
                super::encode_owner_kind_implication(*(item_2), out, budget);
            }
            out.array_end();
            out.key(b"authority");
            super::encode_owner_authority_document(authority, out, budget);
            out.key(b"ceiling");
            super::encode_owner_authority_document(ceiling, out, budget);
            out.object_end();
            out.object_end();
        }
    }
}

pub(super) fn encode_budget(value: &jig_client::contract::Budget, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    out.key(b"total");
    out.unsigned(value.total);
    out.key(b"own");
    out.unsigned(value.own);
    out.key(b"ended");
    out.unsigned(value.ended);
    out.key(b"reserved");
    out.unsigned(value.reserved);
    out.key(b"left");
    out.unsigned(value.left);
    out.key(b"funder");
    super::encode_actor(&value.funder, out, budget);
    out.object_end();
}

pub(super) fn encode_call_identity(
    value: &jig_client::contract::CallIdentity,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"owner");
    out.unsigned(value.owner.raw());
    out.key(b"sequence");
    out.unsigned(u64::from(value.sequence));
    out.key(b"retry");
    out.unsigned(u64::from(value.retry));
    out.key(b"id");
    budget.words(value.id.bytes(), out);
    out.object_end();
}
