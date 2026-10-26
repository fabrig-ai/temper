//! A page's log under the one-process world's instrumented referee (playground.md, 9).
use crate::{
    machine::Machine,
    referee::{self, Store},
    world::{self, Observations, Proc, Programs, ServiceHost, Story},
};
use skein_lib::{List, Time};
use skein_playground::replay::{Differs, Feed};
use skein_ui_web::{
    bridge::{Duty, Said, Stamp, log::Log},
    fetch::Report,
    transport::StreamDuty,
};
use skein_world::{Host, Inherited, Memory, World};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
use temper_playground::{A, Process, Signalled, StoryStart};

pub(crate) struct Driver {
    feed: Option<Feed>,
    exchanges: BTreeSet<u32>,
    streams: u32,
    finished: bool,
}
impl Driver {
    fn new(log: Log) -> Self {
        Self { feed: Some(Feed::new(log)), exchanges: BTreeSet::new(), streams: 0, finished: false }
    }
    fn enter(&mut self, at: Stamp, visitor: &mut skein_playground::visitor::Visitor) -> Result<(), Differs> {
        let feed = self.feed.as_mut().expect("feed before the visit ends");
        while let Some(next) = feed.next() {
            assert!(next.step >= at.step, "replay never passes an input's step");
            if next.step != at.step || next.instant > at.instant {
                break;
            }
            assert_eq!(next, at, "each input enters at its recorded instant");
            let duty = feed.take(at)?;
            match &duty {
                Duty::Exchange(exchange) => assert!(self.exchanges.insert(exchange.id), "one live exchange id"),
                Duty::Abandon { exchange } => {
                    self.exchanges.remove(exchange);
                }
                Duty::Stream(StreamDuty::Open { .. }) => self.streams += 1,
                Duty::Stream(StreamDuty::Close) | Duty::ReadOn => {}
            }
            visitor.duty(duty);
        }
        Ok(())
    }
    fn report(&mut self, report: Report) {
        match &report {
            Report::Answered { exchange, .. } | Report::Failed { exchange } => {
                assert!(self.exchanges.remove(exchange), "one terminal per exchange");
            }
            Report::StreamEnded | Report::StreamRefused { .. } => {
                self.streams = self.streams.checked_sub(1).expect("one terminal per stream open");
            }
            Report::StreamOpened | Report::StreamPiece { .. } => {}
        }
        self.feed.as_mut().expect("reports before the visit ends").said(&Said::Report(report));
    }
    /// Called between whole steps: a slice's reports and lines fold apart.
    pub(crate) fn act(&mut self, now: Time, service: &mut ServiceHost, others: &mut [Proc]) -> bool {
        if self.finished {
            return true;
        }
        for line in service.printed.drain(..) {
            self.feed.as_mut().expect("feed before the visit ends").said(&Said::Line(line));
        }
        let visitor = others
            .iter_mut()
            .find_map(|proc| match proc {
                Proc::Playground(Process::Visitor(visitor)) => Some(visitor.as_mut()),
                Proc::Service(_) | Proc::Client(_) | Proc::Ctl(_) | Proc::Playground(_) => None,
            })
            .expect("page's visitor");
        while let Some(report) = visitor.report() {
            self.report(report);
        }
        self.enter(Stamp { step: service.turns, instant: now }, visitor).expect("session log replays at each input");
        if self.feed.as_ref().expect("feed before the visit ends").next().is_none()
            && self.exchanges.is_empty()
            && self.streams == 0
        {
            // The fixture is a visit ending at its stream's terminal, while the
            // service is still running. Check its snapshot before teardown:
            // that teardown's stop was not part of the recorded visit.
            self.feed.take().expect("finish the log once").finish().expect("session log replays to its end");
            self.finished = true;
        }
        self.finished
    }
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        self.feed.as_ref().and_then(Feed::next).map(|stamp| stamp.instant)
    }
    pub(crate) fn finished(&self) -> bool {
        self.finished
    }
}

