use super::fixtures;
use crate::ChatsFilter;
use crate::pages::{Request, chats::Chats};
use jig_client::{Event, Readable, Readout, Told, Watchable, contract::*};
use skein_lib::{List, Queue};
const FIRST: u64 = 1;
const OUTPUTS: u32 = 16;
const CURSOR_BYTES: u32 = 8;
const SERIAL: u64 = 13;
#[test]
fn starting_a_chat_sends_the_chat_template_filled_once() {
    let limits = fixtures::limits();
    let me = fixtures::chats::me();
    let mut chats = Chats::new(FIRST);
    let mut out = Queue::with_capacity(OUTPUTS);
    let mut serial = SERIAL;
    chats.write(Box::from(*b"Hello"), &limits);
    chats.start(
        &fixtures::env(&limits),
        Some(&me),
        Some(fixtures::PROJECT),
        &crate::pages::choice::Draft::default(),
        &mut serial,
        &mut out,
    );
    chats.start(
        &fixtures::env(&limits),
        Some(&me),
        Some(fixtures::PROJECT),
        &crate::pages::choice::Draft::default(),
        &mut serial,
        &mut out,
    );
    let expected = jig_client::shapes::chat(&me, fixtures::PROJECT, fixtures::acts::text(b"Hello")).expect("template");
    assert_eq!(fixtures::acts::sent(&mut out).as_ref(), [expected]);
}
#[test]
fn the_chats_watch_live_lines_by_last_activity() {
    let limits = fixtures::limits();
    let me = fixtures::chats::me();
    let mut chats = Chats::new(FIRST);
    let mut out = Queue::with_capacity(OUTPUTS);
    chats.configure(Some(&me), Some(fixtures::PROJECT), &limits, &mut out);
    let query = TaskQuery {
        project: Some(fixtures::PROJECT),
        requester: Some(OwnerRequester::Person(fixtures::PARTY)),
        label: Some(7),
        root: None,
        parent: None,
        phase: None,
        executor: None,
        tracked: None,
        order: QueryOrder::Activity,
    };
    assert_eq!(
        out.pop(),
        Some(Request::Client(Event::Watch {
            handle: chats.live,
            subject: Watchable::Jig(Subject::Tasks { query, window: limits.client.domain.rows })
        }))
    );
}
#[test]
fn closed_chats_are_read_a_page_at_a_time() {
    let limits = fixtures::limits();
    let mut chats = Chats::new(FIRST);
    let mut out = Queue::with_capacity(OUTPUTS);
    chats.configure(Some(&fixtures::chats::me()), Some(fixtures::PROJECT), &limits, &mut out);
    out.pop();
    let Some(Request::Client(Event::Read { handle, read: Readable::Jig(Read::Tasks { query, live, ended }) })) =
        out.pop()
    else {
        panic!("closed read")
    };
    assert_eq!(
        (handle, query.phase, query.order, live, ended),
        (chats.closed, Some(PhaseClass::Ended), QueryOrder::Ended, None, None)
    );
    let cursor = Cursor::new(Box::from(*b"next"), CURSOR_BYTES).expect("cursor");
    chats.told(
        Told::Page {
            handle,
            page: Readout::Jig(ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                live: List::with_capacity(0),
                live_after: None,
                ended: EndedGroup::Rows { rows: List::with_capacity(0), after: Some(cursor.clone()), cut: false },
            })))),
        },
        &mut out,
    );
    chats.older(&mut out);
    assert_eq!(
        out.pop(),
        Some(Request::Client(Event::Read {
            handle,
            read: Readable::Jig(Read::Tasks { query, live: None, ended: Some(cursor) })
        }))
    );
}
#[test]
fn starting_a_chat_waits_for_the_first_words() {
    let limits = fixtures::limits();
    let me = fixtures::chats::me();
    let mut chats = Chats::new(FIRST);
    let mut out = Queue::with_capacity(OUTPUTS);
    let mut serial = SERIAL;
    chats.start(
        &fixtures::env(&limits),
        Some(&me),
        Some(fixtures::PROJECT),
        &crate::pages::choice::Draft::default(),
        &mut serial,
        &mut out,
    );
    assert_eq!(fixtures::acts::sent(&mut out).len(), 0);
}
#[test]
fn a_started_chat_opens_its_conversation_activity() {}

