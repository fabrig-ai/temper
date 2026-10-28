//! W1's inner order (programming-model.md, 7.2): route a capability's
//! callback before the hub's next request to that capability. The whole
//! worker world separately checks the outer failed-prepare cleanup order.

use super::{fixtures, harness::Harness};
use crate::{Event, Request, host, route, workspace};
use alloc::boxed::Box;
use skein_lib::List;

// The first operation and the cancellation issued by the later Abort.
const CROSSINGS: u32 = 2;

/// Leave the checkout's Held and first IO queued, then queue the hub's
/// Abort of that same prepare. Neither child's requests have been routed.
fn held_beside_abort() -> Harness {
    let mut h = Harness::new();
    route::event(
        &mut h.domain,
        &h.env,
        Event::Assign {
            assignment: fixtures::assignment(true, Box::new([])),
            items: Ok(fixtures::items(&[b"app"])),
            save: None,
        },
        &mut h.out,
    );
    let host::Request::Prepare { owner, workspace } = h.domain.host_out.pop().expect("assignment prepares") else {
        panic!("the admitted assignment asks its workspace capability to prepare")
    };
    workspace::prepare(&mut h.domain, &h.env, owner, workspace);
    h.domain.host_out.push(host::Request::Abort { owner });
    h
}

#[test]
fn a_capabilitys_callback_is_routed_before_the_hubs_next_request() {
    let mut h = held_beside_abort();
    route::hand_off(&mut h.domain, &h.env, &mut h.out);
    // Abort can name the hold only after Held has installed it. Its Cancel
    // then follows the checkout's already-issued operation at the boundary.
    let mut requests = List::with_capacity(CROSSINGS);
    for _ in 0..h.out.len() {
        requests.push(h.out.pop().expect("queued crossing")).expect("operation and cancellation");
    }
    let requests = requests.into_boxed();
    let [Request::Io { owner: operation, .. }, Request::CancelIo { owner: cancelled }] = requests.as_ref() else {
        panic!("the established hold's first IO is issued before its cancellation: {requests:?}")
    };
    assert_eq!(operation, cancelled, "the later abort cancels the operation of the callback's established hold");
}
