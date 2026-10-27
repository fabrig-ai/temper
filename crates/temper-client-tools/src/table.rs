//! The tools available to this host (client/tools.md, 5).
use crate::jig::{
    argv::{self, Usage},
    call::Parsed,
    rows,
    table::Tool,
};
/// A tool's first word: a noun of temper's, or one of the client's own names.
#[derive(Debug)]
pub struct Group {
    pub name: &'static [u8],
    pub line: &'static [u8],
}
/// In `temper ctl help`'s order.
pub const GROUPS: &[Group] = &[
    Group { name: b"sign-in", line: b"Sign in with the link the deployment printed" },
    Group { name: b"sign-out", line: b"End the sign-in and leave the token empty" },
    Group { name: b"pending", line: b"List the requests DIR keeps, without reaching the engine" },
    Group { name: b"help", line: b"Read the page ctl and the groups, a group's tools, or one tool's record" },
    Group { name: b"me", line: b"Read your party: its projects, what it may start, and what waits on it" },
    Group { name: b"inbox", line: b"Read what waits on your party, then its updates" },
    Group { name: b"task", line: b"Tasks: read one and its parts, and act on it" },
    Group { name: b"chat", line: b"Chats: start one, read its conversation, close it" },
    Group { name: b"proposal", line: b"Proposals: read one, decide it" },
    Group { name: b"hold", line: b"Placed holds: lift one you may lift" },
    Group { name: b"change", line: b"Changes: read a change's way to landing" },
    Group { name: b"repository", line: b"Repositories: adopt one into a project" },
    Group {
        name: b"forge",
        line: b"Forges: read them, set one up, connect it, accept an installation, read what it reaches",
    },
    Group {
        name: b"account",
        line: b"LLM accounts: read them, connect one, manage its keys and sources, move one in or out",
    },
    Group { name: b"model", line: b"Models: read those your party may use" },
    Group { name: b"settings", line: b"The settings: read them, change them, or propose a change to their owners" },
    Group { name: b"revision", line: b"Settings revisions: read one or the history, move running work to the newest" },
    Group { name: b"project", line: b"Projects: read their periods and pools, set their spend per period" },
    Group { name: b"playground", line: b"The playground: start the demo" },
];
pub const TOOLS: &[&Tool] = &[
    &rows::ME,
    &rows::INBOX,
    &rows::TASKS,
    &rows::TASK,
    &rows::CHAT,
    &rows::ACTIVITY,
    &rows::TURN,
    &rows::RUN,
    &rows::CHAT_CREATE,
    &rows::MESSAGE,
    &rows::CLOSE,
    &rows::ANSWER,
    &rows::DECIDE,
    &rows::RELEASE,
    &rows::STOP,
    &rows::CANCEL,
    &rows::TREE,
    &crate::adopt::ROW,
    &rows::SIGN_IN,
    &rows::SIGN_OUT,
    &rows::PENDING,
    &rows::HELP,
    &rows::ACCOUNTS,
    &rows::MODELS,
    &rows::CONNECT,
    &rows::DISCONNECT,
    &rows::ADD_KEY,
    &rows::REPLACE_KEY,
    &rows::FORGET,
    &rows::RANK,
    &rows::RETRY,
    &rows::AMEND,
    &rows::DECLARE,
    &rows::VERIFY,
    &crate::forges::READ,
    &crate::forges::SET_UP,
    &crate::forges::DEMO_START,
    &crate::forges::CONNECT,
    &crate::forges::INSTALL,
    &crate::forges::REPOSITORIES,
    &rows::EXPORT,
    &rows::IMPORT,
    &rows::SETTINGS,
    &rows::SETTING,
    &rows::REVISION,
    &rows::REVISIONS,
    &rows::CHANGE_SETTINGS,
    &rows::PROPOSE_SETTINGS,
    &rows::PROPOSAL,
    &crate::change::READ,
    &crate::forges::ACCEPT,
    &rows::MOVE_TO_NEWEST,
    &rows::TASK_DECIDE,
    &rows::PROJECT_SHOW,
    &rows::PROJECT_FUND,
    &rows::LIFT,
];
pub fn parse(words: &[&[u8]]) -> Result<Parsed, Usage> {
    argv::parse(TOOLS, words, &crate::limits::tools())
}

/// The caller supplies argv to render the offending group and verb.
pub fn refusal(why: Usage, words: &[&[u8]], out: &mut skein_lib::Writer) -> Result<(), skein_lib::Overflow> {
    match why {
        Usage::NoGroup { at } => {
            out.put(b"no group ")?;
            out.put(words.get(usize::from(at)).ok_or(skein_lib::Overflow)?)?;
            out.put(b"; temper ctl help lists them")
        }
        Usage::NeedsVerb { at } => {
            let group = words.get(usize::from(at)).ok_or(skein_lib::Overflow)?;
            out.put(group)?;
            out.put(b" needs a verb; temper ctl help ")?;
            out.put(group)?;
            out.put(b" lists them")
        }
        Usage::NoTool { group, verb } => {
            let group = words.get(usize::from(group)).ok_or(skein_lib::Overflow)?;
            out.put(b"no tool ")?;
            out.put(group)?;
            out.put(b" ")?;
            out.put(words.get(usize::from(verb)).ok_or(skein_lib::Overflow)?)?;
            out.put(b"; temper ctl help ")?;
            out.put(group)?;
            out.put(b" lists them")
        }
        Usage::Timeout => out.put(b"--timeout needs --since or --until"),
        Usage::TerminalPhase { phase } => {
            out.put(b"--until phase:")?;
            out.put(crate::jig::subject::phase_word(phase))?;
            out.put(b" never holds: a task's watch ends with the task; use --until ended")
        }
        Usage::WaitPart { tool, argument } => {
            let row = TOOLS.get(usize::from(tool)).ok_or(skein_lib::Overflow)?;
            let args = row.forms.first().ok_or(skein_lib::Overflow)?;
            let spec = args.get(usize::from(argument)).ok_or(skein_lib::Overflow)?;
            out.put(row.name)?;
            out.put(b" waits on its subject alone: drop --")?;
            out.put(spec.name)
        }
        Usage::AmendmentRevision => out.put(b"task amend needs --rev REV"),
        Usage::Tool
        | Usage::Unknown { .. }
        | Usage::Extra { .. }
        | Usage::Missing { .. }
        | Usage::Twice { .. }
        | Usage::Kind { .. }
        | Usage::Rows { .. }
        | Usage::Long { .. }
        | Usage::Many { .. }
        | Usage::Stdin
        | Usage::Key
        | Usage::Condition
        | Usage::Json => out.put(USAGE),
    }
}
pub const USAGE: &[u8] = b"usage: temper ctl DIR GROUP VERB [ARGUMENT...]; temper ctl help GROUP VERB prints one tool";
