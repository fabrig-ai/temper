//! Recorded green-light frames, edited through their owning codecs.
use jig_protocol_store::{self as store, codec, transcripts::frame};
use serde_json::{Value, json};
use skein_lib::{List, Reader, Writer};
use skein_scratch::Scratch;
use smith_transcript::{self as smith, Block, Message, MessageParts, Role, Turn, TurnParts};
use std::path::{Path, PathBuf};
use temper_twin::{Input, Refusal, Replace, Take, Unheld, twin};

const CHAT: u64 = 1;
const WORK: u64 = 2;
const CODER: u64 = 4;
const REVIEWER: u64 = 5;
const FIRST: u32 = 1;
const TASKS: [u64; 4] = [CHAT, WORK, CODER, REVIEWER];
const CUES: [&str; 4] =
    ["@green_chat Add CSV export to widget", "@green_work", "@green_coding", "Review this change at the named head"];
const AT_BOUND: usize = temper_fake::ENTRIES as usize;
const PAST_BOUND: usize = AT_BOUND + 1;

fn base() -> Box<[u8]> {
    serde_json::to_vec(&json!({
        "listen":{"socket":"127.0.0.1:0"}, "key":"/twin/key", "tls":null,
        "added_keys":["/twin/extra"], "control":{"socket":"127.0.0.1:0"},
        "models":[{"name":"fake", "window":65536,"output":16384}],
        "cued":[{"cue":"discard this script", "turns":["discard this answer"]}],
        "asked":[{"cue":"keep this request", "turns":["keep this answer"]}],
        "otherwise":["off script CSV"]
    }))
    .expect("base JSON")
    .into_boxed_slice()
}
fn path(data: &Path, task: u64) -> PathBuf {
    data.join("transcripts").join(format!("{:016x}", task >> 12)).join(format!("{task:016x}"))
}
fn fixture(tasks: &[u64]) -> (Scratch, Input) {
    let scratch = Scratch::new("twin-frames");
    let mut takes = Vec::new();
    for task in tasks {
        let file = path(scratch.path(), *task);
        std::fs::create_dir_all(file.parent().expect("task directory")).expect("task directory");
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(format!("{task:016x}")), file)
            .expect("recorded task frames");
        let position = TASKS.iter().position(|n| n == task).expect("fixture task");
        takes.push(Take { task: *task, cue: CUES[position].as_bytes().into() });
    }
    let input =
        Input { data: scratch.path().into(), base: base(), takes: takes.into_boxed_slice(), replaces: Box::new([]) };
    (scratch, input)
}
fn limits() -> codec::Limits {
    codec::Limits {
        settings: store::settings_v1::CEILINGS,
        core: store::core_v6::CEILINGS,
        tasks: store::tasks_v9::CEILINGS,
        people: store::people_v13::CEILINGS,
        notes: store::notes_v1::CEILINGS,
    }
}
fn records(input: &Input, task: u64) -> Vec<jig_domain_core::TurnRecord> {
    let bytes = std::fs::read(path(&input.data, task)).expect("task frames");
    let mut records = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().expect("frame length")) as usize;
        records.push(frame::decode(&bytes[at..at + length], &limits()).expect("recorded frame"));
        at += length;
    }
    records
}
fn save(input: &Input, task: u64, records: &[jig_domain_core::TurnRecord]) {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend_from_slice(&frame::encode(record, &limits()).expect("edited frame"));
    }
    std::fs::write(path(&input.data, task), bytes).expect("edited frames");
}
fn first(input: &Input) -> TurnParts {
    let records = records(input, CHAT);
    Turn::decode(&smith::CEILINGS, &mut Reader::new(&records[0].transcript)).expect("Smith turn").into_parts()
}
fn save_first(input: &Input, parts: TurnParts) {
    let turn = Turn::new(&smith::CEILINGS, parts).expect("edited Smith turn");
    let mut writer = Writer::new(turn.measure() as usize);
    turn.encode(&mut writer).expect("measured turn");
    let mut records = records(input, CHAT);
    let mut changed = 0;
    records[0].transcript = writer.finish();
    changed += 1;
    save(input, CHAT, &records);
    assert_eq!(changed, 1, "one recorded turn edited");
}
fn message(blocks: Vec<Block>) -> Message {
    let mut list = List::with_capacity(u32::try_from(blocks.len()).expect("small blocks"));
    for block in blocks {
        list.push(block).expect("sized blocks");
    }
    Message::new(&smith::CEILINGS, MessageParts { role: Role::Assistant, blocks: list }).expect("assistant message")
}
fn completion(parts: &mut TurnParts, blocks: Vec<Block>) {
    let mut messages = List::with_capacity(1);
    messages.push(message(blocks)).expect("one completion");
    parts.messages = messages;
}
fn text(words: &str) -> Block {
    Block::Text(
        smith::Text::new(&smith::CEILINGS, smith::TextParts { text: words.as_bytes().into(), replay: None })
            .expect("text"),
    )
}
fn call(arguments: &[u8]) -> Block {
    Block::Call(
        smith::Call::new(
            &smith::CEILINGS,
            smith::CallParts {
                id: b"recorded-call".as_slice().into(),
                name: b"delegate".as_slice().into(),
                input: arguments.into(),
                replay: None,
            },
        )
        .expect("call"),
    )
}
fn document(input: &Input) -> Value {
    serde_json::from_slice(&twin(input).expect("convert frames").document).expect("twin JSON")
}

