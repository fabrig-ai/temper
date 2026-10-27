//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_lacking(value: &jig_client::contract::Lacking, out: &mut Encoder, budget: &mut super::Cut) {
    match value {
        jig_client::contract::Lacking::HoldOwner { hold, owner } => {
            out.object_start();
            out.key(b"hold_owner");
            out.object_start();
            out.key(b"hold");
            out.unsigned(*hold);
            out.key(b"owner");
            super::placed_hold::encode_owner(*owner, out);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Lacking::Approver => out.string(b"approver"),
        jig_client::contract::Lacking::Budget { needed, left } => {
            encode_lacking_budget(*needed, *left, out, budget);
        }
        jig_client::contract::Lacking::Grant(field_0) => {
            encode_lacking_grant(field_0, out, budget);
        }
        jig_client::contract::Lacking::GrantKind { kind } => {
            encode_lacking_grant_kind(*kind, out, budget);
        }
        jig_client::contract::Lacking::Funding => out.string(b"funding"),
        jig_client::contract::Lacking::Tool => out.string(b"tool"),
        jig_client::contract::Lacking::Scope => out.string(b"scope"),
        jig_client::contract::Lacking::Reference => out.string(b"reference"),
        jig_client::contract::Lacking::ResourceAccess => out.string(b"resource_access"),
        jig_client::contract::Lacking::ResourceUnavailable => out.string(b"resource_unavailable"),
        jig_client::contract::Lacking::Shape => out.string(b"shape"),
        jig_client::contract::Lacking::Again { required } => {
            encode_lacking_again(*required, out, budget);
        }
        jig_client::contract::Lacking::Account => out.string(b"account"),
        jig_client::contract::Lacking::Writer => out.string(b"writer"),
        jig_client::contract::Lacking::Models => out.string(b"models"),
        jig_client::contract::Lacking::Required { connector, kind } => {
            encode_lacking_required(*connector, *kind, out, budget);
        }
        jig_client::contract::Lacking::Failed { connector, kind } => {
            encode_lacking_failed(*connector, *kind, out, budget);
        }
        jig_client::contract::Lacking::Unguarded { connector, kind } => {
            encode_lacking_unguarded(*connector, *kind, out, budget);
        }
        jig_client::contract::Lacking::TaskKind { executor } => {
            encode_lacking_task_kind(executor.as_ref(), out, budget);
        }
        jig_client::contract::Lacking::Depth { needed, allowed } => {
            encode_lacking_depth(needed.as_ref(), *allowed, out, budget);
        }
        jig_client::contract::Lacking::Deadline { asked, latest } => {
            encode_lacking_deadline(asked.as_ref(), latest.as_ref(), out, budget);
        }
        jig_client::contract::Lacking::Tasks { needed, left } => {
            encode_lacking_tasks(needed.as_ref(), *left, out, budget);
        }
    }
}

fn encode_lacking_budget(needed: u64, left: u64, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"budget");
    out.object_start();
    out.key(b"needed");
    out.unsigned(needed);
    out.key(b"left");
    out.unsigned(left);
    out.object_end();
    out.object_end();
}

fn encode_lacking_grant(field_0: &jig_client::contract::Grant, out: &mut Encoder, budget: &mut super::Cut) {
    out.object_start();
    out.key(b"grant");
    super::encode_grant(field_0, out, budget);
    out.object_end();
}

fn encode_lacking_grant_kind(kind: u16, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"grant_kind");
    out.object_start();
    out.key(b"kind");
    out.unsigned(u64::from(kind));
    out.object_end();
    out.object_end();
}

fn encode_lacking_again(required: bool, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"again");
    out.object_start();
    out.key(b"required");
    out.boolean(required);
    out.object_end();
    out.object_end();
}

fn encode_lacking_required(connector: u16, kind: u16, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"required");
    out.object_start();
    out.key(b"connector");
    out.unsigned(u64::from(connector));
    out.key(b"kind");
    out.unsigned(u64::from(kind));
    out.object_end();
    out.object_end();
}

