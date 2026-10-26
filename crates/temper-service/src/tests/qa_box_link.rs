//! Design-only QA: protocol/forge.md 2.8, 8.1; domain/forge.md 4.1.
use super::fixtures;
use skein_io::kernel::Fd;
use skein_lib::Token;
use temper_engine_domain as root;
use temper_engine_domain_forge::{Connection, SetUpStep};

#[test]
fn qa_unasked_and_invalid_lines_are_named_without_state_changes() {
    let mut stand = fixtures::one_process();
    stand.service.adopt_box(Fd::new(3), Fd::new(4), fixtures::system::BOX_BYTES).unwrap();
    fixtures::until_hosting(&mut stand);
    crate::setup::lifecycle::state(stand.service.engine.as_mut().unwrap(), 3, Connection::NotSetUp);
    for _ in 0_u8..64 {
        stand.turn();
    }
    for asked in [false, true] {
        if asked {
            crate::route::request(
                stand.service.engine.as_mut().unwrap(),
                root::Request::SetUp {
                    playground: false,
                    forge: 3,
                    request: Token::new(91),
                    org: b"work".as_slice().into(),
                    repository: b"hello".as_slice().into(),
                },
            );
            for _ in 0_u8..64 {
                stand.turn();
            }
        }
        for line in [
            b"step 4 fetch\n".as_slice(),
            b"failed 4 other\n",
            b"started 4\n",
            b"mystery 3\n",
            b"step xyz fetch\n",
            b"step 65536 fetch\n",
            b"step 3 unknown\n",
            b"\n",
        ] {
            for byte in line {
                stand.kernel.box_answers.push(*byte);
            }
            for _ in 0_u8..64 {
                stand.turn();
            }
            let Some(crate::Note::BoxLink { .. }) = stand.service.notes().pop() else {
                panic!("line {line:?} must be named");
            };
            assert!(stand.service.notes().is_empty(), "one diagnostic per line");
            assert_eq!(
                stand.service.engine.as_ref().unwrap().domain.forge_connection(3),
                if asked { Connection::SettingUp { step: None } } else { Connection::NotSetUp }
            );
            assert_eq!(stand.kernel.box_written.as_slice(), if asked { b"start 3\n".as_slice() } else { b"" });
        }
    }
}

#[test]
fn qa_split_lines_and_coalesced_steps_keep_the_last_step() {
    let mut stand = fixtures::one_process();
    stand.service.adopt_box(Fd::new(3), Fd::new(4), fixtures::system::BOX_BYTES).unwrap();
    fixtures::until_hosting(&mut stand);
    crate::setup::lifecycle::state(stand.service.engine.as_mut().unwrap(), 3, Connection::NotSetUp);
    for _ in 0_u8..64 {
        stand.turn();
    }
    crate::route::request(
        stand.service.engine.as_mut().unwrap(),
        root::Request::SetUp {
            playground: false,
            forge: 3,
            request: Token::new(91),
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
        },
    );
    for _ in 0_u8..64 {
        stand.turn();
    }
    for byte in b"step 3 fet" {
        stand.kernel.box_answers.push(*byte);
    }
    for _ in 0_u8..64 {
        stand.turn();
    }
    assert_eq!(stand.service.engine.as_ref().unwrap().domain.forge_connection(3), Connection::SettingUp { step: None });
    assert!(stand.service.notes().is_empty());
    for byte in b"ch\n" {
        stand.kernel.box_answers.push(*byte);
    }
    for _ in 0_u8..64 {
        stand.turn();
    }
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(3),
        Connection::SettingUp { step: Some(SetUpStep::Fetch) }
    );
    for byte in b"step 3 prepare\nstep 3 run\n" {
        stand.kernel.box_answers.push(*byte);
    }
    for _ in 0_u8..64 {
        stand.turn();
    }
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(3),
        Connection::SettingUp { step: Some(SetUpStep::Run) }
    );
    assert!(stand.service.notes().is_empty());
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 3\n");
}

#[test]
fn qa_line_byte_bound_includes_lf_and_recovers_after_overflow() {
    const BOUND: u32 = 32;
    for (why, exact, overflow) in [
        ("abcdefghijklmnopqrstuv", "failed 3 abcdefghijklmnopqrstuv\n", "failed 3 abcdefghijklmnopqrstuvx\n"),
        ("ééééééééééé", "failed 3 ééééééééééé\n", "failed 3 éééééééééééx\n"),
    ] {
        let mut stand = fixtures::one_process();
        stand.service.adopt_box(Fd::new(3), Fd::new(4), BOUND).unwrap();
        fixtures::until_hosting(&mut stand);
        crate::setup::lifecycle::state(stand.service.engine.as_mut().unwrap(), 3, Connection::NotSetUp);
        for _ in 0_u8..64 {
            stand.turn();
        }
        crate::route::request(
            stand.service.engine.as_mut().unwrap(),
            root::Request::SetUp {
                playground: false,
                forge: 3,
                request: Token::new(91),
                org: b"work".as_slice().into(),
                repository: b"hello".as_slice().into(),
            },
        );
        for _ in 0_u8..64 {
            stand.turn();
        }
        assert_eq!(overflow.len(), usize::try_from(BOUND).unwrap() + 1);
        for byte in overflow.as_bytes() {
            stand.kernel.box_answers.push(*byte);
        }
        for _ in 0_u8..64 {
            stand.turn();
        }
        assert_eq!(
            stand.service.engine.as_ref().unwrap().domain.forge_connection(3),
            Connection::SettingUp { step: None }
        );
        let Some(crate::Note::BoxLink { line }) = stand.service.notes().pop() else {
            panic!("overflow must be named");
        };
        let mut names_bound = false;
        for pair in line.windows(2) {
            if pair == b"32" {
                names_bound = true;
            }
        }
        assert!(names_bound, "diagnostic names bound");
        assert!(stand.service.notes().is_empty());
        assert_eq!(exact.len(), usize::try_from(BOUND).unwrap());
        for byte in exact.as_bytes() {
            stand.kernel.box_answers.push(*byte);
        }
        for _ in 0_u8..64 {
            stand.turn();
        }
        assert_eq!(
            stand.service.engine.as_ref().unwrap().domain.forge_connection(3),
            Connection::SetUpFailed { why: why.as_bytes().into() }
        );
        assert!(stand.service.notes().is_empty());
        assert_eq!(stand.kernel.box_written.as_slice(), b"start 3\n");
    }
}