#[test]
fn new_chats_and_cues_keep_the_send_bound_when_receiving_grows() {
    let mut limits = fixtures::limits();
    limits.client.domain.text = 2048;
    limits.client.domain.turn_text = 2048;
    let bound = usize::try_from(temper_client_protocol_wire::CUE_BYTES).unwrap();
    let me = fixtures::chats::me();
    for suggested in [false, true] {
        for length in [bound, bound + 1] {
            let mut words = skein_lib::bytes::zeroed(length);
            words.fill(b'a');
            let mut chats = Chats::new(FIRST);
            let mut out = Queue::with_capacity(OUTPUTS);
            let mut serial = SERIAL;
            if suggested {
                chats.suggest(
                    &fixtures::env(&limits),
                    Some(&me),
                    Some(fixtures::PROJECT),
                    &words,
                    &crate::pages::choice::Draft::default(),
                    &mut serial,
                    &mut out,
                );
            } else {
                chats.write(words.clone(), &limits);
                chats.start(
                    &fixtures::env(&limits),
                    Some(&me),
                    Some(fixtures::PROJECT),
                    &crate::pages::choice::Draft::default(),
                    &mut serial,
                    &mut out,
                );
            }
            let sent = fixtures::acts::sent(&mut out);
            assert_eq!(sent.len(), usize::from(length == bound), "suggested={suggested}, length={length}");
            if length == bound {
                let words = Text::new(words, temper_client_protocol_wire::CUE_BYTES).unwrap();
                assert_eq!(sent.as_ref(), [jig_client::shapes::chat(&me, fixtures::PROJECT, words).unwrap()]);
            }
        }
    }
}

#[test]
fn chats_open_with_all_selected() {
    assert_eq!(Chats::new(FIRST).filter, ChatsFilter::All);
}

#[test]
fn chat_filter_moves_keep_the_draft_and_the_closed_page() {
    use crate::{
        ChatsFilter as Filter,
        pages::{self, Act, Event, Pages},
    };
    let limits = fixtures::limits();
    let root = crate::unlinked(&limits, fixtures::SEED);
    let mut pages = Pages::new();
    let mut out = Queue::with_capacity(OUTPUTS);
    pages::step(
        &mut pages,
        &fixtures::env(&limits),
        root.client(),
        Event::Went { address: crate::Address::Chats { filter: ChatsFilter::All, start: false } },
        &mut out,
    );
    while out.pop().is_some() {}
    pages::step(
        &mut pages,
        &fixtures::env(&limits),
        root.client(),
        Event::Act { act: Act::ChatWords { words: Box::from(*b"draft") } },
        &mut out,
    );
    assert!(out.is_empty());
    let handles = {
        let chats = pages.chats().expect("chats");
        (chats.live, chats.closed)
    };
    let closed = TaskPage {
        live: List::with_capacity(0),
        live_after: None,
        ended: EndedGroup::Rows { rows: List::with_capacity(0), after: None, cut: false },
    };
    pages::step(
        &mut pages,
        &fixtures::env(&limits),
        root.client(),
        Event::Told {
            told: Told::Page {
                handle: handles.1,
                page: Readout::Jig(ReadResult::Value(ReadValue::Tasks(Box::new(closed.clone())))),
            },
        },
        &mut out,
    );
    assert!(out.is_empty());
    for filter in [Filter::Live, Filter::Closed, Filter::All] {
        let address = crate::Address::Chats { filter, start: false };
        pages::step(
            &mut pages,
            &fixtures::env(&limits),
            root.client(),
            Event::Act { act: Act::ChatFilter { filter } },
            &mut out,
        );
        assert_eq!(out.pop(), Some(Request::Show { address: address.clone() }));
        assert!(out.is_empty());
        pages::step(&mut pages, &fixtures::env(&limits), root.client(), Event::Went { address }, &mut out);
        let chats = pages.chats().expect("chats");
        assert_eq!(chats.filter, filter);
        assert_eq!(&*chats.words, b"draft");
        assert_eq!((chats.live, chats.closed), handles);
        assert_eq!(chats.page.as_ref(), Some(&closed));
        assert!(out.is_empty(), "a filter move reads nothing again");
    }
}

