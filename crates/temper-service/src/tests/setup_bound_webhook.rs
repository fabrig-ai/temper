use super::*;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

#[test]
fn reconcile_hands_the_forge_the_bound_webhook_address_after_a_port_zero_bind() {
    for (fake, bind_first, bound, url) in [
        (false, false, SocketAddr::from((Ipv4Addr::LOCALHOST, 41733)), b"http://127.0.0.1:41733/hooks/1".as_slice()),
        (true, true, SocketAddr::from((Ipv4Addr::LOCALHOST, 41733)), b"http://127.0.0.1:41733/hooks/1".as_slice()),
        (false, true, SocketAddr::from((Ipv6Addr::LOCALHOST, 41733)), b"http://[::1]:41733/hooks/1".as_slice()),
        (true, false, SocketAddr::from((Ipv6Addr::LOCALHOST, 41733)), b"http://[::1]:41733/hooks/1".as_slice()),
    ] {
        let mut config = local_config(true, fake);
        let mut requested = bound;
        requested.set_port(0);
        config.webhooks = Some(skein_forge_connection::hooks::Webhooks { listener: skein_io::Addr::Ip(requested) });
        let mut store = local_store(true);
        kept_record(&mut store, 1, Item::WebhookSecret, b"test-hook");
        let mut service = local_service(config, store);
        let engine = service.engine.as_mut().unwrap();
        // Answer the unrelated worker listener so the real engine pass has routing room.
        for _ in 0..engine.workers.io.len() {
            let Some(skein_io::Request::Listen { owner, addr }) = engine.workers.io.pop() else {
                panic!("worker listener");
            };
            assert!(engine.workers.up(
                Time::ZERO,
                skein_io::Event::Listening { owner, listener: Token::new(78), addr },
                &mut engine.events,
            ));
        }
        engine.forge.iterate(Time::ZERO, Wall::EPOCH);
        let Some(skein_io::Request::Listen { owner, addr }) = engine.forge.io.pop() else {
            panic!("webhook listen");
        };
        assert_eq!(addr, skein_io::Addr::Ip(requested));
        if bind_first {
            engine.forge.up(skein_io::Event::Listening {
                owner,
                listener: Token::new(79),
                addr: skein_io::Addr::Ip(bound),
            });
            crate::engine::forge_pass(engine);
            assert!(!engine.setup.waiting_for_webhooks, "bind before records reaches the staged settings");
            assert!(engine.setup.component.is_none(), "the bind still waits for records");
        }
        for _ in 0_u32..8 {
            crate::forge::records::advance(engine);
        }
        assert!(engine.forge_records.done());
        advance(engine);
        if !bind_first {
            assert!(engine.setup.component.is_none(), "loaded records still wait for the webhook bind");
            engine.forge.up(skein_io::Event::Listening {
                owner,
                listener: Token::new(79),
                addr: skein_io::Addr::Ip(bound),
            });
            crate::engine::forge_pass(engine);
        }
        restore_local(&mut service);
        let mut face = Face::new();
        reconcile_to_repository(&mut service, &mut face);
        face.reply(&mut service, br#"{"permission":"write","user":{"id":42,"login":"temper"}}"#);
        local_sent(&mut service, &mut face, b"GET /api/v1/settings/api ");
        face.reply(&mut service, br#"{"max_response_items":50,"default_paging_num":30}"#);
        local_sent(&mut service, &mut face, b"/hooks?page=1&limit=32 ");
        face.reply(&mut service, b"[]");
        local_sent(&mut service, &mut face, b"POST /api/v1/repos/demo/widget/hooks ");
        assert!(contains(&face.wire.received, url), "hook uses the typed bound address");
    }
}
