//! One signed-in owner's `MakeProject` on the native client wire.
use jig_client::{self as client, contract as c};
use jig_client_shell_native::{Settings, Shell};
use skein_io::kernel::{Complete, Fd, Submit};
use skein_lib::{List, Map, Queue, Time, Token, Wall};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::path::Path;

const KEY: client::Key = client::Key { asker: client::Asker(u32::MAX), number: 1 };
const TAG: u64 = 0xfffd_0000_0000_0000;
/// A bounded setup client. The caller gives it its own parent descriptor.
struct MakeProject {
    client: client::Client,
    shell: Shell,
    out: Queue<client::Request>,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
    name: Option<Box<[u8]>>,
    loaded: bool,
    sent: bool,
    made: Option<u32>,
    closing: bool,
    memory: u64,
    tokens: Map<Token, Token>,
    serial: u32,
}
impl MakeProject {
    #[must_use]
    fn new(
        engine: std::net::SocketAddr,
        directory: Box<[u8]>,
        root: Fd,
        user: u32,
        seed: u64,
        name: Box<[u8]>,
    ) -> Self {
        let limits = temper_client_shell_terminal::limits::native();
        let bounds = limits.client.client;
        let mut shell = Shell::new(Settings {
            engine: skein_io::Addr::Ip(engine),
            directory,
            limits: limits.connections,
            io: limits.io,
        });
        shell.adopt(root, user, seed);
        let output_capacity = client::max_out(&bounds);
        let memory = Self::memory();
        Self {
            client: client::Client::new(&bounds, seed).expect("native project client"),
            shell,
            out: Queue::with_capacity(output_capacity),
            complete: Queue::with_capacity(1024),
            submit: Queue::with_capacity(1024),
            name: Some(name),
            loaded: false,
            sent: false,
            made: None,
            closing: false,
            memory,
            tokens: Map::with_capacity(1024),
            serial: 0,
        }
    }
    /// Reserve this before the world starts; heap ceilings are fixed at spawn.
    #[must_use]
    fn memory() -> u64 {
        let limits = temper_client_shell_terminal::limits::native();
        client::worst_case(&limits.client.client).expect("project client memory")
            + temper_client_shell_terminal::limits::shell(&limits).expect("native shell memory")
            + Queue::<client::Request>::worst_case(client::max_out(&limits.client.client)).expect("project outputs")
            + Queue::<Complete>::worst_case(1024).expect("project completions")
            + Queue::<Submit>::worst_case(1024).expect("project submissions")
            + Map::<Token, Token>::worst_case(1024).expect("native token bindings")
            + 128
    }
    /// Only a committed `ProjectMade` answer supplies this number.
    #[must_use]
    fn project(&self) -> u32 {
        assert!(self.is_empty(), "project setup drained");
        self.made.expect("ProjectMade")
    }
    fn event(&mut self, now: Time, wall: Wall, event: client::Event) {
        self.client.step(now, wall, event, &mut self.out);
    }
    fn outputs(&mut self) {
        while let Some(request) = self.out.pop() {
            if let client::Request::Told { told: client::Told::Answered { key, outcome } } = &request {
                assert_eq!(*key, KEY, "project setup key");
                let client::Outcome::Answered(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                    c::OwnerOutcome::ProjectMade { project },
                )))) = outcome
                else {
                    panic!("MakeProject refused: {outcome:?}")
                };
                self.made = Some(*project);
            }
            self.shell.down(request);
        }
    }
}
impl Host for MakeProject {
    fn iterate(&mut self, now: Time, wall: Wall) {
        while let Some(mut complete) = self.complete.pop() {
            complete.op = self.tokens.remove(&complete.op).expect("native operation binding");
            self.shell.completions().push(complete);
        }
        self.shell.iterate(now, wall);
        if !self.loaded
            && let Some((credential, records)) = self.shell.loaded()
        {
            self.loaded = true;
            let mut kept = List::with_capacity(u32::try_from(records.len()).expect("native records"));
            for record in records {
                kept.push(record).expect("counted records");
            }
            self.event(now, wall, client::Event::Start { kept });
            self.event(
                now,
                wall,
                client::Event::SignIn { with: client::With::Held(credential.expect("signed-in owner token")) },
            );
        }
        while let Some(event) = self.shell.events().pop() {
            self.event(now, wall, event);
        }
        if self.loaded && !self.sent && self.client.link().state == client::Link::Live {
            self.sent = true;
            let name = self.name.take().expect("one project name");
            self.event(
                now,
                wall,
                client::Event::Send {
                    key: KEY,
                    name: None,
                    request: client::Ask::Jig(Box::new(c::Ask::MakeProject {
                        name: c::Text::new(name, 128).expect("bounded project name"),
                        policy: 1,
                    })),
                },
            );
        }
        self.client.fire(now, wall, &mut self.out);
        self.client.reclaim();
        self.outputs();
        if self.made.is_some() && !self.closing {
            self.closing = true;
            self.shell.close();
        }
        self.shell.iterate(now, wall);
        while let Some(mut submit) = self.shell.submissions().pop() {
            self.serial += 1;
            let token = Token::new(TAG | u64::from(self.serial));
            self.tokens.insert(token, submit.op).expect("bounded native operations");
            submit.op = token;
            self.submit.push(submit);
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submit
    }
    fn work_pending(&self, now: Time) -> bool {
        !self.complete.is_empty()
            || self.shell.work_pending(now)
            || self.client.next_deadline().is_some_and(|at| at <= now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.shell.next_deadline().into_iter().chain(self.client.next_deadline()).min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.shell.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.closing && self.shell.is_empty() && self.submit.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        self.shell.operations()
    }
}

struct Judge {
    settled: bool,
    deadline: Time,
}
impl Referee<MakeProject> for Judge {
    fn act(&mut self, _now: Time, _hosts: &mut [MakeProject]) {}
    fn observe(&mut self, _now: Time, hosts: &[MakeProject]) {
        self.settled = hosts.iter().all(Host::is_empty);
    }
    fn next_deadline(&self) -> Option<Time> {
        Some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        if !self.settled && now >= self.deadline { Some("owner's `MakeProject` did not drain".into()) } else { None }
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
pub(super) fn make(directory: &Path) -> u32 {
    use std::os::unix::ffi::OsStrExt;
    let address = std::fs::read_to_string(directory.join("address")).expect("owner's engine address");
    let engine = address.trim().strip_prefix("http://").expect("loopback HTTP address").parse().expect("engine socket");
    let clock = Clock::new();
    let root = skein_shell::open_root(directory.parent().expect("case scratch")).expect("case parent root");
    let leaf = directory.file_name().expect("client directory").as_bytes().into();
    let mut world = real::World::new(Judge { settled: false, deadline: clock.now().now.saturating_add(super::WITHIN) });
    world.spawn_with_fds(vec![root], || {
        MakeProject::new(
            engine,
            leaf,
            root,
            skein_shell::effective_user(),
            temper_real_loop_world::fixtures::SEED,
            b"Project 1".as_slice().into(),
        )
    });
    let outcome = world.run(&clock, super::WITHIN);
    outcome.procs[0].project()
}
