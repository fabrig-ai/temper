use crate::{jig::call::Parsed, tests::fixtures as f};
use jig_client::contract as c;
use skein_lib::{List, Secret};
const KEY: &[u8] = b"one-use-private-key";
const ADD: &[&str] = &["account", "add-key", "9", "key-file", "--deployment", "--key", "add"];
const FORMS: &[&[&str]] = &[
    &["account", "export", "43", "file"],
    &["account", "import", "file", "--key", "move-in"],
    &["account", "connect", "9", "--key", "connect"],
    &["account", "connect", "0", "--key", "zero-provider"],
    &["account", "add-key", "0", "file", "--key", "zero-provider"],
    &["account", "disconnect", "43", "--source", "sign_in"],
    ADD,
    &["account", "add-key", "9", "file", "--account", "43", "--key", "join"],
    &["account", "replace-key", "43", "file"],
    &["account", "forget", "0"],
    &["account", "rank", "0", "1"],
    &["account", "add-key", "9", "file", "--account", "0", "--key", "k"],
    &["account", "forget", "43"],
    &["account", "forget", "43", "--source", "key"],
    &["account", "rank", "43", "2", "--source", "program"],
    &["account", "retry", "43"],
    &["account", "declare", "9", "helper", "work-login", "--key", "declare"],
    &["account", "declare", "9", "helper", "work-login", "--account", "43", "--key", "join"],
    &["account", "verify", "43"],
    &["account", "verify", "43", "--source", "program", "--key", "check"],
];
#[test]
fn account_forms_round_trip_through_the_table() {
    for words in FORMS {
        let call = f::call(words);
        assert_eq!(f::back(&call), f::owned(words));
        assert_eq!(f::printed(&call), Parsed::Call { call, json: false });
    }
}
#[test]
fn account_usage_is_refused_before_sending() {
    for words in [
        &["account", "export", "43", "file", "--key", "forbidden"][..],
        &["account", "export", "4294967296", "file"],
        &["account", "export", "43", "-"],
        &["account", "import", "file"],
        &["account", "import", "-", "--key", "move-in"],
        &["account", "connect", "9"],
        &["account", "add-key", "9", "file"],
        &["account", "add-key", "9", "-", "--key", "add"],
        &["account", "rank", "43", "0"],
        &["account", "retry", "4294967296"],
    ] {
        assert!(f::parse(words).is_err(), "{words:?}");
    }
}
#[test]
fn account_acts_preserve_the_resolved_source() {
    for (words, change) in [
        (&["account", "forget", "43"][..], c::AccountChange::Remove { account: f::ACCOUNT }),
        (&["account", "forget", "43", "--source", "key"], c::AccountChange::RemoveKey { source: f::SOURCE }),
        (&["account", "rank", "43", "2"], c::AccountChange::MoveAccount { account: f::ACCOUNT, place: 1 }),
        (
            &["account", "rank", "43", "2", "--source", "key"],
            c::AccountChange::MoveSource { account: f::ACCOUNT, source: f::SOURCE, place: 1 },
        ),
        (&["account", "retry", "43"], c::AccountChange::Resume { source: f::SOURCE }),
    ] {
        for owner in [false, true] {
            let mut root = f::account_root(owner);
            let call = root.submit(words);
            let mut accounts = f::accounts();
            if owner {
                accounts.accounts = List::with_capacity(0);
            }
            f::deliver_accounts(&mut root, accounts);
            if owner {
                f::deliver_deployment(&mut root, f::deployment());
            }
            let (_, _, ask) = root.sends.get(&call).expect("resolved act");
            let holder = if owner { c::AccountHolder::Deployment } else { c::AccountHolder::Own };
            assert_eq!(ask, &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange { holder, change: change.clone() })));
        }
    }
}
#[test]
fn ambiguous_sources_are_refused_before_sending() {
    for cut in [false, true] {
        let mut root = f::account_root(false);
        let call = root.submit(&["account", "retry", "43", "--source", "key"]);
        let mut accounts = f::accounts();
        let row = accounts.accounts.get_mut(0).expect("row");
        let source = row.sources.get(0).expect("source").clone();
        row.sources = List::with_capacity(2);
        row.sources.push(source.clone()).expect("first");
        row.sources.push(source).expect("second");
        row.cut = cut;
        f::deliver_accounts(&mut root, accounts);
        assert_eq!(root.answer(call).expect("local refusal").status, 1);
        assert!(root.sends.is_empty());
    }
}
#[test]
fn a_key_value_never_enters_the_request_store() {
    for (words, holder, join) in [
        (ADD, c::AccountHolder::Deployment, None),
        (&["account", "add-key", "9", "file", "--key", "add"][..], c::AccountHolder::Own, None),
        (
            &["account", "add-key", "9", "file", "--account", "43", "--key", "add"],
            c::AccountHolder::Own,
            Some(f::ACCOUNT),
        ),
    ] {
        let mut root = f::account_root(false);
        let call = root.submit_value(words, Some(Secret::new(KEY.into())));
        assert_eq!(root.carried.get(&call).expect("transient value").expose(), KEY);
        let (_, name, ask) = root.sends.get(&call).expect("sent once");
        assert_eq!(*name, f::named("add"));
        assert_eq!(
            ask,
            &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                holder,
                change: c::AccountChange::AddKey {
                    provider: f::PROVIDER,
                    join,
                    digest: skein_lib::sha256::digest(KEY)
                },
            }))
        );
        assert!(root.saved.is_empty());
        root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountKeyKept { source: f::SOURCE }));
        assert_eq!(root.answer(call).expect("durable answer").status, 0);
    }
}
#[test]
fn connect_prints_the_watch_that_follows_its_address() {
    let mut root = f::account_root(false);
    let call = root.submit(&["account", "connect", "0", "--key", "connect"]);
    assert_eq!(
        root.sends.get(&call).expect("configured provider zero").2,
        jig_client::Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 0 }))
    );
    root.done(
        call,
        c::Outcome::Owner(c::OwnerOutcome::AccountSignInStarted {
            source: f::SOURCE,
            address: Box::from(b"http://engine/visit".as_slice()),
        }),
    );
    let printed = root.answer(call).expect("durable start");
    assert_eq!(printed.status, 0);
    assert!(skein_lib::bytes::find(&printed.stdout, b"wait accounts --until settled").is_some());
}
#[test]
fn unconfirmed_disconnect_names_the_provider_settings() {
    let mut root = f::account_root(false);
    let call = root.submit(&["account", "disconnect", "43"]);
    let mut me = f::me(f::PARTY);
    me.providers = f::provider_offers();
    root.deliver(c::Delivery::Me(Box::new(me)));
    let mut accounts = f::accounts();
    accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind =
        c::OwnerAccountKind::SignIn;
    f::deliver_accounts(&mut root, accounts);
    assert_eq!(
        root.sends.get(&call).expect("disconnect").2,
        jig_client::Ask::Jig(Box::new(c::Ask::SignOut { holder: c::AccountHolder::Own, source: f::SOURCE }))
    );
    root.done(
        call,
        c::Outcome::Owner(c::OwnerOutcome::AccountSignedOut { confirmed: false, failed_hosts: Box::new([]) }),
    );
    let printed = root.answer(call).expect("removed locally");
    assert_eq!(printed.status, 0);
    assert!(
        skein_lib::bytes::find(&printed.stdout, b"Disconnect temper in Scripted's settings, under Apps.").is_some()
    );
}
#[test]
fn a_value_request_is_not_retried_automatically() {
    for envelope in [skein_lib::RequestEnvelope::Again, skein_lib::RequestEnvelope::Lost] {
        let mut root = f::account_root(false);
        let call = root.submit_value(ADD, Some(Secret::new(KEY.into())));
        let (attempt, name, ask) = root.sends.get(&call).expect("one attempt").clone();
        root.client(jig_client::Event::Up {
            up: jig_client::Up::Answer { attempt, envelope, answer: None, why: Some(jig_client::Retry::Busy) },
        });
        let printed = root.answer(call).expect("caller must resupply");
        assert_eq!(printed.status, 1);
        assert!(root.saved.is_empty());
        root.sends.clear();
        root.now = root.now.saturating_add(skein_lib::Duration::from_secs(60));
        root.fire();
        assert!(root.sends.is_empty());
        let retry = root.submit_value(ADD, Some(Secret::new(KEY.into())));
        let (_, next_name, next_ask) = root.sends.get(&retry).expect("explicit re-entry");
        assert_eq!((*next_name, next_ask), (name, &ask));
    }
}
#[test]
fn an_engine_key_refusal_exits_two() {
    let mut root = f::account_root(false);
    let call = root.submit_value(ADD, Some(Secret::new(KEY.into())));
    root.answered(call, c::Answer::Refused { refusal: c::Refusal::KeyConflict, at: None });
    let printed = root.answer(call).expect("engine refusal");
    assert_eq!(printed.status, 2);
    assert!(skein_lib::bytes::find(&printed.stderr, KEY).is_none());
}
#[test]
fn replacement_draws_an_ephemeral_name() {
    let mut root = f::account_root(false);
    let call = root.submit_value(&["account", "replace-key", "43", "file"], Some(Secret::new(KEY.into())));
    f::deliver_accounts(&mut root, f::accounts());
    let first = root.sent_name(call).expect("drawn name");
    root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountKeyKept { source: f::SOURCE }));
    let next = root.submit_value(&["account", "replace-key", "43", "file"], Some(Secret::new(KEY.into())));
    f::deliver_accounts(&mut root, f::accounts());
    assert_ne!(first, root.sent_name(next).expect("second drawn name"));
    assert!(root.saved.is_empty());
    assert_eq!(
        root.sends.get(&call).expect("replacement").2,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::ReplaceKey { source: f::SOURCE, digest: skein_lib::sha256::digest(KEY) },
        }))
    );
}

