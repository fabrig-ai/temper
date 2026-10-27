//! temper's pages own their handles and navigation (client/README.md, 3.1).
use crate::{Address, Limits, owner::Owner};
use crate::{ChatsFilter, PlanFilter};
use alloc::boxed::Box;
use jig_client::{
    Client, Handle, Latest, Link, Told, Watchable,
    contract::{Delivery, Me, Subject},
};
use skein_lib::{Env, Queue, Time};
pub mod accounts;
pub mod chat;
pub mod chats;
pub mod choice;
pub mod connect_model;
pub mod delegation;
pub mod inbox;
pub mod task;
pub mod welcome;
#[derive(Debug)]
pub struct Pages {
    address: Address,
    project: Option<u32>,
    watching: bool,
    shown: u64,
    pub(crate) inbox: Option<inbox::Inbox>,
    chats: Option<chats::Chats>,
    chat: Option<chat::Chat>,
    task: Option<task::Task>,
    accounts: accounts::Accounts,
    connect_model: Option<Box<connect_model::ConnectModel>>,
    pub(crate) demo_provider: Option<u32>,
    pub(crate) retired_project: Option<u32>,
    pub(crate) demo_model: bool,
    pub(crate) choice: choice::Draft,
    accounts_open: bool,
    welcome: Option<welcome::Welcome>,
    party: Option<u64>,
    next: u64,
    pub(crate) serial: u64,
    seen: u64,
    seeing: Option<(u64, jig_client::Key)>,
    pub(crate) confirm: Option<crate::acts::Confirm>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    OpenProject {
        project: u32,
    },
    Project {
        project: u32,
    },
    Choice(choice::Act),
    SignOut,
    Playground,
    RetryDemo,
    Accounts(accounts::Act),
    ConnectModel(connect_model::Act),
    ChatWords {
        words: Box<[u8]>,
    },
    StartChat,
    Suggest {
        cue: u8,
    },
    MessageWords {
        words: Box<[u8]>,
    },
    Send,
    /// Open the task page's amendment editor (client/README.md, 5.11).
    Amend,
    /// Edit the amendment's specification (client/README.md, 5.11).
    AmendWords {
        words: Box<[u8]>,
    },
    /// Toggle the goal's tracking (client/README.md, 5.11).
    AmendTracked,
    /// Edit the goal's priority (client/README.md, 5.11).
    AmendPriority {
        priority: u32,
    },
    /// Drop the open amendment (client/README.md, 5.11).
    DiscardAmendment,
    /// Send the drafted amendment (client/README.md, 5.11).
    SendAmendment,
    /// Reread the task after a moved refusal (client/README.md, 5.11).
    AmendAgain,
    Stop,
    /// Close the page's task: a chat's close, or a task page's cancel (client/README.md, 5.11).
    Close,
    DelegateFate {
        task: u64,
        fate: jig_client::shapes::Fate,
    },
    Older,
    ClosedChats,
    ChatFilter {
        filter: ChatsFilter,
    },
    PlanFilter {
        filter: PlanFilter,
    },
    Filter {
        filter: crate::address::Filter,
    },
    OpenResult {
        task: u64,
        through: u64,
    },
    Decide {
        object: jig_client::contract::ObjectKey,
        revision: Option<jig_client::contract::Revision>,
        act: jig_client::contract::ActKind,
    },
    CardWords {
        object: jig_client::contract::ObjectKey,
        words: Box<[u8]>,
    },
    Again {
        again: bool,
    },
    Confirm,
    Cancel,
    Go {
        address: Address,
    },
}
#[derive(Debug)]
pub enum Event {
    Started,
    Left,
    AccountKey { value: skein_lib::Secret },
    SignInAddress { value: skein_lib::Secret },
    Went { address: Address },
    Act { act: Act },
    Told { told: Told },
    Link { link: Link },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Client(jig_client::Event),
    Show { address: Address },
    Replace { address: Address },
}
impl Default for Pages {
    fn default() -> Self {
        Self::new()
    }
}
impl Pages {
    #[must_use]
    pub fn new() -> Self {
        Self {
            address: Address::Inbox { filter: crate::address::Filter::ALL },
            project: None,
            watching: false,
            shown: 0,
            inbox: None,
            chats: None,
            chat: None,
            task: None,
            accounts: accounts::Accounts::default(),
            connect_model: None,
            demo_provider: None,
            retired_project: None,
            demo_model: false,
            choice: choice::Draft::default(),
            accounts_open: false,
            welcome: None,
            party: None,
            next: 1,
            serial: 0,
            seen: 0,
            seeing: None,
            confirm: None,
        }
    }
    #[must_use]
    pub fn choice(&self) -> &choice::Draft {
        &self.choice
    }
    #[must_use]
    pub fn accounts(&self) -> Option<&accounts::Accounts> {
        if self.accounts_open && self.party.is_some() { Some(&self.accounts) } else { None }
    }
    #[must_use]
    pub fn connect_model(&self) -> Option<&connect_model::ConnectModel> {
        if self.party.is_some() { self.connect_model.as_deref() } else { None }
    }
    #[must_use]
    pub const fn welcome(&self) -> Option<&welcome::Welcome> {
        self.welcome.as_ref()
    }
    pub(crate) fn decide_welcome(&mut self) {
        if let Some(welcome) = &mut self.welcome {
            welcome.decided = true;
            self.shown = self.shown.checked_add(1).expect("pages shown fits");
        }
    }
    #[must_use]
    pub fn account_name(&self) -> skein_lib::RequestKey {
        let request = match &self.connect_model {
            Some(page) => &page.request,
            None => &self.accounts.request,
        };
        if let Some(pending) = &request.pending
            && (pending.outcome.is_none()
                || pending.outcome == Some(jig_client::Outcome::Unsent)
                || pending.outcome == Some(jig_client::Outcome::Lost))
            && let Some(name) = pending.name
        {
            return name;
        }
        let mut name = [0; 16];
        for (slot, byte) in name.iter_mut().zip(*b"accounts") {
            *slot = byte;
        }
        for (slot, byte) in name
            .get_mut(8..)
            .expect("name number fits")
            .iter_mut()
            .zip(self.serial.checked_add(1).expect("page keys fit").to_be_bytes())
        {
            *slot = byte;
        }

        name
    }
    #[must_use]
    pub fn accounts_document<'a>(&self, client: &'a Client) -> Option<&'a jig_client::contract::OwnerAccounts> {
        if !self.watching || self.party != Some(accounts::current_me(client)?.party.number) {
            return None;
        }
        accounts::document(client)
    }
    #[must_use]
    pub fn availability(&self, client: &Client) -> Option<bool> {
        Some(self.accounts_document(client)?.usable)
    }
    #[must_use]
    pub const fn confirmation(&self) -> Option<&crate::acts::Confirm> {
        self.confirm.as_ref()
    }
    #[must_use]
    pub const fn inbox(&self) -> Option<&inbox::Inbox> {
        self.inbox.as_ref()
    }
    #[must_use]
    pub const fn chats(&self) -> Option<&chats::Chats> {
        self.chats.as_ref()
    }
    #[must_use]
    pub const fn chat(&self) -> Option<&chat::Chat> {
        self.chat.as_ref()
    }
    #[must_use]
    pub const fn task(&self) -> Option<&task::Task> {
        self.task.as_ref()
    }
    pub(crate) fn task_mut(&mut self) -> Option<&mut task::Task> {
        self.task.as_mut()
    }
    #[must_use]
    pub fn cards(&self) -> Option<&skein_lib::List<inbox::Card>> {
        if let Some(task) = &self.task {
            return Some(&task.cards);
        }
        if let Some(chat) = &self.chat { Some(&chat.cards) } else { Some(&self.inbox.as_ref()?.cards) }
    }
    pub(crate) fn cards_mut(&mut self) -> Option<&mut skein_lib::List<inbox::Card>> {
        if let Some(task) = &mut self.task {
            return Some(&mut task.cards);
        }
        if let Some(chat) = &mut self.chat { Some(&mut chat.cards) } else { Some(&mut self.inbox.as_mut()?.cards) }
    }
    pub(crate) fn suggest(&mut self, env: &Env<Limits>, client: &Client, words: &[u8], out: &mut Queue<Request>) {
        let me = self.me(client);
        if let Some(chats) = &mut self.chats {
            chats.suggest(env, me, self.project, words, &self.choice, &mut self.serial, out);
            self.shown = self.shown.checked_add(1).expect("pages shown fits");
        }
    }
    pub(crate) fn seeded(seed: u64) -> Self {
        Self { serial: skein_lib::Rng::new(seed).next_u64(), ..Self::new() }
    }
    #[must_use]
    pub const fn project(&self) -> Option<u32> {
        self.project
    }
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    #[must_use]
    pub fn me<'a>(&self, client: &'a Client) -> Option<&'a Me> {
        match client.document(me_handle())?.latest? {
            Latest::Jig(Delivery::Me(me)) => Some(me),
            Latest::Jig(_) | Latest::Family(_) => None,
        }
    }
}
#[must_use]
pub fn me_handle() -> Handle {
    Handle { asker: Owner::Pages.asker(), number: 0 }
}
#[expect(clippy::too_many_lines, reason = "one ordered dispatch routes page events and refreshes shared facts")]
pub fn step(pages: &mut Pages, env: &Env<Limits>, client: &Client, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Started => started(pages, out),
        Event::Went { address } => went(pages, env, address, out),
        Event::SignInAddress { value } => {
            if let Some(me) = accounts::current_me(client) {
                if let Some(page) = &mut pages.connect_model {
                    page.request.paste_value(value, me.redirect_bytes);
                } else if pages.accounts_open {
                    pages.accounts.request.paste_value(value, me.redirect_bytes);
                }
            }
        }
        Event::AccountKey { value } => {
            if let Some(page) = &mut pages.connect_model {
                page.request.key(value);
            } else if pages.accounts_open
                && accounts::current_me(client).is_some()
                && pages.accounts.key_for.is_some()
                && !pages.accounts.request.busy(client)
            {
                pages.accounts.request.key(value);
            }
        }
        Event::Act { act: Act::ConnectModel(act) } => {
            let chosen = connect_model::way(pages, client);
            if pages.address == (Address::Welcome { step: crate::Step::Ready })
                && let connect_model::Act::SignIn { .. } = &act
                && pages.connect_model.is_none()
            {
                pages.connect_model = Some(Box::new(connect_model::ConnectModel::default()));
            }
            if let Some(page) = &mut pages.connect_model {
                page.playground |= pages.demo_model;
                page.act(client, &pages.address, chosen, pages.demo_provider, act, &mut pages.serial, out);
            }
        }
        Event::Act { act: Act::Accounts(act) } => {
            if pages.accounts_open {
                pages.accounts.act(client, act, &mut pages.serial, out);
            }
        }
        Event::Left => {
            pages.accounts.leave(out);
            if let Some(mut page) = pages.connect_model.take() {
                page.leave();
            }
            pages.welcome = None;
            pages.accounts_open = false;
        }
        Event::Act { act: Act::Choice(act) } => choice::act(pages, client, act, out),
        Event::Act { act: Act::ChatWords { words } } => {
            if let Some(chats) = &mut pages.chats {
                chats.write(words, &env.limits);
            }
        }
        Event::Act { act: Act::StartChat } => start_chat(pages, env, client, out),
        Event::Act { act: Act::Suggest { .. } | Act::Playground | Act::RetryDemo } => {
            unreachable!("suggestions are resolved by the root")
        }
        Event::Act { act: Act::ClosedChats } => closed_chats(pages, out),
        Event::Act { act: Act::ChatFilter { filter } } => chat_filter(pages, filter, out),
        Event::Act { act: Act::PlanFilter { filter } } => plan_filter(pages, filter, out),
        Event::Act { act: Act::Filter { filter } } => out.push(Request::Show { address: Address::Inbox { filter } }),
        Event::Act { act: act @ (Act::MessageWords { .. } | Act::Send | Act::Older) } => {
            if let Some(chat) = &mut pages.chat {
                chat.act(env, client, act, &mut pages.serial, out);
            } else if let Some(task) = &mut pages.task {
                task.act(env, client, act, &mut pages.serial, out);
            }
        }
        Event::Act {
            act:
                act @ (Act::Amend
                | Act::AmendWords { .. }
                | Act::AmendTracked
                | Act::AmendPriority { .. }
                | Act::DiscardAmendment
                | Act::SendAmendment
                | Act::AmendAgain),
        } => {
            if pages.confirm.is_none()
                && let Some(task) = &mut pages.task
            {
                task.amend(env, client, act, &mut pages.serial, out);
            }
        }
        Event::Act { act: Act::OpenResult { task, through } } => {
            open_result(pages, task, through, out);
        }
        Event::Act {
            act:
                act @ (Act::Close
                | Act::Stop
                | Act::DelegateFate { .. }
                | Act::Decide { .. }
                | Act::CardWords { .. }
                | Act::Again { .. }
                | Act::Confirm
                | Act::Cancel),
        } => confirmation(pages, env, client, act, out),
        Event::Act { act: Act::OpenProject { project } } => {
            choose_project(pages, client, project);
            out.push(Request::Show { address: Address::Chats { filter: ChatsFilter::All, start: false } });
        }
        Event::Act { act: Act::Project { project } } => choose_project(pages, client, project),
        Event::Act { act: Act::SignOut } => out.push(Request::Client(jig_client::Event::SignOut)),
        Event::Act { act: Act::Go { address } } => out.push(Request::Show { address }),
        Event::Told { told: Told::Changed { handle } } if handle == me_handle() => me_changed(pages, client, out),
        Event::Link { link } => {
            if let Some(task) = &mut pages.task {
                // Successful sign-in can tell SignedOut then Starting in one step.
                // The installed credential, rather than that intermediate tell,
                // decides whether a cancelled read may be asked again.
                task.link(client.link().state);
            }
            link_changed(pages, link, out);
        }
        Event::Told { told: event } => {
            if !told(pages, env, client, event, out) {
                return;
            }
        }
    }
    returned(pages, client, out);
    let owner = pages.accounts_open
        && match accounts::current_me(client) {
            Some(me) => me.deployment_role == Some(jig_client::contract::HeldRole::Owner),
            None => false,
        };
    pages.accounts.configure(owner, &mut pages.next, out);
    let me = pages.me(client);
    if let Some(chats) = &mut pages.chats {
        chats.configure(me, pages.project, &env.limits, out);
    }
    choice::refresh(pages, client);
    redirect_chat(pages, client, out);
    pages.shown = pages.shown.checked_add(1).expect("pages shown count fits");
}
fn told(pages: &mut Pages, env: &Env<Limits>, client: &Client, told: Told, out: &mut Queue<Request>) -> bool {
    if let Told::Answered { key, outcome } = &told
        && let Some((through, pending)) = pages.seeing
        && *key == pending
    {
        if seen_answered(outcome) {
            pages.seen = through;
        }
        pages.seeing = None;
        return false;
    }
    if pages.choice.told(&told) || crate::acts::told(pages, &told) {
        pages.shown = pages.shown.checked_add(1).expect("pages shown fits");
        return false;
    }
    page_told(pages, env, client, told, out)
}

