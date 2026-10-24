//! Client-managed configuration has no file-backed credentials.
use super::fixtures;
use crate::{
    check::{self, Checked, Members, User},
    problem,
    setup::Mode,
};
use alloc::boxed::Box;

const SETUP: &[u8] = br#"[{"forge":1,"bot":{"login":"temper","email":"temper@example.invalid"},"mode":"managed"}]"#;
fn members() -> Members {
    let legacy = fixtures::members();
    let mut forge = legacy.forges[0].clone();
    forge.api_token = Box::new([]);
    forge.user = User::Id(0);
    forge.identity_token = Box::new([]);
    Members { forges: Box::new([forge]), listeners: Box::new([]) }
}
fn checked(input: &[u8], members: &Members) -> Result<Checked, problem::Problems> {
    check::checked(&fixtures::document(input), 0, &jig_protocol_config::Directories::working(), members, None, &[])
}
fn refused(input: &[u8], members: &Members, expected: &[u8]) {
    assert_eq!(problem::lines(&checked(input, members).unwrap_err()).as_ref(), expected);
}
#[test]
fn managed_string_parses_without_an_admin_token_file() {
    let checked = checked(SETUP, &members()).unwrap();
    let entry = checked.setup.entry(1).unwrap();
    assert_eq!(entry.mode, Mode::Managed { admin_token: None });
    assert!(entry.directory.is_empty());
    assert_eq!(checked.records_only.as_ref(), [1]);
}
#[test]
fn managed_string_names_its_admin_without_temper_prefix() {
    let checked = checked(SETUP, &members()).unwrap();
    assert_eq!(
        crate::given::values(checked.setup.entry(1).unwrap(), None, &checked.limits).admin_name.as_ref(),
        b"admin"
    );
}
#[test]
fn managed_string_refuses_a_directory() {
    let input = fixtures::changed_in(SETUP, br#""forge":1"#, br#""forge":1,"directory":"forge-1""#);
    refused(&input, &members(), b"application.setup[0].directory: invalid form\n");
}
#[test]
fn managed_string_refuses_start() {
    let input = fixtures::changed_in(SETUP, br#""forge":1"#, br#""forge":1,"start":"asked""#);
    refused(&input, &members(), b"application.setup[0].start: invalid form\n");
}
#[test]
fn managed_string_refuses_orgs() {
    let input = fixtures::changed_in(SETUP, br#""forge":1"#, br#""forge":1,"orgs":[]"#);
    refused(&input, &members(), b"application.setup[0].orgs: invalid form\n");
}
#[test]
fn managed_string_refuses_api_token_files() {
    let mut members = members();
    members.forges[0].api_token = b"token".as_slice().into();
    refused(SETUP, &members, b"application.forges[0].api_token: invalid form\n");
}
#[test]
fn managed_string_refuses_user_files() {
    let mut members = members();
    members.forges[0].user = User::File(b"user".as_slice().into());
    refused(SETUP, &members, b"application.forges[0].user: invalid form\n");
}
#[test]
fn managed_string_refuses_webhook_secret_files() {
    let mut members = members();
    members.forges[0].webhook_secret = Some(b"hook".as_slice().into());
    refused(SETUP, &members, b"application.forges[0].webhook_secret: invalid form\n");
}
#[test]
fn managed_string_refuses_identity_token_files() {
    let mut members = members();
    members.forges[0].identity_token = b"git-token".as_slice().into();
    refused(SETUP, &members, b"application.setup[0].forge: invalid form\n");
}
#[test]
fn managed_string_refuses_plaintext_outside_loopback() {
    let mut members = members();
    members.forges[0].plaintext = true;
    members.forges[0].api = Some(([192, 0, 2, 1], 3000).into());
    refused(SETUP, &members, b"application.forges[0].transport: invalid form\n");
}
#[test]
fn managed_string_accepts_plaintext_on_loopback() {
    let mut members = members();
    members.forges[0].plaintext = true;
    members.forges[0].api = Some(([127, 0, 0, 1], 3000).into());
    checked(SETUP, &members).expect("loopback managed Forgejo");
}
#[test]
fn managed_string_accepts_tls() {
    checked(SETUP, &members()).expect("TLS managed Forgejo");
}
#[test]
fn managed_string_starts_serve_without_legacy_reconcile() {
    let checked = checked(SETUP, &members()).unwrap();
    let steps = crate::step::steps(&checked.setup, &[], Box::new([]));
    assert_eq!(steps.always.len(), 1);
    assert_eq!(steps.always[0].arguments()[0].as_ref(), b"serve");
}
#[test]
fn legacy_managed_still_requires_orgs() {
    let input = fixtures::changed(br#""orgs":["acme"],"#, b"");
    refused(&input, &fixtures::members(), b"application.setup[0].orgs: missing\n");
}
#[test]
fn legacy_managed_still_requires_file_credentials() {
    let mut members = fixtures::members();
    members.forges[0] = self::members().forges[0].clone();
    refused(fixtures::TWO, &members, b"application.setup[0].mode: invalid form\n");
}
