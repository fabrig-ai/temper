#[expect(dead_code, reason = "the shared transcript also names the dark-theme request and report for the view tier")]
#[path = "../../../../../testing/first_run.rs"]
mod first_run;
use super::*;
use skein_ui::{Role, Run};
fn text(b: &mut skein_ui::Builder, role: Role, words: &[u8]) {
    b.open(role);
    b.text(Run::Plain, words);
    b.close();
}
pub fn page() -> skein_ui::Tree {
    let mut b = skein_ui::Builder::new(&tree_limits());
    synthetic_frame(&mut b, false);
    b.open(Role::Main);
    text(&mut b, Role::Heading { level: 1 }, "T9 · Hello".as_bytes());
    text(&mut b, Role::Status, b"Writing");
    text(&mut b, Role::Paragraph, &spend(7, 100, 2));
    b.open(Role::Log);
    b.name(b"Conversation");
    text(&mut b, Role::Paragraph, b"You");
    text(&mut b, Role::Paragraph, b"Hello");
    text(&mut b, Role::Paragraph, b"Temper");
    golden_cards::report(&mut b, b"I can help.");
    text(&mut b, Role::Status, b"Writing...");
    text(&mut b, Role::Paragraph, b"Writing an answer");
    b.close();
    for (role, name) in [
        (Role::TextArea, b"Message".as_slice()),
        (Role::Button, b"Send"),
        (Role::Button, b"Stop"),
        (Role::Button, b"Close"),
        (Role::Button, b"Older"),
    ] {
        b.open(role);
        b.name(name);
        b.bind(binding());
        b.close();
    }
    b.close();
    b.close();
    b.finish()
}

