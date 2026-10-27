//! Compiled pages, paging and search (protocol/documentation.md, sections 2 and 3).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use skein_lib::List;

/// The maximum number of compiled pages.
pub const MAX_PAGES: usize = 96;
/// The largest bound of any kind (a reference's, a guide's).
pub const PAGE_BYTES: usize = 32_768;
/// The maximum bytes in a page name.
pub const NAME_BYTES: usize = 32;
/// The maximum bytes in a title.
pub const TITLE_BYTES: usize = 80;
/// The maximum bytes in a summary.
pub const SUMMARY_BYTES: usize = 200;
/// The maximum bytes in a section heading.
pub const HEADING_BYTES: usize = 80;
/// The maximum bytes in a search hit line.
pub const LINE_BYTES: usize = 120;
/// The maximum number of search words.
pub const WORDS: usize = 8;
/// The maximum bytes in a search word.
pub const WORD_BYTES: usize = 32;

/// A page's template and byte bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Index,
    Concept,
    Walk,
    Recipe,
    Reference,
    Guide,
}
impl Kind {
    /// The word of its kind line: "index", "concept", "walk", "recipe", "reference", "guide".
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Concept => "concept",
            Self::Walk => "walk",
            Self::Recipe => "recipe",
            Self::Reference => "reference",
            Self::Guide => "guide",
        }
    }

    /// Its bound in bytes: 4,096; 8,192; 12,288; 3,072; 32,768; 32,768.
    #[must_use]
    pub fn bound(self) -> usize {
        match self {
            Self::Index => 4_096,
            Self::Concept => 8_192,
            Self::Walk => 12_288,
            Self::Recipe => 3_072,
            Self::Reference | Self::Guide => PAGE_BYTES,
        }
    }
}

/// A compiled page and its head.
#[derive(Debug)]
pub struct Page {
    /// The page's name.
    pub name: &'static str,
    /// The page's template and bound.
    pub kind: Kind,
    /// The title without its Markdown prefix.
    pub title: &'static str,
    /// The summary paragraph joined with spaces.
    pub summary: &'static str,
    /// The whole page, or the selected cut.
    pub text: &'static str,
}

