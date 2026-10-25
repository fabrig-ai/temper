use super::fixtures;
use crate::translate;
use skein_lib::List;
use temper_engine_domain as root;
use temper_protocol_worker_wire::v1 as wire;

#[test]
fn a_workspace_reaches_items_with_each_remote_made_from_its_base() {
    let assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    let limits = fixtures::wire_limits();
    for (base, remote) in [
        (b"https://forge.example".as_slice(), b"https://forge.example/acme/shop.git".as_slice()),
        (b"https://other.example/root".as_slice(), b"https://other.example/root/acme/shop.git".as_slice()),
    ] {
        let mut forge = fixtures::engine().forges.into_iter().next().unwrap();
        forge.git = base.into();
        let items = translate::items(&assignment, &[forge], &crate::forge::tokens::Tokens::empty(), &limits)
            .unwrap()
            .unwrap()
            .into_parts();
        let first = items.repositories.into_boxed().into_iter().next().unwrap().into_parts();
        assert_eq!(first.tag, temper_engine_domain_forge::tag(1, 7), "{base:?}");
        assert_eq!(first.remote.as_ref(), remote, "{base:?}");
        assert_eq!(first.git, None, "a forge without its own program uses the worker's git");
        assert_eq!(items.save, None, "{base:?}");
    }
}
#[test]
fn each_repository_takes_its_forges_git_program() {
    let mut forges = List::with_capacity(3);
    for (number, program) in
        [(2, None), (3, Some(b"/data/other/bin/git".as_slice())), (1, Some(b"/data/demo/bin/git".as_slice()))]
    {
        let mut forge = fixtures::forges().into_iter().next().unwrap();
        forge.number = number;
        if let Some(program) = program {
            forge.git_program = Some(program.into());
        }
        forges.push(forge).unwrap();
    }
    let forges = forges.into_boxed();
    for (number, program) in
        [(1, Some(b"/data/demo/bin/git".as_slice())), (2, None), (3, Some(b"/data/other/bin/git".as_slice()))]
    {
        let mut repository = fixtures::repository(7, b"acme", b"shop");
        repository.provider.forge = number;
        let assignment = fixtures::assignment_with(repository);
        let items =
            translate::items(&assignment, &forges, &crate::forge::tokens::Tokens::empty(), &fixtures::wire_limits())
                .unwrap()
                .unwrap();
        let repository = items.repositories().get(0).unwrap();
        assert_eq!(repository.git().as_deref(), program, "forge {number}");
        assert_eq!(repository.remote(), b"https://forge.example/acme/shop.git");
        let (workspace, _) = translate::workspace(Some(&items));
        assert_eq!(workspace.repositories[0].git.as_deref(), program, "worker's forge {number}");
    }
}
fn branch() -> wire::Branch {
    wire::Branch::new(&fixtures::wire_limits(), wire::BranchParts { branch: b"main".as_slice().into() }).unwrap()
}
#[test]
fn each_workspace_start_reaches_its_items() {
    let settings = fixtures::engine();
    let limits = fixtures::wire_limits();
    let rows = [
        (root::ForgeStart::Base(b"main".as_slice().into()), wire::Start::Base(branch())),
        (root::ForgeStart::Branch(b"main".as_slice().into()), wire::Start::Branch(branch())),
        (root::ForgeStart::Saved(b"main".as_slice().into()), wire::Start::Saved(branch())),
        (
            root::ForgeStart::Merge { branch: b"main".as_slice().into(), base: [3; 32] },
            wire::Start::Merge(
                wire::Merge::new(&limits, wire::MergeParts { branch: b"main".as_slice().into(), base: [3; 32] })
                    .unwrap(),
            ),
        ),
    ];
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    for (start, expected) in rows {
        assignment.workspace.repositories[0].start = start;
        let items = translate::items(&assignment, &settings.forges, &crate::forge::tokens::Tokens::empty(), &limits)
            .unwrap()
            .unwrap();
        assert_eq!(items.repositories().get(0).unwrap().start(), &expected, "{expected:?}");
    }
}
#[test]
fn each_workspace_access_reaches_its_items() {
    let settings = fixtures::engine();
    let limits = fixtures::wire_limits();
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    for (push, expected) in [
        (None, wire::Access::ReadOnly),
        (
            Some(Box::from(b"out".as_slice())),
            wire::Access::Writable(
                wire::Writable::new(
                    &limits,
                    wire::WritableParts { push: b"out".as_slice().into(), expected: None, landing: None },
                )
                .unwrap(),
            ),
        ),
    ] {
        assignment.workspace.repositories[0].push = push;
        let items = translate::items(&assignment, &settings.forges, &crate::forge::tokens::Tokens::empty(), &limits)
            .unwrap()
            .unwrap();
        assert_eq!(items.repositories().get(0).unwrap().access(), &expected, "{expected:?}");
    }
}
#[test]
fn a_workspace_past_the_wire_bound_or_without_a_forge_is_refused() {
    let settings = fixtures::engine();
    let assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    let limits = fixtures::wire_limits();
    assert!(
        translate::items(&assignment, &[], &crate::forge::tokens::Tokens::empty(), &limits).is_err(),
        "unconfigured forge"
    );
    for (field, expected) in [("remote", wire::Path::RepositoryRemote), ("repositories", wire::Path::ItemsRepositories)]
    {
        let mut limits = limits;
        if field == "remote" {
            limits.repository_remote = 4;
        } else {
            limits.items_repositories = 0;
        }
        assert_eq!(
            translate::items(&assignment, &settings.forges, &crate::forge::tokens::Tokens::empty(), &limits)
                .unwrap_err()
                .path,
            expected,
            "{field}"
        );
    }
}
#[test]
fn the_heads_a_run_landed_reach_the_roots_pushed_heads() {
    let limits = fixtures::wire_limits();
    let mut landed = List::with_capacity(2);
    for (tag, commit) in
        [(temper_engine_domain_forge::tag(1, 1), [3; 32]), (temper_engine_domain_forge::tag(3, 1), [4; 32])]
    {
        landed.push(wire::Landed::new(&limits, wire::LandedParts { tag, commit }).unwrap()).unwrap();
    }
    let left = wire::Left::new(&limits, wire::LeftParts { landed }).unwrap();
    assert_eq!(
        translate::parts(Some(left), None, None).pushed.as_ref(),
        &[
            temper_engine_domain_forge::Pushed { tag: temper_engine_domain_forge::tag(1, 1), commit: [3; 32] },
            temper_engine_domain_forge::Pushed { tag: temper_engine_domain_forge::tag(3, 1), commit: [4; 32] },
        ]
    );
}
fn saved(landing: wire::Landing, from_saved: bool) -> wire::Saved {
    let limits = fixtures::wire_limits();
    let mut repositories = List::with_capacity(1);
    repositories
        .push(wire::SavedRepository::new(&limits, wire::SavedRepositoryParts { tag: 7, from_saved, landing }).unwrap())
        .unwrap();
    wire::Saved::new(&limits, wire::SavedParts { repositories }).unwrap()
}
fn conflicted() -> wire::Landing {
    wire::Landing::Conflicted(
        wire::Files::new(&fixtures::wire_limits(), wire::FilesParts { files: List::with_capacity(0) }).unwrap(),
    )
}
fn explained() -> wire::Landing {
    let limits = fixtures::wire_limits();
    let diagnostic = wire::Diagnostic::new(&limits, wire::DiagnosticParts { output: Box::new([]), cut: 0 }).unwrap();
    wire::Landing::Explained(
        wire::PushFailure::new(
            &limits,
            wire::PushFailureParts { repository: Some(7), reason: wire::PushReason::Busy, diagnostic },
        )
        .unwrap(),
    )
}
#[test]
fn a_save_names_each_repository_with_saved_work_after_it() {
    let limits = fixtures::wire_limits();
    let landed = wire::Landing::Landed(wire::Commit::new(&limits, wire::CommitParts { commit: [3; 32] }).unwrap());
    for (from_saved, landing, expected) in [
        (false, landed.clone(), &[7_u64][..]),
        (true, landed, &[7][..]),
        (false, wire::Landing::Unchanged, &[][..]),
        (true, wire::Landing::Unchanged, &[7][..]),
        (false, conflicted(), &[][..]),
        (true, conflicted(), &[7][..]),
        (false, explained(), &[][..]),
        (true, explained(), &[7][..]),
        (false, wire::Landing::Moved, &[][..]),
        (true, wire::Landing::Moved, &[7][..]),
        (false, wire::Landing::Failed, &[][..]),
        (true, wire::Landing::Failed, &[7][..]),
        (false, wire::Landing::Refused, &[][..]),
        (true, wire::Landing::Refused, &[7][..]),
    ] {
        let row = landing.clone();
        let parts = translate::parts(None, Some(saved(landing, from_saved)), None);
        assert_eq!(parts.saved.as_deref(), Some(expected), "{row:?} {from_saved}");
    }
}
#[test]
fn each_preparation_reaches_the_roots_parts() {
    let limits = fixtures::wire_limits();
    for (source, expected) in [
        (wire::Missing::Repository, root::Missing::Repository),
        (wire::Missing::Branch, root::Missing::Branch),
        (wire::Missing::Commit, root::Missing::Commit),
    ] {
        let missed = wire::Missed::new(&limits, wire::MissedParts { tag: 7, missing: source.clone() }).unwrap();
        assert_eq!(
            translate::parts(None, None, Some(wire::Preparation::Missing(missed))).preparation,
            Some(root::Preparation::Missing { tag: 7, missing: expected }),
            "{source:?}"
        );
    }
    for (source, expected) in [
        (wire::Preparation::Transient, None),
        (
            wire::Preparation::Refused(wire::Tag::new(&limits, wire::TagParts { tag: 7 }).unwrap()),
            Some(root::Preparation::Refused { tag: 7 }),
        ),
    ] {
        let row = source.clone();
        assert_eq!(translate::parts(None, None, Some(source)).preparation, expected, "{row:?}");
    }
}

