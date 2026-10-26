//! Values crossing the connector top's parent boundary. Durable records are
//! whole rows: the parent saves them in the decision and only then sends
//! `Committed` for a new outbox entry (jig's domain/connectors.md, 4.2).
use alloc::boxed::Box;
use skein_lib::Token;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_issues as issues;

/// A pull request whose context the connector gathers for a run brief.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BriefItem {
    /// Its repository in the deployment's adopted set.
    pub repository: u32,
    /// Its provider number.
    pub number: u64,
}

/// A pinned head for one brief read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BriefCommit(pub [u8; 32]);

/// Which forge context the connector owns for a brief.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BriefSource {
    /// Failed checks and their logs on a pinned head.
    Ci { item: BriefItem, head: BriefCommit },
    /// Reviews and remarks on a pinned head.
    Reviews { item: BriefItem, head: BriefCommit },
    /// A pull request against its base at a pinned head.
    Pull { item: BriefItem, head: BriefCommit },
    /// Why a task reads its project's adopted home without pushing.
    Home { repository: skein_forge::api::Repository },
}

/// The connector's bounded section, or a failed gather.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BriefRead {
    /// The bytes the connector rendered, and bytes left out at its read bound.
    Got { bytes: Box<[u8]>, left: u64 },
    /// The pinned section could not be read.
    Failed,
}

/// A resource of one configured forge repository (domain/forge.md, section 2).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Name {
    pub forge: u16,
    pub repository: u32,
    pub what: What,
}
/// The kind and identity within a repository.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum What {
    /// The repository itself.
    Repository,
    /// A branch's path segments.
    Branch(Box<[Box<[u8]>]>),
    /// A numbered pull request.
    Pull(u64),
    /// A numbered issue.
    Issue(u64),
}
/// What the project may do on an adopted repository.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Temper owns the repository.
    Owned,
    /// Temper may contribute and land in it.
    Adopted,
    /// Temper contributes through a fork.
    Fork,
    /// Temper reads but never writes.
    Context,
}
/// Capabilities observed when the repository was adopted, narrowed on refusal.
#[expect(clippy::struct_excessive_bools, reason = "the forge has independent effect kinds")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Kinds {
    pub read: bool,
    pub push: bool,
    pub open: bool,
    pub land: bool,
    pub review: bool,
    pub status: bool,
    pub comment: bool,
    pub issue: bool,
    pub branch: bool,
}
/// Forge grants needed by the change procedure (domain/forge.md, sections 6 and 8).
pub const CHANGE_KINDS: [u16; 6] = [2, 3, 4, 10, 11, 12];

