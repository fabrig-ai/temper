//! Skein's forge peer shares its domain with hosted git (domain/testing.md, 2.4).
use crate::fixtures;
use skein_fake_forge_domain::{self as domain, api};
use skein_fake_peers::forge::{Delivery, Forge, Kept, Observation, Peer};
use skein_lib::{Duration, Env, Secret, Time, Wall};
use std::{cell::RefCell, net::SocketAddr, path::Path, rc::Rc};

pub const ID: u32 = 2;
pub const OTHER: u64 = 2;
pub const HOOK_SECRET: &[u8] = b"real-loop-hook-secret";
#[derive(Clone, Copy)]
pub struct Fixture {
    pub setup: fn() -> api::Setup,
    pub extra_setups: fn() -> Box<[api::Setup]>,
    pub configure: fn(&mut serde_json::Value, &mut serde_json::Value, SocketAddr),
}
#[must_use]
pub fn fixture() -> Fixture {
    Fixture { setup, extra_setups: || Box::new([]), configure: |_, _, _| {} }
}
fn setup() -> api::Setup {
    api::Setup {
        workflows: Box::new([]),
        id: ID,
        name: b"acme/app".as_slice().into(),
        default: b"main".as_slice().into(),
        tree: [(b"conflict.txt".as_slice(), b"base\n".as_slice()), (b"README.md".as_slice(), b"# app\n".as_slice())]
            .map(|(path, content)| api::File { path: path.into(), content: content.into() })
            .into(),
        labels: Box::new([]),
        checks: api::Checks {
            contexts: vec![b"check".as_slice().into()].into(),
            latency_min: Duration::ZERO,
            latency_max: Duration::ZERO,
            silent: 0,
            passes: 1000,
            reruns: 0,
            cue: None,
        },
        protection: None,
        hooked: true,
    }
}
pub fn write(scratch: &Path, forge: SocketAddr, fixture: &Fixture) -> std::io::Result<()> {
    let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(scratch.join("temper.json"))?)?;
    let mut agent: serde_json::Value = serde_json::from_slice(&std::fs::read(scratch.join("agent.json"))?)?;
    config["application"]["webhooks"] = serde_json::json!({"listener":{"socket":"127.0.0.1:0"}});
    config["application"]["forges"][0]["webhook_secret"] = "secrets/forge-1.webhook-secret".into();
    config["application"]["identities"][0]["login"] = "worker".into();
    (fixture.configure)(&mut config, &mut agent, forge);
    fixtures::turn_bounds::apply(&mut config, fixtures::turn_bounds::turn_body(&agent));
    for (name, bytes) in [
        ("forge-1.api-token", temper_fake_git_hosted::fixtures::TOKEN),
        ("forge-1.git-token", temper_fake_git_hosted::fixtures::TOKEN),
        ("forge-1.user", b"1\n"),
        ("forge-1.webhook-secret", HOOK_SECRET),
    ] {
        fixtures::private(&scratch.join("secrets").join(name), bytes)?;
    }
    fixtures::finish_configuration(scratch, &mut config)?;
    fixtures::private(&scratch.join("agent.json"), &serde_json::to_vec(&agent)?)
}
/// The outside forge API at each change of its branches or open pulls.
pub struct Snapshot {
    pub at: Time,
    pub branches: Box<[(Box<[u8]>, u64)]>,
    pub pulls: Box<[u64]>,
}
#[derive(Default)]
pub struct Seen {
    snapshots: Vec<Snapshot>,
    deliveries: Vec<Delivery>,
    pub(crate) responses: Vec<u16>,
    observations: Vec<domain::Observation>,
    merged_green: bool,
    main: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
}
impl Seen {
    #[must_use]
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    #[must_use]
    pub fn deliveries(&self) -> &[Delivery] {
        &self.deliveries
    }
    #[must_use]
    pub fn responses(&self) -> &[u16] {
        &self.responses
    }
    #[must_use]
    pub fn merged_green(&self) -> bool {
        self.merged_green
    }
    #[must_use]
    pub fn main_file(&self, path: &[u8]) -> &[u8] {
        self.main.get(path).expect("main file")
    }
    #[must_use]
    pub fn main(&self) -> &std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
        &self.main
    }
    /// The forge domain's records, in the order the story made them.
    #[must_use]
    pub fn observations(&self) -> &[domain::Observation] {
        &self.observations
    }
    /// Freeze main after the person's push, including a story with no merge.
    pub(crate) fn pushed(&mut self, handle: &Rc<RefCell<Forge>>) {
        let forge = handle.borrow();
        let api::Answer::Commit(main) = forge
            .domain
            .inspect(&forge.config, b"acme/app", &api::Read::Branch { branch: b"main".as_slice().into() })
            .expect("pushed main head")
        else {
            panic!("main commit")
        };
        self.main = forge
            .domain
            .object(main)
            .expect("pushed main object")
            .tree
            .iter()
            .map(|(path, bytes)| (path.to_vec(), bytes.to_vec()))
            .collect();
    }
    pub(crate) fn collect(&mut self, now: Time, peer: &Peer, handle: &Rc<RefCell<Forge>>, repository: &[u8]) {
        {
            let forge = handle.borrow();
            let branches: Box<[_]> =
                forge.domain.branches(repository).iter().map(|(name, head)| (name.clone(), *head)).collect();
            let api::Answer::Pulls(pulls) = forge
                .domain
                .inspect(&forge.config, repository, &api::Read::OpenPulls { page: 1, limit: 32 })
                .expect("outside open-pulls read")
            else {
                panic!("open pulls")
            };
            let pulls: Box<[_]> = pulls.iter().map(|pull| pull.number).collect();
            if self.snapshots.last().is_none_or(|last| last.branches != branches || last.pulls != pulls) {
                assert!(self.snapshots.len() < STORY_OBSERVATIONS as usize, "bounded forge snapshots");
                self.snapshots.push(Snapshot { at: now, branches, pulls });
            }
        }
        self.deliveries = peer
            .observations()
            .iter()
            .filter_map(|record| {
                let Observation::Delivery { outcome, .. } = record else { return None };
                Some(match outcome {
                    Delivery::Written => Delivery::Written,
                    Delivery::NoAddress => Delivery::NoAddress,
                    Delivery::NoRoom => Delivery::NoRoom,
                    Delivery::Refused => Delivery::Refused,
                    Delivery::Reset => Delivery::Reset,
                })
            })
            .collect();
        loop {
            let Some(observed) = handle.borrow_mut().domain.pop_observation() else { break };
            if let domain::Observation::Merged { repository, head, .. } = &observed {
                let forge = handle.borrow();
                let api::Answer::Checks(checks) = forge
                    .domain
                    .inspect(&forge.config, repository, &api::Read::Checks { commit: *head })
                    .expect("merged checks")
                else {
                    panic!("checks answer")
                };
                self.merged_green =
                    !checks.is_empty() && checks.iter().all(|check| check.status.state == api::Check::Passed);
                let api::Answer::Commit(main) = forge
                    .domain
                    .inspect(&forge.config, repository, &api::Read::Branch { branch: b"main".as_slice().into() })
                    .expect("main head")
                else {
                    panic!("main commit")
                };
                self.main = forge
                    .domain
                    .object(main)
                    .expect("main object")
                    .tree
                    .iter()
                    .map(|(path, bytes)| (path.to_vec(), bytes.to_vec()))
                    .collect();
            }
            assert!(self.observations.len() < STORY_OBSERVATIONS as usize, "bounded domain observations");
            self.observations.push(observed);
        }
    }
}
pub(crate) struct State {
    pub(crate) handle: Rc<RefCell<Forge>>,
    pub(crate) moved: bool,
    pub(crate) repository: Box<[u8]>,
    pub(crate) hooked: bool,
}
impl State {
    pub(crate) fn new(fixture: Fixture) -> (Self, Peer) {
        let setup = (fixture.setup)();
        assert_eq!(setup.id, ID, "the primary repository keeps the hosted fixture identity");
        let setups = std::iter::once(setup).chain((fixture.extra_setups)()).collect::<Vec<_>>();
        let setup = &setups[0];
        let repository = setup.name.clone();
        let mut table =
            temper_fake_git_hosted::fixtures::table(&setup.name, setup.protection.as_ref().map(|p| p.branch.as_ref()));
        // fixtures::table reserves one repository; rebuild before sharing with
        // typed git and HTTP so every setup has the same wire identity.
        table.repositories = skein_lib::List::with_capacity(u32::try_from(setups.len()).expect("repository count"));
        for setup in &setups {
            table
                .repositories
                .push(skein_fake_forge_protocol::Repository {
                    id: setup.id,
                    full: setup.name.clone(),
                    protected: setup.protection.as_ref().map(|p| p.branch.clone()),
                })
                .expect("one table entry per setup");
        }
        let mut config = temper_fake_git_hosted::fixtures::CONFIG;
        // The HTTP peer admits four simultaneous calls, unlike the git-only fixture.
        config.limits.calls = protocol_limits().calls;
        // A change opens its pull under the full specification, before coding's
        // result supplies its final title; the case's brief exceeds 16 bytes.
        config.limits.title_bytes = 512;
        // The case's design pull body and title-plus-body commit message exceed
        // the git-only fixture's 32-byte body and 50-byte message limits.
        config.limits.body_bytes = 2048;
        // A closed pull keeps its item: the case's 5.4 keeps x3's closed pull while a1 opens another.
        config.limits.items = 2;
        // A tracked goal projects its subtree's milestones as separate comments.
        config.limits.comments = 8;
        // A waiting pull's head, a red pushed tip and its repair's head hold CI statuses at once (6.4).
        config.limits.statuses = 3;
        // Forgejo timestamps are wall times; this peer runs on the real monotonic clock.
        let now = skein_shell::Clock::new().now();
        config.skew = domain::Skew::Ahead(Duration::from_nanos(
            now.wall.as_nanos().checked_sub(now.now.as_nanos()).expect("wall time follows monotonic origin"),
        ));
        let mut domain = domain::Domain::new(&config, fixtures::SEED);
        for setup in setups {
            let name = setup.name.clone();
            domain::repository(&mut domain, &config, setup);
            domain::grant(&mut domain, &name, 1, api::Permission::Write);
        }
        let handle = Rc::new(RefCell::new(Forge::new(domain, config)));
        crate::git::share(Some(temper_fake_git_hosted::Typed::new(handle.clone(), &table)));
        let peer = Peer::new(
            ([127, 0, 0, 1], 0).into(),
            peer_limits(),
            table,
            protocol_limits(),
            handle.clone(),
            fixtures::SEED,
        )
        .expect("configured forge peer")
        // The harness reads the deliveries alone, and whether any record exists:
        // the first is a connection's opening or a delivery, so heads and calls,
        // a pair for each of the clients' polls, are never kept.
        .keeping(Kept { connections: true, deliveries: true, ..Kept::NONE });
        (Self { handle, moved: false, hooked: false, repository }, peer)
    }
    pub(crate) fn hand_hooks(&mut self, address: SocketAddr) {
        self.handle.borrow_mut().hooks = Some(skein_fake_forge_protocol::delivery::Endpoint {
            forge: 1,
            host: address.to_string().into_bytes().into(),
            secret: Secret::new(HOOK_SECRET.into()),
        });
        self.hooked = true;
    }
    pub(crate) fn advance(&mut self, now: Time, wall: Wall) {
        self.push(now, wall, b"conflict.txt", b"other\n");
    }
    pub(crate) fn push(&mut self, now: Time, wall: Wall, path: &[u8], words: &[u8]) {
        let mut forge = self.handle.borrow_mut();
        let env = Env { now, wall, limits: forge.config };
        domain::advance(&mut forge.domain, &env, b"acme/app", b"main", path, words, OTHER)
            .expect("another party's push");
        self.moved = true;
    }
}

