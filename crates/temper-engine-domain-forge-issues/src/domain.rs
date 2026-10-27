//! One level-triggered issue projection step (domain/forge.md, section 12).
//!
//! The top commits the returned `Projected` and `Effect` together. A body
//! digest and write time throttle only changed bodies; milestone keys are
//! retained independently and emitted once. No issue is read back, and the
//! symbolic goal on an effect is resolved to its issue by the top.
use alloc::boxed::Box;
use sha2::{Digest, Sha256};
use skein_lib::{List, Wall};

use crate::{Decision, Effect, GoalView, Key, Limits, MilestoneKey, Phase, Projected, Projection};

/// Project one goal from its current committed record and root facts.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one ordered projection decision: body, prefix, folded suffix, Finished, close"
)]
pub fn project(goal: &GoalView, last: &Projected, now: Wall, limits: &Limits) -> Projection {
    if last.milestones.len() > usize::try_from(limits.milestones).expect("u32 fits usize") || !valid_history(goal, last)
    {
        return held(last);
    }
    let mut fitted = goal.clone();
    fit_words(&mut fitted, limits);
    let goal = &fitted;
    let body = body(goal);
    let prefix = usize::try_from(limits.milestones.saturating_sub(2)).expect("u32 fits usize");
    let mut digest = Sha256::new();
    digest.update(goal.title.as_bytes());
    digest.update(&body);
    let hash: [u8; 32] = digest.finalize().into();
    if last.closed {
        let mut complete = has(&last.milestones, MilestoneKey::Finished);
        for milestone in goal.milestones.iter().take(prefix) {
            complete &= has(&last.milestones, milestone.key);
        }
        if let Some(milestone) = goal.milestones.last() {
            complete &= has(&last.milestones, milestone.key);
        }
        return if last.digest == Some(hash) && goal.finished.is_some() && complete {
            Projection { projected: last.clone(), decision: Decision::None }
        } else {
            held(last)
        };
    }
    if !last.opened {
        let mut next = last.clone();
        next.opened = true;
        next.digest = Some(hash);
        next.last_body = Some(now);
        return Projection {
            projected: next,
            decision: Decision::Effect(Effect::Open {
                goal: goal.goal,
                repository: goal.repository,
                key: Key::Open,
                title: goal.title.clone(),
                body,
            }),
        };
    }
    if last.digest != Some(hash) {
        let since = last.last_body.unwrap_or(now);
        let due = Wall::from_nanos(since.as_nanos().saturating_add(limits.interval.as_nanos()));
        if now >= due {
            let Some(revision) = last.body_revision.checked_add(1) else { return held(last) };
            let mut next = last.clone();
            next.digest = Some(hash);
            next.last_body = Some(now);
            next.body_revision = revision;
            return Projection {
                projected: next,
                decision: Decision::Effect(Effect::Body {
                    goal: goal.goal,
                    repository: goal.repository,
                    key: Key::Body(revision),
                    title: goal.title.clone(),
                    body,
                }),
            };
        }
    }
    for milestone in goal.milestones.iter().take(prefix) {
        if !has(&last.milestones, milestone.key) {
            return comment(goal, last, milestone.key, milestone.text.clone(), limits);
        }
    }
    if goal.finished.is_some() && goal.milestones.len() > prefix {
        let milestone = goal.milestones.last().expect("waiting suffix");
        if !has(&last.milestones, milestone.key) {
            // A gathered view already contains one counting entry. Direct
            // callers may still supply the whole waiting suffix.
            let words = if goal.milestones.len() == prefix.checked_add(1).expect("bounded prefix") {
                milestone.text.clone()
            } else {
                counted(
                    goal.milestones.len().checked_sub(prefix).expect("waiting suffix"),
                    &milestone.text,
                    limits.comment_bytes,
                )
            };
            return comment(goal, last, milestone.key, words, limits);
        }
    }
    if let Some(finished) = &goal.finished {
        if last.digest != Some(hash) {
            let since = last.last_body.unwrap_or(now);
            return Projection {
                projected: last.clone(),
                decision: Decision::Wait(Wall::from_nanos(since.as_nanos().saturating_add(limits.interval.as_nanos()))),
            };
        }
        if !has(&last.milestones, MilestoneKey::Finished) {
            return comment(goal, last, MilestoneKey::Finished, finished.clone(), limits);
        }
        let mut next = last.clone();
        next.closed = true;
        return Projection {
            projected: next,
            decision: Decision::Effect(Effect::Close { goal: goal.goal, repository: goal.repository, key: Key::Close }),
        };
    }
    if last.digest != Some(hash) {
        let since = last.last_body.unwrap_or(now);
        return Projection {
            projected: last.clone(),
            decision: Decision::Wait(Wall::from_nanos(since.as_nanos().saturating_add(limits.interval.as_nanos()))),
        };
    }
    Projection { projected: last.clone(), decision: Decision::None }
}

