#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "ordinary tests inspect semantic trees and allocate maximal model-name fixtures"
)]
use super::fixtures;
use jig_client::contract as c;
use skein_lib::{List, Token};
use skein_ui::{Builder, Role};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{self as d, Address};
fn delivered(root: &mut d::Domain, watch: Token, value: c::Delivery) {
    fixtures::root_event(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(value) },
            },
        },
    );
}
fn banner(usable: Option<bool>) -> (d::Domain, Token) {
    let (mut root, watch) = fixtures::account_root();
    if let Some(usable) = usable {
        delivered(
            &mut root,
            watch,
            c::Delivery::OwnerAccounts(Box::new(c::OwnerAccounts {
                program_hosts: List::with_capacity(0),
                accounts: List::with_capacity(0),
                signing: List::with_capacity(0),
                last_end: None,
                usable,
                settled: true,
                cut: false,
                readiness: List::with_capacity(0),
                readiness_cut: false,
            })),
        );
    }
    fixtures::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) },
    );
    (root, watch)
}
fn text(root: &d::Domain) -> String {
    String::from_utf8(fixtures::text(fixtures::page_view(root).tree()).into_vec()).unwrap()
}
#[test]
fn the_accounts_banner_precedes_main() {
    let view = fixtures::page_view(&banner(Some(false)).0);
    let nodes = view.tree().nodes();
    let banner = nodes.iter().position(|n| n.name.as_deref() == Some(b"Agents need an LLM account to run.")).unwrap();
    assert!(nodes.iter().position(|n| n.role == Role::Navigation).unwrap() < banner);
    assert!(banner < nodes.iter().position(|n| n.role == Role::Main).unwrap());
}
#[test]
fn usable_accounts_remove_the_banner() {
    assert!(!text(&banner(Some(true)).0).contains("Agents need"));
}
#[test]
fn unknown_accounts_have_no_banner() {
    assert!(!text(&banner(None).0).contains("Agents need"));
}
#[test]
fn a_behind_accounts_document_removes_the_banner() {
    let (mut root, watch) = banner(Some(false));
    fixtures::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: c::WatchEnd::Dropped } },
        },
    );
    assert!(!text(&root).contains("Agents need"));
}
#[test]
fn missing_accounts_leave_start_available() {
    assert!(fixtures::find(fixtures::page_view(&banner(Some(false)).0).tree(), Role::Button, b"Start chat").is_some());
}
#[test]
fn the_accounts_page_has_no_accounts_banner() {
    let (mut root, _) = banner(Some(false));
    fixtures::root_event(&mut root, d::Event::Went { place: d::Place::Address(Address::Accounts) });
    assert!(!text(&root).contains("Agents need"));
}
#[test]
fn an_absent_choice_uses_the_unsettled_default() {
    assert!(text(&fixtures::chats_root()).contains("Using the default model"));
}
fn task_root() -> d::Domain {
    account_readiness(None)
}
fn account_readiness(ready: Option<c::PayerReadiness>) -> d::Domain {
    account_facts(ready, List::with_capacity(0))
}
fn account_facts(ready: Option<c::PayerReadiness>, accounts: List<c::OwnerAccountLine>) -> d::Domain {
    line_facts(ready, accounts, Some(3), Some(7), None, true)
}
fn line_models(task_page: bool) -> List<c::MeModel> {
    let mut models = List::with_capacity(3);
    models
        .push(c::MeModel {
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            name: c::Text::new(Box::from(if task_page { b"chosen".as_slice() } else { b"gpt-6-luna" }), 32).unwrap(),
            provider: Some(17),
            window: 1024,
            output: 128,
            efforts: List::with_capacity(0),
        })
        .unwrap();
    models
        .push(c::MeModel {
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            name: c::Text::new(Box::from(*b"second"), 32).unwrap(),
            provider: None,
            window: 1024,
            output: 128,
            efforts: List::with_capacity(0),
        })
        .unwrap();
    if !task_page {
        // A non-usable provider before the usable one must not settle the default.
        let mut offered = List::with_capacity(3);
        let mut unavailable = models.get(0).unwrap().clone();
        unavailable.provider = Some(18);
        unavailable.name = c::Text::new(Box::from(b"unavailable".as_slice()), 32).unwrap();
        offered.push(unavailable).unwrap();
        for model in &models {
            offered.push(model.clone()).unwrap();
        }
        return offered;
    }
    models
}
fn line_facts(
    ready: Option<c::PayerReadiness>,
    accounts: List<c::OwnerAccountLine>,
    preference: Option<u32>,
    kept: Option<u32>,
    effort: Option<&[u8]>,
    task_page: bool,
) -> d::Domain {
    line_fixture(ready, accounts, preference, kept, effort, task_page).0
}
fn line_fixture(
    ready: Option<c::PayerReadiness>,
    accounts: List<c::OwnerAccountLine>,
    preference: Option<u32>,
    kept: Option<u32>,
    effort: Option<&[u8]>,
    task_page: bool,
) -> (d::Domain, Token, Option<Token>, Token) {
    let (mut root, me_watch, watch, _, accounts_watch) = fixtures::root_at_full(
        if task_page { Address::Chat { task: 9 } } else { Address::Chats { filter: ChatsFilter::All, start: false } },
        &fixtures::limits(),
        b"Shop",
    );
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    let models = line_models(task_page);
    me.projects.get_mut(0).unwrap().models = models;
    if !task_page {
        let template =
            fixtures::demo_root(Address::Chats { filter: ChatsFilter::All, start: false }, fixtures::PROJECT, true);
        me.projects.get_mut(0).unwrap().templates =
            template.pages().me(template.client()).unwrap().projects.get(0).unwrap().templates.clone();
    }
    me.providers = List::with_capacity(1);
    me.providers
        .push(c::ProviderOffer {
            provider: 17,
            name: c::DocumentText::complete(
                c::Text::new(Box::from(if task_page { b"Supplied Provider".as_slice() } else { b"openai" }), 32)
                    .unwrap(),
            ),
            key_bytes: 4096,
            sign_in: true,
            key: true,
            programs: List::with_capacity(0),
            programs_cut: false,
            none: false,
            signed_in_plan: false,
            manage_usage: None,
        })
        .unwrap();
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    let mut readiness = List::with_capacity(1);
    if let Some(ready) = ready {
        readiness.push(ready).unwrap();
    }
    delivered(
        &mut root,
        accounts_watch.unwrap(),
        c::Delivery::OwnerAccounts(Box::new(c::OwnerAccounts {
            program_hosts: List::with_capacity(0),
            accounts,
            signing: List::with_capacity(0),
            last_end: None,
            usable: true,
            settled: true,
            cut: false,
            readiness,
            readiness_cut: false,
        })),
    );
    let mut task = fixtures::task::document();
    task.chip.task = 9;
    task.project = fixtures::PROJECT;
    task.chip.executor = c::Executor::Agent { charter: 1 };
    task.budget.funder = c::Funder::Pool { project: fixtures::PROJECT, person: 73, period: 1 };
    task.acts = List::with_capacity(2);
    for act in [c::ActKind::Message, c::ActKind::Amend] {
        task.acts.push(c::Offered { act, admission: c::Admission::Allowed, effect: None }).unwrap();
    }
    task.llm = Some(c::LlmChoice {
        account: preference,
        main: c::ModelEffort {
            model: c::Text::new(Box::from(*b"chosen"), 32).unwrap(),
            effort: effort.map(|name| c::Text::new(Box::from(name), 32).unwrap()),
        },
        subagents: List::with_capacity(0),
    });
    task.kept_account = kept;
    if task_page {
        delivered(&mut root, watch.unwrap(), c::Delivery::Task(Box::new(task)));
    }
    (root, me_watch, watch, accounts_watch.unwrap())
}
#[test]
fn a_requested_model_is_labelled_for_the_next_run() {
    assert!(text(&task_root()).contains("Using chosen"));
}
#[test]
fn an_unknown_readiness_keeps_the_model_only() {
    assert!(text(&task_root()).contains("Using chosen"));
}
fn wait(why: c::TaskWaitWhy) -> skein_ui::Tree {
    let mut b = Builder::new(&fixtures::tree_limits());
    let mut bindings = crate::frame::Bindings::new(32);
    b.open(Role::Page);
    crate::choice::waiting(
        &mut b,
        &task_root(),
        Some(c::TaskRunWait::Account { account: Some(7), source: Some(8), why }),
        &fixtures::words(),
        &mut bindings,
        &mut crate::words::Times::new(fixtures::NOW),
    );
    b.close();
    b.finish()
}
#[test]
fn a_limited_wait_links_to_resume() {
    assert!(
        fixtures::find(
            &wait(c::TaskWaitWhy::Source(c::TaskSourceState::Limited { until: None })),
            Role::Link,
            b"Resume"
        )
        .is_some()
    );
}
#[test]
fn a_sign_in_wait_links_to_sign_in() {
    assert!(
        fixtures::find(
            &wait(c::TaskWaitWhy::Source(c::TaskSourceState::NeedsSignIn)),
            Role::Link,
            b"Sign in with an LLM provider"
        )
        .is_some()
    );
}
#[test]
fn an_unknown_reset_has_no_time() {
    assert!(
        !String::from_utf8(
            fixtures::text(&wait(c::TaskWaitWhy::Source(c::TaskSourceState::Limited { until: None }))).into_vec()
        )
        .unwrap()
        .contains("until")
    );
}
#[test]
fn a_known_reset_shows_its_utc_moment() {
    let until = Some(skein_lib::Wall::from_nanos(1_791_828_240_000_000_000));
    let shown = String::from_utf8(
        fixtures::text(&wait(c::TaskWaitWhy::Source(c::TaskSourceState::Limited { until }))).into_vec(),
    )
    .unwrap();
    assert!(shown.contains("Limited until Oct 12, 18:04 UTC"), "{shown}");
}

