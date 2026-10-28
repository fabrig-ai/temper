use super::*;
use fixtures::{BASE, BROWSER, CHANGE, HEAD, MERGE, OPEN};
use temper_engine_domain_forge_tiers as policy;
const TWO_TIERS: usize = 2; // browser and fuzzy in the fixture.

#[test]
fn a_tiered_main_is_watched_with_no_task_naming_it() {
    let d = fixtures::tiered();
    assert_eq!(crate::tiers::watched(&d).as_slice(), &[fixtures::main_name()]);
}
#[test]
fn the_first_tip_seen_starts_each_tiers_baseline() {
    let mut d = fixtures::tiered();
    let out = fixtures::tier_baseline(&mut d);
    assert_eq!(tier_runs(&out).as_slice(), &[(1, BASE); TWO_TIERS]);
    assert!(fixtures::browser(&d).record.run.as_ref().expect("baseline").landings.is_empty());
}
#[test]
fn a_merge_moving_the_main_starts_the_next_run_over_its_landing() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    let out = fixtures::tier_move(&mut d, MERGE, Some(&[MERGE]));
    assert_eq!(tier_runs(&out).as_slice(), &[(2, MERGE); TWO_TIERS]);
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("next run").landings.as_ref(),
        &[policy::Landing { commit: MERGE, key: fixtures::BROWSER_MERGE_KEY }]
    );
}
#[test]
fn a_move_during_a_run_waits_for_the_next_range() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    outputs(&mut d, Event::TierTask { repository: REPO, tier: fixtures::FUZZY.into(), number: 1, task: OPEN });
    let out = fixtures::tier_move(&mut d, HEAD, None);
    assert!(tier_runs(&out).is_empty());
    assert_eq!(
        fixtures::browser(&d).pending.as_ref(),
        &[TierLanding { commit: HEAD, key: Some(fixtures::BROWSER_HEAD_KEY) }]
    );
}
#[test]
fn a_push_temper_did_not_make_is_a_landing_of_its_own() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    fixtures::tier_move(&mut d, HEAD, Some(&[MERGE, HEAD]));
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[policy::Landing { commit: HEAD, key: fixtures::BROWSER_HEAD_KEY }]
    );
}
#[test]
fn a_merge_and_a_push_in_one_move_are_two_landings_oldest_first() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    fixtures::tier_move(&mut d, HEAD, Some(&[MERGE, HEAD]));
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[
            policy::Landing { commit: MERGE, key: fixtures::BROWSER_MERGE_KEY },
            policy::Landing { commit: HEAD, key: fixtures::BROWSER_HEAD_KEY }
        ]
    );
}
#[test]
fn a_move_seen_without_its_commits_is_one_landing_at_the_tip() {
    let mut d = fixtures::tiered();
    fixtures::tier_judged(&mut d);
    fixtures::tier_move(&mut d, HEAD, None);
    assert_eq!(
        fixtures::browser(&d).record.run.as_ref().expect("run").landings.as_ref(),
        &[policy::Landing { commit: HEAD, key: fixtures::BROWSER_HEAD_KEY }]
    );
}
#[test]
fn the_tier_task_first_creates_its_branch_at_the_tip() {
    // Tag 8, repository 1/2 (eight bytes each), tier index 0, run 1, branch operation 1, part 0.
    const PURPOSE_BYTES: [u8; 30] =
        [8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0];
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    let out = outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    let entry = d.entries.get(&OPEN).expect("branch outbox");
    assert_eq!(
        entry.entry.effect.write,
        api::Write::CreateBranch { branch: b"temper/tier/browser.1".as_slice().into(), commit: BASE }
    );
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Pending { entry: OPEN });
    let mut identity = None;
    for request in &out {
        if let Request::TierEffect { purpose, .. } = request {
            identity = purpose.identity();
        }
    }
    assert_eq!(identity.as_deref(), Some(PURPOSE_BYTES.as_slice()));
}
#[test]
fn a_tier_row_is_restored_at_restart() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    let row = fixtures::browser(&d).clone();
    let mut restored = fixtures::tiered();
    outputs(&mut restored, Event::Restore { record: Stored::Tier(row.clone()) });
    assert_eq!(fixtures::browser(&restored), &row);
}
#[test]
fn a_veto_clears_the_uncommitted_branch_entry() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: CHANGE });
    outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    outputs(&mut d, Event::VetoTier { task: CHANGE, entry: OPEN });
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Absent);
    assert!(!d.entries.contains_key(&OPEN));
}

