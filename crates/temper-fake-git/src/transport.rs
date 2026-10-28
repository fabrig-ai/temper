//! Git reaches the same forge domain the HTTP service owns in the world.
use crate::{Missing, check::Credential};
use skein_fake_checkout::git::{Created, Fault, Pushed, Remote, Tree, Want, What};
use skein_fake_forge_domain::{self as domain, api};
use skein_fake_peers::forge::Forge;
use skein_lib::{Env, ReplyTo, Secret, Time, Wall};
use std::{cell::RefCell, rc::Rc};

/// The program's synchronous remote and store, including replay's missing call.
/// A transport preserves the kit's object graph and write promises; a run with
/// a missing call is discarded before any of its files or output are kept.
pub trait Transport: Remote {
    /// Start a fresh replay cursor before running the scanned checkout again.
    fn begin(&mut self) {}
    /// Encoding bounds for a missing route call; typed transports miss none.
    fn route_limits(&self) -> Option<skein_fake_forge_protocol::Limits> {
        None
    }
    /// Retain a missing call's terminal answer once, including an identity refusal.
    fn receive(
        &mut self,
        _call: skein_fake_forge_protocol::git::Call,
        _answer: Result<skein_fake_forge_protocol::git::Answer, api::Error>,
        _authenticated: bool,
    ) {
        unreachable!("a typed transport never misses a call");
    }

    /// Allocate a lazy connection for a route transport; typed calls need none.
    fn connection(&self) -> Option<crate::connection::Connection> {
        None
    }