#[test]
fn chat_filters_outside_chats_are_ignored_and_reopening_resets_them() {
    use crate::{
        ChatsFilter as Filter,
        pages::{self, Act, Event, Pages},
    };
    let limits = fixtures::limits();
    let root = crate::unlinked(&limits, fixtures::SEED);
    let mut pages = Pages::new();
    let mut out = Queue::with_capacity(OUTPUTS);
    pages::step(
        &mut pages,
        &fixtures::env(&limits),
        root.client(),
        Event::Act { act: Act::ChatFilter { filter: Filter::Closed } },
        &mut out,
    );
    assert!(pages.chats().is_none());
    assert!(out.is_empty());
    for address in [
        crate::Address::Chats { filter: ChatsFilter::All, start: false },
        crate::Address::Accounts,
        crate::Address::Chats { filter: ChatsFilter::All, start: false },
    ] {
        pages::step(&mut pages, &fixtures::env(&limits), root.client(), Event::Went { address }, &mut out);
        while out.pop().is_some() {}
        if let Some(chats) = pages.chats() {
            assert_eq!(chats.filter, Filter::All);
        }
        pages::step(
            &mut pages,
            &fixtures::env(&limits),
            root.client(),
            Event::Act { act: Act::ChatFilter { filter: Filter::Live } },
            &mut out,
        );
        if pages.chats().is_some() {
            assert_eq!(
                out.pop(),
                Some(Request::Show { address: crate::Address::Chats { filter: Filter::Live, start: false } })
            );
        }
        assert!(out.is_empty());
    }
}

#[test]
fn live_updates_do_not_replace_an_older_closed_page() {
    let limits = fixtures::limits();
    let mut chats = Chats::new(FIRST);
    let mut out = Queue::with_capacity(OUTPUTS);
    chats.configure(Some(&fixtures::chats::me()), Some(fixtures::PROJECT), &limits, &mut out);
    while out.pop().is_some() {}
    chats.page = Some(TaskPage {
        live: List::with_capacity(0),
        live_after: None,
        ended: EndedGroup::Rows {
            rows: List::with_capacity(0),
            after: Some(Cursor::new(Box::from(*b"next"), CURSOR_BYTES).expect("cursor")),
            cut: false,
        },
    });
    chats.older(&mut out);
    out.pop();
    chats.told(Told::Changed { handle: chats.live }, &mut out);
    assert!(out.is_empty(), "an older page stays put");
}

pub(super) fn chat_line() -> TaskLine {
    let task = fixtures::chat::task();
    let ChipPhase::Owner { details, .. } = task.chip.details.as_ref() else { panic!("owner") };
    let budget = details.budget.clone();
    TaskLine {
        chip: task.chip,
        project: fixtures::PROJECT,
        requester: OwnerRequester::Person(fixtures::PARTY),
        tracked: None,
        label: fixtures::chat::LABEL,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: true, listed: false, retention: Retention::Kept },
        changed: skein_lib::Wall::EPOCH,
        last_activity: skein_lib::Wall::EPOCH,
        ended_at: None,
        budget,
        waits_on: WaitsOn::Nothing,
        phases: OwnerPhaseCounts::ZERO,
        held_below: 0,
        conversation: None,
    }
}

