use super::fixtures;
use crate::{limits, tokens};
use skein_lib::{Duration, Token};

#[test]
fn no_two_component_prefixes_meet() {
    let owners = [
        tokens::WORKER_LISTENERS,
        tokens::WORKER_CONNECTIONS,
        tokens::ACCOUNTS,
        tokens::CLIENTS,
        tokens::SERVER,
        tokens::FORGE,
        tokens::GIT,
        tokens::STORE[0],
        tokens::STORE[1],
        tokens::STORE[2],
    ];
    for (index, owner) in owners.iter().enumerate() {
        for other in owners.iter().skip(index + 1) {
            assert_ne!(tokens::prefix(Token::new(*owner)), tokens::prefix(Token::new(*other)), "{owner:x}, {other:x}");
        }
    }
    assert_eq!(tokens::prefix(Token::new(tokens::SIGNALS)), 0);
    for index in [0, 1, 4094] {
        assert_eq!(tokens::worker(index), Some((3_u64 << 60_u32) + (u64::from(index + 1) << 48_u32)));
        assert_eq!(tokens::git(index), Some((6_u64 << 60_u32) + (u64::from(index + 1) << 48_u32)));
    }
    assert_ne!(tokens::worker(0), Some(tokens::ACCOUNTS));
    for index in [4095, 4096, u32::MAX] {
        assert_eq!(tokens::worker(index), None, "{index}");
        assert_eq!(tokens::git(index), None, "{index}");
    }
}

#[test]
fn an_override_sets_the_field_its_dotted_name_names() {
    let mut limits = limits::profile(limits::Profile::Small);
    assert_eq!((limits.forges.log_origins, limits.forge_component.log_origins), (4, 4));
    assert_eq!(limits.forge_component.log_poll, Duration::from_secs(5));
    assert_eq!(limits.forge_component.log_catch_up, Duration::from_secs(86_400));
    assert_eq!(limits.forge_component.log_pages, 10);
    assert_eq!(limits.forge_component.delivery_bytes, 1_048_576);
    let rows: [(&[u8], u64); 14] = [
        (b"forge.repositories", 3),
        (b"forge.client.calls", 5),
        (b"forge.client.settle_ms", 2_000),
        (b"checkout.remote_timeout_ms", 7_000),
        (b"root.loads.rows", 9),
        (b"root.loads.read_bytes", 131_072),
        (b"clients.bundle_files", 2),
        (b"clients.bundle_bytes", 61),
        (b"forges.log_origins", 3),
        (b"forge_component.log_origins", 7),
        (b"forge_component.log_poll_ms", 7123),
        (b"forge_component.log_catch_up_ms", 9321),
        (b"forge_component.log_pages", 13),
        (b"forge_component.delivery_bytes", 12_345),
    ];
    for (name, value) in rows {
        assert_eq!(limits::apply(&mut limits, name, value), Ok(()), "{name:?}");
    }
    assert_eq!(limits.forge.repositories, 3);
    assert_eq!(limits.forge.client.calls, 5);
    assert_eq!(limits.forge.client.settle, Duration::from_secs(2));
    assert_eq!(limits.checkout.remote_timeout, Duration::from_millis(7_000));
    assert_eq!(limits.root.loads.rows, 9);
    assert_eq!(limits.root.loads.read_bytes, 131_072);
    assert_eq!(limits.clients, limits::Clients { bundle_files: 2, bundle_bytes: 61 });
    assert_eq!((limits.forges.log_origins, limits.forge_component.log_origins), (3, 7));
    assert_eq!(limits.forge_component.log_poll, Duration::from_nanos(7_123_000_000));
    assert_eq!(limits.forge_component.log_catch_up, Duration::from_nanos(9_321_000_000));
    assert_eq!(limits.forge_component.log_pages, 13);
    assert_eq!(limits.forge_component.delivery_bytes, 12_345);
}

