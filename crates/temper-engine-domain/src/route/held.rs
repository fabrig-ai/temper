//! The core's held outputs, each released after its decision commits
//! (jig's `domain/root.md`, 4; `testing/jig-test-domain/src/routes.rs`).
use super::{Decision, Delivery, Domain, Env, Limits, Read, Token, Work, emit, forge_route, host_route, relay_call};
use crate::Range;
use crate::assemble::take_payload;
use crate::boundary::Payload;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;

#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher releases every held core output")]
pub(crate) fn held(domain: &mut Domain, env: &Env<Limits>, decision: &mut Decision, held: core::Held) {
    match held {
        core::Held::ProjectionRecovery { connector, owner, result } => {
            emit(decision, &env.limits, Delivery::ProjectionRecovery { connector, owner, result });
        }
        core::Held::Sources { channel, sources } => {
            let mut names = skein_lib::List::with_capacity(u32::try_from(sources.len()).expect("bounded sources"));
            for name in sources {
                names
                    .push(host::SourceName::new(name.as_bytes()).expect("same source grammar"))
                    .expect("one name per source");
            }
            emit(
                decision,
                &env.limits,
                Delivery::Link { channel, down: host::LinkDown::Sources { sources: names.into_boxed() } },
            );
        }
        core::Held::CheckSource { channel, name } => emit(
            decision,
            &env.limits,
            Delivery::Link {
                channel,
                down: host::LinkDown::Check {
                    name: host::SourceName::new(name.as_bytes()).expect("same source grammar"),
                },
            },
        ),
        core::Held::SignOutSource { channel, name } => emit(
            decision,
            &env.limits,
            Delivery::Link {
                channel,
                down: host::LinkDown::SignOut {
                    name: host::SourceName::new(name.as_bytes()).expect("same source grammar"),
                },
            },
        ),
        core::Held::SourceSignedOut { to, channel, name, result } => {
            emit(decision, &env.limits, Delivery::SourceSignedOut { to, channel, name, result });
        }
        core::Held::Load(load) => super::now::now(domain, env, decision, *load),
        core::Held::SettledCall { to, call, .. } => {
            host_route::settled(domain, &env.limits, decision, to, call);
        }
        core::Held::Relayed { channel, run, attempt, call, answer } => {
            let delivery = host_route::relayed(domain, channel, run, attempt, call, answer);
            emit(decision, &env.limits, delivery);
        }
        core::Held::Inbound { channel, run, attempt, message } => {
            let word = match take_payload(domain, message.words) {
                Some(Payload::InboxWord(word)) => word,
                Some(
                    Payload::Busy
                    | Payload::Call { .. }
                    | Payload::SettledCall(_)
                    | Payload::Turn { .. }
                    | Payload::Answer { .. },
                )
                | None => unreachable!("fleet message payload"),
            };
            let down = crate::link::message(run, attempt, &word);
            emit(decision, &env.limits, Delivery::Link { channel, down });
        }
        core::Held::MakeEffect { connector, entry } => {
            assert!(connector == domain.config.forge_connector, "numbered forge make");
            emit(decision, &env.limits, Delivery::ForgeCommitted { entry });
        }
        core::Held::Relay { task, attempt, previous, word } => {
            emit(decision, &env.limits, Delivery::Relay { task, attempt, previous, word });
        }
        core::Held::PeopleReply { to, reply } => {
            let token = to.into_token();
            let to = skein_lib::ReplyTo::new(token);
            if super::demo::owns(token) {
                super::demo::reply(domain, env, decision, to, reply);
            } else {
                emit(decision, &env.limits, Delivery::WebReply { to, reply });
            }
        }
        core::Held::CarriedUnused { number } => emit(decision, &env.limits, Delivery::CarriedUnused { number }),
        core::Held::CallAnswer { to, key, part } => {
            let answer = forge_route::effect_answer(domain, key, &part);
            relay_call(domain, &env.limits, decision, to, answer);
        }
        core::Held::Result { person, task, words } => {
            emit(decision, &env.limits, Delivery::Result { person, task, words });
        }
        core::Held::Assign { channel, run, attempt, .. } => {
            let assignment = domain.assignments.remove(&run.raw()).expect("durable claim has prepared assignment");
            assert!(assignment.attempt == attempt.raw(), "assignment names current attempt");
            emit(decision, &env.limits, Delivery::Assigned { channel, assignment });
        }
        core::Held::Grant { channel, run, attempt, grant } => {
            let grant = host::Grant {
                form: match grant.form {
                    fleet::Form::Credential(scheme) => host::Form::Credential(match scheme {
                        fleet::Scheme::Bearer => host::Scheme::Bearer,
                        fleet::Scheme::Key => host::Scheme::Key,
                    }),
                    fleet::Form::Application => host::Form::Application,
                    fleet::Form::Program(name) => {
                        host::Form::Program(host::SourceName::new(name.as_bytes()).expect("same source name grammar"))
                    }
                },
                account: grant.account,
                generation: grant.generation,
                valid: grant.valid,
            };
            emit(
                decision,
                &env.limits,
                Delivery::Link { channel, down: host::LinkDown::Grant { run, attempt, grant } },
            );
        }
        core::Held::Acknowledge { channel, run, attempt } => {
            emit(decision, &env.limits, Delivery::Link { channel, down: host::LinkDown::Acknowledge { run, attempt } });
        }
        core::Held::TaskTerminalAcknowledged { task, attempt } => {
            emit(
                decision,
                &env.limits,
                Delivery::Fleet(fleet::Event::Acknowledge { run: Token::new(task), attempt: Token::new(attempt) }),
            );
        }
        core::Held::TaskTurnKept { task, attempt, turn } => {
            emit(
                decision,
                &env.limits,
                Delivery::Fleet(fleet::Event::TurnKept { run: Token::new(task), attempt: Token::new(attempt), turn }),
            );
        }
        core::Held::AcknowledgeTurn { channel, run, attempt, turn } => {
            emit(
                decision,
                &env.limits,
                Delivery::Link { channel, down: host::LinkDown::TurnKept { run, attempt, turn } },
            );
        }
        core::Held::Cancel { channel, run, attempt } => {
            emit(decision, &env.limits, Delivery::Link { channel, down: host::LinkDown::Cancel { run, attempt } });
        }
        core::Held::Refuse { channel } => emit(decision, &env.limits, Delivery::Refuse { channel }),
        core::Held::TurnBusy { channel, run, attempt, turn } => {
            emit(decision, &env.limits, Delivery::TurnBusy { channel, task: run.raw(), attempt: attempt.raw(), turn });
        }
        core::Held::StopRun { task, attempt } => {
            emit(decision, &env.limits, Delivery::Fleet(core::Core::stop_run(task, attempt)));
        }
        core::Held::NotesLoad { owner, range } => match domain.archive_reads.insert(Some(Read::Notes { owner })) {
            Ok(id) => emit(
                decision,
                &env.limits,
                Delivery::Load { waiter: id.token(), range: Range::Core(crate::CoreRange::Notes(range)), after: None },
            ),
            Err(_) => domain.work.push(Work::Core(core::Event::Notes(jig_domain_notes::Event::LoadFailed { owner }))),
        },
        core::Held::ReadLoad { load, range } => emit(decision, &env.limits, Delivery::ReadLoad { load, range }),
        core::Held::Read { reply, answer } => emit(decision, &env.limits, Delivery::Read { reply, answer }),
        core::Held::Delivery { watcher, document } => {
            emit(decision, &env.limits, Delivery::Document { watcher, document });
        }
        core::Held::WatchEnded { watcher, why } | core::Held::ApplicationWatchEnded { watcher, why, .. } => {
            emit(decision, &env.limits, Delivery::WatchEnded { watcher, why });
        }
        core::Held::Drained => emit(decision, &env.limits, Delivery::Drained),
        core::Held::Applied { to, token, .. } => {
            let right = to.into_token();
            let to = skein_lib::ReplyTo::new(right);
            if super::demo::owns(right) {
                let outcome = domain.forge.application_answer(token);
                super::demo::adopted(domain, env, decision, outcome);
            } else {
                emit(
                    decision,
                    &env.limits,
                    Delivery::Application { to, outcome: super::applications::answer(domain, token) },
                );
            }
        }
        core::Held::ApplicationDelivery { .. } => {
            unreachable!("application delivery is taken where it is made (panels.rs)")
        }
        core::Held::NotesWritten { .. } | core::Held::NotesDeleted { .. } => {
            unreachable!("the current application has no note caller")
        }
        core::Held::Acted { to, outcome } => emit(decision, &env.limits, Delivery::DemoActed { to, outcome }),
    }
}
