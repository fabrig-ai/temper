//! Workspaces: what the top level keeps of each workspace the host asks for,
//! from its `Prepare` to the checkout's `Released`, so that the host's names
//! and the checkout's meet. The host names a workspace by the token the top
//! level gives it (its record's), and a push or a save by its own owner; the
//! checkout names its hold, echoing the record's token as its client; and io
//! names the directory the workspace is in, where an agent is spawned.
//!
//! A run cancelled as its workspace is prepared abandons the prepare by
//! releasing the hold: the checkout cancels its operation in flight, and ends
//! the prepare aborted once that has ended, releasing the hold with it, and the
//! host is told the workspace was not prepared, with nothing of it held. A
//! prepare that failed keeps its hold, which the top level releases at once,
//! as the host is told nothing is held then too.
//!
//! The transition table. Every other cell is unreachable by the contracts: the
//! checkout's (a `Held` at once for an admitted prepare, then one `Prepared`;
//! one `Pushed` or `Saved` for each push or save; one `Released` for each
//! release, once nothing touches the workspace) and the host's (a start, a
//! push, a save or a release only of a workspace prepared, one push or save in
//! flight, and a release once nothing is).
//!
//! ```text
//! state      event                          next        emits
//! -          prepare (host)                 Preparing   prepare (checkout)
//! Preparing  held                           Preparing
//!            abort (host)                   Preparing   release (checkout): abandoned
//!            prepared: ready                Ready       prepared (host)
//!            prepared: refused              Closed      unprepared (host)
//!            prepared: failed               Releasing   release (checkout), unprepared (host)
//!            prepared: aborted, abandoned   Releasing   unprepared (host)
//! Ready      start (host)                   Ready       spawn (agent), in its directory
//!            push, save (host)              Ready       push, save (checkout)
//!            pushed, saved                  Ready       pushed, saved (host)
//!            release (host)                 Releasing   release (checkout)
//! Releasing  released                       Closed
//! ```
//!
//! What a transition asks of the child domains is routed once the record has
//! moved, and a record is retired as it closes, in one place ([`follow`]). It
//! closes in the step its run's answer is made in, or before: the host
//! releases a workspace with nothing under way, which the checkout releases at
//! once, and so is one whose prepare has ended.

use alloc::boxed::Box;
use core::mem;

use crate::wire;
use jig_domain_host as host;
use skein_lib::bytes::copy_of;
use skein_lib::{Env, Id, List, Map, Token};
use smith_host_domain as agent;
use temper_worker_domain_checkout as checkout;

use crate::domain::Domain;
use crate::limits::Limits;
use crate::route;
use crate::translate;

/// A run's workspace items and what became of them (jig's
/// `protocol/channel.md`, 6.2): the hub names them by the record's token, as
/// its workspace's items, what the run left and saved, and the resource a
/// prepare failed on. Kept from the assignment until the engine has the
/// run's answer, as the hub keeps the answer, so the parts go up beside
/// every copy of it.
#[derive(Debug)]
pub(crate) struct Items {
    workstream: u64,
    spec: Option<wire::Workspace>,
    tags: Box<[u64]>,
    from_saved: Box<[bool]>,
    landed: Map<u64, [u8; 32]>,
    saved: Option<Box<[wire::SavedRepository]>>,
    delivery: Option<agent::Delivery>,
    preparation: Option<wire::Preparation>,
}

