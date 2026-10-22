use super::fixtures;
use crate::worker_settings;

#[test]
fn the_link_s_credential_names_temper_s_payload_version() {
    let settings = fixtures::worker_settings();
    let files = fixtures::worker_files();
    let secret = files.contents(&settings.jig.secret).unwrap();
    let credential = worker_settings::credential(&settings, secret);
    assert_eq!(credential.payload, temper_protocol_worker_wire::PAYLOAD_VERSION);
    assert_eq!(credential.secret.expose(), b"worker-secret-value");
    assert_eq!(credential.name.as_ref(), b"worker-a");
}

#[test]
fn the_worker_sizes_each_part_from_its_payload_codec() {
    let settings = fixtures::worker_settings();
    let agent = fixtures::worker_agent();
    let limits = fixtures::worker_limits();
    let prepared = worker_settings::prepare(&settings, &agent, &limits).unwrap();
    assert_eq!(prepared.root.host.slots, 1);
    assert_eq!(prepared.root.checkout.repositories, 1);
    assert_eq!(prepared.prefix, (3_u64 << 60_u32) + (1_u64 << 48_u32));
    assert_eq!(u64::from(prepared.link.sizes.items), v1::Items::worst_case_bytes(&prepared.payload).unwrap());
    assert_eq!(prepared.payload.repository_git_some, 4096);
    assert!(worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap() > 0);
}

#[test]
fn a_workspace_start_carries_one_path_per_directory() {
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let mut start = decoded_chat_start(&agent);
    let mut directories = skein_lib::List::with_capacity(2);
    for (token, name, writable) in [(73, b"second".as_slice(), true), (41, b"first".as_slice(), false)] {
        directories
            .push(smith::Directory {
                root: Token::new(token),
                name: name.into(),
                host: Box::new([]),
                writable,
                git: true,
                conflicts: Box::new([]),
            })
            .unwrap();
    }
    start.directories = directories.into_boxed();
    let table = jig_protocol_accounts::table::Table::new(0, 0);
    let values = crate::worker::start_values(&table, &start, b"/worker", fixtures::NOW).unwrap();
    assert_eq!(
        values.paths.as_ref(),
        [Box::from(b"/worker/workspaces/73/second".as_slice()), Box::from(b"/worker/workspaces/41/first".as_slice()),]
    );
    encode_grants(*start, values, &agent).expect("two prepared directories encode with their paths");
}

fn decoded_chat_start(agent: &crate::settings::Agent) -> Box<smith_host_domain::Start> {
    let link::Request::Down { down: hub::LinkDown::Assign { assignment }, .. } = fixtures::chat_assignment() else {
        panic!("chat assignment")
    };
    let source = assignment.assignment;
    crate::worker_route::decode(
        source.run,
        hub::Start {
            workspace: Some(Token::new(3)),
            charter: source.charter,
            activation: 1,
            identity: source.identity,
            turns: Box::new([]),
            answered: Box::new([]),
            messages: Box::new([]),
            grants: Box::new([]),
        },
        agent,
    )
    .unwrap()
}

use jig_domain_host as hub;
use jig_protocol_engine_workers as engine_link;
use jig_protocol_worker_link as link;
use jig_protocol_worker_wire::Part;
use skein_channel::{Lower, LowerEvent};
use skein_io as io;
use skein_lib::{Duration, Intake, Queue, Reader, Secret, Time, Token, Wall, stream};
use temper_protocol_worker_wire::v1;
use temper_worker_domain::{self as root, wire};

#[test]
fn an_assignment_with_items_reaches_the_root_decoded_with_its_save() {
    let mut worker = fixtures::worker();
    worker.link(fixtures::NOW, fixtures::repository_assignment());
    let root::Event::Assign { assignment, items, save } = worker.events.pop().unwrap() else {
        panic!("assignment with items");
    };
    assert_eq!(items.unwrap(), fixtures::one_repository());
    assert_eq!(save.as_deref(), Some(b"temper/saved-7".as_slice()));
    assert!(assignment.assignment.save);
    assert_eq!(assignment.assignment.workspace.unwrap().workstream, 7);
}

#[test]
fn undecodable_items_refuse_the_start_as_invalid_naming_them() {
    let mut peer = LinkPeer::new();
    let mut worker = fixtures::started_worker().worker;
    peer.open(&mut worker);
    let link::Request::Down { down, .. } = fixtures::repository_assignment() else { panic!("assignment") };
    worker.link(
        fixtures::NOW,
        link::Request::Down { down, parts: Box::new([Part { token: Token::new(0), bytes: Box::new([255]) }]) },
    );
    let (up, parts) = peer.answer(&mut worker);
    let hub::LinkUp::Answer { answer, .. } = up else { panic!("refused answer") };
    assert_eq!(
        answer.ending,
        hub::Ending::Refused(hub::Refusal::Invalid(hub::Invalid::Start { why: b"workspace items".as_slice().into() }))
    );
    assert!(parts.is_empty());
    assert_eq!(worker.root.host().hosted(), 0);
}

