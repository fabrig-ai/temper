//! A table row's typed arguments (tools contract, 5.1).
use super::{key::Name, subject::Condition};
use alloc::boxed::Box;
use jig_client::contract::{InboxGroup, ProposalKey, QueryOrder, TurnName};
/// The client tools' carried key bound (client/tools.md, 12).
pub const PROGRAM_NAME: u32 = jig_client::contract::PROGRAM_NAME;
/// jig's protocol/configuration.md program-adapter names are at most 32 bytes.
pub const PROGRAM_ADAPTER: u32 = 32;
pub const KEY_BYTES: u32 = 4096;
/// Whole moved records, client/tools.md, 12.
pub const MOVED_BYTES: u32 = 128 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub tool: u8,
    pub form: u8,
    pub values: Box<[Option<Values>]>,
    pub key: Option<Name>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Values {
    One(Value),
    Many(Box<[Value]>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Number(u64),
    Word(u8),
    Text(Box<[u8]>),
    Stdin,
    File(Box<[u8]>),
    Duration(skein_lib::Duration),
    Proposal(ProposalKey),
    Digest([u8; 8]),
    Condition(Condition),
    Switch,
    Project(Project),
    Turn(TurnName),
    Cursor(Box<[u8]>),
    Group(InboxGroup),
    Order(QueryOrder),
    Tool(u8),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Project {
    Number(u32),
    Name(Box<[u8]>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    Call { call: Call, json: bool },
    File { file: Source, json: bool },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Path(Box<[u8]>),
    Stdin,
}

pub(crate) fn value(call: &Call, index: usize) -> Option<&Value> {
    match call.values.get(index) {
        Some(Some(Values::One(value))) => Some(value),
        _ => None,
    }
}
pub(crate) fn number(call: &Call, index: usize) -> Option<u64> {
    match value(call, index) {
        Some(Value::Number(number)) => Some(*number),
        _ => None,
    }
}
pub(crate) fn cursor(call: &Call, index: usize) -> Option<jig_client::contract::Cursor> {
    match value(call, index) {
        Some(Value::Cursor(bytes)) => {
            Some(jig_client::contract::Cursor::new(bytes.clone(), u32::MAX).expect("parsed cursor"))
        }
        _ => None,
    }
}

/// Whether this read was asked to wait.
#[must_use]
pub fn waiting(row: &super::table::Tool, call: &Call) -> bool {
    argument(row, call, b"since").is_some() || argument(row, call, b"until").is_some()
}
pub(crate) fn argument<'a>(row: &super::table::Tool, call: &'a Call, name: &[u8]) -> Option<&'a Value> {
    let args = row.forms.get(usize::from(call.form))?;
    for (index, arg) in args.iter().enumerate() {
        if arg.name == name {
            return value(call, index);
        }
    }
    None
}
