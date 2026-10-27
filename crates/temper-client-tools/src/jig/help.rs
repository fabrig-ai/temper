//! Usage and records derived from the tools' table.
use super::table::{Argument, Bound, Keyed, Kind, Limits, Occurs, Place, Tool, bound};
use skein_lib::{Decimal, Overflow, Writer};
/// Write every form in the table's published argument order.
pub fn usage(tool: &Tool, out: &mut Writer) -> Result<(), Overflow> {
    for form in tool.forms {
        form_usage(tool, form, out)?;
    }
    Ok(())
}
fn form_usage(tool: &Tool, form: &[Argument], out: &mut Writer) -> Result<(), Overflow> {
    if tool.name == b"task amend" {
        return out.put(b"task amend TASK [--rev REV] [--priority PRIORITY | --untracked] [--deadline TIME | --no-deadline] [--instructions FILE] [--spec FILE] [--dependency TASK]... [--no-dependencies] [--model MODEL [--effort EFFORT]] REASON [--key KEY]\n");
    }
    if tool.name == super::rows::HELP.name {
        return out.put(b"help [GROUP [VERB]]\n");
    }
    out.put(tool.name)?;
    for argument in form {
        argument_usage(argument, out)?;
    }
    out.put(match tool.keyed {
        Keyed::Never => b"",
        Keyed::Optional => b" [--key KEY]",
        Keyed::Required => b" --key KEY",
    })?;
    out.put(b"\n")
}