fn model_words() -> crate::words::Words {
    crate::words::Words::english(fixtures::FORGE, 128, crate::words::Shell::Terminal).unwrap()
}
const NAME_BYTES: u32 = jig_client_protocol_wire::v19::CEILINGS.me_model_name;
const EFFORT_BYTES: u32 = jig_client_protocol_wire::v19::CEILINGS.me_model_efforts_item;
const CUT_MARK: &[u8] = "…".as_bytes();
fn shown_bytes() -> usize {
    usize::try_from(crate::choice::MODEL_SHOWN).unwrap()
}
fn prefix_bytes() -> usize {
    shown_bytes().checked_sub(CUT_MARK.len()).expect("display bound includes cut mark")
}
fn catalogue(name: &[u8], efforts: &[&[u8]]) -> d::Domain {
    let mut declared = List::with_capacity(u32::try_from(efforts.len()).unwrap());
    for effort in efforts {
        declared.push(c::Text::new(Box::from(*effort), EFFORT_BYTES).unwrap()).unwrap();
    }
    let mut models = List::with_capacity(1);
    models
        .push(c::MeModel {
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            name: c::Text::new(Box::from(name), NAME_BYTES).unwrap(),
            provider: None,
            window: 1024,
            output: 128,
            efforts: declared,
        })
        .unwrap();
    catalogue_rows(models)
}
fn catalogue_rows(models: List<c::MeModel>) -> d::Domain {
    let (mut root, me_watch, _) =
        fixtures::root_at(Address::Chats { filter: ChatsFilter::All, start: false }, &fixtures::limits());
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    let project = me.projects.get_mut(0).unwrap();
    project.templates = List::with_capacity(1);
    project
        .templates
        .push(c::Template {
            number: 1,
            name: c::DocumentText::complete(c::Text::new(Box::from(*b"Chat"), 32).unwrap()),
            executor: c::TemplateExecutor::Charter(1),
            contract: c::TaskContractDocument::Report { words: 32 },
            shallower: 1,
            budget: c::TemplateBudget::AsAllowed { cap: None },
            deadline: c::TemplateDeadline::None,
            tracked: None,
            flags: c::TaskFlags {
                ended_by: jig_client::contract::EndedBy::Result,
                conversation: true,
                listed: false,
                retention: c::Retention::Kept,
            },
            label: 7,
            grace: None,
            llm: None,
        })
        .unwrap();
    project.models = models;
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    root
}
fn catalogue_view(root: &d::Domain) -> crate::View {
    let mut tree = fixtures::tree_limits();
    tree.text_per_node = 1024;
    let mut view = crate::View::new(&crate::Limits { tree }, &fixtures::limits(), model_words(), fixtures::FORGE);
    fixtures::build(&mut view, root);
    view
}
fn shows_requested(view: &crate::View, label: &[u8]) {
    assert!(fixtures::find(view.tree(), Role::Button, label).is_some());
    let requested = [b"Using ".as_slice(), label].concat();
    assert!(fixtures::find(view.tree(), Role::Paragraph, &requested).is_some());
}
fn marked_prefix() -> Vec<u8> {
    [vec![b'm'; prefix_bytes()], CUT_MARK.to_vec()].concat()
}
#[test]
fn a_long_model_name_shows_a_marked_prefix() {
    let mut root = catalogue(&vec![b'm'; usize::try_from(NAME_BYTES).unwrap()], &[]);
    edit(&mut root, d::pages::choice::Act::Model { index: 0 });
    shows_requested(&catalogue_view(&root), &marked_prefix());
}
#[test]
fn a_name_at_the_display_bound_shows_whole() {
    let name = vec![b'm'; shown_bytes()];
    let mut root = catalogue(&name, &[]);
    edit(&mut root, d::pages::choice::Act::Model { index: 0 });
    shows_requested(&catalogue_view(&root), &name);
}
#[test]
fn a_name_one_past_the_display_bound_shows_a_marked_prefix() {
    let name = vec![b'm'; shown_bytes() + 1];
    let mut root = catalogue(&name, &[]);
    edit(&mut root, d::pages::choice::Act::Model { index: 0 });
    shows_requested(&catalogue_view(&root), &marked_prefix());
}
#[test]
fn a_cut_model_name_ends_at_a_utf8_boundary() {
    let letter = "é";
    let name = letter.repeat(usize::try_from(NAME_BYTES).unwrap() / letter.len());
    let shown = crate::choice::shown(name.as_bytes(), &model_words());
    assert_eq!(
        String::from_utf8(shown.into_vec()).unwrap(),
        format!("{}…", letter.repeat(prefix_bytes() / letter.len()))
    );
}
#[test]
fn only_the_selected_duplicate_model_shows_its_efforts() {
    let mut models = List::with_capacity(2);
    for (provider, effort) in [(17, b"first-effort".as_slice()), (18, b"second-effort".as_slice())] {
        let mut efforts = List::with_capacity(1);
        efforts.push(c::Text::new(Box::from(effort), EFFORT_BYTES).unwrap()).unwrap();
        models
            .push(c::MeModel {
                prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
                description: None,
                name: c::Text::new(Box::from(*b"shared"), NAME_BYTES).unwrap(),
                provider: Some(provider),
                window: 1024,
                output: 128,
                efforts,
            })
            .unwrap();
    }
    let mut root = catalogue_rows(models);
    edit(&mut root, d::pages::choice::Act::Model { index: 1 });
    let view = catalogue_view(&root);
    let efforts: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|node| {
            node.role == Role::Button
                && [b"first-effort".as_slice(), b"second-effort"].contains(&node.name.as_deref().unwrap_or_default())
        })
        .map(|node| node.name.as_deref())
        .collect();
    assert_eq!(efforts, [Some(b"second-effort".as_slice())]);
}

