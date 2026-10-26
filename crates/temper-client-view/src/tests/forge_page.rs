use super::fixtures;
use crate::forge::{
    bindings::{self, Bindings},
    page,
};
use skein_ui::{Act, Role, State};
use temper_client_domain::{Action, forge};
#[test]
fn adoption_names_the_demo_project_playground() {
    let root = fixtures::root_at_with_demo(
        temper_client_domain::Address::Adopt { project: fixtures::PROJECT },
        &fixtures::domain_limits(),
        b"Project 1",
        Some((fixtures::PROJECT, true)),
    )
    .0;
    for words in [fixtures::words(), fixtures::browser_words()] {
        let mut view = fixtures::view_of(words);
        let tree = fixtures::build(&mut view, &root);
        assert!(has_text(tree, b"Adopt a repository into Playground"));
    }
}
#[test]
fn the_page_names_every_field_a_person_acts_on() {
    let mut view = fixtures::view();
    let root = fixtures::adoption_root();
    let tree = fixtures::build(&mut view, &root);
    let rows: [(Role, &[u8]); 13] = [
        (Role::TextField, b"Forge number"),
        (Role::TextField, b"Owner"),
        (Role::TextField, b"Name"),
        (Role::ChoiceGroup, b"Role"),
        (Role::Choice, b"Owned: temper pushes, opens pull requests and lands"),
        (Role::Choice, b"Fork: temper contributes through a fork"),
        (Role::Choice, b"Context: temper only reads"),
        (Role::Check, b"Make it the project's home"),
        (Role::TextField, b"Branch prefix"),
        (Role::TextField, b"Landing branch"),
        (Role::Check, b"It runs CI"),
        (Role::TextField, b"Checks: gate numbers, separated by spaces or commas"),
        (Role::Button, b"Adopt"),
    ];
    for (role, name) in rows {
        assert!(fixtures::find(tree, role, name).is_some(), "node {role:?}: {name:?}");
    }
    assert!(has_text(tree, b"Adopt a repository into shop"), "heading uses me's name");
}
#[test]
fn a_check_takes_a_toggle_and_refuses_a_press() {
    let mut view = fixtures::view();
    fixtures::build(&mut view, &fixtures::adoption_root());
    for (name, expected) in
        [(b"Make it the project's home".as_slice(), forge::Act::Home), (b"It runs CI".as_slice(), forge::Act::Ci)]
    {
        let node = fixtures::find(view.tree(), Role::Check, name).expect("check");
        assert_eq!(crate::act(&mut view, node, Act::Press), None, "press {name:?}");
        assert_eq!(crate::act(&mut view, node, Act::Toggle), Some(Action::Forge(expected)), "toggle {name:?}");
    }
}
fn form() -> forge::Form {
    let mut form = forge::form(1);
    form.confirmed = true;
    form.texts[forge::Text::Owner.index()].text = Box::from(*b"acme");
    form.texts[forge::Text::Name.index()].text = Box::from(*b"shop");
    form
}
fn tree(form: &forge::Form) -> skein_ui::Tree {
    let words = fixtures::words();
    page_tree(form, &words)
}
fn page_tree(form: &forge::Form, words: &crate::words::Words) -> skein_ui::Tree {
    let mut limits = fixtures::tree_limits();
    limits.text_per_node = 2048;
    limits.text_bytes = 262_144;
    let mut builder = skein_ui::Builder::new(&limits);
    let mut bindings = Bindings::new(limits.bindings, 8, 1);
    page::page(&mut builder, form, Some(b"Shop"), words, &mut bindings, fixtures::tree_limits().text_per_node);
    builder.finish()
}
fn adopted() -> forge::Answer {
    forge::Answer::Adopted(forge::Adopted {
        forge: 1,
        repository: 7,
        owner: Box::from(*b"acme"),
        name: Box::from(*b"shop"),
        role: forge::Role::Owned,
        home: true,
        kinds: forge::Kinds {
            read: true,
            push: true,
            open: true,
            land: true,
            review: true,
            status: true,
            comment: true,
            issue: true,
            branch: true,
        },
        protection: forge::Protection::Unknown,
    })
}
#[test]
fn each_outcome_reads_as_the_designs_sentence() {
    let words = fixtures::words();
    let rows: [(forge::Answer, &[u8], bool); 13] = [
        (adopted(), b"Adopted acme/shop on forge 1 as owned.It is the project's home.temper may: read; push; open pull requests; land; review; set statuses; comment; create issues; create and delete branches.temper's forge user may not read the branch protection: the forge shows it to repository administrators only.", false),
        (forge::Answer::ForgeRefused(forge::Reason::NoSuchForge), b"No forge 1 is configured.", true),
        (forge::Answer::ForgeRefused(forge::Reason::NotFound), b"No repository acme/shop on forge 1 that temper's forge user can see.", true),
        (forge::Answer::ForgeRefused(forge::Reason::NoAccess), b"temper's forge user may not read acme/shop on forge 1.", true),
        (forge::Answer::ForgeRefused(forge::Reason::Refused), b"The forge connector refused it: another project writes acme/shop, the project has a home already, or branches under temper/ are not this deployment's.", true),
        (forge::Answer::ForgeRefused(forge::Reason::AnotherProject), b"Another project writes acme/shop. Adopt it as context to read it, or use the project that writes it.", true),
        (forge::Answer::ForgeRefused(forge::Reason::HomeHeld), b"The project has a home already. Adopt this repository without making it home, or choose a new project.", true),
        (forge::Answer::ForgeRefused(forge::Reason::PrefixOccupied), b"Branches under temper/ are not this deployment's. Choose an unused branch prefix.", true),
        (forge::Answer::ForgeRefused(forge::Reason::InFlight), b"acme/shop is being adopted already; try again when that ends.", true),
        (forge::Answer::ForgeRefused(forge::Reason::TooLarge), b"A field is past the forge's bounds, or the forge's repositories are full.", true),
        (forge::Answer::ForgeRefused(forge::Reason::Unreachable), b"Forge 1 did not answer; nothing was adopted. Try again.", true),
        (forge::Answer::ForgeRefused(forge::Reason::InvalidAnswer), b"Forge 1 answered what temper did not expect; nothing was adopted.", true),
        (forge::Answer::Unknown, b"It is not known whether acme/shop was adopted.", false),
    ];
    for (answer, said, alert) in rows {
        let mut form = form();
        form.outcome = Some(answer.clone());
        let rendered = page_tree(&form, &words);
        let text = fixtures::text(&rendered);
        assert_eq!(&text[b"Adopt a repository into Shop".len()..], said, "outcome {answer:?}");
        assert_eq!(has_role(&rendered, Role::Alert), alert, "refusal alert {answer:?}");
    }
}
#[test]
fn each_kind_of_jig_refusal_uses_jigs_words() {
    let refusal = jig_client::contract::Refusal::Role { held: None };
    for answer in [
        forge::Answer::CoreRefused(refusal.clone()),
        forge::Answer::EntranceRefused(
            jig_client::contract::EntranceRefusal::new(refusal.clone()).expect("entrance refusal"),
        ),
    ] {
        let mut form = form();
        form.outcome = Some(answer.clone());
        let rendered = tree(&form);
        assert_eq!(
            &fixtures::text(&rendered)[b"Adopt a repository into Shop".len()..],
            b"The held role does not permit this actionNo held role was supplied",
            "outcome {answer:?}"
        );
        assert!(has_role(&rendered, Role::Alert), "alert {answer:?}");
    }
}
#[test]
fn typed_fields_reject_invalid_numbers_and_duplicate_checks() {
    for (text, expected) in [
        (b"1".as_slice(), Some(1)),
        (b"65535", Some(65535)),
        (b"0", None),
        (b"65536", None),
        (b"-1", None),
        (b"", None),
    ] {
        assert_eq!(bindings::forge_number(text), expected, "forge {text:?}");
    }
    let rows: [(&[u8], Option<&[u32]>); 7] = [
        (b"", Some(&[])),
        (b"1, 2 3", Some(&[1, 2, 3])),
        (b"1, 1", None),
        (b"0", None),
        (b"x", None),
        (b"4294967296", None),
        (b"1 2 3 4", None),
    ];
    for (text, expected) in rows {
        assert_eq!(bindings::checks(text, 3).as_deref(), expected, "checks {text:?}");
    }
}
#[test]
fn a_field_input_is_typed_and_older_binding_tokens_decode_nothing() {
    let mut bindings = Bindings::new(1, 2, 1);
    let field = bindings.insert(bindings::Meaning::Field(forge::Text::Checks));
    assert_eq!(
        bindings::decode(&bindings, field.token, Act::Input { value: Box::from(*b"1,2") }),
        Some(Action::Forge(forge::Act::Write {
            field: forge::Text::Checks,
            text: Box::from(*b"1,2"),
            typed: forge::Typed::Checks(Some(Box::new([1, 2])))
        }))
    );
    bindings.begin();
    let _new = bindings.insert(bindings::Meaning::Adopt);
    assert_eq!(bindings::decode(&bindings, field.token, Act::Press), None);
}
#[test]
fn adopt_is_disabled_until_a_party_is_confirmed() {
    let mut form = form();
    for (party, disabled) in [(None, true), (Some(73_u64), false), (None, true)] {
        form.confirmed = party.is_some();
        let rendered = tree(&form);
        let button = node_of_role(&rendered, Role::Button);
        assert_eq!(button.states.as_slice().contains(&State::Disabled), disabled, "party {party:?}");
    }
}
#[test]
fn pending_or_out_of_date_disables_adopt() {
    for (pending, served, said) in [
        (
            Some((skein_lib::Token::new(1), skein_lib::RequestProgress::Parked)),
            forge::Served::Yes,
            b"Waiting to send".as_slice(),
        ),
        (
            None,
            forge::Served::OutOfDate,
            b"This temper is out of date for adopting repositories: run the temper the engine runs.".as_slice(),
        ),
    ] {
        let mut form = form();
        form.pending = pending;
        form.served = served;
        let rendered = tree(&form);
        let button = node_of_role(&rendered, Role::Button);
        assert!(button.states.as_slice().contains(&State::Disabled), "{served:?} {pending:?}");
        assert!(fixtures::find(&rendered, Role::Status, said).is_some(), "status {served:?} {pending:?}");
    }
}
fn has_text(tree: &skein_ui::Tree, expected: &[u8]) -> bool {
    for text in fixtures::text(tree).windows(expected.len()) {
        if text == expected {
            return true;
        }
    }
    false
}
fn has_role(tree: &skein_ui::Tree, role: Role) -> bool {
    for node in tree.nodes() {
        if node.role == role {
            return true;
        }
    }
    false
}
fn node_of_role(tree: &skein_ui::Tree, role: Role) -> &skein_ui::Node {
    for node in tree.nodes() {
        if node.role == role {
            return node;
        }
    }
    panic!("missing {role:?}")
}
#[test]
fn the_form_fields_carry_the_domains_values_and_write_counts() {
    let rendered = tree(&form());
    let rows: [(&[u8], &[u8]); 6] = [
        (b"Forge number", b"1"),
        (b"Owner", b"acme"),
        (b"Name", b"shop"),
        (b"Branch prefix", b"temper/"),
        (b"Landing branch", b""),
        (b"Checks: gate numbers, separated by spaces or commas", b""),
    ];
    for (name, expected) in rows {
        let id = fixtures::find(&rendered, Role::TextField, name).expect("field");
        for node in rendered.nodes() {
            if node.id == id && node.name.as_deref() == Some(name) {
                let value = node.value.as_ref().expect("field value");
                assert_eq!((&*value.text, value.written), (expected, 0), "field {name:?}");
            }
        }
    }
}
#[test]
fn each_broken_rule_is_an_invalid_field_with_its_status() {
    let words = fixtures::words();
    let rows: [(forge::Broken, &[u8]); 6] = [
        (forge::Broken::Required, b"Required."),
        (forge::Broken::ForgeNumber, b"A forge number is from 1 to 65535."),
        (forge::Broken::Checks, b"Checks are numbers from 1, each once."),
        (forge::Broken::ContextHome, b"A context repository cannot be the project's home."),
        (forge::Broken::NoCheck, b"Without CI, name at least one check."),
        (forge::Broken::Long { bound: 256 }, b"At most 256 bytes."),
    ];
    for (broken, said) in rows {
        let mut form = form();
        form.texts[forge::Text::Owner.index()].broken = Some(broken);
        let rendered = page_tree(&form, &words);
        assert!(fixtures::find(&rendered, Role::Status, said).is_some(), "status {broken:?}");
        for node in rendered.nodes() {
            if node.role == Role::TextField && node.name.as_deref() == Some(b"Owner") {
                assert!(node.states.as_slice().contains(&State::Invalid), "invalid {broken:?}");
            }
        }
    }
}
#[test]
fn all_roles_take_a_press_or_a_toggle() {
    let mut bindings = Bindings::new(3, 8, 1);
    for role in [forge::Role::Owned, forge::Role::Fork, forge::Role::Context] {
        let binding = bindings.insert(bindings::Meaning::Role(role));
        for (name, act) in [("press", Act::Press), ("toggle", Act::Toggle)] {
            assert_eq!(
                bindings::decode(&bindings, binding.token, act),
                Some(Action::Forge(forge::Act::Choose { role })),
                "{role:?} {name}"
            );
        }
    }
}
#[test]
fn full_bound_repository_names_fit_the_outcome_phrase() {
    let mut form = form();
    let forge::Answer::Adopted(mut adopted) = adopted() else { panic!("adopted fixture") };
    adopted.owner = Box::new([b'o'; 256]);
    adopted.name = Box::new([b'n'; 256]);
    form.outcome = Some(forge::Answer::Adopted(adopted));
    let rendered = tree(&form);
    assert!(has_text(&rendered, b" on forge 1 as owned."));
}