#[test]
fn out_of_bound_secrets_are_refused_at_call_admission() {
    for value in [Box::from([]), Box::from([0; 4097])] {
        let mut root = f::account_root(false);
        let call = root.submit_value(ADD, Some(Secret::new(value)));
        assert_eq!(root.answer(call).expect("client bound").status, 1);
        assert!(root.sends.is_empty());
    }
}

#[test]
fn source_shorthand_resolves_the_supplied_kind() {
    for (word, kind) in [("sign_in", c::OwnerAccountKind::SignIn), ("program", c::OwnerAccountKind::Program)] {
        for tool in ["disconnect", "forget"] {
            let mut root = f::account_root(false);
            let call = root.submit(&["account", tool, "43", "--source", word]);
            let mut accounts = f::accounts();
            accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind = kind;
            f::deliver_accounts(&mut root, accounts);
            assert_eq!(
                root.sends.get(&call).expect("resolved sign-out").2,
                jig_client::Ask::Jig(Box::new(c::Ask::SignOut { holder: c::AccountHolder::Own, source: f::SOURCE }))
            );
        }
    }
}

#[test]
fn a_named_replacement_uses_the_supplied_name() {
    let mut root = f::account_root(false);
    let call = root
        .submit_value(&["account", "replace-key", "43", "file", "--key", "replacement"], Some(Secret::new(KEY.into())));
    f::deliver_accounts(&mut root, f::accounts());
    assert_eq!(root.sent_name(call), Some(f::named("replacement")));
}
#[test]
fn account_zero_reaches_its_holder() {
    let mut root = f::account_root(false);
    let call = root.submit(&["account", "forget", "0"]);
    let mut accounts = f::accounts();
    accounts.accounts.get_mut(0).expect("account").number = 0;
    f::deliver_accounts(&mut root, accounts);
    assert!(root.sends.contains_key(&call));
}
#[test]
fn a_lost_replacement_prints_its_recoverable_name() {
    for envelope in [skein_lib::RequestEnvelope::Again, skein_lib::RequestEnvelope::Lost] {
        let mut root = f::account_root(false);
        let call = root.submit_value(&["account", "replace-key", "43", "file"], Some(Secret::new(KEY.into())));
        f::deliver_accounts(&mut root, f::accounts());
        let (attempt, name, _) = root.sends.get(&call).expect("first replacement").clone();
        root.client(jig_client::Event::Up {
            up: jig_client::Up::Answer { attempt, envelope, answer: None, why: Some(jig_client::Retry::Busy) },
        });
        let printed = root.answer(call).expect("recovery instruction");
        assert_eq!(printed.status, 1);
        let start = skein_lib::bytes::find(&printed.stderr, b"supply --key ").expect("printed name");
        let tail = printed.stderr.get(start.checked_add(13).expect("prefix fits")..).expect("name follows");
        let end = skein_lib::bytes::find(tail, b" ").expect("name end");
        let drawn = crate::jig::key::Name::new(tail.get(..end).expect("printed name")).expect("recoverable name");
        assert!(skein_lib::bytes::find(tail, b" and the same file again").is_some());
        assert_eq!(crate::jig::key::named(crate::key::TOOLS, &drawn), name);
    }
}