fn argument_usage(argument: &Argument, out: &mut Writer) -> Result<(), Overflow> {
    out.put(b" ")?;
    let optional = argument.occurs != Occurs::Required;
    if optional {
        out.put(b"[")?;
    }
    match argument.place {
        Place::Positional => shown(argument, out)?,
        Place::Flag | Place::Switch => {
            out.put(b"--")?;
            out.put(argument.name)?;
            if argument.place == Place::Flag {
                out.put(b" ")?;
                shown(argument, out)?;
            }
        }
    }
    if optional {
        out.put(b"]")?;
    }
    if let Occurs::Repeated { .. } = argument.occurs {
        out.put(b"...")?;
    }
    Ok(())
}
fn shown(argument: &Argument, out: &mut Writer) -> Result<(), Overflow> {
    out.put(argument.shown)
}
/// The table's complete list, following the agents' guide.
pub fn tools(table: &[&Tool], out: &mut Writer) -> Result<(), Overflow> {
    for row in table {
        usage(row, out)?;
        out.put(b"  ")?;
        out.put(row.line)?;
        out.put(b"\n")?;
    }
    Ok(())
}
/// One tool's complete record, with the host's argument bounds.
pub fn tool(tool: &Tool, limits: &Limits, out: &mut Writer) -> Result<(), Overflow> {
    for form in tool.forms {
        form_usage(tool, form, out)?;
        for argument in *form {
            argument_record(argument, limits, out)?;
        }
    }
    out.put(tool.line)?;
    out.put(b"\n")?;
    if !tool.prints.is_empty() {
        out.put(b"Prints: ")?;
        out.put(tool.prints)?;
        out.put(b"\n")?;
    }
    out.put(b"--json: the record ")?;
    out.put(tool.result)?;
    out.put(b".\n")?;
    out.put(match tool.keyed {
        Keyed::Never => b"Key: none.\n",
        Keyed::Optional => b"Key: --key KEY, optional.\n",
        Keyed::Required => b"Key: --key KEY, required.\n",
    })?;
    out.put(tool.notes)?;
    if tool.name == b"wait" {
        wait(out)?;
    }
    Ok(())
}
fn argument_record(argument: &Argument, limits: &Limits, out: &mut Writer) -> Result<(), Overflow> {
    out.put(b"  ")?;
    match argument.place {
        Place::Positional => {
            out.put(argument.shown)?;
            out.put(b" (")?;
            out.put(argument.name)?;
            out.put(b")")?;
        }
        Place::Flag | Place::Switch => {
            out.put(b"--")?;
            out.put(argument.name)?;
            if argument.place == Place::Flag {
                out.put(b" ")?;
                out.put(argument.shown)?;
            }
        }
    }
    out.put(b": ")?;
    if argument.place == Place::Switch {
        out.put(b"a switch")?;
    } else {
        argument_type(argument.kind, out)?;
        argument_bound(argument, limits, out)?;
        if argument.kind == Kind::Text {
            out.put(b", or - to read stdin")?;
        }
        out.put(b"; ")?;
        match argument.occurs {
            Occurs::Required => out.put(b"required")?,
            Occurs::Optional => out.put(b"optional")?,
            Occurs::Repeated { most } => {
                out.put(b"repeated, at most ")?;
                out.put(Decimal::of(u64::from(most)).as_bytes())?;
            }
        }
    }
    out.put(b". ")?;
    out.put(argument.line)?;
    out.put(b"\n")
}
fn argument_type(kind: Kind, out: &mut Writer) -> Result<(), Overflow> {
    out.put(match kind {
        Kind::Number | Kind::NumberRange { .. } => b"a number",
        Kind::Count => b"a count",
        Kind::NamedNumber { words, prefixes, .. } => return choices(words, prefixes, out),
        Kind::Word { set } => return choices(set, &[], out),
        Kind::Text | Kind::Literal => b"text",
        Kind::File => b"a file",
        Kind::Duration => b"a duration: seconds, or a number with s, m or h",
        Kind::Proposal => b"a proposal, tN/K or pN/K",
        Kind::Digest => b"a digest, as a read prints it",
        Kind::Condition => b"a condition",
        Kind::Subject => b"a subject",
        Kind::Project { .. } => b"a project's name or number",
        Kind::Turn => b"an attempt and a turn, A.T",
        Kind::Cursor => b"a cursor, as a read prints it",
        Kind::Group => b"one of questions, proposals, escalations, person-tasks, results or replies",
        Kind::Order => b"one of activity or priority",
        Kind::Tool => b"a tool's name",
    })
}
fn choices(words: &[&[u8]], prefixes: &[&[u8]], out: &mut Writer) -> Result<(), Overflow> {
    out.put(b"one of ")?;
    let count = words.len().checked_add(prefixes.len()).ok_or(Overflow)?;
    for (index, word) in words.iter().chain(prefixes).enumerate() {
        if index != 0 {
            out.put(if index.checked_add(1) == Some(count) { b" or " } else { b", " })?;
        }
        out.put(word)?;
        if index >= words.len() {
            out.put(b"N")?;
        }
    }
    Ok(())
}
fn argument_bound(argument: &Argument, limits: &Limits, out: &mut Writer) -> Result<(), Overflow> {
    match bound(argument, limits) {
        Bound::None => {}
        Bound::Numbers { least, most } => {
            if let Kind::NamedNumber { .. } = argument.kind {
                out.put(b" with N from ")?;
            } else {
                out.put(b" from ")?;
            }
            out.put(Decimal::of(least).as_bytes())?;
            out.put(b" to ")?;
            out.put(Decimal::of(most).as_bytes())?;
        }
        Bound::Bytes { most } => {
            out.put(b", at most ")?;
            out.put(Decimal::of(most).as_bytes())?;
            out.put(b" bytes")?;
        }
        Bound::Seconds { most } => {
            out.put(b", at most ")?;
            out.put(Decimal::of(most).as_bytes())?;
            out.put(b" seconds")?;
        }
    }
    Ok(())
}

fn wait(out: &mut Writer) -> Result<(), Overflow> {
    out.put(b"Live phases: ")?;
    let mut separator = b"".as_slice();
    for phase in super::subject::PHASES {
        if super::subject::live(*phase) {
            out.put(separator)?;
            out.put(super::subject::phase_word(*phase))?;
            separator = b"|";
        }
    }
    out.put(b"\n")
}
