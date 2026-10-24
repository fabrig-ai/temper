use super::fixtures;
use skein_ui::{Act, Role};
use temper_client_domain::{Action, pages};
use temper_client_domain::{ChatsFilter, PlanFilter};

#[test]
fn the_navigation_is_named_main_navigation() {
    let view = fixtures::signed_in_with_project();
    assert!(fixtures::person_find(view.tree(), Role::Navigation, b"Main navigation").is_some());
}

#[test]
fn the_navigation_ends_with_accounts() {
    let view = fixtures::signed_in_with_project();
    assert_eq!(
        fixtures::navigation_link_names(&view).as_ref(),
        [
            Box::from(*b"Inbox"),
            Box::from(*b"Chats"),
            Box::from(*b"Adopt a repository"),
            Box::from(*b"Forges"),
            Box::from(*b"Accounts")
        ]
    );
}
#[test]
fn the_banner_says_the_links_state_and_the_persons_name() {
    let view = fixtures::signed_in_with_project();
    assert!(fixtures::find(view.tree(), Role::Status, b"Live").is_some());
    assert!(fixtures::find(view.tree(), Role::Status, b"Pat").is_some());
}
#[test]
fn the_project_chosen_is_the_first_of_me_at_start() {
    let root = fixtures::root_in_project(None);
    assert_eq!(root.pages().project(), Some(fixtures::PROJECT));
}
#[test]
fn the_running_demo_project_button_is_named_playground() {
    let root = fixtures::demo_root(
        temper_client_domain::Address::Chats { filter: ChatsFilter::All, start: false },
        fixtures::PROJECT,
        true,
    );
    for words in [fixtures::words(), fixtures::browser_words()] {
        let mut view = fixtures::view_of(words);
        fixtures::build(&mut view, &root);
        let button = fixtures::find(view.tree(), Role::Button, b"Playground").expect("demo project");
        assert_eq!(
            crate::act(&mut view, button, Act::Press),
            Some(Action::Pages(pages::Act::Project { project: fixtures::PROJECT }))
        );
        assert!(fixtures::find(view.tree(), Role::Button, b"Shop").is_none());
        assert!(fixtures::find(view.tree(), Role::Paragraph, b"No project yet").is_none());
    }
}
#[test]
fn a_navigation_without_projects_says_no_project_yet() {
    let (mut root, watch) = fixtures::root_with_watch(None);
    let mut me = root.pages().me(root.client()).unwrap().clone();
    me.projects.clear();
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(Box::new(
                        me,
                    ))),
                },
            },
        },
    );
    for words in [fixtures::words(), fixtures::browser_words()] {
        let mut view = fixtures::view_of(words);
        let tree = fixtures::build(&mut view, &root);
        let mut label = None;
        for node in tree.nodes() {
            if node.role == Role::Paragraph && node.name.as_deref() == Some(b"No project yet") {
                label = Some(node);
                break;
            }
        }
        let label = label.expect("empty project label");
        assert_eq!(label.styles.as_slice(), [crate::look::PROJECT_EMPTY]);
        assert!(label.binding.is_none());
    }
}
#[test]
fn no_project_chosen_offers_no_adoption_link() {
    let view = fixtures::built();
    assert!(fixtures::find(view.tree(), Role::Link, b"Adopt a repository").is_none());
}
#[test]
fn adopt_a_repository_is_offered_whatever_the_persons_role() {
    let mut view = fixtures::signed_in_with_project();
    let node = fixtures::find(view.tree(), Role::Link, b"Adopt a repository").expect("adoption link");
    assert_eq!(
        crate::act(&mut view, node, Act::Press),
        Some(Action::Pages(pages::Act::Go {
            address: temper_client_domain::Address::Adopt { project: fixtures::PROJECT },
        }))
    );
}
#[test]
fn the_banner_says_connecting_until_me_then_live_then_offline() {
    let view = fixtures::built();
    assert!(fixtures::find(view.tree(), Role::Status, b"Connecting").is_some());
    let (mut root, watch) = fixtures::root_with_watch(None);
    let mut view = fixtures::view();
    assert!(fixtures::find(fixtures::build(&mut view, &root), Role::Status, b"Live").is_some());
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Dropped },
            },
        },
    );
    assert!(fixtures::find(fixtures::build(&mut view, &root), Role::Status, b"Offline").is_some());
}
#[test]
fn choosing_another_project_changes_the_navigations_project() {
    let mut root = fixtures::root_in_project(None);
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Act {
            action: Action::Pages(pages::Act::Project { project: fixtures::SECOND_PROJECT }),
        },
    );
    let mut view = fixtures::view();
    fixtures::build(&mut view, &root);
    assert_menu(
        &view,
        &[
            (Role::Button, Some(b"Second")),
            (Role::Button, Some(b"Shop")),
            (Role::Navigation, Some(b"Main navigation")),
            (Role::Slot, None),
        ],
    );
    let link = fixtures::find(view.tree(), Role::Link, b"Adopt a repository").expect("selected project's adoption");
    assert_eq!(
        crate::act(&mut view, link, Act::Press),
        Some(Action::Pages(pages::Act::Go {
            address: temper_client_domain::Address::Adopt { project: fixtures::SECOND_PROJECT }
        }))
    );
}
#[test]
fn a_sign_out_that_did_not_reach_the_engine_is_offered_again() {
    let mut root = fixtures::root_in_project(None);
    fixtures::root_event(&mut root, temper_client_domain::Event::Act { action: Action::Pages(pages::Act::SignOut) });
    fixtures::root_event(&mut root, temper_client_domain::Event::Client { event: jig_client::Event::SignOutLost });
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Alert, b"Sign out did not reach the engine.").is_some());
    assert!(fixtures::find(tree, Role::Button, b"Sign out").is_some());
    assert_menu(
        &view,
        &[
            (Role::Button, Some(b"Shop")),
            (Role::Button, Some(b"Second")),
            (Role::Navigation, Some(b"Main navigation")),
            (Role::Slot, None),
        ],
    );
}
#[test]
fn a_page_behind_says_behind_after_the_links_state() {
    let (mut root, watch) = fixtures::root_with_watch(None);
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Dropped },
            },
        },
    );
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert_eq!(
        fixtures::banner_statuses(tree).as_ref(),
        [Box::from(*b"Offline"), Box::from(*b"Behind"), Box::from(*b"Pat")]
    );
}

