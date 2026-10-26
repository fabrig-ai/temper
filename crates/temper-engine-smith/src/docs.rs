//! `read_docs`: temper's documentation, answered at decode
//! (protocol/documentation.md, section 5).

use alloc::boxed::Box;
use skein_lib::{Decimal, List};
use smith_domain_run as run;

use crate::{CALL_NAME_BYTES, DOC_ANSWER, Problem, json::Value};

const PAGE_BYTES: u32 = 1_536;
const HITS: u32 = 8;
const BODY_BYTES: u32 = DOC_ANSWER - CALL_NAME_BYTES - 9;

/// A read, as its input asked it.
pub(crate) enum Read {
    Topic { name: Box<[u8]>, from: u32 },
    Search(Box<[u8]>),
}

/// Decode types and ranges before answering conflicting or missing members.
pub(crate) fn read(input: &Value) -> Result<Result<Read, run::HostAnswer>, Problem> {
    let topic = match input.field(b"topic")? {
        Some(value) => Some(value.text(b"topic")?),
        None => None,
    };
    let search = match input.field(b"search")? {
        Some(value) => Some(value.text(b"search")?),
        None => None,
    };
    let from = match input.field(b"from")? {
        Some(value) => Some(value.small(b"from")?),
        None => None,
    };
    match (topic, search) {
        (None, None) => Ok(Err(error(b"read_docs needs \"topic\" or \"search\""))),
        (Some(_), Some(_)) => Ok(Err(error(b"read_docs takes \"topic\" or \"search\", not both"))),
        (None, Some(_)) if from.is_some() => Ok(Err(error(b"\"from\" goes with \"topic\""))),
        (None, Some(words)) => Ok(Ok(Read::Search(words))),
        (Some(name), None) => {
            let mut valid = !name.is_empty() && name.len() <= temper_docs::NAME_BYTES;
            for byte in &name {
                if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-' {
                    valid = false;
                }
            }
            if !valid {
                return Ok(Err(error(b"a topic's name is at most 32 bytes of a-z, 0-9 and -")));
            }
            Ok(Ok(Read::Topic { name, from: from.unwrap_or(0) }))
        }
    }
}

pub(crate) fn answer(table: &'static [temper_docs::Topic], read: &Read) -> run::HostAnswer {
    let mut text = Text::new();
    match read {
        Read::Topic { name, from } => {
            for topic in table {
                if topic.name.as_bytes() == name.as_ref() {
                    text.add(topic.name.as_bytes());
                    text.add(b": ");
                    text.add(topic.title.as_bytes());
                    text.add(b"\n");
                    let page = temper_docs::page(topic, *from, PAGE_BYTES);
                    text.add(page.text.as_bytes());
                    if !page.text.is_empty() && !page.text.ends_with('\n') {
                        text.add(b"\n");
                    }
                    match page.next {
                        Some(next) => {
                            text.add(b"next: ");
                            text.add(Decimal::of(u64::from(next)).as_bytes());
                            text.add(b"\n");
                        }
                        None => text.add(b"end\n"),
                    }
                    return text.finish(false);
                }
            }
            text.add(b"no topic ");
            text.add(name);
            text.add(b"\n");
            let mut words = name.clone();
            for byte in &mut words {
                if *byte == b'-' {
                    *byte = b' ';
                }
            }
            search(table, &words, text)
        }
        Read::Search(words) => search(table, words, text),
    }
}

fn search(table: &'static [temper_docs::Topic], words: &[u8], mut text: Text) -> run::HostAnswer {
    let hits = match temper_docs::search(table, words, HITS) {
        Ok(hits) => hits,
        Err(refused) => return error(refused.says().as_bytes()),
    };
    for hit in &hits.hits {
        text.add(hit.topic.as_bytes());
        text.add(b"@");
        text.add(Decimal::of(u64::from(hit.offset)).as_bytes());
        text.add(b": ");
        text.add(hit.heading.as_bytes());
        text.add(b": ");
        text.add(hit.line.as_bytes());
        text.add(b"\n");
    }
    if hits.hits.is_empty() {
        text.add(b"nothing matches\n");
    } else if hits.more {
        text.add(b"more: narrow the words\n");
    }
    text.finish(false)
}

fn error(message: &[u8]) -> run::HostAnswer {
    let mut text = Text::new();
    text.add(message);
    text.add(b"\n");
    text.finish(true)
}

// Topic/page: 32 + 2 + 80 + 1 + 1536 + 1 + 12 = 1664.
// Search: 8 * (32 + 1 + 5 + 2 + 80 + 2 + 120 + 1) + 23 = 1967.
// Unknown topic: 9 + 32 + 1 + 1967 = 2009, below BODY_BYTES (2023).
// These are raw owned bytes, as the root counts them. Paging additionally
// limits the page's JSON-rendered bytes, including quotes and backslashes.
struct Text {
    bytes: List<u8>,
}
impl Text {
    fn new() -> Self {
        Self { bytes: List::with_capacity(BODY_BYTES) }
    }

    fn add(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.bytes.push(*byte).expect("documentation parts prove the answer bound");
        }
    }

    fn finish(self, error: bool) -> run::HostAnswer {
        run::HostAnswer::new(self.bytes.into_boxed(), error).expect("documentation fits the host answer")
    }
}