/// The first run, through the production chat view and delegation card.
pub fn delegation_page(landed: bool) -> skein_ui::Tree {
    first_run_page(client::Address::Chat { task: 1 }, landed)
}
pub fn first_run() -> skein_ui::Tree {
    first_run_page(client::Address::Chats { filter: client::ChatsFilter::All, start: false }, true)
}
#[expect(clippy::too_many_lines, reason = "one fixture answers the chat's documents and the session's demo and forges")]
fn first_run_page(address: client::Address, landed: bool) -> skein_ui::Tree {
    use jig_client::contract as c;
    use skein_lib::{Env, List, Queue, Time, Wall};
    use temper_client_domain::{Event, Request};
    let limits = crate::limits::native().client;
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut root = client::unlinked(&limits, 16);
    let mut out = Queue::with_capacity(client::max_out(&limits));
    client::step(&mut root, &env, Event::Start { address, kept: List::with_capacity(0), with: None }, &mut out);
    for event in [
        jig_client::Event::StartUnlinked { kept: List::with_capacity(0) },
        jig_client::Event::SignIn { with: jig_client::With::Transport },
        jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } },
    ] {
        client::step(&mut root, &env, Event::Client { event }, &mut out);
    }
    for _ in 0..128 {
        let Some(request) = out.pop() else { break };
        let up = match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Family { record, .. } },
                    },
            } if temper_client_protocol_wire::decode_watching(&record.bytes, &wire_limits())
                .expect("forge watch")
                .watch()
                == &temper_client_protocol_wire::v1::Watch::Forges =>
            {
                Some(jig_client::Up::FamilyDelivered {
                    watch,
                    record: jig_client::Family {
                        family: temper_client_protocol_wire::FORGE,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes: demo_forges(),
                    },
                })
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(subject) },
                    },
            } => {
                let delivery = match subject {
                    c::Subject::Settings => unreachable!("temper does not watch settings"),
                    c::Subject::Me => Some(c::Delivery::Me(Box::new(demo_me()))),
                    c::Subject::Task { task } => Some(c::Delivery::Task(Box::new(delegation_task(task, landed)))),
                    c::Subject::Accounts if landed => Some(c::Delivery::OwnerAccounts(Box::new(demo_accounts()))),
                    c::Subject::Deployment
                    | c::Subject::Tree { .. }
                    | c::Subject::Chips { .. }
                    | c::Subject::Inbox { .. }
                    | c::Subject::Tasks { .. }
                    | c::Subject::Run { .. }
                    | c::Subject::Accounts => None,
                };
                let Some(delivery) = delivery else { continue };
                Some(jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(delivery) })
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Read {
                                read,
                                what: jig_client::Readable::Jig(c::Read::Activity { before, .. }),
                            },
                    },
            } => Some(jig_client::Up::ReadDone {
                read,
                result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(Box::new(
                    first_run::page(&delegation_activity(landed), before.as_ref(), limits.client.domain.rows),
                )))),
            }),
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Family { record, .. } },
                    },
            } if landed
                && temper_client_protocol_wire::decode_reading(&record.bytes, &wire_limits())
                    .expect("forge reading")
                    .read()
                    == &temper_client_protocol_wire::v1::Read::Demo =>
            {
                Some(jig_client::Up::ReadDone {
                    read,
                    result: jig_client::Readout::Family {
                        family: temper_client_protocol_wire::FORGE,
                        record: Some(demo_answer()),
                    },
                })
            }
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => None,
        };
        if let Some(up) = up {
            client::step(&mut root, &env, Event::Client { event: jig_client::Event::Up { up } }, &mut out);
        }
    }
    assert!(out.is_empty(), "first run fixture settles");
    let mut view =
        temper_client_view::View::new(&temper_client_view::Limits { tree: tree_limits() }, &limits, page_words(), 1);
    temper_client_view::build(&mut view, &root, super::NOW, &mut Queue::with_capacity(tree_limits().patches));
    copy_tree(view.tree())
}
fn demo_me() -> contract::Me {
    use jig_client::contract as c;
    use skein_lib::List;
    let mut me = jig_client_world::stories::frame(1);
    me.deployment_role = Some(c::HeldRole::Owner);
    let mut models = List::with_capacity(1);
    models
        .push(c::MeModel {
            name: delegation_text(b"demo"),
            provider: Some(3),
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            window: 1024,
            output: 128,
            efforts: List::with_capacity(0),
        })
        .expect("demo model");
    me.projects.get_mut(0).expect("demo project").models = models;
    me.projects.get_mut(0).expect("demo project").name = c::DocumentText::complete(delegation_text(b"demo"));
    me
}
fn delegation_task(number: u64, landed: bool) -> contract::Task {
    use jig_client::contract as c;
    let mut task = jig_client_world::stories::conversations::chat(number, b"First run", skein_lib::Wall::EPOCH);
    task.chip.task = number;
    task.chip.title = c::DocumentText::complete(delegation_text(if number == 1 {
        b"Add CSV export to widget"
    } else {
        b"Export widget's table as CSV"
    }));
    task.hold = None;
    task.kept_account = Some(1);
    task.budget.funder = c::Funder::Pool { project: 1, person: 1, period: 1 };
    task.result = None;
    task.chip.phase = c::Phase::Running;
    task.proposals.clear();
    task.activity = if number == 1 { if landed { 9 } else { 5 } } else { 0 };
    task.spec.specification.words = c::DocumentText::complete(delegation_text(first_run::CSV));
    if number == 1 {
        task.requester = c::OwnerRequester::Person(1);
        task.wake.words = c::WakeRule::Immediate;
        task.waits_on = if landed { c::WaitsOn::News } else { c::WaitsOn::Delegates };
        let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("chat owner") };
        // The first-run fixture has one change, still live until its result arrives.
        details.live_delegates = u32::from(!landed);
        details.wake = task.wake;
        if !landed {
            task.delegates = skein_lib::List::with_capacity(1);
            task.delegates
                .push(c::LiveDelegate { chip: delegation_task(2, false).chip, takeover_budget: 0 })
                .expect("live change");
        }
    }
    if number == 2 {
        task.requester = c::OwnerRequester::Task(1);
        task.result_contract = c::TaskContractDocument::Change { connector: 1, kind: 1, words: 256 };
        if landed {
            task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Change {
                kind: 1,
                part: c::OwnerPart { connector: 1, kind: c::OwnerPartKind::Result, key: c::OwnerPartKey::Number(2) },
                words: c::DocumentText::complete(delegation_text(b"1a2b3c4abcdef0123456789abcdef0123456789abcd")),
            }))));
        }
    }
    task
}
fn delegation_activity(landed: bool) -> contract::ActivityPage {
    first_run::activity(false, landed, crate::limits::native().client.client.domain.turn_text)
}

