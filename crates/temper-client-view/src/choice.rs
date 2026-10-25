//! Supplied next-run choices and account waits (client/README.md, 3.5 and 5.5).
use crate::{
    frame::Bindings,
    words::{TimeForm, Times, Word as W, Words},
};
use alloc::boxed::Box;
use jig_client::contract as c;
use skein_ui::{Builder, Role, State, phrases::Hole};
use temper_client_domain::{
    Address, Domain,
    pages::{
        Act,
        choice::{self, Act as C},
    },
};

fn line(b: &mut Builder, w: &Words, word: W, holes: &[Hole]) {
    let text = w.say(word, holes);
    b.open(Role::Paragraph);
    if word != W::AccountBanner {
        b.style(crate::look::ACCOUNT_LINE);
    }
    b.name(&text);
    b.text(skein_ui::Run::Plain, &text);
    b.close();
}
fn control(b: &mut Builder, bindings: &mut Bindings, role: Role, name: &[u8], act: Act, pressed: bool) {
    if role == Role::Link {
        crate::link(b, name, bindings.insert(act));
        b.text(skein_ui::Run::Plain, name);
    } else {
        b.open(role);
        b.name(name);
        b.bind(bindings.insert(act));
    }
    if pressed {
        b.state(State::Pressed);
    }
    b.close();
}
fn account_link(b: &mut Builder, bindings: &mut Bindings, name: &[u8]) {
    control(b, bindings, Role::Link, name, Act::Go { address: Address::Accounts }, false);
}
pub(crate) fn banner(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings) {
    if crate::frame::unavailable(root).is_some()
        || match root.pages().address() {
            Address::Accounts | Address::ConnectModel { .. } | Address::Welcome { .. } => true,
            Address::Forges
            | Address::ConnectForge { .. }
            | Address::Inbox { .. }
            | Address::Chats { .. }
            | Address::Chat { .. }
            | Address::Task { .. }
            | Address::Adopt { .. } => false,
        }
        || root.pages().availability(root.client()) != Some(false)
    {
        return;
    }
    b.open(Role::Region);
    b.name(&w.say(W::AccountRegion, &[]));
    b.style(crate::look::ACCOUNTS);
    line(b, w, W::AccountBanner, &[]);
    if let Some(me) = temper_client_domain::pages::accounts::current_me(root.client()) {
        let demo = root.forge().demo_provider();
        let ways = temper_client_domain::pages::accounts::ways(&me.providers, demo, c::AccountHolder::Own);
        for way in &ways {
            let name = crate::pages::connect_model::label(me, *way, &ways, w);
            control(
                b,
                bindings,
                Role::Link,
                &name,
                Act::Go {
                    address: Address::ConnectModel {
                        way: Some(temper_client_domain::pages::connect_model::model_way(*way)),
                    },
                },
                false,
            );
        }
    }
    b.close();
}
/// The setup strip and composers read the same account and demo facts.
pub(crate) fn can_send(root: &Domain) -> bool {
    let demo_model = match root.demo() {
        Some((_, halves)) => halves.model,
        None => false,
    };
    demo_model || temper_client_domain::pages::welcome::model_usable(root) != Some(false)
}
pub(crate) fn composer(
    b: &mut Builder,
    root: &Domain,
    task: Option<&c::Task>,
    w: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    if let Some(task) = task {
        match task.chip.executor {
            c::Executor::Agent { .. } => {}
            c::Executor::Procedure { .. } | c::Executor::Party { .. } | c::Executor::Role { .. } => return,
        }
    }
    let draft = root.pages().choice();
    b.open(Role::Slot);
    b.style(crate::look::CHOICE);
    if !can_send(root) {
        bindings.block_submissions();
        b.open(Role::Paragraph);
        b.style(crate::look::ACCOUNT_LINE);
        w.text(b, W::ComposerNoModel, &[]);
        control(
            b,
            bindings,
            Role::Link,
            &w.say(W::ConnectModel, &[]),
            Act::Go { address: Address::ConnectModel { way: None } },
            false,
        );
        b.close();
        feedback(b, root, task, w, bindings, times);
        b.close();
        return;
    }
    using(b, root, task, w);
    if let Some(me) = temper_client_domain::pages::accounts::current_me(root.client())
        && let Some(project) = draft.project()
        && let Some(models) = choice::models(me, project)
        && !models.is_empty()
        && draft.pending.is_none()
        && match task {
            Some(task) => editable(task),
            None => true,
        }
    {
        b.open(Role::Disclosure);
        b.name(&w.say(W::ChangeModel, &[]));
        b.open(Role::List);
        b.style(crate::look::ROWS);
        let selected_model = draft.selected(me);
        for (model, index) in models.iter().zip(0u32..) {
            b.open(Role::Item);
            b.style(crate::look::ROW);
            provider(b, root, task, model.provider, w);
            let selected = selected_model == Some(index);
            control(
                b,
                bindings,
                Role::Button,
                &model_shown(model.name.bytes(), w),
                Act::Choice(C::Model { index }),
                selected,
            );
            if selected {
                for (effort, index) in model.efforts.iter().zip(0u32..) {
                    let selected = match &draft.requested {
                        Some(choice) => choice.main.effort.as_ref() == Some(effort),
                        None => false,
                    };
                    control(
                        b,
                        bindings,
                        Role::Button,
                        &shown(effort.bytes(), w),
                        Act::Choice(C::Effort { index }),
                        selected,
                    );
                }
            }
            b.close();
        }
        b.close();
        control(b, bindings, Role::Button, &w.say(W::ChoiceReset, &[]), Act::Choice(C::Default), false);
        if task.is_some() && draft.requested != draft.original {
            match &draft.requested {
                Some(choice) => line(b, w, W::ChoiceDraft, &[Hole::Name(model_shown(choice.main.model.bytes(), w))]),
                None => line(b, w, W::ChoiceDraftDefault, &[]),
            }
            if draft.valid(me) {
                control(b, bindings, Role::Button, &w.say(W::ChoiceApply, &[]), Act::Choice(C::Apply), false);
            }
        }
        b.close();
    }
    feedback(b, root, task, w, bindings, times);
    b.close();
}
fn feedback(
    b: &mut Builder,
    root: &Domain,
    task: Option<&c::Task>,
    w: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    let draft = root.pages().choice();
    if draft.pending.is_some() {
        line(b, w, W::ChoicePending, &[]);
    }
    if let Some(outcome) = &draft.outcome {
        crate::pages::chats::outcome_text(b, w, outcome, 0);
    }
    if let Some(task) = task {
        waiting(b, root, task.run_wait, w, bindings, times);
    }
}

