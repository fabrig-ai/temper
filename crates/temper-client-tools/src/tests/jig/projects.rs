#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "ordinary Rust fixtures and golden comparisons; testing-strategy.md, step tests"
)]
use crate::{jig, tests::fixtures as f};
use jig_client::{Ask, contract as c};
use skein_lib::{List, Wall};

fn list<T>(values: Vec<T>) -> List<T> {
    let mut out = List::with_capacity(values.len().try_into().unwrap());
    for value in values {
        out.push(value).ok().unwrap();
    }
    out
}
fn project() -> c::OwnerProject {
    let authority = c::OwnerAuthorityDocument::complete(c::OwnerAuthority {
        models: None,
        tools: 0,
        grants: list(vec![]),
        delegation: c::OwnerDelegation { kinds: list(vec![]), tasks: 0, depth: 0 },
        spend: 100,
        deadline: None,
        notes: 0,
        note_resources: list(vec![]),
    });
    let period = c::PoolLedger {
        source: c::Funder::Period { project: 1, period: 3 },
        parent: None,
        numbers: c::OwnerNumbers { budget: 100, reserved: 40, spent: 5, spent_below: 15 },
        left: 40,
        closed: false,
        began: Some(Wall::from_nanos(1_793_080_800_000_000_000)),
        next: Some(Wall::from_nanos(1_793_685_600_000_000_000)),
    };
    let mut pools = vec![];
    for person in [1, 2] {
        let mut pool = period.clone();
        pool.source = c::Funder::Pool { project: 1, person, period: 3 };
        pool.parent = Some(period.source);
        pools.push(pool);
    }
    c::OwnerProject {
        acts: list(vec![]),
        number: 1,
        name: f::document_text(b"shop"),
        resources: list(vec![c::OwnerProjectResource {
            part: c::OwnerPart {
                connector: 4,
                kind: c::OwnerPartKind::Resource,
                key: c::OwnerPartKey::Path(list(vec![b"src".as_slice().into(), b"lib.rs".as_slice().into()])),
            },
            role: c::OwnerProjectResourceRole::Owned,
        }]),
        policy: c::OwnerProjectPolicy {
            templates: list(vec![]),
            ceiling: authority.clone(),
            period_spend: 100,
            escalation_role: None,
            projections: list(vec![]),
            roles: list(vec![
                c::OwnerProjectPolicyRole {
                    number: 0,
                    authority: authority.clone(),
                    period_spend: 100,
                    requests: 0,
                    decides: 0,
                },
                c::OwnerProjectPolicyRole { number: 2, authority, period_spend: 50, requests: 0, decides: 0 },
            ]),
            requirements: list(vec![]),
            permissions: None,
            permissions_cut: false,
            cut: false,
        },
        members: list(vec![
            c::OwnerProjectMember {
                party: 1,
                kind: c::PartyKind::Person,
                role: c::HeldRole::Owner,
                login: f::document_text(b"alice"),
                name: f::document_text(b"Alice"),
            },
            c::OwnerProjectMember {
                party: 2,
                kind: c::PartyKind::Person,
                role: c::HeldRole::Member,
                login: f::document_text(b"bob"),
                name: f::document_text(b"Bob"),
            },
        ]),
        current_period: Some(period),
        pools: list(pools),
        configured_period: 3,
        configured_pools: list(vec![]),
        configured_pools_cut: false,
        spend: list(vec![]),
        recurring: list(vec![]),
        cut: false,
    }
}
fn read(argv: &[&str], value: c::OwnerProject) -> jig::Printed {
    let (mut root, call) = f::read_call(argv);
    assert_eq!(root.reads[&call].1, c::Read::Project { project: 1 });
    root.serve(call, c::ReadResult::Value(c::ReadValue::OwnerProject(Box::new(value))));
    root.answer(call).expect("project read completed")
}
#[test]
fn project_show_prints_the_open_period_and_each_pool() {
    let printed = read(&["project", "show", "1"], project());
    assert_eq!(printed.status, 0);
    f::check_golden("project_show", &printed.stdout);
}
#[test]
fn project_show_says_when_no_next_start_is_known() {
    let mut project = project();
    project.current_period.as_mut().unwrap().next = None;
    let printed = read(&["project", "show", "1"], project);
    assert!(String::from_utf8_lossy(&printed.stdout).contains("next start: none"));
}
#[test]
fn project_show_says_when_no_period_is_open() {
    let mut project = project();
    project.current_period = None;
    let printed = read(&["project", "show", "1"], project);
    assert!(String::from_utf8_lossy(&printed.stdout).contains("period: none open\n"));
}
#[test]
fn project_show_json_preserves_integer_ledgers_and_cut_flags() {
    let mut project = project();
    project.cut = true;
    project.configured_pools_cut = true;
    let printed = read(&["project", "show", "1", "--json"], project.clone());
    assert_eq!(printed.status, 0);
    f::unique_keys(&printed.stdout).expect("valid JSON with unique keys");
    f::check_golden("project_show_json", &printed.stdout);
    let printed = read(&["project", "show", "1"], project);
    assert!(String::from_utf8_lossy(&printed.stdout).contains("project: cut\nconfigured pools: cut\ndigest: "));
}
#[test]
fn project_fund_sends_project_spend_and_role_spend() {
    for role in [None, Some("0")] {
        let mut root = f::signed_in(f::PARTY);
        let mut argv = vec!["project", "fund", "1", "200", "--key", "fund"];
        if let Some(role) = role {
            argv.extend(["--role", role]);
        }
        let call = root.submit(&argv);
        let edit = if role.is_some() {
            c::PolicyEdit::RoleSpend { role: 0, period_spend: 200 }
        } else {
            c::PolicyEdit::ProjectSpend { period_spend: 200 }
        };
        assert_eq!(root.sends[&call].2, Ask::Jig(Box::new(c::Ask::ChangePolicy { project: 1, edit })));
        assert_eq!(root.sent_name(call), Some(f::named("fund")));
        assert!(root.reads.is_empty(), "no read before the act's answer");
    }
}
#[test]
fn project_fund_prints_the_period_then_the_roles_pools() {
    for role in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let mut argv = vec!["project", "fund", "1", "200", "--key", "fund"];
        if role {
            argv.extend(["--role", "0"]);
        }
        let call = root.submit(&argv);
        root.done(call, c::Outcome::Owner(c::OwnerOutcome::PolicyChanged { project: 1 }));
        assert_eq!(root.reads[&call].1, c::Read::Project { project: 1 });
        root.serve(call, c::ReadResult::Value(c::ReadValue::OwnerProject(Box::new(project()))));
        let printed = root.answer(call).unwrap();
        assert_eq!(printed.status, 0);
        let expected = if role {
            "period 3: budget 100, left 40\npool, party 1 alice: budget 100, left 40\n"
        } else {
            "period 3: budget 100, left 40\n"
        };
        assert_eq!(printed.stdout.as_ref(), expected.as_bytes());
    }
}
#[test]
fn project_fund_past_the_periods_room_exits_2_naming_what_is_left() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["project", "fund", "1", "200", "--role", "0", "--key", "fund"]);
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Authority { lacking: c::Lacking::Budget { needed: 200, left: 60 }, proposable: None },
            at: None,
        },
    );
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 2);
    assert_eq!(
        printed.stderr.as_ref(),
        b"refused: the period has 60 left, the pools need 200; raise the project's first: project fund 1 AMOUNT\n"
    );
    assert!(root.reads.is_empty());
}
#[test]
fn project_fund_amount_and_role_bounds_are_usage() {
    for amount in ["0", "18446744073709551615"] {
        let call = f::call(&["project", "fund", "1", amount, "--key", "fund"]);
        assert_eq!(f::back(&call), f::owned(&["project", "fund", "1", amount, "--key", "fund"]));
    }
    for amount in ["18446744073709551616", "-1", "no"] {
        f::parse(&["project", "fund", "1", amount, "--key", "fund"]).expect_err("invalid amount is usage");
    }
    for role in ["0", "4294967295"] {
        f::parse(&["project", "fund", "1", "200", "--role", role, "--key", "fund"]).expect("role bound parses");
    }
    f::parse(&["project", "fund", "1", "200", "--role", "4294967296", "--key", "fund"])
        .expect_err("role past bound is usage");
    f::parse(&["project", "fund", "1", "200"]).expect_err("funding requires a key");
}

