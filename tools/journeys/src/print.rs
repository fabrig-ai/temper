//! Static pages and the files beside them (client/journeys.md, 5.2 and 5.3).

use skein_ui::NodeId;
use std::{fmt::Write, fs, io, path::Path};
pub use temper_client_shell_web::journey::Lead;

/// A page as the person saw it, at one mark (client/journeys.md, 3.4).
pub struct Mark {
    pub html: String,
    pub heading: Vec<u8>,
    pub acted: Vec<NodeId>,
    pub lead: Lead,
}

/// One printed page: its file name in the journey's directory and its document.
pub struct Printed {
    pub name: String,
    pub document: String,
}

/// A journey's pages, in the marks' order.
#[must_use]
pub fn pages(marks: &[Mark]) -> Vec<Printed> {
    let mut printed = Vec::new();
    for (index, mark) in marks.iter().enumerate() {
        let prev = (index > 0).then(|| format!("{index:02}.html"));
        let next = (index + 1 < marks.len()).then(|| format!("{:02}.html", index + 2));
        let mut html = mark.html.replacen("<div", "<div id=\"app\"", 1).replace("<dialog ", "<dialog open ");
        for NodeId(node) in &mark.acted {
            let next = next.as_deref().expect("an acted mark has a next page");
            let attribute = format!(" data-node=\"{node}\"");
            assert!(html.contains(&attribute), "the acted control is on the marked page");
            let act = match mark.lead {
                Lead::Press => "",
                Lead::Submit => " data-journey-act=\"submit\"",
                Lead::Toggle => " data-journey-act=\"toggle\"",
            };
            html = html.replacen(&attribute, &format!(" data-journey-next=\"{next}\"{act}"), 1);
        }
        while let Some(start) = html.find(" data-node=\"") {
            let value = start + " data-node=\"".len();
            let end = value + html[value..].find('"').expect("a node attribute has a closing quote") + 1;
            html.replace_range(start..end, "");
        }
        let title = escape(&skein_ui_web::title::text(&mark.heading, b"temper"));
        let mut document = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title}</title>"
        );
        for stylesheet in [
            "../skein-ui.css",
            "../../../../../crates/temper-client-shell-web/temper.css",
            "../../../../../crates/temper-client-shell-web/task-pages.css",
            "../overlay.css",
        ] {
            write!(document, "<link rel=\"stylesheet\" href=\"{stylesheet}\">").expect("write string");
        }
        if let Some(prev) = prev {
            write!(document, "<link rel=\"prev\" href=\"{prev}\">").expect("write string");
        }
        if let Some(next) = next {
            write!(document, "<link rel=\"next\" href=\"{next}\">").expect("write string");
        }
        write!(document, "<script src=\"../journey.js\" defer></script></head><body>{html}</body></html>")
            .expect("write string");
        printed.push(Printed { name: format!("{:02}.html", index + 1), document: lines(&document) });
    }
    printed
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn lines(html: &str) -> String {
    let html = html.strip_prefix("<!doctype html>\n").expect("a printed document starts with the doctype line");
    format!("<!doctype html>\n{}\n", html.replace('>', "\n>"))
}

/// The files beside the journeys, written at each print (client/journeys.md, 5.2):
/// `skein-ui.css`, `journey.js`, and `overlay.css` made empty when missing and never written otherwise.
/// # Errors
/// Returns a filesystem error when the stylesheet, script or a missing overlay cannot be written.
pub fn beside(journeys: &Path) -> io::Result<()> {
    fs::write(journeys.join("skein-ui.css"), skein_ui_bundle::STYLESHEET)?;
    fs::write(journeys.join("journey.js"), SCRIPT)?;
    match fs::OpenOptions::new().write(true).create_new(true).open(journeys.join("overlay.css")) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

/// The pages' script, written beside the journeys.
pub const SCRIPT: &str = include_str!("../journey.js");