#[test]
fn signed_out_the_main_says_so_and_nothing_else() {
    let view = fixtures::page_view(&fixtures::signed_out());
    assert_eq!(&*fixtures::main_text(view.tree()), b"Signed out.");
}
#[test]
fn signed_out_the_browser_main_says_how_to_sign_in() {
    let view = fixtures::browser_page_view(&fixtures::signed_out());
    assert_eq!(
        &*fixtures::main_text(view.tree()),
        b"Signed out. To sign in, open a sign-in link temper serve printed.",
    );
}
#[test]
fn a_full_link_says_the_pool_and_both_moments_in_either_shell() {
    use jig_client_protocol_wire as w;
    let answer = w::SignInAnswer::Full(
        w::SignInsFull::new(
            &w::CEILINGS,
            w::SignInsFullParts { sign_ins: 2, earliest: 3_600_000_000_000, link_until: 300_000_000_000 },
        )
        .expect("full answer"),
    );
    assert_link_main(answer, b"All 2 sign-ins are in use; the earliest ends Jan 1, 01:00 UTC. This link stays usable until Jan 1, 00:05 UTC; after that, opening it has temper serve print a fresh one.");
}

#[test]
fn a_busy_link_says_to_open_it_again_in_either_shell() {
    assert_link_main(
        jig_client_protocol_wire::SignInAnswer::Busy,
        b"temper is busy. This link stays usable until it expires: open it again in a moment.",
    );
}

#[test]
fn a_not_ready_link_says_starting_or_stopping_in_either_shell() {
    assert_link_main(
        jig_client_protocol_wire::SignInAnswer::NotReady,
        b"temper is starting or stopping. This link stays usable until it expires: open it again in a moment.",
    );
}

