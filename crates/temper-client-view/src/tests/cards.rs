use super::fixtures;
use jig_client::{
    contract as c,
    words::{DecisionWord, Word},
};
use skein_ui::{Builder, phrases::Hole};
const FUNDS: u64 = 40;
const LEFT: u64 = 120;
const AFTER: u64 = 80;
#[test]
fn proposal_funding_uses_the_mockups_pool_sentence() {
    let words = fixtures::words();
    assert_eq!(
        words
            .say_word(
                Word::Decision(DecisionWord::Funds),
                &[Hole::Amount(FUNDS), Hole::Amount(LEFT), Hole::Amount(AFTER)]
            )
            .as_ref(),
        b"Funds 40 from your pool \xc2\xb7 120 left, 80 after",
    );
}
#[test]
fn a_result_shows_its_committed_words_in_temper_nodes() {
    let mut builder = Builder::new(&fixtures::tree_limits());
    let result = c::ResultValue::Report(c::DocumentText::complete(
        c::Text::new(Box::from(*b"Ready to review"), fixtures::domain_limits().client.domain.text)
            .expect("report fits"),
    ));
    crate::cards::value(&mut builder, &result, &fixtures::words());
    assert_eq!(fixtures::text(&builder.finish()).as_ref(), b"Ready to review");
}
const OTHER_TASK: u64 = 12;
const ENTRY: u64 = 11;
const POOL: u64 = 7;
const HOLDS: [(c::HoldCategory, &[u8]); 6] = [
    (
        c::HoldCategory::Exceeded(c::Exceeded { bound: c::Bound::Reasoning, value: 2048 }),
        b"Held: the model's reasoning went past its configured allowance of 2048 bytes",
    ),
    (
        c::HoldCategory::Exceeded(c::Exceeded { bound: c::Bound::History, value: 2048 }),
        b"Held: the run's history no longer fits its configured allowance, even compacted",
    ),
    (c::HoldCategory::Circle, b"Held for a circle of waits"),
    (c::HoldCategory::Dependency { task: OTHER_TASK }, b"Dependency task 12"),
    (c::HoldCategory::Uncertain { entry: ENTRY }, b"Uncertain outbox entry 11"),
    (c::HoldCategory::PoolLost { pool: POOL }, b"Allocation lost from pool 7"),
];
#[test]
fn numbered_hold_reasons_keep_the_documents_supplied_identity() {
    for (reason, expected) in HOLDS {
        let mut builder = Builder::new(&fixtures::tree_limits());
        crate::cards::hold(&mut builder, OTHER_TASK, &reason, &fixtures::words());
        assert_eq!(fixtures::text(&builder.finish()).as_ref(), expected);
    }
}

#[test]
fn dependency_refusals_use_jigs_words() {
    for (refusal, expected) in [
        (
            c::Refusal::Circle { tasks: skein_lib::List::with_capacity(0), cut: false },
            b"These tasks would wait for one another".as_slice(),
        ),
        (
            c::Refusal::Dependency { dependency: OTHER_TASK, how: c::OwnerTaskStatus::Failed },
            b"The added dependency has ended",
        ),
        (
            c::Refusal::Kept { pending: skein_lib::List::with_capacity(0) },
            b"This answer waits for pending dependencies",
        ),
    ] {
        let mut builder = Builder::new(&fixtures::tree_limits());
        crate::cards::refusal(&mut builder, &fixtures::words(), &refusal);
        assert_eq!(fixtures::text(&builder.finish()).as_ref(), expected);
    }
}