/// Retain immutable milestone history while replacing the goal and its whole plan.
#[must_use]
pub fn gather(prior: Option<&GoalView>, mut next: GoalView, limits: &Limits) -> Option<GoalView> {
    if !valid_history(&next, &Projected::default()) {
        return None;
    }
    // A retry of the stored desired view must not fold its counting entry again.
    if prior == Some(&next) {
        return Some(next);
    }
    if let Some(words) = &mut next.finished {
        *words = cut(words, usize::try_from(limits.comment_bytes).ok()?);
    }
    let prefix = usize::try_from(limits.milestones.checked_sub(2)?).ok()?;
    let mut milestones = List::with_capacity(limits.milestones);
    if let Some(prior) = prior {
        if prior.goal != next.goal
            || prior.repository != next.repository
            || (prior.finished.is_some() && prior.finished != next.finished)
        {
            return None;
        }
        for item in prior.milestones.iter().take(prefix) {
            milestones.push(item.clone()).ok()?;
        }
    }
    let mut waiting = 0usize;
    let mut last = None;
    if let Some(prior) = prior
        && let Some(item) = prior.milestones.get(prefix)
    {
        waiting = waiting_count(&item.text)?;
        last = Some(item.clone());
    }
    let previous_last = last.clone();
    let mut replaying = false;
    if let Some(previous) = &previous_last {
        for item in &next.milestones {
            replaying |= item.key == previous.key;
        }
    }
    for item in &next.milestones {
        let words = cut(&item.text, usize::try_from(limits.comment_bytes).ok()?);
        let mut found = false;
        for kept in &milestones {
            if kept.key == item.key {
                if kept.text != words {
                    return None;
                }
                found = true;
            }
        }
        if replaying {
            if let Some(previous) = &previous_last
                && previous.key == item.key
            {
                if previous.text != counted(waiting, &item.text, limits.comment_bytes) {
                    return None;
                }
                replaying = false;
            }
            continue;
        }
        if found {
            continue;
        }
        if usize::try_from(milestones.len()).ok()? < prefix {
            milestones.push(crate::Milestone { key: item.key, text: words }).ok()?;
        } else {
            waiting = waiting.checked_add(1)?;
            last = Some(crate::Milestone { key: item.key, text: counted(waiting, &item.text, limits.comment_bytes) });
        }
    }
    if let Some(last) = last {
        milestones.push(last).ok()?;
    }
    next.milestones = milestones.into_boxed();
    fit_words(&mut next, limits);
    Some(next)
}

/// The marker is included in the allowance, and the prefix ends on UTF-8.
#[expect(clippy::disallowed_methods, reason = "the shared cutter preserves already validated UTF-8")]
fn cut(words: &str, allowance: usize) -> Box<str> {
    if words.len() <= allowance {
        return Box::from(words);
    }
    let bytes = crate::text::fit(words.as_bytes(), allowance, "…".as_bytes());
    Box::from(core::str::from_utf8(&bytes).expect("fitted UTF-8 text"))
}

