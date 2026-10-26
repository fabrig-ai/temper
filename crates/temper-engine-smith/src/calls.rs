//! Engine host-tool declarations and whole-object decoding. A malformed call
//! is feedback to Smith's LLM, never a root decision (domain/agent.md, 4.3).

use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, List, Wall, bytes};
use smith_domain_run as run;
use temper_engine_domain as engine;

use crate::{Problem, forge, json::Value, nested, notes};

struct Spec {
    name: &'static [u8],
    description: &'static [u8],
    schema: &'static [u8],
    effect: run::HostEffect,
}

const READ_SCHEMA: &[u8] = br#"{"type":"object","properties":{"repository":{"type":"object"},"read":{"type":"object"}},"required":["repository","read"]}"#;

const SPECS: [Spec; 35] = [
    Spec { name: b"read_items", description: b"Read one page of forge items.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_item", description: b"Read an issue or pull and its comments.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_pull", description: b"Read a pull request.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_pull_for", description: b"Find a pull by head and base.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_reviews", description: b"Read reviews on a pull request.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_statuses", description: b"Read statuses on a commit.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_remarks", description: b"Read review remarks.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_branch", description: b"Read a branch head.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_branches", description: b"List branches.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_pull_files", description: b"Read changed files on a pull.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_compare", description: b"Compare two commits.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_checks", description: b"Read a commit's combined checks.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_file", description: b"Read a bounded file at one head.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_job", description: b"Read one bounded CI job log.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_protection", description: b"Read branch protection.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_settings", description: b"Read repository settings.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_collaborators", description: b"Read repository collaborators.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"read_permission", description: b"Read a user's repository permission.", schema: READ_SCHEMA, effect: run::HostEffect::Read },
    Spec { name: b"effect_forge", description: b"Ask the connector for a keyed forge write.", schema: br#"{"type":"object","properties":{"repository":{"type":"object"},"resource":{"type":"object"},"write":{"type":"object"}},"required":["repository","resource","write"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"subscribe_forge", description: b"Hear matching forge news.", schema: br#"{"type":"object","properties":{"topic":{"type":"object"},"own_change":{"type":"integer","minimum":1},"paths":{"type":"array","items":{"type":"string"}}},"required":["topic"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"change", description: b"Have one change made to a repository of this project and landed: temper writes it on a branch, opens a pull request, waits for CI and merges it; you hear its result. words is all the coder sees: say what to change, which files and the exact text where it matters. Name the repository as owner/name when the project has more than one; the answer names them if you don't.", schema: br#"{"type":"object","properties":{"words":{"type":"string"},"repository":{"type":"string"}},"required":["words"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"delegate", description: b"Create an authorized batch of direct child tasks. tracked {priority} makes one a goal, the lower first; omit it for untracked. Each llm choice names main {model, effort?}, optional subagents and an optional account.", schema: br#"{"type":"object","properties":{"batch":{"type":"array","items":{"type":"object","properties":{"executor":{"type":"object","description":"{kind:agent,charter} | {kind:procedure,connector,code} | {kind:person,person} | {kind:role,role}","properties":{"kind":{"type":"string","enum":["agent","procedure","person","role"]},"charter":{"type":"integer","minimum":0},"connector":{"type":"integer","minimum":0},"code":{"type":"integer","minimum":0},"person":{"type":"integer","minimum":0},"role":{"type":"integer","minimum":0}},"required":["kind"]},"spec":{"type":"object","properties":{"words":{"type":"string"},"parameters":{"type":"array","items":{"type":"object","description":"{name,kind:number,value:integer} | {name,kind:bytes,value:string} | {name,kind:resource,connector,resource}","properties":{"name":{"type":"integer","minimum":0},"kind":{"type":"string","enum":["number","bytes","resource"]},"value":{},"connector":{"type":"integer","minimum":0},"resource":{"type":"integer","minimum":0}},"required":["name","kind"]}},"inputs":{"type":"array","items":{"type":"integer","minimum":0}}},"required":["words"]},"contract":{"type":"object","description":"{kind:report,words} | {kind:change,connector,change_kind,words} | {kind:verdict,choices:[{code,words}]}","properties":{"kind":{"type":"string","enum":["report","change","verdict"]},"words":{"type":"integer","minimum":0},"connector":{"type":"integer","minimum":0},"change_kind":{"type":"integer","minimum":0},"choices":{"type":"array","items":{"type":"object","properties":{"code":{"type":"integer","minimum":0},"words":{"type":"integer","minimum":0}},"required":["code","words"]}}},"required":["kind"]},"authority":{"type":"object","description":"Whole: restate every field you keep.","properties":{"tools":{"type":"integer","minimum":0,"description":"Tool families, as a bit set."},"grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}},"delegation":{"type":"object","properties":{"kinds":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["agent","procedure","role"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"tasks":{"type":"integer","minimum":0},"depth":{"type":"integer","minimum":0}},"required":["kinds","tasks","depth"]},"budget":{"type":"object","properties":{"spend":{"type":"integer","minimum":0},"deadline":{"type":"integer","minimum":0,"description":"Wall-clock nanoseconds since the Unix epoch."}},"required":["spend"]},"notes":{"type":"integer","minimum":0,"maximum":255},"note_resources":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"pattern":{"type":"object","properties":{"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["segments","terminal","last"]}},"required":["connector","pattern"]}}},"required":["tools","grants","delegation","budget","notes"]},"deadline_seconds":{"type":"integer","minimum":1,"description":"It must end this many seconds after it is made. Give this or authority.budget.deadline, not both; with neither, it ends when you must."},"tracked":{"type":"object","properties":{"priority":{"type":"integer","minimum":0,"maximum":4294967295}},"required":["priority"],"description":"Track it as a goal at this priority; the lower goes first. Omit for untracked."},"dependencies":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["batch","existing"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"wake":{"type":"object","properties":{"words":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"notices":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"news":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"results":{"type":"string","enum":["never","each","last_or_failure"]},"questions":{"type":"boolean"},"answers":{"type":"boolean"},"timers":{"type":"boolean"}},"required":["words","notices","news","results","questions","answers","timers"]},"llm":{"type":"object","properties":{"main":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]},"subagents":{"type":"array","items":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]}},"account":{"type":"integer","minimum":0}},"required":["main"]},"symbolic_grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}}},"required":["executor","spec","contract","authority"]}}},"required":["batch"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"amend", description: b"Amend a live delegate within authority, naming only what changes. A tracked delegate's priority alone needs no revision. A deadline is authority.budget.deadline, with the rest of authority restated as it is. An llm choice names main {model, effort?}, optional subagents and an optional account.", schema: br#"{"type":"object","examples":[{"target":3,"revision":1,"amendment":{"reason":"Finish by Friday.","authority":{"tools":5,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":1000,"deadline":1793404800000000000},"notes":0}}}],"properties":{"target":{"type":"integer","minimum":1},"revision":{"type":"integer","minimum":0,"description":"The spec revision you last saw; omit only to change a tracked delegate's priority alone."},"amendment":{"type":"object","properties":{"reason":{"type":"string","description":"What the delegate's live run hears, unless instructions replace it."},"tracked":{"oneOf":[{"type":"object","properties":{"priority":{"type":"integer","minimum":0,"maximum":4294967295}},"required":["priority"]},{"type":"string","enum":["untracked"]}],"description":"{priority} tracks it as a goal, the lower first; \"untracked\" stops."},"instructions":{"type":"string","description":"Its own instructions, whole; empty leaves it none."},"spec":{"type":"object","properties":{"words":{"type":"string"},"parameters":{"type":"array","items":{"type":"object","description":"{name,kind:number,value:integer} | {name,kind:bytes,value:string} | {name,kind:resource,connector,resource}","properties":{"name":{"type":"integer","minimum":0},"kind":{"type":"string","enum":["number","bytes","resource"]},"value":{},"connector":{"type":"integer","minimum":0},"resource":{"type":"integer","minimum":0}},"required":["name","kind"]}},"inputs":{"type":"array","items":{"type":"integer","minimum":0}}},"required":["words"]},"authority":{"type":"object","description":"Whole: restate every field you keep.","properties":{"tools":{"type":"integer","minimum":0,"description":"Tool families, as a bit set."},"grants":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"kind":{"type":"integer","minimum":0},"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["connector","kind","segments","terminal","last"]}},"delegation":{"type":"object","properties":{"kinds":{"type":"array","items":{"type":"object","properties":{"kind":{"type":"string","enum":["agent","procedure","role"]},"number":{"type":"integer","minimum":0}},"required":["kind","number"]}},"tasks":{"type":"integer","minimum":0},"depth":{"type":"integer","minimum":0}},"required":["kinds","tasks","depth"]},"budget":{"type":"object","properties":{"spend":{"type":"integer","minimum":0},"deadline":{"type":"integer","minimum":0,"description":"Wall-clock nanoseconds since the Unix epoch."}},"required":["spend"]},"notes":{"type":"integer","minimum":0,"maximum":255},"note_resources":{"type":"array","items":{"type":"object","properties":{"connector":{"type":"integer","minimum":0},"pattern":{"type":"object","properties":{"segments":{"type":"array","items":{"type":"string"}},"terminal":{"type":"string","enum":["exact","open"]},"last":{"type":"string"}},"required":["segments","terminal","last"]}},"required":["connector","pattern"]}}},"required":["tools","grants","delegation","budget","notes"]},"dependencies":{"type":"array","description":"The whole list of existing tasks.","items":{"type":"integer","minimum":1}},"wake":{"type":"object","properties":{"words":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"notices":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"news":{"type":"object","properties":{"kind":{"type":"string","enum":["never","immediate","batch"]},"count":{"type":"integer","minimum":0},"age":{"type":"integer","minimum":0,"description":"Nanoseconds."}},"required":["kind"]},"results":{"type":"string","enum":["never","each","last_or_failure"]},"questions":{"type":"boolean"},"answers":{"type":"boolean"},"timers":{"type":"boolean"}},"required":["words","notices","news","results","questions","answers","timers"]},"llm":{"type":"object","properties":{"main":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]},"subagents":{"type":"array","items":{"type":"object","properties":{"model":{"type":"integer","minimum":0,"maximum":255},"effort":{"type":"integer","minimum":0,"maximum":255}},"required":["model"]}},"account":{"type":"integer","minimum":0}},"required":["main"]}},"required":["reason"]}},"required":["target","amendment"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"message", description: b"Send words, a question or an answer to a referenced task.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","minimum":1},"form":{"type":"string","enum":["words","question","answer"]},"words":{"type":"string"},"question":{"type":"integer","minimum":1}},"required":["target","form","words"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"cancel", description: b"Cancel a live delegate and its subtree.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","minimum":1},"reason":{"type":"string"}},"required":["target","reason"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"release", description: b"Release a held delegate.", schema: br#"{"type":"object","properties":{"target":{"type":"integer","minimum":1},"revision":{"type":"integer","minimum":0},"again":{"type":"boolean"}},"required":["target","revision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"introduce", description: b"Give two referenced tasks reciprocal references.", schema: br#"{"type":"object","properties":{"left":{"type":"integer","minimum":1},"right":{"type":"integer","minimum":1}},"required":["left","right"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"unsubscribe", description: b"Remove a standing subscription.", schema: br#"{"type":"object","properties":{"subscription":{"type":"integer","minimum":1}},"required":["subscription"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"withdraw", description: b"Withdraw one of your pending proposals.", schema: br#"{"type":"object","properties":{"proposal":{"type":"integer","minimum":1}},"required":["proposal"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"decide_escalation", description: b"Release, reject or pass a held descendant.", schema: br#"{"type":"object","properties":{"task":{"type":"integer","minimum":1},"revision":{"type":"integer","minimum":1},"decision":{"type":"string","enum":["release","reject","pass"]},"reason":{"type":"string"}},"required":["task","revision","decision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"decide", description: b"Accept, reject or pass a pending proposal.", schema: br#"{"type":"object","properties":{"proposer":{"type":"integer","minimum":1},"proposal":{"type":"integer","minimum":1},"decision":{"type":"string","enum":["accept","reject","pass"]},"reason":{"type":"string"}},"required":["proposer","proposal","decision"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"subscribe", description: b"Watch a referenced task or a timer.", schema: br#"{"type":"object","properties":{"kind":{"type":"string","enum":["task","timer"]},"target":{"type":"integer","minimum":1},"held":{"type":"boolean"},"result":{"type":"boolean"},"at":{"type":"integer","minimum":0},"period":{"type":"integer","minimum":1}},"required":["kind"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"propose", description: b"Ask the covering holder to act beyond your authority.", schema: br#"{"type":"object","properties":{"action":{"type":"string","enum":["batch","effect","amend","widen","release"]},"task":{"type":"integer","minimum":1},"revision":{"type":"integer","minimum":0},"batch":{"type":"array","items":{"type":"object"}},"amendment":{"type":"object"},"authority":{"type":"object"},"reason":{"type":"string"},"as_holder":{"type":"boolean"},"repository":{"type":"object"},"resource":{"type":"object"},"write":{"type":"object"}},"required":["action","reason"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"note", description: b"Write a scoped note, or revise one at the revision you recalled.", schema: br#"{"type":"object","properties":{"scope":{"type":"object"},"description":{"type":"string"},"body":{"type":"string"},"references":{"type":"array","items":{"type":"integer","minimum":1}},"name":{"type":"integer","minimum":1},"recalled":{"type":"integer","minimum":1}},"required":["scope","description","body"]}"#, effect: run::HostEffect::Write },
    Spec { name: b"recall", description: b"Recall a note by name or search scoped descriptions.", schema: br#"{"type":"object","properties":{"kind":{"type":"string","enum":["name","search"]},"name":{"type":"integer","minimum":1},"scopes":{"type":"array","items":{"type":"object"}},"query":{"type":"string"},"page":{"type":"integer","minimum":0}},"required":["kind"]}"#, effect: run::HostEffect::Read },
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
#[expect(clippy::too_many_lines, reason = "one exhaustive finite host-tool registry")]
pub fn call(name: run::CallName, tool: &[u8], input: &run::HostInput) -> Result<engine::Call, Problem> {
    let value = crate::json::parse(input.bytes())?;
    let tool = match tool {
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
        | b"read_permission" => engine::Tool::ReadForge {
            repository: forge::repository(value.required(b"repository")?)?,
            read: forge::read_named(tool, value.required(b"read")?)?,
        },
        b"effect_forge" => engine::Tool::EffectForge {
            repository: forge::repository(value.required(b"repository")?)?,
            resource: forge::resource(value.required(b"resource")?)?,
            write: Box::new(forge::write(value.required(b"write")?)?),
        },
        b"subscribe_forge" => engine::Tool::SubscribeForge {
            topic: forge::topic(value.required(b"topic")?)?,
            own_change: match value.field(b"own_change")? {
                Some(change) => Some(change.number()?),
                None => None,
            },
            paths: match value.field(b"paths")? {
                Some(paths) => forge::texts(paths)?,
                None => Box::new([]),
            },
        },
        b"change" => {
            let words = value.required(b"words")?.text()?;
            if words.is_empty() {
                return Err(Problem::Range);
            }
            let repository = match value.field(b"repository")? {
                Some(name) => {
                    let name = name.text()?;
                    let mut slash = None;
                    for (at, byte) in name.iter().enumerate() {
                        if *byte == b'/' {
                            if slash.is_some() {
                                return Err(Problem::Range);
                            }
                            slash = Some(at);
                        }
                    }
                    let slash = slash.ok_or(Problem::Range)?;
                    if slash == 0 || slash.checked_add(1) == Some(name.len()) {
                        return Err(Problem::Range);
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
                Some(value) => value.number()?,
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
                None => return Err(Problem::Missing),
            };
            engine::Tool::Amend { target: positive(&value, b"target")?, revision, amendment }
        }
        b"message" => {
            let target = positive(&value, b"target")?;
            let words = value.required(b"words")?.text()?;
            let form = value.required(b"form")?.text()?;
            let form = match form.as_ref() {
                b"words" => engine::MessageForm::Words,
                b"question" => engine::MessageForm::Question,
                b"answer" => engine::MessageForm::Answer { question: positive(&value, b"question")? },
                _ => return Err(Problem::Range),
            };
            engine::Tool::Message { target, form, words }
        }
        b"cancel" => {
            engine::Tool::Cancel { target: positive(&value, b"target")?, reason: value.required(b"reason")?.text()? }
        }
        b"release" => engine::Tool::Release {
            target: positive(&value, b"target")?,
            revision: number(&value, b"revision")?,
            again: match value.field(b"again")? {
                Some(again) => Some(again.boolean()?),
                None => None,
            },
        },
        b"introduce" => {
            engine::Tool::Introduce { left: positive(&value, b"left")?, right: positive(&value, b"right")? }
        }
        b"unsubscribe" => engine::Tool::Unsubscribe { subscription: positive(&value, b"subscription")? },
        b"withdraw" => engine::Tool::Withdraw { proposal: positive(&value, b"proposal")? },
        b"decide_escalation" => {
            let decision = value.required(b"decision")?.text()?;
            let decision = match decision.as_ref() {
                b"release" => engine::EscalationChoice::Release,
                b"reject" => engine::EscalationChoice::Reject { reason: value.required(b"reason")?.text()? },
                b"pass" => engine::EscalationChoice::Pass,
                _ => return Err(Problem::Range),
            };
            engine::Tool::DecideEscalation {
                task: positive(&value, b"task")?,
                revision: positive(&value, b"revision")?,
                decision,
            }
        }
        b"decide" => {
            let decision = value.required(b"decision")?.text()?;
            let decision = match decision.as_ref() {
                b"accept" => engine::ProposalChoice::Accept,
                b"reject" => engine::ProposalChoice::Reject { reason: value.required(b"reason")?.text()? },
                b"pass" => engine::ProposalChoice::Pass,
                _ => return Err(Problem::Range),
            };
            engine::Tool::Decide {
                proposer: positive(&value, b"proposer")?,
                proposal: positive(&value, b"proposal")?,
                decision,
            }
        }
        b"subscribe" => {
            let kind = value.required(b"kind")?.text()?;
            let kind = match kind.as_ref() {
                b"task" => tasks::SubscriptionKind::Task {
                    target: positive(&value, b"target")?,
                    held: optional_bool(&value, b"held", false)?,
                    result: optional_bool(&value, b"result", true)?,
                },
                b"timer" => tasks::SubscriptionKind::Timer {
                    at: Wall::from_nanos(number(&value, b"at")?),
                    period: match value.field(b"period")? {
                        Some(period) => Some(Duration::from_nanos(period.number()?)),
                        None => None,
                    },
                },
                _ => return Err(Problem::Range),
            };
            engine::Tool::Subscribe { kind }
        }
        b"propose" => {
            let action = value.required(b"action")?.text()?;
            let action = match action.as_ref() {
                b"effect" => engine::ProposedAction::Effect {
                    repository: forge::repository(value.required(b"repository")?)?,
                    resource: forge::resource(value.required(b"resource")?)?,
                    write: Box::new(forge::write(value.required(b"write")?)?),
                },
                b"release" => engine::ProposedAction::Release { task: positive(&value, b"task")? },
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
                _ => return Err(Problem::Range),
            };
            engine::Tool::Propose {
                action,
                reason: value.required(b"reason")?.text()?,
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
    Ok(engine::Call { completion: name.completion, position: name.position, tool })
}

fn number(value: &Value, field: &[u8]) -> Result<u64, Problem> {
    value.required(field)?.number()
}

fn positive(value: &Value, field: &[u8]) -> Result<u64, Problem> {
    let number = number(value, field)?;
    if number == 0 { Err(Problem::Range) } else { Ok(number) }
}

fn optional_bool(value: &Value, field: &[u8], default: bool) -> Result<bool, Problem> {
    match value.field(field)? {
        Some(value) => value.boolean(),
        None => Ok(default),
    }
}
