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
    /// kit's main after its last observed merge or the person's push.
    pub kit: Kit,
}
#[derive(Clone)]
pub struct Told {
    /// Ordered query messages, preserving roles and tool-call identities across a retry.
    pub history: Box<[skein_fake_llm_domain::api::Message]>,
    pub agent: Agent,
    /// Repository conventions and the brief, separate from conversation messages.
    pub system: Box<[u8]>,
    pub words: Box<[u8]>,
    pub model: Box<[u8]>,
    pub credential: Option<u32>,
}
#[derive(Clone)]
pub struct Kit {
    /// Forge records in their observed order, including the person's main push.
    pub records: Vec<skein_fake_forge_domain::Observation>,
    pub main: BTreeMap<Vec<u8>, Vec<u8>>,
    pub merged_green: bool,
    pub pulls: Vec<Pull>,
}
impl Told {
    /// The oracle's per-query view, shared with process-tier observation readers.
    #[must_use]
    pub fn new(
        system: &[u8],
        model: &[u8],
        credential: Option<u32>,
        messages: &[skein_fake_llm_domain::api::Message],
    ) -> Option<Self> {
        let agent = cued(system)?;
        let mut words = system.to_vec();
        words.push(b'\n');
        for message in messages {
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
        Some(Self {
            system: system.into(),
            history: messages.into(),
            agent,
            words: words.into_boxed_slice(),
            model: model.into(),
            credential,
        })
    }
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
            if let Some(query) = Told::new(&query.system, &query.model, query.credential, &query.messages) {
                told.push(query);
            }
        }
        // The model peer observes the delivered brief even when the run ends
        // before a ctl read could see it. Keep the first query after Release.
        if let Some(fresh) =
            told.iter().find(|query| query.agent == Agent::Rules && contains(&query.system, super::NEW_RULES))
        {
            documents
                .insert(Read::RulesFreshBrief, serde_json::json!({"system": String::from_utf8_lossy(&fresh.system)}));
        }
        Self {
            documents,
            commands: Vec::new(),
            told,
            kit: Kit {
                records: ran.forge().observations().to_vec(),
                pulls: pull_digest(ran.forge().observations()),
                main: ran.forge().main().clone(),
                merged_green: ran.forge().merged_green(),
            },
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
        Agent::Rules,
        Agent::Scope,
        Agent::Pause,
        Agent::Owner,
        Agent::Bowner,
        Agent::Helper,
        Agent::Implementer,
        Agent::H1Coder,
        Agent::Shape,
        Agent::Mover,
        Agent::Plan,
        Agent::Planner,
        Agent::Subowner,
        Agent::Misread,
        Agent::MisreadHelper,
        Agent::Direction,
        Agent::StoryOwner,
        Agent::OverlapOwner,
        Agent::X3Writer,
        Agent::PlannedWriter,
        Agent::StoryWriter,
        Agent::BatchDirector,
        Agent::BatchOwner,
        Agent::BatchSubowner,
        Agent::BatchOne,
        Agent::BatchTwo,
        Agent::BatchThree,
        Agent::BatchStory,
        Agent::BlockerDirector,
        Agent::BlockerOwner,
        Agent::BlockerSubowner,
        Agent::BlockerImplementer,
        Agent::BlockerCoder,
        // Act 3, step 6.5 (domain/testing.md, 5.3)
        Agent::BudgetDirector,
        Agent::BudgetOwner,
        Agent::BudgetSubowner,
        Agent::BudgetInvestigator,
        Agent::BudgetAdvisor,
        // 6.4
        Agent::RedDirector,
        Agent::RedOwner,
        Agent::RedCoder,
        Agent::RedRepairCoder,
        // Act 3, step 6.7 (domain/testing.md, 5.3)
        Agent::CapDirector,
        Agent::CapOwner,
        Agent::CapSubowner,
        Agent::CapImplementer,
    ] {
        for (at, word) in system.windows(agent.cue().len()).enumerate() {
            if word == agent.cue() {
                // No later cue can precede zero; equal positions keep this agent's priority.
                if at == 0 {
                    return Some(agent);
                }
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
fn text<'a>(value: &'a Value, path: &str) -> &'a str {
    value.pointer(path).and_then(Value::as_str).unwrap_or_default()
}
fn number(value: &Value, path: &str) -> Option<u64> {
    value.pointer(path).and_then(Value::as_u64)
}
fn contains(words: &[u8], needle: &str) -> bool {
    words.windows(needle.len()).any(|part| part == needle.as_bytes())
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
                    system: Box::new([]),
                    history: Box::new([]),
                    agent: Agent::A,
                    words: format!("Message {QUESTION} sent").into_bytes().into_boxed_slice(),
                    model: Box::new([]),
                    credential: Some(0),
                },
                Told {
                    system: Box::new([]),
                    history: Box::new([]),
                    agent: Agent::Director,
                    words: DESIGN_REPORT.as_bytes().into(),
                    model: Box::new([]),
                    credential: Some(0),
                },
                Told {
                    system: Box::new([]),
                    history: Box::new([]),
                    agent: Agent::A,
                    words: GREEN_LIGHT.as_bytes().into(),
                    model: Box::new([]),
                    credential: Some(0),
                },
            ],
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main, merged_green: true },
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
            system: Box::new([]),
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
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: false },
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
        let passing = limited_met();
        for path in ["/state/until", "/sources/0/state/until", "/sources/0/number", "/usable", "/sources/0/usable"] {
            let mut observed = passing.clone();
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
            system: Box::new([]),
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
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: false },
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
        Told {
            system: Box::new([]),
            agent,
            history: history.into_boxed_slice(),
            words: Box::new([]),
            model: Box::new([]),
            credential: None,
        }
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
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: false },
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

// Act 2, step 5.1 (domain/testing.md, 5.3)
const RULES_CONTROL: Promise = Promise::Must("Stop and Release on the chat (exact)");
const RULES_INBOX: Promise = Promise::Must("questions wait in its inbox (exact)");
const RULES_CONVENTIONS: Promise = Promise::Must("the rules land as conventions that every later run reads (exact)");
const RULES_FRESH: Promise = Promise::Must("the director's next run starts fresh and reads them (exact)");
const RULES_NO_NOTICE: Promise = Promise::Add("no message telling anyone to re-read a rule");
const RULES_DROPPED: Promise = Promise::Add("a held owner needs no notice that a rule was dropped");

#[must_use]
pub fn step_5_1(observed: &Observed) -> Vec<Miss> {
    let mut misses = Vec::new();
    rules_controls(&mut misses, observed);
    rules_director(&mut misses, observed);
    rules_fresh_tail(&mut misses, observed);
    rules_owner(&mut misses, observed);
    rules_messages_and_endings(&mut misses, observed);
    misses
}

fn rules_controls(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{NEW_RULES, RULES, RULES_FILE, RULES_HOLD_REVISION, SCOPE, SCOPE_QUESTION, SCOPE_QUESTION_NUMBER};
    let waiting = task_document(observed, Read::RulesWaiting);
    let held = task_document(observed, Read::RulesHeld);
    let question = task_document(observed, Read::RulesQuestionWaiting);
    for (met, promise, what) in [
        (
            number(waiting, "/chip/task") == Some(RULES)
                && text(waiting, "/chip/details/owner/details/observation/work") == "waiting"
                && waiting["delegates"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| number(row, "/chip/task") == Some(SCOPE))),
            RULES_CONTROL,
            "the director must wait in its run with A delegated",
        ),
        (rules_held(held), RULES_CONTROL, "the chat must be held, stopped by the person at its hold revision"),
        (
            rules_held(question)
                && check_activity(observed, Read::RulesQuestionWaiting).iter().any(|row| {
                    number(row, "/task") == Some(RULES)
                        && number(row, "/line/words/message") == Some(SCOPE_QUESTION_NUMBER)
                        && number(row, "/line/words/by/task") == Some(SCOPE)
                        && text(row, "/line/words/kind") == "question"
                        && text(row, "/line/words/words") == SCOPE_QUESTION
                }),
            RULES_INBOX,
            "A's numbered question must wait at the still-held chat",
        ),
        (
            number(question, "/hold/owner/revision") == Some(RULES_HOLD_REVISION),
            RULES_INBOX,
            "the same hold must remain while the question waits",
        ),
        (
            observed.kit.main.get(RULES_FILE.as_bytes()).is_some_and(|words| words == NEW_RULES.as_bytes()),
            RULES_CONVENTIONS,
            "kit's main must hold the new guide",
        ),
    ] {
        step_check(misses, "5.1", met, promise, what);
    }
}

fn rules_director(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{DROPPED_RULE, NEW_RULES, OLD_RULES, SCOPE_QUESTION};
    use skein_fake_llm_domain::api::Role;
    let director: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Rules).collect();
    let resumed = director.iter().position(|told| {
        contains(&told.system, NEW_RULES) || user_words(told).any(|words| contains(words, SCOPE_QUESTION))
    });
    let before = resumed.map_or(&director[..], |at| &director[..at]);
    let after = resumed.map_or(&director[director.len()..], |at| &director[at..]);
    for (met, promise, what) in [
        (
            !before.is_empty()
                && before.iter().all(|told| contains(&told.system, OLD_RULES))
                && before.first().is_some_and(|first| before.iter().all(|told| told.system == first.system))
                && before.windows(2).all(|pair| pair[1].history.starts_with(&pair[0].history)),
            RULES_CONVENTIONS,
            "the director's opening run must read the old guide",
        ),
        (
            !after.is_empty()
                && after.iter().all(|told| contains(&told.system, NEW_RULES) && !contains(&told.system, DROPPED_RULE)),
            RULES_CONVENTIONS,
            "every director query told the question must read the new guide without the dropped rule",
        ),
        (
            after.first().is_some_and(|first| first.history.iter().all(|message| message.role != Role::Assistant)),
            RULES_FRESH,
            "the director's first query after release must contain none of its earlier assistant turns",
        ),
        (
            after.first().is_some_and(|first| {
                after.iter().all(|told| told.system == first.system)
                    && after.windows(2).all(|pair| pair[1].history.starts_with(&pair[0].history))
                    && after.iter().all(|told| {
                        user_words(told)
                            .map(|words| calls::occurrences(words, SCOPE_QUESTION.as_bytes()))
                            .sum::<usize>()
                            == 1
                    })
            }),
            RULES_INBOX,
            "the question must be delivered once in one director run's growing history",
        ),
    ] {
        step_check(misses, "5.1", met, promise, what);
    }
}

fn rules_fresh_tail(misses: &mut Vec<Miss>, observed: &Observed) {
    let system = text(document(observed, Read::RulesFreshBrief), "/system");
    let tail = system.split_once("Transcript tail").map(|(_, rest)| rest.split("\n#").next().unwrap_or(rest));
    step_check(
        misses,
        "5.1",
        tail.is_some_and(|tail| {
            tail.contains("call delegate:")
                && tail.contains(super::SCOPE_SPEC)
                && tail.contains(&format!("result: Delegated tasks: {}", super::SCOPE))
                && !tail.contains("[earlier bytes not text, cut]")
        }),
        RULES_FRESH,
        "the director's fresh brief must have a Transcript tail holding its earlier committed turn text",
    );
}

fn rules_owner(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{DROPPED_RULE, NEW_RULES, SCOPE_ANSWER};
    let a: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Scope).collect();
    step_check(
        misses,
        "5.1",
        a.first().is_some_and(|told| contains(&told.system, DROPPED_RULE)),
        RULES_DROPPED,
        "A's opening query must read the dropped rule while it is in force",
    );
    let answered: Vec<_> =
        a.iter().filter(|told| user_words(told).any(|words| contains(words, SCOPE_ANSWER))).collect();
    step_check(
        misses,
        "5.1",
        answered.len() == 1
            && answered.iter().all(|told| {
                contains(&told.system, NEW_RULES)
                    && !contains(&told.system, DROPPED_RULE)
                    && user_words(told).map(|words| calls::occurrences(words, SCOPE_ANSWER.as_bytes())).sum::<usize>()
                        == 1
            }),
        RULES_DROPPED,
        "A must hear the answer once in a run reading the new guide without the dropped rule",
    );
}

fn rules_messages_and_endings(misses: &mut Vec<Miss>, observed: &Observed) {
    use super::{DROPPED_RULE, NEW_RULES, RULES, RULES_REPORT, SCOPE, SCOPE_REPORT};
    step_check(
        misses,
        "5.1",
        observed.told.iter().all(|told| {
            message_words(told).all(|words| {
                NEW_RULES
                    .lines()
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .all(|line| !contains(words, line))
                    && !contains(words, "re-read")
                    && !contains(words, "reread")
            })
        }),
        RULES_NO_NOTICE,
        "no query's messages may carry the new rules or a request to re-read them",
    );
    step_check(
        misses,
        "5.1",
        observed.told.iter().all(|told| message_words(told).all(|words| !contains(words, DROPPED_RULE))),
        RULES_DROPPED,
        "no message may tell an owner the dropped rule",
    );
    for read in [Read::RulesActivity, Read::ScopeActivity] {
        let endings: Vec<_> =
            check_activity(observed, read).into_iter().filter_map(|row| row.pointer("/line/run_ended")).collect();
        step_check(
            misses,
            "5.1",
            !endings.is_empty()
                && endings
                    .iter()
                    .all(|ended| ended.pointer("/end/failed/class").and_then(Value::as_str) != Some("invalid")),
            RULES_CONTROL,
            "no run of the director or A may start invalid: a fresh start refused and retried would pass unseen",
        );
    }
    for (read, task, report) in [(Read::RulesEnded, RULES, RULES_REPORT), (Read::ScopeEnded, SCOPE, SCOPE_REPORT)] {
        let ended = task_document(observed, read);
        step_check(
            misses,
            "5.1",
            number(ended, "/chip/task") == Some(task)
                && text(ended, "/chip/phase") == "ended"
                && text(ended, "/result/value/report/bytes") == report,
            RULES_CONTROL,
            "the original director and A must end with their reports",
        );
    }
}

fn rules_held(task: &Value) -> bool {
    use super::{RULES, RULES_HOLD_REVISION};
    number(task, "/chip/task") == Some(RULES)
        && text(task, "/chip/phase") == "held"
        && number(task, "/hold/owner/reason/stopped_by/party") == Some(1)
        && number(task, "/hold/owner/revision") == Some(RULES_HOLD_REVISION)
}

fn message_words(told: &Told) -> impl Iterator<Item = &[u8]> {
    use skein_fake_llm_domain::api::Part;
    told.history.iter().flat_map(|message| &message.parts).filter_map(|part| match part {
        Part::Text { text } | Part::ToolOutput { output: text, .. } => Some(text.as_ref()),
        Part::ToolCall { arguments, .. } => Some(arguments.as_ref()),
        Part::Opaque { .. } => None,
    })
}

#[cfg(test)]
mod rules_tests {
    use super::*;
    use crate::case::{
        DROPPED_RULE, NEW_RULES, OLD_RULES, RULES, RULES_FILE, RULES_HOLD_REVISION, RULES_REPORT, SCOPE, SCOPE_ANSWER,
        SCOPE_QUESTION, SCOPE_QUESTION_NUMBER, SCOPE_REPORT,
    };
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    const OPENING: usize = 0;
    const OPENING_FOLLOWUP: usize = 1;
    const FRESH: usize = 2;
    const LATER: usize = 3;
    const A_OPENING: usize = 4;
    const A_ANSWERED: usize = 5;

    fn user(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn assistant(words: &str) -> Message {
        Message { role: Role::Assistant, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn told(agent: Agent, guide: &str, history: &[Message]) -> Told {
        let mut system = agent.cue().to_vec();
        system.extend_from_slice(guide.as_bytes());
        Told::new(&system, b"fake", Some(0), history).expect("case cue")
    }
    fn held() -> Value {
        json!({"chip":{"task":RULES,"phase":"held"}, "hold":{"owner":{
            "reason":{"stopped_by":{"party":1}},"revision":RULES_HOLD_REVISION}}})
    }
    fn ended(task: u64, report: &str) -> Value {
        json!({"task":{"chip":{"task":task,"phase":"ended"},"result":{"value":{"report":{"bytes":report}}}}})
    }
    fn endings(task: u64, ends: &[Value]) -> Value {
        let rows: Vec<Value> = ends
            .iter()
            .map(|end| json!({"event":{"owner":{"source":{"task":task,"line":{"run_ended":{"end":end}}}}}}))
            .collect();
        json!({"task":{"chip":{"task":task}},"activity":{"rows":rows}})
    }
    fn fresh_tail() -> String {
        format!(
            "## Transcript tail\n\ncall delegate: {}\nresult: Delegated tasks: {}\n",
            super::super::SCOPE_SPEC,
            SCOPE
        )
    }
    fn met() -> Observed {
        let fresh = vec![user(&format!("{SCOPE}: {SCOPE_QUESTION}"))];
        let mut later = fresh.clone();
        later.extend([assistant("Answer A."), user("Answer sent")]);
        Observed {
            documents: BTreeMap::from([
                (
                    Read::RulesWaiting,
                    json!({"chip":{"task":RULES,"details":{"owner":{"details":{
                    "observation":{"work":"waiting"}}}}},"delegates":[{"chip":{"task":SCOPE}}]}),
                ),
                (Read::RulesHeld, held()),
                (
                    Read::RulesQuestionWaiting,
                    json!({"task":held(),"activity":{"rows":[{"event":{"owner":{"source":{
                    "task":RULES,"line":{"words":{"message":SCOPE_QUESTION_NUMBER,"by":{"task":SCOPE},
                        "kind":"question","words":SCOPE_QUESTION}}}}}}]}}),
                ),
                (Read::RulesFreshBrief, json!({"system": fresh_tail()})),
                (Read::RulesEnded, ended(RULES, RULES_REPORT)),
                (Read::ScopeEnded, ended(SCOPE, SCOPE_REPORT)),
                (Read::RulesActivity, endings(RULES, &[json!("finished"), json!({"failed":{"class":"agent"}})])),
                (Read::ScopeActivity, endings(SCOPE, &[json!("finished"), json!("parked")])),
            ]),
            told: vec![
                told(Agent::Rules, OLD_RULES, &[user("Begin the work your brief describes.")]),
                told(
                    Agent::Rules,
                    OLD_RULES,
                    &[user("Begin the work your brief describes."), assistant("Delegate A.")],
                ),
                told(Agent::Rules, NEW_RULES, &fresh),
                told(Agent::Rules, NEW_RULES, &later),
                told(Agent::Scope, OLD_RULES, &[user("Begin the work your brief describes.")]),
                told(Agent::Scope, NEW_RULES, &[user(SCOPE_ANSWER)]),
            ],
            commands: Vec::new(),
            kit: Kit {
                records: Vec::new(),
                pulls: Vec::new(),
                main: BTreeMap::from([(RULES_FILE.as_bytes().to_vec(), NEW_RULES.as_bytes().to_vec())]),
                merged_green: false,
            },
        }
    }
    fn rejects(observed: &Observed, promise: Promise, what: &str) {
        let misses = step_5_1(observed);
        assert!(misses.iter().all(|miss| miss.step == "5.1"));
        assert!(
            misses.iter().any(|miss| miss.promise == promise && miss.what.contains(what)),
            "expected {what}: {}",
            misses.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        );
    }
    #[test]
    fn held_chat_and_fresh_rules_meet_the_checkpoint() {
        assert_met(&step_5_1(&met()));
    }
    #[test]
    fn the_wait_and_both_hold_reads_are_checked() {
        let passing = met();
        for (read, path, promise, what) in [
            (Read::RulesWaiting, "/chip/task", RULES_CONTROL, "wait in its run"),
            (Read::RulesWaiting, "/chip/details/owner/details/observation/work", RULES_CONTROL, "wait in its run"),
            (Read::RulesWaiting, "/delegates/0/chip/task", RULES_CONTROL, "wait in its run"),
            (Read::RulesHeld, "/chip/task", RULES_CONTROL, "stopped by the person"),
            (Read::RulesHeld, "/chip/phase", RULES_CONTROL, "stopped by the person"),
            (Read::RulesHeld, "/hold/owner/reason/stopped_by/party", RULES_CONTROL, "stopped by the person"),
            (Read::RulesHeld, "/hold/owner/revision", RULES_CONTROL, "stopped by the person"),
            (Read::RulesQuestionWaiting, "/task/chip/phase", RULES_INBOX, "still-held"),
            (Read::RulesQuestionWaiting, "/task/hold/owner/revision", RULES_INBOX, "same hold"),
        ] {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&read).expect("document").pointer_mut(path).expect("field") = Value::Null;
            rejects(&observed, promise, what);
        }
    }
    #[test]
    fn the_waiting_question_has_its_recipient_sender_number_and_words() {
        let passing = met();
        for path in ["/task", "/line/words/message", "/line/words/by/task", "/line/words/kind", "/line/words/words"] {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&Read::RulesQuestionWaiting).expect("question")["activity"]["rows"][0]
                ["event"]["owner"]["source"].pointer_mut(path).expect("question field") = Value::Null;
            rejects(&observed, RULES_INBOX, "numbered question");
        }
    }
    #[test]
    fn main_and_every_relevant_query_must_have_the_right_system_guide() {
        let passing = met();
        let mut observed = passing.clone();
        observed.kit.main.clear();
        rejects(&observed, RULES_CONVENTIONS, "kit's main");
        for at in [OPENING, FRESH, LATER, A_OPENING, A_ANSWERED] {
            let mut observed = passing.clone();
            observed.told[at].system = Box::new([]);
            let (promise, what) = match at {
                OPENING => (RULES_CONVENTIONS, "opening run"),
                A_OPENING => (RULES_DROPPED, "while it is in force"),
                A_ANSWERED => (RULES_DROPPED, "hear the answer once"),
                _ => (RULES_CONVENTIONS, "every director query"),
            };
            rejects(&observed, promise, what);
        }
        for at in [FRESH, LATER, A_ANSWERED] {
            let mut observed = passing.clone();
            let mut system = observed.told[at].system.to_vec();
            system.extend_from_slice(DROPPED_RULE.as_bytes());
            observed.told[at].system = system.into();
            rejects(
                &observed,
                if at == A_ANSWERED { RULES_DROPPED } else { RULES_CONVENTIONS },
                if at == A_ANSWERED { "hear the answer once" } else { "every director query" },
            );
        }
    }
    #[test]
    fn the_directors_history_must_be_fresh_then_grow_in_one_run() {
        let passing = met();
        let mut observed = passing.clone();
        let mut history = observed.told[FRESH].history.to_vec();
        history.push(assistant("Delegate A."));
        observed.told[FRESH].history = history.into();
        rejects(&observed, RULES_FRESH, "earlier assistant turns");
        let mut observed = passing.clone();
        observed.told[LATER].system = OLD_RULES.as_bytes().into();
        rejects(&observed, RULES_INBOX, "one director run");
        let mut observed = passing.clone();
        observed.told[LATER].history = Box::new([user("Another run"), user(SCOPE_QUESTION)]);
        rejects(&observed, RULES_INBOX, "one director run");
        for replacement in ["No question", &format!("{SCOPE_QUESTION} {SCOPE_QUESTION}")] {
            let mut observed = passing.clone();
            observed.told[FRESH].history = Box::new([user(replacement)]);
            rejects(&observed, RULES_INBOX, "delivered once");
        }
    }
    #[test]
    fn the_opening_queries_must_also_belong_to_one_run() {
        let passing = met();
        let mut observed = passing.clone();
        observed.told[OPENING_FOLLOWUP].system = format!("{OLD_RULES}another run").into_bytes().into();
        rejects(&observed, RULES_CONVENTIONS, "opening run");
        let mut observed = passing.clone();
        observed.told[OPENING_FOLLOWUP].history = Box::new([user("Another opening")]);
        rejects(&observed, RULES_CONVENTIONS, "opening run");
    }

    #[test]
    fn a_must_receive_the_answer_once() {
        let passing = met();
        let mut observed = passing.clone();
        observed.told[A_ANSWERED].history = Box::new([]);
        rejects(&observed, RULES_DROPPED, "hear the answer once");
        let mut observed = passing.clone();
        observed.told.push(observed.told[A_ANSWERED].clone());
        rejects(&observed, RULES_DROPPED, "hear the answer once");
        let mut observed = passing.clone();
        observed.told[A_ANSWERED].history = Box::new([user(&format!("{SCOPE_ANSWER} {SCOPE_ANSWER}"))]);
        rejects(&observed, RULES_DROPPED, "hear the answer once");
    }
    #[test]
    fn messages_cannot_supply_rules_or_reread_notices() {
        let passing = met();
        for words in NEW_RULES
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .chain(["Please re-read the guide", "Please reread the guide"])
        {
            let mut observed = passing.clone();
            observed.told[A_ANSWERED].history = Box::new([user(words), user(SCOPE_ANSWER)]);
            rejects(&observed, RULES_NO_NOTICE, "no query's messages");
        }
        let mut observed = passing.clone();
        observed.told[A_ANSWERED].history = Box::new([user(DROPPED_RULE), user(SCOPE_ANSWER)]);
        rejects(&observed, RULES_DROPPED, "no message");
    }
    #[test]
    fn a_fresh_tail_must_hold_committed_turn_text_without_a_binary_marker() {
        for system in [
            String::new(),
            "Transcript tail\n[earlier bytes not text, cut]\n".into(),
            "Transcript tail\n[a turn not readable as text]\n".into(),
            format!("{}[earlier bytes not text, cut]\n", fresh_tail()),
            fresh_tail().replace("Transcript tail", "Task"),
            fresh_tail().replace(super::super::SCOPE_SPEC, "other task"),
            fresh_tail().replace("result: Delegated tasks: 2", "result: delegation refused"),
            format!("## Transcript tail\n\n## Notes\n{}", fresh_tail()),
        ] {
            let mut observed = met();
            observed.documents.get_mut(&Read::RulesFreshBrief).expect("fresh brief")["system"] = system.into();
            rejects(&observed, RULES_FRESH, "Transcript tail");
        }
        let mut observed = met();
        observed.documents.remove(&Read::RulesFreshBrief);
        rejects(&observed, RULES_FRESH, "Transcript tail");
    }

    #[test]
    fn no_run_may_start_invalid_and_hide_behind_its_retry() {
        let passing = met();
        for read in [Read::RulesActivity, Read::ScopeActivity] {
            let mut observed = passing.clone();
            let rows =
                observed.documents.get_mut(&read).expect("activity")["activity"]["rows"].as_array_mut().expect("rows");
            rows.insert(
                1,
                json!({"event":{"owner":{"source":{"line":{"run_ended":{"end":{"failed":{"class":"invalid"}}}}}}}}),
            );
            rejects(&observed, RULES_CONTROL, "start invalid");
            let mut observed = passing.clone();
            observed.documents.remove(&read);
            rejects(&observed, RULES_CONTROL, "start invalid");
        }
    }
    #[test]
    fn both_original_tasks_must_end_with_their_reports() {
        let passing = met();
        for read in [Read::RulesEnded, Read::ScopeEnded] {
            for path in ["/task/chip/task", "/task/chip/phase", "/task/result/value/report/bytes"] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("ended").pointer_mut(path).expect("field") = Value::Null;
                rejects(&observed, RULES_CONTROL, "original director and A");
            }
        }
    }
}

// Act 2, step 5.3 (domain/testing.md, 5.3)
const SHAPE_INSTRUCTIONS: Promise = Promise::Must("instructions amended (exact)");
const SHAPE_HISTORY: Promise = Promise::Add("the amendment is in A's history, with what it replaced");
const SHAPE_ENDINGS: Promise = Promise::Step("the chat and A report under the new brief");

/// The observable before/after, without a deadline's hold (waiting on jig)
/// or a ctl read of the stored amendment history (a client gap).
#[must_use]
pub fn step_5_3(observed: &Observed) -> Vec<Miss> {
    use super::{
        MOVER, MOVER_AMENDMENT_REVISION, MOVER_QUESTION, MOVER_QUESTION_NUMBER, MOVER_REPORT, NEW_BRIEF, OLD_BRIEF,
        SHAPE, SHAPE_REPORT,
    };
    let mut misses = Vec::new();
    let ended = task_document(observed, Read::MoverEnded);
    step_check(
        &mut misses,
        "5.3",
        text(ended, "/instructions/bytes") == NEW_BRIEF
            && ended.pointer("/instructions/cut") == Some(&Value::Bool(false))
            && number(ended, "/spec_revision/value") == Some(MOVER_AMENDMENT_REVISION),
        SHAPE_HISTORY,
        "A's document must hold the whole new brief at the amendment's revision",
    );
    let a: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Mover).collect();
    step_check(
        &mut misses,
        "5.3",
        a.first().is_some_and(|told| contains(&told.system, OLD_BRIEF) && !contains(&told.system, NEW_BRIEF)),
        SHAPE_HISTORY,
        "A's first query must carry the old instructions alone in system text",
    );
    let amended = a
        .iter()
        .position(|told| contains(&told.system, NEW_BRIEF) || user_words(told).any(|words| contains(words, NEW_BRIEF)));
    let after = amended.map_or(&a[a.len()..], |at| &a[at..]);
    step_check(
        &mut misses,
        "5.3",
        amended.is_some_and(|at| at > 0)
            && !after.is_empty()
            && after.iter().all(|told| contains(&told.system, NEW_BRIEF) && !contains(&told.system, OLD_BRIEF)),
        SHAPE_INSTRUCTIONS,
        "every A query from the amendment onward must use the new instructions alone",
    );
    step_check(
        &mut misses,
        "5.3",
        !after.is_empty()
            && after.iter().all(|told| {
                user_words(told).map(|words| calls::occurrences(words, NEW_BRIEF.as_bytes())).sum::<usize>() == 1
            }),
        SHAPE_INSTRUCTIONS,
        "A must be told the amendment's whole message once in each later query's history",
    );
    let chat: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Shape).collect();
    step_check(
        &mut misses,
        "5.3",
        !chat.is_empty()
            && chat.iter().all(|told| !contains(&told.system, OLD_BRIEF) && !contains(&told.system, NEW_BRIEF)),
        SHAPE_INSTRUCTIONS,
        "the chat's charter must carry neither of A's instruction briefs",
    );
    let sent = format!("Message {MOVER_QUESTION_NUMBER} sent");
    let question_at = observed
        .told
        .iter()
        .position(|told| told.agent == Agent::Shape && user_words(told).any(|words| contains(words, MOVER_QUESTION)));
    let amendment_at = observed.told.iter().position(|told| {
        told.agent == Agent::Mover
            && (contains(&told.system, NEW_BRIEF) || user_words(told).any(|words| contains(words, NEW_BRIEF)))
    });
    step_check(
        &mut misses,
        "5.3",
        a.iter().any(|told| contains(&told.words, &sent))
            && question_at.zip(amendment_at).is_some_and(|(question, amendment)| question < amendment),
        SHAPE_INSTRUCTIONS,
        "A's numbered question must reach the chat before it amends A",
    );
    for (read, task, report) in [(Read::MoverEnded, MOVER, MOVER_REPORT), (Read::ShapeEnded, SHAPE, SHAPE_REPORT)] {
        let ended = task_document(observed, read);
        step_check(
            &mut misses,
            "5.3",
            number(ended, "/chip/task") == Some(task)
                && number(ended, "/project") == Some(PROJECT)
                && text(ended, "/chip/phase") == "ended"
                && text(ended, "/result/value/report/bytes") == report,
            SHAPE_ENDINGS,
            "the original chat and A must end with their reports in the project",
        );
    }
    misses
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use crate::case::{
        MOVER, MOVER_AMENDMENT_REVISION, MOVER_QUESTION, MOVER_QUESTION_NUMBER, MOVER_REPORT, NEW_BRIEF, OLD_BRIEF,
        SHAPE, SHAPE_REPORT,
    };
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn user(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn told(agent: Agent, brief: &str, messages: &[Message]) -> Told {
        Told::new(&[agent.cue(), b"\n", brief.as_bytes()].concat(), b"fake", Some(0), messages).expect("case cue")
    }
    fn met() -> Observed {
        let a = json!({"chip":{"task":MOVER,"phase":"ended"},"project":PROJECT,
            "instructions":{"bytes":NEW_BRIEF,"cut":false},"spec_revision":{"value":MOVER_AMENDMENT_REVISION},
            "result":{"value":{"report":{"bytes":MOVER_REPORT}}}});
        let shape = json!({"chip":{"task":SHAPE,"phase":"ended"},"project":PROJECT,
            "result":{"value":{"report":{"bytes":SHAPE_REPORT}}}});
        Observed {
            documents: BTreeMap::from([(Read::MoverEnded, a), (Read::ShapeEnded, shape)]),
            told: vec![
                told(Agent::Mover, OLD_BRIEF, &[]),
                told(
                    Agent::Mover,
                    OLD_BRIEF,
                    &[Message {
                        role: Role::User,
                        parts: Box::new([Part::ToolOutput {
                            id: b"question".as_slice().into(),
                            is_error: false,
                            output: format!("Message {MOVER_QUESTION_NUMBER} sent").into_bytes().into(),
                        }]),
                    }],
                ),
                told(Agent::Shape, "", &[user(MOVER_QUESTION)]),
                told(Agent::Mover, NEW_BRIEF, &[user(NEW_BRIEF)]),
                told(Agent::Mover, NEW_BRIEF, &[user(NEW_BRIEF), user("Continue")]),
            ],
            commands: Vec::new(),
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: false },
        }
    }
    fn rejects(observed: &Observed, promise: Promise, what: &str) {
        let misses = step_5_3(observed);
        assert!(misses.iter().all(|miss| miss.step == "5.3"));
        assert!(
            misses.iter().any(|miss| miss.promise == promise && miss.what.contains(what)),
            "expected {what}: {}",
            misses.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        );
    }
    #[test]
    fn amendment_and_its_observable_before_and_after_meet_the_checkpoint() {
        assert_met(&step_5_3(&met()));
    }
    #[test]
    fn the_current_instructions_and_their_revision_are_checked() {
        let passing = met();
        for path in ["/instructions/bytes", "/instructions/cut", "/spec_revision/value"] {
            let mut observed = passing.clone();
            *observed
                .documents
                .get_mut(&Read::MoverEnded)
                .expect("A's document")
                .pointer_mut(path)
                .expect("mutated field") = Value::Null;
            rejects(&observed, SHAPE_HISTORY, "A's document");
        }
    }
    #[test]
    fn the_opening_system_must_have_only_the_old_brief() {
        let passing = met();
        for brief in [String::new(), NEW_BRIEF.into(), format!("{OLD_BRIEF}\n{NEW_BRIEF}")] {
            let mut observed = passing.clone();
            observed.told[0].system = brief.into_bytes().into();
            rejects(&observed, SHAPE_HISTORY, "first query");
        }
    }
    #[test]
    fn every_system_after_the_amendment_must_have_only_the_new_brief() {
        let passing = met();
        for at in [3, 4] {
            for brief in [String::new(), OLD_BRIEF.into(), format!("{OLD_BRIEF}\n{NEW_BRIEF}")] {
                let mut observed = passing.clone();
                observed.told[at].system = brief.into_bytes().into();
                rejects(&observed, SHAPE_INSTRUCTIONS, "every A query");
            }
        }
    }
    #[test]
    fn the_amendment_message_must_arrive_once() {
        let passing = met();
        for at in [3, 4] {
            for messages in [vec![], vec![user(NEW_BRIEF), user(NEW_BRIEF)]] {
                let mut observed = passing.clone();
                observed.told[at].history = messages.into();
                rejects(&observed, SHAPE_INSTRUCTIONS, "whole message once");
            }
        }
        let mut observed = passing.clone();
        observed.told.truncate(3);
        rejects(&observed, SHAPE_INSTRUCTIONS, "every A query");
    }
    #[test]
    fn the_chat_never_inherits_either_brief() {
        let passing = met();
        for brief in [OLD_BRIEF, NEW_BRIEF] {
            let mut observed = passing.clone();
            observed.told[2].system = brief.as_bytes().into();
            rejects(&observed, SHAPE_INSTRUCTIONS, "chat's charter");
        }
        let mut observed = passing.clone();
        observed.told.retain(|told| told.agent != Agent::Shape);
        rejects(&observed, SHAPE_INSTRUCTIONS, "chat's charter");
    }
    #[test]
    fn the_numbered_question_and_its_delivery_are_checked() {
        let passing = met();
        let mut observed = passing.clone();
        observed.told[1] = told(Agent::Mover, OLD_BRIEF, &[]);
        rejects(&observed, SHAPE_INSTRUCTIONS, "numbered question");
        let mut observed = passing.clone();
        observed.told[2].history = Box::new([]);
        rejects(&observed, SHAPE_INSTRUCTIONS, "numbered question");
        let mut observed = passing.clone();
        observed.told.swap(2, 3);
        rejects(&observed, SHAPE_INSTRUCTIONS, "numbered question");
    }
    #[test]
    fn both_numbered_tasks_must_report_in_the_project() {
        let passing = met();
        for read in [Read::MoverEnded, Read::ShapeEnded] {
            for path in ["/chip/task", "/project", "/chip/phase", "/result/value/report/bytes"] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("ended document").pointer_mut(path).expect("mutated field") =
                    Value::Null;
                rejects(&observed, SHAPE_ENDINGS, "original chat and A");
            }
            let mut observed = passing.clone();
            observed.documents.remove(&read);
            rejects(&observed, SHAPE_ENDINGS, "original chat and A");
        }
    }
}

// Act 2, step 5.2 (domain/testing.md, 5.3)
const PERSON_STOP: Promise = Promise::Must("Stop, which is the person's (exact)");
const DELEGATES_GO_ON: Promise =
    Promise::Must("delegates and queued changes going on under a stopped goal (to check, 6.8)");
const OLD_HELPER: Promise = Promise::Must("retiring a charter that still has tasks (to check, 6.8)");
const STOPPED_NEWS: Promise =
    Promise::Add("a stopped goal's news waits for its release, so nothing is relayed by hand");

#[must_use]
pub fn step_5_2(observed: &Observed) -> Vec<Miss> {
    use super::{BOWNER, H1, H1_REVIEWER, HELPER, IMPLEMENTER, OWNER, PAUSE, PROJECT};
    let mut misses = Vec::new();
    for (read, expected, parent) in [
        (Read::ImplementerRunning, IMPLEMENTER, BOWNER),
        (Read::H1Running, H1_REVIEWER, H1),
        (Read::BHeld, BOWNER, PAUSE),
        (Read::BStillHeld, BOWNER, PAUSE),
        (Read::BEnded, BOWNER, PAUSE),
        (Read::OwnerEnded, OWNER, PAUSE),
        (Read::HelperEnded, HELPER, PAUSE),
        (Read::ImplementerEnded, IMPLEMENTER, BOWNER),
        (Read::H1Ended, H1, BOWNER),
    ] {
        let task = task_document(observed, read);
        step_check(
            &mut misses,
            "5.2",
            number(task, "/chip/task") == Some(expected)
                && number(task, "/project") == Some(PROJECT)
                && number(task, "/requester/task") == Some(parent),
            DELEGATES_GO_ON,
            &format!("{read:?} must identify task {expected} under {parent} in project {PROJECT}"),
        );
    }
    for read in [Read::ImplementerRunning, Read::H1Running] {
        let task = task_document(observed, read);
        step_check(
            &mut misses,
            "5.2",
            text(task, "/chip/phase") == "running"
                && text(task, "/chip/details/owner/details/observation/work") == "writing"
                && number(task, "/chip/details/owner/phase/active/running/attempt")
                    .is_some_and(|attempt| number(task, "/chip/details/owner/details/attempt") == Some(attempt))
                && task["result"].is_null(),
            DELEGATES_GO_ON,
            &format!("{read:?} must show an open attempt writing before B's Stop"),
        );
    }
    pause_stop(observed, &mut misses);
    pause_children(observed, &mut misses);
    pause_news(observed, &mut misses);
    misses
}

fn pause_stop(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{B_HOLD, BOWNER, OWNER_REPORT};
    let held = task_document(observed, Read::BHeld);
    let still = task_document(observed, Read::BStillHeld);
    let b = task_document(observed, Read::BEnded);
    let a = task_document(observed, Read::OwnerEnded);
    for task in [held, still] {
        step_check(
            misses,
            "5.2",
            text(task, "/chip/phase") == "held"
                && number(task, "/hold/owner/revision") == Some(B_HOLD)
                && number(task, "/hold/owner/reason/stopped_by/party") == Some(1),
            PERSON_STOP,
            "B must stay held at the person's Stop revision",
        );
    }
    step_check(
        misses,
        "5.2",
        text(a, "/chip/phase") == "ended"
            && text(a, "/result/value/report/bytes") == OWNER_REPORT
            && a["hold"].is_null()
            && check_activity(observed, Read::OwnerEnded)
                .iter()
                .filter(|row| number(row, "/line/milestone/position").is_some())
                .count()
                == 2
            && check_activity(observed, Read::OwnerEnded)
                .iter()
                .filter(|row| number(row, "/line/run_began/attempt").is_some())
                .count()
                == 1
            && pause_activity(observed, Read::OwnerEnded)["cut"] == false
            && pause_activity(observed, Read::OwnerEnded)["before"].is_null()
            && !check_activity(observed, Read::OwnerEnded).iter().any(|row| row["line"].get("decided").is_some()),
        PERSON_STOP,
        "A must finish without a hold or control decision",
    );
    let before = check_activity(observed, Read::BHeld);
    let after: Vec<_> = check_activity(observed, Read::BEnded)
        .into_iter()
        .chain(check_activity(observed, Read::BEndedEarlier))
        .collect();
    let attempts =
        |rows: &[&Value]| rows.iter().filter_map(|row| number(row, "/line/run_began/attempt")).collect::<Vec<_>>();
    let previous = attempts(&before);
    let following = attempts(&after);
    step_check(
        misses,
        "5.2",
        number(held, "/chip/details/owner/details/attempt").is_some()
            && number(held, "/chip/details/owner/details/attempt")
                == number(still, "/chip/details/owner/details/attempt")
            && previous.first().copied() == number(held, "/chip/details/owner/details/attempt")
            && following.len() == previous.len() + 1
            && following.first().copied() == number(b, "/chip/details/owner/details/attempt")
            && following.get(1..) == Some(previous.as_slice())
            && following.first() > following.get(1)
            && after
                .iter()
                .find(|row| number(row, "/line/run_began/attempt").is_some())
                .and_then(|row| number(row, "/position"))
                .zip(number(still, "/activity"))
                .is_some_and(|(began, held_head)| began > held_head)
            && after.iter().all(|row| number(row, "/task") == Some(BOWNER))
            && after.windows(2).all(|pair| {
                number(pair[0], "/position").zip(number(pair[1], "/position")).is_some_and(|(new, old)| new > old)
            })
            && [Read::BHeld, Read::BEnded, Read::BEndedEarlier]
                .iter()
                .all(|read| pause_activity(observed, *read)["cut"] == false)
            && pause_activity(observed, Read::BHeld)["before"].is_null()
            && pause_activity(observed, Read::BEndedEarlier)["before"].is_null(),
        STOPPED_NEWS,
        "B's complete journal must add exactly one attempt after its unchanged held attempt",
    );
}

fn pause_children(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{H1, H1_CODER, H1_DOC, H1_REVIEWER, H1_WORDS, IMPLEMENTER, IMPLEMENTER_RESULT};
    let held = task_document(observed, Read::BHeld);
    let implementer = task_document(observed, Read::ImplementerEnded);
    let h1 = task_document(observed, Read::H1Ended);
    step_check(
        misses,
        "5.2",
        text(implementer, "/chip/phase") == "ended"
            && implementer.pointer("/result/ending/task/done").is_some_and(Value::is_object)
            && text(implementer, "/result/value/report/bytes") == IMPLEMENTER_RESULT,
        DELEGATES_GO_ON,
        "the implementer must finish its report under B",
    );
    step_check(
        misses,
        "5.2",
        text(h1, "/chip/phase") == "ended"
            // The change procedure returns Done(Change) only after landing;
            // ctl renders its commit as hex bytes, not the word "landed".
            && number(h1, "/result/ending/task/done/change/kind") == Some(1)
            && number(h1, "/result/value/change/part/connector") == Some(1)
            && text(h1, "/result/value/change/part/kind") == "result"
            && observed.kit.merged_green
            && observed.kit.main.get(H1_DOC.as_bytes()).is_some_and(|words| words == H1_WORDS.as_bytes()),
        DELEGATES_GO_ON,
        "h1 must end landed with its Markdown on kit's main, merged green",
    );
    let h1_activity = check_activity(observed, Read::H1Ended);
    step_check(
        misses,
        "5.2",
        h1_activity.iter().any(|row| {
            number(row, "/line/heard/from/task") == Some(H1_CODER)
                && row.pointer("/line/heard/kind/result/change").is_some_and(Value::is_object)
        }) && h1_activity.iter().any(|row| {
            number(row, "/line/heard/from/task") == Some(H1_REVIEWER)
                && number(row, "/line/heard/kind/result/verdict/code") == Some(1)
        }),
        DELEGATES_GO_ON,
        "h1 must receive its coder's change and its held reviewer's passing verdict",
    );
    let activity = check_activity(observed, Read::BStillHeld);
    let stopped = number(held, "/activity");
    // BHeld's activity head is read at Stop revision B_HOLD, before either
    // fake release. Each ending's journal position must be beyond that head;
    // BStillHeld follows both ended reads and precedes ctl Release.
    for child in [IMPLEMENTER, H1] {
        let heard = activity.iter().position(|row| {
            number(row, "/line/heard/from/task") == Some(child) && row.pointer("/line/heard/kind/result").is_some()
        });
        step_check(
            misses,
            "5.2",
            heard
                .and_then(|at| number(activity[at], "/position"))
                .zip(stopped)
                .is_some_and(|(heard, stop)| heard > stop && stop != 0),
            DELEGATES_GO_ON,
            &format!("B must retain task {child}'s ending after its Stop decision"),
        );
    }
}

fn pause_news(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{Agent, BOWNER_REPORT, H1, HELPER_CHARTER, HELPER_REASON, IMPLEMENTER_RESULT, PAUSE, PAUSE_REPORT};
    let b = task_document(observed, Read::BEnded);
    let helper = task_document(observed, Read::HelperEnded);
    let pause = task_document(observed, Read::PauseEnded);
    let queries: Vec<_> = observed.told.iter().filter(|query| query.agent == Agent::Bowner).collect();
    let resumed = queries.last();
    let label = format!("Result from task {H1}:\n");
    step_check(
        misses,
        "5.2",
        resumed.is_some_and(|query| contains(&query.words, IMPLEMENTER_RESULT) && contains(&query.words, &label))
            && queries
                .iter()
                .rev()
                .skip(1)
                .all(|query| !contains(&query.words, IMPLEMENTER_RESULT) && !contains(&query.words, &label))
            && resumed.is_some_and(|query| {
                calls::occurrences(&query.words, IMPLEMENTER_RESULT.as_bytes()) == 1
                    && calls::occurrences(&query.words, label.as_bytes()) == 1
            }),
        STOPPED_NEWS,
        "only B's query after Release must receive both endings once",
    );
    step_check(
        misses,
        "5.2",
        number(helper, "/chip/executor/agent/charter") == Some(HELPER_CHARTER)
            && number(helper, "/result/ending/task/cancelled/by/task") == Some(PAUSE)
            && text(helper, "/result/ending/task/cancelled/reason/bytes") == HELPER_REASON
            && text(helper, "/chip/phase") == "ended"
            && number(helper, "/chip/details/owner/details/live_delegates") == Some(0),
        OLD_HELPER,
        "the helper subtree must end cancelled by the director with its reason; charter retirement stays out",
    );
    for (task, expected, label) in [(b, BOWNER_REPORT, "B"), (pause, PAUSE_REPORT, "chat")] {
        step_check(
            misses,
            "5.2",
            text(task, "/chip/phase") == "ended" && text(task, "/result/value/report/bytes") == expected,
            STOPPED_NEWS,
            &format!("{label} must finish with its report"),
        );
    }
    step_check(
        misses,
        "5.2",
        number(pause, "/chip/task") == Some(PAUSE)
            && observed.told.iter().any(|query| query.agent == Agent::Pause && contains(&query.words, BOWNER_REPORT)),
        STOPPED_NEWS,
        "the chat must hear B's report before finishing",
    );
}

fn pause_activity(observed: &Observed, read: Read) -> &Value {
    let doc = document(observed, read);
    doc.get("activity").unwrap_or(doc)
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;

    fn task(task: u64, parent: u64) -> Value {
        json!({"task":{"chip":{"task":task,"phase":"ended","details":{"owner":{"details":{"attempt":1,"live_delegates":0}}}},
            "project":PROJECT,"requester":{"task":parent},"hold":null,
            "result":{"ending":{"task":{"done":{}}}}},
            "activity":{"rows":[],"cut":false,"before":null,"after":null}})
    }
    fn documents() -> BTreeMap<Read, Value> {
        let mut documents = BTreeMap::from([
            (Read::ImplementerRunning, task(IMPLEMENTER, BOWNER)),
            (Read::H1Running, task(H1_REVIEWER, H1)),
            (Read::BHeld, task(BOWNER, PAUSE)),
            (Read::BStillHeld, task(BOWNER, PAUSE)),
            (Read::BEnded, task(BOWNER, PAUSE)),
            (
                Read::BEndedEarlier,
                json!({"activity":{"rows":[{"position":2,"line":{"run_began":{"attempt":1}}}],"before":null,"cut":false}}),
            ),
            (Read::OwnerEnded, task(OWNER, PAUSE)),
            (Read::HelperEnded, task(HELPER, PAUSE)),
            (Read::ImplementerEnded, task(IMPLEMENTER, BOWNER)),
            (Read::H1Ended, task(H1, BOWNER)),
            (Read::PauseEnded, task(PAUSE, 0)),
        ]);
        for read in [Read::ImplementerRunning, Read::H1Running] {
            let document = documents.get_mut(&read).expect("running");
            document["task"]["chip"]["phase"] = "running".into();
            document["task"]["chip"]["details"]["owner"]["phase"] = json!({"active":{"running":{"attempt":1}}});
            document["task"]["chip"]["details"]["owner"]["details"]["observation"] = json!({"work":"writing"});
            document["task"]["result"] = Value::Null;
        }
        for read in [Read::BHeld, Read::BStillHeld] {
            let document = documents.get_mut(&read).expect("held");
            document["task"]["chip"]["phase"] = "held".into();
            document["task"]["hold"] = json!({"owner":{"revision":B_HOLD,"reason":{"stopped_by":{"party":1}}}});
        }
        documents.get_mut(&Read::BHeld).expect("held")["task"]["activity"] = 7.into();
        documents.get_mut(&Read::BStillHeld).expect("still")["task"]["activity"] = 10.into();
        documents.get_mut(&Read::BHeld).expect("held")["activity"]["rows"] =
            json!([{"position":2,"line":{"run_began":{"attempt":1}}}]);
        documents.get_mut(&Read::BStillHeld).expect("still")["activity"]["rows"] = json!([
            {"position":10,"line":{"heard":{"from":{"task":H1},"kind":{"result":"done"}}}},
            {"position":8,"line":{"heard":{"from":{"task":IMPLEMENTER},"kind":{"result":"done"}}}},
            {"position":7,"line":{"milestone":{"position":7}}},
        ]);
        documents.get_mut(&Read::BEnded).expect("B")["activity"]["rows"] =
            json!([{"position":12,"line":{"run_began":{"attempt":2}}}]);
        documents.get_mut(&Read::BEnded).expect("B")["task"]["chip"]["details"]["owner"]["details"]["attempt"] =
            2.into();
        for (read, report) in [
            (Read::OwnerEnded, OWNER_REPORT),
            (Read::BEnded, BOWNER_REPORT),
            (Read::ImplementerEnded, IMPLEMENTER_RESULT),
            (Read::PauseEnded, PAUSE_REPORT),
        ] {
            documents.get_mut(&read).expect("report")["task"]["result"]["value"] = json!({"report":{"bytes":report}});
        }
        let change = json!({"kind":1,"part":{"connector":1,"kind":"result","key":{"number":1}}});
        documents.get_mut(&Read::H1Ended).expect("h1")["task"]["result"] =
            json!({"ending":{"task":{"done":{"change":change}}},"value":{"change":change}});
        documents.get_mut(&Read::HelperEnded).expect("helper")["task"]["result"]["ending"] =
            json!({"task":{"cancelled":{"by":{"task":PAUSE},"reason":{"bytes":HELPER_REASON}}}});
        documents.get_mut(&Read::OwnerEnded).expect("A")["activity"]["rows"] = json!([
            {"position":5,"line":{"milestone":{"position":5}}},
            {"position":2,"line":{"run_began":{"attempt":1}}},
            {"position":1,"line":{"milestone":{"position":1}}},
        ]);
        documents.get_mut(&Read::H1Ended).expect("h1")["activity"]["rows"] = json!([
            {"position":3,"line":{"heard":{"from":{"task":H1_REVIEWER},"kind":{"result":{"verdict":{"code":1}}}}}},
            {"position":2,"line":{"heard":{"from":{"task":H1_CODER},"kind":{"result":{"change":{}}}}}},
        ]);
        documents.get_mut(&Read::HelperEnded).expect("helper")["task"]["chip"]["executor"] =
            json!({"agent":{"charter":HELPER_CHARTER}});
        for document in documents.values_mut() {
            let task = number(document, "/task/chip/task").unwrap_or(BOWNER);
            for row in document["activity"]["rows"].as_array_mut().expect("rows") {
                *row = json!({"event":{"owner":{"source":{"task":task,"position":row["position"].clone(),"line":row["line"].clone()}}}});
            }
        }
        documents
    }
    fn met() -> Observed {
        let documents = documents();
        let told = |agent, words: String| Told {
            system: Box::new([]),
            agent,
            words: words.into_bytes().into(),
            history: Box::new([]),
            model: Box::new([]),
            credential: None,
        };
        Observed {
            documents,
            commands: Vec::new(),
            told: vec![
                told(Agent::Bowner, "Delegated".into()),
                told(Agent::Bowner, format!("{IMPLEMENTER_RESULT}\nResult from task {H1}:\nlanded")),
                told(Agent::Pause, BOWNER_REPORT.into()),
            ],
            kit: Kit {
                records: Vec::new(),
                pulls: Vec::new(),
                main: BTreeMap::from([(H1_DOC.as_bytes().to_vec(), H1_WORDS.as_bytes().to_vec())]),
                merged_green: true,
            },
        }
    }
    #[test]
    fn pause_checkpoint_met() {
        assert_met(&step_5_2(&met()));
    }

    #[test]
    fn every_required_document_field_rejects_a_mutation() {
        let passing = met();
        let required = [
            (Read::BHeld, "/task/chip/task"),
            (Read::BHeld, "/task/project"),
            (Read::BHeld, "/task/requester/task"),
            (Read::BHeld, "/task/chip/phase"),
            (Read::BHeld, "/task/hold/owner/revision"),
            (Read::BHeld, "/task/hold/owner/reason/stopped_by/party"),
            (Read::BStillHeld, "/task/chip/phase"),
            (Read::BStillHeld, "/task/hold/owner/revision"),
            (Read::BStillHeld, "/task/chip/details/owner/details/attempt"),
            (Read::BEnded, "/task/chip/details/owner/details/attempt"),
            (Read::BHeld, "/activity/cut"),
            (Read::BEndedEarlier, "/activity/cut"),
            (Read::BEndedEarlier, "/activity/rows/0/event/owner/source/line/run_began/attempt"),
            (Read::BEnded, "/activity/cut"),
            (Read::BEnded, "/task/result/value/report/bytes"),
            (Read::OwnerEnded, "/task/result/value/report/bytes"),
            (Read::OwnerEnded, "/task/chip/phase"),
            (Read::ImplementerEnded, "/task/result/value/report/bytes"),
            (Read::ImplementerEnded, "/task/result/ending/task/done"),
            (Read::H1Ended, "/task/result/ending/task/done/change/kind"),
            (Read::H1Ended, "/task/result/value/change/part/connector"),
            (Read::H1Ended, "/task/result/value/change/part/kind"),
            (Read::BStillHeld, "/activity/rows/0/event/owner/source/line/heard/from/task"),
            (Read::BStillHeld, "/activity/rows/1/event/owner/source/line/heard/from/task"),
            (Read::BHeld, "/task/activity"),
            (Read::BStillHeld, "/activity/rows/0/event/owner/source/position"),
            (Read::BStillHeld, "/activity/rows/1/event/owner/source/position"),
            (Read::H1Ended, "/activity/rows/0/event/owner/source/line/heard/from/task"),
            (Read::H1Ended, "/activity/rows/0/event/owner/source/line/heard/kind/result/verdict/code"),
            (Read::H1Ended, "/activity/rows/1/event/owner/source/line/heard/from/task"),
            (Read::OwnerEnded, "/activity/rows/0/event/owner/source/line/milestone/position"),
            (Read::OwnerEnded, "/activity/rows/1/event/owner/source/line/run_began/attempt"),
            (Read::HelperEnded, "/task/chip/executor/agent/charter"),
            (Read::HelperEnded, "/task/result/ending/task/cancelled/by/task"),
            (Read::HelperEnded, "/task/result/ending/task/cancelled/reason/bytes"),
            (Read::HelperEnded, "/task/chip/details/owner/details/live_delegates"),
            (Read::PauseEnded, "/task/result/value/report/bytes"),
            (Read::PauseEnded, "/task/chip/task"),
        ];
        for (read, path) in required {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
            assert!(!step_5_2(&observed).is_empty(), "{read:?}{path}");
        }
        for read in [Read::ImplementerRunning, Read::H1Running] {
            for path in [
                "/task/chip/task",
                "/task/project",
                "/task/requester/task",
                "/task/chip/phase",
                "/task/chip/details/owner/phase/active/running/attempt",
                "/task/chip/details/owner/details/attempt",
                "/task/chip/details/owner/details/observation/work",
            ] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("running").pointer_mut(path).expect("field") = Value::Null;
                assert!(!step_5_2(&observed).is_empty(), "{read:?}{path}");
            }
            let mut observed = passing.clone();
            observed.documents.get_mut(&read).expect("running")["task"]["result"] = json!({"done":{}});
            assert!(!step_5_2(&observed).is_empty(), "{read:?} already ended");
        }
        for &(read, _) in crate::case::acts::READS_5_2 {
            let mut observed = passing.clone();
            observed.documents.remove(&read);
            assert!(!step_5_2(&observed).is_empty(), "missing {read:?}");
        }
    }
    #[test]
    fn news_cannot_arrive_early_twice_or_without_both_endings() {
        let passing = met();
        let mut early = passing.clone();
        early.told[0].words = early.told[1].words.clone();
        let mut missing_h1 = passing.clone();
        missing_h1.told[1].words = IMPLEMENTER_RESULT.as_bytes().into();
        let mut twice = passing.clone();
        twice.told[1].words =
            format!("{} {IMPLEMENTER_RESULT}", String::from_utf8_lossy(&twice.told[1].words)).into_bytes().into();
        let mut twice_h1 = passing.clone();
        twice_h1.told[1].words =
            format!("{} Result from task {H1}:\n", String::from_utf8_lossy(&twice_h1.told[1].words))
                .into_bytes()
                .into();
        let mut no_chat_delivery = passing.clone();
        no_chat_delivery.told[2].words = Box::new([]);
        let mut no_queries = passing.clone();
        no_queries.told.clear();
        for (label, observed) in [
            ("early", early),
            ("missing h1", missing_h1),
            ("twice", twice),
            ("twice h1", twice_h1),
            ("chat", no_chat_delivery),
            ("no queries", no_queries),
        ] {
            assert!(!step_5_2(&observed).is_empty(), "{label}");
        }
    }
    #[test]
    fn stop_order_a_hold_and_the_merge_are_checked() {
        let passing = met();
        let mut red = passing.clone();
        red.kit.merged_green = false;
        let mut missing_file = passing.clone();
        missing_file.kit.main.clear();
        let mut early = passing.clone();
        early.documents.get_mut(&Read::BHeld).expect("held")["task"]["activity"] = 10.into();
        let mut a_held = passing.clone();
        a_held.documents.get_mut(&Read::OwnerEnded).expect("A")["task"]["hold"] = json!({"owner":{}});
        let mut a_released = passing.clone();
        a_released.documents.get_mut(&Read::OwnerEnded).expect("A")["activity"]["rows"]
            .as_array_mut()
            .expect("rows")
            .push(json!({"event":{"owner":{"source":{"line":{"milestone":{"position":3}}}}}}));
        let mut a_retried = passing.clone();
        a_retried.documents.get_mut(&Read::OwnerEnded).expect("A")["activity"]["rows"]
            .as_array_mut()
            .expect("rows")
            .push(json!({"event":{"owner":{"source":{"line":{"run_began":{"attempt":3}}}}}}));
        let mut incomplete = passing.clone();
        incomplete.documents.get_mut(&Read::BEndedEarlier).expect("earlier")["activity"]["before"] =
            json!({"value":"older"});
        let mut unordered = passing.clone();
        unordered.documents.get_mut(&Read::BEndedEarlier).expect("earlier")["activity"]["rows"][0]["event"]["owner"]
            ["source"]["position"] = 13.into();
        for (label, observed) in [
            ("red", red),
            ("missing file", missing_file),
            ("early", early),
            ("A held", a_held),
            ("A released", a_released),
            ("A retried", a_retried),
            ("incomplete", incomplete),
            ("unordered", unordered),
        ] {
            assert!(!step_5_2(&observed).is_empty(), "{label}");
        }
    }
}

// Act 2, step 5.5 (domain/testing.md, 5.3)
const PLAN_MESSAGES: Promise = Promise::Must("a question and its answer (exact)");
const PLAN_DEADLINES: Promise = Promise::Must("a mission and sub-owner carry their deadlines");
const PLAN_DEPTH: Promise = Promise::Must("a sub-owner cannot start one of its own (exact)");
const PLAN_FRESH: Promise = Promise::Must("the fresh owner recalls its plan");
const PLAN_ATTEMPTS: Promise = Promise::Add("one run per task, so there is no old session to stop");

/// Only the carried deadlines are checked; their effect and phase policy stay out.
#[must_use]
pub fn step_5_5(observed: &Observed) -> Vec<Miss> {
    use super::{
        PLAN_ANSWER, PLAN_DIRECTOR, PLAN_NOTE, PLAN_NOTE_NUMBER, PLAN_REPORT, PLAN_WORDS, PLANNER, PLANNER_REPORT,
        SUBOWNER, SUBOWNER_REPORT,
    };
    let mut misses = Vec::new();
    for (read, expected, parent, report) in [
        (Read::PlanEnded, PLAN_DIRECTOR, None, PLAN_REPORT),
        (Read::PlannerEnded, PLANNER, Some(PLAN_DIRECTOR), PLANNER_REPORT),
        (Read::SubownerEnded, SUBOWNER, Some(PLANNER), SUBOWNER_REPORT),
    ] {
        let task = task_document(observed, read);
        step_check(
            &mut misses,
            "5.5",
            number(task, "/chip/task") == Some(expected)
                && number(task, "/project") == Some(PROJECT)
                && parent.is_none_or(|parent| number(task, "/requester/task") == Some(parent))
                && text(task, "/chip/phase") == "ended"
                && text(task, "/result/value/report/bytes") == report,
            PLAN_MESSAGES,
            &format!("{read:?} must identify the original ended task and report"),
        );
    }
    plan_authority(observed, &mut misses);
    plan_messages(observed, &mut misses);
    let answer_at = observed
        .told
        .iter()
        .position(|told| told.agent == Agent::Planner && user_words(told).any(|words| contains(words, PLAN_ANSWER)));
    let pending = document(observed, Read::PlanAnswerPending);
    step_check(
        &mut misses,
        "5.5",
        number(pending, "/task") == Some(PLAN_DIRECTOR)
            && text(pending, "/writing/text") == super::PLAN_ANSWER_PENDING
            && pending.pointer("/writing/cut") == Some(&Value::Bool(false)),
        PLAN_FRESH,
        "the director's live answer piece must be held until A's first run has parked",
    );
    let parked = task_document(observed, Read::PlannerParked);
    step_check(
        &mut misses,
        "5.5",
        number(parked, "/chip/task") == Some(PLANNER)
            && text(parked, "/chip/phase") == "idle"
            && answer_at.is_some_and(|at| plan_fresh(&observed.told[at]))
            && plan_note_recalled(observed, Agent::Planner, PLAN_NOTE_NUMBER, PLAN_NOTE, PLAN_WORDS)
            && answer_at.is_some_and(|at| {
                plan_tool_results(observed, Agent::Planner, b"recall").iter().any(|(recalled, _, _)| *recalled > at)
            }),
        PLAN_FRESH,
        "A must park, start fresh on its answer, and recall its written plan in that run",
    );
    step_check(
        &mut misses,
        "5.5",
        plan_attempts(observed, &[Read::PlannerEnded, Read::PlannerEarlier, Read::PlannerFirst], PLANNER, 2),
        PLAN_ATTEMPTS,
        "A's complete ordered activity must end each attempt before the next begins, and none may fail",
    );
    misses
}

fn plan_authority(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{A_DEADLINE, A1_DEADLINE, PLANNER, SPEND_DEFICIT, SUBOWNER, SUBOWNER_REFUSAL};
    let a = task_document(observed, Read::PlannerEnded);
    let a1 = task_document(observed, Read::SubownerEnded);
    step_check(
        misses,
        "5.5",
        number(a, "/authority/authority/deadline") == Some(A_DEADLINE)
            && number(a1, "/authority/authority/deadline") == Some(A1_DEADLINE)
            && number(a1, "/authority/authority/deadline")
                .zip(number(a, "/authority/authority/deadline"))
                .is_some_and(|(child, owner)| child < owner)
            && number(a, "/priority") == Some(1)
            && number(a, "/goal") == Some(PLANNER),
        PLAN_DEADLINES,
        "A must be tracked at priority 1 and both delegated deadlines must be carried, A1's before A's",
    );
    let refused = plan_tool_results(observed, Agent::Subowner, b"delegate");
    step_check(
        misses,
        "5.5",
        number(a1, "/authority/authority/delegation/depth") == Some(0)
            && refused.len() == 1
            && refused
                .iter()
                .all(|(_, _, output)| contains(output, SUBOWNER_REFUSAL) && !contains(output, SPEND_DEFICIT))
            && number(a1, "/chip/details/owner/details/live_delegates") == Some(0)
            && a1["delegates"].as_array().is_some_and(Vec::is_empty)
            && observed.documents.values().all(|doc| number(&doc["task"], "/requester/task") != Some(SUBOWNER)),
        PLAN_DEPTH,
        "A1 must hear its delegation refusal for depth alone, with depth 0 and no child in any read document",
    );
}

fn plan_messages(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{PLAN_ANSWER, PLAN_ANSWER_NUMBER, PLAN_DIRECTOR, PLAN_QUESTION_NUMBER, PLAN_WORDS, PLANNER};
    let questions = plan_tool_results(observed, Agent::Planner, b"message");
    let answers = plan_tool_results(observed, Agent::Plan, b"message");
    let question_at = observed
        .told
        .iter()
        .position(|told| told.agent == Agent::Plan && user_words(told).any(|words| contains(words, PLAN_WORDS)));
    let answer_at = observed
        .told
        .iter()
        .position(|told| told.agent == Agent::Planner && user_words(told).any(|words| contains(words, PLAN_ANSWER)));
    step_check(
        misses,
        "5.5",
        questions.len() == 1
            && questions.iter().all(|(_, args, output)| {
                args["target"] == PLAN_DIRECTOR
                    && args["form"] == "question"
                    && args["words"] == PLAN_WORDS
                    && contains(output, &format!("Message {PLAN_QUESTION_NUMBER} sent"))
            })
            && answers.len() == 1
            && answers.iter().all(|(_, args, output)| {
                args["target"] == PLANNER
                    && args["form"] == "answer"
                    && args["question"] == PLAN_QUESTION_NUMBER
                    && args["words"] == PLAN_ANSWER
                    && contains(output, &format!("Message {PLAN_ANSWER_NUMBER} sent"))
            })
            && [Read::PlannerEnded, Read::PlannerEarlier, Read::PlannerFirst]
                .iter()
                .flat_map(|read| check_activity(observed, *read))
                .filter(|row| {
                    number(row, "/line/words/message") == Some(PLAN_ANSWER_NUMBER)
                        && number(row, "/line/words/by/task") == Some(PLAN_DIRECTOR)
                        && number(row, "/line/words/kind/answer/question") == Some(PLAN_QUESTION_NUMBER)
                        && text(row, "/line/words/words") == PLAN_ANSWER
                })
                .count()
                == 1
            && plan_deliveries(observed, Agent::Plan, PLAN_WORDS) == 1
            && plan_deliveries(observed, Agent::Planner, PLAN_ANSWER) == 1
            && question_at.zip(answer_at).is_some_and(|(question, answer)| question < answer),
        PLAN_MESSAGES,
        "the numbered plan question and its answer must each be sent and delivered once, in order",
    );
}

// Histories repeat earlier turns. Count new user input once within a growing
// history, and count it again if delivered to another fresh conversation.
fn plan_deliveries(observed: &Observed, agent: Agent, words: &str) -> usize {
    use skein_fake_llm_domain::api::{Part, Role};
    let mut previous: &[skein_fake_llm_domain::api::Message] = &[];
    let mut count = 0;
    for told in observed.told.iter().filter(|told| told.agent == agent) {
        let offset = if told.history.starts_with(previous) { previous.len() } else { 0 };
        count += told.history[offset..]
            .iter()
            .filter(|message| message.role == Role::User)
            .flat_map(|message| &message.parts)
            .map(|part| match part {
                Part::Text { text } | Part::ToolOutput { output: text, .. } => {
                    calls::occurrences(text, words.as_bytes())
                }
                Part::Opaque { .. } | Part::ToolCall { .. } => 0,
            })
            .sum::<usize>();
        previous = &told.history;
    }
    count
}

// Match actual tool outputs to their assistant call identities, never to prose
// repeating the expected words. A replayed prefix is still one completed call.
fn plan_tool_results(observed: &Observed, agent: Agent, tool: &[u8]) -> Vec<(usize, Value, Vec<u8>)> {
    use skein_fake_llm_domain::api::{Part, Role};
    let mut seen = BTreeSet::new();
    let mut results = Vec::new();
    for (at, told) in observed.told.iter().enumerate().filter(|(_, told)| told.agent == agent) {
        let mut calls = BTreeMap::new();
        for message in &told.history {
            for part in &message.parts {
                match part {
                    Part::ToolCall { id, name, arguments }
                        if message.role == Role::Assistant && name.as_ref() == tool =>
                    {
                        calls.insert(id.as_ref(), arguments);
                    }
                    Part::ToolOutput { id, output, .. } if message.role == Role::User => {
                        if !seen.contains(id.as_ref())
                            && let Some(arguments) = calls.get(id.as_ref())
                            && let Ok(args) = serde_json::from_slice(arguments)
                            && seen.insert(id.to_vec())
                        {
                            results.push((at, args, output.to_vec()));
                        }
                    }
                    Part::Text { .. } | Part::Opaque { .. } | Part::ToolCall { .. } | Part::ToolOutput { .. } => {}
                }
            }
        }
    }
    results
}

fn plan_fresh(told: &Told) -> bool {
    use skein_fake_llm_domain::api::{Part, Role};
    !told.history.is_empty()
        && told.history.iter().all(|message| {
            message.role == Role::User && message.parts.iter().all(|part| matches!(part, Part::Text { .. }))
        })
}

fn plan_note_recalled(observed: &Observed, agent: Agent, note: u64, description: &str, body: &str) -> bool {
    let writes = plan_tool_results(observed, agent, b"note");
    let recalls = plan_tool_results(observed, agent, b"recall");
    writes.iter().any(|(written, args, output)| {
        args["scope"] == serde_json::json!({"kind":"project","project":PROJECT})
            && args["description"] == description
            && args["body"] == body
            && contains(output, &format!("Note {note} revision 1 written"))
            && recalls.iter().any(|(recalled, args, output)| {
                written < recalled
                    && args["kind"] == "name"
                    && args["name"] == note
                    && contains(output, &format!("Note {note} revision 1: {description}\n{body}\n"))
            })
    })
}

fn plan_attempts(observed: &Observed, reads: &[Read], task: u64, minimum: usize) -> bool {
    let rows: Vec<_> = reads.iter().flat_map(|read| check_activity(observed, *read)).collect();
    let complete = reads.iter().all(|read| pause_activity(observed, *read)["cut"] == false)
        && reads.last().is_some_and(|read| pause_activity(observed, *read)["before"] == Value::Null)
        && reads.first().is_some_and(|read| pause_activity(observed, *read)["after"] == Value::Null);
    let ordered = rows
        .windows(2)
        .all(|pair| number(pair[0], "/position").zip(number(pair[1], "/position")).is_some_and(|(new, old)| new > old));
    let mut open = None;
    let mut attempts = BTreeSet::new();
    let mut valid = complete && ordered && !rows.is_empty();
    for row in rows.iter().rev() {
        valid &= number(row, "/task") == Some(task);
        if let Some(began) = row["line"].get("run_began") {
            valid &= open.is_none();
            open = number(began, "/attempt");
            valid &= open.is_some_and(|attempt| attempts.insert(attempt));
        }
        if let Some(ended) = row["line"].get("run_ended") {
            valid &= open.is_some() && open.take() == number(ended, "/attempt");
            // A failed start's retry would hide it: no attempt may fail.
            valid &= ended["end"].get("failed").is_none();
        }
    }
    valid && open.is_none() && attempts.len() >= minimum
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn input(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn tool(id: &str, name: &str, args: &Value, output: &str) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                parts: Box::new([Part::ToolCall {
                    id: id.as_bytes().into(),
                    name: name.as_bytes().into(),
                    arguments: serde_json::to_vec(args).expect("args").into(),
                }]),
            },
            Message {
                role: Role::User,
                parts: Box::new([Part::ToolOutput {
                    id: id.as_bytes().into(),
                    output: output.as_bytes().into(),
                    is_error: false,
                }]),
            },
        ]
    }
    fn told(agent: Agent, history: &[Message]) -> Told {
        Told::new(agent.cue(), b"fake", None, history).expect("agent cue")
    }
    fn met() -> Observed {
        let mut documents = BTreeMap::new();
        for (read, task, parent, report) in [
            (Read::PlanEnded, PLAN_DIRECTOR, 0, PLAN_REPORT),
            (Read::PlannerEnded, PLANNER, PLAN_DIRECTOR, PLANNER_REPORT),
            (Read::SubownerEnded, SUBOWNER, PLANNER, SUBOWNER_REPORT),
        ] {
            documents.insert(read, json!({"task":{
                "chip":{"task":task,"phase":"ended","details":{"owner":{"details":{"live_delegates":0}}}},
                "project":PROJECT,"requester":{"task":parent},"priority":1,"goal":PLANNER,"delegates":[],
                "authority":{"authority":{"deadline":if task == SUBOWNER { A1_DEADLINE } else { A_DEADLINE },"delegation":{"depth":0}}},
                "result":{"value":{"report":{"bytes":report}}}
            }}));
        }
        documents.insert(Read::PlannerParked, json!({"task":{"chip":{"task":PLANNER,"phase":"idle"}}}));
        documents.insert(
            Read::PlanAnswerPending,
            json!({"task":PLAN_DIRECTOR,
            "writing":{"text":PLAN_ANSWER_PENDING,"cut":false}}),
        );
        let rows: Vec<_> = [
            json!({"run_ended":{"attempt":2,"end":"finished"}}),
            json!({"run_began":{"attempt":2}}),
            json!({"words":{"message":PLAN_ANSWER_NUMBER,"by":{"task":PLAN_DIRECTOR},
                "kind":{"answer":{"question":PLAN_QUESTION_NUMBER}},"words":PLAN_ANSWER}}),
            json!({"run_ended":{"attempt":1,"end":"parked"}}),
            json!({"run_began":{"attempt":1}}),
        ]
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            json!({"event":{"owner":{"source":{
                "task":PLANNER,"position":5-at,"line":line
            }}}})
        })
        .collect();
        documents.get_mut(&Read::PlannerEnded).expect("A")["activity"] =
            json!({"cut":false,"before":{"value":"older"},"after":null,"rows":rows[..2]});
        documents.insert(
            Read::PlannerEarlier,
            json!({"cut":false,"before":{"value":"first"},"after":{"value":"newer"},"rows":rows[2..4]}),
        );
        documents
            .insert(Read::PlannerFirst, json!({"cut":false,"before":null,"after":{"value":"newer"},"rows":rows[4..]}));
        let mut opening = tool(
            "note",
            "note",
            &json!({"scope":{"kind":"project","project":PROJECT},
            "description":PLAN_NOTE,"body":PLAN_WORDS}),
            "Note 1 revision 1 written",
        )
        .to_vec();
        let noted = told(Agent::Planner, &opening);
        opening.extend(tool(
            "question",
            "message",
            &json!({"target":PLAN_DIRECTOR,"form":"question","words":PLAN_WORDS}),
            &format!("Message {PLAN_QUESTION_NUMBER} sent"),
        ));
        let mut director = vec![input(PLAN_WORDS)];
        director.extend(tool(
            "answer",
            "message",
            &json!({"target":PLANNER,"form":"answer","question":PLAN_QUESTION_NUMBER,
            "words":PLAN_ANSWER}),
            &format!("Message {PLAN_ANSWER_NUMBER} sent"),
        ));
        let mut next = vec![input(PLAN_ANSWER)];
        let fresh = told(Agent::Planner, &next);
        next.extend(tool(
            "recall",
            "recall",
            &json!({"kind":"name","name":PLAN_NOTE_NUMBER}),
            &format!("Note {PLAN_NOTE_NUMBER} revision 1: {PLAN_NOTE}\n{PLAN_WORDS}\n"),
        ));
        Observed {
            documents,
            commands: Vec::new(),
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: true },
            told: vec![
                noted,
                told(Agent::Planner, &opening),
                told(Agent::Plan, &director),
                fresh,
                told(Agent::Planner, &next),
                told(Agent::Subowner, &tool("refused", "delegate", &json!({"batch":[{}]}), SUBOWNER_REFUSAL)),
            ],
        }
    }
    #[test]
    fn answered_plan_meets_the_checkpoint_and_replayed_prefixes_are_one_delivery() {
        let mut observed = met();
        assert_met(&step_5_5(&observed));
        observed.told.push(observed.told[4].clone());
        assert_met(&step_5_5(&observed));
    }
    #[test]
    fn every_required_document_field_rejects_a_mutation() {
        let passing = met();
        for (read, paths) in [
            (
                Read::PlanEnded,
                vec!["/task/chip/task", "/task/project", "/task/chip/phase", "/task/result/value/report/bytes"],
            ),
            (
                Read::PlannerEnded,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/requester/task",
                    "/task/chip/phase",
                    "/task/result/value/report/bytes",
                    "/task/priority",
                    "/task/goal",
                    "/task/authority/authority/deadline",
                    "/activity/cut",
                    "/activity/after",
                ],
            ),
            (
                Read::SubownerEnded,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/requester/task",
                    "/task/chip/phase",
                    "/task/result/value/report/bytes",
                    "/task/authority/authority/deadline",
                    "/task/authority/authority/delegation/depth",
                    "/task/chip/details/owner/details/live_delegates",
                    "/task/delegates",
                ],
            ),
            (Read::PlannerParked, vec!["/task/chip/task", "/task/chip/phase"]),
            (
                Read::PlannerEarlier,
                vec![
                    "/cut",
                    "/rows/0/event/owner/source/line/words/message",
                    "/rows/0/event/owner/source/line/words/by/task",
                    "/rows/0/event/owner/source/line/words/kind/answer/question",
                    "/rows/0/event/owner/source/line/words/words",
                ],
            ),
            (Read::PlannerFirst, vec!["/cut", "/before"]),
            (Read::PlanAnswerPending, vec!["/task", "/writing/text", "/writing/cut"]),
        ] {
            for path in paths {
                let mut observed = passing.clone();
                let value = observed.documents.get_mut(&read).expect("read").pointer_mut(path).expect("field");
                *value = if value.is_null() { json!("unexpected cursor") } else { Value::Null };
                assert!(!step_5_5(&observed).is_empty(), "{read:?}{path}");
            }
        }
        for &(read, _) in acts::READS_5_5 {
            let mut observed = passing.clone();
            observed.documents.remove(&read);
            assert!(!step_5_5(&observed).is_empty(), "missing {read:?}");
        }
    }
    #[test]
    fn missing_duplicate_reordered_or_resumed_messages_fail() {
        let passing = met();
        for at in 0..passing.clone().told.len() {
            let mut observed = passing.clone();
            observed.told.remove(at);
            // The first note result also appears in the opening's growing prefix.
            if at != 0 {
                assert!(!step_5_5(&observed).is_empty(), "missing query {at}");
            }
        }
        let mut duplicate = passing.clone();
        duplicate.told[2].history = [duplicate.told[2].history.to_vec(), vec![input(PLAN_WORDS)]].concat().into();
        let mut duplicate_answer = passing.clone();
        duplicate_answer.told[3].history =
            [duplicate_answer.told[3].history.to_vec(), vec![input(PLAN_ANSWER)]].concat().into();
        let mut early = passing.clone();
        early.told.swap(2, 3);
        let mut resumed = passing.clone();
        resumed.told[3].history = resumed.told[1].history.clone();
        resumed.told[3].history = [resumed.told[3].history.to_vec(), vec![input(PLAN_ANSWER)]].concat().into();
        let mut wrong_number = passing.clone();
        if let Part::ToolCall { arguments, .. } = &mut wrong_number.told[2].history[1].parts[0] {
            *arguments =
                serde_json::to_vec(&json!({"target":PLANNER,"form":"answer","question":99,"words":PLAN_ANSWER}))
                    .expect("args")
                    .into();
        }
        for observed in [duplicate, duplicate_answer, early, resumed, wrong_number] {
            assert!(!step_5_5(&observed).is_empty());
        }
    }
    #[test]
    fn refusal_must_be_tool_output_and_no_child_may_be_made() {
        let passing = met();
        let mut prose = passing.clone();
        prose.told[5].history = vec![input(SUBOWNER_REFUSAL)].into();
        let mut child = passing.clone();
        child.documents.get_mut(&Read::SubownerEnded).expect("A1")["task"]["delegates"] = json!([{"chip":{"task":4}}]);
        let mut wrong_parent = passing.clone();
        wrong_parent.documents.get_mut(&Read::PlanEnded).expect("director")["task"]["requester"] =
            json!({"task":SUBOWNER});
        let mut forged_output = passing.clone();
        if let Part::ToolOutput { id, .. } = &mut forged_output.told[5].history[1].parts[0] {
            *id = b"unmatched".as_slice().into();
        }
        let mut spend = passing.clone();
        if let Part::ToolOutput { output, .. } = &mut spend.told[5].history[1].parts[0] {
            *output = format!("{SUBOWNER_REFUSAL}spend allowance; ").into_bytes().into();
        }
        for observed in [prose, child, wrong_parent, forged_output, spend] {
            assert!(!step_5_5(&observed).is_empty());
        }
    }
    #[test]
    fn plan_recall_must_follow_its_write_and_attempts_cannot_overlap() {
        let passing = met();
        let mut early = passing.clone();
        early.told.swap(0, 4);
        early.told[1].history = Box::new([]);
        let mut overlap = passing.clone();
        overlap.documents.get_mut(&Read::PlannerEarlier).expect("older")["rows"][0]["event"]["owner"]["source"]["line"] =
            json!({"run_began":{"attempt":2}});
        let mut unordered = passing.clone();
        unordered.documents.get_mut(&Read::PlannerEarlier).expect("older")["rows"][0]["event"]["owner"]["source"]["position"] =
            json!(99);
        let mut unended = passing.clone();
        unended.documents.get_mut(&Read::PlannerEnded).expect("A")["activity"]["rows"][0]["event"]["owner"]["source"]
            ["line"] = json!({});
        let mut failed = passing.clone();
        let rows = failed.documents.get_mut(&Read::PlannerEnded).expect("A")["activity"]["rows"]
            .as_array_mut()
            .expect("A's rows");
        let ending = rows
            .iter_mut()
            .map(|row| &mut row["event"]["owner"]["source"]["line"])
            .find(|line| line.get("run_ended").is_some())
            .expect("an ending");
        ending["run_ended"]["end"] = json!({"failed":{"class":"invalid"}});
        for observed in [early, overlap, unordered, unended, failed] {
            assert!(!step_5_5(&observed).is_empty());
        }
    }
}

// Act 2, step 5.6 (domain/testing.md, 5.3)
const MISREAD_CANCEL: Promise = Promise::Must("cancel a delegate (exact)");
const MISREAD_NOTED: Promise = Promise::Must("the direction as the chat's notes (exact)");
const MISREAD_PRIORITY: Promise = Promise::Must("priorities and preferred host kinds (exact, hosts left out)");
const MISREAD_FRESH_TIMER: Promise =
    Promise::Add("timers are the task's, outside its run, so they cannot block a fresh start");

#[must_use]
pub fn step_5_6(observed: &Observed) -> Vec<Miss> {
    let mut misses = Vec::new();
    misread_helper(observed, &mut misses);
    misread_timer_and_fresh(observed, &mut misses);
    misread_note(observed, &mut misses);
    misses
}

fn misread_helper(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{MISREAD_DIRECTOR, MISREAD_HELPER, MISREAD_REASON};
    let running = task_document(observed, Read::MisreadHelperRunning);
    let ended = task_document(observed, Read::MisreadHelperEnded);
    for (read, task) in [(Read::MisreadHelperRunning, running), (Read::MisreadHelperEnded, ended)] {
        step_check(
            misses,
            "5.6",
            number(task, "/chip/task") == Some(MISREAD_HELPER)
                && number(task, "/project") == Some(PROJECT)
                && number(task, "/requester/task") == Some(MISREAD_DIRECTOR),
            MISREAD_CANCEL,
            &format!("{read:?} must identify the helper under the director in the project"),
        );
    }
    step_check(
        misses,
        "5.6",
        number(running, "/goal") == Some(MISREAD_HELPER)
            && number(running, "/priority") == Some(1)
            && text(running, "/chip/phase") == "running"
            && text(running, "/chip/details/owner/details/observation/work") == "writing"
            && running["result"].is_null(),
        MISREAD_PRIORITY,
        "before cancellation the helper must be a priority-1 goal with its first LLM turn held",
    );
    let calls = plan_tool_results(observed, Agent::Misread, b"cancel");
    step_check(
        misses,
        "5.6",
        calls.len() == 1
            && calls.iter().all(|(_, args, output)| {
                args["target"] == MISREAD_HELPER && args["reason"] == MISREAD_REASON && output.as_slice() == b"Done"
            })
            && text(ended, "/chip/phase") == "ended"
            && number(ended, "/result/ending/task/cancelled/by/task") == Some(MISREAD_DIRECTOR)
            && text(ended, "/result/ending/task/cancelled/reason/bytes") == MISREAD_REASON
            && number(ended, "/chip/details/owner/details/live_delegates") == Some(0)
            && ended["delegates"].as_array().is_some_and(Vec::is_empty)
            && observed.documents.values().all(|doc| number(&doc["task"], "/requester/task") != Some(MISREAD_HELPER))
            && plan_attempts(observed, &[Read::MisreadHelperEnded], MISREAD_HELPER, 1)
            && check_activity(observed, Read::MisreadHelperEnded).iter().any(|row| {
                text(row, "/line/run_ended/end") == "cancelled"
                    && number(row, "/line/run_ended/attempt") == number(ended, "/chip/details/owner/details/attempt")
            }),
        MISREAD_CANCEL,
        "the director's single cancel must end the helper and its subtree, with the reason and no open run",
    );
    let heads = plan_tool_results(observed, Agent::Misread, b"read_branch");
    let recalls = plan_tool_results(observed, Agent::Misread, b"recall");
    step_check(
        misses,
        "5.6",
        heads.len() == 1
            && heads.iter().all(|(read, args, output)| {
                args["repository"] == serde_json::json!({"forge":1,"repository":crate::forge::ID})
                    && args["read"]["branch"] == "main"
                    && output.as_slice() == draft_commit().as_bytes()
                    && recalls.iter().any(|(recalled, _, _)| recalled < read)
            }),
        MISREAD_CANCEL,
        "after the helper ended and its rule was recalled, kit's main must still be the fixture's original commit",
    );
}

fn misread_timer_and_fresh(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{MISREAD_DIRECTOR, MISREAD_TIMER, MISREAD_TIMER_AT};
    let parked = task_document(observed, Read::MisreadParked);
    let fresh = task_document(observed, Read::MisreadFresh);
    let ended = task_document(observed, Read::MisreadEnded);
    for (read, task) in [(Read::MisreadParked, parked), (Read::MisreadFresh, fresh), (Read::MisreadEnded, ended)] {
        step_check(
            misses,
            "5.6",
            number(task, "/chip/task") == Some(MISREAD_DIRECTOR) && number(task, "/project") == Some(PROJECT),
            MISREAD_FRESH_TIMER,
            &format!("{read:?} must identify the director's original chat"),
        );
    }
    for task in [parked, fresh] {
        step_check(
            misses,
            "5.6",
            task.pointer("/subscriptions/entries").and_then(Value::as_array).is_some_and(|entries| {
                entries.len() == 1
                    && entries.iter().all(|entry| {
                        number(entry, "/number") == Some(MISREAD_TIMER)
                            && number(entry, "/kind/timer/at") == Some(MISREAD_TIMER_AT)
                            && entry.pointer("/kind/timer/period") == Some(&Value::Null)
                    })
            }),
            MISREAD_FRESH_TIMER,
            "the same numbered timer must stay armed after the first run ended and during the next run",
        );
    }
    misread_handover(observed, misses);
    misread_conversation(observed, misses);
}

fn misread_handover(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        MISREAD_CORRECTION, MISREAD_CORRECTION_MESSAGE, MISREAD_DIRECTOR, MISREAD_RULE_PENDING, MISREAD_TIMER,
        MISREAD_TIMER_AT,
    };
    let parked = task_document(observed, Read::MisreadParked);
    let fresh = task_document(observed, Read::MisreadFresh);
    let subscriptions = plan_tool_results(observed, Agent::Misread, b"subscribe");
    let pending = document(observed, Read::MisreadRulePending);
    let prior = number(parked, "/chip/details/owner/details/attempt");
    let next = number(fresh, "/chip/details/owner/details/attempt");
    let rows = check_activity(observed, Read::MisreadFresh);
    let stopped = rows.iter().find_map(|row| {
        (number(row, "/line/run_ended/attempt") == prior && text(row, "/line/run_ended/end") == "parked")
            .then(|| number(row, "/position"))
            .flatten()
    });
    let corrected = rows.iter().find_map(|row| {
        (number(row, "/line/words/message") == Some(MISREAD_CORRECTION_MESSAGE)
            && number(row, "/line/words/by/person") == Some(1)
            && text(row, "/line/words/kind") == "words"
            && text(row, "/line/words/words") == MISREAD_CORRECTION)
            .then(|| number(row, "/position"))
            .flatten()
    });
    let began = rows.iter().find_map(|row| {
        (number(row, "/line/run_began/attempt") == next
            && number(row, "/line/run_began/why/message/message") == Some(MISREAD_CORRECTION_MESSAGE))
        .then(|| number(row, "/position"))
        .flatten()
    });
    step_check(
        misses,
        "5.6",
        text(parked, "/chip/phase") == "idle"
            && text(fresh, "/chip/phase") == "running"
            && text(fresh, "/chip/details/owner/details/observation/work") == "writing"
            && prior.zip(next).is_some_and(|(old, new)| old != new)
            && rows.iter().all(|row| number(row, "/task") == Some(MISREAD_DIRECTOR))
            && stopped
                .zip(corrected)
                .zip(began)
                .is_some_and(|((stop, correction), began)| stop != 0 && stop < correction && correction < began)
            && subscriptions.len() == 1
            && subscriptions.iter().all(|(_, args, output)| {
                args["kind"] == "timer"
                    && args["at"] == MISREAD_TIMER_AT
                    && contains(output, &format!("Subscription {MISREAD_TIMER} added"))
            })
            && number(pending, "/task") == Some(MISREAD_DIRECTOR)
            && text(pending, "/writing/text") == MISREAD_RULE_PENDING
            && pending.pointer("/writing/cut") == Some(&Value::Bool(false)),
        MISREAD_FRESH_TIMER,
        "the parked chat must start its next run on the person's later correction, while its note-recall turn is held",
    );
}

fn misread_conversation(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{MISREAD_CORRECTION, MISREAD_REPORT};
    let ended = task_document(observed, Read::MisreadEnded);
    let helper: Vec<_> =
        observed.told.iter().enumerate().filter(|(_, told)| told.agent == Agent::MisreadHelper).collect();
    let corrected_at = observed.told.iter().position(|told| {
        told.agent == Agent::Misread && user_words(told).any(|words| contains(words, MISREAD_CORRECTION))
    });
    step_check(
        misses,
        "5.6",
        helper.first().is_some_and(|(at, _)| corrected_at.is_some_and(|corrected| *at < corrected))
            && helper.iter().all(|(_, told)| plan_fresh(told)),
        MISREAD_PRIORITY,
        "the helper's first query must be in flight, with no completed turn, before the fresh director acts",
    );
    let queries: Vec<_> = observed.told.iter().filter(|told| told.agent == Agent::Misread).collect();
    let at = queries.iter().position(|told| user_words(told).any(|words| contains(words, MISREAD_CORRECTION)));
    step_check(
        misses,
        "5.6",
        at.is_some_and(|at| {
            at > 0
                && plan_fresh(queries[at])
                && queries[at..].windows(2).all(|pair| pair[1].history.starts_with(&pair[0].history))
        }) && plan_deliveries(observed, Agent::Misread, MISREAD_CORRECTION) == 1
            && text(ended, "/chip/phase") == "ended"
            && text(ended, "/result/value/report/bytes") == MISREAD_REPORT,
        MISREAD_FRESH_TIMER,
        "the next run must contain none of the first run's turns and hear the correction once before reporting",
    );
}

fn misread_note(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{MISREAD_NOTE, MISREAD_RULE};
    let writes = plan_tool_results(observed, Agent::Misread, b"note");
    let recalls = plan_tool_results(observed, Agent::Misread, b"recall");
    let cancels = plan_tool_results(observed, Agent::Misread, b"cancel");
    // Like 4.4, search recalls the project's note: its durable name is taken
    // from the write acknowledgement, never guessed from concurrent endings.
    let kept = writes.len() == 1
        && writes.iter().all(|(written, args, output)| {
            let output = String::from_utf8_lossy(output);
            let name = output
                .strip_prefix("Note ")
                .and_then(|suffix| suffix.strip_suffix(" revision 1 written"))
                .and_then(|digits| digits.parse::<u64>().ok())
                .filter(|name| *name != 0);
            args["scope"] == serde_json::json!({"kind":"project","project":PROJECT})
                && args["description"] == MISREAD_NOTE
                && args["body"] == MISREAD_RULE
                && cancels.iter().any(|(cancelled, _, _)| cancelled < written)
                && name.is_some_and(|name| {
                    recalls.iter().any(|(recalled, args, output)| {
                        written < recalled
                            && args["kind"] == "search"
                            && args["query"] == MISREAD_NOTE
                            && args["scopes"] == serde_json::json!([{"kind":"project","project":PROJECT}])
                            && contains(output, &format!("Note {name} revision 1: {MISREAD_NOTE}\n{MISREAD_RULE}\n"))
                    })
                })
        });
    step_check(
        misses,
        "5.6",
        kept,
        MISREAD_NOTED,
        "after cancelling the helper, the director must write the project's rule and recall that same note after its write",
    );
}

#[cfg(test)]
mod misread_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn input(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn tool(id: &str, name: &str, args: &Value, output: &str) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                parts: Box::new([Part::ToolCall {
                    id: id.as_bytes().into(),
                    name: name.as_bytes().into(),
                    arguments: serde_json::to_vec(args).expect("args").into(),
                }]),
            },
            Message {
                role: Role::User,
                parts: Box::new([Part::ToolOutput {
                    id: id.as_bytes().into(),
                    output: output.as_bytes().into(),
                    is_error: false,
                }]),
            },
        ]
    }
    fn told(agent: Agent, history: &[Message]) -> Told {
        Told::new(agent.cue(), b"fake", None, history).expect("agent cue")
    }
    fn rows(task: u64, lines: &[Value]) -> Value {
        Value::Array(
            lines
                .iter()
                .enumerate()
                .map(|(at, line)| {
                    json!({"event":{"owner":{"source":{
                        "task":task,"position":lines.len()-at,"line":line
                    }}}})
                })
                .collect(),
        )
    }
    fn met() -> Observed {
        let timer =
            json!({"entries":[{"number":MISREAD_TIMER,"kind":{"timer":{"at":MISREAD_TIMER_AT,"period":null}}}]});
        let mut documents = BTreeMap::new();
        for (read, task, parent, phase, attempt) in [
            (Read::MisreadParked, MISREAD_DIRECTOR, 0, "idle", 1),
            (Read::MisreadFresh, MISREAD_DIRECTOR, 0, "running", 3),
            (Read::MisreadEnded, MISREAD_DIRECTOR, 0, "ended", 3),
            (Read::MisreadHelperRunning, MISREAD_HELPER, MISREAD_DIRECTOR, "running", 2),
            (Read::MisreadHelperEnded, MISREAD_HELPER, MISREAD_DIRECTOR, "ended", 2),
        ] {
            documents.insert(
                read,
                json!({"task":{
                    "chip":{"task":task,"phase":phase,"details":{"owner":{"details":{
                        "attempt":attempt,"observation":{"work":"writing"},"live_delegates":0
                    }}}},"project":PROJECT,"requester":{"task":parent},"goal":MISREAD_HELPER,"priority":1,
                    "subscriptions":timer,"delegates":[],"result":null
                }}),
            );
        }
        documents.insert(
            Read::MisreadRulePending,
            json!({"task":MISREAD_DIRECTOR,"writing":{"text":MISREAD_RULE_PENDING,"cut":false}}),
        );
        let fresh = rows(
            MISREAD_DIRECTOR,
            &[
                json!({"run_began":{"attempt":3,"why":{"message":{"message":MISREAD_CORRECTION_MESSAGE}}}}),
                json!({"words":{"message":MISREAD_CORRECTION_MESSAGE,"by":{"person":1},"kind":"words","words":MISREAD_CORRECTION}}),
                json!({"run_ended":{"attempt":1,"end":"parked"}}),
            ],
        );
        documents.get_mut(&Read::MisreadFresh).expect("fresh")["activity"] = json!({"rows":fresh});
        documents.get_mut(&Read::MisreadEnded).expect("chat")["task"]["result"] =
            json!({"value":{"report":{"bytes":MISREAD_REPORT}}});
        let helper = documents.get_mut(&Read::MisreadHelperEnded).expect("helper");
        helper["task"]["result"] = json!({"ending":{"task":{"cancelled":{
            "by":{"task":MISREAD_DIRECTOR},"reason":{"bytes":MISREAD_REASON}
        }}}});
        helper["activity"] = json!({"cut":false,"before":null,"after":null,"rows":rows(MISREAD_HELPER, &[
            json!({"run_ended":{"attempt":2,"end":"cancelled"}}), json!({"run_began":{"attempt":2}}),
        ])});
        let first = tool(
            "timer",
            "subscribe",
            &json!({"kind":"timer","at":MISREAD_TIMER_AT}),
            &format!("Subscription {MISREAD_TIMER} added"),
        );
        let mut next = vec![input(MISREAD_CORRECTION)];
        let fresh = told(Agent::Misread, &next);
        next.extend(tool("cancel", "cancel", &json!({"target":MISREAD_HELPER,"reason":MISREAD_REASON}), "Done"));
        let cancelled = told(Agent::Misread, &next);
        next.extend(tool(
            "note",
            "note",
            &json!({"scope":{"kind":"project","project":PROJECT},
            "description":MISREAD_NOTE,"body":MISREAD_RULE}),
            "Note 3 revision 1 written",
        ));
        let written = told(Agent::Misread, &next);
        next.extend(tool(
            "recall",
            "recall",
            &json!({"kind":"search","scopes":[{"kind":"project","project":PROJECT}],
            "query":MISREAD_NOTE}),
            &format!("Note 3 revision 1: {MISREAD_NOTE}\n{MISREAD_RULE}\n"),
        ));
        let recalled = told(Agent::Misread, &next);
        next.extend(tool(
            "head",
            "read_branch",
            &json!({"repository":{"forge":1,"repository":crate::forge::ID},
            "read":{"branch":"main"}}),
            &draft_commit(),
        ));
        Observed {
            documents,
            commands: Vec::new(),
            told: vec![
                told(Agent::Misread, &first),
                told(Agent::MisreadHelper, &[input("Begin the work your brief describes.")]),
                fresh,
                cancelled,
                written,
                recalled,
                told(Agent::Misread, &next),
            ],
            kit: Kit { records: Vec::new(), pulls: Vec::new(), main: BTreeMap::new(), merged_green: false },
        }
    }
    #[test]
    fn informative_direction_meets_the_checkpoint() {
        let passing = met();
        assert_met(&step_5_6(&passing.clone()));
        let mut replayed = passing.clone();
        replayed.told.insert(2, replayed.told[1].clone());
        assert_met(&step_5_6(&replayed));
    }
    #[test]
    fn every_required_field_and_missing_document_rejects_a_mutation() {
        let passing = met();
        for (read, paths) in [
            (
                Read::MisreadParked,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/chip/phase",
                    "/task/chip/details/owner/details/attempt",
                ],
            ),
            (
                Read::MisreadFresh,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/chip/phase",
                    "/task/chip/details/owner/details/attempt",
                    "/task/chip/details/owner/details/observation/work",
                ],
            ),
            (
                Read::MisreadEnded,
                vec!["/task/chip/task", "/task/project", "/task/chip/phase", "/task/result/value/report/bytes"],
            ),
            (
                Read::MisreadHelperRunning,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/requester/task",
                    "/task/chip/phase",
                    "/task/priority",
                    "/task/goal",
                    "/task/chip/details/owner/details/observation/work",
                    "/task/result",
                ],
            ),
            (
                Read::MisreadHelperEnded,
                vec![
                    "/task/chip/task",
                    "/task/project",
                    "/task/requester/task",
                    "/task/chip/phase",
                    "/task/chip/details/owner/details/attempt",
                    "/task/chip/details/owner/details/live_delegates",
                    "/task/delegates",
                    "/task/result/ending/task/cancelled/by/task",
                    "/task/result/ending/task/cancelled/reason/bytes",
                    "/activity/cut",
                    "/activity/before",
                    "/activity/after",
                    "/activity/rows/0/event/owner/source/line/run_ended/end",
                ],
            ),
            (Read::MisreadRulePending, vec!["/task", "/writing/text", "/writing/cut"]),
        ] {
            for path in paths {
                let mut observed = passing.clone();
                let value = observed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field");
                *value = if value.is_null() { json!("unexpected cursor") } else { Value::Null };
                assert!(!step_5_6(&observed).is_empty(), "{read:?}{path}");
            }
        }
        for &(read, _) in acts::READS_5_6 {
            let mut observed = passing.clone();
            observed.documents.remove(&read);
            assert!(!step_5_6(&observed).is_empty(), "missing {read:?}");
        }
    }
    #[test]
    fn the_timer_identity_deadline_and_period_must_survive_both_reads() {
        let passing = met();
        for read in [Read::MisreadParked, Read::MisreadFresh] {
            for path in [
                "/task/subscriptions/entries/0/number",
                "/task/subscriptions/entries/0/kind/timer/at",
                "/task/subscriptions/entries/0/kind/timer/period",
            ] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("timer").pointer_mut(path).expect("field") = json!(99);
                assert!(!step_5_6(&observed).is_empty(), "{read:?}{path}");
            }
        }
        let mut extra = passing.clone();
        extra.documents.get_mut(&Read::MisreadFresh).expect("fresh")["task"]["subscriptions"]["entries"]
            .as_array_mut()
            .expect("timers")
            .push(json!({"number":2}));
        assert!(!step_5_6(&extra).is_empty());
    }
    #[test]
    fn correction_order_and_the_fresh_conversation_are_checked() {
        let passing = met();
        for row in 0..3 {
            let mut observed = passing.clone();
            observed.documents.get_mut(&Read::MisreadFresh).expect("fresh")["activity"]["rows"][row]["event"]["owner"]
                ["source"]["position"] = json!(0);
            assert!(!step_5_6(&observed).is_empty(), "position {row}");
        }
        let mut resumed = passing.clone();
        resumed.told[2].history = [resumed.told[0].history.to_vec(), resumed.told[2].history.to_vec()].concat().into();
        let mut duplicate = passing.clone();
        duplicate.told[2].history = vec![input(MISREAD_CORRECTION), input(MISREAD_CORRECTION)].into();
        let mut same = passing.clone();
        same.documents.get_mut(&Read::MisreadFresh).expect("fresh")["task"]["chip"]["details"]["owner"]["details"]["attempt"] =
            json!(1);
        let mut late_helper = passing.clone();
        late_helper.told.swap(1, 2);
        let mut completed_helper = passing.clone();
        completed_helper.told[1].history = vec![Message {
            role: Role::Assistant,
            parts: Box::new([Part::Text { text: b"already worked".as_slice().into() }]),
        }]
        .into();
        for observed in [resumed, duplicate, same, late_helper, completed_helper] {
            assert!(!step_5_6(&observed).is_empty());
        }
    }
    #[test]
    fn cancellation_and_notes_must_be_actual_ordered_tool_results() {
        let passing = met();
        for at in 0..7 {
            let mut observed = passing.clone();
            observed.told.remove(at);
            assert!(!step_5_6(&observed).is_empty(), "query {at}");
        }
        let mut wrong_body = passing.clone();
        for told in &mut wrong_body.told {
            for part in told.history.iter_mut().flat_map(|message| &mut message.parts) {
                if let Part::ToolCall { name, arguments, .. } = part
                    && name.as_ref() == b"note"
                {
                    *arguments = serde_json::to_vec(&json!({"scope":{"kind":"project","project":PROJECT},"description":MISREAD_NOTE,"body":"not the rule"})).expect("args").into();
                }
            }
        }
        assert!(!step_5_6(&wrong_body).is_empty());
        let mut wrote_after = passing.clone();
        wrote_after.told.swap(4, 5);
        assert!(!step_5_6(&wrote_after).is_empty());
    }
    #[test]
    fn an_open_subtree_or_any_change_to_main_fails() {
        let passing = met();
        let mut open = passing.clone();
        open.documents.get_mut(&Read::MisreadHelperEnded).expect("helper")["activity"]["rows"][0]["event"]["owner"]["source"]
            ["line"] = json!({});
        let mut child = passing.clone();
        child.documents.get_mut(&Read::MisreadHelperEnded).expect("helper")["task"]["delegates"] =
            json!([{"chip":{"task":3}}]);
        let mut landed = passing.clone();
        for part in landed.told.last_mut().expect("head read").history.iter_mut().flat_map(|message| &mut message.parts)
        {
            if let Part::ToolOutput { id, output, .. } = part
                && id.as_ref() == b"head"
            {
                *output = b"1111111111111111111111111111111111111111".as_slice().into();
            }
        }
        for observed in [open, child, landed] {
            assert!(!step_5_6(&observed).is_empty());
        }
    }
}

// Act 2, step 5.4 (domain/testing.md, 5.3)
/// The forge's pull lifecycle; comments are outside this step's promises.
#[derive(Clone, Debug)]
pub struct Pull {
    pub number: u64,
    pub branch: Vec<u8>,
    pub opened: u64,
    /// Last observed push of this branch before the pull opened.
    pub pushed: Option<u64>,
    /// The forge's merge commit, independent of the coder's result words.
    pub landed: Option<u64>,
    pub merged_head: Option<u64>,
    pub merged: bool,
    pub closed: bool,
    pub deleted: Option<u64>,
}
fn pull_digest(records: &[skein_fake_forge_domain::Observation]) -> Vec<Pull> {
    use skein_fake_forge_domain::Observation;
    let mut pulls = Vec::<Pull>::new();
    let mut heads = BTreeMap::new();
    for record in records {
        match record {
            Observation::Moved { branch, to, .. } => {
                heads.insert(branch.to_vec(), *to);
            }
            Observation::Opened { number, branches: Some(branches), .. } => pulls.push(Pull {
                number: *number,
                branch: branches.head.to_vec(),
                opened: branches.commit,
                pushed: heads.get(branches.head.as_ref()).copied(),
                landed: None,
                merged_head: None,
                merged: false,
                closed: false,
                deleted: None,
            }),
            Observation::Closed { number, .. } => {
                if let Some(pull) = pulls.iter_mut().find(|pull| pull.number == *number) {
                    pull.closed = true;
                }
            }
            Observation::Merged { number, head, commit, .. } => {
                if let Some(pull) = pulls.iter_mut().find(|pull| pull.number == *number) {
                    pull.merged = true;
                    pull.landed = Some(*commit);
                    pull.merged_head = Some(*head);
                }
            }
            Observation::Deleted { branch, at, .. } => {
                if let Some(pull) = pulls.iter_mut().rev().find(|pull| pull.branch == branch.as_ref()) {
                    pull.deleted = Some(*at);
                }
            }
            Observation::Opened { branches: None, .. }
            | Observation::Reopened { .. }
            | Observation::Labelled { .. }
            | Observation::Revised { .. }
            | Observation::Depends { .. }
            | Observation::Requested { .. }
            | Observation::Defined { .. }
            | Observation::Commented { .. }
            | Observation::Edited { .. }
            | Observation::Removed { .. }
            | Observation::Reviewed { .. }
            | Observation::Reported { .. }
            | Observation::Enqueued { .. }
            | Observation::Group { .. }
            | Observation::Dequeued { .. }
            | Observation::Refused { .. }
            | Observation::Rejected { .. }
            | Observation::Wiki { .. } => {}
        }
    }
    pulls
}

const DIRECTION_AMENDED: Promise = Promise::Must("amendment (exact)");
const DIRECTION_CANCELLED: Promise = Promise::Must("cancel, with work kept under its closed pull");
const DIRECTION_OVERLAP: Promise = Promise::Must("grants by convention, with footprints as detection");
const DIRECTION_NEWS: Promise = Promise::Add("B's overlap news is kept for its release");

#[must_use]
pub fn step_5_4(observed: &Observed) -> Vec<Miss> {
    use super::{
        B_FILE, DEFERRED_NOTE, DEFERRED_NOTE_NUMBER, DEFERRED_WORDS, DIRECTION, DIRECTION_BRIEF, DIRECTION_REASON,
        DIRECTION_REPORT, DIRECTION_REVISION, OVERLAP_OWNER, OVERLAP_OWNER_REPORT, PLANNED, PLANNED_CANCEL_REASON,
        STORY_OWNER, STORY_OWNER_REPORT, STORY_WORDS, X3, X3_CANCEL_REASON, X3_FILE,
    };
    let mut misses = Vec::new();
    direction_identities(observed, &mut misses);
    let a = task_document(observed, Read::StoryOwnerEnded);
    step_check(
        &mut misses,
        "5.4",
        text(a, "/instructions/bytes") == DIRECTION_BRIEF
            && a.pointer("/instructions/cut") == Some(&Value::Bool(false))
            && number(a, "/spec_revision/value") == Some(DIRECTION_REVISION)
            && plan_deliveries(observed, Agent::StoryOwner, DIRECTION_BRIEF) == 1,
        DIRECTION_AMENDED,
        "A must hold the whole new direction at its revision and hear the amendment once",
    );
    step_check(
        &mut misses,
        "5.4",
        plan_tool_results(observed, Agent::Direction, b"amend").iter().any(|(_, args, _)| {
            args["target"] == STORY_OWNER
                && args["revision"] == DIRECTION_REVISION - 1
                && args["amendment"]["instructions"] == DIRECTION_BRIEF
                && args["amendment"]["reason"] == DIRECTION_REASON
        }),
        DIRECTION_AMENDED,
        "the director must amend A with the new direction and its reason",
    );
    for (read, task, by, reason) in [
        (Read::X3Ended, X3, STORY_OWNER, X3_CANCEL_REASON),
        (Read::PlannedEnded, PLANNED, OVERLAP_OWNER, PLANNED_CANCEL_REASON),
    ] {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "5.4",
            text(doc, "/chip/phase") == "ended"
                && number(doc, "/result/ending/task/cancelled/by/task") == Some(by)
                && text(doc, "/result/ending/task/cancelled/reason/bytes") == reason
                && number(doc, "/chip/details/owner/details/live_delegates") == Some(0),
            DIRECTION_CANCELLED,
            &format!("task {task}'s subtree must end cancelled by {by} with its reason"),
        );
    }
    step_check(
        &mut misses,
        "5.4",
        plan_note_recalled(observed, Agent::StoryOwner, DEFERRED_NOTE_NUMBER, DEFERRED_NOTE, DEFERRED_WORDS)
            && DEFERRED_WORDS.contains(&format!("Task {X3} (x3)")),
        DIRECTION_CANCELLED,
        "A must write a project note naming x3 and its deferred plan, then recall it",
    );
    let landed = task_document(observed, Read::StoryLanded);
    step_check(
        &mut misses,
        "5.4",
        text(landed, "/chip/phase") == "ended"
            && number(landed, "/result/ending/task/done/change/kind") == Some(1)
            && number(landed, "/result/value/change/part/connector") == Some(1)
            && text(landed, "/result/value/change/part/kind") == "result"
            && observed.kit.merged_green
            && observed.kit.main.get(B_FILE.as_bytes()).is_some_and(|words| words == STORY_WORDS.as_bytes())
            && !observed.kit.main.contains_key(X3_FILE.as_bytes()),
        DIRECTION_OVERLAP,
        "a1 must land green changing B's file, while main excludes x3's file",
    );
    direction_pulls(observed, &mut misses);
    direction_news(observed, &mut misses);
    direction_attempts(observed, &mut misses);
    direction_messages(observed, &mut misses);
    for (read, task, report) in [
        (Read::StoryOwnerEnded, STORY_OWNER, STORY_OWNER_REPORT),
        (Read::DirectionBEnded, OVERLAP_OWNER, OVERLAP_OWNER_REPORT),
        (Read::DirectionEnded, DIRECTION, DIRECTION_REPORT),
    ] {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "5.4",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && text(doc, "/chip/phase") == "ended"
                && text(doc, "/result/value/report/bytes") == report,
            DIRECTION_NEWS,
            &format!("task {task} must finish its report"),
        );
    }
    misses
}

fn direction_identities(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{DIRECTION, OVERLAP_OWNER, PLANNED, STORY_CHANGE, STORY_OWNER, X3};
    for (read, task, parent) in [
        (Read::X3Open, X3, STORY_OWNER),
        (Read::X3Ended, X3, STORY_OWNER),
        (Read::PlannedRunning, PLANNED, OVERLAP_OWNER),
        (Read::PlannedEnded, PLANNED, OVERLAP_OWNER),
        (Read::StoryLanded, STORY_CHANGE, STORY_OWNER),
        (Read::DirectionBHeld, OVERLAP_OWNER, DIRECTION),
        (Read::DirectionBPending, OVERLAP_OWNER, DIRECTION),
        (Read::DirectionBEnded, OVERLAP_OWNER, DIRECTION),
        (Read::StoryOwnerEnded, STORY_OWNER, DIRECTION),
        (Read::DirectionNoteWritten, STORY_OWNER, DIRECTION),
    ] {
        let doc = task_document(observed, read);
        step_check(
            misses,
            "5.4",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && number(doc, "/requester/task") == Some(parent),
            DIRECTION_OVERLAP,
            &format!("{read:?} must identify task {task} under {parent} in project {PROJECT}"),
        );
    }
}

fn direction_pulls(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{DIRECTION, STORY_CHANGE, STORY_PULL_NUMBER, X3, X3_CODER, X3_PULL_NUMBER};
    let x3_branch = format!("temper/{DIRECTION}/c{X3}");
    let a1_branch = format!("temper/{DIRECTION}/c{STORY_CHANGE}");
    let x3 = observed.kit.pulls.iter().find(|pull| pull.branch == x3_branch.as_bytes());
    let a1 = observed.kit.pulls.iter().find(|pull| pull.branch == a1_branch.as_bytes());
    step_check(
        misses,
        "5.4",
        observed.kit.pulls.len() == 2
            && x3.is_some_and(|pull| {
                pull.number == X3_PULL_NUMBER
                    && pull.closed
                    && !pull.merged
                    && pull.landed.is_none()
                    && pull.merged_head.is_none()
                    && pull.pushed == Some(pull.opened)
                    && pull.deleted == pull.pushed
            })
            && a1.is_some_and(|pull| {
                pull.number == STORY_PULL_NUMBER
                    && pull.merged
                    && pull.pushed == Some(pull.opened)
                    && pull.merged_head == pull.pushed
                    && pull.landed.is_some_and(|commit| {
                        text(task_document(observed, Read::StoryLanded), "/result/value/change/words/bytes")
                            == direction_commit(commit)
                    })
            }),
        DIRECTION_CANCELLED,
        "only x3 and a1 must open pulls; x3's unmerged pull must close and its branch be deleted at its opening head",
    );
    let produced = task_document(observed, Read::X3Produced);
    step_check(
        misses,
        "5.4",
        number(produced, "/chip/task") == Some(X3_CODER)
            && number(produced, "/requester/task") == Some(X3)
            && number(produced, "/project") == Some(PROJECT)
            && text(produced, "/chip/phase") == "ended"
            && number(produced, "/result/value/change/kind") == Some(2)
            && x3.is_some_and(|pull| pull.pushed == Some(pull.opened)),
        DIRECTION_CANCELLED,
        "x3's coder must finish a change, whose pull opens at its observed pushed head",
    );
}

fn direction_commit(commit: u64) -> String {
    use std::fmt::Write;
    let mut hex = String::new();
    for byte in &skein_fake_forge_protocol::routes::object(commit).bytes[..20] {
        write!(hex, "{byte:02x}").expect("commit hex");
    }
    hex
}
fn direction_news(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{B_FILE, OVERLAP_HOLD_REVISION, OVERLAP_LANDING_MESSAGE, OVERLAP_OWNER, OVERLAP_SUBSCRIPTION, PLANNED};
    let held = task_document(observed, Read::DirectionBHeld);
    let pending = task_document(observed, Read::DirectionBPending);
    for (read, doc) in [(Read::DirectionBHeld, held), (Read::DirectionBPending, pending)] {
        step_check(
            misses,
            "5.4",
            text(doc, "/chip/phase") == "held"
                && number(doc, "/hold/owner/revision") == Some(OVERLAP_HOLD_REVISION)
                && number(doc, "/hold/owner/reason/stopped_by/party") == Some(1),
            DIRECTION_NEWS,
            &format!("{read:?}: B must remain held at the person's Stop"),
        );
    }
    let planned = task_document(observed, Read::PlannedRunning);
    step_check(
        misses,
        "5.4",
        planned.pointer("/spec/specification/parameters").and_then(Value::as_array).is_some_and(|parameters| {
            parameters.iter().any(|parameter| {
                number(parameter, "/bytes/name") == Some(6) && text(parameter, "/bytes/value/bytes") == B_FILE
            })
        }) && number(planned, "/chip/task") == Some(PLANNED),
        DIRECTION_OVERLAP,
        "h1 must name B's file as its planned footprint before it produces a pull",
    );
    let pending_rows = check_activity(observed, Read::DirectionBPending);
    let news: Vec<_> = pending_rows
        .iter()
        .filter(|row| {
            text(row, "/line/heard/kind/news/class") == "kept"
                && text(row, "/line/heard/words").starts_with("forge landing ")
        })
        .collect();
    let stopped = number(held, "/activity");
    let ended = task_document(observed, Read::DirectionBEnded);
    let rows: Vec<_> = check_activity(observed, Read::DirectionBEnded)
        .into_iter()
        .chain(check_activity(observed, Read::DirectionBFirst))
        .collect();
    let release = rows
        .iter()
        .find(|row| number(row, "/line/decided/revision") == Some(OVERLAP_HOLD_REVISION))
        .and_then(|row| number(row, "/position"));
    let same_attempt = number(held, "/chip/details/owner/details/attempt").is_some()
        && number(held, "/chip/details/owner/details/attempt")
            == number(pending, "/chip/details/owner/details/attempt");
    step_check(
        misses,
        "5.4",
        same_attempt
            && release.zip(number(pending, "/activity")).is_some_and(|(release, head)| release > head)
            && stopped.zip(release).is_some_and(|(stop, release)| {
                stop < release
                    && rows.iter().all(|row| {
                        !number(row, "/position").is_some_and(|at| at > stop && at < release)
                            || row.pointer("/line/run_began").is_none()
                    })
            })
            && rows.iter().any(|row| {
                row.pointer("/line/run_began").is_some()
                    && number(row, "/position").zip(release).is_some_and(|(at, release)| at > release)
            })
            && ended["hold"].is_null(),
        DIRECTION_NEWS,
        "B must run no new attempt during its hold and begin its next attempt only after Release",
    );
    let landing = text(task_document(observed, Read::StoryLanded), "/result/value/change/words/bytes");
    step_check(
        misses,
        "5.4",
        news.len() == 1
            && news.first().is_some_and(|row| {
                let words = text(row, "/line/heard/words");
                let suffix = landing.get(..8).map(|head| format!(" -> {head}"));
                number(row, "/line/heard/number") == Some(OVERLAP_LANDING_MESSAGE)
                    && number(row, "/line/heard/from/task") == Some(OVERLAP_OWNER)
                    && number(row, "/line/heard/hits") == Some(1)
                    && number(row, "/line/heard/kind/news/subscription") == Some(OVERLAP_SUBSCRIPTION)
                    && suffix.is_some_and(|suffix| words.ends_with(&suffix))
                    && number(row, "/position").zip(stopped).is_some_and(|(at, stop)| at > stop)
                    && plan_deliveries(observed, Agent::OverlapOwner, words) == 1
            }),
        DIRECTION_NEWS,
        "a1's overlapping landing must wait in B's held inbox and be told to B exactly once after Release",
    );
    step_check(
        misses,
        "5.4",
        plan_attempts(
            observed,
            &direction_pages(observed, Read::DirectionBEnded, Read::DirectionBFirst),
            OVERLAP_OWNER,
            2,
        ),
        DIRECTION_NEWS,
        "B's complete journal must show its two non-overlapping attempts",
    );
}

fn direction_messages(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        DIRECTION, DIRECTION_AMENDMENT_MESSAGE, DIRECTION_BRIEF, DIRECTION_MESSAGE, DIRECTION_PERSON_MESSAGE,
        DIRECTION_REVISION,
    };
    step_check(
        misses,
        "5.4",
        check_activity(observed, Read::StoryOwnerFirst).iter().any(|row| {
            number(row, "/line/heard/number") == Some(DIRECTION_AMENDMENT_MESSAGE)
                && number(row, "/line/heard/from/task") == Some(DIRECTION)
                && number(row, "/line/heard/kind/amendment/revision") == Some(DIRECTION_REVISION)
                && text(row, "/line/heard/words") == DIRECTION_BRIEF
        }) && check_activity(observed, Read::DirectionEnded).iter().any(|row| {
            number(row, "/line/words/message") == Some(DIRECTION_PERSON_MESSAGE)
                && number(row, "/line/words/by/person") == Some(1)
                && text(row, "/line/words/words") == DIRECTION_MESSAGE
        }),
        DIRECTION_AMENDED,
        "the person's direction and A's amendment must have their fresh message identities and revision",
    );
}

// Full journals, including pages with the starts of earlier runs. Procedures
// have journal lines but no agent attempts. A refused placement recheck is
// allowed only for B's first admission, at zero spend (jig's domain/engine.md,
// 7.1: queued placement losing its selected slot returns the claim without a
// failed try; jig-domain-fleet's attempt.rs, resume, and jig-domain-core's
// routing/children.rs translate Recheck to the journal's refused ending).
// B and the director now fit on one page. Retain support for an earlier
// page in oracle fixtures: plan_attempts still requires complete, ordered
// journals, and rejects an absent page whenever the latest has a cursor.
fn direction_pages(observed: &Observed, latest: Read, earlier: Read) -> Vec<Read> {
    let mut reads = vec![latest];
    if observed.documents.contains_key(&earlier) {
        reads.push(earlier);
    }
    reads
}
fn direction_attempts(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{DIRECTION, OVERLAP_OWNER, OVERLAP_RECHECK_ATTEMPT, PLANNED, STORY_CHANGE, STORY_OWNER, X3, X3_CODER};
    let b_reads = direction_pages(observed, Read::DirectionBEnded, Read::DirectionBFirst);
    let director_reads = direction_pages(observed, Read::DirectionEnded, Read::DirectionFirst);
    for (task, reads, minimum) in [
        (X3, &[Read::X3Ended][..], 0),
        (X3_CODER, &[Read::X3Produced][..], 1),
        (PLANNED, &[Read::PlannedEnded][..], 0),
        (STORY_CHANGE, &[Read::StoryLanded][..], 0),
        (STORY_OWNER, &[Read::StoryOwnerEnded, Read::StoryOwnerEarlier, Read::StoryOwnerFirst][..], 1),
        (OVERLAP_OWNER, b_reads.as_slice(), 2),
        (DIRECTION, director_reads.as_slice(), 1),
    ] {
        step_check(
            misses,
            "5.4",
            plan_attempts(observed, reads, task, minimum)
                && {
                    let rows: Vec<_> = reads.iter().flat_map(|read| check_activity(observed, *read)).collect();
                    rows.last().is_some_and(|row| number(row, "/position") == Some(1))
                        && rows.windows(2).all(|pair| {
                            number(pair[0], "/position")
                                .zip(number(pair[1], "/position"))
                                .is_some_and(|(new, old)| new.checked_sub(1) == Some(old))
                        })
                }
                && reads.iter().flat_map(|read| check_activity(observed, *read)).all(|row| {
                    let Some(end) = row.pointer("/line/run_ended") else { return true };
                    matches!(end["end"].as_str(), Some("finished" | "parked"))
                        || (task == OVERLAP_OWNER
                            && end["end"] == "refused"
                            && number(end, "/attempt") == Some(OVERLAP_RECHECK_ATTEMPT)
                            && number(end, "/spent") == Some(0))
                }),
            DIRECTION_NEWS,
            &format!(
                "task {task}'s complete journal must end every counted attempt without failure or an unexplained refusal"
            ),
        );
    }
}

#[cfg(test)]
mod direction_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn input(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn tool(id: &str, name: &str, args: &Value, output: &str) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                parts: Box::new([Part::ToolCall {
                    id: id.as_bytes().into(),
                    name: name.as_bytes().into(),
                    arguments: serde_json::to_vec(args).expect("arguments").into(),
                }]),
            },
            Message {
                role: Role::User,
                parts: Box::new([Part::ToolOutput {
                    id: id.as_bytes().into(),
                    output: output.as_bytes().into(),
                    is_error: false,
                }]),
            },
        ]
    }
    fn told(agent: Agent, history: &[Message]) -> Told {
        Told::new(agent.cue(), b"fake", None, history).expect("agent cue")
    }
    fn journal(task: u64, attempts: usize, first: u64) -> Value {
        let lines = if attempts == 0 {
            vec![json!({"milestone":{"position":1}})]
        } else {
            vec![
                json!({"run_began":{"attempt":first}}),
                json!({"run_ended":{"attempt":first,"end":"finished","spent":1}}),
            ]
        };
        let rows: Vec<_> = lines
            .into_iter()
            .enumerate()
            .rev()
            .map(
                |(at, line)| json!({"event":{"owner":{"source":{"task":task,"position":first+at as u64,"line":line}}}}),
            )
            .collect();
        json!({"rows":rows,"cut":false,"before":null,"after":null})
    }
    fn documents() -> BTreeMap<Read, Value> {
        let mut documents = BTreeMap::new();
        for (read, task, parent) in [
            (Read::X3Open, X3, STORY_OWNER),
            (Read::X3Produced, X3_CODER, X3),
            (Read::X3Ended, X3, STORY_OWNER),
            (Read::PlannedRunning, PLANNED, OVERLAP_OWNER),
            (Read::PlannedEnded, PLANNED, OVERLAP_OWNER),
            (Read::StoryLanded, STORY_CHANGE, STORY_OWNER),
            (Read::DirectionBHeld, OVERLAP_OWNER, DIRECTION),
            (Read::DirectionBPending, OVERLAP_OWNER, DIRECTION),
            (Read::DirectionBEnded, OVERLAP_OWNER, DIRECTION),
            (Read::StoryOwnerEnded, STORY_OWNER, DIRECTION),
            (Read::DirectionNoteWritten, STORY_OWNER, DIRECTION),
            (Read::DirectionEnded, DIRECTION, 0),
        ] {
            documents.insert(read, json!({"task":{
                "chip":{"task":task,"phase":"ended","details":{"owner":{"details":{"attempt":1,"live_delegates":0}}}},
                "project":PROJECT,"requester":{"task":parent},"hold":null,
                "result":{"value":{}}
            }}));
        }
        for (read, by, reason) in
            [(Read::X3Ended, STORY_OWNER, X3_CANCEL_REASON), (Read::PlannedEnded, OVERLAP_OWNER, PLANNED_CANCEL_REASON)]
        {
            documents.get_mut(&read).expect("cancelled")["task"]["result"]["ending"] =
                json!({"task":{"cancelled":{"by":{"task":by},"reason":{"bytes":reason}}}});
        }
        for (read, report) in [
            (Read::StoryOwnerEnded, STORY_OWNER_REPORT),
            (Read::DirectionBEnded, OVERLAP_OWNER_REPORT),
            (Read::DirectionEnded, DIRECTION_REPORT),
        ] {
            documents.get_mut(&read).expect("report")["task"]["result"]["value"] = json!({"report":{"bytes":report}});
        }
        let a = &mut documents.get_mut(&Read::StoryOwnerEnded).expect("A")["task"];
        a["instructions"] = json!({"bytes":DIRECTION_BRIEF,"cut":false});
        a["spec_revision"] = json!({"value":DIRECTION_REVISION});
        documents.get_mut(&Read::X3Produced).expect("coder")["task"]["result"]["value"] =
            json!({"change":{"kind":2,"words":{"bytes":"The deferred plan."}}});
        let a1 = &mut documents.get_mut(&Read::StoryLanded).expect("a1")["task"];
        a1["result"] = json!({"ending":{"task":{"done":{"change":{"kind":1}}}},
            "value":{"change":{"part":{"connector":1,"kind":"result"},"words":{"bytes":direction_commit(9)}}}});
        documents.get_mut(&Read::PlannedRunning).expect("h1")["task"]["spec"] = json!({"specification":{
            "parameters":[{"bytes":{"name":6,"value":{"bytes":B_FILE,"cut":false}}}]
        }});
        let news = format!("forge landing 12345678 -> {}", &direction_commit(9)[..8]);
        let rows: Vec<_> = [
            json!({"run_began":{"attempt":1}}),
            json!({"run_ended":{"attempt":1,"end":"parked","spent":1}}),
            json!({"decided":{"revision":OVERLAP_HOLD_REVISION}}),
            json!({"heard":{"number":OVERLAP_LANDING_MESSAGE,"from":{"task":OVERLAP_OWNER},"hits":1,"kind":{"news":{"class":"kept","subscription":OVERLAP_SUBSCRIPTION}},"words":news}}),
            json!({"decided":{"revision":OVERLAP_HOLD_REVISION}}),
            json!({"run_began":{"attempt":2}}),
            json!({"run_ended":{"attempt":2,"end":"finished","spent":1}}),
        ]
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            json!({"event":{"owner":{"source":{
                "task":OVERLAP_OWNER,"position":at+1,"line":line
            }}}})
        })
        .rev()
        .collect();
        for (read, head) in [(Read::DirectionBHeld, 3), (Read::DirectionBPending, 4)] {
            let doc = documents.get_mut(&read).expect("held");
            doc["task"]["chip"]["phase"] = json!("held");
            doc["task"]["activity"] = json!(head);
            doc["task"]["hold"] =
                json!({"owner":{"revision":OVERLAP_HOLD_REVISION,"reason":{"stopped_by":{"party":1}}}});
            doc["activity"] = json!({"rows":if head == 4 { vec![rows[3].clone()] } else { Vec::new() }});
        }
        documents.get_mut(&Read::DirectionBEnded).expect("B")["activity"] =
            json!({"cut":false,"after":null,"before":{"value":"older"},"rows":rows[..3]});
        documents.insert(
            Read::DirectionBFirst,
            json!({"cut":false,"before":null,"after":{"value":"newer"},"rows":rows[3..]}),
        );
        journals(&mut documents);
        documents
    }

    fn journals(documents: &mut BTreeMap<Read, Value>) {
        for (read, task, minimum) in [
            (Read::X3Ended, X3, 0),
            (Read::X3Produced, X3_CODER, 1),
            (Read::PlannedEnded, PLANNED, 0),
            (Read::StoryLanded, STORY_CHANGE, 0),
        ] {
            documents.get_mut(&read).expect("task")["activity"] = journal(task, minimum, 1);
        }
        let mut newest = journal(STORY_OWNER, 1, 6);
        newest["before"] = json!({"value":"older"});
        documents.get_mut(&Read::StoryOwnerEnded).expect("A")["activity"] = newest;
        let mut middle = journal(STORY_OWNER, 1, 4);
        middle["before"] = json!({"value":"first"});
        documents.insert(Read::StoryOwnerEarlier, middle);
        documents.insert(Read::StoryOwnerFirst, journal(STORY_OWNER, 1, 1));
        let first =
            documents.get_mut(&Read::StoryOwnerFirst).expect("A's first page")["rows"].as_array_mut().expect("rows");
        first.insert(
            0,
            json!({"event":{"owner":{"source":{"task":STORY_OWNER,"position":3,"line":{
                "heard":{"number":DIRECTION_AMENDMENT_MESSAGE,"from":{"task":DIRECTION},
                "kind":{"amendment":{"revision":DIRECTION_REVISION}},"words":DIRECTION_BRIEF}
            }}}}}),
        );
        let mut newest = journal(DIRECTION, 1, 4);
        newest["rows"].as_array_mut().expect("rows").push(json!({"event":{"owner":{"source":{
            "task":DIRECTION,"position":3,"line":{"words":{"message":DIRECTION_PERSON_MESSAGE,
            "by":{"person":1},"words":DIRECTION_MESSAGE}}
        }}}}));
        newest["before"] = json!({"value":"first"});
        documents.get_mut(&Read::DirectionEnded).expect("director")["activity"] = newest;
        documents.insert(Read::DirectionFirst, journal(DIRECTION, 1, 1));
    }

    fn met() -> Observed {
        let documents = documents();
        let news = format!("forge landing 12345678 -> {}", &direction_commit(9)[..8]);
        let amendment = tool(
            "amend",
            "amend",
            &json!({"target":STORY_OWNER,"revision":DIRECTION_REVISION-1,
            "amendment":{"instructions":DIRECTION_BRIEF,"reason":DIRECTION_REASON}}),
            "Amended",
        );
        let mut a = vec![input(DIRECTION_BRIEF)];
        a.extend(tool(
            "note",
            "note",
            &json!({"scope":{"kind":"project","project":PROJECT},
            "description":DEFERRED_NOTE,"body":DEFERRED_WORDS}),
            &format!("Note {DEFERRED_NOTE_NUMBER} revision 1 written"),
        ));
        let noted = told(Agent::StoryOwner, &a);
        a.extend(tool(
            "recall",
            "recall",
            &json!({"kind":"name","name":DEFERRED_NOTE_NUMBER}),
            &format!("Note {DEFERRED_NOTE_NUMBER} revision 1: {DEFERRED_NOTE}\n{DEFERRED_WORDS}\n"),
        ));
        Observed {
            documents,
            commands: Vec::new(),
            told: vec![
                told(Agent::Direction, &amendment),
                noted,
                told(Agent::StoryOwner, &a),
                told(Agent::OverlapOwner, &[input("Begin B's work.")]),
                told(Agent::OverlapOwner, &[input("Begin B's work."), input(&news)]),
            ],
            kit: Kit {
                records: Vec::new(),
                main: BTreeMap::from([(B_FILE.as_bytes().to_vec(), STORY_WORDS.as_bytes().to_vec())]),
                merged_green: true,
                pulls: vec![
                    Pull {
                        number: X3_PULL_NUMBER,
                        branch: format!("temper/{DIRECTION}/c{X3}").into_bytes(),
                        opened: 7,
                        pushed: Some(7),
                        landed: None,
                        merged_head: None,
                        merged: false,
                        closed: true,
                        deleted: Some(7),
                    },
                    Pull {
                        number: STORY_PULL_NUMBER,
                        branch: format!("temper/{DIRECTION}/c{STORY_CHANGE}").into_bytes(),
                        opened: 8,
                        pushed: Some(8),
                        landed: Some(9),
                        merged_head: Some(8),
                        merged: true,
                        closed: false,
                        deleted: None,
                    },
                ],
            },
        }
    }
    #[test]
    fn new_direction_and_deferred_overlap_meet_the_checkpoint() {
        assert_met(&step_5_4(&met()));
    }
    #[test]
    fn complete_single_page_journals_keep_the_same_checks() {
        let mut observed = met();
        for (latest, earlier) in
            [(Read::DirectionBEnded, Read::DirectionBFirst), (Read::DirectionEnded, Read::DirectionFirst)]
        {
            let older = observed.documents.remove(&earlier).expect("earlier page");
            let page = &mut observed.documents.get_mut(&latest).expect("latest page")["activity"];
            page["rows"].as_array_mut().expect("rows").extend(older["rows"].as_array().expect("older rows").clone());
            page["before"] = Value::Null;
        }
        assert_met(&step_5_4(&observed));
        for latest in [Read::DirectionBEnded, Read::DirectionEnded] {
            let mut missing = observed.clone();
            missing.documents.get_mut(&latest).expect("latest")["activity"]["before"] = json!({"value":"missing"});
            assert!(!step_5_4(&missing).is_empty(), "{latest:?} must not omit an earlier page");
            let mut gap = observed.clone();
            gap.documents.get_mut(&latest).expect("latest")["activity"]["rows"].as_array_mut().expect("rows").remove(1);
            assert!(!step_5_4(&gap).is_empty(), "{latest:?} must have no journal gap");
        }
    }

    #[test]
    fn every_required_document_field_rejects_a_mutation() {
        let passing = met();
        for (read, paths) in [
            (
                Read::StoryOwnerEnded,
                vec!["/task/instructions/bytes", "/task/instructions/cut", "/task/spec_revision/value"],
            ),
            (
                Read::X3Produced,
                vec![
                    "/task/chip/task",
                    "/task/chip/phase",
                    "/task/requester/task",
                    "/task/project",
                    "/task/result/value/change/kind",
                ],
            ),
            (
                Read::X3Ended,
                vec![
                    "/task/result/ending/task/cancelled/by/task",
                    "/task/result/ending/task/cancelled/reason/bytes",
                    "/task/chip/details/owner/details/live_delegates",
                ],
            ),
            (
                Read::PlannedEnded,
                vec![
                    "/task/result/ending/task/cancelled/by/task",
                    "/task/result/ending/task/cancelled/reason/bytes",
                    "/task/chip/details/owner/details/live_delegates",
                ],
            ),
            (
                Read::PlannedRunning,
                vec![
                    "/task/spec/specification/parameters/0/bytes/name",
                    "/task/spec/specification/parameters/0/bytes/value/bytes",
                ],
            ),
            (
                Read::StoryLanded,
                vec![
                    "/task/result/ending/task/done/change/kind",
                    "/task/result/value/change/part/connector",
                    "/task/result/value/change/part/kind",
                    "/task/result/value/change/words/bytes",
                ],
            ),
            (
                Read::DirectionBHeld,
                vec![
                    "/task/chip/phase",
                    "/task/hold/owner/revision",
                    "/task/hold/owner/reason/stopped_by/party",
                    "/task/chip/details/owner/details/attempt",
                    "/task/activity",
                ],
            ),
            (
                Read::DirectionBPending,
                vec![
                    "/task/chip/phase",
                    "/task/hold/owner/revision",
                    "/task/hold/owner/reason/stopped_by/party",
                    "/task/chip/details/owner/details/attempt",
                    "/task/activity",
                    "/activity/rows/0/event/owner/source/line/heard/kind/news/class",
                    "/activity/rows/0/event/owner/source/line/heard/words",
                ],
            ),
            (
                Read::DirectionBEnded,
                vec!["/activity/cut", "/activity/after", "/activity/rows/2/event/owner/source/line/decided/revision"],
            ),
            (Read::DirectionBFirst, vec!["/cut", "/before"]),
        ] {
            for path in paths {
                let mut observed = passing.clone();
                let value = observed.documents.get_mut(&read).expect("document").pointer_mut(path).expect("field");
                *value = if value.is_null() { json!("unexpected") } else { Value::Null };
                assert!(!step_5_4(&observed).is_empty(), "{read:?}{path}");
            }
        }
    }
    #[test]
    fn identities_and_reports_reject_a_mutation() {
        let passing = met();
        for (read, _, _) in [
            (Read::X3Open, X3, STORY_OWNER),
            (Read::X3Ended, X3, STORY_OWNER),
            (Read::PlannedRunning, PLANNED, OVERLAP_OWNER),
            (Read::PlannedEnded, PLANNED, OVERLAP_OWNER),
            (Read::StoryLanded, STORY_CHANGE, STORY_OWNER),
            (Read::DirectionBHeld, OVERLAP_OWNER, DIRECTION),
            (Read::DirectionBPending, OVERLAP_OWNER, DIRECTION),
            (Read::DirectionBEnded, OVERLAP_OWNER, DIRECTION),
            (Read::StoryOwnerEnded, STORY_OWNER, DIRECTION),
            (Read::DirectionNoteWritten, STORY_OWNER, DIRECTION),
        ] {
            for path in ["/task/chip/task", "/task/project", "/task/requester/task"] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("read").pointer_mut(path).expect("field") = Value::Null;
                assert!(!step_5_4(&observed).is_empty(), "{read:?}{path}");
            }
        }
        for read in [Read::StoryOwnerEnded, Read::DirectionBEnded, Read::DirectionEnded] {
            for path in ["/task/chip/task", "/task/project", "/task/chip/phase", "/task/result/value/report/bytes"] {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("read").pointer_mut(path).expect("field") = Value::Null;
                assert!(!step_5_4(&observed).is_empty(), "{read:?}{path}");
            }
        }
    }
    #[test]
    fn the_closed_pull_must_keep_the_pushed_head_and_h1_must_open_nothing() {
        let passing = met();
        for mutation in 0..8 {
            let mut observed = passing.clone();
            match mutation {
                0 => {
                    observed.kit.pulls.remove(0);
                }
                1 => observed.kit.pulls[0].closed = false,
                2 => observed.kit.pulls[0].merged = true,
                3 => observed.kit.pulls[0].deleted = None,
                4 => observed.kit.pulls[0].deleted = Some(99),
                5 => observed.kit.pulls[0].opened = 99,
                6 => observed.kit.pulls[1].merged = false,
                7 => observed.kit.pulls.push(Pull {
                    number: 3,
                    branch: format!("temper/{DIRECTION}/c{PLANNED}").into_bytes(),
                    opened: 9,
                    pushed: Some(9),
                    landed: None,
                    merged_head: None,
                    merged: false,
                    closed: true,
                    deleted: Some(9),
                }),
                _ => unreachable!(),
            }
            assert!(!step_5_4(&observed).is_empty(), "mutation {mutation}");
        }
        let mut red = passing.clone();
        red.kit.merged_green = false;
        let mut absent = passing.clone();
        absent.kit.main.clear();
        let mut old = passing.clone();
        old.kit.main.insert(X3_FILE.as_bytes().to_vec(), X3_WORDS.as_bytes().to_vec());
        for observed in [red, absent, old] {
            assert!(!step_5_4(&observed).is_empty());
        }
    }
    #[test]
    fn news_cannot_be_doubled_or_lost_and_a_run_cannot_start_during_the_hold() {
        let passing = met();
        let mut doubled = passing.clone();
        let extra = doubled.told[4].history[1].clone();
        doubled.told[4].history = [doubled.told[4].history.to_vec(), vec![extra]].concat().into();
        let mut lost = passing.clone();
        lost.told.pop();
        let mut during = passing.clone();
        during.documents.get_mut(&Read::DirectionBFirst).expect("older")["rows"][0]["event"]["owner"]["source"]["line"] =
            json!({"run_began":{"attempt":2}});
        let mut early = passing.clone();
        early.documents.get_mut(&Read::DirectionBEnded).expect("B")["activity"]["rows"][2]["event"]["owner"]["source"]
            ["position"] = json!(2);
        for observed in [doubled, lost, during, early] {
            assert!(!step_5_4(&observed).is_empty());
        }
    }
    #[test]
    fn amendment_and_note_must_be_real_completed_tools() {
        let passing = met();
        for at in [0, 2] {
            let mut observed = passing.clone();
            observed.told.remove(at);
            assert!(!step_5_4(&observed).is_empty());
        }
        let mut missing = passing.clone();
        missing.told[1].history = Box::new([]);
        missing.told[2].history = Box::new([]);
        let mut duplicate = passing.clone();
        duplicate.told[2].history = [duplicate.told[2].history.to_vec(), vec![input(DIRECTION_BRIEF)]].concat().into();
        let mut prose = passing.clone();
        prose.told[0].history = vec![input(DIRECTION_BRIEF)].into();
        for observed in [missing, duplicate, prose] {
            assert!(!step_5_4(&observed).is_empty());
        }
    }
    fn ending_mutations(read: Read, task: u64) {
        for ending in [json!({"failed":"invalid"}), json!("refused"), json!("exceeded")] {
            let mut observed = met();
            let doc = observed.documents.get_mut(&read).expect("journal");
            let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
            let rows = page["rows"].as_array_mut().expect("rows");
            let position = rows[0]["event"]["owner"]["source"]["position"].as_u64().expect("position");
            // Add a later, fully paired attempt, so a successful final result
            // cannot hide an invalid earlier activation.
            rows.insert(
                0,
                json!({"event":{"owner":{"source":{"task":task,"position":position+1,
                "line":{"run_began":{"attempt":99}}}}}}),
            );
            rows.insert(
                0,
                json!({"event":{"owner":{"source":{"task":task,"position":position+2,
                "line":{"run_ended":{"attempt":99,"end":ending,"spent":0}}}}}}),
            );
            assert!(
                step_5_4(&observed)
                    .iter()
                    .any(|miss| miss.what.starts_with(&format!("task {task}'s complete journal")))
            );
        }
    }
    #[test]
    fn x3_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::X3Ended, X3);
    }
    #[test]
    fn x3s_coder_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::X3Produced, X3_CODER);
    }
    #[test]
    fn h1_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::PlannedEnded, PLANNED);
    }
    #[test]
    fn a1_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::StoryLanded, STORY_CHANGE);
    }
    #[test]
    fn a_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::StoryOwnerEnded, STORY_OWNER);
    }
    #[test]
    fn b_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::DirectionBEnded, OVERLAP_OWNER);
    }
    #[test]
    fn the_director_cannot_hide_a_failed_or_refused_attempt() {
        ending_mutations(Read::DirectionEnded, DIRECTION);
    }

    #[test]
    fn only_bs_zero_spend_first_placement_recheck_is_allowed() {
        let mut observed = met();
        for read in [Read::DirectionBEnded, Read::DirectionBFirst, Read::DirectionBPending, Read::DirectionBHeld] {
            let doc = observed.documents.get_mut(&read).expect("B");
            if let Some(head) = doc.pointer_mut("/task/activity") {
                *head = json!(head.as_u64().expect("head") + 2);
            }
            let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
            for row in page["rows"].as_array_mut().expect("rows") {
                let position = &mut row["event"]["owner"]["source"]["position"];
                *position = json!(position.as_u64().expect("position") + 2);
            }
        }
        let rows =
            observed.documents.get_mut(&Read::DirectionBFirst).expect("B first")["rows"].as_array_mut().expect("rows");
        rows.push(json!({"event":{"owner":{"source":{"task":OVERLAP_OWNER,"position":2,
            "line":{"run_ended":{"attempt":OVERLAP_RECHECK_ATTEMPT,"end":"refused","spent":0}}}}}}));
        rows.push(json!({"event":{"owner":{"source":{"task":OVERLAP_OWNER,"position":1,
            "line":{"run_began":{"attempt":OVERLAP_RECHECK_ATTEMPT}}}}}}));
        assert_met(&step_5_4(&observed));
        for field in ["spent", "attempt"] {
            let mut changed = observed.clone();
            let rows = changed.documents.get_mut(&Read::DirectionBFirst).expect("B first")["rows"]
                .as_array_mut()
                .expect("rows");
            let at = rows.len() - 2;
            rows[at]["event"]["owner"]["source"]["line"]["run_ended"][field] = json!(99);
            assert!(!step_5_4(&changed).is_empty(), "{field}");
        }
    }

    #[test]
    fn every_counted_journal_must_be_complete_and_ordered() {
        let passing = met();
        for read in [
            Read::X3Ended,
            Read::X3Produced,
            Read::PlannedEnded,
            Read::StoryLanded,
            Read::StoryOwnerEnded,
            Read::StoryOwnerEarlier,
            Read::StoryOwnerFirst,
            Read::DirectionBEnded,
            Read::DirectionBFirst,
            Read::DirectionEnded,
            Read::DirectionFirst,
        ] {
            let mut missing = passing.clone();
            missing.documents.remove(&read);
            assert!(!step_5_4(&missing).is_empty(), "missing {read:?}");
            for mutation in 0..3 {
                let mut observed = passing.clone();
                let doc = observed.documents.get_mut(&read).expect("journal");
                let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
                match mutation {
                    0 => page["cut"] = json!(true),
                    1 => page["rows"][0]["event"]["owner"]["source"]["task"] = json!(99),
                    2 => {
                        let duplicate = page["rows"][0].clone();
                        page["rows"].as_array_mut().expect("rows").push(duplicate);
                    }
                    _ => unreachable!(),
                }
                assert!(!step_5_4(&observed).is_empty(), "{read:?} mutation {mutation}");
            }
        }
    }

    #[test]
    fn fresh_message_numbers_senders_and_amendment_revision_are_checked() {
        let passing = met();
        for (read, paths) in [
            (
                Read::StoryOwnerFirst,
                vec![
                    "/rows/0/event/owner/source/line/heard/number",
                    "/rows/0/event/owner/source/line/heard/from/task",
                    "/rows/0/event/owner/source/line/heard/kind/amendment/revision",
                    "/rows/0/event/owner/source/line/heard/words",
                ],
            ),
            (
                Read::DirectionEnded,
                vec![
                    "/activity/rows/2/event/owner/source/line/words/message",
                    "/activity/rows/2/event/owner/source/line/words/by/person",
                    "/activity/rows/2/event/owner/source/line/words/words",
                ],
            ),
            (
                Read::DirectionBPending,
                vec![
                    "/activity/rows/0/event/owner/source/line/heard/number",
                    "/activity/rows/0/event/owner/source/line/heard/from/task",
                    "/activity/rows/0/event/owner/source/line/heard/hits",
                    "/activity/rows/0/event/owner/source/line/heard/kind/news/subscription",
                ],
            ),
        ] {
            for path in paths {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
                assert!(!step_5_4(&observed).is_empty(), "{read:?}{path}");
            }
        }
    }

    #[test]
    fn both_pull_heads_and_the_merge_commit_come_from_forge_records() {
        let passing = met();
        for pull in 0..2 {
            for mutation in 0..5 {
                let mut observed = passing.clone();
                let pull = &mut observed.kit.pulls[pull];
                match mutation {
                    0 => pull.number = 99,
                    1 => pull.branch = b"unrelated".to_vec(),
                    2 => pull.pushed = None,
                    3 => pull.pushed = Some(99),
                    4 => pull.landed = Some(99),
                    _ => unreachable!(),
                }
                assert!(!step_5_4(&observed).is_empty());
            }
        }
        let mut observed = passing.clone();
        observed.documents.get_mut(&Read::X3Produced).expect("coder")["task"]["result"]["value"]["change"]["words"] =
            json!({"bytes":"arbitrary result prose"});
        assert_met(&step_5_4(&observed));
    }

    fn forge_records() -> Vec<skein_fake_forge_domain::Observation> {
        use skein_fake_forge_domain::{Branches, Observation};
        let repository: Box<[u8]> = b"acme/app".as_slice().into();
        let mut records = Vec::new();
        for (task, number, commit) in [(X3, X3_PULL_NUMBER, 7), (STORY_CHANGE, STORY_PULL_NUMBER, 8)] {
            let branch = format!("temper/{DIRECTION}/c{task}").into_bytes().into_boxed_slice();
            records.push(Observation::Moved {
                repository: repository.clone(),
                branch: branch.clone(),
                from: None,
                to: commit,
                by: 1,
            });
            records.push(Observation::Opened {
                repository: repository.clone(),
                number,
                kind: skein_fake_forge_domain::api::Kind::Pull,
                title: Box::new([]),
                body: Box::new([]),
                labels: Box::new([]),
                branches: Some(Branches { head: branch.clone(), base: b"main".as_slice().into(), commit }),
                by: 1,
            });
            if task == X3 {
                records.push(Observation::Closed { repository: repository.clone(), number, by: 1 });
            } else {
                records.push(Observation::Merged {
                    repository: repository.clone(),
                    number,
                    base: b"main".as_slice().into(),
                    head: commit,
                    commit: 9,
                    by: 1,
                });
            }
            records.push(Observation::Deleted { repository: repository.clone(), branch, at: commit, by: 1 });
        }
        records
    }

    #[test]
    fn pull_digest_correlates_push_open_close_delete_and_merge_in_record_order() {
        let mut observed = met();
        observed.kit.pulls = pull_digest(&forge_records());
        assert_met(&step_5_4(&observed));
        assert_eq!(observed.kit.pulls[0].pushed, Some(7));
        assert_eq!(observed.kit.pulls[1].pushed, Some(8));
        assert_eq!(observed.kit.pulls[1].merged_head, Some(8));
        assert_eq!(observed.kit.pulls[1].landed, Some(9));
    }

    #[test]
    fn missing_reordered_or_changed_forge_observations_fail_the_checkpoint() {
        use skein_fake_forge_domain::Observation;
        let passing = met();
        for mutation in 0..9 {
            let mut records = forge_records();
            match mutation {
                0 => {
                    records.remove(0);
                }
                1 => {
                    if let Observation::Moved { to, .. } = &mut records[0] {
                        *to = 99;
                    }
                }
                2 => records.swap(0, 1),
                3 => {
                    if let Observation::Closed { number, .. } = &mut records[2] {
                        *number = 99;
                    }
                }
                4 => {
                    if let Observation::Deleted { at, .. } = &mut records[3] {
                        *at = 99;
                    }
                }
                5 => {
                    if let Observation::Merged { head, .. } = &mut records[6] {
                        *head = 99;
                    }
                }
                6 => {
                    if let Observation::Merged { commit, .. } = &mut records[6] {
                        *commit = 99;
                    }
                }
                7 => {
                    records.remove(4);
                }
                8 => {
                    if let Observation::Opened { branches: Some(branches), .. } = &mut records[5] {
                        branches.commit = 99;
                    }
                }
                _ => unreachable!(),
            }
            let mut observed = passing.clone();
            observed.kit.pulls = pull_digest(&records);
            assert!(!step_5_4(&observed).is_empty(), "record mutation {mutation}");
        }
    }
}

// Act 3, step 6.1 (domain/testing.md, 5.3)
const BATCH_MESSAGES: Promise = Promise::Must("a question and its answer (exact)");
const BATCH_DEPENDENCIES: Promise = Promise::Must("a batch with its dependencies (exact)");
const BATCH_PARALLEL: Promise = Promise::Must("parallel delegates (exact)");
const BATCH_ATTEMPTS: Promise = Promise::Add("every attempt ended, none failed");

fn batch_tasks() -> [(Read, u64, u64); 7] {
    use super::{BATCH_DIRECTOR, BATCH_ONE, BATCH_OWNER, BATCH_STORY, BATCH_SUBOWNER, BATCH_THREE, BATCH_TWO};
    [
        (Read::BatchDirectorEnded, BATCH_DIRECTOR, 0),
        (Read::BatchOwnerEnded, BATCH_OWNER, BATCH_DIRECTOR),
        (Read::BatchSubownerEnded, BATCH_SUBOWNER, BATCH_OWNER),
        (Read::BatchOneEnded, BATCH_ONE, BATCH_SUBOWNER),
        (Read::BatchTwoEnded, BATCH_TWO, BATCH_SUBOWNER),
        (Read::BatchThreeEnded, BATCH_THREE, BATCH_SUBOWNER),
        (Read::BatchStoryEnded, BATCH_STORY, BATCH_SUBOWNER),
    ]
}
fn batch_writing(observed: &Observed, read: Read, task: u64, ended: Read) -> bool {
    let doc = task_document(observed, read);
    number(doc, "/chip/task") == Some(task)
        && text(doc, "/chip/phase") == "running"
        && text(doc, "/chip/details/owner/details/observation/work") == "writing"
        && number(doc, "/chip/details/owner/phase/active/running/attempt").is_some_and(|attempt| {
            check_activity(observed, ended).iter().any(|row| number(row, "/line/run_began/attempt") == Some(attempt))
        })
}
fn batch_journal(observed: &Observed, read: Read, task: u64) -> bool {
    batch_journal_pages(observed, &[read], task)
}
fn batch_journal_pages(observed: &Observed, reads: &[Read], task: u64) -> bool {
    let rows: Vec<_> = reads.iter().flat_map(|read| check_activity(observed, *read)).collect();
    plan_attempts(observed, reads, task, 1)
        && rows.last().is_some_and(|row| number(row, "/position") == Some(1))
        && rows.windows(2).all(|pair| {
            number(pair[0], "/position")
                .zip(number(pair[1], "/position"))
                .is_some_and(|(new, old)| new.checked_sub(1) == Some(old))
        })
        && rows.iter().enumerate().all(|(at, row)| {
            let Some(end) = row.pointer("/line/run_ended") else { return true };
            matches!(end["end"].as_str(), Some("parked" | "finished"))
                || (end["end"] == "refused"
                    && number(end, "/spent") == Some(0)
                    && number(end, "/attempt").is_some_and(|attempt| {
                        !rows.iter().any(|r| number(r, "/line/turn/attempt") == Some(attempt))
                    })
                    // Rows are newest first: a finished attempt must follow
                    // this zero-spend placement recheck in commit order.
                    && rows[..at].iter().any(|r| text(r, "/line/run_ended/end") == "finished"))
        })
}
fn batch_unstarted(observed: &Observed, read: Read, pending: &[u64]) -> bool {
    let doc = task_document(observed, read);
    number(doc, "/chip/task") == Some(super::BATCH_STORY)
        && text(doc, "/chip/phase") == "waiting"
        && same_members(doc.pointer("/chip/details/owner/phase/waiting/dependencies"), &serde_json::json!(pending))
        && pause_activity(observed, read)["cut"] == false
        && pause_activity(observed, read)["before"] == Value::Null
        && pause_activity(observed, read)["after"] == Value::Null
        && check_activity(observed, read).iter().all(|row| row.pointer("/line/run_began").is_none())
}
// A task's dependencies are its prerequisites, in no order (jig's domain/tasks.md, 4).
fn same_members(value: Option<&Value>, expected: &Value) -> bool {
    let (Some(Value::Array(found)), Value::Array(wanted)) = (value, expected) else { return false };
    found.len() == wanted.len() && wanted.iter().all(|item| found.contains(item))
}
fn batch_done(observed: &Observed, read: Read, task: u64) -> bool {
    let doc = task_document(observed, read);
    number(doc, "/chip/task") == Some(task)
        && text(doc, "/chip/phase") == "ended"
        && doc.pointer("/result/ending/task/done").is_some_and(Value::is_object)
        && batch_journal(observed, read, task)
}

#[must_use]
pub fn step_6_1(observed: &Observed) -> Vec<Miss> {
    use super::{A_DEADLINE, A1_DEADLINE, BATCH_ONE, BATCH_OWNER, BATCH_STORY, BATCH_THREE, BATCH_TWO, PROJECT};
    let mut misses = Vec::new();
    for (read, task, parent) in batch_tasks() {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "6.1",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && (parent == 0 || number(doc, "/requester/task") == Some(parent))
                && text(doc, "/chip/phase") == "ended",
            BATCH_DEPENDENCIES,
            &format!("{read:?} must identify the ended task {task} under {parent}"),
        );
        step_check(
            &mut misses,
            "6.1",
            batch_done(observed, read, task),
            BATCH_ATTEMPTS,
            &format!("task {task}'s complete ordered journal must end every attempt successfully or parked"),
        );
    }
    let a = task_document(observed, Read::BatchOwnerEnded);
    let a1 = task_document(observed, Read::BatchSubownerEnded);
    step_check(
        &mut misses,
        "6.1",
        number(a, "/goal") == Some(BATCH_OWNER)
            && number(a, "/authority/authority/deadline") == Some(A_DEADLINE)
            && number(a1, "/authority/authority/deadline") == Some(A1_DEADLINE)
            && number(a1, "/authority/authority/delegation/depth") == Some(1),
        BATCH_DEPENDENCIES,
        "A must be tracked and A1 must carry its earlier deadline and depth one",
    );
    let answer_at = batch_messages(observed, &mut misses);
    let delegates = plan_tool_results(observed, Agent::BatchSubowner, b"delegate");
    step_check(
        &mut misses,
        "6.1",
        delegates.len() == 1
            && delegates.iter().all(|(at, args, out)| {
                answer_at.is_some_and(|answer| *at > answer)
                    && contains(
                        out,
                        &format!("Delegated tasks: {BATCH_ONE}, {BATCH_TWO}, {BATCH_THREE}, {BATCH_STORY}"),
                    )
                    && args["batch"].as_array().is_some_and(|batch| {
                        batch.len() == 4
                            && same_members(
                                Some(&batch[3]["dependencies"]),
                                &serde_json::json!([{"kind":"batch","number":0},{"kind":"batch","number":1}]),
                            )
                    })
            })
            && task_document(observed, Read::BatchStoryEnded)["dependency_facts"].as_array().is_some_and(|facts| {
                facts.len() == 2
                    && facts.iter().filter_map(|f| number(f, "/number")).collect::<BTreeSet<_>>()
                        == BTreeSet::from([BATCH_ONE, BATCH_TWO])
            }),
        BATCH_DEPENDENCIES,
        "one A1 delegation after its answer must make all four, with a4 depending on a1 and a2 alone",
    );
    step_check(
        &mut misses,
        "6.1",
        [
            (Read::BatchOneWriting, BATCH_ONE, Read::BatchOneEnded),
            (Read::BatchTwoWriting, BATCH_TWO, Read::BatchTwoEnded),
            (Read::BatchThreeWriting, BATCH_THREE, Read::BatchThreeEnded),
        ]
        .iter()
        .all(|(read, task, end)| batch_writing(observed, *read, *task, *end)),
        BATCH_PARALLEL,
        "the three read run documents must all show their held first turns still writing",
    );
    step_check(
        &mut misses,
        "6.1",
        batch_unstarted(observed, Read::BatchStoryWaiting, &[BATCH_ONE, BATCH_TWO])
            && batch_done(observed, Read::BatchOneBeforeStory, BATCH_ONE)
            && batch_unstarted(observed, Read::BatchStoryAfterOne, &[BATCH_TWO])
            && batch_done(observed, Read::BatchTwoBeforeStory, BATCH_TWO)
            && batch_writing(observed, Read::BatchStoryWriting, BATCH_STORY, Read::BatchStoryEnded)
            && batch_writing(observed, Read::BatchThreeStillWriting, BATCH_THREE, Read::BatchThreeEnded)
            && number(
                task_document(observed, Read::BatchThreeWriting),
                "/chip/details/owner/phase/active/running/attempt",
            ) == number(
                task_document(observed, Read::BatchThreeStillWriting),
                "/chip/details/owner/phase/active/running/attempt",
            ),
        BATCH_PARALLEL,
        "a4 must wait with no attempt before and after a1 ends, then write after both successful endings while a3 still writes",
    );
    misses
}

fn batch_messages(observed: &Observed, misses: &mut Vec<Miss>) -> Option<usize> {
    use super::{
        BATCH_ANSWER, BATCH_ANSWER_NUMBER, BATCH_OWNER, BATCH_QUESTION, BATCH_QUESTION_NUMBER, BATCH_SUBOWNER,
    };
    let questions = plan_tool_results(observed, Agent::BatchSubowner, b"message");
    let answers = plan_tool_results(observed, Agent::BatchOwner, b"message");
    let question_at = observed
        .told
        .iter()
        .position(|t| t.agent == Agent::BatchOwner && user_words(t).any(|w| contains(w, BATCH_QUESTION)));
    let answer_at = observed
        .told
        .iter()
        .position(|t| t.agent == Agent::BatchSubowner && user_words(t).any(|w| contains(w, BATCH_ANSWER)));
    step_check(
        misses,
        "6.1",
        questions.len() == 1
            && answers.len() == 1
            && questions.iter().all(|(_, args, out)| {
                args["target"] == BATCH_OWNER
                    && args["form"] == "question"
                    && args["words"] == BATCH_QUESTION
                    && contains(out, &format!("Message {BATCH_QUESTION_NUMBER} sent"))
            })
            && answers.iter().all(|(_, args, out)| {
                args["target"] == BATCH_SUBOWNER
                    && args["form"] == "answer"
                    && args["question"] == BATCH_QUESTION_NUMBER
                    && args["words"] == BATCH_ANSWER
                    && contains(out, &format!("Message {BATCH_ANSWER_NUMBER} sent"))
            })
            && plan_deliveries(observed, Agent::BatchOwner, BATCH_QUESTION) == 1
            && plan_deliveries(observed, Agent::BatchSubowner, BATCH_ANSWER) == 1
            && question_at.zip(answer_at).is_some_and(|(q, a)| q < a)
            && check_activity(observed, Read::BatchSubownerEnded)
                .iter()
                .filter(|row| {
                    number(row, "/line/words/message") == Some(BATCH_ANSWER_NUMBER)
                        && number(row, "/line/words/by/task") == Some(BATCH_OWNER)
                        && number(row, "/line/words/kind/answer/question") == Some(BATCH_QUESTION_NUMBER)
                        && text(row, "/line/words/words") == BATCH_ANSWER
                })
                .count()
                == 1,
        BATCH_MESSAGES,
        "A1's numbered design question and A's three-amendment answer must each be delivered once, in order",
    );
    answer_at
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part, Role};

    pub(super) fn input(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    pub(super) fn tool(id: &str, name: &str, args: &Value, output: &str) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                parts: Box::new([Part::ToolCall {
                    id: id.as_bytes().into(),
                    name: name.as_bytes().into(),
                    arguments: serde_json::to_vec(args).expect("arguments").into(),
                }]),
            },
            Message {
                role: Role::User,
                parts: Box::new([Part::ToolOutput {
                    id: id.as_bytes().into(),
                    output: output.as_bytes().into(),
                    is_error: false,
                }]),
            },
        ]
    }
    pub(super) fn told(agent: Agent, history: &[Message]) -> Told {
        Told::new(agent.cue(), b"fake", None, history).expect("agent cue")
    }
    pub(super) fn journal(task: u64, lines: &[Value]) -> Value {
        let rows: Vec<_> = lines
            .iter()
            .enumerate()
            .rev()
            .map(|(at, line)| json!({"event":{"owner":{"source":{"task":task,"position":at+1,"line":line}}}}))
            .collect();
        json!({"rows":rows,"cut":false,"before":null,"after":null})
    }
    fn met() -> Observed {
        let mut documents = BTreeMap::new();
        for (read, task, parent) in batch_tasks() {
            documents.insert(read, json!({"task":{
                "chip":{"task":task,"phase":"ended"},"project":PROJECT,"requester":{"task":parent},
                "goal":BATCH_OWNER,"authority":{"authority":{"deadline":if task == BATCH_OWNER { A_DEADLINE } else { A1_DEADLINE },"delegation":{"depth":1}}},
                "result":{"ending":{"task":{"done":{}}}}
            },"activity":journal(task, &[
                json!({"run_began":{"attempt":1}}),
                json!({"run_ended":{"attempt":1,"end":"finished"}}),
            ])}));
        }
        let answer = json!({"words":{"message":BATCH_ANSWER_NUMBER,"by":{"task":BATCH_OWNER},
            "kind":{"answer":{"question":BATCH_QUESTION_NUMBER}},"words":BATCH_ANSWER}});
        documents.get_mut(&Read::BatchSubownerEnded).expect("A1")["activity"] = journal(
            BATCH_SUBOWNER,
            &[json!({"run_began":{"attempt":1}}), answer, json!({"run_ended":{"attempt":1,"end":"finished"}})],
        );
        documents.get_mut(&Read::BatchStoryEnded).expect("a4")["task"]["dependency_facts"] =
            json!([{"number":BATCH_ONE},{"number":BATCH_TWO}]);
        for (read, pending) in
            [(Read::BatchStoryWaiting, vec![BATCH_ONE, BATCH_TWO]), (Read::BatchStoryAfterOne, vec![BATCH_TWO])]
        {
            documents.insert(
                read,
                json!({"task":{"chip":{"task":BATCH_STORY,"phase":"waiting","details":{"owner":{"phase":{"waiting":{"dependencies":pending}}}}}},
                "activity":journal(BATCH_STORY, &[])}),
            );
        }
        for (read, task) in [
            (Read::BatchOneWriting, BATCH_ONE),
            (Read::BatchTwoWriting, BATCH_TWO),
            (Read::BatchThreeWriting, BATCH_THREE),
            (Read::BatchStoryWriting, BATCH_STORY),
            (Read::BatchThreeStillWriting, BATCH_THREE),
        ] {
            documents.insert(
                read,
                json!({"task":{"chip":{"task":task,"phase":"running","details":{"owner":{
                    "phase":{"active":{"running":{"attempt":1}}},"details":{"observation":{"work":"writing"}}
                }}}}}),
            );
        }
        for (before, ended) in
            [(Read::BatchOneBeforeStory, Read::BatchOneEnded), (Read::BatchTwoBeforeStory, Read::BatchTwoEnded)]
        {
            documents.insert(before, documents[&ended].clone());
        }
        let mut sub = tool(
            "question",
            "message",
            &json!({"target":BATCH_OWNER,"form":"question","words":BATCH_QUESTION}),
            &format!("Message {BATCH_QUESTION_NUMBER} sent"),
        )
        .to_vec();
        let mut owner = vec![input(BATCH_QUESTION)];
        owner.extend(tool(
            "answer",
            "message",
            &json!({"target":BATCH_SUBOWNER,"form":"answer","question":BATCH_QUESTION_NUMBER,"words":BATCH_ANSWER}),
            &format!("Message {BATCH_ANSWER_NUMBER} sent"),
        ));
        let mut queries = vec![told(Agent::BatchSubowner, &sub), told(Agent::BatchOwner, &owner)];
        sub.push(input(BATCH_ANSWER));
        queries.push(told(Agent::BatchSubowner, &sub));
        sub.extend(tool(
            "batch",
            "delegate",
            &json!({"batch":[{},{},{},{"dependencies":[{"kind":"batch","number":0},{"kind":"batch","number":1}]}]}),
            &format!("Delegated tasks: {BATCH_ONE}, {BATCH_TWO}, {BATCH_THREE}, {BATCH_STORY}"),
        ));
        queries.push(told(Agent::BatchSubowner, &sub));
        Observed {
            documents,
            told: queries,
            commands: Vec::new(),
            kit: Kit { records: Vec::new(), main: BTreeMap::new(), merged_green: false, pulls: Vec::new() },
        }
    }
    fn rejects(observed: &Observed, promise: Promise) {
        let misses = step_6_1(observed);
        assert!(
            misses.iter().any(|miss| miss.step == "6.1" && miss.promise == promise),
            "mutation must fail {promise:?}"
        );
    }
    #[test]
    fn batch_observation_meets_the_step() {
        assert_met(&step_6_1(&met()));
    }
    #[test]
    fn every_required_document_field_rejects_a_mutation() {
        let passing = met();
        for (read, task, _) in batch_tasks() {
            for path in ["/task/chip/task", "/task/project", "/task/requester/task", "/task/chip/phase"] {
                if task == BATCH_DIRECTOR && path == "/task/requester/task" {
                    continue;
                }
                let mut changed = passing.clone();
                *changed.documents.get_mut(&read).expect("document").pointer_mut(path).expect("field") = Value::Null;
                rejects(&changed, BATCH_DEPENDENCIES);
            }
        }
        for (read, path) in [
            (Read::BatchOwnerEnded, "/task/goal"),
            (Read::BatchOwnerEnded, "/task/authority/authority/deadline"),
            (Read::BatchSubownerEnded, "/task/authority/authority/deadline"),
            (Read::BatchSubownerEnded, "/task/authority/authority/delegation/depth"),
            (Read::BatchStoryEnded, "/task/dependency_facts"),
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&read).expect("document").pointer_mut(path).expect("field") = Value::Null;
            rejects(&changed, BATCH_DEPENDENCIES);
        }
    }
    #[test]
    fn messages_and_batch_require_the_numbered_tools_and_delivery_order() {
        let passing = met();
        for (id, fields, promise) in [
            (b"question".as_slice(), vec!["target", "form", "words"], BATCH_MESSAGES),
            (b"answer".as_slice(), vec!["target", "form", "question", "words"], BATCH_MESSAGES),
            (b"batch".as_slice(), vec!["batch"], BATCH_DEPENDENCIES),
        ] {
            for field in fields {
                let mut changed = passing.clone();
                let mut edits = 0;
                for query in &mut changed.told {
                    for message in &mut query.history {
                        for part in &mut message.parts {
                            if let Part::ToolCall { id: name, arguments, .. } = part
                                && name.as_ref() == id
                            {
                                let mut args: Value = serde_json::from_slice(arguments).expect("args");
                                args[field] = Value::Null;
                                *arguments = serde_json::to_vec(&args).expect("args").into();
                                edits += 1;
                            }
                        }
                    }
                }
                assert!(edits > 0);
                rejects(&changed, promise);
            }
        }
        let mut missing_batch = passing.clone();
        missing_batch.told.pop();
        rejects(&missing_batch, BATCH_DEPENDENCIES);
        let mut premature_batch = passing.clone();
        premature_batch.told.remove(2);
        rejects(&premature_batch, BATCH_DEPENDENCIES);
        let mut reversed = passing.clone();
        reversed.told.swap(1, 2);
        rejects(&reversed, BATCH_MESSAGES);
        let mut duplicate = passing.clone();
        duplicate.told.push(told(Agent::BatchOwner, &[input(BATCH_QUESTION)]));
        rejects(&duplicate, BATCH_MESSAGES);
        let mut duplicate = passing;
        duplicate.told.push(told(Agent::BatchSubowner, &[input(BATCH_ANSWER)]));
        rejects(&duplicate, BATCH_MESSAGES);
    }
    #[test]
    fn message_acknowledgements_and_the_recorded_answer_must_identify_the_exchange() {
        let passing = met();
        for id in [b"question".as_slice(), b"answer", b"batch"] {
            let mut changed = passing.clone();
            for told in &mut changed.told {
                for message in &mut told.history {
                    for part in &mut message.parts {
                        if let Part::ToolOutput { id: name, output, .. } = part
                            && name.as_ref() == id
                        {
                            *output = b"Refused".as_slice().into();
                        }
                    }
                }
            }
            rejects(&changed, if id == b"batch" { BATCH_DEPENDENCIES } else { BATCH_MESSAGES });
        }
        for field in ["message", "by", "kind", "words"] {
            let mut changed = passing.clone();
            let rows = changed.documents.get_mut(&Read::BatchSubownerEnded).expect("A1")["activity"]["rows"]
                .as_array_mut()
                .expect("rows");
            for row in rows {
                if let Some(words) = row.pointer_mut("/event/owner/source/line/words") {
                    words[field] = Value::Null;
                }
            }
            rejects(&changed, BATCH_MESSAGES);
        }
        for facts in [
            json!([{"number":BATCH_ONE}]),
            json!([{"number":BATCH_ONE},{"number":BATCH_THREE}]),
            json!([{"number":BATCH_ONE},{"number":BATCH_TWO},{"number":BATCH_THREE}]),
        ] {
            let mut changed = passing.clone();
            changed.documents.get_mut(&Read::BatchStoryEnded).expect("a4")["task"]["dependency_facts"] = facts;
            rejects(&changed, BATCH_DEPENDENCIES);
        }
    }
    #[test]
    fn each_parallel_run_read_requires_its_held_attempt() {
        let passing = met();
        for read in [
            Read::BatchOneWriting,
            Read::BatchTwoWriting,
            Read::BatchThreeWriting,
            Read::BatchStoryWriting,
            Read::BatchThreeStillWriting,
        ] {
            for path in [
                "/task/chip/task",
                "/task/chip/phase",
                "/task/chip/details/owner/phase/active/running/attempt",
                "/task/chip/details/owner/details/observation/work",
            ] {
                let mut changed = passing.clone();
                *changed.documents.get_mut(&read).expect("run").pointer_mut(path).expect("field") = Value::Null;
                rejects(&changed, BATCH_PARALLEL);
            }
        }
    }
    #[test]
    fn a4_cannot_start_while_either_dependency_is_pending() {
        let passing = met();
        for read in [Read::BatchStoryWaiting, Read::BatchStoryAfterOne] {
            for path in [
                "/task/chip/task",
                "/task/chip/phase",
                "/task/chip/details/owner/phase/waiting/dependencies",
                "/activity/cut",
            ] {
                let mut changed = passing.clone();
                *changed.documents.get_mut(&read).expect("waiting").pointer_mut(path).expect("field") = Value::Null;
                rejects(&changed, BATCH_PARALLEL);
            }
            let mut changed = passing.clone();
            changed.documents.get_mut(&read).expect("waiting")["activity"] =
                journal(BATCH_STORY, &[json!({"run_began":{"attempt":1}})]);
            rejects(&changed, BATCH_PARALLEL);
        }
        let mut reversed = passing.clone();
        *reversed
            .documents
            .get_mut(&Read::BatchStoryWaiting)
            .expect("waiting")
            .pointer_mut("/task/chip/details/owner/phase/waiting/dependencies")
            .expect("dependencies") = json!([BATCH_TWO, BATCH_ONE]);
        assert_met(&step_6_1(&reversed));
        let mut other = passing.clone();
        *other
            .documents
            .get_mut(&Read::BatchStoryWaiting)
            .expect("waiting")
            .pointer_mut("/task/chip/details/owner/phase/waiting/dependencies")
            .expect("dependencies") = json!([BATCH_ONE, BATCH_THREE]);
        rejects(&other, BATCH_PARALLEL);
        for read in [Read::BatchOneBeforeStory, Read::BatchTwoBeforeStory] {
            let mut changed = passing.clone();
            changed.documents.get_mut(&read).expect("ended")["task"]["result"]["ending"] =
                json!({"task":{"failed":{}}});
            rejects(&changed, BATCH_PARALLEL);
        }
    }
    #[test]
    fn a4_must_still_be_waiting_and_a3_cannot_have_ended_in_its_writing_read() {
        let passing = met();
        for (read, phase) in [
            (Read::BatchStoryWaiting, "running"),
            (Read::BatchStoryAfterOne, "running"),
            (Read::BatchThreeStillWriting, "ended"),
        ] {
            let mut changed = passing.clone();
            changed.documents.get_mut(&read).expect("task")["task"]["chip"]["phase"] = json!(phase);
            rejects(&changed, BATCH_PARALLEL);
        }
    }
    #[test]
    fn only_zero_spend_uncommitted_rechecks_with_a_later_finished_attempt_are_allowed() {
        use serde_json::json;
        let passing = met();
        for (read, task, _) in batch_tasks() {
            let lines = vec![
                json!({"run_began":{"attempt":8}}),
                json!({"run_ended":{"attempt":8,"end":"refused","spent":0}}),
                json!({"run_began":{"attempt":1}}),
                json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}),
            ];
            let mut accepted = passing.clone();
            // Keep message evidence in A1's journal while exercising its attempts.
            let mut journal_lines = lines.clone();
            if task == BATCH_SUBOWNER {
                journal_lines.push(json!({"words":{"message":BATCH_ANSWER_NUMBER,"by":{"task":BATCH_OWNER},
                    "kind":{"answer":{"question":BATCH_QUESTION_NUMBER}},"words":BATCH_ANSWER}}));
            }
            accepted.documents.get_mut(&read).expect("task")["activity"] = journal(task, &journal_lines);
            assert_met(&step_6_1(&accepted));
            for mutation in 0..3 {
                let mut changed = accepted.clone();
                let mut bad = lines.clone();
                match mutation {
                    0 => bad[1]["run_ended"]["spent"] = json!(1),
                    1 => bad.truncate(2),
                    _ => bad.insert(1, json!({"turn":{"attempt":8,"turn":1}})),
                }
                changed.documents.get_mut(&read).expect("task")["activity"] = journal(task, &bad);
                rejects(&changed, BATCH_ATTEMPTS);
            }
        }
    }
    #[test]
    fn journals_cannot_hide_an_attempt_in_a_gap_or_a_missing_page() {
        let passing = met();
        for (read, task, _) in batch_tasks() {
            for mutation in 0..7 {
                let mut changed = passing.clone();
                let activity = &mut changed.documents.get_mut(&read).expect("task")["activity"];
                match mutation {
                    0 => activity["before"] = json!(1),
                    1 => activity["after"] = json!(1),
                    2 => activity["rows"].as_array_mut().expect("rows").reverse(),
                    3 => activity["rows"][0]["event"]["owner"]["source"]["position"] = json!(99),
                    4 => activity["rows"][0]["event"]["owner"]["source"]["task"] = json!(task + 1),
                    5 => {
                        *activity = journal(
                            task,
                            &[
                                json!({"run_began":{"attempt":1}}),
                                json!({"run_ended":{"attempt":1,"end":"parked"}}),
                                json!({"run_began":{"attempt":1}}),
                                json!({"run_ended":{"attempt":1,"end":"finished"}}),
                            ],
                        );
                    }
                    _ => {
                        *activity = journal(
                            task,
                            &[
                                json!({"run_began":{"attempt":1}}),
                                json!({"run_began":{"attempt":2}}),
                                json!({"run_ended":{"attempt":2,"end":"finished"}}),
                                json!({"run_ended":{"attempt":1,"end":"finished"}}),
                            ],
                        );
                    }
                }
                rejects(&changed, BATCH_ATTEMPTS);
            }
        }
    }
    #[test]
    fn every_attempt_must_end_without_failure_in_a_complete_journal() {
        let passing = met();
        for (read, task, _) in batch_tasks() {
            for lines in [
                vec![json!({"run_began":{"attempt":1}})],
                vec![json!({"run_began":{"attempt":1}}), json!({"run_ended":{"attempt":1,"end":{"failed":{}}}})],
                vec![json!({"run_began":{"attempt":1}}), json!({"run_ended":{"attempt":2,"end":"finished"}})],
            ] {
                let mut changed = passing.clone();
                changed.documents.get_mut(&read).expect("task")["activity"] = journal(task, &lines);
                rejects(&changed, BATCH_ATTEMPTS);
            }
            for result in [Value::Null, json!({"ending":{"task":{"failed":{}}}})] {
                let mut changed = passing.clone();
                changed.documents.get_mut(&read).expect("task")["task"]["result"] = result;
                rejects(&changed, BATCH_ATTEMPTS);
            }
            let mut changed = passing.clone();
            changed.documents.get_mut(&read).expect("task")["activity"]["cut"] = json!(true);
            rejects(&changed, BATCH_ATTEMPTS);
        }
    }
}

// Act 3, step 6.2 (domain/testing.md, 5.3)
const BLOCKER_MESSAGES: Promise = Promise::Must("two questions and two answers, up then down");
const BLOCKER_CHECKPOINT: Promise = Promise::Must("e1 waits for A1's answer before delegation");
const BLOCKER_DEPENDENCY: Promise = Promise::Must("a dependency added to a live task");
const BLOCKER_KEPT: Promise = Promise::Add("a3 kept pending e1, then told its landing once");
const BLOCKER_LANDING: Promise = Promise::Must("e1 lands green changing B's fake");
const BLOCKER_ATTEMPTS: Promise = Promise::Add("every attempt ended, none failed");

fn blocker_tasks() -> [(Read, u64, u64); 7] {
    use super::{
        BLOCKER_A3, BLOCKER_CODER, BLOCKER_DIRECTOR, BLOCKER_E1, BLOCKER_OWNER, BLOCKER_REVIEWER, BLOCKER_SUBOWNER,
    };
    [
        (Read::BlockerDirectorEnded, BLOCKER_DIRECTOR, 0),
        (Read::BlockerOwnerEnded, BLOCKER_OWNER, BLOCKER_DIRECTOR),
        (Read::BlockerSubownerEnded, BLOCKER_SUBOWNER, BLOCKER_OWNER),
        (Read::BlockerA3Ended, BLOCKER_A3, BLOCKER_SUBOWNER),
        (Read::BlockerE1Ended, BLOCKER_E1, BLOCKER_SUBOWNER),
        (Read::BlockerCoderEnded, BLOCKER_CODER, BLOCKER_E1),
        (Read::BlockerReviewerEnded, BLOCKER_REVIEWER, BLOCKER_E1),
    ]
}

struct BlockerExchange {
    sender: Agent,
    receiver: Agent,
    by: u64,
    target: u64,
    message: u64,
    question: Option<u64>,
    words: &'static str,
    read: Read,
}
fn blocker_exchanges() -> [BlockerExchange; 5] {
    use super::{
        Agent, BLOCKER_A3, BLOCKER_ANSWER, BLOCKER_ANSWER_NUMBER, BLOCKER_DIRECTOR, BLOCKER_FINDING,
        BLOCKER_FINDING_NUMBER, BLOCKER_OWNER, BLOCKER_QUESTION, BLOCKER_QUESTION_NUMBER, BLOCKER_SUBOWNER,
        BLOCKER_UP_ANSWER, BLOCKER_UP_ANSWER_NUMBER, BLOCKER_UP_QUESTION, BLOCKER_UP_QUESTION_NUMBER,
    };
    [
        BlockerExchange {
            sender: Agent::BlockerImplementer,
            receiver: Agent::BlockerSubowner,
            by: BLOCKER_A3,
            target: BLOCKER_SUBOWNER,
            message: BLOCKER_FINDING_NUMBER,
            question: None,
            words: BLOCKER_FINDING,
            read: Read::BlockerSubownerEnded,
        },
        BlockerExchange {
            sender: Agent::BlockerSubowner,
            receiver: Agent::BlockerOwner,
            by: BLOCKER_SUBOWNER,
            target: BLOCKER_OWNER,
            message: BLOCKER_QUESTION_NUMBER,
            question: None,
            words: BLOCKER_QUESTION,
            read: Read::BlockerOwnerEnded,
        },
        BlockerExchange {
            sender: Agent::BlockerOwner,
            receiver: Agent::BlockerDirector,
            by: BLOCKER_OWNER,
            target: BLOCKER_DIRECTOR,
            message: BLOCKER_UP_QUESTION_NUMBER,
            question: None,
            words: BLOCKER_UP_QUESTION,
            read: Read::BlockerDirectorEnded,
        },
        BlockerExchange {
            sender: Agent::BlockerDirector,
            receiver: Agent::BlockerOwner,
            by: BLOCKER_DIRECTOR,
            target: BLOCKER_OWNER,
            message: BLOCKER_UP_ANSWER_NUMBER,
            question: Some(BLOCKER_UP_QUESTION_NUMBER),
            words: BLOCKER_UP_ANSWER,
            read: Read::BlockerOwnerEnded,
        },
        BlockerExchange {
            sender: Agent::BlockerOwner,
            receiver: Agent::BlockerSubowner,
            by: BLOCKER_OWNER,
            target: BLOCKER_SUBOWNER,
            message: BLOCKER_ANSWER_NUMBER,
            question: Some(BLOCKER_QUESTION_NUMBER),
            words: BLOCKER_ANSWER,
            read: Read::BlockerSubownerEnded,
        },
    ]
}
fn blocker_delivery(observed: &Observed, agent: Agent, words: &str) -> Option<usize> {
    observed.told.iter().position(|t| t.agent == agent && user_words(t).any(|w| contains(w, words)))
}
fn blocker_messages(observed: &Observed) -> bool {
    let mut previous = None;
    blocker_exchanges().iter().enumerate().all(|(at, exchange)| {
        let form = if exchange.question.is_some() {
            "answer"
        } else if at == 0 {
            "words"
        } else {
            "question"
        };
        let tools = plan_tool_results(observed, exchange.sender, b"message");
        let tools: Vec<_> = tools.iter().filter(|(_, args, _)| args["words"] == exchange.words).collect();
        let delivered = blocker_delivery(observed, exchange.receiver, exchange.words);
        let ordered = delivered.is_some_and(|delivery| previous.is_none_or(|before| before < delivery));
        previous = delivered;
        tools.len() == 1
            && tools.iter().all(|(_, args, out)| {
                args["target"] == exchange.target
                    && args["form"] == form
                    && exchange.question.is_none_or(|q| args["question"] == q)
                    && contains(out, &format!("Message {} sent", exchange.message))
            })
            && plan_deliveries(observed, exchange.receiver, exchange.words) == 1
            && ordered
            && check_activity(observed, exchange.read)
                .iter()
                .filter(|row| {
                    let kind = row.pointer("/line/words/kind");
                    number(row, "/line/words/message") == Some(exchange.message)
                        && number(row, "/line/words/by/task") == Some(exchange.by)
                        && text(row, "/line/words/words") == exchange.words
                        && kind
                            == Some(&exchange.question.map_or_else(
                                || Value::String(form.into()),
                                |q| serde_json::json!({"answer":{"question":q}}),
                            ))
                })
                .count()
                == 1
    })
}
fn blocker_dependency(observed: &Observed, read: Read, pending: bool) -> bool {
    use super::{BLOCKER_E1, BLOCKER_SUBOWNER};
    task_document(observed, read)["dependency_facts"].as_array().is_some_and(|facts| {
        facts.len() == 1
            && facts[0]["number"] == BLOCKER_E1
            && facts[0]["pending"] == pending
            && number(&facts[0], "/added/by/task") == Some(BLOCKER_SUBOWNER)
            && number(&facts[0], "/added/at").is_some()
    })
}

#[must_use]
pub fn step_6_2(observed: &Observed) -> Vec<Miss> {
    use super::{BLOCKER_A3, BLOCKER_ANSWER, BLOCKER_E1, BLOCKER_E1_SPEC, BLOCKER_REASON, BLOCKER_REVISION};
    let mut misses = Vec::new();
    blocker_identities(observed, &mut misses);
    let a3 = task_document(observed, Read::BlockerA3Ended);
    step_check(
        &mut misses,
        "6.2",
        blocker_messages(observed),
        BLOCKER_MESSAGES,
        "the finding and four numbered question/answer messages must each be told once, in order",
    );
    let delegates = plan_tool_results(observed, Agent::BlockerSubowner, b"delegate");
    let answer_at = blocker_delivery(observed, Agent::BlockerSubowner, BLOCKER_ANSWER);
    step_check(
        &mut misses,
        "6.2",
        delegates.len() == 2
            && delegates
                .iter()
                .filter(|(_, args, _)| {
                    args.pointer("/batch/0/executor/kind") == Some(&Value::String("procedure".into()))
                })
                .count()
                == 1
            && delegates.iter().any(|(at, args, out)| {
                answer_at.is_some_and(|answer| *at > answer)
                    && contains(out, &format!("Delegated tasks: {BLOCKER_E1}"))
                    && args["batch"].as_array().is_some_and(|batch| {
                        batch.len() == 1
                            && batch[0]["executor"] == serde_json::json!({"kind":"procedure","connector":1,"code":2})
                            && batch[0]["spec"]["words"] == BLOCKER_E1_SPEC
                    })
            })
            && text(task_document(observed, Read::BlockerE1Ended), "/spec/specification/words/bytes")
                == BLOCKER_E1_SPEC,
        BLOCKER_CHECKPOINT,
        "A1 must delegate the singleton e1 change in a turn after receiving A's answer",
    );
    let amendments = plan_tool_results(observed, Agent::BlockerSubowner, b"amend");
    let kept = task_document(observed, Read::BlockerKept);
    step_check(
        &mut misses,
        "6.2",
        amendments.len() == 1
            && amendments.iter().all(|(_, args, out)| {
                args["target"] == BLOCKER_A3
                    && args["revision"] == BLOCKER_REVISION - 1
                    && args["amendment"]["dependencies"] == serde_json::json!([BLOCKER_E1])
                    && args["amendment"]["reason"] == BLOCKER_REASON
                    && contains(out, &format!("Done: task {BLOCKER_A3} is at spec revision {BLOCKER_REVISION}"))
            })
            && blocker_dependency(observed, Read::BlockerKept, true)
            && blocker_dependency(observed, Read::BlockerA3Ended, false)
            && kept["dependency_facts"][0]["keeps"] == true
            && number(kept, "/spec_revision/value") == Some(BLOCKER_REVISION)
            && number(a3, "/spec_revision/value") == Some(BLOCKER_REVISION)
            && plan_deliveries(observed, Agent::BlockerImplementer, BLOCKER_REASON) == 1
            && blocker_live_amendment(observed),
        BLOCKER_DEPENDENCY,
        "A1's successful amendment of live a3 must add e1 alone at the fresh spec revision",
    );
    blocker_kept(observed, &mut misses);
    blocker_landing(observed, &mut misses);
    misses
}

fn blocker_identities(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{A_DEADLINE, A1_DEADLINE, BLOCKER_E1, BLOCKER_OWNER, PROJECT};
    for (read, task, parent) in blocker_tasks() {
        let doc = task_document(observed, read);
        step_check(
            misses,
            "6.2",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && (parent == 0 || number(doc, "/requester/task") == Some(parent))
                && text(doc, "/chip/phase") == "ended"
                && doc.pointer("/result/ending/task/done").is_some_and(Value::is_object),
            BLOCKER_DEPENDENCY,
            &format!("{read:?} must identify done task {task} under {parent}"),
        );
        if task != BLOCKER_E1 {
            step_check(
                misses,
                "6.2",
                batch_journal(observed, read, task),
                BLOCKER_ATTEMPTS,
                &format!(
                    "task {task}'s complete journal must end every attempt, allowing only bounded zero-spend rechecks"
                ),
            );
        }
    }
    let a = task_document(observed, Read::BlockerOwnerEnded);
    let a1 = task_document(observed, Read::BlockerSubownerEnded);
    let a3 = task_document(observed, Read::BlockerA3Ended);
    step_check(
        misses,
        "6.2",
        number(a, "/goal") == Some(BLOCKER_OWNER)
            && number(a, "/authority/authority/deadline") == Some(A_DEADLINE)
            && number(a, "/authority/authority/delegation/depth") == Some(4)
            && number(a1, "/authority/authority/deadline") == Some(A1_DEADLINE)
            && number(a1, "/authority/authority/delegation/depth") == Some(3)
            && number(a3, "/authority/authority/delegation/depth") == Some(0),
        BLOCKER_DEPENDENCY,
        "A must be tracked, with A1's deadline and the least depths for the change tree",
    );
}

fn blocker_kept(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{Agent, BLOCKER_A3, BLOCKER_A3_REPORT, BLOCKER_E1, BLOCKER_REVIEWER};
    let kept = task_document(observed, Read::BlockerKept);
    let pending = task_document(observed, Read::BlockerPending);
    let before = check_activity(observed, Read::BlockerKept);
    let after = check_activity(observed, Read::BlockerA3Ended);
    let kept_row = before.iter().find(|r| r.pointer("/line/kept").is_some());
    let met = format!("T{BLOCKER_E1} is done; the task can go on");
    step_check(
        misses,
        "6.2",
        number(kept, "/chip/task") == Some(BLOCKER_A3)
            && text(kept, "/chip/phase") == "idle"
            && kept.get("result") == Some(&Value::Null)
            && kept.pointer("/waits_on/dependencies/pending") == Some(&serde_json::json!([BLOCKER_E1]))
            && number(pending, "/chip/task") == Some(BLOCKER_E1)
            && text(pending, "/chip/phase") == "idle"
            && pending.get("result") == Some(&Value::Null)
            && batch_writing(observed, Read::BlockerReviewWriting, BLOCKER_REVIEWER, Read::BlockerReviewerEnded)
            && kept_row.is_some_and(|row| {
                row.pointer("/line/kept/pending") == Some(&serde_json::json!([BLOCKER_E1]))
                    && number(row, "/line/kept/attempt").is_some_and(|attempt| {
                        before.iter().any(|r| {
                            number(r, "/line/run_ended/attempt") == Some(attempt)
                                && text(r, "/line/run_ended/end") == "parked"
                        })
                    })
                    && number(row, "/position").is_some_and(|position| {
                        !before.iter().any(|r| {
                            r.pointer("/line/run_began").is_some()
                                && number(r, "/position").is_some_and(|p| p > position)
                        }) && after.iter().any(|r| {
                            r.pointer("/line/run_began").is_some()
                                && number(r, "/position").is_some_and(|p| p > position)
                        })
                    })
            })
            && before.iter().all(|row| after.contains(row))
            && plan_deliveries(observed, Agent::BlockerImplementer, &met) == 1
            && blocker_resumed(observed)
            && text(task_document(observed, Read::BlockerA3Ended), "/result/value/report/bytes") == BLOCKER_A3_REPORT,
        BLOCKER_KEPT,
        "a3's saved done run must be kept across the pending held review, then resume on one identified Met notice",
    );
}
// The kept read is a prefix of the final source journal. The held review
// remains pending through that read and is released afterwards by the acts.
// Met after that prefix, followed by its resumed run, orders the query after
// the read without comparing clocks or positions belonging to different tasks.
fn blocker_live_amendment(observed: &Observed) -> bool {
    use super::{BLOCKER_AMENDMENT_NUMBER, BLOCKER_REASON, BLOCKER_REVISION, BLOCKER_SUBOWNER};
    let rows = check_activity(observed, Read::BlockerKept);
    let amendment = rows.iter().find(|r| number(r, "/line/heard/number") == Some(BLOCKER_AMENDMENT_NUMBER));
    amendment.is_some_and(|row| {
        number(row, "/line/heard/from/task") == Some(BLOCKER_SUBOWNER)
            && number(row, "/line/heard/kind/amendment/revision") == Some(BLOCKER_REVISION)
            && text(row, "/line/heard/words") == BLOCKER_REASON
            && row.pointer("/line/heard/eligible") == Some(&Value::Bool(true))
            && number(row, "/position").is_some_and(|at| {
                rows.iter().any(|r| {
                    number(r, "/line/kept/attempt").is_some_and(|attempt| {
                        rows.iter().any(|r| {
                            number(r, "/line/run_began/attempt") == Some(attempt)
                                && number(r, "/position").is_some_and(|begin| begin < at)
                        }) && rows.iter().any(|r| {
                            number(r, "/line/turn/attempt") == Some(attempt)
                                && number(r, "/line/turn/read") == Some(BLOCKER_AMENDMENT_NUMBER)
                                && number(r, "/position").is_some_and(|turn| turn > at)
                        })
                    })
                })
            })
    })
}
fn blocker_resumed(observed: &Observed) -> bool {
    use super::{BLOCKER_E1, BLOCKER_KEPT_REPORT, BLOCKER_REASON};
    use skein_fake_llm_domain::api::Part;
    let before = check_activity(observed, Read::BlockerKept);
    let after = check_activity(observed, Read::BlockerA3Ended);
    let met_rows: Vec<_> = after.iter().filter(|r| r.pointer("/line/heard/kind/met").is_some()).collect();
    let met = format!("T{BLOCKER_E1} is done; the task can go on");
    let reason_at = blocker_delivery(observed, Agent::BlockerImplementer, BLOCKER_REASON);
    let met_at = blocker_delivery(observed, Agent::BlockerImplementer, &met);
    met_rows.len() == 1
        && met_rows.iter().all(|row| {
            number(row, "/line/heard/from/task") == Some(BLOCKER_E1)
                && number(row, "/line/heard/kind/met/dependency") == Some(BLOCKER_E1)
                && row.pointer("/line/heard/kind/met/removed") == Some(&Value::Bool(false))
                && number(row, "/position")
                    .zip(before.first().and_then(|r| number(r, "/position")))
                    .is_some_and(|(met, read)| met > read)
                && number(row, "/line/heard/number").is_some_and(|message| {
                    after.iter().any(|r| {
                        number(r, "/line/run_began/why/message/message") == Some(message)
                            && r.pointer("/line/run_began/resumed") == Some(&Value::Bool(true))
                            && number(r, "/position").zip(number(row, "/position")).is_some_and(|(run, met)| run > met)
                    })
                })
        })
        && reason_at.zip(met_at).is_some_and(|(reason, met)| reason < met)
        && met_at.is_some_and(|at| {
            observed.told[at].history.iter().flat_map(|m| &m.parts).any(|p| {
                if let Part::ToolCall { name, arguments, .. } = p
                    && name.as_ref() == b"finish"
                    && let Ok(args) = serde_json::from_slice::<Value>(arguments)
                {
                    args["form"] == "report" && args["text"] == BLOCKER_KEPT_REPORT
                } else {
                    false
                }
            })
        })
}
fn blocker_landing(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{BLOCKER_DIRECTOR, BLOCKER_E1, BLOCKER_FILE, BLOCKER_FIXED, BLOCKER_PULL};
    let e1 = task_document(observed, Read::BlockerE1Ended);
    step_check(
        misses,
        "6.2",
        observed.kit.merged_green
            && observed.kit.main.get(BLOCKER_FILE.as_bytes()).is_some_and(|v| v == BLOCKER_FIXED.as_bytes())
            && number(e1, "/result/ending/task/done/change/kind") == Some(1)
            && number(e1, "/result/value/change/part/connector") == Some(1)
            && text(e1, "/result/value/change/part/kind") == "result"
            && observed.kit.pulls.len() == 1
            && observed.kit.pulls.iter().all(|pull| {
                pull.number == BLOCKER_PULL
                    && pull.branch == format!("temper/{BLOCKER_DIRECTOR}/c{BLOCKER_E1}").as_bytes()
                    && pull.merged
                    && pull.pushed == Some(pull.opened)
                    && pull.merged_head == pull.pushed
                    && pull
                        .landed
                        .is_some_and(|commit| text(e1, "/result/value/change/words/bytes") == direction_commit(commit))
            }),
        BLOCKER_LANDING,
        "e1's only pull must merge its green pushed head and leave B's fixed file on kit main",
    );
}

#[cfg(test)]
mod blocker_tests {
    use super::batch_tests::{input, journal, told, tool};
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Message, Part};

    fn documents() -> BTreeMap<Read, Value> {
        let mut documents = BTreeMap::new();
        let mut lines = BTreeMap::<Read, Vec<Value>>::new();
        for (read, task, parent) in blocker_tasks() {
            documents.insert(
                read,
                json!({"task":{
                    "chip":{"task":task,"phase":"ended"},"project":PROJECT,"requester":{"task":parent},
                    "goal":BLOCKER_OWNER,"authority":{"authority":{
                        "deadline":if task == BLOCKER_OWNER {A_DEADLINE} else {A1_DEADLINE},
                        "delegation":{"depth":match task { BLOCKER_OWNER => 4, BLOCKER_SUBOWNER => 3, _ => 0 }}
                    }},"result":{"ending":{"task":{"done":{}}}}
                }}),
            );
            lines.insert(read, vec![json!({"run_began":{"attempt":1}})]);
        }
        for exchange in blocker_exchanges() {
            let kind = exchange.question.map_or_else(
                || json!(if exchange.message == BLOCKER_FINDING_NUMBER { "words" } else { "question" }),
                |q| json!({"answer":{"question":q}}),
            );
            lines.get_mut(&exchange.read).expect("lines").push(json!({"words":{
                "message":exchange.message,"by":{"task":exchange.by},"kind":kind,"words":exchange.words}}));
        }
        for (read, task, _) in blocker_tasks() {
            let rows = lines.get_mut(&read).expect("lines");
            rows.push(json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}));
            documents.get_mut(&read).expect("task")["activity"] = journal(task, rows);
        }
        let kept_lines = [
            json!({"run_began":{"attempt":1}}),
            json!({"heard":{"number":BLOCKER_AMENDMENT_NUMBER,"from":{"task":BLOCKER_SUBOWNER},
                "kind":{"amendment":{"revision":BLOCKER_REVISION}},"words":BLOCKER_REASON,"eligible":true}}),
            json!({"turn":{"attempt":1,"read":BLOCKER_AMENDMENT_NUMBER}}),
            json!({"run_ended":{"attempt":1,"end":"parked","spent":1}}),
            json!({"kept":{"attempt":1,"pending":[BLOCKER_E1]}}),
        ];
        let mut ended_lines = kept_lines.to_vec();
        ended_lines.extend([
            json!({"heard":{"number":42,"from":{"task":BLOCKER_E1},
            "kind":{"met":{"dependency":BLOCKER_E1,"removed":false}},"words":""}}),
            json!({"run_began":{"attempt":2,"resumed":true,"why":{"message":{"message":42}}}}),
            json!({"run_ended":{"attempt":2,"end":"finished","spent":1}}),
        ]);
        documents.get_mut(&Read::BlockerA3Ended).expect("a3")["activity"] = journal(BLOCKER_A3, &ended_lines);
        documents.get_mut(&Read::BlockerA3Ended).expect("a3")["task"]["result"]["value"] =
            json!({"report":{"bytes":BLOCKER_A3_REPORT}});
        let mut kept = documents[&Read::BlockerA3Ended].clone();
        kept["task"]["chip"]["phase"] = json!("idle");
        kept["task"]["result"] = Value::Null;
        kept["task"]["waits_on"] = json!({"dependencies":{"pending":[BLOCKER_E1],"count":1}});
        kept["activity"] = journal(BLOCKER_A3, &kept_lines);
        documents.insert(Read::BlockerKept, kept);
        for (read, pending) in [(Read::BlockerKept, true), (Read::BlockerA3Ended, false)] {
            let task = &mut documents.get_mut(&read).expect("a3")["task"];
            task["spec_revision"] = json!({"value":BLOCKER_REVISION});
            task["dependency_facts"] = json!([{"number":BLOCKER_E1,"pending":pending,"keeps":pending,
                "added":{"at":1,"by":{"task":BLOCKER_SUBOWNER}}}]);
        }
        documents
            .insert(Read::BlockerPending, json!({"task":{"chip":{"task":BLOCKER_E1,"phase":"idle"},"result":null}}));
        documents.insert(Read::BlockerReviewWriting, json!({"task":{"chip":{"task":BLOCKER_REVIEWER,"phase":"running",
            "details":{"owner":{"phase":{"active":{"running":{"attempt":1}}},"details":{"observation":{"work":"writing"}}}}}}}));
        let e1 = &mut documents.get_mut(&Read::BlockerE1Ended).expect("e1")["task"];
        e1["spec"] = json!({"specification":{"words":{"bytes":BLOCKER_E1_SPEC}}});
        e1["result"] = json!({"ending":{"task":{"done":{"change":{"kind":1}}}},
            "value":{"change":{"part":{"connector":1,"kind":"result"},"words":{"bytes":direction_commit(9)}}}});
        documents
    }
    fn queries() -> Vec<Told> {
        let mut histories = BTreeMap::<u64, Vec<Message>>::new();
        let mut queries = Vec::new();
        let history = histories.entry(BLOCKER_SUBOWNER).or_default();
        history.extend(tool("a3", "delegate", &json!({"batch":[{}]}), &format!("Delegated tasks: {BLOCKER_A3}")));
        queries.push(told(Agent::BlockerSubowner, history));
        for exchange in blocker_exchanges() {
            let form = if exchange.question.is_some() {
                "answer"
            } else if exchange.message == BLOCKER_FINDING_NUMBER {
                "words"
            } else {
                "question"
            };
            let history = histories.entry(exchange.by).or_default();
            history.extend(tool(
                &format!("message{}", exchange.message),
                "message",
                &json!({"target":exchange.target,"form":form,"question":exchange.question,"words":exchange.words}),
                &format!("Message {} sent", exchange.message),
            ));
            queries.push(told(exchange.sender, history));
            let history = histories.entry(exchange.target).or_default();
            history.push(input(exchange.words));
            queries.push(told(exchange.receiver, history));
        }
        let history = histories.entry(BLOCKER_SUBOWNER).or_default();
        history.extend(tool(
            "e1",
            "delegate",
            &json!({"batch":[{"executor":{"kind":"procedure","connector":1,"code":2},
            "spec":{"words":BLOCKER_E1_SPEC}}]}),
            &format!("Delegated tasks: {BLOCKER_E1}"),
        ));
        queries.push(told(Agent::BlockerSubowner, history));
        history.extend(tool(
            "amend",
            "amend",
            &json!({"target":BLOCKER_A3,"revision":BLOCKER_REVISION-1,
            "amendment":{"dependencies":[BLOCKER_E1],"reason":BLOCKER_REASON}}),
            &format!("Done: task {BLOCKER_A3} is at spec revision {BLOCKER_REVISION}"),
        ));
        queries.push(told(Agent::BlockerSubowner, history));
        let history = histories.entry(BLOCKER_A3).or_default();
        history.push(input(BLOCKER_REASON));
        queries.push(told(Agent::BlockerImplementer, history));
        history.extend(tool("kept", "finish", &json!({"form":"report","text":BLOCKER_KEPT_REPORT}), "accepted"));
        history.push(input(&format!("T{BLOCKER_E1} is done; the task can go on")));
        queries.push(told(Agent::BlockerImplementer, history));
        queries
    }
    fn met() -> Observed {
        Observed {
            documents: documents(),
            told: queries(),
            commands: Vec::new(),
            kit: Kit {
                records: Vec::new(),
                main: BTreeMap::from([(BLOCKER_FILE.as_bytes().into(), BLOCKER_FIXED.as_bytes().into())]),
                merged_green: true,
                pulls: vec![Pull {
                    number: BLOCKER_PULL,
                    branch: format!("temper/{BLOCKER_DIRECTOR}/c{BLOCKER_E1}").into_bytes(),
                    opened: 8,
                    pushed: Some(8),
                    merged_head: Some(8),
                    landed: Some(9),
                    merged: true,
                    closed: false,
                    deleted: None,
                }],
            },
        }
    }
    fn rejects(observed: &Observed, promise: Promise) {
        let misses = step_6_2(observed);
        assert!(
            misses.iter().any(|miss| miss.step == "6.2" && miss.promise == promise),
            "mutation must fail {promise:?}; misses: {}",
            misses.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        );
    }
    #[test]
    fn blocker_observation_meets_the_step() {
        assert_met(&step_6_2(&met()));
    }

    #[test]
    fn each_task_identity_and_ending_is_required() {
        let passing = met();
        for (read, _, parent) in blocker_tasks() {
            for path in [
                "/task/chip/task",
                "/task/project",
                "/task/requester/task",
                "/task/chip/phase",
                "/task/result/ending/task/done",
            ] {
                if parent == 0 && path == "/task/requester/task" {
                    continue;
                }
                let mut changed = passing.clone();
                *changed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
                rejects(&changed, BLOCKER_DEPENDENCY);
            }
        }
        for (read, path) in [
            (Read::BlockerOwnerEnded, "/task/goal"),
            (Read::BlockerOwnerEnded, "/task/authority/authority/deadline"),
            (Read::BlockerOwnerEnded, "/task/authority/authority/delegation/depth"),
            (Read::BlockerSubownerEnded, "/task/authority/authority/deadline"),
            (Read::BlockerSubownerEnded, "/task/authority/authority/delegation/depth"),
            (Read::BlockerA3Ended, "/task/authority/authority/delegation/depth"),
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
            rejects(&changed, BLOCKER_DEPENDENCY);
        }
    }
    fn mutate_tool(observed: &mut Observed, id: &[u8], field: Option<&str>) {
        let mut edits = 0;
        for query in &mut observed.told {
            for message in &mut query.history {
                for part in &mut message.parts {
                    match part {
                        Part::ToolCall { id: name, arguments, .. } if name.as_ref() == id && field.is_some() => {
                            let mut args: Value = serde_json::from_slice(arguments).expect("args");
                            args[field.expect("field")] = Value::Null;
                            *arguments = serde_json::to_vec(&args).expect("args").into();
                            edits += 1;
                        }
                        Part::ToolOutput { id: name, output, .. } if name.as_ref() == id && field.is_none() => {
                            *output = b"Refused".as_slice().into();
                            edits += 1;
                        }
                        Part::Opaque { .. } | Part::Text { .. } | Part::ToolCall { .. } | Part::ToolOutput { .. } => {}
                    }
                }
            }
        }
        assert!(edits > 0);
    }
    #[test]
    fn every_exchange_requires_its_numbered_call_and_unique_ordered_delivery() {
        let passing = met();
        for exchange in blocker_exchanges() {
            let id = format!("message{}", exchange.message);
            for field in [Some("target"), Some("form"), Some("words"), None] {
                let mut changed = passing.clone();
                mutate_tool(&mut changed, id.as_bytes(), field);
                rejects(&changed, BLOCKER_MESSAGES);
            }
            if exchange.question.is_some() {
                let mut changed = passing.clone();
                mutate_tool(&mut changed, id.as_bytes(), Some("question"));
                rejects(&changed, BLOCKER_MESSAGES);
            }
            for field in ["message", "by", "kind", "words"] {
                let mut changed = passing.clone();
                for row in changed.documents.get_mut(&exchange.read).expect("doc")["activity"]["rows"]
                    .as_array_mut()
                    .expect("rows")
                {
                    if row.pointer("/event/owner/source/line/words/message") == Some(&json!(exchange.message)) {
                        row["event"]["owner"]["source"]["line"]["words"][field] = Value::Null;
                    }
                }
                rejects(&changed, BLOCKER_MESSAGES);
            }
            let mut duplicate = passing.clone();
            duplicate.told.push(told(exchange.receiver, &[input(exchange.words)]));
            rejects(&duplicate, BLOCKER_MESSAGES);
            let mut missing = passing.clone();
            missing
                .told
                .retain(|t| !(t.agent == exchange.receiver && user_words(t).any(|w| contains(w, exchange.words))));
            rejects(&missing, BLOCKER_MESSAGES);
        }
        let mut reversed = passing.clone();
        reversed.told.reverse();
        rejects(&reversed, BLOCKER_MESSAGES);
    }
    #[test]
    fn e1_requires_the_answer_checkpoint() {
        let passing = met();
        for field in [Some("batch"), None] {
            let mut changed = passing.clone();
            mutate_tool(&mut changed, b"e1", field);
            rejects(&changed, BLOCKER_CHECKPOINT);
        }
        let mut early = passing.clone();
        let at = early
            .told
            .iter()
            .position(|t| {
                t.history
                    .iter()
                    .flat_map(|m| &m.parts)
                    .any(|p| matches!(p,Part::ToolOutput {id,..} if id.as_ref()==b"e1"))
            })
            .expect("e1 query");
        let query = early.told.remove(at);
        early.told.insert(0, query);
        rejects(&early, BLOCKER_CHECKPOINT);
        let mut wrong = passing.clone();
        wrong.documents.get_mut(&Read::BlockerE1Ended).expect("e1")["task"]["spec"] = Value::Null;
        rejects(&wrong, BLOCKER_CHECKPOINT);
    }
    #[test]
    fn live_amendment_and_its_visible_dependency_are_required() {
        let passing = met();
        for field in [Some("target"), Some("revision"), Some("amendment"), None] {
            let mut changed = passing.clone();
            mutate_tool(&mut changed, b"amend", field);
            rejects(&changed, BLOCKER_DEPENDENCY);
        }
        for read in [Read::BlockerKept, Read::BlockerA3Ended] {
            for path in [
                "/task/dependency_facts/0/number",
                "/task/dependency_facts/0/pending",
                "/task/dependency_facts/0/added/by/task",
                "/task/dependency_facts/0/added/at",
                "/task/spec_revision/value",
            ] {
                let mut changed = passing.clone();
                *changed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
                rejects(&changed, BLOCKER_DEPENDENCY);
            }
            let mut extra = passing.clone();
            extra.documents.get_mut(&read).expect("doc")["task"]["dependency_facts"]
                .as_array_mut()
                .expect("facts")
                .push(json!({"number":BLOCKER_OWNER}));
            rejects(&extra, BLOCKER_DEPENDENCY);
        }
        let mut changed = passing.clone();
        changed.documents.get_mut(&Read::BlockerKept).expect("kept")["task"]["dependency_facts"][0]["keeps"] =
            json!(false);
        rejects(&changed, BLOCKER_DEPENDENCY);
        let mut changed = passing.clone();
        changed
            .told
            .retain(|t| !(t.agent == Agent::BlockerImplementer && user_words(t).any(|w| contains(w, BLOCKER_REASON))));
        rejects(&changed, BLOCKER_DEPENDENCY);
    }
    #[test]
    fn kept_read_requires_the_saved_attempt_pending_review_and_later_met() {
        let passing = met();
        for (read, path, value) in [
            (Read::BlockerKept, "/task/chip/task", Value::Null),
            (Read::BlockerKept, "/task/chip/phase", json!("ended")),
            (Read::BlockerKept, "/task/result", json!({})),
            (Read::BlockerKept, "/task/waits_on", Value::Null),
            (Read::BlockerPending, "/task/chip/task", Value::Null),
            (Read::BlockerPending, "/task/chip/phase", json!("ended")),
            (Read::BlockerPending, "/task/result", json!({})),
            (Read::BlockerReviewWriting, "/task/chip/task", Value::Null),
            (Read::BlockerReviewWriting, "/task/chip/phase", json!("ended")),
            (Read::BlockerReviewWriting, "/task/chip/details/owner/details/observation/work", Value::Null),
            (Read::BlockerReviewWriting, "/task/chip/details/owner/phase/active/running/attempt", json!(99)),
            (Read::BlockerA3Ended, "/task/result/value/report/bytes", Value::Null),
            (Read::BlockerKept, "/activity/rows/0/event/owner/source/line/kept/pending", json!([])),
            (Read::BlockerKept, "/activity/rows/0/event/owner/source/line/kept/attempt", json!(99)),
            (Read::BlockerKept, "/activity/rows/1/event/owner/source/line/run_ended/end", json!("finished")),
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = value;
            rejects(&changed, BLOCKER_KEPT);
        }
        let met_words = format!("T{BLOCKER_E1} is done; the task can go on");
        let mut missing = passing.clone();
        missing.told.pop();
        rejects(&missing, BLOCKER_KEPT);
        let mut duplicate = passing.clone();
        duplicate.told.push(told(Agent::BlockerImplementer, &[input(&met_words)]));
        rejects(&duplicate, BLOCKER_KEPT);
        let mut no_resume = passing.clone();
        no_resume.documents.get_mut(&Read::BlockerA3Ended).expect("a3")["activity"] = journal(BLOCKER_A3, &[]);
        rejects(&no_resume, BLOCKER_KEPT);
    }
    #[test]
    fn amendment_must_reach_the_kept_attempt_and_met_must_start_a_later_saved_run() {
        let passing = met();
        for path in [
            "/activity/rows/3/event/owner/source/line/heard/number",
            "/activity/rows/3/event/owner/source/line/heard/from/task",
            "/activity/rows/3/event/owner/source/line/heard/kind/amendment/revision",
            "/activity/rows/3/event/owner/source/line/heard/words",
            "/activity/rows/3/event/owner/source/line/heard/eligible",
            "/activity/rows/2/event/owner/source/line/turn/attempt",
            "/activity/rows/2/event/owner/source/line/turn/read",
            "/activity/rows/4/event/owner/source/line/run_began/attempt",
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&Read::BlockerKept).expect("kept").pointer_mut(path).expect("field") =
                Value::Null;
            rejects(&changed, BLOCKER_DEPENDENCY);
        }
        for path in [
            "/activity/rows/2/event/owner/source/line/heard/number",
            "/activity/rows/2/event/owner/source/line/heard/from/task",
            "/activity/rows/2/event/owner/source/line/heard/kind/met/dependency",
            "/activity/rows/2/event/owner/source/line/heard/kind/met/removed",
            "/activity/rows/2/event/owner/source/position",
            "/activity/rows/1/event/owner/source/line/run_began/resumed",
            "/activity/rows/1/event/owner/source/line/run_began/why/message/message",
            "/activity/rows/1/event/owner/source/position",
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&Read::BlockerA3Ended).expect("a3").pointer_mut(path).expect("field") =
                Value::Null;
            rejects(&changed, BLOCKER_KEPT);
        }
        let mut removed = passing.clone();
        removed.documents.get_mut(&Read::BlockerA3Ended).expect("a3")["activity"]["rows"][2]["event"]["owner"]["source"]
            ["line"]["heard"]["kind"]["met"]["removed"] = json!(true);
        rejects(&removed, BLOCKER_KEPT);
        let mut duplicate = passing.clone();
        let rows = duplicate.documents.get_mut(&Read::BlockerA3Ended).expect("a3")["activity"]["rows"]
            .as_array_mut()
            .expect("rows");
        rows.push(rows[2].clone());
        rejects(&duplicate, BLOCKER_KEPT);
        for field in ["form", "text"] {
            let mut changed = passing.clone();
            mutate_tool(&mut changed, b"kept", Some(field));
            rejects(&changed, BLOCKER_KEPT);
        }
    }
    #[test]
    fn landing_requires_the_fixed_file_and_green_merge_at_the_pushed_head() {
        let passing = met();
        for mutation in 0..10 {
            let mut changed = passing.clone();
            match mutation {
                0 => changed.kit.merged_green = false,
                1 => {
                    changed.kit.main.insert(BLOCKER_FILE.as_bytes().into(), BLOCKER_WRONG.as_bytes().into());
                }
                2 => changed.kit.pulls.clear(),
                3 => changed.kit.pulls[0].number += 1,
                4 => changed.kit.pulls[0].branch = b"wrong".to_vec(),
                5 => changed.kit.pulls[0].merged = false,
                6 => changed.kit.pulls[0].pushed = None,
                7 => changed.kit.pulls[0].merged_head = Some(7),
                8 => changed.kit.pulls[0].landed = None,
                _ => changed.kit.pulls[0].landed = Some(10),
            }
            rejects(&changed, BLOCKER_LANDING);
        }
        for path in [
            "/task/result/ending/task/done/change/kind",
            "/task/result/value/change/part/connector",
            "/task/result/value/change/part/kind",
            "/task/result/value/change/words/bytes",
        ] {
            let mut changed = passing.clone();
            *changed.documents.get_mut(&Read::BlockerE1Ended).expect("e1").pointer_mut(path).expect("field") =
                Value::Null;
            rejects(&changed, BLOCKER_LANDING);
        }
    }
    #[test]
    fn every_agents_attempts_must_end_and_rechecks_must_be_unspent_and_uncommitted() {
        let passing = met();
        for (read, task, _) in blocker_tasks() {
            if task == BLOCKER_E1 {
                continue;
            }
            for end in [json!("refused"), json!({"failed":{}}), Value::Null] {
                let mut changed = passing.clone();
                changed.documents.get_mut(&read).expect("doc")["activity"] = journal(
                    task,
                    &[json!({"run_began":{"attempt":1}}), json!({"run_ended":{"attempt":1,"end":end,"spent":1}})],
                );
                rejects(&changed, BLOCKER_ATTEMPTS);
            }
            let mut accepted = passing.clone();
            let mut lines =
                vec![json!({"run_began":{"attempt":8}}), json!({"run_ended":{"attempt":8,"end":"refused","spent":0}})];
            lines.extend(check_activity(&accepted, read).iter().rev().map(|row| row["line"].clone()));
            accepted.documents.get_mut(&read).expect("doc")["activity"] = journal(task, &lines);
            if task == BLOCKER_A3 {
                let mut before = lines[..2].to_vec();
                before.extend(
                    check_activity(&passing.clone(), Read::BlockerKept).iter().rev().map(|r| r["line"].clone()),
                );
                accepted.documents.get_mut(&Read::BlockerKept).expect("kept")["activity"] = journal(task, &before);
            }
            assert_met(&step_6_2(&accepted));
            for mutation in 0..4 {
                let mut changed = accepted.clone();
                let mut bad = lines.clone();
                match mutation {
                    0 => bad[1]["run_ended"]["spent"] = json!(1),
                    1 => bad.truncate(2),
                    2 => bad.insert(1, json!({"turn":{"attempt":8,"turn":1}})),
                    _ => {
                        bad.last_mut().expect("end")["run_ended"]["attempt"] = json!(99);
                    }
                }
                changed.documents.get_mut(&read).expect("doc")["activity"] = journal(task, &bad);
                rejects(&changed, BLOCKER_ATTEMPTS);
            }
            for field in ["cut", "before", "after"] {
                let mut changed = passing.clone();
                changed.documents.get_mut(&read).expect("doc")["activity"][field] = json!(true);
                rejects(&changed, BLOCKER_ATTEMPTS);
            }
            let mut gap = passing.clone();
            gap.documents.get_mut(&read).expect("doc")["activity"]["rows"][0]["event"]["owner"]["source"]["position"] =
                json!(99);
            rejects(&gap, BLOCKER_ATTEMPTS);
        }
    }
}

// Act 3, step 6.5 (domain/testing.md, 5.3)
const BUDGET_MESSAGES: Promise = Promise::Must("the budget question climbs two owners and its answers come down once");
const BUDGET_INBOX: Promise = Promise::Must("the person's inbox keeps the question until its answer");
const BUDGET_CHECKPOINT: Promise = Promise::Must("a5 is made after A1 hears the answer");
const BUDGET_EFFORT: Promise = Promise::Must("a5's effort is raised for its next try");
const BUDGET_ONESHOT: Promise = Promise::Must("one advisor reads a5's report on the owner's model");
const BUDGET_RESULT: Promise = Promise::Add("the advisor's report reaches A1 once");
const BUDGET_ATTEMPTS: Promise = Promise::Add("every attempt ended, none failed");

fn budget_tasks() -> [(Read, u64, u64); 5] {
    use super::{BUDGET_A5, BUDGET_ADVISOR, BUDGET_DIRECTOR, BUDGET_OWNER, BUDGET_SUBOWNER};
    [
        (Read::BudgetDirectorEnded, BUDGET_DIRECTOR, 0),
        (Read::BudgetOwnerEnded, BUDGET_OWNER, BUDGET_DIRECTOR),
        (Read::BudgetSubownerEnded, BUDGET_SUBOWNER, BUDGET_OWNER),
        (Read::BudgetA5Ended, BUDGET_A5, BUDGET_SUBOWNER),
        (Read::BudgetAdvisorEnded, BUDGET_ADVISOR, BUDGET_SUBOWNER),
    ]
}

fn budget_pages(read: Read) -> Vec<Read> {
    if read == Read::BudgetDirectorEnded {
        vec![read, Read::BudgetDirectorFirst]
    } else if read == Read::BudgetOwnerEnded {
        vec![read, Read::BudgetOwnerFirst]
    } else if read == Read::BudgetSubownerEnded {
        vec![read, Read::BudgetSubownerFirst]
    } else {
        vec![read]
    }
}
fn budget_rows(observed: &Observed, read: Read) -> Vec<&Value> {
    budget_pages(read).iter().flat_map(|read| check_activity(observed, *read)).collect()
}
fn budget_journal(observed: &Observed, read: Read, task: u64) -> bool {
    let pages = budget_pages(read);
    if pages.len() == 1 {
        return batch_journal(observed, read, task);
    }
    let rows = budget_rows(observed, read);
    // Apply 6.1's attempt rule across complete adjoining cursor pages.
    plan_attempts(observed, &pages, task, 1)
        && rows.last().is_some_and(|r| number(r, "/position") == Some(1))
        && rows.windows(2).all(|pair| {
            number(pair[0], "/position")
                .zip(number(pair[1], "/position"))
                .is_some_and(|(new, old)| new.checked_sub(1) == Some(old))
        })
        && rows.iter().enumerate().all(|(at, row)| {
            let Some(end) = row.pointer("/line/run_ended") else { return true };
            matches!(end["end"].as_str(), Some("parked" | "finished"))
                || (end["end"] == "refused"
                    && number(end, "/spent") == Some(0)
                    && number(end, "/attempt")
                        .is_some_and(|attempt| !rows.iter().any(|r| number(r, "/line/turn/attempt") == Some(attempt)))
                    && rows[..at].iter().any(|r| text(r, "/line/run_ended/end") == "finished"))
        })
}

#[must_use]
pub fn step_6_5(observed: &Observed) -> Vec<Miss> {
    use super::{A_DEADLINE, A1_DEADLINE, BUDGET_OWNER, PROJECT};
    let mut misses = Vec::new();
    for (read, task, parent) in budget_tasks() {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "6.5",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && (parent == 0 || number(doc, "/requester/task") == Some(parent))
                && text(doc, "/chip/phase") == "ended"
                && doc.pointer("/result/ending/task/done").is_some_and(Value::is_object),
            BUDGET_CHECKPOINT,
            &format!("{read:?} must identify done task {task} under {parent}"),
        );
        step_check(
            &mut misses,
            "6.5",
            budget_journal(observed, read, task),
            BUDGET_ATTEMPTS,
            &format!(
                "task {task}'s whole ordered journal must end all attempts, allowing only unspent uncommitted rechecks"
            ),
        );
    }
    let a = task_document(observed, Read::BudgetOwnerEnded);
    let a1 = task_document(observed, Read::BudgetSubownerEnded);
    step_check(
        &mut misses,
        "6.5",
        number(a, "/goal") == Some(BUDGET_OWNER)
            && number(a, "/authority/authority/deadline") == Some(A_DEADLINE)
            && number(a1, "/authority/authority/deadline") == Some(A1_DEADLINE)
            && number(a1, "/authority/authority/delegation/depth") == Some(1),
        BUDGET_CHECKPOINT,
        "A must be tracked and A1 must carry M1's deadline and depth one",
    );
    budget_messages(observed, &mut misses);
    budget_person_answer(observed, &mut misses);
    budget_inbox(observed, &mut misses);
    budget_investigation(observed, &mut misses);
    budget_effort(observed, &mut misses);
    budget_advisor(observed, &mut misses);
    misses
}

fn budget_messages(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        BUDGET_ANSWER, BUDGET_ANSWER_NUMBER, BUDGET_DIRECTOR, BUDGET_OWNER, BUDGET_PERSON_ANSWER, BUDGET_QUESTION,
        BUDGET_QUESTION_NUMBER, BUDGET_RELAY_NUMBER, BUDGET_SUBOWNER, BUDGET_UP_QUESTION, BUDGET_UP_QUESTION_NUMBER,
    };
    let mut previous = None;
    for (agent, receiver, by, target, message, question, words, read) in [
        (
            Agent::BudgetSubowner,
            Agent::BudgetOwner,
            BUDGET_SUBOWNER,
            BUDGET_OWNER,
            BUDGET_QUESTION_NUMBER,
            None,
            BUDGET_QUESTION,
            Read::BudgetOwnerEnded,
        ),
        (
            Agent::BudgetOwner,
            Agent::BudgetDirector,
            BUDGET_OWNER,
            BUDGET_DIRECTOR,
            BUDGET_UP_QUESTION_NUMBER,
            None,
            BUDGET_UP_QUESTION,
            Read::BudgetDirectorEnded,
        ),
        (
            Agent::BudgetDirector,
            Agent::BudgetOwner,
            BUDGET_DIRECTOR,
            BUDGET_OWNER,
            BUDGET_RELAY_NUMBER,
            Some(BUDGET_UP_QUESTION_NUMBER),
            BUDGET_PERSON_ANSWER,
            Read::BudgetOwnerEnded,
        ),
        (
            Agent::BudgetOwner,
            Agent::BudgetSubowner,
            BUDGET_OWNER,
            BUDGET_SUBOWNER,
            BUDGET_ANSWER_NUMBER,
            Some(BUDGET_QUESTION_NUMBER),
            BUDGET_ANSWER,
            Read::BudgetSubownerEnded,
        ),
    ] {
        let delivery = blocker_delivery(observed, receiver, words);
        let ordered = delivery.is_some_and(|at| previous.is_none_or(|before| before < at));
        previous = delivery;
        let form = if question.is_some() { "answer" } else { "question" };
        let tools = plan_tool_results(observed, agent, b"message");
        let tools: Vec<_> = tools.iter().filter(|(_, args, _)| args["words"] == words).collect();
        let sent = tools.len() == 1
            && tools.iter().all(|(_, args, out)| {
                args["target"] == target
                    && args["form"] == form
                    && question.is_none_or(|q| args["question"] == q)
                    && contains(out, &format!("Message {message} sent"))
            });
        let recorded = budget_rows(observed, read)
            .iter()
            .filter(|row| {
                let kind = question
                    .map_or_else(|| Value::String(form.into()), |q| serde_json::json!({"answer":{"question":q}}));
                number(row, "/line/words/message") == Some(message)
                    && number(row, "/line/words/by/task") == Some(by)
                    && row.pointer("/line/words/kind") == Some(&kind)
                    && text(row, "/line/words/words") == words
            })
            .count()
            == 1;
        step_check(
            misses,
            "6.5",
            sent && ordered && recorded && plan_deliveries(observed, receiver, words) == 1,
            BUDGET_MESSAGES,
            &format!("message {message} must reach {receiver:?} once in the question/answer chain"),
        );
    }
}

fn budget_person_answer(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{BUDGET_PERSON_ANSWER, BUDGET_PERSON_ANSWER_NUMBER, BUDGET_UP_QUESTION, BUDGET_UP_QUESTION_NUMBER};
    let question = blocker_delivery(observed, Agent::BudgetDirector, BUDGET_UP_QUESTION);
    let person = blocker_delivery(observed, Agent::BudgetDirector, BUDGET_PERSON_ANSWER);
    let relay = blocker_delivery(observed, Agent::BudgetOwner, BUDGET_PERSON_ANSWER);
    step_check(
        misses,
        "6.5",
        question.zip(person).is_some_and(|(question, person)| question < person)
            && person.zip(relay).is_some_and(|(person, relay)| person < relay)
            && plan_deliveries(observed, Agent::BudgetDirector, BUDGET_PERSON_ANSWER) == 1
            && budget_rows(observed, Read::BudgetDirectorEnded)
                .iter()
                .filter(|r| {
                    number(r, "/line/words/message") == Some(BUDGET_PERSON_ANSWER_NUMBER)
                        && number(r, "/line/words/by/person") == Some(1)
                        && number(r, "/line/words/kind/answer/question") == Some(BUDGET_UP_QUESTION_NUMBER)
                        && text(r, "/line/words/words") == BUDGET_PERSON_ANSWER
                })
                .count()
                == 1,
        BUDGET_MESSAGES,
        "the person must wake the chat once after the question and before its relay to A",
    );
}

fn budget_inbox(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{BUDGET_DIRECTOR, BUDGET_UP_QUESTION_NUMBER, PROJECT};
    let rows = document(observed, Read::BudgetAsked).pointer("/needs/rows").and_then(Value::as_array);
    let ended = document(observed, Read::BudgetInboxAnswered).pointer("/needs/rows").and_then(Value::as_array);
    // The identified reads are the acts' JSON documents. Its command order
    // proves the first inbox read precedes answer and the final read follows.
    let commands: Vec<_> = observed.commands.iter().map(String::as_str).collect();
    step_check(
        misses,
        "6.5",
        rows.is_some_and(|rows| {
            rows.len() == 1
                && rows[0]["reply"].is_null()
                && number(&rows[0], "/task/live/task") == Some(BUDGET_DIRECTOR)
                && number(&rows[0], "/project") == Some(PROJECT)
                && number(&rows[0], "/object/question/message") == Some(BUDGET_UP_QUESTION_NUMBER)
        }) && document(observed, Read::BudgetAsked)["needs"]["cut"] == false
            && document(observed, Read::BudgetInboxAnswered)["needs"]["cut"] == false
            && ended.is_some_and(Vec::is_empty)
            && commands
                .iter()
                .position(|c| *c == "inbox")
                .zip(commands.iter().position(|c| *c == "answer"))
                .is_some_and(|(read, answer)| read < answer)
            && commands
                .iter()
                .position(|c| *c == "answer")
                .zip(commands.iter().rposition(|c| *c == "inbox"))
                .is_some_and(|(answer, read)| answer < read)
            && commands.iter().filter(|c| **c == "answer").count() == 1,
        BUDGET_INBOX,
        "the read before ctl answer must hold A's sole question, absent in the final inbox read",
    );
}

fn budget_investigation(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{BUDGET_A5, BUDGET_ANSWER, BUDGET_MEDIUM_NUMBER, DEFAULT_MODEL_NUMBER};
    let answer = blocker_delivery(observed, Agent::BudgetSubowner, BUDGET_ANSWER);
    let delegates = plan_tool_results(observed, Agent::BudgetSubowner, b"delegate");
    step_check(
        misses,
        "6.5",
        delegates.len() == 2
            && delegates.iter().filter(|(_, _, out)| contains(out, &format!("Delegated tasks: {BUDGET_A5}"))).count()
                == 1
            && delegates.iter().any(|(at, args, out)| {
                answer.is_some_and(|answer| answer < *at)
                && contains(out, &format!("Delegated tasks: {BUDGET_A5}"))
                && args["batch"].as_array().is_some_and(|batch| {
                    batch.len() == 1
                        && batch[0]["llm"]
                            == serde_json::json!({"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_MEDIUM_NUMBER}})
                })
            }),
        BUDGET_CHECKPOINT,
        "a5's singleton delegation on the default model at medium must follow A1 hearing the answer",
    );
}

fn budget_finding(observed: &Observed) -> bool {
    use super::{BUDGET_A5, BUDGET_FINDING, BUDGET_FINDING_NUMBER};
    plan_deliveries(observed, Agent::BudgetSubowner, BUDGET_FINDING) == 1
        && budget_rows(observed, Read::BudgetSubownerEnded)
            .iter()
            .filter(|r| {
                number(r, "/line/words/message") == Some(BUDGET_FINDING_NUMBER)
                    && number(r, "/line/words/by/task") == Some(BUDGET_A5)
                    && text(r, "/line/words/kind") == "words"
                    && text(r, "/line/words/words") == BUDGET_FINDING
            })
            .count()
            == 1
}

fn budget_effort(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        BUDGET_A5, BUDGET_A5_REPORT, BUDGET_AMENDMENT_NUMBER, BUDGET_HIGH, BUDGET_HIGH_NUMBER, BUDGET_MEDIUM,
        BUDGET_REASON, BUDGET_REVISION, BUDGET_SUBOWNER, DEFAULT_MODEL, DEFAULT_MODEL_NUMBER,
    };
    let medium = task_document(observed, Read::BudgetMedium);
    let high = task_document(observed, Read::BudgetHigh);
    let ended = task_document(observed, Read::BudgetA5Ended);
    let amendments = plan_tool_results(observed, Agent::BudgetSubowner, b"amend");
    let choices = [
        (medium, BUDGET_MEDIUM, BUDGET_REVISION - 1),
        (high, BUDGET_HIGH, BUDGET_REVISION),
        (ended, BUDGET_HIGH, BUDGET_REVISION),
    ];
    step_check(
        misses,
        "6.5",
        budget_finding(observed)
            && choices.iter().all(|(doc, effort, revision)| {
                number(doc, "/chip/task") == Some(BUDGET_A5)
                    && text(doc, "/llm/main/model") == DEFAULT_MODEL
                    && text(doc, "/llm/main/effort") == *effort
                    && number(doc, "/spec_revision/value") == Some(*revision)
            })
            && text(medium, "/chip/phase") == "idle"
            && medium["result"].is_null()
            && amendments.len() == 1
            && amendments.iter().all(|(_, args, out)| {
                args["target"] == BUDGET_A5
                    && args["revision"] == BUDGET_REVISION - 1
                    && args["amendment"]["llm"]
                        == serde_json::json!({"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_HIGH_NUMBER}})
                    && args["amendment"]["reason"] == BUDGET_REASON
                    && contains(out, &format!("Done: task {BUDGET_A5} is at spec revision {BUDGET_REVISION}"))
            }),
        BUDGET_EFFORT,
        "a5 must change from medium to high at the amendment's next spec revision",
    );
    let before = check_activity(observed, Read::BudgetMedium);
    let after = check_activity(observed, Read::BudgetHigh);
    let reason: Vec<_> =
        after.iter().filter(|r| number(r, "/line/heard/number") == Some(BUDGET_AMENDMENT_NUMBER)).collect();
    let running = number(high, "/chip/details/owner/phase/active/running/attempt");
    step_check(
        misses,
        "6.5",
        batch_journal(observed, Read::BudgetMedium, BUDGET_A5)
            && batch_writing(observed, Read::BudgetHigh, BUDGET_A5, Read::BudgetA5Ended)
            && before.iter().all(|row| after.contains(row))
            && pause_activity(observed, Read::BudgetHigh)["cut"] == false
            && pause_activity(observed, Read::BudgetHigh)["before"].is_null()
            && after.iter().all(|r| number(r, "/task") == Some(BUDGET_A5))
            && after.windows(2).all(|pair| {
                number(pair[0], "/position")
                    .zip(number(pair[1], "/position"))
                    .is_some_and(|(new, old)| new.checked_sub(1) == Some(old))
            })
            && after.iter().filter(|r| number(r, "/line/decided/revision") == Some(BUDGET_REVISION)).count() == 1
            && reason.len() == 1
            && reason.iter().all(|r| {
                number(r, "/line/heard/from/task") == Some(BUDGET_SUBOWNER)
                    && number(r, "/line/heard/kind/amendment/revision") == Some(BUDGET_REVISION)
                    && text(r, "/line/heard/words") == BUDGET_REASON
                    && number(r, "/position").is_some_and(|amended| {
                        before.first().and_then(|r| number(r, "/position")).is_some_and(|read| read < amended)
                            && after.iter().any(|r| {
                                number(r, "/line/run_began/attempt") == running
                                    && running.is_some()
                                    && number(r, "/position").is_some_and(|began| {
                                        began > amended
                                            && after.iter().any(|decision| {
                                                number(decision, "/line/decided/revision") == Some(BUDGET_REVISION)
                                                    && number(decision, "/position")
                                                        .is_some_and(|decided| amended < decided && decided < began)
                                            })
                                            && !after.iter().any(|between| {
                                                between.pointer("/line/run_began").is_some()
                                                    && number(between, "/position")
                                                        .is_some_and(|at| amended < at && at < began)
                                            })
                                    })
                            })
                    })
            })
            && plan_deliveries(observed, Agent::BudgetInvestigator, BUDGET_REASON) == 1
            && text(ended, "/result/value/report/bytes") == BUDGET_A5_REPORT,
        BUDGET_EFFORT,
        "the next a5 run after the amendment must hear its reason once and report none again",
    );
}

fn budget_report(observed: &Observed, task: u64, message: u64, words: &str) -> bool {
    budget_rows(observed, Read::BudgetSubownerEnded)
        .iter()
        .filter(|r| {
            number(r, "/line/heard/number") == Some(message)
                && number(r, "/line/heard/from/task") == Some(task)
                && text(r, "/line/heard/kind/result") == "report"
                && text(r, "/line/heard/words") == words
        })
        .count()
        == 1
}

fn budget_advisor(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        BUDGET_A5, BUDGET_A5_REPORT, BUDGET_A5_REPORT_NUMBER, BUDGET_ADVISOR, BUDGET_ADVISOR_REPORT,
        BUDGET_ADVISOR_REPORT_NUMBER, BUDGET_ADVISOR_SPEC, OWNER_EFFORT_NUMBER, OWNER_MODEL, OWNER_MODEL_NUMBER,
    };
    let report_at = blocker_delivery(observed, Agent::BudgetSubowner, BUDGET_A5_REPORT);
    let delegated = plan_tool_results(observed, Agent::BudgetSubowner, b"delegate");
    let advisor = task_document(observed, Read::BudgetAdvisorEnded);
    let queries: Vec<_> = observed.told.iter().filter(|t| t.agent == Agent::BudgetAdvisor).collect();
    let rows = check_activity(observed, Read::BudgetAdvisorEnded);
    step_check(
        misses,
        "6.5",
        budget_report(observed, BUDGET_A5, BUDGET_A5_REPORT_NUMBER, BUDGET_A5_REPORT)
            && plan_deliveries(observed, Agent::BudgetSubowner, BUDGET_A5_REPORT) == 1
            && delegated
                .iter()
                .filter(|(_, _, out)| contains(out, &format!("Delegated tasks: {BUDGET_ADVISOR}")))
                .count()
                == 1
            && delegated.iter().any(|(at, args, out)| {
                report_at.is_some_and(|report| report < *at)
                    && contains(out, &format!("Delegated tasks: {BUDGET_ADVISOR}"))
                    && args["batch"].as_array().is_some_and(|batch| {
                        batch.len() == 1
                            && batch[0]["spec"]["words"] == BUDGET_ADVISOR_SPEC
                            && batch[0]["llm"]
                                == serde_json::json!({"main":{"model":OWNER_MODEL_NUMBER,"effort":OWNER_EFFORT_NUMBER}})
                    })
            })
            && text(advisor, "/spec/specification/words/bytes") == BUDGET_ADVISOR_SPEC
            && text(advisor, "/llm/main/model") == OWNER_MODEL
            && text(advisor, "/result/value/report/bytes") == BUDGET_ADVISOR_REPORT
            && queries.len() == 1
            && queries.iter().all(|t| {
                t.model.as_ref() == OWNER_MODEL.as_bytes() && contains(&t.system, BUDGET_A5_REPORT) && plan_fresh(t)
            })
            && rows.iter().filter(|r| r.pointer("/line/run_began").is_some()).count() == 1
            && rows.iter().filter(|r| text(r, "/line/run_ended/end") == "finished").count() == 1
            && rows.iter().filter(|r| r.pointer("/line/turn").is_some()).count() == 1,
        BUDGET_ONESHOT,
        "after a5 reports, one advisor must read its report/evidence on owner and end on its first turn and attempt",
    );
    step_check(
        misses,
        "6.5",
        budget_report(observed, BUDGET_ADVISOR, BUDGET_ADVISOR_REPORT_NUMBER, BUDGET_ADVISOR_REPORT)
            && plan_deliveries(observed, Agent::BudgetSubowner, BUDGET_ADVISOR_REPORT) == 1
            && report_at
                .zip(blocker_delivery(observed, Agent::BudgetSubowner, BUDGET_ADVISOR_REPORT))
                .is_some_and(|(a5, advisor)| a5 < advisor),
        BUDGET_RESULT,
        "A1 must hear the advisor's cause once, after a5's report",
    );
}

#[cfg(test)]
mod budget_tests {
    use super::batch_tests::{input, journal, told, tool};
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_llm_domain::api::{Part, Role};

    fn met() -> Observed {
        let mut observed = Observed {
            documents: BTreeMap::new(),
            told: Vec::new(),
            commands: vec![
                "sign-in", "chat", "wait", "wait", "wait", "inbox", "answer", "task", "task", "wait", "task", "task",
                "task", "task", "task", "task", "task", "inbox",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            kit: Kit { records: Vec::new(), main: BTreeMap::new(), pulls: Vec::new(), merged_green: false },
        };
        budget_documents(&mut observed);
        investigation_documents(&mut observed);
        budget_queries(&mut observed);
        for (latest, earlier, _) in [
            (Read::BudgetDirectorEnded, Read::BudgetDirectorFirst, BUDGET_DIRECTOR),
            (Read::BudgetOwnerEnded, Read::BudgetOwnerFirst, BUDGET_OWNER),
            (Read::BudgetSubownerEnded, Read::BudgetSubownerFirst, BUDGET_SUBOWNER),
        ] {
            let page = &mut observed.documents.get_mut(&latest).expect("latest")["activity"];
            let rows = page["rows"].as_array_mut().expect("rows");
            let older = rows.split_off(3);
            page["before"] = json!({"value":"older"});
            observed
                .documents
                .insert(earlier, json!({"rows":older,"cut":false,"before":null,"after":{"value":"newer"}}));
        }
        observed
    }

    fn budget_documents(observed: &mut Observed) {
        for (read, task, parent) in budget_tasks() {
            observed.documents.insert(read,json!({"task":{
                "chip":{"task":task,"phase":"ended"},"project":PROJECT,"requester":{"task":parent},"goal":BUDGET_OWNER,
                "authority":{"authority":{"deadline":if task == BUDGET_OWNER {A_DEADLINE} else {A1_DEADLINE},"delegation":{"depth":1}}},
                "result":{"ending":{"task":{"done":{}}}}
            },"activity":journal(task,&[
                json!({"run_began":{"attempt":1}}), json!({"turn":{"attempt":1}}),
                json!({"run_ended":{"attempt":1,"end":"finished","spent":1}})
            ])}));
        }
        let mut lines = BTreeMap::new();
        for (read, task, _) in budget_tasks() {
            lines.insert(read, (task, vec![json!({"run_began":{"attempt":1}}), json!({"turn":{"attempt":1}})]));
        }
        for (read, message, by, kind, words) in [
            (
                Read::BudgetOwnerEnded,
                BUDGET_QUESTION_NUMBER,
                json!({"task":BUDGET_SUBOWNER}),
                json!("question"),
                BUDGET_QUESTION,
            ),
            (
                Read::BudgetDirectorEnded,
                BUDGET_UP_QUESTION_NUMBER,
                json!({"task":BUDGET_OWNER}),
                json!("question"),
                BUDGET_UP_QUESTION,
            ),
            (
                Read::BudgetOwnerEnded,
                BUDGET_RELAY_NUMBER,
                json!({"task":BUDGET_DIRECTOR}),
                json!({"answer":{"question":BUDGET_UP_QUESTION_NUMBER}}),
                BUDGET_PERSON_ANSWER,
            ),
            (
                Read::BudgetSubownerEnded,
                BUDGET_ANSWER_NUMBER,
                json!({"task":BUDGET_OWNER}),
                json!({"answer":{"question":BUDGET_QUESTION_NUMBER}}),
                BUDGET_ANSWER,
            ),
        ] {
            lines
                .get_mut(&read)
                .expect("task")
                .1
                .push(json!({"words":{"message":message,"by":by,"kind":kind,"words":words}}));
        }
        lines.get_mut(&Read::BudgetDirectorEnded).expect("director").1.push(json!({"words":{"message":BUDGET_PERSON_ANSWER_NUMBER,"by":{"person":1},"kind":{"answer":{"question":BUDGET_UP_QUESTION_NUMBER}},"words":BUDGET_PERSON_ANSWER}}));
        lines.get_mut(&Read::BudgetSubownerEnded).expect("A1").1.push(json!({"words":{"message":BUDGET_FINDING_NUMBER,"by":{"task":BUDGET_A5},"kind":"words","words":BUDGET_FINDING}}));
        for (task, message, words) in [
            (BUDGET_A5, BUDGET_A5_REPORT_NUMBER, BUDGET_A5_REPORT),
            (BUDGET_ADVISOR, BUDGET_ADVISOR_REPORT_NUMBER, BUDGET_ADVISOR_REPORT),
        ] {
            lines.get_mut(&Read::BudgetSubownerEnded).expect("A1").1.push(
                json!({"heard":{"number":message,"from":{"task":task},"kind":{"result":"report"},"words":words}}),
            );
        }
        for (read, (task, mut rows)) in lines {
            rows.push(json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}));
            observed.documents.get_mut(&read).expect("doc")["activity"] = journal(task, &rows);
        }
        observed.documents.insert(Read::BudgetAsked,json!({"needs":{"cut":false,"rows":[{"task":{"live":{"task":BUDGET_DIRECTOR}},"project":PROJECT,"object":{"question":{"message":BUDGET_UP_QUESTION_NUMBER}},"reply":null}]}}));
        observed.documents.insert(Read::BudgetInboxAnswered, json!({"needs":{"cut":false,"rows":[]}}));
    }

    fn investigation_documents(observed: &mut Observed) {
        let before = vec![
            json!({"run_began":{"attempt":1}}),
            json!({"turn":{"attempt":1}}),
            json!({"run_ended":{"attempt":1,"end":"parked","spent":1}}),
        ];
        let mut high = before.clone();
        high.extend([
            json!({"heard":{"number":BUDGET_AMENDMENT_NUMBER,"from":{"task":BUDGET_SUBOWNER},"kind":{"amendment":{"revision":BUDGET_REVISION}},"words":BUDGET_REASON}}),
            json!({"decided":{"revision":BUDGET_REVISION}}),
            json!({"run_began":{"attempt":2,"why":{"message":{"message":BUDGET_AMENDMENT_NUMBER}}}}),
        ]);
        for (read, effort, revision, phase, rows) in [
            (Read::BudgetMedium, BUDGET_MEDIUM, BUDGET_REVISION - 1, "idle", &before),
            (Read::BudgetHigh, BUDGET_HIGH, BUDGET_REVISION, "running", &high),
        ] {
            observed.documents.insert(read,json!({"task":{"chip":{"task":BUDGET_A5,"phase":phase,"details":{"owner":{"phase":{"active":{"running":{"attempt":2}}},"details":{"observation":{"work":"writing"}}}}},"llm":{"main":{"model":DEFAULT_MODEL,"effort":effort}},"spec_revision":{"value":revision},"result":null},"activity":journal(BUDGET_A5,rows)}));
        }
        high.extend([
            json!({"turn":{"attempt":2,"read":BUDGET_AMENDMENT_NUMBER}}),
            json!({"run_ended":{"attempt":2,"end":"finished","spent":1}}),
        ]);
        let a5 = observed.documents.get_mut(&Read::BudgetA5Ended).expect("a5");
        a5["activity"] = journal(BUDGET_A5, &high);
        a5["task"]["llm"] = json!({"main":{"model":DEFAULT_MODEL,"effort":BUDGET_HIGH}});
        a5["task"]["spec_revision"] = json!({"value":BUDGET_REVISION});
        a5["task"]["result"]["value"] = json!({"report":{"bytes":BUDGET_A5_REPORT}});
        let advisor = observed.documents.get_mut(&Read::BudgetAdvisorEnded).expect("advisor");
        advisor["task"]["spec"] = json!({"specification":{"words":{"bytes":BUDGET_ADVISOR_SPEC}}});
        advisor["task"]["llm"] = json!({"main":{"model":OWNER_MODEL,"effort":OWNER_EFFORT}});
        advisor["task"]["result"]["value"] = json!({"report":{"bytes":BUDGET_ADVISOR_REPORT}});
    }

    fn budget_queries(observed: &mut Observed) {
        let exchange = |id, name, args: Value, output: String| tool(id, name, &args, &output).to_vec();
        observed.told = vec![
            told(
                Agent::BudgetSubowner,
                &exchange(
                    "q",
                    "message",
                    json!({"target":BUDGET_OWNER,"form":"question","words":BUDGET_QUESTION}),
                    format!("Message {BUDGET_QUESTION_NUMBER} sent"),
                ),
            ),
            told(Agent::BudgetOwner, &[input(BUDGET_QUESTION)]),
            told(
                Agent::BudgetOwner,
                &exchange(
                    "up",
                    "message",
                    json!({"target":BUDGET_DIRECTOR,"form":"question","words":BUDGET_UP_QUESTION}),
                    format!("Message {BUDGET_UP_QUESTION_NUMBER} sent"),
                ),
            ),
            told(Agent::BudgetDirector, &[input(BUDGET_UP_QUESTION)]),
            told(Agent::BudgetOwner, &[input(BUDGET_PERSON_ANSWER)]),
            told(
                Agent::BudgetOwner,
                &exchange(
                    "answer",
                    "message",
                    json!({"target":BUDGET_SUBOWNER,"form":"answer","question":BUDGET_QUESTION_NUMBER,"words":BUDGET_ANSWER}),
                    format!("Message {BUDGET_ANSWER_NUMBER} sent"),
                ),
            ),
            told(Agent::BudgetSubowner, &[input(BUDGET_ANSWER)]),
            told(
                Agent::BudgetSubowner,
                &exchange(
                    "a5",
                    "delegate",
                    json!({"batch":[{"llm":{"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_MEDIUM_NUMBER}}}]}),
                    format!("Delegated tasks: {BUDGET_A5}"),
                ),
            ),
            told(
                Agent::BudgetSubowner,
                &exchange(
                    "raise",
                    "amend",
                    json!({"target":BUDGET_A5,"revision":BUDGET_REVISION-1,"amendment":{"llm":{"main":{"model":DEFAULT_MODEL_NUMBER,"effort":BUDGET_HIGH_NUMBER}},"reason":BUDGET_REASON}}),
                    format!("Done: task {BUDGET_A5} is at spec revision {BUDGET_REVISION}"),
                ),
            ),
            told(Agent::BudgetInvestigator, &[input(BUDGET_REASON)]),
            told(Agent::BudgetSubowner, &[input(BUDGET_A5_REPORT)]),
            told(
                Agent::BudgetSubowner,
                &exchange(
                    "advisor",
                    "delegate",
                    json!({"batch":[{"spec":{"words":BUDGET_ADVISOR_SPEC},"llm":{"main":{"model":OWNER_MODEL_NUMBER,"effort":OWNER_EFFORT_NUMBER}}}]}),
                    format!("Delegated tasks: {BUDGET_ADVISOR}"),
                ),
            ),
            Told::new(BUDGET_ADVISOR_SPEC.as_bytes(), OWNER_MODEL.as_bytes(), None, &[input("Begin")])
                .expect("advisor"),
            told(Agent::BudgetSubowner, &[input(BUDGET_ADVISOR_REPORT)]),
        ];
        let mut finding = vec![input(BUDGET_FINDING)];
        finding.extend(observed.told[8].history.iter().cloned());
        observed.told[8].history = finding.into();
        let mut relay = vec![input(BUDGET_PERSON_ANSWER)];
        relay.extend(tool("relay","message",&json!({"target":BUDGET_OWNER,"form":"answer","question":BUDGET_UP_QUESTION_NUMBER,"words":BUDGET_PERSON_ANSWER}),&format!("Message {BUDGET_RELAY_NUMBER} sent")));
        observed.told.insert(4, told(Agent::BudgetDirector, &relay));
    }

    fn rejects(observed: &Observed, promise: Promise) {
        let misses = step_6_5(observed);
        assert!(
            misses.iter().any(|m| m.step == "6.5" && m.promise == promise),
            "expected {promise:?}; got {}",
            misses.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        );
    }
    fn changed_tool(observed: &mut Observed, at: usize, path: &str) {
        let Part::ToolCall { arguments, .. } = observed.told[at]
            .history
            .iter_mut()
            .flat_map(|m| &mut m.parts)
            .find(|p| matches!(p, Part::ToolCall { .. }))
            .expect("tool")
        else {
            unreachable!()
        };
        let mut args: Value = serde_json::from_slice(arguments).expect("args");
        *args.pointer_mut(path).expect("field") = Value::Null;
        *arguments = serde_json::to_vec(&args).expect("args").into();
    }

    #[test]
    fn budget_checkpoint_met() {
        assert_met(&step_6_5(&met()));
    }

    #[test]
    fn each_task_identity_parent_project_and_ending_is_required() {
        let passing = met();
        for (read, _, parent) in budget_tasks() {
            let mut paths =
                vec!["/task/chip/task", "/task/project", "/task/chip/phase", "/task/result/ending/task/done"];
            if parent != 0 {
                paths.push("/task/requester/task");
            }
            for path in paths {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("task").pointer_mut(path).expect("field") = Value::Null;
                rejects(&observed, BUDGET_CHECKPOINT);
            }
        }
        for (read, path) in [
            (Read::BudgetOwnerEnded, "/task/goal"),
            (Read::BudgetOwnerEnded, "/task/authority/authority/deadline"),
            (Read::BudgetSubownerEnded, "/task/authority/authority/deadline"),
            (Read::BudgetSubownerEnded, "/task/authority/authority/delegation/depth"),
        ] {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&read).expect("doc").pointer_mut(path).expect("field") = Value::Null;
            rejects(&observed, BUDGET_CHECKPOINT);
        }
    }

    #[test]
    fn every_chain_message_must_be_delivered_once_and_in_order() {
        let passing = met();
        for at in [1, 3, 4, 5, 7] {
            let mut missing = passing.clone();
            missing.told.remove(at);
            rejects(&missing, BUDGET_MESSAGES);
            let mut duplicated = passing.clone();
            let more = duplicated.told[at].history[0].clone();
            let mut history = duplicated.told[at].history.to_vec();
            history.push(more);
            duplicated.told[at].history = history.into();
            rejects(&duplicated, BUDGET_MESSAGES);
        }
        for (left, right) in [(1, 3), (3, 4), (4, 5), (5, 7)] {
            let mut observed = passing.clone();
            observed.told.swap(left, right);
            rejects(&observed, BUDGET_MESSAGES);
        }
        for (at, paths) in [
            (0, vec!["/target", "/form", "/words"]),
            (2, vec!["/target", "/form", "/words"]),
            (6, vec!["/target", "/form", "/question", "/words"]),
        ] {
            for path in paths {
                let mut observed = passing.clone();
                changed_tool(&mut observed, at, path);
                rejects(&observed, BUDGET_MESSAGES);
            }
            let mut observed = passing.clone();
            observed.told[at].history = observed.told[at].history[..1].into();
            rejects(&observed, BUDGET_MESSAGES);
        }
        for read in [Read::BudgetOwnerEnded, Read::BudgetDirectorEnded, Read::BudgetSubownerEnded] {
            let rows = budget_rows(&passing.clone(), read)
                .into_iter()
                .filter(|r| number(r, "/line/words/message").is_some_and(|message| message <= BUDGET_ANSWER_NUMBER))
                .cloned()
                .collect::<Vec<_>>();
            for row in rows {
                for field in ["message", "by", "kind", "words"] {
                    let mut observed = passing.clone();
                    for page in budget_pages(read) {
                        let doc = observed.documents.get_mut(&page).expect("doc");
                        let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
                        if let Some(changed) = page["rows"]
                            .as_array_mut()
                            .expect("rows")
                            .iter_mut()
                            .find(|r| r["event"]["owner"]["source"] == row)
                        {
                            changed["event"]["owner"]["source"]["line"]["words"][field] = Value::Null;
                        }
                    }
                    rejects(&observed, BUDGET_MESSAGES);
                }
            }
        }
    }

    #[test]
    fn inbox_question_must_survive_until_the_read_before_the_answer() {
        let passing = met();
        for path in ["/needs/rows/0/task/live/task", "/needs/rows/0/project", "/needs/rows/0/object/question/message"] {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&Read::BudgetAsked).expect("inbox").pointer_mut(path).expect("field") =
                Value::Null;
            rejects(&observed, BUDGET_INBOX);
        }
        for mutation in 0..7 {
            let mut observed = passing.clone();
            match mutation {
                0 => observed.documents.get_mut(&Read::BudgetAsked).expect("inbox")["needs"]["rows"] = json!([]),
                1 => {
                    observed.documents.get_mut(&Read::BudgetInboxAnswered).expect("inbox")["needs"]["rows"] =
                        json!([{}]);
                }
                2 => observed.commands.swap(5, 6),
                3 => observed.commands.push("answer".into()),
                4 => observed.documents.get_mut(&Read::BudgetAsked).expect("inbox")["needs"]["cut"] = json!(true),
                5 => {
                    observed.documents.get_mut(&Read::BudgetInboxAnswered).expect("inbox")["needs"]["cut"] =
                        json!(true);
                }
                6 => {
                    observed.documents.get_mut(&Read::BudgetAsked).expect("inbox")["needs"]["rows"][0]["reply"] =
                        json!({"answered":true});
                }
                _ => unreachable!(),
            }
            rejects(&observed, BUDGET_INBOX);
        }
    }

    #[test]
    fn a5_cannot_be_made_before_a1s_answer_or_on_another_choice() {
        let passing = met();
        let mut observed = passing.clone();
        observed.told.swap(7, 8);
        rejects(&observed, BUDGET_CHECKPOINT);
        for path in ["/batch/0/llm/main/model", "/batch/0/llm/main/effort"] {
            let mut observed = passing.clone();
            changed_tool(&mut observed, 8, path);
            rejects(&observed, BUDGET_CHECKPOINT);
        }
        let mut observed = passing.clone();
        observed.told.remove(8);
        rejects(&observed, BUDGET_CHECKPOINT);
    }

    #[test]
    fn medium_high_and_the_successful_amendment_revision_are_required() {
        let passing = met();
        for read in [Read::BudgetMedium, Read::BudgetHigh, Read::BudgetA5Ended] {
            for path in
                ["/task/chip/task", "/task/llm/main/model", "/task/llm/main/effort", "/task/spec_revision/value"]
            {
                let mut observed = passing.clone();
                *observed.documents.get_mut(&read).expect("a5").pointer_mut(path).expect("field") = Value::Null;
                rejects(&observed, BUDGET_EFFORT);
            }
        }
        for path in
            ["/target", "/revision", "/amendment/llm/main/model", "/amendment/llm/main/effort", "/amendment/reason"]
        {
            let mut observed = passing.clone();
            changed_tool(&mut observed, 9, path);
            rejects(&observed, BUDGET_EFFORT);
        }
        let mut observed = passing.clone();
        observed.told.remove(9);
        rejects(&observed, BUDGET_EFFORT);
    }

    #[test]
    fn the_next_run_must_follow_the_amendment_and_hear_the_reason_once() {
        let passing = met();
        for path in [
            "/task/chip/phase",
            "/activity/rows/1/event/owner/source/line/decided/revision",
            "/task/chip/details/owner/phase/active/running/attempt",
            "/task/chip/details/owner/details/observation/work",
            "/activity/rows/2/event/owner/source/line/heard/number",
            "/activity/rows/2/event/owner/source/line/heard/from/task",
            "/activity/rows/2/event/owner/source/line/heard/kind/amendment/revision",
            "/activity/rows/2/event/owner/source/line/heard/words",
        ] {
            let mut observed = passing.clone();
            *observed.documents.get_mut(&Read::BudgetHigh).expect("a5").pointer_mut(path).expect("field") = Value::Null;
            rejects(&observed, BUDGET_EFFORT);
        }
        let mut observed = passing.clone();
        observed.told.remove(10);
        rejects(&observed, BUDGET_EFFORT);
        let mut observed = passing.clone();
        observed.told[10].history = vec![input(BUDGET_REASON), input(BUDGET_REASON)].into();
        rejects(&observed, BUDGET_EFFORT);
        let mut observed = passing.clone();
        observed.documents.get_mut(&Read::BudgetHigh).expect("a5")["activity"]["rows"]
            .as_array_mut()
            .expect("rows")
            .swap(0, 1);
        rejects(&observed, BUDGET_EFFORT);
    }

    #[test]
    fn advisor_requires_the_ended_investigations_evidence_owner_model_and_one_attempt() {
        let passing = met();
        for path in ["/batch/0/spec/words", "/batch/0/llm/main/model", "/batch/0/llm/main/effort"] {
            let mut observed = passing.clone();
            changed_tool(&mut observed, 12, path);
            rejects(&observed, BUDGET_ONESHOT);
        }
        for path in ["/task/spec/specification/words/bytes", "/task/llm/main/model", "/task/result/value/report/bytes"]
        {
            let mut observed = passing.clone();
            *observed
                .documents
                .get_mut(&Read::BudgetAdvisorEnded)
                .expect("advisor")
                .pointer_mut(path)
                .expect("field") = Value::Null;
            rejects(&observed, BUDGET_ONESHOT);
        }
        for mutation in 0..6 {
            let mut observed = passing.clone();
            match mutation {
                0 => observed.told.swap(11, 12),
                1 => observed.told[13].model = DEFAULT_MODEL.as_bytes().into(),
                2 => observed.told[13].system = Agent::BudgetAdvisor.cue().into(),
                3 => {
                    let extra = observed.told[13].clone();
                    observed.told.push(extra);
                }
                4 => observed.told[13].history[0].role = Role::Assistant,
                5 => {
                    observed.documents.get_mut(&Read::BudgetAdvisorEnded).expect("advisor")["activity"] = journal(
                        BUDGET_ADVISOR,
                        &[
                            json!({"run_began":{"attempt":1}}),
                            json!({"turn":{"attempt":1}}),
                            json!({"run_ended":{"attempt":1,"end":"parked"}}),
                            json!({"run_began":{"attempt":2}}),
                            json!({"turn":{"attempt":2}}),
                            json!({"run_ended":{"attempt":2,"end":"finished"}}),
                        ],
                    );
                }
                _ => unreachable!(),
            }
            rejects(&observed, BUDGET_ONESHOT);
        }
    }

    #[test]
    fn advisor_report_must_be_delivered_once_after_the_investigations_report() {
        let passing = met();
        let mut observed = passing.clone();
        observed.told.remove(14);
        rejects(&observed, BUDGET_RESULT);
        let mut observed = passing.clone();
        observed.told[14].history = vec![input(BUDGET_ADVISOR_REPORT), input(BUDGET_ADVISOR_REPORT)].into();
        rejects(&observed, BUDGET_RESULT);
        let mut observed = passing.clone();
        observed.told.swap(11, 14);
        rejects(&observed, BUDGET_RESULT);
    }

    #[test]
    fn no_failed_unended_or_spent_refused_attempt_or_incomplete_journal_is_hidden() {
        let passing = met();
        for (read, task, _) in budget_tasks() {
            for mutation in 0..7 {
                let mut observed = passing.clone();
                let page = &mut observed.documents.get_mut(&read).expect("doc")["activity"];
                match mutation {
                    0 => {
                        page["rows"][0]["event"]["owner"]["source"]["line"]["run_ended"]["end"] =
                            json!({"failed":{"class":"permanent"}});
                    }
                    1 => page["rows"][0]["event"]["owner"]["source"]["line"]["run_ended"]["end"] = json!("refused"),
                    2 => {
                        page["rows"].as_array_mut().expect("rows").remove(0);
                    }
                    3 => page["cut"] = json!(true),
                    4 => page["after"] = json!(1),
                    5 => page["rows"][0]["event"]["owner"]["source"]["task"] = json!(task + 99),
                    6 => {
                        let duplicate = page["rows"][0].clone();
                        page["rows"].as_array_mut().expect("rows").push(duplicate);
                    }
                    _ => unreachable!(),
                }
                rejects(&observed, BUDGET_ATTEMPTS);
            }
        }
    }

    #[test]
    fn every_identified_read_is_required() {
        let passing = met();
        for (read, _) in crate::case::acts::READS_6_5 {
            let mut observed = passing.clone();
            observed.documents.remove(read);
            assert!(!step_6_5(&observed).is_empty(), "{read:?}");
        }
    }
    #[test]
    fn result_messages_require_their_number_sender_kind_and_words() {
        let passing = met();
        for (message, promise) in
            [(BUDGET_A5_REPORT_NUMBER, BUDGET_ONESHOT), (BUDGET_ADVISOR_REPORT_NUMBER, BUDGET_RESULT)]
        {
            for field in ["number", "from", "kind", "words"] {
                let mut observed = passing.clone();
                for read in budget_pages(Read::BudgetSubownerEnded) {
                    let doc = observed.documents.get_mut(&read).expect("page");
                    let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
                    for row in page["rows"].as_array_mut().expect("rows") {
                        let heard = &mut row["event"]["owner"]["source"]["line"]["heard"];
                        if heard["number"] == message {
                            heard[field] = Value::Null;
                        }
                    }
                }
                rejects(&observed, promise);
            }
        }
    }

    #[test]
    fn earlier_pages_cannot_hide_gaps_missing_starts_or_failed_attempts() {
        let passing = met();
        for read in [Read::BudgetDirectorFirst, Read::BudgetOwnerFirst, Read::BudgetSubownerFirst] {
            for mutation in 0..4 {
                let mut observed = passing.clone();
                let page = observed.documents.get_mut(&read).expect("page");
                match mutation {
                    0 => page["cut"] = json!(true),
                    1 => page["before"] = json!({"value":"another page"}),
                    2 => {
                        page["rows"].as_array_mut().expect("rows").pop();
                    }
                    3 => page["rows"][0]["event"]["owner"]["source"]["position"] = json!(99),
                    _ => unreachable!(),
                }
                rejects(&observed, BUDGET_ATTEMPTS);
            }
        }
    }

    #[test]
    fn only_unspent_uncommitted_rechecks_followed_by_a_finished_attempt_are_allowed() {
        let passing = met();
        for (read, task, _) in budget_tasks() {
            let mut accepted = passing.clone();
            let mut lines = vec![
                json!({"run_began":{"attempt":99}}),
                json!({"run_ended":{"attempt":99,"end":"refused","spent":0}}),
            ];
            lines.extend(budget_rows(&accepted, read).iter().rev().map(|r| r["line"].clone()));
            let mut page = journal(task, &lines);
            let pages = budget_pages(read);
            if pages.len() == 2 {
                let older = page["rows"].as_array_mut().expect("rows").split_off(3);
                page["before"] = json!({"value":"older"});
                accepted
                    .documents
                    .insert(pages[1], json!({"rows":older,"cut":false,"before":null,"after":{"value":"newer"}}));
            }
            accepted.documents.get_mut(&read).expect("doc")["activity"] = page;
            assert!(!step_6_5(&accepted).iter().any(|m| m.promise == BUDGET_ATTEMPTS));
            for mutation in 0..3 {
                let mut changed = accepted.clone();
                let last = *pages.last().expect("page");
                let doc = changed.documents.get_mut(&last).expect("doc");
                let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
                let rows = page["rows"].as_array_mut().expect("rows");
                let recheck = rows
                    .iter_mut()
                    .find(|r| r.pointer("/event/owner/source/line/run_ended/attempt") == Some(&json!(99)))
                    .expect("recheck");
                match mutation {
                    0 => recheck["event"]["owner"]["source"]["line"]["run_ended"]["spent"] = json!(1),
                    1 => {
                        recheck["event"]["owner"]["source"]["line"]["run_ended"]["end"] =
                            json!({"failed":{"class":"transient"}});
                    }
                    2 => recheck["event"]["owner"]["source"]["line"]["turn"] = json!({"attempt":99}),
                    _ => unreachable!(),
                }
                rejects(&changed, BUDGET_ATTEMPTS);
            }
        }
    }
    #[test]
    fn the_first_finding_requires_its_number_sender_words_and_unique_delivery() {
        let passing = met();
        for mutation in 0..5 {
            let mut observed = passing.clone();
            if mutation == 4 {
                observed.told[9].history = observed.told[9].history[1..].into();
            } else {
                for read in budget_pages(Read::BudgetSubownerEnded) {
                    let doc = observed.documents.get_mut(&read).expect("page");
                    let page = if doc.get("activity").is_some() { &mut doc["activity"] } else { doc };
                    for row in page["rows"].as_array_mut().expect("rows") {
                        let words = &mut row["event"]["owner"]["source"]["line"]["words"];
                        if words["message"] == BUDGET_FINDING_NUMBER {
                            words[["message", "by", "kind", "words"][mutation]] = Value::Null;
                        }
                    }
                }
            }
            rejects(&observed, BUDGET_EFFORT);
        }
    }
}

// Act 3, step 6.4 (domain/testing.md, 5.3)
const RED_ORDER: Promise = Promise::Add("nothing lands on a red main before its repair");
const RED_DEPLOYMENT: Promise = Promise::Must("one deployment-requested landing-branch repair");
const RED_LANDING: Promise = Promise::Must("a4 lands with green CI after the repair");
const RED_RESULT: Promise = Promise::Must("A1 hears a4's merged ending once");
const RED_INTERIM: Promise = Promise::Must("the director arms and removes its interim timer");
const RED_ATTEMPTS: Promise = Promise::Add("every attempt ended, none failed");

fn red_tasks() -> [(Read, u64, u64); 9] {
    use super::{
        RED_CHANGE, RED_CODER, RED_DIRECTOR, RED_OWNER, RED_REPAIR, RED_REPAIR_CODER, RED_REPAIR_REVIEW, RED_REVIEW,
        RED_UPDATED_REVIEW,
    };
    [
        (Read::RedDirectorEnded, RED_DIRECTOR, 0),
        (Read::RedOwnerEnded, RED_OWNER, RED_DIRECTOR),
        (Read::RedChangeEnded, RED_CHANGE, RED_OWNER),
        (Read::RedCoderEnded, RED_CODER, RED_CHANGE),
        (Read::RedReviewEnded, RED_REVIEW, RED_CHANGE),
        (Read::RedRepairEnded, RED_REPAIR, 0),
        (Read::RedRepairCoderEnded, RED_REPAIR_CODER, RED_REPAIR),
        (Read::RedRepairReviewEnded, RED_REPAIR_REVIEW, RED_REPAIR),
        (Read::RedUpdatedReviewEnded, RED_UPDATED_REVIEW, RED_CHANGE),
    ]
}

// The armed read already owns the journal's prefix. Union its overlap with
// the ended page instead of issuing another ctl activity-page read.
fn red_director_done(observed: &Observed) -> bool {
    use super::RED_DIRECTOR;
    let reads = [Read::RedDirectorEnded, Read::RedTimerArmed];
    let mut rows = BTreeMap::new();
    for read in reads {
        if pause_activity(observed, read)["cut"] != false {
            return false;
        }
        for row in check_activity(observed, read) {
            let Some(position) = number(row, "/position") else { return false };
            if number(row, "/task") != Some(RED_DIRECTOR) {
                return false;
            }
            if let Some(previous) = rows.insert(position, row)
                && previous != row
            {
                return false;
            }
        }
    }
    let mut open = None;
    let mut attempts = BTreeSet::new();
    for (at, (position, row)) in rows.iter().enumerate() {
        if *position != u64::try_from(at).expect("journal position") + 1 {
            return false;
        }
        if let Some(began) = row.pointer("/line/run_began") {
            if open.is_some() {
                return false;
            }
            open = number(began, "/attempt");
            if !open.is_some_and(|attempt| attempts.insert(attempt)) {
                return false;
            }
        }
        if let Some(end) = row.pointer("/line/run_ended") {
            let attempt = number(end, "/attempt");
            if open.is_none() || open.take() != attempt {
                return false;
            }
            let recheck = end["end"] == "refused"
                && number(end, "/spent") == Some(0)
                && !rows.values().any(|row| number(row, "/line/turn/attempt") == attempt)
                && rows.range((position + 1)..).any(|(_, row)| text(row, "/line/run_ended/end") == "finished");
            if !matches!(end["end"].as_str(), Some("parked" | "finished")) && !recheck {
                return false;
            }
        }
    }
    let doc = task_document(observed, Read::RedDirectorEnded);
    open.is_none()
        && !attempts.is_empty()
        && doc.pointer("/result/ending/task/done").is_some_and(Value::is_object)
        && pause_activity(observed, Read::RedTimerArmed)["before"] == Value::Null
        && pause_activity(observed, Read::RedDirectorEnded)["after"] == Value::Null
        && rows.last_key_value().is_some_and(|(last, _)| number(doc, "/activity") == Some(*last))
}

#[must_use]
pub fn step_6_4(observed: &Observed) -> Vec<Miss> {
    use super::{Agent, PROJECT, RED_CHANGE, RED_DIRECTOR, RED_OWNER, RED_REPAIR, RED_REVISION};
    let mut misses = Vec::new();
    for (read, task, parent) in red_tasks() {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "6.4",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && number(doc, "/spec_revision/value") == Some(RED_REVISION)
                && text(doc, "/chip/phase") == "ended"
                && (parent == 0 || number(doc, "/requester/task") == Some(parent)),
            RED_DEPLOYMENT,
            &format!("{read:?} identifies ended task {task} under {parent}"),
        );
        if task != RED_CHANGE && task != RED_REPAIR {
            step_check(
                &mut misses,
                "6.4",
                if task == RED_DIRECTOR { red_director_done(observed) } else { batch_done(observed, read, task) },
                RED_ATTEMPTS,
                &format!(
                    "task {task}'s complete journal ends every admitted attempt and permits only bounded placement rechecks"
                ),
            );
        }
    }
    red_landings(observed, &mut misses);
    red_repair(observed, &mut misses);
    red_timer(observed, &mut misses);
    let change = task_document(observed, Read::RedChangeEnded);
    let merge = text(change, "/result/value/change/words/bytes");
    let result = format!("Result from task {RED_CHANGE}:");
    step_check(
        &mut misses,
        "6.4",
        !merge.is_empty()
            && plan_deliveries(observed, Agent::RedOwner, &result) == 1
            && observed
                .told
                .iter()
                .filter(|t| t.agent == Agent::RedOwner)
                .any(|t| user_words(t).any(|words| contains(words, &result) && contains(words, merge)))
            && number(task_document(observed, Read::RedOwnerEnded), "/goal") == Some(RED_OWNER),
        RED_RESULT,
        "tracked A1 must hear a4's actual merge result exactly once, after that merge",
    );
    misses
}

fn red_landings(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{
        RED_A4, RED_A4_FILE, RED_A4_PULL, RED_BASE, RED_CHANGE, RED_DIRECTOR, RED_FILE, RED_FIRST_HEAD, RED_REPAIR,
        RED_REPAIR_PULL, RED_UPDATED_HEAD,
    };
    use skein_fake_forge_domain::{Observation, api};
    let records = &observed.kit.records;
    let pushes: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(at, record)| {
            let Observation::Moved { repository, branch, to, by, .. } = record else { return None };
            (repository.as_ref() == b"acme/app" && branch.as_ref() == b"main" && *by == crate::forge::OTHER)
                .then_some((at, *to))
        })
        .collect();
    let merges: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(at, record)| {
            let Observation::Merged { repository, base, number, head, commit, .. } = record else { return None };
            Some((at, repository.as_ref(), base.as_ref(), *number, *head, *commit))
        })
        .collect();
    let green = |at: usize, head: u64| {
        records[..at].iter().rev().find_map(|record| {
            let Observation::Reported { repository, commit, context, state, .. } = record else { return None };
            (repository.as_ref() == b"acme/app" && *commit == head && context.as_ref() == b"check").then_some(*state)
        }) == Some(api::Check::Passed)
    };
    let pull_for = |task: u64, root: u64| {
        observed.kit.pulls.iter().find(|pull| pull.branch == format!("temper/{root}/c{task}").as_bytes())
    };
    let repair = pull_for(RED_REPAIR, RED_REPAIR);
    let a4 = pull_for(RED_CHANGE, RED_DIRECTOR);
    let opened_before_push = a4.is_some_and(|pull| {
        pushes.first().is_some_and(|(pushed, _)| {
            records[..*pushed].iter().any(|record| {
                matches!(record,
            Observation::Opened { number, branches: Some(_), .. } if *number == pull.number)
            })
        })
    });
    step_check(
        misses,
        "6.4",
        pushes.len() == 1
            && merges.len() == 2
            && opened_before_push
            && pushes.first().zip(merges.first()).is_some_and(|((pushed, red), first)| {
                *pushed < first.0
                    && records[*pushed..first.0].iter().any(|record| {
                        matches!(record,
                Observation::Reported { commit, state: api::Check::Failed, .. } if commit == red)
                    })
            })
            && repair.is_some_and(|pull| {
                pull.number == RED_REPAIR_PULL
                    && merges.first().is_some_and(|m| m.3 == pull.number && pull.landed == Some(m.5))
            })
            && a4.is_some_and(|pull| {
                pull.number == RED_A4_PULL
                    && merges.get(1).is_some_and(|m| m.3 == pull.number && pull.landed == Some(m.5))
            }),
        RED_ORDER,
        "the person's failed main push must precede exactly two forge-ordered merges: repair, then a4",
    );
    step_check(
        misses,
        "6.4",
        merges.len() == 2
            && observed.kit.pulls.len() == 2
            && observed.kit.merged_green
            && a4.is_some_and(|pull| pull.opened == RED_FIRST_HEAD && pull.merged_head == Some(RED_UPDATED_HEAD))
            && merges.iter().all(|m| m.1 == b"acme/app" && m.2 == b"main" && green(m.0, m.4))
            && [(repair, Read::RedRepairEnded), (a4, Read::RedChangeEnded)].iter().all(|(pull, read)| {
                pull.is_some_and(|pull| {
                    pull.merged
                        && pull.landed.is_some_and(|commit| {
                            text(task_document(observed, *read), "/result/value/change/words/bytes")
                                == direction_commit(commit)
                        })
                })
            }),
        RED_LANDING,
        "each merge must use its green head and identify its task's result",
    );
    step_check(
        misses,
        "6.4",
        observed.kit.main.get(RED_FILE.as_bytes()).is_some_and(|bytes| bytes == RED_BASE.as_bytes())
            && observed.kit.main.get(RED_A4_FILE.as_bytes()).is_some_and(|bytes| bytes == RED_A4.as_bytes()),
        RED_LANDING,
        "main holds the repair and a4's story files",
    );
}

fn red_repair(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{Agent, PROJECT, RED_CHANGE, RED_REPAIR, RED_REPAIR_CODER, RED_REVIEW};
    let repair = task_document(observed, Read::RedRepairPending);
    let final_repair = task_document(observed, Read::RedRepairEnded);
    let agents = [Agent::RedDirector, Agent::RedOwner, Agent::RedCoder, Agent::RedRepairCoder, Agent::Review];
    let delegates: Vec<_> =
        agents.into_iter().flat_map(|agent| plan_tool_results(observed, agent, b"delegate")).collect();
    let paused = task_document(observed, Read::RedChangePaused);
    let opened = task_document(observed, Read::RedChangeOpen);
    let roots = document(observed, Read::RedRootsEnded);
    let pending = document(observed, Read::RedRootsPending);
    step_check(
        misses,
        "6.4",
        pending.pointer("/tasks/more") == Some(&Value::Bool(false))
            && pending.pointer("/tasks/rows").and_then(Value::as_array).is_some_and(|rows| {
                let repairs: Vec<_> =
                    rows.iter().filter(|row| row.pointer("/requester/deployment").is_some()).collect();
                repairs.len() == 1
                    && number(repairs[0], "/chip/task") == Some(RED_REPAIR)
                    && number(repairs[0], "/requester/deployment/project") == Some(PROJECT)
            }),
        RED_DEPLOYMENT,
        "one deployment repair among the pending roots",
    );
    step_check(
        misses,
        "6.4",
        roots.get("live").and_then(Value::as_array).is_some_and(Vec::is_empty)
            && roots.get("live_after") == Some(&Value::Null)
            && roots.pointer("/ended/rows/after") == Some(&Value::Null)
            && roots.pointer("/ended/rows/rows").and_then(Value::as_array).is_some_and(Vec::is_empty),
        RED_DEPLOYMENT,
        "no live or ended roots after",
    );
    step_check(
        misses,
        "6.4",
        [repair, final_repair].iter().all(|doc| {
            number(doc, "/chip/task") == Some(RED_REPAIR)
                && number(doc, "/project") == Some(PROJECT)
                && doc.pointer("/requester/task").is_none()
                && number(doc, "/requester/deployment/project") == Some(PROJECT)
                && text(doc, "/spec/specification/words/bytes") == "Repair landing branch CI"
        }),
        RED_DEPLOYMENT,
        "the repair's documents name the deployment and its words",
    );
    step_check(
        misses,
        "6.4",
        delegates.len() == 2
            && delegates.iter().all(|(_, args, output)| {
                !contains(output, &format!("Delegated tasks: {RED_REPAIR}"))
                    && args["batch"].as_array().is_some_and(|batch| {
                        batch.len() == 1 && batch[0]["spec"]["words"] != "Repair landing branch CI"
                    })
            }),
        RED_DEPLOYMENT,
        "two agent delegations, none making the repair",
    );
    step_check(
        misses,
        "6.4",
        batch_writing(observed, Read::RedRepairWriting, RED_REPAIR_CODER, Read::RedRepairCoderEnded),
        RED_DEPLOYMENT,
        "the repair coder writing",
    );
    step_check(
        misses,
        "6.4",
        batch_writing(observed, Read::RedReviewWriting, RED_REVIEW, Read::RedReviewEnded),
        RED_DEPLOYMENT,
        "the review writing",
    );
    for (doc, what) in
        [(opened, "a4 idle with no result at its open read"), (paused, "a4 idle with no result at its paused read")]
    {
        step_check(
            misses,
            "6.4",
            number(doc, "/chip/task") == Some(RED_CHANGE)
                && number(doc, "/project") == Some(PROJECT)
                && text(doc, "/chip/phase") == "idle"
                && doc.get("result") == Some(&Value::Null),
            RED_DEPLOYMENT,
            what,
        );
    }
}

// A subscribe and wait can share a completion. Its acknowledgement then
// first appears in the query carrying A1's report: order the actual messages
// within that query, retaining the call identity and the successful reply.
fn red_timer_ack_before_report(told: &Told) -> bool {
    use skein_fake_llm_domain::api::{Part, Role};
    let mut subscriptions = BTreeSet::new();
    let mut acknowledged = false;
    for message in &told.history {
        for part in &message.parts {
            match part {
                Part::ToolCall { id, name, .. } if message.role == Role::Assistant && name.as_ref() == b"subscribe" => {
                    subscriptions.insert(id.as_ref());
                }
                Part::Text { text } if message.role == Role::User && contains(text, super::RED_OWNER_REPORT) => {
                    return acknowledged;
                }
                Part::ToolOutput { id, output, .. } if message.role == Role::User => {
                    if contains(output, super::RED_OWNER_REPORT) {
                        return acknowledged;
                    }
                    if subscriptions.contains(id.as_ref())
                        && contains(output, &format!("Subscription {} added", super::RED_TIMER))
                    {
                        acknowledged = true;
                    }
                }
                Part::Opaque { .. } | Part::Text { .. } | Part::ToolCall { .. } | Part::ToolOutput { .. } => {}
            }
        }
    }
    false
}

fn red_timer(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::{Agent, RED_DIRECTOR, RED_MESSAGE, RED_OWNER_REPORT, RED_PERSON_MESSAGE, RED_TIMER, RED_TIMER_AT};
    let armed = task_document(observed, Read::RedTimerArmed);
    let ended = task_document(observed, Read::RedDirectorEnded);
    let added = plan_tool_results(observed, Agent::RedDirector, b"subscribe");
    let removed = plan_tool_results(observed, Agent::RedDirector, b"unsubscribe");
    let subscription = added
        .first()
        .and_then(|(_, _, output)| std::str::from_utf8(output).ok())
        .and_then(|output| output.strip_prefix("Subscription "))
        .and_then(|tail| tail.split_whitespace().next())
        .and_then(|number| number.parse::<u64>().ok());
    let request_at = observed
        .told
        .iter()
        .position(|t| t.agent == Agent::RedDirector && user_words(t).any(|words| contains(words, RED_MESSAGE)));
    let report_at = observed
        .told
        .iter()
        .position(|t| t.agent == Agent::RedDirector && user_words(t).any(|words| contains(words, RED_OWNER_REPORT)));
    step_check(
        misses,
        "6.4",
        check_activity(observed, Read::RedTimerArmed)
            .iter()
            .filter(|row| {
                number(row, "/line/words/message") == Some(RED_PERSON_MESSAGE)
                    && text(row, "/line/words/words") == RED_MESSAGE
            })
            .count()
            == 1
            && plan_deliveries(observed, Agent::RedDirector, RED_MESSAGE) == 1
            && plan_deliveries(observed, Agent::RedDirector, RED_OWNER_REPORT) == 1
            && subscription == Some(RED_TIMER)
            && added.len() == 1
            && removed.len() == 1
            && added.first().zip(removed.first()).is_some_and(|((at, args, _), (later, remove, output))| {
                request_at.is_some_and(|request| request <= *at)
                    && report_at.is_some_and(|report| {
                        report <= *later
                            && (*at < report || (*at == report && red_timer_ack_before_report(&observed.told[report])))
                    })
                    && at < later
                    && args["kind"] == "timer"
                    && args["at"] == RED_TIMER_AT
                    && remove["subscription"].as_u64() == subscription
                    && contains(output, "Subscription removed")
            })
            && number(armed, "/chip/task") == Some(RED_DIRECTOR)
            && armed.pointer("/subscriptions/entries").and_then(Value::as_array).is_some_and(|entries| {
                entries.len() == 1
                    && number(&entries[0], "/number") == subscription
                    && number(&entries[0], "/kind/timer/at") == Some(RED_TIMER_AT)
                    && entries[0].pointer("/kind/timer/period") == Some(&Value::Null)
            })
            && ended.pointer("/subscriptions/entries").and_then(Value::as_array).is_some_and(Vec::is_empty),
        RED_INTERIM,
        "the subscription returned by subscribe must be armed during the pause and removed after A1 reports",
    );
}

#[cfg(test)]
mod red_tests {
    use super::*;
    use crate::case::*;
    use serde_json::json;
    use skein_fake_forge_domain::{Observation, api};
    use skein_fake_llm_domain::api::{Message, Part, Role};

    fn input(words: &str) -> Message {
        Message { role: Role::User, parts: Box::new([Part::Text { text: words.as_bytes().into() }]) }
    }
    fn tool(id: &str, name: &str, args: &Value, output: &str) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                parts: Box::new([Part::ToolCall {
                    id: id.as_bytes().into(),
                    name: name.as_bytes().into(),
                    arguments: serde_json::to_vec(args).expect("args").into(),
                }]),
            },
            Message {
                role: Role::User,
                parts: Box::new([Part::ToolOutput {
                    id: id.as_bytes().into(),
                    output: output.as_bytes().into(),
                    is_error: false,
                }]),
            },
        ]
    }
    fn told(agent: Agent, history: &[Message]) -> Told {
        Told::new(agent.cue(), b"fake", None, history).expect("cue")
    }
    fn journal(task: u64, lines: &[Value]) -> Value {
        let rows: Vec<_> = lines
            .iter()
            .enumerate()
            .rev()
            .map(|(at, line)| json!({"event":{"owner":{"source":{"task":task,"position":at+1,"line":line}}}}))
            .collect();
        json!({"rows":rows,"cut":false,"before":null,"after":null})
    }
    fn met_documents() -> BTreeMap<Read, Value> {
        let mut documents = BTreeMap::new();
        for (read, task, parent) in red_tasks() {
            documents.insert(read, json!({"task":{
                "chip":{"task":task,"phase":"ended"},"project":PROJECT,"spec_revision":{"value":RED_REVISION},
                "requester":if task == RED_REPAIR {json!({"deployment":{"project":PROJECT}})} else {json!({"task":parent})},
                "goal":RED_OWNER,"spec":{"specification":{"words":{"bytes":"Repair landing branch CI"}}},
                "subscriptions":{"entries":[]},
                "result":{"ending":{"task":{"done":{}}},"value":{"change":{"words":{"bytes":direction_commit(if task == RED_REPAIR {5} else {7})}}}}
            },"activity":journal(task, &[
                json!({"run_began":{"attempt":1}}),json!({"run_ended":{"attempt":1,"end":"finished","spent":1}})
            ])}));
        }
        for (read, task) in [(Read::RedReviewWriting, RED_REVIEW), (Read::RedRepairWriting, RED_REPAIR_CODER)] {
            documents.insert(
                read,
                json!({"task":{"chip":{"task":task,"phase":"running","details":{"owner":{
                    "phase":{"active":{"running":{"attempt":1}}},"details":{"observation":{"work":"writing"}}
                }}}}}),
            );
        }
        for read in [Read::RedChangeOpen, Read::RedChangePaused] {
            documents.insert(
                read,
                json!({"task":{"chip":{"task":RED_CHANGE,"phase":"idle"},"project":PROJECT,"result":null}}),
            );
        }
        documents.insert(Read::RedRepairPending, documents[&Read::RedRepairEnded].clone());
        documents.insert(
            Read::RedTimerArmed,
            json!({"task":{"chip":{"task":RED_DIRECTOR},"subscriptions":{"entries":[
                {"number":RED_TIMER,"kind":{"timer":{"at":RED_TIMER_AT,"period":null}}}
            ]}}}),
        );
        documents.insert(
            Read::RedRootsEnded,
            json!({"live":[],"live_after":null,"ended":{"rows":{
                "after":null,"rows":[]
            }}}),
        );
        let director_lines = [
            json!({"run_began":{"attempt":1}}),
            json!({"words":{"message":RED_PERSON_MESSAGE,"words":RED_MESSAGE}}),
            json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}),
        ];
        documents.get_mut(&Read::RedTimerArmed).expect("timer")["activity"] = journal(RED_DIRECTOR, &director_lines);
        documents.get_mut(&Read::RedDirectorEnded).expect("director")["activity"] =
            journal(RED_DIRECTOR, &director_lines);
        documents.get_mut(&Read::RedDirectorEnded).expect("director")["task"]["activity"] = json!(3);
        documents.insert(
            Read::RedRootsPending,
            json!({"tasks":{"more":false,"rows":[{
                "chip":{"task":RED_REPAIR},"requester":{"deployment":{"project":PROJECT}}
            }]}}),
        );
        documents
    }
    fn met_kit() -> Kit {
        let repo: Box<[u8]> = b"acme/app".as_slice().into();
        let mut records = vec![
            Observation::Moved {
                repository: repo.clone(),
                branch: b"main".as_slice().into(),
                from: Some(1),
                to: 3,
                by: crate::forge::OTHER,
            },
            Observation::Reported {
                repository: repo.clone(),
                commit: 3,
                context: b"check".as_slice().into(),
                state: api::Check::Failed,
                by: 1,
            },
            Observation::Reported {
                repository: repo.clone(),
                commit: 4,
                context: b"check".as_slice().into(),
                state: api::Check::Passed,
                by: 1,
            },
            Observation::Merged {
                repository: repo.clone(),
                base: b"main".as_slice().into(),
                number: RED_REPAIR_PULL,
                head: 4,
                commit: 5,
                by: 1,
            },
            Observation::Reported {
                repository: repo.clone(),
                commit: 6,
                context: b"check".as_slice().into(),
                state: api::Check::Passed,
                by: 1,
            },
            Observation::Merged {
                repository: repo,
                base: b"main".as_slice().into(),
                number: RED_A4_PULL,
                head: 6,
                commit: 7,
                by: 1,
            },
        ];
        records.insert(
            0,
            Observation::Opened {
                repository: b"acme/app".as_slice().into(),
                number: RED_A4_PULL,
                kind: api::Kind::Pull,
                title: Box::new([]),
                body: Box::new([]),
                labels: Box::new([]),
                branches: Some(skein_fake_forge_domain::Branches {
                    head: format!("temper/{RED_DIRECTOR}/c{RED_CHANGE}").into_bytes().into(),
                    base: b"main".as_slice().into(),
                    commit: 2,
                }),
                by: 1,
            },
        );
        let pulls = [(RED_CHANGE, RED_DIRECTOR, RED_A4_PULL, 6, 7), (RED_REPAIR, RED_REPAIR, RED_REPAIR_PULL, 4, 5)]
            .into_iter()
            .map(|(task, root, number, head, merge)| Pull {
                number,
                branch: format!("temper/{root}/c{task}").into_bytes(),
                opened: if task == RED_CHANGE { RED_FIRST_HEAD } else { head },
                pushed: Some(head),
                landed: Some(merge),
                merged_head: Some(head),
                merged: true,
                closed: false,
                deleted: None,
            })
            .collect();
        Kit {
            records,
            pulls,
            merged_green: true,
            main: BTreeMap::from([
                (RED_FILE.as_bytes().to_vec(), RED_BASE.as_bytes().to_vec()),
                (RED_A4_FILE.as_bytes().to_vec(), RED_A4.as_bytes().to_vec()),
            ]),
        }
    }
    fn met_queries() -> Vec<Told> {
        let mut director = Vec::new();
        director.extend(tool(
            "owner",
            "delegate",
            &json!({"batch":[{"spec":{"words":RED_OWNER_SPEC}}]}),
            &format!("Delegated tasks: {RED_OWNER}"),
        ));
        let mut queries = vec![told(Agent::RedDirector, &director)];
        director.push(input(RED_MESSAGE));
        director.extend(tool(
            "timer",
            "subscribe",
            &json!({"kind":"timer","at":RED_TIMER_AT}),
            &format!("Subscription {RED_TIMER} added"),
        ));
        queries.push(told(Agent::RedDirector, &director));
        director.push(input(RED_OWNER_REPORT));
        director.extend(tool("remove", "unsubscribe", &json!({"subscription":RED_TIMER}), "Subscription removed"));
        queries.push(told(Agent::RedDirector, &director));
        let mut owner = Vec::from(tool(
            "a4",
            "delegate",
            &json!({"batch":[{"spec":{"words":RED_CHANGE_SPEC}}]}),
            &format!("Delegated tasks: {RED_CHANGE}"),
        ));
        queries.push(told(Agent::RedOwner, &owner));
        owner.push(input(&format!("Result from task {RED_CHANGE}:\n{}", direction_commit(7))));
        queries.push(told(Agent::RedOwner, &owner));
        queries
    }
    fn met() -> Observed {
        Observed { documents: met_documents(), told: met_queries(), commands: Vec::new(), kit: met_kit() }
    }
    fn rejects(observed: &Observed, promise: Promise) {
        assert!(
            step_6_4(observed).iter().any(|miss| miss.step == "6.4" && miss.promise == promise),
            "mutation must fail {promise:?}"
        );
    }
    #[test]
    fn red_observation_meets_the_step() {
        assert_met(&step_6_4(&met()));
    }
    #[test]
    fn missing_reversed_extra_or_premature_merges_fail() {
        for mutation in 0..6 {
            let mut bad = met();
            match mutation {
                0 => {
                    bad.kit.records.remove(0);
                }
                1 => {
                    bad.kit.records.swap(4, 6);
                }
                2 => {
                    bad.kit.records.insert(3, bad.kit.records[6].clone());
                }
                3 => {
                    bad.kit.records.swap(1, 4);
                }
                4 => {
                    bad.kit.records.remove(2);
                }
                _ => {
                    bad.kit.pulls[1].landed = Some(99);
                }
            }
            rejects(&bad, RED_ORDER);
        }
    }
    #[test]
    fn each_head_must_have_passed_ci_before_its_own_merge() {
        for at in [3, 5] {
            let mut bad = met();
            if let Observation::Reported { state, .. } = &mut bad.kit.records[at] {
                *state = api::Check::Failed;
            }
            rejects(&bad, RED_LANDING);
            let mut bad = met();
            bad.kit.records.swap(at, at + 1);
            rejects(&bad, RED_LANDING);
        }
        for read in [Read::RedChangeEnded, Read::RedRepairEnded] {
            let mut bad = met();
            bad.documents.get_mut(&read).expect("task")["task"]["result"]["value"]["change"]["words"]["bytes"] =
                json!("wrong commit");
            rejects(&bad, RED_LANDING);
        }
        let mut bad = met();
        bad.kit.main.clear();
        rejects(&bad, RED_LANDING);
    }
    #[test]
    fn only_a_deployment_repair_with_no_agent_delegation_is_accepted() {
        for read in [Read::RedRepairPending, Read::RedRepairEnded] {
            for requester in [json!({"task":RED_OWNER}), json!({"deployment":{"project":99}}), Value::Null] {
                let mut bad = met();
                bad.documents.get_mut(&read).expect("task")["task"]["requester"] = requester;
                rejects(&bad, RED_DEPLOYMENT);
            }
        }
        let mut bad = met();
        bad.told.push(told(
            Agent::RedOwner,
            &tool(
                "repair",
                "delegate",
                &json!({"batch":[{"spec":{"words":"Repair landing branch CI"}}]}),
                &format!("Delegated tasks: {RED_REPAIR}"),
            ),
        ));
        rejects(&bad, RED_DEPLOYMENT);
        let mut bad = met();
        bad.documents.get_mut(&Read::RedChangePaused).expect("a4")["task"]["chip"]["phase"] = json!("ended");
        rejects(&bad, RED_DEPLOYMENT);
    }
    #[test]
    fn another_deployment_repair_or_a_partial_task_list_fails() {
        let mut bad = met();
        let extra = bad.documents[&Read::RedRootsPending]["tasks"]["rows"][0].clone();
        bad.documents.get_mut(&Read::RedRootsPending).expect("pending")["tasks"]["rows"]
            .as_array_mut()
            .expect("live")
            .push(extra);
        rejects(&bad, RED_DEPLOYMENT);
        let mut bad = met();
        bad.documents.get_mut(&Read::RedRootsPending).expect("pending")["tasks"]["more"] = json!(true);
        rejects(&bad, RED_DEPLOYMENT);
    }
    #[test]
    fn a1_must_hear_the_actual_landed_result_once() {
        let mut bad = met();
        bad.told.pop();
        rejects(&bad, RED_RESULT);
        let mut bad = met();
        bad.told
            .push(told(Agent::RedOwner, &[input(&format!("Result from task {RED_CHANGE}:\n{}", direction_commit(7)))]));
        rejects(&bad, RED_RESULT);
        let mut bad = met();
        bad.told.last_mut().expect("result").history.last_mut().expect("message").parts =
            input("Result from task 3: not a merge").parts;
        rejects(&bad, RED_RESULT);
    }
    #[test]
    fn a_batched_timer_reply_must_precede_the_report_within_the_same_query() {
        let mut observed = met();
        let mut batched = observed.told[2].clone();
        // The subscription reply and A1's report are now in the same query;
        // the next query still supplies unsubscribe's successful result.
        batched.history = batched.history[..6].into();
        observed.told[1] = batched;
        assert_met(&step_6_4(&observed));
        for told in observed.told.iter_mut().filter(|told| told.agent == Agent::RedDirector) {
            if told.history.len() >= 6 {
                told.history.swap(4, 5);
            }
        }
        rejects(&observed, RED_INTERIM);
    }

    #[test]
    fn timer_requires_its_returned_number_armed_read_and_removal() {
        for path in [
            "/task/chip/task",
            "/task/subscriptions/entries/0/number",
            "/task/subscriptions/entries/0/kind/timer/at",
            "/task/subscriptions/entries/0/kind/timer/period",
        ] {
            let mut bad = met();
            *bad.documents.get_mut(&Read::RedTimerArmed).expect("timer").pointer_mut(path).expect("field") = json!(99);
            rejects(&bad, RED_INTERIM);
        }
        let mut bad = met();
        bad.documents.get_mut(&Read::RedDirectorEnded).expect("director")["task"]["subscriptions"] =
            bad.documents[&Read::RedTimerArmed]["task"]["subscriptions"].clone();
        rejects(&bad, RED_INTERIM);
        let mut bad = met();
        bad.told.remove(2);
        rejects(&bad, RED_INTERIM);
    }
    #[test]
    fn each_agents_attempts_must_be_complete_and_without_failure() {
        for (read, task, _) in red_tasks().into_iter().filter(|(_, task, _)| *task != RED_CHANGE && *task != RED_REPAIR)
        {
            for end in [json!({"failed":{}}), json!("refused"), Value::Null] {
                let mut bad = met();
                bad.documents.get_mut(&read).expect("task")["activity"] = journal(
                    task,
                    &[json!({"run_began":{"attempt":1}}), json!({"run_ended":{"attempt":1,"end":end,"spent":1}})],
                );
                rejects(&bad, RED_ATTEMPTS);
            }
            let mut bad = met();
            bad.documents.get_mut(&read).expect("task")["activity"]["cut"] = json!(true);
            rejects(&bad, RED_ATTEMPTS);
            let mut bad = met();
            bad.documents.get_mut(&read).expect("task")["activity"]["rows"].as_array_mut().expect("rows").pop();
            if task == RED_DIRECTOR {
                bad.documents.get_mut(&Read::RedTimerArmed).expect("timer")["activity"]["rows"]
                    .as_array_mut()
                    .expect("rows")
                    .pop();
            }
            rejects(&bad, RED_ATTEMPTS);
        }
    }
    #[test]
    fn each_task_identity_and_each_held_checkpoint_is_required() {
        for (read, _, parent) in red_tasks() {
            for path in [
                "/task/chip/task",
                "/task/project",
                "/task/spec_revision/value",
                "/task/chip/phase",
                "/task/requester/task",
            ] {
                if parent == 0 && path == "/task/requester/task" {
                    continue;
                }
                let mut bad = met();
                *bad.documents.get_mut(&read).expect("task").pointer_mut(path).expect("identity") = Value::Null;
                rejects(&bad, RED_DEPLOYMENT);
            }
        }
        for read in [Read::RedReviewWriting, Read::RedRepairWriting] {
            for path in [
                "/task/chip/task",
                "/task/chip/phase",
                "/task/chip/details/owner/details/observation/work",
                "/task/chip/details/owner/phase/active/running/attempt",
            ] {
                let mut bad = met();
                *bad.documents.get_mut(&read).expect("writing").pointer_mut(path).expect("field") = Value::Null;
                rejects(&bad, RED_DEPLOYMENT);
            }
        }
        for path in ["/live_after", "/ended/rows/after"] {
            let mut bad = met();
            *bad.documents.get_mut(&Read::RedRootsEnded).expect("roots").pointer_mut(path).expect("field") = json!(99);
            rejects(&bad, RED_DEPLOYMENT);
        }
    }
    #[test]
    fn only_unspent_uncommitted_rechecks_followed_by_a_finished_attempt_are_allowed() {
        for (read, task, _) in red_tasks().into_iter().filter(|(_, task, _)| *task != RED_CHANGE && *task != RED_REPAIR)
        {
            let mut lines = vec![
                json!({"run_began":{"attempt":8}}),
                json!({"run_ended":{"attempt":8,"end":"refused","spent":0}}),
                json!({"run_began":{"attempt":1}}),
                json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}),
            ];
            if task == RED_DIRECTOR {
                lines.push(json!({"words":{"message":RED_PERSON_MESSAGE,"words":RED_MESSAGE}}));
            }
            let mut accepted = met();
            accepted.documents.get_mut(&read).expect("task")["activity"] = journal(task, &lines);
            if task == RED_DIRECTOR {
                accepted.documents.get_mut(&read).expect("director")["task"]["activity"] = json!(5);
            }
            if task == RED_DIRECTOR {
                accepted.documents.get_mut(&Read::RedTimerArmed).expect("timer")["activity"] = journal(task, &lines);
            }
            assert_met(&step_6_4(&accepted));
            for mutation in 0..3 {
                let mut bad = accepted.clone();
                let mut changed = lines.clone();
                match mutation {
                    0 => changed[1]["run_ended"]["spent"] = json!(1),
                    1 => changed.truncate(2),
                    _ => changed.insert(1, json!({"turn":{"attempt":8,"turn":1}})),
                }
                bad.documents.get_mut(&read).expect("task")["activity"] = journal(task, &changed);
                if task == RED_DIRECTOR {
                    bad.documents.get_mut(&Read::RedTimerArmed).expect("timer")["activity"] = journal(task, &changed);
                }
                rejects(&bad, RED_ATTEMPTS);
            }
        }
    }
}

// Act 3, step 6.7 (domain/testing.md, 5.3)
const CAP_OPENING: Promise = Promise::Must("a5 is admitted and A1's timer committed before M1's deadline");
const CAP_ADMISSION: Promise = Promise::Must("the timer's wake holds A1 at admission without a committed turn");
const CAP_ESCALATION: Promise = Promise::Must("A1's hold waits at A, then passes up to the director, told once each");
const CAP_AMENDMENT: Promise = Promise::Must("the director amends A1's deadline and instructions at its next revision");
const CAP_RELEASE: Promise = Promise::Must("the amendment precedes release and A1's next run and report");
const CAP_RUNNING: Promise = Promise::Must("a5 writes across the cap in exactly one admitted attempt");
const CAP_INBOX: Promise = Promise::Add("the director decides without asking the person");
const CAP_ATTEMPTS: Promise = Promise::Add("every admitted attempt ends, none failed, with only unspent rechecks");
const CAP_CHECKPOINT: Promise = Promise::Step("the fresh deployment's cast reports the milestone split");

fn cap_tasks() -> [(Read, u64, u64, &'static str); 4] {
    use super::*;
    [
        (Read::CapDirectorEnded, CAP_DIRECTOR, 0, CAP_DIRECTOR_REPORT),
        (Read::CapOwnerEnded, CAP_OWNER, CAP_DIRECTOR, CAP_OWNER_REPORT),
        (Read::CapSubownerEnded, CAP_SUBOWNER, CAP_OWNER, CAP_SUBOWNER_REPORT),
        (Read::CapA5Ended, CAP_A5, CAP_SUBOWNER, CAP_A5_REPORT),
    ]
}
fn cap_position(row: &Value) -> Option<u64> {
    number(row, "/position")
}
fn cap_before(left: Option<u64>, right: Option<u64>) -> bool {
    left.zip(right).is_some_and(|(left, right)| left < right)
}
fn cap_timer(observed: &Observed) -> Option<&Value> {
    check_activity(observed, Read::CapHeld).into_iter().find(|r| r.pointer("/line/heard/kind/timer").is_some())
}
fn cap_parked_turn(observed: &Observed) -> Option<&Value> {
    let rows = check_activity(observed, Read::CapHeld);
    let parked = rows.iter().find(|r| text(r, "/line/run_ended/end") == "parked")?;
    let attempt = number(parked, "/line/run_ended/attempt")?;
    rows.into_iter().find(|r| number(r, "/line/turn/attempt") == Some(attempt))
}
fn cap_pages(read: Read) -> Vec<Read> {
    match read {
        Read::CapSubownerEnded => vec![read, Read::CapSubownerFirst],
        Read::CapDirectorEnded => vec![read, Read::CapDirectorFirst],
        _ => vec![read],
    }
}
/// Script construction through both a5's admission and A1's committed timer.
/// Wall arithmetic belongs here because the deadline itself is an absolute wall.
#[must_use]
pub fn cap_opening_nanos(observed: &Observed) -> Option<u64> {
    let deadline = number(task_document(observed, Read::CapHeld), "/authority/authority/deadline")?;
    let built = deadline.checked_sub(super::CAP_SPAN)?;
    let admitted = check_activity(observed, Read::CapA5Writing)
        .into_iter()
        .find(|r| r.pointer("/line/run_began").is_some())
        .and_then(|r| number(r, "/at"))?;
    let timer = cap_parked_turn(observed).and_then(|r| number(r, "/at"))?;
    admitted.max(timer).checked_sub(built)
}

#[must_use]
pub fn step_6_7(observed: &Observed) -> Vec<Miss> {
    use super::*;
    let mut misses = Vec::new();
    for (read, task, parent, report) in cap_tasks() {
        let doc = task_document(observed, read);
        step_check(
            &mut misses,
            "6.7",
            number(doc, "/chip/task") == Some(task)
                && number(doc, "/project") == Some(PROJECT)
                && (parent == 0 || number(doc, "/requester/task") == Some(parent))
                && text(doc, "/chip/phase") == "ended"
                && doc.pointer("/result/ending/task/done").is_some_and(Value::is_object)
                && text(doc, "/result/value/report/bytes") == report,
            CAP_CHECKPOINT,
            &format!("{read:?} must identify done task {task} under {parent} and its report"),
        );
        step_check(
            &mut misses,
            "6.7",
            batch_journal_pages(observed, &cap_pages(read), task),
            CAP_ATTEMPTS,
            &format!(
                "task {task}'s complete journal must end every attempt, with only zero-spend uncommitted rechecks: {}",
                document(observed, read)
            ),
        );
    }
    cap_opening(observed, &mut misses);
    cap_admission(observed, &mut misses);
    cap_escalation(observed, &mut misses);
    cap_amendment(observed, &mut misses);
    cap_release(observed, &mut misses);
    cap_running(observed, &mut misses);
    for (read, after) in
        [(Read::CapInboxHeld, Read::CapInboxHeldAfter), (Read::CapInboxEnded, Read::CapInboxEndedAfter)]
    {
        let inbox = document(observed, read);
        let continuation = document(observed, after);
        step_check(
            &mut misses,
            "6.7",
            inbox["needs"]["cut"] == false
                && inbox.pointer("/needs/rows").and_then(Value::as_array).is_some_and(Vec::is_empty)
                && continuation["needs"]["cut"] == false
                && continuation.pointer("/needs/after").is_some_and(Value::is_null)
                && continuation.pointer("/needs/rows").and_then(Value::as_array).is_some_and(Vec::is_empty),
            CAP_INBOX,
            &format!("{read:?} and {after:?} must show the person's complete empty inbox: {inbox}; {continuation}"),
        );
    }
    misses
}

fn cap_opening(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let held = task_document(observed, Read::CapHeld);
    let parked = task_document(observed, Read::CapParked);
    let deadline = number(held, "/authority/authority/deadline");
    let a5 = task_document(observed, Read::CapA5Writing);
    let owner = task_document(observed, Read::CapOwnerEnded);
    let delegates = plan_tool_results(observed, Agent::CapSubowner, b"delegate");
    let timers = plan_tool_results(observed, Agent::CapSubowner, b"subscribe");
    let elapsed = cap_opening_nanos(observed);
    let armed = parked.pointer("/subscriptions/entries").and_then(Value::as_array);
    step_check(
        misses,
        "6.7",
        deadline.is_some_and(|deadline| deadline < A1_DEADLINE)
            && elapsed.is_some_and(|elapsed| elapsed < CAP_SPAN)
            && text(parked, "/chip/phase") == "idle"
            && number(parked, "/chip/task") == Some(CAP_SUBOWNER)
            && number(a5, "/authority/authority/deadline") == deadline
            && number(owner, "/authority/authority/deadline") == Some(A_DEADLINE)
            && number(owner, "/goal") == Some(CAP_OWNER)
            && delegates.len() == 1
            && delegates.iter().all(|(_, args, out)| {
                contains(out, &format!("Delegated tasks: {CAP_A5}"))
                    && args["batch"].as_array().is_some_and(|batch| {
                        batch.len() == 1
                            && number(&batch[0], "/authority/budget/deadline") == deadline
                            && number(&batch[0], "/authority/delegation/depth") == Some(0)
                    })
            })
            && timers.len() == 1
            && timers.iter().all(|(_, args, out)| {
                args["kind"] == "timer"
                    && number(args, "/at") == deadline.and_then(|d| d.checked_add(CAP_TIMER_MARGIN))
                    && contains(out, &format!("Subscription {CAP_TIMER} added"))
            })
            && armed.is_some_and(|entries| {
                entries.len() == 1
                    && entries[0]["number"] == CAP_TIMER
                    && number(&entries[0], "/kind/timer/at") == deadline.and_then(|d| d.checked_add(CAP_TIMER_MARGIN))
                    && entries[0]["kind"]["timer"]["period"].is_null()
            }),
        CAP_OPENING,
        &format!(
            "SPAN too short for this run or invalid opening: SPAN={} ms, a5 admission/timer at {:?} ms from build ({:?} ms past deadline)",
            CAP_SPAN / 1_000_000,
            elapsed.map(|n| n / 1_000_000),
            elapsed.map(|n| i128::from(n).saturating_sub(i128::from(CAP_SPAN)) / 1_000_000)
        ),
    );
}

fn cap_admission(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let held = task_document(observed, Read::CapHeld);
    let rows = check_activity(observed, Read::CapHeld);
    let timer = cap_timer(observed);
    let timer_at = timer.and_then(cap_position);
    let parked = rows.iter().find(|r| text(r, "/line/run_ended/end") == "parked");
    step_check(
        misses,
        "6.7",
        number(held, "/chip/task") == Some(CAP_SUBOWNER)
            && text(held, "/chip/phase") == "held"
            && text(held, "/hold/owner/reason") == "deadline"
            && number(held, "/hold/owner/revision") == Some(CAP_HOLD_REVISION)
            && held["result"].is_null()
            && timer.is_some_and(|r| {
                number(r, "/line/heard/kind/timer/subscription") == Some(CAP_TIMER)
                    && number(r, "/at").zip(number(held, "/authority/authority/deadline")).is_some_and(|(at, d)| at > d)
            })
            && cap_before(parked.and_then(|r| cap_position(r)), timer_at)
            && rows.iter().filter(|r| r.pointer("/line/heard/kind/timer").is_some()).count() == 1
            && rows.iter().all(|r| {
                !cap_before(timer_at, cap_position(r))
                    || (r.pointer("/line/turn").is_none()
                        && r.pointer("/line/run_began").is_none()
                        && r.pointer("/line/run_ended").is_none())
            }),
        CAP_ADMISSION,
        "A1 must park, hear its exact timer past the deadline, and hold without admitting or ending another attempt",
    );
}

fn cap_escalation(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let held = task_document(observed, Read::CapHeld);
    let passed = task_document(observed, Read::CapPassed);
    let passes = plan_tool_results(observed, Agent::CapOwner, b"decide_escalation");
    let a = blocker_delivery(observed, Agent::CapOwner, "Held task needs a decision");
    let director = blocker_delivery(observed, Agent::CapDirector, "Held task needs a decision");
    let told_once = [Agent::CapOwner, Agent::CapDirector]
        .iter()
        .all(|agent| plan_deliveries(observed, *agent, "Held task needs a decision") == 1);
    step_check(
        misses,
        "6.7",
        number(held, "/hold/owner/escalation/waiting/holder/task") == Some(CAP_OWNER)
            && number(passed, "/hold/owner/escalation/waiting/holder/task") == Some(CAP_DIRECTOR)
            && number(passed, "/hold/owner/revision") == Some(CAP_HOLD_REVISION)
            && text(passed, "/hold/owner/reason") == "deadline"
            && number(task_document(observed, Read::CapOwnerWaiting), "/chip/task") == Some(CAP_OWNER)
            && told_once
            && passes.len() == 1
            && passes.iter().all(|(at, args, out)| {
                args["task"] == CAP_SUBOWNER
                    && args["revision"] == CAP_HOLD_REVISION
                    && args["decision"] == "pass"
                    && contains(
                        out,
                        &format!("Escalation for task {CAP_SUBOWNER} revision {CAP_HOLD_REVISION} decided"),
                    )
                    && a.is_some_and(|a| a < *at)
            })
            && a.zip(director).is_some_and(|(a, director)| a < director),
        CAP_ESCALATION,
        "the held A1 must wait at A, whose one pass reaches the director, each told once",
    );
}

fn cap_amendment(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let held = task_document(observed, Read::CapHeld);
    let released = task_document(observed, Read::CapReleased);
    let ended = task_document(observed, Read::CapSubownerEnded);
    let amendments = plan_tool_results(observed, Agent::CapDirector, b"amend");
    step_check(
        misses,
        "6.7",
        number(held, "/spec_revision/value") == Some(CAP_REVISION - 1)
            && [released, ended].iter().all(|doc| {
                number(doc, "/spec_revision/value") == Some(CAP_REVISION)
                    && number(doc, "/authority/authority/deadline") == Some(A1_DEADLINE)
                    && text(doc, "/instructions/bytes") == CAP_SPLIT
                    && doc["instructions"]["cut"] == false
            })
            && amendments.len() == 1
            && amendments.iter().all(|(_, args, out)| {
                args["target"] == CAP_SUBOWNER
                    && args["revision"] == CAP_REVISION - 1
                    && args["amendment"]["instructions"] == CAP_SPLIT
                    && args["amendment"]["reason"] == CAP_REASON
                    && args["amendment"]["authority"]
                        == serde_json::json!({"tools":33,"grants":[],"notes":0,
                        "delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":1,"depth":1},
                        "budget":{"spend":22,"deadline":A1_DEADLINE}})
                    && contains(out, &format!("Done: task {CAP_SUBOWNER} is at spec revision {CAP_REVISION}"))
            }),
        CAP_AMENDMENT,
        "the director must restate A1's authority with a later deadline and replace its instructions at the next spec revision",
    );
}

fn cap_release(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let released = task_document(observed, Read::CapReleased);
    let amendments = plan_tool_results(observed, Agent::CapDirector, b"amend");
    let releases = plan_tool_results(observed, Agent::CapDirector, b"release");
    let rows = check_activity(observed, Read::CapSubownerEnded);
    let held_position = check_activity(observed, Read::CapPassed).first().and_then(|r| cap_position(r));
    let mut decisions: Vec<_> = rows
        .iter()
        .filter(|r| {
            matches!(number(r,"/line/decided/revision"),Some(rev) if rev==CAP_REVISION || rev==CAP_RELEASE_DECISION)
                && cap_before(held_position, cap_position(r))
        })
        .filter_map(|r| cap_position(r))
        .collect();
    decisions.sort_unstable();
    let next = rows.iter().rev().find(|r| {
        r.pointer("/line/run_began").is_some()
            && decisions.last().is_some_and(|release| cap_before(Some(*release), cap_position(r)))
    });
    let split_told = observed.told.iter().any(|t| t.agent == Agent::CapSubowner && contains(&t.system, CAP_SPLIT));
    step_check(
        misses,
        "6.7",
        released["hold"].is_null()
            && number(released, "/chip/task") == Some(CAP_SUBOWNER)
            && decisions.len() == 2
            && decisions[0] < decisions[1]
            && releases.len() == 1
            && releases.iter().all(|(at, args, out)| {
                args["target"] == CAP_SUBOWNER
                    && args["revision"] == CAP_HOLD_REVISION
                    && contains(out, &format!("Done: task {CAP_SUBOWNER} is at spec revision {CAP_REVISION}"))
                    && amendments.first().is_some_and(|(amended, _, _)| amended < at)
            })
            && next.is_some()
            && split_told
            && decisions.last().is_some_and(|release| {
                rows.iter().any(|r| {
                    text(r, "/line/run_ended/end") == "finished" && cap_before(Some(*release), cap_position(r))
                })
            })
            && cap_timer(observed).and_then(cap_position).is_some_and(|timer| {
                rows.iter().all(|r| {
                    !cap_before(Some(timer), cap_position(r))
                        || !cap_before(cap_position(r), decisions.last().copied())
                        || (r.pointer("/line/run_began").is_none() && r.pointer("/line/turn").is_none())
                })
            }),
        CAP_RELEASE,
        "amend then release must precede A1's next admitted run, split instructions and report, with no intervening turn",
    );
}

fn cap_running(observed: &Observed, misses: &mut Vec<Miss>) {
    use super::*;
    let writing = task_document(observed, Read::CapA5Writing);
    let held = task_document(observed, Read::CapHeld);
    let rows = check_activity(observed, Read::CapA5Ended);
    let begins: Vec<_> = rows.iter().filter(|r| r.pointer("/line/run_began").is_some()).collect();
    let ends: Vec<_> = rows.iter().filter(|r| r.pointer("/line/run_ended").is_some()).collect();
    let attempt = number(writing, "/chip/details/owner/phase/active/running/attempt");
    step_check(
        misses,
        "6.7",
        batch_writing(observed, Read::CapA5Writing, CAP_A5, Read::CapA5Ended)
            && begins.len() == 1
            && ends.len() == 1
            && attempt.is_some()
            && number(begins[0], "/line/run_began/attempt") == attempt
            && number(ends[0], "/line/run_ended/attempt") == attempt
            && text(ends[0], "/line/run_ended/end") == "finished"
            && number(begins[0], "/at")
                .zip(number(held, "/authority/authority/deadline"))
                .is_some_and(|(at, d)| at < d)
            && number(ends[0], "/at").zip(number(held, "/authority/authority/deadline")).is_some_and(|(at, d)| at > d)
            && number(task_document(observed, Read::CapA5Ended), "/authority/authority/deadline")
                == number(held, "/authority/authority/deadline")
            && writing["hold"].is_null()
            && check_activity(observed, Read::CapA5Writing).iter().all(|r| r.pointer("/line/run_ended").is_none()),
        CAP_RUNNING,
        "a5 must still write while A1 is held, and finish the sole attempt admitted before the cap",
    );
}

#[cfg(test)]
mod cap_tests {
    use super::batch_tests::{input, journal, told, tool};
    use super::*;
    use crate::case::*;
    use serde_json::json;

    const BUILT: u64 = 1_000_000_000;
    fn deadline() -> u64 {
        BUILT + CAP_SPAN
    }
    fn source(task: u64, lines: &[Value], walls: &[u64]) -> Value {
        let mut value = journal(task, lines);
        for row in value["rows"].as_array_mut().expect("rows") {
            let source = &mut row["event"]["owner"]["source"];
            let position = usize::try_from(source["position"].as_u64().expect("position")).expect("position fits");
            source["at"] = json!(walls.get(position - 1).copied().unwrap_or(BUILT));
        }
        value
    }
    fn met() -> Observed {
        let mut observed = Observed {
            documents: BTreeMap::new(),
            told: Vec::new(),
            commands: Vec::new(),
            kit: Kit { records: Vec::new(), main: BTreeMap::new(), merged_green: false, pulls: Vec::new() },
        };
        for (read, task, parent, report) in cap_tasks() {
            observed.documents.insert(
                read,
                json!({"task":{
                "chip":{"task":task,"phase":"ended"},"project":PROJECT,"requester":{"task":parent},
                "goal":CAP_OWNER,"authority":{"authority":{"deadline":A_DEADLINE}},
                "result":{"ending":{"task":{"done":{}}},"value":{"report":{"bytes":report}}}
            },"activity":source(task,&[
                json!({"run_began":{"attempt":1}}),json!({"turn":{"attempt":1}}),
                json!({"run_ended":{"attempt":1,"end":"finished","spent":1}})
            ],&[BUILT;3])}),
            );
        }
        let held = opening_documents(&mut observed);
        released_documents(&mut observed, held);
        implementer_documents(&mut observed);
        for (latest, first) in
            [(Read::CapSubownerEnded, Read::CapSubownerFirst), (Read::CapDirectorEnded, Read::CapDirectorFirst)]
        {
            let page = &mut observed.documents.get_mut(&latest).expect("latest")["activity"];
            let rows = page["rows"].as_array_mut().expect("rows");
            let older = rows.split_off(rows.len() - 2);
            page["before"] = json!({"value":"older"});
            observed.documents.insert(first, json!({"rows":older,"cut":false,"before":null,"after":{"value":"newer"}}));
        }
        queries(&mut observed);
        observed
    }
    fn opening_documents(observed: &mut Observed) -> Vec<Value> {
        let opening = vec![
            json!({"run_began":{"attempt":1}}),
            json!({"turn":{"attempt":1}}),
            json!({"turn":{"attempt":1}}),
            json!({"run_ended":{"attempt":1,"end":"parked","spent":2}}),
        ];
        let mut held = opening.clone();
        held.extend([
            json!({"heard":{"number":1,"kind":{"timer":{"subscription":CAP_TIMER}}}}),
            json!({"decided":{"revision":CAP_HOLD_REVISION}}),
        ]);
        let mut walls = vec![BUILT + 1_000_000; 4];
        walls.extend([deadline() + CAP_TIMER_MARGIN; 2]);
        let held_doc = json!({"task":{"chip":{"task":CAP_SUBOWNER,"phase":"held"},"project":PROJECT,
            "spec_revision":{"value":CAP_REVISION-1},"authority":{"authority":{"deadline":deadline()}},
            "hold":{"owner":{"reason":"deadline","revision":CAP_HOLD_REVISION,
                "escalation":{"waiting":{"holder":{"task":CAP_OWNER}}}}},"result":null},
            "activity":source(CAP_SUBOWNER,&held,&walls)});
        observed.documents.insert(Read::CapHeld, held_doc.clone());
        let mut passed = held_doc;
        passed["task"]["hold"]["owner"]["escalation"]["waiting"]["holder"]["task"] = json!(CAP_DIRECTOR);
        observed.documents.insert(Read::CapPassed, passed);
        observed.documents.insert(Read::CapParked,json!({"task":{"chip":{"task":CAP_SUBOWNER,"phase":"idle"},
            "subscriptions":{"entries":[{"number":CAP_TIMER,"kind":{"timer":{"at":deadline()+CAP_TIMER_MARGIN,"period":null}}}]}},
            "activity":source(CAP_SUBOWNER,&opening,&walls)}));
        held
    }
    fn released_documents(observed: &mut Observed, held: Vec<Value>) {
        let mut released = held;
        released.extend([
            json!({"heard":{"kind":{"amendment":{"revision":CAP_REVISION}},"words":CAP_SPLIT}}),
            json!({"decided":{"revision":CAP_REVISION}}),
            json!({"decided":{"revision":CAP_RELEASE_DECISION}}),
            json!({"run_began":{"attempt":2,"why":"released"}}),
            json!({"turn":{"attempt":2}}),
            json!({"run_ended":{"attempt":2,"end":"parked","spent":1}}),
        ]);
        observed.documents.insert(
            Read::CapReleased,
            json!({"task":{"chip":{"task":CAP_SUBOWNER,"phase":"idle"},
            "spec_revision":{"value":CAP_REVISION},"authority":{"authority":{"deadline":A1_DEADLINE}},
            "instructions":{"bytes":CAP_SPLIT,"cut":false},"hold":null},
            "activity":source(CAP_SUBOWNER,&released,&[])}),
        );
        released.extend([
            json!({"run_began":{"attempt":3}}),
            json!({"turn":{"attempt":3}}),
            json!({"run_ended":{"attempt":3,"end":"finished","spent":1}}),
        ]);
        let ended = observed.documents.get_mut(&Read::CapSubownerEnded).expect("A1");
        ended["activity"] = source(CAP_SUBOWNER, &released, &[]);
        ended["task"]["spec_revision"] = json!({"value":CAP_REVISION});
        ended["task"]["authority"]["authority"]["deadline"] = json!(A1_DEADLINE);
        ended["task"]["instructions"] = json!({"bytes":CAP_SPLIT,"cut":false});
    }
    fn implementer_documents(observed: &mut Observed) {
        observed.documents.get_mut(&Read::CapA5Ended).expect("a5")["task"]["authority"]["authority"]["deadline"] =
            json!(deadline());
        let a5_lines = [
            json!({"run_began":{"attempt":1}}),
            json!({"turn":{"attempt":1}}),
            json!({"run_ended":{"attempt":1,"end":"finished","spent":1}}),
        ];
        observed.documents.get_mut(&Read::CapA5Ended).expect("a5")["activity"] =
            source(CAP_A5, &a5_lines, &[BUILT + 1_000_000, deadline() + 2_000_000, deadline() + 3_000_000]);
        observed.documents.insert(Read::CapA5Writing,json!({"task":{"chip":{"task":CAP_A5,"phase":"running",
            "details":{"owner":{"phase":{"active":{"running":{"attempt":1}}},"details":{"observation":{"work":"writing"}}}}},
            "hold":null,"authority":{"authority":{"deadline":deadline()}}},
            "activity":source(CAP_A5,&a5_lines[..1],&[BUILT+1_000_000])}));
        observed.documents.insert(Read::CapOwnerWaiting, json!({"task":{"chip":{"task":CAP_OWNER}}}));
        for (read, after) in
            [(Read::CapInboxHeld, Read::CapInboxHeldAfter), (Read::CapInboxEnded, Read::CapInboxEndedAfter)]
        {
            observed.documents.insert(read, json!({"needs":{"cut":false,"after":{"value":"stored"},"rows":[]}}));
            observed.documents.insert(after, json!({"needs":{"cut":false,"after":null,"rows":[]}}));
        }
    }
    fn queries(observed: &mut Observed) {
        let mut subowner = tool(
            "delegate",
            "delegate",
            &json!({"batch":[{"authority":{"budget":{"deadline":deadline()},"delegation":{"depth":0}}}]}),
            &format!("Delegated tasks: {CAP_A5}"),
        )
        .to_vec();
        observed.told.push(told(Agent::CapSubowner, &subowner));
        subowner.extend(tool(
            "timer",
            "subscribe",
            &json!({"kind":"timer","at":deadline()+CAP_TIMER_MARGIN}),
            &format!("Subscription {CAP_TIMER} added"),
        ));
        observed.told.push(told(Agent::CapSubowner, &subowner));
        observed.told.push(told(Agent::CapOwner, &[input("Held task needs a decision")]));
        observed.told.push(told(
            Agent::CapOwner,
            &tool(
                "pass",
                "decide_escalation",
                &json!({"task":CAP_SUBOWNER,"revision":CAP_HOLD_REVISION,"decision":"pass"}),
                &format!("Escalation for task {CAP_SUBOWNER} revision {CAP_HOLD_REVISION} decided"),
            ),
        ));
        observed.told.push(told(Agent::CapDirector, &[input("Held task needs a decision")]));
        let authority = json!({"tools":33,"grants":[],"notes":0,
            "delegation":{"kinds":[{"kind":"agent","number":1}],"tasks":1,"depth":1},
            "budget":{"spend":22,"deadline":A1_DEADLINE}});
        let mut director = tool(
            "amend",
            "amend",
            &json!({"target":CAP_SUBOWNER,"revision":CAP_REVISION-1,
            "amendment":{"authority":authority,"instructions":CAP_SPLIT,"reason":CAP_REASON}}),
            &format!("Done: task {CAP_SUBOWNER} is at spec revision {CAP_REVISION}"),
        )
        .to_vec();
        observed.told.push(told(Agent::CapDirector, &director));
        director.extend(tool(
            "release",
            "release",
            &json!({"target":CAP_SUBOWNER,"revision":CAP_HOLD_REVISION}),
            &format!("Done: task {CAP_SUBOWNER} is at spec revision {CAP_REVISION}"),
        ));
        observed.told.push(told(Agent::CapDirector, &director));
        observed.told.push(
            Told::new(
                format!("{}\n{CAP_SPLIT}", String::from_utf8_lossy(Agent::CapSubowner.cue())).as_bytes(),
                b"fake",
                None,
                &[input("Continue")],
            )
            .expect("A1"),
        );
    }
    fn rejects(observed: &Observed, promise: Promise) {
        let misses = step_6_7(observed);
        assert!(
            misses.iter().any(|m| m.step == "6.7" && m.promise == promise),
            "mutation did not fail {promise:?}: {}",
            misses.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        );
    }
    fn field(read: Read, path: &str, promise: Promise) {
        let mut bad = met();
        let value = bad.documents.get_mut(&read).expect("doc").pointer_mut(path).expect(path);
        *value = if value.is_null() { json!("unexpected") } else { Value::Null };
        rejects(&bad, promise);
    }
    #[test]
    fn a_complete_observation_meets_the_step() {
        assert_met(&step_6_7(&met()));
    }
    #[test]
    fn opening_requires_both_commits_before_the_deadline_and_the_exact_timer() {
        for (read, path) in [
            (Read::CapParked, "/task/chip/phase"),
            (Read::CapParked, "/task/subscriptions/entries/0/number"),
            (Read::CapParked, "/task/subscriptions/entries/0/kind/timer/at"),
            (Read::CapA5Writing, "/task/authority/authority/deadline"),
            (Read::CapOwnerEnded, "/task/goal"),
            (Read::CapOwnerEnded, "/task/authority/authority/deadline"),
        ] {
            field(read, path, CAP_OPENING);
        }
        for read in [Read::CapHeld, Read::CapA5Writing] {
            let mut bad = met();
            let rows = bad.documents.get_mut(&read).expect("doc")["activity"]["rows"].as_array_mut().expect("rows");
            for row in rows {
                row["event"]["owner"]["source"]["at"] = json!(deadline() + 1);
            }
            rejects(&bad, CAP_OPENING);
        }
    }
    #[test]
    fn the_deadline_hold_is_an_unadmitted_timer_wake() {
        for path in ["/task/chip/task", "/task/chip/phase", "/task/hold/owner/reason", "/task/hold/owner/revision"] {
            field(Read::CapHeld, path, CAP_ADMISSION);
        }
        for line in [
            json!({"run_began":{"attempt":2}}),
            json!({"turn":{"attempt":2}}),
            json!({"run_ended":{"attempt":2,"end":"failed"}}),
        ] {
            let mut bad = met();
            bad.documents.get_mut(&Read::CapHeld).expect("held")["activity"]["rows"]
                .as_array_mut()
                .expect("rows")
                .insert(0, json!({"event":{"owner":{"source":{"task":CAP_SUBOWNER,"position":7,"line":line}}}}));
            rejects(&bad, CAP_ADMISSION);
        }
    }
    #[test]
    fn the_escalation_waits_at_each_holder_and_is_told_once() {
        for read in [Read::CapHeld, Read::CapPassed] {
            field(read, "/task/hold/owner/escalation/waiting/holder/task", CAP_ESCALATION);
        }
        for agent in [Agent::CapOwner, Agent::CapDirector] {
            let mut missing = met();
            missing
                .told
                .retain(|t| !(t.agent == agent && user_words(t).any(|w| contains(w, "Held task needs a decision"))));
            rejects(&missing, CAP_ESCALATION);
            let mut duplicate = met();
            duplicate.told.push(told(agent, &[input("Held task needs a decision")]));
            rejects(&duplicate, CAP_ESCALATION);
        }
    }
    #[test]
    fn the_director_changes_the_next_revision_deadline_and_instructions() {
        for read in [Read::CapReleased, Read::CapSubownerEnded] {
            for path in [
                "/task/spec_revision/value",
                "/task/authority/authority/deadline",
                "/task/instructions/bytes",
                "/task/instructions/cut",
            ] {
                field(read, path, CAP_AMENDMENT);
            }
        }
        let mut bad = met();
        bad.told.retain(|t| !(t.agent == Agent::CapDirector && contains(&t.words, "Done: task")));
        rejects(&bad, CAP_AMENDMENT);
    }
    #[test]
    fn release_precedes_the_next_run_and_report_and_carries_the_split() {
        let mut held = met();
        held.documents.get_mut(&Read::CapReleased).expect("release")["task"]["hold"] =
            json!({"owner":{"reason":"deadline"}});
        rejects(&held, CAP_RELEASE);
        let mut before = met();
        let rows = before.documents.get_mut(&Read::CapSubownerEnded).expect("A1")["activity"]["rows"]
            .as_array_mut()
            .expect("rows");
        for row in rows {
            let source = &mut row["event"]["owner"]["source"];
            if source["line"].get("run_began").is_some() && source["line"]["run_began"]["attempt"] == 2 {
                source["position"] = json!(8);
            }
        }
        rejects(&before, CAP_RELEASE);
        let mut no_split = met();
        no_split.told.retain(|t| !(t.agent == Agent::CapSubowner && contains(&t.system, CAP_SPLIT)));
        rejects(&no_split, CAP_RELEASE);
    }
    #[test]
    fn a5_must_still_write_and_finish_exactly_its_one_attempt_across_the_cap() {
        field(Read::CapA5Ended, "/task/authority/authority/deadline", CAP_RUNNING);
        for path in [
            "/task/chip/task",
            "/task/chip/details/owner/details/observation/work",
            "/task/chip/details/owner/phase/active/running/attempt",
        ] {
            field(Read::CapA5Writing, path, CAP_RUNNING);
        }
        for (began, ended) in [(deadline() + 1, deadline() + 2), (BUILT, deadline() - 1)] {
            let mut bad = met();
            let rows =
                bad.documents.get_mut(&Read::CapA5Ended).expect("a5")["activity"]["rows"].as_array_mut().expect("rows");
            rows[0]["event"]["owner"]["source"]["at"] = json!(ended);
            rows[2]["event"]["owner"]["source"]["at"] = json!(began);
            rejects(&bad, CAP_RUNNING);
        }
        let mut second = met();
        second.documents.get_mut(&Read::CapA5Ended).expect("a5")["activity"]["rows"]
            .as_array_mut()
            .expect("rows")
            .insert(
                0,
                json!({"event":{"owner":{"source":{"task":CAP_A5,"position":4,"line":{"run_began":{"attempt":2}}}}}}),
            );
        rejects(&second, CAP_RUNNING);
    }
    #[test]
    fn both_inbox_reads_are_complete_and_empty() {
        for read in [Read::CapInboxHeld, Read::CapInboxHeldAfter, Read::CapInboxEnded, Read::CapInboxEndedAfter] {
            field(read, "/needs/cut", CAP_INBOX);
            let mut bad = met();
            bad.documents.get_mut(&read).expect("inbox")["needs"]["rows"] = json!([{"question":1}]);
            rejects(&bad, CAP_INBOX);
        }
    }
    #[test]
    fn both_inbox_continuations_must_be_exhausted() {
        for read in [Read::CapInboxHeldAfter, Read::CapInboxEndedAfter] {
            field(read, "/needs/after", CAP_INBOX);
        }
    }
    #[test]
    fn every_decision_requires_its_successful_tool_exchange() {
        use skein_fake_llm_domain::api::Part;
        for (agent, name, promise) in [
            (Agent::CapSubowner, b"delegate".as_slice(), CAP_OPENING),
            (Agent::CapSubowner, b"subscribe".as_slice(), CAP_OPENING),
            (Agent::CapOwner, b"decide_escalation".as_slice(), CAP_ESCALATION),
            (Agent::CapDirector, b"amend".as_slice(), CAP_AMENDMENT),
            (Agent::CapDirector, b"release".as_slice(), CAP_RELEASE),
        ] {
            let mut bad = met();
            for query in bad.told.iter_mut().filter(|q| q.agent == agent) {
                let ids: BTreeSet<_> = query
                    .history
                    .iter()
                    .flat_map(|m| &m.parts)
                    .filter_map(|p| match p {
                        Part::ToolCall { id, name: called, .. } if called.as_ref() == name => Some(id.clone()),
                        Part::ToolCall { .. } | Part::ToolOutput { .. } | Part::Text { .. } | Part::Opaque { .. } => {
                            None
                        }
                    })
                    .collect();
                for message in &mut query.history {
                    message.parts = message
                        .parts
                        .iter()
                        .filter(|p| {
                            !matches!(p,
                        Part::ToolOutput{id,..} if ids.contains(id))
                        })
                        .cloned()
                        .collect();
                }
            }
            rejects(&bad, promise);
        }
    }
    #[test]
    fn older_pages_must_be_complete_and_preserve_the_first_attempt() {
        for read in [Read::CapSubownerFirst, Read::CapDirectorFirst] {
            field(read, "/cut", CAP_ATTEMPTS);
            field(read, "/before", CAP_ATTEMPTS);
            let mut bad = met();
            bad.documents.get_mut(&read).expect("first")["rows"].as_array_mut().expect("rows").pop();
            rejects(&bad, CAP_ATTEMPTS);
        }
    }
    #[test]
    fn every_cast_member_has_a_done_report_and_complete_attempt_history() {
        for (read, _, _, _) in cap_tasks() {
            for path in
                ["/task/chip/task", "/task/project", "/task/result/value/report/bytes", "/task/result/ending/task/done"]
            {
                field(read, path, CAP_CHECKPOINT);
            }
            field(read, "/activity/cut", CAP_ATTEMPTS);
            let mut bad = met();
            bad.documents.get_mut(&read).expect("task")["activity"]["rows"][0]["event"]["owner"]["source"]["line"]["run_ended"]
                ["end"] = json!({"failed":{"class":"agent"}});
            rejects(&bad, CAP_ATTEMPTS);
        }
    }
}
