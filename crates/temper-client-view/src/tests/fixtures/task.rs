use super::*;
use jig_client::contract as c;
const EXCHANGES: u32 = 8;
const SPENT: u64 = 7;
const BELOW: u64 = 2;
pub fn document() -> c::Task {
    use jig_client_protocol_wire::v19 as w;
    let mut record = jig_client_test_records::task_record().into_parts();
    record.chip = jig_client_test_records::owner_waiting_chip_record();
    let record = w::Task::new(&w::CEILINGS, record).expect("checked fixture");
    let link = jig_client_protocol_link::Limits::new(limits().client.domain, EXCHANGES).expect("link");
    let mut task = jig_client_protocol_link::translate::documents::task(record, &link).expect("task");
    let c::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner chip") };
    *phase = c::OwnerChipPhase::Active(c::RunStage::Idle);
    task.budget.numbers.spent = SPENT;
    task.budget.numbers.spent_below = BELOW;
    task
}
const TASK: u64 = 9;
const EMPTY: u32 = 0;
const NO_TIME: Time = Time::ZERO;
const AT_EPOCH: Wall = Wall::EPOCH;
pub fn tree() -> c::Delivery {
    c::Delivery::OwnerTree(Box::new(c::OwnerTree {
        root: {
            let mut chip = document().chip;
            chip.task = TASK;
            chip
        },
        rows: List::with_capacity(EMPTY),
        folded: List::with_capacity(EMPTY),
        held_below: 0,
        next: None,
    }))
}
pub fn root(executor: c::Executor, priority: Option<u32>) -> temper_client_domain::Domain {
    root_with_hold(executor, priority, false)
}
pub fn held_root() -> temper_client_domain::Domain {
    root_with_hold(c::Executor::Agent { charter: 1 }, Some(1), true)
}
fn root_with_hold(executor: c::Executor, priority: Option<u32>, held: bool) -> temper_client_domain::Domain {
    let mut document = document();
    document.chip.task = TASK;
    document.chip.executor = executor;
    document.priority = priority;
    if held {
        phase(
            &mut document,
            c::OwnerChipPhase::Held {
                reason: c::HoldCategory::Failures { class: c::HoldFailure::Run },
                revision: HOLD_REVISION.value,
            },
        );
        document.hold = Some(c::TaskHold::Owner(Box::new(c::OwnerChipHold {
            placed: None,
            circle: None,
            account: None,
            source: None,
            reason: c::HoldCategory::Failures { class: c::HoldFailure::Run },
            revision: HOLD_REVISION.value,
            before: c::OwnerPriorPhase::Active(c::RunStage::Idle),
            escalation: None,
            tries: c::OwnerFailureTries { transient: 0, permanent: 0, run: 2, agent: 0, lost: 0, invalid: 0 },
            hold_wait_since: None,
        })));
        document.acts = List::with_capacity(ONE_OFFER);
        document
            .acts
            .push(c::Offered {
                act: c::ActKind::Release,
                admission: c::Admission::Allowed,
                effect: Some(c::Effect::Release(c::ReleaseEffect::RunAgain)),
            })
            .expect("release");
    }
    supplied_root(document, held)
}
pub fn root_with(mut document: c::Task) -> temper_client_domain::Domain {
    document.chip.task = TASK;
    document.chip.executor = c::Executor::Agent { charter: 1 };
    document.priority = Some(1);
    supplied_root(document, false)
}
pub fn supplied_root(document: c::Task, held: bool) -> temper_client_domain::Domain {
    supplied_root_watching(document, held).0
}
pub fn supplied_root_watching(document: c::Task, held: bool) -> (temper_client_domain::Domain, Token) {
    use jig_client::{Event as E, Up};
    let limits = domain_limits();
    let (mut root, _, watch, tree_watch) = root_at_with_tree(
        temper_client_domain::Address::Task { task: document.chip.task, filter: temper_client_domain::PlanFilter::All },
        &limits,
    );
    let card = document.clone();
    let mut tree = tree();
    if let c::Delivery::OwnerTree(value) = &mut tree {
        value.root.task = document.chip.task;
    }
    if held && let c::Delivery::OwnerTree(value) = &mut tree {
        value.root.phase = c::Phase::Held;
    }
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    let env = Env { now: NO_TIME, wall: AT_EPOCH, limits };
    temper_client_domain::step(
        &mut root,
        &env,
        temper_client_domain::Event::Client {
            event: E::Up {
                up: Up::Watched {
                    watch: watch.expect("task watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(document))),
                },
            },
        },
        &mut out,
    );
    if let Some(watch) = tree_watch {
        temper_client_domain::step(
            &mut root,
            &env,
            temper_client_domain::Event::Client {
                event: E::Up { up: Up::Watched { watch, event: c::WatchEvent::Delivered(tree) } },
            },
            &mut out,
        );
    }
    if held {
        while let Some(request) = out.pop() {
            if let temper_client_domain::Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Task { .. }) },
                    },
            } = request
            {
                root_event(
                    &mut root,
                    temper_client_domain::Event::Client {
                        event: E::Up {
                            up: Up::ReadDone {
                                read,
                                result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(
                                    card,
                                )))),
                            },
                        },
                    },
                );
                break;
            }
        }
    }
    (root, watch.expect("task watch"))
}

