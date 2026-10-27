//! Worker channels in the hub's link vocabulary (jig's `domain/hosts.md`, 2,
//! 6.4 and 6.5; `domain/engine.md`, 11; `testing/jig-test-domain/src/link.rs`).
//! Payloads stay owned here until the fleet asks for or drops them. The
//! protocol layer alone reads an ended answer's outcome and encodes typed
//! assignments.
use crate::boundary::{Link, Parts, Payload, Request, Work, WorkerRequest};
use crate::domain::Domain;
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, List, Token, Writer};
use temper_engine_domain_forge as forge;
impl Domain {
    /// The unique resource pushed under this fenced answer's authentic writer claim.
    #[must_use]
    pub fn change_resource(&self, payload: Token) -> Option<(u16, u16, u64)> {
        let Some(Some(Payload::Answer { task, attempt, parts, .. })) =
            self.payloads.get(skein_lib::Id::from_token(payload))
        else {
            return None;
        };
        if !self.core.current_proof(*task, *attempt) || !parts_within(parts, &self.limits) {
            return None;
        }
        let tasks::Contract::Change { connector, kind, .. } = self.core.tasks.task(*task)?.contract else {
            return None;
        };
        let heads = self.forge.reported_heads(&self.limits.forge, *task, *attempt, &parts.pushed);
        (connector == self.config.forge_connector).then_some((connector, kind, settled_resource(&heads)?))
    }
}

pub(crate) fn settled_resource(heads: &[(forge::Name, skein_forge::api::Commit)]) -> Option<u64> {
    let [(name, _)] = heads else { return None };
    Some(u64::from(name.repository))
}

/// Translate one channel input into the core's vocabulary. Unkept live pieces
/// are dropped and counted (jig's `domain/engine.md`, 11). Other records return
/// `None` when the root has no room for their bytes, which admission
/// ([`crate::try_step`]) hands back first, and their host sends again.
pub(crate) fn receive(domain: &mut Domain, channel: Token, link: Link) -> Option<core::Event> {
    Some(core::Event::Fleet(match link {
        Link::Hello { worker, hello } => fleet::Event::Hello { channel, worker, hello: self::hello(hello) },
        Link::Up { up: host::LinkUp::Piece { run, attempt, piece }, .. } => {
            if let Ok(offset) = u32::try_from(piece.offset) {
                match piece.kind {
                    host::LiveKind::Text => {
                        return Some(core::Event::RunText {
                            run,
                            attempt,
                            turn: piece.turn,
                            retry: piece.retry,
                            offset,
                            text: piece.bytes,
                            call: None,
                        });
                    }
                    host::LiveKind::Input { call, tool } => {
                        return Some(core::Event::RunInput {
                            run,
                            attempt,
                            turn: piece.turn,
                            retry: piece.retry,
                            offset,
                            id: call,
                            tool,
                            input: piece.bytes,
                        });
                    }
                    host::LiveKind::Reasoning => {}
                }
            }
            domain.pieces_dropped = domain.pieces_dropped.saturating_add(1);
            return None;
        }
        Link::Up { up, parts } => self::up(domain, channel, up, parts)?,
        Link::Lost => fleet::Event::Lost { channel },
    }))
}

/// Whether the root can keep one more worker record's bytes.
pub(crate) fn room(domain: &Domain) -> bool {
    domain.payloads.len() < domain.payloads.capacity()
}

/// temper's parts within their bounds: the heads a run pushed and the
/// resources it saved (domain/forge.md). Beyond them the answer is invalid.
fn parts_within(parts: &Parts, limits: &crate::Limits) -> bool {
    let saved = match &parts.saved {
        Some(tags) => tags.len() <= usize::try_from(limits.tasks.saved_resources).expect("u32 fits usize"),
        None => true,
    };
    saved && parts.pushed.len() <= usize::try_from(limits.forge.resources_per_task).expect("u32 fits usize")
}

fn hello(hello: host::Hello) -> fleet::Hello {
    let mut hosting = List::with_capacity(u32::try_from(hello.hosting.len()).expect("bounded hosted runs"));
    for run in hello.hosting {
        hosting
            .push(fleet::Hosted { run: run.run, attempt: run.attempt, phase: phase(run.phase) })
            .expect("one slot per hosted run");
    }
    fleet::Hello {
        slots: hello.slots,
        stop_bound: hello.stop_bound,
        workstreams: hello.workstreams,
        hosting: hosting.into_boxed(),
        bounds: hello_bounds(hello.bounds),
    }
}

