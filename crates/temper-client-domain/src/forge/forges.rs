//! The deployment's configured forges and their set up (client/README.md, 4.6).
use super::{family, wire_limits};
use alloc::boxed::Box;
use jig_client::{Answered, Ask, Family, Watchable, contract as c};
use temper_client_protocol_wire::{self as wire, v1 as w};
/// The longest setup reason rendered with one replacement per invalid byte.
pub const FAILURE_BYTES: u32 = wire::SET_UP_WHY_BYTES.checked_mul(3).expect("reason text fits");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailedStep {
    Fetch,
    Prepare,
    Run,
    Connect,
}

/// The wire bounds the session's forge rows.
pub const ROWS: u32 = wire::FORGES;
pub const INSTALLATIONS: u32 = wire::INSTALLATIONS;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Forgejo,
    GitHub,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Connected,
    Managed,
    Owned,
    Fake,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Fetch,
    Prepare,
    Run,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    NotOffered,
    NotUnderTheBox,
    Offered,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    NotConnected,
    NotSetUp,
    SettingUp { step: Option<Step> },
    SetUpFailed { why: Box<[u8]>, at: Option<FailedStep> },
    Connecting,
    Connected { id: u64, login: Box<[u8]> },
    Stopped { reason: Box<[u8]> },
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
    pub playground: bool,
    pub org: Box<[u8]>,
    pub repository: Box<[u8]>,
    pub project: SetUpProject,
    pub state: FirstState,
    pub label: Box<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: u64,
    pub login: Box<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    pub id: u64,
    pub account: Option<Account>,
    pub suspended: bool,
    pub stray: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppWay {
    Manifest,
    Pasted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adoption {
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub project: u32,
}
/// Public owned-forge versions and first-password file path (client/forge.md, 5.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub forgejo: Box<[u8]>,
    pub runner: Option<Box<[u8]>>,
    pub password: Box<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub read_only: bool,
    pub number: u16,
    pub kind: Kind,
    pub mode: Mode,
    pub connection: Connection,
    pub offer: Offer,
    pub first: Option<First>,
    pub installations: Box<[Installation]>,
    pub ended: Option<Box<[u8]>>,
    pub app: Option<AppWay>,
    pub web: Option<Box<[u8]>>,
    pub repositories: Box<[Adoption]>,
    pub owned: Option<Owned>,
    pub admin: bool,
    pub ended_stop: Option<AdminStop>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetUpOutcome {
    SettingUp,
    Refused(SetUpRefusal),
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetUpRefusal {
    NotOffered,
    NotUnderTheBox,
    Busy,
    NewProject,
    Invalid,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetUpAnswer {
    Outcome(SetUpOutcome),
    EntranceRefused(c::EntranceRefusal),
}
#[must_use]
pub fn watch(limits: &super::Limits) -> Option<Watchable> {
    let limits = wire_limits(limits)?;
    let watching = w::Watching::new(&limits, w::WatchingParts { watch: w::Watch::Forges }).ok()?;
    Some(Watchable::Family {
        scope: c::Scope::Deployment,
        window: c::Window { rows: 0, after: None },
        record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::watching(&watching) },
    })
}
#[must_use]
pub fn ask(
    forge: u16,
    org: Box<[u8]>,
    repository: Box<[u8]>,
    project: SetUpProject,
    playground: bool,
    limits: &super::Limits,
) -> Option<Ask> {
    Some(Ask::Family {
        scope: c::Scope::Deployment,
        record: family::set_up(forge, org, repository, project, playground, &wire_limits(limits)?)?,
    })
}
#[must_use]
pub fn answer(answer: &Answered, limits: &super::Limits) -> Option<SetUpAnswer> {
    match answer {
        Answered::Family(record) => Some(SetUpAnswer::Outcome(family::set_up_answer(record, &wire_limits(limits)?)?)),
        Answered::Jig(c::Answer::Refused { refusal, .. }) => {
            Some(SetUpAnswer::EntranceRefused(c::EntranceRefusal::new(refusal.clone()).ok()?))
        }
        Answered::Jig(c::Answer::Done(_)) => None,
    }
}
/// Invalid documents are refused by the watch's owner, which unwatches its handle.
#[must_use]
pub fn rows(record: &Family, limits: &super::Limits) -> Option<Box<[Row]>> {
    if record.family != wire::FORGE || record.version != wire::CLIENT_VERSION {
        return None;
    }
    family::forges(&record.bytes, &wire_limits(limits)?)
}

/// The session keeps an ended watch until the next sign-in (client/README.md, 4.6).
#[derive(Debug)]
pub enum State {
    Unknown,
    Rows(Box<[Row]>),
    Ended,
}
/// The set up's editable fields (client/forge.md, 5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupField {
    Organization,
    Repository,
    Project,
}
impl SetupField {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Organization => 0,
            Self::Repository => 1,
            Self::Project => 2,
        }
    }
}
#[derive(Debug)]
pub struct Draft {
    pub fields: [Box<[u8]>; 3],
    pub written: [u64; 3],
    /// The person wrote the project's name; until then it follows the repository's.
    pub named: bool,
    pub checked: bool,
}
impl Draft {
    pub(crate) fn new() -> Self {
        Self {
            fields: [Box::from(b"work".as_slice()), Box::from(b"hello".as_slice()), Box::from(b"hello".as_slice())],
            written: [0; 3],
            named: false,
            checked: false,
        }
    }
    #[must_use]
    pub fn required(&self, field: SetupField) -> bool {
        self.checked && self.text(field).is_empty()
    }
    #[must_use]
    pub fn text(&self, field: SetupField) -> &[u8] {
        self.fields.get(field.index()).expect("three setup fields")
    }
    #[must_use]
    pub fn written(&self, field: SetupField) -> u64 {
        *self.written.get(field.index()).expect("three setup fields")
    }
}
pub(crate) fn write(side: &mut super::Side, limits: super::Limits, field: SetupField, text: Box<[u8]>) {
    if text.len() > usize::try_from(limits.field_bytes).expect("field bytes fit") {
        return;
    }
    let draft = &mut side.setup_form;
    let index = field.index();
    if field == SetupField::Project {
        draft.named = true;
    } else if field == SetupField::Repository && !draft.named {
        // A count of its own, so that the platform's field takes the followed text.
        draft.fields[2].clone_from(&text);
        draft.written[2] = draft.written[2].checked_add(1).expect("written count fits");
    }
    *draft.fields.get_mut(index).expect("three setup fields") = text;
    let written = draft.written.get_mut(index).expect("three setup fields");
    *written = written.checked_add(1).expect("written count fits");
    draft.checked = false;
    if let Some(last) = &mut side.set_up {
        last.answer = None;
    }
    side.changed();
}
/// A single automatic adoption, retained after the page is left.
#[derive(Debug)]
pub struct FirstAdoption {
    pub request: skein_lib::Token,
    pub additional: bool,
    pub value: super::Adoption,
    pub outcome: Option<super::Answer>,
}
/// `MakeProject` is kept independently of `SetUp` and Adopt.
#[derive(Debug)]
pub struct FirstProject {
    pub request: skein_lib::Token,
    pub value: super::start::StartIn,
}
/// The last set up sent by this client, retained across page changes.
#[derive(Debug)]
pub struct SetUp {
    pub playground: bool,
    pub forge: u16,
    pub party: u64,
    pub org: Box<[u8]>,
    pub repository: Box<[u8]>,
    pub project: SetUpProject,
    pub adoption: Option<FirstAdoption>,
    pub starting: Option<FirstProject>,
    pub request: skein_lib::Token,
    pub progress: Option<skein_lib::RequestProgress>,
    pub answer: Option<SetUpAnswer>,
}
pub(crate) fn handle() -> jig_client::Handle {
    jig_client::Handle { asker: crate::owner::Owner::Forge.asker(), number: super::me::FORGES }
}
pub(crate) fn configure(side: &mut super::Side, limits: super::Limits, out: &mut skein_lib::Queue<super::Request>) {
    use jig_client::Link;
    if side.link == Link::SignedOut || side.link == Link::OutOfDate {
        if side.forges_open {
            side.forges_open = false;
            out.push(super::Request::Client { asked: jig_client::Event::Unwatch { handle: handle() } });
        }
        side.forges = State::Unknown;
        if side.link == Link::SignedOut {
            side.set_up = None;
            side.pickers.clear();
            side.admin_form = super::admin::Draft::new();
            side.admin = None;
            side.app_form.clear();
            side.app = None;
        }
        side.changed();
        return;
    }
    if side.party.is_some()
        && !side.forges_open
        && let State::Unknown = side.forges
    {
        if let Some(subject) = watch(&limits) {
            side.forges_open = true;
            out.push(super::Request::Client { asked: jig_client::Event::Watch { handle: handle(), subject } });
        } else {
            side.forges = State::Ended;
        }
        side.changed();
    }
}
pub(crate) fn set_up(
    side: &mut super::Side,
    limits: super::Limits,
    forge: u16,
    out: &mut skein_lib::Queue<super::Request>,
) {
    let Some(party) = side.party else { return };
    if side.served == super::Served::OutOfDate {
        return;
    }
    if let Some(last) = &side.set_up
        && last.progress.is_some()
    {
        return;
    }
    let previous = retry_fields(side, forge);
    let draft = &mut side.setup_form;
    draft.checked = true;
    let (org, repository, project, playground) = match previous {
        Some(fields) => (fields.org, fields.repository, fields.project, fields.playground),
        None => {
            if draft.fields[0].is_empty() || draft.fields[1].is_empty() || draft.fields[2].is_empty() {
                side.changed();
                return;
            }
            (draft.fields[0].clone(), draft.fields[1].clone(), SetUpProject::New(draft.fields[2].clone()), false)
        }
    };
    let Some(ask) = ask(forge, org.clone(), repository.clone(), project.clone(), playground, &limits) else {
        return;
    };
    side.serial = side.serial.checked_add(1).expect("forge request serial fits");
    let request = skein_lib::Token::new(side.serial);
    side.set_up = Some(Box::new(SetUp {
        playground,
        forge,
        party,
        org,
        repository,
        project,
        adoption: None,
        starting: None,
        request,
        progress: Some(skein_lib::RequestProgress::Parked),
        answer: None,
    }));
    out.push(super::Request::Send { request, name: None, ask });
    side.changed();
}
struct SetupValues {
    playground: bool,
    org: Box<[u8]>,
    repository: Box<[u8]>,
    project: SetUpProject,
}
fn retry_fields(side: &super::Side, forge: u16) -> Option<SetupValues> {
    let State::Rows(rows) = &side.forges else { return None };
    for row in rows {
        if row.number != forge || !super::connect::failed(row) {
            continue;
        }
        if let Some(last) = &side.set_up
            && last.forge == forge
        {
            return Some(SetupValues {
                playground: last.playground,
                org: last.org.clone(),
                repository: last.repository.clone(),
                project: last.project.clone(),
            });
        }
        if let Some(first) = &row.first {
            return Some(SetupValues {
                playground: first.playground,
                org: first.org.clone(),
                repository: first.repository.clone(),
                project: first.project.clone(),
            });
        }
    }
    None
}
pub(crate) fn told(
    side: &mut super::Side,
    client: &jig_client::Client,
    value: &jig_client::Told,
    limits: super::Limits,
    out: &mut skein_lib::Queue<super::Request>,
) -> bool {
    use jig_client::{Latest, Outcome, Progress, Told};
    if project_told(side, client, value, limits, out) {
        return true;
    }
    match value {
        Told::Changed { handle: changed } if *changed == handle() => {
            side.forges = match client.document(*changed) {
                Some(document) if document.state == jig_client::State::Ended => State::Ended,
                Some(document) => match document.latest {
                    Some(Latest::Family(record)) => match rows(record, &limits) {
                        Some(rows) => State::Rows(rows),
                        None => State::Ended,
                    },
                    Some(Latest::Jig(_)) => State::Ended,
                    None => State::Unknown,
                },
                None => State::Unknown,
            };
            if let State::Ended = side.forges {
                end(side, out);
            }
        }
        Told::Ended { handle: ended, .. } if *ended == handle() => {
            side.forges = State::Ended;
            end(side, out);
        }
        Told::Pending { key } if setup_key(side, *key) => {
            let Some(last) = &mut side.set_up else { return true };
            let Some(pending) = client.pending(*key) else { return true };
            let progress = match pending.progress {
                Progress::InFlight => skein_lib::RequestProgress::InFlight,
                Progress::Retrying { .. } => skein_lib::RequestProgress::Retrying,
                Progress::NotServed | Progress::Parked => skein_lib::RequestProgress::Parked,
                Progress::Unknown => skein_lib::RequestProgress::Unknown,
            };
            if progress == skein_lib::RequestProgress::Unknown {
                last.progress = None;
                last.answer = Some(SetUpAnswer::Outcome(SetUpOutcome::Unknown));
            } else {
                last.progress = Some(if side.served == super::Served::OutOfDate {
                    skein_lib::RequestProgress::Parked
                } else {
                    progress
                });
            }
        }
        Told::Answered { key, outcome } if setup_key(side, *key) => {
            let Some(last) = &mut side.set_up else { return true };
            last.progress = None;
            last.answer = Some(match outcome {
                Outcome::Answered(answered) => {
                    answer(answered, &limits).unwrap_or(SetUpAnswer::Outcome(SetUpOutcome::Unknown))
                }
                Outcome::Unknown
                | Outcome::Limit
                | Outcome::Held
                | Outcome::NotKept
                | Outcome::Unsent
                | Outcome::Lost
                | Outcome::Full { .. }
                | Outcome::Unconfirmed => SetUpAnswer::Outcome(SetUpOutcome::Unknown),
            });
        }
        Told::Link { .. }
        | Told::Changed { .. }
        | Told::Ended { .. }
        | Told::Page { .. }
        | Told::Text { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. } => return false,
    }
    super::admin::stopped(side);
    maybe_adopt(side, limits, out);
    side.changed();
    true
}
fn project_told(
    side: &mut super::Side,
    client: &jig_client::Client,
    value: &jig_client::Told,
    limits: super::Limits,
    out: &mut skein_lib::Queue<super::Request>,
) -> bool {
    use jig_client::{Outcome, Progress, Told};
    if let Some(last) = &mut side.set_up
        && let Some(starting) = &mut last.starting
        && starting.value.outcome.is_none()
    {
        let key = jig_client::Key { asker: crate::owner::Owner::Forge.asker(), number: starting.request.raw() };
        let outcome = match value {
            Told::Answered { key: answered, outcome } if *answered == key => Some(outcome.clone()),
            Told::Pending { key: pending } if *pending == key => match client.pending(key) {
                Some(pending) if pending.progress == Progress::Unknown => Some(Outcome::Unknown),
                Some(_) | None => return true,
            },
            Told::Link { .. }
            | Told::Pending { .. }
            | Told::Answered { .. }
            | Told::Kept { .. }
            | Told::Dropped { .. }
            | Told::Changed { .. }
            | Told::Text { .. }
            | Told::Ended { .. }
            | Told::Page { .. } => None,
        };
        if let Some(outcome) = outcome {
            let adoption = starting.value.answered(&outcome);
            if side.party == Some(last.party)
                && let Some(adoption) = adoption
            {
                send_adoption(side, limits, &adoption, false, out);
            }
            side.changed();
            return true;
        }
    }
    false
}

