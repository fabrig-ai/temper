//! A workspace-free advisor discovers paths from its forge answers.
use super::*;
use skein_fake_llm_domain::{self as llm, api as model};
use skein_lib::{Env, Queue, Wall};

const CUE: &[u8] = b"directory-advisor";
const SOURCE: &[u8] = b"fn main() {}";
const REPORT: &[u8] = b"Found src/main.rs: fn main() {}";
const MODEL_CALL: Token = Token::new(77);
// Root, src, src/main.rs, then the report.
const COMPLETIONS: u32 = 4;

struct Model {
    domain: llm::Domain,
    env: Env<llm::Config>,
    messages: Vec<model::Message>,
}
impl Model {
    fn new(head: &[u8]) -> Self {
        let mut digits = String::new();
        for byte in head {
            write!(&mut digits, "{byte:02x}").expect("head digits");
        }
        let turns = [".", "src", "src/main.rs"].map(|path| model::Turn {
            lines: Box::new([model::Line::Call {
                name: b"read_file".as_slice().into(),
                arguments: format!(r#"{{"head":"{digits}","path":"{path}"}}"#).into_bytes().into_boxed_slice(),
            }]),
            finish: model::Finish::ToolCalls,
            tokens: 1,
        });
        let mut turns = Vec::from(turns);
        turns.push(model::Turn {
            lines: Box::new([model::Line::Text { text: REPORT.into() }]),
            finish: model::Finish::Stop,
            tokens: 1,
        });
        let scripts = model::Scripts {
            cued: Box::new([model::Script { cue: CUE.into(), turns: turns.into_boxed_slice() }]),
            asked: Box::from([]),
            otherwise: Box::from([]),
        };
        let rules = model::Rules {
            cued: Box::new([Box::new([
                model::Rule { cue: SOURCE.into(), turn: 3 },
                model::Rule { cue: b"main.rs".as_slice().into(), turn: 2 },
                model::Rule { cue: b"src/".as_slice().into(), turn: 1 },
                model::Rule { cue: Box::from([]), turn: 0 },
            ])]),
            asked: Box::from([]),
        };
        let config = temper_fake::llm::LIMITS;
        let domain = llm::Domain::played_by_input(
            &config,
            1,
            scripts,
            model::Menu { arguments: Box::from([]), invalid: Box::from([]) },
            rules,
        )
        .expect("bounded model script");
        Self {
            domain,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: config },
            messages: vec![model::Message {
                role: model::Role::User,
                parts: Box::new([model::Part::Text { text: b"Find the program's entry point".as_slice().into() }]),
            }],
        }
    }
    fn next(&mut self) -> model::Answer {
        // The fake's public configuration supplies all bounds; this query fits them.
        let query = model::Query {
            credential: None,
            caching: model::Caching::Unscoped,
            model: b"fake".as_slice().into(),
            system: CUE.into(),
            tools: Box::new([model::ToolSpec {
                name: b"read_file".as_slice().into(),
                description: b"Read repository paths".as_slice().into(),
                parameters: b"{}".as_slice().into(),
            }]),
            choice: model::ToolChoice::Auto,
            messages: self.messages.clone().into_boxed_slice(),
            max_tokens: 1024,
        };
        let mut out = Queue::with_capacity(llm::MAX_OUT);
        llm::step(
            &mut self.domain,
            &self.env,
            llm::Event::Call { reply_to: ReplyTo::new(MODEL_CALL), call: MODEL_CALL, query },
            &mut out,
        );
        if out.is_empty() {
            llm::fire(&mut self.domain, &self.env, &mut out);
        }
        let llm::Request::Reply { result, .. } = out.pop().expect("scripted model reply") else {
            panic!("whole scripted answer")
        };
        self.domain.reclaim();
        result.expect("scripted completion")
    }
}

#[test]
fn an_advisor_lists_the_root_and_finds_src_main_rs() {
    let (mut world, chat) = world_with(&defaults());
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"README", b"Repository guide", 1)
        .expect("root file");
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"src/main.rs", SOURCE, 1)
        .expect("entry point");
    let work_task = goal(&mut world, &chat, false);
    answer(&mut world, &chat, CHAT_TURNS, tasks::End::Parked);
    let work = admitted(&world, work_task);
    let advisor_task = goal(&mut world, &work, true);
    answer(&mut world, &work, WORK_TURNS, tasks::End::Parked);
    let advisor = admitted(&world, advisor_task);
    let mut model = Model::new(&translate::commit(head));
    let mut paths = Vec::new();
    for turn in 0..COMPLETIONS {
        let completion = model.next();
        match completion.parts.as_ref() {
            [model::Part::ToolCall { id, name, arguments }] => {
                let input: serde_json::Value = serde_json::from_slice(arguments).expect("scripted read input");
                paths.push(input["path"].as_str().expect("path").to_owned());
                let answer = super::super::change_tool::named_read(&mut world, &advisor, 110 + turn, name, arguments);
                let rendered = temper_engine_smith::answer(&answer);
                assert!(!rendered.error(), "{answer:?}");
                model.messages.push(model::Message { role: model::Role::Assistant, parts: completion.parts.clone() });
                model.messages.push(model::Message {
                    role: model::Role::User,
                    parts: Box::new([model::Part::ToolOutput {
                        id: id.clone(),
                        output: rendered.text().into(),
                        is_error: false,
                    }]),
                });
            }
            [model::Part::Text { text }] => {
                assert_eq!(text.as_ref(), REPORT);
                assert_eq!(paths, [".", "src", "src/main.rs"]);
                report(&mut world, &advisor, ADVISOR_TURNS, text);
                assert!(has_report(&world, work_task, advisor_task, REPORT));
                return;
            }
            other => panic!("unexpected model completion: {other:?}"),
        }
    }
    panic!("advisor did not report its discovery");
}
