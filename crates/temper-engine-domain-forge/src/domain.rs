//! Top-level forge routing and commit fence (jig's domain/connectors.md,
//! sections 2–5). Durable rows are held here and mirrored by root store
//! writes.
use alloc::boxed::Box;
use skein_lib::{Duration, Env, List, Map, Queue, Token};
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_issues as issues;

use crate::{
    BranchHead, ChangeRow, CiState, Class, Event, Hold, IssueRow, Key, Kinds, Limits, Name, News, PullState,
    ReleaseEnding, ReleaseRow, Repository, Request, RestartStage, Role, Stored, Subscriber, Topic, What, Writer,
};
use crate::{adoption, brief, held, topics};

mod hints;

/// One comparison in flight for a branch's observed tip move.
#[derive(Debug)]
pub(crate) struct PendingLanding {
    topic: Topic,
    before: skein_forge::api::Commit,
    after: skein_forge::api::Commit,
}
#[derive(Debug)]
pub(crate) struct PendingLost {
    task: u64,
    attempt: u64,
    name: Name,
    comparison: Option<(skein_forge::api::Commit, skein_forge::api::Commit)>,
    state: LostReadState,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LostReadState {
    Asked,
    WaitingForRoom,
    WaitingForConnection,
}
/// One fresh verdict read for a commit with live CI subscribers.
#[derive(Debug)]
pub(crate) struct PendingCi {
    repository: skein_forge::api::Repository,
    head: skein_forge::api::Commit,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StepStage {
    Branch,
    Base,
    Pull,
    Ci,
    BaseCi,
    Compare,
    Reviews(u32),
}

/// One sequence of fresh reads before a level-triggered change step.
#[expect(clippy::struct_excessive_bools, reason = "independent facts and progress of one fresh-read chain")]
#[derive(Debug)]
pub(crate) struct PendingStep {
    task: u64,
    entry: u64,
    stage: StepStage,
    heard: change::Heard,
    gates: Box<[change::GateReport]>,
    queue_repair_active: bool,
    kept: bool,
    branch: Option<skein_forge::api::Commit>,
    base_tip: Option<skein_forge::api::Commit>,
    pull: Option<skein_forge::api::Pull>,
    ci: change::Status,
    base_ci: change::Status,
    contains_base: change::Status,
    reviews: Box<[skein_forge::api::Review]>,
    reviews_complete: bool,
    woken: bool,
}
struct StepInput {
    task: u64,
    entry: u64,
    heard: change::Heard,
    gates: Box<[change::GateReport]>,
    queue_repair_active: bool,
    kept: bool,
}

/// One committed outbox entry. A held one is out of the client's outbox and
/// keeps its room until a person makes it again or it is settled not made
/// (domain/forge.md, 6; jig's `domain/connectors.md`, 4.3).
#[derive(Debug)]
pub(crate) struct Outboxed {
    pub(crate) entry: skein_forge_client::Entry,
    held: bool,
    handoff: Handoff,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handoff {
    Uncommitted,
    /// Restored entries enter the client only at `SettleOutbox`.
    Restored,
    Ready,
    Handed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RestorePhase {
    Cold,
    Restoring,
    Active,
}

/// Volatile connection and the last failure's step.
pub(crate) type ConnectionState = (crate::Connection, Option<crate::FailedStep>);

/// The bounded forge connector subtree.
#[derive(Debug)]
pub struct Domain {
    pub(crate) configured: Map<u16, crate::Configured>,
    pub(crate) under_box: bool,
    pub(crate) demo: Option<crate::DemoConfig>,
    bindings: Map<Box<[u8]>, Box<[u8]>>,
    pub(crate) demo_state: Option<crate::DemoState>,
    pub(crate) demo_acts: Map<[u8; 16], crate::DemoActKind>,
    pub(crate) demo_pending: Map<Token, Token>,
    pub(crate) demo_answers: Map<Token, crate::StartAnswer>,
    pub(crate) connections: Map<u16, ConnectionState>,
    pub(crate) first: Map<u16, crate::First>,
    pub(crate) ended: Map<u16, (Box<[u8]>, Option<crate::AdminStop>)>,
    pub(crate) apps: Map<u16, crate::AppWay>,
    setup_answers: Map<Token, (skein_lib::Wall, crate::SetUpAnswer)>,
    setup_pending: Map<Token, (Token, Duration)>,
    pub(crate) connect: crate::connect::State,
    pub(crate) installations: Map<(u16, u64), crate::Installation>,
    pub(crate) repositories: Map<skein_forge::api::Repository, Repository>,
    names: Map<u64, Box<[Name]>>,
    holds: Map<Name, Hold>,
    pub(crate) subscriptions: Map<(u64, Topic), Subscriber>,
    pub(crate) goal_files: Map<u64, topics::Files>,
    pub(crate) goal_file_reads: Map<Token, topics::Pending>,
    pub(crate) heads: Map<Name, BranchHead>,
    pulls: Map<Name, PullState>,
    /// Each pull's last mergeable, unsaved: a restart reads it afresh.
    pull_mergeable: Map<Name, bool>,
    ci: Map<(skein_forge::api::Repository, skein_forge::api::Commit), CiState>,
    pub(crate) entries: Map<u64, Outboxed>,
    pub(crate) flights: Map<(u64, u64), ()>,
    named_calls: Map<crate::NamedCall, crate::CallPayload>,
    proposed_effects: Map<u64, crate::ProposedEffect>,
    pub(crate) landed: Map<skein_forge::api::Commit, u64>,
    landings: Map<Token, PendingLanding>,
    lost: Map<Token, PendingLost>,
    pending_ci: Map<Token, PendingCi>,
    restart_fresh: Option<bool>,
    pub(crate) sequence: u64,
    pub(crate) changes: Map<u64, ChangeRow>,
    steps: Map<Token, PendingStep>,
    pub(crate) panel_ci: Map<u64, change::Ci>,
    pub(crate) issues: Map<u64, IssueRow>,
    releases: Map<u64, ReleaseRow>,
    namespace: Box<[u8]>,
    pub(crate) adoptions: Map<Token, adoption::PendingAdoption>,
    pub(crate) application: adoption::State,
    pub(crate) writers: Map<u16, u64>,
    /// Each forge's git identity, the application credential its runs push
    /// and fetch with (protocol/credentials.md).
    pub(crate) identities: Map<u16, u32>,
    pub(crate) capabilities: Map<Token, crate::capabilities::Pending>,
    pub(crate) stopping: bool,
    pub(crate) landing: crate::landing::State,
    /// Restored timers stay passive until the scripted fresh-read stage (domain/root.md, 9.1).
    restore_phase: RestorePhase,
    pub(crate) reconciliation: crate::reconcile::Reconciliation,
    restored_demo: Option<crate::Demo>,
    client: skein_forge_client::Domain,
    client_out: Queue<skein_forge_client::Request>,
    pub(crate) brief_fetches: Map<Token, brief::BriefFetch>,
    brief_planned: Map<Token, crate::BriefSource>,
    pub(crate) brief_pending: Map<Token, held::Pending>,
    pub(crate) brief_held: Map<Token, held::Held>,
    judges: crate::Judges,
    gate_policy: crate::GatePolicy,
    judge_criteria: u32,
    projection_due: Map<u64, skein_lib::Wall>,
    change_due: Map<u64, skein_lib::Wall>,
    procedure_wakes: Map<u64, bool>,
    pub(crate) settling: Map<u64, crate::settle::Settle>,
}

impl Domain {
    /// Before the first input, size the volatile routing tables by the connection component's bounds.
    /// The caller charges `connection_worst_case` beside the ordinary domain bound.
    pub fn connection_limits(&mut self, forges: u32, installations: u32) {
        assert!(self.connections.is_empty() && self.installations.is_empty(), "connection limits before inputs");
        self.connections = Map::with_capacity(forges);
        self.first = Map::with_capacity(forges);
        self.ended = Map::with_capacity(forges);
        self.apps = Map::with_capacity(forges);
        self.installations = Map::with_capacity(forges.checked_mul(installations).expect("connection bounds"));
    }
    /// Public startup configuration; never restored from records.
    #[must_use]
    pub fn configured(&self, forge: u16) -> Option<&crate::Configured> {
        self.configured.get(&forge)
    }
    #[must_use]
    pub fn connection(&self, forge: u16) -> crate::Connection {
        if self.forge_ended(forge) {
            return crate::Connection::NotConnected;
        }
        match self.connections.get(&forge) {
            Some((state, _)) => state.clone(),
            None => crate::Connection::NotConnected,
        }
    }
    /// Create's service tell; a forge without a setup intent ignores it.
    pub fn first_made(&mut self, forge: u16, id: u64) -> bool {
        if let Some(first) = self.first.get_mut(&forge) {
            first.state = crate::FirstState::Made { id };
            return true;
        }
        false
    }
    /// Create's stopped step is volatile, like its setup intent.
    pub fn first_failed(&mut self, forge: u16, step: crate::CreateStep) -> bool {
        if let Some(first) = self.first.get_mut(&forge) {
            first.state = crate::FirstState::Failed { step };
            return true;
        }
        false
    }
    #[must_use]
    pub fn forges(&self) -> crate::Forges {
        let mut rows = List::with_capacity(self.configured.len());
        for (forge, config) in &self.configured {
            if self.forge_ended(*forge) {
                continue;
            }
            let mut installations = List::with_capacity(self.installations.len());
            if config.kind == crate::Kind::GitHub {
                for ((owner, id), installation) in &self.installations {
                    if owner == forge {
                        installations.push((*id, installation.clone())).expect("bounded installations");
                    }
                }
            }
            let mut repositories = List::with_capacity(self.repositories.len());
            for (repository, adopted) in &self.repositories {
                if repository.forge == *forge {
                    repositories
                        .push(crate::AdoptedRow {
                            owner: adopted.owner.clone(),
                            name: adopted.name.clone(),
                            project: adopted.project,
                        })
                        .expect("bounded adopted repositories");
                }
            }
            let (ended, ended_stop) = match self.ended.get(forge) {
                Some((why, stop)) => (Some(why.clone()), *stop),
                None => (None, None),
            };
            rows.push(crate::Row {
                read_only: config.kind == crate::Kind::GitHub && config.read_only,
                number: *forge,
                kind: config.kind,
                mode: config.mode,
                connection: self.connection(*forge),
                failed: match self.connections.get(forge) {
                    Some((_, failed)) => *failed,
                    None => None,
                },
                offer: self.offer(*forge),
                first: self.first.get(forge).cloned(),
                installations: installations.into_boxed(),
                ended,
                app: match self.connection(*forge) {
                    crate::Connection::Connected { .. } if config.kind == crate::Kind::GitHub => {
                        self.apps.get(forge).copied()
                    }
                    crate::Connection::Connected { .. }
                    | crate::Connection::NotConnected
                    | crate::Connection::NotSetUp
                    | crate::Connection::SettingUp { .. }
                    | crate::Connection::SetUpFailed { .. }
                    | crate::Connection::Connecting
                    | crate::Connection::Stopped { .. } => None,
                },
                web: config.web.clone(),
                repositories: repositories.into_boxed(),
                owned: config.owned.clone(),
                admin: config.admin_connect,
                ended_stop,
            })
            .expect("configured forges");
        }
        crate::Forges { rows: rows.into_boxed() }
    }
    /// One decision shared by the document and setup admission (domain/forge.md, 4.1).
    #[must_use]
    pub fn offer(&self, forge: u16) -> crate::Offer {
        let Some(config) = self.configured.get(&forge) else { return crate::Offer::NotOffered };
        if self.forge_ended(forge) || config.mode != crate::Mode::Owned || !config.asked {
            return crate::Offer::NotOffered;
        }
        match self.connection(forge) {
            crate::Connection::NotSetUp
            | crate::Connection::SetUpFailed { .. }
            | crate::Connection::SettingUp { .. } => {
                if self.under_box {
                    crate::Offer::Offered
                } else {
                    crate::Offer::NotUnderTheBox
                }
            }
            crate::Connection::NotConnected
            | crate::Connection::Connecting
            | crate::Connection::Connected { .. }
            | crate::Connection::Stopped { .. } => crate::Offer::NotOffered,
        }
    }
    fn setup_answer(
        &mut self,
        env: &Env<Limits>,
        request: Token,
        answer: crate::SetUpAnswer,
        out: &mut Queue<Request>,
    ) {
        if let Some((token, retention)) = self.setup_pending.remove(&request) {
            let until = skein_lib::Wall::from_nanos(env.wall.as_nanos().saturating_add(retention.as_nanos()));
            self.setup_answers.insert(token, (until, answer)).expect("reserved setup answer");
            out.push(Request::ApplicationDone { request, token });
        }
    }
    /// Volatile setup answer or retained adoption outcome for this request.
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code uses explicit matches (programming-model.md, 10)")]
    pub fn application_answer(&self, token: Token) -> Option<crate::Answered> {
        if let Some(answer) = self.connect.answers.get(&token) {
            return Some(answer.clone());
        }
        if let Some(answer) = self.demo_answers.get(&token) {
            return Some(crate::Answered::StartDemo(answer.clone()));
        }
        if let Some((_, answer)) = self.setup_answers.get(&token) {
            return Some(crate::Answered::SetUp(*answer));
        }
        match self.application_outcome(token) {
            Some(outcome) => Some(crate::Answered::Adopt(outcome.clone())),
            None => None,
        }
    }
    #[must_use]
    pub fn installation(&self, forge: u16, installation: u64) -> Option<&crate::Installation> {
        self.installations.get(&(forge, installation))
    }

    pub fn gate_policy(&mut self, policy: crate::GatePolicy, limits: &Limits) -> bool {
        if !crate::gates::within(&policy, limits) {
            return false;
        }
        self.gate_policy = policy;
        true
    }
    #[must_use]
    pub fn gate_candidates(
        &self,
        limits: &Limits,
        project: u32,
        name: &[Box<[u8]>],
    ) -> Option<Box<[crate::GateCandidate]>> {
        crate::gates::candidates(&self.gate_policy, limits, project, name)
    }
    #[must_use]
    pub fn configured_gates(
        &self,
        limits: &Limits,
        repository: &Repository,
        candidates: &[crate::GateCandidate],
        required: &[bool],
    ) -> Option<Box<[change::Gate]>> {
        crate::gates::selected(limits, repository, candidates, required)
    }

    /// Read the connector's retained part for the exact supplied call identity.
    #[must_use]
    pub fn named_answer(&self, key: crate::NamedCall) -> Option<crate::CallPayload> {
        self.named_calls.get(&key).cloned()
    }

    /// Retain a bounded payload after the caller admits the named decision.
    pub fn keep_named_answer(
        &mut self,
        limits: &Limits,
        key: crate::NamedCall,
        answer: crate::CallPayload,
    ) -> Result<Option<crate::CallPayload>, crate::CallPayload> {
        if key.task == 0 || key.attempt == 0 {
            return Err(answer);
        }
        let within = match &answer {
            crate::CallPayload::Read(result) => match &**result {
                Ok(value) => match skein_forge::measure::answer(value) {
                    Some(bytes) => {
                        skein_forge_client::measure::answer_rows(value, limits.client.rows)
                            && bytes <= u64::from(limits.client.answer_bytes)
                    }
                    None => false,
                },
                Err(_) => true,
            },
            crate::CallPayload::Effect { .. } | crate::CallPayload::Refused(_) => true,
        };
        if !within {
            return Err(answer);
        }
        match self.named_calls.insert(key, answer) {
            Ok(previous) => Ok(previous),
            Err((_, answer)) => Err(answer),
        }
    }

    /// Drop the payload when its owner retires the named call.
    pub fn forget_named_answer(&mut self, key: crate::NamedCall) -> Option<crate::CallPayload> {
        self.named_calls.remove(&key)
    }

    /// Identify the named effect whose connector outcome changed.
    #[must_use]
    pub fn named_effect(&self, entry: u64) -> Option<crate::NamedCall> {
        for (key, answer) in &self.named_calls {
            match answer {
                crate::CallPayload::Effect { entry: number, .. } if *number == entry => return Some(*key),
                crate::CallPayload::Effect { .. } | crate::CallPayload::Read(_) | crate::CallPayload::Refused(_) => {}
            }
        }
        None
    }

    /// Update the connector's own part when its client reports an effect outcome.
    pub fn named_outcome(&mut self, entry: u64, outcome: skein_forge_client::Outcome) {
        if outcome == skein_forge_client::Outcome::Uncertain || outcome == skein_forge_client::Outcome::Queued {
            return;
        }
        let Some(key) = self.named_effect(entry) else { return };
        let Some(answer) = self.named_calls.get_mut(&key) else { unreachable!("named effect owner") };
        match answer {
            crate::CallPayload::Effect { outcome: kept, .. } => *kept = Some(outcome),
            crate::CallPayload::Read(_) | crate::CallPayload::Refused(_) => unreachable!("effect payload"),
        }
    }

    /// Keep the connector's retry deadline; the root carries no timer policy.
    pub fn wait_change(&mut self, env: &Env<Limits>, task: u64, until: Option<skein_lib::Wall>) {
        let until = match until {
            Some(until) => until,
            None => skein_lib::Wall::from_nanos(env.wall.as_nanos().saturating_add(env.limits.queue_window.as_nanos())),
        };
        self.change_due.insert(task, until).expect("one bounded change deadline");
    }

    /// A new offered procedure step replaces its previous waiting deadline.
    pub fn offered_change(&mut self, task: u64) {
        self.change_due.remove(&task);
    }

    /// The issue procedure owns the interval reported by its policy child.
    pub fn projection_after(&mut self, goal: u64, when: skein_lib::Wall) {
        self.projection_due.insert(goal, when).expect("one bounded projection deadline");
    }

    /// The core has finished the closing projection handshake.
    pub fn projection_settled(&mut self, goal: u64) {
        self.projection_due.remove(&goal);
    }

    /// Available outbox room including the root's staged descriptions.
    #[must_use]
    pub fn effect_room(&self, limits: &Limits, staged: u32) -> bool {
        self.entries.len().saturating_add(staged) < limits.entries
    }

    /// A connector's durable payload, read again before a holder accepts it.
    #[must_use]
    pub fn proposed_effect(&self, number: u64) -> Option<&crate::ProposedEffect> {
        self.proposed_effects.get(&number)
    }

    /// Bind agent and projection keys to the durable deployment ID before
    /// connector records are restored.
    pub fn bind_deployment(&mut self, id: [u8; 16], limits: &Limits) -> bool {
        if !self.client.bind_deployment(id, &limits.client) {
            return false;
        }
        self.namespace = Box::from(id);
        true
    }
    /// Construct a connector with the deployment namespace and any writers
    /// known at start; invalid limits or writer configuration are refused.
    pub fn new(
        l: &Limits,
        seed: u64,
        config: skein_forge_client::Config,
        identities: &[crate::GitIdentity],
        forges: &[crate::Configured],
        under_box: bool,
    ) -> Result<Domain, skein_forge::api::Error> {
        if crate::worst_case(l).is_none()
            || (crate::purpose::EffectPurpose::Release { task: 0, resource: Box::from(b"reason".as_slice()) })
                .key(&config.namespace, l.client.op_bytes)
                .is_none()
        {
            return Err(skein_forge::api::Error::TooLarge);
        }
        let mut configured = Map::with_capacity(l.client.repositories);
        for forge in forges {
            if forge.git.len() > usize::try_from(crate::PULL_ADDRESS_BYTES).expect("u32 fits usize") {
                return Err(skein_forge::api::Error::TooLarge);
            }
            if configured.insert(forge.number, forge.clone()).is_err() {
                return Err(skein_forge::api::Error::TooLarge);
            }
        }
        let mut credentials = Map::with_capacity(l.repositories);
        for identity in identities {
            if credentials.insert(identity.forge, identity.credential).is_err() {
                return Err(skein_forge::api::Error::TooLarge);
            }
        }
        let mut writers = Map::with_capacity(l.client.repositories);
        for writer in &config.writers {
            if writers.insert(writer.forge, writer.author).is_err() {
                return Err(skein_forge::api::Error::TooLarge);
            }
        }
        Ok(Domain {
            configured,
            under_box,
            demo: None,
            demo_state: None,
            bindings: Map::with_capacity(l.bindings),
            demo_acts: Map::with_capacity(crate::demo_act_bound(l).expect("validated act bound")),
            demo_pending: Map::with_capacity(l.applications),
            demo_answers: Map::with_capacity(l.applications),
            connections: Map::with_capacity(l.client.repositories),
            first: Map::with_capacity(l.client.repositories),
            ended: Map::with_capacity(l.client.repositories),
            apps: Map::with_capacity(l.client.repositories),
            setup_answers: Map::with_capacity(l.applications),
            setup_pending: Map::with_capacity(l.applications),
            connect: crate::connect::State::new(l.applications),
            installations: Map::with_capacity(l.client.rows),
            repositories: Map::with_capacity(l.repositories),
            names: Map::with_capacity(l.tasks),
            holds: Map::with_capacity(l.holds),
            subscriptions: Map::with_capacity(l.subscriptions),
            goal_files: Map::with_capacity(l.changes),
            goal_file_reads: Map::with_capacity(l.changes),
            heads: Map::with_capacity(l.client.resources),
            pulls: Map::with_capacity(l.client.resources),
            pull_mergeable: Map::with_capacity(l.client.resources),
            ci: Map::with_capacity(l.subscriptions),
            entries: Map::with_capacity(l.entries),
            flights: Map::with_capacity(l.entries),
            named_calls: Map::with_capacity(l.named_calls),
            proposed_effects: Map::with_capacity(l.tasks),
            landed: Map::with_capacity(l.landings.max(l.changes)),
            landings: Map::with_capacity(l.landings.max(l.client.resources)),
            lost: Map::with_capacity(l.holds),
            pending_ci: Map::with_capacity(l.subscriptions),
            restart_fresh: None,
            sequence: 0,
            changes: Map::with_capacity(l.changes),
            steps: Map::with_capacity(l.changes),
            panel_ci: Map::with_capacity(l.changes),
            issues: Map::with_capacity(l.issues),
            releases: Map::with_capacity(l.tasks),
            namespace: config.namespace.clone(),
            adoptions: Map::with_capacity(l.adoptions),
            application: adoption::State::new(l),
            capabilities: Map::with_capacity(l.repositories),
            writers,
            identities: credentials,
            stopping: false,
            landing: crate::landing::State::new(l),
            restore_phase: RestorePhase::Cold,
            reconciliation: crate::reconcile::Reconciliation::new(l),
            restored_demo: None,
            client: skein_forge_client::Domain::configured(&l.client, seed, config)?,
            client_out: Queue::with_capacity(skein_forge_client::max_out(&l.client)),
            brief_fetches: Map::with_capacity(l.brief_sections),
            brief_planned: Map::with_capacity(l.brief_sections),
            brief_pending: Map::with_capacity(l.brief_sections),
            brief_held: Map::with_capacity(l.brief_sections),
            judges: crate::Judges::empty(l.judge_projects),
            gate_policy: crate::GatePolicy::empty(l.judge_projects),
            judge_criteria: l.judge_criteria,
            projection_due: Map::with_capacity(l.issues),
            change_due: Map::with_capacity(l.changes),
            procedure_wakes: Map::with_capacity(l.changes),
            settling: Map::with_capacity(l.changes),
        })
    }

    /// Install deployment criteria before decisions begin.
    pub fn deployment_judges(&mut self, criteria: Box<[crate::Criterion]>) -> bool {
        if !self.judge_count_fits(criteria.len()) {
            return false;
        }
        self.judges.deployment = criteria;
        true
    }

    /// Replace one project's judge parameters with its committed policy.
    pub fn project_judges(&mut self, project: u32, criteria: Box<[crate::Criterion]>) -> bool {
        self.judge_count_fits(criteria.len()) && self.judges.projects.insert(project, criteria).is_ok()
    }

    fn judge_count_fits(&self, count: usize) -> bool {
        match u32::try_from(count) {
            Ok(count) => count <= self.judge_criteria,
            Err(_) => false,
        }
    }

    /// Whether the landing effect's exact-head condition guards this judge.
    #[must_use]
    #[expect(clippy::match_like_matches_macro, reason = "the foundation forbids matches in domain steps")]
    pub fn guards_landing(&self, project: u32, requirement: u16, parameters: u32) -> bool {
        match (requirement, crate::judge::criterion(&self.judges, project, parameters)) {
            (1, Some(crate::Criterion::Ci))
            | (3, Some(crate::Criterion::Gate { .. }))
            | (4, Some(crate::Criterion::Approval { .. })) => true,
            _ => false,
        }
    }

    /// Judge one forge requirement for the exact head of a change effect.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "a judgement names its project, criterion and coherent change evidence"
    )]
    pub fn judge_landing(
        &self,
        project: u32,
        requirement: u16,
        parameters: u32,
        task: u64,
        head: skein_forge::api::Commit,
        evidence: &crate::ChangeEvidence,
        reviewers: &[crate::Reviewer],
    ) -> Option<crate::JudgeVerdict> {
        let criterion = crate::judge::criterion(&self.judges, project, parameters)?;
        let matching = match criterion {
            crate::Criterion::Ci => requirement == 1,
            crate::Criterion::UpToDate => requirement == 2,
            crate::Criterion::Gate { .. } => requirement == 3,
            crate::Criterion::Approval { .. } => requirement == 4,
        };
        if !matching {
            return None;
        }
        if let Some(row) = self.landing.rows.get(&task) {
            return crate::landing::judge(self, row, criterion, head, evidence, reviewers);
        }
        let row = self.changes.get(&task)?;
        let repository = self.repositories.get(&row.repository)?;
        Some(crate::judge::judge(criterion, repository, head, &row.change.clean, evidence, reviewers))
    }

