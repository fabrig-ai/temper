use crate::decision::{
    Counters, Decision, Delivery, Journal as SkeinJournal, JournalOutput as Output, Limits, accept_routed,
    committed as mark_committed, fresh as fresh_number, resume as release_ready,
};
use crate::{Deployment, Family, Key, Record, TurnRecord, Write};
use alloc::boxed::Box;
use skein_lib::{List, Queue, Token, Wall};

#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

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
    fn bootstrap(id: [u8; 16], _limits: &Limits) -> Journal {
        Journal { inner: SkeinJournal::uncapped(1, 0), counters: Counters::bootstrap(id), durable: 0 }
    }

    fn new(deployment: Deployment, _limits: &Limits) -> Journal {
        Journal {
            inner: SkeinJournal::uncapped(1, deployment.commits),
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

fn fresh(journal: &mut Journal, family: Family) -> Option<u64> {
    if journal.inner.stopped() { None } else { fresh_number(&mut journal.counters, family) }
}
#[expect(clippy::result_large_err, reason = "test wrapper preserves the decision's owned refusal")]
fn accept(journal: &mut Journal, limits: &Limits, decision: Decision, out: &mut Queue<Output>) -> Result<(), Decision> {
    assert!(out.room() >= 1, "one root journal output reserved");
    accept_routed(&mut journal.inner, &mut journal.counters, limits, decision, true)?;
    if let Some(output) = crate::decision::commit(&mut journal.inner, limits) {
        out.push(output);
    }
    Ok(())
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

fn decision(journal: &mut Journal, number: u32) -> Decision {
    let mut d = Decision::decide(&mut journal.inner, &LIMITS);
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
    let mut d = Decision::decide(&mut j.inner, &LIMITS);
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
    let d = decision(&mut j, 1);
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
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
    let d = Decision::decide(&mut j.inner, &LIMITS);
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
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
    let mut d = Decision::decide(&mut j.inner, &LIMITS);
    d.write(&LIMITS, turn(1, 2)).unwrap();
    d.write(&LIMITS, turn(2, 3)).unwrap();
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
    let mut d = Decision::decide(&mut j.inner, &LIMITS);
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
fn a_decision_past_the_store_write_edge_stops_the_root_journal() {
    for writes in [LIMITS.writes] {
        let mut journal = Journal::new(DEPLOYMENT, &LIMITS);
        let mut out = Queue::with_capacity(1);
        let mut decision = Decision::decide(&mut journal.inner, &LIMITS);
        for number in 1..writes {
            decision.write(&LIMITS, turn(number, 1)).expect("reserved write room");
        }
        decision.deliver(&LIMITS, ack(1)).unwrap();
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
        assert!(journal.inner.stopped());
    }
}

#[test]
fn decisions_keep_the_header_and_release_exactly_the_admitted_deliveries() {
    for (write_count, deliveries, persist_header) in [
        (0, 0, false),
        (0, 2, false),
        (1, 0, true),
        (1, 2, true),
        (1, 2, false),
        (3, 0, true),
        (3, 2, true),
        (3, 2, false),
    ] {
        let mut journal = Journal::bootstrap(DEPLOYMENT.id, &LIMITS);
        let mut decision = Decision::decide(&mut journal.inner, &LIMITS);
        for number in 1..write_count {
            decision.carry_write(&LIMITS, turn(number, 1)).unwrap();
        }
        for number in 1..=deliveries {
            decision.carry_delivery(&LIMITS, ack(number));
        }
        let header = Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
            DEPLOYMENT,
        ))));
        assert_eq!(decision.write(&LIMITS, header.clone()), Err(header), "caller cannot own the header slot");
        accept_routed(&mut journal.inner, &mut journal.counters, &LIMITS, decision, persist_header).unwrap();
        let mut out = Queue::with_capacity(1);
        if write_count <= 1 && !persist_header {
            assert!(crate::decision::commit(&mut journal.inner, &LIMITS).is_none());
            assert!(journal.counters.dirty(), "read-only store terminals never persist the dirty header");
        } else {
            out.push(crate::decision::commit(&mut journal.inner, &LIMITS).unwrap());
            let (number, writes) = commit(&mut out);
            assert_eq!((number, writes.len()), (1, usize::try_from(write_count).unwrap()));
            assert_eq!(
                writes[0],
                Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
                    Deployment { commits: 1, ..DEPLOYMENT }
                ))))
            );
            for number in 1..write_count {
                assert_eq!(writes[usize::try_from(number).unwrap()], turn(number, 1));
            }
            resume(&mut journal, &mut out);
            assert!(out.is_empty(), "held effects wait for the header's commit");
            committed(&mut journal, number);
        }
        for number in 1..=deliveries {
            resume(&mut journal, &mut out);
            assert_eq!(acknowledged(&mut out), number, "each admitted delivery releases once, in order");
        }
        resume(&mut journal, &mut out);
        assert!(out.is_empty());
        assert!(!journal.stopped());
    }
}

