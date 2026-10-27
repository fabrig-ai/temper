//! Bounded connector top state (domain/root.md, section 10;
//! protocol/configuration.md, section 7).
use crate::{BranchHead, Criterion, Hold, Key, Name, PullState, Repository, Stored, Subscriber, Topic};
use core::mem::size_of;
use skein_lib::{Map, Queue};

/// GitHub App slugs become bot logins with this suffix (setup/lifecycle.rs).
pub const BOT_LOGIN_SUFFIX: &[u8] = b"[bot]";
/// Bounds `stopped_text`'s Debug: skein `connection::Stopped` is fixed-shape (boundary.rs:109–151).
pub const STOPPED_TEXT_BYTES: u64 = 128;

/// Last connect diagnostics are at most 256 UTF-8 bytes (client/forge.md, 5.4).
pub const CONNECT_WHY_BYTES: usize = 256;

/// Admin connects carry names checked at the wire ceiling, independently of the engine's setup name limit.
pub const FIRST_NAME_BYTES: u32 = 1024;

/// Capacity of each durable top table and its child.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub repositories: u32,
    pub tasks: u32,
    pub holds: u32,
    pub subscriptions: u32,
    pub entries: u32,
    /// Connector payloads of live named calls, priced independently of outbox entries.
    pub named_calls: u32,
    pub changes: u32,
    pub issues: u32,
    pub resources_per_task: u32,
    pub paths_per_subscription: u32,
    pub name_bytes: u32,
    /// Authority name depth, supplied from `authority.segments` by the root.
    pub authority_segments: u32,
    pub output: u32,
    pub facts: u32,
    pub adoptions: u32,
    pub applications: u32,
    /// Durable member address bindings (protocol/sign-in.md, 5).
    pub bindings: u32,
    pub collaborators: u32,
    pub landings: u32,
    /// Tiers per repository (domain/after-landing.md, 2).
    pub tiers: u32,
    /// Failing tests named by one run (domain/after-landing.md, 4).
    pub failing_tests: u32,
    /// Bytes of one test id.
    pub test_id_bytes: u32,
    /// Project judge tables retained beside committed policy.
    pub judge_projects: u32,
    /// Criteria in each deployment or project judge table.
    pub judge_criteria: u32,
    /// Connector brief sections gathering concurrently.
    pub brief_sections: u32,
    /// Maximum bytes retained for one gathered brief section.
    pub brief_bytes: u32,
    pub issue_policy: temper_engine_domain_forge_issues::Limits,
    pub change_policy: temper_engine_domain_forge_change::Limits,
    pub queue_window: skein_lib::Duration,
    pub client: skein_forge_client::Limits,
}

