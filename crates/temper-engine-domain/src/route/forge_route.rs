//! Root translation for the forge connector. Its store rows and released API
//! calls cross one root decision; the connector never owns the store.
use super::{
    CallAnswer, CallKey, Decision, Delivery, Domain, Env, Family, ForgeRepository, ForgeStart, ForgeWorkspace, Id, Key,
    Limits, List, ProcedureStep, Queue, Record, ReplyTo, Request, StepEnd, Token, Work, Write, authority,
    connector_answer, core_call, decide_call, emit, escalation, forge, forge_change, forge_issues, people, save,
    save_connector_answer, tasks,
};
use crate::commit_text;
use alloc::boxed::Box;
use jig_domain_brief as brief;
use jig_domain_core::connector::{EffectDescription, EffectForm, Recovery};

#[cfg(test)]
mod tests;

/// Translate the worker's connector-specific saved tags at the root boundary.
pub(crate) fn saved_resources(connector: u16, tags: &[u64]) -> Option<Box<[tasks::SavedResource]>> {
    let mut resources = List::with_capacity(u32::try_from(tags.len()).ok()?);
    for tag in tags {
        let (forge, repository) = forge::untag(*tag)?;
        resources
            .push(tasks::SavedResource {
                connector,
                path: Box::new([Box::from(forge.to_be_bytes()), Box::from(repository.to_be_bytes())]),
            })
            .ok()?;
    }
    Some(resources.into_boxed())
}

/// Return the forge's resource tags from task-owned generic saved names.
pub(crate) fn saved_tags(resources: &[tasks::SavedResource], connector: u16) -> Option<Box<[u64]>> {
    let mut tags = List::with_capacity(u32::try_from(resources.len()).ok()?);
    for resource in resources {
        if resource.connector != connector {
            continue;
        }
        let [forge, repository] = resource.path.as_ref() else { return None };
        let forge = u16::from_be_bytes(forge.as_ref().try_into().ok()?);
        let repository = u32::from_be_bytes(repository.as_ref().try_into().ok()?);
        tags.push(forge::tag(forge, repository)).ok()?;
    }
    Some(tags.into_boxed())
}

/// Build a generic people request from temper's forge repository options.
/// The people child retains only the resource name, role and opaque options.
#[must_use]
pub fn adopt_repository_ask(
    adoption: forge::Adoption,
    repository: skein_forge::api::Repository,
    connector: u16,
) -> Option<people::Ask> {
    let role = match adoption.role {
        forge::Role::Owned => people::ResourceRole::Owned,
        forge::Role::Fork => people::ResourceRole::Fork,
        forge::Role::Context => people::ResourceRole::Context,
        forge::Role::Adopted => return None,
    };
    let count = u32::try_from(adoption.checks.len()).ok()?.checked_add(7)?;
    let mut options = List::with_capacity(count);
    options.push(Box::from([u8::from(adoption.home)])).ok()?;
    options.push(adoption.host).ok()?;
    options.push(adoption.owner).ok()?;
    options.push(adoption.name).ok()?;
    options.push(adoption.prefix).ok()?;
    options.push(adoption.landing).ok()?;
    options.push(Box::from([u8::from(adoption.ci)])).ok()?;
    for check in adoption.checks {
        options.push(Box::from(check.to_be_bytes())).ok()?;
    }
    Some(people::Ask::Adopt {
        project: adoption.project,
        adoption: people::Adoption {
            resource: people::ResourceName {
                connector,
                path: Box::new([
                    Box::from(repository.forge.to_be_bytes()),
                    Box::from(repository.repository.to_be_bytes()),
                ]),
            },
            role,
            options: options.into_boxed(),
        },
    })
}

fn permission_number(permission: skein_forge::api::Permission) -> u16 {
    match permission {
        skein_forge::api::Permission::None => 0,
        skein_forge::api::Permission::Read => 1,
        skein_forge::api::Permission::Write => 2,
        skein_forge::api::Permission::Admin => 3,
    }
}

/// The project's role for a collaborator's forge permission (domain/forge.md, 4).
fn seeded_role(domain: &Domain, project: u32, permission: skein_forge::api::Permission) -> Option<people::Role> {
    let number = permission_number(permission);
    let mappings = domain.core.permission_roles.get(&project)?;
    for mapping in mappings.as_ref() {
        if mapping.connector == domain.config.forge_connector && mapping.permission == number {
            return Some(people::Role::from_number(mapping.role));
        }
    }
    None
}

fn append(out: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        if out.room() > 0 {
            out.push(*byte).expect("checked message room");
        }
    }
}

fn append_hex(out: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        append(out, &forge::hex(*byte));
    }
}

fn landing_words(
    merge: &skein_forge::api::Commit,
    bound: u32,
    repository: Option<&forge::Repository>,
    pull: u64,
    configured: Option<&forge::Configured>,
) -> Box<[u8]> {
    let digits = commit_text(merge);
    let bound = usize::try_from(bound).expect("u32 fits usize");
    let length = digits.len().min(bound);
    let id = digits.get(..length).expect("landing words fit merge id");
    let line = match repository {
        Some(repository) if length == digits.len() => match bound.checked_sub(length.saturating_add(1)) {
            Some(room) => landing_pull(repository, pull, configured, room),
            None => None,
        },
        Some(_) | None => None,
    };
    let Some(line) = line else {
        return Box::from(id);
    };
    let bytes = length.checked_add(1).expect("id separator").checked_add(line.len()).expect("bounded line");
    let mut out = List::with_capacity(u32::try_from(bytes).expect("within contract bound"));
    append(&mut out, id);
    append(&mut out, b"\n");
    append(&mut out, &line);
    out.into_boxed()
}

fn landing_pull(
    repository: &forge::Repository,
    pull: u64,
    configured: Option<&forge::Configured>,
    bound: usize,
) -> Option<Box<[u8]>> {
    let number = skein_lib::Decimal::of(pull);
    let parts: [&[u8]; 8] = [
        b"pull request #",
        number.as_bytes(),
        b" of ",
        &repository.owner,
        b"/",
        &repository.name,
        b" on ",
        &repository.host,
    ];
    let mut bytes = 0_usize;
    for part in parts {
        bytes = bytes.checked_add(part.len())?;
    }
    // The descriptive line is atomic even when the contract shortens the id.
    if bytes > bound {
        return None;
    }
    let address = match configured {
        Some(forge) => {
            forge::pull_address(forge.kind, &forge.git, forge.web.as_deref(), &repository.owner, &repository.name, pull)
        }
        None => None,
    };
    if let Some(address) = &address {
        bytes = bytes.checked_add(2)?.checked_add(address.len())?;
    }
    if bytes > bound {
        return None;
    }
    let mut out = List::with_capacity(u32::try_from(bytes).ok()?);
    for part in parts {
        append(&mut out, part);
    }
    if let Some(address) = address {
        append(&mut out, b": ");
        append(&mut out, &address);
    }
    Some(out.into_boxed())
}

fn news_words(news: &forge::News, limit: u32) -> Box<[u8]> {
    let mut out = List::with_capacity(limit);
    match news {
        forge::News::Landing { before, after, .. } => {
            append(&mut out, b"forge landing ");
            append(&mut out, &commit_text(before));
            append(&mut out, b" -> ");
            append(&mut out, &commit_text(after));
        }
        forge::News::Ci { head, status } => {
            append(&mut out, b"forge ci ");
            let label: &[u8] = match status {
                skein_forge::api::Ci::None => b"none",
                skein_forge::api::Ci::Pending => b"pending",
                skein_forge::api::Ci::Passed => b"passed",
                skein_forge::api::Ci::Failed => b"failed",
            };
            append(&mut out, label);
            append(&mut out, b" at ");
            append(&mut out, &commit_text(head));
        }
        forge::News::Changed { number } => {
            append(&mut out, b"forge item ");
            append_hex(&mut out, &number.to_be_bytes());
            append(&mut out, b" changed");
        }
    }
    out.into_boxed()
}

fn topic_name(topic: &forge::Topic, limits: &forge::Limits) -> Option<forge::Name> {
    let (repository, what) = match topic {
        forge::Topic::Landings { repository, branch } => {
            let mut parts = List::with_capacity(limits.name_bytes);
            let mut start = 0;
            for at in 0..=branch.len() {
                if at == branch.len() || branch.get(at) == Some(&b'/') {
                    let part = branch.get(start..at)?;
                    if part.is_empty() || parts.push(Box::from(part)).is_err() {
                        return None;
                    }
                    start = at.checked_add(1)?;
                }
            }
            (*repository, forge::What::Branch(parts.into_boxed()))
        }
        forge::Topic::Pull { repository, number } => (*repository, forge::What::Pull(*number)),
        forge::Topic::Participation { repository, number } => (*repository, forge::What::Issue(*number)),
        forge::Topic::Ci { repository, .. } => (*repository, forge::What::Repository),
    };
    Some(forge::Name { forge: repository.forge, repository: repository.repository, what })
}

pub(crate) fn watch_names(
    domain: &Domain,
    limits: &Limits,
    subscriber: &forge::Subscriber,
) -> Option<Box<[forge::Name]>> {
    let name = topic_name(&subscriber.topic, &limits.forge)?;
    let mut names = List::with_capacity(limits.forge.resources_per_task);
    if let Some(existing) = domain.forge.names(subscriber.task) {
        for old in existing {
            names.push(old.clone()).ok()?;
            if *old == name {
                return Some(names.into_boxed());
            }
        }
    }
    names.push(name).ok()?;
    Some(names.into_boxed())
}

pub(crate) fn claim_names(
    domain: &Domain,
    limits: &Limits,
    task: u64,
    writes: &[forge::Name],
) -> Option<Box<[forge::Name]>> {
    let mut names = List::with_capacity(limits.forge.resources_per_task);
    if let Some(existing) = domain.forge.names(task) {
        for name in existing {
            names.push(name.clone()).ok()?;
        }
    }
    for write in writes {
        if !names.as_slice().contains(write) {
            names.push(write.clone()).ok()?;
        }
    }
    Some(names.into_boxed())
}

