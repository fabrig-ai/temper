use super::fixtures;
use crate::family;
use jig_domain_people as people;
use jig_protocol_engine_clients as kit;
use skein_lib::{List, Reader, ReplyTo, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};
use temper_engine_domain as root;
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as change;
const REPLY: ReplyTo = ReplyTo::new(Token::new(19));
const REQUEST_KEY: [u8; 16] = [5; 16];
const WATCHER: Token = Token::new(7);
fn configured(git: &[u8]) -> Box<[crate::settings::Forge]> {
    let mut forges = fixtures::engine().forges;
    forges.get_mut(0).unwrap().git = git.into();
    forges
}
fn adopt() -> w::Adopt {
    let mut checks = List::with_capacity(2);
    checks.push(2).unwrap();
    checks.push(3).unwrap();
    w::Adopt::new(
        &w::CEILINGS,
        w::AdoptParts {
            project: 7,
            home: true,
            forge: 1,
            owner: b"acme".as_slice().into(),
            name: b"shop".as_slice().into(),
            prefix: b"repo".as_slice().into(),
            role: w::Role::Adopted,
            landing: b"main".as_slice().into(),
            ci: true,
            checks,
        },
    )
    .unwrap()
}
fn exchange(family: u16, version: u32, record: Box<[u8]>) -> kit::ToApplication {
    kit::ToApplication::Request {
        reply_to: REPLY,
        sign_in: 3,
        key: REQUEST_KEY,
        family,
        version,
        record,
        digest: [7; 32],
        carried: None,
        scope: people::ApplicationScope::Project(7),
    }
}
#[test]
fn an_adopt_reaches_the_root_with_its_forges_host() {
    let forges = configured(b"https://forge.example:3000/git");
    let event = family::to_root(
        exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&requesting())),
        &forges,
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Application { reply_to, sign_in, key, digest, request, .. }) = event
    else {
        panic!("application request")
    };
    assert_eq!((reply_to, sign_in, key, digest), (REPLY, 3, [5; 16], [7; 32]));
    assert_eq!(
        request,
        root::Application::Forge(forge::Ask::Adopt(forge::Adoption {
            project: 7,
            home: true,
            forge: 1,
            host: b"forge.example:3000".as_slice().into(),
            owner: b"acme".as_slice().into(),
            name: b"shop".as_slice().into(),
            prefix: b"repo".as_slice().into(),
            role: forge::Role::Adopted,
            landing: b"main".as_slice().into(),
            ci: true,
            checks: Box::new([2, 3]),
        }))
    );
}
#[test]
fn an_adopt_naming_no_configured_forge_reaches_the_root_with_no_host() {
    let event = family::to_root(
        exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&requesting())),
        &[],
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Application { request, .. }) = event else { panic!("adoption") };
    let root::Application::Forge(forge::Ask::Adopt(request)) = request else { panic!("adoption") };
    assert!(request.host.is_empty());
    assert_eq!(request.forge, 1);
}
#[test]
fn an_adopt_of_another_version_is_answered_undecodable() {
    for (row, family, version, record, expected) in [
        ("family", 2, wire::CLIENT_VERSION, wire::requesting(&requesting()), REPLY),
        ("version", 1, wire::CLIENT_VERSION + 1, wire::requesting(&requesting()), REPLY),
        ("malformed", 1, wire::CLIENT_VERSION, Box::new([]), REPLY),
        ("trailing", 1, wire::CLIENT_VERSION, trailing_adopt(), REPLY),
    ] {
        let Err(answer) = family::to_root(exchange(family, version, record), &[], None, &w::CEILINGS) else {
            panic!("{row}: undecodable")
        };
        let kit::Event::Application(kit::FromApplication::Undecodable { to }) = *answer else {
            panic!("{row}: undecodable reply")
        };
        assert_eq!(to, expected, "{row}");
    }
}
fn trailing_adopt() -> Box<[u8]> {
    let record = wire::requesting(&requesting());
    let mut bytes = skein_lib::Writer::new(record.len().checked_add(1).unwrap());
    bytes.put(&record).unwrap();
    bytes.put(&[0]).unwrap();
    bytes.finish()
}
#[test]
fn an_unconfigured_or_unknown_family_read_has_no_record() {
    for (row, family, version, record, scope, expected) in [
        (
            "forge",
            wire::FORGE,
            wire::CLIENT_VERSION,
            Box::<[u8]>::default(),
            people::ApplicationScope::Project(7),
            (REPLY, wire::FORGE, None),
        ),
        ("other", 2, 3, Box::new([]), people::ApplicationScope::Project(7), (REPLY, 2, None)),
        (
            "demo, unconfigured",
            wire::FORGE,
            wire::CLIENT_VERSION,
            wire::reading(&w::Reading::new(&w::CEILINGS, w::ReadingParts { read: w::Read::Demo }).unwrap()),
            people::ApplicationScope::Deployment,
            (REPLY, wire::FORGE, None),
        ),
    ] {
        let input = kit::ToApplication::Read {
            reply: REPLY,
            sign_in: 3,
            family,
            version,
            record,
            scope,
            window: jig_domain_core::documents::Window { rows: 2, after: None },
        };
        let Err(answer) = family::to_root(input, &[], None, &w::CEILINGS) else { panic!("{row}: unknown read") };
        let kit::Event::Application(kit::FromApplication::Read { reply, family, record }) = *answer else {
            panic!("{row}: read")
        };
        assert_eq!((reply, family, record), expected, "{row}");
    }
}
fn demo_read(scope: people::ApplicationScope) -> kit::ToApplication {
    kit::ToApplication::Read {
        reply: REPLY,
        sign_in: 3,
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        record: wire::reading(&w::Reading::new(&w::CEILINGS, w::ReadingParts { read: w::Read::Demo }).unwrap()),
        scope,
        window: jig_domain_core::documents::Window { rows: 2, after: None },
    }
}
fn demo_record(input: kit::ToApplication, demo: &crate::settings::Demo) -> Box<[u8]> {
    let Err(answer) = family::to_root(input, &[], Some(demo), &w::CEILINGS) else { panic!("demo read") };
    let kit::Event::Application(kit::FromApplication::Read { reply, family, record }) = *answer else {
        panic!("read answer")
    };
    assert_eq!((reply, family), (REPLY, wire::FORGE));
    record.unwrap()
}
#[test]
fn the_demo_read_answers_the_configured_demo() {
    let demo = crate::settings::Demo {
        state: Some(forge::DemoState::Running),
        landing: b"main".as_slice().into(),
        key_file: b"/demo-key".as_slice().into(),
        project: 1,
        forge: 2,
        provider: 3,
        owner: b"demo".as_slice().into(),
        name: b"widget".as_slice().into(),
        cues: Box::new([
            b"Add CSV export to widget".as_slice().into(),
            b"Add a dark theme to widget".as_slice().into(),
        ]),
    };
    let mut cues = List::with_capacity(2);
    cues.push(b"Add CSV export to widget".as_slice().into()).unwrap();
    cues.push(b"Add a dark theme to widget".as_slice().into()).unwrap();
    let expected = w::DemoAnswer::new(
        &w::CEILINGS,
        w::DemoAnswerParts {
            state: w::DemoState::Running,
            project: 1,
            forge: 2,
            provider: 3,
            owner: b"demo".as_slice().into(),
            name: b"widget".as_slice().into(),
            cues,
        },
    )
    .unwrap();
    for scope in [people::ApplicationScope::Deployment, people::ApplicationScope::Project(7)] {
        let record = demo_record(demo_read(scope), &demo);
        assert_eq!(wire::decode_demo_answer(&record, &w::CEILINGS), Some(expected.clone()), "{scope:?}");
    }
}
#[test]
fn the_demo_answer_carries_no_key_path_socket_or_placeholder() {
    let service = fixtures::demo_service();
    let record = demo_record(
        demo_read(people::ApplicationScope::Deployment),
        service.engine.as_ref().expect("engine").demo.as_ref().unwrap(),
    );
    // The answer's security rule: no key path, listen socket or placeholder.
    for forbidden in [b"llm.key".as_slice(), b"demo/llm", b"$", b"socket"] {
        assert_eq!(skein_lib::bytes::find(&record, forbidden), None, "{forbidden:?}");
    }
}
#[test]
fn a_configured_demo_does_not_answer_other_or_malformed_reads() {
    let service = fixtures::demo_service();
    let reading = wire::reading(&w::Reading::new(&w::CEILINGS, w::ReadingParts { read: w::Read::Demo }).unwrap());
    let mut malformed = reading.clone();
    *malformed.last_mut().expect("read tag") = 1;
    let mut writer = skein_lib::Writer::new(reading.len().checked_add(1).expect("one trailing byte fits"));
    writer.put(&reading).expect("reading room");
    writer.put(&[0]).expect("trailing byte room");
    let trailing = writer.finish();
    for (row, family, version, record) in [
        ("family", 2, wire::CLIENT_VERSION, reading.clone()),
        ("version", wire::FORGE, wire::CLIENT_VERSION + 1, reading),
        ("malformed", wire::FORGE, wire::CLIENT_VERSION, malformed),
        ("trailing", wire::FORGE, wire::CLIENT_VERSION, trailing),
    ] {
        let mut input = demo_read(people::ApplicationScope::Deployment);
        let kit::ToApplication::Read { family: f, version: v, record: r, .. } = &mut input else { panic!("read") };
        (*f, *v, *r) = (family, version, record);
        let Err(answer) =
            family::to_root(input, &[], service.engine.as_ref().expect("engine").demo.as_ref(), &w::CEILINGS)
        else {
            panic!("{row}: read answer")
        };
        let kit::Event::Application(kit::FromApplication::Read { record, .. }) = *answer else { panic!("{row}: read") };
        assert_eq!(record, None, "{row}");
    }
}
#[test]
fn a_forge_family_watch_is_refused_unknown() {
    for (row, family, version, expected) in [
        ("forge", wire::FORGE, wire::CLIENT_VERSION, (Token::new(7), jig_domain_core::WatchRefusal::Unknown)),
        ("other", 2, 3, (Token::new(7), jig_domain_core::WatchRefusal::Unknown)),
    ] {
        let input = kit::ToApplication::Watch {
            watcher: WATCHER,
            sign_in: 3,
            family,
            version,
            record: Box::new([]),
            scope: people::ApplicationScope::Project(7),
            window: jig_domain_core::documents::Window { rows: 2, after: None },
        };
        let Err(answer) = family::to_root(input, &[], None, &w::CEILINGS) else { panic!("{row}: refused watch") };
        let kit::Event::WatchRefused { watcher, why } = *answer else { panic!("{row}: refusal") };
        assert_eq!((watcher, why), expected, "{row}");
    }
}
#[test]
fn a_git_base_with_a_port_keeps_it_in_the_host() {
    for (git, expected) in [
        (b"https://forge.example/git".as_slice(), b"forge.example".as_slice()),
        (b"http://127.0.0.1:3000/".as_slice(), b"127.0.0.1:3000".as_slice()),
        (b"https://[::1]:3000/git".as_slice(), b"[::1]:3000".as_slice()),
        (b"https://forge.example".as_slice(), b"forge.example".as_slice()),
        (b"https://user@forge.example:3000/git".as_slice(), b"forge.example:3000".as_slice()),
    ] {
        assert_eq!(family::host(git).as_ref(), expected, "{git:?}");
    }
}
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
fn encoded(outcome: Option<forge::ApplicationOutcome>) -> w::Outcome {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small).clients.wire;
    let outcome = match outcome {
        Some(outcome) => Some(temper_engine_domain::ApplicationAnswer::Adopt(outcome)),
        None => None,
    };
    let bytes = family::answer(outcome, &w::CEILINGS, &jig).unwrap();
    let mut reader = Reader::new(&bytes);
    let answer = w::Answer::decode(&w::CEILINGS, &mut reader).unwrap();
    assert!(reader.is_empty());
    answer.into_parts().outcome
}
#[test]
fn each_outcome_is_encoded_as_the_forge_familys_answer() {
    for (row, input, expected) in [
        ("no forge", forge::AdoptionRefusal::NoSuchForge, w::ForgeReason::NoSuchForge),
        ("missing", forge::AdoptionRefusal::NotFound, w::ForgeReason::NotFound),
        ("access", forge::AdoptionRefusal::NoAccess, w::ForgeReason::NoAccess),
        ("refused", forge::AdoptionRefusal::Refused, w::ForgeReason::Refused),
        ("in flight", forge::AdoptionRefusal::InFlight, w::ForgeReason::InFlight),
        ("too large", forge::AdoptionRefusal::TooLarge, w::ForgeReason::TooLarge),
        ("unreachable", forge::AdoptionRefusal::Unreachable, w::ForgeReason::Unreachable),
        ("invalid", forge::AdoptionRefusal::InvalidAnswer, w::ForgeReason::InvalidAnswer),
    ] {
        let w::Outcome::ForgeRefused(answer) = encoded(Some(forge::ApplicationOutcome::ForgeRefused(input))) else {
            panic!("{row}: forge refusal")
        };
        assert_eq!(answer.into_parts().reason, expected, "{row}");
    }
}
#[test]
fn an_outcome_past_its_retention_is_encoded_unknown() {
    assert_eq!(encoded(None), w::Outcome::Unknown);
}
#[test]
fn a_core_refusal_uses_jigs_refusal_record() {
    use jig_client_protocol_wire::v19 as j;
    let limits = fixtures::engine().jig.limits.clients.wire;
    for (row, input, expected) in [
        ("unknown", people::Refusal::Unknown, j::Refusal::Unknown),
        ("name taken", people::Refusal::NameTaken, j::Refusal::NameTaken),
        ("not offered", people::Refusal::NotOffered, j::Refusal::NotOffered),
        (
            "taken",
            people::Refusal::Taken { by: 7 },
            j::Refusal::Taken(j::RefusalTaken::new(&limits, j::RefusalTakenParts { by: 7 }).unwrap()),
        ),
        (
            "authority",
            people::Refusal::Authority { lacking: people::Lack::Writer, proposable: Some(people::Accepter::Role(1)) },
            j::Refusal::Authority(
                j::RefusalAuthority::new(
                    &limits,
                    j::RefusalAuthorityParts { lacking: j::Lacking::Writer, proposable: Some(true) },
                )
                .unwrap(),
            ),
        ),
    ] {
        let w::Outcome::CoreRefused(record) = encoded(Some(forge::ApplicationOutcome::CoreRefused(input))) else {
            panic!("{row}: core refusal")
        };
        let mut reader = Reader::new(record.refusal());
        assert_eq!(j::Refusal::decode(&limits, &mut reader).unwrap(), expected, "{row}");
        assert!(reader.is_empty(), "{row}");
    }
}
fn adopted(protection: forge::Protection) -> forge::ApplicationOutcome {
    forge::ApplicationOutcome::Adopted(forge::AdoptedRepository {
        repository: skein_forge::api::Repository { forge: 1, repository: 42 },
        owner: b"acme".as_slice().into(),
        name: b"shop".as_slice().into(),
        role: forge::Role::Fork,
        home: true,
        kinds: forge::Kinds {
            read: true,
            push: false,
            open: true,
            land: false,
            review: true,
            status: false,
            comment: true,
            issue: false,
            branch: true,
        },
        protection,
    })
}
#[test]
fn an_adopted_repository_keeps_its_facts_in_the_answer() {
    let w::Outcome::Adopted(record) = encoded(Some(adopted(forge::Protection::Absent))) else { panic!("adopted") };
    let a = record.into_parts();
    assert_eq!((a.forge, a.repository, a.home, a.role), (1, 42, true, w::Role::Fork));
    assert_eq!((a.owner.as_ref(), a.name.as_ref()), (b"acme".as_slice(), b"shop".as_slice()));
    let k = a.kinds;
    assert_eq!(
        [k.read(), k.push(), k.open(), k.land(), k.review(), k.status(), k.comment(), k.issue(), k.branch()],
        [true, false, true, false, true, false, true, false, true]
    );
    assert_eq!(a.protection, w::Protection::Absent);
}
#[test]
fn an_adopted_repository_keeps_its_protection_rule() {
    let outcome = adopted(forge::Protection::Rule(skein_forge::api::Protection {
        queue: false,
        signatures: false,
        linear: false,
        code_owners: false,
        bypass: skein_forge::api::Bypass::Never,

        branch: b"main".as_slice().into(),
        contexts: Box::new([b"ci/build".as_slice().into()]),
        approvals: 2,
        dismiss_stale: true,
    }));
    let w::Outcome::Adopted(record) = encoded(Some(outcome)) else { panic!("adopted") };
    let w::Protection::Rule(rule) = record.into_parts().protection else { panic!("rule") };
    let rule = rule.into_parts();
    assert_eq!(rule.branch.as_ref(), b"main");
    assert_eq!(rule.contexts.as_slice(), &[b"ci/build".as_slice().into()]);
    assert_eq!((rule.approvals, rule.dismiss_stale), (2, true));
}

