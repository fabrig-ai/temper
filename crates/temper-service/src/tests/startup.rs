// Fixture parsing without a deployment keeps written paths.
#![expect(clippy::disallowed_macros, reason = "ordinary tests format configuration fixtures")]
#![expect(clippy::disallowed_methods, reason = "ordinary test code edits UTF-8 configuration fixtures")]
use super::fixtures;
use crate::settings;
use crate::startup::{Problems, What, application, lines};
use jig_protocol_config::{self as config, Path, Segment};
use skein_lib::List;

const REFUSED_TEMPLATE: u32 = 7;

fn transcript_document(bytes: u32) -> Box<[u8]> {
    let document = fixtures::document();
    let document = core::str::from_utf8(&document).unwrap();
    let previous = fixtures::startup_agent().limits.bodies.turn_body;
    let needle = format!("\"root.loads.transcript_bytes\":{previous}");
    assert_eq!(document.matches(&needle).count(), 1);
    document.replace(&needle, &format!("\"root.loads.transcript_bytes\":{bytes}")).into_bytes().into_boxed_slice()
}

#[test]
fn startup_admits_a_frame_exactly_at_the_profile_page_ceiling() {
    let document = transcript_document(1_590_046);
    crate::startup(
        &document,
        &config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
    .expect("a whole frame exactly at Small's page ceiling starts");
    let opened = crate::store_limits(&document, &config::Directories::working()).unwrap();
    assert_eq!(opened.store.store.page.bytes, 1_590_110);
}

#[test]
fn startup_names_a_frame_one_byte_past_the_profile_page_ceiling() {
    let document = transcript_document(1_590_047);
    let expected = b"application.limits.overrides.root.loads.transcript_bytes: its turn's transcript frame, 1590111 bytes, needs a store page of 1590111 bytes, past the profile's ceiling of 1590110 bytes (transcript bound 1590047 bytes)\n";
    let named = crate::named(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(lines(&named).as_ref(), expected);
    let opened = crate::store_limits(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(lines(&opened).as_ref(), expected);
}

#[test]
fn startup_names_a_transcript_past_the_codec_ceiling() {
    let document = transcript_document(8_388_609);
    let found = crate::named(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(
        lines(&found).as_ref(),
        b"application.limits.overrides.root.loads.transcript_bytes: 8388609 bytes, past the transcript codec's ceiling of 8388608\n"
    );
}

#[test]
fn startup_names_map_sizing_without_inventing_a_records_bound() {
    let mut engine = fixtures::engine();
    engine.jig.limits.store.store.key = 0;
    let mut problems = Problems::empty();
    crate::startup::checks::records(&engine.jig, &engine.limits, &[], &mut problems);
    assert_eq!(problems.problems.len(), 1);
    let What::StoreMap(jig_protocol_store::sizes::SizeRefusal::Key { needs, has: 0 }) =
        problems.problems.get(0).unwrap().what
    else {
        panic!("the map's key admission is the only cause: {problems:?}");
    };
    assert!(needs > 0);
    assert!(lines(&problems).starts_with(b"engine.limits.store: the store's map key, "));
}

#[test]
fn startup_checks_the_effective_codec_envelope_before_generic_sizing() {
    let jig = config::profile(config::Profile::Small);
    let mut own = crate::limits::profile(crate::limits::Profile::Small);
    // Parsed core limits remain valid. Deliberately corrupt the effective root
    // answer bound to exercise this check independently of parsed admission;
    // journal capacity no longer widens the configured answer bound (R3-114).
    own.root.journal.transcript_bytes = u32::MAX;
    let mut root = crate::limits::engine_candidate(&jig, &own);
    root.call_answer_bytes = u32::MAX;
    let expected = config::check_effective_limits(&temper_engine_domain::core_limits(&root))
        .expect_err("effective answer exceeds the codec");
    let mut problems = Problems::empty();
    crate::startup::checks::engine_envelope(&root, &mut problems);
    assert_eq!(problems.problems.len(), 1);
    let message = expected.codec_message().expect("named codec diagnostic");
    let mut expected_line = skein_lib::Writer::new(b"engine.".len() + message.len() + b"\n".len());
    expected_line.put(b"engine.").unwrap();
    expected_line.put(&message).unwrap();
    expected_line.put(b"\n").unwrap();
    assert_eq!(lines(&problems), expected_line.finish());
}

#[test]
fn startup_checks_a_single_policy_row_at_its_bound_and_one_byte_past_it() {
    let jig = config::profile(config::Profile::Small);
    let own = crate::limits::profile(crate::limits::Profile::Small);
    let mut root = crate::limits::engine_candidate(&jig, &own);
    let bound = temper_engine_domain::policy_row_bound(&root).expect("policy row bound");
    for (capacity, expected) in [
        (bound, b"".as_slice()),
        (
            bound - 1,
            b"application.limits.overrides.root.journal.transcript_bytes: must agree with engine.limits.core.policy_bytes plus its policy row carrier\n",
        ),
    ] {
        root.journal.transcript_bytes = u32::try_from(capacity).unwrap();
        let mut problems = Problems::empty();
        crate::startup::checks::engine_envelope(&root, &mut problems);
        assert_eq!(lines(&problems).as_ref(), expected);
    }
}

#[test]
fn startup_refuses_policy_row_arithmetic_overflow_by_name() {
    let jig = config::profile(config::Profile::Small);
    let own = crate::limits::profile(crate::limits::Profile::Small);
    let mut root = crate::limits::engine_candidate(&jig, &own);
    root.policy_bytes = u64::MAX;
    assert_eq!(temper_engine_domain::policy_row_bound(&root), None);
    let mut problems = Problems::empty();
    crate::startup::checks::engine_envelope(&root, &mut problems);
    assert!(lines(&problems).ends_with(
        b"application.limits.overrides.root.journal.transcript_bytes: must agree with engine.limits.core.policy_bytes plus its policy row carrier\n"
    ));
}

#[test]
fn startup_names_the_policy_row_before_generic_worst_case_refusal() {
    let document = fixtures::with(
        &fixtures::document(),
        b"application.limits.overrides",
        br#"{"root.journal.transcript_bytes":1}"#,
    );
    let found = crate::store_limits(&document, &config::Directories::working()).expect_err("short policy row");
    assert_eq!(
        lines(&found).as_ref(),
        b"application.limits.overrides.root.journal.transcript_bytes: must agree with engine.limits.core.policy_bytes plus its policy row carrier\n"
    );
}

#[test]
fn the_template_shaped_demo_reads_into_its_public_fields() {
    let (members, problems) = application(&fixtures::demo_application(), &fixtures::engine().jig);
    assert_eq!(&*lines(&problems), b"");
    let demo = members.unwrap().demo.unwrap();
    assert_eq!((demo.project, demo.forge, demo.provider), (1, 1, 3));
    assert_eq!((demo.owner.as_ref(), demo.name.as_ref()), (b"demo".as_slice(), b"widget".as_slice()));
    assert_eq!(
        demo.cues.as_ref(),
        &[
            Box::from(b"Add CSV export to widget".as_slice()),
            Box::from(b"Add a dark theme to widget".as_slice()),
            Box::from(b"Fix widget's empty search state".as_slice()),
        ]
    );
}
#[test]
fn a_demo_without_cued_publishes_all_asked() {
    // Restore the legacy document: its coding script was also suggested.
    let input = fixtures::demo_application();
    let input = core::str::from_utf8(&input).unwrap().replace("\"Search\"]}],\n\"cued\":[", "\"Search\"]},");
    let (members, problems) = application(input.as_bytes(), &fixtures::engine().jig);
    assert_eq!(&*lines(&problems), b"");
    let cues = members.unwrap().demo.unwrap().cues;
    assert_eq!(cues.len(), 4);
    assert_eq!(cues[3].as_ref(), b"Export widget's table as CSV");
}
#[test]
fn a_null_demo_reads_as_none() {
    let (members, problems) = application(fixtures::APPLICATION, &fixtures::engine().jig);
    assert_eq!(&*lines(&problems), b"");
    assert!(members.unwrap().demo.is_none());
}
#[test]
fn an_unanswerable_demo_is_refused_at_its_field() {
    use temper_client_protocol_wire::v1::CEILINGS;
    let input = fixtures::demo_application();
    let input = core::str::from_utf8(&input).unwrap();
    for (row, from, to, expected) in [
        (
            "missing forge",
            "\"forge\":1,\"project\":1",
            "\"forge\":2,\"project\":1".into(),
            "application.demo.forge: unknown reference\n",
        ),
        (
            "missing setup forge",
            "\"setup\":[{\"forge\":1",
            "\"setup\":[{\"forge\":2".into(),
            "application.demo.forge: unknown reference\n",
        ),
        ("not fake", "\"fake\":{", "\"managed\":{".into(), "application.demo.forge: invalid form\n"),
        (
            "no repository",
            "[{\"name\":\"widget\",\"landing\":\"main\"},{\"name\":\"other\",\"landing\":\"main\"}]",
            "[]".into(),
            "application.setup[0].mode.fake.repositories: invalid form\n",
        ),
        (
            "cue count at bound",
            "\"asked\":[",
            ["\"asked\":[", &"{\"cue\":\"extra\",\"turns\":[\"x\"]},".repeat(5)].concat(),
            "",
        ),
        (
            "cue count past bound",
            "\"asked\":[",
            ["\"asked\":[", &"{\"cue\":\"extra\",\"turns\":[\"x\"]},".repeat(6)].concat(),
            "application.demo.llm.asked: past its bound\n",
        ),
        (
            "cue at bound",
            "Add CSV export to widget",
            "x".repeat(usize::try_from(CEILINGS.demo_answer_cues_item).unwrap()),
            "",
        ),
        (
            "cue past bound",
            "Add CSV export to widget",
            "x".repeat(usize::try_from(CEILINGS.demo_answer_cues_item + 1).unwrap()),
            "application.demo.llm.asked[0].cue: past its bound\n",
        ),
        (
            "owner at bound",
            "\"owner\":\"demo\"",
            ["\"owner\":\"", &"x".repeat(usize::try_from(CEILINGS.demo_answer_owner).unwrap()), "\""].concat(),
            "",
        ),
        (
            "owner past bound",
            "\"owner\":\"demo\"",
            ["\"owner\":\"", &"x".repeat(usize::try_from(CEILINGS.demo_answer_owner + 1).unwrap()), "\""].concat(),
            "application.setup[0].mode.fake.owner: past its bound\n",
        ),
        (
            "name at bound",
            "\"name\":\"widget\"",
            ["\"name\":\"", &"x".repeat(usize::try_from(CEILINGS.demo_answer_name).unwrap()), "\""].concat(),
            "",
        ),
        (
            "name past bound",
            "\"name\":\"widget\"",
            ["\"name\":\"", &"x".repeat(usize::try_from(CEILINGS.demo_answer_name + 1).unwrap()), "\""].concat(),
            "application.setup[0].mode.fake.repositories[0].name: past its bound\n",
        ),
    ] {
        let input = input.replace(from, &to);
        let (members, problems) = application(input.as_bytes(), &fixtures::engine().jig);
        assert_eq!(members.is_some(), expected.is_empty(), "{row}");
        assert_eq!(&*lines(&problems), expected.as_bytes(), "{row}");
    }
}

#[test]
fn the_clients_listener_is_read_from_the_application_member() {
    let jig = fixtures::engine().jig;
    let (members, problems) = application(fixtures::APPLICATION, &jig);
    assert_eq!(&*lines(&problems), b"", "example accepted");
    let config::Address::Addr(skein_io::Addr::Ip(listener)) = members.unwrap().clients.listener else {
        panic!("a socket address")
    };
    assert_eq!(listener, "127.0.0.1:7100".parse::<core::net::SocketAddr>().unwrap());
}
#[test]
fn a_missing_clients_member_is_named_by_its_path() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    let input = input
        .replace("\"clients\":{\"listener\":{\"socket\":\"127.0.0.1:7100\"},\"public\":null,\"bundle\":null},", "");
    let (members, problems) = application(input.as_bytes(), &jig);
    assert!(members.is_none());
    assert_eq!(&*lines(&problems), b"application.clients: missing\n");
}
#[test]
fn an_in_process_clients_listener_is_refused_as_a_form() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    let input = input.replace("\"socket\":\"127.0.0.1:7100\"", "\"in_process\":\"clients\"");
    let (members, problems) = application(input.as_bytes(), &jig);
    assert!(members.is_none());
    assert_eq!(&*lines(&problems), b"application.clients.listener: invalid form\n");
}

fn application(input: &[u8], jig: &config::Engine) -> (Option<application::Members>, Problems) {
    let document =
        config::document(input, &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 }).unwrap();
    let mut problems = Problems::empty();
    let members = application::parse(&document, 0, jig, &config::Directories::working(), &mut problems);
    (members, problems)
}
#[test]
fn the_standalone_example_reads_into_its_members() {
    let jig = fixtures::engine().jig;
    let (members, problems) = application(fixtures::APPLICATION, &jig);
    assert_eq!(&*lines(&problems), b"", "example accepted");
    let members = members.unwrap();
    assert_eq!(members.forges[0].user, application::User::File(b"secrets/forge-1.user".as_slice().into()));
    assert_eq!(members.forges[0].git.as_ref(), b"http://127.0.0.1:9");
    assert_eq!(members.identities[0].author.name.as_ref(), b"Temper");
    assert_eq!(members.workers[0].git.as_deref(), Some(b"/usr/bin/git".as_slice()));
    assert_eq!(members.charter, 1);
    assert_eq!(application::files(&members, 4096, 8192).len(), 3, "all private files");
    let resolved = application::resolve(&members, &fixtures::files(), &mut Problems::empty()).unwrap();
    assert_eq!(resolved.forges[0].api_token.as_ref().unwrap().expose(), b"api-token");
    assert_eq!(resolved.identities[0].token.expose(), b"git-token");
    assert!(!fixtures::document().is_empty(), "complete example bytes");
    let document = fixtures::parsed();
    assert!(!document.is_empty(), "full standalone example is JSON");
}

fn checked_fake_setup() -> temper_forge_setup::check::Checked {
    checked_local_setup(false)
}
fn checked_local_setup(records: bool) -> temper_forge_setup::check::Checked {
    use temper_forge_setup::check;
    let input = br#"[{"forge":1,"directory":"demo/forge","bot":{"login":"temper","email":"temper@example.test"},
        "mode":{"fake":{"listen":{"socket":"127.0.0.1:9"},"git":"demo/bin/git","owner":"demo",
        "repositories":[{"name":"widget","landing":"main"}]}}}]"#;
    let document = config::document(input, &config::DocumentLimits { bytes: 65_536, tokens: 1024, depth: 32 }).unwrap();
    let mut members = check::Members {
        forges: Box::new([check::Forge {
            number: 1,
            api: Some("127.0.0.1:9".parse().unwrap()),
            plaintext: true,
            git: b"http://127.0.0.1:9".as_slice().into(),
            user: check::User::File(b"/data/demo/forge/user".as_slice().into()),
            api_token: b"/data/demo/forge/api_token".as_slice().into(),
            webhook_secret: None,
            identity_token: b"/data/demo/forge/git_token".as_slice().into(),
            identity_login: b"temper".as_slice().into(),
        }]),
        listeners: Box::new([]),
    };
    if records {
        members.forges[0].api_token = Box::new([]);
        members.forges[0].user = check::User::Id(0);
        members.forges[0].identity_token = Box::new([]);
    }
    check::checked(
        &document,
        0,
        &config::Directories::given(b"/config", b"/data", b"/cache").unwrap(),
        &members,
        None,
        &[],
    )
    .unwrap()
}

