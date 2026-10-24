use super::*;
use crate::Command;
use std::io::Read;

fn configured(arguments: &[&str], environment: &[(&str, &str)]) -> Result<Command, String> {
    let arguments = arguments.iter().map(|s| s.as_bytes().into()).collect::<Vec<_>>();
    let environment = environment.iter().map(|(k, v)| (k.as_bytes().into(), v.as_bytes().into())).collect::<Vec<_>>();
    crate::configure_with(&arguments, &environment, b"/work")
}
#[test]
fn arguments() {
    for (words, environment, kind, document, ready, seed, observations) in [
        (vec!["fake", "llm", "--document", "llm.json"], vec![], Kind::Llm, "llm.json", None, 0, None),
        (vec!["fake", "forge", "--document", "forge.json"], vec![], Kind::Forge, "forge.json", None, 0, None),
        (vec!["fake", "oauth", "--document", "oauth.json"], vec![], Kind::Oauth, "oauth.json", None, 0, None),
        (
            vec!["fake", "oauth", "--document", "oauth.json", "--ready", "", "--seed", "23"],
            vec![],
            Kind::Oauth,
            "oauth.json",
            Some(""),
            23,
            None,
        ),
        (
            vec!["fake", "llm", "--document", "doc", "--observations", "seen"],
            vec![],
            Kind::Llm,
            "doc",
            None,
            0,
            Some("seen"),
        ),
        (
            vec!["fake", "llm", "--document", "llm.json", "--ready", "ready ", "--seed", "23"],
            vec![],
            Kind::Llm,
            "llm.json",
            Some("ready "),
            23,
            None,
        ),
        (
            vec!["fake", "forge", "--document", "forge.json", "--ready", "", "--observations", "seen"],
            vec![],
            Kind::Forge,
            "forge.json",
            Some(""),
            0,
            Some("seen"),
        ),
        (
            vec!["fake", "forge"],
            vec![
                ("TEMPER_DOCUMENT", "env.json"),
                ("TEMPER_READY", "up "),
                ("TEMPER_SEED", "42"),
                ("TEMPER_OBSERVATIONS", "seen"),
            ],
            Kind::Forge,
            "env.json",
            Some("up "),
            42,
            Some("seen"),
        ),
        (
            vec!["fake", "llm", "--document", "flag.json", "--seed", "7"],
            vec![("TEMPER_DOCUMENT", "env.json"), ("TEMPER_SEED", "42")],
            Kind::Llm,
            "flag.json",
            None,
            7,
            None,
        ),
    ] {
        let Command::Fake { arguments } = configured(&words, &environment).expect("valid fixture") else {
            panic!("fake command");
        };
        assert_eq!(
            arguments,
            Arguments {
                kind,
                source: Source::Document(document.into()),
                ready: ready.map(|s| s.as_bytes().into()),
                seed,
                observations: observations.map(Into::into)
            }
        );
    }
}

#[test]
fn argument_refusals() {
    for (words, expected) in [
        (vec!["fake"], "temper fake: expected llm, forge, oauth or git\n"),
        (
            vec!["fake", "oauth"],
            "temper fake oauth: usage: temper fake oauth --document FILE [--ready PREFIX] [--seed N]\n",
        ),
        (
            vec!["fake", "oauth", "1"],
            "temper fake oauth: usage: temper fake oauth --document FILE [--ready PREFIX] [--seed N]\n",
        ),
        (
            vec!["fake", "oauth", "--document", "doc", "--seed", "no"],
            "temper fake oauth: --seed N (TEMPER_SEED): expected a decimal u64\n",
        ),
        (
            vec!["fake", "oauth", "--document", "doc", "--observations", "seen"],
            "temper fake oauth: unexpected argument --observations\n",
        ),
        (vec!["fake", "other"], "temper fake: unknown fake other\n"),
        (
            vec!["fake", "llm"],
            "temper fake llm: --config DIR is not given, TEMPER_CONFIG is not set, and its default $XDG_CONFIG_HOME/temper needs XDG_CONFIG_HOME or HOME as an absolute path\n",
        ),
        (
            vec!["fake", "llm", "--document", "doc", "--seed", "no"],
            "temper fake llm: --seed N (TEMPER_SEED): expected a decimal u64\n",
        ),
        (
            vec!["fake", "forge", "--document", "doc", "extra"],
            "temper fake forge: usage: temper fake forge [DIRECTORIES] N [--ready PREFIX] [--seed N] [--observations FILE] | temper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE]\n",
        ),
    ] {
        assert_eq!(configured(&words, &[]).expect_err("refused fixture"), expected);
    }
}

