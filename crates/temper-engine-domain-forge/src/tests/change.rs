use super::*;

// env() starts at 100 s; the change policy stalls for another 30 s.
const WAIT_UNTIL: Wall = Wall::from_nanos(130_000_000_000);

const LATER_CHANGE: u64 = fixtures::CHANGE + 1;
const FIRST_ENTRY: u64 = fixtures::OPEN + 1;
const LATER_ENTRY: u64 = FIRST_ENTRY + 1;
const LATER_BRANCH: &[u8] = b"temper/52";

fn start_opening(d: &mut Domain, task: u64, entry: u64) {
    start_opening_kept(d, task, entry, false);
}

fn start_opening_kept(d: &mut Domain, task: u64, entry: u64, kept: bool) {
    use temper_engine_domain_forge_change as change;
    outputs(
        d,
        Event::StepChange {
            task,
            entry,
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
            kept,
        },
    );
}

/// Leave the lower chain's last read in flight while the higher entry commits.
fn reversed_openings(lower_requested: bool) -> (Domain, Token) {
    use temper_engine_domain_forge_change as change;
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let mut row = fixtures::opening_row();
    row.pending = None;
    row.pull = None;
    row.effect = change::EffectResult::None;
    row.change.state = change::State::Opening { requested: lower_requested };
    outputs(&mut d, Event::Change { row: row.clone() });
    row.task = LATER_CHANGE;
    row.change.task = LATER_CHANGE;
    row.change.state = change::State::Opening { requested: false };
    row.branch = Box::from(LATER_BRANCH);
    outputs(&mut d, Event::Change { row });
    start_opening(&mut d, fixtures::CHANGE, FIRST_ENTRY);
    fixtures::missing_pull_reads(&mut d);
    let green = fixtures::checks(skein_forge::api::Check::Passed);
    fixtures::answer_next(&mut d, Ok(green.clone()));
    fixtures::answer_next(&mut d, Ok(green));
    let (lower, _) = fixtures::next_call(&mut d);
    start_opening(&mut d, LATER_CHANGE, LATER_ENTRY);
    fixtures::missing_pull_reads(&mut d);
    let built = fixtures::finish_missing(&mut d);
    assert!(built_entry(&built, LATER_ENTRY), "higher effect: {built:?}");
    outputs(&mut d, Event::Committed { entry: LATER_ENTRY });
    (d, lower)
}

fn answer_entry_clock(d: &mut Domain) {
    fixtures::answer_next(d, Ok(skein_forge::api::Answer::Items { items: Box::new([]), more: false, now: env().now }));
}

fn built_entry(requests: &[Request], number: u64) -> bool {
    for request in requests {
        if let Request::ChangeDecision { entry: Some(entry), .. } = request
            && *entry == number
        {
            return true;
        }
    }
    false
}

#[test]
fn changes_finishing_in_reverse_order_reach_the_client_in_number_order() {
    use skein_forge::api::{Answer, Op, Write};
    let (mut d, lower) = reversed_openings(false);
    let built = finish_lower(&mut d, lower);
    assert!(built_entry(&built, FIRST_ENTRY), "lower effect: {built:?}");
    outputs(&mut d, Event::Committed { entry: FIRST_ENTRY });
    answer_entry_clock(&mut d);
    let (first, op) = fixtures::next_call(&mut d);
    let Op::Write(Write::OpenPull { head, .. }) = op else { panic!("first opening") };
    assert_eq!(head.as_ref(), b"temper/51");
    fixtures::answer(&mut d, first, Ok(Answer::Created(2)));
    answer_entry_clock(&mut d);
    let (_, op) = fixtures::next_call(&mut d);
    let Op::Write(Write::OpenPull { head, .. }) = op else { panic!("later opening") };
    assert_eq!(head.as_ref(), LATER_BRANCH);
}