fn encode_lacking_failed(connector: u16, kind: u16, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"failed");
    out.object_start();
    out.key(b"connector");
    out.unsigned(u64::from(connector));
    out.key(b"kind");
    out.unsigned(u64::from(kind));
    out.object_end();
    out.object_end();
}

fn encode_lacking_unguarded(connector: u16, kind: u16, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"unguarded");
    out.object_start();
    out.key(b"connector");
    out.unsigned(u64::from(connector));
    out.key(b"kind");
    out.unsigned(u64::from(kind));
    out.object_end();
    out.object_end();
}

fn encode_lacking_task_kind(
    executor: Option<&jig_client::contract::Executor>,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"task_kind");
    out.object_start();
    out.key(b"executor");
    match executor {
        Some(item_1) => {
            super::encode_executor(item_1, out, budget);
        }
        None => out.null(),
    }
    out.object_end();
    out.object_end();
}

fn encode_lacking_depth(needed: Option<&u32>, allowed: u32, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"depth");
    out.object_start();
    out.key(b"needed");
    match needed {
        Some(item_2) => {
            out.unsigned(u64::from(*(item_2)));
        }
        None => out.null(),
    }
    out.key(b"allowed");
    out.unsigned(u64::from(allowed));
    out.object_end();
    out.object_end();
}

fn encode_lacking_deadline(
    asked: Option<&skein_lib::Wall>,
    latest: Option<&skein_lib::Wall>,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    out.object_start();
    out.key(b"deadline");
    out.object_start();
    out.key(b"asked");
    match asked {
        Some(item_3) => {
            out.unsigned((item_3).as_nanos());
        }
        None => out.null(),
    }
    out.key(b"latest");
    match latest {
        Some(item_4) => {
            out.unsigned((item_4).as_nanos());
        }
        None => out.null(),
    }
    out.object_end();
    out.object_end();
}

fn encode_lacking_tasks(needed: Option<&u32>, left: u32, out: &mut Encoder, _budget: &mut super::Cut) {
    out.object_start();
    out.key(b"tasks");
    out.object_start();
    out.key(b"needed");
    match needed {
        Some(item_5) => {
            out.unsigned(u64::from(*(item_5)));
        }
        None => out.null(),
    }
    out.key(b"left");
    out.unsigned(u64::from(left));
    out.object_end();
    out.object_end();
}
pub(super) fn encode_limit(value: &jig_client::contract::Limit, out: &mut Encoder, budget: &mut super::Cut) {
    match value {
        jig_client::contract::Limit::Admission { which } => {
            out.object_start();
            out.key(b"admission");
            out.object_start();
            out.key(b"which");
            super::encode_admission_limit(*(which), out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Limit::LiveTasks { project, limit } => {
            out.object_start();
            out.key(b"live_tasks");
            out.object_start();
            out.key(b"project");
            out.unsigned(u64::from(*(project)));
            out.key(b"limit");
            out.unsigned(u64::from(*(limit)));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Limit::Inbox { task, limit } => {
            out.object_start();
            out.key(b"inbox");
            out.object_start();
            out.key(b"task");
            out.unsigned(*(task));
            out.key(b"limit");
            out.unsigned(u64::from(*(limit)));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Limit::Words { limit } => {
            out.object_start();
            out.key(b"words");
            out.object_start();
            out.key(b"limit");
            out.unsigned(u64::from(*(limit)));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::Limit::Fields => out.string(b"fields"),
        jig_client::contract::Limit::Records => out.string(b"records"),
        jig_client::contract::Limit::Requests => out.string(b"requests"),
        jig_client::contract::Limit::Reads => out.string(b"reads"),
        jig_client::contract::Limit::Watches => out.string(b"watches"),
        jig_client::contract::Limit::Objects => out.string(b"objects"),
    }
}
