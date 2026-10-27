//! Pending provider sign-in against the shipped binary and an independent local issuer.
use super::{Process, ctl};
use skein_fake_peers::{Transport, oauth};
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{fs, path::Path, sync::LazyLock};

const STOPS_WITHIN: Duration = Duration::from_secs(10);

pub(super) static ACCESS: LazyLock<Box<[u8]>> = LazyLock::new(|| {
    use skein_lib::{Writer, base64};
    let mut token = Vec::new();
    for json in [br#"{"alg":"RS256","typ":"JWT"}"#.as_slice(), br#"{"sub":"process-person"}"#] {
        let length = base64::encoded_len(base64::Alphabet::Url, u32::try_from(json.len()).expect("JWT part"))
            .expect("encoded JWT part");
        let mut part = Writer::new(usize::try_from(length).expect("JWT part length"));
        base64::encode(base64::Alphabet::Url, json, &mut part).expect("JWT JSON as base64url");
        token.extend_from_slice(&part.finish());
        token.push(b'.');
    }
    // A fake signature, with a valid unpadded base64url length and zero tail bits.
    assert_ne!((4096 - token.len()) % 4, 1);
    token.resize(4096, b'A');
    token.into_boxed_slice()
});

const SECOND_ANSWER: &[u8] = b"More words from the plan.";
const CLOSING_ANSWER: &[u8] = b"The plan chat is now closed.";

/// Initialize the shipped document once; give its listeners and fake peers private addresses.
#[must_use]
pub fn configuration(directory: &Path) -> oauth::Peer {
    let initialized = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(super::configuration::flags(directory))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(directory)
        .output()
        .expect("initialize a fresh deployment");
    assert!(initialized.status.success(), "{}", String::from_utf8_lossy(&initialized.stderr));
    patch_configuration(directory)
}

/// Point an initialized config directory at the issuer and the existing TLS fake.
#[must_use]
pub fn patch_configuration(directory: &Path) -> oauth::Peer {
    super::own_listener_files(directory);
    let path = directory.join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("init document")).expect("JSON");
    let taken = [
        "/application/clients/listener/socket",
        "/application/webhooks/listener/socket",
        "/application/demo/llm/listen/socket",
        "/application/setup/0/mode/fake/listen/socket",
        "/application/setup/1/mode/owned/listen/socket",
    ]
    .map(|pointer| {
        value.pointer(pointer).expect("init listener").as_str().expect("socket string").parse().expect("socket")
    });
    let [listener] = super::reserve_listeners(&taken);
    let address = listener.local_addr().expect("issuer address");
    let origin = format!("http://{address}");
    let provider = &mut value["engine"]["llm"]["providers"][0];
    assert_eq!(provider["number"], 1);
    let sign_in = &mut provider["sources"]["sign_in"];
    let original = sign_in["issuer"].as_str().expect("shipped issuer").to_owned();
    for member in ["issuer", "authorization", "token", "revocation", "keys"] {
        sign_in[member] = sign_in[member].as_str().expect("shipped address").replace(&original, &origin).into();
    }
    let redirect = format!(
        "http://127.0.0.1:{}/{}",
        sign_in["redirect_port"].as_u64().expect("shipped redirect port"),
        sign_in["redirect_path"].as_str().expect("shipped callback path")
    );
    sign_in["transport"] = "plaintext".into();
    sign_in.as_object_mut().expect("registration").remove("server_name");
    sign_in.as_object_mut().expect("registration").remove("redirect_port");
    let registration = sign_in.clone();
    value["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket":"127.0.0.1:0"});
    value["engine"]["llm"]["endpoints"][0]["server_name"] = "skein.test".into();
    value["engine"]["llm"]["endpoints"][0]["trust"] = serde_json::json!(directory.join("root.der"));
    super::write_private(&path, &serde_json::to_vec(&value).expect("configuration JSON"));
    let agent_path = directory.join("agent.json");
    let mut agent: serde_json::Value = serde_json::from_slice(&fs::read(&agent_path).expect("agent")).expect("JSON");
    agent["endpoints"][0]["address"] = "127.0.0.1:0".into();
    agent["endpoints"][0]["server_name"] = "skein.test".into();
    agent["endpoints"][0]["trust_der"] = serde_json::json!(directory.join("root.der"));
    super::write_private(&agent_path, &serde_json::to_vec(&agent).expect("agent JSON"));
    super::write_private(&directory.join("root.der"), skein_tls_world::pki::ROOT);
    issuer(listener, &registration, redirect.as_bytes())
}

