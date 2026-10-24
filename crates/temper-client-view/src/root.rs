//! Temper's frame, pages, bindings and checked phrasebook.
use crate::{Limits, forge, words::Words};
use alloc::boxed::Box;
use skein_lib::{Queue, Wall};
use skein_ui::{Act, Builder, NodeId, Patch, Role, Tree};
use temper_client_domain::{Action, Domain};
/// The composed semantic tree and jig's bindings for its last build.
#[derive(Debug)]
pub struct View {
    render: skein_ui::View,
    sign_in: crate::sign_in::Ways,
    forge_sign_in: crate::forge_sign_in::Forges,
    forge_words: Words,
    forge: forge::bindings::Bindings,
    pages: crate::frame::Bindings,
    accounts: crate::accounts::Bindings,
    limits: Limits,
    rows: u32,
    text: u32,
    next_change: Option<Wall>,
    clock_revision: u64,
}
impl View {
    #[must_use]
    pub fn new(limits: &Limits, domain: &temper_client_domain::Limits, forge_words: Words, forge: u16) -> View {
        Self::with_sign_in(limits, domain, forge_words, forge, crate::sign_in::Ways::NONE)
    }
    /// Construct the browser view with the ways advertised by its page.
    #[must_use]
    pub fn with_sign_in(
        limits: &Limits,
        domain: &temper_client_domain::Limits,
        forge_words: Words,
        forge: u16,
        sign_in: crate::sign_in::Ways,
    ) -> View {
        assert!(forge_words.connector == forge, "phrasebook belongs to its forge connector");
        assert!(crate::worst_case(limits, domain, &forge_words).is_some(), "view capacities are checked");
        View {
            sign_in,
            forge_sign_in: crate::forge_sign_in::Forges::new(),
            render: skein_ui::View::new(&limits.tree),
            forge: forge::bindings::Bindings::new(limits.tree.bindings, domain.forge.checks, forge),
            pages: crate::frame::Bindings::new(limits.tree.bindings),
            accounts: crate::accounts::Bindings::new(domain.accounts, domain.client.domain.rows),
            forge_words,
            limits: *limits,
            rows: domain.client.domain.rows,
            text: domain.client.domain.text,
            next_change: None,
            clock_revision: 0,
        }
    }
    /// Supply the page's forge facts before its first build.
    pub fn forge_sign_in(&mut self, forges: crate::forge_sign_in::Forges) {
        self.forge_sign_in = forges;
    }
    fn task_main(&mut self, b: &mut Builder, root: &Domain, times: &mut crate::words::Times) {
        crate::pages::task::main(b, root, &self.forge_words, &mut self.pages, self.rows, times);
    }
    fn adoption_main(&mut self, b: &mut Builder, root: &Domain) {
        let Some(form) = root.forge().form() else { return };
        let project = crate::pages::project_name(
            root.pages().me(root.client()),
            root.forge().demo(),
            form.project,
            &self.forge_words,
        );
        b.open(Role::Main);
        b.style(crate::look::MAIN);
        b.style(crate::look::PAGE);
        b.open(Role::Slot);
        b.style(crate::look::ADOPTION_PAGE);
        forge::page::page(
            b,
            form,
            project.as_deref(),
            &self.forge_words,
            &mut self.forge,
            self.limits.tree.text_per_node,
        );
        b.close();
        b.close();
    }
    /// Earliest change among the time words drawn by the last build.
    #[must_use]
    pub const fn next_change(&self) -> Option<Wall> {
        self.next_change
    }
    #[must_use]
    pub fn tree(&self) -> &Tree {
        self.render.tree()
    }
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }
}
#[expect(clippy::too_many_lines, reason = "one ordered dispatch draws each address's page")]
pub fn build(view: &mut View, root: &Domain, wall: Wall, out: &mut Queue<Patch>) {
    let shown = temper_client_domain::shown(root).checked_add(view.accounts.revision()).expect("view revisions fit");
    let shown = shown.checked_add(view.clock_revision).expect("view clock revisions fit");
    let due = match view.next_change {
        Some(next) => wall >= next,
        None => false,
    };
    if !view.render.stale(shown) && !due {
        return;
    }
    // skein-ui also gates finish by its revision: a clock wake advances it.
    let shown = if due {
        view.clock_revision = view.clock_revision.checked_add(1).expect("view clock revisions fit");
        shown.checked_add(1).expect("view revisions fit")
    } else {
        shown
    };
    let mut times = crate::words::Times::new(wall);
    view.forge.begin();
    view.pages.begin();
    view.accounts.begin(root.pages().accounts().is_none());
    let mut builder = view.render.builder();
    builder.open(Role::Page);
    crate::frame::banner(&mut builder, root, &view.forge_words, &mut view.pages);
    match root.pages().address() {
        temper_client_domain::Address::Welcome { .. } => {}
        temper_client_domain::Address::ConnectModel { .. }
        | temper_client_domain::Address::Accounts
        | temper_client_domain::Address::Chats { .. }
        | temper_client_domain::Address::Chat { .. }
        | temper_client_domain::Address::Task { .. }
        | temper_client_domain::Address::Inbox { .. }
        | temper_client_domain::Address::Forges
        | temper_client_domain::Address::ConnectForge { .. }
        | temper_client_domain::Address::Adopt { .. } => {
            crate::choice::banner(&mut builder, root, &view.forge_words, &mut view.pages);
            crate::frame::demo_banner(&mut builder, root, &view.forge_words, &mut view.pages);
        }
    }
    if let Some(page) = root.authorization() {
        crate::sign_in::authorize(
            &mut builder,
            root,
            page.state(),
            page.query(),
            &view.forge_words,
            &view.sign_in,
            &view.forge_sign_in,
        );
    } else if crate::sign_in::offered(root, &view.forge_words, &view.sign_in)
        && let Some(origin) = &view.sign_in.origin
    {
        crate::sign_in::main(&mut builder, root, &view.forge_words, origin, &view.forge_sign_in);
    } else if crate::frame::unavailable(root).is_some() {
        builder.open(Role::Main);
        builder.style(crate::look::MAIN);
        builder.open(Role::Paragraph);
        builder.style(crate::look::UNAVAILABLE);
        crate::frame::unavailable_main(&mut builder, root, &view.forge_words);
        builder.close();
        if let Some(page) = root.pages().task()
            && page.document(root.client()).is_none()
        {
            crate::pages::task::reading(&mut builder, page, &view.forge_words);
        }
        builder.close();
    } else if let temper_client_domain::Address::Welcome { step } = root.pages().address() {
        crate::pages::welcome::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.pages,
            &mut view.accounts,
            &mut view.forge,
            *step,
            view.limits.tree.text_per_node,
        );
    } else if let temper_client_domain::Address::Inbox { .. } = root.pages().address() {
        crate::pages::inbox::main(&mut builder, root, &view.forge_words, &mut view.pages, view.rows, &mut times);
    } else if let temper_client_domain::Address::Chats { .. } = root.pages().address() {
        crate::pages::chats::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.pages,
            view.limits.tree.text_per_node,
            &mut times,
        );
    } else if let temper_client_domain::Address::Chat { .. } = root.pages().address() {
        crate::pages::chat::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.pages,
            view.rows,
            view.text,
            &mut times,
        );
    } else if let temper_client_domain::Address::Task { .. } = root.pages().address() {
        view.task_main(&mut builder, root, &mut times);
    } else if let temper_client_domain::Address::ConnectModel { .. } = root.pages().address() {
        crate::pages::connect_model::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.accounts,
            view.limits.tree.text_per_node,
        );
    } else if *root.pages().address() == temper_client_domain::Address::Accounts {
        crate::pages::accounts::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.accounts,
            view.limits.tree.text_per_node,
        );
    } else if let temper_client_domain::Address::ConnectForge { forge } = root.pages().address() {
        forge::connect::main(&mut builder, root, &view.forge_words, &mut view.pages, &mut view.forge, *forge);
    } else if *root.pages().address() == temper_client_domain::Address::Forges {
        forge::forges::main(
            &mut builder,
            root,
            &view.forge_words,
            &mut view.pages,
            &mut view.forge,
            &view.forge_sign_in,
        );
    } else if let temper_client_domain::Address::Adopt { .. } = root.pages().address() {
        view.adoption_main(&mut builder, root);
    }
    builder.close();
    view.next_change = times.next;
    view.render.finish(builder, shown, out);
}
#[must_use]
pub fn act(view: &mut View, node: NodeId, act: Act) -> Option<Action> {
    if let temper_client_domain::Event::Act { action } = event(view, node, act)? { Some(action) } else { None }
}
/// The address a link node opens, which the browser shell gives as its href
/// (client/browser.md, 2.3 and 7.1): its external address, or the page its press goes to.
#[must_use]
pub fn href(view: &mut View, node: NodeId) -> Option<Box<[u8]>> {
    for shown in view.tree().nodes() {
        if shown.id == node
            && let Some(address) = &shown.external
        {
            return Some(address.clone());
        }
    }
    match act(view, node, Act::Press)? {
        Action::Pages(temper_client_domain::pages::Act::OpenProject { .. }) => {
            Some(temper_client_domain::address::print(&temper_client_domain::Address::Chats {
                filter: temper_client_domain::ChatsFilter::All,
                start: false,
            }))
        }
        Action::Pages(temper_client_domain::pages::Act::Go { address }) => {
            Some(temper_client_domain::address::print(&address))
        }
        Action::Forge(temper_client_domain::forge::Act::Go {
            address: temper_client_domain::forge::Address::Adopt { project },
        }) => Some(temper_client_domain::address::print(&temper_client_domain::Address::Adopt { project })),
        Action::Pages(_) | Action::Forge(_) => None,
    }
}
/// Shell inputs can move a secret directly to the domain without a clonable action.
#[must_use]
pub fn event(view: &mut View, node: NodeId, act: Act) -> Option<temper_client_domain::Event> {
    let bound = view.render.act(node, act)?;
    if bound.binding.owner == crate::accounts::owner() {
        return view.accounts.decode(bound.binding.token, bound.act);
    }
    if bound.binding.owner == crate::frame::owner() {
        return Some(temper_client_domain::Event::Act { action: view.pages.decode(bound.binding.token, bound.act)? });
    }
    if bound.binding.owner == temper_client_domain::forge::words::owner(view.forge_words.connector) {
        forge::bindings::event(&view.forge, bound.binding.token, bound.act)
    } else {
        None
    }
}