#[test]
fn an_answer_goes_up_with_its_left_and_saved_parts() {
    let mut peer = LinkPeer::new();
    let mut worker = fixtures::started_worker().worker;
    peer.open(&mut worker);
    let hub::LinkUp::Answer { run, attempt, answer } = fixtures::answer(hub::Ending::Parked {
        work: hub::Work { left: Some(Token::new(11)), saved: Some(Token::new(12)) },
    }) else {
        panic!("answer")
    };
    worker.route(root::Request::Answer {
        run,
        attempt,
        answer,
        preparation: None,
        work: wire::Work {
            landed: Box::new([wire::Landed { tag: 3, commit: [7; 32] }]),
            saved: Some(Box::new([wire::SavedRepository { tag: 3, from_saved: true, landing: wire::Landing::Moved }])),
        },
    });
    let (up, parts) = peer.answer(&mut worker);
    let hub::LinkUp::Answer { run, attempt, answer } = up else { panic!("answer crossed") };
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    let hub::Ending::Parked { work } = answer.ending else { panic!("parked") };
    assert_eq!(parts.len(), 2);
    assert_eq!(work.left, Some(parts[0].token));
    assert_eq!(work.saved, Some(parts[1].token));
    let left = v1::Left::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[0].bytes)).unwrap();
    let saved = v1::Saved::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[1].bytes)).unwrap();
    assert_eq!((left.landed().get(0).unwrap().tag(), left.landed().get(0).unwrap().commit()), (3, &[7; 32]));
    let row = saved.repositories().get(0).unwrap();
    assert_eq!((row.tag(), row.from_saved(), row.landing()), (3, true, &v1::Landing::Moved));
}

#[test]
fn a_git_identity_s_value_lands_in_the_identities_table_only() {
    let mut worker = fixtures::worker();
    worker.link(fixtures::NOW, fixtures::application_value(1, 2, b"tok-2"));
    worker.link(fixtures::NOW, fixtures::application_value(1, 1, b"tok-1"));
    assert_eq!(
        worker.identities.newest(1, fixtures::NOW),
        Some(&crate::identities::Value::Forgejo(fixtures::identity(b"tok-2")))
    );
    assert_eq!(worker.values.value(1, 2, fixtures::NOW), None);
    assert!(worker.events.is_empty(), "values stay below the root");
    worker.link(
        fixtures::NOW,
        link::Request::Value {
            account: 1,
            generation: 3,
            expires: Time::from_nanos(u64::MAX),
            value: link::GrantValue::Application { secret: Secret::new(Box::new([255])) },
        },
    );
    assert_eq!(worker.undecodable_values, 1);
    assert_eq!(worker.identities.newest(1, fixtures::NOW), None);
}

#[test]
fn the_agent_runs_as_temper_agent_with_no_credential_in_its_environment() {
    let mut worker = fixtures::started_worker().worker;
    worker.link(fixtures::NOW, fixtures::chat_assignment());
    let (_, launched) = fixtures::spawn(&mut worker);
    assert_eq!(launched.program.as_ref(), fixtures::TEMPER);
    assert_eq!(launched.args.as_ref(), [Box::from(&b"agent"[..]), Box::from(fixtures::AGENT_JSON)]);
    assert_eq!(launched.root, io::kernel::Fd::new(9));
    assert!(launched.env.is_empty(), "no credential reaches an agent");
}

#[test]
fn a_worker_s_secret_is_read_and_checked_at_its_path() {
    use jig_protocol_config::{Path, Problem, Segment, What};
    let empty = crate::settings::Files { entries: skein_lib::List::with_capacity(0) };
    for (row, files, secret_bound, expected) in
        [("missing", empty, 64, What::Unreadable), ("too long", fixtures::worker_files(), 1, What::Bound)]
    {
        let mut settings = fixtures::worker_settings();
        settings.jig.limits.link.secret = secret_bound;
        let problem = crate::Worker::new(
            &settings,
            &fixtures::worker_agent(),
            &fixtures::worker_limits(),
            &files,
            crate::WorkerRuntime { root: io::kernel::Fd::new(9), path: b"/worker".as_slice().into() },
            7,
        )
        .unwrap_err();
        assert_eq!(
            problem.problems.get(0),
            Some(&Problem {
                path: Path::root().member(b"workers").child(Segment::Index(0)).member(b"secret"),
                what: expected,
            }),
            "{row}"
        );
    }
}

