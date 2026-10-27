pub mod support;
use skein_lib::Duration;
use skein_shell::Clock;
use support::{ADMIN, Peer};
use temper_forge_setup_shell::reconcile::{bound, reconcile};

#[test]
fn a_reserved_peer_holds_its_address_until_it_serves() {
    let peer = Peer::reserve();
    assert_eq!(
        std::net::TcpListener::bind(peer.address()).expect_err("reserved peer address stays held").kind(),
        std::io::ErrorKind::AddrInUse
    );
    let settings = support::settings(peer.address());
    std::thread::scope(|scope| {
        let server = scope.spawn(|| peer.serve_after(0, &[(401, b"deleted")]));
        let refused = reconcile(settings, &Clock::new()).expect_err("reserved peer answers");
        assert_eq!(refused.line(), "temper forge reconcile: forge 1: admin temper-admin: HTTP 401\n");
        let heard = server.join().expect("peer thread");
        assert_eq!(heard.requests.len(), 1);
        assert_eq!(heard.requests[0].line, "GET /api/v1/user HTTP/1.1");
    });
}

fn done(settings: temper_forge_setup_shell::reconcile::Settings, peer: Peer) {
    const BOT: &[u8] = br#"{"id":42,"login":"temper-bot","is_admin":false,"full_name":"temper:bot"}"#;
    for file in ["api_token", "git_token"] {
        temper_forge_setup_shell::files::write_private(&settings.directory.join(file), b"kept-token")
            .expect("private token");
    }
    std::thread::scope(|scope| {
        let server = scope.spawn(|| peer.serve_after(0, &[(200, ADMIN), (200, BOT), (200, BOT), (200, BOT)]));
        reconcile(settings, &Clock::new()).expect("Done");
        let heard = server.join().expect("peer thread");
        assert_eq!(heard.requests.len(), 4);
        assert!(heard.requests.iter().all(|request| !request.line.contains("/orgs")));
    });
}

#[test]
fn done_with_a_listener_draws_a_private_hex_secret() {
    use std::os::unix::fs::PermissionsExt;
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.given.listener = Some(peer.address());
    let path = settings.directory.join("webhook_secret");
    done(settings, peer);
    let bytes = std::fs::read(&path).expect("secret kept");
    assert_eq!(bytes.len(), 64);
    assert!(bytes.iter().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)));
    assert_eq!(std::fs::metadata(path).expect("private file").permissions().mode() & 0o777, 0o600);
}

#[test]
fn done_keeps_the_secret_byte_for_byte_without_rewriting() {
    use std::os::unix::fs::MetadataExt;
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.given.listener = Some(peer.address());
    let path = settings.directory.join("webhook_secret");
    let kept = [b'a'; 64];
    temper_forge_setup_shell::files::write_private(&path, &kept).expect("kept secret");
    let inode = std::fs::metadata(&path).expect("kept file").ino();
    done(settings, peer);
    assert!(std::fs::read(&path).expect("secret stays") == kept);
    assert_eq!(std::fs::metadata(path).expect("same file").ino(), inode);
}

