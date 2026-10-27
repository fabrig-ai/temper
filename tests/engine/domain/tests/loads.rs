use core::mem::size_of;
use root::loads::{self, Failure, Limits, Loads, Request};
use skein_lib::{Queue, Token, Wall};
use temper_engine_domain::{self as root, Key, Range, Record, TurnRecord};
use temper_engine_domain_world::store::Store;

const LIMITS: Limits =
    Limits { read_bytes: 65_536, loads: 2, rows: 4, bytes: 1024, reply_bytes: 2048, transcript_bytes: 128 };

fn row(turn: u32, bytes: usize) -> Record {
    Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(TurnRecord {
        task: 1,
        attempt: 1,
        turn,
        spent: u64::from(turn),
        read: None,
        at: Wall::EPOCH,
        opens_window: false,
        transcript: vec![b'x'; bytes].into_boxed_slice(),
    })))
}

fn key(turn: u32) -> Key {
    Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Turn { task: 1, attempt: 1, turn }))
}

fn begin(loads: &mut Loads, after: Option<Key>, most: u32, out: &mut Queue<Request>) -> Token {
    let owner = loads::begin(
        loads,
        Token::new(51),
        Range::Core(temper_engine_domain::CoreRange::Turns { task: 1, attempt: 1 }),
        after.clone(),
        most,
        out,
    )
    .expect("load admitted");
    assert_eq!(
        out.pop(),
        Some(Request::Load {
            owner,
            range: Range::Core(temper_engine_domain::CoreRange::Turns { task: 1, attempt: 1 }),
            after,
            most,
            bytes: LIMITS.reply_bytes
        })
    );
    owner
}

#[test]
fn each_page_has_one_terminal_and_reads_the_answered_commit() {
    let mut store = Store::new();
    for turn in 1..=4 {
        let row = row(turn, 1);
        store.seed(Box::new([row]), Box::new([]));
    }
    let mut loads = Loads::new(&LIMITS);
    let mut out = Queue::with_capacity(1);
    let mut after = None;
    let mut observed = Vec::new();
    for expected_turns in [vec![1, 2], vec![3, 4]] {
        let owner = begin(&mut loads, after.clone(), 2, &mut out);
        let (rows, next) =
            store.page(&Range::Core(temper_engine_domain::CoreRange::Turns { task: 1, attempt: 1 }), after.as_ref(), 2);
        let expected_rows = rows.clone();
        loads::loaded(&mut loads, owner, rows, next.clone(), &mut out);
        let Request::Loaded { waiter, rows, next: found, cut } = out.pop().expect("page terminal") else {
            panic!("loaded");
        };
        assert_eq!(waiter, Token::new(51));
        assert_eq!(rows, expected_rows, "durable store content");
        assert_eq!(cut, None);
        assert_eq!(found, next);
        assert_eq!(
            rows.iter()
                .map(|r| {
                    let Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(r))) = r else {
                        panic!("turn");
                    };
                    r.turn
                })
                .collect::<Vec<_>>(),
            expected_turns
        );
        observed.extend(rows);
        loads::loaded(&mut loads, owner, Box::new([]), None, &mut out);
        loads::unloaded(&mut loads, owner, &mut out);
        assert!(out.is_empty(), "one terminal, including before reclaim");
        loads::reclaim(&mut loads);
        after = next;
    }
    assert_eq!(observed.len(), 4);
    assert_eq!(after, None);
}

