use crate::tests::fixtures;
use jig_client::{
    Readout,
    contract::{ReadNone, ReadResult},
};
#[test]
fn a_turn_past_the_read_bound_exits_0() {
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let call = root.submit(fixtures::TURN_CALL);
    root.serve(call, fixtures::turn_too_large(fixtures::TOO_LARGE));
    let printed = root.answer(call).expect("sized answer");
    assert_eq!((printed.status, printed.stdout), (0, fixtures::golden("turn_limit")));
}
#[test]
fn a_large_page_turn_prints_its_read_call() {
    let printed = fixtures::read_answer(fixtures::ACTIVITY_CALL, fixtures::activity_too_large());
    assert_eq!(printed.stdout, fixtures::golden("activity_turn"));
}
#[test]
fn a_turn_part_is_cut_with_its_continuation() {
    let printed = fixtures::read_answer(fixtures::TURN_CALL, fixtures::turn_text(fixtures::LONG_PART));
    assert_eq!(printed.stdout, fixtures::golden("turn_part"));
}
#[test]
fn a_turn_part_at_the_bound_is_complete() {
    let printed = fixtures::read_answer(fixtures::TURN_CALL, fixtures::turn_text(fixtures::PART_SIZE));
    assert_eq!(printed.stdout, fixtures::golden("turn_part_complete"));
}
#[test]
fn a_turn_continuation_starts_at_its_byte_offset() {
    let printed = fixtures::read_answer(fixtures::TURN_NEXT_CALL, fixtures::turn_text(fixtures::LONG_PART));
    assert_eq!(printed.stdout, fixtures::golden("turn_part_next"));
}
#[test]
fn client_document_limits_exit_1() {
    for readout in [Readout::Limit, Readout::Jig(ReadResult::None(ReadNone::TooLarge { bytes: fixtures::TOO_LARGE }))] {
        let (mut root, call) = fixtures::read_call(fixtures::TASK_CALL);
        root.serve_readout(call, readout);
        assert_eq!(root.answer(call).expect("limit answered").status, 1);
    }
}
#[test]
fn chat_prints_only_the_hosts_provisional_text() {
    let (mut root, call) = fixtures::read_call(fixtures::CHAT_CALL);
    root.serve(call, fixtures::activity_too_large());
    root.deliver_run(fixtures::no_provisional());
    assert_eq!(root.answer(call).expect("chat answered").stdout, fixtures::golden("activity_turn"));
}

#[test]
fn a_run_closes_its_watch_at_its_first_delivery() {
    let (mut root, call) = fixtures::read_call(fixtures::RUN_CALL);
    root.deliver_run(fixtures::no_provisional());
    assert_eq!(root.closed.len(), fixtures::PAGE_ROWS);
    let _printed = root.answer(call).expect("run answered");
}

#[test]
fn a_turn_prints_its_supplied_blocks() {
    let printed = fixtures::read_answer(fixtures::TURN_CALL, fixtures::turn_call(None));
    assert_eq!(printed.stdout, fixtures::golden("turn_call"));
}
#[test]
fn a_path_is_never_cut() {
    let printed = fixtures::read_answer(fixtures::TURN_CALL, fixtures::turn_call(Some(fixtures::long_path())));
    assert_eq!(printed.stdout, fixtures::golden("turn_path"));
}
#[test]
fn a_json_turn_part_carries_its_continuation() {
    let printed = fixtures::read_answer(fixtures::TURN_JSON_CALL, fixtures::turn_text(fixtures::LONG_PART));
    assert_eq!(printed.stdout, fixtures::golden("turn_part_json"));
}

#[test]
fn run_prints_the_hosts_provisional_text() {
    let (mut root, call) = fixtures::read_call(fixtures::RUN_CALL);
    root.deliver_run(fixtures::provisional());
    assert_eq!(root.answer(call).expect("run answered").stdout, fixtures::golden("run_provisional"));
}

#[test]
fn a_document_sized_turn_prints_escaped_tool_names_with_the_shipped_limits() {
    use crate::limits;
    use jig_client::contract as c;
    use skein_lib::List;

    // Leave 256 bytes per call for its source framing. Each retained tool
    // name fits TEXT; together their six-byte NUL escapes pass ask_bytes.
    let tool_bytes = limits::TEXT.checked_sub(256).expect("room for call framing");
    let mut blocks = List::with_capacity(limits::ROWS);
    for number in 0..limits::ROWS {
        blocks
            .push(c::Block::Call(c::Call {
                number: u64::from(number),
                tool: c::Text::new(fixtures::worst_case_json_text(tool_bytes), limits::TEXT)
                    .expect("tool within shipped text bound"),
                input: c::Text::new(Box::default(), limits::TEXT).expect("empty input"),
                outcome: None,
                failed: false,
                objects: List::with_capacity(0),
                parts: List::with_capacity(0),
            }))
            .expect("calls within shipped block bound");
    }
    let printed = fixtures::read_answer(
        &["task", "turn", fixtures::TASK_CALL[2], "3.7", "--json"],
        fixtures::turn_blocks(blocks),
    );

    assert_eq!(printed.status, 0, "{:?}", printed.stderr);
    let escaped = tool_bytes.checked_mul(limits::ROWS).expect("bounded document");
    assert_eq!(fixtures::occurrences(&printed.stdout, br"\u0000"), usize::try_from(escaped).expect("count fits"));
}
