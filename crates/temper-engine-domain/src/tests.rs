use crate::decision::{
    Counters, Decision, Delivery, Journal as SkeinJournal, JournalOutput as Output, Limits, accept as accept_decision,
    committed as mark_committed, fresh as fresh_number, journal_limits, resume as release_ready,
    takes as takes_decision,
};
use crate::{Deployment, Family, Key, Record, TurnRecord, Write};
use alloc::boxed::Box;
use skein_lib::{JournalRoom, List, Queue, Token, Wall};

#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

mod projection_recovery;
mod result;
mod resume;
pub(super) mod resume_fixture;
mod setup;

const LIMITS: Limits =
    Limits { commits: 2, held: 8, writes: 4, deliveries: 4, transcript_bytes: 64, result_bytes: 32, run_bytes: 1 };

const DEPLOYMENT: Deployment = Deployment {
    projects: 0,
    id: [19; 16],
    tasks: 0,
    people: 0,
    sign_ins: 0,
    messages: 0,
    runs: 0,
    calls: 0,
    connector_rows: 0,
    applications: 0,
    commits: 0,
};

struct Journal {
    inner: SkeinJournal,
    counters: Counters,
    durable: u64,
}

impl Journal {
    fn bootstrap(id: [u8; 16], limits: &Limits) -> Journal {
        Journal { inner: SkeinJournal::new(&journal_limits(limits)), counters: Counters::bootstrap(id), durable: 0 }
    }

    fn new(deployment: Deployment, limits: &Limits) -> Journal {
        Journal {
            inner: SkeinJournal::from_durable(&journal_limits(limits), deployment.commits),
            counters: Counters::new(deployment),
            durable: deployment.commits,
        }
    }

    fn deployment(&self) -> Deployment {
        self.counters.deployment()
    }
    fn durable(&self) -> u64 {
        self.durable
    }
    fn stopped(&self) -> bool {
        self.inner.stopped()
    }
}

fn takes(journal: &Journal, limits: &Limits) -> bool {
    takes_decision(&journal.inner, limits)
}
fn fresh(journal: &mut Journal, family: Family) -> Option<u64> {
    if journal.inner.stopped() { None } else { fresh_number(&mut journal.counters, family) }
}
#[expect(clippy::result_large_err, reason = "test wrapper preserves the decision's owned refusal")]
fn accept(journal: &mut Journal, limits: &Limits, decision: Decision, out: &mut Queue<Output>) -> Result<(), Decision> {
    accept_decision(&mut journal.inner, &mut journal.counters, limits, decision, out)
}
fn committed(journal: &mut Journal, number: u64) {
    mark_committed(&mut journal.inner, number);
    if !journal.inner.stopped() {
        journal.durable = number;
    }
}
fn resume(journal: &mut Journal, out: &mut Queue<Output>) {
    release_ready(&mut journal.inner, out);
}
fn bytes(len: u32) -> Box<[u8]> {
    let mut bytes = List::with_capacity(len);
    for _ in 0..len {
        bytes.push(b'x').unwrap();
    }
    bytes.into_boxed()
}

fn turn(number: u32, len: u32) -> Write {
    Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(TurnRecord {
        task: 1,
        attempt: 1,
        turn: number,
        spent: u64::from(number),
        read: Some(u64::from(number)),
        at: Wall::from_nanos(17),
        opens_window: number == 1,
        transcript: bytes(len),
    }))))
}

fn ack(number: u32) -> Delivery {
    let down = jig_domain_host::LinkDown::TurnKept { run: Token::new(1), attempt: Token::new(1), turn: number };
    Delivery::Link { channel: Token::new(7), down }
}

fn decision(number: u32) -> Decision {
    let mut d = Decision::new(&LIMITS);
    d.write(&LIMITS, turn(number, 2)).unwrap();
    d.deliver(&LIMITS, ack(number)).unwrap();
    d
}

fn commit(out: &mut Queue<Output>) -> (u64, Box<[Write]>) {
    let Output::Commit { number, writes } = out.pop().unwrap() else {
        panic!("commit");
    };
    (number, writes)
}

fn acknowledged(out: &mut Queue<Output>) -> u32 {
    let Output::Deliver(Delivery::Link { down: jig_domain_host::LinkDown::TurnKept { turn, .. }, .. }) =
        out.pop().unwrap()
    else {
        panic!("ack");
    };
    turn
}