#[test]
fn declare_validates_the_offer_and_sends_only_the_contract_fields() {
    for join in [false, true] {
        let mut root = f::account_root(false);
        let mut me = f::me(f::PARTY);
        me.providers = f::provider_offers();
        root.deliver(c::Delivery::Me(Box::new(me)));
        let words = if join {
            &["account", "declare", "9", "helper", "work-login", "--account", "43", "--key", "declaration"][..]
        } else {
            &["account", "declare", "9", "helper", "work-login", "--key", "declaration"][..]
        };
        let call = root.submit(words);
        let (_, name, ask) = root.sends.get(&call).expect("declaration");
        assert_eq!(*name, f::named("declaration"));
        assert_eq!(
            ask,
            &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                holder: c::AccountHolder::Own,
                change: c::AccountChange::DeclareProgram {
                    provider: f::PROVIDER,
                    join: if join { Some(f::ACCOUNT) } else { None },
                    name: b"work-login".as_slice().into(),
                },
            }))
        );
        root.done(
            call,
            c::Outcome::Owner(c::OwnerOutcome::AccountProgramDeclared {
                source: f::SOURCE,
                name: b"work-login".as_slice().into(),
            }),
        );
        let printed = root.answer(call).expect("declared");
        assert_eq!(printed.status, 0);
        assert!(f::has(&printed, b"Run on each host: temper program-login work-login"));
    }
}
#[test]
fn declare_refuses_unoffered_adapters_and_providers_locally() {
    for words in [
        &["account", "declare", "9", "absent", "work-login", "--key", "declaration"][..],
        &["account", "declare", "99", "helper", "work-login", "--key", "declaration"],
    ] {
        let mut root = f::account_root(false);
        let mut me = f::me(f::PARTY);
        me.providers = f::provider_offers();
        root.deliver(c::Delivery::Me(Box::new(me)));
        let call = root.submit(words);
        let printed = root.answer(call).expect("local refusal");
        assert_eq!(printed.status, 1);
        assert!(skein_lib::bytes::find(&printed.stderr, b"adapter not offered").is_some());
        assert!(root.sends.is_empty());
    }
}
#[test]
fn declare_refuses_unchecked_adapters_when_provider_offers_are_cut() {
    for (providers_cut, programs_cut, name_cut) in [(true, false, false), (false, true, false), (false, false, true)] {
        let mut root = f::account_root(false);
        let mut me = f::me(f::PARTY);
        me.providers = f::provider_offers();
        me.providers_cut = providers_cut;
        let offer = me.providers.get_mut(0).expect("offer");
        offer.programs_cut = programs_cut;
        offer.name.cut = name_cut;
        if providers_cut {
            me.providers = List::with_capacity(0);
        }
        root.deliver(c::Delivery::Me(Box::new(me)));
        let call = root.submit(&["account", "declare", "9", "absent", "work-login", "--key", "declaration"]);
        let printed = root.answer(call).expect("local refusal");
        assert_eq!(printed.status, 1);
        assert!(skein_lib::bytes::find(&printed.stderr, b"provider offers cut in me; adapter unchecked").is_some());
        assert!(skein_lib::bytes::find(&printed.stderr, b"adapter not offered").is_none());
        assert!(root.sends.is_empty());
    }
}
#[test]
fn declare_checks_the_portable_name_bound_in_argv_and_json() {
    for name in ["", "UPPER", "with_space ", "../escape", "abcdefghijklmnopqrstuvwxyz1234567"] {
        f::parse(&["account", "declare", "9", "helper", name, "--key", "k"]).expect_err("invalid portable name");
    }
    for name in ["a", "-", "abcdefghijklmnopqrstuvwxyz123456"] {
        let call = f::call(&["account", "declare", "9", "helper", name, "--key", "k"]);
        assert_eq!(f::printed(&call), Parsed::Call { call, json: false });
    }
    f::parse(&["account", "declare", "9", "helper", "work-login"]).expect_err("declaration needs a key");
    f::parse(&["account", "declare", "4294967296", "helper", "work-login", "--key", "k"])
        .expect_err("provider fits u32");
}
#[test]
fn verify_asks_the_supplied_program_source_then_names_the_wait() {
    for owner in [false, true] {
        for json in [false, true] {
            let mut root = f::account_root(owner);
            let words = if json {
                &["account", "verify", "43", "--source", "program", "--json"][..]
            } else {
                &["account", "verify", "43"][..]
            };
            let call = root.submit(words);
            let mut accounts = f::accounts();
            accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind =
                c::OwnerAccountKind::Program;
            if owner {
                accounts.accounts = List::with_capacity(0);
            }
            f::deliver_accounts(&mut root, accounts);
            if owner {
                let mut deployment = f::deployment();
                deployment.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind =
                    c::OwnerAccountKind::Program;
                f::deliver_deployment(&mut root, deployment);
            }
            assert_eq!(
                root.sends.get(&call).expect("check request").2,
                jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                    holder: if owner { c::AccountHolder::Deployment } else { c::AccountHolder::Own },
                    change: c::AccountChange::CheckProgram { source: f::SOURCE },
                }))
            );
            assert!(root.answer(call).is_none(), "wait for durable answer");
            root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountChanged));
            let printed = root.answer(call).expect("asked");
            assert_eq!(printed.status, 0);
            assert!(f::has(&printed, b"wait accounts --until settled"));
        }
    }
}
#[test]
fn verify_refuses_sources_without_a_program_login_locally() {
    for kind in [c::OwnerAccountKind::Key, c::OwnerAccountKind::SignIn, c::OwnerAccountKind::None] {
        let mut root = f::account_root(false);
        let call = root.submit(&["account", "verify", "43"]);
        let mut accounts = f::accounts();
        accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind = kind;
        f::deliver_accounts(&mut root, accounts);
        let printed = root.answer(call).expect("local refusal");
        assert_eq!(printed.status, 1);
        assert!(skein_lib::bytes::find(&printed.stderr, b"verify needs a program source").is_some());
        assert!(root.sends.is_empty());
    }
}

