#![expect(
    clippy::disallowed_methods,
    reason = "step fixtures assemble expected text and validate their static ASCII bytes"
)]
#![expect(clippy::arithmetic_side_effects, reason = "fixture offsets and answer lengths are bounded constants")]

use super::{decoded, run};
use crate::{CALL_NAME_BYTES, DOC_ANSWER, Decoded, Problem, docs};
use temper_docs::{Kind, Topic};

static STORIES: [Topic; 2] = [
    Topic {
        name: "small",
        kind: Kind::Concept,
        title: "Small",
        summary: "A short guide.",
        text: "# Small\n\nKind: concept\n\nA short guide.\n\n## Rules\nLanding rules work.",
    },
    Topic {
        name: "many",
        kind: Kind::Concept,
        title: "Many",
        summary: "Find matches.",
        text: "# Many\n\nKind: concept\n\nFind matches.\n\n## One\nmatch\n## Two\nmatch\n## Three\nmatch\n## Four\nmatch\n## Five\nmatch\n## Six\nmatch\n## Seven\nmatch\n## Eight\nmatch\n## Nine\nmatch\n",
    },
];

fn answered(json: &[u8]) -> run::HostAnswer {
    let Decoded::Answered(answer) = decoded(b"read_docs", json).unwrap() else { panic!("answered at decode") };
    answer
}

// The public decoder always uses the compiled table. Fixtures exercise the
// same read/answer functions with named static topics, without replacing it.
fn fixture(table: &'static [Topic], json: &[u8]) -> run::HostAnswer {
    let public = answered(json);
    let answer = match docs::read(&crate::json::parse(json).unwrap()).unwrap() {
        Ok(read) => docs::answer(table, &read),
        Err(answer) => answer,
    };
    assert_eq!(public.error(), answer.error());
    answer
}

fn exact(answer: run::HostAnswer, expected: &str, error: bool) {
    assert_eq!(answer.error(), error);
    let (body, _) = answer.into_parts();
    assert_eq!(body.as_ref(), expected.as_bytes());
}

