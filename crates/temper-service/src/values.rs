//! Lent account credentials stay in the protocol layer (protocol/credentials.md, 3).
use alloc::boxed::Box;
use jig_domain_host as host;
use jig_protocol_accounts::table::{self, Table};
use jig_protocol_engine_workers::{GrantValue, Scheme};
use skein_lib::{List, Map, Secret, Time};

/// A configured key can exceed the OAuth token bound. The worker has no
/// provider declarations and reserves the key ceiling unconditionally.
pub(crate) fn credential_bytes(engine: &jig_protocol_config::Engine) -> u32 {
    let token = engine.limits.token;
    for provider in &engine.llm.providers {
        if provider.keys {
            return crate::worker_settings::credential_bytes(token);
        }
    }
    token
}
/// The engine's git identities' values (protocol/credentials.md, 4): two newest
/// generations per number, each encoded once by temper-protocol-worker-wire.
#[derive(Debug)]
pub struct Identities {
    entries: Map<u32, Generations>,
}
#[derive(Debug)]
struct Generations {
    generation: u64,
    newest: Secret,
    previous: Option<Secret>,
}
impl Identities {
    /// Room for each configured identity's number.
    #[must_use]
    pub fn with_capacity(identities: u32) -> Self {
        Self { entries: Map::with_capacity(identities) }
    }
    /// Keep the next generation, pushing out the older of the two retained values.
    pub fn put(&mut self, number: u32, value: Secret) -> u64 {
        if let Some(entry) = self.entries.get_mut(&number) {
            entry.previous = Some(core::mem::replace(&mut entry.newest, value));
            entry.generation = entry.generation.checked_add(1).expect("identity generations");
            return entry.generation;
        }
        self.entries
            .insert(number, Generations { generation: 1, newest: value, previous: None })
            .expect("configured identity capacity");
        1
    }
    /// Either of the two newest generations, or none for an older one.
    #[must_use]
    pub fn get(&self, number: u32, generation: u64) -> Option<&Secret> {
        let entry = self.entries.get(&number)?;
        if generation == entry.generation {
            Some(&entry.newest)
        } else if generation != 0 && generation.checked_add(1) == Some(entry.generation) {
            entry.previous.as_ref()
        } else {
            None
        }
    }
    /// The newest generation and its value.
    #[must_use]
    pub fn newest(&self, number: u32) -> Option<(u64, &Secret)> {
        let entry = self.entries.get(&number)?;
        Some((entry.generation, &entry.newest))
    }
    pub(crate) fn numbers(&self) -> List<u32> {
        let mut numbers = List::with_capacity(self.entries.len());
        for (number, _) in &self.entries {
            numbers.push(*number).expect("identity count");
        }
        numbers
    }
}
#[derive(Debug)]
pub struct Values {
    pub accounts: Table,
    /// The two newest encoded values of each configured git identity.
    pub identities: Identities,
    pub tokens: crate::forge::tokens::Tokens,
}
impl Values {
    /// Named source slots, bounded by the accounts driver.
    #[must_use]
    pub fn new(accounts: u32, token_bytes: u32) -> Self {
        Self {
            accounts: Table::new(accounts, token_bytes),
            identities: Identities::with_capacity(0),
            tokens: crate::forge::tokens::Tokens::empty(),
        }
    }
}

impl Values {
    /// One optional value per grant, preserving the root's order.
    #[must_use]
    pub fn grants(&self, grants: &[host::Grant], now: Time) -> Box<[Option<GrantValue>]> {
        let mut values = List::with_capacity(u32::try_from(grants.len()).expect("bounded grants"));
        for grant in grants {
            let value = match grant.form {
                host::Form::Credential(_) => {
                    account_grant(grant.form, self.accounts.value(grant.account, grant.generation, now))
                }
                host::Form::Application => identity_grant(self.identities.get(grant.account, grant.generation)),
                host::Form::Program(name) => Some(GrantValue::Program { name }),
            };
            values.push(value).expect("one slot per grant");
        }
        values.into_boxed()
    }
}

fn account_grant(form: host::Form, value: Option<&table::Value>) -> Option<GrantValue> {
    let value = value?;
    let actual = match &value.scheme {
        table::Scheme::Bearer => host::Form::Credential(host::Scheme::Bearer),
        table::Scheme::Key => host::Form::Credential(host::Scheme::Key),
    };
    if form != actual {
        return None;
    }
    Some(match &value.scheme {
        table::Scheme::Bearer => GrantValue::Credential { scheme: Scheme::Bearer, token: value.token.clone() },
        table::Scheme::Key => GrantValue::Credential { scheme: Scheme::Key, token: value.token.clone() },
    })
}
fn identity_grant(value: Option<&Secret>) -> Option<GrantValue> {
    let value = value?;
    Some(GrantValue::Application { secret: value.clone() })
}
impl Values {
    pub(crate) fn fill(
        &mut self,
        settings: &crate::settings::Engine,
        limits: &temper_protocol_worker_wire::v1::Limits,
    ) -> Result<(), jig_protocol_config::Problems> {
        use jig_protocol_config::What;
        let identities = &settings.identities;
        self.tokens = crate::forge::tokens::Tokens::new(settings);
        self.identities = Identities::with_capacity(u32::try_from(identities.len()).expect("identity count"));
        for identity in identities {
            // GitHub gathers tokens until its first Connected.
            if identity.token.expose().is_empty() {
                continue;
            }
            let value = crate::translate::identity(identity, limits).or(Err(identity_problem(What::Bound)))?;
            if self.identities.newest(identity.number).is_some() {
                return Err(identity_problem(What::Duplicate));
            }
            self.identities.put(identity.number, value);
        }
        Ok(())
    }
}
fn identity_problem(what: jig_protocol_config::What) -> jig_protocol_config::Problems {
    let mut problems = jig_protocol_config::Problems::empty();
    problems.push(jig_protocol_config::Path::root().member(b"engine").member(b"identities"), what);
    problems
}
