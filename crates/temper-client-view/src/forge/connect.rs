//! `ConnectForge` and the welcome's shared forge body (client/forge.md, 5.4).
use super::bindings::{Bindings as ForgeBindings, Meaning};
use crate::{
    flow::{self, Control, Mark},
    frame::Bindings,
    look,
    words::{Word, Words},
};
use alloc::boxed::Box;
use skein_lib::bytes::copy_of;
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::{
    Address, Choice, Domain, Step,
    forge::{
        self,
        connect::{self, Name, Shown},
        forges::{Connection, FirstState, Kind, Mode, Offer, Row, SetUpAnswer, SetUpOutcome, SetUpProject, SetupField},
    },
    pages::Act,
};

#[derive(Clone, Copy)]
pub(crate) enum Place {
    Connect,
    Welcome,
}
impl Place {
    fn address(self, forge: Option<Choice>) -> Address {
        match self {
            Self::Connect => Address::ConnectForge { forge },
            Self::Welcome => Address::Welcome { step: Step::Forge { forge } },
        }
    }
}
pub(crate) fn text(b: &mut Builder, w: &Words, role: Role, word: Word) {
    crate::cards::text(b, role, &w.say(word, &[]));
}
pub(crate) fn link(b: &mut Builder, w: &Words, bindings: &mut Bindings, word: Word, address: Address) {
    let name = w.say(word, &[]);
    let to = bindings.insert(Act::Go { address });
    let action = if word == Word::ConnectForge {
        Control::PrimaryLink { name: &name, to }
    } else if word == Word::AddForge || word == Word::ForgeConnectApp {
        Control::SecondaryLink { name: &name, to }
    } else {
        Control::Link { name: &name, to }
    };
    flow::control(b, action);
}
pub(crate) fn name(w: &Words, row: &Row, root: &Domain) -> Box<[u8]> {
    w.say(
        match connect::name(row, root.forge().demo()) {
            Name::Here => Word::ForgeHere,
            Name::GitHub => Word::ForgeGitHub,
            Name::Forgejo => Word::ForgeOther,
            Name::Playground => Word::ForgePlayground,
        },
        &[],
    )
}
pub(crate) fn personal(root: &Domain) -> Option<&Row> {
    connect::personal(root.forges(), root.forge().demo())
}
pub(crate) fn underway(row: &Row) -> bool {
    connect::underway(row)
}
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    forge: &mut ForgeBindings,
    choice: Option<Choice>,
    sign_in: &crate::forge_sign_in::Forges,
) {
    b.open(Role::Main);
    b.style(look::MAIN);
    b.style(look::CONNECT_FORGE);
    text(b, w, Role::Heading { level: 1 }, Word::WelcomeForgeTitle);
    let shown = connect::step(choice, root.forges(), root.forge().demo());
    let connecting = match choice {
        Some(Choice::Elsewhere) => true,
        Some(Choice::Forge(number)) => match connect::row(number, root.forges(), root.forge().demo()) {
            Some(row) => row.kind == Kind::GitHub || row.mode != Mode::Owned,
            None => false,
        },
        None => false,
    };
    let choose = w.say(Word::ForgeChoose, &[]);
    let details = w.say(Word::ForgeDetails, &[]);
    let setting = w.say(if connecting { Word::ForgeConnecting } else { Word::ForgeSettingUp }, &[]);
    let ready = w.say(Word::ForgeReady, &[]);
    let current: usize = match shown {
        Shown::Choose => 0,
        Shown::Details(_) => 1,
        Shown::SettingUp(_) => 2,
        Shown::Ready(_) => 3,
    };
    let github = match choice {
        Some(Choice::Forge(number)) => match connect::row(number, root.forges(), root.forge().demo()) {
            Some(row) => row.kind == Kind::GitHub,
            None => false,
        },
        _ => false,
    };
    if github {
        flow::steps(b, w, &[&choose, &setting, &ready], current.saturating_sub(1));
    } else {
        flow::steps(b, w, &[&choose, &details, &setting, &ready], current);
    }
    match shown {
        Shown::Choose | Shown::Details(_) => body(b, root, w, bindings, forge, choice, Place::Connect),
        Shown::SettingUp(number) => {
            if let Some(row) = connect::row(number, root.forges(), root.forge().demo()) {
                setting_up(b, root, w, forge, row);
            }
        }
        Shown::Ready(number) => {
            if let Some(row) = connect::row(number, root.forges(), root.forge().demo()) {
                ready_card(b, root, w, bindings, forge, row, sign_in);
            }
        }
    }
    b.close();
}
pub(crate) fn body(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    forge: &mut ForgeBindings,
    choice: Option<Choice>,
    place: Place,
) {
    match connect::step(choice, root.forges(), root.forge().demo()) {
        Shown::Choose => choose(b, root, w, bindings, forge, place),
        Shown::Details(choice) => {
            if let Place::Connect = place {
                link(b, w, bindings, Word::ForgeAll, place.address(None));
            }
            match choice {
                Choice::Elsewhere => elsewhere(b, root, w),
                Choice::Forge(number) => {
                    if let Some(row) = connect::row(number, root.forges(), root.forge().demo()) {
                        match row.kind {
                            Kind::GitHub => github(b, root, w, forge, row),
                            Kind::Forgejo if row.admin => admin_details(b, root, w, forge, row),
                            Kind::Forgejo => here(b, root, w, forge, row),
                        }
                    }
                }
            }
        }
        Shown::SettingUp(_) | Shown::Ready(_) => {}
    }
}
/// The welcome keeps GitHub's installation and Ready steps in its own flow.
pub(crate) fn welcome(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    forge: &mut ForgeBindings,
    choice: Option<Choice>,
    sign_in: &crate::forge_sign_in::Forges,
) {
    if let Some(row) = personal(root) {
        if row.admin {
            match connect::step(Some(Choice::Forge(row.number)), root.forges(), root.forge().demo()) {
                Shown::Details(_) => admin_details(b, root, w, forge, row),
                Shown::Ready(_) => ready_card(b, root, w, bindings, forge, row, sign_in),
                Shown::SettingUp(_) => setting_up(b, root, w, forge, row),
                Shown::Choose => {}
            }
        } else if row.kind == Kind::GitHub
            && let Connection::Connected { .. } = row.connection
        {
            if forge::forges::accepted(row) {
                ready_card(b, root, w, bindings, forge, row, sign_in);
            } else {
                setting_up(b, root, w, forge, row);
            }
        } else {
            status(b, root, w, row, false);
            first_line(b, root, w, row);
            if connect::failed(row) {
                progress_in(b, Some(root), w, row);
            }
            link(b, w, bindings, Word::ForgeView, Address::ConnectForge { forge: Some(Choice::Forge(row.number)) });
        }
    } else {
        body(b, root, w, bindings, forge, choice, Place::Welcome);
    }
}
fn choose(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings, forge: &mut ForgeBindings, place: Place) {
    b.open(Role::Slot);
    b.style(look::FLOW_CHOICES);
    for choice in &connect::choices(root.forges(), root.forge().demo()) {
        if let Choice::Forge(number) = *choice
            && let Some(row) = connect::row(number, root.forges(), root.forge().demo())
            && row.admin
        {
            // The welcome draws its mockup's three choices; a configured admin
            // Forgejo's card is Connect a forge's alone (client/forge.md, 5.4).
            if let Place::Welcome = place {
                continue;
            }
            flow::choice(
                b,
                &admin_host(w, row),
                &w.say(Word::AdminCard, &[]),
                b"",
                Control::Link {
                    name: &w.say(Word::AdminTitle, &[]),
                    to: bindings.insert(Act::Go { address: place.address(Some(*choice)) }),
                },
            );
            continue;
        }
        let (heading, words, note, action) = match *choice {
            Choice::Elsewhere => {
                (Word::ForgeElsewhere, Word::ForgeElsewhereDescription, Word::ForgeElsewhereTime, Word::ForgeShowHow)
            }
            Choice::Forge(number) => {
                let Some(row) = connect::row(number, root.forges(), root.forge().demo()) else { continue };
                match row.kind {
                    Kind::GitHub => {
                        (Word::ForgeGitHub, Word::ForgeGitHubDescription, Word::ForgeGitHubTime, Word::ForgeChoose)
                    }
                    Kind::Forgejo => (
                        Word::ForgeHere,
                        Word::ForgeHereDescription,
                        if row.offer == Offer::NotUnderTheBox { Word::ForgeCommandsTime } else { Word::ForgeHereTime },
                        Word::ForgeChoose,
                    ),
                }
            }
        };
        if heading == Word::ForgeGitHub {
            flow::choice_card(b, &w.say(heading, &[]));
            text(b, w, Role::Paragraph, words);
            b.open(Role::Paragraph);
            b.style(look::FLOW_MUTED);
            b.name(&w.say(note, &[]));
            w.text(b, note, &[]);
            b.close();
            b.open(Role::Slot);
            b.style(look::FLOW_ACTIONS);
            if let Choice::Forge(number) = *choice {
                app_control(b, root, w, forge, number, Word::ForgeConnectGitHub, Some(place.address(Some(*choice))));
            }
            link(b, w, bindings, Word::ForgeUseApp, place.address(Some(*choice)));
            b.close();
            b.close();
            continue;
        }
        flow::choice(
            b,
            &w.say(heading, &[]),
            &w.say(words, &[]),
            &w.say(note, &[]),
            Control::SecondaryLink {
                name: &w.say(action, &[]),
                to: bindings.insert(Act::Go { address: place.address(Some(*choice)) }),
            },
        );
    }
    b.close();
}
fn github(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut ForgeBindings, row: &Row) {
    flow::details(b, &w.say(Word::ForgeAppTitle, &[]));
    text(b, w, Role::Paragraph, Word::ForgeAppDescription);
    crate::cards::text(b, Role::Paragraph, &w.say(Word::ForgeSetupUrl, &[Hole::Number(u64::from(row.number))]));
    if w.shell == crate::words::Shell::Terminal {
        crate::cards::text(b, Role::Paragraph, &forge::words::app_terminal(row.number));
    } else {
        let draft = root.forge().app_form();
        for (secret, word, meaning, required) in [
            (false, Word::ForgeAppId, Meaning::AppId, draft.id.is_empty()),
            (true, Word::ForgePrivateKey, Meaning::AppKey, !draft.has_key() && !draft.too_long),
        ] {
            b.open(Role::Slot);
            b.style(look::FLOW_FIELD);
            b.open(Role::TextField);
            b.name(&w.say(word, &[]));
            b.bind(bindings.insert(meaning));
            if secret {
                b.entry(skein_ui::EntryKind::Secret);
                b.value(b"", draft.key_written);
            } else {
                b.value(&draft.id, draft.written);
            }
            if draft.checked && (required || (!secret && draft.number().is_none())) {
                b.state(State::Invalid);
            }
            b.close();
            if draft.checked && required {
                text(b, w, Role::Alert, Word::SetupRequired);
            } else if draft.checked && !secret && draft.number().is_none() {
                crate::cards::text(b, Role::Alert, forge::words::APP_ID);
            }
            b.close();
        }
        app_control(b, root, w, bindings, row.number, Word::ForgeConnectAppButton, None);
        if draft.checked && draft.too_long {
            crate::cards::text(b, Role::Alert, forge::words::APP_TOO_LONG);
        }
        text(b, w, Role::Paragraph, Word::ForgePasted);
    }
    if let Some(line) = forge::words::last_connect(row) {
        crate::cards::text(b, Role::Alert, &line);
    }
    b.close();
}
pub(crate) fn app_control(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut ForgeBindings,
    number: u16,
    word: Word,
    back: Option<Address>,
) {
    // The first installation is primary on Forges too; another account is maintenance.
    let primary = word != Word::ForgeInstallAnother;
    let pending = root.forge().app();
    let busy = match pending {
        Some(pending) => pending.answer.is_none(),
        None => false,
    };
    let name = w.say(word, &[]);
    if busy || root.forge().served() == forge::Served::OutOfDate {
        flow::control(
            b,
            if primary { Control::PrimaryDisabled { name: &name } } else { Control::Disabled { name: &name } },
        );
    } else {
        let meaning = match back {
            Some(back) if word == Word::ForgeConnectGitHub => Meaning::ConnectManifest { forge: number, back },
            Some(back) => Meaning::Install { forge: number, back },
            None => Meaning::ConnectApp(number),
        };
        let to = bindings.insert(meaning);
        flow::control(
            b,
            if primary { Control::PrimaryButton { name: &name, to } } else { Control::Button { name: &name, to } },
        );
    }
    if let Some(pending) = pending
        && pending.forge == number
        && let Some(answer) = &pending.answer
    {
        if let Some(words) = forge::words::app_refusal(answer.clone(), number) {
            crate::cards::text(b, Role::Alert, &words);
        }
        if let forge::github::Answer::Entrance(refusal) = answer {
            b.open(Role::Alert);
            w.refusal(b, refusal.refusal(), w.name_bytes);
            b.close();
        }
        // A path's departure is never refused, so only a whole address needs a link meanwhile.
        if let forge::github::Answer::Link(address) = answer
            && !address.starts_with(b"/")
        {
            b.open(Role::Link);
            b.name(&name);
            b.text(Run::Plain, &name);
            b.external(address);
            b.style(if primary { look::FLOW_PRIMARY } else { look::FLOW_SECONDARY });
            b.close();
        }
    }
}
fn here(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut ForgeBindings, row: &Row) {
    let terminal = row.offer == Offer::NotUnderTheBox;
    flow::details(b, &w.say(if terminal { Word::ForgeTerminal } else { Word::ForgeHere }, &[]));
    if terminal {
        text(b, w, Role::Paragraph, Word::ForgeTerminalDetails);
        let commands = forge::words::set_up_commands(row.number);
        let mut rest = commands.as_ref();
        b.open(Role::Paragraph);
        b.style(look::FLOW_CODE);
        // The shared forge words delimit exactly the four terminal commands with backticks.
        for index in 0_u32..4 {
            let Some(start) = skein_lib::bytes::find(rest, b"`") else { break };
            let tail = rest.get(start..).expect("command start").strip_prefix(b"`").expect("backtick");
            let Some(end) = skein_lib::bytes::find(tail, b"`") else { break };
            if index != 0 {
                b.text(Run::Code, b"\n");
            }
            b.text(Run::Code, tail.get(..end).expect("command end"));
            rest = tail.get(end..).expect("command end").strip_prefix(b"`").expect("backtick");
        }
        b.close();
        text(b, w, Role::Paragraph, Word::ForgeTerminalNext);
    } else {
        if let Some(owned) = &row.owned {
            crate::cards::text(
                b,
                Role::Paragraph,
                &w.say(Word::ForgeHereVersionDetails, &[Hole::Name(copy_of(&owned.forgejo))]),
            );
        } else {
            text(b, w, Role::Paragraph, Word::ForgeHereDetails);
        }
        setup_fields(b, root, w, bindings, row.number);
        b.open(Role::List);
        b.style(look::FLOW_FACTS);
        for note in [Word::ForgeIsolation, Word::ForgeStorage] {
            text(b, w, Role::Item, note);
        }
        b.close();
        setup_control(b, root, w, bindings, row.number, Word::ForgeSetUp);
    }
    b.close();
}
fn code(b: &mut Builder, words: &[u8]) {
    b.open(Role::Paragraph);
    b.style(look::FLOW_CODE);
    b.text(Run::Code, words);
    b.close();
}
fn elsewhere(b: &mut Builder, root: &Domain, w: &Words) {
    flow::details(b, &w.say(Word::ForgeElsewhere, &[]));
    text(b, w, Role::Paragraph, Word::ForgeElsewhereDetails);
    let number = u64::from(connect::next_number(root.forges()));
    for (word, snippet) in [
        (Word::ForgeConfig, Word::ForgeConfigCode),
        (Word::ForgeIdentity, Word::ForgeIdentityCode),
        (Word::ForgeSetupEntry, Word::ForgeSetupCode),
    ] {
        text(b, w, Role::Paragraph, word);
        let holes: &[Hole] = if snippet == Word::ForgeIdentityCode {
            &[Hole::Number(number), Hole::Number(number)]
        } else {
            &[Hole::Number(number)]
        };
        code(b, &w.say(snippet, holes));
    }
    for word in [Word::ForgeBotOnly, Word::ForgeRestart] {
        text(b, w, Role::Paragraph, word);
    }
    b.close();
}
pub(crate) fn setup_control(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut ForgeBindings,
    number: u16,
    word: Word,
) {
    let primary = word == Word::ForgeSetUp;
    let pending = match root.forge().set_up() {
        Some(last) => last.progress.is_some(),
        None => false,
    };
    let name = w.say(word, &[]);
    if pending || root.forge().served() == forge::Served::OutOfDate {
        flow::control(
            b,
            if primary { Control::PrimaryDisabled { name: &name } } else { Control::Disabled { name: &name } },
        );
    } else {
        let to = bindings.insert(Meaning::SetUp(number));
        flow::control(
            b,
            if primary { Control::PrimaryButton { name: &name, to } } else { Control::Button { name: &name, to } },
        );
    }
    if let Some(last) = root.forge().set_up()
        && last.forge == number
        && let Some(answer) = &last.answer
    {
        match answer {
            SetUpAnswer::Outcome(SetUpOutcome::Refused(why)) => {
                crate::cards::text(b, Role::Alert, forge::words::set_up_refusal(*why));
            }
            SetUpAnswer::EntranceRefused(refusal) => {
                b.open(Role::Alert);
                w.refusal(b, refusal.refusal(), w.name_bytes);
                b.close();
            }
            SetUpAnswer::Outcome(SetUpOutcome::SettingUp | SetUpOutcome::Unknown) => {}
        }
    }
}
pub(crate) fn progress_in(b: &mut Builder, root: Option<&Domain>, w: &Words, row: &Row) {
    if row.kind == Kind::GitHub {
        flow::progress(
            b,
            w,
            &w.say(Word::ForgeProgress, &[]),
            &[(
                &forge::words::forge_connection(row),
                if forge::forges::accepted(row) { Mark::Done } else { Mark::UnderWay },
            )],
        );
        return;
    }
    let failed = connect::failed_step(row);
    let failed_index = match failed {
        Some(connect::FailedStep::Fetch) => Some(0),
        Some(connect::FailedStep::Prepare) => Some(1),
        Some(connect::FailedStep::Run) => Some(2),
        Some(connect::FailedStep::Connect) => Some(3),
        Some(connect::FailedStep::Repository) => Some(4),
        None => None,
    };
    let current = if connect::failed(row) {
        failed_index
    } else {
        match row.connection {
            Connection::SettingUp { step: Some(forge::forges::Step::Fetch) } => Some(0),
            Connection::SettingUp { step: Some(forge::forges::Step::Prepare) } => Some(1),
            Connection::SettingUp { step: Some(forge::forges::Step::Run) } => Some(2),
            Connection::Connecting => Some(3),
            Connection::Connected { .. } => Some(match root {
                Some(root) if adopted_project(root, row).is_some() => 5,
                Some(_) | None => 4,
            }),
            Connection::SettingUp { step: None }
            | Connection::NotConnected
            | Connection::NotSetUp
            | Connection::SetUpFailed { .. }
            | Connection::Stopped { .. } => None,
        }
    };
    let fetch = if let Some(owned) = &row.owned
        && let Some(runner) = &owned.runner
    {
        w.say(
            if current > Some(0) { Word::ForgeFetchedVersions } else { Word::ForgeFetchVersions },
            &[Hole::Name(copy_of(&owned.forgejo)), Hole::Name(copy_of(runner))],
        )
    } else {
        w.say(if current > Some(0) { Word::ForgeFetched } else { Word::ForgeFetch }, &[])
    };
    let prepare = w.say(Word::ForgePrepare, &[]);
    let run = w.say(Word::ForgeRun, &[]);
    let connecting = w.say(Word::ForgeConnectingAsTemper, &[]);
    let target: Option<(&[u8], &[u8], &SetUpProject)> = match &row.first {
        Some(first) => Some((&first.org, &first.repository, &first.project)),
        None => match root {
            Some(root) => setup_target(root, row),
            None => None,
        },
    };
    let making = match target {
        Some((org, repository, project)) => {
            let project = match root {
                Some(root) => project_name(root, w, project),
                None => match project {
                    SetUpProject::New(name) => copy_of(name),
                    SetUpProject::Existing(project) => w.say(Word::Project, &[Hole::Number(u64::from(*project))]),
                },
            };
            w.say(
                Word::ForgeMakingAndAdopting,
                &[Hole::Name(forge::words::full_name(org, repository)), Hole::Name(project)],
            )
        }
        None => w.say(Word::ForgeMakingFirst, &[]),
    };
    if current.is_none()
        && let Connection::SettingUp { step: None } = row.connection
    {
        text(b, w, Role::Status, Word::FlowUnderWay);
    }
    flow::progress(
        b,
        w,
        &w.say(Word::ForgeProgress, &[]),
        &[
            (&fetch, stage_mark(current, failed_index, 0)),
            (&prepare, stage_mark(current, failed_index, 1)),
            (&run, stage_mark(current, failed_index, 2)),
            (&connecting, stage_mark(current, failed_index, 3)),
            (&making, stage_mark(current, failed_index, 4)),
        ],
    );
    if connect::failed(row) {
        failure(b, w, row);
    }
}
fn stage_mark(current: Option<usize>, failed: Option<usize>, index: usize) -> Mark {
    if failed == Some(index) {
        return Mark::Failed;
    }
    match current {
        Some(current) => mark(current, index),
        None => Mark::ToCome,
    }
}
fn setup_target<'a>(root: &'a Domain, row: &Row) -> Option<(&'a [u8], &'a [u8], &'a SetUpProject)> {
    match root.forge().set_up() {
        Some(last) if last.forge == row.number => Some((&last.org, &last.repository, &last.project)),
        Some(_) | None => None,
    }
}

