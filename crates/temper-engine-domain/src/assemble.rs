//! Assignments and briefs assembled from their owners' parts (jig's domain/root.md, 7).
use crate::Decision;
use crate::boundary::{BriefBody, BriefConnector, BriefKind, BriefSection, ForgeBriefKind, Payload, Work};
use crate::domain::Domain;
use crate::limits::{BriefBudgets, Limits};
use crate::route::forge_route::unheld_run_branch;
use jig_domain_brief as brief;
use skein_lib::{Env, Id, List, Queue, Token};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as forge_change;

pub(crate) fn brief_outputs(
    domain: &mut Domain,
    env: &Env<Limits>,
    _decision: &mut Decision,
    out: &mut Queue<brief::GatherRequest>,
) {
    for _ in 0..out.len() {
        match out.pop().expect("brief output count") {
            brief::GatherRequest::Gather { connector: 0, section, budget } => {
                if domain.brief_connectors.get(Id::from_token(section)).is_none() {
                    continue;
                }
                domain.work.push(Work::Forge(forge::Event::GatherPlanned {
                    section,
                    parts: env.limits.brief.parts,
                    bytes: budget,
                    ci_budget: env.limits.brief.budgets.ci,
                }));
            }
            brief::GatherRequest::CutTo { connector: 0, section, size } => {
                if let Some(row) = domain.brief_connectors.get_mut(Id::from_token(section)) {
                    row.cutting = true;
                    domain.work.push(Work::Forge(forge::Event::CutBrief { section, bytes: size }));
                }
            }
            brief::GatherRequest::Drop { connector: 0, section } => {
                let id = Id::from_token(section);
                if domain.brief_connectors.get(id).is_some() {
                    domain.brief_connectors.retire(id);
                    domain.work.push(Work::Forge(forge::Event::DropBrief { section }));
                }
            }
            brief::GatherRequest::Gather { .. }
            | brief::GatherRequest::CutTo { .. }
            | brief::GatherRequest::Drop { .. } => unreachable!("temper's sole connector is numbered zero"),
            brief::GatherRequest::Complete { brief, order } => {
                let task = brief.raw();
                let mut sections = List::with_capacity(u32::try_from(order.len()).expect("bounded section count"));
                let mut missing_owner = false;
                for placed in order {
                    let section = match placed {
                        brief::GatherPlaced::Core { kind, text } => {
                            BriefSection { kind: BriefKind::Core(kind), body: BriefBody::Text(text) }
                        }
                        brief::GatherPlaced::CoreMissing { kind, why } => {
                            BriefSection { kind: BriefKind::Core(kind), body: BriefBody::Missing(why) }
                        }
                        brief::GatherPlaced::Connector { token, size, .. } => {
                            let id = Id::from_token(token);
                            let Some(row) = domain.brief_connectors.get(id) else {
                                missing_owner = true;
                                continue;
                            };
                            let kind = row.kind;
                            domain.brief_connectors.retire(id);
                            let bytes = domain.forge.take_brief(token);
                            match bytes {
                                Some(bytes) if bytes.len() == usize::try_from(size).expect("bounded section") => {
                                    BriefSection { kind: BriefKind::Forge(kind), body: BriefBody::Text(bytes) }
                                }
                                Some(_) | None => {
                                    missing_owner = true;
                                    continue;
                                }
                            }
                        }
                        brief::GatherPlaced::Missing { kind, why, .. } => BriefSection {
                            kind: BriefKind::Forge(numbered_brief_kind(kind)),
                            body: BriefBody::Missing(why),
                        },
                    };
                    sections.push(section).expect("bounded brief sections");
                }
                if missing_owner {
                    domain.work.push(Work::Core(jig_domain_core::Event::BriefAssembled { task, ready: false }));
                    continue;
                }
                assert!(domain.brief_sections.insert(task, sections.into_boxed()) == Ok(None), "one brief assembly");
                domain.work.push(Work::Core(jig_domain_core::Event::BriefAssembled { task, ready: true }));
            }
            brief::GatherRequest::Failed { .. } | brief::GatherRequest::Refused { .. } => {
                unreachable!("the core abandons refused brief preparation")
            }
        }
    }
}