#[test]
fn the_chosen_model_and_effort_are_pressed() {
    let mut root = catalogue(b"needs-effort", &[b"low", b"high"]);
    edit(&mut root, d::pages::choice::Act::Model { index: 0 });
    edit(&mut root, d::pages::choice::Act::Effort { index: 1 });
    let view = catalogue_view(&root);
    let states = |name: &[u8]| {
        let node = view.tree().nodes().iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        node.unwrap().states.as_slice().to_vec()
    };
    for name in [b"needs-effort".as_slice(), b"high"] {
        assert!(states(name).contains(&skein_ui::State::Pressed));
        assert!(!states(name).contains(&skein_ui::State::Selected));
    }
    assert!(!states(b"low").contains(&skein_ui::State::Pressed));
}
#[test]
fn an_invalid_draft_withholds_start() {
    let mut root = catalogue(b"needs-effort", &[b"high"]);
    edit(&mut root, d::pages::choice::Act::Model { index: 0 });
    assert!(fixtures::find(catalogue_view(&root).tree(), Role::Button, b"Start chat").is_none());
}

fn readiness(state: c::Readiness) -> c::PayerReadiness {
    c::PayerReadiness { provider: 17, payer: c::OwnerAccountUse::Party(73), readiness: state, settled: Some(true) }
}
#[test]
fn an_unavailable_provider_needs_an_account() {
    assert!(text(&account_readiness(Some(readiness(c::Readiness::Unavailable)))).contains("needs an account"));
}
#[test]
fn a_usable_provider_does_not_need_an_account() {
    assert!(!text(&account_readiness(Some(readiness(c::Readiness::Usable)))).contains("needs an account"));
}
#[test]
fn unknown_provider_readiness_is_named() {
    assert!(text(&account_readiness(Some(readiness(c::Readiness::Unknown)))).contains("Account readiness unknown"));
}
#[test]
fn missing_provider_readiness_stays_unknown() {
    assert!(text(&account_readiness(None)).contains("Account readiness unknown"));
}
#[test]
fn another_payers_readiness_does_not_decide_this_payers() {
    let mut row = readiness(c::Readiness::Unavailable);
    row.payer = c::OwnerAccountUse::Project(fixtures::PROJECT);
    assert!(!text(&account_readiness(Some(row))).contains("needs an account"));
}
#[test]
fn the_provider_keeps_its_supplied_name() {
    assert!(text(&task_root()).contains("Supplied Provider"));
}
#[test]
fn the_accounts_banner_is_a_named_region() {
    let view = fixtures::page_view(&banner(Some(false)).0);
    let node = view.tree().nodes().iter().find(|n| n.styles.as_slice().contains(&crate::look::ACCOUNTS)).unwrap();
    assert_eq!(node.role, Role::Region);
    assert_eq!(node.name.as_deref(), Some(b"LLM account".as_slice()));
}
#[test]
fn the_account_line_precedes_entry_inside_the_composer_area() {
    let view = fixtures::page_view(&task_root());
    let nodes = view.tree().nodes();
    let area = nodes.iter().position(|n| n.styles.as_slice().contains(&crate::look::COMPOSER_AREA)).unwrap();
    let line = nodes.iter().position(|n| n.styles.as_slice().contains(&crate::look::ACCOUNT_LINE)).unwrap();
    let entry = nodes.iter().position(|n| n.styles.as_slice().contains(&crate::look::ENTRY)).unwrap();
    assert_eq!(nodes[line].role, Role::Paragraph);
    assert!(area < line && line < entry && line < area + usize::try_from(nodes[area].size).unwrap());
}

#[test]
fn the_start_choice_is_a_sibling_before_entry() {
    let view = fixtures::page_view(&catalogue(b"first", &[]));
    let nodes = view.tree().nodes();
    let choice = nodes.iter().position(|n| n.styles.as_slice().contains(&crate::look::ACCOUNT_LINE)).unwrap() - 1;
    let entry = nodes.iter().position(|n| n.styles.as_slice().contains(&crate::look::ENTRY)).unwrap();
    assert_eq!(nodes[choice].role, Role::Slot);
    assert_eq!(choice + usize::try_from(nodes[choice].size).unwrap(), entry);
    let children = &nodes[entry + 1..entry + usize::try_from(nodes[entry].size).unwrap()];
    let buttons: Vec<_> = children.iter().filter(|n| n.role == Role::Button).map(|n| n.name.as_deref()).collect();
    assert_eq!(buttons, [Some(b"Start chat".as_slice())]);
}
fn edit(root: &mut d::Domain, act: d::pages::choice::Act) {
    fixtures::root_event(root, d::Event::Act { action: d::Action::Pages(d::pages::Act::Choice(act)) });
}

#[test]
fn an_unapplied_model_has_a_separate_draft_label() {
    let mut root = task_root();
    edit(&mut root, d::pages::choice::Act::Model { index: 1 });
    let shown = text(&root);
    assert!(shown.contains("Using chosen"));
    assert!(shown.contains("Next run if applied: second"));
    assert!(!shown.contains("Using second"));
}
fn named_account() -> d::Domain {
    one_account(3, false)
}
fn one_account(number: u32, plan: bool) -> d::Domain {
    let mut accounts = List::with_capacity(1);
    accounts
        .push(c::OwnerAccountLine {
            number,
            place: 1,
            uses: None,
            plan,
            provider: None,
            provider_name: None,
            manage_usage: None,
            display: c::DocumentText::complete(c::Text::new(Box::from(*b"Personal"), 32).unwrap()),
            holder: c::OwnerAccountHolder::Party(73),
            sources: List::with_capacity(0),
            cut: false,
        })
        .unwrap();
    account_facts(None, accounts)
}

