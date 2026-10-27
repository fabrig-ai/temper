//! The read and host-tool records, in their published order.
use super::table::{
    Keyed, Kind,
    Occurs::{Optional, Required},
    Tool, flag, positional, switch,
};
pub const ME: Tool = Tool {
    name: b"me",
    line: b"Read your party: its projects, what it may start, and what waits on it",
    prints: b"`party N`, its login and name, its projects and bounds, what waits on it, the requests' retention, the providers it may use, and `digest: me`",
    result: b"me",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"Read it first: it gives the project and provider numbers other\ncalls take, and how long the engine keeps a keyed request.\n",
};
pub const INBOX: Tool = Tool {
    name: b"inbox",
    line: b"Read what waits on your party, then its updates",
    prints: b"A line per item, its task and what waits, then `read on with: inbox ... --after C` or `no more`, and, unfiltered, `digest: inbox`",
    result: b"inbox",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Optional, b"Only this project"),
        flag(b"kind", b"K", Kind::Group, Optional, b"Only this group"),
        flag(b"root", b"N", Kind::Number, Optional, b"Only what waits under this goal, by its task number"),
        flag(b"after", b"C", Kind::Cursor, Optional, b"The cursor a page printed, to read on"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"Questions, proposals, escalations and person tasks come first;\nupdates, results and replies, follow once those are read whole.\nA page holds at most 32 lines. A filtered or continued read has\nno digest: wait on the inbox unfiltered.\n",
};
pub const TASKS: Tool = Tool {
    name: b"tasks",
    line: b"Read a project's tasks, a line each",
    prints: b"A line per task, its number, kind and phase, then `read on with: tasks ... --after C` or `no more`; the first page of live tasks ends with a digest",
    result: b"tasks",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Required, b"The project"),
        flag(b"label", b"L", Kind::Word { set: &[b"chat", b"goal"] }, Optional, b"Only chats, or only goals"),
        switch(b"tracked", b"Only tracked tasks"),
        switch(b"mine", b"Only the tasks your party asked for"),
        switch(b"ended", b"Ended tasks, not live ones"),
        flag(b"order", b"O", Kind::Order, Optional, b"By last activity or by priority; by number when absent"),
        flag(b"after", b"C", Kind::Cursor, Optional, b"The cursor a page printed, to read on"),
        flag(b"parent", b"N", Kind::Number, Optional, b"Only this task's delegates"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"A page holds at most 32 tasks. Live and ended tasks are read apart.\nOnly the first page of live tasks has a digest to wait on.\n",
};
pub const TASK: Tool = Tool {
    name: b"task",
    line: b"Read a task: its state, what it waits on, and the acts you may take",
    prints: b"`task N`, its kind, phase, project and tracking, its fields, `acts:` and a digest; each part asked for after it, with its own digest",
    result: b"task",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        switch(b"tree", b"Add its live tree of delegates"),
        flag(b"activity", b"K", Kind::Count, Optional, b"Add its latest K lines of activity"),
        switch(b"run", b"Add its live run"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"`acts:` lists the calls the engine offers your party, ready to copy.\nAn act marked `(no tool)` has no tool here yet; one followed by a\ncolon and a reason is refused as it stands; one marked `proposed\nto` asks another party first. A change task adds `next:` and the\ncall that reads its change. An ended task prints its result.\n",
};
pub const CHAT: Tool = Tool {
    name: b"chat",
    line: b"Read a chat's conversation from its end",
    prints: b"The conversation, oldest line first: each turn's words, each run's start and end, then `read on with: chat N --before C` or `no more`",
    result: b"chat",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The chat's task number"),
        flag(b"before", b"C", Kind::Cursor, Optional, b"The cursor a page printed, to read further back"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"Each page is older than the one before it. A run still writing\nadds its words so far.\n",
};
pub const ACTIVITY: Tool = Tool {
    name: b"activity",
    line: b"Read a task's activity from its end",
    prints: b"A line per event, oldest first, then `read on with: activity N --before C` or `no more`",
    result: b"activity",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        flag(b"before", b"C", Kind::Cursor, Optional, b"The cursor a page printed, to read further back"),
        flag(b"limit", b"K", Kind::Count, Optional, b"At most K lines"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"A turn prints as its size and `read with: turn N A.T`. With\n`--limit`, the page keeps its last K lines, and `read on with:`\nrepeats the read without the limit.\n",
};
pub const TURN: Tool = Tool {
    name: b"turn",
    line: b"Read one turn of a task's run, part by part",
    prints: b"`turn A.T`, then its words, calls and results in order; a cut part ends with `K bytes left; read on with: turn N A.T --part K --from B`",
    result: b"turn",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        positional(b"turn", b"A.T", Kind::Turn, Required, b"The attempt and turn, as activity prints them"),
        flag(b"part", b"K", Kind::Number, Optional, b"Only this part"),
        flag(b"from", b"B", Kind::Number, Optional, b"From this byte of the part"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"A part prints at most 16384 bytes at a time. A turn past ctl's\nbound prints its size and `past the limit; report it`, and one it\ncannot decode `unreadable; report it`, both with exit 0: report\neither as a finding.\n",
};
pub const RUN: Tool = Tool {
    name: b"run",
    line: b"Read a task's live run: its attempt, its call in flight, its words so far",
    prints: b"`run: A` or `run: none`, the call in flight, the words so far, and `digest: run`",
    result: b"run",
    forms: &[&[positional(b"task", b"N", Kind::Number, Required, b"The task's number")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"`wait run N --until ended` returns when the run ends.\n",
};
pub const SIGN_IN: Tool = Tool {
    name: b"sign-in",
    line: b"Sign in with the link the deployment printed",
    prints: b"Your party, as me prints it",
    result: b"sign_in",
    forms: &[&[positional(b"file", b"FILE|-", Kind::File, Required, b"The file holding the link, or - for stdin")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"It writes the token and the engine's address into DIR. A link\nsigns in once, and expires; `the sign-in link was refused` (exit\n1) asks for a fresh link. With every sign-in in use, it says when\nthe earliest ends and until when this link stays usable. It leaves\nthe link's file as it was.\n",
};
pub const SIGN_OUT: Tool = Tool {
    name: b"sign-out",
    line: b"End the sign-in and leave the token empty",
    prints: b"Nothing",
    result: b"sign_out",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"It sends none of the requests DIR keeps.\n",
};
pub const PENDING: Tool = Tool {
    name: b"pending",
    line: b"List the requests DIR keeps, without reaching the engine",
    prints: b"A line per kept request, `OP, party N, first sent T` or `OP, party N, not sent`; nothing when DIR keeps none",
    result: b"pending",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"It reads DIR's store alone: it takes no lock, signs in to nothing\nand sends nothing. Records these tools cannot read are counted,\nnever listed: `N kept records these tools cannot read; a call that\nsends drops them`, with exit 0. A store that cannot be read exits\n1.\n",
};
pub const HELP: Tool = Tool {
    name: b"help",
    line: b"Read the page ctl and every tool, or one tool's record",
    prints: b"The page ctl, then a line per tool; with TOOL, its record",
    result: b"help",
    forms: &[&[positional(b"tool", b"TOOL", Kind::Tool, Optional, b"The tool to describe")]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"It takes no DIR. With `--json`, it prints the first row of the\ntool's name.\n",
};
pub const CHAT_CREATE: Tool = Tool {
    name: b"chat",
    line: b"Start a chat in a project",
    prints: b"`done: task N, chat`, or `proposed: P/K, to RECIPIENT` when another party must agree",
    result: b"chat_create",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Required, b"The project"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"The opening words"),
        super::table::Argument { line: b"A model the chat may use, as models lists it", ..MODEL },
        super::table::Argument { line: b"An effort that model offers", ..EFFORT },
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"The chat's template chooses its model unless `--model` names one;\n`--effort` needs `--model`, and a model that declares efforts needs\n`--effort`. A chat past what your party may start, but within what\nit may propose, is proposed (exit 0). `chat not offered` and `chat\nexceeds offered bounds` (exit 1): your party may not start it.\nThen `wait task N --until replied`.\n",
};
pub const MESSAGE: Tool = Tool {
    name: b"message",
    line: b"Send words to a task",
    prints: b"`done: task N, message`",
    result: b"message",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"The words"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
pub const CLOSE: Tool = Tool {
    name: b"close",
    line: b"Close a chat: move the delegates you name to your party, cancel the rest",
    prints: b"`done: task N, close`",
    result: b"close",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The chat's task number"),
        super::table::repeated(b"move", b"D", Kind::Number, 8, b"A live delegate to move to your party"),
        flag(b"reason", b"TEXT", Kind::Text, Optional, b"Why; empty when absent"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"Every live delegate that `--move` does not name is cancelled with\nthe chat. A `--move` naming no live delegate fails, `move is not a\nlive delegate` (exit 1).\n",
};
pub const ANSWER: Tool = Tool {
    name: b"answer",
    line: b"Answer a task's question",
    prints: b"`done: task N, answer`",
    result: b"answer",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        positional(b"question", b"M", Kind::Number, Required, b"The question's number, as the inbox prints it"),
        positional(b"text", b"TEXT", Kind::Text, Required, b"The answer"),
    ]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const DECIDE: Tool = Tool {
    name: b"decide",
    line: b"Decide a proposal, or the escalation of a held task",
    prints: b"`done: decide`, or `decided first:`, by whom, how and when, if it was decided already",
    result: b"decide",
    forms: &[
        &[
            positional(b"proposal", b"P/K", Kind::Proposal, Required, b"The proposal, as the inbox prints it"),
            positional(
                b"outcome",
                b"OUTCOME",
                Kind::Word { set: &[b"accept", b"reject", b"pass-up"] },
                Required,
                b"The decision",
            ),
            positional(b"reason", b"REASON", Kind::Text, Optional, b"Why; sent with reject alone"),
            flag(
                b"running",
                b"stays|moves",
                Kind::Word { set: &[b"stays", b"moves"] },
                Optional,
                b"For a settings proposal: running work stays on its revision, or moves to the new one; stays when absent",
            ),
        ],
        &[
            positional(b"task", b"N", Kind::Number, Required, b"The held task's number"),
            flag(b"hold", b"R", Kind::Number, Required, b"The hold's revision, as the task prints it"),
            positional(
                b"outcome",
                b"OUTCOME",
                Kind::Word { set: &[b"release", b"leave", b"pass-up"] },
                Required,
                b"The decision",
            ),
            positional(b"reason", b"REASON", Kind::Text, Optional, b"Why; sent with leave alone"),
        ],
    ],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"A decision made already, by your party or another, answers\n`decided first:` with exit 0 and changes nothing.\n",
};
pub const RELEASE: Tool = Tool {
    name: b"release",
    line: b"Lift a task's hold",
    prints: b"`done: task N, release`",
    result: b"release",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        flag(b"hold", b"R", Kind::Number, Required, b"The hold's revision, as the task prints it"),
        switch(b"again", b"Run the task again"),
    ]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"",
};
pub const STOP: Tool = Tool {
    name: b"stop",
    line: b"Stop a task's run and hold it",
    prints: b"`done: task N, stop`",
    result: b"stop",
    forms: &[&[positional(b"task", b"N", Kind::Number, Required, b"The task's number")]],
    keyed: Keyed::Optional,
    area: 1,
    notes: b"The task stays held until `release` lifts the hold or `cancel` ends\nit.\n",
};
pub const CANCEL: Tool = Tool {
    name: b"cancel",
    line: b"Cancel a task and its tree",
    prints: b"`done: task N, cancel`",
    result: b"cancel",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        flag(
            b"grace",
            b"D",
            Kind::Duration,
            Optional,
            b"How long its run may finish first; the engine's default when absent",
        ),
        positional(b"reason", b"REASON", Kind::Text, Required, b"Why"),
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
        positional(b"subject", b"SUBJECT", Kind::Subject, Required, b"What to watch, from the list below"),
        flag(b"since", b"DIGEST", Kind::Digest, Optional, b"The digest of your last read of the subject"),
        flag(b"until", b"CONDITION", Kind::Condition, Optional, b"A condition of the subject, from the list below"),
        flag(b"timeout", b"DURATION", Kind::Duration, Optional, b"How long to wait; 300 seconds when absent"),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};

/// Area 5 reads append to preserve earlier tool and keyed-call identities.
pub const ACCOUNTS: Tool = Tool {
    name: b"accounts",
    line: b"Read the accounts and their sources, never their values",
    prints: b"`current: true`, then each account, its sources, their state and readiness, and `digest: accounts`",
    result: b"accounts",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"The deployment's own accounts show beside your party's when your\nparty owns the deployment. `wait accounts` takes this read's\ndigest.\n",
};
pub const MODELS: Tool = Tool {
    name: b"models",
    line: b"Read the models your projects may use, by provider",
    prints: b"`models:`, each project's providers and models with their windows, efforts and prices, then the accounts and `digest: models`",
    result: b"models",
    forms: &[&[flag(b"project", b"P", Kind::Project { empty: false }, Optional, b"Only this project")]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"Its digest is the models' read, not the accounts': wait on accounts\nwith the digest accounts prints.\n",
};

/// Account source shorthand is resolved against the current accounts watch.
const SOURCE: super::table::Argument = flag(
    b"source",
    b"S",
    Kind::Word { set: &[b"sign_in", b"key", b"program"] },
    Optional,
    b"The source by its kind, when the account has several",
);
pub const CONNECT: Tool = Tool {
    name: b"connect",
    line: b"Start a sign-in to an LLM provider",
    prints: b"`Open this address in a browser on the engine's machine:`, the address, then `wait accounts --until settled`",
    result: b"connect",
    forms: &[&[positional(b"provider", b"PROVIDER", Kind::Number, Required, b"The provider's number, as me prints it")]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"It returns once the sign-in starts. Open the address in a browser\non the engine's machine, where the provider sends the browser back,\nthen `wait accounts --until settled`.\n",
};
pub const DISCONNECT: Tool = Tool {
    name: b"disconnect",
    line: b"Sign an account's source out",
    prints: b"`done: disconnect`; for a program, where it signed out; for a provider that keeps its grant, where to revoke it",
    result: b"disconnect",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"Without `--source`, the account has one source. Where the\nprovider's grant cannot be revoked from here, the answer says where\nto revoke it, in the provider's settings.\n",
};
pub const ADD_KEY: Tool = Tool {
    name: b"add-key",
    line: b"Add an API key from a private file",
    prints: b"`done: add-key`",
    result: b"add_key",
    forms: &[&[
        positional(b"provider", b"PROVIDER", Kind::Number, Required, b"The provider's number, as me prints it"),
        positional(b"file", b"FILE", Kind::File, Required, b"The private file holding the key"),
        flag(b"account", b"A", Kind::Number, Optional, b"Join this account instead of making one"),
        switch(b"deployment", b"Hold it for the deployment, not your party; an owner's"),
    ]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"The file holds 1 to 4096 bytes after one final newline is trimmed.\nctl leaves it in place: remove it once the account is usable. Then\n`wait accounts --until usable`.\n",
};
pub const REPLACE_KEY: Tool = Tool {
    name: b"replace-key",
    line: b"Replace an account's key from a private file",
    prints: b"`done: replace-key`",
    result: b"replace_key",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"),
        positional(b"file", b"FILE", Kind::File, Required, b"The private file holding the new key"),
    ]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"The account has one key source. The file's bounds are add-key's.\n",
};
pub const FORGET: Tool = Tool {
    name: b"forget",
    line: b"Remove an account, or one of its sources",
    prints: b"`done: forget`",
    result: b"forget",
    forms: &[&[positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"), SOURCE]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"Without `--source`, the whole account goes. With it, a sign-in or\nprogram source is signed out, and a key source is erased.\n",
};
pub const RANK: Tool = Tool {
    name: b"rank",
    line: b"Move an account, or one of its sources, to a place in its order",
    prints: b"`done: rank`",
    result: b"rank",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"),
        positional(b"position", b"POSITION", Kind::Number, Required, b"The place, from 1"),
        SOURCE,
    ]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"Without `--source`, the account moves among the accounts; with it,\nthe source moves among its account's sources.\n",
};
pub const RETRY: Tool = Tool {
    name: b"retry",
    line: b"Resume a source that reached a limit",
    prints: b"`done: retry`",
    result: b"retry",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"),
        SOURCE,
    ]],
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
    line: b"Amend a task: each control you name is replaced, the rest kept",
    prints: b"`done: task N, amend`, or `proposed: P/K, to RECIPIENT` when another party must agree",
    result: b"amend",
    forms: &[&[
        positional(b"task", b"N", Kind::Number, Required, b"The task's number"),
        flag(b"rev", b"R", Kind::Number, Optional, b"The spec revision the task prints; not needed for its priority alone"),
        flag(b"priority", b"P", Kind::Number, Optional, b"Track it at this priority"),
        switch(b"untracked", b"Stop tracking it"),
        flag(b"deadline", b"T", Kind::Literal, Optional, b"Its new deadline, as 2026-10-24T17:00Z"),
        switch(b"no-deadline", b"Remove its deadline"),
        flag(b"instructions", b"FILE", Kind::File, Optional, b"A file of its whole instructions, or -; an empty file removes them"),
        flag(b"spec", b"FILE", Kind::File, Optional, b"A file of its spec's words, or -"),
        super::table::repeated(
            b"dependency",
            b"M",
            Kind::Number,
            DEPENDENCIES,
            b"A task it waits on; repeated, the whole list",
        ),
        switch(b"no-dependencies", b"Remove every dependency"),
        super::table::Argument { line: b"A model the task may use, as models lists it", ..MODEL },
        super::table::Argument { line: b"An effort that model offers", ..EFFORT },
        positional(b"reason", b"REASON", Kind::Text, Required, b"Why"),
    ]],
    keyed: Keyed::Optional,
    area: 2,
    notes: b"A task whose spec moved since your read fails with `read again:\ntask N`. Each file is read before anything is sent, one final\nnewline trimmed, at most 32768 bytes of UTF-8. An amendment that\nchanges nothing fails (exit 1).\n",
};

/// Appended so existing keyed tool identities stay stable.
pub const DECLARE: Tool = Tool {
    name: b"declare",
    line: b"Declare a program's login for a provider",
    prints: b"`Program source S: NAME`, then the command to run on each host",
    result: b"declare",
    forms: &[&[
        positional(b"provider", b"PROVIDER", Kind::Number, Required, b"The provider's number, as me prints it"),
        positional(b"adapter", b"ADAPTER", Kind::Literal, Required, b"A program the provider offers, as me prints it"),
        positional(
            b"name",
            b"NAME",
            Kind::Literal,
            Required,
            b"The login's name: 1 to 32 lower-case letters, digits or hyphens",
        ),
        flag(b"account", b"A", Kind::Number, Optional, b"Join this account instead of making one"),
    ]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"No program runs for you: run the printed command on each host that\nruns the program, then `verify`.\n",
};
pub const VERIFY: Tool = Tool {
    name: b"verify",
    line: b"Ask the hosts to check a program's login",
    prints: b"`asked`, then `wait accounts --until settled`",
    result: b"verify",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"),
        SOURCE,
    ]],
    keyed: Keyed::Optional,
    area: 5,
    notes: b"Only a program source has a login to check; a sign-in or key\nsource fails (exit 1).\n",
};

/// Account moves append so existing keyed tool identities stay stable.
pub const EXPORT: Tool = Tool {
    name: b"export",
    line: b"Move a signed-in account out to a new private file",
    prints: b"`moved out: account N to FILE`",
    result: b"export",
    forms: &[&[
        positional(b"account", b"ACCOUNT", Kind::Number, Required, b"The account's number, as accounts prints it"),
        positional(b"file", b"FILE", Kind::File, Required, b"The new file; nothing may be there"),
    ]],
    keyed: Keyed::Never,
    area: 5,
    notes: b"It takes no --key. It creates FILE, mode 0600, before sending\nanything, writes and syncs the account's record, and only then\ntells the engine; the account then leaves this deployment. A\nfailure before that removes FILE, and the engine keeps the account\nand cancels the move at its deadline: export again after it.\nImport FILE, as it is, on the deployment it moves to.\n",
};
pub const IMPORT: Tool = Tool {
    name: b"import",
    line: b"Move an account in from the file export wrote, then remove the file",
    prints: b"`moved in: account N`",
    result: b"import",
    forms: &[&[positional(b"file", b"FILE", Kind::File, Required, b"The private file export wrote")]],
    keyed: Keyed::Required,
    area: 5,
    notes: b"The file holds 1 to 131072 bytes, read whole. It is removed once\nthe engine holds the account, and kept otherwise: the same call\nwith the same key and file gets the first answer. A record of a\nnewer form than this binary reads asks for an upgrade.\n",
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
    line: b"Read the settings, the revisions running work holds, and pending proposals",
    prints: b"`newest revision R`, each setting and its last revision, the held revisions and their live tasks, and the pending proposals",
    result: b"settings",
    forms: &[&[]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"`setting KIND N --json` prints one value to edit. change-settings\nand propose-settings take the newest revision this prints as\n`--against`.\n",
};
pub const SETTING: Tool = Tool {
    name: b"setting",
    line: b"Read one setting's value whole",
    prints: b"`KIND N, revision R`, or `rules, revision R`, then the value as JSON on one line",
    result: b"setting",
    forms: &[
        &[
            positional(
                b"kind",
                b"charter|template|policy",
                Kind::Word { set: &[b"charter", b"template", b"policy"] },
                Required,
                b"The setting's kind",
            ),
            positional(
                b"number",
                b"N",
                Kind::NumberRange { min: 0, max: 4_294_967_295 },
                Required,
                b"The setting's number",
            ),
            flag(
                b"revision",
                b"R",
                Kind::NumberRange { min: 0, max: 4_294_967_295 },
                Optional,
                b"Its value at this revision; the newest when absent",
            ),
        ],
        &[positional(b"kind", b"rules", Kind::Word { set: &[b"rules"] }, Required, b"The deployment's rules")],
    ],
    keyed: Keyed::Never,
    area: 1,
    notes: b"The JSON value is what a settings file's `set` takes.\n",
};
pub const REVISION: Tool = Tool {
    name: b"revision",
    line: b"Read one revision of the settings and its items",
    prints: b"`revision R, against revision A`, when, by whom, from which proposal, whether running work moved, then its items",
    result: b"revision",
    forms: &[&[positional(
        b"revision",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Required,
        b"The revision's number",
    )]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"A forgotten revision prints `revision R is forgotten; the oldest\nkept is revision O`, with exit 0. An item too large to print names\nthe setting call that reads it.\n",
};
pub const REVISIONS: Tool = Tool {
    name: b"revisions",
    line: b"Read the settings' history, newest first, a page at a time",
    prints: b"Each revision's head and its items' edits, then `read on: revisions --before R`, `no older revisions` or `older revisions were forgotten`",
    result: b"revisions",
    forms: &[&[flag(
        b"before",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Optional,
        b"Older than this revision",
    )]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
pub const CHANGE_SETTINGS: Tool = Tool {
    name: b"change-settings",
    line: b"Change the deployment's settings in one decision",
    prints: b"`revision R`, the new revision",
    result: b"change_settings",
    forms: &[&[
        positional(b"items", b"FILE", Kind::File, Required, b"The settings file, JSON, or -"),
        flag(b"against", b"R", Kind::NumberRange { min: 0, max: 4_294_967_295 }, Required, b"The newest revision you read"),
        flag(
            b"running",
            b"stays|moves",
            Kind::Word { set: &[b"stays", b"moves"] },
            Optional,
            b"Running work stays on its revision, or moves to the new one; stays when absent",
        ),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"The file is `{\"items\": [ITEM, ...]}`, at most 1024 items; an item\nis `{\"setting\": {\"kind\": KIND, \"number\": N}, \"set\": VALUE}`, or\n`\"edit\": \"removed\"` or `\"reset\"` in place of `set`; rules takes no\nnumber. A file out of form names the item, member and rule (exit\n1). `stale; newest revision R; read settings again` (exit 2): read\nthe settings, then decide again.\n",
};

/// New rows append to preserve every earlier durable call identity.
pub const PROPOSE_SETTINGS_AT: u8 = 47;
pub const PROPOSAL_AT: u8 = 48;
pub const PROPOSE_SETTINGS: Tool = Tool {
    name: b"propose-settings",
    line: b"Propose a change of the settings to the deployment's owners",
    prints: b"`proposed: pN/K, to RECIPIENT`",
    result: b"propose_settings",
    forms: &[&[
        positional(b"items", b"FILE", Kind::File, Required, b"The settings file, JSON, or -"),
        positional(b"reason", b"REASON", Kind::Text, Optional, b"Why; empty when absent"),
        flag(
            b"against",
            b"R",
            Kind::NumberRange { min: 0, max: 4_294_967_295 },
            Required,
            b"The newest revision you read",
        ),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"The file is change-settings'. Read the proposal with `proposal\npN/K`.\n",
};
pub const PROPOSAL: Tool = Tool {
    name: b"proposal",
    line: b"Read a proposal; for settings, what each item would change",
    prints: b"`proposal P/K` and its action, its reason, and, for settings, each item now and as it would be",
    result: b"proposal",
    forms: &[&[positional(b"proposal", b"P/K", Kind::Proposal, Required, b"The proposal, tN/K or pN/K")]],
    keyed: Keyed::Never,
    area: 2,
    notes: b"A decided proposal prints how it was decided. Decide a live one\nwith `decide`.\n",
};

/// New rows append to preserve every earlier durable call identity.
pub const MOVE_TO_NEWEST_AT: u8 = 51;
pub const MOVE_TO_NEWEST: Tool = Tool {
    name: b"move-to-newest",
    line: b"Move the live tasks on revision R to the newest revision",
    prints: b"`moved N tasks to revision R`, then how many could not, which finish on their revision",
    result: b"move_to_newest",
    forms: &[&[positional(
        b"revision",
        b"R",
        Kind::NumberRange { min: 0, max: 4_294_967_295 },
        Required,
        b"The held revision",
    )]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