#[test]
fn forget_with_cut_sources_keeps_the_source_kinds_unknown() {
    for kind in [c::OwnerAccountKind::Key, c::OwnerAccountKind::Program, c::OwnerAccountKind::SignIn] {
        for failed in [false, true] {
            let mut root = f::account_root(false);
            let call = root.submit(&["account", "forget", "43"]);
            let mut accounts = f::accounts();
            let row = accounts.accounts.get_mut(0).expect("account");
            row.sources.get_mut(0).expect("source").kind = kind;
            row.cut = true;
            f::deliver_accounts(&mut root, accounts);
            assert_eq!(
                root.sends.get(&call).expect("remove request").2,
                jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                    holder: c::AccountHolder::Own,
                    change: c::AccountChange::Remove { account: f::ACCOUNT },
                }))
            );
            let failed_hosts = if failed {
                Box::<[c::AccountHostFailure]>::from([c::AccountHostFailure {
                    name: b"laptop".as_slice().into(),
                    why: c::AccountHostWhy::NotInContact,
                }])
            } else {
                Box::default()
            };
            root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountRemoved { confirmed: false, failed_hosts }));
            let printed = root.answer(call).expect("removal");
            assert_eq!(printed.status, 0);
            if failed {
                assert!(f::has(&printed, b"Not signed out on:\nlaptop: not in contact"));
                assert!(!f::has(&printed, b"under Apps"), "a visible sign-in alone cannot establish the omitted kinds");
            } else {
                assert!(f::has(&printed, b"Disconnect temper in the provider's settings, under Apps."));
                assert!(!f::has(&printed, b"Signed out on every machine."));
            }
            assert!(!f::has(&printed, b"done: forget"));
        }
    }
}

