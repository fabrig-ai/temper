use super::fixtures;
use crate::look::*;
use skein_ui::Role;

#[test]
fn the_frame_carries_its_tokens() {
    let view = fixtures::page_view(&fixtures::inbox_root());
    for (role, token) in [(Role::Banner, BANNER), (Role::Navigation, NAVIGATION)] {
        assert_eq!(fixtures::styles(view.tree(), role).as_ref(), [Box::from([token])], "{role:?}");
    }
}
#[test]
fn the_looks_names_pass_skeins_check() {
    assert!(skein_ui::look::check(&[NAMES]).is_none());
}
const FLOW_NAMES: [(skein_ui::Style, &[u8]); 23] = [
    (FLOW_PRIMARY, b"temper-flow-primary"),
    (FLOW_FAILED, b"temper-flow-failed"),
    (FLOW_PAIR, b"temper-flow-pair"),
    (WELCOME_INTRO, b"temper-welcome-intro"),
    (PROJECT_EMPTY, b"temper-project-empty"),
    (FLOW_SECONDARY, b"temper-flow-secondary"),
    (FLOW_QUIET, b"temper-flow-quiet"),
    (FLOW_CHOICES, b"temper-flow-choices"),
    (FLOW_CHOICE, b"temper-flow-choice"),
    (FLOW_ACTIONS, b"temper-flow-actions"),
    (FLOW_FIELD, b"temper-flow-field"),
    (FLOW_FACTS, b"temper-flow-facts"),
    (MODEL_CHOICES, b"temper-model-choices"),
    (FLOW_DETAILS, b"temper-flow-details"),
    (MODEL_KEY, b"temper-model-key"),
    (ADMIN_FIRST, b"temper-admin-first"),
    (ADMIN_LEGEND, b"temper-admin-legend"),
    (MODEL_FACTS, b"temper-model-facts"),
    (MODEL_ACTIONS, b"temper-model-actions"),
    (FLOW_SUBTITLE, b"temper-flow-subtitle"),
    (MODEL_DESCRIPTION, b"temper-model-description"),
    (MODEL_FACT, b"temper-model-fact"),
    (MODEL_SIGN_IN, b"temper-model-sign-in"),
];
#[test]
fn each_style_or_region_is_named_under_temper() {
    let named: [(skein_ui::Style, &[u8]); 94] = [
        (PROPOSAL_FACTS, b"temper-proposal-facts"),
        (NAV_COUNT, b"temper-nav-count"),
        (BANNER, b"temper-banner"),
        (NAVIGATION, b"temper-navigation"),
        (MAIN, b"temper-main"),
        (SIGN_IN, b"temper-sign-in"),
        (CARD, b"temper-card"),
        (TURN, b"temper-turn"),
        (COMPOSER, b"temper-composer"),
        (BRAND, b"temper-brand"),
        (LIVE, b"temper-live"),
        (CONNECTING, b"temper-connecting"),
        (OFFLINE, b"temper-offline"),
        (BEHIND, b"temper-behind"),
        (ROWS, b"temper-rows"),
        (ROW, b"temper-row"),
        (ENTRY, b"temper-entry"),
        (STATUS_YOURS, b"temper-status-yours"),
        (STATUS_HELD, b"temper-status-held"),
        (STATUS_RETRYING, b"temper-status-retrying"),
        (STATUS_ACTIVE, b"temper-status-active"),
        (STATUS_QUIET, b"temper-status-quiet"),
        (TIME, b"temper-time"),
        (HINT, b"temper-hint"),
        (CORNER, b"temper-corner"),
        (ACCOUNTS, b"temper-accounts"),
        (ACCOUNT_LINE, b"temper-account-line"),
        (CHAT, b"temper-chat"),
        (CHATBAR, b"temper-chatbar"),
        (THREAD, b"temper-thread"),
        (MESSAGE, b"temper-message"),
        (BY_PERSON, b"temper-by-person"),
        (BY_AGENT, b"temper-by-agent"),
        (BY_TOOL, b"temper-by-tool"),
        (BY_SYSTEM, b"temper-by-system"),
        (WHO, b"temper-who"),
        (SAYS, b"temper-says"),
        (WORK, b"temper-work"),
        (MARKER, b"temper-marker"),
        (CARDS, b"temper-cards"),
        (COMPOSER_AREA, b"temper-composer-area"),
        (PAGE, b"temper-page"),
        (PAGEBAR, b"temper-pagebar"),
        (HEADING, b"temper-heading"),
        (FILTERS, b"temper-filters"),
        (UPDATES, b"temper-updates"),
        (META, b"temper-meta"),
        (TITLE, b"temper-title"),
        (DETAILS, b"temper-details"),
        (KIND_PROPOSAL, b"temper-kind-proposal"),
        (KIND_QUESTION, b"temper-kind-question"),
        (KIND_HELD, b"temper-kind-held"),
        (KIND_PERSON_TASK, b"temper-kind-person-task"),
        (KIND_RESULT, b"temper-kind-result"),
        (KIND_REPLY, b"temper-kind-reply"),
        (TASK_HEADING, b"temper-task-heading"),
        (TASK_CARD, b"temper-task-card"),
        (PLAN, b"temper-plan"),
        (PLAN_ROW, b"temper-plan-row"),
        (PLAN_HOLD, b"temper-plan-hold"),
        (EVENTS, b"temper-events"),
        (STATUS_DONE, b"temper-status-done"),
        (CHOICE, b"temper-choice"),
        (ACCOUNTS_PAGE, b"temper-accounts-page"),
        (ADOPTION_PAGE, b"temper-adoption-page"),
        (INVITATION, b"temper-invitation"),
        (REFUSAL, b"temper-refusal"),
        (STEPS, b"temper-steps"),
        (STEP, b"temper-step"),
        (STEP_DONE, b"temper-step-done"),
        (UNAVAILABLE, b"temper-unavailable"),
        (TASK_CONTRACT, b"temper-task-contract"),
        (TASK_BUDGET, b"temper-task-budget"),
        (PLAN_SPEND, b"temper-plan-spend"),
        (DEPENDENCIES, b"temper-dependencies"),
        (STATUS_WAITING, b"temper-status-waiting"),
        (AGENT_LINEAGE, b"temper-agent-lineage"),
        (AGENT_BRIEF, b"temper-agent-brief"),
        (AGENT_RUN, b"temper-agent-run"),
        (AGENT_RESULT, b"temper-agent-result"),
        (REPOSITORY, b"temper-repository"),
        (TASK_ACTS, b"temper-task-acts"),
        (AGENT_STEER, b"temper-agent-steer"),
        (TASK_DANGER, b"temper-task-danger"),
        (TASK_AMEND, b"temper-task-amend"),
        (DAY, b"temper-day"),
        (CHANGE, b"temper-change"),
        (CHANGE_TAG, b"temper-change-tag"),
        (STEP_CURRENT, b"temper-step-current"),
        (CHANGE_HOLD, b"temper-change-hold"),
        (GATE, b"temper-gate"),
        (PLAN_DONE, b"temper-plan-done"),
        (ENDED, b"temper-ended"),
        (MENU, b"temper-menu"),
    ];
    for (style, name) in named.into_iter().chain(FLOW_NAMES) {
        assert_eq!(NAMES.style(style), Some(name));
    }
    assert_eq!(NAMES.region(crate::frame::link_region()), Some(&b"temper-link"[..]));
}
#[test]
fn each_pages_main_carries_the_main_token() {
    let builds: [fn() -> temper_client_domain::Domain; 2] = [fixtures::signed_out, fixtures::out_of_date];
    for build in builds {
        let view = fixtures::page_view(&build());
        assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN])]);
    }
    let view = fixtures::page_view(&fixtures::chats_root());
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, PAGE])]);
    let view = fixtures::page_view(&fixtures::task::held_root());
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, PAGE])]);
    let view = fixtures::page_view(&fixtures::inbox_root());
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, PAGE])]);
    let view = fixtures::page_view(&fixtures::account_root().0);
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, PAGE])]);
    let view = fixtures::page_view(&fixtures::adoption_root());
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, PAGE])]);
    let view = fixtures::page_view(&fixtures::chat::chat_root());
    assert_eq!(fixtures::styles(view.tree(), Role::Main).as_ref(), [Box::from([MAIN, CHAT])]);
}
#[test]
fn the_composers_carry_the_composer_token() {
    for root in fixtures::pair(fixtures::chats_root(), fixtures::chat::messaging_root()) {
        let view = fixtures::page_view(&root);
        assert_eq!(fixtures::styles(view.tree(), Role::TextArea).as_ref(), [Box::from([COMPOSER])]);
    }
}

