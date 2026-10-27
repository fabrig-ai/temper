//! A core-admitted keyed adoption and its retained terminal (domain/forge.md, 4).
use crate::domain::{child, emit};
use crate::{
    Adopted, AdoptedRepository, Adoption, AdoptionRefusal, ApplicationOutcome, Domain, Key, Kinds, Limits, Protection,
    Repository, Request, Role, Stored, reason,
};
use jig_domain_people as people;
use skein_forge::api;
use skein_lib::{Duration, Env, List, Map, Queue, Token, Wall};

#[derive(Debug)]
pub(crate) struct Pending {
    token: Token,
    retention: Duration,
    observed: Option<ApplicationOutcome>,
}

#[derive(Debug)]
pub(crate) struct State {
    pending: Map<Token, Pending>,
    kept: Map<Token, (Wall, ApplicationOutcome)>,
}

impl State {
    pub(crate) fn restore(&mut self, token: u64, until: Wall, outcome: ApplicationOutcome) {
        assert!(self.kept.insert(Token::new(token), (until, outcome)).is_ok(), "restored application outcome fits");
    }
    pub(crate) fn new(limits: &Limits) -> Self {
        Self { pending: Map::with_capacity(limits.adoptions), kept: Map::with_capacity(limits.applications) }
    }
}

impl Domain {
    /// The repository located for this core-admitted application.
    #[must_use]
    pub fn applying(&self, request: Token) -> Option<api::Repository> {
        let pending = self.adoptions.get(&request)?;
        if pending.repository.repository == 0 { None } else { Some(pending.repository) }
    }

    /// The forge's outcome under the people child's token, while retained.
    #[must_use]
    pub fn application_outcome(&self, token: Token) -> Option<&ApplicationOutcome> {
        match self.application.kept.get(&token) {
            Some((_, outcome)) => Some(outcome),
            None => None,
        }
    }
}

pub(crate) fn start(
    d: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    token: Token,
    retention: Duration,
    adoption: Adoption,
    out: &mut Queue<Request>,
) {
    let mut expired = None;
    for (token, (until, _)) in &d.application.kept {
        if *until <= env.wall {
            expired = Some(*token);
            break;
        }
    }
    if let Some(token) = expired {
        d.application.kept.remove(&token);
        out.push(Request::Erase { key: Key::Application(token.raw()) });
    }
    if d.application.kept.len().saturating_add(d.application.pending.len()) >= env.limits.applications
        || d.application.pending.len() >= env.limits.adoptions
    {
        return again(request, people::Again::Busy, out);
    }
    let pending = Pending { token, retention, observed: None };
    if !d.identities.contains_key(&adoption.forge) && !d.writers.contains_key(&adoption.forge) {
        return finish(d, env, request, pending, ApplicationOutcome::ForgeRefused(AdoptionRefusal::NoSuchForge), out);
    }
    let connected = match d.connection(adoption.forge) {
        crate::Connection::Connected { .. } => true,
        crate::Connection::NotSetUp
        | crate::Connection::SettingUp { .. }
        | crate::Connection::SetUpFailed { .. }
        | crate::Connection::Connecting
        | crate::Connection::Stopped { .. } => false,
        // Direct domain worlds may supply writers; the service configures none.
        crate::Connection::NotConnected => d.writers.contains_key(&adoption.forge),
    };
    if d.forge_ended(adoption.forge) || !connected {
        return finish(
            d,
            env,
            request,
            pending,
            ApplicationOutcome::ForgeRefused(reason(api::Error::NotConnected)),
            out,
        );
    }
    assert!(d.application.pending.insert(request, pending).is_ok(), "reserved application flight");
    if invalid_fields(&adoption, &env.limits) {
        let pending = d.application.pending.remove(&request).expect("admitted application");
        return finish(d, env, request, pending, ApplicationOutcome::ForgeRefused(AdoptionRefusal::TooLarge), out);
    }
    let repository = api::Repository { forge: adoption.forge, repository: 0 };
    let read = api::Read::Locate { owner: adoption.owner.clone(), name: adoption.name.clone() };
    let pending = PendingAdoption {
        request: adoption,
        repository,
        stage: AdoptionStage::Locate,
        permission: None,
        settings: None,
        protection: Protection::Unknown,
        collaborators: List::with_capacity(env.limits.collaborators),
    };
    assert!(d.adoptions.insert(request, pending).is_ok(), "reserved adoption flight");
    child(d, env, skein_forge_client::Event::Read { owner: request, repository, read }, out);
}