pub(crate) fn numbered_brief_kind(kind: u16) -> ForgeBriefKind {
    match kind {
        1 => ForgeBriefKind::Ci,
        2 => ForgeBriefKind::Reviews,
        3 => ForgeBriefKind::Pull,
        4 => ForgeBriefKind::Home,
        _ => unreachable!("temper's forge connector uses four section kinds"),
    }
}

pub(crate) fn take_payload(domain: &mut Domain, token: Token) -> Option<Payload> {
    let id = Id::from_token(token);
    let payload = domain.payloads.get_mut(id)?.take()?;
    domain.payloads.retire(id);
    Some(payload)
}

fn plan_home_access(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    parent: Option<u64>,
    wanted: &mut List<brief::Planned>,
) -> bool {
    let Some(row) = domain.core.tasks.task(task) else { return true };
    let Some(home) = domain.forge.home(row.project) else { return true };
    if unheld_run_branch(domain, &env.limits, task, home) != Some(true) {
        return true;
    }
    let inherited = match parent {
        Some(parent) => match domain.forge.change(parent) {
            Some(change) => match change.delegate {
                Some((child, _)) => child == task,
                None => false,
            },
            None => false,
        },
        None => false,
    };
    if inherited {
        return true;
    }
    // Required context takes a slot before optional core context. If every
    // slot is already required, preparation fails rather than omitting Home.
    let mut optional = None;
    for (index, section) in wanted.iter().enumerate() {
        if !section.required() {
            optional = Some(index);
        }
    }
    if wanted.room() == 0 && optional.is_none() {
        return false;
    }
    let source = forge::BriefSource::Home { repository: home.provider };
    let kind = forge_brief_kind(source);
    let token = domain
        .brief_connectors
        .insert(BriefConnector { task, kind, cutting: false })
        .expect("reserved connector section room")
        .token();
    domain.work.push(Work::Forge(forge::Event::PlanBrief { section: token, source }));
    let section = brief::Planned::Connector {
        connector: 0,
        kind: forge_kind_number(kind),
        token,
        size: 0,
        limit: forge_brief_budget(kind, &domain.limits.brief.budgets),
        priority: 5,
        required: true,
    };
    if wanted.room() > 0 {
        wanted.push(section).expect("home section room");
    } else {
        *wanted.get_mut(u32::try_from(optional.expect("optional slot")).expect("section index")).expect("section") =
            section;
    }
    true
}

pub(crate) fn finish_brief_plan(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    parent: Option<u64>,
    mut wanted: List<brief::Planned>,
) {
    if !plan_home_access(domain, env, task, parent, &mut wanted) {
        domain.work.push(Work::Core(jig_domain_core::Event::BriefAssembled { task, ready: false }));
        return;
    }
    if let Some(parent) = parent
        && let Some(row) = domain.forge.change(parent)
        && let Some((child, _)) = row.delegate
        && child == task
        && let Some(number) = row.pull
        && let Some(head) = row.change.last_head
        && wanted.room() > 0
    {
        let item = forge::BriefItem { repository: row.repository.repository, number };
        let source = match row.delegate.expect("matched delegate").1 {
            forge_change::Delegate::Repair(forge_change::Repair::Gate(_)) => {
                forge::BriefSource::Reviews { item, head: forge::BriefCommit(head) }
            }
            forge_change::Delegate::Repair(
                forge_change::Repair::Ci | forge_change::Repair::Semantic | forge_change::Repair::Landing { .. },
            ) => forge::BriefSource::Ci { item, head: forge::BriefCommit(head) },
            forge_change::Delegate::Resolve { .. } => forge::BriefSource::Pull { item, head: forge::BriefCommit(head) },
            forge_change::Delegate::Produce | forge_change::Delegate::Gate { .. } => {
                forge::BriefSource::Pull { item, head: forge::BriefCommit(head) }
            }
        };
        let kind = forge_brief_kind(source);
        let token = domain
            .brief_connectors
            .insert(BriefConnector { task, kind, cutting: false })
            .expect("reserved connector section room")
            .token();
        domain.work.push(Work::Forge(forge::Event::PlanBrief { section: token, source }));
        wanted
            .push(brief::Planned::Connector {
                connector: 0,
                kind: forge_kind_number(kind),
                token,
                size: 0,
                limit: forge_brief_budget(kind, &domain.limits.brief.budgets),
                priority: 5,
                required: true,
            })
            .expect("connector section room");
        let semantic = match row.delegate {
            Some((_, forge_change::Delegate::Repair(forge_change::Repair::Semantic))) => true,
            Some((
                _,
                forge_change::Delegate::Repair(
                    forge_change::Repair::Ci | forge_change::Repair::Gate(_) | forge_change::Repair::Landing { .. },
                )
                | forge_change::Delegate::Resolve { .. }
                | forge_change::Delegate::Gate { .. }
                | forge_change::Delegate::Produce,
            ))
            | None => false,
        };
        if semantic && wanted.room() > 0 {
            let source = forge::BriefSource::Pull { item, head: forge::BriefCommit(head) };
            let kind = forge_brief_kind(source);
            let token = domain
                .brief_connectors
                .insert(BriefConnector { task, kind, cutting: false })
                .expect("reserved connector section room")
                .token();
            domain.work.push(Work::Forge(forge::Event::PlanBrief { section: token, source }));
            wanted
                .push(brief::Planned::Connector {
                    connector: 0,
                    kind: forge_kind_number(kind),
                    token,
                    size: 0,
                    limit: forge_brief_budget(kind, &domain.limits.brief.budgets),
                    priority: 6,
                    required: true,
                })
                .expect("semantic update section room");
        }
    }
    domain.work.push(Work::Brief(brief::GatherEvent::Plan {
        brief: Token::new(task),
        budget: domain.limits.brief.brief_bytes,
        deadline: env.now.saturating_add(domain.limits.brief.gather),
        sections: wanted.into_boxed(),
    }));
}