/// Display names keep the catalogue's bounded presentation.
fn model_shown(name: &[u8], w: &Words) -> Box<[u8]> {
    shown(&w.model_name(name), w)
}

pub(crate) fn provider_shown(root: &Domain, number: Option<u32>, name: &[u8], w: &Words) -> Box<[u8]> {
    if let Some(demo) = root.forge().demo()
        && number == Some(demo.provider)
    {
        w.say(W::Demo, &[])
    } else {
        skein_lib::bytes::copy_of(name)
    }
}

/// Includes the UTF-8 ellipsis; smaller phrasebooks retain their joined Name bound.
pub(crate) const MODEL_SHOWN: u32 = 256;
pub(crate) fn shown(name: &[u8], w: &Words) -> Box<[u8]> {
    let Ok(bound) = usize::try_from(MODEL_SHOWN.min(w.name_bytes)) else { return Box::default() };
    if name.len() <= bound {
        return skein_lib::bytes::copy_of(name);
    }
    let mark = "…".as_bytes();
    let Some(prefix_bound) = bound.checked_sub(mark.len()) else { return Box::default() };
    let Some(scan) = prefix_bound.checked_add(1) else { return Box::default() };
    // Text arrived validated; a continuation byte cannot begin the cut suffix.
    let mut end = 0;
    for (index, byte) in name.iter().enumerate().take(scan) {
        if byte & 0xc0 != 0x80 {
            end = index;
        }
    }
    let Some(prefix) = name.get(..end) else { return Box::default() };
    let Some(bytes) = end.checked_add(mark.len()) else { return Box::default() };
    let mut out = skein_lib::Writer::new(bytes);
    if out.put(prefix).is_err() || out.put(mark).is_err() {
        return Box::default();
    }
    out.finish()
}