#[test]
fn a_request_under_an_owner_without_records_is_answered_undecodable() {
    for owner in [wire::Owner::Pages, wire::Owner::Runner] {
        let number = wire::number(owner);
        let Err(answer) = family::to_root(
            exchange(number, wire::CLIENT_VERSION, wire::requesting(&requesting())),
            &[],
            None,
            &w::CEILINGS,
        ) else {
            panic!("owner is not served: {owner:?}")
        };
        let kit::Event::Application(kit::FromApplication::Undecodable { to }) = *answer else {
            panic!("undecodable answer: {owner:?}")
        };
        assert_eq!(to, REPLY, "{owner:?}");
    }
}

const NOT_OFFERED_RECORD: &[u8] = &[10];
const NO_ESCALATION_RECORD: &[u8] = &[12];
const PROPOSABLE_WRITER_RECORD: &[u8] = &[5, 13, 1, 1];
const MALFORMED_POSITION: u32 = 3;
const MALFORMED_RECORD: &[u8] = &[0, 0, 0, 0, 3, 3];
fn refusal_bytes(refusal: people::Refusal) -> Box<[u8]> {
    let w::Outcome::CoreRefused(record) = encoded(Some(forge::ApplicationOutcome::CoreRefused(refusal))) else {
        panic!("core refusal")
    };
    record.into_parts().refusal
}
#[test]
fn a_core_refusal_is_carried_as_jigs_refusal_record() {
    assert_eq!(refusal_bytes(people::Refusal::NotOffered).as_ref(), NOT_OFFERED_RECORD);
}
#[test]
fn a_refusal_naming_no_one_to_escalate_to_is_carried_with_its_facts() {
    assert_eq!(refusal_bytes(people::Refusal::NoEscalationHolder).as_ref(), NO_ESCALATION_RECORD);
}
#[test]
fn an_authority_refusal_keeps_whom_a_proposal_could_go_to() {
    let refusal =
        people::Refusal::Authority { lacking: people::Lack::Writer, proposable: Some(people::Accepter::Role(1)) };
    assert_eq!(refusal_bytes(refusal).as_ref(), PROPOSABLE_WRITER_RECORD);
}
#[test]
fn a_malformed_refusal_encodes_its_fields() {
    let refusal = people::Refusal::NotWellFormed { at: MALFORMED_POSITION, why: people::Malformed::Cycle };
    assert_eq!(refusal_bytes(refusal).as_ref(), MALFORMED_RECORD);
}
#[test]
fn a_family_request_keeps_its_supplied_admission_scope() {
    let mut input = exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&requesting()));
    let kit::ToApplication::Request { scope, .. } = &mut input else { panic!("request") };
    *scope = people::ApplicationScope::Deployment;
    let event = family::to_root(input, &[], None, &w::CEILINGS).unwrap();
    let root::Event::Party(root::PartyInput::Application { scope, .. }) = event else { panic!("application") };
    assert_eq!(scope, people::ApplicationScope::Deployment);
}

