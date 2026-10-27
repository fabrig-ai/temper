//! CI cuts preserve whole check heads and spend the remaining room on log tails
//! (domain/forge.md, 8.4). Retained structure also governs a parent's later cut.
use alloc::boxed::Box;
use skein_lib::{Decimal, List, Writer};

use crate::{BriefCommit, held};

const START: &[u8] = b"Repair the failed check at change head ";
const AFTER_HEAD: &[u8] = b". Push a new head and wait for checks there.\n";
const END: &[u8] = b"Repair the failed check.\n";

#[derive(Debug)]
pub(crate) struct Check {
    head: Box<[u8]>,
    log: Box<[u8]>,
    truncated: bool,
}

#[derive(Debug)]
pub(crate) struct Section {
    head: BriefCommit,
    checks: List<Check>,
    omitted: u32,
    budget: u32,
}

impl Section {
    pub(crate) fn new(head: BriefCommit, checks: u32, budget: u32) -> Self {
        Self { head, checks: List::with_capacity(checks), omitted: 0, budget }
    }

    pub(crate) fn has_omissions(&self) -> bool {
        self.omitted > 0
    }

    pub(crate) fn omit(&mut self) {
        self.omitted = self.omitted.saturating_add(1);
    }

    pub(crate) fn push(&mut self, head: Box<[u8]>, log: Box<[u8]>, truncated: bool) {
        let room = usize::try_from(self.budget).expect("u32 fits usize");
        let mut heads = head.len();
        let mut logs = log.len();
        for check in &self.checks {
            heads = heads.saturating_add(check.head.len());
            logs = logs.saturating_add(check.log.len());
        }
        if heads > room {
            self.omit();
            return;
        }
        self.checks.push(Check { head, log, truncated }).expect("one record per bounded check");
        if heads.saturating_add(logs) <= room {
            return;
        }
        // New heads take room from already gathered logs. Keep the payload
        // within brief_bytes even before rendering, marking any removed prefix.
        let mut remaining = room.saturating_sub(heads);
        let count = self.checks.len();
        for index in 0..count {
            let check = self.checks.get_mut(index).expect("bounded check");
            let share = remaining
                .checked_div(usize::try_from(count.saturating_sub(index)).expect("u32 fits usize"))
                .expect("positive remaining count");
            let tail = held::edge(&check.log, check.log.len().min(share), true);
            let lost = check.log.len().saturating_sub(tail.len());
            remaining = remaining.saturating_sub(tail.len());
            if lost > 0 {
                check.truncated = true;
                check.log = Box::from(tail);
            }
        }
    }

    pub(crate) fn render(&self, budget: u32) -> Option<Box<[u8]>> {
        let room = usize::try_from(budget).ok()?;
        let all = self.omitted.checked_add(self.checks.len())?;
        // At tiny budgets even the framing can be too large. Still tell the
        // reader how many whole failures could not be carried.
        if room
            < START
                .len()
                .checked_add(16)?
                .checked_add(AFTER_HEAD.len())?
                .checked_add(END.len())?
                .checked_add(omission_size(all))?
        {
            let size = omission_size(all);
            if all == 0 || size > room {
                return None;
            }
            let mut writer = Writer::new(size);
            writer.put(b"[").expect("measured omission");
            writer.put(Decimal::of(u64::from(all)).as_bytes()).expect("measured count");
            writer.put(b" failed checks omitted]\n").expect("measured omission");
            return Some(writer.finish());
        }
        let prefix = self.prefix();
        let fixed = prefix.len().checked_add(END.len())?;
        let total = self.checks.len();
        let mut count = 0_u32;
        let mut heads = fixed;
        // Reserve the omission notice and each log's cut marker before admitting
        // a check. No budget can turn part of its head into a misleading tail.
        for check in &self.checks {
            let next = count.checked_add(1)?;
            let cost = heads.checked_add(check.minimum())?;
            let omitted = self.omitted.checked_add(total.checked_sub(next)?)?;
            if cost.checked_add(omission_size(omitted))? > room {
                break;
            }
            heads = cost;
            count = next;
        }
        let omitted = self.omitted.checked_add(total.checked_sub(count)?)?;
        let mut remaining = room.checked_sub(heads.checked_add(omission_size(omitted))?)?;
        let mut writer = List::with_capacity(budget);
        put(&mut writer, &prefix);
        for index in 0..count {
            let check = self.checks.get(index).expect("admitted check");
            put(&mut writer, &check.head);
            // Share spare room, returning unused room to the checks still to go.
            let extra = remaining.checked_div(usize::try_from(count.checked_sub(index)?).ok()?)?;
            let minimum = check.minimum_log();
            let log =
                held::render_log(check.log.clone(), check.truncated, u32::try_from(minimum.checked_add(extra)?).ok()?)?;
            remaining = remaining.checked_sub(log.len().saturating_sub(minimum))?;
            put(&mut writer, &log);
            if check.has_log() {
                put(&mut writer, b"\n");
            }
        }
        if omitted > 0 {
            put(&mut writer, b"[");
            put(&mut writer, Decimal::of(u64::from(omitted)).as_bytes());
            put(&mut writer, b" failed checks omitted]\n");
        }
        put(&mut writer, END);
        Some(writer.into_boxed())
    }

    fn prefix(&self) -> Box<[u8]> {
        let mut writer = Writer::new(START.len().saturating_add(16).saturating_add(AFTER_HEAD.len()));
        writer.put(START).expect("fixed prefix");
        for byte in self.head.0.get(..8).expect("head prefix") {
            writer.put(&crate::brief::hex(*byte)).expect("fixed head");
        }
        writer.put(AFTER_HEAD).expect("fixed prefix");
        writer.finish()
    }
}

impl Check {
    fn has_log(&self) -> bool {
        !self.log.is_empty() || self.truncated
    }

    fn minimum_log(&self) -> usize {
        if self.truncated { held::LOG_CUT.len() } else { self.log.len().min(held::LOG_CUT.len()) }
    }

    fn minimum(&self) -> usize {
        self.head.len().saturating_add(self.minimum_log()).saturating_add(usize::from(self.has_log()))
    }
}

fn omission_size(count: u32) -> usize {
    if count == 0 {
        0
    } else {
        1_usize
            .saturating_add(Decimal::of(u64::from(count)).as_bytes().len())
            .saturating_add(b" failed checks omitted]\n".len())
    }
}

fn put(out: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        out.push(*byte).expect("measured CI section");
    }
}