pub(super) fn decimal(number: u64) -> Box<[u8]> {
    let mut digits = List::with_capacity(20);
    let mut value = number;
    for _ in 0_u32..20_u32 {
        digits
            .push(b'0'.checked_add(u8::try_from(value % 10).expect("decimal digit")).expect("decimal ascii digit"))
            .expect("u64 decimal width");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let mut forward = List::with_capacity(20);
    for at in (0..digits.len()).rev() {
        forward.push(*digits.get(at).expect("measured decimal digit")).expect("same width");
    }
    forward.into_boxed()
}

fn read_name(repository: &forge::Repository, read: &skein_forge::api::Read, limit: u32) -> Option<authority::Name> {
    let mut segments = List::with_capacity(limit);
    for segment in [b"forge".as_slice(), &repository.host, &repository.owner, &repository.name] {
        segments.push(Box::from(segment)).ok()?;
    }
    let what = match read {
        skein_forge::api::Read::Pull { number }
        | skein_forge::api::Read::Reviews { number, .. }
        | skein_forge::api::Read::Remarks { number, .. }
        | skein_forge::api::Read::PullFiles { number, .. } => Some((b"pull".as_slice(), Some(decimal(*number)))),
        skein_forge::api::Read::Item { number, .. } => Some((b"issue".as_slice(), Some(decimal(*number)))),
        skein_forge::api::Read::Branch { branch } | skein_forge::api::Read::Protection { branch } => {
            Some((b"branch".as_slice(), Some(branch.clone())))
        }
        skein_forge::api::Read::Locate { .. }
        | skein_forge::api::Read::File { .. }
        | skein_forge::api::Read::Items { .. }
        | skein_forge::api::Read::PullFor { .. }
        | skein_forge::api::Read::Statuses { .. }
        | skein_forge::api::Read::Compare { .. }
        | skein_forge::api::Read::Checks { .. }
        | skein_forge::api::Read::Job { .. }
        | skein_forge::api::Read::Branches { .. }
        | skein_forge::api::Read::Settings
        | skein_forge::api::Read::Collaborators { .. }
        | skein_forge::api::Read::Permission { .. } => None,
    };
    if let Some((kind, part)) = what {
        segments.push(Box::from(kind)).ok()?;
        if let Some(part) = part {
            let mut start = 0;
            for at in 0..=part.len() {
                if at == part.len() || part.get(at) == Some(&b'/') {
                    let segment = part.get(start..at)?;
                    if segment.is_empty() {
                        return None;
                    }
                    segments.push(Box::from(segment)).ok()?;
                    start = at.checked_add(1)?;
                }
            }
        }
    }
    Some(authority::Name { segments: segments.into_boxed() })
}

fn resource_name(repository: &forge::Repository, what: &forge::What, limit: u32) -> Option<authority::Name> {
    let mut segments = List::with_capacity(limit);
    for segment in [b"forge".as_slice(), &repository.host, &repository.owner, &repository.name] {
        segments.push(Box::from(segment)).ok()?;
    }
    match what {
        forge::What::Repository => {}
        forge::What::Issue(number) => {
            segments.push(Box::from(&b"issue"[..])).ok()?;
            segments.push(decimal(*number)).ok()?;
        }
        forge::What::Pull(number) => {
            segments.push(Box::from(&b"pull"[..])).ok()?;
            segments.push(decimal(*number)).ok()?;
        }
        forge::What::Branch(parts) => {
            segments.push(Box::from(&b"branch"[..])).ok()?;
            for part in parts {
                if part.is_empty() || part.contains(&b'/') {
                    return None;
                }
                segments.push(part.clone()).ok()?;
            }
        }
    }
    Some(authority::Name { segments: segments.into_boxed() })
}

fn effect_access(repository: &forge::Repository, what: &forge::What, kind: u16) -> authority::EffectAccess {
    match forge::effect_access(repository, what, kind) {
        forge::Access::Owned => authority::EffectAccess::Owned,
        forge::Access::Participant => authority::EffectAccess::Participant,
        forge::Access::Context => authority::EffectAccess::Context,
        forge::Access::Unavailable => authority::EffectAccess::Unavailable,
    }
}

fn effect_key(domain: &Domain, key: CallKey) -> Box<[u8]> {
    temper_engine_domain_forge::purpose::EffectPurpose::Call {
        task: key.task,
        attempt: key.attempt,
        completion: key.completion,
        position: key.position,
    }
    .key(&domain.core.counters.deployment().id, 44)
    .expect("fixed call key fits its frame")
}

#[expect(clippy::too_many_arguments, reason = "one named call carries the root and its typed forge write")]
pub(crate) fn effect_call(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision,
    to: ReplyTo,
    key: CallKey,
    repository: skein_forge::api::Repository,
    resource: forge::What,
    write: skein_forge::api::Write,
    proposal: Option<(Box<[u8]>, bool)>,
) {
    let owner = effect_owner(domain);
    let flight = match describe_agent(domain, env, key, repository, resource, write) {
        Ok(flight) => flight,
        Err(skein_forge::api::Error::Busy) => {
            super::relay_call(domain, &env.limits, decision, to, CallAnswer::Unavailable);
            return;
        }
        Err(why) => {
            decide_call(domain, &env.limits, decision, to, key, CallAnswer::ForgeEffectRefused(why));
            return;
        }
    };
    assert!(domain.forge_effects.insert(owner, flight).is_ok(), "admitted forge effect handoff");
    let deadline = skein_lib::Wall::from_nanos(
        // A call's timeout is its task's charter's (jig's `domain/engine.md`, 7.1).
        env.wall.as_nanos().saturating_add(domain.core.charter_of(key.task).policy.call_timeout.as_nanos()),
    );
    let origin = match proposal {
        Some((reason, as_holder)) => jig_domain_core::EffectOrigin::Propose { to, key, reason, as_holder },
        None => jig_domain_core::EffectOrigin::Call { to, key, deadline },
    };
    domain.work.push(Work::Core(jig_domain_core::Event::EffectStart {
        owner,
        connector: domain.config.forge_connector,
        origin,
    }));
}

fn describe_agent(
    domain: &Domain,
    env: &Env<Limits>,
    key: CallKey,
    repository: skein_forge::api::Repository,
    resource: forge::What,
    write: skein_forge::api::Write,
) -> Result<EffectFlight, skein_forge::api::Error> {
    let mut staged = 0_u32;
    for (_, flight) in &domain.forge_effects {
        match flight {
            EffectFlight::Call { .. } => staged = staged.saturating_add(1),
            EffectFlight::Procedure { .. } | EffectFlight::Projection { .. } => {}
        }
    }
    if !domain.forge.effect_room(&env.limits.forge, staged) {
        return Err(skein_forge::api::Error::Busy);
    }
    let adopted = domain.forge.repository(repository).ok_or(skein_forge::api::Error::Missing)?;
    if !domain.core.connector_project(key.task, adopted.project) {
        return Err(skein_forge::api::Error::Forbidden);
    }
    let described = match forge::describe_agent_effect(
        adopted,
        &resource,
        write,
        effect_key(domain, key),
        env.limits.forge.client.op_bytes,
    ) {
        Ok(described) => described,
        Err(why) => {
            return Err(why);
        }
    };
    let name =
        resource_name(adopted, &resource, env.limits.authority.segments).ok_or(skein_forge::api::Error::TooLarge)?;
    let form = match described.form {
        forge::EffectForm::Creation => EffectForm::Creation,
        forge::EffectForm::Transition => EffectForm::Transition,
        forge::EffectForm::Set => EffectForm::Set,
    };
    let recovery = match described.recovery {
        forge::Recovery::Keyed => Recovery::Keyed,
        forge::Recovery::Conditional => Recovery::Conditional,
        forge::Recovery::Idempotent => Recovery::Idempotent,
        forge::Recovery::Unrecoverable => Recovery::Unrecoverable,
    };
    let description = EffectDescription {
        waits_for_dependencies: forge::waits_for_dependencies(described.kind),
        connector: domain.config.forge_connector,
        purpose: key.completion.into(),
        effect: authority::Effect {
            connector: domain.config.forge_connector,
            kind: described.kind,
            name,
            state: described.state,
            price: None,
            access: effect_access(adopted, &resource, described.kind),
            additional: Box::new([]),
            guards: Box::new([]),
        },
        form,
        recovery,
    };
    Ok(EffectFlight::Call { description: Box::new(description), repository, effect: described.effect, key, resource })
}

pub(crate) fn read_call(
    domain: &mut Domain,
    env: &Env<Limits>,
    _decision: &mut Decision,
    to: ReplyTo,
    key: CallKey,
    repository: skein_forge::api::Repository,
    read: skein_forge::api::Read,
) {
    let Some(adopted) = domain.forge.repository(repository) else {
        return super::host_route::read(
            domain,
            to,
            CallAnswer::ForgeRead(Box::new(Err(skein_forge::api::Error::Missing))),
        );
    };
    let Some(context) = domain.core.tasks.delegation(key.task) else {
        return super::host_route::read(
            domain,
            to,
            CallAnswer::ForgeRead(Box::new(Err(skein_forge::api::Error::Forbidden))),
        );
    };
    let name = read_name(adopted, &read, env.limits.authority.segments);
    let allowed = match name {
        Some(name) if context.project == adopted.project && adopted.kinds.read => {
            let mut findings =
                Queue::with_capacity(authority::max_out(domain.core.authority.limits()).expect("authority findings"));
            domain.core.connector_read_admit(
                key.task,
                authority::Effect {
                    connector: domain.config.forge_connector,
                    kind: 1,
                    name,
                    state: [0; 32],
                    price: None,
                    access: effect_access(adopted, &forge::What::Repository, 1),
                    additional: Box::new([]),
                    guards: Box::new([]),
                },
                &mut findings,
            ) == authority::Answer::Allow
        }
        Some(_) | None => false,
    };
    if !allowed {
        return super::host_route::read(
            domain,
            to,
            CallAnswer::ForgeRead(Box::new(Err(skein_forge::api::Error::Forbidden))),
        );
    }
    let Some(serial) = domain.next_effect_owner.checked_add(1) else {
        return super::host_route::read(
            domain,
            to,
            CallAnswer::ForgeRead(Box::new(Err(skein_forge::api::Error::Busy))),
        );
    };
    if serial >= 1_u64 << 61_u32 {
        return super::host_route::read(
            domain,
            to,
            CallAnswer::ForgeRead(Box::new(Err(skein_forge::api::Error::Busy))),
        );
    }
    domain.next_effect_owner = serial;
    let owner = Token::new(serial | (1_u64 << 61_u32));
    assert!(domain.forge_reading.insert(owner, (to, key)) == Ok(None), "one fresh read correlation");
    domain.work.push(Work::Forge(forge::Event::Client(skein_forge_client::Event::Read { owner, repository, read })));
}

#[expect(clippy::too_many_arguments, reason = "one typed task and connector subscription is routed together")]
pub(crate) fn subscribe_call(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision,
    to: ReplyTo,
    key: CallKey,
    topic: forge::Topic,
    own_change: Option<u64>,
    paths: Box<[Box<[u8]>]>,
) {
    let Some(number) = crate::fresh(&mut domain.core.counters, Family::Message) else {
        decide_call(
            domain,
            &env.limits,
            decision,
            to,
            key,
            CallAnswer::SubscriptionRefused(tasks::Problem::new(Some(key.task), tasks::Refusal::Busy)),
        );
        return;
    };
    let subscriber = forge::Subscriber { task: key.task, number, topic, own_change, goal_tasks: None, paths };
    if !domain.forge.can_subscribe(&env.limits.forge, &subscriber)
        || watch_names(domain, &env.limits, &subscriber).is_none()
    {
        decide_call(
            domain,
            &env.limits,
            decision,
            to,
            key,
            CallAnswer::SubscriptionRefused(tasks::Problem::new(Some(key.task), tasks::Refusal::Subscription)),
        );
        return;
    }
    let token = to.into_token();
    assert!(domain.core.reserve_connector_subscription(token, key, number), "one routed call");
    assert!(domain.forge_subscribing.insert(token, subscriber) == Ok(None), "one connector interest");
    domain.work.push(Work::Tasks(tasks::Event::SubscribeTopic {
        reply_to: ReplyTo::new(token),
        task: key.task,
        subscription: tasks::Subscription {
            number,
            kind: tasks::SubscriptionKind::Topic { connector: domain.config.forge_connector, topic: number },
        },
    }));
}

fn branch_parts(branch: &[u8], limit: u32) -> Option<Box<[Box<[u8]>]>> {
    let mut parts = List::with_capacity(limit);
    let mut start = 0;
    for at in 0..=branch.len() {
        if at == branch.len() || branch.get(at) == Some(&b'/') {
            let part = branch.get(start..at)?;
            if part.is_empty() {
                return None;
            }
            parts.push(Box::from(part)).ok()?;
            start = at.checked_add(1)?;
        }
    }
    Some(parts.into_boxed())
}

fn branch_what(branch: &[u8], limit: u32) -> Option<forge::What> {
    Some(forge::What::Branch(branch_parts(branch, limit)?))
}

#[expect(clippy::manual_map, reason = "the strict subset does not use closures")]
fn tree_branch(
    repository: &forge::Repository,
    root: u64,
    marker: u8,
    task: u64,
    attempt: Option<u64>,
    limit: u32,
) -> Option<Box<[u8]>> {
    let root_bytes = decimal(root);
    let task_bytes = decimal(task);
    let attempt_bytes = match attempt {
        Some(number) => Some(decimal(number)),
        None => None,
    };
    let mut needed =
        repository.prefix.len().checked_add(root_bytes.len())?.checked_add(2)?.checked_add(task_bytes.len())?;
    if let Some(number) = &attempt_bytes {
        needed = needed.checked_add(1)?.checked_add(number.len())?;
    }
    if needed > usize::try_from(limit).ok()? {
        return None;
    }
    let mut result = List::with_capacity(limit);
    append(&mut result, &repository.prefix);
    append(&mut result, &root_bytes);
    append(&mut result, b"/");
    append(&mut result, &[marker]);
    append(&mut result, &task_bytes);
    if let Some(attempt) = attempt_bytes {
        append(&mut result, b"-");
        append(&mut result, &attempt);
    }
    let branch = result.into_boxed();
    branch_what(&branch, limit)?;
    Some(branch)
}

/// The hub sees an opaque, bounded name for a forge branch. Forge path syntax
/// stays here; the task child only compares these literal bytes.
pub(crate) fn hub_name(connector: u16, name: &forge::Name, limits: &tasks::Limits) -> Option<tasks::Name> {
    let forge::What::Branch(parts) = &name.what else { return None };
    if limits.hold_segments < 3 {
        return None;
    }
    let mut branch = List::with_capacity(limits.hold_bytes);
    for (at, part) in parts.iter().enumerate() {
        if at != 0 {
            branch.push(b'/').ok()?;
        }
        for byte in part.iter().copied() {
            branch.push(byte).ok()?;
        }
    }
    if branch.len().checked_add(6)? > limits.hold_bytes {
        return None;
    }
    Some(tasks::Name {
        connector,
        path: Box::new([
            Box::from(name.forge.to_be_bytes()),
            Box::from(name.repository.to_be_bytes()),
            branch.into_boxed(),
        ]),
    })
}

/// Decode only this connector's branch names when projecting a hub hold.
pub(crate) fn forge_name(connector: u16, resource: &tasks::Name, limit: u32) -> Option<forge::Name> {
    if resource.connector != connector {
        return None;
    }
    let [forge, repository, branch] = resource.path.as_ref() else { return None };
    let [one, two] = forge.as_ref() else { return None };
    let [three, four, five, six] = repository.as_ref() else { return None };
    Some(forge::Name {
        forge: u16::from_be_bytes([*one, *two]),
        repository: u32::from_be_bytes([*three, *four, *five, *six]),
        what: branch_what(branch, limit)?,
    })
}

fn write_holding(connector: u16, name: &forge::Name, limits: &tasks::Limits) -> Option<tasks::Holding> {
    Some(tasks::Holding::Write { resource: hub_name(connector, name, limits)?, kind: 1 })
}

/// Name the forge branches a new task will own before the hub admits its batch.
#[expect(clippy::too_many_arguments, reason = "task identity and specification arrive from separate admitted carriers")]
pub(crate) fn task_holdings(
    domain: &mut Domain,
    env: &Env<Limits>,
    project: u32,
    root: u64,
    number: u64,
    executor: tasks::Executor,
    spec: &tasks::Spec,
    parent: Option<u64>,
) -> Option<Box<[tasks::Holding]>> {
    let child_of_change = match parent {
        Some(task) => domain.forge.change(task).is_some(),
        None => false,
    };
    let eligible = match executor {
        tasks::Executor::Person(_) => false,
        tasks::Executor::Procedure { connector, code } => {
            connector == domain.config.forge_connector && (code == 1 || code == 2)
        }
        tasks::Executor::Agent { .. } => !child_of_change,
    };
    if !eligible {
        return Some(Box::new([]));
    }
    let mut selected = List::with_capacity(env.limits.forge.resources_per_task);
    if let tasks::Executor::Procedure { connector, code: 1 | 2 } = executor
        && connector == domain.config.forge_connector
    {
        let mut repository = None;
        let mut chosen = None;
        for parameter in &spec.parameters {
            match parameter {
                tasks::Parameter::Resource { name: 1, connector, resource }
                    if *connector == domain.config.forge_connector =>
                {
                    repository = domain.forge.repository_tag(project, *resource);
                }
                tasks::Parameter::Bytes { name: 3, value } => chosen = Some(value.clone()),
                tasks::Parameter::Number { .. }
                | tasks::Parameter::Bytes { .. }
                | tasks::Parameter::Resource { .. } => {}
            }
        }
        let repository = repository?;
        let branch = match chosen {
            Some(branch) => branch,
            None => tree_branch(repository, root, b'c', number, None, env.limits.forge.name_bytes)?,
        };
        let name = forge::Name {
            forge: repository.provider.forge,
            repository: repository.provider.repository,
            what: branch_what(&branch, env.limits.forge.name_bytes)?,
        };
        let holding = write_holding(domain.config.forge_connector, &name, &env.limits.tasks)?;
        report_resource(domain, env, project, &name);
        return Some(Box::new([holding]));
    }
    if let Some(home) = domain.forge.home(project) {
        include_repository(&mut selected, home)?;
    }
    for parameter in &spec.parameters {
        if let tasks::Parameter::Resource { connector, resource, .. } = parameter
            && *connector == domain.config.forge_connector
        {
            let repository = domain.forge.repository_tag(project, *resource)?;
            include_repository(&mut selected, repository)?;
        }
    }
    let mut holdings = List::with_capacity(env.limits.tasks.holdings);
    let mut reports = List::with_capacity(env.limits.tasks.holdings);
    for repository in &selected {
        if repository.role == forge::Role::Context || !repository.kinds.push {
            continue;
        }
        let branch = tree_branch(repository, root, b'r', number, None, env.limits.forge.name_bytes)?;
        let name = forge::Name {
            forge: repository.provider.forge,
            repository: repository.provider.repository,
            what: branch_what(&branch, env.limits.forge.name_bytes)?,
        };
        holdings.push(write_holding(domain.config.forge_connector, &name, &env.limits.tasks)?).ok()?;
        reports.push(name).ok()?;
    }
    for name in &reports {
        report_resource(domain, env, project, name);
    }
    Some(holdings.into_boxed())
}

#[derive(Debug)]
pub(crate) struct RunWorkspace {
    pub workspace: ForgeWorkspace,
    /// The task holding the workspace: this one, or the change whose branch it inherits.
    pub workstream: u64,
    pub writes: Box<[authority::Write]>,
    pub names: Box<[forge::Name]>,
    pub own_holds: Box<[forge::Name]>,
}

/// Translate committed tracked-task facts into one bounded issue projection.
#[expect(clippy::disallowed_methods, reason = "projection renders already validated task text")]
pub(crate) fn project_goal(domain: &mut Domain, env: &Env<Limits>, feed: &jig_domain_core::ProjectionFeed) {
    goal_topics(domain, env, feed);
    let goal = &feed.goal;
    let Some(repository) = domain.forge.home(goal.project) else {
        if feed.closing {
            domain.work.push(Work::Core(jig_domain_core::Event::ProjectionSettled {
                goal: goal.number,
                connector: domain.config.forge_connector,
            }));
        }
        return;
    };
    let provider = repository.provider;
    let Some(text) = core::str::from_utf8(&goal.words).ok() else {
        fail_projection_feed(domain, provider, feed);
        return;
    };
    let title = text.lines().next().unwrap_or(text);
    let bound = usize::try_from(env.limits.forge.issue_policy.plan_items).expect("u32 fits usize");
    let children = feed.plan.iter().filter(|child| child.number != goal.number).count();
    let mut plan = List::with_capacity(env.limits.forge.issue_policy.plan_items);
    let individual = if children > bound { bound.saturating_sub(1) } else { children };
    let mut seen = 0usize;
    let mut live = false;
    for child in &feed.plan {
        if child.number == goal.number {
            continue;
        }
        let Some(words) = core::str::from_utf8(&child.words).ok() else {
            fail_projection_feed(domain, provider, feed);
            return;
        };
        if seen < individual {
            plan.push(forge_issues::PlanItem { text: Box::from(words), phase: projection_phase(&child.phase) })
                .expect("bounded prefix");
        } else {
            match child.phase {
                tasks::Phase::Ended(_) => {}
                tasks::Phase::Waiting
                | tasks::Phase::Active(_)
                | tasks::Phase::Closing(_)
                | tasks::Phase::Held { .. } => live = true,
            }
        }
        seen = seen.checked_add(1).expect("bounded plan");
    }
    if children > bound && bound > 0 {
        let count = children.checked_sub(individual).expect("folded children");
        let digits = decimal(u64::try_from(count).expect("bounded plan"));
        let mut words = List::with_capacity(31); // Twenty decimal digits and " more tasks".
        for byte in digits.iter().chain(b" more tasks") {
            words.push(*byte).expect("counted task line");
        }
        let words = words.into_boxed();
        let words = Box::<str>::from(core::str::from_utf8(&words).expect("ASCII summary"));
        plan.push(forge_issues::PlanItem {
            text: words,
            phase: if live { forge_issues::Phase::Active } else { forge_issues::Phase::Done },
        })
        .expect("last summary item");
    }
    let mut milestones = List::with_capacity(u32::try_from(feed.milestones.len()).expect("bounded feed"));
    for milestone in &feed.milestones {
        let kept = match &milestone.phase {
            Some(tasks::Phase::Ended(_) | tasks::Phase::Held { .. }) => true,
            Some(tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_)) | None => false,
        };
        if !kept && milestone.change.is_none() {
            continue;
        }
        let key = match milestone.identity {
            jig_domain_core::MilestoneId::Lifecycle { task, position } => {
                forge_issues::MilestoneKey::Lifecycle { task, position }
            }
            jig_domain_core::MilestoneId::Revision { task, revision } => {
                forge_issues::MilestoneKey::TaskRevision { task, revision }
            }
        };
        let words = core::str::from_utf8(&milestone.words).unwrap_or("Task milestone");
        let words = if words.is_empty() { "Task milestone" } else { words };
        milestones.push(forge_issues::Milestone { key, text: Box::from(words) }).expect("feed capacity");
    }
    let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow) else { return };
    domain.work.push(Work::Forge(forge::Event::Project {
        entry,
        repository: provider,
        view: forge_issues::GoalView {
            phase: goal_phase(&goal.phase),
            goal: goal.number,
            repository: u64::from(provider.repository),
            title: Box::from(title),
            goal_text: Box::from(text),
            plan: plan.into_boxed(),
            milestones: milestones.into_boxed(),
            finished: if feed.closing { Some(Box::from("Goal finished")) } else { None },
        },
    }));
}

