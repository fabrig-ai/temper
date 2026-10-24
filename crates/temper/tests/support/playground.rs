//! One fresh native client after the playground's durable retirement.
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_ui::Role;
use skein_world::{Referee, real};
use std::path::Path;
use temper_client_domain::{Address, Step};
use temper_client_shell_terminal::client::{NativeClient, Settings, Start};

#[derive(Debug)]
enum Page {
    Waiting,
    Welcome,
    Model,
    Done,
}
struct Judge {
    page: Page,
    until: Time,
}
fn named(client: &NativeClient, role: Role, name: &[u8]) -> bool {
    client.tree().nodes().iter().any(|node| node.role == role && node.name.as_deref() == Some(name))
}
impl Referee<NativeClient> for Judge {
    fn act(&mut self, now: Time, clients: &mut [NativeClient]) {
        let client = &mut clients[0];
        match self.page {
            Page::Waiting if client.started() && named(client, Role::Status, b"Owner") => {
                client.go(Address::Welcome { step: Step::Welcome });
                self.page = Page::Welcome;
            }
            // This link waits for Me, Accounts, Forges and the demo read, so
            // absence below cannot pass before the retirement answer arrives.
            Page::Welcome if named(client, Role::Link, b"Set up temper") => {
                assert!(!named(client, Role::Button, b"Try the playground"));
                client.go(Address::Welcome { step: Step::Model { way: None } });
                self.page = Page::Model;
            }
            Page::Model if named(client, Role::Button, b"Continue with ChatGPT") => {
                assert!(!named(client, Role::Link, b"Use a demo API key"));
                assert!(!named(client, Role::Button, b"Try the playground"));
                for node in client.tree().nodes() {
                    if let Some(name) = &node.name {
                        assert!(!name.windows(4).any(|part| part == b"demo"), "{name:?}");
                    }
                }
                client.close();
                self.page = Page::Done;
            }
            Page::Waiting | Page::Welcome | Page::Model | Page::Done => {}
        }
        assert!(
            matches!(self.page, Page::Done) || now < self.until,
            "retired reload did not reach Model: {:?}; {:?}",
            self.page,
            client
                .tree()
                .nodes()
                .iter()
                .filter_map(|node| node.name.as_deref())
                .map(String::from_utf8_lossy)
                .collect::<Vec<_>>()
        );
    }
    fn observe(&mut self, _now: Time, _clients: &[NativeClient]) {}
    fn next_deadline(&self) -> Option<Time> {
        (!matches!(self.page, Page::Done)).then_some(self.until)
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        matches!(self.page, Page::Done)
    }
}

pub fn retired_reload(config: &Path, client: &Path) {
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(config.join("temper.json")).expect("configuration")).expect("JSON");
    let engine = document
        .pointer("/application/clients/listener/socket")
        .and_then(serde_json::Value::as_str)
        .expect("clients listener")
        .parse()
        .expect("socket");
    let root = skein_shell::open_root(client).expect("signed-in client's parent root");
    let clock = Clock::new();
    let mut world =
        real::World::new(Judge { page: Page::Waiting, until: clock.now().now.saturating_add(Duration::from_secs(2)) });
    world.spawn_with_fds(vec![root], || {
        NativeClient::new(
            Settings {
                engine: skein_io::Addr::Ip(engine),
                directory: b"client".as_slice().into(),
                limits: temper_client_shell_terminal::limits::native(),
            },
            root,
            skein_shell::effective_user(),
            93,
            Start::Token,
        )
        .expect("reloaded native client")
    });
    let ended = world.run(&clock, Duration::from_secs(3));
    assert!(ended.procs.iter().all(Host::is_empty), "reloaded native client drained");
}