fn setup_key(side: &super::Side, key: jig_client::Key) -> bool {
    let Some(last) = &side.set_up else { return false };
    key.asker == crate::owner::Owner::Forge.asker() && key.number == last.request.raw()
}
fn end(side: &mut super::Side, out: &mut skein_lib::Queue<super::Request>) {
    if side.forges_open {
        side.forges_open = false;
        out.push(super::Request::Client { asked: jig_client::Event::Unwatch { handle: handle() } });
    }
}
#[must_use]
pub fn worst_case(limits: &super::Limits) -> Option<u64> {
    let wire = wire_limits(limits)?;
    let rows = u64::from(temper_client_protocol_wire::FORGES);
    w::Forges::worst_case_heap(&wire)?
        .checked_mul(2)?
        .checked_add(rows.checked_mul(u64::try_from(size_of::<Row>()).ok()?)?)?
        // Three draft fields, three sent fields, two adoption names, main and temper/.
        .checked_add(u64::from(limits.field_bytes).checked_mul(8)?)?
        .checked_add(11)?
        // MakeProject retains a name, an adoption, and jig's bounded refusal.
        .checked_add(u64::from(limits.field_bytes).checked_mul(5)?)?
        .checked_add(65_536)?
        .checked_add(skein_lib::List::<(u16, u64)>::worst_case(ROWS)?)?
        .checked_add(u64::try_from(size_of::<SetUp>()).ok()?)?
        .checked_add(w::Answer::worst_case_heap(&wire)?)
}