const SMALL_RANGE: usize = 3; // 2 * changes (one live change) + 1.
#[test]
fn a_full_range_merges_its_two_oldest_outside_landings() {
    let (mut d, env) = fixtures::full_tier_range(SMALL_RANGE);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::moved(&mut d, &env, REPO, BASE, HEAD, None, &mut out);
    assert_eq!(
        fixtures::browser(&d).pending.as_ref(),
        &[
            TierLanding { commit: [11; 32], key: Some([11; 32]) },
            TierLanding { commit: MERGE, key: Some(MERGE) },
            TierLanding { commit: HEAD, key: None }
        ]
    );
}
#[test]
fn a_range_one_short_of_full_merges_nothing() {
    let (mut d, env) = fixtures::full_tier_range(SMALL_RANGE - 1);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::moved(&mut d, &env, REPO, BASE, HEAD, None, &mut out);
    assert_eq!(
        fixtures::browser(&d).pending.as_ref(),
        &[
            TierLanding { commit: [10; 32], key: Some([10; 32]) },
            TierLanding { commit: [11; 32], key: Some([11; 32]) },
            TierLanding { commit: HEAD, key: None }
        ]
    );
}
#[test]
fn project_tiers_accepts_each_new_members_bound() {
    let mut d = domain();
    assert!(d.project_tiers(
        fixtures::PROJECT,
        fixtures::tier_settings(),
        fixtures::configured_tier(
            WORKFLOW_BYTES,
            usize::try_from(LIMITS.ignored_paths).expect("fixture bound fits usize"),
            usize::try_from(LIMITS.name_bytes).expect("fixture bound fits usize"),
            CHECK_NAME_BYTES
        )
    ));
}
#[test]
fn project_tiers_refuses_each_member_one_past_its_bound() {
    for (workflow, paths, path_bytes, artifact) in [
        (WORKFLOW_BYTES + 1, 0, 1, 1),
        (1, usize::try_from(LIMITS.ignored_paths).expect("fixture bound fits usize") + 1, 1, 1),
        (1, 1, usize::try_from(LIMITS.name_bytes).expect("fixture bound fits usize") + 1, 1),
        (1, 0, 1, CHECK_NAME_BYTES + 1),
    ] {
        let mut d = domain();
        assert!(!d.project_tiers(
            fixtures::PROJECT,
            fixtures::tier_settings(),
            fixtures::configured_tier(workflow, paths, path_bytes, artifact)
        ));
    }
}
#[test]
fn a_made_branch_is_not_emitted_again_after_restore() {
    let mut d = fixtures::tiered();
    fixtures::tier_baseline(&mut d);
    let mut row = fixtures::browser(&d).clone();
    row.task = Some(CHANGE);
    row.branch = TierBranch::Made;
    outputs(&mut d, Event::Restore { record: Stored::Tier(row) });
    let out = outputs(&mut d, Event::StepTier { task: CHANGE, entry: OPEN });
    let mut created = false;
    for request in &out {
        if let Request::TierEffect { entry, .. } = request
            && let api::Write::CreateBranch { .. } = entry.effect.write
        {
            created = true;
        }
    }
    assert!(!created);
}

#[test]
fn project_tiers_refuses_invalid_workflows_artifacts_and_ignored_paths() {
    let rows: &[(&[u8], &[u8], &[u8])] = &[
        (b"", b"report", b"docs"),
        (b"a/b", b"report", b"docs"),
        (b"tier.yml", b"", b"docs"),
        (b"tier.yml", b"a/b", b"docs"),
        (b"tier.yml", b"report", b""),
        (b"tier.yml", b"report", b"/"),
        (b"tier.yml", b"report", b"/docs"),
        (b"tier.yml", b"report", b"docs/"),
        (b"tier.yml", b"report", b"a//b"),
        (b"tier.yml", b"report", b"."),
        (b"tier.yml", b"report", b".."),
        (b"tier.yml", b"report", b"a/../b"),
    ];
    for (workflow, artifact, path) in rows {
        let mut d = domain();
        let mut configured = fixtures::repository_tiers(&[BROWSER]);
        configured[0].tiers[0].workflow = Box::from(*workflow);
        configured[0].tiers[0].artifact = Box::from(*artifact);
        configured[0].tiers[0].ignored = Box::new([Box::from(*path)]);
        assert!(!d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), configured));
    }
}

fn tier_runs(out: &Queue<Request>) -> skein_lib::List<(u64, [u8; 32])> {
    let mut runs = skein_lib::List::with_capacity(out.len());
    for request in out {
        if let Request::TierRun { number, tip, .. } = request {
            runs.push((*number, *tip)).expect("output bounds tier requests");
        }
    }
    runs
}

use skein_forge::api;
const TIER_TASK: u64 = 61; // distinct from the two range members, CHANGE and OPEN.
const DISPATCH: u64 = 21;
const STATUS: u64 = 22;
const RETRY: u64 = 23;
const RUN_ID: u64 = 71;
const READ_OWNER: Token = Token::new(101);

