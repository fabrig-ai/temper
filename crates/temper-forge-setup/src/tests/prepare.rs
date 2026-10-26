use super::fixtures::{self, Drive};
use crate::{
    place::File,
    prepare::{Event, Exit, Failure, Found, Ran, Request},
    spec::{self, Command, Which},
    state,
};
use alloc::boxed::Box;
use skein_lib::Secret;

fn run(command: Command) -> Request {
    Request::Run { command, spec: spec::command(command, &fixtures::owned(), &fixtures::place()) }
}
fn token() -> Request {
    Request::Token { spec: spec::admin_token(&fixtures::owned(), &fixtures::place(), b"temper-admin-0303030303030303") }
}
fn drawing(runner: bool) -> Drive {
    let mut owned = fixtures::owned();
    if !runner {
        owned.runner = None;
    }
    let mut drive = Drive::new(owned);
    if runner {
        assert_eq!(drive.versions(fixtures::first(), true), Request::Lock(File::RunnerLock));
        assert_eq!(drive.send(Event::Done), Request::Draw);
    } else {
        assert_eq!(drive.versions(fixtures::first(), false), Request::Draw);
    }
    drive
}
fn migrate(runner: bool) -> Drive {
    let mut drive = drawing(runner);
    let Request::Write { file: File::State, .. } = drive.drawn(runner) else { panic!("initial record") };
    assert_eq!(drive.send(Event::Done), Request::Layout);
    let Request::Write { file: File::AppIni, .. } = drive.send(Event::Done) else { panic!("settings") };
    let request = drive.send(Event::Done);
    let Request::Run { command: Command::Migrate, .. } = request else { panic!("migrate first") };
    drive
}
fn person(runner: bool) -> Drive {
    let mut drive = migrate(runner);
    assert_eq!(drive.send(fixtures::ran(b"")), run(Command::CreateAdmin));
    assert_eq!(drive.send(fixtures::ran(b"discarded-admin-password\n")), token());
    assert_eq!(
        drive.send(fixtures::ran(b"admin-token\n")),
        Request::Write { file: File::AdminToken, contents: Secret::new(b"admin-token".as_slice().into()) }
    );
    assert_eq!(drive.send(Event::Done), run(Command::CreatePerson));
    drive
}
fn headers(drive: &mut Drive, files: &[File]) -> Request {
    let mut request = drive.send(Event::Done);
    for file in files {
        assert_eq!(request, Request::Draw);
        let (name, value) = if *file == File::PersonReadHeader {
            (b"person-read-0707070707070707".as_slice(), b"read-token".as_slice())
        } else {
            (b"person-write-0707070707070707".as_slice(), b"write-token".as_slice())
        };
        assert_eq!(
            drive.send(Event::Drew(fixtures::draw(7))),
            Request::PersonToken {
                file: *file,
                spec: spec::person_token(&fixtures::owned(), &fixtures::place(), name, *file),
            }
        );
        assert_eq!(
            drive.send(fixtures::ran(value)),
            Request::Write {
                file: *file,
                contents: Secret::new(crate::check::joined(&[b"Authorization: token ", value, b"\n"])),
            }
        );
        request = drive.send(Event::Done);
    }
    request
}
fn later(admin_token: bool) -> Drive {
    let mut drive = Drive::new(fixtures::owned());
    let found = Found { admin_token, ..fixtures::later() };
    assert_eq!(drive.versions(found, true), Request::Layout);
    let Request::Write { file: File::AppIni, contents } = drive.send(Event::Done) else { panic!("settings") };
    assert!(skein_lib::bytes::find(contents.expose(), b"key-secret").is_some());
    drive
}
#[test]
fn another_forgejo_binary_is_refused_naming_its_path_and_line() {
    let mut drive = Drive::new(fixtures::owned());
    let Request::Version { which: Which::Forgejo, spec } = drive.send(Event::Found(fixtures::first())) else {
        panic!("Forgejo checked first")
    };
    assert_eq!(&*spec.program, b"/opt/forgejo/forgejo");
    let line: Box<str> = "another Forgejo".into();
    assert_eq!(
        drive.send(Event::Printed(line.clone())),
        Request::End(Err(Failure::Binary {
            which: Which::Forgejo,
            program: b"/opt/forgejo/forgejo".as_slice().into(),
            printed: line
        }))
    );
}
#[test]
fn another_runner_binary_is_refused_naming_its_path_and_line() {
    let mut drive = Drive::new(fixtures::owned());
    drive.send(Event::Found(fixtures::first()));
    let Request::Version { which: Which::Runner, spec } = drive.send(Event::Printed(fixtures::line(Which::Forgejo)))
    else {
        panic!("runner checked second")
    };
    assert_eq!(&*spec.program, b"/opt/forgejo/forgejo-runner");
    let line: Box<str> = "another runner".into();
    assert_eq!(
        drive.send(Event::Printed(line.clone())),
        Request::End(Err(Failure::Binary {
            which: Which::Runner,
            program: b"/opt/forgejo/forgejo-runner".as_slice().into(),
            printed: line
        }))
    );
}
#[test]
fn a_first_preparation_runs_its_commands_in_order() {
    let mut drive = person(true);
    assert_eq!(
        drive.send(fixtures::person_created()),
        Request::Write { file: File::PersonPassword, contents: Secret::new(b"person-password".as_slice().into()) }
    );
    assert_eq!(
        headers(&mut drive, &[File::PersonReadHeader, File::PersonWriteHeader]),
        Request::Write {
            file: File::RunnerSecret,
            contents: Secret::new(b"0404040404040404040404040404040404040404".as_slice().into())
        }
    );
    assert_eq!(drive.send(Event::Done), run(Command::Register));
    let Request::Write { file: File::RunnerConfiguration, .. } = drive.send(fixtures::ran(b"runner-uuid\n")) else {
        panic!("connection")
    };
    let Request::Write { file: File::State, .. } = drive.send(Event::Done) else { panic!("complete record") };
    assert_eq!(drive.send(Event::Done), Request::End(Ok(())));
}
#[test]
fn a_preparation_records_its_start_and_end() {
    let mut drive = drawing(false);
    let Request::Write { file: File::State, contents } = drive.drawn(false) else { panic!("record before layout") };
    let state = state::decode(contents.expose()).unwrap();
    assert!(!state.prepared);
    assert_eq!(state.runner, None);
    assert_eq!(state.secrets.secret_key.expose(), b"0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(
        state.secrets.internal_token.expose(),
        b"0101010101010101010101010101010101010101010101010101010101010101"
    );
    assert_eq!(state.secrets.jwt_secret.expose(), b"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI");
    let mut drive = person(false);
    let Request::Write { file: File::PersonPassword, .. } = drive.send(fixtures::person_created()) else {
        panic!("password")
    };
    let Request::Write { file: File::State, contents } =
        headers(&mut drive, &[File::PersonReadHeader, File::PersonWriteHeader])
    else {
        panic!("record last")
    };
    assert!(state::decode(contents.expose()).unwrap().prepared);
    assert_eq!(drive.send(Event::Done), Request::End(Ok(())));
}
#[test]
fn a_person_password_without_quotes_ends_preparation() {
    let mut drive = person(false);
    assert_eq!(
        drive.send(fixtures::ran(b"person-password\n")),
        Request::End(Err(Failure::Unread { command: Command::CreatePerson }))
    );
}
#[test]
fn a_first_preparation_with_a_runner_asks_for_the_runners_lock() {
    let mut drive = Drive::new(fixtures::owned());
    assert_eq!(drive.versions(fixtures::first(), true), Request::Lock(File::RunnerLock));
    assert_eq!(drive.send(Event::Done), Request::Draw);
}
#[test]
fn a_later_preparation_at_the_same_versions_writes_app_ini_and_nothing_else() {
    let mut drive = later(true);
    assert_eq!(drive.send(Event::Done), Request::End(Ok(())));
}
#[test]
fn a_missing_admin_token_is_made_again_under_a_fresh_name() {
    let mut drive = later(false);
    assert_eq!(drive.send(Event::Done), Request::Draw);
    assert_eq!(drive.send(Event::Drew(fixtures::draw(3))), token());
    assert_eq!(
        drive.send(fixtures::ran(b"replacement-token\n")),
        Request::Write { file: File::AdminToken, contents: Secret::new(b"replacement-token".as_slice().into()) }
    );
    assert_eq!(drive.send(Event::Done), Request::End(Ok(())));
}
#[test]
fn a_deleted_admin_ends_a_later_preparation_in_a_token_failure() {
    let mut drive = later(false);
    assert_eq!(drive.send(Event::Done), Request::Draw);
    assert_eq!(drive.send(Event::Drew(fixtures::draw(3))), token());
    assert_eq!(
        drive.send(Event::Ran(Ran {
            exit: Exit::Code(1),
            stdout: Secret::new(Box::new([])),
            stderr: Secret::new(b"temper-admin was deleted".as_slice().into())
        })),
        Request::End(Err(Failure::Token { exit: Exit::Code(1), tail: b"temper-admin was deleted".as_slice().into() }))
    );
}
#[test]
fn a_preparation_cut_short_is_removed_and_begun_again_from_an_empty_layout() {
    let mut found = fixtures::later();
    let mut state = state::decode(found.state.as_ref().unwrap().expose()).unwrap();
    state.prepared = false;
    found.state = Some(state::encode(&state));
    let mut drive = Drive::new(fixtures::owned());
    assert_eq!(drive.versions(found, true), Request::Lock(File::RunnerLock));
    assert_eq!(drive.send(Event::Done), Request::Clear);
    assert_eq!(drive.send(Event::Done), Request::Draw);
    let Request::Write { file: File::State, contents } = drive.drawn(true) else { panic!("fresh state") };
    assert!(!state::decode(contents.expose()).unwrap().prepared);
    assert_eq!(drive.send(Event::Done), Request::Layout);
}
#[test]
fn forgejos_files_with_no_state_are_refused() {
    let mut drive = Drive::new(fixtures::owned());
    assert_eq!(
        drive.versions(Found { files: true, ..fixtures::first() }, true),
        Request::End(Err(Failure::Unrecorded))
    );
}
#[test]
fn a_failed_command_names_its_status_and_none_of_the_secrets() {
    let mut drive = person(true);
    drive.send(fixtures::person_created());
    headers(&mut drive, &[File::PersonReadHeader, File::PersonWriteHeader]);
    drive.send(Event::Done);
    let output = b"0000000000000000000000000000000000000000000000000000000000000000 0101010101010101010101010101010101010101010101010101010101010101 AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI 0404040404040404040404040404040404040404 admin-token person-password read-token write-token";
    assert_eq!(
        drive.send(Event::Ran(Ran {
            exit: Exit::Signal(9),
            stdout: Secret::new(output.as_slice().into()),
            stderr: Secret::new(b" failed".as_slice().into())
        })),
        Request::End(Err(Failure::Command {
            command: Command::Register,
            exit: Exit::Signal(9),
            tail: b"[redacted] [redacted] [redacted] [redacted] [redacted] [redacted] [redacted] [redacted] failed"
                .as_slice()
                .into()
        }))
    );
}