#[test]
fn an_empty_workspace_has_no_items() {
    let settings = fixtures::engine();
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    assignment.workspace.repositories = Box::new([]);
    assert_eq!(
        translate::items(
            &assignment,
            &settings.forges,
            &crate::forge::tokens::Tokens::empty(),
            &fixtures::wire_limits()
        ),
        Ok(None)
    );
}
#[test]
fn absent_parts_leave_the_roots_parts_empty() {
    assert_eq!(translate::parts(None, None, None), root::Parts::none());
}

#[test]
fn a_configured_identity_encodes_as_forgejos_kind() {
    let configured = fixtures::engine().identities.into_iter().next().unwrap();
    let limits = fixtures::wire_limits();
    let bytes = translate::identity(&configured, &limits).unwrap();
    let value = wire::Identity::decode(&limits, &mut skein_lib::Reader::new(bytes.expose())).unwrap();
    let expected = wire::Identity::new(
        &limits,
        wire::IdentityParts {
            kind: wire::Kind::Forgejo(wire::Token::new(&limits, wire::TokenParts { token: configured.token }).unwrap()),
            login: configured.login,
            name: configured.author.name,
            email: configured.author.email,
        },
    )
    .unwrap();
    assert_eq!(value, expected);
}

#[test]
fn repositories_without_a_published_entry_carry_no_account() {
    for kind in [crate::settings::Kind::Forgejo, crate::settings::Kind::GitHub] {
        let mut forge = fixtures::forges().into_iter().next().unwrap();
        forge.kind = kind;
        let assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
        let items =
            translate::items(&assignment, &[forge], &crate::forge::tokens::Tokens::empty(), &fixtures::wire_limits())
                .unwrap()
                .unwrap();
        assert_eq!(*items.repositories().get(0).unwrap().account(), None, "{kind:?}");
    }
}