#[test]
fn an_unknown_name_or_a_value_past_its_type_is_refused() {
    let mut limits = limits::profile(limits::Profile::Small);
    let before = limits;
    let rows: [(&[u8], u64, limits::Refused); 5] = [
        (b"forge.unknown", 1, limits::Refused::Unknown),
        (b"Forge.repositories", 1, limits::Refused::Unknown),
        (b"forge.repositories", u64::from(u32::MAX) + 1, limits::Refused::Range),
        (b"root.loads.rows", u64::MAX, limits::Refused::Range),
        (b"checkout.remote_timeout_ms", u64::MAX, limits::Refused::Range),
    ];
    for (name, value, refused) in rows {
        assert_eq!(limits::apply(&mut limits, name, value), Err(refused), "{name:?}");
        assert_eq!(limits, before, "{name:?}");
    }
    for (name, maximum) in [
        (b"forges.log_origins".as_slice(), u64::from(u32::MAX)),
        (b"forge_component.log_origins", u64::from(u32::MAX)),
        (b"forge_component.log_poll_ms", u64::MAX / 1_000_000),
        (b"forge_component.log_catch_up_ms", u64::MAX / 1_000_000),
        (b"forge_component.log_pages", u64::from(u32::MAX)),
        (b"forge_component.delivery_bytes", u64::from(u32::MAX)),
    ] {
        assert_eq!(limits::apply(&mut limits, name, maximum), Ok(()));
        let before = limits;
        assert_eq!(limits::apply(&mut limits, name, maximum + 1), Err(limits::Refused::Range));
        assert_eq!(limits, before, "{name:?}");
    }
}

#[test]
fn the_small_profile_passes_each_domains_worst_case() {
    let settings = fixtures::engine();
    assert!(temper_engine_domain_forge::worst_case(&settings.limits.forge).is_some());
    assert!(temper_worker_domain_checkout::worst_case(&settings.limits.checkout).is_some());
    assert_eq!(temper_worker_protocol_git::check(&settings.limits.git_component, &settings.limits.checkout), Ok(()));
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    let root = limits::engine(&jig, &settings.limits).expect("coherent root profile");
    assert!(temper_engine_domain::worst_case(&root).is_some());
}

#[test]
fn the_wire_admits_a_moved_file_and_temper_values_stay_at_their_bound() {
    let mut settings = fixtures::engine();
    let moved = jig_protocol_accounts::moved::bound(&settings.jig.limits.driver.client.document);
    let value = settings.limits.records.value_bytes;
    assert!(moved > u64::from(value));
    let service = fixtures::started();
    let wire = service.engine.as_ref().unwrap().clients.env.limits.wire.keyed_value_some;
    assert_eq!(u64::from(wire), moved);

    // The effective ceiling remains capped by jig even for a larger moved file.
    let mut larger = settings.jig.limits;
    larger.driver.client.document.record_bytes = u32::MAX;
    assert_eq!(
        limits::client_value_bytes(&larger, &settings.limits),
        settings.jig.limits.clients.wire.keyed_value_some
    );

    settings.setup = Box::new([fixtures::owned_setup(1)]);
    settings.records_only = Box::new([1]);
    settings.forges[0].api_token = None;
    for bytes in [value, value + 1] {
        let mut handover = skein_lib::bytes::zeroed(usize::try_from(bytes).unwrap());
        handover.fill(b'x');
        settings.setup[0].handover = Some(skein_lib::Secret::new(handover));
        let (mut forges, _) = crate::forge::prepare(&settings, fixtures::forge_runtime()).unwrap();
        let result = crate::setup::prepare(&settings, &mut forges, &fixtures::seeds().crypto, 2, false);
        if bytes == value {
            assert!(result.is_ok());
        } else {
            let refused = result.err().expect("setup refuses the oversized value");
            assert_eq!(refused.problems.len(), 1);
            let problem = refused.problems.get(0).unwrap();
            assert_eq!(problem.what, jig_protocol_config::What::Bound);
            assert_eq!(
                problem.path,
                jig_protocol_config::Path::root()
                    .member(b"application")
                    .member(b"limits")
                    .member(b"overrides")
                    .member(b"records.value_bytes")
            );
        }
    }
}