fn assert_link_main(answer: jig_client_protocol_wire::SignInAnswer, expected: &[u8]) {
    use skein_lib::{Env, List, Queue, Secret, Time, Writer};
    use temper_client_domain::{Address, Event, Request};
    let limits = fixtures::domain_limits();
    let env = Env { now: Time::ZERO, wall: fixtures::NOW, limits };
    let mut root = temper_client_domain::new(&limits, fixtures::SEED);
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    temper_client_domain::step(
        &mut root,
        &env,
        Event::Start {
            address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL },
            kept: List::with_capacity(0),
            with: Some(jig_client::With::Link { secret: Secret::new(Box::new([0; 32])), held: None }),
        },
        &mut out,
    );
    let mut exchange = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request: jig_client::Request::Exchange { exchange: token, message: jig_client::Message::SignInLink(_), .. },
        } = request
        {
            assert!(exchange.replace(token).is_none());
        }
    }
    let mut writer = Writer::new(usize::try_from(answer.measure()).expect("answer size"));
    answer.encode(&mut writer).expect("answer room");
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Reply { exchange: exchange.expect("link exchange"), message: writer.finish() },
        },
    );
    assert_eq!(root.client().link().state, jig_client::Link::SignedOut);
    assert!(root.client().link().not_signed_in.is_some());
    for shell in [crate::words::Shell::Browser, crate::words::Shell::Terminal] {
        for google in [false, true] {
            let view = fixtures::sign_in_view(
                &root,
                shell,
                crate::sign_in::Ways { google, origin: Some(Box::from(b"https://temper.example.com".as_slice())) },
            );
            assert_eq!(&*fixtures::main_text(view.tree()), expected, "{shell:?}, google={google}");
        }
    }
}
#[test]
fn out_of_date_the_browser_main_keeps_its_words() {
    let view = fixtures::browser_page_view(&fixtures::out_of_date());
    assert_eq!(&*fixtures::main_text(view.tree()), b"This temper is out of date: run the temper the engine runs.");
}
#[test]
fn the_browser_words_fit_the_views_limits() {
    assert!(crate::Limits::of(&fixtures::domain_limits(), &fixtures::browser_words()).is_some());
}
#[test]
fn signed_out_the_banner_says_the_links_state_alone() {
    let view = fixtures::page_view(&fixtures::signed_out());
    assert_eq!(fixtures::banner_statuses(view.tree()).as_ref(), [Box::from(*b"Signed out")]);
}
#[test]
fn signed_out_the_banner_offers_no_sign_out_button() {
    let view = fixtures::page_view(&fixtures::signed_out());
    assert!(fixtures::find(view.tree(), Role::Button, b"Sign out").is_none());
}
#[test]
fn signed_out_the_page_offers_no_navigation() {
    let view = fixtures::page_view(&fixtures::signed_out());
    for node in view.tree().nodes() {
        assert_ne!(node.role, Role::Navigation);
    }
}
#[test]
fn an_engine_not_serving_jigs_version_says_temper_is_out_of_date() {
    let view = fixtures::page_view(&fixtures::out_of_date());
    assert_eq!(&*fixtures::main_text(view.tree()), b"This temper is out of date: run the temper the engine runs.");
}
#[test]
fn out_of_date_the_banner_says_the_links_state_alone() {
    let view = fixtures::page_view(&fixtures::out_of_date());
    assert_eq!(fixtures::banner_statuses(view.tree()).as_ref(), [Box::from(*b"Out of date")]);
}
#[test]
fn out_of_date_the_banner_offers_no_sign_out_button() {
    let view = fixtures::page_view(&fixtures::out_of_date());
    assert!(fixtures::find(view.tree(), Role::Button, b"Sign out").is_none());
}
#[test]
fn out_of_date_the_page_offers_no_navigation() {
    let view = fixtures::page_view(&fixtures::out_of_date());
    for node in view.tree().nodes() {
        assert_ne!(node.role, Role::Navigation);
    }
}
#[test]
fn each_link_shows_its_name_as_its_text() {
    let view = fixtures::signed_in_with_project();
    let mut links = 0_u32;
    let nodes = view.tree().nodes();
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Link {
            assert_eq!(node.size, 2, "a link holds one text");
            let [run] = nodes[index + 1].text.as_slice() else { panic!("one run per link") };
            assert_eq!(Some(run.text.as_ref()), node.name.as_deref());
            links += 1;
        }
    }
    assert_eq!(links, 6);
}

#[test]
fn the_navigation_is_inside_the_banner_after_projects_before_the_link() {
    let view = fixtures::signed_in_with_project();
    let nodes = view.tree().nodes();
    let mut banner = None;
    let mut nav = None;
    let mut region = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Banner {
            banner = Some(index);
        }
        if node.role == Role::Navigation {
            nav = Some(index);
        }
        if node.region == Some(crate::frame::link_region()) {
            region = Some(index);
        }
    }
    let banner = banner.expect("banner");
    let nav = nav.expect("navigation");
    let region = region.expect("link");
    assert!(nav > banner && nav < banner + usize::try_from(nodes[banner].size).expect("size"));
    assert!(nav < region);
    for (index, node) in nodes.iter().enumerate().take(banner + usize::try_from(nodes[banner].size).expect("size")) {
        if node.role == Role::Button && node.states.as_slice().contains(&skein_ui::State::Pressed) {
            assert!(index < nav, "projects precede navigation");
        }
    }
}
#[test]
fn the_navigation_marks_only_the_current_page() {
    use temper_client_domain::{Address, address::Filter};
    for (address, current) in [
        (Address::Inbox { filter: Filter::ALL }, Some(b"Inbox".as_slice())),
        (Address::Inbox { filter: Filter { goal: Some(9), ..Filter::ALL } }, Some(b"Inbox".as_slice())),
        (Address::Chats { filter: ChatsFilter::All, start: false }, Some(b"Chats".as_slice())),
        (Address::Adopt { project: fixtures::PROJECT }, Some(b"Adopt a repository".as_slice())),
        (Address::Accounts, Some(b"Accounts".as_slice())),
        (Address::Chat { task: 9 }, Some(b"Chats".as_slice())),
        (Address::Task { task: 9, filter: PlanFilter::All }, None),
    ] {
        let root = fixtures::root_at(address, &fixtures::domain_limits()).0;
        let view = fixtures::page_view(&root);
        let mut found = None;
        for node in view.tree().nodes() {
            if node.states.as_slice().contains(&skein_ui::State::Current) {
                assert_eq!(node.role, Role::Link);
                assert!(found.is_none(), "one current page");
                found = node.name.as_deref();
            }
        }
        assert_eq!(found, current);
    }
}