#[test]
fn project_show_resolves_names_and_preserves_read_refusals() {
    let (mut root, call) = f::read_call(&["project", "show", "shop"]);
    assert_eq!(root.reads[&call].1, c::Read::Project { project: f::PROJECT });
    root.serve(call, c::ReadResult::None(c::ReadNone::Refused));
    assert_eq!(root.answer(call).unwrap().status, 2);
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["project", "show", "unknown", "--json"]);
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 1);
    assert_eq!(printed.stderr.as_ref(), br#"{"error":"unknown project"}"#);
}
#[test]
fn project_fund_json_reads_the_committed_ledgers() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["project", "fund", "shop", "200", "--role", "0", "--key", "fund", "--json"]);
    let Ask::Jig(ask) = &root.sends[&call].2 else { unreachable!() };
    assert_eq!(
        ask.as_ref(),
        &c::Ask::ChangePolicy { project: f::PROJECT, edit: c::PolicyEdit::RoleSpend { role: 0, period_spend: 200 } }
    );
    root.done(call, c::Outcome::Owner(c::OwnerOutcome::PolicyChanged { project: 1 }));
    assert_eq!(root.reads[&call].1, c::Read::Project { project: 1 });
    root.serve(call, c::ReadResult::Value(c::ReadValue::OwnerProject(Box::new(project()))));
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 0);
    f::unique_keys(&printed.stdout).expect("valid JSON with unique keys");
    f::check_golden("project_fund_json", &printed.stdout);
}
