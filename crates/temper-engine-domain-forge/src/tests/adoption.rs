//! Core admission and the bounded retained application outcome.
use super::*;
use jig_domain_people as people;

#[test]
fn an_adoption_refuses_a_prefix_whose_branches_exceed_authority_segments_before_a_call() {
    let mut d = domain();
    let mut adoption = fixtures::adoption();
    adoption.prefix = Box::from(b"temper/live-x/".as_slice());
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(adoption),
        },
    );
    assert_eq!(
        d.application_outcome(Token::new(9)),
        Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::TooLarge))
    );
    assert!(ready(&mut d).is_empty(), "no forge read for a prefix that cannot fit");
}

#[test]
fn an_adoption_accepts_a_prefix_whose_branches_fit_authority_segments() {
    use skein_forge::api;
    let mut d = domain();
    fixtures::begin_adoption(&mut d, Token::new(4), fixtures::adoption());
    for answer in [
        api::Answer::Permission(api::Permission::Write),
        api::Answer::Branches(Box::new([])),
        api::Answer::Settings(adopted().settings),
        api::Answer::Protection(None),
    ] {
        fixtures::answer_next(&mut d, Ok(answer));
    }
    let requests =
        fixtures::answer_next(&mut d, Ok(api::Answer::Collaborators { collaborators: Box::new([]), more: false }));
    let [.., Request::Adopted { result: Ok(value), .. }] = requests.as_ref() else { panic!("adoption accepted") };
    assert_eq!(value.repository.prefix.as_ref(), b"temper/");
}

#[test]
fn an_adoption_locates_its_repository_before_any_other_read() {
    let mut d = domain();
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    let sent = ready(&mut d);
    let [Request::Call { repository, op, .. }] = sent.as_ref() else { panic!("one locate call") };
    assert_eq!(*repository, skein_forge::api::Repository { forge: 1, repository: 0 });
    assert_eq!(
        *op,
        skein_forge::api::Op::Read(skein_forge::api::Read::Locate {
            owner: Box::from(&b"org"[..]),
            name: Box::from(&b"repo"[..]),
        })
    );
}

#[test]
fn an_adoption_refuses_fields_outside_its_bounds_before_a_call() {
    for field in 0_u8..5 {
        for bytes in [&[][..], &[b'a'; 129][..]] {
            let mut d = domain();
            let mut request = fixtures::adoption();
            match field {
                0 => request.host = bytes.into(),
                1 => request.owner = bytes.into(),
                2 => request.name = bytes.into(),
                3 => request.prefix = bytes.into(),
                4 => request.landing = bytes.into(),
                _ => panic!("field"),
            }
            outputs(
                &mut d,
                Event::Application {
                    role: None,
                    carried: None,
                    request: Token::new(4),
                    token: Token::new(9),
                    retention: Duration::from_secs(60),
                    ask: Ask::Adopt(request),
                },
            );
            assert_eq!(
                d.application_outcome(Token::new(9)),
                Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::TooLarge)),
                "field {field}"
            );
            assert!(ready(&mut d).is_empty());
        }
    }
}

#[test]
fn a_core_refusal_is_kept_as_jigs_refusal() {
    let mut d = domain();
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    let requests = outputs(
        &mut d,
        Event::ApplicationAnswered {
            request: Token::new(4),
            outcome: people::Outcome::Refused(people::Refusal::Unknown),
        },
    );
    assert!(requests.contains(&Request::ApplicationDone { request: Token::new(4), token: Token::new(9) }));
    assert_eq!(d.application_outcome(Token::new(9)), Some(&ApplicationOutcome::CoreRefused(people::Refusal::Unknown)));
}

#[test]
fn a_full_outcome_table_answers_the_application_again_busy() {
    let mut d = domain();
    for index in 1..=LIMITS.applications {
        let request = Token::new(u64::from(index));
        outputs(
            &mut d,
            Event::Application {
                role: None,
                carried: None,
                request,
                token: request,
                retention: Duration::from_secs(60),
                ask: Ask::Adopt(fixtures::adoption()),
            },
        );
        outputs(
            &mut d,
            Event::ApplicationAnswered { request, outcome: people::Outcome::Refused(people::Refusal::Unknown) },
        );
    }
    let requests = outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(99),
            token: Token::new(99),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    assert_eq!(requests.as_ref(), &[Request::ApplicationAgain { request: Token::new(99), why: people::Again::Busy }]);
    assert_eq!(d.application_outcome(Token::new(99)), None);
}

#[test]
fn a_kept_outcome_is_saved_with_the_end_of_its_retention() {
    let mut d = domain();
    let mut adoption = fixtures::adoption();
    adoption.forge = 9;
    let requests = outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(adoption),
        },
    );
    let row = Stored::Application {
        token: 9,
        until: Wall::from_nanos(160_000_000_000),
        outcome: ApplicationOutcome::ForgeRefused(AdoptionRefusal::NoSuchForge),
    };
    assert!(requests.contains(&Request::Save { record: row, release: None, read_afresh: None }));
    assert!(requests.contains(&Request::ApplicationDone { request: Token::new(4), token: Token::new(9) }));
}

#[test]
fn an_outcome_past_its_retention_is_erased_at_the_next_application() {
    let mut d = domain();
    let outcome = ApplicationOutcome::CoreRefused(people::Refusal::Unknown);
    outputs(&mut d, Event::Restore { record: Stored::Application { token: 9, until: env().wall, outcome } });
    let requests = outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(10),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    assert!(requests.contains(&Request::Erase { key: Key::Application(9) }));
    assert_eq!(d.application_outcome(Token::new(9)), None);
}