#[test]
fn the_signed_in_corner_groups_link_person_and_sign_out_in_order() {
    let mut root = fixtures::root_in_project(None);
    let view = fixtures::page_view(&root);
    assert_corner(&view, &[(Role::Region, None), (Role::Status, Some(b"Pat")), (Role::Button, Some(b"Sign out"))]);
    fixtures::root_event(&mut root, temper_client_domain::Event::Act { action: Action::Pages(pages::Act::SignOut) });
    fixtures::root_event(&mut root, temper_client_domain::Event::Client { event: jig_client::Event::SignOutLost });
    let view = fixtures::page_view(&root);
    assert_corner(
        &view,
        &[
            (Role::Region, None),
            (Role::Status, Some(b"Pat")),
            (Role::Alert, Some(b"Sign out did not reach the engine.")),
            (Role::Button, Some(b"Sign out")),
        ],
    );
}
#[test]
fn the_unavailable_corner_holds_only_the_link_region() {
    for root in fixtures::pair(fixtures::signed_out(), fixtures::out_of_date()) {
        let view = fixtures::page_view(&root);
        assert_corner(&view, &[(Role::Region, None)]);
    }
}

fn assert_corner(view: &crate::View, children: &[(Role, Option<&[u8]>)]) {
    let nodes = view.tree().nodes();
    let mut corner = None;
    let mut banner_end = 0;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Banner {
            banner_end = index.checked_add(usize::try_from(node.size).expect("banner size")).expect("banner end");
        }
        if node.styles.as_slice().contains(&crate::look::CORNER) {
            assert!(corner.is_none(), "one corner");
            assert_eq!(node.role, Role::Slot);
            assert!(index < banner_end, "corner is inside banner");
            corner = Some(index);
        }
    }
    let corner = corner.expect("corner");
    let end = corner.checked_add(usize::try_from(nodes[corner].size).expect("corner size")).expect("corner end");
    assert_eq!(end, banner_end, "corner finishes the banner");
    let mut index = corner.checked_add(1).expect("first child");
    for (role, name) in children {
        let node = &nodes[index];
        assert_eq!(node.role, *role);
        assert_eq!(node.name.as_deref(), *name);
        if *role == Role::Region {
            assert_eq!(node.region, Some(crate::frame::link_region()));
        }
        index = index.checked_add(usize::try_from(node.size).expect("child size")).expect("next child");
    }
    assert_eq!(index, end, "no loose corner contents");
}

#[test]
fn navigation_shows_the_inbox_count_only_when_nonzero() {
    const NEEDS: u32 = 3;
    for (count, expected) in [(0, b"Inbox".as_slice()), (NEEDS, b"Inbox 3")] {
        let (mut root, watch) = fixtures::root_with_watch(None);
        let mut me = root.pages().me(root.client()).unwrap().clone();
        me.needs_total = count;
        fixtures::root_event(
            &mut root,
            temper_client_domain::Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(
                            Box::new(me),
                        )),
                    },
                },
            },
        );
        let view = fixtures::page_view(&root);
        let id = fixtures::find(view.tree(), Role::Link, expected).expect("inbox link");
        let nodes = view.tree().nodes();
        let mut at = None;
        for (index, node) in nodes.iter().enumerate() {
            if node.id == id {
                at = Some(index);
                break;
            }
        }
        let at = at.expect("inbox link position");
        assert_eq!(nodes[at].id, id);
        let end = at + usize::try_from(nodes[at].size).unwrap();
        let mut counts = skein_lib::List::with_capacity(1);
        for node in &nodes[at + 1..end] {
            if node.styles.as_slice().contains(&skein_ui::Style { owner: crate::frame::owner(), token: 92 }) {
                counts.push(node).expect("only one count node");
            }
        }
        if count == 0 {
            assert!(counts.is_empty());
        } else {
            let [node] = counts.as_slice() else { panic!("one count node") };
            assert_eq!(node.role, Role::Text);
            assert_eq!(node.text.get(0).unwrap().text.as_ref(), b"3");
        }
    }
}

#[test]
fn the_banner_menu_holds_projects_navigation_and_corner_in_order() {
    let view = fixtures::signed_in_with_project();
    assert_menu(
        &view,
        &[
            (Role::Button, Some(b"Shop")),
            (Role::Button, Some(b"Second")),
            (Role::Navigation, Some(b"Main navigation")),
            (Role::Slot, None),
        ],
    );
}

