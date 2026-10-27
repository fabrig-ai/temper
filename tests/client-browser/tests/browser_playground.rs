//! The static page boots its world in a worker and spends that world's printed sign-in link
//! (playground.md, 2, 6 and 9); the server only supplies the bundle.
use skein_fake_person::{Face, Find, Found, Next, Step as PersonStep, Target, Why};
use skein_fake_person_browser::{BrowserFace, Download, Harness, Outcome, Play, Script, Step};
use skein_lib::Time;
use skein_ui::Role;
use skein_ui_web::bridge::log::{self, End};
use std::fs;
use temper_client_browser_world::{ABSENT, Tier, WITHIN};
use temper_client_domain::Address;
use temper_playground_world::story_a;

#[test]
fn the_playground_shows_the_welcome_signed_in_by_its_printed_link() {
    let tier = Tier::playground("playground");
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let script = Script::new(Box::new([
        see(Find::exactly_named(Role::Banner, b"Playground")),
        see(Find::text("Loading the playground…".as_bytes())),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Welcome to temper")),
        see(Find::exactly_named(Role::Button, b"Try the playground")),
        see(Find::exactly_named(Role::Button, b"Sign out")),
        Step::Address(Box::from(b"/welcome".as_slice())),
        Step::Person(PersonStep::Gone { find: Find::text("Loading the playground…".as_bytes()), within: WITHIN }),
        see(Find::text(b"The world: ")),
    ]));
    run_with_figures(&tier, script);
}

#[test]
fn story_a_walks_in_the_browser_playground() {
    let tier = Tier::playground("playground-story-a");
    let mut steps = vec![
        Step::Person(PersonStep::See {
            find: Find::exactly_named(Role::Heading { level: 1 }, b"Welcome to temper"),
            within: WITHIN,
        }),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Option, "100×".as_bytes()) }),
    ];
    steps.extend(story_a::whole().into_vec().into_iter().map(browser_step));
    let mut visit = SessionLog {
        figures: Figures { script: Script::new(steps.into_boxed_slice()), words: None },
        press: Script::new(Box::new([Step::Person(PersonStep::Press {
            find: Find::exactly_named(Role::Button, b"Download the session's log")
                .within(Box::new([Target::Role { role: Role::Banner, name: b"Playground".as_slice().into() }])),
        })])),
        downloaded: None,
        deadline: None,
    };
    let setup = tier.playground_download_setup();
    let directory = setup.downloads.clone().expect("this visit allows downloads");
    assert_eq!(Harness::new(setup).run(&mut [&mut visit]), Outcome::Done);
    check_figures(visit.figures.words.as_deref().expect("frame's figures"));
    let download = visit.downloaded.expect("the kit reported the completed file");
    assert_eq!(download.path.parent(), Some(directory.as_path()), "saved in this test's directory");
    let bytes = fs::read(&download.path).expect("downloaded session log");
    let log = log::read(&bytes).expect("the browser's session log decodes");
    assert!(matches!(log.end, End::Said(_)), "the whole visit fits its session log");
    let build = std::str::from_utf8(&log.build).expect("build's ASCII digest");
    let bundle = std::path::PathBuf::from(std::env::var_os("TEMPER_TEST_PLAYGROUND").expect("built playground"));
    let index = fs::read_to_string(bundle.join("index.html")).expect("playground index");
    assert!(index.contains(&format!("src=\"/bundle/{build}/boot.js\"")), "the log names the shipped bundle");
    assert_eq!(download.name.as_ref(), format!("temper-playground-{build}.log").as_bytes());
    assert!(log.start.start.as_nanos() != 0, "the page read its start wall");
    let first = std::str::from_utf8(&bytes).expect("text log").lines().next().expect("first line");
    assert_eq!(
        first,
        format!("{} {} {build} YQ== {} {}", log::FORMAT, log::VERSION, log.start.seed, log.start.start.as_nanos()),
        "the first line names the format, build, story, seed and start"
    );
    let kept = skein_scratch::paths::target_tmp().join(format!("browser-session-{}.log", log.start.seed));
    fs::write(&kept, &bytes).expect("keep this replay's evidence");
    println!("browser session saved: {}", kept.display());
    println!("browser session header: {first}");
    println!("browser session: seed={}, bytes={}, inputs={}", log.start.seed, bytes.len(), log.inputs.len());
    // This real-tier clock only measures the call; it never enters the replayed world.
    let clock = skein_shell::Clock::new();
    let started = clock.now().now;
    let replay = temper_one_process_world::replay::run(&bytes);
    println!("browser session native replay: {} ns", clock.now().now.as_nanos() - started.as_nanos());
    let replay = replay.expect("the browser's session log replays natively");
    assert!(replay.drained);
    assert!(replay.commits > 0, "the referee observed real durable commits");
}