/// One adopted repository, with provider name and project ownership.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Repository {
    pub project: u32,
    /// This project's selected repository for goal issue projections and chats' saved work.
    pub home: bool,
    pub provider: skein_forge::api::Repository,
    pub host: Box<[u8]>,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub prefix: Box<[u8]>,
    pub role: Role,
    /// Whether this repository runs CI; absence of a status never selects this policy.
    pub ci: bool,
    /// Blocking project check gates used when CI is absent.
    pub checks: Box<[u32]>,
    pub kinds: Kinds,
    pub protection: Protection,
    pub settings: skein_forge::api::Settings,
}
/// The git identity a forge's repositories are pushed and fetched with: an
/// application credential numbered by the deployment's configuration
/// (protocol/credentials.md, "each repository to a git identity").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GitIdentity {
    pub forge: u16,
    pub credential: u32,
}
/// Protection as observed by this writer at adoption.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Protection {
    /// The writer could not read protection.
    Unknown,
    /// No applicable rule exists.
    Absent,
    /// An applicable rule was read.
    Rule(skein_forge::api::Protection),
}
/// The caller's request to adopt an existing repository.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Adoption {
    pub project: u32,
    /// Select this repository as the project's home if it has none yet.
    pub home: bool,
    pub forge: u16,
    pub host: Box<[u8]>,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub prefix: Box<[u8]>,
    pub role: Role,
    pub landing: Box<[u8]>,
    pub ci: bool,
    pub checks: Box<[u32]>,
}
/// A FORGE request admitted by the core (client/README.md, 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ask {
    Adopt(Adoption),
    SetUp { forge: u16, org: Box<[u8]>, repository: Box<[u8]>, project: SetUpProject },
    StartDemo,
    Connect { forge: u16, how: How },
    Install { forge: u16, back: Option<Box<[u8]>> },
    Accept { forge: u16, installation: core::num::NonZeroU64 },
    Reachable { forge: u16, installation: Option<u64>, page: u32 },
}
/// The first repository requested when connecting a Forgejo (client/README.md, 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FirstAsk {
    pub org: Box<[u8]>,
    pub repository: Box<[u8]>,
    pub project: SetUpProject,
    pub label: Box<[u8]>,
}
/// The public connect's input; credentials remain carried handles.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum How {
    ForgejoAdmin { first: Option<FirstAsk> },
    GitHubApp { app: core::num::NonZeroU64 },
    GitHubManifest { back: Option<Box<[u8]>> },
}
/// The App records' provenance since this engine started (client/README.md, 4.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppWay {
    Manifest,
    Pasted,
}
/// A retained application answer; only adoption outcomes are durable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Answered {
    Adopt(ApplicationOutcome),
    SetUp(SetUpAnswer),
    StartDemo(StartAnswer),
    Connect(ConnectAnswer),
    Install(InstallAnswer),
    Accept(AcceptAnswer),
    Reachable(ReachableAnswer),
}
/// Public forge admission refusals (client/README.md, 8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ForgeRefusal {
    Role,
    NoSuchForge,
    NotTaken,
    Connected,
    NotConnected,
    Busy,
    NoKey,
    /// A first repository's organization named as the bot's login: Forgejo's users and organizations
    /// share names (protocol/setup.md, 5.5).
    FirstIsBot,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConnectAnswer {
    Connecting,
    Link(Box<[u8]>),
    Refused(ForgeRefusal),
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InstallAnswer {
    Link(Box<[u8]>),
    Connecting,
    Refused(ForgeRefusal),
}
/// An existing installation, verified and retained by the setup host.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AcceptAnswer {
    Accepted { installation: u64 },
    Refused(ForgeRefusal),
    ForgeRefused(AdoptionRefusal),
}
/// A picker answer lives beside the public links, never in the store.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReachableAnswer {
    Page(Page),
    Refused(ForgeRefusal),
    ForgeRefused(AdoptionRefusal),
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Page {
    pub rows: Box<[Reached]>,
    pub more: bool,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reached {
    pub id: u64,
    pub owner_id: u64,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub private: bool,
}
/// The start is answered once both are done, *running*, or at the first that
/// fails, *start failed*, naming why (protocol/setup.md, 8.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StartAnswer {
    Running,
    NotOffered,
    Failed(StartFailure),
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StartFailure {
    Group { why: Box<[u8]> },
    Account,
    Adoption,
    NotConnected,
}
/// The demo's fake LLM, forge and project (protocol/setup.md, 8.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DemoConfig {
    pub provider: u32,
    pub adoption: Adoption,
    pub deadline: skein_lib::Duration,
}
/// The setup request's one answer, independent of its later course.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetUpAnswer {
    SettingUp,
    Refused(SetUpRefusal),
}
/// The configured forge provider (domain/forge.md, 4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Forgejo,
    GitHub,
}
/// How the deployment connects to its forge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Connected,
    Managed,
    Owned,
    Fake,
}
/// Public owned-forge startup details (protocol/setup.md, 6.5).
/// The password field is an absolute file path, never the password itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Owned {
    pub forgejo: Box<[u8]>,
    pub runner: Option<Box<[u8]>>,
    pub password: Box<[u8]>,
}
/// Public startup configuration, without credentials.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Configured {
    /// The records-only managed string form accepts an admin paste.
    pub admin_connect: bool,
    /// The setup entry's bot login; empty for a forge with no setup entry.
    pub bot: Box<[u8]>,
    pub owned: Option<Owned>,
    /// Pull-address base, empty when the configured git base cannot fit the address bound.
    pub git: Box<[u8]>,
    pub web: Option<Box<[u8]>>,
    pub read_only: bool,
    pub number: u16,
    pub kind: Kind,
    pub mode: Mode,
    /// The configuration's `start: asked`; false is the ordinary automatic start.
    pub asked: bool,
}
/// The row's setup offer, also used to admit `SetUp` (domain/forge.md, 4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Offer {
    NotOffered,
    NotUnderTheBox,
    Offered,
}
/// The project selected for the first repository (protocol/setup.md, 6.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetUpProject {
    Existing(u32),
    New(Box<[u8]>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateStep {
    Admin,
    Org,
    Repository,
    File,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstState {
    Making,
    Made { id: u64 },
    Failed { step: CreateStep },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupStage {
    Admin,
    Bot,
    Token,
    Org,
    Repository,
    File,
    App,
    Installation,
    State,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminStop {
    NotAdmin,
    Refused { status: u16, stage: SetupStage },
    Unmarked,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct First {
    pub org: Box<[u8]>,
    pub repository: Box<[u8]>,
    pub project: SetUpProject,
    pub state: FirstState,
    pub label: Box<[u8]>,
}

/// A repository already adopted on this forge (client/README.md, 4.6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AdoptedRow {
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub project: u32,
}

/// One configured forge, including its connection, offer and first repository.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    pub read_only: bool,
    pub number: u16,
    pub kind: Kind,
    pub mode: Mode,
    pub connection: Connection,
    pub failed: Option<FailedStep>,
    pub offer: Offer,
    pub first: Option<First>,
    pub installations: Box<[(u64, Installation)]>,
    pub ended: Option<Box<[u8]>>,
    pub app: Option<AppWay>,
    pub web: Option<Box<[u8]>>,
    pub repositories: Box<[AdoptedRow]>,
    pub owned: Option<Owned>,
    pub admin: bool,
    pub ended_stop: Option<AdminStop>,
}
/// The deployment's forges, ordered by number (domain/forge.md, 4.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Forges {
    pub rows: Box<[Row]>,
}
/// The admitted repository and its observed collaborators.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Adopted {
    pub repository: Repository,
    pub collaborators: Box<[skein_forge::api::Collaborator]>,
}
/// The connector's outcome, kept under the people child's application token.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ApplicationOutcome {
    Adopted(AdoptedRepository),
    CoreRefused(jig_domain_people::Refusal),
    ForgeRefused(AdoptionRefusal),
}
/// The repository facts shown by the adoption's terminal (client/README.md, 4.6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AdoptedRepository {
    pub repository: skein_forge::api::Repository,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub role: Role,
    pub home: bool,
    pub kinds: Kinds,
    pub protection: Protection,
}
/// A forge refusal stated in the adoption form's vocabulary (client/README.md, 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AdoptionRefusal {
    NoSuchForge,
    NotFound,
    NoAccess,
    Refused,
    InFlight,
    TooLarge,
    Unreachable,
    NotInstalled,
    InvalidAnswer,
    AnotherProject,
    HomeHeld,
    PrefixOccupied,
}
/// Project a forge call's refusal into the adoption's terminal reason.
#[must_use]
pub const fn reason(error: skein_forge::api::Error) -> AdoptionRefusal {
    match error {
        skein_forge::api::Error::Missing => AdoptionRefusal::NotFound,
        skein_forge::api::Error::Forbidden => AdoptionRefusal::NoAccess,
        skein_forge::api::Error::Refused => AdoptionRefusal::Refused,
        skein_forge::api::Error::Busy => AdoptionRefusal::InFlight,
        skein_forge::api::Error::TooLarge | skein_forge::api::Error::Full => AdoptionRefusal::TooLarge,
        skein_forge::api::Error::NotInstalled => AdoptionRefusal::NotInstalled,
        skein_forge::api::Error::NotConnected
        | skein_forge::api::Error::Unavailable
        | skein_forge::api::Error::Timeout
        | skein_forge::api::Error::RateLimited { .. } => AdoptionRefusal::Unreachable,
        skein_forge::api::Error::MissingJob
        | skein_forge::api::Error::Empty
        | skein_forge::api::Error::Exists
        | skein_forge::api::Error::NothingToMerge
        | skein_forge::api::Error::Closed
        | skein_forge::api::Error::Stale
        | skein_forge::api::Error::Conflict
        | skein_forge::api::Error::Protected
        | skein_forge::api::Error::InvalidAnswer => AdoptionRefusal::InvalidAnswer,
    }
}
/// Exclusive write hold and its current writer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hold {
    pub name: Name,
    pub task: u64,
    pub writer: Option<Writer>,
    /// Last outside change requiring a person to release this hold.
    pub drift: Option<Drift>,
}

