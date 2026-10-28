//! A real checkout saves a repair, then the root assigns its retry from that tree.
use super::*;
use skein_lib::{Env, Queue, Wall};
use temper_fake_checkout::{
    Checkout,
    git::{Created, Fault, Pushed, Remote, Tree, Want},
};
use temper_worker_checkout_world::translate as checkout_io;
use temper_worker_domain_checkout as checkout;

const CHANNEL: Token = Token::new(7);
const CLIENT: Token = Token::new(901);
const ITERATIONS: u32 = 64; // Make, clone, fetches, checkout, commit, push and release, each with one terminal.
const FILE: &[u8] = b"changes/3.md";
const CONTENT: &[u8] = b"repair survives its unfinished attempt\n";
const REPAIR_ASSIGNMENT: usize = 2; // The requester and producer precede the repair.
const RETRY_ASSIGNMENT: usize = 3; // The repair's next attempt follows its first.
const SPENT: u64 = 1; // One committed turn in the unfinished repair.

struct Git<'a>(&'a mut World);
impl Git<'_> {
    fn call(&mut self, op: raw::Git) -> raw::Answer {
        self.0.fake.reclaim();
        let answer = self.0.external(1, raw::Op::Git(op));
        self.0.fake.reclaim();
        answer
    }
}
impl Remote for Git<'_> {
    fn heads(&mut self, _: &[u8]) -> Result<Vec<u64>, Fault> {
        let raw::Answer::Cloned { branches, .. } = self.call(raw::Git::Clone) else { panic!("clone") };
        Ok(branches.iter().map(|head| head.commit).collect())
    }
    fn fetch(&mut self, _: &[u8], want: Want<'_>) -> Result<u64, Fault> {
        let want = match want {
            Want::Branch(branch) => raw::Want::Branch(branch.into()),
            Want::Commit(commit) => raw::Want::Commit(commit),
            Want::Default => raw::Want::Default,
        };
        let raw::Answer::Commit(commit) = self.call(raw::Git::Fetch { want }) else { panic!("fetch") };
        Ok(commit)
    }
    fn create(&mut self, _: &[u8], branch: &[u8], commit: u64) -> Result<Created, Fault> {
        let raw::Answer::Branch(created) = self.call(raw::Git::Create { branch: branch.into(), commit }) else {
            panic!("create")
        };
        Ok(match created {
            raw::Created::Created => Created::Created,
            raw::Created::Exists => Created::Exists,
        })
    }
    fn push(&mut self, _: &[u8], branch: &[u8], commit: u64, expected: Option<u64>) -> Result<Pushed, Fault> {
        let raw::Answer::Pushed(pushed) = self.call(raw::Git::Push { branch: branch.into(), commit, expected }) else {
            panic!("push")
        };
        Ok(match pushed {
            raw::Pushed::Pushed => Pushed::Pushed,
            raw::Pushed::Rejected => Pushed::Rejected,
        })
    }
    fn parent(&self, commit: u64) -> Option<u64> {
        self.0.fake.object(commit).expect("commit").parent
    }
    fn merge_parent(&self, commit: u64) -> Option<u64> {
        self.0.fake.object(commit).expect("commit").merge_parent
    }
    fn tree(&self, commit: u64) -> Tree {
        self.0.fake.object(commit).expect("commit").tree.iter().map(|(p, c)| (p.to_vec(), c.to_vec())).collect()
    }
    fn message(&self, commit: u64) -> Vec<u8> {
        self.0.fake.object(commit).expect("commit").message.to_vec()
    }
    fn store(&mut self, parent: u64, merging: Option<u64>, tree: Tree, message: &[u8]) -> Option<u64> {
        let files =
            tree.into_iter().map(|(path, content)| raw::File { path: path.into(), content: content.into() }).collect();
        match merging {
            Some(second) => Some(
                fake::merge_commit(&mut self.0.fake, &self.0.fake_env.limits, parent, second, files, message)
                    .expect("merge store"),
            ),
            None => {
                fake::commit(&mut self.0.fake, &self.0.fake_env.limits, parent, files, message).expect("commit store")
            }
        }
    }
}

