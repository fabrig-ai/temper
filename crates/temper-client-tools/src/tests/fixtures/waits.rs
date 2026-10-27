//! Live facts shared by conditions, digests and wait stories.
use super::*;
/// One counted need distinguishes an empty inbox from a waiting one.
pub(crate) const ONE_NEED: u32 = 1;
/// A policy role must differ from the four built-in roles.
pub(crate) const POLICY_ROLE: u32 = 7;
/// A role in another project must not match the task's project.
pub(crate) const OTHER_PROJECT: u32 = PROJECT + 1;
/// The settled chat need not share the task's opaque label.
pub(crate) const CHAT_LABEL: u32 = 97;
pub(crate) const LIVE_PHASES: &[c::Phase] = &[
    c::Phase::Waiting,
    c::Phase::Idle,
    c::Phase::Due,
    c::Phase::Preparing,
    c::Phase::Claimed,
    c::Phase::Running,
    c::Phase::BackingOff,
    c::Phase::Active,
    c::Phase::Closing,
    c::Phase::Held,
];
pub(crate) const ENDED_PHASES: &[c::Phase] = &[c::Phase::Ended, c::Phase::Done, c::Phase::Failed, c::Phase::Cancelled];
pub(crate) fn chat_at(task: u64, stage: c::RunStage, work: Option<c::OwnerChipWork>) -> client::Latest {
    let mut task = goal(task, c::WaitsOn::Nothing);
    task.label = CHAT_LABEL;
    task.flags.conversation = true;
    task.chip.phase = c::OwnerChipPhase::Active(stage).compact();
    task.chip.details = Box::new(c::ChipPhase::Owner {
        phase: c::OwnerChipPhase::Active(stage),
        details: Box::new(c::OwnerChipDetails {
            dependencies: List::with_capacity(EMPTY),
            project: task.project,
            executor: c::OwnerExecutor::Agent { charter: PROJECT },
            phase_since: Wall::EPOCH,
            run_since: None,
            attempt: Some(ATTEMPT),
            retry: Some(c::RetryNotice::Refused { count: ONE_NEED }),
            wake: task.wake,
            person_wait: None,
            hold: None,
            budget: task.budget.clone(),
            activity: 0,
            resources: List::with_capacity(EMPTY),
            resources_cut: false,
            resources_observed: true,
            procedure: None,
            live_delegates: 0,
            closing_connectors: None,
            ending: None,
            ended_at: None,
            observation: None,
        }),
    });
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner fixture") };
    details.observation = None;
    if let Some(work) = work {
        details.observation = Some(c::OwnerChipObservation {
            channel: Token::new(1),
            work,
            since: Wall::EPOCH,
            reported_at: Wall::EPOCH,
            started: None,
        });
    }
    client::Latest::Jig(c::Delivery::Task(Box::new(task)))
}
pub(crate) fn task_in(latest: &mut client::Latest) -> &mut c::Task {
    let client::Latest::Jig(c::Delivery::Task(task)) = latest else { panic!("task fixture") };
    task
}
pub(crate) fn ended_chat(phase: c::Phase) -> client::Latest {
    let mut latest = chat_at(TASK, c::RunStage::Idle, None);
    let task = task_in(&mut latest);
    task.chip.phase = phase;
    let c::ChipPhase::Owner { phase, details } = task.chip.details.as_mut() else { panic!("owner fixture") };
    *phase = c::OwnerChipPhase::Ended;
    details.ended_at = Some(DECIDED_AT);
    latest
}
pub(crate) fn party_wait(whom: c::Whom, kind: c::WaitedFor) -> c::WaitsOn {
    c::WaitsOn::Party { whom, task: OTHER_TASK, kind, entries: ONE_NEED }
}
pub(crate) fn inbox_counts(needs: u32, updates: u32) -> client::Latest {
    client::Latest::Jig(c::Delivery::Inbox(Box::new(c::Inbox {
        entries: List::with_capacity(EMPTY),
        needs: c::Needs { questions: needs, proposals: 0, escalations: 0, person_tasks: 0 },
        updates: c::Count { value: updates, limited: false },
        results_seen: 0,
        replies_seen: List::with_capacity(EMPTY),
        more: false,
    })))
}
pub(crate) fn owner_inbox_counts(needs: u32, updates: u32) -> client::Latest {
    client::Latest::Jig(c::Delivery::OwnerInbox(Box::new(c::OwnerInbox {
        party: PARTY,
        needs: List::with_capacity(EMPTY),
        updates: List::with_capacity(EMPTY),
        next: None,
        cached_needs: needs,
        cached_updates: c::Count { value: updates, limited: false },
        results_seen: 0,
        updates_time_complete: true,
        selection: None,
    })))
}
pub(crate) fn holds(
    condition: crate::jig::subject::Condition,
    latest: &client::Latest,
    me: Option<&c::Me>,
) -> Option<bool> {
    crate::jig::conditions::holds(&condition, &crate::jig::conditions::Facts { subject: latest, me })
}