#[test]
fn a_first_start_commits_its_deployment_before_releasing_a_delivery() {
    let mut j = Journal::bootstrap([37; 16], &LIMITS);
    let mut out = Queue::with_capacity(1);
    let mut d = Decision::new(&LIMITS);
    d.deliver(&LIMITS, ack(1)).unwrap();
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
    let (number, writes) = commit(&mut out);
    assert_eq!(number, 1);
    assert_eq!(
        writes.as_ref(),
        &[Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
            Deployment { id: [37; 16], commits: 1, ..DEPLOYMENT }
        ))))]
    );
    resume(&mut j, &mut out);
    assert!(out.is_empty());
    committed(&mut j, number);
    resume(&mut j, &mut out);
    assert_eq!(acknowledged(&mut out), 1);
}

#[test]
fn a_store_answer_makes_outputs_ready_without_draining_them() {
    let mut j = Journal::new(DEPLOYMENT, &LIMITS);
    let mut out = Queue::with_capacity(1);
    accept(&mut j, &LIMITS, decision(1), &mut out).unwrap();
    let (number, writes) = commit(&mut out);
    assert_eq!(number, 1);
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes[0],
        Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(Deployment {
            commits: 1,
            ..DEPLOYMENT
        }))))
    );
    resume(&mut j, &mut out);
    assert!(out.is_empty());
    committed(&mut j, number);
    assert!(out.is_empty());
    resume(&mut j, &mut out);
    assert_eq!(acknowledged(&mut out), 1);
    resume(&mut j, &mut out);
    assert!(out.is_empty());
}

#[test]
fn fresh_numbers_and_unused_gaps_are_durable_in_the_decisions_header() {
    let mut j = Journal::new(DEPLOYMENT, &LIMITS);
    let mut out = Queue::with_capacity(1);
    for family in [Family::Task, Family::Person, Family::SignIn, Family::Message, Family::Run, Family::Call] {
        assert_eq!(fresh(&mut j, family), Some(1));
    }
    assert_eq!(fresh(&mut j, Family::Task), Some(2));
    accept(&mut j, &LIMITS, Decision::new(&LIMITS), &mut out).unwrap();
    let (number, writes) = commit(&mut out);
    assert_eq!(number, 1);
    let Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(header)))) =
        writes[0]
    else {
        panic!("header");
    };
    assert_eq!(header.tasks, 2);
    let mut restarted = Journal::new(header, &LIMITS);
    assert_eq!(restarted.durable(), 1);
    assert_eq!(fresh(&mut restarted, Family::Task), Some(3));
}

#[test]
fn same_key_saves_and_erases_collapse_to_the_last_write_before_commit() {
    let mut j = Journal::new(DEPLOYMENT, &LIMITS);
    let mut out = Queue::with_capacity(1);
    let mut d = Decision::reserve_room(&mut j.inner, &LIMITS, JournalRoom { writes: 3, held: 0 }).unwrap();
    d.write(&LIMITS, turn(1, 2)).unwrap();
    d.write(&LIMITS, turn(2, 3)).unwrap();
    assert!(!d.room_for(1, 0), "replacement still fits when all child slots are full");
    d.write(
        &LIMITS,
        Write::Erase(Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Turn {
            task: 1,
            attempt: 1,
            turn: 1,
        }))),
    )
    .unwrap();
    d.write(&LIMITS, turn(1, 4)).unwrap();
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
    let (_, writes) = commit(&mut out);
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[1], turn(1, 4));
    assert_eq!(writes[2], turn(2, 3));
}

#[test]
fn rejected_payloads_are_returned_without_replacing_admitted_ownership() {
    let mut j = Journal::new(DEPLOYMENT, &LIMITS);
    let mut d = Decision::reserve_room(&mut j.inner, &LIMITS, JournalRoom { writes: 2, held: 1 }).unwrap();
    d.write(&LIMITS, turn(1, 3)).unwrap();
    let refused = d.write(&LIMITS, turn(1, 65)).unwrap_err();
    assert_eq!(refused, turn(1, 65));
    let refused = d.deliver(&LIMITS, Delivery::Result { person: 1, task: 1, words: bytes(33) }).unwrap_err();
    let Delivery::Result { words, .. } = refused else {
        panic!("result");
    };
    assert_eq!(words.len(), 33);
    assert!(
        d.write(&LIMITS, Write::Erase(Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment))))
            .is_err()
    );
    let mut out = Queue::with_capacity(1);
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
    let (_, writes) = commit(&mut out);
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[1], turn(1, 3));
    committed(&mut j, 1);
    resume(&mut j, &mut out);
    assert!(out.is_empty());
}