fn check_directories(config: &str, data: &str, cache: &str, origin: Origin) -> crate::directories::Directories {
    crate::directories::Directories {
        config: config.as_bytes().into(),
        data: data.as_bytes().into(),
        cache: cache.as_bytes().into(),
        origins: [origin; 3],
    }
}
#[test]
fn check_arguments() {
    for (words, environment, directories, forge) in [
        (
            vec!["fake", "forge", "check", "0", "observations", "copy"],
            vec![("HOME", "/home/test")],
            check_directories(
                "/home/test/.config/temper",
                "/home/test/.local/share/temper",
                "/home/test/.cache/temper",
                Origin::Home,
            ),
            0,
        ),
        (
            vec![
                "fake",
                "forge",
                "check",
                "--data",
                "data",
                "--config",
                "/config",
                "--cache",
                "cache",
                "65535",
                "observations",
                "copy",
            ],
            vec![],
            check_directories("/config", "/work/data", "/work/cache", Origin::Flag),
            u16::MAX,
        ),
    ] {
        assert_eq!(
            configured(&words, &environment),
            Ok(Command::FakeCheck {
                directories,
                check: Check { forge, observations: "observations".into(), store: "copy".into() },
            })
        );
    }
    for (words, expected) in [
        (vec!["fake", "forge", "check"], "temper fake forge check: expected N OBSERVATIONS STORE\n"),
        (vec!["fake", "forge", "check", "1"], "temper fake forge check: expected N OBSERVATIONS STORE\n"),
        (
            vec!["fake", "forge", "check", "1", "observations"],
            "temper fake forge check: expected N OBSERVATIONS STORE\n",
        ),
        (
            vec!["fake", "forge", "check", "1", "observations", "copy", "extra"],
            "temper fake forge check: expected N OBSERVATIONS STORE\n",
        ),
        (
            vec!["fake", "forge", "check", "x", "observations", "copy"],
            "temper fake forge check: N: expected a forge number, a decimal u16\n",
        ),
        (
            vec!["fake", "forge", "check", "65536", "observations", "copy"],
            "temper fake forge check: N: expected a forge number, a decimal u16\n",
        ),
        (
            vec!["fake", "forge", "check", "-1", "observations", "copy"],
            "temper fake forge check: N: expected a forge number, a decimal u16\n",
        ),
        (
            vec!["fake", "forge", "check", "+1", "observations", "copy"],
            "temper fake forge check: N: expected a forge number, a decimal u16\n",
        ),
        (
            vec!["fake", "forge", "check", "1", "observations", "copy", "--unknown"],
            "temper fake forge check: expected N OBSERVATIONS STORE\n",
        ),
        (vec!["fake", "forge", "check", "--config"], "temper fake forge check: --config takes a value: DIR\n"),
    ] {
        assert_eq!(configured(&words, &[("HOME", "/home/test")]).expect_err("refused fixture"), expected);
    }
}

#[test]
fn git_dispatch() {
    for (program, words, expected) in [
        ("git", vec![], Some(vec![])),
        ("/abs/bin/git", vec![], Some(vec![])),
        ("./git", vec![], Some(vec![])),
        ("temper", vec![], None),
        ("/usr/bin/temper", vec![], None),
        ("gitx", vec![], None),
        ("git.exe", vec![], None),
        ("temper", vec!["fake", "git", "version"], Some(vec!["version"])),
    ] {
        let arguments = if git::is_git(std::ffi::OsStr::new(program)) {
            Some(words.iter().map(|s| Box::from(s.as_bytes())).collect::<Vec<_>>())
        } else {
            match configured(&words, &[]) {
                Ok(Command::FakeGit { arguments }) => Some(arguments.into_vec()),
                _ => None,
            }
        };
        let expected = expected.map(|words| words.iter().map(|s| Box::from(s.as_bytes())).collect::<Vec<_>>());
        assert_eq!(arguments, expected, "{program} {words:?}");
    }
}

fn advance(host: &Host, content: &[u8]) {
    let mut shared = host.shared.as_ref().expect("forge").borrow_mut();
    let env = skein_lib::Env { now: Time::ZERO, wall: Wall::EPOCH, limits: forge::LIMITS };
    skein_fake_forge_domain::advance(&mut shared.domain, &env, b"fake/widget", b"main", b"new", content, forge::OWNER)
        .expect("valid fixture");
}

