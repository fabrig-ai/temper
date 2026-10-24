//! The forge's adoption form, demo read, panel watch and receiving state.
use alloc::boxed::Box;
use skein_lib::{Env, Queue, Time};
pub mod admin;
pub mod change;
pub mod connect;
pub mod demo;
pub mod family;
pub mod forges;
mod form;
pub mod github;
pub mod listed;
pub(crate) mod me;
pub mod panel;
pub mod picker;
pub mod start;
mod vocabulary;
pub mod words;
pub use form::{Act, Broken, Check, Field, Form, Marks, Text, Typed, adoption, broken, form, write};
pub use vocabulary::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub field_bytes: u32,
    pub checks: u32,
}
#[derive(Debug)]
pub enum Event {
    Act { act: Act },
    AdminToken { value: skein_lib::Secret },
    AppKey { value: skein_lib::Secret },
    Panel { subject: Option<panel::Subject> },
    RefreshPanel { subject: panel::Subject },
    Watched { handle: jig_client::Handle, watched: panel::Watched },
    Progress { request: skein_lib::Token, progress: skein_lib::RequestProgress },
    Answered { request: skein_lib::Token, answer: Answer },
    Went { address: Address },
    Left,
    Served { served: Served },
}
#[derive(Debug)]
pub enum Request {
    Show { address: Address },
    Leave { to: Box<[u8]> },
    Client { asked: jig_client::Event },
    Send { request: skein_lib::Token, name: Option<skein_lib::RequestKey>, ask: jig_client::Ask },
}
#[derive(Debug)]
pub struct Side {
    demo: demo::State,
    forges: forges::State,
    forges_open: bool,
    set_up: Option<Box<forges::SetUp>>,
    setup_form: forges::Draft,
    pickers: skein_lib::List<picker::Picker>,
    admin_form: admin::Draft,
    admin: Option<admin::Pending>,
    app_form: github::Draft,
    app: Option<github::Pending>,
    panel: Option<panel::Panel>,
    panel_retry: bool,
    address: Option<Address>,
    served: Served,
    form: Option<Form>,
    serial: u64,
    start_attempts: skein_lib::List<(u16, u64)>,
    party: Option<u64>,
    me: bool,
    link: jig_client::Link,
    shown: u64,
}
impl Side {
    #[must_use]
    pub fn new(_limits: &Limits, seed: u64) -> Side {
        Side {
            demo: demo::State::new(),
            forges: forges::State::Unknown,
            forges_open: false,
            set_up: None,
            setup_form: forges::Draft::new(),
            pickers: skein_lib::List::with_capacity(forges::ROWS),
            admin_form: admin::Draft::new(),
            admin: None,
            app_form: github::Draft::new(),
            app: None,
            panel: None,
            panel_retry: false,
            address: None,
            served: Served::Unknown,
            form: None,
            serial: skein_lib::Rng::new(seed).next_u64(),
            start_attempts: skein_lib::List::with_capacity(forges::ROWS),
            party: None,
            me: false,
            link: jig_client::Link::Starting,
            shown: 0,
        }
    }
    #[must_use]
    pub const fn address(&self) -> Option<Address> {
        self.address
    }
    #[must_use]
    pub const fn served(&self) -> Served {
        self.served
    }
    #[must_use]
    pub const fn form(&self) -> Option<&Form> {
        self.form.as_ref()
    }
    #[must_use]
    pub const fn demo_answered(&self) -> bool {
        self.demo.answered
    }
    #[must_use]
    pub const fn demo(&self) -> Option<&Demo> {
        self.demo.answer.as_ref()
    }
    #[must_use]
    pub fn demo_running(&self) -> bool {
        match &self.demo.answer {
            Some(demo) => {
                demo.state == demo::DemoState::Running || self.demo.start.is_some() || self.demo.failure.is_some()
            }
            None => false,
        }
    }
    #[must_use]
    pub const fn demo_starting(&self) -> bool {
        self.demo.start.is_some()
    }
    #[must_use]
    pub const fn demo_failure(&self) -> Option<&demo::StartFailure> {
        self.demo.failure.as_ref()
    }
    #[must_use]
    pub const fn panel(&self) -> Option<&panel::Panel> {
        self.panel.as_ref()
    }
    #[must_use]
    pub const fn forges(&self) -> &forges::State {
        &self.forges
    }
    #[must_use]
    pub fn set_up(&self) -> Option<&forges::SetUp> {
        self.set_up.as_deref()
    }
    #[must_use]
    pub const fn setup_form(&self) -> &forges::Draft {
        &self.setup_form
    }
    #[must_use]
    pub const fn admin_form(&self) -> &admin::Draft {
        &self.admin_form
    }
    #[must_use]
    pub const fn admin(&self) -> Option<&admin::Pending> {
        self.admin.as_ref()
    }
    #[must_use]
    pub const fn app_form(&self) -> &github::Draft {
        &self.app_form
    }
    #[must_use]
    pub const fn app(&self) -> Option<&github::Pending> {
        self.app.as_ref()
    }
    #[must_use]
    pub fn picker(&self, forge: u16) -> Option<&picker::Picker> {
        picker::get(self, forge)
    }
    fn changed(&mut self) {
        self.shown = self.shown.checked_add(1).expect("forge shown count fits");
    }
}
pub fn step(side: &mut Side, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::AdminToken { value } => admin::token(side, value),
        Event::AppKey { value } => github::key(side, value),
        Event::Panel { subject } => panel::subject(side, env.limits, subject, out),
        Event::RefreshPanel { subject } => panel::refresh(side, env.limits, subject, out),
        Event::Watched { handle, watched } => panel::watched(side, handle, watched, out),
        Event::Went { address } => {
            side.address = Some(address);
            let Address::Adopt { project } = address;
            let mut draft = form(project);
            draft.served = side.served;
            draft.confirmed = side.party.is_some();
            side.form = Some(draft);
            side.changed();
        }
        Event::Left => {
            side.form = None;
            if side.address.take().is_some() {
                side.changed();
            }
        }
        Event::Served { served: value } => served(side, value),
        Event::Act { act } => act_on(side, env.limits, act, out),
        Event::Progress { request, progress } => {
            if progress == skein_lib::RequestProgress::Unknown
                && (picker::adopted(side, request, &Answer::Unknown)
                    || forges::adopted(side, env.limits, request, &Answer::Unknown, out))
            {
                return;
            }
            if let Some(draft) = &mut side.form
                && let Some((pending, _)) = draft.pending
                && pending == request
            {
                match progress {
                    skein_lib::RequestProgress::Unknown => {
                        draft.pending = None;
                        draft.outcome = Some(Answer::Unknown);
                    }
                    skein_lib::RequestProgress::InFlight
                    | skein_lib::RequestProgress::Retrying
                    | skein_lib::RequestProgress::Parked => {
                        let progress = match draft.served {
                            Served::OutOfDate => skein_lib::RequestProgress::Parked,
                            Served::Unknown | Served::Yes => progress,
                        };
                        draft.pending = Some((request, progress));
                    }
                }
                side.changed();
            }
        }
        Event::Answered { request, answer } => {
            if picker::adopted(side, request, &answer) || forges::adopted(side, env.limits, request, &answer, out) {
                return;
            }
            if let Some(draft) = &mut side.form
                && let Some((pending, _)) = draft.pending
                && pending == request
            {
                draft.pending = None;
                match answer {
                    Answer::Adopted(_) => form::reset(draft, answer),
                    Answer::CoreRefused(_) | Answer::ForgeRefused(_) | Answer::EntranceRefused(_) | Answer::Unknown => {
                        draft.outcome = Some(answer);
                    }
                }
                side.changed();
            }
        }
    }
    forges::maybe_adopt(side, env.limits, out);
}
fn act_on(side: &mut Side, limits: Limits, act: Act, out: &mut Queue<Request>) {
    match act {
        Act::Picker { forge, act } => picker::act(side, limits, forge, act, out),
        Act::Go { address } => out.push(Request::Show { address }),
        Act::AdminWrite { field, text } => admin::write(side, limits, field, text),
        Act::ConnectAdmin { forge } => admin::send(side, limits, forge, out),
        Act::AppWrite { text } => github::write(side, text, limits),
        Act::ConnectManifest { forge, back } => github::manifest(side, limits, forge, back, out),
        Act::ConnectApp { forge } => github::send(side, limits, forge, None, out),
        Act::Install { forge, back } => github::send(side, limits, forge, Some(back), out),
        Act::StartDemo => demo::start(side, limits, out),
        Act::SetUp { forge } => forges::set_up(side, limits, forge, out),
        Act::SetupWrite { field, text } => forges::write(side, limits, field, text),
        Act::StartIn { forge } => forges::retry_start(side, limits, forge, out),
        Act::Adopt => {
            let Some(_party) = side.party else {
                return;
            };
            let Some(draft) = &mut side.form else {
                return;
            };
            if draft.pending.is_some() {
                return;
            }
            match draft.served {
                Served::OutOfDate => return,
                Served::Yes | Served::Unknown => {}
            }
            form::mark(draft, limits.checks);
            if let Some(adoption) = adoption(draft, limits.checks) {
                side.serial = side.serial.checked_add(1).expect("forge request serial fits u64");
                let request = skein_lib::Token::new(side.serial);
                draft.pending = Some((request, skein_lib::RequestProgress::Parked));
                out.push(Request::Send {
                    request,
                    name: None,
                    ask: ask(&adoption, &limits).expect("validated adoption fits wire"),
                });
            }
            side.changed();
        }
        Act::Write { field, text, typed } => {
            if let Some(draft) = &mut side.form {
                write(draft, field, text, typed, limits.field_bytes, limits.checks);
                side.changed();
            }
        }
        Act::Choose { role } => {
            if let Some(draft) = &mut side.form {
                draft.role = role;
                draft.outcome = None;
                draft.home.broken = None;
                side.changed();
            }
        }
        Act::Home => {
            if let Some(draft) = &mut side.form {
                draft.home.on = !draft.home.on;
                draft.home.broken = None;
                draft.outcome = None;
                side.changed();
            }
        }
        Act::Ci => {
            if let Some(draft) = &mut side.form {
                draft.ci.on = !draft.ci.on;
                draft.outcome = None;
                side.changed();
            }
        }
    }
}
#[must_use]
pub const fn next_deadline(_side: &Side) -> Option<Time> {
    None
}
pub const fn reclaim(_side: &mut Side) {}
#[must_use]
pub const fn shown(side: &Side) -> u64 {
    side.shown
}
#[must_use]
pub const fn max_out(_limits: &Limits) -> u32 {
    // One completed project per retained picker, plus the shown page's read.
    forges::ROWS.checked_add(4).expect("forge output fits")
}
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.field_bytes < 7 {
        return None;
    }
    let wire = wire_limits(limits)?;
    u64::from(limits.field_bytes)
        .checked_mul(6)?
        .checked_add(u64::from(limits.checks).checked_mul(4)?)?
        .checked_add(forges::worst_case(limits)?)?
        .checked_add(picker::worst_case(*limits)?)?
        .checked_add(4097 + 2048)?
        .checked_add(4090)?
        .checked_add(u64::from(limits.field_bytes).checked_mul(3)?)?
        .checked_add(u64::from(limits.field_bytes.max(wire.first_repository_label)))?
        .checked_add(temper_client_protocol_wire::v1::Answer::worst_case_heap(&wire)?)?
        .checked_add(u64::from(limits.field_bytes))?
        .checked_add(temper_client_protocol_wire::v1::Answer::worst_case_heap(&wire)?)?
        .checked_add(temper_client_protocol_wire::v1::DemoAnswer::worst_case_heap(&wire)?)?
        .checked_add(u64::from(wire.group_failed_why))?
        .checked_add(u64::try_from(size_of::<jig_client::Handle>()).ok()?)?
        .checked_add(u64::try_from(size_of::<temper_client_protocol_wire::v1::ChangePanel>()).ok()?)?
        .checked_add(temper_client_protocol_wire::v1::ChangePanel::worst_case_heap(&wire)?)
}

