//! Static browser pages from real client states (client/browser.md, 7).
use skein_lib::Queue;
use skein_ui::{DrawingKind, Role, scene::Scene};
use skein_ui_web::{Look, mapping::Primary, page::Page};
use std::fmt::Write;
use std::{
    fs,
    path::{Path, PathBuf},
};
mod driver;
pub mod samples;
#[cfg(test)]
mod tests;
mod worlds;
pub use samples::Sample;

pub const STYLESHEETS: [&str; 2] = [
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/temper-client-shell-web/temper.css"),
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/temper-client-shell-web/task-pages.css"),
];
#[derive(Debug)]
pub enum Refusal {
    NotGallery { out: PathBuf },
    Bundle(skein_ui_bundle::Refusal),
    Io { path: PathBuf },
}
/// Prints every state's page, the bundle and the index into out (client/browser.md, 7.1).
/// # Errors
/// Refuses an existing directory without index.html, or a bundle or filesystem failure.
pub fn print(out: &Path) -> Result<(), Refusal> {
    if out.exists() {
        if !out.join("index.html").is_file() {
            return Err(Refusal::NotGallery { out: out.to_owned() });
        }
        if fs::remove_dir_all(out).is_err() {
            return Err(Refusal::Io { path: out.to_owned() });
        }
    }
    if fs::create_dir_all(out).is_err() {
        return Err(Refusal::Io { path: out.to_owned() });
    }
    let mut stylesheets = Vec::new();
    for path in STYLESHEETS {
        stylesheets.push(PathBuf::from(path));
    }
    let inputs = skein_ui_bundle::Inputs {
        worker: None,
        name: "temper_client_shell_web".to_owned(),
        prefix: "/bundle/".to_owned(),
        stylesheets,
        pictures: vec![],
        page: None,
        files: Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/temper-client-shell-web/files"))),
        noscript: "temper needs WebAssembly and scripts.".to_owned(),
    };
    let digest = match skein_ui_bundle::bundle(
        &inputs,
        &skein_ui_bundle::Glue { js: vec![], wasm: vec![], snippets: vec![] },
        None,
        &out.join("bundle"),
    ) {
        Ok(digest) => digest,
        Err(why) => return Err(Refusal::Bundle(why)),
    };
    let samples = samples::samples();
    for sample in &samples {
        write(&out.join(format!("{}.html", sample.name)), &page(sample, &digest))?;
    }
    write(&out.join("index.html"), &index(&samples))
}
fn write(path: &Path, bytes: &str) -> Result<(), Refusal> {
    match fs::write(path, bytes) {
        Ok(()) => Ok(()),
        Err(_) => Err(Refusal::Io { path: path.to_owned() }),
    }
}
fn draw(_: DrawingKind, _: &[u8], _: u16, _: u16, _: &mut Scene) -> bool {
    false
}

/// The same browser view construction used by pages and semantic tests.
#[must_use]
pub fn view(sample: &Sample) -> temper_client_view::View {
    let (view, _, _) = built(sample);
    view
}
fn built(sample: &Sample) -> (temper_client_view::View, temper_client_view::Limits, Vec<skein_ui::Patch>) {
    let root = samples::domain(sample);
    let limits = driver::limits();
    let words = temper_client_view::words::Words::english(
        temper_client_protocol_wire::FORGE,
        limits.client.domain.text,
        temper_client_view::words::Shell::Browser,
    )
    .expect("shipped browser words");
    let bounds = temper_client_view::Limits::of(&limits, &words).expect("shipped view bounds");
    let mut view = temper_client_view::View::with_sign_in(
        &bounds,
        &limits,
        words,
        temper_client_protocol_wire::FORGE,
        temper_client_view::sign_in::Ways {
            google: sample.then == samples::Then::GoogleSignIn,
            origin: Some(Box::from(b"https://temper.example.com".as_slice())),
        },
    );
    let mut forge_sign_in = temper_client_view::forge_sign_in::Forges::new();
    forge_sign_in.read("3 forgejo http://127.0.0.1:8368 owned");
    view.forge_sign_in(forge_sign_in);
    let mut patches = Queue::with_capacity(bounds.tree.patches);
    temper_client_view::build(&mut view, &root, worlds::now(sample.world), &mut patches);
    let mut collected = Vec::new();
    for _ in 0..patches.len() {
        collected.push(patches.pop().expect("queued patch"));
    }
    (view, bounds, collected)
}
/// One state's page, a whole HTML document.
#[must_use]
pub fn page(sample: &Sample, digest: &skein_ui_bundle::Digest) -> String {
    let (mut view, bounds, collected) = built(sample);
    let mut page = Page::new(
        Look {
            names: &[temper_client_view::look::NAMES],
            pictures: &[],
            components: &[],
            elements: temper_client_view::look::ELEMENTS,
            draw,
        },
        &bounds.tree,
        b"temper",
        Primary::Control,
    );
    let mut ops = Vec::new();
    page.apply(&collected, &mut ops);
    let links = page.links().to_vec();
    for node in links {
        if let Some(href) = temper_client_view::href(&mut view, node) {
            page.set_href(node, &href, &mut ops);
        }
    }
    let title = escape(&skein_ui_web::title::text(&heading(view.tree()), b"temper"));
    let html = page.html().replacen("<div", "<div id=\"app\"", 1).replace("<dialog ", "<dialog open ");
    let mut styles = String::new();
    for file in ["skein-ui.css", "temper.css", "task-pages.css"] {
        write!(styles, "<link rel=\"stylesheet\" href=\"bundle/{digest}/{file}\">").expect("write string");
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title}</title>{styles}</head><body>{html}</body></html>\n"
    )
}
fn heading(tree: &skein_ui::Tree) -> Vec<u8> {
    let nodes = tree.nodes();
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Main {
            let end = index + usize::try_from(node.size).expect("main subtree");
            for (offset, heading) in nodes[index..end].iter().enumerate() {
                if heading.role == (Role::Heading { level: 1 }) {
                    let start = index + offset;
                    let end = start + usize::try_from(heading.size).expect("heading subtree");
                    let mut result = Vec::new();
                    for node in &nodes[start..end] {
                        for run in &node.text {
                            result.extend_from_slice(&run.text);
                        }
                    }
                    return result;
                }
            }
        }
    }
    Vec::new()
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}
#[must_use]
pub fn index(samples: &[Sample]) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>temper gallery</title></head><body><h1>temper gallery</h1><ul>\n",
    );
    for sample in samples {
        write!(html, "<li><a href=\"{0}.html\">{0}</a>", sample.name).expect("write string");
        if let Some(mockup) = samples::mockup(sample) {
            write!(html, " <span>{}</span>", escape(&mockup)).expect("write string");
        }
        html.push_str("</li>\n");
    }
    html.push_str("</ul></body></html>\n");
    html
}