/// Stages an engine assignment's workspace items at the root, and gives the
/// hub its assignment, naming them: no workspace when it has no repository,
/// and a save only where there is a workspace to save, which did not start
/// from a merge in progress (`domain/worker.md`, 4.2, step 7). Items come with
/// a workstream, which the caller checked. Refused as busy, keeping nothing,
/// when every record is taken: one for each of the hub's slots, a run hosted
/// or its answer kept, and one for an assignment the hub has yet to take or
/// refuse. Items go as soon as their answer is acknowledged or refused, so
/// none waits for the reclaim point.
pub(crate) fn stage(
    domain: &mut Domain,
    mut assignment: host::Assignment,
    mut items: wire::Workspace,
) -> Option<host::Assignment> {
    let run = &mut assignment.assignment;
    if items.repositories.is_empty() {
        run.workspace = None;
        run.save = false;
        return Some(assignment);
    }
    let workstream = run.workspace.expect("the root checked the items come with a workstream").workstream;
    let mut merging = false;
    for repository in &items.repositories {
        match repository.start {
            wire::Start::Merge { .. } => merging = true,
            wire::Start::Base { .. }
            | wire::Start::Branch { .. }
            | wire::Start::Commit { .. }
            | wire::Start::Saved { .. } => {}
        }
    }
    let mut saving = false;
    for repository in &mut items.repositories {
        let writable = match repository.access {
            wire::Access::Writable { .. } => true,
            wire::Access::ReadOnly => false,
        };
        if !run.save || merging || !writable {
            repository.save = None;
        }
        saving |= repository.save.is_some();
    }
    let count = u32::try_from(items.repositories.len()).expect("checked item count");
    let mut tags = List::with_capacity(count);
    let mut from_saved = List::with_capacity(count);
    for repository in &items.repositories {
        tags.push(repository.tag).expect("room for each item tag");
        let saved = match repository.start {
            wire::Start::Saved { .. } => true,
            wire::Start::Merge { .. }
            | wire::Start::Base { .. }
            | wire::Start::Branch { .. }
            | wire::Start::Commit { .. } => false,
        };
        from_saved.push(saved).expect("room for each item start");
    }
    let entry = Items {
        workstream,
        spec: Some(items),
        tags: tags.into_boxed(),
        from_saved: from_saved.into_boxed(),
        landed: Map::with_capacity(count),
        saved: None,
        delivery: None,
        preparation: None,
    };
    if domain.items.len() == domain.items.capacity() {
        return None;
    }
    let token = Token::new(domain.next_items);
    domain.next_items = domain.next_items.checked_add(1).expect("a u64 of items outlasts the worker");
    let old = domain.items.insert(token, entry).expect("checked for room above");
    assert!(old.is_none(), "each items' token is new");
    let old = domain.items_by_run.insert((run.run, run.attempt), token).expect("room for each staged run");
    assert!(old.is_none(), "the root dropped an attempt it hosts");
    run.workspace = Some(host::Workspace { workstream, items: token });
    run.save = saving;
    Some(assignment)
}

/// temper's parts of `answer`, matched by its tokens: what the run left on
/// the forge, what it saved, and for a workspace that could not be prepared,
/// which repository and why.
pub(crate) fn parts(domain: &Domain, answer: &host::Answer) -> (wire::Work, Option<wire::Preparation>) {
    let (work, preparation) = match &answer.ending {
        host::Ending::Refused(_) => (None, None),
        host::Ending::Ended { work, .. } | host::Ending::Parked { work } => (Some(work), None),
        host::Ending::Failed { failure, work, .. } => (Some(work), Some(failure)),
    };
    let mut landed = List::with_capacity(0);
    let mut saved = None;
    if let Some(work) = work {
        if let Some(left) = work.left {
            let items = domain.items.get(&left).expect("a run's items live to its answer");
            landed = List::with_capacity(items.landed.len());
            for (tag, commit) in &items.landed {
                landed.push(wire::Landed { tag: *tag, commit: *commit }).expect("room for each landed item");
            }
        }
        if let Some(at) = work.saved {
            let items = domain.items.get(&at).expect("a run's items live to its answer");
            saved.clone_from(&items.saved);
        }
    }
    let preparation = match preparation {
        Some(host::Failure::Unprepared(host::Preparation::Permanent { resource: Some(resource) })) => {
            let items = domain.items.get(resource).expect("a run's items live to its answer");
            items.preparation
        }
        Some(host::Failure::Unprepared(
            host::Preparation::Transient | host::Preparation::Permanent { resource: None },
        )) => Some(wire::Preparation::Transient),
        Some(host::Failure::Run(_) | host::Failure::Agent(_) | host::Failure::Cancelled(_)) | None => None,
    };
    (wire::Work { landed: landed.into_boxed(), saved }, preparation)
}

/// The engine is done with the answer of the run `run`'s attempt `attempt`,
/// or the answer was a refusal, which nothing acknowledges: its items go.
pub(crate) fn forget(domain: &mut Domain, run: Token, attempt: Token) {
    if let Some(token) = domain.items_by_run.remove(&(run, attempt)) {
        domain.items.remove(&token);
    }
}

pub(crate) fn smith_delivery(domain: &Domain, left: Token) -> agent::Delivery {
    domain.items.get(&left).expect("live delivery items").delivery.clone().expect("settled delivery")
}