#[test]
fn a_chat_and_its_delegates_become_cued_scripts() {
    let (_scratch, input) = fixture(&TASKS);
    let converted = twin(&input).expect("convert frames");
    assert_eq!(&*converted.report.turns, &[(CHAT, 6), (WORK, 5), (CODER, 4), (REVIEWER, 1)]);
    let doc: Value = serde_json::from_slice(&converted.document).expect("twin JSON");
    let before: Value = serde_json::from_slice(&input.base).expect("base JSON");
    for name in ["listen", "key", "tls", "models", "otherwise", "asked", "added_keys", "control"] {
        assert_eq!(doc[name], before[name], "base member {name}");
    }
    for (i, cue) in CUES.iter().enumerate() {
        assert_eq!(doc["cued"][i]["cue"], *cue);
    }
    assert_eq!(doc["cued"][0]["turns"][0]["tokens"], 20);
    assert!(temper_fake::llm::Document::decode(&converted.document).is_ok());
}

#[test]
fn a_call_keeps_its_recorded_arguments_and_finishes_tool_calls() {
    const ARGUMENTS: &[u8] = br#"{ "z": 2, "a" : "CSV" }"#;
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    completion(&mut parts, vec![text("before"), call(ARGUMENTS), text("after")]);
    save_first(&input, parts);
    let converted = twin(&input).expect("convert mixed completion");
    assert!(converted.document.windows(ARGUMENTS.len()).any(|bytes| bytes == ARGUMENTS));
    let doc: Value = serde_json::from_slice(&converted.document).expect("twin JSON");
    let turn = &doc["cued"][0]["turns"][0];
    assert_eq!(
        turn["lines"],
        json!([{"text":"before"},{"call":"delegate","arguments":{"z":2,"a":"CSV"}},{"text":"after"}])
    );
    assert_eq!(turn["finish"], "tool_calls");
}

