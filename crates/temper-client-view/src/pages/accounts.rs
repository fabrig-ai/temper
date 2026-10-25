//! Whole supplied account lists, with no local account or source policy (client/README.md, 5.5).
use crate::{
    accounts::{
        Bindings, Meaning,
        words::{self, Word as W},
    },
    words::Words,
};
use alloc::boxed::Box;
mod rows;
use jig_client::contract as c;
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::{
    Domain,
    pages::accounts::{self, Broken},
};
pub(super) fn run(b: &mut Builder, w: &Words, word: W) {
    b.run(Run::Plain, &words::say(w, word, &[]));
}
pub(super) fn number(b: &mut Builder, w: &Words, value: u64) {
    b.run(Run::Plain, &words::say(w, W::Number, &[Hole::Number(value)]));
}
fn text(b: &mut Builder, w: &Words, role: Role, word: W) {
    crate::cards::text(b, role, &words::say(w, word, &[]));
}
fn done(b: &mut Builder, w: &Words, word: W) {
    let value = words::say(w, word, &[]);
    b.open(Role::Status);
    b.style(crate::look::STATUS_DONE);
    b.name(&value);
    b.text(Run::Plain, &value);
    b.close();
}
fn fact(b: &mut Builder, w: &Words, word: W, value: u64) {
    b.open(Role::Paragraph);
    b.open(Role::Text);
    run(b, w, word);
    number(b, w, value);
    b.close();
    b.close();
}
fn control(
    b: &mut Builder,
    w: &Words,
    bindings: &mut Bindings,
    word: W,
    meaning: Meaning,
    disabled: bool,
    pressed: bool,
) {
    named_control(b, bindings, &words::say(w, word, &[]), meaning, disabled, pressed);
}
fn named_control(
    b: &mut Builder,
    bindings: &mut Bindings,
    name: &[u8],
    meaning: Meaning,
    disabled: bool,
    pressed: bool,
) {
    let model = match &meaning {
        Meaning::Model(_) => true,
        Meaning::Action(_) | Meaning::Go(_) | Meaning::Key | Meaning::SignInAddress | Meaning::Program => false,
    };
    let Some(binding) = bindings.insert(meaning) else { return };
    b.open(Role::Button);
    if model {
        b.style(crate::look::FLOW_SECONDARY);
    }
    b.name(name);
    b.bind(binding);
    if pressed {
        b.state(State::Pressed);
    }
    if disabled {
        b.state(State::Disabled);
    }
    b.close();
}
fn kind(b: &mut Builder, w: &Words, value: c::OwnerAccountKind) {
    run(
        b,
        w,
        match value {
            c::OwnerAccountKind::SignIn => W::SignIn,
            c::OwnerAccountKind::Key => W::Key,
            c::OwnerAccountKind::Program => W::Program,
            c::OwnerAccountKind::None => W::None,
        },
    );
}
fn named(w: &Words, word: W, name: &[u8]) -> Box<[u8]> {
    words::say(w, word, &[Hole::Name(crate::choice::shown(name, w))])
}
fn provider_name(name: Option<&c::DocumentText>) -> Option<&[u8]> {
    let name = name?;
    if name.cut || name.bytes().is_empty() { None } else { Some(name.bytes()) }
}
#[expect(clippy::too_many_arguments, reason = "party and deployment rows share these borrowed facts")]
fn heading(
    w: &Words,
    number_value: u32,
    provider: Option<u32>,
    name: Option<&[u8]>,
    display: &c::DocumentText,
    held: c::OwnerAccountHolder,
    plan: bool,
    sources: &skein_lib::List<c::OwnerAccountSource>,
    me: Option<&c::Me>,
) -> Box<[u8]> {
    let separator = words::say(w, W::Separator, &[]);
    let Some(name) = name else {
        let account = words::say(w, W::Account, &[]);
        let number = words::say(w, W::Number, &[Hole::Number(u64::from(number_value))]);
        let provider_label = words::say(w, W::Provider, &[]);
        let provider_number = match provider {
            Some(n) => words::say(w, W::Number, &[Hole::Number(u64::from(n))]),
            None => Box::new([]),
        };
        return words::join(&[
            &account,
            &number,
            if provider.is_some() { &separator } else { b"" },
            if provider.is_some() { &provider_label } else { b"" },
            &provider_number,
            if display.bytes().is_empty() { b"" } else { &separator },
            display.bytes(),
        ]);
    };
    let (brand, plan_name) = words::provider_words(name);
    let mut program = false;
    let mut program_name = None;
    let mut sign_in = false;
    for source in sources {
        program |= source.kind == c::OwnerAccountKind::Program;
        if source.kind == c::OwnerAccountKind::Program && program_name.is_none() {
            program_name = source.name.as_deref();
        }
        sign_in |= source.kind == c::OwnerAccountKind::SignIn;
    }
    let label = if program {
        named(w, W::Subscription, plan_name)
    } else if plan || sign_in {
        crate::choice::shown(if plan { plan_name } else { brand }, w)
    } else {
        named(w, W::ApiKeyBrand, brand)
    };
    let owner = match held {
        c::OwnerAccountHolder::Deployment => words::say(w, W::HeldDeployment, &[]),
        c::OwnerAccountHolder::Party(_) => match me {
            Some(me) => named(w, W::HeldBy, me.party.name.bytes()),
            None => named(w, W::HeldBy, &words::say(w, W::OwnHolder, &[])),
        },
    };
    let display = if program {
        program_name.unwrap_or(display.bytes())
    } else if plan || sign_in {
        display.bytes()
    } else {
        b""
    };
    let brand = crate::choice::shown(brand, w);
    words::join(&[
        &label,
        if display.is_empty() { b"" } else { &separator },
        display,
        if program || plan || sign_in { &separator } else { b"" },
        if program || plan || sign_in { &brand } else { b"" },
        &separator,
        &owner,
    ])
}
pub(crate) fn unloaded(b: &mut Builder, w: &Words, at: c::OwnerAccountPlace, why: c::OwnerAccountUnread) {
    run(b, w, W::Unloaded);
    run(
        b,
        w,
        match at {
            c::OwnerAccountPlace::Directory => W::Directory,
            c::OwnerAccountPlace::File => W::File,
        },
    );
    b.run(Run::Plain, b": ");
    run(
        b,
        w,
        match why {
            c::OwnerAccountUnread::Unreadable => W::Unreadable,
            c::OwnerAccountUnread::Refused => W::Refused,
            c::OwnerAccountUnread::Failed => W::Failed,
            c::OwnerAccountUnread::Stalled => W::Stalled,
            c::OwnerAccountUnread::TooLarge => W::TooLarge,
        },
    );
}
fn state(b: &mut Builder, w: &Words, source: &c::OwnerAccountSource) {
    match source.state {
        c::OwnerAccountState::Moving => run(b, w, W::Moving),
        c::OwnerAccountState::Later { until } => {
            run(b, w, W::Limited);
            if let Some(at) = until {
                b.run(Run::Plain, &words::say(w, W::Until, &[Hole::Wall(at)]));
            }
        }
        c::OwnerAccountState::Unloaded { at, why } => unloaded(b, w, at, why),
        c::OwnerAccountState::Asking => run(b, w, W::Asking),
        c::OwnerAccountState::Granted => run(b, w, if source.usable { W::Ready } else { W::Granted }),
        c::OwnerAccountState::SignInAgain => run(b, w, W::NeedsSignIn),
        c::OwnerAccountState::Misconfigured => run(b, w, W::Misconfigured),
        c::OwnerAccountState::Cancelled => run(b, w, W::Cancelled),
        c::OwnerAccountState::Closing => run(b, w, W::Closing),
    }
    if source.expiring {
        b.run(Run::Plain, b" \xc2\xb7 ");
        run(b, w, W::Expiring);
    }
    if source.attention {
        b.run(Run::Plain, b" \xc2\xb7 ");
        run(b, w, W::Attention);
    }
    if let Some(at) = source.spent_until {
        b.run(Run::Plain, b" \xc2\xb7 ");
        run(b, w, W::Spent);
        b.run(Run::Plain, b" \xc2\xb7 ");
        b.run(Run::Plain, &words::say(w, W::UsageUntil, &[Hole::Wall(at)]));
    }
}
pub(crate) fn end(b: &mut Builder, w: &Words, end: &c::OwnerAccountEnd, rows: &skein_lib::List<c::OwnerAccountLine>) {
    b.open(Role::Status);
    b.open(Role::Text);
    match end {
        c::OwnerAccountEnd::Made { account, declined } | c::OwnerAccountEnd::Renewed { account, declined } => {
            let (word, fallback) = if let c::OwnerAccountEnd::Renewed { .. } = end {
                (W::RenewedNamed, W::Renewed)
            } else {
                (W::MadeNamed, W::Made)
            };
            if !named_end(b, w, rows, *account, word) {
                run(b, w, fallback);
                number(b, w, u64::from(*account));
            }
            if *declined {
                run(b, w, W::Declined);
            }
        }
        c::OwnerAccountEnd::Refused { refusal } => refused(b, w, *refusal),
        c::OwnerAccountEnd::Failed { failure } => match failure {
            c::OwnerAccountFailure::Later { after } => {
                run(b, w, W::Later);
                if let Some(after) = after {
                    b.run(Run::Plain, &words::say(w, W::After, &[Hole::Span(*after)]));
                }
            }
            c::OwnerAccountFailure::SignInAgain => run(b, w, W::NeedsSignIn),
            c::OwnerAccountFailure::MissingScope { scope } => {
                run(b, w, W::Scope);
                number(b, w, u64::from(*scope));
            }
            c::OwnerAccountFailure::Misconfigured => run(b, w, W::Misconfigured),
            c::OwnerAccountFailure::Unloaded { at, why } => unloaded(b, w, *at, *why),
            c::OwnerAccountFailure::Cancelled => run(b, w, W::Cancelled),
        },
    }
    b.close();
    b.close();
}
fn named_end(b: &mut Builder, w: &Words, rows: &skein_lib::List<c::OwnerAccountLine>, account: u32, word: W) -> bool {
    for row in rows {
        if row.number != account {
            continue;
        }
        let Some(name) = provider_name(row.provider_name.as_ref()) else { return false };
        let (_, plan) = words::provider_words(name);
        if row.display.bytes().is_empty() {
            if word == W::RenewedNamed {
                return false;
            }
            b.run(Run::Plain, &words::say(w, W::MadePlan, &[Hole::Name(skein_lib::bytes::copy_of(plan))]));
            return true;
        }
        let holes = [
            Hole::Name(skein_lib::bytes::copy_of(plan)),
            Hole::Name(skein_lib::bytes::copy_of(row.display.bytes())),
            Hole::Name(skein_lib::bytes::copy_of(plan)),
        ];
        let holes = if word == W::RenewedNamed { &holes } else { holes.get(..2).expect("made has two holes") };
        b.run(Run::Plain, &words::say(w, word, holes));
        return true;
    }
    false
}
fn refused(b: &mut Builder, w: &Words, refusal: c::OwnerAccountRefusal) {
    use c::OwnerAccountRefusal as R;
    let word = match refusal {
        R::NotSignedIn => W::NotSignedIn,
        R::NotOurs => W::NotOurs,
        R::Cancelled => W::MoveCancelled,
        R::Moving => W::Moving,
        R::NotLimited => W::NotLimited,
        R::Uses => W::UsesRefused,
        R::NotKept { why } => match why {
            c::OwnerAccountKeyFailure::TooLong => W::KeyLong,
            c::OwnerAccountKeyFailure::NotStored => W::NotStored,
        },
        R::Limit { bound } => match bound {
            c::OwnerAccountBound::Accounts => W::LimitAccounts,
            c::OwnerAccountBound::HolderAccounts => W::LimitHolder,
            c::OwnerAccountBound::Sources => W::LimitSources,
            c::OwnerAccountBound::SignIns => W::LimitSignIns,
        },
        R::Held { provider } | R::Provider { provider } => {
            run(b, w, if let R::Held { .. } = refusal { W::Held } else { W::ProviderRefused });
            number(b, w, u64::from(provider));
            return;
        }
        R::Subject => W::Subject,
        R::Unknown => W::UnknownNumber,
        R::Numbers => W::Numbers,
        R::Kind { kind: value } => {
            run(b, w, W::Kind);
            kind(b, w, value);
            return;
        }
        R::Closing => W::Closing,
        R::NotStarted => W::NotStarted,
    };
    run(b, w, word);
}
pub(crate) fn main(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings, text_per_node: u32) {
    b.open(Role::Main);
    b.style(crate::look::MAIN);
    b.style(crate::look::PAGE);
    b.open(Role::Slot);
    b.style(crate::look::ACCOUNTS_PAGE);
    b.open(Role::Slot);
    b.style(crate::look::PAGEBAR);
    text(b, w, Role::Heading { level: 1 }, W::Accounts);
    b.close();
    // The heading's row, as the inbox's: the heading, its count, and the link to connect one.
    b.open(Role::Slot);
    b.style(crate::look::HEADING);
    text(b, w, Role::Heading { level: 2 }, W::Own);
    let mut count = 0_u64;
    if let Some(doc) = accounts::document(root.client()) {
        for row in &doc.accounts {
            if !rows::demo(root, row.provider) {
                count = count.checked_add(1).expect("received accounts fit");
            }
        }
        b.open(Role::Paragraph);
        b.style(crate::look::SUBTLE_COUNT);
        let said = words::say(w, W::Number, &[Hole::Number(count)]);
        b.name(&said);
        b.text(Run::Plain, &said);
        b.close();
        connect(b, w, bindings, W::ConnectAdd);
    }
    crate::cards::text(b, Role::Paragraph, &w.say(crate::words::Word::AccountLead, &[]));
    b.close();
    if let Some(doc) = accounts::document(root.client()) {
        if count == 0 {
            // A block of their own, so that the link keeps to the page's column.
            b.open(Role::Slot);
            b.style(crate::look::EMPTY);
            text(b, w, Role::Paragraph, W::NoOwn);
            connect(b, w, bindings, W::Connect);
            b.close();
        }
        let mut shown = 0;
        for row in &doc.accounts {
            if rows::demo(root, row.provider) || shown >= bindings.counts().accounts {
                continue;
            }
            rows::own(b, root, w, bindings, row, text_per_node);
            shown = shown.checked_add(1).expect("displayed rows fit");
        }
        omitted(b, w, W::AccountsOmitted, u32::try_from(count).expect("received rows fit"), bindings.counts().accounts);
        signings_without_rows(b, root, w, doc, bindings.counts());
        if doc.cut {
            text(b, w, Role::Status, W::Cut);
        }
    }
    if let Some(page) = root.pages().accounts()
        && is_owner(root)
        && let Some(doc) = page.deployment(root.client())
        && !doc.accounts.is_empty()
    {
        text(b, w, Role::Heading { level: 2 }, W::Deployment);
        {
            for row in doc.accounts.iter().take(usize::try_from(bindings.counts().accounts).expect("u32 fits")) {
                rows::deployment(b, root, w, bindings, row, text_per_node);
            }
            omitted(b, w, W::AccountsOmitted, doc.accounts.len(), bindings.counts().accounts);
            if doc.cut {
                text(b, w, Role::Status, W::Cut);
            }
        }
    }
    if let Some((_, halves)) = root.demo()
        && halves.model
    {
        text(b, w, Role::Heading { level: 2 }, W::Playground);
        b.open(Role::Disclosure);
        b.style(crate::look::ACCOUNT_ROW);
        b.name(&words::say(w, W::PlaygroundAccount, &[]));
        text(b, w, Role::Paragraph, W::PlaygroundWords);
        b.close();
    }
    b.close();
    b.close();
}
fn signings_without_rows(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    doc: &c::OwnerAccounts,
    counts: temper_client_domain::limits::AccountLimits,
) {
    for signing in doc.signing.iter().take(usize::try_from(counts.sign_ins).expect("bounded sign-ins")) {
        let mut in_row = false;
        let mut shown = 0_u32;
        for row in &doc.accounts {
            if rows::demo(root, row.provider) || shown >= counts.accounts {
                continue;
            }
            shown = shown.checked_add(1).expect("displayed rows fit");
            for source in &row.sources {
                in_row |= source.number == signing.source;
            }
        }
        if in_row {
            continue;
        }
        b.open(Role::Status);
        b.open(Role::Text);
        run(b, w, W::Signing);
        number(b, w, u64::from(signing.source));
        b.run(Run::Plain, b" \xc2\xb7 ");
        run(b, w, W::Provider);
        number(b, w, u64::from(signing.provider));
        b.run(Run::Plain, b" \xc2\xb7 ");
        match signing.holder {
            c::OwnerAccountHolder::Deployment => run(b, w, W::Deployment),
            c::OwnerAccountHolder::Party(party) => {
                run(b, w, W::Party);
                number(b, w, party);
            }
        }
        b.close();
        b.close();
    }
    omitted(b, w, W::SignInsOmitted, doc.signing.len(), counts.sign_ins);
}
fn connect(b: &mut Builder, w: &Words, bindings: &mut Bindings, word: W) {
    if let Some(to) = bindings.insert(Meaning::Go(temper_client_domain::Address::ConnectModel { way: None })) {
        let name = words::say(w, word, &[]);
        crate::flow::control(
            b,
            if word == W::Connect {
                crate::flow::Control::PrimaryLink { name: &name, to }
            } else {
                crate::flow::Control::SecondaryLink { name: &name, to }
            },
        );
    }
}
pub(super) fn key_field(
    b: &mut Builder,
    w: &Words,
    bindings: &mut Bindings,
    form: &accounts::request::Request,
    busy: bool,
) {
    let Some(binding) = bindings.insert(Meaning::Key) else { return };
    b.open(Role::TextField);
    b.name(&words::say(w, W::KeyField, &[]));
    b.entry(skein_ui::EntryKind::Secret);
    b.value(b"", form.key_written());
    b.bind(binding);
    if busy {
        b.state(State::Disabled);
    }
    b.close();
}
#[expect(clippy::too_many_lines, reason = "one request outcome includes its sign-in address and shared paste")]
pub(super) fn outcome(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    form: &accounts::request::Request,
    text_per_node: u32,
) {
    if let Some(broken) = form.broken {
        text(
            b,
            w,
            Role::Alert,
            match broken {
                Broken::Empty => W::KeyEmpty,
                Broken::TooLong => W::KeyLong,
                Broken::Selection => W::Selection,
                Broken::Rank => W::Rank,
                Broken::ValueAgain => W::ValueAgain,
                Broken::ProgramName => W::ProgramInvalid,
            },
        );
    }
    if let Some(pending) = &form.pending {
        match &pending.outcome {
            None => text(b, w, Role::Status, W::InFlight),
            Some(jig_client::Outcome::Lost) => text(b, w, Role::Alert, W::Lost),
            Some(jig_client::Outcome::Unsent) => text(b, w, Role::Alert, W::Unsent),
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountSignedOut { confirmed, failed_hosts },
            ))))) => {
                if *confirmed {
                    done(b, w, W::Confirmed);
                } else {
                    text(b, w, Role::Status, W::Unconfirmed);
                }
                super::account_programs::failures(b, w, failed_hosts, text_per_node);
            }
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountRemoved { confirmed, failed_hosts },
            ))))) => {
                if *confirmed {
                    done(b, w, W::Removed);
                } else {
                    text(b, w, Role::Status, W::RemovedUnconfirmed);
                }
                super::account_programs::failures(b, w, failed_hosts, text_per_node);
            }
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountKeyKept { source },
            ))))) => fact(b, w, W::Saved, u64::from(*source)),
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountProgramDeclared { source, name },
            ))))) => {
                fact(b, w, W::Source, u64::from(*source));
                crate::cards::text(b, Role::Paragraph, &named(w, W::ProgramCommand, name));
            }
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused { refusal, .. }))) => {
                crate::cards::refusal(b, w, refusal);
            }
            Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(outcome)))) => {
                crate::cards::outcome(b, w, outcome);
            }
            Some(
                jig_client::Outcome::Answered(jig_client::Answered::Family(_))
                | jig_client::Outcome::NotKept
                | jig_client::Outcome::Held
                | jig_client::Outcome::Unknown
                | jig_client::Outcome::Limit
                | jig_client::Outcome::Unconfirmed,
            ) => text(b, w, Role::Status, W::Unknown),
        }
    }
    if let Some(address) = form.sign_in_address(root.client()) {
        let elsewhere = match root.pages().connect_model() {
            Some(page) => page.elsewhere(),
            None => false,
        };
        if !elsewhere {
            text(b, w, Role::Paragraph, W::Address);
        }
        if skein_ui::markdown::safe_url(address) {
            let label = if elsewhere {
                named(w, W::OpenPlanSignIn, paste_plan(root, form))
            } else {
                words::say(w, W::OpenSignIn, &[])
            };
            let bytes = u64::try_from(label.len())
                .expect("word length fits")
                .saturating_add(u64::try_from(address.len()).expect("address length fits"));
            if bytes <= u64::from(text_per_node) {
                b.open(Role::Link);
                if root.pages().connect_model().is_some() {
                    b.style(crate::look::FLOW_QUIET);
                    b.style(crate::look::MODEL_SIGN_IN);
                }
                b.text(Run::Plain, &label);
                b.external(address);
                b.close();
            } else {
                b.open(Role::Alert);
                b.text(
                    Run::Plain,
                    &words::say(
                        w,
                        W::AddressLong,
                        &[Hole::Number(u64::try_from(address.len()).expect("address length fits"))],
                    ),
                );
                b.close();
            }
        } else {
            text(b, w, Role::Alert, W::UnsafeAddress);
        }
        paste(b, root, w, bindings, form, !elsewhere);
    } else if form.paste_state(root.client()) == accounts::PasteState::Ended {
        paste(b, root, w, bindings, form, false);
    } else if form.returned() && form.paste_state(root.client()) != accounts::PasteState::Closed {
        // A departed sign-in's address come back to by hand while it still waits (client/README.md, 5.5).
        text(b, w, Role::Paragraph, W::PasteHistory);
        paste(b, root, w, bindings, form, false);
    }
}
fn is_owner(root: &Domain) -> bool {
    match accounts::current_me(root.client()) {
        Some(me) => me.deployment_role == Some(c::HeldRole::Owner),
        None => false,
    }
}

