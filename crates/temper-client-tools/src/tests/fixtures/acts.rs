use super::*;
/// The shared message's task, words and name establish the engine-key mapping.
pub(crate) const MESSAGE_CALL: &[&str] = &["task", "message", "41", "Use port 8080", "--key", "port-answer"];
/// Optional names can be omitted on stop, unlike a message's required name.
pub(crate) const STOP_CALL: &[&str] = &["task", "stop", "41"];
/// Kept operations from another party must stay parked while this party is live.
pub(crate) const OTHER_PARTY: u64 = PARTY + 1;
pub(crate) fn offered(acts: &[c::ActKind]) -> c::Task {
    let mut task = goal(TASK, waiting_on_me());
    task.acts = List::with_capacity(u32::try_from(acts.len()).expect("fixture offers fit"));
    for act in acts {
        task.acts
            .push(c::Offered { act: *act, admission: c::Admission::Allowed, effect: None })
            .expect("fixture offers");
    }
    task
}
pub(crate) fn message() -> (Harness, u32) {
    let mut root = signed_in(PARTY);
    let call = root.submit(MESSAGE_CALL);
    root.serve(call, task_result(offered(&[c::ActKind::Message])));
    (root, call)
}
pub(crate) fn named(word: &str) -> skein_lib::RequestKey {
    crate::jig::key::named(crate::key::TOOLS, &name(word.as_bytes()))
}
impl Harness {
    pub(crate) fn done(&mut self, call: u32, outcome: c::Outcome) {
        self.answered(call, c::Answer::Done(outcome));
    }
    pub(crate) fn answered(&mut self, call: u32, answer: c::Answer) {
        if let c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::PolicyChanged { .. })) = &answer {
            self.pending_reads.push(call);
        }
        let (attempt, _, _) = self.sends.get(&call).expect("call sent");
        self.client(client::Event::Up {
            up: client::Up::Answer {
                attempt: *attempt,
                envelope: skein_lib::RequestEnvelope::Final,
                answer: Some(client::Answered::Jig(answer)),
                why: None,
            },
        });
    }
    pub(crate) fn answers_full(&mut self, call: u32, held: u32, room_at: Wall) {
        let (attempt, _, _) = self.sends.get(&call).expect("call sent");
        self.client(client::Event::Up {
            up: client::Up::Answer {
                attempt: *attempt,
                envelope: skein_lib::RequestEnvelope::Again,
                answer: None,
                why: Some(client::Retry::Full { held, room_at }),
            },
        });
    }
    pub(crate) fn sent_name(&self, call: u32) -> Option<skein_lib::RequestKey> {
        let (_, key, _) = self.sends.get(&call)?;
        Some(*key)
    }
}
/// Distinct clocks and seeds exercise process independence without changing me.
pub(crate) const LATER: u64 = 90_000_000_000;
/// A second seed must differ for a drawn envelope key to differ.
pub(crate) const OTHER_SEED: u64 = SEED + 1;
/// One template and one charter suffice to authorize a chat.
const CHAT_ENTRIES: u32 = 1;
/// The ordinary bound is exercised exactly and one unit above it.
pub(crate) const CHAT_BUDGET: u64 = 100;
/// The hard ceiling remains above the ordinary bound so a proposal fits between.
pub(crate) const CHAT_CEILING: u64 = 500;
/// One unit above a bound is its first excluded amount.
pub(crate) const OVER_BOUND: u64 = 1;
/// A nonzero span must survive both clock changes unchanged.
pub(crate) const CHAT_SPAN: skein_lib::Duration = skein_lib::Duration::from_secs(60);
/// A different valid revision makes escalation arguments observable.
pub(crate) const HOLD: u64 = 2;
/// One proposal number is shared by both typed proposer fixtures.
pub(crate) const PROPOSAL: u64 = 17;
/// A fixed winner's wall prints 14:02 in UTC.
pub(crate) const DECIDED_AT: Wall = Wall::from_nanos(50_520_000_000_000);
/// The proposed recipient differs from both signed-in fixture parties.
pub(crate) const RECIPIENT: u64 = 91;
/// Shared keyed create form, with a project whose name is supplied in me.
pub(crate) const CREATE_CHAT_CALL: &[&str] = &["chat", "start", "hello", "--project", "shop", "--key", "start-chat"];
/// Shared close form; its reason is absent, as the default promises.
pub(crate) const CLOSE_CALL: &[&str] = &["chat", "close", "41", "--key", "close-chat"];
/// Both live delegates must be covered to distinguish move from cancel.
pub(crate) const DELEGATES: u32 = 2;
/// The first delegate is moved; the second must still be cancelled.
pub(crate) const MOVED: u64 = 43;
pub(crate) const CANCELLED: u64 = 47;
/// One slot per named application request being restored.
const RESTORED: u32 = 1;
/// Common explicit name for optional-key acts, independent of each process.
pub(crate) const DECIDE_TASK_CALL: &[&str] = &["proposal", "decide", "t41/17", "accept", "--key", "act-name"];
pub(crate) const DECIDE_PARTY_CALL: &[&str] = &["proposal", "decide", "p73/17", "accept", "--key", "act-name"];
pub(crate) const DECIDE_ESCALATION_CALL: &[&str] =
    &["task", "decide", "41", "release", "--hold", "2", "--key", "act-name"];
