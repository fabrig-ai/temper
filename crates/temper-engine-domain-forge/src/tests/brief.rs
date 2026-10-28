use super::*;
use skein_forge::api::{Answer, Check, JobAttempt, Read, Status, Step};

// temper-service's shipped root.brief.budgets.ci; read_bytes is 4608 and
// forge.client.answer_bytes exceeds the five-check log share of 819 bytes.
const SHIPPED_CI: u32 = 4096;
const SECTION: Token = Token::new(99);
const STEP: &[u8] = b"Each pull request adds changes/<its number>.md";
#[expect(clippy::disallowed_macros, reason = "the regression embeds recorded runner fixture bytes")]
const RUNNER_LOG: &[u8] = include_bytes!("fixtures/ci/red-changes.log");
const HEAD: [u8; 32] = [
    0xc1, 0xfb, 0x35, 0x99, 0xfa, 0x12, 0xc9, 0x76, 0x6f, 0x9c, 0xc1, 0x62, 0x84, 0x44, 0x16, 0xe8, 0x7a, 0x92, 0x91,
    0x82, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// Contexts, descriptions, links and attempt copied from g1's red.json.
fn red_checks() -> Box<[Status]> {
    Box::new([
        Status {
            at: env().now,
            context: b"/ check (pull_request)".as_slice().into(),
            check: Check::Passed,
            description: b"Successful in 0s".as_slice().into(),
            url: b"/playground/widget/actions/runs/4/jobs/0".as_slice().into(),
            job: None,
        },
        Status {
            at: env().now,
            context: b"/ changes (pull_request)".as_slice().into(),
            check: Check::Failed,
            description: b"Failing after 1s".as_slice().into(),
            url: b"/playground/widget/actions/runs/4/jobs/1".as_slice().into(),
            job: Some(JobAttempt { head: HEAD, run: 4, job: 6, attempt: 1 }),
        },
    ])
}

fn gather(checks: Box<[Status]>, log: &[u8], budget: u32) -> Domain {
    gather_for(checks, log, budget, false)
}

fn gather_for(checks: Box<[Status]>, log: &[u8], budget: u32, landing: bool) -> Domain {
    let job_bound = (budget / 5)
        .min(
            budget
                .checked_div(u32::try_from(checks.len()).expect("bounded checks").max(1))
                .expect("positive check count")
                / 2,
        )
        .max(1);
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let mut row = fixtures::opening_row();
    row.pull = Some(1);
    row.change.last_head = Some(HEAD);
    row.change.state = temper_engine_domain_forge_change::State::Repairing {
        requested: true,
        why: temper_engine_domain_forge_change::Repair::Ci,
    };
    row.base_repair = landing;
    if landing {
        row.pull = None;
        row.change.last_head = None;
        row.change.state = temper_engine_domain_forge_change::State::Producing { requested: true };
    }
    outputs(&mut d, Event::Change { row });
    outputs(
        &mut d,
        Event::PlanBrief {
            section: SECTION,
            source: if landing {
                BriefSource::BranchCi { task: fixtures::CHANGE, head: BriefCommit(HEAD) }
            } else {
                BriefSource::Ci { item: BriefItem { repository: REPO.repository, number: 1 }, head: BriefCommit(HEAD) }
            },
        },
    );
    let started =
        outputs(&mut d, Event::GatherPlanned { section: SECTION, parts: 2, bytes: budget, ci_budget: budget });
    let [Request::BriefClient { event: skein_forge_client::Event::Read { owner, read, .. } }] = started.as_ref() else {
        panic!("one checks read: {started:?}")
    };
    assert_eq!(*read, Read::Checks { commit: HEAD });
    let owner = *owner;
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    assert!(brief::brief_answer(&mut d, owner, Ok(Answer::Checks(checks)), &mut out));
    for _ in 0..LIMITS.client.rows.saturating_add(1) {
        let Some(request) = out.pop() else { break };
        if let Request::BriefClient {
            event: skein_forge_client::Event::Read { read: Read::Job { attempt, max_bytes }, .. },
        } = request
        {
            assert_eq!(max_bytes, job_bound, "the shipped five-check allowance reaches the provider");
            let tail = log.len().saturating_sub(usize::try_from(max_bytes).expect("u32"));
            assert!(brief::brief_answer(
                &mut d,
                owner,
                Ok(Answer::Job {
                    attempt,
                    step: Some(Step { number: 2, name: STEP.into() }),
                    log: log[tail..].into(),
                    truncated: tail > 0,
                }),
                &mut out
            ));
        } else if let Request::BriefSized { section: SECTION, size: Some(size) } = request {
            assert!(size <= budget);
        } else {
            panic!("unexpected gather output: {request:?}");
        }
    }
    d
}

fn take(d: &mut Domain) -> Box<[u8]> {
    d.take_brief(SECTION).expect("retained CI section")
}

#[test]
fn a_runners_failure_reaches_the_repair_at_shipped_limits() {
    assert_eq!(RUNNER_LOG.len(), 553, "the runner's recorded bytes");
    let bytes = take(&mut gather(red_checks(), RUNNER_LOG, SHIPPED_CI));
    let words = text(&bytes);
    for expected in [
        "/ changes (pull_request)",
        "Description: Failing after 1s",
        "Link: /playground/widget/actions/runs/4/jobs/1",
        "Failed step: 2 Each pull request adds changes/<its number>.md",
        "Missing changes/1.md",
    ] {
        assert!(words.contains(expected), "{expected}: {words}");
    }
    assert!(!words.contains("cut]"));
    assert!(!words.contains("truncated"));
}

#[test]
fn a_long_log_is_cut_from_its_front_keeping_its_check() {
    let mut log = skein_lib::List::with_capacity(20_018);
    for _ in 0_u32..20_000 {
        log.push(b'x').expect("log room");
    }
    for byte in b"THE LAST LOG BYTES" {
        log.push(*byte).expect("log tail room");
    }
    let mut d = gather(red_checks(), log.as_slice(), SHIPPED_CI);
    let cut = outputs(&mut d, Event::CutBrief { section: SECTION, bytes: 512 });
    let [Request::BriefSized { size: Some(size), .. }] = cut.as_ref() else { panic!("successful cut: {cut:?}") };
    assert!(*size <= 512);
    // Cutting again must use the same structured check, not a previous rendering.
    let cut = outputs(&mut d, Event::CutBrief { section: SECTION, bytes: 480 });
    let [Request::BriefSized { size: Some(size), .. }] = cut.as_ref() else { panic!("successful cut: {cut:?}") };
    assert!(*size <= 480);
    let bytes = take(&mut d);
    let words = text(&bytes);
    assert!(words.contains("Failed check: / changes (pull_request)\nDescription: Failing after 1s\nLink: /playground/widget/actions/runs/4/jobs/1\nFailed step: 2 Each pull request adds changes/<its number>.md\nEnd of its log:\n[log truncated before this]\n"), "{words}");
    assert!(words.contains("THE LAST LOG BYTES\nRepair the failed check.\n"), "{words}");
    assert_eq!(words.matches("[log truncated before this]\n").count(), 1);
    assert!(!words.contains("omitted"));
}

#[test]
fn checks_past_the_budget_are_left_out_whole_and_counted() {
    let mut checks = skein_lib::List::with_capacity(8);
    for name in [b"check-0", b"check-1", b"check-2", b"check-3", b"check-4", b"check-5", b"check-6", b"check-7"] {
        checks
            .push(Status {
                at: env().now,
                context: name.as_slice().into(),
                check: Check::Failed,
                description: b"failed".as_slice().into(),
                url: b"/job".as_slice().into(),
                job: None,
            })
            .expect("eight checks");
    }
    let expected = b"Repair the failed check at change head c1fb3599fa12c976. Push a new head and wait for checks there.\nFailed check: check-0\nDescription: failed\nLink: /job\n[no pinned job log for this head]\nFailed check: check-1\nDescription: failed\nLink: /job\n[no pinned job log for this head]\n[6 failed checks omitted]\nRepair the failed check.\n";
    let budget = u32::try_from(expected.len()).expect("small budget");
    let checks = checks.into_boxed();
    let mut d = gather(checks.clone(), b"", SHIPPED_CI);
    outputs(&mut d, Event::CutBrief { section: SECTION, bytes: budget });
    assert_eq!(take(&mut d).as_ref(), expected);
    let mut short = gather(checks.clone(), b"", SHIPPED_CI);
    outputs(&mut short, Event::CutBrief { section: SECTION, bytes: budget.saturating_sub(1) });
    let bytes = take(&mut short);
    let words = text(&bytes);
    assert!(words.contains("Failed check: check-0\n"));
    assert!(!words.contains("Failed check: check-1\n"));
    assert!(words.contains("[7 failed checks omitted]\n"));
    let mut tiny = gather(checks.clone(), b"", SHIPPED_CI);
    let notice = b"[8 failed checks omitted]\n";
    outputs(&mut tiny, Event::CutBrief { section: SECTION, bytes: u32::try_from(notice.len()).expect("notice size") });
    assert_eq!(take(&mut tiny).as_ref(), notice);
    // The gather's initial allocation follows the same rule as a parent's cut.
    assert_eq!(take(&mut gather(checks, b"", budget)).as_ref(), expected);
}

#[test]
fn a_log_cut_keeps_utf8_and_marks_the_missing_prefix() {
    let mut ci = brief_ci::Section::new(BriefCommit(HEAD), 1, SHIPPED_CI);
    ci.push(
        b"Failed check: unicode\nEnd of its log:\n".as_slice().into(),
        "αβγδεζηθικλμνξοπρστυφχψω".as_bytes().into(),
        false,
    );
    // Both sides of every UTF-8 boundary, including budgets too small for a head.
    for budget in 0..256 {
        if let Some(bytes) = ci.render(budget) {
            assert!(bytes.len() <= usize::try_from(budget).expect("small budget"));
            let words = text(&bytes);
            if let Some((_, log)) = words.split_once("End of its log:\n") {
                let log = log.strip_suffix("\nRepair the failed check.\n").expect("log ending");
                if let Some(tail) = log.strip_prefix("[log truncated before this]\n") {
                    assert!("αβγδεζηθικλμνξοπρστυφχψω".ends_with(tail));
                } else {
                    assert_eq!(log, "αβγδεζηθικλμνξοπρστυφχψω");
                }
            }
        }
    }
}

#[test]
fn large_check_heads_take_room_from_logs_before_a_check_is_omitted() {
    let mut checks = skein_lib::List::with_capacity(3);
    for context in [b"one", b"two", b"tri"] {
        checks
            .push(Status {
                at: env().now,
                context: context.as_slice().into(),
                check: Check::Failed,
                description: Box::new([b'd'; 130]),
                url: b"/job".as_slice().into(),
                job: Some(JobAttempt { head: HEAD, run: 4, job: 6, attempt: 1 }),
            })
            .expect("three checks");
    }
    let bytes = take(&mut gather(checks.into_boxed(), &[b'x'; 8192], 1024));
    let words = text(&bytes);
    assert_eq!(words.matches("Failed check:").count(), 3);
    assert_eq!(words.matches("Failed step: 2 Each pull request adds changes/<its number>.md\n").count(), 3);
    assert!(!words.contains("omitted"), "all whole heads fit: {words}");
    assert!(words.contains("[log truncated before this]\n"), "only the log tails are reduced: {words}");
}

#[expect(clippy::disallowed_methods, reason = "the test reader validates the rendered section's UTF-8")]
fn text(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).expect("UTF-8 CI")
}

