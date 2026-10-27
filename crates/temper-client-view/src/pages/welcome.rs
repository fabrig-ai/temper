//! The first run's welcome and shared account ways (client/README.md, 3.5, 5.6).
use crate::{
    frame::Bindings,
    look,
    words::{Word, Words},
};
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{Address, Domain, Step, forge::forges::Connection, pages::Act};

fn text(b: &mut Builder, w: &Words, role: Role, word: Word) {
    b.open(role);
    if word == Word::WelcomeTitle {
        b.style(look::WELCOME_INTRO);
    }
    b.name(&w.say(word, &[]));
    w.text(b, word, &[]);
    b.close();
}
fn paragraph(b: &mut Builder, w: &Words, word: Word, style: skein_ui::Style) {
    b.open(Role::Paragraph);
    b.style(style);
    b.name(&w.say(word, &[]));
    w.text(b, word, &[]);
    b.close();
}
fn lead(b: &mut Builder, w: &Words) {
    paragraph(b, w, Word::WelcomeLead, look::WELCOME_TAGLINE);
    let describe = w.say(Word::WelcomeDescribe, &[]);
    let what = w.say(Word::WelcomeWhat, &[]);
    let want_and = w.say(Word::WelcomeWantAnd, &[]);
    let how = w.say(Word::WelcomeHow, &[]);
    let want_done = w.say(Word::WelcomeWantDone, &[]);
    b.open(Role::Paragraph);
    b.style(look::WELCOME_LEAD);
    b.name(&crate::accounts::words::join(&[&describe, &what, &want_and, &how, &want_done]));
    b.open(Role::Text);
    b.run(Run::Plain, &describe);
    b.run(Run::Strong, &what);
    b.run(Run::Plain, &want_and);
    b.run(Run::Strong, &how);
    b.run(Run::Plain, &want_done);
    b.close();
    b.close();
}
fn control(b: &mut Builder, w: &Words, bindings: &mut Bindings, word: Word, act: Act) {
    let name = w.say(word, &[]);
    let binding = bindings.insert(act.clone());
    if let Act::Go { .. } = act {
        crate::link(b, &name, binding);
        b.text(Run::Plain, &name);
    } else {
        b.open(Role::Button);
        b.name(&name);
        b.bind(binding);
    }
    if word == Word::WelcomePlayground || word == Word::WelcomeSetup {
        b.style(look::FLOW_PRIMARY);
    } else if word != Word::WelcomePlaygroundInstead {
        b.style(look::FLOW_QUIET);
    }
    b.close();
}
fn go(b: &mut Builder, w: &Words, bindings: &mut Bindings, word: Word, step: Step) {
    control(b, w, bindings, word, Act::Go { address: Address::Welcome { step } });
}
fn steps(b: &mut Builder, root: &Domain, w: &Words, current: Step) {
    let welcome = w.say(Word::WelcomeStep, &[]);
    let forge = w.say(Word::WelcomeForgeStep, &[]);
    let model = w.say(Word::WelcomeModelStep, &[]);
    let ready = w.say(Word::WelcomeReadyStep, &[]);
    let current = match current {
        Step::Welcome => 0,
        Step::Forge { .. } => 1,
        Step::Model { .. } => 2,
        Step::Ready => {
            let connected = match crate::forge::connect::personal(root) {
                Some(row) => match temper_client_domain::forge::connect::step(
                    Some(temper_client_domain::Choice::Forge(row.number)),
                    root.forges(),
                    root.forge().demo(),
                ) {
                    temper_client_domain::forge::connect::Shown::Ready(_) => true,
                    temper_client_domain::forge::connect::Shown::Choose
                    | temper_client_domain::forge::connect::Shown::Details(_)
                    | temper_client_domain::forge::connect::Shown::SettingUp(_) => false,
                },
                None => false,
            };
            let adopted = match crate::forge::connect::personal(root) {
                Some(row) if crate::forge::connect::welcome_target(root, row).is_some() => {
                    crate::forge::connect::adopted_project(root, row).is_some()
                }
                Some(_) | None => connected,
            };
            if adopted && temper_client_domain::pages::welcome::model_usable(root) == Some(true) { 4 } else { 3 }
        }
    };
    crate::flow::steps(b, w, &[&welcome, &forge, &model, &ready], current);
}
fn card(b: &mut Builder, w: &Words, title: Word) {
    b.open(Role::Region);
    b.style(look::WELCOME_CARD);
    b.name(&w.say(title, &[]));
    text(b, w, Role::Heading { level: 2 }, title);
}
fn setup_description(b: &mut Builder, w: &Words) {
    b.open(Role::Slot);
    b.style(look::WELCOME_SETUP_DESCRIPTION);
    b.open(Role::Paragraph);
    w.text(b, Word::WelcomeSetupDescription, &[]);
    b.close();
    b.open(Role::Paragraph);
    w.text(b, Word::WelcomeYourModel, &[]);
    b.close();
    b.close();
}

