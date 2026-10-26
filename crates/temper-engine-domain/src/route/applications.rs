//! The core admits each typed application before its forge connector decides it.
use super::{Domain, Work, forge, forge_route};
use crate::{Application, ApplicationAnswer};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{Duration, ReplyTo, Token};

/// The application's admission as the core decides it, before its owner is named.
pub(super) struct Admission {
    pub(super) reply_to: ReplyTo,
    pub(super) sign_in: u64,
    pub(super) key: [u8; 16],
    pub(super) digest: [u8; 32],
    pub(super) scope: people::ApplicationScope,
    pub(super) carried: Option<core::Carried>,
}

pub(super) fn start(domain: &mut Domain, admission: Admission, request: Application) {
    assert!(domain.application.is_none(), "one application admission value");
    let owner = Token::new(domain.next_application_owner);
    domain.next_application_owner = domain.next_application_owner.checked_add(1).expect("application owner counter");
    let Admission { reply_to, sign_in, key, digest, scope, carried } = admission;
    let scope = match &request {
        Application::Forge(
            forge::Ask::SetUp { .. }
            | forge::Ask::StartDemo
            | forge::Ask::Connect { .. }
            | forge::Ask::Install { .. }
            | forge::Ask::Accept { .. }
            | forge::Ask::Reachable { .. },
        ) => people::ApplicationScope::Deployment,
        Application::Forge(forge::Ask::Adopt(_)) => scope,
        Application::Return { .. } => people::ApplicationScope::Own,
    };
    if request == Application::Forge(forge::Ask::StartDemo) {
        domain.application_demo = Some(super::demo::Context { sign_in, key, carried });
    }
    domain.application_carried = carried;
    domain.application = Some((owner, request));
    domain.work.push(Work::Core(core::Event::Application {
        reply_to,
        sign_in,
        key,
        scope,
        connector: domain.config.forge_connector,
        digest,
        owner,
        carried,
    }));
}

pub(super) fn admitted(
    domain: &mut Domain,
    request: Token,
    token: Token,
    owner: Token,
    retention: Duration,
    role: Option<people::Role>,
    env: &skein_lib::Env<crate::Limits>,
) {
    let (held, ask) = domain.application.take().expect("one admitted application value");
    assert_eq!(held, owner, "the core admits the application's owner");
    let carried = domain.application_carried.take();
    match ask {
        Application::Forge(ask) => {
            if let Some(context) = domain.application_demo.take() {
                if domain.demo_contexts.len() == domain.demo_contexts.capacity() {
                    finished(domain, request, Err(people::Again::Busy));
                    return;
                }
                assert!(domain.demo_contexts.insert(request, context).is_ok(), "bounded admitted starts");
            }
            domain.work.push(Work::Forge(forge::Event::Application { request, token, retention, ask, role, carried }));
        }
        Application::Return { source, path } => {
            // Only live keyed answers need a volatile marker; the core bounds their room.
            loop {
                let mut expired = None;
                for (held, until) in &domain.returns {
                    if until.0 <= env.now || until.1 <= env.wall {
                        expired = Some(*held);
                        break;
                    }
                }
                let Some(expired) = expired else { break };
                domain.returns.remove(&expired);
            }
            domain
                .returns
                .insert(
                    token,
                    (
                        env.now.saturating_add(retention),
                        skein_lib::Wall::from_nanos(env.wall.as_nanos().saturating_add(retention.as_nanos())),
                    ),
                )
                .expect("one marker per retained key");
            assert!(super::now(domain, crate::Request::Return { source, path }), "return output door reserved");
            finished(domain, request, Ok(token));
        }
    }
}

pub(super) fn drop(domain: &mut Domain, owner: Token) {
    if let Some((held, _)) = &domain.application {
        assert_eq!(*held, owner, "drop names the application owner");
        domain.application = None;
        domain.application_demo = None;
        domain.application_carried = None;
    }
}

pub(super) fn part(
    domain: &mut Domain,
    request: Token,
    adoption: forge::Adoption,
    repository: skein_forge::api::Repository,
) {
    match forge_route::adopt_repository_ask(adoption, repository, domain.config.forge_connector) {
        Some(ask) => domain.work.push(Work::Core(core::Event::ApplicationPart { request, ask: Box::new(ask) })),
        None => domain.work.push(Work::Forge(forge::Event::ApplicationAnswered {
            request,
            outcome: people::Outcome::Refused(people::Refusal::Unknown),
        })),
    }
}

pub(super) fn finished(domain: &mut Domain, request: Token, outcome: Result<Token, people::Again>) {
    domain.work.push(Work::Core(core::Event::ApplicationFinished { request, outcome }));
}

pub(super) fn answer(domain: &Domain, token: Token) -> Option<ApplicationAnswer> {
    if domain.returns.contains_key(&token) {
        return Some(ApplicationAnswer::Returning);
    }
    match domain.forge.application_answer(token) {
        Some(forge::Answered::Adopt(answer)) => Some(ApplicationAnswer::Adopt(answer)),
        Some(forge::Answered::SetUp(answer)) => Some(ApplicationAnswer::SetUp(answer)),
        Some(forge::Answered::StartDemo(answer)) => Some(ApplicationAnswer::StartDemo(answer)),
        Some(forge::Answered::Connect(answer)) => Some(ApplicationAnswer::Connect(answer)),
        Some(forge::Answered::Reachable(answer)) => Some(ApplicationAnswer::Reachable(answer)),
        Some(forge::Answered::Accept(answer)) => Some(ApplicationAnswer::Accept(answer)),
        Some(forge::Answered::Install(answer)) => Some(ApplicationAnswer::Install(answer)),
        None => None,
    }
}
