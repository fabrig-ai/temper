use super::fixtures;
use jig_client::contract::{self as c, ActKind, Admission, ObjectKey, Read, ReadValue};
use skein_lib::{List, Wall};
use skein_ui::Role;
use temper_client_domain::{Action, Event, address::Filter, pages::Act};
const PARTY: u64 = 73;
const TEXT_BYTES: u32 = 32;
const SINGLE_OFFER: u32 = 1;
const FUNDING_PERIOD: u64 = 1;
const FUNDING_NEEDED: u64 = 5;
const FUNDING_LEFT: u64 = 20;
const FUNDING_AFTER: u64 = 15;
const EMPTY_BATCH: u32 = 0;
const PROPOSAL_INDEX: u32 = 1;
const PROPOSAL: c::ProposalKey =
    c::ProposalKey { proposer: c::Proposer::Task(fixtures::INBOX_TASK), number: fixtures::INBOX_QUESTION };
fn text(words: &[u8]) -> c::Text {
    c::Text::new(Box::from(words), TEXT_BYTES).expect("fixture text")
}
pub(super) fn root(act: ActKind, admission: Admission) -> temper_client_domain::Domain {
    let mut acts = List::with_capacity(SINGLE_OFFER);
    acts.push(c::Offered { act, admission, effect: None }).expect("one act");
    let mut funding = List::with_capacity(SINGLE_OFFER);
    funding
        .push(c::OwnerFundingOffer {
            source: c::Funder::Pool { project: fixtures::PROJECT, person: PARTY, period: FUNDING_PERIOD },
            needed: FUNDING_NEEDED,
            left: Some(FUNDING_LEFT),
            after: Some(FUNDING_AFTER),
        })
        .expect("one funding source");
    let page = c::OwnerProposal {
        key: PROPOSAL,
        project: fixtures::PROJECT,
        reason: c::DocumentText::complete(text(b"Make a goal")),
        action: c::OwnerProposalAction::Batch(List::with_capacity(EMPTY_BATCH)),
        as_holder: true,
        waiting: c::OwnerProposalWaiting { holder: c::OwnerHistoryHolder::Person(PARTY), since: Wall::EPOCH },
        funding,
        acts,
    };
    fixtures::inbox_with_document(
        Filter::ALL,
        Some((Read::Proposal { proposal: PROPOSAL }, ReadValue::OwnerProposal(Box::new(page)))),
    )
}
fn press(root: &mut temper_client_domain::Domain, act: Act) {
    fixtures::root_event(root, Event::Act { action: Action::Pages(act) });
}
fn open(root: &mut temper_client_domain::Domain, act: ActKind) {
    press(root, Act::Decide { object: ObjectKey::Proposal(PROPOSAL), revision: None, act });
}
const ACCEPT_WORDS: [&[u8]; 4] = [b"Why", b"Funds 5 from your pool", b"20 left", b"15 after"];
#[test]
fn the_accept_confirmation_says_what_it_grants_costs_and_leaves_in_the_pool() {
    let mut root = root(ActKind::Accept, Admission::Allowed);
    open(&mut root, ActKind::Accept);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Dialog, b"Accept").is_some());
    let said = fixtures::text(tree);
    for expected in ACCEPT_WORDS {
        assert!(contains(&said, expected), "{expected:?}");
    }
}
#[test]
fn rejecting_says_so_with_its_reason_before_it_is_sent() {
    let mut root = root(ActKind::Reject, Admission::Allowed);
    press(&mut root, Act::CardWords { object: ObjectKey::Proposal(PROPOSAL), words: Box::from(*b"Too costly") });
    open(&mut root, ActKind::Reject);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Dialog, b"Reject with reason").is_some());
    assert!(contains(&fixtures::text(tree), b"Too costly"));
}
#[test]
fn an_act_only_proposable_says_so_and_to_whom() {
    let root = root(ActKind::Accept, Admission::Proposed { to: c::Recipient::Party(PARTY) });
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Button, b"Accept").is_none());
    assert!(contains(&fixtures::text(tree), b"This needs a proposal to"));
}

fn contains(text: &[u8], expected: &[u8]) -> bool {
    for words in text.windows(expected.len()) {
        if words == expected {
            return true;
        }
    }
    false
}
const OUTCOMES: [(jig_client::Outcome, &[u8]); 3] = [
    (jig_client::Outcome::NotKept, b"The request was not kept."),
    (jig_client::Outcome::Unknown, b"The engine no longer knows this request."),
    (
        jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::NotOffered,
            at: None,
        })),
        b"This act is not offered",
    ),
];
#[test]
fn the_card_reports_client_outcomes_in_jigs_words() {
    let root = root(ActKind::Accept, Admission::Allowed);
    let card = root.pages().inbox().expect("inbox").cards.get(PROPOSAL_INDEX).expect("proposal");
    let limits = fixtures::tree_limits();
    let words = fixtures::words();
    for (outcome, expected) in OUTCOMES {
        let mut card = card.clone();
        card.outcome = Some(outcome);
        let mut builder = skein_ui::Builder::new(&limits);
        let mut bindings = crate::frame::Bindings::new(limits.bindings);
        crate::confirm::card(
            &mut builder,
            &root,
            &card,
            &words,
            &mut bindings,
            fixtures::domain_limits().client.domain.rows,
        );
        assert!(contains(&fixtures::text(&builder.finish()), expected), "{expected:?}");
    }
}

