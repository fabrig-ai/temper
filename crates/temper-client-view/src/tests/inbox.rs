use super::fixtures;
use crate::look;
use jig_client::contract as c;
use skein_ui::{Node, State, Tree};
use temper_client_domain::PlanFilter;
use temper_client_domain::{
    Address, Domain, Event,
    address::{Filter, Kind},
};

use skein_ui::{Act, Role};
const GOAL: u64 = 7;
#[test]
fn entries_are_named_by_kind_and_source_task() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    for name in [
        b"Proposal from T9".as_slice(),
        b"Question from T9",
        b"Held from T9",
        b"Person task from T9",
        b"Updates from T9",
        b"Reply from T10",
    ] {
        assert!(fixtures::person_find(view.tree(), Role::Item, name).is_some(), "{name:?}");
    }
}

#[test]
fn a_proposal_from_the_signed_in_party_names_them() {
    let view = fixtures::page_view(&fixtures::inbox_from(c::Proposer::Party(fixtures::chat::PARTY)));
    assert!(fixtures::person_find(view.tree(), Role::Item, b"Proposal from Pat").is_some());
}

#[test]
fn another_partys_proposal_is_named_by_kind_alone() {
    let view = fixtures::page_view(&fixtures::inbox_from(c::Proposer::Party(fixtures::OTHER_PARTY)));
    assert!(fixtures::person_find(view.tree(), Role::Item, b"Proposal").is_some());
}

#[test]
fn the_needs_list_is_named_by_its_heading() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    assert!(fixtures::person_find(view.tree(), Role::List, b"Needs you").is_some());
}

#[test]
fn the_updates_list_is_named_by_its_heading() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    assert!(fixtures::person_find(view.tree(), Role::List, b"Updates").is_some());
}

#[test]
fn entries_show_proposals_questions_held_and_person_tasks_then_updates() {
    let root = fixtures::inbox_root();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let mut headings = skein_lib::List::with_capacity(8);
    for node in tree.nodes() {
        if node.role == (Role::Heading { level: 2 }) || node.role == (Role::Heading { level: 3 }) {
            headings.push(node.name.as_deref().expect("named heading")).expect("eight headings");
        }
    }
    assert_eq!(headings.as_slice(), [b"Needs you".as_slice(), b"Updates", b"Task 9", b"Task 10"]);
}
#[test]
fn an_entry_naming_a_task_links_to_its_page_by_number() {
    let root = fixtures::inbox_root();
    let mut view = fixtures::view();
    let node = fixtures::find(fixtures::build(&mut view, &root), Role::Link, b"T9").expect("numbered task link");
    assert_eq!(
        crate::act(&mut view, node, Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
            address: Address::Task { task: fixtures::INBOX_TASK, filter: PlanFilter::All }
        }))
    );
}
#[test]
fn the_goal_filter_keeps_its_project_and_kind() {
    let filter = Filter { project: Some(fixtures::PROJECT), kind: Some(Kind::Question), goal: None };
    let root = fixtures::inbox_root_filtered(filter);
    let mut view = fixtures::view();
    let node = fixtures::find(fixtures::build(&mut view, &root), Role::TextField, b"Goal").expect("goal filter");
    assert_eq!(
        crate::act(&mut view, node, Act::Input { value: Box::from(*b"7") }),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Filter {
            filter: Filter { goal: Some(GOAL), ..filter }
        }))
    );
}

