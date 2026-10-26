pub mod support;

use skein_fake_person::{Face, Find, Found, Step, TreeFace};
use skein_lib::Queue;
use skein_ui::{Builder, Limits, Role, Run, View};
use support::person::chatgpt_source_ready;

// Page and two accounts, each with a summary and two sources of Item/Text/Status/Text.
const LIMITS: Limits = Limits {
    nodes: 25,
    depth: 6,
    text_per_node: 64,
    text_bytes: 512,
    drawing_bytes: 0,
    styles_per_node: 0,
    patches: 150,
    bindings: 0,
    slots: 0,
    markdown_depth: 0,
    scene_marks: 0,
    scene_bytes: 0,
};
const ACCOUNT: &[u8] = b"ChatGPT \xc2\xb7 OpenAI \xc2\xb7 held by Owner";

fn status(b: &mut Builder, name: &[u8]) {
    b.open(Role::Status);
    b.name(name);
    b.text(Run::Plain, name);
    b.close();
}

fn account(b: &mut Builder, name: &[u8], sign_in: &[u8]) {
    b.open(Role::Disclosure);
    b.name(name);
    // A Ready summary and API key must not stand in for the sign-in source.
    status(b, b"Ready");
    b.open(Role::List);
    for (kind, state) in [(b"1. ChatGPT sign-in \xc2\xb7 ".as_slice(), sign_in), (b"2. API key \xc2\xb7 ", b"Ready")] {
        b.open(Role::Item);
        b.open(Role::Text);
        b.run(Run::Plain, kind);
        b.close();
        status(b, state);
        b.close();
    }
    b.close();
    b.close();
}

fn fixture(sign_in: &[u8]) -> View {
    let mut view = View::new(&LIMITS);
    let mut b = view.builder();
    b.open(Role::Page);
    account(&mut b, ACCOUNT, sign_in);
    account(&mut b, b"Other account", b"Ready");
    b.close();
    view.finish(b, 1, &mut Queue::with_capacity(LIMITS.patches));
    view
}

fn ready(heading: &[u8]) -> Find {
    let Step::See { find, .. } = chatgpt_source_ready(heading) else { panic!("source readiness expectation") };
    find
}

#[test]
fn a_ready_sign_in_source_is_found_inside_its_account() {
    let view = fixture(b"Ready");
    let found = TreeFace::new(view.tree()).find(&ready(ACCOUNT));
    assert!(matches!(found, Found::Node(_)));
}

#[test]
fn ready_is_scoped_to_the_sign_in_source() {
    let view = fixture(b"Needs sign-in");
    assert_eq!(TreeFace::new(view.tree()).find(&ready(ACCOUNT)), Found::Absent);
}

#[test]
fn ready_is_scoped_to_the_exact_account() {
    let view = fixture(b"Ready");
    assert_eq!(TreeFace::new(view.tree()).find(&ready(b"ChatGPT")), Found::Absent);
}
