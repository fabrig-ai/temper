//! The preparation's locked host; all decisions belong to its machine.
use crate::{command, files, process};
use skein_lib::Queue;
use std::io;
use temper_forge_setup::{
    place::{File, Place},
    prepare::{self, Event, Failure, Found, Prepare, Request},
};
/// Why a preparation did not finish.
#[derive(Debug)]
pub enum Failed {
    Held(File),
    Io(&'static str, io::ErrorKind),
    Command(command::Error),
    Refused(Failure),
}
fn locked(place: &Place, file: File) -> Result<process::Lock, Failed> {
    process::lock(crate::path(&place.path(file))).map_err(|why| match why {
        process::Refused::Held => Failed::Held(file),
        process::Refused::Io(why) => Failed::Io("lock", why),
    })
}
fn found(place: &Place) -> Result<Found, Failed> {
    let state = match skein_shell::read_private_file(
        crate::path(&place.path(File::State)),
        1 << 20,
        skein_shell::effective_user(),
    ) {
        Ok(bytes) => Some(bytes),
        Err(skein_shell::Unread::Absent) => None,
        Err(_) => return Err(Failed::Io("state", io::ErrorKind::PermissionDenied)),
    };
    Ok(Found {
        state,
        runner_secret: crate::path(&place.path(File::RunnerSecret)).exists(),
        admin_token: crate::path(&place.path(File::AdminToken)).exists(),
        person_headers: [
            crate::path(&place.path(File::PersonReadHeader)).exists(),
            crate::path(&place.path(File::PersonWriteHeader)).exists(),
        ],
        files: files::present(place).map_err(|why| Failed::Io("directory", why.kind()))?,
    })
}
/// Holds every acquired lock until the terminal answer.
pub fn run(limits: &skein_io::Limits, place: &Place, mut machine: Prepare) -> Result<(), Failed> {
    let mut locks = vec![locked(place, File::ForgejoLock)?];
    let mut event = Event::Found(found(place)?);
    let mut out = Queue::with_capacity(prepare::MAX_OUT);
    loop {
        prepare::step(&mut machine, event, &mut out);
        event = match out.pop().expect("one preparation request") {
            Request::Version { spec, .. } => {
                let ran = command::run_one(limits, &spec).map_err(Failed::Command)?;
                let line = ran.stdout.expose().split(|b| *b == b'\n').next().unwrap_or_default();
                Event::Printed(
                    String::from_utf8_lossy(line.strip_suffix(b"\r").unwrap_or(line)).into_owned().into_boxed_str(),
                )
            }
            Request::Run { spec, .. } | Request::Token { spec } | Request::PersonToken { spec, .. } => {
                Event::Ran(command::run_one(limits, &spec).map_err(Failed::Command)?)
            }
            Request::Lock(file) => {
                locks.push(locked(place, file)?);
                Event::Done
            }
            Request::Draw => {
                Event::Drew(skein_shell::crypto_seed().map_err(|_| Failed::Io("entropy", io::ErrorKind::Other))?)
            }
            Request::Clear => {
                files::clear(place).map_err(|why| Failed::Io("directory", why.kind()))?;
                Event::Done
            }
            Request::Layout => {
                files::layout(place).map_err(|why| Failed::Io("layout", why.kind()))?;
                Event::Done
            }
            Request::Write { file, contents } => {
                files::write_private(crate::path(&place.path(file)), contents.expose())
                    .map_err(|why| Failed::Io("file", why.kind()))?;
                Event::Done
            }
            Request::Exists { path } => {
                Event::Exists(files::exists(crate::path(&path)).map_err(|why| Failed::Io("backup", why.kind()))?)
            }
            Request::Rename { from, to } => {
                files::rename(crate::path(&from), crate::path(&to)).map_err(|why| Failed::Io("backup", why.kind()))?;
                Event::Done
            }
            Request::End(result) => return result.map_err(Failed::Refused),
        };
    }
}