/// `index`, then the concepts, walks, recipes, reference pages and guides, each by name.
#[expect(clippy::disallowed_macros, reason = "pages compile their single Markdown source as static data")]
pub static PAGES: [Page; 31] = [
    Page {
        name: "index",
        kind: Kind::Index,
        title: "temper",
        summary: "temper runs software work for people: agents take tasks on the repositories of a forge and land their changes there, and this page is where an agent starts.",
        text: include_str!("../../../docs/help/index.md"),
    },
    Page {
        name: "budget",
        kind: Kind::Concept,
        title: "Budgets",
        summary: "A budget is what a task may spend, in units its models' prices make of their usage, and by when; each run draws a share of it, and a task without enough is held.",
        text: include_str!("../../../docs/help/budget.md"),
    },
    Page {
        name: "chat",
        kind: Kind::Concept,
        title: "Chats",
        summary: "A chat is a conversation between a party and an agent in a project: an untracked task of the label chat, which replies, parks between messages, and asks for changes that other tasks make.",
        text: include_str!("../../../docs/help/chat.md"),
    },
    Page {
        name: "ctl",
        kind: Kind::Concept,
        title: "temper ctl",
        summary: "temper ctl is the command line through which an agent works in a deployment for a person, each call a read, an act or a wait in a process of its own.",
        text: include_str!("../../../docs/help/ctl.md"),
    },
    Page {
        name: "goal",
        kind: Kind::Concept,
        title: "Goals",
        summary: "A goal is a tracked task: work given a numeric priority, which orders the goals' listing and the work waiting for resources, and whose tree is its plan.",
        text: include_str!("../../../docs/help/goal.md"),
    },
    Page {
        name: "task",
        kind: Kind::Concept,
        title: "Tasks",
        summary: "A task is one numbered piece of work in a project, done by an agent, a procedure or a person, within an authority, toward a result of a declared kind; tasks make trees of delegates.",
        text: include_str!("../../../docs/help/task.md"),
    },
    Page {
        name: "box",
        kind: Kind::Walk,
        title: "Run a local box",
        summary: "Run a whole deployment on one machine, on loopback, from its configuration to a change landed on its own Forgejo, then stop it without losing the model's account.",
        text: include_str!("../../../docs/help/box.md"),
    },
    Page {
        name: "forgejo",
        kind: Kind::Walk,
        title: "Connect a Forgejo",
        summary: "Connect a deployment to a Forgejo it does not run, as its site admin, so that temper makes and keeps its own bot there, then adopt a repository.",
        text: include_str!("../../../docs/help/forgejo.md"),
    },
    Page {
        name: "github",
        kind: Kind::Walk,
        title: "Connect GitHub",
        summary: "Connect a box to GitHub through a GitHub App, accept or make its installation, and adopt one of its repositories, with no browser when the App and its installation exist already.",
        text: include_str!("../../../docs/help/github.md"),
    },
    Page {
        name: "server",
        kind: Kind::Walk,
        title: "Run temper on a server",
        summary: "Run a long-lived deployment on a server: the engine behind a TLS proxy with Google sign-in, an owned Forgejo beside it, the workers on a second host, and a redeploy that keeps the model's accounts.",
        text: include_str!("../../../docs/help/server.md"),
    },
    Page {
        name: "commands",
        kind: Kind::Reference,
        title: "temper's commands",
        summary: "Every command of the temper binary, by who runs it: its usage, what it does, its arguments and its notes.",
        text: include_str!("../../../docs/help/reference/commands.md"),
    },
    Page {
        name: "ctl-sign-in",
        kind: Kind::Reference,
        title: "temper ctl sign-in",
        summary: "Sign in with the link the deployment printed.",
        text: include_str!("../../../docs/help/reference/ctl-sign-in.md"),
    },
    Page {
        name: "ctl-sign-out",
        kind: Kind::Reference,
        title: "temper ctl sign-out",
        summary: "End the sign-in and leave the token empty.",
        text: include_str!("../../../docs/help/reference/ctl-sign-out.md"),
    },
    Page {
        name: "ctl-pending",
        kind: Kind::Reference,
        title: "temper ctl pending",
        summary: "List the requests DIR keeps, without reaching the engine.",
        text: include_str!("../../../docs/help/reference/ctl-pending.md"),
    },
    Page {
        name: "ctl-help",
        kind: Kind::Reference,
        title: "temper ctl help",
        summary: "Read the page ctl and the groups, a group's tools, or one tool's record.",
        text: include_str!("../../../docs/help/reference/ctl-help.md"),
    },
    Page {
        name: "ctl-me",
        kind: Kind::Reference,
        title: "temper ctl me",
        summary: "Read your party: its projects, what it may start, and what waits on it.",
        text: include_str!("../../../docs/help/reference/ctl-me.md"),
    },
    Page {
        name: "ctl-inbox",
        kind: Kind::Reference,
        title: "temper ctl inbox",
        summary: "Read what waits on your party, then its updates.",
        text: include_str!("../../../docs/help/reference/ctl-inbox.md"),
    },
    Page {
        name: "ctl-task",
        kind: Kind::Reference,
        title: "temper ctl task",
        summary: "Tasks: read one and its parts, and act on it.",
        text: include_str!("../../../docs/help/reference/ctl-task.md"),
    },
    Page {
        name: "ctl-chat",
        kind: Kind::Reference,
        title: "temper ctl chat",
        summary: "Chats: start one, read its conversation, close it.",
        text: include_str!("../../../docs/help/reference/ctl-chat.md"),
    },
    Page {
        name: "ctl-proposal",
        kind: Kind::Reference,
        title: "temper ctl proposal",
        summary: "Proposals: read one, decide it.",
        text: include_str!("../../../docs/help/reference/ctl-proposal.md"),
    },
    Page {
        name: "ctl-change",
        kind: Kind::Reference,
        title: "temper ctl change",
        summary: "Changes: read a change's way to landing.",
        text: include_str!("../../../docs/help/reference/ctl-change.md"),
    },
    Page {
        name: "ctl-repository",
        kind: Kind::Reference,
        title: "temper ctl repository",
        summary: "Repositories: adopt one into a project.",
        text: include_str!("../../../docs/help/reference/ctl-repository.md"),
    },
    Page {
        name: "ctl-forge",
        kind: Kind::Reference,
        title: "temper ctl forge",
        summary: "Forges: read them, set one up, connect it, accept an installation, read what it reaches.",
        text: include_str!("../../../docs/help/reference/ctl-forge.md"),
    },
    Page {
        name: "ctl-account",
        kind: Kind::Reference,
        title: "temper ctl account",
        summary: "LLM accounts: read them, connect one, manage its keys and sources, move one in or out.",
        text: include_str!("../../../docs/help/reference/ctl-account.md"),
    },
    Page {
        name: "ctl-model",
        kind: Kind::Reference,
        title: "temper ctl model",
        summary: "Models: read those your party may use.",
        text: include_str!("../../../docs/help/reference/ctl-model.md"),
    },
    Page {
        name: "ctl-settings",
        kind: Kind::Reference,
        title: "temper ctl settings",
        summary: "The settings: read them, change them, or propose a change to their owners.",
        text: include_str!("../../../docs/help/reference/ctl-settings.md"),
    },
    Page {
        name: "ctl-revision",
        kind: Kind::Reference,
        title: "temper ctl revision",
        summary: "Settings revisions: read one or the history, move running work to the newest.",
        text: include_str!("../../../docs/help/reference/ctl-revision.md"),
    },
    Page {
        name: "ctl-project",
        kind: Kind::Reference,
        title: "temper ctl project",
        summary: "Projects: read their periods and pools, set their spend per period.",
        text: include_str!("../../../docs/help/reference/ctl-project.md"),
    },
    Page {
        name: "ctl-playground",
        kind: Kind::Reference,
        title: "temper ctl playground",
        summary: "The playground: start the demo.",
        text: include_str!("../../../docs/help/reference/ctl-playground.md"),
    },
    Page {
        name: "charters",
        kind: Kind::Guide,
        title: "Charters, templates and policies",
        summary: "What decides how temper's agents work: a charter for each run, a template for each new task, and the policies and rules that bound what work may do.",
        text: include_str!("../../../docs/help/charters.md"),
    },
    Page {
        name: "settings",
        kind: Kind::Guide,
        title: "Settings, revisions and proposals",
        summary: "How you read and change temper's settings with temper ctl, what a revision is, how a proposal asks the deployment's owners, and what still needs a restart.",
        text: include_str!("../../../docs/help/settings.md"),
    },
];

