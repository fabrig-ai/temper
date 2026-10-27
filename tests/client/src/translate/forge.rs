//! The forge application's complete typed request and terminal bridge.
use jig_client_world::{
    harness::ClientId,
    joint::{
        transport::{KeyedOwner, Reply, Transport},
        wire,
    },
};
use skein_lib::{RequestEnvelope, Token};
use temper_client_domain::{self as client, forge};
use temper_engine_domain::{self as engine, Delivery, PartyInput, PartyRequest};
use temper_engine_domain_forge as owner;
#[must_use]
pub fn adoption(value: &forge::Adoption, host: &[u8]) -> owner::Adoption {
    owner::Adoption {
        project: value.project,
        home: value.home,
        forge: value.forge,
        host: host.into(),
        owner: value.owner.clone(),
        name: value.name.clone(),
        prefix: value.prefix.clone(),
        role: match value.role {
            forge::Role::Owned => owner::Role::Owned,
            forge::Role::Adopted => owner::Role::Adopted,
            forge::Role::Fork => owner::Role::Fork,
            forge::Role::Context => owner::Role::Context,
        },
        landing: value.landing.clone(),
        ci: value.ci,
        checks: value.checks.clone(),
    }
}
#[must_use]
pub fn outcome(value: &owner::ApplicationOutcome) -> forge::Answer {
    match value {
        owner::ApplicationOutcome::Adopted(value) => forge::Answer::Adopted(forge::Adopted {
            forge: value.repository.forge,
            repository: value.repository.repository,
            owner: value.owner.clone(),
            name: value.name.clone(),
            role: match value.role {
                owner::Role::Owned => forge::Role::Owned,
                owner::Role::Adopted => forge::Role::Adopted,
                owner::Role::Fork => forge::Role::Fork,
                owner::Role::Context => forge::Role::Context,
            },
            home: value.home,
            kinds: forge::Kinds {
                read: value.kinds.read,
                push: value.kinds.push,
                open: value.kinds.open,
                land: value.kinds.land,
                review: value.kinds.review,
                status: value.kinds.status,
                comment: value.kinds.comment,
                issue: value.kinds.issue,
                branch: value.kinds.branch,
            },
            protection: match &value.protection {
                owner::Protection::Unknown => forge::Protection::Unknown,
                owner::Protection::Absent => forge::Protection::Absent,
                owner::Protection::Rule(value) => forge::Protection::Rule(forge::Rule {
                    branch: value.branch.clone(),
                    contexts: value.contexts.clone(),
                    approvals: value.approvals,
                    dismiss_stale: value.dismiss_stale,
                }),
            },
        }),
        owner::ApplicationOutcome::CoreRefused(why) => {
            forge::Answer::CoreRefused(wire::refusals::refusal(why.clone()).expect("representable application refusal"))
        }
        owner::ApplicationOutcome::ForgeRefused(why) => forge::Answer::ForgeRefused(match why {
            owner::AdoptionRefusal::NoSuchForge => forge::Reason::NoSuchForge,
            owner::AdoptionRefusal::NotFound => forge::Reason::NotFound,
            // temper-07 adds the wire's own not_installed reason.
            owner::AdoptionRefusal::NotInstalled | owner::AdoptionRefusal::NoAccess => forge::Reason::NoAccess,
            owner::AdoptionRefusal::Refused => forge::Reason::Refused,
            owner::AdoptionRefusal::AnotherProject => forge::Reason::AnotherProject,
            owner::AdoptionRefusal::HomeHeld => forge::Reason::HomeHeld,
            owner::AdoptionRefusal::PrefixOccupied => forge::Reason::PrefixOccupied,
            owner::AdoptionRefusal::InFlight => forge::Reason::InFlight,
            owner::AdoptionRefusal::TooLarge => forge::Reason::TooLarge,
            owner::AdoptionRefusal::Unreachable => forge::Reason::Unreachable,
            owner::AdoptionRefusal::InvalidAnswer => forge::Reason::InvalidAnswer,
        }),
    }
}
#[must_use]
pub fn digest(value: &forge::Adoption) -> [u8; 32] {
    let mut bytes = b"temper-adoption/request/1".to_vec();
    bytes.extend_from_slice(&crate::fixtures::FORGE.to_be_bytes());
    bytes.extend_from_slice(&value.project.to_be_bytes());
    bytes.extend_from_slice(&value.forge.to_be_bytes());
    bytes.extend_from_slice(&[
        u8::from(value.home),
        u8::from(value.ci),
        match value.role {
            forge::Role::Owned => 0,
            forge::Role::Adopted => 1,
            forge::Role::Fork => 2,
            forge::Role::Context => 3,
        },
    ]);
    for text in [&value.owner, &value.name, &value.prefix, &value.landing] {
        bytes.extend_from_slice(&u64::try_from(text.len()).expect("bounded field").to_be_bytes());
        bytes.extend_from_slice(text);
    }
    bytes.extend_from_slice(&u64::try_from(value.checks.len()).expect("bounded checks").to_be_bytes());
    for check in &value.checks {
        bytes.extend_from_slice(&check.to_be_bytes());
    }
    let mut writer = skein_lib::Writer::new(32);
    for word in skein_io::digest::digest(&bytes).0 {
        writer.put(&word.to_be_bytes()).expect("32-byte SHA-256");
    }
    writer.finish().as_ref().try_into().expect("32-byte SHA-256")
}
#[must_use]
pub fn request(
    transport: &mut Transport,
    who: ClientId,
    attempt: Token,
    key: [u8; 16],
    scope: jig_client::contract::Scope,
    ask: &jig_client::Family,
    carried: Option<jig_domain_core::Carried>,
) -> engine::Event {
    use temper_client_protocol_wire::{self as w, v1};
    assert_eq!(w::owner(ask.family), Some(w::Owner::Forge));
    assert_eq!(ask.version, w::CLIENT_VERSION);
    let limits = forge::wire_limits(&crate::app::limits().forge).expect("wire bounds");
    let mut reader = skein_lib::Reader::new(&ask.bytes);
    let a = (match v1::Requesting::decode(&limits, &mut reader).expect("whole adoption bytes").into_parts().request {
        v1::Request::Install(install) => {
            assert!(reader.is_empty(), "whole install request");
            let value = install.into_parts();
            let reply_to = transport.reply_to(Reply::Keyed {
                client: who,
                attempt,
                key,
                owner: KeyedOwner::Application { connector: crate::fixtures::FORGE },
            });
            return engine::Event::Party(PartyInput::Application {
                scope: jig_domain_people::ApplicationScope::Deployment,
                reply_to,
                sign_in: transport.sign_in(who).expect("authenticated install"),
                key,
                digest: skein_lib::sha256::digest(&ask.bytes),
                carried: None,
                request: engine::Application::Forge(owner::Ask::Install { forge: value.forge, back: value.back }),
            });
        }
        v1::Request::Accept(_) | v1::Request::Reachable(_) | v1::Request::StartDemo => {
            panic!("client sends no demo start yet")
        }
        v1::Request::Connect(connect) => {
            assert_eq!(scope, jig_client::contract::Scope::Deployment);
            assert!(reader.is_empty(), "whole admin request");
            return admin_request(transport, who, attempt, key, ask, connect, carried);
        }
        v1::Request::Adopt(adopt) => adopt,
        v1::Request::Return(returning) => {
            assert!(reader.is_empty(), "whole return request");
            let value = returning.into_parts();
            let reply_to = transport.reply_to(Reply::Keyed {
                client: who,
                attempt,
                key,
                owner: KeyedOwner::Application { connector: crate::fixtures::FORGE },
            });
            return engine::Event::Party(PartyInput::Application {
                scope: jig_domain_people::ApplicationScope::Own,
                reply_to,
                sign_in: transport.sign_in(who).expect("authenticated return"),
                key,
                digest: skein_lib::sha256::digest(&ask.bytes),
                carried: None,
                request: engine::Application::Return { source: value.source, path: value.path },
            });
        }
        v1::Request::SetUp(set_up) => {
            assert!(reader.is_empty(), "whole setup request");
            return set_up_request(transport, who, attempt, key, scope, ask, set_up.into_parts());
        }
    })
    .into_parts();
    assert!(reader.is_empty());
    let value = forge::Adoption {
        project: a.project,
        home: a.home,
        forge: a.forge,
        owner: a.owner,
        name: a.name,
        prefix: a.prefix,
        landing: a.landing,
        ci: a.ci,
        checks: a.checks.into_boxed(),
        role: match a.role {
            v1::Role::Owned => forge::Role::Owned,
            v1::Role::Adopted => forge::Role::Adopted,
            v1::Role::Fork => forge::Role::Fork,
            v1::Role::Context => forge::Role::Context,
        },
    };
    let sign_in = transport.sign_in(who).expect("authenticated adoption");
    let reply_to = transport.reply_to(Reply::Keyed {
        client: who,
        attempt,
        key,
        owner: KeyedOwner::Application { connector: crate::fixtures::FORGE },
    });
    engine::Event::Party(PartyInput::Application {
        scope: match scope {
            jig_client::contract::Scope::Project(project) => jig_domain_people::ApplicationScope::Project(project),
            jig_client::contract::Scope::Deployment => jig_domain_people::ApplicationScope::Deployment,
            jig_client::contract::Scope::Own => jig_domain_people::ApplicationScope::Own,
        },
        reply_to,
        sign_in,
        key,
        digest: digest(&value),
        carried: None,
        request: temper_engine_domain::Application::Forge(temper_engine_domain_forge::Ask::Adopt(adoption(
            &value,
            crate::fixtures::FORGE_HOST,
        ))),
    })
}
fn admin_request(
    transport: &mut Transport,
    who: ClientId,
    attempt: Token,
    key: [u8; 16],
    ask: &jig_client::Family,
    connect: temper_client_protocol_wire::v1::Connect,
    carried: Option<jig_domain_core::Carried>,
) -> engine::Event {
    use temper_client_protocol_wire::v1;
    let connect = connect.into_parts();
    let v1::How::ForgejoAdmin(admin) = connect.how else { panic!("admin fixture") };
    let first = admin.into_parts().first.map(|first| {
        let first = first.into_parts();
        owner::FirstAsk {
            org: first.org,
            repository: first.repository,
            label: first.label,
            project: match first.project {
                v1::SetUpProject::New(project) => owner::SetUpProject::New(project.into_parts().name),
                v1::SetUpProject::Existing(project) => owner::SetUpProject::Existing(project.project()),
            },
        }
    });
    let reply_to = transport.reply_to(Reply::Keyed {
        client: who,
        attempt,
        key,
        owner: KeyedOwner::Application { connector: crate::fixtures::FORGE },
    });
    engine::Event::Party(PartyInput::Application {
        scope: jig_domain_people::ApplicationScope::Deployment,
        reply_to,
        sign_in: transport.sign_in(who).expect("authenticated admin"),
        key,
        digest: skein_lib::sha256::digest(&ask.bytes),
        carried,
        request: engine::Application::Forge(owner::Ask::Connect {
            forge: connect.forge,
            how: owner::How::ForgejoAdmin { first },
        }),
    })
}
/// Open only the forge's whole, project-scoped change subject (client/forge.md, 9.2).
#[must_use]
pub fn open(
    transport: &mut Transport,
    who: ClientId,
    watch: Token,
    scope: jig_client::contract::Scope,
    window: &jig_client::contract::Window,
    record: &jig_client::Family,
) -> (Vec<engine::Event>, Option<jig_client_world::joint::packets::Incoming>) {
    use jig_client_test_domain::family;
    use jig_client_world::joint::transport::Watched;
    use temper_client_protocol_wire::{self as w, v1};
    if record.family == w::FORGE
        && record.version == w::CLIENT_VERSION
        && let Some(watching) = w::decode_watching(&record.bytes, &v1::CEILINGS)
        && *watching.watch() == v1::Watch::Forges
        && scope == jig_client::contract::Scope::Deployment
    {
        return open_forges(transport, who, watch, window);
    }
    if record.family == w::FORGE
        && record.version == w::CLIENT_VERSION
        && let Some(watching) = w::decode_watching(&record.bytes, &v1::CEILINGS)
        && let v1::Watch::Change(change) = watching.into_parts().watch
        && let jig_client::contract::Scope::Project(project) = scope
        && change.task() > 0
    {
        let scope = jig_domain_people::ApplicationScope::Project(project);
        let watcher = transport.open(
            who,
            watch,
            Watched::Application {
                subject: family::Subject {
                    scope: family::Scope::Project(project),
                    kind: family::Kind::Panel(change.task()),
                },
                scope,
            },
        );
        return (
            vec![engine::Event::Party(PartyInput::Watch {
                watcher,
                sign_in: transport.sign_in(who).expect("authenticated watch"),
                subject: jig_domain_core::documents::Watched::Application {
                    scope,
                    connector: w::FORGE,
                    subject: Token::new(change.task()),
                },
                // The family does not use its window: every panel is whole.
                window: jig_domain_core::documents::Window { rows: window.rows, after: None },
            })],
            None,
        );
    }
    (
        Vec::new(),
        Some(jig_client_world::joint::packets::Incoming::Callback {
            client: who,
            event: jig_client_test_domain::Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Unknown },
                },
            },
        }),
    )
}

