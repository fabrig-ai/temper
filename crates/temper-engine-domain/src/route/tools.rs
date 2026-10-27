//! The decoded engine tools, translated into core or connector vocabulary
//! (domain/root.md, 5.2; jig's `domain/engine.md`, 7.3).
use super::{
    Decision, Domain, Env, EscalationChoice, Limits, MessageForm, Payload, ProposalChoice, ProposedAction, ReplyTo,
    Token, Tool, Work, call_answer, current_proof, delegate_call, forge_route, proposals, relay_call, take_payload,
    tool_kind,
};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_tasks as tasks;

#[expect(
    clippy::too_many_lines,
    reason = "the root translates every temper host tool into core or connector vocabulary"
)]
pub(crate) fn relay_payload(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision,
    reply_to: ReplyTo,
    run: Token,
    attempt: Token,
    body: Token,
) {
    let Some(Payload::Call { key, body }) = take_payload(domain, body) else {
        unreachable!("fleet returns admitted call payload")
    };
    assert!(key.task == run.raw() && key.attempt == attempt.raw(), "fleet call envelope is unchanged");
    assert!(current_proof(domain, key.task, key.attempt), "fleet only relays a current claim");
    // Settings reads enter before replay and admission to the durable call map.
    // The core checks their family and answers denials through its immediate lane.
    if let Tool::Settings { mut read, .. } = body.tool {
        if let core::SettingsRead::Setting { revision, .. } = &mut read
            && *revision == 0
        {
            *revision = domain.core.current.newest();
        }
        domain.work.push(Work::Core(core::Event::NamedAction {
            to: reply_to,
            key,
            action: core::NamedAction::Settings(read),
        }));
        return;
    }
    if let Some(core::CallPart::Effect {
        deadline,
        outcome: None | Some(core::connector::OutboxOutcome::Uncertain | core::connector::OutboxOutcome::Held { .. }),
        ..
    }) = domain.core.call_parts.get(&key)
        && env.wall < *deadline
    {
        let deadline = *deadline;
        domain.work.push(Work::Core(core::Event::EffectStart {
            owner: Token::new(domain.next_effect_owner),
            connector: domain.config.forge_connector,
            origin: core::EffectOrigin::Call { to: reply_to, key, deadline },
        }));
        domain.next_effect_owner = domain.next_effect_owner.checked_add(1).expect("transient effect owner");
        return;
    }
    match call_answer(domain, key) {
        Some(answer) => relay_call(domain, &env.limits, decision, reply_to, answer),
        None => {
            if let Some(kind) = tool_kind(&body.tool)
                && let Some(part) = domain.core.authorize_tool(key, kind)
            {
                domain.work.push(Work::Core(core::Event::NamedAnswer { to: reply_to, key, part }));
                return;
            }
            let tool = if let Tool::ReadNamed { repository, mut read } = body.tool {
                let project = match domain.core.tasks.delegation(key.task) {
                    Some(context) => Some(context.project),
                    None => None,
                };
                let resolved = match project {
                    Some(project) => forge_route::named_repository(domain, project, repository.as_deref()),
                    None => Err(tasks::Refusal::Unknown),
                };
                let Ok(adopted) = resolved else {
                    let repositories = match project {
                        Some(project) => forge_route::read_repositories(domain, project),
                        None => Box::new([]),
                    };
                    super::host_route::read(
                        domain,
                        reply_to,
                        super::CallAnswer::ReadRepositoryRefused { named: repository, repositories },
                    );
                    return;
                };
                if let skein_forge::api::Read::File { max_bytes, .. } | skein_forge::api::Read::Job { max_bytes, .. } =
                    &mut read
                    && *max_bytes == 0
                {
                    *max_bytes = env.limits.forge.client.answer_bytes;
                }
                Tool::ReadForge { repository: adopted.provider, read }
            } else {
                body.tool
            };
            match tool {
                Tool::Settings { .. } => unreachable!("settings reads enter before replay"),
                Tool::ProposeSettings { against, items, reason } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::ProposeSettings { against, items, reason },
                    }));
                }
                Tool::ReadNamed { .. } => unreachable!("named reads resolved before deciding"),
                tool @ (Tool::Change { .. } | Tool::Goal { .. }) => {
                    let expanded = match domain.core.tasks.delegation(key.task) {
                        Some(context) => {
                            if let Tool::Change { words, repository } = tool {
                                forge_route::chat_change(domain, env, &context, words, repository.as_deref())
                            } else {
                                let Tool::Goal { words, advisor } = tool else { unreachable!("goal or change") };
                                super::goal::goal(domain, env, key.task, &context, words, advisor)
                            }
                        }
                        None => Err(tasks::Refusal::Unknown),
                    };
                    let tool = match expanded {
                        Ok(member) => Tool::Delegate { batch: Box::new([member]) },
                        Err(why) => Tool::Rejected(why),
                    };
                    let expanded = super::checked_call(
                        crate::Call { completion: key.completion, position: key.position, tool },
                        &env.limits,
                    );
                    let Tool::Delegate { batch } = expanded.tool else {
                        let Tool::Rejected(why) = expanded.tool else { unreachable!("expanded delegation shape") };
                        domain.work.push(Work::Core(core::Event::NamedAnswer {
                            to: reply_to,
                            key,
                            part: core::CallPart::DelegationRefused(tasks::Problem::new(None, why)),
                        }));
                        return;
                    };
                    delegate_call(domain, env, decision, reply_to, key, batch, false, Box::new([]));
                }
                Tool::Unavailable => {
                    domain.work.push(Work::Core(core::Event::NamedAnswer {
                        to: reply_to,
                        key,
                        part: core::CallPart::Unavailable,
                    }));
                }
                Tool::RejectedNote(why) => domain.work.push(Work::Core(core::Event::NamedAnswer {
                    to: reply_to,
                    key,
                    part: core::CallPart::NoteRefused(why),
                })),
                Tool::Note { entry, recalled } => domain.work.push(Work::Core(core::Event::NamedAction {
                    to: reply_to,
                    key,
                    action: core::NamedAction::Note { entry, recalled },
                })),
                Tool::Recall { by, page } => domain.work.push(Work::Core(core::Event::NamedAction {
                    to: reply_to,
                    key,
                    action: core::NamedAction::Recall { by, page },
                })),
                Tool::Rejected(why) => domain.work.push(Work::Core(core::Event::NamedAnswer {
                    to: reply_to,
                    key,
                    part: core::CallPart::DelegationRefused(tasks::Problem::new(None, why)),
                })),
                Tool::RejectedMessage(why) => domain.work.push(Work::Core(core::Event::NamedAnswer {
                    to: reply_to,
                    key,
                    part: core::CallPart::MessageRefused(tasks::Problem::new(None, why)),
                })),
                Tool::RejectedControl(why) => domain.work.push(Work::Core(core::Event::NamedAnswer {
                    to: reply_to,
                    key,
                    part: core::CallPart::ControlRefused(tasks::Problem::new(None, why)),
                })),
                Tool::RejectedProposal(why) => domain.work.push(Work::Core(core::Event::NamedAnswer {
                    to: reply_to,
                    key,
                    part: core::CallPart::ProposalRefused(tasks::Problem::new(None, why)),
                })),
                Tool::Delegate { batch } => {
                    delegate_call(domain, env, decision, reply_to, key, batch, false, Box::new([]));
                }
                Tool::Propose { action, reason, as_holder } => match action {
                    ProposedAction::Effect { repository, resource, write } => forge_route::effect_call(
                        domain,
                        env,
                        decision,
                        reply_to,
                        key,
                        repository,
                        resource,
                        *write,
                        Some((reason, as_holder)),
                    ),
                    action @ (ProposedAction::Batch(_)
                    | ProposedAction::Amend { .. }
                    | ProposedAction::Widen { .. }
                    | ProposedAction::Release { .. }) => {
                        proposals::propose_call(domain, reply_to, key, action, reason, as_holder);
                    }
                },
                Tool::Decide { proposer, proposal, decision: choice } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::DecideProposal {
                            proposer,
                            proposal,
                            choice: match choice {
                                ProposalChoice::Accept => core::ProposalChoice::Accept,
                                ProposalChoice::Reject { reason } => core::ProposalChoice::Reject { reason },
                                ProposalChoice::Pass => core::ProposalChoice::Pass,
                            },
                        },
                    }));
                }
                Tool::Withdraw { proposal } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::WithdrawProposal { proposal },
                    }));
                }
                Tool::DecideEscalation { task, revision, decision: choice } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::DecideEscalation {
                            task,
                            revision,
                            choice: match choice {
                                EscalationChoice::Release => core::EscalationChoice::Release,
                                EscalationChoice::Reject { reason } => core::EscalationChoice::Reject { reason },
                                EscalationChoice::Pass => core::EscalationChoice::Pass,
                            },
                        },
                    }));
                }
                Tool::Amend { target, revision, amendment } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::Amend { target, revision, amendment },
                    }));
                }
                Tool::Cancel { target, reason } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::Control {
                            target,
                            control: tasks::Control::Cancel { reason, grace: None, message: None },
                        },
                    }));
                }
                Tool::Release { target, revision, again } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::Release { target, revision, again },
                    }));
                }
                Tool::Message { target, form, words } => {
                    let kind = match form {
                        MessageForm::Words => tasks::MessageKind::Words,
                        MessageForm::Question => tasks::MessageKind::Question,
                        MessageForm::Answer { question } => tasks::MessageKind::Answer { question },
                    };
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::Message { target, kind, words },
                    }));
                }
                Tool::Introduce { left, right } => {
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: reply_to,
                        key,
                        action: core::NamedAction::Introduce { left, right },
                    }));
                }
                Tool::Subscribe { kind } => domain.work.push(Work::Core(core::Event::NamedAction {
                    to: reply_to,
                    key,
                    action: core::NamedAction::Subscribe { kind },
                })),
                Tool::SubscribeForge { topic, own_change, paths } => {
                    forge_route::subscribe_call(domain, env, decision, reply_to, key, topic, own_change, paths);
                }
                Tool::ReadForge { repository, read } => {
                    forge_route::read_call(domain, env, decision, reply_to, key, repository, read);
                }
                Tool::EffectForge { repository, resource, write } => {
                    forge_route::effect_call(domain, env, decision, reply_to, key, repository, resource, *write, None);
                }
                Tool::Unsubscribe { subscription } => {
                    let token = reply_to.into_token();
                    if let Some(topic) = domain.forge.subscription(key.task, subscription) {
                        assert!(
                            domain.forge_unsubscribing.insert(token, (key.task, topic)) == Ok(None),
                            "one connector unsubscribe"
                        );
                    }
                    domain.work.push(Work::Core(core::Event::NamedAction {
                        to: ReplyTo::new(token),
                        key,
                        action: core::NamedAction::Unsubscribe { subscription },
                    }));
                }
            }
        }
    }
}
