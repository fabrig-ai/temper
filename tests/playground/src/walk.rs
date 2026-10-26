//! The browser shell's turn on the in-memory bridge (client/journeys.md, 2).
use std::collections::VecDeque;

use jig_client_shell_web::{adapter, sign_in};
use skein_fake_person::{Doing, Next, Person, Step, TreeFace, Why};
use skein_lib::{Duration, Time, Wall};
use skein_playground::native::Native;
use skein_ui_web::{
    bridge::{Output, Start},
    mapping::Primary,
    page::Page,
    platform::{
        Platform,
        worker::{Heard, Limits, Speed},
    },
    store::Store,
    transport,
};
use temper_client_domain::{Address, address};
use temper_client_shell_web::host::{self, Host, Report};
use temper_client_view::sign_in::Ways;
use temper_playground::{Playground, Story, VISITOR_EXCHANGES, world};

const SLICE_STEPS: u32 = 128;
const SESSION_LOG: u32 = 4_194_304;
const START_WITHIN: Duration = Duration::from_secs(10);
const PATIENCE: Duration = Duration::from_secs(180);

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Failed { step: u32, why: Why },
    Impatient { step: u32, now: Time },
    Stalled { step: u32 },
    Stopped,
    Broken { words: Box<[u8]> },
}

/// One visit: a world, its real web host, and the page's memory and inbox.
pub struct Walk {
    story: &'static Story,
    seed: u64,
    native: Native<Playground>,
    host: Host,
    page: Page<'static>,
    store: Store,
    inbox: VecDeque<Report>,
    path: Box<[u8]>,
}

impl Walk {
    /// The bridge's session log, for replaying a failed visit.
    #[must_use]
    pub fn log(&self) -> Box<[u8]> {
        self.native.log()
    }

    /// The displayed tree, retained for a failed walk's diagnostics.
    #[must_use]
    pub fn tree(&self) -> &skein_ui::Tree {
        self.host.tree()
    }

    /// Start exactly as the page does: wait for standard output, take the
    /// printed link out of the address, and let the host spend its secret.
    #[must_use]
    pub fn new(story: &'static Story, seed: u64, start: Wall) -> Self {
        let mut native = Native::new(
            world(story, seed, start),
            Start { story: story.name.as_bytes().into(), seed, start },
            SLICE_STEPS,
            b"temper-native-walk",
            Limits { exchanges: VISITOR_EXCHANGES, log: SESSION_LOG },
            Speed(1),
        );
        let prefix = format!("http://{}", story.clients());
        let taken = 'link: loop {
            native.run();
            while let Some(heard) = native.heard() {
                match heard {
                    Heard::Line(line) if line.process.as_ref() == b"temper" && line.stream == Output::Standard => {
                        if let Some(path) = line.line.strip_prefix(prefix.as_bytes()) {
                            if let Some(taken) = sign_in::take(
                                &sign_in::Route { path: b"/jig/v1/sign-in/link", home: b"/welcome" },
                                path,
                            ) {
                                break 'link taken;
                            }
                        }
                    }
                    Heard::Stopped | Heard::Failed { .. } => panic!("world ended before its printed sign-in link"),
                    Heard::Nothing | Heard::Report(_) | Heard::Line(_) | Heard::Stamp(_) => {}
                }
            }
            assert!(native.now().0 <= Time::ZERO.saturating_add(START_WITHIN), "printed link within startup patience");
            native.advance(native.wake().expect("world wakes before printing its link"));
        };
        let (now, wall) = native.now();
        let store = Store::new(b"temper.", None);
        let mut host = Host::new(seed, now, wall, Ways::NONE);
        host.start(&taken.address, store.read_all(&native), Some(taken.secret));
        let page = host::page(&host.bounds.tree, Primary::Control);
        Self { story, seed, native, host, page, store, inbox: VecDeque::from([Report::Start]), path: taken.address }
    }

    /// Run the same report/duty/draw/person order for every story.
    pub fn run(&mut self, script: Box<[Step<Address>]>) -> Outcome {
        let mut person = Person::new(script);
        let finish = self.native.now().0.saturating_add(PATIENCE);
        loop {
            self.native.run();
            let (now, wall) = self.native.now();
            if now > finish {
                return Outcome::Impatient { step: person.at(), now };
            }
            self.host.clocks(now, wall);
            while let Some(heard) = self.native.heard() {
                match heard {
                    Heard::Report(report) => {
                        self.inbox.push_back(Report::Heard(adapter::fetched(report)));
                    }
                    Heard::Stopped => return Outcome::Stopped,
                    Heard::Failed { words } => return Outcome::Broken { words },
                    Heard::Nothing | Heard::Line(_) | Heard::Stamp(_) => {}
                }
            }
            if self.host.deadline().is_some_and(|due| due <= now) {
                self.inbox.push_back(Report::Due);
            }
            while let Some(report) = self.inbox.pop_front() {
                self.deliver(report);
            }
            self.host.draw(&mut self.page, &mut Vec::new());
            let before = person.at();
            let until = match person.next(&mut TreeFace::new(self.host.tree()), now) {
                Next::Done => return Outcome::Done,
                Next::Failed { step, why } => return Outcome::Failed { step, why },
                Next::Do(doing) => {
                    match doing {
                        Doing::Act { node, act } => self.inbox.push_back(Report::Act { node, act }),
                        Doing::Go { address } => {
                            self.path = address::print(&address);
                            self.inbox.push_back(Report::Place { path: self.path.clone() });
                        }
                        Doing::Reload => self.reload(),
                    }
                    continue;
                }
                Next::Wait { until } => until,
            };
            // A successful See/Gone consumes an instruction but returns Wait.
            if person.at() != before {
                continue;
            }
            let wake = [
                self.native.wake(),
                self.host.deadline().and_then(|due| self.native.timer(due)),
                until.and_then(|due| self.native.timer(due)),
                self.native.timer(finish.saturating_add(Duration::from_nanos(1))),
            ]
            .into_iter()
            .flatten()
            .min();
            let Some(wake) = wake else {
                return Outcome::Stalled { step: person.at() };
            };
            self.native.advance(wake.max(self.native.browser()));
        }
    }

    fn deliver(&mut self, report: Report) {
        let read_on = matches!(&report, Report::Heard(adapter::Heard::StreamOpened | adapter::Heard::Piece { .. }));
        let duties = self.host.report(report);
        let mut requests = adapter::requests(duties.asked);
        requests.navigate = duties.navigate;
        if let Some(to) = duties.replace {
            self.native.navigate(&to);
        }
        let mut carried = Vec::new();
        transport::carry(requests, &mut self.store, &mut self.native, &mut carried);
        if duties.writes != 0 || !carried.is_empty() {
            self.inbox.push_back(Report::Carried { reports: carried, writes: duties.writes });
        }
        if read_on {
            self.native.read_on();
        }
        if let Some((path, _replace)) = duties.show {
            self.path = path;
        }
    }

    fn reload(&mut self) {
        // A playground reload starts a new visit and retains no page storage
        // (playground.md, 6). Its new host takes the new world's printed link.
        let wall = self.native.now().1;
        *self = Self::new(self.story, self.seed, wall);
    }
}
