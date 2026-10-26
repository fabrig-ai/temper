use super::fixtures;
use crate::spec::{self, Command, Which};
use alloc::boxed::Box;
use skein_lib::bytes;

fn arguments(command: Command) -> Box<[Box<[u8]>]> {
    spec::command(command, &fixtures::owned(), &fixtures::place()).arguments
}
fn forgejo_suffix(command: Command, expected: &[&[u8]]) {
    let args = arguments(command);
    assert_eq!(args[0].as_ref(), b"--work-path");
    assert_eq!(args[1].as_ref(), b"/srv/forge-2/forgejo");
    assert_eq!(args[2].as_ref(), b"--config");
    assert_eq!(args[3].as_ref(), b"/srv/forge-2/forgejo/app.ini");
    assert_eq!(args.len(), expected.len().checked_add(4).unwrap());
    for (arg, expected) in args.iter().skip(4).zip(expected) {
        assert_eq!(arg.as_ref(), *expected);
    }
}
#[test]
fn migrate_has_its_confirmed_arguments() {
    forgejo_suffix(Command::Migrate, &[b"migrate"]);
}
#[test]
fn admin_creation_draws_a_password_and_clears_the_change_flag() {
    forgejo_suffix(
        Command::CreateAdmin,
        &[
            b"admin",
            b"user",
            b"create",
            b"--username",
            b"temper-admin",
            b"--email",
            b"temper-admin@example.invalid",
            b"--admin",
            b"--random-password",
            b"--must-change-password=false",
        ],
    );
}
#[test]
fn person_creation_draws_a_private_password_without_forcing_a_change() {
    forgejo_suffix(
        Command::CreatePerson,
        &[
            b"admin",
            b"user",
            b"create",
            b"--username",
            b"ada",
            b"--email",
            b"ada@example.invalid",
            b"--admin",
            b"--random-password",
            b"--must-change-password=false",
        ],
    );
}
#[test]
fn runner_registration_names_the_secret_file_and_labels() {
    forgejo_suffix(
        Command::Register,
        &[
            b"forgejo-cli",
            b"actions",
            b"register",
            b"--secret-file",
            b"/srv/forge-2/runner/secret",
            b"--name",
            b"temper-runner",
            b"--labels",
            b"temper-host:host",
        ],
    );
}
#[test]
fn web_has_its_confirmed_arguments() {
    forgejo_suffix(Command::Web, &[b"web"]);
}
#[test]
fn the_runner_daemon_names_its_connection_configuration() {
    assert_eq!(
        &*arguments(Command::Daemon),
        &[
            Box::from(b"--config".as_slice()),
            Box::from(b"/srv/forge-2/runner/config.yaml".as_slice()),
            Box::from(b"daemon".as_slice())
        ]
    );
}
#[test]
fn runner_registration_joins_all_labels_in_one_argument() {
    let mut owned = fixtures::owned();
    owned.runner.as_mut().unwrap().labels = Box::new([b"one:host".as_slice().into(), b"two:host".as_slice().into()]);
    let spec = spec::command(Command::Register, &owned, &fixtures::place());
    assert_eq!(spec.arguments.last().unwrap().as_ref(), b"one:host,two:host");
}
#[test]
fn no_argument_of_any_command_holds_a_secret() {
    let secrets = fixtures::secrets();
    for command in [
        Command::Dump,
        Command::Migrate,
        Command::CreateAdmin,
        Command::CreatePerson,
        Command::Register,
        Command::Web,
        Command::Daemon,
    ] {
        let spec = spec::command(command, &fixtures::owned(), &fixtures::place());
        for arg in &spec.arguments {
            for secret in [&secrets.secret_key, &secrets.internal_token, &secrets.jwt_secret] {
                assert_eq!(bytes::find(arg, secret.expose()), None, "{command:?}");
            }
        }
    }
}
#[test]
fn the_environment_is_cleared_to_home_path_and_isolated_git() {
    let owned = fixtures::owned();
    for command in [Command::Migrate, Command::CreateAdmin, Command::CreatePerson, Command::Register, Command::Web] {
        let spec = spec::command(command, &owned, &fixtures::place());
        let expected = [
            b"HOME=/srv/forge-2/home".as_slice(),
            b"PATH=/usr/bin:/bin",
            b"TZ=UTC",
            b"GIT_CONFIG_NOSYSTEM=1",
            b"GIT_CONFIG_GLOBAL=/srv/forge-2/home/gitconfig",
        ];
        assert_eq!(spec.environment.len(), expected.len());
        for (value, expected) in spec.environment.iter().zip(expected) {
            assert_eq!(value.as_ref(), expected);
        }
        assert_eq!(spec.program.as_ref(), b"/opt/forgejo/forgejo");
        assert_eq!(spec.directory.as_ref(), b"/srv/forge-2/forgejo");
        assert_eq!(spec.output, 65_536);
    }
}
#[test]
fn the_runner_uses_only_its_home_and_configured_path() {
    let spec = spec::command(Command::Daemon, &fixtures::owned(), &fixtures::place());
    assert_eq!(
        &*spec.environment,
        &[
            Box::from(b"HOME=/srv/forge-2/runner".as_slice()),
            Box::from(b"PATH=/usr/local/bin:/usr/bin:/bin".as_slice())
        ]
    );
    assert_eq!(spec.program.as_ref(), b"/opt/forgejo/forgejo-runner");
    assert_eq!(spec.directory.as_ref(), b"/srv/forge-2/forgejo");
    assert_eq!(spec.output, 65_536);
}
#[test]
fn the_admin_token_asks_for_four_scopes_under_a_suffixed_name() {
    let spec = spec::admin_token(&fixtures::owned(), &fixtures::place(), b"temper-admin-0123456789abcdef");
    let expected = [
        b"--work-path".as_slice(),
        b"/srv/forge-2/forgejo",
        b"--config",
        b"/srv/forge-2/forgejo/app.ini",
        b"admin",
        b"user",
        b"generate-access-token",
        b"--username",
        b"temper-admin",
        b"--token-name",
        b"temper-admin-0123456789abcdef",
        b"--raw",
        b"--scopes",
        b"write:admin,read:user,write:organization,write:repository",
    ];
    assert_eq!(spec.arguments.len(), expected.len());
    for (arg, expected) in spec.arguments.iter().zip(expected) {
        assert_eq!(arg.as_ref(), expected);
    }
}
#[test]
fn each_version_check_has_just_the_version_argument() {
    for (which, program) in
        [(Which::Forgejo, b"/opt/forgejo/forgejo".as_slice()), (Which::Runner, b"/opt/forgejo/forgejo-runner")]
    {
        let spec = spec::version(which, &fixtures::owned(), &fixtures::place());
        assert_eq!(spec.program.as_ref(), program);
        assert_eq!(&*spec.arguments, &[Box::from(b"--version".as_slice())]);
    }
}

#[test]
fn the_upgrade_names_the_partial_and_its_private_temp_directory() {
    forgejo_suffix(
        Command::Dump,
        &[b"dump", b"--file", b"/srv/forge-2/backups/dump.partial.zip", b"--tempdir", b"/srv/forge-2/backups/"],
    );
}

#[test]
fn the_persons_tokens_name_the_person_and_have_only_the_prescribed_scopes() {
    use crate::place::File;
    for (file, scopes) in [
        (File::PersonReadHeader, b"read:repository,read:issue,read:organization,read:user".as_slice()),
        (File::PersonWriteHeader, b"write:repository,write:issue,read:organization,read:user".as_slice()),
    ] {
        let spec = spec::person_token(&fixtures::owned(), &fixtures::place(), b"person-0123456789abcdef", file);
        let expected = [
            b"admin".as_slice(),
            b"user",
            b"generate-access-token",
            b"--username",
            b"ada",
            b"--token-name",
            b"person-0123456789abcdef",
            b"--raw",
            b"--scopes",
            scopes,
        ];
        assert_eq!(spec.arguments.len(), expected.len() + 4);
        for (argument, expected) in spec.arguments.iter().skip(4).zip(expected) {
            assert_eq!(argument.as_ref(), expected);
        }
    }
}