fn fail_projection_feed(
    domain: &mut Domain,
    repository: skein_forge::api::Repository,
    feed: &jig_domain_core::ProjectionFeed,
) {
    domain.work.push(Work::Forge(forge::Event::FailProjection {
        goal: feed.goal.number,
        repository,
        closing: feed.closing,
    }));
}

fn outbox_outcome(entry: u64, outcome: skein_forge_client::Outcome) -> jig_domain_core::connector::OutboxOutcome {
    match outcome {
        skein_forge_client::Outcome::Made { .. } => jig_domain_core::connector::OutboxOutcome::Made,
        skein_forge_client::Outcome::Failed(_) | skein_forge_client::Outcome::Raced { .. } => {
            jig_domain_core::connector::OutboxOutcome::Failed
        }
        skein_forge_client::Outcome::Withdrawn => jig_domain_core::connector::OutboxOutcome::Withdrawn,
        skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => {
            jig_domain_core::connector::OutboxOutcome::Uncertain
        }
        skein_forge_client::Outcome::Held => jig_domain_core::connector::OutboxOutcome::Held { entry },
    }
}

fn projection_failed(domain: &mut Domain, goal: u64, closing: bool) {
    if closing {
        domain.forge.projection_settled(goal);
        domain.work.push(Work::Core(jig_domain_core::Event::ProjectionSettled {
            goal,
            connector: domain.config.forge_connector,
        }));
    }
}

fn projection_phase(phase: &tasks::Phase) -> forge_issues::Phase {
    match phase {
        tasks::Phase::Waiting => forge_issues::Phase::Waiting,
        tasks::Phase::Active(_) => forge_issues::Phase::Active,
        tasks::Phase::Held { .. } => forge_issues::Phase::Held,
        tasks::Phase::Closing(_) => forge_issues::Phase::Settling,
        tasks::Phase::Ended(ending) => ending_phase(ending),
    }
}
fn ending_phase(ending: &tasks::Ending) -> forge_issues::Phase {
    match ending {
        tasks::Ending::Done(_) => forge_issues::Phase::Done,
        tasks::Ending::Failed { .. } => forge_issues::Phase::Failed,
        tasks::Ending::Cancelled { .. } => forge_issues::Phase::Cancelled,
    }
}
fn goal_phase(phase: &tasks::Phase) -> forge_issues::Phase {
    match phase {
        tasks::Phase::Closing(closing) | tasks::Phase::Held { was: tasks::Was::Closing(closing), .. } => {
            ending_phase(&closing.ending)
        }
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } | tasks::Phase::Ended(_) => {
            projection_phase(phase)
        }
    }
}

// Translate the plan's forge procedure parameters before those tasks activate.
fn goal_topics(domain: &mut Domain, env: &Env<Limits>, feed: &jig_domain_core::ProjectionFeed) {
    if feed.closing {
        return;
    }
    for child in &feed.plan {
        let Some(task) = domain.core.tasks.task(child.number) else { continue };
        let relevant = match task.executor {
            tasks::Executor::Procedure { connector, code: 2 } => connector == domain.config.forge_connector,
            tasks::Executor::Procedure { .. } | tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => false,
        };
        if !relevant {
            continue;
        }
        let Some(topic) = change_topic(domain, task) else { continue };
        subscribe_goal(domain, env, feed.goal.number, topic);
    }
}

fn change_topic(domain: &Domain, task: &tasks::TaskRecord) -> Option<forge::Topic> {
    let mut provider = None;
    let mut branch = None;
    for parameter in &task.spec.parameters {
        match parameter {
            tasks::Parameter::Resource { name: 1, connector, resource }
                if *connector == domain.config.forge_connector =>
            {
                let repository = domain.forge.repository_tag(task.project, *resource)?;
                provider = Some(repository.provider);
            }
            tasks::Parameter::Bytes { name: 2, value } => branch = Some(value.clone()),
            tasks::Parameter::Resource { .. } | tasks::Parameter::Bytes { .. } | tasks::Parameter::Number { .. } => {}
        }
    }
    let repository = provider?;
    let adopted = domain.forge.repository(repository)?;
    Some(forge::Topic::Landings { repository, branch: branch.unwrap_or(adopted.settings.default_branch.clone()) })
}

pub(crate) fn subscribe_goal(domain: &mut Domain, env: &Env<Limits>, goal: u64, topic: forge::Topic) {
    if domain.core.tasks.task(goal).is_none() {
        return;
    }
    let mut previous = domain.forge.topic_subscriber(goal, &topic).cloned();
    for work in &domain.work {
        match work {
            Work::GoalSubscribe(subscriber) if subscriber.task == goal && subscriber.topic == topic => {
                previous = Some(subscriber.clone());
            }
            Work::GoalSubscribe(_)
            | Work::ForgeRestored
            | Work::SwitchChecked
            | Work::AdoptRestored
            | Work::AdoptDone
            | Work::Restart(_)
            | Work::Decoded { .. }
            | Work::Core(_)
            | Work::Tasks(_)
            | Work::People(_)
            | Work::Fleet(_)
            | Work::Brief(_)
            | Work::StartBrief { .. }
            | Work::Forge(_)
            | Work::ProjectGoal(_)
            | Work::Activate(_)
            | Work::EscalationLoaded { .. }
            | Work::EscalationFailed { .. }
            | Work::ProposalLoaded { .. }
            | Work::ProposalFailed { .. }
            | Work::EndedLoaded { .. }
            | Work::TaskProposalLoaded { .. }
            | Work::EndedFailed { .. } => {}
        }
    }
    let Some(members) = goal_members(domain, env, goal, &topic, previous.as_ref()) else { return };
    let number = match &previous {
        Some(previous) => previous.number,
        None => match crate::fresh(&mut domain.core.counters, Family::Message) {
            Some(number) => number,
            None => return,
        },
    };
    let subscriber = forge::Subscriber {
        task: goal,
        number,
        topic,
        own_change: None,
        goal_tasks: Some(members.tasks),
        paths: members.paths,
    };
    if previous.as_ref() == Some(&subscriber) {
        return;
    }
    if !domain.forge.can_subscribe(&env.limits.forge, &subscriber)
        || watch_names(domain, &env.limits, &subscriber).is_none()
    {
        return;
    }
    if previous.is_none() {
        domain.work.push(Work::Tasks(tasks::Event::SubscribeTopic {
            reply_to: super::internal(u64::MAX),
            task: goal,
            subscription: tasks::Subscription {
                number,
                kind: tasks::SubscriptionKind::Topic { connector: domain.config.forge_connector, topic: number },
            },
        }));
    }
    domain.work.push(Work::GoalSubscribe(subscriber));
}

struct GoalMembers {
    tasks: Box<[u64]>,
    paths: Box<[Box<[u8]>]>,
}

/// Whether `task` is in `goal`'s subtree below it: its requesters, task by
/// task, reach `goal` (forge.md, 7.2).
fn in_subtree(domain: &Domain, task: &tasks::TaskRecord, goal: u64) -> bool {
    let mut current = task;
    for _ in 0..task.depth {
        let tasks::Party::Task(number) = current.requester else { return false };
        if number == goal {
            return true;
        }
        let Some(parent) = domain.core.tasks.task(number) else { return false };
        current = parent;
    }
    false
}

fn goal_members(
    domain: &Domain,
    env: &Env<Limits>,
    goal: u64,
    topic: &forge::Topic,
    previous: Option<&forge::Subscriber>,
) -> Option<GoalMembers> {
    let mut goal_tasks = List::with_capacity(env.limits.forge.issue_policy.plan_items);
    let mut paths = List::with_capacity(env.limits.forge.paths_per_subscription);
    let mut hints_complete = true;
    if let Some(previous) = previous
        && let Some(tasks) = &previous.goal_tasks
    {
        for task in tasks {
            if goal_tasks.push(*task).is_err() {
                return None;
            }
        }
    }
    for view in domain.core.tasks.view_tasks() {
        let Some(task) = domain.core.tasks.task(view.number) else { continue };
        if !in_subtree(domain, task, goal) {
            continue;
        }
        match task.executor {
            tasks::Executor::Procedure { connector, code: 2 } if connector == domain.config.forge_connector => {}
            tasks::Executor::Procedure { .. } | tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => continue,
        }
        let Some(change_topic) = change_topic(domain, task) else { continue };
        match topic {
            forge::Topic::Landings { .. } if *topic != change_topic => continue,
            forge::Topic::Ci { repository, .. } => match change_topic {
                forge::Topic::Landings { repository: home, .. } if home == *repository => {}
                forge::Topic::Landings { .. }
                | forge::Topic::Ci { .. }
                | forge::Topic::Pull { .. }
                | forge::Topic::Participation { .. } => continue,
            },
            forge::Topic::Landings { .. } | forge::Topic::Pull { .. } | forge::Topic::Participation { .. } => {}
        }
        let mut hinted = false;
        let mut present = false;
        for kept in &goal_tasks {
            if *kept == task.number {
                present = true;
            }
        }
        if !present && goal_tasks.push(task.number).is_err() {
            return None;
        }
        for parameter in &task.spec.parameters {
            match parameter {
                tasks::Parameter::Bytes { name: 6, value } => {
                    hinted = true;
                    if paths.push(value.clone()).is_err() {
                        return None;
                    }
                }
                tasks::Parameter::Bytes { .. }
                | tasks::Parameter::Number { .. }
                | tasks::Parameter::Resource { .. } => {}
            }
        }
        let produced = match domain.forge.change(task.number) {
            Some(change) => change.pull.is_some(),
            None => false,
        };
        if !produced && !hinted {
            hints_complete = false;
        }
    }
    if !hints_complete {
        paths = List::with_capacity(env.limits.forge.paths_per_subscription);
    }
    Some(GoalMembers { tasks: goal_tasks.into_boxed(), paths: paths.into_boxed() })
}

pub(crate) fn goal_subscribed(domain: &mut Domain, env: &Env<Limits>, subscriber: forge::Subscriber) {
    let Some(task) = domain.core.tasks.task(subscriber.task) else { return };
    let mut present = false;
    for subscription in &task.subscriptions {
        if subscription.number == subscriber.number {
            present = true;
            break;
        }
    }
    if !present {
        return;
    }
    let Some(names) = watch_names(domain, &env.limits, &subscriber) else { return };
    domain.work.push(Work::Forge(forge::Event::Subscribe { subscription: subscriber }));
    domain.work.push(Work::Forge(forge::Event::Names { task: task.number, resources: names }));
}

fn may_push(
    domain: &Domain,
    env: &Env<Limits>,
    context: &tasks::RunContext,
    repository: &forge::Repository,
    branch: &[u8],
) -> bool {
    let Some(what) = branch_what(branch, env.limits.forge.name_bytes) else { return false };
    let Some(name) = resource_name(repository, &what, env.limits.authority.segments) else { return false };
    let Some(bound) = authority::max_out(domain.core.authority.limits()) else { return false };
    let mut findings = Queue::with_capacity(bound);
    domain.core.connector_run_write_admit(
        context,
        authority::Effect {
            connector: domain.config.forge_connector,
            kind: 2,
            name,
            state: [0; 32],
            price: None,
            access: effect_access(repository, &what, 2),
            additional: Box::new([]),
            guards: Box::new([]),
        },
        env.wall,
        &mut findings,
    ) == authority::Answer::Allow
}

fn tracked_ancestor(domain: &Domain, requester: tasks::Party) -> Option<u64> {
    let mut next = requester;
    for _ in 0..=domain.limits.tasks.depth {
        let task = match next {
            tasks::Party::Task(task) => task,
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => return None,
        };
        let row = domain.core.tasks.task(task)?;
        if row.tracked.is_some() {
            return Some(task);
        }
        next = row.requester;
    }
    None
}

fn include_repository<'a>(selected: &mut List<&'a forge::Repository>, repository: &'a forge::Repository) -> Option<()> {
    for prior in selected.as_slice() {
        if prior.provider == repository.provider {
            return Some(());
        }
    }
    selected.push(repository).ok()
}

/// An older task can read a newly adopted repository without acquiring its run branch.
pub(crate) fn unheld_run_branch(
    domain: &Domain,
    limits: &Limits,
    task: u64,
    repository: &forge::Repository,
) -> Option<bool> {
    let root = domain.core.tasks.root(task)?;
    let branch = tree_branch(repository, root, b'r', task, None, limits.forge.name_bytes)?;
    let name = forge::Name {
        forge: repository.provider.forge,
        repository: repository.provider.repository,
        what: branch_what(&branch, limits.forge.name_bytes)?,
    };
    Some(domain.core.tasks.holder(&hub_name(domain.config.forge_connector, &name, &limits.tasks)?).is_none())
}

/// Resolve a run's repository tags and change ancestry before asking policy
/// or taking the connector writer slot.
#[expect(clippy::too_many_lines, reason = "one claim translates and checks all repository writes and holds")]
pub(crate) fn run_workspace(
    domain: &Domain,
    env: &Env<Limits>,
    context: &tasks::RunContext,
    _attempt: u64,
) -> Option<RunWorkspace> {
    let root = domain.core.tasks.root(context.task)?;
    let parent = match context.requester {
        tasks::Party::Task(task) => Some(task),
        tasks::Party::Person(_) | tasks::Party::Deployment { .. } => None,
    };
    let change = match parent {
        Some(task) => domain.forge.change(task),
        None => None,
    };
    let inherited = match change {
        Some(row) => match row.delegate {
            Some((child, kind)) if child == context.task => Some((row, kind)),
            Some(_) | None => None,
        },
        None => None,
    };
    let adopted = match inherited {
        Some((row, _)) => domain.forge.repository(row.repository),
        None => domain.forge.home(context.project),
    };
    let mut selected = List::with_capacity(env.limits.forge.resources_per_task);
    if let Some(repository) = adopted {
        include_repository(&mut selected, repository)?;
    }
    let saved = saved_tags(&context.saved, domain.config.forge_connector)?;
    if inherited.is_none() {
        for parameter in &context.spec.parameters {
            match parameter {
                tasks::Parameter::Resource { connector, resource, .. }
                    if *connector == domain.config.forge_connector =>
                {
                    let repository = domain.forge.repository_tag(context.project, *resource)?;
                    if repository.project != context.project {
                        return None;
                    }
                    include_repository(&mut selected, repository)?;
                }
                tasks::Parameter::Resource { .. }
                | tasks::Parameter::Number { .. }
                | tasks::Parameter::Bytes { .. } => {}
            }
        }
        for tag in &saved {
            let Some(repository) = domain.forge.repository_tag(context.project, *tag) else {
                if adopted.is_some() {
                    return None;
                }
                continue;
            };
            include_repository(&mut selected, repository)?;
        }
    }
    let context_repositories =
        domain.forge.context_repositories(context.project, env.limits.forge.resources_per_task)?;
    for provider in &context_repositories {
        let repository = domain.forge.repository(*provider)?;
        include_repository(&mut selected, repository)?;
    }
    let mut workspace_bytes = 0_u64;
    let mut repositories = List::with_capacity(env.limits.forge.resources_per_task);
    let mut writes = List::with_capacity(env.limits.authority.writes);
    let mut names = List::with_capacity(env.limits.forge.resources_per_task);
    let mut own_holds = List::with_capacity(env.limits.forge.resources_per_task);
    let mut key = context.task;
    for repository in &selected {
        let saved = saved.contains(&forge::tag(repository.provider.forge, repository.provider.repository));
        let run_branch = tree_branch(repository, root, b'r', context.task, None, env.limits.forge.name_bytes)?;
        let saved_branch = if saved {
            Some(tree_branch(repository, root, b's', context.task, None, env.limits.forge.name_bytes)?)
        } else {
            None
        };
        let item = domain.forge.workspace_item(forge::items::Seed {
            task: context.task,
            parent,
            repository: repository.provider,
            run_branch,
            saved_branch,
        })?;
        let start = match item.start {
            forge::items::Start::Base(branch) => ForgeStart::Base(branch),
            forge::items::Start::Branch(branch) => ForgeStart::Branch(branch),
            forge::items::Start::Saved(branch) => ForgeStart::Saved(branch),
            forge::items::Start::Merge { branch, base } => ForgeStart::Merge { branch, base },
        };
        let mut push = item.push;
        let holder = item.holder;
        if let Some((row, _)) = inherited {
            key = row.task;
        }
        workspace_bytes = workspace_bytes.checked_add(item.bytes)?;
        if workspace_bytes > u64::from(env.limits.journal.run_bytes) {
            return None;
        }
        if inherited.is_none() {
            let permitted = match &push {
                Some(branch) => may_push(domain, env, context, repository, branch),
                None => false,
            };
            if !permitted {
                push = None;
            }
        }
        if let Some(branch) = &push {
            let name = forge::Name {
                forge: repository.provider.forge,
                repository: repository.provider.repository,
                what: branch_what(branch, env.limits.forge.name_bytes)?,
            };
            let held = match holder {
                Some(owner) => {
                    if domain.core.tasks.holder(&hub_name(domain.config.forge_connector, &name, &env.limits.tasks)?)
                        != Some(owner)
                    {
                        return None;
                    }
                    Some(authority::Writer::Ancestor)
                }
                None if unheld_run_branch(domain, &env.limits, context.task, repository)? => {
                    push = None;
                    None
                }
                None => {
                    if domain.core.tasks.holder(&hub_name(domain.config.forge_connector, &name, &env.limits.tasks)?)
                        != Some(context.task)
                    {
                        return None;
                    }
                    if let Some(existing) = domain.forge.hold(&name) {
                        if existing.task != context.task {
                            return None;
                        }
                    } else {
                        own_holds.push(name.clone()).ok()?;
                    }
                    Some(authority::Writer::Task)
                }
            };
            if let Some(held) = held {
                writes
                    .push(authority::Write {
                        effect: authority::Effect {
                            connector: domain.config.forge_connector,
                            kind: 2,
                            name: resource_name(repository, &name.what, env.limits.authority.segments)?,
                            state: [0; 32],
                            price: None,
                            access: effect_access(repository, &name.what, 2),
                            additional: Box::new([]),
                            guards: Box::new([]),
                        },
                        held,
                    })
                    .ok()?;
                names.push(name).ok()?;
            }
        }
        let provider = item.provider;
        repositories
            .push(ForgeRepository {
                tag: forge::tag(provider.forge, provider.repository),
                provider,
                name: item.name,
                host: item.host,
                owner: item.owner,
                start,
                landing: if push.is_some() { item.landing } else { None },
                push,
                // The repository's git identity, from configuration (D10).
                identity: item.credential?,
            })
            .ok()?;
    }
    Some(RunWorkspace {
        workspace: ForgeWorkspace { repositories: repositories.into_boxed() },
        workstream: key,
        writes: writes.into_boxed(),
        names: names.into_boxed(),
        own_holds: own_holds.into_boxed(),
    })
}