fn started(pages: &mut Pages, out: &mut Queue<Request>) {
    pages.accounts.leave(out);
    if let Some(mut page) = pages.connect_model.take() {
        page.leave();
    }
    pages.accounts_open = false;
    pages.welcome = None;
    pages.address = Address::Inbox { filter: crate::address::Filter::ALL };
    pages.project = None;
    if !pages.watching {
        pages.watching = true;
        out.push(Request::Client(jig_client::Event::Watch {
            handle: me_handle(),
            subject: Watchable::Jig(Subject::Me),
        }));
        out.push(Request::Client(jig_client::Event::Watch {
            handle: accounts::handle(),
            subject: Watchable::Jig(Subject::Accounts),
        }));
    }
}
pub fn fire(pages: &mut Pages, env: &Env<Limits>, _client: &Client, out: &mut Queue<Request>) {
    if let Some(task) = &mut pages.task {
        task.fire(env.now, out);
    }
}
#[must_use]
pub const fn next_deadline(pages: &Pages) -> Option<Time> {
    match &pages.task {
        Some(task) => task.next_deadline(),
        None => None,
    }
}
#[must_use]
pub const fn shown(pages: &Pages) -> u64 {
    pages.shown
}

fn closed_chats(pages: &mut Pages, out: &mut Queue<Request>) {
    if let Some(chats) = &mut pages.chats {
        chats.older(out);
    }
}
fn chat_filter(pages: &Pages, filter: ChatsFilter, out: &mut Queue<Request>) {
    if pages.chats.is_some() {
        out.push(Request::Show { address: Address::Chats { filter, start: false } });
    }
}

