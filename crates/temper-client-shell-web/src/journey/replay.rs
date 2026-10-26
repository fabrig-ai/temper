//! Replay and prove against the shipped host (client/journeys.md, 4).
use super::{
    Facts, Line, Unreadable,
    line::{clone_act, clone_heard},
    lines,
    record::{asked, is_ask},
};
use crate::host::{Host, Report};
use skein_ui::{NodeId, Tree};
use skein_ui_web::{page::Page, store::Record, transport::Carried};

/// A recording given back to the shipped host (client/journeys.md, 4.1).
pub struct Replay {
    lines: Vec<Line>,
    at: usize,
    turn: u32,
    marks: u32,
    host: Host,
    page: Page<'static>,
    in_turn: bool,
    departed: bool,
    ended: bool,
}
/// One mark: the page as the person saw it.
pub struct Mark<'a> {
    pub number: u32,
    pub page: &'a Page<'static>,
    pub tree: &'a Tree,
    pub acted: Vec<NodeId>,
    pub lead: super::Lead,
    pub recorded: Option<&'a [u8]>,
    pub end: bool,
}
/// Where a replay stopped (client/journeys.md, 4.2).
#[derive(Debug, PartialEq, Eq)]
pub struct Stale {
    pub turn: u32,
    pub line: u32,
    pub recorded: String,
    pub asked: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Unreadable(Unreadable),
    Stale(Stale),
    Unproved { mark: u32 },
    Differs { mark: u32, byte: usize },
}
impl Replay {
    /// Read every line and start from the ordered facts.
    ///
    /// # Errors
    /// Refuses malformed records, facts, versions and a missing final end.
    pub fn new(recording: &[u8]) -> Result<Self, Refusal> {
        let (lines, facts, at) = read(recording).map_err(Refusal::Unreadable)?;
        let (host, page) = facts.start();
        Ok(Self { lines, at, turn: 0, marks: 0, host, page, in_turn: false, departed: false, ended: false })
    }
    /// The replayed host, to read once the replay has ended.
    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }
    /// Run until the next mark, returning None after the end mark.
    ///
    /// # Errors
    /// Stops at the first changed duty, deadline or missing control.
    #[expect(clippy::should_implement_trait, reason = "a mark borrows the replay, unlike Iterator::Item")]
    pub fn next(&mut self) -> Result<Option<Mark<'_>>, Stale> {
        if self.ended {
            return Ok(None);
        }
        loop {
            let Some(line) = self.lines.get(self.at) else {
                return Err(self.stale(self.at, "nothing".into()));
            };
            if self.departed && !matches!(line, Line::End(_)) {
                return Err(self.stale(self.at, "end".into()));
            }
            if let Line::Mark(html) | Line::End(html) = line {
                let end = matches!(line, Line::End(_));
                let acted = if end {
                    Vec::new()
                } else if let Some(Line::Act { act, event, .. }) = self.lines.get(self.at + 1) {
                    super::giving(&self.host, act, event)
                } else {
                    Vec::new()
                };
                let lead = match self.lines.get(self.at + 1) {
                    Some(Line::Act { act: skein_ui::Act::Submit, .. }) => super::Lead::Submit,
                    Some(Line::Act { act: skein_ui::Act::Toggle, .. }) => super::Lead::Toggle,
                    _ => super::Lead::Press,
                };
                self.at += 1;
                self.marks += 1;
                self.ended = end;
                return Ok(Some(Mark {
                    number: self.marks,
                    page: &self.page,
                    tree: self.host.tree(),
                    acted,
                    lead,
                    recorded: html.as_deref(),
                    end,
                }));
            }
            if let Line::Turn { now, wall } = line {
                if self.in_turn {
                    return Err(self.stale(self.at, "deadline".into()));
                }
                self.host.clocks(*now, *wall);
                self.turn += 1;
                self.in_turn = true;
                self.at += 1;
                continue;
            }
            if !self.in_turn {
                return Err(self.stale(self.at, "nothing".into()));
            }
            if let Line::Deadline(_) = line {
                self.host.draw(&mut self.page, &mut Vec::new());
                let actual = Line::Deadline(self.host.deadline());
                if &actual != line {
                    return Err(self.stale(self.at, text(&actual)));
                }
                self.in_turn = false;
                self.at += 1;
                continue;
            }
            let (report, at) = self.report()?;
            self.at = at;
            let duties = self.host.report(report);
            self.departed = duties.departs();
            for actual in asked(&duties) {
                if self.lines.get(self.at) != Some(&actual) {
                    return Err(self.stale(self.at, text(&actual)));
                }
                self.at += 1;
            }
            if self.lines.get(self.at).is_some_and(is_ask) {
                return Err(self.stale(self.at, "nothing".into()));
            }
        }
    }
    fn report(&self) -> Result<(Report, usize), Stale> {
        let mut at = self.at;
        let line = &self.lines[at];
        let report = match line {
            Line::Start => Report::Start,
            Line::Due => Report::Due,
            Line::Place(path) => Report::Place { path: path.clone() },
            Line::Heard(heard) => Report::Heard(clone_heard(heard)),
            Line::Act { act, event, .. } => Report::Act {
                node: super::giving(&self.host, act, event)
                    .first()
                    .copied()
                    .ok_or_else(|| self.stale(at, "no control gives the event".into()))?,
                act: clone_act(act),
            },
            Line::Carried { writes } => {
                let writes = *writes;
                at += 1;
                let mut reports = Vec::new();
                while let Some(line) = self.lines.get(at) {
                    let report = if let Line::Written(key) = line {
                        Carried::Written { key: key.clone() }
                    } else if let Line::Unwritten(key) = line {
                        Carried::Refused { key: key.clone() }
                    } else if let Line::Lost(exchange) = line {
                        Carried::Failed { exchange: *exchange }
                    } else {
                        break;
                    };
                    reports.push(report);
                    at += 1;
                }
                return Ok((Report::Carried { reports, writes }, at));
            }
            Line::Journey { .. }
            | Line::Seed(_)
            | Line::Clocks { .. }
            | Line::Primary(_)
            | Line::Ways { .. }
            | Line::Forge(_)
            | Line::Path(_)
            | Line::Secret(_)
            | Line::Stored { .. }
            | Line::Turn { .. }
            | Line::Written(_)
            | Line::Unwritten(_)
            | Line::Lost(_)
            | Line::Exchange { .. }
            | Line::Open { .. }
            | Line::Close
            | Line::Save { .. }
            | Line::Erase(_)
            | Line::Writes(_)
            | Line::Navigate(_)
            | Line::Replace(_)
            | Line::Show { .. }
            | Line::Deadline(_)
            | Line::Mark(_)
            | Line::End(_) => return Err(self.stale(at, "nothing".into())),
        };
        at += 1;
        Ok((report, at))
    }
    fn stale(&self, at: usize, asked: String) -> Stale {
        Stale {
            turn: self.turn,
            line: number(at),
            recorded: self.lines.get(at).map_or_else(|| "nothing".into(), text),
            asked,
        }
    }
}
// Share the recording envelope without replaying: an old walk may be stale.
pub(super) fn read(recording: &[u8]) -> Result<(Vec<Line>, Facts, usize), Unreadable> {
    let lines = lines(recording)?;
    let (facts, at) = facts(&lines)?;
    if !matches!(lines.last(), Some(Line::End(_))) {
        return Err(Unreadable { line: number(lines.len()), why: "missing end" });
    }
    if let Some(at) = lines[..lines.len() - 1].iter().position(|line| matches!(line, Line::End(_))) {
        return Err(Unreadable { line: number(at), why: "end must be last" });
    }
    Ok((lines, facts, at))
}
fn number(index: usize) -> u32 {
    u32::try_from(index + 1).expect("recording line count fits u32")
}
fn text(line: &Line) -> String {
    let mut bytes = Vec::new();
    line.write(&mut bytes);
    bytes.pop();
    String::from_utf8(bytes).expect("recording text is ASCII")
}
fn facts(lines: &[Line]) -> Result<(Facts, usize), Unreadable> {
    let mut at = 0;
    let error = |at| Unreadable { line: number(at), why: "expected ordered start facts for journey 2" };
    if lines.get(at) != Some(&Line::Journey { version: 2 }) {
        return Err(error(at));
    }
    at += 1;
    let Some(Line::Seed(seed)) = lines.get(at) else {
        return Err(error(at));
    };
    at += 1;
    let Some(Line::Clocks { now, wall }) = lines.get(at) else {
        return Err(error(at));
    };
    at += 1;
    let Some(Line::Primary(primary)) = lines.get(at) else {
        return Err(error(at));
    };
    at += 1;
    let ways = if let Some(Line::Ways { content, origin }) = lines.get(at) {
        at += 1;
        Some((content.clone(), origin.clone()))
    } else {
        None
    };
    let mut forges = Vec::new();
    while let Some(Line::Forge(content)) = lines.get(at) {
        forges.push(content.clone());
        at += 1;
    }
    let Some(Line::Path(path)) = lines.get(at) else {
        return Err(error(at));
    };
    at += 1;
    let secret = if let Some(Line::Secret(secret)) = lines.get(at) {
        at += 1;
        Some(secret.clone())
    } else {
        None
    };
    let mut stored = Vec::new();
    while let Some(Line::Stored { key, value }) = lines.get(at) {
        stored.push(Record { key: key.clone(), value: value.clone() });
        at += 1;
    }
    if !matches!(lines.get(at), Some(Line::Turn { .. } | Line::Mark(_) | Line::End(_))) {
        return Err(error(at));
    }
    Ok((
        Facts {
            seed: *seed,
            now: *now,
            wall: *wall,
            primary: *primary,
            ways,
            forges,
            path: path.clone(),
            secret,
            stored,
        },
        at,
    ))
}
/// Prove every live print, then drop its html (client/journeys.md, 4.3).
///
/// # Errors
/// Refuses a stale replay, a bare mark or the first differing html byte.
pub fn prove(recording: &[u8]) -> Result<Box<[u8]>, Refusal> {
    let mut replay = Replay::new(recording)?;
    while let Some(mark) = replay.next().map_err(Refusal::Stale)? {
        let recorded = mark.recorded.ok_or(Refusal::Unproved { mark: mark.number })?;
        let html = mark.page.html();
        let actual = html.as_bytes();
        if actual != recorded {
            let byte =
                actual.iter().zip(recorded).position(|(a, b)| a != b).unwrap_or(actual.len().min(recorded.len()));
            return Err(Refusal::Differs { mark: mark.number, byte });
        }
    }
    let mut out = Vec::new();
    for mut line in replay.lines {
        if let Line::Mark(html) | Line::End(html) = &mut line {
            *html = None;
        }
        line.write(&mut out);
    }
    Ok(out.into())
}