#[test]
fn read_docs_answers_a_topics_first_page_and_its_next() {
    let topic = temper_docs::find(b"box").unwrap();
    let first = temper_docs::page(topic, 0, 1_536);
    let next = first.next.expect("box spans pages");
    let header = "box: Start a local temper box\n";
    exact(answered(br#"{"topic":"box"}"#), &format!("{header}{}next: {next}\n", first.text), false);
    let second = temper_docs::page(topic, next, 1_536);
    assert_eq!(first.text.len(), usize::try_from(next).unwrap());
    assert!(topic.text.get(usize::try_from(next).unwrap()..).unwrap().starts_with(second.text));
    let end = match second.next {
        Some(next) => format!("next: {next}\n"),
        None => "end\n".into(),
    };
    exact(
        answered(format!(r#"{{"topic":"box","from":{next}}}"#).as_bytes()),
        &format!("{header}{}{end}", second.text),
        false,
    );
    exact(answered(br#"{"topic":"box","from":4294967295}"#), &format!("{header}end\n"), false);
}

#[test]
fn read_docs_answers_the_last_page_with_end() {
    exact(
        fixture(&STORIES, br#"{"topic":"small","from":40}"#),
        "small: Small\n## Rules\nLanding rules work.\nend\n",
        false,
    );
    exact(fixture(&STORIES, br#"{"topic":"small","from":999}"#), "small: Small\nend\n", false);
}

#[test]
fn read_docs_answers_a_search_a_line_per_section() {
    exact(
        fixture(&STORIES, br#"{"search":"match"}"#),
        "many@0: Many: Find matches.\nmany@38: One: match\nmany@51: Two: match\nmany@64: Three: match\nmany@79: Four: match\nmany@93: Five: match\nmany@107: Six: match\nmany@120: Seven: match\nmore: narrow the words\n",
        false,
    );
    exact(fixture(&STORIES, br#"{"search":"LANDING rules"}"#), "small@40: Rules: Landing rules work.\n", false);
}

#[test]
fn read_docs_answers_a_search_with_no_hit_nothing_matches() {
    exact(answered(br#"{"search":"unfindable-zqxj"}"#), "nothing matches\n", false);
    exact(fixture(&STORIES, br#"{"search":"absent"}"#), "nothing matches\n", false);
}

#[test]
fn read_docs_answers_an_unknown_topic_with_the_hits_of_its_words() {
    exact(
        fixture(&STORIES, br#"{"topic":"landing-rules"}"#),
        "no topic landing-rules\nsmall@40: Rules: Landing rules work.\n",
        false,
    );
    exact(answered(br#"{"topic":"unfindable-zqxj"}"#), "no topic unfindable-zqxj\nnothing matches\n", false);
}

#[test]
fn read_docs_answers_both_members_or_neither_as_an_error() {
    let cases: &[(&[u8], &str)] = &[
        (b"{}", "read_docs needs \"topic\" or \"search\"\n"),
        (br#"{"topic":"box","search":"box"}"#, "read_docs takes \"topic\" or \"search\", not both\n"),
        (br#"{"search":"box","from":0}"#, "\"from\" goes with \"topic\"\n"),
        (br#"{"topic":"BAD"}"#, "a topic's name is at most 32 bytes of a-z, 0-9 and -\n"),
        (br#"{"topic":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#, "a topic's name is at most 32 bytes of a-z, 0-9 and -\n"),
        (br#"{"topic":""}"#, "a topic's name is at most 32 bytes of a-z, 0-9 and -\n"),
        (br#"{"search":"a b c d e f g h i"}"#, "at most 8 words\n"),
        (br#"{"search":""}"#, "nothing to search for\n"),
        (br#"{"search":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#, "a word is at most 32 bytes\n"),
    ];
    for (json, expected) in cases {
        exact(answered(json), expected, true);
    }
}

#[test]
fn read_docs_rejects_wrong_member_types_and_ranges_before_semantic_errors() {
    let cases: &[(&[u8], Problem)] = &[
        (br#"{"topic":3}"#, Problem::Type(b"topic")),
        (br#"{"search":3}"#, Problem::Type(b"search")),
        (br#"{"topic":"box","from":"0"}"#, Problem::Type(b"from")),
        (br#"{"topic":"box","from":-1}"#, Problem::Range(b"from")),
        (br#"{"topic":"box","from":4294967296}"#, Problem::Range(b"from")),
        (br#"{"topic":"box","from":0.5}"#, Problem::Range(b"from")),
        (br#"{"topic":"box","search":3}"#, Problem::Type(b"search")),
        (br#"{"search":"box","from":-1}"#, Problem::Range(b"from")),
    ];
    for (json, problem) in cases {
        assert_eq!(decoded(b"read_docs", json).unwrap_err(), *problem);
    }
}

#[test]
fn read_docs_is_declared_as_a_read() {
    let declared = crate::tools(skein_lib::Duration::from_secs(1));
    let mut found = false;
    for tool in &declared {
        if tool.name.as_ref() == b"read_docs" {
            found = true;
            assert_eq!(tool.effect, run::HostEffect::Read);
            assert_eq!(tool.schema.as_ref(), br#"{"type":"object","properties":{"topic":{"type":"string"},"from":{"type":"integer","minimum":0},"search":{"type":"string"}}}"#);
            assert_eq!(tool.description.as_ref(), br#"Read temper's own documentation: setting it up, its concepts, recipes and limits. {"search": "WORDS"} answers the sections holding every word, a line each, NAME@OFFSET: HEADING: LINE. {"topic": NAME, "from": OFFSET} answers a page of a topic from OFFSET (0 when absent), then next: OFFSET to read on, or end."#);
        }
    }
    assert!(found);
}

const BOUND_NAME: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOUND_HEADING: &str = "hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh";
const BOUND_WORDS: &str = "needle aaaaaaaaaaaaaaaaaaaaaaaaa";

#[expect(clippy::large_stack_arrays, reason = "the const function builds one static fixture at compile time")]
const fn bound_text() -> [u8; temper_docs::TOPIC_BYTES] {
    let mut text = [b'p'; temper_docs::TOPIC_BYTES];
    let mut section = 0;
    while section < 9 {
        let start = 10_000 + section * 2_500;
        text[start - 1] = b'\n';
        text[start] = b'#';
        text[start + 1] = b'#';
        text[start + 2] = b' ';
        let mut at = 0;
        while at < 80 {
            text[start + 3 + at] = b'h';
            at += 1;
        }
        text[start + 83] = b'\n';
        at = 0;
        while at < BOUND_WORDS.len() {
            text[start + 84 + at] = BOUND_WORDS.as_bytes()[at];
            at += 1;
        }
        text[start + 116] = b'"';
        text[start + 117] = b'\\';
        section += 1;
    }
    text
}
static BOUND_TEXT: [u8; temper_docs::TOPIC_BYTES] = bound_text();
static BOUNDS: [Topic; 1] = [Topic {
    name: BOUND_NAME,
    kind: Kind::Concept,
    title: BOUND_HEADING,
    summary: "Bounds.",
    text: match core::str::from_utf8(&BOUND_TEXT) {
        Ok(text) => text,
        Err(_) => panic!("ASCII fixture"),
    },
}];

#[test]
fn read_docs_answers_fit_doc_answer_at_their_bounds() {
    assert_eq!(BOUND_NAME.len(), temper_docs::NAME_BYTES);
    assert_eq!(BOUND_HEADING.len(), temper_docs::TITLE_BYTES);
    assert_eq!(BOUND_HEADING.len(), temper_docs::HEADING_BYTES);
    assert_eq!(BOUNDS[0].text.len(), temper_docs::TOPIC_BYTES);
    let line = &BOUNDS[0].text[10_084..12_499];
    assert!(line.len() > temper_docs::LINE_BYTES);
    assert!(line.contains('"') && line.contains('\\'));
    let page = fixture(&BOUNDS, format!(r#"{{"topic":"{BOUND_NAME}"}}"#).as_bytes());
    exact(page, &format!("{BOUND_NAME}: {BOUND_HEADING}\n{}\nnext: 1536\n", "p".repeat(1536)), false);
    let mut hits = skein_lib::Writer::new(1967);
    for section in 0..8 {
        let start = 10_000 + section * 2_500;
        hits.put(
            format!("{BOUND_NAME}@{start}: {BOUND_HEADING}: {}\n", &BOUNDS[0].text[start + 84..start + 204]).as_bytes(),
        )
        .unwrap();
    }
    hits.put(b"more: narrow the words\n").unwrap();
    let hit_bytes = hits.finish();
    let hits = core::str::from_utf8(&hit_bytes).unwrap();
    let cases = [
        (format!(r#"{{"topic":"{BOUND_NAME}"}}"#), 1663),
        (format!(r#"{{"search":"{BOUND_WORDS}"}}"#), 1967),
        (r#"{"topic":"needle-aaaaaaaaaaaaaaaaaaaaaaaaa"}"#.into(), 2009),
    ];
    exact(fixture(&BOUNDS, cases[1].0.as_bytes()), hits, false);
    exact(
        fixture(&BOUNDS, cases[2].0.as_bytes()),
        &format!("no topic needle-aaaaaaaaaaaaaaaaaaaaaaaaa\n{hits}"),
        false,
    );
    for (input, largest) in cases {
        let answer = fixture(&BOUNDS, input.as_bytes());
        assert!(!answer.error());
        let (body, _) = answer.into_parts();
        assert_eq!(body.len(), largest);
        assert!(usize::try_from(CALL_NAME_BYTES + 9).unwrap() + body.len() <= usize::try_from(DOC_ANSWER).unwrap());
    }
    let high_page = fixture(&BOUNDS, format!(r#"{{"topic":"{BOUND_NAME}","from":31000}}"#).as_bytes());
    assert_eq!(high_page.text().len(), 1664);
    assert!(
        usize::try_from(CALL_NAME_BYTES + 9).unwrap() + high_page.text().len() <= usize::try_from(DOC_ANSWER).unwrap()
    );
    exact(high_page, &format!("{BOUND_NAME}: {BOUND_HEADING}\n{}\nnext: 32536\n", "p".repeat(1536)), false);
    // Quotes/backslashes consume two rendered bytes apiece; a cut inside
    // their long line still adds a line end before its continuation marker.
    let page = temper_docs::page(&BOUNDS[0], 10_084, 1_536);
    let next = page.next.unwrap();
    assert_eq!(page.text.len(), 1534);
    exact(
        fixture(&BOUNDS, format!(r#"{{"topic":"{BOUND_NAME}","from":10084}}"#).as_bytes()),
        &format!("{BOUND_NAME}: {BOUND_HEADING}\n{}\nnext: {next}\n", page.text),
        false,
    );
}