/// An outside change and its observation time, retained with the affected hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Drift {
    pub at: skein_lib::Wall,
    pub change: DriftChange,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DriftChange {
    Branch { expected: skein_forge::api::Commit, observed: Option<skein_forge::api::Commit> },
    PullClosed { number: u64 },
    Retargeted { number: u64, before: Box<[u8]>, after: Box<[u8]> },
}
/// Durable progress while a closing task releases its connector resources.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReleaseRow {
    pub task: u64,
    pub root: u64,
    pub ending: ReleaseEnding,
    pub close_pull: Option<u64>,
    /// The cancel's reason, still to be posted on its pull before it closes.
    pub comment: Option<Box<[u8]>>,
    pub pending: Option<(u64, Option<Name>)>,
    pub failed: bool,
    /// All effects issued before cleanup have settled.
    pub effects_settled: bool,
    /// The task child has authorized cleanup after effect settlement.
    pub releasing: bool,
}
/// Terminal task class that determines forge cleanup.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReleaseEnding {
    /// The task finished successfully.
    Done,
    /// The task failed and its branch remains until the root closes.
    Failed,
    /// The task was cancelled, so its pull closes before its branch is deleted.
    Cancelled,
}
/// The last head pushed in one repository by a hosted attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pushed {
    pub tag: u64,
    pub commit: skein_forge::api::Commit,
}
/// A run or connector effect occupying one writer slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Writer {
    /// A worker attempt, fenced by the root's activation.
    Run { task: u64, attempt: u64 },
    /// An ended attempt whose fresh branch read must release its writer.
    Lost { task: u64, attempt: u64 },
    /// An outbox entry making a change to the resource.
    Entry(u64),
}
/// A subject published by the forge (domain/forge.md, section 7).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Topic {
    /// A branch's tip moved.
    Landings { repository: skein_forge::api::Repository, branch: Box<[u8]> },
    /// CI changed on one commit.
    Ci { repository: skein_forge::api::Repository, head: skein_forge::api::Commit },
    /// A pull's state changed.
    Pull { repository: skein_forge::api::Repository, number: u64 },
    /// A participant wrote on an object.
    Participation { repository: skein_forge::api::Repository, number: u64 },
}
/// A subscriber's classification of observed news.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Start or resume this task when its inbox permits it.
    Wakes,
    /// Retain for the next run.
    Kept,
    /// The task has no need to hear this event.
    Dropped,
}
/// News delivered through a topic to task inboxes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum News {
    /// A branch moved; overlap is decided from bounded file facts.
    Landing { before: skein_forge::api::Commit, after: skein_forge::api::Commit, files: Option<Box<[Box<[u8]>]>> },
    /// The combined CI state changed at its head.
    Ci { head: skein_forge::api::Commit, status: skein_forge::api::Ci },
    /// A pull or participating object's state changed.
    Changed { number: u64 },
}
/// A live subscription and its known paths for landing overlap.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Subscriber {
    pub task: u64,
    /// Root-issued task subscription, retained across restarts for news delivery.
    pub number: u64,
    pub topic: Topic,
    pub own_change: Option<u64>,
    /// Whole goal subtree for automatic topics; absent for an agent subscription.
    pub goal_tasks: Option<Box<[u64]>>,
    pub paths: Box<[Box<[u8]>]>,
}
/// Last observed branch tip, used to publish each landing once.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BranchHead {
    pub name: Name,
    pub commit: skein_forge::api::Commit,
    /// The store has confirmed this branch was left by our writer.
    pub owned: bool,
}
/// Last published pull state and CI verdict.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PullState {
    pub name: Name,
    pub head: skein_forge::api::Commit,
    pub ci: skein_forge::api::Ci,
    pub state: skein_forge::api::State,
}
/// Last CI verdict published to subscribers of one commit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CiState {
    pub repository: skein_forge::api::Repository,
    pub head: skein_forge::api::Commit,
    pub status: skein_forge::api::Ci,
}
/// A goal's issue projection, including an in-flight write and created issue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IssueRow {
    pub goal: u64,
    pub repository: skein_forge::api::Repository,
    pub state: issues::Projected,
    pub number: Option<u64>,
    pub pending: Option<u64>,
    pub before: Option<issues::Projected>,
    pub failed: bool,
    /// Latest root-authorized goal snapshot, including an ending while a write is pending.
    pub desired: Option<issues::GoalView>,
}
/// A change task's forge parameters and top-owned procedure state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChangeRow {
    pub task: u64,
    pub repository: skein_forge::api::Repository,
    pub branch: Box<[u8]>,
    pub base: Box<[u8]>,
    pub title: Box<[u8]>,
    pub body: Box<[u8]>,
    pub priority: i32,
    pub change: change::Change,
    pub pull: Option<u64>,
    pub pending: Option<u64>,
    pub effect: change::EffectResult,
    /// The one child requested by this procedure, and its latest result.
    pub delegate: Option<(u64, change::Delegate)>,
    pub delegate_status: change::Status,
    /// Bounded current verdict per gate, retained through clean updates.
    pub verdicts: Box<[change::GateReport]>,
    /// Bounded explanations of gate verdicts at their named heads.
    pub gate_remarks: Box<[GateRemark]>,
    pub drift: Option<change::Hold>,
    /// A deployment-owned repair of a landing branch, exempt from that branch's broken CI.
    pub base_repair: bool,
    /// The base repair started while this ordinary change owned the queue.
    pub queue_repair: Option<u64>,
    /// Repairs attempted for this landing queue while this change owned it.
    pub queue_repairs: u32,
}
/// One gate's explanation, retained with its verdict for a repair brief.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GateRemark {
    pub number: u64,
    pub head: skein_forge::api::Commit,
    pub words: Box<[u8]>,
}
/// Coherent provider facts used by the root to check a change effect before
/// releasing its durable outbox entry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChangeEvidence {
    /// State to restore if the root does not authorize this effect.
    pub prior: change::Change,
    pub head: Option<skein_forge::api::Commit>,
    pub base_tip: skein_forge::api::Commit,
    pub contains_base: change::Status,
    pub ci: change::Status,
    pub gates: Box<[change::GateReport]>,
    pub reviews: Box<[skein_forge::api::Review]>,
    pub reviews_complete: bool,
}
/// Identity supplied by the named-call owner, interpreted only for lookup.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NamedCall {
    pub task: u64,
    pub attempt: u64,
    pub completion: u32,
    pub position: u32,
}

