//! Reference-case checkpoints: effects over observations any world can supply.
use super::{
    A, A_REPORT, Agent, DESIGN, DESIGN_DOC, DESIGN_REPORT, DESIGN_WORDS, DIRECTOR, DIRECTOR_REPORT, DRAFT, DRAFT_WORDS,
    GREEN_LIGHT, OWNER_EFFORT, OWNER_MODEL, PROJECT, QUESTION, acts::Read, draft_commit, kit_tree,
};
use crate::{Exit, Ran, calls};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone)]
pub struct Observed {
    /// ctl's whole JSON document, by the act that read it; absent if that act failed.
    pub documents: BTreeMap<Read, Value>,
    /// Each LLM query in order, by its cue: its messages' texts and tool outputs.
    pub told: Vec<Told>,
    /// Command verbs of the submitted ctl argv (reads and waits included).
    pub commands: Vec<String>,
    /// kit's main at its last merged head.
    pub kit: Kit,
}
#[derive(Clone)]
pub struct Told {
    /// Ordered query messages, preserving roles and tool-call identities across a retry.
    pub history: Box<[skein_fake_llm_domain::api::Message]>,
    pub agent: Agent,
    pub words: Box<[u8]>,
    pub model: Box<[u8]>,
    pub credential: Option<u32>,
}
#[derive(Clone)]
pub struct Kit {
    pub main: BTreeMap<Vec<u8>, Vec<u8>>,
    pub merged_green: bool,
}
impl Observed {
    #[must_use]
    pub fn from_ran(ran: &Ran, reads: &[(Read, usize)]) -> Self {
        let mut documents = BTreeMap::new();
        for (read, position) in reads {
            let Some(call) = ran.calls().get(*position) else { continue };
            if call.exit != Some(Exit::Code(0)) {
                continue;
            }
            if let Ok(document) = serde_json::from_slice::<Value>(&call.stdout) {
                documents.insert(*read, document);
            }
        }
        let mut told = Vec::new();
        for observation in ran.llm() {
            let skein_fake_peers::llm::Observation::Query { query, .. } = observation else { continue };
            let Some(agent) = cued(&query.system) else { continue };
            let mut words = query.system.to_vec();
            words.push(b'\n');
            for message in &query.messages {
                for part in &message.parts {
                    use skein_fake_llm_domain::api::Part;
                    match part {
                        Part::Text { text } | Part::ToolOutput { output: text, .. } => {
                            words.extend_from_slice(text);
                            words.push(b'\n');
                        }
                        Part::Opaque { .. } | Part::ToolCall { .. } => {}
                    }
                }
            }
            told.push(Told {
                history: query.messages.clone(),
                agent,
                words: words.into_boxed_slice(),
                model: query.model.clone(),
                credential: query.credential,
            });
        }
        Self {
            documents,
            commands: Vec::new(),
            told,
            kit: Kit { main: ran.forge().main().clone(), merged_green: ran.forge().merged_green() },
        }
    }
}
impl Observed {
    /// The command verbs of the story's ctl argv, in order, for checks on which acts it made.
    #[must_use]
    pub fn with_commands(mut self, steps: &[calls::Step]) -> Self {
        for step in steps {
            if let calls::Step::Call { argv, .. } | calls::Step::Start { argv, .. } | calls::Step::Until { argv, .. } =
                step
                && let Some(calls::Arg::Lit(command)) = argv.0.first()
            {
                self.commands.push((*command).into());
            }
        }
        self
    }
}
fn cued(system: &[u8]) -> Option<Agent> {
    let mut first_at = usize::MAX;
    let mut first = None;
    for agent in [
        Agent::Director,
        Agent::A,
        Agent::Design,
        Agent::Review,
        Agent::Survey,
        Agent::Check,
        Agent::B,
        Agent::Benchmark,
        Agent::Hang,
        Agent::Phase,
    ] {
        for (at, word) in system.windows(agent.cue().len()).enumerate() {
            if word == agent.cue() {
                if at < first_at {
                    first_at = at;
                    first = Some(agent);
                }
                break;
            }
        }
    }
    first
}