#[test]
fn a_brief_holds_a_whole_task_specification() {
    let settings = fixtures::engine();
    let brief = settings.limits.root.brief;
    let spec = settings.jig.limits.core.tasks.spec_bytes;
    // The contract and requester lines follow the words in the task section.
    assert!(brief.budgets.task >= spec + 512, "task section {} for specs of {spec}", brief.budgets.task);
    assert!(brief.read_bytes >= brief.budgets.task && brief.brief_bytes >= brief.budgets.task);
}

#[test]
fn the_root_limits_take_jigs_children_and_temper_own() {
    let settings = fixtures::engine();
    let root = limits::engine(&settings.jig.limits, &settings.limits).expect("coherent root limits");
    let jig = settings.jig.limits.core;
    assert_eq!(root.journal, settings.limits.root.journal);
    assert_eq!(root.loads, settings.limits.root.loads);
    assert_eq!(root.brief, settings.limits.root.brief);
    assert_eq!(root.forge, settings.limits.forge);
    assert_eq!(root.tasks, jig.tasks);
    assert_eq!(root.authority, jig.authority);
    assert_eq!(root.people, jig.people);
    assert_eq!(root.fleet, jig.fleet);
    assert_eq!(root.accounts, jig.accounts);
    assert_eq!(root.notes, jig.notes);
    assert_eq!(root.call_records, jig.call_records);
    let mut refused = settings.limits;
    refused.root.journal.transcript_bytes = 1;
    assert_eq!(limits::engine(&settings.jig.limits, &refused), None);
}

#[test]
fn each_git_component_limit_has_its_dotted_override() {
    let names: [(&[u8], [u32; 6]); 6] = [
        (b"git_component.operations", [7, 1024, 512, 4096, 16, 4080]),
        (b"git_component.stdout_bytes", [3, 7, 512, 4096, 16, 4080]),
        (b"git_component.stderr_bytes", [3, 1024, 7, 4096, 16, 4080]),
        (b"git_component.read_bytes", [3, 1024, 512, 7, 16, 4080]),
        (b"git_component.sweep_entries", [3, 1024, 512, 4096, 7, 4080]),
        (b"git_component.sweep_bytes", [3, 1024, 512, 4096, 16, 7]),
    ];
    for (name, expected) in names {
        let mut limits = limits::profile(limits::Profile::Small);
        limits::apply(&mut limits, name, 7).unwrap();
        let git = limits.git_component;
        assert_eq!(
            [git.operations, git.stdout_bytes, git.stderr_bytes, git.read_bytes, git.sweep_entries, git.sweep_bytes],
            expected,
            "{name:?}"
        );
        assert!(temper_worker_protocol_git::worst_case(&git, &limits.checkout).is_some(), "{name:?}");
    }
}

#[test]
fn the_small_profile_gives_settle_thirty_seconds() {
    assert_eq!(limits::profile(limits::Profile::Small).forge.client.settle, Duration::from_secs(30));
}