/// The worker's advertised agent bounds in the fleet's hello vocabulary.
#[must_use]
pub fn hello_bounds(bounds: host::Bounds) -> fleet::Bounds {
    let mut models = List::with_capacity(u32::try_from(bounds.models.len()).expect("bounded models"));
    for model in bounds.models {
        models
            .push(fleet::ModelBound { name: model.name, window: model.window, output: model.output })
            .expect("one slot per model");
    }
    fleet::Bounds {
        program_endpoints: bounds.program_endpoints,
        models: models.into_boxed(),
        turns: bounds.turns,
        spend: bounds.spend,
        time: bounds.time,
        waiting: bounds.waiting,
        checks: bounds.checks,
        message_bytes: bounds.message_bytes,
        inbox: bounds.inbox,
        unacknowledged: bounds.unacknowledged,
    }
}

fn phase(phase: host::Phase) -> fleet::Phase {
    match phase {
        host::Phase::Preparing => fleet::Phase::Preparing,
        host::Phase::Starting => fleet::Phase::Starting,
        host::Phase::Active => fleet::Phase::Active,
        host::Phase::Waiting => fleet::Phase::Waiting,
        host::Phase::Ending => fleet::Phase::Ending,
        host::Phase::Answered => fleet::Phase::Answered,
    }
}

/// Retain a worker's bytes and temper's parts; translate only their metadata.
fn up(domain: &mut Domain, channel: Token, up: host::LinkUp, parts: Parts) -> Option<fleet::Event> {
    Some(match up {
        host::LinkUp::Piece { .. } => unreachable!("live pieces route directly to the core in receive"),
        host::LinkUp::Logins { logins } => {
            let mut rows = List::with_capacity(u32::try_from(logins.len()).expect("bounded source rows"));
            for login in logins {
                rows.push(fleet::Login { name: source_name(login.name), state: source_login(login.state) })
                    .expect("one source row per input");
            }
            fleet::Event::Logins { channel, logins: rows.into_boxed() }
        }
        host::LinkUp::SignedOut { name, result } => fleet::Event::SignedOut {
            channel,
            name: source_name(name),
            result: match result {
                host::SignOutResult::SignedOut => fleet::SignOutResult::SignedOut,
                host::SignOutResult::Failed { reason, detail } => {
                    fleet::SignOutResult::Failed { reason: source_failure(reason), detail }
                }
            },
        },

        host::LinkUp::Turn { run, attempt, turn } => {
            let number = turn.turn;
            let body = payload(
                domain,
                Payload::Turn {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    turn: number,
                    cumulative: turn.spent,
                    read: read_name(turn.read),
                    transcript: turn.body,
                    opens_window: turn.opens_window,
                    refused: refusals(&turn.refused),
                },
            )?;
            fleet::Event::Turn { channel, run, attempt, turn: number, body }
        }
        host::LinkUp::Answer { run, attempt, answer } => {
            // An answer whose parts exceed their bounds is the run's invalid
            // failure, whatever it says (`answer` below).
            let kind = if parts_within(&parts, &domain.limits) {
                match answer.ending {
                    host::Ending::Ended { .. } => fleet::Answer::Ended,
                    host::Ending::Parked { .. } => fleet::Answer::Parked,
                    host::Ending::Failed { failure, .. } => fleet::Answer::Failed { turned: turned(failure) },
                    host::Ending::Refused(host::Refusal::Busy) => fleet::Answer::Busy,
                    host::Ending::Refused(host::Refusal::Invalid(_)) => fleet::Answer::Invalid,
                }
            } else {
                fleet::Answer::Failed { turned: None }
            };
            let payload = payload(
                domain,
                Payload::Answer {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    cumulative: answer.spent,
                    read: read_name(answer.read),
                    refused: refusals(&answer.refused),
                    ending: answer.ending,
                    parts,
                },
            )?;
            fleet::Event::Answer { channel, run, attempt, answer: kind, payload }
        }
        host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline } => fleet::Event::Relay {
            channel,
            run,
            attempt,
            call: fleet::Call { name: call, tool, writes, input, deadline },
        },
        host::LinkUp::Unauthorized { run, attempt, account, generation } => {
            fleet::Event::Unauthorized { channel, run, attempt, account, generation }
        }
        host::LinkUp::Limited { run, attempt, account, reset } => {
            fleet::Event::Limited { channel, run, attempt, account, reset }
        }
        host::LinkUp::AccountRefused { run, attempt, account, code, reason } => {
            fleet::Event::AccountRefused { channel, run, attempt, account, code, reason }
        }
        host::LinkUp::Activity { run, attempt, activity } => {
            fleet::Event::Observe { channel, run, attempt, report: report(activity) }
        }
    })
}