pub struct Miss {
    pub step: &'static str,
    pub promise: Promise,
    pub what: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Promise {
    Must(&'static str),
    Add(&'static str),
    Step(&'static str),
}
impl fmt::Display for Miss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, words) = match self.promise {
            Promise::Must(words) => ("Must", words),
            Promise::Add(words) => ("Add", words),
            Promise::Step(words) => ("Step", words),
        };
        write!(f, "{} {kind} \"{words}\": {}", self.step, self.what)
    }
}
const DELEGATE: Promise =
    Promise::Must("delegate a goal with instructions, input, priority, host kind, pool and models (exact)");
const AUTHORITY: Promise = Promise::Add("authority bounds A's subtree");
const SPEND: Promise = Promise::Add("spend rolls up to the goal");
const QUESTION_ANSWER: Promise = Promise::Must("a question and its answer (exact)");
const MARKDOWN: Promise = Promise::Must("the fast path for Markdown alone (exact)");
fn check(misses: &mut Vec<Miss>, met: bool, promise: Promise, what: String) {
    if !met {
        misses.push(Miss { step: "4.1", promise, what });
    }
}
fn text(value: &Value, path: &str) -> String {
    value.pointer(path).and_then(Value::as_str).unwrap_or_default().into()
}
fn number(value: &Value, path: &str) -> Option<u64> {
    value.pointer(path).and_then(Value::as_u64)
}
fn contains(words: &[u8], needle: &str) -> bool {
    crate::calls::occurrences(words, needle.as_bytes()) != 0
}

#[must_use]
pub fn step_4_1(observed: &Observed) -> Vec<Miss> {
    let mut misses = Vec::new();
    let mut docs = Vec::new();
    for (read, expected) in [(Read::DirectorEnded, DIRECTOR), (Read::AEnded, A), (Read::DesignEnded, DESIGN)] {
        let document = &observed.documents.get(&read).unwrap_or(&Value::Null)["task"];
        check(
            &mut misses,
            number(document, "/chip/task") == Some(expected),
            DELEGATE,
            format!("{read:?}: task number must be {expected}"),
        );
        check(
            &mut misses,
            number(document, "/project") == Some(PROJECT),
            DELEGATE,
            format!("{read:?}: project must be {PROJECT}"),
        );
        docs.push(document);
    }
    let [director, a, design] = docs.as_slice() else { unreachable!("three reads") };
    check(&mut misses, number(a, "/priority") == Some(1), DELEGATE, "A.priority must be 1".into());
    check(
        &mut misses,
        text(a, "/llm/main/model") == OWNER_MODEL,
        DELEGATE,
        format!("A.llm.main.model must be {OWNER_MODEL}"),
    );
    check(
        &mut misses,
        text(a, "/llm/main/effort") == OWNER_EFFORT,
        DELEGATE,
        format!("A.llm.main.effort must be {OWNER_EFFORT}"),
    );
    let spec = text(a, "/spec/specification/words/bytes");
    check(
        &mut misses,
        spec.contains(DRAFT) && spec.contains(&draft_commit()),
        DELEGATE,
        "A.spec must name the draft and its first commit".into(),
    );
    for path in ["/parent", "/requester/task"] {
        if let Some(value) = a.pointer(path) {
            check(&mut misses, value.as_u64() == Some(DIRECTOR), DELEGATE, format!("A{path} must be the director"));
        }
    }
    authority(&mut misses, director, a, "A");
    authority(&mut misses, a, design, "Design");
    rolled_up(&mut misses, a, design, "A", true);
    rolled_up(&mut misses, director, a, "Director", false);

    question_answer(&mut misses, observed, a, director);
    markdown(&mut misses, observed);
    misses
}

fn question_answer(misses: &mut Vec<Miss>, observed: &Observed, a: &Value, director: &Value) {
    let mut report_at = None;
    let mut acknowledged = false;
    let mut delivered = false;
    let sent = format!("Message {QUESTION} sent");
    for (position, Told { agent, words, .. }) in observed.told.iter().enumerate() {
        if *agent == Agent::Director && contains(words, DESIGN_REPORT) && report_at.is_none() {
            report_at = Some(position);
        }
        if *agent == Agent::A {
            acknowledged |= contains(words, &sent);
            delivered |= contains(words, GREEN_LIGHT) && matches!(report_at, Some(at) if at < position);
        }
    }
    check(misses, report_at.is_some(), QUESTION_ANSWER, "Director was not told A's design report".into());
    check(misses, acknowledged, QUESTION_ANSWER, format!("A was not told {sent}"));
    check(
        misses,
        delivered,
        QUESTION_ANSWER,
        "A was not told the green light after the director heard its question".into(),
    );
    for (document, expected, label) in [(a, A_REPORT, "A"), (director, DIRECTOR_REPORT, "Director")] {
        check(
            misses,
            text(document, "/result/value/report/bytes") == expected,
            QUESTION_ANSWER,
            format!("{label}.result must be {expected:?}"),
        );
    }
}

fn markdown(misses: &mut Vec<Miss>, observed: &Observed) {
    // The fast path itself belongs to the forge's CI and is left out here
    // (domain/testing.md, 5.3's 4.1 row). We check the merge and its Markdown.
    check(misses, observed.kit.merged_green, MARKDOWN, "kit.main must have merged with green CI at its head".into());
    check(
        misses,
        observed.kit.main.get(DESIGN_DOC.as_bytes()).map(Vec::as_slice) == Some(DESIGN_WORDS.as_bytes()),
        MARKDOWN,
        "kit.main must hold A's design words".into(),
    );
    check(
        misses,
        observed.kit.main.get(DRAFT.as_bytes()).map(Vec::as_slice) == Some(DRAFT_WORDS.as_bytes()),
        MARKDOWN,
        "kit's draft must be unchanged".into(),
    );
    let mut before = BTreeMap::new();
    for file in kit_tree() {
        before.insert(file.path.to_vec(), file.content.to_vec());
    }
    let mut paths = BTreeSet::new();
    paths.extend(before.keys());
    paths.extend(observed.kit.main.keys());
    for path in paths {
        if before.get(path) != observed.kit.main.get(path) {
            check(
                misses,
                path.ends_with(b".md"),
                MARKDOWN,
                format!("changed path {:?} is not Markdown", String::from_utf8_lossy(path)),
            );
        }
    }
}

fn rolled_up(misses: &mut Vec<Miss>, parent: &Value, child: &Value, label: &str, positive: bool) {
    let below = number(parent, "/budget/numbers/spent_below");
    let spent = number(child, "/budget/numbers/spent");
    let child_below = number(child, "/budget/numbers/spent_below");
    let sum = match (spent, child_below) {
        (Some(spent), Some(below)) => spent.checked_add(below),
        _ => None,
    };
    check(
        misses,
        at_most(sum, below),
        SPEND,
        format!("{label}.budget.spent_below must cover its child's spent + spent_below"),
    );
    if positive {
        check(
            misses,
            matches!(below, Some(below) if below > 0),
            SPEND,
            format!("{label}.budget.spent_below must be positive"),
        );
    }
}
fn authority(misses: &mut Vec<Miss>, parent: &Value, child: &Value, label: &str) {
    let parent = &parent["authority"]["authority"];
    let child = &child["authority"]["authority"];
    for path in ["/spend", "/delegation/tasks", "/delegation/depth"] {
        check(
            misses,
            at_most(number(child, path), number(parent, path)),
            AUTHORITY,
            format!("{label}.authority{path} exceeds its parent or is absent"),
        );
    }
    check(
        misses,
        matches!((number(child, "/tools"), number(parent, "/tools")), (Some(child), Some(parent)) if child & !parent == 0),
        AUTHORITY,
        format!("{label}.authority.tools must be a subset"),
    );
    match (parent["delegation"]["kinds"].as_array(), child["delegation"]["kinds"].as_array()) {
        (Some(parents), Some(children)) => {
            for kind in children {
                check(
                    misses,
                    parents.contains(kind),
                    AUTHORITY,
                    format!("{label}.authority.delegation.kinds: {kind} is not covered"),
                );
            }
        }
        _ => check(misses, false, AUTHORITY, format!("{label}.authority.delegation.kinds is absent")),
    }
    match (parent["grants"].as_array(), child["grants"].as_array()) {
        (Some(parents), Some(children)) => {
            for grant in children {
                let mut covered = false;
                for parent in parents {
                    covered |= covers(parent, grant);
                }
                check(misses, covered, AUTHORITY, format!("{label}.authority.grants: {grant} is not covered"));
            }
        }
        _ => check(misses, false, AUTHORITY, format!("{label}.authority.grants is absent")),
    }
}
fn covers(parent: &Value, child: &Value) -> bool {
    if parent["connector"].as_u64().is_none()
        || parent["connector"] != child["connector"]
        || parent["kind"].as_u64().is_none()
        || parent["kind"] != child["kind"]
    {
        return false;
    }
    let (Some(parents), Some(children)) =
        (parent["pattern"]["segments"].as_array(), child["pattern"]["segments"].as_array())
    else {
        return false;
    };
    if !children.starts_with(parents) {
        return false;
    }
    let terminal = &parent["pattern"]["last"];
    if let Some(exact) = terminal.get("exact").and_then(Value::as_str) {
        return parents == children && child["pattern"]["last"]["exact"].as_str() == Some(exact);
    }
    let Some(prefix) = terminal.get("open").and_then(Value::as_str) else { return false };
    // Segments and terminal bytes are lower-case hex in ctl's JSON. Hex
    // preserves byte prefixes, so compare at whole-byte boundaries.
    let candidate = if let Some(segment) = children.get(parents.len()) {
        segment.as_str()
    } else {
        match child["pattern"]["last"].get("exact") {
            Some(exact) => exact.as_str(),
            None => child["pattern"]["last"]["open"].as_str(),
        }
    };
    matches!(candidate, Some(candidate) if prefix.len().is_multiple_of(2) && candidate.starts_with(prefix))
}
fn at_most(child: Option<u64>, parent: Option<u64>) -> bool {
    matches!((child, parent), (Some(child), Some(parent)) if child <= parent)
}

pub fn assert_met(misses: &[Miss]) {
    let mut report = String::new();
    for miss in misses {
        use fmt::Write;
        writeln!(report, "{miss}").expect("miss report");
    }
    assert!(misses.is_empty(), "{report}");
}

#[cfg(test)]
mod tests {
    use super::{
        A, A_REPORT, Agent, BTreeMap, DELEGATE, DESIGN, DESIGN_DOC, DESIGN_REPORT, DESIGN_WORDS, DIRECTOR,
        DIRECTOR_REPORT, GREEN_LIGHT, Kit, MARKDOWN, OWNER_EFFORT, OWNER_MODEL, Observed, PROJECT, QUESTION,
        QUESTION_ANSWER, Read, Told, Value, assert_met, contains, draft_commit, kit_tree, step_4_1,
    };
    use crate::case::{A_SPEC_PREFIX, A_SPEC_SUFFIX};
    use serde_json::json;