fn finish_lower(d: &mut Domain, lower: Token) -> Box<[Request]> {
    fixtures::answer(
        d,
        lower,
        Ok(skein_forge::api::Answer::Compare {
            before: fixtures::BASE,
            after: fixtures::HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    )
}

#[test]
fn a_woken_chain_restarts_with_its_newest_entry_number() {
    use skein_forge::api::{Answer, Op, Read, Write};
    let (mut d, lower) = reversed_openings(false);
    let newest = LATER_ENTRY + 2;
    start_opening(&mut d, fixtures::CHANGE, newest - 1);
    start_opening(&mut d, fixtures::CHANGE, newest);
    assert!(finish_lower(&mut d, lower).is_empty(), "the woken chain restarts");
    let (branch, op) = fixtures::next_call(&mut d);
    assert_eq!(op, Op::Read(Read::Branch { branch: Box::from(b"temper/51".as_slice()) }));
    // Leave the restarted chain's first read unanswered while B writes.
    answer_entry_clock(&mut d);
    let (later, op) = fixtures::next_call(&mut d);
    let Op::Write(Write::OpenPull { head, .. }) = op else { panic!("later opening before restarted reads finish") };
    assert_eq!(head.as_ref(), LATER_BRANCH);
    fixtures::answer(&mut d, later, Ok(Answer::Created(2)));
    fixtures::answer(&mut d, branch, Ok(Answer::Commit(fixtures::HEAD)));
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::BASE)));
    fixtures::answer_next(&mut d, Err(skein_forge::api::Error::Missing));
    let built = fixtures::finish_missing(&mut d);
    assert!(built_entry(&built, newest), "restarted effect uses the newest reservation: {built:?}");
}

#[test]
fn a_wake_that_unkeeps_an_inflight_chain_is_not_lost() {
    use skein_forge::api::Answer;
    use temper_engine_domain_forge_change as change;
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let mut row = fixtures::opening_row();
    row.pending = None;
    row.effect = change::EffectResult::None;
    row.pull = Some(1);
    row.change.state = change::State::Gating { head: fixtures::HEAD, asked: Box::new([]) };
    outputs(&mut d, Event::Change { row });
    start_opening_kept(&mut d, fixtures::CHANGE, FIRST_ENTRY, true);
    fixtures::missing_pull_reads(&mut d);
    start_opening(&mut d, fixtures::CHANGE, LATER_ENTRY);
    assert!(fixtures::finish_missing(&mut d).is_empty(), "the kept chain restarts");
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::HEAD)));
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::BASE)));
    fixtures::answer_next(&mut d, Ok(Answer::Pull(fixtures::open_pull())));
    fixtures::finish_open(&mut d);
    let finished = fixtures::answer_next(&mut d, Ok(Answer::Reviews { reviews: Box::new([]), more: false }));
    assert!(
        finished.contains(&Request::ChangeDecision {
            task: fixtures::CHANGE,
            decision: change::Decision::Ready,
            entry: None,
            evidence: None,
        }),
        "the newest wake unkeeps the change: {finished:?}"
    );
}

#[test]
fn a_lower_chain_waiting_without_an_entry_releases_the_later_entry() {
    let (mut d, lower) = reversed_openings(true);
    assert!(ready(&mut d).is_empty(), "the later entry waits for the lower chain");
    let waited = finish_lower(&mut d, lower);
    assert!(
        waited.contains(&Request::ChangeDecision {
            task: fixtures::CHANGE,
            decision: temper_engine_domain_forge_change::Decision::Wait { until: Some(WAIT_UNTIL) },
            entry: None,
            evidence: None,
        }),
        "lower wait: {waited:?}"
    );
    assert!(d.is_ready(), "the committed entry keeps the connector ready");
    answer_entry_clock(&mut d);
    let (_, op) = fixtures::next_call(&mut d);
    let skein_forge::api::Op::Write(skein_forge::api::Write::OpenPull { head, .. }) = op else {
        panic!("later opening")
    };
    assert_eq!(head.as_ref(), LATER_BRANCH);
}

#[test]
fn a_committed_entry_waiting_for_a_lower_chain_can_be_withdrawn() {
    let (mut d, _) = reversed_openings(false);
    let withdrawn = outputs(&mut d, Event::Withdraw { entry: LATER_ENTRY });
    assert!(withdrawn.contains(&Request::Outcome {
        entry: LATER_ENTRY,
        task: LATER_CHANGE,
        outcome: skein_forge_client::Outcome::Withdrawn,
    }));
    assert!(ready(&mut d).is_empty(), "withdrawal releases no forge call");
}

