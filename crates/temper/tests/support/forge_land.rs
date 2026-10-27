//! A person's change on the owned forge, signed in with `ChatGPT` (domain/testing.md, 2.6–2.7).
use super::{Process, sign_in};
use skein_fake_llm_domain::api::{self, Finish, Line, Script, Turn};
use skein_fake_peers::oauth;
use skein_io::kernel::Exit;
use skein_lib::Duration;
use skein_world::{Referee, real};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

pub mod rung;

/// forge-setup's first repository goes into a project of its own, made after
/// project 1 (tools.md 5.5): work/hello is project 2's.
const FIRST_PROJECT: &str = "2";

/// The model side: processes a story adds beside the box and ctl.
#[expect(
    clippy::large_enum_variant,
    reason = "existing shared fake constructors pass peers by value; faces are moved once at startup"
)]
pub enum Face {
    /// Shipped issuer and Responses; no auxiliary model processes.
    Live,
    /// Skein's fake issuer and the existing TLS LLM peer playing the plan's scripts.
    Fake { issuer: oauth::Peer, peer: super::llm::Listening },
}

/// Initialize three fresh private directories, then patch the config for the fakes.
#[must_use]
pub fn fake_deployment(paths: &[PathBuf; 3]) -> oauth::Peer {
    initialize(paths);
    sign_in::patch_configuration(&paths[0])
}

fn initialize(paths: &[PathBuf; 3]) {
    for path in paths {
        fs::create_dir(path).expect("fresh deployment directory");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private deployment directory");
    }
    let initialized = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(
            ["--config", "--data", "--cache"]
                .into_iter()
                .zip(paths)
                .flat_map(|(flag, path)| [std::ffi::OsString::from(flag), path.as_os_str().to_owned()]),
        )
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(&paths[1])
        .output()
        .expect("initialize a fresh deployment");
    assert!(initialized.status.success(), "{}", String::from_utf8_lossy(&initialized.stderr));
}

/// The person's request and the first repository's observed identity and contents.
pub struct Plan<'a> {
    /// The person's words, carrying `line`, which cue the chat.
    pub words: &'a [u8],
    /// The unique README line, without its trailing newline.
    pub line: &'a [u8],
    /// The first repository's resource, its packed tag (`temper_engine_domain_forge::tag(3, first.id)`).
    pub repository: u64,
    /// README.md on main before the change.
    pub readme: &'a [u8],
}

/// The first box's reply, once the change has landed.
pub const LANDED: &str = "The change landed.";
/// The restarted chat's answer: the second box's reply.
pub const RESTARTED: &str = "The change is still on main.";
/// The chat's answer to a held change's escalation.
pub const HELD: &str = "The change is held: its coder could not make it.";

