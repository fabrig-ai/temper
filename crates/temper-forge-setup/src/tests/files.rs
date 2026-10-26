use super::fixtures;
use crate::{
    place::File,
    state::{self, Refused, State},
};
fn recorded() -> State {
    State {
        forgejo: b"forgejo-pin".as_slice().into(),
        runner: Some(b"runner-pin".as_slice().into()),
        prepared: true,
        secrets: fixtures::secrets(),
    }
}
#[test]
fn a_state_encodes_and_decodes_to_itself() {
    for runner in [None, Some(b"runner-pin".as_slice().into())] {
        let state = State { runner, ..recorded() };
        assert_eq!(state::decode(state::encode(&state).expose()), Ok(state));
    }
}
#[test]
fn a_state_of_another_version_or_with_an_unknown_member_is_refused() {
    let suffix = br#""forgejo":"pin","runner":null,"prepared":false,"secret_key":"key","internal_token":"token","jwt_secret":"jwt"}"#;
    for (prefix, expected) in [
        (br#"{"version":2,"#.as_slice(), Refused::Version(2)),
        (br#"{"version":1,"stray":true,"#, Refused::Member),
        (br#"{"version":1,"version":1,"#, Refused::Member),
    ] {
        let input = crate::check::joined(&[prefix, suffix]);
        assert_eq!(state::decode(&input), Err(expected));
    }
}
#[test]
fn a_state_debug_shows_no_secret() {
    let text = alloc::fmt::format(format_args!("{:?}", recorded()));
    for secret in [b"key-secret".as_slice(), b"internal-secret", b"jwt-secret"] {
        assert!(skein_lib::bytes::find(text.as_bytes(), secret).is_none());
    }
    assert!(text.contains("Secret { len: 10 }"));
}
#[test]
fn each_layout_file_is_below_the_forge_directory() {
    for (file, expected) in [
        (File::AppIni, b"/srv/forge-2/forgejo/app.ini".as_slice()),
        (File::DumpPartial, b"/srv/forge-2/backups/dump.partial.zip"),
        (File::State, b"/srv/forge-2/state"),
        (File::AdminToken, b"/srv/forge-2/admin_token"),
        (File::PersonPassword, b"/srv/forge-2/person_password"),
        (File::PersonReadHeader, b"/srv/forge-2/person_read.header"),
        (File::PersonWriteHeader, b"/srv/forge-2/person_write.header"),
        (File::ApiToken, b"/srv/forge-2/api_token"),
        (File::GitToken, b"/srv/forge-2/git_token"),
        (File::User, b"/srv/forge-2/user"),
        (File::WebhookSecret, b"/srv/forge-2/webhook_secret"),
        (File::RunnerSecret, b"/srv/forge-2/runner/secret"),
        (File::RunnerConfiguration, b"/srv/forge-2/runner/config.yaml"),
        (File::ForgejoLock, b"/srv/forge-2/forgejo.lock"),
        (File::RunnerLock, b"/srv/forge-2/runner.lock"),
    ] {
        assert_eq!(&*fixtures::place().path(file), expected);
    }
    assert_eq!(&*fixtures::place().work_path(), b"/srv/forge-2/forgejo");
}