fn made_branch() -> Domain {
    let mut d = fixtures::tiered();
    d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), fixtures::repository_tiers(&[BROWSER]));
    fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: TIER_TASK });
    d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap().branch = TierBranch::Made;
    d
}
fn dispatch(d: &mut Domain, entry: u64) -> Box<[Request]> {
    let out = outputs(d, Event::StepTier { task: TIER_TASK, entry });
    if let Some(saved) = d.entries.get_mut(&entry) {
        saved.entry.start = Some(skein_forge_client::Position { at: env().now, comment: 0, review: 0, review_page: 1 });
        let made = match &saved.entry.effect.write {
            api::Write::CreateBranch { commit, .. } => skein_forge_client::Made::Branch(*commit),
            api::Write::CreateIssue { .. }
            | api::Write::OpenPull { .. }
            | api::Write::Post { .. }
            | api::Write::Review { .. }
            | api::Write::Edit { .. }
            | api::Write::SetReviewers { .. }
            | api::Write::Close { .. }
            | api::Write::Reopen { .. }
            | api::Write::Merge { .. }
            | api::Write::Update { .. }
            | api::Write::Status { .. }
            | api::Write::Dispatch { .. }
            | api::Write::Rerun { .. }
            | api::Write::DeleteBranch { .. } => skein_forge_client::Made::Set,
        };
        let mut ignored = Queue::with_capacity(max_out(&LIMITS));
        crate::tiers::outcome(d, entry, skein_forge_client::Outcome::Made { made, found: false }, &mut ignored);
        d.entries.remove(&entry);
    }
    out
}
fn dispatched() -> Domain {
    let mut d = made_branch();
    dispatch(&mut d, DISPATCH);
    d
}
fn workflow(d: &Domain, conclusion: api::Conclusion, step: Option<u32>) -> api::Run {
    api::Run {
        id: RUN_ID,
        number: 1,
        workflow: b"tier.yml".as_slice().into(),
        commit: match d.tier_follows.get(&(REPO, BROWSER.into())) {
            Some(follow) => follow.commit,
            None => fixtures::browser(d).record.run.as_ref().unwrap().tip,
        },
        event: api::Event::Dispatch,
        actor: 7,
        state: api::RunState::Completed,
        conclusion: Some(conclusion),
        url: b"https://forge/run/71".as_slice().into(),
        created: env().now,
        jobs: match step {
            Some(number) => Box::new([api::RunJob {
                id: 81,
                name: b"test".as_slice().into(),
                attempt: 1,
                state: api::RunState::Completed,
                conclusion: Some(api::Conclusion::Failure),
                step: Some(api::Step { number, name: b"step".as_slice().into() }),
                steps: 3,
            }]),
            None => Box::new([]),
        },
    }
}
fn answer(d: &mut Domain, answer: api::Answer) -> Queue<Request> {
    d.tier_reads.insert(READ_OWNER, (REPO, BROWSER.into())).unwrap();
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::answered(d, &env(), READ_OWNER, Ok(answer), &mut out);
    out
}
fn completed(d: &mut Domain, conclusion: api::Conclusion, step: Option<u32>) {
    let run = workflow(d, conclusion, step);
    answer(d, api::Answer::Runs { runs: Box::new([run.clone()]), more: false });
    answer(d, api::Answer::Run(run));
    // Existing completion stories supply no artifact; its absence is still a fault.
    if conclusion == api::Conclusion::Failure && step == Some(1) {
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        d.tier_reads.insert(READ_OWNER, (REPO, BROWSER.into())).unwrap();
        crate::tiers::answered(d, &env(), READ_OWNER, Err(api::Error::Missing), &mut out);
    }
}
fn judged(d: &mut Domain) -> Box<[Request]> {
    dispatch(d, STATUS);
    let out = outputs(d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    if fixtures::browser(d).branch == (TierBranch::Pending { entry: RETRY }) {
        let mut ignored = Queue::with_capacity(max_out(&LIMITS));
        crate::tiers::outcome(
            d,
            RETRY,
            skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Set, found: false },
            &mut ignored,
        );
        d.entries.remove(&RETRY);
        return outputs(d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    }
    out
}
fn lost(d: &mut Domain) {
    d.tier_follows.get_mut(&(REPO, BROWSER.into())).unwrap().since = Some(Time::ZERO);
    answer(d, api::Answer::Runs { runs: Box::new([]), more: false });
}
fn effect_write(out: &[Request]) -> &api::Write {
    for request in out {
        if let Request::TierEffect { entry, .. } = request {
            return &entry.effect.write;
        }
        if let Request::Save { record: Stored::Entry { entry, .. }, release: Some(_), .. } = request {
            return &entry.effect.write;
        }
    }
    unreachable!("the step made an effect")
}
fn tier_decision(out: &[Request]) -> Option<&TierDecision> {
    for request in out {
        if let Request::TierDecision { decision, .. } = request {
            return Some(decision);
        }
    }
    None
}

#[test]
fn a_made_branch_dispatches_the_tiers_workflow_with_its_inputs() {
    const HEX_TIP: &[u8] = b"0404040404040404040404040404040404040404040404040404040404040404";
    let mut d = made_branch();
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: DISPATCH });
    assert_eq!(
        effect_write(&out),
        &api::Write::Dispatch {
            workflow: b"tier.yml".as_slice().into(),
            branch: b"temper/tier/browser.1".as_slice().into(),
            inputs: Box::new([
                api::Input { name: b"temper_commit".as_slice().into(), value: HEX_TIP.into() },
                api::Input {
                    name: b"temper_key".as_slice().into(),
                    value: b"e1a906fb92a9b1f50e98917117b49e3272d8365bb3eaaf4b2c7d3aff7ba58d95".as_slice().into()
                }
            ]),
        }
    );
}
#[test]
fn a_passed_run_judges_green_clears_its_range_and_wakes_its_changes() {
    let mut d = made_branch();
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    crate::verification::landed(&mut d, REPO, HEAD, OPEN, &mut out);
    let row = d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap();
    let run = row.record.run.as_mut().unwrap();
    run.tip = HEAD;
    run.landings = Box::new([
        policy::Landing { commit: MERGE, key: fixtures::BROWSER_MERGE_KEY },
        policy::Landing { commit: HEAD, key: fixtures::BROWSER_HEAD_KEY },
    ]);
    dispatch(&mut d, DISPATCH);
    completed(&mut d, api::Conclusion::Success, None);
    judged(&mut d);
    assert_eq!(
        crate::verification::verification(&d, REPO, MERGE),
        temper_engine_domain_forge_change::Verification::Cleared
    );
    assert_eq!(
        crate::verification::verification(&d, REPO, HEAD),
        temper_engine_domain_forge_change::Verification::Cleared
    );
    assert!(d.procedure_wakes.contains_key(&CHANGE));
    assert!(d.procedure_wakes.contains_key(&OPEN));
}
#[test]
fn a_lost_run_is_retried_by_a_new_dispatch() {
    let mut d = dispatched();
    lost(&mut d);
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    let api::Write::Dispatch { .. } = effect_write(&out) else { panic!("new dispatch after loss") };
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().faults, 1);
}
#[test]
fn a_failed_job_of_the_set_up_is_rerun() {
    let mut d = dispatched();
    completed(&mut d, api::Conclusion::Failure, Some(0));
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    assert_eq!(
        effect_write(&out),
        &api::Write::Rerun { attempt: api::JobAttempt { head: BASE, run: RUN_ID, job: 81, attempt: 1 } }
    );
}
#[test]
fn a_third_fault_holds_the_task() {
    let mut d = dispatched();
    lost(&mut d);
    dispatch(&mut d, RETRY);
    lost(&mut d);
    dispatch(&mut d, DISPATCH);
    lost(&mut d);
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: STATUS });
    assert_eq!(out.as_ref(), &[Request::TierDecision { task: TIER_TASK, decision: TierDecision::Held }]);
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().faults, 3);
    assert!(d.landing.receipts.is_empty());
}
#[test]
fn a_failed_run_is_a_fault_until_its_report_is_read() {
    let mut d = dispatched();
    completed(&mut d, api::Conclusion::Failure, Some(1));
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().faults, 1);
    assert!(fixtures::browser(&d).result.is_none());
    assert!(d.landing.receipts.is_empty());
}
#[test]
fn a_whole_run_makes_a_receipt_and_a_rerun_none() {
    let mut d = dispatched();
    completed(&mut d, api::Conclusion::Success, None);
    judged(&mut d);
    let key = ReceiptKey {
        project: fixtures::PROJECT,
        repository: REPO,
        check: Check::Tier(BROWSER.into()),
        key: fixtures::BROWSER_BASE_KEY,
    };
    assert!(d.receipt(&key).unwrap().passed);
    assert_eq!(d.receipt(&key).unwrap().run.id, RUN_ID);
}
#[test]
fn a_passed_receipt_at_the_tip_judges_with_no_branch_or_dispatch() {
    let mut d = fixtures::tiered();
    d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), fixtures::repository_tiers(&[BROWSER]));
    let key = ReceiptKey {
        project: fixtures::PROJECT,
        repository: REPO,
        check: Check::Tier(BROWSER.into()),
        key: fixtures::BROWSER_BASE_KEY,
    };
    d.landing
        .receipts
        .insert(
            key,
            Receipt {
                passed: true,
                failures: Some(policy::Failures::Tests(Box::new([]))),
                candidate: BASE,
                ended: env().wall,
                run: ReceiptRun { id: RUN_ID, number: 1, url: b"https://forge/run/71".as_slice().into() },
            },
        )
        .unwrap();
    let start = fixtures::tier_baseline(&mut d);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: TIER_TASK });
    let posted = dispatch(&mut d, STATUS);
    let done = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    assert_eq!(tier_runs(&start).len(), 1);
    let api::Write::Status { check, .. } = effect_write(&posted) else { panic!("receipt posts one status") };
    assert_eq!(*check, api::Check::Passed);
    let Some(TierDecision::Done { words }) = tier_decision(&done) else { panic!("receipt task finishes") };
    assert_eq!(words.as_ref(), b"Tier browser run 1 at 040404040404 judged by receipt: green, 0 landings cleared");
    let mut receipt = false;
    for request in &done {
        if let Request::Save { record: Stored::Receipt { .. }, .. } = request {
            receipt = true;
        }
    }
    assert!(!receipt);
    assert_eq!(fixtures::browser(&d).branch, TierBranch::Absent);
}
#[test]
fn the_next_run_starts_at_the_newest_tip_when_one_ends() {
    let mut d = dispatched();
    fixtures::tier_move(&mut d, MERGE, None);
    fixtures::tier_move(&mut d, HEAD, None);
    completed(&mut d, api::Conclusion::Success, None);
    let done = judged(&mut d);
    let mut next = None;
    for request in &done {
        if let Request::TierRun { number, tip, .. } = request {
            next = Some((*number, *tip));
        }
    }
    assert_eq!(next, Some((2, HEAD)));
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().landings.len(), TWO_TIERS);
}
#[test]
fn a_restart_mid_run_finds_its_run_again_and_judges_it() {
    let d = dispatched();
    let row = fixtures::browser(&d).clone();
    let mut restored = made_branch();
    outputs(&mut restored, Event::Restore { record: Stored::Tier(row) });
    crate::tiers::rebuild(&mut restored);
    completed(&mut restored, api::Conclusion::Success, None);
    let done = judged(&mut restored);
    let Some(TierDecision::Done { .. }) = tier_decision(&done) else { panic!("restored run finishes") };
    assert_eq!(fixtures::browser(&restored).record.sequence, 1);
}