#[test]
fn a_repositorys_account_reaches_the_workers_domain() {
    let settings = fixtures::engine();
    let assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    let limits = fixtures::wire_limits();
    let original = translate::items(&assignment, &settings.forges, &crate::forge::tokens::Tokens::empty(), &limits)
        .unwrap()
        .unwrap();
    let (mut expected, save) = translate::workspace(Some(&original));
    expected.repositories[0].account = Some(19);
    let mut repositories = List::with_capacity(1);
    let repository = original.repositories().get(0).unwrap().clone().into_parts();
    repositories
        .push(wire::Repository::new(&limits, wire::RepositoryParts { account: Some(19), ..repository }).unwrap())
        .unwrap();
    let items = wire::Items::new(&limits, wire::ItemsParts { repositories, save: original.save().clone() }).unwrap();
    let bytes = temper_protocol_worker_wire::items(&items);
    let decoded = wire::Items::decode(&limits, &mut skein_lib::Reader::new(&bytes)).unwrap();
    assert_eq!(translate::workspace(Some(&decoded)), (expected, save));
}

#[test]
fn landing_comparison_roundtrips_and_read_only_access_strips_it() {
    let settings = fixtures::engine();
    let limits = fixtures::wire_limits();
    for writable in [false, true] {
        let mut repository = fixtures::repository(7, b"acme", b"shop");
        repository.landing = Some(b"main".as_slice().into());
        if !writable {
            repository.push = None;
        }
        let assignment = fixtures::assignment_with(repository);
        let items = translate::items(&assignment, &settings.forges, &crate::forge::tokens::Tokens::empty(), &limits)
            .unwrap()
            .unwrap();
        let encoded = temper_protocol_worker_wire::items(&items);
        let decoded = wire::Items::decode(&limits, &mut skein_lib::Reader::new(&encoded)).expect("items roundtrip");
        let (workspace, _) = translate::workspace(Some(&decoded));
        match &workspace.repositories[0].access {
            temper_worker_domain::wire::Access::Writable { landing, .. } => {
                assert!(writable);
                assert_eq!(landing.as_deref(), Some(b"main".as_slice()));
            }
            temper_worker_domain::wire::Access::ReadOnly => assert!(!writable),
        }
    }
}
