use super::*;
use skein_lib::Time;

const LIMITS: Limits = Limits { projects: 3 };
const SPAN: Duration = Duration::from_nanos(10);
fn wall(n: u64) -> Wall {
    Wall::from_nanos(n)
}
fn calendar() -> Calendar {
    Calendar::new(&LIMITS, SPAN)
}
fn decide(calendar: &mut Calendar, now: u64, event: Event) -> Queue<Request> {
    let env = Env { now: Time::ZERO, wall: wall(now), limits: LIMITS };
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    step(calendar, &env, event, &mut out);
    out
}
fn restored(next: Option<Wall>) -> Event {
    Event::Restored { project: 1, period: 4, began: wall(5), next }
}
fn record(calendar: &Calendar, project: u32, period: u64, began: u64, next: u64) {
    assert_eq!(calendar.projects.get(&project), Some(&Record { period, began: wall(began), next: Some(wall(next)) }));
}
fn request(out: &mut Queue<Request>, expected: Request) {
    assert_eq!(out.pop(), Some(expected));
    assert!(out.is_empty());
}

#[test]
fn restored_without_a_next_start_is_given_one_a_span_after_it_began() {
    let mut c = calendar();
    request(&mut decide(&mut c, 100, restored(None)), Request::Next { project: 1, period: 4, next: wall(15) });
    record(&c, 1, 4, 5, 15);
}
#[test]
fn restored_past_its_next_start_opens_one_period_from_now() {
    let mut c = calendar();
    request(
        &mut decide(&mut c, 100, restored(Some(wall(15)))),
        Request::Open { project: 1, period: 5, next: wall(110) },
    );
    record(&c, 1, 5, 100, 110);
}
#[test]
fn restored_before_its_next_start_says_nothing() {
    let mut c = calendar();
    assert!(decide(&mut c, 10, restored(Some(wall(15)))).is_empty());
    record(&c, 1, 4, 5, 15);
}
#[test]
fn began_is_given_a_next_start() {
    let mut c = calendar();
    request(
        &mut decide(&mut c, 5, Event::Began { project: 1, period: 1, began: wall(5) }),
        Request::Next { project: 1, period: 1, next: wall(15) },
    );
    record(&c, 1, 1, 5, 15);
}
#[test]
fn due_opens_each_project_whose_next_start_came() {
    let mut c = calendar();
    for (project, began) in [(3, 30), (2, 6), (1, 5)] {
        drop(decide(&mut c, began, Event::Began { project, period: 1, began: wall(began) }));
    }
    let mut out = decide(&mut c, 16, Event::Due);
    assert_eq!(out.pop(), Some(Request::Open { project: 1, period: 2, next: wall(25) }));
    request(&mut out, Request::Open { project: 2, period: 2, next: wall(26) });
    record(&c, 1, 2, 16, 25);
    record(&c, 2, 2, 16, 26);
    record(&c, 3, 1, 30, 40);
}
#[test]
fn due_keeps_the_alignment_to_the_first_period() {
    let mut c = calendar();
    drop(decide(&mut c, 10, restored(Some(wall(15)))));
    request(&mut decide(&mut c, 16, Event::Due), Request::Open { project: 1, period: 5, next: wall(25) });
    record(&c, 1, 5, 16, 25);
}
#[test]
fn due_after_a_jump_opens_one_period_from_now() {
    let mut c = calendar();
    drop(decide(&mut c, 10, restored(Some(wall(15)))));
    request(&mut decide(&mut c, 100, Event::Due), Request::Open { project: 1, period: 5, next: wall(110) });
    record(&c, 1, 5, 100, 110);
}
#[test]
fn next_start_is_the_earliest_records_and_none_without_records() {
    let mut c = calendar();
    assert_eq!(c.next_start(), None);
    for (project, began) in [(1, 20), (2, 5), (3, 30)] {
        drop(decide(&mut c, began, Event::Began { project, period: 1, began: wall(began) }));
    }
    assert_eq!(c.next_start(), Some(wall(15)));
}
#[test]
fn wall_arithmetic_saturates() {
    let mut c = calendar();
    request(
        &mut decide(&mut c, u64::MAX - 5, Event::Began { project: 1, period: 1, began: wall(u64::MAX - 5) }),
        Request::Next { project: 1, period: 1, next: wall(u64::MAX) },
    );
    record(&c, 1, 1, u64::MAX - 5, u64::MAX);
}