pub(super) fn subtree(tree: &Tree, index: usize) -> &[Node] {
    let node = &tree.nodes()[index];
    &tree.nodes()[index..index.checked_add(usize::try_from(node.size).expect("subtree size")).expect("subtree end")]
}
pub(super) fn styled(tree: &Tree, token: skein_ui::Style) -> usize {
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&token) {
            return index;
        }
    }
    panic!("styled node {token:?}");
}
fn heading(tree: &Tree, name: &[u8]) -> usize {
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&look::HEADING) {
            for child in subtree(tree, index) {
                if child.role == (Role::Heading { level: 2 }) && child.name.as_deref() == Some(name) {
                    return index;
                }
            }
        }
    }
    panic!("section heading");
}
fn style_count(nodes: &[Node], token: skein_ui::Style) -> usize {
    let mut count = 0_usize;
    for node in nodes {
        if node.styles.as_slice().contains(&token) {
            count = count.checked_add(1).expect("tree bounds count");
        }
    }
    count
}
fn role_node(nodes: &[Node], role: Role) -> &Node {
    for node in nodes {
        if node.role == role {
            return node;
        }
    }
    panic!("role {role:?}");
}
#[test]
fn headings_hold_the_page_title_and_filters_without_mark_seen() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    let tree = view.tree();
    let bar = subtree(tree, styled(tree, look::PAGEBAR));
    assert_eq!(bar.len(), 3);
    assert_eq!(bar[1].role, Role::Heading { level: 1 });
    assert_eq!(bar[1].name.as_deref(), Some(b"Inbox".as_slice()));
    let needs = subtree(tree, heading(tree, b"Needs you"));
    assert_eq!(style_count(needs, look::FILTERS), 1);
    for node in needs {
        assert_ne!(node.name.as_deref(), Some(b"Mark seen".as_slice()));
    }
    let updates = subtree(tree, heading(tree, b"Updates"));
    assert_eq!(updates.len(), 5);
    for node in tree.nodes() {
        assert_ne!(node.name.as_deref(), Some(b"Mark seen".as_slice()));
    }
}
#[test]
fn filters_press_exactly_the_current_project_kind_and_all_goals() {
    for (filter, expected) in [
        (Filter::ALL, [b"All projects".as_slice(), b"All goals", b"All kinds"]),
        (
            Filter { project: Some(fixtures::PROJECT), kind: Some(Kind::Question), goal: None },
            [b"Shop".as_slice(), b"All goals", b"Question"],
        ),
    ] {
        let root = fixtures::inbox_root_filtered(filter);
        let mut view = fixtures::page_view(&root);
        let tree = view.tree();
        let mut pressed = skein_lib::List::with_capacity(3);
        let mut ids = skein_lib::List::with_capacity(3);
        let filters = subtree(tree, styled(tree, look::FILTERS));
        for node in filters {
            if node.states.as_slice().contains(&State::Pressed) {
                assert_eq!(node.role, Role::Button);
                pressed.push(node.name.as_deref().expect("filter name")).expect("one pressed per group");
                ids.push(node.id).expect("three controls");
            }
        }
        assert_eq!(pressed.as_slice(), expected);
        // Each pressed button remains an action for the same filter.
        for id in ids.as_slice() {
            assert_eq!(
                crate::act(&mut view, *id, Act::Press),
                Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Filter { filter }))
            );
        }
    }
}
#[test]
fn cards_are_grouped_in_lists_with_kinds_and_named_project_paths() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    let tree = view.tree();
    let mut lists = skein_lib::List::with_capacity(2);
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&look::CARDS) {
            assert_eq!(node.role, Role::List);
            lists.push(index).expect("two lists");
        }
    }
    assert_eq!(lists.len(), 2);
    assert_eq!(tree.nodes()[lists.as_slice()[0]].styles.as_slice(), [look::CARDS]);
    assert_eq!(tree.nodes()[lists.as_slice()[1]].styles.as_slice(), [look::CARDS, look::UPDATES]);
    let kinds = [
        look::KIND_PROPOSAL,
        look::KIND_QUESTION,
        look::KIND_HELD,
        look::KIND_PERSON_TASK,
        look::KIND_RESULT,
        look::KIND_REPLY,
    ];
    for (i, token) in kinds.into_iter().enumerate() {
        let card = styled(tree, token);
        let node = &tree.nodes()[card];
        assert_eq!(node.role, Role::Item);
        assert_eq!(node.styles.as_slice(), [look::CARD, token]);
        let list = lists.as_slice()[usize::from(i >= 4)];
        assert!(card > list && card < list.checked_add(subtree(tree, list).len()).expect("list end"));
        if i < 4 {
            let meta = subtree(tree, card.checked_add(1).expect("meta index"));
            assert_eq!(meta[0].styles.as_slice(), [look::META]);
            assert_eq!(meta[1].styles.as_slice(), [look::INBOX_TAG]);
            assert!(skein_lib::bytes::find(&fixtures::node_text(tree, card), "Shop › T9".as_bytes()).is_some());
            assert_eq!(role_node(meta, Role::Link).name.as_deref(), Some(b"T9".as_slice()));
        } else {
            assert_eq!(style_count(subtree(tree, card), look::META), 0);
        }
    }
}
#[test]
fn absent_groups_have_no_empty_lists() {
    let limits = fixtures::domain_limits();
    let (mut root, _, watch) = fixtures::root_at(Address::Inbox { filter: Filter::ALL }, &limits);
    let mut out = skein_lib::Queue::with_capacity(temper_client_domain::max_out(&limits));
    temper_client_domain::step(
        &mut root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits },
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("inbox watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Inbox(Box::new(c::Inbox {
                        entries: skein_lib::List::with_capacity(0),
                        needs: c::Needs { proposals: 0, questions: 0, escalations: 0, person_tasks: 0 },
                        updates: c::Count { value: 0, limited: false },
                        results_seen: 0,
                        replies_seen: skein_lib::List::with_capacity(0),
                        more: false,
                    }))),
                },
            },
        },
        &mut out,
    );
    let view = fixtures::page_view(&root);
    assert_eq!(style_count(view.tree().nodes(), look::CARDS), 0);
    assert_eq!(style_count(view.tree().nodes(), look::EMPTY), 2);
    for message in [b"Nothing needs you.".as_slice(), b"No updates."] {
        assert!(fixtures::find(view.tree(), Role::Paragraph, message).is_some());
    }
    for section in [b"Needs you".as_slice(), b"Updates"] {
        assert!(has_styled_name(subtree(view.tree(), heading(view.tree(), section)), look::SUBTLE_COUNT, b"0"));
    }
    let filtered =
        fixtures::page_view(&fixtures::inbox_root_filtered(Filter { kind: Some(Kind::Question), ..Filter::ALL }));
    assert_eq!(style_count(filtered.tree().nodes(), look::CARDS), 1);
    assert_eq!(style_count(filtered.tree().nodes(), look::UPDATES), 0);
}
pub(super) fn question_root() -> Domain {
    let task = fixtures::task::document();
    let page = c::Question {
        task: task.chip,
        from: None,
        message: fixtures::INBOX_QUESTION,
        words: fixtures::task::document_words(b"Which path?"),
        since: skein_lib::Wall::EPOCH,
        acts: skein_lib::List::with_capacity(0),
        ending: None,
    };
    fixtures::inbox_with_document(
        Filter::ALL,
        Some((
            c::Read::Question { task: fixtures::INBOX_TASK, message: fixtures::INBOX_QUESTION },
            c::ReadValue::Question(Box::new(page)),
        )),
    )
}
#[test]
fn a_question_keeps_its_title_in_its_line_and_has_no_heading() {
    let root = question_root();
    let view = fixtures::page_view(&root);
    let tree = view.tree();
    let question = styled(tree, look::KIND_QUESTION);
    assert_eq!(style_count(subtree(tree, question), look::TITLE), 0);
    assert!(
        skein_lib::bytes::find(&fixtures::node_text(tree, question), fixtures::task::document().chip.title.bytes())
            .is_some()
    );
    assert_eq!(style_count(tree.nodes(), look::DETAILS), 0);
}
#[test]
fn an_update_omits_its_project_metadata_before_its_title_arrives() {
    let limits = fixtures::domain_limits();
    let (mut root, me_watch, inbox_watch) = fixtures::root_at(Address::Inbox { filter: Filter::ALL }, &limits);
    let mut me = root.pages().me(root.client()).expect("me").clone();
    me.projects.get_mut(0).expect("project").name = jig_client::contract::DocumentText::complete(
        jig_client::contract::Text::new(b"Project 1".as_slice().into(), 64).expect("project name"),
    );
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me_watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                },
            },
        },
    );
    let mut entries = skein_lib::List::with_capacity(1);
    entries
        .push(c::InboxEntry {
            reply: None,
            lineage: skein_lib::List::with_capacity(0),
            goal: None,
            recipient: None,
            stall: None,
            object: c::ObjectKey::Task(10),
            project: fixtures::PROJECT,
            group: c::InboxGroup::Replies,
            position: 1,
            at: skein_lib::Wall::EPOCH,
        })
        .expect("entry");
    let watch = inbox_watch.expect("inbox watch");
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Inbox(Box::new(c::Inbox {
                        entries,
                        needs: c::Needs { proposals: 0, questions: 0, escalations: 0, person_tasks: 0 },
                        updates: c::Count { value: 1, limited: false },
                        results_seen: 0,
                        replies_seen: skein_lib::List::with_capacity(0),
                        more: false,
                    }))),
                },
            },
        },
    );
    let view = fixtures::page_view(&root);
    assert_eq!(style_count(view.tree().nodes(), look::META), 0);
}