pub(super) fn batch_proposal() -> c::ReadValue {
    let mut task = fixtures::task::document();
    task.authority.authority.spend = 6;
    task.authority.authority.delegation.tasks = 1;
    task.authority.authority.delegation.depth = 1;
    task.spec.specification.words =
        c::DocumentText::complete(c::Text::new(Box::from(*b"Ship update\nDetails"), 32).unwrap());
    let mut resources = skein_lib::List::with_capacity(1);
    resources
        .push(c::OwnerPart {
            connector: fixtures::FORGE,
            kind: c::OwnerPartKind::Resource,
            key: c::OwnerPartKey::Path(fixtures::path(&[
                b"forge",
                b"github.com",
                b"robin",
                b"tally",
                b"branch",
                b"main",
            ])),
        })
        .unwrap();
    let mut tasks = skein_lib::List::with_capacity(1);
    tasks
        .push(c::ProposedTask {
            number: 1,
            spec: task.spec,
            executor: c::OwnerExecutor::Agent { charter: 2 },
            contract: task.result_contract,
            authority: task.authority,
            deadline: c::NewDeadline::Named,
            budget: 6,
            funder: task.budget.funder,
            dependencies: skein_lib::List::with_capacity(0),
            resources,
            wake: task.wake,
            priority: Some(2),
            cut: false,
        })
        .unwrap();
    let root = fixtures::chat::proposal_root();
    let mut page = None;
    for card in root.pages().cards().unwrap() {
        if let Some(value) = &card.page {
            page = Some(value.clone());
            break;
        }
    }
    let mut page = page.unwrap();
    let c::ReadValue::OwnerProposal(proposal) = &mut page else { panic!("proposal") };
    proposal.action = c::OwnerProposalAction::Batch(tasks);
    proposal.funding = skein_lib::List::with_capacity(1);
    proposal
        .funding
        .push(c::OwnerFundingOffer {
            source: c::Funder::Pool { project: fixtures::PROJECT, person: 73, period: 1 },
            needed: FUNDS,
            left: Some(LEFT),
            after: Some(AFTER),
        })
        .unwrap();
    page
}

#[test]
fn a_proposals_title_and_labelled_facts_follow_the_mockup() {
    use skein_ui::Role;
    let mut builder = Builder::new(&fixtures::tree_limits());
    crate::cards::proposal(&mut builder, &batch_proposal(), &fixtures::words(), 4);
    let tree = builder.finish();
    assert!(fixtures::find(&tree, Role::Heading { level: 2 }, b"Ship update").is_some());
    assert!(fixtures::find(&tree, Role::Heading { level: 3 }, b"Proposed tasks").is_none());
    let mut labels = skein_lib::List::with_capacity(4);
    for node in tree.nodes() {
        if node.role == Role::Term {
            labels.push(node.name.as_deref().unwrap()).unwrap();
        }
    }
    assert_eq!(labels.as_slice(), [b"Why".as_slice(), b"Goal", b"Landing", b"Budget"]);
    for (label, expected) in [
        (b"Why".as_slice(), b"Make a goal".as_slice()),
        (b"Goal", b"Agent charter 2"),
        (b"Landing", b"branch main of robin/tally on github.com"),
        (b"Budget", b"Funds 40 from your pool"),
    ] {
        let at = super::chat::named_position(tree.nodes(), Role::Term, Some(label)).unwrap();
        let description = at + usize::try_from(tree.nodes()[at].size).unwrap();
        assert_eq!(tree.nodes()[description].role, Role::Description);
        assert!(skein_lib::bytes::find(&fixtures::node_text(&tree, description), expected).is_some());
    }
    assert!(fixtures::find(&tree, Role::Paragraph, b"Priority 2").is_none());
}

