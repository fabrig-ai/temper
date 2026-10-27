//! The binary's table (protocol/README.md, section 4): each command a row,
//! whence the parser's words, `temper --help`, `temper COMMAND --help` and
//! the reference page `commands`.
use crate::{box_command, directories, fake};
use std::fmt::Write;

/// Who runs a command: `temper --help`'s groups, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Documentation,
    Deployment,
    Clients,
    Spawned,
    TestDoubles,
}
impl Who {
    /// "documentation", "deployment", "clients", "spawned", "test doubles".
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Documentation => "documentation",
            Self::Deployment => "deployment",
            Self::Clients => "clients",
            Self::Spawned => "spawned",
            Self::TestDoubles => "test doubles",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    /// After `temper`: `["forge", "run"]`.
    pub words: &'static [&'static str],
    pub who: Who,
    /// Each form's arguments, as its usage shows them after the words.
    pub forms: &'static [&'static str],
    pub line: &'static str,
    pub arguments: &'static [Argument],
    /// The skein settings tables whose flags it takes, printed by its `--help`.
    pub settings: &'static [&'static skein_shell::Settings],
    pub notes: &'static str,
    /// A transition (`forge reconcile`): it goes once its forms do.
    pub transition: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Argument {
    pub shown: &'static str,
    pub line: &'static str,
}

const DIRECTORIES: Argument = Argument {
    shown: "DIRECTORIES",
    line: "[--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.",
};
const FORGE: Argument = Argument { shown: "FORGE", line: "A forge's number in the configuration." };