fn opened_chats() -> (crate::Domain, skein_lib::Token, skein_lib::Token) {
    use crate::{Address, Place};
    let limits = fixtures::limits();
    let env = fixtures::env(&limits);
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let me = fixtures::start(&mut root, List::with_capacity(0));
    let mut out = Queue::uncapped();
    crate::step(
        &mut root,
        &env,
        crate::Event::Client {
            event: Event::Up {
                up: jig_client::Up::Watched {
                    watch: me,
                    event: WatchEvent::Delivered(Delivery::Me(Box::new(fixtures::chats::me()))),
                },
            },
        },
        &mut out,
    );
    crate::step(
        &mut root,
        &env,
        crate::Event::Went { place: Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) },
        &mut out,
    );
    let mut watch = None;
    let mut read = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Open { watch: token, subject: Watchable::Jig(Subject::Tasks { .. }) },
                },
        } = request
        {
            watch = Some(token);
        } else if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read: token, what: Readable::Jig(Read::Tasks { .. }) },
                },
        } = request
        {
            read = Some(token);
        }
    }
    (root, watch.expect("live watch"), read.expect("first closed page"))
}

#[test]
fn an_ending_chat_refreshes_the_first_closed_page_and_remains_listed() {
    use crate::{Action, pages::Act};
    let limits = fixtures::limits();
    let env = fixtures::env(&limits);
    let (mut root, watch, first_read) = opened_chats();
    let mut out = Queue::uncapped();
    crate::step(
        &mut root,
        &env,
        crate::Event::Client {
            event: Event::Up {
                up: jig_client::Up::ReadDone {
                    read: first_read,
                    result: Readout::Jig(ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                        live: List::with_capacity(0),
                        live_after: None,
                        ended: EndedGroup::Rows { rows: List::with_capacity(0), after: None, cut: false },
                    })))),
                },
            },
        },
        &mut out,
    );
    let mut rows = List::with_capacity(1);
    rows.push(chat_line()).expect("one live chat");
    for rows in [rows, List::with_capacity(0)] {
        crate::step(
            &mut root,
            &env,
            crate::Event::Client {
                event: Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: false }))),
                    },
                },
            },
            &mut out,
        );
        if !root.pages().chats().expect("chats").live(root.client()).expect("live group").rows.is_empty() {
            while out.pop().is_some() {}
        }
    }
    assert!(root.pages().chats().expect("chats").live(root.client()).expect("live group").rows.is_empty());
    let mut refreshed = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read, what: Readable::Jig(Read::Tasks { query, live, ended }) },
                },
        } = request
        {
            assert_eq!(
                (query.phase, query.order, live, ended),
                (Some(PhaseClass::Ended), QueryOrder::Ended, None, None)
            );
            refreshed = Some(read);
        }
    }
    let mut ended = chat_line();
    ended.ended_at = Some(skein_lib::Wall::EPOCH);
    let mut rows = List::with_capacity(1);
    rows.push(ended).expect("ended chat");
    crate::step(
        &mut root,
        &env,
        crate::Event::Client {
            event: Event::Up {
                up: jig_client::Up::ReadDone {
                    read: refreshed.expect("ending re-reads first page"),
                    result: Readout::Jig(ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                        live: List::with_capacity(0),
                        live_after: None,
                        ended: EndedGroup::Rows { rows, after: None, cut: false },
                    })))),
                },
            },
        },
        &mut out,
    );
    let EndedGroup::Rows { rows, .. } = &root.pages().chats().expect("chats").page.as_ref().expect("page").ended else {
        panic!("ended rows")
    };
    assert_eq!(rows.get(0).expect("ended chat").chip.task, fixtures::chat::TASK);
    // Selection is independent of the retained page and its cursors.
    crate::step(
        &mut root,
        &env,
        crate::Event::Act { action: Action::Pages(Act::ChatFilter { filter: ChatsFilter::Closed }) },
        &mut out,
    );
    assert_eq!(
        out.pop(),
        Some(crate::Request::Show {
            address: crate::Address::Chats { filter: ChatsFilter::Closed, start: false },
            replace: false,
        })
    );
    assert!(out.is_empty());
}
