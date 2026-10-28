//! Synchronous kit calls replayed against a command's object cache and answer log.
use crate::{Transport, check::Credential, connection::Connection};
use skein_fake_checkout::git::{Created, Fault, Pushed, Remote, Tree, Want};
use skein_fake_forge_domain::api;
use skein_fake_forge_protocol::{
    Limits,
    git::{self, Answer, Call, client},
};
use skein_lib::Time;
use std::{cell::RefCell, collections::BTreeMap};

/// A command cannot make more than this many runs, including its final run.
/// This bounds a faulty peer or an unexpectedly long graph without repeating a write.
pub const MAX_RUNS: u32 = 1024;

/// No write or retained command output carries this id: the run with a miss is
/// thrown away before its changed checkout or output is kept. A merge must still
/// return Some here because the kit's `commit_merging` asserts that it always
/// records both parents, including for an unchanged tree.
const PLACEHOLDER: u64 = u64::MAX;

#[derive(Debug)]
struct Logged {
    call: Call,
    answer: Result<Answer, api::Error>,
    authenticated: bool,
}
/// The first call a run lacked, admitted by the encoder before it is kept.
#[derive(Debug, PartialEq, Eq)]
pub enum Missing {
    None,
    Call(Call),
    Unsendable,
}
/// One command's immutable objects and ordered answers. No cached object lives
/// past the command; each run starts at cursor zero, while writes enter the log once.
#[derive(Debug)]
pub struct Route {
    limits: Limits,
    objects: BTreeMap<u64, git::Object>,
    parents: BTreeMap<u64, git::Parents>,
    log: Vec<Logged>,
    cursor: usize,
    missing: RefCell<Missing>,
    authenticated: bool,
}
impl Route {
    /// The route's encoding/decoding limits, held for this whole command.
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            objects: BTreeMap::new(),
            parents: BTreeMap::new(),
            log: Vec::new(),
            cursor: 0,
            missing: RefCell::new(Missing::None),
            authenticated: false,
        }
    }
    fn miss(&self, call: Call) {
        let mut missing = self.missing.borrow_mut();
        if matches!(*missing, Missing::None) {
            *missing = match client::encode(&call, &self.limits) {
                Ok(_) => Missing::Call(call),
                Err(_) => Missing::Unsendable,
            };
        }
    }
    fn logged(&mut self, call: Call) -> Option<Result<Answer, api::Error>> {
        if !matches!(*self.missing.borrow(), Missing::None) {
            return None;
        }
        if let Some(logged) = self.log.get(self.cursor) {
            assert_eq!(call, logged.call, "replay asked {call:?}; logged {:?}", logged.call);
            self.cursor += 1;
            self.authenticated = logged.authenticated;
            Some(logged.answer.clone())
        } else {
            self.miss(call);
            None
        }
    }
    fn object(&self, commit: u64) -> Option<&git::Object> {
        let object = self.objects.get(&commit);
        if object.is_none() {
            self.miss(Call::Objects { commits: Box::new([commit]) });
        }
        object
    }
    fn parents(&self, commit: u64) -> Option<git::Parents> {
        if let Some(object) = self.objects.get(&commit) {
            return Some(git::Parents { commit, parent: object.parent, merge_parent: object.merge_parent });
        }
        if let Some(parents) = self.parents.get(&commit) {
            return Some(*parents);
        }
        self.miss(Call::Objects { commits: Box::new([commit]) });
        None
    }
    fn faulting(&mut self, call: Call) -> Result<Answer, Fault> {
        self.logged(call).unwrap_or(Err(api::Error::Refused)).map_err(super::transport::fault)
    }
    /// Begin another run, retaining remote answers while discarding the last run's cursor.
    pub fn replay(&mut self) {
        self.cursor = 0;
        *self.missing.borrow_mut() = Missing::None;
    }
    /// Keep the remote's answer once. Objects fill the cache; other calls enter
    /// the ordered log, including refusals and whether the identity was known.
    pub fn receive(&mut self, call: Call, answer: Result<Answer, api::Error>, authenticated: bool) {
        if let Ok(Answer::Objects { objects, graph }) = answer {
            for object in objects {
                self.objects.insert(object.commit, object);
            }
            for parents in graph {
                self.parents.insert(parents.commit, parents);
            }
        } else {
            self.log.push(Logged { call, answer, authenticated });
        }
    }
}
impl Remote for Route {
    fn heads(&mut self, remote: &[u8]) -> Result<Vec<u64>, Fault> {
        let Answer::Cloned { branches, .. } = self.faulting(Call::Clone { repository: repository(remote) })? else {
            panic!("clone answer");
        };
        Ok(branches.iter().map(|b| b.commit).collect())
    }
    fn fetch(&mut self, remote: &[u8], want: Want<'_>) -> Result<u64, Fault> {
        let want = match want {
            Want::Branch(branch) => api::Want::Branch(branch.into()),
            Want::Commit(commit) => api::Want::Commit(commit),
            Want::Default => api::Want::Default,
        };
        let Answer::Commit(commit) = self.faulting(Call::Fetch { repository: repository(remote), want })? else {
            panic!("fetch answer");
        };
        Ok(commit)
    }
    fn create(&mut self, remote: &[u8], branch: &[u8], commit: u64) -> Result<Created, Fault> {
        let Answer::Created(created) =
            self.faulting(Call::Create { repository: repository(remote), branch: branch.into(), commit })?
        else {
            panic!("create answer");
        };
        Ok(match created {
            api::Created::Created => Created::Created,
            api::Created::Exists => Created::Exists,
        })
    }
    fn push(&mut self, remote: &[u8], branch: &[u8], commit: u64, expected: Option<u64>) -> Result<Pushed, Fault> {
        let Answer::Pushed(pushed) =
            self.faulting(Call::Push { repository: repository(remote), branch: branch.into(), commit, expected })?
        else {
            panic!("push answer");
        };
        Ok(match pushed {
            api::Pushed::Pushed => Pushed::Pushed,
            api::Pushed::Rejected => Pushed::Rejected,
        })
    }
    fn parent(&self, commit: u64) -> Option<u64> {
        self.parents(commit).and_then(|p| p.parent)
    }
    fn merge_parent(&self, commit: u64) -> Option<u64> {
        self.parents(commit).and_then(|p| p.merge_parent)
    }
    fn tree(&self, commit: u64) -> Tree {
        self.object(commit)
            .map_or_else(Tree::new, |o| o.tree.iter().map(|f| (f.path.to_vec(), f.content.to_vec())).collect())
    }
    fn message(&self, commit: u64) -> Vec<u8> {
        self.object(commit).map_or_else(Vec::new, |o| o.message.to_vec())
    }
    fn store(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> Option<u64> {
        let tree = files(tree);
        let call = match merging {
            Some(merge_parent) => Call::Merge { parent, merge_parent, tree, message: message.into() },
            None => Call::Commit { parent, tree, message: message.into() },
        };
        match self.logged(call) {
            Some(Ok(Answer::Stored(commit))) => commit,
            None => merging.map(|_| PLACEHOLDER),
            other => panic!("store decoded {other:?}"),
        }
    }
}
impl Transport for Route {
    fn authenticate(&mut self, credential: Option<&Credential>) {
        self.authenticated = credential.is_some();
    }
    fn authenticated(&self) -> bool {
        self.authenticated
    }
    fn branch(&mut self, remote: &[u8], branch: &[u8]) -> Option<u64> {
        match self.logged(Call::Branch { repository: repository(remote), branch: branch.into() }) {
            Some(Ok(Answer::Commit(commit))) => Some(commit),
            Some(Err(api::Error::Missing(api::What::Branch))) | None => None,
            other => panic!("branch decoded {other:?}"),
        }
    }
    fn is_ancestor(&mut self, ancestor: u64, commit: u64) -> bool {
        match self.logged(Call::Ancestor { ancestor, commit }) {
            Some(Ok(Answer::Ancestor(answer))) => answer,
            None => false,
            other => panic!("ancestor decoded {other:?}"),
        }
    }
    fn commit_tree(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> u64 {
        match self.logged(Call::CommitTree { parent, merging, tree: files(tree), message: message.into() }) {
            Some(Ok(Answer::Stored(Some(commit)))) => commit,
            None => PLACEHOLDER,
            other => panic!("commit-tree decoded {other:?}"),
        }
    }
    fn set_time(&mut self, _now: Time) {}
    fn missed(&mut self) -> Missing {
        self.missing.replace(Missing::None)
    }
    fn connection(&self) -> Option<Connection> {
        Some(Connection::new(http_limits(&self.limits), self.limits.documents.body_bytes))
    }
    fn begin(&mut self) {
        self.replay();
    }
    fn route_limits(&self) -> Option<Limits> {
        Some(self.limits)
    }
    fn receive(&mut self, call: Call, answer: Result<Answer, api::Error>, authenticated: bool) {
        Self::receive(self, call, answer, authenticated);
    }
}
fn files(tree: Tree) -> Box<[api::File]> {
    tree.into_iter().map(|(path, content)| api::File { path: path.into(), content: content.into() }).collect()
}
fn repository(url: &[u8]) -> Box<[u8]> {
    let rest = url.strip_prefix(b"https://").or_else(|| url.strip_prefix(b"http://")).expect("admitted remote scheme");
    let name = rest.splitn(2, |b| *b == b'/').nth(1).unwrap_or(b"");
    name.strip_suffix(b".git").unwrap_or(name).into()
}
fn http_limits(limits: &Limits) -> skein_http::client::Limits {
    skein_http::client::Limits {
        request: limits.http.head,
        head: limits.http.response,
        headers: limits.http.headers,
        read: limits.http.read,
        send: limits.http.send,
    }
}