#[test]
fn a_third_refused_dispatch_holds_the_task() {
    let mut d = made_branch();
    outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: DISPATCH });
    let mut out: Box<[Request]> = Box::new([]);
    for (failed, next) in [(DISPATCH, RETRY), (RETRY, STATUS), (STATUS, DISPATCH)] {
        let mut ignored = Queue::with_capacity(max_out(&LIMITS));
        crate::tiers::outcome(&mut d, failed, skein_forge_client::Outcome::Failed(api::Error::Missing), &mut ignored);
        d.entries.remove(&failed);
        out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: next });
    }
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().faults, 3);
    assert_eq!(tier_decision(&out), Some(&TierDecision::Held));
}

fn key_domain(ignored: &[&[u8]]) -> Domain {
    let mut d = fixtures::tiered();
    let mut configured = fixtures::repository_tiers(&[BROWSER]);
    let mut paths = skein_lib::List::with_capacity(LIMITS.ignored_paths);
    for path in ignored {
        paths.push(Box::from(*path)).expect("fixture ignored paths");
    }
    configured[0].tiers[0].ignored = paths.into_boxed();
    d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), configured);
    d
}
fn key_start(ignored: &[&[u8]]) -> (Domain, Queue<Request>) {
    let mut d = key_domain(ignored);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::start_due(&mut d, &env(), REPO, &mut out);
    (d, out)
}
fn tree_call(d: &mut Domain, out: &Queue<Request>) -> (Token, api::Read) {
    for request in out {
        if let Request::Call { call, op: api::Op::Read(read), .. } = request {
            return (*call, read.clone());
        }
    }
    let (call, api::Op::Read(read)) = fixtures::next_call(d) else { panic!("tree read") };
    (call, read)
}
fn tree_reply(d: &mut Domain, out: &Queue<Request>, entries: Box<[api::Entry]>, more: bool) -> Queue<Request> {
    let (call, _) = tree_call(d, out);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    for request in fixtures::answer(d, call, Ok(api::Answer::Tree { entries, more })) {
        out.push(request);
    }
    out
}
fn tree_entry(name: &[u8], object: [u8; 32]) -> api::Entry {
    api::Entry { name: name.into(), mode: 0o040_000, kind: api::EntryKind::Tree, object }
}
fn root(docs: [u8; 32], source: [u8; 32]) -> Box<[api::Entry]> {
    Box::new([tree_entry(b"docs", docs), tree_entry(b"src", source)])
}
fn baseline_key(ignored: &[&[u8]], docs: [u8; 32], source: [u8; 32]) -> [u8; 32] {
    let (mut d, out) = key_start(ignored);
    tree_reply(&mut d, &out, root(docs, source), false);
    fixtures::browser(&d).record.run.as_ref().expect("keyed baseline").key
}
#[test]
fn a_landing_that_changes_only_an_ignored_path_keeps_the_tiers_key() {
    assert_eq!(baseline_key(&[b"docs"], BASE, BASE), baseline_key(&[b"docs"], HEAD, BASE));
}
#[test]
fn a_landing_that_changes_a_read_path_changes_the_tiers_key() {
    assert_ne!(baseline_key(&[b"docs"], BASE, BASE), baseline_key(&[b"docs"], BASE, HEAD));
}
#[test]
fn two_tiers_with_different_ignored_paths_key_one_landing_differently() {
    let mut d = fixtures::tiered();
    let mut configured = fixtures::repository_tiers(&[BROWSER, fixtures::FUZZY]);
    configured[0].tiers[0].ignored = Box::new([Box::from(b"docs".as_slice())]);
    configured[0].tiers[1].ignored = Box::new([Box::from(b"src".as_slice())]);
    d.project_tiers(fixtures::PROJECT, fixtures::tier_settings(), configured);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::start_due(&mut d, &env(), REPO, &mut out);
    // The client may share the same root read between both owners.
    let next = tree_reply(&mut d, &out, root(BASE, HEAD), false);
    if d.tier_row(REPO, fixtures::FUZZY).unwrap().record.run.is_none() {
        tree_reply(&mut d, &next, root(BASE, HEAD), false);
    }
    let browser = fixtures::browser(&d).record.run.as_ref().expect("browser key").key;
    let fuzzy = d.tier_row(REPO, fixtures::FUZZY).unwrap().record.run.as_ref().expect("fuzzy key").key;
    assert_ne!(browser, fuzzy);
}
#[test]
fn a_run_waits_for_its_ranges_keys_then_dispatches_with_its_key() {
    let (mut d, out) = key_start(&[]);
    tree_reply(&mut d, &out, Box::new([tree_entry(b"src", BASE)]), false);
    let row = d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap();
    row.record.judged = Some(policy::Judged { tip: BASE, key: fixtures::BROWSER_BASE_KEY });
    row.record.run = None;
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, MERGE, CHANGE, &mut out);
    fixtures::tier_tip(&mut d, HEAD);
    crate::tiers::moved(&mut d, &env(), REPO, BASE, HEAD, Some(&[MERGE, HEAD]), &mut out);
    for landing in &fixtures::browser(&d).pending {
        assert_eq!(landing.key, None);
    }
    assert!(tier_runs(&out).is_empty());
    let first = tree_reply(&mut d, &out, Box::new([tree_entry(b"src", MERGE)]), false);
    assert!(tier_runs(&first).is_empty());
    let second = tree_reply(&mut d, &first, Box::new([tree_entry(b"src", HEAD)]), false);
    assert_eq!(tier_runs(&second).as_slice(), &[(1, HEAD)]);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 1, task: TIER_TASK });
    d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap().branch = TierBranch::Made;
    let dispatched = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: DISPATCH });
    let api::Write::Dispatch { inputs, .. } = effect_write(&dispatched) else { panic!("keyed dispatch") };
    assert_eq!(inputs[1].name.as_ref(), b"temper_key");
    assert_eq!(inputs[1].value.as_ref(), fixtures::BROWSER_HEAD_HEX);
}
#[test]
fn a_tree_past_the_keys_bounds_falls_back_to_the_commit() {
    const KEY_ENTRIES: u32 = 1; // one source tree fits; adding docs crosses the bound.
    for (entries, expected) in
        [(Box::from([tree_entry(b"src", BASE)]), fixtures::BROWSER_BASE_KEY), (root(BASE, BASE), BASE)]
    {
        let (mut d, out) = key_start(&[]);
        let (call, _) = tree_call(&mut d, &out);
        let mut bounded = env();
        bounded.limits.key_entries = KEY_ENTRIES;
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        step(
            &mut d,
            &bounded,
            Event::Client(skein_forge_client::Event::Answered {
                free: 0,
                rate: None,
                call,
                cost: 1,
                result: Ok(api::Answer::Tree { entries, more: false }),
            }),
            &mut out,
        );
        assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().key, expected);
    }
}
#[test]
fn a_restart_mid_walk_reads_its_trees_again_and_finishes() {
    let (mut d, out) = key_start(&[b"docs/guide.md"]);
    let next = tree_reply(&mut d, &out, root(MERGE, HEAD), false);
    let (_, read) = tree_call(&mut d, &next);
    assert_eq!(read, api::Read::Tree { object: MERGE, page: 1 });
    let row = fixtures::browser(&d).clone();
    let mut restored = key_domain(&[b"docs/guide.md"]);
    outputs(&mut restored, Event::Restore { record: Stored::Tier(row) });
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::restart(&mut restored, &env(), &mut out);
    let (call, read) = tree_call(&mut restored, &out);
    assert_eq!(read, api::Read::Tree { object: BASE, page: 1 });
    let replies =
        fixtures::answer(&mut restored, call, Ok(api::Answer::Tree { entries: root(MERGE, HEAD), more: false }));
    let mut next = Queue::with_capacity(max_out(&LIMITS));
    for request in replies {
        next.push(request);
    }
    tree_reply(&mut restored, &next, Box::new([tree_entry(b"guide.md", BASE)]), false);
    assert!(fixtures::browser(&restored).record.run.is_some());
    assert!(fixtures::browser(&restored).pending.is_empty());
}
#[test]
fn the_definition_bytes_carry_workflow_ignored_artifact_and_format() {
    let mut tier = fixtures::repository_tiers(&[BROWSER])[0].tiers[0].clone();
    tier.ignored = Box::new([Box::from(b"docs".as_slice()), Box::from(b"src/a".as_slice())]);
    for (format, expected) in [
        (policy::Format::Junit, b"\0\x08tier.yml\0\x02\0\x04docs\0\x05src/a\0\x06report\0".as_slice()),
        (policy::Format::Lines, b"\0\x08tier.yml\0\x02\0\x04docs\0\x05src/a\0\x06report\x01".as_slice()),
    ] {
        tier.format = format;
        assert_eq!(crate::tiers::definition(&tier).as_ref(), expected);
    }
}