#[test]
fn an_overtaking_open_outcome_restarts_the_fresh_chain() {
    use skein_forge::api::{Answer, Op, Read};
    use temper_engine_domain_forge_change as change;
    let mut d = fixtures::opened_during_chain();
    fixtures::step_change(&mut d);
    let overtaken = fixtures::finish_missing(&mut d);
    let (branch, branch_read) = fixtures::next_call(&mut d);
    fixtures::answer(&mut d, branch, Ok(Answer::Commit(fixtures::HEAD)));
    let (base, base_read) = fixtures::next_call(&mut d);
    fixtures::answer(&mut d, base, Ok(Answer::Commit(fixtures::BASE)));
    let (pull, pull_read) = fixtures::next_call(&mut d);
    fixtures::answer(&mut d, pull, Ok(Answer::Pull(fixtures::open_pull())));
    fixtures::finish_open(&mut d);
    let finished = fixtures::answer_next(&mut d, Ok(Answer::Reviews { reviews: Box::new([]), more: false }));
    assert_eq!(overtaken.as_ref(), &[]);
    assert_eq!(
        [branch_read, base_read, pull_read],
        [
            Op::Read(Read::Branch { branch: Box::from(b"temper/51".as_slice()) }),
            Op::Read(Read::Branch { branch: Box::from(b"main".as_slice()) }),
            Op::Read(Read::PullFor { head: Box::from(b"temper/51".as_slice()), base: Box::from(b"main".as_slice()) }),
        ]
    );
    let [Request::Save { .. }, Request::Marked { tasks }, Request::ChangeDecision { task, decision, entry, evidence }] =
        finished.as_ref()
    else {
        panic!("one saved decision")
    };
    assert_eq!(tasks.as_ref(), &[fixtures::CHANGE]);
    assert_eq!((*task, decision, entry, evidence), (fixtures::CHANGE, &change::Decision::Ready, &None, &None));
    assert_eq!(ready(&mut d).as_ref(), &[]);
}

#[test]
fn an_unwoken_chain_finishes_with_its_single_decision() {
    use temper_engine_domain_forge_change as change;
    let mut d = fixtures::opened_during_chain();
    let finished = fixtures::finish_missing(&mut d);
    let [Request::Save { .. }, Request::Marked { tasks }, decision] = finished.as_ref() else {
        panic!("one saved decision")
    };
    assert_eq!(tasks.as_ref(), &[fixtures::CHANGE]);
    assert_eq!(
        decision,
        &Request::ChangeDecision {
            task: fixtures::CHANGE,
            decision: change::Decision::Wait { until: Some(WAIT_UNTIL) },
            entry: None,
            evidence: None,
        }
    );
    assert_eq!(ready(&mut d).as_ref(), &[]);
}

fn merging_change(protection: Protection, pull_ci: skein_forge::api::Ci) -> Domain {
    first_change(protection, pull_ci, true)
}

fn first_change(protection: Protection, pull_ci: skein_forge::api::Ci, mergeable: bool) -> Domain {
    use skein_forge::api::Answer;
    use temper_engine_domain_forge_change as change;
    let mut d = domain();
    let mut repository = adopted();
    repository.protection = protection;
    outputs(&mut d, Event::Restore { record: Stored::Repository(repository) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let mut row = fixtures::opening_row();
    row.pending = None;
    row.effect = change::EffectResult::None;
    row.pull = Some(1);
    row.change.state = change::State::First { head: fixtures::HEAD, ready_since: env().wall };
    row.change.owns_turn = true;
    outputs(&mut d, Event::Change { row });
    fixtures::step_change(&mut d);
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::HEAD)));
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::BASE)));
    let mut pull = fixtures::open_pull();
    pull.ci = pull_ci;
    pull.mergeable = mergeable;
    fixtures::answer_next(&mut d, Ok(Answer::Pull(pull)));
    d
}