/// Reconstruct the connector's provisional writer projection from the
/// assignment only after the hub has durably admitted its writer slots.
#[expect(clippy::type_complexity, reason = "the connector claim carries paired bounded names and ancestor holders")]
pub(crate) fn claimed_writes(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
) -> Option<(Box<[forge::Name]>, Box<[u64]>)> {
    let assignment = domain.assignments.get(&task)?;
    let mut writes = List::with_capacity(env.limits.forge.resources_per_task);
    let mut holders = List::with_capacity(env.limits.forge.resources_per_task);
    for repository in &assignment.workspace.repositories {
        let Some(push) = &repository.push else { continue };
        let name = forge::Name {
            forge: repository.provider.forge,
            repository: repository.provider.repository,
            what: branch_what(push, env.limits.forge.name_bytes)?,
        };
        let holder = domain.core.tasks.holder(&hub_name(domain.config.forge_connector, &name, &env.limits.tasks)?)?;
        if holder != task {
            holders.push(holder).ok()?;
        }
        writes.push(name).ok()?;
    }
    Some((writes.into_boxed(), holders.into_boxed()))
}

#[expect(clippy::manual_map, reason = "the strict subset uses an explicit option match")]
fn pull_what(number: Option<u64>) -> Option<forge::What> {
    match number {
        Some(number) => Some(forge::What::Pull(number)),
        None => None,
    }
}

/// Project landing gates are procedure gates before they become effect requirements.
fn gate_required(domain: &Domain, project: u32, name: &authority::Name, parameters: u32, project_scope: bool) -> bool {
    domain.core.connector_gate_required(project, domain.config.forge_connector, name, parameters, project_scope)
}

fn configured_gates(
    domain: &Domain,
    env: &Env<Limits>,
    repository: &forge::Repository,
    base: &[u8],
) -> Option<Box<[forge_change::Gate]>> {
    let what = branch_what(base, env.limits.forge.name_bytes)?;
    let name = resource_name(repository, &what, env.limits.authority.segments)?;
    let candidates = domain.forge.gate_candidates(&env.limits.forge, repository.project, &name.segments)?;
    let mut required = List::with_capacity(u32::try_from(candidates.len()).ok()?);
    for candidate in &candidates {
        required
            .push(gate_required(domain, repository.project, &name, candidate.parameters, candidate.project_scope))
            .ok()?;
    }
    domain.forge.configured_gates(&env.limits.forge, repository, &candidates, required.as_slice())
}

fn landing_reviewers(
    domain: &Domain,
    env: &Env<Limits>,
    row: &forge::ChangeRow,
    evidence: &forge::ChangeEvidence,
) -> Option<Box<[forge::Reviewer]>> {
    let mut reviews = List::with_capacity(env.limits.forge.client.inbox);
    for review in &evidence.reviews {
        let mut superseded = false;
        for other in &evidence.reviews {
            if other.author == review.author
                && (other.at > review.at || (other.at == review.at && other.id > review.id))
            {
                superseded = true;
            }
        }
        if superseded {
            continue;
        }
        let Some((person, role)) = domain.core.people.role_for_identity(
            people::IdentityKey { provider: 0, subject: review.author.to_be_bytes().into() },
            domain.forge.repository(row.repository)?.project,
        ) else {
            continue;
        };
        reviews
            .push(forge::Reviewer {
                person,
                role: escalation::role_number(role),
                head: review.commit,
                verdict: review.verdict,
            })
            .ok()?;
    }
    Some(reviews.into_boxed())
}

fn describe_change_effect(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    effect: forge_change::Effect,
    evidence: &forge::ChangeEvidence,
) -> Option<EffectDescription> {
    let row = domain.forge.change(task)?;
    let repository = domain.forge.repository(row.repository)?;
    let context = domain.core.tasks.delegation(task)?;
    if repository.project != context.project || repository.role == forge::Role::Context {
        return None;
    }
    let pull_what = pull_what(row.pull);
    let (kind, what, state) = match effect {
        forge_change::Effect::Open => {
            (3, branch_what(&row.branch, env.limits.forge.name_bytes), evidence.head.unwrap_or([0; 32]))
        }
        forge_change::Effect::CreateBranch { head } => (9, branch_what(&row.branch, env.limits.forge.name_bytes), head),
        forge_change::Effect::Reopen => (12, pull_what, evidence.head.unwrap_or([0; 32])),
        forge_change::Effect::Retarget => (10, pull_what, evidence.head.unwrap_or([0; 32])),
        forge_change::Effect::Update { head, .. } => (11, branch_what(&row.branch, env.limits.forge.name_bytes), head),
        forge_change::Effect::Merge { head, .. } => (4, branch_what(&row.base, env.limits.forge.name_bytes), head),
    };
    let what = what?;
    let name = resource_name(repository, &what, env.limits.authority.segments)?;
    let effect = authority::Effect {
        connector: domain.config.forge_connector,
        kind,
        name,
        state,
        price: None,
        access: effect_access(repository, &what, kind),
        additional: Box::new([]),
        guards: Box::new([]),
    };
    let (forge_form, forge_recovery) = forge::effect_shape(kind)?;
    let form = match forge_form {
        forge::EffectForm::Creation => EffectForm::Creation,
        forge::EffectForm::Transition => EffectForm::Transition,
        forge::EffectForm::Set => EffectForm::Set,
    };
    let recovery = match forge_recovery {
        forge::Recovery::Keyed => Recovery::Keyed,
        forge::Recovery::Conditional => Recovery::Conditional,
        forge::Recovery::Idempotent => Recovery::Idempotent,
        forge::Recovery::Unrecoverable => Recovery::Unrecoverable,
    };
    Some(EffectDescription {
        waits_for_dependencies: forge::waits_for_dependencies(kind),
        connector: domain.config.forge_connector,
        purpose: u64::from(kind),
        effect,
        form,
        recovery,
    })
}

/// Once a pull is known its keep supplies the base and CI, within the same two names.
/// Only a task still named is renamed: its release's unnamed ends the watch.
fn watch_change_pull(domain: &mut Domain, env: &Env<Limits>, row: &forge::ChangeRow) {
    let Some(number) = row.pull else { return };
    let Some(named) = domain.forge.names(row.task) else { return };
    let Some(parts) = branch_parts(&row.branch, env.limits.forge.name_bytes) else { return };
    let resources = Box::new([
        forge::Name {
            forge: row.repository.forge,
            repository: row.repository.repository,
            what: forge::What::Branch(parts),
        },
        forge::Name {
            forge: row.repository.forge,
            repository: row.repository.repository,
            what: forge::What::Pull(number),
        },
    ]);
    if named != resources.as_slice() {
        domain.work.push(Work::Forge(forge::Event::Rename { task: row.task, resources }));
    }
}

/// Register the connector's procedure beside the task's first due step.
/// Parameter 1 names a project-unique repository tag, 2 the base,
/// 3 an optional already-pushed branch, 4 the pull body, and 5 priority.
#[expect(clippy::too_many_lines, reason = "one procedure registration and fresh-step route")]
pub(crate) fn start_change(domain: &mut Domain, env: &Env<Limits>, context: &tasks::RunContext) -> bool {
    let task = context.task;
    let mut provider = None;
    let mut base = None;
    let mut branch = None;
    let mut body = None;
    let mut priority = 0_i32;
    let mut base_repair = false;
    for parameter in &context.spec.parameters {
        match parameter {
            tasks::Parameter::Resource { name: 1, connector, resource } => {
                if *connector != domain.config.forge_connector {
                    return false;
                }
                let Some(repository) = domain.forge.repository_tag(context.project, *resource) else { return false };
                provider = Some(repository.provider);
            }
            tasks::Parameter::Bytes { name: 2, value } => base = Some(value.clone()),
            tasks::Parameter::Bytes { name: 3, value } => branch = Some(value.clone()),
            tasks::Parameter::Bytes { name: 4, value } => body = Some(value.clone()),
            tasks::Parameter::Number { name: 5, value } => {
                let Ok(number) = i32::try_from(*value) else { return false };
                priority = number;
            }
            tasks::Parameter::Number { name: 9, value: 1 } => base_repair = true,
            tasks::Parameter::Bytes { name: 6, .. } => {}
            tasks::Parameter::Number { .. } | tasks::Parameter::Bytes { .. } | tasks::Parameter::Resource { .. } => {
                return false;
            }
        }
    }
    let Some(provider) = provider else { return false };
    if base_repair {
        match context.requester {
            tasks::Party::Deployment { .. } => {}
            tasks::Party::Task(_) | tasks::Party::Person(_) => return false,
        }
    }
    let Some(adopted) = domain.forge.repository(provider) else { return false };
    if adopted.project != context.project || adopted.role == forge::Role::Context || !adopted.kinds.open {
        return false;
    }
    let base = match base {
        Some(base) => base,
        None => adopted.settings.default_branch.clone(),
    };
    let branch = match branch {
        Some(branch) => branch,
        None => {
            let Some(root) = domain.core.tasks.root(task) else { return false };
            let Some(branch) = tree_branch(adopted, root, b'c', task, None, env.limits.forge.name_bytes) else {
                return false;
            };
            branch
        }
    };
    let Some(parts) = branch_parts(&branch, env.limits.forge.name_bytes) else { return false };
    if base.is_empty() || branch.is_empty() || context.spec.words.is_empty() {
        return false;
    }
    let Some(base_parts) = branch_parts(&base, env.limits.forge.name_bytes) else { return false };
    let Some(gates) = configured_gates(domain, env, adopted, &base) else { return false };
    if domain.forge.change(task).is_none() {
        domain.work.push(Work::Forge(forge::Event::Change {
            row: forge::ChangeRow {
                task,
                repository: provider,
                branch: branch.clone(),
                base: base.clone(),
                title: context.spec.words.clone(),
                body: match body {
                    Some(body) => body,
                    None => context.spec.words.clone(),
                },
                priority,
                ci: None,
                change: forge_change::Change {
                    task,
                    state: forge_change::State::Producing { requested: false },
                    gates,
                    clean: Box::new([]),
                    gate_failures: Box::new([]),
                    released: None,
                    repairs: 0,
                    resolutions: 0,
                    updates: 0,
                    since: env.wall,
                    ready_since: None,
                    owns_turn: false,
                    last_head: None,
                },
                pull: None,
                pending: None,
                effect: forge_change::EffectResult::None,
                delegate: None,
                delegate_status: forge_change::Status::Unknown,
                verdicts: Box::new([]),
                result: forge::ChangeResult::empty(),
                gate_remarks: Box::new([]),
                drift: None,
                base_repair,
                queue_repair: None,
                queue_repairs: 0,
                producing_charter: domain.config.forge_charter,
                review_charter: 4,
            },
        }));
        let branch_name =
            forge::Name { forge: provider.forge, repository: provider.repository, what: forge::What::Branch(parts) };
        let base_name = forge::Name {
            forge: provider.forge,
            repository: provider.repository,
            what: forge::What::Branch(base_parts),
        };
        domain
            .work
            .push(Work::Forge(forge::Event::Names { task, resources: Box::new([branch_name.clone(), base_name]) }));
        domain.work.push(Work::Forge(forge::Event::Hold { task, resource: branch_name, from: None }));
        if let Some(goal) = tracked_ancestor(domain, context.requester) {
            subscribe_goal(domain, env, goal, forge::Topic::Landings { repository: provider, branch: base.clone() });
        }
    } else if let Some((child, _)) = domain.forge.change(task).expect("registered change").delegate {
        for word in &context.inbox {
            if word.from != tasks::Party::Task(child) {
                continue;
            }
            let status = match word.kind {
                tasks::MessageKind::Result(
                    tasks::ResultKind::Report
                    | tasks::ResultKind::Change { .. }
                    | tasks::ResultKind::Verdict { code: 1, .. },
                ) => Some((forge_change::Status::Passed, false, false)),
                tasks::MessageKind::Result(tasks::ResultKind::Failed) => {
                    Some((forge_change::Status::Failed, true, false))
                }
                tasks::MessageKind::Result(tasks::ResultKind::Cancelled) => {
                    Some((forge_change::Status::Failed, false, true))
                }
                tasks::MessageKind::Result(tasks::ResultKind::Verdict { .. } | tasks::ResultKind::Approved { .. }) => {
                    Some((forge_change::Status::Failed, false, false))
                }
                tasks::MessageKind::Escalation { .. }
                | tasks::MessageKind::Proposal { .. }
                | tasks::MessageKind::SettingsDecision { .. }
                | tasks::MessageKind::ProposalDecision { .. }
                | tasks::MessageKind::Words
                | tasks::MessageKind::FinishRequested { .. }
                | tasks::MessageKind::Amendment { .. }
                | tasks::MessageKind::Question
                | tasks::MessageKind::Answer { .. }
                | tasks::MessageKind::Met { .. }
                | tasks::MessageKind::Notice { .. }
                | tasks::MessageKind::Timer { .. }
                | tasks::MessageKind::PersonStalled { .. }
                | tasks::MessageKind::EffectNotMade { .. }
                | tasks::MessageKind::News { .. } => None,
            };
            if let Some((status, failed, cancelled)) = status {
                domain.work.push(Work::Forge(forge::Event::DelegateResult {
                    task,
                    child,
                    status,
                    failed,
                    cancelled,
                    words: word.words.clone(),
                }));
                break;
            }
        }
    }
    let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow) else { return false };
    domain.work.push(Work::Forge(forge::Event::StepChange {
        task,
        entry,
        kept: domain.core.tasks.waiting_effect_problem(task).is_some(),
        heard: forge_change::Heard {
            lacking: change_authority_lack(domain, task),
            spend: change_spend(domain, task),
            delegate: forge_change::Status::Unknown,
            effect: forge_change::EffectResult::None,
            // A held procedure can only run again once jig has lifted its hold.
            // The forge consumes this before starting any asynchronous reads.
            released: match (domain.forge.change(task), domain.core.tasks.task(task)) {
                (Some(change), Some(task)) => {
                    let held = match &change.change.state {
                        forge_change::State::Held { .. } => true,
                        forge_change::State::Producing { .. }
                        | forge_change::State::Opening { .. }
                        | forge_change::State::Recreating { .. }
                        | forge_change::State::Reopening { .. }
                        | forge_change::State::Checking { .. }
                        | forge_change::State::Gating { .. }
                        | forge_change::State::CancellingGates { .. }
                        | forge_change::State::Queued { .. }
                        | forge_change::State::First { .. }
                        | forge_change::State::Updating { .. }
                        | forge_change::State::Resolving { .. }
                        | forge_change::State::Repairing { .. }
                        | forge_change::State::Landing { .. }
                        | forge_change::State::Landed { .. }
                        | forge_change::State::Ended => false,
                    };
                    held && task.due_reason == tasks::Why::Released
                }
                _ => false,
            },
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: match domain.forge.queue_repair(provider, &base) {
            Some((_, repair)) => domain.core.tasks.task(repair).is_some(),
            None => false,
        },
    }));
    true
}

