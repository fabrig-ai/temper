//! The browser's authorization place (protocol/sign-in.md, 9.3).
use crate::{Domain, Limits, Place, Request};
use alloc::boxed::Box;
use jig_client::{AuthorizeWhy, Authorized, Link};
use skein_lib::{Env, Queue};

/// Only signed-out authorization addresses must fit Google's return path (protocol/sign-in.md, 9.3).
const SIGNED_OUT_ADDRESS_BYTES: usize = 1024;
/// Jig's raw authorization query ceiling, shared with its wire codec.
pub const QUERY_BYTES: u32 = jig_client_protocol_wire::v19::CEILINGS.authorize_query;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flight {
    Idle,
    Pending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Waiting,
    Pending,
    Continuing,
    SignedOut,
    Refused(AuthorizeWhy),
    Busy,
}
#[derive(Debug)]
pub struct Authorization {
    query: Box<[u8]>,
    state: State,
}
impl Authorization {
    #[must_use]
    pub fn query(&self) -> &[u8] {
        &self.query
    }
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }
}
fn changed(root: &mut Domain) {
    root.authorization_revision = root.authorization_revision.checked_add(1).expect("authorization revision fits");
}
pub(crate) fn went(root: &mut Domain, place: &Place) {
    root.authorization = match place {
        Place::Authorize { query } => {
            // Typed inputs obey the same bound as the codec.
            let fits = query.len() <= usize::try_from(QUERY_BYTES).expect("jig query ceiling fits usize");
            let state = if fits { State::Waiting } else { State::Refused(AuthorizeWhy::Malformed) };
            let query = if state == State::Waiting { query.clone() } else { Box::from([]) };
            Some(Authorization { query, state })
        }
        Place::AuthorizeMalformed => {
            Some(Authorization { query: Box::from([]), state: State::Refused(AuthorizeWhy::Malformed) })
        }
        Place::Address(_) | Place::Missing => None,
    };
    changed(root);
}
fn signed_out(query: &[u8]) -> State {
    match query.len().checked_add(b"/oauth/authorize?".len()) {
        Some(bytes) if bytes <= SIGNED_OUT_ADDRESS_BYTES => State::SignedOut,
        Some(_) | None => State::Refused(AuthorizeWhy::Malformed),
    }
}
pub(crate) fn progress(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let Some(page) = &mut root.authorization else { return };
    if page.state != State::Waiting || root.authorizing == Flight::Pending {
        return;
    }
    match root.client.link().state {
        Link::SignedOut => {
            page.state = signed_out(&page.query);
            changed(root);
        }
        Link::Live => {
            page.state = State::Pending;
            root.authorizing = Flight::Pending;
            let query = page.query.clone();
            changed(root);
            crate::route::client(root, env, jig_client::Event::Authorize { query }, out);
        }
        Link::Starting | Link::Offline | Link::OutOfDate => {}
    }
}
pub(crate) fn answered(root: &mut Domain, answer: Authorized, out: &mut Queue<Request>) {
    root.authorizing = Flight::Idle;
    let Some(page) = &mut root.authorization else { return };
    if page.state != State::Pending {
        return;
    }
    page.state = match answer {
        Authorized::Go { location } => {
            out.push(Request::Leave { to: location, replace: true });
            State::Continuing
        }
        Authorized::SignedOut => signed_out(&page.query),
        Authorized::Refused(why) => State::Refused(why),
        Authorized::NotReady | Authorized::Busy | Authorized::Lost => State::Busy,
    };
    changed(root);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_macros, clippy::disallowed_methods, reason = "tests construct boundary input buffers")]
    use super::*;
    use crate::tests::fixtures;
    #[test]
    fn raw_queries_and_jigs_query_bound() {
        let prefix = b"/oauth/authorize?";
        let mut path = prefix.to_vec();
        path.extend_from_slice(b"client_id=f%6Frgejo&state=a+b%20c&state=second");
        assert_eq!(crate::address::parse(&path), Place::Authorize { query: path[prefix.len()..].into() });
        path.resize(prefix.len() + usize::try_from(QUERY_BYTES).unwrap(), b'x');
        assert_eq!(crate::address::parse(&path), Place::Authorize { query: path[prefix.len()..].into() });
        path.push(b'x');
        assert_eq!(crate::address::parse(&path), Place::AuthorizeMalformed);
    }
    #[test]
    fn every_answer_has_its_page_and_only_go_replaces_the_address() {
        for (answer, state) in [
            (
                Authorized::Go { location: b"https://forge.example/callback?code=one".as_slice().into() },
                State::Continuing,
            ),
            (Authorized::SignedOut, State::SignedOut),
            (Authorized::NotReady, State::Busy),
            (Authorized::Busy, State::Busy),
            (Authorized::Lost, State::Busy),
            (Authorized::Refused(AuthorizeWhy::UnknownClient), State::Refused(AuthorizeWhy::UnknownClient)),
            (Authorized::Refused(AuthorizeWhy::UnknownRedirect), State::Refused(AuthorizeWhy::UnknownRedirect)),
            (Authorized::Refused(AuthorizeWhy::Malformed), State::Refused(AuthorizeWhy::Malformed)),
            (Authorized::Refused(AuthorizeWhy::NoProvider), State::Refused(AuthorizeWhy::NoProvider)),
            (Authorized::Refused(AuthorizeWhy::NotAPerson), State::Refused(AuthorizeWhy::NotAPerson)),
        ] {
            let mut root = crate::new(&fixtures::limits(), 11);
            root.authorization =
                Some(Authorization { query: b"client_id=forgejo".as_slice().into(), state: State::Pending });
            root.authorizing = Flight::Pending;
            let mut out = Queue::uncapped();
            answered(&mut root, answer, &mut out);
            assert_eq!(root.authorization().expect("page").state(), state);
            if state == State::Continuing {
                assert_eq!(
                    out.pop(),
                    Some(Request::Leave {
                        to: b"https://forge.example/callback?code=one".as_slice().into(),
                        replace: true
                    })
                );
            }
            assert!(out.is_empty());
            answered(
                &mut root,
                Authorized::Go { location: b"https://forge.example/duplicate".as_slice().into() },
                &mut out,
            );
            assert!(out.is_empty());
        }
    }
    fn signed_in() -> Domain {
        use jig_client_protocol_wire::v19 as w;
        let limits = fixtures::limits();
        let mut root = crate::new(&limits, 11);
        let mut out = Queue::uncapped();
        let env = fixtures::env(&limits);
        crate::route::client(
            &mut root,
            &env,
            jig_client::Event::Start { kept: skein_lib::List::with_capacity(0) },
            &mut out,
        );
        while out.pop().is_some() {}
        crate::route::client(
            &mut root,
            &env,
            jig_client::Event::SignIn { with: jig_client::With::Transport },
            &mut out,
        );
        while out.pop().is_some() {}
        let mut served = skein_lib::List::with_capacity(1);
        served
            .push(
                w::Served::new(
                    &w::CEILINGS,
                    w::ServedParts {
                        owner: w::Owner::Jig,
                        first: jig_client_protocol_wire::VERSION,
                        last: jig_client_protocol_wire::VERSION,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        let hello = w::StreamEvent::Hello(
            w::Hello::new(
                &w::CEILINGS,
                w::HelloParts { stream: [7; 16], served, heartbeat: skein_lib::Duration::from_secs(30) },
            )
            .unwrap(),
        );
        let mut writer = skein_lib::Writer::new(usize::try_from(hello.measure()).unwrap());
        hello.encode(&mut writer).unwrap();
        crate::route::client(
            &mut root,
            &env,
            jig_client::Event::Streamed { message: jig_client::Streamed::Record(writer.finish()) },
            &mut out,
        );
        let mut watch = None;
        while let Some(request) = out.pop() {
            if let Request::Client {
                request: jig_client::Request::Exchange { message: jig_client::Message::Watch(body), .. },
            } = request
            {
                let open = w::Open::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&body)).unwrap();
                let subject =
                    w::Subject::decode(&w::CEILINGS, &mut skein_lib::Reader::new(open.subject().record())).unwrap();
                if subject == w::Subject::Me {
                    watch = Some(open.watch());
                }
            }
        }
        let value = w::DeliveryValue::Me(jig_client_test_records::frame_record());
        let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut writer).unwrap();
        let document = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: jig_client_protocol_wire::VERSION, record: writer.finish() },
        )
        .unwrap();
        let delivery = w::StreamEvent::Delivery(
            w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch: watch.expect("me watch"), document }).unwrap(),
        );
        let mut writer = skein_lib::Writer::new(usize::try_from(delivery.measure()).unwrap());
        delivery.encode(&mut writer).unwrap();
        crate::route::client(
            &mut root,
            &env,
            jig_client::Event::Streamed { message: jig_client::Streamed::Record(writer.finish()) },
            &mut out,
        );
        assert_eq!(root.client.link().state, Link::Live);
        root
    }
    fn query(bytes: usize) -> Box<[u8]> {
        let mut query = b"client_id=f%6Frgejo&state=a+b%20c&state=second&pad=".to_vec();
        query.resize(bytes, b'x');
        query.into()
    }
    #[test]
    fn signed_in_sends_long_queries_byte_for_byte_once() {
        use jig_client_protocol_wire::v19 as w;
        for bytes in [1025 - b"/oauth/authorize?".len(), usize::try_from(QUERY_BYTES).unwrap()] {
            let mut root = signed_in();
            let query = query(bytes);
            went(&mut root, &Place::Authorize { query: query.clone() });
            let limits = fixtures::limits();
            let mut out = Queue::uncapped();
            progress(&mut root, &fixtures::env(&limits), &mut out);
            assert_eq!(root.authorization().unwrap().state(), State::Pending);
            let mut sent = 0_u32;
            while let Some(request) = out.pop() {
                if let Request::Client {
                    request: jig_client::Request::Exchange { message: jig_client::Message::Authorize(body), .. },
                } = request
                {
                    let record = w::Authorize::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&body)).unwrap();
                    assert_eq!(record.query(), query.as_ref());
                    sent += 1;
                }
            }
            assert_eq!(sent, 1);
            progress(&mut root, &fixtures::env(&limits), &mut out);
            assert!(out.is_empty());
        }
    }
    #[test]
    fn signed_out_bounds_apply_both_before_sending_and_to_a_signed_out_answer() {
        for (bytes, expected) in [(1024, State::SignedOut), (1025, State::Refused(AuthorizeWhy::Malformed))] {
            let query = query(bytes - b"/oauth/authorize?".len());
            let mut root = fixtures::on_adoption_page();
            let limits = fixtures::limits();
            let mut out = Queue::uncapped();
            crate::route::client(
                &mut root,
                &fixtures::env(&limits),
                jig_client::Event::Up { up: jig_client::Up::SignedOut },
                &mut out,
            );
            while out.pop().is_some() {}
            went(&mut root, &Place::Authorize { query: query.clone() });
            progress(&mut root, &fixtures::env(&limits), &mut out);
            assert!(out.is_empty());
            assert_eq!(root.authorization().unwrap().state(), expected);
            root.authorization = Some(Authorization { query, state: State::Pending });
            root.authorizing = Flight::Pending;
            answered(&mut root, Authorized::SignedOut, &mut out);
            assert_eq!(root.authorization().unwrap().state(), expected);
            assert!(out.is_empty());
        }
    }
    #[test]
    fn oversized_typed_queries_are_refused_before_sending() {
        for signed_out in [false, true] {
            let limits = fixtures::limits();
            let mut root = fixtures::on_adoption_page();
            if signed_out {
                crate::route::client(
                    &mut root,
                    &fixtures::env(&limits),
                    jig_client::Event::Up { up: jig_client::Up::SignedOut },
                    &mut Queue::uncapped(),
                );
            }
            went(
                &mut root,
                &Place::Authorize { query: alloc::vec![b'x'; usize::try_from(QUERY_BYTES).unwrap() + 1].into() },
            );
            let mut out = Queue::uncapped();
            progress(&mut root, &fixtures::env(&limits), &mut out);
            assert!(out.is_empty());
            assert_eq!(root.authorization().expect("page").state(), State::Refused(AuthorizeWhy::Malformed));
        }
    }
}