#[test]
fn a_repairs_pull_section_names_its_pull_request_before_its_contents() {
    let mut config = configured()[0].clone();
    config.git = b"https://forge.example".as_slice().into();
    let mut d = setup_domain(&[config], false);
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let mut row = fixtures::opening_row();
    row.pull = Some(1);
    row.change.last_head = Some(HEAD);
    row.change.state = temper_engine_domain_forge_change::State::Repairing {
        requested: true,
        why: temper_engine_domain_forge_change::Repair::Ci,
    };
    outputs(&mut d, Event::Change { row });
    let source =
        BriefSource::Pull { item: BriefItem { repository: REPO.repository, number: 1 }, head: BriefCommit(HEAD) };
    outputs(&mut d, Event::PlanBrief { section: SECTION, source });
    let started = outputs(&mut d, Event::GatherPlanned { section: SECTION, parts: 2, bytes: 4096, ci_budget: 4096 });
    let [Request::BriefClient { event: skein_forge_client::Event::Read { owner, .. } }] = started.as_ref() else {
        panic!("pull read: {started:?}")
    };
    let owner = *owner;
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    let item = skein_forge::api::Summary {
        labels: Box::new([]),
        kind: skein_forge::api::Kind::Pull,
        author: 7,
        key: None,
        updated: env().now,
        number: 1,
        title: b"Small fix".as_slice().into(),
        body: b"Whole body".as_slice().into(),
        state: skein_forge::api::State::Open,
    };
    assert!(brief::brief_answer(
        &mut d,
        owner,
        Ok(Answer::Item { item, comments: Box::new([]), more: false }),
        &mut out
    ));
    let mut pull = fixtures::open_pull();
    pull.commit = HEAD;
    pull.base_commit = None;
    assert!(brief::brief_answer(&mut d, owner, Ok(Answer::Pull(pull)), &mut out));
    assert!(brief::brief_answer(
        &mut d,
        owner,
        Ok(Answer::PullFiles { head: HEAD, files: Box::new([]), more: false }),
        &mut out
    ));
    let bytes = take(&mut d);
    let words = text(&bytes);
    let expected = "Pull request: #1\nBranch: temper/51\nBase: main\nHead: c1fb3599fa12c9766f9cc162844416e87a929182\nAddress: https://forge.example/org/repo/pulls/1\n";
    assert!(words.starts_with(expected), "{words}");
    assert!(words.contains("Whole body"), "{words}");
}

