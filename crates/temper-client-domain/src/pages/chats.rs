//! Live chats are borrowed from the client; closed rows are one bounded read page.
use super::Request;
use crate::address::ChatsFilter;
use crate::{Address, Limits, owner::Owner};
use alloc::boxed::Box;
use jig_client::contract::{
    self as c, Delivery, EndedGroup, Me, Read, ReadResult, ReadValue, TaskPage, TaskQuery, Text,
};
use jig_client::{Client, Handle, Key, Latest, Readable, Readout, Told, Watchable};
use skein_lib::{Env, Queue};

#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the configured query is private watch bookkeeping")]
pub struct Chats {
    pub filter: ChatsFilter,
    pub live: Handle,
    pub closed: Handle,
    pub words: Box<[u8]>,
    pub start: Option<Key>,
    pub page: Option<TaskPage>,
    pub refusal: Option<jig_client::shapes::NotOffered>,
    pub outcome: Option<jig_client::Outcome>,
    query: Option<TaskQuery>,
    first: bool,
}
impl Chats {
    pub(crate) fn new(first: u64) -> Self {
        Self {
            filter: ChatsFilter::All,
            live: handle(first),
            closed: handle(first.checked_add(1).expect("page handles fit")),
            words: Box::default(),
            start: None,
            page: None,
            refusal: None,
            outcome: None,
            query: None,
            first: true,
        }
    }
    pub(crate) fn leave(&self, out: &mut Queue<Request>) -> u64 {
        for handle in [self.live, self.closed] {
            out.push(Request::Client(jig_client::Event::Unwatch { handle }));
        }
        self.closed.number.checked_add(1).expect("page handles fit")
    }
    pub(crate) fn configure(
        &mut self,
        me: Option<&Me>,
        project: Option<u32>,
        limits: &Limits,
        out: &mut Queue<Request>,
    ) {
        let (Some(me), Some(project)) = (me, project) else { return };
        let Some(template) = jig_client::shapes::template(me, project, jig_client::shapes::CHAT) else { return };
        let query = TaskQuery {
            project: Some(project),
            requester: Some(c::OwnerRequester::Person(me.party.number)),
            root: None,
            parent: None,
            label: Some(template.label),
            phase: None,
            executor: None,
            tracked: None,
            order: c::QueryOrder::Activity,
        };
        if self.query == Some(query) {
            return;
        }
        if self.query.is_some() {
            self.leave(out);
        }
        self.page = None;
        self.first = true;
        self.query = Some(query);
        out.push(Request::Client(jig_client::Event::Watch {
            handle: self.live,
            subject: Watchable::Jig(c::Subject::Tasks { query, window: limits.client.domain.rows }),
        }));
        self.read(None, out);
    }
    fn read(&self, ended: Option<c::Cursor>, out: &mut Queue<Request>) {
        if let Some(query) = self.query {
            out.push(Request::Client(jig_client::Event::Read {
                handle: self.closed,
                read: Readable::Jig(Read::Tasks {
                    query: TaskQuery { phase: Some(c::PhaseClass::Ended), order: c::QueryOrder::Ended, ..query },
                    live: None,
                    ended,
                }),
            }));
        }
    }
    pub(crate) fn older(&mut self, out: &mut Queue<Request>) {
        if let Some(page) = &self.page
            && let EndedGroup::Rows { after: Some(after), .. } = &page.ended
        {
            self.first = false;
            self.read(Some(after.clone()), out);
        }
    }
    pub(crate) fn write(&mut self, words: Box<[u8]>, _limits: &Limits) {
        if words.len() <= usize::try_from(temper_client_protocol_wire::CUE_BYTES).expect("text bound fits") {
            self.words = words;
        }
    }
    pub(crate) fn start(
        &mut self,
        env: &Env<Limits>,
        me: Option<&Me>,
        project: Option<u32>,
        choice: &super::choice::Draft,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        self.send(env, me, project, self.words.clone(), choice, serial, out);
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "suggested words share the composer's identity, choice and send state"
    )]
    pub(crate) fn suggest(
        &mut self,
        env: &Env<Limits>,
        me: Option<&Me>,
        project: Option<u32>,
        words: &[u8],
        choice: &super::choice::Draft,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        self.send(env, me, project, words.into(), choice, serial, out);
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "both entry paths supply their words and borrow the same bounded draft"
    )]
    fn send(
        &mut self,
        _env: &Env<Limits>,
        me: Option<&Me>,
        project: Option<u32>,
        words: Box<[u8]>,
        choice: &super::choice::Draft,
        serial: &mut u64,
        out: &mut Queue<Request>,
    ) {
        if self.start.is_some() || words.is_empty() {
            return;
        }
        let (Some(me), Some(project)) = (me, project) else { return };
        if !choice.valid(me) {
            return;
        }
        let Ok(words) = Text::new(words, temper_client_protocol_wire::CUE_BYTES) else { return };
        match jig_client::shapes::chat(me, project, words) {
            Ok(mut ask) => {
                if let c::Ask::Create { tasks, .. } = &mut ask
                    && let Some(task) = tasks.get_mut(0)
                    && choice.context.is_some()
                {
                    task.llm.clone_from(&choice.requested);
                }
                *serial = serial.checked_add(1).expect("page keys fit");
                let key = Key { asker: Owner::Pages.asker(), number: *serial };
                self.start = Some(key);
                self.refusal = None;
                self.outcome = None;
                out.push(Request::Client(jig_client::Event::Send {
                    key,
                    name: None,
                    request: jig_client::Ask::Jig(Box::new(ask)),
                }));
            }
            Err(why) => self.refusal = Some(why),
        }
    }
    pub(crate) fn told(&mut self, told: Told, out: &mut Queue<Request>) -> bool {
        match told {
            Told::Changed { handle } if handle == self.live => {
                // Live deliveries omit ended tasks. Refresh the first ended page
                // alongside them, including when its previous read is in flight.
                if self.first {
                    self.read(None, out);
                }
                true
            }
            Told::Page { handle, page: Readout::Jig(ReadResult::Value(ReadValue::Tasks(page))) }
                if handle == self.closed =>
            {
                self.page = Some(*page);
                true
            }
            Told::Pending { key } if Some(key) == self.start => true,
            Told::Answered { key, outcome } if Some(key) == self.start => {
                if let jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(value))) = &outcome {
                    if let c::Outcome::Owner(c::OwnerOutcome::Created { first, .. }) = value {
                        out.push(Request::Show { address: Address::Chat { task: *first } });
                    } else if let c::Outcome::Made { what: c::Made::Tasks { tasks } } = value
                        && let Some(task) = tasks.get(0)
                    {
                        out.push(Request::Show { address: Address::Chat { task: *task } });
                    }
                }
                self.outcome = Some(outcome);
                self.start = None;
                true
            }
            Told::Link { .. }
            | Told::Pending { .. }
            | Told::Answered { .. }
            | Told::Kept { .. }
            | Told::Dropped { .. }
            | Told::Changed { .. }
            | Told::Text { .. }
            | Told::Ended { .. }
            | Told::Page { .. } => false,
        }
    }
    #[must_use]
    pub fn live<'a>(&self, client: &'a Client) -> Option<&'a c::Tasks> {
        if let Some(Latest::Jig(Delivery::Tasks(tasks))) = client.document(self.live)?.latest {
            Some(tasks)
        } else {
            None
        }
    }
}

fn handle(number: u64) -> Handle {
    Handle { asker: Owner::Pages.asker(), number }
}