#[test]
fn updates_do_not_repeat_the_demo_project() {
    let (mut root, _, watch, _, _) = fixtures::root_at_with_demo(
        Address::Inbox { filter: Filter::ALL },
        &fixtures::domain_limits(),
        b"Project 1",
        Some((fixtures::PROJECT, true)),
    );
    let mut entries = skein_lib::List::with_capacity(1);
    entries
        .push(c::InboxEntry {
            reply: None,
            lineage: skein_lib::List::with_capacity(0),
            goal: None,
            recipient: None,
            stall: None,
            object: c::ObjectKey::Task(10),
            project: fixtures::PROJECT,
            group: c::InboxGroup::Replies,
            position: 1,
            at: skein_lib::Wall::EPOCH,
        })
        .unwrap();
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("inbox watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Inbox(Box::new(c::Inbox {
                        entries,
                        needs: c::Needs { proposals: 0, questions: 0, escalations: 0, person_tasks: 0 },
                        updates: c::Count { value: 1, limited: false },
                        results_seen: 0,
                        replies_seen: skein_lib::List::with_capacity(0),
                        more: false,
                    }))),
                },
            },
        },
    );
    for words in [fixtures::words(), fixtures::browser_words()] {
        let mut view = fixtures::view_of(words);
        let tree = fixtures::build(&mut view, &root);
        assert_eq!(style_count(tree.nodes(), look::META), 0);
    }
}