/// Keep payer selection shared with the catalogue's provider readiness.
fn payer(me: &c::Me, task: Option<&c::Task>) -> Option<c::OwnerAccountUse> {
    match task {
        None => Some(c::OwnerAccountUse::Party(me.party.number)),
        Some(task) => match task.budget.funder {
            c::Funder::Pool { person, .. } => Some(c::OwnerAccountUse::Party(person)),
            c::Funder::Period { project, .. } | c::Funder::Recurring { project, .. } => {
                Some(c::OwnerAccountUse::Project(project))
            }
            c::Funder::Task(_) => None,
        },
    }
}
pub(crate) fn readiness(
    doc: &c::OwnerAccounts,
    provider: Option<u32>,
    payer: Option<c::OwnerAccountUse>,
) -> c::Readiness {
    for row in &doc.readiness {
        if Some(row.provider) == provider && Some(row.payer) == payer {
            return row.readiness;
        }
    }
    c::Readiness::Unknown
}
// Translation of jig-domain-core coverage.rs and jig-domain-accounts selection.rs.
// The own-party slice alone preserves jig's payer/explicit/deployment ordering.
fn eligible(row: &c::OwnerAccountLine, viewer: u64, provider: Option<u32>) -> bool {
    if row.provider != provider || row.holder != c::OwnerAccountHolder::Party(viewer) {
        return false;
    }
    // In the own-holder slice, plan and non-plan coverage coincide.
    match &row.uses {
        Some(uses) => uses.as_slice().contains(&c::OwnerAccountUse::Party(viewer)),
        None => true,
    }
}
fn pinned(
    doc: &c::OwnerAccounts,
    viewer: u64,
    provider: Option<u32>,
    pins: [Option<u32>; 2],
) -> Option<&c::OwnerAccountLine> {
    for number in pins.into_iter().flatten() {
        for row in &doc.accounts {
            if row.number == number && eligible(row, viewer, provider) {
                return Some(row);
            }
        }
    }
    None
}
fn program_endpoint(me: &c::Me, provider: Option<u32>) -> Option<bool> {
    for offer in &me.providers {
        if Some(offer.provider) == provider {
            if offer.programs_cut {
                return None;
            }
            if offer.programs.is_empty() {
                return Some(false);
            }
            // Me has no per-model endpoint: mixed offers cannot establish its kind.
            return if offer.key || offer.sign_in || offer.none { None } else { Some(true) };
        }
    }
    None
}
fn held(state: c::OwnerAccountState) -> Option<bool> {
    match state {
        c::OwnerAccountState::Asking | c::OwnerAccountState::Granted => Some(false),
        // The public Later state also carries jig's Limited state.
        c::OwnerAccountState::Later { .. } => None,
        c::OwnerAccountState::Moving
        | c::OwnerAccountState::SignInAgain
        | c::OwnerAccountState::Misconfigured
        | c::OwnerAccountState::Unloaded { .. }
        | c::OwnerAccountState::Cancelled
        | c::OwnerAccountState::Closing => Some(true),
    }
}
enum SourceChoice {
    Chosen(u32),
    Wait,
    Undecidable,
}
fn chosen_source(row: &c::OwnerAccountLine, program: bool) -> SourceChoice {
    // An unusable "later" program source is jig's Later, which goes on to the
    // account's next program source, or its Limited, which waits: a source
    // chosen after it is undecidable.
    let mut later = false;
    for (source, index) in row.sources.iter().zip(0u32..) {
        let fits = match source.kind {
            c::OwnerAccountKind::Program => program,
            c::OwnerAccountKind::Key | c::OwnerAccountKind::SignIn | c::OwnerAccountKind::None => !program,
        };
        if !fits {
            continue;
        }
        match held(source.state) {
            Some(true) => return SourceChoice::Wait,
            None => {
                if source.usable {
                    return SourceChoice::Undecidable;
                }
                if !program {
                    return SourceChoice::Wait;
                }
                later = true;
                continue;
            }
            Some(false) => {}
        }
        if source.usable {
            return if later { SourceChoice::Undecidable } else { SourceChoice::Chosen(index) };
        }
        if !program {
            return SourceChoice::Wait;
        }
    }
    SourceChoice::Wait
}
fn selected_account(
    doc: &c::OwnerAccounts,
    viewer: u64,
    provider: Option<u32>,
    program: bool,
) -> Option<&c::OwnerAccountLine> {
    for row in &doc.accounts {
        if !eligible(row, viewer, provider) {
            continue;
        }
        if row.cut {
            return Some(row);
        }
        match chosen_source(row, program) {
            SourceChoice::Chosen(_) => return Some(row),
            SourceChoice::Wait => {}
            SourceChoice::Undecidable => return None,
        }
    }
    None
}
fn using(b: &mut Builder, root: &Domain, task: Option<&c::Task>, w: &Words) {
    let draft = root.pages().choice();
    let choice = match task {
        Some(_) => draft.original.as_ref(),
        None => draft.requested.as_ref(),
    };
    if let Some((name, provider)) = model_words(root, task, choice, draft.project(), w) {
        using_line(b, root, task, choice, provider, name, w);
    } else {
        line(b, w, W::UsingDefault, &[]);
    }
}

