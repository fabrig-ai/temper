//! The inbox owns its reads; watched documents stay in the client (ruling 235).
use super::Request;
use crate::{
    Limits,
    address::{Filter, Kind},
    owner::Owner,
};
use jig_client::{
    Client, Handle, Latest, Readable, Readout, Told, Watchable,
    contract::{self, Delivery, InboxGroup, InboxReadGroup, ObjectKey, Read, ReadResult, ReadValue},
};
use skein_lib::{List, Queue};
#[derive(Clone, Debug)]
pub struct Card {
    pub object: ObjectKey,
    pub group: InboxGroup,
    pub project: Option<u32>,
    pub position: Option<u64>,
    pub unread: Option<contract::Count>,
    pub handle: Handle,
    pub page: Option<ReadValue>,
    pub words: Option<contract::Text>,
    pub key: Option<jig_client::Key>,
    pub outcome: Option<jig_client::Outcome>,
}
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "refresh bookkeeping is private to the page")]
pub struct Inbox {
    pub filter: Filter,
    pub entries: Handle,
    pub cards: List<Card>,
    /// Incomplete source documents, for needs and updates respectively.
    pub cut: [bool; 2],
    previous: List<Card>,
    pub queries: [Handle; 2],
    pub next: u64,
    pub window: u32,
}
fn client(out: &mut Queue<Request>, event: jig_client::Event) {
    out.push(Request::Client(event));
}
impl Inbox {
    pub(super) fn new(filter: Filter, first: u64, limits: &Limits, out: &mut Queue<Request>) -> Self {
        let inbox = Self {
            filter,
            cut: [false; 2],
            entries: handle(first),
            queries: [
                handle(first.checked_add(1).expect("inbox handles fit")),
                handle(first.checked_add(2).expect("inbox handles fit")),
            ],
            next: first.checked_add(3).expect("inbox handles fit"),
            window: limits.client.domain.rows.min((limits.client.domain.reads / 2).max(1)),
            cards: List::with_capacity(limits.client.domain.reads),
            previous: List::with_capacity(limits.client.domain.reads),
        };
        client(
            out,
            jig_client::Event::Watch {
                handle: inbox.entries,
                subject: Watchable::Jig(contract::Subject::Inbox { window: inbox.window }),
            },
        );
        inbox
    }
    pub(super) fn leave(&self, out: &mut Queue<Request>) -> u64 {
        for handle in [self.entries, self.queries[0], self.queries[1]] {
            client(out, jig_client::Event::Unwatch { handle });
        }
        for card in &self.cards {
            client(out, jig_client::Event::Unwatch { handle: card.handle });
        }
        self.next
    }
    #[expect(clippy::manual_map, reason = "step code uses an explicit optional kind")]
    fn refresh(&mut self, source: &Client, out: &mut Queue<Request>) {
        for card in &self.cards {
            client(out, jig_client::Event::Unwatch { handle: card.handle });
        }
        self.cut = [false; 2];
        self.previous.clear();
        core::mem::swap(&mut self.cards, &mut self.previous);
        for index in 0..self.previous.len() {
            let old = self.previous.get_mut(index).expect("bounded card");
            if old.key.is_some() {
                self.cards.push(old.clone()).expect("previous cards fit");
            }
            old.page = None;
        }
        if self.filter.goal.is_some() {
            for (handle, group) in self.queries.into_iter().zip([InboxReadGroup::Needs, InboxReadGroup::Updates]) {
                if self.filter.kind.is_some() && group == InboxReadGroup::Updates {
                    continue;
                }
                client(
                    out,
                    jig_client::Event::Read {
                        handle,
                        read: Readable::Jig(Read::Inbox {
                            group,
                            project: self.filter.project,
                            kind: match self.filter.kind {
                                Some(value) => Some(kind(value)),
                                None => None,
                            },
                            goal: self.filter.goal,
                            after: None,
                        }),
                    },
                );
            }
            return;
        }
        let latest = match source.document(self.entries) {
            Some(document) => document.latest,
            None => None,
        };
        if let Some(Latest::Jig(Delivery::Inbox(inbox))) = latest {
            self.cut = [inbox.more; 2];
            for row in &inbox.entries {
                self.add(
                    row.object,
                    row.group,
                    Some(row.project),
                    Some(row.position),
                    unread(row.reply.as_deref()),
                    out,
                );
            }
        } else if let Some(Latest::Jig(Delivery::OwnerInbox(inbox))) = latest {
            self.cut = [inbox.next.is_some(); 2];
            for rows in [&inbox.needs, &inbox.updates] {
                for row in rows {
                    self.add(
                        row.card_key(),
                        row.group(),
                        row.project,
                        position(row.object),
                        unread(row.reply.as_deref()),
                        out,
                    );
                }
            }
        }
    }
    fn add(
        &mut self,
        object: ObjectKey,
        group: InboxGroup,
        project: Option<u32>,
        position: Option<u64>,
        unread: Option<contract::Count>,
        out: &mut Queue<Request>,
    ) {
        if self.cards.len() == self.cards.capacity() {
            return;
        }
        if let Some(filter) = self.filter.project
            && Some(filter) != project
        {
            return;
        }
        if let Some(filter) = self.filter.kind
            && kind(filter) != group
        {
            return;
        }
        for card in &self.cards {
            if card.object == object {
                return;
            }
        }
        let read = match object {
            ObjectKey::Task(task) => Read::Task { task },
            ObjectKey::Proposal(proposal) => Read::Proposal { proposal },
            ObjectKey::Escalation(task) => Read::Escalation { task },
            ObjectKey::Question { task, message } => Read::Question { task, message },
            ObjectKey::PersonTask(task) => Read::PersonTask { task },
            ObjectKey::Result(task) => Read::Result { task },
            ObjectKey::Note { .. } => return,
        };
        let handle = Handle { asker: Owner::Pages.asker(), number: self.next };
        self.next = self.next.checked_add(1).expect("inbox handles fit");
        self.cards
            .push(Card {
                object,
                group,
                project,
                position,
                unread,
                handle,
                page: None,
                words: None,
                key: None,
                outcome: None,
            })
            .expect("inbox cards bounded");
        for index in 0..self.previous.len() {
            let old = self.previous.get_mut(index).expect("bounded card");
            if old.object == object {
                let index = self.cards.len().checked_sub(1).expect("new card");
                let card = self.cards.get_mut(index).expect("new card");
                card.words = old.words.take();
                card.key = old.key;
                card.outcome = old.outcome.take();
                break;
            }
        }
        client(out, jig_client::Event::Read { handle, read: Readable::Jig(read) });
    }
    pub(super) fn told(&mut self, source: &Client, told: Told, out: &mut Queue<Request>) -> bool {
        if let Told::Changed { handle } = &told
            && *handle == self.entries
        {
            self.refresh(source, out);
            return true;
        }
        let Told::Page { handle, page: Readout::Jig(ReadResult::Value(value)) } = told else {
            return false;
        };
        if self.queries.contains(&handle) && self.filter.goal.is_some() {
            let index = usize::from(handle == self.queries[1]);
            if let ReadValue::Inbox(page) = value {
                *self.cut.get_mut(index).expect("query names one section") = page.cut;
                for row in &page.rows {
                    self.add(
                        row.object,
                        row.group,
                        Some(row.project),
                        Some(row.position),
                        unread(row.reply.as_deref()),
                        out,
                    );
                }
            } else if let ReadValue::OwnerInbox(page) = value {
                *self.cut.get_mut(index).expect("query names one section") = page.cut;
                for row in &page.rows {
                    self.add(
                        row.card_key(),
                        row.group(),
                        row.project,
                        position(row.object),
                        unread(row.reply.as_deref()),
                        out,
                    );
                }
            }
        } else {
            for index in 0..self.cards.len() {
                let card = self.cards.get_mut(index).expect("bounded card index");
                if card.handle == handle {
                    card.page = Some(value);
                    return true;
                }
            }
            return false;
        }
        true
    }
    #[must_use]
    pub fn results_through(&self) -> Option<u64> {
        let mut through = None;
        for card in &self.cards {
            if card.group == InboxGroup::Results {
                through = through.max(card.position);
            }
        }
        through
    }
}
#[must_use]
pub const fn kind(kind: Kind) -> InboxGroup {
    match kind {
        Kind::Proposal => InboxGroup::Proposals,
        Kind::Question => InboxGroup::Questions,
        Kind::Held => InboxGroup::Escalations,
        Kind::PersonTask => InboxGroup::PersonTasks,
    }
}
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    limits.client.domain.reads.checked_mul(2).expect("pages output fits").checked_add(8).expect("pages output fits")
}

fn handle(number: u64) -> Handle {
    Handle { asker: Owner::Pages.asker(), number }
}
fn position(object: contract::OwnerInboxObject) -> Option<u64> {
    if let contract::OwnerInboxObject::Result { position } = object { Some(position) } else { None }
}

#[expect(clippy::manual_map, reason = "step code spells out optional reply facts")]
fn unread(reply: Option<&contract::TaskLine>) -> Option<contract::Count> {
    match reply {
        Some(line) => match &line.conversation {
            Some(conversation) => Some(conversation.unread),
            None => None,
        },
        None => None,
    }
}
