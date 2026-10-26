//! Pure admission and standing requirements, before settings are wired to the root.
use alloc::boxed::Box;
use jig_domain_authority::{Last, Name, Pattern};
use temper_engine_domain_forge_change::{self as change, Status};

use crate::settings::{
    self, Bounds, Held, Item, Kind, LandingRule, Refusal, RuleApproval, RuleGate, Standing, StandingVerdict, Value,
};
use crate::{ChangeEvidence, Criterion, JudgeFreshness, JudgeVerdict, Reviewer};

const BOUNDS: Bounds = Bounds { landing_rules: 4, gates: 2, roles: 4, charters: 8, segments: 6, segment_bytes: 16 };
const EXACT: JudgeFreshness = JudgeFreshness::Exact;
const CLEAN: JudgeFreshness = JudgeFreshness::Clean;
const HEAD: skein_forge::api::Commit = super::fixtures::HEAD;
const OLD: skein_forge::api::Commit = [2; 32];

fn rule() -> LandingRule {
    LandingRule {
        pattern: Pattern { segments: Box::new([Box::from(b"forge".as_slice())]), last: Last::Open(Box::new([])) },
        enforce: true,
        ci: false,
        up_to_date: false,
        gates: Box::new([]),
        approvals: Box::new([]),
    }
}

fn rule_gate(number: u32) -> RuleGate {
    RuleGate { number, blocking: true, freshness: EXACT }
}

fn rule_approval(role: u32, people: u32) -> RuleApproval {
    RuleApproval { role, people, freshness: EXACT }
}

fn item(kind: u32, scope: u32, number: u32, value: Option<Value>) -> Item {
    Item { kind, scope, number, value }
}

fn deployment(rule: LandingRule) -> Item {
    item(1, 0, 1, Some(Value::Rule(rule)))
}

fn pattern(segments: Box<[Box<[u8]>]>, last: Last) -> LandingRule {
    LandingRule { pattern: Pattern { segments, last }, ..rule() }
}