fn plan_filter(pages: &Pages, filter: PlanFilter, out: &mut Queue<Request>) {
    if let Some(task) = &pages.task {
        out.push(Request::Show { address: Address::Task { task: task.number, filter } });
    }
}

fn contains(me: &Me, project: Option<u32>) -> bool {
    for row in &me.projects {
        if Some(row.project) == project {
            return true;
        }
    }
    false
}

fn went(pages: &mut Pages, env: &Env<Limits>, address: Address, out: &mut Queue<Request>) {
    if let (Address::Chats { .. }, Address::Chats { filter, .. }, Some(page)) =
        (&pages.address, &address, &mut pages.chats)
    {
        page.filter = *filter;
        pages.address = address;
        return;
    }
    if let (Address::Task { task: current, .. }, Address::Task { task, filter }, Some(page)) =
        (&pages.address, &address, &mut pages.task)
        && current == task
    {
        page.filter = *filter;
        pages.address = address;
        return;
    }
    if (!pages.accounts_open
        && pages.inbox.is_none()
        && pages.chats.is_none()
        && pages.chat.is_none()
        && pages.task.is_none()
        && pages.welcome.is_none())
        || pages.address != address
    {
        if let Some(page) = &mut pages.connect_model {
            page.changing_way(&pages.address, &address);
        }
        pages.accounts.leave(out);
        pages.accounts_open = address == Address::Accounts;
        if connect_model::open(&address) {
            if pages.connect_model.is_none() {
                pages.connect_model = Some(Box::new(connect_model::ConnectModel::default()));
            }
            if let Address::Welcome { .. } = address {
                let page = pages.connect_model.as_mut().expect("open flow has state");
                if page.holder != jig_client::contract::AccountHolder::Own {
                    page.leave();
                    **page = connect_model::ConnectModel::default();
                }
            }
        } else if let Some(mut page) = pages.connect_model.take() {
            page.leave();
        }
        pages.welcome = None;
        if let Address::Welcome { .. } = address {
            pages.welcome = Some(welcome::Welcome::new());
        }
        if let Some(inbox) = pages.inbox.take() {
            pages.next = inbox.leave(out);
        }
        if let Some(chats) = pages.chats.take() {
            pages.next = chats.leave(out);
        }
        if let Some(chat) = pages.chat.take() {
            pages.next = chat.leave(out);
        }
        if let Some(task) = pages.task.take() {
            pages.next = task.leave(out);
        }
        if let Address::Task { task, filter } = address {
            let mut page = task::Task::new(task, pages.next, &env.limits, out);
            page.filter = filter;
            pages.task = Some(page);
        }
        if let Address::Chat { task } = address {
            pages.chat = Some(chat::Chat::new(task, pages.next, &env.limits, out));
        }
        if let Address::Chats { filter, .. } = address {
            let mut page = chats::Chats::new(pages.next);
            page.filter = filter;
            pages.chats = Some(page);
        }
        if let Address::Inbox { filter } = address {
            pages.inbox = Some(inbox::Inbox::new(filter, pages.next, &env.limits, out));
        }
        pages.choice = choice::Draft::default();
        pages.confirm = None;
    }
    if let Some(page) = &mut pages.connect_model {
        let source = match &address {
            Address::ConnectModel { way: Some(way) }
            | Address::Welcome { step: crate::Step::Model { way: Some(way) } }
                if way.kind == crate::WayKind::SignIn =>
            {
                way.source
            }
            Address::Inbox { .. }
            | Address::Chats { .. }
            | Address::Accounts
            | Address::Forges
            | Address::ConnectForge { .. }
            | Address::ConnectModel { .. }
            | Address::Welcome { .. }
            | Address::Chat { .. }
            | Address::Task { .. }
            | Address::Adopt { .. } => None,
        };
        page.request.return_from(source);
    }
    pages.address = address;
}