#[test]
fn abandoned_io_keeps_capacity_until_its_real_terminal_and_old_tokens_are_fenced() {
    let limits = Limits { loads: 1, ..LIMITS };
    let mut loads = Loads::new(&limits);
    let mut out = Queue::with_capacity(1);
    let old = begin(&mut loads, None, 1, &mut out);
    loads::abandon(&mut loads, old);
    loads::reclaim(&mut loads);
    assert!(
        loads::begin(
            &mut loads,
            Token::new(8),
            Range::Core(temper_engine_domain::CoreRange::Deployment),
            None,
            1,
            &mut out
        )
        .is_none()
    );
    loads::loaded(&mut loads, old, vec![row(1, 4)].into_boxed_slice(), None, &mut out);
    assert!(out.is_empty());
    assert!(
        loads::begin(
            &mut loads,
            Token::new(8),
            Range::Core(temper_engine_domain::CoreRange::Deployment),
            None,
            1,
            &mut out
        )
        .is_none(),
        "retired until reclaim"
    );
    loads::reclaim(&mut loads);
    let new = begin(&mut loads, None, 1, &mut out);
    assert_ne!(old, new);
    loads::unloaded(&mut loads, old, &mut out);
    assert!(out.is_empty());
    loads::unloaded(&mut loads, new, &mut out);
    assert_eq!(out.pop(), Some(Request::Unloaded { waiter: Token::new(51), failure: Failure::Store }));
    loads::abandon(&mut loads, new);
    loads::unloaded(&mut loads, new, &mut out);
    assert!(out.is_empty());
}

#[test]
fn malformed_pages_are_refused_whole_before_any_row_is_delivered() {
    let cases = [
        (vec![row(1, 0), row(1, 0)], None, Failure::Order),
        (vec![row(2, 0), row(1, 0)], None, Failure::Order),
        (
            vec![Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
                temper_engine_domain_world::store::HEADER,
            )))],
            None,
            Failure::Range,
        ),
        (vec![row(1, 0)], Some(key(2)), Failure::Cursor),
        (vec![], Some(key(1)), Failure::Cursor),
        ((1..=5).map(|turn| row(turn, 0)).collect(), None, Failure::Rows),
        (vec![row(1, 2048)], None, Failure::Bytes),
    ];
    for (rows, next, expected) in cases {
        let mut loads = Loads::new(&LIMITS);
        let mut out = Queue::with_capacity(1);
        let owner = begin(&mut loads, None, 4, &mut out);
        loads::loaded(&mut loads, owner, rows.into_boxed_slice(), next, &mut out);
        assert_eq!(out.pop(), Some(Request::Unloaded { waiter: Token::new(51), failure: expected }));
        loads::loaded(&mut loads, owner, vec![row(1, 0)].into_boxed_slice(), None, &mut out);
        assert!(out.is_empty());
    }
}

#[test]
fn byte_cut_keeps_the_whole_prefix_and_reports_exact_omissions_and_resume_cursor() {
    let slot = u32::try_from(size_of::<Record>()).expect("small fixed record");
    let limits = Limits { bytes: 2 * slot + 8, ..LIMITS };
    let mut loads = Loads::new(&limits);
    let mut out = Queue::with_capacity(1);
    let owner = begin(&mut loads, None, 4, &mut out);
    loads::loaded(
        &mut loads,
        owner,
        vec![row(1, 4), row(2, 4), row(3, 4), row(4, 4)].into_boxed_slice(),
        None,
        &mut out,
    );
    assert_eq!(
        out.pop(),
        Some(Request::Loaded {
            waiter: Token::new(51),
            rows: vec![row(1, 4), row(2, 4)].into_boxed_slice(),
            next: Some(key(2)),
            cut: Some(loads::Cut { rows: 2, bytes: 2 * (u64::from(slot) + 4) })
        })
    );
    loads::reclaim(&mut loads);
    let owner = begin(&mut loads, Some(key(2)), 2, &mut out);
    loads::loaded(&mut loads, owner, vec![row(3, 129), row(4, 0)].into_boxed_slice(), None, &mut out);
    assert_eq!(
        out.pop(),
        Some(Request::Loaded {
            waiter: Token::new(51),
            rows: Box::new([]),
            next: Some(key(2)),
            cut: Some(loads::Cut { rows: 2, bytes: 2 * u64::from(slot) + 129 })
        }),
        "an oversized first row is explicit nonprogress, never a false end of range"
    );
}