fn remarks_section(remarks: &[u8], budget: u32) -> Box<[u8]> {
    // The shipped Reviews read is 4,608 bytes; keep the same room in this
    // child's tiny fixture, with upstream remarks admitted up to 8 KiB.
    let limits = Limits {
        brief_bytes: 4608,
        client: skein_forge_client::Limits { answer_bytes: 8192, ..LIMITS.client },
        ..LIMITS
    };
    let env = Env { limits, ..env() };
    let mut d = domain_with_limits(&limits);
    outputs_with_env(&mut d, Event::Restore { record: Stored::Repository(adopted()) }, &env);
    let mut row = fixtures::opening_row();
    row.pull = Some(1);
    row.change.last_head = Some(HEAD);
    row.change.state = temper_engine_domain_forge_change::State::Repairing {
        requested: true,
        why: temper_engine_domain_forge_change::Repair::Gate(1),
    };
    row.gate_remarks = Box::new([GateRemark { number: 1, head: HEAD, words: remarks.into() }]);
    outputs_with_env(&mut d, Event::Change { row }, &env);
    let source =
        BriefSource::Reviews { item: BriefItem { repository: REPO.repository, number: 1 }, head: BriefCommit(HEAD) };
    outputs_with_env(&mut d, Event::PlanBrief { section: SECTION, source }, &env);
    let started = outputs_with_env(
        &mut d,
        Event::GatherPlanned { section: SECTION, parts: 2, bytes: budget, ci_budget: 4096 },
        &env,
    );
    let [Request::BriefClient { event: skein_forge_client::Event::Read { owner, .. } }] = started.as_ref() else {
        panic!("review read: {started:?}")
    };
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    assert!(brief::brief_answer(&mut d, *owner, Ok(Answer::Reviews { reviews: Box::new([]), more: false }), &mut out));
    take(&mut d)
}

