//! The browser half of recording again, over the native walk (client/journeys.md, 8.1).
//! Like skein's harness (ui-web.md, 14.2), this is ordinary Rust: the clock is
//! supplied by the harness, and only completed downloads are read here.
use crate::Refusal;
use skein_browser::boundary::Key;
use skein_fake_person::{Doing, Face, Find, Found, Next};
use skein_fake_person_browser::{
    BrowserFace,
    harness::{Harness, Names, Outcome, Output, Play, Printed, Sent, Setup},
};
use skein_lib::{Duration, Time};
use skein_ui::Act;
use std::{
    fs,
    path::{Path, PathBuf},
};
use temper_client_shell_web::journey::{self, Control, Step};

/// What `again` starts from, found by `main` (client/journeys.md, 8.1).
pub struct Paths {
    pub temper: PathBuf,
    pub bundle: PathBuf,
    pub chromium: PathBuf,
    pub scratch: PathBuf,
}

/// Record the journey NAME again: walk it, check it, add it.
/// # Errors
/// Refuses an unwalkable journey, a page that stays moving, a missing event,
/// a different walk, an unproved save, or a browser, box or filesystem failure.
pub fn again(journeys: &Path, name: &str, paths: &Paths) -> Result<(), Refusal> {
    let steps = starting(journeys, name)?;
    let downloads = paths.scratch.join("downloads");
    fs::create_dir(&downloads).map_err(|_| crate::io_refusal(&downloads))?;
    let mut args =
        vec!["box".into(), "--fake-backends".into(), "--bundle".into(), paths.bundle.clone().into_os_string()];
    for (flag, directory) in [("--config", "config"), ("--data", "data"), ("--cache", "cache")] {
        args.push(flag.into());
        args.push(paths.scratch.join(directory).into_os_string());
    }
    let mut harness = Harness::new(Setup {
        server: paths.temper.clone(),
        restart: args.clone(),
        args,
        printed: Printed { sent: Sent::Page, prefix: b"http://127.0.0.1:", on: Output::Stdout, names: Names::Url },
        chromium: paths.chromium.clone(),
        downloads: Some(downloads),
        expected: vec![],
        origins: vec![],
    });
    let mut walker = Walker {
        name: name.to_owned(),
        steps,
        at: 0,
        phase: Phase::Start,
        since: None,
        missing_since: None,
        refusal: None,
        recording: None,
    };
    let outcome = harness.run(&mut [&mut walker]);
    if let Some(refusal) = walker.refusal {
        return Err(refusal);
    }
    if let Outcome::Failed { step, report, .. } = outcome {
        return Err(Refusal::Harness { name: name.to_owned(), step: step.saturating_add(1), report });
    }
    let recording = walker.recording.expect("completed walker keeps its end save");
    let new =
        journey::walk(&recording).map_err(|unwalkable| Refusal::Unwalkable { name: name.to_owned(), unwalkable })?;
    if let Some(step) = journey::differs(&walker.steps, &new) {
        return Err(Refusal::WalkDiffers { name: name.to_owned(), step });
    }
    crate::add(journeys, name, &recording)
}

/// Check the old walk before building or starting anything.
/// # Errors
/// Refuses an invalid name, a missing recording or an unwalkable recording.
pub fn starting(journeys: &Path, name: &str) -> Result<Vec<Step>, Refusal> {
    if !crate::named(name) {
        return Err(Refusal::Name(name.to_owned()));
    }
    let path = journeys.join(name).join("recording");
    let recording = fs::read(&path).map_err(|_| crate::io_refusal(&path))?;
    journey::walk(&recording).map_err(|unwalkable| Refusal::Unwalkable { name: name.to_owned(), unwalkable })
}