/// Find a page by its exact name.
#[must_use]
#[expect(clippy::manual_find, reason = "the programming model uses bounded loops without closures")]
pub fn find(name: &[u8]) -> Option<&'static Page> {
    for entry in &PAGES {
        if entry.name.as_bytes() == name {
            return Some(entry);
        }
    }
    None
}

/// A cut of a page and its continuation offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    /// The whole page, or the selected cut.
    pub text: &'static str,
    /// The next byte offset, absent at the end.
    pub next: Option<u32>,
}

/// Cut a page within a rendered byte budget.
#[must_use]
pub fn page(entry: &Page, from: u32, bytes: u32) -> Cut {
    assert!(bytes >= 2, "a page must fit every rendered byte");
    let mut from = usize::try_from(from).expect("offset fits usize");
    while from < entry.text.len() && !entry.text.is_char_boundary(from) {
        from = from.checked_add(1).expect("entry offset fits usize");
    }
    if from >= entry.text.len() {
        return Cut { text: "", next: None };
    }
    let mut end = from;
    let mut line_end = from;
    let mut rendered = 0_u32;
    for byte in entry.text.as_bytes().get(from..).expect("offset within text") {
        let size = match byte {
            b'"' | b'\\' | b'\n' => 2,
            _ => 1,
        };
        if size > bytes.checked_sub(rendered).expect("page within budget") {
            break;
        }
        rendered = rendered.checked_add(size).expect("page within budget");
        end = end.checked_add(1).expect("entry offset fits usize");
        if *byte == b'\n' {
            line_end = end;
        }
    }
    if end == entry.text.len() {
        return Cut { text: entry.text.get(from..).expect("cut at character boundaries"), next: None };
    }
    if line_end > from {
        end = line_end;
    } else {
        while !entry.text.is_char_boundary(end) {
            end = end.checked_sub(1).expect("cut after start");
        }
    }
    Cut {
        text: entry.text.get(from..end).expect("cut at character boundaries"),
        next: Some(u32::try_from(end).expect("entry offset fits u32")),
    }
}

