//! temper's calendar: each project's funding period, renewed on one span
//! (domain/root.md, 5.7). It keeps no money and stores nothing: the periods
//! are the core's records, and the root fills each period's budget.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

use skein_lib::{Duration, Env, Map, Queue, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub projects: u32,
}

/// One project's current period as the calendar last said or was told it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Record {
    pub period: u64,
    pub began: Wall,
    pub next: Option<Wall>,
}

#[derive(Debug)]
pub struct Calendar {
    span: Duration,
    projects: Map<u32, Record>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// A restored project's current period, before decisions open.
    Restored { project: u32, period: u64, began: Wall, next: Option<Wall> },
    /// A first period began without a next start.
    Began { project: u32, period: u64, began: Wall },
    /// The deadline fired: every project whose next start has come.
    Due,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    /// Give the current period its next start.
    Next { project: u32, period: u64, next: Wall },
    /// Open the next period at the decision's wall.
    Open { project: u32, period: u64, next: Wall },
}

impl Calendar {
    #[must_use]
    pub fn new(limits: &Limits, span: Duration) -> Calendar {
        assert!(span > Duration::ZERO, "a period's span is positive");
        Calendar { span, projects: Map::with_capacity(limits.projects) }
    }

    /// The earliest next start: the one deadline the root arms.
    #[must_use]
    pub fn next_start(&self) -> Option<Wall> {
        let mut next = None;
        for (_, record) in &self.projects {
            if let Some(wall) = record.next {
                next = Some(match next {
                    Some(previous) => wall.min(previous),
                    None => wall,
                });
            }
        }
        next
    }
}

/// One firing may renew every project.
#[must_use]
pub const fn max_out(limits: &Limits) -> u32 {
    limits.projects
}

pub fn step(calendar: &mut Calendar, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    assert!(out.room() >= max_out(&env.limits), "calendar output room reserved");
    match event {
        Event::Began { project, period, began } => {
            let next = after(began, calendar.span);
            keep(calendar, project, Record { period, began, next: Some(next) });
            out.push(Request::Next { project, period, next });
        }
        Event::Restored { project, period, began, next } => match next {
            None => {
                let next = after(began, calendar.span);
                keep(calendar, project, Record { period, began, next: Some(next) });
                out.push(Request::Next { project, period, next });
            }
            Some(next) if next <= env.wall => open(calendar, env.wall, project, period, env.wall, out),
            Some(_) => keep(calendar, project, Record { period, began, next }),
        },
        Event::Due => {
            // Copy one record before mutating the map; traverse once in project order.
            let mut first = 0;
            loop {
                let entry = calendar.projects.range(first..).next();
                let Some((&project, &record)) = entry else { break };
                if let Some(next) = record.next
                    && next <= env.wall
                {
                    let aligned = after(next, calendar.span);
                    let start = if aligned <= env.wall { env.wall } else { next };
                    open(calendar, env.wall, project, record.period, start, out);
                }
                let Some(after) = project.checked_add(1) else { break };
                first = after;
            }
        }
    }
}

fn after(wall: Wall, span: Duration) -> Wall {
    Wall::from_nanos(wall.as_nanos().saturating_add(span.as_nanos()))
}

fn keep(calendar: &mut Calendar, project: u32, record: Record) {
    calendar.projects.insert(project, record).expect("root checked jig's project bound");
}

fn open(calendar: &mut Calendar, wall: Wall, project: u32, period: u64, start: Wall, out: &mut Queue<Request>) {
    let period = period.checked_add(1).expect("period number fits u64");
    let next = after(start, calendar.span);
    keep(calendar, project, Record { period, began: wall, next: Some(next) });
    out.push(Request::Open { project, period, next });
}

#[cfg(test)]
mod tests;