type ItemRow = (&'static str, Item, Result<(), Refusal>);

#[test]
fn the_forge_judges_each_item() {
    for table in [item_identity_rows(), value_refusal_rows(), accepted_item_rows()] {
        for (name, item, expected) in table {
            assert_eq!(settings::judge(&item, &BOUNDS), expected, "{name}");
        }
    }
}

fn item_identity_rows() -> Box<[ItemRow]> {
    let plain = Some(Value::Rule(rule()));
    let charter = Some(Value::Charter(1));
    Box::new([
        ("kind zero", item(0, 0, 1, plain.clone()), Err(Refusal::Kind)),
        ("unknown kind", item(4, 0, 1, None), Err(Refusal::Kind)),
        ("deployment at project", item(1, 5, 1, plain.clone()), Err(Refusal::Scope)),
        ("project at deployment", item(2, 0, 1, plain.clone()), Err(Refusal::Scope)),
        ("charter at project", item(3, 5, 1, charter.clone()), Err(Refusal::Scope)),
        ("rule number zero", item(1, 0, 0, plain.clone()), Err(Refusal::Number)),
        ("rule number past bound", item(2, 5, 5, plain.clone()), Err(Refusal::Number)),
        ("charter number zero", item(3, 0, 0, charter.clone()), Err(Refusal::Number)),
        ("charter number two", item(3, 0, 2, charter.clone()), Err(Refusal::Number)),
        ("charter removed", item(3, 0, 1, None), Err(Refusal::Removed)),
        ("deployment value wrong kind", item(1, 0, 1, charter.clone()), Err(Refusal::Decode)),
        ("project value wrong kind", item(2, 5, 1, charter.clone()), Err(Refusal::Decode)),
        ("charter value wrong kind", item(3, 0, 1, plain.clone()), Err(Refusal::Decode)),
    ])
}

fn value_refusal_rows() -> Box<[ItemRow]> {
    let empty = Last::Open(Box::new([]));
    let first: Box<[u8]> = b"forge".as_slice().into();
    let long: Box<[u8]> = Box::new([b'x'; 17]);
    Box::new([
        ("no first segment", deployment(pattern(Box::new([]), empty.clone())), Err(Refusal::Pattern)),
        (
            "non-forge first segment",
            deployment(pattern(Box::new([b"other".as_slice().into()]), empty.clone())),
            Err(Refusal::Pattern),
        ),
        (
            "segment count past bound",
            deployment(pattern(
                Box::new([
                    first.clone(),
                    first.clone(),
                    first.clone(),
                    first.clone(),
                    first.clone(),
                    first.clone(),
                    first.clone(),
                ]),
                empty.clone(),
            )),
            Err(Refusal::Pattern),
        ),
        (
            "base bytes past bound",
            deployment(pattern(Box::new([first.clone(), long.clone()]), empty)),
            Err(Refusal::Pattern),
        ),
        (
            "exact terminal past bound",
            deployment(pattern(Box::new([first.clone()]), Last::Exact(long.clone()))),
            Err(Refusal::Pattern),
        ),
        (
            "open terminal past bound",
            deployment(pattern(Box::new([first.clone()]), Last::Open(long))),
            Err(Refusal::Pattern),
        ),
        (
            "one gate past count bound",
            deployment(LandingRule { gates: Box::new([rule_gate(1), rule_gate(2), rule_gate(3)]), ..rule() }),
            Err(Refusal::Gates),
        ),
        (
            "duplicate gate number",
            deployment(LandingRule { gates: Box::new([rule_gate(1), rule_gate(1)]), ..rule() }),
            Err(Refusal::Gates),
        ),
        (
            "zero approvers",
            deployment(LandingRule { approvals: Box::new([rule_approval(1, 0)]), ..rule() }),
            Err(Refusal::Approvals),
        ),
        (
            "duplicate approval role",
            deployment(LandingRule { approvals: Box::new([rule_approval(1, 1), rule_approval(1, 2)]), ..rule() }),
            Err(Refusal::Approvals),
        ),
        (
            "approval role at bound",
            deployment(LandingRule { approvals: Box::new([rule_approval(4, 1)]), ..rule() }),
            Err(Refusal::Approvals),
        ),
        (
            "approval role past bound",
            deployment(LandingRule { approvals: Box::new([rule_approval(5, 1)]), ..rule() }),
            Err(Refusal::Approvals),
        ),
        ("charter zero", item(3, 0, 1, Some(Value::Charter(0))), Err(Refusal::Charter)),
        ("charter past bound", item(3, 0, 1, Some(Value::Charter(9))), Err(Refusal::Charter)),
    ])
}

fn accepted_item_rows() -> Box<[ItemRow]> {
    let plain = Some(Value::Rule(rule()));
    let charter = Some(Value::Charter(1));
    let first: Box<[u8]> = b"forge".as_slice().into();
    Box::new([
        ("deployment rule", item(1, 0, 1, plain.clone()), Ok(())),
        ("project rule at last number", item(2, 5, 4, plain), Ok(())),
        ("deployment rule removed", item(1, 0, 1, None), Ok(())),
        ("project rule removed", item(2, 5, 4, None), Ok(())),
        ("charter first", item(3, 0, 1, charter), Ok(())),
        ("charter last", item(3, 0, 1, Some(Value::Charter(8))), Ok(())),
        (
            "gate numbers are identifiers",
            deployment(LandingRule { gates: Box::new([rule_gate(0), rule_gate(u32::MAX)]), ..rule() }),
            Ok(()),
        ),
        (
            "approval roles need not be held",
            deployment(LandingRule {
                approvals: Box::new([rule_approval(0, 1), rule_approval(3, u32::MAX)]),
                ..rule()
            }),
            Ok(()),
        ),
        (
            "pattern at bounds",
            deployment(pattern(
                Box::new([first.clone(), first.clone(), first.clone(), first.clone(), first.clone(), first]),
                Last::Exact(Box::new([b'x'; 16])),
            )),
            Ok(()),
        ),
    ])
}

#[test]
fn the_kind_numbers_are_in_jigs_application_space() {
    for (number, kind) in [(1, Kind::DeploymentRules), (2, Kind::ProjectRules), (3, Kind::ForgeCharter)] {
        assert_eq!(kind.number(), number);
        assert_eq!(Kind::of(number), Some(kind));
    }
    for number in [0, 4, u32::MAX] {
        assert_eq!(Kind::of(number), None);
    }
}

fn branch(last: &[u8]) -> Name {
    // route/forge_route.rs's resource_name: the merge's full authority path.
    Name {
        segments: Box::new([
            b"forge".as_slice().into(),
            b"forge.example".as_slice().into(),
            b"org".as_slice().into(),
            b"repo".as_slice().into(),
            b"branch".as_slice().into(),
            last.into(),
        ]),
    }
}

fn branch_rule(last: Last) -> LandingRule {
    let segments = Box::new([
        b"forge".as_slice().into(),
        b"forge.example".as_slice().into(),
        b"org".as_slice().into(),
        b"repo".as_slice().into(),
        b"branch".as_slice().into(),
    ]);
    LandingRule { pattern: Pattern { segments, last }, ci: true, ..rule() }
}

fn evidence(ci: Status, contains_base: Status, gates: Box<[change::GateReport]>) -> ChangeEvidence {
    ChangeEvidence {
        prior: super::fixtures::opening_row().change,
        head: Some(HEAD),
        base_tip: super::fixtures::BASE,
        contains_base,
        ci,
        gates,
        reviews: Box::new([]),
        reviews_complete: true,
    }
}

fn report(number: u64, status: Status, head: skein_forge::api::Commit) -> change::GateReport {
    change::GateReport { number, head, status }
}

fn reviewer(verdict: skein_forge::api::Verdict, head: skein_forge::api::Commit) -> Reviewer {
    Reviewer { person: 7, role: 1, head, verdict }
}

fn held(scope: u32, number: u32, rule: LandingRule) -> Held {
    Held { scope, number, rule }
}

fn verdict(verdict: JudgeVerdict, scope: u32, number: u32, criterion: Criterion) -> StandingVerdict {
    StandingVerdict { verdict, rule: Some((scope, number)), criterion: Some(criterion) }
}

struct Row {
    name: &'static str,
    rules: Box<[Held]>,
    branch: Name,
    evidence: ChangeEvidence,
    reviewers: Box<[Reviewer]>,
    clean: Box<[skein_forge::api::Commit]>,
    which: Standing,
    expected: StandingVerdict,
}

fn row(
    name: &'static str,
    rules: Box<[Held]>,
    evidence: ChangeEvidence,
    which: Standing,
    expected: StandingVerdict,
) -> Row {
    Row {
        name,
        rules,
        branch: branch(b"main"),
        evidence,
        reviewers: Box::new([]),
        clean: Box::new([]),
        which,
        expected,
    }
}

#[test]
fn the_standing_verdicts_over_a_table_of_rules() {
    for table in [
        ci_and_gate_rows(),
        approval_and_current_rows(),
        combining_rule_rows(),
        coverage_rows(),
        head_and_freshness_rows(),
        criterion_order_rows(),
    ] {
        for row in table {
            assert_eq!(
                settings::standing(
                    row.which,
                    &row.rules,
                    &row.branch,
                    &super::adopted(),
                    HEAD,
                    &row.clean,
                    &row.evidence,
                    &row.reviewers
                ),
                row.expected,
                "{}",
                row.name
            );
        }
    }
}

fn ci_and_gate_rows() -> Box<[Row]> {
    let ci = LandingRule { ci: true, ..rule() };
    let gate = LandingRule { gates: Box::new([rule_gate(2)]), ..rule() };
    let met = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };
    let gate_criterion = Criterion::Gate { number: 2, freshness: EXACT };
    Box::new([
        row(
            "deployment CI met",
            Box::new([held(0, 1, ci.clone())]),
            evidence(Status::Passed, Status::Unknown, Box::new([])),
            Standing::Rules,
            met,
        ),
        row(
            "deployment CI waits",
            Box::new([held(0, 1, ci.clone())]),
            evidence(Status::Pending, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Wait, 0, 1, Criterion::Ci),
        ),
        row(
            "deployment CI refuses",
            Box::new([held(0, 1, ci.clone())]),
            evidence(Status::Failed, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 0, 1, Criterion::Ci),
        ),
        row(
            "project adds gate met",
            Box::new([held(0, 1, ci.clone()), held(5, 2, gate.clone())]),
            evidence(Status::Passed, Status::Unknown, Box::new([report(2, Status::Passed, HEAD)])),
            Standing::Rules,
            met,
        ),
        row(
            "project adds gate wait",
            Box::new([held(0, 1, ci.clone()), held(5, 2, gate.clone())]),
            evidence(Status::Passed, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Wait, 5, 2, gate_criterion),
        ),
        row(
            "project adds gate refuse",
            Box::new([held(0, 1, ci.clone()), held(5, 2, gate.clone())]),
            evidence(Status::Passed, Status::Unknown, Box::new([report(2, Status::Failed, HEAD)])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 5, 2, gate_criterion),
        ),
    ])
}

