use super::{checks::*, *};
use std::{fs, path::Path};

#[test]
#[expect(clippy::disallowed_macros, reason = "the step test locates the pages through CARGO_MANIFEST_DIR")]
fn the_table_is_the_directory() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help");
    let mut names = Vec::new();
    for (directory, reference) in [(directory.clone(), false), (directory.join("reference"), true)] {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if reference && error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("{}: {error}", directory.display()),
        };
        for entry in entries {
            let path = entry.expect("page file").path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let name = path.file_stem().expect("page stem").to_str().expect("ASCII name");
                names.push(String::from(name));
                let entry = find(name.as_bytes()).expect("file in table");
                let text = fs::read_to_string(&path).expect("page bytes");
                assert_eq!(entry.text, text, "{name}: file bytes");
                let kind = if reference {
                    "reference"
                } else {
                    text.lines().nth(2).expect("kind line").strip_prefix("Kind: ").expect("kind prefix")
                };
                assert_eq!(entry.kind.word(), kind, "{name}: file kind");
            }
        }
    }
    assert!(PAGES.len() <= MAX_PAGES, "table: MAX_PAGES");
    let mut previous = None;
    for entry in &PAGES {
        let kind = KINDS.iter().position(|kind| *kind == entry.kind).expect("page kind");
        let key = (kind, entry.name);
        if let Some(previous) = previous {
            assert!(previous < key, "{}: kind and name order", entry.name);
        }
        previous = Some(key);
    }
    assert_eq!(PAGES[0].name, "index");
    assert_eq!(PAGES.iter().filter(|page| page.kind == Kind::Index).count(), 1);
    let mut table_names = PAGES.iter().map(|entry| String::from(entry.name)).collect::<Vec<_>>();
    names.sort();
    table_names.sort();
    assert_eq!(names, table_names, "table names equal the directory");
}

#[test]
fn each_page_keeps_its_head_template_links_and_voice() {
    for entry in &PAGES {
        for check in [head, template, links, placeholders, voice] {
            check(entry).unwrap_or_else(|why| panic!("{why}"));
        }
    }
}

#[test]
fn each_guide_is_one_of_the_eight_names() {
    let guides = PAGES.iter().filter(|page| page.kind == Kind::Guide).map(|page| page.name).collect::<Vec<_>>();
    let expected = GUIDES.into_iter().filter(|name| guides.contains(name)).collect::<Vec<_>>();
    assert_eq!(guides, expected);
}

#[test]
fn the_index_names_every_concept_walk_and_recipe_once() {
    let text = find(b"index").expect("index").text;
    for entry in &PAGES {
        if written(entry) && entry.kind != Kind::Index {
            let count = text
                .split("temper help ")
                .skip(1)
                .filter(|after| {
                    after.split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).next()
                        == Some(entry.name)
                })
                .count();
            assert_eq!(count, 1, "{}: index names page exactly once", entry.name);
        }
    }
}

const CONCEPT: &str = "## What it is\n## How it changes\n## What temper guarantees\n## Acting on it\n## Limits\n";
const WALK: &str = "## Before you start\n## 1. Read\nExpect: read\n## 2. Act\nExpect: done\n## Clean up\n## Next\n";
const RECIPE: &str = "## Goal\n## Settings\n## Enforced and followed\n## Checking it took\n";