#[test]
fn not_installed_projects_to_no_access_until_temper_07_adds_its_wire_reason() {
    let w::Outcome::ForgeRefused(answer) =
        encoded(Some(forge::ApplicationOutcome::ForgeRefused(forge::AdoptionRefusal::NotInstalled)))
    else {
        panic!("forge refusal")
    };
    assert_eq!(answer.reason(), &w::ForgeReason::NoAccess);
}

const CHANGE_TASK: u64 = 51;
fn watching(task: u64) -> Box<[u8]> {
    wire::watching(
        &w::Watching::new(
            &w::CEILINGS,
            w::WatchingParts {
                watch: w::Watch::Change(w::ChangeSubject::new(&w::CEILINGS, w::ChangeSubjectParts { task }).unwrap()),
            },
        )
        .unwrap(),
    )
}
fn watch_exchange(family: u16, version: u32, record: Box<[u8]>) -> kit::ToApplication {
    kit::ToApplication::Watch {
        watcher: WATCHER,
        sign_in: 3,
        family,
        version,
        record,
        scope: people::ApplicationScope::Project(7),
        window: jig_domain_core::documents::Window { rows: 2, after: None },
    }
}
#[test]
fn a_change_watch_reaches_the_root_with_its_scope_subject_and_window() {
    let event = family::to_root(
        watch_exchange(wire::FORGE, wire::CLIENT_VERSION, watching(CHANGE_TASK)),
        &[],
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Watch { watcher, sign_in, subject, window }) = event else {
        panic!("root watch")
    };
    assert_eq!((watcher, sign_in), (WATCHER, 3));
    assert_eq!(
        subject,
        jig_domain_core::documents::Watched::Application {
            scope: people::ApplicationScope::Project(7),
            connector: wire::FORGE,
            subject: Token::new(CHANGE_TASK),
        }
    );
    assert_eq!(window, jig_domain_core::documents::Window { rows: 2, after: None });
}
#[test]
fn invalid_change_watches_are_refused_unknown_without_the_root() {
    let record = watching(CHANGE_TASK);
    let mut trailing = skein_lib::Writer::new(record.len().checked_add(1).unwrap());
    trailing.put(&record).unwrap();
    trailing.put(&[0]).unwrap();
    for (row, family, version, record) in [
        ("family", wire::number(wire::Owner::Pages), wire::CLIENT_VERSION, watching(CHANGE_TASK)),
        ("version", wire::FORGE, wire::CLIENT_VERSION + 1, watching(CHANGE_TASK)),
        ("malformed", wire::FORGE, wire::CLIENT_VERSION, Box::new([])),
        ("trailing", wire::FORGE, wire::CLIENT_VERSION, trailing.finish()),
        ("zero task", wire::FORGE, wire::CLIENT_VERSION, watching(0)),
    ] {
        let Err(answer) = family::to_root(watch_exchange(family, version, record), &[], None, &w::CEILINGS) else {
            panic!("{row}: refused watch")
        };
        let kit::Event::WatchRefused { watcher, why } = *answer else { panic!("{row}: unknown") };
        assert_eq!((watcher, why), (WATCHER, jig_domain_core::WatchRefusal::Unknown), "{row}");
    }
}
fn panel() -> forge::Panel {
    forge::Panel {
        task: CHANGE_TASK,
        forge: 1,
        owner: b"acme".as_slice().into(),
        name: b"shop".as_slice().into(),
        branch: b"temper/51".as_slice().into(),
        landing: b"main".as_slice().into(),
        head: Some([4; 32]),
        pull: Some(123),
        step: forge::PanelStep::Queued { place: 3, ahead: Box::new([49, 47]) },
        hold: Some(change::Hold::Updates),
        runs: Some(81),
        ci: change::Status::Pending,
        gates: Box::new([
            forge::PanelGate {
                number: 1,
                kind: change::GateKind::Agent,
                blocking: true,
                verdict: change::Status::Passed,
            },
            forge::PanelGate {
                number: 2,
                kind: change::GateKind::Person,
                blocking: true,
                verdict: change::Status::Failed,
            },
            forge::PanelGate {
                number: 3,
                kind: change::GateKind::Check,
                blocking: false,
                verdict: change::Status::Pending,
            },
            forge::PanelGate {
                number: 4,
                kind: change::GateKind::Check,
                blocking: true,
                verdict: change::Status::Unknown,
            },
        ]),
        repairs: forge::PanelUsed { used: 1, limit: 3 },
        resolutions: forge::PanelUsed { used: 2, limit: 4 },
        updates: forge::PanelUsed { used: 3, limit: 5 },
    }
}
fn decoded_panel(value: forge::Panel, forges: &[crate::settings::Forge]) -> w::ChangePanel {
    let bytes = family::panel(value, forges, &w::CEILINGS).unwrap();
    wire::decode_change_panel(&bytes, &w::CEILINGS).expect("whole panel at CLIENT_VERSION")
}
fn queued(place: u32, tasks: &[u64]) -> w::Step {
    let mut ahead = List::with_capacity(wire::TASKS_AHEAD);
    for task in tasks {
        ahead.push(*task).unwrap();
    }
    w::Step::Queued(w::Queued::new(&w::CEILINGS, w::QueuedParts { place, ahead }).unwrap())
}
#[test]
fn a_panel_encodes_and_decodes_back_whole() {
    let mut gates = List::with_capacity(wire::GATES);
    for (number, kind, blocks, verdict) in [
        (1, w::GateKind::Review, true, w::Verdict::Approved),
        (2, w::GateKind::Approval, true, w::Verdict::Changes),
        (3, w::GateKind::Check, false, w::Verdict::Pending),
        (4, w::GateKind::Check, true, w::Verdict::None),
    ] {
        gates.push(w::Gate::new(&w::CEILINGS, w::GateParts { number, kind, blocks, verdict }).unwrap()).unwrap();
    }
    let expected = w::ChangePanel::new(
        &w::CEILINGS,
        w::ChangePanelParts {
            task: CHANGE_TASK,
            forge: 1,
            owner: b"acme".as_slice().into(),
            name: b"shop".as_slice().into(),
            branch: b"temper/51".as_slice().into(),
            landing: b"main".as_slice().into(),
            head: Some([4; 32]),
            pull: Some(w::Pull::new(&w::CEILINGS, w::PullParts { number: 123, address: Box::new([]) }).unwrap()),
            step: queued(3, &[49, 47]),
            hold: Some(w::Hold::Updates),
            runs: Some(81),
            ci: w::Ci::Pending,
            gates,
            repairs: w::Used::new(&w::CEILINGS, w::UsedParts { used: 1, limit: 3 }).unwrap(),
            resolutions: w::Used::new(&w::CEILINGS, w::UsedParts { used: 2, limit: 4 }).unwrap(),
            updates: w::Used::new(&w::CEILINGS, w::UsedParts { used: 3, limit: 5 }).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(decoded_panel(panel(), &[]), expected);
}
fn repairing(reason: w::Reason) -> w::Step {
    w::Step::Repairing(w::Repairing::new(&w::CEILINGS, w::RepairingParts { reason }).unwrap())
}
#[test]
fn every_panel_step_keeps_its_payload_on_the_wire() {
    for (input, expected) in [
        (forge::PanelStep::Producing, w::Step::Producing),
        (forge::PanelStep::Opening, w::Step::Opening),
        (forge::PanelStep::Recreating, w::Step::Recreating),
        (forge::PanelStep::Reopening, w::Step::Reopening),
        (forge::PanelStep::Checking, w::Step::Checking),
        (forge::PanelStep::Gating, w::Step::Gating),
        (forge::PanelStep::Queued { place: 3, ahead: Box::new([49, 47]) }, queued(3, &[49, 47])),
        (forge::PanelStep::First, w::Step::First),
        (forge::PanelStep::Updating, w::Step::Updating),
        (forge::PanelStep::Resolving, w::Step::Resolving),
        (forge::PanelStep::Repairing(change::Repair::Ci), repairing(w::Reason::Ci)),
        (
            forge::PanelStep::Repairing(change::Repair::Gate(9)),
            repairing(w::Reason::Gate(w::GateReason::new(&w::CEILINGS, w::GateReasonParts { gate: 9 }).unwrap())),
        ),
        (forge::PanelStep::Repairing(change::Repair::Semantic), repairing(w::Reason::Semantic)),
        (forge::PanelStep::Landing, w::Step::Landing),
        (
            forge::PanelStep::Landed { merge: [8; 32] },
            w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [8; 32] }).unwrap()),
        ),
    ] {
        let mut value = panel();
        value.step = input;
        assert_eq!(*decoded_panel(value, &[]).step(), expected);
    }
}
#[test]
fn every_hold_and_ci_verdict_keeps_its_meaning_on_the_wire() {
    for (input, expected) in [
        (None, None),
        (Some(change::Hold::Stalled), Some(w::Hold::Stalled)),
        (Some(change::Hold::Repairs), Some(w::Hold::Repairs)),
        (Some(change::Hold::Resolutions), Some(w::Hold::Resolutions)),
        (Some(change::Hold::Updates), Some(w::Hold::Updates)),
        (Some(change::Hold::BranchMoved), Some(w::Hold::Moved)),
        (Some(change::Hold::BranchMissing), Some(w::Hold::Missing)),
        (Some(change::Hold::PullClosed), Some(w::Hold::Closed)),
        (Some(change::Hold::Retargeted), Some(w::Hold::Retargeted)),
        (Some(change::Hold::Failed), Some(w::Hold::Failed)),
        (Some(change::Hold::Rejected), Some(w::Hold::Rejected)),
    ] {
        let mut value = panel();
        value.hold = input;
        assert_eq!(*decoded_panel(value, &[]).hold(), expected);
    }
    for (input, expected) in [
        (change::Status::Unknown, w::Ci::Unknown),
        (change::Status::Pending, w::Ci::Pending),
        (change::Status::Passed, w::Ci::Passed),
        (change::Status::Failed, w::Ci::Failed),
    ] {
        let mut value = panel();
        value.ci = input;
        assert_eq!(*decoded_panel(value, &[]).ci(), expected);
    }
}
#[test]
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
fn a_panels_pull_address_uses_its_configured_forge_kind_and_web() {
    use crate::settings::Kind;
    for (row, kind, web, number, expected) in [
        (
            "github",
            Kind::GitHub,
            Some(b"https://github.example".as_slice()),
            1,
            b"https://github.example/acme/shop/pull/123".as_slice(),
        ),
        (
            "forgejo",
            Kind::Forgejo,
            Some(b"https://forge.example/base/".as_slice()),
            1,
            b"https://forge.example/base/acme/shop/pulls/123".as_slice(),
        ),
        ("no web", Kind::GitHub, None, 1, b"".as_slice()),
        ("another forge", Kind::Forgejo, Some(b"https://other.example".as_slice()), 2, b"".as_slice()),
    ] {
        let mut forges = fixtures::forges();
        let forge = forges.get_mut(0).unwrap();
        forge.kind = kind;
        forge.number = number;
        forge.web = match web {
            Some(web) => Some(web.into()),
            None => None,
        };
        let got = decoded_panel(panel(), &forges).into_parts().pull.unwrap();
        assert_eq!(got.number(), 123, "{row}");
        assert_eq!(got.address(), expected, "{row}");
    }
}
#[test]
#[expect(clippy::disallowed_methods, reason = "the test spells out the expected address")]
fn a_panels_pull_address_past_its_bound_is_left_out_and_the_panel_still_delivered() {
    use crate::settings::Kind;
    let web = b"https://forge.example".as_slice();
    let owner = core::iter::repeat_n(b'o', 1024).collect::<Box<[u8]>>();
    for (kind, route) in [(Kind::GitHub, b"/pull/".as_slice()), (Kind::Forgejo, b"/pulls/".as_slice())] {
        // The longest name whose address, WEB/OWNER/NAME + route + "123", fills
        // ADDRESS_BYTES exactly; one byte more passes the bound.
        let fits = usize::try_from(wire::ADDRESS_BYTES).unwrap() - (web.len() + 1 + owner.len() + 1 + route.len() + 3);
        for (row, length, shown) in [("at the bound", fits, true), ("past the bound", fits + 1, false)] {
            let name = core::iter::repeat_n(b'n', length).collect::<Box<[u8]>>();
            let mut forges = fixtures::forges();
            let forge = forges.get_mut(0).unwrap();
            forge.kind = kind;
            forge.web = Some(web.into());
            let mut value = panel();
            value.owner = owner.clone();
            value.name = name.clone();
            let decoded = decoded_panel(value, &forges);
            assert_eq!(decoded.owner(), &*owner, "{row}");
            assert_eq!(decoded.name(), &*name, "{row}");
            let pull = decoded.into_parts().pull.unwrap();
            assert_eq!(pull.number(), 123, "{row}");
            if shown {
                assert_eq!(pull.address(), [web, b"/", &owner, b"/", &name, route, b"123"].concat(), "{row}");
            } else {
                assert_eq!(pull.address(), b"", "{row}");
            }
        }
    }
}
#[test]
fn a_panel_before_the_first_head_has_no_pull_or_running_delegate() {
    let mut value = panel();
    value.head = None;
    value.pull = None;
    value.runs = None;
    value.hold = None;
    value.step = forge::PanelStep::Producing;
    value.ci = change::Status::Unknown;
    value.gates = Box::new([]);
    let got = decoded_panel(value, &fixtures::forges()).into_parts();
    assert_eq!((got.head, got.pull, got.runs, got.hold), (None, None, None, None));
    assert_eq!(got.step, w::Step::Producing);
    assert_eq!(got.ci, w::Ci::Unknown);
    assert!(got.gates.is_empty());
}