fn merge_after_base_reads(d: &mut Domain) {
    use skein_forge::api::{Answer, Check, Op, Read};
    use temper_engine_domain_forge_change as change;
    let (call, op) = fixtures::next_call(d);
    assert_eq!(op, Op::Read(Read::Checks { commit: fixtures::BASE }));
    fixtures::answer(d, call, Ok(fixtures::checks(Check::Passed)));
    fixtures::answer_next(
        d,
        Ok(Answer::Compare {
            before: fixtures::BASE,
            after: fixtures::HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    );
    let finished = fixtures::answer_next(d, Ok(Answer::Reviews { reviews: Box::new([]), more: false }));
    let mut merging = false;
    for request in &finished {
        if let Request::ChangeDecision {
            decision: change::Decision::Effect(change::Effect::Merge { head, .. }), ..
        } = request
        {
            assert_eq!(*head, fixtures::HEAD);
            merging = true;
        }
    }
    assert!(merging, "merge at the checked head: {finished:?}");
}

#[test]
fn passed_pull_ci_reads_the_head_contexts_for_its_landed_result() {
    let mut d = merging_change(Protection::Absent, skein_forge::api::Ci::Passed);
    let (call, op) = fixtures::next_call(&mut d);
    assert_eq!(op, skein_forge::api::Op::Read(skein_forge::api::Read::Checks { commit: fixtures::HEAD }));
    fixtures::answer(&mut d, call, Ok(fixtures::checks(skein_forge::api::Check::Passed)));
    merge_after_base_reads(&mut d);
}

#[test]
fn required_head_checks_ignore_a_failed_non_required_row() {
    use skein_forge::api::{Answer, Check, Op, Read};
    let protection = Protection::Rule(skein_forge::api::Protection {
        queue: false,
        signatures: false,
        linear: false,
        code_owners: false,
        bypass: skein_forge::api::Bypass::Never,
        branch: Box::from(b"main".as_slice()),
        contexts: Box::new([Box::from(b"ci".as_slice())]),
        approvals: 0,
        dismiss_stale: false,
    });
    // The failed lint row makes the pull's aggregate fail, too.
    let mut d = merging_change(protection, skein_forge::api::Ci::Failed);
    let (call, op) = fixtures::next_call(&mut d);
    assert_eq!(op, Op::Read(Read::Checks { commit: fixtures::HEAD }));
    let Answer::Checks(rows) = fixtures::checks(Check::Passed) else { unreachable!() };
    let mut lint = rows[0].clone();
    lint.context = Box::from(b"lint".as_slice());
    lint.check = Check::Failed;
    fixtures::answer(&mut d, call, Ok(Answer::Checks(Box::new([lint, rows[0].clone()]))));
    merge_after_base_reads(&mut d);
}

fn named_pull_verdict(number: u64) -> Box<[Request]> {
    use skein_forge::api::{Answer, Ci, Kind, Op, Read, State, Summary};
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let mut row = fixtures::opening_row();
    row.pull = Some(1);
    outputs(&mut d, Event::Change { row: row.clone() });
    // Another change in the same repository must not wake with this pull.
    row.task = LATER_CHANGE;
    row.change.task = LATER_CHANGE;
    row.pull = Some(2);
    row.branch = Box::from(LATER_BRANCH);
    outputs(&mut d, Event::Change { row });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(
        &mut d,
        Event::Names {
            task: fixtures::CHANGE,
            resources: Box::new([Name { forge: REPO.forge, repository: REPO.repository, what: What::Pull(number) }]),
        },
    );
    let mut seen = skein_lib::List::with_capacity(max_out(&LIMITS).checked_mul(4).expect("four bounded output passes"));
    // At most a repository listing, the item, and the pull itself.
    for _ in 0_u32..3 {
        let (call, op) = fixtures::next_call(&mut d);
        if op == Op::Read(Read::Pull { number }) {
            let mut pull = fixtures::open_pull();
            pull.number = number;
            pull.ci = Ci::Passed;
            for request in fixtures::answer(&mut d, call, Ok(Answer::Pull(pull))) {
                seen.push(request).expect("four bounded output passes");
            }
            for request in ready(&mut d) {
                seen.push(request).expect("four bounded output passes");
            }
            return seen.into_boxed();
        }
        let answer = if let Op::Read(Read::Items { .. }) = op {
            Answer::Items { items: Box::new([]), more: false, now: env().now }
        } else {
            let Op::Read(Read::Item { number: read_number, .. }) = op else { panic!("{op:?}") };
            assert_eq!(read_number, number);
            Answer::Item {
                item: Summary {
                    labels: Box::new([]),
                    number,
                    kind: Kind::Pull,
                    state: State::Open,
                    author: 7,
                    key: None,
                    title: Box::new([]),
                    body: Box::new([]),
                    updated: env().now,
                },
                comments: Box::new([]),
                more: false,
            }
        };
        for request in fixtures::answer(&mut d, call, Ok(answer)) {
            seen.push(request).expect("four bounded output passes");
        }
    }
    panic!("the named pull must be read");
}

#[test]
fn a_named_pulls_ci_changed_wakes_only_its_own_change() {
    let seen = named_pull_verdict(1);
    let mut wakes = 0_u32;
    for request in &seen {
        if let Request::WakeProcedure { task } = request {
            assert_eq!(*task, fixtures::CHANGE, "CI Changed: {seen:?}");
            wakes += 1;
        }
    }
    assert_eq!(wakes, 1, "one owning change wakes: {seen:?}");
}

#[test]
fn a_named_pulls_changed_without_an_owning_change_wakes_nothing() {
    let seen = named_pull_verdict(99);
    for request in &seen {
        if let Request::WakeProcedure { .. } = request {
            panic!("unowned Changed: {seen:?}");
        }
    }
}

#[test]
fn a_settling_change_relies_on_its_own_pull_watch_but_keeps_its_deadline() {
    use skein_forge::api::{Answer, Ci, Op, Read};
    use temper_engine_domain_forge_change as change;
    for (watch, compared, delay) in [
        (Some(1), true, LIMITS.client.settle),
        (Some(1), false, LIMITS.client.hinted),
        (Some(2), true, LIMITS.client.hinted),
        (None, true, LIMITS.client.hinted),
    ] {
        let mut d = first_change(Protection::Absent, Ci::Passed, false);
        fixtures::answer_next(&mut d, Ok(fixtures::checks(skein_forge::api::Check::Passed)));
        fixtures::answer_next(&mut d, Ok(fixtures::checks(skein_forge::api::Check::Passed)));
        fixtures::answer_next(
            &mut d,
            if compared {
                Ok(Answer::Compare {
                    before: fixtures::BASE,
                    after: fixtures::HEAD,
                    contains_before: true,
                    files: Box::new([]),
                    commits: Box::new([]),
                })
            } else {
                Err(skein_forge::api::Error::Missing)
            },
        );
        let (reviews, op) = fixtures::next_call(&mut d);
        assert_eq!(op, Op::Read(Read::Reviews { number: 1, page: 1 }));
        if let Some(number) = watch {
            outputs(
                &mut d,
                Event::Names {
                    task: fixtures::CHANGE,
                    resources: Box::new([Name {
                        forge: REPO.forge,
                        repository: REPO.repository,
                        what: What::Pull(number),
                    }]),
                },
            );
        }
        let finished = fixtures::answer(&mut d, reviews, Ok(Answer::Reviews { reviews: Box::new([]), more: false }));
        let until = Wall::from_nanos(env().wall.as_nanos() + delay.as_nanos());
        assert!(
            finished.contains(&Request::ChangeDecision {
                task: fixtures::CHANGE,
                decision: change::Decision::Wait { until: Some(until) },
                entry: None,
                evidence: None,
            }),
            "watch {watch:?}, compared {compared}: the pull wake replaces complete gathers, with a deadline: {finished:?}"
        );
    }
}

// Exercise admission, retained words, and the actual operation handed to the client.
fn projected_words(words: &[u8]) -> skein_forge::api::Write {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let mut row = fixtures::opening_row();
    row.pending = None;
    row.effect = temper_engine_domain_forge_change::EffectResult::None;
    row.change.state = temper_engine_domain_forge_change::State::Opening { requested: false };
    row.title = words.into();
    row.body = words.into();
    let registered = outputs(&mut d, Event::Change { row });
    for request in &registered {
        if let Request::Refused { .. } = request {
            panic!("change registration refused: {request:?}");
        }
    }
    assert_eq!(d.change(fixtures::CHANGE).unwrap().title.as_ref(), words);
    assert_eq!(d.change(fixtures::CHANGE).unwrap().body.as_ref(), words);
    start_opening(&mut d, fixtures::CHANGE, FIRST_ENTRY);
    fixtures::missing_pull_reads(&mut d);
    let built = fixtures::finish_missing(&mut d);
    assert!(built_entry(&built, FIRST_ENTRY));
    outputs(&mut d, Event::Committed { entry: FIRST_ENTRY });
    answer_entry_clock(&mut d);
    let (_, op) = fixtures::next_call(&mut d);
    let skein_forge::api::Op::Write(write) = op else { panic!("open write") };
    assert!(skein_forge::measure::write(&write).unwrap() <= u64::from(CLIENT.op_bytes));
    write
}

#[test]
fn change_words_past_operation_bound_register_whole_and_open_with_marked_cuts() {
    let words = "界".repeat(700);
    let skein_forge::api::Write::OpenPull { title, body, .. } = projected_words(words.as_bytes()) else {
        panic!("open pull")
    };
    assert!(title.len() <= 256);
    let title_prefix = title.strip_suffix("…".as_bytes()).expect("marked title");
    assert_eq!(title_prefix, "界".repeat(title_prefix.len() / 3).as_bytes());
    let body_prefix =
        body.strip_suffix("…\nThe change's full words are on its task in temper.".as_bytes()).expect("marked body");
    assert_eq!(body_prefix, "界".repeat(body_prefix.len() / 3).as_bytes());
}

#[test]
fn change_words_within_operation_bound_open_unchanged_with_first_line_title() {
    let words = b"Small fix\nKeep the full specification.";
    let skein_forge::api::Write::OpenPull { title, body, .. } = projected_words(words) else { panic!("open pull") };
    assert_eq!(title.as_ref(), b"Small fix");
    assert_eq!(body.as_ref(), words);
}
