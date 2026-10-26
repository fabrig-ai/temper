// Fixture parsing without a deployment keeps written paths.
use crate::problem;
use skein_forge_setup::given::{Place, Refused};

#[test]
fn each_kit_place_names_its_temper_path() {
    for (place, other, line) in [
        (Place::BotLogin, None, "application.setup[1].bot.login: invalid form\n"),
        (Place::BotEmail, None, "application.setup[1].bot.email: invalid form\n"),
        (Place::Orgs, None, "application.setup[1].orgs: past its bound\n"),
        (Place::Org(2), None, "application.setup[1].orgs[2]: invalid form\n"),
        (Place::OrgOwner, None, "application.setup[1].mode.owned.person.login: invalid form\n"),
        (Place::OrgsLimit, None, "application.limits.overrides.setup.orgs: outside its type\n"),
        (Place::Deadline, None, "application.limits.overrides.setup.deadline_ms: outside its type\n"),
        (Place::Backoff, None, "application.limits.overrides.setup.backoff_ms: outside its type\n"),
        (Place::BackoffMax, None, "application.limits.overrides.setup.backoff_max_ms: outside its type\n"),
        (
            Place::Backoff,
            Some(Place::BackoffMax),
            "application.limits.overrides.setup.backoff_ms: must agree with application.limits.overrides.setup.backoff_max_ms\n",
        ),
        (
            Place::BackoffMax,
            Some(Place::Deadline),
            "application.limits.overrides.setup.backoff_max_ms: must agree with application.limits.overrides.setup.deadline_ms\n",
        ),
        (Place::Word, None, "temper's word: invalid form\n"),
        (Place::AdminName, None, "temper's admin name: invalid form\n"),
        (Place::ApiName, None, "temper's API token name: invalid form\n"),
        (Place::GitName, None, "temper's git token name: invalid form\n"),
    ] {
        let problems = crate::given::refusal(1, Refused { place, other });
        assert_eq!(&*problem::lines(&problems), line.as_bytes(), "{place:?}");
    }
}

fn expected_given() -> skein_forge_setup::given::Given {
    use alloc::boxed::Box;
    use skein_lib::Duration;
    skein_forge_setup::given::Given {
        forge: 1,
        bot_login: b"temper".as_slice().into(),
        bot_email: b"temper@example.invalid".as_slice().into(),
        orgs: Box::new([]),
        // None adopted until temper-14a gives the owned forge’s adopted list.
        adopted: Box::new([]),
        org_owner: None,
        listener: None,
        word: b"temper".as_slice().into(),
        admin_name: b"temper-admin".as_slice().into(),
        api_name: b"temper-api".as_slice().into(),
        git_name: b"temper-git".as_slice().into(),
        limits: skein_forge_setup::limits::Limits {
            orgs: 8,
            files: 2,
            file_bytes: 4096,
            reachable: 8,
            repositories: crate::limits::small().repositories,
            deadline: Duration::from_secs(120),
            backoff: Duration::from_millis(100),
            backoff_max: Duration::from_secs(5),
        },
    }
}

#[test]
fn a_managed_setup_maps_every_given_value() {
    let setup = super::fixtures::setup();
    let given = crate::given::values(setup.entry(1).unwrap(), None, &crate::limits::small());
    assert_eq!(given, expected_given());
    assert_eq!(skein_forge_setup::given::check(&given), Ok(()));
}

#[test]
fn a_fake_setup_maps_every_given_value() {
    let setup = super::fixtures::fake_setup();
    let given = crate::given::values(setup.entry(3).unwrap(), None, &crate::limits::small());
    assert_eq!(given, skein_forge_setup::given::Given { forge: 3, ..expected_given() });
    assert_eq!(skein_forge_setup::given::check(&given), Ok(()));
}

#[test]
fn an_owned_setup_maps_every_given_value() {
    let setup = crate::setup::Setup::read(
        &super::fixtures::document(super::fixtures::TWO),
        0,
        &crate::limits::small(),
        &jig_protocol_config::Directories::working(),
        &mut problem::Problems::empty(),
    )
    .unwrap();
    let listener = super::fixtures::listener();
    let given = crate::given::values(setup.entry(2).unwrap(), Some(listener), &crate::limits::small());
    let expected = skein_forge_setup::given::Given {
        forge: 2,
        orgs: Box::new([]),
        org_owner: Some(b"ada".as_slice().into()),
        listener: Some(listener),
        ..expected_given()
    };
    assert_eq!(given, expected);
    assert_eq!(skein_forge_setup::given::check(&given), Ok(()));
}

#[test]
fn a_refused_limit_pair_keeps_its_order_and_is_printed_once() {
    use super::fixtures;
    for bytes in [fixtures::TWO, b"[]"] {
        let problems = crate::check::checked(
            &fixtures::document(bytes),
            0,
            &jig_protocol_config::Directories::working(),
            &fixtures::members(),
            None,
            &[(b"setup.backoff_ms".as_slice().into(), 121_000), (b"setup.backoff_max_ms".as_slice().into(), 120_000)],
        )
        .unwrap_err();
        assert_eq!(
            &*problem::lines(&problems),
            b"application.limits.overrides.setup.backoff_ms: must agree with application.limits.overrides.setup.backoff_max_ms\n",
            "{bytes:?}",
        );
    }
}

#[test]
fn each_kit_item_names_its_temper_file() {
    use crate::place::{self, File};
    use skein_forge_setup::item::Item;
    for (item, expected) in [
        (Item::AdminToken, File::AdminToken),
        (Item::ApiToken, File::ApiToken),
        (Item::GitToken, File::GitToken),
        (Item::User, File::User),
        (Item::WebhookSecret, File::WebhookSecret),
    ] {
        assert_eq!(place::item(item), expected, "{item:?}");
    }
}

#[test]
fn configured_repository_capacity_reaches_checked_given_and_names_its_refusal() {
    use super::fixtures;
    let directories = jig_protocol_config::Directories::working();
    for capacity in [1_u64, 3] {
        let checked = crate::check::checked(
            &fixtures::document(fixtures::TWO),
            0,
            &directories,
            &fixtures::members(),
            None,
            &[(b"forge.client.repositories".as_slice().into(), capacity)],
        )
        .unwrap();
        let given = crate::given::values(checked.setup.entry(2).unwrap(), Some(fixtures::listener()), &checked.limits);
        assert_eq!(u64::from(given.limits.repositories), capacity);
    }
    let refused = crate::check::checked(
        &fixtures::document(fixtures::TWO),
        0,
        &directories,
        &fixtures::members(),
        None,
        &[(b"forge.client.repositories".as_slice().into(), 0)],
    )
    .unwrap_err();
    assert_eq!(
        problem::lines(&refused).as_ref(),
        b"application.limits.overrides.forge.client.repositories: outside its type\n"
    );
    for place in [Place::Repositories, Place::Repository(1), Place::RepositoriesLimit] {
        assert_eq!(
            problem::lines(&crate::given::refusal(0, Refused { place, other: None })).as_ref(),
            b"application.limits.overrides.forge.client.repositories: outside its type\n"
        );
    }
}