fn document_proposal() -> c::ReadValue {
    let key = fixtures::chat::PROPOSAL;
    c::ReadValue::Proposal(Box::new(c::Proposal {
        key,
        project: fixtures::PROJECT,
        proposer: key.proposer,
        why: fixtures::first_run::text(b"It touches sign-in."),
        action: c::ProposalAction::Batch { tasks: skein_lib::List::with_capacity(0), own: true },
        needs: c::Authority {
            models: None,
            budget: FUNDS,
            grants: skein_lib::List::with_capacity(0),
            task_kinds: skein_lib::List::with_capacity(0),
            depth: 1,
            tasks: 1,
            deadline: None,
            tools: skein_lib::List::with_capacity(0),
            note_scopes: skein_lib::List::with_capacity(0),
        },
        funding: c::Budget { total: LEFT, own: 0, ended: 0, reserved: 0, left: LEFT, funder: c::Actor::Party(73) },
        waiting: c::Recipient::Party(73),
        trail: skein_lib::List::with_capacity(0),
        since: skein_lib::Wall::EPOCH,
        acts: skein_lib::List::with_capacity(0),
        ending: None,
    }))
}
fn occurrences(tree: &skein_ui::Tree, text: &[u8]) -> usize {
    let written = fixtures::text(tree);
    let mut count = 0_usize;
    for part in written.windows(text.len()) {
        if part == text {
            count = count.checked_add(1).unwrap();
        }
    }
    count
}
#[test]
fn proposal_titles_head_the_cards_and_funding_is_said_once_in_both_pages() {
    use skein_ui::Role;
    for (page, owner) in [(batch_proposal(), true), (document_proposal(), false)] {
        for root in fixtures::pair(
            fixtures::chat::proposal_root_with_document(None, Some(page.clone())),
            fixtures::inbox_with_document(
                temper_client_domain::address::Filter::ALL,
                Some((
                    c::Read::Proposal {
                        proposal: c::ProposalKey {
                            proposer: c::Proposer::Task(fixtures::INBOX_TASK),
                            number: fixtures::INBOX_QUESTION,
                        },
                    },
                    page,
                )),
            ),
        ) {
            let view = fixtures::page_view(&root);
            let tree = view.tree();
            let budget = super::chat::named_position(tree.nodes(), Role::Term, Some(b"Budget")).expect("Budget row");
            let description = budget.checked_add(usize::try_from(tree.nodes()[budget].size).unwrap()).unwrap();
            if owner {
                let title = super::chat::named_position(
                    tree.nodes(),
                    Role::Heading { level: if root.pages().inbox().is_some() { 3 } else { 2 } },
                    Some(b"Ship update"),
                )
                .expect("proposal title heads the card");
                assert!(title < budget);
                for expected in [b"Funds 40 from your pool".as_slice(), b"120 left", b"80 after"] {
                    assert!(skein_lib::bytes::find(&fixtures::node_text(tree, description), expected).is_some());
                    assert_eq!(occurrences(tree, expected), 1);
                }
                for expected in [b"May spend 6".as_slice(), b"May delegate 1 task, depth 1", b"Budget 6"] {
                    assert_eq!(occurrences(tree, expected), 0);
                }
            } else {
                let funding = b"Funds 40 from your pool \xc2\xb7 120 left, 80 after";
                assert!(skein_lib::bytes::find(&fixtures::node_text(tree, description), funding).is_some());
                assert_eq!(occurrences(tree, funding), 1);
                for expected in [b"May spend 40".as_slice(), b"May delegate 1 task, depth 1"] {
                    assert_eq!(occurrences(tree, expected), 0);
                }
            }
        }
    }
}

#[test]
fn a_proposal_kicker_precedes_the_title_in_a_chat_but_not_in_the_inbox() {
    use crate::cards::Place;
    use skein_ui::Role;
    let page = batch_proposal();
    // One root at a time: two outgrow a stack array's 16 KiB.
    for place in [Place::Chat, Place::Inbox] {
        let root = if place == Place::Chat {
            fixtures::chat::proposal_root_with_document(None, Some(page.clone()))
        } else {
            fixtures::inbox_with_document(
                temper_client_domain::address::Filter::ALL,
                Some((
                    c::Read::Proposal {
                        proposal: c::ProposalKey {
                            proposer: c::Proposer::Task(fixtures::INBOX_TASK),
                            number: fixtures::INBOX_QUESTION,
                        },
                    },
                    page.clone(),
                )),
            )
        };
        let view = fixtures::page_view(&root);
        let tree = view.tree();
        let level = if place == Place::Chat { 2 } else { 3 };
        let title = super::chat::named_position(tree.nodes(), Role::Heading { level }, Some(b"Ship update")).unwrap();
        let mut kickers = 0_u32;
        let mut first = None;
        for (at, node) in tree.nodes().iter().enumerate() {
            if node.styles.as_slice().contains(&crate::look::PROPOSAL_KICKER) {
                kickers += 1;
                first = first.or(Some(at));
            }
        }
        if place == Place::Chat {
            assert_eq!(kickers, 1, "one Proposal kicker in a chat");
            let kicker = first.expect("Proposal kicker");
            assert_eq!(fixtures::node_text(tree, kicker).as_ref(), b"Proposal");
            assert!(kicker < title);
        } else {
            assert_eq!(kickers, 0, "the inbox's tag says the kind: no kicker");
        }
    }
}