pub(crate) fn tree_at(chip: c::Chip) -> client::Latest {
    let mut nodes = List::with_capacity(PAGE_ROWS);
    nodes
        .push(c::TreeNode {
            chip,
            parent: None,
            dependencies: List::with_capacity(EMPTY),
            proposals: List::with_capacity(EMPTY),
            spend: 0,
            folded: phase_counts(),
            held_below: 0,
        })
        .expect("root row");
    client::Latest::Jig(c::Delivery::Tree(Box::new(c::Tree {
        task: TASK,
        nodes,
        folded: phase_counts(),
        held_below: 0,
    })))
}
pub(crate) fn phase_counts() -> c::PhaseCounts {
    c::PhaseCounts { ended: 0, running: 0, waiting: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}
pub(crate) fn owner_tree_at(chip: c::Chip) -> client::Latest {
    client::Latest::Jig(c::Delivery::OwnerTree(Box::new(c::OwnerTree {
        root: chip,
        rows: List::with_capacity(EMPTY),
        folded: List::with_capacity(EMPTY),
        held_below: 0,
        next: None,
    })))
}
pub(crate) fn chip_phase(latest: &mut client::Latest, phase: c::OwnerChipPhase) {
    let task = task_in(latest);
    task.chip.phase = phase.compact();
    let c::ChipPhase::Owner { phase: current, .. } = task.chip.details.as_mut() else { panic!("owner chip") };
    *current = phase;
}

pub(crate) const WAIT_TASK: &[&str] = TASK_CALL;
pub(crate) const WAIT_TREE: &[&str] = &["task", "tree", TASK_CALL[2]];
pub(crate) const WAIT_RUN: &[&str] = &["task", "run", TASK_CALL[2]];
pub(crate) const WAIT_ME: &[&str] = ME_CALL;
pub(crate) const WAIT_INBOX: &[&str] = INBOX_CALL;
pub(crate) const WAIT_TASKS: &[&str] = TASKS_CALL;
/// One second exposes before/at deadline behavior without any real-time wait.
pub(crate) const WAIT_SPAN: skein_lib::Duration = skein_lib::Duration::from_secs(1);
pub(crate) const SHORT_TIMEOUT: &[&str] = &["--timeout", "1s"];
/// The shared me remains open after releasing every handle owned by a call.
pub(crate) const SHARED_WATCHES: usize = 1;
/// Two callers exercise independent handles on the same client watch.
pub(crate) const TWO_CALLERS: usize = 2;
pub(crate) fn task_watch() -> client::Watchable {
    client::Watchable::Jig(c::Subject::Task { task: TASK })
}
pub(crate) fn waiting_call(argv: &[&str], flags: &[&str]) -> (Harness, u32) {
    let mut root = signed_in(PARTY);
    let argv = extended(argv, flags);
    let argv = extended(
        argv.as_slice(),
        if argv.as_slice().contains(&"--since") || argv.as_slice().contains(&"--until") {
            &["--json"]
        } else {
            &["--until", "changed", "--json"]
        },
    );
    let call = root.submit(argv.as_slice());
    (root, call)
}
pub(crate) fn delivery(latest: client::Latest) -> c::Delivery {
    let client::Latest::Jig(delivery) = latest else { panic!("jig fixture") };
    delivery
}
pub(crate) fn parked() -> c::Delivery {
    delivery(chat_at(TASK, c::RunStage::Idle, None))
}
pub(crate) fn writing_chat() -> c::Delivery {
    delivery(chat_at(TASK, c::RunStage::Running { attempt: ATTEMPT }, Some(c::OwnerChipWork::Writing)))
}
pub(crate) fn task_digest(delivery: &c::Delivery) -> crate::jig::digest::Digest {
    crate::jig::digest::part(&crate::jig::render::Part::delivery(delivery).expect("subject"), &limits())
}
pub(crate) fn json_has(printed: &crate::jig::Printed, expected: &[u8]) -> bool {
    has(printed, expected)
}
pub(crate) fn has(printed: &crate::jig::Printed, expected: &[u8]) -> bool {
    skein_lib::bytes::find(&printed.stdout, expected).is_some()
}
impl Harness {
    pub(crate) fn fire(&mut self) {
        let env =
            Env { now: self.now, wall: self.wall, limits: root::Limits::new(self.limits.domain, self.limits.tools) };
        let mut out = Queue::with_capacity(OUTPUTS);
        root::fire(&mut self.root, &env, &mut out);
        self.outputs(out);
    }
    pub(crate) fn watch_event(&mut self, wanted: &client::Watchable, event: c::WatchEvent) {
        let token = self.watch_token(wanted);
        self.client(client::Event::Up { up: client::Up::Watched { watch: token, event } });
    }
}
impl Harness {
    pub(crate) fn wait_since(&mut self, argv: &[&str], flags: &[&str], digest: &[u8]) -> u32 {
        let argv = extended(argv, flags);
        let mut words = List::with_capacity(WORDS);
        for word in &argv {
            words.push(word.as_bytes()).expect("wait words");
        }
        for word in [b"--since".as_slice(), digest, b"--json".as_slice()] {
            words.push(word).expect("since words");
        }
        let crate::jig::call::Parsed::Call { call, json } = crate::table::parse(words.as_slice()).expect("wait call")
        else {
            panic!("parsed wait")
        };
        self.submit_call(call, json)
    }
}
pub(crate) struct WaitCase {
    pub argv: &'static [&'static str],
    pub watch: client::Watchable,
    pub delivery: c::Delivery,
    pub read_digest: Box<[u8]>,
}
/// Each of the six generic subjects has its own common document form.
const WAIT_SUBJECTS: usize = 6;
pub(crate) fn wait_cases(root: &mut Harness) -> [WaitCase; WAIT_SUBJECTS] {
    let call = root.submit(ME_JSON_CALL);
    let me = root.answer(call).expect("shared me read");
    let (_, inbox) = inbox_first_window(OVER_WINDOW);
    let argv = extended(INBOX_CALL, &["--json"]);
    let call = root.submit(argv.as_slice());
    root.serve(call, inbox_rows(c::InboxReadGroup::Needs, SHORT_WINDOW, false));
    root.serve(call, inbox_rows(c::InboxReadGroup::Updates, WINDOW_UPDATES, false));
    let inbox_read = root.answer(call).expect("inbox read");
    let (tasks, watched) = tasks_first_window(OVER_WINDOW, true);
    let task = goal(TASK, waiting_on_me());
    let argv = extended(TASKS_CALL, &["--json"]);
    let call = root.submit(argv.as_slice());
    root.serve(call, c::ReadResult::Value(c::ReadValue::Tasks(Box::new(tasks))));
    let tasks_read = root.answer(call).expect("tasks read");
    let argv = extended(TASK_CALL, &["--tree", "--json"]);
    let call = root.submit(argv.as_slice());
    root.serve(call, task_result(task.clone()));
    root.deliver_subject(
        &client::Watchable::Jig(c::Subject::Tree { task: TASK, window: crate::limits::ROWS }),
        c::Delivery::Tree(Box::new(composite_tree())),
    );
    let tree_read = root.answer(call).expect("composite read");
    let argv = extended(RUN_CALL, &["--json"]);
    let call = root.submit(argv.as_slice());
    root.deliver_run(no_provisional());
    let run_read = root.answer(call).expect("run read");
    let query = crate::jig::runner::reads::task_query(&call_of(TASKS_CALL), &super::me(PARTY)).expect("task query");
    [
        WaitCase {
            argv: WAIT_ME,
            watch: client::Watchable::Jig(c::Subject::Me),
            delivery: c::Delivery::Me(Box::new(super::me(PARTY))),
            read_digest: last_digest(&me.stdout),
        },
        WaitCase {
            argv: WAIT_INBOX,
            watch: client::Watchable::Jig(c::Subject::Inbox { window: crate::limits::ROWS }),
            delivery: c::Delivery::Inbox(Box::new(inbox)),
            read_digest: last_digest(&inbox_read.stdout),
        },
        WaitCase {
            argv: WAIT_TASKS,
            watch: client::Watchable::Jig(c::Subject::Tasks { query, window: crate::limits::ROWS }),
            delivery: c::Delivery::Tasks(Box::new(watched)),
            read_digest: last_digest(&tasks_read.stdout),
        },
        WaitCase {
            argv: WAIT_TASK,
            watch: task_watch(),
            delivery: c::Delivery::Task(Box::new(task.clone())),
            read_digest: first_digest(&tree_read.stdout),
        },
        WaitCase {
            argv: WAIT_TREE,
            watch: client::Watchable::Jig(c::Subject::Tree { task: TASK, window: crate::limits::ROWS }),
            delivery: c::Delivery::Tree(Box::new(composite_tree())),
            read_digest: last_digest(&tree_read.stdout),
        },
        WaitCase {
            argv: WAIT_RUN,
            watch: client::Watchable::Jig(c::Subject::Run { task: TASK, turn: None, offset: 0 }),
            delivery: no_provisional(),
            read_digest: last_digest(&run_read.stdout),
        },
    ]
}
fn call_of(argv: &[&str]) -> crate::jig::call::Call {
    let crate::jig::call::Parsed::Call { call, .. } = parse(argv).expect("fixture call") else { panic!("call") };
    call
}
pub(crate) fn changed_delivery(mut delivery: c::Delivery) -> c::Delivery {
    match &mut delivery {
        c::Delivery::Settings(_) => unreachable!("temper does not watch settings"),
        c::Delivery::Me(me) => me.needs_total = me.needs_total.checked_add(ONE_NEED).expect("new need"),
        c::Delivery::Inbox(inbox) => {
            inbox.entries = List::with_capacity(EMPTY);
            inbox.more = false;
        }
        c::Delivery::Tasks(tasks) => {
            tasks.rows = List::with_capacity(EMPTY);
            tasks.more = false;
        }
        c::Delivery::Task(task) => task.chip.title = document_text(b"changed"),
        c::Delivery::Tree(tree) => tree.held_below = ONE_NEED,
        c::Delivery::RunState(run) => run.committed_turn = TURN.checked_add(ONE_NEED).expect("next turn"),
        c::Delivery::OwnerProject(_)
        | c::Delivery::OwnerProposal(_)
        | c::Delivery::OwnerTree(_)
        | c::Delivery::Chips(_)
        | c::Delivery::OwnerInbox(_)
        | c::Delivery::Run(_)
        | c::Delivery::Deployment(_)
        | c::Delivery::OwnerDeployment(_)
        | c::Delivery::OwnerAccounts(_) => panic!("fixture subject"),
    }
    delivery
}