#[test]
fn observation_write_failure_drains_the_peer_and_keeps_its_outputs() {
    // The read-only descriptor fails writes with EBADF without damaging the
    // real FILE. Restoring write access after failure detects later writes,
    // including a misleading checkpoint at stop. The tiny buffer also checks
    // the drain's bound failure under the same refused-reply contract.
    for (name, checkpoint, bound_failure) in
        [("first", false, false), ("checkpoint", true, false), ("bound", true, true)]
    {
        let directory = std::env::temp_dir().join(format!("temper-fake-write-failure-{}-{name}", std::process::id()));
        crate::directories::make_private(&directory, skein_shell::effective_user()).expect("valid fixture");
        let document = directory.join("forge.json");
        fs::write(
            &document,
            serde_json::to_vec(&serde_json::json!({"listen":{"socket":"127.0.0.1:0"},
            "admin_token":directory.join("forge/admin_token"),"owner":"fake",
            "repositories":[{"name":"widget","landing":"main","ci":null}]}))
            .expect("valid fixture"),
        )
        .expect("valid fixture");
        let path = directory.join("observations");
        let arguments = Arguments {
            kind: Kind::Forge,
            source: Source::Document(document.clone()),
            ready: None,
            seed: 23,
            observations: Some(path.clone()),
        };
        let (mut host, mut kernel) = start(&arguments, &mut Created::default()).expect("valid fixture");
        if checkpoint {
            advance(&host, b"before");
            host.observations();
        }
        let clock = skein_shell::Clock::new();
        let deadline = clock.now().now.saturating_add(skein_lib::Duration::from_millis(300));
        let mut client = None;
        let before = loop {
            assert!(clock.now().now < deadline, "{name}: reaches the reply pass");
            kernel.reap(host.completions());
            let skein_shell::Now { now, wall } = clock.now();
            host.iterate(now, wall);
            if client.is_none()
                && let Some(address) = host.peer.address()
            {
                let mut socket = std::net::TcpStream::connect(address).expect("client");
                socket.set_read_timeout(Some(std::time::Duration::from_millis(300))).expect("timeout");
                let token = credential(23, credential::ADMIN_TOKEN);
                write!(socket, "GET /api/v1/repositories/1 HTTP/1.1\r\nHost: localhost\r\nAuthorization: token {}\r\nConnection: keep-alive\r\n\r\n",
                    std::str::from_utf8(&token).expect("hex")).expect("request");
                client = Some(socket);
            }
            let reply = host.submissions.iter().any(|submission| matches!(submission.kind, Op::Send { .. }));
            if reply {
                // A request is in this pass, with its reply still unsubmitted.
                advance(&host, b"lost");
                if bound_failure {
                    host.buffer = List::with_capacity(1);
                } else {
                    host.observations = Some(File::open(&path).expect("read-only writer seam"));
                }
            }
            let before = fs::read(&path).expect("FILE before this pass");
            host.drain();
            if reply {
                assert!(host.failed, "{name}: the pass's write fails");
                assert!(
                    host.completions
                        .iter()
                        .any(|c| matches!(c.kind, Op::Send { .. })
                            && c.result == Err(skein_io::kernel::Error::BrokenPipe)),
                    "{name}: reply refused"
                );
                host.observations = Some(OpenOptions::new().append(true).open(&path).expect("restore writer"));
                advance(&host, b"later");
                break before;
            }
            kernel.submit(host.submissions(), skein_shell::Wait::No);
        };
        // The client's sending half remains open: settling must cancel Recv.
        assert_eq!(skein_shell::drive(&mut kernel, &clock, &mut host), Exit::Code(1), "{name}");
        let mut client = client.expect("open client");
        let mut bytes = Vec::new();
        match client.read_to_end(&mut bytes) {
            Ok(_) => {}
            Err(why) if why.kind() == io::ErrorKind::ConnectionReset => {}
            Err(why) => panic!("{name}: client end: {why}"),
        }
        assert!(bytes.is_empty(), "{name}: no reply bytes");
        assert!(host.is_empty());
        assert_eq!(kernel.in_flight(), 0);
        assert!(host.shared.as_ref().expect("forge").borrow_mut().domain.pop_observation().is_none());
        assert_eq!(fs::read(&path).expect("FILE"), before, "{name}: no checkpoint after loss");
        if checkpoint {
            let records = String::from_utf8(before).expect("JSON lines");
            assert_eq!(records.lines().last(), Some("{\"observations_lost\":0}"));
        } else {
            assert_eq!(before, b"{\"observations_lost\":0}\n", "{name}: the first drain failed");
        }
        assert!(document.exists());
        assert!(directory.join("forge/admin_token").exists());
        fs::remove_dir_all(directory).expect("valid fixture");
    }
}