    /// The next child timer, if one is armed.
    #[must_use]
    pub fn next_deadline(&self) -> Option<skein_lib::Time> {
        if self.restore_phase == RestorePhase::Restoring {
            return None;
        }
        self.client.next_deadline()
    }

    /// Earliest procedure or projection wait, reported to the root's clock.
    #[must_use]
    pub fn next_wall_deadline(&self) -> Option<skein_lib::Wall> {
        if self.stopping || self.restore_phase == RestorePhase::Restoring {
            return None;
        }
        let mut next: Option<skein_lib::Wall> = None;
        for deadlines in [&self.projection_due, &self.change_due, &self.landing.due] {
            for (_, deadline) in deadlines {
                next = Some(match next {
                    Some(previous) => previous.min(*deadline),
                    None => *deadline,
                });
            }
        }
        next
    }

    /// Whether a child call or a newly committed entry may be sent.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        if self.restore_phase == RestorePhase::Restoring {
            return false;
        }
        if self.stopping {
            return self.client.is_ready();
        }
        self.next_handoff().is_some()
            || !self.procedure_wakes.is_empty()
            || self.client.is_ready()
            || (self.lost_waiting() && self.client.has_room())
    }

    fn next_handoff(&self) -> Option<u64> {
        for (number, row) in &self.entries {
            if row.handoff == Handoff::Handed {
                continue;
            }
            if row.handoff != Handoff::Ready {
                return None;
            }
            for (_, pending) in &self.steps {
                if pending.entry < *number {
                    return None;
                }
            }
            return Some(*number);
        }
        None
    }

    /// Whether a lost writer's read waits to be asked again.
    fn lost_waiting(&self) -> bool {
        let mut waiting = false;
        for (_, pending) in &self.lost {
            waiting |= pending.state == LostReadState::WaitingForRoom;
        }
        waiting
    }

    /// Whether no brief section is being gathered by the connector.
    #[must_use]
    pub fn briefs_idle(&self) -> bool {
        self.brief_fetches.is_empty()
            && self.brief_planned.is_empty()
            && self.brief_pending.is_empty()
            && self.brief_held.is_empty()
    }

    /// Snapshot the provider ids restored for one forge, within the client's capacity.
    #[must_use]
    pub fn adopted_ids(&self, forge: u16) -> Box<[u64]> {
        let mut ids = List::with_capacity(self.repositories.len());
        for (repository, _) in &self.repositories {
            if repository.forge == forge && !self.forge_ended(forge) {
                ids.push(u64::from(repository.repository)).expect("bounded adopted repositories");
            }
        }
        ids.into_boxed()
    }

    /// Transfer one completed section to the parent assembling an assignment.
    pub fn take_brief(&mut self, section: Token) -> Option<Box<[u8]>> {
        match self.brief_held.remove(&section) {
            Some(held) => Some(held.words),
            None => None,
        }
    }

    /// Reclaim transient child buffers after the current decision.
    pub fn reclaim(&mut self) {
        self.client.reclaim();
    }

    /// Check one typed task interest before the root adds its matching durable task row.
    #[must_use]
    pub fn can_subscribe(&self, limits: &Limits, subscription: &Subscriber) -> bool {
        if match &subscription.goal_tasks {
            Some(tasks) => tasks.len() > usize::try_from(limits.issue_policy.plan_items).expect("u32 fits usize"),
            None => false,
        } || subscription.number == 0
            || subscription.paths.len() > usize::try_from(limits.paths_per_subscription).expect("u32 fits usize")
        {
            return false;
        }
        match self.subscriptions.get(&(subscription.task, subscription.topic.clone())) {
            Some(existing) if existing.number != subscription.number => return false,
            None if self.subscriptions.len() == limits.subscriptions => return false,
            Some(_) | None => {}
        }
        for path in &subscription.paths {
            if path.len() > usize::try_from(limits.name_bytes).expect("u32 fits usize") {
                return false;
            }
        }
        let repository = match &subscription.topic {
            Topic::Landings { repository, branch } => {
                if branch.len() > usize::try_from(limits.name_bytes).expect("u32 fits usize") {
                    return false;
                }
                *repository
            }
            Topic::Ci { repository, .. } | Topic::Pull { repository, .. } | Topic::Participation { repository, .. } => {
                *repository
            }
        };
        self.repositories.contains_key(&repository)
    }

    /// Current subscriber for an automatic goal topic, preserving its durable number.
    #[must_use]
    pub fn topic_subscriber(&self, task: u64, topic: &Topic) -> Option<&Subscriber> {
        self.subscriptions.get(&(task, topic.clone()))
    }

    /// Find the connector half of one durable task subscription.
    #[must_use]
    pub fn subscription(&self, task: u64, number: u64) -> Option<Topic> {
        for (_, row) in &self.subscriptions {
            if row.task == task && row.number == number {
                return Some(row.topic.clone());
            }
        }
        None
    }

    /// Current live resource names of one task, for root-side name additions.
    #[must_use]
    pub fn names(&self, task: u64) -> Option<&[Name]> {
        match self.names.get(&task) {
            Some(names) => Some(names),
            None => None,
        }
    }

    /// Resolve worker head reports against this attempt's durable forge writers.
    /// Prepared assignments leave the root when delivered; their temporary
    /// lifetime must not determine whether a finished writer can be released.
    #[must_use]
    pub fn reported_heads(
        &self,
        limits: &Limits,
        task: u64,
        attempt: u64,
        pushed: &[crate::Pushed],
    ) -> Box<[(Name, skein_forge::api::Commit)]> {
        let mut names = List::with_capacity(limits.resources_per_task);
        for (name, hold) in &self.holds {
            if hold.writer != Some(Writer::Run { task, attempt }) {
                continue;
            }
            for reported in pushed {
                if untag(reported.tag) == Some((name.forge, name.repository)) {
                    names.push((name.clone(), reported.commit)).expect("claimed writer destinations are bounded");
                    break;
                }
            }
        }
        names.into_boxed()
    }

    /// Current holder and writer of one named resource for root claim checks.
    #[must_use]
    pub fn hold(&self, name: &Name) -> Option<&Hold> {
        self.holds.get(name)
    }

    /// The last durable head observed for a named branch.
    #[must_use]
    #[expect(clippy::manual_map, reason = "the strict subset uses a match without a closure")]
    pub fn branch_head(&self, name: &Name) -> Option<skein_forge::api::Commit> {
        match self.heads.get(name) {
            Some(row) => Some(row.commit),
            None => None,
        }
    }

    /// Adopted repository facts used by root authority and workspace translation.
    #[must_use]
    pub fn repository(&self, provider: skein_forge::api::Repository) -> Option<&Repository> {
        self.repositories.get(&provider)
    }

    /// Resolve a deployment-wide saved repository tag within its project.
    #[must_use]
    pub fn repository_tag(&self, project: u32, tag: u64) -> Option<&Repository> {
        let (forge, repository) = untag(tag)?;
        let adopted = self.repository(skein_forge::api::Repository { forge, repository })?;
        if adopted.project == project { Some(adopted) } else { None }
    }

    /// All adopted repositories of this project on forges that have not ended.
    #[must_use]
    pub fn readable_repositories(&self, project: u32) -> Box<[&Repository]> {
        let mut repositories = List::with_capacity(self.repositories.capacity());
        for (_, repository) in &self.repositories {
            if repository.project == project && !self.forge_ended(repository.provider.forge) {
                repositories.push(repository).expect("bounded adopted repositories");
            }
        }
        repositories.into_boxed()
    }

    /// Adopted repositories in this project that still permit a workspace push.
    #[must_use]
    pub fn writable_repositories(&self, project: u32) -> Box<[&Repository]> {
        let mut repositories = List::with_capacity(self.repositories.capacity());
        for (_, repository) in &self.repositories {
            if repository.project == project
                && repository.role != Role::Context
                && repository.kinds.push
                && !self.forge_ended(repository.provider.forge)
            {
                repositories.push(repository).expect("bounded adopted repositories");
            }
        }
        repositories.into_boxed()
    }

    /// This project's explicitly selected home repository, when adopted.
    #[must_use]
    #[expect(clippy::manual_find, reason = "the strict subset uses explicit iteration instead of iterator closures")]
    pub fn home(&self, project: u32) -> Option<&Repository> {
        for (_, repository) in &self.repositories {
            if repository.project == project && repository.home && !self.forge_ended(repository.provider.forge) {
                return Some(repository);
            }
        }
        None
    }

    /// Current durable change state used by the root's procedure route.
    #[must_use]
    pub fn change(&self, task: u64) -> Option<&ChangeRow> {
        self.changes.get(&task)
    }

    /// A change's committed pull identity for bounded root brief sections.
    #[must_use]
    #[expect(clippy::manual_find, reason = "the strict subset uses explicit iteration instead of iterator closures")]
    pub fn change_for_pull(&self, repository: u32, number: u64) -> Option<&ChangeRow> {
        for (_, row) in &self.changes {
            if row.repository.repository == repository && row.pull == Some(number) {
                return Some(row);
            }
        }
        None
    }

    /// Repair owner and task for one landing queue, retained through restart.
    #[must_use]
    pub fn queue_repair(&self, repository: skein_forge::api::Repository, base: &[u8]) -> Option<(u64, u64)> {
        let mut newest = None;
        for (_, row) in &self.changes {
            if row.repository == repository
                && row.base.as_ref() == base
                && let Some(repair) = row.queue_repair
                && match newest {
                    Some((_, prior)) => repair > prior,
                    None => true,
                }
            {
                newest = Some((row.task, repair));
            }
        }
        newest
    }

    #[must_use]
    pub fn queue_repairs(&self, repository: skein_forge::api::Repository, base: &[u8]) -> u32 {
        let mut count = 0_u32;
        for (_, row) in &self.changes {
            if row.repository == repository && row.base.as_ref() == base {
                count = count.saturating_add(row.queue_repairs);
            }
        }
        count
    }

    #[must_use]
    pub fn queue_repair_owner(&self, repair: u64) -> Option<u64> {
        for (_, row) in &self.changes {
            if row.queue_repair == Some(repair) {
                return Some(row.task);
            }
        }
        None
    }

    /// Committed goal projection for root subscription and retry routing.
    #[must_use]
    pub fn issue(&self, goal: u64) -> Option<&IssueRow> {
        self.issues.get(&goal)
    }

    /// Save a changed goal priority and invalidate the affected queue panels (domain/forge.md, 9).
    pub fn priority(&mut self, env: &Env<Limits>, task: u64, priority: i32, out: &mut Queue<Request>) {
        let Some(row) = self.changes.get_mut(&task) else { return };
        if row.priority == priority {
            return;
        }
        let before = crate::panel::ready(row);
        row.priority = priority;
        let saved = row.clone();
        save_queue_change(self, env, before, saved, out);
    }

    /// Live change procedures in one repository, bounded by the connector's change table.
    #[must_use]
    pub fn changes_for(&self, repository: skein_forge::api::Repository, limit: u32) -> Box<[u64]> {
        let mut tasks = List::with_capacity(limit);
        for (task, row) in &self.changes {
            if row.repository == repository {
                tasks.push(*task).expect("change table bounds this list");
            }
        }
        tasks.into_boxed()
    }
}

/// Maximum requests from one top step, including retirement's child burst.
#[must_use]
pub const fn max_out(l: &Limits) -> u32 {
    // At most one panel mark accompanies each previously bounded output:
    // a saved change, or a read completion whose CI verdict changed.
    // Connection and Restored use the second output cohort for retirement's
    // Keep instead of panel marks (limits.rs). A demo start can also finish
    // every bounded waiting application together.
    l.output
        .saturating_mul(2)
        .saturating_add(l.applications)
        .saturating_add(l.tasks.saturating_add(1).saturating_mul(2))
        .saturating_add(l.landings)
        .saturating_add(l.landing_members)
        .saturating_add(l.entries)
}