    fn met() -> Observed {
        let mut documents = BTreeMap::new();
        for (read, task, tools, spend, tasks, depth, spent, below, report) in [
            (Read::DirectorEnded, DIRECTOR, 3, 40, 7, 4, 2, 5, DIRECTOR_REPORT),
            (Read::AEnded, A, 3, 20, 6, 3, 2, 3, A_REPORT),
            (Read::DesignEnded, DESIGN, 1, 10, 4, 2, 1, 2, ""),
        ] {
            documents.insert(read, json!({"task":{
                "chip":{"task":task}, "project":PROJECT, "requester":{"task":DIRECTOR},
                "priority":1, "llm":{"main":{"model":OWNER_MODEL,"effort":OWNER_EFFORT}},
                "spec":{"specification":{"words":{"bytes":format!("{A_SPEC_PREFIX}{}{A_SPEC_SUFFIX}", draft_commit())}}},
                "authority":{"authority":{"tools":tools,"spend":spend,
                    "grants":[{"connector":1,"kind":2,"pattern":{"segments":["666f726765"],"last":{"open":"61636d65"}}}],
                    "delegation":{"kinds":[{"charter":1},{"procedure":2}],"tasks":tasks,"depth":depth}}},
                "budget":{"numbers":{"spent":spent,"spent_below":below}},
                "result":{"value":{"report":{"bytes":report}}}
            }}));
        }
        let mut main = BTreeMap::new();
        for file in kit_tree() {
            main.insert(file.path.to_vec(), file.content.to_vec());
        }
        main.insert(DESIGN_DOC.as_bytes().to_vec(), DESIGN_WORDS.as_bytes().to_vec());
        Observed {
            documents,
            commands: Vec::new(),
            told: vec![
                Told {
                    history: Box::new([]),
                    agent: Agent::A,
                    words: format!("Message {QUESTION} sent").into_bytes().into_boxed_slice(),
                    model: Box::new([]),
                    credential: Some(0),
                },
                Told {
                    history: Box::new([]),
                    agent: Agent::Director,
                    words: DESIGN_REPORT.as_bytes().into(),
                    model: Box::new([]),
                    credential: Some(0),
                },
                Told {
                    history: Box::new([]),
                    agent: Agent::A,
                    words: GREEN_LIGHT.as_bytes().into(),
                    model: Box::new([]),
                    credential: Some(0),
                },
            ],
            kit: Kit { main, merged_green: true },
        }
    }

    #[test]
    fn a_met_checkpoint_has_no_misses() {
        assert_met(&step_4_1(&met()));
    }

    #[test]
    fn the_checkpoint_reports_every_independent_miss_together() {
        // Three independent corruptions of otherwise sufficient observations.
        const MISSES: usize = 3;
        const GREEN_LIGHT_QUERIES: usize = 1;
        let mut broken = met().clone();
        broken.documents.get_mut(&Read::AEnded).expect("A document")["task"]["priority"] = Value::Null;
        broken.kit.main.insert(b"src/export.rs".to_vec(), b"fn export() {}\n".to_vec());
        let mut removed = 0;
        for Told { agent, words, .. } in &mut broken.told {
            if *agent == Agent::A && contains(words, GREEN_LIGHT) {
                *words = Box::new([]);
                removed += 1;
            }
        }
        assert_eq!(removed, GREEN_LIGHT_QUERIES);
        let misses = step_4_1(&broken);
        assert_eq!(misses.len(), MISSES);
        let [priority, answer, markdown] = misses.as_slice() else { panic!("three misses") };
        assert_eq!(priority.promise, DELEGATE);
        assert!(priority.what.contains("A.priority"));
        assert_eq!(answer.promise, QUESTION_ANSWER);
        assert!(answer.what.contains("green light"));
        assert_eq!(markdown.promise, MARKDOWN);
        assert!(markdown.what.contains("src/export.rs"));
    }
}

const ACCOUNT_WAIT: Promise =
    Promise::Must("an account's states hold the work they pay for, and name the act they wait for (exact)");
const SURVIVES: Promise = Promise::Add("no session dies, so nothing has to be recreated afterwards");

fn step_check(misses: &mut Vec<Miss>, step: &'static str, met: bool, promise: Promise, what: &str) {
    if !met {
        misses.push(Miss { step, promise, what: what.into() });
    }
}
fn document(observed: &Observed, read: Read) -> &Value {
    observed.documents.get(&read).unwrap_or(&Value::Null)
}

fn task_document(observed: &Observed, read: Read) -> &Value {
    let value = document(observed, read);
    value.get("task").filter(|task| task.is_object()).unwrap_or(value)
}

#[must_use]
pub fn step_4_5_refused(observed: &Observed) -> Vec<Miss> {
    use super::{FIRST_ACCOUNT, HELD_A_SPEC};
    let mut misses = Vec::new();
    let waiting = document(observed, Read::AWaiting);
    let accounts = document(observed, Read::Refused);
    for (met, promise, what) in [
        (
            number(waiting, "/chip/task") == Some(A)
                && number(waiting, "/run_wait/account/account") == Some(FIRST_ACCOUNT)
                && text(waiting, "/run_wait/account/why") == "refused",
            ACCOUNT_WAIT,
            "A's waiting task must name its account and refused",
        ),
        (
            accounts.pointer("/accounts/deployment").and_then(Value::as_array).is_some_and(|rows| {
                rows.iter().any(|row| {
                    number(row, "/number") == Some(FIRST_ACCOUNT) && text(row, "/state/kind") == "sign_in_again"
                })
            }),
            ACCOUNT_WAIT,
            "the refused account must need sign_in_again",
        ),
    ] {
        step_check(&mut misses, "4.5", met, promise, what);
    }
    account_survives(&mut misses, observed);
    let resumed = observed.told.iter().position(|told| told.agent == Agent::A && told.credential == Some(1));
    step_check(
        &mut misses,
        "4.5",
        resumed.is_some_and(|at| {
            let told = &observed.told[at];
            contains(&told.words, HELD_A_SPEC)
                && observed.told[..at].iter().all(|told| told.credential == Some(0))
                && observed.told[..at].iter().any(|told| told.agent == Agent::A && contains(&told.words, HELD_A_SPEC))
        }),
        SURVIVES,
        "A's resumed query must retain its spec on credential 1, after queries on credential 0",
    );
    misses
}

fn account_survives(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{FIRST_ACCOUNT, RESUMED_REPORT};
    let waiting = document(observed, Read::AWaiting);
    let a = &document(observed, Read::AEnded)["task"];
    let director = task_document(observed, Read::DirectorEnded);
    for (met, promise, what) in [
        (
            number(a, "/chip/task") == Some(A)
                && number(a, "/spec_revision/value") == Some(0)
                && number(a, "/kept_account") == Some(FIRST_ACCOUNT)
                && a.pointer("/result/ending/task/done").is_some()
                && text(a, "/result/value/report/bytes") == RESUMED_REPORT,
            SURVIVES,
            "A must finish done with its original task, revision and kept account",
        ),
        (
            number(waiting, "/chip/task") == number(a, "/chip/task") && number(a, "/chip/task").is_some(),
            SURVIVES,
            "the waiting and ended task numbers must be the same",
        ),
        (
            number(director, "/chip/task") == Some(DIRECTOR)
                && text(director, "/result/value/report/bytes") == RESUMED_REPORT,
            SURVIVES,
            "the original director must finish its report",
        ),
        (a["delegates"].as_array().is_some_and(Vec::is_empty), SURVIVES, "A must have no delegate"),
        (
            observed.told.iter().all(|told| matches!(told.agent, Agent::A | Agent::Director)),
            SURVIVES,
            "no agent beyond the director and A may run",
        ),
    ] {
        step_check(misses, "4.5", met, promise, what);
    }
}

#[must_use]
pub fn step_4_5_limited(observed: &Observed) -> Vec<Miss> {
    use super::{FIRST_ACCOUNT, HELD_A_SPEC};
    let mut misses = Vec::new();
    let waiting = document(observed, Read::AWaiting);
    let until = number(waiting, "/run_wait/account/why/limited/until");
    for (met, promise, what) in [
        (
            number(waiting, "/chip/task") == Some(A)
                && text(waiting, "/chip/phase") == "due"
                && number(waiting, "/run_wait/account/account") == Some(FIRST_ACCOUNT)
                && number(waiting, "/run_wait/account/source") == Some(0)
                && until.is_some(),
            ACCOUNT_WAIT,
            "A's waiting task must name its account, source and limited until while due",
        ),
        (
            until.is_some()
                && document(observed, Read::Limited)
                    .pointer("/accounts/deployment")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| {
                        rows.iter().any(|row| {
                            number(row, "/number") == Some(FIRST_ACCOUNT)
                                && later(row, until)
                                && row["sources"].as_array().is_some_and(|sources| {
                                    sources
                                        .iter()
                                        .any(|source| number(source, "/number") == Some(0) && later(source, until))
                                })
                        })
                    }),
            ACCOUNT_WAIT,
            "the account and its source must be unusable and later until the same reset",
        ),
        (
            until
                .zip(number(task_document(observed, Read::AEnded), "/result/at"))
                .is_some_and(|(reset, ended)| ended > reset),
            SURVIVES,
            "A's result timestamp must be past the reset",
        ),
        (
            observed.commands.iter().position(|command| command == "chat").is_some_and(|chat| {
                observed.commands[chat + 1..]
                    .iter()
                    .all(|command| matches!(command.as_str(), "wait" | "task" | "accounts"))
            }),
            ACCOUNT_WAIT,
            "no person's act may follow the chat, including retry or replace-key",
        ),
    ] {
        step_check(&mut misses, "4.5", met, promise, what);
    }
    account_survives(&mut misses, observed);
    // The failed call committed no assistant turn: a fresh attempt must still
    // receive the spec that was in the first query, on the original credential.
    let mut queries = observed.told.iter().filter(|told| told.agent == Agent::A);
    let first = queries.next();
    let resumed = queries.next_back();
    step_check(
        &mut misses,
        "4.5",
        first.zip(resumed).is_some_and(|(first, resumed)| {
            first.credential == Some(0)
                && resumed.credential == Some(0)
                && contains(&first.words, HELD_A_SPEC)
                && contains(&resumed.words, HELD_A_SPEC)
        }),
        SURVIVES,
        "A's resumed query must retain its spec on credential 0 after its failed query",
    );
    misses
}

