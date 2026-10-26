//! A site admin's one-shot token and first repository (client/forge.md, 5.4).
use super::{Limits, Request, Served, Side, forges, github};
use alloc::boxed::Box;
use jig_client::{Ask, Event, Family, Key, Told, contract as c};
use skein_lib::{Queue, Secret};
use temper_client_protocol_wire::{self as wire, v1 as w};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Organization,
    Repository,
    Project,
    Label,
}
impl Field {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Organization => 0,
            Self::Repository => 1,
            Self::Project => 2,
            Self::Label => 3,
        }
    }
}
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the view reads presentation; the token stays private")]
pub struct Draft {
    pub fields: [Box<[u8]>; 4],
    pub written: [u64; 4],
    pub token_written: u64,
    pub checked: bool,
    pub too_long: bool,
    named: bool,
    token: Option<Secret>,
}
impl Draft {
    pub(crate) fn new() -> Self {
        Self {
            fields: [
                b"work".as_slice().into(),
                b"hello".as_slice().into(),
                b"hello".as_slice().into(),
                b"docker".as_slice().into(),
            ],
            written: [0; 4],
            token_written: 0,
            checked: false,
            too_long: false,
            named: false,
            token: None,
        }
    }
    #[must_use]
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }
    #[must_use]
    pub fn valid_label(&self) -> bool {
        let label = &self.fields[3];
        if label.is_empty() || label.len() > 64 {
            return false;
        }
        for byte in label {
            if !byte.is_ascii_alphanumeric() && !b"._-".contains(byte) {
                return false;
            }
        }
        true
    }
    pub(crate) fn clear(&mut self) {
        self.token = None;
        self.checked = false;
        self.too_long = false;
        self.token_written = self.token_written.checked_add(1).expect("token writes fit");
    }
}
#[derive(Debug)]
pub struct Pending {
    pub forge: u16,
    pub key: Key,
    pub answer: Option<github::Answer>,
}
fn busy(side: &Side) -> bool {
    match &side.admin {
        Some(pending) => pending.answer.is_none(),
        None => false,
    }
}
pub(crate) fn token(side: &mut Side, value: Secret) {
    if busy(side) {
        return;
    }
    side.admin_form.token = None;
    side.admin_form.too_long = value.len() > 4090;
    if !value.is_empty() && !side.admin_form.too_long {
        side.admin_form.token = Some(value);
    }
    side.admin_form.checked = false;
    side.changed();
}
pub(crate) fn write(side: &mut Side, limits: Limits, field: Field, text: Box<[u8]>) {
    if busy(side) || text.len() > usize::try_from(limits.field_bytes).expect("field bound fits") {
        return;
    }
    let draft = &mut side.admin_form;
    if field == Field::Project {
        draft.named = true;
    }
    if field == Field::Repository && !draft.named {
        draft.fields[2].clone_from(&text);
        draft.written[2] = draft.written[2].checked_add(1).expect("writes fit");
    }
    let index = field.index();
    *draft.fields.get_mut(index).expect("four fields") = text;
    let written = draft.written.get_mut(index).expect("four fields");
    *written = written.checked_add(1).expect("writes fit");
    draft.checked = false;
    side.changed();
}
pub(crate) fn send(side: &mut Side, limits: Limits, forge: u16, out: &mut Queue<Request>) {
    let Some(party) = side.party else { return };
    if busy(side) || side.served == Served::OutOfDate {
        return;
    }
    let Some(row) = super::connect::row(forge, &side.forges, side.demo()) else { return };
    if !row.admin || row.kind != forges::Kind::Forgejo {
        return;
    }
    match row.connection {
        forges::Connection::NotConnected | forges::Connection::Stopped { .. } => {}
        forges::Connection::Connected { .. } if super::connect::failed(row) => {}
        forges::Connection::NotSetUp
        | forges::Connection::SettingUp { .. }
        | forges::Connection::SetUpFailed { .. }
        | forges::Connection::Connecting
        | forges::Connection::Connected { .. } => return,
    }
    let draft = &mut side.admin_form;
    draft.checked = true;
    if !draft.has_token()
        || !draft.valid_label()
        || draft.fields[0].is_empty()
        || draft.fields[1].is_empty()
        || draft.fields[2].is_empty()
    {
        side.changed();
        return;
    }
    let bounds = super::wire_limits(&limits).expect("checked limits");
    let project = forges::SetUpProject::New(draft.fields[2].clone());
    let first = w::FirstAsk::new(
        &bounds,
        w::FirstAskParts {
            org: draft.fields[0].clone(),
            repository: draft.fields[1].clone(),
            project: w::SetUpProject::New(
                w::NewProject::new(&bounds, w::NewProjectParts { name: draft.fields[2].clone() })
                    .expect("project fits"),
            ),
            label: draft.fields[3].clone(),
        },
    );
    let Ok(first) = first else {
        side.changed();
        return;
    };
    let how = w::How::ForgejoAdmin(
        w::ForgejoAdmin::new(&bounds, w::ForgejoAdminParts { first: Some(first) }).expect("first fits"),
    );
    let request = w::Request::Connect(w::Connect::new(&bounds, w::ConnectParts { forge, how }).expect("connect fits"));
    let record = w::Requesting::new(&bounds, w::RequestingParts { request }).expect("request fits");
    let request = Ask::Family {
        scope: c::Scope::Deployment,
        record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&record) },
    };
    let token = draft.token.take().expect("validated token");
    let mut bytes = skein_lib::Writer::new(token.len().checked_add(6).expect("bundle fits"));
    bytes.put(&[1, 1, 1, 0]).expect("header fits");
    bytes.put(&u16::try_from(token.len()).expect("bounded token").to_le_bytes()).expect("length fits");
    bytes.put(token.expose()).expect("token fits");
    draft.clear();
    side.serial = side.serial.checked_add(1).expect("keys fit");
    let key = Key { asker: crate::owner::Owner::Forge.asker(), number: side.serial };
    let name = crate::key::named(
        b"temper-admin",
        &crate::key::Name::new(skein_lib::Decimal::of(side.serial).as_bytes()).expect("serial fits"),
    );
    side.set_up = Some(Box::new(forges::SetUp {
        playground: false,
        forge,
        party,
        org: draft.fields[0].clone(),
        repository: draft.fields[1].clone(),
        project,
        adoption: None,
        starting: None,
        request: skein_lib::Token::new(side.serial),
        progress: None,
        answer: None,
    }));
    side.admin = Some(Pending { forge, key, answer: None });
    out.push(Request::Client { asked: Event::Carry { key, name, request, value: Some(Secret::new(bytes.finish())) } });
    side.changed();
}
pub(crate) fn told(
    side: &mut Side,
    client: &jig_client::Client,
    told: &Told,
    limits: Limits,
    _out: &mut Queue<Request>,
) -> bool {
    let Some(pending) = &mut side.admin else { return false };
    let answer = match told {
        Told::Answered { key, outcome } if *key == pending.key => github::decode(outcome, false, limits),
        Told::Pending { key } if *key == pending.key => {
            if let Some(value) = client.pending(*key)
                && value.progress == jig_client::Progress::Unknown
            {
                Some(github::Answer::Unknown)
            } else {
                return true;
            }
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
    if pending.answer.is_none() {
        pending.answer = Some(answer.unwrap_or(github::Answer::Unknown));
        side.admin_form.clear();
        side.changed();
    }
    true
}
pub(crate) fn stopped(side: &mut Side) {
    let forges::State::Rows(rows) = &side.forges else { return };
    for row in rows {
        if !row.admin {
            continue;
        }
        if let Some(pending) = &side.admin
            && pending.forge != row.number
        {
            continue;
        }
        if super::connect::admin_stopped(row) {
            side.admin_form.clear();
            if let Some(pending) = &mut side.admin
                && pending.answer.is_none()
            {
                pending.answer = Some(github::Answer::Connecting);
            }
        }
        if side.admin.is_none()
            && side.admin_form.written == [0; 4]
            && let Some(first) = &row.first
        {
            side.admin_form.fields[0].clone_from(&first.org);
            side.admin_form.fields[1].clone_from(&first.repository);
            if let forges::SetUpProject::New(name) = &first.project {
                side.admin_form.fields[2].clone_from(name);
                side.admin_form.named = *name != first.repository;
            }
            side.admin_form.fields[3].clone_from(&first.label);
            side.admin_form.written = [1; 4];
        }
    }
}
