//! Ready's per-forge repository choice and new project (client/forge.md, 5.4).
use super::{Limits, Request, Served, Side, family, forges, github};
use alloc::boxed::Box;
use jig_client::{Ask, Client, Family, Key, Outcome, Told, contract as c};
use skein_lib::{Duration, Queue, Time, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};

const BUSY_WAIT: Duration = Duration::from_secs(2);
const BUSY_REREADS: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Account(u64),
    Page(u32),
    Choose(u32),
    Write(Box<[u8]>),
    Adopt,
}
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads presentation while retry timing stays private")]
pub struct Picker {
    pub forge: u16,
    pub installation: u64,
    pub installations: Box<[forges::Installation]>,
    pub page: u32,
    pub chosen: u32,
    pub key: Option<Key>,
    pub answer: Option<github::Answer>,
    pub name: Box<[u8]>,
    pub named: bool,
    pub written: u64,
    pub checked: bool,
    pub party: u64,
    pub starting: Option<forges::FirstProject>,
    pub adoption: Option<forges::FirstAdoption>,
    busy_rereads: u8,
    retry_at: Option<Time>,
}
impl Picker {
    #[must_use]
    pub fn retry_exhausted(&self) -> bool {
        self.busy_rereads == BUSY_REREADS && self.answer == Some(github::Answer::Refused(w::ForgeRefusal::Busy))
    }
    fn retry_due(&mut self, now: Time) -> bool {
        if self.answer != Some(github::Answer::Refused(w::ForgeRefusal::Busy)) || self.busy_rereads == BUSY_REREADS {
            return false;
        }
        match self.retry_at {
            Some(at) if at <= now => {
                self.busy_rereads = self.busy_rereads.checked_add(1).expect("bounded busy rereads");
                true
            }
            Some(_) => false,
            None => {
                self.retry_at = now.checked_add(BUSY_WAIT);
                false
            }
        }
    }
    #[must_use]
    pub fn selected(&self) -> Option<&family::Reached> {
        let Some(github::Answer::Page(page)) = &self.answer else { return None };
        page.rows.get(usize::try_from(self.chosen).ok()?)
    }
    #[must_use]
    pub fn pending(&self) -> bool {
        if let Some(adoption) = &self.adoption {
            return match &adoption.outcome {
                None | Some(super::Answer::Adopted(_) | super::Answer::Unknown) => true,
                Some(
                    super::Answer::CoreRefused(_) | super::Answer::ForgeRefused(_) | super::Answer::EntranceRefused(_),
                ) => false,
            };
        }
        match &self.starting {
            Some(starting) => match &starting.value.outcome {
                Some(Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused { .. }))) => false,
                None | Some(_) => true,
            },
            None => false,
        }
    }
    fn follow(&mut self) {
        if !self.named
            && let Some(row) = self.selected()
            && self.name != row.name
        {
            self.name = row.name.clone();
            self.written = self.written.checked_add(1).expect("picker writes fit");
        }
    }
}
#[expect(clippy::manual_find, reason = "bounded step loops")]
pub(crate) fn get(side: &Side, forge: u16) -> Option<&Picker> {
    for picker in &side.pickers {
        if picker.forge == forge {
            return Some(picker);
        }
    }
    None
}
#[must_use]
pub fn installation(rows: &[forges::Installation], selected: Option<u64>) -> Option<&forges::Installation> {
    for row in rows {
        if row.suspended || row.stray {
            continue;
        }
        match selected {
            Some(id) if row.id != id => {}
            Some(_) | None => return Some(row),
        }
    }
    None
}
fn index(side: &Side, forge: u16) -> Option<u32> {
    for index in 0..side.pickers.len() {
        if side.pickers.get(index)?.forge == forge {
            return Some(index);
        }
    }
    None
}
pub(crate) fn progress(
    side: &mut Side,
    client: &Client,
    address: &crate::Address,
    limits: Limits,
    now: Time,
    out: &mut Queue<Request>,
) {
    if side.party.is_none() || side.served == Served::OutOfDate {
        return;
    }
    let row = match address {
        crate::Address::ConnectForge { forge: Some(crate::Choice::Forge(forge)) } => {
            super::connect::row(*forge, &side.forges, side.demo.answer.as_ref())
        }
        crate::Address::Welcome { step: crate::Step::Forge { .. } } => {
            super::connect::personal(&side.forges, side.demo.answer.as_ref())
        }
        crate::Address::Inbox { .. }
        | crate::Address::Chats { .. }
        | crate::Address::Accounts
        | crate::Address::Forges
        | crate::Address::ConnectForge { .. }
        | crate::Address::ConnectModel { .. }
        | crate::Address::Welcome { .. }
        | crate::Address::Chat { .. }
        | crate::Address::Task { .. }
        | crate::Address::Adopt { .. } => None,
    };
    let mut shown = None;
    if let Some(row) = row
        && row.kind == forges::Kind::GitHub
        && row.repositories.is_empty()
        && let super::connect::Shown::Ready(_) =
            super::connect::step(Some(crate::Choice::Forge(row.number)), &side.forges, side.demo.answer.as_ref())
    {
        let forge = row.number;
        shown = Some(forge);
        let installations = row.installations.clone();
        if index(side, forge).is_none() {
            let Some(first) = installation(&installations, None) else { return };
            let selected = first.id;
            side.pickers
                .push(Picker {
                    forge,
                    installation: selected,
                    installations: installations.clone(),
                    page: 1,
                    chosen: 0,
                    key: None,
                    answer: None,
                    name: Box::default(),
                    named: false,
                    written: 0,
                    checked: false,
                    party: side.party.expect("confirmed"),
                    starting: None,
                    adoption: None,
                    busy_rereads: 0,
                    retry_at: None,
                })
                .expect("one picker per configured forge");
        }
        let picker = side.pickers.get_mut(index(side, forge).expect("picker exists")).expect("picker index");
        if picker.installations != installations {
            picker.installations = installations;
            if installation(&picker.installations, Some(picker.installation)).is_none() {
                let Some(first) = installation(&picker.installations, None) else { return };
                picker.installation = first.id;
            }
            picker.page = 1;
            picker.key = None;
            picker.answer = None;
            picker.busy_rereads = 0;
            picker.retry_at = None;
        }
        if picker.key.is_none() || picker.retry_due(now) {
            read(side, limits, forge, out);
        }
    }
    // An answered project may precede the Me frame that names it. Wait for that
    // frame, including after navigation, rather than adopting into an unknown name.
    for index in 0..side.pickers.len() {
        let picker = side.pickers.get_mut(index).expect("picker index");
        if Some(picker.forge) != shown {
            // Hidden flows do not read. Returning starts a fresh wait, keeping
            // the same retry bound rather than restarting it on navigation.
            picker.retry_at = None;
        }
        advance(side, client, limits, index, out);
    }
}
pub(crate) fn next_deadline(side: &Side) -> Option<Time> {
    if side.party.is_none() || side.served == Served::OutOfDate {
        return None;
    }
    let mut next = None;
    for picker in &side.pickers {
        if let Some(at) = picker.retry_at {
            next = Some(match next {
                Some(earlier) => at.min(earlier),
                None => at,
            });
        }
    }
    next
}
fn read(side: &mut Side, limits: Limits, forge: u16, out: &mut Queue<Request>) {
    let Some(index) = index(side, forge) else { return };
    let picker = side.pickers.get_mut(index).expect("picker");
    let bounds = super::wire_limits(&limits).expect("validated limits");
    let request = w::Request::Reachable(
        w::Reachable::new(
            &bounds,
            w::ReachableParts { forge, installation: Some(picker.installation), page: picker.page },
        )
        .expect("picker request fits"),
    );
    let request = w::Requesting::new(&bounds, w::RequestingParts { request }).expect("request fits");
    side.serial = side.serial.checked_add(1).expect("forge serial fits");
    let key = Key { asker: crate::owner::Owner::Forge.asker(), number: side.serial };
    picker.key = Some(key);
    picker.answer = None;
    picker.retry_at = None;
    picker.chosen = 0;
    out.push(Request::Send {
        request: Token::new(key.number),
        name: None,
        ask: Ask::Family {
            scope: c::Scope::Deployment,
            record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&request) },
        },
    });
    side.changed();
}
pub(crate) fn act(side: &mut Side, limits: Limits, forge: u16, act: Act, out: &mut Queue<Request>) {
    if side.party.is_none() || side.served == Served::OutOfDate {
        return;
    }
    let Some(index) = index(side, forge) else { return };
    let picker = side.pickers.get_mut(index).expect("picker");
    if picker.party != side.party.expect("confirmed") || picker.pending() {
        return;
    }
    match act {
        Act::Account(id) => {
            if installation(&picker.installations, Some(id)).is_none() {
                return;
            }
            picker.installation = id;
            picker.page = 1;
            picker.busy_rereads = 0;
            read(side, limits, forge, out);
        }
        Act::Page(page) => {
            let Some(github::Answer::Page(answer)) = &picker.answer else { return };
            if page == 0 || (page != 1 && (picker.page.checked_add(1) != Some(page) || !answer.more)) {
                return;
            }
            picker.page = page;
            picker.busy_rereads = 0;
            read(side, limits, forge, out);
        }
        Act::Choose(chosen) => {
            let Some(github::Answer::Page(page)) = &picker.answer else { return };
            if usize::try_from(chosen).expect("row index fits") >= page.rows.len() {
                return;
            }
            picker.chosen = chosen;
            picker.follow();
            picker.checked = false;
        }
        Act::Write(text) => {
            if text.len() > usize::try_from(limits.field_bytes).expect("field bound fits") {
                return;
            }
            picker.name = text;
            picker.named = true;
            picker.written = picker.written.checked_add(1).expect("picker writes fit");
            picker.checked = false;
        }
        Act::Adopt => {
            picker.checked = true;
            if picker.name.is_empty() {
                side.changed();
                return;
            }
            let Some(row) = picker.selected() else { return };
            let adoption = super::Adoption {
                project: 0,
                forge,
                owner: row.owner.login.clone(),
                name: row.name.clone(),
                prefix: Box::from(b"temper/".as_slice()),
                role: super::Role::Owned,
                landing: Box::from(b"main".as_slice()),
                ci: true,
                checks: Box::default(),
                home: true,
            };
            let name = picker.name.clone();
            let starting = forges::start_in(side, limits, forge, adoption, name, out);
            let picker = side.pickers.get_mut(index).expect("picker");
            picker.starting = starting;
            picker.adoption = None;
        }
    }
    side.changed();
}
pub(crate) fn told(side: &mut Side, client: &Client, told: &Told, limits: Limits) -> bool {
    let (key, outcome) = match told {
        Told::Answered { key, outcome } => (*key, outcome.clone()),
        Told::Pending { key } => match client.pending(*key) {
            Some(pending) if pending.progress == jig_client::Progress::Unknown => (*key, Outcome::Unknown),
            Some(_) | None => return false,
        },
        Told::Link { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Changed { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => return false,
    };
    for index in 0..side.pickers.len() {
        let picker = side.pickers.get_mut(index).expect("picker");
        if picker.key == Some(key) {
            if picker.answer.is_none() {
                if outcome == Outcome::Unknown {
                    picker.key = None;
                    side.changed();
                    return true;
                }
                match github::decode(&outcome, false, limits) {
                    Some(github::Answer::Unknown) => picker.key = None,
                    Some(answer) => picker.answer = Some(answer),
                    None => picker.answer = Some(github::Answer::Unknown),
                }
                picker.follow();
                side.changed();
            }
            return true;
        }
        if let Some(starting) = &mut picker.starting
            && key.asker == crate::owner::Owner::Forge.asker()
            && key.number == starting.request.raw()
        {
            if starting.value.outcome.is_none() {
                starting.value.answered(&outcome);
                side.changed();
            }
            return true;
        }
    }
    false
}
fn advance(side: &mut Side, client: &Client, limits: Limits, index: u32, out: &mut Queue<Request>) {
    let picker = side.pickers.get_mut(index).expect("picker");
    if picker.adoption.is_some() || side.party != Some(picker.party) {
        return;
    }
    let Some(starting) = &picker.starting else { return };
    let Some(same_name) = starting.value.same_project_name(client) else { return };
    let adoption = starting.value.adoption.clone();
    let name = starting.value.name.clone();
    if same_name {
        let pending = forges::adoption_request(side, limits, &adoption, false, out);
        side.pickers.get_mut(index).expect("picker").adoption = pending;
    } else {
        // A different banner name belongs to an earlier attempt after a reload.
        // Try start-project-F-1 (and onward), leaving that project intact.
        let forge = picker.forge;
        let starting = forges::start_in(side, limits, forge, adoption, name, out);
        side.pickers.get_mut(index).expect("picker").starting = starting;
    }
    side.changed();
}
pub(crate) fn adopted(side: &mut Side, request: Token, answer: &super::Answer) -> bool {
    for index in 0..side.pickers.len() {
        let picker = side.pickers.get_mut(index).expect("picker");
        if let Some(adoption) = &mut picker.adoption
            && adoption.request == request
        {
            if adoption.outcome.is_none() {
                adoption.outcome = Some(answer.clone());
                side.changed();
            }
            return true;
        }
    }
    false
}
