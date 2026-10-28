//! Child limits and checked route bounds (jig's domain/root.md, 10).
use crate::domain::Domain;
use crate::{JournalLimits, loads};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_core::{Model, RunCharter, RunPolicy};
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use skein_lib::Env;
use temper_engine_domain_forge as forge;

/// How many of the root's own facts wait to be drained.
pub const FACTS: u32 = 32;
/// A fresh run's transcript tail, in bytes, jig's small-profile figure; each
/// protocol-rendered turn shares this bound.
pub const TRANSCRIPT_TAIL_BYTES: u32 = 2048;

/// Root startup bounds, supplied by configuration and immutable at every step
/// (domain/root.md, 10). `valid` checks child limits, edges and payload
/// capacities.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Admission watermark, per-commit store edges and journal memory estimates.
    pub journal: JournalLimits,
    /// Paged store reads; every startup page must fit whole.
    pub loads: loads::Limits,
    /// Authentic task/funding room, also bounding root current-claim proof slots; transport
    /// evidence belongs to root.
    pub tasks: tasks::Limits,
    /// Deployment/project policy and finding room, supplied at startup.
    pub authority: authority::Limits,
    /// Identity, session and keyed request room.
    pub people: people::Limits,
    /// Worker attempts and unacknowledged turns.
    pub fleet: fleet::Limits,
    /// Maximum named decisions awaiting a turn or task end across live tasks.
    pub call_records: u32,
    /// Required task section gathering and cuts.
    pub brief: BriefLimits,
    /// Secret-free credential policy.
    pub accounts: accounts::Limits,
    /// Bounded notes indexes and pages kept by the core.
    pub notes: notes::Limits,
    /// Configured typed document construction bounds and served versions.
    pub documents: jig_domain_core::documents::DocumentLimits,
    /// Maximum configured templates accepted by the core.
    pub templates: u32,
    /// Configured policy byte budget, copied unchanged into the core.
    pub policy_bytes: u64,
    /// Configured settled-call byte budget, independent of journal capacity.
    pub call_answer_bytes: u32,
    /// Settings history and value bounds parsed by jig.
    pub settings: jig_domain_core::settings::Limits,
    /// Forge connector subtree and its bounded outbox.
    pub forge: forge::Limits,
}

/// Root brief configuration, including temper's source and forge budgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BriefLimits {
    pub briefs: u32,
    pub sections: u32,
    pub parts: u32,
    pub read_bytes: u32,
    pub budgets: BriefBudgets,
    pub brief_bytes: u32,
    pub gather: skein_lib::Duration,
}

/// Byte ceilings for the root's core sections and its forge connector.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BriefBudgets {
    pub task: u32,
    pub dependencies: u32,
    pub ci: u32,
    pub reviews: u32,
    pub pull: u32,
    pub attempts: u32,
    pub plan: u32,
    pub notes: u32,
}

pub(crate) fn environment_core(env: &Env<Limits>) -> Env<jig_domain_core::Limits> {
    Env { now: env.now, wall: env.wall, limits: core_limits(&env.limits) }
}

/// The store map's bytes at or above which new party work is refused, as
/// jig's `crates/jig-protocol-config/src/profile/small.rs` sets it until
/// temper's configuration names its own (domain/root.md, 10).
pub const FILL_MARK: u64 = 268_435_456;
/// Streams use temper's client's own budget of 16, above jig's small
/// profile's 4 (`small.rs`, clients; protocol/configuration.md, 7).
pub const WATCHES_PER_STREAM: u32 = 16;
/// Watches use the client's budget above jig's small profile's 4 (`small.rs`, views).
/// Live runs and provisional text retain jig's small profile's limits.
pub const VIEWS: views::Limits = views::Limits {
    runs: 4,
    watchers: 16,
    watchers_per_party: 16,
    subjects_per_change: 32,
    text_bytes: 65_536,
    facts: 32,
};

/// The effective core bounds shared by the root and its store.
#[must_use]
pub fn core_limits(limits: &Limits) -> jig_domain_core::Limits {
    jig_domain_core::Limits {
        fill_mark: FILL_MARK,
        load_bytes: limits.loads.read_bytes,
        documents: limits.documents,
        templates: limits.templates,
        settings: limits.settings,
        connectors: 1,
        resume_bytes: limits.journal.transcript_bytes,
        run_bytes: limits.journal.run_bytes,
        policy_bytes: limits.policy_bytes,
        escalation_reason_bytes: limits
            .tasks
            .result_bytes
            .min(limits.journal.result_bytes)
            .min(limits.journal.transcript_bytes),
        tasks: limits.tasks,
        authority: limits.authority,
        load_slots: limits.loads.loads,
        call_records: limits.call_records,
        call_answer_bytes: limits.call_answer_bytes,
        people: limits.people,
        fleet: limits.fleet,
        brief: brief_limits(&limits.brief),
        brief_parts: limits.brief.parts,
        brief_core_budgets: jig_domain_core::CoreBriefBudgets {
            tail: TRANSCRIPT_TAIL_BYTES,
            task: limits.brief.budgets.task,
            dependencies: limits.brief.budgets.dependencies,
            attempts: limits.brief.budgets.attempts,
            plan: limits.brief.budgets.plan,
            notes: limits.brief.budgets.notes,
        },
        accounts: limits.accounts,
        notes: limits.notes,
        views: VIEWS,
    }
}

