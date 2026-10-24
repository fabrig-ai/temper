//! The first-run decision, using the session's forge rows (client/README.md, 3.5, 4.6).
use super::accounts;
use crate::{
    Domain,
    forge::{
        self,
        forges::{self, State as Forges},
    },
};
use jig_client::{Latest, State};
#[derive(Debug)]
pub struct Welcome {
    pub decided: bool,
}
impl Welcome {
    pub(crate) const fn new() -> Self {
        Self { decided: false }
    }
}
/// None while facts are missing; otherwise whether the person's first run is over.
#[must_use]
pub fn decision(root: &Domain) -> Option<bool> {
    let me = accounts::current_me(root.client())?;
    if !root.demo_answered() {
        return None;
    }
    let model = model_usable(root)?;
    let demo = root.forge().demo();
    if me.deployment_role != Some(jig_client::contract::HeldRole::Owner) {
        return Some(model || root.forge().demo_running());
    }
    match root.forges() {
        Forges::Unknown => None,
        Forges::Ended => Some(root.forge().demo_running()),
        Forges::Rows(rows) => {
            if root.forge().demo_running() {
                return Some(true);
            }
            for row in rows {
                let own = match demo {
                    Some(demo) => row.number != demo.forge,
                    None => true,
                };
                if own && let forges::Connection::Connected { .. } = row.connection {
                    return Some(model);
                }
            }
            Some(false)
        }
    }
}

/// The welcome and setup strip use the same model facts, including an ended watch.
#[must_use]
pub fn model_usable(root: &Domain) -> Option<bool> {
    let document = root.client().document(accounts::handle())?;
    let accounts = if document.state == State::Ended {
        None
    } else if let Some(Latest::Jig(jig_client::contract::Delivery::OwnerAccounts(accounts))) = document.latest {
        Some(accounts.as_ref())
    } else {
        return None;
    };
    let Some(accounts) = accounts else { return Some(false) };
    let Some(provider) = root.forge().demo_provider() else { return Some(accounts.usable) };
    Some(forge::demo::other_model_usable(
        accounts,
        forge::demo::owner_deployment(root.forge(), root.client()),
        provider,
    ))
}
