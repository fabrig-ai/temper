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
    pub(crate) fn defaults(&self, llm: &jig_protocol_config::Llm) -> Defaults {
        match self {
            Self::Stock => defaults(llm),
            Self::Own(own) => {
                let mut policies = Map::with_capacity(own.policies.len());
                for (number, policy) in &own.policies {
                    policies.insert(*number, policy.clone()).expect("counted policies");
                }
                Defaults {
                    charters: own.charters.clone(),
                    templates: own.templates.clone(),
                    policies,
                    rules: own.rules.clone(),
                }
            }
        }
    }
}

const FORGE: u16 = 1;

/// Delegate (a chat's `change` and `delegate`) and Read (the forge's reads), jig's families
/// (jig's `domain/authority.md`); a chat answers from its own forge (protocol/configuration.md, 13.4).
const CHAT_TOOLS: authority::Tools = authority::Tools(1 | 512);

/// The defaults temper passes jig's builder at each start. Each charter offers every
/// model `llm` configures, in its order, the first its default.
/// # Panics
/// If `llm` offers no model; temper's startup refuses that document before calling this.
#[must_use]
pub fn defaults(llm: &jig_protocol_config::Llm) -> Defaults {
    let charters = Box::new([
        charter(1, include_bytes!("stock/chat.txt"), llm),
        charter(2, include_bytes!("stock/coding.txt"), llm),
    ]);
    let templates = Box::new([chat(), goal()]);
    let mut policies = Map::with_capacity(1);
    let mut role_authority = ceiling();
    role_authority.budget.spend = 40;
    policies
        .insert(
            1,
            ConfiguredPolicy {
                policy: authority::Policy {
                    escalation_role: Some(0),
                    ceiling: ceiling(),
                    projections: grants(),
                    period_spend: 100,
                    roles: Box::new([authority::Role {
                        number: 0,
                        authority: role_authority,
                        period_spend: 100,
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
    Defaults {
        charters,
        templates,
        policies,
        rules: authority::Rules {
            ceiling: ceiling(),
            period_spend: 100,
            minimum_run_spend: 1,
            maximum_run_spend: 10,
            implies: authority::Implies::new(Box::default(), 0).expect("empty stock implications"),
            requirements: Box::default(),
        },
    }
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
        spend: 20,
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
fn grants() -> Box<[authority::Grant]> {
    let mut grants = List::with_capacity(13);
    for kind in [1, 2, 3, 4, 7, 8, 9, 10, 11, 12, 13, 16, 17] {
        grants
            .push(authority::Grant {
                connector: FORGE,
                kind,
                pattern: authority::Pattern {
                    segments: Box::new([bytes::copy_of(b"forge")]),
                    last: authority::Last::Open(Box::default()),
                },
            })
            .expect("thirteen stock forge grants");
    }
    grants.into_boxed()
}
fn ceiling() -> authority::Authority {
    authority::Authority {
        tools: CHAT_TOOLS,
        models: authority::Models::ALL,
        grants: grants(),
        delegation: authority::Delegation {
            kinds: Box::new([
                authority::Executor::Charter(1),
                authority::Executor::Charter(2),
                authority::Executor::Procedure(2),
                authority::Executor::Procedure(3),
                authority::Executor::Procedure(4),
            ]),
            tasks: 8,
            depth: 4,
        },
        budget: authority::Budget { spend: 100, deadline: None },
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
