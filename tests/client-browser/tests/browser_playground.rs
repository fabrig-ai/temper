//! The static page boots its world in a worker and spends that world's printed sign-in link
//! (playground.md, 2, 6 and 9); the server only supplies the bundle.
use skein_fake_person::{Face, Find, Found, Next, Step as PersonStep, Why};
use skein_fake_person_browser::{BrowserFace, Harness, Outcome, Play, Script, Step};
use skein_lib::Time;
use skein_ui::Role;
use temper_client_browser_world::{Tier, WITHIN};

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
    let mut story = Figures { script, words: None };
    assert_eq!(Harness::new(tier.playground_setup()).run(&mut [&mut story]), Outcome::Done);
    let words = std::str::from_utf8(story.words.as_deref().expect("frame's figures")).expect("figures UTF-8");
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
