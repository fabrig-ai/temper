//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_task_contract_document(
    value: &jig_client::contract::TaskContractDocument,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::TaskContractDocument::Approval { state, part } => {
            out.object_start();
            out.key(b"approval");
            out.object_start();
            out.key(b"state");
            super::hex(state, out);
            out.key(b"part");
            super::encode_owner_part(part, out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::TaskContractDocument::Report { words } => {
            out.object_start();
            out.key(b"report");
            out.object_start();
            out.key(b"words");
            out.unsigned(u64::from(*(words)));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::TaskContractDocument::Verdict { choices, cut } => {
            out.object_start();
            out.key(b"verdict");
            out.object_start();
            out.key(b"choices");
            out.array_start();
            for item_1 in choices {
                super::encode_verdict_choice(item_1, out, budget);
            }
            out.array_end();
            out.key(b"cut");
            out.boolean(*(cut));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::TaskContractDocument::Change { connector, kind, words } => {
            out.object_start();
            out.key(b"change");
            out.object_start();
            out.key(b"connector");
            out.unsigned(u64::from(*(connector)));
            out.key(b"kind");
            out.unsigned(u64::from(*(kind)));
            out.key(b"words");
            out.unsigned(u64::from(*(words)));
            out.object_end();
            out.object_end();
        }
    }
}

pub(super) fn encode_task_flags(value: jig_client::contract::TaskFlags, out: &mut Encoder, budget: &mut super::Cut) {
    let value = &value;
    out.object_start();
    out.key(b"conversation");
    out.boolean(value.conversation);
    out.key(b"listed");
    out.boolean(value.listed);
    out.key(b"retention");
    super::encode_retention(value.retention, out, budget);
    out.object_end();
}

pub(super) fn encode_task_hold(value: &jig_client::contract::TaskHold, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    if let Some(placed) = value.placed() {
        out.key(b"placed");
        super::placed_hold::encode(placed, out, budget);
    }
    match value {
        jig_client::contract::TaskHold::Document(field_0) => {
            out.key(b"document");
            super::encode_hold(field_0, out, budget);
        }
        jig_client::contract::TaskHold::Owner(field_0) => {
            out.key(b"owner");
            super::encode_owner_chip_hold(field_0, out, budget);
        }
    }
    out.object_end();
}

pub(super) fn encode_task_resource(
    value: &jig_client::contract::TaskResource,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::TaskResource::Part(field_0) => {
            out.object_start();
            out.key(b"part");
            super::encode_part(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::TaskResource::Owner(field_0) => {
            out.object_start();
            out.key(b"owner");
            super::encode_owner_resource_line(field_0, out, budget);
            out.object_end();
        }
    }
}

pub(super) fn encode_task_result(value: &jig_client::contract::TaskResult, out: &mut Encoder, budget: &mut super::Cut) {
    match value {
        jig_client::contract::TaskResult::Document(field_0) => {
            out.object_start();
            out.key(b"document");
            super::encode_result(field_0, out, budget);
            out.object_end();
        }
        jig_client::contract::TaskResult::Ending(field_0) => {
            out.object_start();
            out.key(b"ending");
            super::encode_owner_chip_ending(field_0, out, budget);
            out.object_end();
        }
    }
}

pub(super) fn encode_template(value: &jig_client::contract::Template, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    out.key(b"number");
    out.unsigned(u64::from(value.number));
    out.key(b"name");
    super::encode_document_text(&value.name, out, budget);
    out.key(b"executor");
    super::encode_template_executor(value.executor, out, budget);
    out.key(b"contract");
    encode_task_contract_document(&value.contract, out, budget);
    out.key(b"shallower");
    out.unsigned(u64::from(value.shallower));
    out.key(b"budget");
    encode_template_budget(&value.budget, out, budget);
    out.key(b"deadline");
    super::encode_template_deadline(&value.deadline, out, budget);
    out.key(b"tracked");
    match &value.tracked {
        Some(item_1) => {
            out.unsigned(u64::from(*(item_1)));
        }
        None => out.null(),
    }
    out.key(b"flags");
    encode_task_flags(value.flags, out, budget);
    out.key(b"label");
    out.unsigned(u64::from(value.label));
    out.key(b"grace");
    match &value.grace {
        Some(item_2) => {
            out.unsigned((item_2).as_nanos());
        }
        None => out.null(),
    }
    out.object_end();
}

pub(super) fn encode_template_budget(
    value: &jig_client::contract::TemplateBudget,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::TemplateBudget::Amount(field_0) => {
            out.object_start();
            out.key(b"amount");
            out.unsigned(*(field_0));
            out.object_end();
        }
        jig_client::contract::TemplateBudget::AsAllowed { cap } => {
            out.object_start();
            out.key(b"as_allowed");
            out.object_start();
            out.key(b"cap");
            match cap {
                Some(item_1) => {
                    out.unsigned(*(item_1));
                }
                None => out.null(),
            }
            out.object_end();
            out.object_end();
        }
        jig_client::contract::TemplateBudget::Given => out.string(b"given"),
    }
}