#[test]
fn restored_unconfirmed_sign_out_and_removal_print_the_provider_settings_advice() {
    for remove in [false, true] {
        for json in [false, true] {
            let mut records = f::restored_message(f::PARTY);
            let ask = if remove {
                c::Ask::AccountChange {
                    holder: c::AccountHolder::Own,
                    change: c::AccountChange::Remove { account: f::ACCOUNT },
                }
            } else {
                c::Ask::SignOut { holder: c::AccountHolder::Own, source: f::SOURCE }
            };
            records.get_mut(f::EMPTY).expect("restored request").request.request = jig_client::Ask::Jig(Box::new(ask));
            let mut root = f::started_with(records, f::SEED, json);
            root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
            let done = if remove {
                c::OwnerOutcome::AccountRemoved { confirmed: false, failed_hosts: Box::default() }
            } else {
                c::OwnerOutcome::AccountSignedOut { confirmed: false, failed_hosts: Box::default() }
            };
            root.done(f::EMPTY, c::Outcome::Owner(done));
            let line = root.kept_lines.get(0).expect("restored answer");
            assert!(
                skein_lib::bytes::find(line, b"Disconnect temper in the provider's settings, under Apps.").is_some()
            );
            assert!(skein_lib::bytes::find(line, b"read accounts").is_none());
        }
    }
}

