use super::*;
#[test]
fn a_restart_over_a_live_change_answers_a_person() {
    let settings = fixtures::calm(fixtures::SEED);
    let run = Rc::new(RefCell::new(referee::Run::default()));
    let (_, disk, store) =
        run_phase(settings, Story::HeldChange, Phase::Story, machine::fresh(), Store::new(), run.clone());
    let (outcome, _, _) = run_phase(settings, Story::HeldChange, Phase::SignIn, disk, store, run);
    assert_eq!(outcome.signed_in, 1);
}

#[test]
fn an_adoption_saves_a_pending_window_before_the_engine_commit() {
    let setup = Rc::new(RefCell::new(referee::Run::default()));
    let (outcome, _, _) =
        run_phase(fixtures::calm(1), Story::Adoption, Phase::Story, machine::fresh(), Store::new(), setup.clone());
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(outcome.adopted, 1);
    PROGRESS.with(|progress| {
        assert!(
            setup.borrow().window.is_some(),
            "saved pending adoption window: {:?}",
            progress.borrow().cache.borrow()
        );
    });
}

use skein_io::kernel::{Done, Error, Op};
use skein_lib::Token;
// Two fixture binds exercise kernel-assigned hooks and fixed service ports.
const HOOKS: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 41000));
fn bind(asked: u16, result: Result<Done, Error>) -> Complete {
    Complete { op: Token::new(1), kind: Op::Bind { fd: Fd::new(1), addr: ([127, 0, 0, 1], asked).into() }, result }
}
fn listen() -> Complete {
    Complete { op: Token::new(2), kind: Op::Listen { fd: Fd::new(1), backlog: 8 }, result: Ok(Done::Nothing) }
}
#[test]
fn a_port_zero_listener_names_the_hooks_address_once_it_listens() {
    let mut bound = None;
    let complete = bind(0, Ok(Done::Bound(HOOKS)));
    assert_eq!(hooks_address(&mut bound, &complete), None);
    assert_eq!(hooks_address(&mut bound, &listen()), Some(HOOKS));
}
#[test]
fn a_fixed_port_bind_is_not_a_hooks_address() {
    let mut bound = None;
    let fixed = ([127, 0, 0, 1], 7102).into();
    let complete = bind(7102, Ok(Done::Bound(fixed)));
    assert_eq!(hooks_address(&mut bound, &complete), None);
    assert_eq!(hooks_address(&mut bound, &listen()), None);
}
#[test]
fn a_failed_bind_is_not_a_hooks_address() {
    let mut bound = None;
    let complete = bind(0, Err(Error::AddressInUse));
    assert_eq!(hooks_address(&mut bound, &complete), None);
}

#[test]
fn a_refused_restore_ends_with_its_in_process_worker() {
    let scenario = Story::Chat { words: b"Hello" };
    let settings = fixtures::calm(fixtures::SEED);
    let run = Rc::new(RefCell::new(referee::Run::default()));
    let (_, disk, store) = run_phase(settings, scenario, Phase::SignIn, machine::fresh(), Store::new(), run.clone());
    let (outcome, _, _) = run_phase(settings, scenario, Phase::Refused, disk, store, run);
    let Some(temper_service::End::Stopped(temper_service::Stop::Start(problems))) = outcome.end else {
        panic!("the stored policy refuses restore: {:?}", outcome.end);
    };
    let problems = temper_service::startup::Problems::jig(problems, &jig_protocol_config::Path::root());
    assert_eq!(
        temper_service::startup::lines(&problems).as_ref(),
        b"settings.binary's default.policy[1].shape: invalid form\n"
    );
}