fn seen_answered(outcome: &jig_client::Outcome) -> bool {
    match outcome {
        jig_client::Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Done(_))) => true,
        jig_client::Outcome::Answered(
            jig_client::Answered::Jig(jig_client::contract::Answer::Refused { .. }) | jig_client::Answered::Family(_),
        )
        | jig_client::Outcome::NotKept
        | jig_client::Outcome::Unknown
        | jig_client::Outcome::Limit
        | jig_client::Outcome::Full { .. }
        | jig_client::Outcome::Unconfirmed
        | jig_client::Outcome::Held
        | jig_client::Outcome::Unsent
        | jig_client::Outcome::Lost => false,
    }
}

fn open_result(pages: &mut Pages, task: u64, through: u64, out: &mut Queue<Request>) {
    if pages.inbox.is_some() && through > pages.seen && pages.seeing.is_none() {
        pages.serial = pages.serial.checked_add(1).expect("pages keys fit");
        let key = jig_client::Key { asker: Owner::Pages.asker(), number: pages.serial };
        pages.seeing = Some((through, key));
        out.push(Request::Client(jig_client::Event::Send {
            key,
            name: None,
            request: jig_client::Ask::Jig(Box::new(jig_client::contract::Ask::Seen {
                through: jig_client::contract::SeenThrough::Results { through },
            })),
        }));
    }
    out.push(Request::Show { address: Address::Task { task, filter: PlanFilter::All } });
}