#[test]
fn store_terminals_are_taken_while_the_commit_barrier_has_no_admission_room() {
    let limits = skein_lib::JournalLimits { commits: 3, writes: 1, held: 3, now: 1, release: 1 };
    let room = skein_lib::JournalRoom { writes: 1, held: 1 };
    let mut journal = skein_lib::Journal::<u32, u32>::new(&limits);
    for turn in 1..=3 {
        let mut decision = journal.decision(&room).expect("one admitted decision");
        decision.write(turn).expect("one bounded write");
        decision.hold(turn).expect("one bounded output");
        journal.accept(decision);
    }
    assert!(!journal.takes(&room));
    let mut loads = Loads::new(&LIMITS);
    let mut out = Queue::with_capacity(1);
    let owner = begin(&mut loads, None, 1, &mut out);
    loads::unloaded(&mut loads, owner, &mut out);
    assert_eq!(out.pop(), Some(Request::Unloaded { waiter: Token::new(51), failure: Failure::Store }));
}

fn ended_record(number: u64) -> Record {
    use temper_engine_domain_world::walking::{Settings, World};
    let mut world = World::new(Settings::calm(65));
    world.run();
    let mut record = world
        .store
        .rows
        .values()
        .find_map(|row| {
            if let Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Ended(record))) = row {
                Some(record.clone())
            } else {
                None
            }
        })
        .expect("story ended its task");
    record.number = number;
    Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Ended(record)))
}
fn census_range() -> Range {
    Range::Census(jig_domain_core::Census {
        results: Box::new([
            jig_domain_core::ReadResults { party: 8, after: 0 },
            jig_domain_core::ReadResults { party: 4, after: 0 },
        ]),
        questions: Box::new([]),
    })
}
fn result_key(party: u64, position: u64) -> Key {
    Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Result { party, position }))
}
#[test]
fn a_census_page_takes_repeated_owners_and_an_index_cursor() {
    let mut store = Store::new();
    for number in [5, 3] {
        let record = ended_record(number);
        store.seed(Box::new([record]), Box::new([]));
    }
    for (party, position, task) in [(8, 1, 5), (8, 2, 5), (8, 3, 3), (4, 1, 3)] {
        let record = Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Result(
            jig_domain_tasks::ResultRow { party, position, task },
        )));
        store.seed(Box::new([record]), Box::new([]));
    }
    let limits = Limits { bytes: 16_384, reply_bytes: 16_384, transcript_bytes: 16_384, ..LIMITS };
    let mut loads = Loads::new(&limits);
    let mut out = Queue::with_capacity(1);
    let owner = loads::begin(&mut loads, Token::new(51), census_range(), None, 3, &mut out).expect("census admitted");
    out.pop();
    let (rows, next) = store.page(&census_range(), None, 3);
    assert_eq!(next, Some(result_key(8, 3)));
    loads::loaded(&mut loads, owner, rows, next, &mut out);
    let Some(Request::Loaded { rows, next, cut: None, .. }) = out.pop() else { panic!("census page") };
    assert_eq!(
        rows.iter().map(Record::key).collect::<Vec<_>>(),
        [5, 5, 3].map(|number| Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Ended(number))))
    );
    assert_eq!(next, Some(result_key(8, 3)));
    let (rows, next) = store.page(&census_range(), Some(&result_key(8, 3)), 3);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Ended(3))));
    assert_eq!(next, None);
}

#[test]
fn a_census_cursor_behind_its_start_is_out_of_order() {
    let mut loads = Loads::new(&LIMITS);
    let mut out = Queue::with_capacity(1);
    for (after, next, expected) in [
        (result_key(4, 2), result_key(8, 3), Some(Failure::Order)),
        (result_key(8, 1), ended_read_key(5, 8), Some(Failure::Order)),
        (result_key(8, 2), result_key(8, 2), Some(Failure::Order)),
        (result_key(8, 2), result_key(8, 3), None),
        (result_key(8, 2), key(3), Some(Failure::Cursor)),
    ] {
        let owner = loads::begin(&mut loads, Token::new(51), census_range(), Some(after), 3, &mut out)
            .expect("census cursor admitted");
        out.pop();
        loads::loaded(&mut loads, owner, Box::new([]), Some(next.clone()), &mut out);
        match expected {
            Some(failure) => assert_eq!(out.pop(), Some(Request::Unloaded { waiter: Token::new(51), failure })),
            None => assert_eq!(
                out.pop(),
                Some(Request::Loaded { waiter: Token::new(51), rows: Box::new([]), next: Some(next), cut: None })
            ),
        }
        loads::reclaim(&mut loads);
    }
}