/// Decide one parent event and collect its durable records and outputs.
#[expect(clippy::too_many_lines, reason = "one exhaustive connector input dispatcher")]
pub fn step(d: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Landing { row } => crate::landing::register(d, env, row, out),
        Event::VetoLanding { task, entry } => crate::landing::veto(d, task, entry, out),
        Event::StepLanding { task, entry, heard } => crate::landing::step(d, env, task, entry, heard, out),
        Event::Settle => {
            d.stopping = true;
            child(d, env, skein_forge_client::Event::Settle, out);
        }
        Event::Bind { request, mut address, subject } => {
            address.make_ascii_lowercase();
            assert!(crate::binding_address(&address) && subject.len() <= 255, "admitted binding fields");
            let answer = if let Some(bound) = d.bindings.get(&address) {
                if *bound == subject { crate::BindAnswer::Bound } else { crate::BindAnswer::Another }
            } else if d.bindings.insert(address.clone(), subject.clone()).is_ok() {
                out.push(Request::Save {
                    record: Stored::Binding { address, subject },
                    read_afresh: None,
                    release: None,
                });
                crate::BindAnswer::Bound
            } else {
                crate::BindAnswer::Full
            };
            out.push(Request::Bound { request, answer });
        }
        Event::DemoActs { acts } => crate::demo::acts(d, acts, out),
        Event::DemoActAnswered { key, keep } => crate::demo::act_answered(d, key, keep, out),
        Event::SendDemoActs => crate::demo::send_acts(d, out),
        Event::DemoAnswered { answer } => crate::demo::answered(d, answer, out),
        Event::BoxClosed => {
            d.under_box = false;
            for (forge, _) in &d.configured {
                if d.forge_ended(*forge) {
                    continue;
                }
                if let Some((state, failed)) = d.connections.get_mut(forge)
                    && let crate::Connection::SettingUp { .. } = state
                {
                    *failed = failed_at(state, *failed);
                    *state = crate::Connection::SetUpFailed { why: Box::from(b"the box closed the pipe".as_slice()) };
                }
            }
            out.push(Request::MarkedForges);
        }
        Event::AppRecorded { forge, app } => {
            if d.configured.contains_key(&forge) && !d.forge_ended(forge) {
                d.apps.insert(forge, app).expect("configured forge App way");
                out.push(Request::MarkedForges);
            }
        }
        Event::ConnectAnswered { forge, request, answer } => crate::connect::connected(d, forge, request, answer, out),
        Event::ReachableAnswered { forge, request, answer } => crate::connect::reached(d, forge, request, answer, out),
        Event::AcceptAnswered { forge, request, answer, stopped } => {
            crate::connect::accepted(d, forge, request, answer, stopped, out);
        }
        Event::InstallAnswered { forge, request, answer, stopped } => {
            crate::connect::installed(d, forge, request, answer, stopped, out);
        }
        Event::ConnectEnded { forge, request, connected, why, stop } => {
            crate::connect::ended(d, forge, request, connected, why, stop, out);
        }
        Event::SetUpAnswered { forge: _, request, answer } => {
            if d.demo_pending.contains_key(&request) {
                if let crate::SetUpAnswer::Refused(_) = answer {
                    crate::demo::answered(
                        d,
                        crate::StartAnswer::Failed(crate::StartFailure::Group { why: Box::from(b"busy".as_slice()) }),
                        out,
                    );
                }
            } else {
                d.setup_answer(env, request, answer, out);
            }
        }
        Event::FirstMade { forge, id } => {
            if d.first_made(forge, id) {
                out.push(Request::MarkedForges);
            }
        }
        Event::FirstFailed { forge, step } => {
            if d.first_failed(forge, step) {
                out.push(Request::MarkedForges);
            }
        }
        Event::Connection { forge, state } => {
            if d.forge_ended(forge) {
                return;
            }
            match &state {
                crate::Connection::Connected { id, .. } => {
                    d.ended.remove(&forge);
                    crate::demo::connected(d, forge, out);
                    retire_ended(d, env, out);
                    assert!(
                        writer(d, env, skein_forge_client::Writer { forge, author: *id }, out),
                        "same forge writer"
                    );
                    // A restart's NotConnected narrows nothing (domain/forge.md,
                    // 16). Resume these reads as client call room becomes free.
                    let mut waiting = List::with_capacity(env.limits.holds);
                    for (owner, pending) in &d.lost {
                        if pending.name.forge == forge && pending.state == LostReadState::WaitingForConnection {
                            waiting.push(*owner).expect("lost reads bounded by holds");
                        }
                    }
                    for owner in &waiting {
                        d.lost.get_mut(owner).expect("collected lost read").state = LostReadState::WaitingForRoom;
                    }
                }
                crate::Connection::NotConnected
                | crate::Connection::NotSetUp
                | crate::Connection::SettingUp { .. }
                | crate::Connection::SetUpFailed { .. }
                | crate::Connection::Connecting
                | crate::Connection::Stopped { .. } => {}
            }
            let failed = match &state {
                crate::Connection::SetUpFailed { .. } => match d.connections.get(&forge) {
                    Some((held, failed)) => failed_at(held, *failed),
                    None => None,
                },
                crate::Connection::NotConnected
                | crate::Connection::NotSetUp
                | crate::Connection::SettingUp { .. }
                | crate::Connection::Connecting
                | crate::Connection::Connected { .. }
                | crate::Connection::Stopped { .. } => None,
            };
            d.connections.insert(forge, (state, failed)).expect("configured forge connections");
            out.push(Request::MarkedForges);
        }
        Event::Installation { forge, installation, state } => {
            match state {
                Some(state) => {
                    d.installations.insert((forge, installation), state).expect("bounded installations");
                }
                None => {
                    let _gone = d.installations.remove(&(forge, installation));
                }
            }
            out.push(Request::MarkedForges);
        }
        Event::Application { request, token, retention, ask, role, carried } => match ask {
            ask @ (crate::Ask::Connect { .. }
            | crate::Ask::Install { .. }
            | crate::Ask::Accept { .. }
            | crate::Ask::Reachable { .. }) => {
                crate::connect::start(d, request, token, ask, role, carried, out);
            }
            crate::Ask::StartDemo => crate::demo::start(d, request, token, out),
            crate::Ask::Adopt(adoption) => adoption::start(d, env, request, token, retention, adoption, out),
            crate::Ask::SetUp { forge, org, repository, project, playground } => {
                let mut expired = None;
                for (token, (until, _)) in &d.setup_answers {
                    if *until <= env.wall {
                        expired = Some(*token);
                        break;
                    }
                }
                if let Some(token) = expired {
                    d.setup_answers.remove(&token);
                }
                if d.setup_pending.len().saturating_add(d.setup_answers.len()) >= env.limits.applications {
                    out.push(Request::ApplicationAgain { request, why: jig_domain_people::Again::Busy });
                } else {
                    d.setup_pending.insert(request, (token, retention)).expect("setup answer room checked");
                    let invalid_project = match &project {
                        crate::SetUpProject::New(name) => {
                            name.is_empty()
                                || name.len() > usize::try_from(env.limits.name_bytes).expect("name bound fits")
                        }
                        crate::SetUpProject::Existing(_) => false,
                    };
                    let refusal = if !setup_login(&org)
                        || !setup_repository(&repository)
                        || org.len() > usize::try_from(env.limits.name_bytes).expect("name bound fits")
                        || repository.len() > usize::try_from(env.limits.name_bytes).expect("name bound fits")
                        || invalid_project
                        || (playground && (org.as_ref() != b"playground" || repository.as_ref() != b"widget"))
                    {
                        Some(crate::SetUpRefusal::Invalid)
                    } else {
                        match d.offer(forge) {
                            crate::Offer::NotOffered => Some(crate::SetUpRefusal::NotOffered),
                            crate::Offer::NotUnderTheBox => Some(crate::SetUpRefusal::NotUnderTheBox),
                            crate::Offer::Offered => {
                                let mut busy = false;
                                for (_, (state, _)) in &d.connections {
                                    if let crate::Connection::SettingUp { .. } = state {
                                        busy = true;
                                    }
                                }
                                if busy { Some(crate::SetUpRefusal::Busy) } else { None }
                            }
                        }
                    };
                    match refusal {
                        Some(why) => d.setup_answer(env, request, crate::SetUpAnswer::Refused(why), out),
                        None => {
                            d.first
                                .insert(
                                    forge,
                                    crate::First {
                                        playground,
                                        org: org.clone(),
                                        repository: repository.clone(),
                                        project,
                                        state: crate::FirstState::Making,
                                        label: Box::from(b"temper-host".as_slice()),
                                    },
                                )
                                .expect("configured forges bound first repositories");
                            out.push(Request::MarkedForges);
                            out.push(Request::SetUp { forge, request, org, repository, playground });
                        }
                    }
                }
            }
        },
        Event::ApplicationAnswered { request, outcome } => adoption::answered(d, env, request, outcome, out),
        Event::ForgetProjection { goal } => {
            if let Some(row) = d.issues.get(&goal)
                && row.pending.is_none()
                && !row.failed
            {
                d.issues.remove(&goal);
                emit(out, Request::Erase { key: Key::Issue(goal) });
            }
        }
        Event::PlanBrief { section, source } => {
            assert!(d.brief_planned.insert(section, source) == Ok(None), "one bounded planned section");
        }
        Event::GatherPlanned { section, parts, bytes, ci_budget } => {
            let Some(source) = d.brief_planned.remove(&section) else {
                out.push(Request::BriefSized { section, size: None });
                return;
            };
            let max_job_bytes = match source {
                crate::BriefSource::Ci { .. } => env.limits.client.answer_bytes.min(ci_budget / 5).max(1),
                crate::BriefSource::Home { .. }
                | crate::BriefSource::Reviews { .. }
                | crate::BriefSource::Pull { .. } => 0,
            };
            held::gather(d, section, source, parts, bytes, max_job_bytes, env.limits.brief_bytes, out);
        }
        Event::GatherBrief { owner, source, parts, bytes, max_job_bytes } => {
            brief::gather(d, owner, source, parts, bytes, max_job_bytes, env.limits.brief_bytes, out);
        }
        Event::GatherBriefHeld { section, source, parts, bytes, max_job_bytes } => {
            held::gather(d, section, source, parts, bytes, max_job_bytes, env.limits.brief_bytes, out);
        }
        Event::CutBrief { section, bytes } => held::cut(d, section, bytes, out),
        Event::TakeBrief { section } => held::take(d, section, out),
        Event::DropBrief { section } => {
            d.brief_planned.remove(&section);
            held::drop_section(d, section);
        }
        Event::Adopt { request } => adoption::adopt(d, env, request, out),
        Event::ForgetAdoption { repository, restore } => match restore {
            Some(previous) => {
                d.repositories.insert(repository, previous.clone()).expect("restores an admitted repository");
                emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Repository(previous) });
                emit(out, Request::MarkedForges);
            }
            None => {
                if d.repositories.remove(&repository).is_some() {
                    emit(out, Request::Erase { key: Key::Repository(repository) });
                    emit(out, Request::MarkedForges);
                }
            }
        },
        Event::Names { task, resources } => names(d, env, task, resources, out),
        Event::Rename { task, resources } => {
            if d.names.contains_key(&task) {
                names(d, env, task, resources, out);
            }
        }
        Event::Unnamed { task } => unnamed(d, env, task, out),
        Event::Hold { task, resource, from } => hold(d, env, task, resource, from, out),
        Event::Claim { task, attempt, writes, holders } => claim(d, env, task, attempt, &writes, &holders, out),
        Event::Answered { task, attempt, pushed } => answered(d, env, task, attempt, &pushed, out),
        Event::Lost { task, attempt } => lost(d, env, task, attempt, out),
        Event::KeepProposedEffect { row } => {
            assert!(
                d.proposed_effects.insert(row.number, row.clone()).is_ok(),
                "one bounded effect proposal per live task"
            );
            emit(out, Request::Save { read_afresh: None, release: None, record: Stored::ProposedEffect(row) });
        }
        Event::DropProposedEffect { number } => {
            if d.proposed_effects.remove(&number).is_some() {
                emit(out, Request::Erase { key: Key::ProposedEffect(number) });
            }
        }
        Event::Enqueue { entry } => enqueue(d, env, entry, out),
        Event::KeepProjection { row, entry } => {
            d.issues.insert(row.goal, row.clone()).expect("projection capacity admitted");
            emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Issue(row) });
            keep_entry(d, entry, None, out);
        }
        Event::Project { entry, repository, view } => project(d, env, entry, repository, view, out),
        Event::FailProjection { goal, repository, closing } => {
            fail_projection(d, env, goal, Some(repository), closing, out);
        }
        Event::ProjectDesired { entry, goal } => {
            if let Some(row) = d.issues.get(&goal)
                && let Some(view) = &row.desired
            {
                project(d, env, entry, row.repository, view.clone(), out);
            }
        }
        Event::Change { row } => register_change(d, env, row, out),
        Event::QueueRepairStarted { owner, repair } => {
            if let Some(row) = d.changes.get_mut(&owner)
                && !row.base_repair
            {
                row.queue_repair = Some(repair);
                row.queue_repairs = row.queue_repairs.saturating_add(1);
                let saved = row.clone();
                save_change(d, out, saved);
            }
        }
        Event::Delegated { task, child, kind } => delegated(d, task, child, kind, out),
        Event::DelegateRefused { task, child } => delegate_refused(d, env, task, child, out),
        Event::DelegateResult { task, child, status, failed, cancelled, words } => {
            delegate_result(d, env, task, child, status, failed, cancelled, &words, out);
        }
        Event::StepChange { task, entry, heard, gates, queue_repair_active, kept } => {
            step_change(d, env, StepInput { task, entry, heard, gates, queue_repair_active, kept }, out);
        }
        Event::ReleaseChange { task } => release_change(d, env, task, out),
        Event::SettleEffects { task, root, ending } => settle_effects(d, env, task, root, ending, out),
        Event::Release { task, root, ending, reason, entry } => release(d, env, task, root, ending, reason, entry, out),
        Event::ContinueRelease { task, entry } => continue_release(d, env, task, entry, out),
        Event::ReleaseProjection { goal } => release_projection(d, goal, out),
        Event::Committed { entry } => committed(d, env, entry, out),
        Event::Resolve { entry, again } => resolve(d, env, entry, again, out),
        Event::Withdraw { entry } => withdraw(d, env, entry, out),
        Event::VetoChange { task, entry, prior } => veto_change(d, env, task, entry, prior, out),
        Event::Subscribe { subscription } => subscribe(d, env, subscription, out),
        Event::Unsubscribe { task, topic } => unsubscribe(d, task, topic, out),
        Event::Hint { hint } => {
            crate::landing::hinted(d, &hint, out);
            let changes = d.changes_for(hint.repository, env.limits.changes);
            // An unrecognised resource (for example after a rename) cannot
            // establish irrelevance: retain the repository-wide fallback.
            let mut matched = false;
            for task in &changes {
                if hints::concerns(d, d.changes.get(task).expect("repository's change"), &hint) {
                    matched = true;
                    break;
                }
            }
            crate::settle::hinted(d, env, &hint, &changes);
            for task in changes {
                if !matched || hints::concerns(d, d.changes.get(&task).expect("repository's change"), &hint) {
                    d.procedure_wakes.insert(task, true).expect("one pending wake per change");
                }
            }
            ci_hint(d, env, &hint, out);
            child(d, env, skein_forge_client::Event::Hint { hint }, out);
        }
        Event::Client(event) => child(d, env, event, out),
        Event::Restore { record } => restore(d, env, record),
        Event::Restored { clock } => restored(d, env, clock, out),
        Event::ReadAfresh => read_afresh(d, env, out),
        Event::SettleOutbox => settle_outbox(d, env, out),
    }
}

/// Drive one child call after prior saves have committed.
pub fn resume(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if d.restore_phase == RestorePhase::Restoring && !d.stopping {
        return;
    }
    if d.stopping {
        skein_forge_client::resume(&mut d.client, &child_env(env), &mut d.client_out);
        drain(d, env, out);
        return;
    }
    let before = out.len();
    handoff_one(d, env, out);
    if out.len() != before {
        // An admission refusal already spent this step's child output burst.
        return;
    }
    let child_env = child_env(env);
    skein_forge_client::resume(&mut d.client, &child_env, &mut d.client_out);
    drain(d, env, out);
    if d.client.has_room() {
        ask_lost_again(d, env, out);
    }
    if out.room() > 0
        && let Some((task, _)) = d.procedure_wakes.iter().next()
    {
        let task = *task;
        d.procedure_wakes.remove(&task);
        emit(out, Request::WakeProcedure { task });
    }
}

/// Expire one client timer.
pub fn fire(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if d.restore_phase == RestorePhase::Restoring && !d.stopping {
        return;
    }
    if d.stopping {
        skein_forge_client::fire(&mut d.client, &child_env(env), &mut d.client_out);
        drain(d, env, out);
        return;
    }
    // Keep the client child's whole cohort available. Excess ready timers stay
    // in their owner's bounded tables for the next iteration.
    let room = out.room().saturating_sub(skein_forge_client::max_out(&env.limits.client));
    let mut goals = List::with_capacity(env.limits.issues.min(room));
    for (goal, when) in &d.projection_due {
        if *when <= env.wall && goals.room() > 0 {
            goals.push(*goal).expect("bounded ready projection");
        }
    }
    for goal in &goals {
        d.projection_due.remove(goal);
        emit(out, Request::ProjectAfter { goal: *goal, when: None });
    }
    let room = out.room().saturating_sub(skein_forge_client::max_out(&env.limits.client));
    let mut changes = List::with_capacity(env.limits.changes.min(room));
    for (task, when) in &d.change_due {
        if *when <= env.wall && changes.room() > 0 {
            changes.push(*task).expect("bounded ready change");
        }
    }
    for task in &changes {
        d.change_due.remove(task);
        emit(out, Request::WakeProcedure { task: *task });
    }
    let room = out.room().saturating_sub(skein_forge_client::max_out(&env.limits.client));
    let mut landings = List::with_capacity(env.limits.landings.min(room));
    for (task, when) in &d.landing.due {
        if *when <= env.wall && landings.room() > 0 {
            landings.push(*task).expect("bounded due landings");
        }
    }
    for task in &landings {
        d.landing.due.remove(task);
        emit(out, Request::WakeProcedure { task: *task });
    }
    let child_env = child_env(env);
    skein_forge_client::fire(&mut d.client, &child_env, &mut d.client_out);
    drain(d, env, out);
}

/// Give a forge its writer once access is known (domain/forge.md, 4.1).
/// A refusal keeps neither a writer nor a durable row.
pub fn writer(d: &mut Domain, env: &Env<Limits>, writer: skein_forge_client::Writer, out: &mut Queue<Request>) -> bool {
    if !skein_forge_client::writer(&mut d.client, &child_env(env), writer, &mut d.client_out) {
        return false;
    }
    d.writers.insert(writer.forge, writer.author).expect("client writer fits the connector's table");
    drain(d, env, out);
    true
}

fn child_env(env: &Env<Limits>) -> Env<skein_forge_client::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.client }
}

pub(crate) fn child(d: &mut Domain, env: &Env<Limits>, event: skein_forge_client::Event, out: &mut Queue<Request>) {
    skein_forge_client::step(&mut d.client, &child_env(env), event, &mut d.client_out);
    drain(d, env, out);
}

#[expect(
    clippy::too_many_lines,
    reason = "one client-output cohort updates connector state before routing its outcomes"
)]
fn drain(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    for _ in 0..d.client_out.len() {
        let request = d.client_out.pop().expect("counted child output");
        match request {
            skein_forge_client::Request::Settled { left } => {
                let mut unfinished = List::with_capacity(env.limits.entries);
                for entry in left {
                    let row = d.entries.get(&entry).expect("client’s unfinished entry remains durable");
                    unfinished.push(crate::Unfinished { entry, task: row.entry.task }).expect("bounded entries");
                }
                emit(out, Request::Settled { unfinished: unfinished.into_boxed() });
            }
            skein_forge_client::Request::Save { record } => {
                let record = match record {
                    skein_forge_client::Stored::Writer(row) => Stored::Writer(row),
                    row @ (skein_forge_client::Stored::Live(_) | skein_forge_client::Stored::Repository(_)) => {
                        Stored::Client(row)
                    }
                };
                emit(out, Request::Save { read_afresh: None, release: None, record });
            }
            skein_forge_client::Request::Erase { key } => emit(out, Request::Erase { key: Key::Client(key) }),
            skein_forge_client::Request::Progress { entry } => keep_entry(d, entry, None, out),
            skein_forge_client::Request::Outcome { entry, task, outcome } => {
                let mut projection = false;
                for (_, row) in &d.issues {
                    projection |= row.pending == Some(entry);
                }
                let lost_comment = projection && outcome == skein_forge_client::Outcome::Held && posting(d, entry);
                d.named_outcome(entry, outcome);
                outcome_facts(d, env, entry, task, outcome, out);
                let retain = crate::landing::outcome(d, entry, outcome, out);
                match outcome {
                    skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => {}
                    skein_forge_client::Outcome::Held => {
                        if lost_comment {
                            // domain/forge.md, 12: an unfindable milestone counts
                            // as posted. Retire its entry without sending it again.
                            not_made(d, entry, out);
                        } else {
                            // It stays in the outbox, keeping its room, until a
                            // person or its task's close settles it.
                            if let Some(row) = d.entries.get_mut(&entry) {
                                row.held = true;
                                let record = Stored::Entry { entry: row.entry.clone(), held: true };
                                emit(out, Request::Save { read_afresh: None, release: None, record });
                            }
                            erase_flight(d, task, entry, out);
                        }
                    }
                    skein_forge_client::Outcome::Made { .. }
                    | skein_forge_client::Outcome::Failed(_)
                    | skein_forge_client::Outcome::Raced { .. }
                    | skein_forge_client::Outcome::Withdrawn => {
                        if !retain {
                            d.entries.remove(&entry);
                            emit(out, Request::Erase { key: Key::Entry(entry) });
                        }
                        erase_flight(d, task, entry, out);
                    }
                }
                if projection {
                    emit(out, Request::ProjectionOutcome { entry, outcome });
                } else {
                    emit(out, Request::Outcome { entry, task, outcome });
                }
                if outcome != skein_forge_client::Outcome::Uncertain
                    && outcome != skein_forge_client::Outcome::Queued
                    && match d.releases.get(&task) {
                        Some(row) => !row.failed,
                        None => false,
                    }
                {
                    emit(out, Request::ContinueRelease { task });
                }
            }
            skein_forge_client::Request::Changed { resource, result } => changed(d, env, resource, result, out),
            skein_forge_client::Request::Drift { resource, expected, observed } => {
                drift(d, env, resource, crate::DriftChange::Branch { expected, observed }, out);
            }
            skein_forge_client::Request::Call { call, repository, op } => {
                emit(out, Request::Call { call, repository, op });
            }
            skein_forge_client::Request::Kept { owner, result } => {
                if result.is_err() {
                    emit(out, Request::Refused { task: owner.raw() });
                }
            }
            skein_forge_client::Request::Read { owner, result } => {
                if d.landing.pending.contains_key(&owner) {
                    crate::landing::answered(d, env, owner, result, out);
                } else if d.capabilities.contains_key(&owner) {
                    crate::capabilities::answered(d, env, owner, result, out);
                } else if d.brief_fetches.contains_key(&owner) {
                    brief::brief_answer(d, owner, result, out);
                } else if d.adoptions.contains_key(&owner) {
                    adoption::adoption_read(d, env, owner, result, out);
                } else if d.landings.contains_key(&owner) {
                    landing_read(d, owner, result, out);
                } else if d.goal_file_reads.contains_key(&owner) {
                    topics::answer(d, env, owner, result, out);
                } else if d.steps.contains_key(&owner) {
                    step_read(d, env, owner, result, out);
                } else if d.pending_ci.contains_key(&owner) {
                    ci_read(d, owner, result, out);
                } else if owner.raw() & (1_u64 << 61_u32) != 0 && owner.raw() & (1_u64 << 62_u32) == 0 {
                    emit(out, Request::Read { owner, result });
                } else {
                    lost_read(d, env, owner, result, out);
                }
            }
            skein_forge_client::Request::ReadAfreshDone => d.restart_fresh = Some(true),
            skein_forge_client::Request::OutboxDone => emit(out, Request::RestartDone { stage: RestartStage::Settled }),
        }
    }
    read_afresh_complete(d, out);
}

fn outcome_facts(
    d: &mut Domain,
    env: &Env<Limits>,
    entry: u64,
    task: u64,
    outcome: skein_forge_client::Outcome,
    out: &mut Queue<Request>,
) {
    let forbidden = match outcome {
        skein_forge_client::Outcome::Failed(
            skein_forge::api::Error::Forbidden | skein_forge::api::Error::Protected,
        ) => true,
        skein_forge_client::Outcome::Made { .. }
        | skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Uncertain
        | skein_forge_client::Outcome::Queued
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => false,
    };
    if forbidden && let Some(row) = d.entries.get(&entry) {
        let repository = row.entry.repository;
        crate::capabilities::refresh(d, env, repository, out);
    }
    projection_outcome(d, env, entry, outcome, out);
    change_outcome(d, env, entry, outcome, out);
    release_outcome(d, entry, outcome, out);
    match outcome {
        skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Branch(commit), .. } => {
            if let Some(Outboxed { entry: kept, .. }) = d.entries.get(&entry) {
                let branch = match &kept.effect.write {
                    skein_forge::api::Write::CreateBranch { branch, .. } => Some(branch.clone()),
                    skein_forge::api::Write::CreateIssue { .. }
                    | skein_forge::api::Write::OpenPull { .. }
                    | skein_forge::api::Write::Post { .. }
                    | skein_forge::api::Write::Review { .. }
                    | skein_forge::api::Write::Edit { .. }
                    | skein_forge::api::Write::SetReviewers { .. }
                    | skein_forge::api::Write::Close { .. }
                    | skein_forge::api::Write::Reopen { .. }
                    | skein_forge::api::Write::Merge { .. }
                    | skein_forge::api::Write::Update { .. }
                    | skein_forge::api::Write::Status { .. }
                    | skein_forge::api::Write::Dispatch { .. }
                    | skein_forge::api::Write::Rerun { .. }
                    | skein_forge::api::Write::DeleteBranch { .. } => None,
                };
                if let Some(branch) = branch {
                    let resource = skein_forge_client::Resource {
                        repository: kept.repository,
                        what: skein_forge_client::What::Branch(branch),
                    };
                    if let Some(name) = from_client(&resource, &env.limits) {
                        let head = BranchHead { name: name.clone(), commit, owned: true };
                        if d.heads.insert(name, head.clone()).is_ok() {
                            emit(
                                out,
                                Request::Save { read_afresh: None, release: None, record: Stored::BranchHead(head) },
                            );
                        }
                    }
                }
            }
        }
        skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Merged(commit), .. } => {
            if d.landed.len() < env.limits.landings.max(env.limits.changes) || d.landed.contains_key(&commit) {
                d.landed.insert(commit, task).expect("preflighted landed capacity");
                emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Landed { commit, task } });
            }
        }
        skein_forge_client::Outcome::Made {
            made:
                skein_forge_client::Made::Created(_)
                | skein_forge_client::Made::Commented(_)
                | skein_forge_client::Made::Reviewed(_)
                | skein_forge_client::Made::Updated(_)
                | skein_forge_client::Made::Set,
            ..
        }
        | skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Uncertain
        | skein_forge_client::Outcome::Queued
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => {}
    }
}

pub(crate) fn emit(out: &mut Queue<Request>, request: Request) {
    out.push(request);
}

fn save_hold(out: &mut Queue<Request>, hold: Hold) {
    let read_afresh = if hold.writer.is_none() { Some(hold.name.clone()) } else { None };
    emit(out, Request::Save { record: Stored::Hold(hold), release: None, read_afresh });
}

fn valid_name(d: &Domain, l: &Limits, name: &Name) -> bool {
    if !d.repositories.contains_key(&skein_forge::api::Repository { forge: name.forge, repository: name.repository }) {
        return false;
    }
    match &name.what {
        What::Repository | What::Pull(_) | What::Issue(_) => true,
        What::Branch(parts) => {
            if parts.is_empty() {
                return false;
            }
            let mut bytes = 0usize;
            for part in parts {
                if part.is_empty() || part.contains(&b'/') {
                    return false;
                }
                let Some(size) = bytes.checked_add(part.len()) else { return false };
                let Some(next) = size.checked_add(1) else {
                    return false;
                };
                bytes = next;
            }
            bytes <= usize::try_from(l.name_bytes).expect("u32 fits usize")
        }
    }
}

fn to_client(name: &Name, l: &Limits) -> Option<skein_forge_client::Resource> {
    let repository = skein_forge::api::Repository { forge: name.forge, repository: name.repository };
    let what = match &name.what {
        What::Repository => skein_forge_client::What::Repository,
        What::Pull(number) => skein_forge_client::What::Pull(*number),
        What::Issue(number) => skein_forge_client::What::Issue(*number),
        What::Branch(parts) => {
            let mut bytes = List::with_capacity(l.name_bytes);
            for (index, part) in parts.iter().enumerate() {
                if index > 0 && bytes.push(b'/').is_err() {
                    return None;
                }
                for byte in part {
                    if bytes.push(*byte).is_err() {
                        return None;
                    }
                }
            }
            skein_forge_client::What::Branch(bytes.into_boxed())
        }
    };
    Some(skein_forge_client::Resource { repository, what })
}

fn participating(d: &Domain, resource: &skein_forge_client::Resource) -> bool {
    let (skein_forge_client::What::Issue(number) | skein_forge_client::What::Pull(number)) = &resource.what else {
        return false;
    };
    for (_, subscriber) in &d.subscriptions {
        if subscriber.topic == (Topic::Participation { repository: resource.repository, number: *number }) {
            return true;
        }
    }
    false
}