fn page_told(pages: &mut Pages, env: &Env<Limits>, client: &Client, told: Told, out: &mut Queue<Request>) -> bool {
    if let Some(page) = &mut pages.connect_model {
        if let Told::Kept { key, request: jig_client::Ask::Jig(ask) } = &told
            && let jig_client::contract::Ask::AccountChange {
                change: jig_client::contract::AccountChange::FinishSignIn { .. },
                ..
            } = ask.as_ref()
            && !pages.accounts.request.paste.has_pending(*key)
        {
            return page.request.told(client, &told);
        }
        let claim = match &told {
            Told::Kept { key, request: jig_client::Ask::Jig(ask) } => {
                let row_request = match &pages.accounts.request.pending {
                    Some(pending) => pending.key == *key,
                    None => false,
                };
                !row_request && connect_model::creation(ask)
            }
            Told::Kept { .. } => false,
            Told::Answered { .. }
            | Told::Pending { .. }
            | Told::Changed { .. }
            | Told::Link { .. }
            | Told::Dropped { .. }
            | Told::Text { .. }
            | Told::Ended { .. }
            | Told::Page { .. } => true,
        };
        if claim && page.request.told(client, &told) {
            // Both requests observe watch changes, but answers belong to their own key.
            if let Told::Changed { .. } = told {
                pages.accounts.request.refresh(client);
            }
            return true;
        }
    }
    if pages.accounts.request.told(client, &told) {
        return true;
    }
    if let Told::Changed { handle } | Told::Ended { handle, .. } = &told
        && (*handle == accounts::handle() || *handle == me_handle() || Some(*handle) == pages.accounts.deployment)
    {
        return true;
    }
    if let Some(task) = &mut pages.task {
        return task.told(env.now, client, told, out);
    }
    if let Some(chat) = &mut pages.chat {
        let moved = match &told {
            Told::Answered { outcome, .. } => seen_answered(outcome),
            Told::Changed { .. } | Told::Text { .. } | Told::Page { .. } => true,
            Told::Link { .. }
            | Told::Pending { .. }
            | Told::Kept { .. }
            | Told::Dropped { .. }
            | Told::Ended { .. } => false,
        };
        if !chat.told(client, told, out) {
            return false;
        }
        // Pages owns the shared key sequence, including automatic conversation reads.
        if moved {
            chat.see(&mut pages.serial, out);
        }
    } else if let Some(chats) = &mut pages.chats {
        if !chats.told(told, out) {
            return false;
        }
    } else if !match &mut pages.inbox {
        Some(inbox) => inbox.told(client, told, out),
        None => false,
    } {
        return false;
    }
    true
}