fn leaked(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn fails(check: fn(&Page) -> Result<(), String>, entry: &Page, rule: &str) {
    let error = check(entry).expect_err("fixture refused");
    assert!(error.contains(entry.name) && error.contains(rule) && error.contains("line "), "{error}");
}

#[test]
fn a_page_at_its_kinds_bound_passes_and_one_byte_more_fails() {
    for kind in KINDS {
        let text = match kind {
            Kind::Concept => CONCEPT,
            Kind::Walk => WALK,
            Kind::Recipe => RECIPE,
            Kind::Index | Kind::Reference | Kind::Guide => "",
        };
        let mut padded = String::from(text);
        padded.push_str(&"x".repeat(kind.bound() - text.len()));
        template(&fixture(kind, leaked(padded.clone()))).expect("exact bound passes");
        padded.push('x');
        fails(template, &fixture(kind, leaked(padded)), &format!("bound {}", kind.bound()));
    }
}

#[test]
fn a_concept_keeps_its_headings_in_order() {
    for text in [CONCEPT.into(), CONCEPT.replace("## How it changes\n", "")] {
        template(&fixture(Kind::Concept, leaked(text))).expect("concept headings");
    }
    for text in [
        CONCEPT.replace("## Limits\n", ""),
        CONCEPT.replace("What it is", "Limits").replace("## Limits\n", "## What it is\n"),
        format!("{CONCEPT}## Another\n"),
    ] {
        fails(template, &fixture(Kind::Concept, leaked(text)), "headings");
    }
}

#[test]
fn a_walks_steps_number_from_one_and_say_what_to_expect() {
    template(&fixture(Kind::Walk, WALK)).expect("steps 1 and 2");
    for text in [
        WALK.replace("2. Act", "3. Act"),
        WALK.replace("## 1. Read\nExpect: read\n", ""),
        WALK.replace("## Clean up\n", ""),
    ] {
        fails(template, &fixture(Kind::Walk, leaked(text)), "headings");
    }
    fails(template, &fixture(Kind::Walk, leaked(WALK.replace("Expect: done\n", ""))), "Expect:");
    fails(
        template,
        &fixture(Kind::Walk, leaked(WALK.replace("Expect: done\n", "```\nExpect: done\n```\n"))),
        "Expect:",
    );
}

#[test]
fn a_recipe_keeps_its_four_headings() {
    template(&fixture(Kind::Recipe, RECIPE)).expect("recipe headings");
    fails(template, &fixture(Kind::Recipe, leaked(RECIPE.replace("## Settings\n", ""))), "headings");
}

#[test]
fn a_capital_word_after_temper_help_is_no_link() {
    links(&fixture(Kind::Index, "temper help NAME\ntemper help --search\ntemper help\n")).expect("no links");
    fails(links, &fixture(Kind::Index, "temper help nope"), "link nope");
}

#[test]
fn placeholders_are_the_set_or_declared_by_the_walk() {
    placeholders(&fixture(Kind::Concept, "```\ntemper ctl DIR task TASK\n```\n")).expect("set");
    fails(placeholders, &fixture(Kind::Concept, "```\ntemper ctl DIR task N\n```\n"), "placeholder N");
    let text =
        "## Before you start\n- `OWNER` is the owner.\n## 1. Act\n```\ntemper ctl DIR adopt --owner OWNER\n```\n";
    placeholders(&fixture(Kind::Walk, text)).expect("declared");
    placeholders(&fixture(Kind::Walk, leaked(text.replace("OWNER", "N")))).expect("one-byte declaration");
    fails(
        placeholders,
        &fixture(Kind::Walk, leaked(text.replace("- `OWNER` is the owner.\n", ""))),
        "placeholder OWNER",
    );
    placeholders(&fixture(Kind::Concept, "OWNER in prose\n```\ntemper ctl DIR task taskTASK\n```\n"))
        .expect("prose or mixed case");
    placeholders(&fixture(Kind::Guide, "```\ntemper ctl DIRECTORY task N\n```\n")).expect("guides exempt");
}

#[test]
fn the_designs_words_stay_in_code() {
    voice(&fixture(Kind::Concept, "`jig`\n```\njig\n```\njigsaw\n")).expect("code and larger words");
    for text in ["jig in prose", "smith in prose", "skein in prose", "see box.md", "temper-docs"] {
        fails(voice, &fixture(Kind::Concept, text), "voice");
    }
    voice(&fixture(Kind::Guide, "jig in prose")).expect("guides exempt");
}

#[test]
fn a_head_refuses_wrong_fields_and_tests_each_byte_bound_on_both_sides() {
    let text = "# Fixture\n\nKind: index\n\nFixture.\n";
    head(&fixture(Kind::Index, text)).expect("valid head");
    for (text, rule) in [
        (text.replace("# Fixture", "Fixture"), "title"),
        (text.replace("Kind: index", "Kind: walk"), "kind"),
        (text.replace("Fixture.\n", "Other.\n"), "summary"),
        (format!("{text}\t"), "ASCII"),
        (format!("{text}é"), "ASCII"),
    ] {
        fails(head, &fixture(Kind::Index, leaked(text)), rule);
    }
    for (bound, field) in
        [(NAME_BYTES, "name"), (TITLE_BYTES, "title"), (SUMMARY_BYTES, "summary"), (HEADING_BYTES, "heading")]
    {
        for extra in [0, 1] {
            let value = leaked("x".repeat(bound + extra));
            let mut entry = fixture(Kind::Index, text);
            match field {
                "name" => entry.name = value,
                "title" => {
                    entry.title = value;
                    entry.text = leaked(format!("# {value}\n\nKind: index\n\nFixture.\n"));
                }
                "summary" => {
                    let summary = leaked(format!("{}.", &value[1..]));
                    entry.summary = summary;
                    entry.text = leaked(format!("# Fixture\n\nKind: index\n\n{summary}\n"));
                }
                _ => entry.text = leaked(format!("{text}\n## {value}\n")),
            }
            if extra == 0 {
                head(&entry).expect("bound passes");
            } else {
                fails(head, &entry, field);
            }
        }
    }
    for name in ["", "Upper", "under_score"] {
        fails(head, &Page { name, ..fixture(Kind::Index, text) }, "name");
    }
    head(&fixture(Kind::Index, "# Fixture\n\nKind: index\n\nFix\nture.\n")).expect_err("joined summary differs");
    head(&Page { summary: "Fix ture.", ..fixture(Kind::Index, "# Fixture\n\nKind: index\n\nFix\nture.\n") })
        .expect("joined summary");
}