fn with_forge_git_program(input: &[u8], value: &str) -> Box<[u8]> {
    core::str::from_utf8(input)
        .unwrap()
        .replace("\"forges\":[{", &format!("\"forges\":[{{\"git_program\":{value},"))
        .into_bytes()
        .into_boxed_slice()
}

#[test]
fn forge_git_program_reads_data_and_absolute_paths_and_refuses_bad_values() {
    let jig = fixtures::engine().jig;
    let directories = config::Directories::given(b"/config", b"/data", b"/cache").unwrap();
    for (value, expected) in [(r#""bin/git""#, b"/data/bin/git".as_slice()), (r#""/opt/git""#, b"/opt/git".as_slice())]
    {
        let input = with_forge_git_program(fixtures::APPLICATION, value);
        let document =
            config::document(&input, &config::DocumentLimits { bytes: 65_536, tokens: 4096, depth: 32 }).unwrap();
        let mut problems = Problems::empty();
        let members = application::parse(&document, 0, &jig, &directories, &mut problems).unwrap();
        assert_eq!(lines(&problems).as_ref(), b"");
        assert_eq!(members.forges[0].git_program.as_deref(), Some(expected));
    }
    for value in ["null", "false", r#""""#, r#""../git""#, r#""bad\u0000path""#] {
        let input = with_forge_git_program(fixtures::APPLICATION, value);
        let (_, problems) = application(&input, &jig);
        assert!(lines(&problems).starts_with(b"application.forges[0].git_program:"), "{value}: {:?}", lines(&problems));
    }
}

#[test]
fn the_forges_git_program_reaches_startup_without_fake_setup() {
    let input = with_forge_git_program(&fixtures::document(), r#""/opt/custom/git""#);
    let service = start_with_git_program(&input, None).unwrap();
    assert_eq!(
        service.engine.as_ref().unwrap().workers.forges[0].git_program.as_deref(),
        Some(b"/opt/custom/git".as_slice())
    );
}

#[test]
fn startup_refuses_both_sources_of_a_forges_git_program() {
    let input = with_forge_git_program(&fixtures::document(), r#""/opt/custom/git""#);
    let problems = start_with_git_program(&input, Some(&checked_fake_setup())).err().expect("two sources refused");
    let paths: Box<[_]> = problems.problems.iter().map(|problem| problem.path.clone()).collect();
    assert_eq!(
        paths.as_ref(),
        &[
            Path::root().member(b"application").member(b"forges").child(Segment::Index(0)).member(b"git_program"),
            Path::root()
                .member(b"application")
                .member(b"setup")
                .child(Segment::Index(0))
                .member(b"mode")
                .member(b"fake")
                .member(b"git"),
        ]
    );
}

#[test]
fn the_checked_fake_git_program_reaches_the_services_forge_settings() {
    let service = start_with_git_program(&fixtures::document(), Some(&checked_fake_setup())).unwrap();
    let forge = &service.engine.as_ref().expect("engine").workers.forges[0];
    assert_eq!(forge.git_program.as_deref(), Some(b"/data/demo/bin/git".as_slice()));
    assert_eq!(forge.git.as_ref(), b"http://127.0.0.1:9");
}

fn start_with_git_program(
    input: &[u8],
    checked: Option<&temper_forge_setup::check::Checked>,
) -> Result<crate::Service, Problems> {
    let mut files = fixtures::files();
    let mut entries = List::with_capacity(files.entries.len().checked_add(1).unwrap());
    for entry in files.entries.into_boxed() {
        entries.push(entry).unwrap();
    }
    files.entries = entries;
    files
        .entries
        .push((
            b"/data/demo/forge/admin_token".as_slice().into(),
            skein_lib::Secret::new(b"0123456789abcdef0123456789abcdef01234567".as_slice().into()),
        ))
        .unwrap();
    crate::startup::with_setup(
        input,
        &config::Directories::working(),
        &files,
        Some(&fixtures::startup_agent()),
        &fixtures::seeds(),
        None,
        Some(fixtures::worker_runtime()),
        fixtures::forge_runtime(),
        checked,
        false,
    )
}

#[test]
fn the_worker_channels_count_the_retained_fake_git_program() {
    let mut settings = fixtures::engine();
    let before = crate::workers::worst_case(&settings, 4).unwrap();
    let program = b"/data/demo/bin/git";
    settings.forges[0].git_program = Some(program.as_slice().into());
    assert_eq!(crate::workers::worst_case(&settings, 4).unwrap() - before, u64::try_from(program.len()).unwrap());
}
#[test]
fn the_example_engine_keeps_jigs_chat_and_goal_templates() {
    let jig = fixtures::engine().jig;
    let labels: Box<[u32]> = jig.templates.iter().map(|template| template.label).collect();
    assert_eq!(*labels, [1, 2], "jig's chat and goal templates, in order");
    assert_eq!(jig.templates[0].number, 1);
    assert_eq!(jig.templates[0].flags.ended_by, jig_domain_tasks::EndedBy::Requester);
    assert_eq!(jig.templates[1].flags.ended_by, jig_domain_tasks::EndedBy::Result);
}
#[test]
fn a_chats_member_is_refused_as_unknown_naming_its_path() {
    let input = fixtures::document();
    let input = core::str::from_utf8(&input).unwrap().replacen("\"templates\": [", "\"chats\": [", 1);
    let problems = crate::named(input.as_bytes(), &jig_protocol_config::Directories::working()).unwrap_err();
    assert_eq!(&*lines(&problems), b"engine.templates: missing\nengine.chats: unknown member\n");
}
fn refused(input: &[u8]) -> Box<[u8]> {
    let (members, problems) = application(input, &fixtures::engine().jig);
    assert!(members.is_none());
    lines(&problems)
}
#[test]
fn an_unknown_user_member_is_named_by_its_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION)
        .unwrap()
        .replace("\"user\":{\"file\":\"secrets/forge-1.user\"}", "\"usr\":42");
    assert_eq!(
        &*refused(input.as_bytes()),
        b"application.forges[0].user: missing\napplication.forges[0].usr: unknown member\n"
    );
}
#[test]
fn a_missing_user_member_is_named_by_its_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION)
        .unwrap()
        .replace(",\"user\":{\"file\":\"secrets/forge-1.user\"}", "");
    assert_eq!(&*refused(input.as_bytes()), b"application.forges[0].user: missing\n");
}
#[test]
fn an_unknown_profile_is_refused_naming_its_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace("\"small\"", "\"large\"");
    assert_eq!(&*refused(input.as_bytes()), b"application.limits.profile: the only profile is \"small\"\n");
}
#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary tests format input and diagnostics")]
fn an_override_by_dotted_name_is_set_or_refused() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    for (override_value, expected) in [
        (
            "\"overrides\":{\"forge.repositories\":3,\"forges.log_origins\":3,\"forge_component.log_origins\":7,\"forge_component.log_poll_ms\":7123,\"forge_component.log_catch_up_ms\":9321,\"forge_component.log_pages\":13,\"forge_component.delivery_bytes\":12345}",
            "",
        ),
        (
            "\"overrides\":{\"forge.nothing\":3}",
            "application.limits.overrides.forge.nothing: no limit of temper's has this name\n",
        ),
        (
            "\"overrides\":{\"checkout.name_bytes\":4294967296}",
            "application.limits.overrides.checkout.name_bytes: outside its type\n",
        ),
    ] {
        let (members, problems) = application(input.replace("\"overrides\":{}", override_value).as_bytes(), &jig);
        assert_eq!(&*lines(&problems), expected.as_bytes(), "row {override_value}");
        if let Some(members) = members {
            assert_eq!(members.limits.forge.repositories, 3, "set {override_value}");
            assert_eq!((members.limits.forges.log_origins, members.limits.forge_component.log_origins), (3, 7));
            assert_eq!(members.limits.forge_component.log_poll, skein_lib::Duration::from_millis(7123));
            assert_eq!(members.limits.forge_component.log_catch_up, skein_lib::Duration::from_millis(9321));
            assert_eq!(members.limits.forge_component.log_pages, 13);
            assert_eq!(members.limits.forge_component.delivery_bytes, 12_345);
        }
    }
    for (name, value) in [
        ("forges.log_origins", u64::from(u32::MAX) + 1),
        ("forge_component.log_origins", u64::from(u32::MAX) + 1),
        ("forge_component.log_poll_ms", u64::MAX / 1_000_000 + 1),
        ("forge_component.log_catch_up_ms", u64::MAX / 1_000_000 + 1),
        ("forge_component.log_pages", u64::from(u32::MAX) + 1),
        ("forge_component.delivery_bytes", u64::from(u32::MAX) + 1),
    ] {
        let override_value = format!("\"overrides\":{{\"{name}\":{value}}}");
        let (_, problems) = application(input.replace("\"overrides\":{}", &override_value).as_bytes(), &jig);
        assert_eq!(
            lines(&problems).as_ref(),
            format!("application.limits.overrides.{name}: outside its type\n").as_bytes()
        );
    }
}
fn application_with_landing_requirements() -> Box<[u8]> {
    let input = fixtures::with(
        fixtures::APPLICATION,
        b"landing.projects[0].rules[0].gates",
        br#"[{"number":3,"blocking":true,"freshness":"exact"}]"#,
    );
    fixtures::with(&input, b"landing.projects[0].rules[0].approvals", br#"[{"role":0,"people":1,"freshness":"clean"}]"#)
}
#[test]
fn a_landing_rule_reads_jigs_pattern_form() {
    let jig = fixtures::engine().jig;
    let (members, problems) = application(&application_with_landing_requirements(), &jig);
    assert_eq!(&*lines(&problems), b"", "landing accepted");
    let members = members.unwrap();
    let rule = &members.landing.projects.get(&1).unwrap()[0];
    assert_eq!(rule.pattern.segments[0].as_ref(), b"forge");
    assert_eq!(rule.pattern.last, jig_domain_authority::Last::Open(b"main".as_slice().into()));
    assert_eq!(rule.gates[0].number, 3);
    assert_eq!(rule.approvals[0].role, 0);
    assert_eq!(rule.approvals[0].freshness, temper_engine_domain::Freshness::Clean);
}
#[test]
fn a_git_base_with_a_user_or_a_final_slash_is_refused() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    for (git, expected) in [
        ("http://user@127.0.0.1:9", "application.forges[0].git: invalid form\n"),
        ("http://127.0.0.1:9/", "application.forges[0].git: invalid form\n"),
        ("http://forge.example", "application.forges[0].git: invalid form\n"),
        ("https://forge.example?token=1", "application.forges[0].git: invalid form\n"),
    ] {
        let (members, problems) = application(input.replace("http://127.0.0.1:9", git).as_bytes(), &jig);
        assert!(members.is_none(), "refused {git}");
        assert_eq!(&*lines(&problems), expected.as_bytes(), "row {git}");
    }
}
#[test]
fn problems_print_one_line_each_with_their_path() {
    let mut problems = Problems::empty();
    let forge = Path::root().member(b"application").member(b"forges").child(Segment::Index(0));
    problems.push(forge.member(b"user"), What::Jig(config::What::Missing));
    problems.push(Path::root().member(b"application").member(b"webhooks"), What::NoWebhooks);
    problems
        .push(Path::root().member(b"engine").member(b"memory_bytes"), What::Jig(config::What::Memory { over: 4096 }));
    assert_eq!(&*lines(&problems), b"application.forges[0].user: missing\napplication.webhooks: needs application.webhooks\nengine.memory_bytes: the worst case passes memory_bytes by 4096 bytes\n".as_slice());
    for _ in 0..config::PROBLEMS {
        problems.push(Path::root(), What::NoWebhooks);
    }
    assert_eq!(problems.past, 3);
    assert!(lines(&problems).ends_with(b"and 3 more\n"), "capped diagnostics name overflow");
}
#[test]
fn jig_refusals_print_words() {
    for (what, expected) in [
        (config::What::Json, b"at: invalid JSON\n".as_slice()),
        (config::What::Unknown, b"at: unknown member\n"),
        (config::What::Missing, b"at: missing\n"),
        (config::What::Duplicate, b"at: duplicate\n"),
        (config::What::Kind, b"at: wrong value kind\n"),
        (config::What::Bound, b"at: past its bound\n"),
        (config::What::Form, b"at: invalid form\n"),
        (config::What::Unique, b"at: must be unique\n"),
        (config::What::Refers, b"at: unknown reference\n"),
        (config::What::Plaintext, b"at: plaintext requires loopback or in-process\n"),
        (config::What::Digest, b"at: invalid digest\n"),
        (config::What::Reserve { charter: 7 }, b"at: reserve does not fit charter 7\n"),
        (config::What::Budget { charter: 7 }, b"at: budget does not fit charter 7\n"),
        (config::What::Grace { template: REFUSED_TEMPLATE }, b"at: grace does not fit template 7\n"),
        (
            config::What::Operations { operations: 4, delegates: 4, goals: 8 },
            b"at: operations do not fit compound requests: operations 4, delegates 4, goals 8\n",
        ),
        (config::What::Message, b"at: message bounds do not agree\n"),
        (config::What::Silence, b"at: heartbeat does not fit silence bound\n"),
        (config::What::StopBound, b"at: stop bound does not fit grace\n"),
        (config::What::Memory { over: 7 }, b"at: the worst case passes memory_bytes by 7 bytes\n"),
        (config::What::Override, b"at: invalid override\n"),
        (config::What::Unreadable, b"at: not read\n"),
        (config::What::Relative, b"at: relative path must be non-empty with no .. component\n"),
    ] {
        let mut problems = Problems::empty();
        problems.push(Path::root().member(b"at"), What::Jig(what));
        assert_eq!(&*lines(&problems), expected, "row {what:?}");
    }
}
#[test]
fn members_and_references_are_checked() {
    let jig = fixtures::engine().jig;
    let input = application_with_landing_requirements();
    let input = core::str::from_utf8(&input).unwrap();
    for (from, to, expected) in [
        ("\"charter\":1", "\"charter\":2", "application.charter: unknown reference\n"),
        ("\"kind\":4", "\"kind\":3", "application.landing.projects[0].rules[0].kind: invalid form\n"),
        ("\"role\":0", "\"role\":1", "application.landing.projects[0].rules[0].approvals[0].role: unknown reference\n"),
        ("\"project\":1", "\"project\":2", "application.landing.projects[0].project: unknown reference\n"),
        ("\"login\":\"temper\"", "\"login\":\"bad:login\"", "application.identities[0].login: invalid form\n"),
        ("\"name\":\"Temper\"", "\"name\":\"<Temper>\"", "application.identities[0].author.name: invalid form\n"),
        ("\"user\":{\"file\":\"secrets/forge-1.user\"}", "\"user\":0", "application.forges[0].user: invalid form\n"),
        (
            "\"user\":{\"file\":\"secrets/forge-1.user\"}",
            "\"user\":{\"file\":\"secrets/forge-1.user\"},\"user\":43",
            "application.forges[0].user: duplicate\n",
        ),
        ("\"git\":\"/usr/bin/git\"", "\"git\":\"git\"", "application.workers[0].git: must be an absolute path\n"),
        ("\"git\":\"/usr/bin/git\"", "\"git\":null", ""),
        (
            "\"transport\":\"plaintext\"",
            "\"transport\":{\"tls\":{\"server_name\":\"forge\"}}",
            "application.forges[0].transport.tls.trust: missing\n",
        ),
    ] {
        let (members, problems) = application(input.replace(from, to).as_bytes(), &jig);
        assert_eq!(members.is_some(), expected.is_empty(), "row {from}: {to}");
        assert_eq!(&*lines(&problems), expected.as_bytes(), "row {from}");
        if expected.is_empty() {
            assert_eq!(members.unwrap().workers[0].git, None);
        }
    }
}
#[test]
fn application_files_keep_paths_bounds_privacy_and_resolution_refusals() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace(
        "\"transport\":\"plaintext\"",
        "\"transport\":{\"tls\":{\"server_name\":\"forge\",\"trust\":\"root.der\"}}",
    );
    let (members, problems) = application(input.as_bytes(), &jig);
    assert_eq!(&*lines(&problems), b"", "TLS file form accepted");
    let members = members.unwrap();
    let files = application::files(&members, 4096, 8192);
    for (index, path, max, private) in [
        (0, b"secrets/forge-1.api-token".as_slice(), 4096, true),
        (1, b"secrets/forge-1.user".as_slice(), 21, true),
        (2, b"root.der".as_slice(), 1_048_576, false),
        (3, b"secrets/forge-1.git-token".as_slice(), 8192, true),
    ] {
        assert_eq!(files[index].path.as_ref(), path, "file {index}");
        assert_eq!(files[index].max, max, "bound {index}");
        assert_eq!(files[index].private, private, "privacy {index}");
    }
    let mut problems = Problems::empty();
    assert!(application::resolve(&members, &fixtures::files(), &mut problems).is_none(), "trust file must be read");
    assert_eq!(&*lines(&problems), b"application.forges[0].transport.tls.trust: not read\n".as_slice());
}
#[test]
fn named_lists_each_file_with_its_bound_and_privacy() {
    let named = crate::named(&fixtures::document(), &jig_protocol_config::Directories::working()).unwrap();
    assert_eq!(named.files.len(), 7, "all files listed");
    // The optional program carriers add 16 B to both RunPolicy and its model.
    // With the same run budget, this direct charter has 32 B less instruction room.
    let instruction_bytes = 16_070 - 2 * 16;
    for (name, path, max, private) in [
        (b"charters/chat.txt".as_slice(), b"engine.charters[0].instructions".as_slice(), instruction_bytes, false),
        (b"secrets/worker-a.digest", b"engine.hosts.workers[0].digest", 65, true),
        (b"worker-secret", b"workers[0].secret", 257, true),
        (fixtures::AGENT_JSON, b"workers[0].agent.configuration", 1_048_576, false),
        (b"secrets/forge-1.api-token", b"application.forges[0].api_token", 4096, true),
        (b"secrets/forge-1.git-token", b"application.identities[0].token", 8192, true),
    ] {
        let file = named.files.iter().find(|file| file.path.as_ref() == name).expect("listed file");
        assert_eq!(file.max, max, "bound {name:?}");
        assert_eq!(file.private, private, "privacy {name:?}");
        let mut problems = Problems::empty();
        problems.push(file.at.clone(), What::Jig(config::What::Missing));
        let expected = [path, b": missing\n"].concat();
        assert_eq!(&*lines(&problems), expected.as_slice(), "path {name:?}");
    }
}
#[test]
fn named_returns_the_worker_root_and_the_agent_configuration() {
    let named = crate::named(&fixtures::document(), &jig_protocol_config::Directories::working()).unwrap();
    assert_eq!(named.deployment.as_ref(), b"state");
    assert_eq!(named.worker.as_ref().expect("standalone worker").root.as_ref(), b"worker-root");
    assert_eq!(named.worker.as_ref().expect("standalone worker").agent.as_ref(), fixtures::AGENT_JSON);
}
#[test]
fn a_file_named_but_not_given_refuses_the_start_naming_it() {
    let mut files = fixtures::files();
    files.entries = fixtures::without(files.entries, b"secrets/forge-1.git-token");
    let refused = crate::startup(
        &fixtures::document(),
        &jig_protocol_config::Directories::working(),
        &files,
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    );
    let Err(problems) = refused else { panic!("a missing token refuses the start") };
    assert_eq!(&*lines(&problems), b"application.identities[0].token: not read\n".as_slice());
}
#[test]
fn jigs_problems_keep_their_paths_under_engine() {
    let input = fixtures::document();
    let input = core::str::from_utf8(&input)
        .unwrap()
        .replace("\"memory_bytes\": 1152921504606846976,", "\"memory_bytes\": \"bad\",");
    let problems = crate::named(input.as_bytes(), &jig_protocol_config::Directories::working()).unwrap_err();
    assert_eq!(&*lines(&problems), b"engine.memory_bytes: wrong value kind\n".as_slice());
}
#[test]
fn startup_makes_the_one_process_from_the_example() {
    let mut service = match crate::startup(
        &fixtures::document(),
        &jig_protocol_config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    ) {
        Ok(service) => service,
        Err(problems) => panic!("startup: {}", core::str::from_utf8(&lines(&problems)).unwrap()),
    };
    service.engine.as_mut().expect("engine").store = fixtures::memory_store();
    let mut stand = fixtures::system::SystemStand { service, kernel: fixtures::Kernel::new() };
    fixtures::until_hosting(&mut stand);
    let (channel, hello) = fixtures::hello(&stand.service).expect("worker linked through bytes");
    assert_eq!(channel, skein_lib::Token::new(1));
    assert_eq!(hello.slots, 1);
    for _ in 0_u16..256 {
        if stand.service.engine.as_mut().expect("engine").domain.ready() {
            break;
        }
        stand.turn();
    }
    assert!(stand.service.engine.as_ref().expect("engine").domain.ready(), "engine restored its store");
    assert_eq!(stand.service.end(), None);
}
#[test]
fn a_workers_missing_or_oversized_secret_keeps_its_absolute_path() {
    let document = fixtures::document();
    let agent = fixtures::startup_agent();
    let seeds = fixtures::seeds();
    for (row, bytes, expected) in [
        ("missing", None, b"workers[0].secret: not read\n".as_slice()),
        ("past", Some(Box::from([b'x'; 257])), b"workers[0].secret: past its bound\n"),
    ] {
        let mut files = fixtures::files();
        files.entries = fixtures::without(files.entries, b"worker-secret");
        if let Some(bytes) = bytes {
            files.entries.push((b"worker-secret".as_slice().into(), skein_lib::Secret::new(bytes))).unwrap();
        }
        let found = match crate::startup(
            &document,
            &jig_protocol_config::Directories::working(),
            &files,
            &agent,
            &seeds,
            None,
            fixtures::worker_runtime(),
            fixtures::forge_runtime(),
        ) {
            Ok(service) => {
                drop(service);
                Problems::empty()
            }
            Err(problems) => problems,
        };
        assert_eq!(&*lines(&found), expected, "row {row}");
    }
}
#[test]
fn a_worker_secret_at_its_bound_excludes_its_final_newline() {
    let mut worker = fixtures::worker_settings();
    worker.jig.limits.link.secret = 4;
    let mut entries = List::with_capacity(1);
    entries.push((b"worker-secret".as_slice().into(), skein_lib::Secret::new(b"four\n".as_slice().into()))).unwrap();
    let made = crate::Worker::new(
        &worker,
        &fixtures::worker_agent(),
        &fixtures::worker_limits(),
        &settings::Files { entries },
        fixtures::worker_runtime(),
        7,
    );
    assert!(made.is_ok(), "four secret bytes plus their final newline fit four");
}

