//! Tables for store evidence and exact check lines (protocol/setup.md, 8.4).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary Rust table fixtures and projections (programming-model.md, 10.2)"
)]
use crate::check::{self, Reason};
use skein_fake_forge_domain::promises::{Allowance, Breach, Decision, Profile, Promise, Verdict, Why};
use skein_fake_forge_protocol::routes;
use skein_forge::api::{Repository, Write};
use skein_forge_client::{Condition, Effect, Entry, Writer};
use skein_lib::{Duration, Wall};
use temper_engine_domain_forge as d;
use temper_engine_domain_forge_change as change;
use temper_protocol_store::{keys, records};

type Row = (Box<[u8]>, Box<[u8]>);

const REPO: Repository = Repository { forge: 1, repository: 2 };
const CLIENT: skein_forge_client::Limits = skein_forge_client::Limits {
    pending: 4,
    calls: 2,
    rate: 100,
    reserve: 0,
    window: Duration::from_secs(10),
    op_bytes: 512,
    answer_bytes: 4096,
    rows: 8,
    inbox: 16,
    read_attempts: 3,
    backoff: Duration::from_secs(1),
    backoff_max: Duration::from_secs(4),
    entries: 8,
    write_attempts: 3,
    lifetime: Duration::from_secs(10),
    clock_margin: Duration::from_secs(1),
    resources: 8,
    repositories: 2,
    poll: Duration::from_secs(5),
    poll_max: Duration::from_secs(30),
    hinted: Duration::from_secs(1),
    slow: Duration::from_secs(40),
    settle: Duration::from_secs(30),
    facts: 16,
};
pub const LIMITS: d::Limits = d::Limits {
    repositories: 2,
    tasks: 8,
    holds: 8,
    subscriptions: 8,
    entries: 8,
    named_calls: 16,
    changes: 8,
    issues: 8,
    resources_per_task: 4,
    paths_per_subscription: 4,
    authority_segments: 8,
    name_bytes: 128,
    output: 80,
    facts: 16,
    adoptions: 2,
    applications: 4,
    bindings: 4,
    collaborators: 8,
    landings: 8,
    landing_members: 2,
    gates: 2,
    receipts_per_gate: 4,
    ignored_paths: 4,
    key_entries: 16,
    key_path_bytes: 1024,
    trees_in_flight: 2,
    judge_projects: 2,
    judge_criteria: 8,
    tiers: 2,
    // Test gates per repository (domain/landing.md, 13.1).
    gates: 2,
    // Ignored paths per gate or tier (domain/landing.md, 6.1).
    ignored_paths: 4,
    failing_tests: 16,
    test_id_bytes: 256,
    brief_sections: 32,
    brief_bytes: 4096,
    client: CLIENT,
    issue_policy: temper_engine_domain_forge_issues::Limits {
        plan_items: 8,
        milestones: 8,
        title_bytes: 128,
        body_bytes: 512,
        comment_bytes: 256,
        interval: Duration::from_secs(5),
    },
    change_policy: temper_engine_domain_forge_change::Limits {
        repairs: 3,
        resolutions: 3,
        updates: 3,
        stall: Duration::from_secs(30),
        gates: 8,
        clean_heads: 8,
    },
    queue_window: Duration::from_secs(30),
};