// Each document projects the engine's one monotonic deadline to wall time as
// it is read (jig crates/jig-domain-core/src/documents/deployment.rs:163), so
// two reads differ by their reads' gap. Allow 1 ms, in nanoseconds.
const PROJECTION: u64 = 1_000_000;
fn later(value: &Value, until: Option<u64>) -> bool {
    text(value, "/state/kind") == "later"
        && number(value, "/state/until").zip(until).is_some_and(|(state, until)| state.abs_diff(until) <= PROJECTION)
        && value["usable"] == false
}

#[cfg(test)]
mod account_tests {
    use super::{
        ACCOUNT_WAIT, Agent, BTreeMap, DIRECTOR, Kit, Observed, PROJECTION, Read, SURVIVES, Told, Value, assert_met,
        later, step_4_5_limited, step_4_5_refused,
    };
    use crate::case::{A, FIRST_ACCOUNT, HELD_A_SPEC, RESUMED_REPORT};
    use serde_json::json;

    fn told(agent: Agent, words: &str, credential: u32) -> Told {
        Told {
            history: Box::new([]),
            agent,
            words: words.as_bytes().into(),
            model: Box::new([]),
            credential: Some(credential),
        }
    }
    fn refused_met() -> Observed {
        Observed {
            documents: BTreeMap::from([
                (
                    Read::AWaiting,
                    json!({"chip":{"task":A},
                    "run_wait":{"account":{"account":FIRST_ACCOUNT,"why":"refused"}}}),
                ),
                (
                    Read::Refused,
                    json!({"accounts":{"deployment":[{"number":FIRST_ACCOUNT,
                    "state":{"kind":"sign_in_again"}}]}}),
                ),
                (
                    Read::AEnded,
                    json!({"task":{"chip":{"task":A},"spec_revision":{"value":0},
                    "kept_account":FIRST_ACCOUNT,"delegates":[],
                    "result":{"ending":{"task":{"done":{}}},"value":{"report":{"bytes":RESUMED_REPORT}}}}}),
                ),
                (
                    Read::DirectorEnded,
                    json!({"task":{"chip":{"task":DIRECTOR},
                    "result":{"ending":{"task":{"done":{}}},"value":{"report":{"bytes":RESUMED_REPORT}}}}}),
                ),
            ]),
            commands: Vec::new(),
            told: vec![told(Agent::A, HELD_A_SPEC, 0), told(Agent::A, HELD_A_SPEC, 1)],
            kit: Kit { main: BTreeMap::new(), merged_green: false },
        }
    }
    #[test]
    fn refused_checkpoint_met() {
        assert_met(&step_4_5_refused(&refused_met()));
    }
    #[test]
    fn refused_checkpoint_reports_independent_misses_together() {
        const MISSES: usize = 3;
        let mut observed = refused_met();
        observed.documents.get_mut(&Read::AWaiting).expect("waiting")["run_wait"] = Value::Null;
        observed.documents.remove(&Read::Refused);
        observed.told.last_mut().expect("resumed").words = Box::new([]);
        let misses = step_4_5_refused(&observed);
        assert_eq!(misses.len(), MISSES);
        assert!(misses.iter().all(|miss| miss.step == "4.5"));
        assert!(misses.iter().any(|miss| miss.promise == ACCOUNT_WAIT && miss.what.contains("waiting task")));
        assert!(misses.iter().any(|miss| miss.promise == ACCOUNT_WAIT && miss.what.contains("sign_in_again")));
        assert!(misses.iter().any(|miss| miss.promise == SURVIVES && miss.what.contains("resumed query")));
    }