/// The account a failed run turned on, as the fleet carries it.
const fn turned(failure: host::Failure) -> Option<fleet::Turned> {
    match failure {
        host::Failure::Run(host::RunFailure::Limited { account, reset }) => {
            Some(fleet::Turned::Limited { account, reset })
        }
        host::Failure::Run(host::RunFailure::Refused { account }) => Some(fleet::Turned::Refused { account }),
        host::Failure::Run(
            host::RunFailure::Model
            | host::RunFailure::Exceeded(_)
            | host::RunFailure::Budget
            | host::RunFailure::Policy
            | host::RunFailure::Cancelled
            | host::RunFailure::Stale,
        )
        | host::Failure::Unprepared(_)
        | host::Failure::Agent(_)
        | host::Failure::Cancelled(_) => None,
    }
}

fn payload(domain: &mut Domain, value: Payload) -> Option<Token> {
    Some(domain.payloads.insert(Some(value)).ok()?.token())
}

fn read_name(name: Option<Token>) -> Option<u64> {
    Some(name?.raw())
}

fn refusals(values: &[host::Refused]) -> Box<[u64]> {
    let mut names = List::with_capacity(u32::try_from(values.len()).expect("bounded refusals"));
    for value in values {
        names.push(value.name.raw()).expect("one slot per refusal");
    }
    names.into_boxed()
}

/// The fleet fenced an answer: an ended one's outcome goes to the protocol
/// layer to read; any other ending is translated here.
pub(crate) fn answer(domain: &mut Domain, run: Token, attempt: Token, payload: Token) {
    let end = match domain.payloads.get(skein_lib::Id::from_token(payload)) {
        Some(Some(Payload::Answer { task, attempt: retained, ending, parts, .. })) => {
            assert!(*task == run.raw() && *retained == attempt.raw(), "fenced answer identity");
            if parts_within(parts, &domain.limits) {
                match ending {
                    host::Ending::Ended { outcome, .. } => {
                        let request = Request::Worker(WorkerRequest::ReadResult { payload, outcome: outcome.clone() });
                        assert!(crate::domain::now(domain, request), "result read door reserved");
                        return;
                    }
                    other @ (host::Ending::Refused(_) | host::Ending::Parked { .. } | host::Ending::Failed { .. }) => {
                        end(other)
                    }
                }
            } else {
                tasks::End::Failed(tasks::Class::Invalid)
            }
        }
        Some(
            Some(
                Payload::Busy
                | Payload::Call { .. }
                | Payload::SettledCall(_)
                | Payload::RefusedCall { .. }
                | Payload::InboxWord(_)
                | Payload::Turn { .. },
            )
            | None,
        )
        | None => unreachable!("the fleet retained the answer body"),
    };
    let event = result_event(domain, payload, end).expect("retained answer");
    domain.work.push(Work::Core(event));
}

/// The answer's task terminal, with temper's saved work translated and what
/// it left on the forge kept for a lost run (domain/forge.md).
pub(crate) fn result_event(domain: &mut Domain, payload: Token, end: tasks::End) -> Option<core::Event> {
    let (task, attempt, cumulative, read, refused, parts) =
        match domain.payloads.get(skein_lib::Id::from_token(payload)) {
            Some(Some(Payload::Answer { task, attempt, cumulative, read, refused, parts, .. })) => {
                (*task, *attempt, *cumulative, *read, refused.clone(), parts.clone())
            }
            Some(
                Some(
                    Payload::Busy
                    | Payload::Call { .. }
                    | Payload::SettledCall(_)
                    | Payload::RefusedCall { .. }
                    | Payload::InboxWord(_)
                    | Payload::Turn { .. },
                )
                | None,
            )
            | None => return None,
        };
    let Parts { saved, pushed, preparation } = if parts_within(&parts, &domain.limits) { parts } else { Parts::none() };
    if let Some(preparation) = preparation
        && domain.facts.try_push(crate::Fact::PreparationFailed { task, attempt, preparation }).is_err()
    {
        domain.facts_lost = domain.facts_lost.saturating_add(1);
    }
    drop(domain.forge_left.insert((task, attempt), pushed));
    let (saved, invalid_saved) = match saved {
        Some(tags) => match crate::route::forge_route::saved_resources(domain.config.forge_connector, &tags) {
            Some(resources) => (Some(resources), false),
            None => (None, true),
        },
        None => (None, false),
    };
    Some(core::Event::AnswerPayload {
        run: Token::new(task),
        attempt: Token::new(attempt),
        payload,
        cumulative,
        end,
        saved,
        invalid_saved,
        read,
        refused,
    })
}

