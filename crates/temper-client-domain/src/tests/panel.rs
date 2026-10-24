use super::fixtures::{self as f, panel as p};
use crate::{
    Address, Domain, Event, Place, Request,
    forge::{
        self,
        panel::{State, Subject},
    },
};
use crate::{ChatsFilter, PlanFilter};
use jig_client::{Down, Handle, Told, Up, Watchable, Why, contract as c};
use skein_lib::{List, Queue, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};

const TASK: u64 = 17;
const NEXT: u64 = 19;
const PROJECT: u32 = f::PROJECT + 1;
const SUBJECT: Subject = Subject { task: TASK, project: PROJECT };
const PROCEDURE: c::Executor = c::Executor::Procedure { connector: f::FORGE, kind: 1234 };

fn event(root: &mut Domain, event: Event) -> Queue<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::step(root, &f::env(&f::limits()), event, &mut out);
    out
}
fn up(root: &mut Domain, up: Up) -> Queue<Request> {
    event(root, Event::Client { event: jig_client::Event::Up { up } })
}
fn went(root: &mut Domain, place: Place) -> Queue<Request> {
    event(root, Event::Went { place })
}
fn task_page(root: &mut Domain, task: u64) -> Token {
    let mut out = went(root, Place::Address(Address::Task { task, filter: PlanFilter::All }));
    let mut found = None;
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Open { watch, subject } } } = request
        {
            if subject == Watchable::Jig(c::Subject::Task { task }) {
                found = Some(watch);
            }
            up(root, Up::Watched { watch, event: c::WatchEvent::Opened });
        }
    }
    found.expect("task watch")
}
fn before_document() -> (Domain, Token) {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let frame = f::start(&mut root, List::with_capacity(0));
    f::confirm(&mut root, frame, f::PARTY, &mut Queue::with_capacity(crate::max_out(&f::limits())));
    let watch = task_page(&mut root, TASK);
    (root, watch)
}
fn document(root: &mut Domain, watch: Token, task: u64, executor: c::Executor) -> Queue<Request> {
    up(
        root,
        Up::Watched {
            watch,
            event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(p::task(task, PROJECT, executor)))),
        },
    )
}
fn family_open(out: &mut Queue<Request>, task: u64) -> Option<Token> {
    let mut found = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                jig_client::Request::Down {
                    down: Down::Open { watch, subject: Watchable::Family { scope, window, record } },
                },
        } = request
        {
            assert!(found.is_none(), "one panel watch");
            assert_eq!(scope, c::Scope::Project(PROJECT));
            assert_eq!(window, c::Window { rows: 0, after: None });
            assert_eq!((record.family, record.version), (wire::FORGE, wire::CLIENT_VERSION));
            let watching =
                wire::decode_watching(&record.bytes, &forge::wire_limits(&f::limits().forge).unwrap()).unwrap();
            let w::Watch::Change(subject) = watching.watch() else { panic!("change watch") };
            assert_eq!(subject.task(), task);
            found = Some(watch);
        }
    }
    found
}
fn opened() -> (Domain, Token, Handle) {
    let (mut root, task) = before_document();
    let watch = family_open(&mut document(&mut root, task, TASK, PROCEDURE), TASK).expect("panel watch");
    let panel = root.forge().panel().expect("panel");
    assert_eq!(panel.subject, SUBJECT);
    assert_eq!(panel.state, State::Loading);
    let handle = panel.handle.expect("open handle");
    assert_eq!(handle.asker, f::FORGE_ASKER);
    assert_ne!(handle.number, 0);
    up(&mut root, Up::Watched { watch, event: c::WatchEvent::Opened });
    (root, watch, handle)
}
fn deliver(root: &mut Domain, watch: Token, panel: &w::ChangePanel) -> Queue<Request> {
    up(
        root,
        Up::FamilyDelivered {
            watch,
            record: jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::change_panel(panel),
            },
        },
    )
}
fn closes(out: &mut Queue<Request>) -> List<Token> {
    let mut watches = List::with_capacity(out.len());
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Close { watch } } } = request {
            watches.push(watch).unwrap();
        }
    }
    watches
}
#[test]
fn only_a_forge_procedure_document_opens_its_projects_panel() {
    for (executor, watches) in [
        (PROCEDURE, true),
        (c::Executor::Procedure { connector: f::FORGE, kind: 1 }, true),
        (c::Executor::Agent { charter: 1 }, false),
        (c::Executor::Party { party: f::PARTY }, false),
        (c::Executor::Role { project: PROJECT, role: 1 }, false),
        (c::Executor::Procedure { connector: f::FORGE + 1, kind: 1 }, false),
    ] {
        let (mut root, task) = before_document();
        assert!(root.forge().panel().is_none(), "before its document");
        assert_eq!(
            family_open(&mut document(&mut root, task, TASK, executor), TASK).is_some(),
            watches,
            "{executor:?}"
        );
        assert_eq!(root.forge().panel().is_some(), watches);
        assert!(family_open(&mut document(&mut root, task, TASK, executor), TASK).is_none(), "same subject");
    }
}
#[test]
fn leaving_closes_the_panel_and_reopening_uses_a_fresh_handle() {
    for place in [
        Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }),
        Place::Address(Address::Adopt { project: PROJECT }),
    ] {
        let (mut root, watch, first) = opened();
        let closed = closes(&mut went(&mut root, place.clone()));
        assert!(closed.as_slice().contains(&watch), "{place:?}");
        assert!(root.forge().panel().is_none());
        for &watch in &closed {
            up(&mut root, Up::WatchEnded { watch, end: c::WatchEnd::Closed });
        }
        let task = task_page(&mut root, TASK);
        let next_watch = family_open(&mut document(&mut root, task, TASK, PROCEDURE), TASK).expect("reopened panel");
        assert_ne!(root.forge().panel().unwrap().handle, Some(first));
        assert_ne!(next_watch, watch);
    }
}
#[test]
fn another_tasks_page_closes_the_old_panel_and_watches_its_own_subject() {
    let (mut root, watch, first) = opened();
    // task_page acknowledges the task and tree openings; the old panel closes on navigation.
    let mut out = went(&mut root, Place::Address(Address::Task { task: NEXT, filter: PlanFilter::All }));
    let mut task = None;
    let mut closed = List::with_capacity(out.len());
    while let Some(request) = out.pop() {
        match request {
            Request::Client { request: jig_client::Request::Down { down: Down::Close { watch } } } => {
                closed.push(watch).unwrap();
            }
            Request::Client { request: jig_client::Request::Down { down: Down::Open { watch, subject } } } => {
                if subject == Watchable::Jig(c::Subject::Task { task: NEXT }) {
                    task = Some(watch);
                }
                up(&mut root, Up::Watched { watch, event: c::WatchEvent::Opened });
            }
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => {}
        }
    }
    assert!(closed.as_slice().contains(&watch));
    assert!(root.forge().panel().is_none());
    for &watch in &closed {
        up(&mut root, Up::WatchEnded { watch, end: c::WatchEnd::Closed });
    }
    let next =
        family_open(&mut document(&mut root, task.expect("next task"), NEXT, PROCEDURE), NEXT).expect("next panel");
    assert_ne!(root.forge().panel().unwrap().handle, Some(first));
    assert_ne!(watch, next);
    assert_eq!(root.forge().panel().unwrap().subject, Subject { task: NEXT, project: PROJECT });
}
#[test]
fn deliveries_replace_the_whole_decoded_panel() {
    let (mut root, watch, _) = opened();
    for step in [
        w::Step::Producing,
        p::queued(),
        w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0x47; 32] }).unwrap()),
    ] {
        let panel = p::panel(TASK, step);
        let before = forge::shown(root.forge());
        assert!(deliver(&mut root, watch, &panel).is_empty());
        assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(panel.clone())));
        assert_ne!(forge::shown(root.forge()), before);
        let shown = forge::shown(root.forge());
        assert!(deliver(&mut root, watch, &panel).is_empty());
        assert_eq!(forge::shown(root.forge()), shown, "identical delivery");
    }
}
#[test]
fn invalid_records_end_unknown_and_unwatch_once() {
    let panel = p::panel(TASK, w::Step::Opening);
    let bytes = wire::change_panel(&panel);
    let mut version = bytes.clone();
    version[1] = u8::try_from(wire::CLIENT_VERSION + 1).unwrap();
    let mut trailing = skein_lib::bytes::zeroed(bytes.len() + 1);
    for (target, byte) in trailing.iter_mut().zip(bytes.iter()) {
        *target = *byte;
    }
    for (row, family, version, bytes) in [
        ("version metadata", wire::FORGE, wire::CLIENT_VERSION + 1, bytes.clone()),
        ("version bytes", wire::FORGE, wire::CLIENT_VERSION, version),
        ("family", wire::FORGE + 1, wire::CLIENT_VERSION, bytes),
        ("undecodable", wire::FORGE, wire::CLIENT_VERSION, Box::from([255])),
        ("trailing", wire::FORGE, wire::CLIENT_VERSION, trailing),
        ("other task", wire::FORGE, wire::CLIENT_VERSION, wire::change_panel(&p::panel(NEXT, w::Step::Opening))),
    ] {
        let (mut root, task_watch, watch, mut task) = change_opened();
        let out =
            &mut up(&mut root, Up::FamilyDelivered { watch, record: jig_client::Family { family, version, bytes } });
        assert_eq!(closes(out).as_slice(), &[watch], "{row}");
        assert_eq!(root.forge().panel().unwrap().state, State::Unknown, "{row}");
        assert_eq!(root.forge().panel().unwrap().handle, None);
        assert!(deliver(&mut root, watch, &panel).is_empty(), "late delivery");
        assert_eq!(root.forge().panel().unwrap().state, State::Unknown);
        assert!(
            family_open(
                &mut went(&mut root, Place::Address(Address::Task { task: TASK, filter: PlanFilter::All })),
                TASK
            )
            .is_none()
        );
        assert!(
            up(&mut root, Up::WatchEnded { watch, end: c::WatchEnd::Closed }).is_empty(),
            "local unwatch acknowledgement"
        );
        task.activity += 1;
        assert!(
            family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(),
            "{row}: terminal after a later task document"
        );
        let closed =
            closes(&mut went(&mut root, Place::Address(Address::Chats { filter: ChatsFilter::All, start: false })));
        assert!(!closed.as_slice().contains(&watch), "already unwatched");
        for &watch in &closed {
            up(&mut root, Up::WatchEnded { watch, end: c::WatchEnd::Closed });
        }
        assert!(root.forge().panel().is_none());
        let task = task_page(&mut root, TASK);
        assert!(family_open(&mut document(&mut root, task, TASK, PROCEDURE), TASK).is_some(), "opened afresh");
        assert_eq!(root.forge().panel().unwrap().state, State::Loading);
    }
}
#[test]
fn jig_endings_keep_unknown_and_other_reasons_apart_from_loading() {
    for (end, expected) in [
        (c::WatchEnd::Unknown, State::Unknown),
        (c::WatchEnd::NotAllowed, State::Ended(Why::NotAllowed)),
        (c::WatchEnd::SignedOut, State::Ended(Why::SignedOut)),
        (c::WatchEnd::SubjectEnded, State::Unknown),
        (c::WatchEnd::NotServed, State::Ended(Why::NotServed)),
    ] {
        let (mut root, task_watch, watch, mut task) = change_opened();
        let shown = forge::shown(root.forge());
        assert!(up(&mut root, Up::WatchEnded { watch, end }).is_empty());
        assert_eq!(root.forge().panel().unwrap().state, expected);
        assert_ne!(forge::shown(root.forge()), shown);
        assert!(root.forge().panel().unwrap().handle.is_none());
        assert!(
            family_open(
                &mut went(&mut root, Place::Address(Address::Task { task: TASK, filter: PlanFilter::All })),
                TASK
            )
            .is_none()
        );
        if end != c::WatchEnd::SubjectEnded {
            task.activity += 1;
            assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(), "{end:?}");
            assert_eq!(root.forge().panel().unwrap().state, expected);
            assert_eq!(root.forge().panel().unwrap().handle, None);
        }
        let closed =
            closes(&mut went(&mut root, Place::Address(Address::Chats { filter: ChatsFilter::All, start: false })));
        assert!(!closed.as_slice().contains(&watch), "jig already ended it");
        for &watch in &closed {
            up(&mut root, Up::WatchEnded { watch, end: c::WatchEnd::Closed });
        }
        assert!(root.forge().panel().is_none());
        let task = task_page(&mut root, TASK);
        assert!(family_open(&mut document(&mut root, task, TASK, PROCEDURE), TASK).is_some(), "opened afresh");
        assert_eq!(root.forge().panel().unwrap().state, State::Loading);
    }
}
fn change_task() -> c::Task {
    let mut task = p::task(TASK, PROJECT, c::Executor::Procedure { connector: f::FORGE, kind: 2 });
    task.result_contract = c::TaskContractDocument::Change { connector: f::FORGE, kind: 1, words: 64 };
    task
}
fn task_document(root: &mut Domain, watch: Token, task: &c::Task) -> Queue<Request> {
    up(root, Up::Watched { watch, event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task.clone()))) })
}
fn change_opened() -> (Domain, Token, Token, c::Task) {
    let (mut root, task_watch) = before_document();
    let task = change_task();
    let panel_watch = family_open(&mut task_document(&mut root, task_watch, &task), TASK).expect("panel watch");
    up(&mut root, Up::Watched { watch: panel_watch, event: c::WatchEvent::Opened });
    (root, task_watch, panel_watch, task)
}
fn finish_change(task: &mut c::Task) {
    task.activity = task.activity.checked_add(1).expect("one finished task update");
    f::chat::phase(task, c::OwnerChipPhase::Ended);
    let mut value = f::chat::change_value(b"abcdef0123456789abcdef0123456789abcdef0123");
    let c::ResultValue::Change { part, .. } = &mut value else { panic!("change result") };
    part.key = c::OwnerPartKey::Number(TASK);
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(value))));
}
#[test]
fn a_panel_falling_behind_before_its_first_record_retries_and_shows_the_landed_change() {
    for end in [c::WatchEnd::Busy, c::WatchEnd::Dropped, c::WatchEnd::Stopping] {
        let (mut root, task_watch) = before_document();
        let mut task = change_task();
        let first = family_open(&mut task_document(&mut root, task_watch, &task), TASK).expect("panel watch");
        // Jig reports Behind without a latest record while it backs off this opening.
        assert!(up(&mut root, Up::WatchEnded { watch: first, end }).is_empty());
        finish_change(&mut task);
        assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none());
        let limits = f::limits();
        let mut env = f::env(&limits);
        env.now = env.now.saturating_add(skein_lib::Duration::from_secs(10));
        let mut out = Queue::with_capacity(crate::max_out(&limits));
        crate::fire(&mut root, &env, &mut out);
        let fresh = family_open(&mut out, TASK)
            .expect("a panel without its first record must retain jig's retry after falling behind");
        assert_ne!(fresh, first);
        let landed = p::panel(
            TASK,
            w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0x47; 32] }).unwrap()),
        );
        for up in [
            Up::Watched { watch: fresh, event: c::WatchEvent::Opened },
            Up::FamilyDelivered {
                watch: fresh,
                record: jig_client::Family {
                    family: wire::FORGE,
                    version: wire::CLIENT_VERSION,
                    bytes: wire::change_panel(&landed),
                },
            },
        ] {
            crate::step(&mut root, &env, Event::Client { event: jig_client::Event::Up { up } }, &mut out);
            assert!(out.is_empty());
        }
        assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(landed)), "{end:?}");
    }
}
#[test]
fn a_late_panel_ending_after_the_last_task_document_reopens_the_panel() {
    let (mut root, task_watch, panel_watch, mut task) = change_opened();
    finish_change(&mut task);
    assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(), "watch still open");
    up(&mut root, Up::WatchEnded { watch: task_watch, end: c::WatchEnd::SubjectEnded });
    let fresh =
        family_open(&mut up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }), TASK)
            .expect("a finished task cannot await another document after its late panel ending");
    assert_ne!(fresh, panel_watch);
    assert!(up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }).is_empty());
    assert!(up(&mut root, Up::Watched { watch: fresh, event: c::WatchEvent::Opened }).is_empty());
    assert_eq!(root.forge().panel().unwrap().state, State::Loading);
    assert!(root.forge().panel().unwrap().handle.is_some());
    let landed =
        p::panel(TASK, w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0x47; 32] }).unwrap()));
    assert!(deliver(&mut root, fresh, &landed).is_empty());
    assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(landed)));
}
#[test]
fn only_a_finished_change_reopens_its_panel_on_the_ending() {
    for row in ["live", "done phase", "failed phase", "cancelled phase", "no result", "report", "failed result"] {
        let (mut root, task_watch, panel_watch, mut task) = change_opened();
        finish_change(&mut task);
        match row {
            "live" => task.chip.phase = c::Phase::Active,
            "done phase" => task.chip.phase = c::Phase::Done,
            "failed phase" => task.chip.phase = c::Phase::Failed,
            "cancelled phase" => task.chip.phase = c::Phase::Cancelled,
            "no result" => task.result = None,
            "report" => {
                task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Report(
                    task.chip.title.clone(),
                )))));
            }
            "failed result" => {
                task.result =
                    Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(task.chip.title.clone()))));
            }
            _ => unreachable!(),
        }
        assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none());
        assert!(
            up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }).is_empty(),
            "{row}"
        );
        assert_eq!(root.forge().panel().unwrap().state, State::Unknown, "{row}");
        assert_eq!(root.forge().panel().unwrap().handle, None, "{row}");
    }
}
#[test]
fn two_panel_endings_on_the_same_finished_document_reopen_only_once() {
    let (mut root, task_watch, panel_watch, mut task) = change_opened();
    finish_change(&mut task);
    assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none());
    let fresh =
        family_open(&mut up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }), TASK)
            .expect("one immediate reopen on the finished document");
    assert!(up(&mut root, Up::Watched { watch: fresh, event: c::WatchEvent::Opened }).is_empty());
    assert!(up(&mut root, Up::WatchEnded { watch: fresh, end: c::WatchEnd::SubjectEnded }).is_empty());
    assert_eq!(root.forge().panel().unwrap().state, State::Unknown);
    assert_eq!(root.forge().panel().unwrap().handle, None);
    assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(), "same document");
    task.activity += 1;
    let next = family_open(&mut task_document(&mut root, task_watch, &task), TASK)
        .expect("a new document can recover the unknown panel");
    assert_ne!(next, fresh);
    assert!(up(&mut root, Up::Watched { watch: next, event: c::WatchEvent::Opened }).is_empty());
    assert_eq!(root.forge().panel().unwrap().state, State::Loading);
    assert!(root.forge().panel().unwrap().handle.is_some());
}
#[test]
fn a_panel_ending_before_the_last_task_document_reopens_exactly_once() {
    let (mut root, task_watch, panel_watch, mut task) = change_opened();
    let mut out = up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded });
    finish_change(&mut task);
    let mut final_delivery = task_document(&mut root, task_watch, &task);
    let before = family_open(&mut out, TASK);
    let after = family_open(&mut final_delivery, TASK);
    assert!(before.is_none(), "a live task waits for its later document");
    let fresh = after.expect("one reopening when the finished document arrives");
    assert_ne!(fresh, panel_watch);
    assert!(up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }).is_empty());
    assert!(up(&mut root, Up::Watched { watch: fresh, event: c::WatchEvent::Opened }).is_empty());
    assert_eq!(root.forge().panel().unwrap().state, State::Loading);
}
#[test]
fn leaving_before_a_late_panel_ending_asks_nothing() {
    for address in [
        Address::Chats { filter: ChatsFilter::All, start: false },
        Address::Task { task: NEXT, filter: PlanFilter::All },
    ] {
        let (mut root, task_watch, panel_watch, mut task) = change_opened();
        finish_change(&mut task);
        task_document(&mut root, task_watch, &task);
        let handle = root.forge().panel().unwrap().handle.unwrap();
        let mut out = went(&mut root, Place::Address(address));
        assert!(closes(&mut out).as_slice().contains(&panel_watch));
        assert!(up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded }).is_empty());
        let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
        crate::route::told(&mut root, &f::env(&f::limits()), Told::Ended { handle, why: Why::SubjectEnded }, &mut out);
        assert!(out.is_empty(), "a stale child ending cannot reopen a panel after leaving");
        assert!(root.forge().panel().is_none());
    }
}
#[test]
fn a_task_page_recovers_its_ended_panel_when_the_change_is_made_and_landed() {
    for end in [c::WatchEnd::SubjectEnded, c::WatchEnd::Closed] {
        let (mut root, task_watch, mut panel_watch, mut task) = change_opened();
        let mut handle = root.forge().panel().unwrap().handle.unwrap();
        for landed in [false, true] {
            assert!(up(&mut root, Up::WatchEnded { watch: panel_watch, end }).is_empty());
            assert_eq!(root.forge().panel().unwrap().state, State::Unknown);
            assert_eq!(root.forge().panel().unwrap().handle, None);
            // Neither a repeated end nor revisiting this address is a new task document.
            assert!(up(&mut root, Up::WatchEnded { watch: panel_watch, end }).is_empty());
            assert!(
                family_open(
                    &mut went(&mut root, Place::Address(Address::Task { task: TASK, filter: PlanFilter::All })),
                    TASK
                )
                .is_none()
            );
            task.activity = task.activity.checked_add(1).expect("two task updates");
            if landed {
                f::chat::phase(&mut task, c::OwnerChipPhase::Ended);
                let mut value = f::chat::change_value(b"abcdef0123456789abcdef0123456789abcdef0123");
                let c::ResultValue::Change { part, .. } = &mut value else { panic!("change result") };
                part.key = c::OwnerPartKey::Number(TASK);
                task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(value))));
            }
            let next = family_open(&mut task_document(&mut root, task_watch, &task), TASK)
                .expect("one fresh watch per later task document");
            assert_ne!(next, panel_watch);
            let panel = root.forge().panel().unwrap();
            assert_eq!(panel.subject, SUBJECT);
            assert_eq!(panel.state, State::Loading);
            assert_ne!(panel.handle, Some(handle));
            handle = panel.handle.unwrap();
            panel_watch = next;
            up(&mut root, Up::Watched { watch: panel_watch, event: c::WatchEvent::Opened });
        }
        let page = root.pages().task().expect("same task page");
        let document = page.document(root.client()).expect("landed task document");
        assert_eq!(document.chip.phase, c::Phase::Ended);
        assert_eq!(crate::pages::delegation::facts(document).state, crate::pages::delegation::State::Landed);
        let landed = p::panel(
            TASK,
            w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0x47; 32] }).unwrap()),
        );
        assert!(deliver(&mut root, panel_watch, &landed).is_empty());
        assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(landed)));
        assert!(
            family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(),
            "a live watch needs no replacement"
        );
    }
}
#[test]
fn an_ended_panel_reopens_only_for_its_live_change_task_or_change_result() {
    for row in [
        "another task",
        "another procedure",
        "another connector",
        "another contract",
        "ended without result",
        "failed",
        "cancelled",
    ] {
        let (mut root, task_watch, panel_watch, mut task) = change_opened();
        up(&mut root, Up::WatchEnded { watch: panel_watch, end: c::WatchEnd::SubjectEnded });
        task.activity += 1;
        match row {
            "another task" => {
                // A delivery on a different task's watch is not this page's document.
                let other = Handle { asker: root.pages().task().unwrap().document.asker, number: 9999 };
                let mut out = event(
                    &mut root,
                    Event::Client {
                        event: jig_client::Event::Watch {
                            handle: other,
                            subject: Watchable::Jig(c::Subject::Task { task: NEXT }),
                        },
                    },
                );
                let mut other_watch = None;
                while let Some(request) = out.pop() {
                    if let Request::Client {
                        request:
                            jig_client::Request::Down {
                                down: Down::Open { watch, subject: Watchable::Jig(c::Subject::Task { task: NEXT }) },
                            },
                    } = request
                    {
                        other_watch = Some(watch);
                    }
                }
                task.chip.task = NEXT;
                assert!(
                    family_open(&mut task_document(&mut root, other_watch.expect("another task watch"), &task), TASK)
                        .is_none()
                );
                assert_eq!(root.forge().panel().unwrap().state, State::Unknown);
                assert_eq!(root.forge().panel().unwrap().handle, None);
                continue;
            }
            "another procedure" => task.chip.executor = PROCEDURE,
            "another connector" => {
                task.result_contract = c::TaskContractDocument::Change { connector: f::FORGE + 1, kind: 1, words: 64 }
            }
            "another contract" => task.result_contract = p::task(TASK, PROJECT, PROCEDURE).result_contract,
            "ended without result" => f::chat::phase(&mut task, c::OwnerChipPhase::Ended),
            "failed" | "cancelled" => {
                f::chat::phase(&mut task, c::OwnerChipPhase::Ended);
                task.chip.phase = if row == "failed" { c::Phase::Failed } else { c::Phase::Cancelled };
            }
            _ => unreachable!(),
        }
        assert!(family_open(&mut task_document(&mut root, task_watch, &task), TASK).is_none(), "{row}");
        assert_eq!(root.forge().panel().unwrap().state, State::Unknown, "{row}");
        assert_eq!(root.forge().panel().unwrap().handle, None, "{row}");
    }
}
#[test]
fn a_late_tell_from_the_previous_subject_changes_nothing() {
    let (mut root, watch, first) = opened();
    let mut out =
        event(&mut root, Event::Forge(forge::Event::Panel { subject: Some(Subject { task: NEXT, project: PROJECT }) }));
    // This event exercises the side's two-output replacement through the root.
    assert!(out.len() <= crate::max_out(&f::limits()));
    let next = family_open(&mut out, NEXT).expect("replacement");
    let handle = root.forge().panel().unwrap().handle.unwrap();
    assert_ne!(handle, first);
    let shown = forge::shown(root.forge());
    for told in [Told::Changed { handle: first }, Told::Ended { handle: first, why: Why::Unknown }] {
        let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
        crate::route::told(&mut root, &f::env(&f::limits()), told, &mut out);
        assert!(out.is_empty());
        assert_eq!(forge::shown(root.forge()), shown);
        assert_eq!(root.forge().panel().unwrap().handle, Some(handle));
        assert_eq!(root.forge().panel().unwrap().state, State::Loading);
    }
    deliver(&mut root, watch, &p::panel(TASK, w::Step::Producing));
    deliver(&mut root, next, &p::panel(NEXT, w::Step::Producing));
    assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(p::panel(NEXT, w::Step::Producing))));
}
#[test]
fn output_and_memory_bounds_cover_replacement_and_a_full_panel() {
    const PANEL_REPLACEMENT_REQUESTS: u32 = 2; // Unwatch followed by watch.
    let limits = f::limits();
    let mut side = forge::Side::new(&limits.forge, f::SEED);
    let mut out = Queue::with_capacity(forge::max_out(&limits.forge));
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: limits.forge };
    forge::step(&mut side, &env, forge::Event::Panel { subject: Some(SUBJECT) }, &mut out);
    let Some(forge::Request::Client { asked: jig_client::Event::Watch { handle: first, .. } }) = out.pop() else {
        panic!("watch")
    };
    forge::step(
        &mut side,
        &env,
        forge::Event::Panel { subject: Some(Subject { task: NEXT, project: PROJECT }) },
        &mut out,
    );
    assert_eq!(out.len(), PANEL_REPLACEMENT_REQUESTS);
    let Some(forge::Request::Client { asked: jig_client::Event::Unwatch { handle } }) = out.pop() else {
        panic!("unwatch first")
    };
    assert_eq!(handle, first);
    let Some(forge::Request::Client { asked: jig_client::Event::Watch { handle, .. } }) = out.pop() else {
        panic!("watch next")
    };
    assert_ne!(handle, first);
    let (mut root, watch, _) = opened();
    let panel = p::panel(TASK, p::queued());
    deliver(&mut root, watch, &panel);
    assert_eq!(root.forge().panel().unwrap().state, State::Shown(Box::new(panel.clone())));
    let panel_heap = size_of::<w::ChangePanel>()
        + panel.owner().len()
        + panel.name().len()
        + panel.branch().len()
        + panel.landing().len()
        + panel.pull().as_ref().unwrap().address().len()
        + usize::try_from(wire::GATES).unwrap() * size_of::<w::Gate>()
        + usize::try_from(wire::TASKS_AHEAD).unwrap() * size_of::<u64>();
    let wire = forge::wire_limits(&limits.forge).unwrap();
    assert_eq!(
        u64::try_from(panel_heap).unwrap(),
        u64::try_from(size_of::<w::ChangePanel>()).unwrap() + w::ChangePanel::worst_case_heap(&wire).unwrap()
    );
    let existing = u64::from(limits.forge.field_bytes) * 6
        + u64::from(limits.forge.checks) * 4
        + w::DemoAnswer::worst_case_heap(&wire).unwrap()
        + w::Answer::worst_case_heap(&wire).unwrap()
        + u64::from(wire.group_failed_why)
        + forge::forges::worst_case(&limits.forge).unwrap()
        + forge::picker::worst_case(limits.forge).unwrap()
        // Admin token, four fields and its answer.
        + 4090 + u64::from(limits.forge.field_bytes) * 3
        + u64::from(limits.forge.field_bytes.max(wire.first_repository_label))
        + w::Answer::worst_case_heap(&wire).unwrap()
        // App draft key, answered install link and editable App id.
        + 4097 + 2048 + u64::from(limits.forge.field_bytes)
        + u64::try_from(size_of::<Handle>()).unwrap();
    assert_eq!(forge::worst_case(&limits.forge), Some(existing + u64::try_from(panel_heap).unwrap()));
    assert!(crate::worst_case(&limits).unwrap() >= forge::worst_case(&limits.forge).unwrap());
}