pub(super) fn omitted(b: &mut Builder, w: &Words, word: W, actual: u32, limit: u32) {
    if actual > limit {
        fact(b, w, word, u64::from(actual.saturating_sub(limit)));
    }
}
fn confirmation(b: &mut Builder, root: &Domain, w: &Words, form: &accounts::Accounts, ask: &c::Ask) {
    let (held, account, source, word) = if let c::Ask::SignOut { holder, source } = ask {
        (*holder, source_account(root, form, *holder, *source), Some(*source), W::ConfirmSignOut)
    } else if let c::Ask::AccountChange { holder, change: c::AccountChange::RemoveKey { source } } = ask {
        (*holder, source_account(root, form, *holder, *source), Some(*source), W::ConfirmKey)
    } else if let c::Ask::AccountChange { holder, change: c::AccountChange::Remove { account } } = ask {
        (*holder, Some(*account), None, W::ConfirmAccount)
    } else {
        return;
    };
    b.open(Role::Alert);
    b.open(Role::Text);
    run(b, w, word);
    b.run(Run::Plain, b" \xc2\xb7 ");
    run(b, w, if held == c::AccountHolder::Own { W::OwnHolder } else { W::DeploymentHolder });
    b.run(Run::Plain, b" \xc2\xb7 ");
    if let Some(account) = account {
        run(b, w, W::Account);
        number(b, w, u64::from(account));
    } else {
        run(b, w, W::UnknownAccount);
    }
    if let Some(source) = source {
        b.run(Run::Plain, b" \xc2\xb7 ");
        run(b, w, W::Source);
        number(b, w, u64::from(source));
    }
    b.close();
    b.close();
}
// One confirmation names its source's account even when that row is beyond the display cap.
// Lookup allocates nothing; its input remains bounded by the admitted document.
fn source_account(root: &Domain, form: &accounts::Accounts, holder: c::AccountHolder, source: u32) -> Option<u32> {
    if holder == c::AccountHolder::Own {
        for row in &accounts::document(root.client())?.accounts {
            for entry in &row.sources {
                if entry.number == source {
                    return Some(row.number);
                }
            }
        }
    } else {
        for row in &form.deployment(root.client())?.accounts {
            for entry in &row.sources {
                if entry.number == source {
                    return Some(row.number);
                }
            }
        }
    }
    None
}