#[test]
fn a_decision_past_its_reserved_write_room_stops_the_root_journal() {
    for writes in 1..=3 {
        let mut journal = Journal::new(DEPLOYMENT, &LIMITS);
        let mut out = Queue::with_capacity(1);
        let mut decision = Decision::reserve_room(&mut journal.inner, &LIMITS, JournalRoom { writes, held: 1 })
            .expect("smaller route room admitted");
        for number in 1..writes {
            decision.write(&LIMITS, turn(number, 1)).expect("reserved write room");
        }
        decision.deliver(&LIMITS, ack(1)).unwrap();
        assert!(!decision.room_for(1, 0), "the deployment header owns the other slot");
        let refused = turn(writes, 1);
        let Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(row)))) = &refused
        else {
            panic!("turn");
        };
        let address = row.transcript.as_ptr();
        let refused = decision.carry_write(&LIMITS, refused).unwrap_err();
        let Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(row)))) = refused
        else {
            panic!("original refused turn");
        };
        assert_eq!(row.transcript.as_ptr(), address, "refusal moves the original payload back");
        assert_eq!(row.turn, writes);
        accept(&mut journal, &LIMITS, decision, &mut out).expect("overrun is accepted to stop");
        assert!(journal.stopped());
        resume(&mut journal, &mut out);
        assert!(out.is_empty(), "no partial commit or held delivery escapes an overrun");
        assert!(!takes(&journal, &LIMITS));
    }
}

#[test]
fn reserved_buffers_allocate_only_the_admitted_room_even_when_global_maxima_grow() {
    use skein_world::domain::heap::Meter;

    let write_slot = u64::try_from(size_of::<Write>()).unwrap();
    let delivery_slot = u64::try_from(size_of::<Delivery>()).unwrap();
    let output_slot = u64::try_from(size_of::<crate::Output>()).unwrap();
    let rooms =
        [JournalRoom { writes: 0, held: 0 }, JournalRoom { writes: 1, held: 0 }, JournalRoom { writes: 3, held: 2 }];
    let mut observed = [[(0, 0); 3]; 2];
    let mut expected = [(0, 0); 3];
    for (maximum, (writes, deliveries)) in [(64, 32), (4096, 2048)].into_iter().enumerate() {
        let limits = Limits { writes, deliveries, held: deliveries, ..LIMITS };
        for (at, reserved) in rooms.into_iter().enumerate() {
            // Journal storage is startup state. Measure the whole admitted
            // decision, including its two Skein batch containers.
            let mut journal = SkeinJournal::new(&journal_limits(&limits));
            let meter = Meter::new();
            meter.start();
            let decision = Decision::reserve_room(&mut journal, &limits, reserved).unwrap();
            let measured = meter.end();
            let batch = u64::from(reserved.writes) * write_slot + u64::from(reserved.held) * output_slot;
            let private =
                u64::from(reserved.writes.saturating_sub(1)) * write_slot + u64::from(reserved.held) * delivery_slot;
            expected[at] = (batch + private, batch + private);
            observed[maximum][at] = (measured.held(), measured.peak());
            drop(decision);
            assert_eq!(meter.held(), 0, "dropping the decision frees every metered slot");
        }
    }
    assert_eq!(observed[0], expected, "only reserved slots, with no transient maximum-sized buffers");
    assert_eq!(observed[1], expected, "larger global maxima cost the same for the same reservation");
    assert_eq!(observed[0], observed[1]);
}