#[test]
fn an_unread_readiness_withholds_the_accounts_words() {
    assert!(text(&one_account(7, true)).contains("Using chosen"));
}
#[test]
fn a_requested_account_waits_for_readiness() {
    let root = named_account();
    assert!(text(&root).contains("Using chosen"));
}
#[test]
fn a_waiting_account_shows_its_display() {
    let root = named_account();
    let mut b = Builder::new(&fixtures::tree_limits());
    b.open(Role::Page);
    crate::choice::waiting(
        &mut b,
        &root,
        Some(c::TaskRunWait::Account { account: Some(3), source: None, why: c::TaskWaitWhy::Refreshing }),
        &fixtures::words(),
        &mut crate::frame::Bindings::new(32),
        &mut crate::words::Times::new(fixtures::NOW),
    );
    b.close();
    assert!(String::from_utf8(fixtures::text(&b.finish()).into_vec()).unwrap().contains("Waiting account: Personal"));
}

#[test]
fn a_task_waiting_on_a_moving_account_says_why() {
    let root = named_account();
    let mut b = Builder::new(&fixtures::tree_limits());
    b.open(Role::Page);
    crate::choice::waiting(
        &mut b,
        &root,
        Some(c::TaskRunWait::Account {
            account: Some(3),
            source: Some(8),
            why: c::TaskWaitWhy::Source(c::TaskSourceState::Moving),
        }),
        &fixtures::words(),
        &mut crate::frame::Bindings::new(32),
        &mut crate::words::Times::new(fixtures::NOW),
    );
    b.close();
    let tree = b.finish();
    assert!(
        fixtures::find(&tree, Role::Paragraph, b"Waiting because the account is moving to another deployment")
            .is_some()
    );
    assert!(!tree.nodes().iter().any(|node| node.role == Role::Button || node.role == Role::Link));
}

#[test]
fn a_refused_next_run_says_why() {
    use c::{OpenProgram as P, OwnerHelloRefusal as F, OwnerHostKind as H, TaskWaitRefused as R};
    for (name, refusal, expected) in [
        (
            "model",
            R::Program(P::Model),
            "Next run refused · This model runs through a program. A chat can't stay open through a program yet. To chat now, choose a model that doesn't run through a program.",
        ),
        (
            "source",
            R::Program(P::Source { account: 3, source: 8 }),
            "Next run refused · Account 3 would use its program source 8. A chat can't stay open through a program yet. This chat does not switch to another source on its own.",
        ),
        (
            "engine",
            R::Hello { kind: H::Engine, at: fixtures::NOW, why: F::ReservedChannel },
            "Next run refused · An engine host was refused at 00:00. Channel zero belongs to the engine's own host.",
        ),
        (
            "worker",
            R::Hello { kind: H::Worker, at: fixtures::NOW, why: F::Position { worker: 7 } },
            "Next run refused · A worker was refused at 00:00. Unconfigured or occupied worker position 7.",
        ),
    ] {
        let root = task_root();
        let mut builder = Builder::new(&fixtures::tree_limits());
        let mut times = crate::words::Times::new(fixtures::NOW);
        crate::choice::waiting(
            &mut builder,
            &root,
            Some(c::TaskRunWait::Refused(refusal)),
            &fixtures::words(),
            &mut crate::frame::Bindings::new(32),
            &mut times,
        );
        assert_eq!(fixtures::text(&builder.finish()).as_ref(), expected.as_bytes(), "{name}");
        let midnight = match refusal {
            R::Hello { .. } => Some(skein_lib::Wall::from_nanos(86_400_000_000_000)),
            R::Program(_) => None,
        };
        assert_eq!(times.next, midnight, "{name}");
        let source = fixtures::chat::messaging_root();
        let mut task = source.pages().chat().unwrap().task(source.client()).unwrap().clone();
        task.flags.ended_by = c::EndedBy::Requester;
        task.run_wait = Some(c::TaskRunWait::Refused(refusal));
        let c::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner") };
        *phase = c::OwnerChipPhase::Active(c::RunStage::Due);
        let (mut root, _, watch) =
            fixtures::root_at(Address::Chat { task: fixtures::chat::TASK }, &fixtures::domain_limits());
        delivered(&mut root, watch.unwrap(), c::Delivery::Task(Box::new(task)));
        let view = fixtures::page_view(&root);
        assert!(fixtures::find(view.tree(), Role::Status, b"Cannot start yet").is_some(), "{name}");
        assert!(fixtures::find(view.tree(), Role::Paragraph, expected.as_bytes()).is_some(), "{name}");
        assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_some(), "{name}");
        assert!(fixtures::find(view.tree(), Role::Button, b"Send").is_some(), "{name}");
        assert_eq!(view.next_change(), midnight, "{name}");
    }
}

#[test]
fn every_hello_refusal_keeps_its_cause_and_numbers() {
    use c::{OwnerAgentShortfall as S, OwnerHelloRefusal as F};
    for (why, cause) in [
        (F::Workers { limit: 7 }, "No room for another worker. Limit: 7."),
        (
            F::StopBound { grace: skein_lib::Duration::from_secs(30) },
            "It would not stop before the engine's grace ends. Allowed grace: 30.",
        ),
        (F::Agent { why: S::Model { charter: 3, model: 8 } }, "Agent model missing for charter 3. Model position 8."),
        (
            F::Agent { why: S::Window { charter: 3, model: 8 } },
            "Agent window too small for charter 3. Model position 8.",
        ),
        (
            F::Agent { why: S::Output { charter: 3, model: 8 } },
            "Agent output too small for charter 3. Model position 8.",
        ),
        (F::Agent { why: S::Turns }, "Agent turns too few."),
        (F::Agent { why: S::Spend }, "Agent spend too small."),
        (F::Agent { why: S::Time }, "Agent time too short."),
        (F::Agent { why: S::Waiting }, "Agent waiting too short."),
        (F::Agent { why: S::Checks }, "Agent checks exceed the change reserve."),
        (F::Agent { why: S::Message }, "Agent message bound too small."),
        (F::Agent { why: S::Inbox }, "Agent inbox too small."),
        (F::Agent { why: S::Unacknowledged }, "Agent unacknowledged turns too few."),
    ] {
        let mut builder = Builder::new(&fixtures::tree_limits());
        crate::choice::waiting(
            &mut builder,
            &task_root(),
            Some(c::TaskRunWait::Refused(c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Worker,
                at: fixtures::NOW,
                why,
            })),
            &fixtures::words(),
            &mut crate::frame::Bindings::new(32),
            &mut crate::words::Times::new(fixtures::NOW),
        );
        let expected = format!("Next run refused · A worker was refused at 00:00. {cause}");
        assert_eq!(fixtures::text(&builder.finish()).as_ref(), expected.as_bytes());
    }
}

