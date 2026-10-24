//! Isolated process specifications for owned setup (protocol/setup.md, 6.3–6.4).
use crate::{
    check::joined,
    ini::flatten,
    place::{File, Place},
    setup::Owned,
};
use alloc::boxed::Box;
use skein_lib::List;
/// Which pinned binary is checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    /// Forgejo, including its offline administration commands.
    Forgejo,
    /// The runner daemon.
    Runner,
}
/// One preparation command or supervised process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    /// Dump the prepared Forgejo before migrating it.
    Dump,
    /// Prepare the database schema.
    Migrate,
    /// Make temper-admin with a discarded random password.
    CreateAdmin,
    /// Make the person with a random password to change.
    CreatePerson,
    /// Register the global runner through its secret file.
    Register,
    /// Run Forgejo until stopped.
    Web,
    /// Run the runner until stopped.
    Daemon,
}
/// A bounded process, with the entire environment and no shell or path search.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Spec {
    pub program: Box<[u8]>,            // Absolute binary.
    pub arguments: Box<[Box<[u8]>]>,   // Arguments after argv[0].
    pub environment: Box<[Box<[u8]>]>, // Whole environment, NAME=value.
    pub directory: Box<[u8]>,          // Working directory.
    pub output: u32,                   // Bytes retained from each stream.
}
/// Each command stream's byte ceiling.
pub const OUTPUT: u32 = 65_536;
/// Temper's site administrator, also named by a token failure.
pub const ADMIN: &[u8] = b"temper-admin";
fn environment(which: Which, owned: &Owned, place: &Place) -> Box<[Box<[u8]>]> {
    match which {
        Which::Forgejo => Box::new([
            joined(&[b"HOME=", &place.directory, b"/home"]),
            b"PATH=/usr/bin:/bin".as_slice().into(),
            b"TZ=UTC".as_slice().into(),
            b"GIT_CONFIG_NOSYSTEM=1".as_slice().into(),
            joined(&[b"GIT_CONFIG_GLOBAL=", &place.directory, b"/home/gitconfig"]),
        ]),
        Which::Runner => Box::new([
            joined(&[b"HOME=", &place.directory, b"/runner"]),
            joined(&[b"PATH=", &owned.runner.as_ref().expect("runner configured").path]),
        ]),
    }
}
fn specification(which: Which, owned: &Owned, place: &Place, arguments: Box<[Box<[u8]>]>) -> Spec {
    let program = match which {
        Which::Forgejo => owned.forgejo.clone(),
        Which::Runner => owned.runner.as_ref().expect("runner configured").binary.clone(),
    };
    Spec {
        program,
        arguments,
        environment: environment(which, owned, place),
        directory: place.work_path(),
        output: OUTPUT,
    }
}
fn forgejo(owned: &Owned, place: &Place, suffix: &[Box<[u8]>]) -> Spec {
    let mut args = List::with_capacity(
        u32::try_from(suffix.len()).expect("bounded arguments").checked_add(4).expect("bounded arguments"),
    );
    for arg in
        [b"--work-path".as_slice().into(), place.work_path(), b"--config".as_slice().into(), place.path(File::AppIni)]
    {
        args.push(arg).expect("measured arguments");
    }
    for arg in suffix {
        args.push(arg.clone()).expect("measured arguments");
    }
    specification(Which::Forgejo, owned, place, args.into_boxed())
}
fn words(input: &[&[u8]]) -> Box<[Box<[u8]>]> {
    let mut args = List::with_capacity(u32::try_from(input.len()).expect("bounded arguments"));
    for arg in input {
        args.push(Box::from(*arg)).expect("measured arguments");
    }
    args.into_boxed()
}
/// Builds only the commands confirmed by the CLI and runner facts.
#[must_use]
pub fn command(command: Command, owned: &Owned, place: &Place) -> Spec {
    let args = match command {
        Command::Dump => words(&[b"dump", b"--file", &place.path(File::DumpPartial), b"--tempdir", &place.backup(b"")]),
        Command::Migrate => words(&[b"migrate"]),
        Command::CreateAdmin => words(&[
            b"admin",
            b"user",
            b"create",
            b"--username",
            ADMIN,
            b"--email",
            b"temper-admin@example.invalid",
            b"--admin",
            b"--random-password",
            b"--must-change-password=false",
        ]),
        Command::CreatePerson => words(&[
            b"admin",
            b"user",
            b"create",
            b"--username",
            &owned.person.login,
            b"--email",
            &owned.person.email,
            b"--admin",
            b"--random-password",
        ]),
        Command::Register => {
            let runner = owned.runner.as_ref().expect("runner configured");
            let mut labels = List::with_capacity(16);
            for (i, label) in runner.labels.iter().enumerate() {
                if i > 0 {
                    labels.push(b",".as_slice().into()).expect("eight labels");
                }
                labels.push(label.clone()).expect("eight labels");
            }
            words(&[
                b"forgejo-cli",
                b"actions",
                b"register",
                b"--secret-file",
                &place.path(File::RunnerSecret),
                b"--name",
                b"temper-runner",
                b"--labels",
                &flatten(labels.as_slice()),
            ])
        }
        Command::Web => words(&[b"web"]),
        Command::Daemon => {
            return specification(
                Which::Runner,
                owned,
                place,
                words(&[b"--config", &place.path(File::RunnerConfiguration), b"daemon"]),
            );
        }
    };
    forgejo(owned, place, &args)
}
/// Makes the admin's three-scoped token under a fresh suffixed name.
#[must_use]
pub fn admin_token(owned: &Owned, place: &Place, name: &[u8]) -> Spec {
    forgejo(
        owned,
        place,
        &words(&[
            b"admin",
            b"user",
            b"generate-access-token",
            b"--username",
            ADMIN,
            b"--token-name",
            name,
            b"--raw",
            b"--scopes",
            b"write:admin,read:user,write:organization,write:repository",
        ]),
    )
}
/// Checks a binary in the same isolated environment as its commands.
#[must_use]
pub fn version(which: Which, owned: &Owned, place: &Place) -> Spec {
    let mut spec = specification(which, owned, place, words(&[b"--version"]));
    // The work path does not exist until the first preparation creates its layout.
    spec.directory.clone_from(&place.directory);
    spec
}

/// Marks the exec alone as receiving fd 3; version and preparation specs stay isolated.
#[must_use]
pub fn listening(mut spec: Spec) -> Spec {
    let count = u32::try_from(spec.environment.len()).expect("bounded environment");
    let mut environment = List::with_capacity(count.checked_add(1).expect("bounded environment"));
    for entry in spec.environment {
        environment.push(entry).expect("measured environment");
    }
    environment.push(b"LISTEN_FDS=1".as_slice().into()).expect("listener entry");
    spec.environment = environment.into_boxed();
    spec
}
