use super::{connect, fixtures as f};
use crate::{
    Choice, Event, Request,
    forge::{self, admin::Field, connect::Shown},
};
use jig_client::{Down, Up, contract as c};
use skein_lib::{List, Queue, Secret, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};
const FORGE: u16 = 3;
const TOKEN: &[u8] = b"admin-token-placeholder-0123";
fn event(root: &mut crate::Domain, event: Event) -> Queue<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::step(root, &f::env(&f::limits()), event, &mut out);
    out
}
fn act(root: &mut crate::Domain, act: forge::Act) -> Queue<Request> {
    event(root, Event::Forge(forge::Event::Act { act }))
}
fn token(root: &mut crate::Domain, bytes: &[u8]) {
    event(root, Event::Forge(forge::Event::AdminToken { value: Secret::new(bytes.into()) }));
}
fn row(
    admin: bool,
    connection: w::Connection,
    stop: Option<w::AdminStop>,
    first: Option<w::FirstRepository>,
) -> w::ForgeRow {
    w::ForgeRow::new(
        &w::CEILINGS,
        w::ForgeRowParts {
            number: FORGE,
            kind: w::Kind::Forgejo,
            mode: w::Mode::Managed,
            connection,
            offer: w::Offer::NotOffered,
            first,
            installations: List::with_capacity(0),
            ended: None,
            app: None,
            web: None,
            repositories: List::with_capacity(0),
            owned: None,
            admin,
            read_only: false,
            ended_stop: stop,
        },
    )
    .unwrap()
}
fn deliver(root: &mut crate::Domain, watch: Token, row: w::ForgeRow) -> Queue<Request> {
    let mut rows = List::with_capacity(1);
    rows.push(row).unwrap();
    event(
        root,
        Event::Client {
            event: jig_client::Event::Up {
                up: Up::FamilyDelivered {
                    watch,
                    record: jig_client::Family {
                        family: wire::FORGE,
                        version: wire::CLIENT_VERSION,
                        bytes: wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges: rows }).unwrap()),
                    },
                },
            },
        },
    )
}
fn opened() -> (crate::Domain, Token) {
    let (mut root, _, watch) = connect::opened(true);
    deliver(&mut root, watch, row(true, w::Connection::NotConnected, None, None));
    (root, watch)
}
fn send(root: &mut crate::Domain) -> Queue<Request> {
    act(root, forge::Act::ConnectAdmin { forge: FORGE })
}
fn write(root: &mut crate::Domain, field: Field, bytes: &[u8]) {
    act(root, forge::Act::AdminWrite { field, text: bytes.into() });
}
#[test]
fn only_the_configured_admin_form_has_a_card() {
    for (admin, expected) in [(false, &[Choice::Elsewhere][..]), (true, &[Choice::Forge(FORGE), Choice::Elsewhere][..])]
    {
        let (mut root, watch) = opened();
        deliver(&mut root, watch, row(admin, w::Connection::NotConnected, None, None));
        assert_eq!(forge::connect::choices(root.forges(), None).as_slice(), expected);
    }
}
#[test]
fn empty_tokens_and_names_and_invalid_labels_send_nothing() {
    let (mut root, _) = opened();
    assert!(send(&mut root).is_empty());
    assert!(root.forge().admin_form().checked);
    for field in [Field::Organization, Field::Repository, Field::Project] {
        let (mut root, _) = opened();
        token(&mut root, TOKEN);
        write(&mut root, field, b"");
        assert!(send(&mut root).is_empty());
    }
    for label in [b"".as_slice(), b"a b", b"a/b", b"a:b", b"a\n", "é".as_bytes(), &[b'a'; 65]] {
        let (mut root, _) = opened();
        token(&mut root, TOKEN);
        write(&mut root, Field::Label, label);
        assert!(send(&mut root).is_empty());
        assert!(!root.forge().admin_form().valid_label());
    }
}
#[test]
fn the_bundle_bound_includes_its_six_byte_header() {
    for (bytes, accepted) in [(&[b'x'; 4090][..], true), (&[b'x'; 4091][..], false)] {
        let (mut root, _) = opened();
        token(&mut root, bytes);
        write(&mut root, Field::Label, &[b'a'; 64]);
        assert_eq!(root.forge().admin_form().too_long, !accepted);
        assert_eq!(send(&mut root).is_empty(), !accepted);
    }
}
#[test]
#[expect(clippy::disallowed_macros, reason = "capture Debug to check secret redaction")]
fn the_token_is_secret_and_carried_once_with_the_first_ask() {
    let (mut root, _) = opened();
    write(&mut root, Field::Repository, b"widget");
    assert_eq!(root.forge().admin_form().fields[2].as_ref(), b"widget");
    write(&mut root, Field::Project, b"Next");
    write(&mut root, Field::Repository, b"app");
    write(&mut root, Field::Organization, b"team");
    write(&mut root, Field::Label, b"linux.arm_64-1");
    let input = forge::Event::AdminToken { value: Secret::new(TOKEN.into()) };
    assert!(skein_lib::bytes::find(format!("{input:?}").as_bytes(), TOKEN).is_none());
    event(&mut root, Event::Forge(input));
    assert!(skein_lib::bytes::find(format!("{root:?}").as_bytes(), TOKEN).is_none());
    let mut out = send(&mut root);
    let mut count = 0_u32;
    while let Some(request) = out.pop() {
        assert!(skein_lib::bytes::find(format!("{request:?}").as_bytes(), TOKEN).is_none());
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Send { request, value: Some(value), .. } },
        } = request
        {
            count += 1;
            let mut expected = vec![1, 1, 1, 0, 28, 0];
            expected.extend_from_slice(TOKEN);
            assert_eq!(value.expose(), expected);
            let jig_client::Ask::Family { record, .. } = request else { panic!("family") };
            assert!(skein_lib::bytes::find(&record.bytes, TOKEN).is_none());
            let w::Request::Connect(connect) =
                wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request
            else {
                panic!("connect")
            };
            let w::How::ForgejoAdmin(admin) = connect.how() else { panic!("admin") };
            let first = admin.first().as_ref().unwrap();
            assert_eq!(
                (first.org(), first.repository(), first.label()),
                (b"team".as_slice(), b"app".as_slice(), b"linux.arm_64-1".as_slice())
            );
            let w::SetUpProject::New(project) = first.project() else { panic!("new") };
            assert_eq!(project.name(), b"Next");
        }
    }
    assert_eq!(count, 1);
    assert!(!root.forge().admin_form().has_token());
    assert!(send(&mut root).is_empty());
}
#[test]
fn each_stop_returns_to_details_and_erases_only_the_token() {
    for stop in [
        w::AdminStop::NotAdmin,
        w::AdminStop::Unmarked,
        w::AdminStop::Other,
        w::AdminStop::Refused(
            w::AdminRefused::new(&w::CEILINGS, w::AdminRefusedParts { status: 401, stage: w::SetupStage::Admin })
                .unwrap(),
        ),
        w::AdminStop::Refused(
            w::AdminRefused::new(&w::CEILINGS, w::AdminRefusedParts { status: 403, stage: w::SetupStage::Bot })
                .unwrap(),
        ),
    ] {
        let (mut root, watch) = opened();
        write(&mut root, Field::Organization, b"team");
        write(&mut root, Field::Repository, b"widget");
        write(&mut root, Field::Project, b"Next");
        write(&mut root, Field::Label, b"linux-arm");
        token(&mut root, TOKEN);
        deliver(&mut root, watch, row(true, w::Connection::NotConnected, Some(stop), None));
        assert_eq!(
            forge::connect::step(Some(Choice::Forge(FORGE)), root.forges(), None),
            Shown::Details(Choice::Forge(FORGE))
        );
        assert!(!root.forge().admin_form().has_token());
        for (field, expected) in [
            (Field::Organization, b"team".as_slice()),
            (Field::Repository, b"widget"),
            (Field::Project, b"Next"),
            (Field::Label, b"linux-arm"),
        ] {
            assert_eq!(root.forge().admin_form().fields[field.index()].as_ref(), expected);
        }
    }
}
fn connected() -> w::Connection {
    w::Connection::Connected(
        w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 7, login: b"temper".as_slice().into() }).unwrap(),
    )
}
fn first() -> w::FirstRepository {
    w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::New(
                w::NewProject::new(&w::CEILINGS, w::NewProjectParts { name: b"hello".as_slice().into() }).unwrap(),
            ),
            state: w::FirstState::Made(w::Made::new(&w::CEILINGS, w::MadeParts { id: 42 }).unwrap()),
            label: b"docker".as_slice().into(),
        },
    )
    .unwrap()
}
#[test]
fn connecting_then_ready_starts_the_project_only_once() {
    let (mut root, watch) = opened();
    token(&mut root, TOKEN);
    send(&mut root);
    deliver(&mut root, watch, row(true, w::Connection::Connecting, None, None));
    assert_eq!(forge::connect::step(Some(Choice::Forge(FORGE)), root.forges(), None), Shown::SettingUp(FORGE));
    let mut out = deliver(&mut root, watch, row(true, connected(), None, Some(first())));
    let mut projects = 0_u32;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Send { request: jig_client::Ask::Jig(ask), .. } },
        } = request
        {
            let c::Ask::MakeProject { name, policy } = *ask else { panic!("MakeProject") };
            assert_eq!(name.bytes(), b"hello");
            assert_eq!(policy, 1);
            projects += 1;
        }
    }
    assert_eq!(projects, 1);
    assert_eq!(forge::connect::step(Some(Choice::Forge(FORGE)), root.forges(), None), Shown::Ready(FORGE));
    assert!(deliver(&mut root, watch, row(true, connected(), None, Some(first()))).is_empty());
}
#[test]
fn signing_out_erases_the_admin_draft() {
    let (mut root, _) = opened();
    token(&mut root, TOKEN);
    write(&mut root, Field::Repository, b"widget");
    event(&mut root, Event::SignedOut);
    assert!(!root.forge().admin_form().has_token());
    assert_eq!(root.forge().admin_form().fields[1].as_ref(), b"hello");
}