/// Recreate the fake issuer on its recorded address without changing the deployment.
#[must_use]
pub fn restart_issuer(directory: &Path) -> oauth::Peer {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("patched document")).expect("JSON");
    let registration = &value["engine"]["llm"]["providers"][0]["sources"]["sign_in"];
    let address = registration["issuer"]
        .as_str()
        .expect("patched issuer")
        .strip_prefix("http://")
        .expect("local plaintext issuer")
        .parse::<std::net::SocketAddr>()
        .expect("issuer socket");
    // The fixture permits a varying callback port; no second sign-in is asked.
    let redirect = format!("http://127.0.0.1:0/{}", registration["redirect_path"].as_str().expect("callback path"));
    let listener = std::net::TcpListener::bind(address).expect("restart issuer listener");
    issuer(listener, registration, redirect.as_bytes())
}

pub(super) fn issuer(
    listener: std::net::TcpListener,
    registration: &serde_json::Value,
    redirect: &[u8],
) -> oauth::Peer {
    use std::os::fd::{AsRawFd, IntoRawFd};
    let address = listener.local_addr().expect("issuer address");
    let bytes =
        |member: &str| -> Box<[u8]> { registration[member].as_str().expect("registration member").as_bytes().into() };
    let profile = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small);
    let limits = skein_fake_oauth::Limits {
        document: profile.driver.client.document,
        uri_bytes: 1024,
        request_bytes: 4096,
        codes: 1,
        rotations: 2,
        carried: 5,
        // Code grant, a move's refresh, and a spare successful rotation.
        plans: 3,
    };
    let mut peer = oauth::Peer::new(
        address,
        Transport::Plaintext,
        skein_fake_peers::Limits {
            io: profile.io,
            connections: 2,
            queue: 64,
            plaintext: 131_072,
            ciphertext: 131_072,
            observations: 64,
            observation_bytes: 4096,
        },
        skein_fake_oauth::Config {
            authorization_url: bytes("authorization"),
            token_endpoint: bytes("token"),
            client_id: bytes("client"),
            client_secret: None,
            redirect_uri: redirect.into(),
            refresh_token: b"fake-refresh".as_slice().into(),
            terms: Some(skein_fake_oauth::Terms {
                issued: Box::new([b"issued-client".as_slice().into()]),
                omit_issued: false,
                varying_port: true,
                revocation_endpoint: Some(bytes("revocation")),
                openid: Some(skein_fake_oauth::OpenId {
                    issuer: bytes("issuer"),
                    key_set_url: bytes("keys"),
                    subject: b"process-person".as_slice().into(),
                    lifetime: Duration::from_secs(300),
                    header_kid: true,
                    fault: None,
                }),
            }),
        },
        limits,
        skein_http::server::Limits {
            response: oauth::least_response(&limits).expect("issuer response"),
            body: u64::from(limits.request_bytes),
            ..profile.driver.server
        },
    )
    .expect("local issuer");
    let scopes = registration["scopes"]
        .as_array()
        .expect("shipped scopes")
        .iter()
        .map(|scope| scope.as_str().expect("scope"))
        .collect::<Vec<_>>()
        .join(" ");
    peer.queue(skein_fake_oauth::Plan {
        status: 200,
        body: skein_fake_oauth::Body::Token(skein_oauth::TokenResponse {
            access_token: ACCESS.as_ref().into(),
            refresh_token: Some(b"fake-refresh".as_slice().into()),
            expires_in: 300,
            scope: Some(scopes.into_bytes().into()),
            id_token: None,
            earliest_refresh_at: None,
        }),
        delay: Duration::ZERO,
        retry_after: Duration::ZERO,
    })
    .expect("one fake grant");
    peer.adopt_listener(skein_io::kernel::Fd::new(listener.as_raw_fd()), address).expect("held issuer listener");
    let _ = listener.into_raw_fd();
    peer
}