fn is_application(transport: &Transport, watcher: Token) -> bool {
    matches!(
        transport.watch(watcher).map(|route| &route.subject),
        Some(jig_client_world::joint::transport::Watched::Application { .. })
    )
}

fn ended(
    transport: &mut Transport,
    watcher: Token,
    end: jig_client::contract::WatchEnd,
) -> Option<(ClientId, client::Event)> {
    let route = transport.end_watch(watcher)?;
    if end == jig_client::contract::WatchEnd::SignedOut {
        transport.signed_out(route.client);
    }
    Some((
        route.client,
        client::Event::Client {
            event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch: route.native, end } },
        },
    ))
}

fn delivered(transport: &mut Transport, watcher: Token, panel: owner::Panel) -> Option<(ClientId, client::Event)> {
    if !is_application(transport, watcher) {
        return None;
    }
    let route = transport.watch(watcher).expect("original application route").clone();
    let bytes = temper_service::family::panel(
        panel,
        &crate::fixtures::forge_settings(),
        &temper_client_protocol_wire::v1::CEILINGS,
    )
    .expect("whole forge panel");
    assert!(transport.delivered(watcher, jig_client_world::joint::transport::Pace::Document));
    Some((
        route.client,
        client::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::FamilyDelivered {
                    watch: route.native,
                    record: jig_client::Family {
                        family: temper_client_protocol_wire::FORGE,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes,
                    },
                },
            },
        },
    ))
}