#[test]
fn mixed_forget_reports_program_hosts_independently_of_sign_in_revocation() {
    for failed in [false, true] {
        let mut root = f::account_root(false);
        let call = root.submit(&["account", "forget", "43"]);
        let mut accounts = f::accounts();
        let row = accounts.accounts.get_mut(0).expect("account");
        let mut source = row.sources.get(0).expect("source").clone();
        row.sources = List::with_capacity(2);
        source.kind = c::OwnerAccountKind::Program;
        row.sources.push(source.clone()).expect("program");
        source.number = f::SOURCE.checked_add(1).expect("other source");
        source.kind = c::OwnerAccountKind::SignIn;
        row.sources.push(source).expect("sign-in");
        f::deliver_accounts(&mut root, accounts);
        let failed_hosts = if failed {
            Box::<[c::AccountHostFailure]>::from([c::AccountHostFailure {
                name: b"laptop".as_slice().into(),
                why: c::AccountHostWhy::NotInContact,
            }])
        } else {
            Box::default()
        };
        root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountRemoved { confirmed: false, failed_hosts }));
        let printed = root.answer(call).expect("mixed removal");
        assert_eq!(printed.status, 0);
        if failed {
            assert!(f::has(&printed, b"Not signed out on:\nlaptop: not in contact"));
        } else {
            assert!(f::has(&printed, b"Signed out on every machine."));
        }
        assert!(f::has(&printed, b"Disconnect temper in Scripted's settings, under Apps."));
        assert!(!f::has(&printed, b"Sign-out not confirmed on every machine."));
    }
}

#[test]
fn confirmed_key_and_sign_in_results_keep_done_in_text_only() {
    for kind in [c::OwnerAccountKind::Key, c::OwnerAccountKind::SignIn] {
        for remove in [false, true] {
            for json in [false, true] {
                let mut root = f::account_root(false);
                let word = if remove { "forget" } else { "disconnect" };
                let call = if json {
                    root.submit(&["account", word, "43", "--json"])
                } else {
                    root.submit(&["account", word, "43"])
                };
                let mut accounts = f::accounts();
                accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").kind = kind;
                f::deliver_accounts(&mut root, accounts);
                let done = if remove {
                    c::OwnerOutcome::AccountRemoved { confirmed: true, failed_hosts: Box::default() }
                } else {
                    c::OwnerOutcome::AccountSignedOut { confirmed: true, failed_hosts: Box::default() }
                };
                root.done(call, c::Outcome::Owner(done));
                let printed = root.answer(call).expect("confirmed result");
                assert_eq!(printed.status, 0);
                if json {
                    let expected = if remove {
                        b"{\"operation\":\"forget\",\"outcome\":{\"done\":{\"owner\":{\"account_removed\":{\"confirmed\":true,\"failed_hosts\":[]}}}},\"task\":null,\"to\":null}".as_slice()
                    } else {
                        b"{\"operation\":\"disconnect\",\"outcome\":{\"done\":{\"owner\":{\"account_signed_out\":{\"confirmed\":true,\"failed_hosts\":[]}}}},\"task\":null,\"to\":null}".as_slice()
                    };
                    assert_eq!(printed.stdout.as_ref(), expected);
                } else {
                    assert!(f::has(&printed, if remove { b"done: forget" } else { b"done: disconnect" }));
                }
            }
        }
    }
}