#[expect(clippy::disallowed_methods, reason = "rendering joins already validated UTF-8 text and fixed ASCII")]
fn boxed_text(text: List<u8>) -> Box<str> {
    let text = text.into_boxed();
    Box::from(core::str::from_utf8(&text).expect("rendered UTF-8 text"))
}

fn waiting_count(words: &str) -> Option<usize> {
    let mut count = 0usize;
    let mut digits = 0usize;
    for byte in words.as_bytes() {
        if !byte.is_ascii_digit() {
            break;
        }
        count = count.checked_mul(10)?.checked_add(usize::from(byte.checked_sub(b'0')?))?;
        digits = digits.checked_add(1)?;
    }
    if digits == 0 || !words.as_bytes().get(digits..)?.starts_with(b" more milestones; the last: ") {
        return None;
    }
    Some(count)
}

fn counted(count: usize, last: &str, bound: u32) -> Box<str> {
    // At most twenty decimal digits for a u64 history count.
    let mut digits = [b'0'; 20];
    let mut at = digits.len();
    let mut number = count;
    loop {
        at = at.checked_sub(1).expect("usize decimal width");
        *digits.get_mut(at).expect("decimal width") =
            b'0'.checked_add(u8::try_from(number % 10).expect("decimal digit")).expect("decimal ASCII");
        number /= 10;
        if number == 0 {
            break;
        }
    }
    let header = b" more milestones; the last: ";
    let room = usize::try_from(bound).expect("u32 fits usize");
    let prefix = digits.len().checked_sub(at).expect("decimal prefix").checked_add(header.len()).expect("fixed header");
    let mut text = List::with_capacity(bound.max(u32::try_from(prefix).expect("fixed header")));
    append(&mut text, digits.get(at..).expect("decimal prefix"));
    append(&mut text, header);
    let words = cut(last, room.saturating_sub(prefix));
    append(&mut text, words.as_bytes());
    cut(&boxed_text(text), room)
}

