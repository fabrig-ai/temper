//! The bare story world and its plain factories (playground.md, 4).
use crate::{Machine, Process, Service, Signalled, Start, Story};
use skein_lib::{List, Rng, Secret, Wall};
use skein_sim_world::{HostedProgram, Inherited, Printer, StartupRoot, Stream, World};
use std::{cell::RefCell, rc::Rc};

const USER: u32 = 1000;

struct Seeds {
    simulator: u64,
    service: temper_service::settings::Seeds,
    agent: u64,
    llm: u64,
    forge: u64,
}
/// Draw order is part of replay: simulator, service's four inputs, agent, LLM, forge.
fn seeds(seed: u64) -> Seeds {
    let mut rng = Rng::new(seed);
    let simulator = rng.next_u64();
    let deployment = draw_id(&mut rng);
    let service_seed = rng.next_u64();
    let crypto = Secret::new([draw_id(&mut rng), draw_id(&mut rng)].concat().into_boxed_slice());
    let installation = draw_id(&mut rng);
    Seeds {
        simulator,
        service: temper_service::settings::Seeds { deployment, seed: service_seed, crypto, installation },
        agent: rng.next_u64(),
        llm: rng.next_u64(),
        forge: rng.next_u64(),
    }
}
fn draw_id(rng: &mut Rng) -> [u8; 16] {
    let mut id = [0; 16];
    for chunk in id.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    id
}
struct Context {
    story: &'static Story,
    worker_path: Box<[u8]>,
    seeds: Seeds,
    printer: Printer,
    shared: Rc<RefCell<skein_fake_peers::forge::Forge>>,
    table: skein_fake_forge_protocol::Config,
}
thread_local! {
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
}

