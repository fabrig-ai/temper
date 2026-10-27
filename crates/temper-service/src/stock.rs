//! temper's stock settings, the binary's defaults (protocol/configuration.md, 13.4 and 13.6).
#![expect(
    clippy::disallowed_macros,
    reason = "stock instructions are held in the binary (protocol/configuration.md, 13.4)"
)]
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core::{Charter, ConfiguredPolicy, RunPolicy, Template, settings::Defaults};
use jig_domain_people::{TemplateBudget, TemplateDeadline, TemplateExecutor};
use jig_domain_tasks::{Contract, EndedBy, Flags, Retention};
use skein_lib::{Duration, List, Map, bytes};

/// Settings defaults supplied at a start (protocol/configuration.md, 13.4).
/// The binary uses Stock; an in-process test world may pass its own values.
#[derive(Debug)]
pub enum Given {
    Stock,
    Own(Box<Defaults>),
}
impl Given {
    pub(crate) fn defaults(&self, llm: &jig_protocol_config::Llm) -> Option<Defaults> {
        match self {
            Self::Stock => checked(llm),
            Self::Own(own) => {
                let mut policies = Map::with_capacity(own.policies.len());
                for (number, policy) in &own.policies {
                    policies.insert(*number, policy.clone()).expect("counted policies");
                }
                Some(Defaults {
                    charters: own.charters.clone(),
                    templates: own.templates.clone(),
                    policies,
                    rules: own.rules.clone(),
                })
            }
        }
    }
}

const FORGE: u16 = 1;

