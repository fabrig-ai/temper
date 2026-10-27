use crate::jig::digest::Digest;
use crate::tests::fixtures as f;
use jig_client::contract as c;
use skein_lib::List;

#[test]
fn account_reads_close_authorized_watches_after_the_first_projection() {
    for owner in [false, true] {
        let mut root = f::account_root(owner);
        let call = root.submit(&["account", "list", "--json"]);
        assert!(root.reads.is_empty(), "accounts has no ReadValue");
        let mut deployment = false;
        for watched in root.watches.values() {
            deployment |= *watched == f::DEPLOYMENT_WATCH;
        }
        assert_eq!(deployment, owner);
        f::deliver_accounts(&mut root, f::accounts());
        if owner {
            assert!(root.answer(call).is_none(), "owner waits for deployment too");
            f::deliver_deployment(&mut root, f::deployment());
        }
        let printed = root.answer(call).expect("first deliveries");
        assert_eq!(printed.status, 0, "{printed:?}");
        for fact in [
            b"\"number\":43".as_slice(),
            b"\"number\":71",
            b"\"provider\":9",
            b"\"until\":100",
            b"\"project\":1",
            b"\"provider_name\":",
            b"https://provider.example/usage",
        ] {
            assert!(f::json_has(&printed, fact), "{fact:?}");
        }
        assert_eq!(root.watches.len(), 1, "only shared me remains");
        let common = f::json_without_digest(&printed.stdout);
        assert_eq!(f::last_digest(&printed.stdout).as_ref(), Digest::of(&common).hex());
    }
}
#[test]
fn account_digests_distinguish_changes_to_all_projected_account_facts() {
    let original = f::accounts();
    let read = f::account_read(&mut f::account_root(false), original.clone(), None);
    let digest = f::last_digest(&read.stdout);
    for change in 0..6u8 {
        let mut value = original.clone();
        let row = value.accounts.get_mut(0).expect("account");
        match change {
            0 => row.sources.get_mut(0).expect("source").spent_until = Some(skein_lib::Wall::from_nanos(101)),
            1 => row.sources.get_mut(0).expect("source").state = c::OwnerAccountState::Granted,
            2 => row.uses = None,
            3 => row.place = 2,
            4 => row.cut = true,
            _ => {
                value.last_end = Some(c::OwnerAccountLastEnd {
                    source: f::SOURCE,
                    end: c::OwnerAccountEnd::Failed { failure: c::OwnerAccountFailure::MissingScope { scope: 7 } },
                });
            }
        }
        let changed = f::account_read(&mut f::account_root(false), value, None);
        assert_ne!(f::last_digest(&changed.stdout), digest, "change {change}");
    }
}
#[test]
fn a_moving_source_and_a_refused_move_print_their_names() {
    let mut value = f::accounts();
    value.accounts.get_mut(0).expect("account").sources.get_mut(0).expect("source").state =
        c::OwnerAccountState::Moving;
    value.last_end = Some(c::OwnerAccountLastEnd {
        source: f::SOURCE,
        end: c::OwnerAccountEnd::Refused { refusal: c::OwnerAccountRefusal::NotOurs },
    });
    let printed = f::account_read(&mut f::account_root(false), value, None);
    assert_eq!(printed.status, 0, "{printed:?}");
    for fact in [b"\"kind\":\"moving\"".as_slice(), b"\"refused\":\"not_ours\""] {
        assert!(f::json_has(&printed, fact), "{fact:?}");
    }
}
#[test]
fn models_group_only_project_authorized_models_without_guessing_provider_readiness() {
    let mut root = f::account_root(false);
    let mut me = f::me(f::PARTY);
    let project = me.projects.get_mut(0).expect("project");
    project.models = List::with_capacity(3);
    for (name, provider) in [(b"first".as_slice(), Some(f::PROVIDER)), (b"other", None), (b"last", Some(f::PROVIDER))] {
        let mut efforts = List::with_capacity(1);
        efforts.push(c::Text::new(b"low".as_slice().into(), 32).expect("effort")).expect("one effort");
        project
            .models
            .push(c::MeModel {
                prices: c::ModelPrices { input: 7, cached: 2, cache_write: Some(9), output: 11, unit: 1000 },
                description: Some(c::Text::new(b"Careful reasoning".as_slice().into(), 64).expect("description")),
                name: c::Text::new(name.into(), 32).expect("model"),
                provider,
                window: 8192,
                output: 1024,
                efforts,
            })
            .expect("models");
    }
    me.providers = f::provider_offers();
    root.deliver(c::Delivery::Me(Box::new(me)));
    for project in ["absent", "999"] {
        let call = root.submit(&["model", "list", "--project", project]);
        assert_eq!(root.answer(call).expect("membership error").status, 1);
        assert_eq!(root.watches.len(), 1, "invalid projects open no watch");
    }
    for flags in [&["model", "list", "--project", "shop", "--json"][..], &["model", "list", "--json"]] {
        let call = root.submit(flags);
        f::deliver_accounts(&mut root, f::accounts());
        let printed = root.answer(call).expect("models join");
        for fact in
            [b"\"window\":8192".as_slice(), b"\"output\":1024", b"first", b"other", b"last", b"\"efforts\":[\"low\"]"]
        {
            assert!(f::json_has(&printed, fact), "{fact:?}");
        }
        for fact in [
            b"\"description\":\"Careful reasoning\"".as_slice(),
            b"\"prices\":{\"input\":7,\"cached\":2,\"cache_write\":9,\"output\":11,\"unit\":1000}",
        ] {
            assert!(f::json_has(&printed, fact), "{fact:?}");
        }
        let mut groups = 0u32;
        let end = skein_lib::bytes::find(&printed.stdout, b"\"current\":").expect("model projection ends");
        for window in printed.stdout.get(..end).expect("models").windows(b"\"provider\":9,\"provider_name\"".len()) {
            if window == b"\"provider\":9,\"provider_name\"" {
                groups += 1;
            }
        }
        assert_eq!(groups, 1, "one group per supplied provider");
        for absent in [b"needs an account".as_slice(), b"OpenAI"] {
            assert!(!f::json_has(&printed, absent));
        }
        let accounts = f::account_read(&mut root, f::accounts(), None);
        assert_ne!(
            f::last_digest(&printed.stdout),
            f::last_digest(&accounts.stdout),
            "models is its own joined digest"
        );
    }
}
#[test]
fn account_output_truncates_oversized_projections_within_the_result_window() {
    let mut value = f::accounts();
    let mut row = value.accounts.get(0).expect("row").clone();
    let mut escaped = skein_lib::Writer::new(usize::try_from(crate::limits::TEXT).expect("text fits"));
    for _ in 0..crate::limits::TEXT {
        escaped.put(b"\"").expect("escaped text fits");
    }
    row.display = f::document_text(&escaped.finish());
    value.accounts = List::with_capacity(crate::limits::ROWS + 1);
    for _ in 0..=crate::limits::ROWS {
        value.accounts.push(row.clone()).expect("one past window");
        row.display = f::document_text(b"Work");
    }
    for json in [false, true] {
        let mut root = f::account_root(false);
        let flags = if json { &["account", "list", "--json"][..] } else { &["account", "list"][..] };
        let call = root.submit(flags);
        f::deliver_accounts(&mut root, value.clone());
        let printed = root.answer(call).expect("cut result");
        assert_eq!(printed.status, 0, "{printed:?}");
        assert!(printed.stdout.len() <= usize::try_from(crate::limits::TEXT).expect("text fits"));
        assert!(f::json_has(&printed, if json { b"\"cut\":true" } else { b"cut: true" }));
        assert!(f::json_has(&printed, b"bytes_left"));
    }
}