/// The playground's world for `story` (playground.md, 4): the composition's
/// processes started from its documents, calm at every layer, seeds derived
/// from `seed`, wall starting at `start`, trace off. Its caller steps it bare.
#[must_use]
pub fn world(story: &'static Story, seed: u64, start: Wall) -> World<Process, Machine> {
    let seeds = seeds(seed);
    let forge_document = temper_fake::forge::Document::decode(story.forge).expect("story forge");
    let token = temper_fake::credential::credential(seeds.forge, temper_fake::credential::ADMIN_TOKEN);
    let (peer, shared) = temper_fake::forge::peer(&forge_document, &token, seeds.forge).expect("story forge peer");
    let (_, table) = forge_document.seed(seeds.forge);
    let config = skein_sim::Config { wall: start, trace: skein_sim::Trace::Off, ..skein_sim::Config::calm() };
    let mut world = World::new(seeds.simulator, config).with_machine(Machine::new(Machine::fresh(USER)));
    let printer = world.printer(b"temper", Stream::Output);
    CONTEXT.with(|context| {
        *context.borrow_mut() =
            Some(Context { story, worker_path: b"/worker".as_slice().into(), seeds, printer, shared, table });
    });
    let agents = smith_agent_shell::config::parse(story.agent).expect("story agent");
    world.host_roots(
        HostedProgram {
            program: crate::AGENT.into(),
            make: agent,
            instances: 8,
            operations: agents
                .service
                .limits
                .routes
                .checked_add(3)
                // smith reserves one hosted open per permitted requested root.
                .and_then(|count| count.checked_add(agents.service.limits.domain.run.directories))
                // Close the additional inherited launch directory.
                .and_then(|count| count.checked_add(1))
                .expect("hosted agent operations"),
        },
        agent_roots,
    );
    world.host_roots(
        HostedProgram { program: crate::GIT.into(), make: git, instances: 8, operations: 1 },
        crate::worker_roots,
    );
    world.host(HostedProgram { program: crate::SH.into(), make: shell, instances: 8, operations: 1 });
    let deployment: serde_json::Value = serde_json::from_slice(story.deployment).expect("story deployment");
    let git_path =
        deployment["application"]["setup"][0]["mode"]["fake"]["git"].as_str().expect("story fake git").as_bytes();
    if git_path != crate::GIT {
        world.host_roots(
            HostedProgram { program: git_path.into(), make: git, instances: 8, operations: 1 },
            crate::worker_roots,
        );
    }
    world.spawn_restartable(
        vec![
            StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() },
            StartupRoot { name: b"worker".as_slice().into(), path: b"worker".as_slice().into() },
        ],
        service,
    );
    world.spawn(|| Process::Llm(Box::new(Signalled::new(model(), None))));
    world.spawn(|| Process::Forge(Box::new(Signalled::new(peer, None).after_iteration(identities))));
    world
}
fn service(inherited: &Inherited) -> Process {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("one world per thread");
        let start = start(context.story, &context.seeds, &context.worker_path);
        Process::Service(Box::new(
            Service::start(&start, inherited).with_printer(context.printer.clone()).with_files(&start.files),
        ))
    })
}
fn agent_roots(spawn: &skein_io::kernel::Spawn) -> Vec<StartupRoot> {
    CONTEXT.with(|context| {
        let context = context.borrow();
        crate::agent_roots(spawn, &context.as_ref().expect("world context").worker_path)
    })
}
fn model() -> skein_fake_peers::llm::Peer {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("world context");
        let document = temper_fake::llm::Document::decode(context.story.llm).expect("story LLM");
        let key = temper_fake::credential::credential(context.seeds.llm, temper_fake::credential::KEY);
        temper_fake::llm::peer(temper_fake::llm::configuration(&document, &key, context.seeds.llm))
            .expect("story LLM peer")
    })
}
// Reconcile issues the bot's credentials through the peer. Git sees that same
// identity table at its next spawn, rather than a pre-seeded bot or grant.
fn identities(peer: &mut skein_fake_peers::forge::Peer) {
    CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        let context = context.as_mut().expect("world context");
        let store = peer.setup_mut();
        let document: serde_json::Value = serde_json::from_slice(context.story.deployment).expect("story deployment");
        let login = document["application"]["setup"][0]["bot"]["login"].as_str().expect("story bot").as_bytes();
        if let Some(bot) = store.find(login) {
            let mut users = List::with_capacity(1);
            users.push((bot.id, bot.login.clone())).expect("one bot");
            context.table.users = users;
            let mut tokens = List::with_capacity(u32::try_from(store.issued().len()).expect("bounded tokens"));
            for token in store.issued().iter().filter(|token| token.user == bot.id) {
                tokens.push((token.secret.clone(), token.user)).expect("bounded tokens");
            }
            context.table.tokens = tokens;
        }
    });
}
fn git(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Process {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("world context");
        let transport = temper_fake_git_hosted::Typed::new(context.shared.clone(), &context.table);
        Process::Git(Box::new(crate::git(spawn, inherited, transport)))
    })
}
fn agent(_spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Process {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let context = context.as_ref().expect("world context");
        Process::Agent(Box::new(crate::agent(
            inherited,
            smith_agent_shell::config::parse(context.story.agent).expect("story agent"),
            context.seeds.agent,
            101,
        )))
    })
}
fn shell(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Process {
    Process::Shell(Box::new(crate::Shell::new(spawn, inherited)))
}

fn start(story: &Story, seeds: &Seeds, worker_path: &[u8]) -> Start {
    let llm = temper_fake::llm::Document::decode(story.llm).expect("story LLM");
    let forge = temper_fake::forge::Document::decode(story.forge).expect("story forge");
    let key = temper_fake::credential::credential(seeds.llm, temper_fake::credential::KEY);
    let token = temper_fake::credential::credential(seeds.forge, temper_fake::credential::ADMIN_TOKEN);
    let worker_secret = hex(&skein_lib::sha256::digest(seeds.service.crypto.expose()));
    let worker_digest = hex(&skein_lib::sha256::digest(&worker_secret));
    let mut entries = List::with_capacity(5);
    for (path, content) in [
        (b"worker.secret".as_slice(), worker_secret.as_slice()),
        (b"worker.digest", worker_digest.as_slice()),
        (b"/opt/temper/agent.json", story.agent),
        (llm.key(), key.as_slice()),
        (forge.admin_token(), token.as_slice()),
    ] {
        entries.push((path.into(), Secret::new(content.into()))).expect("five startup files");
    }
    let mut forges = List::with_capacity(1);
    forges
        .push(temper_service::ForgeRuntime {
            number: 1,
            address: forge.listen(),
            transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
            trust_roots: 0,
            trust_root_bytes: 0,
        })
        .expect("one forge");
    Start {
        worker_path: worker_path.into(),
        document: story.deployment.into(),
        files: temper_service::settings::Files { entries },
        agent: agent_settings(story),
        seeds: temper_service::settings::Seeds {
            deployment: seeds.service.deployment,
            seed: seeds.service.seed,
            crypto: seeds.service.crypto.clone(),
            installation: seeds.service.installation,
        },
        forges,
        user: USER,
    }
}
fn agent_settings(story: &Story) -> temper_service::settings::Agent {
    let configuration = smith_agent_shell::config::parse(story.agent).expect("story agent");
    let limits = &configuration.service.limits;
    let host_limits =
        smith_local_service::profile::host_limits(&configuration.profile, limits).expect("agent host limits");
    let channel = limits.channel;
    let mut protocol = smith_host_protocol::Limits {
        bodies: channel.bodies,
        charter: channel.charter,
        transcript: channel.transcript,
        endpoints: channel.endpoints,
        channel: channel.channel,
        start_bytes: channel.start_bytes,
        calls: host_limits.calls,
    };
    protocol.channel.output_bytes = smith_host_protocol::output_bytes(&protocol).expect("agent output bound");
    let models = configuration
        .declarations
        .endpoints
        .iter()
        .flat_map(|endpoint| endpoint.models.iter())
        .map(|model| jig_domain_host::ModelBound {
            name: model.name.clone(),
            window: model.window,
            output: model.output,
        })
        .collect();
    let policy = configuration.profile.policy;
    temper_service::settings::Agent {
        tool_payload: configuration.profile.declared.tool_payload,
        programs: Box::new([]),
        endpoints: configuration.service.channel_endpoints,
        limits: protocol,
        host_limits,
        bounds: jig_domain_host::Bounds {
            program_endpoints: Box::new([]),
            models,
            turns: policy.max_turns,
            spend: policy.max_spend,
            time: policy.max_time,
            waiting: policy.max_waiting,
            checks: policy.check_timeout,
            message_bytes: limits
                .domain
                .run
                .message_bytes
                .checked_sub(u32::try_from(temper_worker_domain::SEPARATOR).expect("separator"))
                .expect("message separator"),
            inbox: policy.inbox,
            unacknowledged: policy.unacknowledged,
        },
    }
}

fn hex(bytes: &[u8; 32]) -> Vec<u8> {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in bytes {
        write!(text, "{byte:02x}").expect("format hexadecimal into a String");
    }
    text.into_bytes()
}