fn names(d: &mut Domain, env: &Env<Limits>, task: u64, resources: Box<[Name]>, out: &mut Queue<Request>) {
    if resources.len() > usize::try_from(env.limits.resources_per_task).expect("u32 fits usize")
        || (!d.names.contains_key(&task) && d.names.len() == env.limits.tasks)
    {
        emit(out, Request::Refused { task });
        return;
    }
    let mut watches = Map::with_capacity(env.limits.client.resources);
    for (other_task, row) in &d.names {
        if *other_task != task {
            for name in row {
                if d.forge_ended(name.forge) {
                    continue;
                }
                let Some(resource) = to_client(name, &env.limits) else {
                    emit(out, Request::Refused { task });
                    return;
                };
                let participating = participating(d, &resource);
                if watches.insert(resource.clone(), skein_forge_client::Watch { resource, participating }).is_err() {
                    emit(out, Request::Refused { task });
                    return;
                }
            }
        }
    }
    for name in &resources {
        if !valid_name(d, &env.limits, name) {
            emit(out, Request::Refused { task });
            return;
        }
        if d.forge_ended(name.forge) {
            continue;
        }
        let Some(resource) = to_client(name, &env.limits) else {
            emit(out, Request::Refused { task });
            return;
        };
        let participating = participating(d, &resource);
        if watches.insert(resource.clone(), skein_forge_client::Watch { resource, participating }).is_err() {
            emit(out, Request::Refused { task });
            return;
        }
    }
    let mut list: List<skein_forge_client::Watch> = List::with_capacity(env.limits.client.resources);
    for (_, watch) in &watches {
        list.push(watch.clone()).expect("map capped to watch list");
    }
    skein_forge_client::step(
        &mut d.client,
        &child_env(env),
        skein_forge_client::Event::Keep { owner: Token::new(task), watches: list.into_boxed() },
        &mut d.client_out,
    );
    if kept(&d.client_out) {
        d.names.insert(task, resources.clone()).expect("preflighted task capacity");
        for name in &resources {
            if let Some((role, hold)) = d.resource_facts(name) {
                emit(
                    out,
                    Request::Resource {
                        project: d
                            .repository(skein_forge::api::Repository { forge: name.forge, repository: name.repository })
                            .expect("named adopted resource")
                            .project,
                        name: name.clone(),
                        role,
                        hold,
                    },
                );
            }
        }
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Names { task, resources } });
    }
    drain(d, env, out);
}

fn unnamed(d: &mut Domain, env: &Env<Limits>, task: u64, out: &mut Queue<Request>) {
    let mut list: List<skein_forge_client::Watch> = List::with_capacity(env.limits.client.resources);
    for (owner, row) in &d.names {
        if *owner == task {
            continue;
        }
        for name in row {
            if d.forge_ended(name.forge) {
                continue;
            }
            let Some(resource) = to_client(name, &env.limits) else { continue };
            let mut present = false;
            for watch in &list {
                if watch.resource == resource {
                    present = true;
                }
            }
            let participating = participating(d, &resource);
            if !present && list.push(skein_forge_client::Watch { resource, participating }).is_err() {
                emit(out, Request::Refused { task });
                return;
            }
        }
    }
    skein_forge_client::step(
        &mut d.client,
        &child_env(env),
        skein_forge_client::Event::Keep { owner: Token::new(task), watches: list.into_boxed() },
        &mut d.client_out,
    );
    if kept(&d.client_out) && d.names.remove(&task).is_some() {
        emit(out, Request::Erase { key: Key::Names(task) });
    }
    drain(d, env, out);
}

fn kept(out: &Queue<skein_forge_client::Request>) -> bool {
    for request in out {
        match request {
            skein_forge_client::Request::Kept { result, .. } => return result.is_ok(),
            skein_forge_client::Request::Changed { .. }
            | skein_forge_client::Request::Drift { .. }
            | skein_forge_client::Request::Save { .. }
            | skein_forge_client::Request::Progress { .. }
            | skein_forge_client::Request::Erase { .. }
            | skein_forge_client::Request::Outcome { .. }
            | skein_forge_client::Request::Call { .. }
            | skein_forge_client::Request::Read { .. }
            | skein_forge_client::Request::ReadAfreshDone
            | skein_forge_client::Request::OutboxDone
            | skein_forge_client::Request::Settled { .. } => {}
        }
    }
    false
}

fn hold(d: &mut Domain, env: &Env<Limits>, task: u64, resource: Name, from: Option<u64>, out: &mut Queue<Request>) {
    let holdable = match d.resource_facts(&resource) {
        Some((_, crate::resources::HoldKind::Exclusive { .. })) => true,
        Some((_, crate::resources::HoldKind::Shared)) | None => false,
    };
    if !holdable
        || !valid_name(d, &env.limits, &resource)
        || (!d.holds.contains_key(&resource) && d.holds.len() == env.limits.holds)
    {
        emit(out, Request::Refused { task });
        return;
    }
    if let Some(existing) = d.holds.get(&resource) {
        if existing.task != task && Some(existing.task) != from {
            emit(out, Request::Taken { task, resource, by: existing.task });
            return;
        }
    } else if from.is_some() {
        emit(out, Request::Refused { task });
        return;
    }
    let writer = match d.holds.get(&resource) {
        Some(row) => row.writer,
        None => None,
    };
    let drift = match d.holds.get(&resource) {
        Some(row) => row.drift.clone(),
        None => None,
    };
    let row = Hold { name: resource.clone(), task, writer, drift };
    d.holds.insert(resource, row.clone()).expect("preflighted hold capacity");
    save_hold(out, row);
}

fn claim(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    attempt: u64,
    writes: &[Name],
    holders: &[u64],
    out: &mut Queue<Request>,
) {
    if writes.len() > usize::try_from(env.limits.resources_per_task).expect("u32 fits usize") {
        emit(out, Request::Refused { task });
        return;
    }
    for name in writes {
        let Some(row) = d.holds.get(name) else {
            emit(out, Request::Refused { task });
            return;
        };
        if row.task != task && !holders.contains(&row.task) {
            emit(out, Request::Taken { task, resource: name.clone(), by: row.task });
            return;
        }
        match row.writer {
            Some(Writer::Run { task: owner, attempt: old }) if owner == task && old == attempt => {}
            Some(Writer::Run { task: owner, .. } | Writer::Lost { task: owner, .. }) => {
                emit(out, Request::Taken { task, resource: name.clone(), by: owner });
                return;
            }
            Some(Writer::Entry(entry)) => {
                emit(out, Request::Taken { task, resource: name.clone(), by: entry });
                return;
            }
            None => {}
        }
    }
    for name in writes {
        let Some(row) = d.holds.get_mut(name) else { unreachable!("preflighted hold") };
        row.writer = Some(Writer::Run { task, attempt });
        let saved = row.clone();
        save_hold(out, saved);
        if let Some(resource) = to_client(name, &env.limits) {
            child(d, env, skein_forge_client::Event::Writer { resource, taken: true }, out);
        }
    }
}

fn answered(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    attempt: u64,
    pushed: &[(Name, skein_forge::api::Commit)],
    out: &mut Queue<Request>,
) {
    let mut freed = List::with_capacity(env.limits.holds);
    for (name, row) in &d.holds {
        let mut reported = false;
        for (pushed_name, _) in pushed {
            if pushed_name == name {
                reported = true;
            }
        }
        if row.writer == Some(Writer::Run { task, attempt }) && reported {
            freed.push(name.clone()).expect("list sized to holds");
        }
    }
    for name in &freed {
        let Some(row) = d.holds.get_mut(name) else { unreachable!("collected hold") };
        row.writer = None;
        let saved = row.clone();
        save_hold(out, saved);
        let undrifted = row.drift.is_none();
        if let Some(resource) = to_client(name, &env.limits) {
            for (pushed_name, commit) in pushed {
                if pushed_name == name && undrifted {
                    let head = BranchHead { name: name.clone(), commit: *commit, owned: true };
                    d.heads.insert(name.clone(), head.clone()).expect("reported head was a live resource");
                    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::BranchHead(head) });
                    child(
                        d,
                        env,
                        skein_forge_client::Event::Pushed { resource: resource.clone(), commit: *commit },
                        out,
                    );
                }
            }
            child(d, env, skein_forge_client::Event::Writer { resource, taken: false }, out);
        }
    }
}

fn lost(d: &mut Domain, env: &Env<Limits>, task: u64, attempt: u64, out: &mut Queue<Request>) {
    let mut names = List::with_capacity(env.limits.holds);
    for (name, row) in &d.holds {
        if d.forge_ended(name.forge) {
            continue;
        }
        if row.writer == Some(Writer::Run { task, attempt }) || row.writer == Some(Writer::Lost { task, attempt }) {
            names.push(name.clone()).expect("list sized to holds");
        }
    }
    for name in &names {
        let Some(row) = d.holds.get_mut(name) else { unreachable!("collected hold") };
        row.writer = Some(Writer::Lost { task, attempt });
        save_hold(out, row.clone());
        let Some(resource) = to_client(name, &env.limits) else { continue };
        let read = match resource.what {
            skein_forge_client::What::Branch(branch) => skein_forge::api::Read::Branch { branch },
            skein_forge_client::What::Repository
            | skein_forge_client::What::Pull(_)
            | skein_forge_client::What::Issue(_) => {
                lost_release(d, name, true, out);
                emit(out, Request::Refused { task });
                continue;
            }
        };
        let Some(sequence) = d.sequence.checked_add(1) else {
            lost_release(d, name, true, out);
            emit(out, Request::Refused { task });
            return;
        };
        d.sequence = sequence;
        let owner = Token::new(sequence | (1_u64 << 62));
        if d.lost
            .insert(
                owner,
                PendingLost { task, attempt, name: name.clone(), comparison: None, state: LostReadState::Asked },
            )
            .is_err()
        {
            lost_release(d, name, true, out);
            emit(out, Request::Refused { task });
            return;
        }
        child(d, env, skein_forge_client::Event::Read { owner, repository: resource.repository, read }, out);
    }
}

/// Ask one waiting lost writer's read again: its branch, or the comparison
/// of the head it found with the head the forge last knew.
fn ask_lost_again(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut waiting = None;
    for (owner, pending) in &d.lost {
        if pending.state == LostReadState::WaitingForRoom && waiting.is_none() {
            waiting = Some(*owner);
        }
    }
    let Some(owner) = waiting else { return };
    let pending = d.lost.get_mut(&owner).expect("found above");
    pending.state = LostReadState::Asked;
    let comparison = pending.comparison;
    let Some(resource) = to_client(&pending.name, &env.limits) else { return };
    let read = match (comparison, resource.what) {
        (Some((before, after)), _) => skein_forge::api::Read::Compare { before, after },
        (None, skein_forge_client::What::Branch(branch)) => skein_forge::api::Read::Branch { branch },
        (
            None,
            skein_forge_client::What::Repository
            | skein_forge_client::What::Pull(_)
            | skein_forge_client::What::Issue(_),
        ) => {
            unreachable!("a lost writer holds a branch")
        }
    };
    child(d, env, skein_forge_client::Event::Read { owner, repository: resource.repository, read }, out);
}

fn lost_read(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(mut pending) = d.lost.remove(&owner) else { return };
    // The writer slot is freed only once the branch is read afresh (jig's
    // domain/tasks.md, 6.3). Busy waits for call room; NotConnected waits
    // for this forge's connection (domain/forge.md, 16), without ready work.
    let waiting = match result {
        Err(skein_forge::api::Error::Busy) => Some(LostReadState::WaitingForRoom),
        Err(skein_forge::api::Error::NotConnected) => Some(LostReadState::WaitingForConnection),
        Ok(_) | Err(_) => None,
    };
    if let Some(state) = waiting {
        pending.state = state;
        d.lost.insert(owner, pending).expect("the read keeps its own lost slot");
        return;
    }
    let Some(resource) = to_client(&pending.name, &env.limits) else { return };
    let baseline = d.branch_head(&pending.name);
    let observed = match (pending.comparison, result) {
        (None, Ok(skein_forge::api::Answer::Commit(commit))) => {
            if let Some(before) = baseline
                && before != commit
            {
                pending.comparison = Some((before, commit));
                d.lost.insert(owner, pending).expect("comparison occupies its reserved lost-read slot");
                child(
                    d,
                    env,
                    skein_forge_client::Event::Read {
                        owner,
                        repository: resource.repository,
                        read: skein_forge::api::Read::Compare { before, after: commit },
                    },
                    out,
                );
                return;
            }
            Some(commit)
        }
        (
            Some((expected_before, expected_after)),
            Ok(skein_forge::api::Answer::Compare { before, after, contains_before, .. }),
        ) if before == expected_before && after == expected_after => {
            if !contains_before {
                drift(
                    d,
                    env,
                    resource.clone(),
                    crate::DriftChange::Branch { expected: before, observed: Some(after) },
                    out,
                );
            }
            Some(after)
        }
        (None, Err(skein_forge::api::Error::Missing)) => {
            if let Some(expected) = baseline {
                drift(d, env, resource.clone(), crate::DriftChange::Branch { expected, observed: None }, out);
            }
            // A run can end before its first push; fresh absence releases
            // its writer when no previous head was recorded.
            None
        }
        (_, Ok(_) | Err(_)) => {
            lost_release(d, &pending.name, true, out);
            emit(out, Request::Refused { task: pending.task });
            return;
        }
    };
    let Some(row) = d.holds.get_mut(&pending.name) else { return };
    if row.writer != Some(Writer::Lost { task: pending.task, attempt: pending.attempt }) {
        return;
    }
    row.writer = None;
    let undrifted = row.drift.is_none();
    save_hold(out, row.clone());
    if undrifted && let Some(commit) = observed {
        let head = BranchHead { name: pending.name.clone(), commit, owned: true };
        if d.heads.insert(pending.name.clone(), head.clone()).is_err() {
            lost_release(d, &pending.name, true, out);
            emit(out, Request::Refused { task: pending.task });
            return;
        }
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::BranchHead(head) });
        child(d, env, skein_forge_client::Event::Pushed { resource: resource.clone(), commit }, out);
    }
    child(d, env, skein_forge_client::Event::Writer { resource, taken: false }, out);
    lost_release(d, &pending.name, false, out);
}

/// Wake the holder's existing release after its lost writer's reread ends.
fn lost_release(d: &Domain, name: &Name, failed: bool, out: &mut Queue<Request>) {
    let Some(hold) = d.holds.get(name) else { return };
    let task = hold.task;
    let Some(row) = d.releases.get(&task) else { return };
    if row.releasing && !row.failed && row.pending.is_none() {
        emit(out, if failed { Request::ReleaseFailed { task } } else { Request::ContinueRelease { task } });
    }
}

fn enqueue(d: &mut Domain, env: &Env<Limits>, entry: skein_forge_client::Entry, out: &mut Queue<Request>) {
    let task = entry.task;
    let Some(repository) = d.repositories.get(&entry.repository) else {
        emit(out, Request::Refused { task });
        return;
    };
    if repository.role == Role::Context
        || !effect_permitted(repository.kinds, &entry.effect.write)
        || (!d.entries.contains_key(&entry.number) && d.entries.len() == env.limits.entries)
    {
        emit(out, Request::Refused { task });
        return;
    }
    if d.entries.contains_key(&entry.number) {
        emit(out, Request::Refused { task });
        return;
    }
    let release = Some(entry.number);
    keep_entry(d, entry, release, out);
}

/// Save one entry in the client's hands: newly committed, for the client to
/// make once the commit is durable, or a known entry's progress.
pub(crate) fn keep_entry(
    d: &mut Domain,
    entry: skein_forge_client::Entry,
    release: Option<u64>,
    out: &mut Queue<Request>,
) {
    let handoff = match d.entries.get(&entry.number) {
        Some(row) => row.handoff,
        None => Handoff::Uncommitted,
    };
    if entry.attempt.is_some() {
        d.flights.insert((entry.task, entry.number), ()).expect("flights are bounded by entries");
        emit(
            out,
            Request::Save {
                read_afresh: None,
                release: None,
                record: Stored::Flight { task: entry.task, entry: entry.number },
            },
        );
    }
    let row = Outboxed { entry: entry.clone(), held: false, handoff };
    d.entries.insert(entry.number, row).expect("entry capacity checked or a known entry replaced");
    emit(out, Request::Save { read_afresh: None, release, record: Stored::Entry { entry, held: false } });
}

fn effect_permitted(kinds: Kinds, write: &skein_forge::api::Write) -> bool {
    match write {
        skein_forge::api::Write::CreateIssue { .. } => kinds.issue,
        skein_forge::api::Write::OpenPull { .. } => kinds.open,
        skein_forge::api::Write::Post { .. } => kinds.comment,
        skein_forge::api::Write::Review { .. } | skein_forge::api::Write::SetReviewers { .. } => kinds.review,
        skein_forge::api::Write::Edit { .. }
        | skein_forge::api::Write::Close { .. }
        | skein_forge::api::Write::Reopen { .. } => kinds.issue || kinds.open,
        skein_forge::api::Write::Merge { .. } => kinds.land,
        skein_forge::api::Write::Update { .. }
        | skein_forge::api::Write::CreateBranch { .. }
        | skein_forge::api::Write::Dispatch { .. }
        | skein_forge::api::Write::Rerun { .. }
        | skein_forge::api::Write::DeleteBranch { .. } => kinds.branch,
        skein_forge::api::Write::Status { .. } => kinds.status,
    }
}

fn register_change(d: &mut Domain, env: &Env<Limits>, mut row: ChangeRow, out: &mut Queue<Request>) {
    let task = row.task;
    if !d.repositories.contains_key(&row.repository)
        || row.change.task != task
        || row.branch.is_empty()
        || row.base.is_empty()
        || row.branch.len() > usize::try_from(env.limits.name_bytes).expect("u32 fits usize")
        || row.base.len() > usize::try_from(env.limits.name_bytes).expect("u32 fits usize")
        || (!d.changes.contains_key(&task) && d.changes.len() == env.limits.changes)
        || d.changes.contains_key(&task)
    {
        emit(out, Request::Refused { task });
        return;
    }
    row.change.way = d.landing.way(d.repositories.get(&row.repository).expect("adopted").project);
    d.changes.insert(task, row.clone()).expect("change capacity checked");
    save_queue_change(d, env, None, row, out);
}

fn delegated(d: &mut Domain, task: u64, child: u64, kind: change::Delegate, out: &mut Queue<Request>) {
    let Some(row) = d.changes.get_mut(&task) else {
        emit(out, Request::Refused { task });
        return;
    };
    row.delegate = Some((child, kind));
    row.delegate_status = change::Status::Unknown;
    let saved = row.clone();
    save_change(d, out, saved);
}

fn delegate_refused(d: &mut Domain, env: &Env<Limits>, task: u64, child: u64, out: &mut Queue<Request>) {
    let Some(row) = d.changes.get_mut(&task) else { return };
    let before = crate::panel::ready(row);
    let Some((expected, kind)) = row.delegate else { return };
    if expected != child {
        return;
    }
    row.delegate = None;
    row.delegate_status = change::Status::Unknown;
    let state = match (kind, &row.change.state) {
        (change::Delegate::Produce, change::State::Producing { .. }) => change::State::Producing { requested: false },
        (change::Delegate::Repair(why), change::State::Repairing { .. }) => {
            row.change.repairs = row.change.repairs.saturating_sub(1);
            change::State::Repairing { requested: false, why }
        }
        (change::Delegate::Resolve { base }, change::State::Resolving { .. }) => {
            row.change.resolutions = row.change.resolutions.saturating_sub(1);
            change::State::Resolving { requested: false, base }
        }
        (change::Delegate::Gate { number, .. }, change::State::Gating { head, asked }) => {
            let mut remaining = List::with_capacity(u32::try_from(asked.len()).expect("bounded gate list"));
            for prior in asked {
                if *prior != number {
                    remaining.push(*prior).expect("same gate room");
                }
            }
            change::State::Gating { head: *head, asked: remaining.into_boxed() }
        }
        _ => row.change.state.clone(),
    };
    row.change.state = change::State::Held { was: Box::new(state), why: change::Hold::Failed };
    let saved = row.clone();
    save_queue_change(d, env, before, saved, out);
}

#[expect(clippy::too_many_arguments, reason = "one child's terminal carries its status, failure standing and words")]
fn delegate_result(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    child: u64,
    status: change::Status,
    failed: bool,
    cancelled: bool,
    words: &[u8],
    out: &mut Queue<Request>,
) {
    let Some(row) = d.changes.get_mut(&task) else { return };
    let Some((expected, kind)) = row.delegate else { return };
    if expected != child {
        return;
    }
    row.delegate = None;
    row.delegate_status = status;
    match kind {
        change::Delegate::Gate { number, head } => {
            // A result racing the CI cancellation is no verdict or review failure.
            let cancelling = row.change.state == (change::State::CancellingGates { head });
            if cancelled || cancelling {
                let saved = row.clone();
                save_change(d, out, saved);
                return;
            }
            let report = change::GateReport { number, head, status };
            let mut verdicts = List::with_capacity(env.limits.change_policy.gates);
            for old in &row.verdicts {
                if old.number != number {
                    verdicts.push(*old).expect("bounded prior gate reports");
                }
            }
            if !failed && verdicts.push(report).is_err() {
                emit(out, Request::Refused { task });
                return;
            }
            row.verdicts = verdicts.into_boxed();
            if failed {
                row.change = change::review_failed(&row.change, number, head, env.wall, &env.limits.change_policy);
            }
            let mut remarks = List::with_capacity(env.limits.change_policy.gates);
            for old in &row.gate_remarks {
                if old.number != number {
                    remarks.push(old.clone()).expect("bounded prior gate remarks");
                }
            }
            let kept = words.len().min(usize::try_from(env.limits.client.answer_bytes).expect("u32 fits usize"));
            remarks
                .push(crate::GateRemark {
                    number,
                    head,
                    words: Box::from(words.get(..kept).expect("kept is in bounds")),
                })
                .expect("gate room checked");
            row.gate_remarks = remarks.into_boxed();
        }
        change::Delegate::Produce | change::Delegate::Repair(_) | change::Delegate::Resolve { .. } => {}
    }
    let saved = row.clone();
    save_change(d, out, saved);
}