#[expect(clippy::too_many_arguments, reason = "welcome composes the forge and accounts flows with their bindings")]
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    accounts: &mut crate::accounts::Bindings,
    forge: &mut crate::forge::bindings::Bindings,
    step: Step,
    text_per_node: u32,
    sign_in: &crate::forge_sign_in::Forges,
) {
    b.open(Role::Main);
    b.style(look::MAIN);
    b.style(look::WELCOME);
    steps(b, root, w, step);
    let playground = root.demo_answered() && root.forge().demo().is_some() && !real_forge_connected(root);
    match step {
        Step::Welcome => {
            text(b, w, Role::Heading { level: 1 }, Word::WelcomeTitle);
            if let Some(page) = root.pages().welcome()
                && page.decided
            {
                lead(b, w);
                b.open(Role::Slot);
                b.style(look::WELCOME_CHOICES);
                if playground {
                    card(b, w, Word::WelcomePlayground);
                    text(b, w, Role::Paragraph, Word::WelcomePlaygroundDescription);
                    paragraph(b, w, Word::WelcomePlaygroundCost, look::WELCOME_MUTED);
                    control(b, w, bindings, Word::WelcomePlayground, Act::Playground);
                    b.close();
                }
                card(b, w, Word::WelcomeSetup);
                setup_description(b, w);
                paragraph(b, w, Word::WelcomeSetupTime, look::WELCOME_MUTED);
                go(b, w, bindings, Word::WelcomeSetup, Step::Forge { forge: None });
                b.close();
                b.close();
                if playground {
                    paragraph(b, w, Word::WelcomeAfter, look::WELCOME_AFTER);
                }
            }
        }
        Step::Forge { forge: forge_choice } => {
            text(b, w, Role::Heading { level: 1 }, Word::WelcomeForgeTitle);
            paragraph(b, w, Word::WelcomeForgeLead, look::WELCOME_LEAD);
            crate::forge::connect::welcome(b, root, w, bindings, forge, forge_choice, sign_in);
        }
        Step::Model { .. } => {
            if let Some(row) = crate::forge::connect::personal(root) {
                b.open(Role::Slot);
                b.style(look::WELCOME_STATUS);
                crate::forge::connect::welcome_line(b, root, w, row);
                b.close();
            }
            text(b, w, Role::Heading { level: 1 }, Word::WelcomeModelTitle);
            paragraph(b, w, Word::WelcomeModelLead, look::WELCOME_LEAD);
            super::connect_model::body(b, root, w, accounts, text_per_node);
        }
        Step::Ready => {
            ready_heading(b, root, w);
            ready(b, root, w, bindings, accounts, forge, sign_in);
        }
    }
    if let Step::Forge { .. } | Step::Model { .. } = step {
        b.open(Role::Slot);
        b.style(look::WELCOME_FOOT);
        let back = match step {
            Step::Model { way: Some(_) } | Step::Ready => Step::Model { way: None },
            Step::Forge { forge: Some(_) } | Step::Model { way: None } => Step::Forge { forge: None },
            Step::Welcome | Step::Forge { forge: None } => Step::Welcome,
        };
        go(b, w, bindings, Word::WelcomeBack, back);
        control(
            b,
            w,
            bindings,
            Word::WelcomeLater,
            Act::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        );
        if playground {
            control(b, w, bindings, Word::WelcomePlaygroundInstead, Act::Playground);
        }
        b.close();
    }
    b.close();
}