pub(crate) fn forge_brief_kind(source: forge::BriefSource) -> ForgeBriefKind {
    match source {
        forge::BriefSource::Home { .. } => ForgeBriefKind::Home,
        forge::BriefSource::Ci { .. } => ForgeBriefKind::Ci,
        forge::BriefSource::Reviews { .. } => ForgeBriefKind::Reviews,
        forge::BriefSource::Pull { .. } => ForgeBriefKind::Pull,
    }
}

pub(crate) fn forge_kind_number(kind: ForgeBriefKind) -> u16 {
    match kind {
        ForgeBriefKind::Home => 4,
        ForgeBriefKind::Ci => 1,
        ForgeBriefKind::Reviews => 2,
        ForgeBriefKind::Pull => 3,
    }
}

pub(crate) fn forge_brief_budget(kind: ForgeBriefKind, budgets: &BriefBudgets) -> u32 {
    match kind {
        ForgeBriefKind::Home => forge::HOME_SECTION_BYTES,
        ForgeBriefKind::Ci => budgets.ci,
        ForgeBriefKind::Reviews => budgets.reviews,
        ForgeBriefKind::Pull => budgets.pull,
    }
}

#[cfg(test)]
mod tests {
    use super::{ForgeBriefKind, forge_brief_budget};
    use temper_engine_domain_forge as forge;

    #[test]
    fn the_home_section_has_the_adoption_holds_exact_words() {
        let expected = b"This chat began before org/repo was adopted, so you can read org/repo but not push to it. Tell the person, in your next reply: \"This chat began before org/repo was adopted, so I can read it but not push. Start a new chat to push.\"";
        assert_eq!(
            forge::home_access_reason(b"org", b"repo", forge::HOME_SECTION_BYTES),
            forge::BriefRead::Got { bytes: expected.as_slice().into(), left: 0 },
        );
    }

    #[test]
    fn the_home_section_constant_fits_the_longest_admitted_names() {
        let mut names = skein_lib::List::with_capacity(forge::PANEL_NAME_BYTES);
        for _ in 0..forge::PANEL_NAME_BYTES {
            names.push(b'x').expect("name bound");
        }
        let names = names.into_boxed();
        let forge::BriefRead::Got { bytes, left } =
            forge::home_access_reason(&names, &names, forge::HOME_SECTION_BYTES)
        else {
            panic!("home reason")
        };
        assert_eq!(left, 0);
        assert_eq!(bytes.len(), usize::try_from(forge::HOME_SECTION_BYTES).expect("section bytes"));
        let budgets = crate::BriefBudgets {
            task: 1,
            dependencies: 1,
            ci: 1,
            reviews: 1,
            pull: 1,
            attempts: 1,
            plan: 1,
            notes: 1,
        };
        assert_eq!(forge_brief_budget(ForgeBriefKind::Home, &budgets), forge::HOME_SECTION_BYTES);
    }
}