fn approval_and_current_rows() -> Box<[Row]> {
    let approval = LandingRule { approvals: Box::new([rule_approval(1, 1)]), ..rule() };
    let current = LandingRule { up_to_date: true, ..rule() };
    let met = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };
    let approval_criterion = Criterion::Approval { role: 1, people: 1, freshness: EXACT };
    Box::new([
        Row {
            reviewers: Box::new([reviewer(skein_forge::api::Verdict::Approve, HEAD)]),
            ..row(
                "approval met",
                Box::new([held(5, 1, approval.clone())]),
                evidence(Status::Unknown, Status::Unknown, Box::new([])),
                Standing::Rules,
                met,
            )
        },
        row(
            "approval waits",
            Box::new([held(5, 1, approval.clone())]),
            evidence(Status::Unknown, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Wait, 5, 1, approval_criterion),
        ),
        Row {
            reviewers: Box::new([reviewer(skein_forge::api::Verdict::RequestChanges, HEAD)]),
            ..row(
                "approval refuses",
                Box::new([held(5, 1, approval.clone())]),
                evidence(Status::Unknown, Status::Unknown, Box::new([])),
                Standing::Rules,
                verdict(JudgeVerdict::Refuse, 5, 1, approval_criterion),
            )
        },
        row(
            "up to date met",
            Box::new([held(0, 1, current.clone())]),
            evidence(Status::Unknown, Status::Passed, Box::new([])),
            Standing::UpToDate,
            met,
        ),
        row(
            "up to date waits",
            Box::new([held(0, 1, current.clone())]),
            evidence(Status::Unknown, Status::Unknown, Box::new([])),
            Standing::UpToDate,
            verdict(JudgeVerdict::Wait, 0, 1, Criterion::UpToDate),
        ),
        row(
            "up to date refuses",
            Box::new([held(5, 1, current.clone())]),
            evidence(Status::Unknown, Status::Failed, Box::new([])),
            Standing::UpToDate,
            verdict(JudgeVerdict::Refuse, 5, 1, Criterion::UpToDate),
        ),
    ])
}

