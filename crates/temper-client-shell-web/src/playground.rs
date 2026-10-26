//! The page's starter and frame (playground.md, 2 and 6). Browser shell code,
//! ordinary Rust under skein's programming-model.md, 10.2.
use super::{Inbox, Platform, Report as ShellReport, Shell, limits, platform, sign_in};
use skein_lib::Wall;
use skein_ui_web::{
    bridge::Start,
    platform::worker::{Heard, Limits, Speed},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

// The session log's bound: past 4 MiB, the log says it was cut.
const LOG_BYTES: u32 = 4 * 1024 * 1024;

thread_local! {
    // Independent of the shell and bridge borrows, including during a panic.
    static INTERVAL: Cell<Option<i32>> = const { Cell::new(None) };
    static PANICKED: Cell<bool> = const { Cell::new(false) };
    static STOP: RefCell<Option<platform::Stop>> = const { RefCell::new(None) };
    static LOG: RefCell<Option<(String, platform::Log)>> = const { RefCell::new(None) };
}

pub(super) enum Report {
    Tick,
    Pause,
    Speed(u32),
    Reset,
    Story(String),
    DownloadLog,
}

pub(super) fn start() {
    install_panic_hook();
    let window = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let query =
        web_sys::UrlSearchParams::new_with_str(&window.location().search().expect("query")).expect("query parameters");
    let story = query.get("story").unwrap_or_else(|| "a".to_owned());
    let inbox = Inbox::new();
    let facts = platform::facts();
    let script = document
        .query_selector("script[type='module'][src]")
        .expect("starter selector")
        .expect("bundle starter")
        .dyn_into::<web_sys::HtmlScriptElement>()
        .expect("script element");
    let directory = web_sys::Url::new_with_base(".", &script.src()).expect("starter directory").href();
    let build = directory.trim_end_matches('/').rsplit('/').next().expect("bundle digest").to_owned();
    let frame = Frame::new(&story, build.clone(), &inbox);
    let platform = Platform::new(
        &directory,
        Start { story: story.into_bytes().into(), seed: facts.seed, start: facts.wall },
        Limits { exchanges: limits::EXCHANGES + 1, log: LOG_BYTES },
        Speed(1),
        Rc::clone(&inbox),
        ShellReport::World,
        || ShellReport::Due,
    );
    STOP.with_borrow_mut(|stop| *stop = Some(platform.stop()));
    LOG.with_borrow_mut(|log| *log = Some((build, platform.log_handle())));
    let mut starter = Starter { starting: Some(Starting { platform, frame, facts }), shell: None };
    // Worker::new can have queued a failure already. The stop handle is kept
    // before installing the turn, which takes those reports synchronously.
    let reports = Rc::clone(&inbox);
    inbox.start(Box::new(move |_| starter.turn(&reports)));
}

struct Starting {
    platform: Platform,
    frame: Frame,
    facts: platform::Facts,
}
struct Starter {
    starting: Option<Starting>,
    shell: Option<Shell>,
}
impl Starter {
    fn turn(&mut self, inbox: &Rc<Inbox<ShellReport>>) {
        if self.shell.is_none() {
            let starting = self.starting.as_mut().expect("waiting for the printed link");
            starting.frame.time(starting.platform.clocks().1);
            let mut linked = false;
            while let Some(report) = inbox.next() {
                match report {
                    ShellReport::World(Heard::Line(line)) if !starting.frame.ended() => {
                        let Some(path) = printed_link(&line.line) else { continue };
                        web_sys::window()
                            .expect("window")
                            .history()
                            .expect("history")
                            .replace_state_with_url(&JsValue::NULL, "", Some(&path))
                            .expect("printed link in the address");
                        linked = true;
                        break;
                    }
                    ShellReport::World(heard) => starting.frame.world(heard, &starting.platform),
                    ShellReport::Playground(report) => starting.frame.report(report, &starting.platform),
                    ShellReport::Start
                    | ShellReport::Due
                    | ShellReport::Act(..)
                    | ShellReport::Carried(..)
                    | ShellReport::Place(_) => {}
                    #[cfg(feature = "record")]
                    ShellReport::Mark | ShellReport::Save => {}
                }
            }
            if linked {
                let mut starting = self.starting.take().expect("one client start");
                (starting.facts.now, starting.facts.wall) = starting.platform.clocks();
                self.shell = Some(Shell::new(starting.platform, &starting.facts, inbox, starting.frame));
                element("playground-loading").set_attribute("hidden", "").expect("hide loading line");
                inbox.raise(ShellReport::Start);
            }
        }
        if let Some(shell) = &mut self.shell {
            shell.turn(inbox);
        }
    }
}

fn printed_link(line: &[u8]) -> Option<String> {
    let line = std::str::from_utf8(line).ok()?;
    if !line.starts_with("http://") || line.chars().any(char::is_whitespace) {
        return None;
    }
    let url = web_sys::Url::new(line).ok()?;
    let path = url.pathname() + &url.search();
    sign_in::take(&super::SIGN_IN, path.as_bytes())?;
    Some(path)
}

pub(super) struct Frame {
    story: String,
    build: String,
    speed: u32,
    paused: bool,
    ended: bool,
    interval: i32,
    _tick: Closure<dyn FnMut()>,
    _listeners: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}
impl Frame {
    fn new(story: &str, build: String, inbox: &Rc<Inbox<ShellReport>>) -> Self {
        element("playground-version").set_text_content(Some(env!("CARGO_PKG_VERSION")));
        let select = element("playground-story").dyn_into::<web_sys::HtmlSelectElement>().expect("story select");
        if story != "a" {
            let option = web_sys::window()
                .expect("window")
                .document()
                .expect("document")
                .create_element("option")
                .expect("story option");
            option.set_attribute("value", story).expect("story value");
            option.set_text_content(Some(story));
            select.append_child(&option).expect("current story");
        }
        select.set_value(story);
        let mut listeners = Vec::new();
        for (id, event, wrap) in [
            ("playground-pause", "click", (|_: web_sys::Event| Report::Pause) as fn(web_sys::Event) -> Report),
            ("playground-reset", "click", |_: web_sys::Event| Report::Reset),
            ("playground-log", "click", |_: web_sys::Event| Report::DownloadLog),
            ("playground-speed", "change", |event: web_sys::Event| {
                Report::Speed(select_value(&event).parse().expect("offered speed"))
            }),
            ("playground-story", "change", |event: web_sys::Event| Report::Story(select_value(&event))),
        ] {
            let inbox = Rc::clone(inbox);
            let listener = Closure::new(move |event| {
                if !PANICKED.get() {
                    inbox.raise(ShellReport::Playground(wrap(event)));
                }
            });
            element(id)
                .add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
                .expect("frame control");
            listeners.push(listener);
        }
        let inbox = Rc::clone(inbox);
        let tick = Closure::new(move || {
            if !PANICKED.get() {
                inbox.raise(ShellReport::Playground(Report::Tick));
            }
        });
        let interval = web_sys::window()
            .expect("window")
            .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 1_000)
            .expect("world time interval");
        INTERVAL.set(Some(interval));
        Self {
            story: story.to_owned(),
            build,
            speed: 1,
            paused: false,
            ended: false,
            interval,
            _tick: tick,
            _listeners: listeners,
        }
    }

    pub(super) fn ended(&self) -> bool {
        self.ended
    }

    pub(super) fn time(&self, wall: Wall) {
        if !self.ended {
            // Integer milliseconds of any u64-nanosecond wall fit a double's
            // exact integers. Date formats that world wall, never Date::now.
            let millis: f64 = (wall.as_nanos() / 1_000_000).to_string().parse().expect("integer milliseconds");
            let date = js_sys::Date::new(&JsValue::from_f64(millis));
            let words: String = date.to_iso_string().into();
            element("playground-time").set_text_content(Some(&words));
        }
    }

    pub(super) fn report(&mut self, report: Report, platform: &Platform) {
        match report {
            Report::DownloadLog => {
                let blob = log_blob(&platform.log()).expect("session log blob");
                download(&blob, &log_name(&self.build)).expect("session log download");
            }
            Report::Reset => reload(&self.story, platform),
            Report::Story(story) => reload(&story, platform),
            Report::Pause if !self.ended => {
                self.paused = !self.paused;
                platform.pace(Speed(if self.paused { 0 } else { self.speed }));
                element("playground-pause").set_text_content(Some(if self.paused { "Resume" } else { "Pause" }));
                element("playground-pause")
                    .set_attribute("aria-pressed", if self.paused { "true" } else { "false" })
                    .expect("pause state");
            }
            Report::Speed(speed) if !self.ended => {
                self.speed = speed;
                platform.pace(Speed(if self.paused { 0 } else { self.speed }));
            }
            Report::Tick | Report::Pause | Report::Speed(_) => {}
        }
    }

    pub(super) fn world(&mut self, heard: Heard, platform: &Platform) {
        match heard {
            Heard::Stopped => {
                platform.end();
                self.end();
                element("playground-status").set_text_content(Some("The world stopped."));
                element("playground-loading").set_attribute("hidden", "").expect("hide loading line");
            }
            Heard::Failed { words } => {
                platform.end();
                self.end();
                show_failure(&String::from_utf8_lossy(&words), Some(&platform.log()), &self.build);
            }
            Heard::Nothing | Heard::Line(_) | Heard::Stamp(_) | Heard::Report(_) => {}
        }
    }

    fn end(&mut self) {
        self.ended = true;
        web_sys::window().expect("window").set_onbeforeunload(None);
        web_sys::window().expect("window").clear_interval_with_handle(self.interval);
        INTERVAL.set(None);
        for id in ["playground-pause", "playground-speed"] {
            element(id).set_attribute("disabled", "").expect("ended control");
        }
    }
}

