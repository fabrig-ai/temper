//! A worker notice changes the real account documents and completes the tools' wait.
use jig_client_world::harness::Until;
use jig_domain_host::LinkUp;
use skein_lib::{Duration, Token};
use temper_client_world::fixtures;
use temper_engine_domain_world::protocol;
// The shared client fixture hosts its single worker on channel 7.
const WORKER: Token = Token::new(7);
// walking::config's configured source 1 is granted to that worker's scripted model.
// The tools unit fixtures separately distinguish billing account 43 from source 71.
const SOURCE: u32 = 1;

#[test]
fn a_changing_accounts_watch_completes_the_agents_wait() {
    let mut world = fixtures::accounts_world(fixtures::SEED);
    world.front().engine.hello(WORKER);
    let agent = fixtures::agent(&mut world);
    let chat = fixtures::give(
        &mut world,
        agent,
        &[b"chat", b"start", b"--project", b"1", b"--key", b"account-read", b"Hello"],
    );
    let task = fixtures::done_task(&fixtures::answered(&mut world, agent, chat));
    let read = fixtures::give(&mut world, agent, &[b"account", b"list", b"--json"]);
    let before = fixtures::answered(&mut world, agent, read);
    assert_eq!(before.status, 0, "{before:?}");
    let text = std::str::from_utf8(&before.stdout).expect("account JSON UTF-8");
    let digest = text.split("\"digest\":\"").nth(1).expect("read digest").split('"').next().expect("digest end");
    let waiting = fixtures::give(&mut world, agent, &[b"account", b"list", b"--since", digest.as_bytes(), b"--json"]);
    world.run(Until::Steps(2));
    assert!(world.root(agent).agent().printed(waiting).is_none(), "unchanged documents keep waiting");
    let attempt = world.front().engine.task(task).expect("running chat").attempt;
    world.front().engine.send(protocol::up(
        WORKER,
        LinkUp::Limited {
            run: Token::new(task),
            attempt: Token::new(attempt),
            account: SOURCE,
            reset: Some(Duration::from_secs(10)),
        },
        temper_engine_domain::Parts::none(),
    ));
    let changed = fixtures::answered(&mut world, agent, waiting);
    assert_eq!(changed.status, 0, "{changed:?}");
    let after = std::str::from_utf8(&changed.stdout).expect("wait JSON UTF-8");
    assert!(!after.contains(digest), "the joined account digest changed");
    assert!(after.contains("\"kind\":\"later\""), "the deployment account shows the worker's limit");
    assert!(after.ends_with(",\"met\":true}"));
}
