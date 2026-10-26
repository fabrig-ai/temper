//! The QA model's document and played scripts (protocol/setup.md, 8.4).
pub mod observations;
use crate::{Refusal, credential, read, turn};
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::Path;
use skein_fake_llm_domain::{self as domain, api};
use skein_json::Document as Json;
use skein_lib::{Duration, List};

/// One checked advertised model. The peer has no model-name admission table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Model {
    pub name: Box<[u8]>,
    pub window: u32,
    pub output: u32,
}
/// A standalone model document, checked before any output is created.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Document {
    listen: SocketAddr,
    control: Option<SocketAddr>,
    key: Box<[u8]>,
    added_keys: Box<[Box<[u8]>]>,
    root: Option<Box<[u8]>>,
    models: Box<[Model]>,
    scripts: api::Scripts,
    rules: api::Rules,
    reply: Duration,
}
impl Document {
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        let d = read::document(bytes)?;
        let p = Path::root();
        let mut o = read::object(&d, 0, &p)?;
        let listen_at = read::required(&mut o, &d, b"listen", &p)?;
        let control_at = o.member(&d, b"control");
        let key = read::field(&mut o, &d, b"key", crate::limits::PATH_BYTES, &p)?;
        let added_keys_at = o.member(&d, b"added_keys");
        let tls_at = read::required(&mut o, &d, b"tls", &p)?;
        let models_at = read::required(&mut o, &d, b"models", &p)?;
        let cued_at = read::required(&mut o, &d, b"cued", &p)?;
        let asked_at = read::required(&mut o, &d, b"asked", &p)?;
        let otherwise_at = read::required(&mut o, &d, b"otherwise", &p)?;
        let pace_at = o.member(&d, b"pace");
        read::finish(o, &d, &p)?;
        let reply = read::pace(&d, pace_at, b"reply_ms", &p.member(b"pace"))?;
        let listen = read::listen(&d, listen_at, &p.member(b"listen"))?;
        let control = match control_at {
            Some(at) => Some(read::listen(&d, at, &p.member(b"control"))?),
            None => None,
        };
        read::absolute(&key, &p.member(b"key"))?;
        let added_keys = match added_keys_at {
            Some(at) => added_keys(&d, at, &key, &p.member(b"added_keys"))?,
            None => Box::new([]),
        };
        let credentials =
            u32::try_from(added_keys.len()).expect("fifteen added keys").checked_add(1).expect("primary key");
        let root = if read::is_null(&d, tls_at) {
            None
        } else {
            let p = p.member(b"tls");
            let mut o = read::object(&d, tls_at, &p)?;
            let root = read::field(&mut o, &d, b"root", crate::limits::PATH_BYTES, &p)?;
            read::finish(o, &d, &p)?;
            read::absolute(&root, &p.member(b"root"))?;
            Some(root)
        };
        let models = models(&d, models_at, &p.member(b"models"))?;
        let (cued, cued_rules) = scripts_with_inputs(&d, cued_at, &p.member(b"cued"), Some(credentials))?;
        let (asked, asked_rules) = scripts_with_inputs(&d, asked_at, &p.member(b"asked"), Some(credentials))?;
        let scripts = api::Scripts {
            cued,
            asked,
            otherwise: standalone_turns(&d, otherwise_at, &p.member(b"otherwise"), Some(credentials))?,
        };
        if control.is_none() && holds(&scripts) {
            return Err(read::why(&p.member(b"control"), b"required when a piece has a hold"));
        }
        let rules = api::Rules { cued: cued_rules, asked: asked_rules };
        check_inputs(&scripts, &rules, &p.member(b"otherwise"))?;
        Ok(Self { listen, control, key, added_keys, root, models, scripts, rules, reply })
    }
    pub(crate) fn checked(
        listen: SocketAddr,
        key: Box<[u8]>,
        root: Option<Box<[u8]>>,
        models: Box<[Model]>,
        scripts: api::Scripts,
        p: &Path,
    ) -> Result<Self, Refusal> {
        check_scripts(&scripts, &p.member(b"otherwise"))?;
        let rules = api::Rules { cued: counted_rules(&scripts.cued), asked: counted_rules(&scripts.asked) };
        Ok(Self {
            listen,
            control: None,
            key,
            added_keys: Box::new([]),
            root,
            models,
            scripts,
            rules,
            reply: Duration::ZERO,
        })
    }
    #[must_use]
    pub fn listen(&self) -> SocketAddr {
        self.listen
    }
    #[must_use]
    pub fn control(&self) -> Option<SocketAddr> {
        self.control
    }
    #[must_use]
    pub fn key(&self) -> &[u8] {
        &self.key
    }
    #[must_use]
    pub fn added_keys(&self) -> &[Box<[u8]>] {
        &self.added_keys
    }
    #[must_use]
    pub fn root(&self) -> Option<&[u8]> {
        self.root.as_deref()
    }
    #[must_use]
    pub fn models(&self) -> &[Model] {
        &self.models
    }
    #[must_use]
    pub fn reply(&self) -> Duration {
        self.reply
    }
    /// The same fixed latency for every reply, without random failures.
    #[must_use]
    pub fn limits(&self) -> domain::Config {
        domain::Config { latency_min: self.reply(), latency_max: self.reply(), ..LIMITS }
    }
    /// No menu or random failure. Unknown model names receive the same scripts:
    /// skein's provider retains no model list and performs no per-model admission.
    #[must_use]
    pub fn domain(&self, seed: u64) -> domain::Domain {
        domain::Domain::played_by_input(&self.limits(), seed, self.scripts.clone(), empty_menu(), self.rules.clone())
            .expect("checked scripts and rules")
    }
}
/// `OpenAI`'s public-route configuration, accepting the primary and added keys.
/// Typed wire codecs are constructed by the peer factory; the document's advertised
/// models stay here beside the played domain, which has no per-model admission.
#[derive(Debug)]
pub struct Configuration {
    pub listen: SocketAddr,
    pub control: Option<SocketAddr>,
    pub tls: bool,
    pub key: skein_lib::Secret,
    pub added_keys: Box<[skein_lib::Secret]>,
    pub models: Box<[Model]>,
    pub domain: domain::Domain,
    pub limits: domain::Config,
}
/// The public `OpenAI` Responses route; no Codex account id is configured.
pub const ROUTE: &[u8] = b"/v1/responses";
/// Builds the paced played domain without a menu, and its accepted keys.
#[must_use]
pub fn configuration(document: &Document, key: &[u8], seed: u64) -> Configuration {
    let mut added_keys = List::with_capacity(u32::try_from(document.added_keys.len()).expect("fifteen keys"));
    for i in 0..u32::try_from(document.added_keys.len()).expect("fifteen keys") {
        let key = credential::credential(seed, credential::added_key(i));
        added_keys.push(skein_lib::Secret::new(skein_lib::bytes::copy_of(&key))).expect("fifteen keys");
    }
    Configuration {
        listen: document.listen,
        control: document.control,
        tls: document.root.is_some(),
        key: skein_lib::Secret::new(skein_lib::bytes::copy_of(key)),
        added_keys: added_keys.into_boxed(),
        models: document.models.clone(),
        domain: document.domain(seed),
        limits: document.limits(),
    }
}
fn empty_menu() -> api::Menu {
    api::Menu { arguments: Box::new([]), invalid: Box::new([]) }
}
fn added_keys(d: &Json, at: u32, key: &[u8], p: &Path) -> Result<Box<[Box<[u8]>]>, Refusal> {
    let items = match jig_protocol_config::read::list(d, at, credential::ADDED_KEYS) {
        Ok(items) => items,
        Err(w) => return Err(read::kind(p, w)),
    };
    let mut keys = List::<Box<[u8]>>::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let path = read::text(d, *at, crate::limits::PATH_BYTES, &p)?;
        read::absolute(&path, &p)?;
        if path.as_ref() == key || keys.as_slice().contains(&path) {
            return Err(read::why(&p, b"must be unique and other than key"));
        }
        keys.push(path).expect("fifteen added keys");
    }
    Ok(keys.into_boxed())
}
fn models(d: &Json, at: u32, p: &Path) -> Result<Box<[Model]>, Refusal> {
    let items = read::list(d, at, 1, p)?;
    let mut out = List::<Model>::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let mut o = read::object(d, *at, &p)?;
        let name = read::field(&mut o, d, b"name", crate::limits::MODEL_NAME_BYTES, &p)?;
        let window_at = read::required(&mut o, d, b"window", &p)?;
        let output_at = read::required(&mut o, d, b"output", &p)?;
        read::finish(o, d, &p)?;
        let window = read::integer(d, window_at, u64::from(u32::MAX), &p.member(b"window"))?;
        let output = read::integer(d, output_at, u64::from(u32::MAX), &p.member(b"output"))?;
        out.push(Model {
            name,
            window: u32::try_from(window).expect("u32 bound"),
            output: u32::try_from(output).expect("u32 bound"),
        })
        .expect("eight models");
    }
    check_models(out.as_slice(), p, true)?;
    Ok(out.into_boxed())
}
/// The same model bounds for the document and a deployment's derivation.
/// The latter reports every model problem at its provider member.
pub(crate) fn check_models(models: &[Model], p: &Path, indexed: bool) -> Result<(), Refusal> {
    if models.len() > usize::try_from(crate::ENTRIES).expect("eight models") {
        return Err(read::why(p, b"past its bound"));
    }
    if models.is_empty() {
        return Err(read::why(p, b"must be non-empty"));
    }
    for (i, model) in models.iter().enumerate() {
        let here = if indexed { read::index(p, i) } else { p.clone() };
        let name = if indexed { here.member(b"name") } else { here.clone() };
        let window = if indexed { here.member(b"window") } else { here.clone() };
        let output = if indexed { here.member(b"output") } else { here.clone() };
        read::nonempty(&model.name, &name)?;
        if model.name.len() > usize::try_from(crate::limits::MODEL_NAME_BYTES).expect("name bound") {
            return Err(read::why(&name, b"past its bound"));
        }
        if model.window == 0 {
            return Err(read::why(&window, b"must be at least 1"));
        }
        if model.output == 0 {
            return Err(read::why(&output, b"must be at least 1"));
        }
        if model.output > model.window {
            return Err(read::why(&output, b"must not exceed window"));
        }
        for prev in models.iter().take(i) {
            if prev.name == model.name {
                return Err(read::why(&name, b"must be unique"));
            }
        }
    }
    Ok(())
}
fn counted_rules(scripts: &[api::Script]) -> InputRules {
    let mut rules = List::with_capacity(u32::try_from(scripts.len()).expect("eight scripts"));
    for _ in scripts {
        rules.push(Box::<[api::Rule]>::default()).expect("eight scripts");
    }
    rules.into_boxed()
}
fn empty_rules() -> api::Rules {
    api::Rules { cued: Box::new([]), asked: Box::new([]) }
}
pub(crate) fn check_scripts(scripts: &api::Scripts, p: &Path) -> Result<(), Refusal> {
    check_inputs(scripts, &empty_rules(), p)
}
fn check_inputs(scripts: &api::Scripts, rules: &api::Rules, p: &Path) -> Result<(), Refusal> {
    // The domain owns the joint byte accounting, including every wrapper.
    match domain::Domain::played_by_input(&LIMITS, 0, scripts.clone(), empty_menu(), rules.clone()) {
        Ok(_) => Ok(()),
        Err(_) => Err(read::why(p, b"scripts past their joint byte bound")),
    }
}
pub(crate) fn scripts(d: &Json, at: u32, p: &Path) -> Result<Box<[api::Script]>, Refusal> {
    Ok(scripts_with_inputs(d, at, p, None)?.0)
}
type InputRules = Box<[Box<[api::Rule]>]>;
/// Only standalone scripts may carry input rules, pieces and failing lines.
fn scripts_with_inputs(
    d: &Json,
    at: u32,
    p: &Path,
    credentials: Option<u32>,
) -> Result<(Box<[api::Script]>, InputRules), Refusal> {
    let items = read::list(d, at, 0, p)?;
    let mut out = List::<api::Script>::with_capacity(items.len());
    let mut rules = List::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let mut o = read::object(d, *at, &p)?;
        let cue = read::field(&mut o, d, b"cue", crate::limits::CUE_BYTES, &p)?;
        let turns_at = read::required(&mut o, d, b"turns", &p)?;
        let inputs_at = if credentials.is_some() { o.member(d, b"inputs") } else { None };
        read::finish(o, d, &p)?;
        read::nonempty(&cue, &p.member(b"cue"))?;
        for prev in &out {
            if prev.cue == cue {
                return Err(read::why(&p.member(b"cue"), b"must be unique"));
            }
        }
        let turns = standalone_turns(d, turns_at, &p.member(b"turns"), credentials)?;
        let inputs = match inputs_at {
            Some(at) => inputs(d, at, turns.len(), &p.member(b"inputs"))?,
            None => Box::new([]),
        };
        rules.push(inputs).expect("eight scripts");
        out.push(api::Script { cue, turns }).expect("eight scripts");
    }
    Ok((out.into_boxed(), rules.into_boxed()))
}
fn inputs(d: &Json, at: u32, turns: usize, p: &Path) -> Result<Box<[api::Rule]>, Refusal> {
    const RULES: u32 = 16;
    let items = match jig_protocol_config::read::list(d, at, RULES) {
        Ok(items) => items,
        Err(w) => return Err(read::kind(p, w)),
    };
    if items.is_empty() {
        return Err(read::why(p, b"must be non-empty"));
    }
    let mut rules = List::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let mut o = read::object(d, *at, &p)?;
        let cue = read::field(&mut o, d, b"cue", crate::limits::CUE_BYTES, &p)?;
        let turn_at = read::required(&mut o, d, b"turn", &p)?;
        read::finish(o, d, &p)?;
        if cue.is_empty()
            != (i.checked_add(1).expect("sixteen rules") == usize::try_from(items.len()).expect("sixteen rules"))
        {
            return Err(read::why(&p.member(b"cue"), b"only the last rule must have an empty cue"));
        }
        let turn = read::integer(d, turn_at, u64::from(u32::MAX), &p.member(b"turn"))?;
        if turn >= u64::try_from(turns).expect("eight turns") {
            return Err(read::why(&p.member(b"turn"), b"must index this script's turns"));
        }
        rules.push(api::Rule { cue, turn: u32::try_from(turn).expect("eight turns") }).expect("sixteen rules");
    }
    Ok(rules.into_boxed())
}
pub(crate) fn turns(d: &Json, at: u32, p: &Path) -> Result<Box<[api::Turn]>, Refusal> {
    standalone_turns(d, at, p, None)
}
fn standalone_turns(d: &Json, at: u32, p: &Path, credentials: Option<u32>) -> Result<Box<[api::Turn]>, Refusal> {
    let items = read::list(d, at, 1, p)?;
    let mut out = List::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let turn = turn::Turn::read_standalone(d, *at, &read::index(p, i), credentials)?;
        out.push(turn::into_skein(turn)).expect("eight turns");
    }
    Ok(out.into_boxed())
}
fn holds(scripts: &api::Scripts) -> bool {
    for script in scripts.cued.iter().chain(scripts.asked.iter()) {
        if turns_hold(&script.turns) {
            return true;
        }
    }
    turns_hold(&scripts.otherwise)
}
fn turns_hold(turns: &[api::Turn]) -> bool {
    for turn in turns {
        for line in &turn.lines {
            if let api::Line::Pieces { pieces } = line {
                for piece in pieces {
                    if piece.hold.is_some() {
                        return true;
                    }
                }
            }
        }
    }
    false
}
/// Live-domain limits: joint script bytes include all 17 scripts' wrappers.
pub const LIMITS: domain::Config = domain::Config {
    calls: 8,
    query_bytes: 1 << 20,
    script_bytes: crate::SCRIPT_BYTES,
    answer_bytes: crate::ANSWER_BYTES,
    latency_min: Duration::ZERO,
    latency_max: Duration::ZERO,
    overloaded: 0,
    rate_limited: 0,
    retry_after: Duration::ZERO,
    unavailable: 0,
    too_long: 0,
    unauthorized: 0,
    refused: 0,
    no_calls: 0,
    answer_tokens: 16_384,
    calls_per_answer: 8,
    malformed: 0,
    outside_choice: 0,
    tool_rounds: 0,
    cache_lifetime: Duration::ZERO,
    cache_entries: 0,
    unscoped_reads: 0,
};