fn select_value(event: &web_sys::Event) -> String {
    event.target().expect("control target").dyn_into::<web_sys::HtmlSelectElement>().expect("select control").value()
}
fn element(id: &str) -> web_sys::Element {
    web_sys::window().expect("window").document().expect("document").get_element_by_id(id).expect("playground element")
}
fn reload(story: &str, platform: &Platform) {
    let query = web_sys::UrlSearchParams::new().expect("story query");
    query.append("story", story);
    platform.end();
    let window = web_sys::window().expect("window");
    window.set_onbeforeunload(None);
    window.location().set_href(&format!("/?{}", String::from(query.to_string()))).expect("fresh world page");
}
fn install_panic_hook() {
    skein_fail_stop::install();
    let fail_stop = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        PANICKED.set(true);
        STOP.with_borrow(|stop| {
            if let Some(stop) = stop {
                stop.stop();
            }
        });
        if let Some(interval) = INTERVAL.take()
            && let Some(window) = web_sys::window()
        {
            window.clear_interval_with_handle(interval);
        }
        let words = panic.to_string();
        LOG.with_borrow(|log| {
            if let Some((build, log)) = log {
                let file = log.file();
                show_failure(&words, file.as_deref(), build);
            } else {
                show_failure(&words, None, "");
            }
        });
        fail_stop(panic);
    }));
}
fn show_failure(words: &str, file: Option<&[u8]>, build: &str) {
    // The panic hook must not panic again, even if startup failed before it
    // found the page's elements. Failure words are text, never markup.
    if let Some(window) = web_sys::window() {
        window.set_onbeforeunload(None);
        if let Some(document) = window.document() {
            if PANICKED.get()
                && let Some(button) = document.get_element_by_id("playground-log")
            {
                let _disabled = button.set_attribute("disabled", "");
            }
            for id in ["app", "playground-loading"] {
                if let Some(element) = document.get_element_by_id(id) {
                    let _hidden = element.set_attribute("hidden", "");
                }
            }
            if let Some(element) = document.get_element_by_id("playground-failure-words") {
                element.set_text_content(Some(words));
            }
            if let Some(holder) = document.get_element_by_id("playground-failure-log") {
                holder.set_text_content(None);
                if let Some(file) = file {
                    let _offered = offer_failure_log(&document, &holder, file, build);
                }
            }
            if let Some(element) = document.get_element_by_id("playground-failure") {
                let _visible = element.remove_attribute("hidden");
            }
        }
    }
}