#[test]
fn a_user_file_member_reads_as_a_private_file_of_21_bytes() {
    let (members, problems) = application(fixtures::APPLICATION, &fixtures::engine().jig);
    assert_eq!(&*lines(&problems), b"");
    let files = application::files(&members.unwrap(), 4096, 8192);
    let file = files.iter().find(|file| file.path.as_ref() == b"secrets/forge-1.user").unwrap();
    assert_eq!(file.max, 21);
    assert!(file.private);
    let mut problems = Problems::empty();
    problems.push(file.at.clone(), What::Jig(config::What::Missing));
    assert_eq!(&*lines(&problems), b"application.forges[0].user.file: missing\n");
}
#[test]
fn a_user_file_resolves_to_the_decimal_id_it_holds() {
    let (members, _) = application(fixtures::APPLICATION, &fixtures::engine().jig);
    let mut problems = Problems::empty();
    let resolved = application::resolve(&members.unwrap(), &fixtures::files(), &mut problems).unwrap();
    assert_eq!(resolved.forges[0].user, 42);
    assert_eq!(&*lines(&problems), b"");
}
#[test]
fn a_user_file_holding_no_positive_decimal_is_refused_by_path() {
    let (members, _) = application(fixtures::APPLICATION, &fixtures::engine().jig);
    let members = members.unwrap();
    for value in
        [b"".as_slice(), b"0", b"042", b"-1", b"4 2", b"42\n\n", b"123456789012345678901", b"18446744073709551616"]
    {
        let mut files = fixtures::files();
        files.entries = fixtures::without(files.entries, b"secrets/forge-1.user");
        files.entries.push((b"secrets/forge-1.user".as_slice().into(), skein_lib::Secret::new(value.into()))).unwrap();
        let mut problems = Problems::empty();
        assert!(application::resolve(&members, &files, &mut problems).is_none(), "{value:?}");
        assert_eq!(&*lines(&problems), b"application.forges[0].user.file: invalid form\n", "{value:?}");
    }
}
fn refused_user(value: &str) -> Box<[u8]> {
    let input =
        core::str::from_utf8(fixtures::APPLICATION).unwrap().replace("{\"file\":\"secrets/forge-1.user\"}", value);
    let (members, problems) = application(input.as_bytes(), &fixtures::engine().jig);
    assert!(members.is_none());
    lines(&problems)
}
#[test]
fn a_user_object_with_a_missing_file_is_refused() {
    assert_eq!(&*refused_user("{}"), b"application.forges[0].user.file: missing\n");
}
#[test]
fn a_user_object_with_a_stray_member_is_refused() {
    assert_eq!(&*refused_user("{\"file\":\"x\",\"stray\":1}"), b"application.forges[0].user.stray: unknown member\n");
}
#[test]
fn a_user_object_with_an_empty_path_is_refused() {
    assert_eq!(&*refused_user("{\"file\":\"\"}"), b"application.forges[0].user.file: invalid form\n");
}
#[test]
fn a_missing_setup_or_demo_is_refused_naming_its_path() {
    for (member, expected) in [
        ("\"setup\":[],", b"application.setup: missing\n".as_slice()),
        ("\"demo\":null,", b"application.demo: missing\n".as_slice()),
    ] {
        let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace(member, "");
        let (members, problems) = application(input.as_bytes(), &fixtures::engine().jig);
        assert!(members.is_none());
        assert_eq!(&*lines(&problems), expected);
    }
}

