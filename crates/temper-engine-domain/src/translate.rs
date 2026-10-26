//! Total vocabulary and configuration translations (jig's domain/root.md, 6).
use crate::boundary::{Call, EscalationChoice, MessageForm, ProposalChoice, ProposedAction, Tool};
use crate::domain::Domain;
use crate::limits::Limits;
use crate::route::{LandingRule, host_route};
use crate::{CallAnswer, Decision};
use alloc::boxed::Box;
use jig_domain_notes as notes;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Map, ReplyTo};
use temper_engine_domain_forge as forge;

/// Startup configuration, supplied once by the shell (domain/root.md, 6):
/// the core's whole, as jig's applications hold it, and temper's own.
#[derive(Debug)]
pub struct Config {
    /// The core's rules, charters, owners and projects; its one connector is
    /// the forge's; whether the workers run in the engine's process is
    /// configuration too, set by an in-process hosts' listener: standalone,
    /// they do (domain/root.md, 2).
    pub core: jig_domain_core::Config,
    /// Forge landing requirements and their connector-owned judge parameters.
    pub landing: LandingPolicy,
    /// Forge writer identities and this deployment's branch namespace.
    pub forge: skein_forge_client::Config,
    /// Each forge's git identity, an application credential (D10;
    /// protocol/credentials.md, "each repository to a git identity").
    pub identities: Box<[forge::GitIdentity]>,
    /// The charter the forge's procedures start their agent work under, one
    /// of the core's charters (domain/root.md, 6).
    pub forge_charter: u32,
    pub forges: Box<[forge::Configured]>,
    pub under_box: bool,
    pub demo: Option<forge::DemoConfig>,
}

/// What the root keeps of its configuration once its children hold theirs.
#[derive(Debug)]
pub(crate) struct RootConfig {
    pub(crate) forge_connector: u16,
    pub(crate) forge_charter: u32,
}

/// Typed forge landing policy retained by the application for policy snapshots.
#[derive(Debug)]
pub struct LandingPolicy {
    pub deployment: Box<[LandingRule]>,
    pub projects: Map<u32, Box<[LandingRule]>>,
}

pub(crate) fn environment_forge(env: &Env<Limits>) -> Env<forge::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.forge }
}

pub(crate) fn checked_call(mut body: Call, limits: &Limits) -> Call {
    let note_valid = match &body.tool {
        Tool::Note { entry, .. } => notes::valid_new(entry, &limits.notes),
        Tool::Recall { by, .. } => notes::valid_recall(by, &limits.notes),
        Tool::Goal { .. }
        | Tool::Settings { .. }
        | Tool::ProposeSettings { .. }
        | Tool::Change { .. }
        | Tool::Delegate { .. }
        | Tool::Message { .. }
        | Tool::Amend { .. }
        | Tool::Cancel { .. }
        | Tool::Release { .. }
        | Tool::Decide { .. }
        | Tool::DecideEscalation { .. }
        | Tool::Withdraw { .. }
        | Tool::Propose { .. }
        | Tool::Introduce { .. }
        | Tool::Subscribe { .. }
        | Tool::SubscribeForge { .. }
        | Tool::ReadNamed { .. }
        | Tool::ReadForge { .. }
        | Tool::EffectForge { .. }
        | Tool::Unsubscribe { .. }
        | Tool::Unavailable
        | Tool::Rejected(_)
        | Tool::RejectedMessage(_)
        | Tool::RejectedControl(_)
        | Tool::RejectedProposal(_)
        | Tool::RejectedNote(_) => true,
    };
    if !note_valid {
        body.tool = Tool::RejectedNote(notes::Refusal::Oversized);
    }
    if let Some(why) = call_shape(&body.tool, &limits.tasks) {
        body.tool = match body.tool {
            Tool::Message { .. } => Tool::RejectedMessage(why),
            Tool::Amend { .. } | Tool::Cancel { .. } | Tool::Release { .. } | Tool::DecideEscalation { .. } => {
                Tool::RejectedControl(why)
            }
            Tool::Propose { .. } | Tool::Decide { .. } | Tool::Withdraw { .. } => Tool::RejectedProposal(why),
            Tool::Note { .. } | Tool::Recall { .. } => Tool::RejectedNote(jig_domain_notes::Refusal::Oversized),
            Tool::Goal { .. }
            | Tool::Settings { .. }
            | Tool::ProposeSettings { .. }
            | Tool::Change { .. }
            | Tool::Delegate { .. }
            | Tool::Introduce { .. }
            | Tool::Subscribe { .. }
            | Tool::SubscribeForge { .. }
            | Tool::ReadNamed { .. }
            | Tool::ReadForge { .. }
            | Tool::EffectForge { .. }
            | Tool::Unsubscribe { .. }
            | Tool::Rejected(_)
            | Tool::RejectedMessage(_)
            | Tool::RejectedControl(_)
            | Tool::RejectedProposal(_)
            | Tool::RejectedNote(_)
            | Tool::Unavailable => Tool::Rejected(why),
        };
    }
    body
}

