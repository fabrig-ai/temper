//! The fake peer's listener comes from the kernel, not a fixture port.
use skein_fake_llm_domain::{self as fake, api};
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{llm, scripts};
#[global_allocator]
static HEAP: Counting = Counting;

// The ordinary scripted peer's string admission.
const STRINGS: u32 = 16_384;

#[test]
fn the_scripted_peer_listens_on_a_kernel_assigned_port() {
    let bound = llm::bound(|| llm::scripted(scripts::chat_scripts(1)));
    assert!(bound.ip().is_loopback() && bound.port() != 0, "{bound}");
}

fn script(cue: &[u8], replies: &[&[u8]]) -> api::Script {
    api::Script {
        cue: cue.into(),
        turns: replies
            .iter()
            .map(|text| api::Turn {
                lines: Box::new([api::Line::Text { text: (*text).into() }]),
                finish: api::Finish::Stop,
                tokens: 1,
            })
            .collect(),
    }
}

fn reply(recipe: &mut llm::Recipe, system: &[u8], brief: &[u8]) -> Box<[u8]> {
    let query = api::Query {
        credential: None,
        caching: api::Caching::Unscoped,
        model: b"fake".as_slice().into(),
        system: system.into(),
        tools: Box::new([]),
        choice: api::ToolChoice::Auto,
        messages: Box::new([api::Message {
            role: api::Role::User,
            parts: Box::new([api::Part::Text { text: brief.into() }]),
        }]),
        max_tokens: 32,
    };
    let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: recipe.domain_limits };
    let mut out = Queue::uncapped();
    fake::step(
        &mut recipe.domain,
        &env,
        fake::Event::Call { reply_to: ReplyTo::new(Token::new(1)), call: Token::new(1), query },
        &mut out,
    );
    while out.is_empty() {
        env.now = recipe.domain.next_deadline().expect("scheduled reply");
        fake::fire(&mut recipe.domain, &env, &mut out);
    }
    let fake::Request::Reply { result, .. } = out.pop().expect("reply") else { panic!("unheld reply") };
    let answer = result.expect("accepted query");
    let api::Part::Text { text } = answer.parts.into_vec().remove(0) else { panic!("scripted text") };
    text
}

#[test]
fn an_open_script_answers_the_brief_and_a_finite_script_keeps_its_system_cue() {
    let mut recipe = llm::scripted_open(
        Box::new([script(b"person's request", &[b"Open reply"]), script(b"finite task", &[b"Finite reply"])]),
        STRINGS,
    );
    for (system, brief, expected) in [
        (b"Reply to the person".as_slice(), b"## Task\n\nperson's request".as_slice(), b"Open reply".as_slice()),
        (b"## Task\n\nfinite task".as_slice(), b"person's request".as_slice(), b"Finite reply".as_slice()),
    ] {
        assert_eq!(reply(&mut recipe, system, brief).as_ref(), expected);
    }
}

#[test]
fn open_input_rules_answer_a_later_person_message_without_the_original_cue() {
    let rules =
        |rows: &[(&[u8], u32)]| rows.iter().map(|(cue, turn)| api::Rule { cue: (*cue).into(), turn: *turn }).collect();
    let mut recipe = llm::played_by_input_open(
        Box::new([script(b"original request", &[b"First reply", b"Later reply", b"Waiting"])]),
        Box::new([rules(&[(b"original request", 0), (b"New direction", 1), (b"", 2)])]),
    );
    for (words, expected) in [
        (b"## Task\n\noriginal request".as_slice(), b"First reply".as_slice()),
        (b"New direction".as_slice(), b"Later reply".as_slice()),
    ] {
        assert_eq!(reply(&mut recipe, b"Reply to the person", words).as_ref(), expected);
    }
}