struct LinkPeer {
    component: engine_link::Component,
    out: Queue<engine_link::Request>,
    engine_bytes: Intake,
    worker_bytes: Intake,
    engine_read: Option<stream::Read>,
    worker_read: Option<stream::Read>,
    owner: Token,
}
impl LinkPeer {
    fn new() -> Self {
        let settings = fixtures::worker_settings();
        let prepared =
            worker_settings::prepare(&settings, &fixtures::worker_agent(), &fixtures::worker_limits()).unwrap();
        let limits = engine_link::Limits {
            sizes: prepared.link.sizes,
            names: 1,
            workers: 1,
            opening: 1,
            chunk: prepared.link.chunk,
            handshake: prepared.link.handshake,
            hello: prepared.link.handshake,
            ping: prepared.link.ping,
            silence: prepared.link.silence,
            stall: prepared.link.stall,
        };
        Self {
            component: engine_link::Component::new(
                &limits,
                Box::new([engine_link::Worker {
                    name: settings.jig.name,
                    digest: io::digest::digest(fixtures::worker_files().contents(b"worker-secret").unwrap()),
                }]),
                temper_protocol_worker_wire::PAYLOAD_VERSION,
            )
            .unwrap(),
            out: Queue::with_capacity(64),
            engine_bytes: Intake::with_capacity(65536),
            worker_bytes: Intake::with_capacity(65536),
            engine_read: None,
            worker_read: None,
            owner: Token::new(prepared.prefix),
        }
    }
    fn open(&mut self, worker: &mut crate::Worker) {
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        worker.up(fixtures::NOW, io::Event::Connecting { owner: self.owner, socket: Token::new(44) });
        worker.up(fixtures::NOW, io::Event::Connected { owner: self.owner });
        self.component.down(fixtures::NOW, engine_link::Event::Accepted { connection: Token::new(1) }, &mut self.out);
        for _ in 0_u16..1000 {
            if let Some(engine_link::Request::Hello { .. }) = self.turn(worker) {
                return;
            }
        }
        panic!("worker opens and says hello");
    }
    fn answer(&mut self, worker: &mut crate::Worker) -> (hub::LinkUp, Box<[Part]>) {
        for _ in 0_u16..1000 {
            if let Some(engine_link::Request::Up { up: up @ hub::LinkUp::Answer { .. }, parts, .. }) = self.turn(worker)
            {
                return (up, parts);
            }
        }
        panic!("worker sends its answer");
    }
    fn turn(&mut self, worker: &mut crate::Worker) -> Option<engine_link::Request> {
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        while let Some(request) = worker.io().pop() {
            match request {
                io::Request::Output { down: stream::OutputDown::Room { right, .. }, .. } => worker.up(
                    fixtures::NOW,
                    io::Event::Output {
                        owner: self.owner,
                        up: stream::OutputUp::Settled { right, outcome: stream::OutputOutcome::Granted },
                    },
                ),
                io::Request::Output { down: stream::OutputDown::Send { bytes, .. }, .. } => {
                    self.engine_bytes.append(&bytes).unwrap();
                }
                io::Request::Stream { down: stream::Down::Demand { read, .. }, .. } => self.worker_read = Some(read),
                other @ (io::Request::Listen { .. }
                | io::Request::Connect { .. }
                | io::Request::Bind { .. }
                | io::Request::Reject { .. }
                | io::Request::Stream { .. }
                | io::Request::Output { .. }
                | io::Request::Spawn { .. }
                | io::Request::Signal { .. }
                | io::Request::Usage { .. }
                | io::Request::Window { .. }
                | io::Request::Close { .. }
                | io::Request::Abort { .. }) => panic!("link request: {other:?}"),
            }
        }
        while let Some(request) = self.out.pop() {
            match request {
                engine_link::Request::Lower { lower: Lower::Write(stream::OutputDown::Room { right, .. }), .. } => self
                    .lower(LowerEvent::Write(stream::OutputUp::Settled {
                        right,
                        outcome: stream::OutputOutcome::Granted,
                    })),
                engine_link::Request::Lower { lower: Lower::Write(stream::OutputDown::Send { bytes, .. }), .. } => {
                    self.worker_bytes.append(&bytes).unwrap();
                }
                engine_link::Request::Lower { lower: Lower::Read(stream::Down::Demand { read, .. }), .. } => {
                    self.engine_read = Some(read);
                }
                engine_link::Request::Take { .. } => {}
                request @ (engine_link::Request::Hello { .. } | engine_link::Request::Up { .. }) => {
                    return Some(request);
                }
                other @ (engine_link::Request::TurnAway { .. }
                | engine_link::Request::Lower { .. }
                | engine_link::Request::Close { .. }
                | engine_link::Request::Abort { .. }
                | engine_link::Request::Lost { .. }
                | engine_link::Request::Closed) => panic!("engine request: {other:?}"),
            }
        }
        if let Some(read) = self.engine_read
            && let Some(bytes) = self.engine_bytes.meet(read)
        {
            self.engine_read = None;
            self.lower(LowerEvent::Read(stream::Up::Bytes(bytes)));
        }
        if let Some(read) = self.worker_read
            && let Some(bytes) = self.worker_bytes.meet(read)
        {
            self.worker_read = None;
            worker.up(fixtures::NOW, io::Event::Stream { owner: self.owner, up: stream::Up::Bytes(bytes) });
        }
        None
    }
    fn lower(&mut self, event: LowerEvent) {
        self.component.down(
            fixtures::NOW,
            engine_link::Event::Lower { connection: Token::new(1), event },
            &mut self.out,
        );
    }
}