fn me_read(me: c::Me, json: bool) -> crate::jig::Printed {
    let mut root = f::account_root(true);
    root.deliver(c::Delivery::Me(Box::new(me)));
    let flags = if json { &["me", "--json"][..] } else { &["me"][..] };
    let call = root.submit(flags);
    root.answer(call).expect("me projection")
}
#[test]
fn me_provider_name_loss_marks_cut() {
    let mut me = f::me(f::PARTY);
    me.providers = f::provider_offers();
    me.providers.get_mut(0).expect("offer").name.cut = true;
    assert!(f::json_has(&me_read(me, true), b"\"cut\":true,\"retention\":"));
}
#[test]
fn me_program_list_loss_marks_cut() {
    let mut me = f::me(f::PARTY);
    me.providers = f::provider_offers();
    me.providers.get_mut(0).expect("offer").programs_cut = true;
    assert!(f::json_has(&me_read(me, true), b"\"cut\":true,\"retention\":"));
}
#[test]
fn me_provider_list_loss_marks_cut() {
    let mut me = f::me(f::PARTY);
    me.providers = f::provider_offers();
    me.providers_cut = true;
    assert!(f::json_has(&me_read(me, true), b"\"cut\":true,\"retention\":"));
}
#[test]
fn party_readiness_loss_marks_accounts_cut() {
    let mut party = f::accounts();
    let deployment = f::deployment();
    party.readiness_cut = true;
    let printed = f::account_read(&mut f::account_root(true), party, Some(deployment));
    assert!(f::json_without_digest(&printed.stdout).ends_with(b"\"cut\":true}"));
}
#[test]
fn deployment_readiness_loss_marks_accounts_cut() {
    let party = f::accounts();
    let mut deployment = f::deployment();
    deployment.readiness_cut = true;
    let printed = f::account_read(&mut f::account_root(true), party, Some(deployment));
    assert!(f::json_without_digest(&printed.stdout).ends_with(b"\"cut\":true}"));
}
#[test]
fn deployment_provider_name_loss_marks_accounts_cut() {
    let party = f::accounts();
    let mut deployment = f::deployment();
    deployment.providers = f::provider_offers();
    deployment.providers.get_mut(0).expect("offer").name.cut = true;
    let printed = f::account_read(&mut f::account_root(true), party, Some(deployment));
    assert!(f::json_without_digest(&printed.stdout).ends_with(b"\"cut\":true}"));
}
#[test]
fn deployment_program_list_loss_marks_accounts_cut() {
    let party = f::accounts();
    let mut deployment = f::deployment();
    deployment.providers = f::provider_offers();
    deployment.providers.get_mut(0).expect("offer").programs_cut = true;
    let printed = f::account_read(&mut f::account_root(true), party, Some(deployment));
    assert!(f::json_without_digest(&printed.stdout).ends_with(b"\"cut\":true}"));
}
#[test]
fn deployment_provider_list_loss_marks_accounts_cut() {
    let party = f::accounts();
    let mut deployment = f::deployment();
    deployment.providers = f::provider_offers();
    deployment.providers_cut = true;
    let printed = f::account_read(&mut f::account_root(true), party, Some(deployment));
    assert!(f::json_without_digest(&printed.stdout).ends_with(b"\"cut\":true}"));
}
#[test]
fn me_text_exposes_configured_provider_offers() {
    let mut root = f::account_root(true);
    let mut me = f::me(f::PARTY);
    me.providers = f::provider_offers();
    root.deliver(c::Delivery::Me(Box::new(me)));
    let call = root.submit(&["me"]);
    let printed = root.answer(call).expect("offers text");
    for fact in [
        b"name: Scripted name_cut: false key_bytes: 256".as_slice(),
        b"sign_in: true key: true none: false signed_in_plan: false programs_cut: false",
        b"programs: helper manage_usage: https://provider.example/usage",
        b"providers_cut: false\nkey_bytes: 256\ncut: false\n",
    ] {
        assert!(f::has(&printed, fact), "{fact:?}");
    }
}
#[test]
fn deployment_offers_share_the_accounts_result_window() {
    const OFFERS: u32 = 64;
    let mut deployment = f::deployment();
    let offer = f::provider_offers().get(0).expect("offer").clone();
    deployment.providers = List::with_capacity(OFFERS);
    deployment.readiness = List::with_capacity(OFFERS);
    for _ in 0..OFFERS {
        deployment.providers.push(offer.clone()).expect("offer room");
        deployment.readiness.push(*f::payer_readiness().get(0).expect("readiness")).expect("readiness room");
    }
    for flags in [&["account", "list"][..], &["account", "list", "--json"]] {
        let mut root = f::account_root(true);
        let call = root.submit(flags);
        f::deliver_accounts(&mut root, f::accounts());
        f::deliver_deployment(&mut root, deployment.clone());
        let printed = root.answer(call).expect("bounded offers");
        assert_eq!(printed.status, 0);
        assert!(printed.stdout.len() <= usize::try_from(crate::limits::TEXT).expect("text fits"));
        assert!(f::json_has(&printed, if flags.len() > 2 { b"\"cut\":true" } else { b"cut: true" }));
    }
}

