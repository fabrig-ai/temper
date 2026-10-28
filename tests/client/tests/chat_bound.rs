//! A person's request through the tools client on init's engine document.
use jig_client_world::harness::{FrontFault, Settings, Until, World};
use jig_domain_people as people;
use jig_protocol_config as config;
use temper_client_world::{app::Temper, fixtures, front::Joint};
use temper_engine_domain as engine;

fn init() -> (config::Engine, temper_service::limits::Limits) {
    use std::num::NonZeroU16;
    let generated = temper_init::files(
        &temper_init::Facts {
            binary: b"/bin/temper",
            git: Some(b"/bin/git"),
            claude: None,
            login: Some(b"ada"),
            ports: temper_init::Ports {
                clients: NonZeroU16::new(8367).expect("port"),
                owned: NonZeroU16::new(8368).expect("port"),
                forge: NonZeroU16::new(8369).expect("port"),
                llm: NonZeroU16::new(8370).expect("port"),
                webhooks: NonZeroU16::new(8371).expect("port"),
            },
        },
        &temper_init::Directories { config: b"/config", data: b"/data" },
        &[0; 32],
        &[1; 32],
    )
    .expect("init files");
    let mut bytes = None;
    for file in generated.files {
        if file.name == b"temper.json" {
            let temper_init::Content::Bytes { bytes: value, .. } = file.content else { panic!("document bytes") };
            bytes = Some(value);
        }
    }
    let bytes = bytes.expect("init document");
    let document = config::document(&bytes, &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 })
        .expect("init document");
    let at = config::read::Object::new(&document, 0)
        .expect("document object")
        .member(&document, b"engine")
        .expect("engine member");
    let settings =
        temper_service::startup::engine(&document, at, &config::Directories::working()).expect("init engine");
    let mut at = 0;
    for name in [b"application".as_slice(), b"limits", b"overrides"] {
        at = config::read::Object::new(&document, at).expect("object").member(&document, name).expect("member");
    }
    let mut own = temper_service::limits::profile(temper_service::limits::Profile::Small);
    let after = config::read::after(&document, at).expect("overrides end");
    for index in (at + 1..after - 1).step_by(2) {
        let name = document.text(document.token(index).expect("override key")).expect("key text");
        let value = config::read::integer(&document, index + 1, u64::MAX).expect("override value");
        temper_service::limits::apply(&mut own, name, value).expect("init application limits");
    }
    (settings, own)
}

fn request(settings: &config::Engine, own: &temper_service::limits::Limits, bytes: usize, accepted: bool) {
    let seed = fixtures::SEED;
    let front = Joint::on_init(seed, settings, own);
    let mut world =
        World::<Temper, Joint>::new(Settings { seed, words: fixtures::decision_words(), ..Settings::default() }, front);
    world.randomness(2, 0, 0);
    let agent = fixtures::agent(&mut world);
    if !accepted {
        // This is an unsaved entrance refusal. Observe it before delivering a
        // final answer: the joint referee requires a durable key for finals.
        world.fault(FrontFault::NoAnswer);
    }
    let words = vec![b'a'; bytes];
    let number =
        fixtures::give(&mut world, agent, &[b"chat", b"start", b"--project", b"1", b"--key", b"probe", &words]);
    if accepted {
        let printed = fixtures::answered(&mut world, agent, number);
        let replies: Vec<_> = world
            .front()
            .engine
            .delivered
            .iter()
            .filter(|row| matches!(row, engine::Delivery::WebReply { .. }))
            .collect();
        assert_eq!(printed.status, 0, "{bytes} bytes: {printed:?}; engine replies: {replies:?}");
        let task = fixtures::done_task(&printed);
        assert_eq!(world.front().engine.task(task).expect("created chat").spec.words.as_ref(), words);
        for record in world.front().engine.store.rows.values() {
            if let engine::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { ask, .. })) = record
                && let people::Ask::Create { batch, .. } = ask.as_ref()
            {
                let mut member = batch[0].clone();
                let actual = people::member_bytes(&member).expect("member bytes");
                member.words = vec![b'a'; 4096].into();
                eprintln!(
                    "{bytes} request bytes, {actual} member bytes; 4096 request bytes need {} member bytes",
                    people::member_bytes(&member).expect("member bytes")
                );
            }
        }
    } else {
        for _ in 0..4096 {
            world.run(Until::Steps(1));
            if !world.front().entrance_refusals.is_empty() {
                break;
            }
        }
        assert_eq!(
            world.front().entrance_refusals,
            [people::Refusal::Limit { which: people::LimitKind::Words }],
            "{bytes} bytes must name people.words at the engine boundary"
        );
        assert_eq!(fixtures::chats(&world.front().engine), 0);
    }
}

#[test]
fn the_small_profiles_people_words_refuses_a_persons_chat_request() {
    let (mut settings, own) = init();
    settings.limits.core.people.words = 2048;
    for bytes in [2047, 2048, 2049, 2054] {
        request(&settings, &own, bytes, bytes <= 2048);
    }
}

#[test]
fn a_persons_chat_request_of_4_kib_starts_on_inits_box() {
    let (settings, own) = init();
    for bytes in [4095, 4096, 4097] {
        request(&settings, &own, bytes, bytes <= 4096);
    }
}