fn combining_rule_rows() -> Box<[Row]> {
    let ci = LandingRule { ci: true, ..rule() };
    let gate = LandingRule { gates: Box::new([rule_gate(2)]), ..rule() };
    let approval = LandingRule { approvals: Box::new([rule_approval(1, 1)]), ..rule() };
    let current = LandingRule { up_to_date: true, ..rule() };
    let met = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };
    let gate_criterion = Criterion::Gate { number: 2, freshness: EXACT };
    Box::new([
        row(
            "refusal wins over earlier wait",
            Box::new([held(0, 1, ci.clone()), held(5, 2, gate.clone())]),
            evidence(Status::Pending, Status::Unknown, Box::new([report(2, Status::Failed, HEAD)])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 5, 2, gate_criterion),
        ),
        row(
            "first wait kept",
            Box::new([held(0, 1, ci.clone()), held(0, 2, gate.clone()), held(5, 1, approval.clone())]),
            evidence(Status::Unknown, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Wait, 0, 1, Criterion::Ci),
        ),
        row(
            "first refusal kept",
            Box::new([held(0, 2, gate.clone()), held(5, 1, ci.clone())]),
            evidence(Status::Failed, Status::Unknown, Box::new([report(2, Status::Failed, HEAD)])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 0, 2, gate_criterion),
        ),
        row(
            "advisory rule skipped",
            Box::new([held(0, 1, LandingRule { enforce: false, ..ci.clone() }), held(5, 1, gate.clone())]),
            evidence(Status::Failed, Status::Unknown, Box::new([report(2, Status::Passed, HEAD)])),
            Standing::Rules,
            met,
        ),
        row(
            "non-blocking gate skipped",
            Box::new([held(
                0,
                1,
                LandingRule { gates: Box::new([RuleGate { blocking: false, ..rule_gate(2) }]), ..rule() },
            )]),
            evidence(Status::Unknown, Status::Unknown, Box::new([report(2, Status::Failed, HEAD)])),
            Standing::Rules,
            met,
        ),
        row(
            "rules excludes containment",
            Box::new([held(0, 1, current.clone())]),
            evidence(Status::Unknown, Status::Failed, Box::new([])),
            Standing::Rules,
            met,
        ),
    ])
}