#[test]
fn a_replace_applies_to_arguments_alone_and_is_counted() {
    let (_scratch, mut input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    completion(&mut parts, vec![text("CSV"), call(br#"{"a":"CSV CSV"}"#)]);
    save_first(&input, parts);
    input.replaces = Box::new([Replace { old: b"CSV".as_slice().into(), new: b"TSV".as_slice().into() }]);
    let converted = twin(&input).expect("replace arguments");
    // The first completion has two, and the chat's final finish has one.
    assert_eq!(&*converted.report.replaced, &[3]);
    let doc: Value = serde_json::from_slice(&converted.document).expect("twin JSON");
    assert_eq!(doc["cued"][0]["cue"], CUES[0]);
    assert_eq!(doc["cued"][0]["turns"][0]["lines"][0]["text"], "CSV");
    assert_eq!(doc["cued"][0]["turns"][0]["lines"][1]["arguments"]["a"], "TSV TSV");
    assert_eq!(doc["otherwise"], json!(["off script CSV"]));
}

#[test]
fn two_takes_with_one_cue_are_refused() {
    let (_scratch, mut input) = fixture(&[CHAT, WORK]);
    input.takes[1].cue = input.takes[0].cue.clone();
    assert_eq!(twin(&input).expect_err("duplicate cue"), Refusal::SameCue { cue: input.takes[0].cue.clone() });
}

#[test]
fn a_frame_cut_at_the_stop_is_refused_naming_its_task() {
    let (_scratch, input) = fixture(&[CHAT]);
    let file = path(&input.data, CHAT);
    let bytes = std::fs::read(&file).expect("frames");
    let length = u32::from_be_bytes(bytes[..4].try_into().expect("length")) as usize;
    std::fs::write(file, &bytes[..length - 1]).expect("cut first frame");
    assert_eq!(twin(&input).expect_err("cut frame"), Refusal::Frame { task: CHAT, at: 0 });
}

#[test]
fn a_missing_transcript_is_refused_with_its_path() {
    let (_scratch, input) = fixture(&[CHAT]);
    let file = path(&input.data, CHAT);
    std::fs::remove_file(&file).expect("remove transcript");
    assert_eq!(twin(&input).expect_err("missing transcript"), Refusal::Read { path: file });
}

#[test]
fn a_bad_checksum_is_refused_at_the_frame_offset() {
    let (_scratch, input) = fixture(&[CHAT]);
    let file = path(&input.data, CHAT);
    let mut bytes = std::fs::read(&file).expect("frames");
    let first_length = u32::from_be_bytes(bytes[..4].try_into().expect("length")) as usize;
    let second_length = u32::from_be_bytes(bytes[first_length..first_length + 4].try_into().expect("length")) as usize;
    bytes[first_length + second_length - 8] ^= 1;
    std::fs::write(file, bytes).expect("broken checksum");
    assert_eq!(twin(&input).expect_err("checksum"), Refusal::Frame { task: CHAT, at: first_length as u64 });
}

#[test]
fn an_older_frame_version_is_refused() {
    let (_scratch, input) = fixture(&[CHAT]);
    let file = path(&input.data, CHAT);
    let mut bytes = std::fs::read(&file).expect("frames");
    let length = u32::from_be_bytes(bytes[..4].try_into().expect("length")) as usize;
    // No current codec writes an old version: change its version word and CRC.
    bytes[4..6].copy_from_slice(&5_u16.to_be_bytes());
    let crc = frame::crc32c(&bytes[4..length - 8]);
    bytes[length - 8..length - 4].copy_from_slice(&crc.to_be_bytes());
    std::fs::write(file, bytes).expect("older frame");
    assert_eq!(twin(&input).expect_err("older version"), Refusal::Version { task: CHAT, version: 5 });
}

#[test]
fn an_older_smith_turn_version_is_refused() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut records = records(&input, CHAT);
    records[0].transcript[..2].copy_from_slice(&5_u16.to_be_bytes());
    save(&input, CHAT, &records);
    assert_eq!(twin(&input).expect_err("older Smith version"), Refusal::Version { task: CHAT, version: 5 });
}

#[test]
fn an_opaque_completion_is_refused_naming_task_and_turn() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    let opaque =
        smith::Opaque::new(&smith::CEILINGS, smith::OpaqueParts { dialect: 1, bytes: b"opaque".as_slice().into() })
            .expect("opaque");
    completion(&mut parts, vec![Block::Opaque(opaque)]);
    save_first(&input, parts);
    assert_eq!(
        twin(&input).expect_err("opaque completion"),
        Refusal::Turn { task: CHAT, turn: FIRST, why: Unheld::Opaque }
    );
}

#[test]
fn a_cut_call_is_refused_naming_task_and_turn() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    completion(&mut parts, vec![call(b"{\"unfinished\":")]);
    save_first(&input, parts);
    assert_eq!(twin(&input).expect_err("cut call"), Refusal::Turn { task: CHAT, turn: FIRST, why: Unheld::CutShort });
}

#[test]
fn a_compaction_is_refused_naming_task_and_turn() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    parts.kind = smith::TurnKind::Window;
    save_first(&input, parts);
    assert_eq!(
        twin(&input).expect_err("compaction"),
        Refusal::Turn { task: CHAT, turn: FIRST, why: Unheld::Compaction }
    );
}