#[derive(Debug)]
pub(crate) struct Workspace {
    /// The hosted run it is for: the host's token for the run.
    run: Token,
    items: Token,
    repositories: u32,
    /// Each repository's directory, what the agent calls it, and whether it
    /// may be written, in the assignment's order.
    roots: Box<[Root]>,
    conflicts: Box<[checkout::Conflicts]>,
    state: State,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub(crate) struct Root {
    name: Box<[u8]>,
    writable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum State {
    /// Its prepare is in flight: `hold` names it once the checkout has
    /// admitted it, and it is `abandoned` once its run is cancelled and the
    /// hold released.
    Preparing { hold: Option<Token>, abandoned: bool },
    /// Prepared in the directory io names `directory`. `asked` is the host's
    /// owner of the push or the save in flight, if there is one.
    Ready { hold: Token, directory: Token, asked: Option<Token> },
    /// Released, until the checkout says nothing touches it.
    Releasing,
    /// Terminal: holds nothing.
    Closed,
}

/// What a transition asks of the child domains: a record of the checkout's and
/// one of the host's, routed in that order once the record has moved.
#[derive(Debug)]
struct Then {
    checkout: Option<checkout::Event>,
    host: Option<host::Event>,
}

impl Then {
    const NOTHING: Then = Then { checkout: None, host: None };
}

// Entry points, one per record: look the workspace up, take its state out,
// run the cell's handler, follow from the new state.

/// The host asks for `workspace` to be prepared for its run `owner`.
pub(crate) fn prepare(domain: &mut Domain, env: &Env<Limits>, owner: Token, workspace: host::Workspace) {
    let items = workspace.items;
    let staged = domain.items.get_mut(&items).expect("the root staged the workspace items");
    let workstream = staged.workstream;
    let spec = staged.spec.take().expect("prepared once");
    let repositories = u32::try_from(spec.repositories.len()).expect("the root checked the repositories");
    let mut roots = List::with_capacity(repositories);
    for repository in &spec.repositories {
        let writable = match repository.access {
            wire::Access::ReadOnly => false,
            wire::Access::Writable { .. } => true,
        };
        roots.push(Root { name: copy_of(&repository.name), writable }).expect("room for every repository");
    }
    let preparing = State::Preparing { hold: None, abandoned: false };
    let record = Workspace {
        run: owner,
        items,
        repositories,
        roots: roots.into_boxed(),
        conflicts: Box::new([]),
        state: preparing,
    };
    let id = domain.workspaces.insert(record).expect("a workspace for every slot");
    let fresh = domain.preparing.insert(owner, id).expect("a prepare for every slot");
    assert!(fresh.is_none(), "a run's workspace is prepared once");
    let spec = translate::spec(workstream, spec);
    let then = Then { checkout: Some(checkout::Event::Prepare { client: id.token(), spec }), host: None };
    follow(domain, env, id, then);
}

/// The host abandons the prepare of its run `owner`.
pub(crate) fn abort(domain: &mut Domain, env: &Env<Limits>, owner: Token) {
    let id = *domain.preparing.get(&owner).expect("an abort is of a prepare in flight");
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let mut then = Then::NOTHING;
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Preparing { hold: Some(hold), abandoned: false } => abandon(hold, &mut then),
        State::Preparing { hold: None, .. } => unreachable!("an admitted prepare is held at once"),
        State::Preparing { hold: Some(_), abandoned: true } => unreachable!("a run is cancelled once"),
        State::Ready { .. } | State::Releasing | State::Closed => {
            unreachable!("the host abandons only a prepare in flight")
        }
    };
    follow(domain, env, id, then);
}

/// The host's push or save, the host's `owner`, of `workspace`.
pub(crate) fn write(domain: &mut Domain, env: &Env<Limits>, owner: Token, workspace: Token, write: Write) {
    let id = Id::<Workspace>::from_token(workspace);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let mut then = Then::NOTHING;
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Ready { hold, directory, asked: None } => asked(hold, directory, owner, write, &mut then),
        State::Ready { asked: Some(_), .. } => unreachable!("one push or save at a time"),
        State::Preparing { .. } | State::Releasing | State::Closed => {
            unreachable!("only a prepared workspace pushes or saves")
        }
    };
    follow(domain, env, id, then);
}

/// Save through the application's checkout using the branch kept with its items.
pub(crate) fn save(domain: &mut Domain, env: &Env<Limits>, owner: Token, workspace: Token) {
    write(domain, env, owner, workspace, Write::Save);
}

/// A push or a save, as the host asks it.
#[derive(PartialEq, Eq, Hash, Debug)]
pub(crate) enum Write {
    Push { title: Box<[u8]>, body: Box<[u8]> },
    Save,
}