/// Constructs the checked configuration's peer without io (protocol/setup.md, 8.4).
pub fn peer(configuration: Configuration) -> Result<skein_fake_peers::llm::Peer, skein_fake_peers::Error> {
    crate::wire::llm(configuration, false)
}

/// Constructs a peer retaining the real loop's bounded outside query records.
pub fn recording_peer(configuration: Configuration) -> Result<skein_fake_peers::llm::Peer, skein_fake_peers::Error> {
    crate::wire::llm(configuration, true)
}

/// One bounded, bodyless HTTP/1.1 control exchange on skein's server machine.
/// The host queues the requested release before acknowledging it (protocol/setup.md, 8.4).
pub mod control {
    use alloc::boxed::Box;
    use skein_http::{Method, Version, server as http};
    use skein_lib::{
        Env, Queue, Time, Wall, bytes,
        stream::{Down, Up},
    };

    const LIMITS: http::Limits =
        http::Limits { head: 1024, headers: 1024 / 3, body: 0, read: 1, response: 256, send: 1 };
    const QUEUE: u32 = 8;

    #[derive(Debug)]
    pub struct Exchange {
        server: http::Server,
        events: Queue<http::Event>,
        requests: Queue<http::Request>,
        below: Queue<Down>,
        release: Option<Box<[u8]>>,
        done: bool,
    }
    impl Default for Exchange {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Exchange {
        #[must_use]
        pub fn new() -> Self {
            let mut requests = Queue::with_capacity(QUEUE);
            requests.push(http::Request::Next);
            Self {
                server: http::Server::new(&LIMITS),
                events: Queue::with_capacity(QUEUE),
                requests,
                below: Queue::with_capacity(QUEUE),
                release: None,
                done: false,
            }
        }
        pub fn up(&mut self, now: Time, wall: Wall, up: Up) {
            http::up(&mut self.server, &Env { now, wall, limits: LIMITS }, up, &mut self.events, &mut self.below);
            self.events();
        }
        pub fn advance(&mut self, now: Time, wall: Wall) {
            for _ in 0..QUEUE {
                if self.events.room() < 3 || self.below.room() < 2 {
                    break;
                }
                let Some(request) = self.requests.pop() else { break };
                http::down(
                    &mut self.server,
                    &Env { now, wall, limits: LIMITS },
                    request,
                    &mut self.events,
                    &mut self.below,
                );
                self.events();
            }
        }
        pub fn shutdown(&mut self) {
            for _ in 0..self.requests.len() {
                self.requests.pop().expect("one pass over requests");
            }
            self.release = None;
            self.requests.push(http::Request::Close);
        }
        pub fn release(&mut self) -> Option<Box<[u8]>> {
            self.release.take()
        }
        pub fn released(&mut self, queued: bool) {
            self.respond(if queued { 204 } else { 400 });
        }
        pub fn pop(&mut self) -> Option<Down> {
            self.below.pop()
        }
        #[must_use]
        pub fn done(&self) -> bool {
            self.done && self.below.is_empty() && self.requests.is_empty()
        }
        #[must_use]
        pub fn work_pending(&self) -> bool {
            self.done || self.release.is_some() || !self.requests.is_empty() || !self.below.is_empty()
        }
        #[must_use]
        pub fn worst_case() -> u64 {
            let mut bytes = http::worst_case(&LIMITS).expect("fixed HTTP limits");
            for owned in [
                Queue::<http::Event>::worst_case(QUEUE),
                Queue::<http::Request>::worst_case(QUEUE),
                Queue::<Down>::worst_case(QUEUE),
                u64::from(QUEUE).checked_mul(u64::from(LIMITS.response)),
                Some(64),
            ] {
                bytes = bytes.checked_add(owned.expect("bounded control storage")).expect("fixed control footprint");
            }
            bytes
        }
        fn respond(&mut self, status: u16) {
            // Respond before discarding: a forbidden body gets no read or 100
            // Continue. The discard then settles even an empty body's stream.
            self.requests.push(http::Request::Respond(http::Response {
                status,
                headers: Box::new([]),
                body: http::Body::None,
                close: true,
            }));
            self.requests.push(http::Request::Discard);
        }
        fn events(&mut self) {
            while let Some(event) = self.events.pop() {
                match event {
                    http::Event::Call(call) => {
                        let bodyless = match call.body {
                            http::Body::None | http::Body::Length(0) => true,
                            http::Body::Length(_) | http::Body::Chunked => false,
                        };
                        if call.version != Version::Http11 || !bodyless {
                            self.respond(400);
                        } else if call.method != Method::Post || !call.target.starts_with(b"/release/") {
                            self.respond(404);
                        } else {
                            let name = call.target.strip_prefix(b"/release/").expect("checked release prefix");
                            if name.is_empty() || name.len() > 64 {
                                self.respond(400);
                            } else {
                                self.release = Some(bytes::copy_of(name));
                            }
                        }
                    }
                    http::Event::Failed(http::Error::Rejected(rejection)) => {
                        // The control has one refusal status, even for a head past its
                        // bound. Preserve the server's reserved Send and stream demands.
                        let answer: &[u8] = if rejection == http::Rejection::Method {
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        } else {
                            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        };
                        for _ in 0..self.below.len() {
                            let down = self.below.pop().expect("one pass over outputs");
                            self.below.push(match down {
                                Down::Send(_) => Down::Send(bytes::copy_of(answer)),
                                other @ (Down::Demand { .. } | Down::Finish) => other,
                            });
                        }
                        self.requests.push(http::Request::Close);
                    }
                    http::Event::Failed(http::Error::Truncated) => {
                        // A partial head still owns the response room reserved before
                        // its first line. End crossed its outstanding read.
                        self.below.push(Down::Demand { read: skein_lib::stream::Read::Nothing, room: 0 });
                        self.below.push(Down::Send(bytes::copy_of(
                            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )));
                        self.requests.push(http::Request::Close);
                    }
                    http::Event::Done(_) | http::Event::Ended | http::Event::Failed(_) => {
                        self.requests.push(http::Request::Close);
                    }
                    http::Event::Closed => self.done = true,
                    http::Event::Body(_) => {}
                    http::Event::Reply(_) | http::Event::Refused(_) => unreachable!("bodyless control exchange"),
                }
            }
        }
    }
}