/// Delegate, wait, then reply as an Open chat.
#[must_use]
pub fn plan_scripts(plan: &Plan<'_>) -> Box<[Script]> {
    let line = std::str::from_utf8(plan.line).expect("README line UTF-8");
    // Past the 128 bytes a brief's task section once held: the coder's script is
    // cued by its whole spec, so it runs only when the brief carries all of it.
    let coding = format!(
        "@coding Append this README line: {line} Keep every existing line as it is, add the new line last with a \
         newline after it, and change nothing else."
    );
    assert!(coding.len() > 128, "coder words past the old task section");
    assert!(!plan.words.is_empty(), "nonempty chat cue");
    assert!(!plan.line.is_empty() && !plan.line.contains(&b'\n'), "one nonempty README line");
    assert!(plan.words.windows(plan.line.len()).any(|part| part == plan.line), "request carries the line");
    // System cues match the earliest byte position. Each task's original spec
    // supplies its cue before any conversation history. Keep the coder's spec
    // distinct from the chat cue.
    assert!(!coding.as_bytes().windows(plan.words.len()).any(|part| part == plan.words), "distinct chat cue");
    let edit = format!("{line}\n");
    let mut scripts = temper_real_loop_world::landing::change_scripts(
        temper_real_loop_world::landing::ChangeAuthority { coding_kind: 2, scope: None },
        plan.repository,
        plan.readme,
        edit.as_bytes(),
        plan.words,
        &coding,
    )
    .into_vec();
    // Keep the first repository's proof that the worker cloned the starter workflow.
    let workflow = Turn {
        lines: Box::new([Line::Call {
            name: b"read".as_slice().into(),
            arguments: br#"{"path":".forgejo/workflows/ci.yml"}"#.as_slice().into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    };
    let mut coder = std::mem::take(&mut scripts[1].turns).into_vec();
    coder.insert(1, workflow);
    scripts[1].turns = coder.into_boxed_slice();
    let text =
        |bytes: &[u8]| Turn { lines: Box::new([Line::Text { text: bytes.into() }]), finish: Finish::Stop, tokens: 20 };
    let chat = &mut scripts[0];
    chat.turns[0].lines = Box::new([Line::Call {
        name: b"change".as_slice().into(),
        arguments: serde_json::to_vec(&serde_json::json!({"words": coding, "repository": "work/hello"}))
            .expect("change tool JSON")
            .into_boxed_slice(),
    }]);
    // An Open chat replies with text and keeps listening (domain/agent.md, 4.6).
    // The first box stops without Close; no finish request or closing turn runs here.
    chat.turns[2] = text(LANDED.as_bytes());
    scripts.into_boxed_slice()
}

/// The coder fails at once, so the change holds and escalates to the chat, which
/// answers with HELD instead of LANDED.
#[must_use]
pub fn held_scripts(plan: &Plan<'_>) -> Box<[Script]> {
    let mut scripts = plan_scripts(plan);
    scripts[1].turns = Box::new([Turn {
        lines: Box::new([Line::Call {
            name: b"finish".as_slice().into(),
            arguments: br#"{"form":"failure","reason":"cannot","fields":{}}"#.as_slice().into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }]);
    scripts[0].turns[2] =
        Turn { lines: Box::new([Line::Text { text: HELD.as_bytes().into() }]), finish: Finish::Stop, tokens: 20 };
    scripts
}

/// The chat reports a review hold.
pub const REVIEW_HELD: &str = "The change is held: its review could not run.";

/// Every fresh reviewer fails, while the coder follows the owned-forge story.
#[must_use]
pub fn failed_review_scripts(plan: &Plan<'_>) -> Box<[Script]> {
    let mut scripts = plan_scripts(plan);
    scripts[2].turns = Box::new([Turn {
        lines: Box::new([Line::Call {
            name: b"finish".as_slice().into(),
            arguments: br#"{"form":"failure","reason":"review unavailable","fields":{}}"#.as_slice().into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }]);
    let mut chat = std::mem::take(&mut scripts[0].turns).into_vec();
    chat.truncate(2);
    for _ in 0..8 {
        chat.push(Turn {
            lines: Box::new([Line::Text { text: REVIEW_HELD.as_bytes().into() }]),
            finish: Finish::Stop,
            tokens: 20,
        });
    }
    scripts[0].turns = chat.into_boxed_slice();
    scripts
}

/// The fake face uses the shared plan bearer and existing test TLS root.
#[must_use]
pub fn fake_face(issuer: oauth::Peer, scripts: Box<[Script]>) -> Face {
    Face::Fake { issuer, peer: sign_in::plan_peer(scripts) }
}

/// Indexes of the face's processes, following the story's box and ctl.
pub struct FaceProcs {
    pub issuer: usize,
    pub visitor: usize,
    pub peer: usize,
}

impl Face {
    /// Add the issuer, redirect visitor and model peer to the story's world.
    pub(super) fn spawn<R: Referee<Process>>(self, world: &mut real::World<Process, R>) -> FaceProcs {
        match self {
            Self::Live => panic!("the live face runs without peers through run_live"),
            Self::Fake { issuer, peer } => FaceProcs {
                issuer: world.spawn(|| Process::Issuer(Box::new(issuer))),
                visitor: world.spawn(|| Process::Visitor(Box::default())),
                peer: peer.spawn(world),
            },
        }
    }
}

/// What the model peer saw, without credential values.
pub struct Seen {
    pub requests: Vec<api::Query>,
    pub answers: Vec<Result<(), api::Error>>,
    pub schemes: Vec<Option<skein_llm::Scheme>>,
}

#[must_use]
pub(super) fn seen(procs: &[Process], at: &FaceProcs) -> Seen {
    peer_seen(procs, at.peer)
}

pub(super) fn peer_seen(procs: &[Process], index: usize) -> Seen {
    use skein_fake_peers::llm::Observation;
    let Process::Peer(peer) = &procs[index] else { panic!("LLM peer") };
    let mut seen = Seen { requests: Vec::new(), answers: Vec::new(), schemes: Vec::new() };
    for event in peer.observations() {
        match event {
            Observation::Query { query, .. } => seen.requests.push(query.clone()),
            Observation::Answered { result, .. } => seen.answers.push(*result),
            Observation::Credential { scheme, .. } => seen.schemes.push(*scheme),
            Observation::Accepted { .. } | Observation::Head { .. } | Observation::Closed { .. } => {}
        }
    }
    seen
}

/// A face and the conversation reads its box must follow before stopping.
pub struct CtlFace {
    pub face: Face,
    pub first: bool,
    /// The chat's reply the box waits for before it stops.
    pub reply: &'static str,
}

/// The plan story's face: the first box waits for the landing, the second for the restarted answer.
#[must_use]
pub fn plan_face(issuer: oauth::Peer, scripts: Box<[Script]>, first: bool) -> CtlFace {
    CtlFace { face: fake_face(issuer, scripts), first, reply: if first { LANDED } else { RESTARTED } }
}

/// Follow the committed reply, paging the conversation and waiting from a read's digest.
pub(super) struct Reply {
    first: bool,
    awaited: &'static str,
    snapshot: Option<String>,
    phase: ReplyPhase,
    replied: bool,
}
enum ReplyPhase {
    Snapshot,
    Pages,
    Delegates(u64),
    Done,
}
impl Reply {
    pub fn new(first: bool, awaited: &'static str) -> Self {
        Self { first, awaited, snapshot: None, phase: ReplyPhase::Snapshot, replied: false }
    }
    pub fn advance(&mut self, calls: &mut super::ctl::Calls, expected_exits: &mut Vec<(usize, u8)>) {
        use super::ctl::{Call, Step, json};
        if matches!(self.phase, ReplyPhase::Done) || !calls.foreground_idle() {
            return;
        }
        let last = json(calls.ends().last().expect("reply read"));
        if matches!(self.phase, ReplyPhase::Snapshot) {
            self.snapshot = Some(last["digest"].as_str().expect("task read digest").to_owned());
            calls.push(Step::Run(Call::new(&["chat", "show", "1", "--json"])));
            self.phase = ReplyPhase::Pages;
            return;
        }
        if let ReplyPhase::Delegates(parent) = self.phase {
            if let Some(after) = last["ended"]["rows"]["after"]["value"].as_str() {
                calls.push(Step::Run(delegate_page(parent, Some(after))));
            } else if parent == 1 {
                calls.push(Step::Run(delegate_page(2, None)));
                self.phase = ReplyPhase::Delegates(2);
            } else {
                if !self.first {
                    // Closing adds no work: task 5 must still be unknown.
                    expected_exits.push((calls.ends().len() + 3, 2));
                    for args in [
                        &["chat", "close", "1", "--key", "close-plan", "--json"][..],
                        &["task", "show", "1", "--until", "ended", "--timeout", "60s", "--json"],
                        &["task", "list", "--project", FIRST_PROJECT, "--json"],
                        &["task", "show", "5", "--json"],
                    ] {
                        calls.push(Step::Run(Call::new(args)));
                    }
                }
                calls.release();
                self.phase = ReplyPhase::Done;
            }
            return;
        }
        // The first box waits for the change's reply; the second, for the
        // restarted chat's answer, before it closes the chat.
        self.replied |=
            serde_json::to_string(&last["activity"]["rows"]).expect("conversation JSON").contains(self.awaited);
        if let Some(before) = last["activity"]["before"]["value"].as_str() {
            calls.push(Step::Run(Call::new(&["chat", "show", "1", "--before", before, "--json"])));
        } else if !self.replied {
            // 'Change requested' plus wait already meets replied. A snapshot
            // taken before reading the conversation prevents missing its next reply.
            calls.push(Step::Run(Call::new(&[
                "task",
                "show",
                "1",
                "--until",
                "replied",
                "--since",
                self.snapshot.as_deref().expect("snapshot before conversation"),
                "--timeout",
                "120s",
                "--json",
            ])));
            calls.push(Step::Run(Call::new(&["task", "show", "1", "--json"])));
            self.phase = ReplyPhase::Snapshot;
        } else {
            calls.push(Step::Run(delegate_page(1, None)));
            self.phase = ReplyPhase::Delegates(1);
        }
    }
}

fn delegate_page(parent: u64, after: Option<&str>) -> super::ctl::Call {
    let parent = parent.to_string();
    let mut args = vec!["task", "list", "--project", FIRST_PROJECT, "--parent", &parent, "--ended", "--json"];
    if let Some(after) = after {
        args.extend(["--after", after]);
    }
    super::ctl::Call::new(&args)
}

#[must_use]
pub fn restart_script(chat_cue: &[u8]) -> Box<[Script]> {
    // A system cue's turns count every assistant message (fake-llm.md 2.4):
    // the first box's delegate, wait and reply are turns 0 to 2, played there.
    // The restart runs the chat's lost attempt again, which answers (turn 3);
    // Close then asks the parked run to finish, and its text ends it (turn 4,
    // domain/agent.md, 4.6). An Open run offers no finish tool.
    let text =
        |bytes: &[u8]| Turn { lines: Box::new([Line::Text { text: bytes.into() }]), finish: Finish::Stop, tokens: 20 };
    let played = b"Played in the first box.";
    Box::new([Script {
        cue: chat_cue.into(),
        turns: Box::new([
            text(played),
            text(played),
            text(played),
            text(RESTARTED.as_bytes()),
            text(b"The plan chat is now closed."),
        ]),
    }])
}

/// The plan chat receives the entire landed pull line, including its address.
pub fn assert_landed_chat_pull(run: &str, evidence: &super::first_repository::Evidence) {
    let pull = format!("pull request #1 of work/hello on {0}: http://{0}/work/hello/pulls/1", evidence.address);
    assert!(
        evidence.delegation.contains(&pull),
        "{run}: the chat receives the whole pull line: {}",
        evidence.delegation
    );
}

#[must_use]
pub fn landed_identity(task: &str, page: &str) -> (String, String) {
    let (landed, _) = super::ctl::landed_result(task);
    let pull = page.lines().find(|line| line.starts_with("PR #")).expect("pull identity on the change page");
    assert!(pull.contains(" · work/hello · "), "pull belongs to the first repository: {pull}");
    let number = pull.strip_prefix("PR #").expect("pull prefix").split(" · ").next().expect("pull number");
    assert!(landed.starts_with(&format!("result: Landed · pull request #{number} · merge commit ")), "{landed}");
    (landed.to_owned(), pull.to_owned())
}

#[must_use]
pub fn plan_ended_tasks(calls: &[super::Ended]) -> (usize, [Vec<u64>; 2]) {
    let tasks_at = calls
        .iter()
        .position(|call| {
            serde_json::from_slice::<serde_json::Value>(&call.stdout)
                .ok()
                .is_some_and(|value| value["ended"].is_object() && value["live"].is_array())
        })
        .expect("ended delegate pages");
    let tasks =
        [0, 1].map(|offset| ended_tasks(&calls[tasks_at..], 1 + offset, if offset == 0 { &[2] } else { &[3, 4] }));
    (tasks_at, tasks)
}

#[must_use]
pub fn ended_tasks(calls: &[super::Ended], parent: u64, expected: &[u64]) -> Vec<u64> {
    let mut tasks = Vec::new();
    let mut whole = false;
    for call in calls {
        let value = super::ctl::json(call);
        let ended = &value["ended"]["rows"];
        let rows = ended["rows"].as_array().expect("ended rows");
        if !rows.iter().any(|row| row["requester"]["task"] == parent) {
            continue;
        }
        assert_no_live_tasks(&value);
        assert_eq!(ended["cut"], false, "uncut ended task page: {value}");
        whole = ended["after"].is_null();
        for row in rows {
            assert_eq!(row["requester"]["task"], parent, "one parent's delegates per query");
            tasks.push(row["chip"]["task"].as_u64().expect("task number"));
        }
    }
    assert!(whole, "every ended task page was read");
    tasks.sort_unstable();
    assert_eq!(tasks, expected, "exact ended delegates of this parent");
    tasks
}
pub fn assert_no_live_tasks(value: &serde_json::Value) {
    assert_eq!(value["live"], serde_json::json!([]), "no live tasks in this query: {value}");
    assert!(value["live_after"].is_null(), "all live tasks read");
}
/// A project's live roots read: `task list --project PROJECT` without `--ended`.
pub fn assert_no_live_roots(value: &serde_json::Value) {
    assert_eq!(value["tasks"]["rows"], serde_json::json!([]), "no live root tasks: {value}");
    assert_eq!(value["tasks"]["more"], false, "all live root tasks read: {value}");
}

pub fn assert_reply(calls: &[super::Ended], first: bool) {
    assert_eq!(replies(calls, LANDED), 1, "one committed final reply, across all conversation pages");
    assert_eq!(replies(calls, RESTARTED), usize::from(!first), "the restarted chat answers once");
}
fn replies(calls: &[super::Ended], text: &str) -> usize {
    let mut replies = std::collections::BTreeMap::new();
    for call in calls {
        let page = super::ctl::json(call);
        let Some(rows) = page["activity"]["rows"].as_array() else { continue };
        assert_eq!(page["activity"]["cut"], false);
        for row in rows {
            let Some(turn) = row.pointer("/event/owner/decoded/turn/decoded") else { continue };
            assert_eq!(turn["cut"], false, "whole turn");
            for message in turn["messages"].as_array().expect("turn messages") {
                if message["speaker"] != "assistant" {
                    continue;
                }
                for block in message["blocks"].as_array().expect("message blocks") {
                    if block["text"]["text"] == text {
                        replies.insert(row["position"].as_u64().expect("position"), turn.clone());
                    }
                }
            }
        }
    }
    replies.len()
}

pub fn assert_requests(seen: &Seen, count: usize, words: &[u8], first: bool) {
    let mut counts = [0; 3];
    for query in &seen.requests {
        let has = |cue: &[u8]| query.system.windows(cue.len()).any(|part| part == cue);
        let kind = if has(words) {
            0
        } else if has(b"## Task\n\n@coding Append this README line:") {
            1
        } else if has(b"Review this change at the named head") {
            2
        } else {
            panic!("unexpected task cue")
        };
        counts[kind] += 1;
    }
    assert_eq!(counts, if first { [3, 4, 1] } else { [2, 0, 0] }, "chat, coder, review requests");
    assert_eq!(seen.requests.len(), count, "exact scripted model requests");
    assert_eq!(seen.answers, vec![Ok(()); count], "every request answered");
    assert_eq!(seen.schemes, vec![Some(skein_llm::Scheme::Bearer); count], "a bearer on every request");
}

pub fn plan_change_steps(
    steps: &mut Vec<super::ctl::Step>,
    first: bool,
    setup_calls: usize,
    words: &str,
) -> (usize, usize) {
    change_steps(steps, first, setup_calls, words, "120s", Ends::Landed)
}

/// The first box's steps when its coder fails: the change holds, and the chat,
/// woken by the held change's escalation, answers.
pub fn held_change_steps(steps: &mut Vec<super::ctl::Step>, setup_calls: usize, words: &str) -> (usize, usize) {
    change_steps(steps, true, setup_calls, words, "120s", Ends::Held)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ends {
    Landed,
    Held,
}

fn change_steps(
    steps: &mut Vec<super::ctl::Step>,
    first: bool,
    setup_calls: usize,
    words: &str,
    within: &str,
    ends: Ends,
) -> (usize, usize) {
    use super::ctl::{Call, Step};
    steps.push(Step::Run(Call::new(&["me", "--json"])));
    if first {
        steps.extend([
            Step::Run(Call::new(&["account", "connect", "1", "--key", "connect-plan"])),
            Step::Run(Call::new(&["account", "list", "--until", "settled", "--json"])),
        ]);
    }
    let account_at = setup_calls + if first { 3 } else { 1 };
    steps.push(Step::Run(Call::new(&["account", "list", "--json"])));
    if first {
        steps.push(Step::Run(Call::new(&[
            "chat",
            "start",
            "--project",
            FIRST_PROJECT,
            "--model",
            "gpt-6-luna",
            "--key",
            "change-on-plan",
            words,
            "--json",
        ])));
    }
    if first {
        // Chat creation answers before its delegate exists. As in the
        // first-repository story, wait for the chat's initial reply first.
        steps.push(Step::Run(Call::new(&["task", "show", "1", "--until", "replied", "--timeout", within, "--json"])));
    }
    let found_at = account_at + 1 + 2 * usize::from(first);
    steps.extend(match ends {
        Ends::Landed => super::first_repository::found_steps(),
        // The box's referee then waits for the chat's HELD reply to the
        // escalation; a chat refused before each run never gives it.
        Ends::Held => vec![
            Step::Run(Call::new(&["task", "show", "2", "--until", "held", "--timeout", within, "--json"])),
            Step::Run(Call::new(&["task", "show", "2", "--json"])),
        ],
    });
    // The referee holds ctl open, reads and pages the conversation, and
    // follows --since if the procedure's end has not yet reached the chat.
    steps.push(Step::Run(Call::new(&["task", "show", "1", "--json"])));
    (account_at, found_at)
}

pub fn assert_plan_box<T>(run: &str, first: bool, outcome: &super::boxed::CtlOutcome<T>, stderr: &str) {
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "{run}: {stderr}");
    assert_eq!(outcome.left, Some(vec![]), "{run}: {stderr}");
    for (index, call) in outcome.calls.iter().enumerate() {
        assert_eq!(
            call.exit,
            Some(Exit::Code(if !first && index == outcome.calls.len() - 1 { 2 } else { 0 })),
            "{run} call {index}: stdout {}; stderr {}; box {stderr}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
}

#[must_use]
pub fn deployment_flags(paths: &[std::path::PathBuf; 3]) -> Vec<String> {
    ["--config", "--data", "--cache"]
        .into_iter()
        .zip(paths)
        .flat_map(|(flag, path)| [flag.into(), path.display().to_string()])
        .collect()
}

#[must_use]
pub fn cache_forgejo_binaries(cache: &std::path::Path) -> ([std::path::PathBuf; 2], [std::path::PathBuf; 2]) {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    let (forgejo, runner) = super::owned::binaries();
    let cached = [
        cache.join(format!("forgejo/{VERSION}/forgejo")),
        cache.join(format!("forgejo-runner/{RUNNER_VERSION}/forgejo-runner")),
    ];
    let sources = [forgejo, runner];
    for (source, target) in sources.iter().zip(&cached) {
        cache_binary(source, target);
    }
    (cached, sources)
}

#[must_use]
pub fn forgejo_setup_steps(run: &str) -> Vec<super::ctl::Step> {
    use super::ctl::{Call, Step};
    let mut steps = vec![Step::Mark("start"), Step::Run(Call::sign_in())];
    if run == "first" {
        steps.extend([
            Step::Run(Call::new(&["forge", "list"])),
            Step::Run(Call::new(&["forge", "list", "--json"])),
            Step::Run(Call::new(&["forge", "setup", "3"])),
        ]);
    }
    // Real preparation, hashing two releases, runner registration and reconcile
    // take seconds; allow three minutes, plus ten seconds to stop and reap.
    steps.extend([
        Step::Run(Call::new(&["forge", "list", "--until", "settled", "--timeout", "180s"])),
        Step::Run(Call::new(&["forge", "list", "--json"])),
        Step::Run(Call::new(&["forge", "list"])),
    ]);
    steps
}

pub fn cache_binary(source: &std::path::Path, target: &std::path::Path) {
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, PermissionsExt},
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(target.parent().expect("cache parent"))
        .expect("private binary cache");
    fs::copy(source, target).expect("copy pinned executable, never link it");
    fs::set_permissions(target, fs::Permissions::from_mode(0o700)).expect("private executable");
}

#[must_use]
pub fn assert_forgejo_setup(
    run: &str,
    calls: &[super::Ended],
    stderr: &str,
    previous: Option<u64>,
    data: &std::path::Path,
) -> u64 {
    let signed = &calls[0];
    assert_eq!(
        signed.exit,
        Some(Exit::Code(0)),
        "{run}: sign-in: {}; box: {stderr}",
        String::from_utf8_lossy(&signed.stderr)
    );
    assert_forgejo_setup_after_sign_in(run, &calls[1..], stderr, previous, data)
}

/// The setup reads also serve a box whose signed-in person first tried the demo.
#[must_use]
pub fn assert_forgejo_setup_after_sign_in(
    run: &str,
    calls: &[super::Ended],
    stderr: &str,
    previous: Option<u64>,
    data: &std::path::Path,
) -> u64 {
    for call in calls {
        assert_eq!(
            call.exit,
            Some(Exit::Code(0)),
            "{run}: ctl stdout: {}; stderr: {}; box: {stderr}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    assert_eq!(calls.len(), if run == "first" { 6 } else { 3 }, "all setup calls ran: {stderr}");
    if run == "first" {
        assert!(
            String::from_utf8_lossy(&calls[0].stdout)
                .lines()
                .any(|line| line == "forge 3 · Forgejo · owned · Not set up")
        );
        let offered = super::ctl::json(&calls[1]);
        let forge = offered["forges"]
            .as_array()
            .expect("forge rows")
            .iter()
            .find(|forge| forge["number"] == 3)
            .expect("forge 3 row");
        assert_eq!(forge["offer"], "offered");
        assert_eq!(forge["state"], "not_set_up");
        assert!(calls[2].stdout.starts_with(b"setting up\n"));
        let adoption = String::from_utf8_lossy(&calls[2].stdout);
        assert!(
            adoption.contains("Adopted work/hello on forge 3 as owned."),
            "ctl printed the first adoption: {adoption}"
        );
        eprintln!("{run} first repository adoption: {adoption}");
        let made = super::ctl::json(&calls[4]);
        let row =
            made["forges"].as_array().expect("forges rows").iter().find(|row| row["number"] == 3).expect("forge 3");
        eprintln!("{run} first repository read: {}", row["first"]);
        assert_eq!(row["first"]["state"], "made");
        assert_eq!(row["first"]["project"]["new"], "hello");
        assert_eq!(row["first"]["org"], "work");
        assert_eq!(row["first"]["repository"], "hello");
        assert_eq!(row["first"]["id"], 1, "the script tags forge 3, repository 1");
        assert_eq!(row["state"], "connected");
        // Preparation is silent on success; Forgejo's startup
        // and the runner's lines reach the box with their step prefixes.
        for step in ["forgejo 3: ", "runner 3: "] {
            assert!(stderr.lines().any(|line| line.starts_with(step)), "group step {step}: {stderr}");
        }
    } else {
        assert!(
            !stderr.lines().any(|line| line.starts_with("fetch ") || line.contains("download")),
            "the restarted group fetches nothing: {stderr}"
        );
    }
    let stdout = String::from_utf8_lossy(&calls.last().expect("forges read").stdout);
    let user = stdout.lines().find_map(|line| {
        line.strip_prefix("forge 3 · Forgejo · owned · Connected as temper (")?.split(')').next()?.parse::<u64>().ok()
    });
    assert!(user.is_some_and(|user| user != 0), "{run}: forges row: {stdout}; box: {stderr}");
    let user = user.expect("connected bot's ID");
    if let Some(previous) = previous {
        assert_eq!(user, previous, "the second box connects as the same bot");
    }
    for name in ["admin_token", "api_token", "git_token", "user", "webhook_secret"] {
        assert!(!data.join("forge-3").join(name).exists(), "{run}: no forge-3/{name}");
    }
    user
}

/// The SUT's cached copy still contains every byte of its pinned source.
pub fn assert_cached_binary_unchanged(cached: &std::path::Path, source: &std::path::Path) {
    use std::io::Read;
    let mut cached = std::fs::File::open(cached).expect("cached binary");
    let mut source = std::fs::File::open(source).expect("pinned source binary");
    assert_eq!(
        cached.metadata().expect("cached metadata").len(),
        source.metadata().expect("source metadata").len(),
        "cached binary unchanged"
    );
    let mut cached_bytes = vec![0; 262_144].into_boxed_slice();
    let mut source_bytes = vec![0; 262_144].into_boxed_slice();
    loop {
        let count = cached.read(&mut cached_bytes).expect("cached binary chunk");
        source.read_exact(&mut source_bytes[..count]).expect("pinned source chunk");
        assert!(cached_bytes[..count] == source_bytes[..count], "cached binary unchanged");
        if count == 0 {
            assert_eq!(source.read(&mut source_bytes).expect("pinned source EOF"), 0, "cached binary unchanged");
            break;
        }
    }
}

/// Init's shipped charters on the process story's three roots. The model
/// choice changes only ordinary provider and program configuration.
#[must_use]
pub fn live_deployment(paths: &[PathBuf; 3], model: LiveModel<'_>) -> std::net::SocketAddr {
    initialize(paths);
    if let LiveModel::ClaudeCode { .. } = model {
        let executable = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .filter(|directory| directory.is_absolute())
            .map(|directory| directory.join("claude"))
            .find(|path| {
                fs::metadata(path)
                    .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            })
            .expect("Claude Code must be installed on PATH");
        configure_live_model(&paths[0], model, &executable);
    }
    let path = paths[0].join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    value["engine"]["limits"]["overrides"]["accounts.sign_in_deadline_ms"] = 600_000.into();
    super::write_private(&path, &serde_json::to_vec(&value).expect("live configuration"));
    value["application"]["forges"]
        .as_array()
        .expect("forges")
        .iter()
        .find(|forge| forge["number"] == 3)
        .expect("owned forge")["api"]["socket"]
        .as_str()
        .expect("forge address")
        .parse()
        .expect("socket")
}

/// Select the launcher's store and admit it before starting any process; a
/// held account moves into a fresh store only.
#[must_use]
pub fn live_store(account: bool) -> (PathBuf, bool) {
    let store = std::path::PathBuf::from(
        std::env::var_os("TEMPER_LIVE_STORE").expect("run live through scripts/proof-live.sh forge-land"),
    );
    let kept = reserve_live_store(&store).and_then(|kept| {
        if kept && account { Err("a held account moves into a fresh store only".to_owned()) } else { Ok(kept) }
    });
    let kept = kept.unwrap_or_else(|message| {
        super::live::notice(&message);
        panic!("{message}")
    });
    (store, kept)
}

/// The held record `TEMPER_LIVE_ACCOUNT` names, admitted before any store is.
#[must_use]
pub fn live_account() -> Option<PathBuf> {
    let account = PathBuf::from(std::env::var_os("TEMPER_LIVE_ACCOUNT")?);
    if let Err(message) = held_account(&account) {
        super::live::notice(message);
        panic!("{message}");
    }
    Some(account)
}

/// Admit the held `ChatGPT` record a fresh store's first run moves in: a
/// regular file only its owner reads, as ctl account import's FILE must be. Its words
/// never name the path, which goes to `account import` alone.
pub fn held_account(file: &std::path::Path) -> Result<(), &'static str> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(file).map_err(|_| "TEMPER_LIVE_ACCOUNT names no file")?;
    if !metadata.file_type().is_file() {
        return Err("TEMPER_LIVE_ACCOUNT is not a regular file");
    }
    if metadata.mode() & 0o7777 != 0o600 {
        return Err("TEMPER_LIVE_ACCOUNT is not 0600");
    }
    Ok(())
}

/// A live command as notices and failures print it: an imported record's path
/// stays out of them, and ctl's own words are all that is said of it.
#[must_use]
pub fn printed_command(args: &[String]) -> Vec<String> {
    match args {
        [account, import, _file, rest @ ..] if account == "account" && import == "import" => {
            [account.clone(), import.clone(), "FILE".into()].into_iter().chain(rest.iter().cloned()).collect()
        }
        _ => args.to_vec(),
    }
}

/// A unique marker for this live sitting, carried into its README and notice.
#[must_use]
pub fn live_tag() -> String {
    let minute = std::process::Command::new("/usr/bin/date")
        .args(["-u", "+%Y-%m-%dT%H:%MZ"])
        .env_clear()
        .output()
        .expect("UTC ISO minute");
    assert!(minute.status.success());
    format!("live-{}-{}", String::from_utf8(minute.stdout).expect("date UTF-8").trim(), std::process::id())
}

/// Reserve a fresh store, or validate a kept deployment using metadata only.
/// An empty directory is fresh; an incomplete deployment is never repaired.
pub fn reserve_live_store(path: &std::path::Path) -> Result<bool, String> {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("create kept store {}: {error}", path.display())),
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    require(metadata.is_dir(), &format!("kept store {} is not a directory", path.display()))?;
    require(metadata.permissions().mode() & 0o777 == 0o700, "kept store must have mode 0700")?;
    if fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Ok(false);
    }
    // The deployment alone: a rerun makes its own client roots, and a sitting
    // that failed before its restart never made `second`.
    for (name, directory) in [
        ("config", true),
        ("data", true),
        ("cache", true),
        ("data/deployment", true),
        ("data/forge-3/forgejo", true),
        ("config/temper.json", false),
        ("config/agent.json", false),
        ("data/box/started/3", false),
        ("data/forge-3/forgejo/app.ini", false),
        ("data/forge-3/forgejo/forgejo.db", false),
    ] {
        let present = fs::symlink_metadata(path.join(name))
            .is_ok_and(|meta| if directory { meta.is_dir() } else { meta.is_file() });
        require(present, &format!("kept store {} lacks {}", path.display(), name))?;
    }
    for name in
        [format!("cache/forgejo/{VERSION}/forgejo"), format!("cache/forgejo-runner/{RUNNER_VERSION}/forgejo-runner")]
    {
        require(
            fs::symlink_metadata(path.join(&name)).is_ok_and(|meta| meta.is_file()),
            &format!("kept store {} lacks {}", path.display(), name),
        )?;
    }
    Ok(true)
}

pub const KEPT_SIGN_IN_EXPIRED: &str = "the kept ChatGPT sign-in is expired or refused; the person signs in afresh";

/// The forge's durable identity, read from this run's private owned repository.
#[derive(Debug, PartialEq, Eq)]
pub struct ForgeIdentity {
    pub number: u64,
    pub url: String,
    pub merge: String,
    pub head: String,
    pub branches: Vec<(String, String)>,
}

pub struct LiveEvidence {
    pub task: String,
    pub page: String,
    pub completions: u32,
    pub address: std::net::SocketAddr,
    pub tasks: LiveTasks,
    pub baseline: std::collections::BTreeSet<u64>,
    pub roots: std::collections::BTreeSet<u64>,
    pub forge: ForgeIdentity,
    pub replies: std::collections::BTreeMap<u64, serde_json::Value>,
    pub delegates: [Vec<u64>; 2],
}

fn pull_number(pull: &serde_json::Value) -> u64 {
    pull["number"].as_u64().expect("pull number")
}
fn merged_pulls(address: std::net::SocketAddr, token: &str) -> Vec<serde_json::Value> {
    let mut merged = Vec::new();
    for page in 1..=100 {
        let pulls = super::owned::answer(
            address.port(),
            &format!("/api/v1/repos/work/hello/pulls?state=closed&limit=100&page={page}"),
            Some(token),
        )
        .expect("owned forge pulls");
        let pulls = pulls.as_array().expect("pull list");
        if pulls.is_empty() {
            return merged;
        }
        merged.extend(pulls.iter().filter(|pull| pull["merged"] == true).cloned());
    }
    panic!("merged pull pagination exceeded 100 pages")
}

#[must_use]
pub fn last_readme_line(readme: &[u8]) -> Option<&str> {
    std::str::from_utf8(readme).expect("README UTF-8").lines().rfind(|line| !line.is_empty())
}

fn forge_identity(
    address: std::net::SocketAddr,
    line: &str,
    token: &str,
    baseline: &std::collections::BTreeSet<u64>,
) -> ForgeIdentity {
    use skein_lib::base64::{self, Alphabet};
    let get = |suffix: &str| {
        super::owned::answer(address.port(), &format!("/api/v1/repos/work/hello/{suffix}"), Some(token))
            .unwrap_or_else(|why| panic!("owned forge read {suffix}: {why}"))
    };
    let pulls = merged_pulls(address, token);
    let numbers: std::collections::BTreeSet<_> = pulls.iter().map(pull_number).collect();
    assert!(baseline.is_subset(&numbers), "previous merged pulls survive");
    assert_eq!(numbers.len(), baseline.len() + 1, "exactly one more merged pull");
    let pull = pulls.iter().find(|pull| !baseline.contains(&pull_number(pull))).expect("new merged pull");
    let head = pull["head"]["sha"].as_str().expect("pull head").to_owned();
    let status = get(&format!("commits/{head}/status"));
    assert_eq!(status["state"], "success", "CI at the merged head");
    assert!(!status["statuses"].as_array().expect("CI jobs").is_empty(), "a CI job ran");
    let readme = get(&format!("contents/README.md?ref={head}"));
    assert_eq!(readme["encoding"], "base64");
    let encoded: Vec<_> = readme["content"]
        .as_str()
        .expect("README content")
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let readme = base64::decode(Alphabet::Standard, &encoded, 1 << 20).expect("README base64");
    assert_eq!(last_readme_line(&readme), Some(line), "the merged pull head ends with the tagged line");
    let branches = get("branches?limit=100&page=1");
    assert_eq!(get("branches?limit=100&page=2"), serde_json::json!([]));
    let mut branches: Vec<_> = branches
        .as_array()
        .expect("branches")
        .iter()
        .map(|branch| {
            (
                branch["name"].as_str().expect("branch name").to_owned(),
                branch["commit"]["id"].as_str().expect("branch head").to_owned(),
            )
        })
        .collect();
    branches.sort();
    let merge = pull["merge_commit_sha"].as_str().expect("merge hash").to_owned();
    assert!(branches.iter().any(|(name, head)| name == "main" && head == &merge), "main is at the pull's merge commit");
    ForgeIdentity {
        number: pull["number"].as_u64().expect("pull number"),
        url: pull["html_url"].as_str().expect("pull URL").to_owned(),
        merge,
        head,
        branches,
    }
}

/// The starter is private. Mint an independent, read-only oracle credential on
/// this disposable forge, rather than reading temper's stored credentials.
fn oracle_token(paths: &[PathBuf; 3], first: bool) -> String {
    scoped_token(paths, first, "temper", "read:repository,read:issue")
}

/// A separate fixture credential, never one of temper's stored credentials.
#[must_use]
pub(super) fn scoped_token(paths: &[PathBuf; 3], first: bool, login: &str, scopes: &str) -> String {
    use std::{
        io::Read,
        process::{Command, Stdio},
    };
    let forge = paths[1].join("forge-3/forgejo");
    let (binary, _) = super::owned::binaries();
    let mut child = Command::new(binary)
        .arg("--work-path")
        .arg(&forge)
        .arg("--config")
        .arg(forge.join("app.ini"))
        .args([
            "admin",
            "user",
            "generate-access-token",
            "--username",
            login,
            "--token-name",
            &format!(
                "live-oracle-{}-{}-{}",
                std::process::id(),
                if first { "first" } else { "restart" },
                skein_shell::Clock::new().now().now.as_nanos()
            ),
            "--scopes",
            scopes,
            "--raw",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        // The owned forge commands use this same private home.
        .env("HOME", paths[1].join("forge-3/home"))
        .current_dir(&forge)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("oracle token command");
    let clock = skein_shell::Clock::new();
    let until = clock.now().now.saturating_add(Duration::from_secs(10));
    loop {
        if let Some(status) = child.try_wait().expect("oracle token status") {
            assert!(status.success(), "read-only oracle token creation failed");
            break;
        }
        if clock.now().now >= until {
            child.kill().expect("stop expired oracle command");
            child.wait().expect("reap oracle command");
            panic!("oracle token creation exceeded ten seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut bytes = Vec::new();
    child.stdout.take().expect("oracle token output").take(1024).read_to_end(&mut bytes).expect("oracle token bytes");
    let token = String::from_utf8(bytes).expect("oracle token encoding");
    let token = token.trim();
    assert_eq!(token.len(), 40, "oracle token length");
    assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()), "oracle token format");
    token.to_owned()
}

/// Commands are admitted one at a time so a failure cannot start the next act.
/// A fresh store's account comes from a person's sign-in, or from a held
/// record moved in by `import`, as qa-live.md's 0b seed does (4.3), or a
/// held Claude Code directory declared and verified on the worker.
#[must_use]
pub fn live_commands(fresh: bool, model: LiveModel<'_>) -> (Vec<Vec<String>>, usize) {
    let mut commands = vec![vec!["sign-in", "-"]];
    if fresh {
        commands.extend([vec!["forge", "list"], vec!["forge", "list", "--json"], vec!["forge", "setup", "3"]]);
    }
    commands.extend([
        vec!["forge", "list", "--until", "settled", "--timeout", "180s"],
        vec!["forge", "list", "--json"],
        vec!["forge", "list"],
        vec!["me", "--json"],
    ]);
    if fresh {
        match model {
            LiveModel::ChatGpt { account } => {
                commands.extend(match account.map(|file| file.to_str().expect("held account path is UTF-8")) {
                    Some(file) => [
                        vec!["account", "import", file, "--key", "connect-plan"],
                        vec!["account", "list", "--until", "usable", "--timeout", "600s", "--json"],
                    ],
                    None => [
                        vec!["account", "connect", "1", "--key", "connect-plan"],
                        vec!["account", "list", "--until", "settled", "--timeout", "600s", "--json"],
                    ],
                });
            }
            LiveModel::ClaudeCode { login } => {
                commands.push(vec![
                    "account",
                    "declare",
                    "2",
                    "claude_code",
                    source_name(login),
                    "--key",
                    "live-program",
                ]);
            }
        }
    }
    let account = commands.len();
    commands.push(vec!["account", "list", "--json"]);
    (commands.into_iter().map(|args| args.into_iter().map(str::to_owned).collect()).collect(), account)
}

/// The live root task numbers of a `task list --project PROJECT --json` read, whole.
pub fn live_roots(read: &serde_json::Value) -> Result<std::collections::BTreeSet<u64>, String> {
    require(read["tasks"]["more"] == false, "whole live root tasks read")?;
    read["tasks"]["rows"]
        .as_array()
        .ok_or("live root task rows missing")?
        .iter()
        .map(|row| row["chip"]["task"].as_u64().ok_or_else(|| "live root task number missing".to_owned()))
        .collect()
}

/// After the chat closes, the project's live roots are exactly those before this sitting.
pub fn require_closed_roots(read: &serde_json::Value, before: &std::collections::BTreeSet<u64>) -> Result<(), String> {
    let after = live_roots(read)?;
    let extra = after.difference(before).copied().collect::<Vec<_>>();
    let missing = before.difference(&after).copied().collect::<Vec<_>>();
    require(
        extra.is_empty() && missing.is_empty(),
        &format!("live root tasks this sitting left: {extra:?}; earlier live root tasks missing: {missing:?}"),
    )
}

/// The held source selects the same story's model, without copying a login.
#[derive(Clone, Copy)]
pub enum LiveModel<'a> {
    ChatGpt { account: Option<&'a std::path::Path> },
    ClaudeCode { login: &'a std::path::Path },
}

fn source_name(login: &std::path::Path) -> &str {
    let name = login.file_name().and_then(|name| name.to_str()).expect("held login source name");
    let _name = smith_host_domain::SourceName::new(name.as_bytes()).expect("valid held login source name");
    name
}

impl LiveModel<'_> {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ChatGpt { .. } => "gpt-6-luna",
            Self::ClaudeCode { .. } => "sonnet",
        }
    }
    const fn unavailable(self) -> &'static str {
        match self {
            Self::ChatGpt { .. } => KEPT_SIGN_IN_EXPIRED,
            Self::ClaudeCode { .. } => "the held Claude Code login is not usable; stop and return it to its holder",
        }
    }
    pub fn ready(self, value: &serde_json::Value) -> Result<(), String> {
        match self {
            Self::ChatGpt { .. } => super::live::ready_account(value),
            Self::ClaudeCode { login } => {
                let account = declared_program(value, source_name(login))?;
                require(
                    account["sources"].as_array().ok_or("account sources missing")?.iter().any(|source| {
                        source["kind"] == "program" && source["name"] == source_name(login) && source["usable"] == true
                    }),
                    "held Claude Code source is not usable",
                )?;
                let hosts = value["accounts"]["program_hosts"].as_array().ok_or("program hosts missing")?;
                require(
                    hosts.iter().any(|host| {
                        host["name"] == "worker"
                            && host["connected"] == true
                            && host["programs"].as_array().is_some_and(|programs| {
                                programs.iter().any(|program| {
                                    program["name"] == source_name(login) && program["state"] == "usable"
                                })
                            })
                    }),
                    "held Claude Code login is not usable on the worker",
                )
            }
        }
    }
}

fn declared_program<'a>(value: &'a serde_json::Value, name: &str) -> Result<&'a serde_json::Value, String> {
    let accounts = value["accounts"]["party"].as_array().ok_or("person's accounts missing")?;
    require(accounts.iter().all(|account| account["provider"] == 2), "only the Claude Code provider has an account")?;
    accounts
        .iter()
        .find(|account| {
            account["sources"].as_array().is_some_and(|sources| {
                sources.iter().any(|source| source["kind"] == "program" && source["name"] == name)
            })
        })
        .ok_or_else(|| "held Claude Code account missing".to_owned())
}

/// Verify the declared account number returned by ctl, then wait for the host.
pub fn program_verification(login: &std::path::Path, accounts: &serde_json::Value) -> Result<Vec<Vec<String>>, String> {
    let account = declared_program(accounts, source_name(login))?["number"]
        .as_u64()
        .ok_or("program account number missing")?
        .to_string();
    Ok([
        vec!["account", "verify", &account, "--source", "program", "--json"],
        vec!["account", "list", "--until", "usable", "--timeout", "600s", "--json"],
        vec!["account", "list", "--json"],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(str::to_owned).collect())
    .collect())
}

/// Write the ordinary provider and agent configuration; inspect no login entry.
/// A single program model makes both stock charters choose it at startup.
pub fn configure_live_model(config: &std::path::Path, model: LiveModel<'_>, executable: &std::path::Path) {
    let LiveModel::ClaudeCode { login } = model else { return };
    let path = config.join("temper.json");
    let agent_path = config.join("agent.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("deployment")).expect("JSON");
    let mut agent: serde_json::Value = serde_json::from_slice(&fs::read(&agent_path).expect("agent")).expect("JSON");
    let mut endpoint = agent["endpoints"][0].clone();
    endpoint["name"] = "claude".into();
    endpoint["number"] = 2.into();
    endpoint["dialect"] = 2.into();
    endpoint["provider"] = "anthropic".into();
    endpoint["program"] = "claude_code".into();
    endpoint["address"] = "api.anthropic.com:443".into();
    endpoint["server_name"] = "api.anthropic.com".into();
    endpoint["models"][0]["name"] = model.name().into();
    let mut declared = value["engine"]["llm"]["models"][0].clone();
    declared["name"] = model.name().into();
    declared["endpoint"] = "claude".into();
    let mut route = value["engine"]["llm"]["endpoints"][0].clone();
    for key in ["name", "number", "dialect", "provider", "program", "server_name"] {
        route[key] = endpoint[key].clone();
    }
    route["transport"] = "tls".into();
    route["address"] = serde_json::json!({"host":"api.anthropic.com", "port":443});
    value["engine"]["llm"] = serde_json::json!({
        "providers":[{"name":"anthropic", "number":2, "signed_in_plan":false,
            "manage_usage":"https://claude.ai/settings/usage", "models":[model.name()],
            "sources":{"programs":["claude_code"]}}],
        "endpoints":[route], "models":[declared]
    });
    // The demo's LLM binding requires its own provider. This sitting uses
    // only the held program provider and never offers the demo.
    value["application"]["demo"] = serde_json::Value::Null;
    agent["endpoints"] = serde_json::json!([endpoint]);
    agent["programs"] = serde_json::json!({"claude_code":{
        "path":executable, "directory":login.parent().expect("held login parent")
    }});
    super::write_private(&path, &serde_json::to_vec(&value).expect("live configuration"));
    super::write_private(&agent_path, &serde_json::to_vec(&agent).expect("live agent"));
}

pub struct LivePlan<'a> {
    pub paths: &'a [PathBuf; 3],
    pub client: &'a std::path::Path,
    pub first: bool,
    pub kept: bool,
    pub model: LiveModel<'a>,
    pub address: Option<std::net::SocketAddr>,
    pub tasks: Option<LiveTasks>,
    pub baseline: Option<&'a std::collections::BTreeSet<u64>>,
    pub roots: Option<&'a std::collections::BTreeSet<u64>>,
    pub words: &'a str,
    pub line: &'a str,
}

impl LivePlan<'_> {
    fn starting_phase(&self) -> LivePhase {
        if self.first && !self.kept && matches!(self.model, LiveModel::ClaudeCode { .. }) {
            LivePhase::ProgramDeclared
        } else {
            LivePhase::Setup
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveTasks {
    pub project: u64,
    pub chat: u64,
    pub change: u64,
}

/// Only committed assistant text at or after the child's end counts as its reply.
pub fn replies_after(
    activity: &serde_json::Value,
    ended_at: u64,
) -> Result<std::collections::BTreeMap<u64, serde_json::Value>, String> {
    let mut replies = std::collections::BTreeMap::new();
    for row in activity["rows"].as_array().ok_or("chat activity missing")? {
        if row["at"].as_u64().ok_or("activity timestamp missing")? < ended_at {
            continue;
        }
        let Some(turn) = row.pointer("/event/owner/decoded/turn/decoded") else { continue };
        require(turn["cut"] == false, "whole reply turn")?;
        for message in turn["messages"].as_array().ok_or("turn messages missing")? {
            if message["speaker"] == "assistant"
                && message["blocks"].as_array().ok_or("message blocks missing")?.iter().any(|block| {
                    block
                        .pointer("/text/text")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
                })
            {
                replies.insert(row["position"].as_u64().ok_or("reply position missing")?, turn.clone());
            }
        }
    }
    Ok(replies)
}

/// Names from actual committed call blocks, never from the model's prose.
#[must_use]
pub fn successful_tools(activity: &serde_json::Value) -> std::collections::BTreeSet<String> {
    let Some(rows) = activity["rows"].as_array() else { return std::collections::BTreeSet::new() };
    committed_tools(rows)
        .filter(|call| call["failed"] == false && !call["outcome"].is_null())
        .filter_map(|call| call["tool"].as_str().map(str::to_owned))
        .collect()
}

fn committed_tools(rows: &[serde_json::Value]) -> impl Iterator<Item = &serde_json::Value> {
    rows.iter()
        .filter_map(|row| {
            row.pointer("/event/owner/decoded/turn/decoded/messages").and_then(serde_json::Value::as_array)
        })
        .flatten()
        .filter(|message| message["speaker"] == "assistant")
        .flat_map(|message| message["blocks"].as_array().into_iter().flatten())
        .filter_map(|block| block.get("call"))
}

pub fn require_chat_progress(task: &serde_json::Value) -> Result<(), String> {
    let bad = "chat progress facts missing";
    let owner = task.pointer("/chip/details/owner").ok_or(bad)?;
    let phase = owner.get("phase").ok_or(bad)?;
    if phase.get("active").and_then(serde_json::Value::as_str) != Some("idle") {
        return Ok(());
    }
    let details = &owner["details"];
    let attempt = details.get("attempt").ok_or(bad)?;
    require(attempt.is_null() || attempt.is_u64(), bad)?;
    let children = details["live_delegates"].as_u64().ok_or(bad)?;
    let waits = task.get("waits_on").ok_or(bad)?;
    let run_wait = task.get("run_wait").ok_or(bad)?;
    let entries = task["subscriptions"]["entries"].as_array().ok_or(bad)?;
    let cut = task["cut"].as_bool().ok_or(bad)?;
    let wake = task.get("wake").ok_or(bad)?;
    let shipped = serde_json::json!({
        "words":"immediate", "notices":"immediate", "news":"immediate",
        "results":"last_or_failure", "questions":true, "answers":true, "timers":true
    });
    let empty_park = !cut
        && !attempt.is_null()
        && children == 0
        && waits == "news"
        && run_wait.is_null()
        && entries.is_empty()
        && wake == &shipped;
    require(!empty_park, "chat parked without delegating a change")
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.to_owned()) }
}
fn live_json(call: &super::Ended) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&call.stdout).map_err(|error| format!("live ctl JSON: {error}"))
}

enum LivePhase {
    ProgramDeclared,
    Setup,
    Settings,
    SettingsChanged,
    Roots,
    Created,
    Delegates,
    Waiting,
    Progress,
    Snapshot,
    Pages,
    Closing,
    Done,
    Stopping,
    Settled,
}
struct LiveJudge<'a> {
    plan: LivePlan<'a>,
    visitor: Option<usize>,
    account_at: usize,
    tasks: LiveTasks,
    baseline: std::collections::BTreeSet<u64>,
    roots: std::collections::BTreeSet<u64>,
    address: Option<std::net::SocketAddr>,
    token: Option<String>,
    found_at: usize,
    checked: usize,
    pending: std::collections::VecDeque<Vec<String>>,
    arguments: Vec<Vec<String>>,
    active_until: Option<skein_lib::Time>,
    wait_started: Option<skein_lib::Time>,
    wait_until: Option<skein_lib::Time>,
    wait_call: Option<usize>,
    failure: &'a std::cell::Cell<Option<String>>,
    signing_until: Option<skein_lib::Time>,
    account_checked: bool,
    linked: bool,
    cleanup: bool,
    phase: LivePhase,
    snapshot: Option<String>,
    replies: std::collections::BTreeMap<u64, serde_json::Value>,
    tasks_at: usize,
    tools: std::collections::BTreeSet<String>,
    ended_at: Option<u64>,
    pages: usize,
    evidence: &'a std::cell::Cell<Option<LiveEvidence>>,
    deadline: skein_lib::Time,
    poll: skein_lib::Time,
}
impl LiveJudge<'_> {
    fn advance(&mut self, now: skein_lib::Time, calls: &mut super::ctl::Calls) -> Result<(), String> {
        if !calls.foreground_idle() || !self.pending.is_empty() {
            return Ok(());
        }
        if matches!(self.phase, LivePhase::SettingsChanged) {
            self.queue(&["task", "list", "--project", &self.tasks.project.to_string(), "--json"]);
            self.phase = LivePhase::Roots;
            return Ok(());
        }
        let last = live_json(calls.ends().last().ok_or("live read missing")?)?;
        let chat = self.tasks.chat.to_string();
        let change = self.tasks.change.to_string();
        let project = self.tasks.project.to_string();
        match self.phase {
            LivePhase::ProgramDeclared => {
                let LiveModel::ClaudeCode { login } = self.plan.model else { unreachable!("program model") };
                let commands = program_verification(login, &last)?;
                self.account_at = self.arguments.len() + commands.len() - 1;
                self.pending.extend(commands);
                self.phase = LivePhase::Setup;
            }
            LivePhase::Setup => self.setup(now, calls)?,
            LivePhase::Settings => self.settings(calls)?,
            LivePhase::SettingsChanged => unreachable!("settings change handled before the JSON reads"),
            LivePhase::Roots => {
                self.roots = live_roots(&last)?;
                let key = format!("live-chat-{}", now.as_nanos());
                self.queue(&[
                    "chat",
                    "start",
                    "--project",
                    &project,
                    "--model",
                    self.plan.model.name(),
                    "--key",
                    &key,
                    self.plan.words,
                    "--json",
                ]);
                self.phase = LivePhase::Created;
            }
            LivePhase::Created => {
                self.tasks.chat = last["outcome"]["done"]["owner"]["created"]["first"]
                    .as_u64()
                    .ok_or("created chat number missing")?;
                self.wait_started = Some(now);
                self.wait_until = Some(now.saturating_add(Duration::from_secs(1800)));
                self.queue_delegates();
            }
            LivePhase::Delegates => self.find_delegates(now, calls, &last)?,
            LivePhase::Waiting => {
                self.wait_call = Some(calls.ends().len() - 1);
                if last["met"] == false {
                    self.queue(&["task", "show", &change, "--json"]);
                    self.queue(&["task", "list", "--project", &project, "--parent", &change, "--json"]);
                    self.queue(&["account", "list", "--json"]);
                    self.phase = LivePhase::Progress;
                } else {
                    // The person's signal that the change ended; the landing's checks follow.
                    super::live::notice(&format!(
                        "CHANGE ENDED {} after {}s: task{}={}",
                        if self.plan.first { "first box" } else { "after restart" },
                        self.wait_started.map_or(0, |started| now.saturating_since(started).as_nanos() / 1_000_000_000),
                        self.tasks.change,
                        last["task"]["chip"]["phase"].as_str().unwrap_or("?")
                    ));
                    self.found_at = calls.ends().len() - 1;
                    self.queue(&["task", "show", &change]);
                    self.queue(&["task", "show", &chat, "--json"]);
                    self.phase = LivePhase::Snapshot;
                }
            }
            LivePhase::Progress => self.progress(now, calls, &last)?,
            LivePhase::Snapshot => {
                let ended = live_json(&calls.ends()[self.found_at])?;
                require(ended["met"] == true, "change ended within its watch")?;
                self.ended_at = Some(
                    ended["task"]["chip"]["details"]["owner"]["details"]["ended_at"]
                        .as_u64()
                        .ok_or("change ended timestamp missing")?,
                );
                self.snapshot = Some(last["digest"].as_str().ok_or("chat digest missing")?.to_owned());

                self.pages = 0;
                self.queue(&["chat", "show", &chat, "--json"]);
                self.phase = LivePhase::Pages;
            }
            LivePhase::Pages => self.read_page(calls, &last)?,
            LivePhase::Closing => {
                self.check_landing(calls)?;
                calls.release();
                self.phase = LivePhase::Done;
            }
            LivePhase::Done | LivePhase::Stopping | LivePhase::Settled => {}
        }
        Ok(())
    }
    fn setup(&mut self, now: skein_lib::Time, calls: &super::ctl::Calls) -> Result<(), String> {
        let address = if let Some(address) = self.address {
            address
        } else {
            // first_repository::live's configuration socket read, without
            // its started-record or transcript reads on a kept deployment.
            let document: serde_json::Value = serde_json::from_slice(
                &fs::read(self.plan.paths[0].join("temper.json")).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            document["application"]["forges"]
                .as_array()
                .ok_or("forges missing")?
                .iter()
                .find(|forge| forge["number"] == 3)
                .ok_or("owned forge missing")?["api"]["socket"]
                .as_str()
                .ok_or("forge address missing")?
                .parse()
                .map_err(|_| "forge socket invalid")?
        };
        self.address = Some(address);
        let token = oracle_token(self.plan.paths, self.plan.first);
        if self.plan.first {
            let pulls = merged_pulls(address, &token);
            self.baseline = pulls.iter().map(pull_number).collect();
            if self.plan.kept {
                // The first-repository offer is transient and closed chats
                // are unlisted. The owned pull names its root task; ctl's
                // durable task document supplies the project it belongs to.
                let me = live_json(&calls.ends()[4])?;
                self.tasks.project = kept_project(self.plan.client, &me, &pulls)?;
            } else {
                let forges = live_json(&calls.ends()[5])?;
                let forge = forges["forges"]
                    .as_array()
                    .ok_or("forges missing")?
                    .iter()
                    .find(|forge| forge["number"] == 3)
                    .ok_or("owned forge missing")?;
                require(
                    forge["first"]["org"] == "work" && forge["first"]["repository"] == "hello",
                    "first repository is work/hello",
                )?;
                let made = forge["first"]["project"]["new"].as_str().ok_or("first project missing")?;
                self.tasks.project = live_json(&calls.ends()[7])?["projects"]
                    .as_array()
                    .ok_or("person's projects missing")?
                    .iter()
                    .find(|row| row["name"]["bytes"] == made)
                    .and_then(|row| row["project"].as_u64())
                    .ok_or("made first project missing")?;
            }
        }
        self.token = Some(token);
        if self.plan.first {
            // All fixed setup indices were consumed before these extra reads.
            if self.plan.kept {
                self.queue(&["task", "list", "--project", &self.tasks.project.to_string(), "--json"]);
                self.phase = LivePhase::Roots;
            } else {
                self.queue(&["settings", "show", "--json"]);
                self.queue(&["settings", "show", "charter", "1", "--json"]);
                self.queue(&["settings", "show", "charter", "2", "--json"]);
                self.phase = LivePhase::Settings;
            }
        } else {
            self.wait_started = Some(now);
            self.wait_until = Some(now.saturating_add(Duration::from_secs(1800)));
            self.wait_round(now)?;
        }

        Ok(())
    }
    fn settings(&mut self, calls: &super::ctl::Calls) -> Result<(), String> {
        let reads = &calls.ends()[calls.ends().len() - 3..];
        let revision = live_json(&reads[0])?["newest"].as_u64().ok_or("settings revision missing")?;
        let mut items = Vec::new();
        for read in &reads[1..] {
            let setting = live_json(read)?;
            let mut value = setting["value"].clone();
            require(value["model"]["name"] == self.plan.model.name(), "the selected live model in every charter")?;
            value["time"] = 240_000_000_000_u64.into();
            items.push(serde_json::json!({"setting":setting["setting"],"set":value}));
        }
        let file = self.plan.client.join("settings.json");
        let change = serde_json::json!({"items":items});
        fs::write(&file, serde_json::to_vec(&change).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        self.queue(&[
            "settings",
            "apply",
            file.to_str().ok_or("settings FILE path is not UTF-8")?,
            "--against",
            &revision.to_string(),
            "--key",
            "live-settings",
        ]);
        self.phase = LivePhase::SettingsChanged;
        Ok(())
    }
    fn find_delegates(
        &mut self,
        now: skein_lib::Time,
        calls: &super::ctl::Calls,
        last: &serde_json::Value,
    ) -> Result<(), String> {
        let mut delegates = Vec::new();
        for row in last["live"].as_array().ok_or("live delegates missing")? {
            delegates.push(row["chip"]["task"].as_u64().ok_or("delegate number missing")?);
        }
        let ended = &last["ended"]["rows"];
        require(
            last["live_after"].is_null() && ended["after"].is_null() && ended["cut"] == false,
            "whole chat delegation set",
        )?;
        for row in ended["rows"].as_array().ok_or("ended delegates missing")? {
            delegates.push(row["chip"]["task"].as_u64().ok_or("ended delegate number missing")?);
        }
        if delegates.is_empty() {
            let chat = live_json(calls.ends().iter().rev().nth(1).ok_or("chat snapshot missing")?)?;
            require_chat_progress(&chat)?;
            require(now < self.wait_until.ok_or("delegate deadline missing")?, "chat did not delegate within 1800s")?;
            self.queue(&[
                "task",
                "show",
                &self.tasks.chat.to_string(),
                "--until",
                "replied",
                "--timeout",
                "5s",
                "--json",
            ]);
            self.queue(&["account", "list", "--json"]);
            self.queue_delegates();
        } else {
            require(delegates.len() == 1, "one change delegated by the new chat")?;
            self.tasks.change = delegates[0];
            super::live::notice(&format!(
                "CHANGE task={} chat={} project={}",
                self.tasks.change, self.tasks.chat, self.tasks.project
            ));
            self.wait_round(now)?;
        }

        Ok(())
    }
    fn read_page(&mut self, calls: &super::ctl::Calls, last: &serde_json::Value) -> Result<(), String> {
        let chat = self.tasks.chat.to_string();
        let change = self.tasks.change.to_string();
        let project = self.tasks.project.to_string();
        self.pages += 1;
        require(self.pages <= 32, "bounded conversation pages")?;
        let activity = &last["activity"];
        require(activity["cut"] == false, "whole activity page")?;
        self.tools.extend(successful_tools(activity));
        self.replies.extend(replies_after(activity, self.ended_at.ok_or("change end missing")?)?);
        if let Some(before) = activity["before"]["value"].as_str() {
            self.queue(&["chat", "show", &chat, "--before", before, "--json"]);
        } else if self.replies.is_empty() {
            self.queue(&[
                "task",
                "show",
                &chat,
                "--until",
                "replied",
                "--since",
                &self.snapshot.clone().ok_or("snapshot before conversation missing")?,
                "--timeout",
                "5s",
                "--json",
            ]);
            self.queue(&["account", "list", "--json"]);
            self.queue(&["task", "show", &chat, "--json"]);
            self.phase = LivePhase::Snapshot;
        } else {
            self.tasks_at = calls.ends().len();
            for parent in [&chat, &change] {
                self.queue(&["task", "list", "--project", &project, "--parent", parent, "--ended", "--json"]);
            }
            if !self.plan.first {
                let key = format!("close-live-chat-{chat}");
                for args in [
                    &["chat", "close", &chat, "--key", &key, "--json"][..],
                    &["task", "tree", &chat, "--until", "ended", "--timeout", "300s", "--json"],
                    &["task", "list", "--project", &project, "--json"],
                ] {
                    self.queue(args);
                }
            }
            self.phase = LivePhase::Closing;
        }

        Ok(())
    }
    fn progress(
        &mut self,
        now: skein_lib::Time,
        calls: &super::ctl::Calls,
        _last: &serde_json::Value,
    ) -> Result<(), String> {
        let task = live_json(&calls.ends()[calls.ends().len() - 3])?;
        let last = live_json(&calls.ends()[calls.ends().len() - 2])?;
        let children = last["tasks"]["rows"]
            .as_array()
            .ok_or("children rows missing")?
            .iter()
            .map(|row| format!("{}:{}", row["chip"]["task"], row["chip"]["phase"].as_str().unwrap_or("?")))
            .collect::<Vec<_>>()
            .join(",");
        super::live::notice(&format!(
            "PROGRESS {} task{}={} children={} waits_on={} details={}",
            now.saturating_since(self.wait_started.ok_or("watch start missing")?).as_nanos() / 1_000_000_000,
            self.tasks.change,
            task["chip"]["phase"].as_str().unwrap_or("?"),
            children,
            task["waits_on"],
            task["chip"]["details"]
        ));
        if now >= self.wait_until.ok_or("watch deadline missing")? {
            self.fail("change watch exceeded 1800s", calls, self.wait_call);
        } else {
            self.wait_round(now)?;
        }
        Ok(())
    }
    fn stopping(&self) -> bool {
        matches!(self.phase, LivePhase::Stopping | LivePhase::Settled)
    }

    fn check_landing(&mut self, calls: &super::ctl::Calls) -> Result<(), String> {
        let last = live_json(calls.ends().last().ok_or("closing reads missing")?)?;
        if !self.plan.first {
            require(live_json(&calls.ends()[calls.ends().len() - 2])?["met"] == true, "chat tree closed")?;
            require_closed_roots(&last, &self.roots)?;
        }
        let page = live_change_page(client_address(calls)?, self.plan.client, self.tasks.change);
        require(self.tools.contains("change"), "the chat's committed change call succeeded")?;
        let mut delegates = [Vec::new(), Vec::new()];
        for (offset, tasks) in delegates.iter_mut().enumerate() {
            let value = live_json(&calls.ends()[self.tasks_at + offset])?;
            require(value["live"] == serde_json::json!([]) && value["live_after"].is_null(), "no live delegates")?;
            let ended = &value["ended"]["rows"];
            require(ended["cut"] == false, "whole delegate set")?;
            require(ended["after"].is_null(), "whole delegate set")?;
            *tasks = ended["rows"]
                .as_array()
                .ok_or("ended delegates missing")?
                .iter()
                .map(|row| row["chip"]["task"].as_u64().ok_or("delegate task missing"))
                .collect::<Result<_, _>>()?;
            tasks.sort_unstable();
        }
        require(delegates[0].contains(&self.tasks.change), "change task belongs to the chat that called change")?;
        require(!self.tools.contains("delegate"), "no raw delegate started the task")?;
        let mut tasks = vec![self.tasks.chat, self.tasks.change];
        tasks.extend(&delegates[1]);
        tasks.sort_unstable();
        tasks.dedup();
        let completions = super::live::ctl_completions(self.plan.client, &tasks)?;
        require((1..=25).contains(&completions), "live completion budget: 1–25")?;
        super::live::notice(&format!(
            "COMPLETIONS chat={} change={} count={completions}",
            self.tasks.chat, self.tasks.change
        ));
        let forge = forge_identity(
            self.address.ok_or("forge address missing")?,
            self.plan.line,
            self.token.as_deref().ok_or("oracle credential missing")?,
            &self.baseline,
        );
        self.evidence.set(Some(LiveEvidence {
            task: String::from_utf8_lossy(&calls.ends()[self.found_at + 1].stdout).into_owned(),
            page,
            completions,
            address: self.address.ok_or("forge address missing")?,
            tasks: self.tasks.clone(),
            baseline: self.baseline.clone(),
            roots: self.roots.clone(),
            forge,
            replies: std::mem::take(&mut self.replies),
            delegates,
        }));
        Ok(())
    }
}
impl LiveJudge<'_> {
    fn queue_delegates(&mut self) {
        self.queue(&["task", "show", &self.tasks.chat.to_string(), "--json"]);
        self.queue(&[
            "task",
            "list",
            "--project",
            &self.tasks.project.to_string(),
            "--parent",
            &self.tasks.chat.to_string(),
            "--ended",
            "--json",
        ]);
        self.phase = LivePhase::Delegates;
    }
    fn queue(&mut self, args: &[&str]) {
        self.pending.push_back(args.iter().map(|arg| (*arg).to_owned()).collect());
    }
    fn wait_round(&mut self, now: skein_lib::Time) -> Result<(), String> {
        let change = self.tasks.change.to_string();
        let left = self.wait_until.ok_or("watch deadline missing")?.saturating_since(now).as_nanos() / 1_000_000_000;
        self.queue(&[
            "task",
            "show",
            &change,
            "--until",
            "ended",
            "--timeout",
            &format!("{}s", left.clamp(1, 60)),
            "--json",
        ]);
        self.phase = LivePhase::Waiting;
        Ok(())
    }
    fn dump(&self, index: usize, call: &super::Ended) {
        super::live::notice(&format!(
            "EVIDENCE cwd={} argv=temper ctl client {:?} exit={:?}\nstdout: {}\nstderr: {}",
            self.plan.client.display(),
            self.arguments.get(index),
            call.exit,
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        ));
    }
    fn fail(&mut self, message: &str, calls: &super::ctl::Calls, failing: Option<usize>) {
        let chat = self.tasks.chat.to_string();
        let change = self.tasks.change.to_string();
        let project = self.tasks.project.to_string();
        let previous = self.failure.take();
        if previous.is_some() {
            self.failure.set(previous);
            return;
        }
        let account = if self.plan.kept || self.account_checked {
            self.diagnostic(&["account", "list", "--json"]).and_then(|read| live_json(&read).ok())
        } else {
            None
        };
        let message = if account.as_ref().is_some_and(|value| self.plan.model.ready(value).is_err()) {
            self.plan.model.unavailable()
        } else {
            message
        };
        let message = if let Some(index) = failing {
            format!("{message}: argv={:?} exit={:?}", self.arguments.get(index), calls.ends()[index].exit)
        } else {
            message.to_owned()
        };
        self.failure.set(Some(super::live::redact(&message)));
        super::live::notice(&format!("FAILURE {message}"));
        if let Some(index) = failing {
            self.dump(index, &calls.ends()[index]);
        } else if let Some(call) = calls.pending() {
            self.dump(calls.ends().len(), call);
        }
        // Each independent read uses ctl's existing 60s bound. Even an unknown
        // task or a broken read is evidence, never a reason to omit later reads.
        for args in [
            &["task", "show", &chat, "--json"][..],
            &["task", "show", &change, "--json"],
            &["task", "list", "--project", &project, "--json"],
        ] {
            self.diagnostic(args);
        }
        let mut before = None::<String>;
        for _ in 0..32 {
            let mut args = vec!["chat", "show", &chat, "--json"];
            if let Some(cursor) = &before {
                args.extend(["--before", cursor]);
            }
            let Some(read) = self.diagnostic(&args) else { break };
            let Ok(page) = serde_json::from_slice::<serde_json::Value>(&read.stdout) else { break };
            if let Some(rows) = page["activity"]["rows"].as_array() {
                for call in committed_tools(rows) {
                    super::live::notice(&format!(
                        "TOOL name={} failed={} outcome={}",
                        call["tool"], call["failed"], call["outcome"]
                    ));
                }
            }
            before = page["activity"]["before"]["value"].as_str().map(str::to_owned);
            if before.is_none() {
                break;
            }
        }
        self.pending.clear();
        self.phase = LivePhase::Done;
    }
    fn diagnostic(&self, args: &[&str]) -> Option<super::Ended> {
        if let Ok(read) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::ctl::alone(self.plan.client, super::ctl::Call::new(args))
        })) {
            // Do not copy conversation prose or tool arguments into notices.
            let stdout = if args.first() == Some(&"chat") {
                "(committed tools below)".into()
            } else {
                String::from_utf8_lossy(&read.stdout)
            };
            super::live::notice(&format!(
                "EVIDENCE cwd={} argv=temper ctl client {args:?} exit={:?}\nstdout: {stdout}\nstderr: {}",
                self.plan.client.display(),
                read.exit,
                String::from_utf8_lossy(&read.stderr)
            ));
            Some(read)
        } else {
            super::live::notice(&format!("EVIDENCE {args:?}: bounded read unavailable"));
            None
        }
    }
    fn check_calls(&mut self, now: skein_lib::Time, calls: &super::ctl::Calls) {
        for index in self.checked..calls.ends().len() {
            let call = &calls.ends()[index];
            if (self.plan.kept || self.account_checked)
                && self.arguments.get(index).is_some_and(|args| args == &["account", "list", "--json"])
                && live_json(call).and_then(|value| self.plan.model.ready(&value)).is_err()
            {
                self.fail(self.plan.model.unavailable(), calls, Some(index));
                break;
            }
            if (self.plan.kept || self.account_checked)
                && let Ok(value) = live_json(call)
            {
                let task = value.get("task").unwrap_or(&value);
                if matches!(
                    task.pointer("/run_wait/account/why").and_then(serde_json::Value::as_str),
                    Some("needs_sign_in" | "refused")
                ) {
                    self.fail(self.plan.model.unavailable(), calls, Some(index));
                    break;
                }
            }
            let not_yet = self
                .arguments
                .get(index)
                .is_some_and(|args| args.iter().any(|arg| matches!(arg.as_str(), "--until" | "--since")))
                && call.exit == Some(Exit::Code(3));
            if call.exit != Some(Exit::Code(0)) && !not_yet {
                self.fail("live ctl failed", calls, Some(index));
                break;
            }
            if self.plan.first
                && !self.plan.kept
                && matches!(self.plan.model, LiveModel::ChatGpt { .. })
                && index == self.account_at - 2
            {
                super::live::notice(&String::from_utf8_lossy(&call.stdout));
                if matches!(self.plan.model, LiveModel::ChatGpt { account: None }) {
                    self.signing_until = Some(now.saturating_add(Duration::from_secs(600)));
                }
            }
        }
        if calls.ends().len() > self.checked {
            self.active_until = None;
        }
        self.checked = calls.ends().len();
        if !matches!(self.phase, LivePhase::Done | LivePhase::Stopping | LivePhase::Settled)
            && !matches!(self.phase, LivePhase::ProgramDeclared)
            && !self.account_checked
            && calls.ends().len() > self.account_at
        {
            match live_json(&calls.ends()[self.account_at]).and_then(|value| self.plan.model.ready(&value)) {
                Ok(()) => self.account_checked = true,
                Err(message) => self.fail(
                    if self.plan.kept { self.plan.model.unavailable() } else { &message },
                    calls,
                    Some(self.account_at),
                ),
            }
        }
    }
    fn drive(
        &mut self,
        now: skein_lib::Time,
        binary: &mut skein_world::end_to_end::Binary,
        calls: &mut super::ctl::Calls,
    ) {
        use skein_shell::Host;
        calls.observe_box_stderr(binary.stderr(), now);
        if !self.linked && super::printed_link(calls.stdout()).is_some() {
            super::live::client_notice(&String::from_utf8_lossy(calls.link()));
            self.linked = true;
        }
        self.check_calls(now, calls);
        if !self.stopping() && !matches!(self.phase, LivePhase::Done) {
            if self.signing_until.is_some_and(|until| !self.account_checked && now >= until) {
                self.fail("ChatGPT sign-in exceeded 10 minutes", calls, None);
            } else if now >= self.deadline {
                self.fail("live story exceeded its budget", calls, None);
            } else if binary.exit_status().is_some() {
                super::live::notice(&format!(
                    "BOX exit={:?} stderr: {}",
                    binary.exit_status(),
                    String::from_utf8_lossy(binary.stderr())
                ));
                self.fail("live box stopped before its reads", calls, None);
            } else if self.active_until.is_some_and(|until| now >= until) {
                self.fail("live ctl exceeded its per-call bound", calls, None);
            } else if now >= self.poll {
                self.poll = now.saturating_add(Duration::from_secs(1));
            }
        }
        if !self.stopping() && !matches!(self.phase, LivePhase::Done) {
            if let Err(message) = self.advance(now, calls) {
                self.fail(&message, calls, calls.ends().len().checked_sub(1));
            }
            if calls.foreground_idle()
                && let Some(args) = self.pending.pop_front()
            {
                let seconds = args
                    .windows(2)
                    .find(|pair| pair[0] == "--timeout")
                    .and_then(|pair| pair[1].strip_suffix('s'))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                self.active_until = Some(now.saturating_add(Duration::from_secs(seconds + 60)));
                let words = args.iter().map(String::as_str).collect::<Vec<_>>();
                calls.push(super::ctl::Step::Run(if words == ["sign-in", "-"] {
                    super::ctl::Call::sign_in()
                } else {
                    super::ctl::Call::new(&words)
                }));
                self.arguments.push(printed_command(&args));
            }
        }
        if !self.stopping() && matches!(self.phase, LivePhase::Done) {
            calls.release();
            calls.cancel();
            binary.signal(skein_io::kernel::Signal::Terminate);
            self.phase = LivePhase::Stopping;
        }
        if !self.cleanup && now >= self.deadline && !binary.is_empty() {
            binary.signal_tree(skein_io::kernel::Signal::Kill);
            self.cleanup = true;
        }
    }
}
impl Referee<Process> for LiveJudge<'_> {
    fn act(&mut self, now: skein_lib::Time, procs: &mut [Process]) {
        if let Some(index) = self.visitor {
            let Process::Calls(calls) = &procs[1] else { return };
            if let Some(address) = calls.ends().iter().find_map(sign_in::authorization_address) {
                let address = address.to_vec();
                let Process::Visitor(visitor) = &mut procs[index] else { return };
                visitor.hand(&address);
                self.visitor = None;
            }
        }
        let (drivers, peers) = procs.split_at_mut(2);
        let [Process::Binary(binary), Process::Calls(calls)] = drivers else { return };
        // Legacy oracle assertions must never unwind through the world's Tree.
        // Their recorded failure is raised by the caller after run_live returns.
        if let Err(failed) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.drive(now, binary, calls))) {
            let message = failed
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| failed.downcast_ref::<&str>().copied())
                .unwrap_or("live referee validation failed");
            self.fail(message, calls, calls.ends().len().checked_sub(1));
            calls.release();
            calls.cancel();
            binary.signal(skein_io::kernel::Signal::Terminate);
            self.phase = LivePhase::Stopping;
        }
        if self.stopping() {
            for peer in peers {
                match peer {
                    Process::Peer(peer) => peer.shutdown(),
                    Process::Issuer(issuer) => issuer.shutdown(),
                    Process::Calls(_)
                    | Process::Binary(_)
                    | Process::Stdout(_)
                    | Process::Person(_)
                    | Process::Visitor(_) => {}
                }
            }
        }
    }
    fn observe(&mut self, _now: skein_lib::Time, procs: &[Process]) {
        use skein_shell::Host;
        if self.stopping() && procs.iter().all(Host::is_empty) {
            self.phase = LivePhase::Settled;
        }
    }
    fn next_deadline(&self) -> Option<skein_lib::Time> {
        if matches!(self.phase, LivePhase::Settled) || self.cleanup {
            None
        } else if self.stopping() {
            Some(self.deadline)
        } else {
            Some(self.poll.min(self.deadline))
        }
    }
    fn overdue(&self, _now: skein_lib::Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        matches!(self.phase, LivePhase::Settled)
    }
}

/// The live face has only the supervised box and ctl, no issuer, visitor or peer.
pub fn run_live(
    plan: LivePlan<'_>,
    budget: Duration,
    face: Face,
) -> Result<super::boxed::CtlOutcome<LiveEvidence>, String> {
    use skein_shell::{Clock, Host};
    use skein_world::end_to_end::{Binary, Command, Expectation, Mode};
    if let Face::Fake { peer, .. } = &face {
        peer.configure(&plan.paths[0]);
    }
    let clock = Clock::new();
    let mut binary = Binary::start_with_tree(
        Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("box".into())
                .chain(deployment_flags(plan.paths).into_iter().map(Into::into))
                .chain(["--echo".into(), "steps".into()])
                .collect(),
            directory: plan.paths[0].parent().expect("scratch root").into(),
            environment: vec![("PATH".into(), "/usr/bin:/bin".into())],
        },
        Mode::Pipes,
        262_144,
        Expectation::Supervise,
        false,
    )
    .expect("live box starts");
    let streams = binary.take_streams().expect("box pipes");
    let mut fds = streams.descriptors();
    let root = skein_shell::open_root(plan.client).expect("fresh client root");
    fds.push(root);
    let (commands, account_at) = live_commands(plan.first && !plan.kept, plan.model);
    let failure = std::cell::Cell::new(None);
    let evidence = std::cell::Cell::new(None);
    let mut world = real::World::new(LiveJudge {
        tasks: plan.tasks.clone().unwrap_or_default(),
        baseline: plan.baseline.cloned().unwrap_or_default(),
        roots: plan.roots.cloned().unwrap_or_default(),
        address: plan.address,
        token: None,
        visitor: matches!(face, Face::Fake { .. }).then_some(3),
        account_at,
        found_at: 0,
        checked: 0,
        pending: commands.into(),
        arguments: Vec::new(),
        active_until: None,
        wait_started: None,
        wait_until: None,
        wait_call: None,
        failure: &failure,
        signing_until: None,
        account_checked: false,
        linked: false,
        cleanup: false,
        phase: plan.starting_phase(),
        plan,
        snapshot: None,
        replies: std::collections::BTreeMap::new(),
        tasks_at: 0,
        tools: std::collections::BTreeSet::new(),
        ended_at: None,
        pages: 0,
        evidence: &evidence,
        deadline: clock
            .now()
            .now
            .saturating_add(Duration::from_nanos(budget.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()))),
        poll: clock.now().now,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        let mut calls = super::ctl::Calls::new(root, vec![], Some(&streams), None).with_box_stderr();
        calls.hold();
        Process::Calls(Box::new(calls))
    });
    if matches!(face, Face::Fake { .. }) {
        face.spawn(&mut world);
    }
    // Failure diagnostics: at most 36 ctl reads (60s each), a 20s native
    // change-page read, and ten seconds to cancel, drain and reap afterwards.
    let drain_budget = Duration::from_secs(36 * 60 + 30);
    let ended = world.run(&clock, Duration::from_nanos(budget.as_nanos().saturating_add(drain_budget.as_nanos())));
    if !ended.procs.iter().all(Host::is_empty) {
        return Err("live box, descendants and ctl did not drain".into());
    }
    let [Process::Binary(binary), Process::Calls(calls), ..] = ended.procs.as_slice() else {
        panic!("live box and ctl")
    };
    if let Some(message) = failure.take() {
        super::live::notice(&format!("BOX stderr: {}", String::from_utf8_lossy(binary.stderr())));
        return Err(message);
    }
    Ok(super::boxed::CtlOutcome {
        model: None,
        exit: binary.exit_status(),
        stdout: calls.stdout().to_vec(),
        stderr: binary.stderr().to_vec(),
        left: binary.tree_status().live_at_leader_exit.clone(),
        calls: calls
            .ends()
            .iter()
            .map(|call| super::Ended { exit: call.exit, stdout: call.stdout.clone(), stderr: call.stderr.clone() })
            .collect(),
        checked: evidence.take(),
        visits: Vec::new(),
    })
}

fn live_change_page(engine: std::net::SocketAddr, client: &std::path::Path, task: u64) -> String {
    use skein_shell::{Clock, Host};
    use skein_world::{Referee, real};
    use temper_client_shell_terminal::client::{NativeClient, Settings, Start};
    struct Judge<'a> {
        task: u64,
        opened: bool,
        closing: bool,
        page: &'a std::cell::Cell<String>,
        until: skein_lib::Time,
    }
    impl Referee<NativeClient> for Judge<'_> {
        fn act(&mut self, now: skein_lib::Time, clients: &mut [NativeClient]) {
            let client = &mut clients[0];
            if client.started() && !self.opened {
                client.go(temper_client_domain::Address::Task {
                    task: self.task,
                    filter: temper_client_domain::PlanFilter::All,
                });
                self.opened = true;
            }
            if self.closing {
                return;
            }
            let mut page = String::new();
            for node in client.tree().nodes().iter().filter(|node| !node.text.is_empty()) {
                for run in &node.text {
                    page.push_str(&String::from_utf8_lossy(&run.text));
                }
                page.push('\n');
            }
            if (page.contains("work/hello") && page.contains("CI at its head")) || now >= self.until {
                self.page.set(page);
                client.close();
                self.closing = true;
            }
        }
        fn observe(&mut self, _now: skein_lib::Time, _clients: &[NativeClient]) {}
        fn next_deadline(&self) -> Option<skein_lib::Time> {
            (!self.closing).then_some(self.until)
        }
        fn overdue(&self, _now: skein_lib::Time) -> Option<String> {
            None
        }
        fn passed(&self) -> bool {
            self.closing
        }
    }
    let root = skein_shell::open_root(client).expect("client's parent root");
    let clock = Clock::new();
    let page = std::cell::Cell::new(String::new());
    let mut world = real::World::new(Judge {
        task,
        opened: false,
        closing: false,
        page: &page,
        until: clock.now().now.saturating_add(skein_lib::Duration::from_secs(10)),
    });
    world.spawn_with_fds(vec![root], || {
        NativeClient::new(
            Settings {
                engine: skein_io::Addr::Ip(engine),
                directory: b"client".as_slice().into(),
                limits: temper_client_shell_terminal::limits::native(),
            },
            root,
            skein_shell::effective_user(),
            91,
            Start::Token,
        )
        .expect("native reader")
    });
    let ended = world.run(&clock, skein_lib::Duration::from_secs(20));
    assert!(ended.procs.iter().all(Host::is_empty), "native reader drained");
    page.take()
}