#[test]
fn a_reply_card_names_its_task_after_the_task_ends() {
    use skein_fake_person::{Face, Find, Found, Target, TreeFace};

    let mut task = fixtures::task::document();
    task.chip.task = fixtures::INBOX_REPLY_TASK;
    task.chip.title = c::DocumentText {
        bytes: c::Text::new(Box::from(*b"Land a small change"), 32).expect("task title"),
        cut: false,
    };
    task.chip.phase = c::Phase::Ended;
    let result = c::Result {
        task: task.chip.clone(),
        ending: c::Ending::Done,
        value: None,
        at: None,
        details: None,
        acts: skein_lib::List::with_capacity(0),
    };
    let root = fixtures::inbox_with_document(
        Filter::ALL,
        Some((
            c::Read::Task { task: fixtures::INBOX_REPLY_TASK },
            c::ReadValue::Historical(Box::new(c::OwnerHistorical {
                key: c::ObjectKey::Task(fixtures::INBOX_REPLY_TASK),
                value: c::OwnerHistoricalValue::Task { document: Box::new(task), result: Box::new(result) },
                at: None,
                by: None,
            })),
        )),
    );
    let view = fixtures::page_view(&root);
    let mut face = TreeFace::new(view.tree());
    let find = Find::named(Role::Heading { level: 3 }, b"T10 Land a small change replied")
        .within(Box::new([Target::Role { role: Role::Item, name: Box::from(b"Reply from T10".as_slice()) }]));
    let found = match face.find(&find) {
        Found::Node(_) => true,
        Found::Absent | Found::Pending => false,
    };
    assert!(found, "ended task's named Item holds its reply heading");
}

