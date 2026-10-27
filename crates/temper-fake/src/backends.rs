//! Bounded public-document rewrites for configuration.md, 12.1. No files or sockets are opened here.
use crate::{Refusal, read};
use alloc::boxed::Box;
use core::net::{IpAddr, SocketAddr};
use jig_protocol_config::{Path, read as config};
use skein_json::{
    Document,
    document::Kind,
    writer::{Encoder, Limits},
};
use skein_lib::{Decimal, List, bytes};
use temper_forge_setup::{Kind as StepKind, Step};

/// Absolute effective directories and the already provisioned loopback bundle.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Paths {
    pub config: Box<[u8]>,
    pub data: Box<[u8]>,
    pub cache: Box<[u8]>,
    pub bundle: Box<[u8]>,
    pub claude: Box<[u8]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Peers {
    pub github: SocketAddr,
    pub oauth: SocketAddr,
    pub llm: SocketAddr,
    pub forgejo: SocketAddr,
}
/// Shared by the supervised welcome peers (protocol/setup.md, 7.5).
pub const SEED: u64 = 7;

/// The fake program's model, alongside the credential-backed `OpenAI` model.
pub const PROGRAM_MODEL: &str = "fake-claude";

/// Long peers before ordinary box steps. The caller checks absolute, bounded config bytes and sockets.
#[must_use]
pub fn steps(config: &[u8], peers: &Peers) -> Box<[Step]> {
    let mut steps = List::with_capacity(4);
    let seed = Decimal::of(SEED);
    for (name, command, document, listen) in [
        (b"fake github".as_slice(), b"forge".as_slice(), b"github.json".as_slice(), peers.github),
        (b"fake issuer", b"oauth", b"oauth.json", peers.oauth),
        (b"fake model", b"llm", b"llm.json", peers.llm),
        (b"fake forgejo", b"forge", b"forgejo.json", peers.forgejo),
    ] {
        let mut arguments = List::with_capacity(6);
        for word in [b"fake".as_slice(), command, b"--document"] {
            arguments.push(bytes::copy_of(word)).expect("peer arguments counted");
        }
        arguments.push(path(config, document)).expect("peer document counted");
        arguments.push(bytes::copy_of(b"--seed")).expect("seed flag counted");
        arguments.push(bytes::copy_of(seed.as_bytes())).expect("seed counted");
        steps
            .push(Step::new(bytes::copy_of(name), arguments.into_boxed(), StepKind::Long).listening(Box::new([listen])))
            .expect("four welcome peers counted");
    }
    steps.into_boxed()
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Documents {
    pub temper: Box<[u8]>,
    pub agent: Box<[u8]>,
    pub github: Box<[u8]>,
    pub oauth: Box<[u8]>,
    pub llm: Box<[u8]>,
    pub forgejo: Box<[u8]>,
}
const OUTPUT: Limits = Limits { depth: 32, length: crate::DOCUMENT_BYTES };
// Init-shaped temper: forge roots/program (5), sign-in (6),
// model endpoint (2), program route (3), two charter alternatives (2), bundle
// (1), worker argv/configuration (2), managed forge/provider limits (9). Each of the
// forge, sign-in and endpoint may also remove all four stale TLS members.
// The agent document needs only its endpoint (6), route (1) and adapter (1).
const EDITS: u32 = 5 + 6 + 2 + 3 + 2 + 1 + 2 + 9 + 3 * 4;
// The Model step must see OpenAI, demo and Anthropic in one provider offer.
const PROVIDER_LINES: u32 = 3;
#[derive(Debug)]
struct Edit {
    first: u32,
    after: u32,
    value: Option<Document>,
}
#[derive(Debug)]
struct Edits {
    rows: List<Edit>,
}
impl Edits {
    fn new() -> Self {
        Self { rows: List::with_capacity(EDITS) }
    }
    fn set(&mut self, d: &Document, at: u32, bytes: &[u8], p: &Path) -> Result<(), Refusal> {
        let after = config::after(d, at).or(Err(read::why(p, b"invalid value")))?;
        let value = Some(read::document(bytes)?);
        self.rows.push(Edit { first: at, after, value }).or(Err(read::why(p, b"too many rewrites")))
    }
    fn text(&mut self, d: &Document, at: u32, value: &[u8], p: &Path) -> Result<(), Refusal> {
        self.set(d, at, &text(value)?, p)
    }
    fn remove(&mut self, d: &Document, at: u32, p: &Path) -> Result<(), Refusal> {
        let first = at.checked_sub(1).expect("an object value follows its key");
        let after = config::after(d, at).or(Err(read::why(p, b"invalid value")))?;
        self.rows.push(Edit { first, after, value: None }).or(Err(read::why(p, b"too many rewrites")))
    }
    fn write(&self, d: &Document, out: &mut Encoder) {
        let mut skip = 0;
        for at in 0..d.len() {
            if at < skip {
                continue;
            }
            let mut edited = false;
            for edit in &self.rows {
                if edit.first == at {
                    if let Some(value) = &edit.value {
                        if edit.first == edit.after
                            && value.token(0).expect("inserted value").kind == Kind::ObjectStart
                            && d.token(at).expect("container close").kind == Kind::ObjectEnd
                        {
                            for inner in 1..value.len().checked_sub(1).expect("object close") {
                                token(value, inner, out);
                            }
                        } else {
                            out.document(value);
                        }
                    }
                    skip = edit.after;
                    edited = true;
                    break;
                }
            }
            if !edited || skip == at {
                token(d, at, out);
            }
        }
    }
    fn finish(&self, d: &Document) -> Result<Box<[u8]>, Refusal> {
        let mut out = Encoder::measure(&OUTPUT);
        self.write(d, &mut out);
        let size = out.measured().or(Err(read::why(&Path::root(), b"effective document past its byte bound")))?;
        let mut out = Encoder::write(size, &OUTPUT);
        self.write(d, &mut out);
        Ok(out.finish())
    }
}
fn token(d: &Document, at: u32, out: &mut Encoder) {
    let t = d.token(at).expect("bounded document token");
    let bytes = d.text(t).expect("document text admitted");
    match t.kind {
        Kind::ObjectStart => out.object_start(),
        Kind::ObjectEnd => out.object_end(),
        Kind::ArrayStart => out.array_start(),
        Kind::ArrayEnd => out.array_end(),
        Kind::Key => out.key(bytes),
        Kind::String => out.string(bytes),
        Kind::Number => out.number(bytes),
        Kind::True => out.boolean(true),
        Kind::False => out.boolean(false),
        Kind::Null => out.null(),
        Kind::Long => unreachable!("whole document parsing retains bounded strings"),
    }
}
fn text(bytes: &[u8]) -> Result<Box<[u8]>, Refusal> {
    let mut out = Encoder::measure(&OUTPUT);
    out.string(bytes);
    let size = out.measured().or(Err(read::why(&Path::root(), b"text past its byte bound")))?;
    let mut out = Encoder::write(size, &OUTPUT);
    out.string(bytes);
    Ok(out.finish())
}
fn member(d: &Document, at: u32, name: &[u8], p: &Path) -> Result<u32, Refusal> {
    let mut o = read::object(d, at, p)?;
    read::required(&mut o, d, name, p)
}
fn selected(d: &Document, at: u32, key: &[u8], wanted: &[u8], number: bool, p: &Path) -> Result<u32, Refusal> {
    let items = read::list(d, at, 1, p)?;
    let mut found = None;
    for (i, at) in items.iter().enumerate() {
        let row = read::index(p, i);
        let index = member(d, *at, key, &row)?;
        let value = if number {
            let n = read::integer(d, index, u64::from(u32::MAX), &row.member(key))?;
            read::joined(&[Decimal::of(n).as_bytes()])
        } else {
            read::text(d, index, 128, &row.member(key))?
        };
        if value.as_ref() == wanted {
            if found.is_some() {
                return Err(read::why(p, b"target must be unique"));
            }
            found = Some(*at);
        }
    }
    found.ok_or(read::why(p, b"target missing"))
}
fn socket(addr: SocketAddr, at: &Path) -> Result<Box<[u8]>, Refusal> {
    if !addr.ip().is_loopback() || addr.port() == 0 || addr.port() == 1455 {
        return Err(read::why(at, b"expected a nonzero loopback peer, distinct from callback 1455"));
    }
    match addr {
        SocketAddr::V6(ip) if ip.scope_id() != 0 || ip.flowinfo() != 0 => {
            return Err(read::why(at, b"scoped IPv6 peers are unsupported"));
        }
        SocketAddr::V4(_) | SocketAddr::V6(_) => {}
    }
    let port = Decimal::of(u64::from(addr.port()));
    match addr.ip() {
        IpAddr::V4(ip) => {
            let [a, b, c, d] = ip.octets();
            let a = Decimal::of(u64::from(a));
            let b = Decimal::of(u64::from(b));
            let c = Decimal::of(u64::from(c));
            let d = Decimal::of(u64::from(d));
            Ok(read::joined(&[
                a.as_bytes(),
                b".",
                b.as_bytes(),
                b".",
                c.as_bytes(),
                b".",
                d.as_bytes(),
                b":",
                port.as_bytes(),
            ]))
        }
        IpAddr::V6(_) => Ok(read::joined(&[b"[::1]:", port.as_bytes()])),
    }
}
fn address(socket: &[u8]) -> Result<Box<[u8]>, Refusal> {
    Ok(read::joined(&[b"{\"socket\":", &text(socket)?, b"}"]))
}
fn path(base: &[u8], name: &[u8]) -> Box<[u8]> {
    if base.ends_with(b"/") { read::joined(&[base, name]) } else { read::joined(&[base, b"/", name]) }
}
#[expect(clippy::disallowed_methods, reason = "public path input is validated at this JSON protocol boundary")]
fn paths(paths: &Paths) -> Result<(), Refusal> {
    for (name, bytes) in [
        (b"config".as_slice(), &paths.config),
        (b"data", &paths.data),
        (b"cache", &paths.cache),
        (b"bundle", &paths.bundle),
        (b"claude", &paths.claude),
    ] {
        let p = Path::root().member(name);
        read::absolute(bytes, &p)?;
        if core::str::from_utf8(bytes).is_err() {
            return Err(read::why(&p, b"path must be UTF-8"));
        }
        let suffix = match name {
            b"config" => b"/agent.json".len(),
            b"data" => b"/github-admin.token".len(),
            _ => 0,
        };
        if bytes.len().saturating_add(suffix) > 4096 || bytes.ends_with(b"/..") || bytes::find(bytes, b"/../").is_some()
        {
            return Err(read::why(&p, b"path past its bound"));
        }
    }
    Ok(())
}
fn tls(d: &Document, at: u32, edits: &mut Edits, p: &Path) -> Result<(), Refusal> {
    let mut o = read::object(d, at, p)?;
    for name in [b"server_name".as_slice(), b"trust", b"trust_der", b"identity_file"] {
        if let Some(value) = o.member(d, name) {
            edits.remove(d, value, &p.member(name))?;
        }
    }
    Ok(())
}
fn input_address(d: &Document, at: u32, p: &Path) -> Result<(), Refusal> {
    let mut problems = jig_protocol_config::Problems::empty();
    let value = jig_protocol_config::address::read(d, at, p, &mut problems);
    if let Some(problem) = problems.problems.get(0) {
        return Err(read::kind(&problem.path, problem.what));
    }
    match value {
        Some(_) => Ok(()),
        None => Err(read::why(p, b"wrong value kind")),
    }
}
fn endpoint(d: &Document, at: u32, socket: &[u8], agent: bool, edits: &mut Edits, p: &Path) -> Result<(), Refusal> {
    let at = selected(d, at, b"name", b"openai", false, p)?;
    let number = member(d, at, b"number", p)?;
    if read::integer(d, number, u64::from(u32::MAX), &p.member(b"number"))? != 1 {
        return Err(read::why(&p.member(b"number"), b"openai endpoint must be number 1"));
    }
    let address_at = member(d, at, b"address", p)?;
    if agent {
        let _value = read::text(d, address_at, 4096, &p.member(b"address"))?;
    } else {
        input_address(d, address_at, &p.member(b"address"))?;
    }
    let transport = member(d, at, b"transport", p)?;
    let _value = read::text(d, transport, 32, &p.member(b"transport"))?;
    edits.set(d, address_at, &if agent { text(socket)? } else { address(socket)? }, &p.member(b"address"))?;
    edits.text(d, transport, b"plaintext", &p.member(b"transport"))?;
    tls(d, at, edits, p)
}

fn program_model(agent: bool) -> Result<Box<[u8]>, Refusal> {
    // Match the shipped direct model's window, output and positive fixture prices.
    Ok(read::joined(&[
        b"{\"name\":",
        &text(PROGRAM_MODEL.as_bytes())?,
        br#", "window":1048576,"output":8192,"prices":{"input":1,"cached":1,"output":1,"unit":1000000},"#,
        if agent { br#""reasoning_item":65536,"head":60000,"idle":30000"# } else { br#""endpoint":"fake-claude""# },
        b"}",
    ]))
}

fn program_endpoint(socket: &[u8], agent: bool) -> Result<Box<[u8]>, Refusal> {
    let tail = if agent {
        read::joined(&[
            br#""identity":"plain","connect":10000,"handshake":10000,"models":["#,
            &program_model(true)?,
            b"]",
        ])
    } else {
        bytes::copy_of(br#""connect_ms":10000,"handshake_ms":10000"#)
    };
    Ok(read::joined(&[
        br#"{"name":"fake-claude","number":2,"dialect":2,"provider":"anthropic","program":"claude_code","address":"#,
        &if agent { text(socket)? } else { address(socket)? },
        br#", "transport":"plaintext","headers":[],"#,
        &tail,
        b"}",
    ]))
}

fn program_route(d: &Document, llm: u32, socket: &[u8], edits: &mut Edits, p: &Path) -> Result<(), Refusal> {
    let endpoints = member(d, llm, b"endpoints", p)?;
    append(d, endpoints, &program_endpoint(socket, false)?, edits)?;
    append(d, member(d, llm, b"models", p)?, &program_model(false)?, edits)?;
    let providers = member(d, llm, b"providers", p)?;
    // Init ships OpenAI and demo, leaving provider 2 for Claude's program way.
    // Keep the public provider/model/source declaration in the ordinary schema.
    append(
        d,
        providers,
        &read::joined(&[
            br#"{"name":"anthropic","number":2,"signed_in_plan":false,"manage_usage":"https://claude.ai/settings/usage","models":["#,
            &text(PROGRAM_MODEL.as_bytes())?,
            br#"],"sources":{"programs":["claude_code"]}}"#,
        ]),
        edits,
    )
}

/// Wire welcome peers and a program model in ordinary documents. The shell validates and rebases imported resources first.
pub fn rewrite(temper: &[u8], agent: &[u8], paths: &Paths, peers: &Peers) -> Result<Documents, Refusal> {
    for (name, bytes) in [(b"temper".as_slice(), temper), (b"agent", agent)] {
        if bytes.len() > usize::try_from(crate::DOCUMENT_BYTES).expect("document bound fits") {
            return Err(read::why(&Path::root().member(name), b"past its bound"));
        }
    }
    self::paths(paths)?;
    let github = socket(peers.github, &Path::root().member(b"github"))?;
    let oauth = socket(peers.oauth, &Path::root().member(b"oauth"))?;
    let llm = socket(peers.llm, &Path::root().member(b"llm"))?;
    let forgejo = socket(peers.forgejo, &Path::root().member(b"forgejo"))?;
    if peers.github == peers.oauth
        || peers.github == peers.llm
        || peers.oauth == peers.llm
        || [peers.github, peers.oauth, peers.llm].contains(&peers.forgejo)
    {
        return Err(read::why(&Path::root(), b"peer sockets must be distinct"));
    }
    let d = read::document(temper)?;
    let a = read::document(agent)?;
    duplicates(&d)?;
    duplicates(&a)?;
    let root = Path::root();
    let mut edits = Edits::new();
    let application = member(&d, 0, b"application", &root)?;
    let ap = root.member(b"application");
    let git_program = path(&paths.data, b"demo/bin/git");
    forge(&d, application, &github, &git_program, &mut edits, &ap)?;
    let engine = member(&d, 0, b"engine", &root)?;
    let ep = root.member(b"engine").member(b"llm");
    let llm_at = member(&d, engine, b"llm", &root.member(b"engine"))?;
    sign_in(&d, llm_at, &oauth, &mut edits, &ep)?;
    let endpoints = member(&d, llm_at, b"endpoints", &ep)?;
    endpoint(&d, endpoints, &llm, false, &mut edits, &ep.member(b"endpoints"))?;
    program_route(&d, llm_at, &llm, &mut edits, &ep)?;
    let clients = member(&d, application, b"clients", &ap)?;
    let bundle = member(&d, clients, b"bundle", &ap.member(b"clients"))?;
    if !read::is_null(&d, bundle) {
        let _value = read::text(&d, bundle, 4096, &ap.member(b"clients").member(b"bundle"))?;
    }
    edits.text(&d, bundle, &paths.bundle, &ap.member(b"clients").member(b"bundle"))?;
    worker(&d, &paths.config, &mut edits)?;
    managed(&d, application, &forgejo, &git_program, &mut edits)?;
    let mut agent_edits = Edits::new();
    let endpoints = member(&a, 0, b"endpoints", &root)?;
    endpoint(&a, endpoints, &llm, true, &mut agent_edits, &root.member(b"endpoints"))?;
    append(&a, endpoints, &program_endpoint(&llm, true)?, &mut agent_edits)?;
    let programs = member(&a, 0, b"programs", &root)?;
    let adapter = read::joined(&[
        b"{\"path\":",
        &text(&paths.claude)?,
        b",\"directory\":",
        &text(&path(&paths.data, b"programs"))?,
        b"}",
    ]);
    object_member(&a, programs, b"claude_code", &adapter, &mut agent_edits)?;
    Ok(Documents {
        temper: edits.finish(&d)?,
        agent: agent_edits.finish(&a)?,
        github: forge_document(&github, &paths.data, &d, clients)?,
        oauth: read::joined(&[
            b"{\"listen\":",
            &address(&oauth)?,
            b",\"redirect_port\":1455,\"redirect_path\":\"auth/callback\"}",
        ]),
        llm: model_document(&llm, &paths.data)?,
        forgejo: read::joined(&[
            b"{\"listen\":",
            &address(&forgejo)?,
            b",\"admin_token\":",
            &text(&path(&paths.data, b"forgejo-admin.token"))?,
            br#", "owner":"fake-person","repositories":[{"name":"widget","landing":"main","ci":null}]}"#,
        ]),
    })
}
// Insert at the closing token so edits to existing children remain independent.
fn append(d: &Document, at: u32, bytes: &[u8], edits: &mut Edits) -> Result<(), Refusal> {
    let close = config::after(d, at)
        .or(Err(read::why(&Path::root(), b"invalid container")))?
        .checked_sub(1)
        .expect("container close");
    let value = read::document(bytes)?;
    // A zero-width insertion leaves the original container close to be emitted.
    edits
        .rows
        .push(Edit { first: close, after: close, value: Some(value) })
        .or(Err(read::why(&Path::root(), b"too many rewrites")))
}
fn object_member(d: &Document, at: u32, name: &[u8], value: &[u8], edits: &mut Edits) -> Result<(), Refusal> {
    let p = Path::root().member(name);
    let mut object = read::object(d, at, &p)?;
    if let Some(old) = object.member(d, name) {
        edits.set(d, old, value, &p)
    } else {
        // A fragment is encoded as an object; write strips its outer container.
        append(d, at, &read::joined(&[b"{", &text(name)?, b":", value, b"}"]), edits)
    }
}
fn managed(
    d: &Document,
    application: u32,
    socket: &[u8],
    git_program: &[u8],
    edits: &mut Edits,
) -> Result<(), Refusal> {
    let p = Path::root().member(b"application");
    let origin = text(&read::joined(&[b"http://", socket]))?;
    let forge = read::joined(&[
        br#"{"number":4,"kind":"forgejo","api":"#.as_slice(),
        &address(socket)?,
        b",\"web\":null,\"git\":",
        &origin,
        b",\"git_program\":",
        &text(git_program)?,
        br#", "transport":"plaintext"}"#,
    ]);
    for (name, value) in [
        (b"forges".as_slice(), forge.as_ref()),
        (
            b"identities",
            br#"{"number":4,"forge":4,"login":"temper","author":{"name":"Temper","email":"temper@localhost"}}"#,
        ),
        (b"setup", br#"{"forge":4,"bot":{"login":"temper","email":"temper@localhost"},"mode":"managed"}"#),
    ] {
        append(d, member(d, application, name, &p)?, value, edits)?;
    }
    let limits = member(d, application, b"limits", &p)?;
    let overrides = member(d, limits, b"overrides", &p)?;
    object_member(d, overrides, b"forge.client.repositories", b"4", edits)?;
    object_member(d, overrides, b"forge.repositories", b"4", edits)?;
    // One extra client repository adds two terminal effects to the existing batch.
    object_member(d, overrides, b"forge.output", b"84", edits)?;
    let engine = member(d, 0, b"engine", &Path::root())?;
    let limits = member(d, engine, b"limits", &Path::root())?;
    let overrides = member(d, limits, b"overrides", &Path::root())?;
    object_member(d, overrides, b"accounts.credentials", b"4", edits)?;
    let lines = Decimal::of(u64::from(PROVIDER_LINES));
    object_member(d, overrides, b"documents.lines", lines.as_bytes(), edits)?;
    let workers = member(d, 0, b"workers", &Path::root())?;
    for worker in &read::list(d, workers, 1, &Path::root().member(b"workers"))? {
        let limits = member(d, *worker, b"limits", &Path::root())?;
        let overrides = member(d, limits, b"overrides", &Path::root())?;
        object_member(d, overrides, b"accounts.credentials", b"4", edits)?;
    }
    Ok(())
}
fn forge(
    d: &Document,
    application: u32,
    github: &[u8],
    git_program: &[u8],
    edits: &mut Edits,
    ap: &Path,
) -> Result<(), Refusal> {
    let forges = member(d, application, b"forges", ap)?;
    let fp = ap.member(b"forges");
    let forge = selected(d, forges, b"number", b"2", true, &fp)?;
    let kind = member(d, forge, b"kind", &fp)?;
    if read::text(d, kind, 16, &fp.member(b"kind"))?.as_ref() != b"github" {
        return Err(read::why(&fp.member(b"kind"), b"forge 2 must be GitHub"));
    }
    let origin = read::joined(&[b"http://", github]);
    for name in [b"api".as_slice(), b"web", b"git", b"transport"] {
        let at = member(d, forge, name, &fp)?;
        if !read::is_null(d, at) {
            if name == b"api" {
                input_address(d, at, &fp.member(name))?;
            } else {
                let _text = read::text(d, at, 4096, &fp.member(name))?;
            }
        }
        let value = match name {
            b"api" => address(github)?,
            b"transport" => text(b"plaintext")?,
            _ => text(&origin)?,
        };
        edits.set(d, at, &value, &fp.member(name))?;
    }
    object_member(d, forge, b"git_program", &text(git_program)?, edits)?;
    tls(d, forge, edits, &fp)?;
    Ok(())
}
fn sign_in(d: &Document, llm_at: u32, oauth: &[u8], edits: &mut Edits, ep: &Path) -> Result<(), Refusal> {
    let providers = member(d, llm_at, b"providers", ep)?;
    let pp = ep.member(b"providers");
    let provider = selected(d, providers, b"number", b"1", true, &pp)?;
    let name = member(d, provider, b"name", &pp)?;
    if read::text(d, name, 128, &pp.member(b"name"))?.as_ref() != b"openai" {
        return Err(read::why(&pp.member(b"name"), b"provider 1 must be openai"));
    }
    let sources = member(d, provider, b"sources", &pp)?;
    let sp = pp.member(b"sources").member(b"sign_in");
    let sign_in = member(d, sources, b"sign_in", &pp.member(b"sources"))?;
    registration(d, sign_in, &sp)?;
    let origin = read::joined(&[b"http://", oauth]);
    for (name, suffix) in [
        (b"issuer".as_slice(), b"".as_slice()),
        (b"authorization", b"/api/accounts/authorize"),
        (b"token", b"/api/accounts/oauth/token"),
        (b"revocation", b"/api/accounts/oauth/revoke"),
        (b"keys", b"/.well-known/jwks.json"),
    ] {
        let at = member(d, sign_in, name, &sp)?;
        let old = read::text(d, at, 1024, &sp.member(name))?;
        let expected = read::joined(&[b"https://auth.openai.com", suffix]);
        if old != expected {
            return Err(read::why(&sp.member(name), b"expected the shipped sign-in origin and path"));
        }
        edits.text(d, at, &read::joined(&[&origin, suffix]), &sp.member(name))?;
    }
    let transport = member(d, sign_in, b"transport", &sp)?;
    let _value = read::text(d, transport, 32, &sp.member(b"transport"))?;
    edits.text(d, transport, b"plaintext", &sp.member(b"transport"))?;
    tls(d, sign_in, edits, &sp)?;
    Ok(())
}
fn worker(d: &Document, config: &[u8], edits: &mut Edits) -> Result<(), Refusal> {
    let workers = member(d, 0, b"workers", &Path::root())?;
    let wp = Path::root().member(b"workers");
    let workers = read::list(d, workers, 1, &wp)?;
    if workers.len() != 1 {
        return Err(read::why(&wp, b"expected one standalone worker"));
    }
    let worker = *workers.get(0).expect("one worker");
    let agent_at = member(d, worker, b"agent", &wp)?;
    let program_at = member(d, agent_at, b"program", &wp.member(b"agent"))?;
    let program = read::text(d, program_at, 4096, &wp.member(b"agent").member(b"program"))?;
    read::absolute(&program, &wp.member(b"agent").member(b"program"))?;
    let configuration = member(d, agent_at, b"configuration", &wp.member(b"agent"))?;
    let arguments = member(d, agent_at, b"arguments", &wp.member(b"agent"))?;
    let args = read::list(d, arguments, 1, &wp.member(b"agent").member(b"arguments"))?;
    if args.len() != 3
        || read::text(d, *args.get(0).expect("nonempty argv"), 16, &wp)?.as_ref() != b"internal"
        || read::text(d, *args.get(1).expect("three argv entries"), 16, &wp)?.as_ref() != b"agent"
    {
        return Err(read::why(&wp.member(b"agent").member(b"arguments"), b"expected agent and its configuration"));
    }
    let previous = read::text(d, configuration, 4096, &wp.member(b"agent").member(b"configuration"))?;
    let argument =
        read::text(d, *args.get(2).expect("three argv entries"), 4096, &wp.member(b"agent").member(b"arguments"))?;
    if previous != argument {
        return Err(read::why(
            &wp.member(b"agent").member(b"arguments"),
            b"configuration and argv must name the same agent document",
        ));
    }
    let name = path(config, b"agent.json");
    edits.text(d, configuration, &name, &wp)?;
    edits.set(d, arguments, &read::joined(&[b"[\"internal\",\"agent\",", &text(&name)?, b"]"]), &wp)?;
    Ok(())
}
fn forge_document(socket: &[u8], data: &[u8], d: &Document, clients: u32) -> Result<Box<[u8]>, Refusal> {
    let p = Path::root().member(b"application").member(b"clients").member(b"listener");
    let listener = member(d, clients, b"listener", &p)?;
    let at = member(d, listener, b"socket", &p)?;
    let clients = read::text(d, at, 128, &p.member(b"socket"))?;
    let setup = text(&read::joined(&[b"http://", &clients, b"/forges/2/github/setup"]))?;
    Ok(read::joined(&[b"{\"listen\":", &address(socket)?, b",\"admin_token\":", &text(&path(data, b"github-admin.token"))?,
        br#", "owner":"fake-person","repositories":[{"name":"widget","landing":"main","ci":null}],"github":{"app":{"id":1,"slug":"temper-fake","bot":2,"key":0,"setup_url":"#,&setup,br#"},"accounts":[{"login":"fake-person","id":3,"kind":"user","installation":{"id":1,"repositories":"all"}}]}}"#]))
}
fn model_document(socket: &[u8], data: &[u8]) -> Result<Box<[u8]>, Refusal> {
    Ok(read::joined(&[b"{\"listen\":", &address(socket)?, b",\"key\":", &text(&path(data, b"llm.key"))?,
        b",\"added_keys\":[", &text(&path(data, b"signed-in.key"))?,
        br#"],"tls":null,"models":[{"name":"gpt-6-luna","window":1048576,"output":8192}],"cued":[],"asked":[],"otherwise":["Hello from the local fake model."]}"#]))
}
fn duplicates(d: &Document) -> Result<(), Refusal> {
    for at in 0..d.len() {
        if d.token(at).expect("document token").kind != Kind::ObjectStart {
            continue;
        }
        let p = Path::root();
        let mut object = read::object(d, at, &p)?;
        let end = config::after(d, at).or(Err(read::why(&p, b"invalid object")))?;
        let mut cursor = at.checked_add(1).expect("bounded token");
        for _ in at..end {
            if cursor == end.checked_sub(1).expect("object close") {
                break;
            }
            let key = d.token(cursor).expect("key token");
            let name = d.text(key).expect("key text");
            let _member = object.member(d, name);
            cursor = config::after(d, cursor.checked_add(1).expect("value token"))
                .or(Err(read::why(&p, b"invalid value")))?;
        }
        read::finish(object, d, &p)?;
    }
    Ok(())
}
fn registration(d: &Document, at: u32, p: &Path) -> Result<(), Refusal> {
    const SCOPES: &[&[u8]] =
        &[b"openid", b"profile", b"email", b"offline_access", b"resource.invoke", b"chatgpt.tokens.use.direct"];
    const MUST_GRANT: &[&[u8]] = &[b"chatgpt.tokens.use.direct"];
    for (name, expected) in [
        (b"client".as_slice(), b"dynamic_agent_client".as_slice()),
        (b"name_hint", b"temper"),
        (b"resource", b"https://api.openai.com/v1"),
        (b"redirect_path", b"auth/callback"),
    ] {
        let value = member(d, at, name, p)?;
        if read::text(d, value, 1024, &p.member(name))?.as_ref() != expected {
            return Err(read::why(&p.member(name), b"expected the shipped registration"));
        }
    }
    for (name, expected) in [(b"scopes".as_slice(), SCOPES), (b"must_grant", MUST_GRANT)] {
        let value = member(d, at, name, p)?;
        let values = read::list(d, value, 1, &p.member(name))?;
        if usize::try_from(values.len()).expect("bounded list") != expected.len() {
            return Err(read::why(&p.member(name), b"expected shipped scopes"));
        }
        for (value, wanted) in values.iter().zip(expected) {
            if read::text(d, *value, 1024, &p.member(name))?.as_ref() != *wanted {
                return Err(read::why(&p.member(name), b"expected shipped scopes"));
            }
        }
    }
    let port = member(d, at, b"redirect_port", p)?;
    if read::integer(d, port, u64::from(u16::MAX), &p.member(b"redirect_port"))? != 1455 {
        return Err(read::why(&p.member(b"redirect_port"), b"expected shipped callback 1455"));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
