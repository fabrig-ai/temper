//! Shared adoption and change fixtures.
use super::*;
pub fn adoption() -> Adoption {
    Adoption {
        project: 5,
        home: true,
        forge: REPO.forge,
        host: Box::from(&b"forge.example"[..]),
        owner: Box::from(&b"org"[..]),
        name: Box::from(&b"repo"[..]),
        prefix: Box::from(&b"temper/"[..]),
        role: Role::Owned,
        landing: Box::from(&b"main"[..]),
        ci: true,
        checks: Box::new([]),
    }
}

/// Play the core after the connector's locate, returning the adoption outputs.
pub fn begin_adoption(d: &mut Domain, request: Token, adoption: Adoption) -> Box<[Request]> {
    outputs(
        d,
        Event::Application {
            role: None,
            carried: None,
            request,
            token: request,
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(adoption),
        },
    );
    let sent = ready(d);
    let [Request::Call { call, .. }] = sent.as_ref() else { panic!("one locate call") };
    outputs(
        d,
        Event::Client(skein_forge_client::Event::Answered {
            free: 0,
            rate: None,
            call: *call,
            cost: 1,
            result: Ok(skein_forge::api::Answer::Located { repository: REPO.repository }),
        }),
    );
    outputs(d, Event::Adopt { request })
}

/// This change and its outbox entry identify the one opening in flight.
pub const CHANGE: u64 = 51;
pub const OPEN: u64 = 11;
/// The branch pair differs so each fresh read has an unambiguous subject.
pub const HEAD: skein_forge::api::Commit = [3; 32];
pub const BASE: skein_forge::api::Commit = [4; 32];

pub fn open_pull() -> skein_forge::api::Pull {
    skein_forge::api::Pull {
        reviewers: Box::new([]),
        number: 1,
        state: skein_forge::api::State::Open,
        head: Box::from(b"temper/51".as_slice()),
        base: Box::from(b"main".as_slice()),
        commit: HEAD,
        base_commit: Some(BASE),
        merged: None,
        mergeable: true,
        ci: skein_forge::api::Ci::Passed,
        queued: None,
        dequeued: None,
    }
}

pub fn opening_row() -> ChangeRow {
    use temper_engine_domain_forge_change as change;
    ChangeRow {
        task: CHANGE,
        repository: REPO,
        branch: Box::from(b"temper/51".as_slice()),
        base: Box::from(b"main".as_slice()),
        title: Box::from(b"Small fix".as_slice()),
        body: Box::new([]),
        priority: 1,
        ci: None,
        change: change::Change {
            way: temper_engine_domain_forge_change::Way::Queue,
            landing: None,
            task: CHANGE,
            state: change::State::Opening { requested: true },
            gates: Box::new([]),
            clean: Box::new([]),
            gate_failures: Box::new([]),
            released: None,
            repairs: 0,
            resolutions: 0,
            updates: 0,
            since: env().wall,
            ready_since: None,
            owns_turn: false,
            last_head: Some(HEAD),
        },
        pull: None,
        pending: Some(OPEN),
        effect: change::EffectResult::Pending,
        delegate: None,
        delegate_status: change::Status::Passed,
        verdicts: Box::new([]),
        gate_remarks: Box::new([]),
        drift: None,
        base_repair: false,
        queue_repair: None,
        queue_repairs: 0,
        producing_charter: 2,
        review_charter: 4,
    }
}