#[test]
fn loaded_task_held_and_result_titles_keep_the_title_token() {
    let mut task = fixtures::task::document();
    task.chip.task = fixtures::INBOX_TASK;
    let mut reply = task.clone();
    reply.chip.task = fixtures::INBOX_REPLY_TASK;
    for (read, value) in [
        (c::Read::Task { task: fixtures::INBOX_REPLY_TASK }, c::ReadValue::Task(Box::new(reply))),
        (
            c::Read::Escalation { task: fixtures::INBOX_TASK },
            c::ReadValue::OwnerEscalation(Box::new(c::OwnerEscalation {
                task: Box::new(task.clone()),
                acts: skein_lib::List::with_capacity(0),
            })),
        ),
        (
            c::Read::Result { task: fixtures::INBOX_TASK },
            c::ReadValue::Result(Box::new(c::Result {
                task: task.chip.clone(),
                ending: c::Ending::Done,
                value: None,
                at: None,
                details: None,
                acts: skein_lib::List::with_capacity(0),
            })),
        ),
    ] {
        let root = fixtures::inbox_with_document(Filter::ALL, Some((read.clone(), value)));
        let mut loaded = false;
        for card in &root.pages().inbox().expect("inbox").cards {
            loaded |= card.page.is_some();
        }
        assert!(loaded, "{read:?}");
        let view = fixtures::page_view(&root);
        let mut found = false;
        for title in view.tree().nodes() {
            if title.role == (Role::Heading { level: 3 })
                && let Some(name) = &title.name
            {
                found |= skein_lib::bytes::find(name, task.chip.title.bytes()).is_some();
            }
        }
        assert!(found, "loaded title");
    }
}

#[test]
fn a_result_entry_marks_seen_when_its_link_is_followed() {
    let root = fixtures::inbox_root();
    let mut view = fixtures::page_view(&root);
    let tree = view.tree();
    let mut item = None;
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.role == Role::Item && node.name.as_deref() == Some(b"Updates from T9") {
            item = Some(index);
            break;
        }
    }
    let item = item.expect("result entry");
    let link = role_node(subtree(tree, item), Role::Link).id;
    assert_eq!(
        crate::act(&mut view, link, Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::OpenResult {
            task: fixtures::INBOX_TASK,
            through: fixtures::INBOX_POSITION
        }))
    );
}

