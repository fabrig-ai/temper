//! Route the shared client's tells to temper's owning child.
use crate::ChatsFilter;
use crate::{Action, Address, Domain, Event, Fact, Limits, Place, Request, forge, owner::Owner};
use skein_lib::{Env, Queue};
pub(crate) fn forge(root: &mut Domain, env: &Env<Limits>, event: forge::Event, out: &mut Queue<Request>) {
    // Any first-repository adoption advances the welcome, whichever flow asked for it
    // (client/README.md, The welcome). Standalone ConnectForge keeps its Ready card.
    let mut adopted_in_welcome = false;
    if let Address::Welcome { step: crate::Step::Forge { .. } } = root.pages.address()
        && let forge::Event::Answered { request, answer: forge::Answer::Adopted(adopted) } = &event
    {
        let pending = match root.forge.picker(adopted.forge) {
            Some(picker) => picker.adoption.as_ref(),
            None => match root.forge.set_up() {
                Some(last) if last.forge == adopted.forge => last.adoption.as_ref(),
                Some(_) | None => None,
            },
        };
        if let Some(adoption) = pending
            && adoption.request == *request
            && adoption.outcome.is_none()
        {
            adopted_in_welcome = true;
        }
    }
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::step(&mut root.forge, &Env { now: env.now, wall: env.wall, limits: env.limits.forge }, event, &mut child);
    route_forge(root, env, &mut child, out);
    if adopted_in_welcome {
        out.push(Request::Show {
            address: Address::Welcome { step: crate::Step::Model { way: None } },
            replace: false,
        });
    }
}
pub(crate) fn event(root: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Start { address, kept, with } => {
            if !root.unlinked {
                client(root, env, jig_client::Event::Start { kept }, out);
            }
            // Start restores the client; sign-in must precede the page's one-shot reads.
            if let Some(with) = with {
                client(root, env, jig_client::Event::SignIn { with }, out);
            }
            told(root, env, jig_client::Told::Link { link: root.client.link().state }, out);
            pages(root, env, crate::pages::Event::Started, out);
            went(root, env, Place::Address(address), out);
        }
        Event::Went { place } => went(root, env, place, out),
        Event::Act { action } => match action {
            Action::Pages(crate::pages::Act::SignOut) => {
                root.signed_out = true;
                root.authorization_revision = root.authorization_revision.checked_add(1).expect("revision fits");
                pages(root, env, crate::pages::Event::Act { act: crate::pages::Act::SignOut }, out);
            }
            Action::Pages(crate::pages::Act::RetryDemo) => {
                forge(root, env, forge::Event::Act { act: forge::Act::StartDemo }, out);
            }
            Action::Pages(crate::pages::Act::Playground) => playground(root, env, out),
            Action::Pages(crate::pages::Act::Suggest { cue }) => suggest(root, env, cue, out),
            Action::Pages(act) => pages(root, env, crate::pages::Event::Act { act }, out),
            Action::Forge(act) => forge(root, env, forge::Event::Act { act }, out),
        },
        Event::Client { event } => client(root, env, event, out),
        Event::Forge(event) => forge(root, env, event, out),
        Event::SignInAddress { value } => pages(root, env, crate::pages::Event::SignInAddress { value }, out),
        Event::AccountKey { value } => pages(root, env, crate::pages::Event::AccountKey { value }, out),
        Event::SignedOut => client(root, env, jig_client::Event::Up { up: jig_client::Up::SignedOut }, out),
    }
}
fn went(root: &mut Domain, env: &Env<Limits>, place: Place, out: &mut Queue<Request>) {
    crate::authorize::went(root, &place);
    match place {
        Place::Address(address) => {
            pages(root, env, crate::pages::Event::Went { address: address.clone() }, out);
            if let Address::Adopt { project } = address {
                forge(root, env, forge::Event::Went { address: forge::Address::Adopt { project } }, out);
            } else {
                forge(root, env, forge::Event::Left, out);
            }
        }
        Place::Authorize { .. } | Place::AuthorizeMalformed | Place::Missing => {
            pages(root, env, crate::pages::Event::Left, out);
            forge(root, env, forge::Event::Left, out);
        }
    }
}
pub(crate) fn pages(root: &mut Domain, env: &Env<Limits>, event: crate::pages::Event, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(crate::pages::max_out(&env.limits));
    demo_facts(root);
    let task_document = match &event {
        crate::pages::Event::Told {
            told: jig_client::Told::Changed { handle } | jig_client::Told::Page { handle, .. },
        } => match root.pages.task() {
            Some(task) => *handle == task.document,
            None => false,
        },
        crate::pages::Event::Started
        | crate::pages::Event::Left
        | crate::pages::Event::SignInAddress { .. }
        | crate::pages::Event::AccountKey { .. }
        | crate::pages::Event::Went { .. }
        | crate::pages::Event::Act { .. }
        | crate::pages::Event::Told { .. }
        | crate::pages::Event::Link { .. } => false,
    };
    crate::pages::step(&mut root.pages, env, &root.client, event, &mut child);
    route_pages(root, env, &mut child, out);
    let subject = match root.pages.task() {
        Some(task) => task.panel(&root.client, root.forge_number()),
        None => None,
    };
    #[expect(clippy::manual_map, reason = "the client's domain matches without a closure")]
    let watched = match root.forge.panel() {
        Some(panel) => Some(panel.subject),
        None => None,
    };
    if watched != subject {
        forge(root, env, forge::Event::Panel { subject }, out);
    } else if task_document && let Some(subject) = subject {
        refresh_panel(root, env, subject, false, out);
    }
}

