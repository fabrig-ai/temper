//! Neutral whole settings values and the four settings read views.
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall};
pub const AUTHORITY_MODELS: u32 = 64;
pub const AUTHORITY_MODEL_BYTES: u32 = 4_096;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AuthorityModels {
    All,
    Named(List<Box<[u8]>>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChangeSettings {
    pub against: u32,
    pub items: List<SettingItem>,
    pub running: Running,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Charter {
    /// Resume, inspect, modify, shell and agents, in that order.
    pub switches: [bool; 5],
    pub number: u32,
    pub instructions: Text,
    pub waiting: Duration,
    pub turns: u32,
    pub time: Duration,
    pub reserve_turns: u32,
    pub reserve_time: Duration,
    pub model: SettingModel,
    pub alternatives: List<SettingModel>,
    pub call_timeout: Duration,
    pub hosts: SettingHosts,
    pub spend: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentText {
    pub bytes: Text,
    pub cut: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EndedBy {
    /// Its run's judged result ends it.
    Result,
    /// Only its requester ends it; a run's raw finish parks it (5.8).
    Requester,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldRule {
    pub name: Box<[u8]>,
    pub max: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HeldRevision {
    pub head: RevisionHead,
    pub holders: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LlmChoice {
    /// Creator preference, separate from the task's last successfully claimed account.
    pub account: Option<u32>,
    pub main: ModelEffort,
    pub subagents: List<ModelEffort>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelEffort {
    pub model: Text,
    /// One of the efforts the model declares; `None` is its LLM provider's default.
    pub effort: Option<Text>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerAuthority {
    pub models: Option<AuthorityModels>,
    pub tools: u64,
    pub grants: List<OwnerGrant>,
    pub delegation: OwnerDelegation,
    pub spend: u64,
    pub deadline: Option<Wall>,
    pub notes: u8,
    pub note_resources: List<OwnerResourceScope>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerAuthorityExecutor {
    Charter(u32),
    Procedure(u32),
    Role(u32),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerDelegation {
    pub kinds: List<OwnerAuthorityExecutor>,
    pub tasks: u32,
    pub depth: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerGrant {
    pub connector: u16,
    pub kind: u16,
    pub pattern: OwnerPattern,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnerKindImplication {
    pub connector: u16,
    pub kind: u16,
    pub implies: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerPart {
    pub connector: u16,
    pub kind: OwnerPartKind,
    pub key: OwnerPartKey,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerPartKey {
    Path(List<Box<[u8]>>),
    Number(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerPartKind {
    Resource,
    Pool,
    Result,
    Procedure,
    Effect,
    Outbox,
    /// The supplied connector account for the numbered held task.
    Hold,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerPattern {
    pub segments: List<Box<[u8]>>,
    pub last: OwnerPatternEnd,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerPatternEnd {
    Exact(Box<[u8]>),
    Open(Box<[u8]>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerResourceScope {
    pub connector: u16,
    pub pattern: OwnerPattern,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PolicyEditRole {
    pub number: u32,
    pub authority: OwnerAuthority,
    pub period_spend: u64,
    pub requests: u16,
    pub decides: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PolicyGuard {
    Guarded,
    Observed { freshness: Duration },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PolicyJudge {
    pub connector: u16,
    pub requirement: u16,
    pub parameters: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PolicyPermission {
    pub connector: u16,
    pub permission: u16,
    pub role: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PolicyRequirement {
    pub connector: u16,
    pub kind: u16,
    pub pattern: OwnerPattern,
    pub judge: PolicyJudge,
    pub guard: PolicyGuard,
    pub must_be_guarded: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalKey {
    pub proposer: Proposer,
    pub number: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Proposer {
    /// A task's proposal store.
    Task(u64),
    /// A party's proposal store.
    Party(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Retention {
    Kept,
    ForgottenOnceRead,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionAnswer {
    Revision(RevisionDocument),
    Forgotten(RevisionForgotten),
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionDecider {
    FirstStart,
    Release,
    Party(RevisionParty),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionDocument {
    pub head: RevisionHead,
    pub items: List<RevisionItem>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionEdit {
    Set(SettingSet),
    TooLarge(Size),
    Removed,
    Reset,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionForgotten {
    pub oldest: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionHead {
    pub number: u32,
    pub against: u32,
    pub at: Wall,
    pub by: RevisionDecider,
    pub proposal: Option<ProposalKey>,
    pub moved: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionItem {
    pub setting: Setting,
    pub edit: RevisionEdit,
    pub replaced: RevisionReplaced,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionItemEdit {
    Set,
    Removed,
    Reset,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionItemHead {
    pub setting: Setting,
    pub edit: RevisionItemEdit,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionParty {
    pub number: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionRow {
    pub head: RevisionHead,
    pub items: List<RevisionItemHead>,
    pub more: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionsPage {
    pub rows: List<RevisionRow>,
    pub next: Option<u32>,
    pub forgotten: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rules {
    pub ceiling: OwnerAuthority,
    pub period_spend: u64,
    pub minimum_run_spend: u64,
    pub maximum_run_spend: u64,
    pub implies: List<OwnerKindImplication>,
    pub requirements: List<PolicyRequirement>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Running {
    Stays,
    Moves,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    Charter(SettingNumber),
    Template(SettingNumber),
    Policy(SettingNumber),
    Rules,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingDocument {
    pub setting: Setting,
    pub revision: u32,
    pub value: SettingValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingEdit {
    Set(SettingSet),
    Removed,
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingFault {
    Shape,
    Count,
    Reference,
    Rules,
    Budget,
    Grace,
    Owner,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingFollows {
    Default,
    Change,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingHosts {
    Workers,
    Engine,
    Both,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingItem {
    pub setting: Setting,
    pub edit: SettingEdit,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingLine {
    pub setting: Setting,
    pub name: Option<DocumentText>,
    pub follows: SettingFollows,
    pub last: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingModel {
    pub dialect: u32,
    pub endpoint: u32,
    pub program: Option<Text>,
    pub name: Text,
    pub window: u32,
    pub output: u32,
    pub input_price: u64,
    pub cached_price: u64,
    pub cache_write_price: Option<u64>,
    pub output_price: u64,
    pub price_unit: u32,
    pub efforts: List<Text>,
    pub effort: Option<Text>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SettingNumber {
    pub number: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingPolicy {
    pub escalation_role: Option<u32>,
    pub ceiling: OwnerAuthority,
    pub projections: List<OwnerGrant>,
    pub period_spend: u64,
    pub roles: List<PolicyEditRole>,
    pub requirements: List<PolicyRequirement>,
    pub permissions: List<PolicyPermission>,
    pub templates: List<TemplateOverride>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingSet {
    pub value: SettingValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingValue {
    Charter(Box<Charter>),
    Template(Box<Template>),
    Policy(Box<SettingPolicy>),
    Rules(Box<Rules>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsDocument {
    pub newest: u32,
    pub settings: List<SettingLine>,
    pub held: List<HeldRevision>,
    pub pending: List<SettingsProposal>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsProposal {
    pub proposer: Proposer,
    pub number: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskContractDocument {
    Approval { state: Box<[u8]>, part: OwnerPart },
    Report { words: u32 },
    Verdict { choices: List<VerdictChoice>, cut: bool },
    Change { connector: u16, kind: u16, words: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskFlags {
    pub conversation: bool,
    pub listed: bool,
    pub retention: Retention,
    pub ended_by: EndedBy,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Template {
    pub number: u32,
    pub name: DocumentText,
    pub executor: TemplateExecutor,
    pub contract: TaskContractDocument,
    pub shallower: u32,
    pub budget: TemplateBudget,
    pub deadline: TemplateDeadline,
    pub tracked: Option<u32>,
    pub flags: TaskFlags,
    pub label: u32,
    pub grace: Option<Duration>,
    /// The LLM choice the template gives its task; `None` is its charter's default.
    pub llm: Option<LlmChoice>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateBudget {
    Amount(u64),
    AsAllowed { cap: Option<u64> },
    Given,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateDeadline {
    None,
    Span(Duration),
    Given,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateExecutor {
    Charter(u32),
    Chosen,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TemplateOverride {
    pub template: u32,
    pub executor: Option<TemplateExecutor>,
    pub budget: Option<TemplateBudget>,
    pub deadline: Option<TemplateDeadline>,
    pub priority: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Text {
    bytes: Box<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VerdictChoice {
    pub code: u32,
    pub words: u32,
    pub followups: u32,
    pub fields: List<FieldRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadValue {
    Settings(Box<SettingsDocument>),
    Setting(Box<SettingDocument>),
    Revision(Box<RevisionAnswer>),
    Revisions(Box<RevisionsPage>),
}
impl Text {
    pub fn new(bytes: Box<[u8]>, limit: u32) -> Result<Self, Box<[u8]>> {
        let bounds = skein_json::writer::Limits { depth: 1, length: limit.saturating_mul(6).saturating_add(2) };
        let mut text = skein_json::writer::Encoder::measure(&bounds);
        text.string(&bytes);
        if bytes.len() > usize::try_from(limit).expect("text bound") || text.measured().is_err() {
            return Err(bytes);
        }
        Ok(Self { bytes })
    }
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.bytes
    }
}
impl DocumentText {
    #[must_use]
    pub fn complete(bytes: Text) -> Self {
        Self { bytes, cut: false }
    }
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bytes.bytes()
    }
    #[must_use]
    pub fn whole(&self) -> Option<Text> {
        if self.cut { None } else { Some(self.bytes.clone()) }
    }
}
impl Charter {
    #[must_use]
    pub fn resume(&self) -> bool {
        *self.switches.first().expect("charter switch")
    }
    #[must_use]
    pub fn inspect(&self) -> bool {
        *self.switches.get(1).expect("charter switch")
    }
    #[must_use]
    pub fn modify(&self) -> bool {
        *self.switches.get(2).expect("charter switch")
    }
    #[must_use]
    pub fn shell(&self) -> bool {
        *self.switches.get(3).expect("charter switch")
    }
    #[must_use]
    pub fn agents(&self) -> bool {
        *self.switches.get(4).expect("charter switch")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionReplaced {
    Value(SettingSet),
    TooLarge(Size),
    Default,
    Removed,
    NotOffered,
}
