//! Native forms, LLM scripts and independent forge observations for repository stories.
use crate::{fixtures, world::Story};
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use skein_fake_person::{Find, Step};
use skein_lib::Duration;
use skein_ui::Role;
use std::collections::BTreeMap;
use temper_engine_domain as root;
use temper_fake_checkout::git::Remote;

// One hook per push: the setup's topic push and the agent's change.
pub(crate) const ADOPT_HOOKS: usize = 2;

thread_local! { static STORY: std::cell::Cell<Story> = const { std::cell::Cell::new(Story::Chat { words: b"Hello" }) }; }
pub(crate) fn set(story: Story) {
    STORY.with(|slot| slot.set(story));
}

pub(crate) fn client_limits() -> temper_client_shell_terminal::limits::Limits {
    let mut limits = fixtures::client_limits();
    if STORY.with(|slot| matches!(slot.get(), Story::Adopt)) {
        // Batch this whole-exchange story without changing the other stories' schedule.
        limits.io.receive = 16_384;
        limits.io.intake = limits.io.receive;
        limits.io.output = limits.io.receive;
        limits.connections.http.read = limits.io.receive;
        limits.connections.http.send = limits.io.receive;
    }
    limits
}

pub(crate) fn person() -> Vec<Step<temper_client_domain::Address>> {
    let mut steps = vec![
        Step::Go {
            address: temper_client_domain::Address::Inbox { filter: temper_client_domain::address::Filter::ALL },
        },
        see(Find::named(Role::Link, b"Adopt a repository")),
        press(Role::Link, b"Adopt a repository"),
        see(Find::named(Role::TextField, b"Owner")),
        ty(Role::TextField, b"Owner", b"acme"),
        ty(Role::TextField, b"Name", b"app"),
        ty(Role::TextField, b"Landing branch", b"main"),
        press(Role::Check, b"Make it the project's home"),
        press(Role::Button, b"Adopt"),
        see(Find::text(b"Adopted acme/app on forge 1 as owned.")),
        Step::Go {
            address: temper_client_domain::Address::Chats {
                filter: temper_client_domain::ChatsFilter::All,
                start: false,
            },
        },
        see(Find::named(Role::TextArea, b"Start a chat")),
        ty(Role::TextArea, b"Start a chat", b"@plan Fix the adopted repository"),
        press(Role::Button, b"Start"),
    ];
    steps.push(see(Find::text(b"@plan Fix the adopted repository")));
    steps
}
pub(crate) fn adoption_person() -> Vec<Step<temper_client_domain::Address>> {
    let mut steps = vec![Step::Go { address: temper_client_domain::Address::Adopt { project: fixtures::PROJECT } }];
    steps.extend(person().into_iter().skip(fixtures::ADOPTION_FORM_START).take(fixtures::ADOPTION_FORM_STEPS));
    steps
}
fn see(find: Find) -> Step<temper_client_domain::Address> {
    Step::See { find, within: fixtures::PERSON_WAIT }
}
fn press(role: Role, name: &[u8]) -> Step<temper_client_domain::Address> {
    Step::Press { find: Find::named(role, name) }
}
fn ty(role: Role, name: &[u8], words: &[u8]) -> Step<temper_client_domain::Address> {
    Step::Type { find: Find::named(role, name), words: words.into() }
}
fn call(name: &[u8], arguments: &[u8]) -> Turn {
    Turn {
        lines: Box::new([Line::Call { name: name.into(), arguments: arguments.into() }]),
        finish: Finish::ToolCalls,
        tokens: fixtures::CALL_TOKENS,
    }
}
pub(crate) fn provider(story: Story) -> skein_fake_peers::llm::Peer {
    if matches!(story, Story::Adoption) {
        return fixtures::provider();
    }
    if matches!(story, Story::ChatReadThenWait { .. }) {
        return fixtures::read_then_wait_provider();
    }
    if matches!(story, Story::Chat { .. }) {
        return fixtures::provider();
    }
    let grant = |kind| serde_json::json!({"connector":fixtures::FORGE_CONNECTOR,"kind":kind,"segments":["forge","127.0.0.1:3000"],"terminal":"open","last":"acme"});
    let mut delegate = serde_json::json!({"batch":[{
        "executor":{"kind":"procedure","connector":fixtures::FORGE_CONNECTOR,"code":fixtures::PROCEDURE_CODE},
        "spec":{"words":"@edit Fix","parameters":[{"name":fixtures::PARAMETER_NAMES[0],"kind":"resource","connector":fixtures::FORGE_CONNECTOR,"resource":fixtures::REPOSITORY_TAG},{"name":fixtures::PARAMETER_NAMES[1],"kind":"bytes","value":"main"}]},
        "contract":{"kind":"change","connector":fixtures::FORGE_CONNECTOR,"change_kind":fixtures::CHANGE_KIND,"words":fixtures::CHANGE_WORDS},
        "authority":{"tools":fixtures::PRODUCER_TOOLS,"grants":fixtures::GRANT_KINDS.map(grant),"delegation":{"kinds":[{"kind":"agent","number":fixtures::AGENT_KIND}],"tasks":fixtures::PRODUCER_TASKS,"depth":fixtures::PRODUCER_DEPTH},"budget":{"spend":fixtures::PRODUCER_SPEND},"notes":fixtures::PRODUCER_NOTES}
    }]});
    if matches!(story, Story::Merge) {
        delegate["batch"][0]["spec"]["parameters"]
            .as_array_mut()
            .expect("parameters")
            .push(serde_json::json!({"name":fixtures::PARAMETER_NAMES[2],"kind":"bytes","value":std::str::from_utf8(fixtures::MERGE_BRANCH).expect("fixture branch")}));
    }
    let delegate = serde_json::to_vec(&delegate).expect("delegate JSON");
    let finish = call(
        b"finish",
        br#"{"form":"change","text":"","fields":{"title":"Fix the file","body":"The file is fixed."}}"#,
    );
    let edits = if matches!(story, Story::Merge) {
        vec![finish.clone()]
    } else {
        vec![call(b"shell", br#"{"command":"echo fixed > conflict.txt"}"#), finish.clone()]
    };
    fixtures::scripted_provider(Box::new([
        Script {
            cue: b"@plan".as_slice().into(),
            turns: Box::new([call(b"delegate", &delegate), call(b"wait", b"{}"), call(b"wait", b"{}")]),
        },
        Script { cue: b"@edit".as_slice().into(), turns: edits.into_boxed_slice() },
        Script {
            cue: b"Resolve the merge conflict".as_slice().into(),
            turns: Box::new([call(b"shell", br#"{"command":"echo fixed > conflict.txt"}"#), finish]),
        },
    ]))
}
pub(crate) fn forge(story: Story) -> temper_fake_git_hosted::Typed {
    let config = skein_fake_forge_domain::Config {
        limits: skein_fake_forge_domain::Limits {
            statuses: fixtures::FORGE_STATUSES,
            ..temper_fake_git_hosted::fixtures::CONFIG.limits
        },
        ..temper_fake_git_hosted::fixtures::CONFIG
    };
    let mut domain = skein_fake_forge_domain::Domain::new(&config, fixtures::SEED);
    let base = skein_fake_forge_domain::repository(
        &mut domain,
        &config,
        skein_fake_forge_domain::api::Setup {
            id: fixtures::REPOSITORY,
            name: b"acme/app".as_slice().into(),
            default: b"main".as_slice().into(),
            tree: Box::new([skein_fake_forge_domain::api::File {
                path: b"conflict.txt".as_slice().into(),
                content: b"base\n".as_slice().into(),
            }]),
            labels: Box::new([]),
            checks: skein_fake_forge_domain::api::Checks {
                contexts: Box::new([b"check".as_slice().into()]),
                latency_min: Duration::ZERO,
                latency_max: Duration::ZERO,
                silent: fixtures::CI_SILENT,
                passes: fixtures::CI_PASSES,
                reruns: fixtures::CI_RERUNS,
                cue: None,
            },
            protection: None,
            hooked: matches!(story, Story::Adopt),
        },
    );
    skein_fake_forge_domain::grant(
        &mut domain,
        b"acme/app",
        fixtures::WORKER_USER,
        skein_fake_forge_domain::api::Permission::Write,
    );
    let forge = std::rc::Rc::new(std::cell::RefCell::new(skein_fake_peers::forge::Forge::new(domain, config)));
    let mut transport =
        temper_fake_git_hosted::Typed::new(forge, &temper_fake_git_hosted::fixtures::table(b"acme/app", None));
    let identity = temper_fake_git_hosted::fixtures::identity();
    transport.authenticate(Some(&(identity.login, identity.token)));
    assert_eq!(
        transport.commit_tree(base, None, BTreeMap::from([(b"conflict.txt".to_vec(), b"topic\n".to_vec())]), b"topic"),
        fixtures::BRANCH_HEAD
    );

    transport
        .push(b"http://127.0.0.1:3000/acme/app", b"topic", fixtures::BRANCH_HEAD, None)
        .expect("the fixture's existing topic is already on the forge");
    if matches!(story, Story::Merge) {
        let mut forge = transport.forge.borrow_mut();
        let config = forge.config;
        let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::from_nanos(0), limits: config };
        let moved = skein_fake_forge_domain::advance(
            &mut forge.domain,
            &env,
            b"acme/app",
            b"main",
            b"conflict.txt",
            b"moved\n",
            fixtures::MOVING_USER,
        )
        .expect("another party moves the base");
        assert_eq!(moved, fixtures::MOVED_BASE, "counted moved base");
    }
    while transport.forge.borrow_mut().domain.pop_observation().is_some() {}
    transport
}
pub(crate) fn prepare_merge_branch(transport: &temper_fake_git_hosted::Typed) {
    let mut forge = transport.forge.borrow_mut();
    let env = skein_lib::Env { now: forge.now, wall: skein_lib::Wall::EPOCH, limits: forge.config };
    assert!(forge.pending.room() >= skein_fake_forge_domain::MAX_OUT, "reserve external domain output");
    let token = forge.next_reply();
    let skein_fake_peers::forge::Forge { domain, pending, .. } = &mut *forge;
    skein_fake_forge_domain::step(
        domain,
        &env,
        skein_fake_forge_domain::Event::Call {
            reply_to: skein_lib::ReplyTo::new(token),
            user: fixtures::WORKER_USER,
            repository: b"acme/app".as_slice().into(),
            op: skein_fake_forge_domain::api::Op::Git(skein_fake_forge_domain::api::Git::Create {
                branch: fixtures::MERGE_BRANCH.into(),
                commit: fixtures::BRANCH_HEAD,
            }),
        },
        pending,
    );
    let mut created = false;
    for _ in 0..32 {
        for _ in 0..forge.pending.len() {
            let reply = forge.pending.pop().expect("pending pass length");
            match reply {
                skein_fake_forge_domain::Request::Reply { to, result } => {
                    let destination = to.into_token();
                    if destination != token {
                        forge.pending.push(skein_fake_forge_domain::Request::Reply {
                            to: skein_lib::ReplyTo::new(destination),
                            result,
                        });
                        continue;
                    }
                    assert_eq!(
                        result,
                        Ok(skein_fake_forge_domain::api::Answer::Branch(
                            skein_fake_forge_domain::api::Created::Created
                        )),
                        "the existing private merge branch"
                    );
                    created = true;
                }
                reply @ skein_fake_forge_domain::Request::Hook { .. } => forge.pending.push(reply),
            }
        }
        if created {
            break;
        }
        assert!(forge.pending.room() >= skein_fake_forge_domain::MAX_OUT, "reserve external domain output");
        let skein_fake_peers::forge::Forge { domain, pending, .. } = &mut *forge;
        skein_fake_forge_domain::fire(domain, &env, pending);
    }
    forge.domain.reclaim();
    assert!(created, "the existing private merge branch answered");
}
pub(crate) fn adopted(rows: &BTreeMap<root::Key, root::Record>) -> u32 {
    u32::try_from(rows.values().filter(|row| matches!(row,
        root::Record::Forge { row } if matches!(row.as_ref(), temper_engine_domain_forge::Stored::Repository(_))
    )).count()).expect("adopted repositories")
}
pub(crate) fn configure(value: &mut serde_json::Value) {
    if STORY.with(|slot| matches!(slot.get(), Story::ChatReadThenWait { .. })) {
        return;
    }
    if STORY.with(|slot| matches!(slot.get(), Story::Chat { .. })) {
        return;
    }
    value["workers"][0]["slots"] = fixtures::AGENTS.into();
    value["workers"][0]["limits"]["overrides"]["hub.slots"] = fixtures::AGENTS.into();
    let grants: Vec<_> = fixtures::GRANT_KINDS
        .into_iter()
        .map(|kind| {
            serde_json::json!({
                "connector":fixtures::FORGE_CONNECTOR,"kind":kind,"pattern":{"segments":["forge","127.0.0.1:3000"],"last":"open","value":"acme"}
            })
        })
        .collect();
    let kinds = serde_json::json!([{"kind":"charter","number":fixtures::CHARTER_NUMBER},{"kind":"procedure","number":fixtures::PROCEDURE_CODE}]);
    let mut ceiling = value["engine"]["projects"][0]["policy"]["ceiling"].clone();
    ceiling["tools"] = fixtures::PRODUCER_TOOLS.into();
    ceiling["grants"] = grants.into();
    ceiling["delegation"]["kinds"] = kinds;
    ceiling["delegation"]["depth"] = fixtures::AUTHORITY_DEPTH.into();
    value["engine"]["projects"][0]["policy"]["ceiling"] = ceiling.clone();
    value["engine"]["projects"][0]["policy"]["roles"][0]["authority"] = ceiling;
    for (name, count) in fixtures::ENGINE_OVERRIDES {
        value["engine"]["limits"]["overrides"][name] = count.into();
    }
    let overrides = value["engine"]["limits"]["overrides"].as_object_mut().expect("engine overrides");
    for name in [
        "tasks.funders",
        "tasks.dependencies",
        "tasks.charters",
        "people.amendment_bytes",
        "people.projects",
        "tasks.hold_bytes",
        "notes.entries_per_scope",
        "notes.recalled",
        "views.runs",
        "people.holdings",
        "people.waiters",
        "people.goals",
        "people.pending",
    ] {
        overrides.remove(name);
    }
}

/// The scripted shell's echo redirection acts on the inherited machine files.
pub(crate) struct Shell {
    jobs: std::collections::VecDeque<skein_io::kernel::Op>,
    owned: std::collections::BTreeSet<skein_io::kernel::Fd>,
    content: Box<[u8]>,
    pub(crate) target: Vec<u8>,
    pub(crate) written: Vec<u8>,
    code: u8,
    completions: skein_lib::Queue<skein_io::kernel::Complete>,
    submissions: skein_lib::Queue<skein_io::kernel::Submit>,
    pending: bool,
    finished: bool,
}
impl Shell {
    pub(crate) fn new(spawn: &skein_io::kernel::Spawn, inherited: &skein_world::Inherited) -> Self {
        use skein_io::kernel::{Op, OpenHow};
        assert_eq!(spawn.args[0].as_ref(), b"-c", "shell command invocation");
        let command = std::str::from_utf8(&spawn.args[1]).ok();
        let edit = command.and_then(|command| command.split_once('>')).and_then(|(echo, target)| {
            let text = echo.trim().strip_prefix("echo ")?;
            let target = target.trim().as_bytes();
            (!target.is_empty() && !target.contains(&b'/') && target != b"..").then_some((text, target))
        });
        let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("shell worker root").1;
        let mut jobs = std::collections::VecDeque::new();
        let (content, target, code) = if let Some((text, target)) = edit {
            let path = [spawn.dir.as_ref(), b"/", target].concat();
            jobs.push_back(Op::Open { root, path: path.into(), how: OpenHow::WriteNoFollow });
            ([text.as_bytes(), b"\n"].concat().into(), target.to_vec(), 0)
        } else {
            let error = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("shell stderr").1;
            jobs.push_back(Op::PipeWrite {
                fd: error,
                bytes: b"/bin/sh: command not found\n".as_slice().into(),
                from: 0,
            });
            (Box::new([]) as Box<[u8]>, Vec::new(), 127)
        };
        let mut owned: std::collections::BTreeSet<_> = inherited.roots.iter().map(|(_, fd)| *fd).collect();
        owned.extend(inherited.pipes.iter().map(|(_, fd)| *fd));
        owned.insert(inherited.signal);
        Self {
            jobs,
            owned,
            content,
            target,
            written: Vec::new(),
            code,
            completions: skein_lib::Queue::with_capacity(1),
            submissions: skein_lib::Queue::with_capacity(1),
            pending: false,
            finished: false,
        }
    }
}
impl skein_shell::Host for Shell {
    fn iterate(&mut self, _now: skein_lib::Time, _wall: skein_lib::Wall) {
        use skein_io::kernel::{Done, Op};
        while let Some(complete) = self.completions.pop() {
            self.pending = false;
            match (complete.kind, complete.result.expect("shell file operation")) {
                (Op::Open { .. }, Done::Fd(fd)) => {
                    self.owned.insert(fd);
                    self.jobs.push_back(Op::Truncate { fd, length: 0 });
                }
                (Op::Truncate { fd, .. }, Done::Nothing) => {
                    self.written.clear();
                    self.jobs.push_back(Op::Write { fd, bytes: self.content.clone(), from: 0, at: 0 });
                }
                (Op::Write { fd, bytes, from, .. }, Done::Count(count)) => {
                    self.written.extend_from_slice(&bytes[from as usize..(from + count) as usize]);
                    let from = from + count;
                    if (from as usize) < bytes.len() {
                        self.jobs.push_back(Op::Write { fd, bytes, from, at: u64::from(from) });
                    }
                }
                (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => {
                    if (from + count) as usize != bytes.len() {
                        self.jobs.push_back(Op::PipeWrite { fd, bytes, from: from + count });
                    }
                }
                (Op::Close { fd }, Done::Nothing) => {
                    self.owned.remove(&fd);
                }
                (op, done) => panic!("shell terminal {op:?}: {done:?}"),
            }
        }
        if !self.pending {
            if let Some(kind) = self.jobs.pop_front().or_else(|| self.owned.first().map(|fd| Op::Close { fd: *fd })) {
                self.pending = true;
                self.submissions.push(skein_io::kernel::Submit { op: skein_lib::Token::new(1), kind });
            } else {
                self.finished = true;
            }
        }
    }
    fn completions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: skein_lib::Time) -> bool {
        !self.completions.is_empty() || (!self.finished && !self.pending)
    }
    fn next_deadline(&self) -> Option<skein_lib::Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<skein_lib::Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.finished
    }
    fn exit(&self) -> Option<skein_io::kernel::Exit> {
        self.finished.then_some(skein_io::kernel::Exit::Code(self.code))
    }
    fn worst_case(&self) -> u64 {
        8192
    }
    fn operations(&self) -> u32 {
        1
    }
}

/// Chat-only seeds fit the stock ceiling; repository seeds use wider own defaults.
pub(crate) fn uses_stock() -> bool {
    STORY.with(|slot| matches!(slot.get(), Story::Chat { .. } | Story::ChatReadThenWait { .. }))
}

thread_local! { static SETTINGS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(crate) fn setup_settings(enabled: bool) {
    SETTINGS.with(|slot| slot.set(enabled));
}
pub(crate) fn needs_settings() -> bool {
    SETTINGS.with(std::cell::Cell::get)
}
pub(crate) fn settings_file() -> Vec<u8> {
    let document: serde_json::Value = serde_json::from_slice(&fixtures::document()).expect("deployment JSON");
    let stock = crate::settings::stock(&document);
    // This world's restart smoke changes its 1 s Own wait to 500 ms through ctl.
    let mut charter = stock["charter"].clone();
    charter["waiting"] = (u64::from(fixtures::CHAT_WAIT_MS) * 1_000_000 / 2).into();
    serde_json::to_vec(&serde_json::json!({"items":[crate::settings::item("charter", 1, charter)]}))
        .expect("settings FILE")
}

/// The service's typed defaults, starting from the binary's stock and the document's models.
pub(crate) fn given(document: &serde_json::Value) -> temper_service::stock::Given {
    let engine = crate::settings::configuration(document);
    let mut defaults = temper_service::stock::defaults(&engine.llm);
    let read_wait = STORY.with(|slot| matches!(slot.get(), Story::ChatReadThenWait { .. }));
    if read_wait {
        defaults.charters[0].policy.waiting = skein_lib::Duration::from_millis(u64::from(fixtures::CHAT_WAIT_MS));
    } else {
        defaults.templates[0].flags.ended_by = jig_domain_tasks::EndedBy::Result;
    }
    if !uses_stock() {
        let seed = engine.projects.first().expect("repository seed");
        defaults.rules.ceiling = seed.policy.ceiling.clone();
        defaults.rules.maximum_run_spend = fixtures::PRODUCER_SPEND;
        let pairs = fixtures::IMPLICATIONS.map(|implies| jig_domain_authority::Implication {
            connector: fixtures::FORGE_CONNECTOR,
            kind: fixtures::GRANT_KINDS[0],
            implies,
        });
        defaults.rules.implies = jig_domain_authority::Implies::new(Box::new(pairs), 2).expect("fixture implications");
        defaults
            .policies
            .insert(
                1,
                jig_domain_core::ConfiguredPolicy {
                    policy: seed.policy.clone(),
                    permissions: seed.roles.clone(),
                    templates: seed.templates.clone(),
                },
            )
            .expect("existing policy slot");
        let charter = &mut defaults.charters[0];
        charter.policy.inspect = true;
        charter.policy.modify = true;
        charter.policy.shell = true;
        charter.policy.agents = false;
        charter.policy.turns = fixtures::CHARTER_TURNS;
        charter.spend = fixtures::PRODUCER_SPEND;
    }
    temper_service::stock::Given::Own(Box::new(defaults))
}