/// Record exactly what the adoption reads decided before forwarding to the core.
pub(crate) fn adopted(d: &mut Domain, out: &mut Queue<Request>, reply_to: Token, result: Result<Adopted, api::Error>) {
    if let Some(pending) = d.application.pending.get_mut(&reply_to) {
        pending.observed = Some(match &result {
            Ok(adopted) => {
                let row = &adopted.repository;
                ApplicationOutcome::Adopted(AdoptedRepository {
                    repository: row.provider,
                    owner: row.owner.clone(),
                    name: row.name.clone(),
                    role: row.role,
                    home: row.home,
                    kinds: row.kinds,
                    protection: row.protection.clone(),
                })
            }
            Err(error) => ApplicationOutcome::ForgeRefused(reason(*error)),
        });
    }
    out.push(Request::Adopted { reply_to, result });
}

// The core's part still sees the provider-shaped refusal; the application
// retains the admission decision, which the provider's Error cannot express.
fn refused(d: &mut Domain, out: &mut Queue<Request>, request: Token, why: AdoptionRefusal) {
    adopted(d, out, request, Err(api::Error::Refused));
    if let Some(pending) = d.application.pending.get_mut(&request) {
        pending.observed = Some(ApplicationOutcome::ForgeRefused(why));
    }
}

pub(crate) fn answered(
    d: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    outcome: people::Outcome,
    out: &mut Queue<Request>,
) {
    let Some(mut pending) = d.application.pending.remove(&request) else { return };
    d.adoptions.remove(&request);
    let outcome = if let Some(ApplicationOutcome::ForgeRefused(why)) = &pending.observed {
        ApplicationOutcome::ForgeRefused(*why)
    } else if let people::Outcome::Refused(why) = outcome {
        ApplicationOutcome::CoreRefused(why)
    } else if let people::Outcome::Again(why) = outcome {
        return again(request, why, out);
    } else if let people::Outcome::Adopted { .. } = outcome {
        pending.observed.take().expect("adoption reads recorded the repository")
    } else {
        unreachable!("the core's adoption part ends with an adoption outcome");
    };
    finish(d, env, request, pending, outcome, out);
}

fn finish(
    d: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    pending: Pending,
    outcome: ApplicationOutcome,
    out: &mut Queue<Request>,
) {
    let until = Wall::from_nanos(env.wall.as_nanos().saturating_add(pending.retention.as_nanos()));
    let row = Stored::Application { token: pending.token.raw(), until, outcome: outcome.clone() };
    assert!(d.application.kept.insert(pending.token, (until, outcome)).is_ok(), "reserved application outcome");
    out.push(Request::Save { record: row, release: None, read_afresh: None });
    out.push(Request::ApplicationDone { request, token: pending.token });
}

fn again(request: Token, why: people::Again, out: &mut Queue<Request>) {
    out.push(Request::ApplicationAgain { request, why });
}

pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    let payload = u64::from(limits.name_bytes).checked_mul(2)?.checked_add(u64::from(limits.client.answer_bytes))?;
    let request = u64::from(limits.name_bytes)
        .checked_mul(5)?
        .checked_add(u64::from(limits.change_policy.gates).checked_mul(4)?)?
        .checked_add(List::<api::Collaborator>::worst_case(limits.collaborators)?)?;
    Map::<Token, Pending>::worst_case(limits.adoptions)?
        .checked_add(Map::<Token, (Wall, ApplicationOutcome)>::worst_case(limits.applications)?)?
        .checked_add(request.checked_mul(u64::from(limits.adoptions))?)?
        .checked_add(payload.checked_mul(u64::from(limits.adoptions).checked_add(u64::from(limits.applications))?)?)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AdoptionStage {
    Locate,
    Part,
    Permission,
    Branches,
    Settings,
    Protection,
    Collaborators(u32),
}

/// One admission read chain; only rows proven from current provider facts
/// can become durable adopted repositories.
#[derive(Debug)]
pub(crate) struct PendingAdoption {
    request: Adoption,
    repository: api::Repository,
    stage: AdoptionStage,
    permission: Option<api::Permission>,
    settings: Option<api::Settings>,
    protection: Protection,
    collaborators: List<api::Collaborator>,
}

pub(crate) fn adopt(d: &mut Domain, env: &Env<Limits>, reply_to: Token, out: &mut Queue<Request>) {
    let Some(mut pending) = d.adoptions.remove(&reply_to) else { return };
    let request = &pending.request;
    let repository = pending.repository;
    let invalid = !d.repositories.contains_key(&repository) && d.repositories.len() == env.limits.repositories;
    if invalid {
        adopted(d, out, reply_to, Err(api::Error::TooLarge));
        return;
    }
    if let Some(existing) = d.repositories.get(&repository)
        && existing.project != request.project
        && existing.role != Role::Context
        && request.role != Role::Context
    {
        refused(d, out, reply_to, AdoptionRefusal::AnotherProject);
        return;
    }
    if request.home && request.role == Role::Context {
        adopted(d, out, reply_to, Err(api::Error::Refused));
        return;
    }
    if !request.ci && request.checks.is_empty() {
        adopted(d, out, reply_to, Err(api::Error::Refused));
        return;
    }
    if request.checks.len() > usize::try_from(env.limits.change_policy.gates).expect("u32 fits usize")
        || request.checks.contains(&0)
    {
        adopted(d, out, reply_to, Err(api::Error::TooLarge));
        return;
    }
    for (index, check) in request.checks.iter().enumerate() {
        if request.checks.get(..index).expect("index in checks").contains(check) {
            adopted(d, out, reply_to, Err(api::Error::Refused));
            return;
        }
    }
    if request.home {
        for (_, existing) in &d.repositories {
            if existing.project == request.project
                && existing.home
                && existing.provider != repository
                && !d.forge_ended(existing.provider.forge)
            {
                refused(d, out, reply_to, AdoptionRefusal::HomeHeld);
                return;
            }
        }
    }
    for (_, pending) in &d.adoptions {
        if pending.repository == repository {
            adopted(d, out, reply_to, Err(api::Error::Busy));
            return;
        }
    }
    let author = *d.writers.get(&request.forge).expect("application checked configured forge");
    pending.stage = AdoptionStage::Permission;
    d.adoptions.insert(reply_to, pending).expect("adoption capacity checked");
    child(
        d,
        env,
        skein_forge_client::Event::Read { owner: reply_to, repository, read: api::Read::Permission { user: author } },
        out,
    );
}

fn owns_branch(d: &Domain, repository: api::Repository, branch: &[u8]) -> bool {
    for (name, head) in &d.heads {
        if head.owned
            && name.forge == repository.forge
            && name.repository == repository.repository
            && crate::items::branch_matches(name, branch)
        {
            return true;
        }
    }
    false
}