// The first repository is adopted on the forge side, even away from the flow
// (protocol/setup.md, 6.7), after jig has made its project.
pub(crate) fn maybe_adopt(side: &mut super::Side, limits: super::Limits, out: &mut skein_lib::Queue<super::Request>) {
    if side.party.is_none() || side.served == super::Served::OutOfDate {
        return;
    }
    let Some(last) = &side.set_up else {
        return;
    };
    if last.starting.is_some() || last.adoption.is_some() || side.party != Some(last.party) {
        return;
    }
    let State::Rows(rows) = &side.forges else {
        return;
    };
    for row in rows {
        if row.number != last.forge {
            continue;
        }
        // As ctl does, wait for the forge to accept the made repository's Adopt.
        match row.connection {
            Connection::Connected { .. } => {}
            Connection::NotConnected
            | Connection::NotSetUp
            | Connection::SettingUp { .. }
            | Connection::SetUpFailed { .. }
            | Connection::Connecting
            | Connection::Stopped { .. } => return,
        }
        if let Some(first) = &row.first {
            match first.state {
                FirstState::Made { .. } => {}
                FirstState::Making | FirstState::Failed { .. } => return,
            }
            let SetUpProject::New(name) = &last.project else { return };
            let name = name.clone();
            let adoption = super::Adoption {
                project: 0,
                forge: last.forge,
                owner: first.org.clone(),
                name: first.repository.clone(),
                prefix: Box::from(b"temper/".as_slice()),
                role: super::Role::Owned,
                landing: Box::from(b"main".as_slice()),
                ci: true,
                checks: Box::default(),
                home: true,
            };
            let forge = last.forge;
            setup_start(side, limits, forge, adoption, name, out);
            return;
        }
    }
}
/// The project's number is deliberately absent until jig answers `MakeProject`.
fn setup_start(
    side: &mut super::Side,
    limits: super::Limits,
    forge: u16,
    adoption: super::Adoption,
    name: Box<[u8]>,
    out: &mut skein_lib::Queue<super::Request>,
) {
    let Some(starting) = start_in(side, limits, forge, adoption, name, out) else { return };
    side.set_up.as_mut().expect("setup retained").starting = Some(starting);
    side.changed();
}
/// Shared keyed `MakeProject` send; each caller retains its own pending start.
pub(crate) fn start_in(
    side: &mut super::Side,
    limits: super::Limits,
    forge: u16,
    adoption: super::Adoption,
    name: Box<[u8]>,
    out: &mut skein_lib::Queue<super::Request>,
) -> Option<FirstProject> {
    let value = super::start::StartIn::new(adoption, name);
    let ask = value.ask(&limits)?;
    let mut attempt = None;
    for index in 0..side.start_attempts.len() {
        let (number, next) = side.start_attempts.get_mut(index).expect("counted attempts");
        if *number == forge {
            attempt = Some(*next);
            *next = next.checked_add(1).expect("start attempts fit");
            break;
        }
    }
    let attempt = match attempt {
        Some(attempt) => attempt,
        None => {
            side.start_attempts.push((forge, 1)).ok()?;
            0
        }
    };
    // The name is the engine's dedup key: after refusal, changing the text
    // needs start-project-F-N, counting attempts within this client.
    side.serial = side.serial.checked_add(1).expect("forge request serial fits");
    let request = skein_lib::Token::new(side.serial);
    out.push(super::Request::Send { request, name: Some(super::start_project_key(forge, attempt)), ask });
    Some(FirstProject { request, value })
}
pub(crate) fn retry_start(
    side: &mut super::Side,
    limits: super::Limits,
    forge: u16,
    out: &mut skein_lib::Queue<super::Request>,
) {
    let Some(last) = &side.set_up else { return };
    if last.forge != forge || side.party != Some(last.party) || side.served == super::Served::OutOfDate {
        return;
    }
    let Some(starting) = &last.starting else { return };
    // Retry only a known refusal: an unknown answer may already have made a project.
    let Some(jig_client::Outcome::Answered(Answered::Jig(c::Answer::Refused { .. }))) = &starting.value.outcome else {
        return;
    };
    side.setup_form.checked = true;
    let name = side.setup_form.text(SetupField::Project);
    if name.is_empty() {
        side.changed();
        return;
    }
    let adoption = starting.value.adoption.clone();
    let name = Box::from(name);
    setup_start(side, limits, forge, adoption, name, out);
}