#[test]
fn program_disconnect_and_forget_report_confirmation_or_each_failed_host() {
    for words in [
        &["account", "disconnect", "43"][..],
        &["account", "forget", "43", "--source", "program"],
        &["account", "forget", "43"],
    ] {
        for confirmed in [false, true] {
            for json in [false, true] {
                let mut root = f::account_root(false);
                let mut argv = List::with_capacity(8);
                for word in words {
                    argv.push(*word).expect("argv room");
                }
                if json {
                    argv.push("--json").expect("json flag room");
                }
                let call = root.submit(argv.as_slice());
                let mut accounts = f::accounts();
                let source = accounts.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source");
                source.kind = c::OwnerAccountKind::Program;
                source.name = Some(b"work-login".as_slice().into());
                f::deliver_accounts(&mut root, accounts);
                let failed_hosts = if confirmed {
                    Box::<[c::AccountHostFailure]>::default()
                } else {
                    Box::new([
                        c::AccountHostFailure {
                            name: b"laptop".as_slice().into(),
                            why: c::AccountHostWhy::NotInContact,
                        },
                        c::AccountHostFailure { name: b"desktop".as_slice().into(), why: c::AccountHostWhy::Lost },
                        c::AccountHostFailure { name: b"builder".as_slice().into(), why: c::AccountHostWhy::Failed },
                    ])
                };
                let done = if words.len() == 3 && words[1] == "forget" {
                    c::OwnerOutcome::AccountRemoved { confirmed, failed_hosts }
                } else {
                    c::OwnerOutcome::AccountSignedOut { confirmed, failed_hosts }
                };
                root.done(call, c::Outcome::Owner(done));
                let printed = root.answer(call).expect("sign-out result");
                assert_eq!(printed.status, 0);
                if confirmed {
                    if json {
                        assert!(!f::has(&printed, b"\"next\":"));
                        assert!(!f::has(&printed, b"done: "));
                    } else {
                        assert!(f::has(&printed, b"Signed out on every machine."));
                    }
                } else {
                    if json {
                        assert!(f::has(&printed, b"\"next\":"));
                    }
                    for fact in [b"laptop: not in contact".as_slice(), b"desktop: contact lost", b"builder: failed"] {
                        assert!(f::has(&printed, fact), "{printed:?}");
                    }
                }
                assert!(!f::has(&printed, b"under Apps"));
            }
        }
    }
}

#[test]
fn exports_resolve_the_holder_without_selecting_a_source() {
    for owner in [false, true] {
        let mut root = f::account_root(owner);
        let call = root.submit(&["account", "export", "43", "record"]);
        let mut accounts = f::accounts();
        // Source state and completeness are the engine's policy, not ctl's.
        accounts.accounts.get_mut(0).expect("account").sources = List::with_capacity(0);
        accounts.accounts.get_mut(0).expect("account").cut = true;
        if owner {
            accounts.accounts = List::with_capacity(0);
        }
        f::deliver_accounts(&mut root, accounts);
        if owner {
            f::deliver_deployment(&mut root, f::deployment());
        }
        let (_, _, ask) = root.sends.get(&call).expect("move out sent");
        let holder = if owner { c::AccountHolder::Deployment } else { c::AccountHolder::Own };
        assert_eq!(
            ask,
            &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                holder,
                change: c::AccountChange::MoveOut { account: f::ACCOUNT }
            }))
        );
        assert!(root.saved.is_empty());
        assert!(root.carried.is_empty());
    }
}
#[test]
fn imports_carry_the_whole_record_once_as_own_at_the_bound() {
    const RECORD: &[u8] = &[b'r'; 131_072]; // client/tools.md, 12.
    let mut root = f::account_root(false);
    let call =
        root.submit_value(&["account", "import", "record", "--key", "move-in"], Some(Secret::new(RECORD.into())));
    let (_, _, ask) = root.sends.get(&call).expect("move in sent");
    assert_eq!(
        ask,
        &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::MoveIn { digest: skein_lib::sha256::digest(RECORD) },
        }))
    );
    assert_eq!(root.carried.get(&call).expect("carried record").expose(), RECORD);
    assert!(root.saved.is_empty());
}
#[test]
fn imports_refuse_empty_or_oversized_values_before_sending() {
    const TOO_LARGE: &[u8] = &[b'r'; 131_073]; // One beyond client/tools.md, 12.
    for bytes in [b"".as_slice(), TOO_LARGE] {
        let mut root = f::account_root(false);
        let call =
            root.submit_value(&["account", "import", "record", "--key", "move-in"], Some(Secret::new(bytes.into())));
        let printed = root.answer(call).expect("bound failure");
        assert_eq!(printed.status, 1);
        assert_eq!(printed.file, crate::jig::FileEnd::Keep);
        assert!(root.sends.is_empty());
    }
}

