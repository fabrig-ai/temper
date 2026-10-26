pub mod support;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
};
use temper_forge_setup::{
    prepare::Prepare,
    setup::{Account, Fetch, Owned},
    state,
};
use temper_forge_setup_shell::{files, prepare};

fn owned(scratch: &support::Scratch) -> Owned {
    let program = support::script(
        scratch.path(),
        r#"
case "$*" in
  --version) echo 'forgejo version 17.0.0';;
  *'admin user create'*) echo "generated random password is 'private-password'";;
  *'generate-access-token'*) echo 'private-token';;
esac
"#,
    );
    Owned {
        public: None,
        fetch: Fetch { forgejo: false, runner: false, mirror: None },
        forgejo: program.program,
        runner: None,
        listen: "127.0.0.1:3000".parse().expect("fixture"),
        person: Account { login: b"ada".as_slice().into(), email: b"ada@example.invalid".as_slice().into() },
    }
}
fn run(scratch: &support::Scratch, owned: &Owned) {
    prepare::run(&support::io(), &scratch.place(), Prepare::new(owned.clone(), scratch.place(), None))
        .expect("prepared");
}
#[test]
fn person_headers_are_whole_private_and_each_missing_header_is_remade_alone() {
    let scratch = support::Scratch::new("person_headers");
    let owned = owned(&scratch);
    run(&scratch, &owned);
    let names = ["person_read.header", "person_write.header"];
    for name in names {
        let path = scratch.path().join(name);
        assert_eq!(fs::read(&path).expect("fixture"), b"Authorization: token private-token\n");
        assert_eq!(fs::metadata(&path).expect("fixture").permissions().mode() & 0o777, 0o600);
    }
    assert_eq!(
        fs::metadata(scratch.path().join("person_password")).expect("fixture").permissions().mode() & 0o777,
        0o600
    );
    for (missing, kept) in [(names[0], names[1]), (names[1], names[0])] {
        let path = scratch.path().join(kept);
        files::write_private(&path, b"Authorization: token kept-token\n").expect("fixture");
        let inode = fs::metadata(&path).expect("fixture").ino();
        fs::remove_file(scratch.path().join(missing)).expect("fixture");
        run(&scratch, &owned);
        assert_eq!(fs::read(&path).expect("fixture"), b"Authorization: token kept-token\n");
        assert_eq!(fs::metadata(&path).expect("fixture").ino(), inode);
        assert_eq!(fs::read(scratch.path().join(missing)).expect("fixture"), b"Authorization: token private-token\n");
    }
    scratch.remove();
}
#[test]
fn an_interrupted_first_preparation_discards_both_headers_before_starting_again() {
    let scratch = support::Scratch::new("person_headers_interrupted");
    let owned = owned(&scratch);
    run(&scratch, &owned);
    let path = scratch.path().join("state");
    let mut state = state::decode(&fs::read(&path).expect("fixture")).expect("fixture");
    state.prepared = false;
    files::write_private(&path, state::encode(&state).expose()).expect("fixture");
    for name in ["person_read.header", "person_write.header"] {
        files::write_private(&scratch.path().join(name), b"Authorization: token abandoned-token\n").expect("fixture");
    }
    run(&scratch, &owned);
    for name in ["person_read.header", "person_write.header"] {
        assert_eq!(fs::read(scratch.path().join(name)).expect("fixture"), b"Authorization: token private-token\n");
    }
    assert!(state::decode(&fs::read(path).expect("fixture")).expect("fixture").prepared);
    scratch.remove();
}