fn legacy_ready(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    accounts: &mut crate::accounts::Bindings,
    forge: &mut crate::forge::bindings::Bindings,
    sign_in: &crate::forge_sign_in::Forges,
) {
    use crate::forge::connect;
    use skein_ui::phrases::Hole;
    b.open(Role::List);
    b.style(look::FLOW_PROGRESS);
    b.style(look::WELCOME_CHECKLIST);
    b.open(Role::Item);
    let row = connect::personal(root);
    if let Some(row) = row {
        let ready = temper_client_domain::forge::connect::step(
            Some(temper_client_domain::Choice::Forge(row.number)),
            root.forges(),
            root.forge().demo(),
        );
        if row.first.is_some() && ready == temper_client_domain::forge::connect::Shown::Ready(row.number) {
            connect::ready_card(b, root, w, bindings, forge, row, sign_in);
        } else if row.first.is_some() && ready == temper_client_domain::forge::connect::Shown::SettingUp(row.number) {
            connect::setting_up(b, root, w, forge, row);
        } else if row.first.is_none()
            && let Connection::Connected { login, .. } = &row.connection
        {
            crate::cards::text(
                b,
                Role::Paragraph,
                &w.say(
                    Word::WelcomeForgeConnected,
                    &[Hole::Name(connect::name(w, row, root)), Hole::Name(skein_lib::bytes::copy_of(login))],
                ),
            );
        } else {
            connect::status(b, root, w, row, true);
            if connect::underway(row) || temper_client_domain::forge::connect::failed(row) {
                connect::each_step(b, root, w, row);
            }
        }
    } else {
        text(b, w, Role::Paragraph, Word::WelcomeNoForge);
        go(b, w, bindings, Word::WelcomeForgeTitle, Step::Forge { forge: None });
    }
    b.close();
    model_line(b, root, w, bindings, accounts);
    b.close();
    if let Some(row) = row
        && row.first.is_none()
        && let Connection::Connected { .. } = row.connection
    {
        connect::adopt(b, root, w, bindings);
    }
}