/// Maximum retained heap for the top and its child, excluding owned output
/// copies that the root takes at each decision boundary.
#[must_use]
#[expect(clippy::too_many_lines, reason = "one checked sum of the connector owners and retained payloads")]
pub fn worst_case(l: &Limits) -> Option<u64> {
    if l.repositories == 0
        || l.tasks == 0
        || l.holds == 0
        || l.subscriptions == 0
        || l.entries == 0
        || l.resources_per_task == 0
        || l.name_bytes == 0
        || l.name_bytes > crate::PANEL_NAME_BYTES
        || l.change_policy.gates > crate::PANEL_GATES
        || l.queue_window == skein_lib::Duration::ZERO
        || l.output == 0
        || l.adoptions == 0
        || l.applications == 0
        || l.collaborators == 0
        || l.landings == 0
        || l.brief_sections == 0
        || l.brief_bytes == 0
        || l.repositories > l.client.repositories
        // The top's entries, held ones included, are the client's only
        // makes, so a held entry made again always finds the client's room.
        || l.entries > l.client.entries
        || l.holds > l.client.resources
    {
        return None;
    }
    if l.output
        < skein_forge_client::max_out(&l.client)
            .checked_add(l.subscriptions.checked_mul(3)?)?
            .checked_add(l.holds.checked_mul(3)?)?
            .checked_add(9)?
        || l.output < skein_forge_client::max_out(&l.client).checked_add(l.resources_per_task)?.checked_add(1)?
    {
        return None;
    }
    // Connection and Restored emit no panel marks. Their ordinary cohort and
    // retirement's Keep each fit output (Keep <= client.max_out <= output),
    // so 2 * output covers both. Other events use that second cohort for
    // panel marks instead; application answers and demo acts are separate.
    l.output.checked_mul(2)?.checked_add(l.applications)?.checked_add(demo_act_bound(l)?.checked_mul(2)?)?;
    let names =
        u64::from(l.tasks).checked_mul(u64::from(l.resources_per_task))?.checked_mul(u64::from(l.name_bytes))?;
    let goal_tasks = u64::from(l.subscriptions).checked_mul(u64::from(l.issue_policy.plan_items))?.checked_mul(8)?;
    let subscribers = u64::from(l.subscriptions)
        .checked_mul(u64::from(l.paths_per_subscription))?
        .checked_mul(u64::from(l.name_bytes))?;
    let criteria = u64::from(l.judge_criteria).checked_mul(u64::try_from(size_of::<Criterion>()).ok()?)?;
    let judges = Map::<u32, Box<[Criterion]>>::worst_case(l.judge_projects)?
        .checked_add(criteria.checked_mul(u64::from(l.judge_projects).checked_add(1)?)?)?;
    // Restart retains row references until reconciliation.
    skein_lib::List::<crate::reconcile::Reference>::worst_case(crate::reconcile::capacity(l)?)?
        .checked_add(Map::<Box<[u8]>, Box<[u8]>>::worst_case(l.bindings)?)?
        .checked_add(u64::from(l.bindings).checked_mul(254 + 255)?)?
        .checked_add(crate::adoption::worst_case(l)?)?
        .checked_add(retirement_scratch(l)?)?
        .checked_add(Map::<[u8; 16], crate::DemoActKind>::worst_case(demo_act_bound(l)?)?)?
        .checked_add(crate::connect::worst_case(l.applications)?)?
        .checked_add(Map::<skein_lib::Token, skein_lib::Token>::worst_case(l.applications)?)?
        .checked_add(Map::<skein_lib::Token, crate::StartAnswer>::worst_case(l.applications)?)?
        .checked_add(u64::from(l.applications).checked_mul(256)?)?
        .checked_add(5 * 1024)?
        .checked_add(Map::<skein_lib::Token, skein_lib::Token>::worst_case(l.applications)?)?
        .checked_add(Map::<u16, crate::Configured>::worst_case(l.client.repositories)?)?
        // Public startup details: the git/web addresses and setup entry's bot login.
        .checked_add(u64::from(l.client.repositories).checked_mul(2048 + 2048 + 40)?)?
        // Owned startup details: two versions and the first-password file path.
        .checked_add(u64::from(l.client.repositories).checked_mul(64 + 64 + 512)?)?
        .checked_add(Map::<u16, crate::First>::worst_case(l.client.repositories)?)?
        .checked_add(Map::<u16, crate::AppWay>::worst_case(l.client.repositories)?)?
        .checked_add(Map::<u16, (Box<[u8]>, Option<crate::AdminStop>)>::worst_case(l.client.repositories)?)?
        .checked_add(u64::from(l.client.repositories).checked_mul(u64::try_from(CONNECT_WHY_BYTES).ok()?)?)?
        .checked_add(
            u64::from(l.client.repositories)
                .checked_mul(u64::from(l.name_bytes.max(FIRST_NAME_BYTES)).checked_mul(3)?.checked_add(64)?)?,
        )?
        .checked_add(skein_forge_client::worst_case(&l.client)?)?
        .checked_add(Map::<skein_forge::api::Repository, Repository>::worst_case(l.repositories)?)?
        .checked_add(Map::<u16, crate::domain::ConnectionState>::worst_case(l.client.repositories)?)?
        .checked_add(u64::from(l.client.repositories).checked_mul(
            u64::from(l.name_bytes).checked_add(u64::try_from(BOT_LOGIN_SUFFIX.len()).ok()?)?.max(STOPPED_TEXT_BYTES),
        )?)?
        .checked_add(Map::<(u16, u64), crate::Installation>::worst_case(l.client.rows)?)?
        .checked_add(u64::from(l.client.rows).checked_mul(u64::from(l.name_bytes))?)?
        // Each forge's writer and git identity.
        .checked_add(Map::<u16, u64>::worst_case(l.client.repositories)?)?
        .checked_add(Map::<u16, u32>::worst_case(l.repositories)?)?
        .checked_add(
            u64::from(l.repositories)
                .checked_mul(u64::from(l.change_policy.gates))?
                .checked_mul(u64::try_from(size_of::<u32>()).ok()?)?,
        )?
        .checked_add(Map::<u64, Box<[Name]>>::worst_case(l.tasks)?)?
        .checked_add(Map::<Name, Hold>::worst_case(l.holds)?)?
        .checked_add(skein_lib::List::<skein_forge_client::Resource>::worst_case(l.holds)?)?
        .checked_add(u64::from(l.holds).checked_mul(u64::from(l.name_bytes))?)?
        .checked_add(u64::from(l.holds).checked_mul(u64::from(l.client.answer_bytes).checked_mul(2)?)?)?
        .checked_add(Map::<(u64, Topic), Subscriber>::worst_case(l.subscriptions)?)?
        .checked_add(Map::<Name, BranchHead>::worst_case(l.client.resources)?)?
        .checked_add(Map::<Name, PullState>::worst_case(l.client.resources)?)?
        .checked_add(Map::<Name, bool>::worst_case(l.client.resources)?)?
        .checked_add(Map::<(skein_forge::api::Repository, skein_forge::api::Commit), crate::CiState>::worst_case(
            l.subscriptions,
        )?)?
        .checked_add(Map::<skein_lib::Token, crate::domain::PendingCi>::worst_case(l.subscriptions)?)?
        .checked_add(Map::<skein_lib::Token, crate::capabilities::Pending>::worst_case(l.repositories)?)?
        .checked_add(u64::from(l.repositories).checked_mul(u64::from(l.client.answer_bytes))?)?
        .checked_add(Map::<u64, crate::domain::Outboxed>::worst_case(l.entries)?)?
        // One key-only flight per attempted entry (domain/forge.md, 6).
        .checked_add(Map::<(u64, u64), ()>::worst_case(l.entries)?)?
        .checked_add(u64::from(l.entries).checked_mul(u64::try_from(size_of::<crate::Unfinished>()).ok()?)?)?
        .checked_add(Map::<crate::NamedCall, crate::CallPayload>::worst_case(l.named_calls)?)?
        .checked_add(u64::from(l.named_calls).checked_mul(u64::from(l.client.answer_bytes))?)?
        .checked_add(Map::<u64, crate::ProposedEffect>::worst_case(l.tasks)?)?
        .checked_add(
            u64::from(l.tasks).checked_mul(u64::from(l.client.op_bytes).checked_add(u64::from(l.name_bytes))?)?,
        )?
        .checked_add(crate::verification::worst_case(l)?)?
        .checked_add(Map::<skein_forge::api::Commit, u64>::worst_case(l.landings)?)?
        .checked_add(Map::<skein_lib::Token, crate::domain::PendingLanding>::worst_case(l.landings)?)?
        .checked_add(Map::<skein_lib::Token, crate::domain::PendingLost>::worst_case(l.holds)?)?
        .checked_add(Map::<u64, crate::ChangeRow>::worst_case(l.changes)?)?
        .checked_add(
            u64::from(l.changes)
                .checked_mul(u64::from(l.change_policy.gates))?
                .checked_mul(u64::try_from(size_of::<temper_engine_domain_forge_change::GateFailure>()).ok()?)?,
        )?
        .checked_add(Map::<u64, temper_engine_domain_forge_change::Ci>::worst_case(l.changes)?)?
        .checked_add(skein_lib::List::<(Box<[u8]>, crate::DriftChange)>::worst_case(l.changes)?)?
        .checked_add(u64::from(l.changes).checked_mul(u64::from(l.name_bytes).checked_mul(3)?)?)?
        .checked_add(Map::<u64, crate::topics::Files>::worst_case(l.changes)?)?
        .checked_add(Map::<skein_lib::Token, crate::topics::Pending>::worst_case(l.changes)?)?
        .checked_add(file_payload(l)?)?
        .checked_add(
            u64::from(l.changes)
                .checked_mul(u64::from(l.change_policy.gates))?
                .checked_mul(u64::try_from(size_of::<temper_engine_domain_forge_change::GateReport>()).ok()?)?,
        )?
        .checked_add(u64::from(l.changes).checked_mul(u64::from(l.change_policy.gates))?.checked_mul(
            u64::try_from(size_of::<crate::GateRemark>()).ok()?.checked_add(u64::from(l.client.answer_bytes))?,
        )?)?
        .checked_add(Map::<skein_lib::Token, crate::domain::PendingStep>::worst_case(l.changes)?)?
        .checked_add(Map::<u64, crate::IssueRow>::worst_case(l.issues)?)?
        .checked_add(Map::<u64, skein_lib::Wall>::worst_case(l.issues)?)?
        .checked_add(Map::<u64, skein_lib::Wall>::worst_case(l.changes)?)?
        .checked_add(Map::<u64, bool>::worst_case(l.changes)?)?
        .checked_add(Map::<u64, crate::settle::Settle>::worst_case(l.changes)?)?
        .checked_add(Map::<u64, crate::ReleaseRow>::worst_case(l.tasks)?)?
        // A cancel reason is bounded by jig result_bytes, checked against op_bytes by the root.
        .checked_add(u64::from(l.tasks).checked_mul(u64::from(l.client.op_bytes))?)?
        .checked_add(u64::from(l.tasks).checked_mul(u64::from(l.name_bytes))?)?
        .checked_add(issue_payload(l)?)?
        .checked_add(Map::<skein_lib::Token, crate::adoption::PendingAdoption>::worst_case(l.adoptions)?)?
        .checked_add(Map::<skein_lib::Token, crate::brief::BriefFetch>::worst_case(l.brief_sections)?)?
        .checked_add(Map::<skein_lib::Token, crate::BriefSource>::worst_case(l.brief_sections)?)?
        .checked_add(Map::<skein_lib::Token, crate::held::Pending>::worst_case(l.brief_sections)?)?
        .checked_add(Map::<skein_lib::Token, crate::held::Held>::worst_case(l.brief_sections)?)?
        .checked_add(u64::from(l.brief_sections).checked_mul(u64::from(l.brief_bytes))?)?
        .checked_add(u64::from(l.brief_sections).checked_mul(u64::from(l.brief_bytes))?)?
        .checked_add(u64::from(l.brief_sections).checked_mul(u64::from(l.client.answer_bytes))?)?
        .checked_add(u64::from(l.brief_bytes).checked_mul(2)?)?
        .checked_add(Queue::<Stored>::worst_case(l.output)?)?
        .checked_add(Queue::<Key>::worst_case(l.output)?)?
        .checked_add(Queue::<skein_forge_client::Fact>::worst_case(l.facts)?)?
        .checked_add(names)?
        .checked_add(subscribers)?
        .checked_add(goal_tasks)?
        .checked_add(judges)
}