pub const HOLD_REVISION: c::Revision = c::Revision { value: 7 };
const ONE_OFFER: u32 = 1;

pub const RUN_ATTEMPT: u64 = 2;
pub const RUN_SINCE: Wall = Wall::from_nanos(1_792_640_880_000_000_000);
pub fn delegate() -> c::Task {
    let mut task = document();
    task.chip.task = TASK;
    task.chip.executor = c::Executor::Agent { charter: 1 };
    task.chip.title = document_words(b"Refresh token handling");
    task.priority = None;
    task.requester = c::OwnerRequester::Task(12);
    task.lineage_chips = List::with_capacity(2);
    let mut ancestor = task.chip.clone();
    ancestor.task = 3;
    ancestor.title = document_words(b"The root goal");
    task.lineage_chips.push(ancestor).expect("ancestor");
    let mut parent = task.chip.clone();
    parent.task = 12;
    parent.title = document_words(b"Rotate the credentials");
    task.lineage_chips.push(parent).expect("parent");
    task.spec.specification.words = document_words(b"Rotate tokens **safely**.");
    task.waits_on = c::WaitsOn::Nothing;
    task.hold = None;
    task.result = None;
    task.dependencies = List::with_capacity(0);
    task.activity = 1;
    phase(&mut task, c::OwnerChipPhase::Active(c::RunStage::Running { attempt: RUN_ATTEMPT }));
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.attempt = Some(RUN_ATTEMPT);
    details.run_since = Some(RUN_SINCE);
    details.ending = None;
    details.observation = Some(c::OwnerChipObservation {
        channel: Token::new(1),
        work: c::OwnerChipWork::Writing,
        since: RUN_SINCE,
        reported_at: RUN_SINCE,
        started: None,
    });
    task
}
pub fn document_words(bytes: &[u8]) -> c::DocumentText {
    c::DocumentText::complete(c::Text::new(Box::from(bytes), 32).expect("document words"))
}
pub fn phase(task: &mut c::Task, phase: c::OwnerChipPhase) {
    task.chip.phase = phase.compact();
    let c::ChipPhase::Owner { phase: old, .. } = task.chip.details.as_mut() else { panic!("owner") };
    *old = phase;
}
pub fn agent_root(document: c::Task, tree: Option<c::Delivery>, conversation: bool) -> temper_client_domain::Domain {
    agent_root_with_page(document, tree, activity(conversation))
}
pub fn agent_root_with_page(
    document: c::Task,
    tree: Option<c::Delivery>,
    page: c::ActivityPage,
) -> temper_client_domain::Domain {
    use jig_client::{Event as E, Up};
    use temper_client_domain::{Event, Request};
    let limits = domain_limits();
    let env = Env { now: NO_TIME, wall: AT_EPOCH, limits };
    let mut root = temper_client_domain::unlinked(&limits, SEED);
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    temper_client_domain::step(
        &mut root,
        &env,
        Event::Start {
            address: temper_client_domain::Address::Task { task: TASK, filter: temper_client_domain::PlanFilter::All },
            kept: List::with_capacity(0),
            with: None,
        },
        &mut out,
    );
    for event in [
        E::StartUnlinked { kept: List::with_capacity(0) },
        E::SignIn { with: jig_client::With::Transport },
        E::Up { up: Up::Served { served: true, versions: None } },
    ] {
        temper_client_domain::step(&mut root, &env, Event::Client { event }, &mut out);
    }
    while let Some(request) = out.pop() {
        let up = match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(subject) },
                    },
            } => {
                let value = match subject {
                    c::Subject::Me => Some(c::Delivery::Me(Box::new(fixture_me(b"Shop")))),
                    c::Subject::Task { task: TASK } => Some(c::Delivery::Task(Box::new(document.clone()))),
                    c::Subject::Tree { .. } => tree.clone(),
                    c::Subject::Task { .. }
                    | c::Subject::Chips { .. }
                    | c::Subject::Inbox { .. }
                    | c::Subject::Tasks { .. }
                    | c::Subject::Run { .. }
                    | c::Subject::Deployment
                    | c::Subject::Settings
                    | c::Subject::Accounts => None,
                };
                let Some(value) = value else { continue };
                Some(Up::Watched { watch, event: c::WatchEvent::Delivered(value) })
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Activity { .. }) },
                    },
            } => Some(Up::ReadDone {
                read,
                result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(Box::new(page.clone())))),
            }),
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Task { task: TASK }) },
                    },
            } => Some(Up::ReadDone {
                read,
                result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(document.clone())))),
            }),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => None,
        };
        if let Some(up) = up {
            temper_client_domain::step(&mut root, &env, Event::Client { event: E::Up { up } }, &mut out);
        }
    }
    root
}
pub fn activity(conversation: bool) -> c::ActivityPage {
    let mut rows = List::with_capacity(1);
    if conversation {
        let mut blocks = List::with_capacity(2);
        blocks
            .push(c::Block::Text(c::Text::new(Box::from(*b"Checking the credentials."), 32).expect("turn text")))
            .expect("text");
        blocks
            .push(c::Block::Call(c::Call {
                number: 1,
                tool: c::Text::new(Box::from(*b"delegate"), 32).expect("tool"),
                input: c::Text::new(Box::from(*b"Review the change"), 32).expect("input"),
                outcome: Some(c::Text::new(Box::from(*b"Delegated tasks: 17"), 32).expect("outcome")),
                failed: false,
                objects: {
                    let mut objects = List::with_capacity(1);
                    objects.push(c::ObjectKey::Task(17)).expect("delegate");
                    objects
                },
                parts: List::with_capacity(0),
            }))
            .expect("call");
        let mut messages = List::with_capacity(1);
        messages.push(c::Message { speaker: c::Speaker::Assistant, blocks }).expect("message");
        rows.push(c::ActivityLine {
            position: 1,
            at: RUN_SINCE,
            event: c::Activity::Turn {
                name: c::TurnName { attempt: RUN_ATTEMPT, number: 1 },
                turn: c::TurnRead::Decoded(c::Turn {
                    messages,
                    used: c::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
                    spent: 0,
                    read_through: None,
                    cut: false,
                }),
            },
        })
        .expect("turn");
    }
    c::ActivityPage { rows, before: None, after: None, cut: false }
}

