//! A placed hold's supplied identity, ownership and lift permission.
use super::{Cut, Text};
use jig_client::contract as c;
use skein_json::writer::Encoder;

pub(super) fn text(task: &c::Task, me: Option<&c::Me>, out: &mut Text) {
    let Some(hold) = task.hold.as_ref().and_then(c::TaskHold::placed) else { return };
    out.put(b"\nhold: ");
    out.number(hold.number);
    out.put(b" \xc2\xb7 ");
    out.put(mode(hold.mode));
    out.put(b" \xc2\xb7 ");
    match hold.scope {
        c::HoldScope::Subtree { task } => {
            out.put(b"task ");
            out.number(task);
            out.put(b" and below");
        }
        c::HoldScope::Project => {
            if let Some(me) = me {
                for project in &me.projects {
                    if project.project == task.project {
                        out.put(b"project ");
                        break;
                    }
                }
            }
            super::project(task.project, me, out);
        }
    }
    out.put(b" \xc2\xb7 placed by ");
    owner(hold.owner, me, out);
    out.put(b" at ");
    super::clock::deadline(hold.placed_at, out);
    out.put(b" \xc2\xb7 revision ");
    out.number(hold.revision.value);
    if hold.lifted {
        out.put(b" \xc2\xb7 lifted");
    }
    out.put(b" \xc2\xb7 ");
    if hold.may_lift {
        out.put(b"you may lift it");
    } else {
        out.put(b"only ");
        owner(hold.owner, me, out);
        out.put(match hold.owner {
            c::HoldOwner::Party { .. } => b", or a role above theirs, may release it",
            c::HoldOwner::Task { .. } => b" or a task above it may release it",
        });
    }
    if !hold.reason.is_empty() {
        out.put(b"\nhold reason: ");
        out.words(&hold.reason);
    }
}
fn owner(owner: c::HoldOwner, me: Option<&c::Me>, out: &mut Text) {
    match owner {
        c::HoldOwner::Party { person } => {
            if let Some(me) = me
                && me.party.number == person
            {
                out.put(b"you");
            } else {
                out.put(b"person ");
                out.number(person);
            }
        }
        c::HoldOwner::Task { task } => {
            out.put(b"task ");
            out.number(task);
        }
    }
}
fn mode(mode: c::HoldMode) -> &'static [u8] {
    match mode {
        c::HoldMode::Stop => b"stop",
        c::HoldMode::Drain => b"drain",
    }
}
pub(super) fn encode(hold: &c::PlacedHold, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    out.key(b"hold");
    out.unsigned(hold.number);
    out.key(b"owner");
    encode_owner(hold.owner, out);
    out.key(b"mode");
    out.string(mode(hold.mode));
    out.key(b"scope");
    match hold.scope {
        c::HoldScope::Subtree { task } => {
            out.object_start();
            out.key(b"task");
            out.unsigned(task);
            out.object_end();
        }
        c::HoldScope::Project => out.string(b"project"),
    }
    out.key(b"reason");
    budget.words(&hold.reason, out);
    out.key(b"placed_at");
    out.unsigned(hold.placed_at.as_nanos());
    out.key(b"revision");
    out.unsigned(hold.revision.value);
    out.key(b"lifted");
    out.boolean(hold.lifted);
    out.key(b"may_lift");
    out.boolean(hold.may_lift);
    out.object_end();
}
pub(super) fn encode_owner(owner: c::HoldOwner, out: &mut Encoder) {
    out.object_start();
    match owner {
        c::HoldOwner::Party { person } => {
            out.key(b"person");
            out.unsigned(person);
        }
        c::HoldOwner::Task { task } => {
            out.key(b"task");
            out.unsigned(task);
        }
    }
    out.object_end();
}