#[test]
fn a_setup_or_box_override_is_handed_on_with_its_value() {
    for (override_value, repositories) in [("", 2_u64), (",\"forge.client.repositories\":3", 3)] {
        let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace(
            "\"overrides\":{}",
            &["\"overrides\":{\"setup.orgs\":2,\"box.steps\":3", override_value, "}"].concat(),
        );
        let (members, problems) = application(input.as_bytes(), &fixtures::engine().jig);
        assert_eq!(&*lines(&problems), b"");
        assert_eq!(
            members.unwrap().overrides.as_ref(),
            &[
                (b"setup.orgs".as_slice().into(), 2),
                (b"box.steps".as_slice().into(), 3),
                (b"forge.client.repositories".as_slice().into(), repositories),
            ]
        );
    }
}

#[test]
fn the_webhook_listener_and_secret_file_are_read() {
    let named = crate::named(&fixtures::webhook_document(), &jig_protocol_config::Directories::working()).unwrap();
    let webhooks = named.webhooks.unwrap();
    let config::Address::Addr(listener) = webhooks.listener else { panic!("socket") };
    assert_eq!(listener.socket().unwrap(), "127.0.0.1:7200".parse().unwrap());
    let file = named.files.iter().find(|file| file.path.as_ref() == b"secrets/forge-1.webhook-secret").unwrap();
    assert_eq!(file.max, 128);
    assert!(file.private);
    let mut problems = Problems::empty();
    problems.push(file.at.clone(), What::Jig(config::What::Missing));
    assert_eq!(&*lines(&problems), b"application.forges[0].webhook_secret: missing\n");
}
// protocol/sign-in.md, 9: a configured provider is admitted, and startup reads its key and
// client secret as private files, refusing either one missing by its path.
#[test]
fn a_clients_provider_key_and_secret_are_private_files_named_by_path() {
    let document = core::str::from_utf8(&fixtures::document()).unwrap().replace(
        "\"clients\": {",
        r#""clients": {"provider":{"key":"provider.pem","clients":[{"id":"app","secret":"app.secret","redirect_uri":"https://app.example.com/cb"}],"addresses":[]},"#,
    );
    let named = crate::named(document.as_bytes(), &jig_protocol_config::Directories::working()).unwrap();
    for (name, expected) in [
        (b"provider.pem".as_slice(), b"engine.clients.provider.key: missing\n".as_slice()),
        (b"app.secret", b"engine.clients.provider.clients[0].secret: missing\n"),
    ] {
        let file = named.files.iter().find(|file| file.path.as_ref() == name).unwrap();
        assert!(file.private);
        let mut problems = Problems::empty();
        problems.push(file.at.clone(), What::Jig(config::What::Missing));
        assert_eq!(&*lines(&problems), expected);
    }
}
#[test]
fn null_webhook_members_are_read_as_none() {
    let (members, problems) = application(fixtures::APPLICATION, &fixtures::engine().jig);
    assert_eq!(&*lines(&problems), b"");
    let members = members.unwrap();
    assert!(members.webhooks.is_none());
    assert!(members.forges[0].webhook_secret.is_none());
}
#[test]
fn a_missing_webhooks_member_is_refused_by_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace("\"webhooks\":null,", "");
    assert_eq!(&*refused(input.as_bytes()), b"application.webhooks: missing\n");
}
#[test]
fn a_missing_webhook_secret_is_refused_by_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace("\"webhook_secret\":null,", "");
    assert_eq!(&*refused(input.as_bytes()), b"application.forges[0].webhook_secret: missing\n");
}
#[test]
fn an_in_process_webhook_listener_is_refused() {
    let input =
        fixtures::with(&fixtures::document(), b"application.webhooks", br#"{"listener":{"in_process":"hooks"}}"#);
    assert_eq!(
        &*lines(&crate::named(&input, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"application.webhooks.listener: invalid form\n"
    );
}
#[test]
fn the_webhook_secret_excludes_its_final_newline() {
    let input = core::str::from_utf8(fixtures::APPLICATION)
        .unwrap()
        .replace("\"webhooks\":null", "\"webhooks\":{\"listener\":{\"socket\":\"127.0.0.1:7200\"}}")
        .replace("\"webhook_secret\":null", "\"webhook_secret\":\"secrets/forge-1.webhook-secret\"");
    let (members, _) = application(input.as_bytes(), &fixtures::engine().jig);
    let resolved = application::resolve(&members.unwrap(), &fixtures::files(), &mut Problems::empty()).unwrap();
    assert_eq!(resolved.forges[0].webhook_secret.as_ref().unwrap().expose(), b"fixture-hook-secret");
}
#[test]
fn the_configured_bundle_is_named_for_the_binary() {
    let document = fixtures::with(&fixtures::document(), b"application.clients.bundle", br#""/bundle""#);
    let named = crate::named(&document, &jig_protocol_config::Directories::working()).expect("named");
    assert_eq!(named.bundle.as_deref(), Some(b"/bundle".as_slice()));
    assert_eq!(named.bundle_limits, crate::limits::Clients { bundle_files: 128, bundle_bytes: 16 << 20 });
}
#[test]
fn a_missing_bundle_is_named_by_its_path() {
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap().replace(",\"bundle\":null}", "}");
    assert_eq!(&*refused(input.as_bytes()), b"application.clients.bundle: missing\n");
}
#[test]
fn a_webhook_listener_with_a_bundle_is_refused() {
    let document = fixtures::with(
        &fixtures::webhook_document(),
        b"application.webhooks",
        br#"{"listener":{"socket":"127.0.0.1:7200"},"bundle":null}"#,
    );
    assert_eq!(
        &*lines(&crate::named(&document, &jig_protocol_config::Directories::working()).unwrap_err()),
        b"application.webhooks.bundle: unknown member\n"
    );
}

#[test]
fn paths_are_resolved_by_kind_and_absolute_only_members_refuse_relative_values() {
    let directories = config::Directories::given(b"/configuration", b"/data", b"/cache").unwrap();
    for (member, input, expected, refused) in [
        (b"application.workers[0].root".as_slice(), br#""root""#.as_slice(), b"/data/root".as_slice(), false),
        (b"application.workers[0].root", br#""/kept/root""#, b"/kept/root", false),
        (b"engine.deployment.directory", br#""state""#, b"/data/state", false),
        (b"engine.deployment.directory", br#""/kept/state""#, b"/kept/state", false),
        (b"engine.charters[0].instructions", br#""chat.txt""#, b"/configuration/chat.txt", false),
        (b"engine.charters[0].instructions", br#""/kept/chat.txt""#, b"/kept/chat.txt", false),
        (b"workers[0].secret", br#""secret""#, b"/data/secret", false),
        (b"workers[0].secret", br#""/kept/secret""#, b"/kept/secret", false),
        (b"engine.hosts.workers[0].digest", br#""digest""#, b"/data/digest", false),
        (b"engine.hosts.workers[0].digest", br#""/kept/digest""#, b"/kept/digest", false),
        (b"application.workers[0].git", br#""git""#, b"", true),
        (b"application.clients.bundle", br#""bundle""#, b"", true),
        (b"application.clients.bundle", br#""/kept/bundle""#, b"/kept/bundle", false),
        (b"workers[0].agent.program", br#""temper""#, b"", true),
        (b"workers[0].agent.configuration", br#""agent.json""#, b"", true),
        (b"workers[0].agent.arguments", br#"["agent","agent.json"]"#, b"", true),
    ] {
        let document = fixtures::with(&fixtures::document(), member, input);
        let result = crate::named(&document, &directories);
        if refused {
            let lines = lines(&result.unwrap_err());
            let expected = [member, b": must be an absolute path\n"].concat();
            assert_eq!(lines.as_ref(), expected, "{}", core::str::from_utf8(member).expect("test path"));
        } else {
            let named = result.unwrap();
            let value = match member {
                b"application.workers[0].root" => &*named.worker.as_ref().expect("standalone worker").root,
                b"engine.deployment.directory" => &*named.deployment,
                b"application.clients.bundle" => named.bundle.as_deref().unwrap(),
                _ => &*named.files.iter().find(|file| file.path.as_ref() == expected).expect("resolved file key").path,
            };
            assert_eq!(value, expected);
        }
    }
    for trust in [br#""roots.der""#.as_slice(), br#""/kept/roots.der""#] {
        let tls = [br#"{"tls":{"server_name":"example.test","trust":"#.as_slice(), trust, b"}}"].concat();
        let document = fixtures::with(&fixtures::document(), b"application.forges[0].transport", &tls);
        let named = crate::named(&document, &directories).unwrap();
        let application::Transport::Tls { trust: Some(value), .. } = &named.forges[0].transport else { panic!("TLS") };
        assert_eq!(
            value.as_ref(),
            if trust == br#""roots.der""# { b"/configuration/roots.der".as_slice() } else { b"/kept/roots.der" }
        );
    }
}

fn github_application() -> Box<str> {
    github_input(core::str::from_utf8(fixtures::APPLICATION).unwrap())
}
pub(super) fn github_input(input: &str) -> Box<str> {
    input
                .replace("\"kind\":\"forgejo\"", "\"kind\":\"github\"")
        .replace("\"api\":{\"socket\":\"127.0.0.1:9\"}", "\"api\":null")
        .replace("\"transport\":\"plaintext\"", "\"transport\":null")
        .replace("\"git\":\"http://127.0.0.1:9\"", "\"git\":null,\"web\":null")
        .replace(",\"webhook_secret\":null,\"api_token\":\"secrets/forge-1.api-token\",\"user\":{\"file\":\"secrets/forge-1.user\"},\"identity\":1", "")
        .replace("\"token\":\"secrets/forge-1.git-token\",\"login\":\"temper\",\"author\":{\"name\":\"Temper\",\"email\":\"temper@example.test\"}", "\"forge\":1,\"login\":null,\"author\":null").into_boxed_str()
}
#[test]
fn load_refuses_a_record_one_byte_short_of_app_key() {
    let document = github_input(core::str::from_utf8(&fixtures::document()).unwrap());
    let needle = "\"overrides\":{\"checkout.name_bytes\"";
    assert_eq!(document.matches(needle).count(), 1);
    let document = document.replacen(needle, "\"overrides\":{\"records.value_bytes\":1196,\"checkout.name_bytes\"", 1);
    let found = crate::named(document.as_bytes(), &config::Directories::working()).unwrap_err();
    assert_eq!(
        lines(&found).as_ref(),
        b"application.limits.overrides.records.value_bytes: must agree with AppKey (forge_component.key_bytes)\n"
    );
}
#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary tests format input and diagnostics")]
fn load_refuses_log_limits_naming_both_bounds() {
    let document = fixtures::document();
    let document = core::str::from_utf8(&document).unwrap();
    let needle = "\"overrides\":{\"checkout.name_bytes\"";
    assert_eq!(document.matches(needle).count(), 1);
    for (overrides, expected) in [
        (
            "\"forges.log_origins\":0",
            "application.limits.overrides.forges.log_origins: must agree with forges.log_origins (above 0)\n",
        ),
        (
            "\"forge_component.log_origins\":0",
            "application.limits.overrides.forge_component.log_origins: must agree with forge_component.log_origins (above 0)\napplication.limits.overrides.forges.log_origins: must agree with forge_component.log_origins\n",
        ),
        (
            "\"forge_component.log_poll_ms\":0",
            "application.limits.overrides.forge_component.log_poll_ms: must agree with forge_component.log_poll_ms (above 0)\n",
        ),
        (
            "\"forge_component.log_catch_up_ms\":0",
            "application.limits.overrides.forge_component.log_catch_up_ms: must agree with forge_component.log_catch_up_ms (above 0)\napplication.limits.overrides.forge_component.log_catch_up_ms: must agree with forge_component.log_poll_ms\n",
        ),
        (
            "\"forge_component.log_pages\":0",
            "application.limits.overrides.forge_component.log_pages: must agree with forge_component.log_pages (above 0)\n",
        ),
        (
            "\"forge_component.delivery_bytes\":0",
            "application.limits.overrides.forge_component.delivery_bytes: must agree with forge_component.delivery_bytes (above 0)\n",
        ),
        (
            "\"forges.log_origins\":5,\"forge_component.log_origins\":5",
            "application.limits.overrides.forges.log_origins: must agree with forges.log_origins (at most 4)\n",
        ),
        (
            "\"forges.log_origins\":3,\"forge_component.log_origins\":2",
            "application.limits.overrides.forges.log_origins: must agree with forge_component.log_origins\n",
        ),
        (
            "\"forge_component.log_poll_ms\":86400000,\"forge_component.log_catch_up_ms\":86399999",
            "application.limits.overrides.forge_component.log_catch_up_ms: must agree with forge_component.log_poll_ms\n",
        ),
        (
            "\"forge_component.log_catch_up_ms\":86400001",
            "application.limits.overrides.forge_component.log_catch_up_ms: must agree with forge_component.log_catch_up_ms (at most 86400000 ms / 24 h)\n",
        ),
    ] {
        let input = document.replacen(needle, &format!("\"overrides\":{{{overrides},\"checkout.name_bytes\""), 1);
        let found = crate::named(input.as_bytes(), &config::Directories::working()).unwrap_err();
        assert_eq!(lines(&found).as_ref(), expected.as_bytes(), "{overrides}");
    }
    let overrides = "\"forges.log_origins\":3,\"forge_component.log_origins\":7,\"forge_component.log_poll_ms\":7123,\"forge_component.log_catch_up_ms\":9321,\"forge_component.log_pages\":13,\"forge_component.delivery_bytes\":12345";
    let input = document.replacen(needle, &format!("\"overrides\":{{{overrides},\"checkout.name_bytes\""), 1);
    crate::named(input.as_bytes(), &config::Directories::working()).unwrap();
    // Named checks all relations; inspect the same document through the existing application parser.
    let parsed =
        config::document(input.as_bytes(), &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 })
            .unwrap();
    let at = config::read::Object::new(&parsed, 0).unwrap().member(&parsed, b"application").unwrap();
    let jig = fixtures::engine().jig;
    let mut problems = Problems::empty();
    let loaded = application::parse(&parsed, at, &jig, &config::Directories::working(), &mut problems).unwrap().limits;
    assert!(problems.is_empty());
    assert_eq!((loaded.forges.log_origins, loaded.forge_component.log_origins), (3, 7));
    assert_eq!(loaded.forge_component.log_poll, skein_lib::Duration::from_millis(7123));
    assert_eq!(loaded.forge_component.log_catch_up, skein_lib::Duration::from_millis(9321));
    assert_eq!(loaded.forge_component.log_pages, 13);
    assert_eq!(loaded.forge_component.delivery_bytes, 12_345);
}
#[test]
fn github_app_read_only_defaults_false_and_resolves_the_configured_boolean() {
    let jig = fixtures::engine().jig;
    for (member, expected) in [("", false), (",\"app_read_only\":false", false), (",\"app_read_only\":true", true)] {
        let input = github_application().replace("\"web\":null", &["\"web\":null", member].concat());
        let (members, problems) = application(input.as_bytes(), &jig);
        assert_eq!(lines(&problems).as_ref(), b"");
        let members = members.unwrap();
        assert_eq!(members.forges[0].app_read_only, expected);
        let mut problems = Problems::empty();
        let resolved =
            application::resolve(&members, &settings::Files { entries: List::with_capacity(0) }, &mut problems)
                .unwrap();
        assert_eq!(lines(&problems).as_ref(), b"");
        assert_eq!(resolved.forges[0].app_read_only, expected);
    }
}
#[test]
fn github_app_read_only_refuses_non_booleans_at_its_path() {
    let input = github_application().replace("\"web\":null", "\"web\":null,\"app_read_only\":\"true\"");
    let (_, problems) = application(input.as_bytes(), &fixtures::engine().jig);
    assert_eq!(lines(&problems).as_ref(), b"application.forges[0].app_read_only: wrong value kind\n");
}
#[test]
fn forgejo_refuses_app_read_only_even_when_false() {
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    for value in ["true", "false"] {
        let input = input.replace("\"kind\":\"forgejo\"", &["\"kind\":\"forgejo\",\"app_read_only\":", value].concat());
        let (_, problems) = application(input.as_bytes(), &fixtures::engine().jig);
        assert_eq!(lines(&problems).as_ref(), b"application.forges[0].app_read_only: invalid form\n");
    }
}
#[test]
fn github_defaults_and_the_identity_join_need_no_credential_files() {
    use crate::settings::Kind;
    let jig = fixtures::engine().jig;
    let (members, problems) = application(github_application().as_bytes(), &jig);
    assert_eq!(lines(&problems).as_ref(), b"");
    let members = members.unwrap();
    let forge = &members.forges[0];
    assert_eq!(forge.kind, Kind::GitHub);
    assert_eq!(forge.api, config::Address::Host { name: Box::from(b"api.github.com".as_slice()), port: 443 });
    match &forge.transport {
        application::Transport::Tls { server_name, trust } => {
            assert_eq!(server_name.as_ref(), b"api.github.com");
            assert!(trust.is_none());
        }
        application::Transport::Plaintext => panic!("GitHub defaults to TLS"),
    }
    assert_eq!(forge.git.as_ref(), b"https://github.com");
    assert_eq!(forge.web.as_deref(), Some(b"https://github.com".as_slice()));
    assert_eq!(forge.identity, 1);
    assert_eq!(members.identities[0].forge, Some(1));
    assert!(application::files(&members, 4096, 8192).is_empty());
    let resolved =
        application::resolve(&members, &settings::Files { entries: List::with_capacity(0) }, &mut Problems::empty())
            .unwrap();
    assert!(resolved.forges[0].api_token.is_none());
    assert_eq!(resolved.identities[0].forge, 1);
    let input = github_application()
        .replace("\"api\":null", "\"api\":{\"socket\":\"127.0.0.1:9\"}")
        .replace("\"transport\":null", "\"transport\":\"plaintext\"")
        .replace("\"web\":null", "\"web\":\"http://127.0.0.1:9/web\"");
    let (members, problems) = application(input.as_bytes(), &jig);
    assert_eq!(lines(&problems).as_ref(), b"");
    assert_eq!(members.unwrap().forges[0].web.as_deref(), Some(b"http://127.0.0.1:9/web".as_slice()));
}
#[test]
fn github_refusals_name_the_configured_member() {
    let jig = fixtures::engine().jig;
    for (needle, replacement, path) in [
        ("\"kind\":\"github\"", "\"kind\":\"other\"", "forges[0].kind"),
        ("\"web\":null", "\"web\":\"https://bad/\"", "forges[0].web"),
        ("\"web\":null", "\"web\":null,\"api_token\":\"file\"", "forges[0].api_token"),
        ("\"web\":null", "\"web\":null,\"user\":42", "forges[0].user"),
        ("\"web\":null", "\"web\":null,\"webhook_secret\":null", "forges[0].webhook_secret"),
        ("\"forge\":1", "\"forge\":1,\"token\":null", "identities[0].token"),
        ("127.0.0.1:7100", "127.0.0.1:0", "clients.listener"),
        ("\"forge\":1", "\"forge\":2", "forges[0].identity"),
        ("\"forge\":1,", "", "forges[0].identity"),
        ("\"web\":null", "\"web\":null,\"identity\":1", "forges[0].identity"),
        ("\"web\":null", "\"web\":null,\"identity\":0", "forges[0].identity"),
        (
            "\"identities\":[",
            "\"identities\":[{\"number\":2,\"forge\":1,\"login\":null,\"author\":null},",
            "forges[0].identity",
        ),
    ] {
        let input = github_application().replace(needle, replacement);
        let (members, problems) = application(input.as_bytes(), &jig);
        assert!(members.is_none(), "{path}");
        assert!(core::str::from_utf8(&lines(&problems)).unwrap().contains(path), "{path}: {:?}", lines(&problems));
    }
    let input = core::str::from_utf8(fixtures::APPLICATION)
        .unwrap()
        .replace("\"kind\":\"forgejo\"", "\"kind\":\"forgejo\",\"web\":\"https://forge.example\"");
    let (members, problems) = application(input.as_bytes(), &jig);
    assert!(members.is_none());
    assert!(core::str::from_utf8(&lines(&problems)).unwrap().contains("forges[0].web"));
}

#[test]
fn a_configured_github_forge_without_records_starts_not_connected() {
    let document = fixtures::document();
    let document = github_input(core::str::from_utf8(&document).unwrap());
    let mut service = crate::startup(
        document.as_bytes(),
        &config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
    .unwrap();
    assert_eq!(service.engine.as_ref().expect("engine").workers.forges[0].kind, settings::Kind::GitHub);
    service.engine.as_mut().expect("engine").store = fixtures::memory_store();
    let mut face = fixtures::ForgeFace::new();
    for _ in 0_u8..32 {
        face.turn(&mut service);
    }
    assert_eq!(
        service.notes().pop(),
        Some(crate::Note::ForgeNotConnected { number: 1, why: crate::NotConnected::NoRecords })
    );
    assert!(service.engine.as_ref().expect("engine").credentials.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge_records.done());
    assert!(service.engine.as_ref().expect("engine").domain.ready());
    assert_eq!(service.end(), None);
}

#[test]
fn an_unresolved_github_forge_starts_not_connected_naming_its_host() {
    let document = fixtures::document();
    let document = github_input(core::str::from_utf8(&document).unwrap());
    let mut runtime = fixtures::forge_runtime();
    runtime.get_mut(0).unwrap().address = crate::forge::HELD_ADDRESS;
    let mut service = crate::startup(
        document.as_bytes(),
        &config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        runtime,
    )
    .unwrap();
    assert_eq!(service.engine.as_ref().expect("engine").workers.forges[0].kind, settings::Kind::GitHub);
    service.engine.as_mut().expect("engine").store = fixtures::memory_store();
    let mut face = fixtures::ForgeFace::new();
    for _ in 0_u8..32 {
        face.turn(&mut service);
    }
    assert_eq!(
        service.notes().pop(),
        Some(crate::Note::ForgeNotConnected {
            number: 1,
            why: crate::NotConnected::Unreachable { host: b"api.github.com".as_slice().into() }
        })
    );
    assert!(service.engine.as_ref().expect("engine").credentials.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge_records.done());
    assert!(service.engine.as_ref().expect("engine").domain.ready());
    assert_eq!(service.end(), None);
}

#[test]
fn startup_admits_the_full_setup_total_and_refuses_one_byte_short() {
    let document = github_input(core::str::from_utf8(&fixtures::document()).unwrap())
        .replace("\"api\":null", "\"api\":{\"socket\":\"127.0.0.1:9\"}")
        .replace("\"transport\":null", "\"transport\":\"plaintext\"");
    let directories = config::Directories::working();
    let files = fixtures::files();
    let agent = fixtures::startup_agent();
    let seeds = fixtures::seeds();
    let required = crate::startup::memory_requirements(
        document.as_bytes(),
        &directories,
        &files,
        &agent,
        &seeds,
        None,
        fixtures::forge_runtime(),
        None,
    )
    .unwrap();
    for short in [false, true] {
        let input = document
            .replacen("1152921504606846976", &(required.engine_bytes - u64::from(short)).to_string(), 1)
            .replace("1152921504606846976", &required.worker_bytes.to_string());
        let started = crate::startup(
            input.as_bytes(),
            &directories,
            &files,
            &agent,
            &seeds,
            None,
            fixtures::worker_runtime(),
            fixtures::forge_runtime(),
        );
        if short {
            assert_eq!(
                lines(&started.err().unwrap()).as_ref(),
                b"engine.memory_bytes: the worst case passes memory_bytes by 1 bytes\n"
            );
        } else {
            assert_eq!(started.unwrap().worst_case(), required.engine_bytes + required.worker_bytes);
        }
    }
}
#[test]
fn startup_maps_refused_setup_and_total_overflow_to_memory_bound() {
    let mut engine = fixtures::engine();
    let (endpoints, _) = crate::forge::prepare(&engine, fixtures::forge_runtime()).unwrap();
    let root = crate::limits::engine(&engine.jig.limits, &engine.limits).unwrap();
    let store = crate::store::limits(&root, &root.forge, &engine.jig.limits.store).unwrap();
    let io = engine.jig.limits.io;
    // u32::MAX runs overflows setup's writes = runs + forges, after the host bound fits.
    for (runs, roots) in [(0, 0), (u32::MAX, 0), (8, u64::MAX)] {
        engine.limits.setup.runs = runs;
        assert_eq!(runs.checked_add(engine.limits.forge_component.forges).is_none(), runs == u32::MAX);
        assert_eq!(crate::setup::worst_case(&engine, &endpoints, 2).is_none(), runs != 8);
        let found = crate::system::startup_worst_case(&engine, &root, &store, None, 0, &io, (roots, 0, &endpoints))
            .unwrap_err();
        assert_eq!(lines(&Problems::jig(found, &Path::root())).as_ref(), b"engine.memory_bytes: past its bound\n");
    }
}

#[test]
fn admission_model_words_use_tokens_and_missing_names_make_no_amount_claim() {
    use jig_domain_fleet::AgentShortfall;
    for (why, expected) in [
        (
            AgentShortfall::Model { charter: 7, model: 0 },
            "model: not in the agent's configuration under the same name\n",
        ),
        (
            AgentShortfall::Window { charter: 7, model: 0 },
            "model: needs a context window of 8192 tokens; the agent offers 1024 tokens\n",
        ),
        (
            AgentShortfall::Output { charter: 7, model: 0 },
            "model: needs 8192 output tokens; the agent offers 1024 tokens\n",
        ),
    ] {
        let mut problems = Problems::empty();
        problems.push(Path::root().member(b"model"), What::Admission { why, offered: 1024, required: 8192 });
        assert_eq!(&*lines(&problems), expected.as_bytes());
    }
}

#[test]
fn a_store_reader_derives_serves_limits_or_their_problem_path() {
    let document = fixtures::document();
    let directories = config::Directories::working();
    let service = crate::startup(
        &document,
        &directories,
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
    .unwrap();
    assert_eq!(
        service.engine.as_ref().unwrap().store.opening_limits().core,
        temper_engine_domain::core_limits(&service.engine.as_ref().unwrap().env.limits),
    );
    let refused = core::str::from_utf8(&document).unwrap().replacen(
        "\"overrides\":{\"checkout.name_bytes\"",
        "\"overrides\":{\"root.loads.rows\":0,\"checkout.name_bytes\"",
        1,
    );
    for (name, input, expected) in [
        (
            "serve",
            document.as_ref(),
            Ok((
                service.engine.as_ref().expect("engine").store.opening_limits(),
                service.engine.as_ref().expect("engine").env.limits.forge,
            )),
        ),
        ("limits", refused.as_bytes(), Err(b"engine.limits: past its bound\n".as_slice())),
    ] {
        let found = crate::store_limits(input, &directories);
        let found = found.map(|limits| (limits.store, limits.forge)).map_err(|problems| lines(&problems));
        assert_eq!(found.as_ref().map_err(AsRef::as_ref), expected.as_ref().map_err(AsRef::as_ref), "{name}");
    }
}

#[test]
fn an_engine_alone_prepares_without_a_worker_or_its_files_and_agent() {
    let input = fixtures::engine_alone_document();
    let named = crate::named(&input, &config::Directories::working()).unwrap();
    assert!(named.worker.is_none());
    let mut files = fixtures::files();
    for path in [b"worker-secret".as_slice(), fixtures::AGENT_JSON] {
        files.entries = fixtures::without(files.entries, path);
    }
    let service = crate::startup::with_setup(
        &input,
        &config::Directories::working(),
        &files,
        None,
        &fixtures::seeds(),
        None,
        None,
        fixtures::forge_runtime(),
        None,
        false,
    )
    .unwrap();
    assert!(service.worker.is_none());
}

#[test]
fn an_engine_alone_admits_its_own_worst_case_and_names_an_insufficient_memory_declaration() {
    let input = fixtures::engine_alone_document();
    let requirements = crate::startup::memory_requirements(
        &input,
        &config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::forge_runtime(),
        None,
    )
    .unwrap();
    assert_eq!(requirements.worker_bytes, 0);
    assert!(requirements.engine_bytes > 0);
    let terms = crate::startup::memory_terms(
        &input,
        &config::Directories::working(),
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::forge_runtime(),
        None,
    )
    .unwrap();
    assert!(terms.worker.is_empty());
    assert_eq!(crate::startup::memory_sum(&terms.engine), Some(requirements.engine_bytes));
    for (memory, accepted) in
        [(requirements.engine_bytes, true), (requirements.engine_bytes.checked_sub(1).unwrap(), false)]
    {
        let input = fixtures::with(&input, b"engine.memory_bytes", memory.to_string().as_bytes());
        let result = crate::startup::with_setup(
            &input,
            &config::Directories::working(),
            &fixtures::files(),
            None,
            &fixtures::seeds(),
            None,
            None,
            fixtures::forge_runtime(),
            None,
            false,
        );
        if accepted {
            assert!(result.is_ok(), "at the calculated worst case");
        } else {
            let found = result.err().unwrap();
            assert_eq!(found.problems.len(), 1);
            assert_eq!(found.problems.get(0).unwrap().path, Path::root().member(b"engine").member(b"memory_bytes"));
            assert_eq!(found.problems.get(0).unwrap().what, What::Jig(config::What::Memory { over: 1 }));
        }
    }
}

#[test]
fn a_checked_worker_alone_has_its_shape_and_refuses_engine_application_members() {
    let input = fixtures::worker_alone_document();
    assert_eq!(crate::startup::shape(&input, &config::Directories::working()), Ok(crate::startup::Shape::WorkerAlone));
    let text = core::str::from_utf8(&input).unwrap().replace("\"application\":{", "\"application\":{\"forges\":[],");
    let found = crate::startup::shape(text.as_bytes(), &config::Directories::working()).unwrap_err();
    assert_eq!(found.problems.len(), 1);
    assert_eq!(found.problems.get(0).unwrap().path, Path::root().member(b"application").member(b"forges"));
    assert_eq!(found.problems.get(0).unwrap().what, What::WorkerAlone);
}

#[test]
fn a_worker_alone_checks_its_application_paths_and_its_git_and_channel_limits() {
    let input = fixtures::worker_alone_document();
    for (member, value, path, what) in [
        (
            b"application.workers[0].root".as_slice(),
            b"12".as_slice(),
            Path::root().member(b"application").member(b"workers").child(Segment::Index(0)).member(b"root"),
            What::Jig(config::What::Kind),
        ),
        (
            b"application.workers[0].root",
            br#""../escape""#,
            Path::root().member(b"application").member(b"workers").child(Segment::Index(0)).member(b"root"),
            What::Jig(config::What::Relative),
        ),
        (
            b"application.workers[0].git",
            br#""git""#,
            Path::root().member(b"application").member(b"workers").child(Segment::Index(0)).member(b"git"),
            What::Absolute,
        ),
        (
            b"workers[0].agent.configuration",
            br#""temper""#,
            Path::root().member(b"workers").child(Segment::Index(0)).member(b"agent").member(b"configuration"),
            What::Absolute,
        ),
        (
            b"application.limits.overrides",
            br#"{"git_component.operations":1}"#,
            Path::root()
                .member(b"application")
                .member(b"limits")
                .member(b"overrides")
                .member(b"git_component.operations"),
            What::Against { other: b"checkout.workspaces" },
        ),
        (
            b"application.limits.overrides",
            br#"{"checkout.repositories":4294967295}"#,
            Path::root().member(b"application").member(b"limits").member(b"overrides").member(b"checkout.repositories"),
            What::Past {
                bytes: 4_294_967_295,
                bound: u64::from(temper_protocol_worker_wire::v1::CEILINGS.items_repositories),
            },
        ),
    ] {
        let input = fixtures::with(&input, member, value);
        let found = crate::startup::shape(&input, &config::Directories::working()).unwrap_err();
        assert_eq!(found.problems.len(), 1, "{}: {:?}", core::str::from_utf8(member).unwrap(), found);
        assert_eq!((found.problems.get(0).unwrap().path.clone(), found.problems.get(0).unwrap().what), (path, what));
    }
}

#[test]
fn fake_handovers_are_private_and_refused_unless_exactly_40_lowercase_hex_digits() {
    let checked = checked_fake_setup();
    let listed = crate::startup::setup_files(&checked);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path.as_ref(), b"/data/demo/forge/admin_token");
    assert!(listed[0].private);
    assert_eq!(listed[0].max, 40);
    for value in [
        b"".as_slice(),
        b"0123456789abcdef0123456789abcdef0123456",
        b"0123456789abcdef0123456789abcdef012345678",
        b"0123456789abcdef0123456789abcdef0123456A",
        b"0123456789abcdef0123456789abcdef0123456g",
        b"0123456789abcdef0123456789abcdef0123456\n",
        b"0123456789abcdef0123456789abcdef01234567",
    ] {
        let mut files = fixtures::files();
        let mut entries = List::with_capacity(files.entries.len().checked_add(1).unwrap());
        for entry in files.entries.into_boxed() {
            entries.push(entry).unwrap();
        }
        entries.push((listed[0].path.clone(), skein_lib::Secret::new(value.into()))).unwrap();
        files.entries = entries;
        let result = fake_start(&checked, &files);
        if value == b"0123456789abcdef0123456789abcdef01234567" {
            assert!(result.is_ok());
        } else if value.len() > 40 {
            assert_eq!(lines(&result.err().unwrap()).as_ref(), b"application.setup[0].admin_token: past its bound\n");
        } else {
            assert_eq!(
                lines(&result.err().unwrap()).as_ref(),
                b"application.setup[0].admin_token: /data/demo/forge/admin_token: admin_token must be exactly 40 lowercase hexadecimal digits\n"
            );
        }
    }
    let problems = fake_start(&checked, &fixtures::files()).err().unwrap();
    assert_eq!(lines(&problems).as_ref(), b"application.setup[0].admin_token: not read\n");
}

fn fake_start(
    checked: &temper_forge_setup::check::Checked,
    files: &settings::Files,
) -> Result<crate::Service, Problems> {
    crate::startup::with_setup(
        &fixtures::document(),
        &config::Directories::working(),
        files,
        Some(&fixtures::startup_agent()),
        &fixtures::seeds(),
        None,
        Some(fixtures::worker_runtime()),
        fixtures::forge_runtime(),
        Some(checked),
        false,
    )
}

#[test]
fn a_worker_alones_complete_envelope_must_fit_its_own_memory() {
    let input = fixtures::worker_alone_document();
    let directories = jig_protocol_config::Directories::working();
    let required = crate::startup::memory_requirements(
        &input,
        &directories,
        &fixtures::files(),
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        List::with_capacity(0),
        None,
    )
    .unwrap();
    assert_eq!(required.engine_bytes, 0);
    let part = crate::startup::worker_part(&input, &directories).unwrap();
    let worker = crate::worker_worst_case(&part.worker, &fixtures::startup_agent(), &part.limits).unwrap();
    assert!(required.worker_bytes > worker, "includes shared IO and process routing");
    let input = fixtures::with(&input, b"workers[0].memory_bytes", (required.worker_bytes - 1).to_string().as_bytes());
    let part = crate::startup::worker_part(&input, &directories).unwrap();
    let found = crate::startup::worker_alone(
        part,
        &fixtures::files(),
        &fixtures::startup_agent(),
        fixtures::worker_runtime(),
        7,
    )
    .err()
    .unwrap();
    assert_eq!(lines(&found).as_ref(), b"workers[0].memory_bytes: the worst case passes memory_bytes by 1 bytes\n");
}

#[test]
fn a_worker_alone_with_the_owned_examples_worker_fits_one_ring() {
    // Linux's IORING_MAX_ENTRIES, the largest submission ring.
    const IORING_MAX_ENTRIES: u32 = 32_768;
    let input = fixtures::owned_worker_alone_document();
    let directories = config::Directories::working();
    let agent = fixtures::owned_worker_agent();
    let mut part = crate::startup::worker_part(&input, &directories).unwrap();
    let required = crate::system::worker_alone_worst_case(&part.worker, &agent, &part.limits).unwrap();
    // The separate payload-memory work owns the example's memory_bytes.
    part.worker.jig.memory = required;
    let operations = crate::system::worker_alone_operations(
        &part.worker.jig.limits.io,
        crate::worker_settings::operations(&part.limits.git_component).unwrap(),
    )
    .unwrap();
    let mut files = settings::Files { entries: List::with_capacity(2) };
    for file in &part.files {
        files
            .entries
            .push((file.path.clone(), skein_lib::Secret::new(b"fixture-worker-secret".as_slice().into())))
            .unwrap();
    }
    let service = crate::startup::worker_alone(part, &files, &agent, fixtures::worker_runtime(), 7).unwrap();
    assert!(service.operations() <= IORING_MAX_ENTRIES);
    assert_eq!(service.operations(), operations, "the start's check and the ring agree");
    assert_eq!(service.worst_case(), required);
}

fn owned_start_document(asked: bool) -> Box<[u8]> {
    let setup = br#"[{"forge":1,"directory":"forge-1","bot":{"login":"temper","email":"temper@example.test"},
        "start":"asked","mode":{"owned":{"forgejo":"/opt/forgejo","runner":null,
        "listen":{"socket":"127.0.0.1:9"},"person":{"login":"ada","email":"ada@example.test"}}}}]"#;
    let input = fixtures::with(&fixtures::document(), b"application.setup", setup);
    let input = fixtures::with(&input, b"application.forges[0].webhook_secret", br#""secrets/forge-1.webhook-secret""#);
    let input = fixtures::with(&input, b"application.webhooks", br#"{"listener":{"socket":"127.0.0.1:7102"}}"#);
    if asked {
        input
    } else {
        core::str::from_utf8(&input).unwrap().replace("\"start\":\"asked\",", "").into_bytes().into_boxed_slice()
    }
}
fn start_owned(asked: bool, files: &settings::Files) -> Result<crate::Service, Problems> {
    crate::startup(
        &owned_start_document(asked),
        &config::Directories::working(),
        files,
        &fixtures::startup_agent(),
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
}
fn assert_owned_not_set_up(files: &settings::Files) {
    let mut service = start_owned(true, files).expect("absent asked files are allowed");
    let engine = service.engine.as_mut().unwrap();
    assert_eq!(engine.forge.component.start(1), Some(skein_forge_connection::start::Start::Unconnected));
    assert!(engine.values.identities.newest(1).is_none(), "no partial identity");
    engine.store = fixtures::memory_store();
    let mut face = fixtures::ForgeFace::new();
    for _ in 0_u8..32 {
        face.turn(&mut service);
    }
    assert_eq!(
        service.engine.as_ref().unwrap().domain.forge_connection(1),
        temper_engine_domain_forge::Connection::NotSetUp
    );
    assert!(face.sent().is_empty());
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty(), "no Connect request");
    assert!(service.engine.as_ref().unwrap().credentials.is_empty());
    assert_eq!(service.end(), None);
}
#[test]
fn an_asked_owned_forge_without_any_files_starts_not_set_up() {
    let mut files = fixtures::files();
    for name in [
        b"secrets/forge-1.api-token".as_slice(),
        b"secrets/forge-1.user",
        b"secrets/forge-1.git-token",
        b"secrets/forge-1.webhook-secret",
    ] {
        files.entries = fixtures::without(files.entries, name);
    }
    assert_owned_not_set_up(&files);
}
#[test]
fn an_asked_owned_forges_partial_files_never_grant_access() {
    for absent in [
        b"secrets/forge-1.user".as_slice(),
        b"secrets/forge-1.api-token",
        b"secrets/forge-1.git-token",
        b"secrets/forge-1.webhook-secret",
    ] {
        let mut files = fixtures::files();
        files.entries = fixtures::without(files.entries, absent);
        assert_owned_not_set_up(&files);
    }
}
#[test]
fn an_unasked_owned_forges_absent_api_token_still_refuses_startup() {
    let mut files = fixtures::files();
    files.entries = fixtures::without(files.entries, b"secrets/forge-1.api-token");
    assert_eq!(
        lines(&start_owned(false, &files).err().unwrap()).as_ref(),
        b"application.forges[0].api_token: not read\n"
    );
}
#[test]
fn an_asked_owned_forge_with_all_files_starts_from_their_access() {
    let mut service = start_owned(true, &fixtures::files()).unwrap();
    let engine = service.engine.as_mut().unwrap();
    assert_eq!(engine.workers.forges[0].user, 42);
    assert!(engine.values.identities.newest(1).is_some(), "identity from its file");
    engine.store = fixtures::memory_store();
    let mut face = fixtures::ForgeFace::new();
    face.connected(&mut service);
    assert!(skein_lib::bytes::find(face.sent(), b"Authorization: token api-token\r\n").is_some());
    assert_eq!(service.end(), None);
}
#[test]
fn asked_files_keep_the_same_paths_bounds_and_privacy_as_required_files() {
    let required = crate::startup::named(&owned_start_document(false), &config::Directories::working()).unwrap();
    let asked = crate::startup::named(&owned_start_document(true), &config::Directories::working()).unwrap();
    assert_eq!(asked.asked_files.len(), 4);
    for file in asked.asked_files {
        let original = required.files.iter().find(|original| original.path == file.path).unwrap();
        assert_eq!(file.at, original.at);
        assert_eq!(file.max, original.max);
        assert_eq!(file.private, original.private);
        assert!(!asked.files.iter().any(|required| required.path == file.path));
    }
}
#[test]
fn an_asked_file_present_past_its_bound_still_refuses_startup() {
    let mut files = fixtures::files();
    files.entries = fixtures::without(files.entries, b"secrets/forge-1.api-token");
    files
        .entries
        .push((
            b"secrets/forge-1.api-token".as_slice().into(),
            skein_lib::Secret::new(Box::from([b'a'; 4097].as_slice())),
        ))
        .unwrap();
    assert_eq!(
        lines(&start_owned(true, &files).err().unwrap()).as_ref(),
        b"application.forges[0].api_token: past its bound\n"
    );
}

#[test]
fn forge_configuration_keeps_kind_mode_and_asked_start() {
    use temper_engine_domain_forge::{Configured, Kind, Mode};
    let mut settings = fixtures::engine();
    for (source, kind) in [(settings::Kind::Forgejo, Kind::Forgejo), (settings::Kind::GitHub, Kind::GitHub)] {
        settings.forges[0].kind = source;
        assert_eq!(
            application::configured(&settings).as_ref(),
            [Configured {
                bot: Box::new([]),
                admin_connect: false,
                owned: None,
                web: None,
                read_only: false,
                number: 1,
                kind,
                mode: Mode::Connected,
                asked: false
            }]
        );
    }
    settings.forges[0].kind = settings::Kind::Forgejo;
    settings.setup = Box::new([fixtures::owned_setup(1)]);
    assert_eq!(
        application::configured(&settings).as_ref(),
        [Configured {
            bot: b"temper".as_slice().into(),
            admin_connect: false,
            owned: Some(temper_engine_domain_forge::Owned {
                forgejo: skein_forge::forgejo::pin::VERSION.as_bytes().into(),
                runner: None,
                password: b"/data/forge/person_password".as_slice().into(),
            }),
            web: None,
            read_only: false,
            number: 1,
            kind: Kind::Forgejo,
            mode: Mode::Owned,
            asked: true,
        }]
    );
    for (admin_token, allowed) in [(None, true), (Some(b"admin-file".as_slice().into()), false)] {
        settings.setup[0].entry.mode = temper_forge_setup::setup::Mode::Managed { admin_token };
        let configured = application::configured(&settings);
        assert_eq!(configured[0].mode, Mode::Managed);
        assert_eq!(configured[0].admin_connect, allowed);
    }
}

#[test]
fn records_only_local_entries_need_the_checked_boundary_and_name_no_files() {
    let input = records_only_document();
    // The inverse join is retained until temper-15; no token member is present.
    let named = crate::named(&input, &config::Directories::working()).unwrap();
    assert!(named.files.iter().all(|file| !file.path.is_empty()));
    assert!(named.files.iter().all(|file| !file.path.starts_with(b"secrets/forge-1")));
    let checked = checked_local_setup(true);
    assert!(crate::startup::setup_files(&checked).is_empty());
    for (setup, admitted) in [(None, false), (Some(&checked), true)] {
        let result = crate::startup::with_setup(
            &input,
            &config::Directories::working(),
            &fixtures::files(),
            Some(&fixtures::startup_agent()),
            &fixtures::seeds(),
            None,
            Some(fixtures::worker_runtime()),
            fixtures::forge_runtime(),
            setup,
            false,
        );
        assert_eq!(result.is_ok(), admitted, "checked local input: {:?}", result.err());
    }
}

fn records_only_document() -> Box<[u8]> {
    let input = fixtures::document();
    let input = fixtures::with(&input, b"application.forges[0]", br#"{"number":1,"kind":"forgejo","api":{"socket":"127.0.0.1:9"},"transport":"plaintext","git":"http://127.0.0.1:9","identity":1}"#);
    fixtures::with(
        &input,
        b"application.identities[0]",
        br#"{"number":1,"login":"temper","author":{"name":"Temper","email":"temper@example.test"}}"#,
    )
}
#[test]
fn any_old_member_keeps_the_partial_old_forms_missing_member_refusals() {
    let input = records_only_document();
    let text = core::str::from_utf8(&input).unwrap();
    for (member, expected) in [
        (
            "\"api_token\":\"old\",",
            "application.forges[0].webhook_secret: missing\napplication.forges[0].user: missing\n",
        ),
        ("\"user\":42,", "application.forges[0].api_token: missing\napplication.forges[0].webhook_secret: missing\n"),
        ("\"webhook_secret\":null,", "application.forges[0].api_token: missing\napplication.forges[0].user: missing\n"),
    ] {
        let partial = text.replace("\"kind\":\"forgejo\",", &[member, "\"kind\":\"forgejo\","].concat());
        let refused = crate::named(partial.as_bytes(), &config::Directories::working()).unwrap_err();
        assert_eq!(lines(&refused).as_ref(), expected.as_bytes());
    }
    let partial = text.replace("\"number\":1,\"login\"", "\"token\":\"old\",\"number\":1,\"login\"");
    let refused = crate::named(partial.as_bytes(), &config::Directories::working()).unwrap_err();
    assert_eq!(lines(&refused).as_ref(), b"application.forges[0].api_token: missing\napplication.forges[0].webhook_secret: missing\napplication.forges[0].user: missing\n");
}

#[test]
fn a_records_only_forgejo_identity_still_requires_its_configured_author() {
    let input = fixtures::with(&records_only_document(), b"application.identities[0].author", b"null");
    let refused = crate::named(&input, &config::Directories::working()).unwrap_err();
    assert_eq!(lines(&refused).as_ref(), b"application.identities[0].author: invalid form\n");
}

#[test]
fn clients_public_keeps_the_parsed_origin_and_jig_name() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    let (members, problems) = application(input.as_bytes(), &jig);
    assert!(lines(&problems).is_empty());
    assert!(members.unwrap().clients.public.is_none());
    for (origin, scheme, host, port, name) in [
        ("https://temper.example.com", application::Scheme::Https, "temper.example.com", None, "temper.example.com"),
        (
            "https://temper.example.com:8443",
            application::Scheme::Https,
            "temper.example.com",
            Some(8443),
            "temper.example.com:8443",
        ),
        ("http://127.0.0.1:9000", application::Scheme::Http, "127.0.0.1", Some(9000), "127.0.0.1:9000"),
        ("http://localhost", application::Scheme::Http, "localhost", None, "localhost"),
        ("http://[::1]:9000", application::Scheme::Http, "[::1]", Some(9000), "[::1]:9000"),
    ] {
        let input = input.replace("\"public\":null", &format!("\"public\":\"{origin}\""));
        let (members, problems) = application(input.as_bytes(), &jig);
        assert!(lines(&problems).is_empty(), "{origin}: {:?}", lines(&problems));
        let public = members.unwrap().clients.public.unwrap();
        assert_eq!(
            (public.scheme, public.host.as_ref(), public.port, public.name.as_ref()),
            (scheme, host.as_bytes(), port, name.as_bytes())
        );
        assert_eq!(public.base().as_ref(), format!("{origin}/").as_bytes());
    }
}
#[test]
fn clients_public_refusals_name_the_public_member() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    let missing = input.replace("\"public\":null,", "");
    let (_, problems) = application(missing.as_bytes(), &jig);
    assert_eq!(lines(&problems).as_ref(), b"application.clients.public: missing: null for none\n");
    for origin in [
        "https://temper.example.com:443",
        "https://Temper.example.com",
        "https://temper.example.com/path",
        "https://temper.example.com/",
        "https://temper.example.com?query",
        "https://temper.example.com#fragment",
        "https://user@temper.example.com",
        "http://temper.example.com",
        "https://-bad.example.com",
        "https://bad-.example.com",
        "https://bad..example.com",
        "https://bad_label.example.com",
        "https://temper.example.com:",
        "https://temper.example.com:0",
        "https://temper.example.com:65536",
        "https://temper.example.com:+8443",
        "https://",
        "ftp://temper.example.com",
        "http://127.0.0.1:9000/path",
        "http://192.0.2.1:9000",
        "http://[::2]:9000",
        "https://127.0.0.1",
        "https://temper.example.com.",
    ] {
        let input = input.replace("\"public\":null", &format!("\"public\":\"{origin}\""));
        let (_, problems) = application(input.as_bytes(), &jig);
        assert_eq!(lines(&problems).as_ref(), b"application.clients.public: invalid form\n", "{origin}");
    }
    for value in ["true", "8", "{}"] {
        let input = input.replace("\"public\":null", &format!("\"public\":{value}"));
        let (_, problems) = application(input.as_bytes(), &jig);
        assert_eq!(lines(&problems).as_ref(), b"application.clients.public: wrong value kind\n", "{value}");
    }
}
#[test]
fn webhooks_public_is_an_unknown_member() {
    let input = core::str::from_utf8(fixtures::APPLICATION)
        .unwrap()
        .replace("\"webhooks\":null", "\"webhooks\":{\"listener\":{\"socket\":\"127.0.0.1:7200\"},\"public\":null}");
    let (_, problems) = application(input.as_bytes(), &fixtures::engine().jig);
    assert_eq!(lines(&problems).as_ref(), b"application.webhooks.public: unknown member\n");
}

#[test]
fn clients_public_bounds_dns_labels_and_the_host() {
    let jig = fixtures::engine().jig;
    let input = core::str::from_utf8(fixtures::APPLICATION).unwrap();
    let longest = format!("{}.{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(61));
    assert_eq!(longest.len(), 253);
    for (host, admitted) in [(longest.clone(), true), (format!("{longest}d"), false), ("a".repeat(64), false)] {
        let origin = format!("https://{host}");
        let input = input.replace("\"public\":null", &format!("\"public\":\"{origin}\""));
        let (members, problems) = application(input.as_bytes(), &jig);
        if admitted {
            assert!(lines(&problems).is_empty());
            assert_eq!(members.unwrap().clients.public.unwrap().host.as_ref(), host.as_bytes());
        } else {
            assert_eq!(lines(&problems).as_ref(), b"application.clients.public: invalid form\n");
        }
    }
}

#[test]
fn string_managed_starts_disconnected_without_a_setup_run() {
    use temper_forge_setup::check;
    let setup = br#"[{"forge":1,"bot":{"login":"temper","email":"temper@example.test"},"mode":"managed"}]"#;
    let input = fixtures::with(&records_only_document(), b"application.setup", setup);
    let document =
        config::document(&input, &config::DocumentLimits { bytes: 65_536, tokens: 16_384, depth: 32 }).unwrap();
    let named = crate::named(&input, &config::Directories::working()).unwrap();
    let application = config::read::Object::new(&document, 0).unwrap().member(&document, b"application").unwrap();
    let at = config::read::Object::new(&document, application).unwrap().member(&document, b"setup").unwrap();
    let members = check::Members {
        forges: Box::new([check::Forge {
            number: 1,
            api: Some(([127, 0, 0, 1], 9).into()),
            plaintext: true,
            git: named.forges[0].git.clone(),
            user: check::User::Id(0),
            api_token: Box::new([]),
            webhook_secret: None,
            identity_token: Box::new([]),
            identity_login: b"temper".as_slice().into(),
        }]),
        listeners: Box::new([]),
    };
    let checked = check::checked(&document, at, &config::Directories::working(), &members, None, &[]).unwrap();
    let mut service = crate::startup::with_setup(
        &input,
        &config::Directories::working(),
        &fixtures::files(),
        Some(&fixtures::startup_agent()),
        &fixtures::seeds(),
        None,
        Some(fixtures::worker_runtime()),
        fixtures::forge_runtime(),
        Some(&checked),
        false,
    )
    .unwrap();
    service.engine.as_mut().unwrap().store = fixtures::records_store();
    // Drain the startup record reads, then let setup act on their absence.
    for _ in 0_u32..8 {
        crate::forge::records::advance(service.engine.as_mut().unwrap());
    }
    crate::setup::advance(service.engine.as_mut().unwrap());
    let engine = service.engine.as_ref().unwrap();
    let bytes = crate::family::forges(engine.domain.forges(), &temper_client_protocol_wire::v1::CEILINGS).unwrap();
    let rows = temper_client_protocol_wire::decode_forges(&bytes, &temper_client_protocol_wire::v1::CEILINGS).unwrap();
    assert_eq!(rows.forges().get(0).unwrap().mode(), &temper_client_protocol_wire::v1::Mode::Managed);
    assert_eq!(rows.forges().get(0).unwrap().connection(), &temper_client_protocol_wire::v1::Connection::NotConnected);
    assert!(!engine.setup.busy(1));
    assert!(engine.setup.staged.is_empty());
    assert!(engine.setup.io.is_empty());
}

#[test]
fn owned_startup_details_keep_the_runner_pin_and_directory_path_and_managed_has_none() {
    use temper_forge_setup::setup::{Mode, Runner};
    let mut settings = fixtures::engine();
    settings.setup = Box::new([fixtures::owned_setup(1)]);
    settings.setup[0].entry.directory = b"/data/another-forge".as_slice().into();
    let Mode::Owned(owned) = &mut settings.setup[0].entry.mode else { panic!("owned fixture") };
    owned.runner = Some(Runner {
        binary: b"/bin/runner".as_slice().into(),
        labels: Box::new([]),
        capacity: 1,
        path: b"/bin".as_slice().into(),
    });
    let rows = application::configured(&settings);
    let details = rows[0].owned.as_ref().unwrap();
    assert_eq!(details.forgejo.as_ref(), skein_forge::forgejo::pin::VERSION.as_bytes());
    assert_eq!(details.runner.as_deref(), Some(skein_forge::forgejo::pin::RUNNER_VERSION.as_bytes()));
    assert_eq!(details.password.as_ref(), b"/data/another-forge/person_password");
    settings.setup[0].entry.mode = Mode::Managed { admin_token: Some(b"/fake/admin-token".as_slice().into()) };
    assert!(application::configured(&settings)[0].owned.is_none());
}
