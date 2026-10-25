use super::fixtures::{self as f, panel as p, task as t};
use crate::look;
use jig_client::{Down, Up, Watchable, contract as c};
use skein_lib::{List, Queue, Token};
use skein_ui::{Role, Tree};
use temper_client_domain::{Domain, Event, Request, forge};
use temper_client_protocol_wire::{self as wire, v1 as w};
const TASK: u64 = 9;
fn event(root: &mut Domain, up: Up) -> Queue<Request> {
    let limits = f::limits();
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    temper_client_domain::step(
        root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: f::NOW, limits },
        Event::Client { event: jig_client::Event::Up { up } },
        &mut out,
    );
    out
}
fn opened() -> (Domain, Token) {
    let mut task = t::document();
    task.chip.task = TASK;
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.procedure = Some(c::OwnerPart {
        connector: f::FORGE,
        kind: c::OwnerPartKind::Procedure,
        key: c::OwnerPartKey::Number(TASK),
    });
    task.priority = Some(3);
    task.chip.executor = c::Executor::Procedure { connector: f::FORGE + 1, kind: 1 };
    let (mut root, watch) = t::supplied_root_watching(task.clone(), false);
    task.chip.executor = c::Executor::Procedure { connector: f::FORGE, kind: 1 };
    let mut out =
        event(&mut root, Up::Watched { watch, event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))) });
    let mut family = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Open { watch, subject: Watchable::Family { .. } } },
        } = request
        {
            assert!(family.is_none());
            family = Some(watch);
            event(&mut root, Up::Watched { watch, event: c::WatchEvent::Opened });
        }
    }
    (root, family.expect("family_open"))
}
fn deliver(root: &mut Domain, watch: Token, panel: &w::ChangePanel) {
    event(
        root,
        Up::FamilyDelivered {
            watch,
            record: jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::change_panel(panel),
            },
        },
    );
}
fn parts(step: w::Step) -> w::ChangePanelParts {
    let mut parts = p::panel(TASK, step).into_parts();
    parts.owner = Box::from(*b"acme");
    parts.name = Box::from(*b"shop");
    parts.branch = Box::from(*b"temper/9");
    parts.landing = Box::from(*b"main");
    parts.pull = Some(
        w::Pull::new(
            &w::CEILINGS,
            w::PullParts { number: 23, address: Box::from(*b"https://forge.example/acme/shop/pull/23") },
        )
        .unwrap(),
    );
    parts.hold = None;
    parts.runs = None;
    parts.gates = List::with_capacity(0);
    parts
}
fn panel(parts: w::ChangePanelParts) -> w::ChangePanel {
    w::ChangePanel::new(&w::CEILINGS, parts).unwrap()
}
fn page_view(root: &Domain) -> crate::View {
    let words = f::words();
    let domain = f::limits();
    let limits = crate::Limits::of(&domain, &words).unwrap();
    let mut view = crate::View::new(&limits, &domain, words, f::FORGE);
    f::build(&mut view, root);
    view
}
fn draw(parts: w::ChangePanelParts) -> Tree {
    let (mut root, watch) = opened();
    deliver(&mut root, watch, &panel(parts));
    let words = f::words();
    let limits = crate::Limits::of(&f::limits(), &words).unwrap();
    let mut builder = skein_ui::Builder::new(&limits.tree);
    crate::pages::task::main(
        &mut builder,
        &root,
        &words,
        &mut crate::frame::Bindings::new(limits.tree.bindings),
        f::limits().client.domain.rows,
        &mut crate::words::Times::new(super::fixtures::NOW),
    );
    builder.finish()
}
fn position(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    for (index, span) in bytes.windows(needle.len()).enumerate() {
        if span == needle {
            return Some(index);
        }
    }
    None
}
fn contains(tree: &Tree, value: &str) -> bool {
    position(&f::text(tree), value.as_bytes()).is_some()
}
fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).unwrap();
    }
    let mut writer = skein_lib::Writer::new(length);
    for part in parts {
        writer.put(part).unwrap();
    }
    writer.finish()
}
fn task_name(task: u64) -> Box<[u8]> {
    joined(&[b"Task ", skein_lib::Decimal::of(task).as_bytes()])
}
fn styled(tree: &Tree, token: skein_ui::Style) -> Box<[usize]> {
    let mut found = List::with_capacity(u32::try_from(tree.nodes().len()).unwrap());
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&token) {
            found.push(index).unwrap();
        }
    }
    found.into_boxed()
}
fn queued(place: u32, names: &[u64]) -> w::Step {
    let mut ahead = List::with_capacity(u32::try_from(names.len()).unwrap());
    for n in names {
        ahead.push(*n).unwrap();
    }
    w::Step::Queued(w::Queued::new(&w::CEILINGS, w::QueuedParts { place, ahead }).unwrap())
}
fn repair(reason: w::Reason) -> w::Step {
    w::Step::Repairing(w::Repairing::new(&w::CEILINGS, w::RepairingParts { reason }).unwrap())
}
#[test]
fn shown_header_has_its_tag_pull_and_optional_safe_external_link() {
    let mut parts = parts(w::Step::Opening);
    for (pull, expected, link) in [
        (parts.pull.clone(), "PR #23 · acme/shop · temper/9 → main", true),
        (
            Some(w::Pull::new(&w::CEILINGS, w::PullParts { number: 23, address: Box::default() }).unwrap()),
            "PR #23 · acme/shop · temper/9 → main",
            false,
        ),
        (
            Some(
                w::Pull::new(&w::CEILINGS, w::PullParts { number: 23, address: Box::from(*b"javascript:bad") })
                    .unwrap(),
            ),
            "PR #23 · acme/shop · temper/9 → main",
            false,
        ),
        (None, "No pull request yet · acme/shop · temper/9 → main", false),
    ] {
        parts.pull = pull;
        let tree = draw(parts.clone());
        assert_eq!(styled(&tree, look::CHANGE_TAG).len(), 1);
        assert!(contains(&tree, "Change · Task 9"));
        assert!(!contains(&tree, "Goal · priority"));
        assert!(contains(&tree, expected));
        assert!(!contains(&tree, "Shown on the forge."));
        let mut external = List::with_capacity(1);
        for node in tree.nodes() {
            if node.role == Role::Link && node.external.is_some() {
                external.push(node).unwrap();
            }
        }
        assert_eq!(external.len(), u32::from(link));
        if link {
            assert_eq!(external.as_slice()[0].external.as_deref().unwrap(), b"https://forge.example/acme/shop/pull/23");
            assert!(contains(&tree, "View on forge ↗"));
        }
    }
}
#[test]
fn each_step_draws_its_current_done_slots_and_its_sentence() {
    for (step, current, sentence, queue_word) in [
        (w::Step::Producing, Some(0), "Producing its branch.", "Queued"),
        (w::Step::Opening, Some(1), "Opening its pull request.", "Queued"),
        (w::Step::Recreating, Some(1), "Making its branch again at the head temper last knew.", "Queued"),
        (w::Step::Reopening, Some(1), "Reopening its pull request.", "Queued"),
        (w::Step::Checking, Some(2), "Waiting for CI at its head.", "Queued"),
        (w::Step::Gating, Some(3), "Waiting for its gates at its head.", "Queued"),
        (queued(2, &[7]), Some(4), "2nd in the queue for main, after Task 7.", "Queued · 2nd"),
        (w::Step::First, Some(4), "First in the queue for main.", "Queued · 1st"),
        (w::Step::Updating, Some(4), "Updating from main.", "Queued"),
        (w::Step::Resolving, Some(4), "Resolving its conflicts with main.", "Queued"),
        (repair(w::Reason::Ci), Some(2), "Repairing: CI failed at its head.", "Queued"),
        (
            repair(w::Reason::Gate(w::GateReason::new(&w::CEILINGS, w::GateReasonParts { gate: 33 }).unwrap())),
            Some(3),
            "Repairing: gate 33 asked for changes.",
            "Queued",
        ),
        (repair(w::Reason::Semantic), Some(4), "Repairing: a semantic conflict with main.", "Queued"),
        (w::Step::Landing, Some(5), "Merging at its head.", "Queued"),
        (
            w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0xab; 32] }).unwrap()),
            None,
            "Landed · merge commit abababa",
            "Queued",
        ),
    ] {
        let tree = draw(parts(step));
        let steps = styled(&tree, look::STEP);
        assert_eq!(steps.len(), 6);
        assert_eq!(styled(&tree, look::STEP_DONE).len(), current.unwrap_or(6));
        assert_eq!(styled(&tree, look::STEP_CURRENT).len(), usize::from(current.is_some()));
        for (slot, index) in steps.iter().enumerate() {
            let node = &tree.nodes()[*index];
            assert_eq!(node.states.as_slice().contains(&skein_ui::State::Current), current == Some(slot));
            assert_eq!(
                node.states.as_slice().contains(&skein_ui::State::Completed),
                node.styles.as_slice().contains(&look::STEP_DONE)
            );
        }
        assert_eq!(f::node_text(&tree, steps[4]).as_ref(), queue_word.as_bytes());
        assert!(contains(&tree, sentence), "{sentence}");
    }
}
#[test]
fn queues_keep_the_sentence_and_each_named_tasks_link() {
    for (place, names, sentence) in [
        (1, &[][..], "1st in the queue for main."),
        (2, &[7][..], "2nd in the queue for main, after Task 7."),
        (3, &[7, 9][..], "3rd in the queue for main, after Task 7 and Task 9."),
        (4, &[7, 9][..], "4th in the queue for main, after Task 7, Task 9 and 1 more."),
        (3, &[][..], "3rd in the queue for main, after 2 tasks."),
        (2, &[][..], "2nd in the queue for main, after 1 task."),
    ] {
        let tree = draw(parts(queued(place, names)));
        let mut paragraph = None;
        for (index, node) in tree.nodes().iter().enumerate() {
            if node.role == Role::Paragraph && f::node_text(&tree, index).as_ref() == sentence.as_bytes() {
                paragraph = Some(index);
                break;
            }
        }
        let paragraph = paragraph.expect(sentence);
        let end = paragraph + usize::try_from(tree.nodes()[paragraph].size).unwrap();
        let mut links = List::with_capacity(2);
        for node in &tree.nodes()[paragraph..end] {
            if node.role == Role::Link {
                links.push(node).unwrap();
            }
        }
        assert_eq!(usize::try_from(links.len()).unwrap(), names.len());
        for (node, task) in links.as_slice().iter().zip(names) {
            assert_eq!(node.name.as_deref().unwrap(), task_name(*task).as_ref());
        }
    }
}
#[test]
fn the_running_task_has_its_link_after_the_step_line() {
    let mut parts = parts(w::Step::Producing);
    parts.runs = Some(29);
    let tree = draw(parts);
    assert!(f::find(&tree, Role::Link, b"Task 29").is_some());
    let text = f::text(&tree);
    assert!(position(&text, b"Producing its branch.").unwrap() < position(&text, b"Task 29").unwrap());
}
#[test]
fn each_hold_draws_its_step_and_exact_reason_as_a_notice() {
    for (hold, reason) in [
        (w::Hold::Stalled, "its wait stalled"),
        (w::Hold::Repairs, "its repairs are spent"),
        (w::Hold::Resolutions, "its resolutions are spent"),
        (w::Hold::Updates, "its updates are spent"),
        (w::Hold::Moved, "its branch was moved on the forge"),
        (w::Hold::Missing, "its branch is missing on the forge"),
        (w::Hold::Closed, "its pull request was closed on the forge"),
        (w::Hold::Retargeted, "its pull request was retargeted on the forge"),
        (w::Hold::Failed, "a delegate or a write failed for good"),
        (w::Hold::Rejected, "a person rejected a gate"),
    ] {
        let mut parts = parts(w::Step::First);
        parts.hold = Some(hold);
        let tree = draw(parts);
        let notice = styled(&tree, look::CHANGE_HOLD);
        assert_eq!(notice.len(), 1);
        assert_eq!(
            f::node_text(&tree, notice[0]).as_ref(),
            joined(&[b"Held while first in the queue: ", reason.as_bytes(), b"."]).as_ref()
        );
    }
}
#[test]
fn ci_gates_and_bounds_draw_the_supplied_words_in_order() {
    for (ci, state) in
        [(w::Ci::Unknown, "Not yet"), (w::Ci::Pending, "Running"), (w::Ci::Passed, "Passed"), (w::Ci::Failed, "Failed")]
    {
        let mut parts = parts(w::Step::Gating);
        parts.ci = ci;
        let tree = draw(parts);
        let rows = styled(&tree, look::GATE);
        assert_eq!(f::node_text(&tree, rows[0]).as_ref(), joined(&[b"CI at its head", state.as_bytes()]).as_ref());
        assert_eq!(f::node_text(&tree, rows[1]).as_ref(), b"No other gate.");
    }
    // A landed change's unknown CI is a restart's forgetting, not CI to come.
    for (ci, state) in [(w::Ci::Unknown, "Not read since the restart"), (w::Ci::Passed, "Passed")] {
        let mut parts =
            parts(w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0xab; 32] }).unwrap()));
        parts.ci = ci;
        let tree = draw(parts);
        let rows = styled(&tree, look::GATE);
        assert_eq!(f::node_text(&tree, rows[0]).as_ref(), joined(&[b"CI at its head", state.as_bytes()]).as_ref());
    }
    let mut parts = parts(w::Step::Gating);
    parts.gates = List::with_capacity(24);
    let mut expected = List::with_capacity(24);
    for (kind, name) in
        [(w::GateKind::Review, "review"), (w::GateKind::Approval, "approval"), (w::GateKind::Check, "check")]
    {
        for (blocks, rule) in [(true, "blocking"), (false, "advisory")] {
            for (verdict, word) in [
                (w::Verdict::None, "Not yet"),
                (w::Verdict::Pending, "Waiting"),
                (w::Verdict::Approved, "Approved"),
                (w::Verdict::Changes, "Changes asked"),
            ] {
                let number = u64::from(parts.gates.len()) + 1;
                parts
                    .gates
                    .push(
                        w::Gate::new(&w::CEILINGS, w::GateParts { number, kind: kind.clone(), blocks, verdict })
                            .unwrap(),
                    )
                    .unwrap();
                expected
                    .push(joined(&[
                        b"Gate ",
                        skein_lib::Decimal::of(number).as_bytes(),
                        " · ".as_bytes(),
                        name.as_bytes(),
                        " · ".as_bytes(),
                        rule.as_bytes(),
                        word.as_bytes(),
                    ]))
                    .unwrap();
            }
        }
    }
    let tree = draw(parts);
    let rows = styled(&tree, look::GATE);
    assert_eq!(rows.len(), 25);
    for (index, expected) in rows[1..].iter().zip(expected.as_slice()) {
        assert_eq!(f::node_text(&tree, *index).as_ref(), expected.as_ref());
    }
    assert!(!contains(&tree, "No other gate."));
    assert!(contains(&tree, "Repairs 1 of 2 · resolutions 3 of 4 · updates 5 of 6"));
}
#[test]
fn unshown_panels_say_where_they_are_and_other_subjects_draw_nothing() {
    for (end, word) in [
        (None, "Loading the change from the forge."),
        (Some(c::WatchEnd::Unknown), "The forge knows no change for this task."),
        (Some(c::WatchEnd::NotAllowed), "The forge cannot show this change."),
    ] {
        let (mut root, watch) = opened();
        if let Some(end) = end {
            event(&mut root, Up::WatchEnded { watch, end });
        }
        let view = page_view(&root);
        let tree = view.tree();
        assert!(contains(tree, word));
        assert!(styled(tree, look::CHANGE).is_empty());
        assert!(!contains(tree, "Change · Task"));
    }
    let (mut root, _) = opened();
    f::root_event(
        &mut root,
        Event::Forge(forge::Event::Panel { subject: Some(forge::panel::Subject { task: 77, project: 1 }) }),
    );
    let view = page_view(&root);
    assert!(!contains(view.tree(), "Loading the change"));
    assert!(contains(view.tree(), "Shown on the forge."));
    assert!(styled(view.tree(), look::CHANGE).is_empty());
    let root = t::root(c::Executor::Procedure { connector: f::FORGE + 1, kind: 1 }, None);
    assert!(styled(page_view(&root).tree(), look::CHANGE).is_empty());
}
#[test]
fn the_card_keeps_the_parts_in_design_order() {
    let mut parts = parts(w::Step::Producing);
    parts.runs = Some(29);
    parts.hold = Some(w::Hold::Moved);
    let tree = draw(parts);
    let text = f::text(&tree);
    let mut previous = 0;
    for part in [
        "Change · Task 9",
        "PR #23",
        "View on forge ↗",
        "ProducingOpeningCheckingGatingQueuedLanded",
        "Producing its branch.",
        "Task 29",
        "Held while",
        "Its tasks",
        "Gates at its head",
        "CI at its head",
        "No other gate.",
        "Repairs 1 of 2",
    ] {
        let at = position(&text, part.as_bytes()).expect(part);
        assert!(at >= previous, "{part}");
        previous = at;
    }
}
#[test]
fn a_full_bound_panel_fits_the_production_tree_text_and_links() {
    let (mut root, watch) = opened();
    let panel = p::panel(TASK, p::queued());
    deliver(&mut root, watch, &panel);
    let view = page_view(&root);
    let tree = view.tree();
    assert_eq!(styled(tree, look::GATE).len(), 65);
    assert_eq!(styled(tree, look::STEP).len(), 6);
    let text = f::text(tree);
    assert!(position(&text, panel.branch()).is_some());
    for task in 1_u64..=64 {
        assert!(f::find(tree, Role::Link, &task_name(task)).is_some());
    }
    assert!(contains(tree, "Repairs 1 of 2 · resolutions 3 of 4 · updates 5 of 6"));
}

#[test]
fn a_full_length_safe_forge_address_fits_with_its_label() {
    let mut parts = p::panel(TASK, p::queued()).into_parts();
    let mut address = skein_lib::bytes::zeroed(usize::try_from(wire::ADDRESS_BYTES).unwrap());
    address.fill(b'a');
    for (byte, prefix) in address.iter_mut().zip(b"https://forge.example/") {
        *byte = *prefix;
    }
    parts.pull = Some(w::Pull::new(&w::CEILINGS, w::PullParts { number: u64::MAX, address: address.clone() }).unwrap());
    let tree = draw(parts);
    let mut links = List::with_capacity(1);
    for node in tree.nodes() {
        if node.external.is_some() {
            links.push(node).unwrap();
        }
    }
    assert_eq!(links.len(), 1);
    assert_eq!(links.as_slice()[0].external.as_deref().unwrap(), address.as_ref());
    assert!(contains(&tree, "View on forge ↗"));
}