pub fn opening() -> (Domain, Token) {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let row = opening_row();
    let entry = skein_forge_client::Entry {
        number: OPEN,
        task: CHANGE,
        repository: REPO,
        effect: skein_forge_client::Effect {
            write: skein_forge::api::Write::OpenPull {
                title: row.title.clone(),
                body: row.body.clone(),
                head: row.branch.clone(),
                base: row.base.clone(),
            },
            condition: skein_forge_client::Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    outputs(&mut d, Event::Change { row });
    outputs(&mut d, Event::Enqueue { entry });
    outputs(&mut d, Event::Committed { entry: OPEN });
    answer_next(&mut d, Ok(skein_forge::api::Answer::Items { items: Box::new([]), more: false, now: env().now }));
    let (call, _) = next_call(&mut d);
    (d, call)
}

pub fn next_call(d: &mut Domain) -> (Token, skein_forge::api::Op) {
    for request in ready(d) {
        if let Request::Call { call, op, .. } = request {
            return (call, op);
        }
    }
    panic!("one forge call")
}

pub fn answer(
    d: &mut Domain,
    call: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
) -> Box<[Request]> {
    let answered =
        outputs(d, Event::Client(skein_forge_client::Event::Answered { free: 0, rate: None, call, cost: 1, result }));
    d.reclaim();
    answered
}

pub fn answer_next(
    d: &mut Domain,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
) -> Box<[Request]> {
    let (call, _) = next_call(d);
    answer(d, call, result)
}

pub fn step_change(d: &mut Domain) {
    use temper_engine_domain_forge_change as change;
    outputs(
        d,
        Event::StepChange {
            task: CHANGE,
            entry: OPEN + 1,
            heard: change::Heard {
                lacking: None,
                spend: None,
                delegate: change::Status::Passed,
                effect: change::EffectResult::None,
                released: false,
                cancelled: false,
            },
            gates: Box::new([]),
            queue_repair_active: false,
            kept: false,
        },
    );
}

pub fn missing_pull(d: &mut Domain) {
    step_change(d);
    missing_pull_reads(d);
}

pub fn missing_pull_reads(d: &mut Domain) {
    answer_next(d, Ok(skein_forge::api::Answer::Commit(HEAD)));
    answer_next(d, Ok(skein_forge::api::Answer::Commit(BASE)));
    answer_next(d, Err(skein_forge::api::Error::Missing));
}

pub fn finish_missing(d: &mut Domain) -> Box<[Request]> {
    let green = checks(skein_forge::api::Check::Passed);
    answer_next(d, Ok(green.clone()));
    answer_next(d, Ok(green));
    answer_next(
        d,
        Ok(skein_forge::api::Answer::Compare {
            before: BASE,
            after: HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    )
}

/// `OpenPull` succeeds after the current chain has already read Missing.
pub fn opened_during_chain() -> Domain {
    let (mut d, opening) = opening();
    missing_pull(&mut d);
    answer(&mut d, opening, Ok(skein_forge::api::Answer::Created(1)));
    d
}

/// A reported CI row, shared by the fresh-read fixtures.
pub fn checks(check: skein_forge::api::Check) -> skein_forge::api::Answer {
    skein_forge::api::Answer::Checks(Box::new([skein_forge::api::Status {
        at: Time::ZERO,
        context: Box::from(b"ci".as_slice()),
        check,
        description: Box::new([]),
        url: Box::new([]),
        job: None,
    }]))
}

/// The matching pull already supplied head CI, leaving just the base and comparison.
pub fn finish_open(d: &mut Domain) -> Box<[Request]> {
    answer_next(d, Ok(checks(skein_forge::api::Check::Passed)));
    answer_next(
        d,
        Ok(skein_forge::api::Answer::Compare {
            before: BASE,
            after: HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    )
}

/// The merge commit differs from both parents.
pub const MERGE: skein_forge::api::Commit = [5; 32];
pub const PROJECT: u32 = 5;
pub const BROWSER: &[u8] = b"browser";
pub const FUZZY: &[u8] = b"fuzzy";

pub fn repository_tiers(names: &[&[u8]]) -> Box<[RepositoryTiers]> {
    let mut tiers = skein_lib::List::with_capacity(u32::try_from(names.len()).expect("fixture names"));
    for name in names {
        tiers.push(Tier { name: Box::from(*name) }).expect("fixture names");
    }
    Box::new([RepositoryTiers { repository: REPO, tiers: tiers.into_boxed() }])
}

pub fn verifying() -> Domain {
    use temper_engine_domain_forge_change as change;
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    d.project_tiers(PROJECT, repository_tiers(&[BROWSER, FUZZY]));
    let mut row = opening_row();
    row.change.state = change::State::Landed { merge: MERGE };
    row.pending = None;
    row.effect = change::EffectResult::Made;
    outputs(&mut d, Event::Restore { record: Stored::Change(row) });
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d
}

pub fn verification_step(d: &mut Domain) -> Box<[Request]> {
    use temper_engine_domain_forge_change as change;
    outputs(
        d,
        Event::StepChange {
            task: CHANGE,
            entry: OPEN,
            heard: change::Heard {
                delegate: change::Status::Passed,
                effect: change::EffectResult::Made,
                released: false,
                cancelled: false,
            },
            gates: Box::new([]),
            queue_repair_active: false,
            kept: false,
        },
    )
}

pub fn judge(d: &mut Domain, tier: &[u8], blame: temper_engine_domain_forge_tiers::Blame) -> Box<[Request]> {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::judged(d, REPO, tier, OPEN, &[(MERGE, blame)], &mut out);
    let mut requests = skein_lib::List::with_capacity(out.len());
    while let Some(request) = out.pop() {
        requests.push(request).expect("sized to output");
    }
    requests.into_boxed()
}

/// A committed merge entry, through the client's ordinary find and make.
pub fn merged(tiers: bool) -> (Domain, Box<[Request]>) {
    use skein_forge::api::{Answer, Write};
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    if tiers {
        d.project_tiers(PROJECT, repository_tiers(&[BROWSER, FUZZY]));
    }
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let entry = skein_forge_client::Entry {
        number: OPEN,
        task: CHANGE,
        repository: REPO,
        effect: skein_forge_client::Effect {
            write: Write::Merge { number: 1, head: HEAD },
            condition: skein_forge_client::Condition::Merge { base: Box::from(b"main".as_slice()) },
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    outputs(&mut d, Event::Enqueue { entry });
    let committed = outputs(&mut d, Event::Committed { entry: OPEN });
    let clocked = answer_after(&mut d, &committed, Answer::Items { items: Box::new([]), more: false, now: env().now });
    let checked = answer_after(&mut d, &clocked, Answer::Pull(open_pull()));
    let made = answer_after(&mut d, &checked, Answer::Merged(MERGE));
    let mut pull = open_pull();
    pull.state = skein_forge::api::State::Closed;
    pull.merged = Some(MERGE);
    let requests = answer_after(&mut d, &made, Answer::Pull(pull));
    (d, requests)
}

fn answer_after(d: &mut Domain, requests: &[Request], result: skein_forge::api::Answer) -> Box<[Request]> {
    for request in requests {
        if let Request::Call { call, .. } = request {
            return answer(d, *call, Ok(result));
        }
    }
    answer_next(d, Ok(result))
}