fn refresh_panel(
    root: &mut Domain,
    env: &Env<Limits>,
    subject: forge::panel::Subject,
    from_ending: bool,
    out: &mut Queue<Request>,
) {
    let forge_number = root.forge_number();
    if let Some(page) = root.pages.task_mut()
        && page.panel(&root.client, forge_number) == Some(subject)
        && let Some(task) = page.document(&root.client)
        && task.chip.task == page.number
        && forge::panel::refreshable(task, forge_number)
        && page.panel_reopened_at != Some(task.activity)
    {
        if from_ending {
            // Only a finished Change can recover without a later document; bound
            // missing-record replies to one immediate reopen per document.
            if task.chip.phase != jig_client::contract::Phase::Ended {
                return;
            }
            page.panel_reopened_at = Some(task.activity);
        }
        forge(root, env, forge::Event::RefreshPanel { subject }, out);
    }
}

pub(crate) fn fire_pages(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(crate::pages::max_out(&env.limits));
    crate::pages::fire(&mut root.pages, env, &root.client, &mut child);
    route_pages(root, env, &mut child, out);
}

fn route_pages(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<crate::pages::Request>,
    out: &mut Queue<Request>,
) {
    for _ in 0..child.len() {
        match child.pop().expect("counted pages output") {
            crate::pages::Request::Client(event) => client(root, env, event, out),
            crate::pages::Request::Replace { address } => out.push(Request::Show { address, replace: true }),
            crate::pages::Request::Show { address } => out.push(Request::Show { address, replace: false }),
        }
    }
}