/// The host is done with `workspace`.
pub(crate) fn release(domain: &mut Domain, env: &Env<Limits>, workspace: Token) {
    let id = Id::<Workspace>::from_token(workspace);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let mut then = Then::NOTHING;
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Ready { hold, directory: _, asked: None } => releasing(hold, &mut then),
        State::Ready { asked: Some(_), .. } => unreachable!("the host releases a workspace with nothing in flight"),
        State::Preparing { .. } | State::Releasing | State::Closed => {
            unreachable!("the host releases a workspace it was given, once")
        }
    };
    follow(domain, env, id, then);
}

/// The checkout admitted the prepare for `client`, and names its hold `hold`.
pub(crate) fn held(domain: &mut Domain, env: &Env<Limits>, client: Token, hold: Token) {
    let id = Id::<Workspace>::from_token(client);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Preparing { hold: None, abandoned } => State::Preparing { hold: Some(hold), abandoned },
        State::Preparing { hold: Some(_), .. } | State::Ready { .. } | State::Releasing | State::Closed => {
            unreachable!("a prepare is held once, as it is admitted")
        }
    };
    follow(domain, env, id, Then::NOTHING);
}

/// The prepare for `client` ended.
pub(crate) fn prepared(domain: &mut Domain, env: &Env<Limits>, client: Token, prepared: checkout::Prepared) {
    let id = Id::<Workspace>::from_token(client);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let items = record.items;
    let failure = match &prepared {
        checkout::Prepared::Refused { refusal } => Some(translate::checkout_refusal(*refusal)),
        checkout::Prepared::Failed { failure } => {
            let items = domain.items.get(&items).expect("the assignment lives through prepare");
            Some(translate::failure(*failure, &items.tags))
        }
        checkout::Prepared::Ready { .. } | checkout::Prepared::Aborted => None,
    };
    if let Some(failure) = failure {
        domain.items.get_mut(&items).expect("the assignment lives through prepare").preparation = Some(failure);
    }
    let owner = record.run;
    let mut then = Then::NOTHING;
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Preparing { hold, abandoned } => match prepared {
            checkout::Prepared::Ready { workspace, conflicts } => {
                record.conflicts = conflicts;
                ended(
                    owner,
                    client,
                    items,
                    hold,
                    abandoned,
                    checkout::Prepared::Ready { workspace, conflicts: Box::new([]) },
                    &mut then,
                )
            }
            checkout::Prepared::Refused { .. } | checkout::Prepared::Failed { .. } | checkout::Prepared::Aborted => {
                ended(owner, client, items, hold, abandoned, prepared, &mut then)
            }
        },
        State::Ready { .. } | State::Releasing | State::Closed => unreachable!("a prepare ends once"),
    };
    domain.preparing.remove(&owner);
    follow(domain, env, id, then);
}

/// The push or the save for `client` ended, with `outcome`.
pub(crate) fn wrote(domain: &mut Domain, env: &Env<Limits>, client: Token, outcome: checkout::Outcome, push: bool) {
    let id = Id::<Workspace>::from_token(client);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let repositories = record.repositories;
    let items = record.items;
    let landings = translate::landings(outcome, repositories);
    let next = domain.items.get_mut(&items).expect("workspace items live until the answer");
    let done = record_written(next, items, landings, push);
    let mut then = Then::NOTHING;
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Ready { hold, directory, asked: Some(owner) } => written(hold, directory, owner, done, &mut then),
        State::Ready { asked: None, .. } | State::Preparing { .. } | State::Releasing | State::Closed => {
            unreachable!("a push or a save ends once, as the workspace stays held")
        }
    };
    follow(domain, env, id, then);
}

/// Keep feedback by deployment tag before the host is told the write ended.
fn record_written(next: &mut Items, items: Token, landings: Box<[wire::Landing]>, push: bool) -> Written {
    if push {
        let mut changed = false;
        for (index, landing) in landings.iter().enumerate() {
            match landing {
                wire::Landing::Landed { commit } => {
                    changed = true;
                    let tag = *next.tags.get(index).expect("every item has a tag");
                    next.landed.insert(tag, *commit).expect("room for every item");
                }
                wire::Landing::Conflicted { .. }
                | wire::Landing::Explained { .. }
                | wire::Landing::Moved
                | wire::Landing::Failed
                | wire::Landing::Refused
                | wire::Landing::Unchanged => {}
            }
        }
        next.delivery = Some(crate::smith_delivery::landings(&landings));
        let outcome = translate::delivery_outcome(&translate::summarize(landings));
        Written::Delivered(host::Delivery { outcome, left: items, changed })
    } else {
        let mut saved = List::with_capacity(u32::try_from(next.tags.len()).expect("checked item count"));
        for (index, landing) in landings.into_iter().enumerate() {
            let tag = *next.tags.get(index).expect("every item has a tag");
            let from_saved = *next.from_saved.get(index).expect("every item has a start");
            saved.push(wire::SavedRepository { tag, from_saved, landing }).expect("room for every saved item");
        }
        next.saved = Some(saved.into_boxed());
        Written::Saved(items)
    }
}