/// Digest fields are sixteen hex digits; smaller would omit part of the hash.
const DIGEST_DIGITS: usize = 16;
pub(crate) fn last_digest(bytes: &[u8]) -> Box<[u8]> {
    let field = b"\"digest\":\"";
    let mut start = None;
    for (at, window) in bytes.windows(field.len()).enumerate() {
        if window == field {
            start = Some(at.checked_add(field.len()).expect("field range"));
        }
    }
    let start = start.expect("read's subject digest");
    bytes[start..start.checked_add(DIGEST_DIGITS).expect("digest range")].into()
}

fn first_digest(bytes: &[u8]) -> Box<[u8]> {
    let field = b"\"digest\":\"";
    let start =
        skein_lib::bytes::find(bytes, field).expect("task part digest").checked_add(field.len()).expect("field range");
    bytes[start..start.checked_add(DIGEST_DIGITS).expect("digest range")].into()
}

pub(crate) fn plain_wait(flags: &[&str]) -> (Harness, u32) {
    let mut root = signed_in(PARTY);
    let argv = extended(WAIT_TASK, flags);
    let argv = extended(
        argv.as_slice(),
        if argv.as_slice().contains(&"--since") || argv.as_slice().contains(&"--until") {
            &[]
        } else {
            &["--until", "changed"]
        },
    );
    let call = root.submit(argv.as_slice());
    (root, call)
}
pub(crate) fn appended(bytes: &[u8], line: &[u8]) -> Box<[u8]> {
    let mut out = skein_lib::Writer::new(bytes.len().checked_add(line.len()).expect("expected text fits"));
    out.put(bytes).expect("subject text");
    out.put(line).expect("wait line");
    out.finish()
}
pub(crate) fn parked_task() -> c::Task {
    let c::Delivery::Task(task) = parked() else { panic!("parked task") };
    *task
}