pub(crate) fn route_forge(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<forge::Request>,
    out: &mut Queue<Request>,
) {
    for _ in 0..child.len() {
        match child.pop().expect("counted forge output") {
            forge::Request::PanelEnded { subject } => refresh_panel(root, env, subject, true, out),
            forge::Request::Client { asked } => client(root, env, asked, out),
            forge::Request::Leave { to } => out.push(Request::Leave { to, replace: false }),
            forge::Request::Show { address } => {
                let forge::Address::Adopt { project } = address;
                out.push(Request::Show { address: Address::Adopt { project }, replace: false });
            }
            forge::Request::Send { request, name, ask } => client(
                root,
                env,
                jig_client::Event::Send {
                    key: jig_client::Key { asker: Owner::Forge.asker(), number: request.raw() },
                    name,
                    request: ask,
                },
                out,
            ),
        }
    }
}
pub(crate) fn client(root: &mut Domain, env: &Env<Limits>, event: jig_client::Event, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(jig_client::max_out(&env.limits.client));
    root.client.step(env.now, env.wall, event, &mut child);
    route_client(root, env, &mut child, out);
}
pub(crate) fn route_client(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<jig_client::Request>,
    out: &mut Queue<Request>,
) {
    while let Some(request) = child.pop() {
        match request {
            jig_client::Request::Told { told: value } => told(root, env, value, out),
            jig_client::Request::Authorized(answer) => crate::authorize::answered(root, answer, out),
            request @ (jig_client::Request::Save { .. }
            | jig_client::Request::Erase { .. }
            | jig_client::Request::Exchange { .. }
            | jig_client::Request::OpenStream { .. }
            | jig_client::Request::CloseStream
            | jig_client::Request::Down { .. }
            | jig_client::Request::ServiceCredential { .. }
            | jig_client::Request::SignOut) => out.push(Request::Client { request }),
        }
    }
    forge_served(root, env, out);
}
pub(crate) fn told(root: &mut Domain, env: &Env<Limits>, told: jig_client::Told, out: &mut Queue<Request>) {
    use jig_client::Told;
    if crate::departure::told(root, env, &told, out) {
        return;
    }
    let asker = match &told {
        Told::Link { link } => {
            pages(root, env, crate::pages::Event::Link { link: *link }, out);
            forge_served(root, env, out);
            forge_demo(root, env, &told, out);
            forge_me(root, env, &told, out);
            return;
        }
        Told::Dropped { count } => {
            out.push(Request::Dropped { count: *count });
            return;
        }
        Told::Kept { key, .. } | Told::Pending { key } | Told::Answered { key, .. } => key.asker,
        Told::Changed { handle } | Told::Text { handle } | Told::Ended { handle, .. } | Told::Page { handle, .. } => {
            handle.asker
        }
    };
    match Owner::of(asker) {
        Some(Owner::Pages) => pages(root, env, crate::pages::Event::Told { told }, out),
        Some(Owner::Forge) => {
            if forge_demo(root, env, &told, out) {
                return;
            }
            if forge_forges(root, env, &told, out) {
                return;
            }
            if forge_me(root, env, &told, out) {
                return;
            }
            if let Some(event) = forge::told(&root.client, &env.limits, told) {
                forge(root, env, event, out);
            }
        }
        Some(Owner::Runner) | None => out.push(Request::Fact { fact: Fact::UnknownAsker }),
    }
}

fn forge_served(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let served = if root.client.link().not_served.contains(&root.forge_number()) {
        forge::Served::OutOfDate
    } else {
        forge::Served::Yes
    };
    if root.forge.served() != served {
        forge(root, env, forge::Event::Served { served }, out);
    }
}

fn forge_me(root: &mut Domain, env: &Env<Limits>, told: &jig_client::Told, out: &mut Queue<Request>) -> bool {
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    let consumed = forge::me::told(&mut root.forge, &root.client, told, &mut child);
    if consumed {
        forge::forges::configure(&mut root.forge, env.limits.forge, &mut child);
    }
    route_forge(root, env, &mut child, out);
    consumed
}