    const RESET: u64 = 10_000_000;
    fn limited_met() -> Observed {
        let mut observed = refused_met();
        observed.documents.insert(
            Read::AWaiting,
            json!({"chip":{"task":A,"phase":"due"},
                "run_wait":{"account":{"account":FIRST_ACCOUNT,"source":0,
                    "why":{"limited":{"until":RESET}}}}}),
        );
        observed.documents.remove(&Read::Refused);
        observed.documents.insert(
            Read::Limited,
            json!({"accounts":{"deployment":[{"number":FIRST_ACCOUNT,"usable":false,
                "state":{"kind":"later","until":RESET},
                "sources":[{"number":0,"usable":false,"state":{"kind":"later","until":RESET}}]}]}}),
        );
        observed.documents.get_mut(&Read::AEnded).expect("ended")["task"]["result"]["at"] = (RESET + 1).into();
        observed.commands = ["chat", "wait", "wait", "task", "accounts", "task"].map(String::from).into();
        observed.told = vec![told(Agent::A, HELD_A_SPEC, 0), told(Agent::A, HELD_A_SPEC, 0)];
        observed
    }
    #[test]
    fn limited_checkpoint_met() {
        assert_met(&step_4_5_limited(&limited_met()));
    }
    #[test]
    fn reset_projection_accepts_the_tolerance_in_both_directions() {
        for until in [RESET - PROJECTION, RESET + PROJECTION] {
            let row = json!({"usable":false,"state":{"kind":"later","until":until}});
            assert!(later(&row, Some(RESET)));
        }
    }
    #[test]
    fn reset_projection_rejects_one_past_the_tolerance_in_both_directions() {
        for until in [RESET - PROJECTION - 1, RESET + PROJECTION + 1] {
            let row = json!({"usable":false,"state":{"kind":"later","until":until}});
            assert!(!later(&row, Some(RESET)));
        }
    }
    #[test]
    fn limited_checkpoint_checks_both_account_and_source() {
        const MISSES: usize = 1;
        for path in ["/state/until", "/sources/0/state/until", "/sources/0/number", "/usable", "/sources/0/usable"] {
            let mut observed = limited_met();
            *observed.documents.get_mut(&Read::Limited).expect("limited")["accounts"]["deployment"][0]
                .pointer_mut(path)
                .expect("account field") = Value::Null;
            let misses = step_4_5_limited(&observed);
            assert_eq!(misses.len(), MISSES, "{path}");
            assert!(misses.iter().all(|miss| miss.promise == ACCOUNT_WAIT && miss.what.contains("unusable")));
        }
    }
    #[test]
    fn limited_checkpoint_rejects_completion_at_the_reset() {
        let mut observed = limited_met();
        observed.documents.get_mut(&Read::AEnded).expect("ended")["task"]["result"]["at"] = RESET.into();
        let misses = step_4_5_limited(&observed);
        assert!(misses.iter().any(|miss| miss.promise == SURVIVES && miss.what.contains("timestamp")));
    }
    #[test]
    fn limited_checkpoint_reports_independent_misses_together() {
        const MISSES: usize = 6;
        let mut observed = limited_met();
        observed.documents.get_mut(&Read::AWaiting).expect("waiting")["chip"]["phase"] = "running".into();
        observed.documents.remove(&Read::Limited);
        observed.documents.get_mut(&Read::AEnded).expect("ended")["task"]["result"]["at"] = RESET.into();
        observed.documents.get_mut(&Read::AEnded).expect("ended")["task"]["kept_account"] = Value::Null;
        observed.commands.push("retry".into());
        observed.told.last_mut().expect("resumed").words = Box::new([]);
        let misses = step_4_5_limited(&observed);
        assert_eq!(misses.len(), MISSES);
        assert!(misses.iter().all(|miss| miss.step == "4.5"));
        for what in ["waiting task", "unusable", "timestamp", "person's act", "original task", "resumed query"] {
            assert!(misses.iter().any(|miss| miss.what.contains(what)), "{what}");
        }
    }
}

const PACE_INBOX: Promise = Promise::Must("a question in the person's inbox (exact)");
const ONE_QUESTION: Promise = Promise::Add("the question waits in one inbox, not in two checks' reports");
const PACE_KEPT: Promise = Promise::Step("the answer goes into the director's notes");
const MODELS: Promise = Promise::Must("models amended (exact)");
const ORDER: Promise = Promise::Must("the holder's order of sources (exact)");

#[must_use]
pub fn step_4_4(observed: &Observed) -> Vec<Miss> {
    let mut misses = Vec::new();
    pace_inbox(&mut misses, observed);
    pace_note(&mut misses, observed);
    pace_model(&mut misses, observed);
    pace_accounts(&mut misses, observed);
    misses
}
fn pace_inbox(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{PACE_ANSWER, PACE_QUESTION, PACE_QUESTION_MESSAGE};
    const ONE: usize = 1;
    let rows = document(observed, Read::PaceAsked).pointer("/needs/rows").and_then(Value::as_array);
    step_check(
        misses,
        "4.4",
        rows.is_some_and(|rows| {
            rows.iter().any(|row| {
                number(row, "/task/live/task") == Some(DIRECTOR)
                    && number(row, "/project") == Some(PROJECT)
                    && number(row, "/object/question/message") == Some(PACE_QUESTION_MESSAGE)
            })
        }),
        PACE_INBOX,
        "the inbox must hold A's question on the director in project 1",
    );
    step_check(
        misses,
        "4.4",
        rows.is_some_and(|rows| rows.len() == ONE),
        ONE_QUESTION,
        "the person's questions inbox must have exactly one row",
    );
    let asked =
        observed.told.iter().position(|told| told.agent == Agent::Director && contains(&told.words, PACE_QUESTION));
    let answered = observed
        .told
        .iter()
        .position(|told| told.agent == Agent::Director && contains(&told.words, &format!("1: {PACE_ANSWER}")));
    step_check(
        misses,
        "4.4",
        matches!((asked, answered), (Some(asked), Some(answered)) if asked < answered),
        PACE_INBOX,
        "the director must hear A's question, then the person's answer",
    );
    let sent = format!("Message {PACE_QUESTION_MESSAGE} sent");
    step_check(
        misses,
        "4.4",
        observed.told.iter().any(|told| told.agent == Agent::A && contains(&told.words, &sent)),
        PACE_INBOX,
        "A must be told the question's named message number",
    );
}
fn pace_note(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{PACE_ANSWER, PACE_NOTE};
    let mut written = None;
    for (at, told) in observed.told.iter().enumerate() {
        if told.agent != Agent::Director {
            continue;
        }
        let words = String::from_utf8_lossy(&told.words);
        for suffix in words.split("Note ").skip(1) {
            let Some((digits, _)) = suffix.split_once(" revision 1 written") else { continue };
            if let Ok(note) = digits.parse::<u64>()
                && note > 0
            {
                written = Some((at, note));
                break;
            }
        }
        if written.is_some() {
            break;
        }
    }
    step_check(misses, "4.4", written.is_some(), PACE_KEPT, "the director must be told Note N revision 1 written");
    step_check(
        misses,
        "4.4",
        written.is_some_and(|(at, note)| {
            let recalled = format!("Note {note} revision 1: {PACE_NOTE}\n{PACE_ANSWER}\n");
            observed
                .told
                .iter()
                .skip(at + 1)
                .any(|told| told.agent == Agent::Director && contains(&told.words, &recalled))
        }),
        PACE_KEPT,
        "after writing the note the director must recall its answer",
    );
}
fn pace_model(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{DEFAULT_MODEL, PACE_ANSWER};
    let a = task_document(observed, Read::AEnded);
    step_check(
        misses,
        "4.4",
        number(a, "/chip/task") == Some(A)
            && number(a, "/project") == Some(PROJECT)
            && number(a, "/spec_revision/value") == Some(1)
            && text(a, "/llm/main/model") == DEFAULT_MODEL
            && a.pointer("/llm/main/effort") == Some(&Value::Null),
        MODELS,
        "A must have the default model without effort at spec revision 1",
    );
    let queries: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::A).collect();
    step_check(
        misses,
        "4.4",
        queries.first().is_some_and(|told| told.model.as_ref() == OWNER_MODEL.as_bytes())
            && queries.last().is_some_and(|told| {
                told.model.as_ref() == DEFAULT_MODEL.as_bytes() && contains(&told.words, PACE_ANSWER)
            }),
        MODELS,
        "A must first query owner, then use fake with the person's relayed answer",
    );
}
fn pace_accounts(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{FIRST_ACCOUNT, SECOND_ACCOUNT, SURVEY, SURVEY_REPORT};
    let rows = document(observed, Read::Ranked).pointer("/accounts/deployment").and_then(Value::as_array);
    let ranked = rows.is_some_and(|rows| {
        let numbers: Vec<_> = rows.iter().map(|row| number(row, "/number")).collect();
        let places: Vec<_> = rows.iter().map(|row| number(row, "/place")).collect();
        numbers == [Some(SECOND_ACCOUNT), Some(FIRST_ACCOUNT)] && places == [Some(0), Some(1)]
    });
    step_check(misses, "4.4", ranked, ORDER, "deployment accounts must put the second account first");
    let a = task_document(observed, Read::AEnded);
    step_check(
        misses,
        "4.4",
        number(a, "/chip/task") == Some(A) && number(a, "/kept_account") == Some(FIRST_ACCOUNT),
        ORDER,
        "A must keep its original account",
    );
    let survey = task_document(observed, Read::SurveyEnded);
    step_check(
        misses,
        "4.4",
        number(survey, "/chip/task") == Some(SURVEY)
            && number(survey, "/project") == Some(PROJECT)
            && number(survey, "/kept_account") == Some(SECOND_ACCOUNT)
            && text(survey, "/result/value/report/bytes") == SURVEY_REPORT,
        ORDER,
        "the survey must finish on the newly first account",
    );
    let queries: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Survey).collect();
    step_check(
        misses,
        "4.4",
        !queries.is_empty() && queries.iter().all(|told| told.credential == Some(1)),
        ORDER,
        "every survey query must use credential 1",
    );
}