/// # Errors
/// Returns every request belonging to another owner unchanged.
pub fn incoming(
    transport: &mut Transport,
    request: engine::Request,
    limits: &jig_client::DomainLimits,
) -> Result<Option<(ClientId, client::Event)>, Box<engine::Request>> {
    let delivery = match request {
        engine::Request::Party(PartyRequest::WatchRefused { watcher, why }) if is_application(transport, watcher) => {
            return Ok(ended(transport, watcher, wire::watches::refused(why)));
        }
        engine::Request::Party(PartyRequest::Deliver(delivery)) => delivery,
        request @ (engine::Request::Store(_)
        | engine::Request::Worker(_)
        | engine::Request::Forge { .. }
        | engine::Request::Account(_)
        | engine::Request::SetUpGroup { .. }
        | engine::Request::Connect { .. }
        | engine::Request::Manifest { .. }
        | engine::Request::Reachable { .. }
        | engine::Request::Install { .. }
        | engine::Request::Accept { .. }
        | engine::Request::EraseCarried { .. }
        | engine::Request::StopGroup { .. }
        | engine::Request::SetUp { .. }
        | engine::Request::Bound { .. }
        | engine::Request::Return { .. }
        | engine::Request::Setup { .. }
        | engine::Request::ToChild(_)
        | engine::Request::Busy(_)
        | engine::Request::Unsettled(_)
        | engine::Request::Drained
        | engine::Request::Stop
        | engine::Request::Party(
            PartyRequest::SignInRefused { .. } | PartyRequest::WatchRefused { .. } | PartyRequest::RunText { .. },
        )) => return Err(Box::new(request)),
    };
    let (to, body) = match delivery {
        Delivery::Panel { watcher, panel } => return Ok(delivered(transport, watcher, *panel)),
        Delivery::Forges { watcher, forges } => return Ok(delivered_forges(transport, watcher, forges)),
        Delivery::WatchEnded { watcher, why } if is_application(transport, watcher) => {
            return Ok(ended(transport, watcher, wire::watches::ended(why)));
        }
        Delivery::Application { to, outcome: value } => (to, value),
        Delivery::WebReply { to, reply } => return admission_reply(transport, to, reply, limits),
        delivery @ (Delivery::DemoActed { .. }
        | Delivery::DemoNote { .. }
        | Delivery::SourceSignedOut { .. }
        | Delivery::Result { .. }
        | Delivery::Host(_)
        | Delivery::Link { .. }
        | Delivery::ProjectMade { .. }
        | Delivery::ForgeCommitted { .. }
        | Delivery::ForgeCall { .. }
        | Delivery::Bound { .. }
        | Delivery::Procedure { .. }
        | Delivery::Relay { .. }
        | Delivery::ReadEscalationDecision { .. }
        | Delivery::Fleet(_)
        | Delivery::Assigned { .. }
        | Delivery::ReadLoad { .. }
        | Delivery::Drained
        | Delivery::Restart(_)
        | Delivery::Refuse { .. }
        | Delivery::TurnBusy { .. }
        | Delivery::NotesLoad { .. }
        | Delivery::Load { .. }
        | Delivery::Read { .. }
        | Delivery::Document { .. }
        | Delivery::WatchEnded { .. }
        | Delivery::CarriedUnused { .. }) => {
            return Err(Box::new(engine::Request::Party(PartyRequest::Deliver(delivery))));
        }
    };
    let Some(route) = transport.take_reply(to.into_token()) else { return Ok(None) };
    Ok(Some(answered(&route, body)))
}
fn admission_reply(
    transport: &mut Transport,
    to: skein_lib::ReplyTo,
    reply: jig_domain_people::Reply,
    limits: &jig_client::DomainLimits,
) -> Result<Option<(ClientId, client::Event)>, Box<engine::Request>> {
    // Preserve the affine answer right until its route is known.
    let matching = transport.reply_tokens().into_iter().find(|token| to == skein_lib::ReplyTo::new(*token));
    let Some(token) = matching else {
        return Err(Box::new(engine::Request::Party(PartyRequest::Deliver(Delivery::WebReply { to, reply }))));
    };
    if !matches!(transport.reply(token), Some(Reply::Keyed { owner: KeyedOwner::Application { .. }, .. })) {
        return Err(Box::new(engine::Request::Party(PartyRequest::Deliver(Delivery::WebReply { to, reply }))));
    }
    let (envelope, answer) = wire::answers::keyed(
        wire::answers::reply(reply, &transport.models, limits).expect("bounded application admission reply"),
    )
    .expect("application admission disposition");
    let Reply::Keyed { client, attempt, .. } = transport.take_reply(to.into_token()).expect("original route") else {
        panic!("keyed route")
    };
    if envelope == RequestEnvelope::SignedOut {
        transport.signed_out(client);
    }
    Ok(Some((
        client,
        client::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope,
                    answer: answer.map(jig_client::Answered::Jig),
                    why: None,
                },
            },
        },
    )))
}
/// The application's outcome, answered on its keyed route as the forge family's bytes.
fn answered(route: &Reply, body: Option<temper_engine_domain::ApplicationAnswer>) -> (ClientId, client::Event) {
    let Reply::Keyed { client, attempt, owner: KeyedOwner::Application { connector }, .. } = *route else {
        panic!("application reply route")
    };
    assert_eq!(connector, crate::fixtures::FORGE);
    (
        client,
        client::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: RequestEnvelope::Final,
                    why: None,
                    answer: Some(jig_client::Answered::Family(jig_client::Family {
                        family: connector,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes: temper_service::family::answer(
                            body,
                            &forge::wire_limits(&crate::app::limits().forge).expect("wire bounds"),
                            &jig_client_protocol_link::Limits::new(
                                crate::app::limits().client.domain,
                                crate::app::limits().client.exchanges,
                            )
                            .expect("link bounds")
                            .wire,
                        )
                        .expect("whole forge answer"),
                    })),
                },
            },
        },
    )
}