// A whole story's frozen peer records, its connections and deliveries alone
// (`State::new`): room for the largest stories, 5.4's and the green light's
// (~160 records idle, up to ~240 under eight busy loops, which stretch a
// story's wall and its connections' lives), eight times over, at the skein
// peer fixture's 2 KiB per record.
const STORY_OBSERVATIONS: u32 = 2_048;
const STORY_OBSERVATION_BYTES: u32 = 4_194_304;
#[must_use]
pub fn peer_limits() -> skein_fake_peers::Limits {
    skein_fake_peers::Limits {
        io: skein_io::Limits {
            sockets: 16,
            names: 0,
            refusals: 8,
            intake: 65_536,
            receive: 1024,
            output: 65_536,
            sends: 8,
            accepts: 4,
            backlog: 8,
            close_timeout: Duration::from_secs(1),
            retry: Duration::from_millis(1),
        },
        connections: protocol_limits().connections,
        queue: 64,
        plaintext: 32_768,
        ciphertext: 32_768,
        observations: STORY_OBSERVATIONS,
        observation_bytes: STORY_OBSERVATION_BYTES,
    }
}
/// The forge client's simultaneous calls on the Forgejo peer.
const CALLS: u32 = 4;
/// The Forgejo protocol's limits. Bodies, documents and strings hold the
/// git route's whole trees of the domain the worlds share (256 files of
/// 64 KiB, about 22.5 MB as base64), as the peer requires (skein's
/// `git::fits`, and a body fits one string); a buffer's pages are resident
/// only as a call fills them.
#[must_use]
pub fn protocol_limits() -> skein_fake_forge_protocol::Limits {
    skein_fake_forge_protocol::Limits {
        calls: CALLS,
        // Room past the forge client's calls for the peer's other
        // connections: with only the calls' four, a call found the slots
        // taken now and then (4 to 6 refusals a fuzzy run) and waited out
        // its 1 s read retry.
        connections: CALLS + 3,
        raw_bytes: 4096,
        http: skein_http::server::Limits {
            head: 4096,
            headers: 16,
            body: 25_165_824,
            read: 1024,
            response: 4096,
            send: 1024,
        },
        documents: skein_forge::forgejo::limits::Limits {
            json: skein_json::tokenizer::Limits {
                chunk: 1024,
                string: 25_165_824,
                number: 32,
                depth: 16,
                length: 25_165_824,
            },
            writer: skein_json::writer::Limits { depth: 16, length: 25_165_824 },
            string_bytes: 512,
            body_bytes: 25_165_824,
            inner: 256,
            target_bytes: 4096,
        },
    }
}
