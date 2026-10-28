//! Accounts bindings contain row targets and operation names, never a key value.
use skein_lib::{List, Secret, Token};
use skein_ui::{Act, Binding};
use temper_client_domain::{
    Action, Event,
    pages::{self, accounts::Act as AccountAct},
};
pub(crate) mod words;
#[derive(Clone, Debug)]
pub(crate) enum Meaning {
    Action(AccountAct),
    Model(pages::connect_model::Act),
    Go(temper_client_domain::Address),
    Key,
    SignInAddress,
    Program,
}
#[derive(Debug)]
pub(crate) struct Bindings {
    entries: List<(Token, Meaning)>,
    next: u64,
    revision: u64,
    counts: temper_client_domain::limits::AccountLimits,
}
pub(crate) const fn owner() -> skein_ui::Owner {
    // View-local owner, outside the u16 wire family/asker namespace.
    skein_ui::Owner { number: 65536 }
}
impl Bindings {
    pub fn new(counts: temper_client_domain::limits::AccountLimits, providers: u32) -> Self {
        let capacity = binding_capacity(&counts, providers);
        Self { entries: List::with_capacity(capacity), next: 0, revision: 0, counts }
    }
    pub const fn counts(&self) -> temper_client_domain::limits::AccountLimits {
        self.counts
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn begin(&mut self, _reset: bool) {
        self.entries.clear();
    }
    pub fn insert(&mut self, meaning: Meaning) -> Option<Binding> {
        self.next = self.next.checked_add(1)?;
        let token = Token::new(self.next);
        self.entries.push((token, meaning)).ok()?;
        Some(Binding { owner: owner(), token })
    }
    pub fn decode(&self, token: Token, act: Act) -> Option<Event> {
        for (key, meaning) in &self.entries {
            if *key != token {
                continue;
            }
            match (meaning, &act) {
                (Meaning::Model(action), Act::Press) => {
                    return Some(Event::Act { action: Action::Pages(pages::Act::ConnectModel(action.clone())) });
                }
                (Meaning::Go(address), Act::Press) => {
                    return Some(Event::Act { action: Action::Pages(pages::Act::Go { address: address.clone() }) });
                }
                (Meaning::Program, Act::Input { value }) => {
                    return Some(Event::Act {
                        action: Action::Pages(pages::Act::ConnectModel(pages::connect_model::Act::ProgramName {
                            value: value.clone(),
                        })),
                    });
                }
                (Meaning::Model(_) | Meaning::Go(_), _) => return None,
                _ => {}
            }
            let action = match (meaning, act) {
                (Meaning::SignInAddress, Act::Input { value }) => {
                    return Some(Event::SignInAddress { value: Secret::new(value) });
                }
                (Meaning::Key, Act::Input { value }) => return Some(Event::AccountKey { value: Secret::new(value) }),
                (Meaning::Action(action), Act::Press) => action.clone(),
                (_, Act::Submit | Act::Toggle | Act::Drawing { .. })
                | (Meaning::Action(_) | Meaning::Model(_) | Meaning::Go(_) | Meaning::Program, Act::Input { .. })
                | (
                    Meaning::Key | Meaning::SignInAddress | Meaning::Program | Meaning::Model(_) | Meaning::Go(_),
                    Act::Press,
                ) => return None,
            };
            return Some(Event::Act { action: Action::Pages(pages::Act::Accounts(action)) });
        }
        None
    }
}
/// Each of two account lists has source actions, state actions, ranks and one editor.
pub(crate) fn binding_capacity(counts: &temper_client_domain::limits::AccountLimits, providers: u32) -> u32 {
    let source = counts.sources.checked_mul(6).expect("source controls fit");
    let accounts = source
        .checked_add(16)
        .expect("row controls fit")
        .checked_mul(counts.accounts)
        .expect("account controls fit")
        .checked_mul(2)
        .expect("two holders fit");
    accounts
        .checked_add(providers.checked_mul(3).expect("ways fit"))
        .expect("controls fit")
        .checked_add(16)
        .expect("page controls fit")
}
