use super::*;

const TASK: u64 = 1;
const ATTEMPT: u64 = 2;
const HEAD: skein_forge::api::Commit = [7; 32];
const RESTART: Clock = Clock { now: 1 };

#[test]
fn a_worker_reports_its_unchanged_push_head_after_an_engine_restart() {
    let config = Config::new(71, Some(jig_conformance::scenarios::Scenario::Grace));
    let mut peers = Temper::peers(&config, 71);
    peers.reported.insert(TASK, HEAD);
    Temper::cold(&mut peers, &Store::new(), RESTART);
    let event = link::worker_event(
        &mut peers,
        workers::Up::Answer {
            channel: link::CHANNEL,
            task: TASK,
            attempt: ATTEMPT,
            spent: 0,
            end: workers::End::Parked,
            read: None,
            refused: Box::new([]),
        },
    );
    let engine::Event::Worker(engine::WorkerInput::Link { link: engine::Link::Up { parts, .. }, .. }) = event else {
        panic!("worker answer reaches the engine");
    };
    assert_eq!(
        parts.pushed,
        Box::new([forge::Pushed {
            tag: forge::tag(temper_engine_forge_world::REPO.forge, temper_engine_forge_world::REPO.repository),
            commit: HEAD
        }]) as Box<[forge::Pushed]>
    );
}

#[test]
fn ci_evidence_follows_native_commit_and_pull_reads() {
    use skein_fake_forge_domain::api as raw;
    use skein_forge::api;
    use temper_engine_forge_world::translate;

    let clock = Clock { now: 1_000_000_000 };
    let mut forge = systems::FakeForge::new(91);
    forge.create(clock, b"change");
    let head = forge.advance(clock, b"change", b"new head");
    forge.poll(clock);
    let raw_head = forge.domain.branch(b"org/repo", b"change").expect("change head");
    let raw::Answer::Created(number) = forge.external(
        clock,
        raw::Op::Write(raw::Write::OpenPull {
            title: b"change".as_slice().into(),
            body: Box::new([]),
            head: b"change".as_slice().into(),
            base: b"main".as_slice().into(),
            opens_empty: false,
        }),
    ) else {
        panic!("opened pull");
    };
    let reads = [
        api::Read::Statuses { commit: head, page: 1 },
        api::Read::Checks { commit: head },
        api::Read::PullFor { head: b"change".as_slice().into(), base: b"main".as_slice().into() },
        api::Read::Pull { number },
    ];
    let store = Store::new();
    for state in [raw::Check::Passed, raw::Check::Pending, raw::Check::Failed] {
        forge.external(
            clock,
            raw::Op::Write(raw::Write::Status {
                commit: raw_head,
                context: b"build".as_slice().into(),
                state,
                description: Box::new([]),
                link: Box::new([]),
            }),
        );
        for read in &reads {
            forge.take_observed();
            let events =
                forge.call(&store, clock, Token::new(1), temper_engine_forge_world::REPO, api::Op::Read(read.clone()));
            assert!(events.is_empty());
            forge.poll(clock);
            let observed = forge.take_observed();
            let [r::Observed::Read { resource, state: observed_state, at }] = observed.as_slice() else {
                panic!("one CI observation for {read:?}: {observed:?}");
            };
            let mut expected = observations::repository();
            expected.path.extend([b"branch".to_vec(), b"main".to_vec()]);
            assert_eq!(*resource, expected);
            assert_eq!(*observed_state, (state == raw::Check::Passed).then(|| observations::value(&head)));
            assert_eq!(*at, clock.now);
        }
    }
    // Reading the base's passing CI must not stand in for the change head.
    let base = translate::commit(forge.domain.branch(b"org/repo", b"main").expect("main head"));
    forge.call(
        &store,
        clock,
        Token::new(1),
        temper_engine_forge_world::REPO,
        api::Op::Read(api::Read::Checks { commit: base }),
    );
    forge.poll(clock);
    assert!(forge.take_observed().is_empty());
}