fn set_up_request(
    transport: &mut Transport,
    who: ClientId,
    attempt: Token,
    key: [u8; 16],
    scope: jig_client::contract::Scope,
    record: &jig_client::Family,
    setup: temper_client_protocol_wire::v1::SetUpParts,
) -> engine::Event {
    let reply_to = transport.reply_to(Reply::Keyed {
        client: who,
        attempt,
        key,
        owner: KeyedOwner::Application { connector: crate::fixtures::FORGE },
    });
    engine::Event::Party(PartyInput::Application {
        scope: match scope {
            jig_client::contract::Scope::Deployment => jig_domain_people::ApplicationScope::Deployment,
            jig_client::contract::Scope::Project(project) => jig_domain_people::ApplicationScope::Project(project),
            jig_client::contract::Scope::Own => jig_domain_people::ApplicationScope::Own,
        },
        reply_to,
        sign_in: transport.sign_in(who).expect("authenticated setup"),
        key,
        digest: skein_lib::sha256::digest(&record.bytes),
        carried: None,
        request: temper_engine_domain::Application::Forge(owner::Ask::SetUp {
            playground: setup.playground,
            forge: setup.forge,
            org: setup.org,
            repository: setup.repository,
            project: match setup.project {
                temper_client_protocol_wire::v1::SetUpProject::Existing(value) => {
                    owner::SetUpProject::Existing(value.project())
                }
                temper_client_protocol_wire::v1::SetUpProject::New(value) => {
                    owner::SetUpProject::New(value.into_parts().name)
                }
            },
        }),
    })
}
fn open_forges(
    transport: &mut Transport,
    who: ClientId,
    watch: Token,
    window: &jig_client::contract::Window,
) -> (Vec<engine::Event>, Option<jig_client_world::joint::packets::Incoming>) {
    use jig_client_test_domain::family;
    let scope = jig_domain_people::ApplicationScope::Deployment;
    let watcher = transport.open(
        who,
        watch,
        jig_client_world::joint::transport::Watched::Application {
            subject: family::Subject { scope: family::Scope::Deployment, kind: family::Kind::Deployment },
            scope,
        },
    );
    (
        vec![engine::Event::Party(PartyInput::Watch {
            watcher,
            sign_in: transport.sign_in(who).expect("authenticated forges watch"),
            subject: jig_domain_core::documents::Watched::Application {
                scope,
                connector: temper_client_protocol_wire::FORGE,
                subject: Token::new(0),
            },
            window: jig_domain_core::documents::Window { rows: window.rows, after: None },
        })],
        None,
    )
}
fn delivered_forges(
    transport: &mut Transport,
    watcher: Token,
    forges: owner::Forges,
) -> Option<(ClientId, client::Event)> {
    if !is_application(transport, watcher) {
        return None;
    }
    let route = transport.watch(watcher).expect("original forges route").clone();
    let bytes = temper_service::family::forges(forges, &temper_client_protocol_wire::v1::CEILINGS)
        .expect("whole forges document");
    assert!(transport.delivered(watcher, jig_client_world::joint::transport::Pace::Document));
    Some((
        route.client,
        client::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::FamilyDelivered {
                    watch: route.native,
                    record: jig_client::Family {
                        family: temper_client_protocol_wire::FORGE,
                        version: temper_client_protocol_wire::CLIENT_VERSION,
                        bytes,
                    },
                },
            },
        },
    ))
}