fn end(ending: &host::Ending) -> tasks::End {
    match ending {
        host::Ending::Refused(host::Refusal::Busy) => tasks::End::Refused,
        host::Ending::Refused(host::Refusal::Invalid(_)) => tasks::End::Failed(tasks::Class::Invalid),
        host::Ending::Parked { .. } => tasks::End::Parked,
        host::Ending::Failed { failure: host::Failure::Run(host::RunFailure::Exceeded(exceeded)), .. } => {
            let bound = match exceeded.bound {
                host::Bound::Reasoning => tasks::Bound::Reasoning,
                host::Bound::Metadata => tasks::Bound::Metadata,
                host::Bound::Items => tasks::Bound::Items,
                host::Bound::Answer => tasks::Bound::Answer,
                host::Bound::Tools => tasks::Bound::Tools,
                host::Bound::HistoryItems => tasks::Bound::HistoryItems,
                host::Bound::History => tasks::Bound::History,
                host::Bound::Window => tasks::Bound::Window,
                host::Bound::Output => tasks::Bound::Output,
            };
            tasks::End::Exceeded(tasks::Exceeded { bound, value: exceeded.value })
        }
        host::Ending::Failed { failure, .. } => tasks::End::Failed(match failure {
            host::Failure::Unprepared(host::Preparation::Transient) => tasks::Class::Transient,
            host::Failure::Unprepared(host::Preparation::Permanent { .. }) => tasks::Class::Permanent,
            host::Failure::Run(_) => tasks::Class::Run,
            host::Failure::Agent(_) | host::Failure::Cancelled(_) => tasks::Class::Agent,
        }),
        host::Ending::Ended { .. } => unreachable!("ended payloads leave for the protocol routing"),
    }
}

fn report(activity: host::Activity) -> fleet::Report {
    match activity {
        host::Activity::Phase(phase) => fleet::Report::Activity(match phase {
            host::Phase::Preparing => fleet::Activity::Preparing,
            host::Phase::Starting => fleet::Activity::Starting,
            host::Phase::Active => fleet::Activity::Active,
            host::Phase::Waiting => fleet::Activity::Waiting,
            host::Phase::Ending | host::Phase::Answered => fleet::Activity::Ending,
        }),
        host::Activity::Long { operation, span } => fleet::Report::Activity(fleet::Activity::Long {
            operation: match operation {
                host::Operation::Checks => fleet::Operation::Checks,
                host::Operation::Completion => fleet::Operation::Completion,
                host::Operation::Command => fleet::Operation::Command,
            },
            span,
        }),
        host::Activity::LongDone => fleet::Report::Activity(fleet::Activity::Active),
        host::Activity::Count { kind, elapsed, count } => fleet::Report::Count {
            kind: match kind {
                host::CountKind::Admitted => fleet::CountKind::Admitted,
                host::CountKind::Ended => fleet::CountKind::Ended,
                host::CountKind::LlmStarted => fleet::CountKind::LlmStarted,
                host::CountKind::LlmRetried => fleet::CountKind::LlmRetried,
                host::CountKind::LlmFinished => fleet::CountKind::LlmFinished,
                host::CountKind::ToolStarted => fleet::CountKind::ToolStarted,
                host::CountKind::ToolFinished => fleet::CountKind::ToolFinished,
                host::CountKind::CheckStarted => fleet::CountKind::CheckStarted,
                host::CountKind::CheckFinished => fleet::CountKind::CheckFinished,
                host::CountKind::TextArrived => fleet::CountKind::TextArrived,
            },
            elapsed,
            count,
        },
    }
}

