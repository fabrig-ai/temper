//! A sign-in's browser address, pasted and sent once (client/README.md, 3.5, The paste).
use super::{Pending, SignInState, document, request::Request};
use crate::{owner::Owner, pages::Request as Output};
use alloc::boxed::Box;
use jig_client::{Client, Key, contract as c};
use skein_lib::{Queue, RequestKey, Secret};

#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "the pasted address is never bound or persisted")]
pub struct Paste {
    value: Option<Secret>,
    written: u64,
    pub pending: Option<Pending>,
    pub too_long: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteState {
    Closed,
    Waiting { ready: bool },
    TooLong,
    Sending,
    Finishing,
    Refused { why: c::SignInRedirectWhy },
    Ended,
    Again,
}
impl Paste {
    #[must_use]
    pub const fn written(&self) -> u64 {
        self.written
    }
    pub(crate) fn out(&self) -> bool {
        match &self.pending {
            Some(pending) => pending.outcome.is_none(),
            None => false,
        }
    }
    pub(crate) fn has_pending(&self, key: Key) -> bool {
        match &self.pending {
            Some(pending) => pending.key == key,
            None => false,
        }
    }
    pub(super) fn clear(&mut self) {
        self.value = None;
        self.written = self.written.checked_add(1).expect("paste writes fit");
    }
}
#[expect(clippy::derivable_impls, reason = "the programming model uses explicit Default implementations")]
impl Default for Paste {
    fn default() -> Self {
        Self { value: None, written: 0, pending: None, too_long: false }
    }
}
impl Request {
    pub(crate) fn paste_value(&mut self, value: Secret, bound: u32) {
        if self.paste.out() {
            return;
        }
        self.paste.value = None;
        // Only the four surrounding characters named by the paste's contract are trimmed.
        let bytes = value.expose();
        let mut start = None;
        let mut end = 0;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte != b' ' && *byte != b'\t' && *byte != b'\r' && *byte != b'\n' {
                if start.is_none() {
                    start = Some(index);
                }
                end = index.checked_add(1).expect("paste length fits");
            }
        }
        let bytes = bytes.get(start.unwrap_or(end)..end).expect("trimmed range fits");
        self.paste.too_long = bytes.len() > usize::try_from(bound).expect("redirect bound fits");
        if !self.paste.too_long && !bytes.is_empty() {
            self.paste.value = Some(Secret::new(Box::from(bytes)));
        }
        self.paste.pending = None;
    }
    pub(crate) fn finish(&mut self, client: &Client, name: RequestKey, serial: &mut u64, out: &mut Queue<Output>) {
        if self.paste.out() {
            return;
        }
        let source = match self.sign_in_state(client) {
            SignInState::Started { source } | SignInState::Signing { source } => source,
            SignInState::Unknown | SignInState::Ended { .. } => return,
        };
        let Some(value) = self.paste.value.take() else { return };
        let digest = skein_lib::sha256::digest(value.expose());
        self.paste.clear();
        *serial = serial.checked_add(1).expect("page keys fit");
        let key = Key { asker: Owner::Pages.asker(), number: *serial };
        let ask = c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::FinishSignIn { source, digest },
        };
        self.paste.pending =
            Some(Pending { key, name: Some(name), ask: ask.clone(), outcome: None, signing: false, ended: false });
        out.push(Output::Client(jig_client::Event::Carry {
            key,
            name,
            request: jig_client::Ask::Jig(Box::new(ask)),
            value: Some(value),
        }));
    }
    #[must_use]
    pub fn paste_state(&self, client: &Client) -> PasteState {
        let state = self.sign_in_state(client);
        let source = match state {
            SignInState::Started { source } | SignInState::Signing { source } | SignInState::Ended { source } => {
                Some(source)
            }
            SignInState::Unknown => None,
        };
        if let SignInState::Ended { .. } = state {
            if let Some(doc) = document(client)
                && let Some(end) = &doc.last_end
                && Some(end.source) == source
            {
                return PasteState::Closed;
            }
            return PasteState::Ended;
        }
        let Some(pending) = &self.paste.pending else {
            return match state {
                SignInState::Started { .. } | SignInState::Signing { .. } => {
                    if self.paste.too_long {
                        PasteState::TooLong
                    } else {
                        PasteState::Waiting { ready: self.paste.value.is_some() }
                    }
                }
                SignInState::Unknown | SignInState::Ended { .. } => PasteState::Closed,
            };
        };
        match &pending.outcome {
            None => PasteState::Sending,
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountFinishing { .. },
            ))))) => PasteState::Finishing,
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
                refusal: c::Refusal::SignInRedirect { why },
                ..
            }))) => {
                if *why == c::SignInRedirectWhy::NotWaiting {
                    PasteState::Ended
                } else {
                    PasteState::Refused { why: *why }
                }
            }
            Some(
                jig_client::Outcome::Lost
                | jig_client::Outcome::Unsent
                | jig_client::Outcome::Answered(_)
                | jig_client::Outcome::NotKept
                | jig_client::Outcome::Held
                | jig_client::Outcome::Unknown
                | jig_client::Outcome::Limit
                | jig_client::Outcome::Full { .. }
                | jig_client::Outcome::Unconfirmed,
            ) => PasteState::Again,
        }
    }
}