pub(super) fn start(directory: &Path) -> skein_world::end_to_end::Binary {
    use skein_world::end_to_end::{Binary, Command, Mode};
    // A fresh init's demo forge is prepared by box before it starts serve.
    Binary::start(
        Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("box".into()).chain(super::configuration::flags(directory)).collect(),
            directory: directory.into(),
            environment: Vec::new(),
        },
        Mode::Pipes,
        65_536,
    )
    .expect("box starts the initialized deployment")
}

struct Judge {
    sent: Option<Time>,
    killed: bool,
    deadline: Time,
    settled: bool,
}
impl Referee<Process> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        if self.sent.is_none() && now >= self.deadline {
            let Process::Calls(calls) = &mut procs[1] else { panic!("ctl calls") };
            for ended in calls.ends().iter().chain(calls.pending()) {
                eprintln!(
                    "setup call {:?}: stdout {}; stderr {}",
                    ended.exit,
                    String::from_utf8_lossy(&ended.stdout),
                    String::from_utf8_lossy(&ended.stderr)
                );
            }
            calls.cancel();
            let Process::Binary(binary) = &mut procs[0] else { panic!("temper") };
            eprintln!("serve setup stderr: {}", String::from_utf8_lossy(binary.stderr()));
            binary.signal(Signal::Kill);
            self.sent = Some(now);
            self.killed = true;
            self.deadline = now.saturating_add(STOPS_WITHIN);
        }
        let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
        if self.sent.is_none()
            && let [signed, connected] = calls.ends()
        {
            assert_eq!(signed.exit, Some(Exit::Code(0)), "sign-in answered");
            assert_eq!(
                connected.exit,
                Some(Exit::Code(0)),
                "connect answered: {}",
                String::from_utf8_lossy(&connected.stderr)
            );
            let answer = ctl::json(connected);
            let address = answer["outcome"]["done"]["owner"]["account_sign_in_started"]["address"]
                .as_str()
                .expect("sign-in address");
            assert!(address.starts_with("http://127.0.0.1:"), "local authorization address");
            // Nobody visits this address. Keep the issuer alive until temper exits.
            let Process::Binary(binary) = &mut procs[0] else { panic!("temper") };
            binary.signal(Signal::Terminate);
            self.sent = Some(now);
            self.deadline = now.saturating_add(STOPS_WITHIN);
        }
        let Process::Binary(binary) = &mut procs[0] else { panic!("temper") };
        if binary.exit_status().is_some() {
            assert!(self.killed || now <= self.deadline, "SIGTERM exceeded {STOPS_WITHIN:?}");
            let Process::Issuer(issuer) = &mut procs[2] else { panic!("issuer") };
            issuer.shutdown();
        } else if self.sent.is_some() && now >= self.deadline && !self.killed {
            // Reap our owned child even in the deliberately broken regression run.
            binary.signal(Signal::Kill);
            self.killed = true;
            self.deadline = now.saturating_add(STOPS_WITHIN);
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        let Process::Binary(binary) = &procs[0] else { panic!("temper") };
        assert!(
            binary.exit_status().is_none() || self.sent.is_some(),
            "serve exited early: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        self.settled = self.sent.is_some() && procs.iter().all(Host::is_empty);
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "sign-in stop did not settle".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}

pub fn run(directory: &Path, issuer: oauth::Peer) {
    let clock = Clock::new();
    let mut binary = start(directory);
    let streams = binary.take_streams().expect("serve pipes");
    let mut fds = streams.descriptors();
    let root = skein_shell::open_root(directory).expect("scratch root");
    fds.push(root);
    let mut world = real::World::new(Judge {
        sent: None,
        killed: false,
        deadline: clock.now().now.saturating_add(super::WITHIN),
        settled: false,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        Process::Calls(Box::new(ctl::Calls::new(
            root,
            vec![
                ctl::Step::Run(ctl::Call::sign_in()),
                ctl::Step::Run(ctl::Call::new(&["connect", "1", "--key", "connect-llm", "--json"])),
            ],
            Some(&streams),
            None,
        )))
    });
    world.spawn(|| Process::Issuer(Box::new(issuer)));
    let outcome = world.run(&clock, Duration::from_secs(90));
    let Process::Issuer(issuer) = &outcome.procs[2] else { panic!("issuer") };
    assert!(
        !issuer.observations().iter().any(|event| matches!(event, oauth::Observation::Authorize { .. })),
        "no authorization visit"
    );
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("temper") };
    assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "SIGTERM must stop within {STOPS_WITHIN:?}");
    assert!(outcome.procs.iter().all(Host::is_empty), "all sign-in io settled");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Observation {
    Waiting,
    Signing,
    Visited,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlanChat {
    Answered,
    Exceeded,
    Released,
}
struct AnswerJudge {
    plan: PlanChat,
    allowance: u64,
    chat_pages: usize,
    conversation_read: bool,
    observation: Observation,
    stopped: bool,
    settled: bool,
    deadline: Time,
}
impl Referee<Process> for AnswerJudge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        // Go and See must finish before authorization can complete the sign-in.
        const SIGNING_SEEN: u32 = 2;
        let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
        if self.observation == Observation::Waiting
            && let [_, me, _, pending, ..] = calls.ends()
        {
            let party = ctl::json(me)["party"]["number"].as_u64().expect("signed-in person");
            let pending = ctl::json(pending);
            let [signing] = pending["accounts"]["signing"].as_array().expect("signing list").as_slice() else {
                panic!("one sign-in under way: {pending}")
            };
            assert_eq!(signing["provider"], 1);
            assert_eq!(signing["holder"]["party"], party);
            let signing = format!(
                "Sign-in in progress: source {} · LLM provider 1 · Person {party}",
                signing["source"].as_u64().expect("signing source")
            );
            let me = ctl::json(me);
            assert_eq!(me["party"]["name"]["cut"], false, "person's full name");
            let name = me["party"]["name"]["bytes"].as_str().expect("person's name");
            let link = calls.link().to_vec();
            let Process::Person(person) = &mut procs[5] else { panic!("native observer") };
            person.observe(&link, super::person::signed_in_account(signing.as_bytes(), name.as_bytes()));
            self.observation = Observation::Signing;
        }
        let Process::Person(person) = &procs[5] else { panic!("native observer") };
        assert!(person.failed.is_none(), "{:?}", person.failed);
        if self.observation == Observation::Signing && person.steps_seen() >= SIGNING_SEEN {
            let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
            let connected = &calls.ends()[2];
            let address = authorization_address(connected).expect("connect prints the address").to_vec();
            let Process::Visitor(visitor) = &mut procs[3] else { panic!("browser") };
            visitor.hand(&address);
            self.observation = Observation::Visited;
        }
        let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
        for (index, ended) in calls.ends().iter().enumerate() {
            assert_eq!(
                ended.exit,
                Some(Exit::Code(0)),
                "ctl call {index}: {}; output {}",
                String::from_utf8_lossy(&ended.stderr),
                String::from_utf8_lossy(&ended.stdout)
            );
        }
        if self.plan == PlanChat::Exceeded && calls.ends().len() == FIRST_REPLY_CALLS {
            let revision = exceeded_hold(&calls.ends()[8], self.allowance);
            assert_eq!(model_requests(procs), 1, "one model request before release");
            let Process::Calls(calls) = &mut procs[1] else { panic!("ctl calls") };
            calls.push(ctl::Step::Run(ctl::Call::new(&["release", "1", "--hold", &revision.to_string(), "--json"])));
            calls.push(ctl::Step::Run(ctl::Call::new(&["wait", "task", "1", "--until", "held", "--json"])));
            calls.push(ctl::Step::Run(ctl::Call::new(&["task", "1", "--json"])));
            calls.release();
            self.plan = PlanChat::Released;
        }
        let Process::Calls(calls) = &mut procs[1] else { panic!("ctl calls") };
        if self.plan == PlanChat::Answered
            && !self.conversation_read
            && calls.ends().len() == CONVERSATION_CALL + self.chat_pages + 1
        {
            let chat = ctl::json(calls.ends().last().expect("conversation page"));
            self.chat_pages += 1;
            if chat["activity"]["before"].is_null() {
                // ctl reads a chat through its watch, which an ended chat ends: read
                // the whole conversation, then Close.
                calls.push(ctl::Step::Run(ctl::Call::new(&["close", "1", "--key", "close-plan-chat", "--json"])));
                calls.push(ctl::Step::Run(ctl::Call::new(&["wait", "task", "1", "--until", "ended", "--json"])));
                calls.release();
                self.conversation_read = true;
            } else {
                let before = chat["activity"]["before"]["value"].as_str().expect("older conversation cursor");
                calls.push(ctl::Step::Run(ctl::Call::new(&["chat", "1", "--before", before, "--json"])));
            }
        }
        let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
        let Process::Person(person) = &procs[5] else { panic!("native observer") };
        let ready = if self.plan == PlanChat::Answered {
            self.conversation_read && calls.ends().len() == CONVERSATION_CALL + self.chat_pages + 2
        } else {
            calls.ends().len() == FIRST_REPLY_CALLS + 3
        };
        if !self.stopped && ready && person.finished {
            let Process::Binary(binary) = &mut procs[0] else { panic!("temper") };
            binary.signal(Signal::Terminate);
            for proc in procs {
                match proc {
                    Process::Peer(peer) => peer.shutdown(),
                    Process::Issuer(issuer) => issuer.shutdown(),
                    Process::Binary(_)
                    | Process::Calls(_)
                    | Process::Person(_)
                    | Process::Stdout(_)
                    | Process::Visitor(_) => {}
                }
            }
            self.stopped = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        let Process::Binary(binary) = &procs[0] else { panic!("temper") };
        assert!(self.stopped || binary.exit_status().is_none(), "serve: {}", String::from_utf8_lossy(binary.stderr()));
        self.settled = self.stopped && procs.iter().all(Host::is_empty);
        let Process::Person(person) = &procs[5] else { panic!("native observer") };
        let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
        assert!(
            now < self.deadline || self.settled,
            "signed-in chat did not settle: observer step {}, finished {}, observation {:?}, calls {}",
            person.steps_seen(),
            person.finished,
            self.observation,
            calls.ends().len()
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "signed-in chat deadline".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
const FIRST_REPLY_CALLS: usize = 9;
const CONVERSATION_CALL: usize = FIRST_REPLY_CALLS + 3;

fn reasoning_allowance(directory: &Path) -> u64 {
    let agent: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("agent.json")).expect("initialized agent")).expect("JSON");
    agent["endpoints"]
        .as_array()
        .expect("endpoints")
        .iter()
        .flat_map(|endpoint| endpoint["models"].as_array().expect("models"))
        .find(|model| model["name"] == "gpt-6-luna")
        .expect("plan model")["reasoning_item"]
        .as_u64()
        .expect("reasoning allowance")
}

fn answer_peer(allowance: u64, exceeded: bool) -> super::llm::Listening {
    use skein_fake_llm_domain::api::{Finish, Line, Turn};
    // The chat is ended by its requester (domain/agent.md, 4.6): its answer is the
    // turn's text, and the finish only ends the turn.
    let item = usize::try_from(allowance)
        .expect("item length")
        .checked_add(usize::from(exceeded))
        .expect("allowance plus one");
    let bytes = serde_json::to_vec(&serde_json::json!({
        "type": "reasoning", "id": "reasoning-1", "summary": [],
        "encrypted_content": "a".repeat(item)
    }))
    .expect("native reasoning item");
    let reasoning = Line::Opaque { bytes: bytes.into() };
    let turns = if exceeded {
        vec![Turn { lines: Box::new([reasoning]), finish: Finish::Stop, tokens: 1 }]
    } else {
        vec![
            Turn { lines: Box::new([Line::Text { text: super::ANSWER.into() }]), finish: Finish::Stop, tokens: 1 },
            Turn {
                lines: Box::new([reasoning, Line::Text { text: SECOND_ANSWER.into() }]),
                finish: Finish::Stop,
                tokens: 1,
            },
            Turn { lines: Box::new([Line::Text { text: CLOSING_ANSWER.into() }]), finish: Finish::Stop, tokens: 1 },
        ]
    };
    plan_peer(Box::new([skein_fake_llm_domain::api::Script { cue: Box::default(), turns: turns.into_boxed_slice() }]))
}

/// The existing TLS peer, with the plan bearer and room for the largest scripted item.
#[must_use]
pub(super) fn plan_peer(scripts: Box<[skein_fake_llm_domain::api::Script]>) -> super::llm::Listening {
    use skein_fake_llm_domain::api::Line;
    let largest = scripts
        .iter()
        .flat_map(|script| &script.turns)
        .flat_map(|turn| &turn.lines)
        .map(|line| match line {
            Line::Opaque { bytes } => bytes.len(),
            Line::Text { text } => text.len(),
            Line::Call { name, arguments } => name.len().checked_add(arguments.len()).expect("call bytes"),
            Line::Pieces { pieces } => pieces.iter().map(|piece| piece.text.len()).max().unwrap_or(0),
            Line::Fail { report, .. } => report.code.len().max(report.request.len()).max(report.field.len()),
        })
        .max()
        .unwrap_or(0);
    // Retain the ordinary peer's room for script slots and wire framing.
    let strings = 16_384_u32.checked_add(u32::try_from(largest).expect("scripted item length")).expect("peer strings");
    let mut recipe = temper_real_loop_world::llm::scripted_sized(scripts, strings);
    // The sign-in peer alone takes a real-length plan bearer: 4096 bytes plus
    // the request line and headers exceed 4096; the next power of two is 8192.
    recipe.protocol_limits.http.head = 8192;
    recipe.credential = skein_llm::Credential::bearer(ACCESS.as_ref().into());
    super::llm::Listening::new(
        skein_fake_peers::llm::Peer::new(
            (std::net::Ipv4Addr::LOCALHOST, 0).into(),
            Transport::Tls,
            recipe.limits,
            recipe.config,
            recipe.credential,
            recipe.protocol_limits,
            recipe.domain,
            recipe.domain_limits,
        )
        .expect("fake LLM peer"),
    )
}

pub fn answered(directory: &Path, issuer: oauth::Peer) {
    plan_chat(directory, issuer, false);
}

pub fn exceeded(directory: &Path, issuer: oauth::Peer) {
    plan_chat(directory, issuer, true);
}

fn model_requests(procs: &[Process]) -> usize {
    super::forge_land::peer_seen(procs, 4).requests.len()
}

fn model_answers(procs: &[Process]) -> Vec<Result<(), skein_fake_llm_domain::api::Error>> {
    super::forge_land::peer_seen(procs, 4).answers
}

fn exceeded_hold(ended: &super::Ended, allowance: u64) -> u64 {
    let task = ctl::json(ended);
    let held = &task["chip"]["details"]["owner"]["phase"]["held"];
    assert_eq!(held["reason"], serde_json::json!({"exceeded":{"bound":"reasoning","value":allowance}}), "{task}");
    held["revision"].as_u64().expect("hold revision")
}

fn plan_chat(directory: &Path, issuer: oauth::Peer, exceeded: bool) {
    let allowance = reasoning_allowance(directory);
    let peer = answer_peer(allowance, exceeded);
    peer.configure(directory);
    let clock = Clock::new();
    let mut binary = start(directory);
    let streams = binary.take_streams().expect("serve pipes");
    let mut fds = streams.descriptors();
    let root = skein_shell::open_root(directory).expect("scratch root");
    fds.push(root);
    let mut world = real::World::new(AnswerJudge {
        plan: if exceeded { PlanChat::Exceeded } else { PlanChat::Answered },
        allowance,
        chat_pages: 0,
        conversation_read: false,
        observation: Observation::Waiting,
        stopped: false,
        settled: false,
        deadline: clock.now().now.saturating_add(super::WITHIN),
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        let mut calls = ctl::Calls::new(
            root,
            vec![
                ctl::Step::Run(ctl::Call::sign_in()),
                ctl::Step::Run(ctl::Call::new(&["me", "--json"])),
                ctl::Step::Run(ctl::Call::new(&["connect", "1", "--key", "connect-llm"])),
                ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])),
                ctl::Step::Run(ctl::Call::new(&["wait", "accounts", "--until", "settled", "--json"])),
                ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])),
                ctl::Step::Run(ctl::Call::new(&[
                    "chat",
                    "--project",
                    "1",
                    "--model",
                    "gpt-6-luna",
                    "--key",
                    "chat-on-plan",
                    "Hello",
                    "--json",
                ])),
                ctl::Step::Run(ctl::Call::new(&[
                    "wait",
                    "task",
                    "1",
                    "--until",
                    if exceeded { "held" } else { "replied" },
                    "--json",
                ])),
                ctl::Step::Run(ctl::Call::new(&["task", "1", "--json"])),
            ],
            Some(&streams),
            None,
        );
        // The referee follows conversation cursors, or authorizes the held run's release.
        calls.hold();
        if !exceeded {
            for arguments in [
                &["message", "1", "More", "--key", "more-on-plan", "--json"][..],
                &["wait", "task", "1", "--until", "replied", "--json"],
                &["task", "1", "--json"],
                &["chat", "1", "--json"],
            ] {
                calls.push(ctl::Step::Run(ctl::Call::new(arguments)));
            }
        }
        Process::Calls(Box::new(calls))
    });
    let face = super::forge_land::Face::Fake { issuer, peer }.spawn(&mut world);
    assert_eq!((face.issuer, face.visitor, face.peer), (2, 3, 4));
    let observer_root = skein_shell::open_root(directory).expect("observer's scratch root");
    world.spawn_with_fds(vec![observer_root], || {
        Process::Person(Box::new(super::person::Person::waiting_for_observation(observer_root)))
    });
    let outcome = world.run(&clock, super::WITHIN);
    let Process::Calls(calls) = &outcome.procs[1] else { panic!("ctl calls") };
    let [_, me, _, _, _, ready, _, _, first, ..] = calls.ends() else { panic!("signed-in chat calls") };
    assert_plan_account(me, ready);
    if exceeded {
        let first = exceeded_hold(first, allowance);
        let second = exceeded_hold(&calls.ends()[11], allowance);
        assert!(second > first, "a fresh hold after the authorized release");
        assert_eq!(model_requests(&outcome.procs), 2, "one additional model request after release");
    } else {
        // Close cancels with the chat template's grace: one final model turn to finish.
        // A provider's context-too-long rejection would start Smith's compaction
        // (smith docs/design/domain/session.md, 8.2), a fourth request.
        assert_eq!(model_requests(&outcome.procs), 3, "two replies and the closing grace turn, without retry");
        assert_eq!(model_answers(&outcome.procs), [Ok(()); 3], "each request answered, the closing turn too");
    }
    let seen = super::forge_land::seen(&outcome.procs, &face);
    assert!(seen.schemes.contains(&Some(skein_llm::Scheme::Bearer)), "the peer observed a bearer scheme");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("temper") };
    assert_eq!(binary.exit_status(), Some(Exit::Code(0)));
    assert!(outcome.procs.iter().all(Host::is_empty), "all signed-in chat io settled");
    if !exceeded {
        assert_lasting_chat(calls.ends(), allowance);
    }
}

