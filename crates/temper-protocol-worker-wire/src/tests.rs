//! Domain bounds and identity field rules (protocol/channel.md, sections 6 and 8).
mod fixtures;
use crate::v1;
use alloc::boxed::Box;
use skein_lib::List;

#[test]
fn limits_take_each_bound_from_its_source_field() {
    assert_eq!(
        fixtures::limits(),
        v1::Limits {
            merge_branch: 16,
            branch_branch: 16,
            writable_push: 16,
            writable_landing_some: 16,
            repository_name: 16,
            repository_remote: 48,
            repository_git_some: 4096,
            items_repositories: 2,
            items_save_some: 16,
            left_landed: 2,
            files_files: 2,
            files_files_item: 8,
            diagnostic_output: 512,
            saved_repositories: 2,
            token_token: 40,
            entries_entries: 3,
            entry_token: 24,
            entry_account_login: 16,
            identity_login: 16,
            identity_name: 16,
            identity_email: 16,
        }
    );
    let bounds = crate::Bounds { repositories: 65, ..fixtures::BOUNDS };
    assert_eq!(crate::limits(&bounds), None, "past the repository ceiling");
}

#[test]
fn items_one_repository_past_their_bound_are_refused() {
    let limits = fixtures::limits();
    v1::Items::new(&limits, fixtures::items(2)).expect("at the bound");
    let problem = v1::Items::new(&limits, fixtures::items(3)).expect_err("past the count");
    assert_eq!(problem, v1::Problem { path: v1::Path::ItemsRepositories, reason: skein_codec::Reason::Bound });
}

#[test]
fn repository_byte_fields_stop_at_their_bounds() {
    let limits = fixtures::limits();
    for (program, bound, path) in
        [(false, 48_usize, v1::Path::RepositoryRemote), (true, 4096_usize, v1::Path::RepositoryGit)]
    {
        for past in [false, true] {
            let length = if past { bound.checked_add(1).expect("test length") } else { bound };
            let mut parts = fixtures::repository_parts(1, Box::from(&b"remote"[..]));
            let mut bytes = skein_lib::bytes::zeroed(length);
            bytes.fill(b'x');
            if program {
                parts.git = Some(bytes);
            } else {
                parts.remote = bytes;
            }
            let made = v1::Repository::new(&limits, parts);
            if past {
                assert_eq!(made.expect_err("past bound"), v1::Problem { path, reason: skein_codec::Reason::Bound });
            } else {
                made.expect("at bound");
            }
        }
    }
}

#[test]
fn saved_conflicts_past_their_count_are_refused() {
    let mut files = List::with_capacity(3);
    for _ in 0_u32..3 {
        files.push(Box::from(&b"file"[..])).expect("counted room");
    }
    let conflicts = v1::Files::new(&v1::CEILINGS, v1::FilesParts { files }).expect("within ceilings");
    let repository = v1::SavedRepository::new(
        &v1::CEILINGS,
        v1::SavedRepositoryParts { tag: 1, from_saved: false, landing: v1::Landing::Conflicted(conflicts) },
    )
    .expect("within ceilings");
    let mut repositories = List::with_capacity(1);
    repositories.push(repository).expect("counted room");
    let problem =
        v1::Saved::new(&fixtures::limits(), v1::SavedParts { repositories }).expect_err("past the conflicts count");
    assert_eq!(problem, v1::Problem { path: v1::Path::FilesFiles, reason: skein_codec::Reason::Bound });
}

#[test]
fn largest_parts_are_the_generated_worst_cases() {
    let limits = fixtures::limits();
    let sizes = crate::largest(&limits).expect("test limits fit");
    assert_eq!(u64::from(sizes.items), v1::Items::worst_case_bytes(&limits).expect("items"));
    assert_eq!(u64::from(sizes.left), v1::Left::worst_case_bytes(&limits).expect("left"));
    assert_eq!(u64::from(sizes.saved), v1::Saved::worst_case_bytes(&limits).expect("saved"));
    assert_eq!(u64::from(sizes.resource), v1::Preparation::worst_case_bytes(&limits).expect("preparation"));
    assert_eq!(u64::from(sizes.grant), v1::Identity::worst_case_bytes(&limits).expect("identity"));
    assert_eq!(sizes, crate::Largest { items: 8669, left: 84, saved: 1084, resource: 10, grant: 233 });
}