fn mark(current: usize, index: usize) -> Mark {
    match index.cmp(&current) {
        core::cmp::Ordering::Less => Mark::Done,
        core::cmp::Ordering::Equal => Mark::UnderWay,
        core::cmp::Ordering::Greater => Mark::ToCome,
    }
}
pub(crate) fn setting_up(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut ForgeBindings, row: &Row) {
    if row.admin {
        flow::card(b, &w.say(Word::AdminConnecting, &[Hole::Number(u64::from(row.number))]));
        let repository = match &row.first {
            Some(first) => forge::words::full_name(&first.org, &first.repository),
            None => {
                let draft = root.forge().admin_form();
                forge::words::full_name(&draft.fields[0], &draft.fields[1])
            }
        };
        crate::cards::text(b, Role::Paragraph, &w.say(Word::AdminMaking, &[Hole::Name(repository)]));
        b.close();
        return;
    }
    if row.kind == Kind::GitHub {
        if forge::forges::slug(row).is_some() {
            flow::details(b, &w.say(Word::ForgeInstallTitle, &[]));
            if row.app == Some(forge::forges::AppWay::Manifest) {
                let slug = forge::forges::slug(row).expect("connected App slug");
                crate::cards::text(
                    b,
                    Role::Paragraph,
                    &w.say(Word::ForgeManifestInstall, &[Hole::Name(copy_of(slug))]),
                );
            } else {
                crate::cards::text(b, Role::Paragraph, &forge::words::app_install(row));
            }
            app_control(b, root, w, bindings, row.number, Word::ForgeInstall, Some(root.pages().address().clone()));
        } else {
            flow::details(b, &w.say(Word::ForgeConnecting, &[]));
            text(b, w, Role::Paragraph, Word::ForgeLeave);
        }
        if let Some(line) = forge::words::last_connect(row) {
            crate::cards::text(b, Role::Alert, &line);
        }
        b.close();
        return;
    }
    flow::details(b, &w.say(Word::ForgeSettingUpTitle, &[]));
    first_line(b, root, w, row);
    progress_in(b, Some(root), w, row);
    if connect::failed(row) {
        setup_control(b, root, w, bindings, row.number, Word::ForgeAgain);
    } else {
        text(b, w, Role::Paragraph, Word::ForgeLeave);
    }
    b.close();
}
pub(crate) fn ready_card(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    forge: &mut ForgeBindings,
    row: &Row,
    sign_in: &crate::forge_sign_in::Forges,
) {
    if row.admin {
        admin_ready(b, root, w, bindings, row);
        return;
    }
    if let Some(first) = &row.first {
        let repository = forge::words::full_name(&first.org, &first.repository);
        let project = project_name(root, w, &first.project);
        flow::details(b, &w.say(Word::SetupReady, &[Hole::Name(repository)]));
        flow::subtitle(b, &w.say(Word::SetupIn, &[Hole::Name(project.clone()), Hole::Name(name(w, row, root))]));
        progress_in(b, Some(root), w, row);
        let pending = match root.forge().set_up() {
            Some(last) if last.forge == row.number => match &last.adoption {
                Some(adoption) => adoption.outcome.is_none(),
                None => true,
            },
            Some(_) | None => false,
        };
        let open = w.say(Word::SetupOpen, &[Hole::Name(project)]);
        if pending {
            flow::control(b, Control::PrimaryDisabled { name: &open });
        } else {
            let project = match root.forge().set_up() {
                Some(last) if last.forge == row.number => match &last.adoption {
                    Some(adoption) => match &adoption.outcome {
                        Some(forge::Answer::Adopted(_)) => Some(adoption.value.project),
                        Some(
                            forge::Answer::CoreRefused(_)
                            | forge::Answer::ForgeRefused(_)
                            | forge::Answer::EntranceRefused(_)
                            | forge::Answer::Unknown,
                        )
                        | None => None,
                    },
                    None => None,
                },
                Some(_) | None => match first.project {
                    SetUpProject::Existing(project) => Some(project),
                    SetUpProject::New(_) => None,
                },
            };
            if let Some(project) = project {
                flow::control(
                    b,
                    Control::PrimaryLink { name: &open, to: bindings.insert(Act::OpenProject { project }) },
                );
            }
        }
        if !pending && adopted_project(root, row).is_some() {
            super::forges::open_link(b, w, row, sign_in, Some(look::FLOW_QUIET));
        }
        setup_ending(b, root, w, forge, row);
    } else if row.kind == Kind::GitHub {
        if row.repositories.is_empty() {
            github_picker(b, root, w, forge, row);
        } else {
            github_adopted(b, root, w, bindings, row);
        }
    } else {
        flow::details(b, &w.say(Word::ForgeConnectedTitle, &[Hole::Name(name(w, row, root))]));
        if let Connection::Connected { login, .. } = &row.connection {
            crate::cards::text(b, Role::Paragraph, &w.say(Word::ForgeConnectedAs, &[Hole::Name(copy_of(login))]));
        }
        app_line(b, w, row);
        adopt(b, root, w, bindings);
    }
    b.open(Role::Slot);
    b.style(look::FLOW_SUBTITLE);
    w.text(b, Word::ForgeOnForges, &[Hole::Name(name(w, row, root))]);
    link(b, w, bindings, Word::ForgesTitle, Address::Forges);
    w.text(b, Word::ForgeOnForgesTail, &[]);
    b.close();
    b.close();
}
/// Completion follows the durable row, including after a reload.
/// The row's adoptions last after a reload and in other tabs; the originating tab's Adopt answer
/// counts before the row is next published.
pub(crate) fn adopted_project(root: &Domain, row: &Row) -> Option<u32> {
    match row.connection {
        Connection::Connected { .. } => {}
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SettingUp { .. }
        | Connection::SetUpFailed { .. }
        | Connection::Connecting
        | Connection::Stopped { .. } => return None,
    }
    if row.kind == Kind::GitHub {
        return Some(row.repositories.first()?.project);
    }
    let first = row.first.as_ref()?;
    match first.state {
        FirstState::Made { .. } => {}
        FirstState::Making | FirstState::Failed { .. } => return None,
    }
    for adoption in &row.repositories {
        if adoption.owner == first.org && adoption.name == first.repository {
            return Some(adoption.project);
        }
    }
    if let Some(last) = root.forge().set_up()
        && last.forge == row.number
        && let Some(adoption) = &last.adoption
        && let Some(forge::Answer::Adopted(_)) = &adoption.outcome
    {
        return Some(adoption.value.project);
    }
    None
}
/// Refusals remain actionable from the welcome's checklist as well as `ConnectForge`'s card.
pub(crate) fn setup_ending(b: &mut Builder, root: &Domain, w: &Words, forge: &mut ForgeBindings, row: &Row) {
    if connect::failed(row) {
        setup_control(b, root, w, forge, row.number, Word::ForgeAgain);
    }
    if let Some(first) = &row.first {
        if let Some(last) = root.forge().set_up()
            && last.forge == row.number
            && let Some(starting) = &last.starting
            && let Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(
                jig_client::contract::Answer::Refused { refusal, .. },
            ))) = &starting.value.outcome
        {
            b.open(Role::Alert);
            w.refusal(b, refusal, w.name_bytes);
            b.close();
            setup_field(b, root, w, forge, SetupField::Project, Word::SetupNew, row.number);
            let label = w.say(Word::SetupAdopt, &[Hole::Name(forge::words::full_name(&first.org, &first.repository))]);
            flow::control(b, Control::PrimaryButton { name: &label, to: forge.insert(Meaning::StartIn(row.number)) });
        }
        if let Some(last) = root.forge().set_up()
            && last.forge == row.number
            && let Some(adoption) = &last.adoption
            && let Some(answer) = &adoption.outcome
        {
            match answer {
                forge::Answer::Adopted(_) => {}
                forge::Answer::CoreRefused(_)
                | forge::Answer::ForgeRefused(_)
                | forge::Answer::EntranceRefused(_)
                | forge::Answer::Unknown => {
                    adoption_outcome(b, w, &adoption.value, answer);
                }
            }
        }
    }
}
pub(crate) fn adopt(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings) {
    if let Some(project) = root.pages().project() {
        link(b, w, bindings, Word::ForgeAdopt, Address::Adopt { project });
    }
}
pub(crate) fn status(b: &mut Builder, root: &Domain, w: &Words, row: &Row, ready: bool) {
    let word = if ready { Word::WelcomeForgeStatus } else { Word::ForgeStatus };
    crate::cards::text(
        b,
        Role::Paragraph,
        &w.say(word, &[Hole::Name(name(w, row, root)), Hole::Name(forge::words::forge_connection(row))]),
    );
}
/// The welcome uses GitHub's durable adoption, or the first-repository setup's target.
/// The asking tab also knows it made a new project; a durable adoption alone does not.
pub(crate) fn welcome_target<'a>(root: &'a Domain, row: &'a Row) -> Option<(&'a [u8], &'a [u8], SetUpProject)> {
    if row.kind == Kind::GitHub {
        let adopted = row.repositories.first()?;
        let project = if let Some(picker) = root.forge().picker(row.number)
            && let Some(starting) = &picker.starting
            && starting.value.adoption.project == adopted.project
        {
            SetUpProject::New(starting.value.name.clone())
        } else {
            SetUpProject::Existing(adopted.project)
        };
        return Some((&adopted.owner, &adopted.name, project));
    }
    if let Some(first) = &row.first {
        return Some((&first.org, &first.repository, first.project.clone()));
    }
    let last = root.forge().set_up()?;
    if last.forge == row.number { Some((&last.org, &last.repository, last.project.clone())) } else { None }
}
/// The welcome's Model step says what Forge did (client/README.md, 3.5).
pub(crate) fn welcome_line(b: &mut Builder, root: &Domain, w: &Words, row: &Row) {
    let connected = match row.connection {
        Connection::Connected { .. } => true,
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SettingUp { .. }
        | Connection::SetUpFailed { .. }
        | Connection::Connecting
        | Connection::Stopped { .. } => false,
    };
    let done = if row.kind == Kind::GitHub {
        adopted_project(root, row).is_some()
    } else {
        match &row.first {
            Some(first) => match first.state {
                FirstState::Made { .. } => connected,
                FirstState::Making | FirstState::Failed { .. } => false,
            },
            None => false,
        }
    };
    let target = welcome_target(root, row);
    let forge = name(w, row, root);
    let line = if connected && !underway(row) && target.is_none() {
        w.say(Word::WelcomeForgeNoRepository, &[Hole::Name(forge)])
    } else if let Some((org, repository, project)) = target {
        let project = match &project {
            SetUpProject::New(name) => w.say(Word::WelcomeNewProject, &[Hole::Name(copy_of(name))]),
            SetUpProject::Existing(_) => project_name(root, w, &project),
        };
        let repository = forge::words::full_name(org, repository);
        if done {
            w.say(Word::WelcomeForgeDone, &[Hole::Name(forge), Hole::Name(repository), Hole::Name(project)])
        } else {
            w.say(
                Word::WelcomeForgeLine,
                &[
                    Hole::Name(forge),
                    Hole::Name(forge::words::forge_connection(row)),
                    Hole::Name(repository),
                    Hole::Name(project),
                ],
            )
        }
    } else {
        w.say(Word::ForgeStatus, &[Hole::Name(forge), Hole::Name(forge::words::forge_connection(row))])
    };
    b.open(Role::Paragraph);
    if done {
        b.style(look::FLOW_DONE);
    }
    b.name(&line);
    if done {
        b.text(Run::Plain, "✓ ".as_bytes());
    }
    b.text(Run::Plain, &line);
    b.close();
    if !done {
        each_step(b, root, w, row);
    }
}
pub(crate) fn each_step(b: &mut Builder, root: &Domain, w: &Words, row: &Row) {
    b.open(Role::Disclosure);
    b.name(&w.say(Word::ForgeProgress, &[]));
    progress_in(b, Some(root), w, row);
    b.close();
}