pub(crate) fn adoption_read(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    result: Result<api::Answer, api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(mut pending) = d.adoptions.remove(&owner) else { return };
    let repository = pending.repository;
    match pending.stage {
        AdoptionStage::Locate => match result {
            Ok(api::Answer::Located { repository: id }) if id != 0 => {
                pending.repository.repository = id;
                pending.stage = AdoptionStage::Part;
                let repository = pending.repository;
                let adoption = pending.request.clone();
                d.adoptions.insert(owner, pending).expect("replaces locate flight");
                out.push(Request::ApplicationPart { request: owner, adoption, repository });
            }
            Err(error) => locate_refused(d, env, owner, reason(error), out),
            Ok(_) => locate_refused(d, env, owner, AdoptionRefusal::InvalidAnswer, out),
        },
        AdoptionStage::Part => unreachable!("no read while the application waits for the core"),
        AdoptionStage::Permission => match result {
            Ok(api::Answer::Permission(permission)) => {
                if permission == api::Permission::None {
                    adopted(d, out, owner, Err(api::Error::Forbidden));
                    return;
                }
                pending.permission = Some(permission);
                let check_prefix = pending.request.role != Role::Context;
                pending.stage = if check_prefix { AdoptionStage::Branches } else { AdoptionStage::Settings };
                let read = if check_prefix {
                    api::Read::Branches { prefix: pending.request.prefix.clone() }
                } else {
                    api::Read::Settings
                };
                d.adoptions.insert(owner, pending).expect("replaces read chain");
                child(d, env, skein_forge_client::Event::Read { owner, repository, read }, out);
            }
            Ok(_) => adopted(d, out, owner, Err(api::Error::InvalidAnswer)),
            Err(error) => adopted(d, out, owner, Err(error)),
        },
        AdoptionStage::Branches => match result {
            Ok(api::Answer::Branches(branches)) => {
                for branch in branches {
                    if branch.starts_with(&pending.request.prefix) && !owns_branch(d, repository, &branch) {
                        refused(d, out, owner, AdoptionRefusal::PrefixOccupied);
                        return;
                    }
                }
                pending.stage = AdoptionStage::Settings;
                d.adoptions.insert(owner, pending).expect("replaces read chain");
                child(d, env, skein_forge_client::Event::Read { owner, repository, read: api::Read::Settings }, out);
            }
            Ok(_) => adopted(d, out, owner, Err(api::Error::InvalidAnswer)),
            Err(error) => adopted(d, out, owner, Err(error)),
        },
        AdoptionStage::Settings => match result {
            Ok(api::Answer::Settings(settings)) => {
                pending.settings = Some(settings);
                pending.stage = AdoptionStage::Protection;
                let branch = pending.request.landing.clone();
                d.adoptions.insert(owner, pending).expect("replaces read chain");
                child(
                    d,
                    env,
                    skein_forge_client::Event::Read { owner, repository, read: api::Read::Protection { branch } },
                    out,
                );
            }
            Ok(_) => adopted(d, out, owner, Err(api::Error::InvalidAnswer)),
            Err(error) => adopted(d, out, owner, Err(error)),
        },
        AdoptionStage::Protection => {
            match result {
                Ok(api::Answer::Protection(Some(protection))) => {
                    pending.protection = Protection::Rule(protection);
                }
                Ok(api::Answer::Protection(None)) => pending.protection = Protection::Absent,
                Err(api::Error::Forbidden) => pending.protection = Protection::Unknown,
                Err(error) => {
                    adopted(d, out, owner, Err(error));
                    return;
                }
                Ok(_) => {
                    adopted(d, out, owner, Err(api::Error::InvalidAnswer));
                    return;
                }
            }
            pending.stage = AdoptionStage::Collaborators(1);
            d.adoptions.insert(owner, pending).expect("replaces read chain");
            child(
                d,
                env,
                skein_forge_client::Event::Read { owner, repository, read: api::Read::Collaborators { page: 1 } },
                out,
            );
        }
        AdoptionStage::Collaborators(page) => adoption_collaborators(d, env, owner, pending, page, result, out),
    }
}