pub fn added_dependency(number: u64, by: c::OwnerRequester, keeps: bool) -> c::Dependency {
    c::Dependency { number, pending: keeps, added: Some(c::DependencyAdded { at: RUN_SINCE, by }), keeps, status: None }
}
pub fn amended_dependency_facts() -> List<c::Dependency> {
    let added_by_viewer = added_dependency(17, c::OwnerRequester::Person(73), true);
    let added_by_task = added_dependency(23, c::OwnerRequester::Task(7), false);
    let original = c::Dependency { number: 13, pending: true, added: None, keeps: true, status: None };
    let mut facts = List::with_capacity(3);
    for fact in [added_by_task, original, added_by_viewer] {
        facts.push(fact).expect("amended dependency facts");
    }
    facts
}

/// Supply the live tree before the document, then optionally answer its ended read.
pub fn root_with_ended(
    document: c::Task,
    tree: c::Delivery,
    page: Option<c::TaskPage>,
) -> temper_client_domain::Domain {
    root_with_optional_tree_and_ended(document, Some(tree), page)
}

/// Optionally deliver the tree before the document, then answer its ended read.
pub fn root_with_optional_tree_and_ended(
    document: c::Task,
    tree: Option<c::Delivery>,
    page: Option<c::TaskPage>,
) -> temper_client_domain::Domain {
    root_with_ended_and_held(document, tree, page, None)
}
/// As above, also answering a held delegate's card read before the ended page.
pub fn root_with_ended_and_held(
    document: c::Task,
    tree: Option<c::Delivery>,
    page: Option<c::TaskPage>,
    held: Option<c::Task>,
) -> temper_client_domain::Domain {
    use jig_client::{Event as E, Up};
    use temper_client_domain::{Event, Request};
    let limits = domain_limits();
    let (mut root, _, watch, tree_watch) = root_at_with_tree(
        temper_client_domain::Address::Task { task: document.chip.task, filter: temper_client_domain::PlanFilter::All },
        &limits,
    );
    let env = Env { now: NO_TIME, wall: AT_EPOCH, limits };
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    for (watch, value) in [
        (tree_watch.expect("tree watch"), tree),
        (watch.expect("task watch"), Some(c::Delivery::Task(Box::new(document)))),
    ] {
        let Some(value) = value else { continue };
        temper_client_domain::step(
            &mut root,
            &env,
            Event::Client { event: E::Up { up: Up::Watched { watch, event: c::WatchEvent::Delivered(value) } } },
            &mut out,
        );
    }
    if let Some(page) = page {
        let mut token = None;
        let mut held_token = None;
        while let Some(request) = out.pop() {
            if let Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Tasks { .. }) },
                    },
            } = request
            {
                assert!(token.is_none(), "one ended read");
                token = Some(read);
            } else if let Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Task { task }) },
                    },
            } = request
                && let Some(held) = &held
                && held.chip.task == task
            {
                held_token = Some(read);
            }
        }
        if let Some(held) = held {
            root_event(
                &mut root,
                Event::Client {
                    event: E::Up {
                        up: Up::ReadDone {
                            read: held_token.expect("held card read"),
                            result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(held)))),
                        },
                    },
                },
            );
        }
        temper_client_domain::step(
            &mut root,
            &env,
            Event::Client {
                event: E::Up {
                    up: Up::ReadDone {
                        read: token.expect("ended read"),
                        result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(Box::new(page)))),
                    },
                },
            },
            &mut out,
        );
    }
    root
}