fn setup_fields(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut ForgeBindings, number: u16) {
    b.open(Role::Slot);
    b.style(look::FLOW_PAIR);
    for (field, word) in
        [(SetupField::Organization, Word::SetupOrganization), (SetupField::Repository, Word::SetupRepository)]
    {
        setup_field(b, root, w, bindings, field, word, number);
    }
    b.close();
    b.open(Role::Slot);
    b.style(look::FLOW_PAIR);
    setup_field(b, root, w, bindings, SetupField::Project, Word::SetupNew, number);
    b.close();
}
fn setup_field(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut ForgeBindings,
    field: SetupField,
    word: Word,
    number: u16,
) {
    let draft = root.forge().setup_form();
    let invalid = match root.forge().set_up() {
        Some(last) if last.forge == number => {
            last.answer == Some(SetUpAnswer::Outcome(SetUpOutcome::Refused(forge::forges::SetUpRefusal::Invalid)))
        }
        Some(_) | None => false,
    };
    b.open(Role::Slot);
    b.style(look::FLOW_FIELD);
    b.open(Role::TextField);
    b.name(&w.say(word, &[]));
    b.value(draft.text(field), draft.written(field));
    b.bind(bindings.insert(Meaning::SetupField(field)));
    if draft.required(field) || (invalid && field != SetupField::Project) {
        b.state(State::Invalid);
    }
    b.close();
    if draft.required(field) {
        text(b, w, Role::Alert, Word::SetupRequired);
    }
    b.close();
}
pub(crate) fn project_name(root: &Domain, w: &Words, project: &SetUpProject) -> Box<[u8]> {
    match project {
        SetUpProject::Existing(project) => {
            match crate::pages::project_name(root.pages().me(root.client()), root.forge().demo(), *project, w) {
                Some(name) => name,
                None => w.say(Word::Project, &[Hole::Number(u64::from(*project))]),
            }
        }
        SetUpProject::New(name) => {
            if let Some(last) = root.forge().set_up()
                && let Some(starting) = &last.starting
                && last.project == *project
            {
                if starting.value.adoption.project != 0
                    && let Some(name) = crate::pages::project_name(
                        root.pages().me(root.client()),
                        root.forge().demo(),
                        starting.value.adoption.project,
                        w,
                    )
                {
                    return name;
                }
                return starting.value.name.clone();
            }
            copy_of(name)
        }
    }
}
pub(crate) fn first_line(b: &mut Builder, root: &Domain, w: &Words, row: &Row) {
    let (org, repository, project) = match &row.first {
        Some(first) => (&first.org, &first.repository, &first.project),
        None => match root.forge().set_up() {
            Some(last) if last.forge == row.number => (&last.org, &last.repository, &last.project),
            Some(_) | None => return,
        },
    };
    flow::subtitle(
        b,
        &w.say(
            Word::SetupInto,
            &[Hole::Name(forge::words::full_name(org, repository)), Hole::Name(project_name(root, w, project))],
        ),
    );
}
fn adoption_outcome(b: &mut Builder, w: &Words, adoption: &forge::Adoption, answer: &forge::Answer) {
    // Reuse the adoption page's outcome, with the submitted fields.
    let mut form = forge::form(adoption.project);
    form.texts.get_mut(forge::Text::Forge.index()).expect("forge field").text =
        Box::from(skein_lib::Decimal::of(u64::from(adoption.forge)).as_bytes());
    form.texts.get_mut(forge::Text::Owner.index()).expect("owner field").text.clone_from(&adoption.owner);
    form.texts.get_mut(forge::Text::Name.index()).expect("name field").text.clone_from(&adoption.name);
    super::page::outcome(b, &form, answer, w, w.name_bytes);
}

