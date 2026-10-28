//! What the worker's limits must hold to be honoured.

use super::fixtures::limits;
use crate::{Limits, agent, host, valid};

#[test]
fn the_fixture_limits_fit() {
    assert!(valid(&limits()));
}

#[test]
fn a_smith_window_of_more_turns_than_the_hub_keeps_does_not_fit() {
    let fixture = limits();
    let limits = Limits { agent: agent::Limits { turns: 3, ..fixture.agent }, ..limits() };
    assert!(!valid(&limits));
}

#[test]
fn a_smith_window_of_turns_larger_than_the_hubs_bytes_does_not_fit() {
    let fixture = limits();
    let limits = Limits { host: host::Limits { turn_bytes: 65, ..fixture.host }, ..limits() };
    assert!(!valid(&limits));
}

#[test]
fn a_hello_promising_a_larger_inbox_than_the_hub_takes_does_not_fit() {
    let fixture = limits();
    let limits = Limits { bounds: host::Bounds { inbox: 3, ..fixture.bounds }, ..limits() };
    assert!(!valid(&limits));
}

#[test]
fn a_hello_promising_larger_messages_than_the_hub_takes_does_not_fit() {
    let fixture = limits();
    let limits = Limits { bounds: host::Bounds { message_bytes: 97, ..fixture.bounds }, ..limits() };
    assert!(!valid(&limits));
}

#[test]
fn a_hello_promising_more_unacknowledged_turns_than_smith_sends_does_not_fit() {
    let fixture = limits();
    let limits = Limits { bounds: host::Bounds { unacknowledged: 3, ..fixture.bounds }, ..limits() };
    assert!(!valid(&limits));
}
