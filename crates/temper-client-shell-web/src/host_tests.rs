//! 16:52Z ruling: the host owns secret/clearing/target evidence; Chromium owns navigation and re-entry.
use super::*;
use jig_client::contract as c;
use skein_ui::{EntryKind, Role};
#[path = "host_tests_authorize.rs"]
mod authorize;
#[path = "host_tests_departure.rs"]
mod departure;
const KEY: &[u8] = b"private-host-key";
const ADDRESS: &[u8] = b"http://127.0.0.1:9/authorize?state=test";
fn fixture() -> Host {
    fixture_forges(None)
}
fn fixture_forges(forges: Option<&temper_client_protocol_wire::v1::Forges>) -> Host {
    let mut host = Host::new(11, Time::ZERO, Wall::EPOCH, temper_client_view::sign_in::Ways::NONE);
    host.root = root::unlinked(&host.env.limits, 11);
    host.event(Event::Start { address: Address::Accounts, kept: List::with_capacity(0), with: None });
    host.client(jig_client::Event::StartUnlinked { kept: List::with_capacity(0) });
    host.client(jig_client::Event::SignIn { with: jig_client::With::Transport });
    host.client(jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } });
    let me = host
        .out
        .iter()
        .find_map(|request| match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(c::Subject::Me) },
                    },
            } => Some(*watch),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => None,
        })
        .expect("me watch");
    let link =
        jig_client_protocol_link::Limits::new(host.env.limits.client.domain, limits::EXCHANGES).expect("link bounds");
    let mut frame =
        jig_client_protocol_link::translate::documents::frame(jig_client_test_records::frame_record(), &link)
            .expect("frame");
    frame.providers = List::with_capacity(1);
    frame
        .providers
        .push(c::ProviderOffer {
            provider: 17,
            name: c::DocumentText::complete(
                c::Text::new(Box::from(b"openai".as_slice()), 32).expect("provider name fits"),
            ),
            key_bytes: 4096,
            sign_in: true,
            signed_in_plan: true,
            key: true,
            programs: List::with_capacity(0),
            programs_cut: false,
            none: false,
            manage_usage: None,
        })
        .expect("one offered provider");
    host.client(jig_client::Event::Up {
        up: jig_client::Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(frame))) },
    });
    if let Some(forges) = forges {
        deliver_forges(&mut host, forges);
    }
    host.requests();
    host
}
fn deliver_forges(host: &mut Host, forges: &temper_client_protocol_wire::v1::Forges) {
    let watch = host
        .out
        .iter()
        .find_map(|request| match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Family { .. } },
                    },
            } => Some(*watch),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => None,
        })
        .expect("forges watch");
    host.client(jig_client::Event::Up {
        up: jig_client::Up::FamilyDelivered {
            watch,
            record: jig_client::Family {
                family: temper_client_protocol_wire::FORGE,
                version: temper_client_protocol_wire::CLIENT_VERSION,
                bytes: temper_client_protocol_wire::forges(forges),
            },
        },
    });
}
fn fixture_at(path: &[u8]) -> Host {
    let mut host = fixture();
    host.went(path);
    host.requests();
    host
}
fn build(host: &mut Host) -> Vec<Patch> {
    host.patches()
}
fn node(host: &Host, role: Role, name: &[u8]) -> NodeId {
    host.view
        .tree()
        .nodes()
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .expect("account control")
        .id
}
fn input(host: &mut Host, name: &[u8], value: &[u8]) -> Vec<Patch> {
    let field = node(host, Role::TextField, name);
    host.act(field, Act::Input { value: Box::from(value) });
    build(host)
}
fn send(host: &mut Host, name: &[u8]) -> Token {
    let button = node(host, Role::Button, name);
    host.act(button, Act::Press);
    host.client(jig_client::Event::Stored);
    host.out
        .iter()
        .find_map(|request| match request {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } } } => {
                Some(*attempt)
            }
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => None,
        })
        .expect("account request left the host")
}
#[test]
fn the_key_patch_declares_a_secret_entry() {
    let mut host = fixture();
    host.went(b"/accounts/connect");
    build(&mut host);
    let way = node(&host, Role::Link, b"Use an API key");
    host.act(way, Act::Press);
    host.requests();
    let patches = build(&mut host);
    let key = node(&host, Role::TextField, b"Key (pasted once)");
    let made =
        patches
            .iter()
            .find_map(|patch| {
                if let Patch::Insert { nodes, .. } = patch { nodes.iter().find(|made| made.id == key) } else { None }
            })
            .expect("inserted key field");
    assert_eq!(made.attribute.entry, Some(EntryKind::Secret));
    assert!(made.value.as_ref().expect("empty field").text.is_empty());
    let patches = input(&mut host, b"Key (pasted once)", KEY);
    assert!(!format!("{patches:?} {:?}", host.view).contains("private-host-key"));
}
#[test]
fn sending_a_key_emits_the_empty_value_patch() {
    let mut host = fixture();
    host.went(b"/accounts/connect");
    build(&mut host);
    let way = node(&host, Role::Link, b"Use an API key");
    host.act(way, Act::Press);
    host.requests();
    build(&mut host);
    input(&mut host, b"Key (pasted once)", KEY);
    let key = node(&host, Role::TextField, b"Key (pasted once)");
    send(&mut host, b"Add key");
    let patches = build(&mut host);
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch,Patch::Value {node,value:Some(value)} if *node==key && value.text.is_empty()))
    );
}
#[test]
fn a_key_never_reaches_the_small_store_output() {
    let mut host = fixture();
    host.went(b"/accounts/connect");
    build(&mut host);
    let way = node(&host, Role::Link, b"Use an API key");
    host.act(way, Act::Press);
    host.requests();
    build(&mut host);
    input(&mut host, b"Key (pasted once)", KEY);
    send(&mut host, b"Add key");
    let requests = host.requests();
    assert!(
        !requests.iter().any(|request| matches!(request, Asked::Save { .. })),
        "carried requests are not small-store writes"
    );
}
#[test]
fn the_external_href_is_the_supplied_sign_in_address() {
    let mut host = fixture();
    host.went(b"/accounts/connect");
    build(&mut host);
    let attempt = send(&mut host, b"Continue with ChatGPT");
    host.client(jig_client::Event::Up {
        up: jig_client::Up::Answer {
            attempt,
            envelope: skein_lib::RequestEnvelope::Final,
            answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountSignInStarted { source: 43, address: Box::from(ADDRESS) },
            )))),
            why: None,
        },
    });
    build(&mut host);
    let link = host
        .view
        .tree()
        .nodes()
        .iter()
        .find(|node| node.external.as_deref() == Some(ADDRESS))
        .expect("sign-in address")
        .id;
    assert_eq!(host.href(link).as_deref(), Some(ADDRESS));
}
#[test]
fn the_accounts_href_navigates_inside_the_app() {
    let mut host = fixture();
    build(&mut host);
    let link = node(&host, Role::Link, b"Accounts");
    assert_eq!(host.href(link).as_deref(), Some(b"/accounts".as_slice()));
    host.went(b"/chats");
    build(&mut host);
    let link = node(&host, Role::Link, b"Accounts");
    host.act(link, Act::Press);
    host.requests();
    assert_eq!(host.show, Some((Box::from(b"/accounts".as_slice()), false)));
}

