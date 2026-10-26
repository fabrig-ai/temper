use super::*;

#[test]
fn a_capture_preserves_all_pages_empty_directories_modes_and_bytes() {
    // More than the capture's 32-entry directory page, with a raw byte name.
    const FILES: u32 = 40;
    let mut items = vec![Item::directory(b"empty").mode(0o750), Item::directory(b"nested")];
    items.push(Item::file(b"nested/\xff", b"\0kept\xff").mode(0o444).owner(1001));
    for number in 0..FILES {
        items.push(Item::file(format!("file-{number:02}").as_bytes(), b"").mode(0o600));
    }
    let mut machine = Machine::for_user(1000);
    let root = machine.lay(&items);
    machine.close(root);
    let startup = Startup {
        configuration: b"kept configuration".as_slice().into(),
        entries: BTreeMap::from([(b"private/input".as_slice().into(), b"kept startup bytes".as_slice().into())]),
    };
    let capture = Deployment::capture(&mut machine, &startup);
    let expected: BTreeMap<_, _> = items
        .into_iter()
        .map(|item| {
            let (kind, bytes) = match item.made {
                skein_fake_machine::Made::File(bytes) => (Is::File, bytes.into_boxed_slice()),
                skein_fake_machine::Made::Directory => (Is::Directory, Box::default()),
                skein_fake_machine::Made::Link(_)
                | skein_fake_machine::Made::HardLink(_)
                | skein_fake_machine::Made::Fifo
                | skein_fake_machine::Made::Device => unreachable!("fixture files and directories"),
            };
            (item.path.into_boxed_slice(), Entry { kind, mode: item.mode, owner: item.owner.unwrap_or(1000), bytes })
        })
        .collect();
    assert_eq!(capture.files, expected);
    let mut fresh = capture.machine();
    assert_eq!(Deployment::capture(&mut fresh, &capture.startup()), capture);
}

#[test]
fn application_secrets_are_decoded_by_their_numbered_item() {
    let item = temper_protocol_records::Item::ApiToken;
    let number = temper_protocol_records::number(1, item);
    let mut key = skein_kv::KeyWriter::new(u32::MAX);
    key.tag(2).tag(0).u64(number);
    let rows = Rows::from([(
        key.finish().expect("secret key"),
        temper_protocol_records::encode(item, fixtures::API_TOKEN).expect("secret encoding").into(),
    )]);
    let (jig, application) = decoded_secrets(&rows);
    assert!(jig.is_empty());
    assert_eq!(application, BTreeMap::from([(number, fixtures::API_TOKEN.into())]));
}