fn requesting() -> w::Requesting {
    w::Requesting::new(&w::CEILINGS, w::RequestingParts { request: w::Request::Adopt(adopt()) }).unwrap()
}

#[test]
fn setup_requests_preserve_the_keyed_envelope_and_forge() {
    let request = w::Requesting::new(
        &w::CEILINGS,
        w::RequestingParts {
            request: w::Request::SetUp(
                w::SetUp::new(
                    &w::CEILINGS,
                    w::SetUpParts {
                        forge: 3,
                        org: b"work".as_slice().into(),
                        repository: b"hello".as_slice().into(),
                        project: w::SetUpProject::Existing(
                            w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 1 })
                                .expect("project"),
                        ),
                    },
                )
                .unwrap(),
            ),
        },
    )
    .unwrap();
    let event = family::to_root(
        exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&request)),
        &[],
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Application { reply_to, sign_in, key, digest, request, .. }) = event
    else {
        panic!("setup application")
    };
    assert_eq!(
        (reply_to, sign_in, key, digest, request),
        (
            REPLY,
            3,
            [5; 16],
            [7; 32],
            root::Application::Forge(forge::Ask::SetUp {
                forge: 3,
                org: b"work".as_slice().into(),
                repository: b"hello".as_slice().into(),
                project: forge::SetUpProject::Existing(1)
            })
        )
    );
}

