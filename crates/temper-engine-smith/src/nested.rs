//! Typed task values admitted from bounded JSON host inputs. Root authority
//! and tasks still check these values against current durable facts.

use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, List, Wall};
use temper_engine_domain as engine;

use crate::{Problem, json::Value};

pub(crate) fn delegate(value: &Value) -> Result<engine::Delegate, Problem> {
    let value = value.object(b"batch")?;
    let executor = executor(value.required(b"executor")?)?;
    let spec = spec(value.required(b"spec")?)?;
    let contract = contract(value.required(b"contract")?)?;
    let authority = authority(value.required(b"authority")?)?;
    let deadline = match value.field(b"deadline_seconds")? {
        Some(value) => {
            let seconds = value.number(b"deadline_seconds")?;
            if seconds == 0 || seconds > u64::MAX / 1_000_000_000 || authority.budget.deadline.is_some() {
                return Err(Problem::Range(b"deadline_seconds"));
            }
            tasks::Deadline::Span(Duration::from_secs(seconds))
        }
        None => tasks::Deadline::Named,
    };
    let symbolic_grants = match value.field(b"symbolic_grants")? {
        Some(grants) => grants_list(grants)?,
        None => Box::new([]),
    };
    let dependencies = match value.field(b"dependencies")? {
        Some(items) => {
            let items = items.array(b"dependencies")?;
            let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
            let mut out = List::with_capacity(capacity);
            for item in items {
                let kind = item.required(b"kind")?.text(b"kind")?;
                let dependency = match kind.as_ref() {
                    b"batch" => engine::Dependency::Batch(item.required(b"number")?.small(b"number")?),
                    b"existing" => engine::Dependency::Existing(item.required(b"number")?.number(b"number")?),
                    _ => return Err(Problem::Range(b"kind")),
                };
                let Ok(()) = out.push(dependency) else { return Err(Problem::TooLarge) };
            }
            out.into_boxed()
        }
        None => Box::new([]),
    };
    let wake = match value.field(b"wake")? {
        Some(wake) => wake_policy(wake)?,
        None => tasks::WakePolicy::DEFAULT,
    };
    Ok(engine::Delegate {
        deadline,
        executor,
        spec,
        contract,
        authority,
        llm: llm(value.field(b"llm")?)?,
        symbolic_grants,
        dependencies,
        wake,
        tracked: match value.field(b"tracked")? {
            Some(value) => Some(value.object(b"tracked")?.required(b"priority")?.small(b"priority")?),
            None => None,
        },
        label: 0,
        flags: tasks::Flags::PLAIN,
        budget: jig_domain_people::Budget::Named,
    })
}

pub(crate) fn delegates(value: &Value) -> Result<Box<[engine::Delegate]>, Problem> {
    let items = value.array(b"batch")?;
    let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for item in items {
        let Ok(()) = out.push(delegate(item)?) else { return Err(Problem::TooLarge) };
    }
    Ok(out.into_boxed())
}

pub(crate) fn amendment(value: &Value) -> Result<tasks::Amendment, Problem> {
    let value = value.object(b"amendment")?;
    let spec = match value.field(b"spec")? {
        Some(value) => Some(spec(value)?),
        None => None,
    };
    let wake = match value.field(b"wake")? {
        Some(value) => Some(wake_policy(value)?),
        None => None,
    };
    let dependencies = match value.field(b"dependencies")? {
        Some(value) => Some(numbers(value, b"dependencies")?),
        None => None,
    };
    let authority = match value.field(b"authority")? {
        Some(value) => Some(authority(value)?),
        None => None,
    };
    Ok(tasks::Amendment {
        tracked: match value.field(b"tracked")? {
            Some(Value::String(value)) => match value.as_ref() {
                b"untracked" => Some(tasks::Tracking::Untracked),
                _ => return Err(Problem::Range(b"tracked")),
            },
            Some(value) => Some(tasks::Tracking::Tracked {
                priority: value.object(b"tracked")?.required(b"priority")?.small(b"priority")?,
            }),
            None => None,
        },
        addressed: None,
        instructions: match value.field(b"instructions")? {
            Some(value) => Some(value.text(b"instructions")?),
            None => None,
        },
        release: None,
        spec,
        wake,
        dependencies,
        authority,
        llm: llm(value.field(b"llm")?)?,
        reason: value.required(b"reason")?.text(b"reason")?,
    })
}