fn source(kind: c::OwnerAccountKind, usable: bool) -> c::OwnerAccountSource {
    c::OwnerAccountSource {
        name: Some(Box::from(b"claude-robin".as_slice())),
        place: 1,
        number: 1,
        kind,
        state: c::OwnerAccountState::Granted,
        usable,
        expiring: false,
        attention: false,
        spent_until: None,
    }
}
fn account(number: u32, kind: c::OwnerAccountKind, address: Option<&[u8]>) -> c::OwnerAccountLine {
    let mut sources = List::with_capacity(2);
    sources.push(source(c::OwnerAccountKind::Program, false)).unwrap();
    sources.push(source(kind, true)).unwrap();
    c::OwnerAccountLine {
        number,
        place: number,
        uses: None,
        plan: kind == c::OwnerAccountKind::SignIn,
        provider: Some(17),
        provider_name: None,
        manage_usage: address.map(Box::from),
        display: c::DocumentText::complete(c::Text::new(Box::from(b"Personal".as_slice()), 32).unwrap()),
        holder: c::OwnerAccountHolder::Party(73),
        sources,
        cut: false,
    }
}
fn accounts(rows: &[c::OwnerAccountLine]) -> List<c::OwnerAccountLine> {
    let mut accounts = List::with_capacity(u32::try_from(rows.len()).unwrap());
    for row in rows {
        accounts.push(row.clone()).unwrap();
    }
    accounts
}
fn using_text(root: &d::Domain) -> String {
    let view = fixtures::page_view(root);
    let at = view.tree().nodes().iter().position(|n| n.styles.as_slice().contains(&crate::look::ACCOUNT_LINE)).unwrap();
    String::from_utf8(fixtures::node_text(view.tree(), at).into_vec()).unwrap()
}
#[test]
fn a_keys_line_uses_the_configured_provider_name_and_appends_effort() {
    let root = line_facts(
        Some(readiness(c::Readiness::Usable)),
        accounts(&[account(3, c::OwnerAccountKind::Key, None)]),
        Some(3),
        Some(7),
        Some(b"high"),
        true,
    );
    assert_eq!(using_text(&root), "Using your Supplied Provider API key · chosen · high");
}
#[test]
fn a_plan_has_a_manage_usage_link_only_for_a_safe_address() {
    for address in [
        None,
        Some(b"https://provider.example/usage".as_slice()),
        Some(b"javascript:bad"),
        Some(b"https://provider.example/usage?secret"),
    ] {
        let root = line_facts(
            Some(readiness(c::Readiness::Usable)),
            accounts(&[account(3, c::OwnerAccountKind::SignIn, address)]),
            Some(3),
            None,
            None,
            true,
        );
        let view = fixtures::page_view(&root);
        let link =
            view.tree().nodes().iter().find(|n| n.role == Role::Link && n.name.as_deref() == Some(b"Manage usage"));
        let safe = address == Some(b"https://provider.example/usage".as_slice());
        assert_eq!(link.is_some(), safe);
        if let Some(link) = link {
            assert_eq!(link.external.as_deref(), address);
        }
        assert_eq!(
            using_text(&root),
            if safe {
                "Using Supplied Provider plan · chosen · Manage usage"
            } else {
                "Using Supplied Provider plan · chosen"
            }
        );
    }
}
#[test]
fn a_program_line_names_the_subscription_and_program() {
    let (mut root, me_watch, task_watch, _) = line_fixture(
        Some(readiness(c::Readiness::Usable)),
        accounts(&[account(3, c::OwnerAccountKind::Program, None)]),
        Some(3),
        None,
        Some(b"high"),
        true,
    );
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    let offer = me.providers.get_mut(0).unwrap();
    offer.name = c::DocumentText::complete(c::Text::new(Box::from(b"anthropic".as_slice()), 32).unwrap());
    offer.key = false;
    offer.sign_in = false;
    offer.programs = List::with_capacity(1);
    offer.programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
    me.projects.get_mut(0).unwrap().models.get_mut(0).unwrap().name =
        c::Text::new(Box::from(b"Opus".as_slice()), 32).unwrap();
    let mut task = root.pages().chat().unwrap().task(root.client()).unwrap().clone();
    task.llm.as_mut().unwrap().main.model = c::Text::new(Box::from(b"Opus".as_slice()), 32).unwrap();
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    delivered(&mut root, task_watch.unwrap(), c::Delivery::Task(Box::new(task)));
    assert_eq!(using_text(&root), "Using your Claude subscription, through Claude Code · Opus · high");
    let rendered = text(&root);
    assert!(!rendered.contains("Last claimed account") && !rendered.contains("Requested for next run"));
}

#[test]
fn preference_then_kept_then_the_first_usable_account_decides_the_line() {
    let rows = accounts(&[
        account(1, c::OwnerAccountKind::Key, None),
        account(7, c::OwnerAccountKind::SignIn, None),
        account(3, c::OwnerAccountKind::Key, None),
    ]);
    for (preference, kept, expected) in [
        (Some(3), Some(7), "Using your Supplied Provider API key · chosen"),
        (None, Some(7), "Using Supplied Provider plan · chosen"),
        (None, None, "Using your Supplied Provider API key · chosen"),
    ] {
        let root = line_facts(Some(readiness(c::Readiness::Usable)), rows.clone(), preference, kept, None, true);
        assert_eq!(using_text(&root), expected);
    }
}
#[test]
fn automatic_accounts_skip_other_providers_payers_and_unusable_sources() {
    let mut other_provider = account(1, c::OwnerAccountKind::SignIn, None);
    other_provider.provider = Some(18);
    let mut other_payer = account(2, c::OwnerAccountKind::Program, None);
    other_payer.uses = Some(List::with_capacity(0));
    let mut unusable = account(3, c::OwnerAccountKind::SignIn, None);
    unusable.sources.get_mut(1).unwrap().usable = false;
    let rows = accounts(&[other_provider, other_payer, unusable, account(4, c::OwnerAccountKind::Key, None)]);
    let root = line_facts(Some(readiness(c::Readiness::Usable)), rows, None, None, None, true);
    assert_eq!(using_text(&root), "Using your Supplied Provider API key · chosen");
}
#[test]
fn a_new_chats_default_is_the_first_model_usable_for_its_payer() {
    let mut ready = readiness(c::Readiness::Usable);
    ready.payer = c::OwnerAccountUse::Party(73);
    let root =
        line_facts(Some(ready), accounts(&[account(3, c::OwnerAccountKind::Key, None)]), None, None, None, false);
    assert_eq!(using_text(&root), "Using your OpenAI API key · gpt-6-luna");
}
#[test]
fn the_three_unsettled_lines_name_only_known_facts() {
    assert_eq!(using_text(&task_root()), "Using chosen");
    assert_eq!(using_text(&fixtures::chats_root()), "Using the default model");
    assert_eq!(using_text(&account_readiness(Some(readiness(c::Readiness::Unavailable)))), "chosen · needs an account");
}
#[test]
fn change_model_contains_the_controls_and_is_never_expanded() {
    let mut root = task_root();
    edit(&mut root, d::pages::choice::Act::Model { index: 1 });
    let view = fixtures::page_view(&root);
    let nodes = view.tree().nodes();
    let at =
        nodes.iter().position(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Change model")).unwrap();
    assert!(!nodes[at].states.as_slice().contains(&skein_ui::State::Expanded));
    let end = at + usize::try_from(nodes[at].size).unwrap();
    for (role, name) in [
        (Role::Button, b"chosen".as_slice()),
        (Role::Button, b"second"),
        (Role::Button, b"Reset choice"),
        (Role::Paragraph, b"Next run if applied: second"),
        (Role::Button, b"Apply to next run"),
    ] {
        assert!(nodes[at + 1..end].iter().any(|n| n.role == role && n.name.as_deref() == Some(name)));
    }
}

pub(super) fn ordinary_chats() -> d::Domain {
    catalogue(b"first", &[])
}
#[test]
fn a_live_chat_without_message_keeps_the_line_and_chooser() {
    let (mut root, me_watch, watch, _, _) =
        fixtures::root_at_full(Address::Chat { task: 9 }, &fixtures::limits(), b"Shop");
    let template = task_root();
    let mut task = template.pages().chat().unwrap().task(template.client()).unwrap().clone();
    let mut acts = List::with_capacity(1);
    acts.push(c::Offered { act: c::ActKind::Amend, admission: c::Admission::Allowed, effect: None }).unwrap();
    task.acts = acts;
    // Reuse the model catalogue on a root with an observable task watch.
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.projects.get_mut(0).unwrap().models =
        d::pages::accounts::current_me(template.client()).unwrap().projects.get(0).unwrap().models.clone();
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    delivered(&mut root, watch.unwrap(), c::Delivery::Task(Box::new(task)));
    let view = fixtures::page_view(&root);
    assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_none());
    assert_eq!(using_text(&root), "Using chosen");
    assert!(fixtures::find(view.tree(), Role::Disclosure, b"Change model").is_some());
    assert!(view.tree().nodes().iter().any(|n| n.styles.as_slice().contains(&crate::look::ACCOUNT_LINE)));
}

