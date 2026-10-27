//! M5 QA-B: FILE refusals and an open client during an account move.
pub mod support;

const MOVED_PARTS: usize = 6; // protocol/llm.md 5.6: five outer fields and a record

#[test]
fn refused_files_leave_the_account_with_its_holder() {
    let clock = skein_shell::Clock::new();
    let started = clock.now().now;
    let scratch = support::Scratch::new("qa_b_account_file");
    support::account_file_qa::files(scratch.path());
    scratch.remove();
    eprintln!("QA-B FILE probes: {:?}", clock.now().now.saturating_since(started));
}

#[test]
fn an_open_terminal_sees_the_move_and_the_tasks_moving_wait() {
    let scratch = support::Scratch::new("qa_b_account_terminal");
    support::account_file_qa::terminal(scratch.path());
    scratch.remove();
}

#[test]
fn an_old_jigm_imports_and_its_next_export_uses_the_new_form() {
    use support::account_move_qa::{Fixture, success};
    let fixture = Fixture::new("qa-old-jigm-conversion");
    success(&fixture.a().call(&["account", "export", &fixture.number(), fixture.path()], None));
    let exported = std::fs::read(&fixture.file).expect("synthetic account export");
    support::write_private(&fixture.file, &old_encoder(&exported));
    success(&fixture.b.call(&["account", "import", fixture.path(), "--key", "qa-old-jigm"], None));
    assert!(!fixture.file.exists(), "old file spent after adoption");
    let account = fixture.b.account_state("granted");
    let number = account["number"].as_u64().expect("adopted account number").to_string();
    success(&fixture.b.call(&["account", "export", &number, fixture.path()], None));
    let converted = std::fs::read(&fixture.file).expect("next export");
    let converted_parts = exported_parts(&converted);
    let _record = skein_oauth::decode_record(converted_parts[5], &record_limits())
        .expect("next export's nested record uses the schema form");
}

// The old encoder at jig 365afd554^: JIGM, u16 version 1 and six
// u32-prefixed parts. The nested record follows skein cad332f2 record.rs,
// not the later schema codec. Preserve only the synthetic export's runtime
// issuer/client/subject and token bytes; populate every optional field.
// No current encoder makes this old input, and no held login is used.
fn old_encoder(exported: &[u8]) -> Box<[u8]> {
    use skein_lib::{Wall, Writer};
    const OUTER_FIXED: usize = 30; // JIGM + u16 + six u32 lengths
    let mut parts = exported_parts(exported);
    let mut record = skein_oauth::decode_record(parts[5], &record_limits()).expect("synthetic exported record");
    record.metadata = Some(b"m".as_slice().into());
    record.issued_client_id = Some(parts[2].into());
    record.subject = Some(parts[3].into());
    if record.granted_scopes.is_none() {
        record.granted_scopes = Some(b"openid".as_slice().into());
    }
    if record.id_token.is_none() {
        record.id_token = Some(b"i".as_slice().into());
    }
    record.earliest_refresh_at = Some(Wall::from_nanos(100));
    let old_record = old_record(&record);
    parts[5] = &old_record;
    let size = parts.iter().try_fold(OUTER_FIXED, |size, part| size.checked_add(part.len())).expect("old file size");
    let mut out = Writer::new(size);
    out.put(b"JIGM").expect("room");
    out.put(&1_u16.to_be_bytes()).expect("room");
    for part in parts {
        old_field(&mut out, part);
    }
    out.finish()
}

fn record_limits() -> skein_oauth::Limits {
    jig_protocol_config::profile(jig_protocol_config::Profile::Small).driver.client.document
}

fn exported_parts(exported: &[u8]) -> [&[u8]; MOVED_PARTS] {
    use skein_lib::Reader;
    let mut input = Reader::new(exported);
    assert_eq!(input.u16(), Some(1), "export uses the generated outer form");
    let mut parts = [b"".as_slice(); MOVED_PARTS];
    for part in &mut parts {
        let len = input.u32().expect("part length");
        *part = input.bytes(len).expect("part");
    }
    assert!(input.is_empty(), "all six exported parts retained");
    parts
}

fn old_record(record: &skein_oauth::SavedToken) -> Box<[u8]> {
    use skein_lib::Writer;
    // SKOT + u16 + key/generation/expiry + seven u32 lengths + five
    // presence marks + earliest wall: 6 + 20 + 28 + 5 + 8 = 67.
    const FULL_FIXED: usize = 67;
    const UNMARKED_FIELDS: usize = 3; // access, refresh, metadata precede marked additions
    let fields = [
        record.access_token.as_ref(),
        record.refresh_token.as_deref().expect("export can refresh"),
        record.metadata.as_deref().expect("full metadata"),
        record.issued_client_id.as_deref().expect("full issued client"),
        record.granted_scopes.as_deref().expect("full scopes"),
        record.subject.as_deref().expect("full subject"),
        record.id_token.as_deref().expect("full ID token"),
    ];
    let size =
        fields.iter().try_fold(FULL_FIXED, |size, field| size.checked_add(field.len())).expect("old record size");
    let mut out = Writer::new(size);
    out.put(b"SKOT").expect("room");
    out.put(&3_u16.to_be_bytes()).expect("room");
    out.put(&record.key.to_be_bytes()).expect("room");
    out.put(&record.generation.to_be_bytes()).expect("room");
    out.put(&record.expires_at.as_nanos().to_be_bytes()).expect("room");
    for field in &fields[..UNMARKED_FIELDS] {
        old_field(&mut out, field);
    }
    for field in &fields[UNMARKED_FIELDS..] {
        out.put(&[1]).expect("room");
        old_field(&mut out, field);
    }
    out.put(&[1]).expect("room");
    out.put(&record.earliest_refresh_at.expect("full earliest refresh").as_nanos().to_be_bytes()).expect("room");
    out.finish()
}

fn old_field(out: &mut skein_lib::Writer, field: &[u8]) {
    out.put(&u32::try_from(field.len()).expect("bounded field").to_be_bytes()).expect("room");
    out.put(field).expect("room");
}