/// The code-2 procedure's repository and landing parameters, shared by chat
/// changes and deployment queue repairs (domain/agent.md, 4.3.1).
fn change_parameters(connector: u16, provider: skein_forge::api::Repository, base: Box<[u8]>) -> [tasks::Parameter; 2] {
    [
        tasks::Parameter::Resource { name: 1, connector, resource: forge::tag(provider.forge, provider.repository) },
        tasks::Parameter::Bytes { name: 2, value: base },
    ]
}

fn repository_name(repository: &forge::Repository) -> Box<[u8]> {
    let count = repository
        .owner
        .len()
        .checked_add(repository.name.len())
        .expect("bounded repository name")
        .checked_add(1)
        .expect("repository slash");
    let count = u32::try_from(count).expect("bounded repository name");
    let mut name = List::with_capacity(count);
    append(&mut name, &repository.owner);
    append(&mut name, b"/");
    append(&mut name, &repository.name);
    name.into_boxed()
}

/// Bounded display names, attached to an unsettled change flight only.
pub(crate) fn change_repositories(domain: &Domain, project: u32) -> Box<[Box<[u8]>]> {
    let repositories = domain.forge.writable_repositories(project);
    let mut names = List::with_capacity(u32::try_from(repositories.len()).expect("bounded repositories"));
    for repository in &repositories {
        names.push(repository_name(repository)).expect("bounded repositories");
    }
    names.into_boxed()
}

/// Resolve a run's repository name before checking authority on the read's resource.
pub(crate) fn named_repository(
    domain: &Domain,
    project: u32,
    named: Option<&[u8]>,
) -> Result<forge::Repository, tasks::Refusal> {
    let repositories = domain.forge.readable_repositories(project);
    let mut chosen = None;
    for repository in &repositories {
        let fits = match named {
            Some(named) => repository_name(repository).as_ref() == named,
            None => repositories.len() == 1,
        };
        if fits {
            if chosen.is_some() {
                return Err(tasks::Refusal::Unknown);
            }
            chosen = Some(*repository);
        }
    }
    match chosen {
        Some(repository) => Ok(repository.clone()),
        None => Err(tasks::Refusal::Unknown),
    }
}

pub(crate) fn read_repositories(domain: &Domain, project: u32) -> Box<[Box<[u8]>]> {
    let repositories = domain.forge.readable_repositories(project);
    let mut names = List::with_capacity(u32::try_from(repositories.len()).expect("bounded repositories"));
    for repository in &repositories {
        names.push(repository_name(repository)).expect("bounded repositories");
    }
    names.into_boxed()
}

/// Intersect the run's grants with the resources the change reads and writes.
fn change_grants(
    domain: &Domain,
    env: &Env<Limits>,
    repository: &forge::Repository,
    held_grants: &[tasks::Grant],
) -> Result<Box<[tasks::Grant]>, tasks::Refusal> {
    let name = resource_name(repository, &forge::What::Repository, env.limits.authority.segments)
        .ok_or(tasks::Refusal::AuthorityShape)?;
    let exact = tasks::Pattern {
        segments: Box::from(name.segments.get(..3).expect("repository prefix")),
        last: tasks::Last::Exact(repository.name.clone()),
    };
    let descendants = authority::Pattern { segments: name.segments.clone(), last: authority::Last::Open(Box::new([])) };
    let mut grants = List::with_capacity(env.limits.tasks.authority_grants);
    for grant in held_grants {
        if grant.connector != domain.config.forge_connector
            || !(forge::CHANGE_KINDS.contains(&grant.kind) || forge::READ_KINDS.contains(&grant.kind))
        {
            continue;
        }
        // Exact repository and descendant patterns keep sibling repositories out,
        // even when the chat holds an open forge or repository-name prefix.
        let held = authority::Pattern {
            segments: grant.pattern.segments.clone(),
            last: match &grant.pattern.last {
                tasks::Last::Exact(bytes) => authority::Last::Exact(bytes.clone()),
                tasks::Last::Open(bytes) => authority::Last::Open(bytes.clone()),
            },
        };
        // Change writes name branches or pulls; only reads also name the repository itself.
        if forge::READ_KINDS.contains(&grant.kind)
            && authority::pattern_covers(&held, &name)
            && grants.push(tasks::Grant { pattern: exact.clone(), ..grant.clone() }).is_err()
        {
            return Err(tasks::Refusal::AuthorityShape);
        }
        let pattern = if authority::pattern_at_most(&held, &descendants) {
            Some(grant.pattern.clone())
        } else if authority::pattern_at_most(&descendants, &held) {
            Some(tasks::Pattern { segments: name.segments.clone(), last: tasks::Last::Open(Box::new([])) })
        } else {
            None
        };
        if let Some(pattern) = pattern
            && grants.push(tasks::Grant { pattern, ..grant.clone() }).is_err()
        {
            return Err(tasks::Refusal::AuthorityShape);
        }
    }
    Ok(grants.into_boxed())
}

/// Expand a chat's request; ordinary delegation shape and authority admission
/// still decide the resulting batch by its original call name.
pub(crate) fn chat_change(
    domain: &Domain,
    env: &Env<Limits>,
    context: &tasks::DelegationContext,
    words: Box<[u8]>,
    named: Option<&[u8]>,
) -> Result<super::Delegate, tasks::Refusal> {
    let repositories = domain.forge.writable_repositories(context.project);
    let mut chosen = None;
    for repository in &repositories {
        let fits = match named {
            Some(named) => repository_name(repository).as_ref() == named,
            None => repositories.len() == 1,
        };
        if fits {
            if chosen.is_some() {
                return Err(tasks::Refusal::Unknown);
            }
            chosen = Some(*repository);
        }
    }
    let repository = chosen.ok_or(tasks::Refusal::Unknown)?;
    // The procedure takes a task and a level, and its producer another of each; with none at
    // all, the delegation's own admission names the missing authority.
    if context.tasks_left == 1 {
        return Err(tasks::Refusal::Tree);
    }
    if context.authority.delegation.depth == 1 {
        return Err(tasks::Refusal::Depth);
    }
    let available = context
        .numbers
        .budget
        .checked_sub(context.numbers.spent)
        .ok_or(tasks::Refusal::Funding)?
        .checked_sub(context.numbers.spent_below)
        .ok_or(tasks::Refusal::Funding)?
        .checked_sub(context.numbers.reserved)
        .ok_or(tasks::Refusal::Funding)?;
    let spend = available / 2;
    if spend <= domain.core.authority.rules().minimum_run_spend {
        return Err(tasks::Refusal::Funding);
    }
    let mut authority = context.authority.clone();
    authority.grants = change_grants(domain, env, repository, &authority.grants)?;
    let mut kinds = List::with_capacity(3);
    kinds.push(tasks::AuthorityExecutor::Charter(domain.config.forge_charter)).expect("producer kind");
    if domain.config.forge_charter != 4 {
        kinds.push(tasks::AuthorityExecutor::Charter(4)).expect("review kind");
    }
    let gates =
        configured_gates(domain, env, repository, &repository.settings.default_branch).ok_or(tasks::Refusal::Spec)?;
    for gate in &gates {
        if gate.kind == forge_change::GateKind::Person && !kinds.as_slice().contains(&tasks::AuthorityExecutor::Role(1))
        {
            kinds.push(tasks::AuthorityExecutor::Role(1)).expect("one person gate role");
        }
    }
    authority.delegation.kinds = kinds.into_boxed();
    // A producer, a review, and two repairs each followed by its review.
    authority.delegation.tasks = 6.min(context.tasks_left.saturating_sub(1));
    authority.delegation.depth = 2.min(authority.delegation.depth.saturating_sub(1));
    authority.budget.spend = spend;
    authority.notes = tasks::Scopes(0);
    authority.note_resources = Box::new([]);
    Ok(super::Delegate {
        deadline: tasks::Deadline::Named,
        executor: tasks::Executor::Procedure { connector: domain.config.forge_connector, code: 2 },
        spec: tasks::Spec {
            words,
            parameters: Box::new(change_parameters(
                domain.config.forge_connector,
                repository.provider,
                repository.settings.default_branch.clone(),
            )),
            inputs: Box::new([]),
        },
        // Leave room for the pull's words and address as well as the merge id.
        contract: tasks::Contract::Change {
            connector: domain.config.forge_connector,
            kind: 1,
            words: 512.min(env.limits.tasks.result_bytes),
        },
        authority,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        tracked: None,
        label: 0,
        flags: tasks::Flags::PLAIN,
        budget: people::Budget::Named,
    })
}

/// jig owns both the descendant ledger and remaining spend; take a fresh snapshot.
fn change_authority_lack(domain: &Domain, task: u64) -> Option<forge_change::AuthorityHold> {
    use forge_change::{AuthorityHold, AuthorityLack};
    let context = domain.core.tasks.delegation(task)?;
    let (lack, had) = if context.tasks_left == 0 {
        (AuthorityLack::Tasks, u64::from(context.authority.delegation.tasks))
    } else if context.authority.delegation.depth == 0 {
        (AuthorityLack::Depth, 0)
    } else {
        let available = context
            .numbers
            .budget
            .saturating_sub(context.numbers.spent)
            .saturating_sub(context.numbers.spent_below)
            .saturating_sub(context.numbers.reserved);
        if available > domain.core.authority.rules().minimum_run_spend {
            return None;
        }
        (AuthorityLack::Spend, available)
    };
    Some(AuthorityHold { recovery: change_recovery(domain, task)?, lack, had, released: false })
}

fn change_recovery(domain: &Domain, task: u64) -> Option<forge_change::AuthorityRecovery> {
    let row = domain.core.tasks.task(task)?;
    Some(forge_change::AuthorityRecovery {
        task,
        spec: row.revisions.spec,
        hold: row.revisions.hold.checked_add(1)?,
        // made includes the change itself; that count admits one more descendant.
        tasks: u64::from(row.made),
    })
}

fn change_spend(domain: &Domain, task: u64) -> Option<forge_change::DelegateSpend> {
    let context = domain.core.tasks.delegation(task)?;
    let left = context
        .numbers
        .budget
        .saturating_sub(context.numbers.spent)
        .saturating_sub(context.numbers.spent_below)
        .saturating_sub(context.numbers.reserved);
    Some(forge_change::DelegateSpend {
        recovery: change_recovery(domain, task)?,
        left,
        coding: jig_domain_core::reserve_floor(&domain.core.charter(domain.config.forge_charter)?.policy)
            .unwrap_or(u64::MAX),
        review: jig_domain_core::reserve_floor(&domain.core.charter(domain.config.forge_charter)?.policy)
            .unwrap_or(u64::MAX),
    })
}

fn child_authority(context: &tasks::DelegationContext) -> Option<tasks::Authority> {
    let mut authority = context.authority.clone();
    let available = context
        .numbers
        .budget
        .checked_sub(context.numbers.spent)?
        .checked_sub(context.numbers.spent_below)?
        .checked_sub(context.numbers.reserved)?;
    authority.budget.spend = authority.budget.spend.min(available);
    if authority.budget.spend == 0 {
        return None;
    }
    authority.delegation.kinds = Box::new([]);
    authority.delegation.tasks = 0;
    authority.delegation.depth = 0;
    Some(authority)
}

fn change_delegate(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    kind: forge_change::Delegate,
) -> Option<super::Delegate> {
    let row = domain.forge.change(task)?;
    let context = domain.core.tasks.delegation(task)?;
    let mut authority = child_authority(&context)?;
    // Every agent below a change asks no one (domain/agent.md, 8.2) and
    // leaves deployment settings to its requester.
    authority.tools.0 &= !(domain.core.settings.tools.message.0 | domain.core.settings.tools.settings);
    let mut parameters = List::with_capacity(env.limits.tasks.parameters);
    parameters
        .push(tasks::Parameter::Resource {
            name: 1,
            connector: domain.config.forge_connector,
            resource: forge::tag(row.repository.forge, row.repository.repository),
        })
        .ok()?;
    parameters.push(tasks::Parameter::Bytes { name: 2, value: row.base.clone() }).ok()?;
    parameters.push(tasks::Parameter::Bytes { name: 3, value: row.branch.clone() }).ok()?;
    let (executor, words, contract) = match kind {
        forge_change::Delegate::Produce => (
            tasks::Executor::Agent { charter: row.producing_charter },
            produce_words(&row.title, &row.base, &row.branch, env.limits.tasks.spec_bytes),
            tasks::Contract::Change {
                connector: domain.config.forge_connector,
                kind: 2,
                words: env.limits.tasks.result_bytes,
            },
        ),
        forge_change::Delegate::Repair(why) => {
            let words: &[u8] = match why {
                forge_change::Repair::Ci => b"Repair the failed CI on the change branch",
                forge_change::Repair::Semantic => b"Repair the base update's semantic conflict",
                forge_change::Repair::Gate(_) => b"Repair the requested gate changes",
            };
            (
                tasks::Executor::Agent { charter: row.producing_charter },
                Box::from(words),
                tasks::Contract::Change {
                    connector: domain.config.forge_connector,
                    kind: 2,
                    words: env.limits.tasks.result_bytes,
                },
            )
        }
        forge_change::Delegate::Resolve { base } => {
            parameters.push(tasks::Parameter::Bytes { name: 6, value: commit_text(&base) }).ok()?;
            (
                tasks::Executor::Agent { charter: row.producing_charter },
                Box::from(&b"Resolve the merge conflict and push the merge commit"[..]),
                tasks::Contract::Change {
                    connector: domain.config.forge_connector,
                    kind: 2,
                    words: env.limits.tasks.result_bytes,
                },
            )
        }
        forge_change::Delegate::Gate { number, head } => {
            parameters.push(tasks::Parameter::Number { name: 7, value: number }).ok()?;
            parameters.push(tasks::Parameter::Bytes { name: 8, value: commit_text(&head) }).ok()?;
            let mut found = None;
            for gate in &row.change.gates {
                if gate.number == number {
                    found = Some(*gate);
                }
            }
            let gate = found?;
            gate_authority(domain, env, &mut authority, gate.kind)?;
            (
                gate_executor(row, gate.kind),
                review_words(row, number, &head, parameters.as_slice(), env.limits.tasks.spec_bytes)?,
                tasks::Contract::Verdict {
                    choices: Box::new([
                        tasks::Verdict {
                            code: 1,
                            words: env.limits.tasks.result_bytes,
                            followups: 0,
                            fields: Box::new([]),
                        },
                        tasks::Verdict {
                            code: 2,
                            words: env.limits.tasks.result_bytes,
                            followups: 0,
                            fields: Box::new([]),
                        },
                    ]),
                },
            )
        }
    };
    Some(super::Delegate {
        deadline: tasks::Deadline::Named,
        tracked: None,
        label: 0,
        flags: tasks::Flags::PLAIN,
        budget: jig_domain_people::Budget::Named,
        executor,
        spec: tasks::Spec { words, parameters: parameters.into_boxed(), inputs: Box::new([]) },
        contract,
        authority,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        llm: None,
    })
}

/// What the producer is not to do: the change task does the rest of the landing.
const PRODUCE_SCOPE: &[u8] = b"The host pushes what you deliver; the change task opens the pull request, \
waits for CI and merges it. Finish with your change once it is checked. \
Report the branch and head you produced; only the change task reports landing after merge.";