#[test]
fn a_forge_row_survives_the_real_store_byte_for_byte() {
    let (driver, _) = temper_engine_domain_world::direct::Driver::chat();
    let mut store = driver.store.fork();
    assert_eq!(store.page(&Range::Forge, None, 8).0.len(), 0, "a chat's store holds no forge row");
    let row = Record::Forge { row: Box::new(temper_engine_domain_forge::Stored::Landed { commit: [7; 32], task: 3 }) };
    store.seed(Box::new([row.clone()]), Box::new([]));
    let (rows, next) = store.page(&Range::Forge, None, 8);
    assert_eq!((rows.as_ref(), next), ([row].as_slice(), None));
}

fn ended_read_key(task: u64, party: u64) -> Key {
    Key::Core(jig_domain_core::Key::People(jig_domain_people::Key::EndedRead { task, party }))
}

#[test]
fn a_census_page_takes_an_ended_read_position_then_its_task() {
    let mut store = Store::new();
    let position = Record::Core(jig_domain_core::Record::People(jig_domain_people::Stored::EndedRead {
        task: 5,
        party: 8,
        through: 1,
    }));
    store.seed(
        Box::new([
            position.clone(),
            ended_record(5),
            Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Result(
                jig_domain_tasks::ResultRow { party: 8, position: 1, task: 5 },
            ))),
        ]),
        Box::new([]),
    );
    let limits = Limits { bytes: 16_384, reply_bytes: 16_384, transcript_bytes: 16_384, ..LIMITS };
    let mut loads = Loads::new(&limits);
    let mut out = Queue::with_capacity(1);
    let owner = loads::begin(&mut loads, Token::new(51), census_range(), None, 2, &mut out).expect("census admitted");
    out.pop();
    let (rows, next) = store.page(&census_range(), None, 2);
    assert_eq!(rows, Box::new([position, ended_record(5)]) as Box<[Record]>);
    assert_eq!(next, Some(ended_read_key(5, 8)));
    loads::loaded(&mut loads, owner, rows.clone(), next.clone(), &mut out);
    assert_eq!(out.pop(), Some(Request::Loaded { waiter: Token::new(51), rows, next: next.clone(), cut: None }));
    let (rows, next) = store.page(&census_range(), next.as_ref(), 2);
    assert_eq!(rows, Box::new([ended_record(5)]) as Box<[Record]>);
    assert_eq!(next, None);
}

#[test]
fn an_ended_conversation_query_joins_its_owner_and_read_position_after_restart() {
    use jig_domain_core::{self as core, reads};
    use jig_domain_tasks as tasks;
    const PARTY: u64 = 1;
    const TASK: u64 = 1;
    let mut store = Store::new();
    let owner = ended_record(TASK);
    let Record::Core(core::Record::Tasks(tasks::Stored::Ended(task))) = &owner else { panic!("ended fixture") };
    let index = core::Record::Tasks(tasks::Stored::EndedByParty(tasks::EndedByParty {
        party: PARTY,
        task: TASK,
        label: task.label,
        ended_at: task.ended_at.expect("ended time"),
    }));
    store.seed(Box::new([owner, Record::Core(index.clone())]), Box::new([]));
    store.crash(71);
    let loaded = store.read(Box::new(reads::Range {
        party: PARTY,
        scope: reads::Scope::Tasks {
            query: core::documents::TaskQuery {
                project: None,
                requester: Some(tasks::Party::Person(PARTY)),
                root: None,
                parent: None,
                label: None,
                phase: None,
                executor: None,
                tracked: None,
                order: core::documents::QueryOrder::Number,
            },
        },
        after: None,
        backwards: false,
        rows: 4,
        bytes: 65_536,
    }));
    let reads::Loaded::Page { rows, next } = loaded else { panic!("ended query page") };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value, index);
    assert_eq!(rows[0].owner.as_ref().map(|owner| owner.number), Some(TASK));
    assert_eq!(next, None);
}