fn served(side: &mut Side, served: Served) {
    side.served = served;
    if served == Served::OutOfDate
        && let Some(set_up) = &mut side.set_up
        && set_up.progress.is_some()
    {
        set_up.progress = Some(skein_lib::RequestProgress::Parked);
    }
    if let Some(draft) = &mut side.form {
        draft.served = side.served;
        if side.served == Served::OutOfDate
            && let Some((request, _)) = draft.pending
        {
            draft.pending = Some((request, skein_lib::RequestProgress::Parked));
        }
    }
    side.changed();
}

/// The forge's receiving limits, shared by its bytes and bounds accounting.
#[must_use]
pub fn wire_limits(limits: &Limits) -> Option<temper_client_protocol_wire::v1::Limits> {
    temper_client_protocol_wire::limits(&temper_client_protocol_wire::Bounds {
        field_bytes: limits.field_bytes,
        checks: limits.checks,
        contexts: limits.checks,
        refusal_bytes: 65_536,
    })
}
pub(crate) fn told(client: &jig_client::Client, limits: &crate::Limits, told: jig_client::Told) -> Option<Event> {
    use jig_client::{Outcome, Told};
    match told {
        Told::Changed { handle } => {
            Some(Event::Watched { handle, watched: panel::decode(client, handle, limits.forge) })
        }
        Told::Ended { handle, why } => Some(Event::Watched { handle, watched: panel::Watched::Ended(why) }),
        Told::Pending { key } => {
            let pending = client.pending(key)?;
            let progress = match pending.progress {
                jig_client::Progress::InFlight => skein_lib::RequestProgress::InFlight,
                jig_client::Progress::Retrying { .. } => skein_lib::RequestProgress::Retrying,
                jig_client::Progress::NotServed | jig_client::Progress::Parked => skein_lib::RequestProgress::Parked,
                jig_client::Progress::Unknown => skein_lib::RequestProgress::Unknown,
            };
            Some(Event::Progress { request: skein_lib::Token::new(key.number), progress })
        }
        Told::Answered { key, outcome } => {
            let request = skein_lib::Token::new(key.number);
            let answer = match outcome {
                Outcome::Answered(answered) => answer(limits, &answered)?,
                Outcome::Unknown
                | Outcome::Limit
                | Outcome::Held
                | Outcome::NotKept
                | Outcome::Unsent
                | Outcome::Lost
                | Outcome::Unconfirmed => Answer::Unknown,
            };
            Some(Event::Answered { request, answer })
        }
        Told::Kept { .. } | Told::Dropped { .. } | Told::Link { .. } | Told::Text { .. } | Told::Page { .. } => None,
    }
}

