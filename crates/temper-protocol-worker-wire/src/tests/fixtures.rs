use crate::{Bounds, v1};
use alloc::boxed::Box;
use skein_lib::{List, Secret};

pub const BOUNDS: Bounds = Bounds {
    repositories: 2,
    name_bytes: 16,
    remote_bytes: 48,
    conflicts: 2,
    path_bytes: 8,
    token_bytes: 40,
    entries: 3,
    entry_token_bytes: 24,
    text_bytes: 16,
};

pub fn limits() -> v1::Limits {
    crate::limits(&BOUNDS).expect("test bounds within ceilings")
}

pub fn repository_parts(tag: u64, remote: Box<[u8]>) -> v1::RepositoryParts {
    let limits = limits();
    let branch = v1::Branch::new(&limits, v1::BranchParts { branch: Box::from(&b"main"[..]) }).expect("base branch");
    let writable = v1::Writable::new(
        &limits,
        v1::WritableParts { push: Box::from(&b"temper/work"[..]), expected: None, landing: None },
    )
    .expect("push branch");
    v1::RepositoryParts {
        tag,
        name: Box::from(&b"shop"[..]),
        remote,
        git: None,
        start: v1::Start::Base(branch),
        access: v1::Access::Writable(writable),
        identity: 1,
        account: None,
    }
}

pub fn items(count: u32) -> v1::ItemsParts {
    let limits = limits();
    let mut repositories = List::with_capacity(count);
    for tag in 0..count {
        let repository = v1::Repository::new(
            &limits,
            repository_parts(u64::from(tag), Box::from(&b"https://forge/acme/shop.git"[..])),
        )
        .expect("repository");
        repositories.push(repository).expect("counted room");
    }
    v1::ItemsParts { repositories, save: None }
}

pub fn identity(token: &[u8]) -> v1::Identity {
    v1::Identity::new(
        &limits(),
        v1::IdentityParts {
            kind: v1::Kind::Forgejo(
                v1::Token::new(&limits(), v1::TokenParts { token: Secret::new(Box::from(token)) }).expect("token"),
            ),
            login: Box::from(&b"temper"[..]),
            name: Box::from(&b"Temper"[..]),
            email: Box::from(&b"t@forge.example"[..]),
        },
    )
    .expect("test identity")
}