#[test]
fn owned_reconcile_makes_no_org_or_secret_without_a_listener() {
    let peer = Peer::reserve();
    let settings = support::settings(peer.address());
    let path = settings.directory.join("webhook_secret");
    done(settings, peer);
    assert!(!path.exists());
}
#[test]
fn a_first_request_is_made_again_until_the_api_answers() {
    let peer = Peer::reserve();
    let settings = support::settings(peer.address());
    let heard = std::thread::scope(|scope| {
        let server = scope.spawn(|| peer.serve_after(120, &[(403, br#"{"message":"no"}"#)]));
        let refused = reconcile(settings, &Clock::new()).expect_err("403 stops run");
        assert_eq!(refused.line(), "temper forge reconcile: forge 1: admin temper-admin: HTTP 403\n");
        server.join().expect("peer thread")
    });
    assert_eq!(heard.requests.len(), 1);
    assert_eq!(heard.requests[0].line, "GET /api/v1/user HTTP/1.1");
}
#[test]
fn the_admins_token_is_sent_as_a_token_header() {
    let heard = support::refused(403, b"", "temper forge reconcile: forge 1: admin temper-admin: HTTP 403\n");
    assert!(heard.requests[0].headers.contains(&("authorization".into(), "token admin-token-value".into())));
}
#[test]
fn a_run_ends_at_its_deadline_when_nothing_answers() {
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    drop(peer); // This story has no listener; the reserved peers above serve their held sockets.
    settings.given.limits.deadline = Duration::from_millis(300); // A short wait keeps the host test focused.
    let refused = reconcile(settings, &Clock::new()).expect_err("no peer");
    assert_eq!(refused.line(), "temper forge reconcile: forge 1: Forgejo did not answer within 300 ms\n");
}
#[test]
fn a_forge_that_goes_silent_after_its_answer_ends_the_run_at_the_deadline() {
    use skein_lib::Time;
    use skein_shell::{Config, Host, Kernel, Wait};
    use std::io::{Read, Write};
    use temper_forge_setup_shell::reconcile::Running;

    // Bind before starting the host. This peer stays until the host closes the
    // silent request, rather than racing its close/reconnect against an accept timeout.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("peer binds");
    let mut settings = support::settings(listener.local_addr().expect("address"));
    let deadline = Duration::from_millis(200);
    settings.given.limits.deadline = deadline;
    let began = Time::from_nanos(0);
    let end = began.saturating_add(deadline);
    let wall = Clock::new().now().wall;
    let mut host = Running::new(settings, began).expect("reconcile host");
    let mut kernel = Kernel::open(Config { operations: host.operations() }).expect("real kernel");
    let (sent, heard) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let (answer, allowed) = std::sync::mpsc::channel();
        let server = scope.spawn(move || {
            fn request(listener: &std::net::TcpListener, target: &str) -> std::net::TcpStream {
                use std::io::BufRead;
                let (mut socket, _) = listener.accept().expect("request connects");
                let mut head = std::io::BufReader::new(&mut socket);
                let mut line = String::new();
                head.read_line(&mut line).expect("request line");
                assert_eq!(line, format!("GET {target} HTTP/1.1\r\n"));
                loop {
                    line.clear();
                    assert_ne!(head.read_line(&mut line).expect("request header"), 0, "complete request head");
                    if line == "\r\n" {
                        break;
                    }
                }
                socket
            }
            let mut admin = request(&listener, "/api/v1/user");
            sent.send(None).expect("admin request heard");
            allowed.recv().expect("admin answer allowed");
            let mut response =
                format!("HTTP/1.1 200 Answer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", ADMIN.len())
                    .into_bytes();
            response.extend_from_slice(ADMIN);
            admin.write_all(&response).expect("admin answer");
            drop(admin);
            let mut bot = request(&listener, "/api/v1/users/temper-bot");
            sent.send(Some(())).expect("bot request heard");
            assert_eq!(bot.read(&mut [0]).expect("silent connection closes"), 0);
        });
        // Spend half the run before letting the admin answer, then freeze time
        // until the bot request arrives. A per-request deadline would end too late.
        let mut now = began;
        loop {
            kernel.reap(host.completions());
            host.iterate(now, wall);
            kernel.submit(host.submissions(), Wait::No);
            assert!(host.outcome().is_none(), "the bot request must stay pending");
            match heard.try_recv() {
                Ok(None) => {
                    now = began.saturating_add(Duration::from_nanos(deadline.as_nanos() / 2));
                    answer.send(()).expect("allow admin answer");
                }
                Ok(Some(())) => break,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("peer stopped before bot request"),
            }
        }
        assert_eq!(host.next_deadline(), Some(end), "the second request shares the run's deadline");
        let before = Time::from_nanos(end.as_nanos() - 1);
        kernel.reap(host.completions());
        host.iterate(before, wall);
        kernel.submit(host.submissions(), Wait::No);
        assert!(host.outcome().is_none(), "one nanosecond before the whole-run deadline");
        while !host.is_empty() {
            kernel.reap(host.completions());
            host.iterate(end, wall);
            kernel.submit(host.submissions(), Wait::No);
        }
        let refused = host.outcome().expect("terminal reconcile").as_ref().expect_err("silent bot request");
        assert_eq!(refused.line(), "temper forge reconcile: forge 1: bot temper-bot: unavailable\n");
        server.join().expect("peer thread");
    });
}
#[test]
fn a_later_request_that_fails_is_not_made_again() {
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.given.listener = Some(peer.address());
    let directory = settings.directory.clone();
    let heard = std::thread::scope(|scope| {
        let server = scope.spawn(|| peer.serve_after(0, &[(200, ADMIN), (503, b"")]));
        let refused = reconcile(settings, &Clock::new()).expect_err("later 503");
        assert_eq!(refused.line(), "temper forge reconcile: forge 1: bot temper-bot: HTTP 503\n");
        server.join().expect("peer thread")
    });
    assert_eq!(heard.requests.len(), 2);
    assert_eq!(heard.requests[1].line, "GET /api/v1/users/temper-bot HTTP/1.1");
    assert!(!directory.join("state").exists());
    assert!(!directory.join("webhook_secret").exists());
}
#[test]
fn a_forbidden_request_names_its_step_and_holds_no_secret() {
    let heard =
        support::refused(403, b"admin-token-value", "temper forge reconcile: forge 1: admin temper-admin: HTTP 403\n");
    assert_eq!(heard.requests.len(), 1);
}
#[test]
fn a_deleted_admin_is_refused_naming_its_step() {
    let heard = support::refused(401, b"deleted", "temper forge reconcile: forge 1: admin temper-admin: HTTP 401\n");
    assert_eq!(heard.requests.len(), 1);
}
#[test]
fn an_admin_token_readable_by_others_is_refused_naming_it() {
    use std::os::unix::fs::PermissionsExt;
    let peer = Peer::reserve();
    let settings = support::settings(peer.address());
    std::fs::set_permissions(&settings.admin_token, std::fs::Permissions::from_mode(0o644)).expect("unsafe token");
    let refused = reconcile(settings, &Clock::new()).expect_err("unsafe token");
    assert_eq!(
        refused.line(),
        "temper forge reconcile: forge 1: admin_token: readable by others or reached through a link\n"
    );
}
#[test]
fn each_file_is_read_within_its_own_bound() {
    use temper_forge_setup::place::File;
    let peer = Peer::reserve();
    let settings = support::settings(peer.address());
    for (file, expected) in [
        (File::AdminToken, settings.token_bytes),
        (File::ApiToken, settings.token_bytes),
        (File::GitToken, settings.token_bytes),
        (File::User, 21),
        (File::WebhookSecret, settings.hook_secret_bytes),
    ] {
        assert_eq!(bound(&settings, file), expected);
    }
}
#[test]
fn io_short_of_the_http_intake_is_refused_before_any_request() {
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.io.intake = skein_http::client::largest_read(&settings.http) - 1;
    let refused = reconcile(settings, &Clock::new()).expect_err("short intake");
    assert_eq!(refused.line(), "temper forge reconcile: forge 1: io.intake: below the http client's largest read\n");
}
#[test]
fn io_short_of_the_http_output_is_refused_before_any_request() {
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.io.output = skein_http::client::largest_room(&settings.http) - 1;
    let refused = reconcile(settings, &Clock::new()).expect_err("short output");
    assert_eq!(refused.line(), "temper forge reconcile: forge 1: io.output: below the http client's largest read\n");
}

#[test]
fn invalid_given_values_are_refused_before_io() {
    let peer = Peer::reserve();
    let mut settings = support::settings(peer.address());
    settings.given.bot_login = Box::new([]);
    settings.io.output = 0;
    let refused = reconcile(settings, &Clock::new()).expect_err("invalid given");
    assert_eq!(refused.line(), "temper forge reconcile: forge 1: application.setup[0].bot.login: invalid form\n");
    let temper_forge_setup_shell::reconcile::Refused::Setup { forge, .. } = refused else {
        panic!("setup refused before io");
    };
    assert_eq!(forge, 1);
}

#[test]
fn github_refusals_name_the_stage_and_reason() {
    use skein_forge_setup::reconcile::{Failure, Stage, Why};
    use temper_forge_setup_shell::reconcile::Refused;
    for (step, why, words) in [
        (Stage::Installation, Why::KeyForm, "installation app: key unusable"),
        (Stage::App, Why::Unknown, "app app: unknown, used or expired"),
        (Stage::State, Why::OtherApp, "state app: another app"),
    ] {
        let refused = Refused::Stopped(Failure { forge: 1, step, subject: b"app".as_slice().into(), why });
        assert_eq!(refused.line(), format!("temper forge reconcile: forge 1: {words}\n"));
    }
}
