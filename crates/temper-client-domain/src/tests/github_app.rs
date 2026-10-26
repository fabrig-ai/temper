use super::{connect, fixtures as f};
use crate::{Event, Request, forge};
use jig_client::{Down, contract as c};
use skein_lib::{Queue, Secret};
use temper_client_protocol_wire::{self as wire, v1 as w};
fn event(root: &mut crate::Domain, event: Event) -> Queue<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::step(root, &f::env(&f::limits()), event, &mut out);
    out
}
fn act(root: &mut crate::Domain, act: forge::Act) -> Queue<Request> {
    event(root, Event::Forge(forge::Event::Act { act }))
}
fn key(root: &mut crate::Domain, bytes: &[u8]) {
    event(root, Event::Forge(forge::Event::AppKey { value: Secret::new(bytes.into()) }));
}
fn id(root: &mut crate::Domain, bytes: &[u8]) {
    act(root, forge::Act::AppWrite { text: bytes.into() });
}
#[test]
fn an_app_of_ones_own_is_refused_on_the_page_without_an_id_or_key() {
    let (mut root, _, _) = connect::opened(true);
    assert!(act(&mut root, forge::Act::ConnectApp { forge: 2 }).is_empty());
    assert!(root.forge().app_form().checked);
    assert!(root.forge().app().is_none());
    id(&mut root, b"7");
    assert!(act(&mut root, forge::Act::ConnectApp { forge: 2 }).is_empty());
    key(&mut root, b"fixture");
    id(&mut root, b"");
    assert!(act(&mut root, forge::Act::ConnectApp { forge: 2 }).is_empty());
    assert!(root.forge().app_form().has_key());
}
#[test]
fn an_app_id_takes_only_digits_and_a_nonzero_u64() {
    let (mut root, _, _) = connect::opened(true);
    for (text, expected) in [
        (b"0".as_slice(), None),
        (b"+1", None),
        (b"1a", None),
        (b" 1", None),
        (b"18446744073709551616", None),
        (b"18446744073709551615", Some(u64::MAX)),
        (b"007", Some(7)),
    ] {
        id(&mut root, text);
        assert_eq!(root.forge().app_form().number(), expected);
    }
}
#[test]
fn a_key_past_the_pasted_bundle_bound_is_refused_on_the_page() {
    let (mut root, _, _) = connect::opened(true);
    id(&mut root, b"7");
    key(&mut root, &[b'x'; 4090]);
    assert!(act(&mut root, forge::Act::ConnectApp { forge: 2 }).is_empty());
    assert!(root.forge().app_form().too_long);
    assert!(!root.forge().app_form().has_key());
}
#[expect(clippy::disallowed_macros, reason = "the test captures Debug to verify secret redaction")]
fn carried(pem: &[u8]) -> Secret {
    let (mut root, _, _) = connect::opened(true);
    id(&mut root, b"7");
    key(&mut root, pem);
    assert!(skein_lib::bytes::find(format!("{root:?}").as_bytes(), pem).is_none());
    let mut out = act(&mut root, forge::Act::ConnectApp { forge: 2 });
    let mut carried = None;
    while let Some(request) = out.pop() {
        assert!(skein_lib::bytes::find(format!("{request:?}").as_bytes(), pem).is_none());
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Save { .. } } | jig_client::Request::Save { .. },
        } = &request
        {
            panic!("a carried key never enters the small store");
        }
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Send { request, value: Some(value), .. } },
        } = request
        {
            assert!(carried.is_none(), "exactly one carried value");
            let jig_client::Ask::Family { scope, record } = request else { panic!("family") };
            assert_eq!(scope, c::Scope::Deployment);
            assert!(skein_lib::bytes::find(&record.bytes, pem).is_none());
            let bounds = forge::wire_limits(&f::limits().forge).unwrap();
            let w::Request::Connect(connect) =
                wire::decode_requesting(&record.bytes, &bounds).unwrap().into_parts().request
            else {
                panic!("Connect")
            };
            assert_eq!(connect.forge(), 2);
            let w::How::GithubApp(app) = connect.how() else { panic!("pasted App") };
            assert_eq!(app.id(), 7);
            carried = Some(value);
        }
    }
    assert!(!root.forge().app_form().has_key());
    assert_eq!(root.forge().app_form().id.as_ref(), b"7");
    assert!(skein_lib::bytes::find(format!("{root:?}").as_bytes(), pem).is_none());
    carried.expect("carried key")
}
#[test]
#[expect(clippy::disallowed_macros, reason = "the test spells PEM forms and compares their base64 bodies")]
fn password_lf_and_crlf_forms_carry_the_same_body_with_the_armour_separators() {
    let body = "A".repeat(64);
    let flat = format!("-----BEGIN RSA PRIVATE KEY-----{body}{body}-----END RSA PRIVATE KEY-----");
    let restored = format!("-----BEGIN RSA PRIVATE KEY-----\n{body}{body}\n-----END RSA PRIVATE KEY-----");
    let file = format!("-----BEGIN RSA PRIVATE KEY-----\n{body}\n{body}\n-----END RSA PRIVATE KEY-----\n");
    let crlf = file.replace('\n', "\r\n");
    for (input, expected) in [
        (flat.as_bytes(), restored.as_bytes()),
        (restored.as_bytes(), restored.as_bytes()),
        (format!("{restored}\n").as_bytes(), restored.as_bytes()),
        (file.as_bytes(), file.strip_suffix('\n').unwrap().as_bytes()),
        (crlf.as_bytes(), crlf.strip_suffix("\r\n").unwrap().as_bytes()),
    ] {
        let value = carried(input);
        assert_eq!(value.expose(), forge::forges::pasted_app_key(expected).unwrap().expose());
        let pem = &value.expose()[7..];
        assert!(
            pem.starts_with(b"-----BEGIN RSA PRIVATE KEY-----\n")
                || pem.starts_with(b"-----BEGIN RSA PRIVATE KEY-----\r\n")
        );
        assert!(pem.ends_with(b"\n-----END RSA PRIVATE KEY-----"));
        let content = pem
            .strip_prefix(b"-----BEGIN RSA PRIVATE KEY-----")
            .unwrap()
            .strip_suffix(b"-----END RSA PRIVATE KEY-----")
            .unwrap();
        let mut body_bytes = skein_lib::Writer::new(128);
        for byte in content {
            if *byte != b'\r' && *byte != b'\n' {
                body_bytes.put(&[*byte]).unwrap();
            }
        }
        assert_eq!(body_bytes.finish().as_ref(), format!("{body}{body}").as_bytes());
    }
}
#[test]
fn restoration_trims_outer_whitespace_and_changes_only_missing_rsa_armour_separators() {
    for (input, expected) in [
        (b" \t\r\nfake-private-key\nsecond line\n \t".as_slice(), b"fake-private-key\nsecond line".as_slice()),
        (
            b"-----BEGIN PRIVATE KEY-----body-----END PRIVATE KEY-----",
            b"-----BEGIN PRIVATE KEY-----body-----END PRIVATE KEY-----",
        ),
        (
            b"-----BEGIN RSA PRIVATE KEY-----a b\tc!-----END RSA PRIVATE KEY-----",
            b"-----BEGIN RSA PRIVATE KEY-----\na b\tc!\n-----END RSA PRIVATE KEY-----",
        ),
        (
            b"-----BEGIN RSA PRIVATE KEY-----\rbody\r-----END RSA PRIVATE KEY-----",
            b"-----BEGIN RSA PRIVATE KEY-----\rbody\r\n-----END RSA PRIVATE KEY-----",
        ),
        (b"-----BEGIN RSA PRIVATE KEY-----body", b"-----BEGIN RSA PRIVATE KEY-----\nbody"),
        (b"body-----END RSA PRIVATE KEY-----", b"body\n-----END RSA PRIVATE KEY-----"),
    ] {
        assert_eq!(carried(input).expose(), forge::forges::pasted_app_key(expected).unwrap().expose());
    }
}
#[test]
#[expect(clippy::disallowed_macros, reason = "the test constructs a PEM at the bundle bound")]
fn the_bundle_bound_includes_restored_separators() {
    assert_eq!(carried(&[b'x'; 4089]).len(), 4096);
    let pem = format!("-----BEGIN RSA PRIVATE KEY-----{}-----END RSA PRIVATE KEY-----", "A".repeat(4029));
    assert_eq!(pem.len(), 4089);
    let (mut root, _, _) = connect::opened(true);
    id(&mut root, b"7");
    key(&mut root, pem.as_bytes());
    assert!(act(&mut root, forge::Act::ConnectApp { forge: 2 }).is_empty());
    assert!(root.forge().app_form().too_long);
    assert!(root.forge().app_form().has_key());
}
#[test]
#[expect(clippy::disallowed_macros, reason = "the test captures Debug to verify secret redaction")]
fn each_forge_refusal_is_said_in_forge_md_words() {
    for (why, words) in [
        (w::ForgeRefusal::Role, "Only the deployment's owner connects a forge."),
        (w::ForgeRefusal::NoSuchForge, "No forge 2 is configured."),
        (w::ForgeRefusal::NotTaken, "Forge 2 does not take an App."),
        (w::ForgeRefusal::Connected, "GitHub is connected already."),
        (w::ForgeRefusal::NotConnected, "Connect the App first."),
        (w::ForgeRefusal::Busy, "temper is busy with forge 2: try again when it ends."),
        (w::ForgeRefusal::NoKey, "The key did not reach temper: paste it again."),
    ] {
        let pasted = b"fake-private-key-refused";
        let (mut root, _, _) = connect::opened(true);
        id(&mut root, b"7");
        key(&mut root, pasted);
        let mut out = act(&mut root, forge::Act::ConnectApp { forge: 2 });
        let mut attempt = None;
        while let Some(request) = out.pop() {
            if let Request::Client { request: jig_client::Request::Down { down: Down::Send { attempt: sent, .. } } } =
                request
            {
                attempt = Some(sent);
            }
        }
        let record = w::Answer::new(
            &w::CEILINGS,
            w::AnswerParts {
                outcome: w::Outcome::ConnectRefused(
                    w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why }).unwrap(),
                ),
            },
        )
        .unwrap();
        event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Answer {
                        attempt: attempt.unwrap(),
                        envelope: skein_lib::RequestEnvelope::Final,
                        answer: Some(jig_client::Answered::Family(jig_client::Family {
                            family: wire::FORGE,
                            version: wire::CLIENT_VERSION,
                            bytes: wire::answer(&record),
                        })),
                        why: None,
                    },
                },
            },
        );
        let answer = root.forge().app().unwrap().answer.as_ref().unwrap();
        let refusal = forge::words::app_refusal(answer.clone(), 2).unwrap();
        assert_eq!(refusal.as_ref(), words.as_bytes());
        assert!(skein_lib::bytes::find(&refusal, pasted).is_none());
        assert!(!root.forge().app_form().has_key());
        assert_eq!(root.forge().app_form().id.as_ref(), b"7");
        assert!(!format!("{root:?}").contains("fake-private-key-refused"));
    }
}