use alloc::boxed::Box;

fn file_payload(l: &Limits) -> Option<u64> {
    u64::from(l.changes)
        .checked_mul(u64::from(l.paths_per_subscription))?
        .checked_mul(u64::from(l.name_bytes).checked_add(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?)?
        .checked_mul(2)
}

fn issue_payload(l: &Limits) -> Option<u64> {
    use temper_engine_domain_forge_issues::{Milestone, MilestoneKey, PlanItem};
    let plan = u64::from(l.issue_policy.plan_items)
        .checked_mul(u64::from(l.issue_policy.body_bytes).checked_add(u64::try_from(size_of::<PlanItem>()).ok()?)?)?;
    let milestones = u64::from(l.issue_policy.milestones).checked_mul(
        u64::from(l.issue_policy.comment_bytes)
            .checked_add(u64::try_from(size_of::<Milestone>()).ok()?)?
            .checked_add(u64::try_from(size_of::<MilestoneKey>()).ok()?.checked_mul(2)?)?,
    )?;
    u64::from(l.issues).checked_mul(
        u64::from(l.issue_policy.title_bytes)
            .checked_add(u64::from(l.issue_policy.body_bytes))?
            .checked_add(u64::from(l.issue_policy.comment_bytes))?
            .checked_add(plan)?
            .checked_add(milestones)?,
    )
}

/// Volatile tables when the protocol owner supplies its own connection bounds.
#[must_use]
pub fn connection_worst_case(forges: u32, installations: u32, name_bytes: u32) -> Option<u64> {
    let count = forges.checked_mul(installations)?;
    Map::<u16, crate::First>::worst_case(forges)?
        .checked_add(Map::<u16, crate::AppWay>::worst_case(forges)?)?
        .checked_add(Map::<u16, (Box<[u8]>, Option<crate::AdminStop>)>::worst_case(forges)?)?
        .checked_add(u64::from(forges).checked_mul(u64::try_from(CONNECT_WHY_BYTES).ok()?)?)?
        .checked_add(
            u64::from(forges)
                .checked_mul(u64::from(name_bytes.max(FIRST_NAME_BYTES)).checked_mul(3)?.checked_add(64)?)?,
        )?
        .checked_add(Map::<u16, crate::domain::ConnectionState>::worst_case(forges)?)?
        .checked_add(u64::from(forges).checked_mul(
            u64::from(name_bytes).checked_add(u64::try_from(BOT_LOGIN_SUFFIX.len()).ok()?)?.max(STOPPED_TEXT_BYTES),
        )?)?
        .checked_add(Map::<(u16, u64), crate::Installation>::worst_case(count)?)?
        .checked_add(u64::from(count).checked_mul(u64::from(name_bytes))?)
}

/// One close per live task and the demo provider's deployment account.
#[must_use]
pub const fn demo_act_bound(l: &Limits) -> Option<u32> {
    l.tasks.checked_add(1)
}

// Each removal list is consumed before the next is allocated. The watch map
// then coexists with its replacement list through child Keep: two resource
// names in each map entry (key + watch), plus one in the list = 3 * name_bytes.
// Price the larger phase; the child's own removal scratch remains in its bound.
fn retirement_scratch(l: &Limits) -> Option<u64> {
    let tokens = skein_lib::List::<skein_lib::Token>::worst_case(l.holds)?
        .max(skein_lib::List::<skein_lib::Token>::worst_case(l.subscriptions)?)
        .max(skein_lib::List::<skein_lib::Token>::worst_case(l.changes)?);
    let watches = Map::<skein_forge_client::Resource, skein_forge_client::Watch>::worst_case(l.client.resources)?
        .checked_add(skein_lib::List::<skein_forge_client::Watch>::worst_case(l.client.resources)?)?
        .checked_add(u64::from(l.client.resources).checked_mul(u64::from(l.name_bytes))?.checked_mul(3)?)?;
    Some(tokens.max(watches))
}