/// Ready pairs a supplied model only with an account selected for that model's provider.
pub(crate) fn ready_model(root: &Domain, w: &Words) -> Box<[u8]> {
    use crate::accounts::words::{self as aw, Word as A};
    let project = root.pages().project();
    let choice = match (temper_client_domain::pages::accounts::current_me(root.client()), project) {
        (Some(me), Some(project)) => match jig_client::shapes::template(me, project, jig_client::shapes::CHAT) {
            Some(template) => template.llm.as_ref(),
            None => None,
        },
        _ => None,
    };
    let Some((model, provider)) = model_words(root, None, choice, project, w) else {
        return match crate::pages::accounts::first_usable_heading(root, w) {
            Some(account) => account,
            None => new_chat_model(root, w),
        };
    };
    let Some(me) = temper_client_domain::pages::accounts::current_me(root.client()) else { return model };
    let Some(doc) = temper_client_domain::pages::accounts::document(root.client()) else { return model };
    if doc.cut {
        return model;
    }
    let Some(program) = program_endpoint(me, provider) else { return model };
    let preferred = match choice {
        Some(choice) => choice.account,
        None => None,
    };
    let account = match pinned(doc, me.party.number, provider, [preferred, None]) {
        Some(row) => Some(row),
        None => selected_account(doc, me.party.number, provider, program),
    };
    let Some(account) = account else { return model };
    if account.cut {
        return model;
    }
    let SourceChoice::Chosen(_) = chosen_source(account, program) else { return model };
    // The account the model runs on, by its whole heading, then the model.
    let separator = aw::say(w, A::Separator, &[]);
    let heading = crate::pages::accounts::account_heading(root, w, account);
    aw::join(&[&heading, &separator, &model])
}

