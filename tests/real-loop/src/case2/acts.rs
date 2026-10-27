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
