//! Kept names, replay admission and diagnostics (client/tools.md, 7 and 8.2).
use alloc::{boxed::Box, collections::BTreeMap};
use jig_client::{Ask, Client, Held, Key, Record};
use jig_client_protocol_link::{Limits, translate};
use jig_client_protocol_wire as w;
use skein_lib::{List, Reader, RequestRecord};

#[derive(Debug)]
pub(crate) struct Recovery {
    rows: BTreeMap<Key, RequestRecord<Held>>,
    sent: BTreeMap<Key, bool>,
    pub(crate) joined: BTreeMap<Key, Key>,
}
impl Recovery {
    pub(crate) fn new() -> Self {
        Self { rows: BTreeMap::new(), sent: BTreeMap::new(), joined: BTreeMap::new() }
    }
    pub(crate) fn load(&mut self, records: &List<Record>, limits: &Limits) {
        for record in records {
            if let Some(row) = restore(record, limits) {
                self.rows.entry(row.request.key).or_insert(row);
            }
        }
    }
    pub(crate) fn load_rows(&mut self, rows: &List<RequestRecord<Held>>) {
        for row in rows {
            self.rows.entry(row.request.key).or_insert(row.clone());
        }
    }
    pub(crate) fn kept(&mut self, key: Key) {
        if self.rows.contains_key(&key) {
            self.sent.insert(key, false);
        }
    }
    pub(crate) fn answered(&mut self, key: Key) {
        self.rows.remove(&key);
        self.sent.remove(&key);
    }
    pub(crate) fn output(&mut self, request: &jig_client::Request, limits: &Limits) {
        match request {
            jig_client::Request::Save { records } => {
                for record in records {
                    if let Some(row) = restore(record, limits)
                        && self.sent.contains_key(&row.request.key)
                    {
                        self.rows.insert(row.request.key, row);
                    }
                }
            }
            jig_client::Request::Exchange { message: jig_client::Message::Request(bytes), .. } => {
                if let Ok(request) = w::Keyed::decode(&limits.wire, &mut Reader::new(bytes)) {
                    self.sent_name(*request.key());
                }
            }
            jig_client::Request::Down { down: jig_client::Down::Save { record } } => {
                if self.sent.contains_key(&record.request.key) {
                    self.rows.insert(record.request.key, record.clone());
                }
            }
            jig_client::Request::Down { down: jig_client::Down::Send { key, .. } } => self.sent_name(*key),
            jig_client::Request::Exchange { .. }
            | jig_client::Request::Down { .. }
            | jig_client::Request::OpenStream { .. }
            | jig_client::Request::CloseStream
            | jig_client::Request::Erase { .. }
            | jig_client::Request::Told { .. }
            | jig_client::Request::SignOut
            | jig_client::Request::Authorized(_)
            | jig_client::Request::ServiceCredential { .. } => {}
        }
    }
    fn sent_name(&mut self, name: skein_lib::RequestKey) {
        for (key, row) in &self.rows {
            if row.key == name
                && let Some(sent) = self.sent.get_mut(key)
            {
                *sent = true;
            }
        }
    }
    pub(crate) fn replayed(&self, client: &Client, party: u64) -> bool {
        for (key, sent) in &self.sent {
            if !sent
                && let Some(pending) = client.pending(*key)
                && pending.party == party
            {
                return false;
            }
        }
        true
    }
    pub(crate) fn same(&self, name: skein_lib::RequestKey, request: &Ask, party: u64, client: &Client) -> Option<Key> {
        for (key, row) in &self.rows {
            if self.sent.contains_key(key)
                && row.key == name
                && row.scope == party
                && row.request.request == *request
                && client.pending(*key).is_some()
            {
                return Some(*key);
            }
        }
        None
    }
    pub(crate) fn unanswered(&self, client: &Client, party: u64) -> Box<[u8]> {
        let mut rows = List::with_capacity(u32::try_from(self.sent.len()).expect("bounded requests"));
        for key in self.sent.keys() {
            if let Some(row) = self.rows.get(key)
                && row.scope == party
                && client.pending(*key).is_some()
            {
                rows.push(row.clone()).expect("counted requests");
            }
        }
        crate::jig::render::pending::unanswered(rows.as_slice())
    }
}
/// Decode a native request with the same checks used by the client.
#[must_use]
pub fn restore(entry: &Record, limits: &Limits) -> Option<RequestRecord<Held>> {
    if entry.key.len() != 16
        || u64::try_from(entry.bytes.len()).expect("record length fits") > limits.client.document_bytes
    {
        return None;
    }
    let mut reader = Reader::new(&entry.bytes);
    let kept = w::Kept::decode(&limits.wire, &mut reader).ok()?;
    if reader.remaining() != 0 || kept.store_version() != jig_client_domain::TABLE_VERSION {
        return None;
    }
    let mut reader = Reader::new(kept.record());
    let row = translate::store::restore_table(w::TableEntry::decode(&limits.wire, &mut reader).ok()?, limits).ok()?;
    if reader.remaining() != 0 || row.key.as_slice() != entry.key.as_ref() {
        return None;
    }
    if let Ask::Family { record, .. } = &row.request.request {
        let mut declared = false;
        for owner in limits.client.owners {
            if owner.family == record.family && owner.version == record.version {
                declared = true;
                break;
            }
        }
        if !declared {
            return None;
        }
    }
    Some(row)
}