fn has_named(nodes: &[Node], role: Role, name: &[u8]) -> bool {
    for node in nodes {
        if node.role == role && node.name.as_deref() == Some(name) {
            return true;
        }
    }
    false
}
fn has_styled_name(nodes: &[Node], style: skein_ui::Style, name: &[u8]) -> bool {
    for node in nodes {
        if node.styles.as_slice().contains(&style) && node.name.as_deref() == Some(name) {
            return true;
        }
    }
    false
}
#[test]
fn section_counts_use_listed_cards_and_the_sources_cut_fact() {
    for (cut, needs, updates) in [(false, b"4".as_slice(), b"2 new".as_slice()), (true, b"4+", b"2+ new")] {
        let root = fixtures::inbox_facts(Filter::ALL, None, c::Proposer::Task(fixtures::INBOX_TASK), cut, None);
        let view = fixtures::page_view(&root);
        for (section, expected) in [(b"Needs you".as_slice(), needs), (b"Updates".as_slice(), updates)] {
            assert!(has_styled_name(subtree(view.tree(), heading(view.tree(), section)), look::SUBTLE_COUNT, expected));
        }
    }
}
#[test]
fn the_kind_disclosure_names_the_choice_and_keeps_the_five_buttons_folded() {
    for (kind, name, pressed) in [
        (None, b"Kind".as_slice(), b"All kinds".as_slice()),
        (Some(Kind::Question), "Kind · Question".as_bytes(), b"Question"),
    ] {
        let view = fixtures::page_view(&fixtures::inbox_root_filtered(Filter { kind, ..Filter::ALL }));
        let tree = view.tree();
        let mut menu = None;
        for (index, node) in tree.nodes().iter().enumerate() {
            if node.role == Role::Disclosure && node.name.as_deref() == Some(name) {
                menu = Some(index);
                break;
            }
        }
        let menu = menu.unwrap();
        assert_eq!(tree.nodes()[menu].role, Role::Disclosure);
        assert_eq!(tree.nodes()[menu].name.as_deref(), Some(name));
        assert!(tree.nodes()[menu].states.is_empty());
        let mut buttons = 0_u32;
        let mut selected = 0_u32;
        for node in subtree(tree, menu) {
            if node.role == Role::Button {
                buttons = buttons.checked_add(1).unwrap();
                if node.states.as_slice().contains(&State::Pressed) {
                    selected = selected.checked_add(1).unwrap();
                    assert_eq!(node.name.as_deref(), Some(pressed));
                }
            }
        }
        assert_eq!((buttons, selected), (5, 1));
    }
}
#[test]
fn the_inbox_proposal_projects_the_same_rows_with_an_h3_and_no_chat_kicker_or_self_link() {
    let root = fixtures::inbox_with_document(
        Filter::ALL,
        Some((
            c::Read::Proposal {
                proposal: c::ProposalKey {
                    proposer: c::Proposer::Task(fixtures::INBOX_TASK),
                    number: fixtures::INBOX_QUESTION,
                },
            },
            super::cards::batch_proposal(),
        )),
    );
    let view = fixtures::page_view(&root);
    let tree = view.tree();
    let card = subtree(tree, styled(tree, look::KIND_PROPOSAL));
    assert!(has_named(card, Role::Heading { level: 3 }, b"Ship update"));
    for node in card {
        assert_ne!(node.role, Role::Heading { level: 2 });
    }
    assert_eq!(style_count(card, look::PROPOSAL_KICKER), 0);
    assert!(!has_named(card, Role::Link, "Open in inbox ↗".as_bytes()));
    for label in [b"Why".as_slice(), b"Goal", b"Landing", b"Budget"] {
        assert!(has_named(card, Role::Term, label));
    }
}
#[test]
fn reply_updates_show_the_task_heading_and_unread_age_without_the_request() {
    for (unread, expected) in [
        (None, b"12 min ago".as_slice()),
        (Some(c::Count { value: 0, limited: false }), b"12 min ago"),
        (Some(c::Count { value: 1, limited: false }), "1 new · 12 min ago".as_bytes()),
        (Some(c::Count { value: 5, limited: true }), "5+ new · 12 min ago".as_bytes()),
    ] {
        let mut task = fixtures::task::document();
        task.chip.task = fixtures::INBOX_REPLY_TASK;
        task.chip.title = c::DocumentText::complete(c::Text::new(Box::from(*b"Ship update"), 32).unwrap());
        task.last_activity = skein_lib::Wall::EPOCH;
        let request = skein_lib::bytes::copy_of(task.spec.specification.words.bytes());
        let root = fixtures::inbox_facts(
            Filter::ALL,
            Some((c::Read::Task { task: fixtures::INBOX_REPLY_TASK }, c::ReadValue::Task(Box::new(task)))),
            c::Proposer::Task(fixtures::INBOX_TASK),
            false,
            unread,
        );
        let mut view = fixtures::view();
        let mut patches = skein_lib::Queue::with_capacity(view.limits().tree.patches);
        crate::build(&mut view, &root, skein_lib::Wall::from_nanos(720_000_000_000), &mut patches);
        let card = subtree(view.tree(), styled(view.tree(), look::KIND_REPLY));
        assert!(has_named(card, Role::Heading { level: 3 }, b"T10 Ship update replied"));
        assert!(has_styled_name(card, look::UPDATE_TIME, expected));
        assert!(!has_named(card, Role::Paragraph, &request));
    }
}
#[test]
fn result_updates_name_the_ending_and_show_only_the_first_nonempty_result_line() {
    let report = c::ResultValue::Report(c::DocumentText::complete(
        c::Text::new(Box::from(*b"\n  \nFirst line\nSecond line"), 64).unwrap(),
    ));
    for (ending, value, heading) in [
        (c::Ending::Done, Some(report.clone()), b"T9 Ship update is done".as_slice()),
        (c::Ending::Deleted, Some(report.clone()), b"T9 Ship update ended"),
        (c::Ending::Task(Box::new(c::OwnerChipEnding::Done(report))), None, b"T9 Ship update is done"),
    ] {
        let mut chip = fixtures::task::document().chip;
        chip.task = fixtures::INBOX_TASK;
        chip.title = c::DocumentText::complete(c::Text::new(Box::from(*b"Ship update"), 32).unwrap());
        let result = c::Result {
            task: chip,
            ending,
            value,
            at: Some(skein_lib::Wall::EPOCH),
            details: None,
            acts: skein_lib::List::with_capacity(0),
        };
        let view = fixtures::page_view(&fixtures::inbox_with_document(
            Filter::ALL,
            Some((c::Read::Result { task: fixtures::INBOX_TASK }, c::ReadValue::Result(Box::new(result)))),
        ));
        let card = subtree(view.tree(), styled(view.tree(), look::KIND_RESULT));
        assert!(has_named(card, Role::Heading { level: 3 }, heading));
        assert!(has_named(card, Role::Paragraph, b"First line"));
        assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), b"Second line").is_none());
    }
}