#[test]
fn the_hook_limits_have_dotted_overrides() {
    type Row = (&'static [u8], u64, u64, fn(&limits::Limits) -> u64);
    let mut own = limits::profile(limits::Profile::Small);
    let rows: [Row; 5] = [
        (b"forge_component.hooks", 3, 3, |l| u64::from(l.forge_component.hooks)),
        (b"forge_component.hook_head_bytes", 4096, 4096, |l| u64::from(l.forge_component.hook_head_bytes)),
        (b"forge_component.hook_body_bytes", 8192, 8192, |l| u64::from(l.forge_component.hook_body_bytes)),
        (b"forge_component.hook_secret_bytes", 64, 64, |l| u64::from(l.forge_component.hook_secret_bytes)),
        (b"forge_component.hook_deadline_ms", 7000, 7_000_000_000, |l| l.forge_component.hook_deadline.as_nanos()),
    ];
    for (name, input, expected, read) in rows {
        assert_eq!(limits::apply(&mut own, name, input), Ok(()), "{name:?}");
        assert_eq!(read(&own), expected, "{name:?}");
    }
}

#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary tests inspect diagnostic text")]
fn records_and_installations_override_and_check_both_bounds() {
    let jig = fixtures::engine().jig;
    let defaults = limits::profile(limits::Profile::Small);
    assert_eq!(defaults.records.value_bytes, 4096);
    assert_eq!(defaults.forges.installations, 8);
    assert_eq!(defaults.forge_component.installations, 8);
    assert_eq!(defaults.forge_component.key_bytes, u32::try_from(skein_tls::RS256_KEY_BYTES).unwrap());
    assert_eq!(defaults.forge_component.renew_before, Duration::from_secs(600));
    for (name, value, other) in [
        (b"records.value_bytes".as_slice(), 0, b"forge_component.key_bytes".as_slice()),
        (
            b"records.value_bytes",
            u64::from(crate::startup::checks::secret_value_bound(&jig, &defaults).unwrap()) + 1,
            b"engine.limits.store.value",
        ),
        (b"records.value_bytes", u64::from(defaults.forge_component.key_bytes), b"forge_component.key_bytes"),
        (b"forges.installations", 512, b"records.value_bytes"),
        (b"forge_component.key_bytes", u64::from(u32::MAX), b"AppKey"),
        (b"forges.installations", u64::from(u32::MAX), b"Accepted"),
        (b"forges.installations", 0, b"forge_component.installations"),
        (b"forges.installations", 9, b"forge_component.installations"),
        (b"forge_component.installations", 7, b"forges.installations"),
    ] {
        let mut own = defaults;
        limits::apply(&mut own, name, value).unwrap();
        let mut problems = crate::startup::Problems::empty();
        crate::startup::checks::records(&jig, &own, &[], &mut problems);
        let lines = crate::startup::lines(&problems);
        assert!(lines.windows(name.len()).any(|part| part == name), "{name:?}");
        assert!(lines.windows(other.len()).any(|part| part == other), "{other:?}");
    }
    let mut own = defaults;
    for (name, value) in [(b"forge_component.key_bytes".as_slice(), 2000), (b"forge_component.renew_before_ms", 60_000)]
    {
        limits::apply(&mut own, name, value).unwrap();
    }
    assert_eq!(own.forge_component.key_bytes, 2000);
    assert_eq!(own.forge_component.renew_before, Duration::from_secs(60));
    let mut problems = crate::startup::Problems::empty();
    crate::startup::checks::records(&jig, &own, &[], &mut problems);
    assert!(problems.is_empty());
}
#[test]
fn encoded_setup_writes_fit_exactly_and_name_the_short_bound() {
    use crate::startup::{Problems, checks};
    use temper_protocol_records as records;
    let settings = fixtures::engine();
    let mut own = settings.limits;
    let key = skein_lib::bytes::zeroed(usize::try_from(own.forge_component.key_bytes).unwrap());
    assert_eq!(records::encode(&key).len(), 1197); // DER bound plus VERSION.
    let mut ids = skein_lib::List::with_capacity(150);
    for id in 1..=150 {
        ids.push(id).unwrap();
    }
    assert_eq!(records::encode(skein_forge_setup::item::accepted_item(ids.as_slice()).expose()).len(), 1201);
    for (installations, value, expected) in [
        (8, 1197, ""),
        (
            8,
            1196,
            "application.limits.overrides.records.value_bytes: must agree with AppKey (forge_component.key_bytes)\n",
        ),
        (150, 1201, "application.limits.overrides.forge_component.installations: 150 bytes, past its bound of 64\n"),
        (
            150,
            1200,
            concat!(
                "application.limits.overrides.records.value_bytes: must agree with Accepted (forges.installations)\n",
                "application.limits.overrides.forge_component.installations: 150 bytes, past its bound of 64\n"
            ),
        ),
    ] {
        own.forges.installations = installations;
        own.forge_component.installations = installations;
        own.records.value_bytes = value;
        let mut problems = Problems::empty();
        checks::records(&settings.jig, &own, &[], &mut problems);
        assert_eq!(crate::startup::lines(&problems).as_ref(), expected.as_bytes());
    }
}
#[test]
#[expect(clippy::disallowed_macros, clippy::disallowed_methods, reason = "tests format diagnostic text")]
fn log_limits_check_zero_capacity_and_timing_relations() {
    let jig = fixtures::engine().jig;
    let defaults = limits::profile(limits::Profile::Small);
    for name in [
        "forges.log_origins",
        "forge_component.log_origins",
        "forge_component.log_poll_ms",
        "forge_component.log_catch_up_ms",
        "forge_component.log_pages",
        "forge_component.delivery_bytes",
    ] {
        let mut own = defaults;
        limits::apply(&mut own, name.as_bytes(), 0).unwrap();
        let mut problems = crate::startup::Problems::empty();
        crate::startup::checks::records(&jig, &own, &[], &mut problems);
        let mut expected = format!("application.limits.overrides.{name}: must agree with {name} (above 0)\n");
        if name == "forge_component.log_origins" {
            expected +=
                "application.limits.overrides.forges.log_origins: must agree with forge_component.log_origins\n";
        }
        if name == "forge_component.log_catch_up_ms" {
            expected += "application.limits.overrides.forge_component.log_catch_up_ms: must agree with forge_component.log_poll_ms\n";
        }
        assert_eq!(crate::startup::lines(&problems).as_ref(), expected.as_bytes());
    }
    let day = 86_400_000;
    for (origins, capacity, poll, catch_up, expected) in [
        (4, 4, 5000, day, ""),
        (4, 5, 5000, day, ""),
        (5, 5, 5000, day, "forges.log_origins: must agree with forges.log_origins (at most 4)\n"),
        (2, 2, 5000, day, ""),
        (3, 2, 5000, day, "forges.log_origins: must agree with forge_component.log_origins\n"),
        (4, 4, day, day, ""),
        (4, 4, day, day - 1, "forge_component.log_catch_up_ms: must agree with forge_component.log_poll_ms\n"),
        (
            4,
            4,
            5000,
            day + 1,
            "forge_component.log_catch_up_ms: must agree with forge_component.log_catch_up_ms (at most 86400000 ms / 24 h)\n",
        ),
    ] {
        let mut own = defaults;
        own.forges.log_origins = origins;
        own.forge_component.log_origins = capacity;
        own.forge_component.log_poll = Duration::from_millis(poll);
        own.forge_component.log_catch_up = Duration::from_millis(catch_up);
        let mut problems = crate::startup::Problems::empty();
        crate::startup::checks::records(&jig, &own, &[], &mut problems);
        let expected = [if expected.is_empty() { "" } else { "application.limits.overrides." }, expected].concat();
        assert_eq!(crate::startup::lines(&problems).as_ref(), expected.as_bytes());
    }
}
#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary tests inspect diagnostic text")]
fn origin_inventory_keeps_configured_text_prefixes_and_forge_indices() {
    use crate::startup::checks;
    use jig_protocol_config::{Address, Directories};
    use temper_protocol_records as records;
    let document = fixtures::document();
    let input = super::startup::github_input(core::str::from_utf8(&document).unwrap());
    let mut forges = skein_lib::List::with_capacity(5);
    forges.push(crate::named(&document, &Directories::working()).unwrap().forges.into_vec().pop().unwrap()).unwrap();
    for (api, length) in [
        (Address::Host { name: Box::from(b"api.github.com".as_slice()), port: 443 }, 20),
        (Address::Host { name: Box::from(b"forge.example".as_slice()), port: 12345 }, 21),
        (Address::Addr(skein_io::Addr::Ip("127.0.0.1:3000".parse().unwrap())), 16),
        (Address::Addr(skein_io::Addr::Ip("[::1]:3000".parse().unwrap())), 12),
    ] {
        let mut forge =
            crate::named(input.as_bytes(), &Directories::working()).unwrap().forges.into_vec().pop().unwrap();
        forge.api = api;
        let encoded =
            records::encode(&records::origin(skein_forge_connection::settings::Kind::GitHub, &forge.api).encode());
        assert_eq!(encoded.len(), length);
        assert_eq!(&encoded[..2], &[records::VERSION, 2]);
        let index = usize::try_from(forges.len()).unwrap();
        forges.push(forge).unwrap();
        let inventory = checks::record_writes(&fixtures::engine().limits, forges.as_slice());
        assert_eq!(inventory.len(), forges.len() + 2);
        assert_eq!(
            *inventory.get(u32::try_from(index + 2).unwrap()).unwrap(),
            (Some(u64::try_from(length).unwrap()), b"Origin".as_slice(), Some(index))
        );
    }
}
#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary tests inspect diagnostic text")]
fn github_component_checks_key_deadline_and_renewal() {
    for (name, value, expected) in [
        (b"forge_component.key_bytes".as_slice(), 1195, "forge_component.key_bytes"),
        (b"forge_component.deadline_ms", 271_000, "forge_component.deadline_ms"),
        (b"forge_component.renew_before_ms", 19_999, "forge_component.renew_before_ms"),
        (b"forge_component.renew_before_ms", 1_800_001, "forge_component.renew_before_ms"),
        (b"forge_component.installations", 0, "forge_component"),
    ] {
        let mut config = fixtures::engine();
        config.forges[0].kind = crate::settings::Kind::GitHub;
        config.forges[0].api_token = None;
        limits::apply(&mut config.limits, name, value).unwrap();
        if name == b"forge_component.deadline_ms" {
            config.limits.forge.client.lifetime = Duration::from_secs(300);
        }
        let Err(found) = crate::forge::prepare(&config, fixtures::forge_runtime()) else { panic!("{expected}") };
        let found = crate::startup::Problems::jig(found, &jig_protocol_config::Path::root());
        let lines = crate::startup::lines(&found);
        assert!(core::str::from_utf8(&lines).unwrap().contains(expected), "{expected}: {lines:?}");
    }
}

#[test]
fn installation_capacity_fits_the_client_row_at_64_and_refuses_65() {
    // client/README.md, 4.6: the row carries at most 64 installations.
    const ROW_INSTALLATIONS: u32 = 64;
    let jig = fixtures::engine().jig;
    for (installations, accepted) in [(ROW_INSTALLATIONS, true), (ROW_INSTALLATIONS + 1, false)] {
        let mut own = limits::profile(limits::Profile::Small);
        own.forges.installations = installations;
        own.forge_component.installations = installations;
        let mut problems = crate::startup::Problems::empty();
        crate::startup::checks::records(&jig, &own, &[], &mut problems);
        assert_eq!(problems.is_empty(), accepted);
    }
}

#[test]
fn picker_page_has_its_own_override_and_startup_bounds() {
    let mut settings = fixtures::engine();
    assert_eq!(settings.limits.picker.page, 30);
    for page in [0_u64, 1, 30, 100, 101] {
        let mut own = settings.limits;
        limits::apply(&mut own, b"picker.page", page).unwrap();
        assert_eq!(u64::from(own.picker.page), page);
        let mut found = crate::startup::Problems::empty();
        crate::startup::checks::records(&settings.jig, &own, &[], &mut found);
        let lines = crate::startup::lines(&found);
        assert_eq!(skein_lib::bytes::find(&lines, b"picker.page").is_some(), page == 0 || page > 100);
    }
    assert_eq!(limits::apply(&mut settings.limits, b"picker.page", u64::MAX), Err(limits::Refused::Range));
}

#[test]
fn effective_core_limits_preserve_each_configured_policy_budget() {
    let own = limits::profile(limits::Profile::Small);
    for configured in [16_384, 512, 4096] {
        let mut jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
        jig.core.policy_bytes = configured;
        let root = limits::engine(&jig, &own).expect("configured policy budget fits the root");
        assert_eq!(temper_engine_domain::core_limits(&root).policy_bytes, configured);
    }
}