fn send_adoption(
    side: &mut super::Side,
    limits: super::Limits,
    adoption: &super::Adoption,
    additional: bool,
    out: &mut skein_lib::Queue<super::Request>,
) {
    let pending = adoption_request(side, limits, adoption, additional, out);
    side.set_up.as_mut().expect("setup retained").adoption = pending;
    side.changed();
}
/// Shared keyed `Adopt` send; the setup and picker retain their own answer.
pub(crate) fn adoption_request(
    side: &mut super::Side,
    limits: super::Limits,
    adoption: &super::Adoption,
    additional: bool,
    out: &mut skein_lib::Queue<super::Request>,
) -> Option<FirstAdoption> {
    let ask = super::ask(adoption, &limits)?;
    side.serial = side.serial.checked_add(1).expect("forge request serial fits");
    let request = skein_lib::Token::new(side.serial);
    out.push(super::Request::Send {
        request,
        name: Some(super::first_repository_key(adoption.forge, additional)),
        ask,
    });
    Some(FirstAdoption { request, additional, value: adoption.clone(), outcome: None })
}
pub(crate) fn adopted(
    side: &mut super::Side,
    limits: super::Limits,
    request: skein_lib::Token,
    answer: &super::Answer,
    out: &mut skein_lib::Queue<super::Request>,
) -> bool {
    let Some(last) = &mut side.set_up else {
        return false;
    };
    let Some(adopting) = &mut last.adoption else {
        return false;
    };
    if adopting.request != request || adopting.outcome.is_some() {
        return false;
    }
    let refused = *answer == super::Answer::ForgeRefused(super::Reason::HomeHeld);
    if refused && !adopting.additional && side.party == Some(last.party) {
        let mut adoption = adopting.value.clone();
        adoption.home = false;
        send_adoption(side, limits, &adoption, true, out);
    } else {
        adopting.outcome = Some(answer.clone());
        side.changed();
    }
    true
}

