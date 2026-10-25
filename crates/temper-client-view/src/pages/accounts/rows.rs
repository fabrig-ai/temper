//! Folded account rows; every control carries the row that offered it.
use super::{
    confirmation, heading, key_field, named, named_control, omitted, outcome, provider_name, run, state, text,
};
use crate::{
    accounts::{
        Bindings, Meaning,
        words::{self, Word as W},
    },
    words::Words,
};
use alloc::boxed::Box;
use jig_client::contract as c;
use skein_lib::List;
use skein_ui::{Builder, Role, Run};
use temper_client_domain::{
    Domain,
    pages::accounts::{self, Act, Operation, Row, SignInState, Way},
};

pub(super) fn demo(root: &Domain, provider: Option<u32>) -> bool {
    match root.forge().demo() {
        Some(demo) => provider == Some(demo.provider),
        None => false,
    }
}
pub(super) fn own(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    account: &c::OwnerAccountLine,
    text_bytes: u32,
) {
    draw(
        b,
        root,
        w,
        bindings,
        Row { holder: c::AccountHolder::Own, account: account.number },
        account.place,
        account.provider,
        account.provider_name.as_ref(),
        &account.display,
        account.holder,
        account.plan,
        account.uses.as_ref(),
        &account.sources,
        account.manage_usage.as_deref(),
        None,
        None,
        account.cut,
        text_bytes,
    );
}
pub(super) fn deployment(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    account: &c::OwnerAccount,
    text_bytes: u32,
) {
    draw(
        b,
        root,
        w,
        bindings,
        Row { holder: c::AccountHolder::Deployment, account: account.number },
        account.place,
        account.provider,
        account.provider_name.as_ref(),
        &account.display,
        account.holder,
        account.plan,
        account.uses.as_ref(),
        &account.sources,
        account.manage_usage.as_deref(),
        Some(&account.runs),
        Some(&account.waiting),
        account.cut,
        text_bytes,
    );
}
#[expect(
    clippy::too_many_arguments,
    reason = "borrow common fields of the two contract account types without copying documents"
)]
#[expect(clippy::too_many_lines, reason = "one row in document order, with its own sources, facts, acts and outcome")]
fn draw(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    row: Row,
    place: u32,
    provider: Option<u32>,
    provider_text: Option<&c::DocumentText>,
    display: &c::DocumentText,
    holder: c::OwnerAccountHolder,
    plan: bool,
    uses: Option<&List<c::OwnerAccountUse>>,
    sources: &List<c::OwnerAccountSource>,
    manage: Option<&[u8]>,
    runs: Option<&List<c::Chip>>,
    waiting: Option<&List<c::Chip>>,
    cut: bool,
    text_bytes: u32,
) {
    if demo(root, provider) {
        return;
    }
    let name = provider_name(provider_text);
    let title =
        heading(w, row.account, provider, name, display, holder, plan, sources, accounts::current_me(root.client()));
    let deciding = sources.get(0);
    let mut moving = false;
    for source in sources {
        moving |= source.state == c::OwnerAccountState::Moving;
    }
    let pays = pays(w, plan, uses, bindings.counts().uses);
    let use_line: Box<[u8]> = match row.holder {
        c::AccountHolder::Own => Box::new([]),
        c::AccountHolder::Deployment => use_line(w, runs, waiting),
    };
    let models = models(root, w, provider, text_bytes / 4);
    b.open(Role::Disclosure);
    b.style(crate::look::ACCOUNT_ROW);
    b.name(&title);
    // Skein includes the description in the native disclosure summary.
    let separator = words::say(w, W::Separator, &[]);
    let summary = words::join(&[
        &models,
        if models.is_empty() { b"" } else { &separator },
        &summary_state(w, deciding),
        &words::say(w, W::Separator, &[]),
        &pays,
        if use_line.is_empty() { b"" } else { &separator },
        &use_line,
    ]);
    b.description(&summary);
    b.open(Role::Slot);
    b.style(crate::look::HEADING);
    b.open(Role::Heading { level: 3 });
    b.name(&title);
    let visible = if name.is_some() { caption(w, name, display, plan, sources) } else { title.clone() };
    b.text(Run::Plain, &visible);
    b.close();
    let held = holder_words(root, w, name, holder, sources);
    crate::cards::text(b, Role::Paragraph, &held);
    if let Some(source) = deciding {
        tag(b, w, source);
    }
    let line = words::join(&[
        &models,
        if models.is_empty() { b"" } else { &separator },
        &pays,
        if use_line.is_empty() { b"" } else { &separator },
        &use_line,
    ]);
    crate::cards::text(b, Role::Paragraph, &line);
    b.close();
    let Some(page) = root.pages().accounts() else {
        b.close();
        return;
    };
    let busy = page.request.busy(root.client());
    if moving {
        text(b, w, Role::Paragraph, W::Moving);
    } else if let Some(source) = deciding {
        notice(b, root, w, bindings, row, provider, name, source, sources, manage, busy);
    }
    crate::cards::text(b, Role::Paragraph, &w.say(crate::words::Word::AccountSources, &[]));
    b.open(Role::List);
    for (index, source) in
        sources.iter().take(usize::try_from(bindings.counts().sources).expect("bounded sources")).enumerate()
    {
        b.open(Role::Item);
        b.open(Role::Text);
        b.run(
            Run::Plain,
            &words::say(
                w,
                W::Number,
                &[skein_ui::phrases::Hole::Number(
                    u64::try_from(index).expect("bounded source").checked_add(1).expect("bounded source"),
                )],
            ),
        );
        b.run(Run::Plain, b". ");
        match source.kind {
            c::OwnerAccountKind::SignIn => {
                b.run(Run::Plain, &named(w, W::PlanSignIn, words::provider_words(name.unwrap_or(b"")).1));
            }
            c::OwnerAccountKind::Program => {
                b.run(Run::Plain, &named(w, W::CodeLogin, source.name.as_deref().unwrap_or(b"")));
            }
            c::OwnerAccountKind::Key | c::OwnerAccountKind::None => super::kind(b, w, source.kind),
        }
        b.run(Run::Plain, &words::say(w, W::Separator, &[]));
        b.close();
        tag(b, w, source);
        if source.kind == c::OwnerAccountKind::Program {
            program_hosts(b, root, w, source, false, bindings.counts());
        }
        if moving {
            b.close();
            continue;
        }
        match source.kind {
            c::OwnerAccountKind::Key => {
                key_for(b, w, bindings, W::Replace, row, Some(source.number), busy);
                button(b, root, w, bindings, W::Remove, row, Operation::RemoveKey { source: source.number }, busy);
            }
            c::OwnerAccountKind::SignIn | c::OwnerAccountKind::Program => {
                button(
                    b,
                    root,
                    w,
                    bindings,
                    if source.kind == c::OwnerAccountKind::Program { W::ProgramSignOut } else { W::SignOut },
                    row,
                    Operation::SignOut { source: source.number },
                    busy,
                );
            }
            c::OwnerAccountKind::None => {}
        }
        ranks(b, root, w, bindings, row, Some(source.number), source.place, busy);
        b.close();
    }
    b.close();
    omitted(b, w, W::SourcesOmitted, sources.len(), bindings.counts().sources);
    b.open(Role::Slot);
    b.style(crate::look::DETAILS);
    if !models.is_empty() {
        pair(b, w, W::Models, &models);
    }
    pair(b, w, W::PaysFor, &pays);
    if row.holder == c::AccountHolder::Deployment {
        pair(b, w, W::UsedBy, &chips(w, runs, waiting));
        pair(b, w, W::WaitingFor, &chips(w, waiting, None));
    }
    b.close();
    if !moving {
        let mut has_key = false;
        for source in sources {
            has_key |= source.kind == c::OwnerAccountKind::Key;
        }
        if !has_key && provider.is_some() {
            key_for(b, w, bindings, W::AddApiKey, row, None, busy);
        }
        usage(b, w, manage);
        button(b, root, w, bindings, W::RemoveAccount, row, Operation::RemoveAccount, busy);
        ranks(b, root, w, bindings, row, None, place, busy);
        if let Some((target, source)) = page.key_for
            && target == row
        {
            key_field(b, w, bindings, &page.request, busy);
            let operation = match (source, provider) {
                (Some(source), _) => Some(Operation::ReplaceKey { source }),
                (None, Some(provider)) => Some(Operation::AddKey { provider }),
                (None, None) => None,
            };
            if let Some(operation) = operation {
                button(
                    b,
                    root,
                    w,
                    bindings,
                    if source.is_some() { W::ReplaceKey } else { W::AddKey },
                    row,
                    operation,
                    busy,
                );
            }
        }
    }
    if page.request_row == Some(row) || (page.request.broken.is_some() && editing(page, row)) {
        outcome(b, root, w, bindings, &page.request, text_bytes);
        if let SignInState::Ended { source } = page.request.sign_in_state(root.client())
            && let Some(doc) = accounts::document(root.client())
            && let Some(last) = &doc.last_end
            && last.source == source
            && let Some(provider) = provider
        {
            super::super::connect_model::sign_in_end(b, root, w, Way::SignIn { provider, plan }, &last.end);
            if !moving {
                signin(b, root, w, bindings, row, Some(provider), busy);
            }
        }
        if !moving && let Some((_, ask)) = &page.request.confirm {
            confirmation(b, root, w, page, ask);
            super::control(b, w, bindings, W::Confirm, Meaning::Action(Act::Confirm), busy, false);
        }
        if !moving {
            super::control(b, w, bindings, W::Cancel, Meaning::Action(Act::Cancel), false, false);
        }
    }
    if let Some(doc) = accounts::document(root.client())
        && row.holder == c::AccountHolder::Own
    {
        for signing in doc.signing.iter().take(usize::try_from(bindings.counts().sign_ins).expect("bounded sign-ins")) {
            for source in sources {
                if source.number == signing.source {
                    super::fact(b, w, W::Signing, u64::from(signing.source));
                    break;
                }
            }
        }
        if let Some(last) = &doc.last_end
            && page.request_row != Some(row)
        {
            let mut belongs = false;
            for source in sources {
                belongs |= source.number == last.source;
            }
            match last.end {
                c::OwnerAccountEnd::Made { account, .. } | c::OwnerAccountEnd::Renewed { account, .. } => {
                    belongs = account == row.account;
                }
                c::OwnerAccountEnd::Refused { .. } | c::OwnerAccountEnd::Failed { .. } => {}
            }
            if belongs {
                super::end(b, w, &last.end, &doc.accounts);
            }
        }
    }
    if !moving && editing(page, row) && page.request_row != Some(row) {
        super::control(b, w, bindings, W::Cancel, Meaning::Action(Act::Cancel), false, false);
    }
    if cut || display.cut {
        text(b, w, Role::Status, W::Cut);
    }
    b.close();
}
fn caption(
    w: &Words,
    name: Option<&[u8]>,
    display: &c::DocumentText,
    plan: bool,
    sources: &List<c::OwnerAccountSource>,
) -> Box<[u8]> {
    let (brand, plan_name) = words::provider_words(name.unwrap_or(b""));
    for source in sources {
        if source.kind == c::OwnerAccountKind::Program {
            return words::join(&[
                &named(w, W::Subscription, plan_name),
                &words::say(w, W::Separator, &[]),
                source.name.as_deref().unwrap_or(b""),
            ]);
        }
    }
    let mut signed = plan;
    for source in sources {
        signed |= source.kind == c::OwnerAccountKind::SignIn;
    }
    if signed {
        let separator = words::say(w, W::Separator, &[]);
        let label = if plan { plan_name } else { brand };
        return words::join(&[
            label,
            if display.bytes().is_empty() || display.bytes() == label { b"" } else { &separator },
            if display.bytes() == label { b"" } else { display.bytes() },
        ]);
    }
    named(w, W::ApiKeyBrand, brand)
}
fn holder_words(
    root: &Domain,
    w: &Words,
    name: Option<&[u8]>,
    holder: c::OwnerAccountHolder,
    sources: &List<c::OwnerAccountSource>,
) -> Box<[u8]> {
    let owner = match holder {
        c::OwnerAccountHolder::Deployment => words::say(w, W::HeldDeployment, &[]),
        c::OwnerAccountHolder::Party(_) => match accounts::current_me(root.client()) {
            Some(me) => named(w, W::HeldBy, me.party.name.bytes()),
            None => named(w, W::HeldBy, &words::say(w, W::OwnHolder, &[])),
        },
    };
    for source in sources {
        if source.kind == c::OwnerAccountKind::SignIn || source.kind == c::OwnerAccountKind::Program {
            return words::join(&[
                words::provider_words(name.unwrap_or(b"")).0,
                &words::say(w, W::Separator, &[]),
                &owner,
            ]);
        }
    }
    owner
}
fn tag(b: &mut Builder, w: &Words, source: &c::OwnerAccountSource) {
    b.open(Role::Status);
    b.style(if source.usable {
        crate::look::STATUS_ACTIVE
    } else if let c::OwnerAccountState::Later { .. } = source.state {
        crate::look::STATUS_RETRYING
    } else {
        crate::look::STATUS_HELD
    });
    let label = summary_state(w, Some(source));
    b.name(&label);
    b.open(Role::Text);
    state(b, w, source);
    b.close();
    b.close();
}
fn summary_state(w: &Words, source: Option<&c::OwnerAccountSource>) -> Box<[u8]> {
    let word = match source {
        None => W::Unknown,
        Some(source) => {
            if source.spent_until.is_some() && source.state != c::OwnerAccountState::Moving {
                W::Limited
            } else {
                match source.state {
                    c::OwnerAccountState::Moving => W::Moving,
                    c::OwnerAccountState::Granted if source.usable => W::Ready,
                    c::OwnerAccountState::Granted => W::Granted,
                    c::OwnerAccountState::Later { .. } => W::Limited,
                    c::OwnerAccountState::SignInAgain => W::NeedsSignIn,
                    c::OwnerAccountState::Misconfigured => W::Misconfigured,
                    c::OwnerAccountState::Unloaded { .. } => W::Unloaded,
                    c::OwnerAccountState::Asking => W::Asking,
                    c::OwnerAccountState::Cancelled => W::Cancelled,
                    c::OwnerAccountState::Closing => W::Closing,
                }
            }
        }
    };
    words::say(w, word, &[])
}
fn pair(b: &mut Builder, w: &Words, word: W, value: &[u8]) {
    b.open(Role::Region);
    b.name(&words::say(w, word, &[]));
    text(b, w, Role::Term, word);
    crate::cards::text(b, Role::Description, value);
    b.close();
}
fn pays(w: &Words, plan: bool, uses: Option<&List<c::OwnerAccountUse>>, cap: u32) -> Box<[u8]> {
    let Some(uses) = uses else { return words::say(w, if plan { W::PlanPays } else { W::OwnPays }, &[]) };
    let mut result = words::say(w, W::None, &[]);
    for (index, use_) in uses.iter().take(usize::try_from(cap).expect("bounded uses")).enumerate() {
        let (word, number) = match use_ {
            c::OwnerAccountUse::Party(n) => (W::Party, *n),
            c::OwnerAccountUse::Project(n) => (W::Project, u64::from(*n)),
        };
        let entry = words::join(&[
            &words::say(w, word, &[]),
            &words::say(w, W::Number, &[skein_ui::phrases::Hole::Number(number)]),
        ]);
        result = if index == 0 { entry } else { words::join(&[&result, b", ", &entry]) };
    }
    if uses.len() > cap {
        result = words::join(&[
            &result,
            b"; ",
            &words::say(w, W::UsesOmitted, &[]),
            &words::say(w, W::Number, &[skein_ui::phrases::Hole::Number(u64::from(uses.len().saturating_sub(cap)))]),
        ]);
    }
    result
}
fn chips(w: &Words, rows: Option<&List<c::Chip>>, excluded: Option<&List<c::Chip>>) -> Box<[u8]> {
    let mut result = words::say(w, W::None, &[]);
    let mut first = true;
    if let Some(rows) = rows {
        for row in rows {
            let mut skip = false;
            if let Some(excluded) = excluded {
                for waiting in excluded {
                    skip |= waiting.task == row.task;
                }
            }
            if skip {
                continue;
            }
            let task = words::say(w, W::Number, &[skein_ui::phrases::Hole::Number(row.task)]);
            result = words::join(&[if first { b"" } else { &result }, if first { b"T" } else { b", T" }, &task]);
            first = false;
        }
    }
    result
}
fn use_line(w: &Words, runs: Option<&List<c::Chip>>, waiting: Option<&List<c::Chip>>) -> Box<[u8]> {
    if let Some(waiting) = waiting
        && !waiting.is_empty()
    {
        return words::join(&[
            &words::say(w, W::Number, &[skein_ui::phrases::Hole::Number(u64::from(waiting.len()))]),
            b" tasks waiting",
        ]);
    }
    if let Some(runs) = runs
        && !runs.is_empty()
    {
        return words::join(&[b"used by ", &chips(w, Some(runs), waiting)]);
    }
    words::say(w, W::NoUsers, &[])
}
#[expect(clippy::too_many_arguments, reason = "each bound control names its row and operation")]
fn button(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    word: W,
    row: Row,
    operation: Operation,
    busy: bool,
) {
    super::control(
        b,
        w,
        bindings,
        word,
        Meaning::Action(Act::Submit { name: root.pages().account_name(), row, operation }),
        busy,
        false,
    );
}
fn key_for(b: &mut Builder, w: &Words, bindings: &mut Bindings, word: W, row: Row, source: Option<u32>, busy: bool) {
    super::control(b, w, bindings, word, Meaning::Action(Act::KeyFor { row, source }), busy, false);
}
fn signin(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    row: Row,
    provider: Option<u32>,
    busy: bool,
) {
    if row.holder == c::AccountHolder::Deployment {
        return;
    }
    let Some(provider) = provider else { return };
    let mut plan = b"".as_slice();
    if let Some(doc) = accounts::document(root.client()) {
        for account in &doc.accounts {
            if account.number == row.account
                && let Some(name) = provider_name(account.provider_name.as_ref())
            {
                plan = words::provider_words(name).1;
            }
        }
    }
    if let Some(me) = accounts::current_me(root.client()) {
        for offer in &me.providers {
            if offer.provider == provider {
                plan = words::provider_words(offer.name.bytes()).1;
            }
        }
    }
    named_control(
        b,
        bindings,
        &named(w, W::Continue, plan),
        Meaning::Action(Act::Submit {
            name: root.pages().account_name(),
            row,
            operation: Operation::SignIn { provider },
        }),
        busy,
        false,
    );
}
fn usage(b: &mut Builder, w: &Words, address: Option<&[u8]>) {
    if let Some(address) = address
        && c::valid_manage_usage(address)
        && skein_ui::markdown::safe_url(address)
    {
        b.open(Role::Link);
        b.name(&w.say(crate::words::Word::Usage, &[]));
        b.text(Run::Plain, &w.say(crate::words::Word::Usage, &[]));
        b.external(address);
        b.close();
    }
}
#[expect(clippy::too_many_arguments, reason = "the state's acts borrow their deciding source and row")]
fn notice(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    row: Row,
    provider: Option<u32>,
    provider_text: Option<&[u8]>,
    source: &c::OwnerAccountSource,
    sources: &List<c::OwnerAccountSource>,
    manage: Option<&[u8]>,
    busy: bool,
) {
    let mut declined = false;
    if row.holder == c::AccountHolder::Own
        && let Some(doc) = accounts::document(root.client())
        && let Some(last) = &doc.last_end
    {
        match last.end {
            c::OwnerAccountEnd::Made { account, declined: yes }
            | c::OwnerAccountEnd::Renewed { account, declined: yes } => declined = yes && account == row.account,
            c::OwnerAccountEnd::Refused { .. } | c::OwnerAccountEnd::Failed { .. } => {}
        }
    }
    if declined {
        text(b, w, Role::Alert, W::DeclinedPlan);
        signin(b, root, w, bindings, row, provider, busy);
        key_for(b, w, bindings, W::KeyWayOnly, row, None, busy);
        return;
    }
    let limited =
        if let c::OwnerAccountState::Later { .. } = source.state { true } else { source.spent_until.is_some() };
    if limited {
        let reset = if let c::OwnerAccountState::Later { until } = source.state { until } else { None };
        if let Some(reset) = reset {
            b.open(Role::Paragraph);
            b.open(Role::Text);
            b.run(Run::Plain, &words::say(w, W::LimitedBy, &[]));
            let mut brand = words::provider_words(provider_text.unwrap_or(b"")).0;
            if let Some(me) = accounts::current_me(root.client()) {
                for offer in &me.providers {
                    if Some(offer.provider) == provider {
                        brand = words::provider_words(offer.name.bytes()).0;
                    }
                }
            }
            b.run(Run::Plain, brand);
            b.run(Run::Plain, &words::say(w, W::ResetUntil, &[skein_ui::phrases::Hole::Wall(reset)]));
            b.close();
            b.close();
        } else {
            text(
                b,
                w,
                Role::Paragraph,
                if source.kind == c::OwnerAccountKind::SignIn { W::PlanLimited } else { W::Limited },
            );
            usage(b, w, manage);
        }
        button(b, root, w, bindings, W::Resume, row, Operation::Resume { source: source.number }, busy);
        for next in sources {
            if next.place > source.place && next.kind == c::OwnerAccountKind::Key {
                button(
                    b,
                    root,
                    w,
                    bindings,
                    W::KeyFirst,
                    row,
                    Operation::MoveSource { source: next.number, place: source.place },
                    busy,
                );
                break;
            }
        }
    } else if source.kind == c::OwnerAccountKind::Program {
        b.open(Role::Paragraph);
        b.open(Role::Text);
        state(b, w, source);
        b.close();
        b.close();
        let needs_check = program_hosts(b, root, w, source, true, bindings.counts());
        if needs_check || source.state == c::OwnerAccountState::SignInAgain {
            button(b, root, w, bindings, W::CheckAgain, row, Operation::CheckProgram { source: source.number }, busy);
        }
    } else {
        match source.state {
            c::OwnerAccountState::SignInAgain => {
                text(b, w, Role::Paragraph, W::SignedOutPlan);
                signin(b, root, w, bindings, row, provider, busy);
            }
            c::OwnerAccountState::Misconfigured => text(b, w, Role::Paragraph, W::MisconfiguredPlan),
            c::OwnerAccountState::Moving
            | c::OwnerAccountState::Asking
            | c::OwnerAccountState::Granted
            | c::OwnerAccountState::Later { .. }
            | c::OwnerAccountState::Unloaded { .. }
            | c::OwnerAccountState::Cancelled
            | c::OwnerAccountState::Closing => {
                b.open(Role::Paragraph);
                b.open(Role::Text);
                state(b, w, source);
                b.close();
                b.close();
            }
        }
    }
}
fn program_hosts(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    source: &c::OwnerAccountSource,
    notice: bool,
    counts: temper_client_domain::limits::AccountLimits,
) -> bool {
    let Some(doc) = accounts::document(root.client()) else { return false };
    let mut needs_check = false;
    for host in doc.program_hosts.iter().take(usize::try_from(counts.accounts).expect("bounded hosts")) {
        for login in host.programs.iter().take(usize::try_from(counts.sources).expect("bounded programs")) {
            if Some(login.name.as_ref()) != source.name.as_deref() {
                continue;
            }
            if notice && !needs_program(login.state.as_ref()) {
                continue;
            }
            b.open(Role::Paragraph);
            b.name(&host.name);
            b.open(Role::Text);
            if notice && login.state == Some(c::OwnerProgramState::NotSignedIn) {
                needs_check = true;
                b.run(Run::Plain, &named(w, W::ProgramNeedsSignIn, &login.name));
            }
            if program_failed(login.state.as_ref()) {
                run(b, w, W::UnusableOn);
            }
            b.run(Run::Plain, &host.name);
            match &login.state {
                Some(c::OwnerProgramState::Failed { reason, detail }) => {
                    b.run(Run::Plain, b": ");
                    super::super::account_programs::failure(b, w, *reason, detail, w.name_bytes);
                }
                Some(c::OwnerProgramState::SignedInOtherwise { method }) => {
                    run(b, w, W::SignedInOtherwise);
                    b.run(Run::Plain, method);
                }
                Some(c::OwnerProgramState::Usable) => run(b, w, W::Ready),
                None => run(b, w, W::Unknown),
                Some(c::OwnerProgramState::NotSignedIn) => {}
            }
            if !host.connected {
                run(b, w, W::NotInContact);
            }
            if host.awaiting_logins {
                run(b, w, W::AwaitingLogins);
            }
            if host.cut || login.cut {
                run(b, w, W::Cut);
            }
            if login.cleanup {
                run(b, w, W::Cleanup);
            }
            if notice && login.state == Some(c::OwnerProgramState::NotSignedIn) {
                b.run(Run::Plain, b".");
            }
            if let Some(sign_out) = &login.sign_out {
                match sign_out {
                    c::OwnerProgramSignOut::SignedOut => run(b, w, W::SignedOut),
                    c::OwnerProgramSignOut::Failed { reason, detail } => {
                        super::super::account_programs::failure(b, w, *reason, detail, w.name_bytes);
                    }
                }
            }
            b.close();
            b.close();
            if notice && login.state == Some(c::OwnerProgramState::NotSignedIn) {
                crate::cards::text(b, Role::Code, &named(w, W::ProgramCommand, &login.name));
            }
            break;
        }
    }
    needs_check
}
#[expect(clippy::too_many_arguments, reason = "rank controls share the current row and source positions")]
fn ranks(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    row: Row,
    source: Option<u32>,
    place: u32,
    busy: bool,
) {
    for (target, word) in [
        (place.checked_sub(1), if source.is_some() { W::UpSource } else { W::UpAccount }),
        (place.checked_add(1), if source.is_some() { W::DownSource } else { W::DownAccount }),
    ] {
        if let Some(place) = target {
            let op = match source {
                Some(source) => Operation::MoveSource { source, place },
                None => Operation::MoveAccount { place },
            };
            button(b, root, w, bindings, word, row, op, busy);
        }
    }
}