struct Context {
    start: StoryStart,
    store: Rc<RefCell<Store>>,
    shared: Rc<RefCell<skein_fake_peers::forge::Forge>>,
    table: skein_fake_forge_protocol::Config,
    issued: Vec<(&'static str, Box<[u8]>)>,
}
thread_local! {
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
}
/// The page's issued fixture tokens, for the same secret checks as terminal worlds.
pub(crate) fn issued() -> Vec<(&'static str, Box<[u8]>)> {
    CONTEXT.with(|context| context.borrow().as_ref().map_or_else(Vec::new, |context| context.issued.clone()))
}
fn service(inherited: &Inherited) -> Proc {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("replay context");
        let start = &context.start.service;
        let directories = jig_protocol_config::Directories::working();
        let limits = temper_service::store_limits(&start.document, &directories, Some(&start.agent))
            .expect("story's store opening bounds");
        let named = temper_service::named(&start.document, &directories).expect("story's model names");
        world::instrumented(
            start,
            inherited,
            context.store.clone(),
            Observations {
                codec: jig_protocol_store::codec::limits(&limits.store.core).expect("story codec bounds"),
                families: temper_protocol_store::bounds(&limits.forge).expect("story's application codec bounds"),
                names: jig_protocol_store::codec::names(&named.llm.models),
                agent: start.agent.limits.bodies,
            },
            true,
        )
    })
}
fn agent(_spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("replay context");
        Proc::Playground(Process::Agent(Box::new(temper_playground::agent(
            inherited,
            smith_agent_shell::config::parse(A.agent).expect("story agent"),
            context.start.agent,
            101,
        ))))
    })
}
fn agent_roots(spawn: &skein_io::kernel::Spawn) -> Vec<skein_world::StartupRoot> {
    CONTEXT.with(|context| {
        temper_playground::agent_roots(
            spawn,
            &context.borrow().as_ref().expect("replay context").start.service.worker_path,
        )
    })
}
fn git(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("replay context");
        let transport = temper_fake_git_hosted::Typed::new(context.shared.clone(), &context.table);
        Proc::Playground(Process::Git(Box::new(temper_playground::git(spawn, inherited, transport))))
    })
}
// Exactly as the bare composition: reconciliation issues the bot, and later
// git spawns take its current identity table, rather than a pre-seeded bot.
fn identities(peer: &mut skein_fake_peers::forge::Peer) {
    CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        let context = context.as_mut().expect("replay context");
        let store = peer.setup_mut();
        let document: serde_json::Value = serde_json::from_slice(A.deployment).expect("story deployment");
        let login = document["application"]["setup"][0]["bot"]["login"].as_str().expect("story bot").as_bytes();
        if let Some(bot) = store.find(login) {
            let mut users = List::with_capacity(1);
            users.push((bot.id, bot.login.clone())).expect("one bot");
            context.table.users = users;
            let mut tokens = List::with_capacity(u32::try_from(store.issued().len()).expect("bounded tokens"));
            for token in store.issued().iter().filter(|token| token.user == bot.id) {
                tokens.push((token.secret.clone(), token.user)).expect("bounded tokens");
                if !context.issued.iter().any(|(_, bytes)| bytes.as_ref() == token.secret.expose()) {
                    context.issued.push(("git identity token", token.secret.expose().into()));
                }
            }
            context.table.tokens = tokens;
        }
    });
}