/// Bundle a pasted App's PEM for jig's carried value (protocol/credentials.md, 2.2).
#[must_use]
pub fn pasted_app_key(pem: &[u8]) -> Option<skein_lib::Secret> {
    temper_protocol_records::encode_pasted(11, pem).ok()
}

/// Restore only RSA armour separators stripped by a password input, then bundle it.
/// Body bytes stay untouched; Paste decides whether they form a key.
#[must_use]
pub fn restored_app_key(pem: &[u8]) -> Option<skein_lib::Secret> {
    const BEGIN: &[u8] = b"-----BEGIN RSA PRIVATE KEY-----";
    const END: &[u8] = b"-----END RSA PRIVATE KEY-----";
    let pem = trimmed_app_key(pem);
    let after_begin = pem.strip_prefix(BEGIN);
    let before_end = pem.strip_suffix(END);
    let begin_lf = match after_begin {
        Some(rest) => !rest.starts_with(b"\r") && !rest.starts_with(b"\n"),
        None => false,
    };
    let end_lf = match before_end {
        Some(rest) => !rest.ends_with(b"\n"),
        None => false,
    };
    let length = pem.len().checked_add(usize::from(begin_lf))?.checked_add(usize::from(end_lf))?;
    if length == 0 || length > PASTED_PAYLOAD {
        return None;
    }
    let mut out = skein_lib::Writer::new(length);
    let mut body = pem;
    if begin_lf {
        out.put(BEGIN).ok()?;
        out.put(b"\n").ok()?;
        body = body.strip_prefix(BEGIN)?;
    }
    if end_lf {
        out.put(body.strip_suffix(END)?).ok()?;
        out.put(b"\n").ok()?;
        out.put(END).ok()?;
    } else {
        out.put(body).ok()?;
    }
    pasted_app_key(&out.finish())
}
pub(crate) fn trimmed_app_key(mut pem: &[u8]) -> &[u8] {
    while let Some(b' ' | b'\t' | b'\r' | b'\n') = pem.first() {
        pem = pem.get(1..).expect("first byte exists");
    }
    while let Some(b' ' | b'\t' | b'\r' | b'\n') = pem.last() {
        pem = pem.get(..pem.len().checked_sub(1).expect("last byte exists")).expect("shorter slice");
    }
    pem
}

