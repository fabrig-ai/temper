//! Project periods and pools, directly from the supplied owner ledger.
use super::{Text, projects_json};
use crate::jig::{
    call::{Call, number},
    rows,
};
use jig_client::contract as c;

pub(super) fn text(
    project: &c::OwnerProject,
    call: &Call,
    limits: &crate::jig::Limits,
    vocabulary: &dyn crate::jig::render::Parts,
    out: &mut Text,
) {
    if call.tool == rows::PROJECT_FUND_AT {
        funded(project, call, out);
    } else {
        out.put(b"project ");
        out.number(u64::from(project.number));
        out.put(b", ");
        out.words(project.name.bytes());
        out.put(b"\nhomes:");
        for resource in &project.resources {
            if resource.role == c::OwnerProjectResourceRole::Owned {
                out.put(b" ");
                vocabulary.owner_part(&resource.part, out);
            }
        }
        out.put(b"\nspend per period: ");
        out.number(project.policy.period_spend);
        out.put(b"\n");
        roles(project, out);
        period(project.current_period.as_ref(), false, out);
        for pool in &project.pools {
            pool_text(project, pool, false, out);
        }
    }
    if project.cut {
        if call.tool == rows::PROJECT_FUND_AT {
            out.put(b"now: ");
        }
        out.put(b"project: cut\n");
    }
    if call.tool == rows::PROJECT_SHOW_AT && project.policy.cut {
        out.put(b"policy: cut\n");
    }
    if project.configured_pools_cut {
        if call.tool == rows::PROJECT_FUND_AT {
            out.put(b"now: ");
        }
        out.put(b"configured pools: cut\n");
    }
    if call.tool == rows::PROJECT_SHOW_AT
        && let Some(digest) = projects_json::digest(project, call, limits)
    {
        out.put(b"digest: ");
        out.put(&digest.hex());
        out.put(b"\n");
    }
}
fn roles(project: &c::OwnerProject, out: &mut Text) {
    for role in &project.policy.roles {
        out.put(b"role ");
        out.number(u64::from(role.number));
        if role.number < 4 {
            out.put(b", ");
            super::role(held(role.number), out);
        }
        out.put(b": spend per period ");
        out.number(role.period_spend);
        out.put(b"; parties:");
        for member in &project.members {
            if member_role(member.role) == role.number {
                out.put(b" ");
                party(member.party, project, out);
            }
        }
        out.put(b"\n");
    }
}
fn member_role(role: c::HeldRole) -> u32 {
    match role {
        c::HeldRole::Owner => 0,
        c::HeldRole::Maintainer => 1,
        c::HeldRole::Member => 2,
        c::HeldRole::Observer => 3,
        c::HeldRole::Policy { role } => role,
    }
}
fn held(role: u32) -> c::HeldRole {
    match role {
        0 => c::HeldRole::Owner,
        1 => c::HeldRole::Maintainer,
        2 => c::HeldRole::Member,
        3 => c::HeldRole::Observer,
        role => c::HeldRole::Policy { role },
    }
}
pub(super) fn pool_role(project: &c::OwnerProject, pool: &c::PoolLedger, role: u64) -> bool {
    let c::Funder::Pool { person: party, .. } = pool.source else { return false };
    for member in &project.members {
        if member.party == party && u64::from(member_role(member.role)) == role {
            return true;
        }
    }
    false
}
fn funded(project: &c::OwnerProject, call: &Call, out: &mut Text) {
    let role = number(call, 2);
    match role {
        Some(role) => {
            out.put(b"role ");
            out.number(role);
        }
        None => {
            out.put(b"project ");
            out.number(u64::from(project.number));
        }
    }
    out.put(b": spend per period ");
    out.number(number(call, 1).expect("fund amount"));
    out.put(b"\nnow: ");
    period(project.current_period.as_ref(), true, out);
    if let Some(role) = role {
        for pool in &project.pools {
            if pool_role(project, pool, role) {
                out.put(b"now: ");
                pool_text(project, pool, true, out);
            }
        }
    }
}
fn period(period: Option<&c::PoolLedger>, brief: bool, out: &mut Text) {
    let Some(period) = period else {
        out.put(b"period: none open\n");
        return;
    };
    out.put(b"period ");
    if let c::Funder::Period { period, .. } = period.source {
        out.number(period);
    }
    if !brief {
        out.put(b", from ");
        match period.began {
            Some(wall) => super::clock::minute(wall, out),
            None => out.put(b"unknown"),
        }
        match period.next {
            Some(wall) => {
                out.put(b", next start ");
                super::clock::minute(wall, out);
            }
            None => out.put(b", next start: none"),
        }
    }
    out.put(b": ");
    numbers(period, brief, out);
}
fn pool_text(project: &c::OwnerProject, pool: &c::PoolLedger, brief: bool, out: &mut Text) {
    out.put(b"pool, ");
    if let c::Funder::Pool { person: number, .. } = pool.source {
        party(number, project, out);
    }
    out.put(b": ");
    numbers(pool, brief, out);
}
fn party(number: u64, project: &c::OwnerProject, out: &mut Text) {
    out.put(b"party ");
    out.number(number);
    for member in &project.members {
        if member.party == number {
            out.put(b" ");
            out.words(member.login.bytes());
            break;
        }
    }
}
fn numbers(ledger: &c::PoolLedger, brief: bool, out: &mut Text) {
    out.put(b"budget ");
    out.number(ledger.numbers.budget);
    if !brief {
        out.put(b", reserved ");
        out.number(ledger.numbers.reserved);
        out.put(b", spent ");
        out.number(ledger.numbers.spent.checked_add(ledger.numbers.spent_below).expect("ledger spend fits budget"));
    }
    out.put(b", left ");
    out.number(ledger.left);
    out.put(b"\n");
}
pub(super) fn encode(
    project: &c::OwnerProject,
    call: &Call,
    limits: &crate::jig::Limits,
    out: &mut skein_json::writer::Encoder,
    budget: &mut super::Cut,
) {
    if call.tool == rows::PROJECT_SHOW_AT {
        budget.digest = projects_json::digest(project, call, limits);
    }
    projects_json::encode(project, call, out, budget);
}