#[test]
fn a_decision_with_more_outputs_than_the_door_releases_them_all_in_order() {
    let mut journal = Journal::new(DEPLOYMENT, &LIMITS);
    let mut decision = Decision::decide(&mut journal.inner, &LIMITS);
    decision.write(&LIMITS, turn(1, 1)).unwrap();
    for number in 1..=LIMITS.deliveries + 2 {
        decision.deliver(&LIMITS, ack(number)).unwrap();
    }
    let mut out = Queue::with_capacity(1);
    accept(&mut journal, &LIMITS, decision, &mut out).unwrap();
    let (number, writes) = commit(&mut out);
    assert_eq!(number, 1);
    assert_eq!(writes.len(), 2, "one write and the deployment header");
    assert_eq!(writes[1], turn(1, 1));
    resume(&mut journal, &mut out);
    assert!(out.is_empty(), "all outputs wait for durability");
    committed(&mut journal, number);
    for number in 1..=LIMITS.deliveries + 2 {
        resume(&mut journal, &mut out);
        assert_eq!(acknowledged(&mut out), number, "every delivery releases once, in order");
    }
    resume(&mut journal, &mut out);
    assert!(out.is_empty());
    assert!(!journal.stopped());
}

#[test]
fn a_root_store_edge_overrun_releases_only_stop_and_discards_immediate_outputs() {
    let (mut domain, env) = resume_fixture::root();
    let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
    for number in 1..=env.limits.journal.writes {
        crate::domain::save(&mut decision, &env.limits, turn(number, 1));
    }
    let mut out = Queue::with_capacity(crate::max_out(&env.limits));
    out.push(crate::Request::Drained);
    crate::domain::close(&mut domain, &env, decision, &mut out);
    crate::domain::stage_now(&mut domain, &mut out);
    crate::release(&mut domain, &env, &mut out);
    assert_eq!(out.pop(), Some(crate::Request::Stop));
    assert!(out.is_empty());
    crate::try_step(&mut domain, &env, crate::Event::Resume).unwrap();
    crate::release(&mut domain, &env, &mut out);
    assert!(out.is_empty(), "nothing from the failed decision or after it leaves");
    assert!(domain.journal.stopped());
}

#[test]
fn new_work_waits_at_the_watermark_and_admitted_work_is_always_taken() {
    use crate::{Event, PartyInput, StoreInput};
    use skein_lib::{ReplyTo, Token};
    fn call() -> Event {
        Event::Party(PartyInput::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(72)),
            sign_in: 99,
            key: [7; 16],
            ask: jig_domain_people::Ask::SetRoles { project: 1, holdings: Box::new([]) },
        })
    }
    let mut limits = resume_fixture::limits();
    limits.watermark = limits.journal.commits + 2;
    assert_ne!(limits.watermark, crate::default_watermark(&limits.journal));
    let (mut domain, env) = resume_fixture::root_with_config_and_limits(resume_fixture::config(1), &limits);
    // Header-only commits give an exact backlog without held-output entries.
    for _ in 0..u64::from(env.limits.watermark) - 1 {
        crate::fresh(&mut domain.core.counters, Family::Message).unwrap();
        crate::try_step(&mut domain, &env, Event::Resume).unwrap();
    }
    assert_eq!(crate::limits::backlog(&domain), u64::from(env.limits.watermark) - 1);
    assert!(crate::limits::backlog(&domain) > u64::from(env.limits.journal.commits));
    crate::try_step(&mut domain, &env, call()).expect("past commits but below the watermark the call is taken");
    // This unauthenticated call owns one held refusal, bringing backlog to the mark.
    assert_eq!(crate::limits::backlog(&domain), u64::from(env.limits.watermark));
    let returned = crate::try_step(&mut domain, &env, call()).expect_err("at the mark new work waits");
    let Event::Party(PartyInput::Ask { reply_to, sign_in, key, ask, models }) = &returned else {
        panic!("the original call is returned");
    };
    assert_eq!(reply_to, &ReplyTo::new(Token::new(72)));
    assert_eq!((*sign_in, *key), (99, [7; 16]));
    assert!(models.is_empty());
    assert_eq!(ask, &jig_domain_people::Ask::SetRoles { project: 1, holdings: Box::new([]) });
    let released = Event::Released(crate::Released(Delivery::Fleet(jig_domain_fleet::Event::TurnKept {
        run: Token::new(1),
        attempt: Token::new(1),
        turn: 1,
    })));
    crate::try_step(&mut domain, &env, released).expect("released work advances at the mark");
    let mut last = 0;
    while let Some(Output::Commit { number, .. }) = crate::commit(&mut domain.journal, &env.limits.journal) {
        last = number;
    }
    crate::try_step(&mut domain, &env, Event::Store(StoreInput::Committed { number: last, fill: 0 }))
        .expect("store completion is always taken");
    assert!(crate::limits::backlog(&domain) < u64::from(env.limits.watermark));
    crate::try_step(&mut domain, &env, returned).expect("after completion the returned call is taken");
    assert!(!domain.journal.stopped());
}