#[test]
fn a_stop_before_its_acceptance_answer_allows_a_fresh_paste() {
    let (mut root, watch) = opened();
    write(&mut root, Field::Organization, b"team");
    token(&mut root, TOKEN);
    send(&mut root);
    deliver(&mut root, watch, row(true, w::Connection::NotConnected, Some(w::AdminStop::NotAdmin), None));
    token(&mut root, TOKEN);
    assert!(root.forge().admin_form().has_token());
    assert_eq!(root.forge().admin_form().fields[0].as_ref(), b"team");
    assert!(!send(&mut root).is_empty());
}
#[test]
fn create_stops_keep_the_fields_and_empty_the_token() {
    for step in [w::CreateStep::Admin, w::CreateStep::Org, w::CreateStep::Repository, w::CreateStep::File] {
        let (mut root, watch) = opened();
        token(&mut root, TOKEN);
        let mut parts = first().into_parts();
        parts.state = w::FirstState::Failed(w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step }).unwrap());
        deliver(
            &mut root,
            watch,
            row(true, w::Connection::NotConnected, None, Some(w::FirstRepository::new(&w::CEILINGS, parts).unwrap())),
        );
        assert!(!root.forge().admin_form().has_token());
        assert_eq!(
            forge::connect::step(Some(Choice::Forge(FORGE)), root.forges(), None),
            Shown::Details(Choice::Forge(FORGE))
        );
        assert_eq!(root.forge().admin_form().fields[1].as_ref(), b"hello");
    }
}

