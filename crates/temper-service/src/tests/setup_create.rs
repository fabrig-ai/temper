//! The service's local setup sequence against the shared fake forge's records.
use super::*;
use skein_fake_forge_domain as git;
use skein_fake_forge_protocol as fake;
use skein_forge_machine_world::{
    fixtures as forge_fixtures,
    pipe::{Cut, Pipe},
};
use skein_lib::stream::Down;

pub(super) struct Forge {
    pub(super) service: fake::Service,
    env: Env<fake::Limits>,
    pub(super) git: git::Domain,
    pub(super) config: git::Config,
    socket: Option<(Token, bool)>,
    server: Option<fake::Server>,
    pipe: Pipe,
    events: Queue<fake::Event>,
    down: Queue<Down>,
    requests: List<Box<[u8]>>,
}
impl Forge {
    pub(super) fn new(refuse_file: bool) -> Self {
        let limits = forge_fixtures::fake_limits();
        let config = fake::Config {
            hooks: None,
            base: b"http://127.0.0.1:9".as_slice().into(),
            tokens: List::with_capacity(0),
            users: List::with_capacity(0),
            repositories: List::with_capacity(0),
            max_response_items: 32,
            default_paging_num: 30,
            faults: fake::Faults { reset: 0, drop: 0, server_error: 0 },
        };
        let mut service = fake::Service::new(config, &limits, 7).unwrap();
        let store = service.setup_mut();
        let admin = store.account(b"temper-admin", true, b"");
        store.token(
            admin,
            b"handover",
            &[b"write:admin", b"read:user", b"write:organization", b"write:repository"],
            Secret::new(b"test-admin".as_slice().into()),
        );
        store.account(b"owner", true, b"");
        let bot = store.account(b"temper", false, b"temper:bot");
        store.token(
            bot,
            b"temper-api",
            &[b"read:user", b"write:repository"],
            Secret::new(b"test-api".as_slice().into()),
        );
        store.token(
            bot,
            b"temper-git",
            &[b"read:user", b"write:repository"],
            Secret::new(b"test-git".as_slice().into()),
        );
        let mut config = forge_fixtures::fake_config();
        config.latency_min = Duration::ZERO;
        config.latency_max = Duration::ZERO;
        config.limits.repositories = 4;
        config.limits.name_bytes = 512;
        config.limits.content_bytes = if refuse_file { 128 } else { 4096 };
        Self {
            service,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            git: git::Domain::new(&config, 7),
            config,
            socket: None,
            server: None,
            pipe: Pipe::new(7, Cut::Never),
            events: Queue::with_capacity(8),
            down: Queue::with_capacity(32),
            requests: List::with_capacity(4096),
        }
    }
    fn permit_file(&mut self) {
        self.config.limits.content_bytes = 4096;
    }
    fn turn(&mut self, service: &mut crate::Service) {
        while service.engine.as_mut().unwrap().notes.pop().is_some() {}
        self.env.now = self.env.now.checked_add(Duration::from_millis(1)).unwrap();
        let engine = service.engine.as_mut().unwrap();
        engine.env.now = self.env.now;
        advance(engine);
        engine.forge.iterate(self.env.now, Wall::EPOCH);
        self.turn_io(engine);
        self.turn_server(engine);
        for _ in 0_u32..4 {
            root_turn(engine);
        }
        self.service.reclaim();
    }
    fn record_requests(&mut self) {
        for line in self.pipe.request_lines() {
            self.requests.push(Box::from(line.as_slice())).unwrap();
        }
    }
    pub(super) fn turn_io(&mut self, engine: &mut crate::engine::Engine) {
        for setup in [true, false] {
            let io = if setup { &mut engine.setup.io } else { &mut engine.forge.io };
            let mut pending = List::with_capacity(32);
            while let Some(request) = io.pop() {
                pending.push(request).unwrap();
            }
            for request in pending.into_boxed() {
                match request {
                    skein_io::Request::Connect { owner, .. } => {
                        assert!(self.socket.is_none());
                        self.socket = Some((owner, setup));
                        self.pipe = Pipe::new(7, Cut::Never);
                        let mut server = fake::Server::new(owner, &self.env.limits).unwrap();
                        fake::start(&mut server, &mut self.service, &self.env, &mut self.events, &mut self.down);
                        self.server = Some(server);
                        for event in [
                            skein_io::Event::Connecting { owner, socket: Token::new(123) },
                            skein_io::Event::Connected { owner },
                        ] {
                            if setup {
                                engine.setup.up(event);
                            } else {
                                engine.forge.up(event);
                            }
                        }
                    }
                    skein_io::Request::Stream { down, .. } => self.pipe.client_down(down),
                    skein_io::Request::Close { .. } | skein_io::Request::Abort { .. } => {
                        let (owner, _) = self.socket.unwrap();
                        self.record_requests();
                        self.socket = None;
                        self.server = None;
                        if setup {
                            engine.setup.up(skein_io::Event::Closed { owner });
                        } else {
                            engine.forge.up(skein_io::Event::Closed { owner });
                        }
                    }
                    skein_io::Request::Listen { .. }
                    | skein_io::Request::Bind { .. }
                    | skein_io::Request::Reject { .. }
                    | skein_io::Request::Output { .. }
                    | skein_io::Request::Spawn { .. }
                    | skein_io::Request::Signal { .. }
                    | skein_io::Request::Usage { .. }
                    | skein_io::Request::Window { .. } => panic!("unexpected forge io request"),
                }
            }
        }
    }
    pub(super) fn turn_server(&mut self, engine: &mut crate::engine::Engine) {
        if let Some(server) = &mut self.server {
            fake::resume_with_domain(
                server,
                &mut self.service,
                fake::Hosted { domain: &mut self.git, config: &self.config },
                &self.env,
                &mut self.events,
                &mut self.down,
            );
            let _head = server.take_head();
            while let Some(down) = self.down.pop() {
                self.pipe.server_down(down);
            }
            if let Some(up) = self.pipe.server_up() {
                fake::up_with_domain(
                    server,
                    &mut self.service,
                    fake::Hosted { domain: &mut self.git, config: &self.config },
                    &self.env,
                    up,
                    &mut self.events,
                    &mut self.down,
                );
            }
            let _head = server.take_head();
            if let Some(up) = self.pipe.client_up() {
                let (owner, setup) = self.socket.unwrap();
                if setup {
                    engine.setup.up(skein_io::Event::Stream { owner, up });
                } else {
                    engine.forge.up(skein_io::Event::Stream { owner, up });
                }
            }
            while let Some(event) = self.events.pop() {
                match event {
                    fake::Event::Domain(event) => {
                        let mut replies = Queue::with_capacity(git::MAX_OUT);
                        let env = Env { now: self.env.now, wall: Wall::EPOCH, limits: self.config };
                        git::step(&mut self.git, &env, event, &mut replies);
                        git::fire(&mut self.git, &env, &mut replies);
                        while let Some(reply) = replies.pop() {
                            if let git::Request::Reply { .. } = reply {
                                fake::down(
                                    server,
                                    &mut self.service,
                                    &self.env,
                                    reply,
                                    &mut self.events,
                                    &mut self.down,
                                );
                            }
                        }
                        let _head = server.take_head();
                        self.git.reclaim();
                    }
                    fake::Event::Close => {
                        fake::closed(server, &mut self.service, &self.env, &mut self.events, &mut self.down);
                    }
                    fake::Event::Closed => {}
                }
            }
        }
    }
    fn settle(&mut self, service: &mut crate::Service) {
        for _ in 0_u32..20_000 {
            self.turn(service);
            let engine = service.engine.as_ref().unwrap();
            if engine.setup.runs.is_empty() && connected(engine) {
                self.record_requests();
                return;
            }
            if failed(engine) && engine.setup.runs.is_empty() && self.socket.is_none() {
                return;
            }
        }
        panic!("local setup did not settle: {:?}", service.engine.as_ref().unwrap().domain.forge_connection(1));
    }
}
fn configured(forge: &Forge) -> crate::Service {
    let mut config = local_config(true, false);
    config.setup[0].entry.asked = true;
    config.setup[0].entry.orgs = Box::new([]);
    config.webhooks =
        Some(skein_forge_connection::hooks::Webhooks { listener: skein_io::Addr::Ip(([127, 0, 0, 1], 7101).into()) });
    let mut store = fixtures::records_store();
    for (item, bytes) in [
        (Item::AdminToken, b"test-admin".as_slice()),
        (Item::ApiToken, b"test-api"),
        (Item::GitToken, b"test-git"),
        (Item::WebhookSecret, b"test-hook"),
    ] {
        kept_record(&mut store, 1, item, bytes);
    }
    kept_record(&mut store, 1, Item::User, &forge.service.setup().find(b"temper").unwrap().id.to_le_bytes());
    let io = config.jig.limits.io;
    let (endpoints, roots) = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
    let owned = skein_forge_connection::settings::owned_bytes(&endpoints).unwrap();
    let mut service =
        crate::Service::prepared(config, &fixtures::seeds(), None, (endpoints, roots, owned), None, 0, io, true)
            .unwrap();
    service.engine.as_mut().unwrap().store = store;
    service.engine.as_mut().unwrap().box_link = crate::route::box_link::Link::new(256, true);
    service.engine.as_mut().unwrap().setup.lifecycle.local_mut(1).unwrap().pending = false;
    // Bind is covered independently by setup_bound_webhook; use the checked
    // configured listener here so the fake socket loop owns only forge traffic.
    service.engine.as_mut().unwrap().setup.waiting_for_webhooks = false;
    while service.engine.as_mut().unwrap().forge.io.pop().is_some() {}
    restore_local(&mut service);
    service
}
fn ask(service: &mut crate::Service, number: u64) {
    ask_copy(service, number, false);
}
fn ask_copy(service: &mut crate::Service, number: u64, playground: bool) {
    let engine = service.engine.as_mut().unwrap();
    engine.step(root::Event::Forge(Box::new(domain::Event::Connection {
        forge: 1,
        state: domain::Connection::SetUpFailed { why: b"retry".as_slice().into() },
    })));
    engine.step(root::Event::Forge(Box::new(domain::Event::Application {
        role: None,
        carried: None,
        request: Token::new(number),
        token: Token::new(number.checked_add(1).unwrap()),
        retention: Duration::from_secs(60),
        ask: domain::Ask::SetUp {
            playground,
            forge: 1,
            org: if playground { b"playground".as_slice().into() } else { b"work".as_slice().into() },
            repository: if playground { b"widget".as_slice().into() } else { b"hello".as_slice().into() },
            project: domain::SetUpProject::Existing(1),
        },
    })));
    // Drive the root's journal seam, including its child continuations.
    for _ in 0_u32..256 {
        root_turn(engine);
        if engine.box_link.write.is_some() {
            break;
        }
    }
    assert!(engine.box_link.write.is_some(), "admitted intent reaches routing: {:?}", engine.domain.forges());
    engine.box_link.write.take().expect("start line");
    let mut out = Queue::with_capacity(3);
    engine.box_link.read(b"started 1\n", &mut out);
    crate::route::box_link::drain(engine, &mut out);
    read_handover(service);
}
fn read_handover(service: &mut crate::Service) {
    let read = service.take_handover_read().unwrap();
    service.handover_read(1, read.run, Ok(None));
}
#[test]
fn reconcile_then_create_then_connect_makes_the_first_repository_and_bot_workflow() {
    let mut forge = Forge::new(false);
    let mut service = configured(&forge);
    ask(&mut service, 91);
    forge.settle(&mut service);
    let engine = service.engine.as_ref().unwrap();
    assert!(!failed(engine), "{:?}: {:?}", engine.domain.forge_connection(1), forge.requests);
    assert_eq!(engine.domain.forges().rows[0].first.as_ref().unwrap().state, domain::FirstState::Made { id: 1 });
    assert!(connected(engine));
    let reads = &forge.requests;
    let mut admin_reads = List::with_capacity(64);
    let mut creation = None;
    for (at, line) in reads.as_slice().iter().enumerate() {
        if line.starts_with(b"GET /api/v1/user ") {
            admin_reads.push(at).unwrap();
        }
        if creation.is_none() && line.starts_with(b"GET /api/v1/orgs/work ") {
            creation = Some(at);
        }
    }
    let creation = creation.unwrap();
    let admin_reads = admin_reads.as_slice();
    assert!(admin_reads[0] < admin_reads[1] && admin_reads[1] < creation && creation < *admin_reads.last().unwrap());
    assert_eq!(forge.service.setup().organization(b"work").unwrap().owner.as_ref(), b"owner");
    assert!(forge.service.setup().is_private(1));
    assert_eq!(forge.service.setup().collaborator(1, b"temper"), Some(skein_forge::forgejo::request::Grant::Write));
    assert_eq!(forge.service.setup().repository_hooks(1).count(), 1);
    let head = forge.git.branch(b"work/hello", b"main").unwrap();
    let object = forge.git.object(head).unwrap();
    assert_eq!(object.tree.get(b"README.md".as_slice()).unwrap().as_ref(), b"# hello\n");
    assert_eq!(
        object.tree.get(temper_forge_setup::STARTER_PATH).unwrap().as_ref(),
        temper_forge_setup::starter_workflow(b"temper-host").unwrap().as_ref()
    );
    assert!(object.tree.get(temper_forge_setup::PLAYGROUND_PATH).is_none());
    assert_eq!(object.author.as_ref().unwrap().name.as_ref(), b"temper");
    assert_eq!(object.author, object.committer);
}
#[test]
fn no_first_intent_reconciles_and_connects_without_create() {
    let mut forge = Forge::new(false);
    let mut service = configured(&forge);
    lifecycle::started(service.engine.as_mut().unwrap(), 1);
    read_handover(&mut service);
    forge.settle(&mut service);
    assert!(service.engine.as_ref().unwrap().domain.forges().rows[0].first.is_none());
    for line in forge.requests.as_slice() {
        assert!(!line.starts_with(b"GET /api/v1/orgs/") && !line.starts_with(b"POST /api/v1/orgs/"));
    }
    assert!(forge.service.setup().organization(b"work").is_none());
}
#[test]
fn a_file_stop_names_the_step_and_a_second_ask_finishes_once() {
    let mut forge = Forge::new(true);
    let mut service = configured(&forge);
    ask(&mut service, 91);
    forge.settle(&mut service);
    let engine = service.engine.as_ref().unwrap();
    assert!(engine.forge.starts.is_empty());
    assert_eq!(
        engine.domain.forges().rows[0].first.as_ref().unwrap().state,
        domain::FirstState::Failed { step: domain::CreateStep::File }
    );
    assert_eq!(
        engine.domain.forge_connection(1),
        domain::Connection::SetUpFailed { why: b"making work/hello: File (Refused(413))".as_slice().into() }
    );
    let readme = forge.git.branch(b"work/hello", b"main").unwrap();
    assert!(forge.git.object(readme).unwrap().tree.get(temper_forge_setup::STARTER_PATH).is_none());
    forge.permit_file();
    ask(&mut service, 93);
    forge.settle(&mut service);
    assert_eq!(
        service.engine.as_ref().unwrap().domain.forges().rows[0].first.as_ref().unwrap().state,
        domain::FirstState::Made { id: 1 },
        "{:?}",
        service.engine.as_ref().unwrap().domain.forge_connection(1)
    );
    assert!(connected(service.engine.as_ref().unwrap()));
    let head = forge.git.branch(b"work/hello", b"main").unwrap();
    assert_eq!(forge.git.object(head).unwrap().parent, Some(readme));
    assert_eq!(
        forge.git.object(head).unwrap().tree.get(temper_forge_setup::STARTER_PATH).unwrap().as_ref(),
        temper_forge_setup::starter_workflow(b"temper-host").unwrap().as_ref()
    );
}