#[test]
fn pending_choice_outcome_and_wait_are_siblings_after_the_chooser() {
    let (mut root, me_watch, watch, _, _) =
        fixtures::root_at_full(Address::Chat { task: 9 }, &fixtures::limits(), b"Shop");
    let template = task_root();
    let me = d::pages::accounts::current_me(template.client()).unwrap().clone();
    let mut task = template.pages().chat().unwrap().task(template.client()).unwrap().clone();
    task.run_wait = Some(c::TaskRunWait::Credential);
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    delivered(&mut root, watch.unwrap(), c::Delivery::Task(Box::new(task)));
    edit(&mut root, d::pages::choice::Act::Model { index: 1 });
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&fixtures::limits()));
    d::step(
        &mut root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: fixtures::limits() },
        d::Event::Act { action: d::Action::Pages(d::pages::Act::Choice(d::pages::choice::Act::Apply)) },
        &mut out,
    );
    let pending = fixtures::page_view(&root);
    assert!(fixtures::find(pending.tree(), Role::Disclosure, b"Change model").is_none());
    let nodes = pending.tree().nodes();
    let status = nodes.iter().position(|n| n.name.as_deref() == Some(b"Next-run choice request in progress")).unwrap();
    let wait = nodes.iter().position(|n| n.name.as_deref() == Some(b"Waiting for a credential")).unwrap();
    assert!(status < wait);
    d::step(
        &mut root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: fixtures::limits() },
        d::Event::Client { event: jig_client::Event::Stored },
        &mut out,
    );
    let attempt = out
        .iter()
        .find_map(|request| {
            if let d::Request::Client {
                request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } },
            } = request
            {
                Some(*attempt)
            } else {
                None
            }
        })
        .unwrap();
    fixtures::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Changed))),
                    why: None,
                },
            },
        },
    );
    let view = fixtures::page_view(&root);
    let nodes = view.tree().nodes();
    let chooser =
        nodes.iter().position(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Change model")).unwrap();
    let end = chooser + usize::try_from(nodes[chooser].size).unwrap();
    let outcome = nodes.iter().position(|n| n.role == Role::Status && n.name.as_deref() == Some(b"Applied")).unwrap();
    let wait = nodes.iter().position(|n| n.name.as_deref() == Some(b"Waiting for a credential")).unwrap();
    assert!(end <= outcome && outcome < wait);
}

