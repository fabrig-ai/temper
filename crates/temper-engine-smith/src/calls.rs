//! Engine host-tool declarations and whole-object decoding. A malformed call
//! is feedback to Smith's LLM, never a root decision (domain/agent.md, 4.3).

use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, List, Wall, bytes};
use smith_domain_run as run;
use temper_engine_domain as engine;

use crate::{Decoded, Problem, docs, forge, json::Value, nested, notes};

/// A host tool's description, at most (protocol/documentation.md, 8).
#[cfg(test)]
pub(crate) const DESCRIPTION_BYTES: usize = 160;
/// A member's description in a tool's schema, at most, at any depth.
#[cfg(test)]
pub(crate) const MEMBER_BYTES: usize = 120;

pub(crate) struct Spec {
    pub(crate) name: &'static [u8],
    pub(crate) description: &'static [u8],
    pub(crate) schema: &'static [u8],
    pub(crate) effect: run::HostEffect,
}

const ITEMS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"since":{"type":"integer","description":"Only items updated at or after this time, in nanoseconds.","minimum":0,"maximum":18446744073709551615,"default":0},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1},"item_kind":{"type":"string","description":"Issues or pull requests alone; both when absent.","enum":["issue","pull"]}},"required":[]}"#;
const ITEMS_SPEC: Spec = Spec {
    name: b"read_items",
    description:
        b"Read a page of a repository's issues and pull requests, those updated least recently first, from a time on.",
    schema: ITEMS_SCHEMA,
    effect: run::HostEffect::Read,
};
const ITEM_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"number":{"type":"integer","description":"The issue's or pull request's number.","minimum":1,"maximum":18446744073709551615},"after":{"type":"integer","description":"Only the comments after this comment's id.","minimum":0,"maximum":18446744073709551615,"default":0}},"required":["number"]}"#;
const ITEM_SPEC: Spec = Spec {
    name: b"read_item",
    description: b"Read an issue or a pull request with its comments.",
    schema: ITEM_SCHEMA,
    effect: run::HostEffect::Read,
};
const PULL_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"number":{"type":"integer","description":"The pull request's number.","minimum":1,"maximum":18446744073709551615}},"required":["number"]}"#;
const PULL_SPEC: Spec = Spec {
    name: b"read_pull",
    description: b"Read a pull request.",
    schema: PULL_SCHEMA,
    effect: run::HostEffect::Read,
};
const PULL_FOR_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"head":{"type":"string","description":"The branch it merges from."},"base":{"type":"string","description":"The branch it merges into."}},"required":["head","base"]}"#;
const PULL_FOR_SPEC: Spec = Spec {
    name: b"read_pull_for",
    description: b"Find the pull request from one branch into another.",
    schema: PULL_FOR_SCHEMA,
    effect: run::HostEffect::Read,
};
const REVIEWS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"number":{"type":"integer","description":"The pull request's number.","minimum":1,"maximum":18446744073709551615},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1}},"required":["number"]}"#;
const REVIEWS_SPEC: Spec = Spec {
    name: b"read_reviews",
    description: b"Read a page of a pull request's reviews.",
    schema: REVIEWS_SCHEMA,
    effect: run::HostEffect::Read,
};
const STATUSES_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"commit":{"type":"string","description":"The commit's id: 40 or 64 lowercase hexadecimal digits."},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1}},"required":["commit"]}"#;
const STATUSES_SPEC: Spec = Spec {
    name: b"read_statuses",
    description: b"Read a page of the statuses posted on a commit.",
    schema: STATUSES_SCHEMA,
    effect: run::HostEffect::Read,
};
const REMARKS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"number":{"type":"integer","description":"The pull request's number.","minimum":1,"maximum":18446744073709551615},"review":{"type":"integer","description":"The review's id, as read_reviews answers it.","minimum":1,"maximum":18446744073709551615},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1}},"required":["number","review"]}"#;
const REMARKS_SPEC: Spec = Spec {
    name: b"read_remarks",
    description: b"Read a page of one review's remarks on a pull request.",
    schema: REMARKS_SCHEMA,
    effect: run::HostEffect::Read,
};
const BRANCH_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"branch":{"type":"string","description":"The branch's name."}},"required":["branch"]}"#;
const BRANCH_SPEC: Spec = Spec {
    name: b"read_branch",
    description: b"Read a branch's head.",
    schema: BRANCH_SCHEMA,
    effect: run::HostEffect::Read,
};
const BRANCHES_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"prefix":{"type":"string","description":"The names' prefix."}},"required":["prefix"]}"#;
const BRANCHES_SPEC: Spec = Spec {
    name: b"read_branches",
    description: b"List the branches whose names start with a prefix.",
    schema: BRANCHES_SCHEMA,
    effect: run::HostEffect::Read,
};
const PULL_FILES_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"number":{"type":"integer","description":"The pull request's number.","minimum":1,"maximum":18446744073709551615},"head":{"type":"string","description":"The head's commit id: 40 or 64 lowercase hexadecimal digits."},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1}},"required":["number","head"]}"#;
const PULL_FILES_SPEC: Spec = Spec {
    name: b"read_pull_files",
    description: b"Read a page of the files a pull request changes at one head.",
    schema: PULL_FILES_SCHEMA,
    effect: run::HostEffect::Read,
};
const COMPARE_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"before":{"type":"string","description":"The earlier commit's id: 40 or 64 lowercase hexadecimal digits."},"after":{"type":"string","description":"The later commit's id: 40 or 64 lowercase hexadecimal digits."}},"required":["before","after"]}"#;
const COMPARE_SPEC: Spec = Spec {
    name: b"read_compare",
    description: b"Compare two commits.",
    schema: COMPARE_SCHEMA,
    effect: run::HostEffect::Read,
};
const CHECKS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"commit":{"type":"string","description":"The commit's id: 40 or 64 lowercase hexadecimal digits."}},"required":["commit"]}"#;
const CHECKS_SPEC: Spec = Spec {
    name: b"read_checks",
    description: b"Read a commit's combined checks.",
    schema: CHECKS_SCHEMA,
    effect: run::HostEffect::Read,
};
const FILE_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"head":{"type":"string","description":"The commit's id: 40 or 64 lowercase hexadecimal digits."},"path":{"type":"string","description":"The file's path in the repository."},"max_bytes":{"type":"integer","description":"At most this many bytes; the read's limit when absent.","minimum":1,"maximum":4294967295}},"required":["head","path"]}"#;
const FILE_SPEC: Spec = Spec {
    name: b"read_file",
    description: b"Read a file at one head, cut at a byte limit.",
    schema: FILE_SCHEMA,
    effect: run::HostEffect::Read,
};
const JOB_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"head":{"type":"string","description":"The commit's id the CI ran on: 40 or 64 lowercase hexadecimal digits."},"run":{"type":"integer","description":"The CI run's id.","minimum":1,"maximum":18446744073709551615},"job":{"type":"integer","description":"The CI job's id, which may be 0 on Forgejo.","minimum":0,"maximum":18446744073709551615},"attempt":{"type":"integer","description":"The attempt's number, which may be 0 on Forgejo.","minimum":0,"maximum":4294967295},"max_bytes":{"type":"integer","description":"At most this many bytes; the read's limit when absent.","minimum":1,"maximum":4294967295}},"required":["head","run","job","attempt"]}"#;
const JOB_SPEC: Spec = Spec {
    name: b"read_job",
    description: b"Read one CI job's log, cut at a byte limit.",
    schema: JOB_SCHEMA,
    effect: run::HostEffect::Read,
};
const PROTECTION_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"branch":{"type":"string","description":"The branch's name."}},"required":["branch"]}"#;
const PROTECTION_SPEC: Spec = Spec {
    name: b"read_protection",
    description: b"Read a branch's protection.",
    schema: PROTECTION_SCHEMA,
    effect: run::HostEffect::Read,
};
const SETTINGS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."}},"required":[]}"#;
const SETTINGS_SPEC: Spec = Spec {
    name: b"read_settings",
    description: b"Read the repository's settings.",
    schema: SETTINGS_SCHEMA,
    effect: run::HostEffect::Read,
};
const COLLABORATORS_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"page":{"type":"integer","description":"The page.","minimum":1,"maximum":4294967295,"default":1}},"required":[]}"#;
const COLLABORATORS_SPEC: Spec = Spec {
    name: b"read_collaborators",
    description: b"Read a page of the repository's collaborators.",
    schema: COLLABORATORS_SCHEMA,
    effect: run::HostEffect::Read,
};
const PERMISSION_SCHEMA: &[u8] = br#"{"type":"object","additionalProperties":false,"properties":{"repository":{"type":"string","description":"The repository, as owner/name; leave it out, or empty, when the project has one."},"user":{"type":"integer","description":"The forge user's id.","minimum":1,"maximum":18446744073709551615}},"required":["user"]}"#;
const PERMISSION_SPEC: Spec = Spec {
    name: b"read_permission",
    description: b"Read a forge user's permission on the repository.",
    schema: PERMISSION_SCHEMA,
    effect: run::HostEffect::Read,
};