#[test]
fn the_adoption_page_is_titled_with_the_project_me_names() {
    let mut view = fixtures::view();
    let root = fixtures::adoption_page(fixtures::PROJECT, b"shop");
    let tree = fixtures::build(&mut view, &root);
    let title = fixtures::find(tree, Role::Heading { level: 1 }, b"Adopt a repository into shop");
    assert!(title.is_some(), "the title names the project as me names it");
}

#[test]
fn the_adoption_page_for_a_project_me_does_not_name_is_titled_by_its_number() {
    const UNKNOWN_PROJECT: u32 = 7;
    let mut view = fixtures::view();
    let root = fixtures::adoption_page(UNKNOWN_PROJECT, b"shop");
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Heading { level: 1 }, b"Adopt a repository into project 7").is_some());
}

#[test]
fn the_adoption_region_is_named_by_its_heading_and_contains_the_whole_form() {
    for (project, title) in [
        (fixtures::PROJECT, b"Adopt a repository into shop".as_slice()),
        (7, b"Adopt a repository into project 7".as_slice()),
    ] {
        let mut view = fixtures::view();
        let root = fixtures::adoption_page(project, b"shop");
        let tree = fixtures::build(&mut view, &root);
        let nodes = tree.nodes();
        let mut heading = None;
        let mut region = None;
        for (index, node) in nodes.iter().enumerate() {
            if node.role == (Role::Heading { level: 1 }) {
                assert_eq!(node.name.as_deref(), Some(title));
                heading = Some(index);
            }
            if node.role == Role::Region && node.name.as_deref() == Some(title) {
                assert!(region.is_none(), "one region for the form");
                region = Some(index);
            }
        }
        let heading = heading.expect("page heading");
        let at = region.expect("named adoption region");
        assert!(heading < at);
        assert!(nodes[at].styles.as_slice().is_empty());
        let end = at + usize::try_from(nodes[at].size).unwrap();
        assert_eq!(end, nodes.len(), "the region includes everything after the heading slot");
        for (role, name) in [
            (Role::TextField, b"Forge number".as_slice()),
            (Role::TextField, b"Owner"),
            (Role::TextField, b"Name"),
            (Role::TextField, b"Branch prefix"),
            (Role::TextField, b"Landing branch"),
            (Role::TextField, b"Checks: gate numbers, separated by spaces or commas"),
            (Role::ChoiceGroup, b"Role"),
            (Role::Check, b"Make it the project's home"),
            (Role::Check, b"It runs CI"),
            (Role::Button, b"Adopt"),
        ] {
            let mut found = false;
            for node in &nodes[at + 1..end] {
                if node.role == role && node.name.as_deref() == Some(name) {
                    found = true;
                    break;
                }
            }
            assert!(found, "region contains {role:?}: {name:?}");
        }
    }
}

