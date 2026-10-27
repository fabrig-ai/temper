//! Bounded problems and measured stderr lines (protocol/configuration.md, 2).
use crate::limits;
use alloc::boxed::Box;
use jig_protocol_config as config;
use skein_lib::{Decimal, List, Writer};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum What {
    Jig(config::What),
    Standalone,
    EngineAlone,
    WorkerAlone,
    Loopback,
    Connectors,
    Absolute,
    OldAgentCommand,
    NoWebhooks,
    Profile,
    Override(limits::Refused),
    Against {
        other: &'static [u8],
    },
    Past {
        bytes: u64,
        bound: u64,
    },
    Short {
        bytes: u64,
        of: &'static [u8],
        needs: u64,
    },
    Within {
        bytes: u64,
        limit: &'static [u8],
        bound: u64,
    },
    StorePage {
        transcript: u32,
        frame: u32,
        page: u32,
        ceiling: u32,
    },
    StoreMap(jig_protocol_store::sizes::SizeRefusal),
    /// An explicit call bound below the local agent's call input
    /// (protocol/configuration.md, 7): `bytes` the configured bound, also
    /// the wire answer's; `input` the agent's `tool_payload`; `ask` its
    /// channel's `host_ask_input`; `reply` its `host_reply_text`, smith's
    /// `host_reply_bytes`.
    CallInput {
        bytes: u64,
        input: u64,
        ask: u64,
        reply: u64,
    },
    Agent,
    Admission {
        why: jig_domain_fleet::AgentShortfall,
        offered: u64,
        required: u64,
    },
    Adapter,
    Redirect(super::Listener),
    NoHub,
    Credentials,
    Escalation,
    Provider,
    Handover,
    MissingNull,
    InlineSeedPolicy,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problem {
    pub path: config::Path,
    pub what: What,
    pub file: Option<Box<[u8]>>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problems {
    pub problems: List<Problem>,
    pub past: u32,
}
impl Problems {
    #[must_use]
    pub fn empty() -> Self {
        Self { problems: List::with_capacity(config::PROBLEMS), past: 0 }
    }
    pub fn push(&mut self, path: config::Path, what: What) {
        self.push_file(path, what, None);
    }
    pub(crate) fn push_file(&mut self, path: config::Path, what: What, file: Option<Box<[u8]>>) {
        if self.problems.push(Problem { path, what, file }).is_err() {
            self.past = self.past.saturating_add(1);
        }
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.problems.is_empty() && self.past == 0
    }
    #[must_use]
    pub fn jig(found: config::Problems, prefix: &config::Path) -> Self {
        let mut result = Self::empty();
        result.append(found, prefix);
        result
    }
    pub(crate) fn append(&mut self, found: config::Problems, prefix: &config::Path) {
        for problem in &found.problems {
            let mut path = prefix.clone();
            for segment in &problem.path.segments {
                path = path.child(segment.clone());
            }
            self.push(path, What::Jig(problem.what));
        }
        self.past = self.past.saturating_add(found.past);
    }
}
fn put(writer: &mut Writer, length: &mut usize, measuring: bool, bytes: &[u8]) {
    *length = length.checked_add(bytes.len()).expect("bounded diagnostic fits usize");
    if !measuring {
        writer.put(bytes).expect("diagnostic was measured");
    }
}
fn words(what: What) -> &'static [u8] {
    match what {
        What::InlineSeedPolicy => b"name a policy, policy: 1; change it with temper ctl settings apply",
        What::MissingNull => b"missing: null for none",
        What::Standalone => b"standalone takes one worker, joined to the engine by the in-process name \"workers\"",
        What::EngineAlone => b"an engine alone takes no local worker, one plaintext loopback listener and workers with distinct digest paths",
        What::WorkerAlone => b"a worker alone takes one worker dialling a plaintext loopback socket, and only application.workers and application.limits",
        What::Loopback => b"must be a socket address on loopback",
        What::Connectors => b"must be [1], the forge connector",
        What::Absolute => b"must be an absolute path",
        What::OldAgentCommand => b"\"agent\" is now \"internal\", \"agent\": write [\"internal\", \"agent\", PATH], PATH agent.configuration's",
        What::NoWebhooks => b"needs application.webhooks",
        What::Profile => b"the only profile is \"small\"",
        What::Override(limits::Refused::Unknown) => b"no limit of temper's has this name",
        What::Override(limits::Refused::Range) => b"outside its type",
        What::Against { .. } => b"must agree with ",
        What::Past { .. }
        | What::Within { .. }
        | What::Short { .. }
        | What::CallInput { .. }
        | What::Admission { .. }
        | What::StorePage { .. }
        | What::StoreMap(_) => b"",
        What::Redirect(_) => b"fixed port conflicts with ",
        What::Adapter => b"program adapter not declared in the agent's programs",
        What::Agent => b"not in the agent's configuration under the same name",
        What::NoHub => b"the root has no hub, so the fleet must place every run on a worker",
        What::Credentials => b"must be from 1 to accounts.credentials, in engine.limits and in the worker's limits",
        What::Escalation => b"an owner's project must name an escalation role from 0 to 3",
        What::Handover => b"admin_token must be exactly 40 lowercase hexadecimal digits",
        What::Provider => b"the deployment's provider must not be 0, the forge's sign-in provider",
        What::Jig(what) => match what {
            config::What::PolicyEnvelope { .. }
            | config::What::AmendmentEnvelope { .. }
            | config::What::CodecCeiling { .. }
            | config::What::CodecArithmetic
            | config::What::StoreCodec => b"",
            config::What::Json => b"invalid JSON",
            config::What::Stored => b"settings live in the store; read them with temper ctl DIR settings show",
            config::What::Unknown => b"unknown member",
            config::What::Missing => b"missing",
            config::What::Duplicate => b"duplicate",
            config::What::Kind => b"wrong value kind",
            config::What::Bound => b"past its bound",
            config::What::Form | config::What::ProviderKey { .. } => b"invalid form",
            config::What::Unique => b"must be unique",
            config::What::Refers => b"unknown reference",
            config::What::Plaintext => b"plaintext requires loopback or in-process",
            config::What::Confidential => b"a confidential registration is refused: only a public one signs in",
            config::What::NoProvider => b"no provider offers this model",
            config::What::Digest => b"invalid digest",
            config::What::Reserve { .. } => b"reserve does not fit charter ",
            config::What::Budget { .. } => b"budget does not fit charter ",
            config::What::Grace { .. } => b"grace does not fit template ",
            config::What::Operations { .. } => b"operations do not fit compound requests: operations ",
            config::What::Message => b"message bounds do not agree",
            config::What::Silence => b"heartbeat does not fit silence bound",
            config::What::StopBound => b"stop bound does not fit grace",
            config::What::Memory { .. } => b"the worst case passes memory_bytes by ",
            config::What::Override => b"invalid override",
            config::What::Unreadable => b"not read",
            config::What::Relative => b"relative path must be non-empty with no .. component",
        },
    }
}
fn redirect_listener(listener: super::Listener, writer: &mut Writer, length: &mut usize, measuring: bool) {
    let name = match listener {
        super::Listener::Demo => b"application.demo.llm.listen".as_slice(),
        super::Listener::Clients => b"application.clients.listener".as_slice(),
        super::Listener::Webhooks => b"application.webhooks.listener".as_slice(),
        super::Listener::Owned(index) => {
            put(writer, length, measuring, b"application.setup[");
            put(writer, length, measuring, Decimal::of(u64::from(index)).as_bytes());
            b"].mode.owned.listen".as_slice()
        }
        super::Listener::Fake(index) => {
            put(writer, length, measuring, b"application.setup[");
            put(writer, length, measuring, Decimal::of(u64::from(index)).as_bytes());
            b"].mode.fake.listen".as_slice()
        }
    };
    put(writer, length, measuring, name);
}
fn admission(
    why: jig_domain_fleet::AgentShortfall,
    offered: u64,
    required: u64,
    writer: &mut Writer,
    length: &mut usize,
    measuring: bool,
) {
    use jig_domain_fleet::AgentShortfall;
    // Missing names carry no numerical claim, even if constructed by a caller.
    if let AgentShortfall::Model { .. } = why {
        put(writer, length, measuring, b"not in the agent's configuration under the same name");
        return;
    }
    let (before, between, after): (&[u8], &[u8], &[u8]) = match why {
        AgentShortfall::Model { .. } => unreachable!("name diagnostic above"),
        AgentShortfall::Window { .. } => (b"needs a context window of ", b" tokens; the agent offers ", b" tokens"),
        AgentShortfall::Output { .. } => (b"needs ", b" output tokens; the agent offers ", b" tokens"),
        AgentShortfall::Turns { .. } => (b"needs ", b" turns; the agent's profile.policy.max_turns allows ", b""),
        AgentShortfall::Spend => {
            (b"needs a spend allowance of ", b"; the agent's profile.policy.max_spend allows ", b"")
        }
        AgentShortfall::Time { .. } => {
            (b"needs ", b" ms of run time; the agent's profile.policy.max_time allows ", b" ms")
        }
        AgentShortfall::Waiting { .. } => {
            (b"needs ", b" ms of waiting; the agent's profile.policy.max_waiting allows ", b" ms")
        }
        AgentShortfall::Checks { .. } => {
            (b"needs checks within ", b" ms; the agent's profile.policy.check_timeout is ", b" ms")
        }
        AgentShortfall::Message => {
            (b"needs ", b"-byte messages (tasks.message_bytes); the agent offers ", b" (tool_payload less 2)")
        }
        AgentShortfall::Inbox => {
            (b"needs an inbox of ", b" (tasks.run_inbox); the agent's profile.policy.inbox allows ", b"")
        }
        AgentShortfall::Unacknowledged => (
            b"needs an unacknowledged-turn allowance of ",
            b" (hub.turns); the agent's profile.policy.unacknowledged allows ",
            b"",
        ),
    };
    put(writer, length, measuring, before);
    put(writer, length, measuring, Decimal::of(required).as_bytes());
    put(writer, length, measuring, between);
    put(writer, length, measuring, Decimal::of(offered).as_bytes());
    put(writer, length, measuring, after);
}
fn render_path(path: &config::Path, writer: &mut Writer, length: &mut usize, measuring: bool) {
    let mut member = false;
    for segment in &path.segments {
        match segment {
            config::Segment::Member(name) => {
                if member {
                    put(writer, length, measuring, b".");
                }
                put(writer, length, measuring, name);
                member = true;
            }
            config::Segment::Index(index) => {
                put(writer, length, measuring, b"[");
                put(writer, length, measuring, Decimal::of(u64::from(*index)).as_bytes());
                put(writer, length, measuring, b"]");
            }
        }
    }
}
fn codec_words(what: config::What, writer: &mut Writer, length: &mut usize, measuring: bool) {
    let problem = config::Problem { path: config::Path::root(), what };
    let line = problem.codec_message().expect("codec-envelope diagnostic variant");
    let words = line.strip_prefix(b": ").expect("root codec diagnostic prefix");
    put(writer, length, measuring, words);
}

fn store_page(frame: u32, ceiling: u32, writer: &mut Writer, length: &mut usize, measuring: bool) {
    put(writer, length, measuring, b"its turn's transcript frame, ");
    put(writer, length, measuring, Decimal::of(u64::from(frame)).as_bytes());
    put(writer, length, measuring, b" bytes, needs a store page past the profile's ceiling of ");
    put(writer, length, measuring, Decimal::of(u64::from(ceiling)).as_bytes());
    put(writer, length, measuring, b" bytes");
}

fn store_map(why: jig_protocol_store::sizes::SizeRefusal, writer: &mut Writer, length: &mut usize, measuring: bool) {
    use jig_protocol_store::sizes::SizeRefusal;
    if why == SizeRefusal::Ceiling {
        put(writer, length, measuring, b"the store's map sizing is past its codec or arithmetic bounds");
        return;
    }
    let (name, needs, has, unit): (&[u8], u64, u64, &[u8]) = match why {
        SizeRefusal::Key { needs, has } => (b"key", u64::from(needs), u64::from(has), b"bytes"),
        SizeRefusal::Value { needs, has } => (b"value", u64::from(needs), u64::from(has), b"bytes"),
        SizeRefusal::Ops { needs, has } => (b"commit", u64::from(needs), u64::from(has), b"operations"),
        SizeRefusal::Commit { needs, has } => (b"commit", u64::from(needs), u64::from(has), b"bytes"),
        SizeRefusal::Page { needs, has } => (b"page", u64::from(needs), u64::from(has), b"bytes"),
        SizeRefusal::Queue { needs, has } => (b"queue", u64::from(needs), u64::from(has), b"commits"),
        SizeRefusal::QueuedBytes { needs, has } => (b"queue", needs, has, b"bytes"),
        SizeRefusal::Whole { needs, has } => (b"whole-file read", u64::from(needs), u64::from(has), b"bytes"),
        SizeRefusal::Ceiling => unreachable!("arithmetic diagnostic above"),
    };
    put(writer, length, measuring, b"the store's map ");
    put(writer, length, measuring, name);
    put(writer, length, measuring, b", ");
    put(writer, length, measuring, Decimal::of(needs).as_bytes());
    put(writer, length, measuring, b" ");
    put(writer, length, measuring, unit);
    put(writer, length, measuring, b", past its bound of ");
    put(writer, length, measuring, Decimal::of(has).as_bytes());
    put(writer, length, measuring, b" ");
    put(writer, length, measuring, unit);
}
#[expect(clippy::too_many_lines, reason = "one exhaustive diagnostic vocabulary shared by both rendering passes")]
fn render(problems: &Problems, writer: &mut Writer, measuring: bool) -> usize {
    let mut length = 0_usize;
    for problem in &problems.problems {
        render_path(&problem.path, writer, &mut length, measuring);
        put(writer, &mut length, measuring, b": ");
        if let Some(file) = &problem.file {
            put(writer, &mut length, measuring, file);
            put(writer, &mut length, measuring, b": ");
        }
        put(writer, &mut length, measuring, words(problem.what));
        match problem.what {
            What::Jig(
                what @ (config::What::PolicyEnvelope { .. }
                | config::What::AmendmentEnvelope { .. }
                | config::What::CodecCeiling { .. }
                | config::What::CodecArithmetic
                | config::What::StoreCodec),
            ) => codec_words(what, writer, &mut length, measuring),
            What::CallInput { bytes, input, ask, reply } => {
                put(writer, &mut length, measuring, Decimal::of(bytes).as_bytes());
                put(
                    writer,
                    &mut length,
                    measuring,
                    b" bytes, short of the agent's call input (profile.declared.tool_payload), ",
                );
                put(writer, &mut length, measuring, Decimal::of(input).as_bytes());
                put(writer, &mut length, measuring, b" bytes; raise it to at least ");
                put(writer, &mut length, measuring, Decimal::of(input).as_bytes());
                put(writer, &mut length, measuring, b" bytes, or omit it to follow the agent's call input\n");
                render_path(&problem.path, writer, &mut length, measuring);
                put(writer, &mut length, measuring, b": call input ");
                put(writer, &mut length, measuring, Decimal::of(ask).as_bytes());
                put(writer, &mut length, measuring, b" bytes; wire answer ");
                put(writer, &mut length, measuring, Decimal::of(bytes).as_bytes());
                put(writer, &mut length, measuring, b" bytes; smith host_reply_bytes ");
                put(writer, &mut length, measuring, Decimal::of(reply).as_bytes());
                put(writer, &mut length, measuring, b" bytes");
            }
            What::Admission { why, offered, required } => {
                admission(why, offered, required, writer, &mut length, measuring);
            }
            What::StorePage { frame, ceiling, .. } => {
                store_page(frame, ceiling, writer, &mut length, measuring);
            }
            What::StoreMap(why) => store_map(why, writer, &mut length, measuring),
            What::Redirect(listener) => redirect_listener(listener, writer, &mut length, measuring),
            What::Against { other } => put(writer, &mut length, measuring, other),
            What::Past { bytes, bound } => {
                put(writer, &mut length, measuring, Decimal::of(bytes).as_bytes());
                put(writer, &mut length, measuring, b" bytes, past its bound of ");
                put(writer, &mut length, measuring, Decimal::of(bound).as_bytes());
            }
            What::Short { bytes, of, needs } => {
                put(writer, &mut length, measuring, Decimal::of(bytes).as_bytes());
                put(writer, &mut length, measuring, b" bytes, short of ");
                put(writer, &mut length, measuring, of);
                put(writer, &mut length, measuring, b", ");
                put(writer, &mut length, measuring, Decimal::of(needs).as_bytes());
                put(writer, &mut length, measuring, b" bytes");
            }
            What::Within { bytes, limit, bound } => {
                put(writer, &mut length, measuring, Decimal::of(bytes).as_bytes());
                put(writer, &mut length, measuring, b" bytes, past ");
                put(writer, &mut length, measuring, limit);
                put(writer, &mut length, measuring, b" of ");
                put(writer, &mut length, measuring, Decimal::of(bound).as_bytes());
            }
            What::Jig(config::What::Memory { over }) => {
                put(writer, &mut length, measuring, Decimal::of(over).as_bytes());
                put(writer, &mut length, measuring, b" bytes");
            }
            What::Jig(config::What::Operations { operations, delegates, goals }) => {
                put(writer, &mut length, measuring, Decimal::of(u64::from(operations)).as_bytes());
                put(writer, &mut length, measuring, b", delegates ");
                put(writer, &mut length, measuring, Decimal::of(u64::from(delegates)).as_bytes());
                put(writer, &mut length, measuring, b", goals ");
                put(writer, &mut length, measuring, Decimal::of(u64::from(goals)).as_bytes());
            }
            What::Jig(
                config::What::Reserve { charter }
                | config::What::Budget { charter }
                | config::What::Grace { template: charter },
            ) => {
                put(writer, &mut length, measuring, Decimal::of(u64::from(charter)).as_bytes());
            }
            What::Jig(
                config::What::Json
                | config::What::Stored
                | config::What::Unknown
                | config::What::Missing
                | config::What::Duplicate
                | config::What::Kind
                | config::What::Bound
                | config::What::Form
                | config::What::Unique
                | config::What::Refers
                | config::What::Plaintext
                | config::What::Confidential
                | config::What::NoProvider
                | config::What::ProviderKey { .. }
                | config::What::Digest
                | config::What::Message
                | config::What::Silence
                | config::What::StopBound
                | config::What::Override
                | config::What::Unreadable
                | config::What::Relative,
            )
            | What::EngineAlone
            | What::WorkerAlone
            | What::Standalone
            | What::Loopback
            | What::Connectors
            | What::Absolute
            | What::OldAgentCommand
            | What::NoWebhooks
            | What::Profile
            | What::Override(_)
            | What::Adapter
            | What::Agent
            | What::NoHub
            | What::Credentials
            | What::Escalation
            | What::Provider
            | What::MissingNull
            | What::InlineSeedPolicy
            | What::Handover => {}
        }
        put(writer, &mut length, measuring, b"\n");
    }
    if problems.past > 0 {
        put(writer, &mut length, measuring, b"and ");
        put(writer, &mut length, measuring, Decimal::of(u64::from(problems.past)).as_bytes());
        put(writer, &mut length, measuring, b" more\n");
    }
    length
}
#[must_use]
pub fn lines(problems: &Problems) -> Box<[u8]> {
    let length = render(problems, &mut Writer::new(0), true);
    let mut writer = Writer::new(length);
    assert_eq!(render(problems, &mut writer, false), length, "diagnostic passes measure identically");
    writer.finish()
}