#[test]
fn a_failed_world_shows_the_failure_page_and_offers_its_log() {
    let tier = Tier::playground("playground-failure");
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let failure = || Box::new([Target::Role { role: Role::Alert, name: b"".as_slice().into() }]) as Box<[Target]>;
    let failed = || Find::exactly_named(Role::Heading { level: 1 }, b"The playground failed.").within(failure());
    let mut script = Script::new(Box::new([
        Step::Person(PersonStep::Go { address: b"/?story=missing-story".as_slice().into() }),
        see(failed()),
        see(Find::text(b"known playground story directory").within(failure())),
        see(Find::exactly_named(Role::Button, b"Download the session's log").within(failure())),
        see(Find::exactly_named(Role::Link, b"Start over").within(failure())),
        Step::Person(PersonStep::Gone { find: Find::text("Loading the playground…".as_bytes()), within: WITHIN }),
        Step::Person(PersonStep::Pause { span: ABSENT }),
        Step::Address(b"/?story=missing-story".as_slice().into()),
        see(failed()),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Link, b"Start over").within(failure()) }),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Welcome to temper")),
        see(Find::exactly_named(Role::Button, b"Try the playground")),
        Step::Address(b"/welcome".as_slice().into()),
        Step::Person(PersonStep::Gone { find: failed(), within: WITHIN }),
    ]));
    assert_eq!(Harness::new(tier.playground_setup()).run(&mut [&mut script]), Outcome::Done);
}

/// Keep the shared script's actions, observations and person-time waits unchanged.
fn browser_step(step: PersonStep<Address>) -> Step {
    Step::Person(match step {
        PersonStep::Go { .. } => {
            panic!("the browser story takes no address moves: a reload would end the world")
        }
        PersonStep::Press { find } => PersonStep::Press { find },
        PersonStep::Type { find, words } => PersonStep::Type { find, words },
        PersonStep::Send { find } => PersonStep::Send { find },
        PersonStep::See { find, within } => PersonStep::See { find, within },
        PersonStep::Gone { find, within } => PersonStep::Gone { find, within },
        PersonStep::Reload => PersonStep::Reload,
        PersonStep::Pause { span } => PersonStep::Pause { span },
    })
}

fn run_with_figures(tier: &Tier, script: Script) {
    let mut story = Figures { script, words: None };
    assert_eq!(Harness::new(tier.playground_setup()).run(&mut [&mut story]), Outcome::Done);
    check_figures(story.words.as_deref().expect("frame's figures"));
}

fn check_figures(bytes: &[u8]) {
    let words = std::str::from_utf8(bytes).expect("figures UTF-8");
    println!("playground figures: {words}");
    let (memory, slice) = words
        .strip_prefix("The world: ")
        .expect("figures prefix")
        .split_once(" MiB, slices up to ")
        .expect("memory and slice figures");
    let (whole, tenth) = memory.split_once('.').expect("memory in tenths of a MiB");
    let whole = whole.parse::<u64>().expect("whole MiB");
    assert_eq!(tenth.len(), 1, "one decimal place for MiB");
    let tenth = tenth.parse::<u8>().expect("tenths of a MiB");
    assert!(whole != 0 || tenth != 0, "the worker reported memory beyond the initial placeholder");
    slice.strip_suffix(" ms").expect("slice units").parse::<u64>().expect("slice milliseconds");
}

/// Read the figures' visible text through the same semantic face as the story.
struct Figures {
    script: Script,
    words: Option<Box<[u8]>>,
}

impl Play for Figures {
    fn next(&mut self, face: &mut BrowserFace<'_>, now: Time) -> Next<Box<[u8]>> {
        match self.script.next(face, now) {
            Next::Done => {}
            next @ (Next::Do(_) | Next::Wait { .. } | Next::Failed { .. }) => return next,
        }
        let find = Find::text(b"The world: ");
        match face.find(&find) {
            Found::Pending => Next::Wait { until: None },
            Found::Absent => Next::Failed { step: self.script.at(), why: Why::Missing { find } },
            Found::Node(node) => {
                self.words = Some(face.seen(node).expect("observed figures node").name.clone());
                Next::Done
            }
        }
    }

    fn at(&self) -> u32 {
        self.script.at()
    }

    fn check_deadline(&mut self, face: &mut BrowserFace<'_>, now: Time) {
        self.script.check_deadline(face, now);
    }
}

/// After the figures, press the frame's key and wait for the kit's completed file.
struct SessionLog {
    figures: Figures,
    press: Script,
    downloaded: Option<Download>,
    deadline: Option<Time>,
}

impl Play for SessionLog {
    fn next(&mut self, face: &mut BrowserFace<'_>, now: Time) -> Next<Box<[u8]>> {
        if self.figures.words.is_none() {
            match self.figures.next(face, now) {
                Next::Done => {}
                next @ (Next::Do(_) | Next::Wait { .. } | Next::Failed { .. }) => return next,
            }
        }
        match self.press.next(face, now) {
            Next::Done => {}
            next @ (Next::Do(_) | Next::Wait { .. } | Next::Failed { .. }) => return next,
        }
        let deadline = *self.deadline.get_or_insert_with(|| now.saturating_add(WITHIN));
        if let Some(download) = face.downloads().first() {
            assert_eq!(face.downloads().len(), 1, "one press saves one log");
            self.downloaded = Some(download.clone());
            Next::Done
        } else {
            assert!(now < deadline, "the session log download completed within the story's wait");
            Next::Wait { until: Some(deadline) }
        }
    }

    fn at(&self) -> u32 {
        self.figures.at() + self.press.at()
    }

    fn check_deadline(&mut self, face: &mut BrowserFace<'_>, now: Time) {
        self.figures.check_deadline(face, now);
        self.press.check_deadline(face, now);
        if let Some(deadline) = self.deadline {
            assert!(now < deadline, "the session log download completed within the story's wait");
        }
    }
}