#[test]
fn proposal_facts_are_labelled_joined_and_contained_in_their_rows() {
    use skein_ui::Role;
    let owner = batch_proposal();
    let mut document = document_proposal();
    let c::ReadValue::OwnerProposal(source) = &owner else { panic!("owner proposal") };
    let c::OwnerProposalAction::Batch(tasks) = &source.action else { panic!("batch") };
    let c::ReadValue::Proposal(target) = &mut document else { panic!("document proposal") };
    let mut tasks = tasks.clone();
    let task = tasks.get_mut(0).unwrap();
    task.authority.authority.delegation.tasks = 0;
    task.authority.authority.delegation.depth = 0;
    target.action = c::ProposalAction::Batch { tasks, own: true };
    for (page, _delegation, budget) in [
        (owner, "1 task, depth 1", "Funds 40 from your pool · 120 left, 80 after"),
        (document, "0 tasks, depth 0", "Funds 40 from your pool · 120 left, 80 after"),
    ] {
        let mut builder = Builder::new(&fixtures::tree_limits());
        crate::cards::proposal(&mut builder, &page, &fixtures::words(), 4);
        let tree = builder.finish();
        let mut rows = None;
        for (at, node) in tree.nodes().iter().enumerate() {
            if node.styles.as_slice().contains(&crate::look::PROPOSAL_FACTS) {
                rows = Some(at);
                break;
            }
        }
        let rows = rows.unwrap();
        let goal = alloc::fmt::format(format_args!("yours · Agent charter 2"));
        for (label, expected) in [
            (b"Goal".as_slice(), goal.as_str()),
            (b"Landing", "branch main of robin/tally on github.com · priority 2"),
            (b"Budget", budget),
        ] {
            let at = super::chat::named_position(tree.nodes(), Role::Term, Some(label)).unwrap();
            let description = at + usize::try_from(tree.nodes()[at].size).unwrap();
            assert_eq!(tree.nodes()[description].role, Role::Description);
            assert_eq!(fixtures::node_text(&tree, description).as_ref(), expected.as_bytes(), "row {label:?}");
        }
        assert_eq!(
            rows + usize::try_from(tree.nodes()[rows].size).unwrap(),
            tree.nodes().len(),
            "no loose fact after the rows"
        );
    }
}

#[test]
fn a_non_batch_proposal_keeps_its_deciding_facts() {
    let mut page = batch_proposal();
    let c::ReadValue::OwnerProposal(proposal) = &mut page else { panic!("owner proposal") };
    proposal.action =
        c::OwnerProposalAction::Release { task: c::OwnerTaskLink::Stored { number: 9, status: None }, hold: None };
    let mut builder = Builder::new(&fixtures::tree_limits());
    crate::cards::proposal(&mut builder, &page, &fixtures::words(), 4);
    let tree = builder.finish();
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"Make a goal").is_some());
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"Make task 9 yours").is_some());
    assert!(fixtures::find(&tree, skein_ui::Role::Term, b"Landing").is_none());
}

#[test]
fn a_budget_held_card_names_the_task_to_amend_and_explains_release() {
    let mut task = fixtures::task::document();
    task.chip.task = OTHER_TASK;
    task.hold = Some(c::TaskHold::Owner(Box::new(c::OwnerChipHold {
        circle: None,
        account: None,
        source: None,
        reason: c::HoldCategory::Budget,
        revision: 1,
        before: c::OwnerPriorPhase::Active(c::RunStage::Idle),
        escalation: None,
        tries: c::OwnerFailureTries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
        hold_wait_since: None,
    })));
    let mut builder = Builder::new(&fixtures::tree_limits());
    crate::cards::proposal(&mut builder, &c::ReadValue::Task(Box::new(task)), &fixtures::words(), 4);
    let text = fixtures::text(&builder.finish());
    assert!(skein_lib::bytes::find(&text,
        b"Insufficient activation budget. A release alone holds it again: give the task more budget (task amend 12 with a larger budget, or decide its budget proposal), then release it; or cancel it.").is_some(), "{text:?}");
}