#[test]
fn forges_watches_name_the_deployment_subject() {
    let watching = w::Watching::new(&w::CEILINGS, w::WatchingParts { watch: w::Watch::Forges }).unwrap();
    let event = family::to_root(
        watch_exchange(wire::FORGE, wire::CLIENT_VERSION, wire::watching(&watching)),
        &[],
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Watch { watcher, subject, .. }) = event else { panic!("forges watch") };
    assert_eq!(
        (watcher, subject),
        (
            WATCHER,
            jig_domain_core::documents::Watched::Application {
                scope: people::ApplicationScope::Deployment,
                connector: wire::FORGE,
                subject: Token::new(0),
            }
        )
    );
}

#[test]
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
fn setup_outcomes_use_the_family_answer_including_unknown() {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small).clients.wire;
    for (outcome, expected) in [
        (None, w::Outcome::Unknown),
        (Some(forge::SetUpAnswer::SettingUp), w::Outcome::SettingUp),
        (
            Some(forge::SetUpAnswer::Refused(forge::SetUpRefusal::NotOffered)),
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why: w::SetUpRefusal::NotOffered }).unwrap(),
            ),
        ),
        (
            Some(forge::SetUpAnswer::Refused(forge::SetUpRefusal::NotUnderTheBox)),
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why: w::SetUpRefusal::NotUnderTheBox })
                    .unwrap(),
            ),
        ),
        (
            Some(forge::SetUpAnswer::Refused(forge::SetUpRefusal::Busy)),
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap(),
            ),
        ),
    ] {
        let outcome = match outcome {
            Some(outcome) => Some(temper_engine_domain::ApplicationAnswer::SetUp(outcome)),
            None => None,
        };
        let bytes = family::answer(outcome, &w::CEILINGS, &jig).unwrap();
        assert_eq!(w::Answer::decode(&w::CEILINGS, &mut Reader::new(&bytes)).unwrap().into_parts().outcome, expected);
    }
}
fn forge_row(
    number: u16,
    kind: forge::Kind,
    mode: forge::Mode,
    connection: forge::Connection,
    offer: forge::Offer,
) -> forge::Row {
    forge::Row {
        number,
        kind,
        mode,
        connection,
        failed: None,
        offer,
        first: None,
        installations: Box::new([]),
        ended: None,
        app: None,
        web: None,
        repositories: Box::new([]),
        owned: None,
        admin: false,
        read_only: false,
        ended_stop: None,
    }
}
fn configured_forge_rows() -> forge::Forges {
    forge::Forges {
        rows: Box::new([
            forge::Row {
                number: 1,
                kind: forge::Kind::Forgejo,
                mode: forge::Mode::Owned,
                connection: forge::Connection::SettingUp { step: Some(forge::SetUpStep::Prepare) },
                failed: None,
                offer: forge::Offer::Offered,
                first: None,
                installations: Box::new([]),
                ended: None,
                app: None,
                web: None,
                repositories: Box::new([]),
                owned: Some(forge::Owned {
                    forgejo: b"17.0.0".as_slice().into(),
                    runner: Some(b"13.2.0".as_slice().into()),
                    password: b"/fake/forge/person_password".as_slice().into(),
                }),
                admin: false,
                read_only: false,
                ended_stop: None,
            },
            forge_row(
                2,
                forge::Kind::GitHub,
                forge::Mode::Connected,
                forge::Connection::NotConnected,
                forge::Offer::NotOffered,
            ),
            forge_row(
                3,
                forge::Kind::Forgejo,
                forge::Mode::Managed,
                forge::Connection::NotSetUp,
                forge::Offer::NotUnderTheBox,
            ),
            forge_row(
                4,
                forge::Kind::Forgejo,
                forge::Mode::Fake,
                forge::Connection::Connecting,
                forge::Offer::NotOffered,
            ),
        ]),
    }
}
#[test]
fn forges_encode_the_domains_rows_without_settings() {
    let value = configured_forge_rows();
    for (failed, expected) in [
        (None, None),
        (Some(forge::FailedStep::Fetch), Some(w::FailedStep::Fetch)),
        (Some(forge::FailedStep::Prepare), Some(w::FailedStep::Prepare)),
        (Some(forge::FailedStep::Run), Some(w::FailedStep::Run)),
        (Some(forge::FailedStep::Connect), Some(w::FailedStep::Connect)),
    ] {
        let mut failure = value.clone();
        failure.rows[0].connection = forge::Connection::SetUpFailed { why: b"stopped".as_slice().into() };
        failure.rows[0].failed = failed;
        let bytes = family::forges(failure, &w::CEILINGS).unwrap();
        let rows = wire::decode_forges(&bytes, &w::CEILINGS).unwrap().into_parts().forges;
        let w::Connection::SetUpFailed(failure) = rows.get(0).unwrap().connection() else { panic!("setup failure") };
        assert_eq!(failure.at(), &expected);
        assert_eq!(failure.why(), b"stopped");
    }
    let bytes = family::forges(value, &w::CEILINGS).unwrap();
    let rows = temper_client_protocol_wire::decode_forges(&bytes, &w::CEILINGS).unwrap().into_parts().forges;
    let expected = [
        (
            1,
            w::Kind::Forgejo,
            w::Mode::Owned,
            w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(w::SetUpStep::Prepare) }).unwrap(),
            ),
            w::Offer::Offered,
        ),
        (2, w::Kind::Github, w::Mode::Connected, w::Connection::NotConnected, w::Offer::NotOffered),
        (3, w::Kind::Forgejo, w::Mode::Managed, w::Connection::NotSetUp, w::Offer::NotUnderTheBox),
        (4, w::Kind::Forgejo, w::Mode::Fake, w::Connection::Connecting, w::Offer::NotOffered),
    ];
    assert_eq!(rows.len(), u32::try_from(expected.len()).unwrap());
    for (row, (number, kind, mode, connection, offer)) in rows.into_iter().zip(expected) {
        assert_eq!(row, &expected_forge_row(number, kind, mode, connection, offer));
    }
}

