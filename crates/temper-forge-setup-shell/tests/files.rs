pub mod support;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
};
use temper_forge_setup_shell::files;
#[test]
fn a_file_is_written_whole_private_and_renamed_over() {
    let scratch = support::Scratch::new("file_whole");
    let path = scratch.path().join("value");
    files::write_private(&path, b"before").expect("first write");
    let first = fs::metadata(&path).expect("first inode").ino();
    files::write_private(&path, b"after").expect("replacement");
    assert_eq!(fs::read(&path).expect("whole file"), b"after");
    assert_eq!(fs::metadata(&path).expect("mode").permissions().mode() & 0o777, 0o600);
    assert_ne!(fs::metadata(&path).expect("new inode").ino(), first);
    assert_eq!(fs::read_dir(scratch.path()).expect("directory").count(), 1);
    scratch.remove();
}
#[test]
fn clear_removes_a_first_preparations_files_and_never_backups() {
    let scratch = support::Scratch::new("file_clear");
    files::layout(&scratch.place()).expect("layout");
    for name in [
        "state",
        "backups/dump.zip",
        "forgejo/leftover",
        "runner/secret",
        "admin_token",
        "person_password",
        "person_read.header",
        "person_write.header",
    ] {
        fs::write(scratch.path().join(name), b"kept").expect("file");
    }
    files::clear(&scratch.place()).expect("cleared");
    for name in ["forgejo", "runner", "admin_token", "person_password", "person_read.header", "person_write.header"] {
        assert!(!scratch.path().join(name).exists(), "{name} removed");
    }
    for name in ["state", "backups/dump.zip"] {
        assert_eq!(fs::read(scratch.path().join(name)).expect("retained"), b"kept", "{name}");
    }
    scratch.remove();
}

#[test]
fn a_renamed_file_exists_under_its_new_name_only() {
    let scratch = support::Scratch::new("file_rename");
    let from = scratch.path().join("dump.partial.zip");
    let to = scratch.path().join("backup.zip");
    files::write_private(&from, b"PKcontents").expect("partial");
    assert!(!files::exists(&to).expect("absent"));
    files::rename(&from, &to).expect("rename");
    assert!(!files::exists(&from).expect("old name"));
    assert!(files::exists(&to).expect("backup"));
    assert_eq!(fs::read(to).expect("whole backup"), b"PKcontents");
    scratch.remove();
}
