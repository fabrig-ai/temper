//! Cold restart evidence through the native person, ctl and the owned Forgejo
//! (domain/landing.md, 7 and 15).
use super::{
    ANSWER, ChatJudge, ChatPhase, Ending, LANDING, Process, WITHIN, ctl, fake_llm, forgejo::Throwaway, landing_by_turn,
    landing_scripts, native_settings_setup, person, serve, write_private,
};
use serde_json::{Value, json};
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, end_to_end::Streams, real};
use std::{cell::Cell, fs, path::Path};

#[derive(Debug)]
pub struct Reads {
    pub landing: Value,
    pub list: Value,
    pub change: Value,
    pub task: Value,
}

fn read(root: &Path, arguments: &[&str]) -> Value {
    let call = ctl::alone(root, ctl::Call::new(arguments).in_directory("private").json());
    assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    ctl::json(&call)
}

#[must_use]
pub fn reads(root: &Path, landing: u64, change: u64) -> Reads {
    Reads {
        landing: read(root, &["landing", "show", &landing.to_string()]),
        list: read(root, &["landing", "list", "--project", "1", "--ended"]),
        change: read(root, &["change", "show", &change.to_string()]),
        task: read(root, &["task", "show", &landing.to_string()]),
    }
}

/// This tier seeds project 1 from its document. Leave its landing task a share
/// of that period, as `landing_by_turn::fund_landings` does for a new project.
pub fn configure(root: &Path, repository: u64) {
    let path = root.join("temper.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    // The stock's fourteen-grant rules need more than the process fixture's
    // twelve-grant allowance; each grant also requires a note scope.
    config["engine"]["limits"]["overrides"]["authority.grants"] = 16.into();
    config["engine"]["limits"]["overrides"]["tasks.authority_grants"] = 16.into();
    config["engine"]["limits"]["overrides"]["notes.scopes"] = 32.into();
    for project in config["engine"]["projects"].as_array_mut().expect("seed projects") {
        let policy = &mut project["policy"];
        let period = policy["period_spend"].as_u64().expect("project period");
        policy["roles"][0]["period_spend"] = json!(period * landing_by_turn::ROLE_PERIOD_IN_K / 48);
        // Landing tasks need status, dispatch and rerun grants in the project's
        // ceiling, independently of its owner's pool (domain/landing.md, 4.2).
        for kind in [6, 16, 17] {
            let grants = policy["ceiling"]["grants"].as_array_mut().expect("project grants");
            let mut grant = grants[0].clone();
            grant["kind"] = json!(kind);
            grants.push(grant);
        }
        policy["ceiling"]["delegation"]["kinds"]
            .as_array_mut()
            .expect("executors")
            .push(json!({"kind":"procedure","number":3}));
    }
    config["application"]["landing"]["projects"] = json!([{
        "project":1,"by":"turn","rules":[],
        "gates":[{"forge":1,"repository":repository,
            "gates":[{"name":"gate","workflow":"gate.yaml","ignored":["docs"],"blocking":true}]}]
    }]);
    write_private(&path, &serde_json::to_vec(&config).expect("configuration JSON"));
}

pub struct Proof {
    pub reads: Reads,
    pub original: String,
    pub moved: String,
    pub run: u64,
}

struct Judge<'a> {
    chat: ChatJudge<'a>,
    root: &'a Path,
    proof: &'a Cell<Option<Proof>>,
    landed: &'a Cell<Option<Reads>>,
    previous: Option<&'a Proof>,
    retry: bool,
    advanced: Option<(String, String)>,
    poll: Time,
    progress: (u32, Time),
}
impl Judge<'_> {
    fn sample(&mut self) -> bool {
        let forge = self.chat.forge.expect("owned forge");
        if let Some(previous) = self.previous {
            if self.retry {
                let landing = previous.reads.landing["task"].as_u64().expect("landing task");
                let change = previous.reads.change["task"].as_u64().expect("change task");
                let panel = read(self.root, &["landing", "show", &landing.to_string()]);
                let gate = &panel["members"][0]["gates"][0];
                if gate["faults"] != 1 || gate["proof"]["dispatched"]["attempt"] != 2 {
                    return false;
                }
                let runs = forge.repository_observation("acme", "widget", "actions/runs?limit=100");
                let Some(run) = runs["workflow_runs"].as_array().expect("runs").iter().find(|run| {
                    run["workflow_id"] == "gate.yaml" && run["status"] == "running" && run["id"] != previous.run
                }) else {
                    return false;
                };
                self.proof.set(Some(Proof {
                    reads: reads(self.root, landing, change),
                    original: previous.original.clone(),
                    moved: previous.moved.clone(),
                    run: run["id"].as_u64().expect("retry run"),
                }));
                return true;
            }
            if forge.merged_pull().is_some() {
                let landing = previous.reads.landing["task"].as_u64().expect("landing task");
                let change = previous.reads.change["task"].as_u64().expect("change task");
                let snapshot = reads(self.root, landing, change);
                if snapshot.landing["phase"] != "landed"
                    || snapshot.task["chip"]["phase"] != "ended"
                    || snapshot.change["step"] != "landed"
                {
                    return false;
                }
                self.landed.set(Some(snapshot));
                return true;
            }
            return false;
        }
        let pulls = forge.repository_observation("acme", "widget", "pulls?state=open&limit=2");
        let Some(pull) = pulls.as_array().expect("pull list").first() else { return false };
        if self.advanced.is_none() {
            let original = pull["head"]["sha"].as_str().expect("original head").to_owned();
            forge.add_file("notes.txt", landing_by_turn::NOTES);
            let main = forge.repository_observation("acme", "widget", "branches/main");
            self.advanced = Some((original, main["commit"]["id"].as_str().expect("moved main").to_owned()));
            eprintln!("turn restart: main advanced after the pull opened");
        }
        let runs = forge.repository_observation("acme", "widget", "actions/runs?limit=100");
        let Some(run) = runs["workflow_runs"]
            .as_array()
            .expect("workflow runs")
            .iter()
            .find(|run| run["workflow_id"] == "gate.yaml" && run["status"] == "running")
        else {
            return false;
        };
        let list = read(self.root, &["landing", "list", "--project", "1"]);
        let row = list["landings"].as_array().expect("landing rows").first().expect("live landing");
        let landing = row["task"].as_u64().expect("landing task");
        let change = row["changes"][0].as_u64().expect("member change");
        let reads = reads(self.root, landing, change);
        let candidate = reads.landing["members"][0]["candidate"].as_str().expect("candidate");
        let statuses = forge.repository_observation("acme", "widget", &format!("commits/{}/status", &candidate[..40]));
        if !statuses["statuses"]
            .as_array()
            .expect("statuses")
            .iter()
            .any(|status| status["context"] == "temper/gate/gate" && status["status"] == "pending")
        {
            return false;
        }
        let (original, moved) = self.advanced.clone().expect("main advanced");
        self.proof.set(Some(Proof { reads, original, moved, run: run["id"].as_u64().expect("run id") }));
        true
    }
}
impl Referee<Process> for Judge<'_> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        // ChatJudge hands the settings session to the native person. Its usual
        // stop waits for a merge; this story stops at its own durable read.
        if matches!(self.chat.phase, ChatPhase::SettingUp) {
            self.chat.act(now, procs);
        }
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        if person.steps_seen() != self.progress.0 {
            self.progress = (person.steps_seen(), now);
        }
        if !self.chat.stopping() && now >= self.progress.1.saturating_add(WITHIN) {
            let setup = procs.iter().find_map(|proc| match proc {
                Process::Calls(calls) => Some(calls.ends().iter().map(|call| call.exit).collect::<Vec<_>>()),
                Process::Binary(_)
                | Process::Stdout(_)
                | Process::Person(_)
                | Process::Peer(_)
                | Process::Issuer(_)
                | Process::Visitor(_) => None,
            });
            eprintln!("stalled change: {}", read(self.root, &["change", "show", "3"]));
            eprintln!("stalled landing task: {}", read(self.root, &["task", "show", "5"]));
            eprintln!("stalled landings: {}", read(self.root, &["landing", "list", "--project", "1", "--ended"]));
            eprintln!("stalled tasks: {}", read(self.root, &["task", "list", "--project", "1"]));
            let runs =
                self.chat.forge.expect("forge").repository_observation("acme", "widget", "actions/runs?limit=100");
            let runs: Vec<_> = runs["workflow_runs"]
                .as_array()
                .expect("runs")
                .iter()
                .map(|run| json!({"id":run["id"],"workflow":run["workflow_id"],"status":run["status"]}))
                .collect();
            eprintln!("stalled runs: {runs:?}");
            panic!(
                "turn restart stalled: native step {}, finished={}, setup={setup:?}, forge={}",
                person.steps_seen(),
                person.finished,
                self.chat.forge.expect("forge").failure_state()
            );
        }
        if !self.chat.stopping() && now >= self.poll {
            self.poll = now.saturating_add(Duration::from_millis(100));
            // A listener may draw another port on restart. Let the native
            // client keep that address before ctl loads its client directory.
            if self.previous.is_some() && (!person.finished || !person.client.as_ref().is_some_and(Host::is_empty)) {
                return;
            }
            // Moving main starts as soon as the pull opens, before readiness.
            let ready = self.sample();
            if ready && person.finished && person.client.as_ref().is_some_and(Host::is_empty) {
                let Process::Binary(binary) = &mut procs[0] else { panic!("serve") };
                binary.signal(Signal::Terminate);
                for proc in procs {
                    if let Process::Peer(peer) = proc {
                        peer.shutdown();
                    }
                }
                self.chat.phase = ChatPhase::Stopping;
            }
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.chat.observe(now, procs);
    }
    fn next_deadline(&self) -> Option<Time> {
        self.chat.next_deadline().map(|deadline| if self.chat.stopping() { deadline } else { deadline.min(self.poll) })
    }
    fn overdue(&self, now: Time) -> Option<String> {
        self.chat.overdue(now)
    }
    fn passed(&self) -> bool {
        self.chat.passed()
    }
}

