//! Forge phrases as checked English data (client/README.md, 4.4, 4.7 and 5.10).
use super::{Broken, Kinds, Role};
use alloc::boxed::Box;
use skein_lib::{List, Reader, Writer, bytes::copy_of};
use skein_ui::phrases::{Form, Hole, HoleKind, HoleShape, Phrase, PhraseId, PhraseShape, PluralClass, Segment};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Title,
    TitleNumber,
    Forge,
    Owner,
    Name,
    Role,
    OwnedChoice,
    ForkChoice,
    ContextChoice,
    Home,
    Prefix,
    Landing,
    Ci,
    Checks,
    Adopt,
    Required,
    ForgeNumber,
    BadChecks,
    ContextHome,
    NoCheck,
    Long,
    OutOfDate,
    Adopted,
    Owned,
    Fork,
    Context,
    AdoptedRole,
    IsHome,
    May,
    Protection,
    NoSuchForge,
    NotFound,
    NoAccess,
    Refused,
    InFlight,
    TooLarge,
    Unreachable,
    InvalidAnswer,
    Unknown,
    Read,
    Push,
    Open,
    Land,
    Review,
    Status,
    Comment,
    Issue,
    Branch,
    Runs,
    RunsWithoutRunner,
    Password,
    AnotherProject,
    HomeHeld,
    PrefixOccupied,
}
use HoleKind::{Name, Number};
pub type Row = (Word, &'static [&'static [u8]], &'static [HoleKind]);
pub const ROWS: [Row; 54] = [
    (Word::Title, &[b"Adopt a repository into ", b""], &[Name]),
    (Word::TitleNumber, &[b"Adopt a repository into project ", b""], &[Number]),
    (Word::Forge, &[b"Forge number"], &[]),
    (Word::Owner, &[b"Owner"], &[]),
    (Word::Name, &[b"Name"], &[]),
    (Word::Role, &[b"Role"], &[]),
    (Word::OwnedChoice, &[b"Owned: temper pushes, opens pull requests and lands"], &[]),
    (Word::ForkChoice, &[b"Fork: temper contributes through a fork"], &[]),
    (Word::ContextChoice, &[b"Context: temper only reads"], &[]),
    (Word::Home, &[b"Make it the project's home"], &[]),
    (Word::Prefix, &[b"Branch prefix"], &[]),
    (Word::Landing, &[b"Landing branch"], &[]),
    (Word::Ci, &[b"It runs CI"], &[]),
    (Word::Checks, &[b"Checks: gate numbers, separated by spaces or commas"], &[]),
    (Word::Adopt, &[b"Adopt"], &[]),
    (Word::Required, &[b"Required."], &[]),
    (Word::ForgeNumber, &[b"A forge number is from 1 to 65535."], &[]),
    (Word::BadChecks, &[b"Checks are numbers from 1, each once."], &[]),
    (Word::ContextHome, &[b"A context repository cannot be the project's home."], &[]),
    (Word::NoCheck, &[b"Without CI, name at least one check."], &[]),
    (Word::Long, &[b"At most ", b" bytes."], &[Number]),
    (Word::OutOfDate, &[b"This temper is out of date for adopting repositories: run the temper the engine runs."], &[]),
    (Word::Adopted, &[b"Adopted ", b" on forge ", b" as ", b"."], &[Name, Number, Name]),
    (Word::Owned, &[b"owned"], &[]),
    (Word::Fork, &[b"fork"], &[]),
    (Word::Context, &[b"context"], &[]),
    (Word::AdoptedRole, &[b"adopted"], &[]),
    (Word::IsHome, &[b"It is the project's home."], &[]),
    (Word::May, &[b"temper may: ", b"."], &[Name]),
    (Word::Protection, &[b"temper's forge user may not read the branch protection: the forge shows it to repository administrators only."], &[]),
    (Word::NoSuchForge, &[b"No forge ", b" is configured."], &[Name]),
    (Word::NotFound, &[b"No repository ", b" on forge ", b" that temper's forge user can see."], &[Name, Name]),
    (Word::NoAccess, &[b"temper's forge user may not read ", b" on forge ", b"."], &[Name, Name]),
    (
        Word::Refused,
        &[
            b"The forge connector refused it: another project writes ",
            b", the project has a home already, or branches under ",
            b" are not this deployment's.",
        ],
        &[Name, Name],
    ),
    (Word::InFlight, &[b"", b" is being adopted already; try again when that ends."], &[Name]),
    (Word::TooLarge, &[b"A field is past the forge's bounds, or the forge's repositories are full."], &[]),
    (Word::Unreachable, &[b"Forge ", b" did not answer; nothing was adopted. Try again."], &[Name]),
    (Word::InvalidAnswer, &[b"Forge ", b" answered what temper did not expect; nothing was adopted."], &[Name]),
    (Word::Unknown, &[b"It is not known whether ", b" was adopted."], &[Name]),
    (Word::Read, &[b"read"], &[]),
    (Word::Push, &[b"push"], &[]),
    (Word::Open, &[b"open pull requests"], &[]),
    (Word::Land, &[b"land"], &[]),
    (Word::Review, &[b"review"], &[]),
    (Word::Status, &[b"set statuses"], &[]),
    (Word::Comment, &[b"comment"], &[]),
    (Word::Issue, &[b"create issues"], &[]),
    (Word::Branch, &[b"create and delete branches"], &[]),
    (Word::Runs, &[b"Forgejo ", b" and its runner ", b", on this machine. CI jobs run here, as you."], &[Name, Name]),
    (Word::RunsWithoutRunner, &[b"Forgejo ", b", on this machine, with no runner."], &[Name]),
    (Word::Password, &[b"Your first password is in ", b". Change it on Forgejo when you like."], &[Name]),
    (Word::AnotherProject, &[b"Another project writes ", b". Adopt it as context to read it, or use the project that writes it."], &[Name]),
    (Word::HomeHeld, &[b"The project has a home already. Adopt this repository without making it home, or choose a new project."], &[]),
    (Word::PrefixOccupied, &[b"Branches under ", b" are not this deployment's. Choose an unused branch prefix."], &[Name]),
];
fn phrase_id(connector: u16, index: usize) -> PhraseId {
    PhraseId {
        owner: owner(connector),
        number: u16::try_from(index).expect("54 phrases").checked_add(1000).expect("phrase id fits"),
    }
}
pub fn extend(connector: u16, text: u32, shapes: &mut List<PhraseShape>, phrases: &mut List<Phrase>) {
    for (index, (word, literals, kinds)) in ROWS.iter().enumerate() {
        let mut holes = List::with_capacity(3);
        let mut segments = List::with_capacity(7);
        for (index, literal) in literals.iter().enumerate() {
            segments.push(Segment::Text(copy_of(literal))).expect("phrase segments fit");
            if let Some(kind) = kinds.get(index) {
                holes.push(HoleShape { kind: *kind, bytes: name_bytes(*word, index, text) }).expect("holes fit");
                segments
                    .push(Segment::Hole { index: u16::try_from(index).expect("three holes"), kind: *kind })
                    .expect("segments fit");
            }
        }
        let id = phrase_id(connector, index);
        shapes.push(PhraseShape { id, holes: holes.into_boxed(), plural: None }).expect("shapes fit");
        phrases
            .push(Phrase { id, forms: Box::new([Form { class: PluralClass::Other, segments: segments.into_boxed() }]) })
            .expect("phrases fit");
    }
}
#[must_use]
pub fn name_bytes(word: Word, index: usize, text: u32) -> u32 {
    match word {
        // Arbitrary bytes can expand threefold when shown as UTF-8.
        Word::Runs | Word::RunsWithoutRunner => 192,
        Word::Password => super::OWNED_PASSWORD_BYTES,
        Word::Title => text.checked_add(16).expect("checked text bound"),
        Word::Adopted => {
            if index == 0 {
                513
            } else if index == 2 {
                7
            } else {
                0
            }
        }
        Word::May => 128,
        Word::NotFound | Word::NoAccess | Word::Refused => {
            if index == 0 {
                513
            } else {
                256
            }
        }
        Word::InFlight | Word::Unknown | Word::AnotherProject => 513,
        Word::PrefixOccupied | Word::NoSuchForge | Word::Unreachable | Word::InvalidAnswer => 256,
        Word::HomeHeld
        | Word::TitleNumber
        | Word::Forge
        | Word::Owner
        | Word::Name
        | Word::Role
        | Word::OwnedChoice
        | Word::ForkChoice
        | Word::ContextChoice
        | Word::Home
        | Word::Prefix
        | Word::Landing
        | Word::Ci
        | Word::Checks
        | Word::Adopt
        | Word::Required
        | Word::ForgeNumber
        | Word::BadChecks
        | Word::ContextHome
        | Word::NoCheck
        | Word::Long
        | Word::OutOfDate
        | Word::Owned
        | Word::Fork
        | Word::Context
        | Word::AdoptedRole
        | Word::IsHome
        | Word::Protection
        | Word::TooLarge
        | Word::Read
        | Word::Push
        | Word::Open
        | Word::Land
        | Word::Review
        | Word::Status
        | Word::Comment
        | Word::Issue
        | Word::Branch => 0,
    }
}

