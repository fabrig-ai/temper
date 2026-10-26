use super::fixtures;
use jig_client::contract;
use temper_client_domain::forge;
use temper_client_domain::forge::family;
use temper_client_protocol_wire::{self as wire, CLIENT_VERSION, FORGE, v1 as w};

#[test]
fn an_adoption_round_trips_through_the_family_record() {
    let limits = fixtures::wire_limits();
    for role in [forge::Role::Owned, forge::Role::Adopted, forge::Role::Fork, forge::Role::Context] {
        let mut adoption = fixtures::adoption();
        adoption.role = role;
        let record = family::request(&adoption, &limits).expect("fixture encodes");
        assert_eq!((record.family, record.version), (FORGE, CLIENT_VERSION), "{role:?}: owner");
        let value = (match w::Requesting::decode(&limits, &mut skein_lib::Reader::new(&record.bytes))
            .expect("adoption decodes")
            .into_parts()
            .request
        {
            w::Request::Adopt(adopt) => adopt,
            w::Request::Reachable(_)
            | w::Request::Connect(_)
            | w::Request::Accept(_)
            | w::Request::Install(_)
            | w::Request::Return(_)
            | w::Request::SetUp(_)
            | w::Request::StartDemo => {
                panic!("expected adoption")
            }
        })
        .into_parts();
        assert_eq!((value.project, value.home, value.forge), (adoption.project, adoption.home, adoption.forge));
        assert_eq!(
            (value.owner, value.name, value.prefix, value.landing),
            (adoption.owner, adoption.name, adoption.prefix, adoption.landing)
        );
        assert_eq!((value.ci, value.checks.as_slice()), (adoption.ci, adoption.checks.as_ref()));
        assert_eq!(
            value.role,
            match role {
                forge::Role::Owned => w::Role::Owned,
                forge::Role::Adopted => w::Role::Adopted,
                forge::Role::Fork => w::Role::Fork,
                forge::Role::Context => w::Role::Context,
            }
        );
    }
}
fn adopted(protection: w::Protection) -> w::Outcome {
    let limits = fixtures::wire_limits();
    w::Outcome::Adopted(
        w::Adopted::new(
            &limits,
            w::AdoptedParts {
                forge: 1,
                repository: 7,
                owner: Box::from(*b"acme"),
                name: Box::from(*b"shop"),
                role: w::Role::Owned,
                home: true,
                kinds: w::Kinds::new(
                    &limits,
                    w::KindsParts { skein_bools: [true, false, true, false, true, false, true, false, true] },
                )
                .expect("kinds fit"),
                protection,
            },
        )
        .expect("adopted fits"),
    )
}
fn protection_rule() -> w::Protection {
    let limits = fixtures::wire_limits();
    let mut contexts = skein_lib::List::with_capacity(1);
    contexts.push(Box::from(*b"check")).expect("one context");
    w::Protection::Rule(
        w::Rule::new(
            &limits,
            w::RuleParts { branch: Box::from(*b"main"), contexts, approvals: 2, dismiss_stale: true },
        )
        .expect("rule fits"),
    )
}
fn answer(outcome: w::Outcome) -> jig_client::Family {
    let record = w::Answer::new(&fixtures::wire_limits(), w::AnswerParts { outcome }).expect("outcome fits");
    jig_client::Family { family: FORGE, version: CLIENT_VERSION, bytes: wire::answer(&record) }
}
fn refused(reason: w::ForgeReason) -> w::Outcome {
    w::Outcome::ForgeRefused(
        w::ForgeRefused::new(&fixtures::wire_limits(), w::ForgeRefusedParts { reason }).expect("reason fits"),
    )
}
fn outcome_rows() -> [(&'static str, w::Outcome, forge::Answer); 13] {
    let core = w::Outcome::CoreRefused(
        w::CoreRefused::new(&fixtures::wire_limits(), w::CoreRefusedParts { refusal: fixtures::core_refusal() })
            .expect("core refusal fits"),
    );
    [
        (
            "unknown protection",
            adopted(w::Protection::Unknown),
            forge::Answer::Adopted(fixtures::adopted(forge::Protection::Unknown)),
        ),
        (
            "absent protection",
            adopted(w::Protection::Absent),
            forge::Answer::Adopted(fixtures::adopted(forge::Protection::Absent)),
        ),
        (
            "protection rule",
            adopted(protection_rule()),
            forge::Answer::Adopted(fixtures::adopted(forge::Protection::Rule(forge::Rule {
                branch: Box::from(*b"main"),
                contexts: Box::from([Box::from(&b"check"[..])]),
                approvals: 2,
                dismiss_stale: true,
            }))),
        ),
        ("core refused", core, forge::Answer::CoreRefused(contract::Refusal::Role { held: None })),
        (
            "no such forge",
            refused(w::ForgeReason::NoSuchForge),
            forge::Answer::ForgeRefused(forge::Reason::NoSuchForge),
        ),
        ("not found", refused(w::ForgeReason::NotFound), forge::Answer::ForgeRefused(forge::Reason::NotFound)),
        ("no access", refused(w::ForgeReason::NoAccess), forge::Answer::ForgeRefused(forge::Reason::NoAccess)),
        ("refused", refused(w::ForgeReason::Refused), forge::Answer::ForgeRefused(forge::Reason::Refused)),
        ("in flight", refused(w::ForgeReason::InFlight), forge::Answer::ForgeRefused(forge::Reason::InFlight)),
        ("too large", refused(w::ForgeReason::TooLarge), forge::Answer::ForgeRefused(forge::Reason::TooLarge)),
        ("unreachable", refused(w::ForgeReason::Unreachable), forge::Answer::ForgeRefused(forge::Reason::Unreachable)),
        (
            "invalid answer",
            refused(w::ForgeReason::InvalidAnswer),
            forge::Answer::ForgeRefused(forge::Reason::InvalidAnswer),
        ),
        ("unknown", w::Outcome::Unknown, forge::Answer::Unknown),
    ]
}
#[test]
fn each_outcome_decodes_to_the_forge_sides_answer() {
    let wire = fixtures::wire_limits();
    let link = fixtures::link_limits();
    for (name, outcome, expected) in outcome_rows() {
        assert_eq!(family::answer(&answer(outcome), &wire, &link), Some(expected), "{name}");
    }
}

#[test]
fn setup_outcomes_are_undecodable_for_an_adoption() {
    let limits = fixtures::wire_limits();
    for outcome in [
        w::Outcome::SettingUp,
        w::Outcome::SetUpRefused(
            w::SetUpRefused::new(&limits, w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).expect("refusal"),
        ),
    ] {
        assert_eq!(family::answer(&answer(outcome), &limits, &fixtures::link_limits()), None);
    }
}