#[test]
fn retained_program_metadata_is_priced_exactly_once_at_capacity() {
    use crate::settings::{Adapter, PROGRAM_PATH_BYTES, Program};
    let (settings, mut agent, limits) = program_worker_settings();
    let without = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    let empty_heap = worker_heap(&settings, &agent, &limits, without);
    let path: Box<[u8]> = Box::new([b'/'; PROGRAM_PATH_BYTES]);
    agent.programs = Box::new([Program { adapter: Adapter::ClaudeCode, path: path.clone(), directory: path }]);
    let with = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    assert_eq!(with - without, u64::try_from(2 * PROGRAM_PATH_BYTES + size_of::<Program>()).unwrap());
    let declared_heap = worker_heap(&settings, &agent, &limits, with);
    assert_eq!(declared_heap - empty_heap, with - without, "the retained copy is priced once");
}
#[test]
fn worker_preparation_refuses_a_program_path_past_capacity() {
    use crate::settings::{Adapter, PROGRAM_PATH_BYTES, Program};
    let (settings, mut agent, limits) = program_worker_settings();
    agent.programs = Box::new([Program {
        adapter: Adapter::ClaudeCode,
        path: Box::new([b'/'; PROGRAM_PATH_BYTES + 1]),
        directory: Box::new([b'/'; PROGRAM_PATH_BYTES]),
    }]);
    worker_settings::prepare(&settings, &agent, &limits).unwrap_err();
}

fn worker_heap(
    settings: &crate::settings::Worker,
    agent: &crate::settings::Agent,
    limits: &crate::limits::Limits,
    bound: u64,
) -> u64 {
    let files = fixtures::worker_files();
    let meter = skein_world::domain::heap::Meter::new();
    meter.start();
    let worker = crate::Worker::new(settings, agent, limits, &files, fixtures::worker_runtime(), 7).unwrap();
    let measured = meter.end();
    let held = measured.held();
    meter.check(measured, bound, &"worker construction");
    drop(worker);
    held
}

#[test]
fn start_and_renewal_require_the_declared_form_and_live_generation() {
    use jig_protocol_accounts::table::{Scheme, Table, Value};
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let forms = [
        smith::Form::Credential(smith::Scheme::Bearer),
        smith::Form::Credential(smith::Scheme::Key),
        smith::Form::Withdrawn,
    ];
    for (scheme, expected) in [
        (Scheme::Bearer, forms[0]),
        (Scheme::Key, forms[1]),
        (Scheme::Withdrawn { id: Secret::new(b"provider-account".as_slice().into()) }, forms[2]),
    ] {
        let mut table = Table::new(1, 64);
        for generation in [2, 3] {
            table
                .granted(
                    7,
                    generation,
                    Value { scheme: scheme.clone(), token: Secret::new(b"credential".as_slice().into()) },
                    Time::from_nanos(9),
                )
                .unwrap();
        }
        for (number, generation, now, live) in
            [(7, 2, 8, true), (7, 3, 8, true), (7, 1, 8, false), (7, 3, 9, false), (8, 3, 8, false)]
        {
            for form in forms {
                let grant = smith::Grant { account: number, generation, valid: Duration::from_secs(5), form };
                let now = Time::from_nanos(now);
                let mut start = decoded_chat_start(&agent);
                start.grants = Box::new([grant]);
                let opening = crate::worker::start_values(&table, &start, b"/worker", now);
                let renewal = crate::worker::grant_value(&table, grant, now);
                let accepted = live && form == expected;
                assert_eq!(opening.is_some(), accepted, "start {grant:?} at {now:?}");
                assert_eq!(renewal.is_some(), accepted, "renewal {grant:?} at {now:?}");
                if let Some(values) = opening {
                    assert_eq!(smith_host_protocol::grant_form(&values.credentials[0]).unwrap(), form);
                    encode_grants(*start, values, &agent).unwrap();
                }
            }
        }
    }
}

#[test]
fn a_program_grant_lends_only_its_validated_name_without_a_table_entry() {
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let table = jig_protocol_accounts::table::Table::new(0, 0);
    for size in [0, 1, smith::SourceName::BYTES, smith::SourceName::BYTES + 1] {
        let bytes = repeated(b'a', size);
        let Some(name) = smith::SourceName::new(&bytes) else {
            assert!(size == 0 || size > smith::SourceName::BYTES);
            continue;
        };
        let grant = smith::Grant {
            account: 17,
            generation: 4,
            valid: Duration::from_secs(6),
            form: smith::Form::Program(name),
        };
        let mut start = decoded_chat_start(&agent);
        start.grants = Box::new([grant]);
        let values = crate::worker::start_values(&table, &start, b"/worker", Time::ZERO).unwrap();
        assert_eq!(values.credentials.as_ref(), [smith_protocol_channel::GrantValue::Program(bytes.clone())]);
        assert_eq!(crate::worker::grant_value(&table, grant, Time::ZERO).unwrap().credentials, values.credentials);
        let encoded = encode_grants(*start, values, &agent).unwrap();
        let body = &encoded.bytes()[encoded.bytes().len() - usize::try_from(encoded.body_len()).unwrap()..];
        let decoded = smith_channel::Start::decode(&agent.limits.bodies, &mut Reader::new(body)).unwrap();
        let value = decoded.grants().get(0).unwrap();
        assert_eq!(
            (value.account(), value.generation(), value.valid()),
            (grant.account, grant.generation, grant.valid)
        );
        let smith_channel::GrantValue::Program(value) = value.value() else { panic!("only a program name") };
        assert_eq!(value.name(), bytes.as_ref());
        let mut mismatched = decoded_chat_start(&agent);
        mismatched.grants = Box::new([grant]);
        let wrong = smith_host_protocol::Values {
            paths: Box::default(),
            credentials: Box::new([smith_protocol_channel::GrantValue::Bearer(Box::default())]),
        };
        encode_grants(*mismatched, wrong, &agent).unwrap_err();
    }
    for bytes in [b"UPPER".as_slice(), b"slash/name", b"claude_code"] {
        assert!(smith::SourceName::new(bytes).is_none());
    }
}

