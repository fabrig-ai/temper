//! Local synthetic GitHub connection frames; no forge is contacted.
use super::{World, sample_forge_answer};
use skein_lib::{List, Queue, Token};
use temper_client_domain::{Domain, Request};
use temper_client_protocol_wire::{self as wire, v1 as w};

pub(super) fn forges(world: World) -> Box<[u8]> {
    let World::FirstRun { step } = world else { unreachable!("GitHub sample") };
    let mut parts = w::ForgeRowParts {
        number: 2,
        kind: w::Kind::Github,
        mode: w::Mode::Connected,
        connection: w::Connection::Connected(
            w::Connected::new(
                &w::CEILINGS,
                w::ConnectedParts { id: 248_113_907, login: b"temper-7f3k2q[bot]".as_slice().into() },
            )
            .expect("sample connected"),
        ),
        offer: w::Offer::NotOffered,
        owned: None,
        admin: false,
        read_only: false,
        ended_stop: None,
        installations: List::with_capacity(1),
        ended: None,
        first: None,
        app: Some(w::AppWay::Manifest),
        web: Some(b"http://github.test".as_slice().into()),
        repositories: List::with_capacity(1),
    };
    if step >= 21 {
        parts
            .installations
            .push(
                w::Installation::new(
                    &w::CEILINGS,
                    w::InstallationParts {
                        id: 61_022_417,
                        account: Some(
                            w::Account::new(&w::CEILINGS, w::AccountParts { id: 10, login: b"sam".as_slice().into() })
                                .expect("sample account"),
                        ),
                        suspended: false,
                        stray: false,
                    },
                )
                .expect("sample installation"),
            )
            .expect("installation room");
    }
    if step == 22 {
        parts
            .repositories
            .push(
                w::Adoption::new(
                    &w::CEILINGS,
                    w::AdoptionParts { owner: b"sam".as_slice().into(), name: b"widget".as_slice().into(), project: 1 },
                )
                .expect("sample adoption"),
            )
            .expect("adoption room");
    }
    let mut forges = List::with_capacity(1);
    forges.push(w::ForgeRow::new(&w::CEILINGS, parts).expect("sample row")).expect("row room");
    wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges }).expect("sample forges"))
}
pub(super) fn answer_page(root: &mut Domain, attempt: Token, out: &mut Queue<Request>) {
    let mut rows = List::with_capacity(4);
    for (id, name, private) in [
        (1, b"widget".as_slice(), false),
        (2, b"notes".as_slice(), true),
        (3, b"dotfiles".as_slice(), false),
        (4, b"site".as_slice(), false),
    ] {
        rows.push(
            w::Reached::new(
                &w::CEILINGS,
                w::ReachedParts {
                    id,
                    owner: w::Account::new(&w::CEILINGS, w::AccountParts { id: 10, login: b"sam".as_slice().into() })
                        .expect("sample owner"),
                    name: name.into(),
                    private,
                },
            )
            .expect("sample repository"),
        )
        .expect("repository room");
    }
    sample_forge_answer(
        root,
        attempt,
        w::Outcome::Page(w::Page::new(&w::CEILINGS, w::PageParts { rows, more: false }).expect("sample page")),
        out,
    );
}