fn forge_demo(root: &mut Domain, env: &Env<Limits>, told: &jig_client::Told, out: &mut Queue<Request>) -> bool {
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    let unknown = if let jig_client::Told::Pending { key } = told {
        match root.client.pending(*key) {
            Some(pending) => pending.progress == jig_client::Progress::Unknown,
            None => false,
        }
    } else {
        false
    };
    let value;
    let told = if unknown && let jig_client::Told::Pending { key } = told {
        value = jig_client::Told::Answered { key: *key, outcome: jig_client::Outcome::Unknown };
        &value
    } else {
        told
    };
    let consumed = forge::demo::told(&mut root.forge, told, env.limits.forge, &mut child);
    route_forge(root, env, &mut child, out);
    // A demo answer can arrive after the page and account watch. Way resolution
    // must exclude its provider on that same rebuild.
    demo_facts(root);
    consumed
}
fn suggest(root: &mut Domain, env: &Env<Limits>, cue: u8, out: &mut Queue<Request>) {
    if root.forge.demo_starting() || root.forge.demo_failure().is_some() {
        return;
    }
    let Some((demo, halves)) = root.demo() else { return };
    if !halves.model || root.pages.project() != Some(demo.project) {
        return;
    }
    let Some(words) = demo.cues.get(usize::from(cue)) else { return };
    let words = words.clone();
    let mut child = Queue::with_capacity(crate::pages::max_out(&env.limits));
    root.pages.suggest(env, &root.client, &words, &mut child);
    while let Some(request) = child.pop() {
        match request {
            crate::pages::Request::Client(event) => client(root, env, event, out),
            crate::pages::Request::Replace { address } => out.push(Request::Show { address, replace: true }),
            crate::pages::Request::Show { address } => out.push(Request::Show { address, replace: false }),
        }
    }
}

fn playground(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let Some(demo) = root.forge.demo() else { return };
    if !root.demo_answered() {
        return;
    }
    let project = demo.project;
    forge(root, env, forge::Event::Act { act: forge::Act::StartDemo }, out);
    demo_facts(root);
    pages(root, env, crate::pages::Event::Act { act: crate::pages::Act::Project { project } }, out);
    out.push(Request::Show { address: Address::Chats { filter: ChatsFilter::All, start: false }, replace: false });
}

fn forge_forges(root: &mut Domain, env: &Env<Limits>, told: &jig_client::Told, out: &mut Queue<Request>) -> bool {
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    let consumed = forge::picker::told(&mut root.forge, &root.client, told, env.limits.forge)
        || forge::admin::told(&mut root.forge, &root.client, told, env.limits.forge, &mut child)
        || forge::github::told(&mut root.forge, &root.client, told, env.limits.forge, &mut child)
        || forge::forges::told(&mut root.forge, &root.client, told, env.limits.forge, &mut child);
    if consumed
        && let jig_client::Told::Answered { .. } = told
        && let Some(last) = root.forge.set_up()
        && last.answer == Some(forge::forges::SetUpAnswer::Outcome(forge::forges::SetUpOutcome::SettingUp))
        && let Address::Welcome { step: crate::Step::Forge { .. } } = root.pages.address()
    {
        out.push(Request::Show {
            address: Address::Welcome { step: crate::Step::Model { way: None } },
            replace: false,
        });
    }
    route_forge(root, env, &mut child, out);
    consumed
}

#[expect(clippy::manual_map, reason = "step code reads retained identity without closures")]
fn demo_facts(root: &mut Domain) {
    root.pages.demo_provider = root.forge.demo_provider();
    root.pages.retired_project = match root.forge.retired_demo() {
        Some(demo) => Some(demo.project),
        None => None,
    };
    crate::pages::project_changed(&mut root.pages, &root.client);
    root.pages.demo_model = match root.demo() {
        Some((_, halves)) => halves.model,
        None => false,
    };
}

/// Reconcile demo lifecycle after the shared client's account and forge tells.
pub(crate) fn demo_progress(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::demo::progress(&mut root.forge, root.pages.accounts_document(&root.client), &root.client, &mut child);
    route_forge(root, env, &mut child, out);
    demo_facts(root);
}

/// Ready reads only while its flow is shown; pending starts survive navigation.
pub(crate) fn picker_progress(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::picker::progress(&mut root.forge, &root.client, root.pages.address(), env.limits.forge, env.now, &mut child);
    route_forge(root, env, &mut child, out);
}