struct Workspace {
    domain: checkout::Domain,
    env: Env<checkout::Limits>,
    out: Queue<checkout::Request>,
    disk: Checkout,
    hold: Option<Token>,
}
impl Workspace {
    fn new() -> Self {
        let limits = temper_worker_checkout_world::LIMITS;
        Self {
            domain: checkout::Domain::new(&limits),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(checkout::MAX_OUT),
            disk: Checkout::new(),
            hold: None,
        }
    }
    fn finish(&mut self, world: &mut World, mut event: checkout::Event) -> checkout::Request {
        for _ in 0..ITERATIONS {
            checkout::step(&mut self.domain, &self.env, event, &mut self.out);
            let mut next = None;
            let mut terminal = None;
            while let Some(request) = self.out.pop() {
                match request {
                    checkout::Request::Held { hold, .. } => {
                        self.hold = Some(hold);
                    }
                    checkout::Request::Io { owner, op, .. } => {
                        let done = checkout_io::perform(&mut Git(world), &mut self.disk, op);
                        assert!(next.replace(checkout::Event::Done { owner, done }).is_none());
                    }
                    checkout::Request::Cancel { .. } => panic!("no cancellation"),
                    answer => {
                        assert!(terminal.replace(answer).is_none());
                    }
                }
            }
            while self.domain.pop_fact().is_some() {}
            self.domain.reclaim();
            if let Some(answer) = terminal {
                assert!(next.is_none());
                return answer;
            }
            event = next.expect("operation in flight");
        }
        panic!("checkout settles");
    }
    fn attempt(&mut self, world: &mut World, run: &engine::Assignment, unfinished: bool) -> engine::Parts {
        let repository = &run.workspace.repositories[0];
        let start = match &repository.start {
            engine::ForgeStart::Base(branch) => checkout::Start::Base { branch: branch.clone() },
            engine::ForgeStart::Branch(branch) => checkout::Start::Branch { branch: branch.clone() },
            engine::ForgeStart::Saved(branch) => checkout::Start::Saved { branch: branch.clone() },
            engine::ForgeStart::Merge { branch, base } => {
                checkout::Start::Merge { branch: branch.clone(), base: checkout::git::Commit::new(*base) }
            }
        };
        let spec = checkout::Spec {
            key: run.workstream.to_be_bytes().into(),
            repositories: Box::new([checkout::Repository {
                name: repository.name.clone(),
                remote: b"org/repo".as_slice().into(),
                git: None,
                start,
                push: repository.push.clone(),
                expected: None,
                landing: repository.landing.clone(),
                identity: 0,
                account: None,
                save: if run.run.policy.resume { repository.save.clone() } else { None },
            }]),
        };
        let checkout::Request::Prepared { prepared: checkout::Prepared::Ready { workspace, .. }, .. } =
            self.finish(world, checkout::Event::Prepare { client: CLIENT, spec })
        else {
            panic!("prepared")
        };
        if unfinished {
            let path =
                checkout_io::path(&checkout::git::Place { git: None, workspace, repository: repository.name.clone() });
            self.disk.write(&[path.as_slice(), b"/", FILE].concat(), CONTENT);
        }
        let hold = self.hold.take().expect("held");
        let message = checkout::Message { title: b"Repair CI".as_slice().into(), body: Box::new([]) };
        let event =
            if unfinished { checkout::Event::Save { hold, message } } else { checkout::Event::Push { hold, message } };
        let outcome = match self.finish(world, event) {
            checkout::Request::Saved { outcome, .. } | checkout::Request::Pushed { outcome, .. } => outcome,
            answer => panic!("write terminal: {answer:?}"),
        };
        let checkout::Outcome::Pushed { landings } = outcome else { panic!("write admitted") };
        let [checkout::Landing::Landed { commit }] = landings.as_ref() else { panic!("file is pushed: {landings:?}") };
        let parts = if unfinished {
            engine::Parts { saved: Some(Box::new([repository.tag])), pushed: Box::new([]), preparation: None }
        } else {
            engine::Parts {
                saved: None,
                pushed: Box::new([forge_top::Pushed { tag: repository.tag, commit: commit.raw() }]),
                preparation: None,
            }
        };
        self.finish(world, checkout::Event::Release { hold });
        assert_eq!(self.domain.holds(), 0);
        parts
    }
}