#[test]
fn grant_sizes_take_credentials_source_names_and_generated_identities() {
    let name = u32::try_from(smith_host_domain::SourceName::BYTES).unwrap();
    for (credential, identity, expected) in [
        (name - 1, name - 1, name),
        (name, name, name),
        (name + 1, name, name + 1),
        (name, name + 1, name + 1),
        (4096, name, 4096),
    ] {
        assert_eq!(worker_settings::grant_bytes(credential, identity), Some(expected));
    }
    let mut engine = fixtures::engine();
    let mut worker = fixtures::worker_settings();
    let payload = worker_settings::payload_limits(&fixtures::worker_limits()).unwrap();
    for credential in [1, name, 4096] {
        engine.jig.limits.token = credential;
        // Table capacity, not OAuth's separate parsing bound, sizes every credential scheme.
        engine.jig.limits.driver.client.document.token_bytes = 1;
        worker.jig.limits.token = credential;
        let left = crate::workers::limits(&engine).unwrap();
        let right = worker_settings::link_limits(&worker.jig, &payload).unwrap();
        let wire = crate::translate::limits(&engine.limits).unwrap();
        let identity = u32::try_from(v1::Identity::worst_case_bytes(&wire).unwrap()).unwrap();
        assert_eq!(left.sizes.grant, credential.max(name).max(identity));
        assert_eq!(
            right.sizes.grant,
            worker_settings::credential_bytes(credential)
                .max(name)
                .max(temper_protocol_worker_wire::largest(&payload).unwrap().grant)
        );
    }
    engine.jig.limits.hub.charter_bytes = u64::MAX;
    assert!(crate::workers::limits(&engine).is_none());
    worker.jig.limits.hub.charter_bytes = u64::MAX;
    assert!(worker_settings::link_limits(&worker.jig, &payload).is_none());
    assert!(
        worker_settings::worker_worst_case(&worker, &fixtures::worker_agent(), &fixtures::worker_limits()).is_none()
    );
}

#[test]
fn multiple_models_and_providers_preserve_the_supplied_grant_list() {
    use smith_host_domain as smith;
    let mut agent = fixtures::worker_agent();
    let mut endpoints = skein_lib::List::with_capacity(2);
    for (name, number) in [(b"fake".as_slice(), 1), (b"other", 2)] {
        endpoints
            .push(smith_protocol_channel::Endpoint { name: name.into(), number, dialect: fixtures::DIALECT })
            .unwrap();
    }
    agent.endpoints = smith_protocol_channel::Endpoints::new(endpoints);
    let mut charter = decoded_chat_start(&agent).charter.into_value();
    let mut main = charter.llm.clone();
    main.account = Some(7);
    let mut alternative = main.clone();
    alternative.model = b"alternative".as_slice().into();
    let mut other = main.clone();
    other.account = Some(19);
    other.endpoint.0 = 2;
    charter.llm = main;
    charter.models = Box::new([alternative, other]);
    let charter = smith_protocol_channel::encode_charter(&charter, &agent.limits.charter, &agent.endpoints).unwrap();
    let grants = [
        hub::Grant {
            account: 19,
            generation: 8,
            valid: Duration::from_secs(11),
            form: hub::Form::Credential(hub::Scheme::Key),
        },
        hub::Grant {
            account: 7,
            generation: 3,
            valid: Duration::from_secs(5),
            form: hub::Form::Credential(hub::Scheme::Bearer),
        },
        hub::Grant { account: 99, generation: 1, valid: Duration::from_secs(6), form: hub::Form::Application },
    ];
    let start = crate::worker_route::decode(
        Token::new(31),
        hub::Start {
            workspace: None,
            charter,
            activation: 2,
            identity: [3; 16],
            turns: Box::default(),
            answered: Box::default(),
            messages: Box::default(),
            grants: grants.into(),
        },
        &agent,
    )
    .unwrap();
    assert_eq!(start.charter.value().models.len(), 2);
    assert_eq!(
        start.grants.as_ref(),
        [
            smith::Grant {
                account: 19,
                generation: 8,
                valid: Duration::from_secs(11),
                form: smith::Form::Credential(smith::Scheme::Key)
            },
            smith::Grant {
                account: 7,
                generation: 3,
                valid: Duration::from_secs(5),
                form: smith::Form::Credential(smith::Scheme::Bearer)
            },
        ]
    );
}