/// The connector's part of a named answer; core decisions remain outside it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CallPayload {
    Effect { entry: u64, deadline: skein_lib::Wall, outcome: Option<skein_forge_client::Outcome> },
    Refused(skein_forge::api::Error),
    Read(Box<Result<skein_forge::api::Answer, skein_forge::api::Error>>),
}

/// The connector's durable records, wrapped by the root store.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Stored {
    /// The connector part of a live named call.
    Call {
        key: NamedCall,
        answer: CallPayload,
    },
    /// Connector-owned payload retained under an explicit proposal's number.
    ProposedEffect(ProposedEffect),
    /// One adopted repository.
    Repository(Repository),
    /// One held resource.
    Hold(Hold),
    /// All live resources named by one task.
    Names {
        task: u64,
        resources: Box<[Name]>,
    },
    /// One topic subscription.
    Subscription(Subscriber),
    /// One branch's last observed tip.
    BranchHead(BranchHead),
    /// One pull's last published state.
    PullState(PullState),
    Ci(CiState),
    /// A merge made by this deployment, for own-change news classification.
    Landed {
        commit: skein_forge::api::Commit,
        task: u64,
    },
    /// An unsettled outbox entry. A held one is out of the client's outbox,
    /// keeping its room until a person makes it again or it is settled not
    /// made (domain/forge.md, 6; jig's `domain/connectors.md`, 4.3).
    Entry {
        entry: skein_forge_client::Entry,
        held: bool,
    },
    /// A client working-set row.
    Client(skein_forge_client::Stored),
    /// A change procedure's committed state.
    Change(ChangeRow),
    /// A goal issue's committed projection.
    Issue(IssueRow),
    Release(ReleaseRow),
    /// A keyed adoption outcome retained until its wall-clock end.
    Application {
        token: u64,
        until: skein_lib::Wall,
        outcome: ApplicationOutcome,
    },
    /// A forge's writer, kept before any restored effect (domain/forge.md, 4.1).
    Writer(skein_forge_client::Writer),
    /// The demo's state, kept by its forge (protocol/setup.md, 8.4): no record is not started.
    Demo(Demo),
    /// First Google subject for a member address (protocol/sign-in.md, 5).
    Binding {
        address: Box<[u8]>,
        subject: Box<[u8]>,
    },
    /// An end act awaiting a saved answer (protocol/setup.md, 8.4).
    DemoAct(DemoAct),
    /// A sent write not yet settled (domain/forge.md, 6; protocol/store.md, 2).
    Flight {
        task: u64,
        entry: u64,
    },
}
/// A bounded connector payload awaiting an authority holder's decision.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ProposedEffect {
    pub number: u64,
    pub resource: What,
    pub entry: skein_forge_client::Entry,
}

