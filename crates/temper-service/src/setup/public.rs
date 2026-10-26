//! Public Paste and Install; carried bytes stay beside the host (protocol/forge.md, 2.6).
use crate::engine::Engine;
use alloc::boxed::Box;
use core::num::NonZeroU64;
use jig_domain_core::Carried;
use jig_protocol_store::{CarriedKind, Event, SecretOwner, SecretValue, Secrets};
use skein_forge_setup::host;
use skein_lib::{Secret, Time, Token};
use temper_engine_domain as root;
use temper_engine_domain_forge as domain;

#[derive(Debug)]
pub(super) enum Caller {
    Manifest { forge: u16, request: Token, back: Option<Box<[u8]>> },
    Redirect { forge: u16, call: Token, back: Option<Box<[u8]>> },
    // Admin runs start with Other for local failures; skein failures replace it.
    Connect { forge: u16, request: Token, number: u64, pasted: Option<Pasted>, stop: Option<domain::AdminStop> },
    Install { forge: u16, request: Token, back: Option<Box<[u8]>> },
    AcceptAsked { forge: u16, request: Token, installation: u64, stop: domain::AdoptionRefusal },
    Accept { forge: u16, call: Token, back: Option<Box<[u8]>> },
    Reachable { forge: u16, request: Token },
}
#[derive(Debug)]
pub(super) enum Pasted {
    App(Secret),
    Admin(Secret),
}
pub(super) struct ReturnTo {
    pub(super) forge: u16,
    pub(super) state: Box<[u8]>,
    pub(super) back: Option<Box<[u8]>>,
    pub(super) until: Time,
}
fn event(engine: &mut Engine, event: domain::Event) {
    engine.events.push(root::Event::Forge(Box::new(event)));
}
fn full(engine: &Engine, forge: u16) -> bool {
    super::lifecycle::busy(engine, forge)
        || engine.setup.runs.len() == engine.setup.env.limits.runs
        || engine.setup.public.len() == engine.setup.public.capacity()
}
fn stage(engine: &mut Engine, run: Token, plan: host::Plan) {
    engine.setup.staged.insert(run, plan).expect("public run room checked");
    super::request(engine, run);
}
pub(crate) fn admin_connect(
    engine: &mut Engine,
    forge: u16,
    request: Token,
    first: Option<domain::FirstAsk>,
    carried: Carried,
) {
    if full(engine, forge) {
        erase(engine, carried.number);
        event(
            engine,
            domain::Event::ConnectAnswered {
                forge,
                request,
                answer: domain::ConnectAnswer::Refused(domain::ForgeRefusal::Busy),
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    event(engine, domain::Event::ConnectAnswered { forge, request, answer: domain::ConnectAnswer::Connecting });
    let value = engine.store.secrets(run, Secrets::Get { owner: SecretOwner::Application, number: carried.number });
    let parsed = match value {
        Some(Event::Secret { value: Some(SecretValue::Carried { kind: CarriedKind::Value, value }), .. })
            if skein_lib::sha256::digest(value.expose()) == carried.digest =>
        {
            pasted(value.expose(), 1, b"the pasted value is not a token")
        }
        Some(_) | None => Err(b"the pasted value changed".as_slice()),
    };
    let (pasted, failure) = match parsed {
        Ok(value) => (Some(Pasted::Admin(value)), None),
        Err(reason) => (None, Some(reason)),
    };
    engine
        .setup
        .public
        .insert(
            run,
            Caller::Connect { forge, request, number: carried.number, pasted, stop: Some(domain::AdminStop::Other) },
        )
        .expect("public room checked");
    let first = match first {
        Some(first) => Some(super::lifecycle::Intent {
            playground: false,
            org: first.org,
            repository: first.repository,
            label: first.label,
        }),
        None => None,
    };
    engine
        .setup
        .runs
        .insert(
            run,
            super::lifecycle::Run {
                forge,
                first,
                kind: super::lifecycle::Kind::AdminReconcile,
                internal: false,
                previous: engine.domain.forge_connection(forge),
            },
        )
        .expect("public run room checked");
    if let Some(reason) = failure {
        super::lifecycle::refused(engine, run, forge, Box::from(reason));
    } else {
        super::run_plan(engine, run, host::Plan::Reconcile { forge, adopted: Box::new([]) });
    }
}
pub(crate) fn connect(engine: &mut Engine, forge: u16, request: Token, app: NonZeroU64, carried: Carried) {
    if full(engine, forge) {
        event(
            engine,
            domain::Event::ConnectAnswered {
                forge,
                request,
                answer: domain::ConnectAnswer::Refused(domain::ForgeRefusal::Busy),
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    event(engine, domain::Event::ConnectAnswered { forge, request, answer: domain::ConnectAnswer::Connecting });
    let value = engine.store.secrets(run, Secrets::Get { owner: SecretOwner::Application, number: carried.number });
    let parsed = match value {
        Some(Event::Secret { value: Some(SecretValue::Carried { kind: CarriedKind::Value, value }), .. }) => {
            if skein_lib::sha256::digest(value.expose()) == carried.digest {
                pasted(value.expose(), 11, b"the pasted value is not a key")
            } else {
                Err(b"the pasted value changed".as_slice())
            }
        }
        Some(_) | None => Err(b"the pasted value changed".as_slice()),
    };
    let (pasted, failure) = match parsed {
        Ok(pem) => (Some(Pasted::App(pem)), None),
        Err(reason) => (None, Some(reason)),
    };
    engine
        .setup
        .public
        .insert(run, Caller::Connect { forge, request, number: carried.number, pasted, stop: None })
        .expect("public room checked");
    if let Some(reason) = failure {
        super::ended(engine, run, root::SetupEnd::Stopped { forge, reason: Box::from(reason) });
    } else {
        stage(engine, run, host::Plan::Paste { forge, app });
    }
}
pub(crate) fn manifest(engine: &mut Engine, forge: u16, request: Token, back: Option<Box<[u8]>>) {
    if full(engine, forge) {
        event(
            engine,
            domain::Event::ConnectAnswered {
                forge,
                request,
                answer: domain::ConnectAnswer::Refused(domain::ForgeRefusal::Busy),
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine.setup.public.insert(run, Caller::Manifest { forge, request, back }).expect("public room checked");
    stage(engine, run, host::Plan::Manifest { forge, target: skein_forge_setup::github::manifest::Target::Personal });
}
fn manifest_link(engine: &Engine, forge: u16, state: &[u8], page: bool) -> Box<[u8]> {
    let number = skein_lib::Decimal::of(u64::from(forge));
    let prefix = if page { b"/".as_slice() } else { engine.setup.listener.as_ref() };
    let parts = [prefix, b"forges/", number.as_bytes(), b"/github/manifest?state=", state];
    let mut size = 0_usize;
    for part in parts {
        size = size.checked_add(part.len()).expect("bounded manifest link");
    }
    let mut writer = skein_lib::Writer::new(size);
    for part in parts {
        writer.put(part).expect("measured manifest link");
    }
    writer.finish()
}
fn pasted(bytes: &[u8], item: u16, invalid: &'static [u8]) -> Result<Secret, &'static [u8]> {
    let (found, value) = match temper_protocol_records::decode_pasted(bytes) {
        Ok(value) => value,
        Err(temper_protocol_records::PastedError::Version) => {
            return Err(temper_protocol_records::PastedError::Version.message());
        }
        Err(temper_protocol_records::PastedError::Unreadable) => return Err(invalid),
    };
    if found != item || value.expose().is_empty() {
        return Err(invalid);
    }
    Ok(value)
}
pub(crate) fn install(engine: &mut Engine, forge: u16, request: Token, back: Option<Box<[u8]>>) {
    // The engine's own run on the forge holds an Install rather than refusing it (setup.md, 5.1).
    let held = engine.setup.busy_internally(forge) && engine.setup.public.len() < engine.setup.public.capacity();
    if !held && full(engine, forge) {
        event(
            engine,
            domain::Event::InstallAnswered {
                forge,
                request,
                answer: domain::InstallAnswer::Refused(domain::ForgeRefusal::Busy),
                stopped: None,
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine.setup.public.insert(run, Caller::Install { forge, request, back }).expect("public room checked");
    stage(engine, run, host::Plan::Install { forge });
}
pub(crate) fn accept(engine: &mut Engine, forge: u16, request: Token, installation: NonZeroU64) {
    if full(engine, forge) {
        event(
            engine,
            domain::Event::AcceptAnswered {
                forge,
                request,
                answer: domain::AcceptAnswer::Refused(domain::ForgeRefusal::Busy),
                stopped: None,
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine
        .setup
        .public
        .insert(
            run,
            Caller::AcceptAsked {
                forge,
                request,
                installation: installation.get(),
                stop: domain::AdoptionRefusal::InvalidAnswer,
            },
        )
        .expect("public room checked");
    stage(engine, run, host::Plan::Accept { forge, installation, state: None });
}
fn matches_app(pasted: Option<&Pasted>) -> bool {
    match pasted {
        Some(Pasted::App(_)) => true,
        Some(Pasted::Admin(_)) | None => false,
    }
}
fn done(end: &root::SetupEnd) -> bool {
    match end {
        root::SetupEnd::Done { .. } => true,
        root::SetupEnd::Stopped { .. } => false,
    }
}
fn accept_answer(end: &root::SetupEnd, installation: u64, stop: domain::AdoptionRefusal) -> domain::AcceptAnswer {
    if done(end) {
        domain::AcceptAnswer::Accepted { installation }
    } else if let root::SetupEnd::Stopped { reason, .. } = end
        && reason.as_ref() == b"setup host: busy"
    {
        domain::AcceptAnswer::Refused(domain::ForgeRefusal::Busy)
    } else {
        domain::AcceptAnswer::ForgeRefused(stop)
    }
}
pub(super) fn ended(engine: &mut Engine, run: Token, end: &root::SetupEnd) -> bool {
    let Some(caller) = engine.setup.public.remove(&run) else { return false };
    if let root::SetupEnd::Stopped { reason, .. } = end {
        engine.notes.push(crate::Note::BoxLink { line: reason.clone() });
    }
    let stopped = match end {
        root::SetupEnd::Stopped { reason, .. } => Some(reason.clone()),
        root::SetupEnd::Done { .. } => None,
    };
    match caller {
        Caller::Manifest { forge, request, back } => {
            let answer = match end {
                root::SetupEnd::Done { outcome: root::SetupOutcome::Manifest { state }, .. } => {
                    let page = back.is_some();
                    hold(engine, forge, state.clone(), back);
                    domain::ConnectAnswer::Link(manifest_link(engine, forge, state, page))
                }
                root::SetupEnd::Done { .. } | root::SetupEnd::Stopped { .. } => {
                    event(
                        engine,
                        domain::Event::ConnectEnded { forge, request, connected: false, why: stopped, stop: None },
                    );
                    domain::ConnectAnswer::Connecting
                }
            };
            event(engine, domain::Event::ConnectAnswered { forge, request, answer });
        }
        Caller::Redirect { forge, call, back } => match end {
            root::SetupEnd::Done { outcome: root::SetupOutcome::Install { state, url }, .. } => {
                hold(engine, forge, state.clone(), back);
                engine.clients.github_answer(call, 302, Some(url), b"");
            }
            root::SetupEnd::Done { .. } | root::SetupEnd::Stopped { .. } => super::routes::ended(engine, call, forge),
        },
        Caller::Connect { forge, request, number, pasted, stop } => {
            if done(end) && matches_app(pasted.as_ref()) {
                engine.setup.pastes.insert(number, forge).expect("one pending connect per forge");
            }
            event(
                engine,
                domain::Event::ConnectEnded {
                    forge,
                    request,
                    connected: done(end),
                    why: stopped,
                    stop: if done(end) { None } else { stop },
                },
            );
        }
        Caller::Install { forge, request, back } => {
            let answer = match end {
                root::SetupEnd::Done { outcome: root::SetupOutcome::Install { state, url }, .. } => {
                    hold(engine, forge, state.clone(), back);
                    domain::InstallAnswer::Link(url.clone())
                }
                root::SetupEnd::Done { .. } | root::SetupEnd::Stopped { .. } => domain::InstallAnswer::Connecting,
            };
            event(engine, domain::Event::InstallAnswered { forge, request, answer, stopped });
        }
        Caller::AcceptAsked { forge, request, installation, stop } => {
            let answer = accept_answer(end, installation, stop);
            event(engine, domain::Event::AcceptAnswered { forge, request, answer, stopped });
        }
        Caller::Reachable { forge, request } => {
            let answer = match end {
                root::SetupEnd::Stopped { reason, .. } if reason.as_ref() == b"setup host: busy" => {
                    domain::ReachableAnswer::Refused(domain::ForgeRefusal::Busy)
                }
                root::SetupEnd::Done { .. } | root::SetupEnd::Stopped { .. } => {
                    domain::ReachableAnswer::ForgeRefused(domain::AdoptionRefusal::Unreachable)
                }
            };
            event(engine, domain::Event::ReachableAnswered { forge, request, answer });
        }
        Caller::Accept { forge, call, back } => match end {
            root::SetupEnd::Done { .. } => {
                if let Some(back) = back {
                    engine.clients.github_answer(
                        call,
                        303,
                        Some(&back),
                        b"temper has the installation; you may close this tab.",
                    );
                } else {
                    super::routes::page(
                        &mut engine.clients,
                        call,
                        200,
                        b"GitHub is connected",
                        b"temper has the installation; you may close this tab.",
                        b"/forges",
                    );
                }
            }
            root::SetupEnd::Stopped { .. } => super::routes::ended(engine, call, forge),
        },
    }
    true
}
pub(crate) fn erase(engine: &mut Engine, number: u64) {
    assert!(number >= temper_protocol_records::CARRIED, "only carried rows erased");
    let owner = if engine.forge_records.erasing(number) {
        crate::forge::records::LOAD
    } else {
        super::lifecycle::next_run(&mut engine.setup.lifecycle)
    };
    if let Some(forge) = engine.setup.pastes.remove(&number) {
        engine.setup.erases.insert(owner, forge).expect("one pending erase per forge");
    }
    if let Some(event) = engine.store.secrets(owner, Secrets::Erase { owner: SecretOwner::Application, number }) {
        super::answer(engine, event);
    }
}
pub(super) fn hold(engine: &mut Engine, forge: u16, state: Box<[u8]>, back: Option<Box<[u8]>>) {
    if engine.setup.backs.room() == 0 {
        let _oldest = engine.setup.backs.pop();
    }
    engine.setup.backs.push(ReturnTo { forge, state, back, until: engine.env.now.saturating_add(engine.setup.life) });
}
pub(super) fn take_back(engine: &mut Engine, forge: u16, state: &[u8]) -> Option<ReturnTo> {
    let mut found = None;
    for _ in 0..engine.setup.backs.len() {
        let back = engine.setup.backs.pop().expect("back count");
        if back.until <= engine.env.now {
            continue;
        }
        if back.forge == forge && back.state.as_ref() == state {
            found = Some(back);
        } else {
            engine.setup.backs.push(back);
        }
    }
    found
}

pub(crate) fn reachable(engine: &mut Engine, forge: u16, request: Token, installation: Option<u64>, page: u32) {
    if engine.setup.reading(forge)
        || engine.setup.runs.len() == engine.setup.env.limits.runs
        || engine.setup.public.len() == engine.setup.public.capacity()
    {
        event(
            engine,
            domain::Event::ReachableAnswered {
                forge,
                request,
                answer: domain::ReachableAnswer::Refused(domain::ForgeRefusal::Busy),
            },
        );
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine.setup.public.insert(run, Caller::Reachable { forge, request }).expect("public room checked");
    stage(
        engine,
        run,
        host::Plan::Reachable {
            forge,
            installation,
            page: core::num::NonZeroU32::new(page).expect("admitted nonzero page"),
        },
    );
}
pub(super) fn reached(engine: &mut Engine, run: Token, answer: domain::ReachableAnswer) {
    let Some(Caller::Reachable { forge, request }) = engine.setup.public.remove(&run) else {
        unreachable!("Reachable has its public caller")
    };
    event(engine, domain::Event::ReachableAnswered { forge, request, answer });
}

/// Preserve structured failures before the diagnostic is formatted or bounded.
pub(super) fn stopped(engine: &mut Engine, run: Token, failure: &skein_forge_setup::reconcile::Failure) {
    use skein_forge_setup::reconcile::{Stage, Why};
    if let Some(Caller::AcceptAsked { forge, installation, .. }) = engine.setup.public.get(&run) {
        let reason = accept_failure(engine, *forge, *installation, failure);
        if let Some(Caller::AcceptAsked { stop, .. }) = engine.setup.public.get_mut(&run) {
            *stop = reason;
        }
    }
    if let Some(Caller::Connect { stop: Some(stop), .. }) = engine.setup.public.get_mut(&run) {
        *stop = match failure.why {
            Why::NotAdmin => domain::AdminStop::NotAdmin,
            Why::Refused(status) => domain::AdminStop::Refused {
                status,
                stage: match failure.step {
                    Stage::Admin => domain::SetupStage::Admin,
                    Stage::Bot => domain::SetupStage::Bot,
                    Stage::Token => domain::SetupStage::Token,
                    Stage::Org => domain::SetupStage::Org,
                    Stage::Repository => domain::SetupStage::Repository,
                    Stage::File => domain::SetupStage::File,
                    Stage::App => domain::SetupStage::App,
                    Stage::Installation => domain::SetupStage::Installation,
                    Stage::State => domain::SetupStage::State,
                },
            },
            Why::Unmarked => domain::AdminStop::Unmarked,
            Why::Missing
            | Why::BotIsAdmin
            | Why::OtherUser
            | Why::OtherApp
            | Why::Unknown
            | Why::Busy
            | Why::Crowded
            | Why::Down
            | Why::Unexpected
            | Why::KeyForm => domain::AdminStop::Other,
        };
    }
}

// client/README.md, 4.6: classify the structured stop, never its diagnostic text.
fn accept_failure(
    engine: &Engine,
    forge: u16,
    installation: u64,
    failure: &skein_forge_setup::reconcile::Failure,
) -> domain::AdoptionRefusal {
    use skein_forge_setup::reconcile::{Stage, Why};
    match failure.why {
        Why::Missing => domain::AdoptionRefusal::NotFound,
        Why::Refused(_) | Why::KeyForm => domain::AdoptionRefusal::NoAccess,
        Why::Down => domain::AdoptionRefusal::Unreachable,
        Why::Unexpected if failure.step == Stage::Installation => {
            let ids = super::lifecycle::accepted(engine, forge);
            if ids.len() == usize::try_from(engine.forge_records.bounds.installations).expect("installation bound")
                && !ids.contains(&installation)
            {
                domain::AdoptionRefusal::TooLarge
            } else {
                domain::AdoptionRefusal::InvalidAnswer
            }
        }
        Why::Unexpected
        | Why::NotAdmin
        | Why::BotIsAdmin
        | Why::Unmarked
        | Why::OtherUser
        | Why::OtherApp
        | Why::Unknown
        | Why::Busy
        | Why::Crowded => domain::AdoptionRefusal::InvalidAnswer,
    }
}