#[test]
fn reserved_bounds_keep_the_header_and_release_exactly_the_admitted_deliveries() {
    for (reserved, persist_header) in [
        (JournalRoom { writes: 0, held: 0 }, false),
        (JournalRoom { writes: 0, held: 2 }, false),
        (JournalRoom { writes: 1, held: 0 }, true),
        (JournalRoom { writes: 1, held: 2 }, true),
        (JournalRoom { writes: 1, held: 2 }, false),
        (JournalRoom { writes: 3, held: 0 }, true),
        (JournalRoom { writes: 3, held: 2 }, true),
        (JournalRoom { writes: 3, held: 2 }, false),
    ] {
        let mut journal = Journal::bootstrap(DEPLOYMENT.id, &LIMITS);
        let mut decision = Decision::reserve_room(&mut journal.inner, &LIMITS, reserved).unwrap();
        for number in 1..reserved.writes {
            decision.carry_write(&LIMITS, turn(number, 1)).unwrap();
        }
        for number in 1..=reserved.held {
            decision.carry_delivery(&LIMITS, ack(number)).unwrap();
        }
        assert!(!decision.room_for(1, 0), "header owns the first write slot");
        assert!(!decision.room_for(0, 1), "held room is exactly full");
        let header = Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
            DEPLOYMENT,
        ))));
        assert_eq!(decision.write(&LIMITS, header.clone()), Err(header), "caller cannot own the header slot");
        crate::decision::accept_routed(&mut journal.inner, &mut journal.counters, &LIMITS, decision, persist_header)
            .unwrap();
        let mut out = Queue::with_capacity(1);
        if reserved.writes <= 1 && !persist_header {
            assert!(crate::decision::commit(&mut journal.inner, &LIMITS).is_none());
            assert!(journal.counters.dirty(), "read-only store terminals never persist the dirty header");
        } else {
            out.push(crate::decision::commit(&mut journal.inner, &LIMITS).unwrap());
            let (number, writes) = commit(&mut out);
            assert_eq!((number, writes.len()), (1, usize::try_from(reserved.writes).unwrap()));
            assert_eq!(
                writes[0],
                Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
                    Deployment { commits: 1, ..DEPLOYMENT }
                ))))
            );
            for number in 1..reserved.writes {
                assert_eq!(writes[usize::try_from(number).unwrap()], turn(number, 1));
            }
            resume(&mut journal, &mut out);
            assert!(out.is_empty(), "held effects wait for the header's commit");
            committed(&mut journal, number);
        }
        for number in 1..=reserved.held {
            resume(&mut journal, &mut out);
            assert_eq!(acknowledged(&mut out), number, "each admitted delivery releases once, in order");
        }
        resume(&mut journal, &mut out);
        assert!(out.is_empty());
        assert!(!journal.stopped());
    }
}

#[test]
fn a_delivery_past_its_reserved_room_returns_ownership_and_stops_without_partial_outputs() {
    for reserved in [
        JournalRoom { writes: 0, held: 0 },
        JournalRoom { writes: 1, held: 0 },
        JournalRoom { writes: 1, held: 1 },
        JournalRoom { writes: 3, held: 2 },
    ] {
        let mut journal = Journal::new(DEPLOYMENT, &LIMITS);
        let mut decision = Decision::reserve_room(&mut journal.inner, &LIMITS, reserved).unwrap();
        for number in 1..reserved.writes {
            decision.write(&LIMITS, turn(number, 1)).unwrap();
        }
        for number in 1..=reserved.held {
            decision.deliver(&LIMITS, ack(number)).unwrap();
        }
        let words = bytes(3);
        let address = words.as_ptr();
        let refused = decision.carry_delivery(&LIMITS, Delivery::Result { person: 7, task: 9, words }).unwrap_err();
        let Delivery::Result { person, task, words } = refused else { panic!("original refused result") };
        assert_eq!((person, task, words.as_ptr(), words.len()), (7, 9, address, 3));
        let mut out = Queue::with_capacity(1);
        accept(&mut journal, &LIMITS, decision, &mut out).unwrap();
        assert!(journal.stopped());
        resume(&mut journal, &mut out);
        assert!(out.is_empty(), "no admitted prefix escapes an overrun");
        assert_eq!(journal.deployment().commits, 0);
    }
}