#[test]
fn exports_wait_for_the_record_acknowledgment_and_render_public_fields() {
    const RECORD: &[u8] = b"opaque-move-record";
    for json in [false, true] {
        let mut root = f::account_root(false);
        let call = root.submit(if json {
            &["account", "export", "43", "record", "--json"]
        } else {
            &["account", "export", "43", "record"]
        });
        f::deliver_accounts(&mut root, f::accounts());
        let name = root.sent_name(call).expect("move name");
        root.done(
            call,
            c::Outcome::Made {
                what: c::Made::MovingOut { account: f::ACCOUNT, number: 83, record: Secret::new(RECORD.into()) },
            },
        );
        assert_eq!(root.records.remove(&call).expect("shell record").expose(), RECORD);
        assert!(root.answer(call).is_none());
        assert!(root.saved.is_empty());
        let mut out = skein_lib::Queue::with_capacity(f::OUTPUTS);
        let env = skein_lib::Env {
            now: root.now,
            wall: root.wall,
            limits: crate::root::Limits::new(root.limits.domain, root.limits.tools),
        };
        root.root.recorded(&env, call, Ok(()), &mut out);
        root.outputs(out);
        assert_ne!(root.sent_name(call).expect("written name"), name);
        let (_, _, ask) = root.sends.get(&call).expect("written");
        assert_eq!(
            ask,
            &jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                holder: c::AccountHolder::Own,
                change: c::AccountChange::Written { number: 83 }
            }))
        );
        root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountChanged));
        let printed = root.answer(call).expect("exported");
        assert_eq!((printed.status, printed.file), (0, crate::jig::FileEnd::Keep));
        if json {
            assert!(
                skein_lib::bytes::find(&printed.stdout, br#""outcome":{"account":43,"number":83,"file":"record"}"#)
                    .is_some()
            );
        } else {
            assert_eq!(printed.stdout.as_ref(), b"moved out: account 43 to record\n");
        }
        assert!(skein_lib::bytes::find(&printed.stdout, RECORD).is_none());
        assert!(printed.stderr.is_empty());
    }
}
#[test]
fn imports_render_the_account_and_renewal_from_the_answer() {
    for json in [false, true] {
        for (renewed, text) in
            [(false, b"moved in: account 43\n".as_slice()), (true, b"moved in: account 43, renewed\n")]
        {
            let mut root = f::account_root(false);
            let words = if json {
                &["account", "import", "record", "--key", "move-in", "--json"][..]
            } else {
                &["account", "import", "record", "--key", "move-in"]
            };
            let call = root.submit_value(words, Some(Secret::new(KEY.into())));
            root.done(call, c::Outcome::Owner(c::OwnerOutcome::AccountMovedIn { account: f::ACCOUNT, renewed }));
            let printed = root.answer(call).expect("imported");
            assert_eq!((printed.status, printed.file), (0, crate::jig::FileEnd::Remove));
            if json {
                assert!(
                    skein_lib::bytes::find(&printed.stdout, br#""account_moved_in":{"account":43,"renewed":"#)
                        .is_some()
                );
            } else {
                assert_eq!(printed.stdout.as_ref(), text);
            }
            assert!(skein_lib::bytes::find(&printed.stdout, KEY).is_none());
            assert!(printed.stderr.is_empty());
        }
    }
}
