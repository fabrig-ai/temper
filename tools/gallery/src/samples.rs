//! The promised names, in client/browser.md, 7.2's order.
use crate::driver::{limits, step};
use jig_client::contract as c;
use skein_lib::{Queue, Token};
use temper_client_domain::{Action, Address, Domain, Event, Request, address::Filter, pages::Act};
use temper_client_domain::{ChatsFilter, PlanFilter};

use Then::{
    AccountRefused, Closing, Declined, Filled, Held, KeyReady, OutOfDate, Refused, Rejecting, Releasing, Renewed,
    Shown, SignInReady, SignInRefused, SignedOut, SigningIn,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    Reference,
    Solo,
    FirstRun { step: u8 },
    Bare,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Shown,
    Filled,
    Refused,
    SignedOut,
    GoogleSignIn,
    OutOfDate,
    Closing,
    Rejecting,
    Held,
    Releasing,
    SigningIn,
    Paste(PasteFrame),
    AccountRefused,
    KeyReady,
    SignInReady,
    Declined,
    Renewed,
    SignInRefused,
}
/// The paste's visible frames, reached through the same public events as a browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteFrame {
    Waiting,
    Written,
    TooLong,
    Sending,
    Finishing,
    Address,
    State,
    TooMany,
    Ended,
    Expired,
    Lost,
    Unsent,
}
#[derive(Clone, Debug)]
pub struct Sample {
    pub name: &'static str,
    pub world: World,
    pub address: Address,
    pub then: Then,
}
pub const N: usize = 81;
const CHATS: Address = Address::Chats { filter: ChatsFilter::All, start: false };
use World::{Bare, FirstRun, Reference, Solo};
#[must_use]
fn task(task: u64) -> Address {
    Address::Task { task, filter: PlanFilter::All }
}