/// Stable store key of a connector record.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Key {
    Call(NamedCall),
    ProposedEffect(u64),
    Repository(skein_forge::api::Repository),
    Hold(Name),
    Names(u64),
    Subscription {
        task: u64,
        topic: Topic,
    },
    BranchHead(Name),
    PullState(Name),
    Ci {
        repository: skein_forge::api::Repository,
        head: skein_forge::api::Commit,
    },
    Landed(skein_forge::api::Commit),
    Entry(u64),
    Client(skein_forge_client::Key),
    Change(u64),
    Issue(u64),
    Release(u64),
    Application(u64),
    Writer(u16),
    /// The demo's forge, application.demo.forge.
    Demo(u16),
    Binding(Box<[u8]>),
    DemoAct([u8; 16]),
    Flight {
        task: u64,
        entry: u64,
    },
}
/// A completed phase of the connector's restart contract.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestartStage {
    /// Durable connector records and entries are restored.
    Restored,
    /// Every live resource has had its first fresh read.
    ReadAfresh,
    /// Every uncertain restored entry has been looked for.
    Settled,
}
/// Volatile connection information, rebuilt from routing at every start (domain/forge.md, 4.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Connection {
    NotConnected,
    /// An owned forge the box starts when asked, its files absent (protocol/forge.md, 8.1).
    NotSetUp,
    /// The box runs the forge's group, at the step it last named (protocol/forge.md, 2.8).
    SettingUp {
        step: Option<SetUpStep>,
    },
    /// The box's `failed`, or its pipe closed during the ask, with why (protocol/forge.md, 2.8).
    SetUpFailed {
        why: Box<[u8]>,
    },
    Connecting,
    Connected {
        id: u64,
        login: Box<[u8]>,
    },
    Stopped {
        reason: Box<[u8]>,
    },
}
/// The box's word for the step it runs (protocol/setup.md, 7.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetUpStep {
    Fetch,
    Prepare,
    Run,
}
/// The held connection's step when setup failed (domain/forge.md, 4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailedStep {
    Fetch,
    Prepare,
    Run,
    Connect,
}
/// Why a `SetUp` is refused (protocol/forge.md, 2.8; domain/forge.md, 4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetUpRefusal {
    NotOffered,
    NotUnderTheBox,
    Busy,
    NewProject,
    Invalid,
}
/// Installation information for the deployment's page (protocol/forge.md, 2.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Installation {
    pub account: Option<(u64, Box<[u8]>)>,
    pub stray: bool,
    pub suspended: bool,
}
/// A parent or child input to the connector top.
#[derive(PartialEq, Eq, Debug)]
pub enum Event {
    /// The engine stops: finish sent entries without starting another write (domain/forge.md, 6).
    Settle,
    /// A successful conversion kept its App records.
    AppRecorded {
        forge: u16,
        app: AppWay,
    },
    /// Bind the folded member address before minting a sign-in.
    Bind {
        request: Token,
        address: Box<[u8]>,
        subject: Box<[u8]>,
    },
    DemoActs {
        acts: Box<[DemoAct]>,
    },
    DemoActAnswered {
        key: [u8; 16],
        keep: bool,
    },
    SendDemoActs,
    DemoAnswered {
        answer: StartAnswer,
    },
    /// The box ended its pipe (domain/forge.md, 4.1).
    BoxClosed,
    /// The routing's answer to a core-admitted connect request.
    ConnectAnswered {
        forge: u16,
        request: Token,
        answer: ConnectAnswer,
    },
    ReachableAnswered {
        forge: u16,
        request: Token,
        answer: ReachableAnswer,
    },
    AcceptAnswered {
        forge: u16,
        request: Token,
        answer: AcceptAnswer,
        stopped: Option<Box<[u8]>>,
    },
    InstallAnswered {
        forge: u16,
        request: Token,
        answer: InstallAnswer,
        stopped: Option<Box<[u8]>>,
    },
    ConnectEnded {
        stop: Option<AdminStop>,
        forge: u16,
        request: Token,
        connected: bool,
        why: Option<Box<[u8]>>,
    },
    /// The routing's answer to a core-admitted setup request.
    SetUpAnswered {
        forge: u16,
        request: Token,
        answer: SetUpAnswer,
    },
    FirstMade {
        forge: u16,
        id: u64,
    },
    FirstFailed {
        forge: u16,
        step: CreateStep,
    },
    Connection {
        forge: u16,
        state: Connection,
    },
    Installation {
        forge: u16,
        installation: u64,
        state: Option<Installation>,
    },
    /// Decide one core-admitted keyed adoption and keep its outcome.
    Application {
        request: Token,
        token: Token,
        retention: skein_lib::Duration,
        ask: Ask,
        role: Option<jig_domain_people::Role>,
        carried: Option<jig_domain_core::Carried>,
    },
    /// The core's answer to the application's adoption part.
    ApplicationAnswered {
        request: Token,
        outcome: jig_domain_people::Outcome,
    },
    /// Drop a projection only after the core has settled its closing feed.
    ForgetProjection {
        goal: u64,
    },
    /// Keep a connector-owned section source while the core budgets its brief.
    PlanBrief {
        section: Token,
        source: BriefSource,
    },
    /// Gather a planned section with the core's allotted budget.
    GatherPlanned {
        section: Token,
        parts: u32,
        bytes: u32,
        ci_budget: u32,
    },
    /// Gather one pinned, connector-owned section for a run brief.
    GatherBrief {
        owner: Token,
        source: BriefSource,
        parts: u32,
        bytes: u32,
        max_job_bytes: u32,
    },
    /// Gather a section and retain its rendered bytes until the root takes or drops its token.
    GatherBriefHeld {
        section: Token,
        source: BriefSource,
        parts: u32,
        bytes: u32,
        max_job_bytes: u32,
    },
    /// Cut retained words to the brief's allotment, preserving this source's end.
    CutBrief {
        section: Token,
        bytes: u32,
    },
    /// Transfer retained words to the root for a completed assignment.
    TakeBrief {
        section: Token,
    },
    /// Release a section, including one still gathering.
    DropBrief {
        section: Token,
    },
    /// Read current permissions, settings, protection and collaborators.
    Adopt {
        request: Token,
    },
    /// Withdraw a just-adopted repository when its parent cannot commit the role seed.
    ForgetAdoption {
        repository: skein_forge::api::Repository,
        restore: Option<Repository>,
    },
    /// Replace the resources this live task names.
    Names {
        task: u64,
        resources: Box<[Name]>,
    },
    /// Replace the resources of a task still named; a task its release unnamed stays so.
    Rename {
        task: u64,
        resources: Box<[Name]>,
    },
    /// Remove all resources and subscriptions of an ended task.
    Unnamed {
        task: u64,
    },
    /// Take a hold or hand one down from its current holder.
    Hold {
        task: u64,
        resource: Name,
        from: Option<u64>,
    },
    /// Claim writer slots for one task's attempt.
    Claim {
        task: u64,
        attempt: u64,
        writes: Box<[Name]>,
        holders: Box<[u64]>,
    },
    /// Release slots and confirm any pushed heads.
    Answered {
        task: u64,
        attempt: u64,
        pushed: Box<[(Name, skein_forge::api::Commit)]>,
    },
    /// Read a lost writer's resource afresh before making its slot available.
    Lost {
        task: u64,
        attempt: u64,
    },
    /// Retain an explicitly proposed agent effect without making it.
    KeepProposedEffect {
        row: ProposedEffect,
    },
    /// Remove the payload of a terminal proposal.
    DropProposedEffect {
        number: u64,
    },
    /// Enqueue a checked effect in the same decision as its cause.
    Enqueue {
        entry: skein_forge_client::Entry,
    },
    /// Keep a projection candidate after core admission.
    KeepProjection {
        row: IssueRow,
        entry: skein_forge_client::Entry,
    },
    /// Describe the next write for a goal projection.
    Project {
        entry: u64,
        repository: skein_forge::api::Repository,
        view: issues::GoalView,
    },
    /// Continue the latest authorized snapshot after an outbox outcome or interval.
    ProjectDesired {
        entry: u64,
        goal: u64,
    },
    /// Register one activated change task with its held branch.
    Change {
        row: ChangeRow,
    },
    /// Commit the deployment repair identity beside the task creation decision.
    QueueRepairStarted {
        owner: u64,
        repair: u64,
    },
    /// Remember a committed procedure child and its role in this change.
    Delegated {
        task: u64,
        child: u64,
        kind: change::Delegate,
    },
    /// A proposed child was refused by tasks before creation; keep a releasable state.
    DelegateRefused {
        task: u64,
        child: u64,
    },
    /// Resolve the named child's terminal result before stepping again.
    DelegateResult {
        task: u64,
        child: u64,
        status: change::Status,
        words: Box<[u8]>,
    },
    /// Gather fresh forge facts and step one registered change.
    StepChange {
        task: u64,
        entry: u64,
        heard: change::Heard,
        gates: Box<[change::GateReport]>,
        queue_repair_active: bool,
        kept: bool,
    },
    /// A person released a drift-held change after inspecting the new head.
    ReleaseChange {
        task: u64,
    },
    /// Wait for a closing task's prior effects before cleanup begins.
    SettleEffects {
        task: u64,
        root: u64,
        ending: ReleaseEnding,
    },
    /// Release a closing task's holds after prior effects settled.
    Release {
        task: u64,
        root: u64,
        ending: ReleaseEnding,
        reason: Option<Box<[u8]>>,
        entry: u64,
    },
    /// Continue a release after one of its outbox effects settles.
    ContinueRelease {
        task: u64,
        entry: u64,
    },
    /// Lift a held projection after a person resolves its permanent failure.
    ReleaseProjection {
        goal: u64,
    },
    /// The root has durably committed this entry.
    Committed {
        entry: u64,
    },
    /// A person settled a held entry: make it again, or record it not made
    /// (domain/forge.md, 6; jig's `domain/connectors.md`, 4.3).
    Resolve {
        entry: u64,
        again: bool,
    },
    /// Withdraw an entry whose write has not gone out.
    Withdraw {
        entry: u64,
    },
    /// Root authority vetoed a new, unsent change effect in this decision.
    VetoChange {
        task: u64,
        entry: u64,
        prior: change::Change,
    },
    /// Subscribe a live task to a topic.
    Subscribe {
        subscription: Subscriber,
    },
    /// Unsubscribe a task from a topic.
    Unsubscribe {
        task: u64,
        topic: Topic,
    },
    /// One provider hint passed from the protocol.
    Hint {
        hint: skein_forge::api::Hint,
    },
    /// A client event returned after an earlier connector request.
    Client(skein_forge_client::Event),
    /// Restore one durable connector record.
    Restore {
        record: Stored,
    },
    /// Finish restoration and reconcile loaded entries.
    Restored {
        clock: skein_forge_client::RecoveryClock,
    },
    /// Read the system's live resources before procedures step.
    ReadAfresh,
    /// Look for each restored uncertain entry before procedures step.
    SettleOutbox,
}
/// The connector's output to the root, including child API calls.
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    /// The client finished sent entries or reached its bound; those without an outcome.
    Settled {
        unfinished: Box<[Unfinished]>,
    },
    /// The root holds this answer behind the decision's durable commit.
    Bound {
        request: Token,
        answer: BindAnswer,
    },
    /// Select the live chats and deployment account in this decision.
    ListDemoActs,
    SendDemoAct(DemoAct),
    /// Stop the ended demo's box group; no request waits for its answer.
    StopGroup {
        forge: u16,
    },
    /// Start the demo's box group.
    SetUpGroup {
        forge: u16,
        request: Token,
    },
    DemoStart {
        request: Token,
    },
    /// Ask the box link to set up the forge (protocol/forge.md, 2.8).
    SetUp {
        forge: u16,
        request: Token,
        org: Box<[u8]>,
        repository: Box<[u8]>,
    },
    Connect {
        forge: u16,
        request: Token,
        how: How,
        carried: jig_domain_core::Carried,
    },
    Manifest {
        forge: u16,
        request: Token,
        back: Option<Box<[u8]>>,
    },
    Accept {
        forge: u16,
        request: Token,
        installation: core::num::NonZeroU64,
    },
    Install {
        forge: u16,
        request: Token,
        back: Option<Box<[u8]>>,
    },
    Reachable {
        forge: u16,
        request: Token,
        installation: Option<u64>,
        page: u32,
    },
    CarriedUnused {
        number: u64,
    },
    EraseCarried {
        number: u64,
    },
    /// The deployment's forges document changed.
    MarkedForges,
    /// Changed panel subjects, bounded by `MARKED` (domain/root.md, 10).
    Marked {
        tasks: Box<[u64]>,
    },
    ApplicationPart {
        request: Token,
        adoption: Adoption,
        repository: skein_forge::api::Repository,
    },
    ApplicationDone {
        request: Token,
        token: Token,
    },
    ApplicationAgain {
        request: Token,
        why: jig_domain_people::Again,
    },
    /// The connector has fresh news or a retry deadline for its procedure.
    WakeProcedure {
        task: u64,
    },
    /// Current role and hold admission for one named resource.
    Resource {
        project: u32,
        name: Name,
        role: crate::Access,
        hold: crate::resources::HoldKind,
    },

    /// Add a goal's topic through the core's durable task subscriptions.
    GoalTopic {
        goal: u64,
        topic: Topic,
    },
    /// Remove a goal's CI topic after no change names that head.
    GoalUntopic {
        goal: u64,
        topic: Topic,
    },
    /// Feed the next bounded read of a forge section to this connector's client.
    BriefClient {
        event: skein_forge_client::Event,
    },
    /// One connector-owned section is ready for the root to route to the brief.
    BriefReady {
        owner: Token,
        read: BriefRead,
    },
    /// A held section has this rendered size, or its gather/cut failed.
    BriefSized {
        section: Token,
        size: Option<u32>,
    },
    /// A held section transferred its rendered bytes to the root.
    BriefTaken {
        section: Token,
        bytes: Option<Box<[u8]>>,
    },
    /// Terminal result of a repository adoption, for the root's person route.
    Adopted {
        reply_to: Token,
        result: Result<Adopted, skein_forge::api::Error>,
    },
    /// Save one record atomically with the current decision, carrying the
    /// owner's post-commit entry and read-afresh handoffs without inspecting it.
    Save {
        record: Stored,
        release: Option<u64>,
        read_afresh: Option<Name>,
    },
    /// Erase one record atomically with the current decision.
    Erase {
        key: Key,
    },
    /// A resource is held or a writer slot is taken.
    Taken {
        task: u64,
        resource: Name,
        by: u64,
    },
    /// A request exceeds the connector's configured bounds.
    Refused {
        task: u64,
    },
    /// An entry settled or remains uncertain; root routes the task's news.
    Outcome {
        entry: u64,
        task: u64,
        outcome: skein_forge_client::Outcome,
    },
    /// A held entry settled as not made, by a person or its task's close.
    NotMade {
        entry: u64,
        task: u64,
    },
    /// A prior effect or release write settled; advance the closing task.
    ContinueRelease {
        task: u64,
    },
    /// All effects issued before resource cleanup have settled.
    EffectsSettled {
        task: u64,
    },
    /// All connector resources of a closing task are released.
    Released {
        task: u64,
    },
    /// A failed task's resource remains held by its tree root until that root closes.
    Retained {
        task: u64,
        root: u64,
        resource: Name,
    },
    /// A cleanup effect failed and the closing task needs a person's decision.
    ReleaseFailed {
        task: u64,
    },
    /// A connector-described projection candidate, awaiting core admission.
    ProjectionEffect {
        row: IssueRow,
        entry: skein_forge_client::Entry,
        description: crate::ProjectionEffect,
    },
    /// A projection has to wait until its interval or prior effect settles.
    ProjectAfter {
        goal: u64,
        when: Option<skein_lib::Wall>,
    },
    /// The closed projection has no remaining desired write.
    ProjectionSettled {
        goal: u64,
    },
    /// A projection could not continue under current facts or limits.
    ProjectionFailed {
        goal: u64,
    },
    /// A change policy step and its checked effect entry, when any.
    ChangeDecision {
        task: u64,
        decision: change::Decision,
        entry: Option<u64>,
        evidence: Option<ChangeEvidence>,
    },
    /// Task-specific news with a connector classification.
    News {
        task: u64,
        subscription: u64,
        topic: Topic,
        news: News,
        class: Class,
    },
    /// Drift holds this task until a person releases it.
    Drift {
        task: u64,
        resource: Name,
    },
    /// A bounded provider call, released after preceding progress commits.
    Call {
        call: Token,
        repository: skein_forge::api::Repository,
        op: skein_forge::api::Op,
    },
    /// One bounded fresh connector read for a root worker tool.
    Read {
        owner: Token,
        result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    },
    /// The requested restart phase finished; the root may request the next.
    RestartDone {
        stage: RestartStage,
    },
}

/// The demo's durable state (protocol/setup.md, 8.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Demo {
    pub forge: u16,
    pub state: DemoState,
}
/// No record is not started.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoState {
    Running,
    Ended,
}

/// Whether this member address belongs to the presented Google subject.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindAnswer {
    Bound,
    Another,
    /// Every binding row is taken: rows outlive their members (protocol/sign-in.md, 5).
    Full,
}
/// The end's selection, kept before asking the core (protocol/store.md, 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DemoAct {
    pub key: [u8; 16],
    pub act: DemoActKind,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoActKind {
    Close { project: u32, task: u64 },
    RemoveAccount { account: u32 },
}

/// A sent entry still without its outcome when the client’s stop bound passed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unfinished {
    pub entry: u64,
    pub task: u64,
}
