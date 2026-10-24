#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "ordinary Rust for table fixtures and assertions (programming-model.md, 10.2)"
)]
use super::fixtures;
use crate::{
    check, limits, problem,
    setup::{Fetch, Mirror, Mode, Origin, Owned, Setup},
};

fn origin(host: &[u8], port: u16) -> Origin {
    Origin { host: host.into(), port }
}
fn read_owned(owned: &str) -> Result<Owned, Box<[u8]>> {
    let source = core::str::from_utf8(fixtures::TWO).unwrap();
    let start = source.find("{\"forgejo\"").unwrap();
    let end = source.rfind("}}}").unwrap().checked_add(1).unwrap();
    let source = format!("{}{}{}", &source[..start], owned, &source[end..]);
    let mut ps = problem::Problems::empty();
    let value = Setup::read(
        &fixtures::document(source.as_bytes()),
        0,
        &limits::small(),
        &jig_protocol_config::Directories::given(b"/config", b"/data", b"/cache").unwrap(),
        &mut ps,
    );
    if !ps.is_empty() {
        return Err(problem::lines(&ps));
    }
    match value.unwrap().entries[1].mode.clone() {
        Mode::Owned(owned) => Ok(owned),
        Mode::Managed { .. } | Mode::Fake(_) => panic!("owned"),
    }
}
fn owned_json(forgejo: &str, runner: Option<&str>, mirror: Option<&str>, public: Option<&str>) -> String {
    let runner = runner.map_or("null".into(), |binary| format!(r#"{{"binary":"{binary}","labels":["temper-host:host"],"capacity":1,"path":"/usr/local/bin:/usr/bin:/bin"}}"#));
    let mirror = mirror.map_or(String::new(), |mirror| format!(",\"mirror\":{mirror}"));
    let public = public.map_or(String::new(), |public| format!(",\"public\":{public}"));
    format!(
        r#"{{"forgejo":"{forgejo}","runner":{runner},"listen":{{"socket":"127.0.0.1:3000"}},"person":{{"login":"ada","email":"ada@example.invalid"}}{mirror}{public}}}"#
    )
}
#[test]
fn owned_binaries_and_mirror() {
    let base = fixtures::owned();
    let mirror = r#"{"forgejo":"https://mirror.test","runner":"https://127.0.0.1:8443"}"#;
    for (forgejo, runner, mirrors) in [
        ("fetched", Some("/opt/forgejo/forgejo-runner"), None),
        ("/opt/forgejo/forgejo", Some("fetched"), None),
        ("fetched", Some("fetched"), None),
        ("/opt/forgejo/forgejo", Some("/opt/forgejo/forgejo-runner"), None),
        ("fetched", None, None),
        ("fetched", Some("fetched"), Some(mirror)),
        ("/opt/forgejo/forgejo", None, Some(mirror)),
    ] {
        let mut expected = base.clone();
        expected.fetch = Fetch {
            forgejo: forgejo == "fetched",
            runner: runner == Some("fetched"),
            mirror: mirrors
                .map(|_| Mirror { forgejo: origin(b"mirror.test", 443), runner: origin(b"127.0.0.1", 8443) }),
        };
        expected.forgejo = if expected.fetch.forgejo {
            b"/cache/forgejo/17.0.0/forgejo".as_slice().into()
        } else {
            forgejo.as_bytes().into()
        };
        expected.runner = runner.map(|binary| {
            let mut value = base.runner.clone().unwrap();
            value.binary = if binary == "fetched" {
                b"/cache/forgejo-runner/13.2.0/forgejo-runner".as_slice().into()
            } else {
                binary.as_bytes().into()
            };
            value
        });
        assert_eq!(read_owned(&owned_json(forgejo, runner, mirrors, None)), Ok(expected));
    }
    for bad in [
        "http://mirror.test",
        "https://mirror.test/",
        "https://mirror.test/releases",
        "https://u@mirror.test",
        "https://mirror.test?a",
        "https://mirror.test#a",
        "https://",
        "https://mirror.test:0",
        "https://mirror.test:65536",
    ] {
        let mirror = format!(r#"{{"forgejo":"{bad}","runner":"https://mirror.test"}}"#);
        assert_eq!(
            read_owned(&owned_json("fetched", None, Some(&mirror), None)),
            Err(b"application.setup[1].mode.owned.mirror.forgejo: invalid form\n".as_slice().into()),
            "{bad}"
        );
    }
    for (mirror, expected) in [
        (r#"{"forgejo":"https://mirror.test"}"#, "application.setup[1].mode.owned.mirror.runner: missing\n"),
        (
            r#"{"forgejo":"https://mirror.test","runner":"https://mirror.test","third":1}"#,
            "application.setup[1].mode.owned.mirror.third: unknown member\n",
        ),
    ] {
        assert_eq!(read_owned(&owned_json("fetched", None, Some(mirror), None)), Err(expected.as_bytes().into()));
    }
}
#[test]
fn owned_public_address() {
    for (public, expected) in [
        (None, None),
        (Some(r#""https://git.example.com""#), Some(origin(b"git.example.com", 443))),
        (Some(r#""https://git.example.com:8443""#), Some(origin(b"git.example.com", 8443))),
    ] {
        let mut owned = fixtures::owned();
        owned.public = expected;
        assert_eq!(
            read_owned(&owned_json("/opt/forgejo/forgejo", Some("/opt/forgejo/forgejo-runner"), None, public)),
            Ok(owned)
        );
    }
}
#[test]
fn owned_public_address_refuses_invalid_forms() {
    for bad in [
        r#""http://git.example.com""#,
        r#""https://git.example.com/""#,
        r#""https://git.example.com/x""#,
        r#""https://git.example.com:0""#,
        r#""git.example.com""#,
        r#""https://git.example.com?q""#,
        r#""https://git.example.com#fragment""#,
        "42",
    ] {
        assert_eq!(
            read_owned(&owned_json("fetched", None, None, Some(bad))),
            Err(b"application.setup[1].mode.owned.public: invalid form\n".as_slice().into()),
            "{bad}"
        );
    }
}
#[test]
fn fetch_bound_through_checked() {
    for (value, expected) in [
        (None, Ok(268_435_456)),
        (Some(1), Ok(1)),
        (Some(u64::MAX), Ok(u64::MAX)),
        (Some(0), Err(b"application.limits.overrides.setup.fetch_bytes: outside its type\n".as_slice())),
    ] {
        let overrides = value.map_or(Vec::new(), |value| vec![(b"setup.fetch_bytes".as_slice().into(), value)]);
        let got = check::checked(
            &fixtures::document(fixtures::TWO),
            0,
            &jig_protocol_config::Directories::working(),
            &fixtures::members(),
            None,
            &overrides,
        );
        match expected {
            Ok(expected) => assert_eq!(got.unwrap().fetch_bytes, expected),
            Err(expected) => assert_eq!(problem::lines(&got.unwrap_err()).as_ref(), expected),
        }
    }
}

use crate::fetch::{self, Event, Failure, Pin, Request};
use skein_lib::{Queue, sha256::Sha256};
fn sha(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.finish()
}
fn unhex(text: &str) -> [u8; 32] {
    let mut value = [0; 32];
    for (value, digits) in value.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *value = u8::from_str_radix(core::str::from_utf8(digits).unwrap(), 16).unwrap();
    }
    value
}
#[test]
#[expect(clippy::too_many_lines, reason = "one table of pinned architectures and selections")]
fn architecture_pins() {
    for (machine, forgejo, runner, mirror, expected) in [
        (
            "x86_64",
            true,
            true,
            false,
            Ok((
                "amd64",
                "5a3b07e124ed70b4e46ee3939515f003bd3cc85352257a9a4607a527251042ca",
                "fadaec897f5e6641c363f87ecaf75f866f99c81b7097ec6b433e4c48123fcad1",
            )),
        ),
        (
            "aarch64",
            true,
            true,
            false,
            Ok((
                "arm64",
                "e5ec437cd6357e8d1b4580cb47ab1daa58a32b33971a63db63446a535d33adb7",
                "9d3d462ee83e4e629f959faeacceb20eed2f25290bddea402e65ace46a31afd9",
            )),
        ),
        (
            "x86_64",
            true,
            false,
            false,
            Ok((
                "amd64",
                "5a3b07e124ed70b4e46ee3939515f003bd3cc85352257a9a4607a527251042ca",
                "fadaec897f5e6641c363f87ecaf75f866f99c81b7097ec6b433e4c48123fcad1",
            )),
        ),
        (
            "x86_64",
            false,
            true,
            false,
            Ok((
                "amd64",
                "5a3b07e124ed70b4e46ee3939515f003bd3cc85352257a9a4607a527251042ca",
                "fadaec897f5e6641c363f87ecaf75f866f99c81b7097ec6b433e4c48123fcad1",
            )),
        ),
        (
            "x86_64",
            true,
            true,
            true,
            Ok((
                "amd64",
                "5a3b07e124ed70b4e46ee3939515f003bd3cc85352257a9a4607a527251042ca",
                "fadaec897f5e6641c363f87ecaf75f866f99c81b7097ec6b433e4c48123fcad1",
            )),
        ),
        ("riscv64", true, true, false, Err(Failure::Architecture(b"riscv64".as_slice().into()))),
        (
            "x86_64",
            false,
            false,
            false,
            Ok((
                "amd64",
                "5a3b07e124ed70b4e46ee3939515f003bd3cc85352257a9a4607a527251042ca",
                "fadaec897f5e6641c363f87ecaf75f866f99c81b7097ec6b433e4c48123fcad1",
            )),
        ),
    ] {
        let mut owned = fixtures::owned();
        owned.fetch = Fetch {
            forgejo,
            runner,
            mirror: mirror.then(|| Mirror { forgejo: origin(b"mirror.test", 443), runner: origin(b"127.0.0.1", 8443) }),
        };
        let expected = expected.map(|(arch, fj, run)| {
            let mut pins = Vec::new();
            if forgejo {
                pins.push(Pin {
                    origin: if mirror { origin(b"mirror.test", 443) } else { origin(b"codeberg.org", 443) },
                    path: format!("/forgejo/forgejo/releases/download/v17.0.0/forgejo-17.0.0-linux-{arch}")
                        .as_bytes()
                        .into(),
                    digest: unhex(fj),
                    file: owned.forgejo.clone(),
                });
            }
            if runner {
                pins.push(Pin {
                    origin: if mirror { origin(b"127.0.0.1", 8443) } else { origin(b"code.forgejo.org", 443) },
                    path: format!("/forgejo/runner/releases/download/v13.2.0/forgejo-runner-13.2.0-linux-{arch}")
                        .as_bytes()
                        .into(),
                    digest: unhex(run),
                    file: owned.runner.as_ref().unwrap().binary.clone(),
                });
            }
            pins.into_boxed_slice()
        });
        assert_eq!(fetch::pins(&owned, machine), expected, "{machine} {forgejo} {runner} {mirror}");
    }
}
fn pin(file: &[u8]) -> Pin {
    Pin {
        origin: origin(b"mirror.test", 8443),
        path: b"/binary".as_slice().into(),
        digest: sha(b"abc"),
        file: file.into(),
    }
}
fn hash(file: &[u8]) -> Request {
    Request::Hash { file: file.into() }
}
fn get() -> Request {
    Request::Get { origin: origin(b"mirror.test", 8443), path: b"/binary".as_slice().into() }
}
fn open(file: &str) -> Request {
    Request::Open { part: format!("{file}.part").as_bytes().into() }
}
fn write(offset: u64, bytes: &[u8]) -> Request {
    Request::Write { offset, bytes: bytes.into() }
}
fn finish(file: &str) -> Request {
    Request::Finish { part: format!("{file}.part").as_bytes().into(), file: file.as_bytes().into() }
}
fn remove() -> Request {
    Request::Remove { part: b"/cache/bin.part".as_slice().into() }
}
fn url() -> Box<[u8]> {
    b"https://mirror.test:8443/binary".as_slice().into()
}
#[test]
#[expect(clippy::too_many_lines, reason = "one table of complete request and event traces")]
fn fetch_machine() {
    let success = vec![
        (Event::Begin, hash(b"/cache/bin")),
        (Event::Hashed(None), get()),
        (Event::Answered { status: 200, length: Some(3) }, open("/cache/bin")),
        (Event::Done, Request::Read),
        (Event::Chunk(b"a".as_slice().into()), write(0, b"a")),
        (Event::Done, Request::Read),
        (Event::Chunk(b"bc".as_slice().into()), write(1, b"bc")),
        (Event::Done, Request::Read),
        (Event::Ended, finish("/cache/bin")),
        (Event::Done, Request::End(Ok(()))),
    ];
    let mut two = success[..9]
        .iter()
        .map(|(event, request)| {
            (
                event.clone(),
                match request {
                    Request::Hash { file } => hash(file),
                    Request::Get { .. } => get(),
                    Request::Open { .. } => open("/cache/bin"),
                    Request::Read => Request::Read,
                    Request::Write { offset, bytes } => write(*offset, bytes),
                    Request::Finish { .. } => finish("/cache/bin"),
                    Request::Remove { .. } | Request::End(_) => unreachable!(),
                },
            )
        })
        .collect::<Vec<_>>();
    two.extend([
        (Event::Done, hash(b"/cache/two")),
        (Event::Hashed(None), get()),
        (Event::Answered { status: 200, length: None }, open("/cache/two")),
        (Event::Done, Request::Read),
        (Event::Chunk(b"abc".as_slice().into()), write(0, b"abc")),
        (Event::Done, Request::Read),
        (Event::Ended, finish("/cache/two")),
        (Event::Done, Request::End(Ok(()))),
    ]);
    for (name, count, bound, steps) in [
        ("absent two chunks", 1, 10, success),
        (
            "matching",
            1,
            3,
            vec![(Event::Begin, hash(b"/cache/bin")), (Event::Hashed(Some(sha(b"abc"))), Request::End(Ok(())))],
        ),
        (
            "present mismatch stops before second",
            2,
            3,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (
                    Event::Hashed(Some(sha(b"x"))),
                    Request::End(Err(Failure::Present { file: b"/cache/bin".as_slice().into() })),
                ),
            ],
        ),
        (
            "302",
            1,
            3,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (
                    Event::Answered { status: 302, length: None },
                    Request::End(Err(Failure::Status { url: url(), status: 302 })),
                ),
            ],
        ),
        (
            "404",
            1,
            3,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (
                    Event::Answered { status: 404, length: None },
                    Request::End(Err(Failure::Status { url: url(), status: 404 })),
                ),
            ],
        ),
        (
            "length past bound",
            1,
            2,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (Event::Answered { status: 200, length: Some(3) }, Request::End(Err(Failure::Bytes { url: url() }))),
            ],
        ),
        (
            "length at bound",
            1,
            3,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (Event::Answered { status: 200, length: Some(3) }, open("/cache/bin")),
                (Event::Done, Request::Read),
                (Event::Chunk(b"abc".as_slice().into()), write(0, b"abc")),
                (Event::Done, Request::Read),
                (Event::Ended, finish("/cache/bin")),
                (Event::Done, Request::End(Ok(()))),
            ],
        ),
        (
            "stream past bound",
            1,
            2,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (Event::Answered { status: 200, length: None }, open("/cache/bin")),
                (Event::Done, Request::Read),
                (Event::Chunk(b"ab".as_slice().into()), write(0, b"ab")),
                (Event::Done, Request::Read),
                (Event::Chunk(b"c".as_slice().into()), remove()),
                (Event::Done, Request::End(Err(Failure::Bytes { url: url() }))),
            ],
        ),
        (
            "wrong digest",
            1,
            3,
            vec![
                (Event::Begin, hash(b"/cache/bin")),
                (Event::Hashed(None), get()),
                (Event::Answered { status: 200, length: None }, open("/cache/bin")),
                (Event::Done, Request::Read),
                (Event::Chunk(b"x".as_slice().into()), write(0, b"x")),
                (Event::Done, Request::Read),
                (Event::Ended, remove()),
                (Event::Done, Request::End(Err(Failure::Digest { url: url(), pinned: sha(b"abc"), got: sha(b"x") }))),
            ],
        ),
        ("both fetched", 2, 3, two),
        ("none", 0, 3, vec![(Event::Begin, Request::End(Ok(())))]),
    ] {
        let pins = match count {
            0_u8 => vec![],
            1 => vec![pin(b"/cache/bin")],
            2 => vec![pin(b"/cache/bin"), pin(b"/cache/two")],
            _ => unreachable!(),
        };
        let mut machine = fetch::Fetch::new(pins.into_boxed_slice(), bound);
        let mut out = Queue::with_capacity(fetch::MAX_OUT);
        for (event, expected) in steps {
            fetch::step(&mut machine, event, &mut out);
            assert_eq!(out.pop(), Some(expected), "{name}");
            assert!(out.is_empty());
        }
    }
}
#[test]
fn failures_words_and_urls() {
    for (failure, expected) in [
        (Failure::Architecture(b"riscv64".as_slice().into()), "no pinned binaries for architecture riscv64"),
        (Failure::Present { file: b"/cache/bin".as_slice().into() }, "/cache/bin: does not match the pinned digest"),
        (Failure::Status { url: url(), status: 302 }, "https://mirror.test:8443/binary: HTTP status 302"),
        (Failure::Bytes { url: url() }, "https://mirror.test:8443/binary: body past setup.fetch_bytes"),
        (
            Failure::Digest { url: url(), pinned: sha(b"abc"), got: sha(b"x") },
            "https://mirror.test:8443/binary: digest differs: pinned ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad, got 2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881",
        ),
    ] {
        assert_eq!(failure.words().as_ref(), expected.as_bytes());
    }
    assert_eq!(fetch::url(&origin(b"mirror.test", 443), b"/binary").as_ref(), b"https://mirror.test/binary");
}

fn command(action: &str, forge: u16, which: Option<&str>, flags: &[Box<[u8]>]) -> crate::Step {
    let number = forge.to_string();
    let mut arguments = vec![b"forge".as_slice().into(), action.as_bytes().into()];
    arguments.extend_from_slice(flags);
    arguments.push(number.as_bytes().into());
    if let Some(which) = which {
        arguments.push(which.as_bytes().into());
    }
    let name = format!("{} {forge}", which.unwrap_or(action));
    crate::Step::new(
        name.as_bytes().into(),
        arguments.into_boxed_slice(),
        if which.is_some() { crate::Kind::Long } else { crate::Kind::Once },
    )
}
#[test]
#[expect(clippy::too_many_lines, reason = "one table of complete steps and asked groups")]
fn groups_fetch_before_preparation() {
    use crate::{Group, Stage, Steps};
    let flags: Box<[Box<[u8]>]> =
        [b"--config".as_slice(), b"/config", b"--data", b"/data", b"--cache", b"/cache"].map(Box::from).into();
    let cmd = |action, forge, which| {
        let step = command(action, forge, which, &flags);
        if which == Some("forgejo") { step.with_ready(([127, 0, 0, 1], 3000).into()) } else { step }
    };
    let serve = || {
        let mut args = vec![b"serve".as_slice().into()];
        args.extend_from_slice(&flags);
        args.push(b"--box".as_slice().into());
        crate::Step::new(b"serve".as_slice().into(), args.into_boxed_slice(), crate::Kind::Long)
            .listening(Box::new([([127, 0, 0, 1], 8367).into(), ([127, 0, 0, 1], 8371).into()]))
    };
    for (forgejo, runner, asked, preceding, expected) in [
        (
            true,
            true,
            false,
            false,
            Steps {
                always: vec![
                    cmd("fetch", 3, None),
                    cmd("prepare", 3, None),
                    cmd("run", 3, Some("forgejo")),
                    cmd("run", 3, Some("runner")),
                    serve(),
                ]
                .into_boxed_slice(),
                asked: Box::new([]),
            },
        ),
        (
            true,
            true,
            true,
            true,
            Steps {
                always: vec![
                    cmd("prepare", 1, None),
                    cmd("run", 1, Some("forgejo")),
                    cmd("run", 1, Some("runner")),
                    serve(),
                ]
                .into_boxed_slice(),
                asked: Box::new([Group {
                    recorded: true,
                    forge: 3,
                    before: 3,
                    steps: Box::new([
                        (Stage::Fetch, cmd("fetch", 3, None)),
                        (Stage::Prepare, cmd("prepare", 3, None)),
                        (Stage::Run, cmd("run", 3, Some("forgejo"))),
                        (Stage::Run, cmd("run", 3, Some("runner"))),
                    ]),
                }]),
            },
        ),
        (
            false,
            true,
            false,
            false,
            Steps {
                always: vec![
                    cmd("fetch", 3, None),
                    cmd("prepare", 3, None),
                    cmd("run", 3, Some("forgejo")),
                    cmd("run", 3, Some("runner")),
                    serve(),
                ]
                .into_boxed_slice(),
                asked: Box::new([]),
            },
        ),
        (
            false,
            false,
            false,
            false,
            Steps {
                always: vec![
                    cmd("prepare", 3, None),
                    cmd("run", 3, Some("forgejo")),
                    cmd("run", 3, Some("runner")),
                    serve(),
                ]
                .into_boxed_slice(),
                asked: Box::new([]),
            },
        ),
    ] {
        let mut entry = fixtures::setup().entries[1].clone();
        entry.forge = 3;
        entry.asked = asked;
        let Mode::Owned(owned) = &mut entry.mode else { panic!("owned") };
        owned.fetch.forgejo = forgejo;
        owned.fetch.runner = runner;
        let entries = if preceding {
            let mut first = fixtures::setup().entries[1].clone();
            first.forge = 1;
            vec![first, entry]
        } else {
            vec![entry]
        };
        let setup = Setup { entries: entries.into_boxed_slice() };
        assert_eq!(
            crate::steps(&setup, &flags, Box::new([([127, 0, 0, 1], 8367).into(), ([127, 0, 0, 1], 8371).into()])),
            expected,
            "{forgejo} {runner} {asked}"
        );
    }
}