#[test]
fn projects_me_leaves_unnamed_take_their_numbers_in_the_banner() {
    let (mut root, watch) = fixtures::root_with_watch(Some(fixtures::PROJECT));
    let mut me = root.pages().me(root.client()).expect("me").clone();
    for index in 0..me.projects.len() {
        let row = me.projects.get_mut(index).expect("project");
        row.name = jig_client::contract::DocumentText::complete(
            jig_client::contract::Text::new(
                fixtures::words()
                    .say(crate::words::Word::Project, &[skein_ui::phrases::Hole::Number(u64::from(row.project))]),
                64,
            )
            .expect("project name"),
        );
    }
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(Box::new(
                        me,
                    ))),
                },
            },
        },
    );
    let view = fixtures::page_view(&root);
    assert_menu(
        &view,
        &[
            (Role::Button, Some(b"Project 1")),
            (Role::Button, Some(b"Project 2")),
            (Role::Navigation, Some(b"Main navigation")),
            (Role::Slot, None),
        ],
    );
    for name in [b"Project 1", b"Project 2"] {
        assert!(fixtures::person_find(view.tree(), Role::Button, name).is_some(), "a person finds {name:?}");
    }
}

#[test]
fn the_banner_menu_keeps_navigation_and_corner_while_connecting() {
    assert_menu(&fixtures::built(), &[(Role::Navigation, Some(b"Main navigation")), (Role::Slot, None)]);
}

#[test]
fn the_unavailable_banner_menu_holds_its_corner_alone() {
    for root in fixtures::pair(fixtures::signed_out(), fixtures::out_of_date()) {
        assert_menu(&fixtures::page_view(&root), &[(Role::Slot, None)]);
    }
}

#[test]
fn the_offline_banner_menu_keeps_its_choices() {
    let (mut root, watch) = fixtures::root_with_watch(None);
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch, end: jig_client::contract::WatchEnd::Dropped },
            },
        },
    );
    assert_menu(
        &fixtures::page_view(&root),
        &[
            (Role::Button, Some(b"Shop")),
            (Role::Button, Some(b"Second")),
            (Role::Navigation, Some(b"Main navigation")),
            (Role::Slot, None),
        ],
    );
}

#[test]
fn the_qa_controls_keep_their_accessible_names_with_the_menu_closed() {
    for (root, controls) in [
        (
            Box::new(fixtures::chats_root()),
            &[
                (Role::Button, b"Sign out".as_slice()),
                (Role::Link, b"Chats"),
                (Role::TextArea, b"Start a chat"),
                (Role::Button, b"Start chat"),
            ][..],
        ),
        (
            Box::new(fixtures::chat::messaging_root()),
            &[
                (Role::Button, b"Sign out".as_slice()),
                (Role::Link, b"Chats"),
                (Role::TextArea, b"Message"),
                (Role::Button, b"Send"),
                (Role::Log, b"Conversation"),
            ][..],
        ),
    ] {
        let view = fixtures::page_view(&root);
        for (role, name) in controls {
            assert!(fixtures::person_find(view.tree(), *role, name).is_some(), "{role:?}: {name:?}");
        }
    }
}

fn assert_menu(view: &crate::View, children: &[(Role, Option<&[u8]>)]) {
    let nodes = view.tree().nodes();
    let mut banner = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Banner {
            assert!(banner.is_none(), "one banner");
            banner = Some(index);
        }
    }
    let banner = banner.expect("banner");
    let end = banner.checked_add(usize::try_from(nodes[banner].size).expect("banner size")).expect("banner end");
    let brand = banner.checked_add(1).expect("brand index");
    assert_eq!(nodes[brand].role, Role::Paragraph);
    assert_eq!(nodes[brand].name.as_deref(), Some(b"temper".as_slice()));
    let menu = brand.checked_add(usize::try_from(nodes[brand].size).expect("brand size")).expect("menu index");
    assert_eq!(nodes[menu].role, Role::Disclosure);
    assert_eq!(nodes[menu].name.as_deref(), Some(b"Menu".as_slice()));
    assert_eq!(nodes[menu].styles.as_slice(), [crate::look::MENU]);
    assert!(!nodes[menu].states.as_slice().contains(&skein_ui::State::Expanded));
    let menu_end = menu.checked_add(usize::try_from(nodes[menu].size).expect("menu size")).expect("menu end");
    assert_eq!(menu_end, end, "menu finishes banner");
    assert_eq!(fixtures::person_find(view.tree(), Role::Disclosure, b"Menu"), Some(nodes[menu].id));
    let mut index = menu.checked_add(1).expect("first child");
    for (role, name) in children {
        assert_eq!(nodes[index].role, *role);
        assert_eq!(nodes[index].name.as_deref(), *name);
        index = index.checked_add(usize::try_from(nodes[index].size).expect("child size")).expect("next child");
    }
    assert_eq!(index, end, "all banner contents follow the brand inside Menu");
}