fn log_name(build: &str) -> String {
    format!("temper-playground-{build}.log")
}

fn log_blob(bytes: &[u8]) -> Result<web_sys::Blob, JsValue> {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
}

fn offer_failure_log(
    document: &web_sys::Document,
    holder: &web_sys::Element,
    file: &[u8],
    build: &str,
) -> Result<(), JsValue> {
    let blob = log_blob(file)?;
    let name = log_name(build);
    let button = document.create_element("button")?;
    button.set_attribute("type", "button")?;
    button.set_text_content(Some("Download the session's log"));
    // A panic leaves the interrupted inbox turn borrowed. This separate
    // listener owns a snapshot and works without the inbox or shell.
    let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        let _downloaded = download(&blob, &name);
    });
    button.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
    holder.append_child(&button)?;
    // One failure page per visit; the document owns this listener's lifetime.
    listener.forget();
    Ok(())
}

fn download(blob: &web_sys::Blob, name: &str) -> Result<(), JsValue> {
    let window = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let anchor = document.create_element("a")?.dyn_into::<web_sys::HtmlAnchorElement>()?;
    let url = web_sys::Url::create_object_url_with_blob(blob)?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    // Let the browser start the download before releasing this click's URL.
    let revoke = Closure::once_into_js(move || {
        let _revoked = web_sys::Url::revoke_object_url(&url);
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 1_000)?;
    Ok(())
}
