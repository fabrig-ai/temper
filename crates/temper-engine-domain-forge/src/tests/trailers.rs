use super::*;
use fixtures::{BASE, MERGE};
use skein_forge::api::{Answer, CommitMessage, Op, Read};
const GOAL: u64 = 301;
const SUBSCRIPTION: u64 = 302;
fn subscription(markers: u32) -> Subscriber {
    Subscriber {
        markers,
        task: GOAL,
        number: SUBSCRIPTION,
        topic: Topic::Landings { repository: REPO, branch: b"main".as_slice().into() },
        own_change: None,
        goal_tasks: Some(Box::new([])),
        paths: Box::new([b"elsewhere".as_slice().into()]),
    }
}
fn parsed(message: &[u8], limits: &Limits) -> (Box<[Box<[u8]>]>, bool) {
    let mut cut = false;
    let lines = crate::trailers::collect(&[], message, limits, &mut cut);
    (lines, cut)
}
#[test]
fn only_a_messages_terminal_trailer_block_counts() {
    for message in [
        b"Taught: prose\n\nlast paragraph".as_slice(),
        b"title\n\nTaught: embedded\nprose",
        b"title\nTaught: no paragraph",
    ] {
        assert_eq!(parsed(message, &LIMITS), (Vec::<Box<[u8]>>::new().into_boxed_slice(), false));
    }
    assert_eq!(
        parsed(b"Taught: earlier\n\nQA: terminal\nCo-Authored-By: someone\n", &LIMITS),
        (vec![Box::from(b"QA: terminal".as_slice())].into_boxed_slice(), false)
    );
}
#[test]
fn trailers_past_their_bound_are_cut_and_said_so() {
    let limits = Limits { trailers: 1, trailer_bytes: 9, ..LIMITS }; // One nine-byte 'Taught: a' line.
    assert_eq!(
        parsed(b"title\n\nTaught: a\n", &limits),
        (vec![Box::from(b"Taught: a".as_slice())].into_boxed_slice(), false)
    );
    assert_eq!(
        parsed(b"title\n\nTaught: ab\nQA: extra", &limits),
        (vec![Box::from(b"Taught: a".as_slice())].into_boxed_slice(), true)
    );
}
fn reading_compare(result: Result<Answer, skein_forge::api::Error>) -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::BranchHead(BranchHead { name: fixtures::main_name(), commit: BASE, owned: false }),
        },
    );
    outputs(&mut d, Event::Subscribe { subscription: subscription(1) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    fixtures::prime_main(&mut d);
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    domain::branch_changed(
        &mut d,
        &env(),
        skein_forge_client::Resource {
            repository: REPO,
            what: skein_forge_client::What::Branch(b"main".as_slice().into()),
        },
        MERGE,
        &mut out,
    );
    fixtures::answer_next(&mut d, result);
    d
}
fn reading() -> Domain {
    reading_compare(Ok(Answer::Compare {
        before: BASE,
        after: MERGE,
        contains_before: true,
        files: Box::new([]),
        commits: Box::new([MERGE]),
    }))
}
#[test]
fn a_failed_landing_read_keeps_its_trailers_and_marks_the_news_cut() {
    let mut d = reading_compare(Err(skein_forge::api::Error::Forbidden));
    let out = fixtures::answer_next(&mut d, Ok(messages(&[b"title\n\nTaught: retained"], false)));
    assert!(matches!(landing(&out), News::Landing { cut: true, trailers, .. } if trailers.len() == 1));

    let mut d = reading();
    fixtures::answer_next(&mut d, Ok(messages(&[b"title\n\nTaught: retained"], true)));
    let out = fixtures::answer_next(&mut d, Err(skein_forge::api::Error::Forbidden));
    assert!(matches!(landing(&out), News::Landing { cut: true, trailers, .. } if trailers.len() == 1));
}
fn messages(values: &[&[u8]], more: bool) -> Answer {
    let commits = values
        .iter()
        .enumerate()
        .map(|(index, message)| CommitMessage {
            id: [u8::try_from(index + 1).unwrap(); 32],
            parents: Box::new([]),
            message: Box::from(*message),
        })
        .collect();
    Answer::Commits { more, commits }
}
fn landing(out: &[Request]) -> &News {
    out.iter()
        .find_map(|request| match request {
            Request::News { news, .. } => Some(news),
            _ => None,
        })
        .expect("landing news")
}
#[test]
fn a_landings_trailers_are_read_from_its_commits() {
    let mut d = reading();
    let (call, op) = fixtures::next_call(&mut d);
    assert_eq!(op, Op::Read(Read::Commits { before: BASE, after: MERGE, page: 1 }));
    let out = fixtures::answer(&mut d, call, Ok(messages(&[b"one\n\nTaught: first", b"two\n\nQA: second"], false)));
    let News::Landing { trailers, cut, .. } = landing(&out) else { panic!("landing") };
    assert_eq!(trailers.as_ref(), &[Box::from(b"Taught: first".as_slice()), Box::from(b"QA: second".as_slice())]);
    assert!(!cut);
}
#[test]
fn a_range_past_its_pages_keeps_what_it_read() {
    let mut d = reading();
    fixtures::answer_next(&mut d, Ok(messages(&[b"title\n\nTaught: first"], true)));
    for _ in 1..LIMITS.trailers - 1 {
        fixtures::answer_next(&mut d, Ok(messages(&[], true)));
    }
    let out = fixtures::answer_next(&mut d, Ok(messages(&[], true)));
    let News::Landing { trailers, cut, .. } = landing(&out) else { panic!("landing") };
    assert_eq!(trailers.as_ref(), &[Box::from(b"Taught: first".as_slice())]);
    assert!(cut);
}
#[test]
fn landing_news_and_the_log_carry_the_trailers() {
    let mut d = reading();
    let out = fixtures::answer_next(&mut d, Ok(messages(&[b"title\n\nRepairs: item"], false)));
    let News::Landing { trailers, .. } = landing(&out) else { panic!("landing") };
    assert_eq!(d.landing_logs.get(&REPO).unwrap().landings[0].trailers, *trailers);
    assert!(out.iter().any(|request| matches!(request, Request::Save { record: Stored::LandingLog(row), .. } if row.landings[0].trailers == *trailers)));
}
fn classified(markers: u32, own: bool) -> Class {
    let mut d = domain();
    if own {
        d.landed.insert(MERGE, GOAL).unwrap();
    }
    let mut subscriber = subscription(markers);
    subscriber.goal_tasks = Some(Box::new([GOAL]));
    topics::classify(
        &d,
        &subscriber,
        &News::Landing {
            before: BASE,
            after: MERGE,
            files: Some(Box::new([])),
            trailers: Box::new([b"Taught: a lesson".as_slice().into()]),
            cut: false,
        },
    )
}
#[test]
fn a_goal_naming_taught_wakes_on_a_landing_carrying_one_whatever_its_overlap() {
    for own in [false, true] {
        assert_eq!(classified(1, own), Class::Wakes);
    }
}
#[test]
fn a_goal_naming_no_marker_keeps_a_landing_it_does_not_overlap() {
    assert_eq!(classified(0, false), Class::Kept);
}
