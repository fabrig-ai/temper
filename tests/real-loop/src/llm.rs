//! Independent LLM peer configuration (skein's fake-llm.md, 6).
use skein_fake_llm_domain::{self as domain, api};
use skein_fake_llm_protocol::{documents, provider};
use skein_fake_peers::{Limits, Transport, llm::Peer};
use skein_lib::{Duration, Time};
use std::net::{Ipv4Addr, SocketAddr};

/// Every argument after the peer's address, in its own boundary types.
pub struct Recipe {
    pub transport: Transport,
    pub limits: Limits,
    pub config: provider::Config,
    pub credential: skein_llm::Credential,
    pub protocol_limits: provider::Limits,
    pub domain: domain::Domain,
    pub domain_limits: domain::Config,
}

#[must_use]
pub fn scripted(scripts: Box<[api::Script]>) -> Recipe {
    scripted_sized(scripts, 16_384)
}

/// The same peer with room for a story's larger scripted item and its framing.
#[must_use]
pub fn scripted_sized(scripts: Box<[api::Script]>, strings: u32) -> Recipe {
    played_sized(api::Scripts { cued: scripts, asked: Box::new([]), otherwise: Box::new([]) }, strings)
}

/// The first script belongs to the Open chat; its delegates remain system cued.
#[must_use]
pub fn scripted_open(scripts: Box<[api::Script]>, strings: u32) -> Recipe {
    played_sized(open_scripts(scripts), strings)
}

fn open_scripts(scripts: Box<[api::Script]>) -> api::Scripts {
    let mut scripts = scripts.into_vec();
    let asked = if scripts.is_empty() { Box::new([]) } else { vec![scripts.remove(0)].into_boxed_slice() };
    api::Scripts { cued: scripts.into_boxed_slice(), asked, otherwise: Box::new([]) }
}

fn played_sized(scripts: api::Scripts, strings: u32) -> Recipe {
    // Smith's configured peer bounds, for an agent's prompt and finish call.
    // REQUEST is this fake's admission, not production's bound (50,331,648 B
    // from the template's window, crates/temper-init/template/agent.json).
    // The first-turn request with a workspace mounted measured 31,007 B
    // when the delegate and amend schemas landed.
    const REQUEST: u32 = 65_536;
    // Larger wire chunks keep the zero-latency real-loop peer cheap.
    const WIRE_CHUNK: u32 = 4096;
    // The fake also parses an opaque scripted item through its request document
    // reader: a later request carries the prompt and finish call beside each
    // scripted item the conversation sends back.
    let request = REQUEST.checked_add(strings).expect("fake request admission");
    let mut config = skein_llm_world::fake::config();
    config.latency_min = Duration::ZERO;
    config.latency_max = Duration::ZERO;
    config.script_bytes = config.script_bytes.max(strings);
    config.answer_bytes = config.answer_bytes.max(strings);
    // The domain admits the query that request carries: prompt, tools and items.
    config.query_bytes = config.query_bytes.max(request);
    let mut protocol = skein_llm_world::fake::limits(&skein_llm_world::limits());
    protocol.http.read = WIRE_CHUNK;
    protocol.http.send = WIRE_CHUNK;
    protocol.sse.chunk = WIRE_CHUNK;
    protocol.documents.openai.request = request;
    protocol.documents.openai.strings = strings;
    protocol.documents.openai.answer = protocol.documents.openai.answer.max(strings);
    protocol.http.body = protocol.http.body.max(u64::from(request));
    protocol.sse.event = protocol.sse.event.max(strings);
    protocol.documents.openai.tokens = 4096;
    protocol.documents.openai.output_items = 256;
    protocol.documents.anthropic.output_items = 256;
    Recipe {
        transport: Transport::Plaintext,
        limits: smith_agent_process_world::fake::limits(),
        config: provider::Config {
            without_credential: false,
            echo: skein_llm::openai::Echo::NONE,
            usage_fields: documents::UsageFields::ALL,
            provider: documents::Provider::OpenAi,
            path: b"/v1/responses".as_slice().into(),
            headers: Box::new([]),
        },
        credential: skein_llm::Credential {
            value: jig_channel_world::fixtures::KEY.into(),
            scheme: skein_llm::Scheme::Key,
            account_id: Box::new([]),
        },
        protocol_limits: protocol,
        domain: domain::Domain::played(
            &config,
            17,
            scripts,
            api::Menu { arguments: Box::new([]), invalid: Box::new([]) },
        )
        .expect("bounded real-loop scripts"),
        domain_limits: config,
    }
}

