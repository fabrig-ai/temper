//! GitHub's setup visit, after jig's claim (client/forge.md, 9.1).
use crate::engine::Engine;
use alloc::boxed::Box;
use core::num::NonZeroU64;
use skein_forge_setup::host;
use skein_http::{Method, server::Call};
use skein_lib::{Decimal, Token, Writer};

fn number(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    let mut number = 0_u64;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        number = number.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(number)
}
fn split(target: &[u8]) -> (&[u8], &[u8]) {
    match skein_lib::bytes::find(target, b"?") {
        Some(at) => (target.get(..at).expect("query"), target.get(at.saturating_add(1)..).expect("query")),
        None => (target, b""),
    }
}
#[derive(Clone, Copy)]
enum Route {
    Setup,
    Manifest,
    Redirect,
}
fn forge(path: &[u8]) -> Option<(u16, Route)> {
    let path = path.strip_prefix(b"/forges/")?;
    for (suffix, route) in [
        (b"/github/setup".as_slice(), Route::Setup),
        (b"/github/manifest", Route::Manifest),
        (b"/github/redirect", Route::Redirect),
    ] {
        if let Some(digits) = path.strip_suffix(suffix) {
            return Some((u16::try_from(number(digits)?).ok()?, route));
        }
    }
    None
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => byte.checked_sub(b'0'),
        b'a'..=b'f' => byte.checked_sub(b'a')?.checked_add(10),
        b'A'..=b'F' => byte.checked_sub(b'A')?.checked_add(10),
        _ => None,
    }
}
fn value(bytes: &[u8]) -> Option<Box<[u8]>> {
    let mut escapes = 0_usize;
    for byte in bytes {
        if *byte == b'%' {
            escapes = escapes.checked_add(1)?;
        }
    }
    let mut writer = Writer::new(bytes.len().checked_sub(escapes.checked_mul(2)?)?);
    let mut at = 0_usize;
    for _ in 0..bytes.len() {
        if at == bytes.len() {
            break;
        }
        let byte = *bytes.get(at)?;
        let decoded = if byte == b'%' {
            let hi = hex(*bytes.get(at.checked_add(1)?)?)?;
            let lo = hex(*bytes.get(at.checked_add(2)?)?)?;
            at = at.checked_add(2)?;
            hi.checked_mul(16)?.checked_add(lo)?
        } else if byte == b'+' {
            b' '
        } else {
            byte
        };
        if !(b'!'..=b'~').contains(&decoded) {
            return None;
        }
        writer.put(core::slice::from_ref(&decoded)).ok()?;
        at = at.checked_add(1)?;
    }
    Some(writer.finish())
}
struct Query {
    installation: Option<NonZeroU64>,
    action: Box<[u8]>,
    state: Option<Box<[u8]>>,
}
fn query(bytes: &[u8]) -> Option<Query> {
    let mut installation = None;
    let mut action = None;
    let mut state = None;
    let mut rest = Some(bytes);
    for _ in 0_u32..3 {
        let Some(bytes) = rest.take() else {
            break;
        };
        let field = match skein_lib::bytes::find(bytes, b"&") {
            Some(at) => {
                rest = Some(bytes.get(at.checked_add(1)?..)?);
                bytes.get(..at)?
            }
            None => bytes,
        };
        let eq = skein_lib::bytes::find(field, b"=")?;
        let name = value(field.get(..eq)?)?;
        let bytes = value(field.get(eq.checked_add(1)?..)?)?;
        match name.as_ref() {
            b"installation_id" if installation.is_none() => installation = Some(NonZeroU64::new(number(&bytes)?)?),
            b"setup_action" if action.is_none() => action = Some(bytes),
            b"state" if state.is_none() && !bytes.is_empty() => state = Some(bytes),
            _ => return None,
        }
    }
    if rest.is_some() {
        return None;
    }
    let action = action?;
    match action.as_ref() {
        b"install" | b"update" if installation.is_some() => {}
        b"request" if installation.is_none() => {}
        _ => return None,
    }
    Some(Query { installation, action, state })
}
fn fallback(forge: u16) -> Box<[u8]> {
    let digits = Decimal::of(u64::from(forge));
    let mut writer =
        Writer::new(b"/forges/connect?forge=".len().checked_add(digits.as_bytes().len()).expect("bounded fallback"));
    writer.put(b"/forges/connect?forge=").expect("fallback room");
    writer.put(digits.as_bytes()).expect("forge room");
    writer.finish()
}
fn escaped(byte: &u8) -> &[u8] {
    match byte {
        b'&' => b"&amp;",
        b'"' => b"&quot;",
        b'<' => b"&lt;",
        b'>' => b"&gt;",
        _ => core::slice::from_ref(byte),
    }
}
const HEAD: &[u8] = b"<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>temper</title><style>";
const STYLE: &[u8] = b":root{color-scheme:light dark;font-family:system-ui,sans-serif}body{max-width:42rem;margin:3rem auto;padding:0 1rem;line-height:1.5}main{padding:1.5rem;border:1px solid;border-radius:.5rem}a,button{display:inline-block;padding:.6rem 1rem;border:1px solid;border-radius:.3rem;font:inherit}a{color:inherit;text-decoration:none}button{cursor:pointer}";
const MANIFEST_HIDDEN: &[u8] = b".js body{display:none}";
// Submit in a task after load: an earlier submit can replace this history entry in Chromium.
const MANIFEST_SCRIPT: &[u8] = b"document.documentElement.className='js';addEventListener('pageshow',event=>{if(event.persisted)document.documentElement.classList.remove('js')});if(performance.getEntriesByType('navigation')[0]?.type==='back_forward')document.documentElement.classList.remove('js');else addEventListener('load',()=>setTimeout(()=>document.forms[0].submit(),0))";
/// SHA-256 of the shell's sole style, pinned against the rendered page by a unit test.
pub(crate) const PAGE_STYLE_HASH: &[u8] = b"Ya3+uu96WxkZ2SGOjLBRaen9XHM8Pr9IIXNSrVIdNv0=";
/// SHA-256 of the form page's sole style, pinned against the rendered page by a unit test.
pub(crate) const MANIFEST_STYLE_HASH: &[u8] = b"M+/mGdSBiyK4G1SjsI0ULT8eCqVWM1EruKC3biZrspQ=";
/// SHA-256 of the form page's sole script, pinned against the rendered page by a unit test.
pub(crate) const MANIFEST_SCRIPT_HASH: &[u8] = b"JrFcwlKHdmmuH1N1ZfBSAzAIt5hrhzBSBaqsKQjvCmQ=";
/// The form posting GitHub's manifest to `target`, submitted by its one script and
/// with no external resources; attributes escape. Refuses a page past `max`.
pub(super) fn form(target: &[u8], manifest: &[u8], back: Option<&[u8]>, forge: u16, max: u32) -> Option<Box<[u8]>> {
    let digits = Decimal::of(u64::from(forge));
    let (back_prefix, back) = match back {
        Some(back) => (b"".as_slice(), back),
        None => (b"/forges/connect?forge=".as_slice(), digits.as_bytes()),
    };
    let parts = [
        HEAD,
        STYLE,
        MANIFEST_HIDDEN,
        b"</style><script>",
        MANIFEST_SCRIPT,
        "</script></head><body><h1>temper</h1><main><h2>temper’s App, on GitHub</h2><form method=\"post\" action=\""
            .as_bytes(),
        b"\"><input type=\"hidden\" name=\"manifest\" value=\"",
        b"\"><button type=\"submit\">Continue to GitHub</button></form><p><a href=\"",
        back_prefix,
        b"\">Back to temper</a></p></main></body></html>",
    ];
    let values = [b"".as_slice(), b"", b"", b"", b"", target, manifest, b"", back, b""];
    let mut length = 0_usize;
    for (part, value) in parts.iter().zip(values) {
        length = length.checked_add(part.len())?;
        for byte in value {
            length = length.checked_add(escaped(byte).len())?;
        }
    }
    if length > usize::try_from(max).ok()? {
        return None;
    }
    let mut writer = Writer::new(length);
    for (part, value) in parts.iter().zip(values) {
        writer.put(part).expect("measured form");
        for byte in value {
            writer.put(escaped(byte)).expect("measured form attribute");
        }
    }
    Some(writer.finish())
}
pub(super) fn page(
    clients: &mut crate::clients::Clients,
    call: Token,
    status: u16,
    heading: &[u8],
    why: &[u8],
    back: &[u8],
) {
    let parts =
        [HEAD, STYLE, b"</style></head><body><h1>temper</h1><main><h2>", heading, b"</h2><p>", why, b"</p><a href=\""];
    let suffix = b"\">Back to temper</a></main></body></html>";
    let mut length = suffix.len();
    for part in parts {
        length = length.checked_add(part.len()).expect("bounded page");
    }
    for byte in back {
        length = length.checked_add(escaped(byte).len()).expect("bounded escaped return");
    }
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).expect("measured page");
    }
    for byte in back {
        writer.put(escaped(byte)).expect("measured return");
    }
    writer.put(suffix).expect("measured suffix");
    clients.github_answer(call, status, None, &writer.finish());
}
#[expect(clippy::manual_map, reason = "the step subset uses explicit matches")]
pub(crate) fn busy(clients: &mut crate::clients::Clients, call: Token, target: &[u8]) {
    let back = match forge(split(target).0) {
        Some((forge, _)) => Some(fallback(forge)),
        None => None,
    };
    page(
        clients,
        call,
        503,
        b"temper is busy; try again.",
        b"Start again from temper.",
        back.as_deref().unwrap_or(b"/forges"),
    );
}
pub(super) fn ended(engine: &mut Engine, call: Token, forge: u16) {
    page(
        &mut engine.clients,
        call,
        410,
        b"This link has ended",
        b"It was used already, it expired, or temper restarted since. Start again from temper.",
        &fallback(forge),
    );
}
fn route(engine: &mut Engine, call: Token, head: Call) {
    let (path, bytes) = split(&head.target);
    let Some((forge, route)) = forge(path) else {
        page(&mut engine.clients, call, 404, b"Forge not found", b"Start again from temper.", b"/forges");
        return;
    };
    let mut configured = false;
    for config in &engine.workers.forges {
        if config.number == forge && config.kind == crate::settings::Kind::GitHub {
            configured = true;
            break;
        }
    }
    if !configured {
        page(&mut engine.clients, call, 404, b"Forge not found", b"Start again from temper.", &fallback(forge));
        return;
    }
    if head.method != Method::Get {
        page(&mut engine.clients, call, 405, b"Use GET", b"Open this link with GET.", &fallback(forge));
        return;
    }
    if bytes.len() > usize::try_from(engine.clients.query_bytes).expect("query bound") {
        bad_request(engine, call, forge);
        return;
    }
    match route {
        Route::Manifest | Route::Redirect => {
            manifest_visit(engine, call, forge, route, bytes);
            return;
        }
        Route::Setup => {}
    }
    let parsed = if bytes.len() <= usize::try_from(engine.clients.query_bytes).expect("query bound") {
        query(bytes)
    } else {
        None
    };
    let Some(query) = parsed else {
        page(
            &mut engine.clients,
            call,
            400,
            b"This visit could not be read",
            b"Start again from temper.",
            &fallback(forge),
        );
        return;
    };
    if query.action.as_ref() == b"request" {
        let Some(state) = query.state else {
            ended(engine, call, forge);
            return;
        };
        let Some(back) = super::public::take_back(engine, forge, &state) else {
            ended(engine, call, forge);
            return;
        };
        page(&mut engine.clients, call, 200, b"GitHub sent a request, not an installation", "Only an owner of an organization can install temper’s App there, and temper can’t wait for an owner to approve it yet: an installation approved later isn’t used. Install it on your own account, or on an organization you own.".as_bytes(), back.back.as_deref().unwrap_or(&fallback(forge)));
        return;
    }
    let installation = query.installation.expect("validated install or update");
    let Some(state) = query.state else {
        if super::lifecycle::accepted(engine, forge).contains(&installation.get()) {
            engine.clients.github_answer(call, 303, Some(b"/forges"), b"");
        } else {
            ended(engine, call, forge);
        }
        return;
    };
    // The host alone authenticates/consumes the state; this map holds presentation only.
    let back = match super::public::take_back(engine, forge, &state) {
        Some(return_to) => return_to.back,
        None => None,
    };
    if engine.setup.busy(forge) && engine.setup.waiting.room() > 0 {
        // The forge's one run, as the reconcile after a manifest's Connected, ends first.
        let until = engine.env.now.saturating_add(engine.setup.life);
        let waiting = super::Waiting { forge, call, installation, state, back, until };
        engine.setup.waiting.push(waiting);
        return;
    }
    accept(engine, call, forge, installation, state, back);
}
fn accept(
    engine: &mut Engine,
    call: Token,
    forge: u16,
    installation: NonZeroU64,
    state: Box<[u8]>,
    back: Option<Box<[u8]>>,
) {
    if engine.setup.busy(forge)
        || engine.setup.runs.len() == engine.setup.env.limits.runs
        || engine.setup.public.len() == engine.setup.public.capacity()
    {
        ended(engine, call, forge);
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine.setup.public.insert(run, super::public::Caller::Accept { forge, call, back }).expect("public room checked");
    engine
        .setup
        .staged
        .insert(run, host::Plan::Accept { forge, installation, state: Some(state) })
        .expect("staged room");
    super::request(engine, run);
}
fn bad_request(engine: &mut Engine, call: Token, forge: u16) {
    page(
        &mut engine.clients,
        call,
        400,
        b"This visit could not be read",
        b"Start again from temper.",
        &fallback(forge),
    );
}
struct ManifestQuery {
    state: Box<[u8]>,
    code: Option<Box<[u8]>>,
}
fn manifest_query(bytes: &[u8], redirect: bool) -> Option<ManifestQuery> {
    let mut state = None;
    let mut code = None;
    let mut rest = Some(bytes);
    for _ in 0_u32..2 {
        let Some(bytes) = rest.take() else { break };
        let field = match skein_lib::bytes::find(bytes, b"&") {
            Some(at) => {
                rest = Some(bytes.get(at.checked_add(1)?..)?);
                bytes.get(..at)?
            }
            None => bytes,
        };
        let eq = skein_lib::bytes::find(field, b"=")?;
        let name = value(field.get(..eq)?)?;
        let bytes = value(field.get(eq.checked_add(1)?..)?)?;
        if bytes.is_empty() {
            return None;
        }
        match name.as_ref() {
            b"state" if state.is_none() => state = Some(bytes),
            b"code" if redirect && code.is_none() => code = Some(bytes),
            _ => return None,
        }
    }
    if rest.is_some() || redirect && code.is_none() {
        return None;
    }
    Some(ManifestQuery { state: state?, code })
}
fn manifest_visit(engine: &mut Engine, call: Token, forge: u16, route: Route, bytes: &[u8]) {
    let redirect = match route {
        Route::Redirect => true,
        Route::Manifest => false,
        Route::Setup => unreachable!("manifest visits"),
    };
    let Some(ManifestQuery { state, code }) = manifest_query(bytes, redirect) else {
        bad_request(engine, call, forge);
        return;
    };
    if !redirect {
        let Some(page) = engine.setup.page(forge, &state, engine.env.now) else {
            ended(engine, call, forge);
            return;
        };
        let mut web = b"https://github.com".as_slice();
        for config in &engine.workers.forges {
            if config.number == forge {
                if let Some(address) = &config.web {
                    web = address;
                }
                break;
            }
        }
        engine.clients.github_manifest_answer(call, page, web);
        return;
    }
    let live = engine.setup.take_page(forge, &state, engine.env.now);
    let back = match super::public::take_back(engine, forge, &state) {
        Some(back) => back.back,
        None => None,
    };
    if !live
        || super::lifecycle::busy(engine, forge)
        || engine.setup.runs.len() == engine.setup.env.limits.runs
        || engine.setup.public.len() == engine.setup.public.capacity()
    {
        ended(engine, call, forge);
        return;
    }
    let run = super::lifecycle::next_run(&mut engine.setup.lifecycle);
    engine
        .setup
        .public
        .insert(run, super::public::Caller::Redirect { forge, call, back })
        .expect("public room checked");
    engine
        .setup
        .staged
        .insert(run, host::Plan::Convert { forge, code: skein_lib::Secret::new(code.expect("redirect code")), state })
        .expect("staged room");
    super::request(engine, run);
}
fn room(engine: &Engine) -> bool {
    engine.events.room() >= 4
        && engine.setup.room()
        && engine.store.room()
        && engine.forge.room(2)
        && engine.notes.room() > 0
}
pub(super) fn advance(engine: &mut Engine) {
    for _ in 0..engine.setup.waiting.len() {
        if !room(engine) {
            break;
        }
        let waiting = engine.setup.waiting.pop().expect("waiting count");
        if !engine.setup.busy(waiting.forge) {
            let super::Waiting { forge, call, installation, state, back, .. } = waiting;
            accept(engine, call, forge, installation, state, back);
        } else if engine.env.now < waiting.until {
            engine.setup.waiting.push(waiting);
        } else {
            ended(engine, waiting.call, waiting.forge);
        }
    }
    for _ in 0..engine.clients.github_calls.len() {
        if !room(engine) {
            break;
        }
        let (call, head) = engine.clients.github_calls.pop().expect("route count");
        route(engine, call, head);
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::disallowed_methods, reason = "ordinary tests inspect rendered HTTP answers")]
    use super::*;
    use crate::tests::fixtures::http::Peer;
    use skein_lib::Duration;

    fn contains(bytes: &[u8], needle: &[u8]) -> bool {
        skein_lib::bytes::find(bytes, needle).is_some()
    }
    fn hash(page: &[u8], start: &[u8], end: &[u8]) -> Box<[u8]> {
        let first = skein_lib::bytes::find(page, start).unwrap().checked_add(start.len()).unwrap();
        let last = skein_lib::bytes::find(page, end).unwrap();
        let digest = skein_lib::sha256::digest(page.get(first..last).unwrap());
        let alphabet = skein_lib::base64::Alphabet::Standard;
        let size = skein_lib::base64::encoded_len(alphabet, u32::try_from(digest.len()).unwrap()).unwrap();
        let mut writer = Writer::new(usize::try_from(size).unwrap());
        skein_lib::base64::encode(alphabet, &digest, &mut writer).unwrap();
        writer.finish()
    }
    fn service() -> crate::Service {
        super::super::tests::routing_service(8, Duration::from_secs(3600))
    }
    fn pump(engine: &mut Engine, peer: &mut Peer) {
        for _ in 0_u32..4 {
            peer.pump(engine, false);
            advance(engine);
        }
        peer.pump(engine, false);
    }
    fn shell(response: &[u8], status: &[u8], heading: &[u8], back: &[u8]) {
        assert!(response.starts_with(status), "{}", core::str::from_utf8(response).unwrap());
        for part in [
            HEAD,
            b"<h2>",
            heading,
            b"Back to temper</a>",
            back,
            b"Cache-Control: no-store",
            b"Referrer-Policy: no-referrer",
            b"Content-Type: text/html; charset=utf-8",
        ] {
            assert!(contains(response, part), "missing {}", core::str::from_utf8(part).unwrap());
        }
        let policy =
            [b"Content-Security-Policy: default-src 'none'; style-src 'sha256-".as_slice(), PAGE_STYLE_HASH, b"'\r\n"]
                .concat();
        assert!(contains(response, &policy));
        assert_eq!(hash(response, b"<style>", b"</style>").as_ref(), PAGE_STYLE_HASH);
        assert!(!contains(response, b"<script>"));
        let body = skein_lib::bytes::find(response, b"\r\n\r\n").unwrap().checked_add(4).unwrap();
        let name = core::str::from_utf8(heading).unwrap();
        super::super::tests::measure_page(name, response.len().checked_sub(body).unwrap());
    }
    #[test]
    fn errors_and_ended_visits_use_the_shell_and_fallback() {
        for (method, target, status, heading, back) in [
            (
                b"GET".as_slice(),
                b"/forges/1/github/setup".as_slice(),
                b"HTTP/1.1 400".as_slice(),
                b"This visit could not be read".as_slice(),
                b"href=\"/forges/connect?forge=1\"".as_slice(),
            ),
            (b"GET", b"/forges/no/github/setup", b"HTTP/1.1 404", b"Forge not found", b"href=\"/forges\""),
            (
                b"GET",
                b"/forges/2/github/setup",
                b"HTTP/1.1 404",
                b"Forge not found",
                b"href=\"/forges/connect?forge=2\"",
            ),
            (b"POST", b"/forges/1/github/setup", b"HTTP/1.1 405", b"Use GET", b"href=\"/forges/connect?forge=1\""),
            (
                b"GET",
                b"/forges/1/github/setup?setup_action=request&state=unknown",
                b"HTTP/1.1 410",
                b"This link has ended",
                b"href=\"/forges/connect?forge=1\"",
            ),
        ] {
            let mut service = service();
            let engine = service.engine.as_mut().unwrap();
            let mut peer = Peer::new(engine, method, target, &[], None);
            pump(engine, &mut peer);
            shell(peer.received(), status, heading, back);
        }
    }
    #[test]
    fn full_route_queue_uses_the_busy_shell_with_known_or_unknown_forge() {
        for (target, back) in [
            (b"/forges/1/github/setup".as_slice(), b"href=\"/forges/connect?forge=1\"".as_slice()),
            (b"/forges/no/github/setup", b"href=\"/forges\""),
        ] {
            let mut service = service();
            let engine = service.engine.as_mut().unwrap();
            engine.clients.github_calls = skein_lib::Queue::with_capacity(0);
            let mut peer = Peer::new(engine, b"GET", target, &[], None);
            pump(engine, &mut peer);
            shell(peer.received(), b"HTTP/1.1 503", b"temper is busy; try again.", back);
        }
    }
    #[test]
    fn request_uses_the_shell_and_escapes_the_held_back_address() {
        for (back, escaped) in [
            (Some(b"/forges?x=\"<>&y=1".as_slice()), b"href=\"/forges?x=&quot;&lt;&gt;&amp;y=1\"".as_slice()),
            (None, b"href=\"/forges/connect?forge=1\""),
        ] {
            let mut service = service();
            let engine = service.engine.as_mut().unwrap();
            super::super::public::hold(engine, 1, Box::from(b"held".as_slice()), back.map(Box::from));
            let mut peer =
                Peer::new(engine, b"GET", b"/forges/1/github/setup?setup_action=request&state=held", &[], None);
            pump(engine, &mut peer);
            shell(peer.received(), b"HTTP/1.1 200", b"GitHub sent a request, not an installation", escaped);
        }
    }
    #[test]
    fn accepted_without_back_uses_the_shell_and_with_back_keeps_its_redirect() {
        for back in [None, Some(b"/forges?first=1&next=2".as_slice())] {
            let mut service = service();
            let engine = service.engine.as_mut().unwrap();
            let mut peer = Peer::new(engine, b"GET", b"/forges/1/github/setup", &[], None);
            // Stop at the queued call, before the route decides it, to exercise Accept's terminal answer.
            peer.pump(engine, false);
            let (call, _) = engine.clients.github_calls.pop().unwrap();
            let run = Token::new(1001);
            engine
                .setup
                .public
                .insert(run, super::super::public::Caller::Accept { forge: 1, call, back: back.map(Box::from) })
                .unwrap();
            super::super::public::ended(
                engine,
                run,
                &temper_engine_domain::SetupEnd::Done { forge: 1, outcome: temper_engine_domain::SetupOutcome::Done },
            );
            pump(engine, &mut peer);
            if back.is_some() {
                assert!(peer.received().starts_with(b"HTTP/1.1 303"));
                assert!(contains(peer.received(), b"Location: /forges?first=1&next=2\r\n"));
                assert!(peer.received().ends_with(b"temper has the installation; you may close this tab."));
            } else {
                shell(peer.received(), b"HTTP/1.1 200", b"GitHub is connected", b"href=\"/forges\"");
                assert!(contains(peer.received(), b"temper has the installation; you may close this tab."));
            }
        }
    }
    #[test]
    fn the_manifest_form_escapes_its_attributes_and_refuses_a_page_past_its_bound() {
        let target = b"http://forge.test/settings/apps/new?state=s&x=\"<>";
        let manifest = br#"{"name":"temper-abc"}"#;
        let back = b"/forges?first=\"<>&next=2";
        let page = form(target, manifest, Some(back), 1, 4096).expect("form fits");
        assert!(contains(&page, b"action=\"http://forge.test/settings/apps/new?state=s&amp;x=&quot;&lt;&gt;\""));
        assert!(contains(&page, b"value=\"{&quot;name&quot;:&quot;temper-abc&quot;}\""));
        assert!(contains(&page, b"href=\"/forges?first=&quot;&lt;&gt;&amp;next=2\""));
        assert!(contains(&page, b"<button type=\"submit\">Continue to GitHub</button>"));
        assert!(contains(&page, "<h2>temper’s App, on GitHub</h2>".as_bytes()));
        assert!(page.starts_with(b"<!doctype html><html lang=\"en\">"));
        assert!(
            skein_lib::bytes::find(&page, b"<script>").unwrap() < skein_lib::bytes::find(&page, b"<body>").unwrap(),
            "the script hides the page before the body is parsed"
        );
        let fits = u32::try_from(page.len()).unwrap();
        assert!(form(target, manifest, Some(back), 1, fits).is_some());
        assert!(form(target, manifest, Some(back), 1, fits - 1).is_none());
    }
    #[test]
    fn the_manifest_script_hash_matches_the_form_page() {
        let page = form(b"http://forge.test/settings/apps/new?state=s", b"{}", None, 1, 4096).unwrap();
        assert_eq!(hash(&page, b"<script>", b"</script>").as_ref(), MANIFEST_SCRIPT_HASH);
        assert_eq!(hash(&page, b"<style>", b"</style>").as_ref(), MANIFEST_STYLE_HASH);
        assert!(contains(&page, b"href=\"/forges/connect?forge=1\""));
        let mut service = service();
        let engine = service.engine.as_mut().unwrap();
        engine.setup.hold(1, Box::from(b"held".as_slice()), page);
        let mut peer = Peer::new(engine, b"GET", b"/forges/1/github/manifest?state=held", &[], None);
        pump(engine, &mut peer);
        assert!(peer.received().starts_with(b"HTTP/1.1 200"));
        let policy = [
            b"Content-Security-Policy: default-src 'none'; form-action http://127.0.0.1:3000; script-src 'sha256-"
                .as_slice(),
            MANIFEST_SCRIPT_HASH,
            b"'; style-src 'sha256-",
            MANIFEST_STYLE_HASH,
            b"'\r\n",
        ]
        .concat();
        assert!(contains(peer.received(), &policy));
        assert!(contains(peer.received(), "<h2>temper’s App, on GitHub</h2>".as_bytes()));
        assert!(contains(peer.received(), b"href=\"/forges/connect?forge=1\""));
    }
}