/// Ready uses the same model and effort words as a new chat's composer.
pub(crate) fn new_chat_model(root: &Domain, w: &Words) -> Box<[u8]> {
    let project = root.pages().project();
    let choice = match (temper_client_domain::pages::accounts::current_me(root.client()), project) {
        (Some(me), Some(project)) => match jig_client::shapes::template(me, project, jig_client::shapes::CHAT) {
            Some(template) => template.llm.as_ref(),
            None => None,
        },
        _ => None,
    };
    match model_words(root, None, choice, project, w) {
        Some((name, _)) => name,
        None => w.say(W::DefaultModel, &[]),
    }
}
fn model_words(
    root: &Domain,
    task: Option<&c::Task>,
    choice: Option<&c::LlmChoice>,
    project: Option<u32>,
    w: &Words,
) -> Option<(Box<[u8]>, Option<u32>)> {
    let me = temper_client_domain::pages::accounts::current_me(root.client());
    let doc = temper_client_domain::pages::accounts::document(root.client());
    let payer = match me {
        Some(me) => payer(me, task),
        None => None,
    };
    let models = match (me, project) {
        (Some(me), Some(project)) => choice::models(me, project),
        _ => None,
    };
    let mut model = None;
    if let Some(models) = models {
        for offered in models {
            let matches = match choice {
                Some(choice) => offered.name == choice.main.model,
                None => match doc {
                    Some(doc) => readiness(doc, offered.provider, payer) == c::Readiness::Usable,
                    None => false,
                },
            };
            if matches {
                model = Some(offered);
                break;
            }
        }
    }
    let name = match choice {
        Some(choice) => choice.main.model.bytes(),
        None => match model {
            Some(model) => model.name.bytes(),
            None => return None,
        },
    };
    let mut name = model_shown(name, w);
    if let Some(choice) = choice
        && let Some(effort) = &choice.main.effort
    {
        name = w.say(W::ModelEffort, &[Hole::Name(name), Hole::Name(shown(effort.bytes(), w))]);
    }
    let provider = match model {
        Some(model) => model.provider,
        None => None,
    };
    Some((name, provider))
}
fn using_line(
    b: &mut Builder,
    root: &Domain,
    task: Option<&c::Task>,
    choice: Option<&c::LlmChoice>,
    provider: Option<u32>,
    name: Box<[u8]>,
    w: &Words,
) {
    let me = temper_client_domain::pages::accounts::current_me(root.client());
    let doc = temper_client_domain::pages::accounts::document(root.client());
    let payer = match me {
        Some(me) => payer(me, task),
        None => None,
    };
    let ready = match doc {
        Some(doc) => readiness(doc, provider, payer),
        None => c::Readiness::Unknown,
    };
    let preferred = match choice {
        Some(choice) => choice.account,
        None => None,
    };
    let kept = match task {
        Some(task) => task.kept_account,
        None => None,
    };
    let own = match me {
        Some(me) => payer == Some(c::OwnerAccountUse::Party(me.party.number)),
        None => false,
    };
    let pin = match (me, doc) {
        (Some(me), Some(doc)) if own => pinned(doc, me.party.number, provider, [preferred, kept]),
        (Some(_), Some(_)) | (None, _) | (_, None) => None,
    };
    // Demo.provider is public even when its deployment-held account is outside this slice.
    if pin.is_none()
        && let Some(demo) = root.forge().demo()
        && provider == Some(demo.provider)
    {
        if ready == c::Readiness::Unavailable {
            line(b, w, W::ModelNeedsAccount, &[Hole::Name(name)]);
        } else {
            line(b, w, W::DemoUsing, &[Hole::Name(w.say(W::Default, &[]))]);
        }
        return;
    }
    let (Some(me), Some(doc)) = (me, doc) else {
        line(b, w, W::UsingModel, &[Hole::Name(name)]);
        return;
    };
    let program = program_endpoint(me, provider);
    let account = if own
        && !doc.cut
        && let Some(program) = program
    {
        match pin {
            Some(row) => Some(row),
            None => selected_account(doc, me.party.number, provider, program),
        }
    } else {
        None
    };
    if let Some(account) = account {
        let source = chosen_source(account, program.expect("selected with a known endpoint"));
        let undecidable = match source {
            SourceChoice::Undecidable => true,
            SourceChoice::Chosen(_) | SourceChoice::Wait => false,
        };
        if account.cut || (ready != c::Readiness::Usable && !undecidable) {
            line(b, w, W::UsingModel, &[Hole::Name(name)]);
        } else {
            match source {
                SourceChoice::Chosen(index) => {
                    let source = account.sources.get(index).expect("chosen source exists");
                    using_account(b, root, account, source, name, w);
                }
                SourceChoice::Wait => line(b, w, W::ModelNeedsAccount, &[Hole::Name(name)]),
                SourceChoice::Undecidable => {
                    let word = if ready == c::Readiness::Unavailable { W::ModelNeedsAccount } else { W::UsingModel };
                    line(b, w, word, &[Hole::Name(name)]);
                }
            }
        }
    } else {
        let word = if ready == c::Readiness::Unavailable { W::ModelNeedsAccount } else { W::UsingModel };
        line(b, w, word, &[Hole::Name(name)]);
    }
}
fn program_name(root: &Domain, provider: Option<u32>, w: &Words) -> Box<[u8]> {
    if let Some(me) = temper_client_domain::pages::accounts::current_me(root.client()) {
        for offer in &me.providers {
            if Some(offer.provider) == provider
                && let Some(program) = offer.programs.get(0)
            {
                return shown(if program.bytes() == b"claude_code" { b"Claude Code" } else { program.bytes() }, w);
            }
        }
    }
    Box::default()
}
fn using_account(
    b: &mut Builder,
    root: &Domain,
    account: &c::OwnerAccountLine,
    source: &c::OwnerAccountSource,
    name: Box<[u8]>,
    w: &Words,
) {
    let mut provider_name = None;
    if let Some(me) = temper_client_domain::pages::accounts::current_me(root.client()) {
        for offer in &me.providers {
            if Some(offer.provider) == account.provider {
                provider_name = Some(crate::accounts::words::provider_words(offer.name.bytes()));
                break;
            }
        }
    }
    let Some(provider_name) = provider_name else {
        line(b, w, W::UsingModel, &[Hole::Name(name)]);
        return;
    };
    let text = match source.kind {
        c::OwnerAccountKind::Key => {
            w.say(W::UsingKey, &[Hole::Name(skein_lib::bytes::copy_of(provider_name.0)), Hole::Name(name)])
        }
        c::OwnerAccountKind::SignIn => {
            w.say(W::UsingPlan, &[Hole::Name(skein_lib::bytes::copy_of(provider_name.1)), Hole::Name(name)])
        }
        c::OwnerAccountKind::Program => w.say(
            W::UsingProgram,
            &[
                Hole::Name(skein_lib::bytes::copy_of(provider_name.1)),
                Hole::Name(program_name(root, account.provider, w)),
                Hole::Name(name),
            ],
        ),
        c::OwnerAccountKind::None => w.say(W::UsingModel, &[Hole::Name(name)]),
    };
    b.open(Role::Paragraph);
    b.style(crate::look::ACCOUNT_LINE);
    b.name(&text);
    b.text(skein_ui::Run::Plain, &text);
    if source.kind == c::OwnerAccountKind::SignIn
        && let Some(address) = &account.manage_usage
        && skein_ui::markdown::safe_url(address)
        && c::valid_manage_usage(address)
    {
        b.text(skein_ui::Run::Plain, &w.say(W::ToolSeparator, &[]));
        b.open(Role::Link);
        let text = w.say(W::Usage, &[]);
        b.name(&text);
        b.text(skein_ui::Run::Plain, &text);
        b.external(address);
        b.close();
    }
    b.close();
}
#[expect(clippy::manual_find, reason = "the programming model uses bounded loops, not closures")]
fn account_row(root: &Domain, account: u32) -> Option<&c::OwnerAccountLine> {
    let doc = temper_client_domain::pages::accounts::document(root.client())?;
    for row in &doc.accounts {
        if row.number == account {
            return Some(row);
        }
    }
    None
}
fn account_line(b: &mut Builder, root: &Domain, w: &Words, account: u32, labels: (W, W)) {
    match account_row(root, account) {
        Some(row) => {
            let name = match root.forge().demo() {
                Some(demo) if row.provider == Some(demo.provider) => w.say(W::DemoModelName, &[]),
                Some(_) | None => skein_lib::bytes::copy_of(row.display.bytes()),
            };
            line(b, w, labels.1, &[Hole::Name(name)]);
        }
        None => line(b, w, labels.0, &[Hole::Number(u64::from(account))]),
    }
}

