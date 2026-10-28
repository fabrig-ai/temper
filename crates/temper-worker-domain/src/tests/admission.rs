//! Every entrance's admission class, with a small pacing bound.
use super::fixtures::{self, ATTEMPT, RUN};
use crate::{Encoded, Event, Invalid, admits, agent, checkout, host, is_new_work};
use alloc::boxed::Box;
use skein_lib::Token;

const OWNER: Token = Token::new(23);
const MESSAGE: Token = Token::new(24);
// One in-flight hand-out can reach this scenario's pacing bound.
const WATERMARK: u32 = 1;

#[test]
fn new_work_is_an_assignment_a_message_a_check_or_a_sign_out() {
    let source = host::SourceName::new(b"alice").unwrap();
    let rows = [
        (
            Event::Assign {
                assignment: fixtures::assignment(false, Box::new([])),
                items: Ok(fixtures::items(&[])),
                save: None,
            },
            true,
        ),
        (Event::Link(host::LinkDown::Assign { assignment: fixtures::assignment(false, Box::new([])) }), true),
        (
            Event::Link(host::LinkDown::Message {
                run: RUN,
                attempt: ATTEMPT,
                name: MESSAGE,
                sender: b"person".as_slice().into(),
                words: b"hello".as_slice().into(),
                finish_requested: false,
            }),
            true,
        ),
        (Event::Link(host::LinkDown::Check { name: source }), true),
        (Event::Link(host::LinkDown::SignOut { name: source }), true),
        (Event::Link(host::LinkDown::Sources { sources: Box::new([source]) }), false),
        (
            Event::Link(host::LinkDown::Relayed {
                run: RUN,
                attempt: ATTEMPT,
                call: b"call".as_slice().into(),
                answer: host::Relayed::Busy,
            }),
            false,
        ),
        (Event::Link(host::LinkDown::TurnKept { run: RUN, attempt: ATTEMPT, turn: 1 }), false),
        (Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }), false),
        (Event::Link(host::LinkDown::Cancel { run: RUN, attempt: ATTEMPT }), false),
        (
            Event::Link(host::LinkDown::Grant {
                run: RUN,
                attempt: ATTEMPT,
                grant: fixtures::grant(host::GrantTable::Accounts, fixtures::ACCOUNT),
            }),
            false,
        ),
        (
            Event::SourceAnswered {
                owner: OWNER,
                answer: host::sources::Answer::SignedOut(host::SignOutResult::SignedOut),
            },
            false,
        ),
        (Event::Ready, false),
        (Event::Lost, false),
        (Event::GracePassed, false),
        (Event::Resend { run: RUN, attempt: ATTEMPT, turn: 1 }, false),
        (Event::RelayCancelled { run: RUN, attempt: ATTEMPT, call: b"call".as_slice().into() }, false),
        (Event::Process(Box::new(agent::process::Event::Hangup { owner: OWNER })), false),
        (
            Event::Live {
                agent: OWNER,
                piece: host::Piece {
                    turn: 1,
                    retry: 0,
                    kind: host::LiveKind::Text,
                    offset: 0,
                    bytes: b"hello".as_slice().into(),
                },
            },
            false,
        ),
        (Event::Decoded { agent: OWNER, start: Err(Invalid::Charter) }, false),
        (Event::Encoded { owner: OWNER, encoded: Encoded::Undecodable }, false),
        (Event::Activity { owner: OWNER, activity: None }, false),
        (Event::Done { owner: OWNER, done: checkout::git::Done::Succeeded }, false),
        (Event::Shutdown, false),
    ];
    for (event, new_work) in rows {
        assert_eq!(is_new_work(&event), new_work, "{event:?}");
    }
}

#[test]
fn new_work_waits_at_or_above_the_watermark() {
    let limits = crate::Limits { watermark: WATERMARK, ..fixtures::limits() };
    for (backlog, taken) in [(WATERMARK - 1, true), (WATERMARK, false), (WATERMARK + 1, false)] {
        assert_eq!(admits(&limits, backlog), taken);
    }
}
