//! The core's held outputs, each released after its decision commits
//! (jig's `domain/root.md`, 4; `testing/jig-test-domain/src/routes.rs`).
use super::{Decision, Delivery, Domain, Env, Limits, Read, Token, emit, forge_route, host_route, relay_call};
use crate::Range;
use crate::assemble::take_payload;
use crate::boundary::Payload;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;

#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher releases every held core output")]
pub(crate) fn held(domain: &mut Domain, env: &Env<Limits>, decision: &mut Decision, held: core::Held) {
    match held {
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
                    | Payload::RefusedCall { .. }
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
        core::Held::NotesLoad { owner, range } => notes_load(domain, env, decision, owner, range),
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

// LoadFailed makes the core retry a pending brief immediately. Archive pressure
// must yield to a later iteration, when earlier reads can finish and reclaim.
pub(crate) fn notes_load(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision,
    owner: Token,
    range: jig_domain_notes::Range,
) {
    let delivery = match domain.archive_reads.insert(Some(Read::Notes { owner })) {
        Ok(id) => {
            Delivery::Load { waiter: id.token(), range: Range::Core(crate::CoreRange::Notes(range)), after: None }
        }
        Err(_) => Delivery::NotesLoad { owner, range },
    };
    emit(decision, &env.limits, delivery);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notes_load_waits_for_archive_room_without_a_synchronous_failure_loop() {
        let (mut domain, env) = crate::tests::resume_fixture::root();
        let mut reads = Vec::new();
        for _ in 0..env.limits.loads.loads {
            reads.push(domain.archive_reads.insert(Some(Read::Transcript { task: 1 })).unwrap());
        }
        let mut decision = Decision::new(&env.limits.journal);
        held(
            &mut domain,
            &env,
            &mut decision,
            core::Held::NotesLoad {
                owner: Token::new(1),
                range: jig_domain_notes::Range::Lines { scope: jig_domain_notes::Scope::Deployment, after: None },
            },
        );
        assert!(domain.work.is_empty(), "a full archive must not synchronously fail and retry the same notes load");
        let retry = delivery(&mut domain, &env.limits, decision);
        assert!(matches!(&retry, Delivery::NotesLoad { owner, .. } if *owner == Token::new(1)));
        for id in reads {
            domain.archive_reads.get_mut(id).unwrap().take();
            domain.archive_reads.retire(id);
        }
        domain.reclaim();
        let Delivery::NotesLoad { owner, range } = retry else { unreachable!() };
        let mut decision = Decision::new(&env.limits.journal);
        notes_load(&mut domain, &env, &mut decision, owner, range);
        let Delivery::Load { waiter, range, after } = delivery(&mut domain, &env.limits, decision) else {
            panic!("the deferred load is issued when room returns")
        };
        assert!(after.is_none());
        assert!(matches!(
            range,
            Range::Core(crate::CoreRange::Notes(jig_domain_notes::Range::Lines {
                scope: jig_domain_notes::Scope::Deployment,
                after: None
            }))
        ));
        assert!(
            matches!(domain.archive_reads.get(skein_lib::Id::from_token(waiter)), Some(Some(Read::Notes { owner })) if *owner == Token::new(1))
        );
        assert!(domain.work.is_empty());
    }
    fn delivery(domain: &mut Domain, limits: &Limits, decision: Decision) -> Delivery {
        crate::decision::accept_pending(&mut domain.journal, &mut domain.core.counters, &limits.journal, decision)
            .unwrap();
        while let Some(output) = crate::decision::commit(&mut domain.journal, &limits.journal) {
            let crate::decision::JournalOutput::Commit { number, .. } = output else { panic!("store commit") };
            crate::decision::committed(&mut domain.journal, number);
        }
        let mut out = skein_lib::Queue::with_capacity(1);
        crate::decision::resume(&mut domain.journal, &mut out);
        let Some(crate::decision::JournalOutput::Deliver(delivery)) = out.pop() else {
            panic!("one deferred delivery")
        };
        delivery
    }
}
