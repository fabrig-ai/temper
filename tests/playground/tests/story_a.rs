use skein_fake_person::{Find, Step};
use skein_lib::{Duration, Wall};
use skein_ui::Role;
use skein_ui_web::bridge::log::{self, End};
use temper_client_shell_web::limits;
use temper_playground::{A, VISITOR_ANSWER, VISITOR_EXCHANGES, VISITOR_PIECE};
use temper_playground_world::story_a;
use temper_playground_world::walk::{Outcome, Walk};

const START: Wall = Wall::from_nanos(1_700_000_000_000_000_000);
const SEED: u64 = 7;
const WELCOME_WITHIN: Duration = Duration::from_secs(40);

#[test]
fn the_visitor_signs_in_by_the_printed_link_and_the_welcome_shows() {
    let script = Box::new([Step::See {
        find: Find::exactly_named(Role::Button, b"Try the playground"),
        within: WELCOME_WITHIN,
    }]);
    let mut walk = Walk::new(&A, SEED, START);
    assert_eq!(walk.run(script), Outcome::Done);
    walk.close();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/welcome.log");
    let log = walk.log();
    if std::env::var("TEMPER_PLAYGROUND_RECORD").as_deref() == Ok("1") {
        std::fs::create_dir_all(fixture.parent().expect("fixture directory")).expect("create fixture directory");
        std::fs::write(&fixture, &log).expect("record welcome session log");
    } else {
        let expected = std::fs::read(&fixture).expect("welcome fixture; record with TEMPER_PLAYGROUND_RECORD=1");
        assert!(log.as_ref() == expected, "welcome log differs; record with TEMPER_PLAYGROUND_RECORD=1");
    }
}

#[test]
fn the_visitors_limits_are_the_web_clients() {
    assert_eq!(VISITOR_EXCHANGES, limits::EXCHANGES + 1);
    assert_eq!(VISITOR_ANSWER, limits::BODY_BYTES);
    assert_eq!(VISITOR_PIECE, limits::PIECE);
}

#[test]
fn story_a_lands_the_csv_change_and_answers_the_other_two() {
    let mut walk = Walk::new(&A, SEED, START);
    let outcome = walk.run(story_a::whole());
    assert_eq!(outcome, Outcome::Done, "{:?}", walk.tree());
    let stamp = walk.stamp();
    let bytes = walk.log();
    let log = log::read(&bytes).expect("the completed story's session log decodes");
    assert!(matches!(log.end, End::Said(_)), "the whole story fits its session log");
    println!(
        "Story A: world={} ns, steps={}, session={} bytes, inputs={}",
        stamp.instant.as_nanos(),
        stamp.step,
        bytes.len(),
        log.inputs.len()
    );
}