#[cfg(test)]
mod watch_tests {
    use super::*;
    use jig_client::{Down, Watchable, contract as c};
    use jig_client_world::joint::transport::Transport;
    use skein_lib::Token;
    use temper_client_protocol_wire::{self as w, v1};

    const CLIENT: ClientId = ClientId(0);
    const NATIVE: Token = Token::new(91);
    const TASK: u64 = 7;
    const SIGN_IN: u64 = 23;

    fn transport() -> Transport {
        let mut transport = Transport::new(c::VersionRange {
            first: jig_client_protocol_wire::VERSION,
            last: jig_client_protocol_wire::VERSION,
        });
        transport.bind(
            CLIENT,
            crate::fixtures::PARTY,
            jig_domain_people::Identity {
                key: jig_domain_people::IdentityKey { provider: 0, subject: 1_u64.to_be_bytes().into() },
                login: b"person".as_slice().into(),
                name: b"Person".as_slice().into(),
            },
        );
        transport.signed_in(CLIENT, crate::fixtures::PARTY, SIGN_IN);
        transport
    }

    fn record(task: u64) -> jig_client::Family {
        let subject = v1::ChangeSubject::new(&v1::CEILINGS, v1::ChangeSubjectParts { task }).expect("subject");
        let watching =
            v1::Watching::new(&v1::CEILINGS, v1::WatchingParts { watch: v1::Watch::Change(subject) }).expect("watch");
        jig_client::Family { family: w::FORGE, version: w::CLIENT_VERSION, bytes: w::watching(&watching) }
    }