#[test]
fn a_filter_chosen_pushes_the_inbox_at_its_query() {
    let mut host = fixture_at(b"/");
    build(&mut host);
    let question = node(&host, Role::Button, b"Question");
    host.act(question, Act::Press);
    host.requests();
    assert_eq!(host.show.take(), Some((Box::from(b"/?kind=question".as_slice()), false)));
    build(&mut host);
    let all = node(&host, Role::Button, b"All kinds");
    host.act(all, Act::Press);
    host.requests();
    assert_eq!(host.show, Some((Box::from(b"/".as_slice()), false)));
}
#[test]
fn a_load_at_a_filtered_address_opens_the_inbox_filtered() {
    let mut host = fixture_at(b"/?kind=question");
    build(&mut host);
    assert_eq!(
        host.address,
        Address::Inbox { filter: Filter { kind: Some(root::address::Kind::Question), ..Filter::ALL } }
    );
    assert_eq!(host.show, None);
    node(&host, Role::Disclosure, "Kind · Question".as_bytes());
}
#[test]
fn a_stale_filter_link_opens_less_filtered_at_its_printed_address() {
    let host = fixture_at(b"/?kind=reply&project=3");
    assert_eq!(host.address, Address::Inbox { filter: Filter { project: Some(3), ..Filter::ALL } });
    assert_eq!(host.show, Some((Box::from(b"/?project=3".as_slice()), true)));
}

#[path = "../../../testing/memory_figures.rs"]
mod memory_figures;
// Domain state, view tree/bindings/words, HTTP decoding and exchange bodies.

#[test]
#[cfg(target_pointer_width = "64")]
fn the_web_preset_has_a_literal_price() {
    let actual = limits::worst_case(&limits::client()).expect("priced web preset");
    memory_figures::check(
        &skein_scratch::paths::workspace().join("crates/temper/tests/figures/memory/client-web"),
        &limits::memory_terms(&limits::client()).expect("web memory terms"),
        actual,
    );
}

