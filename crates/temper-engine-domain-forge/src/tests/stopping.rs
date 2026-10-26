use super::fixtures::{answer, next_call};
use super::*;

fn sent() -> (Domain, Token) {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    outputs(&mut d, Event::Enqueue { entry: status(1, 7) });
    outputs(&mut d, Event::Committed { entry: 1 });
    let (call, _) = next_call(&mut d);
    answer(&mut d, call, Ok(skein_forge::api::Answer::Items { items: Box::new([]), more: false, now: env().now }));
    let (call, op) = next_call(&mut d);
    assert_eq!(op, skein_forge::api::Op::Write(status(1, 7).effect.write));
    (d, call)
}

#[test]
fn settle_with_no_write_sent_settles_at_once() {
    let mut d = domain();
    assert_eq!(outputs(&mut d, Event::Settle).as_ref(), &[Request::Settled { unfinished: Box::new([]) }]);
}

#[test]
fn settle_waits_for_a_sent_write_and_starts_no_other() {
    let (mut d, call) = sent();
    outputs(&mut d, Event::Enqueue { entry: status(2, 8) });
    outputs(&mut d, Event::Committed { entry: 2 });
    assert!(outputs(&mut d, Event::Settle).is_empty());
    assert!(ready(&mut d).is_empty());
    let requests = outputs(
        &mut d,
        Event::Client(skein_forge_client::Event::Answered {
            call,
            cost: 1,
            free: 0,
            rate: None,
            result: Ok(skein_forge::api::Answer::Done),
        }),
    );
    assert!(requests.contains(&Request::Erase { key: Key::Entry(1) }));
    assert_eq!(requests.last(), Some(&Request::Settled { unfinished: Box::new([]) }));
    assert!(ready(&mut d).is_empty());
}

#[test]
fn settle_names_a_write_without_its_outcome_at_its_bound() {
    let (mut d, _call) = sent();
    outputs(&mut d, Event::Settle);
    let mut env = env();
    let until = env.now.saturating_add(CLIENT.lifetime.saturating_add(CLIENT.clock_margin).saturating_mul(2));
    assert_eq!(d.next_deadline(), Some(until));
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    env.now = Time::from_nanos(until.as_nanos().checked_sub(1).unwrap());
    fire(&mut d, &env, &mut out);
    assert!(out.is_empty());
    env.now = until;
    fire(&mut d, &env, &mut out);
    assert_eq!(out.pop(), Some(Request::Settled { unfinished: Box::new([Unfinished { entry: 1, task: 7 }]) }));
}

#[test]
fn an_entry_committed_while_settling_is_not_made() {
    let (mut d, _call) = sent();
    outputs(&mut d, Event::Enqueue { entry: status(2, 8) });
    outputs(&mut d, Event::Settle);
    assert!(outputs(&mut d, Event::Committed { entry: 2 }).is_empty());
    assert!(ready(&mut d).is_empty());
}
