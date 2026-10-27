use crate::tests::fixtures as f;
use jig_client::contract as c;
use skein_lib::Time;
const WAIT_ACCOUNTS: &[&str] = &["wait", "accounts"];

#[test]
fn account_waits_recompute_the_same_projection_as_account_reads() {
    for owner in [false, true] {
        let mut root = f::account_root(owner);
        let mut party = f::accounts();
        let mut deployment = f::deployment();
        let before = f::account_read(&mut root, party.clone(), owner.then_some(deployment.clone()));
        let digest = f::last_digest(&before.stdout);
        let call = root.wait_since(WAIT_ACCOUNTS, &["--until", "settled"], &digest);
        f::deliver_accounts(&mut root, party.clone());
        if owner {
            f::deliver_deployment(&mut root, deployment.clone());
        }
        assert!(root.answer(call).is_none(), "identical joined digest stays waiting");
        if owner {
            deployment.accounts.get_mut(0).expect("deployment account").generation = Some(2);
            f::deliver_deployment(&mut root, deployment.clone());
        } else {
            party.accounts.get_mut(0).expect("party account").place = 2;
            f::deliver_accounts(&mut root, party.clone());
        }
        let changed = root.answer(call).expect("change and settled on the same current projection");
        let read = f::account_read(&mut root, party, owner.then_some(deployment));
        assert_eq!(changed.status, 0, "{changed:?}");
        let prefix = changed.stdout.strip_suffix(b",\"met\":true}").expect("wait envelope");
        assert_eq!(f::appended(prefix, b"}"), read.stdout, "read and wait expose identical facts/digest");
        assert_eq!(root.watches.len(), 1, "both joined handles closed");
    }
}
#[test]
fn account_conditions_wait_for_the_owners_deployment_document() {
    for usable in [false, true] {
        for settled in [false, true] {
            for condition in ["usable", "settled"] {
                let (mut root, call, _) = account_condition(condition, usable, settled);
                assert!(root.answer(call).is_none(), "owner's deployment is still unknown");
            }
        }
    }
}
#[test]
fn account_conditions_complete_when_the_party_boolean_is_true() {
    for (usable, settled, condition) in
        [(true, false, "usable"), (true, true, "usable"), (false, true, "settled"), (true, true, "settled")]
    {
        let (mut root, call, _) = account_condition(condition, usable, settled);
        f::deliver_deployment(&mut root, f::deployment());
        assert_eq!(root.answer(call).expect("supplied true").status, 0);
    }
}
#[test]
fn account_conditions_ignore_deployment_usability_until_the_party_changes() {
    for (usable, settled, condition) in
        [(false, false, "usable"), (false, true, "usable"), (false, false, "settled"), (true, false, "settled")]
    {
        let (mut root, call, mut party) = account_condition(condition, usable, settled);
        f::deliver_deployment(&mut root, f::deployment());
        assert!(root.answer(call).is_none(), "deployment usability cannot satisfy party condition");
        party.usable = true;
        party.settled = true;
        f::deliver_accounts(&mut root, party);
        assert_eq!(root.answer(call).expect("now true").status, 0);
    }
}
#[test]
fn a_behind_join_never_answers_from_its_cached_facts() {
    for behind in [&f::ACCOUNT_WATCH, &f::DEPLOYMENT_WATCH] {
        let mut root = f::account_root(true);
        let call = root.submit(&["wait", "accounts", "--json"]);
        let mut party = f::accounts();
        let mut deployment = f::deployment();
        f::deliver_accounts(&mut root, party.clone());
        f::deliver_deployment(&mut root, deployment.clone());
        // jig-client marks a retained document Behind when its stream drops.
        root.end(call, behind, c::WatchEnd::Dropped);
        if *behind == f::ACCOUNT_WATCH {
            deployment.accounts.get_mut(0).expect("deployment account").generation = Some(2);
            f::deliver_deployment(&mut root, deployment.clone());
        } else {
            party.usable = true;
            f::deliver_accounts(&mut root, party.clone());
        }
        assert!(root.answer(call).is_none(), "other handle changed while join was Behind");
        root.now = crate::root::next_deadline(&root.root).expect("reopen deadline");
        root.fire();
        f::deliver_accounts(&mut root, party);
        f::deliver_deployment(&mut root, deployment);
        assert_eq!(root.answer(call).expect("both current after refresh").status, 0);
    }
}
#[test]
fn account_timeouts_release_both_watches_without_retaining_a_deadline() {
    for delivered in [false, true] {
        let mut root = f::account_root(true);
        let call = root.submit(&["wait", "accounts", "--until", "usable", "--timeout", "1", "--json"]);
        root.watch_event(&f::ACCOUNT_WATCH, c::WatchEvent::Opened);
        root.watch_event(&f::DEPLOYMENT_WATCH, c::WatchEvent::Opened);
        if delivered {
            f::deliver_accounts(&mut root, f::accounts());
            f::deliver_deployment(&mut root, f::deployment());
        }
        root.now = Time::ZERO.checked_add(skein_lib::Duration::from_secs(1)).expect("deadline");
        root.fire();
        let printed = root.answer(call).expect("timeout");
        assert_eq!(printed.status, 3);
        assert!(f::json_has(&printed, b"\"why\":\"timeout\",\"met\":false"));
        assert_eq!(root.watches.len(), 1);
        assert_eq!(crate::root::next_deadline(&root.root), None);
    }
}
#[test]
fn account_permission_loss_releases_both_joined_watches() {
    let mut root = f::account_root(true);
    let call = root.submit(&["wait", "accounts", "--until", "usable"]);
    root.watch_event(&f::ACCOUNT_WATCH, c::WatchEvent::Opened);
    root.watch_event(&f::DEPLOYMENT_WATCH, c::WatchEvent::Opened);
    root.end(call, &f::DEPLOYMENT_WATCH, c::WatchEnd::NotAllowed);
    assert_eq!(root.answer(call).expect("deployment permission lost").status, 2);
    assert_eq!(root.watches.len(), 1);
}
#[test]
fn account_waits_need_a_changed_digest_with_a_met_condition() {
    let mut root = f::account_root(false);
    let mut party = f::accounts();
    let before = f::account_read(&mut root, party.clone(), None);
    let call = root.wait_since(WAIT_ACCOUNTS, &["--until", "usable"], &f::last_digest(&before.stdout));
    f::deliver_accounts(&mut root, party.clone());
    party.accounts.get_mut(0).expect("row").place = 2;
    f::deliver_accounts(&mut root, party.clone());
    assert!(root.answer(call).is_none(), "digest changed but usable is false");
    party.usable = true;
    f::deliver_accounts(&mut root, party.clone());
    assert_eq!(root.answer(call).expect("both hold").status, 0);
}
#[test]
fn account_text_waits_append_the_condition_to_read_output() {
    let mut root = f::account_root(false);
    let mut party = f::accounts();
    party.usable = true;
    let read = root.submit(&["account", "list"]);
    f::deliver_accounts(&mut root, party.clone());
    let expected = root.answer(read).expect("text read");
    let wait = root.submit(&["wait", "accounts", "--until", "usable"]);
    f::deliver_accounts(&mut root, party);
    assert_eq!(root.answer(wait).expect("text wait").stdout, f::appended(&expected.stdout, b"met\n"));
}