fn models(root: &Domain, w: &Words, provider: Option<u32>, bound: u32) -> Box<[u8]> {
    let mut result: Box<[u8]> = Box::new([]);
    if let Some(me) = accounts::current_me(root.client()) {
        for project in &me.projects {
            if Some(project.project) != root.pages().project() {
                continue;
            }
            for model in &project.models {
                if model.provider != provider || provider.is_none() {
                    continue;
                }
                let shown = crate::choice::shown(model.name.bytes(), w);
                let next = words::join(&[&result, if result.is_empty() { b"" } else { b", " }, &shown]);
                if next.len() > usize::try_from(bound).expect("text bound fits") {
                    return result;
                }
                result = next;
            }
        }
    }
    result
}

fn editing(page: &accounts::Accounts, row: Row) -> bool {
    match page.key_for {
        Some((target, _)) => target == row,
        None => false,
    }
}
fn needs_program(state: Option<&c::OwnerProgramState>) -> bool {
    match state {
        Some(c::OwnerProgramState::NotSignedIn | c::OwnerProgramState::Failed { .. }) => true,
        None | Some(c::OwnerProgramState::Usable | c::OwnerProgramState::SignedInOtherwise { .. }) => false,
    }
}
fn program_failed(state: Option<&c::OwnerProgramState>) -> bool {
    match state {
        Some(c::OwnerProgramState::Failed { .. }) => true,
        Some(
            c::OwnerProgramState::Usable
            | c::OwnerProgramState::NotSignedIn
            | c::OwnerProgramState::SignedInOtherwise { .. },
        )
        | None => false,
    }
}
