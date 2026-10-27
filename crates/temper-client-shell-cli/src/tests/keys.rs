use super::fixtures;
use skein_io::kernel::{Error, Exit, Kind, Stat};
const ADD: &[&[u8]] = &[b"account", b"add-key", b"1", b"/linked-parent/link-file", b"--deployment", b"--key", b"key"];
const AT_BOUND: &[u8] = &[b'k'; 4096];
const OVER_BOUND: &[u8] = &[b'k'; 4097];
const BOUND_NEWLINE: &[u8] = &{
    let mut bytes = [b'k'; 4097];
    bytes[4096] = b'\n';
    bytes
};

#[test]
fn a_key_file_strips_only_one_final_newline() {
    for (file, expected) in [
        (b"private-value\n".as_slice(), b"private-value".as_slice()),
        (b"private-value\n\n", b"private-value\n"),
        (AT_BOUND, AT_BOUND),
        (BOUND_NEWLINE, AT_BOUND),
    ] {
        let mut f = fixtures::Frame::act(ADD);
        f.link = file;
        f.sign_in_then_deliver_me();
        assert_eq!(f.carried_keys.len(), 1);
        assert_eq!(f.carried_keys[0].expose(), expected);
        let store = f.stored(b"store").expect("kept act");
        assert!(!store.windows(expected.len()).any(|w| w == expected));
        assert!(f.stdout().is_empty());
        assert!(f.stderr().is_empty());
    }
}
#[test]
fn an_invalid_key_size_opens_no_directory() {
    for file in [b"".as_slice(), b"\n", OVER_BOUND] {
        let mut f = fixtures::Frame::act(ADD);
        f.link = file;
        f.drain();
        assert_eq!(f.exit(), Some(Exit::Code(1)));
        assert!(!f.opened(b"directory"));
        assert_eq!(f.requests(), 0);
    }
}
#[test]
fn a_key_file_requires_private_regular_owned_metadata() {
    for (kind, mode, owner) in [
        (Kind::Directory, 0o600, 7),
        (Kind::Other, 0o600, 7),
        (Kind::File, 0o640, 7),
        (Kind::File, 0o606, 7),
        (Kind::File, 0o600, 8),
    ] {
        let mut f = fixtures::Frame::act(ADD);
        f.link = b"secret-value";
        f.input_stat = Some(Stat { kind, mode, owner, size: f.link.len() as u64, links: 1 });
        f.drain();
        assert_eq!(f.exit(), Some(Exit::Code(1)));
        assert!(!f.opened(b"directory"));
        assert_eq!(f.requests(), 0);
        assert!(!f.stderr().windows(f.link.len()).any(|w| w == f.link));
    }
}
#[test]
fn a_key_file_refuses_a_link_at_its_name() {
    let mut f = fixtures::Frame::act(ADD);
    f.linked_leaf = Some(Error::TooManyLinks);
    f.drain();
    assert_eq!(f.exit(), Some(Exit::Code(1)));
    assert!(!f.opened(b"directory"));
    assert_eq!(f.stderr(), b"temper ctl: key file must not be a link\n");
}
#[test]
fn a_missing_key_file_names_the_file() {
    let mut f = fixtures::Frame::act(ADD);
    f.linked_leaf = Some(Error::NotFound);
    f.drain();
    assert_eq!(f.exit(), Some(Exit::Code(1)));
    assert!(!f.opened(b"directory"));
    assert_eq!(f.stderr(), b"temper ctl: key file not found\n");
}
#[test]
fn a_key_file_accepts_its_opened_parent() {
    let mut f = fixtures::Frame::act(ADD);
    f.link = b"private-key";
    f.sign_in_then_deliver_me();
    assert_eq!(f.link_root, Some(fixtures::FILE_PARENT));
    assert_eq!(f.link_leaf.as_deref(), Some(b"link-file".as_slice()));
    assert_eq!(f.carried_keys[0].expose(), f.link);
}
#[test]
fn json_cannot_name_a_key_file() {
    let mut f = fixtures::Frame::read(&["--call", "-"]);
    f.input = br#"{"tool":"account add-key","arguments":{"provider":1,"file":"link-file","key":"add"}}"#;
    f.drain();
    assert_eq!(f.exit(), Some(Exit::Code(1)));
    assert!(!f.opened(b"directory"));
}

const APP: &[&[u8]] =
    &[b"forge", b"connect", b"3", b"github-app", b"4242", b"/linked-parent/link-file", b"--key", b"app"];
const APP_BOUND: &[u8] = &[b'p'; 4089];
const APP_OVER: &[u8] = &[b'p'; 4090];
const APP_NEWLINE: &[u8] = &{
    let mut bytes = [b'p'; 4090];
    bytes[4089] = b'\n';
    bytes
};