fn model_line(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    accounts: &mut crate::accounts::Bindings,
) {
    if let Some(usable) = temper_client_domain::pages::welcome::model_usable(root) {
        b.open(Role::Item);
        if usable {
            b.style(look::FLOW_DONE);
            let account = crate::choice::ready_model(root, w);
            crate::cards::text(b, Role::Paragraph, &w.say(Word::WelcomeModelAccount, &[Hole::Name(account)]));
        } else if let Some(provider) = super::accounts::own_signing_name(root, w) {
            crate::cards::text(b, Role::Paragraph, &w.say(Word::WelcomeModelSigningIn, &[Hole::Name(provider)]));
        } else {
            b.open(Role::Slot);
            b.style(look::ACCOUNT_LINE);
            b.name(&w.say(Word::WelcomeNoModel, &[]));
            b.open(Role::Text);
            b.run(Run::Plain, &w.say(Word::WelcomeNoModel, &[]));
            b.run(Run::Plain, b" \xc2\xb7 ");
            b.close();
            go(b, w, bindings, Word::WelcomeModelTitle, Step::Model { way: None });
            b.close();
            b.open(Role::Slot);
            b.style(look::WELCOME_END);
            super::connect_model::welcome_end(b, root, w, accounts);
            b.close();
        }
        if usable {
            text(b, w, Role::Status, Word::FlowDone);
        }
        b.close();
    }
}
fn real_forge_connected(root: &Domain) -> bool {
    if let temper_client_domain::forge::forges::State::Rows(rows) = root.forges() {
        for row in rows {
            if row.mode != temper_client_domain::forge::forges::Mode::Fake
                && let Connection::Connected { .. } = row.connection
                && match root.forge().demo() {
                    Some(demo) => row.number != demo.forge,
                    None => true,
                }
            {
                return true;
            }
        }
    }
    false
}
fn ready_heading(b: &mut Builder, root: &Domain, w: &Words) {
    if let Some(row) = crate::forge::connect::personal(root)
        && crate::forge::connect::adopted_project(root, row).is_some()
        && let Some((org, repository, _)) = crate::forge::connect::welcome_target(root, row)
    {
        crate::cards::text(
            b,
            Role::Heading { level: 1 },
            &w.say(Word::SetupReady, &[Hole::Name(temper_client_domain::forge::words::full_name(org, repository))]),
        );
    } else {
        text(b, w, Role::Heading { level: 1 }, Word::WelcomeReadyTitle);
    }
}
fn ready(
    b: &mut Builder,
    root: &Domain,
    w: &Words,
    bindings: &mut Bindings,
    accounts: &mut crate::accounts::Bindings,
    forge: &mut crate::forge::bindings::Bindings,
    sign_in: &crate::forge_sign_in::Forges,
) {
    use crate::{
        flow::{self, Control},
        forge::connect,
    };
    let Some(row) = connect::personal(root) else {
        legacy_ready(b, root, w, bindings, accounts, forge, sign_in);
        return;
    };
    let Some((org, repository, project)) = connect::welcome_target(root, row) else {
        legacy_ready(b, root, w, bindings, accounts, forge, sign_in);
        return;
    };
    let adopted = connect::adopted_project(root, row);
    let repository = temper_client_domain::forge::words::full_name(org, repository);
    let project = connect::project_name(root, w, &project);
    b.open(Role::List);
    b.style(look::FLOW_PROGRESS);
    b.style(look::WELCOME_CHECKLIST);
    if adopted.is_none() {
        model_line(b, root, w, bindings, accounts);
    }
    b.open(Role::Item);
    if adopted.is_some() {
        b.style(look::FLOW_DONE);
        if let Connection::Connected { login, .. } = &row.connection {
            crate::cards::text(
                b,
                Role::Paragraph,
                &w.say(
                    Word::WelcomeForgeConnected,
                    &[Hole::Name(connect::name(w, row, root)), Hole::Name(skein_lib::bytes::copy_of(login))],
                ),
            );
        }
        text(b, w, Role::Status, Word::FlowDone);
    } else {
        b.state(State::Current);
        connect::status(b, root, w, row, true);
        text(b, w, Role::Status, Word::FlowUnderWay);
        connect::each_step(b, root, w, row);
    }
    b.close();
    if adopted.is_some() {
        model_line(b, root, w, bindings, accounts);
    }
    b.open(Role::Item);
    if adopted.is_some() {
        b.style(look::FLOW_DONE);
    }
    crate::cards::text(
        b,
        Role::Paragraph,
        &w.say(Word::WelcomeProject, &[Hole::Name(project.clone()), Hole::Name(repository.clone())]),
    );
    if adopted.is_some() {
        text(b, w, Role::Status, Word::FlowDone);
    }
    b.close();
    b.close();
    let open = w.say(Word::SetupOpen, &[Hole::Name(project)]);
    if let Some(project) = adopted {
        b.open(Role::Slot);
        b.style(look::WELCOME_ACTIONS);
        flow::control(b, Control::PrimaryLink { name: &open, to: bindings.insert(Act::OpenProject { project }) });
        crate::forge::forges::open_link(b, w, row, sign_in, Some(look::FLOW_QUIET));
        b.close();
    } else {
        flow::control(b, Control::PrimaryDisabled { name: &open });
        b.open(Role::Paragraph);
        b.style(look::WELCOME_AFTER);
        b.style(look::WELCOME_TINY);
        b.name(&w.say(Word::WelcomeOpensOnce, &[Hole::Name(repository.clone())]));
        b.text(Run::Plain, &w.say(Word::WelcomeOpensOnce, &[Hole::Name(repository)]));
        b.close();
    }
    connect::setup_ending(b, root, w, forge, row);
}