fn client_address(calls: &super::ctl::Calls) -> Result<std::net::SocketAddr, String> {
    let link = std::str::from_utf8(calls.link()).map_err(|_| "client link encoding")?;
    link.strip_prefix("http://")
        .ok_or("local client link")?
        .split('/')
        .next()
        .ok_or("client listener")?
        .parse()
        .map_err(|_| "client listener address".into())
}

/// Shipped branches are temper/ROOT/NAME (`forge_route::tree_branch`).
/// Forgejo keeps the branch in head.label after merge (head.ref becomes
/// refs/pull/N/head). The root's ctl document supplies its project.
fn kept_project(client: &std::path::Path, me: &serde_json::Value, pulls: &[serde_json::Value]) -> Result<u64, String> {
    let projects = me["projects"].as_array().ok_or("person's projects missing")?;
    for pull in pulls.iter().rev() {
        let Some(root) = pull["head"]["label"]
            .as_str()
            .and_then(|branch| branch.strip_prefix("temper/"))
            .and_then(|branch| branch.split_once('/'))
            .and_then(|(root, _)| root.parse::<u64>().ok())
        else {
            continue;
        };
        let read = super::ctl::alone(client, super::ctl::Call::new(&["task", "show", &root.to_string(), "--json"]));
        require(read.exit == Some(Exit::Code(0)), "kept change read failed")?;
        let task = live_json(&read)?;
        let project = task["project"].as_u64().ok_or("kept change project missing")?;
        require(projects.iter().any(|row| row["project"] == project), "kept change project is visible to the person")?;
        return Ok(project);
    }
    Err("kept deployment lacks a merged work/hello change".into())
}