#[test]
fn the_chat_structure_tokens_stay_on_their_roles() {
    let view = fixtures::page_view(&super::chats::listed_root());
    for (token, role) in [
        (BRAND, Role::Link),
        (LIVE, Role::Status),
        (ROWS, Role::List),
        (ROW, Role::Item),
        (ENTRY, Role::Slot),
        (CORNER, Role::Slot),
        (TIME, Role::Paragraph),
        (HINT, Role::Paragraph),
    ] {
        let mut found = false;
        for node in view.tree().nodes() {
            if node.styles.as_slice().contains(&token) {
                assert_eq!(node.role, role);
                found = true;
            }
        }
        assert!(found, "{token:?}");
    }
    for (line, token) in super::chats::status_rows() {
        let (tree, _) = super::chats::tree(&line);
        assert_eq!(fixtures::styles(&tree, Role::Status).as_ref(), [Box::from([token])]);
    }
}
#[test]
fn the_connection_tokens_stay_on_statuses() {
    let (mut root, watch) = fixtures::root_with_watch(None);
    let connecting = fixtures::built();
    assert_status_token(&connecting, CONNECTING);
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Dropped },
            },
        },
    );
    let view = fixtures::page_view(&root);
    for token in [OFFLINE, BEHIND] {
        assert_status_token(&view, token);
    }
}