#[test]
fn own_key_and_plan_of_openai_follow_place_order() {
    for (first, second, expected) in [
        (c::OwnerAccountKind::Key, c::OwnerAccountKind::SignIn, "Using your OpenAI API key · chosen"),
        (c::OwnerAccountKind::SignIn, c::OwnerAccountKind::Key, "Using ChatGPT plan · chosen"),
    ] {
        let mut rows = accounts(&[account(9, first, None), account(3, second, None)]);
        rows.get_mut(0).unwrap().place = 1;
        rows.get_mut(1).unwrap().place = 2;
        let (mut root, me_watch, _, _) =
            line_fixture(Some(readiness(c::Readiness::Usable)), rows, None, None, None, true);
        let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
        me.providers.get_mut(0).unwrap().name =
            c::DocumentText::complete(c::Text::new(Box::from(b"openai".as_slice()), 32).unwrap());
        delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
        assert_eq!(using_text(&root), expected);
    }
}
#[test]
fn another_members_chat_and_project_work_never_name_the_viewers_plan() {
    for funder in [
        c::Funder::Pool { project: fixtures::PROJECT, person: 74, period: 1 },
        c::Funder::Period { project: fixtures::PROJECT, period: 1 },
    ] {
        let (mut root, _, task_watch, accounts_watch) = line_fixture(
            Some(readiness(c::Readiness::Usable)),
            accounts(&[account(3, c::OwnerAccountKind::SignIn, None)]),
            Some(3),
            Some(3),
            None,
            true,
        );
        let mut task = root.pages().chat().unwrap().task(root.client()).unwrap().clone();
        task.budget.funder = funder;
        let mut doc = d::pages::accounts::document(root.client()).unwrap().clone();
        doc.readiness.get_mut(0).unwrap().payer = match funder {
            c::Funder::Pool { person, .. } => c::OwnerAccountUse::Party(person),
            c::Funder::Period { project, .. } => c::OwnerAccountUse::Project(project),
            c::Funder::Recurring { .. } | c::Funder::Task(_) => unreachable!("fixture funder"),
        };
        delivered(&mut root, task_watch.unwrap(), c::Delivery::Task(Box::new(task)));
        delivered(&mut root, accounts_watch, c::Delivery::OwnerAccounts(Box::new(doc)));
        let line = using_text(&root);
        assert_eq!(line, "Using chosen");
        assert!(!line.contains("plan") && !line.contains("API key"));
    }
}
#[test]
fn a_held_pin_stays_pinned_while_fresh_selection_passes_to_the_next_account() {
    for state in [
        c::OwnerAccountState::Later { until: None },
        c::OwnerAccountState::SignInAgain,
        c::OwnerAccountState::Misconfigured,
        c::OwnerAccountState::Unloaded { at: c::OwnerAccountPlace::File, why: c::OwnerAccountUnread::Unreadable },
        c::OwnerAccountState::Cancelled,
        c::OwnerAccountState::Closing,
    ] {
        let mut held = account(3, c::OwnerAccountKind::SignIn, None);
        held.sources.get_mut(1).unwrap().state = state;
        if let c::OwnerAccountState::Later { .. } = state {
            // An unusable Later source waits in both possible underlying states.
            held.sources.get_mut(1).unwrap().usable = false;
        } else {
            // Even contradictory usable evidence cannot override a held deciding source.
            assert!(held.sources.get(1).unwrap().usable);
        }
        let rows = accounts(&[held, account(7, c::OwnerAccountKind::Key, None)]);
        for (pin, expected) in
            [(Some(3), "chosen · needs an account"), (None, "Using your Supplied Provider API key · chosen")]
        {
            let root = line_facts(Some(readiness(c::Readiness::Usable)), rows.clone(), pin, None, None, true);
            assert_eq!(using_text(&root), expected);
        }
    }
}
#[test]
fn a_moving_source_is_held_even_if_supplied_as_usable() {
    let mut moving = account(3, c::OwnerAccountKind::SignIn, None);
    moving.sources.get_mut(1).unwrap().state = c::OwnerAccountState::Moving;
    assert!(moving.sources.get(1).unwrap().usable);
    let rows = accounts(&[moving, account(7, c::OwnerAccountKind::Key, None)]);
    for (pin, expected) in
        [(Some(3), "chosen · needs an account"), (None, "Using your Supplied Provider API key · chosen")]
    {
        let root = line_facts(Some(readiness(c::Readiness::Usable)), rows.clone(), pin, None, None, true);
        assert_eq!(using_text(&root), expected);
    }
}
#[test]
fn a_waiting_direct_source_does_not_jump_to_a_later_direct_source() {
    let mut waiting = account(3, c::OwnerAccountKind::Key, None);
    waiting.sources.get_mut(0).unwrap().kind = c::OwnerAccountKind::SignIn;
    let rows = accounts(&[waiting, account(7, c::OwnerAccountKind::SignIn, None)]);
    for (pin, expected) in [(Some(3), "chosen · needs an account"), (None, "Using Supplied Provider plan · chosen")] {
        let root = line_facts(Some(readiness(c::Readiness::Usable)), rows.clone(), pin, None, None, true);
        assert_eq!(using_text(&root), expected);
    }
}
#[test]
fn ineligible_preference_passes_to_the_kept_account() {
    let mut wrong = account(3, c::OwnerAccountKind::Key, None);
    wrong.uses = Some(List::with_capacity(0));
    let root = line_facts(
        Some(readiness(c::Readiness::Usable)),
        accounts(&[wrong, account(7, c::OwnerAccountKind::SignIn, None)]),
        Some(3),
        Some(7),
        None,
        true,
    );
    assert_eq!(using_text(&root), "Using Supplied Provider plan · chosen");
}
#[test]
fn truncated_documents_and_accounts_name_only_the_model() {
    for cut_document in [true, false] {
        let (mut root, _, _, watch) = line_fixture(
            Some(readiness(c::Readiness::Usable)),
            accounts(&[account(3, c::OwnerAccountKind::Key, None)]),
            Some(3),
            None,
            None,
            true,
        );
        let mut doc = d::pages::accounts::document(root.client()).unwrap().clone();
        if cut_document {
            doc.cut = true;
        } else {
            doc.accounts.get_mut(0).unwrap().cut = true;
        }
        delivered(&mut root, watch, c::Delivery::OwnerAccounts(Box::new(doc)));
        assert_eq!(using_text(&root), "Using chosen");
    }
}
#[test]
fn readiness_cross_checks_the_translated_account_across_expiry() {
    for ready in [c::Readiness::Unknown, c::Readiness::Unavailable] {
        let root = line_facts(
            Some(readiness(ready)),
            accounts(&[account(3, c::OwnerAccountKind::Key, None)]),
            Some(3),
            None,
            None,
            true,
        );
        assert_eq!(using_text(&root), "Using chosen");
    }
}

#[test]
fn mixed_or_cut_program_offers_do_not_guess_a_models_endpoint() {
    for cut in [false, true] {
        let (mut root, me_watch, _, _) = line_fixture(
            Some(readiness(c::Readiness::Usable)),
            accounts(&[account(3, c::OwnerAccountKind::Key, None)]),
            Some(3),
            None,
            None,
            true,
        );
        let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
        let offer = me.providers.get_mut(0).unwrap();
        offer.programs = List::with_capacity(1);
        offer.programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
        offer.programs_cut = cut;
        delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
        assert_eq!(using_text(&root), "Using chosen");
    }
}

#[test]
fn a_usable_later_source_names_no_account() {
    for (ready, pin) in [
        (c::Readiness::Usable, None),
        (c::Readiness::Unavailable, None),
        (c::Readiness::Usable, Some(3)),
        (c::Readiness::Unavailable, Some(3)),
    ] {
        let mut first = account(3, c::OwnerAccountKind::Key, None);
        first.sources.get_mut(1).unwrap().state = c::OwnerAccountState::Later { until: None };
        let rows = accounts(&[first, account(7, c::OwnerAccountKind::SignIn, None)]);
        let (mut root, me_watch, _, _) = line_fixture(Some(readiness(ready)), rows, pin, None, None, true);
        let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
        me.providers.get_mut(0).unwrap().name =
            c::DocumentText::complete(c::Text::new(Box::from(b"openai".as_slice()), 32).unwrap());
        delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
        assert_eq!(
            using_text(&root),
            if ready == c::Readiness::Unavailable { "chosen · needs an account" } else { "Using chosen" }
        );
    }
}
#[test]
fn an_unusable_later_program_source_names_no_account_when_a_program_source_after_it_would_be_chosen() {
    for (ready, pin, after_usable) in [
        (c::Readiness::Usable, None, true),
        (c::Readiness::Unavailable, None, true),
        (c::Readiness::Usable, Some(3), true),
        (c::Readiness::Unavailable, Some(3), true),
        (c::Readiness::Usable, None, false),
    ] {
        // jig's Later goes on to the account's next program source; its Limited waits.
        let mut first = account(3, c::OwnerAccountKind::Program, None);
        first.sources.get_mut(0).unwrap().state = c::OwnerAccountState::Later { until: None };
        first.sources.get_mut(1).unwrap().usable = after_usable;
        let rows = accounts(&[first, account(7, c::OwnerAccountKind::Program, None)]);
        let (mut root, me_watch, _, _) = line_fixture(Some(readiness(ready)), rows, pin, None, None, true);
        let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
        let offer = me.providers.get_mut(0).unwrap();
        offer.name = c::DocumentText::complete(c::Text::new(Box::from(b"anthropic".as_slice()), 32).unwrap());
        offer.key = false;
        offer.sign_in = false;
        offer.programs = List::with_capacity(1);
        offer.programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
        delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
        let expected = match (after_usable, ready) {
            (true, c::Readiness::Unavailable) => "chosen · needs an account",
            (true, _) => "Using chosen",
            // Both of jig's states wait on this account, so the next one is chosen.
            (false, _) => "Using your Claude subscription, through Claude Code · chosen",
        };
        assert_eq!(using_text(&root), expected, "ready {ready:?}, pin {pin:?}");
    }
}