    fn request(
        transport: &mut Transport,
        record: jig_client::Family,
        scope: c::Scope,
    ) -> (Vec<engine::Event>, Option<jig_client_world::joint::packets::Incoming>) {
        let mut request = client::Request::Client {
            request: jig_client::Request::Down {
                down: Down::Open {
                    watch: NATIVE,
                    subject: Watchable::Family { scope, window: c::Window { rows: 0, after: None }, record },
                },
            },
        };
        crate::translate::request(transport, CLIENT, &mut request, &crate::fixtures::domain_limits().client.domain)
    }

    fn open_watch(transport: &mut Transport) -> Token {
        let (events, packet) = request(transport, record(TASK), c::Scope::Project(1));
        assert!(packet.is_none());
        let [engine::Event::Party(PartyInput::Watch { watcher, sign_in, subject, window })] = events.as_slice() else {
            panic!("one root watch: {events:?}")
        };
        assert_eq!(*sign_in, SIGN_IN);
        assert_eq!(
            *subject,
            jig_domain_core::documents::Watched::Application {
                scope: jig_domain_people::ApplicationScope::Project(1),
                connector: w::FORGE,
                subject: Token::new(TASK),
            }
        );
        assert_eq!(window.rows, 0);
        assert!(window.after.is_none());
        assert_eq!(transport.application_watch_token(CLIENT, NATIVE), Some(*watcher));
        let route = transport.watch(*watcher).expect("application route");
        assert_eq!(route.client, CLIENT);
        assert_eq!(route.native, NATIVE);
        assert!(matches!(
            route.subject,
            jig_client_world::joint::transport::Watched::Application {
                subject: jig_client_test_domain::family::Subject {
                    scope: jig_client_test_domain::family::Scope::Project(1),
                    kind: jig_client_test_domain::family::Kind::Panel(TASK),
                },
                scope: jig_domain_people::ApplicationScope::Project(1),
            }
        ));
        *watcher
    }