/// The checkout released the workspace of `client`: nothing touches it.
pub(crate) fn released(domain: &mut Domain, env: &Env<Limits>, client: Token) {
    let id = Id::<Workspace>::from_token(client);
    let record = domain.workspaces.get_mut(id).expect("a workspace lives until it is released");
    let state = mem::replace(&mut record.state, State::Closed);
    record.state = match state {
        State::Releasing => State::Closed,
        State::Preparing { .. } | State::Ready { .. } | State::Closed => {
            unreachable!("a workspace is released once, after its release")
        }
    };
    follow(domain, env, id, Then::NOTHING);
}

/// What a record's new state implies, applied after every transition: a
/// Closed record is retired; then what the transition asks of the child domains
/// is routed, the checkout's first.
fn follow(domain: &mut Domain, env: &Env<Limits>, id: Id<Workspace>, then: Then) {
    let record = domain.workspaces.get(id).expect("a workspace lives until it is retired");
    let closed = match record.state {
        State::Closed => true,
        State::Preparing { .. } | State::Ready { .. } | State::Releasing => false,
    };
    if closed {
        domain.workspaces.retire(id);
    }
    let Then { checkout, host } = then;
    if let Some(event) = checkout {
        route::checkout_step(domain, env, event);
    }
    if let Some(event) = host {
        route::host_step(domain, env, event);
    }
}

// Cell handlers.

/// Preparing, abort: the hold is released, which ends the prepare aborted.
fn abandon(hold: Token, then: &mut Then) -> State {
    then.checkout = Some(checkout::Event::Release { hold });
    State::Preparing { hold: Some(hold), abandoned: true }
}

/// Ready, push or save: asked of the checkout, for the host's `owner`.
fn asked(hold: Token, directory: Token, owner: Token, write: Write, then: &mut Then) -> State {
    then.checkout = Some(match write {
        Write::Push { title, body } => checkout::Event::Push { hold, message: checkout::Message { title, body } },
        Write::Save => checkout::Event::Save { hold, message: translate::saved() },
    });
    State::Ready { hold, directory, asked: Some(owner) }
}

/// What a push or a save left, for the host.
enum Written {
    Delivered(host::Delivery),
    /// Saved: the token of the items that keep what the save did.
    Saved(Token),
}

/// Ready, pushed or saved: the host's `owner` is told what became of each
/// repository.
fn written(hold: Token, directory: Token, owner: Token, written: Written, then: &mut Then) -> State {
    then.host = Some(match written {
        Written::Delivered(delivery) => host::Event::Delivered { owner, delivery },
        Written::Saved(at) => host::Event::Saved { owner, at: Some(at) },
    });
    State::Ready { hold, directory, asked: None }
}

/// Ready, release.
fn releasing(hold: Token, then: &mut Then) -> State {
    then.checkout = Some(checkout::Event::Release { hold });
    State::Releasing
}

/// Preparing, prepared: ready, or the host told it was not prepared. A
/// prepare refused at the entrance holds nothing; one that failed holds its
/// workspace, released now; one abandoned released it already.
fn ended(
    owner: Token,
    client: Token,
    items: Token,
    hold: Option<Token>,
    abandoned: bool,
    prepared: checkout::Prepared,
    then: &mut Then,
) -> State {
    match prepared {
        checkout::Prepared::Ready { workspace: directory, conflicts } => {
            assert!(conflicts.is_empty(), "conflicts moved to the workspace record before the transition");
            assert!(!abandoned, "a released hold's prepare ends aborted");
            then.host = Some(host::Event::Prepared { owner, workspace: client });
            State::Ready { hold: hold.expect("a prepare that ran was held"), directory, asked: None }
        }
        checkout::Prepared::Refused { refusal } => {
            then.host = Some(unprepared(owner, translate::preparation(translate::checkout_refusal(refusal), items)));
            State::Closed
        }
        checkout::Prepared::Failed { failure } => {
            assert!(!abandoned, "a released hold's prepare ends aborted");
            let hold = hold.expect("a prepare that ran was held");
            then.checkout = Some(checkout::Event::Release { hold });
            let failure = match failure {
                checkout::Failure::Transient => host::Preparation::Transient,
                checkout::Failure::Missing { .. } | checkout::Failure::Refused { .. } => {
                    host::Preparation::Permanent { resource: Some(items) }
                }
            };
            then.host = Some(unprepared(owner, failure));
            State::Releasing
        }
        checkout::Prepared::Aborted => {
            assert!(abandoned, "only an abandoned prepare is aborted");
            then.host = Some(unprepared(owner, host::Preparation::Transient));
            State::Releasing
        }
    }
}