#[test]
fn the_filters_row_starts_with_a_new_chat_link() {
    let root = fixtures::inbox_root();
    let mut view = fixtures::page_view(&root);
    let tree = view.tree();
    let needs = subtree(tree, heading(tree, b"Needs you"));
    let mut link = None;
    let mut filters = None;
    for (index, node) in needs.iter().enumerate() {
        if node.role == Role::Link && node.name.as_deref() == Some(b"+ New chat".as_slice()) {
            link = Some(index);
        }
        if node.styles.as_slice().contains(&look::FILTERS) {
            filters = Some(index);
        }
    }
    let link = link.expect("new chat link in heading");
    let filters = filters.expect("heading filters");
    assert_eq!(link, filters + 1, "the link is the filters' first child");
    let id = needs[link].id;
    assert_eq!(crate::href(&mut view, id).as_deref(), Some(b"/chats?at=start".as_slice()));
}

#[test]
fn the_project_filter_row_appears_only_for_two_or_more_projects() {
    for count in [1, 2] {
        let (mut root, me_watch, _) =
            fixtures::root_at(Address::Inbox { filter: Filter::ALL }, &fixtures::domain_limits());
        let mut me = root.pages().me(root.client()).unwrap().clone();
        let mut projects = skein_lib::List::with_capacity(count);
        for project in me.projects.iter().take(usize::try_from(count).unwrap()) {
            projects.push(project.clone()).unwrap();
        }
        me.projects = projects;
        fixtures::root_event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch: me_watch,
                        event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                    },
                },
            },
        );
        let view = fixtures::page_view(&root);
        let filters = subtree(view.tree(), styled(view.tree(), look::FILTERS));
        assert_eq!(has_named(filters, Role::Disclosure, b"All projects"), count >= 2);
        for name in [b"All projects".as_slice(), b"Shop", b"Second"] {
            let mut shown = false;
            for node in filters {
                shown |= node.role == Role::Button && node.name.as_deref() == Some(name);
            }
            assert_eq!(shown, count >= 2, "{name:?}, {count}");
        }
        assert!(fixtures::find(view.tree(), Role::Disclosure, b"Kind").is_some());
        assert!(fixtures::find(view.tree(), Role::Button, b"All goals").is_some());
    }
}