#[test]
fn a_first_organization_named_as_the_bot_asks_for_another_name() {
    let words = forge::words::app_refusal(forge::github::Answer::Refused(w::ForgeRefusal::FirstIsBot), 3).unwrap();
    assert_eq!(words.as_ref(), b"That organization has the name of temper's bot on this forge, and on Forgejo users and organizations share names: name another organization.");
}

fn github_row() -> forge::forges::Row {
    forge::forges::Row {
        number: 2,
        kind: forge::forges::Kind::GitHub,
        mode: forge::forges::Mode::Connected,
        connection: forge::forges::Connection::Connected { id: 9, login: b"my-app[bot]".as_slice().into() },
        offer: forge::forges::Offer::NotOffered,
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
#[test]
fn a_connected_app_without_an_accepted_installation_asks_to_install() {
    use forge::{
        connect::{Shown, step},
        forges::{Installation, State},
    };
    for (suspended, stray, expected) in
        [(false, false, Shown::Ready(2)), (true, false, Shown::SettingUp(2)), (false, true, Shown::SettingUp(2))]
    {
        let mut row = github_row();
        row.installations = Box::new([Installation { id: 1, account: None, suspended, stray }]);
        assert_eq!(step(Some(crate::Choice::Forge(2)), &State::Rows(Box::new([row])), None), expected);
    }
    assert_eq!(step(Some(crate::Choice::Forge(2)), &State::Rows(Box::new([github_row()])), None), Shown::SettingUp(2));
}
#[test]
fn a_stopped_connect_returns_to_details_with_its_reason() {
    use forge::{
        connect::{Shown, step},
        forges::{Connection, State},
    };
    for connection in [Connection::NotConnected, Connection::Stopped { reason: b"stopped".as_slice().into() }] {
        let mut row = github_row();
        row.connection = connection;
        row.ended = Some(b"the pasted value changed".as_slice().into());
        assert_eq!(
            forge::words::last_connect(&row).unwrap().as_ref(),
            b"The last connect stopped: the pasted value changed"
        );
        assert_eq!(
            step(Some(crate::Choice::Forge(2)), &State::Rows(Box::new([row])), None),
            Shown::Details(crate::Choice::Forge(2))
        );
    }
}
#[test]
fn the_github_row_names_its_app_by_slug_and_lists_its_installations() {
    use forge::forges::{Account, Installation};
    let mut row = github_row();
    assert_eq!(forge::words::app_line(&row).unwrap().as_ref(), "my-app · installed nowhere yet".as_bytes());
    row.installations = Box::new([
        Installation {
            id: 1,
            account: Some(Account { id: 9, login: b"acme".as_slice().into() }),
            suspended: false,
            stray: false,
        },
        Installation { id: 2, account: None, suspended: true, stray: true },
    ]);
    assert_eq!(
        forge::words::app_line(&row).unwrap().as_ref(),
        "my-app · acme, installation 2 · suspended · not accepted · uninstalled at the next reconcile".as_bytes()
    );
    assert!(forge::forges::accepted(&row));
}
#[test]
fn install_sends_its_way_back_and_leaves_once_only_from_the_answer() {
    let (mut root, _, _) = connect::opened(true);
    let mut out = act(&mut root, forge::Act::Install { forge: 2, back: b"/forges/connect?forge=2".as_slice().into() });
    let mut attempt = None;
    while let Some(request) = out.pop() {
        if let Request::Leave { .. } = &request {
            panic!("only the first answer may leave the page");
        }
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Send { attempt: sent, request, value, .. } },
        } = request
        {
            attempt = Some(sent);
            assert!(value.is_none());
            let jig_client::Ask::Family { record, .. } = request else { panic!("family") };
            let w::Request::Install(install) =
                wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request
            else {
                panic!("Install")
            };
            assert_eq!(install.back().as_deref(), Some(b"/forges/connect?forge=2".as_slice()));
        }
    }
    let attempt = attempt.unwrap();
    let answer = jig_client::Answered::Family(jig_client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::answer(
            &w::Answer::new(
                &w::CEILINGS,
                w::AnswerParts {
                    outcome: w::Outcome::Link(
                        w::Link::new(
                            &w::CEILINGS,
                            w::LinkParts { address: b"https://fake.invalid/install".as_slice().into() },
                        )
                        .unwrap(),
                    ),
                },
            )
            .unwrap(),
        ),
    });
    let up = jig_client::Up::Answer {
        attempt,
        envelope: skein_lib::RequestEnvelope::Final,
        answer: Some(answer),
        why: None,
    };
    let mut out = event(&mut root, Event::Client { event: jig_client::Event::Up { up: up.clone() } });
    let mut leaves = 0_u32;
    while let Some(request) = out.pop() {
        if let Request::Leave { to, .. } = request {
            leaves += 1;
            assert_eq!(to.as_ref(), b"https://fake.invalid/install");
        }
    }
    assert_eq!(leaves, 1);
    let mut duplicate = event(&mut root, Event::Client { event: jig_client::Event::Up { up } });
    while let Some(request) = duplicate.pop() {
        if let Request::Leave { .. } = &request {
            panic!("only the first answer may leave the page");
        }
    }
}