#[test]
fn smith_channel_notices_keep_the_source_generation_and_optional_reset() {
    use smith_host_domain as smith;
    use smith_host_protocol::{OpenEvent, ProcessEvent};
    let owner = Token::new(32);
    for (notice, expected) in [
        (
            OpenEvent::Unauthorized { account: 17, generation: 4 },
            smith::Up::Unauthorized { account: 17, generation: 4 },
        ),
        (OpenEvent::Limited { account: 17, reset: None }, smith::Up::Limited { account: 17, reset: None }),
        (
            OpenEvent::Limited { account: 17, reset: Some(Duration::ZERO) },
            smith::Up::Limited { account: 17, reset: Some(Duration::ZERO) },
        ),
        (
            OpenEvent::Limited { account: 17, reset: Some(Duration::from_secs(9)) },
            smith::Up::Limited { account: 17, reset: Some(Duration::from_secs(9)) },
        ),
        (
            OpenEvent::Refused {
                account: 17,
                code: b"quota".as_slice().into(),
                reason: b"provider refused".as_slice().into(),
            },
            smith::Up::Refused {
                account: 17,
                code: b"quota".as_slice().into(),
                reason: b"provider refused".as_slice().into(),
            },
        ),
    ] {
        let actual = crate::worker_route::process(ProcessEvent::Channel { agent: owner, event: Box::new(notice) });
        assert_eq!(
            actual,
            Some(root::Event::Process(Box::new(smith::process::Event::Received { owner, message: expected })))
        );
    }
}

fn encode_grants(
    start: smith_host_domain::Start,
    values: smith_host_protocol::Values,
    agent: &crate::settings::Agent,
) -> Result<skein_channel::Frame, smith_host_protocol::Error> {
    smith_host_protocol::encode_start(
        start,
        smith_host_domain::Window { turns: agent.host_limits.turns, bytes: agent.host_limits.unacknowledged_bytes },
        values,
        &agent.limits.bodies,
        &agent.limits.charter,
        &agent.limits.transcript,
        &agent.endpoints,
    )
}

#[test]
fn key_values_reach_smith_at_their_bound_and_fit_the_workers_memory() {
    const SOURCE: u32 = 17;
    use jig_protocol_accounts::table::{Scheme, Value};
    use smith_host_domain as smith;
    let key_bytes = jig_client_protocol_wire::v11::CEILINGS.keyed_value_some;
    let mut settings = fixtures::worker_settings();
    settings.jig.limits.token = 1;
    let agent = fixtures::worker_agent();
    let limits = fixtures::worker_limits();
    let files = fixtures::worker_files();
    let worst = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    let meter = skein_world::domain::heap::Meter::new();
    meter.start();
    let mut worker = crate::Worker::new(&settings, &agent, &limits, &files, fixtures::worker_runtime(), 7).unwrap();
    for generation in [2, 3] {
        worker
            .values
            .granted(
                SOURCE,
                generation,
                Value { scheme: Scheme::Key, token: Secret::new(repeated(b'k', usize::try_from(key_bytes).unwrap())) },
                Time::from_nanos(9),
            )
            .unwrap();
    }
    let measured = meter.end();
    meter.check(measured, worst, &"two retained full-size key generations");
    for size in [key_bytes - 1, key_bytes, key_bytes + 1] {
        let token = Secret::new(repeated(b'k', usize::try_from(size).unwrap()));
        let accepted = worker.values.granted(SOURCE, 3, Value { scheme: Scheme::Key, token }, Time::from_nanos(9));
        assert_eq!(accepted.is_ok(), size <= key_bytes);
        if accepted.is_err() {
            continue;
        }
        let mut start = decoded_chat_start(&agent);
        start.grants = Box::new([smith::Grant {
            account: SOURCE,
            generation: 3,
            valid: Duration::from_secs(5),
            form: smith::Form::Credential(smith::Scheme::Key),
        }]);
        let values = crate::worker::start_values(&worker.values, &start, b"/worker", Time::ZERO).unwrap();
        let encoded = encode_grants(*start, values, &agent).unwrap();
        let offset = encoded.bytes().len() - usize::try_from(encoded.body_len()).unwrap();
        let decoded =
            smith_channel::Start::decode(&agent.limits.bodies, &mut Reader::new(&encoded.bytes()[offset..])).unwrap();
        let smith_channel::GrantValue::Credential(value) = decoded.grants().get(0).unwrap().value() else {
            panic!("credential")
        };
        assert_eq!(value.scheme(), &smith_channel::Scheme::Key);
        assert_eq!(value.bytes().expose().len(), usize::try_from(size).unwrap());
    }
}