fn adoption_collaborators(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    mut pending: PendingAdoption,
    page: u32,
    result: Result<api::Answer, api::Error>,
    out: &mut Queue<Request>,
) {
    let repository = pending.repository;
    match result {
        Ok(api::Answer::Collaborators { collaborators, more }) => {
            for person in collaborators {
                if pending.collaborators.push(person).is_err() {
                    adopted(d, out, owner, Err(api::Error::TooLarge));
                    return;
                }
            }
            if more {
                let Some(next) = page.checked_add(1) else {
                    adopted(d, out, owner, Err(api::Error::TooLarge));
                    return;
                };
                pending.stage = AdoptionStage::Collaborators(next);
                d.adoptions.insert(owner, pending).expect("replaces read chain");
                child(
                    d,
                    env,
                    skein_forge_client::Event::Read {
                        owner,
                        repository,
                        read: api::Read::Collaborators { page: next },
                    },
                    out,
                );
            } else {
                finish_adoption(d, owner, pending, out);
            }
        }
        Ok(_) => adopted(d, out, owner, Err(api::Error::InvalidAnswer)),
        Err(error) => adopted(d, out, owner, Err(error)),
    }
}

fn finish_adoption(d: &mut Domain, owner: Token, pending: PendingAdoption, out: &mut Queue<Request>) {
    let Some(permission) = pending.permission else { unreachable!("permission read before collaborators") };
    let Some(settings) = pending.settings else { unreachable!("settings read before collaborators") };
    let writable = match permission {
        api::Permission::Write | api::Permission::Admin => true,
        api::Permission::None | api::Permission::Read => false,
    } && pending.request.role != Role::Context;
    let kinds = Kinds {
        read: true,
        push: writable,
        open: writable,
        land: writable && (settings.merge || settings.squash || settings.rebase),
        review: writable,
        status: writable,
        comment: writable,
        issue: writable,
        branch: writable,
    };
    let request = pending.request;
    let row = Repository {
        project: request.project,
        home: request.home,
        provider: pending.repository,
        host: request.host,
        owner: request.owner,
        name: request.name,
        prefix: request.prefix,
        role: request.role,
        ci: request.ci,
        checks: request.checks,
        kinds,
        protection: pending.protection,
        settings,
    };
    // The core keeps each project's read grant. This row keeps the writer:
    // another project's context adoption must leave its home and effects alone.
    let writer_held = row.role == Role::Context
        && match d.repositories.get(&row.provider) {
            Some(existing) => existing.project != row.project && existing.role != Role::Context,
            None => false,
        };
    if !writer_held {
        d.repositories.insert(row.provider, row.clone()).expect("adoption capacity checked");
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Repository(row.clone()) });
    }
    // The forges row lists its adoptions (client/forge.md, 5.4).
    emit(out, Request::MarkedForges);
    adopted(d, out, owner, Ok(Adopted { repository: row, collaborators: pending.collaborators.into_boxed() }));
}

fn invalid_fields(request: &Adoption, limits: &Limits) -> bool {
    let bound = usize::try_from(limits.name_bytes).expect("u32 fits usize");
    for field in [&request.host, &request.owner, &request.name, &request.landing, &request.prefix] {
        if field.is_empty() || field.len() > bound {
            return true;
        }
    }
    // resource_name counts forge/host/owner/repository/branch first (five).
    // tree_branch appends <root>/<c|r|s><task>[-<attempt>] to the prefix:
    // two branch parts, plus one for every slash already in the prefix.
    let mut segments = 7_u64;
    for byte in &request.prefix {
        if *byte == b'/' {
            segments = segments.checked_add(1).expect("prefix length is bounded by u32 name_bytes");
        }
    }
    if segments > u64::from(limits.authority_segments) {
        return true;
    }
    request.checks.len() > usize::try_from(limits.change_policy.gates).expect("u32 fits usize")
}

fn locate_refused(d: &mut Domain, env: &Env<Limits>, request: Token, why: AdoptionRefusal, out: &mut Queue<Request>) {
    let pending = d.application.pending.remove(&request).expect("locate belongs to an application");
    finish(d, env, request, pending, ApplicationOutcome::ForgeRefused(why), out);
}
