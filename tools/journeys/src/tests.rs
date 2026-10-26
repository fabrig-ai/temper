use crate::{
    JOURNEYS,
    print::{Mark, SCRIPT, beside, pages},
};
use skein_scratch::Scratch;
use skein_ui::NodeId;
use std::{fs, path::Path};
use temper_client_shell_web::journey;

#[test]
fn the_committed_journeys_are_their_print() {
    let scratch = Scratch::new("temper-client-journeys-committed");
    let committed = Path::new(JOURNEYS);
    let stale = crate::reprint(committed, scratch.path())
        .expect("reprint committed journeys; run cargo run -p temper-client-journeys");
    assert!(stale.is_empty(), "stale journeys: {stale:?}; run cargo run -p temper-client-journeys");
    let committed_pages = printed_files(committed);
    let reprinted_pages = printed_files(scratch.path());
    for path in committed_pages.union(&reprinted_pages) {
        let expected = fs::read(committed.join(path)).ok();
        let actual = fs::read(scratch.path().join(path)).ok();
        assert!(
            expected.is_some() && actual.is_some(),
            "{} is missing or unreadable; run cargo run -p temper-client-journeys",
            path.display()
        );
        assert_eq!(expected, actual, "{} differs; run cargo run -p temper-client-journeys", path.display());
    }
    for file in ["skein-ui.css", "journey.js"] {
        let reprinted = fs::read(scratch.path().join(file)).expect("reprinted file beside the journeys");
        assert_eq!(
            fs::read(committed.join(file)).ok().as_deref(),
            Some(reprinted.as_slice()),
            "{file} differs; run cargo run -p temper-client-journeys"
        );
    }
}

fn printed_files(root: &Path) -> std::collections::BTreeSet<std::path::PathBuf> {
    let mut files = std::collections::BTreeSet::new();
    for entry in fs::read_dir(root).expect("journeys directory") {
        let entry = entry.expect("journey entry");
        if !entry.file_type().expect("journey type").is_dir() {
            continue;
        }
        for page in fs::read_dir(entry.path()).expect("journey directory") {
            let page = page.expect("page entry");
            if crate::printed_page(&page.path()) {
                files.insert(Path::new(&entry.file_name()).join(page.file_name()));
            }
        }
    }
    files
}

#[test]
fn a_name_outside_the_pattern_is_refused() {
    let scratch = Scratch::new("temper-client-journeys-name");
    for name in ["Welcome", "1-welcome", "welcome_set", ""] {
        assert!(!crate::named(name), "invalid name: {name:?}");
        assert!(matches!(crate::add(scratch.path(), name, b""), Err(crate::Refusal::Name(refused)) if refused == name));
    }
    assert!(crate::named("welcome-set-up"));
    assert!(matches!(crate::add(scratch.path(), "welcome-set-up", b""), Err(crate::Refusal::Proof { .. })));
}

#[test]
fn an_unreadable_recording_is_refused_by_add() {
    let scratch = Scratch::new("temper-client-journeys-unreadable");
    assert!(matches!(
        crate::add(scratch.path(), "welcome", b"not a recording\n"),
        Err(crate::Refusal::Proof { name, refusal: journey::Refusal::Unreadable(_) }) if name == "welcome"
    ));
    assert!(!scratch.path().join("welcome").exists());
}

fn mark(html: &str, heading: &str, acted: Option<u32>) -> Mark {
    Mark {
        html: html.to_owned(),
        heading: heading.as_bytes().to_vec(),
        acted: acted.into_iter().map(NodeId).collect(),
        lead: journey::Lead::Press,
    }
}

#[test]
fn a_page_breaks_inside_each_tag_before_its_close() {
    let printed = pages(&[mark("<div data-node=\"1\"><p data-node=\"2\">Hi</p></div>", "", None)]);
    let document = &printed[0].document;
    let body = document.split_once("<body\n>").expect("body starts").1.split_once("</body\n>").expect("body ends").0;
    assert_eq!(body, "<div id=\"app\"\n><p\n>Hi</p\n></div\n>");
    assert!(document.starts_with("<!doctype html>\n<html lang=\"en\"\n>"));
    assert!(document.ends_with("</html\n>\n"));
}

#[test]
fn the_acted_control_leads_to_the_next_page() {
    let mut first = mark(
        "<div data-node=\"1\"><h1 data-node=\"2\">Welcome</h1><button data-node=\"3\">Set up temper</button><a data-node=\"5\">Begin</a></div>",
        "Welcome",
        Some(3),
    );
    first.acted.push(NodeId(5));
    let printed = pages(&[first, mark("<div data-node=\"4\">Forge</div>", "Forge", None)]);
    assert!(printed[0].document.contains("<button data-journey-next=\"02.html\"\n>Set up temper</button\n>"));
    assert!(printed[0].document.contains("<a data-journey-next=\"02.html\"\n>Begin</a\n>"));
    for page in printed {
        assert!(!page.document.contains("data-node"));
    }
}

#[test]
fn a_page_names_its_neighbours() {
    let printed = pages(&[mark("<div></div>", "", None), mark("<div></div>", "", None), mark("<div></div>", "", None)]);
    assert_eq!(printed.iter().map(|page| page.name.as_str()).collect::<Vec<_>>(), ["01.html", "02.html", "03.html"]);
    assert!(!printed[0].document.contains("rel=\"prev\""));
    assert!(printed[0].document.contains("<link rel=\"next\" href=\"02.html\"\n>"));
    assert!(printed[1].document.contains("<link rel=\"prev\" href=\"01.html\"\n>"));
    assert!(printed[1].document.contains("<link rel=\"next\" href=\"03.html\"\n>"));
    assert!(printed[2].document.contains("<link rel=\"prev\" href=\"02.html\"\n>"));
    assert!(!printed[2].document.contains("rel=\"next\""));
}