fn repeated(byte: u8, len: usize) -> Box<[u8]> {
    let mut bytes = skein_lib::bytes::zeroed(len);
    bytes.fill(byte);
    bytes
}

fn program_worker_settings() -> (crate::settings::Worker, crate::settings::Agent, crate::limits::Limits) {
    (fixtures::worker_settings(), fixtures::worker_agent(), fixtures::worker_limits())
}

fn socket_stand() -> fixtures::WorkerStand {
    let mut settings = fixtures::worker_settings();
    settings.jig.engine = jig_protocol_config::Address::Addr(io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
    let mut stand = fixtures::WorkerStand::new();
    stand.worker = crate::Worker::new(
        &settings,
        &fixtures::worker_agent(),
        &fixtures::worker_limits(),
        &fixtures::worker_files(),
        fixtures::worker_runtime(),
        7,
    )
    .unwrap();
    for _ in 0_u16..1000 {
        if !stand.turn() {
            return stand;
        }
    }
    panic!("socket worker starts");
}

#[test]
fn stopping_keeps_a_dial_until_the_root_sends_stop() {
    let mut stand = socket_stand();
    stand.worker.stop();
    stand.worker.link(fixtures::NOW, link::Request::Dial);
    let request = stand.worker.io().pop().expect("stopping worker still dials");
    let io::Request::Connect { addr, .. } = request else { panic!("dial reaches IO") };
    assert_eq!(addr, io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
}

#[test]
fn linked_is_emitted_after_ready_at_each_opening() {
    let mut stand = socket_stand();
    assert!(stand.worker.notes().is_empty(), "never before Ready");
    LinkPeer::new().open(&mut stand.worker);
    let expected =
        crate::Note::Linked { worker: b"worker-a".as_slice().into(), address: Box::new(([127, 0, 0, 1], 7100).into()) };
    assert_eq!(stand.worker.notes().pop(), Some(expected));
    assert!(stand.worker.notes().is_empty());
    let owner = Token::new(
        worker_settings::prepare(&fixtures::worker_settings(), &fixtures::worker_agent(), &fixtures::worker_limits())
            .unwrap()
            .prefix,
    );
    stand.worker.up(fixtures::NOW, io::Event::Closed { owner });
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let at = stand.worker.next_deadline().expect("redial");
    stand.worker.iterate(at, Wall::EPOCH);
    let Some(io::Request::Connect { .. }) = stand.worker.io().pop() else { panic!("redial before reopening") };
    LinkPeer::new().open(&mut stand.worker);
    assert_eq!(
        stand.worker.notes().pop(),
        Some(crate::Note::Linked {
            worker: b"worker-a".as_slice().into(),
            address: Box::new(([127, 0, 0, 1], 7100).into())
        })
    );
    assert!(stand.worker.notes().is_empty(), "one per opening");
}

#[test]
fn refusals_are_notes_and_leave_later_dials_intact() {
    use skein_channel::Closed;
    let mut stand = socket_stand();
    for (why, reason) in [
        (Closed::RefusedPeer(256), crate::Refusal::Peer(256)),
        (Closed::RefusedPeer(257), crate::Refusal::Peer(257)),
        (Closed::RefusedPeer(258), crate::Refusal::Peer(258)),
        (Closed::RefusedPeer(259), crate::Refusal::Peer(259)),
        (Closed::RefusedPeer(260), crate::Refusal::Peer(260)),
        (Closed::RefusedPeer(261), crate::Refusal::Peer(261)),
        (Closed::RefusedPeer(42), crate::Refusal::Peer(42)),
        (Closed::RefusedHere(2), crate::Refusal::Terms),
        (Closed::RefusedHere(3), crate::Refusal::Malformed),
    ] {
        stand.worker.link(fixtures::NOW, link::Request::Fact { why });
        assert_eq!(
            stand.worker.notes().pop(),
            Some(crate::Note::Refused {
                worker: b"worker-a".as_slice().into(),
                address: Box::new(([127, 0, 0, 1], 7100).into()),
                reason
            })
        );
        stand.worker.link(fixtures::NOW, link::Request::Dial);
        let Some(io::Request::Connect { .. }) = stand.worker.io().pop() else { panic!("redial reaches IO") };
    }
    stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::Stream });
    assert!(stand.worker.notes().is_empty());
}

fn alone_service() -> crate::Service {
    let part =
        crate::startup::worker_part(&fixtures::worker_alone_document(), &jig_protocol_config::Directories::working())
            .unwrap();
    crate::startup::worker_alone(part, &fixtures::files(), &fixtures::startup_agent(), fixtures::worker_runtime(), 7)
        .expect("worker alone prepared")
}

#[test]
fn a_worker_alone_has_no_engine_or_store_and_a_second_signal_aborts() {
    let mut service = alone_service();
    assert!(service.engine.is_none());
    assert_eq!(service.adopt_store(io::kernel::Fd::new(99), 1000), Err(io::kernel::Fd::new(99)));
    service.shutdown();
    assert!(service.worker.as_ref().unwrap().stopping());
    assert_eq!(service.end(), None);
    service.shutdown();
    assert_eq!(service.end(), Some(&crate::End::Aborted));
}