#[cfg(test)]
mod pace_tests {
    use super::{
        Agent, BTreeMap, DIRECTOR, Kit, MODELS, ORDER, OWNER_MODEL, Observed, PACE_INBOX, PROJECT, Read, Told, Value,
        assert_met, step_4_4,
    };
    use crate::case::{
        A, DEFAULT_MODEL, FIRST_ACCOUNT, PACE_ANSWER, PACE_NOTE, PACE_QUESTION, PACE_QUESTION_MESSAGE, SECOND_ACCOUNT,
        SURVEY, SURVEY_REPORT,
    };
    use serde_json::json;

    fn told(agent: Agent, words: &str, model: &str, credential: u32) -> Told {
        Told {
            history: Box::new([]),
            agent,
            words: words.as_bytes().into(),
            model: model.as_bytes().into(),
            credential: Some(credential),
        }
    }
    fn met() -> Observed {
        Observed {
            documents: BTreeMap::from([
                (
                    Read::PaceAsked,
                    json!({"needs":{"rows":[{"task":{"live":{"task":DIRECTOR}},
                    "project":PROJECT,"object":{"question":{"message":PACE_QUESTION_MESSAGE}}}]}}),
                ),
                (
                    Read::Ranked,
                    json!({"accounts":{"deployment":[
                    {"number":SECOND_ACCOUNT,"place":0},{"number":FIRST_ACCOUNT,"place":1}]}}),
                ),
                (
                    Read::AEnded,
                    json!({"task":{"chip":{"task":A},"project":PROJECT,
                    "spec_revision":{"value":1},"llm":{"main":{"model":DEFAULT_MODEL,"effort":null}},
                    "kept_account":FIRST_ACCOUNT}}),
                ),
                (
                    Read::SurveyEnded,
                    json!({"task":{"chip":{"task":SURVEY},"project":PROJECT,
                    "kept_account":SECOND_ACCOUNT,"result":{"value":{"report":{"bytes":SURVEY_REPORT}}}}}),
                ),
            ]),
            commands: Vec::new(),
            told: vec![
                told(Agent::A, &format!("Message {PACE_QUESTION_MESSAGE} sent"), OWNER_MODEL, 0),
                told(Agent::Director, PACE_QUESTION, DEFAULT_MODEL, 0),
                told(Agent::Director, &format!("1: {PACE_ANSWER}\nNote 1 revision 1 written"), DEFAULT_MODEL, 0),
                told(Agent::A, PACE_ANSWER, DEFAULT_MODEL, 0),
                told(Agent::Survey, "Begin", DEFAULT_MODEL, 1),
                told(Agent::Director, &format!("Note 1 revision 1: {PACE_NOTE}\n{PACE_ANSWER}\n"), DEFAULT_MODEL, 0),
            ],
            kit: Kit { main: BTreeMap::new(), merged_green: false },
        }
    }
    #[test]
    fn pace_checkpoint_met() {
        assert_met(&step_4_4(&met()));
    }
    #[test]
    fn pace_checkpoint_reports_independent_misses_together() {
        const MISSES: usize = 3;
        const QUESTION_ROW: usize = 0;
        let mut observed = met();
        observed.documents.get_mut(&Read::PaceAsked).expect("inbox")["needs"]["rows"][QUESTION_ROW]["project"] =
            Value::Null;
        observed.documents.get_mut(&Read::AEnded).expect("A")["task"]["spec_revision"] = Value::Null;
        observed.documents.get_mut(&Read::Ranked).expect("ranked")["accounts"]["deployment"]
            .as_array_mut()
            .expect("accounts")
            .reverse();
        let misses = step_4_4(&observed);
        assert_eq!(misses.len(), MISSES);
        assert!(misses.iter().all(|miss| miss.step == "4.4"));
        for promise in [PACE_INBOX, MODELS, ORDER] {
            assert!(misses.iter().any(|miss| miss.promise == promise));
        }
    }
}