fn step_change(d: &mut Domain, env: &Env<Limits>, input: StepInput, out: &mut Queue<Request>) {
    if d.stopping {
        return;
    }
    let StepInput { task, entry, mut heard, gates, queue_repair_active, kept } = input;
    if heard.released {
        release_change(d, env, task, out);
        heard.released = false;
    }
    let Some(row) = d.changes.get(&task) else {
        emit(out, Request::Refused { task });
        return;
    };
    let mut owner = None;
    for (token, pending) in &d.steps {
        if pending.task == task {
            owner = Some(*token);
        }
    }
    if gates.len() > usize::try_from(env.limits.change_policy.gates).expect("u32 fits usize") {
        emit(out, Request::Refused { task });
        return;
    }
    if let Some(owner) = owner {
        let pending = d.steps.get_mut(&owner).expect("found pending step");
        // Entries hand off in number order (skein's forge-client.md, 7.2);
        // the newest wake reserves the restart's entry and reads jig's current kept answer.
        pending.entry = entry;
        pending.heard.lacking = heard.lacking;
        pending.heard.spend = heard.spend;
        pending.kept = kept;
        pending.queue_repair_active = queue_repair_active;
        pending.gates = gates;
        pending.woken = true;
        return;
    }
    if d.steps.len() == env.limits.changes {
        emit(out, Request::Refused { task });
        return;
    }
    let repository = row.repository;
    let branch = row.branch.clone();
    let Some(sequence) = d.sequence.checked_add(1) else {
        emit(out, Request::Refused { task });
        return;
    };
    d.sequence = sequence;
    let owner = Token::new(sequence | (1_u64 << 61));
    let pending = PendingStep {
        task,
        entry,
        stage: StepStage::Branch,
        heard: change::Heard {
            delegate: if row.delegate_status == change::Status::Unknown { heard.delegate } else { row.delegate_status },
            ..heard
        },
        gates: if gates.is_empty() { row.verdicts.clone() } else { gates },
        queue_repair_active,
        kept,
        branch: None,
        base_tip: None,
        pull: None,
        ci: change::Status::Unknown,
        base_ci: change::Status::Unknown,
        contains_base: change::Status::Unknown,
        reviews: Box::new([]),
        reviews_complete: true,
        woken: false,
    };
    d.steps.insert(owner, pending).expect("step capacity checked");
    child(
        d,
        env,
        skein_forge_client::Event::Read { owner, repository, read: skein_forge::api::Read::Branch { branch } },
        out,
    );
}

fn status(ci: skein_forge::api::Ci) -> change::Status {
    match ci {
        skein_forge::api::Ci::None => change::Status::Unknown,
        skein_forge::api::Ci::Pending => change::Status::Pending,
        skein_forge::api::Ci::Passed => change::Status::Passed,
        skein_forge::api::Ci::Failed => change::Status::Failed,
    }
}

#[expect(clippy::too_many_lines, reason = "one bounded fresh-read chain preserves its stage and facts")]
fn step_read(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(mut pending) = d.steps.remove(&owner) else { return };
    if d.stopping {
        return;
    }
    if result == Err(skein_forge::api::Error::Busy) {
        emit(
            out,
            Request::ChangeDecision {
                task: pending.task,
                decision: change::Decision::Wait {
                    until: Some(skein_lib::Wall::from_nanos(env.wall.as_nanos().saturating_add(1_000_000_000))),
                },
                entry: None,
                evidence: None,
            },
        );
        return;
    }
    let Some(row) = d.changes.get(&pending.task) else { return };
    let repository = row.repository;
    let branch = row.branch.clone();
    let base = row.base.clone();
    let required = match d.repositories.get(&repository) {
        Some(repository) => crate::ci::required(&repository.protection),
        None => &[],
    };
    let next = match pending.stage {
        StepStage::Branch => {
            match result {
                Ok(skein_forge::api::Answer::Commit(commit)) => pending.branch = Some(commit),
                Err(skein_forge::api::Error::Missing) => pending.branch = None,
                Ok(_) | Err(_) => {
                    emit(out, Request::Refused { task: pending.task });
                    return;
                }
            }
            pending.stage = StepStage::Base;
            skein_forge::api::Read::Branch { branch: base }
        }
        StepStage::Base => {
            match result {
                Ok(skein_forge::api::Answer::Commit(commit)) => {
                    pending.base_tip =
                        Some(if let change::State::Preparing { main, .. } = row.change.state { main } else { commit });
                }
                Ok(_) | Err(_) => {
                    emit(out, Request::Refused { task: pending.task });
                    return;
                }
            }
            pending.stage = StepStage::Pull;
            skein_forge::api::Read::PullFor { head: branch, base }
        }
        StepStage::Pull => {
            match result {
                Ok(skein_forge::api::Answer::Pull(pull)) => pending.pull = Some(pull),
                Err(skein_forge::api::Error::Missing) => pending.pull = None,
                Ok(_) | Err(_) => {
                    emit(out, Request::Refused { task: pending.task });
                    return;
                }
            }
            pending.stage = StepStage::Ci;
            let Some(commit) = pending.branch else {
                pending.stage = StepStage::BaseCi;
                let Some(base_tip) = pending.base_tip else { unreachable!("base read") };
                d.steps.insert(owner, pending).expect("replaces step");
                child(
                    d,
                    env,
                    skein_forge_client::Event::Read {
                        owner,
                        repository,
                        read: skein_forge::api::Read::Checks { commit: base_tip },
                    },
                    out,
                );
                return;
            };
            if required.is_empty()
                && let Some(pull) = &pending.pull
                && pull.commit == commit
            {
                pending.ci = status(pull.ci);
                keep_panel_ci(d, pending.task, change::Ci { head: commit, status: pending.ci }, out);
                pending.stage = StepStage::BaseCi;
                let Some(base_tip) = pending.base_tip else { unreachable!("base read") };
                skein_forge::api::Read::Checks { commit: base_tip }
            } else {
                skein_forge::api::Read::Checks { commit }
            }
        }
        StepStage::Ci => {
            match result {
                Ok(skein_forge::api::Answer::Checks(rows)) => pending.ci = status(crate::ci::fold(&rows, required)),
                Ok(_) | Err(_) => pending.ci = change::Status::Unknown,
            }
            if let Some(head) = pending.branch {
                keep_panel_ci(d, pending.task, change::Ci { head, status: pending.ci }, out);
            }
            pending.stage = StepStage::BaseCi;
            let Some(base_tip) = pending.base_tip else { unreachable!("base read") };
            skein_forge::api::Read::Checks { commit: base_tip }
        }
        StepStage::BaseCi => {
            match result {
                Ok(skein_forge::api::Answer::Checks(rows)) => {
                    pending.base_ci = status(crate::ci::fold(&rows, required));
                }
                Ok(_) | Err(_) => pending.base_ci = change::Status::Unknown,
            }
            let Some(head) = pending.branch else {
                finish_step(d, env, pending, out);
                return;
            };
            let Some(base_tip) = pending.base_tip else { unreachable!("base read") };
            pending.stage = StepStage::Compare;
            skein_forge::api::Read::Compare { before: base_tip, after: head }
        }
        StepStage::Compare => {
            match result {
                Ok(skein_forge::api::Answer::Compare { before, after, contains_before, .. })
                    if Some(before) == pending.base_tip && Some(after) == pending.branch =>
                {
                    pending.contains_base =
                        if contains_before { change::Status::Passed } else { change::Status::Failed };
                }
                Ok(_) | Err(_) => pending.contains_base = change::Status::Unknown,
            }
            if let Some(pull) = &pending.pull {
                pending.stage = StepStage::Reviews(1);
                skein_forge::api::Read::Reviews { number: pull.number, page: 1 }
            } else {
                finish_step(d, env, pending, out);
                return;
            }
        }
        StepStage::Reviews(page) => match result {
            Ok(skein_forge::api::Answer::Reviews { reviews, more }) => {
                let Some(total) = pending.reviews.len().checked_add(reviews.len()) else {
                    emit(out, Request::Refused { task: pending.task });
                    return;
                };
                if total > usize::try_from(env.limits.client.inbox).expect("u32 fits usize") {
                    pending.reviews_complete = false;
                    finish_step(d, env, pending, out);
                    return;
                }
                let mut gathered = List::with_capacity(env.limits.client.inbox);
                for review in &pending.reviews {
                    gathered.push(review.clone()).expect("review bound preflighted");
                }
                for review in reviews {
                    gathered.push(review).expect("review bound preflighted");
                }
                pending.reviews = gathered.into_boxed();
                if more {
                    let Some(next) = page.checked_add(1) else {
                        pending.reviews_complete = false;
                        finish_step(d, env, pending, out);
                        return;
                    };
                    pending.stage = StepStage::Reviews(next);
                    let Some(pull) = &pending.pull else { unreachable!("review read has a pull") };
                    skein_forge::api::Read::Reviews { number: pull.number, page: next }
                } else {
                    finish_step(d, env, pending, out);
                    return;
                }
            }
            Ok(_) | Err(_) => {
                pending.reviews_complete = false;
                finish_step(d, env, pending, out);
                return;
            }
        },
    };
    d.steps.insert(owner, pending).expect("replaces step");
    child(d, env, skein_forge_client::Event::Read { owner, repository, read: next }, out);
}

pub(crate) fn queue_ready(d: &Domain, row: &ChangeRow, l: &Limits) -> (List<change::Ready>, Option<u64>) {
    let mut base_repair = None;
    for (_, repair) in &d.changes {
        if repair.repository == row.repository
            && repair.base == row.base
            && repair.base_repair
            && crate::panel::ready(repair).is_some()
        {
            base_repair = Some(repair.task);
            break;
        }
    }
    let mut ready = List::with_capacity(l.changes);
    for (_, other) in &d.changes {
        if other.repository != row.repository || other.base != row.base {
            continue;
        }
        let since = match other.change.state {
            change::State::Queued { ready_since, .. } | change::State::First { ready_since, .. } => Some(ready_since),
            change::State::Producing { .. }
            | change::State::Opening { .. }
            | change::State::Recreating { .. }
            | change::State::Reopening { .. }
            | change::State::Checking { .. }
            | change::State::Gating { .. }
            | change::State::Ready { .. }
            | change::State::Preparing { .. }
            | change::State::CancellingGates { .. }
            | change::State::Updating { .. }
            | change::State::Resolving { .. }
            | change::State::Repairing { .. }
            | change::State::Landing { .. }
            | change::State::Ended
            | change::State::Landed { .. }
            | change::State::Held { .. } => None,
        };
        if let Some(since) = since {
            ready
                .push(change::Ready { task: other.task, priority: other.priority, since })
                .expect("changes bound ready");
        }
    }
    (ready, base_repair)
}

fn queue_first(d: &Domain, row: &ChangeRow, now: skein_lib::Wall, l: &Limits) -> bool {
    let (ready, repair) = queue_ready(d, row, l);
    match repair {
        Some(task) => row.task == task,
        None => change::first(ready.as_slice(), l.queue_window, now) == Some(row.task),
    }
}

fn finish_step(d: &mut Domain, env: &Env<Limits>, pending: PendingStep, out: &mut Queue<Request>) {
    if pending.woken {
        step_change(
            d,
            env,
            StepInput {
                task: pending.task,
                entry: pending.entry,
                heard: pending.heard,
                gates: pending.gates,
                queue_repair_active: pending.queue_repair_active,
                kept: pending.kept,
            },
            out,
        );
    } else {
        decide_step(d, env, &pending, out);
    }
}

fn decide_step(d: &mut Domain, env: &Env<Limits>, pending: &PendingStep, out: &mut Queue<Request>) {
    let Some(mut row) = d.changes.get(&pending.task).cloned() else { return };
    let before = crate::panel::ready(&row);
    let prior = row.change.clone();
    let Some(base_tip) = pending.base_tip else {
        emit(out, Request::Refused { task: pending.task });
        return;
    };
    row.pull = match &pending.pull {
        Some(pull) => Some(pull.number),
        None => row.pull,
    };
    if let Some(pull) = &pending.pull {
        let changed = if pull.base != row.base {
            Some(crate::DriftChange::Retargeted {
                number: pull.number,
                before: row.base.clone(),
                after: pull.base.clone(),
            })
        } else if pull.state == skein_forge::api::State::Closed && pull.merged.is_none() {
            Some(crate::DriftChange::PullClosed { number: pull.number })
        } else {
            None
        };
        if let Some(change) = changed {
            let resource = skein_forge_client::Resource {
                repository: row.repository,
                what: skein_forge_client::What::Branch(row.branch.clone()),
            };
            if let Some(name) = from_client(&resource, &env.limits)
                && let Some(hold) = d.holds.get_mut(&name)
            {
                hold.drift = Some(crate::Drift { at: env.wall, change });
                save_hold(out, hold.clone());
            }
        }
    }
    crate::settle::observe(d, env, &row, pending.pull.as_ref(), pending.ci, base_tip);
    if row.change.released == Some(change::Hold::BranchMoved)
        && let Some(head) = pending.branch
    {
        accept_released_branch(d, env, &mut row, head, out);
    }
    let facts = change_facts(d, env, &row, pending, base_tip);
    let heard = change::Heard {
        effect: if row.pending.is_some() { change::EffectResult::Pending } else { row.effect },
        ..pending.heard
    };
    for _ in 0_u32..16 {
        let stepped = change::step(&row.change, &facts, &heard, env.wall, &env.limits.change_policy);
        let advanced = stepped.change != row.change;
        row.change = stepped.change;
        let decision = stepped.decision;
        match decision {
            change::Decision::None if advanced => {}
            change::Decision::Effect(effect) => {
                emit_change_effect(d, env, &mut row, pending, prior, effect, out);
                return;
            }
            decision @ (change::Decision::None
            | change::Decision::Wait { .. }
            | change::Decision::Delegate(_)
            | change::Decision::CancelGates { .. }
            | change::Decision::Ready
            | change::Decision::QueueRepair
            | change::Decision::Finish { .. }
            | change::Decision::Cancel
            | change::Decision::Hold(_)) => {
                crate::landing::change_advanced(d, &row, &prior, decision, out);
                d.changes.insert(row.task, row.clone()).expect("replaces change");
                if in_queue(&prior.state) && !in_queue(&row.change.state) {
                    wake_queue(d, &row);
                }
                save_queue_change(d, env, before, row.clone(), out);
                topics::head_changed(d, row.task, out);
                topics::refresh(d, env, row.task, out);
                emit(
                    out,
                    Request::ChangeDecision {
                        task: row.task,
                        decision: crate::settle::cap(d, env, &row, &facts, decision),
                        entry: None,
                        evidence: None,
                    },
                );
                return;
            }
        }
    }
    emit(out, Request::Refused { task: row.task });
}

fn in_queue(state: &change::State) -> bool {
    match state {
        change::State::Queued { .. } | change::State::First { .. } => true,
        change::State::Producing { .. }
        | change::State::Opening { .. }
        | change::State::Recreating { .. }
        | change::State::Reopening { .. }
        | change::State::Checking { .. }
        | change::State::Gating { .. }
        | change::State::Ready { .. }
        | change::State::Preparing { .. }
        | change::State::CancellingGates { .. }
        | change::State::Updating { .. }
        | change::State::Resolving { .. }
        | change::State::Repairing { .. }
        | change::State::Landing { .. }
        | change::State::Ended
        | change::State::Landed { .. }
        | change::State::Held { .. } => false,
    }
}

fn wake_queue(d: &mut Domain, row: &ChangeRow) {
    for (_, other) in &d.changes {
        if other.task != row.task
            && other.repository == row.repository
            && other.base == row.base
            && in_queue(&other.change.state)
        {
            d.procedure_wakes.insert(other.task, true).expect("one pending wake per change");
        }
    }
}

#[expect(clippy::manual_map, reason = "step code uses concrete matches, without callbacks")]
fn change_facts(
    d: &Domain,
    env: &Env<Limits>,
    row: &ChangeRow,
    pending: &PendingStep,
    base_tip: skein_forge::api::Commit,
) -> change::Facts {
    let pull = match &pending.pull {
        Some(pull) if pull.merged.is_some() => change::Pull::Merged { commit: pull.merged.expect("checked merged") },
        Some(pull) if pull.state == skein_forge::api::State::Closed => change::Pull::Closed,
        Some(pull) => change::Pull::Open { head: pull.commit, base: pull.base_commit.unwrap_or(base_tip) },
        None => change::Pull::Missing,
    };
    let retargeted = match &pending.pull {
        Some(pull) => pull.base != row.base,
        None => false,
    };
    let branch_name = skein_forge_client::Resource {
        repository: row.repository,
        what: skein_forge_client::What::Branch(row.branch.clone()),
    };
    let writer_taken = match from_client(&branch_name, &env.limits) {
        Some(name) => match d.holds.get(&name) {
            Some(hold) => hold.writer.is_some(),
            None => false,
        },
        None => false,
    };
    let may_merge = match d.repositories.get(&row.repository) {
        Some(repository) => repository.kinds.land,
        None => false,
    };
    let has_ci = match d.repositories.get(&row.repository) {
        Some(repository) => repository.ci,
        None => true,
    };
    let window = crate::settle::window(d, row.task);
    change::Facts {
        ask: crate::landing::ask_for(d, row),
        branch: pending.branch,
        expected_base: base_tip,
        pull,
        ci: change::Ci {
            head: pending.branch.unwrap_or([0; 32]),
            status: if has_ci { pending.ci } else { change::Status::Passed },
        },
        base_ci: change::Ci {
            head: base_tip,
            status: if !has_ci || row.base_repair { change::Status::Passed } else { pending.base_ci },
        },
        base_tip,
        contains_base: pending.contains_base,
        mergeable: match &pending.pull {
            Some(pull) if pull.mergeable => change::Status::Passed,
            Some(_) if window.is_some() => change::Status::Pending,
            Some(_) => change::Status::Failed,
            None => change::Status::Unknown,
        },
        gates: pending.gates.clone(),
        live_gate: match row.delegate {
            Some((_, change::Delegate::Gate { head, .. })) => Some(head),
            Some(_) | None => None,
        },
        writer_taken,
        drift: if retargeted { Some(change::Hold::Retargeted) } else { row.drift },
        first: queue_first(d, row, env.wall, &env.limits),
        may_merge,
        queue_repair_active: pending.queue_repair_active,
        kept: pending.kept,
        settling: match window {
            Some((_, end)) => Some(end),
            None => None,
        },
    }
}

fn emit_change_effect(
    d: &mut Domain,
    env: &Env<Limits>,
    row: &mut ChangeRow,
    pending: &PendingStep,
    prior: change::Change,
    effect: change::Effect,
    out: &mut Queue<Request>,
) {
    let Some(write) = change_write(row, effect, &env.limits) else {
        emit(
            out,
            Request::ChangeDecision {
                task: row.task,
                decision: change::Decision::Hold(change::Hold::Failed),
                entry: None,
                evidence: None,
            },
        );
        return;
    };
    if d.entries.contains_key(&pending.entry) || d.entries.len() == env.limits.entries {
        emit(out, Request::Refused { task: row.task });
        return;
    }
    let condition = match effect {
        change::Effect::Update { head, base } | change::Effect::Prepare { head, main: base } => {
            skein_forge_client::Condition::Update { head, base }
        }
        change::Effect::Merge { .. } => skein_forge_client::Condition::Merge { base: row.base.clone() },
        // landing.md 3: the top's task
        change::Effect::Open
        | change::Effect::CreateBranch { .. }
        | change::Effect::Reopen
        | change::Effect::Retarget => skein_forge_client::Condition::None,
    };
    let entry = skein_forge_client::Entry {
        number: pending.entry,
        task: row.task,
        repository: row.repository,
        effect: skein_forge_client::Effect { write, condition },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    crate::landing::change_advanced(d, row, &prior, change::Decision::Effect(effect), out);
    row.pending = Some(pending.entry);
    row.effect = change::EffectResult::Pending;
    let before = match d.changes.get(&row.task) {
        Some(row) => crate::panel::ready(row),
        None => None,
    };
    d.changes.insert(row.task, row.clone()).expect("replaces change");
    save_queue_change(d, env, before, row.clone(), out);
    keep_entry(d, entry, None, out);
    emit(
        out,
        Request::ChangeDecision {
            task: row.task,
            decision: change::Decision::Effect(effect),
            entry: Some(pending.entry),
            evidence: Some(crate::ChangeEvidence {
                prior,
                head: pending.branch,
                base_tip: pending.base_tip.expect("step read base tip"),
                contains_base: pending.contains_base,
                ci: if match d.repositories.get(&row.repository) {
                    Some(repository) => repository.ci,
                    None => true,
                } {
                    pending.ci
                } else {
                    change::Status::Passed
                },
                gates: pending.gates.clone(),
                reviews: pending.reviews.clone(),
                reviews_complete: pending.reviews_complete,
            }),
        },
    );
}

fn change_write(row: &ChangeRow, effect: change::Effect, limits: &Limits) -> Option<skein_forge::api::Write> {
    match effect {
        change::Effect::Open => {
            // domain/forge.md, 8.1: retain the spec; fit only its forge projection.
            let room = usize::try_from(limits.client.op_bytes)
                .expect("u32 fits usize")
                .saturating_sub(row.branch.len())
                .saturating_sub(row.base.len());
            let mut end = row.title.len();
            for (index, byte) in row.title.iter().enumerate() {
                if *byte == b'\n' {
                    end = index;
                    break;
                }
            }
            let first = row.title.get(..end).expect("first line");
            let first = first.strip_suffix(b"\r").unwrap_or(first);
            let marker = "…\nThe change's full words are on its task in temper.".as_bytes();
            let title_room = room.saturating_sub(marker.len().min(row.body.len()));
            let title = crate::text::fit(first, title_room.min(256), "…".as_bytes());
            let body = crate::text::fit(&row.body, room.saturating_sub(title.len()), marker);
            Some(skein_forge::api::Write::OpenPull { title, body, head: row.branch.clone(), base: row.base.clone() })
        }
        change::Effect::CreateBranch { head } => {
            Some(skein_forge::api::Write::CreateBranch { branch: row.branch.clone(), commit: head })
        }
        change::Effect::Reopen => Some(skein_forge::api::Write::Reopen { number: row.pull? }),
        change::Effect::Prepare { .. } => {
            Some(skein_forge::api::Write::Update { number: row.pull?, style: skein_forge::api::UpdateStyle::Rebase })
        }
        change::Effect::Retarget => None,
        change::Effect::Update { .. } => {
            Some(skein_forge::api::Write::Update { number: row.pull?, style: skein_forge::api::UpdateStyle::Merge })
        }
        change::Effect::Merge { head, .. } => {
            Some(skein_forge::api::Write::Merge { number: row.pull?, head, how: skein_forge::api::MergeHow::Style })
        }
    }
}

fn change_outcome(
    d: &mut Domain,
    env: &Env<Limits>,
    entry: u64,
    outcome: skein_forge_client::Outcome,
    out: &mut Queue<Request>,
) {
    let mut task = None;
    for (number, row) in &d.changes {
        if row.pending == Some(entry) {
            task = Some(*number);
        }
    }
    let Some(task) = task else { return };
    let Some(row) = d.changes.get_mut(&task) else { unreachable!("found change row") };
    let pull_closed = match outcome {
        skein_forge_client::Outcome::Raced {
            made: skein_forge_client::Made::Created(_),
            why: skein_forge::api::Error::Closed,
        } => true,
        skein_forge_client::Outcome::Made { .. }
        | skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Uncertain
        | skein_forge_client::Outcome::Queued
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => false,
    };
    row.effect = match outcome {
        skein_forge_client::Outcome::Made { .. } => change::EffectResult::Made,
        skein_forge_client::Outcome::Failed(skein_forge::api::Error::Conflict) => change::EffectResult::Conflict,
        skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => change::EffectResult::Failed,
        skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => return,
    };
    row.pending = None;
    if pull_closed {
        row.drift = Some(change::Hold::PullClosed);
    }
    let saved = row.clone();
    save_change(d, out, saved.clone());
    if pull_closed {
        let branch = skein_forge_client::Resource {
            repository: saved.repository,
            what: skein_forge_client::What::Branch(saved.branch.clone()),
        };
        if from_client(&branch, &env.limits).is_some() {
            drift(
                d,
                env,
                branch,
                crate::DriftChange::PullClosed { number: saved.pull.expect("raced pull is known") },
                out,
            );
        }
    }
    match outcome {
        skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Updated(commit), .. } => {
            child(
                d,
                env,
                skein_forge_client::Event::Pushed {
                    resource: skein_forge_client::Resource {
                        repository: saved.repository,
                        what: skein_forge_client::What::Branch(saved.branch),
                    },
                    commit,
                },
                out,
            );
        }
        skein_forge_client::Outcome::Made {
            made:
                skein_forge_client::Made::Created(_)
                | skein_forge_client::Made::Commented(_)
                | skein_forge_client::Made::Reviewed(_)
                | skein_forge_client::Made::Merged(_)
                | skein_forge_client::Made::Branch(_)
                | skein_forge_client::Made::Set,
            ..
        }
        | skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Uncertain
        | skein_forge_client::Outcome::Queued
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => {}
    }
    emit(out, Request::ChangeDecision { task, decision: change::Decision::None, entry: None, evidence: None });
}