#[test]
fn an_admin_first_adoption_advances_only_the_welcome() {
    for welcome in [true, false] {
        let (mut root, watch) = opened();
        let address = if welcome {
            crate::Address::Welcome { step: crate::Step::Forge { forge: Some(Choice::Forge(FORGE)) } }
        } else {
            crate::Address::ConnectForge { forge: Some(Choice::Forge(FORGE)) }
        };
        event(&mut root, Event::Went { place: crate::Place::Address(address) });
        token(&mut root, TOKEN);
        send(&mut root);
        let out = deliver(&mut root, watch, row(true, connected(), None, Some(first())));
        let mut project_attempt = None;
        for request in &out {
            if let Request::Client {
                request:
                    jig_client::Request::Down { down: Down::Send { attempt, request: jig_client::Ask::Jig(ask), .. } },
            } = request
                && let c::Ask::MakeProject { .. } = ask.as_ref()
            {
                project_attempt = Some(*attempt);
            }
        }
        let out = event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: Up::Answer {
                        attempt: project_attempt.expect("the admin's first repository needs its project"),
                        envelope: skein_lib::RequestEnvelope::Final,
                        answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                            c::OwnerOutcome::ProjectMade { project: 7 },
                        )))),
                        why: None,
                    },
                },
            },
        );
        assert!(!out.is_empty(), "the project answer asks to adopt the first repository");
        let pending = root.forge().set_up().unwrap().adoption.as_ref().expect("first adoption pending");
        let request = pending.request;
        let out = event(
            &mut root,
            Event::Forge(forge::Event::Answered {
                request,
                answer: forge::Answer::Adopted(forge::Adopted {
                    forge: FORGE,
                    repository: 42,
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    role: forge::Role::Owned,
                    home: true,
                    kinds: forge::Kinds {
                        read: true,
                        push: true,
                        open: true,
                        land: true,
                        review: true,
                        status: true,
                        comment: true,
                        issue: true,
                        branch: true,
                    },
                    protection: forge::Protection::Unknown,
                }),
            }),
        );
        let mut moves = 0;
        for request in &out {
            if let Request::Show { address, replace } = request {
                assert_eq!(address, &crate::Address::Welcome { step: crate::Step::Model { way: None } });
                assert!(!replace);
                moves += 1;
            }
        }
        assert_eq!(moves, u32::from(welcome));
    }
}