pub(crate) fn relay_call(
    domain: &mut Domain,
    limits: &Limits,
    decision: &mut Decision,
    to: ReplyTo,
    answer: CallAnswer,
) {
    host_route::render(domain, limits, decision, to, answer);
}

pub(crate) fn call_needs_input(tool: &Tool) -> bool {
    match tool {
        Tool::Goal { .. }
        | Tool::Settings { .. }
        | Tool::ProposeSettings { .. }
        | Tool::Change { .. }
        | Tool::Unavailable
        | Tool::Rejected(_)
        | Tool::RejectedMessage(_)
        | Tool::RejectedControl(_)
        | Tool::RejectedProposal(_)
        | Tool::RejectedNote(_)
        | Tool::Note { .. }
        | Tool::Recall { .. }
        | Tool::Message { .. }
        | Tool::Introduce { .. }
        | Tool::Subscribe { .. }
        | Tool::SubscribeForge { .. }
        | Tool::ReadNamed { .. }
        | Tool::ReadForge { .. }
        | Tool::EffectForge { .. }
        | Tool::Unsubscribe { .. }
        | Tool::Cancel { .. }
        | Tool::Release { .. }
        | Tool::Amend { .. }
        | Tool::Propose { .. }
        | Tool::Decide { .. }
        | Tool::Withdraw { .. }
        | Tool::DecideEscalation { .. } => false,
        Tool::Delegate { batch } => {
            for member in batch {
                if !member.spec.inputs.is_empty() {
                    return true;
                }
            }
            false
        }
    }
}

