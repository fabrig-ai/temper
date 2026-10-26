//! The journeys: the client's pages recorded, replayed and printed (client/journeys.md).

use skein_ui::Role;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use temper_client_shell_web::journey::{self, Stale};

pub mod print;
/// Where the journeys live, by the manifest's path (client/journeys.md, 5.1).
pub const JOURNEYS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/client/journeys");

#[derive(Debug)]
pub enum Refusal {
    Name(String),
    Proof { name: String, refusal: journey::Refusal },
    Unreadable { name: String, unreadable: journey::Unreadable },
    Io { path: PathBuf },
}

/// Whether NAME is a journey's name, `[a-z][a-z0-9-]*` (client/journeys.md, 5.1).
#[must_use]
pub fn named(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Takes a saved recording as the journey NAME, replacing one of that name:
/// proves it, keeps it as `NAME/recording` without its live prints, and prints it.
/// # Errors
/// Refuses an invalid name, an unproved recording or a filesystem failure.
pub fn add(journeys: &Path, name: &str, recording: &[u8]) -> Result<(), Refusal> {
    if !named(name) {
        return Err(Refusal::Name(name.to_owned()));
    }
    let recording = journey::prove(recording).map_err(|refusal| Refusal::Proof { name: name.to_owned(), refusal })?;
    let directory = journeys.join(name);
    fs::create_dir_all(&directory).map_err(|_| io_refusal(&directory))?;
    let path = directory.join("recording");
    fs::write(&path, &recording).map_err(|_| io_refusal(&path))?;
    if let Some(stale) = print_one(&directory, name, &recording)? {
        return Err(Refusal::Proof { name: name.to_owned(), refusal: journey::Refusal::Stale(stale) });
    }
    print::beside(journeys).map_err(|_| io_refusal(journeys))
}

/// Reprints every journey under `journeys` (each directory holding a `recording`)
/// into `out`, in name order, and writes the files beside them. A stale journey's
/// pages are left as they were; it is returned with where it went stale.
/// # Errors
/// Refuses an unreadable committed recording or a filesystem failure.
pub fn reprint(journeys: &Path, out: &Path) -> Result<Vec<(String, Stale)>, Refusal> {
    let mut stale = Vec::new();
    for name in names(journeys)? {
        let path = journeys.join(&name).join("recording");
        let recording = fs::read(&path).map_err(|_| io_refusal(&path))?;
        if let Some(stopped) = print_one(&out.join(&name), &name, &recording)? {
            stale.push((name, stopped));
        }
    }
    fs::create_dir_all(out).map_err(|_| io_refusal(out))?;
    print::beside(out).map_err(|_| io_refusal(out))?;
    Ok(stale)
}

fn names(journeys: &Path) -> Result<Vec<String>, Refusal> {
    let entries = match fs::read_dir(journeys) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(io_refusal(journeys)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| io_refusal(journeys))?;
        let path = entry.path();
        if !entry.file_type().map_err(|_| io_refusal(&path))?.is_dir() {
            continue;
        }
        let recording = path.join("recording");
        if recording.try_exists().map_err(|_| io_refusal(&recording))? {
            names.push(entry.file_name().into_string().map_err(|_| io_refusal(&path))?);
        }
    }
    names.sort();
    Ok(names)
}

fn print_one(out: &Path, name: &str, recording: &[u8]) -> Result<Option<Stale>, Refusal> {
    let mut replay = journey::Replay::new(recording).map_err(|refusal| match refusal {
        journey::Refusal::Unreadable(unreadable) => Refusal::Unreadable { name: name.to_owned(), unreadable },
        refusal @ (journey::Refusal::Stale(_)
        | journey::Refusal::Unproved { .. }
        | journey::Refusal::Differs { .. }) => Refusal::Proof { name: name.to_owned(), refusal },
    })?;
    let mut marks = Vec::new();
    loop {
        match replay.next() {
            Ok(Some(mark)) => {
                marks.push(print::Mark {
                    html: mark.page.html(),
                    heading: heading(mark.tree),
                    acted: mark.acted,
                    lead: mark.lead,
                });
            }
            Ok(None) => break,
            Err(stale) => return Ok(Some(stale)),
        }
    }
    let pages = print::pages(&marks);
    fs::create_dir_all(out).map_err(|_| io_refusal(out))?;
    for entry in fs::read_dir(out).map_err(|_| io_refusal(out))? {
        let entry = entry.map_err(|_| io_refusal(out))?;
        let path = entry.path();
        if printed_page(&path) && entry.file_type().map_err(|_| io_refusal(&path))?.is_file() {
            fs::remove_file(&path).map_err(|_| io_refusal(&path))?;
        }
    }
    for page in pages {
        let path = out.join(page.name);
        fs::write(&path, page.document).map_err(|_| io_refusal(&path))?;
    }
    Ok(None)
}

fn printed_page(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".html"))
        .is_some_and(|number| number.len() >= 2 && number.bytes().all(|byte| byte.is_ascii_digit()))
}

fn io_refusal(path: &Path) -> Refusal {
    Refusal::Io { path: path.to_owned() }
}

// The main region's level-1 heading, as in the gallery (client/browser.md, 7.1).
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

#[cfg(test)]
mod tests;