#[test]
fn counter_and_commit_overflow_cannot_reuse_names() {
    let mut j = Journal::new(Deployment { tasks: u64::MAX, commits: u64::MAX, ..DEPLOYMENT }, &LIMITS);
    assert_eq!(fresh(&mut j, Family::Task), None);
    let mut out = Queue::with_capacity(1);
    let d = decision(&mut j, 1);
    accept(&mut j, &LIMITS, d, &mut out).unwrap();
    assert!(j.stopped());
    assert!(out.is_empty());
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
    let mut journal = SkeinJournal::uncapped(1, 0);
    let mut decision = Decision::decide(&mut journal, &limits);
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
    let mut journal = SkeinJournal::uncapped(1, 0);
    let mut decision = Decision::decide(&mut journal, &limits);
    let mut assignment = held_assignment();
    assert!(
        decision
            .deliver(&limits, Delivery::Assigned { channel: Token::new(1), assignment: assignment.clone() })
            .is_err()
    );
    assignment.sections = Box::new([]);
    let bytes = crate::limits::run_charter_bytes(&assignment.run).expect("fixture charter bytes");
    let exact = crate::JournalLimits { run_bytes: u32::try_from(bytes).expect("tiny charter"), ..limits };
    let mut journal = SkeinJournal::uncapped(1, 0);
    let mut decision = Decision::decide(&mut journal, &exact);
    decision
        .deliver(&exact, Delivery::Assigned { channel: Token::new(1), assignment: assignment.clone() })
        .expect("the exact run byte envelope accepts its binding arrays");
    assignment.run.sources = Box::new([Some(1), Some(2)]);
    assignment.run.accounts = Box::new([
        jig_domain_authority::AccountBinding { provider: 1, account: 1, source: 1 },
        jig_domain_authority::AccountBinding { provider: 2, account: 2, source: 2 },
    ]);
    let mut journal = SkeinJournal::uncapped(1, 0);
    let mut decision = Decision::decide(&mut journal, &exact);
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
            revision: 1,
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
    let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
    crate::route::route_core_requests(&mut domain, &env, &mut decision, core::Requests::Out(requests));
    let assignment = domain.assignments.get(&fixture.task).expect("prepared assignment");
    assert!(assignment.live.text);
    assert!(!assignment.live.reasoning);
    assert!(!assignment.live.input);
}

#[test]
fn a_refused_settlement_uses_cores_returned_reply_address() {
    let (mut domain, _) = resume_fixture::root();
    let payload = domain
        .payloads
        .insert(Some(crate::boundary::Payload::RefusedCall { name: b"returned opaque address".as_slice().into() }))
        .unwrap();
    let delivery = crate::route::host_route::relayed(
        &mut domain,
        Token::new(7),
        Token::new(3),
        Token::new(9),
        b"fleet address".as_slice().into(),
        payload.token(),
    );
    assert_eq!(
        delivery,
        Delivery::Link {
            channel: Token::new(7),
            down: jig_domain_host::LinkDown::Relayed {
                run: Token::new(3),
                attempt: Token::new(9),
                call: b"returned opaque address".as_slice().into(),
                answer: jig_domain_host::Relayed::Answer { error: true, text: b"busy".as_slice().into() },
            },
        }
    );
}

#[test]
fn zero_watermark_refuses_startup_and_the_default_saturates() {
    let mut limits = resume_fixture::limits();
    limits.watermark = 0;
    assert!(crate::worst_case(&limits).is_none());
    let journal = crate::JournalLimits { commits: u32::MAX, ..limits.journal };
    assert_eq!(crate::default_watermark(&journal), u32::MAX);
}
