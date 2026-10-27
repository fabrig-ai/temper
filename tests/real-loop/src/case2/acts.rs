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
    Roots,
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
    // A parent's reply proves its delegation committed before the next numbered
    // read. Keep every member's held piece in place throughout the checkpoint.
    for member in cast.members {
        for child in cast.members {
            if child.under == Some(member.name) {
                acts.push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--until", "replied"]));
                break;
            }
        }
    }
    for member in cast.members {
        acts.push(calls::writing_task(D1, cast.number(member.name)));
        acts.push(call(D1, argv!["task", "show", number(cast.number(member.name)), "--activity", "32", "--json"]));
    }
    acts.push(call(D1, argv!["task", "list", "--project", number(PROJECT), "--json"]));
    acts
}
#[must_use]
pub fn reads(cast: &Cast) -> Vec<(Read, usize)> {
    let mut reads = Vec::with_capacity(2 * cast.members.len() + 1);
    // sign-in, adoptions, chat, then each member's replied read.
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
    reads.push((Read::Roots, base + 2 * cast.members.len()));
    reads
}
#[must_use]
pub fn hour_0() -> Vec<calls::Step> {
    acts(&CAST)
}
