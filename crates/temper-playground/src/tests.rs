use crate::{A, world};
use skein_lib::{Duration, Time, Wall};
use skein_sim_world::{Printed, Stream};

const START: Wall = Wall::from_nanos(1_700_000_000_000_000_000);
const START_STEPS: u64 = 10_000;
const START_SECONDS: Duration = Duration::from_secs(10);

// The sole bare loop: step while busy, otherwise move exactly to the next due instant.
fn printed_link() -> (Vec<Printed>, u64, Time, Wall) {
    let mut world = world(&A, 7, START);
    for _ in 0..START_STEPS {
        if !world.busy() {
            world.advance(world.next_due().expect("something due before the sign-in link"));
        }
        assert!(world.now() <= Time::ZERO.saturating_add(START_SECONDS), "link within simulated startup budget");
        world.step();
        let lines = world.printed();
        if !lines.is_empty() {
            return (lines, world.steps(), world.now(), world.wall());
        }
    }
    panic!("sign-in link within startup step budget");
}
#[test]
fn story_a_starts_and_the_service_prints_its_link() {
    let (lines, _, _, _) = printed_link();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].process.as_ref(), b"temper");
    assert_eq!(lines[0].stream, Stream::Output);
    let prefix = format!("http://{}/jig/v1/sign-in/link?secret=", A.clients());
    let line = std::str::from_utf8(&lines[0].line).expect("link text");
    let secret = line.strip_prefix(&prefix).expect("listener is the link's host");
    assert!(!secret.is_empty());
    assert!(!secret.bytes().any(|byte| byte.is_ascii_whitespace()));
}
#[test]
fn the_same_seed_starts_the_same_world() {
    assert_eq!(printed_link(), printed_link());
}
#[test]
fn story_a_documents_decode() {
    // Service::start performs the deployment's startup checks before returning it.
    let world = world(&A, 7, START);
    assert!(matches!(world.processes()[0], crate::Process::Service(_)));
    smith_agent_shell::config::parse(A.agent).expect("agent document");
    let llm = temper_fake::llm::Document::decode(A.llm).expect("LLM document");
    let forge = temper_fake::forge::Document::decode(A.forge).expect("forge document");
    assert_eq!(llm.reply(), Duration::from_millis(1500));
    assert_eq!(forge.ci(), Duration::from_millis(5000));
    assert!(llm.root().is_none());
    assert!(forge.repositories()[0].ci.is_none());
}
#[test]
fn story_a_scripts_are_the_templates() {
    let template: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../temper-init/template/temper.json")).expect("template");
    let llm: serde_json::Value = serde_json::from_slice(A.llm).expect("story LLM");
    let demo = &template["application"]["demo"]["llm"];
    let cued: Vec<_> = demo["asked"]
        .as_array()
        .expect("asked scripts")
        .iter()
        .chain(demo["cued"].as_array().expect("cued scripts"))
        .cloned()
        .collect();
    assert_eq!(llm["asked"], serde_json::json!([]));
    assert_eq!(llm["cued"], serde_json::json!(cued));
    let deployment: serde_json::Value = serde_json::from_slice(A.deployment).expect("deployment");
    for member in ["asked", "cued", "otherwise"] {
        assert_eq!(deployment["application"]["demo"]["llm"][member], demo[member]);
    }
    assert_ne!(llm["otherwise"], template["application"]["demo"]["llm"]["otherwise"]);
}

#[test]
fn story_a_cues_match_apart() {
    use skein_fake_llm_domain::api::{Finish, Part};
    // The same isolated-script domain check as temper-fake's
    // the_csv_cues_select_only_their_own_agents_briefs, for every story cue.
    let source: serde_json::Value = serde_json::from_slice(A.llm).expect("story LLM");
    let scripts: Vec<_> = ["asked", "cued"]
        .into_iter()
        .flat_map(|list| source[list].as_array().expect("scripts").iter().map(move |script| (list, script)))
        .collect();
    for (list, script) in &scripts {
        let mut isolated = source.clone();
        isolated["asked"] = serde_json::json!([]);
        isolated["cued"] = serde_json::json!([]);
        isolated[*list] = serde_json::json!([script]);
        let document = temper_fake::llm::Document::decode(&serde_json::to_vec(&isolated).expect("isolated JSON"))
            .expect("isolated script within fake bounds");
        let cue = script["cue"].as_str().expect("cue");
        for (_, other) in &scripts {
            let other_cue = other["cue"].as_str().expect("other cue");
            let system = format!("Begin the work your brief describes: {other_cue}.");
            let answer = answer(&document, system.as_bytes());
            if cue == other_cue {
                let lines = script["turns"][0]["lines"].as_array().expect("first turn lines");
                assert_eq!(answer.parts.len(), lines.len(), "selected {cue}");
                for (part, expected) in answer.parts.iter().zip(lines) {
                    match part {
                        Part::Text { text } => {
                            assert_eq!(text.as_ref(), expected["text"].as_str().expect("text").as_bytes());
                        }
                        Part::ToolCall { name, arguments, .. } => {
                            assert_eq!(name.as_ref(), expected["call"].as_str().expect("call").as_bytes());
                            assert_eq!(
                                serde_json::from_slice::<serde_json::Value>(arguments).expect("tool JSON"),
                                expected["arguments"]
                            );
                        }
                        Part::Opaque { .. } | Part::ToolOutput { .. } => panic!("script emits text or a call"),
                    }
                }
            } else {
                let [Part::Text { text }] = answer.parts.as_ref() else { panic!("fallback text alone") };
                assert_eq!(
                    text.as_ref(),
                    source["otherwise"][0]["lines"][0]["text"].as_str().expect("fallback").as_bytes(),
                    "{cue} absent from {other_cue}"
                );
                assert_eq!(answer.finish, Finish::Stop);
            }
        }
    }
}
fn answer(document: &temper_fake::llm::Document, system: &[u8]) -> skein_fake_llm_domain::api::Answer {
    use skein_fake_llm_domain::{self as llm, api};
    use skein_lib::{Env, Queue, ReplyTo, Token};
    let mut domain = document.domain(23);
    let limits = document.limits();
    let mut env = Env { now: Time::ZERO, wall: START, limits };
    let mut out = Queue::with_capacity(1);
    let query = api::Query {
        credential: Some(0),
        model: b"demo".as_slice().into(),
        system: system.into(),
        messages: Box::new([api::Message {
            role: api::Role::User,
            parts: Box::new([api::Part::Text { text: b"Begin the work your brief describes.".as_slice().into() }]),
        }]),
        tools: Box::new([]),
        choice: api::ToolChoice::None,
        max_tokens: 8192,
        caching: api::Caching::Unscoped,
    };
    llm::step(
        &mut domain,
        &env,
        llm::Event::Call { reply_to: ReplyTo::new(Token::new(1)), call: Token::new(2), query },
        &mut out,
    );
    env.now = Time::ZERO.saturating_add(document.reply());
    llm::fire(&mut domain, &env, &mut out);
    match out.pop().expect("paced reply") {
        llm::Request::Reply { result, .. } => result.expect("script answer"),
        llm::Request::Piece { .. } | llm::Request::Hold { .. } => panic!("whole answer"),
    }
}