#[test]
fn full_size_decisions_keep_all_ownership_when_a_full_journal_refuses_them() {
    let mut journal = Journal::new(DEPLOYMENT, &LIMITS);
    for reserved in
        [JournalRoom { writes: LIMITS.writes + 1, held: 0 }, JournalRoom { writes: 1, held: LIMITS.deliveries + 1 }]
    {
        assert!(Decision::reserve_room(&mut journal.inner, &LIMITS, reserved).is_none());
        assert!(takes(&journal, &LIMITS), "an oversized request reserves nothing");
    }
    let mut out = Queue::with_capacity(1);
    for number in 1..=LIMITS.commits {
        let mut decision = Decision::reserve(&mut journal.inner, &LIMITS).unwrap();
        decision.write(&LIMITS, turn(number, 1)).unwrap();
        accept(&mut journal, &LIMITS, decision, &mut out).unwrap();
        assert_eq!(commit(&mut out).0, u64::from(number));
    }
    assert!(!takes(&journal, &LIMITS));
    assert!(Decision::reserve(&mut journal.inner, &LIMITS).is_none());
    let mut decision = Decision::new(&LIMITS);
    for number in 1..LIMITS.writes {
        decision.write(&LIMITS, turn(number, 1)).unwrap();
    }
    for number in 1..=LIMITS.deliveries {
        decision.deliver(&LIMITS, ack(number)).unwrap();
    }
    assert!(!decision.room_for(1, 0));
    assert!(!decision.room_for(0, 1));
    let decision = accept(&mut journal, &LIMITS, decision, &mut out).unwrap_err();
    assert!(out.is_empty());
    committed(&mut journal, u64::from(LIMITS.commits));
    accept(&mut journal, &LIMITS, decision, &mut out).unwrap();
    let (number, writes) = commit(&mut out);
    assert_eq!((number, writes.len()), (3, usize::try_from(LIMITS.writes).unwrap()));
    for number in 1..LIMITS.writes {
        assert_eq!(writes[usize::try_from(number).unwrap()], turn(number, 1));
    }
    resume(&mut journal, &mut out);
    assert!(out.is_empty(), "refused and retried effects still wait for their own commit");
    committed(&mut journal, number);
    for number in 1..=LIMITS.deliveries {
        resume(&mut journal, &mut out);
        assert_eq!(acknowledged(&mut out), number);
    }
    resume(&mut journal, &mut out);
    assert!(out.is_empty());
}

#[test]
fn counter_and_commit_overflow_cannot_reuse_names() {
    let mut j = Journal::new(Deployment { tasks: u64::MAX, commits: u64::MAX, ..DEPLOYMENT }, &LIMITS);
    assert_eq!(fresh(&mut j, Family::Task), None);
    assert!(!takes(&j, &LIMITS));
    assert_eq!(j.deployment().tasks, u64::MAX);
}

#[test]
fn deep_child_rows_and_arbitrary_internal_payloads_are_refused_before_retention() {
    use jig_domain_fleet as fleet;
    use jig_domain_people as people;
    let limits = crate::JournalLimits {
        commits: 1,
        held: 2,
        writes: 2,
        deliveries: 2,
        transcript_bytes: 4,
        result_bytes: 4,
        run_bytes: 1,
    };
    let mut decision = Decision::new(&limits);
    let row = Record::Core(jig_domain_core::Record::People(people::Stored::Person {
        number: 1,
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 1_u64.to_be_bytes().into() },
            login: b"abc".as_slice().into(),
            name: b"de".as_slice().into(),
        },
    }));
    assert_eq!(crate::record_bytes(&row), Some(13));
    assert!(decision.write(&limits, Write::Save(row)).is_err());
    let roster_answer = Record::Core(jig_domain_core::Record::People(people::Stored::Answer {
        models: Box::new([]),
        key: people::RequestKey { person: 1, key: [1; 16] },
        ask: Box::new(people::Ask::SetRoles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
        }),
        outcome: people::Outcome::RolesSet { project: 1 },
        at: Wall::EPOCH,
    }));
    assert_eq!(
        crate::record_bytes(&roster_answer),
        Some(u64::try_from(size_of::<people::Holding>()).expect("holding slot"))
    );
    assert!(
        decision.write(&limits, Write::Save(roster_answer)).is_err(),
        "saved keyed roster has separately owned bounded bytes"
    );
    let callback = Delivery::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(1),
        hello: fleet::Hello {
            stop_bound: skein_lib::Duration::from_secs(0),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
            bounds: fleet::Bounds {
                program_endpoints: Box::new([]),
                models: Box::new([]),
                turns: 1,
                spend: 1,
                time: skein_lib::Duration::from_secs(1),
                waiting: skein_lib::Duration::from_secs(1),
                checks: skein_lib::Duration::from_secs(1),
                message_bytes: 1,
                inbox: 1,
                unacknowledged: 1,
            },
        },
    });
    assert!(
        decision.deliver(&limits, callback).is_err(),
        "arbitrary fleet events cannot bypass bounds through the journal"
    );
}