fn delegation_text(bytes: &[u8]) -> contract::Text {
    contract::Text::new(Box::from(bytes), 256).expect("first run text fits")
}

fn demo_accounts() -> contract::OwnerAccounts {
    use jig_client::contract as c;
    use skein_lib::List;
    let mut readiness = List::with_capacity(1);
    readiness
        .push(c::PayerReadiness {
            provider: 3,
            payer: c::OwnerAccountUse::Party(1),
            readiness: c::Readiness::Usable,
            settled: Some(true),
        })
        .expect("demo readiness");
    let mut sources = List::with_capacity(1);
    sources
        .push(c::OwnerAccountSource {
            name: None,
            place: 1,
            number: 1,
            kind: c::OwnerAccountKind::None,
            state: c::OwnerAccountState::Granted,
            usable: true,
            expiring: false,
            attention: false,
            spent_until: None,
        })
        .expect("demo source");
    let mut accounts = List::with_capacity(1);
    accounts
        .push(c::OwnerAccountLine {
            number: 1,
            place: 1,
            uses: None,
            plan: false,
            provider: Some(3),
            provider_name: None,
            manage_usage: None,
            display: c::DocumentText::complete(delegation_text(b"Demo")),
            holder: c::OwnerAccountHolder::Deployment,
            sources,
            cut: false,
        })
        .expect("demo account");
    contract::OwnerAccounts {
        program_hosts: skein_lib::List::with_capacity(0),
        readiness,
        readiness_cut: false,
        accounts,
        signing: skein_lib::List::with_capacity(0),
        last_end: None,
        usable: true,
        settled: true,
        cut: false,
    }
}
fn demo_answer() -> Box<[u8]> {
    use temper_client_protocol_wire::{self as wire, v1 as w};
    let mut cues = skein_lib::List::with_capacity(3);
    for cue in [
        b"Add CSV export to widget".as_slice(),
        b"Add a dark theme to widget",
        "Fix widget’s empty search state".as_bytes(),
    ] {
        cues.push(cue.into()).expect("three demo cues");
    }
    wire::demo_answer(
        &w::DemoAnswer::new(
            &wire_limits(),
            w::DemoAnswerParts {
                state: w::DemoState::Running,
                project: 1,
                forge: 1,
                provider: 3,
                owner: b"playground".as_slice().into(),
                name: b"widget".as_slice().into(),
                cues,
            },
        )
        .expect("demo answer fits"),
    )
}

fn demo_forges() -> Box<[u8]> {
    use temper_client_protocol_wire::{self as wire, v1 as w};
    let limits = wire_limits();
    let mut forges = skein_lib::List::with_capacity(1);
    forges
        .push(
            w::ForgeRow::new(
                &limits,
                w::ForgeRowParts {
                    number: 1,
                    kind: w::Kind::Forgejo,
                    mode: w::Mode::Fake,
                    connection: w::Connection::Connected(
                        w::Connected::new(&limits, w::ConnectedParts { id: 1, login: b"temper".as_slice().into() })
                            .expect("demo account"),
                    ),
                    offer: w::Offer::NotOffered,
                    first: None,
                    installations: skein_lib::List::with_capacity(0),
                    ended: None,
                    app: None,
                    web: None,
                    repositories: skein_lib::List::with_capacity(0),
                    owned: None,
                    admin: false,
                    read_only: false,
                    ended_stop: None,
                },
            )
            .expect("demo forge"),
        )
        .expect("one demo forge");
    wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).expect("demo forges"))
}