/// The scripted peer with opt-in fresh-input turn selection (skein's fake-llm.md, 2.4).
#[must_use]
pub fn played_by_input(scripts: Box<[api::Script]>, rules: Box<[Box<[api::Rule]>]>) -> Recipe {
    let mut recipe = scripted(Box::new([]));
    recipe.domain = domain::Domain::played_by_input(
        &recipe.domain_limits,
        17,
        api::Scripts { cued: scripts, asked: Box::new([]), otherwise: Box::new([]) },
        api::Menu { arguments: Box::new([]), invalid: Box::new([]) },
        api::Rules { cued: rules, asked: Box::new([]) },
    )
    .expect("bounded real-loop scripts and rules");
    recipe
}

/// Fresh-input rules paired with one Open chat followed by Finite delegates.
/// Its rules choose the reply even when the person's next message has a new cue.
#[must_use]
pub fn played_by_input_open(scripts: Box<[api::Script]>, rules: Box<[Box<[api::Rule]>]>) -> Recipe {
    assert_eq!(scripts.len(), rules.len(), "index-parallel script rules");
    let mut scripts = open_scripts(scripts);
    if let Some(chat) = scripts.asked.first_mut() {
        chat.cue = Box::new([]);
    }
    let mut rules = rules.into_vec();
    let asked = if rules.is_empty() { Box::new([]) } else { vec![rules.remove(0)].into_boxed_slice() };
    let mut recipe = scripted(Box::new([]));
    recipe.domain = domain::Domain::played_by_input(
        &recipe.domain_limits,
        17,
        scripts,
        api::Menu { arguments: Box::new([]), invalid: Box::new([]) },
        api::Rules { cued: rules.into_boxed_slice(), asked },
    )
    .expect("bounded Open scripts and rules");
    recipe
}

pub fn peer(recipe: Recipe) -> Result<Peer, skein_fake_peers::Error> {
    let mut peer = Peer::new(
        (Ipv4Addr::LOCALHOST, 0).into(),
        recipe.transport,
        recipe.limits,
        recipe.config,
        recipe.credential,
        recipe.protocol_limits,
        recipe.domain,
        recipe.domain_limits,
    )?;
    peer.accept(skein_llm::Credential {
        value: crate::fixtures::SECOND_KEY.into(),
        scheme: skein_llm::Scheme::Key,
        account_id: Box::new([]),
    })?;
    Ok(peer)
}

/// Bind and drain the peer alone under exact ownership checking.
pub fn bound(recipe: impl FnOnce() -> Recipe) -> SocketAddr {
    let mut world = skein_world::real::World::new_checked(Bound { seen: false, wake: None });
    world.spawn(|| peer(recipe()).expect("configured independent peer"));
    let outcome = world.run(&skein_shell::Clock::new(), crate::PATIENCE);
    outcome.procs[0].address().expect("peer listened before shutdown")
}

struct Bound {
    seen: bool,
    wake: Option<Time>,
}
impl skein_world::Referee<Peer> for Bound {
    fn act(&mut self, _now: Time, procs: &mut [Peer]) {
        if !self.seen && procs[0].address().is_some() {
            self.seen = true;
            self.wake = None;
            procs[0].shutdown();
        }
    }
    fn observe(&mut self, now: Time, procs: &[Peer]) {
        if !self.seen && procs[0].address().is_some() {
            self.wake = Some(now);
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        self.wake
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        self.seen
    }
}