fn account_condition(condition: &str, usable: bool, settled: bool) -> (f::Harness, u32, c::OwnerAccounts) {
    let mut root = f::account_root(true);
    let call = root.submit(&["wait", "accounts", "--until", condition, "--json"]);
    let mut party = f::accounts();
    party.usable = usable;
    party.settled = settled;
    f::deliver_accounts(&mut root, party.clone());
    (root, call, party)
}

#[test]
fn me_offer_changes_wake_a_wait() {
    for change in OfferChange::ALL {
        let mut root = f::account_root(true);
        let mut me = f::me(f::PARTY);
        me.providers = f::provider_offers();
        root.deliver(c::Delivery::Me(Box::new(me.clone())));
        let read = root.submit(&["me", "--json"]);
        let before = root.answer(read).expect("me projection");
        let call = root.wait_since(&["wait", "me"], &[], &f::last_digest(&before.stdout));
        root.deliver(c::Delivery::Me(Box::new(me.clone())));
        assert!(root.answer(call).is_none(), "unchanged offers");
        (me.providers, me.providers_cut, me.key_bytes) = changed_offers(change);
        root.deliver(c::Delivery::Me(Box::new(me)));
        assert_eq!(root.answer(call).expect("offer change").status, 0, "change {change:?}");
    }
}
#[test]
fn deployment_offer_changes_wake_an_accounts_wait() {
    for change in OfferChange::ALL {
        let mut root = f::account_root(true);
        let mut deployment = f::deployment();
        deployment.providers = f::provider_offers();
        let before = f::account_read(&mut root, f::accounts(), Some(deployment.clone()));
        let call = root.wait_since(WAIT_ACCOUNTS, &[], &f::last_digest(&before.stdout));
        f::deliver_accounts(&mut root, f::accounts());
        f::deliver_deployment(&mut root, deployment.clone());
        assert!(root.answer(call).is_none(), "unchanged offers");
        (deployment.providers, deployment.providers_cut, deployment.key_bytes) = changed_offers(change);
        f::deliver_deployment(&mut root, deployment);
        assert_eq!(root.answer(call).expect("offer change").status, 0, "change {change:?}");
    }
}
#[test]
fn party_readiness_changes_wake_an_accounts_wait() {
    for change in ReadinessChange::ALL {
        let mut root = f::account_root(false);
        let mut party = f::accounts();
        party.readiness = f::payer_readiness();
        let before = f::account_read(&mut root, party.clone(), None);
        let call = root.wait_since(WAIT_ACCOUNTS, &[], &f::last_digest(&before.stdout));
        f::deliver_accounts(&mut root, party.clone());
        assert!(root.answer(call).is_none(), "unchanged readiness");
        (party.readiness, party.readiness_cut) = changed_readiness(change);
        f::deliver_accounts(&mut root, party);
        assert_eq!(root.answer(call).expect("readiness change").status, 0, "change {change:?}");
    }
}
#[test]
fn deployment_readiness_changes_wake_an_accounts_wait() {
    for change in ReadinessChange::ALL {
        let mut root = f::account_root(true);
        let mut deployment = f::deployment();
        deployment.readiness = f::payer_readiness();
        let before = f::account_read(&mut root, f::accounts(), Some(deployment.clone()));
        let call = root.wait_since(WAIT_ACCOUNTS, &[], &f::last_digest(&before.stdout));
        f::deliver_accounts(&mut root, f::accounts());
        f::deliver_deployment(&mut root, deployment.clone());
        assert!(root.answer(call).is_none(), "unchanged readiness");
        (deployment.readiness, deployment.readiness_cut) = changed_readiness(change);
        f::deliver_deployment(&mut root, deployment);
        assert_eq!(root.answer(call).expect("readiness change").status, 0, "change {change:?}");
    }
}
#[derive(Clone, Copy, Debug)]
enum OfferChange {
    Removed,
    Provider,
    KeyBytes,
    Name,
    NameCut,
    SignIn,
    Key,
    None,
    SignedInPlan,
    Program,
    ProgramsCut,
    ManageUsage,
    ProvidersCut,
    Ceiling,
}
impl OfferChange {
    const ALL: [Self; 14] = [
        Self::Removed,
        Self::Provider,
        Self::KeyBytes,
        Self::Name,
        Self::NameCut,
        Self::SignIn,
        Self::Key,
        Self::None,
        Self::SignedInPlan,
        Self::Program,
        Self::ProgramsCut,
        Self::ManageUsage,
        Self::ProvidersCut,
        Self::Ceiling,
    ];
}
#[derive(Clone, Copy, Debug)]
enum ReadinessChange {
    Removed,
    Provider,
    Payer,
    Usable,
    Unavailable,
    Settled,
    Cut,
}
impl ReadinessChange {
    const ALL: [Self; 7] =
        [Self::Removed, Self::Provider, Self::Payer, Self::Usable, Self::Unavailable, Self::Settled, Self::Cut];
}
fn changed_offers(change: OfferChange) -> (skein_lib::List<c::ProviderOffer>, bool, u32) {
    let mut providers = f::provider_offers();
    let row = providers.get_mut(0).expect("offer");
    let mut cut = false;
    let mut key_bytes = 256;
    match change {
        OfferChange::Removed => providers = skein_lib::List::with_capacity(0),
        OfferChange::Provider => row.provider = f::PROVIDER.checked_add(1).expect("other provider"),
        OfferChange::KeyBytes => row.key_bytes = 257,
        OfferChange::Name => row.name.bytes = c::Text::new(b"renamed".as_slice().into(), 32).expect("name"),
        OfferChange::NameCut => row.name.cut = true,
        OfferChange::SignIn => row.sign_in = false,
        OfferChange::Key => row.key = false,
        OfferChange::None => row.none = true,
        OfferChange::SignedInPlan => row.signed_in_plan = true,
        OfferChange::Program => {
            *row.programs.get_mut(0).expect("program") = c::Text::new(b"other".as_slice().into(), 32).expect("program");
        }
        OfferChange::ProgramsCut => row.programs_cut = true,
        OfferChange::ManageUsage => row.manage_usage = None,
        OfferChange::ProvidersCut => cut = true,
        OfferChange::Ceiling => key_bytes = 257,
    }
    (providers, cut, key_bytes)
}
fn changed_readiness(change: ReadinessChange) -> (skein_lib::List<c::PayerReadiness>, bool) {
    let mut rows = f::payer_readiness();
    let row = rows.get_mut(0).expect("readiness");
    let mut cut = false;
    match change {
        ReadinessChange::Removed => rows = skein_lib::List::with_capacity(0),
        ReadinessChange::Provider => row.provider = f::PROVIDER.checked_add(1).expect("other provider"),
        ReadinessChange::Payer => row.payer = c::OwnerAccountUse::Project(1),
        ReadinessChange::Usable => row.readiness = c::Readiness::Usable,
        ReadinessChange::Unavailable => row.readiness = c::Readiness::Unavailable,
        ReadinessChange::Settled => row.settled = Some(false),
        ReadinessChange::Cut => cut = true,
    }
    (rows, cut)
}