#[test]
fn ready_pairs_the_default_model_with_its_selected_account() {
    let root = line_facts(
        Some(c::PayerReadiness {
            provider: 17,
            payer: c::OwnerAccountUse::Party(73),
            readiness: c::Readiness::Usable,
            settled: Some(true),
        }),
        accounts(&[account(3, c::OwnerAccountKind::SignIn, None)]),
        None,
        None,
        None,
        false,
    );
    // Ready names the account by the heading the accounts page draws, then the model.
    let heading = crate::pages::accounts::first_usable_heading(&root, &fixtures::words()).expect("a usable account");
    let expected = [heading.as_ref(), " · gpt-6-luna".as_bytes()].concat();
    assert_eq!(crate::choice::ready_model(&root, &fixtures::words()).as_ref(), expected.as_slice());
}
#[test]
fn ready_does_not_pair_another_providers_account_with_the_model() {
    let mut other = account(3, c::OwnerAccountKind::SignIn, None);
    other.provider = Some(18);
    let root = line_facts(
        Some(c::PayerReadiness {
            provider: 17,
            payer: c::OwnerAccountUse::Party(73),
            readiness: c::Readiness::Usable,
            settled: Some(true),
        }),
        accounts(&[other]),
        None,
        None,
        None,
        false,
    );
    assert_eq!(crate::choice::ready_model(&root, &fixtures::words()).as_ref(), b"gpt-6-luna");
}

/// Keep the catalogue unchanged across the playground's retirement.
pub(super) fn playground_offers_root(address: Address, playground: bool) -> (d::Domain, Token) {
    use temper_client_protocol_wire::v1 as w;
    let (mut root, me_watch, _, _, accounts, forges) = fixtures::root_at_with_forges(
        address,
        &fixtures::domain_limits(),
        b"Shop",
        playground.then_some((fixtures::PROJECT, true, w::DemoState::Running)),
        None,
        Some(super::connect_forge::rows(w::Connection::Connecting, w::Offer::Offered)),
    );
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.providers = List::with_capacity(2);
    for (provider, name, sign_in) in [(17, b"demo".as_slice(), false), (18, b"openai".as_slice(), true)] {
        me.providers
            .push(c::ProviderOffer {
                provider,
                name: super::connect_model::text(name),
                key_bytes: 4096,
                sign_in,
                signed_in_plan: sign_in,
                key: true,
                programs: List::with_capacity(0),
                programs_cut: false,
                none: false,
                manage_usage: None,
            })
            .unwrap();
    }
    delivered(&mut root, me_watch, c::Delivery::Me(Box::new(me)));
    delivered(
        &mut root,
        accounts.unwrap(),
        c::Delivery::OwnerAccounts(Box::new(c::OwnerAccounts {
            program_hosts: List::with_capacity(0),
            accounts: List::with_capacity(0),
            signing: List::with_capacity(0),
            last_end: None,
            usable: false,
            settled: true,
            cut: false,
            readiness: List::with_capacity(0),
            readiness_cut: false,
        })),
    );
    (root, forges.unwrap())
}

pub(super) fn retire_playground(root: &mut d::Domain, watch: Token) -> Token {
    use temper_client_protocol_wire::v1 as w;
    let mut out = super::connect_forge::deliver(
        root,
        watch,
        super::connect_forge::rows(super::connect_forge::connected(), w::Offer::Offered),
    );
    while let Some(request) = out.pop() {
        if let d::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read, what: jig_client::Readable::Family { record, .. } },
                },
        } = request
        {
            assert_eq!(
                temper_client_protocol_wire::decode_reading(&record.bytes, &w::CEILINGS).unwrap().read(),
                &w::Read::Demo
            );
            return read;
        }
    }
    panic!("a real forge's connection rereads the demo");
}

pub(super) fn answer_retired_playground(root: &mut d::Domain, read: Token) {
    fixtures::root_event(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::ReadDone {
                    read,
                    result: jig_client::Readout::Family { family: temper_client_protocol_wire::FORGE, record: None },
                },
            },
        },
    );
    assert!(root.forge().demo().is_none());
}

#[test]
fn forge_banners_exclude_the_playgrounds_provider_before_and_after_retirement() {
    for address in [Address::ConnectForge { forge: None }, Address::Forges] {
        let (mut root, watch) = playground_offers_root(address.clone(), true);
        let check = |root: &d::Domain| {
            let mut view = fixtures::page_view(root);
            let tree = view.tree();
            assert!(fixtures::find(tree, Role::Paragraph, b"Agents need an LLM account to run.").is_some());
            assert!(fixtures::find(tree, Role::Link, b"Use a demo API key").is_none());
            let sign_in = fixtures::find(tree, Role::Link, b"Continue with ChatGPT").unwrap();
            let key = fixtures::find(tree, Role::Link, b"Use an API key").unwrap();
            for (node, kind) in [(sign_in, d::WayKind::SignIn), (key, d::WayKind::Key)] {
                assert_eq!(
                    crate::act(&mut view, node, skein_ui::Act::Press),
                    Some(d::Action::Pages(d::pages::Act::Go {
                        address: Address::ConnectModel { way: Some(d::ModelWay { provider: 18, source: None, kind }) }
                    }))
                );
            }
        };
        check(&root);
        let read = retire_playground(&mut root, watch);
        check(&root);
        answer_retired_playground(&mut root, read);
        check(&root);
        // A provider named demo in a configuration without a playground is a legitimate key offer.
        let (root, _) = playground_offers_root(address, false);
        assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Link, b"Use a demo API key").is_some());
    }
}

#[test]
fn a_configured_openai_key_names_openai_on_the_whole_line() {
    let (mut root, me, _, _) = line_fixture(
        Some(readiness(c::Readiness::Usable)),
        accounts(&[account(3, c::OwnerAccountKind::Key, None)]),
        Some(3),
        None,
        None,
        true,
    );
    configured_provider(&mut root, me, b"openai");
    assert_eq!(using_text(&root), "Using your OpenAI API key · chosen");
}
#[test]
fn a_configured_openai_plan_names_chatgpt_and_manage_usage_on_the_whole_line() {
    let (mut root, me, _, _) = line_fixture(
        Some(readiness(c::Readiness::Usable)),
        accounts(&[account(3, c::OwnerAccountKind::SignIn, Some(b"https://provider.example/usage"))]),
        Some(3),
        None,
        None,
        true,
    );
    configured_provider(&mut root, me, b"openai");
    assert_eq!(using_text(&root), "Using ChatGPT plan · chosen · Manage usage");
}
fn configured_provider(root: &mut d::Domain, watch: Token, name: &[u8]) {
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.providers.get_mut(0).unwrap().name = c::DocumentText::complete(c::Text::new(Box::from(name), 32).unwrap());
    delivered(root, watch, c::Delivery::Me(Box::new(me)));
}
