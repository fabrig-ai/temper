use super::fixtures;
use crate::{ini, runner, setup::Origin};
const APP_INI_WITH_RUNNER: &[u8] = b"RUN_USER = ada\nWORK_PATH = /srv/forge-2/forgejo\n[server]\nPROTOCOL = http\nHTTP_ADDR = 127.0.0.1\nHTTP_PORT = 3000\nROOT_URL = http://127.0.0.1:3000/\nAPP_DATA_PATH = /srv/forge-2/forgejo/data\nDISABLE_SSH = true\nOFFLINE_MODE = true\n[database]\nDB_TYPE = sqlite3\nPATH = /srv/forge-2/forgejo/forgejo.db\n[repository]\nROOT = /srv/forge-2/forgejo/repositories\nDEFAULT_BRANCH = main\n[security]\nINSTALL_LOCK = true\nSECRET_KEY = key-secret\nINTERNAL_TOKEN = internal-secret\n[oauth2]\nJWT_SECRET = jwt-secret\n[service]\nDISABLE_REGISTRATION = true\n[mailer]\nENABLED = false\n[actions]\nENABLED = true\n[log]\nMODE = file\nLEVEL = Error\nROOT_PATH = /srv/forge-2/forgejo/log\n";
const APP_INI_WITH_A_PUBLIC_ADDRESS: &[u8] = b"RUN_USER = ada\nWORK_PATH = /srv/forge-2/forgejo\n[server]\nPROTOCOL = http\nHTTP_ADDR = 127.0.0.1\nHTTP_PORT = 3000\nDOMAIN = git.example.com\nROOT_URL = https://git.example.com/\nAPP_DATA_PATH = /srv/forge-2/forgejo/data\nDISABLE_SSH = true\nOFFLINE_MODE = true\n[database]\nDB_TYPE = sqlite3\nPATH = /srv/forge-2/forgejo/forgejo.db\n[repository]\nROOT = /srv/forge-2/forgejo/repositories\nDEFAULT_BRANCH = main\n[security]\nINSTALL_LOCK = true\nSECRET_KEY = key-secret\nINTERNAL_TOKEN = internal-secret\n[oauth2]\nJWT_SECRET = jwt-secret\n[service]\nDISABLE_REGISTRATION = true\n[mailer]\nENABLED = false\n[actions]\nENABLED = true\n[log]\nMODE = file\nLEVEL = Error\nROOT_PATH = /srv/forge-2/forgejo/log\n";
const RUNNER: &[u8] = b"runner:\n  capacity: 1\n  timeout: 3h\n  shutdown_timeout: 0s\n  fetch_interval: 1s\ncache:\n  enabled: false\nhost:\n  workdir_parent: \"/srv/forge-2/runner/jobs\"\nserver:\n  connections:\n    forgejo:\n      url: \"http://127.0.0.1:3000\"\n      uuid: \"runner-uuid\"\n      token_url: \"file:/srv/forge-2/runner/secret\"\n      labels: [\"temper-host:host\"]\n";
#[test]
fn app_ini_for_an_owned_forge_with_a_runner_matches_the_golden_text() {
    let text = ini::text(&fixtures::owned(), &fixtures::place(), &fixtures::secrets(), None, false);
    assert_eq!(text.expose(), APP_INI_WITH_RUNNER, "written by hand from protocol/setup.md, 6.2");
}
#[test]
fn app_ini_without_a_runner_turns_actions_off() {
    let mut owned = fixtures::owned();
    owned.runner = None;
    let text = ini::text(&owned, &fixtures::place(), &fixtures::secrets(), None, false);
    assert!(skein_lib::bytes::find(text.expose(), b"[actions]\nENABLED = false\n").is_some());
}
#[test]
fn app_ini_allows_the_webhooks_listener_when_given_one() {
    let text = ini::text(
        &fixtures::owned(),
        &fixtures::place(),
        &fixtures::secrets(),
        Some(core::net::Ipv4Addr::LOCALHOST.into()),
        false,
    );
    assert!(skein_lib::bytes::find(text.expose(), b"[webhook]\nALLOWED_HOST_LIST = 127.0.0.1\n").is_some());
}
#[test]
fn the_runner_configuration_matches_the_golden_text() {
    let owned = fixtures::owned();
    let text = runner::configuration(owned.runner.as_ref().unwrap(), &owned, &fixtures::place(), b"runner-uuid");
    assert_eq!(&*text, RUNNER);
}
#[test]
fn app_ini_with_a_public_address_matches_the_golden_text() {
    let mut owned = fixtures::owned();
    owned.public = Some(Origin { host: b"git.example.com".as_slice().into(), port: 443 });
    let text = ini::text(&owned, &fixtures::place(), &fixtures::secrets(), None, false);
    assert_eq!(text.expose(), APP_INI_WITH_A_PUBLIC_ADDRESS, "written by hand from protocol/setup.md, 6.2");
    let runner = runner::configuration(owned.runner.as_ref().unwrap(), &owned, &fixtures::place(), b"runner-uuid");
    assert_eq!(&*runner, RUNNER);
}
#[test]
fn app_ini_with_a_public_address_keeps_a_nondefault_port() {
    let mut owned = fixtures::owned();
    owned.public = Some(Origin { host: b"git.example.com".as_slice().into(), port: 8443 });
    let text = ini::text(&owned, &fixtures::place(), &fixtures::secrets(), None, false);
    assert!(
        skein_lib::bytes::find(text.expose(), b"DOMAIN = git.example.com\nROOT_URL = https://git.example.com:8443/\n")
            .is_some()
    );
}

#[test]
fn sign_in_enables_only_external_registration_with_the_member_login() {
    let text = ini::text(&fixtures::owned(), &fixtures::place(), &fixtures::secrets(), None, true);
    let registration = b"ALLOW_ONLY_EXTERNAL_REGISTRATION = true\n[oauth2_client]\nENABLE_AUTO_REGISTRATION = true\nUSERNAME = preferred_username\nACCOUNT_LINKING = login\n";
    let at = skein_lib::bytes::find(APP_INI_WITH_RUNNER, b"[mailer]").unwrap();
    let before = APP_INI_WITH_RUNNER.get(..at).unwrap();
    let after = APP_INI_WITH_RUNNER.get(at..).unwrap();
    let mut expected = skein_lib::Writer::new(APP_INI_WITH_RUNNER.len() + registration.len());
    expected.put(before).unwrap();
    expected.put(registration).unwrap();
    expected.put(after).unwrap();
    assert_eq!(text.expose(), expected.finish().as_ref());
}