#[test]
fn identity_rules_refuse_each_forbidden_byte() {
    let original = fixtures::identity(b"forge-token");
    assert!(crate::valid(&original), "ordinary identity");
    let rows: [(&[u8], &[u8], &[u8]); 12] = [
        (b"a:b", b"name", b"email"),
        (b"a\x7f", b"name", b"email"),
        (b"login", b"", b"email"),
        (b"login", b"name", b""),
        (b"login", b"a\0b", b"email"),
        (b"login", b"a\nb", b"email"),
        (b"login", b"a<b", b"email"),
        (b"login", b"a>b", b"email"),
        (b"login", b"name", b"a\0b"),
        (b"login", b"name", b"a\nb"),
        (b"login", b"name", b"a<b"),
        (b"login", b"name", b"a>b"),
    ];
    for kind in [
        original.kind().clone(),
        v1::Kind::Github(
            v1::Entries::new(&fixtures::limits(), v1::EntriesParts { entries: List::with_capacity(0) })
                .expect("empty entries"),
        ),
    ] {
        for (row, (login, name, email)) in rows.iter().enumerate() {
            let parts = v1::IdentityParts {
                kind: kind.clone(),
                login: Box::from(*login),
                name: Box::from(*name),
                email: Box::from(*email),
            };
            let identity = v1::Identity::new(&fixtures::limits(), parts).expect("bounded valid text");
            assert!(!crate::valid(&identity), "forbidden row {row}");
        }
    }
}

#[test]
fn a_github_values_entries_stop_at_their_bound() {
    let limits = fixtures::limits();
    for (count, accepted) in
        [(limits.entries_entries, true), (limits.entries_entries.checked_add(1).expect("test count"), false)]
    {
        let mut entries = List::with_capacity(count);
        for account in 0..count {
            entries
                .push(
                    v1::Entry::new(
                        &limits,
                        v1::EntryParts {
                            account: u64::from(account),
                            account_login: Box::from(&b"owner"[..]),
                            token: skein_lib::Secret::new(Box::from(&b"token"[..])),
                        },
                    )
                    .expect("entry"),
                )
                .expect("counted room");
        }
        let kind = v1::Kind::Github(v1::Entries::new(&v1::CEILINGS, v1::EntriesParts { entries }).expect("ceiling"));
        let parts = v1::IdentityParts { kind, ..fixtures::identity(b"token").into_parts() };
        let made = v1::Identity::new(&limits, parts);
        if accepted {
            made.expect("at bound");
        } else {
            assert_eq!(
                made.expect_err("past bound"),
                v1::Problem { path: v1::Path::EntriesEntries, reason: skein_codec::Reason::Bound }
            );
        }
    }
}

#[test]
fn an_entrys_token_and_login_stop_at_their_bounds() {
    let limits = fixtures::limits();
    for (field, bound, path) in [
        ("token", limits.entry_token, v1::Path::EntryToken),
        ("login", limits.entry_account_login, v1::Path::EntryAccountLogin),
    ] {
        for past in [false, true] {
            let length = usize::try_from(bound.checked_add(u32::from(past)).expect("test bound")).expect("test length");
            let mut bytes = skein_lib::bytes::zeroed(length);
            bytes.fill(b'x');
            let mut parts = v1::EntryParts {
                account: 7,
                account_login: Box::from(&b"owner"[..]),
                token: skein_lib::Secret::new(Box::from(&b"token"[..])),
            };
            if field == "token" {
                parts.token = skein_lib::Secret::new(bytes);
            } else {
                parts.account_login = bytes;
            }
            let made = v1::Entry::new(&limits, parts);
            if past {
                assert_eq!(
                    made.expect_err("past bound"),
                    v1::Problem { path, reason: skein_codec::Reason::Bound },
                    "{field}"
                );
            } else {
                made.expect("at bound");
            }
        }
    }
}

#[test]
fn landing_branch_uses_the_name_bound_and_largest_items_count_it() {
    let limits = fixtures::limits();
    for length in [16, 17] {
        let parts = v1::WritableParts {
            push: Box::from(&b"topic"[..]),
            expected: None,
            landing: Some(skein_lib::bytes::zeroed(length)),
        };
        let value = v1::Writable::new(&limits, parts);
        if length == 16 {
            let value = value.expect("at bound");
            let mut bytes = skein_lib::Writer::new(usize::try_from(value.measure()).unwrap());
            value.encode(&mut bytes).unwrap();
            assert_eq!(v1::Writable::decode(&limits, &mut skein_lib::Reader::new(&bytes.finish())).unwrap(), value);
        } else {
            assert_eq!(value.expect_err("past bound").path, v1::Path::WritableLanding);
        }
    }
    let without = v1::Limits { writable_landing_some: 0, ..limits };
    assert_eq!(
        crate::largest(&limits).unwrap().items - crate::largest(&without).unwrap().items,
        limits.items_repositories * limits.writable_landing_some
    );
}
