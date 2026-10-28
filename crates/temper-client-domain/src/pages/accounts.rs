//! Borrowed account facts and row-bound operations (client/README.md, 3.5–3.6).
use super::Request;
use crate::owner::Owner;
use jig_client::{Client, Handle, Latest, State, contract as c};
use skein_lib::Queue;
mod acts;
pub mod paste;
pub mod request;
pub use paste::{Paste, PasteState};
mod ways;
pub use acts::{Act, Broken, Operation, Pending};
pub use ways::{Way, ways};
/// The carried-value bound (client/README.md, 3.6); the engine catalogue is staged.
pub const KEY_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub holder: c::AccountHolder,
    pub account: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInState {
    Unknown,
    Started { source: u32 },
    Signing { source: u32 },
    Ended { source: u32 },
}

/// A fixed global handle outside the increasing page namespace; zero belongs to `me`.
#[must_use]
pub fn handle() -> Handle {
    Handle { asker: Owner::Pages.asker(), number: u64::MAX }
}

/// Missing, behind and ended watches are unknown, rather than false availability.
#[must_use]
pub fn document(client: &Client) -> Option<&c::OwnerAccounts> {
    current_me(client)?;
    let doc = client.document(handle())?;
    if doc.state != State::Live {
        return None;
    }
    if let Latest::Jig(c::Delivery::OwnerAccounts(accounts)) = doc.latest? { Some(accounts) } else { None }
}

#[derive(Debug)]
pub struct Accounts {
    pub deployment: Option<Handle>,
    pub key_for: Option<(Row, Option<u32>)>,
    pub request_row: Option<Row>,
    pub request: request::Request,
}

impl Default for Accounts {
    fn default() -> Self {
        Self::new()
    }
}

impl Accounts {
    #[must_use]
    pub fn new() -> Self {
        Self { deployment: None, key_for: None, request_row: None, request: request::Request::default() }
    }

    pub(crate) fn configure(&mut self, owner: bool, next: &mut u64, out: &mut Queue<Request>) {
        let deployment_key = match self.key_for {
            Some((row, _)) => row.holder == c::AccountHolder::Deployment,
            None => false,
        };
        if !owner && self.deployment.is_some() && deployment_key {
            self.request.clear_key();
            self.request.confirm = None;
            self.key_for = None;
        }
        if owner && self.deployment.is_none() {
            let handle = Handle { asker: Owner::Pages.asker(), number: *next };
            *next = next.checked_add(1).expect("page handles fit");
            self.deployment = Some(handle);
            out.push(Request::Client(jig_client::Event::Watch {
                handle,
                subject: jig_client::Watchable::Jig(c::Subject::Deployment),
            }));
        } else if !owner && let Some(handle) = self.deployment.take() {
            out.push(Request::Client(jig_client::Event::Unwatch { handle }));
        }
    }

    pub(crate) fn leave(&mut self, out: &mut Queue<Request>) {
        self.request.clear_key();
        self.request.confirm = None;
        self.key_for = None;
        if let Some(handle) = self.deployment.take() {
            out.push(Request::Client(jig_client::Event::Unwatch { handle }));
        }
    }

    #[must_use]
    pub fn deployment<'a>(&self, client: &'a Client) -> Option<&'a c::OwnerDeployment> {
        if current_me(client)?.deployment_role != Some(c::HeldRole::Owner) {
            return None;
        }
        let doc = client.document(self.deployment?)?;
        if doc.state != State::Live {
            return None;
        }
        if let Latest::Jig(c::Delivery::OwnerDeployment(deployment)) = doc.latest? { Some(deployment) } else { None }
    }
}

/// Current identity only; old owner facts cannot keep a deployment watch open.
#[must_use]
pub fn current_me(client: &Client) -> Option<&c::Me> {
    let doc = client.document(super::me_handle())?;
    if doc.state != State::Live {
        return None;
    }
    if let Latest::Jig(c::Delivery::Me(me)) = doc.latest? { Some(me) } else { None }
}