fn confirmation(pages: &mut Pages, env: &Env<Limits>, client: &Client, act: Act, out: &mut Queue<Request>) {
    if let Act::Confirm = act
        && let Some(page) = &mut pages.task
        && let Some(open) = &pages.confirm
        && open.object == jig_client::contract::ObjectKey::Task(page.number)
        && let jig_client::contract::ActKind::Stop | jig_client::contract::ActKind::Cancel = open.act
    {
        let kind =
            if open.act == jig_client::contract::ActKind::Stop { task::Sending::Stop } else { task::Sending::Cancel };
        pages.confirm = None;
        page.send(env, client, kind, &mut pages.serial, out);
        return;
    }
    let closing = match &pages.confirm {
        Some(open) => open.act == jig_client::contract::ActKind::Cancel,
        None => false,
    };
    match act {
        Act::Stop if pages.chat.is_some() => {
            pages.chat.as_mut().expect("open chat").act(env, client, Act::Stop, &mut pages.serial, out);
        }
        Act::Stop => task_confirmation(pages, client, jig_client::contract::ActKind::Stop),
        Act::Close => {
            if let Some(chat) = &mut pages.chat {
                chat.prepare_close(client);
                if chat.closing.is_some() {
                    pages.confirm = Some(crate::acts::Confirm {
                        object: jig_client::contract::ObjectKey::Task(chat.task),
                        revision: None,
                        act: jig_client::contract::ActKind::Cancel,
                        again: None,
                    });
                }
            } else {
                task_confirmation(pages, client, jig_client::contract::ActKind::Cancel);
            }
        }
        Act::DelegateFate { task, fate } => {
            if closing && let Some(chat) = &mut pages.chat {
                chat.delegate_fate(task, fate);
            }
        }
        Act::Confirm if closing => {
            pages.confirm = None;
            if let Some(chat) = &mut pages.chat {
                chat.close(&mut pages.serial, out);
            }
        }
        Act::Cancel => {
            pages.confirm = None;
            if let Some(chat) = &mut pages.chat {
                chat.closing = None;
            }
        }
        act @ (Act::Decide { .. } | Act::CardWords { .. } | Act::Again { .. } | Act::Confirm) => {
            crate::acts::step(pages, env, act, out);
        }
        Act::Amend
        | Act::AmendWords { .. }
        | Act::AmendTracked
        | Act::AmendPriority { .. }
        | Act::DiscardAmendment
        | Act::SendAmendment
        | Act::AmendAgain
        | Act::Choice(_)
        | Act::OpenProject { .. }
        | Act::Project { .. }
        | Act::Accounts(_)
        | Act::ConnectModel(_)
        | Act::SignOut
        | Act::ChatWords { .. }
        | Act::StartChat
        | Act::Playground
        | Act::RetryDemo
        | Act::Suggest { .. }
        | Act::MessageWords { .. }
        | Act::Send
        | Act::Older
        | Act::ChatFilter { .. }
        | Act::PlanFilter { .. }
        | Act::ClosedChats
        | Act::Filter { .. }
        | Act::OpenResult { .. }
        | Act::Go { .. } => unreachable!("only deciding acts routed here"),
    }
}