#[test]
fn the_web_preset_sends_a_whole_suggested_request() {
    assert!(limits::client().client.domain.text >= temper_client_protocol_wire::CUE_BYTES);
}

// Wire fixtures keep the regression on Host's linked root and public transport surface.
fn encoded(size: u32, write: impl FnOnce(&mut skein_lib::Writer) -> Result<(), skein_lib::Overflow>) -> Box<[u8]> {
    let mut writer = skein_lib::Writer::new(usize::try_from(size).expect("fixture fits"));
    write(&mut writer).expect("measured fixture");
    writer.finish()
}
fn owned(record: Box<[u8]>) -> jig_client_protocol_wire::v19::Owned {
    use jig_client_protocol_wire::{VERSION, v19 as w};
    w::Owned::new(&w::CEILINGS, w::OwnedParts { owner: w::Owner::Jig, version: VERSION, record })
        .expect("fixture envelope")
}
fn streamed(host: &mut Host, event: &jig_client_protocol_wire::v19::StreamEvent) {
    use skein_lib::base64::{self, Alphabet};
    let bytes = encoded(event.measure(), |writer| event.encode(writer));
    let size = base64::encoded_len(Alphabet::Standard, u32::try_from(bytes.len()).expect("fixture fits"))
        .expect("base64 fits");
    let mut writer = skein_lib::Writer::new(usize::try_from(size).expect("fixture fits"));
    base64::encode(Alphabet::Standard, &bytes, &mut writer).expect("measured base64");
    let mut piece = b"data: ".to_vec();
    piece.extend_from_slice(&writer.finish());
    piece.extend_from_slice(b"\n\n");
    host.heard(Heard::Piece { bytes: piece.into_boxed_slice() });
}
#[test]
fn the_first_task_activity_answer_survives_transport_sign_in() {
    use jig_client_protocol_wire::{VERSION, v19 as w};
    use skein_lib::{Duration, Reader};
    // task_record's chip is task 9; the answer carries one committed run marker.
    const TASK_PATH: &[u8] = b"/tasks/9";
    const RUN: u64 = 44;
    const ONE_LINE: u32 = 1;
    let mut host = Host::new(11, Time::ZERO, Wall::EPOCH, temper_client_view::sign_in::Ways::NONE);
    host.start(TASK_PATH, Vec::new(), None);
    let activity = host
        .requests()
        .into_iter()
        .find_map(|request| match request {
            Asked::Exchange { id, call, .. } if call.path.as_ref() == b"/jig/v1/read" => Some(id),
            Asked::Exchange { .. }
            | Asked::OpenStream { .. }
            | Asked::CloseStream
            | Asked::Save { .. }
            | Asked::Erase { .. } => None,
        })
        .expect("first activity read left the host");
    host.heard(Heard::StreamOpened);
    let mut served = List::with_capacity(1);
    served
        .push(
            w::Served::new(&w::CEILINGS, w::ServedParts { owner: w::Owner::Jig, first: VERSION, last: VERSION })
                .expect("served version"),
        )
        .expect("one served family");
    streamed(
        &mut host,
        &w::StreamEvent::Hello(
            w::Hello::new(&w::CEILINGS, w::HelloParts { stream: [7; 16], served, heartbeat: Duration::from_secs(30) })
                .expect("hello"),
        ),
    );
    let watch = host
        .requests()
        .into_iter()
        .find_map(|request| {
            let Asked::Exchange { call, .. } = request else { return None };
            if call.path.as_ref() != b"/jig/v1/watch" {
                return None;
            }
            let open = w::Open::decode(&w::CEILINGS, &mut Reader::new(&call.body)).expect("watch envelope");
            let subject = w::Subject::decode(&w::CEILINGS, &mut Reader::new(open.subject().record())).expect("subject");
            if matches!(subject, w::Subject::Task(_)) { Some(open.watch()) } else { None }
        })
        .expect("task watch");
    let mut task = jig_client_test_records::task_record().into_parts();
    task.chip = jig_client_test_records::owner_waiting_chip_record();
    let document = w::DeliveryValue::Task(Box::new(w::Task::new(&w::CEILINGS, task).expect("agent task")));
    streamed(
        &mut host,
        &w::StreamEvent::Delivery(
            w::Delivery::new(
                &w::CEILINGS,
                w::DeliveryParts {
                    watch,
                    document: owned(encoded(document.measure(), |writer| document.encode(writer))),
                },
            )
            .expect("task delivery"),
        ),
    );
    let event = w::Activity::RunBegan(
        w::ActivityRunBegan::new(
            &w::CEILINGS,
            w::ActivityRunBeganParts { attempt: RUN, how: w::RunBeginning::Resumed, why: Box::from([]) },
        )
        .expect("run marker"),
    );
    let line =
        w::ActivityLine::new(&w::CEILINGS, w::ActivityLineParts { position: 1, at: 1, event }).expect("activity line");
    let page = jig_client_test_records::activity_rows(&[line]);
    let value = w::ReadValue::Activity(page);
    let answer = w::ReadAnswer::Value(owned(encoded(value.measure(), |writer| value.encode(writer))));
    host.heard(Heard::Answered {
        exchange: activity,
        status: 200,
        body: encoded(answer.measure(), |writer| answer.encode(writer)),
    });
    build(&mut host);
    let shown = host.view.tree().nodes().iter().filter(|node| node.name.as_deref() == Some(b"Run 44 began")).count();
    assert_eq!(shown, usize::try_from(ONE_LINE).expect("one line fits"), "first activity answer must reach the page");
}