/// Stable names and words for forge connection refusals (client/README.md, 4.6).
#[must_use]
pub fn connect_refusal(why: &temper_client_protocol_wire::v1::ForgeRefusal) -> (&'static [u8], &'static [u8]) {
    match why {
        temper_client_protocol_wire::v1::ForgeRefusal::Role => (b"role", b"not the deployment's owner"),
        temper_client_protocol_wire::v1::ForgeRefusal::NoSuchForge => (b"no_such_forge", b"no such forge"),
        temper_client_protocol_wire::v1::ForgeRefusal::NotTaken => (b"not_taken", b"not taken by this forge"),
        temper_client_protocol_wire::v1::ForgeRefusal::Connected => {
            (b"connected", b"connected already: forge-replace replaces its key")
        }
        temper_client_protocol_wire::v1::ForgeRefusal::NotConnected => {
            (b"not_connected", b"not connected: connect the App first")
        }
        temper_client_protocol_wire::v1::ForgeRefusal::Busy => (b"busy", b"busy"),
        temper_client_protocol_wire::v1::ForgeRefusal::NoKey => (b"no_key", b"no key"),
        temper_client_protocol_wire::v1::ForgeRefusal::FirstIsBot => {
            (b"first_is_bot", b"the organization is the bot's login: name another organization")
        }
    }
}

/// One accepted, usable installation makes repositories reachable.
#[must_use]
pub fn accepted(row: &Row) -> bool {
    for installation in &row.installations {
        if !installation.stray && !installation.suspended {
            return true;
        }
    }
    false
}
#[must_use]
pub fn slug(row: &Row) -> Option<&[u8]> {
    if row.kind != Kind::GitHub {
        return None;
    }
    let Connection::Connected { login, .. } = &row.connection else {
        return None;
    };
    Some(login.strip_suffix(b"[bot]").unwrap_or(login))
}

/// Payload space within the existing carried-value budget after schema framing.
pub use temper_protocol_records::PASTED_PAYLOAD;

/// Bundle a site admin's token with the same family as a pasted App key.
#[must_use]
pub fn pasted_admin_token(token: &[u8]) -> Option<skein_lib::Secret> {
    temper_protocol_records::encode_pasted(1, token).ok()
}