/// One policy at its configured bound, including its largest owned row carrier.
/// `store::owned_bytes(Write::Save)` excludes the outer Record slot: project
/// policies add their name, while settings values add `ConfiguredPolicy` and a
/// single-item revision adds `Item` (`store/settings_bytes.rs`). Whole changes
/// still pass the existing request and per-write admission separately.
#[must_use]
pub fn policy_row_bound(limits: &Limits) -> Option<u64> {
    let project = limits.policy_bytes.checked_add(u64::try_from(people::NAME_BYTES).ok()?)?;
    let setting = limits
        .policy_bytes
        .checked_add(u64::try_from(size_of::<jig_domain_core::ConfiguredPolicy>()).ok()?)?
        .checked_add(u64::try_from(size_of::<jig_domain_core::settings::Item>()).ok()?)?;
    Some(project.max(setting))
}

pub(crate) fn brief_limits(limits: &BriefLimits) -> brief::Limits {
    brief::Limits {
        briefs: limits.briefs,
        sections: limits.sections,
        read_bytes: limits.read_bytes,
        brief_bytes: limits.brief_bytes,
    }
}

pub(crate) fn payload_slots(limits: &Limits) -> Option<u32> {
    limits
        .fleet
        .turns
        .checked_add(limits.fleet.attempts)?
        .checked_mul(2)?
        .checked_add(limits.fleet.calls.checked_mul(2)?)
}

// The fixtures admit one cohort from every child before acknowledging writes.
#[cfg(test)]
pub(crate) fn work_estimate(limits: &Limits) -> Option<u32> {
    let routes = tasks::max_out(&limits.tasks)
        .checked_mul(8)?
        .checked_add(people::max_out(&limits.people).checked_mul(4)?)?
        // Every in-flight historical decision may complete under journal pressure:
        // one retained IO terminal and one people Decided callback per flight.
        .checked_add(limits.people.pending.checked_mul(2)?)?
        .checked_add(fleet::max_out(&limits.fleet).checked_mul(4)?)?
        .checked_add(forge::max_out(&limits.forge))?
        // One serialized role cohort revisits each Waiting task, then answers.
        .checked_add(limits.tasks.tasks.checked_add(4)?)?;
    // Each due project saves its opened ledger and then its next start.
    let routes = routes.checked_add(limits.authority.projects.checked_mul(2)?)?;
    Some(routes.max(jig_domain_core::settings::writes_worst_case(&core_limits(limits))?))
}

/// The root's watermark: new external work waits while the backlog is at
/// or past it (skein's draft dynamic-memory.md, 4.4). The journal's commit
/// count is reused so no Limits literal changes and pacing stays close to
/// the reserved journal's unanswered commits.
pub(crate) fn watermark(limits: &Limits) -> u64 {
    u64::from(limits.journal.commits)
}

/// What the root holds of admitted work, in entries (the draft's 4.1):
/// the journal's backlog, the work queue and the queued store answers.
pub(crate) fn backlog(domain: &Domain) -> u64 {
    domain
        .journal
        .backlog()
        .checked_add(u64::from(domain.work.len()))
        .expect("four u32 queue counts fit in u64")
        .checked_add(u64::from(domain.store_answers.len()))
        .expect("five u32 queue counts fit in u64")
}

pub(crate) fn run_policy_bytes(policy: &RunPolicy) -> Option<u64> {
    if policy.turns == 0
        || policy.waiting == skein_lib::Duration::ZERO
        || policy.time == skein_lib::Duration::ZERO
        || policy.call_timeout == skein_lib::Duration::ZERO
        || policy.model.name.is_empty()
        || policy.model.window == 0
        || policy.model.output == 0
        || policy.model.price_unit == 0
    {
        return None;
    }
    let mut bytes = u64::try_from(size_of::<RunCharter>())
        .ok()?
        .checked_add(u64::try_from(policy.instructions.len()).ok()?)?
        .checked_add(u64::try_from(policy.model.name.len()).ok()?)?
        .checked_add(
            u64::try_from(policy.alternatives.len()).ok()?.checked_mul(u64::try_from(size_of::<Model>()).ok()?)?,
        )?;
    for model in &policy.alternatives {
        if model.name.is_empty() || model.window == 0 || model.output == 0 || model.price_unit == 0 {
            return None;
        }
        bytes = bytes.checked_add(u64::try_from(model.name.len()).ok()?)?;
    }
    Some(bytes)
}