#[test]
fn account_projection_includes_configured_worker_login_facts() {
    let mut accounts = f::accounts();
    let mut programs = List::with_capacity(1);
    programs
        .push(c::OwnerProgramLogin {
            name: b"work-login".as_slice().into(),
            state: Some(c::OwnerProgramState::SignedInOtherwise { method: b"other-method".as_slice().into() }),
            sign_out: None,
            cleanup: false,
            cut: false,
        })
        .unwrap();
    accounts.program_hosts = List::with_capacity(1);
    accounts
        .program_hosts
        .push(c::OwnerProgramHost {
            name: b"configured-worker".as_slice().into(),
            channel: 17,
            connected: true,
            awaiting_logins: false,
            programs,
            cut: false,
        })
        .unwrap();
    let mut root = f::account_root(false);
    let call = root.submit(&["account", "list", "--json"]);
    f::deliver_accounts(&mut root, accounts);
    let printed = root.answer(call).expect("account projection");
    for fact in [b"configured-worker".as_slice(), b"work-login", b"other-method"] {
        assert!(skein_lib::bytes::find(&printed.stdout, fact).is_some(), "{printed:?}");
    }
}

#[test]
fn model_cache_writes_default_to_the_supplied_input_price() {
    let mut root = f::account_root(false);
    let mut me = f::me(f::PARTY);
    let project = me.projects.get_mut(0).expect("project");
    project.models = List::with_capacity(1);
    project
        .models
        .push(c::MeModel {
            name: c::Text::new(b"reasoner".as_slice().into(), 32).expect("model"),
            provider: Some(f::PROVIDER),
            window: 8192,
            output: 1024,
            efforts: List::with_capacity(0),
            description: None,
            prices: c::ModelPrices { input: 17, cached: 3, cache_write: None, output: 29, unit: 100 },
        })
        .expect("model room");
    root.deliver(c::Delivery::Me(Box::new(me)));
    let call = root.submit(&["model", "list", "--json"]);
    f::deliver_accounts(&mut root, f::accounts());
    let printed = root.answer(call).expect("model prices");
    assert!(f::json_has(&printed, b"\"description\":null"));
    assert!(f::json_has(
        &printed,
        b"\"prices\":{\"input\":17,\"cached\":3,\"cache_write\":17,\"output\":29,\"unit\":100}"
    ));
}