#[test]
fn llm_records_full_counts_connections_without_queries() {
    let directory = std::env::temp_dir().join(format!("temper-llm-records-full-{}", std::process::id()));
    crate::directories::make_private(&directory, skein_shell::effective_user()).expect("private fixture");
    let document = directory.join("llm.json");
    fs::write(
        &document,
        serde_json::to_vec(&serde_json::json!({
            "listen":{"socket":"127.0.0.1:0"},"key":directory.join("key"),"tls":null,
            "models":[{"name":"qa","window":4096,"output":1024}],
            "cued":[],"asked":[],"otherwise":["answer"]
        }))
        .expect("document"),
    )
    .expect("fixture");
    let path = directory.join("observations");
    let arguments = Arguments {
        kind: Kind::Llm,
        source: Source::Document(document),
        ready: None,
        seed: 23,
        observations: Some(path.clone()),
    };
    let (mut host, mut kernel) = start(&arguments, &mut Created::default()).expect("fixture");
    let clock = skein_shell::Clock::new();
    let mut connected = false;
    let mut seen = 0;
    while !host.failed {
        kernel.reap(host.completions());
        let skein_shell::Now { now, wall } = clock.now();
        host.iterate(now, wall);
        host.drain();
        let Peer::Llm(peer) = &host.peer else { panic!("LLM peer") };
        let closed =
            peer.observations().iter().skip(seen).any(|record| matches!(record, llm_peer::Observation::Closed { .. }));
        seen = peer.observations().len();
        if !host.failed
            && (!connected || closed)
            && let Some(address) = host.peer.address()
        {
            // Accept and close are retained records even though no query is sent.
            drop(std::net::TcpStream::connect(address).expect("connect"));
            connected = true;
        }
        let wait = if host.work_pending(now) {
            skein_shell::Wait::No
        } else {
            host.next_deadline().map_or(skein_shell::Wait::Forever, skein_shell::Wait::Until)
        };
        kernel.submit(host.submissions(), wait);
    }
    assert_eq!(host.llm_cursor, usize::try_from(limits::LLM_OBSERVATIONS).expect("record cap"));
    assert_eq!(skein_shell::drive(&mut kernel, &clock, &mut host), Exit::Code(1));
    assert!(host.is_empty());
    assert_eq!(kernel.in_flight(), 0);
    let records = fs::read_to_string(&path).expect("records");
    let rows: Vec<serde_json::Value> = records.lines().map(|line| serde_json::from_str(line).expect("JSON")).collect();
    assert!(rows.iter().all(|row| row["observation"] != "query"));
    assert_eq!(rows[rows.len() - 2], serde_json::json!({"observations_lost":0}));
    assert_eq!(
        rows.last(),
        Some(&serde_json::json!({"observation":"records_full","capacity":limits::LLM_OBSERVATIONS}))
    );
    fs::remove_dir_all(directory).expect("remove fixture");
}

#[test]
fn deployment_arguments() {
    for (words, environment, directories, forge) in [
        (
            vec!["fake", "forge", "1", "--ready", "ready ", "--seed", "23", "--observations", "seen"],
            vec![("HOME", "/home/test")],
            check_directories(
                "/home/test/.config/temper",
                "/home/test/.local/share/temper",
                "/home/test/.cache/temper",
                Origin::Home,
            ),
            1,
        ),
        (
            vec![
                "fake",
                "forge",
                "--data",
                "data",
                "--config",
                "/config",
                "--cache",
                "cache",
                "65535",
                "--ready",
                "ready ",
                "--seed",
                "23",
                "--observations",
                "seen",
            ],
            vec![],
            check_directories("/config", "/work/data", "/work/cache", Origin::Flag),
            u16::MAX,
        ),
    ] {
        assert_eq!(
            configured(&words, &environment),
            Ok(Command::Fake {
                arguments: Arguments {
                    kind: Kind::Forge,
                    source: Source::Setup { directories, forge },
                    ready: Some(b"ready ".as_slice().into()),
                    seed: 23,
                    observations: Some("seen".into()),
                }
            })
        );
    }
    for number in ["-1", "+1", "65536", "forge", ""] {
        assert_eq!(
            configured(&["fake", "forge", number], &[("HOME", "/home/test")]),
            Err("temper fake forge: N: expected a forge number, a decimal u16\n".into())
        );
    }
    for (words, environment, directories) in [
        (
            vec!["fake", "llm"],
            vec![("HOME", "/home/test")],
            check_directories(
                "/home/test/.config/temper",
                "/home/test/.local/share/temper",
                "/home/test/.cache/temper",
                Origin::Home,
            ),
        ),
        (
            vec!["fake", "llm", "--config", "/config", "--data", "data", "--cache", "cache"],
            vec![],
            check_directories("/config", "/work/data", "/work/cache", Origin::Flag),
        ),
    ] {
        assert_eq!(
            configured(&words, &environment),
            Ok(Command::Fake {
                arguments: Arguments {
                    kind: Kind::Llm,
                    source: Source::Demo { directories },
                    ready: None,
                    seed: 0,
                    observations: None,
                }
            })
        );
    }
}