fn root_turn(engine: &mut crate::engine::Engine) {
    // These asks enter at the setup routing seam; the root's application
    // acknowledgement is exercised separately in the root setup tests.
    let mut events = Queue::with_capacity(engine.events.len());
    while let Some(event) = engine.events.pop() {
        if let root::Event::Forge(forge) = &event
            && let domain::Event::SetUpAnswered { .. } = forge.as_ref()
        {
            continue;
        }
        events.push(event);
    }
    while let Some(event) = events.pop() {
        engine.events.push(event);
    }
    crate::engine::iterate(engine, engine.env.now, Wall::EPOCH);
}

fn connected(engine: &crate::engine::Engine) -> bool {
    match engine.domain.forge_connection(1) {
        domain::Connection::Connected { .. } => true,
        domain::Connection::NotConnected
        | domain::Connection::NotSetUp
        | domain::Connection::SettingUp { .. }
        | domain::Connection::SetUpFailed { .. }
        | domain::Connection::Connecting
        | domain::Connection::Stopped { .. } => false,
    }
}
fn failed(engine: &crate::engine::Engine) -> bool {
    match engine.domain.forge_connection(1) {
        domain::Connection::SetUpFailed { .. } => true,
        domain::Connection::NotConnected
        | domain::Connection::NotSetUp
        | domain::Connection::SettingUp { .. }
        | domain::Connection::Connected { .. }
        | domain::Connection::Connecting
        | domain::Connection::Stopped { .. } => false,
    }
}

#[test]
fn playground_create_copies_the_fake_source_beside_the_starter_and_keeps_forgejos_readme() {
    let mut forge = Forge::new(false);
    let mut service = configured(&forge);
    ask_copy(&mut service, 91, true);
    forge.settle(&mut service);
    let engine = service.engine.as_ref().unwrap();
    assert!(connected(engine), "{:?}", engine.domain.forge_connection(1));
    assert!(engine.domain.forges().rows[0].first.as_ref().unwrap().playground);
    let head = forge.git.branch(b"playground/widget", b"main").unwrap();
    let object = forge.git.object(head).unwrap();
    assert_eq!(object.tree.get(b"README.md".as_slice()).unwrap().as_ref(), b"# widget\n");
    assert_eq!(
        object.tree.get(temper_forge_setup::STARTER_PATH).unwrap().as_ref(),
        temper_forge_setup::starter_workflow(b"temper-host").unwrap().as_ref()
    );
    assert_eq!(
        object.tree.get(temper_forge_setup::PLAYGROUND_PATH).unwrap().as_ref(),
        temper_forge_setup::PLAYGROUND_MAIN
    );
    assert!(object.tree.get(b".forgejo/workflows/check.yml".as_slice()).is_none());
}