/// The producer's brief: the change's words, then its scope, if the spec's
/// bound has room for both beside its branches; the words alone otherwise.
/// domain/agent.md, 8: a review holds only the run's existing reads.
fn gate_authority(
    domain: &Domain,
    env: &Env<Limits>,
    authority: &mut tasks::Authority,
    kind: forge_change::GateKind,
) -> Option<()> {
    if kind != forge_change::GateKind::Agent {
        return Some(());
    }
    authority.tools.0 &= domain.core.settings.tools.read.0;
    let mut grants = List::with_capacity(env.limits.tasks.authority_grants);
    for grant in &authority.grants {
        if grant.connector == domain.config.forge_connector && forge::READ_KINDS.contains(&grant.kind) {
            grants.push(grant.clone()).ok()?;
        }
    }
    authority.grants = grants.into_boxed();
    Some(())
}

fn review_failure(row: &forge::ChangeRow, number: u64, head: skein_forge::api::Commit) -> Option<&[u8]> {
    for failure in &row.change.gate_failures {
        if failure.number != number || failure.count == 0 {
            continue;
        }
        for remark in &row.gate_remarks {
            if remark.number == number && remark.head == head {
                return Some(&remark.words);
            }
        }
    }
    None
}

/// domain/forge.md, 8.3: identity precedes the change's words and pull section.
fn review_words(
    row: &forge::ChangeRow,
    gate: u64,
    head: &skein_forge::api::Commit,
    parameters: &[tasks::Parameter],
    bound: u32,
) -> Option<Box<[u8]>> {
    // The spec's byte bound includes its base, branch and named head.
    let mut room = bound;
    for parameter in parameters {
        if let tasks::Parameter::Bytes { value, .. } = parameter {
            room = room.saturating_sub(u32::try_from(value.len()).unwrap_or(u32::MAX));
        }
    }
    let mut words = List::with_capacity(room);
    // domain/forge.md, 8.3: the retry's own context precedes the common review words.
    if let Some(failure) = review_failure(row, gate, *head) {
        append(&mut words, b"This review is asked again at the same head: the first review failed: ");
        append(&mut words, failure);
        append(&mut words, b"\n");
    }
    let number = skein_lib::Decimal::of(row.pull?);
    let head = commit_text(head);
    let identity: [&[u8]; 9] = [
        b"Review this change at the named head\nPull request: #",
        number.as_bytes(),
        b"\nBranch: ",
        &row.branch,
        b"\nBase: ",
        &row.base,
        b"\nHead: ",
        &head,
        b"\n",
    ];
    for part in identity {
        // The identity is required; a too-small spec cannot start an unnamed review.
        if part.len() > usize::try_from(words.room()).expect("bounded spec room") {
            return None;
        }
        append(&mut words, part);
    }
    append(&mut words, b"Verdict 1 approves; verdict 2 requests changes. Change: ");
    append(&mut words, &row.title);
    Some(words.into_boxed())
}

fn produce_words(title: &[u8], base: &[u8], branch: &[u8], bound: u32) -> Box<[u8]> {
    let room = usize::try_from(bound).unwrap_or(usize::MAX).saturating_sub(base.len()).saturating_sub(branch.len());
    let parts = [title, b"\n\n".as_slice(), PRODUCE_SCOPE];
    let length = title.len().saturating_add(2).saturating_add(PRODUCE_SCOPE.len());
    if length > room {
        return title.into();
    }
    let mut writer = skein_lib::Writer::new(length);
    for part in parts {
        writer.put(part).expect("measured brief");
    }
    writer.finish()
}

/// Keep the decision's messages first, then fill its remaining room with inbox replies.
pub(super) fn append_change_replies(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    decision: ProcedureStep,
) -> ProcedureStep {
    let mut messages = List::with_capacity(env.limits.tasks.batch);
    for message in decision.messages {
        messages.push(message).expect("the decision's messages fit the step batch");
    }
    ProcedureStep { messages: change_replies(domain, env, task, messages), ..decision }
}

/// domain/forge.md, 8.1: replies take the same inbox prefix as their step.
fn change_replies(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    mut messages: List<jig_domain_core::StepMessage>,
) -> Box<[jig_domain_core::StepMessage]> {
    let Some(row) = domain.core.tasks.task(task) else { return messages.into_boxed() };
    let Some(offered) = domain.core.tasks.procedure_offered(task) else { return messages.into_boxed() };
    for word in &row.inbox {
        // domain/forge.md, 8.1: answer the oldest within the batch; the rest are taken unanswered.
        if messages.len() == env.limits.tasks.batch {
            break;
        }
        if word.number > offered {
            continue;
        }
        let kind = match word.kind {
            tasks::MessageKind::Question => tasks::MessageKind::Answer { question: word.number },
            tasks::MessageKind::Words | tasks::MessageKind::Answer { .. } => tasks::MessageKind::Words,
            tasks::MessageKind::Result(_)
            | tasks::MessageKind::Escalation { .. }
            | tasks::MessageKind::Proposal { .. }
            | tasks::MessageKind::ProposalDecision { .. }
            | tasks::MessageKind::SettingsDecision { .. }
            | tasks::MessageKind::FinishRequested { .. }
            | tasks::MessageKind::Amendment { .. }
            | tasks::MessageKind::Met { .. }
            | tasks::MessageKind::Notice { .. }
            | tasks::MessageKind::Timer { .. }
            | tasks::MessageKind::PersonStalled { .. }
            | tasks::MessageKind::EffectNotMade { .. }
            | tasks::MessageKind::News { .. } => continue,
        };
        let tasks::Party::Task(target) = word.from else { continue };
        // Jig's domain/tasks.md, 8.5: a step can address only these tasks.
        if row.requester != tasks::Party::Task(target)
            && !row.delegates.contains(&target)
            && !row.references.contains(&target)
        {
            continue;
        }
        messages
            .push(jig_domain_core::StepMessage {
                target,
                kind,
                words: takes_no_messages(row.requester, env.limits.tasks.message_bytes),
            })
            .expect("replies capped at the step batch");
    }
    messages.into_boxed()
}

/// The change names who requested its already agreed work (domain/forge.md, 8.1).
fn takes_no_messages(requester: tasks::Party, limit: u32) -> Box<[u8]> {
    let mut words = List::with_capacity(limit);
    append(&mut words, b"A change takes no questions, answers or words; its requester is ");
    let (kind, number): (&[u8], u64) = match requester {
        tasks::Party::Task(number) => (b"task ", number),
        tasks::Party::Person(number) => (b"person ", number),
        tasks::Party::Deployment { project } => (b"deployment for project ", u64::from(project)),
    };
    append(&mut words, kind);
    append(&mut words, &decimal(number));
    append(&mut words, b".");
    words.into_boxed()
}

fn gate_executor(row: &forge::ChangeRow, kind: forge_change::GateKind) -> tasks::Executor {
    match kind {
        forge_change::GateKind::Agent => tasks::Executor::Agent { charter: row.review_charter },
        forge_change::GateKind::Check => tasks::Executor::Agent { charter: row.producing_charter },
        forge_change::GateKind::Person => tasks::Executor::Person(tasks::PersonAddress::Role(1)),
    }
}

/// Admit one project-funded repair of a broken landing branch. The owner row
/// records its identity in the same root decision as the new task.
fn start_queue_repair(domain: &mut Domain, env: &Env<Limits>, task: u64) -> bool {
    let Some(row) = domain.forge.change(task) else { return false };
    if let Some((_, repair)) = domain.forge.queue_repair(row.repository, &row.base)
        && domain.core.tasks.task(repair).is_some()
    {
        return true;
    }
    if domain.forge.queue_repairs(row.repository, &row.base) >= env.limits.forge.change_policy.repairs {
        return false;
    }
    let Some(repository) = domain.forge.repository(row.repository) else { return false };
    let project = repository.project;
    let Some(seed) = domain.core.repair_seed(project) else { return false };
    let repair = seed.number();
    let provider = row.repository;
    let base = row.base.clone();
    let [repository_parameter, base_parameter] = change_parameters(domain.config.forge_connector, provider, base);
    let spec = tasks::Spec {
        words: Box::from(&b"Repair landing branch CI"[..]),
        parameters: Box::new([
            repository_parameter,
            base_parameter,
            // queue_before and queue_first put the base repair first themselves; priority need not claim it.
            tasks::Parameter::Number { name: 5, value: 0 },
            tasks::Parameter::Number { name: 9, value: 1 },
        ]),
        inputs: Box::new([]),
    };
    let executor = tasks::Executor::Procedure { connector: domain.config.forge_connector, code: 2 };
    let Some(holdings) = task_holdings(domain, env, project, repair, repair, executor, &spec, None) else {
        return false;
    };
    domain.work.push(Work::Forge(forge::Event::QueueRepairStarted { owner: task, repair }));
    domain.work.push(Work::Core(jig_domain_core::Event::ConnectorRepair {
        seed: Box::new(seed),
        executor,
        spec,
        contract: tasks::Contract::Change {
            connector: domain.config.forge_connector,
            kind: 1,
            words: env.limits.tasks.result_bytes,
        },
        holdings,
    }));
    true
}

fn change_landing_words(
    domain: &Domain,
    task: u64,
    merge: &skein_forge::api::Commit,
    pull: u64,
    bound: u32,
) -> Box<[u8]> {
    let row = domain.forge.change(task).expect("landed change");
    let repository = domain.forge.repository(row.repository);
    let configured = domain.forge.configured(row.repository.forge);
    let identity = landing_words(merge, bound, repository, pull, configured);
    let Some(head) = row.change.last_head else { return identity };
    let room = bound.saturating_sub(u32::try_from(identity.len()).expect("bounded identity")).saturating_sub(1);
    let Some(evidence) = row.result.words(&head, bound).fitted(room) else { return identity };
    let mut out = List::with_capacity(bound);
    append(&mut out, &identity);
    append(&mut out, b"\n");
    append(&mut out, &evidence);
    out.into_boxed()
}

/// Explain a failed review in the same decision that holds its change.
fn review_failure_message(domain: &Domain, env: &Env<Limits>, task: u64) -> Option<jig_domain_core::StepMessage> {
    let row = domain.forge.change(task)?;
    let context = domain.core.tasks.delegation(task)?;
    let tasks::Party::Task(requester) = context.requester else { return None };
    let mut reason = None;
    for failure in &row.change.gate_failures {
        if failure.count < 2 {
            continue;
        }
        for remark in &row.gate_remarks {
            if remark.number == failure.number && Some(remark.head) == row.change.last_head {
                let mut words = List::with_capacity(env.limits.tasks.message_bytes);
                append(&mut words, b"Change held: review gate ");
                append(&mut words, &decimal(failure.number));
                append(&mut words, b" failed twice: ");
                append(&mut words, &remark.words);
                reason = Some(words.into_boxed());
                break;
            }
        }
        if reason.is_some() {
            break;
        }
    }
    Some(jig_domain_core::StepMessage { target: requester, kind: tasks::MessageKind::Words, words: reason? })
}

fn authority_hold_message(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    cause: forge_change::AuthorityHold,
) -> Option<jig_domain_core::StepMessage> {
    let context = domain.core.tasks.delegation(task)?;
    let tasks::Party::Task(target) = context.requester else { return None };
    let mut words = List::with_capacity(env.limits.tasks.message_bytes);
    append(&mut words, &forge_change::authority_words(cause));
    Some(jig_domain_core::StepMessage { target, kind: tasks::MessageKind::Words, words: words.into_boxed() })
}

fn hold_change_decision(domain: &Domain, env: &Env<Limits>, task: u64, why: forge_change::Hold) -> ProcedureStep {
    let message = if let forge_change::Hold::Authority(cause) = why {
        authority_hold_message(domain, env, task, cause)
    } else if why == forge_change::Hold::Failed {
        review_failure_message(domain, env, task)
    } else {
        None
    };
    let why = match why {
        forge_change::Hold::BranchMoved
        | forge_change::Hold::BranchMissing
        | forge_change::Hold::Retargeted
        | forge_change::Hold::PullClosed => tasks::Hold::Drift,
        forge_change::Hold::Authority(cause) => match cause.lack {
            forge_change::AuthorityLack::Tasks | forge_change::AuthorityLack::Depth => tasks::Hold::Procedure,
            forge_change::AuthorityLack::Spend => tasks::Hold::Budget,
        },
        forge_change::Hold::Stalled => tasks::Hold::Stalled,
        forge_change::Hold::Repairs | forge_change::Hold::Resolutions | forge_change::Hold::Updates => {
            tasks::Hold::Procedure
        }
        forge_change::Hold::Failed | forge_change::Hold::Rejected => tasks::Hold::EffectFailed,
    };
    let mut decision = ProcedureStep::new(StepEnd::Hold(why));
    if let Some(message) = message {
        decision.messages = Box::new([message]);
    }
    decision
}

fn cancel_change_gates(domain: &Domain, task: u64, head: [u8; 32]) -> ProcedureStep {
    let child = match domain.forge.change(task) {
        Some(row) => match row.delegate {
            Some((child, forge_change::Delegate::Gate { head: live, .. })) if live == head => Some(child),
            Some(_) | None => None,
        },
        None => None,
    };
    let cancels: Box<[jig_domain_core::StepCancel]> = match child {
        Some(child) => {
            let mut reason = List::with_capacity(96);
            append(&mut reason, b"CI failed at head ");
            for byte in &head {
                append(&mut reason, &forge::hex(*byte));
            }
            Box::new([jig_domain_core::StepCancel { task: child, reason: reason.into_boxed(), grace: None }])
        }
        None => Box::new([]),
    };
    ProcedureStep { cancels, ..ProcedureStep::new(StepEnd::Wait) }
}

fn change_result_bound(domain: &Domain, task: u64) -> u32 {
    match domain.core.tasks.task(task) {
        Some(row) => match row.contract {
            tasks::Contract::Change { words, .. } => words,
            tasks::Contract::Approval { .. } | tasks::Contract::Report { .. } | tasks::Contract::Verdict { .. } => 0,
        },
        None => 0,
    }
}