#[test]
fn held_assignment_checks_owned_bytes_and_section_backing_before_acceptance() {
    let limits = crate::JournalLimits {
        commits: 1,
        held: 1,
        writes: 1,
        deliveries: 1,
        transcript_bytes: 4,
        result_bytes: 4,
        run_bytes: 512,
    };
    let mut decision = Decision::new(&limits);
    let mut assignment = held_assignment();
    assert!(
        decision
            .deliver(&limits, Delivery::Assigned { channel: Token::new(1), assignment: assignment.clone() })
            .is_err()
    );
    assignment.sections = Box::new([]);
    let bytes = crate::limits::run_charter_bytes(&assignment.run).expect("fixture charter bytes");
    let exact = crate::JournalLimits { run_bytes: u32::try_from(bytes).expect("tiny charter"), ..limits };
    let mut decision = Decision::new(&exact);
    decision
        .deliver(&exact, Delivery::Assigned { channel: Token::new(1), assignment: assignment.clone() })
        .expect("the exact run byte envelope accepts its binding arrays");
    assignment.run.sources = Box::new([Some(1), Some(2)]);
    assignment.run.accounts = Box::new([
        jig_domain_authority::AccountBinding { provider: 1, account: 1, source: 1 },
        jig_domain_authority::AccountBinding { provider: 2, account: 2, source: 2 },
    ]);
    let mut decision = Decision::new(&exact);
    assert!(decision.deliver(&exact, Delivery::Assigned { channel: Token::new(1), assignment }).is_err());
}

fn held_assignment() -> crate::Assignment {
    use jig_domain_accounts as accounts;
    use jig_domain_brief as brief;
    crate::Assignment {
        task: 1,
        attempt: 1,
        conversation: 1,
        live: jig_domain_host::LiveAsk::NONE,
        charter: 1,
        run: Box::new(crate::RunCharter {
            ended_by: jig_domain_tasks::EndedBy::Result,
            sources: Box::new([Some(1)]),
            accounts: Box::new([jig_domain_authority::AccountBinding { provider: 1, account: 1, source: 1 }]),
            placement: jig_domain_fleet::PlacementConstraint {
                channel: Token::new(1),
                kind: jig_domain_fleet::HostKind::Worker,
            },
            policy: crate::RunPolicy {
                instructions: Box::new([]),
                waiting: skein_lib::Duration::from_secs(1),
                resume: true,
                turns: 2,
                time: skein_lib::Duration::from_secs(2),
                reserve_turns: 1,
                reserve_time: skein_lib::Duration::from_secs(1),
                model: crate::Model {
                    program: None,
                    dialect: 1,
                    endpoint: 1,
                    name: b"m".as_slice().into(),
                    window: 1,
                    output: 1,
                    input_price: 1,
                    cached_price: 1,
                    cache_write_price: None,
                    output_price: 1,
                    price_unit: 1,
                    efforts: Box::new([]),
                    effort: None,
                },
                alternatives: Box::new([]),
                inspect: false,
                modify: false,
                shell: false,
                agents: false,
                call_timeout: skein_lib::Duration::from_secs(1),
            },
            contract: jig_domain_tasks::Contract::Report { words: 1 },
            authority: jig_domain_tasks::Authority {
                tools: jig_domain_tasks::Tools(0),
                models: jig_domain_tasks::Models::NONE,
                grants: Box::new([]),
                delegation: jig_domain_tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
                budget: jig_domain_tasks::Budget { spend: 1, deadline: None },
                notes: jig_domain_tasks::Scopes(0),
                note_resources: Box::new([]),
            },
            budget: 1,
        }),
        sections: Box::new([crate::BriefSection {
            kind: crate::BriefKind::Core(brief::Core::Task),
            body: crate::BriefBody::Text(b"12345".as_slice().into()),
        }]),
        messages: Box::new([]),
        saved: Box::new([]),
        workspace: crate::ForgeWorkspace { repositories: Box::new([]) },
        workstream: 1,
        transcript: Box::new([]),
        answered: Box::new([]),
        settled: Box::new([]),
        grants: Box::new([accounts::Grant {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: skein_lib::Duration::from_secs(1),
        }]),
    }
}