#[test]
fn the_adoption_region_holds_the_form_with_no_card_around_it() {
    let mut view = fixtures::view();
    let root = fixtures::adoption_page(fixtures::PROJECT, b"shop");
    let tree = fixtures::build(&mut view, &root);
    let nodes = tree.nodes();
    let mut region = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Region && node.name.as_deref() == Some(b"Adopt a repository into shop") {
            region = Some(index);
        }
    }
    let at = region.expect("adoption region");
    let end = at + usize::try_from(nodes[at].size).unwrap();
    let mut children = skein_lib::List::with_capacity(nodes[at].size);
    let mut child = at + 1;
    while child < end {
        children.push(child).unwrap();
        child += usize::try_from(nodes[child].size).unwrap();
    }
    assert_eq!(child, end, "direct children fill the region");
    let [fields, roles, ..] = children.as_slice() else { panic!("the form's parts are direct children") };
    assert_eq!(nodes[*fields].role, Role::Slot);
    assert!(nodes[*fields].styles.as_slice().contains(&crate::look::CARD));
    let expected = [b"Forge number".as_slice(), b"Owner", b"Name"];
    let mut names = skein_lib::List::with_capacity(u32::try_from(expected.len()).unwrap());
    for node in &nodes[*fields + 1..*roles] {
        if node.role == Role::TextField {
            names.push(node.name.as_deref().expect("named field")).unwrap();
        }
    }
    assert_eq!(names.as_slice(), expected);
    assert_eq!(nodes[*roles].role, Role::ChoiceGroup);
    assert_eq!(nodes[*roles].name.as_deref(), Some(b"Role".as_slice()));
}