#[test]
fn a_move_while_the_baseline_keys_waits_for_the_next_range() {
    let (mut d, out) = key_start(&[]);
    fixtures::tier_tip(&mut d, MERGE);
    let mut moved = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::moved(&mut d, &env(), REPO, BASE, MERGE, None, &mut moved);
    tree_reply(&mut d, &out, Box::new([tree_entry(b"src", BASE)]), false);
    let row = fixtures::browser(&d);
    assert_eq!(row.record.run.as_ref().expect("original baseline").tip, BASE);
    assert!(row.record.run.as_ref().unwrap().landings.is_empty());
    assert_eq!(row.pending.as_ref(), &[TierLanding { commit: MERGE, key: None }]);
}

const THIRD: api::Commit = [9; 32];
const THIRD_CHANGE: u64 = 93;
const TWO_FAILURES: &[u8] = b"suite.first\nsuite.second\n";
fn tests() -> Box<[policy::TestId]> {
    Box::new([b"suite.first".as_slice().into(), b"suite.second".as_slice().into()])
}
fn failures() -> policy::Failures {
    policy::Failures::Tests(tests())
}
fn red_range(commits: &[api::Commit]) -> Domain {
    let mut d = made_branch();
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    for (commit, task) in commits.iter().zip([CHANGE, OPEN, THIRD_CHANGE]) {
        crate::verification::landed(&mut d, REPO, *commit, task, &mut out);
    }
    let row = d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap();
    row.record.judged = Some(policy::Judged { tip: BASE, key: fixtures::BROWSER_BASE_KEY });
    let run = row.record.run.as_mut().unwrap();
    run.tip = *commits.last().unwrap();
    run.key = run.tip;
    let mut landings = skein_lib::List::with_capacity(LIMITS.changes);
    for commit in commits {
        landings.push(policy::Landing { commit: *commit, key: *commit }).expect("range fits changes");
    }
    run.landings = landings.into_boxed();
    dispatch(&mut d, DISPATCH);
    d
}
fn failed_part(d: &mut Domain) {
    let mut run = workflow(d, api::Conclusion::Failure, Some(1));
    run.commit = d.tier_follows.get(&(REPO, BROWSER.into())).unwrap().commit;
    answer(d, api::Answer::Runs { runs: Box::new([run.clone()]), more: false });
    answer(d, api::Answer::Run(run));
}
fn report(d: &mut Domain, bytes: &[u8], truncated: bool) {
    answer(d, api::Answer::File { head: HEAD, path: b"failed.txt".as_slice().into(), bytes: bytes.into(), truncated });
}
fn red(d: &mut Domain) {
    failed_part(d);
    report(d, TWO_FAILURES, false);
    dispatch(d, RETRY);
    failed_part(d);
    report(d, TWO_FAILURES, false);
}
fn blame(d: &Domain, commit: api::Commit) -> &policy::Blame {
    &d.verifications.get(&(REPO, commit)).unwrap().tiers[0].verdict.as_ref().unwrap().blame
}
fn finish_parts(d: &mut Domain) {
    for entry in 201..213 {
        // status, at most one probe deletion and the whole branch deletion.
        let out = dispatch(d, entry);
        match tier_decision(&out) {
            Some(TierDecision::Done { .. }) => return,
            Some(TierDecision::Wait | TierDecision::Held) | None => {}
        }
    }
    panic!("tier task finishes after its branch effects");
}
#[test]
fn a_failed_runs_tests_are_read_from_its_junit_artifact() {
    let mut d = red_range(&[MERGE]);
    let configured = d.tiers.get_mut(&fixtures::PROJECT).unwrap();
    configured[0].tiers[0].format = policy::Format::Junit;
    failed_part(&mut d);
    report(&mut d, b"<testsuite><testcase classname='suite' name='first'><failure/></testcase><testcase classname='suite' name='second'><error/></testcase></testsuite>", false);
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().part, policy::Part::Rerun { failed: tests() });
}
#[test]
fn a_failed_runs_tests_are_read_from_its_lines_artifact() {
    let mut d = red_range(&[MERGE]);
    failed_part(&mut d);
    outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    let (_, op) = fixtures::next_call(&mut d);
    assert_eq!(
        op,
        api::Op::Read(api::Read::Artifact {
            run: RUN_ID,
            name: b"report".as_slice().into(),
            max_bytes: LIMITS.client.answer_bytes
        })
    );
    report(&mut d, TWO_FAILURES, false);
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().part, policy::Part::Rerun { failed: tests() });
}
#[test]
fn a_failed_run_whose_artifact_is_missing_is_a_fault_and_retried() {
    let mut d = red_range(&[MERGE]);
    failed_part(&mut d);
    d.tier_reads.insert(READ_OWNER, (REPO, BROWSER.into())).unwrap();
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::answered(&mut d, &env(), READ_OWNER, Err(api::Error::Missing), &mut out);
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    let api::Write::Dispatch { .. } = effect_write(&out) else { panic!("the fault is retried by dispatch") };
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().faults, 1);
    assert!(d.verifications.get(&(REPO, MERGE)).unwrap().tiers[0].verdict.is_none());
}
#[test]
fn a_truncated_report_blames_by_landing_alone() {
    let mut d = red_range(&[MERGE]);
    failed_part(&mut d);
    report(&mut d, b"suite.fir", true);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(policy::Failures::Many));
}
#[test]
fn failed_tests_rerun_alone_and_passing_alone_are_a_flake() {
    let mut d = red_range(&[MERGE]);
    failed_part(&mut d);
    report(&mut d, TWO_FAILURES, false);
    let out = dispatch(&mut d, RETRY);
    let api::Write::Dispatch { branch, inputs, .. } = effect_write(&out) else { panic!("selected rerun") };
    assert_eq!(branch.as_ref(), b"temper/tier/browser.1");
    assert_eq!(inputs[2], api::Input { name: b"temper_tests".as_slice().into(), value: TWO_FAILURES.into() });
    completed(&mut d, api::Conclusion::Success, None);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Cleared);
    assert_eq!(
        fixtures::browser(&d).record.flakes.as_ref(),
        &[
            policy::Flake { test: b"suite.first".as_slice().into(), count: 1 },
            policy::Flake { test: b"suite.second".as_slice().into(), count: 1 },
        ]
    );
    assert!(d.landing.receipts.is_empty(), "a partial rerun never makes a receipt");
}
#[test]
fn failures_the_previous_run_had_are_inherited() {
    let mut d = red_range(&[MERGE]);
    red(&mut d);
    finish_parts(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::verification::landed(&mut d, REPO, HEAD, OPEN, &mut out);
    fixtures::tier_move(&mut d, HEAD, None);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 2, task: TIER_TASK });
    dispatch(&mut d, STATUS); // make the new run's branch.
    dispatch(&mut d, DISPATCH);
    red(&mut d);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(failures()));
    assert!(fixtures::browser(&d).record.failing == failures());
    assert_eq!(blame(&d, HEAD), &policy::Blame::Cleared);
    assert_eq!(
        crate::verification::verification(&d, REPO, HEAD),
        temper_engine_domain_forge_change::Verification::Cleared
    );
}
#[test]
fn new_failures_of_one_landing_are_its_own() {
    let mut d = red_range(&[MERGE]);
    red(&mut d);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(failures()));
}
fn probe_step(d: &mut Domain, entry: u64) -> Box<[Request]> {
    let out = outputs(d, Event::StepTier { task: TIER_TASK, entry });
    if let api::Write::CreateBranch { .. } = effect_write(&out) {
        let mut ignored = Queue::with_capacity(max_out(&LIMITS));
        crate::tiers::outcome(
            d,
            entry,
            skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Branch(MERGE), found: false },
            &mut ignored,
        );
        d.entries.remove(&entry);
    }
    out
}
#[test]
fn new_failures_over_three_landings_are_bisected() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    red(&mut d);
    probe_step(&mut d, 301);
    dispatch(&mut d, 302);
    completed(&mut d, api::Conclusion::Success, None);
    dispatch(&mut d, 303); // delete the first probe.
    probe_step(&mut d, 304);
    dispatch(&mut d, 305);
    failed_part(&mut d);
    report(&mut d, TWO_FAILURES, false);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Cleared);
    assert_eq!(blame(&d, HEAD), &policy::Blame::Owns(failures()));
    assert_eq!(blame(&d, THIRD), &policy::Blame::Cleared);
}
#[test]
fn a_landing_with_the_previous_key_is_never_probed() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    d.tier_rows.get_mut(&(REPO, BROWSER.into())).unwrap().record.run.as_mut().unwrap().landings[1].key = MERGE;
    red(&mut d);
    probe_step(&mut d, 301);
    dispatch(&mut d, 302);
    completed(&mut d, api::Conclusion::Success, None);
    assert_eq!(
        fixtures::browser(&d).result.as_ref().unwrap().verdict.landings.as_ref(),
        &[policy::Blame::Cleared, policy::Blame::Cleared, policy::Blame::Owns(failures()),]
    );
}
#[test]
fn tell_all_makes_every_landing_of_the_range_a_suspect() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    d.tier_settings.get_mut(&fixtures::PROJECT).unwrap().verdict.suspects = policy::Suspects::TellAll;
    red(&mut d);
    finish_parts(&mut d);
    for commit in [MERGE, HEAD, THIRD] {
        assert_eq!(blame(&d, commit), &policy::Blame::Suspect(failures()));
    }
}
#[test]
fn a_probes_branch_is_made_at_its_landing_and_released_after() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    red(&mut d);
    let made = probe_step(&mut d, 301);
    assert_eq!(
        effect_write(&made),
        &api::Write::CreateBranch { branch: b"temper/tier/browser.1.0".as_slice().into(), commit: MERGE }
    );
    let out = dispatch(&mut d, 302);
    let api::Write::Dispatch { inputs, .. } = effect_write(&out) else { panic!("probe dispatch") };
    assert_eq!(inputs[0].value, landing::inputs(MERGE, MERGE)[0].value);
    failed_part(&mut d);
    report(&mut d, TWO_FAILURES, false);
    dispatch(&mut d, STATUS); // post the verdict before deleting the last probe.
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: 303 });
    assert_eq!(effect_write(&out), &api::Write::DeleteBranch { branch: b"temper/tier/browser.1.0".as_slice().into() });
    let saved = &d.entries.get(&303).unwrap().entry;
    assert_eq!(d.tier_effect_kind(saved), None);
    assert_release(&out, 303);
}
#[test]
fn a_judged_runs_branch_is_released_before_its_task_is_done() {
    let mut d = dispatched();
    completed(&mut d, api::Conclusion::Success, None);
    dispatch(&mut d, STATUS);
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    assert_eq!(effect_write(&out), &api::Write::DeleteBranch { branch: b"temper/tier/browser.1".as_slice().into() });
    assert_eq!(tier_decision(&out), Some(&TierDecision::Wait));
    assert!(fixtures::browser(&d).result.as_ref().unwrap().applied);
    assert_release(&out, RETRY);
}
fn assert_release(out: &[Request], number: u64) {
    let mut found = false;
    for request in out {
        if let Request::TierEffect { .. } = request {
            panic!("cleanup is not an authority-gated effect")
        }
        if let Request::Save { record: Stored::Entry { entry, .. }, release: Some(release), .. } = request {
            assert_eq!((*release, entry.number), (number, number));
            found = true;
        }
    }
    assert!(found, "cleanup hands its durable entry to the client after commit");
}
#[test]
fn a_restart_mid_probe_follows_it_to_its_verdict() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    red(&mut d);
    probe_step(&mut d, 301);
    dispatch(&mut d, 302);
    let row = fixtures::browser(&d).clone();
    outputs(&mut d, Event::Restore { record: Stored::Tier(row) });
    d.tier_follows.remove(&(REPO, BROWSER.into()));
    crate::tiers::rebuild(&mut d);
    assert_eq!(d.tier_follows.get(&(REPO, BROWSER.into())).unwrap().commit, MERGE);
    failed_part(&mut d);
    report(&mut d, TWO_FAILURES, false);
    finish_parts(&mut d);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(failures()));
}
#[test]
fn a_red_landing_is_repaired_by_a_run_that_passes_its_tests() {
    let mut d = red_range(&[MERGE]);
    red(&mut d);
    finish_parts(&mut d);
    fixtures::tier_move(&mut d, HEAD, None);
    outputs(&mut d, Event::TierTask { repository: REPO, tier: BROWSER.into(), number: 2, task: TIER_TASK });
    dispatch(&mut d, STATUS);
    dispatch(&mut d, DISPATCH);
    completed(&mut d, api::Conclusion::Success, None);
    finish_parts(&mut d);
    assert_eq!(d.verifications.get(&(REPO, MERGE)).unwrap().tiers[0].repaired, Some(HEAD));
    assert_eq!(
        crate::verification::verification(&d, REPO, MERGE),
        temper_engine_domain_forge_change::Verification::Cleared
    );
}