#[test]
fn the_deadline_takes_the_earlier_domain_or_view_time_from_the_same_clocks() {
    let now = Time::from_nanos(100);
    let wall = Wall::from_nanos(1000);
    for (domain, next, expected) in [
        (Some(105), Some(1020), Some(105)),
        (Some(130), Some(1020), Some(120)),
        (None, Some(1020), Some(120)),
        (Some(130), None, Some(130)),
        (None, None, None),
        (None, Some(990), Some(100)),
    ] {
        assert_eq!(
            deadline(domain.map(Time::from_nanos), next.map(Wall::from_nanos), now, wall),
            expected.map(Time::from_nanos)
        );
    }
}

#[test]
fn the_patch_queue_is_reserved_once_at_start() {
    let mut host = fixture();
    assert!(!host.patches().is_empty());
    assert!(host.patches.is_empty());
    assert_eq!(host.patches.room(), host.bounds.tree.patches);
    host.went(b"/chats");
    assert!(!host.patches().is_empty());
    assert!(host.patches.is_empty());
    assert_eq!(host.patches.room(), host.bounds.tree.patches);
}

#[test]
fn the_github_rows_external_web_link_has_its_address_and_noopener() {
    use temper_client_protocol_wire::v1 as w;
    for web in [None, Some(b"https://github.test/base/?a=1&b=2".as_slice())] {
        let mut forges = List::with_capacity(1);
        forges
            .push(
                w::ForgeRow::new(
                    &w::CEILINGS,
                    w::ForgeRowParts {
                        web: web.map(Into::into),
                        repositories: List::with_capacity(0),
                        app: Some(w::AppWay::Pasted),
                        number: 2,
                        kind: w::Kind::Github,
                        mode: w::Mode::Connected,
                        connection: w::Connection::Connected(
                            w::Connected::new(
                                &w::CEILINGS,
                                w::ConnectedParts { id: 9, login: b"my-app[bot]".as_slice().into() },
                            )
                            .expect("connected App"),
                        ),
                        offer: w::Offer::NotOffered,
                        first: None,
                        installations: List::with_capacity(0),
                        ended: None,
                        owned: None,
                        admin: false,
                        read_only: false,
                        ended_stop: None,
                    },
                )
                .expect("forge row"),
            )
            .expect("one forge");
        let mut host = fixture_forges(Some(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges }).expect("forges")));
        host.went(b"/forges");
        let mut page = skein_ui_web::page::Page::new(
            skein_ui_web::Look {
                names: &[temper_client_view::look::NAMES],
                pictures: &[],
                components: &[],
                elements: temper_client_view::look::ELEMENTS,
                draw: |_, _, _, _, _| false,
            },
            &host.bounds.tree,
            b"temper",
            skein_ui_web::mapping::Primary::Control,
        );
        let mut ops = Vec::new();
        page.apply(&build(&mut host), &mut ops);
        for link in page.links().to_vec() {
            if let Some(address) = host.href(link) {
                page.set_href(link, &address, &mut ops);
            }
        }
        let html = page.html();
        let label = "Open on GitHub ↗";
        assert_eq!(html.contains(label), web.is_some());
        if web.is_some() {
            let at = html.find(label).expect("GitHub web link");
            let start = html[..at].rfind("<a ").expect("external anchor");
            let end = start + html[start..].find('>').expect("anchor attributes");
            let anchor = &html[start..end];
            assert!(anchor.contains("href=\"https://github.test/base/?a=1&amp;b=2\""), "{anchor}");
            let (_, attributes) = anchor.split_once("rel=\"").expect("external link rel");
            let (rel, _) = attributes.split_once('"').expect("closed rel attribute");
            assert!(rel.split_ascii_whitespace().any(|word| word == "noopener"), "{anchor}");
            assert!(anchor.contains("target=\"_blank\""), "{anchor}");
        }
    }
}