// A second source lets the deciding-source test keep a later needing source.
const NAVIGATION_SOURCES: u32 = 2;
const NAVIGATION_PROVIDER: u32 = 18;
const NAVIGATION_PARTY: u64 = 73; // Pat in fixture_me.

fn navigation_accounts(states: &[jig_client::contract::OwnerAccountState]) -> jig_client::contract::OwnerAccounts {
    use jig_client::contract as c;
    let mut accounts = skein_lib::List::with_capacity(u32::try_from(states.len()).unwrap());
    for (index, state) in states.iter().enumerate() {
        let number = u32::try_from(index.checked_add(1).unwrap()).unwrap();
        let mut sources = skein_lib::List::with_capacity(NAVIGATION_SOURCES);
        sources
            .push(c::OwnerAccountSource {
                name: None,
                place: 1,
                number,
                kind: c::OwnerAccountKind::Key,
                state: *state,
                usable: *state == c::OwnerAccountState::Granted,
                expiring: false,
                attention: false,
                spent_until: None,
            })
            .unwrap();
        accounts
            .push(c::OwnerAccountLine {
                number,
                place: number,
                uses: None,
                plan: false,
                provider: Some(NAVIGATION_PROVIDER),
                provider_name: None,
                manage_usage: None,
                display: c::DocumentText::complete(c::Text::new(Box::from(b"Pat".as_slice()), 32).unwrap()),
                holder: c::OwnerAccountHolder::Party(NAVIGATION_PARTY),
                sources,
                cut: false,
            })
            .unwrap();
    }
    c::OwnerAccounts {
        program_hosts: skein_lib::List::with_capacity(0),
        readiness: skein_lib::List::with_capacity(0),
        readiness_cut: false,
        accounts,
        signing: skein_lib::List::with_capacity(0),
        last_end: None,
        usable: false,
        settled: true,
        cut: false,
    }
}

fn deliver_navigation_accounts(
    root: &mut temper_client_domain::Domain,
    watch: skein_lib::Token,
    doc: jig_client::contract::OwnerAccounts,
) {
    fixtures::root_event(
        root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::OwnerAccounts(
                        Box::new(doc),
                    )),
                },
            },
        },
    );
}

fn assert_account_mark(root: &temper_client_domain::Domain, expected: Option<(crate::accounts::words::Mark, &[u8])>) {
    let view = fixtures::page_view(root);
    let tree = view.tree();
    let link = fixtures::person_find(tree, Role::Link, b"Accounts").expect("person finds Accounts");
    let nodes = tree.nodes();
    let mut at = 0;
    for (index, node) in nodes.iter().enumerate() {
        if node.id == link {
            at = index;
        }
    }
    let end = at.checked_add(usize::try_from(nodes[at].size).unwrap()).unwrap();
    let children = &nodes[at.checked_add(1).unwrap()..end];
    assert_eq!(children[0].text.get(0).unwrap().text.as_ref(), b"Accounts");
    let mut marks = 0_u32;
    let mut found = None;
    for node in children {
        for style in node.styles.as_slice() {
            if *style == crate::look::NAV_NEEDS || *style == crate::look::NAV_LIMITED {
                marks = marks.checked_add(1).unwrap();
                found = Some(node);
            }
        }
    }
    match expected {
        None => assert_eq!(marks, 0),
        Some((mark, words)) => {
            assert_eq!(marks, 1, "one navigation dot");
            let node = found.unwrap();
            assert_eq!(fixtures::person_find(tree, Role::Text, words), Some(node.id));
            assert_eq!(node.text.get(0).unwrap().text.as_ref(), words);
            assert_eq!(
                node.styles.as_slice(),
                [match mark {
                    crate::accounts::words::Mark::Needs => crate::look::NAV_NEEDS,
                    crate::accounts::words::Mark::Limited => crate::look::NAV_LIMITED,
                }]
            );
            assert_eq!(children.len(), 2, "the state has its own text node after the name");
        }
    }
}

#[test]
fn accounts_navigation_has_no_dot_without_accounts_or_with_usable_accounts() {
    use jig_client::contract::OwnerAccountState as S;
    let (mut root, watch) = fixtures::account_root();
    assert_account_mark(&root, None);
    for states in [&[][..], &[S::Granted, S::Granted][..]] {
        deliver_navigation_accounts(&mut root, watch, navigation_accounts(states));
        assert_account_mark(&root, None);
    }
}

#[test]
fn a_moving_account_has_no_navigation_mark() {
    let (mut root, watch) = fixtures::account_root();
    deliver_navigation_accounts(
        &mut root,
        watch,
        navigation_accounts(&[jig_client::contract::OwnerAccountState::Moving]),
    );
    assert_eq!(crate::accounts::words::mark(&root), None);
    assert_account_mark(&root, None);
}

