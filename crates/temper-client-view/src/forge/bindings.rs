//! One bounded table for the last tree, with typed form input (client/README.md, 5.2).
use alloc::boxed::Box;
use skein_lib::{List, Token};
use skein_ui::{Act, Binding};
use temper_client_domain::{Action, forge};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meaning {
    Field(forge::Text),
    Role(forge::Role),
    Home,
    Ci,
    Adopt,
    AdminToken,
    AdminField(forge::admin::Field),
    ConnectAdmin(u16),
    AppId,
    AppKey,
    ConnectApp(u16),
    ConnectManifest { forge: u16, back: temper_client_domain::Address },
    Install { forge: u16, back: temper_client_domain::Address },
    SetUp(u16),
    SetUpAtOnce(u16),
    SetupField(forge::forges::SetupField),
    StartIn(u16),
    Picker { forge: u16, act: forge::picker::Act },
    PickerName(u16),
}
#[derive(Debug)]
struct Entry {
    token: Token,
    meaning: Meaning,
}
#[derive(Debug)]
pub struct Bindings {
    entries: List<Entry>,
    next: u64,
    connector: u16,
    checks: u32,
}
impl Bindings {
    #[must_use]
    pub fn new(capacity: u32, checks: u32, connector: u16) -> Self {
        Self { entries: List::with_capacity(capacity), next: 0, connector, checks }
    }
    pub fn begin(&mut self) {
        self.entries.clear();
    }
    #[must_use]
    pub fn insert(&mut self, meaning: Meaning) -> Binding {
        self.next = self.next.checked_add(1).expect("binding tokens fit");
        let token = Token::new(self.next);
        self.entries.push(Entry { token, meaning }).expect("form bindings fit");
        Binding { owner: forge::words::owner(self.connector), token }
    }
}
#[must_use]
pub fn decode(bindings: &Bindings, token: Token, act: Act) -> Option<Action> {
    for entry in &bindings.entries {
        if entry.token != token {
            continue;
        }
        return match (entry.meaning.clone(), act) {
            (Meaning::PickerName(forge), Act::Input { value }) => {
                Some(Action::Forge(forge::Act::Picker { forge, act: forge::picker::Act::Write(value) }))
            }
            (Meaning::AdminField(field), Act::Input { value }) => {
                Some(Action::Forge(forge::Act::AdminWrite { field, text: value }))
            }
            (Meaning::AppId, Act::Input { value }) => Some(Action::Forge(forge::Act::AppWrite { text: value })),
            (Meaning::SetupField(field), Act::Input { value }) => {
                Some(Action::Forge(forge::Act::SetupWrite { field, text: value }))
            }
            (Meaning::Field(field), Act::Input { value }) => {
                let typed = match field {
                    forge::Text::Forge => forge::Typed::Forge(forge_number(&value)),
                    forge::Text::Checks => forge::Typed::Checks(checks(&value, bindings.checks)),
                    forge::Text::Owner | forge::Text::Name | forge::Text::Prefix | forge::Text::Landing => {
                        forge::Typed::Bytes
                    }
                };
                Some(Action::Forge(forge::Act::Write { field, text: value, typed }))
            }
            (meaning, Act::Press | Act::Toggle) => Some(match meaning {
                Meaning::Role(role) => Action::Forge(forge::Act::Choose { role }),
                Meaning::Home => Action::Forge(forge::Act::Home),
                Meaning::Ci => Action::Forge(forge::Act::Ci),
                Meaning::SetUp(forge) => Action::Forge(forge::Act::SetUp { forge }),
                Meaning::SetUpAtOnce(forge) => Action::Forge(forge::Act::SetUpAtOnce { forge }),
                Meaning::ConnectAdmin(forge) => Action::Forge(forge::Act::ConnectAdmin { forge }),
                Meaning::ConnectApp(forge) => Action::Forge(forge::Act::ConnectApp { forge }),
                Meaning::ConnectManifest { forge, back } => Action::Forge(forge::Act::ConnectManifest {
                    forge,
                    back: temper_client_domain::address::print(&back),
                }),
                Meaning::Install { forge, back } => {
                    Action::Forge(forge::Act::Install { forge, back: temper_client_domain::address::print(&back) })
                }
                Meaning::Adopt => Action::Forge(forge::Act::Adopt),
                Meaning::Picker { forge, act } => Action::Forge(forge::Act::Picker { forge, act }),
                Meaning::StartIn(forge) => Action::Forge(forge::Act::StartIn { forge }),
                Meaning::PickerName(_)
                | Meaning::AdminField(_)
                | Meaning::AdminToken
                | Meaning::AppId
                | Meaning::AppKey
                | Meaning::Field(_)
                | Meaning::SetupField(_) => return None,
            }),
            (_, Act::Input { .. } | Act::Submit | Act::Drawing { .. }) => None,
        };
    }
    None
}
#[must_use]
pub fn forge_number(text: &[u8]) -> Option<u16> {
    let number = u16::try_from(skein_ui::number(text)?).ok()?;
    if number == 0 { None } else { Some(number) }
}
#[must_use]
pub fn checks(text: &[u8], most: u32) -> Option<Box<[u32]>> {
    let capacity = most.min(u32::try_from(text.len()).ok()?);
    let mut values = List::with_capacity(capacity);
    let mut start = None;
    for (index, byte) in text.iter().enumerate() {
        if *byte == b' ' || *byte == b',' {
            if let Some(from) = start {
                check(&mut values, text.get(from..index)?)?;
                start = None;
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(from) = start {
        check(&mut values, text.get(from..)?)?;
    }
    Some(values.into_boxed())
}
fn check(values: &mut List<u32>, text: &[u8]) -> Option<()> {
    let value = u32::try_from(skein_ui::number(text)?).ok()?;
    if value == 0 || values.as_slice().contains(&value) {
        return None;
    }
    values.push(value).ok()
}

/// Move secret input straight into the forge event, outside clonable actions.
#[must_use]
pub fn event(bindings: &Bindings, token: Token, act: Act) -> Option<temper_client_domain::Event> {
    for entry in &bindings.entries {
        if entry.token == token && (entry.meaning == Meaning::AppKey || entry.meaning == Meaning::AdminToken) {
            if let Act::Input { value } = act {
                let value = skein_lib::Secret::new(value);
                return Some(temper_client_domain::Event::Forge(if entry.meaning == Meaning::AdminToken {
                    forge::Event::AdminToken { value }
                } else {
                    forge::Event::AppKey { value }
                }));
            }
            return None;
        }
    }
    Some(temper_client_domain::Event::Act { action: decode(bindings, token, act)? })
}