/// An account's heading as the accounts page draws it (5.5, An account).
pub(crate) fn account_heading(root: &Domain, w: &Words, row: &c::OwnerAccountLine) -> Box<[u8]> {
    let supplied = provider_name(row.provider_name.as_ref()).unwrap_or(b"");
    let shown = crate::choice::provider_shown(root, row.provider, supplied, w);
    let name = if supplied.is_empty() { None } else { Some(shown.as_ref()) };
    heading(
        w,
        row.number,
        row.provider,
        name,
        &row.display,
        row.holder,
        row.plan,
        &row.sources,
        accounts::current_me(root.client()),
    )
}

/// The welcome names the same first usable account the accounts page draws.
#[expect(clippy::manual_map, reason = "step code matches optional facts without closures")]
pub(crate) fn first_usable_heading(root: &Domain, w: &Words) -> Option<Box<[u8]>> {
    let document = accounts::document(root.client())?;
    let provider = match root.forge().demo() {
        Some(demo) => Some(demo.provider),
        None => None,
    };
    for row in &document.accounts {
        if !temper_client_domain::forge::demo::usable_account(row, provider) {
            continue;
        }
        return Some(account_heading(root, w, row));
    }
    let demo = root.forge().demo()?;
    let deployment = temper_client_domain::forge::demo::owner_deployment(root.forge(), root.client())?;
    let account = temper_client_domain::forge::demo::deployment_model(deployment, demo.provider)?;
    let supplied = provider_name(account.provider_name.as_ref()).unwrap_or(b"");
    let shown = crate::choice::provider_shown(root, account.provider, supplied, w);
    let name = if supplied.is_empty() { None } else { Some(shown.as_ref()) };
    Some(heading(
        w,
        account.number,
        account.provider,
        name,
        &account.display,
        account.holder,
        account.plan,
        &account.sources,
        accounts::current_me(root.client()),
    ))
}

