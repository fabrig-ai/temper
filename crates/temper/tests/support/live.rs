//! The retained native stories with fake completions until live sign-in is available.
use super::{Process, Seen, forgejo::Throwaway, person, write_private};
use jig_protocol_store::{codec, transcripts};
use serde_json::Value;
use skein_io::kernel::Signal;
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, end_to_end::Streams, real};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

#[must_use]
pub fn configure(directory: &Path, forge: &Throwaway) -> skein_fake_peers::llm::Peer {
    let path = directory.join("temper.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    value["engine"]["charters"][0]["turns"] = 5.into();
    value["engine"]["charters"][0]["time_ms"] = 240_000.into();
    let agent_path = directory.join("agent.json");
    let mut agent: Value = serde_json::from_slice(&fs::read(&agent_path).expect("agent")).expect("agent JSON");
    agent["profile"]["policy"]["max_turns"] = 5.into();
    let scripts = super::landing_scripts(super::ANSWER, forge);
    let skein_fake_llm_domain::api::Line::Call { arguments, .. } = &scripts[0].turns[0].lines[0] else {
        panic!("the process fixture delegates the coding procedure");
    };
    let instructions = format!(
        "A task whose contract is a change reads, edits and finishes with a change, and delegates nothing. Follow the task's role exactly. For the initial greeting conversation, call finish with {{\"form\":\"report\",\"text\":\"Process answer\",\"fields\":{{}}}}. \
        For the conversation requesting a small change, call delegate exactly with {}, then call wait with {{}}. \
        For the delegated coding task, read README.md, append exactly Changed by temper. followed by a newline, use edit, \
        then finish with {{\"form\":\"change\",\"fields\":{{\"title\":\"Update README\",\"body\":\"Changed by temper.\"}}}}. \
        For a change review task, finish with {{\"form\":\"verdict\",\"label\":\"1\",\"text\":\"Checked\",\"fields\":{{}},\"items\":[]}}.",
        std::str::from_utf8(arguments).expect("delegate JSON").replace("@coding", "\\u0040coding")
    );
    write_private(&directory.join("charters/chat.txt"), instructions.as_bytes());
    write_private(&path, &serde_json::to_vec(&value).expect("live configuration JSON"));
    write_private(&agent_path, &serde_json::to_vec(&agent).expect("live agent JSON"));
    super::fake_llm(scripts)
}

pub struct Pushed {
    pub seen: Seen,
    pub branch: Box<[u8]>,
    pub pushed: Box<[u8]>,
    pub wall_seconds: u64,
}
type Push = super::forgejo::BranchHead;
struct Judge<'a> {
    directory: &'a Path,
    forge: &'a Throwaway,
    baseline: Box<[u8]>,
    pushed: Rc<RefCell<Option<Push>>>,
    poll: Time,
    deadline: Time,
    ended: bool,
    settled: bool,
}
fn files(directory: &Path, output: &mut Vec<PathBuf>) {
    if !directory.exists() {
        return;
    }
    for entry in fs::read_dir(directory).expect("proof directory").map(|entry| entry.expect("proof entry")) {
        let path = entry.path();
        if entry.file_type().expect("proof file type").is_dir() {
            files(&path, output);
        } else {
            output.push(path);
        }
    }
}
fn pushed(directory: &Path, forge: &Throwaway, baseline: &[u8]) -> Option<Push> {
    let heads: Vec<_> = forge
        .branch_heads()
        .into_iter()
        .filter(|(branch, commit)| branch.starts_with(b"temper/") && commit.as_ref() != baseline)
        .collect();
    if heads.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    files(&directory.join("workspaces"), &mut paths);
    for path in paths {
        if path.file_name().is_none_or(|name| name != "HEAD") {
            continue;
        }
        let git = path.parent().expect("HEAD parent");
        if git.file_name().is_none_or(|name| name != ".git") {
            continue;
        }
        let output = std::process::Command::new("/usr/bin/git")
            .env_clear()
            .arg("--git-dir")
            .arg(git)
            .args(["cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype)"])
            .output()
            .expect("git's object database");
        assert!(output.status.success(), "the run's git objects can be read");
        for line in output.stdout.split(|byte| *byte == b'\n') {
            let Some(commit) = line.strip_suffix(b" commit") else { continue };
            if let Some((branch, _)) = heads.iter().find(|(_, head)| head.as_ref() == commit) {
                return Some((branch.clone(), commit.into()));
            }
        }
    }
    None
}
impl Referee<Process> for Judge<'_> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        if !self.ended && self.pushed.borrow().is_none() && now >= self.poll {
            *self.pushed.borrow_mut() = pushed(self.directory, self.forge, &self.baseline);
            self.poll = now.saturating_add(Duration::from_millis(100));
        }
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        if !self.ended
            && person.finished
            && person.client.as_ref().is_some_and(Host::is_empty)
            && self.pushed.borrow().is_some()
        {
            let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
            binary.signal(Signal::Terminate);
            for proc in procs {
                if let Process::Peer(peer) = proc {
                    peer.shutdown();
                }
            }
            self.ended = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.settled = self.ended && procs.iter().all(Host::is_empty);
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        assert!(
            person.failed.is_none(),
            "{}; {}",
            person.failed.as_deref().unwrap_or("person"),
            String::from_utf8_lossy(binary.stderr())
        );
        assert!(
            self.ended || binary.exit_status().is_none(),
            "serve failed: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        assert!(now < self.deadline || self.settled, "live story deadline; pushed={}", self.pushed.borrow().is_some());
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(if self.ended || self.pushed.borrow().is_some() {
            self.deadline
        } else {
            self.poll.min(self.deadline)
        })
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "live native stories did not settle".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
#[must_use]
pub fn run(directory: &Path, forge: &Throwaway, peer: skein_fake_peers::llm::Peer, deadline: Duration) -> Pushed {
    let clock = Clock::new();
    let started = clock.now().now;
    let mut binary = super::serve(directory);
    let streams = binary.take_streams().expect("binary pipes");
    let mut fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
    let root = skein_shell::open_root(directory).expect("client's parent root");
    fds.push(root);
    let pushed = Rc::new(RefCell::new(None));
    let mut world = real::World::new(Judge {
        directory,
        forge,
        baseline: forge.branch_head(b"main").expect("initial main"),
        pushed: pushed.clone(),
        poll: started,
        deadline: started.saturating_add(deadline),
        ended: false,
        settled: false,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        Process::Person(Box::new(person::Person::new(root, input, output, person::adopt_and_change(forge))))
    });
    world.spawn(|| Process::Peer(Box::new(peer)));
    let outcome = world.run(&clock, deadline);
    assert!(outcome.procs.iter().all(Host::is_empty), "all real io settles");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("binary") };
    let Process::Person(person) = &outcome.procs[1] else { panic!("native person") };
    let (branch, pushed) = pushed.borrow_mut().take().expect("a new run commit was pushed");
    Pushed {
        seen: Seen {
            signed_in: person.client.as_ref().expect("client").seen().signed_in,
            finished: person.finished,
            stderr: binary.stderr().to_vec(),
        },
        branch,
        pushed,
        wall_seconds: clock.now().now.saturating_since(started).as_nanos() / 1_000_000_000,
    }
}
#[must_use]
pub fn completions(directory: &Path) -> u32 {
    let value = fs::read(directory.join("temper.json")).expect("configuration");
    let document = jig_protocol_config::document(
        &value,
        &jig_protocol_config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 },
    )
    .expect("document");
    let at = jig_protocol_config::read::Object::new(&document, 0)
        .expect("object")
        .member(&document, b"engine")
        .expect("engine member");
    let engine = jig_protocol_config::engine(&document, at, &super::configuration::directories(directory).jig())
        .expect("engine configuration");
    let limits = codec::limits(&engine.limits.core).expect("store codec limits");
    let mut paths = Vec::new();
    files(&directory.join("state/transcripts"), &mut paths);
    let mut count = 0;
    for path in paths {
        let bytes = fs::read(path).expect("transcript");
        let mut at = 0;
        while at + 4 <= bytes.len() {
            let length = usize::try_from(u32::from_be_bytes(bytes[at..at + 4].try_into().expect("frame length")))
                .expect("u32 fits");
            if length < 12 || at + length > bytes.len() {
                break;
            }
            transcripts::frame::decode(&bytes[at..at + length], &limits).expect("durable turn");
            count += 1;
            at += length;
        }
    }
    count
}