#[must_use]
pub fn id(connector: u16, word: Word) -> PhraseId {
    for (index, (candidate, _, _)) in ROWS.iter().enumerate() {
        if *candidate == word {
            return phrase_id(connector, index);
        }
    }
    unreachable!("closed phrase vocabulary")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceWord {
    Connector,
    Repository,
    Branch,
    Pull,
    Issue,
    Adopted,
    Path,
    Read,
    Push,
    OpenPull,
    MergePull,
    Review,
    Status,
    Comment,
    CreateIssue,
    CreateBranch,
    EditPull,
    UpdatePull,
    ClosePull,
    EditIssue,
    Reviewers,
    DeleteBranch,
    OtherKind,
    Shown,
    AdoptedBranch,
}
pub const RESOURCES: [ResourceWord; 25] = [
    ResourceWord::Connector,
    ResourceWord::Repository,
    ResourceWord::Branch,
    ResourceWord::Pull,
    ResourceWord::Issue,
    ResourceWord::Adopted,
    ResourceWord::Path,
    ResourceWord::Read,
    ResourceWord::Push,
    ResourceWord::OpenPull,
    ResourceWord::MergePull,
    ResourceWord::Review,
    ResourceWord::Status,
    ResourceWord::Comment,
    ResourceWord::CreateIssue,
    ResourceWord::CreateBranch,
    ResourceWord::EditPull,
    ResourceWord::UpdatePull,
    ResourceWord::ClosePull,
    ResourceWord::EditIssue,
    ResourceWord::Reviewers,
    ResourceWord::DeleteBranch,
    ResourceWord::OtherKind,
    ResourceWord::Shown,
    ResourceWord::AdoptedBranch,
];
impl ResourceWord {
    #[must_use]
    pub fn id(self, connector: u16) -> PhraseId {
        let number = match self {
            Self::Connector => 0,
            Self::Repository => 1,
            Self::Branch => 2,
            Self::Pull => 3,
            Self::Issue => 4,
            Self::Adopted => 5,
            Self::Path => 6,
            Self::Read => 7,
            Self::Push => 8,
            Self::OpenPull => 9,
            Self::MergePull => 10,
            Self::Review => 11,
            Self::Status => 12,
            Self::Comment => 13,
            Self::CreateIssue => 14,
            Self::CreateBranch => 15,
            Self::EditPull => 16,
            Self::UpdatePull => 17,
            Self::ClosePull => 18,
            Self::EditIssue => 19,
            Self::Reviewers => 20,
            Self::DeleteBranch => 21,
            Self::OtherKind => 22,
            Self::Shown => 23,
            Self::AdoptedBranch => 25,
        };
        PhraseId { owner: owner(connector), number }
    }
}
#[must_use]
pub fn resource_english(word: ResourceWord) -> (&'static [&'static [u8]], &'static [HoleKind]) {
    match word {
        ResourceWord::Connector => (&[b"forge"], &[]),
        ResourceWord::Repository => (&[b"", b" on ", b""], &[Name, Name]),
        ResourceWord::Branch => (&[b"branch ", b" of ", b" on ", b""], &[Name, Name, Name]),
        ResourceWord::Pull => (&[b"pull request #", b" of ", b" on ", b""], &[Name, Name, Name]),
        ResourceWord::Issue => (&[b"issue #", b" of ", b" on ", b""], &[Name, Name, Name]),
        ResourceWord::Adopted => (&[b"repository ", b" on forge ", b""], &[Number, Number]),
        ResourceWord::AdoptedBranch => (&[b"branch ", b" of repository ", b" on forge ", b""], &[Name, Number, Number]),
        ResourceWord::Path => (&[b"", b""], &[Name]),
        ResourceWord::Read => (&[b"read"], &[]),
        ResourceWord::Push => (&[b"push"], &[]),
        ResourceWord::OpenPull => (&[b"open a pull request"], &[]),
        ResourceWord::MergePull => (&[b"merge a pull request"], &[]),
        ResourceWord::Review => (&[b"post a review"], &[]),
        ResourceWord::Status => (&[b"set a commit status"], &[]),
        ResourceWord::Comment => (&[b"post a comment"], &[]),
        ResourceWord::CreateIssue => (&[b"create an issue"], &[]),
        ResourceWord::CreateBranch => (&[b"create a branch"], &[]),
        ResourceWord::EditPull => (&[b"edit a pull request"], &[]),
        ResourceWord::UpdatePull => (&[b"update a pull request from its base"], &[]),
        ResourceWord::ClosePull => (&[b"close or reopen a pull request"], &[]),
        ResourceWord::EditIssue => (&[b"edit, close or reopen an issue"], &[]),
        ResourceWord::Reviewers => (&[b"set reviewers"], &[]),
        ResourceWord::DeleteBranch => (&[b"delete a branch"], &[]),
        ResourceWord::OtherKind => (&[b"kind ", b""], &[Number]),
        ResourceWord::Shown => (&[b"Shown on the forge."], &[]),
    }
}
#[must_use]
pub const fn kind(kind: u16) -> Option<ResourceWord> {
    match kind {
        1 => Some(ResourceWord::Read),
        2 => Some(ResourceWord::Push),
        3 => Some(ResourceWord::OpenPull),
        4 => Some(ResourceWord::MergePull),
        5 => Some(ResourceWord::Review),
        6 => Some(ResourceWord::Status),
        7 => Some(ResourceWord::Comment),
        8 => Some(ResourceWord::CreateIssue),
        9 => Some(ResourceWord::CreateBranch),
        10 => Some(ResourceWord::EditPull),
        11 => Some(ResourceWord::UpdatePull),
        12 => Some(ResourceWord::ClosePull),
        13 => Some(ResourceWord::EditIssue),
        14 => Some(ResourceWord::Reviewers),
        15 => Some(ResourceWord::DeleteBranch),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource<'a> {
    Repository { host: &'a [u8], owner: &'a [u8], name: &'a [u8] },
    Branch { host: &'a [u8], owner: &'a [u8], name: &'a [u8], branch: &'a [Box<[u8]>] },
    Pull { host: &'a [u8], owner: &'a [u8], name: &'a [u8], number: &'a [u8] },
    Issue { host: &'a [u8], owner: &'a [u8], name: &'a [u8], number: &'a [u8] },
    Adopted { forge: u16, repository: u32 },
    AdoptedBranch { forge: u16, repository: u32, branch: &'a [Box<[u8]>] },
    Other,
}
#[must_use]
pub fn resource(path: &[Box<[u8]>]) -> Resource<'_> {
    match path {
        [forge, repository] if forge.len() == 2 && repository.len() == 4 => Resource::Adopted {
            forge: Reader::new(forge).u16().expect("two bytes"),
            repository: Reader::new(repository).u32().expect("four bytes"),
        },
        [forge, repository, branch @ ..] if forge.len() == 2 && repository.len() == 4 && !branch.is_empty() => {
            Resource::AdoptedBranch {
                forge: Reader::new(forge).u16().expect("two bytes"),
                repository: Reader::new(repository).u32().expect("four bytes"),
                branch,
            }
        }
        [prefix, host, owner, name] if prefix.as_ref() == b"forge" => Resource::Repository { host, owner, name },
        [prefix, host, owner, name, tag, branch @ ..]
            if prefix.as_ref() == b"forge" && tag.as_ref() == b"branch" && !branch.is_empty() =>
        {
            Resource::Branch { host, owner, name, branch }
        }
        [prefix, host, owner, name, tag, number] if prefix.as_ref() == b"forge" && tag.as_ref() == b"pull" => {
            Resource::Pull { host, owner, name, number }
        }
        [prefix, host, owner, name, tag, number] if prefix.as_ref() == b"forge" && tag.as_ref() == b"issue" => {
            Resource::Issue { host, owner, name, number }
        }
        _ => Resource::Other,
    }
}
#[must_use]
pub fn joined(segments: &[Box<[u8]>], most: u32) -> Box<[u8]> {
    let mut bytes = segments.len().saturating_sub(1);
    for segment in segments {
        bytes = bytes.checked_add(segment.len()).expect("bounded resource path fits");
    }
    let maximum = usize::try_from(most).expect("path text bound fits");
    let mut first = 0;
    let cut = bytes > maximum;
    if cut {
        let available = maximum.saturating_sub(4);
        for (index, segment) in segments.iter().enumerate() {
            if bytes <= available {
                break;
            }
            bytes = bytes.saturating_sub(segment.len()).saturating_sub(1);
            first = index.checked_add(1).expect("path index fits");
        }
    }
    let tail = segments.get(first..).expect("tail lies in path");
    let mut writer = Writer::new(bytes.checked_add(if cut { 4 } else { 0 }).expect("path mark fits"));
    if cut {
        writer.put(b"\xe2\x80\xa6/").expect("path mark counted");
    }
    for (index, segment) in tail.iter().enumerate() {
        if index != 0 {
            writer.put(b"/").expect("separator counted");
        }
        writer.put(segment).expect("segment counted");
    }
    writer.finish()
}
#[must_use]
pub fn full_name(owner: &[u8], name: &[u8]) -> Box<[u8]> {
    let size =
        owner.len().checked_add(name.len()).expect("bounded full name fits").checked_add(1).expect("separator fits");
    let mut writer = Writer::new(size);
    writer.put(owner).expect("owner counted");
    writer.put(b"/").expect("separator counted");
    writer.put(name).expect("name counted");
    writer.finish()
}
#[must_use]
pub fn owner(connector: u16) -> skein_ui::Owner {
    skein_ui::Owner { number: u32::from(connector) }
}
#[must_use]
pub fn path(path: &[Box<[u8]>], most: u32) -> (ResourceWord, Box<[Hole]>) {
    match resource(path) {
        Resource::Repository { host, owner, name } => {
            (ResourceWord::Repository, Box::new([Hole::Name(full_name(owner, name)), Hole::Name(copy_of(host))]))
        }
        Resource::Branch { host, owner, name, branch } => (
            ResourceWord::Branch,
            Box::new([Hole::Name(joined(branch, most)), Hole::Name(full_name(owner, name)), Hole::Name(copy_of(host))]),
        ),
        Resource::Pull { host, owner, name, number } => (
            ResourceWord::Pull,
            Box::new([Hole::Name(copy_of(number)), Hole::Name(full_name(owner, name)), Hole::Name(copy_of(host))]),
        ),
        Resource::Issue { host, owner, name, number } => (
            ResourceWord::Issue,
            Box::new([Hole::Name(copy_of(number)), Hole::Name(full_name(owner, name)), Hole::Name(copy_of(host))]),
        ),
        Resource::Adopted { forge, repository } => {
            (ResourceWord::Adopted, Box::new([Hole::Number(u64::from(repository)), Hole::Number(u64::from(forge))]))
        }
        Resource::AdoptedBranch { forge, repository, branch } => (
            ResourceWord::AdoptedBranch,
            Box::new([
                Hole::Name(joined(branch, most)),
                Hole::Number(u64::from(repository)),
                Hole::Number(u64::from(forge)),
            ]),
        ),
        Resource::Other => (ResourceWord::Path, Box::new([Hole::Name(joined(path, most))])),
    }
}
#[must_use]
pub fn broken(broken: Option<Broken>) -> Option<(Word, Box<[Hole]>)> {
    let word = match broken {
        None => return None,
        Some(Broken::Required) => Word::Required,
        Some(Broken::ForgeNumber) => Word::ForgeNumber,
        Some(Broken::Checks) => Word::BadChecks,
        Some(Broken::ContextHome) => Word::ContextHome,
        Some(Broken::NoCheck) => Word::NoCheck,
        Some(Broken::Long { bound }) => return Some((Word::Long, Box::new([Hole::Number(u64::from(bound))]))),
    };
    Some((word, Box::new([])))
}
#[must_use]
pub const fn kinds(kinds: Kinds) -> [(bool, Word); 9] {
    [
        (kinds.read, Word::Read),
        (kinds.push, Word::Push),
        (kinds.open, Word::Open),
        (kinds.land, Word::Land),
        (kinds.review, Word::Review),
        (kinds.status, Word::Status),
        (kinds.comment, Word::Comment),
        (kinds.issue, Word::Issue),
        (kinds.branch, Word::Branch),
    ]
}
#[must_use]
pub const fn role(role: Role) -> Word {
    match role {
        Role::Owned => Word::Owned,
        Role::Adopted => Word::AdoptedRole,
        Role::Fork => Word::Fork,
        Role::Context => Word::Context,
    }
}

/// The change page's vocabulary (client/README.md, 5.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeWord {
    Tag,
    Pull,
    NoPull,
    Repository,
    View,
    Loading,
    Unknown,
    Ended,
    Producing,
    Opening,
    Checking,
    Gating,
    Queued,
    Landed,
    QueuedSt,
    QueuedNd,
    QueuedRd,
    QueuedTh,
    ProduceLine,
    OpenLine,
    RecreateLine,
    ReopenLine,
    CheckLine,
    GateLine,
    QueueSt,
    QueueNd,
    QueueRd,
    QueueTh,
    After,
    Comma,
    And,
    More,
    Tasks,
    Period,
    FirstLine,
    UpdateLine,
    ResolveLine,
    RepairCi,
    RepairGate,
    RepairSemantic,
    LandingLine,
    LandedLine,
    Held,
    HeldProducing,
    HeldOpening,
    HeldRecreating,
    HeldReopening,
    HeldChecking,
    HeldGating,
    HeldQueued,
    HeldFirst,
    HeldUpdating,
    HeldResolving,
    HeldRepairing,
    HeldLanding,
    HeldLanded,
    Stalled,
    RepairsSpent,
    ResolutionsSpent,
    UpdatesSpent,
    Moved,
    Missing,
    Closed,
    Retargeted,
    FailedHold,
    Rejected,
    Gates,
    Ci,
    NotYet,
    NotRead,
    Running,
    Passed,
    Failed,
    ReviewBlocking,
    ReviewAdvisory,
    ApprovalBlocking,
    ApprovalAdvisory,
    CheckBlocking,
    CheckAdvisory,
    Waiting,
    Approved,
    Changes,
    NoGate,
    Repairs,
    Resolutions,
    Updates,
    Separator,
    EndedLine,
    AuthorityHold,
    ReadyLine,
    ReadyAloneLine,
    PrepareLine,
    HeldReady,
    HeldPreparing,
}
pub type ChangeRow = (ChangeWord, &'static [&'static [u8]], &'static [HoleKind]);
pub const CHANGES: [ChangeRow; 94] = [
    (ChangeWord::Tag, &[b"Change \xc2\xb7 Task ", b""], &[Number]),
    (ChangeWord::Pull, &[b"PR #", b" \xc2\xb7 "], &[Number]),
    (ChangeWord::NoPull, &[b"No pull request yet \xc2\xb7 "], &[]),
    (ChangeWord::Repository, &[b"", b" \xc2\xb7 ", b" \xe2\x86\x92 ", b""], &[Name, Name, Name]),
    (ChangeWord::View, &[b"View on forge \xe2\x86\x97"], &[]),
    (ChangeWord::Loading, &[b"Loading the change from the forge."], &[]),
    (ChangeWord::Unknown, &[b"The forge knows no change for this task."], &[]),
    (ChangeWord::Ended, &[b"The forge cannot show this change."], &[]),
    (ChangeWord::Producing, &[b"Producing"], &[]),
    (ChangeWord::Opening, &[b"Opening"], &[]),
    (ChangeWord::Checking, &[b"Checking"], &[]),
    (ChangeWord::Gating, &[b"Gating"], &[]),
    (ChangeWord::Queued, &[b"Queued"], &[]),
    (ChangeWord::Landed, &[b"Landed"], &[]),
    (ChangeWord::QueuedSt, &[b"Queued \xc2\xb7 ", b"st"], &[Number]),
    (ChangeWord::QueuedNd, &[b"Queued \xc2\xb7 ", b"nd"], &[Number]),
    (ChangeWord::QueuedRd, &[b"Queued \xc2\xb7 ", b"rd"], &[Number]),
    (ChangeWord::QueuedTh, &[b"Queued \xc2\xb7 ", b"th"], &[Number]),
    (ChangeWord::ProduceLine, &[b"Producing its branch."], &[]),
    (ChangeWord::OpenLine, &[b"Opening its pull request."], &[]),
    (ChangeWord::RecreateLine, &[b"Making its branch again at the head temper last knew."], &[]),
    (ChangeWord::ReopenLine, &[b"Reopening its pull request."], &[]),
    (ChangeWord::CheckLine, &[b"Waiting for CI at its head."], &[]),
    (ChangeWord::GateLine, &[b"Waiting for its gates at its head."], &[]),
    (ChangeWord::QueueSt, &[b"", b"st in the queue for ", b""], &[Number, Name]),
    (ChangeWord::QueueNd, &[b"", b"nd in the queue for ", b""], &[Number, Name]),
    (ChangeWord::QueueRd, &[b"", b"rd in the queue for ", b""], &[Number, Name]),
    (ChangeWord::QueueTh, &[b"", b"th in the queue for ", b""], &[Number, Name]),
    (ChangeWord::After, &[b", after "], &[]),
    (ChangeWord::Comma, &[b", "], &[]),
    (ChangeWord::And, &[b" and "], &[]),
    (ChangeWord::More, &[b"", b" more"], &[Number]),
    (ChangeWord::Tasks, &[b"", b" tasks"], &[Number]),
    (ChangeWord::Period, &[b"."], &[]),
    (ChangeWord::FirstLine, &[b"First in the queue for ", b"."], &[Name]),
    (ChangeWord::UpdateLine, &[b"Updating from ", b"."], &[Name]),
    (ChangeWord::ResolveLine, &[b"Resolving its conflicts with ", b"."], &[Name]),
    (ChangeWord::RepairCi, &[b"Repairing: CI failed at its head."], &[]),
    (ChangeWord::RepairGate, &[b"Repairing: gate ", b" asked for changes."], &[Number]),
    (ChangeWord::RepairSemantic, &[b"Repairing: a semantic conflict with ", b"."], &[Name]),
    (ChangeWord::LandingLine, &[b"Merging at its head."], &[]),
    (ChangeWord::LandedLine, &[b"Landed \xc2\xb7 merge commit ", b""], &[Name]),
    (ChangeWord::Held, &[b"Held while ", b": ", b"."], &[Name, Name]),
    (ChangeWord::HeldProducing, &[b"producing"], &[]),
    (ChangeWord::HeldOpening, &[b"opening"], &[]),
    (ChangeWord::HeldRecreating, &[b"recreating"], &[]),
    (ChangeWord::HeldReopening, &[b"reopening"], &[]),
    (ChangeWord::HeldChecking, &[b"checking"], &[]),
    (ChangeWord::HeldGating, &[b"gating"], &[]),
    (ChangeWord::HeldQueued, &[b"queued"], &[]),
    (ChangeWord::HeldFirst, &[b"first in the queue"], &[]),
    (ChangeWord::HeldUpdating, &[b"updating"], &[]),
    (ChangeWord::HeldResolving, &[b"resolving"], &[]),
    (ChangeWord::HeldRepairing, &[b"repairing"], &[]),
    (ChangeWord::HeldLanding, &[b"landing"], &[]),
    (ChangeWord::HeldLanded, &[b"landed"], &[]),
    (ChangeWord::Stalled, &[b"its wait stalled"], &[]),
    (ChangeWord::RepairsSpent, &[b"its repairs are spent"], &[]),
    (ChangeWord::ResolutionsSpent, &[b"its resolutions are spent"], &[]),
    (ChangeWord::UpdatesSpent, &[b"its updates are spent"], &[]),
    (ChangeWord::Moved, &[b"its branch was moved on the forge"], &[]),
    (ChangeWord::Missing, &[b"its branch is missing on the forge"], &[]),
    (ChangeWord::Closed, &[b"its pull request was closed on the forge"], &[]),
    (ChangeWord::Retargeted, &[b"its pull request was retargeted on the forge"], &[]),
    (ChangeWord::FailedHold, &[b"a delegate or a write failed for good"], &[]),
    (ChangeWord::Rejected, &[b"a person rejected a gate"], &[]),
    (ChangeWord::Gates, &[b"Gates at its head"], &[]),
    (ChangeWord::Ci, &[b"CI at its head"], &[]),
    (ChangeWord::NotYet, &[b"Not yet"], &[]),
    (ChangeWord::Running, &[b"Running"], &[]),
    (ChangeWord::Passed, &[b"Passed"], &[]),
    (ChangeWord::Failed, &[b"Failed"], &[]),
    (ChangeWord::ReviewBlocking, &[b"Gate ", b" \xc2\xb7 review \xc2\xb7 blocking"], &[Number]),
    (ChangeWord::ReviewAdvisory, &[b"Gate ", b" \xc2\xb7 review \xc2\xb7 advisory"], &[Number]),
    (ChangeWord::ApprovalBlocking, &[b"Gate ", b" \xc2\xb7 approval \xc2\xb7 blocking"], &[Number]),
    (ChangeWord::ApprovalAdvisory, &[b"Gate ", b" \xc2\xb7 approval \xc2\xb7 advisory"], &[Number]),
    (ChangeWord::CheckBlocking, &[b"Gate ", b" \xc2\xb7 check \xc2\xb7 blocking"], &[Number]),
    (ChangeWord::CheckAdvisory, &[b"Gate ", b" \xc2\xb7 check \xc2\xb7 advisory"], &[Number]),
    (ChangeWord::Waiting, &[b"Waiting"], &[]),
    (ChangeWord::Approved, &[b"Approved"], &[]),
    (ChangeWord::Changes, &[b"Changes asked"], &[]),
    (ChangeWord::NoGate, &[b"No other gate."], &[]),
    (ChangeWord::Repairs, &[b"Repairs ", b" of ", b""], &[Number, Number]),
    (ChangeWord::Resolutions, &[b"resolutions ", b" of ", b""], &[Number, Number]),
    (ChangeWord::Updates, &[b"updates ", b" of ", b""], &[Number, Number]),
    (ChangeWord::Separator, &[b" \xc2\xb7 "], &[]),
    (ChangeWord::NotRead, &[b"Not read since the restart"], &[]),
    (ChangeWord::EndedLine, &[b"Change ended."], &[]),
    (ChangeWord::AuthorityHold, &[b"", b""], &[Name]),
    (ChangeWord::ReadyLine, &[b"Ready: its landing, task ", b", lands it."], &[Number]),
    (ChangeWord::ReadyAloneLine, &[b"Ready: waiting for room for its landing."], &[]),
    (ChangeWord::PrepareLine, &[b"Being rebased onto ", b" for its landing."], &[Name]),
    (ChangeWord::HeldReady, &[b"ready"], &[]),
    (ChangeWord::HeldPreparing, &[b"preparing"], &[]),
];
impl ChangeWord {
    #[must_use]
    pub fn id(self, connector: u16) -> PhraseId {
        for (index, (word, _, _)) in CHANGES.iter().enumerate() {
            if *word == self {
                return PhraseId {
                    owner: owner(connector),
                    number: 2000_u16
                        .checked_add(u16::try_from(index).expect("change vocabulary fits"))
                        .expect("change id fits"),
                };
            }
        }
        unreachable!("closed change vocabulary")
    }
}
/// Byte bounds of the change's named holes; number holes carry no bytes.
#[must_use]
#[expect(clippy::too_many_lines, reason = "exhaustive change vocabulary with its name bounds")]
pub fn change_name_bytes(word: ChangeWord, index: usize) -> u32 {
    match word {
        ChangeWord::Repository => {
            if index == 0 {
                513
            } else {
                256
            }
        }
        ChangeWord::QueueSt | ChangeWord::QueueNd | ChangeWord::QueueRd | ChangeWord::QueueTh if index == 1 => 256,
        ChangeWord::Held if index == 0 => 18,
        ChangeWord::PrepareLine
        | ChangeWord::FirstLine
        | ChangeWord::UpdateLine
        | ChangeWord::ResolveLine
        | ChangeWord::RepairSemantic => 256,
        ChangeWord::AuthorityHold | ChangeWord::Held => 512,
        ChangeWord::LandedLine => 7,
        ChangeWord::ReadyLine
        | ChangeWord::ReadyAloneLine
        | ChangeWord::HeldReady
        | ChangeWord::HeldPreparing
        | ChangeWord::QueueSt
        | ChangeWord::QueueNd
        | ChangeWord::QueueRd
        | ChangeWord::QueueTh
        | ChangeWord::Tag
        | ChangeWord::Pull
        | ChangeWord::NoPull
        | ChangeWord::View
        | ChangeWord::Loading
        | ChangeWord::Unknown
        | ChangeWord::Ended
        | ChangeWord::EndedLine
        | ChangeWord::Producing
        | ChangeWord::Opening
        | ChangeWord::Checking
        | ChangeWord::Gating
        | ChangeWord::Queued
        | ChangeWord::Landed
        | ChangeWord::QueuedSt
        | ChangeWord::QueuedNd
        | ChangeWord::QueuedRd
        | ChangeWord::QueuedTh
        | ChangeWord::ProduceLine
        | ChangeWord::OpenLine
        | ChangeWord::RecreateLine
        | ChangeWord::ReopenLine
        | ChangeWord::CheckLine
        | ChangeWord::GateLine
        | ChangeWord::After
        | ChangeWord::Comma
        | ChangeWord::And
        | ChangeWord::More
        | ChangeWord::Tasks
        | ChangeWord::Period
        | ChangeWord::RepairCi
        | ChangeWord::RepairGate
        | ChangeWord::LandingLine
        | ChangeWord::HeldProducing
        | ChangeWord::HeldOpening
        | ChangeWord::HeldRecreating
        | ChangeWord::HeldReopening
        | ChangeWord::HeldChecking
        | ChangeWord::HeldGating
        | ChangeWord::HeldQueued
        | ChangeWord::HeldFirst
        | ChangeWord::HeldUpdating
        | ChangeWord::HeldResolving
        | ChangeWord::HeldRepairing
        | ChangeWord::HeldLanding
        | ChangeWord::HeldLanded
        | ChangeWord::Stalled
        | ChangeWord::RepairsSpent
        | ChangeWord::ResolutionsSpent
        | ChangeWord::UpdatesSpent
        | ChangeWord::Moved
        | ChangeWord::Missing
        | ChangeWord::Closed
        | ChangeWord::Retargeted
        | ChangeWord::FailedHold
        | ChangeWord::Rejected
        | ChangeWord::Gates
        | ChangeWord::Ci
        | ChangeWord::NotYet
        | ChangeWord::NotRead
        | ChangeWord::Running
        | ChangeWord::Passed
        | ChangeWord::Failed
        | ChangeWord::ReviewBlocking
        | ChangeWord::ReviewAdvisory
        | ChangeWord::ApprovalBlocking
        | ChangeWord::ApprovalAdvisory
        | ChangeWord::CheckBlocking
        | ChangeWord::CheckAdvisory
        | ChangeWord::Waiting
        | ChangeWord::Approved
        | ChangeWord::Changes
        | ChangeWord::NoGate
        | ChangeWord::Repairs
        | ChangeWord::Resolutions
        | ChangeWord::Updates
        | ChangeWord::Separator => 0,
    }
}
pub fn extend_change(connector: u16, shapes: &mut List<PhraseShape>, phrases: &mut List<Phrase>) {
    for (word, literals, kinds) in CHANGES {
        let mut holes = List::with_capacity(3);
        let mut segments = List::with_capacity(7);
        for (index, literal) in literals.iter().enumerate() {
            segments.push(Segment::Text(copy_of(literal))).expect("change segments fit");
            if let Some(kind) = kinds.get(index) {
                holes.push(HoleShape { kind: *kind, bytes: change_name_bytes(word, index) }).expect("change holes fit");
                segments
                    .push(Segment::Hole { index: u16::try_from(index).expect("three holes"), kind: *kind })
                    .expect("change segments fit");
            }
        }
        let id = word.id(connector);
        shapes
            .push(PhraseShape {
                id,
                holes: holes.into_boxed(),
                plural: if word == ChangeWord::Tasks { Some(0) } else { None },
            })
            .expect("change shapes fit");
        let other = Form { class: PluralClass::Other, segments: segments.into_boxed() };
        let forms: Box<[Form]> = if word == ChangeWord::Tasks {
            Box::new([
                Form {
                    class: PluralClass::One,
                    segments: Box::new([
                        Segment::Text(copy_of(b"")),
                        Segment::Hole { index: 0, kind: Number },
                        Segment::Text(copy_of(b" task")),
                    ]),
                },
                other,
            ])
        } else {
            Box::new([other])
        };
        phrases.push(Phrase { id, forms }).expect("change phrases fit");
    }
}

/// The connection table's words (client/forge.md, 5.4), shared by every client.
#[must_use]
pub const fn forge_kind(kind: super::forges::Kind) -> &'static [u8] {
    match kind {
        super::forges::Kind::Forgejo => b"Forgejo",
        super::forges::Kind::GitHub => b"GitHub",
    }
}
#[must_use]
pub const fn forge_mode(mode: super::forges::Mode) -> &'static [u8] {
    use super::forges::Mode;
    match mode {
        Mode::Connected => b"connected",
        Mode::Managed => b"managed",
        Mode::Owned => b"owned",
        Mode::Fake => b"fake",
    }
}
#[must_use]
pub const fn set_up_refusal(why: super::forges::SetUpRefusal) -> &'static [u8] {
    use super::forges::SetUpRefusal;
    match why {
        SetUpRefusal::NotOffered => b"not offered",
        SetUpRefusal::NotUnderTheBox => b"not under the box",
        SetUpRefusal::Busy => b"busy",
        SetUpRefusal::NewProject => b"a new project",
        SetUpRefusal::Invalid => b"invalid: the organization, repository or project name is empty or too long",
    }
}
#[must_use]
pub const fn set_up_outcome(outcome: super::forges::SetUpOutcome) -> &'static [u8] {
    use super::forges::SetUpOutcome;
    match outcome {
        SetUpOutcome::SettingUp => b"setting up",
        SetUpOutcome::Refused(why) => set_up_refusal(why),
        SetUpOutcome::Unknown => b"unknown",
    }
}
/// The manual steps, also printed beside a set-up refusal without the box.
#[must_use]
pub fn set_up_commands(number: u16) -> Box<[u8]> {
    let n = skein_lib::Decimal::of(u64::from(number));
    let n = n.as_bytes();
    let parts: &[&[u8]] = &[
        b"run `temper forge fetch ",
        n,
        b"`, `temper forge prepare ",
        n,
        b"`, `temper forge run ",
        n,
        b" forgejo` and `temper forge run ",
        n,
        b" runner`, then restart `temper serve`",
    ];
    join_words(parts)
}
fn join_words(parts: &[&[u8]]) -> Box<[u8]> {
    let mut size = 0usize;
    for part in parts {
        size = size.checked_add(part.len()).expect("bounded forge words");
    }
    let mut out = Writer::new(size);
    for part in parts {
        out.put(part).expect("measured words");
    }
    out.finish()
}
/// A connected row's words on the Forges page, "Connected as LOGIN" (client/forge.md, 5.4).
#[must_use]
pub fn forge_connected_as(login: &[u8]) -> Box<[u8]> {
    join_words(&[b"Connected as ", &lossy(login)])
}
/// A connected row's Account line, "LOGIN · user ID" (client/forge.md, 5.4).
#[must_use]
pub fn forge_account(login: &[u8], id: u64) -> Box<[u8]> {
    join_words(&[&lossy(login), b" \xc2\xb7 user ", skein_lib::Decimal::of(id).as_bytes()])
}
/// What runs on this machine for an owned forge (client/forge.md, 5.4).
#[must_use]
pub fn forge_runs(owned: &super::forges::Owned) -> Box<[u8]> {
    match &owned.runner {
        Some(runner) => join_words(&[
            b"Forgejo ",
            &lossy(&owned.forgejo),
            b" and its runner ",
            &lossy(runner),
            b", on this machine. CI jobs run here, as you.",
        ]),
        None => join_words(&[b"Forgejo ", &lossy(&owned.forgejo), b", on this machine, with no runner."]),
    }
}
/// The first password's file path, never its value (protocol/setup.md, 6.5).
#[must_use]
pub fn forge_password(path: &[u8]) -> Box<[u8]> {
    join_words(&[b"Your first password is in ", &lossy(path), b". Change it on Forgejo when you like."])
}
/// A row's summary, its "forge N · KIND" and its connection's words, joined
/// here: a login or a reason can outgrow any phrase hole once shown as UTF-8.
#[must_use]
pub fn forge_summary(head: &[u8], connection: &[u8]) -> Box<[u8]> {
    join_words(&[head, b" \xc2\xb7 ", connection])
}
#[must_use]
pub fn forge_connection(row: &super::forges::Row) -> Box<[u8]> {
    use super::forges::{Connection as C, Offer, Step};
    if let Some(super::forges::First { state: super::forges::FirstState::Failed { step }, .. }) = &row.first {
        return join_words(&[b"Set up failed: ", create_step(*step)]);
    }
    let mut has_accepted = false;
    for installation in &row.installations {
        has_accepted |= !installation.stray;
    }
    match &row.connection {
        C::NotConnected => Box::from(b"Not connected".as_slice()),
        C::NotSetUp => {
            if row.offer == Offer::NotUnderTheBox {
                join_words(&[b"Not set up: ", &set_up_commands(row.number)])
            } else {
                Box::from(b"Not set up".as_slice())
            }
        }
        C::SettingUp { step } => Box::from(match step {
            Some(Step::Fetch) => b"Setting up: fetching".as_slice(),
            Some(Step::Prepare) => b"Setting up: preparing".as_slice(),
            Some(Step::Run) => b"Setting up: starting".as_slice(),
            None => b"Setting up".as_slice(),
        }),
        C::SetUpFailed { why, .. } => join_words(&[b"Set up failed: ", &lossy(why)]),
        C::Connecting => Box::from(b"Connecting".as_slice()),
        C::Connected { .. } if row.kind == super::forges::Kind::GitHub && !has_accepted => {
            Box::from(if row.installations.is_empty() {
                b"Connecting: installed nowhere yet".as_slice()
            } else {
                b"Connecting: no installation accepted yet".as_slice()
            })
        }
        C::Connected { id, login } => {
            join_words(&[b"Connected as ", &lossy(login), b" (", skein_lib::Decimal::of(*id).as_bytes(), b")"])
        }
        C::Stopped { reason } => join_words(&[b"Stopped: ", &lossy(reason)]),
    }
}