/// A move needs a sibling delegate left to cancel; one would miss that distinction.
pub(crate) const CLOSE_MOVE: &[&str] = &["--move", "43"];
pub(crate) const CLOSE_REASON_WORDS: &str = "all finished";
pub(crate) const CLOSE_REASON: &[&str] = &["--reason", CLOSE_REASON_WORDS];
/// A smaller task number would not match the director's restored-proposal example.
pub(crate) const KEPT_PROPOSER: u64 = 3;
/// The changed request differs from the held message under the same name.
pub(crate) const OTHER_MESSAGE: &[&str] = &["task", "message", "41", "different words", "--key", "port-answer"];
/// JSON's extra word is shared by all act-output cases.
pub(crate) const JSON_FLAG: &[&str] = &["--json"];
/// Every act form is stable under clocks and seeds; the row carries its arguments.
pub(crate) const ACT_CALLS: &[&[&str]] = &[
    MESSAGE_CALL,
    CLOSE_CALL,
    &["task", "answer", "41", "3", "yes", "--key", "act-name"],
    DECIDE_TASK_CALL,
    DECIDE_ESCALATION_CALL,
    &["task", "release", "41", "--hold", "2", "--again", "--key", "act-name"],
    &["task", "stop", "41", "--key", "act-name"],
    &["task", "cancel", "41", "finished", "--key", "act-name"],
    &["hold", "lift", "12", "--rev", "1", "--with-tasks", "--key", "lift-name"],
];
pub(crate) fn chat_me(budget: u64) -> c::Me {
    let mut me = me(PARTY);
    let mut authority = goal(TASK, waiting_on_me()).authority;
    authority.cut = false;
    authority.authority.delegation.depth = CHAT_ENTRIES;
    let mut charters = List::with_capacity(CHAT_ENTRIES);
    charters.push(1).expect("one charter");
    let project = me.projects.get_mut(0).expect("one project");
    project.creating = Some(c::Creating {
        within: c::CreateBounds {
            authority: Box::new(authority),
            pool_left: CHAT_CEILING,
            charters,
            procedures: List::with_capacity(EMPTY),
            latest: None,
            budget: CHAT_BUDGET,
            maximum_budget: CHAT_CEILING,
            maximum_deadline: None,
        },
        beyond: c::Beyond::Proposed { to: c::Recipient::Party(RECIPIENT) },
    });
    project.templates = List::with_capacity(CHAT_ENTRIES);
    project
        .templates
        .push(c::Template {
            number: jig_client::shapes::CHAT,
            name: document_text(b"chat"),
            executor: c::TemplateExecutor::Charter(1),
            contract: c::TaskContractDocument::Report { words: crate::limits::TEXT },
            shallower: CHAT_ENTRIES,
            budget: c::TemplateBudget::Amount(budget),
            deadline: c::TemplateDeadline::Span(CHAT_SPAN),
            tracked: None,
            flags: c::TaskFlags {
                ended_by: jig_client::contract::EndedBy::Result,
                conversation: true,
                listed: false,
                retention: c::Retention::Kept,
            },
            label: jig_client::shapes::CHAT,
            grace: Some(CHAT_SPAN),
            llm: None,
        })
        .expect("one template");
    me
}
pub(crate) fn chat_root(budget: u64, seed: u64) -> Harness {
    let mut root = started_with(List::with_capacity(EMPTY), seed, false);
    root.deliver(c::Delivery::Me(Box::new(chat_me(budget))));
    root
}
pub(crate) fn proposal(proposer: c::Proposer, acts: &[c::ActKind]) -> c::ReadValue {
    c::ReadValue::OwnerProposal(Box::new(c::OwnerProposal {
        key: c::ProposalKey { proposer, number: PROPOSAL },
        project: PROJECT,
        reason: document_text(b"more time"),
        action: c::OwnerProposalAction::Batch(List::with_capacity(EMPTY)),
        as_holder: true,
        waiting: c::OwnerProposalWaiting { holder: c::OwnerHistoryHolder::Person(PARTY), since: Wall::EPOCH },
        funding: List::with_capacity(EMPTY),
        acts: offered(acts).acts,
    }))
}
pub(crate) fn act_document(call: &crate::jig::call::Call) -> c::ReadResult {
    let task = act_task();
    c::ReadResult::Value(match call.tool {
        11 => c::ReadValue::Question(Box::new(c::Question {
            task: task.chip,
            from: None,
            message: 3,
            words: document_text(b"Which port?"),
            since: Wall::EPOCH,
            acts: task.acts,
            ending: None,
        })),
        crate::jig::rows::DECIDE_AT => {
            let Some(crate::jig::call::Value::Proposal(key)) = crate::jig::call::value(call, 0) else {
                panic!("proposal form")
            };
            proposal(key.proposer, &[c::ActKind::Accept, c::ActKind::Reject, c::ActKind::PassUp])
        }
        crate::jig::rows::TASK_DECIDE_AT => c::ReadValue::OwnerEscalation(Box::new(c::OwnerEscalation {
            acts: task.acts.clone(),
            task: Box::new(task),
        })),
        _ => c::ReadValue::Task(Box::new(task)),
    })
}
pub(crate) fn send_act(argv: &[&str], seed: u64, now: u64) -> (Harness, u32) {
    let mut root = chat_root(CHAT_BUDGET, seed);
    root.now = Time::from_nanos(now);
    root.wall = Wall::from_nanos(now);
    let call = root.submit(argv);
    let crate::jig::call::Parsed::Call { call: parsed, .. } = parse(argv).expect("act argv") else {
        panic!("act call")
    };
    if parsed.tool == crate::jig::rows::LIFT_AT {
        return (root, call);
    }
    root.serve(call, act_document(&parsed));
    if root.reads.contains_key(&call) {
        root.serve(call, task_result(act_task()));
    }
    (root, call)
}
pub(crate) fn act_text(bytes: &[u8]) -> c::Text {
    c::Text::new(Box::from(bytes), crate::limits::TEXT).expect("fixture text")
}
pub(crate) fn encoded(ask: &client::Ask) -> Box<[u8]> {
    let client::Ask::Jig(ask) = ask else { panic!("jig ask") };
    let wire = jig_client_protocol_link::translate::asks::ask(ask, &jig_client_protocol_wire::v19::CEILINGS)
        .expect("wire ask");
    let mut out = skein_lib::Writer::new(usize::try_from(wire.measure()).expect("wire bytes fit"));
    wire.encode(&mut out).expect("measured ask");
    out.finish()
}
pub(crate) fn restored_message(party: u64) -> List<skein_lib::RequestRecord<client::Held>> {
    let mut records = List::with_capacity(RESTORED);
    records
        .push(skein_lib::RequestRecord {
            key: named("port-answer"),
            scope: party,
            first_sent: None,
            request: client::Held {
                key: client::Key { asker: root::RUNNER.asker(), number: (u64::from(RESTORED_PREFIX) << 32u32) | 1 },
                request: client::Ask::Jig(Box::new(c::Ask::Message {
                    project: PROJECT,
                    task: TASK,
                    words: call_words(MESSAGE_CALL, 1),
                })),
            },
        })
        .expect("one restored message");
    records
}

