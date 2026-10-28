//! Policy ceiling facts, rendered as client/tools.md, 5.3 prescribes.
use super::Text;
use jig_client::contract::{Ceiling, CeilingDimension as Dimension, CeilingHolder as Holder};
use skein_json::writer::Encoder;

pub(super) fn text(ceiling: &Ceiling, project: u32, application: &[u8], out: &mut Text) {
    if ceiling.dimension == Dimension::PeriodSpend {
        match ceiling.holder {
            Holder::Rules => {
                out.put(b"past the rules' period spend");
                value(ceiling, out);
                out.put(b"; raise the rules first with ");
                out.put(application);
                out.put(b" ctl settings apply");
            }
            Holder::Project => {
                out.put(b"past the project's period spend");
                value(ceiling, out);
                out.put(b"; fund the project first: ");
                out.put(application);
                out.put(b" ctl project fund ");
                out.number(u64::from(project));
                out.put(b" AMOUNT");
            }
            Holder::Role { role } => {
                out.put(b"below role ");
                out.number(u64::from(role));
                out.put(b"'s period spend");
                value(ceiling, out);
                out.put(b"; lower the role first: ");
                out.put(application);
                out.put(b" ctl project fund ");
                out.number(u64::from(project));
                out.put(b" AMOUNT --role ");
                out.number(u64::from(role));
            }
        }
        return;
    }
    match ceiling.role {
        Some(role) => {
            out.put(b"role ");
            out.number(u64::from(role));
            out.put(b"'s authority is past ");
        }
        None => out.put(b"the project's authority is past "),
    }
    match ceiling.holder {
        Holder::Rules => out.put(b"the rules' "),
        Holder::Project => out.put(b"the project's "),
        Holder::Role { role } => {
            out.put(b"role ");
            out.number(u64::from(role));
            out.put(b"'s ");
        }
    }
    out.put(dimension(ceiling.dimension, false));
    value(ceiling, out);
    out.put(b"; raise the rules with ");
    out.put(application);
    out.put(b" ctl settings apply, or narrow the project's policy");
}
fn value(ceiling: &Ceiling, out: &mut Text) {
    if let Some(value) = ceiling.value {
        out.put(b", ");
        out.number(value);
    }
}
fn dimension(value: Dimension, json: bool) -> &'static [u8] {
    match value {
        Dimension::PeriodSpend => {
            if json {
                b"period_spend"
            } else {
                b"period spend"
            }
        }
        Dimension::Spend => b"spend",
        Dimension::Tools => b"tools",
        Dimension::Tasks => {
            if json {
                b"tasks"
            } else {
                b"delegation tasks"
            }
        }
        Dimension::Depth => {
            if json {
                b"depth"
            } else {
                b"delegation depth"
            }
        }
        Dimension::Executors => b"executors",
        Dimension::Grants => b"grants",
        Dimension::Deadline => b"deadline",
        Dimension::Notes => b"notes",
    }
}
pub(super) fn json(ceiling: &Ceiling, out: &mut Encoder) {
    out.object_start();
    out.key(b"ceiling");
    out.object_start();
    out.key(b"holder");
    match ceiling.holder {
        Holder::Rules => out.string(b"rules"),
        Holder::Project => out.string(b"project"),
        Holder::Role { role } => {
            out.object_start();
            out.key(b"role");
            out.object_start();
            out.key(b"role");
            out.unsigned(u64::from(role));
            out.object_end();
            out.object_end();
        }
    }
    out.key(b"role");
    match ceiling.role {
        Some(role) => out.unsigned(u64::from(role)),
        None => out.null(),
    }
    out.key(b"dimension");
    out.string(dimension(ceiling.dimension, true));
    out.key(b"value");
    match ceiling.value {
        Some(value) => out.unsigned(value),
        None => out.null(),
    }
    out.key(b"edit");
    match ceiling.edit {
        Some(edit) => out.unsigned(edit),
        None => out.null(),
    }
    out.object_end();
    out.object_end();
}