enum Phase {
    Start,
    Download { after: usize, earlier: Option<Vec<u8>> },
    Gap { earlier: Vec<u8>, until: Time },
    Still { save: Vec<u8> },
    Find { control: Control },
    Finished,
}
struct Walker {
    name: String,
    steps: Vec<Step>,
    at: usize,
    phase: Phase,
    since: Option<Time>,
    missing_since: Option<Time>,
    refusal: Option<Refusal>,
    recording: Option<Vec<u8>>,
}
impl Walker {
    fn step(&self) -> u32 {
        u32::try_from(self.at + 1).expect("walk length fits u32")
    }
    fn save_refusal(&self, refusal: journey::Refusal) -> Refusal {
        Refusal::Save { name: self.name.clone(), step: self.step(), refusal }
    }
    fn save(&mut self, face: &mut BrowserFace<'_>, earlier: Option<Vec<u8>>) {
        self.phase = Phase::Download { after: face.downloads().len(), earlier };
        face.key(Key::Letter { letter: b's', alt: true }, true);
    }
    fn advance(&mut self) {
        self.at += 1;
        self.since = None;
        self.missing_since = None;
        self.phase = Phase::Start;
    }
    fn deadline(&self, now: Time) -> Result<(), Refusal> {
        if let Some(since) = self.missing_since {
            if now >= since.saturating_add(Duration::from_secs(10)) {
                return Err(Refusal::NoControl { name: self.name.clone(), step: self.step() });
            }
        } else if self.since.is_some_and(|since| now >= since.saturating_add(Duration::from_secs(30))) {
            return Err(Refusal::NotStill { name: self.name.clone(), step: self.step() });
        }
        Ok(())
    }
    fn progress(&mut self, face: &mut BrowserFace<'_>, now: Time) -> Result<Next<Box<[u8]>>, Refusal> {
        self.deadline(now)?;
        match std::mem::replace(&mut self.phase, Phase::Finished) {
            Phase::Start => {
                self.since.get_or_insert(now);
                self.save(face, None);
            }
            Phase::Download { after, earlier } => {
                if face.downloads().len() > after {
                    let path = &face.downloads().last().expect("newest completed download").path;
                    let save = fs::read(path).map_err(|error| Refusal::Harness {
                        name: self.name.clone(),
                        step: self.step(),
                        report: format!("cannot read download {}: {error}", path.display()),
                    })?;
                    if let Some(earlier) = earlier {
                        if journey::still(&earlier, &save)
                            .map_err(|unreadable| self.save_refusal(journey::Refusal::Unreadable(unreadable)))?
                        {
                            self.since = None;
                            self.phase = Phase::Still { save };
                        } else {
                            // Each failed comparison starts a fresh pair.
                            self.save(face, None);
                        }
                    } else {
                        self.phase =
                            Phase::Gap { earlier: save, until: now.saturating_add(Duration::from_millis(500)) };
                    }
                } else {
                    self.phase = Phase::Download { after, earlier };
                }
            }
            Phase::Gap { earlier, until } => {
                if now >= until {
                    self.save(face, Some(earlier));
                } else {
                    self.phase = Phase::Gap { earlier, until };
                    return Ok(Next::Wait { until: Some(until) });
                }
            }
            Phase::Still { save } => match &self.steps[self.at] {
                Step::Act { act, event } => {
                    if let Some(control) = journey::control(&save, act, event).map_err(|why| self.save_refusal(why))? {
                        self.phase = Phase::Find { control };
                    } else {
                        self.missing_since.get_or_insert(now);
                        self.save(face, None);
                    }
                }
                Step::Mark => {
                    face.key(Key::Letter { letter: b'm', alt: true }, true);
                    self.advance();
                }
                Step::End => {
                    self.recording = Some(save);
                    return Ok(Next::Done);
                }
            },
            Phase::Find { control } => {
                let find = Find::exactly_named(control.role, &control.name).nth(control.order);
                match face.find(&find) {
                    Found::Node(node) => {
                        let Step::Act { act, .. } = &self.steps[self.at] else {
                            unreachable!("finding an act's control")
                        };
                        let act = copy_act(act);
                        self.advance();
                        return Ok(Next::Do(Doing::Act { node, act }));
                    }
                    Found::Pending => self.phase = Phase::Find { control },
                    Found::Absent => {
                        return Err(Refusal::Harness {
                            name: self.name.clone(),
                            step: self.step(),
                            report: format!("the live page has no {find}"),
                        });
                    }
                }
            }
            Phase::Finished => return Ok(Next::Done),
        }
        Ok(Next::Wait { until: None })
    }
}
impl Play for Walker {
    fn at(&self) -> u32 {
        u32::try_from(self.at).expect("walk length fits u32")
    }
    fn next(&mut self, face: &mut BrowserFace<'_>, now: Time) -> Next<Box<[u8]>> {
        match self.progress(face, now) {
            Ok(next) => next,
            Err(why) => {
                // Done lets the harness close both children before returning the refusal.
                self.refusal = Some(why);
                Next::Done
            }
        }
    }
}
fn copy_act(act: &Act) -> Act {
    match act {
        Act::Press => Act::Press,
        Act::Submit => Act::Submit,
        Act::Toggle => Act::Toggle,
        Act::Input { value } => Act::Input { value: value.clone() },
        Act::Drawing { .. } => unreachable!("walk refuses drawings before starting the harness"),
    }
}