#[must_use]
pub const fn forge_set_up_action(row: &super::forges::Row) -> Option<&'static [u8]> {
    use super::forges::{Connection, Offer};
    match (&row.connection, row.offer) {
        (Connection::NotSetUp, Offer::Offered) => Some(b"Set up a Forgejo here"),
        (Connection::SetUpFailed { .. }, Offer::Offered) => Some(b"Set up again"),
        _ => None,
    }
}

// Keep owned byte strings under the domain's subset while showing invalid UTF-8
// with the same replacement character as the clients' text shells.
#[must_use]
pub fn lossy(bytes: &[u8]) -> Box<[u8]> {
    let mut size = 0usize;
    for chunk in bytes.utf8_chunks() {
        size = size.checked_add(chunk.valid().len()).expect("bounded forge words");
        if !chunk.invalid().is_empty() {
            size = size.checked_add(3).expect("replacement fits");
        }
    }
    let mut out = Writer::new(size);
    for chunk in bytes.utf8_chunks() {
        out.put(chunk.valid().as_bytes()).expect("measured words");
        if !chunk.invalid().is_empty() {
            out.put(b"\xef\xbf\xbd").expect("measured replacement");
        }
    }
    out.finish()
}

/// The first repository's step, in the row's words (client/forge.md, 5.4).
#[must_use]
pub const fn create_step(step: super::forges::CreateStep) -> &'static [u8] {
    use super::forges::CreateStep;
    match step {
        CreateStep::Admin => b"checking the admin's token",
        CreateStep::Org => b"making the organization",
        CreateStep::Repository => b"making the repository",
        CreateStep::File => b"adding the starter workflow",
    }
}
#[must_use]
pub fn first_repository(first: &super::forges::First) -> Box<[u8]> {
    use super::forges::{FirstState, SetUpProject};
    let name = full_name(&lossy(&first.org), &lossy(&first.repository));
    match first.state {
        FirstState::Making => join_words(&[b"Making ", &name]),
        FirstState::Made { .. } => {
            let project = match &first.project {
                SetUpProject::Existing(number) => Box::from(skein_lib::Decimal::of(u64::from(*number)).as_bytes()),
                SetUpProject::New(name) => lossy(name),
            };
            join_words(&[&name, b" made, in ", &project])
        }
        FirstState::Failed { step } => join_words(&[b"Making ", &name, b" failed: ", create_step(step)]),
    }
}

