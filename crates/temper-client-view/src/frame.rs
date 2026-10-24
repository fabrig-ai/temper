//! temper's banner, with navigation after the projects (client/README.md, 5).
use skein_lib::{List, Token};
use skein_ui::{Binding, Builder, Region, Role, Run};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{Address, Domain, pages::Act};
#[must_use]
pub const fn owner() -> skein_ui::Owner {
    skein_ui::Owner { number: temper_client_domain::owner::Owner::Pages.asker().0 }
}
#[must_use]
pub fn link_region() -> Region {
    Region { owner: owner(), number: 0 }
}
#[derive(Debug)]
pub(crate) struct Bindings {
    entries: List<(Token, Act)>,
    next: u64,
}
impl Bindings {
    pub fn new(capacity: u32) -> Self {
        Self { entries: List::with_capacity(capacity), next: 0 }
    }
    pub fn begin(&mut self) {
        self.entries.clear();
    }
    pub(crate) fn insert(&mut self, act: Act) -> Binding {
        self.next = self.next.checked_add(1).expect("pages binding tokens fit");
        let token = Token::new(self.next);
        self.entries.push((token, act)).expect("pages bindings fit");
        Binding { owner: owner(), token }
    }
    pub fn decode(&self, token: Token, act: skein_ui::Act) -> Option<temper_client_domain::Action> {
        for (key, meaning) in &self.entries {
            if *key == token {
                let meaning = match (meaning, act) {
                    (Act::Filter { filter }, skein_ui::Act::Input { value }) => {
                        let mut filter = *filter;
                        filter.goal = if value.is_empty() { None } else { Some(skein_ui::number(&value)?) };
                        Act::Filter { filter }
                    }
                    (Act::AmendWords { .. }, skein_ui::Act::Input { value }) => Act::AmendWords { words: value },
                    (Act::AmendPriority { .. }, skein_ui::Act::Input { value }) => {
                        Act::AmendPriority { priority: u32::try_from(skein_ui::number(&value)?).ok()? }
                    }
                    (Act::AmendTracked, skein_ui::Act::Toggle) => Act::AmendTracked,
                    (Act::ChatWords { .. }, skein_ui::Act::Input { value }) => Act::ChatWords { words: value },
                    (Act::MessageWords { .. }, skein_ui::Act::Input { value }) => Act::MessageWords { words: value },
                    (Act::CardWords { object, .. }, skein_ui::Act::Input { value }) => {
                        Act::CardWords { object: *object, words: value }
                    }
                    (Act::ChatWords { .. }, skein_ui::Act::Submit) => Act::StartChat,
                    (Act::MessageWords { .. }, skein_ui::Act::Submit) => Act::Send,
                    (meaning, skein_ui::Act::Press) => meaning.clone(),
                    _ => return None,
                };
                return Some(temper_client_domain::Action::Pages(meaning));
            }
        }
        None
    }
    pub fn worst_case(capacity: u32) -> Option<u64> {
        List::<(Token, Act)>::worst_case(capacity)
    }
}
fn text(builder: &mut Builder, role: Role, words: &[u8], style: Option<skein_ui::Style>) {
    builder.open(role);
    if let Some(style) = style {
        builder.style(style);
    }
    builder.name(words);
    builder.text(Run::Plain, words);
    builder.close();
}
fn action(builder: &mut Builder, role: Role, words: &[u8], act: Act, current: bool, bindings: &mut Bindings) {
    if role == Role::Link {
        crate::link(builder, words, bindings.insert(act));
        builder.text(Run::Plain, words);
    } else {
        builder.open(role);
        builder.name(words);
        builder.bind(bindings.insert(act));
    }
    if current {
        builder.state(skein_ui::State::Current);
    }
    builder.close();
}
pub(crate) fn unavailable(root: &Domain) -> Option<crate::words::Word> {
    match root.client().link().state {
        jig_client::Link::SignedOut => Some(crate::words::Word::SignedOut),
        jig_client::Link::OutOfDate => Some(crate::words::Word::OutOfDate),
        jig_client::Link::Starting | jig_client::Link::Live | jig_client::Link::Offline => None,
    }
}
pub(crate) fn unavailable_main(builder: &mut Builder, root: &Domain, words: &crate::words::Words) {
    use crate::words::Word;
    use jig_client::NotSignedIn;
    use skein_ui::phrases::Hole;
    if root.client().link().state == jig_client::Link::SignedOut {
        match root.client().link().not_signed_in {
            Some(NotSignedIn::Full { sign_ins, earliest, link_until }) => words.text(
                builder,
                Word::SignInFull,
                &[Hole::Number(u64::from(sign_ins)), Hole::Wall(earliest), Hole::Wall(link_until)],
            ),
            Some(NotSignedIn::Busy) => words.text(builder, Word::SignInBusy, &[]),
            Some(NotSignedIn::NotReady) => words.text(builder, Word::SignInNotReady, &[]),
            None => words.text(builder, Word::SignedOutMain, &[]),
        }
    } else {
        words.text(builder, Word::OutOfDateMain, &[]);
    }
}
fn project_buttons(builder: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) {
    if let Some(me) = root.pages().me(root.client()) {
        let mut projects = false;
        for selected in [true, false] {
            for row in &me.projects {
                if let Some(demo) = root.forge().retired_demo()
                    && demo.project == row.project
                {
                    continue;
                }
                if (Some(row.project) == root.pages().project()) == selected {
                    let name = crate::pages::project_name(Some(me), root.forge().demo(), row.project, words)
                        .expect("project from me has a name");
                    builder.open(Role::Button);
                    builder.name(&name);
                    if selected {
                        builder.state(skein_ui::State::Pressed);
                    }
                    builder.bind(bindings.insert(Act::Project { project: row.project }));
                    builder.close();
                    projects = true;
                }
            }
        }
        if !projects {
            text(
                builder,
                Role::Paragraph,
                &words.say(crate::words::Word::NoProjectYet, &[]),
                Some(crate::look::PROJECT_EMPTY),
            );
        }
    }
}
pub(crate) fn banner(builder: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) {
    let pages = root.pages();
    let me = pages.me(root.client());
    builder.open(Role::Banner);
    builder.style(crate::look::BANNER);
    text(builder, Role::Paragraph, &words.say(crate::words::Word::Brand, &[]), Some(crate::look::BRAND));
    if let Address::Welcome { .. } = pages.address()
        && unavailable(root).is_none()
    {
        builder.open(Role::Slot);
        builder.style(crate::look::CORNER);
        if let Some(me) = me {
            text(builder, Role::Status, me.party.name.bytes(), None);
            action(builder, Role::Button, &words.say(crate::words::Word::SignOut, &[]), Act::SignOut, false, bindings);
        }
        builder.close();
        builder.close();
        return;
    }
    builder.open(Role::Disclosure);
    builder.name(&words.say(crate::words::Word::Menu, &[]));
    builder.style(crate::look::MENU);
    if let Some(word) = unavailable(root) {
        builder.open(Role::Slot);
        builder.style(crate::look::CORNER);
        builder.open(Role::Region);
        builder.region(link_region());
        text(builder, Role::Status, &words.say(word, &[]), None);
        builder.close();
        builder.close();
        builder.close();
        builder.close();
        return;
    }
    project_buttons(builder, root, words, bindings);
    navigation(builder, root, words, bindings);
    builder.open(Role::Slot);
    builder.style(crate::look::CORNER);
    builder.open(Role::Region);
    builder.region(link_region());
    let (status, style) = match root.client().link().state {
        jig_client::Link::Offline => (crate::words::Word::Offline, crate::look::OFFLINE),
        jig_client::Link::Live if me.is_some() => (crate::words::Word::Live, crate::look::LIVE),
        jig_client::Link::SignedOut
        | jig_client::Link::Starting
        | jig_client::Link::Live
        | jig_client::Link::OutOfDate => (crate::words::Word::Connecting, crate::look::CONNECTING),
    };
    text(builder, Role::Status, &words.say(status, &[]), Some(style));
    let me_behind = match root.client().document(temper_client_domain::pages::me_handle()) {
        Some(document) => document.state == jig_client::State::Behind,
        None => false,
    };
    let page_behind = match pages.inbox() {
        Some(inbox) => match root.client().document(inbox.entries) {
            Some(document) => document.state == jig_client::State::Behind,
            None => false,
        },
        None => false,
    };
    if me_behind || page_behind {
        text(builder, Role::Status, &words.say(crate::words::Word::Behind, &[]), Some(crate::look::BEHIND));
    }
    builder.close();
    if let Some(me) = me {
        text(builder, Role::Status, me.party.name.bytes(), None);
    }
    if root.client().link().sign_out_lost {
        text(builder, Role::Alert, &words.say(crate::words::Word::SignOutLost, &[]), None);
    }
    if me.is_some() || root.client().link().sign_out_lost {
        action(builder, Role::Button, &words.say(crate::words::Word::SignOut, &[]), Act::SignOut, false, bindings);
    }
    builder.close();
    builder.close();
    builder.close();
}
#[expect(clippy::too_many_lines, reason = "one navigation tree keeps page states and account marks together")]
pub(crate) fn navigation(builder: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) {
    if unavailable(root).is_some() {
        return;
    }
    let (inbox_current, adopt_current) = match root.pages().address() {
        Address::Inbox { .. } => (true, false),
        Address::Adopt { .. } => (false, true),
        Address::ConnectForge { .. }
        | Address::Forges
        | Address::Welcome { .. }
        | Address::Chats { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::ConnectModel { .. }
        | Address::Accounts => (false, false),
    };
    let count = match root.pages().me(root.client()) {
        Some(me) => me.needs_total,
        None => 0,
    };
    let name = if count == 0 {
        words.say(crate::words::Word::InboxTitle, &[])
    } else {
        words.say(crate::words::Word::Inbox, &[skein_ui::phrases::Hole::Number(u64::from(count))])
    };
    builder.open(Role::Navigation);
    builder.name(&words.say(crate::words::Word::MainNavigation, &[]));
    builder.style(crate::look::NAVIGATION);
    crate::link(
        builder,
        &name,
        bindings.insert(Act::Go { address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL } }),
    );
    if inbox_current {
        builder.state(skein_ui::State::Current);
    }
    if count == 0 {
        builder.text(Run::Plain, &name);
    } else {
        let number = skein_lib::Decimal::of(u64::from(count));
        let prefix = name.len().checked_sub(number.as_bytes().len()).expect("count is the phrase suffix");
        builder.text(Run::Plain, name.get(..prefix).expect("prefix fits"));
        builder.open(Role::Text);
        builder.style(crate::look::NAV_COUNT);
        builder.run(Run::Plain, number.as_bytes());
        builder.close();
    }
    builder.close();
    action(
        builder,
        Role::Link,
        &words.say(crate::words::Word::Chats, &[]),
        Act::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        match root.pages().address() {
            Address::Chats { .. } | Address::Chat { .. } => true,
            Address::ConnectForge { .. }
            | Address::Forges
            | Address::Welcome { .. }
            | Address::Inbox { .. }
            | Address::Task { .. }
            | Address::Adopt { .. }
            | Address::ConnectModel { .. }
            | Address::Accounts => false,
        },
        bindings,
    );
    if let Some(project) = root.pages().project() {
        action(
            builder,
            Role::Link,
            &words.say(crate::words::Word::Adoption, &[]),
            Act::Go { address: Address::Adopt { project } },
            adopt_current,
            bindings,
        );
    }
    forges_entry(builder, root, words, bindings);
    let name = crate::accounts::words::say(words, crate::accounts::words::Word::Accounts, &[]);
    crate::link(builder, &name, bindings.insert(Act::Go { address: Address::Accounts }));
    builder.text(Run::Plain, &name);
    if match root.pages().address() {
        Address::Accounts | Address::ConnectModel { .. } => true,
        Address::Inbox { .. }
        | Address::Chats { .. }
        | Address::Welcome { .. }
        | Address::Forges
        | Address::ConnectForge { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::Adopt { .. } => false,
    } {
        builder.state(skein_ui::State::Current);
    }
    if let Some(mark) = crate::accounts::words::mark(root) {
        use crate::accounts::words::{Mark, Word};
        let (word, style) = match mark {
            Mark::Needs => (Word::NavNeeds, crate::look::NAV_NEEDS),
            Mark::Limited => (Word::NavLimited, crate::look::NAV_LIMITED),
        };
        let words = crate::accounts::words::say(words, word, &[]);
        builder.open(Role::Text);
        builder.style(style);
        builder.name(&words);
        builder.run(Run::Plain, &words);
        builder.close();
    }
    builder.close();
    builder.close();
}

/// The demo's one banner, after the accounts' banner and before the page and its chatbar (client/README.md, 5.1).
pub(crate) fn demo_banner(b: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) {
    if unavailable(root).is_some() {
        return;
    }
    match root.pages().address() {
        Address::Accounts
        | Address::ConnectModel { .. }
        | Address::Welcome { .. }
        | Address::ConnectForge { .. }
        | Address::Forges => return,
        Address::Chats { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::Inbox { .. }
        | Address::Adopt { .. } => {}
    }
    if let Some(setup) = temper_client_domain::forge::listed::setup(root) {
        b.open(Role::Region);
        b.style(crate::look::SETUP_STRIP);
        b.name(&words.say(crate::words::Word::Setup, &[]));
        let word = if setup.forge && setup.model {
            crate::words::Word::SetupBoth
        } else if setup.forge {
            crate::words::Word::SetupForge
        } else {
            crate::words::Word::SetupModel
        };
        text(b, Role::Paragraph, &words.say(word, &[]), None);
        setup_underway(b, root, words, bindings);
        action(
            b,
            Role::Link,
            &words.say(crate::words::Word::SetupContinue, &[]),
            Act::Go { address: Address::Welcome { step: setup.step } },
            false,
            bindings,
        );
        if setup.playground {
            action(
                b,
                Role::Button,
                &words.say(crate::words::Word::WelcomePlayground, &[]),
                Act::Playground,
                false,
                bindings,
            );
        }
        b.close();
        return;
    }
    let Some((_, halves)) = root.demo() else { return };
    b.open(Role::Region);
    b.style(crate::look::ACCOUNTS);
    b.name(&words.say(crate::words::Word::Demo, &[]));
    let word = if halves.model && halves.forge {
        crate::words::Word::DemoBoth
    } else if halves.model {
        crate::words::Word::DemoModel
    } else {
        crate::words::Word::DemoForge
    };
    text(b, Role::Paragraph, &words.say(word, &[]), None);
    if halves.model {
        action(
            b,
            Role::Link,
            &words.say(crate::words::Word::ConnectModel, &[]),
            Act::Go { address: Address::ConnectModel { way: None } },
            false,
            bindings,
        );
    }
    if halves.forge && !setup_underway(b, root, words, bindings) {
        action(
            b,
            Role::Link,
            &words.say(crate::words::Word::ConnectForge, &[]),
            Act::Go { address: Address::ConnectForge { forge: None } },
            false,
            bindings,
        );
    }
    b.close();
}

fn setup_underway(b: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) -> bool {
    let Some(row) = temper_client_domain::forge::connect::underway_forge(root.forges(), root.forge().demo()) else {
        return false;
    };
    crate::forge::connect::status(b, root, words, row, false);
    crate::forge::connect::link(
        b,
        words,
        bindings,
        crate::words::Word::ForgeView,
        Address::ConnectForge { forge: Some(temper_client_domain::Choice::Forge(row.number)) },
    );
    true
}

fn forges_entry(builder: &mut Builder, root: &Domain, words: &crate::words::Words, bindings: &mut Bindings) {
    use temper_client_domain::forge::listed::{self, Dot};
    crate::link(
        builder,
        &words.say(crate::words::Word::ForgesTitle, &[]),
        bindings.insert(Act::Go { address: Address::Forges }),
    );
    let current = match root.pages().address() {
        Address::Forges | Address::ConnectForge { .. } => true,
        Address::Accounts
        | Address::ConnectModel { .. }
        | Address::Welcome { .. }
        | Address::Chats { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::Inbox { .. }
        | Address::Adopt { .. } => false,
    };
    if current {
        builder.state(skein_ui::State::Current);
    }
    builder.text(Run::Plain, &words.say(crate::words::Word::ForgesTitle, &[]));
    let dot = match listed::dot(root.forges(), root.forge().demo()) {
        Dot::None => None,
        Dot::Amber => Some((crate::look::NAV_LIMITED, crate::words::Word::ForgeDotAmber)),
        Dot::Red => Some((crate::look::NAV_NEEDS, crate::words::Word::ForgeDotRed)),
    };
    // Drawn as the Accounts entry's dot: its state in words names the node.
    if let Some((style, word)) = dot {
        let said = words.say(word, &[]);
        builder.open(Role::Text);
        builder.style(style);
        builder.name(&said);
        builder.run(Run::Plain, &said);
        builder.close();
    }
    builder.close();
}