fn unprepared(owner: Token, failure: host::Preparation) -> host::Event {
    host::Event::Unprepared { owner, failure, detail: Box::new([]) }
}

/// Where the run's repositories sit, for smith's start: the directory io
/// prepared the workspace in, the root of each repository's, with the files
/// a merge in progress left in conflict. A directory's host path is for an
/// inline agent's workspace aliases (smith's `Directory`): smith's spawned
/// adapter attaches its own, so it is left empty.
pub(crate) fn directories(domain: &mut Domain, workspace: Token) -> (Token, Box<[agent::Directory]>) {
    let record = domain.workspaces.get_mut(Id::from_token(workspace)).expect("a workspace lives until released");
    let directory = match record.state {
        State::Ready { directory, .. } => directory,
        State::Preparing { .. } | State::Releasing | State::Closed => unreachable!("only a prepared workspace starts"),
    };
    let mut roots = List::with_capacity(record.repositories);
    for root in &record.roots {
        roots
            .push(agent::Directory {
                root: directory,
                name: copy_of(&root.name),
                host: Box::new([]),
                writable: root.writable,
                git: true,
                conflicts: Box::new([]),
            })
            .expect("one mount per repository");
    }
    for conflicts in mem::replace(&mut record.conflicts, Box::new([])) {
        roots.get_mut(conflicts.repository).expect("known repository").conflicts = conflicts.files;
    }
    (directory, roots.into_boxed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures;

    #[test]
    fn a_workspace_keeps_two_forges_repositories_with_one_id() {
        let limits = fixtures::limits();
        let mut domain = Domain::new(&limits);
        let tags = [temper_engine_domain_forge::tag(1, 1), temper_engine_domain_forge::tag(3, 1)];
        let mut items = fixtures::items(&[b"playground", b"work"]);
        for (repository, tag) in items.repositories.iter_mut().zip(tags) {
            repository.tag = tag;
        }
        let assignment = stage(&mut domain, fixtures::assignment(true, Box::new([])), items).expect("room");
        let workspace = assignment.assignment.workspace.expect("a workspace");
        let commits = [[2; 32], [4; 32]];
        for push in [true, false] {
            let next = domain.items.get_mut(&workspace.items).expect("retained workspace");
            let written = record_written(
                next,
                workspace.items,
                Box::new([wire::Landing::Landed { commit: commits[0] }, wire::Landing::Landed { commit: commits[1] }]),
                push,
            );
            match written {
                Written::Delivered(delivery) => {
                    assert!(push);
                    assert_eq!(delivery.left, workspace.items);
                    assert!(delivery.changed);
                }
                Written::Saved(at) => {
                    assert!(!push);
                    assert_eq!(at, workspace.items);
                }
            }
        }
        let answer = host::Answer {
            read: None,
            refused: Box::new([]),
            turns: 0,
            spent: 0,
            ending: host::Ending::Parked {
                work: host::Work { left: Some(workspace.items), saved: Some(workspace.items) },
            },
        };
        let (work, preparation) = parts(&domain, &answer);
        assert_eq!(preparation, None);
        assert_eq!(
            work.landed.as_ref(),
            &[wire::Landed { tag: tags[0], commit: commits[0] }, wire::Landed { tag: tags[1], commit: commits[1] }]
        );
        let saved = work.saved.expect("both repositories saved");
        assert_eq!(saved.len(), 2);
        for (index, repository) in saved.iter().enumerate() {
            assert_eq!(repository.tag, tags[index]);
            assert_eq!(repository.landing, wire::Landing::Landed { commit: commits[index] });
        }
    }
}