/// Encode the validated adoption in its project scope.
#[must_use]
pub fn ask(adoption: &Adoption, limits: &Limits) -> Option<jig_client::Ask> {
    Some(jig_client::Ask::Family {
        scope: jig_client::contract::Scope::Project(adoption.project),
        record: family::request(adoption, &wire_limits(limits)?)?,
    })
}
/// Decode the same terminal answers the adoption side receives.
#[must_use]
pub fn answer(limits: &crate::Limits, answered: &jig_client::Answered) -> Option<Answer> {
    match answered {
        jig_client::Answered::Family(record) => {
            let link = jig_client_protocol_link::Limits::new(limits.client.domain, limits.client.exchanges).ok()?;
            family::answer(record, &wire_limits(&limits.forge)?, &link)
        }
        jig_client::Answered::Jig(jig_client::contract::Answer::Refused { refusal, .. }) => {
            Some(Answer::EntranceRefused(jig_client::contract::EntranceRefusal::new(refusal.clone()).ok()?))
        }
        jig_client::Answered::Jig(jig_client::contract::Answer::Done(_)) => Some(Answer::Unknown),
    }
}

/// The page and ctl name the same adoption (client/forge.md, 5.4).
#[must_use]
pub fn first_repository_key(forge: u16, additional: bool) -> skein_lib::RequestKey {
    let prefix: &[u8] = if additional { b"first-repository-additional-" } else { b"first-repository-" };
    let number = skein_lib::Decimal::of(u64::from(forge));
    let mut text = skein_lib::Writer::new(prefix.len().checked_add(number.as_bytes().len()).expect("key fits"));
    text.put(prefix).expect("key prefix fits");
    text.put(number.as_bytes()).expect("key number fits");
    crate::key::named(crate::key::TOOLS, &crate::key::Name::new(&text.finish()).expect("first repository key fits"))
}

/// Project creation has its own dedup name; a refused attempt needs a new one.
#[must_use]
pub fn start_project_key(forge: u16, attempt: u64) -> skein_lib::RequestKey {
    let prefix = b"start-project-";
    let forge = skein_lib::Decimal::of(u64::from(forge));
    let number = skein_lib::Decimal::of(attempt);
    let suffix = if attempt == 0 { 0 } else { number.as_bytes().len().checked_add(1).expect("suffix fits") };
    let length =
        prefix.len().checked_add(forge.as_bytes().len()).expect("forge fits").checked_add(suffix).expect("key fits");
    let mut text = skein_lib::Writer::new(length);
    text.put(prefix).expect("key prefix fits");
    text.put(forge.as_bytes()).expect("forge fits");
    if attempt != 0 {
        text.put(b"-").expect("separator fits");
        text.put(number.as_bytes()).expect("attempt fits");
    }
    crate::key::named(crate::key::TOOLS, &crate::key::Name::new(&text.finish()).expect("start project key fits"))
}