pub(crate) const UNTIL_ENDED: &[&str] = &["--until", "ended"];
pub(crate) const UNTIL_REPLIED: &[&str] = &["--until", "replied"];
pub(crate) const UNTIL_RUNNING: &[&str] = &["--until", "phase:running"];
pub(crate) const UNTIL_WAITING_ON_ME: &[&str] = &["--until", "waiting-on-me"];
pub(crate) fn tree_watch() -> client::Watchable {
    client::Watchable::Jig(c::Subject::Tree { task: TASK, window: crate::limits::ROWS })
}
pub(crate) fn run_watch() -> client::Watchable {
    client::Watchable::Jig(c::Subject::Run { task: TASK, turn: None, offset: 0 })
}
pub(crate) fn ended_task(phase: c::Phase) -> c::Task {
    let c::Delivery::Task(task) = delivery(ended_chat(phase)) else { panic!("ended task") };
    *task
}
pub(crate) fn finishing_call() -> (Harness, u32) {
    let (mut root, call) = waiting_call(WAIT_TASK, UNTIL_ENDED);
    root.end(call, &task_watch(), c::WatchEnd::SubjectEnded);
    (root, call)
}
pub(crate) fn met_json(read: &[u8]) -> Box<[u8]> {
    appended(&appended(b"{\"task\":", read), b",\"met\":true}")
}
impl Harness {
    pub(crate) fn watch_token(&self, wanted: &client::Watchable) -> Token {
        for (token, subject) in &self.watches {
            if subject == wanted {
                return *token;
            }
        }
        panic!("open subject")
    }
    pub(crate) fn end(&mut self, call: u32, wanted: &client::Watchable, end: c::WatchEnd) -> Token {
        let watch = self.watch_token(wanted);
        self.watches.remove(&watch).expect("engine ended watch");
        self.serving = Some(call);
        self.client(client::Event::Up { up: client::Up::WatchEnded { watch, end } });
        self.serving = None;
        watch
    }
    pub(crate) fn final_delivery(&mut self, call: u32, task: c::Task) {
        self.serving = Some(call);
        self.deliver_subject(&task_watch(), c::Delivery::Task(Box::new(task)));
        self.serving = None;
    }
}

/// Only the runner's shared me fits; a task handle must hit the client's limit.
pub(crate) const SHARED_HANDLES: u32 = 1;

pub(crate) fn signed_in_with_handle_limit() -> Harness {
    let mut limits = limits_root();
    limits.domain.client.domain.handles = SHARED_HANDLES;
    let mut root = started_with_limits(List::with_capacity(EMPTY), SEED, false, limits);
    root.deliver(c::Delivery::Me(Box::new(me(PARTY))));
    root
}