/// A release adopts an observed head, including the client's drift baseline.
fn accept_released_branch(
    d: &mut Domain,
    env: &Env<Limits>,
    row: &mut ChangeRow,
    head: skein_forge::api::Commit,
    out: &mut Queue<Request>,
) {
    row.drift = None;
    let resource = skein_forge_client::Resource {
        repository: row.repository,
        what: skein_forge_client::What::Branch(row.branch.clone()),
    };
    let Some(name) = from_client(&resource, &env.limits) else { return };
    if let Some(hold) = d.holds.get_mut(&name) {
        hold.drift = None;
        save_hold(out, hold.clone());
    }
    let head = BranchHead { name: name.clone(), commit: head, owned: true };
    d.heads.insert(name, head.clone()).expect("the held branch has a head slot");
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::BranchHead(head.clone()) });
    child(d, env, skein_forge_client::Event::Pushed { resource, commit: head.commit }, out);
}

fn release_change(d: &mut Domain, env: &Env<Limits>, task: u64, out: &mut Queue<Request>) {
    let Some(row) = d.changes.get_mut(&task) else {
        emit(out, Request::Refused { task });
        return;
    };
    let before = crate::panel::ready(row);
    row.change = change::released(&row.change, env.wall);
    row.drift = None;
    let mut saved = row.clone();
    save_queue_change(d, env, before, saved.clone(), out);
    let mut affected = List::with_capacity(d.holds.len());
    let mut observed = None;
    for (name, hold) in &d.holds {
        if hold.task == task && hold.drift.is_some() {
            affected.push(name.clone()).expect("at most all held resources");
            if let Some(drift) = &hold.drift {
                match &drift.change {
                    crate::DriftChange::Branch { observed: head, .. } => observed = *head,
                    crate::DriftChange::PullClosed { .. } | crate::DriftChange::Retargeted { .. } => {}
                }
            }
        }
    }
    for name in &affected {
        let hold = d.holds.get_mut(name).expect("collected held resource");
        hold.drift = None;
        save_hold(out, hold.clone());
    }
    if saved.change.released == Some(change::Hold::BranchMoved)
        && let Some(head) = observed
    {
        // A poll may finish before the procedure's fresh snapshot does.
        accept_released_branch(d, env, &mut saved, head, out);
    }
}

fn settle_effects(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    root: u64,
    ending: ReleaseEnding,
    out: &mut Queue<Request>,
) {
    crate::landing::closing(d, task, ending, out);
    if let Some(row) = d.releases.get_mut(&task) {
        if row.root != root || (row.ending != ending && ending != ReleaseEnding::Cancelled) {
            emit(out, Request::ReleaseFailed { task });
            return;
        }
        if row.ending != ending {
            row.ending = ending;
            row.releasing = false;
            row.close_pull = match d.changes.get(&task) {
                Some(change) => change.pull,
                None => None,
            };
            emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row.clone()) });
        }
        if row.effects_settled {
            emit(out, Request::EffectsSettled { task });
            return;
        }
    } else {
        if d.releases.len() == env.limits.tasks {
            emit(out, Request::ReleaseFailed { task });
            return;
        }
        let row = ReleaseRow {
            task,
            root,
            ending,
            close_pull: if ending == ReleaseEnding::Cancelled {
                match d.changes.get(&task) {
                    Some(change) => change.pull,
                    None => None,
                }
            } else {
                None
            },
            comment: None,
            pending: None,
            failed: false,
            effects_settled: false,
            releasing: false,
        };
        d.releases.insert(task, row.clone()).expect("release capacity checked");
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row) });
    }
    if ending == ReleaseEnding::Cancelled {
        let mut unsent = List::with_capacity(env.limits.entries);
        for (number, row) in &d.entries {
            if row.entry.task == task && !row.held && row.entry.attempt.is_none() {
                unsent.push(*number).expect("entry table bounded");
            }
        }
        for number in unsent.into_boxed() {
            withdraw(d, env, number, out);
            if let Some(row) = d.entries.get(&number)
                && row.entry.start.is_none()
            {
                d.entries.remove(&number);
                emit(out, Request::Erase { key: Key::Entry(number) });
                d.named_outcome(number, skein_forge_client::Outcome::Withdrawn);
                emit(out, Request::Outcome { entry: number, task, outcome: skein_forge_client::Outcome::Withdrawn });
            }
        }
    }
    settle_prior_effects(d, task, out);
}

fn settle_prior_effects(d: &mut Domain, task: u64, out: &mut Queue<Request>) {
    if let Some(issue) = d.issues.get(&task)
        && let Some(desired) = &issue.desired
        && desired.finished.is_some()
        && !issue.state.closed
    {
        return;
    }
    if !settle_held(d, task, out) {
        return;
    }
    let row = d.releases.get_mut(&task).expect("effect settlement row");
    if !row.effects_settled {
        row.effects_settled = true;
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row.clone()) });
    }
    emit(out, Request::EffectsSettled { task });
}

#[expect(clippy::too_many_arguments, reason = "cleanup receives the closing task and its bounded cancel reason")]
fn release(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    root: u64,
    ending: ReleaseEnding,
    reason: Option<Box<[u8]>>,
    entry: u64,
    out: &mut Queue<Request>,
) {
    if let Some(reason) = &reason
        && reason.len() > usize::try_from(env.limits.client.op_bytes).expect("u32 fits usize")
    {
        emit(out, Request::ReleaseFailed { task });
        return;
    }
    if let Some(row) = d.releases.get_mut(&task) {
        if row.root != root || (row.ending != ending && ending != ReleaseEnding::Cancelled) {
            emit(out, Request::ReleaseFailed { task });
            return;
        }
        if row.ending != ending {
            row.ending = ending;
            row.releasing = false;
            row.close_pull = match d.changes.get(&task) {
                Some(change) => change.pull,
                None => None,
            };
        }
        if row.failed {
            row.failed = false;
        }
        if !row.releasing && ending == ReleaseEnding::Cancelled && row.close_pull.is_some() {
            row.comment = reason;
        }
        row.releasing = true;
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row.clone()) });
    } else {
        if d.releases.len() == env.limits.tasks {
            emit(out, Request::ReleaseFailed { task });
            return;
        }
        let mut row = ReleaseRow {
            task,
            root,
            ending,
            close_pull: if ending == ReleaseEnding::Cancelled {
                match d.changes.get(&task) {
                    Some(change) => change.pull,
                    None => None,
                }
            } else {
                None
            },
            comment: None,
            pending: None,
            failed: false,
            effects_settled: true,
            releasing: true,
        };
        if ending == ReleaseEnding::Cancelled && row.close_pull.is_some() {
            row.comment = reason;
        }
        d.releases.insert(task, row.clone()).expect("release capacity checked");
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row) });
    }
    continue_release(d, env, task, entry, out);
}

fn continue_release(d: &mut Domain, env: &Env<Limits>, task: u64, entry: u64, out: &mut Queue<Request>) {
    for _ in 0..=env.limits.holds {
        let Some(mut row) = d.releases.get(&task).cloned() else { return };
        if row.failed || row.pending.is_some() {
            return;
        }
        if !row.effects_settled {
            settle_prior_effects(d, task, out);
            return;
        }
        if !row.releasing {
            return;
        }
        if !settle_held(d, task, out) {
            return;
        }
        if let Some(number) = row.close_pull {
            let Some(change) = d.changes.get(&task) else {
                emit(out, Request::ReleaseFailed { task });
                return;
            };
            let repository = change.repository;
            let write = release_pull_write(&row, &d.namespace, number, &env.limits);
            enqueue_release(d, env, &mut row, entry, repository, write, None, out);
            return;
        }
        let mut selected = None;
        for (name, hold) in &d.holds {
            if hold.task == task {
                selected = Some((name.clone(), hold.writer));
                break;
            }
        }
        if let Some((name, writer)) = selected {
            if d.forge_ended(name.forge) {
                d.holds.remove(&name);
                emit(out, Request::Erase { key: Key::Hold(name) });
                continue;
            }
            if let Some(Writer::Lost { task: lost_task, attempt }) = writer {
                for (_, pending) in &d.lost {
                    if pending.name == name && pending.task == lost_task && pending.attempt == attempt {
                        return;
                    }
                }
            }
            if writer.is_some() {
                emit(out, Request::ReleaseFailed { task });
                return;
            }
            if row.ending == ReleaseEnding::Failed && row.root != task {
                let hold = d.holds.get_mut(&name).expect("selected hold");
                hold.task = row.root;
                save_hold(out, hold.clone());
                emit(out, Request::Retained { task, root: row.root, resource: name });
                continue;
            }
            if let Some(resource) = to_client(&name, &env.limits) {
                match resource.what {
                    skein_forge_client::What::Branch(branch) => {
                        enqueue_release(
                            d,
                            env,
                            &mut row,
                            entry,
                            resource.repository,
                            skein_forge::api::Write::DeleteBranch { branch },
                            Some(name),
                            out,
                        );
                        return;
                    }
                    skein_forge_client::What::Repository
                    | skein_forge_client::What::Pull(_)
                    | skein_forge_client::What::Issue(_) => {}
                }
            }
            d.holds.remove(&name);
            emit(out, Request::Erase { key: Key::Hold(name) });
            continue;
        }
        let mut topics = List::with_capacity(env.limits.subscriptions);
        for ((owner, topic), _) in &d.subscriptions {
            if *owner == task {
                topics.push(topic.clone()).expect("subscription capacity");
            }
        }
        for topic in &topics {
            unsubscribe(d, task, topic.clone(), out);
        }
        unnamed(d, env, task, out);
        end_change(d, env, task, out);
        d.releases.remove(&task);
        emit(out, Request::Erase { key: Key::Release(task) });
        emit(out, Request::Released { task });
        return;
    }
    unreachable!("every release pass removes a hold or completes");
}

/// The panel outlives its task, but never its task's live state (domain/forge.md, 8.3).
fn end_change(d: &mut Domain, env: &Env<Limits>, task: u64, out: &mut Queue<Request>) {
    let Some(row) = d.changes.get_mut(&task) else { return };
    let before = crate::panel::ready(row);
    let mut merge = None;
    for (commit, owner) in &d.landed {
        if *owner == task {
            merge = Some(*commit);
        }
    }
    row.change.state = match &row.change.state {
        change::State::Landed { merge } => change::State::Landed { merge: *merge },
        change::State::Producing { .. }
        | change::State::Opening { .. }
        | change::State::Recreating { .. }
        | change::State::Reopening { .. }
        | change::State::Checking { .. }
        | change::State::Gating { .. }
        | change::State::CancellingGates { .. }
        | change::State::Queued { .. }
        | change::State::First { .. }
        | change::State::Updating { .. }
        | change::State::Resolving { .. }
        | change::State::Repairing { .. }
        | change::State::Landing { .. }
        | change::State::Ended
        | change::State::Held { .. } => match merge {
            Some(merge) => change::State::Landed { merge },
            None => change::State::Ended,
        },
    };
    row.change.released = None;
    row.change.owns_turn = false;
    row.delegate = None;
    row.drift = None;
    let saved = row.clone();
    save_queue_change(d, env, before, saved, out);
}

fn release_pull_write(row: &ReleaseRow, namespace: &[u8], number: u64, limits: &Limits) -> skein_forge::api::Write {
    match &row.comment {
        Some(body) => {
            let key =
                crate::purpose::EffectPurpose::Release { task: row.task, resource: Box::from(b"reason".as_slice()) }
                    .key(namespace, limits.client.op_bytes)
                    .expect("release key fits the checked operation bound");
            skein_forge::api::Write::Post { number, key, body: body.clone() }
        }
        None => skein_forge::api::Write::Close { number },
    }
}

#[expect(clippy::too_many_arguments, reason = "one cleanup effect carries its durable entry and held resource")]
fn enqueue_release(
    d: &mut Domain,
    env: &Env<Limits>,
    row: &mut ReleaseRow,
    number: u64,
    repository: skein_forge::api::Repository,
    write: skein_forge::api::Write,
    resource: Option<Name>,
    out: &mut Queue<Request>,
) {
    if d.entries.len() == env.limits.entries || d.entries.contains_key(&number) {
        emit(out, Request::ReleaseFailed { task: row.task });
        return;
    }
    let entry = skein_forge_client::Entry {
        number,
        task: row.task,
        repository,
        effect: skein_forge_client::Effect { write, condition: skein_forge_client::Condition::None },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    row.pending = Some((number, resource));
    d.releases.insert(row.task, row.clone()).expect("replaces release");
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row.clone()) });
    keep_entry(d, entry, Some(number), out);
}

fn release_outcome(d: &mut Domain, entry: u64, outcome: skein_forge_client::Outcome, out: &mut Queue<Request>) {
    if outcome == skein_forge_client::Outcome::Uncertain || outcome == skein_forge_client::Outcome::Queued {
        return;
    }
    let mut owner = None;
    for (task, row) in &d.releases {
        if match &row.pending {
            Some((number, _)) => *number == entry,
            None => false,
        } {
            owner = Some(*task);
            break;
        }
    }
    let Some(task) = owner else { return };
    let mut row = d.releases.get(&task).expect("selected release").clone();
    let (_, resource) = row.pending.take().expect("selected pending release");
    if resource.is_none() && row.comment.is_some() {
        // A permanent post failure must not prevent the pull from closing.
        // Uncertain/queued outcomes returned above keep the reason and keyed entry.
        row.comment = None;
    } else {
        match outcome {
            skein_forge_client::Outcome::Made { .. } => match resource {
                Some(name) => {
                    d.holds.remove(&name);
                    emit(out, Request::Erase { key: Key::Hold(name) });
                }
                None => row.close_pull = None,
            },
            skein_forge_client::Outcome::Failed(_)
            | skein_forge_client::Outcome::Raced { .. }
            | skein_forge_client::Outcome::Held
            | skein_forge_client::Outcome::Withdrawn => {
                row.failed = true;
                emit(out, Request::ReleaseFailed { task });
            }
            skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => {
                unreachable!("uncertain returned above")
            }
        }
    }
    d.releases.insert(task, row.clone()).expect("replaces release");
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Release(row) });
}

#[expect(
    clippy::too_many_lines,
    reason = "one projection admission saves facts, waits, or describes its next keyed effect"
)]
fn project(
    d: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    repository: skein_forge::api::Repository,
    view: issues::GoalView,
    out: &mut Queue<Request>,
) {
    let goal = view.goal;
    let Some(adopted) = d.repositories.get(&repository) else {
        fail_projection(d, env, goal, Some(repository), view.finished.is_some(), out);
        return;
    };
    if view.repository != u64::from(repository.repository)
        || adopted.role == Role::Context
        || !adopted.kinds.issue
        || (!d.issues.contains_key(&goal) && d.issues.len() == env.limits.issues)
    {
        fail_projection(d, env, goal, Some(repository), view.finished.is_some(), out);
        return;
    }
    let mut prior = match d.issues.get(&goal) {
        Some(row) if row.repository != repository => {
            fail_projection(d, env, goal, Some(repository), view.finished.is_some(), out);
            return;
        }
        Some(row) => row.clone(),
        None => IssueRow {
            goal,
            repository,
            state: issues::Projected::default(),
            number: None,
            pending: None,
            before: None,
            failed: false,
            desired: None,
        },
    };
    let Some(policy) = projection_limits(d, &env.limits, &prior, &view) else {
        fail_projection(d, env, goal, Some(repository), view.finished.is_some(), out);
        return;
    };
    let closing = view.finished.is_some();
    let Some(view) = issues::gather(prior.desired.as_ref(), view, &policy) else {
        fail_projection(d, env, goal, Some(repository), closing, out);
        return;
    };
    let changed = prior.desired.as_ref() != Some(&view);
    if changed {
        prior.desired = Some(view.clone());
        d.issues.insert(goal, prior.clone()).expect("projection room checked");
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Issue(prior.clone()) });
    }
    if prior.failed {
        // An Open lost to a crash consumes its committed open bit without
        // yielding an issue number. It can never be sent again, even after
        // its held outbox entry has settled (domain/forge.md, 6 and 12).
        if view.finished.is_some() || (prior.state.opened && prior.number.is_none()) {
            settled_projection(d, goal, out);
            return;
        }
        if !changed
            && let Some(when) = d.projection_due.get(&goal)
            && *when > env.wall
        {
            emit(out, Request::ProjectAfter { goal, when: Some(*when) });
            return;
        }
    }
    if prior.pending.is_some() {
        emit(out, Request::ProjectAfter { goal, when: None });
        return;
    }
    let projected = issues::project(&view, &prior.state, env.wall, &policy);
    match projected.decision {
        issues::Decision::None | issues::Decision::Wait(_) => match projected.decision {
            issues::Decision::Wait(when) => emit(out, Request::ProjectAfter { goal, when: Some(when) }),
            issues::Decision::None => settled_projection(d, goal, out),
            issues::Decision::Hold | issues::Decision::Effect(_) => unreachable!("matched above"),
        },
        issues::Decision::Hold => fail_projection(d, env, goal, Some(repository), closing, out),
        issues::Decision::Effect(effect) => {
            if d.entries.len() == env.limits.entries || d.entries.contains_key(&number) {
                emit(out, Request::ProjectAfter { goal, when: None });
                return;
            }
            let Some(write) = project_write(d, &prior, effect, &env.limits) else {
                fail_projection(d, env, goal, Some(repository), closing, out);
                return;
            };
            let entry = skein_forge_client::Entry {
                number,
                task: goal,
                repository,
                effect: skein_forge_client::Effect { write, condition: skein_forge_client::Condition::None },
                start: None,
                attempt: None,
                failures: 0,
                queued: false,
            };
            let row = IssueRow {
                goal,
                repository,
                state: projected.projected,
                number: prior.number,
                pending: Some(number),
                before: Some(prior.state),
                failed: false,
                desired: Some(view),
            };
            let Some(description) = crate::describe_projection_effect(adopted, &entry.effect.write) else {
                fail_projection(d, env, goal, Some(repository), closing, out);
                return;
            };
            emit(out, Request::ProjectionEffect { row, entry, description });
        }
    }
}

fn projection_limits(d: &Domain, limits: &Limits, prior: &IssueRow, view: &issues::GoalView) -> Option<issues::Limits> {
    let title_bound = usize::try_from(limits.issue_policy.title_bytes).expect("u32 fits usize");
    let title_bytes = if view.title.len() <= title_bound {
        view.title.len()
    } else {
        let mut end = title_bound.saturating_sub("…".len());
        while !view.title.is_char_boundary(end) {
            end = end.checked_sub(1).expect("UTF-8 title boundary");
        }
        end.checked_add("…".len().min(title_bound)).expect("bounded title")
    };
    // A create retains its purpose key too; an edit carries only title and body.
    let key_bytes = if prior.state.opened {
        0
    } else {
        let key = projection_key(&d.namespace, view.goal, issues::Key::Open, limits)?;
        u32::try_from(key.len()).expect("bounded key")
    };
    Some(issues::Limits {
        body_bytes: limits.issue_policy.body_bytes.min(
            limits
                .client
                .op_bytes
                .saturating_sub(key_bytes)
                .saturating_sub(u32::try_from(title_bytes).expect("bounded title")),
        ),
        ..limits.issue_policy
    })
}

/// A projection's failed row and timer are independent of the task's lifecycle.
fn fail_projection(
    d: &mut Domain,
    env: &Env<Limits>,
    goal: u64,
    repository: Option<skein_forge::api::Repository>,
    closing: bool,
    out: &mut Queue<Request>,
) {
    if !d.issues.contains_key(&goal)
        && d.issues.len() < env.limits.issues
        && let Some(repository) = repository
        && d.repositories.contains_key(&repository)
    {
        d.issues
            .insert(
                goal,
                IssueRow {
                    goal,
                    repository,
                    state: issues::Projected::default(),
                    number: None,
                    pending: None,
                    before: None,
                    failed: true,
                    desired: None,
                },
            )
            .expect("projection room checked");
    }
    if let Some(row) = d.issues.get_mut(&goal) {
        row.failed = true;
        emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Issue(row.clone()) });
    }
    emit(out, Request::ProjectionFailed { goal });
    if closing {
        emit(out, Request::ProjectionSettled { goal });
    } else if d.issues.contains_key(&goal) {
        let when = skein_lib::Wall::from_nanos(
            env.wall.as_nanos().saturating_add(env.limits.issue_policy.interval.as_nanos()),
        );
        d.projection_due.insert(goal, when).expect("one deadline per issue");
        emit(out, Request::ProjectAfter { goal, when: Some(when) });
    }
}