fn provider(b: &mut Builder, root: &Domain, task: Option<&c::Task>, number: Option<u32>, w: &Words) {
    let Some(number) = number else { return };
    let Some(me) = temper_client_domain::pages::accounts::current_me(root.client()) else { return };
    for offer in &me.providers {
        if offer.provider == number {
            let configured = provider_shown(root, Some(number), offer.name.bytes(), w);
            let shown = crate::accounts::words::provider_words(&configured).0;
            line(b, w, W::ChoiceProvider, &[Hole::Name(skein_lib::bytes::copy_of(shown))]);
            break;
        }
    }
    let readiness = match temper_client_domain::pages::accounts::document(root.client()) {
        Some(doc) => readiness(doc, Some(number), payer(me, task)),
        None => c::Readiness::Unknown,
    };
    match readiness {
        c::Readiness::Unavailable => line(b, w, W::NeedsAccount, &[]),
        c::Readiness::Unknown => line(b, w, W::AccountReadinessUnknown, &[]),
        c::Readiness::Usable => {}
    }
}
fn editable(task: &c::Task) -> bool {
    match jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Amend) {
        Some(c::Admission::Allowed | c::Admission::Bounded { .. } | c::Admission::Proposed { .. }) => true,
        Some(c::Admission::Refused(_)) | None => false,
    }
}
pub(crate) fn waiting(
    b: &mut Builder,
    root: &Domain,
    wait: Option<c::TaskRunWait>,
    w: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    match wait {
        None => {}
        Some(c::TaskRunWait::Credential) => line(b, w, W::WaitCredential, &[]),
        Some(c::TaskRunWait::Refused(why)) => {
            line(b, w, W::WaitRefused, &[Hole::Name(refused(w, times, why))]);
        }
        Some(c::TaskRunWait::Host) => line(b, w, W::WaitCapacity, &[]),
        Some(c::TaskRunWait::Account { account, source, why }) => {
            account_wait(b, root, account, source, why, w, bindings);
        }
    }
}
fn sentences(w: &Words, first: Box<[u8]>, next: Box<[u8]>) -> Box<[u8]> {
    w.say(W::RefusalSentences, &[Hole::Name(first), Hole::Name(next)])
}
fn refused(w: &Words, times: &mut Times, why: c::TaskWaitRefused) -> Box<[u8]> {
    match why {
        c::TaskWaitRefused::Program(program) => {
            let (first, last) = match program {
                c::OpenProgram::Model => (w.say(W::OpenProgramModel, &[]), W::OpenProgramChooseModel),
                c::OpenProgram::Source { account, source } => (
                    w.say(W::OpenProgramSource, &[Hole::Number(u64::from(account)), Hole::Number(u64::from(source))]),
                    W::OpenProgramNoSwitch,
                ),
            };
            sentences(w, sentences(w, first, w.say(W::OpenProgramUnavailable, &[])), w.say(last, &[]))
        }
        c::TaskWaitRefused::Hello { kind, at, why } => {
            let word = match kind {
                c::OwnerHostKind::Engine => W::RefusedEngineAt,
                c::OwnerHostKind::Worker => W::RefusedWorkerAt,
            };
            let first = w.say(word, &[Hole::Name(times.say(w, TimeForm::Clock, at))]);
            sentences(w, first, hello_refusal(w, why))
        }
    }
}
fn hello_refusal(w: &Words, why: c::OwnerHelloRefusal) -> Box<[u8]> {
    match why {
        c::OwnerHelloRefusal::ReservedChannel => w.say(W::HelloReservedChannel, &[]),
        c::OwnerHelloRefusal::Position { worker } => w.say(W::HelloPosition, &[Hole::Number(u64::from(worker))]),
        c::OwnerHelloRefusal::Workers { limit } => {
            sentences(w, w.say(W::HelloWorkers, &[]), w.say(W::HelloLimit, &[Hole::Number(u64::from(limit))]))
        }
        c::OwnerHelloRefusal::StopBound { grace } => {
            sentences(w, w.say(W::HelloStopBound, &[]), w.say(W::HelloAllowedGrace, &[Hole::Span(grace)]))
        }
        c::OwnerHelloRefusal::Agent { why } => agent_shortfall(w, why),
    }
}
fn agent_shortfall(w: &Words, why: c::OwnerAgentShortfall) -> Box<[u8]> {
    use c::OwnerAgentShortfall as S;
    let word = match why {
        S::Model { .. } => W::AgentModel,
        S::Window { .. } => W::AgentWindow,
        S::Output { .. } => W::AgentOutput,
        S::Turns => W::AgentTurns,
        S::Spend => W::AgentSpend,
        S::Time => W::AgentTime,
        S::Waiting => W::AgentWaiting,
        S::Checks => W::AgentChecks,
        S::Message => W::AgentMessage,
        S::Inbox => W::AgentInbox,
        S::Unacknowledged => W::AgentUnacknowledged,
    };
    match why {
        S::Model { charter, model } | S::Window { charter, model } | S::Output { charter, model } => sentences(
            w,
            w.say(word, &[Hole::Number(u64::from(charter))]),
            w.say(W::AgentModelPosition, &[Hole::Number(u64::from(model))]),
        ),
        S::Turns | S::Spend | S::Time | S::Waiting | S::Checks | S::Message | S::Inbox | S::Unacknowledged => {
            w.say(word, &[])
        }
    }
}
fn account_wait(
    b: &mut Builder,
    root: &Domain,
    account: Option<u32>,
    source: Option<u32>,
    why: c::TaskWaitWhy,
    w: &Words,
    bindings: &mut Bindings,
) {
    use crate::accounts::words::{self, Word as A};
    use c::{TaskSourceState as S, TaskWaitWhy as R};
    b.open(Role::Status);
    b.style(crate::look::STATUS_ACTIVE);
    let name = w.say(W::AccountWaiting, &[]);
    b.name(&name);
    b.text(skein_ui::Run::Plain, &name);
    b.close();
    if let Some(account) = account {
        account_line(b, root, w, account, (W::WaitingAccount, W::WaitingAccountName));
    }
    if let Some(source) = source {
        b.open(Role::Paragraph);
        b.text(skein_ui::Run::Plain, &words::say(w, A::Source, &[]));
        b.text(skein_ui::Run::Plain, &words::say(w, A::Number, &[Hole::Number(u64::from(source))]));
        b.close();
    }
    match why {
        R::Source(S::Moving) => line(b, w, W::WaitMoving, &[]),
        R::Refreshing => line(b, w, W::WaitRefreshing, &[]),
        R::Host => line(b, w, W::WaitHost, &[]),
        R::Empty => {
            line(b, w, W::WaitEmpty, &[]);
            account_link(b, bindings, &w.say(W::AccountSignIn, &[]));
            account_link(b, bindings, &w.say(W::AccountKey, &[]));
        }
        R::Invalid(why) => {
            line(b, w, W::WaitInvalid, &[]);
            if let Some(account) = account {
                w.refusal(b, &c::Refusal::Account { account, why }, 0);
            }
        }
        R::Source(S::Limited { until }) => {
            source_wait(b, w, A::Limited, until);
            account_link(b, bindings, &words::say(w, A::Resume, &[]));
        }
        R::Source(S::Later { until }) => source_wait(b, w, A::Later, until),
        R::Source(S::Unloaded { at, why }) => {
            b.open(Role::Paragraph);
            crate::pages::accounts::unloaded(b, w, at, why);
            b.close();
        }
        R::Source(S::Asking) => source_wait(b, w, A::Asking, None),
        R::Source(S::Granted) => source_wait(b, w, A::Granted, None),
        R::Source(S::Refused) => {
            source_wait(b, w, A::Refused, None);
            account_link(b, bindings, &w.say(W::AccountKey, &[]));
        }
        R::Source(S::NeedsSignIn) => {
            source_wait(b, w, A::NeedsSignIn, None);
            account_link(b, bindings, &w.say(W::AccountSignIn, &[]));
            account_link(b, bindings, &w.say(W::AccountKey, &[]));
        }
        R::Source(S::Misconfigured) => source_wait(b, w, A::Misconfigured, None),
        R::Source(S::Cancelled) => source_wait(b, w, A::Cancelled, None),
        R::Source(S::Closing) => source_wait(b, w, A::Closing, None),
    }
}
fn source_wait(b: &mut Builder, w: &Words, word: crate::accounts::words::Word, until: Option<skein_lib::Wall>) {
    use crate::accounts::words::{self, Word as A};
    b.open(Role::Paragraph);
    b.text(skein_ui::Run::Plain, &words::say(w, word, &[]));
    if let Some(until) = until {
        b.text(skein_ui::Run::Plain, &words::say(w, A::Until, &[Hole::Wall(until)]));
    }
    b.close();
}