fn executor(value: &Value) -> Result<tasks::Executor, Problem> {
    let value = value.object(b"executor")?;
    let kind = value.required(b"kind")?.text(b"kind")?;
    match kind.as_ref() {
        b"agent" => Ok(tasks::Executor::Agent { charter: value.required(b"charter")?.small(b"charter")? }),
        b"procedure" => Ok(tasks::Executor::Procedure {
            connector: value.required(b"connector")?.narrow(b"connector")?,
            code: value.required(b"code")?.small(b"code")?,
        }),
        b"person" => {
            Ok(tasks::Executor::Person(tasks::PersonAddress::Person(value.required(b"person")?.number(b"person")?)))
        }
        b"role" => Ok(tasks::Executor::Person(tasks::PersonAddress::Role(value.required(b"role")?.small(b"role")?))),
        _ => Err(Problem::Range(b"kind")),
    }
}

fn spec(value: &Value) -> Result<tasks::Spec, Problem> {
    let value = value.object(b"spec")?;
    let words = value.required(b"words")?.text(b"words")?;
    let parameters = match value.field(b"parameters")? {
        Some(items) => {
            let items = items.array(b"parameters")?;
            let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
            let mut out = List::with_capacity(capacity);
            for item in items {
                let name = item.required(b"name")?.small(b"name")?;
                let kind = item.required(b"kind")?.text(b"kind")?;
                let parameter = match kind.as_ref() {
                    b"number" => tasks::Parameter::Number { name, value: item.required(b"value")?.number(b"value")? },
                    b"bytes" => tasks::Parameter::Bytes { name, value: item.required(b"value")?.text(b"value")? },
                    b"resource" => tasks::Parameter::Resource {
                        name,
                        connector: item.required(b"connector")?.narrow(b"connector")?,
                        resource: item.required(b"resource")?.number(b"resource")?,
                    },
                    _ => return Err(Problem::Range(b"kind")),
                };
                let Ok(()) = out.push(parameter) else { return Err(Problem::TooLarge) };
            }
            out.into_boxed()
        }
        None => Box::new([]),
    };
    let inputs = match value.field(b"inputs")? {
        Some(items) => numbers(items, b"inputs")?,
        None => Box::new([]),
    };
    Ok(tasks::Spec { words, parameters, inputs })
}

fn contract(value: &Value) -> Result<tasks::Contract, Problem> {
    let value = value.object(b"contract")?;
    let kind = value.required(b"kind")?.text(b"kind")?;
    match kind.as_ref() {
        b"report" => Ok(tasks::Contract::Report { words: value.required(b"words")?.small(b"words")? }),
        b"change" => Ok(tasks::Contract::Change {
            connector: value.required(b"connector")?.narrow(b"connector")?,
            kind: value.required(b"change_kind")?.narrow(b"change_kind")?,
            words: value.required(b"words")?.small(b"words")?,
        }),
        b"verdict" => {
            let choices = value.required(b"choices")?.array(b"choices")?;
            let Ok(capacity) = u32::try_from(choices.len()) else { return Err(Problem::TooLarge) };
            let mut out = List::with_capacity(capacity);
            for choice in choices {
                let Ok(()) = out.push(tasks::Verdict {
                    code: choice.required(b"code")?.small(b"code")?,
                    words: choice.required(b"words")?.small(b"words")?,
                    followups: 0,
                    fields: Box::new([]),
                }) else {
                    return Err(Problem::TooLarge);
                };
            }
            Ok(tasks::Contract::Verdict { choices: out.into_boxed() })
        }
        _ => Err(Problem::Range(b"kind")),
    }
}