#[test]
fn an_invalid_base_is_refused() {
    let (_scratch, mut input) = fixture(&[CHAT]);
    input.base = b"{}".as_slice().into();
    assert_eq!(twin(&input).expect_err("invalid base"), Refusal::Base);
}

#[test]
fn a_document_past_the_fakes_bounds_is_refused() {
    let (_scratch, mut input) = fixture(&[CHAT]);
    let mut takes = Vec::new();
    for i in 0..PAST_BOUND {
        takes.push(Take { task: CHAT, cue: format!("cue {i}").into_bytes().into_boxed_slice() });
    }
    input.takes = takes.into_boxed_slice();
    assert_eq!(twin(&input).expect_err("nine scripts"), Refusal::Bounds { what: "scripts" });
}

#[test]
fn eight_scripts_fit_the_fakes_bound() {
    let (_scratch, mut input) = fixture(&[REVIEWER]);
    let mut takes = Vec::new();
    for i in 0..AT_BOUND {
        takes.push(Take { task: REVIEWER, cue: format!("cue {i}").into_bytes().into_boxed_slice() });
    }
    input.takes = takes.into_boxed_slice();
    assert_eq!(document(&input)["cued"].as_array().expect("scripts").len(), AT_BOUND);
}

#[test]
fn an_absent_token_count_becomes_one_and_text_finishes_stop() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    let mut usage = parts.usage.into_parts();
    usage.output = None;
    parts.usage = smith::Usage::new(&smith::CEILINGS, usage).expect("optional usage");
    completion(&mut parts, vec![text("answer")]);
    save_first(&input, parts);
    let doc = document(&input);
    assert_eq!(doc["cued"][0]["turns"][0]["tokens"], 1);
    assert_eq!(doc["cued"][0]["turns"][0]["finish"], "stop");
}

#[test]
fn a_recorded_cut_off_result_refuses_even_valid_placeholder_arguments() {
    let (_scratch, input) = fixture(&[CHAT]);
    let mut parts = first(&input);
    completion(&mut parts, vec![call(b"{}")]);
    let problem = smith::CallProblem::CutOff(
        smith::CutProblem::new(&smith::CEILINGS, smith::CutProblemParts { bytes: 7 }).expect("cut problem"),
    );
    let invalid = smith::Invalid::new(&smith::CEILINGS, smith::InvalidParts { problem }).expect("invalid result");
    let result = smith::ToolResult::new(
        &smith::CEILINGS,
        smith::ToolResultParts { id: b"recorded-call".as_slice().into(), returned: smith::Returned::Invalid(invalid) },
    )
    .expect("cut result");
    let mut blocks = List::with_capacity(1);
    blocks.push(Block::Result(result)).expect("one result");
    let result = Message::new(&smith::CEILINGS, MessageParts { role: Role::User, blocks }).expect("result message");
    let mut messages = List::with_capacity(2);
    messages.push(parts.messages.as_slice()[0].clone()).expect("room for completion");
    messages.push(result).expect("room for result");
    parts.messages = messages;
    save_first(&input, parts);
    assert_eq!(
        twin(&input).expect_err("cut off result"),
        Refusal::Turn { task: CHAT, turn: FIRST, why: Unheld::CutShort }
    );
}