#[test]
fn a_dialog_is_printed_open() {
    let printed = pages(&[mark(
        "<div data-node=\"1\"><dialog data-node=\"2\" aria-label=\"Choice\">Choose</dialog></div>",
        "",
        None,
    )]);
    assert!(printed[0].document.contains("<dialog open aria-label=\"Choice\"\n>Choose</dialog\n>"));
}

#[test]
fn the_title_is_the_main_heading_then_temper() {
    let printed = pages(&[mark("<div></div>", "Welcome & more", None), mark("<div></div>", "", None)]);
    assert!(printed[0].document.contains("<title\n>Welcome &amp; more · temper</title\n>"));
    assert!(printed[1].document.contains("<title\n>temper</title\n>"));
}

#[test]
fn the_stylesheets_resolve_in_the_checkout() {
    let printed = pages(&[mark("<div></div>", "", None)]);
    let stylesheets: Vec<&str> = printed[0]
        .document
        .split("<link rel=\"stylesheet\" href=\"")
        .skip(1)
        .map(|rest| rest.split_once('"').expect("stylesheet href closes").0)
        .collect();
    assert_eq!(
        stylesheets,
        [
            "../skein-ui.css",
            "../../../../../crates/temper-client-shell-web/temper.css",
            "../../../../../crates/temper-client-shell-web/task-pages.css",
            "../overlay.css",
        ]
    );
    for stylesheet in &stylesheets[1..3] {
        // Neither the journeys root nor a journey need exist before the first print.
        let mut path = Path::new(JOURNEYS).join("example");
        for component in Path::new(stylesheet).components() {
            if component == std::path::Component::ParentDir {
                assert!(path.pop(), "stylesheet stays in the checkout");
            } else {
                path.push(component);
            }
        }
        assert!(path.is_file(), "stylesheet exists: {}", path.display());
    }
}

#[test]
fn beside_writes_the_kits_and_the_journeys_files_and_keeps_an_overlay() {
    let scratch = Scratch::new("temper-client-journeys-beside");
    beside(scratch.path()).expect("write beside the journeys");
    let overlay = scratch.path().join("overlay.css");
    let script = scratch.path().join("journey.js");
    let stylesheet = scratch.path().join("skein-ui.css");
    assert_eq!(fs::read(&overlay).expect("empty overlay"), b"");
    assert_eq!(fs::read_to_string(&script).expect("script"), SCRIPT);
    assert_eq!(fs::read(&stylesheet).expect("stylesheet"), skein_ui_bundle::STYLESHEET);
    let rule = b"#app { color: red; }\n";
    fs::write(&overlay, rule).expect("person's overlay");
    fs::write(&script, "old script").expect("old script");
    fs::write(&stylesheet, "old stylesheet").expect("old stylesheet");
    beside(scratch.path()).expect("rewrite beside the journeys");
    assert_eq!(fs::read(&overlay).expect("kept overlay"), rule);
    assert_eq!(fs::read_to_string(&script).expect("rewritten script"), SCRIPT);
    assert_eq!(fs::read(&stylesheet).expect("rewritten stylesheet"), skein_ui_bundle::STYLESHEET);
}

#[test]
fn the_overlay_is_empty() {
    let overlay = fs::read(Path::new(JOURNEYS).join("overlay.css")).expect("committed overlay.css");
    assert!(
        overlay.is_empty(),
        "overlay.css holds a request: move its rules into temper.css or task-pages.css, and empty it"
    );
}
#[test]
fn every_journey_has_its_description() {
    for name in crate::names(Path::new(JOURNEYS)).expect("committed journeys") {
        let path = Path::new(JOURNEYS).join(&name).join("README.md");
        assert!(
            fs::read_to_string(path).is_ok_and(|text| !text.trim().is_empty()),
            "{name} needs a README.md holding its description"
        );
    }
}
#[test]
fn a_reprint_keeps_a_journeys_readme() {
    let scratch = Scratch::new("temper-client-journeys-readme");
    let directory = scratch.path().join("welcome-set-up");
    fs::create_dir(&directory).expect("journey directory");
    let readme = directory.join("README.md");
    let description = b"The journey's description.\n";
    fs::write(&readme, description).expect("write description");
    fs::write(directory.join("99.html"), "old page").expect("old page");
    assert!(crate::reprint(Path::new(JOURNEYS), scratch.path()).expect("reprint").is_empty());
    assert_eq!(fs::read(readme).expect("description kept"), description);
    assert!(!directory.join("99.html").exists());
}

#[test]
fn a_submits_field_leads_on_by_its_act() {
    let mut first = mark("<div><form><input data-node=\"3\"></form></div>", "", Some(3));
    first.lead = journey::Lead::Submit;
    let printed = pages(&[first, mark("<div></div>", "", None)]);
    assert!(printed[0].document.contains("<input data-journey-next=\"02.html\" data-journey-act=\"submit\"\n>"));
}
#[test]
fn a_toggles_box_leads_on_by_its_act() {
    let mut first = mark("<div><input type=\"checkbox\" data-node=\"3\"></div>", "", Some(3));
    first.lead = journey::Lead::Toggle;
    let printed = pages(&[first, mark("<div></div>", "", None)]);
    assert!(
        printed[0]
            .document
            .contains("<input type=\"checkbox\" data-journey-next=\"02.html\" data-journey-act=\"toggle\"\n>")
    );
}