// The diagnosis's unanswered question identity, already written in tasks v9.
const QUESTION_TASK: u64 = 6;
const QUESTION_LINE: u64 = 13;
const RESULT_PARTY: u64 = 1;
const CENSUS_WAITER: Token = Token::new(51); // The load fixture's waiter.
const QUESTION_PAGE: u32 = 1; // One exact question activity row.

fn question_census() -> Range {
    Range::Census(jig_domain_core::Census {
        questions: Box::new([(QUESTION_TASK, QUESTION_LINE)]),
        results: Box::new([jig_domain_core::ReadResults { party: RESULT_PARTY, after: 0 }]),
    })
}

fn question_activity(task: u64, position: u64) -> Record {
    use jig_domain_tasks as tasks;
    Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
        task,
        position,
        at: Wall::EPOCH,
        line: tasks::Line::Words {
            message: 1,
            by: tasks::Party::Task(7),
            kind: tasks::MessageKind::Question,
            words: b"ship?".as_slice().into(),
        },
    })))
}

#[test]
fn a_census_restores_a_preexisting_question_activity() {
    let record = question_activity(QUESTION_TASK, QUESTION_LINE);
    let mut store = Store::new();
    store.seed(Box::new([record.clone()]), Box::new([]));
    store.crash(71);
    let mut loads = Loads::new(&LIMITS);
    let mut out = Queue::with_capacity(1);
    let owner = loads::begin(&mut loads, CENSUS_WAITER, question_census(), None, QUESTION_PAGE, &mut out)
        .expect("question census admitted");
    out.pop();
    let (rows, next) = store.page(&question_census(), None, QUESTION_PAGE);
    loads::loaded(&mut loads, owner, rows, next, &mut out);
    assert_eq!(
        out.pop(),
        Some(Request::Loaded { waiter: CENSUS_WAITER, rows: Box::new([record]), next: None, cut: None })
    );
}

#[test]
fn a_census_refuses_activity_outside_its_named_questions() {
    for (task, position) in [(QUESTION_TASK, QUESTION_LINE - 1), (QUESTION_TASK + 1, QUESTION_LINE)] {
        let mut loads = Loads::new(&LIMITS);
        let mut out = Queue::with_capacity(1);
        let owner = loads::begin(&mut loads, CENSUS_WAITER, question_census(), None, QUESTION_PAGE, &mut out)
            .expect("question census admitted");
        out.pop();
        loads::loaded(&mut loads, owner, Box::new([question_activity(task, position)]), None, &mut out);
        assert_eq!(out.pop(), Some(Request::Unloaded { waiter: CENSUS_WAITER, failure: Failure::Range }));
    }
}

fn first_restore_load() -> (temper_engine_domain_world::direct::Driver, Token) {
    use temper_engine_domain_world::{direct::Driver, walking};
    let mut driver = Driver::cold(Store::new(), walking::config(91), &walking::limits());
    // The direct world's fixed immediate-work horizon.
    for _ in 0..200 {
        driver.advance(true);
        if let Some((owner, _)) = driver.loads.first() {
            let owner = *owner;
            driver.events.clear();
            return (driver, owner);
        }
    }
    panic!("the first restore asks for the deployment header");
}

#[test]
fn a_failed_restore_load_retains_its_kind_and_store_failure() {
    let (mut driver, owner) = first_restore_load();
    driver.send(root::Event::Store(root::StoreInput::Unloaded { owner }));
    driver.pump();
    assert!(driver.stopped);
    assert_eq!(
        driver.root.restore_load_failure(),
        Some(loads::RestoreFailure { range: loads::Kind::Deployment, failure: Failure::Store })
    );
}

#[test]
fn a_malformed_restore_page_retains_its_kind_and_range_failure() {
    let (mut driver, owner) = first_restore_load();
    driver.send(root::Event::Store(root::StoreInput::Loaded {
        owner,
        rows: Box::new([row(1, 0)]),
        next: None,
        texts: None,
    }));
    driver.pump();
    assert!(driver.stopped);
    assert_eq!(
        driver.root.restore_load_failure(),
        Some(loads::RestoreFailure { range: loads::Kind::Deployment, failure: Failure::Range })
    );
}