// ctl wraps the source journal lines in its decoded activity rows.
fn check_activity(observed: &Observed, read: Read) -> Vec<&Value> {
    let document = document(observed, read);
    document
        .get("activity")
        .filter(|value| value.is_object())
        .unwrap_or(document)
        .pointer("/rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.pointer("/event/owner/source"))
        .collect()
}

/// Reference case 4.3's supported timer, cancel and empty-worker checkpoint.
#[must_use]
pub fn step_4_3(observed: &Observed) -> Vec<Miss> {
    use super::{B, B_REPORT, BENCHMARK, CHECK, CHECK_REASON, CHECK_REPORT};
    const TIMER: Promise = Promise::Must("a timer (exact)");
    const CANCEL: Promise = Promise::Must("cancel (exact)");
    const EMPTY: Promise = Promise::Add("a moved owner leaves nothing running behind");
    let mut misses = Vec::new();
    let director = task_document(observed, Read::CheckEnded);
    let benchmark = task_document(observed, Read::BenchmarkEnded);
    let b = task_document(observed, Read::BEnded);
    let director_activity = check_activity(observed, Read::DirectorTimer);
    let timer = director_activity
        .iter()
        .find_map(|line| number(line, "/line/heard/kind/timer/subscription").zip(number(line, "/line/heard/number")));
    step_check(
        &mut misses,
        "4.3",
        timer.is_some(),
        TIMER,
        "the director's activity must record the timer's subscription and wake",
    );
    let queries: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Check).collect();
    step_check(
        &mut misses,
        "4.3",
        timer.is_some_and(|(subscription, _message)| {
            let result = format!("Subscription {subscription} added");
            let wake = format!("\n{CHECK}: \n");
            queries.iter().position(|told| contains(&told.words, &result)).is_some_and(|at| {
                !contains(&queries[at].words, &wake)
                    && queries[at + 1..].iter().any(|told| {
                        contains(&told.words, &wake)
                            && contains(&told.words, "Messages arrived. Read them before continuing.")
                    })
            })
        }),
        TIMER,
        "after the subscription result, a later director query must contain the timer's empty wake and live wait result",
    );
    step_check(
        &mut misses,
        "4.3",
        number(benchmark, "/chip/task") == Some(BENCHMARK)
            && text(benchmark, "/chip/phase") == "ended"
            && number(benchmark, "/result/ending/task/cancelled/by/task") == Some(CHECK)
            && text(benchmark, "/result/ending/task/cancelled/reason/bytes") == CHECK_REASON,
        CANCEL,
        "the ended benchmark must record cancellation by the director with its reason",
    );
    let activity = check_activity(observed, Read::BenchmarkEnded);
    let attempt = number(benchmark, "/chip/details/owner/details/attempt");
    let cancelled = activity.iter().any(|line| {
        text(line, "/line/run_ended/end") == "cancelled"
            && number(line, "/line/run_ended/attempt").zip(attempt).is_some_and(|(ended, current)| ended == current)
    });
    step_check(&mut misses, "4.3", cancelled, CANCEL, "the benchmark's activity must record a cancelled run ending");
    let began: BTreeSet<_> = activity.iter().filter_map(|line| number(line, "/line/run_began/attempt")).collect();
    let ended: BTreeSet<_> = activity.iter().filter_map(|line| number(line, "/line/run_ended/attempt")).collect();
    step_check(
        &mut misses,
        "4.3",
        !began.is_empty()
            && began.is_subset(&ended)
            && document(observed, Read::BenchmarkEnded).pointer("/activity/cut") == Some(&Value::Bool(false))
            && document(observed, Read::BenchmarkEnded).pointer("/activity/before") == Some(&Value::Null)
            && document(observed, Read::BenchmarkEnded).pointer("/activity/after") == Some(&Value::Null)
            && number(benchmark, "/chip/details/owner/details/ended_at").is_some(),
        EMPTY,
        "the benchmark's complete activity must end every begun run, with the task ended",
    );
    // B's journal orders its lines, newest first (../client/tools.md, 11.3): the
    // cancel heard is older than the run that finished with B's report. The
    // journal's order, not two wall times, so a clock step can't reorder them.
    let b_activity = check_activity(observed, Read::BEnded);
    let reported = b_activity.iter().position(|line| text(line, "/line/run_ended/end") == "finished");
    let heard = b_activity.iter().position(|line| {
        text(line, "/line/heard/kind/result") == "cancelled"
            && number(line, "/line/heard/from/task") == Some(BENCHMARK)
            && text(line, "/line/heard/words") == CHECK_REASON
    });
    let heard_cancel = reported.zip(heard).is_some_and(|(reported, heard)| reported < heard);
    step_check(
        &mut misses,
        "4.3",
        number(b, "/chip/task") == Some(B)
            && text(b, "/result/value/report/bytes") == B_REPORT
            && heard_cancel
            && observed.told.iter().any(|told| {
                told.agent == Agent::B
                    && contains(&told.words, CHECK_REASON)
                    && contains(&told.words, "Messages arrived. Read them before continuing.")
            }),
        EMPTY,
        "B must hear its delegate's cancelled ending before reporting",
    );
    step_check(
        &mut misses,
        "4.3",
        number(director, "/chip/task") == Some(CHECK)
            && text(director, "/result/value/report/bytes") == CHECK_REPORT
            && observed.told.iter().any(|told| told.agent == Agent::Check && contains(&told.words, B_REPORT)),
        EMPTY,
        "the check director must hear B's result and end with its check report",
    );
    misses
}

const REPLACED: Promise = Promise::Must("the chat's run placed again after its worker's grace (exact)");
const FENCED: Promise = Promise::Must("one run per task, the lost attempt fenced before the next (gone)");
const QUESTIONS_WAIT: Promise = Promise::Step("A's questions wait, since the director lives there");
const ONCE: Promise = Promise::Add("nothing resumes twice");

/// Reference case 4.6's supported retry, retained question and single answer checkpoint.
#[must_use]
pub fn step_4_6(observed: &Observed) -> Vec<Miss> {
    use super::{HANG, HANG_REPORT, PHASE, PHASE_QUESTION_NUMBER, PHASE_REPORT};
    let mut misses = Vec::new();
    let reads = [Read::HangActivity, Read::HangActivityOlder, Read::HangActivityFirst];
    let rows: Vec<_> =
        reads.iter().flat_map(|read| document(observed, *read)["rows"].as_array().into_iter().flatten()).collect();
    let complete = reads.iter().all(|read| document(observed, *read)["cut"] == false)
        && document(observed, Read::HangActivityFirst)["before"].is_null();
    let ordered = rows
        .windows(2)
        .all(|pair| number(pair[0], "/position").zip(number(pair[1], "/position")).is_some_and(|(new, old)| new > old));
    let activity: Vec<_> = reads.iter().flat_map(|read| check_activity(observed, *read)).collect();
    let mut open = None;
    let mut sequenced = true;
    let mut attempts = Vec::new();
    let mut endings = Vec::new();
    for source in activity.iter().rev() {
        let line = &source["line"];
        if let Some(began) = line.get("run_began") {
            sequenced &= open.is_none();
            open = number(began, "/attempt");
            sequenced &= open.is_some();
            attempts.push(open);
        }
        if let Some(ended) = line.get("run_ended") {
            sequenced &= open.is_some() && open.take() == number(ended, "/attempt");
            endings.push(ended);
        }
    }
    sequenced &= open.is_none();
    step_check(
        &mut misses,
        "4.6",
        complete && ordered && sequenced,
        FENCED,
        "the complete journal must order every run beginning after the previous ending, with no attempt left open",
    );
    let director = task_document(observed, Read::HangEnded);
    let retried = matches!(attempts.as_slice(), [Some(first), Some(next)] if first != next)
        && matches!(endings.as_slice(), [first, next]
            if text(first, "/end/failed/class") == "agent" && text(next, "/end") == "finished");
    step_check(
        &mut misses,
        "4.6",
        retried
            && complete
            && ordered
            && sequenced
            && number(director, "/chip/task") == Some(HANG)
            && text(director, "/chip/phase") == "ended"
            && text(director, "/result/value/report/bytes") == HANG_REPORT,
        REPLACED,
        "the chat must have exactly two ordered attempts, failed as agent then finished, and end with its report",
    );
    let read = check_activity(observed, Read::HangQuestionRead)
        .iter()
        .any(|source| number(source, "/line/turn/read") == Some(PHASE_QUESTION_NUMBER));
    let waiting = task_document(observed, Read::HangWaiting);
    let (retained, answered_once) = restart_messages(observed);
    step_check(
        &mut misses,
        "4.6",
        read && number(waiting, "/chip/task") == Some(HANG)
            && text(waiting, "/chip/details/owner/details/observation/work") == "waiting"
            && retained,
        QUESTIONS_WAIT,
        "before the stop, the director must have committed A's question read and be waiting; its retry must retain one copy",
    );
    let phase = task_document(observed, Read::PhaseEnded);
    step_check(
        &mut misses,
        "4.6",
        answered_once
            && number(phase, "/chip/task") == Some(PHASE)
            && text(phase, "/chip/phase") == "ended"
            && text(phase, "/result/value/report/bytes") == PHASE_REPORT,
        ONCE,
        "the director must send one answer call, A must hear it once, and A's next phase must end with its report",
    );
    misses
}

fn user_words(told: &Told) -> impl Iterator<Item = &[u8]> {
    use skein_fake_llm_domain::api::{Part, Role};
    told.history.iter().filter(|message| message.role == Role::User).flat_map(|message| &message.parts).filter_map(
        |part| match part {
            Part::Text { text } | Part::ToolOutput { output: text, .. } => Some(text.as_ref()),
            Part::Opaque { .. } | Part::ToolCall { .. } => None,
        },
    )
}