fn assert_status_token(view: &crate::View, token: skein_ui::Style) {
    for node in view.tree().nodes() {
        if node.role == Role::Status && node.styles.as_slice().contains(&token) {
            return;
        }
    }
    panic!("status token {token:?} absent");
}

#[test]
fn the_conversation_tokens_stay_on_their_roles() {
    let message_tree = fixtures::chat::tree(&super::chat::structured_thread());
    let proposal = fixtures::page_view(&fixtures::chat::proposal_root());
    let composer = fixtures::page_view(&fixtures::chat::messaging_root());
    for token in [
        CHAT,
        CHATBAR,
        THREAD,
        MESSAGE,
        BY_PERSON,
        BY_AGENT,
        BY_TOOL,
        BY_SYSTEM,
        WHO,
        SAYS,
        WORK,
        MARKER,
        CARDS,
        COMPOSER_AREA,
    ] {
        let role = match token {
            CHAT => Role::Main,
            CHATBAR | MARKER => Role::Region,
            CARDS => Role::List,
            _ => Role::Slot,
        };
        let mut found = false;
        for tree in [&message_tree, proposal.tree(), composer.tree()] {
            for node in tree.nodes() {
                if node.styles.as_slice().contains(&token) {
                    if token == WORK && node.role == Role::Disclosure {
                        assert!(!node.states.as_slice().contains(&skein_ui::State::Expanded));
                    } else {
                        assert_eq!(node.role, role);
                    }
                    found = true;
                }
            }
        }
        assert!(found, "{token:?}");
    }
}

#[test]
fn the_inbox_tokens_stay_on_their_roles() {
    let mut roots = skein_lib::List::with_capacity(3);
    roots.push(fixtures::inbox_root()).expect("inbox");
    roots
        .push(super::confirm::root(jig_client::contract::ActKind::Accept, jig_client::contract::Admission::Allowed))
        .expect("proposal");
    roots.push(super::inbox::question_root()).expect("question");
    for (token, role) in [
        (PAGE, Role::Main),
        (PAGEBAR, Role::Slot),
        (HEADING, Role::Slot),
        (FILTERS, Role::Slot),
        (UPDATES, Role::List),
        (META, Role::Slot),
        (TITLE, Role::Heading { level: 3 }),
        (PROPOSAL_FACTS, Role::Slot),
        (SUBTLE_COUNT, Role::Paragraph),
        (KIND_MENU, Role::Disclosure),
        (INBOX_TAG, Role::Paragraph),
        (KIND_PROPOSAL, Role::Item),
        (KIND_QUESTION, Role::Item),
        (KIND_HELD, Role::Item),
        (KIND_PERSON_TASK, Role::Item),
        (KIND_RESULT, Role::Item),
        (KIND_REPLY, Role::Item),
    ] {
        let mut found = false;
        for root in &roots {
            let view = fixtures::page_view(root);
            for node in view.tree().nodes() {
                if node.styles.as_slice().contains(&token) {
                    assert_eq!(node.role, role);
                    found = true;
                }
            }
        }
        assert!(found, "{token:?}");
    }
}

