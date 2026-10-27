//! The read and host-tool records, in their published order.
use super::table::{
    Keyed, Kind,
    Occurs::{Optional, Required},
    Tool, flag, positional, switch,
};
pub const ME: Tool = Tool {
    name: b"me",
    line: b"Read the party, projects and waiting counts",
    prints: b"",
    result: b"me",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const INBOX: Tool = Tool {
    name: b"inbox",
    line: b"Read what needs the party and its updates",
    prints: b"",
    result: b"inbox",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Optional, b"Project name or number"),
        flag(b"kind", b"K", Kind::Group, Optional, b"Inbox group"),
        flag(b"root", b"N", Kind::Number, Optional, b"Root task"),
        flag(b"after", b"C", Kind::Cursor, Optional, b"Continue after this cursor"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const TASKS: Tool = Tool {
    name: b"tasks",
    line: b"Read task lines",
    prints: b"",
    result: b"tasks",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Required, b"Project name or number"),
        flag(b"label", b"L", Kind::Word { set: &[b"chat", b"goal"] }, Optional, b"Task label"),
        switch(b"tracked", b"Only tracked tasks"),
        switch(b"mine", b"Only my tasks"),
        switch(b"ended", b"Only ended tasks"),
        flag(b"order", b"O", Kind::Order, Optional, b"activity or priority; number by default"),
        flag(b"after", b"C", Kind::Cursor, Optional, b"Continue after this cursor"),
        flag(b"parent", b"N", Kind::Number, Optional, b"Only this task's delegates"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const TASK: Tool = Tool {
    name: b"task",
    line: b"Read a task and selected related documents",
    prints: b"",
    result: b"task",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        switch(b"tree", b"Include the live tree"),
        flag(b"activity", b"K", Kind::Count, Optional, b"Latest activity lines"),
        switch(b"run", b"Include the live run"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const CHAT: Tool = Tool {
    name: b"chat",
    line: b"Read a conversation from its end",
    prints: b"",
    result: b"chat",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        flag(b"before", b"C", Kind::Cursor, Optional, b"Continue before this cursor"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const ACTIVITY: Tool = Tool {
    name: b"activity",
    line: b"Read task activity from its end",
    prints: b"",
    result: b"activity",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        flag(b"before", b"C", Kind::Cursor, Optional, b"Continue before this cursor"),
        flag(b"limit", b"K", Kind::Count, Optional, b"Most activity lines"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const TURN: Tool = Tool {
    name: b"turn",
    line: b"Read one turn and its parts",
    prints: b"",
    result: b"turn",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        positional(b"turn", b"A.T", Kind::Turn, Required, b"Attempt and turn"),
        flag(b"part", b"K", Kind::Number, Optional, b"Part number"),
        flag(b"from", b"B", Kind::Number, Optional, b"Byte offset"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const RUN: Tool = Tool {
    name: b"run",
    line: b"Read a task's live run",
    prints: b"",
    result: b"run",
    forms: &[&[positional(b"task", b"N", Kind::Number, Required, b"Task number")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const SIGN_IN: Tool = Tool {
    name: b"sign-in",
    line: b"Sign in with the link the engine's host printed",
    prints: b"",
    result: b"sign_in",
    forms: &[&[positional(b"file", b"FILE|-", Kind::File, Required, b"File holding the sign-in link")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const SIGN_OUT: Tool = Tool {
    name: b"sign-out",
    line: b"End sign-in and leave the token empty",
    prints: b"",
    result: b"sign_out",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const PENDING: Tool = Tool {
    name: b"pending",
    line: b"List requests kept in the store",
    prints: b"",
    result: b"pending",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: br#"Reads only the store: no lock, client or send.
Rows: operation, party N, first sending in wall-time words or not sent.
chat, party 3, not sent
{"pending":[{"operation":"chat","party":3,"first_sent":null}],"unread":2}
2 kept records these tools cannot read; a call that sends drops them
Unread records are counted, never listed; exit 0.
An invalid store: DIRECTORY's store cannot be read (exit 1).
"#,
};
pub const HELP: Tool = Tool {
    name: b"help",
    line: b"Read the guide or one tool's record",
    prints: b"",
    result: b"help",
    forms: &[&[positional(b"tool", b"TOOL", Kind::Tool, Optional, b"Tool to describe")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const CHAT_CREATE: Tool = Tool {
    name: b"chat",
    line: b"Start a chat",
    prints: b"",
    result: b"chat_create",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Required, b"Project name or number"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"Opening words"),
        MODEL,
        EFFORT,
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
pub const MESSAGE: Tool = Tool {
    name: b"message",
    line: b"Send words to a task",
    prints: b"",
    result: b"message",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"Words to send"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
pub const CLOSE: Tool = Tool {
    name: b"close",
    line: b"Close a chat and cancel or move its delegates",
    prints: b"",
    result: b"close",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        super::table::repeated(b"move", b"D", Kind::Number, 8, b"Delegate to move to the party"),
        flag(b"reason", b"TEXT", Kind::Text, Optional, b"Reason, empty by default"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
pub const ANSWER: Tool = Tool {
    name: b"answer",
    line: b"Answer a task's question",
    prints: b"",
    result: b"answer",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        positional(b"question", b"M", Kind::Number, Required, b"Question number"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"Answer"),
    ]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const DECIDE: Tool = Tool {
    name: b"decide",
    line: b"Decide a proposal or an escalation",
    prints: b"",
    result: b"decide",
    forms: &[
        &[
            positional(b"proposal", b"P/K", Kind::Proposal, Required, b"Typed proposer and proposal number"),
            positional(
                b"outcome",
                b"OUTCOME",
                Kind::Word { set: &[b"accept", b"reject", b"pass-up"] },
                Required,
                b"Decision",
            ),
            positional(b"reason", b"REASON", Kind::Text, Optional, b"Reason"),
            flag(
                b"running",
                b"stays|moves",
                Kind::Word { set: &[b"stays", b"moves"] },
                Optional,
                b"Running work stays by default",
            ),
        ],
        &[
            positional(b"task", b"N", Kind::Number, Required, b"Task number"),
            flag(b"hold", b"R", Kind::Number, Required, b"Hold revision"),
            positional(
                b"outcome",
                b"OUTCOME",
                Kind::Word { set: &[b"release", b"leave", b"pass-up"] },
                Required,
                b"Decision",
            ),
            positional(b"reason", b"REASON", Kind::Text, Optional, b"Reason"),
        ],
    ],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const RELEASE: Tool = Tool {
    name: b"release",
    line: b"Lift a task's hold",
    prints: b"",
    result: b"release",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        flag(b"hold", b"R", Kind::Number, Required, b"Hold revision"),
        switch(b"again", b"Run again"),
    ]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const STOP: Tool = Tool {
    name: b"stop",
    line: b"Stop a task's run and hold it",
    prints: b"",
    result: b"stop",
    forms: &[&[positional(b"task", b"N", Kind::Number, Required, b"Task number")]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const CANCEL: Tool = Tool {
    name: b"cancel",
    line: b"Cancel a task and its tree",
    prints: b"",
    result: b"cancel",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        flag(b"grace", b"D", Kind::Duration, Optional, b"Grace before cancellation"),
        positional(b"reason", b"REASON", Kind::Text, Required, b"Reason"),
    ]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const WAIT: Tool = Tool {
    name: b"wait",
    line: b"Wait for a watch's change or condition",
    prints: b"",
    result: b"wait",
    forms: &[&[
        positional(b"subject", b"SUBJECT", Kind::Subject, Required, b"Watch subject"),
        flag(b"since", b"DIGEST", Kind::Digest, Optional, b"Wait for a different digest"),
        flag(b"until", b"CONDITION", Kind::Condition, Optional, b"Wait for a condition"),
        flag(b"timeout", b"DURATION", Kind::Duration, Optional, b"Timeout, 300 seconds by default"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};

/// Area 5 reads append to preserve earlier tool and keyed-call identities.
pub const ACCOUNTS: Tool = Tool {
    name: b"accounts",
    line: b"Read accounts and their sources without values",
    prints: b"",
    result: b"accounts",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"",
};
pub const MODELS: Tool = Tool {
    name: b"models",
    line: b"Read authorized models grouped by provider",
    prints: b"",
    result: b"models",
    forms: &[&[flag(b"project", b"P", Kind::Project { empty: false }, Optional, b"Project name or number")]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"",
};

/// Account source shorthand is resolved against the current accounts watch.
const SOURCE: super::table::Argument =
    flag(b"source", b"S", Kind::Word { set: &[b"sign_in", b"key", b"program"] }, Optional, b"One source by kind");
pub const CONNECT: Tool = Tool {
    name: b"connect",
    line: b"Start an LLM provider sign-in",
    prints: b"",
    result: b"connect",
    forms: &[&[positional(b"provider", b"PROVIDER", Kind::Number, Required, b"LLM provider number")]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"",
};
pub const DISCONNECT: Tool = Tool {
    name: b"disconnect",
    line: b"Sign an account source out",
    prints: b"",
    result: b"disconnect",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};
pub const ADD_KEY: Tool = Tool {
    name: b"add-key",
    line: b"Add a key from a private file",
    prints: b"",
    result: b"add_key",
    forms: &[&[
        positional(b"provider", b"PROVIDER", Kind::Number, Required, b"LLM provider number"),
        positional(b"file", b"FILE", Kind::File, Required, b"Private key file; never stdin"),
        flag(b"account", b"A", Kind::Number, Optional, b"Join an account"),
        switch(b"deployment", b"Deployment holder, by an owner"),
    ]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"",
};
pub const REPLACE_KEY: Tool = Tool {
    name: b"replace-key",
    line: b"Replace an account's key from a private file",
    prints: b"",
    result: b"replace_key",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"),
        positional(b"file", b"FILE", Kind::File, Required, b"Private key file; never stdin"),
    ]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};
pub const FORGET: Tool = Tool {
    name: b"forget",
    line: b"Remove an account or one source",
    prints: b"",
    result: b"forget",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};
pub const RANK: Tool = Tool {
    name: b"rank",
    line: b"Move an account or source to a one-based position",
    prints: b"",
    result: b"rank",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"),
        positional(b"position", b"POSITION", Kind::Number, Required, b"Place from one"),
        SOURCE,
    ]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};
pub const RETRY: Tool = Tool {
    name: b"retry",
    line: b"Resume a limited source",
    prints: b"",
    result: b"retry",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};

/// Names are checked against the project's current me document before sending.
const MODEL: super::table::Argument = flag(b"model", b"M", Kind::Text, Optional, b"Configured model name");
const EFFORT: super::table::Argument = flag(b"effort", b"E", Kind::Text, Optional, b"Effort offered by that model");

/// The published whole dependency list fits one client document window.
pub const DEPENDENCIES: u32 = 32;

/// Each named control is replaced whole on the task's supplied document.
pub const AMEND: Tool = Tool {
    name: b"amend",
    line: b"Amend a task: each change named, the rest kept",
    prints: b"",
    result: b"amend",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"Task number"),
        flag(b"rev", b"R", Kind::Number, Optional, b"Shown spec revision; none for a tracked task's priority alone"),
        flag(b"priority", b"P", Kind::Number, Optional, b"Track the task at this priority"),
        switch(b"untracked", b"Stop tracking the task"),
        flag(b"deadline", b"T", Kind::Literal, Optional, b"New deadline, as 2026-10-24T17:00Z"),
        switch(b"no-deadline", b"Remove the deadline"),
        flag(b"instructions", b"FILE", Kind::File, Optional, b"File of the whole instructions, or -; empty for none"),
        flag(b"spec", b"FILE", Kind::File, Optional, b"File of the spec's words, or -"),
        super::table::repeated(
            b"dependency",
            b"M",
            Kind::Number,
            DEPENDENCIES,
            b"A dependency; repeated, the whole list",
        ),
        switch(b"no-dependencies", b"Remove every dependency"),
        MODEL,
        EFFORT,
        positional(b"reason", b"REASON", Kind::Text, Required, b"Reason for the amendment"),
    ]],
    keyed: Keyed::Optional,
    area: 2,
    notes: b"",
};

/// Appended so existing keyed tool identities stay stable.
pub const DECLARE: Tool = Tool {
    name: b"declare",
    line: b"Declare a program login; the answer names the command for each host",
    prints: b"",
    result: b"declare",
    forms: &[&[
        positional(b"provider", b"PROVIDER", Kind::Number, Required, b"LLM provider number from me"),
        positional(b"adapter", b"ADAPTER", Kind::Literal, Required, b"Program offered by the provider in me"),
        positional(b"name", b"NAME", Kind::Literal, Required, b"1 to 32 lower-case letters, digits or hyphens"),
        flag(b"account", b"A", Kind::Number, Optional, b"Join an account"),
    ]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"",
};
pub const VERIFY: Tool = Tool {
    name: b"verify",
    line: b"Ask hosts to check a program login; then wait accounts --until settled",
    prints: b"",
    result: b"verify",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"",
};

/// Account moves append so existing keyed tool identities stay stable.
pub const EXPORT: Tool = Tool {
    name: b"export",
    line: b"Move a signed-in account out to a new private file, synced before written",
    prints: b"",
    result: b"export",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"Account number"),
        positional(b"file", b"FILE", Kind::File, Required, b"New private record file; never stdin"),
    ]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"",
};
pub const IMPORT: Tool = Tool {
    name: b"import",
    line: b"Move a record in from a private file, then remove the spent file",
    prints: b"",
    result: b"import",
    forms: &[&[positional(b"file", b"FILE", Kind::File, Required, b"Private record file; never stdin")]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"",
};
/// `export`'s and `import`'s places in the host's table, `table::TOOLS`,
/// after every row before them (tests/help.rs checks them).
pub const EXPORT_AT: u8 = 40;
pub const IMPORT_AT: u8 = 41;

/// Settings append after account moves to preserve every published identity.
pub const SETTINGS_AT: u8 = 42;
pub const SETTING_AT: u8 = 43;
pub const REVISION_AT: u8 = 44;
pub const REVISIONS_AT: u8 = 45;
pub const CHANGE_SETTINGS_AT: u8 = 46;
pub const SETTINGS: Tool = Tool {
    name: b"settings",
    line: b"Read the settings, held revisions and pending proposals",
    prints: b"",
    result: b"settings",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const SETTING: Tool = Tool {
    name: b"setting",
    line: b"Read one setting's value whole; JSON values feed change-settings",
    prints: b"",
    result: b"setting",
    forms: &[
        &[
            positional(
                b"kind",
                b"charter|template|policy",
                Kind::Word { set: &[b"charter", b"template", b"policy"] },
                Required,
                b"Setting kind",
            ),
            positional(b"number", b"N", Kind::NumberRange { min: 0, max: 4_294_967_295 }, Required, b"Setting number"),
            flag(
                b"revision",
                b"R",
                Kind::NumberRange { min: 0, max: 4_294_967_295 },
                Optional,
                b"Held charter revision; newest by default",
            ),
        ],
        &[positional(b"kind", b"rules", Kind::Word { set: &[b"rules"] }, Required, b"Deployment rules")],
    ],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const REVISION: Tool = Tool {
    name: b"revision",
    line: b"Read a revision and its whole items, or the oldest kept if forgotten",
    prints: b"",
    result: b"revision",
    forms: &[&[positional(
        b"revision",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Required,
        b"Revision number",
    )]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const REVISIONS: Tool = Tool {
    name: b"revisions",
    line: b"Read history newest first, a page at a time",
    prints: b"",
    result: b"revisions",
    forms: &[&[flag(
        b"before",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Optional,
        b"Read before this revision",
    )]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const CHANGE_SETTINGS: Tool = Tool {
    name: b"change-settings",
    line: b"Change deployment settings in one keyed decision",
    prints: b"",
    result: b"change_settings",
    forms: &[&[
        positional(b"items", b"FILE", Kind::File, Required, b"JSON items file, or - for stdin"),
        flag(b"against", b"R", Kind::NumberRange { min: 0, max: 4_294_967_295 }, Required, b"Newest revision read"),
        flag(
            b"running",
            b"stays|moves",
            Kind::Word { set: &[b"stays", b"moves"] },
            Optional,
            b"Running work stays by default",
        ),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};

/// New rows append to preserve every earlier durable call identity.
pub const PROPOSE_SETTINGS_AT: u8 = 47;
pub const PROPOSAL_AT: u8 = 48;
pub const PROPOSE_SETTINGS: Tool = Tool {
    name: b"propose-settings",
    line: b"Propose a change of deployment settings to its owners",
    prints: b"",
    result: b"propose_settings",
    forms: &[&[
        positional(b"items", b"FILE", Kind::File, Required, b"JSON items file, or - for stdin"),
        positional(b"reason", b"REASON", Kind::Text, Optional, b"Reason"),
        flag(b"against", b"R", Kind::NumberRange { min: 0, max: 4_294_967_295 }, Required, b"Newest revision read"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
pub const PROPOSAL: Tool = Tool {
    name: b"proposal",
    line: b"Read a proposal, with its review for settings",
    prints: b"",
    result: b"proposal",
    forms: &[&[positional(b"proposal", b"P/K", Kind::Proposal, Required, b"Typed proposer and proposal number")]],
    keyed: Keyed::Never,
    area: 2,
    notes: b"",
};

/// New rows append to preserve every earlier durable call identity.
pub const MOVE_TO_NEWEST_AT: u8 = 51;
pub const MOVE_TO_NEWEST: Tool = Tool {
    name: b"move-to-newest",
    line: b"Move every live agent task on held revision R that may move to the newest",
    prints: b"",
    result: b"move_to_newest",
    forms: &[&[positional(
        b"revision",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Required,
        b"Held revision number",
    )]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