#[test]
fn accounts_show_supplied_deployment_activity_and_no_party_activity() {
    for json in [false, true] {
        let mut root = f::account_root(true);
        let mut deployment = f::deployment();
        let row = deployment.accounts.get_mut(0).expect("account");
        row.runs = List::with_capacity(1);
        row.waiting = List::with_capacity(1);
        let mut chip = f::task_line().chip;
        chip.task = 211;
        row.runs.push(chip.clone()).expect("running task");
        chip.task = 223;
        row.waiting.push(chip).expect("waiting task");
        let call = root.submit(if json { &["account", "list", "--json"][..] } else { &["account", "list"][..] });
        f::deliver_accounts(&mut root, f::accounts());
        f::deliver_deployment(&mut root, deployment);
        let printed = root.answer(call).expect("activity");
        let expected: &[&[u8]] = if json {
            &[b"\"runs\":null,\"waiting\":null", b"\"runs\":[211],\"waiting\":[223]"]
        } else {
            &[b"runs: null", b"waiting: null", b"runs: \n        - 211", b"waiting: \n        - 223", b"Scripted"]
        };
        for fact in expected {
            assert!(f::has(&printed, fact), "{fact:?}: {printed:?}");
        }
    }
}
#[test]
fn model_text_prints_supplied_descriptions_prices_efforts_and_provider_name() {
    let mut root = f::account_root(false);
    let mut me = f::choice_me(100);
    me.providers = f::provider_offers();
    let model = me.projects.get_mut(0).expect("project").models.get_mut(0).expect("model");
    model.provider = Some(f::PROVIDER);
    model.description = Some(f::act_text(b"Reasoning and code"));
    model.prices = c::ModelPrices { input: 13, cached: 7, cache_write: None, output: 31, unit: 1000 };
    root.deliver(c::Delivery::Me(Box::new(me)));
    let call = root.submit(&["model", "list", "--project", "shop"]);
    f::deliver_accounts(&mut root, f::accounts());
    let printed = root.answer(call).expect("model text");
    assert_eq!(printed.status, 0);
    assert!(!printed.stdout.starts_with(b"{"));
    for fact in [
        b"Scripted".as_slice(),
        b"description: Reasoning and code",
        b"input: 13",
        b"cached: 7",
        b"cache_write: 13",
        b"output: 31",
        b"unit: 1000",
        b"- high",
    ] {
        assert!(f::has(&printed, fact), "{fact:?}: {printed:?}");
    }
}

#[test]
fn owner_accounts_include_each_supplied_deployment_host_login_state() {
    let mut deployment = f::deployment();
    deployment.hosts = List::with_capacity(1);
    deployment
        .hosts
        .push(c::OwnerHost {
            name: Some(b"deployment-worker".as_slice().into()),
            awaiting_logins: false,
            rule: c::HostRule::Open,
            programs: List::with_capacity(1),
            channel: 17,
            kind: c::OwnerHostKind::Worker,
            slots: 1,
            used: 0,
            free: 1,
            draining: false,
            connected: true,
            grace_until: None,
            grace_remaining: None,
            runs: List::with_capacity(0),
            cut: false,
        })
        .expect("host room");
    let host = deployment.hosts.get_mut(0).expect("fixture host");
    host.programs
        .push(c::OwnerProgramLogin {
            name: b"deployment-login".as_slice().into(),
            state: Some(c::OwnerProgramState::NotSignedIn),
            sign_out: None,
            cleanup: false,
            cut: false,
        })
        .expect("login");
    let printed = f::account_read(&mut f::account_root(true), f::accounts(), Some(deployment));
    for fact in [b"deployment-worker".as_slice(), b"deployment-login", b"not_signed_in"] {
        assert!(f::has(&printed, fact), "{printed:?}");
    }
}
