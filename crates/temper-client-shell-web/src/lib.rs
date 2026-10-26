//! The client host on every target and the browser's shell on wasm32.
#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
use jig_client_shell_web::{adapter, sign_in};
#[cfg(target_arch = "wasm32")]
use skein_ui::{Act, NodeId};
#[cfg(target_arch = "wasm32")]
use skein_ui_web::{
    fetch, history,
    inbox::Inbox,
    page::{Page, web::Dom},
    platform,
    store::Store,
    timer, title, transport, wasm_bindgen,
};
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
pub mod host;
#[cfg(any(not(target_arch = "wasm32"), feature = "record"))]
pub mod journey;
pub mod limits;
pub mod records;
#[cfg(target_arch = "wasm32")]
enum Report {
    Start,
    #[cfg(feature = "record")]
    Mark,
    #[cfg(feature = "record")]
    Save,
    Due,
    Act(NodeId, Act),
    Fetch(fetch::Report),
    Carried(Vec<transport::Carried>, u32),
    Place(Box<[u8]>),
}
#[cfg(target_arch = "wasm32")]
struct Shell {
    host: host::Host,
    #[cfg(feature = "record")]
    recorder: Option<journey::Recorder>,
    page: Rc<RefCell<Page<'static>>>,
    dom: Dom,
    platform: platform::Browser<Report>,
    store: Store,
    history: history::web::History,
    timer: timer::web::Timer,
    title: title::web::Title,
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    if !skein_ui_web::restart::Restart::new(
        3,
        skein_lib::Duration::from_secs(60),
        "temper-restarts",
        "The application failed.",
    )
    .install("app")
    {
        return;
    }
    // Replace the printed secret's address before starting anything that can send.
    let secret = sign_in::web::take_from_page(&sign_in::Route { path: b"/jig/v1/sign-in/link", home: b"/welcome" });
    let window = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let ways = document
        .query_selector("meta[name=\"temper-sign-in\"]")
        .expect("sign-in selector")
        .and_then(|meta| meta.get_attribute("content"))
        .map(|content| {
            let origin = window.location().origin().expect("origin");
            (content, origin)
        });
    let parsed_ways = ways.as_ref().map_or(temper_client_view::sign_in::Ways::NONE, |(content, origin)| {
        temper_client_view::sign_in::Ways::parse(content, origin.as_bytes())
    });
    let mount = document.get_element_by_id("app").expect("bundle mount");
    let facts = platform::facts();
    let path = window.location().pathname().expect("path") + &window.location().search().expect("query");
    #[cfg(debug_assertions)]
    assert_ne!(path.split('?').next(), Some("/test/panic"), "the test address panics at start");
    let inbox = Inbox::new();
    #[cfg(feature = "record")]
    record_keys(&document, &inbox);
    let platform =
        platform::Browser::new(fetch::web::Fetch::new(limits::EXCHANGES + 1, Rc::clone(&inbox), Report::Fetch));
    let store = Store::new(b"temper.", None);
    let mut host = host::Host::new(facts.seed, facts.now, facts.wall, parsed_ways);
    #[cfg(feature = "record")]
    let mut forges = Vec::new();
    host.forge_sign_in(forge_sign_in(
        &document,
        #[cfg(feature = "record")]
        &mut forges,
    ));
    let stored = store.read_all(&platform);
    #[cfg(feature = "record")]
    let recorder = Some(journey::Recorder::new(&journey::Facts {
        seed: facts.seed,
        now: facts.now,
        wall: facts.wall,
        primary: facts.primary,
        ways: ways.map(|(content, origin)| (content.into_bytes().into(), origin.into_bytes().into())),
        forges,
        path: path.as_bytes().into(),
        secret: secret.clone(),
        stored: stored.clone(),
    }));
    host.start(path.as_bytes(), stored, secret);
    let page = Rc::new(RefCell::new(host::page(&host.bounds.tree, facts.primary)));
    let dom = Dom::new(mount, &page, &inbox, Report::Act, &[]);
    let history = history::web::History::new(Rc::clone(&inbox), Report::Place);
    let timer = timer::web::Timer::new(Rc::clone(&inbox), || Report::Due);
    let mut shell = Shell {
        host,
        #[cfg(feature = "record")]
        recorder,
        page,
        dom,
        platform,
        store,
        history,
        timer,
        title: title::web::Title::default(),
    };
    inbox.start(Box::new(move |inbox| shell.turn(inbox)));
    inbox.raise(Report::Start);
}
#[cfg(target_arch = "wasm32")]
impl Shell {
    fn turn(&mut self, inbox: &Inbox<Report>) {
        let (now, wall) = platform::clocks();
        #[cfg(feature = "record")]
        if let Some(recorder) = &mut self.recorder {
            recorder.turn(now, wall);
        }
        self.host.clocks(now, wall);
        let mut new_page = false;
        while let Some(report) = inbox.next() {
            let read_on =
                matches!(&report, Report::Fetch(fetch::Report::StreamOpened | fetch::Report::StreamPiece { .. }));
            let report = match report {
                Report::Start => host::Report::Start,
                #[cfg(feature = "record")]
                Report::Mark => {
                    if let Some(recorder) = &mut self.recorder {
                        recorder.mark(&self.page.borrow());
                    }
                    continue;
                }
                #[cfg(feature = "record")]
                Report::Save => {
                    self.save_recording(false);
                    continue;
                }
                Report::Due => {
                    self.timer.fired();
                    host::Report::Due
                }
                Report::Act(node, act) => host::Report::Act { node, act },
                Report::Place(path) => host::Report::Place { path },
                Report::Fetch(report) => host::Report::Heard(adapter::fetched(report)),
                Report::Carried(reports, writes) => host::Report::Carried { reports, writes },
            };
            #[cfg(feature = "record")]
            let duties = if let Some(recorder) = &mut self.recorder {
                recorder.report(&mut self.host, &self.page.borrow(), report)
            } else {
                self.host.report(report)
            };
            #[cfg(not(feature = "record"))]
            let duties = self.host.report(report);
            #[cfg(feature = "record")]
            if duties.departs() {
                self.save_recording(true);
            }
            let mut requests = adapter::requests(duties.asked);
            requests.navigate = duties.navigate;
            if requests.navigate.is_some() {
                // This acknowledged departure erases its finished request before
                // navigating. The previous turn's close warning is already stale.
                web_sys::window().expect("window").set_onbeforeunload(None);
            }
            if let Some(to) = duties.replace {
                let window = web_sys::window().expect("window");
                window.set_onbeforeunload(None);
                window
                    .location()
                    .replace(std::str::from_utf8(&to).expect("jig address is UTF-8"))
                    .expect("replace authorization address");
            }
            let mut carried = Vec::new();
            transport::carry(requests, &mut self.store, &mut self.platform, &mut carried);
            if duties.writes != 0 || !carried.is_empty() {
                inbox.raise(Report::Carried(carried, duties.writes));
            }
            if read_on {
                self.platform.fetch.read_on();
            }
            if let Some((path, replace)) = duties.show {
                self.history.show(&path, replace);
                new_page = !replace;
            }
        }
        let mut ops = Vec::new();
        self.host.draw(&mut self.page.borrow_mut(), &mut ops);
        self.dom.apply(&ops);
        let document = web_sys::window().expect("window").document().expect("document");
        // Nodes can be reused by a later page: remove this shell's previous
        // tab policy before applying it to the current forge links.
        for _ in 0..temper_client_view::forge_sign_in::FORGES {
            let Some(link) =
                document.query_selector("a[data-temper-forge-sign-in]").expect("previous forge link selector")
            else {
                break;
            };
            for name in ["target", "rel", "data-temper-forge-sign-in"] {
                link.remove_attribute(name).expect("previous forge link policy");
            }
        }
        // These are the page-owned Forgejo sign-in links. The semantic link
        // supplies its checked href; the browser owns opening a new tab.
        for _ in 0..temper_client_view::forge_sign_in::FORGES {
            let Some(link) =
                document.query_selector("a[href$='/user/oauth2/temper']:not([target])").expect("forge link selector")
            else {
                break;
            };
            link.set_attribute("data-temper-forge-sign-in", "").expect("forge link policy marker");
            link.set_attribute("target", "_blank").expect("link target");
            link.set_attribute("rel", "noopener noreferrer").expect("link relation");
        }
        self.finish(&ops, new_page, now);
    }
    fn finish(&mut self, ops: &[skein_ui_web::page::Op], new_page: bool, now: skein_lib::Time) {
        let window = web_sys::window().expect("window");
        let document = window.document().expect("document");
        let heading = document.query_selector("main h1").expect("heading selector");
        let words = heading.as_ref().and_then(|element| element.text_content()).unwrap_or_default();
        // Session storage survives reload, but closing the tab loses its requests.
        let closing_loses = self.store.read_all(&self.platform).iter().any(|record| record.key.len() == 16);
        self.title.set(words.as_bytes(), b"temper", b"en", closing_loses);
        if new_page {
            window.scroll_to_with_x_and_y(0.0, 0.0);
            if !ops.iter().any(|op| matches!(op, skein_ui_web::page::Op::Focus { .. })) {
                use wasm_bindgen::JsCast;
                if let Some(target) = heading.or_else(|| document.query_selector("main").expect("main selector")) {
                    target.set_attribute("tabindex", "-1").expect("focus target");
                    target.unchecked_ref::<web_sys::HtmlElement>().focus().expect("focus mounted target");
                }
            }
        }
        self.history.restore();
        let deadline = self.host.deadline();
        #[cfg(feature = "record")]
        if let Some(recorder) = &mut self.recorder {
            recorder.deadline(deadline);
        }
        self.timer.set(deadline, now);
    }
    #[cfg(feature = "record")]
    fn save_recording(&mut self, departing: bool) {
        if departing {
            if let Some(recorder) = self.recorder.take() {
                save_recording(&recorder.end(&self.page.borrow()));
            }
        } else if let Some(recorder) = &self.recorder {
            save_recording(&recorder.save(&self.page.borrow()));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn forge_sign_in(
    document: &web_sys::Document,
    #[cfg(feature = "record")] raw: &mut Vec<Box<[u8]>>,
) -> temper_client_view::forge_sign_in::Forges {
    let mut facts = temper_client_view::forge_sign_in::Forges::new();
    let mut selector = String::from("meta[name='temper-forge-sign-in']");
    for _ in 0..temper_client_view::forge_sign_in::FORGES {
        let Some(meta) = document.query_selector(&selector).expect("forge meta selector") else { break };
        if let Some(content) = meta.get_attribute("content") {
            facts.read(&content);
            #[cfg(feature = "record")]
            raw.push(content.into_bytes().into());
        }
        selector.push_str(" ~ meta[name='temper-forge-sign-in']");
    }
    facts
}

#[cfg(all(target_arch = "wasm32", feature = "record"))]
fn record_keys(document: &web_sys::Document, inbox: &Rc<Inbox<Report>>) {
    use wasm_bindgen::{JsCast, closure::Closure};
    let inbox = Rc::clone(inbox);
    let listener = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
        if !event.alt_key() || !event.shift_key() {
            return;
        }
        let report = match event.code().as_str() {
            "KeyM" => Report::Mark,
            "KeyS" => Report::Save,
            _ => return,
        };
        event.prevent_default();
        event.stop_immediate_propagation();
        if !event.repeat() {
            inbox.raise(report);
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    document
        .add_event_listener_with_callback_and_add_event_listener_options(
            "keydown",
            listener.as_ref().unchecked_ref(),
            &options,
        )
        .expect("recording keys listener");
    listener.forget();
}

#[cfg(all(target_arch = "wasm32", feature = "record"))]
fn save_recording(bytes: &[u8]) {
    use wasm_bindgen::{JsCast, closure::Closure};
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).expect("recording blob");
    let url = web_sys::Url::create_object_url_with_blob(&blob).expect("recording URL");
    let window = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let anchor = document.create_element("a").expect("recording anchor").unchecked_into::<web_sys::HtmlAnchorElement>();
    anchor.set_href(&url);
    anchor.set_download("journey.recording");
    anchor.click();
    // Give the browser a later task to start the download before releasing its URL.
    let revoke = Closure::once_into_js(move || web_sys::Url::revoke_object_url(&url).expect("revoke recording URL"));
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 1_000)
        .expect("recording URL cleanup timer");
}