#[test]
fn the_task_tokens_stay_on_their_roles() {
    let view = fixtures::page_view(&fixtures::task::held_root());
    let mut builder = skein_ui::Builder::new(&fixtures::tree_limits());
    let mut task = fixtures::task::document();
    task.chip.phase = jig_client::contract::Phase::Done;
    crate::pages::task::header(
        &mut builder,
        &task,
        None,
        None,
        &fixtures::words(),
        &mut crate::frame::Bindings::new(fixtures::tree_limits().bindings),
        None,
    );
    let done = builder.finish();
    let activity = super::task::activity_tree(&[super::task::activity_news()]);
    let plan = super::task::dependency_plan();
    for (token, role) in [
        (TASK_HEADING, Role::Region),
        (TASK_CONTRACT, Role::Paragraph),
        (TASK_BUDGET, Role::Region),
        (TASK_CARD, Role::Region),
        (PLAN, Role::Tree),
        (PLAN_ROW, Role::Slot),
        (PLAN_SPEND, Role::Paragraph),
        (DEPENDENCIES, Role::List),
        (PLAN_HOLD, Role::List),
        (EVENTS, Role::List),
        (STATUS_DONE, Role::Status),
        (STATUS_WAITING, Role::Status),
    ] {
        let mut found = false;
        for tree in [view.tree(), &done, &activity, &plan] {
            for node in tree.nodes() {
                if node.styles.as_slice().contains(&token) {
                    assert_eq!(node.role, role);
                    found = true;
                }
            }
        }
        assert!(found, "{token:?}");
    }
}

#[test]
fn the_choice_token_stays_on_the_composer_slot() {
    let mut roots = skein_lib::List::with_capacity(3);
    roots.push(fixtures::chats_root()).expect("chats");
    roots.push(fixtures::chat::messaging_root()).expect("chat");
    roots.push(fixtures::task::root(jig_client::contract::Executor::Agent { charter: 1 }, Some(1))).expect("task");
    for root in &roots {
        let view = fixtures::page_view(root);
        let mut found = false;
        for node in view.tree().nodes() {
            if node.styles.as_slice().contains(&CHOICE) {
                assert_eq!(node.role, Role::Slot);
                found = true;
            }
        }
        assert!(found);
    }
}

#[test]
fn the_account_and_adoption_bars_keep_their_headings() {
    // One root at a time: two outgrow a stack array's 16 KiB.
    for token in [ACCOUNTS_PAGE, ADOPTION_PAGE] {
        let root = if token == ACCOUNTS_PAGE { fixtures::account_root().0 } else { fixtures::adoption_root() };
        let view = fixtures::page_view(&root);
        assert_eq!(fixtures::styles(view.tree(), Role::Heading { level: 1 }).as_ref(), [Box::from([])]);
        let mut found = false;
        for node in view.tree().nodes() {
            if node.styles.as_slice().contains(&token) {
                assert_eq!(node.role, Role::Slot);
            }
            if node.styles.as_slice().contains(&PAGEBAR) {
                assert_eq!(node.role, Role::Slot);
                found = true;
            }
        }
        assert!(found);
    }
}

#[test]
fn the_delegation_steps_keep_their_roles() {
    super::delegation::step_roles();
}

#[test]
fn account_rows_and_sign_ins_have_distinct_style_names() {
    assert_eq!(NAMES.style(ACCOUNT_ROW), Some(b"temper-account-row".as_slice()));
    assert_eq!(NAMES.style(SIGN_IN), Some(b"temper-sign-in".as_slice()));
    assert_ne!(ACCOUNT_ROW, SIGN_IN);
}