fn coverage_rows() -> Box<[Row]> {
    let ci = LandingRule { ci: true, ..rule() };
    let met = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };
    Box::new([
        row(
            "containment excludes CI",
            Box::new([held(0, 1, ci.clone())]),
            evidence(Status::Failed, Status::Unknown, Box::new([])),
            Standing::UpToDate,
            met,
        ),
        Row {
            branch: branch(b"other"),
            ..row(
                "uncovered branch rules met",
                Box::new([held(0, 1, branch_rule(Last::Exact(b"main".as_slice().into())))]),
                evidence(Status::Failed, Status::Failed, Box::new([])),
                Standing::Rules,
                met,
            )
        },
        Row {
            branch: branch(b"other"),
            ..row(
                "uncovered branch containment met",
                Box::new([held(
                    0,
                    1,
                    LandingRule { up_to_date: true, ..branch_rule(Last::Exact(b"main".as_slice().into())) },
                )]),
                evidence(Status::Failed, Status::Failed, Box::new([])),
                Standing::UpToDate,
                met,
            )
        },
        row(
            "exact branch covered",
            Box::new([held(0, 1, branch_rule(Last::Exact(b"main".as_slice().into())))]),
            evidence(Status::Failed, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 0, 1, Criterion::Ci),
        ),
        row(
            "open branch prefix covered",
            Box::new([held(0, 1, branch_rule(Last::Open(b"ma".as_slice().into())))]),
            evidence(Status::Pending, Status::Unknown, Box::new([])),
            Standing::Rules,
            verdict(JudgeVerdict::Wait, 0, 1, Criterion::Ci),
        ),
        row(
            "no held rules",
            Box::new([]),
            evidence(Status::Failed, Status::Failed, Box::new([])),
            Standing::Rules,
            met,
        ),
    ])
}

fn head_and_freshness_rows() -> Box<[Row]> {
    let ci = LandingRule { ci: true, ..rule() };
    let gate = LandingRule { gates: Box::new([rule_gate(2)]), ..rule() };
    let approval = LandingRule { approvals: Box::new([rule_approval(1, 1)]), ..rule() };
    let met = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };
    let gate_criterion = Criterion::Gate { number: 2, freshness: EXACT };
    let approval_criterion = Criterion::Approval { role: 1, people: 1, freshness: EXACT };
    Box::new([
        Row {
            evidence: ChangeEvidence { head: Some(OLD), ..evidence(Status::Passed, Status::Passed, Box::new([])) },
            ..row(
                "evidence for another head waits",
                Box::new([held(0, 1, ci.clone())]),
                evidence(Status::Passed, Status::Passed, Box::new([])),
                Standing::Rules,
                verdict(JudgeVerdict::Wait, 0, 1, Criterion::Ci),
            )
        },
        Row {
            clean: Box::new([OLD]),
            ..row(
                "exact gate ignores clean predecessor",
                Box::new([held(0, 1, gate.clone())]),
                evidence(Status::Unknown, Status::Unknown, Box::new([report(2, Status::Passed, OLD)])),
                Standing::Rules,
                verdict(JudgeVerdict::Wait, 0, 1, gate_criterion),
            )
        },
        Row {
            clean: Box::new([OLD]),
            ..row(
                "clean gate uses predecessor",
                Box::new([held(
                    0,
                    1,
                    LandingRule { gates: Box::new([RuleGate { freshness: CLEAN, ..rule_gate(2) }]), ..rule() },
                )]),
                evidence(Status::Unknown, Status::Unknown, Box::new([report(2, Status::Passed, OLD)])),
                Standing::Rules,
                met,
            )
        },
        Row {
            clean: Box::new([OLD]),
            reviewers: Box::new([reviewer(skein_forge::api::Verdict::Approve, OLD)]),
            ..row(
                "exact approval ignores predecessor",
                Box::new([held(5, 1, approval.clone())]),
                evidence(Status::Unknown, Status::Unknown, Box::new([])),
                Standing::Rules,
                verdict(JudgeVerdict::Wait, 5, 1, approval_criterion),
            )
        },
        Row {
            clean: Box::new([OLD]),
            reviewers: Box::new([reviewer(skein_forge::api::Verdict::Approve, OLD)]),
            ..row(
                "clean approval uses predecessor",
                Box::new([held(
                    5,
                    1,
                    LandingRule {
                        approvals: Box::new([RuleApproval { freshness: CLEAN, ..rule_approval(1, 1) }]),
                        ..rule()
                    },
                )]),
                evidence(Status::Unknown, Status::Unknown, Box::new([])),
                Standing::Rules,
                met,
            )
        },
        row(
            "refusal within rule wins over CI wait",
            Box::new([held(0, 1, LandingRule { ci: true, ..gate.clone() })]),
            evidence(Status::Pending, Status::Unknown, Box::new([report(2, Status::Failed, HEAD)])),
            Standing::Rules,
            verdict(JudgeVerdict::Refuse, 0, 1, gate_criterion),
        ),
    ])
}

fn criterion_order_rows() -> Box<[Row]> {
    let approval = LandingRule { approvals: Box::new([rule_approval(1, 1)]), ..rule() };
    Box::new([row(
        "same rule first criterion wait",
        Box::new([held(0, 1, LandingRule { ci: true, ..approval })]),
        evidence(Status::Unknown, Status::Unknown, Box::new([])),
        Standing::Rules,
        verdict(JudgeVerdict::Wait, 0, 1, Criterion::Ci),
    )])
}