#[test]
fn accounts_navigation_is_red_for_each_state_needing_the_person() {
    use c::OwnerAccountState as S;
    use jig_client::contract as c;
    let (mut root, watch) = fixtures::account_root();
    for state in [
        S::Unloaded { at: c::OwnerAccountPlace::File, why: c::OwnerAccountUnread::Refused },
        S::SignInAgain,
        S::Misconfigured,
        S::Unloaded { at: c::OwnerAccountPlace::Directory, why: c::OwnerAccountUnread::Unreadable },
    ] {
        deliver_navigation_accounts(&mut root, watch, navigation_accounts(&[state]));
        assert_account_mark(&root, Some((crate::accounts::words::Mark::Needs, b"an account needs you")));
    }
}

#[test]
fn accounts_navigation_is_amber_for_limited_and_red_takes_precedence() {
    use crate::accounts::words::Mark;
    use jig_client::contract::OwnerAccountState as S;
    let (mut root, watch) = fixtures::account_root();
    for (states, mark, words) in [
        (&[S::Later { until: None }][..], Mark::Limited, b"an account is limited".as_slice()),
        (&[S::Later { until: None }, S::SignInAgain][..], Mark::Needs, b"an account needs you".as_slice()),
        (&[S::SignInAgain, S::Later { until: None }][..], Mark::Needs, b"an account needs you".as_slice()),
    ] {
        deliver_navigation_accounts(&mut root, watch, navigation_accounts(states));
        assert_account_mark(&root, Some((mark, words)));
    }
}

#[test]
fn accounts_navigation_uses_only_the_first_supplied_source_even_when_cut() {
    use jig_client::contract::OwnerAccountState as S;
    let (mut root, watch) = fixtures::account_root();
    let mut doc = navigation_accounts(&[S::Granted, S::SignInAgain]);
    let later = doc.accounts.get(1).unwrap().sources.get(0).unwrap().clone();
    doc.accounts.truncate(1);
    let row = doc.accounts.get_mut(0).unwrap();
    row.sources.push(later).unwrap();
    row.cut = true;
    doc.cut = true;
    deliver_navigation_accounts(&mut root, watch, doc);
    assert_account_mark(&root, None);
    let mut doc = navigation_accounts(&[S::SignInAgain]);
    doc.cut = true;
    doc.accounts.get_mut(0).unwrap().cut = true;
    deliver_navigation_accounts(&mut root, watch, doc);
    assert_account_mark(&root, Some((crate::accounts::words::Mark::Needs, b"an account needs you")));
}

#[test]
fn accounts_navigation_ignores_the_demo_provider_and_other_holders() {
    use jig_client::contract as c;
    let (mut root, _, _, _, watch) = fixtures::root_at_with_demo(
        temper_client_domain::Address::Accounts,
        &fixtures::domain_limits(),
        b"Shop",
        Some((fixtures::PROJECT, true)),
    );
    let provider = root.demo().unwrap().0.provider;
    let mut doc = navigation_accounts(&[c::OwnerAccountState::SignInAgain; 3]);
    doc.accounts.get_mut(0).unwrap().provider = Some(provider);
    doc.accounts.get_mut(1).unwrap().holder = c::OwnerAccountHolder::Deployment;
    doc.accounts.get_mut(2).unwrap().holder = c::OwnerAccountHolder::Party(fixtures::OTHER_PARTY);
    for cut in [false, true] {
        doc.cut = cut;
        deliver_navigation_accounts(&mut root, watch.unwrap(), doc.clone());
        assert_account_mark(&root, None);
    }
}

#[test]
fn accounts_navigation_reads_attention_as_needing_and_spent_usage_as_limited() {
    use crate::accounts::words::Mark;
    use jig_client::contract::OwnerAccountState as S;
    let (mut root, watch) = fixtures::account_root();
    let mut attention = navigation_accounts(&[S::Granted]);
    attention.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().attention = true;
    deliver_navigation_accounts(&mut root, watch, attention);
    assert_account_mark(&root, Some((Mark::Needs, b"an account needs you")));
    let mut spent = navigation_accounts(&[S::Granted]);
    spent.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().spent_until =
        Some(skein_lib::Wall::from_nanos(60_000_000_000));
    deliver_navigation_accounts(&mut root, watch, spent);
    assert_account_mark(&root, Some((Mark::Limited, b"an account is limited")));
}

