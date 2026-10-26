//! An App's editable id and one-shot key (client/forge.md, 5.4).
use super::{Limits, Request, Served, Side, forges};
use alloc::boxed::Box;
use jig_client::{Answered, Ask, Event, Family, Key, Outcome, Told, contract as c};
use skein_lib::{Queue, Secret};
use temper_client_protocol_wire::{self as wire, v1 as w};

#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads presentation while the one-shot key stays private")]
pub struct Draft {
    pub id: Box<[u8]>,
    pub written: u64,
    pub key_written: u64,
    pub checked: bool,
    pub too_long: bool,
    key: Option<Secret>,
}
impl Draft {
    pub(crate) fn new() -> Self {
        Self { id: Box::default(), written: 0, key_written: 0, checked: false, too_long: false, key: None }
    }
    #[must_use]
    pub fn has_key(&self) -> bool {
        match &self.key {
            Some(key) => !forges::trimmed_app_key(key.expose()).is_empty(),
            None => false,
        }
    }
    #[must_use]
    pub fn number(&self) -> Option<u64> {
        let mut number = 0_u64;
        if self.id.is_empty() {
            return None;
        }
        for byte in &self.id {
            if !byte.is_ascii_digit() {
                return None;
            }
            number = number.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
        }
        if number == 0 { None } else { Some(number) }
    }
    pub(crate) fn clear(&mut self) {
        self.key = None;
        self.key_written = self.key_written.checked_add(1).expect("key writes fit");
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Connecting,
    Link(Box<[u8]>),
    Page(super::family::Page),
    NoAccess,
    NotFound,
    Refused(w::ForgeRefusal),
    Entrance(c::EntranceRefusal),
    Unknown,
}
#[derive(Debug)]
pub struct Pending {
    pub forge: u16,
    pub key: Key,
    pub linking: bool,
    pub answer: Option<Answer>,
}
pub(crate) fn write(side: &mut Side, text: Box<[u8]>, limits: Limits) {
    if let Some(pending) = &side.app
        && pending.answer.is_none()
    {
        return;
    }
    if text.len() > usize::try_from(limits.field_bytes).expect("field bound fits") {
        return;
    }
    side.app_form.id = text;
    side.app_form.written = side.app_form.written.checked_add(1).expect("id writes fit");
    side.app_form.checked = false;
    side.app = None;
    side.changed();
}
pub(crate) fn key(side: &mut Side, value: Secret) {
    if let Some(pending) = &side.app
        && pending.answer.is_none()
    {
        return;
    }
    side.app_form.key = None;
    side.app = None;
    side.app_form.too_long = value.len() > 4090;
    if !value.is_empty() && !side.app_form.too_long {
        side.app_form.key = Some(value);
    }
    side.app_form.checked = false;
    side.changed();
}
pub(crate) fn send(side: &mut Side, limits: Limits, forge: u16, back: Option<Box<[u8]>>, out: &mut Queue<Request>) {
    if !available(side) {
        return;
    }
    let installing = back.is_some();
    let bounds = super::wire_limits(&limits).expect("checked forge limits");
    let request = if let Some(back) = back {
        w::Request::Install(
            w::Install::new(&bounds, w::InstallParts { forge, back: Some(back) }).expect("printed flow address fits"),
        )
    } else {
        side.app_form.checked = true;
        let Some(id) = side.app_form.number() else {
            side.changed();
            return;
        };
        if !side.app_form.has_key() {
            side.changed();
            return;
        }
        let Some(key) = &side.app_form.key else {
            side.changed();
            return;
        };
        let pem = key.expose();
        if forges::restored_app_key(pem).is_none() {
            side.app_form.too_long = !pem.is_empty();
            side.changed();
            return;
        }
        w::Request::Connect(
            w::Connect::new(
                &bounds,
                w::ConnectParts {
                    forge,
                    how: w::How::GithubApp(w::GitHubApp::new(&bounds, w::GitHubAppParts { id }).expect("valid App id")),
                },
            )
            .expect("connect fits"),
        )
    };
    dispatch(side, &bounds, forge, request, installing, out);
}
fn available(side: &Side) -> bool {
    side.party.is_some()
        && side.served != Served::OutOfDate
        && match &side.app {
            Some(pending) => pending.answer.is_some(),
            None => true,
        }
}
pub(crate) fn manifest(side: &mut Side, limits: Limits, forge: u16, back: Box<[u8]>, out: &mut Queue<Request>) {
    if !available(side) {
        return;
    }
    let bounds = super::wire_limits(&limits).expect("checked forge limits");
    let how = w::How::GithubManifest(
        w::GitHubManifest::new(&bounds, w::GitHubManifestParts { back: Some(back) })
            .expect("printed flow address fits"),
    );
    let request = w::Request::Connect(w::Connect::new(&bounds, w::ConnectParts { forge, how }).expect("connect fits"));
    dispatch(side, &bounds, forge, request, true, out);
}
fn dispatch(
    side: &mut Side,
    bounds: &w::Limits,
    forge: u16,
    request: w::Request,
    linking: bool,
    out: &mut Queue<Request>,
) {
    let record = w::Requesting::new(bounds, w::RequestingParts { request }).expect("request fits");
    let request = Ask::Family {
        scope: c::Scope::Deployment,
        record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&record) },
    };
    side.serial = side.serial.checked_add(1).expect("forge keys fit");
    let key = Key { asker: crate::owner::Owner::Forge.asker(), number: side.serial };
    let event = if linking {
        Event::Send { key, name: None, request }
    } else {
        let value = side.app_form.key.take().expect("validated key");
        let pem = value.expose();
        let value = forges::restored_app_key(pem).expect("validated bundle");
        side.app_form.clear();
        // A fresh request name, never reused with another pasted value.
        let name = crate::key::named(
            b"temper-app",
            &crate::key::Name::new(skein_lib::Decimal::of(side.serial).as_bytes()).expect("serial name fits"),
        );
        Event::Carry { key, name, request, value: Some(value) }
    };
    side.app = Some(Pending { forge, key, linking, answer: None });
    out.push(Request::Client { asked: event });
    side.changed();
}
pub(crate) fn told(
    side: &mut Side,
    client: &jig_client::Client,
    told: &Told,
    limits: Limits,
    out: &mut Queue<Request>,
) -> bool {
    let Some(pending) = &mut side.app else {
        return false;
    };
    let outcome = match told {
        Told::Answered { key, outcome } if *key == pending.key => outcome,
        Told::Pending { key } if *key == pending.key => {
            if let Some(value) = client.pending(*key)
                && value.progress == jig_client::Progress::Unknown
            {
                pending.answer = Some(Answer::Unknown);
                side.changed();
            }
            return true;
        }
        Told::Link { .. }
        | Told::Pending { .. }
        | Told::Answered { .. }
        | Told::Kept { .. }
        | Told::Dropped { .. }
        | Told::Changed { .. }
        | Told::Text { .. }
        | Told::Ended { .. }
        | Told::Page { .. } => return false,
    };
    if pending.answer.is_some() {
        return true;
    }
    let answer = decode(outcome, pending.linking, limits).unwrap_or(Answer::Unknown);
    if let Answer::Link(to) = &answer {
        out.push(Request::Leave { to: to.clone() });
    }
    pending.answer = Some(answer);
    side.changed();
    true
}
pub(crate) fn decode(outcome: &Outcome, linking: bool, limits: Limits) -> Option<Answer> {
    let Outcome::Answered(answer) = outcome else {
        return Some(Answer::Unknown);
    };
    match answer {
        Answered::Jig(c::Answer::Refused { refusal, .. }) => {
            Some(Answer::Entrance(c::EntranceRefusal::new(refusal.clone()).ok()?))
        }
        Answered::Family(record) if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION => {
            let mut reader = skein_lib::Reader::new(&record.bytes);
            let answer = w::Answer::decode(&super::wire_limits(&limits)?, &mut reader).ok()?;
            if !reader.is_empty() {
                return None;
            }
            match answer.into_parts().outcome {
                w::Outcome::Connecting => Some(Answer::Connecting),
                w::Outcome::ConnectRefused(refusal) => Some(Answer::Refused(refusal.why().clone())),
                w::Outcome::Link(link) if linking => Some(Answer::Link(link.into_parts().address)),
                w::Outcome::Unknown => Some(Answer::Unknown),
                w::Outcome::Page(page) => Some(Answer::Page(super::family::page(page))),
                w::Outcome::ForgeRefused(refusal) => match refusal.into_parts().reason {
                    w::ForgeReason::NoAccess => Some(Answer::NoAccess),
                    w::ForgeReason::NotFound => Some(Answer::NotFound),
                    w::ForgeReason::NoSuchForge
                    | w::ForgeReason::Refused
                    | w::ForgeReason::InFlight
                    | w::ForgeReason::TooLarge
                    | w::ForgeReason::Unreachable
                    | w::ForgeReason::InvalidAnswer => None,
                },
                w::Outcome::Accepted(_)
                | w::Outcome::Adopted(_)
                | w::Outcome::CoreRefused(_)
                | w::Outcome::SettingUp
                | w::Outcome::SetUpRefused(_)
                | w::Outcome::Running
                | w::Outcome::StartRefused(_)
                | w::Outcome::StartFailed(_)
                | w::Outcome::Returning
                | w::Outcome::Link(_) => None,
            }
        }
        Answered::Jig(_) | Answered::Family(_) => None,
    }
}