fn manifest(root: &mut crate::Domain) -> Queue<Request> {
    act(root, forge::Act::ConnectManifest { forge: 2, back: b"/forges/connect".as_slice().into() })
}
#[test]
fn manifest_connect_needs_a_party() {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    assert!(manifest(&mut root).is_empty());
}
#[test]
fn manifest_connect_waits_for_a_current_service() {
    let (mut root, _, _) = connect::opened(true);
    event(&mut root, Event::Forge(forge::Event::Served { served: forge::Served::OutOfDate }));
    assert!(manifest(&mut root).is_empty());
}
#[test]
fn manifest_connect_waits_for_the_pending_app_answer() {
    let (mut root, _, _) = connect::opened(true);
    manifest(&mut root);
    assert!(manifest(&mut root).is_empty());
}
#[test]
fn manifest_connect_shares_the_pending_install_guard() {
    let (mut root, _, _) = connect::opened(true);
    act(&mut root, forge::Act::Install { forge: 2, back: b"/forges/connect".as_slice().into() });
    assert!(manifest(&mut root).is_empty());
}

#[test]
fn github_connecting_words_distinguish_none_from_strays_and_respect_read_only() {
    use forge::{
        forges::{Account, Installation},
        words,
    };
    let mut row = github_row();
    assert_eq!(words::forge_connection(&row).as_ref(), b"Connecting: installed nowhere yet");
    row.installations = Box::new([Installation {
        id: 22,
        account: Some(Account { id: 2, login: b"acme".as_slice().into() }),
        stray: true,
        suspended: false,
    }]);
    assert_eq!(words::forge_connection(&row).as_ref(), b"Connecting: no installation accepted yet");
    assert_eq!(
        words::app_line(&row).unwrap().as_ref(),
        "my-app · acme · not accepted · uninstalled at the next reconcile".as_bytes()
    );
    row.read_only = true;
    assert_eq!(
        words::app_line(&row).unwrap().as_ref(),
        "my-app · acme · not accepted · kept: the App is read-only".as_bytes()
    );
    row.installations[0].stray = false;
    assert_eq!(words::forge_connection(&row).as_ref(), b"Connected as my-app[bot] (9)");
    row.installations[0].suspended = true;
    assert_eq!(words::forge_connection(&row).as_ref(), b"Connected as my-app[bot] (9)");
}
