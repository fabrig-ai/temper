//! The person's opening; reads are named independently of task numbers.
use super::{CAST, Cast, PROJECT};
use crate::{
    argv,
    calls::{self, D1, call, number, sign_in},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    Writing(&'static str),
    Task(&'static str),
    Held(&'static str),
    RunStopped(&'static str),
    Inbox,
    Proposal,
    Waiting(&'static str),
    Resumed(&'static str),
    Parked(&'static str),
}
#[must_use]
pub fn acts(cast: &Cast) -> Vec<calls::Step> {
    let mut acts = Vec::with_capacity(3 + cast.repositories.len() + 3 * cast.members.len());
    acts.push(sign_in(D1));
    for home in [true, false] {
        for repository in cast.repositories {
            if repository.home != home {
                continue;
            }
            let mut args = argv![
                "repository",
                "adopt",
                "--project",
                number(PROJECT),
                "--owner",
                repository.owner,
                "--name",
                repository.name,
                "--landing",
                "main"
            ];
            if !repository.home {
                args.0.push(calls::Arg::Lit("--no-home"));
            }
            args.0.extend(argv!["--key", repository.name].0);
            acts.push(call(D1, args));
        }
    }
    acts.push(call(
        D1,
        argv!["chat", "start", "--project", number(PROJECT), "--key", "case2-hour-0", cast.members[0].spec],
    ));
    // A live delegate card proves creation before its numbered task read.
    // A held stream has not replied yet, so waiting for `replied` here would
    // wait for the very piece the checkpoint deliberately leaves unreleased.
    for member in cast.members {
        let mut last = None;
        for child in cast.members {
            if child.under == Some(member.name) {
                last = Some(child);
            }
        }
        if let Some(child) = last {
            acts.push(calls::until(
                D1,
                argv!["task", "show", number(cast.number(member.name)), "--json"],
                child.cue.as_bytes(),
                b"\"task\"",
            ));
        }
    }
    for member in cast.members {
        acts.push(calls::writing_task(D1, cast.number(member.name)));
        acts.push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--activity", "32", "--json"]));
    }
    acts
}
#[must_use]
pub fn reads(cast: &Cast) -> Vec<(Read, usize)> {
    let mut reads = Vec::with_capacity(2 * cast.members.len());
    // sign-in, adoptions, chat, then each parent's delegate-card read.
    let mut base = 2 + cast.repositories.len();
    for member in cast.members {
        for child in cast.members {
            if child.under == Some(member.name) {
                base += 1;
                break;
            }
        }
    }
    for (at, member) in cast.members.iter().enumerate() {
        reads.push((Read::Writing(member.name), base + 2 * at));
        reads.push((Read::Task(member.name), base + 2 * at + 1));
    }
    reads
}
#[must_use]
pub fn hour_0() -> Vec<calls::Step> {
    let mut steps = acts(&CAST);
    steps.extend(close(&CAST));
    steps
}

/// After the checkpoint (or a later step), end the chat's subtree through ctl.
/// The harness then checks ordinary completion and drains every hosted child.
#[must_use]
pub fn close(cast: &Cast) -> Vec<calls::Step> {
    let director = cast.number(cast.members[0].name);
    let mut steps = vec![
        call(
            D1,
            argv!["task", "cancel", number(director), "The case's checkpoint is complete.", "--key", "case2-close"],
        ),
        call(D1, argv!["task", "show", number(director), "--until", "ended"]),
    ];
    for member in cast.members {
        steps.push(calls::release_hold(member.cue.as_bytes()));
    }
    steps
}

/// Append 8.1 to the opening checkpoint, then drain the ordinary hour-0 work.
#[must_use]
pub fn step_8_1() -> Vec<calls::Step> {
    let mut steps = acts(&CAST);
    steps.extend(pause(&CAST));
    steps.extend(close(&CAST));
    steps
}
fn pause(cast: &Cast) -> Vec<calls::Step> {
    use super::scripts::{DIRECTION, PROPOSAL};
    let mut steps = Vec::with_capacity(12 + 7 * cast.members.len());
    steps.push(call(D1, argv!["task", "stop", number(cast.number("P")), "--key", "case2-stop-p"]));
    // Stop has been acknowledged. Release the canceled fake streams so they can
    // drain while the following reads verify the held tasks and retired attempts.
    for member in &cast.members[1..] {
        steps.push(calls::release_hold(member.cue.as_bytes()));
    }
    for member in &cast.members[1..] {
        steps.push(calls::until(
            D1,
            argv!["task", "show", number(cast.number(member.name)), "--activity", "32", "--json"],
            b"\"run_ended\"",
            b"\"task\"",
        ));
        steps.push(call(D1, argv!["task", "run", number(cast.number(member.name)), "--json"]));
    }
    // The director was never stopped. Finish its held response and park before
    // delivering the person's direction, so the next scripted turn sees it.
    steps.push(calls::release_hold(cast.members[0].cue.as_bytes()));
    steps.push(call(D1, argv!["task", "show", number(cast.number("director")), "--until", "phase:idle"]));
    steps.push(call(
        D1,
        argv!["task", "message", number(cast.number("director")), DIRECTION, "--key", "case2-use-slots"],
    ));
    steps.push(call(D1, argv!["inbox", "--until", "waiting", "--json"]));
    steps.push(call(D1, argv!["proposal", "show", PROPOSAL, "--json"]));
    for member in cast.members {
        steps.push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--json"]));
    }
    steps.push(call(D1, argv!["proposal", "decide", PROPOSAL, "accept", "--key", "case2-accept"]));
    for member in &cast.members[1..] {
        steps.push(calls::writing_task(D1, cast.number(member.name)));
        steps.push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--activity", "32", "--json"]));
    }
    // Let the implementers' stock waiting periods overlap. Their scripts all
    // finish before the unchanged world patience, without changing any period.
    for member in &cast.members[1..] {
        steps.push(calls::release_hold(member.cue.as_bytes()));
    }
    for member in &cast.members[1..] {
        steps
            .push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--until", "phase:idle", "--json"]));
    }
    steps
}
#[must_use]
pub fn step_8_1_reads(cast: &Cast) -> Vec<(Read, usize)> {
    let mut result = Vec::with_capacity(8 * cast.members.len() + 2);
    result.extend(reads(cast));
    let mut at = result.last().expect("opening reads").1 + 2; // Stop follows the last opening read.
    for member in &cast.members[1..] {
        result.push((Read::Held(member.name), at));
        result.push((Read::RunStopped(member.name), at + 1));
        at += 2;
    }
    at += 2; // director idle, direction
    result.push((Read::Inbox, at));
    result.push((Read::Proposal, at + 1));
    at += 2;
    for member in cast.members {
        result.push((Read::Waiting(member.name), at));
        at += 1;
    }
    at += 1; // acceptance
    for member in &cast.members[1..] {
        result.push((Read::Resumed(member.name), at + 1));
        at += 2;
    }
    for member in &cast.members[1..] {
        result.push((Read::Parked(member.name), at));
        at += 1;
    }
    result
}