fn restart_messages(observed: &Observed) -> (bool, bool) {
    use super::{PHASE, PHASE_ANSWER, PHASE_QUESTION, PHASE_QUESTION_NUMBER};
    use skein_fake_llm_domain::api::Part;
    let mut resumed = false;
    let mut retained = true;
    let mut answers = BTreeSet::new();
    let mut valid_answers = true;
    let mut received = 0;
    for told in &observed.told {
        if told.agent == Agent::Hang {
            let mut waits = 0;
            for part in told.history.iter().flat_map(|message| &message.parts) {
                if let Part::ToolCall { id, name, arguments } = part {
                    waits += usize::from(name.as_ref() == b"wait");
                    if name.as_ref() == b"message"
                        && let Ok(args) = serde_json::from_slice::<Value>(arguments)
                        && args["form"] == "answer"
                        && args["question"] == PHASE_QUESTION_NUMBER
                    {
                        valid_answers &= args["target"] == PHASE && args["words"] == PHASE_ANSWER;
                        answers.insert(id.as_ref());
                    }
                }
            }
            if waits >= 2 && user_words(told).any(|words| contains(words, "Begin the work your brief describes.")) {
                resumed = true;
                retained &=
                    user_words(told).map(|words| calls::occurrences(words, PHASE_QUESTION.as_bytes())).sum::<usize>()
                        == 1;
            }
        }
        if told.agent == Agent::Phase {
            received += user_words(told).map(|words| calls::occurrences(words, PHASE_ANSWER.as_bytes())).sum::<usize>();
        }
    }
    (resumed && retained, valid_answers && answers.len() == 1 && received == 1)
}

#[cfg(test)]
mod restart_tests {
    use super::*;
    use crate::case::{HANG, HANG_REPORT, PHASE, PHASE_ANSWER, PHASE_QUESTION, PHASE_QUESTION_NUMBER, PHASE_REPORT};
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn call(id: &str, name: &str, arguments: &Value) -> Part {
        Part::ToolCall {
            id: id.as_bytes().into(),
            name: name.as_bytes().into(),
            arguments: serde_json::to_vec(arguments).expect("tool arguments").into(),
        }
    }
    fn told(agent: Agent, history: Vec<Message>) -> Told {
        Told { agent, history: history.into_boxed_slice(), words: Box::new([]), model: Box::new([]), credential: None }
    }
    fn met() -> Observed {
        let events = [
            json!({"run_ended":{"attempt":8,"end":"finished"}}),
            json!({"run_began":{"attempt":8}}),
            json!({"run_ended":{"attempt":7,"end":{"failed":{"class":"agent"}}}}),
            json!({"run_began":{"attempt":7}}),
        ];
        let mut documents = BTreeMap::new();
        for (index, read) in
            [Read::HangActivity, Read::HangActivityOlder, Read::HangActivityFirst].into_iter().enumerate()
        {
            let selected = if index == 0 { &events[..2] } else { &events[index + 1..index + 2] };
            let newest = if index == 0 { 4 } else { 3 - index };
            let rows: Vec<_> = selected
                .iter()
                .enumerate()
                .map(|(offset, line)| json!({"position":newest-offset,"event":{"owner":{"source":{"line":line}}}}))
                .collect();
            documents.insert(read, json!({"cut":false,"before":if index == 2 { Value::Null } else { json!({"value":"older"}) },"rows":rows}));
        }
        for (read, task, report) in [(Read::HangEnded, HANG, HANG_REPORT), (Read::PhaseEnded, PHASE, PHASE_REPORT)] {
            documents.insert(
                read,
                json!({"task":{"chip":{"task":task,"phase":"ended"},"result":{"value":{"report":{"bytes":report}}}}}),
            );
        }
        documents.insert(
            Read::HangWaiting,
            json!({"task":{"chip":{"task":HANG,"details":{"owner":{"details":{"observation":{"work":"waiting"}}}}}}}),
        );
        documents.insert(
            Read::HangQuestionRead,
            json!({"rows":[{"event":{"owner":{"source":{"line":{"turn":{"read":PHASE_QUESTION_NUMBER}}}}}}]}),
        );
        let answer = call(
            "answer",
            "message",
            &json!({"target":PHASE,"form":"answer","question":PHASE_QUESTION_NUMBER,"words":PHASE_ANSWER}),
        );
        Observed {
            documents,
            commands: Vec::new(),
            kit: Kit { main: BTreeMap::new(), merged_green: false },
            told: vec![
                told(
                    Agent::Hang,
                    vec![
                        Message {
                            role: Role::Assistant,
                            parts: vec![
                                call("wait-one", "wait", &json!({})),
                                call("wait-two", "wait", &json!({})),
                                answer,
                            ]
                            .into_boxed_slice(),
                        },
                        Message {
                            role: Role::User,
                            parts: vec![Part::Text {
                                text: format!("Begin the work your brief describes. {PHASE_QUESTION}")
                                    .into_bytes()
                                    .into_boxed_slice(),
                            }]
                            .into_boxed_slice(),
                        },
                    ],
                ),
                told(
                    Agent::Phase,
                    vec![Message {
                        role: Role::User,
                        parts: vec![Part::ToolOutput {
                            id: b"wait".as_slice().into(),
                            output: PHASE_ANSWER.as_bytes().into(),
                            is_error: false,
                        }]
                        .into_boxed_slice(),
                    }],
                ),
            ],
        }
    }
    #[test]
    fn a_restarted_chat_meets_the_checkpoint() {
        assert_met(&step_4_6(&met()));
    }
    #[test]
    fn restart_promises_report_independent_failures_together() {
        const MISSES: usize = 4;
        let mut observed = met();
        observed.documents.get_mut(&Read::HangActivity).expect("activity")["rows"]
            .as_array_mut()
            .expect("rows")
            .reverse();
        observed.documents.get_mut(&Read::HangWaiting).expect("waiting")["task"]["chip"]["details"]["owner"]["details"]
            ["observation"]["work"] = json!("running");
        observed.documents.get_mut(&Read::PhaseEnded).expect("phase report")["task"]["result"]["value"]["report"]["bytes"] =
            json!("wrong report");
        let misses = step_4_6(&observed);
        assert_eq!(misses.len(), MISSES);
        assert!(misses.iter().all(|miss| miss.step == "4.6"));
        for promise in [REPLACED, FENCED, QUESTIONS_WAIT, ONCE] {
            assert!(misses.iter().any(|miss| miss.promise == promise));
        }
    }
    #[test]
    fn replayed_history_is_one_answer_but_a_new_call_is_another() {
        const DIRECTOR_QUERY: usize = 0;
        let mut observed = met();
        observed.told.push(observed.told[DIRECTOR_QUERY].clone());
        assert_met(&step_4_6(&observed));
        let duplicate = call(
            "second-answer",
            "message",
            &json!({"target":PHASE,"form":"answer","question":PHASE_QUESTION_NUMBER,"words":PHASE_ANSWER}),
        );
        observed.told.push(told(
            Agent::Hang,
            vec![Message { role: Role::Assistant, parts: vec![duplicate].into_boxed_slice() }],
        ));
        let misses = step_4_6(&observed);
        assert!(misses.iter().any(|miss| miss.promise == ONCE));
    }
    #[test]
    fn a_missing_question_or_a_duplicate_delivery_breaks_its_promise() {
        const DIRECTOR_QUERY: usize = 0;
        const USER_MESSAGE: usize = 1;
        const PHASE_QUERY: usize = 1;
        let mut observed = met();
        observed.told[DIRECTOR_QUERY].history[USER_MESSAGE].parts =
            vec![Part::Text { text: b"Begin the work your brief describes.".as_slice().into() }].into_boxed_slice();
        observed.told.push(observed.told[PHASE_QUERY].clone());
        let misses = step_4_6(&observed);
        assert!(misses.iter().any(|miss| miss.promise == QUESTIONS_WAIT));
        assert!(misses.iter().any(|miss| miss.promise == ONCE));
    }
}