fn fit_words(goal: &mut GoalView, limits: &Limits) {
    goal.title = cut(&goal.title, usize::try_from(limits.title_bytes).expect("u32 fits usize"));
    let mut fixed = 9usize.checked_add(label(goal.phase).len()).expect("fixed syntax");
    let mut text = goal.goal_text.len();
    for item in &goal.plan {
        fixed = fixed
            .checked_add(10)
            .expect("bounded plan syntax")
            .checked_add(label(item.phase).len())
            .expect("phase label");
        text = text.checked_add(item.text.len()).expect("bounded plan words");
    }
    let room = usize::try_from(limits.body_bytes).expect("u32 fits usize").saturating_sub(fixed);
    if text <= room {
        return;
    }
    goal.goal_text = cut(&goal.goal_text, goal.goal_text.len().min(room / 2));
    let mut left = room.checked_sub(goal.goal_text.len()).expect("goal share fits room");
    let mut pending = goal.plan.len();
    let mut shares = List::with_capacity(u32::try_from(pending).expect("bounded plan"));
    for _ in &goal.plan {
        shares.push(None).expect("plan capacity");
    }
    let mut shares = shares.into_boxed();
    // Water fill: short items release their unused bytes to the remainder.
    loop {
        if pending == 0 {
            break;
        }
        let share = left.checked_div(pending).expect("pending items");
        let mut changed = false;
        for (item, assigned) in goal.plan.iter().zip(shares.iter_mut()) {
            if assigned.is_none() && item.text.len() <= share {
                *assigned = Some(item.text.len());
                left = left.checked_sub(item.text.len()).expect("short item fits share");
                pending = pending.checked_sub(1).expect("pending item");
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (item, assigned) in goal.plan.iter_mut().zip(shares.iter()) {
        let allowance = match assigned {
            Some(bytes) => *bytes,
            None => {
                let bytes = left.div_ceil(pending);
                left = left.checked_sub(bytes).expect("item fits remaining room");
                pending = pending.checked_sub(1).expect("pending item");
                bytes
            }
        };
        item.text = cut(&item.text, allowance);
    }
}

fn held(last: &Projected) -> Projection {
    Projection { projected: last.clone(), decision: Decision::Hold }
}

fn has(keys: &[MilestoneKey], key: MilestoneKey) -> bool {
    for item in keys {
        if *item == key {
            return true;
        }
    }
    false
}

fn valid_history(goal: &GoalView, last: &Projected) -> bool {
    for item in &goal.milestones {
        if item.key == MilestoneKey::Finished {
            return false;
        }
        let mut found = false;
        for previous in &goal.milestones {
            if previous.key == item.key {
                if found {
                    return false;
                }
                found = true;
            }
        }
    }
    for key in &last.milestones {
        if *key != MilestoneKey::Finished {
            let mut found = false;
            for item in &goal.milestones {
                if item.key == *key {
                    found = true;
                }
            }
            if !found {
                return false;
            }
        }
    }
    if goal.finished.is_none() && has(&last.milestones, MilestoneKey::Finished) {
        return false;
    }
    true
}

fn body(goal: &GoalView) -> Box<[u8]> {
    // Fixed syntax is never cut, even for a policy too small for its skeleton.
    let mut bytes = goal
        .goal_text
        .len()
        .checked_add(9)
        .expect("bounded goal words")
        .checked_add(label(goal.phase).len())
        .expect("phase label");
    for item in &goal.plan {
        bytes = bytes
            .checked_add(10)
            .expect("bounded plan syntax")
            .checked_add(item.text.len())
            .expect("bounded item words")
            .checked_add(label(item.phase).len())
            .expect("phase label");
    }
    let mut rendered = List::with_capacity(u32::try_from(bytes).expect("bounded issue body"));
    append(&mut rendered, goal.goal_text.as_bytes());
    append(&mut rendered, b"\nState: ");
    append(&mut rendered, label(goal.phase));
    rendered.push(b'\n').expect("body size checked");
    for item in &goal.plan {
        let done = match item.phase {
            Phase::Done | Phase::Failed | Phase::Cancelled => true,
            Phase::Waiting | Phase::Active | Phase::Settling | Phase::Held => false,
        };
        append(&mut rendered, if done { b"- [x] " } else { b"- [ ] " });
        append(&mut rendered, item.text.as_bytes());
        append(&mut rendered, b" (");
        append(&mut rendered, label(item.phase));
        append(&mut rendered, b")");
        rendered.push(b'\n').expect("body size checked");
    }
    rendered.into_boxed()
}

fn label(phase: Phase) -> &'static [u8] {
    match phase {
        Phase::Waiting => b"waiting",
        Phase::Active => b"active",
        Phase::Settling => b"settling",
        Phase::Held => b"held",
        Phase::Done => b"done",
        Phase::Failed => b"failed",
        Phase::Cancelled => b"cancelled",
    }
}

fn append(into: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        into.push(*byte).expect("body size checked");
    }
}

fn comment(goal: &GoalView, last: &Projected, key: MilestoneKey, words: Box<str>, limits: &Limits) -> Projection {
    let words = cut(&words, usize::try_from(limits.comment_bytes).expect("u32 fits usize"));
    if last.milestones.len() >= usize::try_from(limits.milestones).expect("u32 fits usize") {
        return held(last);
    }
    let mut keys = List::with_capacity(limits.milestones);
    for prior in &last.milestones {
        keys.push(*prior).expect("retained milestone bound checked");
    }
    keys.push(key).expect("new milestone has room");
    let mut next = last.clone();
    next.milestones = keys.into_boxed();
    Projection {
        projected: next,
        decision: Decision::Effect(Effect::Comment {
            goal: goal.goal,
            repository: goal.repository,
            key: Key::Milestone(key),
            body: words,
        }),
    }
}