/// GitHub App presentation from the row's facts (client/forge.md, 5.4).
#[must_use]
pub fn app_line(row: &super::forges::Row) -> Option<Box<[u8]>> {
    let slug = super::forges::slug(row)?;
    let mut line = join_words(&[&lossy(slug), b" \xc2\xb7 "]);
    if row.installations.is_empty() {
        return Some(join_words(&[&line, b"installed nowhere yet"]));
    }
    for (index, installation) in row.installations.iter().enumerate() {
        let separator: &[u8] = if index == 0 { b"" } else { b", " };
        line = join_words(&[&line, separator, &installation_words(installation, row.read_only)]);
    }
    Some(line)
}
#[must_use]
pub fn installation_words(installation: &super::forges::Installation, read_only: bool) -> Box<[u8]> {
    let account = match &installation.account {
        Some(account) => lossy(&account.login),
        None => join_words(&[b"installation ", skein_lib::Decimal::of(installation.id).as_bytes()]),
    };
    join_words(&[
        &account,
        if installation.suspended { b" \xc2\xb7 suspended" } else { b"" },
        if installation.stray {
            if read_only {
                b" \xc2\xb7 not accepted \xc2\xb7 kept: the App is read-only"
            } else {
                b" \xc2\xb7 not accepted \xc2\xb7 uninstalled at the next reconcile"
            }
        } else {
            b""
        },
    ])
}
#[must_use]
pub fn last_connect(row: &super::forges::Row) -> Option<Box<[u8]>> {
    Some(join_words(&[b"The last connect stopped: ", &lossy(row.ended.as_ref()?)]))
}
#[must_use]
pub fn app_refusal(why: super::github::Answer, forge: u16) -> Option<Box<[u8]>> {
    use super::github::Answer;
    use temper_client_protocol_wire::v1::ForgeRefusal as F;
    let number = skein_lib::Decimal::of(u64::from(forge));
    Some(match why {
        Answer::Refused(F::Role) => join_words(&[b"Only the deployment's owner connects a forge."]),
        Answer::Refused(F::NoSuchForge) => join_words(&[b"No forge ", number.as_bytes(), b" is configured."]),
        Answer::Refused(F::NotTaken) => join_words(&[b"Forge ", number.as_bytes(), b" does not take an App."]),
        Answer::Refused(F::Connected) => join_words(&[b"GitHub is connected already."]),
        Answer::Refused(F::Busy) => {
            join_words(&[b"temper is busy with forge ", number.as_bytes(), b": try again when it ends."])
        }
        Answer::Refused(F::NoKey) => join_words(&[b"The key did not reach temper: paste it again."]),
        Answer::Refused(F::FirstIsBot) => join_words(&[b"That organization has the name of temper's bot on this forge, and on Forgejo users and organizations share names: name another organization."]),
        Answer::Refused(F::NotConnected) => join_words(&[b"Connect the App first."]),
        Answer::NoAccess => join_words(&[
            b"GitHub refused temper's read of the App's repositories: the App may be suspended. See Forges.",
        ]),
        Answer::NotFound => join_words(&[b"GitHub no longer has this installation: install the App again."]),
        Answer::Unknown => join_words(&[b"It is not known whether the App was connected: see Forges."]),
        Answer::Connecting | Answer::Link(_) | Answer::Page(_) | Answer::Entrance(_) => return None,
    })
}
#[must_use]
pub fn app_terminal(forge: u16) -> Box<[u8]> {
    join_words(&[
        b"Paste the key on this page in a browser, or run temper ctl DIR forge connect ",
        skein_lib::Decimal::of(u64::from(forge)).as_bytes(),
        b" github-app ID FILE --key KEY, FILE the key's file, readable by you alone.",
    ])
}
#[must_use]
pub fn app_install(row: &super::forges::Row) -> Box<[u8]> {
    join_words(&[
        &lossy(super::forges::slug(row).unwrap_or(b"")),
        b" is connected to temper, and installed nowhere yet. Install it, and choose the repositories temper may use.",
    ])
}
pub const APP_ID: &[u8] = b"An App's id is a number, as GitHub shows it on the App's page.";
pub const APP_TOO_LONG: &[u8] = b"The pasted values are past 4 KiB, the most a request carries.";

#[must_use]
pub fn app_head(slug: &[u8], empty: bool) -> Box<[u8]> {
    join_words(&[&lossy(slug), b" \xc2\xb7 ", if empty { b"installed nowhere yet" } else { b"" }])
}
#[must_use]
pub fn app_installation(installation: &super::forges::Installation, first: bool, read_only: bool) -> Box<[u8]> {
    join_words(&[if first { b"" } else { b", " }, &installation_words(installation, read_only)])
}
