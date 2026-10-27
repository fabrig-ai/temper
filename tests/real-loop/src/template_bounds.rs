//! A private bounds fixture; scripted delegates need neither forge work nor git.
use crate::world::Proc::{Caller, Service};
use crate::{agent, calls, fixtures, llm, service::ServiceHost};
use serde_json::{Value, json};
use skein_io::kernel::{Fd, ServiceSignal};
use skein_lib::{Duration, Time};
use skein_shell::Host;
use skein_world::{
    HostedProgram, Referee,
    real::{Controls, World},
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

enum Proc {
    Llm { peer: Box<skein_fake_peers::llm::Peer>, release: bool, pending_streak: crate::patience::PendingStreak },
    Hosted(crate::world::Proc),
    Page { caller: Box<calls::Caller>, path: PathBuf, root: Fd, pending: Option<PageCommand> },
}
#[derive(Clone, Copy)]
enum PageCommand {
    Read { json: bool },
    Finish,
}
impl Proc {
    fn caller(&self) -> &calls::Caller {
        match self {
            Self::Hosted(Caller(caller)) | Self::Page { caller, .. } => caller,
            Self::Llm { .. } | Self::Hosted(_) => panic!("caller"),
        }
    }
    fn host(&self) -> &dyn Host {
        match self {
            Self::Llm { peer, .. } => peer.as_ref(),
            Self::Hosted(proc) => proc,
            Self::Page { caller, .. } => caller.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Llm { peer, .. } => peer.as_mut(),
            Self::Hosted(proc) => proc,
            Self::Page { caller, .. } => caller.as_mut(),
        }
    }
}
impl Host for Proc {
    fn iterate(&mut self, now: Time, wall: skein_lib::Wall) {
        if let Self::Hosted(proc) = self {
            proc.iterate(now, wall);
            return;
        }
        if let Self::Llm { peer, release, .. } = self
            && std::mem::take(release)
        {
            // The release name is allocated under the peer's own heap meter.
            peer.release(WATCH).expect("release root after task watch delivery");
        }
        if let Self::Page { caller, path, root, pending } = self
            && let Some(command) = pending.take()
        {
            // Build and drop the caller under this process's heap meter, not the referee's.
            let steps = match command {
                PageCommand::Read { json } => {
                    let mut argv = crate::argv!["--call", calls::file("bounds-call.json")];
                    if json {
                        argv.0.push(calls::Arg::Lit("--json"));
                    }
                    vec![calls::call(calls::D1, argv)]
                }
                PageCommand::Finish => vec![],
            };
            **caller = calls::Caller::new(steps, path.clone(), *root);
            caller.released = true;
        }
        let pending = self.host().work_pending(now);
        if let Self::Llm { pending_streak, .. } = self {
            pending_streak.observe(pending);
        }
        let host = self.host_mut();
        if pending
            || !host.completions().is_empty()
            || host.next_deadline().is_some_and(|deadline| deadline <= now)
            || host.next_policy_deadline().is_some_and(|deadline| deadline <= now)
        {
            host.iterate(now, wall);
        }
    }
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn completions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        matches!(self, Self::Page { pending: Some(_), .. } | Self::Llm { release: true, .. })
            || self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn exit(&self) -> Option<crate::Exit> {
        self.host().exit()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}

// Root 1 has ended report delegates 2 and 3, with 3 depending on 2;
// person task 4 and agents 5 and 6 waiting on 4 remain live.
pub const ROOT: u64 = 1;
pub const ENDED: u64 = 3;
pub const LAST: u64 = 6;
const PAGES: usize = 12;

fn fixture() -> crate::forge::Fixture {
    crate::forge::Fixture {
        setup: crate::forge::fixture().setup,
        configure: |value, agent, address| {
            crate::landing::configure(value, agent, address);
            value["application"]["webhooks"] = Value::Null;
            crate::fixtures::own_policy(value);
            value["application"]["forges"][0]["webhook_secret"] = Value::Null;
            value["engine"]["rules"]["ceiling"]["grants"] = json!([]);
            value["engine"]["projects"][0]["policy"]["ceiling"]["grants"] = json!([]);
            value["engine"]["projects"][0]["policy"]["roles"][0]["authority"]["grants"] = json!([]);
            value["engine"]["projects"][0]["policy"]["projections"] = json!([]);
            value["engine"]["limits"]["overrides"]["documents.rows"] = 2.into();
            // At the read, the root document carries all three live delegates whole.
            // Its line allowance is independent of the two-row tree/page window.
            value["engine"]["limits"]["overrides"]["documents.lines"] = 3.into();
            for name in ["tasks.batch", "authority.batch"] {
                value["engine"]["limits"]["overrides"][name] = 4.into();
            }
            value["engine"]["limits"]["overrides"]["tasks.delegates"] = 5.into();
            agent["profile"]["declared"]["calls_per_response"] = 2.into();
            // Keep two calls, payload 4,096 and output 8,192: lowering either bound alone
            // cannot fit Small. Window 262,144 gives final A = 1,320,403 and
            // F(A) = P = 1,320,467, within Small's page ceiling of 1,590,110.
            agent["endpoints"][0]["models"][0]["window"] = 262_144.into();
            value["engine"]["llm"]["models"][0]["window"] = 262_144.into();
            value["workers"][0]["limits"]["overrides"]["hub.run_calls"] = 2.into();
            // These report-only members carry no forge grants or parameters.
            value["engine"]["limits"]["overrides"]["tasks.authority_grants"] = 1.into();
            // Five delegates and the root's goal operation fit the same admission.
            value["engine"]["limits"]["overrides"]["people.operations"] = 6.into();
            let kinds = json!([{"kind":"charter","number":1},{"kind":"role","number":0}]);
            value["engine"]["rules"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
            value["engine"]["projects"][0]["policy"]["ceiling"]["delegation"]["kinds"] = kinds.clone();
            value["engine"]["projects"][0]["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = kinds;
            fixtures::turn_bounds::apply(value, fixtures::turn_bounds::turn_body(agent));
            // ServiceHost::configured admits the final document with its checked agent
            // through envelope_in after both fixture documents have been written.
        },
        ..crate::forge::fixture()
    }
}
fn turn(name: &[u8], value: &Value) -> skein_fake_llm_domain::api::Turn {
    use skein_fake_llm_domain::api::{Finish, Line, Turn};
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 1,
    }
}
// Hold the root's wait response until task 3's live watch is installed.
const WATCH: &[u8] = b"bounds-watch";
fn ready() -> skein_fake_llm_domain::api::Turn {
    use skein_fake_llm_domain::api::{Line, Piece};
    let mut turn = turn(b"wait", &json!({}));
    turn.lines = vec![
        Line::Pieces {
            pieces: Box::new([Piece { text: b"Bounds ready".as_slice().into(), hold: Some(WATCH.into()) }]),
        },
        turn.lines[0].clone(),
    ]
    .into_boxed_slice();
    turn
}
fn batch_created(peer: &skein_fake_peers::llm::Peer) -> bool {
    peer.observations().iter().any(|observation| {
        let skein_fake_peers::llm::Observation::Query { query, .. } = observation else { return false };
        query.messages.iter().any(|message| {
            message.parts.iter().any(|part| matches!(part, skein_fake_llm_domain::api::Part::ToolOutput { .. }))
        })
    })
}
fn scripts() -> Box<[skein_fake_llm_domain::api::Script]> {
    use skein_fake_llm_domain::api::Script;
    let member = |words, executor, dependencies| {
        json!({
            "executor":executor, "spec":{"words":words,"parameters":[]},
            "contract":{"kind":"report","words":128},
            "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":5},"notes":0},
            "dependencies":dependencies
        })
    };
    let agent = json!({"kind":"agent","charter":1});
    let delegates = json!({"batch":[
        member("@bounds-dependency", agent.clone(), json!([])),
        member("@bounds-ended", agent.clone(), json!([{"kind":"batch","number":0}])),
        member("Bounds person", json!({"kind":"role","role":0}), json!([])),
        member("Bounds waiting one", agent.clone(), json!([{"kind":"batch","number":2}]))
    ]});
    let last = json!({"batch":[member("Bounds waiting last", agent, json!([{"kind":"existing","number":4}]))]});
    // The store's batch ceiling is four. Both write calls run in order in
    // one model completion, so the second can name person task 4.
    let mut opening = turn(b"delegate", &delegates);
    opening.lines = vec![opening.lines[0].clone(), turn(b"delegate", &last).lines[0].clone()].into_boxed_slice();
    Box::new([
        Script {
            cue: b"@bounds-root".as_slice().into(),
            // The three live delegates keep both reports from being the last
            // result, so the root's default wake policy leaves it parked.
            turns: Box::new([opening, ready()]),
        },
        Script {
            cue: b"@bounds-dependency".as_slice().into(),
            turns: Box::new([turn(b"finish", &json!({"form":"report","text":"Dependency ended","fields":{}}))]),
        },
        Script {
            cue: b"@bounds-ended".as_slice().into(),
            turns: Box::new([turn(b"finish", &json!({"form":"report","text":"Dependent ended","fields":{}}))]),
        },
    ])
}

/// Observations kept outside the loop so even a deadline failure retains evidence.
#[derive(Default)]
pub struct Seen {
    pub activity_elapsed: Option<Duration>,
    pub completions: usize,
    pub activity: Vec<Value>,
    pub activity_text: Vec<String>,
    pub tree: String,
    pub delegates: Vec<u64>,
    pub task: Option<Value>,
}
#[derive(Clone, Copy, Debug)]
enum Stage {
    Activity,
    Tree,
    Tasks,
    Done,
}
struct Judge {
    patience: crate::patience::Patience,
    started: Option<Time>,
    controls: Controls,
    seen: Rc<RefCell<Seen>>,
    path: PathBuf,
    current: usize,
    stage: Stage,
    deadline: Option<Time>,
    wake: Option<Time>,
    stopping: bool,
    finished: bool,
    watch_released: bool,
}
impl Judge {
    fn next(&mut self, procs: &mut [Proc], words: &[&str], json: bool) {
        self.current += 1;
        assert!(self.current < PAGES, "bounded continuation pages");
        let bytes = words.iter().map(|word| word.as_bytes()).collect::<Vec<_>>();
        let temper_client_tools::jig::call::Parsed::Call { call, .. } =
            temper_client_tools::table::parse(&bytes).expect("printed continuation parses")
        else {
            panic!("continuation call")
        };
        let encoded = temper_client_tools::jig::json::write_call(temper_client_tools::table::TOOLS, &call)
            .expect("continuation JSON");
        fixtures::private(&self.path.join("bounds-call.json"), &encoded).expect("continuation file");
        let Proc::Page { pending, .. } = &mut procs[2 + self.current] else { panic!("page caller") };
        *pending = Some(PageCommand::Read { json });
    }
    fn answered(&mut self, now: Time, procs: &mut [Proc]) {
        let caller = procs[2 + self.current].caller();
        let call = caller.calls.last().expect("completed read");
        assert_eq!(call.exit, Some(crate::Exit::Code(0)), "{call:?}");
        match self.stage {
            Stage::Activity => {
                let page: Value = serde_json::from_slice(&call.stdout).expect("activity JSON");
                assert!(!page["rows"].as_array().expect("activity rows").is_empty(), "{page}");
                let next = page["read_on"].as_str().map(str::to_owned);
                let mut seen = self.seen.borrow_mut();
                seen.activity_text.push(String::from_utf8(call.stdout.clone()).expect("activity JSON text"));
                let previous = seen
                    .activity
                    .last()
                    .and_then(|page| page["rows"].as_array().expect("rows").last())
                    .map(|row| row["position"].as_u64().expect("position"));
                let mut positions = previous.into_iter().chain(
                    page["rows"]
                        .as_array()
                        .expect("rows")
                        .iter()
                        .map(|row| row["position"].as_u64().expect("position")),
                );
                if let Some(mut previous) = positions.next() {
                    for position in positions {
                        assert_eq!(position + 1, previous, "newest first, no lost row");
                        previous = position;
                    }
                    if next.is_none() {
                        assert_eq!(previous, 1, "the first activity row");
                    }
                }
                seen.activity.push(page);
                drop(seen);
                if let Some(next) = next {
                    self.next(procs, &next.split_ascii_whitespace().collect::<Vec<_>>(), true);
                } else {
                    self.seen.borrow_mut().activity_elapsed =
                        Some(now.saturating_since(self.started.expect("loop started")));
                    let seen = self.seen.borrow();
                    assert_eq!(seen.activity[0]["rows"][0]["position"], seen.task.as_ref().expect("task")["activity"]);
                    drop(seen);
                    self.stage = Stage::Tree;
                    self.deadline = Some(now.saturating_add(Duration::from_secs(2)));
                    self.next(procs, &["task", "show", "1", "--tree"], false);
                }
            }
            Stage::Tree => {
                self.deadline = None;
                let text = std::str::from_utf8(&call.stdout).expect("tree text").to_owned();
                let next = text
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("cut: ")
                            .and_then(|line| line.split_once("; read on with: ").map(|(_, call)| call.to_owned()))
                    })
                    .expect("tree cut call");
                self.seen.borrow_mut().tree = text;
                self.stage = Stage::Tasks;
                self.next(procs, &next.split_ascii_whitespace().collect::<Vec<_>>(), false);
            }
            Stage::Tasks => {
                self.seen.borrow_mut().delegates.extend(calls::listed(&call.stdout));
                let text = std::str::from_utf8(&call.stdout).expect("task page");
                if let Some(next) = text.lines().find_map(|line| line.strip_prefix("read on with: ")) {
                    let next = next.to_owned();
                    self.next(procs, &next.split_ascii_whitespace().collect::<Vec<_>>(), false);
                } else {
                    self.deadline = None;
                    self.stage = Stage::Done;
                    for index in self.current + 1..PAGES {
                        let Proc::Page { pending, .. } = &mut procs[2 + index] else { panic!("page caller") };
                        *pending = Some(PageCommand::Finish);
                    }
                }
            }
            Stage::Done => {}
        }
    }
}
impl Judge {
    fn describe_pending(&self, now: Time, procs: &mut [Proc]) -> String {
        let mut report = format!(
            "temper bounds judge overdue: stage={:?} page={} stopping={} finished={}\n",
            self.stage, self.current, self.stopping, self.finished
        );
        for (id, proc) in procs.iter_mut().enumerate() {
            match proc {
                Proc::Hosted(host) => crate::patience::describe(id, host, now, &mut report),
                Proc::Llm { peer, pending_streak, .. } => {
                    crate::patience::llm(id, peer, pending_streak.passes(), now, &mut report);
                }
                Proc::Page { caller, .. } => crate::patience::caller(id, caller, now, &mut report),
            }
        }
        report
    }
}
impl Referee<Proc> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        if self.patience.due(now) {
            let report = self.describe_pending(now, procs);
            self.patience.capture(report);
            crate::patience::fail_overdue::<Proc, _>(self, now);
        }
        self.wake = None;
        self.started.get_or_insert(now);
        assert!(
            self.deadline.is_none_or(|deadline| now < deadline),
            "{}",
            if self.stopping {
                "service shutdown did not finish within one second"
            } else {
                "task show 1 --tree did not answer within one second"
            }
        );
        let Proc::Llm { peer, .. } = &procs[0] else { panic!("LLM") };
        let address = peer.address();
        let created = batch_created(peer);
        let watching = procs[2].caller().watching;
        let Proc::Hosted(Service(service)) = &mut procs[1] else { panic!("service") };
        service.address = address;
        // The fixture makes no forge calls; a bound loopback address is sufficient.
        service.forge_address = address;
        if let Some(call) = watching {
            service.arm_watch(call);
        }
        let delivered = service.delivered();
        let links = service.links();
        let linked = links != 0;
        let Proc::Hosted(Caller(caller)) = &mut procs[2] else { panic!("initial caller") };
        caller.links.set(links);
        caller.advanced.set(created);
        if watching.is_some() {
            caller.watched = delivered;
        }
        caller.released |= linked;
        if delivered && !self.watch_released {
            let Proc::Llm { release, .. } = &mut procs[0] else { panic!("LLM") };
            *release = true;
            self.watch_released = true;
        }
        let caller = procs[2 + self.current].caller();
        if caller.is_empty() && !matches!(self.stage, Stage::Done) {
            if self.current == 0 {
                for (index, call) in caller.calls.iter().enumerate() {
                    assert_eq!(
                        call.exit,
                        Some(crate::Exit::Code(0)),
                        "setup call {index}: {} {}",
                        String::from_utf8_lossy(&call.stdout),
                        String::from_utf8_lossy(&call.stderr)
                    );
                }
                // An ended wait finishes with the same historical task read.
                let waited: Value = serde_json::from_slice(&caller.calls[2].stdout).expect("ended wait JSON");
                assert_eq!(waited["met"], true);
                let task = waited["task"].clone();
                assert_eq!(task["chip"]["task"], ENDED);
                assert_eq!(task["chip"]["phase"], "ended");
                assert_eq!(task["dependencies"][0]["retained"]["chip"]["task"], 2);
                assert_eq!(task["dependencies"][0]["retained"]["chip"]["phase"], "ended");
                assert_eq!(task["lineage_chips"][0]["details"]["owner"]["details"]["live_delegates"], 3);
                for (call, number) in caller.calls[3..6].iter().zip(4..=6) {
                    let live: Value = serde_json::from_slice(&call.stdout).expect("live delegate JSON");
                    assert_eq!(live["chip"]["task"], number);
                    assert_ne!(live["chip"]["phase"], "ended");
                }
                self.seen.borrow_mut().task = Some(task);
            }
            self.answered(now, procs);
        }
        if matches!(self.stage, Stage::Done) && !self.stopping && procs[2..2 + PAGES].iter().all(Host::is_empty) {
            let declared = procs[2].caller().leftovers();
            let mut obligations = crate::ending::Obligations::default();
            for (index, proc) in procs.iter().enumerate() {
                if let Proc::Hosted(proc) = proc {
                    if let Service(service) = proc {
                        obligations.read(service, declared);
                    }
                    obligations.read_child(index, proc);
                }
            }
            obligations.check(declared).unwrap_or_else(|error| panic!("{error}"));
            self.deadline = Some(now.saturating_add(Duration::from_secs(2)));
            self.controls.signal(1, ServiceSignal::Terminate);
            self.stopping = true;
        }
        let Proc::Hosted(Service(service)) = &procs[1] else { panic!("service") };
        if service.is_empty() && !self.finished {
            let Proc::Llm { peer, .. } = &mut procs[0] else { panic!("LLM") };
            peer.shutdown();
            self.deadline = None;
            self.finished = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        let Proc::Llm { peer, .. } = &procs[0] else { panic!("LLM") };
        let Proc::Hosted(Service(service)) = &procs[1] else { panic!("service") };
        let Proc::Hosted(Caller(caller)) = &procs[2] else { panic!("initial caller") };
        let current = procs[2 + self.current].caller();
        if (peer.address().is_some() && service.address.is_none())
            || (service.links() != 0 && !caller.released)
            || (!caller.advanced.get() && batch_created(peer))
            || (caller.watching.is_some() && (!service.watching() || service.delivered() && !caller.watched))
            || (current.is_empty() && !matches!(self.stage, Stage::Done))
            || (matches!(self.stage, Stage::Done) && !self.stopping && procs[2..2 + PAGES].iter().all(Host::is_empty))
            || (service.is_empty() && !self.finished)
        {
            self.wake = Some(now);
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.wake, self.deadline, self.patience.deadline()].into_iter().flatten().min()
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        self.patience.overdue()
    }
    fn passed(&self) -> bool {
        self.finished
    }
}