pub(crate) fn authority(value: &Value) -> Result<tasks::Authority, Problem> {
    let value = value.object(b"authority")?;
    let tools = tasks::Tools(value.required(b"tools")?.number(b"tools")?);
    let grants = grants_list(value.required(b"grants")?)?;
    let delegation = value.required(b"delegation")?;
    let kinds = delegation.required(b"kinds")?.array(b"kinds")?;
    let Ok(capacity) = u32::try_from(kinds.len()) else { return Err(Problem::TooLarge) };
    let mut permitted = List::with_capacity(capacity);
    for item in kinds {
        let kind = item.required(b"kind")?.text(b"kind")?;
        let executor = match kind.as_ref() {
            b"agent" => tasks::AuthorityExecutor::Charter(item.required(b"number")?.small(b"number")?),
            b"procedure" => tasks::AuthorityExecutor::Procedure(item.required(b"number")?.small(b"number")?),
            b"role" => tasks::AuthorityExecutor::Role(item.required(b"number")?.small(b"number")?),
            _ => return Err(Problem::Range(b"kind")),
        };
        let Ok(()) = permitted.push(executor) else { return Err(Problem::TooLarge) };
    }
    let budget = value.required(b"budget")?;
    let deadline = match budget.field(b"deadline")? {
        Some(deadline) => Some(Wall::from_nanos(deadline.number(b"deadline")?)),
        None => None,
    };
    let Ok(notes) = u8::try_from(value.required(b"notes")?.narrow(b"notes")?) else {
        return Err(Problem::Range(b"notes"));
    };
    let note_resources = match value.field(b"note_resources")? {
        Some(scopes) => note_resources_list(scopes)?,
        None => Box::new([]),
    };
    Ok(tasks::Authority {
        tools,
        grants,
        delegation: tasks::Delegation {
            kinds: permitted.into_boxed(),
            tasks: delegation.required(b"tasks")?.small(b"tasks")?,
            depth: delegation.required(b"depth")?.small(b"depth")?,
        },
        budget: tasks::Budget { spend: budget.required(b"spend")?.number(b"spend")?, deadline },
        notes: tasks::Scopes(notes),
        note_resources,
        // A delegate names no models: the core gives it its creator's (domain/authority.md, 3).
        models: tasks::Models::NONE,
    })
}