#[test]
fn project_and_goal_menus_follow_new_chat_and_precede_kind() {
    for (filter, projects, goals) in [
        (Filter::ALL, b"All projects".as_slice(), b"All goals".as_slice()),
        (Filter { project: Some(fixtures::PROJECT), goal: Some(7), ..Filter::ALL }, b"Shop", "Goal · T7".as_bytes()),
    ] {
        let root = fixtures::inbox_root_filtered(filter);
        let view = fixtures::page_view(&root);
        let filters = subtree(view.tree(), styled(view.tree(), look::FILTERS));
        assert_eq!(filters[1].name.as_deref(), Some(b"+ New chat".as_slice()));
        let mut names = skein_lib::List::with_capacity(3);
        for node in filters {
            if node.role == Role::Disclosure {
                names.push(node.name.as_deref().unwrap()).unwrap();
            }
        }
        assert_eq!(names.as_slice(), [projects, goals, b"Kind"]);
    }
}
#[test]
fn questions_answer_inside_a_folded_disclosure_and_keep_refusals() {
    let task = fixtures::task::document();
    let mut acts = skein_lib::List::with_capacity(1);
    acts.push(c::Offered { act: c::ActKind::Answer, admission: c::Admission::Allowed, effect: None }).unwrap();
    let page = c::Question {
        task: task.chip,
        from: None,
        message: fixtures::INBOX_QUESTION,
        words: fixtures::task::document_words(b"Which path?"),
        since: skein_lib::Wall::EPOCH,
        acts,
        ending: None,
    };
    let root = fixtures::inbox_with_document(
        Filter::ALL,
        Some((
            c::Read::Question { task: fixtures::INBOX_TASK, message: fixtures::INBOX_QUESTION },
            c::ReadValue::Question(Box::new(page)),
        )),
    );
    let mut view = fixtures::page_view(&root);
    let menu = fixtures::find(view.tree(), Role::Disclosure, b"Answer in words").expect("answer disclosure");
    let mut at = 0;
    for (index, node) in view.tree().nodes().iter().enumerate() {
        if node.id == menu {
            at = index;
        }
    }
    let nodes = subtree(view.tree(), at);
    assert!(nodes[0].states.is_empty());
    let answer = role_node(nodes, Role::Button).id;
    assert_eq!(
        crate::act(&mut view, answer, Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Decide {
            object: c::ObjectKey::Question { task: fixtures::INBOX_TASK, message: fixtures::INBOX_QUESTION },
            revision: None,
            act: c::ActKind::Answer
        }))
    );
    let card = root.pages().inbox().unwrap().cards.get(0).unwrap();
    let mut card = card.clone();
    card.outcome = Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
        refusal: c::Refusal::NotOffered,
        at: None,
    })));
    let mut builder = skein_ui::Builder::new(&fixtures::tree_limits());
    crate::confirm::card_controls(
        &mut builder,
        &root,
        &card,
        &fixtures::words(),
        &mut crate::frame::Bindings::new(128),
    );
    assert!(skein_lib::bytes::find(&fixtures::text(&builder.finish()), b"This act is not offered").is_some());
}

#[test]
fn each_needs_you_card_offers_its_task_number_as_a_terminal_command() {
    let mut view = fixtures::page_view(&fixtures::inbox_root());
    let mut controls = skein_lib::List::with_capacity(4);
    for node in view.tree().nodes() {
        if node.role == Role::Link && node.name.as_deref() == Some(b"T9") {
            controls.push(node.id).unwrap();
        }
    }
    assert_eq!(controls.len(), 4, "proposal, question, held and person task");
    for control in &controls {
        let (node, first) =
            fixtures::terminal_command(view.tree(), &view.limits().tree, *control, b"T9", "Shop › ".as_bytes());
        assert_eq!(
            crate::act(&mut view, node, first),
            Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
                address: Address::Task { task: fixtures::INBOX_TASK, filter: PlanFilter::All },
            },))
        );
    }
}