#[expect(clippy::too_many_lines, reason = "one tool shape entrance preflights every bounded call")]
pub(crate) fn call_shape(tool: &Tool, limits: &tasks::Limits) -> Option<tasks::Refusal> {
    match tool {
        Tool::Propose { action, reason, .. } => {
            if reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize") {
                return Some(tasks::Refusal::Read);
            }
            match action {
                ProposedAction::Batch(batch) => {
                    if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
                        return Some(tasks::Refusal::Batch);
                    }
                    for member in batch {
                        if !tasks::valid_spec(limits, &member.spec) || !member.spec.inputs.is_empty() {
                            return Some(tasks::Refusal::Spec);
                        }
                        if !tasks::valid_contract(limits, &member.contract)
                            || !tasks::valid_authority(limits, &member.authority)
                            || member.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
                        {
                            return Some(tasks::Refusal::AuthorityShape);
                        }
                    }
                    None
                }
                ProposedAction::Amend { amendment, .. } => {
                    if amendment.reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize") {
                        Some(tasks::Refusal::Read)
                    } else if match &amendment.authority {
                        Some(authority) => !tasks::valid_authority(limits, authority),
                        None => true,
                    } {
                        Some(tasks::Refusal::AuthorityShape)
                    } else {
                        None
                    }
                }
                ProposedAction::Widen { authority, .. } => {
                    if tasks::valid_authority(limits, authority) {
                        None
                    } else {
                        Some(tasks::Refusal::AuthorityShape)
                    }
                }
                ProposedAction::Effect { .. } | ProposedAction::Release { .. } => None,
            }
        }
        Tool::Decide { decision, .. } => match decision {
            ProposalChoice::Reject { reason }
                if reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize") =>
            {
                Some(tasks::Refusal::Read)
            }
            ProposalChoice::Accept | ProposalChoice::Reject { .. } | ProposalChoice::Pass => None,
        },
        Tool::DecideEscalation { decision, .. } => match decision {
            EscalationChoice::Reject { reason }
                if reason.len() > usize::try_from(limits.result_bytes).expect("u32 fits usize") =>
            {
                Some(tasks::Refusal::Read)
            }
            EscalationChoice::Release | EscalationChoice::Reject { .. } | EscalationChoice::Pass => None,
        },
        Tool::Goal { .. }
        | Tool::Settings { .. }
        | Tool::ProposeSettings { .. }
        | Tool::Change { .. }
        | Tool::Withdraw { .. }
        | Tool::Unavailable
        | Tool::Rejected(_)
        | Tool::RejectedMessage(_)
        | Tool::RejectedControl(_)
        | Tool::RejectedProposal(_)
        | Tool::RejectedNote(_)
        | Tool::Note { .. }
        | Tool::Recall { .. }
        | Tool::Introduce { .. }
        | Tool::Subscribe { .. }
        | Tool::SubscribeForge { .. }
        | Tool::ReadNamed { .. }
        | Tool::ReadForge { .. }
        | Tool::EffectForge { .. }
        | Tool::Unsubscribe { .. }
        | Tool::Release { .. } => None,
        Tool::Cancel { reason, .. } => {
            if reason.len() > usize::try_from(limits.result_bytes).expect("u32 fits usize") {
                Some(tasks::Refusal::Read)
            } else {
                None
            }
        }
        Tool::Amend { amendment, .. } => {
            if amendment.reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize") {
                return Some(tasks::Refusal::Read);
            }
            if let Some(spec) = &amendment.spec
                && (!tasks::valid_spec(limits, spec) || !spec.inputs.is_empty())
            {
                return Some(tasks::Refusal::Spec);
            }
            if let Some(authority) = &amendment.authority
                && !tasks::valid_authority(limits, authority)
            {
                return Some(tasks::Refusal::AuthorityShape);
            }
            if let Some(dependencies) = &amendment.dependencies
                && dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
            {
                return Some(tasks::Refusal::Dependencies);
            }
            None
        }
        Tool::Message { form, words, .. } => {
            if words.is_empty() || words.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize") {
                return Some(tasks::Refusal::Read);
            }
            match form {
                MessageForm::Answer { question } if *question == 0 => return Some(tasks::Refusal::Read),
                MessageForm::Words | MessageForm::Question | MessageForm::Answer { .. } => {}
            }
            None
        }
        Tool::Delegate { batch } => {
            if batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
                return Some(tasks::Refusal::Batch);
            }
            for member in batch {
                if !tasks::valid_spec(limits, &member.spec) {
                    return Some(tasks::Refusal::Spec);
                }
                if !tasks::valid_contract(limits, &member.contract) {
                    return Some(tasks::Refusal::Contract);
                }
                if !tasks::valid_authority(limits, &member.authority) {
                    return Some(tasks::Refusal::AuthorityShape);
                }
                if member.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize") {
                    return Some(tasks::Refusal::Dependencies);
                }
            }
            None
        }
    }
}

pub(crate) fn tool_kind(tool: &Tool) -> Option<jig_domain_core::ToolKind> {
    Some(match tool {
        Tool::Settings { .. } | Tool::ProposeSettings { .. } => jig_domain_core::ToolKind::Settings,
        Tool::Goal { .. } | Tool::Change { .. } | Tool::Delegate { .. } => jig_domain_core::ToolKind::Delegate,
        Tool::Message { target, .. } => jig_domain_core::ToolKind::Message { target: *target },
        Tool::Introduce { left, .. } => jig_domain_core::ToolKind::Message { target: *left },
        Tool::Amend { .. } | Tool::Cancel { .. } | Tool::Release { .. } => jig_domain_core::ToolKind::Control,
        Tool::Decide { .. } | Tool::Withdraw { .. } | Tool::DecideEscalation { .. } => {
            jig_domain_core::ToolKind::Decide
        }
        Tool::Propose { .. } => jig_domain_core::ToolKind::Propose,
        Tool::Subscribe { .. } | Tool::SubscribeForge { .. } | Tool::Unsubscribe { .. } => {
            jig_domain_core::ToolKind::Subscribe
        }
        Tool::EffectForge { .. } => jig_domain_core::ToolKind::Effect,
        Tool::Note { entry, .. } => jig_domain_core::ToolKind::Note { scope: entry.scope.clone() },
        Tool::Recall { by, .. } => jig_domain_core::ToolKind::Recall { by: by.clone() },
        Tool::ReadNamed { .. } | Tool::ReadForge { .. } => jig_domain_core::ToolKind::Read,
        Tool::Unavailable
        | Tool::Rejected(_)
        | Tool::RejectedMessage(_)
        | Tool::RejectedControl(_)
        | Tool::RejectedProposal(_)
        | Tool::RejectedNote(_) => return None,
    })
}