fn expected_forge_row(
    number: u16,
    kind: w::Kind,
    mode: w::Mode,
    connection: w::Connection,
    offer: w::Offer,
) -> w::ForgeRow {
    w::ForgeRow::new(
        &w::CEILINGS,
        w::ForgeRowParts {
            number,
            kind,
            mode,
            connection,
            offer,
            first: None,
            installations: List::with_capacity(0),
            ended: None,
            app: None,
            web: None,
            repositories: List::with_capacity(0),
            owned: if number == 1 {
                Some(
                    w::OwnedForge::new(
                        &w::CEILINGS,
                        w::OwnedForgeParts {
                            forgejo: b"17.0.0".as_slice().into(),
                            runner: Some(b"13.2.0".as_slice().into()),
                            password: b"/fake/forge/person_password".as_slice().into(),
                        },
                    )
                    .unwrap(),
                )
            } else {
                None
            },
            admin: false,
            read_only: false,
            ended_stop: None,
        },
    )
    .unwrap()
}

#[test]
fn an_exceeded_hold_refusal_keeps_each_bound_and_its_configured_value() {
    use jig_client_protocol_wire::v19 as j;
    for (bound, expected) in [
        (people::Bound::Reasoning, j::Bound::Reasoning),
        (people::Bound::Metadata, j::Bound::Metadata),
        (people::Bound::Items, j::Bound::Items),
        (people::Bound::Answer, j::Bound::Answer),
        (people::Bound::Tools, j::Bound::Tools),
        (people::Bound::HistoryItems, j::Bound::HistoryItems),
        (people::Bound::History, j::Bound::History),
        (people::Bound::Window, j::Bound::Window),
        (people::Bound::Output, j::Bound::Output),
    ] {
        let bytes = refusal_bytes(people::Refusal::Moved {
            now: people::Moved::Hold {
                revision: 7,
                reason: people::HoldReason::Exceeded(people::Exceeded { bound, value: u64::MAX }),
            },
        });
        let mut reader = Reader::new(&bytes);
        let j::Refusal::Moved(moved) = j::Refusal::decode(&j::CEILINGS, &mut reader).expect("jig refusal") else {
            panic!("moved")
        };
        let j::Moved::Hold(hold) = moved.what() else { panic!("hold") };
        let Some(j::CurrentHold::Reason(reason)) = hold.reason() else { panic!("hold reason") };
        let j::HoldCategory::Exceeded(exceeded) = reason.value() else { panic!("exceeded") };
        assert_eq!(exceeded.bound(), &expected);
        assert_eq!(exceeded.value(), u64::MAX);
        assert_eq!(moved.now().value(), 7);
        assert!(reader.is_empty());
    }
}

#[test]
fn setup_projects_and_first_repository_states_translate_without_losing_fields() {
    for project in [forge::SetUpProject::Existing(7), forge::SetUpProject::New(b"Next".as_slice().into())] {
        let wire_project = match &project {
            forge::SetUpProject::Existing(project) => w::SetUpProject::Existing(
                w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: *project }).unwrap(),
            ),
            forge::SetUpProject::New(name) => w::SetUpProject::New(
                w::NewProject::new(&w::CEILINGS, w::NewProjectParts { name: name.clone() }).unwrap(),
            ),
        };
        let record = w::Requesting::new(
            &w::CEILINGS,
            w::RequestingParts {
                request: w::Request::SetUp(
                    w::SetUp::new(
                        &w::CEILINGS,
                        w::SetUpParts {
                            forge: 3,
                            org: b"team".as_slice().into(),
                            repository: b"widget".as_slice().into(),
                            project: wire_project.clone(),
                        },
                    )
                    .unwrap(),
                ),
            },
        )
        .unwrap();
        let event = family::to_root(
            exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&record)),
            &[],
            None,
            &w::CEILINGS,
        )
        .unwrap();
        let root::Event::Party(root::PartyInput::Application { request, .. }) = event else { panic!("application") };
        assert_eq!(
            request,
            root::Application::Forge(forge::Ask::SetUp {
                forge: 3,
                org: b"team".as_slice().into(),
                repository: b"widget".as_slice().into(),
                project: project.clone()
            })
        );
        for state in [
            forge::FirstState::Making,
            forge::FirstState::Made { id: 42 },
            forge::FirstState::Failed { step: forge::CreateStep::Admin },
            forge::FirstState::Failed { step: forge::CreateStep::Org },
            forge::FirstState::Failed { step: forge::CreateStep::Repository },
            forge::FirstState::Failed { step: forge::CreateStep::File },
        ] {
            let first = forge::First {
                org: b"team".as_slice().into(),
                repository: b"widget".as_slice().into(),
                project: project.clone(),
                state,
                label: Box::from(b"temper-host".as_slice()),
            };
            let mut row = forge_row(
                3,
                forge::Kind::Forgejo,
                forge::Mode::Owned,
                forge::Connection::Connecting,
                forge::Offer::NotOffered,
            );
            row.first = Some(first);
            let value = forge::Forges { rows: Box::new([row]) };
            let bytes = family::forges(value, &w::CEILINGS).unwrap();
            let rows = wire::decode_forges(&bytes, &w::CEILINGS).unwrap().into_parts().forges;
            let first = rows.get(0).unwrap().first().as_ref().unwrap();
            assert_eq!(
                (first.org(), first.repository(), first.project()),
                (b"team".as_slice(), b"widget".as_slice(), &wire_project)
            );
            match (state, first.state()) {
                (forge::FirstState::Making, w::FirstState::Making) => {}
                (forge::FirstState::Made { id }, w::FirstState::Made(value)) => assert_eq!(id, value.id()),
                (forge::FirstState::Failed { step }, w::FirstState::Failed(value)) => assert_eq!(
                    value.step(),
                    &match step {
                        forge::CreateStep::Admin => w::CreateStep::Admin,
                        forge::CreateStep::Org => w::CreateStep::Org,
                        forge::CreateStep::Repository => w::CreateStep::Repository,
                        forge::CreateStep::File => w::CreateStep::File,
                    }
                ),
                _ => panic!("first state changed"),
            }
        }
    }
}
#[test]
fn new_project_and_invalid_setup_refusals_translate_to_the_wire() {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small).clients.wire;
    for (domain, wire) in [
        (forge::SetUpRefusal::NewProject, w::SetUpRefusal::NewProject),
        (forge::SetUpRefusal::Invalid, w::SetUpRefusal::Invalid),
    ] {
        let bytes = family::answer(
            Some(temper_engine_domain::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(domain))),
            &w::CEILINGS,
            &jig,
        )
        .unwrap();
        let w::Outcome::SetUpRefused(value) =
            w::Answer::decode(&w::CEILINGS, &mut Reader::new(&bytes)).unwrap().into_parts().outcome
        else {
            panic!("refusal")
        };
        assert_eq!(value.why(), &wire);
    }
}