/// One committed inbox word for its live host, its kind kept in its rendered line.
pub(crate) fn message(run: Token, attempt: Token, word: &tasks::Word) -> host::LinkDown {
    host::LinkDown::Message {
        run,
        attempt,
        name: Token::new(word.number),
        sender: inbox_sender(word),
        words: inbox_words(word),
        finish_requested: word.finish_requested(),
    }
}

/// Preserve the task-owned message kind when translating its opaque words,
/// for a live run's message and a starting run's inbox alike.
#[must_use]
pub fn inbox_words(word: &tasks::Word) -> Box<[u8]> {
    if let tasks::MessageKind::SettingsDecision { outcome, .. } = word.kind {
        return settings_decision_words(outcome, &word.words);
    }
    let (label, number) = match word.kind {
        tasks::MessageKind::SettingsDecision { .. } => unreachable!("settings decision rendered above"),
        tasks::MessageKind::Result(_) => (
            // The client's chat cards read this line (temper-client-domain pages/delegation.rs `results`).
            b"Result from task ".as_slice(),
            match word.from {
                tasks::Party::Task(task) => task,
                tasks::Party::Person(person) => person,
                tasks::Party::Deployment { .. } => 0,
            },
        ),
        tasks::MessageKind::ProposalDecision { proposal, accepted } => {
            (if accepted { b"Proposal accepted ".as_slice() } else { b"Proposal rejected ".as_slice() }, proposal)
        }
        tasks::MessageKind::PersonStalled { task } => (b"Person task past its waiting deadline ".as_slice(), task),
        tasks::MessageKind::EffectNotMade { entry } => (b"Effect not made ".as_slice(), entry),
        tasks::MessageKind::Met { dependency, removed } => return met_words(dependency, removed, &word.words),
        tasks::MessageKind::Words
        | tasks::MessageKind::Question
        | tasks::MessageKind::Answer { .. }
        | tasks::MessageKind::Notice { .. }
        | tasks::MessageKind::News { .. }
        | tasks::MessageKind::Timer { .. }
        | tasks::MessageKind::FinishRequested { .. }
        | tasks::MessageKind::Amendment { .. }
        | tasks::MessageKind::Proposal { .. }
        | tasks::MessageKind::Escalation { .. } => return word.words.clone(),
    };
    let number = Decimal::of(number);
    let bytes = label.len().checked_add(number.as_bytes().len()).expect("bounded message label");
    let bytes = bytes.checked_add(2).expect("bounded message separator");
    let bytes = bytes.checked_add(word.words.len()).expect("bounded message envelope");
    let mut writer = Writer::new(bytes);
    writer.put(label).expect("measured label");
    writer.put(number.as_bytes()).expect("measured source");
    writer.put(b":\n").expect("measured separator");
    writer.put(&word.words).expect("measured words");
    writer.finish()
}

fn met_words(dependency: u64, removed: bool, words: &[u8]) -> Box<[u8]> {
    let dependency = Decimal::of(dependency);
    let line = if removed {
        b" was removed from its dependencies; the task can go on".as_slice()
    } else {
        b" is done; the task can go on".as_slice()
    };
    let separator = if words.is_empty() { b"".as_slice() } else { b":\n".as_slice() };
    let bytes = 1_usize.checked_add(dependency.as_bytes().len()).expect("bounded dependency label");
    let bytes = bytes.checked_add(line.len()).expect("bounded dependency notice");
    let bytes = bytes.checked_add(separator.len()).expect("bounded message separator");
    let bytes = bytes.checked_add(words.len()).expect("bounded message envelope");
    let mut writer = Writer::new(bytes);
    writer.put(b"T").expect("measured label");
    writer.put(dependency.as_bytes()).expect("measured dependency");
    writer.put(line).expect("measured notice");
    writer.put(separator).expect("measured separator");
    writer.put(words).expect("measured words");
    writer.finish()
}

