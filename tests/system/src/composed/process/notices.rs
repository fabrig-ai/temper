//! Notices scripted at the existing fake process channel, with the real roots.
use super::*;
use crate::composed::{CHANNEL, World, report_script};
use jig_domain_accounts as accounts;
use jig_domain_host as host;
use skein_lib::{Duration, Wall};

fn saves(world: &World) -> Vec<accounts::Stored> {
    world
        .store
        .rows
        .values()
        .filter_map(|record| match record {
            Record::Core(jig_domain_core::Record::Accounts(record)) => Some(record.clone()),
            Record::Core(_) | Record::Forge { .. } | Record::Delivery { .. } => None,
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Notice {
    Unauthorized,
    Limited { reset: Option<Duration> },
    Refused,
}
impl Notice {
    fn message(self) -> agent::Up {
        match self {
            Self::Unauthorized => agent::Up::Unauthorized { account: 1, generation: 1 },
            Self::Limited { reset } => agent::Up::Limited { account: 1, reset },
            Self::Refused => agent::Up::Refused {
                account: 1,
                code: b"quota".as_slice().into(),
                reason: b"provider refused".as_slice().into(),
            },
        }
    }
}

#[test]
fn composed_notices_reach_the_accounts_child_before_the_attempt_ends() {
    for notice in [
        Notice::Unauthorized,
        Notice::Limited { reset: None },
        Notice::Limited { reset: Some(Duration::ZERO) },
        Notice::Limited { reset: Some(Duration::from_secs(9)) },
        Notice::Refused,
    ] {
        let mut world = World::signed_in_chat(930, report_script(true));
        world.until_call();
        assert_eq!(world.observed.answers, 0);
        world.process.up.push_front(notice.message());
        world.run();
        match notice {
            Notice::Unauthorized => assert!(world.protocol.accounts.contains(&accounts::Request::Component(
                accounts::ToComponent::Rejected { account: 1, generation: 1 }
            ))),
            Notice::Limited { reset } => {
                if reset == Some(Duration::ZERO) {
                    assert!(
                        !saves(&world).iter().any(|record| matches!(
                            record,
                            accounts::Stored::Source { held: Some(accounts::Held::Limited { reset: None }), .. }
                        )),
                        "supplied zero is never an unknown hold"
                    );
                } else {
                    let held = saves(&world)
                        .into_iter()
                        .find_map(|record| match record {
                            accounts::Stored::Source {
                                account: 0,
                                number: 1,
                                held: Some(accounts::Held::Limited { reset }),
                                ..
                            } => Some(reset),
                            accounts::Stored::Source { .. }
                            | accounts::Stored::Issued { .. }
                            | accounts::Stored::Account { .. }
                            | accounts::Stored::Next { .. } => None,
                        })
                        .expect("the accounts child saves the source's hold");
                    assert_eq!(held.is_some(), reset.is_some(), "unknown stays unknown");
                    if let Some(reset) = reset {
                        assert!(held.expect("supplied reset") >= Wall::from_nanos(reset.as_nanos()));
                    }
                }
            }
            Notice::Refused => assert!(saves(&world).iter().any(|record| matches!(
                record,
                accounts::Stored::Source { account: 0, number: 1, held: Some(accounts::Held::Refused), .. }
            ))),
        }
        assert_eq!(world.observed.answers, 1, "notice does not replace the terminal");
        let before = world.protocol.accounts.clone();
        let assignment = world.assignment();
        let up = host::LinkUp::Unauthorized {
            run: Token::new(assignment.task),
            attempt: Token::new(assignment.attempt),
            account: 1,
            generation: 2,
        };
        world.root_step(temper_engine_domain_world::protocol::up(CHANNEL, up, engine::Parts::none()));
        assert_eq!(world.protocol.accounts, before, "ended attempt cannot report again");
    }
}

#[test]
fn composed_notice_fences_drop_old_channels_attempts_sources_and_generations() {
    for (channel, old_attempt, source, generation, renewed) in [
        (Token::new(99), false, 1, 1, false),
        (CHANNEL, true, 1, 1, false),
        (CHANNEL, false, 17, 1, false),
        (CHANNEL, false, 1, 1, true),
    ] {
        let mut world = World::signed_in_chat(931, report_script(true));
        world.until_call();
        if renewed {
            world.root_step(engine::Event::Account(accounts::Event::Component(accounts::FromComponent::Granted {
                account: 1,
                generation: 2,
                valid: Duration::from_secs(3600),
                form: accounts::Form::Credential(accounts::Scheme::Bearer),
            })));
        }
        let assignment = world.assignment();
        let up = host::LinkUp::Unauthorized {
            run: Token::new(assignment.task),
            attempt: Token::new(if old_attempt { assignment.attempt + 1 } else { assignment.attempt }),
            account: source,
            generation,
        };
        world.root_step(temper_engine_domain_world::protocol::up(channel, up, engine::Parts::none()));
        world.run();
        assert!(
            !world
                .protocol
                .accounts
                .iter()
                .any(|request| matches!(request, accounts::Request::Component(accounts::ToComponent::Rejected { .. })))
        );
        assert!(!saves(&world).iter().any(|record| matches!(record, accounts::Stored::Source { held: Some(_), .. })));
    }
}