pub(crate) const SPECS: [Spec; 39] = [
    Spec { name: b"goal", description: b"Have one goal in your words tracked and reported; it may plan, ask you and call change. With advisor set, it reads and reports and changes nothing.", schema: br#"{"type":"object","additionalProperties":false,"properties":{"words":{"type":"string"},"advisor":{"type":"boolean"}},"required":["words"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"change", description: b"Have one change written on a branch of a repository of this project, opened as a pull request and merged once its checks pass; you hear its result.", schema: br#"{"type":"object","properties":{"words":{"type":"string","description":"What to change: the files, and the exact text where it matters. It is all the change's coder reads."},"repository":{"type":"string","description":"The repository, as owner/name; leave it out when the project has one you may write."}},"required":["words"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"delegate", description: b"Make a batch of direct delegates within your authority. tracked {priority} makes one a goal, the lower first. For a change to a repository, call change.", schema: br#"{"type":"object","properties":{"batch":{"type":"array","description":"The tasks: each with executor, spec, contract, authority; optionally deadline_seconds, tracked, dependencies, wake, llm.","items":{"type":"object","properties":{"executor":{"type":"object","description":"{kind: agent, charter}, {kind: procedure, connector, code}, {kind: person, person} or {kind: role, role}.","properties":{"kind":{"type":"string","enum":["agent","procedure","person","role"]},"charter":{"type":"integer","minimum":0},"connector":{"type":"integer","minimum":0},"code":{"type":"integer","minimum":0},"person":{"type":"integer","minimum":0},"role":{"type":"integer","minimum":0}},"required":["kind"]},"spec":{"type":"object","properties":{"words":{"type":"string"},"parameters":{"type":"array","items":{"type":"object","description":"{name, kind: number, value}, {name, kind: bytes, value} or {name, kind: resource, connector, resource}.","properties":{"name":{"type":"integer","minimum":0},"kind":{"type":"string","enum":["number","bytes","resource"]},"value":{},"connector":{"type":"integer","minimum":0},"resource":{"type":"integer","minimum":0}},"required":["name","kind"]}},"inputs":{"type":"array","items":{"type":"integer","minimum":0}}},"required":["words"]},"contract":{"type":"object","description":"{kind: report, words}, {kind: change, connector, change_kind, words} or {kind: verdict, choices: [{code, words}]}.","properties":{"kind":{"type":"string","enum":["report","change","verdict"]},"words":{"type":"integer","minimum":0},"connector":{"type":"integer","minimum":0},"change_kind":{"type":"integer","minimum":0},"choices":{"type":"array","items":{"type":"object","properties":{"code":{"type":"integer","minimum":0},"words":{"type":"integer","minimum":0}},"required":["code","words"]}}},"required":["kind"]},"authority":{"type":"object","description":"Whole: restate every member you keep.","properties":{"tools":{"type":"integer","description":"The tool families, as a bit set.","minimum":0},"grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}},"delegation":{"type":"object","properties":{"kinds":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["agent","procedure","role"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"tasks":{"type":"integer","minimum":0},"depth":{"type":"integer","minimum":0}},"required":["kinds","tasks","depth"]},"budget":{"type":"object","properties":{"spend":{"type":"integer","minimum":0},"deadline":{"type":"integer","description":"Wall-clock nanoseconds since the Unix epoch.","minimum":0}},"required":["spend"]},"notes":{"type":"integer","minimum":0,"maximum":255},"note_resources":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"pattern":{"type":"object","properties":{"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["segments","terminal","last"]}},"required":["connector","pattern"]}}},"required":["tools","grants","delegation","budget","notes"]},"deadline_seconds":{"type":"integer","description":"It must end this many seconds after it is made. Give this or authority.budget.deadline; with neither, it ends with you.","minimum":1},"tracked":{"type":"object","description":"Track it as a goal at this priority, the lower first. Leave it out for an untracked task.","properties":{"priority":{"type":"integer","minimum":0,"maximum":4294967295}},"required":["priority"]},"dependencies":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["batch","existing"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"wake":{"type":"object","properties":{"words":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"notices":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"news":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"results":{"type":"string","enum":["never","each","last_or_failure"]},"questions":{"type":"boolean"},"answers":{"type":"boolean"},"timers":{"type":"boolean"}},"required":["words","notices","news","results","questions","answers","timers"]},"llm":{"type":"object","description":"main {model, effort} names the model, effort optional; subagents and account are optional.","properties":{"main":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]},"subagents":{"type":"array","items":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]}},"account":{"type":"integer","minimum":0}},"required":["main"]},"symbolic_grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}}},"required":["executor","spec","contract","authority"]}}},"required":["batch"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"amend", description: b"Change a live delegate within your authority, naming only what changes and the spec revision you read; authority and instructions are whole.", schema: br#"{"type":"object","examples":[{"target":3,"revision":1,"amendment":{"reason":"Finish by Friday.","authority":{"tools":5,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":1000,"deadline":1793404800000000000},"notes":0}}}],"properties":{"target":{"type":"integer","description":"The delegate's number.","minimum":1},"revision":{"type":"integer","description":"The spec revision you last read. Leave it out only to change a tracked delegate's priority alone.","minimum":0},"amendment":{"type":"object","description":"The reason, which its live run hears, and what changes: tracked, instructions, spec, authority, dependencies, wake, llm.","properties":{"reason":{"type":"string","description":"What the delegate's live run hears, unless new instructions replace it."},"tracked":{"description":"{priority} tracks it as a goal, the lower first; untracked stops tracking it.","oneOf":[{"type":"object","properties":{"priority":{"type":"integer","minimum":0,"maximum":4294967295}},"required":["priority"]},{"type":"string","enum":["untracked"]}]},"instructions":{"type":"string","description":"Its own instructions, whole; empty leaves it none."},"spec":{"type":"object","properties":{"words":{"type":"string"},"parameters":{"type":"array","items":{"type":"object","description":"{name, kind: number, value}, {name, kind: bytes, value} or {name, kind: resource, connector, resource}.","properties":{"name":{"type":"integer","minimum":0},"kind":{"type":"string","enum":["number","bytes","resource"]},"value":{},"connector":{"type":"integer","minimum":0},"resource":{"type":"integer","minimum":0}},"required":["name","kind"]}},"inputs":{"type":"array","items":{"type":"integer","minimum":0}}},"required":["words"]},"authority":{"type":"object","description":"Whole: restate every member you keep. A deadline is budget.deadline.","properties":{"tools":{"type":"integer","description":"The tool families, as a bit set.","minimum":0},"grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}},"delegation":{"type":"object","properties":{"kinds":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["agent","procedure","role"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"tasks":{"type":"integer","minimum":0},"depth":{"type":"integer","minimum":0}},"required":["kinds","tasks","depth"]},"budget":{"type":"object","properties":{"spend":{"type":"integer","minimum":0},"deadline":{"type":"integer","description":"Wall-clock nanoseconds since the Unix epoch.","minimum":0}},"required":["spend"]},"notes":{"type":"integer","minimum":0,"maximum":255},"note_resources":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"pattern":{"type":"object","properties":{"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["segments","terminal","last"]}},"required":["connector","pattern"]}}},"required":["tools","grants","delegation","budget","notes"]},"dependencies":{"type":"array","description":"The whole list of the existing tasks it waits on.","items":{"type":"integer","minimum":1}},"wake":{"type":"object","properties":{"words":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"notices":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"news":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","description":"Nanoseconds.","minimum":0}},"required":["kind"]},"results":{"type":"string","enum":["never","each","last_or_failure"]},"questions":{"type":"boolean"},"answers":{"type":"boolean"},"timers":{"type":"boolean"}},"required":["words","notices","news","results","questions","answers","timers"]},"llm":{"type":"object","description":"main {model, effort} names the model, effort optional; subagents and account are optional.","properties":{"main":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]},"subagents":{"type":"array","items":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]}},"account":{"type":"integer","minimum":0}},"required":["main"]}},"required":["reason"]}},"required":["target","amendment"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"message", description: b"Send words, a question, or the answer to a question, to a task you reference. Its run reads it at its next turn.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","description":"The task's number.","minimum":1},"form":{"type":"string","description":"words; a question, which its run may answer; or an answer, naming the question.","enum":["words","question","answer"]},"words":{"type":"string","description":"What you say."},"question":{"type":"integer","description":"For an answer: the question's number.","minimum":1}},"required":["target","form","words"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"cancel", description: b"Cancel a live delegate and every task below it, with a reason.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","description":"The delegate's number.","minimum":1},"reason":{"type":"string","description":"Why it is cancelled."}},"required":["target","reason"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"release", description: b"Release a held delegate, naming its hold revision. One a party's hold holds is theirs to release: propose it.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","description":"The held delegate's number.","minimum":1},"revision":{"type":"integer","description":"The hold's revision, as you last read it.","minimum":0},"again":{"type":"boolean","description":"For an effect held uncertain alone: true makes it again, false does not."}},"required":["target","revision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"introduce", description: b"Give two tasks you reference a reference to each other, so that each can message the other.", schema: br#"{"type":"object","properties":{"left":{"type":"integer","description":"One task's number.","minimum":1},"right":{"type":"integer","description":"The other task's number.","minimum":1}},"required":["left","right"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"subscribe", description: b"Hear of a task you reference, held or ended, or of a timer, once or periodic. Answers the subscription's number.", schema: br#"{"type":"object","properties":{"kind":{"type":"string","description":"task, to hear of a task; timer, to hear at a time.","enum":["task","timer"]},"target":{"type":"integer","description":"For task: the task's number.","minimum":1},"held":{"type":"boolean","description":"For task: true to hear when it is held; false when absent."},"result":{"type":"boolean","description":"For task: true to hear its result; true when absent."},"at":{"type":"integer","description":"For timer: when, in wall-clock nanoseconds since the Unix epoch.","minimum":0},"period":{"type":"integer","description":"For timer: again every this many nanoseconds; once when absent.","minimum":1}},"required":["kind"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"unsubscribe", description: b"Remove one of your subscriptions.", schema: br#"{"type":"object","properties":{"subscription":{"type":"integer","description":"The subscription's number, as subscribe answered it.","minimum":1}},"required":["subscription"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"decide", description: b"Decide a proposal waiting for you: accept it, reject it with a reason, or pass it up to the holder above.", schema: br#"{"type":"object","properties":{"proposer":{"type":"integer","description":"The proposing task's number.","minimum":1},"proposal":{"type":"integer","description":"The proposal's number among its proposer's.","minimum":1},"decision":{"type":"string","description":"accept, reject or pass.","enum":["accept","reject","pass"]},"reason":{"type":"string","description":"Why; required to reject."}},"required":["proposer","proposal","decision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"decide_escalation", description: b"Decide a held descendant's escalation: release it, reject it with a reason, or pass it up to the holder above.", schema: br#"{"type":"object","properties":{"task":{"type":"integer","description":"The held task's number.","minimum":1},"revision":{"type":"integer","description":"Its hold's revision, as you last read it.","minimum":1},"decision":{"type":"string","description":"release, reject or pass.","enum":["release","reject","pass"]},"reason":{"type":"string","description":"Why; required to reject."}},"required":["task","revision","decision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"propose", description: b"Ask the covering holder to act beyond your authority. To release a task a party's hold holds, give task, hold and the hold's revision: it waits for the hold's owner.", schema: br#"{"type":"object","properties":{"action":{"type":"string","description":"The act; each takes the members named for it below.","enum":["batch","effect","amend","widen","release"]},"task":{"type":"integer","description":"For amend, widen and release: the task's number.","minimum":1},"revision":{"type":"integer","description":"For amend: its spec revision, as you last read it.","minimum":0},"hold":{"type":"integer","minimum":1},"batch":{"type":"array","description":"For batch: the tasks, as delegate's batch.","items":{"type":"object"}},"amendment":{"type":"object","description":"For amend: what changes, as amend's amendment."},"authority":{"type":"object","description":"For widen: the authority wanted, whole."},"reason":{"type":"string","description":"Why; the holder reads it."},"as_holder":{"type":"boolean","description":"For batch: true makes the tasks the accepter's delegates rather than yours."},"repository":{"type":"object","description":"For effect: as effect_forge's repository."},"resource":{"type":"object","description":"For effect: as effect_forge's resource."},"write":{"type":"object","description":"For effect: as effect_forge's write."}},"required":["action","reason"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"withdraw", description: b"Withdraw one of your pending proposals.", schema: br#"{"type":"object","properties":{"proposal":{"type":"integer","description":"The proposal's number, as propose answered it.","minimum":1}},"required":["proposal"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"note", description: b"Write a note in a scope for later runs to recall, or revise one, naming the revision you recalled.", schema: br#"{"type":"object","properties":{"scope":{"type":"object","description":"{kind}: deployment; project, with project; goal, with project and goal; resources, with project, connector, pattern."},"description":{"type":"string","description":"What the note is about: a search reads it."},"body":{"type":"string","description":"The note's text."},"references":{"type":"array","description":"The numbers of the tasks it refers to.","items":{"type":"integer","minimum":1}},"name":{"type":"integer","description":"To revise a note: its name. Leave it out for a new note.","minimum":1},"recalled":{"type":"integer","description":"To revise a note: its revision, as recall answered it.","minimum":1}},"required":["scope","description","body"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"recall", description: b"Recall a note by its name, or search the descriptions of the notes in some scopes, a page at a time.", schema: br#"{"type":"object","properties":{"kind":{"type":"string","description":"name, by its name; search, by words in scopes.","enum":["name","search"]},"name":{"type":"integer","description":"For name: the note's name.","minimum":1},"scopes":{"type":"array","description":"For search: the scopes, each as note's scope.","items":{"type":"object"}},"query":{"type":"string","description":"For search: the words."},"page":{"type":"integer","description":"The page, from 0; 0 when absent.","minimum":0}},"required":["kind"]}"#, effect: run::HostEffect::Read },
    Spec { name: b"subscribe_forge", description: b"Hear forge news: landings on a branch, CI on a head, a pull request's state, or the comments and reviews on an issue or pull request.", schema: br#"{"type":"object","properties":{"topic":{"type":"object","description":"{kind, repository: {forge, repository}}: landings with branch, ci with head, pull or participation with number."},"own_change":{"type":"integer","description":"For landings: your change task's number; its own landing does not wake you.","minimum":1},"paths":{"type":"array","description":"For landings: a landing wakes you when it changes one of these paths, or a file below one.","items":{"type":"string"}}},"required":["topic"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"effect_forge", description: b"Make one write on an adopted repository through the forge's connector: an issue, a comment or a status, among the forge's writes.", schema: br#"{"type":"object","properties":{"repository":{"type":"object","description":"The repository, as {forge, repository}: the forge's number and the repository's."},"resource":{"type":"object","description":"What it writes on: {kind: repository}, {kind: pull, number}, {kind: issue, number} or {kind: branch, segments}."},"write":{"type":"object","description":"{kind: create_issue, key, title, body}, {kind: post, number, key, body} or {kind: status, commit, context, check}."}},"required":["repository","resource","write"]}"#, effect: run::HostEffect::Write },
    Spec {
        name: b"settings", description: b"Read the deployment's settings: the inventory with no member, or a setting, a revision's record or a page of the history; a long answer goes on from an offset.",
        schema: br#"{"type":"object","additionalProperties":false,"properties":{"setting":{"type":"object","additionalProperties":false,"properties":{"kind":{"enum":["charter","template","policy","rules"],"description":"The setting's kind."},"number":{"type":"integer","minimum":1,"maximum":4294967295,"description":"Its number; the rules have none."}},"required":["kind"],"description":"One setting's whole value, named as a settings file names it."},"revision":{"type":"integer","minimum":1,"maximum":4294967295,"description":"Alone, that revision's record: its items and who decided it. With a charter, the charter as that held revision has it."},"history":{"type":"object","additionalProperties":false,"properties":{"before":{"type":"integer","minimum":1,"maximum":4294967295,"description":"The page before this revision; the newest page when absent."}},"description":"A page of the revisions, the newest first."},"from":{"type":"integer","minimum":0,"maximum":4294967295,"description":"Where to go on in the answer's text: the offset after next: on its last line; 0 when absent."}}}"#,
        effect: run::HostEffect::Read,
    },
    Spec {
        name: b"propose_settings", description: b"Propose a change of the deployment's settings to its owners, with a reason. You hear in your messages whether it was accepted, rejected or went stale.",
        schema: br#"{"type":"object","additionalProperties":false,"properties":{"against":{"type":"integer","minimum":1,"maximum":4294967295,"description":"The newest revision you read. A proposal against an older one is stale, and you hear the newest."},"reason":{"type":"string","description":"Why; the owners read it."},"items":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"setting":{"type":"object","properties":{"kind":{"enum":["charter","template","policy","rules"],"description":"The setting's kind."},"number":{"type":"integer","minimum":1,"description":"Its number; the rules have none."}},"required":["kind"],"description":"The setting, named as a settings file names it."},"set":{"type":"object","description":"Its whole new value, one JSON object as a settings file holds it; read the setting first."},"edit":{"enum":["reset","removed"],"description":"In place of set: reset, to follow its default again, or removed, no longer offered."}},"required":["setting"]},"description":"The changes, at most one per setting."}},"required":["against","items","reason"]}"#,
        effect: run::HostEffect::Write,
    },
    ITEMS_SPEC,
    ITEM_SPEC,
    PULL_SPEC,
    PULL_FOR_SPEC,
    REVIEWS_SPEC,
    STATUSES_SPEC,
    REMARKS_SPEC,
    BRANCH_SPEC,
    BRANCHES_SPEC,
    PULL_FILES_SPEC,
    COMPARE_SPEC,
    CHECKS_SPEC,
    FILE_SPEC,
    JOB_SPEC,
    PROTECTION_SPEC,
    SETTINGS_SPEC,
    COLLABORATORS_SPEC,
    PERMISSION_SPEC,
    Spec { name: b"read_docs", description: b"Read temper's pages: {topic, from} gives a page from an offset, then next: OFFSET or end; {search} the sections holding every word, a line each.", schema: br#"{"type":"object","properties":{"topic":{"type":"string","description":"A page's name, such as index."},"from":{"type":"integer","description":"The offset to read from, as next: gave it; 0 when absent.","minimum":0},"search":{"type":"string","description":"The words a section must hold, every one: at most 8."}}}"#, effect: run::HostEffect::Read },
];

/// Host declarations offered to Smith with exact schemas and write effects.
/// Root authority still decides each accepted call by its durable name.
#[must_use]
pub fn tools(timeout: Duration) -> Box<[run::HostTool]> {
    let mut tools = List::with_capacity(u32::try_from(SPECS.len()).expect("fixed tool list"));
    for spec in SPECS {
        tools
            .push(run::HostTool {
                name: bytes::copy_of(spec.name),
                description: bytes::copy_of(spec.description),
                schema: bytes::copy_of(spec.schema),
                effect: spec.effect,
                timeout,
            })
            .expect("fixed tool list");
    }
    tools.into_boxed()
}

/// Decode an attested Smith host call under one activation-qualified name.
/// The caller supplies its task/attempt envelope separately to the root.
pub fn call(name: run::CallName, tool: &[u8], input: &run::HostInput) -> Result<Decoded, Problem> {
    call_configured(name, tool, input, &[])
}

/// Decode host calls with the deployment's models for settings values.
#[expect(clippy::too_many_lines, reason = "one exhaustive finite host-tool registry")]
pub fn call_configured(
    name: run::CallName,
    tool: &[u8],
    input: &run::HostInput,
    models: &[engine::Model],
) -> Result<Decoded, Problem> {
    if tool == b"propose_settings" {
        return crate::settings::proposal(name, input.bytes(), models);
    }
    let value = crate::json::parse(input.bytes())?;
    let tool = match tool {
        b"settings" => crate::settings::read(&value)?,
        b"read_docs" => {
            let answer = match docs::read(&value)? {
                Ok(read) => docs::answer(&temper_docs::PAGES, &read),
                Err(answer) => answer,
            };
            return Ok(Decoded::Answered(answer));
        }
        b"read_items"
        | b"read_item"
        | b"read_pull"
        | b"read_pull_for"
        | b"read_reviews"
        | b"read_statuses"
        | b"read_remarks"
        | b"read_branch"
        | b"read_branches"
        | b"read_pull_files"
        | b"read_compare"
        | b"read_checks"
        | b"read_file"
        | b"read_job"
        | b"read_protection"
        | b"read_settings"
        | b"read_collaborators"
        | b"read_permission" => engine::Tool::ReadNamed {
            repository: match value.field(b"repository")? {
                Some(value) => {
                    let name = value.text(b"repository")?;
                    if name.is_empty() { None } else { Some(name) }
                }
                None => None,
            },
            read: forge::read_named(tool, &value)?,
        },
        b"effect_forge" => engine::Tool::EffectForge {
            repository: forge::repository(value.required(b"repository")?)?,
            resource: forge::resource(value.required(b"resource")?)?,
            write: Box::new(forge::write(value.required(b"write")?)?),
        },
        b"subscribe_forge" => engine::Tool::SubscribeForge {
            topic: forge::topic(value.required(b"topic")?)?,
            own_change: match value.field(b"own_change")? {
                Some(change) => Some(change.number(b"own_change")?),
                None => None,
            },
            paths: match value.field(b"paths")? {
                Some(paths) => forge::texts(paths, b"paths")?,
                None => Box::new([]),
            },
        },
        b"goal" => {
            let words = value.required(b"words")?.text(b"words")?;
            if words.is_empty() {
                return Err(Problem::Range(b"words"));
            }
            engine::Tool::Goal { words, advisor: optional_bool(&value, b"advisor", false)? }
        }
        b"change" => {
            let words = value.required(b"words")?.text(b"words")?;
            if words.is_empty() {
                return Err(Problem::Range(b"words"));
            }
            let repository = match value.field(b"repository")? {
                Some(name) => {
                    let name = name.text(b"repository")?;
                    let mut slash = None;
                    for (at, byte) in name.iter().enumerate() {
                        if *byte == b'/' {
                            if slash.is_some() {
                                return Err(Problem::Range(b"repository"));
                            }
                            slash = Some(at);
                        }
                    }
                    let slash = slash.ok_or(Problem::Range(b"repository"))?;
                    if slash == 0 || slash.checked_add(1) == Some(name.len()) {
                        return Err(Problem::Range(b"repository"));
                    }
                    Some(name)
                }
                None => None,
            };
            engine::Tool::Change { words, repository }
        }
        b"delegate" => engine::Tool::Delegate { batch: nested::delegates(value.required(b"batch")?)? },
        b"amend" => {
            let amendment = nested::amendment(value.required(b"amendment")?)?;
            let priority = match amendment.tracked {
                Some(tasks::Tracking::Tracked { .. }) => true,
                Some(tasks::Tracking::Untracked) | None => false,
            };
            let revision = match value.field(b"revision")? {
                Some(value) => value.number(b"revision")?,
                None if priority
                    && amendment.spec.is_none()
                    && amendment.wake.is_none()
                    && amendment.dependencies.is_none()
                    && amendment.authority.is_none()
                    && amendment.instructions.is_none()
                    && amendment.llm.is_none() =>
                {
                    0
                }
                None => return Err(Problem::Missing(b"revision")),
            };
            engine::Tool::Amend { target: positive(&value, b"target")?, revision, amendment }
        }
        b"message" => {
            let target = positive(&value, b"target")?;
            let words = value.required(b"words")?.text(b"words")?;
            let form = value.required(b"form")?.text(b"form")?;
            let form = match form.as_ref() {
                b"words" => engine::MessageForm::Words,
                b"question" => engine::MessageForm::Question,
                b"answer" => engine::MessageForm::Answer { question: positive(&value, b"question")? },
                _ => return Err(Problem::Range(b"form")),
            };
            engine::Tool::Message { target, form, words }
        }
        b"cancel" => engine::Tool::Cancel {
            target: positive(&value, b"target")?,
            reason: value.required(b"reason")?.text(b"reason")?,
        },
        b"release" => engine::Tool::Release {
            target: positive(&value, b"target")?,
            revision: number(&value, b"revision")?,
            again: match value.field(b"again")? {
                Some(again) => Some(again.boolean(b"again")?),
                None => None,
            },
        },
        b"introduce" => {
            engine::Tool::Introduce { left: positive(&value, b"left")?, right: positive(&value, b"right")? }
        }
        b"unsubscribe" => engine::Tool::Unsubscribe { subscription: positive(&value, b"subscription")? },
        b"withdraw" => engine::Tool::Withdraw { proposal: positive(&value, b"proposal")? },
        b"decide_escalation" => {
            let decision = value.required(b"decision")?.text(b"decision")?;
            let decision = match decision.as_ref() {
                b"release" => engine::EscalationChoice::Release,
                b"reject" => engine::EscalationChoice::Reject { reason: value.required(b"reason")?.text(b"reason")? },
                b"pass" => engine::EscalationChoice::Pass,
                _ => return Err(Problem::Range(b"decision")),
            };
            engine::Tool::DecideEscalation {
                task: positive(&value, b"task")?,
                revision: positive(&value, b"revision")?,
                decision,
            }
        }
        b"decide" => {
            let decision = value.required(b"decision")?.text(b"decision")?;
            let decision = match decision.as_ref() {
                b"accept" => engine::ProposalChoice::Accept,
                b"reject" => engine::ProposalChoice::Reject { reason: value.required(b"reason")?.text(b"reason")? },
                b"pass" => engine::ProposalChoice::Pass,
                _ => return Err(Problem::Range(b"decision")),
            };
            engine::Tool::Decide {
                proposer: positive(&value, b"proposer")?,
                proposal: positive(&value, b"proposal")?,
                decision,
            }
        }
        b"subscribe" => {
            let kind = value.required(b"kind")?.text(b"kind")?;
            let kind = match kind.as_ref() {
                b"task" => tasks::SubscriptionKind::Task {
                    target: positive(&value, b"target")?,
                    held: optional_bool(&value, b"held", false)?,
                    result: optional_bool(&value, b"result", true)?,
                },
                b"timer" => tasks::SubscriptionKind::Timer {
                    at: Wall::from_nanos(number(&value, b"at")?),
                    period: match value.field(b"period")? {
                        Some(period) => Some(Duration::from_nanos(period.number(b"period")?)),
                        None => None,
                    },
                },
                _ => return Err(Problem::Range(b"kind")),
            };
            engine::Tool::Subscribe { kind }
        }
        b"propose" => {
            let action = value.required(b"action")?.text(b"action")?;
            let action = match action.as_ref() {
                b"effect" => engine::ProposedAction::Effect {
                    repository: forge::repository(value.required(b"repository")?)?,
                    resource: forge::resource(value.required(b"resource")?)?,
                    write: Box::new(forge::write(value.required(b"write")?)?),
                },
                b"release" => engine::ProposedAction::Release {
                    task: positive(&value, b"task")?,
                    hold: positive(&value, b"hold")?,
                    revision: positive(&value, b"revision")?,
                },
                b"batch" => engine::ProposedAction::Batch(nested::delegates(value.required(b"batch")?)?),
                b"amend" => engine::ProposedAction::Amend {
                    task: positive(&value, b"task")?,
                    revision: number(&value, b"revision")?,
                    amendment: nested::amendment(value.required(b"amendment")?)?,
                },
                b"widen" => engine::ProposedAction::Widen {
                    task: positive(&value, b"task")?,
                    authority: nested::authority(value.required(b"authority")?)?,
                },
                _ => return Err(Problem::Range(b"action")),
            };
            engine::Tool::Propose {
                action,
                reason: value.required(b"reason")?.text(b"reason")?,
                as_holder: optional_bool(&value, b"as_holder", false)?,
            }
        }
        b"note" => {
            let (entry, recalled) = notes::new(&value)?;
            engine::Tool::Note { entry, recalled }
        }
        b"recall" => {
            let (by, page) = notes::recall(&value)?;
            engine::Tool::Recall { by, page }
        }
        _ => return Err(Problem::UnknownTool),
    };
    Ok(Decoded::Call(engine::Call { completion: name.completion, position: name.position, tool }))
}

fn number(value: &Value, field: &'static [u8]) -> Result<u64, Problem> {
    value.required(field)?.number(field)
}

fn positive(value: &Value, field: &'static [u8]) -> Result<u64, Problem> {
    let number = number(value, field)?;
    if number == 0 { Err(Problem::Range(field)) } else { Ok(number) }
}

fn optional_bool(value: &Value, field: &'static [u8], default: bool) -> Result<bool, Problem> {
    match value.field(field)? {
        Some(value) => value.boolean(field),
        None => Ok(default),
    }
}