#[test]
fn remarks_within_the_budget_reach_the_repair_whole() {
    assert_eq!(
        remarks_section(b"Please cite this pull request's number and address.", 128).as_ref(),
        b"Gate remarks: Please cite this pull request's number and address.\n"
    );
}

#[test]
fn remarks_over_the_budget_keep_their_prefix_and_mark_every_cut_byte() {
    let remarks = "Please cite this pull request's number and address. ".repeat(10);
    let whole_len = b"Gate remarks: ".len() + remarks.len() + 1;
    let bytes = remarks_section(remarks.as_bytes(), 128);
    let words = text(&bytes);
    let (kept, marker) = words.rsplit_once("\n[").expect("marked cut");
    assert!(remarks.starts_with(kept.strip_prefix("Gate remarks: ").expect("remarks heading")));
    let count = skein_lib::Decimal::of(u64::try_from(whole_len - kept.len()).unwrap());
    assert_eq!(marker.strip_suffix(" bytes cut]\n").expect("cut marker").as_bytes(), count.as_bytes());
    assert!(words.len() <= 128);
}

#[test]
fn a_landing_tip_repair_reads_the_same_failed_step_and_logs_end() {
    let bytes = take(&mut gather_for(red_checks(), RUNNER_LOG, SHIPPED_CI, true));
    let words = text(&bytes);
    assert!(words.contains("Failed check: / changes (pull_request)"), "{words}");
    assert!(words.contains("Failed step: 2 Each pull request adds changes/<its number>.md"), "{words}");
    assert!(words.contains("Missing changes/1.md"), "{words}");
    assert!(words.contains("c1fb3599fa12c976"), "failed tip: {words}");
}

#[test]
fn four_thousand_bytes_of_remarks_reach_the_repair_whole() {
    let remarks = [b'r'; 4000];
    let bytes = remarks_section(&remarks, 4608);
    assert_eq!(&bytes[..b"Gate remarks: ".len()], b"Gate remarks: ");
    assert_eq!(&bytes[b"Gate remarks: ".len()..bytes.len() - 1], &remarks);
    assert_eq!(bytes.last(), Some(&b'\n'));
}

#[test]
fn five_thousand_bytes_of_remarks_are_cut_from_the_end_and_marked() {
    let remarks = [b'r'; 5000];
    let bytes = remarks_section(&remarks, 4608);
    let words = text(&bytes);
    let (kept, marker) = words.rsplit_once("\n[").expect("marked end cut");
    let kept_remarks = kept.strip_prefix("Gate remarks: ").expect("remarks heading");
    assert!(remarks.starts_with(kept_remarks.as_bytes()));
    let whole_len = b"Gate remarks: ".len() + remarks.len() + 1;
    let cut = skein_lib::Decimal::of(u64::try_from(whole_len - kept.len()).unwrap());
    assert_eq!(marker.strip_suffix(" bytes cut]\n").expect("cut suffix").as_bytes(), cut.as_bytes());
    assert!(bytes.len() <= 4608);
    assert!(kept_remarks.len() > 4000, "the new working room is used");
}