pub(crate) fn run_carriers_bound(limits: &Limits) -> Option<u64> {
    u64::from(limits.tasks.contract_choices)
        .checked_mul(u64::try_from(size_of::<tasks::Verdict>()).ok()?)?
        .checked_add(
            u64::from(limits.tasks.authority_grants).checked_mul(u64::try_from(size_of::<tasks::Grant>()).ok()?)?,
        )?
        .checked_add(
            u64::from(limits.tasks.authority_grants)
                .checked_mul(u64::try_from(size_of::<tasks::ResourceScope>()).ok()?)?,
        )?
        .checked_add(
            u64::from(limits.tasks.authority_grants)
                .checked_mul(2)?
                .checked_mul(u64::from(limits.tasks.authority_segments))?
                .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
        )?
        .checked_add(u64::from(limits.tasks.authority_bytes))?
        .checked_add(
            u64::from(limits.tasks.executor_kinds)
                .checked_mul(u64::try_from(size_of::<tasks::AuthorityExecutor>()).ok()?)?,
        )
}

pub(crate) fn run_policy_bound(policy: &RunPolicy, limits: &Limits) -> Option<u64> {
    run_policy_bytes(policy)?.checked_add(run_carriers_bound(limits)?)?.checked_add(
        u64::from(tasks::MODELS).checked_mul(
            u64::try_from(size_of::<Option<u32>>())
                .ok()?
                .checked_add(u64::try_from(size_of::<jig_domain_authority::AccountBinding>()).ok()?)?,
        )?,
    )
}

pub(crate) fn run_charter_bytes(charter: &RunCharter) -> Option<u64> {
    let mut bytes = run_policy_bytes(&charter.policy)?
        .checked_add(
            u64::try_from(charter.sources.len()).ok()?.checked_mul(u64::try_from(size_of::<Option<u32>>()).ok()?)?,
        )?
        .checked_add(
            u64::try_from(charter.accounts.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<jig_domain_authority::AccountBinding>()).ok()?)?,
        )?;
    match &charter.contract {
        tasks::Contract::Report { .. } | tasks::Contract::Change { .. } => {}
        tasks::Contract::Approval { state, .. } => bytes = bytes.checked_add(u64::try_from(state.len()).ok()?)?,
        tasks::Contract::Verdict { choices } => {
            bytes = bytes.checked_add(
                u64::try_from(choices.len()).ok()?.checked_mul(u64::try_from(size_of::<tasks::Verdict>()).ok()?)?,
            )?;
        }
    }
    bytes = bytes
        .checked_add(
            u64::try_from(charter.authority.grants.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<tasks::Grant>()).ok()?)?,
        )?
        .checked_add(
            u64::try_from(charter.authority.note_resources.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<tasks::ResourceScope>()).ok()?)?,
        )?
        .checked_add(
            u64::try_from(charter.authority.delegation.kinds.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<tasks::AuthorityExecutor>()).ok()?)?,
        )?;
    for grant in &charter.authority.grants {
        bytes = bytes.checked_add(
            u64::try_from(grant.pattern.segments.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
        )?;
        for segment in &grant.pattern.segments {
            bytes = bytes.checked_add(u64::try_from(segment.len()).ok()?)?;
        }
        let terminal = match &grant.pattern.last {
            tasks::Last::Exact(bytes) | tasks::Last::Open(bytes) => bytes.len(),
        };
        bytes = bytes.checked_add(u64::try_from(terminal).ok()?)?;
    }
    for scope in &charter.authority.note_resources {
        bytes = bytes.checked_add(
            u64::try_from(scope.pattern.segments.len())
                .ok()?
                .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
        )?;
        for segment in &scope.pattern.segments {
            bytes = bytes.checked_add(u64::try_from(segment.len()).ok()?)?;
        }
        let terminal = match &scope.pattern.last {
            tasks::Last::Exact(bytes) | tasks::Last::Open(bytes) => bytes.len(),
        };
        bytes = bytes.checked_add(u64::try_from(terminal).ok()?)?;
    }
    Some(bytes)
}

