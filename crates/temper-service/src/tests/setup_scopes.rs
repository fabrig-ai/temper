use super::*;
use crate::tests::fixtures;
use skein_fake_forge_protocol as fake;
use skein_lib::List;
use temper_forge_setup::{place::Place, setup::Mode, spec};
use tests::{Face, start, terminal};

// The existing setup stream face also speaks to the scope-enforcing peer.
impl Face {
    fn forge_reply(&mut self, service: &mut crate::Service, forge: &mut fake::Service, env: &Env<fake::Limits>) {
        self.sent(service);
        let mut server = fake::Server::new(Token::new(1), &env.limits).unwrap();
        let mut wire = skein_world::stream::Wire::new(17);
        wire.write(&self.wire.received);
        let mut above = Queue::with_capacity(fake::MAX_UP);
        let mut below = Queue::with_capacity(fake::MAX_DOWN);
        fake::start(&mut server, forge, env, &mut above, &mut below);
        let mut closed = false;
        for _ in 0_u32..4096 {
            fake::resume(&mut server, forge, env, &mut above, &mut below);
            for _ in 0..below.len() {
                wire.take(below.pop().unwrap());
            }
            for _ in 0..above.len() {
                match above.pop().unwrap() {
                    fake::Event::Domain(_) => panic!("setup requests are served without domain calls"),
                    fake::Event::Close => fake::closed(&mut server, forge, env, &mut above, &mut below),
                    fake::Event::Closed => closed = true,
                }
            }
            if let Some(up) = wire.answer() {
                fake::up(&mut server, forge, env, up, &mut above, &mut below);
            }
        }
        assert!(closed, "fake completed the request and closed its connection");
        assert!(!wire.received.is_empty(), "fake answered");
        self.wire.write(&wire.received);
        for _ in 0_u32..1024 {
            self.turn(service);
        }
    }
}

fn start_adoption(
    service: &mut crate::Service,
    repository: core::num::NonZeroU64,
    token: Secret,
    access: skein_forge_connection::Access,
) {
    // Match settings::prepare's files adoption, retaining access until the last Adopt completes.
    // The sequential fake-start helpers use repositories 1 and 2; this scope fixture targets 7.
    service
        .engine
        .as_mut()
        .expect("engine")
        .setup
        .lifecycle
        .adoptions
        .insert(
            1,
            lifecycle::Adoption {
                next: repository,
                repositories: repository.get(),
                files: Some(lifecycle::AdoptionFiles { admin_token: token, access }),
            },
        )
        .unwrap();
    start(service, 1, host::Plan::Adopt { forge: 1, repository });
}

#[test]
fn preparations_four_scopes_grant_adopt_and_without_repository_it_is_refused_at_repository() {
    for without_repository in [false, true] {
        let mut config = fixtures::engine();
        config.setup = Box::new([fixtures::owned_setup(1)]);
        let Mode::Owned(owned) = &config.setup[0].entry.mode else { panic!("owned fixture") };
        let place = Place { directory: config.setup[0].entry.directory.clone(), user: b"fixture".as_slice().into() };
        let spec = spec::admin_token(owned, &place, b"temper-admin-0123456789abcdef");
        // Preparation issues four scopes; the refusal case removes only repository access.
        let mut scopes = List::with_capacity(4);
        for args in spec.arguments.windows(2) {
            if args[0].as_ref() != b"--scopes" {
                continue;
            }
            let mut from = 0;
            for to in 0..=args[1].len() {
                if to < args[1].len() && args[1][to] != b',' {
                    continue;
                }
                let scope = &args[1][from..to];
                if !without_repository || scope != b"write:repository" {
                    scopes.push(scope).unwrap();
                }
                from = to.saturating_add(1);
            }
        }
        fixtures::fill_files(&mut config);
        let mut endpoints = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
        let access = endpoints.forges.get_mut(0).unwrap().access.take().expect("file access");
        let mut service = crate::Service::engine(config, &fixtures::seeds(), None, fixtures::forge_runtime()).unwrap();
        service.engine.as_mut().expect("engine").store = fixtures::records_store();
        for _ in 0_u32..8 {
            crate::forge::records::advance(service.engine.as_mut().expect("engine"));
        }
        advance(service.engine.as_mut().expect("engine"));
        let limits = service.engine.as_ref().expect("engine").setup.env.limits;
        let env = Env {
            now: Time::ZERO,
            wall: Wall::EPOCH,
            limits: fake::Limits {
                calls: 4,
                connections: 4,
                raw_bytes: 4096,
                documents: host::documents(&limits),
                http: skein_http::server::Limits {
                    head: 4096,
                    headers: 16,
                    body: 65536,
                    read: 65536,
                    response: 4096,
                    send: 65536,
                },
            },
        };
        let mut repositories = List::with_capacity(1);
        repositories.push(fake::Repository { id: 7, full: b"acme/widget".as_slice().into(), protected: None }).unwrap();
        let mut forge = fake::Service::new(
            fake::Config {
                hooks: None,
                base: b"http://127.0.0.1:3000".as_slice().into(),
                tokens: List::with_capacity(0), // Only the issued scoped token authenticates.
                users: List::with_capacity(0),
                repositories,
                max_response_items: 2,
                default_paging_num: 30,
                faults: fake::Faults { reset: 0, drop: 0, server_error: 0 },
            },
            &env.limits,
            19,
        )
        .unwrap();
        let admin = forge.setup_mut().account(b"temper-admin", true, b"");
        forge.setup_mut().account(b"temper", false, b"");
        let token = Secret::new(b"0123456789abcdef0123456789abcdef01234567".as_slice().into());
        forge.setup_mut().token(admin, b"prepared", scopes.as_slice(), token.clone());
        let repository = core::num::NonZeroU64::new(7).unwrap();
        start_adoption(&mut service, repository, token, access);
        let mut face = Face::new();
        for _ in 0_u32..if without_repository { 2 } else { 4 } {
            face.forge_reply(&mut service, &mut forge, &env);
            if service.engine.as_ref().expect("engine").setup.runs.is_empty() {
                break;
            }
        }
        let end = terminal(&mut service);
        if without_repository {
            let root::SetupEnd::Stopped { forge: number, reason } = end else { panic!("repository scope refused") };
            assert_eq!(number, 1);
            assert_eq!(reason.as_ref(), b"setup Repository 7: Refused(403)");
            assert_eq!(forge.setup().collaborator(7, b"temper"), None);
        } else {
            let root::SetupEnd::Done { forge: 1, outcome: root::SetupOutcome::Done } = end else {
                panic!("adoption: {end:?}");
            };
            assert_eq!(forge.setup().collaborator(7, b"temper"), Some(skein_forge::forgejo::request::Grant::Write));
        }
    }
}