pub(crate) fn call_words(argv: &[&str], index: usize) -> c::Text {
    let call = call(argv);
    let Some(crate::jig::call::Value::Text(words)) = crate::jig::call::value(&call, index) else {
        panic!("fixture words")
    };
    act_text(words)
}
pub(crate) fn with_delegates() -> c::Task {
    let mut task = offered(&[c::ActKind::Cancel]);
    task.delegates = List::with_capacity(DELEGATES);
    for number in [MOVED, CANCELLED] {
        task.delegates
            .push(c::LiveDelegate { chip: goal(number, waiting_on_me()).chip, takeover_budget: CHAT_BUDGET })
            .expect("two delegates");
    }
    task
}
pub(crate) fn restored_chat() -> List<skein_lib::RequestRecord<client::Held>> {
    let mut records = restored_message(PARTY);
    let record = records.get_mut(EMPTY).expect("one restored record");
    let c::Ask::Create { project, tasks, .. } =
        jig_client::shapes::chat(&chat_me(CHAT_BUDGET), PROJECT, call_words(CREATE_CHAT_CALL, 1)).expect("chat shape")
    else {
        panic!("chat creates")
    };
    record.request.request = client::Ask::Jig(Box::new(c::Ask::Propose {
        project,
        action: c::ProposeAction::Batch { tasks },
        reason: act_text(b""),
    }));
    records
}
impl Harness {
    pub(crate) fn tell(&mut self, told: client::Told) {
        let env = Env { now: self.now, wall: self.wall, limits: self.limits.tools };
        let mut out = root::act_fixture::told(&mut self.root, &env, told);
        for _ in 0..out.len() {
            match out.pop().expect("counted outputs") {
                crate::jig::runner::Out::Printed { number, printed } => {
                    self.answers.insert(number, printed);
                }
                crate::jig::runner::Out::Kept { line } => self.kept_lines.push(line).expect("recovery output"),
                crate::jig::runner::Out::Progress { line } => self.progress.push(line).expect("bounded progress lines"),
                crate::jig::runner::Out::Client { .. }
                | crate::jig::runner::Out::Record { .. }
                | crate::jig::runner::Out::Ready
                | crate::jig::runner::Out::DraftAmendment { .. } => panic!("answer output"),
            }
        }
    }
    pub(crate) fn client_answer(&mut self, call: u32, outcome: client::Outcome) {
        self.tell(client::Told::Answered {
            key: client::Key { asker: root::RUNNER.asker(), number: (u64::from(call) << 32u32) | 1 },
            outcome,
        });
    }
}

pub(crate) fn json_message() -> List<&'static str> {
    extended(JSON_FLAG, MESSAGE_CALL)
}

fn act_task() -> c::Task {
    offered(&[
        c::ActKind::Message,
        c::ActKind::Cancel,
        c::ActKind::Answer,
        c::ActKind::Accept,
        c::ActKind::Reject,
        c::ActKind::Release,
        c::ActKind::LeaveHeld,
        c::ActKind::PassUp,
        c::ActKind::Stop,
    ])
}

pub(crate) fn send_chat(budget: u64, seed: u64, now: u64) -> (Harness, u32) {
    let mut root = chat_root(budget, seed);
    root.now = Time::from_nanos(now);
    root.wall = Wall::from_nanos(now);
    let call = root.submit(CREATE_CHAT_CALL);
    (root, call)
}
pub(crate) fn act_ask(root: &Harness, call: u32) -> &c::Ask {
    let (_, _, client::Ask::Jig(ask)) = root.sends.get(&call).expect("act sent") else { panic!("jig act") };
    ask
}
pub(crate) fn close_operations(root: &Harness, call: u32) -> &List<c::Operation> {
    let c::Ask::Compound { operations, .. } = act_ask(root, call) else { panic!("compound close") };
    operations
}
