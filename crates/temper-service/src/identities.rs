//! The two newest git identity generations by grant number (protocol/credentials.md, 4).
use alloc::boxed::Box;
use skein_lib::{List, Secret, Time};
use temper_worker_protocol_git::Identity;

/// A git identity's decoded value (protocol/channel.md, section 6).
#[derive(PartialEq, Eq, Debug)]
pub enum Value {
    Forgejo(Identity),
    GitHub { login: Box<[u8]>, name: Box<[u8]>, email: Box<[u8]>, entries: List<Entry> },
}
/// One installation account's token; Debug reveals only its length.
#[derive(PartialEq, Eq, Debug)]
pub struct Entry {
    pub account: u64,
    pub token: Secret,
}

#[derive(Debug)]
pub struct IdentityTable {
    identities: List<Generations>,
}
#[derive(Debug)]
struct Generations {
    newest: Option<Grant>,
    older: Option<Grant>,
}
#[derive(Debug)]
struct Grant {
    generation: u64,
    value: Option<Value>,
    expires: Time,
}
/// A grant's number outside the configured table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Unfit;

impl IdentityTable {
    /// Room for numbers `1..=identities`, including undecodable generations.
    #[must_use]
    pub fn new(identities: u32) -> Self {
        let mut slots = List::with_capacity(identities);
        for _ in 0..identities {
            slots.push(Generations { newest: None, older: None }).expect("one slot per configured identity");
        }
        Self { identities: slots }
    }
    /// Keep the two newest generations, replacing a repeated generation.
    /// An undecodable grant takes its place with no value.
    pub fn granted(&mut self, number: u32, generation: u64, value: Option<Value>, expires: Time) -> Result<(), Unfit> {
        let slot = number.checked_sub(1).ok_or(Unfit)?;
        let slots = self.identities.get_mut(slot).ok_or(Unfit)?;
        let entry = Grant { generation, value, expires };
        match &slots.newest {
            Some(newest) if generation < newest.generation => match &slots.older {
                Some(older) if generation < older.generation => (),
                Some(_) | None => slots.older = Some(entry),
            },
            Some(newest) if generation == newest.generation => slots.newest = Some(entry),
            Some(_) | None => {
                slots.older = slots.newest.take();
                slots.newest = Some(entry);
            }
        }
        Ok(())
    }
    /// Only the newest generation can supply a value, while its grant is valid.
    #[must_use]
    pub fn newest(&self, number: u32, now: Time) -> Option<&Value> {
        let slot = number.checked_sub(1)?;
        let entry = self.identities.get(slot)?.newest.as_ref()?;
        // The accounts' table uses the same holder clock and strict expiry edge
        // (jig's protocol/channel.md, 8). Keep the generation after it lapses:
        // neither expiry nor a failed decode can restore an older value.
        if now < entry.expires { entry.value.as_ref() } else { None }
    }
}
/// Fixed slots and at most two decoded values within each grant's byte bound.
#[must_use]
pub fn identities_worst_case(identities: u32, value_bytes: u32) -> Option<u64> {
    List::<Generations>::worst_case(identities)?
        .checked_add(u64::from(identities).checked_mul(2)?.checked_mul(u64::from(value_bytes))?)
}