fn finish_change(world: &mut World, run: &engine::Assignment, words: &[u8], parts: engine::Parts) {
    let event = world.protocol.answer(
        CHANNEL,
        run.task,
        run.attempt,
        SPENT,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: words.into(),
            },
            cancel_delegates: false,
        },
        parts,
    );
    world.send(event);
}

#[test]
fn a_repair_that_wrote_and_ended_unfinished_finishes_with_its_file_on_retry() {
    let (mut world, _, producer, branch) = change_world(true, 1000, None);
    world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"needs repair", 1)
        .expect("producer head");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: head,
            context: b"CI".as_slice().into(),
            state: raw::Check::Failed,
        }),
    );
    finish_change(&mut world, &producer, b"pushed", engine::Parts::none());
    let repair = super::reviews::next_assignment(&mut world, REPAIR_ASSIGNMENT);
    let mut workspace = Workspace::new();
    let parts = workspace.attempt(&mut world, &repair, true);
    world.send(protocol::turn(CHANNEL, repair.task, repair.attempt, 1, SPENT, None, b"wrote changes/3.md"));
    let event = world.protocol.answer(
        CHANNEL,
        repair.task,
        repair.attempt,
        SPENT,
        tasks::End::Failed(tasks::Class::Run),
        parts,
    );
    world.send(event);
    let retry = super::reviews::next_assignment(&mut world, RETRY_ASSIGNMENT);
    assert_eq!(retry.task, repair.task);
    assert!(retry.attempt > repair.attempt);
    assert!(!retry.transcript.is_empty());
    let parts = workspace.attempt(&mut world, &retry, false);
    let pushed = parts.pushed[0].commit;
    finish_change(&mut world, &retry, b"repaired", parts);
    let object = world.fake.object(checkout_io::fake(checkout::git::Commit::new(pushed))).expect("pushed head");
    assert_eq!(object.tree.get(FILE).map(AsRef::as_ref), Some(CONTENT));
    let raw::Answer::Commit(head) = world.external(1, raw::Op::Read(raw::Read::Branch { branch })) else {
        panic!("branch head")
    };
    assert_eq!(checkout_io::commit(head).raw(), pushed);
}

#[test]
fn a_landed_change_waits_for_its_live_producers_terminal() {
    let (mut world, _, producer, branch) = change_world(false, 1000, None);
    let tasks::Party::Task(change) = live_task(&world, producer.task).requester else {
        panic!("producer belongs to a change")
    };
    world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"delivered", 1)
        .expect("producer head");
    let raw::Answer::Created(pull) = world.external(
        2,
        raw::Op::Write(raw::Write::OpenPull {
            opens_empty: false,
            title: b"change".as_slice().into(),
            body: Box::new([]),
            head: branch.clone(),
            base: b"main".as_slice().into(),
        }),
    ) else {
        panic!("opened pull")
    };
    assert!(matches!(
        world.external(1, raw::Op::Write(raw::Write::Merge { number: pull, head })),
        raw::Answer::Merged(_)
    ));
    world.send(engine::Event::Forge(Box::new(forge_top::Event::Hint {
        hint: skein_forge::api::Hint {
            repository: forge_world::REPO,
            change: skein_forge::api::Change::Branch(branch),
            key: None,
        },
    })));
    let mut landed = false;
    for _ in 0..200 {
        world.tick();
        landed = world.store.rows.values().any(|stored| matches!(stored,
            Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(row)
                if row.task == change && matches!(row.change.state, temper_engine_domain_forge_change::State::Landed { .. }))));
        if landed && live_task(&world, change).phase == tasks::Phase::Active(tasks::Active::Idle) {
            break;
        }
    }
    assert!(landed, "the forge is already landed before its live producer finishes");
    assert_eq!(live_task(&world, change).phase, tasks::Phase::Active(tasks::Active::Idle));
    assert_eq!(live_task(&world, change).delegates.as_ref(), &[producer.task]);
    finish_change(&mut world, &producer, b"delivered", engine::Parts::none());
    let ended = Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(change)));
    for _ in 0..200 {
        world.tick();
        if world.store.rows.contains_key(&ended) {
            break;
        }
    }
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row)))) = world.store.rows.get(&ended)
    else {
        panic!("the producer's typed terminal wakes and finishes the change")
    };
    assert!(matches!(row.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))));
}
