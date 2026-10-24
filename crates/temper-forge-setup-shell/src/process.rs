//! The one exception for the lock and exec (protocol/setup.md, 6.4).
use std::{
    fs, io,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
};
use temper_forge_setup::spec::Spec;
/// Held until dropped or handed across exec as standard input.
#[derive(Debug)]
pub struct Lock {
    file: fs::File,
}
/// A lock is held, or its file could not be opened.
#[derive(Debug)]
pub enum Refused {
    Held,
    Io(io::ErrorKind),
}
/// Takes an exclusive lock without waiting, making its file private.
pub fn lock(path: &Path) -> Result<Lock, Refused> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|why| Refused::Io(why.kind()))?;
    file.try_lock().map_err(|why| match why {
        fs::TryLockError::WouldBlock => Refused::Held,
        fs::TryLockError::Error(why) => Refused::Io(why.kind()),
    })?;
    Ok(Lock { file })
}
/// Builds the exec's isolated command; its lock is standard input.
#[must_use]
pub fn command(spec: &Spec, lock: Lock) -> Command {
    let mut command = Command::new(crate::path(&spec.program));
    command
        .args(spec.arguments.iter().map(|arg| crate::path(arg)))
        .env_clear()
        .current_dir(crate::path(&spec.directory))
        .stdin(Stdio::from(lock.file));
    for entry in &spec.environment {
        let split = entry.iter().position(|c| *c == b'=').expect("environment assignment");
        command.env(crate::path(&entry[..split]), crate::path(&entry[split + 1..]));
    }
    command
}
/// Replaces this process; only an unsuccessful exec returns.
/// An inherited listener at fd 3 remains open: `listener_at` leaves its flags
/// unchanged, the parent's dup2 cleared CLOEXEC, and `CommandExt::exec` does
/// not close descriptors without CLOEXEC (protocol/setup.md, 6.4).
#[must_use]
pub fn exec(spec: &Spec, lock: Lock) -> io::Error {
    command(spec, lock).exec()
}

/// Writes a sign-in key to a preopened private partial file; the caller publishes it by rename.
pub fn sign_in_key(file: fs::File) -> Result<(), String> {
    // The preopened stdout is 0600; umask also confines any file opens by openssl.
    Command::new("/bin/sh")
        .args(["-c", "umask 077; exec openssl \"$@\"", "openssl"])
        .args(temper_forge_setup::sign_in::KEY_ARGUMENTS.iter().map(|arg| std::ffi::OsStr::from_bytes(arg)))
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null())
        .status()
        .map_err(|why| format!("openssl could not start: {why}"))
        .and_then(
            |status| {
                if status.success() { Ok(()) } else { Err("openssl genpkey failed; openssl must be on PATH".into()) }
            },
        )
}