fn task_confirmation(pages: &mut Pages, client: &Client, act: jig_client::contract::ActKind) {
    if let Some(page) = &pages.task
        && let Some(task) = page.document(client)
        && chat::allowed(task, act)
    {
        pages.confirm = Some(crate::acts::Confirm {
            object: jig_client::contract::ObjectKey::Task(page.number),
            revision: None,
            act,
            again: None,
        });
    }
}

fn choose_project(pages: &mut Pages, client: &Client, project: u32) {
    if let Some(me) = pages.me(client) {
        for row in &me.projects {
            if row.project == project {
                pages.project = Some(project);
                break;
            }
        }
    }
}

fn redirect_chat(pages: &Pages, client: &Client, out: &mut Queue<Request>) {
    if let Some(task) = &pages.task
        && let Some(address) = task.chat(client, pages.me(client))
    {
        out.push(Request::Show { address });
    }
}

/// Navigation can also close/reopen the two account watches.
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    inbox::max_out(limits).checked_add(6).expect("pages output fits")
}

fn me_changed(pages: &mut Pages, client: &Client, out: &mut Queue<Request>) {
    if let Some(me) = accounts::current_me(client) {
        let parked = if let Some(pending) = &pages.accounts.request.pending
            && let Some(held) = client.pending(pending.key)
        {
            held.party != me.party.number
        } else {
            false
        };
        if (pages.party.is_some() && pages.party != Some(me.party.number)) || parked {
            pages.accounts.leave(out);
            if let Some(mut page) = pages.connect_model.take() {
                page.leave();
            }
            pages.accounts = accounts::Accounts::default();
            out.push(Request::Client(jig_client::Event::Unwatch { handle: accounts::handle() }));
            out.push(Request::Client(jig_client::Event::Watch {
                handle: accounts::handle(),
                subject: Watchable::Jig(Subject::Accounts),
            }));
        }
        if pages.party != Some(me.party.number) {
            pages.seen = 0;
            pages.seeing = None;
            pages.choice = choice::Draft::default();
        }
        pages.party = Some(me.party.number);
        if connect_model::open(&pages.address) {
            if pages.connect_model.is_none() {
                pages.connect_model = Some(Box::new(connect_model::ConnectModel::default()));
            }
            let page = pages.connect_model.as_mut().expect("open flow has state");
            if page.holder == jig_client::contract::AccountHolder::Deployment
                && me.deployment_role != Some(jig_client::contract::HeldRole::Owner)
            {
                page.request.clear_key();
                page.holder = jig_client::contract::AccountHolder::Own;
            }
        }
    }
    project_changed(pages, client);
}