#[test]
fn secrets_across_the_two_streams_are_masked_before_the_tail_is_cut() {
    let mut drive = person(false);
    assert_eq!(
        drive.send(Event::Ran(Ran {
            exit: Exit::Code(1),
            stdout: Secret::new(b"admin-".as_slice().into()),
            stderr: Secret::new(b"token".as_slice().into()),
        })),
        Request::End(Err(Failure::Command {
            command: Command::CreatePerson,
            exit: Exit::Code(1),
            tail: b"[redacted]".as_slice().into()
        }))
    );
}

#[test]
fn each_missing_person_header_is_remade_without_touching_the_other() {
    for (present, file) in [([false, true], File::PersonReadHeader), ([true, false], File::PersonWriteHeader)] {
        let mut drive = Drive::new(fixtures::owned());
        assert_eq!(drive.versions(Found { person_headers: present, ..fixtures::later() }, true), Request::Layout);
        let Request::Write { file: File::AppIni, .. } = drive.send(Event::Done) else { panic!("settings") };
        assert_eq!(headers(&mut drive, &[file]), Request::End(Ok(())));
    }
}
#[test]
fn a_preparation_from_before_person_headers_makes_both() {
    let mut drive = Drive::new(fixtures::owned());
    assert_eq!(drive.versions(Found { person_headers: [false; 2], ..fixtures::later() }, true), Request::Layout);
    drive.send(Event::Done);
    assert_eq!(headers(&mut drive, &[File::PersonReadHeader, File::PersonWriteHeader]), Request::End(Ok(())));
}

