//! Diagnose the stock's floor before a numbered seed's copied policy bound.
use super::member;
use jig_protocol_config::{self as config, Segment, read};
use skein_json::{
    Compact, Document,
    document::{Kind, Limits},
};
use skein_lib::List;

/// A project refusal permits a diagnostic parse with no projects. The engine's
/// token index stays fixed because only its projects' value is replaced.
pub(super) fn without_projects(document: &Document, engine: u32, found: &config::Problems) -> Option<Document> {
    let mut project_problem = false;
    for problem in &found.problems {
        if let Some(Segment::Member(name)) = problem.path.segments.get(0)
            && name.as_ref() == b"projects"
        {
            project_problem = true;
        }
    }
    if !project_problem {
        return None;
    }
    let at = member(document, engine, b"projects")?;
    let after = read::after(document, at).ok()?;
    let mut text = List::with_capacity(document.text_len());
    let mut tokens = List::with_capacity(document.len().checked_add(2)?);
    for index in 0..document.len() {
        if index == at {
            for kind in [Kind::ArrayStart, Kind::ArrayEnd] {
                tokens.push(Compact { kind, start: 0, len: 0 }).expect("document plus empty array");
            }
        }
        if index >= at && index < after {
            continue;
        }
        let mut token = *document.token(index).expect("retained token");
        let bytes = document.text(&token).expect("retained text");
        if !bytes.is_empty() {
            token.start = text.len();
            for byte in bytes {
                text.push(*byte).expect("retained document text");
            }
        }
        tokens.push(token).expect("document plus empty array");
    }
    Some(
        Document::from_parts(
            text.into_boxed(),
            tokens.into_boxed(),
            &Limits { tokens: document.len().checked_add(2)?, text: document.text_len() },
        )
        .expect("retained document with an empty projects array"),
    )
}