pub(crate) fn project_changed(pages: &mut Pages, client: &Client) {
    if let Some(me) = pages.me(client)
        && (!contains(me, pages.project) || pages.project == pages.retired_project)
    {
        let mut project = None;
        for row in &me.projects {
            if Some(row.project) != pages.retired_project {
                project = Some(row.project);
                break;
            }
        }
        pages.project = project;
    }
}
// jig ends every pending read at a credential boundary and replays none
// (jig's `client/domain/client.md`, 7): a page that has read nothing yet asks
// once more when a successful sign-in has ended its first read.
fn awaits_first_read(thread: &jig_client::thread::Thread) -> bool {
    thread.loaded == 0
        && thread.lines.rows.is_empty()
        && match &thread.reading {
            Some(jig_client::thread::Reading::Latest { known }) => *known == 0,
            Some(jig_client::thread::Reading::Older | jig_client::thread::Reading::Newer) | None => false,
        }
}

fn link_changed(pages: &mut Pages, link: Link, out: &mut Queue<Request>) {
    match link {
        Link::SignedOut | Link::OutOfDate => {
            if pages.watching {
                out.push(Request::Client(jig_client::Event::Unwatch { handle: me_handle() }));
                pages.watching = false;
                out.push(Request::Client(jig_client::Event::Unwatch { handle: accounts::handle() }));
            }
            pages.accounts.leave(out);
            if let Some(mut page) = pages.connect_model.take() {
                page.leave();
            }
            pages.accounts = accounts::Accounts::default();
            pages.choice = choice::Draft::default();
            pages.party = None;
            pages.project = None;
            pages.seen = 0;
            pages.seeing = None;
        }
        Link::Starting | Link::Live | Link::Offline => {
            if !pages.watching {
                pages.watching = true;
                out.push(Request::Client(jig_client::Event::Watch {
                    handle: me_handle(),
                    subject: Watchable::Jig(Subject::Me),
                }));
                out.push(Request::Client(jig_client::Event::Watch {
                    handle: accounts::handle(),
                    subject: Watchable::Jig(Subject::Accounts),
                }));
            }
        }
    }
}

fn start_chat(pages: &mut Pages, env: &Env<Limits>, client: &Client, out: &mut Queue<Request>) {
    let me = pages.me(client);
    if let Some(chats) = &mut pages.chats {
        chats.start(env, me, pages.project, &pages.choice, &mut pages.serial, out);
    }
}

fn returned(pages: &mut Pages, client: &Client, out: &mut Queue<Request>) {
    let Some(page) = &mut pages.connect_model else { return };
    if !page.request.returned()
        || accounts::document(client).is_none()
        || page.request.sign_in_state(client) != accounts::SignInState::Unknown
    {
        return;
    }
    match &mut pages.address {
        Address::ConnectModel { way: Some(way) } | Address::Welcome { step: crate::Step::Model { way: Some(way) } } => {
            way.source = None;
        }
        Address::Inbox { .. }
        | Address::Chats { .. }
        | Address::Accounts
        | Address::Forges
        | Address::ConnectForge { .. }
        | Address::ConnectModel { .. }
        | Address::Welcome { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::Adopt { .. } => return,
    }
    page.request.return_from(None);
    pages.shown = pages.shown.checked_add(1).expect("pages shown fits");
    out.push(Request::Replace { address: pages.address.clone() });
}