    /// Select the identity for this run; keep its secret on the transport side.
    fn authenticate(&mut self, credential: Option<&Credential>);
    /// Whether the last repository refusal named a known identity.
    fn authenticated(&self) -> bool;
    /// Read the branch's current head, or no head when it is absent or refused.
    fn branch(&mut self, remote: &[u8], branch: &[u8]) -> Option<u64>;
    /// Ask whether the first commit is an ancestor of the second.
    fn is_ancestor(&mut self, ancestor: u64, commit: u64) -> bool;
    /// Store a fresh commit, even for an unchanged tree, with these exact parents.
    fn commit_tree(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> u64;
    /// Set the forge's input clock before the run makes any call.
    fn set_time(&mut self, now: Time);
    /// Take the first miss, including an encoding refusal; typed calls miss none.
    fn missed(&mut self) -> Missing;
}

/// The known tokens and logins stay solely on this side of the boundary.
#[derive(Debug, Clone)]
pub struct Typed {
    pub forge: Rc<RefCell<Forge>>,
    tokens: Rc<Vec<(Secret, u64)>>,
    users: Rc<Vec<(u64, Box<[u8]>)>>,
    user: Option<u64>,
}
impl Typed {
    /// Copy the protocol scenario's identity tables before it takes ownership.
    #[must_use]
    pub fn new(forge: Rc<RefCell<Forge>>, table: &skein_fake_forge_protocol::Config) -> Self {
        Self {
            forge,
            tokens: Rc::new(table.tokens.iter().cloned().collect()),
            users: Rc::new(table.users.iter().cloned().collect()),
            user: None,
        }
    }
    pub fn authenticate(&mut self, credential: Option<&Credential>) {
        self.user = credential.and_then(|(login, token)| {
            self.tokens.iter().find_map(|(known, user)| {
                (known == token && self.users.iter().any(|(id, name)| id == user && name == login)).then_some(*user)
            })
        });
    }
    #[must_use]
    pub const fn authenticated(&self) -> bool {
        self.user.is_some()
    }
    pub fn commit_tree(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> u64 {
        let mut forge = self.forge.borrow_mut();
        let config = forge.config;
        let files =
            tree.into_iter().map(|(path, content)| api::File { path: path.into(), content: content.into() }).collect();
        domain::commit_tree(&mut forge.domain, &config, parent, merging, files, message).expect("scenario commit room")
    }
    fn call(&mut self, remote: &[u8], operation: api::Git) -> Result<api::Answer, Fault> {
        let user = self.user.ok_or(Fault::Refused)?;
        let remote = repository(remote);
        let mut forge = self.forge.borrow_mut();
        assert!(forge.pending.room() >= domain::MAX_OUT, "reserve external domain output");
        let token = forge.next_reply();
        let env = Env { now: forge.now, wall: Wall::EPOCH, limits: forge.config };
        let Forge { domain, pending, .. } = &mut *forge;
        domain::step(
            domain,
            &env,
            domain::Event::Call {
                reply_to: ReplyTo::new(token),
                user,
                repository: remote.into(),
                op: api::Op::Git(operation),
            },
            pending,
        );
        assert!(pending.room() >= domain::MAX_OUT, "reserve external domain output");
        domain::fire(domain, &env, pending);
        forge.domain.reclaim();
        let mut terminal = None;
        for _ in 0..forge.pending.len() {
            let request = forge.pending.pop().expect("pending pass length");
            match request {
                domain::Request::Reply { to, result } => {
                    let replied = to.into_token();
                    if replied == token {
                        terminal = Some(result.map_err(fault));
                    } else {
                        forge.pending.push(domain::Request::Reply { to: ReplyTo::new(replied), result });
                    }
                }
                request @ domain::Request::Hook { .. } => forge.pending.push(request),
            }
        }
        terminal.expect("the world's git transport uses an immediate forge domain")
    }
    #[must_use]
    pub fn branch(&self, remote: &[u8], branch: &[u8]) -> Option<u64> {
        let forge = self.forge.borrow();
        match forge.domain.inspect(&forge.config, repository(remote), &api::Read::Branch { branch: branch.into() }) {
            Ok(api::Answer::Commit(commit)) => Some(commit),
            Ok(_) | Err(_) => None,
        }
    }
    #[must_use]
    pub fn is_ancestor(&self, ancestor: u64, commit: u64) -> bool {
        self.forge.borrow().domain.is_ancestor(ancestor, commit)
    }
}
impl Remote for Typed {
    fn heads(&mut self, remote: &[u8]) -> Result<Vec<u64>, Fault> {
        let api::Answer::Cloned { branches, .. } = self.call(remote, api::Git::Clone)? else {
            panic!("clone answer");
        };
        Ok(branches.iter().map(|head| head.commit).collect())
    }
    fn fetch(&mut self, remote: &[u8], want: Want<'_>) -> Result<u64, Fault> {
        let want = match want {
            Want::Branch(branch) => api::Want::Branch(branch.into()),
            Want::Commit(commit) => api::Want::Commit(commit),
            Want::Default => api::Want::Default,
        };
        let api::Answer::Commit(commit) = self.call(remote, api::Git::Fetch { want })? else {
            panic!("fetch answer");
        };
        Ok(commit)
    }
    fn create(&mut self, remote: &[u8], branch: &[u8], commit: u64) -> Result<Created, Fault> {
        let api::Answer::Branch(created) = self.call(remote, api::Git::Create { branch: branch.into(), commit })?
        else {
            panic!("create answer");
        };
        Ok(match created {
            api::Created::Created => Created::Created,
            api::Created::Exists => Created::Exists,
        })
    }
    fn push(&mut self, remote: &[u8], branch: &[u8], commit: u64, expected: Option<u64>) -> Result<Pushed, Fault> {
        let api::Answer::Pushed(pushed) =
            self.call(remote, api::Git::Push { branch: branch.into(), commit, expected })?
        else {
            panic!("push answer");
        };
        Ok(match pushed {
            api::Pushed::Pushed => Pushed::Pushed,
            api::Pushed::Rejected => Pushed::Rejected,
        })
    }
    fn parent(&self, commit: u64) -> Option<u64> {
        self.forge.borrow().domain.object(commit).expect("stored commit").parent
    }
    fn merge_parent(&self, commit: u64) -> Option<u64> {
        self.forge.borrow().domain.object(commit).expect("stored commit").merge_parent
    }
    fn tree(&self, commit: u64) -> Tree {
        self.forge
            .borrow()
            .domain
            .object(commit)
            .expect("stored commit")
            .tree
            .iter()
            .map(|(path, content)| (path.to_vec(), content.to_vec()))
            .collect()
    }
    fn message(&self, commit: u64) -> Vec<u8> {
        self.forge.borrow().domain.object(commit).expect("stored commit").message.to_vec()
    }
    fn store(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> Option<u64> {
        let mut forge = self.forge.borrow_mut();
        let config = forge.config;
        let files =
            tree.into_iter().map(|(path, content)| api::File { path: path.into(), content: content.into() }).collect();
        match merging {
            Some(second) => Some(
                domain::merge_commit(&mut forge.domain, &config, parent, second, files, message)
                    .expect("scenario commit room"),
            ),
            None => domain::commit(&mut forge.domain, &config, parent, files, message).expect("scenario commit room"),
        }
    }
}
fn repository(url: &[u8]) -> &[u8] {
    let rest = url.strip_prefix(b"https://").or_else(|| url.strip_prefix(b"http://")).expect("admitted URL");
    let slash = rest.iter().position(|b| *b == b'/').expect("repository URL path");
    rest[slash + 1..].strip_suffix(b".git").unwrap_or(&rest[slash + 1..])
}
pub(crate) fn fault(error: api::Error) -> Fault {
    match error {
        api::Error::Missing(api::What::Repository) => Fault::Missing(What::Repository),
        api::Error::Missing(api::What::Branch) => Fault::Missing(What::Branch),
        api::Error::Missing(api::What::Commit) => Fault::Missing(What::Commit),
        api::Error::Refused | api::Error::Forbidden | api::Error::Protected => Fault::Refused,
        api::Error::Unreachable | api::Error::Unavailable | api::Error::Timeout | api::Error::RateLimited { .. } => {
            Fault::Unreachable
        }
        api::Error::Missing(
            api::What::Item
            | api::What::Pull
            | api::What::Comment
            | api::What::Label
            | api::What::File
            | api::What::Page
            | api::What::Review
            | api::What::Job,
        )
        | api::Error::TooLarge
        | api::Error::Full
        | api::Error::Exists
        | api::Error::Circular
        | api::Error::NothingToMerge
        | api::Error::Empty
        | api::Error::Closed
        | api::Error::Stale
        | api::Error::AlreadyQueued(_)
        | api::Error::Conflict => panic!("unexpected git fault: {error:?}"),
    }
}

impl Transport for Typed {
    fn authenticate(&mut self, credential: Option<&Credential>) {
        Self::authenticate(self, credential);
    }
    fn authenticated(&self) -> bool {
        Self::authenticated(self)
    }
    fn branch(&mut self, remote: &[u8], branch: &[u8]) -> Option<u64> {
        Self::branch(self, remote, branch)
    }
    fn is_ancestor(&mut self, ancestor: u64, commit: u64) -> bool {
        Self::is_ancestor(self, ancestor, commit)
    }
    fn commit_tree(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> u64 {
        Self::commit_tree(self, parent, merging, tree, message)
    }
    fn set_time(&mut self, now: Time) {
        self.forge.borrow_mut().now = now;
    }
    fn missed(&mut self) -> Missing {
        Missing::None
    }
}