fn settle_worker_service(linked: bool) {
    let mut service = alone_service();
    let mut stand = socket_stand();
    if linked {
        LinkPeer::new().open(&mut stand.worker);
    } else {
        let owner = Token::new(
            worker_settings::prepare(
                &fixtures::worker_settings(),
                &fixtures::worker_agent(),
                &fixtures::worker_limits(),
            )
            .unwrap()
            .prefix,
        );
        stand.worker.up(fixtures::NOW, io::Event::Failed { owner, error: io::Error::Unreachable });
    }
    service.worker = Some(stand.worker);
    service.shutdown();
    stand.worker = service.worker.take().unwrap();
    let now = if linked { fixtures::NOW } else { fixtures::NOW.saturating_add(Duration::from_secs(60)) };
    for _ in 0_u16..1000 {
        stand.worker.iterate(now, Wall::EPOCH);
        // The link's Close is answered by its socket owner, as IO would.
        for _ in 0..stand.worker.io().len() {
            let request = stand.worker.io().pop().unwrap();
            match request {
                io::Request::Close { .. } | io::Request::Abort { .. } => {
                    stand.worker.up(
                        now,
                        io::Event::Closed {
                            owner: Token::new(
                                worker_settings::prepare(
                                    &fixtures::worker_settings(),
                                    &fixtures::worker_agent(),
                                    &fixtures::worker_limits(),
                                )
                                .unwrap()
                                .prefix,
                            ),
                        },
                    );
                }
                io::Request::Connect { owner, .. } => {
                    stand.worker.up(now, io::Event::Failed { owner, error: io::Error::Unreachable });
                }
                io::Request::Stream { stream, .. } | io::Request::Output { stream, .. } if stream == Token::new(44) => {
                }
                other @ (io::Request::Listen { .. }
                | io::Request::Bind { .. }
                | io::Request::Reject { .. }
                | io::Request::Stream { .. }
                | io::Request::Output { .. }
                | io::Request::Spawn { .. }
                | io::Request::Signal { .. }
                | io::Request::Usage { .. }
                | io::Request::Window { .. }) => stand.below.out.io.push(other),
            }
        }
        stand.drain();
        stand.input();
        if stand.worker.is_empty() {
            break;
        }
    }
    assert!(stand.worker.is_empty(), "all worker children and its root closed");
    service.worker = Some(stand.worker);
    crate::iterate(&mut service, now, Wall::EPOCH);
    assert_eq!(service.end(), Some(&crate::End::Drained));
    assert!(service.is_empty());
}
#[test]
fn the_first_signal_drains_a_linked_worker_without_waiting_for_grace() {
    settle_worker_service(true);
}
#[test]
fn a_worker_stopped_unlinked_has_ended_past_its_grace() {
    settle_worker_service(false);
}

#[test]
fn signals_route_to_a_worker_alone_then_abort() {
    let mut service = alone_service();
    service.told.push(io::Event::Shutdown { signal: io::kernel::ServiceSignal::Terminate });
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    assert!(service.worker.as_ref().unwrap().stopping());
    assert_eq!(service.end(), None, "its root is still closing");
    service.told.push(io::Event::Shutdown { signal: io::kernel::ServiceSignal::Terminate });
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    assert_eq!(service.end(), Some(&crate::End::Aborted));
}

#[test]
fn repeated_refusals_are_noted_once_until_the_reason_changes_or_the_worker_links() {
    use skein_channel::Closed;
    const ONE_REFUSAL: u32 = 1;
    const TWO_REFUSALS: u32 = 2;
    let mut stand = socket_stand();
    for _ in 0_u8..2 {
        stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::RefusedPeer(256) });
    }
    assert_eq!(stand.worker.notes().len(), ONE_REFUSAL);
    stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::RefusedHere(2) });
    assert_eq!(stand.worker.notes().len(), TWO_REFUSALS);
    LinkPeer::new().open(&mut stand.worker);
    stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::RefusedPeer(256) });
    for reason in [crate::Refusal::Peer(256), crate::Refusal::Terms] {
        assert_eq!(stand.worker.notes().pop(), Some(refusal_note(reason)));
    }
    assert_eq!(
        stand.worker.notes().pop(),
        Some(crate::Note::Linked {
            worker: b"worker-a".as_slice().into(),
            address: Box::new(([127, 0, 0, 1], 7100).into()),
        })
    );
    assert_eq!(stand.worker.notes().pop(), Some(refusal_note(crate::Refusal::Peer(256))));
    assert!(stand.worker.notes().is_empty(), "three refusals with Linked between the second and third");
}

fn refusal_note(reason: crate::Refusal) -> crate::Note {
    crate::Note::Refused {
        worker: b"worker-a".as_slice().into(),
        address: Box::new(([127, 0, 0, 1], 7100).into()),
        reason,
    }
}