pub(crate) fn app_line(b: &mut Builder, w: &Words, row: &Row) {
    if let Some(slug) = forge::forges::slug(row) {
        b.open(Role::Slot);
        text(b, w, Role::Term, Word::ForgeAppLabel);
        b.open(Role::Description);
        crate::cards::text(b, Role::Slot, &forge::words::app_head(slug, row.installations.is_empty()));
        for (index, installation) in row.installations.iter().enumerate() {
            crate::cards::text(b, Role::Slot, &forge::words::app_installation(installation, index == 0, row.read_only));
        }
        b.close();
        b.close();
    }
}

fn picker_control(
    b: &mut Builder,
    forge: &mut ForgeBindings,
    number: u16,
    label: &[u8],
    act: forge::picker::Act,
    disabled: bool,
) {
    let primary = match act {
        forge::picker::Act::Adopt => true,
        forge::picker::Act::Account(_)
        | forge::picker::Act::Choose(_)
        | forge::picker::Act::Page(_)
        | forge::picker::Act::Write(_) => false,
    };
    if disabled {
        flow::control(
            b,
            if primary { Control::PrimaryDisabled { name: label } } else { Control::Disabled { name: label } },
        );
    } else {
        let to = forge.insert(Meaning::Picker { forge: number, act });
        flow::control(
            b,
            if primary { Control::PrimaryButton { name: label, to } } else { Control::Button { name: label, to } },
        );
    }
}
fn github_picker(b: &mut Builder, root: &Domain, w: &Words, forge: &mut ForgeBindings, row: &Row) {
    use forge::picker::Act;
    flow::details(b, &w.say(Word::GitHubPicker, &[]));
    let picker = root.forge().picker(row.number);
    let installation = match picker {
        Some(picker) => forge::picker::installation(&row.installations, Some(picker.installation)),
        None => forge::picker::installation(&row.installations, None),
    };
    if let Some(installation) = installation
        && let Some(slug) = forge::forges::slug(row)
    {
        let account = match &installation.account {
            Some(account) => copy_of(&account.login),
            None => Box::from(skein_lib::Decimal::of(installation.id).as_bytes()),
        };
        crate::cards::text(
            b,
            Role::Paragraph,
            &w.say(Word::GitHubInstalled, &[Hole::Name(copy_of(slug)), Hole::Name(account)]),
        );
    }
    let Some(picker) = picker else {
        text(b, w, Role::Status, Word::GitHubReading);
        return;
    };
    let disabled = picker.pending() || root.forge().served() == forge::Served::OutOfDate;
    github_choices(b, w, forge, row, picker, disabled);
    b.open(Role::TextField);
    b.name(&w.say(Word::SetupNew, &[]));
    b.value(&picker.name, picker.written);
    b.bind(forge.insert(Meaning::PickerName(row.number)));
    if disabled {
        b.state(State::Disabled);
    }
    if picker.checked && picker.name.is_empty() {
        b.state(State::Invalid);
    }
    b.close();
    if picker.checked && picker.name.is_empty() {
        text(b, w, Role::Alert, Word::SetupRequired);
    }
    if let Some(selected) = picker.selected() {
        let label =
            w.say(Word::SetupAdopt, &[Hole::Name(forge::words::full_name(&selected.owner.login, &selected.name))]);
        picker_control(b, forge, row.number, &label, Act::Adopt, disabled);
    }
    if let Some(starting) = &picker.starting
        && let Some(outcome) = &starting.value.outcome
    {
        match outcome {
            jig_client::Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Refused {
                refusal,
                ..
            })) => {
                b.open(Role::Alert);
                w.refusal(b, refusal, w.name_bytes);
                b.close();
            }
            jig_client::Outcome::Answered(_) => {}
            jig_client::Outcome::NotKept
            | jig_client::Outcome::Unknown
            | jig_client::Outcome::Limit
            | jig_client::Outcome::Unconfirmed
            | jig_client::Outcome::Held
            | jig_client::Outcome::Unsent
            | jig_client::Outcome::Lost => text(b, w, Role::Alert, Word::GitHubUnknown),
        }
    }
    if let Some(adoption) = &picker.adoption
        && let Some(answer) = &adoption.outcome
    {
        adoption_outcome(b, w, &adoption.value, answer);
    }
    b.open(Role::Slot);
    b.style(look::FLOW_MUTED);
    w.text(b, Word::GitHubNotListed, &[]);
    let label = w.say(Word::GitHubChooseRepositories, &[]);
    if disabled {
        flow::control(b, Control::QuietDisabled { name: &label });
    } else {
        flow::control(
            b,
            Control::Link {
                name: &label,
                to: forge.insert(Meaning::Install { forge: row.number, back: root.pages().address().clone() }),
            },
        );
    }
    w.text(b, Word::GitHubNotListedAfter, &[]);
    b.close();
}
fn github_choices(
    b: &mut Builder,
    w: &Words,
    forge: &mut ForgeBindings,
    row: &Row,
    picker: &forge::picker::Picker,
    disabled: bool,
) {
    use forge::{github::Answer, picker::Act};
    let mut accepted = 0_u32;
    for installation in &row.installations {
        if !installation.suspended && !installation.stray {
            accepted = accepted.checked_add(1).expect("bounded installations");
        }
    }
    if accepted > 1 {
        b.open(Role::ChoiceGroup);
        b.name(&w.say(Word::ForgeAccountLabel, &[]));
        for installation in &row.installations {
            if installation.suspended || installation.stray {
                continue;
            }
            let account = match &installation.account {
                Some(account) => copy_of(&account.login),
                None => Box::from(skein_lib::Decimal::of(installation.id).as_bytes()),
            };
            b.open(Role::Choice);
            b.name(&account);
            if installation.id == picker.installation {
                b.state(State::Selected);
            }
            if disabled {
                b.state(State::Disabled);
            }
            b.bind(forge.insert(Meaning::Picker { forge: row.number, act: Act::Account(installation.id) }));
            b.close();
        }
        b.close();
    }
    match &picker.answer {
        None => text(b, w, Role::Status, Word::GitHubReading),
        Some(Answer::Page(page)) => {
            if page.rows.is_empty() {
                text(b, w, Role::Status, Word::GitHubNone);
            } else {
                b.open(Role::ChoiceGroup);
                b.name(&w.say(Word::GitHubRepositories, &[]));
                for (index, repository) in page.rows.iter().enumerate() {
                    let name = forge::words::full_name(&repository.owner.login, &repository.name);
                    b.open(Role::Slot);
                    b.style(look::REPOSITORY_CHOICE);
                    if picker.chosen == u32::try_from(index).expect("bounded page") {
                        b.style(look::REPOSITORY_SELECTED);
                    }
                    b.open(Role::Choice);
                    b.name(&name);
                    let index = u32::try_from(index).expect("bounded page");
                    if picker.chosen == index {
                        b.state(State::Selected);
                    }
                    if disabled {
                        b.state(State::Disabled);
                    }
                    b.bind(forge.insert(Meaning::Picker { forge: row.number, act: Act::Choose(index) }));
                    b.close();
                    if repository.private {
                        b.open(Role::Text);
                        b.style(look::REPOSITORY_PRIVATE);
                        w.text(b, Word::GitHubPrivateTag, &[]);
                        b.close();
                    }
                    b.close();
                }
                b.close();
            }
            if page.more
                && let Some(next) = picker.page.checked_add(1)
            {
                picker_control(b, forge, row.number, &w.say(Word::GitHubNext, &[]), Act::Page(next), disabled);
            }
            if picker.page > 1 {
                picker_control(b, forge, row.number, &w.say(Word::GitHubFirst, &[]), Act::Page(1), disabled);
            }
        }
        Some(Answer::Entrance(refusal)) => {
            b.open(Role::Alert);
            w.refusal(b, refusal.refusal(), w.name_bytes);
            b.close();
        }
        Some(Answer::NoAccess) => text(b, w, Role::Alert, Word::GitHubReadNoAccess),
        Some(Answer::NotFound) => text(b, w, Role::Alert, Word::GitHubReadNotFound),
        Some(answer) => {
            if let Some(words) = forge::words::app_refusal(answer.clone(), row.number) {
                crate::cards::text(b, Role::Alert, &words);
            }
            if picker.retry_exhausted() {
                text(b, w, Role::Paragraph, Word::GitHubReadReload);
            }
        }
    }
}
fn github_adopted(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings, row: &Row) {
    let Some(adopted) = row.repositories.first() else { return };
    let repository = forge::words::full_name(&adopted.owner, &adopted.name);
    let project = project_name(root, w, &SetUpProject::Existing(adopted.project));
    flow::details(b, &w.say(Word::SetupReady, &[Hole::Name(repository)]));
    flow::subtitle(b, &w.say(Word::GitHubInProject, &[Hole::Name(project.clone())]));
    if let Some(picker) = root.forge().picker(row.number)
        && let Some(adoption) = &picker.adoption
        && adoption.value.project == adopted.project
        && let Some(forge::Answer::Adopted(found)) = &adoption.outcome
    {
        // GitHub's App reads commit statuses; the generic adoption kinds also serve Forgejo.
        let mut shown = found.clone();
        shown.kinds.status = false;
        adoption_outcome(b, w, &adoption.value, &forge::Answer::Adopted(shown));
        text(b, w, Role::Paragraph, Word::GitHubStatuses);
        match &found.protection {
            forge::Protection::Unknown => {}
            forge::Protection::Absent => text(b, w, Role::Paragraph, Word::GitHubNoRules),
            forge::Protection::Rule(rule) => {
                crate::cards::text(
                    b,
                    Role::Paragraph,
                    &w.say(
                        Word::GitHubRule,
                        &[
                            Hole::Name(rule.branch.clone()),
                            Hole::Number(u64::from(rule.approvals)),
                            Hole::Name(forge::words::joined(&rule.contexts, w.name_bytes)),
                        ],
                    ),
                );
                if rule.dismiss_stale {
                    text(b, w, Role::Paragraph, Word::GitHubDismiss);
                }
            }
        }
    }
    flow::control(
        b,
        Control::PrimaryLink {
            name: &w.say(Word::SetupOpen, &[Hole::Name(project)]),
            to: bindings.insert(Act::OpenProject { project: adopted.project }),
        },
    );
    if let Some(web) = &row.web {
        let address = github_repository_address(web, &adopted.owner, &adopted.name);
        b.open(Role::Link);
        b.style(look::FLOW_QUIET);
        b.name(&w.say(Word::GitHubOpen, &[]));
        b.external(&address);
        b.close();
    }
}
fn github_repository_address(web: &[u8], owner: &[u8], name: &[u8]) -> Box<[u8]> {
    let mut web = web;
    for _ in 0..web.len() {
        let Some(prefix) = web.strip_suffix(b"/") else { break };
        web = prefix;
    }
    let mut bytes = web.len().checked_add(2).expect("address separators fit");
    for segment in [owner, name] {
        for byte in segment {
            bytes = bytes
                .checked_add(if byte.is_ascii_alphanumeric() || b"-._~".contains(byte) { 1 } else { 3 })
                .expect("encoded address fits");
        }
    }
    let mut address = skein_lib::Writer::new(bytes);
    address.put(web).expect("web capacity");
    for segment in [owner, name] {
        address.put(b"/").expect("separator capacity");
        for byte in segment {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(byte) {
                address.put(&[*byte]).expect("plain segment capacity");
            } else {
                let hex = b"0123456789ABCDEF";
                address
                    .put(&[
                        b'%',
                        *hex.get(usize::from(*byte >> 4_u8)).expect("hex digit"),
                        *hex.get(usize::from(*byte & 15)).expect("hex digit"),
                    ])
                    .expect("encoded segment capacity");
            }
        }
    }
    address.finish()
}