fn repository(provider: Repository, prefix: &[u8], review: bool) -> d::Stored {
    d::Stored::Repository(d::Repository {
        project: 1,
        home: true,
        provider,
        host: b"forge".as_slice().into(),
        owner: b"fake".as_slice().into(),
        name: b"widget".as_slice().into(),
        prefix: prefix.into(),
        role: d::Role::Owned,
        ci: true,
        checks: Box::new([]),
        kinds: d::Kinds {
            read: true,
            push: true,
            open: true,
            land: true,
            review,
            status: true,
            comment: true,
            issue: true,
            branch: true,
        },
        protection: d::Protection::Absent,
        settings: skein_forge::api::Settings {
            default_branch: b"main".as_slice().into(),
            merge: true,
            squash: true,
            rebase: true,
        },
    })
}
fn entry(repository: Repository, number: u64, head: [u8; 32], parked: bool) -> d::Stored {
    d::Stored::Entry {
        entry: Entry {
            number,
            task: 4,
            repository,
            effect: Effect {
                write: Write::Merge { number, head, how: skein_forge::api::MergeHow::Style },
                condition: Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
        held: parked,
    }
}
fn changed(task: u64, repository: Repository, pull: Option<u64>, head: Option<[u8; 32]>) -> d::Stored {
    d::Stored::Change(d::ChangeRow {
        task,
        repository,
        branch: b"temper/change".as_slice().into(),
        base: b"main".as_slice().into(),
        title: b"change".as_slice().into(),
        body: Box::new([]),
        priority: 0,
        ci: None,
        change: change::Change {
            way: change::Way::Queue,
            landing: None,
            task,
            state: change::State::Landed { merge: routes::object(99).bytes },
            gates: Box::new([]),
            clean: Box::new([]),
            gate_failures: Box::new([]),
            released: None,
            repairs: 0,
            resolutions: 0,
            updates: 0,
            since: Wall::from_nanos(0),
            ready_since: None,
            owns_turn: false,
            last_head: head,
        },
        pull,
        pending: None,
        effect: change::EffectResult::Made,
        delegate: None,
        delegate_status: change::Status::Passed,
        verdicts: Box::new([]),
        gate_remarks: Box::new([]),
        drift: None,
        base_repair: false,
        queue_repair: None,
        queue_repairs: 0,
        producing_charter: 2,
        review_charter: 4,
    })
}
fn landed(commit: u64) -> d::Stored {
    d::Stored::Landed { commit: routes::object(commit).bytes, task: 4 }
}
fn deployment() -> Vec<d::Stored> {
    vec![d::Stored::Writer(Writer { forge: 1, author: 7 }), repository(REPO, b"temper/", false)]
}
fn rows(add: Vec<d::Stored>, remove: Option<d::Key>) -> Vec<d::Stored> {
    let mut rows = deployment();
    rows.retain(|row| Some(d::stored_key(row)) != remove);
    rows.extend(add);
    rows
}
#[derive(Debug, PartialEq, Eq)]
struct ProfileView {
    client: u64,
    marker: (Box<[u8]>, Box<[u8]>),
    prefixes: Vec<Box<[u8]>>,
    allowed: Vec<&'static str>,
    objects: u32,
    decisions: u32,
}
fn profile(profile: Profile) -> ProfileView {
    ProfileView {
        client: profile.client,
        marker: profile.marker,
        prefixes: profile.owned_branches.into_vec(),
        allowed: profile
            .allowed
            .iter()
            .map(|allowance| match allowance {
                Allowance::Comment => "comment",
                Allowance::Merge => "merge",
                Allowance::Close => "close",
                Allowance::Review => "review",
                Allowance::Request => "request",
                Allowance::Labels(_) => "labels",
            })
            .collect(),
        objects: profile.bounds.objects,
        decisions: profile.bounds.decisions,
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    profile: ProfileView,
    decisions: Vec<Decision>,
    complete: bool,
}
#[expect(clippy::unnecessary_wraps, reason = "success fixtures share the table Result shape")]
fn expected(decisions: &[(u64, u64)], complete: bool, prefixes: &[&[u8]], review: bool) -> Result<Expected, Reason> {
    let mut allowed = vec!["comment", "merge", "close"];
    if review {
        allowed.extend(["review", "request"]);
    }
    Ok(Expected {
        profile: ProfileView {
            client: 7,
            marker: (b"<!-- temper:key ".as_slice().into(), b" -->\n".as_slice().into()),
            prefixes: prefixes.iter().map(|p| Box::from(*p)).collect(),
            allowed,
            objects: 256,
            decisions: u32::try_from(decisions.len()).unwrap(),
        },
        decisions: decisions
            .iter()
            .map(|(number, head)| Decision {
                repository: b"fake/widget".as_slice().into(),
                number: *number,
                head: *head,
            })
            .collect(),
        complete,
    })
}
#[test]
#[expect(clippy::too_many_lines, reason = "one evidence table, with independent rows and exact expectations")]
fn evidence() {
    let head = routes::object(23).bytes;
    let foreign = Repository { forge: 2, repository: 2 };
    let mut full = LIMITS;
    full.landings = 2;
    let mut invalid = LIMITS;
    invalid.name_bytes = u32::MAX;
    let fresh = expected(&[], true, &[b"temper/"], false);
    let merge = expected(&[(3, 23)], true, &[b"temper/"], false);
    // Each table row changes a fresh deployment; raw rows exercise the key/decoder boundary.
    let cases = vec![
        ("fresh", deployment(), None, LIMITS, fresh),
        ("entry", rows(vec![entry(REPO, 3, head, false)], None), None, LIMITS, merge),
        (
            "held entry",
            rows(vec![entry(REPO, 3, head, true)], None),
            None,
            LIMITS,
            expected(&[(3, 23)], true, &[b"temper/"], false),
        ),
        (
            "landed",
            rows(vec![landed(99), changed(4, REPO, Some(3), Some(head))], None),
            None,
            LIMITS,
            expected(&[(3, 23)], true, &[b"temper/"], false),
        ),
        (
            "entry and landed",
            rows(vec![entry(REPO, 3, head, false), landed(99), changed(4, REPO, Some(3), Some(head))], None),
            None,
            LIMITS,
            expected(&[(3, 23)], true, &[b"temper/"], false),
        ),
        (
            "foreign rows",
            rows(
                vec![
                    d::Stored::Writer(Writer { forge: 2, author: 8 }),
                    repository(foreign, b"outside/", true),
                    entry(foreign, 3, [255; 32], false),
                    landed(99),
                    changed(4, foreign, None, None),
                ],
                None,
            ),
            None,
            LIMITS,
            expected(&[], true, &[b"temper/"], false),
        ),
        (
            "review",
            rows(vec![repository(REPO, b"temper/", true)], Some(d::Key::Repository(REPO))),
            None,
            LIMITS,
            expected(&[], true, &[b"temper/"], true),
        ),
        ("no writer", rows(vec![], Some(d::Key::Writer(1))), None, LIMITS, Err(Reason::ClientUnknown)),
        (
            "conflicting writers",
            rows(vec![d::Stored::Writer(Writer { forge: 1, author: 8 })], None),
            None,
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "same writers",
            rows(vec![d::Stored::Writer(Writer { forge: 1, author: 7 })], None),
            None,
            LIMITS,
            expected(&[], true, &[b"temper/"], false),
        ),
        ("landed without change", rows(vec![landed(99)], None), None, LIMITS, Err(Reason::StoreUnreadable)),
        (
            "change without pull",
            rows(vec![landed(99), changed(4, REPO, None, Some(head))], None),
            None,
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "change without last head",
            rows(vec![landed(99), changed(4, REPO, Some(3), None)], None),
            None,
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "entry head not fake",
            rows(vec![entry(REPO, 3, [255; 32], false)], None),
            None,
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "landed head not fake",
            rows(vec![landed(99), changed(4, REPO, Some(3), Some([255; 32]))], None),
            None,
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "undecodable",
            deployment(),
            Some((Box::from([255_u8]), Box::from([255_u8]))),
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        (
            "key mismatch",
            deployment(),
            Some((Box::from([0_u8]), records::encode(&d::Stored::Writer(Writer { forge: 1, author: 7 })))),
            LIMITS,
            Err(Reason::StoreUnreadable),
        ),
        ("no bounds", deployment(), None, invalid, Err(Reason::StoreUnreadable)),
        (
            "landings below limit",
            rows(vec![landed(99), changed(4, REPO, Some(3), Some(head))], None),
            None,
            full,
            expected(&[(3, 23)], true, &[b"temper/"], false),
        ),
        (
            "landings at limit across forges",
            rows(
                vec![
                    landed(99),
                    d::Stored::Landed { commit: routes::object(100).bytes, task: 5 },
                    changed(4, REPO, Some(3), Some(head)),
                    changed(5, foreign, None, None),
                ],
                None,
            ),
            None,
            full,
            expected(&[(3, 23)], false, &[b"temper/"], false),
        ),
        (
            "entry repository gone",
            rows(vec![entry(REPO, 3, head, false)], Some(d::Key::Repository(REPO))),
            None,
            LIMITS,
            expected(&[], false, &[], false),
        ),
        (
            "landed repository gone",
            rows(vec![landed(99), changed(4, REPO, Some(3), Some(head))], Some(d::Key::Repository(REPO))),
            None,
            LIMITS,
            expected(&[], false, &[], false),
        ),
        (
            "change alone adds no decision",
            rows(vec![changed(4, REPO, Some(3), Some(head))], None),
            None,
            LIMITS,
            expected(&[], true, &[b"temper/"], false),
        ),
        (
            "decisions sorted",
            rows(vec![entry(REPO, 9, head, false), entry(REPO, 2, routes::object(24).bytes, false)], None),
            None,
            LIMITS,
            expected(&[(2, 24), (9, 23)], true, &[b"temper/"], false),
        ),
        (
            "prefixes sorted and deduped",
            rows(
                vec![
                    repository(Repository { forge: 1, repository: 3 }, b"aaa/", false),
                    repository(Repository { forge: 1, repository: 4 }, b"temper/", false),
                ],
                None,
            ),
            None,
            LIMITS,
            expected(&[], true, &[b"aaa/", b"temper/"], false),
        ),
    ];
    for (label, rows, raw, limits, expected) in cases {
        let mut encoded: Vec<Row> = rows
            .iter()
            .map(|row| (keys::encode(&d::stored_key(row), u32::MAX).unwrap(), records::encode(row)))
            .collect();
        if let Some(raw) = raw {
            encoded.push(raw);
        }
        let actual = check::evidence(1, &encoded, &limits).map(|evidence| Expected {
            profile: profile(evidence.profile),
            decisions: evidence.decisions.into_vec(),
            complete: evidence.complete,
        });
        assert_eq!(actual, expected, "{label}");
    }
}

fn breach(promise: Promise) -> Breach {
    // The judge supplies an already percent-encoded subject; line() retains its bytes.
    Breach { promise, subject: b"fake%2Fwidget/pull/3%20%FF".as_slice().into() }
}
#[test]
fn lines() {
    let once = b"{\"fail\":\"promise/once/fake%2Fwidget/pull/3%20%FF\",\"summary\":\"a marked key created twice\"}\n"
        .as_slice();
    let forced =
        b"{\"fail\":\"promise/forced/fake%2Fwidget/pull/3%20%FF\",\"summary\":\"a move that does not descend\"}\n"
            .as_slice();
    let foreign = b"{\"fail\":\"promise/foreign/fake%2Fwidget/pull/3%20%FF\",\"summary\":\"a write to what the client did not make\"}\n".as_slice();
    let order = b"{\"fail\":\"promise/order/fake%2Fwidget/pull/3%20%FF\",\"summary\":\"a merge with no decision at its head\"}\n".as_slice();
    let incomplete = b"{\"undecided\":\"decisions incomplete\"}\n".as_slice();
    let lost = b"{\"undecided\":\"observations lost\"}\n".as_slice();
    for (label, verdict, complete, status, bytes) in [
        ("pass", Verdict::Pass, true, 0, vec![]),
        ("pass incomplete", Verdict::Pass, false, 0, vec![]),
        ("one fail", Verdict::Fail(Box::new([breach(Promise::Once)])), true, 1, once.to_vec()),
        ("forced", Verdict::Fail(Box::new([breach(Promise::Forced)])), true, 1, forced.to_vec()),
        ("foreign", Verdict::Fail(Box::new([breach(Promise::Foreign)])), true, 1, foreign.to_vec()),
        ("order complete", Verdict::Fail(Box::new([breach(Promise::Order)])), true, 1, order.to_vec()),
        ("order incomplete", Verdict::Fail(Box::new([breach(Promise::Order)])), false, 2, incomplete.to_vec()),
        (
            "once and order incomplete",
            Verdict::Fail(Box::new([breach(Promise::Once), breach(Promise::Order)])),
            false,
            1,
            [once, incomplete].concat(),
        ),
        (
            "undecided lost with breach",
            Verdict::Undecided(Why::Lost(5), Box::new([breach(Promise::Once)])),
            true,
            1,
            [once, lost].concat(),
        ),
        (
            "undecided full",
            Verdict::Undecided(Why::Full, Box::new([])),
            true,
            2,
            b"{\"undecided\":\"judge full\"}\n".to_vec(),
        ),
        (
            "judge reason wins",
            Verdict::Undecided(Why::Lost(5), Box::new([breach(Promise::Order)])),
            false,
            2,
            lost.to_vec(),
        ),
        (
            "judge order",
            Verdict::Fail(Box::new([breach(Promise::Foreign), breach(Promise::Once)])),
            false,
            1,
            [foreign, once].concat(),
        ),
    ] {
        let actual = check::verdict(&verdict, complete);
        assert_eq!((actual.status, actual.stdout.as_ref()), (status, bytes.as_slice()), "{label}");
    }
    for (reason, text) in [
        (Reason::ObservationsLost, "observations lost"),
        (Reason::ObservationsMissing, "observations missing"),
        (Reason::JudgeFull, "judge full"),
        (Reason::ObservationsUnforwarded, "observations unforwarded"),
        (Reason::StoreUnreadable, "store unreadable"),
        (Reason::ClientUnknown, "client unknown"),
        (Reason::DecisionsIncomplete, "decisions incomplete"),
    ] {
        assert_eq!(reason.text(), text);
        let expected = [b"{\"undecided\":\"", text.as_bytes(), b"\"}\n"].concat();
        let actual = check::undecided(reason);
        assert_eq!((actual.status, actual.stdout.as_ref()), (2, expected.as_slice()), "{reason:?}");
    }
    for (why, expected) in reasons() {
        let output = check::verdict(&Verdict::Undecided(why, Box::new([])), true);
        assert_eq!(output.stdout, check::undecided(expected).stdout);
    }
    for (why, expected) in reasons() {
        assert_eq!(Reason::from(why), expected);
    }
}

fn reasons() -> [(Why, Reason); 5] {
    [
        (Why::Lost(5), Reason::ObservationsLost),
        (Why::Truncated, Reason::ObservationsMissing),
        (Why::Full, Reason::JudgeFull),
        (Why::NoDecisions, Reason::DecisionsIncomplete),
        (Why::Unforwarded, Reason::ObservationsUnforwarded),
    ]
}

fn stderr_evidence(
    prefix: &[u8],
    allowed: Box<[Allowance]>,
    decisions: &[(u64, u64)],
    complete: bool,
) -> check::Evidence {
    check::Evidence {
        profile: Profile {
            client: 7,
            marker: (Box::from(b"<!-- temper:key ".as_slice()), Box::from(b" -->\n".as_slice())),
            owned_branches: Box::new([prefix.into()]),
            allowed,
            bounds: skein_fake_forge_domain::promises::Bounds { objects: check::OBJECTS, decisions: 1 },
        },
        decisions: decisions
            .iter()
            .map(|(number, head)| Decision {
                repository: b"playground/widget".as_slice().into(),
                number: *number,
                head: *head,
            })
            .collect(),
        complete,
    }
}
#[test]
fn stderr() {
    enum Input {
        Boundary(u64, u64),
        Evidence(check::Evidence),
    }
    let allowed = || Box::from([Allowance::Comment, Allowance::Merge, Allowance::Close]);
    for (name, input, expected) in [
        ("boundary", Input::Boundary(3, 0), "{\"boundary\":{\"lines\":3,\"lost\":0}}\n"),
        (
            "profile",
            Input::Evidence(stderr_evidence(b"temper/", allowed(), &[], true)),
            "{\"profile\":{\"client\":7,\"marker\":[\"<!-- temper:key \",\" -->\\n\"],\"owned_branches\":[\"temper/\"],\"allowed\":[\"comment\",\"merge\",\"close\"],\"bounds\":{\"objects\":256,\"decisions\":1}},\"decisions_complete\":true}\n",
        ),
        (
            "two decisions incomplete",
            Input::Evidence(stderr_evidence(b"temper/", allowed(), &[(9, 2), (1, 3)], false)),
            concat!(
                "{\"profile\":{\"client\":7,\"marker\":[\"<!-- temper:key \",\" -->\\n\"],\"owned_branches\":[\"temper/\"],\"allowed\":[\"comment\",\"merge\",\"close\"],\"bounds\":{\"objects\":256,\"decisions\":1}},\"decisions_complete\":false}\n",
                "{\"decision\":{\"repository\":\"playground/widget\",\"number\":9,\"head\":2}}\n",
                "{\"decision\":{\"repository\":\"playground/widget\",\"number\":1,\"head\":3}}\n"
            ),
        ),
        (
            "every allowance",
            Input::Evidence(stderr_evidence(
                b"temper/",
                Box::new([
                    Allowance::Comment,
                    Allowance::Review,
                    Allowance::Request,
                    Allowance::Merge,
                    Allowance::Close,
                    Allowance::Labels(b"a\"label".as_slice().into()),
                ]),
                &[],
                true,
            )),
            "{\"profile\":{\"client\":7,\"marker\":[\"<!-- temper:key \",\" -->\\n\"],\"owned_branches\":[\"temper/\"],\"allowed\":[\"comment\",\"review\",\"request\",\"merge\",\"close\",{\"labels\":\"a\\\"label\"}],\"bounds\":{\"objects\":256,\"decisions\":1}},\"decisions_complete\":true}\n",
        ),
        (
            "non UTF-8 prefix",
            Input::Evidence(stderr_evidence(b"temper/\xff\xfe", allowed(), &[], true)),
            "{\"profile\":{\"client\":7,\"marker\":[\"<!-- temper:key \",\" -->\\n\"],\"owned_branches\":[\"temper/��\"],\"allowed\":[\"comment\",\"merge\",\"close\"],\"bounds\":{\"objects\":256,\"decisions\":1}},\"decisions_complete\":true}\n",
        ),
    ] {
        let found = match input {
            Input::Boundary(lines, lost) => check::boundary_line(lines, lost),
            Input::Evidence(evidence) => check::evidence_lines(&evidence),
        };
        assert_eq!(found.as_ref(), expected.as_bytes(), "{name}");
    }
}