/// Each project's funding span, from its first period (protocol/configuration.md, 13.4).
pub const PERIOD_SPAN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Delegate (`change` and `delegate`), Message (delegates' questions and answers)
/// and Read (the forge's reads), jig's families (jig's `domain/authority.md`);
/// protocol/configuration.md, 13.4.
const CHAT_TOOLS: authority::Tools = authority::Tools(1 | 2 | 512);

/// The defaults temper passes jig's builder at each start. Each charter offers every
/// model `llm` configures, in its order, the first its default.
/// # Panics
/// If `llm` offers no model or its budgets overflow; startup checks both before calling this.
#[must_use]
pub fn defaults(llm: &jig_protocol_config::Llm) -> Defaults {
    checked(llm).expect("startup checked the default model and its stock budgets")
}

/// Checked stock construction lets startup refuse unrepresentable model-based budgets.
pub(crate) fn checked(llm: &jig_protocol_config::Llm) -> Option<Defaults> {
    llm.models.first()?;
    let mut charters = Box::new([
        charter(1, include_bytes!("stock/chat.txt"), llm),
        charter(2, include_bytes!("stock/coding.txt"), llm),
        reviewer(llm),
    ]);
    // jig prices the costliest completion; one reserve turn plus one is 2C.
    // Its existing admission helper keeps the stock on that same rounding rule.
    let minimum_run = jig_domain_core::reserve_floor(&charters.first()?.policy)?;
    let run = minimum_run.checked_mul(2)?; // K = 4C.
    let chat_spend = run.checked_mul(24)?;
    let ceiling_spend = run.checked_mul(48)?;
    for charter in charters.iter_mut() {
        charter.spend = run;
    }
    let templates = Box::new([chat(), goal()]);
    let mut policies = Map::with_capacity(1);
    let mut role_authority = ceiling(ceiling_spend);
    role_authority.budget.spend = chat_spend;
    policies
        .insert(
            1,
            ConfiguredPolicy {
                policy: authority::Policy {
                    escalation_role: Some(0),
                    ceiling: ceiling(ceiling_spend),
                    projections: grants(),
                    period_spend: ceiling_spend,
                    roles: Box::new([authority::Role {
                        number: 0,
                        authority: role_authority,
                        period_spend: ceiling_spend,
                        requests: authority::Requests(511),
                        decides: authority::Proposals(31),
                    }]),
                    requirements: Box::default(),
                },
                permissions: Box::new([jig_domain_people::PermissionRole { connector: FORGE, permission: 1, role: 0 }]),
                templates: Box::default(),
            },
        )
        .expect("one stock policy");
    Some(Defaults {
        charters,
        templates,
        policies,
        rules: authority::Rules {
            ceiling: ceiling(ceiling_spend),
            period_spend: ceiling_spend,
            minimum_run_spend: minimum_run,
            maximum_run_spend: run,
            implies: authority::Implies::new(Box::default(), 0).expect("empty stock implications"),
            requirements: Box::default(),
        },
    })
}

fn charter(number: u32, instructions: &[u8], llm: &jig_protocol_config::Llm) -> Charter {
    let model = llm.models.first().expect("parsed llm offers at least one model").clone();
    let mut alternatives = List::with_capacity(u32::try_from(llm.models.len().saturating_sub(1)).expect("model count"));
    for alternative in llm.models.iter().skip(1) {
        alternatives.push(alternative.clone()).expect("one per remaining configured model");
    }
    Charter {
        number,
        hosts: jig_domain_fleet::Kinds::Workers,
        spend: 0,
        policy: RunPolicy {
            instructions: bytes::copy_of(instructions),
            model,
            alternatives: alternatives.into_boxed(),
            waiting: Duration::from_millis(10_000),
            resume: true,
            turns: 8,
            time: Duration::from_millis(120_000),
            reserve_turns: 1,
            reserve_time: Duration::from_millis(60_000),
            inspect: true,
            modify: true,
            shell: false,
            agents: true,
            call_timeout: Duration::from_millis(10_000),
        },
    }
}
fn reviewer(llm: &jig_protocol_config::Llm) -> Charter {
    let mut value = charter(4, include_bytes!("stock/review.txt"), llm);
    value.policy.modify = false;
    value.policy.agents = false;
    value
}
fn grants() -> Box<[authority::Grant]> {
    let mut grants = List::with_capacity(10);
    for kind in [1, 2, 3, 4, 7, 8, 10, 11, 12, 13] {
        grants
            .push(authority::Grant {
                connector: FORGE,
                kind,
                pattern: authority::Pattern {
                    segments: Box::new([bytes::copy_of(b"forge")]),
                    last: authority::Last::Open(Box::default()),
                },
            })
            .expect("ten stock forge grants");
    }
    grants.into_boxed()
}
fn ceiling(ceiling_spend: u64) -> authority::Authority {
    authority::Authority {
        tools: CHAT_TOOLS,
        models: authority::Models::ALL,
        grants: grants(),
        delegation: authority::Delegation {
            kinds: Box::new([
                authority::Executor::Charter(1),
                authority::Executor::Charter(2),
                authority::Executor::Charter(4),
                authority::Executor::Procedure(2),
            ]),
            tasks: 14,
            depth: 4,
        },
        budget: authority::Budget { spend: ceiling_spend, deadline: None },
        notes: authority::Scopes(0),
        note_resources: Box::default(),
    }
}
fn chat() -> Template {
    Template {
        number: 1,
        name: bytes::copy_of(b"chat"),
        executor: TemplateExecutor::Charter(1),
        contract: Contract::Report { words: 32 },
        shallower: 1,
        budget: TemplateBudget::AsAllowed { cap: None },
        deadline: TemplateDeadline::None,
        tracked: None,
        flags: Flags { ended_by: EndedBy::Requester, conversation: true, listed: false, retention: Retention::Kept },
        label: 1,
        grace: Some(Duration::from_millis(5000)),
        llm: None,
    }
}
fn goal() -> Template {
    Template {
        number: 2,
        name: bytes::copy_of(b"goal"),
        executor: TemplateExecutor::Chosen,
        contract: Contract::Report { words: 32 },
        shallower: 1,
        budget: TemplateBudget::Given,
        deadline: TemplateDeadline::Given,
        tracked: Some(1),
        flags: Flags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        label: 2,
        grace: None,
        llm: None,
    }
}
