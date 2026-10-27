//! Integer JSON projected from the same typed project facts as the text.
use super::Cut;
use crate::jig::{
    call::{Call, number},
    rows,
};
use jig_client::contract as c;
use skein_json::writer::Encoder;

pub(super) fn encode(project: &c::OwnerProject, call: &Call, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    out.key(b"number");
    out.unsigned(u64::from(project.number));
    if call.tool == rows::PROJECT_SHOW_AT {
        facts(project, out, budget);
    }
    out.key(b"current_period");
    match &project.current_period {
        Some(period) => super::encode_pool_ledger(period, out, budget),
        None => out.null(),
    }
    out.key(b"pools");
    out.array_start();
    for pool in &project.pools {
        let selected = match number(call, 2) {
            Some(role) => super::projects::pool_role(project, pool, role),
            None => false,
        };
        if call.tool == rows::PROJECT_SHOW_AT || selected {
            super::encode_pool_ledger(pool, out, budget);
        }
    }
    out.array_end();
    out.key(b"cut");
    out.boolean(project.cut);
    out.key(b"configured_pools_cut");
    out.boolean(project.configured_pools_cut);
    super::subject_end(budget.digest.take(), out);
}
fn facts(project: &c::OwnerProject, out: &mut Encoder, budget: &mut Cut) {
    out.key(b"name");
    super::encode_document_text(&project.name, out, budget);
    out.key(b"resources");
    out.array_start();
    for resource in &project.resources {
        out.object_start();
        out.key(b"part");
        super::encode_owner_part(&resource.part, out, budget);
        out.key(b"role");
        out.string(match resource.role {
            c::OwnerProjectResourceRole::Owned => b"owned",
            c::OwnerProjectResourceRole::Participant => b"participant",
            c::OwnerProjectResourceRole::Unavailable => b"unavailable",
            c::OwnerProjectResourceRole::Context => b"context",
        });
        out.object_end();
    }
    out.array_end();
    out.key(b"policy");
    policy(&project.policy, out, budget);
    out.key(b"members");
    out.array_start();
    for member in &project.members {
        out.object_start();
        out.key(b"party");
        out.unsigned(member.party);
        out.key(b"kind");
        super::encode_party_kind(member.kind, out, budget);
        out.key(b"role");
        super::encode_held_role(member.role, out, budget);
        out.key(b"login");
        super::encode_document_text(&member.login, out, budget);
        out.key(b"name");
        super::encode_document_text(&member.name, out, budget);
        out.object_end();
    }
    out.array_end();
}
fn policy(policy: &c::OwnerProjectPolicy, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    out.key(b"period_spend");
    out.unsigned(policy.period_spend);
    out.key(b"roles");
    out.array_start();
    for role in &policy.roles {
        out.object_start();
        out.key(b"number");
        out.unsigned(u64::from(role.number));
        out.key(b"authority");
        super::encode_owner_authority_document(&role.authority, out, budget);
        out.key(b"period_spend");
        out.unsigned(role.period_spend);
        out.key(b"requests");
        out.unsigned(u64::from(role.requests));
        out.key(b"decides");
        out.unsigned(u64::from(role.decides));
        out.object_end();
    }
    out.array_end();
    out.key(b"cut");
    out.boolean(policy.cut);
    out.object_end();
}

pub(super) fn digest(
    project: &c::OwnerProject,
    call: &Call,
    limits: &crate::jig::Limits,
) -> Option<crate::jig::digest::Digest> {
    let bounds = skein_json::writer::Limits {
        depth: 32,
        length: u32::try_from(limits.result_bytes).expect("result bound fits"),
    };
    let mut budget = Cut {
        room: usize::try_from(limits.text).expect("text fits"),
        left: 0,
        turn: None,
        subject: true,
        digest: None,
    };
    let mut measure = Encoder::measure(&bounds);
    encode(project, call, &mut measure, &mut budget);
    let size = measure.measured().ok()?;
    let mut write = Encoder::write(size, &bounds);
    budget.room = usize::try_from(limits.text).expect("text fits");
    budget.left = 0;
    encode(project, call, &mut write, &mut budget);
    Some(crate::jig::digest::Digest::of(&write.finish()))
}