#[test]
fn a_failed_deletion_holds_the_task_without_undoing_its_verdict() {
    let mut d = red_range(&[MERGE]);
    red(&mut d);
    dispatch(&mut d, STATUS);
    outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: 403 });
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::outcome(&mut d, 403, skein_forge_client::Outcome::Failed(api::Error::Refused), &mut out);
    let mut held = false;
    for request in &out {
        if let Request::TierDecision { decision, .. } = request {
            assert_eq!(decision, &TierDecision::Held);
            held = true;
        }
    }
    assert!(held);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(failures()));
    assert_eq!(fixtures::browser(&d).record.failing, failures());
}
#[test]
fn a_failed_last_probe_deletion_keeps_the_judged_verdict() {
    let mut d = red_range(&[MERGE, HEAD, THIRD]);
    red(&mut d);
    probe_step(&mut d, 301);
    dispatch(&mut d, 302);
    failed_part(&mut d);
    report(&mut d, TWO_FAILURES, false);
    dispatch(&mut d, STATUS);
    let out = outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: 403 });
    assert_eq!(effect_write(&out), &api::Write::DeleteBranch { branch: b"temper/tier/browser.1.0".as_slice().into() });
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    crate::tiers::outcome(&mut d, 403, skein_forge_client::Outcome::Failed(api::Error::Refused), &mut out);
    assert_eq!(blame(&d, MERGE), &policy::Blame::Owns(failures()));
    assert_eq!(fixtures::browser(&d).record.failing, failures());
    assert!(fixtures::browser(&d).result.as_ref().unwrap().applied);
}
#[test]
fn a_restart_while_reading_a_failed_report_reads_it_again() {
    let mut d = red_range(&[MERGE]);
    failed_part(&mut d);
    let row = fixtures::browser(&d).clone();
    outputs(&mut d, Event::Restore { record: Stored::Tier(row) });
    d.tier_follows.remove(&(REPO, BROWSER.into()));
    crate::tiers::rebuild(&mut d);
    outputs(&mut d, Event::StepTier { task: TIER_TASK, entry: RETRY });
    let (_, op) = fixtures::next_call(&mut d);
    assert_eq!(
        op,
        api::Op::Read(api::Read::Artifact {
            run: RUN_ID,
            name: b"report".as_slice().into(),
            max_bytes: LIMITS.client.answer_bytes
        })
    );
    report(&mut d, TWO_FAILURES, false);
    assert_eq!(fixtures::browser(&d).record.run.as_ref().unwrap().part, policy::Part::Rerun { failed: tests() });
}
