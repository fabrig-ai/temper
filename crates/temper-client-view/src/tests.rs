mod chats;
mod choice;
mod confirm;
mod delegation;
mod demo;
mod demo_start;
pub(crate) mod fixtures;
mod forge_page;
mod forges;
mod frame;
mod inbox;
mod look;
use jig_client::contract::{Part, PartScope};
use skein_ui::Role;

#[test]
fn each_resource_path_shape_says_its_words() {
    let words = fixtures::words();
    let rows: [(&[&[u8]], &[u8]); 9] = [
        (&[b"forge", b"forge.example", b"acme", b"shop"], b"acme/shop on forge.example"),
        (
            &[b"forge", b"forge.example", b"acme", b"shop", b"branch", b"temper", b"7"],
            b"branch temper/7 of acme/shop on forge.example",
        ),
        (
            &[b"forge", b"forge.example", b"acme", b"shop", b"pull", b"12"],
            b"pull request #12 of acme/shop on forge.example",
        ),
        (&[b"forge", b"forge.example", b"acme", b"shop", b"issue", b"3"], b"issue #3 of acme/shop on forge.example"),
        (&[&[0, 1], &[0, 0, 0, 42]], b"repository 42 on forge 1"),
        (&[b"elsewhere", b"x"], b"elsewhere/x"),
        (&[b"forge", b"host", b"owner", b"name", b"branch"], b"forge/host/owner/name/branch"),
        (&[b"forge", b"host", b"owner", b"name", b"pull", b"x", b"y"], b"forge/host/owner/name/pull/x/y"),
        (&[], b""),
    ];
    for (path, said) in rows {
        assert_eq!(&*fixtures::owner_part_text(&words, path), said, "path {path:?}");
        let part = Part::Resource { connector: fixtures::FORGE, path: fixtures::path(path) };
        assert_eq!(&*fixtures::part_text(&words, part), said, "held path {path:?}");
    }
}
#[test]
fn each_numbered_kind_says_its_words_and_others_kind_n() {
    let words = fixtures::words();
    let rows: [(u16, &[u8]); 17] = [
        (1, b"read"),
        (2, b"push"),
        (3, b"open a pull request"),
        (4, b"merge a pull request"),
        (5, b"post a review"),
        (6, b"set a commit status"),
        (7, b"post a comment"),
        (8, b"create an issue"),
        (9, b"create a branch"),
        (10, b"edit a pull request"),
        (11, b"update a pull request from its base"),
        (12, b"close or reopen a pull request"),
        (13, b"edit, close or reopen an issue"),
        (14, b"set reviewers"),
        (15, b"delete a branch"),
        (0, b"kind 0"),
        (16, b"kind 16"),
    ];
    for (kind, said) in rows {
        assert_eq!(&*fixtures::part_text(&words, Part::Kind { connector: fixtures::FORGE, kind }), said, "kind {kind}");
    }
}
#[test]
fn a_part_needing_a_document_says_shown_on_the_forge() {
    use jig_client::contract::ProposalKey;
    let words = fixtures::words();
    let connector = fixtures::FORGE;
    let parts = [
        Part::Panel { connector, task: 1 },
        Part::Result { connector, task: 1 },
        Part::Effect {
            connector,
            proposal: ProposalKey { proposer: jig_client::contract::Proposer::Task(1), number: 1 },
        },
        Part::OwnerEffect { connector, proposal: 1 },
        Part::Outbox { connector, scope: PartScope::Project(fixtures::PROJECT), entry: 1 },
        Part::Hold { connector, task: 1 },
        Part::Project { connector, project: fixtures::PROJECT },
        Part::Deployment { connector },
    ];
    for part in parts {
        let tree = fixtures::part_tree(&words, part.clone());
        assert_eq!(&*fixtures::text(&tree), b"Shown on the forge.", "part {part:?}");
        assert_eq!(tree.nodes().first().expect("paragraph present").role, Role::Paragraph, "part {part:?}");
    }
}

mod chat;

mod task;

mod cards;

const PATH_SEGMENT_BYTES: usize = 64;
#[test]
fn a_long_forge_path_shows_its_last_whole_segment() {
    let words = fixtures::words();
    let path = fixtures::path(&[&[b'a'; PATH_SEGMENT_BYTES], &[b'b'; PATH_SEGMENT_BYTES], b"last"]);
    assert_eq!(
        fixtures::part_text(&words, Part::Resource { connector: fixtures::FORGE, path }).as_ref(),
        b"\xe2\x80\xa6/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/last"
    );
}

mod accounts;
mod connect_model;

mod times;

mod change;

mod welcome;

mod flow;

mod connect_forge;

#[test]
fn validity_accepts_stock_limits_and_refuses_invalid_edges() {
    let mut domain = fixtures::limits();
    let words = fixtures::words();
    let limits = crate::Limits::of(&domain, &words).unwrap();
    assert!(crate::valid(&limits, &domain, &words));
    domain.client.domain.rows = u32::MAX; // Checked account/node row counts overflow.
    assert!(!crate::valid(&limits, &domain, &words));
    assert!(crate::Limits::of(&domain, &words).is_none());
}
