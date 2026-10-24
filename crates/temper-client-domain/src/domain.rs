//! Temper's pages and forge share one durable jig client.
use crate::{Event, Limits, Request, forge, route};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, Rng, Time};
#[derive(Debug)]
pub struct Domain {
    pub(crate) pages: crate::pages::Pages,
    pub(crate) forge: forge::Side,
    pub(crate) client: jig_client::Client,
    pub(crate) unlinked: bool,
    pub(crate) departing: bool,
    pub(crate) authorization: Option<crate::authorize::Authorization>,
    pub(crate) authorizing: crate::authorize::Flight,
    pub(crate) authorization_revision: u64,
    pub(crate) signed_out: bool,
    pub(crate) returning: Option<(jig_client::Key, Box<[u8]>)>,
}
#[must_use]
pub fn new(limits: &Limits, seed: u64) -> Domain {
    build(limits, seed, false)
}
/// A browser root that leaves only after its own sign-in and return answers.
#[must_use]
pub fn new_departing(limits: &Limits, seed: u64) -> Domain {
    let mut root = new(limits, seed);
    root.departing = true;
    root
}
#[must_use]
pub fn unlinked(limits: &Limits, seed: u64) -> Domain {
    build(limits, seed, true)
}
/// The departing root on the typed transport used by the joined client world.
#[must_use]
pub fn unlinked_departing(limits: &Limits, seed: u64) -> Domain {
    let mut root = unlinked(limits, seed);
    root.departing = true;
    root
}
fn build(limits: &Limits, seed: u64, unlinked: bool) -> Domain {
    assert!(crate::worst_case(limits).is_some(), "root capacities are checked");
    let mut rng = Rng::new(seed);
    let client = if unlinked {
        jig_client::Client::unlinked(&limits.client, rng.next_u64())
    } else {
        jig_client::Client::new(&limits.client, rng.next_u64())
    }
    .expect("checked client limits");
    Domain {
        pages: crate::pages::Pages::seeded(rng.next_u64()),
        forge: forge::Side::new(&limits.forge, rng.next_u64()),
        client,
        unlinked,
        departing: false,
        returning: None,
        authorization: None,
        authorizing: crate::authorize::Flight::Idle,
        authorization_revision: 0,
        signed_out: false,
    }
}
impl Domain {
    #[must_use]
    pub const fn authorization(&self) -> Option<&crate::authorize::Authorization> {
        self.authorization.as_ref()
    }
    #[must_use]
    pub const fn signed_out(&self) -> bool {
        self.signed_out
    }
    #[must_use]
    pub const fn demo_answered(&self) -> bool {
        self.forge.demo_answered()
    }
    #[must_use]
    pub fn demo(&self) -> Option<(&forge::Demo, forge::demo::Halves)> {
        if !self.forge.demo_running() {
            return None;
        }
        let answer = self.forge.demo()?;
        let halves = forge::demo::current(&self.forge, self.pages.accounts_document(&self.client), &self.client);
        if halves.model || halves.forge { Some((answer, halves)) } else { None }
    }

    #[must_use]
    pub const fn pages(&self) -> &crate::pages::Pages {
        &self.pages
    }
    #[must_use]
    pub const fn client(&self) -> &jig_client::Client {
        &self.client
    }
    #[must_use]
    pub const fn forges(&self) -> &forge::forges::State {
        self.forge.forges()
    }
    #[must_use]
    pub const fn forge(&self) -> &forge::Side {
        &self.forge
    }
    #[must_use]
    pub const fn forge_number(&self) -> u16 {
        temper_client_protocol_wire::number(temper_client_protocol_wire::Owner::Forge)
    }
}
pub fn step(root: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    route::event(root, env, event, out);
    route::picker_progress(root, env, out);
    route::demo_progress(root, env, out);
    crate::authorize::progress(root, env, out);
    welcome(root, out);
}
pub fn fire(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut client = Queue::with_capacity(jig_client::max_out(&env.limits.client));
    root.client.fire(env.now, env.wall, &mut client);
    route::route_client(root, env, &mut client, out);
    route::fire_pages(root, env, out);
    route::picker_progress(root, env, out);
    route::demo_progress(root, env, out);
    crate::authorize::progress(root, env, out);
    welcome(root, out);
}
#[must_use]
pub fn next_deadline(root: &Domain) -> Option<Time> {
    [crate::pages::next_deadline(&root.pages), forge::next_deadline(&root.forge), root.client.next_deadline()]
        .into_iter()
        .flatten()
        .min()
}
/// Reclaim only after every child's tells have been routed.
pub fn take(root: &mut Domain, _env: &Env<Limits>, _out: &mut Queue<Request>) {
    root.client.reclaim();
    forge::reclaim(&mut root.forge);
}
#[must_use]
pub fn shown(root: &Domain) -> u64 {
    let count = crate::pages::shown(&root.pages);
    count
        .checked_add(forge::shown(&root.forge))
        .expect("forge shown count fits")
        .checked_add(root.authorization_revision)
        .expect("authorization revision fits")
}

fn welcome(root: &mut Domain, out: &mut Queue<Request>) {
    if root.authorization.is_some() {
        return;
    }
    if *root.pages.address() != (crate::Address::Welcome { step: crate::Step::Welcome }) {
        return;
    }
    let Some(page) = root.pages.welcome() else { return };
    if page.decided {
        return;
    }
    if let Some(over) = crate::pages::welcome::decision(root) {
        root.pages.decide_welcome();
        if over {
            out.push(Request::Show {
                address: crate::Address::Chats { filter: crate::ChatsFilter::All, start: false },
                replace: true,
            });
        }
    }
}