/// Every row, in `temper --help`'s order: by `Who`, then as listed in protocol/README.md, section 4.
pub const COMMANDS: [Row; 20] = [
    Row {
        words: &["help"],
        who: Who::Documentation,
        forms: &["[NAME | --search WORDS...]"],
        line: "Print the index of temper's pages, one page whole, or the sections that hold every word.",
        arguments: &[
            Argument { shown: "NAME", line: "A page's name, as the index and the pages give it." },
            Argument {
                shown: "WORDS",
                line: "Words that a section must hold every one of, case folded: at most 8, each at most 32 bytes.",
            },
        ],
        settings: &[],
        notes: "Reads no directory and no configuration, so it works before temper init. A name that is no page, or a search that matches nothing, exits 1. A search prints at most 20 hits, a line each, NAME: HEADING: LINE, then \"and more; narrow the words\" when more matched.",
        transition: false,
    },
    Row {
        words: &["init"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES]"],
        line: "Write a deployment that starts, with temper's default configuration, into the three directories.",
        arguments: &[DIRECTORIES],
        settings: &[&directories::SETTINGS],
        notes: "Refused, changing nothing, when the configuration directory already holds temper.json.",
        transition: false,
    },
    Row {
        words: &["serve"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES]"],
        line: "Run the deployment's services, as its configuration's shape names them, until stopped.",
        arguments: &[DIRECTORIES],
        settings: &[&directories::SETTINGS],
        notes: "Its stdout carries only sign-in links, one a line; its stderr, its startup notes and the reason of a failure. A first termination signal settles the engine's forge writes and drains it; a second aborts. Exit 0 after a drained stop, 1 on a refused start or any failure.",
        transition: false,
    },
    Row {
        words: &["box"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] [--echo none|steps]", "[DIRECTORIES] --fake-backends [--bundle BUNDLE]"],
        line: "Run a deployment on this machine as one box: the forge's setup steps, then temper serve, each a child process.",
        arguments: &[
            DIRECTORIES,
            Argument {
                shown: "--echo",
                line: "none prints the box's own lines and a failed step's log; steps prints every step's lines too, prefixed by its name.",
            },
            Argument {
                shown: "--fake-backends",
                line: "Run against temper's fake LLM provider, forge and OAuth issuer, as a deployment of its own under DATA/box/fake-backends that shares the cache.",
            },
            Argument {
                shown: "BUNDLE",
                line: "The web client's bundle directory, in place of the configured one; a relative path is taken from the working directory.",
            },
        ],
        settings: &[&directories::SETTINGS, &box_command::ECHO_SETTINGS],
        notes: "Steps run in order, each restarted when it exits until it has exited too often within its window, when the box gives up, stops the others and exits 1; they stop in reverse order. Its stdout is temper serve's sign-in links, each line whole; its stderr, what --echo chooses; each step logs under the data directory.",
        transition: false,
    },
    Row {
        words: &["program-login"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] NAME"],
        line: "Sign a program source in on this host, through the program's own sign-in.",
        arguments: &[
            DIRECTORIES,
            Argument { shown: "NAME", line: "The program source's name: 1 to 32 of a to z, 0 to 9 and -." },
        ],
        settings: &[&directories::SETTINGS],
        notes: "Checks the configuration and the agent's, makes the source's directory, then replaces itself by the program's sign-in, whose exit status is then its own. The engine need not run.",
        transition: false,
    },
    Row {
        words: &["forge", "fetch"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] FORGE"],
        line: "Download an owned forge's Forgejo and runner binaries into the cache.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&directories::SETTINGS],
        notes: "Fetches only the binaries the forge's setup gives as \"fetched\", at temper's pinned versions, for amd64 or arm64; another machine is refused, naming its architecture.",
        transition: false,
    },
    Row {
        words: &["forge", "prepare"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] FORGE"],
        line: "Prepare an owned forge before its Forgejo first runs.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&directories::SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["forge", "run"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] FORGE forgejo|runner"],
        line: "Run an owned forge's Forgejo or its runner.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&directories::SETTINGS],
        notes: "Checks the binary and the preparation, and takes the forge's lock, first. Run alone, it replaces itself by the program, whose exit status is then its own; given the forge's listener, as the box gives it, it runs the program as its child.",
        transition: false,
    },
    Row {
        words: &["forge", "sign-in"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] FORGE"],
        line: "Make what people need to sign in to temper through a Forgejo, and print the Forgejo command that registers temper.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&directories::SETTINGS],
        notes: "For a Forgejo of application.sign_in.forges, on temper's host, before temper serve. Writes the provider's key and the forge's client secret where they are missing, private, and leaves any already there; prints Forgejo's add-oauth command, never running it.",
        transition: false,
    },
    Row {
        words: &["forge", "reconcile"],
        who: Who::Deployment,
        forms: &["[DIRECTORIES] FORGE"],
        line: "Reconcile a managed or connected Forgejo still in its old form.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&directories::SETTINGS],
        notes: "A forge that starts from its records is refused: temper serve takes its handover.",
        transition: true,
    },
    Row {
        words: &["ctl"],
        who: Who::Clients,
        forms: &[
            "DIR GROUP VERB [ARGUMENT...] [--json]",
            "DIR me|inbox|sign-in|sign-out|pending [ARGUMENT...] [--json]",
            "DIR --call FILE [--json]",
            "help [GROUP [VERB]]",
        ],
        line: "Act in a deployment as one party, one call a process: the agents' command line.",
        arguments: &[
            Argument {
                shown: "DIR",
                line: "ctl's private directory for one deployment and one party: its sign-in, its kept requests and the engine's address.",
            },
            Argument {
                shown: "GROUP",
                line: "A group of tools, a noun of temper's or one of the client's own names; temper ctl help lists them.",
            },
            Argument { shown: "VERB", line: "One of the group's tools; temper ctl help GROUP lists them." },
            Argument {
                shown: "FILE",
                line: "One call as JSON, {\"tool\": \"GROUP VERB\", \"arguments\": {...}}, or - for stdin.",
            },
        ],
        settings: &[],
        notes: "temper ctl help prints how to work through it, then the groups.",
        transition: false,
    },
    Row {
        words: &["open"],
        who: Who::Clients,
        forms: &["DIR LINK_OR_ADDRESS"],
        line: "Open a person's terminal client on a deployment.",
        arguments: &[
            Argument { shown: "DIR", line: "The client's private directory, never one that ctl uses." },
            Argument {
                shown: "LINK_OR_ADDRESS",
                line: "A sign-in link temper serve printed, or the address of an engine the directory is signed in to.",
            },
        ],
        settings: &[],
        notes: "For people; an agent works through temper ctl.",
        transition: false,
    },
    Row {
        words: &["internal", "agent"],
        who: Who::Spawned,
        forms: &["AGENT.json"],
        line: "Run one agent for a worker's run, on smith's agent configuration.",
        arguments: &[Argument {
            shown: "AGENT.json",
            line: "The agent's configuration, as the worker's agent.configuration names it.",
        }],
        settings: &[],
        notes: "",
        transition: false,
    },
    Row {
        words: &["internal", "serve"],
        who: Who::Spawned,
        forms: &["[DIRECTORIES] --box"],
        line: "Run temper serve as the box's step, on the listeners and the pipe the box gives it.",
        arguments: &[DIRECTORIES],
        settings: &[&directories::SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["internal", "forge-child"],
        who: Who::Spawned,
        forms: &["--parent PID --signal kill|terminate -- PROGRAM ARGS..."],
        line: "Run an owned forge's program, sent SIGNAL when its parent PID ends.",
        arguments: &[
            Argument { shown: "PID", line: "The process whose end ends the program." },
            Argument { shown: "PROGRAM", line: "The program to run, with its ARGS." },
        ],
        settings: &[],
        notes: "",
        transition: false,
    },
    Row {
        words: &["fake", "llm"],
        who: Who::TestDoubles,
        forms: &[
            "[DIRECTORIES] [--ready PREFIX] [--seed N] [--observations FILE]",
            "--document FILE [--ready PREFIX] [--seed N] [--observations FILE]",
        ],
        line: "Serve a fake LLM provider: the demo's, from the configuration, or one from a JSON document.",
        arguments: &[DIRECTORIES],
        settings: &[&fake::LLM_SETTINGS, &directories::SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["fake", "forge"],
        who: Who::TestDoubles,
        forms: &[
            "[DIRECTORIES] FORGE [--ready PREFIX] [--seed N] [--observations FILE]",
            "--document FILE [--ready PREFIX] [--seed N] [--observations FILE]",
        ],
        line: "Serve a fake forge: forge FORGE of the configuration, or one from a JSON document.",
        arguments: &[DIRECTORIES, FORGE],
        settings: &[&fake::FORGE_SETTINGS, &directories::SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["fake", "forge", "check"],
        who: Who::TestDoubles,
        forms: &["[DIRECTORIES] FORGE OBSERVATIONS STORE"],
        line: "Judge a fake forge's observations against a stopped deployment's store.",
        arguments: &[
            DIRECTORIES,
            FORGE,
            Argument { shown: "OBSERVATIONS", line: "The observations file the fake forge wrote." },
            Argument { shown: "STORE", line: "A copy of the deployment's store." },
        ],
        settings: &[&directories::SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["fake", "oauth"],
        who: Who::TestDoubles,
        forms: &["--document FILE [--ready PREFIX] [--seed N]"],
        line: "Serve a fake OAuth issuer from a JSON document.",
        arguments: &[],
        settings: &[&fake::OAUTH_SETTINGS],
        notes: "",
        transition: false,
    },
    Row {
        words: &["fake", "git"],
        who: Who::TestDoubles,
        forms: &["ARGS..."],
        line: "Run temper's fake git with git's arguments, as the demo's git does.",
        arguments: &[],
        settings: &[],
        notes: "",
        transition: false,
    },
];

/// The row whose words begin `words`, the longest such.
#[must_use]
pub fn find(words: &[Box<[u8]>]) -> Option<&'static Row> {
    let mut found: Option<&Row> = None;
    for row in &COMMANDS {
        if begins(words, row.words) && found.is_none_or(|before| before.words.len() < row.words.len()) {
            found = Some(row);
        }
    }
    found
}

fn begins(words: &[Box<[u8]>], prefix: &[&str]) -> bool {
    words.len() >= prefix.len() && words.iter().zip(prefix).all(|(word, text)| word.as_ref() == text.as_bytes())
}

/// The command words up to the first word no row continues with.
pub(crate) fn unknown(words: &[Box<[u8]>]) -> String {
    let mut prefix = Vec::new();
    for word in words {
        prefix.push(word.clone());
        if !COMMANDS.iter().any(|row| row.words.len() >= prefix.len() && begins(&prefix, &row.words[..prefix.len()])) {
            break;
        }
    }
    let given = String::from_utf8_lossy(&prefix.join(&b' ')).into_owned();
    let given = if given.is_empty() && words.is_empty() { String::new() } else { format!(" {given}") };
    format!("temper: no command{given}; temper --help lists them\n")
}

pub(crate) fn forms(row: &Row) -> String {
    let mut text = String::new();
    for form in row.forms {
        writeln!(text, "usage: temper {} {form}", row.words.join(" ")).expect("writing into a string");
    }
    text
}

pub(crate) fn refused(row: &Row, why: &str) -> String {
    format!("temper {}: {why}\n{}", row.words.join(" "), forms(row))
}

/// `temper --help`.
#[must_use]
pub fn help() -> String {
    let mut text = String::from(
        "usage: temper COMMAND [ARGUMENT...]\ntemper help prints the index of temper's pages; temper COMMAND --help prints one command.\n",
    );
    let mut previous = None;
    for row in &COMMANDS {
        if previous != Some(row.who) {
            let label = if row.who == Who::Spawned { "spawned, never run by hand" } else { row.who.word() };
            write!(text, "\n{label}:\n").expect("writing into a string");
            previous = Some(row.who);
        }
        for form in row.forms {
            writeln!(text, "  temper {} {form}", row.words.join(" ")).expect("writing into a string");
        }
        if row.who != Who::Spawned {
            let transition = if row.transition { " (a transition)" } else { "" };
            writeln!(text, "    {}{transition}", row.line).expect("writing into a string");
        }
    }
    text
}

/// `temper COMMAND --help`.
#[must_use]
pub fn row(row: &Row) -> String {
    let mut text = forms(row);
    writeln!(text, "{}", row.line).expect("writing into a string");
    for argument in row.arguments {
        writeln!(text, "  {}: {}", argument.shown, argument.line).expect("writing into a string");
    }
    if !row.notes.is_empty() {
        writeln!(text, "{}", row.notes).expect("writing into a string");
    }
    let mut text = text.into_bytes();
    for table in row.settings {
        skein_shell::help(table, &mut text);
    }
    String::from_utf8(text).expect("help is UTF-8")
}

/// The reference page `commands`, whole, as `docs/help/reference/commands.md` holds it.
#[must_use]
pub fn reference() -> String {
    let mut text = String::from(
        "# temper's commands\n\nKind: reference\n\nEvery command of the temper binary, by who runs it: its usage, what it does, its arguments and its notes.\n",
    );
    for row in &COMMANDS {
        let words = row.words.join(" ");
        write!(text, "\n## temper {words}\n").expect("writing into a string");
        for form in row.forms {
            write!(text, "\n`temper {words} {form}`\n").expect("writing into a string");
        }
        write!(text, "\n{}", row.line).expect("writing into a string");
        if row.who == Who::Spawned {
            text.push_str(" Spawned by temper; never run by hand.");
        }
        if row.transition {
            text.push_str(" A transition: it goes once the forms it serves do.");
        }
        text.push('\n');
        if !row.arguments.is_empty() {
            text.push('\n');
            for argument in row.arguments {
                writeln!(text, "- `{}`: {}", argument.shown, argument.line).expect("writing into a string");
            }
        }
        if !row.notes.is_empty() {
            write!(text, "\n{}\n", row.notes).expect("writing into a string");
        }
    }
    text
}