#[test]
fn a_person_creation_failure_masks_the_password_it_just_printed() {
    let mut drive = person(false);
    let Request::End(Err(Failure::Command { tail, .. })) = drive.send(Event::Ran(Ran {
        exit: Exit::Code(1),
        stdout: Secret::new(b"generated random password is 'new-private-password'\n".as_slice().into()),
        stderr: Secret::new(b"failed with new-private-password".as_slice().into()),
    })) else {
        panic!("creation failure")
    };
    assert_eq!(tail.as_ref(), b"generated random password is '[redacted]'\nfailed with [redacted]");
}

#[test]
fn a_person_token_failure_names_the_header_and_masks_the_token() {
    let mut drive = Drive::new(fixtures::owned());
    drive.versions(Found { person_headers: [false, true], ..fixtures::later() }, true);
    drive.send(Event::Done);
    assert_eq!(drive.send(Event::Done), Request::Draw);
    let Request::PersonToken { file: File::PersonReadHeader, .. } = drive.send(Event::Drew(fixtures::draw(7))) else {
        panic!("read token")
    };
    assert_eq!(
        drive.send(Event::Ran(Ran {
            exit: Exit::Code(1),
            stdout: Secret::new(b"new-token\n".as_slice().into()),
            stderr: Secret::new(b"failed with new-token".as_slice().into()),
        })),
        Request::End(Err(Failure::PersonToken {
            file: File::PersonReadHeader,
            exit: Exit::Code(1),
            tail: b"[redacted]\nfailed with [redacted]".as_slice().into()
        }))
    );
}
