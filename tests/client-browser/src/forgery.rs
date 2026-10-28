//! A forged chat over jig's `tests/client-browser/src/forgery.rs`, with the chat template's task
//! (client/browser.md, 4.1 story 5; jig's protocol/browser.md, 15).
use jig_client::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Writer};

const PROJECT: u32 = 1;
const CHAT_CHARTER: u32 = 1;
const SPEC_WORDS: u32 = 32;
const RESULT_WORDS: u32 = 32;
const LABEL: u32 = 1;
const REQUEST_KEY: [u8; 16] = [0x46; 16];

fn task() -> c::NewTask {
    c::NewTask {
        deadline: c::NewDeadline::Named,
        executor: c::OwnerExecutor::Agent { charter: CHAT_CHARTER },
        spec: c::Specification {
            asks: false,
            words: c::Text::new(b"Forged".as_slice().into(), SPEC_WORDS).expect("bounded specification"),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
        },
        contract: c::NewContract::Report { words: RESULT_WORDS },
        authority: c::OwnerAuthority {
            models: None,
            tools: 0,
            grants: List::with_capacity(0),
            delegation: c::OwnerDelegation { kinds: List::with_capacity(0), tasks: 0, depth: 0 },
            spend: 0,
            deadline: None,
            notes: 0,
            note_resources: List::with_capacity(0),
        },
        budget: c::NewBudget::AsAllowed { cap: None },
        wake: None,
        dependencies: List::with_capacity(0),
        named: List::with_capacity(0),
        tracked: None,
        label: LABEL,
        flags: c::TaskFlags {
            ended_by: jig_client::contract::EndedBy::Result,
            conversation: true,
            listed: false,
            retention: c::Retention::Kept,
        },
        llm: None,
    }
}

pub(super) fn request() -> Box<[u8]> {
    let mut tasks = List::with_capacity(1);
    tasks.push(task()).expect("one forged task");
    let ask = jig_client_protocol_link::translate::asks::ask(&c::Ask::Create { project: PROJECT, tasks }, &w::CEILINGS)
        .expect("forged request fits wire");
    let mut writer = Writer::new(usize::try_from(ask.measure()).expect("request size fits"));
    ask.encode(&mut writer).expect("measured request");
    let keyed = w::Keyed::new(
        &w::CEILINGS,
        w::KeyedParts {
            key: REQUEST_KEY,
            owner: w::Owner::Jig,
            version: jig_client_protocol_wire::VERSION,
            request: writer.finish(),
            value: None,
        },
    )
    .expect("bounded keyed request");
    let mut writer = Writer::new(usize::try_from(keyed.measure()).expect("keyed size fits"));
    keyed.encode(&mut writer).expect("measured keyed request");
    writer.finish()
}