fn note_resources_list(value: &Value) -> Result<Box<[tasks::ResourceScope]>, Problem> {
    let scopes = value.array(b"note_resources")?;
    let Ok(capacity) = u32::try_from(scopes.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for scope in scopes {
        let pattern = scope.required(b"pattern")?;
        let terminal = pattern.required(b"terminal")?.text(b"terminal")?;
        let bytes = pattern.required(b"last")?.text(b"last")?;
        let last = match terminal.as_ref() {
            b"exact" => tasks::Last::Exact(bytes),
            b"open" => tasks::Last::Open(bytes),
            _ => return Err(Problem::Range(b"terminal")),
        };
        let Ok(()) = out.push(tasks::ResourceScope {
            connector: scope.required(b"connector")?.narrow(b"connector")?,
            pattern: tasks::Pattern { segments: texts(pattern.required(b"segments")?, b"segments")?, last },
        }) else {
            return Err(Problem::TooLarge);
        };
    }
    Ok(out.into_boxed())
}

fn grants_list(value: &Value) -> Result<Box<[tasks::Grant]>, Problem> {
    let grants = value.array(b"grants")?;
    let Ok(capacity) = u32::try_from(grants.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for grant in grants {
        let segments = texts(grant.required(b"segments")?, b"segments")?;
        let terminal = grant.required(b"terminal")?.text(b"terminal")?;
        let bytes = grant.required(b"last")?.text(b"last")?;
        let last = match terminal.as_ref() {
            b"exact" => tasks::Last::Exact(bytes),
            b"open" => tasks::Last::Open(bytes),
            _ => return Err(Problem::Range(b"terminal")),
        };
        let Ok(()) = out.push(tasks::Grant {
            connector: grant.required(b"connector")?.narrow(b"connector")?,
            kind: grant.required(b"kind")?.narrow(b"kind")?,
            pattern: tasks::Pattern { segments, last },
        }) else {
            return Err(Problem::TooLarge);
        };
    }
    Ok(out.into_boxed())
}

fn wake_policy(value: &Value) -> Result<tasks::WakePolicy, Problem> {
    let value = value.object(b"wake")?;
    Ok(tasks::WakePolicy {
        words: wake_rule(value.required(b"words")?)?,
        notices: wake_rule(value.required(b"notices")?)?,
        news: wake_rule(value.required(b"news")?)?,
        results: match value.required(b"results")?.text(b"results")?.as_ref() {
            b"never" => tasks::ResultsWake::Never,
            b"each" => tasks::ResultsWake::Each,
            b"last_or_failure" => tasks::ResultsWake::LastOrFailure,
            _ => return Err(Problem::Range(b"results")),
        },
        questions: value.required(b"questions")?.boolean(b"questions")?,
        answers: value.required(b"answers")?.boolean(b"answers")?,
        timers: value.required(b"timers")?.boolean(b"timers")?,
    })
}

fn wake_rule(value: &Value) -> Result<tasks::WakeRule, Problem> {
    let value = value.object(b"kind")?;
    match value.required(b"kind")?.text(b"kind")?.as_ref() {
        b"never" => Ok(tasks::WakeRule::Never),
        b"immediate" => Ok(tasks::WakeRule::Immediate),
        b"batch" => Ok(tasks::WakeRule::Batch {
            count: value.required(b"count")?.small(b"count")?,
            age: Duration::from_nanos(value.required(b"age")?.number(b"age")?),
        }),
        _ => Err(Problem::Range(b"kind")),
    }
}

fn numbers(value: &Value, member: &'static [u8]) -> Result<Box<[u64]>, Problem> {
    let items = value.array(member)?;
    let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for item in items {
        let Ok(()) = out.push(item.number(member)?) else { return Err(Problem::TooLarge) };
    }
    Ok(out.into_boxed())
}

fn texts(value: &Value, member: &'static [u8]) -> Result<Box<[Box<[u8]>]>, Problem> {
    let items = value.array(member)?;
    let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for item in items {
        let Ok(()) = out.push(item.text(member)?) else { return Err(Problem::TooLarge) };
    }
    Ok(out.into_boxed())
}

// Model indices name the configured order; account numbers name durable accounts.
fn llm(value: Option<&Value>) -> Result<Option<tasks::LlmChoice>, Problem> {
    let Some(value) = value else { return Ok(None) };
    let value = value.object(b"llm")?;
    let subagents: Box<[tasks::ModelEffort]> = match value.field(b"subagents")? {
        None => Box::new([]),
        Some(values) => {
            let items = values.array(b"subagents")?;
            let count = u32::try_from(items.len()).or(Err(Problem::TooLarge))?;
            if count > tasks::MODELS {
                return Err(Problem::TooLarge);
            }
            let mut out = List::with_capacity(count);
            for item in items {
                if out.push(model_effort(item)?).is_err() {
                    return Err(Problem::TooLarge);
                }
            }
            out.into_boxed()
        }
    };
    Ok(Some(tasks::LlmChoice {
        account: match value.field(b"account")? {
            Some(value) => Some(value.small(b"account")?),
            None => None,
        },
        main: model_effort(value.required(b"main")?)?,
        subagents,
    }))
}
fn model_effort(value: &Value) -> Result<tasks::ModelEffort, Problem> {
    let value = value.object(b"main")?;
    Ok(tasks::ModelEffort {
        model: u8::try_from(value.required(b"model")?.small(b"model")?).or(Err(Problem::Range(b"model")))?,
        effort: match value.field(b"effort")? {
            Some(value) => Some(u8::try_from(value.small(b"effort")?).or(Err(Problem::Range(b"effort")))?),
            None => None,
        },
    })
}