/// Evidence from the instrumented world's completed replay and teardown.
#[derive(Debug)]
pub struct Outcome {
    pub steps: u32,
    pub end: Time,
    pub commits: u64,
    pub drained: bool,
}
/// Replay story A's kept visit with the one-process referee's ordinary checks.
#[must_use]
pub fn run(log: Log) -> Outcome {
    assert_eq!(log.start.story.as_ref(), A.name.as_bytes(), "the fixture names story A");
    let start = temper_playground::story_start(&A, log.start.seed);
    let machine = Machine(start.machine());
    let config = skein_sim::Config { wall: log.start.start, trace: skein_sim::Trace::Off, ..skein_sim::Config::calm() };
    let forge_document = temper_fake::forge::Document::decode(A.forge).expect("story forge");
    let token = temper_fake::credential::credential(start.forge, temper_fake::credential::ADMIN_TOKEN);
    let (peer, shared) = temper_fake::forge::peer(&forge_document, &token, start.forge).expect("story forge peer");
    let (_, table) = forge_document.seed(start.forge);
    let llm_document = temper_fake::llm::Document::decode(A.llm).expect("story LLM");
    let key = temper_fake::credential::credential(start.llm, temper_fake::credential::KEY);
    let model = temper_fake::llm::peer(temper_fake::llm::configuration(&llm_document, &key, start.llm))
        .expect("story LLM peer");
    let scenario = Story::Chat { words: b"" }; // Terminal story expectations are replaced by the page driver.
    let transport = temper_fake_git_hosted::Typed::new(shared.clone(), &table);
    let forge = Rc::new(RefCell::new(referee::forge::Forge::new(transport, scenario).at(forge_document.listen())));
    let store = Rc::new(RefCell::new(Store::new()));
    let run = Rc::new(RefCell::new(referee::Run::default()));
    let simulator = start.simulator;
    CONTEXT.with(|context| {
        *context.borrow_mut() = Some(Context {
            start,
            store: store.clone(),
            shared,
            table,
            issued: vec![("LLM access token", key.into()), ("forge API token", token.into())],
        });
    });
    let mut world = World::new_controlled(simulator, config, Memory::Unchecked, |controls| {
        referee::Referee::new(controls, store.clone(), scenario, forge, run.clone(), world::Phase::Story)
            .with_page(Driver::new(log))
    })
    .with_machine(machine);
    let agents = smith_agent_shell::config::parse(A.agent).expect("story agent");
    world::host_programs(
        &mut world,
        &Programs {
            agent,
            git,
            agents: 8,
            children: 8,
            operations: agents
                .service
                .limits
                .routes
                .checked_add(3)
                .and_then(|n| n.checked_add(agents.service.limits.domain.run.directories))
                .and_then(|n| n.checked_add(1))
                .expect("hosted agent operations"),
            roots: agent_roots,
        },
    );
    let deployment: serde_json::Value = serde_json::from_slice(A.deployment).expect("story deployment");
    let git_path =
        deployment["application"]["setup"][0]["mode"]["fake"]["git"].as_str().expect("story fake git").as_bytes();
    if git_path != temper_playground::GIT {
        world.host_roots(
            skein_world::HostedProgram { program: git_path.into(), make: git, instances: 8, operations: 1 },
            temper_playground::worker_roots,
        );
    }
    world.spawn_restartable(
        vec![
            skein_world::StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() },
            skein_world::StartupRoot { name: b"worker".as_slice().into(), path: b"worker".as_slice().into() },
        ],
        service,
    );
    world.spawn(|| Proc::Playground(Process::Llm(Box::new(Signalled::new(model, None)))));
    world.spawn(|| Proc::Playground(Process::Forge(Box::new(Signalled::new(peer, None).after_iteration(identities)))));
    world.spawn(|| Proc::Playground(Process::Visitor(Box::new(temper_playground::visitor(&A)))));
    let outcome = world.run();
    assert!(outcome.procs.iter().all(Host::is_empty), "every process drains");
    let result = Outcome {
        steps: outcome.iterations,
        end: outcome.end,
        commits: store.borrow().applied,
        drained: run.borrow().drained,
    };
    CONTEXT.with(|context| *context.borrow_mut() = None);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_ui_web::bridge::{Line, Output, log};

    #[test]
    fn a_log_whose_digest_differs_fails_at_that_input() {
        let mut log = log::read(include_bytes!("../../playground/fixtures/welcome.log")).expect("welcome fixture");
        let build = log.build.clone();
        let stamp = log.inputs[0].stamp;
        let logged = {
            log.inputs[0].digest[0] ^= 1;
            log.inputs[0].digest
        };
        let mut world = temper_playground::world(&A, log.start.seed, log.start.start);
        let mut driver = Driver::new(log);
        while world.steps() < stamp.step {
            if !world.busy() {
                world.advance(world.next_due().expect("startup wakes"));
            }
            world.step();
            for printed in world.printed() {
                driver.feed.as_mut().expect("feed before first input").said(&Said::Line(Line {
                    process: printed.process,
                    stream: match printed.stream {
                        skein_sim_world::Stream::Output => Output::Standard,
                        skein_sim_world::Stream::Error => Output::Error,
                    },
                    line: printed.line,
                }));
            }
        }
        assert_eq!(world.now(), stamp.instant);
        let Process::Visitor(visitor) = world.process(3) else { panic!("visitor fourth") };
        let differs = driver.enter(stamp, visitor).expect_err("the edited digest differs");
        assert_eq!(differs.input, 0);
        assert_eq!(differs.build, build);
        assert_eq!(differs.logged, logged);
        let mut original = logged;
        original[0] ^= 1;
        assert_eq!(differs.replayed, original, "only the digest edit made the input differ");
    }
}