fn project_write(d: &Domain, row: &IssueRow, effect: issues::Effect, l: &Limits) -> Option<skein_forge::api::Write> {
    match effect {
        issues::Effect::Open { goal, key, title, body, .. } => {
            let key = projection_key(&d.namespace, goal, key, l)?;
            Some(skein_forge::api::Write::CreateIssue { key, title: Box::from(title.as_bytes()), body })
        }
        issues::Effect::Body { title, body, .. } => Some(skein_forge::api::Write::Edit {
            number: row.number?,
            title: Some(Box::from(title.as_bytes())),
            body: Some(body),
        }),
        issues::Effect::Comment { goal, key, body, .. } => {
            let key = projection_key(&d.namespace, goal, key, l)?;
            Some(skein_forge::api::Write::Post { number: row.number?, key, body: Box::from(body.as_bytes()) })
        }
        issues::Effect::Close { .. } => Some(skein_forge::api::Write::Close { number: row.number? }),
    }
}

fn projection_key(namespace: &[u8], goal: u64, key: issues::Key, l: &Limits) -> Option<Box<[u8]>> {
    let (part, number) = match key {
        issues::Key::Open => (1, 0),
        issues::Key::Body(revision) => (2, revision),
        issues::Key::Milestone(milestone) => match milestone {
            issues::MilestoneKey::Lifecycle { task, position } => {
                return crate::purpose::EffectPurpose::ProjectionHistory { goal, task, position, family: 1 }
                    .key(namespace, l.client.op_bytes);
            }
            issues::MilestoneKey::TaskRevision { task, revision } => {
                return crate::purpose::EffectPurpose::ProjectionHistory { goal, task, position: revision, family: 2 }
                    .key(namespace, l.client.op_bytes);
            }
            issues::MilestoneKey::PlanAccepted => (3, 0),
            issues::MilestoneKey::Revision(number) => (4, number),
            issues::MilestoneKey::ChangeLanded(number) => (5, number),
            issues::MilestoneKey::ChangeHeld(number) => (6, number),
            issues::MilestoneKey::Report(number) => (7, number),
            issues::MilestoneKey::Finished => (8, 0),
        },
        issues::Key::Close => (9, 0),
    };
    crate::purpose::EffectPurpose::Projection { goal, part, number }.key(namespace, l.client.op_bytes)
}

fn projection_outcome(
    d: &mut Domain,
    env: &Env<Limits>,
    entry: u64,
    outcome: skein_forge_client::Outcome,
    out: &mut Queue<Request>,
) {
    let lost_comment = outcome == skein_forge_client::Outcome::Held && posting(d, entry);
    let mut goal = None;
    for (number, row) in &d.issues {
        if row.pending == Some(entry) {
            goal = Some(*number);
        }
    }
    let Some(goal) = goal else { return };
    let Some(row) = d.issues.get_mut(&goal) else { unreachable!("found issue row") };
    match outcome {
        skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => return,
        // The projected milestone prefix already includes this comment. A
        // crash-lost creation must never be resent (domain/forge.md, 6 and 12).
        skein_forge_client::Outcome::Held if lost_comment => {}
        skein_forge_client::Outcome::Held if row.number.is_none() => {
            // Keep the consumed Open bit so future feeds cannot create again.
            row.failed = true;
        }
        skein_forge_client::Outcome::Made { made, .. } => {
            if row.number.is_none() {
                match made {
                    skein_forge_client::Made::Created(number) => row.number = Some(number),
                    skein_forge_client::Made::Commented(_)
                    | skein_forge_client::Made::Reviewed(_)
                    | skein_forge_client::Made::Merged(_)
                    | skein_forge_client::Made::Updated(_)
                    | skein_forge_client::Made::Branch(_)
                    | skein_forge_client::Made::Set => row.failed = true,
                }
            }
        }
        skein_forge_client::Outcome::Failed(_)
        | skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => {
            if let Some(before) = row.before.take() {
                row.state = before;
            }
            row.failed = true;
        }
    }
    row.pending = None;
    row.before = None;
    let saved = row.clone();
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Issue(saved) });
    if row.failed {
        emit(out, Request::ProjectionFailed { goal });
        if match &row.desired {
            Some(view) => view.finished.is_some(),
            None => false,
        } {
            settled_projection(d, goal, out);
        } else {
            let when = skein_lib::Wall::from_nanos(
                env.wall.as_nanos().saturating_add(env.limits.issue_policy.interval.as_nanos()),
            );
            d.projection_due.insert(goal, when).expect("one deadline per issue");
            emit(out, Request::ProjectAfter { goal, when: Some(when) });
        }
    } else {
        emit(out, Request::ProjectAfter { goal, when: None });
    }
}

fn posting(d: &Domain, entry: u64) -> bool {
    match d.entries.get(&entry) {
        Some(row) => match &row.entry.effect.write {
            skein_forge::api::Write::Post { .. } => true,
            skein_forge::api::Write::CreateIssue { .. }
            | skein_forge::api::Write::OpenPull { .. }
            | skein_forge::api::Write::Review { .. }
            | skein_forge::api::Write::SetReviewers { .. }
            | skein_forge::api::Write::Edit { .. }
            | skein_forge::api::Write::Close { .. }
            | skein_forge::api::Write::Reopen { .. }
            | skein_forge::api::Write::Merge { .. }
            | skein_forge::api::Write::Update { .. }
            | skein_forge::api::Write::CreateBranch { .. }
            | skein_forge::api::Write::DeleteBranch { .. }
            | skein_forge::api::Write::Status { .. } => false,
        },
        None => false,
    }
}

fn release_projection(d: &mut Domain, goal: u64, out: &mut Queue<Request>) {
    let Some(row) = d.issues.get_mut(&goal) else { return };
    if row.pending.is_some() {
        return;
    }
    row.failed = false;
    let saved = row.clone();
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Issue(saved) });
    emit(out, Request::ProjectAfter { goal, when: None });
}

fn committed(d: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    if d.stopping {
        return;
    }
    if let Some(row) = d.entries.get_mut(&number)
        && row.handoff == Handoff::Uncommitted
    {
        row.handoff = Handoff::Ready;
    }
    handoff_one(d, env, out);
}

fn handoff_one(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if d.stopping || d.restore_phase == RestorePhase::Restoring {
        return;
    }
    // skein's forge-client.md, 7.2: each new Make is above every held entry.
    // Fresh-read chains retain their reserved numbers; built changes,
    // projections and releases share this outbox and wait for lower chains
    // and entries to finish or commit. Hand over at most one per step.
    let Some(number) = d.next_handoff() else { return };
    let row = d.entries.get_mut(&number).expect("selected entry remains");
    row.handoff = Handoff::Handed;
    let entry = row.entry.clone();
    child(d, env, skein_forge_client::Event::Make { entry }, out);
}

/// A person's choice for a held entry: again hands it back to the client,
/// which looks for it first and then makes it under a new attempt; the
/// entry kept its room, so the client has room for it.
fn resolve(d: &mut Domain, env: &Env<Limits>, number: u64, again: bool, out: &mut Queue<Request>) {
    let Some(row) = d.entries.get_mut(&number) else { return };
    if !row.held {
        return;
    }
    if !again {
        not_made(d, number, out);
        return;
    }
    row.held = false;
    let entry = row.entry.clone();
    emit(
        out,
        Request::Save { read_afresh: None, release: None, record: Stored::Entry { entry: entry.clone(), held: false } },
    );
    child(d, env, skein_forge_client::Event::Remake { entry }, out);
}

fn erase_flight(d: &mut Domain, task: u64, entry: u64, out: &mut Queue<Request>) {
    if d.flights.remove(&(task, entry)).is_some() {
        emit(out, Request::Erase { key: Key::Flight { task, entry } });
    }
}

/// Settle one held entry as not made, freeing its room.
fn not_made(d: &mut Domain, number: u64, out: &mut Queue<Request>) {
    let row = d.entries.remove(&number).expect("a held entry lives");
    emit(out, Request::Erase { key: Key::Entry(number) });
    erase_flight(d, row.entry.task, number, out);
    emit(out, Request::NotMade { entry: number, task: row.entry.task });
}

/// A closing task's held entries are settled not made, as many as the
/// output has room for, so its effects settle and their room is freed. True
/// when none of the task's entries remains; one settled here continues the
/// close in a later step.
fn settle_held(d: &mut Domain, task: u64, out: &mut Queue<Request>) -> bool {
    let mut held = List::with_capacity(d.entries.len());
    let mut pending = false;
    for (number, row) in &d.entries {
        if row.entry.task != task {
            continue;
        }
        if row.held {
            held.push(*number).expect("held entries are bounded by the outbox");
        } else {
            pending = true;
        }
    }
    if held.is_empty() {
        return !pending;
    }
    for number in &held {
        // Both possible erases, settlement, and the continuation below.
        if out.room() < 4 {
            break;
        }
        not_made(d, *number, out);
    }
    emit(out, Request::ContinueRelease { task });
    false
}

fn withdraw(d: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    if let Some(row) = d.entries.get(&number)
        && !row.held
    {
        if row.handoff != Handoff::Handed && row.entry.attempt.is_none() {
            d.client_out.push(skein_forge_client::Request::Outcome {
                entry: number,
                task: row.entry.task,
                outcome: skein_forge_client::Outcome::Withdrawn,
            });
            drain(d, env, out);
        } else {
            child(d, env, skein_forge_client::Event::Withdraw { entry: number }, out);
        }
    }
}

fn veto_change(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    entry: u64,
    prior: change::Change,
    out: &mut Queue<Request>,
) {
    let Some(row) = d.changes.get_mut(&task) else { return };
    if row.pending != Some(entry) || !d.entries.contains_key(&entry) {
        return;
    }
    let before = crate::panel::ready(row);
    row.change = prior;
    row.pending = None;
    row.effect = change::EffectResult::None;
    let saved = row.clone();
    save_queue_change(d, env, before, saved, out);
    d.entries.remove(&entry);
    emit(out, Request::Erase { key: Key::Entry(entry) });
}

fn subscribe(d: &mut Domain, env: &Env<Limits>, subscription: Subscriber, out: &mut Queue<Request>) {
    let task = subscription.task;
    let ci = match &subscription.topic {
        Topic::Ci { repository, head } => Some((*repository, *head)),
        Topic::Landings { .. } | Topic::Pull { .. } | Topic::Participation { .. } => None,
    };
    if !d.can_subscribe(&env.limits, &subscription) {
        emit(out, Request::Refused { task });
        return;
    }
    let key = (task, subscription.topic.clone());
    d.subscriptions.insert(key, subscription.clone()).expect("preflighted subscription capacity");
    topics::subscribe(d, &subscription, out);
    let tasks = subscription.goal_tasks.clone();
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Subscription(subscription) });
    if let Some(tasks) = tasks {
        for task in tasks {
            topics::refresh(d, env, task, out);
        }
    }
    if let Some((repository, head)) = ci {
        ci_refresh(d, env, repository, head, out);
    }
}

fn unsubscribe(d: &mut Domain, task: u64, topic: Topic, out: &mut Queue<Request>) {
    if d.subscriptions.remove(&(task, topic.clone())).is_some() {
        emit(out, Request::Erase { key: Key::Subscription { task, topic: topic.clone() } });
        match topic {
            Topic::Ci { repository, head } => {
                let mut named = false;
                for (_, subscriber) in &d.subscriptions {
                    if subscriber.topic == (Topic::Ci { repository, head }) {
                        named = true;
                    }
                }
                if !named && d.ci.remove(&(repository, head)).is_some() {
                    emit(out, Request::Erase { key: Key::Ci { repository, head } });
                }
            }
            Topic::Landings { .. } | Topic::Pull { .. } | Topic::Participation { .. } => {}
        }
    }
}

fn ci_refresh(
    d: &mut Domain,
    env: &Env<Limits>,
    repository: skein_forge::api::Repository,
    head: skein_forge::api::Commit,
    out: &mut Queue<Request>,
) {
    if d.forge_ended(repository.forge) {
        return;
    }
    for (_, pending) in &d.pending_ci {
        if pending.repository == repository && pending.head == head {
            return;
        }
    }
    if d.pending_ci.len() == env.limits.subscriptions {
        return;
    }
    let Some(sequence) = d.sequence.checked_add(1) else { return };
    d.sequence = sequence;
    let owner = Token::new(sequence | (1_u64 << 60));
    d.pending_ci.insert(owner, PendingCi { repository, head }).expect("CI read capacity checked");
    child(
        d,
        env,
        skein_forge_client::Event::Read { owner, repository, read: skein_forge::api::Read::Checks { commit: head } },
        out,
    );
}

fn ci_hint(d: &mut Domain, env: &Env<Limits>, hint: &skein_forge::api::Hint, out: &mut Queue<Request>) {
    let head = match &hint.change {
        skein_forge::api::Change::Commit(head) => *head,
        skein_forge::api::Change::Item(_)
        | skein_forge::api::Change::Branch(_)
        | skein_forge::api::Change::BranchMoved { .. } => {
            return;
        }
    };
    let mut subscribed = false;
    for (_, subscriber) in &d.subscriptions {
        match &subscriber.topic {
            Topic::Ci { repository, head: watched } if *repository == hint.repository && *watched == head => {
                subscribed = true;
                break;
            }
            Topic::Ci { .. } | Topic::Landings { .. } | Topic::Pull { .. } | Topic::Participation { .. } => {}
        }
    }
    if subscribed {
        ci_refresh(d, env, hint.repository, head, out);
    }
}

fn ci_read(
    d: &mut Domain,
    owner: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(pending) = d.pending_ci.remove(&owner) else { return };
    if let Ok(skein_forge::api::Answer::Checks(rows)) = result {
        let required = match d.repositories.get(&pending.repository) {
            Some(repository) => crate::ci::required(&repository.protection),
            None => &[],
        };
        let ci = crate::ci::fold(&rows, required);
        ci_observed(d, pending.repository, pending.head, ci, out);
    }
}

fn ci_observed(
    d: &mut Domain,
    repository: skein_forge::api::Repository,
    head: skein_forge::api::Commit,
    status: skein_forge::api::Ci,
    out: &mut Queue<Request>,
) {
    let topic = Topic::Ci { repository, head };
    let mut named = false;
    for (_, subscriber) in &d.subscriptions {
        if subscriber.topic == topic {
            named = true;
        }
    }
    let same = match d.ci.get(&(repository, head)) {
        Some(prior) => prior.status == status,
        None => false,
    };
    if !named || same {
        return;
    }
    if d.ci.len() == d.ci.capacity() && !d.ci.contains_key(&(repository, head)) {
        return;
    }
    let row = CiState { repository, head, status };
    d.ci.insert((repository, head), row.clone()).expect("CI subscription capacity");
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Ci(row) });
    publish(d, topic, News::Ci { head, status }, out);
}

fn changed(
    d: &mut Domain,
    env: &Env<Limits>,
    resource: skein_forge_client::Resource,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    match result {
        Ok(skein_forge::api::Answer::Commit(commit)) => branch_changed(d, env, resource, commit, out),
        Ok(skein_forge::api::Answer::Pull(pull)) => pull_changed(d, env, resource, pull, out),
        Ok(skein_forge::api::Answer::Item { item, comments, more: _ }) => {
            let topic = Topic::Participation { repository: resource.repository, number: item.number };
            if !comments.is_empty() {
                publish(d, topic, News::Changed { number: item.number }, out);
            }
        }
        Ok(skein_forge::api::Answer::Reviews { reviews, more: _ }) => match resource.what {
            skein_forge_client::What::Pull(number) if !reviews.is_empty() => {
                let topic = Topic::Participation { repository: resource.repository, number };
                publish(d, topic, News::Changed { number }, out);
            }
            skein_forge_client::What::Pull(_)
            | skein_forge_client::What::Repository
            | skein_forge_client::What::Branch(_)
            | skein_forge_client::What::Issue(_) => {}
        },
        Ok(
            skein_forge::api::Answer::Items { .. }
            | skein_forge::api::Answer::Located { .. }
            | skein_forge::api::Answer::Branches(_)
            | skein_forge::api::Answer::Statuses { .. }
            | skein_forge::api::Answer::Remarks { .. }
            | skein_forge::api::Answer::PullFiles { .. }
            | skein_forge::api::Answer::Compare { .. }
            | skein_forge::api::Answer::Checks(_)
            | skein_forge::api::Answer::File { .. }
            | skein_forge::api::Answer::Job { .. }
            | skein_forge::api::Answer::Protection(_)
            | skein_forge::api::Answer::Settings(_)
            | skein_forge::api::Answer::Collaborators { .. }
            | skein_forge::api::Answer::Permission(_)
            | skein_forge::api::Answer::Created(_)
            | skein_forge::api::Answer::Commented(_)
            | skein_forge::api::Answer::Reviewed(_)
            | skein_forge::api::Answer::Merged(_)
            | skein_forge::api::Answer::Runs { .. }
            | skein_forge::api::Answer::Run(_)
            | skein_forge::api::Answer::Tree { .. }
            | skein_forge::api::Answer::Queued
            | skein_forge::api::Answer::Branch(_)
            | skein_forge::api::Answer::Done,
        )
        | Err(_) => {}
    }
}

fn from_client(resource: &skein_forge_client::Resource, l: &Limits) -> Option<Name> {
    let what = match &resource.what {
        skein_forge_client::What::Repository => What::Repository,
        skein_forge_client::What::Pull(number) => What::Pull(*number),
        skein_forge_client::What::Issue(number) => What::Issue(*number),
        skein_forge_client::What::Branch(branch) => {
            if branch.is_empty() || branch.len() > usize::try_from(l.name_bytes).ok()? {
                return None;
            }
            let mut parts = List::with_capacity(l.name_bytes);
            let mut start = 0usize;
            for (index, byte) in branch.iter().enumerate() {
                if *byte == b'/' {
                    if index == start {
                        return None;
                    }
                    parts.push(Box::from(branch.get(start..index)?)).ok()?;
                    start = index.checked_add(1)?;
                }
            }
            if start == branch.len() {
                return None;
            }
            parts.push(Box::from(branch.get(start..)?)).ok()?;
            What::Branch(parts.into_boxed())
        }
    };
    Some(Name { forge: resource.repository.forge, repository: resource.repository.repository, what })
}

#[expect(clippy::manual_map, reason = "step code has no closures")]
fn branch_changed(
    d: &mut Domain,
    env: &Env<Limits>,
    resource: skein_forge_client::Resource,
    commit: skein_forge::api::Commit,
    out: &mut Queue<Request>,
) {
    let Some(name) = from_client(&resource, &env.limits) else { return };
    if let Some(hold) = d.holds.get(&name)
        && hold.drift.is_some()
    {
        return;
    }
    let Some(branch) = (match &resource.what {
        skein_forge_client::What::Branch(branch) => Some(branch.clone()),
        skein_forge_client::What::Repository
        | skein_forge_client::What::Pull(_)
        | skein_forge_client::What::Issue(_) => None,
    }) else {
        return;
    };
    let previous = match d.heads.get(&name) {
        Some(row) => Some(row.commit),
        None => None,
    };
    let recorded = match d.heads.get(&name) {
        Some(row) => row.owned,
        None => false,
    };
    let row = BranchHead { name: name.clone(), commit, owned: recorded };
    if d.heads.insert(name, row.clone()).is_err() {
        return;
    }
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::BranchHead(row) });
    let Some(before) = previous else { return };
    if before == commit {
        return;
    }
    let topic = Topic::Landings { repository: resource.repository, branch };
    if d.landings.len() >= env.limits.landings.max(env.limits.client.resources) {
        publish(d, topic, News::Landing { before, after: commit, files: None }, out);
        return;
    }
    let Some(sequence) = d.sequence.checked_add(1) else {
        publish(d, topic, News::Landing { before, after: commit, files: None }, out);
        return;
    };
    d.sequence = sequence;
    let owner = Token::new(sequence | (1_u64 << 63));
    d.landings.insert(owner, PendingLanding { topic, before, after: commit }).expect("landing capacity preflighted");
    child(
        d,
        env,
        skein_forge_client::Event::Read {
            owner,
            repository: resource.repository,
            read: skein_forge::api::Read::Compare { before, after: commit },
        },
        out,
    );
}

fn landing_read(
    d: &mut Domain,
    owner: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(pending) = d.landings.remove(&owner) else { return };
    let files = match result {
        Ok(skein_forge::api::Answer::Compare { before, after, files, commits: _, contains_before: _ })
            if before == pending.before && after == pending.after =>
        {
            let mut names = List::with_capacity(u32::try_from(files.len()).unwrap_or(u32::MAX));
            for file in files {
                names.push(file.path).expect("sized from bounded comparison");
            }
            Some(names.into_boxed())
        }
        Ok(_) | Err(_) => None,
    };
    publish(d, pending.topic, News::Landing { before: pending.before, after: pending.after, files }, out);
}

fn pull_changed(
    d: &mut Domain,
    env: &Env<Limits>,
    resource: skein_forge_client::Resource,
    pull: skein_forge::api::Pull,
    out: &mut Queue<Request>,
) {
    let name = from_client(&resource, &env.limits);
    let prior = match &name {
        Some(name) => d.pulls.get(name).cloned(),
        None => None,
    };
    // Only what the gather reads wakes the change: an unchanged poll does not (8.3).
    let wakes = match (&prior, &name) {
        (Some(row), Some(name)) => {
            row.head != pull.commit
                || row.ci != pull.ci
                || row.state != pull.state
                || d.pull_mergeable.get(name) != Some(&pull.mergeable)
        }
        (Some(_), None) | (None, _) => true,
    };
    let mut changed = List::with_capacity(env.limits.changes);
    for (_, row) in &d.changes {
        if row.repository == resource.repository && row.pull == Some(pull.number) {
            if wakes {
                d.procedure_wakes.insert(row.task, true).expect("one pending wake per change");
            }
            // A cancelled change's release closes its own pull (8.6): no drift.
            let cancelling = match d.releases.get(&row.task) {
                Some(release) => release.ending == ReleaseEnding::Cancelled,
                None => false,
            };
            let change = if row.base != pull.base {
                Some(crate::DriftChange::Retargeted {
                    number: pull.number,
                    before: row.base.clone(),
                    after: pull.base.clone(),
                })
            } else if pull.state == skein_forge::api::State::Closed && pull.merged.is_none() && !cancelling {
                Some(crate::DriftChange::PullClosed { number: pull.number })
            } else {
                None
            };
            if let Some(change) = change {
                changed.push((row.branch.clone(), change)).expect("bounded owned changes");
            }
        }
    }
    for (branch, change) in changed.into_boxed() {
        drift(
            d,
            env,
            skein_forge_client::Resource {
                repository: resource.repository,
                what: skein_forge_client::What::Branch(branch),
            },
            change,
            out,
        );
    }
    let Some(name) = name else { return };
    let state = PullState { name: name.clone(), head: pull.commit, ci: pull.ci, state: pull.state };
    if d.pulls.insert(name.clone(), state.clone()).is_err() {
        return;
    }
    d.pull_mergeable.insert(name, pull.mergeable).expect("mergeable beside each pull row");
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::PullState(state) });
    let topic = Topic::Pull { repository: resource.repository, number: pull.number };
    let changed_pull = match &prior {
        Some(row) => row.head != pull.commit || row.state != pull.state,
        None => true,
    };
    if changed_pull {
        publish(d, topic, News::Changed { number: pull.number }, out);
    }
    let changed_ci = match &prior {
        Some(row) => row.head != pull.commit || row.ci != pull.ci,
        None => true,
    };
    if changed_ci {
        let required = match d.repositories.get(&resource.repository) {
            Some(repository) => crate::ci::required(&repository.protection),
            None => &[],
        };
        if required.is_empty() {
            ci_observed(d, resource.repository, pull.commit, pull.ci, out);
        } else {
            ci_refresh(d, env, resource.repository, pull.commit, out);
        }
    }
}