pub(crate) const fn named_call(key: crate::CallKey) -> forge::NamedCall {
    forge::NamedCall { task: key.task, attempt: key.attempt, completion: key.completion, position: key.position }
}

pub(crate) const fn core_call(key: forge::NamedCall) -> crate::CallKey {
    crate::CallKey { task: key.task, attempt: key.attempt, completion: key.completion, position: key.position }
}

pub(crate) fn connector_answer(domain: &Domain, key: crate::CallKey) -> Option<CallAnswer> {
    let payload = domain.forge.named_answer(named_call(key))?;
    Some(call_payload_answer(payload))
}

pub(crate) fn call_payload_answer(payload: forge::CallPayload) -> CallAnswer {
    match payload {
        forge::CallPayload::Effect { entry, deadline, outcome } => CallAnswer::ForgeEffect { entry, deadline, outcome },
        forge::CallPayload::Refused(error) => CallAnswer::ForgeEffectRefused(error),
        forge::CallPayload::Read(answer) => CallAnswer::ForgeRead(answer),
    }
}

#[expect(clippy::result_large_err, reason = "a refused answer returns to its caller whole")]
pub(crate) fn keep_connector_answer(
    domain: &mut Domain,
    key: crate::CallKey,
    answer: CallAnswer,
) -> Result<Option<CallAnswer>, CallAnswer> {
    let payload = match answer {
        CallAnswer::ForgeEffect { entry, deadline, outcome } => forge::CallPayload::Effect { entry, deadline, outcome },
        CallAnswer::ForgeEffectRefused(error) => forge::CallPayload::Refused(error),
        CallAnswer::ForgeRead(answer) => forge::CallPayload::Read(answer),
        answer @ (CallAnswer::ForgeEffectDenied { .. }
        | CallAnswer::ToolDenied { .. }
        | CallAnswer::EscalationDecided { .. }
        | CallAnswer::EscalationRefused(_)
        | CallAnswer::Proposed { .. }
        | CallAnswer::ProposalDecided { .. }
        | CallAnswer::ProposalRefused(_)
        | CallAnswer::Again { .. }
        | CallAnswer::PassedUp { .. }
        | CallAnswer::Revised { .. }
        | CallAnswer::Amended { .. }
        | CallAnswer::Moved { .. }
        | CallAnswer::Controlled
        | CallAnswer::EffectRefused(_)
        | CallAnswer::ControlRefused(_)
        | CallAnswer::ControlDenied { .. }
        | CallAnswer::Sent { .. }
        | CallAnswer::Introduced
        | CallAnswer::MessageRefused(_)
        | CallAnswer::Subscribed { .. }
        | CallAnswer::Unsubscribed
        | CallAnswer::SubscriptionRefused(_)
        | CallAnswer::Delegated(_)
        | CallAnswer::DelegationDenied { .. }
        | CallAnswer::ReadRepositoryRefused { .. }
        | CallAnswer::ChangeRefused { .. }
        | CallAnswer::DelegationRefused(_)
        | CallAnswer::NoteWritten { .. }
        | CallAnswer::NoteRecalled { .. }
        | CallAnswer::NoteRefused(_)
        | CallAnswer::SettingsRefused(_)
        | CallAnswer::Unavailable) => return Err(answer),
    };
    match domain.forge.keep_named_answer(&domain.limits.forge, named_call(key), payload) {
        Ok(Some(payload)) => Ok(Some(call_payload_answer(payload))),
        Ok(None) => Ok(None),
        Err(payload) => Err(call_payload_answer(payload)),
    }
}