#[test]
fn demo_start_reaches_the_root_with_its_admission_envelope() {
    let record = w::Requesting::new(&w::CEILINGS, w::RequestingParts { request: w::Request::StartDemo }).unwrap();
    let Ok(root::Event::Party(root::PartyInput::Application {
        reply_to, sign_in, key, digest, request, carried, ..
    })) = family::to_root(
        exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&record)),
        &[],
        None,
        &w::CEILINGS,
    )
    else {
        panic!("start decoded")
    };
    assert_eq!(reply_to, REPLY);
    assert_eq!((sign_in, key, digest, carried), (3, REQUEST_KEY, [7; 32], None));
    assert_eq!(request, root::Application::Forge(forge::Ask::StartDemo));
}

#[test]
fn demo_read_tracks_not_started_running_and_ended() {
    let Err(answer) = family::to_root(demo_read(people::ApplicationScope::Deployment), &[], None, &w::CEILINGS) else {
        panic!("read")
    };
    let kit::Event::Application(kit::FromApplication::Read { record, .. }) = *answer else { panic!("demo read") };
    assert!(record.is_none());
    let mut service = fixtures::demo_service();
    let demo = service.engine.as_mut().unwrap().demo.as_mut().unwrap();
    for (state, expected) in [
        (None, Some(w::DemoState::NotStarted)),
        (Some(forge::DemoState::Running), Some(w::DemoState::Running)),
        (Some(forge::DemoState::Ended), Some(w::DemoState::Ended)),
    ] {
        demo.state = state;
        let Err(answer) =
            family::to_root(demo_read(people::ApplicationScope::Deployment), &[], Some(demo), &w::CEILINGS)
        else {
            panic!("read")
        };
        let kit::Event::Application(kit::FromApplication::Read { record, .. }) = *answer else { panic!("demo read") };
        let got = match record {
            Some(record) => {
                let answer = wire::decode_demo_answer(&record, &w::CEILINGS).unwrap();
                assert_eq!(
                    (answer.provider(), answer.project(), answer.forge()),
                    (demo.provider, demo.project, demo.forge)
                );
                Some(answer.state().clone())
            }
            None => None,
        };
        assert_eq!(got, expected);
    }
}

#[test]
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
fn demo_start_outcomes_encode_every_failure_and_unknown() {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small).clients.wire;
    let group = w::GroupFailed::new(&w::CEILINGS, w::GroupFailedParts { why: b"prepare 1 exited 1".as_slice().into() })
        .unwrap();
    for (answer, expected) in [
        (None, w::Outcome::Unknown),
        (Some(forge::StartAnswer::Running), w::Outcome::Running),
        (
            Some(forge::StartAnswer::NotOffered),
            w::Outcome::StartRefused(
                w::StartRefused::new(&w::CEILINGS, w::StartRefusedParts { why: w::StartRefusal::NotOffered }).unwrap(),
            ),
        ),
        (
            Some(forge::StartAnswer::Failed(forge::StartFailure::Account)),
            w::Outcome::StartFailed(
                w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Account }).unwrap(),
            ),
        ),
        (
            Some(forge::StartAnswer::Failed(forge::StartFailure::Adoption)),
            w::Outcome::StartFailed(
                w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Adoption }).unwrap(),
            ),
        ),
        (
            Some(forge::StartAnswer::Failed(forge::StartFailure::NotConnected)),
            w::Outcome::StartFailed(
                w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::NotConnected }).unwrap(),
            ),
        ),
        (
            Some(forge::StartAnswer::Failed(forge::StartFailure::Group {
                why: b"prepare 1 exited 1".as_slice().into(),
            })),
            w::Outcome::StartFailed(
                w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Group(group) }).unwrap(),
            ),
        ),
    ] {
        let answer = match answer {
            Some(answer) => Some(root::ApplicationAnswer::StartDemo(answer)),
            None => None,
        };
        let bytes = family::answer(answer, &w::CEILINGS, &jig).unwrap();
        assert_eq!(w::Answer::decode(&w::CEILINGS, &mut Reader::new(&bytes)).unwrap().into_parts().outcome, expected);
    }
}

#[test]
fn return_install_and_manifest_paths_are_bounded_visible_paths_on_this_listener() {
    let mut at_bound = [b'a'; 512];
    at_bound[0] = b'/';
    let mut past_bound = [b'a'; 513];
    past_bound[0] = b'/';
    for (name, path, accepted) in [
        ("bound", at_bound.as_slice(), true),
        ("past bound", past_bound.as_slice(), false),
        ("empty", b"".as_slice(), false),
        ("root", b"/", true),
        ("authority", b"//x", false),
        ("backslash", b"/\\x", false),
        ("space", b"/a b", false),
        ("scheme", b"http://x", false),
        ("control", b"/a\n", false),
        ("non ASCII", b"/\x80", false),
        ("delete", b"/\x7f", false),
        ("connect", b"/accounts/connect?provider=1&way=sign-in&source=7", true),
        ("welcome", b"/welcome?at=ready", true),
        ("path colon", b"/a:b?c=d#e", true),
        ("path scheme", b"/https://x", true),
        ("forges", b"/forges?x=1&y=2", true),
    ] {
        assert_eq!(family::return_path(path), accepted, "{name}");
        // Oversized values cannot pass the wire's own bound; all other rows also cross decode.
        if path.len() <= 512 {
            for request in [
                w::Request::Return(
                    w::Return::new(&w::CEILINGS, w::ReturnParts { source: 7, path: path.into() }).unwrap(),
                ),
                w::Request::Install(
                    w::Install::new(&w::CEILINGS, w::InstallParts { forge: 1, back: Some(path.into()) }).unwrap(),
                ),
                w::Request::Connect(
                    w::Connect::new(
                        &w::CEILINGS,
                        w::ConnectParts {
                            forge: 1,
                            how: w::How::GithubManifest(
                                w::GitHubManifest::new(
                                    &w::CEILINGS,
                                    w::GitHubManifestParts { back: Some(path.into()) },
                                )
                                .unwrap(),
                            ),
                        },
                    )
                    .unwrap(),
                ),
            ] {
                let request = w::Requesting::new(&w::CEILINGS, w::RequestingParts { request }).unwrap();
                let result = family::to_root(
                    exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&request)),
                    &[],
                    None,
                    &w::CEILINGS,
                );
                if accepted {
                    let root::Event::Party(root::PartyInput::Application { request, .. }) = result.unwrap() else {
                        panic!("{name}: application")
                    };
                    let actual = match request {
                        root::Application::Return { source, path } => {
                            assert_eq!(source, 7, "{name}");
                            path
                        }
                        root::Application::Forge(ask) => {
                            let (forge::Ask::Install { forge, back: Some(back) }
                            | forge::Ask::Connect { forge, how: forge::How::GitHubManifest { back: Some(back) } }) =
                                ask
                            else {
                                panic!("{name}: install")
                            };
                            assert_eq!(forge, 1, "{name}");
                            back
                        }
                    };
                    assert_eq!(actual.as_ref(), path, "{name}");
                } else {
                    let kit::Event::Application(kit::FromApplication::Undecodable { to }) = *result.unwrap_err() else {
                        panic!("{name}: undecodable")
                    };
                    assert_eq!(to, REPLY, "{name}");
                }
            }
        }
    }
}