fn failure(b: &mut Builder, w: &Words, row: &Row) {
    b.open(Role::Alert);
    b.style(look::FLOW_FAILED);
    let word = match connect::failed_step(row) {
        Some(connect::FailedStep::Repository) => {
            let words = forge::words::first_repository(row.first.as_ref().expect("failed first repository"));
            b.name(&words);
            b.text(Run::Plain, &words);
            b.close();
            return;
        }
        Some(connect::FailedStep::Fetch) => Word::ForgeFetchFailed,
        Some(connect::FailedStep::Prepare) => Word::ForgePrepareFailed,
        Some(connect::FailedStep::Run) => Word::ForgeRunFailed,
        Some(connect::FailedStep::Connect) => Word::ForgeConnectFailed,
        None => Word::ForgeSetupFailed,
    };
    let why = match &row.connection {
        Connection::SetUpFailed { why, .. } => forge::words::lossy(why),
        Connection::NotConnected
        | Connection::NotSetUp
        | Connection::SettingUp { .. }
        | Connection::Connecting
        | Connection::Connected { .. }
        | Connection::Stopped { .. } => {
            b.close();
            return;
        }
    };
    let words = w.say(word, &[Hole::Name(why)]);
    b.name(&words);
    b.text(Run::Plain, &words);
    b.close();
}