/// Ready names an own sign-in with the same provider words as its offered way.
pub(crate) fn own_signing_name(root: &Domain, w: &Words) -> Option<Box<[u8]>> {
    let document = accounts::document(root.client())?;
    let me = accounts::current_me(root.client())?;
    for signing in &document.signing {
        if signing.holder != c::OwnerAccountHolder::Party(me.party.number) {
            continue;
        }
        for offer in &me.providers {
            if offer.provider == signing.provider {
                let (brand, plan) = words::provider_words(offer.name.bytes());
                return Some(crate::choice::shown(if offer.signed_in_plan { plan } else { brand }, w));
            }
        }
        let label = words::say(w, W::Provider, &[]);
        let number = words::say(w, W::Number, &[Hole::Number(u64::from(signing.provider))]);
        return Some(words::join(&[&label, &number]));
    }
    None
}

fn paste_way(root: &Domain, form: &accounts::request::Request) -> Option<accounts::Way> {
    if let Some(pending) = &form.pending
        && let c::Ask::SignIn { provider, .. } = pending.ask
    {
        for way in &accounts::ways(
            &accounts::current_me(root.client())?.providers,
            root.forge().demo_provider(),
            c::AccountHolder::Own,
        ) {
            if let accounts::Way::SignIn { provider: number, .. } = *way
                && number == provider
            {
                return Some(*way);
            }
        }
    }
    temper_client_domain::pages::connect_model::way(root.pages(), root.client())
}
fn paste_plan<'a>(root: &'a Domain, form: &accounts::request::Request) -> &'a [u8] {
    if let Some(way) = paste_way(root, form)
        && let Some(me) = accounts::current_me(root.client())
    {
        for offer in &me.providers {
            if offer.provider == way.provider() {
                return words::provider_words(offer.name.bytes()).1;
            }
        }
    }
    b""
}
fn paste_restart(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    form: &accounts::request::Request,
) {
    let Some(way) = paste_way(root, form) else { return };
    let Some(me) = accounts::current_me(root.client()) else { return };
    let ways = accounts::ways(&me.providers, root.forge().demo_provider(), c::AccountHolder::Own);
    let meaning = if root.pages().connect_model().is_some() {
        Meaning::Model(temper_client_domain::pages::connect_model::Act::SignIn {
            name: root.pages().account_name(),
            way,
        })
    } else if let Some(page) = root.pages().accounts()
        && let Some(row) = page.request_row
    {
        Meaning::Action(accounts::Act::Submit {
            name: root.pages().account_name(),
            row,
            operation: accounts::Operation::SignIn { provider: way.provider() },
        })
    } else {
        return;
    };
    named_control(b, bindings, &super::connect_model::label(me, way, &ways, w), meaning, false, false);
}
/// The wrapper is separate so the elsewhere entry point reuses the same body.
pub(super) fn paste(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    form: &accounts::request::Request,
    folded: bool,
) {
    if form.paste_state(root.client()) == accounts::PasteState::Closed {
        return;
    }
    if folded {
        b.open(Role::Disclosure);
        if root.pages().connect_model().is_some() {
            b.style(crate::look::MODEL_SIGN_IN);
        }
        b.name(&words::say(w, W::PasteDisclosure, &[]));
        text(b, w, Role::Paragraph, W::PasteHistory);
    }
    paste_body(b, root, w, bindings, form);
    if folded {
        b.close();
    }
}
fn paste_body(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings, form: &accounts::request::Request) {
    use accounts::PasteState as S;
    let state = form.paste_state(root.client());
    let alert = match state {
        S::Closed | S::Refused { why: c::SignInRedirectWhy::NotWaiting } => return,
        S::Finishing => {
            text(b, w, Role::Status, W::PasteFinishing);
            return;
        }
        S::Ended => {
            text(b, w, Role::Status, W::PasteEnded);
            paste_restart(b, root, w, bindings, form);
            return;
        }
        S::Refused { why: c::SignInRedirectWhy::TooMany } => {
            crate::cards::text(b, Role::Alert, &named(w, W::PasteTooMany, paste_plan(root, form)));
            paste_restart(b, root, w, bindings, form);
            return;
        }
        S::Refused { why: c::SignInRedirectWhy::Address } => Some(W::PasteAddress),
        S::Refused { why: c::SignInRedirectWhy::State } => Some(W::PasteState),
        S::TooLong => Some(W::PasteLong),
        S::Again => Some(W::PasteAgain),
        S::Waiting { .. } | S::Sending => None,
    };
    crate::cards::text(b, Role::Paragraph, &named(w, W::PasteInstructions, paste_plan(root, form)));
    if let Some(word) = alert {
        text(b, w, Role::Alert, word);
    }
    if let Some(binding) = bindings.insert(Meaning::SignInAddress) {
        b.open(Role::TextField);
        if root.pages().connect_model().is_some() {
            b.style(crate::look::FLOW_FIELD);
        }
        b.name(&words::say(w, W::PasteField, &[]));
        b.entry(skein_ui::EntryKind::Secret);
        b.value(b"", form.paste.written());
        b.bind(binding);
        if state == S::Sending {
            b.state(State::Disabled);
        }
        b.close();
    }
    let meaning = if root.pages().connect_model().is_some() {
        Meaning::Model(temper_client_domain::pages::connect_model::Act::Finish { name: root.pages().account_name() })
    } else {
        Meaning::Action(accounts::Act::Finish { name: root.pages().account_name() })
    };
    control(
        b,
        w,
        bindings,
        if state == S::Sending { W::PasteSending } else { W::PasteFinish },
        meaning,
        state != (S::Waiting { ready: true }),
        false,
    );
}