pub(crate) fn row_bound(limits: &Limits) -> Option<u64> {
    let tasks = limits.tasks;
    let mut bytes = u64::try_from(size_of::<tasks::TaskRecord>()).ok()?;
    for retained in [
        u64::from(tasks.spec_bytes),
        u64::from(tasks::HOLD_ACCOUNT_BYTES),
        u64::from(tasks.result_bytes).checked_mul(3)?,
        u64::from(tasks.inbox_bytes),
        u64::from(tasks.inbox_messages).checked_mul(u64::try_from(size_of::<tasks::Word>()).ok()?)?,
        // Saved names own their path segments and bytes, including the forge/repository pair.
        u64::from(tasks.saved_resources).checked_mul(
            u64::try_from(size_of::<tasks::SavedResource>())
                .ok()?
                .checked_add(
                    u64::from(tasks.authority_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                )?
                .checked_add(u64::from(tasks.authority_bytes))?,
        )?,
        u64::from(tasks.parameters).checked_mul(u64::try_from(size_of::<tasks::Parameter>()).ok()?)?,
        u64::from(tasks.inputs)
            .checked_add(u64::from(tasks.dependencies).checked_mul(2)?)?
            .checked_add(u64::from(tasks.delegates))?
            .checked_mul(8)?,
        u64::from(tasks.contract_choices).checked_mul(u64::try_from(size_of::<tasks::Verdict>()).ok()?)?,
        u64::from(tasks.authority_grants).checked_mul(u64::try_from(size_of::<tasks::Grant>()).ok()?.checked_add(
            u64::from(tasks.authority_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
        )?)?,
        u64::from(tasks.authority_grants).checked_mul(
            u64::try_from(size_of::<tasks::ResourceScope>()).ok()?.checked_add(
                u64::from(tasks.authority_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
            )?,
        )?,
        u64::from(tasks.authority_bytes),
        u64::from(tasks.executor_kinds).checked_mul(u64::try_from(size_of::<tasks::AuthorityExecutor>()).ok()?)?,
    ] {
        bytes = bytes.checked_add(retained)?;
    }
    let people = limits.people;
    Some(
        bytes
            .max(u64::from(people.identity_bytes))
            .max(u64::from(people.words))
            // A retained change and its queued copies own both whole word fields.
            .max(
                u64::from(tasks.spec_bytes)
                    .checked_mul(2)?
                    .checked_add(u64::try_from(size_of::<forge::ChangeRow>()).ok()?)?
                    .checked_add(u64::from(forge::EFFECT_ACCOUNT_BYTES))?,
            )
            .max(u64::from(limits.forge.client.answer_bytes))
            .max(u64::from(people.holdings).checked_mul(u64::try_from(size_of::<people::Holding>()).ok()?)?),
    )
}

/// Whether the root's limits are representable and agree with its children's
/// (domain/root.md, 10): the rules the former sum held them to beyond the heap
/// it counted. Startup refuses limits it rejects.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    let Some(brief_fetches) = brief_fetches(limits) else {
        return false;
    };
    let Some(row) = row_bound(limits) else { return false };
    if limits.forge.tasks < limits.tasks.tasks
        || limits.brief.parts == 0
        || limits.brief.gather == skein_lib::Duration::ZERO
        || limits.forge.brief_sections < brief_fetches
        || limits.forge.brief_bytes < limits.brief.read_bytes
        || limits.forge.named_calls < limits.call_records
        || limits.call_records == 0
        || limits.loads.loads < 2
        || limits.loads.rows < 2
        || limits.journal.result_bytes < limits.brief.brief_bytes
        || limits.forge.client.op_bytes < limits.tasks.result_bytes
        || limits.journal.result_bytes < limits.tasks.result_bytes
        || limits.journal.result_bytes < limits.people.words
        || limits.people.inbox_entries == 0
        || u64::from(limits.journal.transcript_bytes) < row
    {
        return false;
    }
    // jig's valid() replaces this at the set's repin
    jig_domain_core::worst_case(&core_limits(limits)).is_some()
        && forge::worst_case(&limits.forge).is_some()
        && loads::worst_case(&limits.loads).is_some()
        && crate::journal_worst_case(&limits.journal).is_some()
        && jig_domain_core::settings::revision_worst_case(&core_limits(limits)).is_some()
        && jig_domain_core::reads::answer_worst_case(&core_limits(limits)).is_some()
        && payload_slots(limits).is_some()
        && limits.call_records.checked_add(limits.tasks.tasks).is_some()
        && inbox_slots(limits).is_some()
}

fn brief_fetches(limits: &Limits) -> Option<u32> {
    limits.brief.briefs.checked_mul(limits.brief.sections)?.checked_mul(2)
}
fn inbox_slots(limits: &Limits) -> Option<u32> {
    limits.tasks.tasks.checked_mul(2)?.checked_add(limits.tasks.inbox_messages)
}