#[test]
fn known_sign_in_ways_ignore_unknown_words_and_duplicates() {
    for (content, google) in
        [("", false), ("unknown", false), ("google", true), ("future google google", true), ("Google", false)]
    {
        assert_eq!(
            crate::sign_in::Ways::parse(content, b"https://temper.example.com"),
            crate::sign_in::Ways { google, origin: Some(Box::from(b"https://temper.example.com".as_slice())) },
            "{content}"
        );
    }
    for origin in [b"".as_slice(), b"null", b"file://", b"chrome-extension://x"] {
        assert_eq!(crate::sign_in::Ways::parse("google", origin), crate::sign_in::Ways::NONE);
    }
}
#[test]
fn browser_google_sign_in_has_the_mark_heading_and_return_link() {
    use crate::{sign_in::Ways, words::Shell};
    use temper_client_domain::{Address, Event};
    for (address, expected) in [
        // Signed out, the page stays at its address; its query is percent-encoded into `then`.
        (
            Address::Chats { filter: ChatsFilter::All, start: false },
            b"https://temper.example.com/sign-in/google?then=/%3Fproject%3D1".as_slice(),
        ),
    ] {
        let mut root = fixtures::signed_out();
        fixtures::root_event(&mut root, Event::Act { action: Action::Pages(pages::Act::Go { address }) });
        let mut view = fixtures::sign_in_view(
            &root,
            Shell::Browser,
            Ways { google: true, origin: Some(Box::from(b"https://temper.example.com".as_slice())) },
        );
        assert!(fixtures::find(view.tree(), Role::Heading { level: 1 }, b"Sign in").is_some());
        let brand = fixtures::styles(view.tree(), Role::Paragraph);
        let mut marked = false;
        for styles in &brand {
            marked |= styles.contains(&crate::look::BRAND);
        }
        assert!(marked);
        let link = fixtures::find(view.tree(), Role::Link, b"Continue with Google").unwrap();
        assert_eq!(crate::href(&mut view, link).as_deref(), Some(expected));
    }
}
#[test]
fn terminal_with_google_keeps_its_signed_out_main() {
    let view = fixtures::sign_in_view(
        &fixtures::signed_out(),
        crate::words::Shell::Terminal,
        crate::sign_in::Ways { google: true, origin: Some(Box::from(b"https://temper.example.com".as_slice())) },
    );
    assert_eq!(&*fixtures::main_text(view.tree()), b"Signed out.");
}

#[test]
fn signing_out_explains_each_forges_independent_sign_in() {
    let mut root = fixtures::signed_out();
    fixtures::root_event(&mut root, temper_client_domain::Event::Act { action: Action::Pages(pages::Act::SignOut) });
    let domain = fixtures::domain_limits();
    let limits = crate::Limits { tree: fixtures::tree_limits() };
    let mut view = crate::View::with_sign_in(
        &limits,
        &domain,
        fixtures::browser_words(),
        fixtures::FORGE,
        crate::sign_in::Ways::parse("google", b"https://temper.example"),
    );
    let mut facts = crate::forge_sign_in::Forges::new();
    facts.read("1 forgejo https://forge.example other");
    facts.read("2 owned http://127.0.0.1:3000 owned");
    view.forge_sign_in(facts);
    fixtures::build(&mut view, &root);
    let text = fixtures::main_text(view.tree());
    assert!(
        skein_lib::bytes::find(
            &text,
            b"You signed out. Forgejo at forge.example keeps its own sign-in: sign out there too."
        )
        .is_some()
    );
    assert!(
        skein_lib::bytes::find(
            &text,
            b"You signed out. Forgejo on this machine keeps its own sign-in: sign out there too."
        )
        .is_some()
    );
}

#[test]
fn retired_navigation_omits_the_playground_project_after_reload() {
    use jig_client::contract as c;
    use temper_client_domain::{Address, Event};
    use temper_client_protocol_wire::v1 as w;
    for another in [false, true] {
        let (mut root, watch, _, _, _, _) = fixtures::root_at_with_forges(
            Address::Chats { filter: ChatsFilter::All, start: false },
            &fixtures::domain_limits(),
            b"Shop",
            Some((fixtures::PROJECT, true, w::DemoState::Ended)),
            None,
            None,
        );
        let mut me = root.pages().me(root.client()).unwrap().clone();
        let first = me.projects.get(0).unwrap().clone();
        let mut other = first.clone();
        other.project = fixtures::SECOND_PROJECT;
        other.name = c::DocumentText::complete(c::Text::new(b"Other".as_slice().into(), 64).unwrap());
        me.projects = skein_lib::List::with_capacity(2);
        me.projects.push(first).unwrap();
        if another {
            me.projects.push(other).unwrap();
        }
        fixtures::root_event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                    },
                },
            },
        );
        for words in [fixtures::words(), fixtures::browser_words()] {
            let mut view = fixtures::view_of(words);
            let tree = fixtures::build(&mut view, &root);
            assert!(fixtures::find(tree, Role::Button, b"Playground").is_none());
            assert!(fixtures::find(tree, Role::Button, b"Shop").is_none());
            assert_eq!(fixtures::find(tree, Role::Button, b"Other").is_some(), another);
            assert_eq!(fixtures::find(tree, Role::Paragraph, b"No project yet").is_some(), !another);
        }
    }
}
