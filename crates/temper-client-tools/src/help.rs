//! Help bytes for the host to write, bounded by the tools' text window.
use crate::table::Group;
use crate::{jig, limits, table};
use skein_lib::{Overflow, Writer};
/// The reference page `ctl-GROUP`, whole, as docs/help/reference holds it.
pub fn reference(group: &Group, out: &mut Writer) -> Result<(), Overflow> {
    out.put(b"# temper ctl ")?;
    out.put(group.name)?;
    out.put(b"\n\nKind: reference\n\n")?;
    out.put(group.line)?;
    out.put(b".\n")?;
    for (index, row) in table::TOOLS.iter().enumerate() {
        if jig::table::name_words(row.name).0 != group.name {
            continue;
        }
        let mut earlier = false;
        for prior in table::TOOLS.get(..index).ok_or(Overflow)? {
            earlier |= prior.name == row.name;
        }
        if earlier {
            continue;
        }
        out.put(b"\n## ")?;
        out.put(row.name)?;
        out.put(b"\n\n```\n")?;
        for record in table::TOOLS {
            if record.name == row.name {
                jig::help::tool(record, &limits::tools(), out)?;
            }
        }
        out.put(b"```\n")?;
    }
    Ok(())
}
/// None: the ctl page and groups. Some: the selected table row's record.
pub fn text(tool: Option<u8>, out: &mut Writer) -> Result<(), Overflow> {
    let mut measured = Writer::new(usize::try_from(limits::TEXT).expect("text fits usize"));
    write(tool, &mut measured)?;
    let mut exact = Writer::new(measured.written());
    write(tool, &mut exact)?;
    out.put(&exact.finish())
}
fn write(tool: Option<u8>, out: &mut Writer) -> Result<(), Overflow> {
    match tool {
        None => {
            out.put(temper_docs::find(b"ctl").expect("compiled ctl page").text.as_bytes())?;
            for group in table::GROUPS {
                out.put(group.name)?;
                out.put(b"  ")?;
                out.put(group.line)?;
                out.put(b"\n")?;
            }
            Ok(())
        }
        Some(index) => {
            let row = table::TOOLS.get(usize::from(index)).ok_or(Overflow)?;
            for later in table::TOOLS {
                if later.name == row.name {
                    jig::help::tool(later, &limits::tools(), out)?;
                }
            }
            Ok(())
        }
    }
}

/// A help call selects the overview, a group's tools, or a whole tool record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    All,
    Group(u8),
    Tool(u8),
}
#[must_use]
pub fn selection(call: &jig::call::Call) -> Option<Selection> {
    use jig::call::{Value, value};
    let Some(Value::Text(group)) = value(call, 0) else {
        return if value(call, 1).is_none() { Some(Selection::All) } else { None };
    };
    let verb = match value(call, 1) {
        Some(Value::Text(verb)) => Some(verb.as_ref()),
        None => None,
        Some(_) => return None,
    };
    for (index, row) in table::TOOLS.iter().enumerate() {
        let (first, next) = jig::table::name_words(row.name);
        if first == group.as_ref() {
            if next == verb {
                return Some(Selection::Tool(u8::try_from(index).expect("tool index")));
            }
            if verb.is_none() {
                for (index, known) in table::GROUPS.iter().enumerate() {
                    if known.name == group.as_ref() {
                        return Some(Selection::Group(u8::try_from(index).expect("group index")));
                    }
                }
            }
        }
    }
    None
}
/// Render the selection without publishing a partial record on overflow.
pub fn selected(selected: Selection, out: &mut Writer) -> Result<(), Overflow> {
    match selected {
        Selection::All => text(None, out),
        Selection::Tool(index) => text(Some(index), out),
        Selection::Group(index) => {
            let mut measured = Writer::new(usize::try_from(limits::TEXT).expect("text fits"));
            group(index, &mut measured)?;
            let mut exact = Writer::new(measured.written());
            group(index, &mut exact)?;
            out.put(&exact.finish())
        }
    }
}
fn group(index: u8, out: &mut Writer) -> Result<(), Overflow> {
    let group = table::GROUPS.get(usize::from(index)).ok_or(Overflow)?;
    for row in table::TOOLS {
        if jig::table::name_words(row.name).0 == group.name {
            jig::help::usage(row, out)?;
            out.put(b"  ")?;
            out.put(row.line)?;
            out.put(b"\n")?;
        }
    }
    Ok(())
}