fn app_file_refused(f: &fixtures::Frame) {
    assert_eq!(f.exit(), Some(Exit::Code(1)));
    assert!(!f.opened(b"directory"));
    assert_eq!(f.requests(), 0);
    assert!(skein_lib::bytes::find(f.stderr(), b"/linked-parent/link-file").is_some());
    if !f.link.is_empty() && f.link != b"\n" {
        assert!(skein_lib::bytes::find(f.stderr(), f.link).is_none());
    }
}
#[test]
fn an_app_key_requires_a_private_owned_regular_file_and_refuses_symlinks() {
    for (kind, mode, owner) in [
        (Kind::Directory, 0o600, 7),
        (Kind::Other, 0o600, 7),
        (Kind::File, 0o640, 7),
        (Kind::File, 0o606, 7),
        (Kind::File, 0o600, 8),
    ] {
        let mut f = fixtures::Frame::act(APP);
        f.link = b"fake-app-key";
        f.input_stat = Some(Stat { kind, mode, owner, size: f.link.len() as u64, links: 1 });
        f.drain();
        app_file_refused(&f);
    }
    for error in [Error::TooManyLinks, Error::NotFound] {
        let mut f = fixtures::Frame::act(APP);
        f.link = b"fake-app-key";
        f.linked_leaf = Some(error);
        f.drain();
        app_file_refused(&f);
    }
}
#[test]
fn an_app_key_refuses_empty_and_oversized_files_before_opening_directory() {
    for file in [b"".as_slice(), b"\n", APP_OVER, OVER_BOUND] {
        let mut f = fixtures::Frame::act(APP);
        f.link = file;
        f.drain();
        app_file_refused(&f);
    }
}
#[test]
fn an_app_key_strips_one_final_lf_and_reserves_seven_bytes_for_the_bundle() {
    for (file, pem) in [
        (b"fake-pem\n".as_slice(), b"fake-pem".as_slice()),
        (b"fake-pem\n\n", b"fake-pem\n"),
        (APP_BOUND, APP_BOUND),
        (APP_NEWLINE, APP_BOUND),
    ] {
        let mut f = fixtures::Frame::act(APP);
        f.link = file;
        f.sign_in_then_deliver_me();
        assert_eq!(f.carried_keys.len(), 1);
        let mut expected = vec![0, 1, 10];
        expected.extend_from_slice(&u32::try_from(pem.len()).expect("bounded PEM").to_be_bytes());
        expected.extend_from_slice(pem);
        assert_eq!(f.carried_keys[0].expose(), expected);
        assert!(f.stdout().is_empty());
        assert!(f.stderr().is_empty());
        if let Some(store) = f.stored(b"store") {
            assert!(skein_lib::bytes::find(store, pem).is_none());
        }
    }
}

const ADMIN: &[&[u8]] =
    &[b"forge", b"connect", b"3", b"forgejo-admin", b"/linked-parent/link-file", b"--key", b"admin"];
const ADMIN_TOKEN: &[u8] = b"admin-token-placeholder-0123";

fn no_admin_token(f: &fixtures::Frame) {
    for bytes in [f.stdout(), f.stderr()] {
        assert!(skein_lib::bytes::find(bytes, ADMIN_TOKEN).is_none());
    }
}
#[test]
fn an_admin_token_refuses_every_group_or_other_permission_before_reading() {
    for permission in [0o040, 0o020, 0o010, 0o004, 0o002, 0o001] {
        let mut f = fixtures::Frame::act(ADMIN);
        let mode = 0o600 | permission;
        f.link = ADMIN_TOKEN;
        f.input_stat = Some(Stat { kind: Kind::File, mode, owner: 7, size: f.link.len() as u64, links: 1 });
        f.drain();
        app_file_refused(&f);
        assert_eq!(f.input_reads, 0);
        assert!(skein_lib::bytes::find(f.stderr(), format!("{mode:#o}").as_bytes()).is_some());
        no_admin_token(&f);
    }
}
#[test]
fn an_admin_token_strips_one_final_lf_and_carries_item_one_without_printing_it() {
    for file in
        [ADMIN_TOKEN, b"admin-token-placeholder-0123\n", b"admin-token-placeholder-0123\n\n", APP_BOUND, APP_NEWLINE]
    {
        let mut f = fixtures::Frame::act(ADMIN);
        f.link = file;
        f.sign_in_then_deliver_me();
        let token = file.strip_suffix(b"\n").unwrap_or(file);
        let mut expected = vec![0, 1, 0];
        expected.extend_from_slice(&u32::try_from(token.len()).expect("bounded token").to_be_bytes());
        expected.extend_from_slice(token);
        assert_eq!(f.carried_keys.len(), 1);
        assert_eq!(f.carried_keys[0].expose(), expected);
        assert!(expected.len() <= 4096);
        no_admin_token(&f);
        if let Some(store) = f.stored(b"store") {
            assert!(skein_lib::bytes::find(store, token).is_none());
        }
    }
}
#[test]
fn an_admin_token_refuses_empty_and_oversized_files_without_printing_them() {
    for file in [b"".as_slice(), b"\n", APP_OVER] {
        let mut f = fixtures::Frame::act(ADMIN);
        f.link = file;
        f.drain();
        app_file_refused(&f);
        no_admin_token(&f);
    }
}