/// Hosted ctl calls read each supplied continuation in the same running service.
pub fn run(seen: &Rc<RefCell<Seen>>) {
    use calls::{D1, call, sign_in};
    let scratch = crate::Scratch::new("template-bounds");
    let patience = crate::PATIENCE;
    let mut world = World::new_controlled_checked(|controls| Judge {
        controls,
        patience: crate::patience::Patience::new(patience),
        started: None,
        seen: seen.clone(),
        path: scratch.path().into(),
        current: 0,
        stage: Stage::Activity,
        deadline: None,
        wake: None,
        stopping: false,
        finished: false,
        watch_released: false,
    });
    world.spawn(|| Proc::Llm {
        peer: Box::new(llm::peer(llm::scripted(scripts())).expect("scripted peer")),
        release: false,
        pending_streak: crate::patience::PendingStreak::default(),
    });
    world.spawn_signals(|signals| {
        Proc::Hosted(Service(Box::new(ServiceHost::configured(scratch.path().into(), signals, Some(fixture()), false))))
    });
    for index in 0..PAGES {
        let root = skein_shell::open_root(scratch.path()).expect("caller root");
        world.spawn_with_fds(vec![root], || {
            if index == 0 {
                Proc::Hosted(Caller(Box::new(calls::Caller::provisioned(
                    vec![
                        sign_in(D1),
                        call(D1, crate::argv!["chat", "start", "--project", "1", "--key", "bounds", "@bounds-root"]),
                        // The next model query carries both committed delegate results.
                        // Hold its wait response until this live task watch is delivered.
                        calls::advance(),
                        calls::start(D1, crate::argv!["task", "show", "3", "--until", "ended", "--json"]),
                        calls::watching(2),
                        calls::join(2),
                        call(D1, crate::argv!["task", "show", "4", "--json"]),
                        call(D1, crate::argv!["task", "show", "5", "--json"]),
                        call(D1, crate::argv!["task", "show", "6", "--json"]),
                        call(D1, crate::argv!["task", "activity", "3", "--json"]),
                        calls::ends_leaving(&[crate::ending::Leftover {
                            task: 4,
                            may_be: &[crate::ending::Left::Due],
                            why: "bounds pages retain the unclaimed human delegate; the other delegates wait on dependencies",
                        }]),
                    ],
                    scratch.path().into(),
                    root,
                ))))
            } else {
                Proc::Page {
                    caller: Box::new(calls::Caller::new(
                        vec![call(D1, crate::argv!["me"])],
                        scratch.path().into(),
                        root,
                    )),
                    path: scratch.path().into(),
                    root,
                    pending: None,
                }
            }
        });
    }
    let limits = temper_client_shell_cli::limits::native();
    world.host_roots(
        HostedProgram {
            program: fixtures::CTL.into(),
            make: |spawn, inherited| Proc::Hosted(fixtures::ctl(spawn, inherited)),
            instances: fixtures::CALLS_AT_ONCE,
            operations: skein_io::operations(&limits.io).expect("tools operations") * 2 + 5,
        },
        fixtures::ctl_roots,
    );
    let configuration = smith_agent_shell::config::read(&scratch.path().join("agent.json")).expect("agent bounds");
    world.host_roots(
        HostedProgram {
            program: fixtures::AGENT.into(),
            make: |spawn, inherited| Proc::Hosted(agent::agent(spawn, inherited)),
            instances: 1,
            operations: configuration
                .service
                .limits
                .routes
                .checked_add(3)
                // smith reserves one hosted open per permitted requested root.
                .and_then(|count| count.checked_add(configuration.service.limits.domain.run.directories))
                .expect("hosted agent operations"),
        },
        agent::roots,
    );
    let outcome = world.run(&skein_shell::Clock::new(), patience);
    let Proc::Llm { peer, .. } = &outcome.procs[0] else { panic!("LLM") };
    seen.borrow_mut().completions = peer
        .observations()
        .iter()
        .filter(|observation| {
            matches!(observation, skein_fake_peers::llm::Observation::Answered { result: Ok(()), .. })
        })
        .count();
    let Proc::Hosted(Service(service)) = &outcome.procs[1] else { panic!("service") };
    assert_eq!(service.service.as_ref().and_then(temper_service::Service::end), Some(&crate::End::Drained));
    assert!(outcome.killed.is_empty(), "no requested kills");
}