#[test]
fn install_without_a_back_is_admitted() {
    let request = w::Requesting::new(
        &w::CEILINGS,
        w::RequestingParts {
            request: w::Request::Install(
                w::Install::new(&w::CEILINGS, w::InstallParts { forge: 1, back: None }).unwrap(),
            ),
        },
    )
    .unwrap();
    let event = family::to_root(
        exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&request)),
        &[],
        None,
        &w::CEILINGS,
    )
    .unwrap();
    let root::Event::Party(root::PartyInput::Application {
        request: root::Application::Forge(forge::Ask::Install { forge: 1, back: None }),
        ..
    }) = event
    else {
        panic!("install without back")
    };
}

#[test]
fn installations_and_the_last_connect_end_encode_unchanged() {
    let bytes = family::forges(
        forge::Forges {
            rows: Box::new([forge::Row {
                number: 1,
                kind: forge::Kind::GitHub,
                mode: forge::Mode::Connected,
                connection: forge::Connection::NotConnected,
                failed: None,
                offer: forge::Offer::NotOffered,
                first: None,
                installations: Box::new([
                    (
                        11,
                        forge::Installation {
                            account: Some((111, Box::from(b"acme".as_slice()))),
                            suspended: true,
                            stray: false,
                        },
                    ),
                    (22, forge::Installation { account: None, suspended: false, stray: true }),
                ]),
                ended: Some(Box::from(b"the pasted value changed".as_slice())),
                app: None,
                web: None,
                repositories: Box::new([]),
                owned: None,
                admin: true,
                read_only: false,
                ended_stop: Some(forge::AdminStop::Refused { status: 403, stage: forge::SetupStage::Token }),
            }]),
        },
        &w::CEILINGS,
    )
    .unwrap();
    let rows = wire::decode_forges(&bytes, &w::CEILINGS).unwrap().into_parts().forges;
    let row = rows.get(0).unwrap();
    assert!(row.admin());
    assert_eq!(
        row.ended_stop(),
        &Some(w::AdminStop::Refused(
            w::AdminRefused::new(&w::CEILINGS, w::AdminRefusedParts { status: 403, stage: w::SetupStage::Token })
                .unwrap()
        ))
    );
    let installation = row.installations().get(0).unwrap();
    assert_eq!((installation.id(), installation.suspended(), installation.stray()), (11, true, false));
    let account = installation.account().as_ref().unwrap();
    assert_eq!((account.id(), account.login()), (111, b"acme".as_slice()));
    let installation = row.installations().get(1).unwrap();
    assert_eq!(
        (installation.id(), installation.account(), installation.suspended(), installation.stray()),
        (22, &None, false, true)
    );
    assert_eq!(row.ended().as_ref().unwrap().why(), b"the pasted value changed");
}

#[test]
fn embedded_client_refusals_keep_their_bytes_at_client_eleven() {
    // The pinned jig v15 generated Unknown golden was [9]. All 422 existing
    // refusal dependency goldens are identical at v16; new tags are appended.
    let mut after = skein_lib::Writer::new(1);
    jig_client_protocol_wire::v19::Refusal::Unknown.encode(&mut after).unwrap();
    assert_eq!(after.finish().as_ref(), &[9]);
    assert_eq!(wire::CLIENT_VERSION, 11);
}

#[test]
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
fn owned_details_fit_their_bounds_and_an_overlong_field_drops_only_the_details() {
    for (forgejo, runner, password, kept) in [
        (64, Some(64), 512, true),
        (64, None, 512, true),
        (65, Some(64), 512, false),
        (64, Some(65), 512, false),
        (64, Some(64), 513, false),
    ] {
        let value = forge::Forges {
            rows: Box::new([forge::Row {
                number: 1,
                kind: forge::Kind::Forgejo,
                mode: forge::Mode::Owned,
                connection: forge::Connection::Connecting,
                failed: None,
                offer: forge::Offer::NotOffered,
                first: None,
                installations: Box::new([]),
                ended: None,
                app: None,
                web: None,
                repositories: Box::new([]),
                owned: Some(forge::Owned {
                    forgejo: Box::from(&[b'v'; 65][..forgejo]),
                    runner: match runner {
                        Some(len) => Some(Box::from(&[b'r'; 65][..len])),
                        None => None,
                    },
                    password: Box::from(&[b'/'; 513][..password]),
                }),
                admin: false,
                read_only: false,
                ended_stop: None,
            }]),
        };
        let bytes = family::forges(value, &w::CEILINGS).expect("the row is still available");
        let rows = temper_client_protocol_wire::decode_forges(&bytes, &w::CEILINGS).unwrap();
        assert_eq!(rows.forges().len(), 1);
        let owned = rows.forges().as_slice()[0].owned();
        assert_eq!(owned.is_some(), kept);
        if let Some(owned) = owned {
            assert_eq!(owned.forgejo().len(), forgejo);
            assert_eq!(
                match owned.runner() {
                    Some(runner) => Some(runner.len()),
                    None => None,
                },
                runner
            );
            assert_eq!(owned.password().len(), password);
        }
    }
}

#[test]
fn accept_decodes_only_a_nonzero_installation_and_keeps_its_id() {
    for installation in [0, 22, u64::MAX] {
        let request = w::Requesting::new(
            &w::CEILINGS,
            w::RequestingParts {
                request: w::Request::Accept(
                    w::Accept::new(&w::CEILINGS, w::AcceptParts { forge: 1, installation }).unwrap(),
                ),
            },
        )
        .unwrap();
        let result = family::to_root(
            exchange(wire::FORGE, wire::CLIENT_VERSION, wire::requesting(&request)),
            &[],
            None,
            &w::CEILINGS,
        );
        if installation == 0 {
            assert!(result.is_err());
        } else {
            let root::Event::Party(root::PartyInput::Application {
                request: root::Application::Forge(forge::Ask::Accept { forge: 1, installation: id }),
                ..
            }) = result.unwrap()
            else {
                panic!("accept")
            };
            assert_eq!(id.get(), installation);
        }
    }
}
#[test]
fn accept_answers_use_the_appended_success_and_existing_refusals() {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small).clients.wire;
    for (answer, expected) in [
        (
            forge::AcceptAnswer::Accepted { installation: 22 },
            w::Outcome::Accepted(
                w::AcceptedInstallation::new(&w::CEILINGS, w::AcceptedInstallationParts { installation: 22 }).unwrap(),
            ),
        ),
        (
            forge::AcceptAnswer::Refused(forge::ForgeRefusal::Role),
            w::Outcome::ConnectRefused(
                w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::Role }).unwrap(),
            ),
        ),
        (
            forge::AcceptAnswer::ForgeRefused(forge::AdoptionRefusal::NotFound),
            w::Outcome::ForgeRefused(
                w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason: w::ForgeReason::NotFound }).unwrap(),
            ),
        ),
    ] {
        let bytes = family::answer(Some(root::ApplicationAnswer::Accept(answer)), &w::CEILINGS, &jig).unwrap();
        assert_eq!(w::Answer::decode(&w::CEILINGS, &mut Reader::new(&bytes)).unwrap().into_parts().outcome, expected);
    }
}