/// The sender label and question reference that smith prefixes to the words.
/// Shared by live delivery and the inbox carried at start (domain/agent.md, 5).
#[must_use]
pub fn inbox_sender(word: &tasks::Word) -> Box<[u8]> {
    let label = sender(word.from);
    let (kind, question, end) = match word.kind {
        tasks::MessageKind::Question => (b" asks (question ".as_slice(), word.number, b")".as_slice()),
        tasks::MessageKind::Answer { question } => (b" answers question ".as_slice(), question, b"".as_slice()),
        tasks::MessageKind::Words
        | tasks::MessageKind::Result(_)
        | tasks::MessageKind::ProposalDecision { .. }
        | tasks::MessageKind::SettingsDecision { .. }
        | tasks::MessageKind::PersonStalled { .. }
        | tasks::MessageKind::EffectNotMade { .. }
        | tasks::MessageKind::Met { .. }
        | tasks::MessageKind::Notice { .. }
        | tasks::MessageKind::News { .. }
        | tasks::MessageKind::Timer { .. }
        | tasks::MessageKind::FinishRequested { .. }
        | tasks::MessageKind::Amendment { .. }
        | tasks::MessageKind::Proposal { .. }
        | tasks::MessageKind::Escalation { .. } => return label,
    };
    let question = Decimal::of(question);
    let bytes = label.len().checked_add(kind.len()).expect("bounded message kind");
    let bytes = bytes.checked_add(question.as_bytes().len()).expect("bounded question number");
    let bytes = bytes.checked_add(end.len()).expect("bounded message label");
    let mut writer = Writer::new(bytes);
    writer.put(&label).expect("measured sender");
    writer.put(kind).expect("measured kind");
    writer.put(question.as_bytes()).expect("measured question");
    writer.put(end).expect("measured end");
    writer.finish()
}

fn sender(value: tasks::Party) -> Box<[u8]> {
    match value {
        tasks::Party::Task(number) | tasks::Party::Person(number) => Box::from(Decimal::of(number).as_bytes()),
        tasks::Party::Deployment { .. } => Box::from(&b"deployment"[..]),
    }
}

fn source_name(name: host::SourceName) -> fleet::SourceName {
    fleet::SourceName::new(name.as_bytes()).expect("same source name grammar")
}

fn source_login(state: host::LoginState) -> fleet::LoginState {
    match state {
        host::LoginState::Usable => fleet::LoginState::Usable,
        host::LoginState::NotSignedIn => fleet::LoginState::NotSignedIn,
        host::LoginState::SignedInOtherwise(method) => fleet::LoginState::SignedInOtherwise(
            fleet::LoginMethod::new(method.as_bytes()).expect("same printable method grammar"),
        ),
        host::LoginState::Failed { reason, detail } => {
            fleet::LoginState::Failed { reason: source_failure(reason), detail }
        }
    }
}
const fn source_failure(reason: host::SourceFailure) -> fleet::SourceFailure {
    match reason {
        host::SourceFailure::Missing => fleet::SourceFailure::Missing,
        host::SourceFailure::Deadline => fleet::SourceFailure::Deadline,
        host::SourceFailure::Unreadable => fleet::SourceFailure::Unreadable,
        host::SourceFailure::Exited => fleet::SourceFailure::Exited,
        host::SourceFailure::Unconfigured => fleet::SourceFailure::Unconfigured,
        host::SourceFailure::Unknown => fleet::SourceFailure::Unknown,
        host::SourceFailure::Closed => fleet::SourceFailure::Closed,
    }
}