fn admin_host(w: &Words, row: &Row) -> Box<[u8]> {
    let host = match &row.web {
        Some(web) => copy_of(web),
        None => {
            let number = skein_lib::Decimal::of(u64::from(row.number));
            let mut out = skein_lib::Writer::new(number.as_bytes().len().checked_add(6).expect("host fits"));
            out.put(b"forge ").expect("prefix fits");
            out.put(number.as_bytes()).expect("number fits");
            out.finish()
        }
    };
    w.say(Word::AdminHost, &[Hole::Name(host)])
}
fn admin_details(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut ForgeBindings, row: &Row) {
    flow::card(b, &w.say(Word::AdminTitle, &[]));
    admin_stop(b, w, row);
    if let Some(pending) = root.forge().admin()
        && pending.forge == row.number
        && let Some(answer) = &pending.answer
        && *answer != forge::github::Answer::Connecting
    {
        if let Some(line) = forge::words::app_refusal(answer.clone(), row.number) {
            crate::cards::text(b, Role::Alert, &line);
        }
        text(b, w, Role::Paragraph, Word::AdminKept);
    }
    text(b, w, Role::Paragraph, Word::AdminDescription);
    let draft = root.forge().admin_form();
    b.open(Role::TextField);
    b.name(&w.say(Word::AdminToken, &[]));
    b.entry(skein_ui::EntryKind::Secret);
    b.value(b"", draft.token_written);
    b.bind(bindings.insert(Meaning::AdminToken));
    if draft.checked && !draft.has_token() {
        b.state(State::Invalid);
    }
    b.close();
    if draft.checked && draft.too_long {
        text(b, w, Role::Alert, Word::AdminTooLong);
    } else if draft.checked && !draft.has_token() {
        text(b, w, Role::Alert, Word::SetupRequired);
    }
    b.open(Role::Region);
    b.style(look::ADMIN_FIRST);
    b.name(&w.say(Word::AdminFirst, &[]));
    b.open(Role::Heading { level: 3 });
    b.style(look::ADMIN_LEGEND);
    b.name(&w.say(Word::AdminFirst, &[]));
    w.text(b, Word::AdminFirst, &[]);
    b.close();
    let named_bot = if let Some(pending) = root.forge().admin()
        && pending.forge == row.number
        && pending.answer
            == Some(forge::github::Answer::Refused(temper_client_protocol_wire::v1::ForgeRefusal::FirstIsBot))
    {
        true
    } else {
        false
    };
    for (field, word) in [
        (forge::admin::Field::Organization, Word::SetupOrganization),
        (forge::admin::Field::Repository, Word::SetupRepository),
        (forge::admin::Field::Project, Word::SetupNew),
        (forge::admin::Field::Label, Word::AdminLabel),
    ] {
        let index = field.index();
        let label = field == forge::admin::Field::Label;
        let invalid =
            if label { !draft.valid_label() } else { draft.fields.get(index).expect("four fields").is_empty() };
        b.open(Role::TextField);
        b.name(&w.say(word, &[]));
        b.value(draft.fields.get(index).expect("four fields"), *draft.written.get(index).expect("four writes"));
        b.bind(bindings.insert(Meaning::AdminField(field)));
        if (draft.checked && invalid) || (field == forge::admin::Field::Organization && named_bot) {
            b.state(State::Invalid);
        }
        b.close();
        if draft.checked && invalid {
            text(b, w, Role::Alert, if label { Word::AdminLabelInvalid } else { Word::SetupRequired });
        }
    }
    text(b, w, Role::Paragraph, Word::AdminLabelHint);
    b.close();
    flow::control(
        b,
        Control::PrimaryButton {
            name: &w.say(Word::AdminTitle, &[]),
            to: bindings.insert(Meaning::ConnectAdmin(row.number)),
        },
    );
    b.close();
}
fn admin_stage(stage: forge::forges::SetupStage) -> &'static [u8] {
    use forge::forges::SetupStage;
    match stage {
        SetupStage::Admin => b"checking the admin's token",
        SetupStage::Bot => b"making the bot",
        SetupStage::Token => b"making the bot's token",
        SetupStage::Org => b"making the organization",
        SetupStage::Repository => b"making the repository",
        SetupStage::File => b"adding the starter workflow",
        SetupStage::App => b"making the App",
        SetupStage::Installation => b"installing the App",
        SetupStage::State => b"saving the state",
    }
}
fn admin_stop(b: &mut Builder, w: &Words, row: &Row) {
    use forge::forges::AdminStop;
    if let Some(first) = &row.first
        && let FirstState::Failed { step } = first.state
    {
        let why = match &row.ended {
            Some(why) => copy_of(why),
            None => copy_of(b"Forgejo stopped the request"),
        };
        let step = match step {
            forge::forges::CreateStep::Admin => b"checking the admin's token".as_slice(),
            forge::forges::CreateStep::Org => b"its organization",
            forge::forges::CreateStep::Repository => b"its repository",
            forge::forges::CreateStep::File => b"its starter workflow",
        };
        crate::cards::text(
            b,
            Role::Alert,
            &w.say(
                Word::AdminFirstFailed,
                &[
                    Hole::Name(forge::words::full_name(&first.org, &first.repository)),
                    Hole::Name(copy_of(step)),
                    Hole::Name(why),
                ],
            ),
        );
    } else if let Some(stop) = row.ended_stop {
        match stop {
            AdminStop::NotAdmin => text(b, w, Role::Alert, Word::AdminNotAdmin),
            AdminStop::Refused { status: 401, .. } => text(b, w, Role::Alert, Word::AdminUnauthorized),
            AdminStop::Refused { status: 403, stage } => {
                crate::cards::text(
                    b,
                    Role::Alert,
                    &w.say(Word::AdminScope, &[Hole::Name(copy_of(admin_stage(stage)))]),
                );
            }
            AdminStop::Unmarked => text(b, w, Role::Alert, Word::AdminUnmarked),
            AdminStop::Refused { .. } | AdminStop::Other => {
                if let Some(line) = &row.ended {
                    crate::cards::text(b, Role::Alert, line);
                }
            }
        }
    } else {
        return;
    }
    text(b, w, Role::Paragraph, Word::AdminKept);
}
fn admin_ready(b: &mut Builder, root: &Domain, w: &Words, bindings: &mut Bindings, row: &Row) {
    let heading = if let Connection::Connected { id, login } = &row.connection {
        let login = forge::words::lossy(login);
        let id = skein_lib::Decimal::of(*id);
        let parts = [b"Connected as ".as_slice(), &login, b" (id ", id.as_bytes(), b"), managed."];
        let mut size = 0_usize;
        for part in parts {
            size = size.checked_add(part.len()).expect("heading fits");
        }
        let mut heading = skein_lib::Writer::new(size);
        for part in parts {
            heading.put(part).expect("measured heading");
        }
        heading.finish()
    } else {
        Box::default()
    };
    flow::card(b, &heading);
    if let Some(first) = &row.first {
        let repository = forge::words::full_name(&first.org, &first.repository);
        let project = project_name(root, w, &first.project);
        crate::cards::text(
            b,
            Role::Paragraph,
            &w.say(
                Word::AdminReady,
                &[Hole::Name(repository.clone()), Hole::Name(project.clone()), Hole::Name(first.label.clone())],
            ),
        );
        let open = w.say(Word::SetupOpen, &[Hole::Name(project)]);
        if let Some(project) = adopted_project(root, row) {
            flow::control(b, Control::PrimaryLink { name: &open, to: bindings.insert(Act::OpenProject { project }) });
        } else {
            flow::control(b, Control::PrimaryDisabled { name: &open });
        }
        if let Some(web) = &row.web {
            b.open(Role::Link);
            b.name(&w.say(Word::AdminRepository, &[Hole::Name(repository)]));
            b.external(&github_repository_address(web, &first.org, &first.repository));
            b.close();
        }
    }
    b.close();
}