#[test]
fn not_installed_is_a_terminal_application_refusal_rather_than_a_retry() {
    let mut d = domain();
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    let sent = ready(&mut d);
    let [Request::Call { call, .. }] = sent.as_ref() else { panic!("locate call") };
    let requests = outputs(
        &mut d,
        Event::Client(skein_forge_client::Event::Answered {
            call: *call,
            cost: 0,
            free: 0,
            rate: None,
            result: Err(skein_forge::api::Error::NotInstalled),
        }),
    );
    assert!(requests.contains(&Request::ApplicationDone { request: Token::new(4), token: Token::new(9) }));
    assert_eq!(
        d.application_outcome(Token::new(9)),
        Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::NotInstalled))
    );
}

#[test]
fn a_configured_forge_without_a_writer_refuses_adoption_before_any_read() {
    let mut d = Domain::new(
        &LIMITS,
        4,
        skein_forge_client::Config { namespace: Box::from(b"test".as_slice()), writers: Box::new([]) },
        &[GitIdentity { forge: 1, credential: 1 }],
        &[],
        false,
    )
    .unwrap();
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    assert_eq!(
        d.application_outcome(Token::new(9)),
        Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::Unreachable))
    );
    assert!(ready(&mut d).is_empty(), "no Locate before Connected");
}
#[test]
fn a_stopped_forge_keeps_its_writer_but_refuses_adoption_before_a_read() {
    let mut d = domain();
    outputs(
        &mut d,
        Event::Connection { forge: 1, state: Connection::Stopped { reason: Box::from(b"key refused".as_slice()) } },
    );
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(9),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(fixtures::adoption()),
        },
    );
    assert_eq!(
        d.application_outcome(Token::new(9)),
        Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::Unreachable))
    );
    assert_eq!(d.writers.get(&1), Some(&7));
    assert!(ready(&mut d).is_empty());
}

#[test]
fn each_admission_conflict_keeps_its_own_reason() {
    for why in [AdoptionRefusal::AnotherProject, AdoptionRefusal::HomeHeld, AdoptionRefusal::PrefixOccupied] {
        let mut d = domain();
        if why != AdoptionRefusal::PrefixOccupied {
            let mut row = adopted();
            if why == AdoptionRefusal::AnotherProject {
                row.project += 1;
            } else {
                row.provider.repository += 1;
            }
            outputs(&mut d, Event::Restore { record: Stored::Repository(row) });
        }
        let request = Token::new(4);
        let mut requests = fixtures::begin_adoption(&mut d, request, fixtures::adoption());
        if why == AdoptionRefusal::PrefixOccupied {
            fixtures::answer_next(
                &mut d,
                Ok(skein_forge::api::Answer::Permission(skein_forge::api::Permission::Write)),
            );
            requests = fixtures::answer_next(
                &mut d,
                Ok(skein_forge::api::Answer::Branches(Box::new([Box::from(b"temper/foreign".as_slice())]))),
            );
        }
        assert!(
            requests.contains(&Request::Adopted { reply_to: request, result: Err(skein_forge::api::Error::Refused) }),
            "{why:?}"
        );
        let requests = outputs(
            &mut d,
            Event::ApplicationAnswered { request, outcome: people::Outcome::Refused(people::Refusal::Unknown) },
        );
        let outcome = ApplicationOutcome::ForgeRefused(why);
        assert_eq!(d.application_outcome(request), Some(&outcome));
        assert!(requests.contains(&Request::Save {
            record: Stored::Application { token: request.raw(), until: Wall::from_nanos(160_000_000_000), outcome },
            release: None,
            read_afresh: None
        }));
        assert!(requests.contains(&Request::ApplicationDone { request, token: request }));
    }
}

#[test]
fn thirty_two_adoptions_are_answered_during_retention() {
    let limits = Limits { applications: 32, ..LIMITS };
    let env = Env { limits, ..env() };
    let mut d = Domain::new(
        &limits,
        4,
        skein_forge_client::Config {
            namespace: Box::from(b"test".as_slice()),
            writers: Box::new([skein_forge_client::Writer { forge: 1, author: 7 }]),
        },
        &[GitIdentity { forge: 1, credential: 1 }],
        &[],
        false,
    )
    .expect("valid connector");
    let mut out = Queue::with_capacity(max_out(&limits));
    for number in 1..=32 {
        let request = Token::new(number);
        let mut adoption = fixtures::adoption();
        adoption.forge = 9;
        step(
            &mut d,
            &env,
            Event::Application {
                role: None,
                carried: None,
                request,
                token: request,
                retention: Duration::from_secs(86400),
                ask: Ask::Adopt(adoption),
            },
            &mut out,
        );
        let mut done = false;
        while let Some(output) = out.pop() {
            if output == (Request::ApplicationDone { request, token: request }) {
                done = true;
            }
        }
        assert!(done, "adoption {number}, including fifth and 32nd, must answer");
        assert_eq!(
            d.application_outcome(request),
            Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::NoSuchForge))
        );
    }
    assert_eq!(
        d.application_outcome(Token::new(1)),
        Some(&ApplicationOutcome::ForgeRefused(AdoptionRefusal::NoSuchForge)),
        "earlier answers remain retained"
    );
    let request = Token::new(33);
    step(
        &mut d,
        &env,
        Event::Application {
            role: None,
            carried: None,
            request,
            token: request,
            retention: Duration::from_secs(86400),
            ask: Ask::Adopt(fixtures::adoption()),
        },
        &mut out,
    );
    assert_eq!(out.pop(), Some(Request::ApplicationAgain { request, why: people::Again::Busy }));
    assert!(out.is_empty());
}
