//! Classification uses the tools' single argument table.
use temper_client_tools::{
    jig::{
        argv::Usage,
        call::{Call, Parsed, Source, Value, Values},
        rows,
        table::Keyed,
    },
    table::{self, TOOLS},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Act,
    Read,
    Wait,
    SignIn,
    SignOut,
    Pending,
    Help,
}
impl Class {
    #[must_use]
    pub const fn locks(self) -> bool {
        matches!(self, Self::Act | Self::SignIn | Self::SignOut)
    }
}
/// Parse the words after DIRECTORY through the tools' own table.
/// # Errors
/// Returns the table's usage error unchanged.
pub fn parse(words: &[&[u8]]) -> Result<Parsed, Usage> {
    table::parse(words)
}
/// The parent of the FILE this invocation reads, opened separately from DIRECTORY.
#[must_use]
pub fn file_root(parsed: &Parsed) -> Option<Box<[u8]>> {
    let path = match parsed {
        Parsed::File { file: Source::Path(path), .. } => path.as_ref(),
        Parsed::File { file: Source::Stdin, .. } => return None,
        Parsed::Call { call, .. } => {
            let name = TOOLS[usize::from(call.tool)].name;
            if class(call) == Class::Help || name == b"amend" || name == b"change-settings" {
                return None;
            }
            call.values.iter().find_map(|values| {
                if let Some(Values::One(Value::File(path))) = values { Some(path.as_ref()) } else { None }
            })?
        }
    };
    if path == b"-" {
        return None;
    }
    Some(
        match path.iter().rposition(|byte| *byte == b'/') {
            Some(0) => b"/".as_slice(),
            Some(at) => &path[..at],
            None => b".",
        }
        .into(),
    )
}
#[must_use]
pub fn class(call: &Call) -> Class {
    let tool = TOOLS.get(usize::from(call.tool)).expect("parsed tool");
    if tool.keyed != Keyed::Never || tool.name == rows::EXPORT.name {
        Class::Act
    } else if tool.name == rows::SIGN_IN.name {
        Class::SignIn
    } else if tool.name == rows::SIGN_OUT.name {
        Class::SignOut
    } else if tool.name == rows::PENDING.name {
        Class::Pending
    } else if tool.name == rows::HELP.name {
        Class::Help
    } else if tool.name == rows::WAIT.name {
        Class::Wait
    } else {
        Class::Read
    }
}

/// Key files have a separate purpose and never feed the ordinary text input.
pub(crate) fn key_file(call: &Call) -> Option<&[u8]> {
    let name = TOOLS.get(usize::from(call.tool))?.name;
    let index = if name == rows::IMPORT.name {
        0
    } else if name == b"forge-connect" && call.form == 0 {
        3
    } else if name == b"forge-connect" && call.form == 3 {
        2
    } else if name == rows::ADD_KEY.name || name == rows::REPLACE_KEY.name {
        1
    } else {
        return None;
    };
    match call.values.get(index)? {
        Some(Values::One(Value::File(path))) => Some(path),
        _ => None,
    }
}

/// Amendment text files each get their own parent, opened before the loop.
#[must_use]
pub fn text_files(parsed: &Parsed) -> Vec<(u8, Box<[u8]>)> {
    let mut files = Vec::new();
    let Parsed::Call { call, .. } = parsed else { return files };
    let name = TOOLS[usize::from(call.tool)].name;
    if name != b"amend" && name != b"change-settings" {
        return files;
    }
    for (index, slot) in call.values.iter().enumerate() {
        if let Some(Values::One(Value::File(path))) = slot {
            let parent = match path.iter().rposition(|byte| *byte == b'/') {
                Some(0) => b"/".as_slice(),
                Some(at) => &path[..at],
                None => b".",
            };
            files.push((u8::try_from(index).expect("argument index"), parent.into()));
        }
    }
    files
}

/// A forge credential has six bytes of carried-bundle framing within jig's 4096-byte bound.
pub(crate) fn key_bytes(call: &Call) -> u32 {
    if call.tool == temper_client_tools::jig::rows::IMPORT_AT {
        temper_client_tools::limits::MOVED_BYTES
    } else if TOOLS[usize::from(call.tool)].name == b"forge-connect" {
        4090
    } else {
        temper_client_tools::limits::KEY_BYTES
    }
}