#[test]
fn a_retained_model_context_counts_its_owned_names() {
    use jig_domain_people as people;
    let row = Record::Core(jig_domain_core::Record::People(people::Stored::Answer {
        key: people::RequestKey { person: 1, key: [1; 16] },
        ask: Box::new(people::Ask::SetRoles { project: 1, holdings: Box::new([]) }),
        models: Box::new([Box::from(b"small".as_slice())]),
        outcome: people::Outcome::RolesSet { project: 1 },
        at: Wall::EPOCH,
    }));
    let expected = u64::try_from(size_of::<Box<[u8]>>() + b"small".len()).expect("one name fits");
    assert_eq!(crate::record_bytes(&row), Some(expected));
}

#[test]
fn a_program_declaration_counts_both_retained_names_and_models() {
    use jig_domain_people as people;
    let name = b"work-login";
    let row = Record::Core(jig_domain_core::Record::People(people::Stored::Answer {
        key: people::RequestKey { person: 1, key: [1; 16] },
        ask: Box::new(people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::DeclareProgram { provider: 1, join: None, name: name.as_slice().into() },
        }),
        models: Box::new([Box::from(b"small".as_slice())]),
        outcome: people::Outcome::AccountProgramDeclared { source: 1, name: name.as_slice().into() },
        at: Wall::EPOCH,
    }));
    let expected = size_of::<Box<[u8]>>() + b"small".len() + 2 * name.len();
    assert_eq!(crate::record_bytes(&row), Some(u64::try_from(expected).expect("bounded owned bytes")));
}
#[test]
fn an_issued_program_counts_its_retained_name() {
    let name = b"work-login";
    let row = Record::Core(jig_domain_core::Record::Accounts(jig_domain_accounts::Stored::Issued {
        number: 1,
        holder: jig_domain_accounts::Holder::Party(1),
        name: name.as_slice().into(),
        signed_out: true,
    }));
    assert_eq!(crate::record_bytes(&row), Some(u64::try_from(name.len()).expect("bounded name bytes")));
}

#[test]
fn a_program_sign_out_counts_its_retained_worker_failures() {
    use jig_domain_people as people;
    let name = b"worker-a";
    let row = Record::Core(jig_domain_core::Record::People(people::Stored::Answer {
        key: people::RequestKey { person: 1, key: [1; 16] },
        ask: Box::new(people::Ask::AccountSignOut { holder: people::AccountHolder::Own, source: 1 }),
        models: Box::new([]),
        outcome: people::Outcome::AccountSignedOut {
            confirmed: false,
            failed_hosts: Box::new([people::AccountHostFailure {
                name: name.as_slice().into(),
                why: people::AccountHostWhy::NotInContact,
            }]),
        },
        at: Wall::EPOCH,
    }));
    let expected = size_of::<people::AccountHostFailure>() + name.len();
    assert_eq!(crate::record_bytes(&row), Some(u64::try_from(expected).expect("bounded owned bytes")));
}

#[test]
fn a_conversations_prepared_text_ask_reaches_its_assignment() {
    use crate::boundary::PreparedWorkspace;
    use crate::route::forge_route::RunWorkspace;
    use jig_domain_core as core;

    let (mut domain, env) = resume_fixture::root();
    let fixture = held_assignment();
    domain.brief_sections.insert(fixture.task, Box::new([])).unwrap();
    domain
        .run_workspaces
        .insert(
            fixture.task,
            PreparedWorkspace {
                workspace: RunWorkspace {
                    workspace: crate::ForgeWorkspace { repositories: Box::new([]) },
                    workstream: fixture.task,
                    writes: Box::new([]),
                    names: Box::new([]),
                    own_holds: Box::new([]),
                },
                claim_names: Box::new([]),
            },
        )
        .unwrap();
    let mut requests = Queue::with_capacity(1);
    requests.push(core::Request::Now(Box::new(core::Now::RunPrepared {
        task: fixture.task,
        attempt: fixture.attempt,
        conversation: fixture.conversation,
        live: core::LiveAsk { text: true, reasoning: false, input: false },
        charter: fixture.charter,
        run: fixture.run,
        messages: Box::new([]),
        saved: Box::new([]),
        transcript: Box::new([]),
        answered: Box::new([]),
        grants: Box::new([]),
    })));
    let mut decision = Decision::new(&env.limits.journal);
    crate::route::route_core_requests(&mut domain, &env, &mut decision, core::Requests::Out(requests));
    let assignment = domain.assignments.get(&fixture.task).expect("prepared assignment");
    assert!(assignment.live.text);
    assert!(!assignment.live.reasoning);
    assert!(!assignment.live.input);
}