fn change_decision(domain: &mut Domain, env: &Env<Limits>, task: u64, choice: forge_change::Decision) {
    let Some((connector, code, step)) = domain.core.tasks.procedure_due(task) else { return };
    if connector != domain.config.forge_connector || code != 2 {
        return;
    }
    domain.forge.offered_change(task);
    let mut again = false;
    let decision = match choice {
        forge_change::Decision::Finish { merge } => {
            let pull = match domain.forge.change(task) {
                Some(row) => row.pull,
                None => None,
            };
            let Some(pull) = pull else {
                domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
                    task,
                    step,
                    connector,
                    code,
                    decision: append_change_replies(
                        domain,
                        env,
                        task,
                        ProcedureStep::new(StepEnd::Hold(tasks::Hold::Effects)),
                    ),
                }));
                return;
            };
            let bound = change_result_bound(domain, task);
            ProcedureStep::new(StepEnd::Result(tasks::TaskResult::Change {
                connector: domain.config.forge_connector,
                kind: 1,
                resource: pull,
                words: change_landing_words(domain, task, &merge, pull, bound),
            }))
        }
        forge_change::Decision::Hold(why) => hold_change_decision(domain, env, task, why),
        forge_change::Decision::CancelGates { head } => cancel_change_gates(domain, task, head),
        forge_change::Decision::Ready => {
            again = true;
            ProcedureStep::new(StepEnd::Wait)
        }
        forge_change::Decision::QueueRepair => {
            if start_queue_repair(domain, env, task) {
                domain.forge.wait_change(&super::environment_forge(env), task, None);
                ProcedureStep::new(StepEnd::Wait)
            } else {
                ProcedureStep::new(StepEnd::Hold(tasks::Hold::Effects))
            }
        }
        forge_change::Decision::Wait { until } => {
            domain.forge.wait_change(&super::environment_forge(env), task, until);
            ProcedureStep::new(StepEnd::Wait)
        }
        forge_change::Decision::None | forge_change::Decision::Effect(_) | forge_change::Decision::Cancel => {
            ProcedureStep::new(StepEnd::Wait)
        }
        forge_change::Decision::Delegate(kind) => {
            let Some(delegate) = change_delegate(domain, env, task, kind) else {
                domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
                    task,
                    step,
                    connector,
                    code,
                    decision: append_change_replies(
                        domain,
                        env,
                        task,
                        ProcedureStep::new(StepEnd::Hold(tasks::Hold::Effects)),
                    ),
                }));
                return;
            };
            assert!(domain.forge_delegating.insert((task, step), kind).is_ok(), "one forge procedure delegate");
            domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
                task,
                step,
                connector,
                code,
                decision: append_change_replies(
                    domain,
                    env,
                    task,
                    ProcedureStep { batch: Box::new([delegate]), ..ProcedureStep::new(StepEnd::Wait) },
                ),
            }));
            return;
        }
    };
    let decision = append_change_replies(domain, env, task, decision);
    domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep { task, step, connector, code, decision }));
    if again {
        domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task }));
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive connector output translation")]
pub(crate) fn outputs(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut Decision,
    out: &mut Queue<forge::Request>,
) {
    for _ in 0..out.len() {
        let request = out.pop().expect("connector output count");
        match request {
            forge::Request::CarriedUnused { number } => emit(decision, &env.limits, Delivery::CarriedUnused { number }),
            forge::Request::EraseCarried { number } => {
                assert!(super::now(domain, Request::EraseCarried { number }), "erase door");
            }
            forge::Request::Connect { forge, request, how, carried } => {
                assert!(super::now(domain, Request::Connect { forge, request, how, carried }), "connect door");
            }
            forge::Request::Manifest { forge, request, back } => {
                assert!(super::now(domain, Request::Manifest { forge, request, back }), "manifest door");
            }
            forge::Request::Reachable { forge, request, installation, page } => {
                assert!(
                    super::now(domain, Request::Reachable { forge, request, installation, page }),
                    "reachable door"
                );
            }
            forge::Request::Accept { forge, request, installation } => {
                assert!(super::now(domain, Request::Accept { forge, request, installation }), "accept door");
            }
            forge::Request::Install { forge, request, back } => {
                assert!(super::now(domain, Request::Install { forge, request, back }), "install door");
            }
            forge::Request::Bound { request, answer } => {
                emit(decision, &env.limits, Delivery::Bound { request, answer });
            }
            forge::Request::ListDemoActs => super::demo::list_end_acts(domain),
            forge::Request::SendDemoAct(row) => super::demo::send_end_act(domain, row),
            forge::Request::DemoStart { request } => super::demo::start(domain, env, decision, request),
            forge::Request::StopGroup { forge } => {
                assert!(super::now(domain, Request::StopGroup { forge }), "demo stop door reserved");
            }
            forge::Request::SetUpGroup { forge, request } => {
                assert!(super::now(domain, Request::SetUpGroup { forge, request }), "demo box group door reserved");
            }
            forge::Request::SetUp { forge, request, org, repository, playground } => {
                assert!(
                    super::now(domain, Request::SetUp { forge, request, org, repository, playground }),
                    "forge set up door reserved"
                );
            }

            forge::Request::ApplicationPart { request, adoption, repository } => {
                super::applications::part(domain, request, adoption, repository);
            }
            forge::Request::ApplicationDone { request, token } => {
                super::demo::done(domain, env, decision, request, true);
                super::applications::finished(domain, request, Ok(token));
            }
            forge::Request::ApplicationAgain { request, why } => {
                super::demo::done(domain, env, decision, request, false);
                super::applications::finished(domain, request, Err(why));
            }
            forge::Request::WakeProcedure { task } => {
                // A restored change also repairs names saved before its pull was known.
                if let Some(row) = domain.forge.change(task).cloned() {
                    watch_change_pull(domain, env, &row);
                }
                domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task }));
            }
            forge::Request::Resource { project, name, role, hold } => {
                route_resource(domain, env, project, &name, role, hold);
            }

            forge::Request::GoalTopic { goal, topic } => subscribe_goal(domain, env, goal, topic),
            forge::Request::GoalUntopic { goal, topic } => {
                if let Some(subscriber) = domain.forge.topic_subscriber(goal, &topic) {
                    domain.work.push(Work::Tasks(tasks::Event::Unsubscribe {
                        reply_to: super::internal(u64::MAX),
                        task: goal,
                        subscription: subscriber.number,
                    }));
                    domain.work.push(Work::Forge(forge::Event::Unsubscribe { task: goal, topic }));
                }
            }
            forge::Request::RestartDone { stage } => {
                let stage = match stage {
                    forge::RestartStage::Restored => jig_domain_core::connector::RestartStage::Restored,
                    forge::RestartStage::ReadAfresh => jig_domain_core::connector::RestartStage::ReadAfresh,
                    forge::RestartStage::Settled => jig_domain_core::connector::RestartStage::Settled,
                };
                super::connector_restart_done(domain, stage);
            }
            forge::Request::BriefClient { event } => domain.work.push(Work::Forge(forge::Event::Client(event))),
            forge::Request::BriefReady { .. } => unreachable!("the root uses held forge sections"),
            forge::Request::BriefSized { section, size } => {
                let Some(row) = domain.brief_connectors.get(Id::from_token(section)) else { continue };
                let brief = Token::new(row.task);
                let event = match size {
                    Some(size) if row.cutting => brief::GatherEvent::Cut { brief, section, size },
                    Some(size) => brief::GatherEvent::Ready { brief, section, size },
                    None => brief::GatherEvent::Missing { brief, section },
                };
                domain.work.push(Work::Brief(event));
            }
            forge::Request::BriefTaken { .. } => unreachable!("the root takes completed sections directly"),
            forge::Request::MarkedForges => {
                let mut subjects = List::with_capacity(1);
                subjects.push(Token::new(0)).expect("one forges subject");
                domain.work.push(Work::Core(jig_domain_core::Event::Marked {
                    connector: domain.config.forge_connector,
                    subjects,
                }));
            }
            forge::Request::Marked { tasks } => {
                const {
                    assert!(
                        forge::MARKED <= crate::limits::VIEWS.subjects_per_change,
                        "forge marks fit the views subject budget"
                    );
                }
                assert!(tasks.len() <= usize::try_from(forge::MARKED).expect("u32 fits usize"), "bounded panel marks");
                let mut subjects = List::with_capacity(forge::MARKED);
                for task in tasks {
                    subjects.push(Token::new(task)).expect("bounded panel marks");
                }
                domain.work.push(Work::Core(jig_domain_core::Event::Marked {
                    connector: domain.config.forge_connector,
                    subjects,
                }));
            }
            forge::Request::Settled { unfinished } => crate::settling::finished(domain, unfinished),
            forge::Request::Save { record, release, read_afresh } => {
                // Both opening and finding a pull save the change with its number.
                if let forge::Stored::Change(row) = &record {
                    watch_change_pull(domain, env, row);
                }
                if let Some(name) = read_afresh
                    && let Some(resource) = hub_name(domain.config.forge_connector, &name, &env.limits.tasks)
                {
                    domain.work.push(Work::Tasks(tasks::Event::ReadAfresh { resource }));
                }
                save(decision, &env.limits, Write::Save(Record::Forge { row: Box::new(record) }));
                if let Some(entry) = release {
                    let core_effect = domain.forge_effecting.remove(&entry).is_some();
                    if !core_effect {
                        emit(decision, &env.limits, Delivery::ForgeCommitted { entry });
                    }
                }
            }
            forge::Request::Erase { key } => {
                save(decision, &env.limits, Write::Erase(Key::Forge(key)));
            }
            forge::Request::Call { call, repository, op } => match op {
                op @ skein_forge::api::Op::Read(_) => {
                    assert!(super::now(domain, Request::Forge { call, repository, op }), "forge read door reserved");
                }
                op @ skein_forge::api::Op::Write(_) => {
                    emit(decision, &env.limits, Delivery::ForgeCall { call, repository, op });
                }
            },
            forge::Request::Read { owner, result } => {
                if let Some((to, _key)) = domain.forge_reading.remove(&owner) {
                    super::host_route::read(domain, to, CallAnswer::ForgeRead(Box::new(result)));
                }
            }
            forge::Request::Adopted { reply_to, result } => {
                let previous = domain.adoption_restore.remove(&reply_to).expect("one admitted adoption result");
                match result {
                    Ok(adopted) => {
                        let mut parties = List::with_capacity(env.limits.forge.collaborators);
                        for collaborator in &adopted.collaborators {
                            if let Some(role) = seeded_role(domain, adopted.repository.project, collaborator.permission)
                            {
                                parties
                                    .push(jig_domain_core::connector::AdoptedParty {
                                        subject: collaborator.user.to_be_bytes().into(),
                                        role,
                                    })
                                    .expect("bounded collaborators");
                            }
                        }
                        match domain.core.connector_adopt_parties(
                            &env.limits.people,
                            adopted.repository.project,
                            0,
                            parties.into_boxed(),
                        ) {
                            Ok(seeds) => {
                                domain.work.push(Work::People(people::Event::Seed {
                                    project: adopted.repository.project,
                                    collaborators: seeds,
                                }));
                                domain.work.push(Work::People(people::Event::Decided {
                                    request: reply_to,
                                    outcome: people::Outcome::Adopted { project: adopted.repository.project },
                                }));
                            }
                            Err(why) => {
                                domain.work.push(Work::Forge(forge::Event::ForgetAdoption {
                                    repository: adopted.repository.provider,
                                    restore: previous,
                                }));
                                domain
                                    .work
                                    .push(Work::People(people::Event::Decided { request: reply_to, outcome: why }));
                            }
                        }
                    }
                    Err(error) => {
                        // The forge's adoption errors as jig's facts (brief D12).
                        let outcome = adoption_failure(error);
                        domain.work.push(Work::People(people::Event::Decided { request: reply_to, outcome }));
                    }
                }
            }
            forge::Request::News { task, subscription, news, class, .. } => {
                let class = match class {
                    forge::Class::Wakes => tasks::NewsClass::Wakes,
                    forge::Class::Kept => tasks::NewsClass::Kept,
                    forge::Class::Dropped => tasks::NewsClass::Dropped,
                };
                if let Some(event) = domain.core.connector_news(
                    task,
                    subscription,
                    class,
                    news_words(&news, env.limits.tasks.message_bytes),
                    env.wall,
                ) {
                    domain.work.push(Work::Tasks(event));
                }
            }
            forge::Request::Taken { task, .. } | forge::Request::Refused { task } => {
                if domain.core.claiming.contains_key(&task) {
                    domain.work.push(Work::Core(jig_domain_core::Event::ClaimRefused { task }));
                    continue;
                }
                let mut failed = None;
                for (&entry, (_, key)) in &domain.forge_effecting {
                    if key.task == task {
                        failed = Some(entry);
                        break;
                    }
                }
                if let Some(entry) = failed {
                    let (_, key) = domain.forge_effecting.remove(&entry).expect("effect awaiting connector admission");
                    domain
                        .forge
                        .named_outcome(entry, skein_forge_client::Outcome::Failed(skein_forge::api::Error::Busy));
                    domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                        jig_domain_core::connector::Event::Outbox {
                            connector: domain.config.forge_connector,
                            entry,
                            task: key.task,
                            outcome: jig_domain_core::connector::OutboxOutcome::Failed,
                        },
                    )));
                }
                domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
            }
            forge::Request::NotMade { entry, task } => {
                domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                    jig_domain_core::connector::Event::Outbox {
                        connector: domain.config.forge_connector,
                        entry,
                        task,
                        outcome: jig_domain_core::connector::OutboxOutcome::NotMade,
                    },
                )));
            }
            forge::Request::ProjectionOutcome { entry, outcome } => {
                domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                    jig_domain_core::connector::Event::ProjectionOutbox {
                        connector: domain.config.forge_connector,
                        entry,
                        outcome: outbox_outcome(entry, outcome),
                    },
                )));
            }
            forge::Request::Outcome { entry, task, outcome } => {
                if let Some(named) = domain.forge.named_effect(entry) {
                    save_connector_answer(domain, &env.limits, decision, core_call(named));
                }
                let result = outbox_outcome(entry, outcome);
                domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                    jig_domain_core::connector::Event::Outbox {
                        connector: domain.config.forge_connector,
                        entry,
                        task,
                        outcome: result,
                    },
                )));
            }
            forge::Request::ContinueRelease { task } => {
                if let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow) {
                    domain.work.push(Work::Forge(forge::Event::ContinueRelease { task, entry }));
                } else {
                    domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
                }
            }
            forge::Request::Released { task } => {
                domain.work.push(Work::Tasks(tasks::Event::Settled { task }));
            }
            forge::Request::Retained { task, root, resource } => {
                if let Some(holding) = write_holding(domain.config.forge_connector, &resource, &env.limits.tasks) {
                    domain.work.push(Work::Tasks(tasks::Event::Retained { task, root, holding }));
                } else {
                    domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
                }
            }
            forge::Request::EffectsSettled { task } => {
                domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                    jig_domain_core::connector::Event::Closed { task, connector: domain.config.forge_connector },
                )));
            }
            forge::Request::ReleaseFailed { task } => {
                domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::EffectFailed }));
            }
            forge::Request::ProjectionEffect { row, entry, description } => {
                let Some(adopted) = domain.forge.repository(entry.repository) else { continue };
                let Some(name) = resource_name(adopted, &description.resource, env.limits.authority.segments) else {
                    continue;
                };
                let access = match description.access {
                    forge::Access::Owned => authority::EffectAccess::Owned,
                    forge::Access::Participant => authority::EffectAccess::Participant,
                    forge::Access::Context => authority::EffectAccess::Context,
                    forge::Access::Unavailable => authority::EffectAccess::Unavailable,
                };
                let description = EffectDescription {
                    waits_for_dependencies: forge::waits_for_dependencies(description.kind),
                    connector: domain.config.forge_connector,
                    purpose: entry.number,
                    effect: authority::Effect {
                        connector: domain.config.forge_connector,
                        kind: description.kind,
                        name,
                        state: [0; 32],
                        price: None,
                        access,
                        additional: Box::new([]),
                        guards: Box::new([]),
                    },
                    form: match description.form {
                        forge::EffectForm::Creation => EffectForm::Creation,
                        forge::EffectForm::Transition => EffectForm::Transition,
                        forge::EffectForm::Set => EffectForm::Set,
                    },
                    recovery: match description.recovery {
                        forge::Recovery::Keyed => Recovery::Keyed,
                        forge::Recovery::Conditional => Recovery::Conditional,
                        forge::Recovery::Idempotent => Recovery::Idempotent,
                        forge::Recovery::Unrecoverable => Recovery::Unrecoverable,
                    },
                };
                let goal = row.goal;
                let number = entry.number;
                let owner = effect_owner(domain);
                domain
                    .forge_effects
                    .insert(
                        owner,
                        EffectFlight::Projection { description: Box::new(description), row: Box::new(row), entry },
                    )
                    .expect("bounded projection handoff");
                domain.work.push(Work::Core(jig_domain_core::Event::EffectStart {
                    owner,
                    connector: domain.config.forge_connector,
                    origin: jig_domain_core::EffectOrigin::Projection { goal, entry: Some(number) },
                }));
            }
            forge::Request::ProjectionFailed { goal } => {
                projection_failed(domain, goal, domain.core.tasks.task(goal).is_none());
            }
            forge::Request::Drift { task, .. } => {
                if domain.forge.change(task).is_some() {
                    // domain/forge.md, 8.5: let the procedure retain its precise
                    // hold and current step before holding the task. A generic
                    // hold here prevents its pending wake from running.
                    domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task }));
                } else {
                    domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Drift }));
                }
            }
            forge::Request::ChangeDecision { task, decision: choice, entry, evidence } => match choice {
                forge_change::Decision::Effect(effect) => {
                    if let Some(entry) = entry
                        && let Some(evidence) = evidence
                    {
                        match describe_change_effect(domain, env, task, effect, &evidence) {
                            Some(description) => {
                                let Some((_, _, step)) = domain.core.tasks.procedure_due(task) else { continue };
                                let owner = effect_owner(domain);
                                assert!(
                                    domain
                                        .forge_effects
                                        .insert(
                                            owner,
                                            EffectFlight::Procedure {
                                                description: Box::new(description),
                                                entry,
                                                task,
                                                evidence: Box::new(evidence)
                                            }
                                        )
                                        .is_ok(),
                                    "admitted procedure effect handoff"
                                );
                                domain.work.push(Work::Core(jig_domain_core::Event::EffectStart {
                                    owner,
                                    connector: domain.config.forge_connector,
                                    origin: jig_domain_core::EffectOrigin::Procedure { task, step, entry: Some(entry) },
                                }));
                            }
                            None => {
                                domain.work.push(Work::Forge(forge::Event::VetoChange {
                                    task,
                                    entry,
                                    prior: evidence.prior,
                                }));
                                change_decision(
                                    domain,
                                    env,
                                    task,
                                    forge_change::Decision::Hold(forge_change::Hold::Failed),
                                );
                            }
                        }
                    }
                }
                forge_change::Decision::None
                | forge_change::Decision::Wait { .. }
                | forge_change::Decision::Delegate(_)
                | forge_change::Decision::CancelGates { .. }
                | forge_change::Decision::Ready
                | forge_change::Decision::QueueRepair
                | forge_change::Decision::Finish { .. }
                | forge_change::Decision::Cancel
                | forge_change::Decision::Hold(_) => change_decision(domain, env, task, choice),
            },
            forge::Request::ProjectionSettled { goal } => {
                if domain.core.tasks.task(goal).is_none() {
                    domain.forge.projection_settled(goal);
                    domain.work.push(Work::Core(jig_domain_core::Event::ProjectionSettled {
                        goal,
                        connector: domain.config.forge_connector,
                    }));
                }
            }
            forge::Request::ProjectAfter { goal, when } => {
                if let Some(when) = when {
                    domain.forge.projection_after(goal, when);
                } else if match domain.forge.issue(goal) {
                    Some(row) => row.pending.is_none(),
                    None => false,
                } && let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow)
                {
                    domain.work.push(Work::Forge(forge::Event::ProjectDesired { entry, goal }));
                }
            }
        }
    }
}

