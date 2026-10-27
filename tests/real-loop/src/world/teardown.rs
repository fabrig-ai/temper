//! Startup failure is terminal after the unreleased caller closes its descriptors.
use super::*;
use std::{cell::Cell, net::TcpListener, rc::Rc};

#[derive(Default)]
struct Receipt {
    settled: Cell<bool>,
    calls: Cell<usize>,
    passed: Cell<bool>,
}
struct Recording {
    judge: Judge,
    receipt: Rc<Receipt>,
}
impl Referee<Proc> for Recording {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        self.judge.act(now, procs);
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        self.judge.observe(now, procs);
        if self.judge.failure.is_some() {
            self.receipt.settled.set(procs.iter().all(Host::is_empty));
            self.receipt.passed.set(self.judge.passed());
            let Proc::Caller(caller) = &procs[2] else { panic!("caller root") };
            self.receipt.calls.set(caller.calls.len());
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        self.judge.next_deadline()
    }
    fn overdue(&self, now: Time) -> Option<String> {
        self.judge.overdue(now)
    }
    fn passed(&self) -> bool {
        self.judge.passed()
    }
}

fn startup(
    scratch: &skein_scratch::Scratch,
    occupied: Option<&TcpListener>,
    receipt: Rc<Receipt>,
) -> skein_world::real::Outcome<Proc> {
    let patience = crate::PATIENCE;
    let fixture = crate::github::fixture();
    let (state, peer) = crate::github::State::new(fixture);
    let mut world = World::new_controlled(|controls| {
        let mut judge = cluster_judge(controls, &[], None, patience);
        judge.github = Some(state);
        Recording { judge, receipt }
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            pending_streak: crate::patience::PendingStreak::default(),
            peer: llm::peer(llm::scripted(Box::new([]))).expect("deferred LLM"),
            scripts: Some(|_| Box::new([])),
            recipe: None,
            rules: None,
            cue: false,
            release: None,
            released: None,
        }))
    });
    world.spawn_signals(|signals| {
        let mut service = ServiceHost::github(scratch.path().into(), signals, fixture);
        if let Some(listener) = occupied {
            service.clients_occupied_by(listener);
        }
        Proc::Service(Box::new(service))
    });
    host(&mut world, scratch.path(), || vec![crate::calls::sign_in(crate::calls::D1)], true, 1);
    world.spawn(|| Proc::Forge(Box::new(ForgePeer::new(peer))));
    world.run(&skein_shell::Clock::new(), patience)
}

#[test]
fn a_service_exit_before_its_link_fails_after_the_caller_settles() {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("occupied listener");
    let scratch = skein_scratch::Scratch::new("failed-start");
    let receipt = Rc::new(Receipt::default());
    let descriptors = std::fs::read_dir("/proc/self/fd").expect("own descriptors").count();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| startup(&scratch, Some(&listener), receipt.clone())));
    let Err(payload) = result else { panic!("a service exit before its link must fail the story") };
    let message = payload.downcast_ref::<String>().expect("failure message");
    assert!(message.contains("service 1 exited before caller release: Code(1)"), "{message}");
    assert!(message.contains("stderr tail:\ntemper serve: client listener failed\n"), "{message}");
    assert!(!receipt.passed.get(), "failed startup never passes the referee");
    assert!(receipt.settled.get(), "the failure is reported after every host empties");
    assert_eq!(receipt.calls.get(), 0, "no CLI command ran before release");
    assert_eq!(
        std::fs::read_dir("/proc/self/fd").expect("own descriptors").count(),
        descriptors,
        "failed startup releases every descriptor"
    );
}

#[test]
fn a_released_caller_runs_its_story_and_the_world_settles() {
    let scratch = skein_scratch::Scratch::new("released-start");
    let outcome = startup(&scratch, None, Rc::new(Receipt::default()));
    let Proc::Caller(caller) = &outcome.procs[2] else { panic!("caller root") };
    let [call] = caller.calls.as_slice() else { panic!("the story has one sign-in command") };
    assert_eq!(call.exit, Some(Exit::Code(0)));
    assert!(caller.released);
    assert!(outcome.procs.iter().all(Host::is_empty));
}