    #[test]
    fn an_adoption_request_envelope_decodes_to_the_root_request() {
        let adoption = forge::Adoption {
            project: 7,
            home: true,
            forge: w::FORGE,
            owner: b"acme".as_slice().into(),
            name: b"shop".as_slice().into(),
            prefix: b"work/".as_slice().into(),
            landing: b"main".as_slice().into(),
            role: forge::Role::Owned,
            ci: true,
            checks: Box::new([2, 3]),
        };
        let limits = forge::wire_limits(&crate::app::limits().forge).expect("wire bounds");
        let record = forge::family::request(&adoption, &limits).expect("request envelope");
        let mut transport = transport();
        let event = super::request(&mut transport, CLIENT, NATIVE, [5; 16], c::Scope::Project(7), &record, None);
        let engine::Event::Party(PartyInput::Application { request, .. }) = event else { panic!("root adoption") };
        assert_eq!(
            request,
            engine::Application::Forge(owner::Ask::Adopt(super::adoption(&adoption, crate::fixtures::FORGE_HOST)))
        );
    }

    #[test]
    fn a_change_watch_opens_the_roots_project_scoped_application_watch() {
        open_watch(&mut transport());
    }

    #[test]
    fn invalid_change_watches_are_unknown_without_entering_the_root() {
        let mut version = record(TASK);
        version.version += 1;
        let mut undecodable = record(TASK);
        undecodable.bytes = Box::new([]);
        let mut trailing = record(TASK);
        trailing.bytes = trailing.bytes.iter().copied().chain([0]).collect();
        let mut family = record(TASK);
        family.family += 1;
        for (record, scope) in [
            (version, c::Scope::Project(1)),
            (undecodable, c::Scope::Project(1)),
            (trailing, c::Scope::Project(1)),
            (record(0), c::Scope::Project(1)),
            (family, c::Scope::Project(1)),
            (record(TASK), c::Scope::Deployment),
            (record(TASK), c::Scope::Own),
        ] {
            let mut transport = transport();
            let (events, packet) = request(&mut transport, record, scope);
            assert!(events.is_empty());
            assert!(transport.watches().is_empty());
            assert!(matches!(
                packet,
                Some(jig_client_world::joint::packets::Incoming::Callback {
                    client: CLIENT,
                    event: jig_client_test_domain::Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::WatchEnded { watch: NATIVE, end: c::WatchEnd::Unknown },
                        }
                    },
                })
            ));
        }
    }

    #[test]
    fn a_whole_panel_reaches_its_native_client_route_and_is_acknowledged_once() {
        let mut front =
            crate::front::Joint::with_change_landing(crate::fixtures::SEED, &crate::fixtures::domain_limits());
        crate::fixtures::adopt_change_repository(&mut front);
        front.engine.forge.as_mut().expect("stand").push_branch(7, b"temper/panel");
        let task = crate::fixtures::goal_with_pushed_change(&mut front, b"temper/panel");
        let mut transport = transport();
        transport.signed_in(CLIENT, crate::fixtures::PARTY, front.engine.session());
        let (events, packet) = request(&mut transport, record(task), c::Scope::Project(1));
        assert!(packet.is_none());
        let watcher = transport.application_watch_token(CLIENT, NATIVE).expect("route");
        for event in events {
            front.engine.send(event);
        }
        front.engine.settle();
        let panel = front
            .engine
            .delivered
            .iter()
            .find_map(|delivery| {
                if let Delivery::Panel { watcher: original, panel } = delivery
                    && *original == watcher
                {
                    Some(panel.clone())
                } else {
                    None
                }
            })
            .expect("root's whole panel");
        let expected = temper_service::family::panel(*panel.clone(), &crate::fixtures::forge_settings(), &v1::CEILINGS)
            .expect("service encoding");
        let delivered = incoming(
            &mut transport,
            engine::Request::Party(PartyRequest::Deliver(Delivery::Panel { watcher, panel })),
            &crate::fixtures::domain_limits().client.domain,
        )
        .expect("forge route")
        .expect("delivery");
        let (
            who,
            client::Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::FamilyDelivered { watch, record } },
            },
        ) = delivered
        else {
            panic!("forge's family delivery")
        };
        assert_eq!(who, CLIENT);
        assert_eq!(watch, NATIVE);
        assert_eq!(record.family, w::FORGE);
        assert_eq!(record.version, w::CLIENT_VERSION);
        assert_eq!(record.bytes, expected);
        let panel = w::decode_change_panel(&record.bytes, &v1::CEILINGS).expect("whole panel bytes");
        assert_eq!(panel.task(), task);
        assert_eq!(panel.owner(), b"acme");
        assert_eq!(panel.name(), b"shop");
        assert!(
            matches!(transport.application_taken(CLIENT, NATIVE), Some(jig_domain_core::Event::Delivered { watcher: original }) if original == watcher)
        );
        assert!(transport.application_taken(CLIENT, NATIVE).is_none());
    }

    #[test]
    fn closing_a_change_watch_unwatches_and_keeps_its_route_until_the_terminal() {
        let mut transport = transport();
        let watcher = open_watch(&mut transport);
        let mut request =
            client::Request::Client { request: jig_client::Request::Down { down: Down::Close { watch: NATIVE } } };
        let (events, packet) = crate::translate::request(
            &mut transport,
            CLIENT,
            &mut request,
            &crate::fixtures::domain_limits().client.domain,
        );
        assert!(packet.is_none());
        assert!(
            matches!(events.as_slice(), [engine::Event::Party(PartyInput::Unwatch { watcher: original })] if *original == watcher)
        );
        assert!(transport.application_watch_token(CLIENT, NATIVE).is_none());
        assert!(transport.watch(watcher).is_some());
        let terminal = incoming(
            &mut transport,
            engine::Request::Party(PartyRequest::Deliver(Delivery::WatchEnded {
                watcher,
                why: jig_domain_views::End::Unwatched,
            })),
            &crate::fixtures::domain_limits().client.domain,
        )
        .expect("forge terminal")
        .expect("callback");
        assert!(matches!(
            terminal,
            (
                CLIENT,
                client::Event::Client {
                    event: jig_client::Event::Up {
                        up: jig_client::Up::WatchEnded { watch: NATIVE, end: c::WatchEnd::Closed }
                    }
                }
            )
        ));
        assert!(transport.watch(watcher).is_none());
    }

    #[test]
    fn application_refusals_and_ends_use_temper_callbacks_and_retire_the_route() {
        for why in [
            jig_domain_core::WatchRefusal::Unknown,
            jig_domain_core::WatchRefusal::Busy,
            jig_domain_core::WatchRefusal::NotAllowed,
            jig_domain_core::WatchRefusal::SignedOut,
        ] {
            let mut transport = transport();
            let watcher = open_watch(&mut transport);
            let expected = wire::watches::refused(why);
            let terminal = incoming(
                &mut transport,
                engine::Request::Party(PartyRequest::WatchRefused { watcher, why }),
                &crate::fixtures::domain_limits().client.domain,
            )
            .expect("forge refusal")
            .expect("terminal");
            assert!(
                matches!(terminal, (CLIENT, client::Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch: NATIVE, end } } }) if end == expected)
            );
            assert!(transport.watch(watcher).is_none());
            assert_eq!(transport.sign_in(CLIENT).is_none(), expected == c::WatchEnd::SignedOut);
        }
        for why in [
            jig_domain_views::End::Gone,
            jig_domain_views::End::Unwatched,
            jig_domain_views::End::NotAllowed,
            jig_domain_views::End::SignedOut,
            jig_domain_views::End::Stopping,
        ] {
            let mut transport = transport();
            let watcher = open_watch(&mut transport);
            let expected = wire::watches::ended(why);
            let terminal = incoming(
                &mut transport,
                engine::Request::Party(PartyRequest::Deliver(Delivery::WatchEnded { watcher, why })),
                &crate::fixtures::domain_limits().client.domain,
            )
            .expect("forge end")
            .expect("terminal");
            assert!(
                matches!(terminal, (CLIENT, client::Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch: NATIVE, end } } }) if end == expected)
            );
            assert!(transport.watch(watcher).is_none());
            assert_eq!(transport.sign_in(CLIENT).is_none(), expected == c::WatchEnd::SignedOut);
        }
    }
}
