//! temper's kinds of settings, the forge's to judge and hold
//! (protocol/configuration.md, 13.2 and 13.3; domain/forge.md, 8.7).

use alloc::boxed::Box;
use jig_domain_authority::{Last, Name, Pattern, pattern_covers};

use crate::{ChangeEvidence, Criterion, JudgeFreshness, JudgeVerdict, Repository, Reviewer};

/// temper's three kinds, numbered in jig's space for an application's kinds (13.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    DeploymentRules,
    ProjectRules,
    ForgeCharter,
}
impl Kind {
    #[must_use]
    pub const fn number(self) -> u32 {
        match self {
            Self::DeploymentRules => 1,
            Self::ProjectRules => 2,
            Self::ForgeCharter => 3,
        }
    }

    #[must_use]
    pub const fn of(number: u32) -> Option<Kind> {
        match number {
            1 => Some(Self::DeploymentRules),
            2 => Some(Self::ProjectRules),
            3 => Some(Self::ForgeCharter),
            _ => None,
        }
    }
}

/// Section 3's landing rule without connector and kind, always the forge's merge.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LandingRule {
    pub pattern: Pattern,
    pub enforce: bool,
    pub ci: bool,
    pub up_to_date: bool,
    pub gates: Box<[RuleGate]>,
    pub approvals: Box<[RuleApproval]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleGate {
    pub number: u32,
    pub blocking: bool,
    pub freshness: JudgeFreshness,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleApproval {
    pub role: u32,
    pub people: u32,
    pub freshness: JudgeFreshness,
}

/// An item's bounds, the forge's and jig's (13.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bounds {
    /// `forge.landing_rules`: a scope's rules, numbered from 1.
    pub landing_rules: u32,
    /// `forge.change_policy.gates`: the count, not a gate's number.
    pub gates: u32,
    /// jig's `authority.roles`: an approval's role is below it.
    pub roles: u32,
    /// jig's `tasks.charters`: the forge's charter is from 1 to it.
    pub charters: u32,
    pub segments: u32,
    pub segment_bytes: u32,
}

/// A value of temper's kinds, decoded (13.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Value {
    Rule(LandingRule),
    Charter(u32),
}

/// An item as the root hands it to be judged; none for a removal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub kind: u32,
    pub scope: u32,
    pub number: u32,
    pub value: Option<Value>,
}

/// What fails in an item, named by kind, scope and number in the forge's words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// Not one of temper's kinds.
    Kind,
    /// Kinds 1 and 3 require scope 0; kind 2 requires a nonzero project scope.
    Scope,
    /// Rule number outside 1 to `landing_rules`, or charter number other than 1.
    Number,
    /// The charter removed: it is set or reset, never removed.
    Removed,
    /// A value of the wrong kind, or bytes the protocol layer could not decode.
    Decode,
    /// First pattern segment other than `forge`, or jig's pattern bounds exceeded.
    Pattern,
    /// More gates than `gates`, or a gate's number twice.
    Gates,
    /// No people, a role twice, or a role at or past `roles`.
    Approvals,
    /// Charter outside 1 to `charters`.
    Charter,
}

/// Judge one decoded item within `bounds` alone, reading no other state (13.2).
/// # Errors
/// What fails in it. The protocol layer refuses undecodable bytes as `Decode`.
pub fn judge(item: &Item, bounds: &Bounds) -> Result<(), Refusal> {
    let kind = Kind::of(item.kind).ok_or(Refusal::Kind)?;
    let scope = match kind {
        Kind::DeploymentRules | Kind::ForgeCharter => item.scope == 0,
        Kind::ProjectRules => item.scope != 0,
    };
    if !scope {
        return Err(Refusal::Scope);
    }
    let number = match kind {
        Kind::DeploymentRules | Kind::ProjectRules => (1..=bounds.landing_rules).contains(&item.number),
        Kind::ForgeCharter => item.number == 1,
    };
    if !number {
        return Err(Refusal::Number);
    }
    match (&item.value, kind) {
        (None, Kind::DeploymentRules | Kind::ProjectRules) => Ok(()),
        (None, Kind::ForgeCharter) => Err(Refusal::Removed),
        (Some(Value::Charter(charter)), Kind::ForgeCharter) => {
            if (1..=bounds.charters).contains(charter) {
                Ok(())
            } else {
                Err(Refusal::Charter)
            }
        }
        (Some(Value::Rule(rule)), Kind::DeploymentRules | Kind::ProjectRules) => judge_rule(rule, bounds),
        (Some(Value::Rule(_)), Kind::ForgeCharter)
        | (Some(Value::Charter(_)), Kind::DeploymentRules | Kind::ProjectRules) => Err(Refusal::Decode),
    }
}

fn within(length: usize, bound: u32) -> bool {
    length <= usize::try_from(bound).expect("u32 bounds fit usize")
}

