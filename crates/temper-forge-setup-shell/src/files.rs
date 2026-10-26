//! Whole private files and the preparation's exact removable layout.
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
use temper_forge_setup::place::Place;
/// Writes beside the destination, syncs, renames and syncs its directory.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("partial");
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).or_else(|why| {
        if why.kind() != io::ErrorKind::AlreadyExists {
            return Err(why);
        }
        fs::remove_file(&temporary)?;
        fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)
    })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    fs::File::open(path.parent().expect("a forge file has a parent"))?.sync_all()
}
const CLEARED: [&str; 6] =
    ["forgejo", "runner", "admin_token", "person_password", "person_read.header", "person_write.header"];
/// Whether a first preparation's files exist, including a dangling link.
pub fn present(place: &Place) -> io::Result<bool> {
    let root = crate::path(&place.directory);
    for name in CLEARED {
        match fs::symlink_metadata(root.join(name)) {
            Ok(_) => return Ok(true),
            Err(why) if why.kind() == io::ErrorKind::NotFound => {}
            Err(why) => return Err(why),
        }
    }
    Ok(false)
}
/// Clears only a recorded unfinished preparation; keeps backups and state.
pub fn clear(place: &Place) -> io::Result<()> {
    for name in CLEARED {
        let path = crate::path(&place.directory).join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(path)?,
            Ok(_) => fs::remove_file(path)?,
            Err(why) if why.kind() == io::ErrorKind::NotFound => {}
            Err(why) => return Err(why),
        }
    }
    Ok(())
}
/// Makes the owned layout private, including dump's repository directory.
pub fn layout(place: &Place) -> io::Result<()> {
    for name in ["home", "forgejo", "forgejo/repositories", "runner", "backups"] {
        let path = crate::path(&place.directory).join(name);
        fs::create_dir_all(&path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    write_private(&crate::path(&place.directory).join("home/gitconfig"), b"")
}

/// Whether a backup is already present; inspection failures refuse the preparation.
pub fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(why) => Err(why),
    }
}
/// Syncs a completed partial, renames it and syncs its directory.
pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    fs::File::open(from)?.sync_all()?;
    fs::rename(from, to)?;
    fs::File::open(to.parent().expect("a backup has a parent"))?.sync_all()
}