fn assert_lasting_chat(ends: &[super::Ended], allowance: u64) {
    for index in [8, 11] {
        let task = ctl::json(&ends[index]);
        // A parked hosted chat keeps its run; the host reports it waiting.
        assert_eq!(task["chip"]["phase"], "running", "parked chat: {task}");
        let owner = &task["chip"]["details"]["owner"];
        assert!(owner["phase"]["active"]["running"].is_object(), "chat's run remains live");
        assert_eq!(owner["details"]["observation"]["work"], "waiting", "chat parked after reply");
        assert!(task["hold"].is_null(), "parked without a hold");
    }
    let ended = ctl::json(ends.last().expect("ended wait"));
    assert_eq!(ended["met"], true, "ended condition met");
    assert_eq!(ended["task"]["chip"]["phase"], "ended", "Close ended the parked chat: {ended}");
    let cancelled = &ended["task"]["chip"]["details"]["owner"]["details"]["ending"]["cancelled"];
    let person = ctl::json(&ends[1])["party"]["number"].as_u64().expect("signed-in person");
    assert_eq!(cancelled["by"]["person"], person, "chat closed by its person: {ended}");
    assert!(cancelled["result"].is_null(), "the closing turn's finish is no task result: {ended}");
    let pages: Vec<_> = ends[CONVERSATION_CALL..ends.len() - 2].iter().map(ctl::json).collect();
    let mut rows = Vec::new();
    for page in &pages {
        assert_eq!(page["activity"]["cut"], false, "whole conversation page");
        rows.extend(page["activity"]["rows"].as_array().expect("conversation rows"));
    }
    assert!(pages.last().expect("conversation read")["activity"]["before"].is_null(), "all conversation rows read");
    rows.sort_by_key(|row| row["position"].as_u64().expect("activity position"));
    for pair in rows.windows(2) {
        assert_ne!(pair[0]["position"], pair[1]["position"], "each conversation row read once");
    }
    // ctl reads a chat through its watch, which an ended chat ends: the person reads
    // the conversation before Close, and the closing answer is the peer's to show.
    let turns: Vec<_> = rows
        .iter()
        .filter(|row| row["event"]["owner"]["source"]["line"]["turn"].is_object())
        .map(|row| {
            let turn = &row["event"]["owner"]["decoded"]["turn"]["decoded"];
            assert!(turn.is_object(), "turn loaded and decoded whole, without a too-large reference: {row}");
            assert_eq!(turn["cut"], false, "whole committed turn");
            turn
        })
        .collect();
    assert_eq!(turns.len(), 2, "both replies committed");
    let blocks: Vec<_> = turns
        .iter()
        .flat_map(|turn| turn["messages"].as_array().expect("turn messages"))
        .filter(|message| message["speaker"] == "assistant")
        .flat_map(|message| message["blocks"].as_array().expect("assistant blocks"))
        .collect();
    let replies: Vec<_> = blocks.iter().filter_map(|block| block["text"]["text"].as_str()).collect();
    assert_eq!(replies.len(), 2, "both replies in the conversation");
    for answer in [super::ANSWER, SECOND_ANSWER] {
        let answer = std::str::from_utf8(answer).expect("answer text");
        assert_eq!(replies.iter().filter(|text| **text == answer).count(), 1, "each answer exactly once");
    }
    let reasoning: Vec<_> = blocks.iter().filter_map(|block| block["opaque"]["size"].as_u64()).collect();
    let [size] = reasoning.as_slice() else { panic!("one reasoning item in the committed conversation") };
    assert!(*size >= allowance && *size > 65_536, "native item and its turn pass 64 KiB: {size}");
}

pub fn assert_plan_account(me: &super::Ended, ready: &super::Ended) {
    let person = ctl::json(me)["party"]["number"].as_u64().expect("person number");
    let ready = ctl::json(ready);
    let account = ready["accounts"]["party"]
        .as_array()
        .expect("person's accounts")
        .iter()
        .find(|row| row["provider"] == 1)
        .expect("OpenAI account");
    assert_eq!(account["plan"], true);
    assert_eq!(account["holder"]["party"], person);
    assert!(
        account["sources"].as_array().expect("sources").iter().any(|source| {
            source["kind"] == "sign_in" && source["usable"] == true && source["state"]["kind"] == "granted"
        }),
        "usable signed-in source"
    );
}

/// The authorization address printed by connect, handed to the fake browser by either referee.
pub(super) fn authorization_address(connected: &super::Ended) -> Option<&[u8]> {
    connected.stdout.split(|byte| *byte == b'\n').find(|line| line.starts_with(b"http://127.0.0.1:"))
}