#[test]
fn the_inbox_keeps_proposal_and_decision_facts_before_its_controls() {
    let view = fixtures::page_view(&root(ActKind::Accept, Admission::Allowed));
    let tree = view.tree();
    let details = super::inbox::styled(tree, crate::look::PROPOSAL_FACTS);
    let facts = super::inbox::subtree(tree, details);
    for expected in ACCEPT_WORDS {
        let mut found = false;
        for node in facts {
            for run in &node.text {
                found |= contains(&run.text, expected);
            }
        }
        assert!(found, "{expected:?}");
    }
    let accept = fixtures::find(tree, Role::Button, b"Accept").expect("accept");
    let mut control = None;
    let mut reason = None;
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.id == accept {
            control = Some(index);
        }
        for run in &node.text {
            if run.text.as_ref() == b"Make a goal" {
                reason = Some(index);
            }
        }
    }
    assert!(control.expect("control") >= details.checked_add(facts.len()).expect("facts end"));
    for node in facts {
        assert_ne!(node.role, Role::Button);
        assert_ne!(node.role, Role::TextArea);
    }

    assert!(reason.expect("reason") > details);
}

#[test]
fn proposal_rows_and_task_release_keep_their_own_shapes() {
    for (root, expected) in [
        (
            Box::new(fixtures::chat::proposal_root()),
            &[
                Role::Paragraph,
                Role::Text,
                Role::Slot,
                Role::Term,
                Role::Text,
                Role::Description,
                Role::Paragraph,
                Role::Text,
                Role::Text,
                Role::Text,
                Role::Term,
                Role::Text,
                Role::Description,
                Role::Paragraph,
                Role::Button,
                Role::Disclosure,
                Role::TextArea,
                Role::Button,
                Role::Link,
                Role::Text,
            ][..],
        ),
        (
            Box::new(fixtures::task::held_root()),
            &[Role::Paragraph, Role::Text, Role::Paragraph, Role::Text, Role::Button][..],
        ),
    ] {
        let mut loaded = None;
        for card in root.pages().cards().expect("cards") {
            if card.page.is_some() {
                loaded = Some(card);
                break;
            }
        }
        let card = loaded.expect("loaded card");
        let limits = fixtures::tree_limits();
        let mut builder = skein_ui::Builder::new(&limits);
        crate::confirm::card(
            &mut builder,
            &root,
            card,
            &fixtures::words(),
            &mut crate::frame::Bindings::new(limits.bindings),
            fixtures::domain_limits().client.domain.rows,
        );
        let tree = builder.finish();
        let mut roles = List::with_capacity(32);
        for node in tree.nodes() {
            roles.push(node.role).expect("flat card shape");
        }
        assert_eq!(roles.as_slice(), expected);
    }
}

#[test]
fn proposal_rejection_starts_in_a_closed_disclosure_in_both_pages() {
    for root in fixtures::pair(fixtures::chat::proposal_root(), root(ActKind::Reject, Admission::Allowed)) {
        let view = fixtures::page_view(&root);
        let nodes = view.tree().nodes();
        let at = super::chat::named_position(nodes, Role::Disclosure, Some(b"Reject with reason"))
            .expect("reject disclosure");
        assert!(!nodes[at].states.as_slice().contains(&skein_ui::State::Expanded));
        let children = super::inbox::subtree(view.tree(), at);
        assert!(super::chat::named_position(children, Role::TextArea, Some(b"Reason")).is_some());
        assert!(super::chat::named_position(children, Role::Button, Some(b"Reject")).is_some());
        assert!(super::chat::named_position(children, Role::Button, Some(b"Accept")).is_none());
    }
}
#[test]
fn a_rejecting_chat_keeps_its_reason_disclosure_open() {
    let view = fixtures::page_view(&fixtures::chat::rejecting_chat());
    let disclosure = fixtures::find(view.tree(), Role::Disclosure, b"Reject with reason").expect("reject disclosure");
    assert!(view.tree().find(disclosure).unwrap().states.as_slice().contains(&skein_ui::State::Expanded));
}
#[test]
fn releases_card_tree_stays_byte_identical() {
    let root = fixtures::task::held_root();
    let mut loaded = None;
    for card in root.pages().cards().unwrap() {
        if card.page.is_some() {
            loaded = Some(card);
            break;
        }
    }
    let card = loaded.unwrap();
    let mut builder = skein_ui::Builder::new(&fixtures::tree_limits());
    crate::confirm::card(
        &mut builder,
        &root,
        card,
        &fixtures::words(),
        &mut crate::frame::Bindings::new(fixtures::tree_limits().bindings),
        2,
    );
    let tree = builder.finish();
    // Test-only diagnostic serialization freezes all of Release's node fields.
    let actual = alloc::fmt::format(format_args!("{:?}", tree.nodes()));
    if let Some(path) = std::env::var_os("TEMPER_RECORD_RELEASE") {
        std::fs::write(path, &actual).unwrap();
        return;
    }
    assert_eq!(actual.as_bytes(), std::fs::read("src/tests/release-card.txt").unwrap());
}