/// One matching section of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The matching page's name.
    pub page: &'static str,
    /// The section's heading.
    pub heading: &'static str,
    /// The section's first byte offset.
    pub offset: u32,
    /// The first line holding the first search word.
    pub line: &'static str,
}
/// Search hits in table order, with whether more matched.
#[derive(Debug)]
pub struct Hits {
    /// The matching sections.
    pub hits: List<Hit>,
    /// Whether another section matched.
    pub more: bool,
}
/// A refusal of invalid search words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    NoWords,
    TooManyWords,
    WordTooLong,
}
impl Refused {
    /// "nothing to search for", "at most 8 words", "a word is at most 32 bytes".
    #[must_use]
    pub fn says(self) -> &'static str {
        match self {
            Self::NoWords => "nothing to search for",
            Self::TooManyWords => "at most 8 words",
            Self::WordTooLong => "a word is at most 32 bytes",
        }
    }
}

// A section begins outside a fence; its opening heading is part of it.
fn section_end(text: &str, from: usize) -> usize {
    let mut at = from;
    let mut fenced = false;
    for line in text.get(from..).expect("section in entry").split_inclusive('\n') {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if at != from && !fenced && line.starts_with("## ") {
            return at;
        }
        at = at.checked_add(line.len()).expect("entry offset fits usize");
    }
    at
}

fn contains(text: &str, word: &[u8]) -> bool {
    for window in text.as_bytes().windows(word.len()) {
        if window.eq_ignore_ascii_case(word) {
            return true;
        }
    }
    false
}

fn matching_line(section: &'static str, word: &[u8]) -> &'static str {
    for line in section.split('\n') {
        if contains(line, word) {
            let mut end = line.len().min(LINE_BYTES);
            while !line.is_char_boundary(end) {
                end = end.checked_sub(1).expect("cut after start");
            }
            return line.get(..end).expect("cut at character boundaries");
        }
    }
    unreachable!("matching section has a line holding its first word")
}

fn next_word(words: &[u8], mut at: usize) -> Option<(&[u8], usize)> {
    while let Some(byte) = words.get(at) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        at = at.checked_add(1).expect("word offset fits usize");
    }
    let start = at;
    while let Some(byte) = words.get(at) {
        if byte.is_ascii_whitespace() {
            break;
        }
        at = at.checked_add(1).expect("word offset fits usize");
    }
    if at == start { None } else { Some((words.get(start..at).expect("word in input"), at)) }
}

/// Find sections holding every search word, folded on ASCII.
pub fn search(table: &'static [Page], words: &[u8], limit: u32) -> Result<Hits, Refused> {
    let mut asked = List::with_capacity(u32::try_from(WORDS).expect("word count fits u32"));
    let mut at = 0_usize;
    while let Some((word, next)) = next_word(words, at) {
        at = next;
        if asked.len() == u32::try_from(WORDS).expect("word count fits u32") {
            return Err(Refused::TooManyWords);
        }
        if word.len() > WORD_BYTES {
            return Err(Refused::WordTooLong);
        }
        asked.push(word).expect("word count checked");
    }
    let Some(first) = asked.as_slice().first() else { return Err(Refused::NoWords) };
    let mut hits = List::with_capacity(limit);
    for entry in table {
        let mut from = 0_usize;
        while from < entry.text.len() {
            let end = section_end(entry.text, from);
            let section = entry.text.get(from..end).expect("section within entry");
            let mut matches = true;
            for word in asked.as_slice() {
                if !contains(section, word) {
                    matches = false;
                    break;
                }
            }
            if matches {
                if hits.len() == limit {
                    return Ok(Hits { hits, more: true });
                }
                let heading = if from == 0 {
                    entry.title
                } else {
                    section.lines().next().expect("section heading").strip_prefix("## ").expect("section heading")
                };
                hits.push(Hit {
                    page: entry.name,
                    heading,
                    offset: u32::try_from(from).expect("entry offset fits u32"),
                    line: matching_line(section, first),
                })
                .expect("hit count checked");
            }
            from = end;
        }
    }
    Ok(Hits { hits, more: false })
}

#[cfg(test)]
mod tests;