/// Each call starts a new shipped serve process and drains it and every host.
#[must_use]
pub fn run(root: &Path, forge: &Throwaway, previous: Option<&Proof>) -> (Option<Proof>, Option<Reads>) {
    run_to(root, forge, previous, false)
}

/// A cancelled run is redispatched by the policy; stop inside that retry's job.
#[must_use]
pub fn retry(root: &Path, forge: &Throwaway, previous: &Proof) -> Proof {
    run_to(root, forge, Some(previous), true).0.expect("stopped inside the retry")
}

fn run_to(root: &Path, forge: &Throwaway, previous: Option<&Proof>, retry: bool) -> (Option<Proof>, Option<Reads>) {
    let peer = fake_llm(landing_scripts(ANSWER, forge));
    peer.configure(root);
    let first = previous.is_none();
    let script = if first { person::adopt_and_start_change(forge) } else { person::found() };
    let clock = Clock::new();
    let mut binary = serve(root);
    let streams = binary.take_streams().expect("serve pipes");
    let mut fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes") };
    let person_root = skein_shell::open_root(root).expect("native root");
    let proof = Cell::new(None);
    let landed = Cell::new(None);
    let mut world = real::World::new(Judge {
        chat: ChatJudge {
            phase: if first { ChatPhase::SettingUp } else { ChatPhase::Running },
            ending: Ending::Stop,
            deadline: clock.now().now.saturating_add(LANDING),
            forge: Some(forge),
            poll: clock.now().now,
            merged: false,
        },
        root,
        proof: &proof,
        landed: &landed,
        previous,
        retry,
        advanced: None,
        poll: clock.now().now,
        progress: (0, clock.now().now),
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    if first {
        world.spawn_with_fds(vec![person_root], || {
            Process::Person(Box::new(person::Person::waiting_for_ctl_with_key(person_root, script)))
        });
        let ctl_root = skein_shell::open_root(root).expect("ctl root");
        fds.push(ctl_root);
        world.spawn_with_fds(fds, || {
            Process::Calls(Box::new(ctl::Calls::new(ctl_root, native_settings_setup(root), Some(&streams), None)))
        });
    } else {
        fds.push(person_root);
        world
            .spawn_with_fds(fds, || Process::Person(Box::new(person::Person::new(person_root, input, output, script))));
    }
    peer.spawn(&mut world);
    let outcome = world.run(&clock, LANDING.saturating_add(Duration::from_secs(10)));
    assert!(outcome.procs.iter().all(Host::is_empty), "all real io settles before restarting");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("serve") };
    assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "{}", String::from_utf8_lossy(binary.stderr()));
    (proof.take(), landed.take())
}
