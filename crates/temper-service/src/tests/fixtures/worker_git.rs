use crate::Worker;
use skein_io::{self as io, kernel};
use skein_lib::{Time, Token};
use temper_worker_protocol_git_world::io::{Below, Input, Invocation, Script};
pub const IDENTITY: u32 = 1;

pub struct WorkerStand {
    pub worker: Worker,
    pub below: Below,
    pub seed: u64,
}
impl WorkerStand {
    pub fn new() -> Self {
        let mut below = Below::new(Script::default());
        below.out.io = skein_lib::Queue::with_capacity(128);
        below.out.kernel = skein_lib::Queue::with_capacity(32);
        below.adopt_root(kernel::Fd::new(9));
        below.script.invocations.push_back(Invocation::captured("version"));
        Self { worker: super::worker(), below, seed: 7 }
    }
    pub fn turn(&mut self) -> bool {
        self.worker.iterate(self.worker.env.now, self.worker.env.wall);
        self.drain();
        self.input()
    }
    pub fn drain(&mut self) {
        for _ in 0..self.worker.io().len() {
            let request = self.worker.io().pop().unwrap();
            if let io::Request::Connect { .. } = request {
                continue;
            }
            self.below.out.io.push(request);
        }
        for _ in 0..self.worker.submitted().len() {
            self.below.out.kernel.push(self.worker.submitted().pop().unwrap());
        }
        self.below.drain();
    }
    pub fn input(&mut self) -> bool {
        match self.below.next(&mut self.seed) {
            Some(Input::Io(event)) => self.worker.up(self.worker.env.now, event),
            Some(Input::Kernel(complete)) => self.worker.complete(self.worker.env.now, complete).unwrap(),
            None => return false,
        }
        true
    }
    pub fn answer_until_spawn(&mut self, command: &[u8]) -> (Token, kernel::Spawn) {
        for _ in 0_u16..1000 {
            self.worker.iterate(self.worker.env.now, self.worker.env.wall);
            for _ in 0..self.worker.io().len() {
                let request = self.worker.io().pop().unwrap();
                if let io::Request::Spawn { owner, spawn } = request {
                    if let Some(word) = spawn.args.get(26)
                        && word.as_ref() == command
                    {
                        return (owner, spawn);
                    }
                    self.below.out.io.push(io::Request::Spawn { owner, spawn });
                } else if let io::Request::Connect { .. } = request {
                } else {
                    self.below.out.io.push(request);
                }
            }
            self.drain();
            self.input();
        }
        panic!("requested git command is spawned");
    }
}
pub fn started_worker() -> WorkerStand {
    let mut stand = WorkerStand::new();
    for _ in 0_u16..1000 {
        if !stand.turn() {
            assert!(stand.worker.git.ready);
            return stand;
        }
    }
    panic!("git start finishes");
}

pub fn github_record(login: &[u8], second_token: &[u8]) -> temper_protocol_worker_wire::v1::Identity {
    use skein_lib::{List, Secret};
    use temper_protocol_worker_wire::v1;
    let limits = super::wire_limits();
    let mut entries = List::with_capacity(2);
    for (account, account_login, token) in
        [(11, b"first".as_slice(), b"tok-other".as_slice()), (22, b"second".as_slice(), second_token)]
    {
        entries
            .push(
                v1::Entry::new(
                    &limits,
                    v1::EntryParts { account, account_login: account_login.into(), token: Secret::new(token.into()) },
                )
                .unwrap(),
            )
            .unwrap();
    }
    v1::Identity::new(
        &limits,
        v1::IdentityParts {
            kind: v1::Kind::Github(v1::Entries::new(&limits, v1::EntriesParts { entries }).unwrap()),
            login: login.into(),
            name: b"Temper App".as_slice().into(),
            email: b"app@example.test".as_slice().into(),
        },
    )
    .unwrap()
}

pub fn github_value(generation: u64, secret: skein_lib::Secret) -> jig_protocol_worker_link::Request {
    jig_protocol_worker_link::Request::Value {
        account: IDENTITY,
        generation,
        expires: Time::from_nanos(u64::MAX),
        value: jig_protocol_worker_link::GrantValue::Application { secret },
    }
}

pub fn github_assignment(account: Option<u64>) -> jig_protocol_worker_link::Request {
    use temper_protocol_worker_wire::{self as payload, v1};
    let jig_protocol_worker_link::Request::Down { down, mut parts } = super::repository_assignment() else {
        panic!("assignment")
    };
    let limits = super::wire_limits();
    let items = v1::Items::decode(&limits, &mut skein_lib::Reader::new(&parts[0].bytes)).unwrap().into_parts();
    let mut repositories = skein_lib::List::with_capacity(1);
    let mut repository = items.repositories.get(0).unwrap().clone().into_parts();
    repository.account = account;
    repositories.push(v1::Repository::new(&limits, repository).unwrap()).unwrap();
    parts[0].bytes =
        payload::items(&v1::Items::new(&limits, v1::ItemsParts { repositories, save: items.save }).unwrap());
    jig_protocol_worker_link::Request::Down { down, parts }
}

pub fn git_place() -> temper_worker_domain_checkout::git::Place {
    temper_worker_domain_checkout::git::Place {
        workspace: Token::new(91),
        repository: b"app".as_slice().into(),
        git: None,
    }
}
pub fn github_push() -> temper_worker_domain_checkout::git::Op {
    use temper_worker_domain_checkout::git;
    git::Op::Push {
        at: git_place(),
        remote: b"https://forge.example/acme/app.git".as_slice().into(),
        commit: git::Commit::new([2; 32]),
        branch: b"main".as_slice().into(),
        expected: None,
        identity: IDENTITY,
        account: Some(22),
    }
}

impl WorkerStand {
    pub fn route_git(&mut self, owner: Token, op: temper_worker_domain_checkout::git::Op) {
        self.worker.route(temper_worker_domain::Request::Io { owner, op, deadline: Time::from_nanos(u64::MAX) });
    }
    pub fn finish_git(
        &mut self,
        owner: Token,
        spawn: kernel::Spawn,
        transcript: &str,
    ) -> temper_worker_domain_checkout::git::Done {
        self.below.script.invocations.push_back(Invocation::captured(transcript));
        self.below.out.io.push(io::Request::Spawn { owner, spawn });
        for _ in 0_u16..1000 {
            self.drain();
            if let Some(temper_worker_domain::Event::Done { done, .. }) = self.worker.events.pop() {
                return done;
            }
            assert!(self.input(), "git reaches its terminal");
        }
        panic!("git settles");
    }
}