fn settings_decision_words(outcome: tasks::SettingsOutcome, reason: &[u8]) -> Box<[u8]> {
    let number = Decimal::of(match outcome {
        tasks::SettingsOutcome::Accepted { revision } => u64::from(revision),
        tasks::SettingsOutcome::Stale { newest } => u64::from(newest),
        tasks::SettingsOutcome::Rejected => 0,
    });
    let parts: [&[u8]; 3] = match outcome {
        tasks::SettingsOutcome::Accepted { .. } => [b"accepted: revision ", number.as_bytes(), b" made"],
        tasks::SettingsOutcome::Rejected => [b"rejected: ", reason, b""],
        tasks::SettingsOutcome::Stale { .. } => [b"withdrawn as stale: the newest is ", number.as_bytes(), b""],
    };
    let mut size = 0_usize;
    for part in parts {
        size = size.checked_add(part.len()).expect("bounded inbox words");
    }
    let mut out = Writer::new(size);
    for part in parts {
        out.put(part).expect("measured inbox words");
    }
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_lib::Duration;

    #[test]
    fn text_pieces_reach_the_core_with_their_identity_and_offsets() {
        let (mut domain, _) = crate::tests::resume_fixture::root();
        let run = Token::new(31);
        let attempt = Token::new(932);
        for offset in [0, u32::MAX] {
            let event = receive(
                &mut domain,
                Token::new(7),
                Link::Up {
                    up: host::LinkUp::Piece {
                        run,
                        attempt,
                        piece: host::Piece {
                            turn: 3,
                            retry: 2,
                            kind: host::LiveKind::Text,
                            offset: u64::from(offset),
                            bytes: Box::from(&b"words"[..]),
                        },
                    },
                    parts: Parts::none(),
                },
            );
            let Some(core::Event::RunText {
                run: actual_run,
                attempt: actual_attempt,
                turn,
                retry,
                offset: actual_offset,
                text,
                call,
            }) = event
            else {
                panic!("text reaches core")
            };
            assert_eq!((actual_run, actual_attempt, turn, retry, actual_offset), (run, attempt, 3, 2, offset));
            assert_eq!(text.as_ref(), b"words");
            assert_eq!(call, None);
        }
        assert_eq!(domain.pieces_dropped(), 0);
    }

    #[test]
    fn input_pieces_reach_the_core_with_their_provider_identity_and_offsets() {
        let (mut domain, _) = crate::tests::resume_fixture::root();
        let run = Token::new(31);
        let attempt = Token::new(932);
        for offset in [0, u32::MAX] {
            let event = receive(
                &mut domain,
                Token::new(7),
                Link::Up {
                    up: host::LinkUp::Piece {
                        run,
                        attempt,
                        piece: host::Piece {
                            turn: 3,
                            retry: 2,
                            kind: host::LiveKind::Input {
                                call: Box::from(b"call".as_slice()),
                                tool: Box::from(b"tool".as_slice()),
                            },
                            offset: u64::from(offset),
                            bytes: Box::from(b"{}".as_slice()),
                        },
                    },
                    parts: Parts::none(),
                },
            );
            let Some(core::Event::RunInput {
                run: actual_run,
                attempt: actual_attempt,
                turn,
                retry,
                offset: actual_offset,
                id,
                tool,
                input,
            }) = event
            else {
                panic!("input reaches core")
            };
            assert_eq!((actual_run, actual_attempt, turn, retry, actual_offset), (run, attempt, 3, 2, offset));
            assert_eq!(
                (id.as_ref(), tool.as_ref(), input.as_ref()),
                (b"call".as_slice(), b"tool".as_slice(), b"{}".as_slice())
            );
        }
        assert_eq!(domain.pieces_dropped(), 0);
    }

    #[test]
    fn unkept_piece_kinds_and_offsets_are_dropped_and_counted() {
        let (mut domain, _) = crate::tests::resume_fixture::root();
        for (index, (kind, offset)) in [
            (host::LiveKind::Reasoning, 0),
            (
                host::LiveKind::Input { call: Box::from(&b"call"[..]), tool: Box::from(&b"tool"[..]) },
                u64::from(u32::MAX) + 1,
            ),
            (host::LiveKind::Text, u64::from(u32::MAX) + 1),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                receive(
                    &mut domain,
                    Token::new(7),
                    Link::Up {
                        up: host::LinkUp::Piece {
                            run: Token::new(31),
                            attempt: Token::new(932),
                            piece: host::Piece { turn: 3, retry: 2, kind, offset, bytes: Box::from(&b"words"[..]) }
                        },
                        parts: Parts::none(),
                    }
                )
                .is_none()
            );
            assert_eq!(domain.pieces_dropped(), u64::try_from(index + 1).expect("three cases"));
        }
        domain.pieces_dropped = u64::MAX;
        assert!(
            receive(
                &mut domain,
                Token::new(7),
                Link::Up {
                    up: host::LinkUp::Piece {
                        run: Token::new(31),
                        attempt: Token::new(932),
                        piece: host::Piece {
                            turn: 3,
                            retry: 2,
                            kind: host::LiveKind::Reasoning,
                            offset: 0,
                            bytes: Box::new([]),
                        }
                    },
                    parts: Parts::none(),
                }
            )
            .is_none()
        );
        assert_eq!(domain.pieces_dropped(), u64::MAX);
    }

    fn rendered_line(from: tasks::Party, kind: tasks::MessageKind) -> Box<[u8]> {
        let word = tasks::Word {
            number: 4,
            from,
            kind,
            words: b"Go ahead?\nKeep the newline.".as_slice().into(),
            at: skein_lib::Wall::EPOCH,
            hits: 1,
            eligible: true,
        };
        let host::LinkDown::Message { sender, words, name, .. } = message(Token::new(1), Token::new(2), &word) else {
            panic!("message translation")
        };
        assert_eq!(name, Token::new(4));
        let bytes = sender.len().checked_add(2).expect("test sender and separator");
        let bytes = bytes.checked_add(words.len()).expect("test line");
        let mut line = Writer::new(bytes);
        line.put(&sender).expect("sender fits");
        line.put(b": ").expect("separator fits");
        line.put(&words).expect("words fit");
        line.finish()
    }

    #[test]
    fn words_render_with_their_sender() {
        assert_eq!(
            rendered_line(tasks::Party::Task(2), tasks::MessageKind::Words).as_ref(),
            b"2: Go ahead?\nKeep the newline."
        );
        assert_eq!(
            rendered_line(tasks::Party::Person(1), tasks::MessageKind::Words).as_ref(),
            b"1: Go ahead?\nKeep the newline."
        );
    }

    #[test]
    fn a_question_renders_its_number_for_the_answer_form() {
        assert_eq!(
            rendered_line(tasks::Party::Task(2), tasks::MessageKind::Question).as_ref(),
            b"2 asks (question 4): Go ahead?\nKeep the newline."
        );
    }

    #[test]
    fn an_answer_renders_the_question_it_answers() {
        assert_eq!(
            rendered_line(tasks::Party::Person(1), tasks::MessageKind::Answer { question: 3 }).as_ref(),
            b"1 answers question 3: Go ahead?\nKeep the newline."
        );
    }

    #[test]
    fn met_notices_name_the_dependency_and_preserve_any_words() {
        let cases: &[(bool, &[u8], &[u8])] = &[
            (false, b"", b"T42 is done; the task can go on"),
            (true, b"", b"T42 was removed from its dependencies; the task can go on"),
            (false, b"landed", b"T42 is done; the task can go on:\nlanded"),
            (
                true,
                b"no longer needed",
                b"T42 was removed from its dependencies; the task can go on:\nno longer needed",
            ),
        ];
        for &(removed, words, expected) in cases {
            let word = tasks::Word {
                number: 2,
                from: tasks::Party::Task(7),
                kind: tasks::MessageKind::Met { dependency: 42, removed },
                words: words.into(),
                at: skein_lib::Wall::EPOCH,
                hits: 1,
                eligible: true,
            };
            assert_eq!(inbox_words(&word).as_ref(), expected);
        }
    }

    #[test]
    fn hello_advertises_the_public_bounds_conversion() {
        let bounds = host::Bounds {
            program_endpoints: Box::new([7]),
            models: Box::new([host::ModelBound { name: Box::from(b"model".as_slice()), window: 8192, output: 1024 }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(360),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 4096,
            inbox: 8,
            unacknowledged: 4,
        };
        let converted = hello_bounds(bounds.clone());
        let hello = hello(host::Hello {
            slots: 2,
            stop_bound: Duration::from_secs(5),
            workstreams: Box::new([11]),
            hosting: Box::new([]),
            bounds: bounds.clone(),
        });
        assert_eq!(hello.bounds, converted);
        assert_eq!(hello.bounds.program_endpoints.as_ref(), &[7]);
        assert_eq!(hello.bounds.models[0].name, bounds.models[0].name);
        assert_eq!(hello.bounds.models[0].window, bounds.models[0].window);
        assert_eq!(hello.bounds.models[0].output, bounds.models[0].output);
        assert_eq!(hello.bounds.turns, bounds.turns);
        assert_eq!(hello.bounds.spend, bounds.spend);
        assert_eq!(hello.bounds.time, bounds.time);
        assert_eq!(hello.bounds.waiting, bounds.waiting);
        assert_eq!(hello.bounds.checks, bounds.checks);
        assert_eq!(hello.bounds.message_bytes, bounds.message_bytes);
        assert_eq!(hello.bounds.inbox, bounds.inbox);
        assert_eq!(hello.bounds.unacknowledged, bounds.unacknowledged);
    }
}