pub(crate) fn overlap(files: &[Box<[u8]>], paths: &[Box<[u8]>]) -> bool {
    for file in files {
        for path in paths {
            if file.as_ref() == path.as_ref() {
                return true;
            }
            if file.starts_with(path) && file.get(path.len()) == Some(&b'/') {
                return true;
            }
        }
    }
    false
}

pub(crate) fn classify(d: &Domain, subscription: &Subscriber, news: &News) -> Class {
    match news {
        News::Landing { after, files, .. } => {
            if let Some(owner) = d.landed.get(after)
                && subscription.own_change == Some(*owner)
            {
                return Class::Kept;
            }
            match files {
                None => Class::Wakes,
                Some(files) if overlap(files, &subscription.paths) => Class::Wakes,
                Some(_) => Class::Kept,
            }
        }
        News::Ci { .. } | News::Changed { .. } => Class::Wakes,
    }
}

fn publish(d: &Domain, topic: Topic, news: News, out: &mut Queue<Request>) {
    for (_, subscriber) in &d.subscriptions {
        if subscriber.topic == topic {
            let class = topics::classify(d, subscriber, &news);
            emit(
                out,
                Request::News {
                    task: subscriber.task,
                    subscription: subscriber.number,
                    topic: topic.clone(),
                    news: news.clone(),
                    class,
                },
            );
        }
    }
}

#[expect(
    clippy::match_like_matches_macro,
    clippy::wildcard_enum_match_arm,
    reason = "the outbox test uses explicit matches in this strict subset"
)]
fn updating_entry(d: &Domain, task: u64) -> bool {
    let Some(change) = d.changes.get(&task) else { return false };
    if !match change.change.state {
        change::State::Updating { .. } => true,
        _ => false,
    } {
        return false;
    }
    let Some(number) = change.pending else { return false };
    let Some(Outboxed { entry, .. }) = d.entries.get(&number) else { return false };
    match entry.effect.write {
        skein_forge::api::Write::Update { .. } => true,
        _ => false,
    }
}

fn drift(
    d: &mut Domain,
    env: &Env<Limits>,
    resource: skein_forge_client::Resource,
    change: crate::DriftChange,
    out: &mut Queue<Request>,
) {
    let mut affected = List::with_capacity(env.limits.holds);
    for (name, row) in &d.holds {
        if to_client(name, &env.limits) == Some(resource.clone()) {
            if updating_entry(d, row.task) || deleting_entry(d, row.task, &resource, &change) {
                continue;
            }
            emit(out, Request::Drift { task: row.task, resource: name.clone() });
            affected.push((row.task, name.clone())).expect("one affected task per hold");
        }
    }
    for (task, name) in &affected {
        if let Some(hold) = d.holds.get_mut(name) {
            hold.drift = Some(crate::Drift { at: env.wall, change: change.clone() });
            save_hold(out, hold.clone());
        }
        if let Some(row) = d.changes.get_mut(task) {
            row.drift = Some(match change {
                crate::DriftChange::Branch { .. } => change::Hold::BranchMoved,
                crate::DriftChange::PullClosed { .. } => change::Hold::PullClosed,
                crate::DriftChange::Retargeted { .. } => change::Hold::Retargeted,
            });
            let saved = row.clone();
            save_change(d, out, saved);
        }
    }
}

fn deleting_entry(d: &Domain, task: u64, resource: &skein_forge_client::Resource, change: &crate::DriftChange) -> bool {
    match change {
        crate::DriftChange::Branch { observed: None, .. } => {}
        crate::DriftChange::Branch { observed: Some(_), .. }
        | crate::DriftChange::PullClosed { .. }
        | crate::DriftChange::Retargeted { .. } => return false,
    }
    let Some(release) = d.releases.get(&task) else { return false };
    let Some((number, _)) = &release.pending else { return false };
    let Some(Outboxed { entry, .. }) = d.entries.get(number) else { return false };
    match &entry.effect.write {
        skein_forge::api::Write::DeleteBranch { branch } => match &resource.what {
            skein_forge_client::What::Branch(found) => entry.repository == resource.repository && branch == found,
            skein_forge_client::What::Repository
            | skein_forge_client::What::Pull(_)
            | skein_forge_client::What::Issue(_) => false,
        },
        skein_forge::api::Write::CreateIssue { .. }
        | skein_forge::api::Write::OpenPull { .. }
        | skein_forge::api::Write::Post { .. }
        | skein_forge::api::Write::Review { .. }
        | skein_forge::api::Write::Edit { .. }
        | skein_forge::api::Write::SetReviewers { .. }
        | skein_forge::api::Write::Close { .. }
        | skein_forge::api::Write::Reopen { .. }
        | skein_forge::api::Write::Merge { .. }
        | skein_forge::api::Write::Update { .. }
        | skein_forge::api::Write::Status { .. }
        | skein_forge::api::Write::Dispatch { .. }
        | skein_forge::api::Write::Rerun { .. }
        | skein_forge::api::Write::CreateBranch { .. } => false,
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive durable row restoration dispatcher")]
fn restore(d: &mut Domain, env: &Env<Limits>, record: Stored) {
    if d.restore_phase != RestorePhase::Active {
        d.restore_phase = RestorePhase::Restoring;
    }
    if d.restore_phase == RestorePhase::Restoring {
        d.reconciliation.restore(&record);
    }
    match record {
        Stored::Landing(row) => {
            d.landing.rows.insert(row.landing.task, row).expect("restored landing within limits");
        }
        Stored::Receipt { key, receipt } => {
            d.landing.receipts.insert(key, receipt).expect("restored receipt within limits");
        }
        Stored::Application { token, until, outcome } => d.application.restore(token, until, outcome),
        Stored::Call { key, answer } => {
            d.keep_named_answer(&env.limits, key, answer).expect("restored named-call payload within limits");
        }
        Stored::Repository(row) => {
            d.repositories.insert(row.provider, row).expect("restored repository within limits");
        }
        Stored::Hold(row) => {
            d.holds.insert(row.name.clone(), row).expect("restored hold within limits");
        }
        Stored::Names { task, resources } => {
            d.names.insert(task, resources).expect("restored names within limits");
        }
        Stored::Subscription(row) => {
            d.subscriptions.insert((row.task, row.topic.clone()), row).expect("restored subscription within limits");
        }
        Stored::BranchHead(row) => {
            d.heads.insert(row.name.clone(), row).expect("restored branch head within limits");
        }
        Stored::PullState(row) => {
            d.pulls.insert(row.name.clone(), row).expect("restored pull within limits");
        }
        Stored::Ci(row) => {
            d.ci.insert((row.repository, row.head), row).expect("restored CI within limits");
        }
        Stored::Landed { commit, task } => {
            d.landed.insert(commit, task).expect("restored landing within limits");
        }
        Stored::ProposedEffect(row) => {
            assert!(d.proposed_effects.insert(row.number, row).is_ok(), "restored proposed effect fits");
        }
        Stored::Entry { entry, held } => {
            d.entries
                .insert(
                    entry.number,
                    Outboxed { entry, held, handoff: if held { Handoff::Handed } else { Handoff::Restored } },
                )
                .expect("restored entry within limits");
        }
        Stored::Binding { address, subject } => {
            assert!(crate::binding_address(&address) && subject.len() <= 255, "restored binding fields");
            d.bindings.insert(address, subject).expect("restored binding within limits");
        }
        Stored::Flight { task, entry } => {
            // Cross-record reconciliation is the later restart stage (domain/root.md, 9.1).
            d.flights.insert((task, entry), ()).expect("restored flights within entry limit");
        }
        Stored::DemoAct(row) => {
            d.demo_acts.insert(row.key, row.act).expect("restored demo acts within limits");
        }
        Stored::Demo(row) => {
            if d.restore_phase == RestorePhase::Restoring {
                assert!(d.restored_demo.is_none(), "one restored demo row");
                d.restored_demo = Some(row);
            }
            if let Some(demo) = &d.demo
                && row.forge == demo.adoption.forge
            {
                d.demo_state = Some(row.state);
            }
        }
        Stored::Writer(row) => {
            let mut discarded = Queue::with_capacity(env.limits.output);
            child(
                d,
                env,
                skein_forge_client::Event::Restore { record: skein_forge_client::Stored::Writer(row) },
                &mut discarded,
            );
            assert!(discarded.is_empty(), "restoration emits no output");
            d.writers.insert(row.forge, row.author).expect("restored writer within limits");
        }
        Stored::Client(row) => {
            let mut discarded = Queue::with_capacity(env.limits.output);
            child(d, env, skein_forge_client::Event::Restore { record: row }, &mut discarded);
            assert!(discarded.is_empty(), "restoration emits no output");
        }
        Stored::Change(row) => {
            // Volatile waits are rebuilt by stepping every restored change (domain/forge.md, 16).
            d.change_due.insert(row.task, env.wall).expect("a wake per restored change");
            d.changes.insert(row.task, row).expect("restored change within limits");
        }
        Stored::Issue(row) => {
            if row.pending.is_none() && !row.failed && !row.state.closed && row.desired.is_some() {
                // A committed desired view may still await its body interval,
                // milestone, or closing write when the transient timer is lost.
                d.projection_due.insert(row.goal, env.wall).expect("projection timers are bounded by issues");
            }
            d.issues.insert(row.goal, row).expect("restored issue within limits");
        }
        Stored::Release(row) => {
            d.releases.insert(row.task, row).expect("restored release within limits");
        }
    }
}

fn restored(d: &mut Domain, env: &Env<Limits>, clock: skein_forge_client::RecoveryClock, out: &mut Queue<Request>) {
    d.restore_phase = RestorePhase::Restoring;
    child(d, env, skein_forge_client::Event::Restored { clock }, out);
    retire_ended(d, env, out);
    // The client's transient watcher flags are rebuilt from the connector's
    // durable writers before any fresh background read can classify a push.
    {
        let mut resources = List::with_capacity(env.limits.holds);
        for (name, hold) in &d.holds {
            match hold.writer {
                Some(Writer::Run { .. } | Writer::Lost { .. }) => {
                    if let Some(resource) = to_client(name, &env.limits) {
                        resources.push(resource).expect("writers are bounded by holds");
                    }
                }
                Some(Writer::Entry(_)) | None => {}
            }
        }
        for resource in &resources {
            child(d, env, skein_forge_client::Event::Writer { resource: resource.clone(), taken: true }, out);
        }
    }
    child(d, env, skein_forge_client::Event::PauseOutbox, out);
    emit(out, Request::RestartDone { stage: RestartStage::Restored });
}

fn settle_outbox(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    // A held entry waits for a person, out of the client's outbox.
    let mut entries = List::with_capacity(env.limits.entries);
    for (_, row) in &d.entries {
        if !row.held && row.handoff != Handoff::Handed {
            entries.push(row.entry.clone()).expect("entry list capacity");
        }
    }
    for entry in &entries {
        // Startup reconciliation can build entries before Restored too.
        d.entries.get_mut(&entry.number).expect("restored entry remains").handoff = Handoff::Handed;
        child(d, env, skein_forge_client::Event::Make { entry: entry.clone() }, out);
    }
    child(d, env, skein_forge_client::Event::SettleOutbox, out);
}

fn read_afresh(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    crate::landing::rebuild(d);
    d.restore_phase = RestorePhase::Active;
    crate::demo::restart(d, out);
    d.restart_fresh = Some(false);
    let mut lost_runs = List::with_capacity(env.limits.holds);
    for (_, hold) in &d.holds {
        if let Some(Writer::Lost { task, attempt }) = hold.writer
            && !lost_runs.as_slice().contains(&(task, attempt))
        {
            lost_runs.push((task, attempt)).expect("runs bounded by holds");
        }
    }
    for (task, attempt) in &lost_runs {
        lost(d, env, *task, *attempt, out);
    }
    let mut ci = List::with_capacity(env.limits.subscriptions);
    for (_, subscriber) in &d.subscriptions {
        match &subscriber.topic {
            Topic::Ci { repository, head } if !ci.as_slice().contains(&(*repository, *head)) => {
                ci.push((*repository, *head)).expect("subscription capacity");
            }
            Topic::Ci { .. } | Topic::Landings { .. } | Topic::Pull { .. } | Topic::Participation { .. } => {}
        }
    }
    let mut changes = List::with_capacity(env.limits.changes);
    for (task, _) in &d.changes {
        changes.push(*task).expect("change capacity");
    }
    for task in &changes {
        topics::refresh(d, env, *task, out);
    }
    for (repository, head) in &ci {
        ci_refresh(d, env, *repository, *head, out);
    }
    crate::landing::refresh(d, env, out);
    child(d, env, skein_forge_client::Event::ReadAfresh, out);
}

// A permanently retired forge has no future observation to await. Its
// historical rows stay; its live watches and local release gates do not.
fn retire_ended(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if d.demo_state != Some(crate::DemoState::Ended) {
        return;
    }
    let mut removed = List::with_capacity(env.limits.holds);
    for (owner, pending) in &d.lost {
        if d.forge_ended(pending.name.forge) {
            removed.push(*owner).expect("bounded lost reads");
        }
    }
    for owner in removed.into_boxed() {
        d.lost.remove(&owner);
    }
    let mut ci = List::with_capacity(env.limits.subscriptions);
    for (owner, pending) in &d.pending_ci {
        if d.forge_ended(pending.repository.forge) {
            ci.push(*owner).expect("bounded CI reads");
        }
    }
    for owner in ci.into_boxed() {
        d.pending_ci.remove(&owner);
    }
    let mut files = List::with_capacity(env.limits.changes);
    for (owner, pending) in &d.goal_file_reads {
        if d.forge_ended(pending.repository.forge) {
            files.push(*owner).expect("bounded goal file reads");
        }
    }
    for owner in files.into_boxed() {
        d.goal_file_reads.remove(&owner);
    }
    let mut watches = Map::with_capacity(env.limits.client.resources);
    for (_, names) in &d.names {
        for name in names {
            if d.forge_ended(name.forge) {
                continue;
            }
            let Some(resource) = to_client(name, &env.limits) else { continue };
            if watches.contains_key(&resource) {
                continue;
            }
            let participating = participating(d, &resource);
            watches
                .insert(resource.clone(), skein_forge_client::Watch { resource, participating })
                .expect("existing bounded watch set");
        }
    }
    let mut list = List::with_capacity(env.limits.client.resources);
    for (_, watch) in &watches {
        list.push(watch.clone()).expect("bounded watches");
    }
    child(d, env, skein_forge_client::Event::Keep { owner: Token::new(u64::MAX), watches: list.into_boxed() }, out);
}

fn read_afresh_complete(d: &mut Domain, out: &mut Queue<Request>) {
    if d.restart_fresh == Some(true)
        && d.landing.pending.is_empty()
        && d.pending_ci.is_empty()
        && d.goal_file_reads.is_empty()
        && d.lost.is_empty()
    {
        d.restart_fresh = None;
        emit(out, Request::RestartDone { stage: RestartStage::ReadAfresh });
    }
}

fn settled_projection(d: &Domain, goal: u64, out: &mut Queue<Request>) {
    let settled = match d.issues.get(&goal) {
        Some(row) => {
            (row.state.closed || row.failed)
                && match &row.desired {
                    Some(view) => view.finished.is_some(),
                    None => false,
                }
        }
        None => false,
    };
    if settled {
        emit(out, Request::ProjectionSettled { goal });
    }
}

/// Capture terminal CI in the save already ending the change; a later save
/// without a fresh read keeps the observation restored with the row.
fn retain_terminal_ci(d: &mut Domain, row: &mut ChangeRow) {
    if !row.terminal() {
        return;
    }
    let read = match d.panel_ci.get(&row.task) {
        Some(read) if Some(read.head) == row.change.last_head => Some(*read),
        Some(_) | None => row.ci,
    };
    row.ci = match read {
        Some(read) if Some(read.head) == row.change.last_head => Some(read),
        Some(_) | None => None,
    };
    d.changes.get_mut(&row.task).expect("saved change is retained").ci = row.ci;
}

/// Every saved record invalidates its own panel (domain/forge.md, 8.8).
pub(crate) fn save_change(d: &mut Domain, out: &mut Queue<Request>, mut row: ChangeRow) {
    retain_terminal_ci(d, &mut row);
    let task = row.task;
    let tasks = match row.change.landing {
        Some(landing) => Box::from([task, landing]),
        None => Box::from([task]),
    };
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Change(row) });
    emit(out, Request::Marked { tasks });
}

/// Include the nearest affected followers when a saved row changes its queue order.
fn save_queue_change(
    d: &mut Domain,
    env: &Env<Limits>,
    before: Option<change::Ready>,
    mut row: ChangeRow,
    out: &mut Queue<Request>,
) {
    let after = crate::panel::ready(&row);
    if before == after {
        save_change(d, out, row);
        return;
    }
    let (ready, repair) = queue_ready(d, &row, &env.limits);
    let old_repair = if row.base_repair && before.is_some() { Some(row.task) } else { repair };
    let mut followers = List::with_capacity(env.limits.changes);
    for candidate in &ready {
        if candidate.task == row.task {
            continue;
        }
        let was_behind = match before {
            Some(old) => crate::panel::queue_before(&old, candidate, old_repair, &env.limits, env.wall),
            None => false,
        };
        let now_behind = match after {
            Some(new) => crate::panel::queue_before(&new, candidate, repair, &env.limits, env.wall),
            None => false,
        };
        if was_behind || now_behind {
            followers.push(*candidate).expect("queue bounds followers");
        }
    }
    let mut tasks = List::with_capacity(crate::MARKED);
    tasks.push(row.task).expect("own subject");
    if let Some(landing) = row.change.landing {
        tasks.push(landing).expect("member landing subject");
    }
    let mut previous = None;
    while tasks.room() > 0 {
        let mut next = None;
        for candidate in &followers {
            if let Some(previous) = previous
                && !crate::panel::queue_before(&previous, candidate, repair, &env.limits, env.wall)
            {
                continue;
            }
            next = match next {
                Some(current) if crate::panel::queue_before(&current, candidate, repair, &env.limits, env.wall) => {
                    Some(current)
                }
                Some(_) | None => Some(*candidate),
            };
        }
        let Some(next) = next else { break };
        tasks.push(next.task).expect("nearest follower room");
        previous = Some(next);
    }
    retain_terminal_ci(d, &mut row);
    emit(out, Request::Save { read_afresh: None, release: None, record: Stored::Change(row) });
    emit(out, Request::Marked { tasks: tasks.into_boxed() });
}

/// A read at a different head or with a different verdict changes the whole panel.
pub(crate) fn keep_panel_ci(d: &mut Domain, task: u64, read: change::Ci, out: &mut Queue<Request>) {
    assert!(d.changes.contains_key(&task), "CI belongs to a held change");
    if d.panel_ci.get(&task) != Some(&read) {
        d.panel_ci.insert(task, read).expect("one CI verdict per held change");
        emit(out, Request::Marked { tasks: Box::new([task]) });
    }
}

// Admission mirrors skein's Create grammar without an io dependency in this domain.
// The tests pin both grammars to the same boundary cases (protocol/setup.md, 6.7).
fn setup_login(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.first().expect("nonempty login").is_ascii_alphanumeric()
        && setup_characters(name)
}
fn setup_repository(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= 100 && name != b"." && name != b".." && setup_characters(name)
}
fn setup_characters(name: &[u8]) -> bool {
    for byte in name {
        if !byte.is_ascii_alphanumeric() && !b"._-".contains(byte) {
            return false;
        }
    }
    true
}

/// Pack a repository's complete forge identity (domain/forge.md, section 4).
#[must_use]
pub const fn tag(forge: u16, repository: u32) -> u64 {
    let [forge_high, forge_low] = forge.to_be_bytes();
    let [one, two, three, four] = repository.to_be_bytes();
    u64::from_be_bytes([0, 0, forge_high, forge_low, one, two, three, four])
}

/// Decode a repository tag, rejecting bits outside the forge identity's width.
#[must_use]
pub const fn untag(tag: u64) -> Option<(u16, u32)> {
    let [high, low, forge_high, forge_low, one, two, three, four] = tag.to_be_bytes();
    if high != 0 || low != 0 {
        None
    } else {
        Some((u16::from_be_bytes([forge_high, forge_low]), u32::from_be_bytes([one, two, three, four])))
    }
}

// One rule for a producer's failure and the box pipe closing (domain/forge.md, 4.1).
fn failed_at(held: &crate::Connection, failed: Option<crate::FailedStep>) -> Option<crate::FailedStep> {
    match held {
        crate::Connection::SettingUp { step: Some(crate::SetUpStep::Fetch) } => Some(crate::FailedStep::Fetch),
        crate::Connection::SettingUp { step: Some(crate::SetUpStep::Prepare) } => Some(crate::FailedStep::Prepare),
        crate::Connection::SettingUp { step: Some(crate::SetUpStep::Run) } => Some(crate::FailedStep::Run),
        crate::Connection::Connecting => Some(crate::FailedStep::Connect),
        crate::Connection::SetUpFailed { .. } => failed,
        crate::Connection::NotConnected
        | crate::Connection::NotSetUp
        | crate::Connection::SettingUp { step: None }
        | crate::Connection::Connected { .. }
        | crate::Connection::Stopped { .. } => None,
    }
}