#[must_use]
#[expect(clippy::too_many_lines, reason = "one entry per named gallery state, in index order")]
pub fn samples() -> [Sample; N] {
    [
        Sample { name: "chats-solo", world: Solo, address: CHATS, then: Shown },
        Sample { name: "chat-solo-T10", world: Solo, address: Address::Chat { task: 10 }, then: Shown },
        Sample { name: "inbox-solo", world: Solo, address: Address::Inbox { filter: Filter::ALL }, then: Shown },
        Sample { name: "task-solo-T3", world: Solo, address: task(3), then: Shown },
        Sample { name: "accounts-solo", world: Solo, address: Address::Accounts, then: Shown },
        welcome_sample("welcome-first-run-0", temper_client_domain::Step::Welcome),
        Sample { name: "chats-first-run-1", world: FirstRun { step: 1 }, address: CHATS, then: Shown },
        owned_sample("connect-forge-first-run-6", 6, false),
        Sample { name: "forges-first-run-7", world: FirstRun { step: 7 }, address: Address::Forges, then: Shown },
        Sample {
            name: "connect-forge-choose",
            world: FirstRun { step: 0 },
            address: Address::ConnectForge { forge: None },
            then: Shown,
        },
        welcome_sample("welcome-forge-choose", temper_client_domain::Step::Forge { forge: None }),
        welcome_sample(
            "welcome-forge-details",
            temper_client_domain::Step::Forge { forge: Some(temper_client_domain::Choice::Forge(3)) },
        ),
        welcome_sample("welcome-model-choose", temper_client_domain::Step::Model { way: None }),
        welcome_model_sample("welcome-model-key-details", temper_client_domain::WayKind::Key, Shown),
        welcome_model_sample("welcome-model-signing-in", temper_client_domain::WayKind::SignIn, SigningIn),
        welcome_model_sample("welcome-model-signin-ready", temper_client_domain::WayKind::SignIn, SignInReady),
        welcome_model_sample("welcome-model-declined", temper_client_domain::WayKind::SignIn, Declined),
        welcome_model_sample("welcome-model-renewed", temper_client_domain::WayKind::SignIn, Renewed),
        welcome_model_sample("welcome-model-refused", temper_client_domain::WayKind::SignIn, SignInRefused),
        paste_sample("welcome-paste-waiting", PasteFrame::Waiting),
        paste_sample("welcome-paste-written", PasteFrame::Written),
        paste_sample("welcome-paste-too-long", PasteFrame::TooLong),
        paste_sample("welcome-paste-sending", PasteFrame::Sending),
        paste_sample("welcome-paste-finishing", PasteFrame::Finishing),
        paste_sample("welcome-paste-address", PasteFrame::Address),
        paste_sample("welcome-paste-state", PasteFrame::State),
        paste_sample("welcome-paste-too-many", PasteFrame::TooMany),
        paste_sample("welcome-paste-ended", PasteFrame::Ended),
        paste_sample("welcome-paste-expired", PasteFrame::Expired),
        paste_sample("welcome-paste-lost", PasteFrame::Lost),
        paste_sample("welcome-paste-unsent", PasteFrame::Unsent),
        github_sample("github-app-details", 0),
        github_sample("github-install", 20),
        github_sample("github-picker", 21),
        github_sample("github-adopted", 22),
        owned_sample("forge-details", 0, false),
        owned_sample("forge-setting-up-middle", 6, false),
        owned_sample("forge-setup-failed", 9, false),
        owned_sample("welcome-setup-failed", 9, true),
        owned_sample("welcome-ready-under-way", 6, true),
        owned_sample("welcome-ready-adopted", 8, true),
        Sample { name: "chats-setup-strip", world: Bare, address: CHATS, then: Shown },
        Sample { name: "adoption", world: Reference, address: Address::Adopt { project: 1 }, then: Shown },
        Sample { name: "adoption-filled", world: Reference, address: Address::Adopt { project: 1 }, then: Filled },
        Sample { name: "adoption-refused", world: Reference, address: Address::Adopt { project: 1 }, then: Refused },
        Sample { name: "chats-reference", world: Reference, address: CHATS, then: Shown },
        Sample { name: "chat-reference-T1", world: Reference, address: Address::Chat { task: 1 }, then: Shown },
        Sample { name: "chat-solo-T6", world: Solo, address: Address::Chat { task: 6 }, then: Shown },
        first_chat("chat-first-run-2-T1", 2, 1),
        first_chat("chat-first-run-3-T1", 3, 1),
        first_chat("chat-first-run-4-T4", 4, 4),
        first_chat("chat-first-run-7-T1", 7, 1),
        Sample { name: "chats-first-run-5", world: FirstRun { step: 5 }, address: CHATS, then: Shown },
        Sample {
            name: "inbox-reference",
            world: Reference,
            address: Address::Inbox { filter: Filter::ALL },
            then: Shown,
        },
        Sample { name: "accounts-reference", world: Reference, address: Address::Accounts, then: Shown },
        Sample { name: "accounts-first-run-1", world: FirstRun { step: 1 }, address: Address::Accounts, then: Shown },
        Sample { name: "google-sign-in", world: Bare, address: CHATS, then: Then::GoogleSignIn },
        Sample { name: "signed-out", world: Bare, address: CHATS, then: SignedOut },
        Sample { name: "out-of-date", world: Bare, address: CHATS, then: OutOfDate },
        Sample { name: "chats-empty", world: Bare, address: CHATS, then: Shown },
        Sample { name: "inbox-empty", world: Bare, address: Address::Inbox { filter: Filter::ALL }, then: Shown },
        Sample { name: "chat-closing", world: Solo, address: Address::Chat { task: 6 }, then: Closing },
        Sample { name: "chat-rejecting", world: Solo, address: Address::Chat { task: 10 }, then: Rejecting },
        Sample { name: "task-held", world: Solo, address: task(3), then: Held },
        Sample { name: "task-releasing", world: Solo, address: task(3), then: Releasing },
        Sample {
            name: "connect-model-choose",
            world: Reference,
            address: Address::ConnectModel { way: None },
            then: Shown,
        },
        model_sample("connect-model-key-details", temper_client_domain::WayKind::Key, Shown),
        model_sample("connect-model-program-details", temper_client_domain::WayKind::Program, Shown),
        model_sample("connect-model-signing-in", temper_client_domain::WayKind::SignIn, SigningIn),
        model_sample("connect-model-key-ready", temper_client_domain::WayKind::Key, KeyReady),
        model_sample("connect-model-signin-ready", temper_client_domain::WayKind::SignIn, SignInReady),
        model_sample("connect-model-declined", temper_client_domain::WayKind::SignIn, Declined),
        model_sample("connect-model-renewed", temper_client_domain::WayKind::SignIn, Renewed),
        model_sample("connect-model-refused", temper_client_domain::WayKind::SignIn, SignInRefused),
        model_sample("connect-model-key-lost", temper_client_domain::WayKind::Key, AccountRefused),
        Sample { name: "task-reference-T28", world: Reference, address: task(28), then: Shown },
        Sample { name: "task-reference-T24", world: Reference, address: task(24), then: Shown },
        Sample { name: "task-reference-T12", world: Reference, address: task(12), then: Shown },
        Sample { name: "task-solo-T9", world: Solo, address: task(9), then: Shown },
        Sample { name: "forges-github-install", world: FirstRun { step: 20 }, address: Address::Forges, then: Shown },
        owned_sample("forge-admin-details", 23, false),
    ]
}
fn first_chat(name: &'static str, step: u8, task: u64) -> Sample {
    Sample { name, world: World::FirstRun { step }, address: Address::Chat { task }, then: Then::Shown }
}
fn github_sample(name: &'static str, step: u8) -> Sample {
    Sample {
        name,
        world: World::FirstRun { step },
        address: Address::ConnectForge { forge: Some(temper_client_domain::Choice::Forge(2)) },
        then: Then::Shown,
    }
}
fn welcome_sample(name: &'static str, step: temper_client_domain::Step) -> Sample {
    Sample { name, world: World::FirstRun { step: 0 }, address: Address::Welcome { step }, then: Then::Shown }
}
fn owned_sample(name: &'static str, step: u8, ready: bool) -> Sample {
    Sample {
        name,
        world: World::FirstRun { step },
        address: if ready {
            Address::Welcome { step: temper_client_domain::Step::Ready }
        } else {
            Address::ConnectForge { forge: Some(temper_client_domain::Choice::Forge(3)) }
        },
        then: Then::Shown,
    }
}
fn paste_sample(name: &'static str, frame: PasteFrame) -> Sample {
    welcome_model_sample(name, temper_client_domain::WayKind::SignIn, Then::Paste(frame))
}
fn welcome_model_sample(name: &'static str, kind: temper_client_domain::WayKind, then: Then) -> Sample {
    let mut sample = model_sample(name, kind, then);
    if let Address::ConnectModel { way } = sample.address {
        sample.address = Address::Welcome { step: temper_client_domain::Step::Model { way } };
    }
    sample
}
fn model_sample(name: &'static str, kind: temper_client_domain::WayKind, then: Then) -> Sample {
    let way = temper_client_domain::ModelWay {
        source: None,
        provider: if kind == temper_client_domain::WayKind::Program { 17 } else { 18 },
        kind,
    };
    Sample { name, world: World::Reference, address: Address::ConnectModel { way: Some(way) }, then }
}
fn special_mockup(name: &str) -> Option<String> {
    match name {
        "forges-github-install" => return Some("forges.html?world=first-run-github&step=3&bare".to_owned()),
        "forge-admin-details" => {
            return Some("connect-forge.html?world=first-run-elsewhere&step=2&forge=3&bare".to_owned());
        }
        "forge-setup-failed" | "welcome-setup-failed" => {
            return Some("connect-forge.html?world=first-run&step=6&forge=3&bare#other-states".to_owned());
        }
        "welcome-model-key-details" => {
            return Some("welcome.html?world=first-run&step=0&at=model&way=key&bare".to_owned());
        }
        "welcome-model-signing-in" | "welcome-model-signin-ready" => {
            return Some("connect-model.html?world=first-run&step=5&way=chatgpt&bare".to_owned());
        }
        "github-app-details" => {
            return Some("connect-forge.html?world=first-run-github&step=1&forge=2&bare".to_owned());
        }
        "github-install" => return Some("connect-forge.html?world=first-run-github&step=3&forge=2&bare".to_owned()),
        "github-picker" => return Some("connect-forge.html?world=first-run-github&step=4&forge=2&bare".to_owned()),
        "github-adopted" => return Some("connect-forge.html?world=first-run-github&step=5&forge=2&bare".to_owned()),
        "welcome-forge-choose" => return Some("welcome.html?world=first-run&step=0&at=forge&bare".to_owned()),
        "welcome-forge-details" => return Some("welcome.html?world=first-run&step=0&at=forge&forge=3&bare".to_owned()),
        "welcome-model-choose" => return Some("welcome.html?world=first-run&step=0&at=model&bare".to_owned()),
        "welcome-ready-under-way" => return Some("welcome.html?world=first-run-real&step=3&at=ready&bare".to_owned()),
        "welcome-ready-adopted" => return Some("welcome.html?world=first-run-real&step=4&at=ready&bare".to_owned()),
        _ => {}
    }
    None
}
#[must_use]
pub fn mockup(sample: &Sample) -> Option<String> {
    if let Then::Paste(frame) = sample.then {
        return Some(format!(
            "connect-model.html?world=first-run&step=0&way=chatgpt&paste={}&elsewhere=1&bare",
            frame.mockup()
        ));
    }
    if let Some(reference) = special_mockup(sample.name) {
        return Some(reference);
    }
    if sample.then == Then::GoogleSignIn {
        return Some("sign-in.html?world=google-sign-in&bare".to_owned());
    }
    if sample.then != Then::Shown {
        if !matches!(sample.address, Address::ConnectModel { .. }) {
            return None;
        }
        // The mockup draws these endings in the first run after ChatGPT is connected.
        // It draws no sign-in under way, key Ready, or lost key answer.
        let anchor = match sample.then {
            Then::SignInReady => "",
            Then::Declined => "#other-states-declined",
            Then::Renewed => "#other-states-renewed",
            Then::SignInRefused => "#other-states-refused",
            Then::Shown
            | Then::Filled
            | Then::Refused
            | Then::SignedOut
            | Then::GoogleSignIn
            | Then::OutOfDate
            | Then::Closing
            | Then::Rejecting
            | Then::Held
            | Then::Releasing
            | Then::SigningIn
            | Then::Paste(_)
            | Then::AccountRefused
            | Then::KeyReady => return None,
        };
        return Some(format!("connect-model.html?world=first-run&step=5&way=chatgpt&bare{anchor}"));
    }
    let world = match sample.world {
        World::Reference => "reference".to_owned(),
        World::Solo => "solo".to_owned(),
        World::FirstRun { step } => format!("first-run&step={step}"),
        World::Bare => return None,
    };
    let (page, task) = match sample.address {
        Address::Chats { .. } => ("chats", None),
        Address::Chat { task } => ("chat", Some(task)),
        Address::Inbox { .. } => ("inbox", None),
        Address::Accounts => ("accounts", None),
        Address::ConnectModel { .. } => ("connect-model", None),
        Address::Welcome { .. } => ("welcome", None),
        Address::Task { task, .. } => ("task", Some(task)),
        Address::ConnectForge { .. } => ("connect-forge", None),
        Address::Forges => ("forges", None),
        Address::Adopt { .. } => return None,
    };
    let task = match task {
        Some(task) => format!("&task=T{task}"),
        None => String::new(),
    };
    let forge = match sample.address {
        Address::ConnectForge { forge: Some(temper_client_domain::Choice::Forge(number)) } => {
            format!("&forge={number}")
        }
        Address::Inbox { .. }
        | Address::Chats { .. }
        | Address::Accounts
        | Address::ConnectModel { .. }
        | Address::Forges
        | Address::ConnectForge { forge: None | Some(temper_client_domain::Choice::Elsewhere) }
        | Address::Welcome { .. }
        | Address::Chat { .. }
        | Address::Task { .. }
        | Address::Adopt { .. } => String::new(),
    };
    let way = if let Address::ConnectModel { way: Some(way) } = sample.address {
        match way.kind {
            temper_client_domain::WayKind::SignIn => "&way=chatgpt",
            temper_client_domain::WayKind::Key => "&way=key",
            temper_client_domain::WayKind::Program => "&way=program",
        }
    } else {
        ""
    };
    Some(format!("{page}.html?world={world}{task}{forge}{way}&bare"))
}
#[must_use]
pub fn domain(sample: &Sample) -> Domain {
    let (mut root, accounts_watch) = crate::worlds::world(sample.world, sample.address.clone(), sample.then);
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits()));
    match sample.then {
        Then::Shown | Then::Held => {}
        Then::Filled | Then::Refused => adoption(&mut root, sample.then == Then::Refused, &mut out),
        Then::SignedOut | Then::GoogleSignIn => step(&mut root, Event::SignedOut, &mut out),
        Then::OutOfDate => step(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Served { served: false, versions: None } },
            },
            &mut out,
        ),
        Then::Closing => step(&mut root, Event::Act { action: Action::Pages(Act::Close) }, &mut out),
        Then::Rejecting => step(
            &mut root,
            Event::Act {
                action: Action::Pages(Act::Decide {
                    object: c::ObjectKey::Proposal(crate::worlds::PROPOSAL),
                    revision: None,
                    act: c::ActKind::Reject,
                }),
            },
            &mut out,
        ),
        Then::Releasing => step(
            &mut root,
            Event::Act {
                action: Action::Pages(Act::Decide {
                    object: c::ObjectKey::Task(3),
                    revision: Some(c::Revision { value: 7 }),
                    act: c::ActKind::Release,
                }),
            },
            &mut out,
        ),
        Then::SigningIn
        | Then::Paste(_)
        | Then::AccountRefused
        | Then::KeyReady
        | Then::SignInReady
        | Then::Declined
        | Then::Renewed
        | Then::SignInRefused => account(&mut root, sample.then, &mut out),
    }
    if sample.then == Then::Paste(PasteFrame::Expired) {
        let mut doc = temper_client_domain::pages::accounts::document(root.client()).expect("sample accounts").clone();
        doc.signing = skein_lib::List::with_capacity(1);
        doc.signing
            .push(c::OwnerAccountSignIn { source: 43, provider: 18, holder: c::OwnerAccountHolder::Party(73) })
            .expect("one signing");
        crate::driver::deliver(
            &mut root,
            accounts_watch.expect("paste sample accounts watch"),
            c::Delivery::OwnerAccounts(Box::new(doc.clone())),
            &mut out,
        );
        doc.signing.clear();
        doc.cut = false;
        crate::driver::deliver(
            &mut root,
            accounts_watch.expect("paste sample accounts watch"),
            c::Delivery::OwnerAccounts(Box::new(doc)),
            &mut out,
        );
    }
    root
}
fn sent_attempt(out: &mut Queue<Request>) -> Token {
    for _ in 0..out.len() {
        if let Some(request) = out.pop()
            && let Some(down) = crate::driver::down(request)
        {
            match down {
                jig_client::Down::Send { attempt, .. } => return attempt,
                jig_client::Down::Open { .. }
                | jig_client::Down::Close { .. }
                | jig_client::Down::Continue { .. }
                | jig_client::Down::Read { .. }
                | jig_client::Down::Save { .. }
                | jig_client::Down::Erase { .. } => {}
            }
        }
    }
    unreachable!("the sample's request was stored and sent")
}
fn adoption(root: &mut Domain, refused: bool, out: &mut Queue<Request>) {
    use temper_client_domain::forge::{self, Act as A, Text as T, Typed};
    for (field, value, typed) in [
        (T::Forge, "1", Typed::Forge(Some(1))),
        (T::Owner, "acme", Typed::Bytes),
        (T::Name, "shop", Typed::Bytes),
        (T::Prefix, "temper/", Typed::Bytes),
        (T::Landing, "main", Typed::Bytes),
        (T::Checks, "1, 2", Typed::Checks(Some(Box::new([1, 2])))),
    ] {
        step(
            root,
            Event::Act { action: Action::Forge(A::Write { field, text: Box::from(value.as_bytes()), typed }) },
            out,
        );
    }
    step(root, Event::Act { action: Action::Forge(A::Choose { role: forge::Role::Owned }) }, out);
    if refused {
        step(root, Event::Act { action: Action::Forge(A::Adopt) }, out);
        step(root, Event::Client { event: jig_client::Event::Stored }, out);
        let attempt = sent_attempt(out);
        step(
            root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Answer {
                        attempt,
                        envelope: skein_lib::RequestEnvelope::Final,
                        answer: Some(jig_client::Answered::Jig(c::Answer::Refused {
                            refusal: c::Refusal::NotOffered,
                            at: None,
                        })),
                        why: None,
                    },
                },
            },
            out,
        );
    }
}
fn account(root: &mut Domain, then: Then, out: &mut Queue<Request>) {
    use temper_client_domain::pages::{accounts::Way, connect_model::Act as A};
    let refused = then == Then::AccountRefused;
    let key = refused || then == Then::KeyReady;
    let way = if key { Way::Key { provider: 18 } } else { Way::SignIn { provider: 18, plan: true } };
    step(
        root,
        Event::Went {
            place: temper_client_domain::Place::Address(temper_client_domain::pages::connect_model::address(
                root.pages().address(),
                Some(temper_client_domain::pages::connect_model::model_way(way)),
            )),
        },
        out,
    );
    if !key {
        let act = if matches!(then, Then::Paste(_)) {
            A::SignInElsewhere { name: [97; 16], way }
        } else {
            A::SignIn { name: [97; 16], way }
        };
        step(root, Event::Act { action: Action::Pages(Act::ConnectModel(act)) }, out);
    }
    if key {
        step(
            root,
            Event::AccountKey { value: skein_lib::Secret::new(Box::from(b"gallery-example-key".as_slice())) },
            out,
        );
    }
    if key {
        step(root, Event::Act { action: Action::Pages(Act::ConnectModel(A::Submit { name: [97; 16] })) }, out);
    }
    step(root, Event::Client { event: jig_client::Event::Stored }, out);
    let attempt = sent_attempt(out);
    // The tool branch's carried-key refusal: its final answer was lost, so
    // the account form explains that the key must be supplied again.
    let answer = if refused {
        None
    } else if key {
        Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountKeyKept {
            source: 3,
        }))))
    } else {
        Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted {
            source: 43,
            address: Box::from(b"https://issuer.example/visit/gallery-sign-in".as_slice()),
        }))))
    };
    step(
        root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: if refused {
                        skein_lib::RequestEnvelope::Lost
                    } else {
                        skein_lib::RequestEnvelope::Final
                    },
                    answer,
                    why: None,
                },
            },
        },
        out,
    );
    if let Then::Paste(frame) = then {
        paste(root, frame, out);
    }
}
impl PasteFrame {
    fn mockup(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Written => "written",
            Self::TooLong => "too-long",
            Self::Sending => "sending",
            Self::Finishing => "finishing",
            Self::Address => "address",
            Self::State => "state",
            Self::TooMany => "too-many",
            Self::Ended => "not-waiting",
            Self::Expired => "expired",
            Self::Lost => "lost",
            Self::Unsent => "unsent",
        }
    }
}
fn paste(root: &mut Domain, frame: PasteFrame, out: &mut Queue<Request>) {
    use PasteFrame as F;
    use temper_client_domain::pages::connect_model::Act as A;
    if matches!(frame, F::Waiting | F::Expired) {
        return;
    }
    let value = if frame == F::TooLong {
        vec![
            b'x';
            usize::try_from(
                temper_client_domain::pages::accounts::current_me(root.client()).expect("sample me").redirect_bytes
            )
            .expect("redirect bound fits")
                + 1
        ]
    } else {
        b"gallery-private-address".to_vec()
    };
    step(root, Event::SignInAddress { value: skein_lib::Secret::new(value.into_boxed_slice()) }, out);
    if matches!(frame, F::Written | F::TooLong) {
        return;
    }
    step(root, Event::Act { action: Action::Pages(Act::ConnectModel(A::Finish { name: [98; 16] })) }, out);
    step(root, Event::Client { event: jig_client::Event::Stored }, out);
    let attempt = sent_attempt(out);
    if frame == F::Sending {
        return;
    }
    let envelope = match frame {
        F::Lost => skein_lib::RequestEnvelope::Lost,
        F::Unsent => skein_lib::RequestEnvelope::Again,
        F::Waiting
        | F::Written
        | F::TooLong
        | F::Sending
        | F::Finishing
        | F::Address
        | F::State
        | F::TooMany
        | F::Ended
        | F::Expired => skein_lib::RequestEnvelope::Final,
    };
    let answer = match frame {
        F::Finishing => {
            Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountFinishing {
                source: 43,
            }))))
        }
        F::Address | F::State | F::TooMany | F::Ended => {
            let why = match frame {
                F::Address => c::SignInRedirectWhy::Address,
                F::State => c::SignInRedirectWhy::State,
                F::TooMany => c::SignInRedirectWhy::TooMany,
                F::Ended => c::SignInRedirectWhy::NotWaiting,
                F::Waiting | F::Written | F::TooLong | F::Sending | F::Finishing | F::Expired | F::Lost | F::Unsent => {
                    unreachable!("only refusal frames select a redirect refusal")
                }
            };
            Some(jig_client::Answered::Jig(c::Answer::Refused {
                refusal: c::Refusal::SignInRedirect { why },
                at: None,
            }))
        }
        F::Waiting | F::Written | F::TooLong | F::Sending | F::Expired | F::Lost | F::Unsent => None,
    };
    step(
        root,
        Event::Client {
            event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt, envelope, answer, why: None } },
        },
        out,
    );
}