fn judge_rule(rule: &LandingRule, bounds: &Bounds) -> Result<(), Refusal> {
    let forge = match rule.pattern.segments.first() {
        Some(segment) => segment.as_ref() == b"forge",
        None => false,
    };
    if !forge || !within(rule.pattern.segments.len(), bounds.segments) {
        return Err(Refusal::Pattern);
    }
    for segment in &rule.pattern.segments {
        if !within(segment.len(), bounds.segment_bytes) {
            return Err(Refusal::Pattern);
        }
    }
    let (Last::Exact(last) | Last::Open(last)) = &rule.pattern.last;
    if !within(last.len(), bounds.segment_bytes) {
        return Err(Refusal::Pattern);
    }
    if !within(rule.gates.len(), bounds.gates) {
        return Err(Refusal::Gates);
    }
    for (at, gate) in rule.gates.iter().enumerate() {
        for earlier in rule.gates.get(..at).expect("gate position in bounds") {
            if earlier.number == gate.number {
                return Err(Refusal::Gates);
            }
        }
    }
    if !within(rule.approvals.len(), bounds.roles) {
        return Err(Refusal::Approvals);
    }
    for (at, approval) in rule.approvals.iter().enumerate() {
        if approval.people == 0 || approval.role >= bounds.roles {
            return Err(Refusal::Approvals);
        }
        for earlier in rule.approvals.get(..at).expect("approval position in bounds") {
            if earlier.role == approval.role {
                return Err(Refusal::Approvals);
            }
        }
    }
    Ok(())
}

/// One held rule, read in scope/number order by the standing verdicts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Held {
    pub scope: u32,
    pub number: u32,
    pub rule: LandingRule,
}

/// Requirements 5 (CI, blocking gates and approvals) and 6 (up to date).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Standing {
    Rules,
    UpToDate,
}

/// The verdict and its first refusing criterion, else its first waiting one (13.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StandingVerdict {
    pub verdict: JudgeVerdict,
    pub rule: Option<(u32, u32)>,
    pub criterion: Option<Criterion>,
}

const MET: StandingVerdict = StandingVerdict { verdict: JudgeVerdict::Met, rule: None, criterion: None };

/// Judge every covering enforcing rule supplied for this deployment and project.
/// Rules are supplied deployment first, then project, each by number. A refusal
/// wins over a wait; otherwise the first wait is kept. No covering rule meets both.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "a standing judgement names its rules, branch and coherent change evidence"
)]
pub fn standing(
    which: Standing,
    rules: &[Held],
    branch: &Name,
    repository: &Repository,
    head: skein_forge::api::Commit,
    clean: &[skein_forge::api::Commit],
    evidence: &ChangeEvidence,
    reviewers: &[Reviewer],
) -> StandingVerdict {
    let mut result = MET;
    for entry in rules {
        if !entry.rule.enforce || !pattern_covers(&entry.rule.pattern, branch) {
            continue;
        }
        let judged = standing_rule(which, entry, repository, head, clean, evidence, reviewers);
        match judged.verdict {
            JudgeVerdict::Refuse => return judged,
            JudgeVerdict::Wait if result.verdict == JudgeVerdict::Met => result = judged,
            JudgeVerdict::Wait | JudgeVerdict::Met => {}
        }
    }
    result
}

fn standing_rule(
    which: Standing,
    entry: &Held,
    repository: &Repository,
    head: skein_forge::api::Commit,
    clean: &[skein_forge::api::Commit],
    evidence: &ChangeEvidence,
    reviewers: &[Reviewer],
) -> StandingVerdict {
    let rule = &entry.rule;
    let mut result = MET;
    match which {
        Standing::Rules => {
            if rule.ci {
                let criterion = Criterion::Ci;
                let verdict = crate::judge::judge(criterion, repository, head, clean, evidence, reviewers);
                record(entry, criterion, verdict, &mut result);
            }
            for gate in &rule.gates {
                if gate.blocking {
                    let criterion = Criterion::Gate { number: gate.number, freshness: gate.freshness };
                    let verdict = crate::judge::judge(criterion, repository, head, clean, evidence, reviewers);
                    record(entry, criterion, verdict, &mut result);
                }
            }
            for approval in &rule.approvals {
                let criterion =
                    Criterion::Approval { role: approval.role, people: approval.people, freshness: approval.freshness };
                let verdict = crate::judge::judge(criterion, repository, head, clean, evidence, reviewers);
                record(entry, criterion, verdict, &mut result);
            }
        }
        Standing::UpToDate if rule.up_to_date => {
            let criterion = Criterion::UpToDate;
            let verdict = crate::judge::judge(criterion, repository, head, clean, evidence, reviewers);
            record(entry, criterion, verdict, &mut result);
        }
        Standing::UpToDate => {}
    }
    result
}

fn record(entry: &Held, criterion: Criterion, verdict: JudgeVerdict, result: &mut StandingVerdict) {
    if result.verdict == JudgeVerdict::Refuse || verdict == JudgeVerdict::Met {
        return;
    }
    if verdict == JudgeVerdict::Refuse || result.verdict == JudgeVerdict::Met {
        *result = StandingVerdict { verdict, rule: Some((entry.scope, entry.number)), criterion: Some(criterion) };
    }
}