/// Connector-owned effect data while the core chooses its requirements.
#[derive(Debug)]
pub(crate) enum EffectFlight {
    Call {
        description: Box<EffectDescription>,
        repository: skein_forge::api::Repository,
        effect: skein_forge_client::Effect,
        key: CallKey,
        resource: forge::What,
    },
    Projection {
        description: Box<EffectDescription>,
        row: Box<forge::IssueRow>,
        entry: skein_forge_client::Entry,
    },
    Procedure {
        description: Box<EffectDescription>,
        entry: u64,
        task: u64,
        evidence: Box<forge::ChangeEvidence>,
    },
}

/// Installation is missing access, not a transient busy adoption (domain/forge.md, 4.3).
fn adoption_failure(error: skein_forge::api::Error) -> people::Outcome {
    match error {
        skein_forge::api::Error::NotConnected
        | skein_forge::api::Error::Busy
        | skein_forge::api::Error::Unavailable
        | skein_forge::api::Error::Timeout
        | skein_forge::api::Error::RateLimited { .. } => people::Outcome::Again(people::Again::Busy),
        skein_forge::api::Error::TooLarge | skein_forge::api::Error::Full => {
            people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::Resource })
        }
        skein_forge::api::Error::NotInstalled
        | skein_forge::api::Error::Forbidden
        | skein_forge::api::Error::Protected
        | skein_forge::api::Error::Refused => people::Outcome::Refused(people::Refusal::Authority {
            lacking: people::Lack::ResourceAccess,
            proposable: None,
        }),
        skein_forge::api::Error::Missing => people::Outcome::Refused(people::Refusal::Authority {
            lacking: people::Lack::ResourceUnavailable,
            proposable: None,
        }),
        skein_forge::api::Error::MissingJob
        | skein_forge::api::Error::Empty
        | skein_forge::api::Error::Exists
        | skein_forge::api::Error::NothingToMerge
        | skein_forge::api::Error::Closed
        | skein_forge::api::Error::Stale
        | skein_forge::api::Error::Conflict
        | skein_forge::api::Error::InvalidAnswer => people::Outcome::Refused(people::Refusal::Unknown),
    }
}

fn effect_owner(domain: &mut Domain) -> Token {
    let owner = Token::new(domain.next_effect_owner);
    domain.next_effect_owner = domain.next_effect_owner.checked_add(1).expect("transient effect owner counter");
    owner
}

/// Assemble connector evidence without replacing the core's status decision.
pub(crate) fn effect_answer(domain: &Domain, key: CallKey, part: &jig_domain_core::CallPart) -> CallAnswer {
    match part {
        jig_domain_core::CallPart::Effect { .. } => match connector_answer(domain, key) {
            Some(answer) => answer,
            None => CallAnswer::from_core(part).expect("core effect status"),
        },
        jig_domain_core::CallPart::Connector { .. }
        | jig_domain_core::CallPart::EffectDenied { .. }
        | jig_domain_core::CallPart::ToolDenied { .. }
        | jig_domain_core::CallPart::EscalationDecided { .. }
        | jig_domain_core::CallPart::EscalationRefused(_)
        | jig_domain_core::CallPart::Proposed { .. }
        | jig_domain_core::CallPart::ProposalDecided { .. }
        | jig_domain_core::CallPart::ProposalRefused(_)
        | jig_domain_core::CallPart::Again { .. }
        | jig_domain_core::CallPart::PassedUp { .. }
        | jig_domain_core::CallPart::Revised { .. }
        | jig_domain_core::CallPart::Moved { .. }
        | jig_domain_core::CallPart::Controlled
        | jig_domain_core::CallPart::EffectRefused(_)
        | jig_domain_core::CallPart::ControlRefused(_)
        | jig_domain_core::CallPart::ControlDenied { .. }
        | jig_domain_core::CallPart::Sent { .. }
        | jig_domain_core::CallPart::Introduced
        | jig_domain_core::CallPart::MessageRefused(_)
        | jig_domain_core::CallPart::Subscribed { .. }
        | jig_domain_core::CallPart::Unsubscribed
        | jig_domain_core::CallPart::SubscriptionRefused(_)
        | jig_domain_core::CallPart::Delegated(_)
        | jig_domain_core::CallPart::DelegationDenied { .. }
        | jig_domain_core::CallPart::DelegationRefused(_)
        | jig_domain_core::CallPart::NoteWritten { .. }
        | jig_domain_core::CallPart::NoteRecalled { .. }
        | jig_domain_core::CallPart::NoteRefused(_)
        | jig_domain_core::CallPart::SettingsRefused(_)
        | jig_domain_core::CallPart::Unavailable => CallAnswer::from_core(part).expect("core owns its answer"),
    }
}

/// The root only translates each handoff that the core selected.
#[expect(clippy::too_many_lines, reason = "one exhaustive core effect handoff translator")]
pub(crate) fn effect_ask(domain: &mut Domain, env: &Env<Limits>, number: u16, ask: jig_domain_core::connector::Ask) {
    match ask {
        // A person settles a held uncertain effect (brief D14; jig's
        // `domain/connectors.md`, 4.3): again remakes it, a creation looked
        // for first; otherwise it is recorded not made.
        jig_domain_core::connector::Ask::ResolveUncertain { entry, again } => {
            if number == domain.config.forge_connector {
                domain.work.push(Work::Forge(forge::Event::Resolve { entry, again }));
            }
        }
        jig_domain_core::connector::Ask::Describe { owner } => {
            let description = match domain.forge_effects.get(&owner) {
                Some(
                    EffectFlight::Call { description, .. }
                    | EffectFlight::Procedure { description, .. }
                    | EffectFlight::Projection { description, .. },
                ) => description.clone(),
                None => return,
            };
            domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                jig_domain_core::connector::Event::Described { owner, description },
            )));
        }
        jig_domain_core::connector::Ask::DescribeProposal { owner, proposal } => {
            let flight = match domain.forge.proposed_effect(proposal) {
                Some(row) => {
                    let key = match domain.core.tasks.proposal(row.entry.task, proposal) {
                        Some(pending) => match pending.action {
                            tasks::ProposalAction::Effect { attempt, completion, position, .. } => {
                                CallKey { task: pending.proposer, attempt, completion, position }
                            }
                            tasks::ProposalAction::Batch(_)
                            | tasks::ProposalAction::ProcedureEffect { .. }
                            | tasks::ProposalAction::Amend { .. }
                            | tasks::ProposalAction::Widen { .. }
                            | tasks::ProposalAction::Settings { .. }
                            | tasks::ProposalAction::Release { .. }
                            | tasks::ProposalAction::ReleasePlaced { .. } => return,
                        },
                        None => return,
                    };
                    describe_agent(
                        domain,
                        env,
                        key,
                        row.entry.repository,
                        row.resource.clone(),
                        row.entry.effect.write.clone(),
                    )
                }
                None => Err(skein_forge::api::Error::Missing),
            };
            match flight {
                Ok(flight) => {
                    assert!(domain.forge_effects.insert(owner, flight).is_ok(), "fresh proposal description");
                    effect_ask(domain, env, number, jig_domain_core::connector::Ask::Describe { owner });
                }
                Err(why) => domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                    if why == skein_forge::api::Error::Busy {
                        jig_domain_core::connector::Event::DescribeBusy { owner }
                    } else {
                        jig_domain_core::connector::Event::DescribeRefused { owner }
                    },
                ))),
            }
        }
        jig_domain_core::connector::Ask::KeepProposal { owner, proposal, task, project: _ } => {
            match domain.forge_effects.remove(&owner) {
                Some(EffectFlight::Call { repository, effect, resource, .. }) => {
                    domain.work.push(Work::Forge(forge::Event::KeepProposedEffect {
                        row: forge::ProposedEffect {
                            number: proposal,
                            resource,
                            entry: skein_forge_client::Entry {
                                number: 0,
                                task,
                                repository,
                                effect,
                                start: None,
                                attempt: None,
                                failures: 0,
                                queued: false,
                            },
                        },
                    }));
                }
                Some(EffectFlight::Procedure { .. } | EffectFlight::Projection { .. }) | None => {
                    unreachable!("agent effect proposal was described")
                }
            }
        }
        jig_domain_core::connector::Ask::DropProposal { proposal } => {
            domain.work.push(Work::Forge(forge::Event::DropProposedEffect { number: proposal }));
        }
        jig_domain_core::connector::Ask::Keep { owner, entry, task, key: core_key } => {
            match domain.forge_effects.remove(&owner) {
                Some(EffectFlight::Call { repository, effect, key, description, .. }) => {
                    assert!(
                        core_key
                            == jig_domain_core::EffectKey {
                                deployment: domain.core.counters.deployment().id,
                                task: key.task,
                                origin: jig_domain_core::EffectPurpose::Call {
                                    attempt: key.attempt,
                                    completion: key.completion,
                                    position: key.position,
                                },
                                purpose: description.purpose,
                            },
                        "core preserves the connector's original call purpose"
                    );
                    assert!(
                        domain.forge_effecting.insert(entry, (ReplyTo::new(owner), key)).is_ok(),
                        "admitted connector keep"
                    );
                    domain.work.push(Work::Forge(forge::Event::Enqueue {
                        entry: skein_forge_client::Entry {
                            number: entry,
                            task,
                            repository,
                            effect,
                            start: None,
                            attempt: None,
                            failures: 0,
                            queued: false,
                        },
                    }));
                }
                Some(EffectFlight::Projection { row, entry: kept, .. }) => {
                    assert!(kept.number == entry && kept.task == task, "admitted projection entry");
                    domain.work.push(Work::Forge(forge::Event::KeepProjection { row: *row, entry: kept }));
                }
                Some(EffectFlight::Procedure { entry: kept, .. }) => {
                    assert!(kept == entry, "core releases the described procedure entry");
                }
                None => {}
            }
        }
        jig_domain_core::connector::Ask::Drop { owner, answer } => match domain.forge_effects.remove(&owner) {
            Some(EffectFlight::Procedure { entry, task, evidence, .. }) => {
                domain.work.push(Work::Forge(forge::Event::VetoChange { task, entry, prior: evidence.prior }));
                if answer == authority::Answer::Wait {
                    domain.forge.wait_change(&super::environment_forge(env), task, None);
                } else if answer != authority::Answer::Refuse
                    || domain.core.tasks.waiting_effect_problem(task).is_none()
                {
                    domain.work.push(Work::Tasks(tasks::Event::Hold { task, why: tasks::Hold::Effects }));
                }
            }
            Some(EffectFlight::Projection { row, .. }) => {
                let closing = match &row.desired {
                    Some(view) => view.finished.is_some(),
                    None => false,
                };
                domain.work.push(Work::Forge(forge::Event::FailProjection {
                    goal: row.goal,
                    repository: row.repository,
                    closing,
                }));
            }
            Some(EffectFlight::Call { .. }) | None => {}
        },
        jig_domain_core::connector::Ask::Judge { owner, judge, state, resources: _ } => {
            let (verdict, guarded) = judge_effect(domain, env, owner, number, judge, state);
            domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
                jig_domain_core::connector::Event::Verdict { owner, judge, verdict, at: env.wall, guarded, state },
            )));
        }
        jig_domain_core::connector::Ask::Names { .. }
        | jig_domain_core::connector::Ask::Unname { .. }
        | jig_domain_core::connector::Ask::Hold { .. }
        | jig_domain_core::connector::Ask::ReleaseHold { .. }
        | jig_domain_core::connector::Ask::Writer { .. }
        | jig_domain_core::connector::Ask::Make { .. }
        | jig_domain_core::connector::Ask::ProcedureMade { .. }
        | jig_domain_core::connector::Ask::ProcedureActivate { .. }
        | jig_domain_core::connector::Ask::ProcedureMessage { .. }
        | jig_domain_core::connector::Ask::ProcedureClose { .. }
        | jig_domain_core::connector::Ask::Project { .. }
        | jig_domain_core::connector::Ask::Release { .. }
        | jig_domain_core::connector::Ask::Subscribe { .. }
        | jig_domain_core::connector::Ask::Unsubscribe { .. }
        | jig_domain_core::connector::Ask::Read { .. }
        | jig_domain_core::connector::Ask::Gather { .. }
        | jig_domain_core::connector::Ask::CutTo { .. }
        | jig_domain_core::connector::Ask::Take { .. }
        | jig_domain_core::connector::Ask::Prepare { .. }
        | jig_domain_core::connector::Ask::Left { .. }
        | jig_domain_core::connector::Ask::Adopt { .. }
        | jig_domain_core::connector::Ask::Restore { .. }
        | jig_domain_core::connector::Ask::ReadAfresh
        | jig_domain_core::connector::Ask::SettleOutbox => unreachable!("core effect handoff"),
    }
}

fn judge_effect(
    domain: &Domain,
    env: &Env<Limits>,
    owner: Token,
    number: u16,
    judge: authority::Judge,
    state: [u8; 32],
) -> (authority::Verdict, bool) {
    if number != domain.config.forge_connector {
        return (authority::Verdict::Wait, false);
    }
    match domain.forge_effects.get(&owner) {
        Some(EffectFlight::Procedure { task, evidence, description, .. }) => {
            let Some(row) = domain.forge.change(*task) else { return (authority::Verdict::Refuse, false) };
            let Some(repository) = domain.forge.repository(row.repository) else {
                return (authority::Verdict::Refuse, false);
            };
            let Some(reviewers) = landing_reviewers(domain, env, row, evidence) else {
                return (authority::Verdict::Refuse, false);
            };
            let guarded = description.effect.kind == 4
                && domain.forge.guards_landing(repository.project, judge.requirement, judge.parameters);
            let verdict = domain.forge.judge_landing(
                repository.project,
                judge.requirement,
                judge.parameters,
                *task,
                state,
                evidence,
                &reviewers,
            );
            let verdict = match verdict {
                Some(forge::JudgeVerdict::Met) => authority::Verdict::Met,
                Some(forge::JudgeVerdict::Wait) | None => authority::Verdict::Wait,
                Some(forge::JudgeVerdict::Refuse) => authority::Verdict::Refuse,
            };
            (verdict, guarded)
        }
        Some(EffectFlight::Call { .. } | EffectFlight::Projection { .. }) | None => (authority::Verdict::Wait, false),
    }
}

/// Translate reported heads only against the assignment's granted write destinations.
pub(crate) fn left(domain: &mut Domain, env: &Env<Limits>, task: u64, attempt: u64, pushed: Box<[forge::Pushed]>) {
    let names = domain.forge.reported_heads(&env.limits.forge, task, attempt, &pushed);
    domain.work.push(Work::Forge(forge::Event::Answered { task, attempt, pushed: names }));
    domain.work.push(Work::Forge(forge::Event::Lost { task, attempt }));
}

fn report_resource(domain: &mut Domain, env: &Env<Limits>, project: u32, name: &forge::Name) {
    if let Some((role, hold)) = domain.forge.resource_facts(name) {
        route_resource(domain, env, project, name, role, hold);
    }
}

fn route_resource(
    domain: &mut Domain,
    env: &Env<Limits>,
    project: u32,
    name: &forge::Name,
    role: forge::Access,
    hold: forge::resources::HoldKind,
) {
    let Some(name) = hub_name(domain.config.forge_connector, name, &env.limits.tasks) else { return };
    let role = match role {
        forge::Access::Owned => jig_domain_core::connector::ResourceRole::Owned,
        forge::Access::Participant => jig_domain_core::connector::ResourceRole::Participant,
        forge::Access::Context => jig_domain_core::connector::ResourceRole::Context,
        forge::Access::Unavailable => jig_domain_core::connector::ResourceRole::Unavailable,
    };
    let hold = match hold {
        forge::resources::HoldKind::Shared => jig_domain_core::connector::HoldKind::Shared,
        forge::resources::HoldKind::Exclusive { wait } => jig_domain_core::connector::HoldKind::Exclusive { wait },
    };
    domain.work.push(Work::Core(jig_domain_core::Event::EffectConnector(
        jig_domain_core::connector::Event::Resource { project, name, role, hold },
    )));
}
