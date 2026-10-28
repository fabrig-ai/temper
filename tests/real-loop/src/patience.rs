//! Temper-only detail at the world's patience boundary. No report is built on a passing path.
use crate::{calls::Caller, world::Proc};
use skein_lib::{Duration, Time};
use skein_shell::Host;
use skein_world::{Controls, Referee};
use std::fmt::Write;

#[derive(Debug)]
pub(crate) struct Patience {
    limit: Duration,
    after: Duration,
    deadline: Time,
    report: Option<String>,
    deferred: bool,
}
impl Patience {
    pub(crate) fn new(patience: Duration) -> Self {
        let margin = Duration::from_secs(1);
        let after = patience.as_nanos().checked_sub(margin.as_nanos()).expect("patience exceeds diagnostic margin");
        assert!(after > 0, "a positive diagnostic window");
        Self {
            limit: patience,
            after: Duration::from_nanos(after),
            deadline: Time::ZERO,
            report: None,
            deferred: false,
        }
    }
    /// The first act uses the same pass clock as the world, after all fixture admission.
    pub(crate) fn due(&mut self, now: Time) -> bool {
        if self.deferred || now < self.deadline {
            return false;
        }
        if self.deadline == Time::ZERO {
            self.deadline = now.saturating_add(self.after);
            return false;
        }
        true
    }
    /// Read scheduler accounting only when the early diagnostic falls due.
    pub(crate) fn due_in_world(&mut self, now: Time, controls: &Controls) -> bool {
        self.due(now) && !self.defer_if_starved(controls.runnable_wait(), std::env::var_os("NEXTEST").is_some())
    }
    /// Leave starvation classification and forced cleanup to the world's own
    /// patience. The threshold uses the full patience, not our diagnostic margin.
    fn defer_if_starved(&mut self, waited: Option<Duration>, nextest: bool) -> bool {
        if nextest && waited.is_some_and(|waited| waited.as_nanos() >= self.limit.as_nanos().div_ceil(2)) {
            self.deferred = true;
        }
        self.deferred
    }
    pub(crate) fn deadline(&self) -> Option<Time> {
        (!self.deferred && self.deadline != Time::ZERO).then_some(self.deadline)
    }
    pub(crate) fn capture(&mut self, report: String) {
        self.report = Some(report);
    }
    pub(crate) fn overdue(&self) -> Option<String> {
        self.report.clone()
    }
}

/// Recorded beside the readiness check that the host already makes, once per loop pass.
#[derive(Debug, Default)]
pub(crate) struct PendingStreak(u64);
impl PendingStreak {
    pub(crate) fn observe(&mut self, pending: bool) {
        self.0 = if pending { self.0.saturating_add(1) } else { 0 };
    }
    pub(crate) const fn passes(&self) -> u64 {
        self.0
    }
}

/// Skein asks overdue only on an idle pass. A busy spin must ask it from act as well.
pub(crate) fn fail_overdue<P, R: Referee<P>>(judge: &R, now: Time) {
    if let Some(why) = judge.overdue(now) {
        panic!("at {now:?}, the referee failed the real loop before patience:\n{why}");
    }
}

pub(crate) fn header(id: usize, name: &str, host: &dyn Host, streak: u64, now: Time, out: &mut String) {
    let pending = host.work_pending(now);
    let streak = if pending { streak } else { 0 };
    writeln!(out, "proc slot={id} {name}: work_pending={pending} pending_passes={streak} next_deadline={:?} next_policy_deadline={:?} empty={} exit={:?}", host.next_deadline(), host.next_policy_deadline(), host.is_empty(), host.exit()).expect("string writer");
}

pub(crate) fn llm(id: usize, peer: &mut skein_fake_peers::llm::Peer, streak: u64, now: Time, out: &mut String) {
    header(id, "LLM", peer, streak, now, out);
    let outstanding = peer.observations().iter().fold(0_i64, |pending, event| {
        use skein_fake_peers::llm::Observation;
        pending + i64::from(matches!(event, Observation::Query { .. }))
            - i64::from(matches!(event, Observation::Answered { .. }))
    });
    // These peer getters return the queues directly; unlike the service adapter they drain nothing.
    let completions = peer.completions().len();
    let submissions = peer.submissions().len();
    writeln!(out, "LLM queues completions={completions} submissions={submissions} observed_unanswered={outstanding}")
        .expect("string writer");
}

pub(crate) fn caller(id: usize, caller: &Caller, now: Time, out: &mut String) {
    header(id, "caller", caller, 0, now, out);
    out.push_str(&caller.describe_pending());
}

/// Formatting belongs to the std harness; the service only returns closed data.
fn service_pending(service: &temper_service::Service, now: Time, out: &mut String) {
    writeln!(out, "service {:#?}", service.pending(now)).expect("string writer");
}

/// Constructed only at overdue, including the caller's bounded terminal transcript metadata.
pub(crate) fn processes(procs: &mut [Proc], now: Time, out: &mut String) {
    let typist = procs.iter().find_map(|proc| match proc {
        Proc::Caller(caller) => Some(caller.terminal_pending()),
        Proc::Llm(_)
        | Proc::Forge(_)
        | Proc::Issuer(_)
        | Proc::Service(_)
        | Proc::Ctl(_)
        | Proc::Terminal(_)
        | Proc::Agent(_)
        | Proc::Git(_)
        | Proc::GitRoute(_)
        | Proc::Claude(_)
        | Proc::Relay(_) => None,
    });
    for (id, proc) in procs.iter_mut().enumerate() {
        describe(id, proc, now, out);
        if matches!(proc, Proc::Terminal(_)) {
            writeln!(out, "terminal typist {typist:#?}").expect("string writer");
        }
    }
}

pub(crate) fn describe(id: usize, proc: &mut Proc, now: Time, out: &mut String) {
    match proc {
        Proc::Llm(peer) => {
            llm(id, &mut peer.peer, peer.pending_streak.passes(), now, out);
            writeln!(out, "LLM hold release={:?} released={:?}", peer.release, peer.released).expect("string writer");
        }
        Proc::Service(host) => {
            header(id, "service", host.as_ref(), host.pending_streak.passes(), now, out);
            if let Some(service) = &host.service {
                service_pending(service, now, out);
            } else {
                out.push_str("service awaiting preparation\n");
            }
        }
        Proc::Caller(host) => caller(id, host, now, out),
        Proc::Forge(host) => {
            header(id, "forge", &host.peer, host.pending_streak.passes(), now, out);
            let completions = host.peer.completions().len();
            let submissions = host.peer.submissions().len();
            writeln!(
                out,
                "forge queues completions={completions} submissions={submissions} blob_records={:?}",
                host.peer.blob_records().map(skein_lib::Queue::len)
            )
            .expect("string writer");
        }
        Proc::Ctl(recorded) => {
            header(id, "ctl", recorded.as_ref(), 0, now, out);
            writeln!(out, "ctl call={} setup={}", recorded.call, recorded.setup).expect("string writer");
        }
        Proc::Agent(host) => header(id, "smith agent", host.as_ref(), 0, now, out),
        Proc::Claude(host) => header(id, "fake Claude", host.as_ref(), 0, now, out),
        Proc::Relay(host) => header(id, "smith relay", host.as_ref(), 0, now, out),
        Proc::Git(host) => header(id, "hosted git", host.as_ref(), 0, now, out),
        Proc::GitRoute(host) => header(id, "git route", host.as_ref(), 0, now, out),
        Proc::Issuer(host) => header(id, "issuer", host.as_ref(), 0, now, out),
        Proc::Terminal(host) => {
            header(id, "terminal", host.as_ref(), 0, now, out);
            writeln!(out, "terminal {:#?}", host.pending()).expect("string writer");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Patience, fail_overdue, header};
    use crate::service::ServiceHost;
    use skein_io::kernel::{Complete, Exit, Fd, Submit};
    use skein_lib::{Duration, Queue, Time, Wall};
    use skein_shell::Host;
    use skein_world::{Referee, real::World};

    /// Deliberately never advances a prepared service's startup store work.
    struct Frozen {
        service: ServiceHost,
        completions: Queue<Complete>,
        submissions: Queue<Submit>,
    }
    impl Host for Frozen {
        fn iterate(&mut self, now: Time, _wall: Wall) {
            self.service.pending_streak.observe(self.service.work_pending(now));
        }
        fn completions(&mut self) -> &mut Queue<Complete> {
            &mut self.completions
        }
        fn submissions(&mut self) -> &mut Queue<Submit> {
            &mut self.submissions
        }
        fn work_pending(&self, now: Time) -> bool {
            self.service.work_pending(now)
        }
        fn next_deadline(&self) -> Option<Time> {
            None
        }
        fn next_policy_deadline(&self) -> Option<Time> {
            None
        }
        fn is_empty(&self) -> bool {
            false
        }
        fn exit(&self) -> Option<Exit> {
            None
        }
        fn operations(&self) -> u32 {
            1
        }
    }
    struct Judge {
        patience: Patience,
    }
    impl Referee<Frozen> for Judge {
        fn act(&mut self, now: Time, procs: &mut [Frozen]) {
            if self.patience.due(now) {
                let frozen = &procs[0];
                let mut report = String::new();
                header(0, "service", &frozen.service, frozen.service.pending_streak.passes(), now, &mut report);
                super::service_pending(frozen.service.service.as_ref().expect("prepared service"), now, &mut report);
                self.patience.capture(report);
                fail_overdue::<Frozen, _>(self, now);
            }
        }
        fn observe(&mut self, _now: Time, _procs: &[Frozen]) {}
        fn next_deadline(&self) -> Option<Time> {
            self.patience.deadline()
        }
        fn overdue(&self, _now: Time) -> Option<String> {
            self.patience.overdue()
        }
        fn passed(&self) -> bool {
            false
        }
    }

    #[test]
    fn a_busy_world_fails_through_overdue_naming_the_pending_service_component() {
        let scratch = crate::Scratch::new("overdue-service");
        let mut service = ServiceHost::new(scratch.path().into(), Fd::new(0), true);
        let prepared = temper::prepare_in(&crate::service::directories(scratch.path())).expect("prepared service");
        let first = format!("{:?}", prepared.pending(Time::ZERO));
        let second = format!("{:?}", prepared.pending(Time::ZERO));
        assert_eq!(first, second, "taking the snapshot never advances the service");
        assert!(prepared.work_pending(Time::ZERO));
        service.service = Some(prepared);
        let patience = Duration::from_nanos(1_010_000_000);
        let mut world = World::new(Judge { patience: Patience::new(patience) });
        world.spawn(|| Frozen { service, completions: Queue::with_capacity(1), submissions: Queue::with_capacity(1) });
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _outcome = world.run(&skein_shell::Clock::new(), patience);
        }))
        .expect_err("the world cannot settle");
        let message = error
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| error.downcast_ref::<&str>().copied())
            .expect("panic text");
        assert!(message.contains("the referee failed the real loop before patience"), "{message}");
        assert!(message.contains("engine: Some(\n        Engine {\n            work_pending: true"), "{message}");
        assert!(message.contains("store: true"), "{message}");
        assert!(message.contains("settled_revision:"), "{message}");
        assert!(message.contains("pending_passes="), "{message}");
        assert!(!message.contains("pending_passes=0 "), "the busy passes were counted: {message}");
    }

    #[test]
    fn the_caller_snapshot_names_the_current_wait_without_advancing_it() {
        let mut caller = crate::calls::Caller::new(
            vec![crate::calls::call(
                crate::calls::D1,
                crate::argv!["wait", "task", crate::calls::number(3), "--until", "ended"],
            )],
            std::path::PathBuf::new(),
            Fd::new(0),
        );
        caller.released = true;
        caller.iterate(Time::from_nanos(1), Wall::EPOCH);
        let queued = caller.submissions().len();
        let first = caller.describe_pending();
        assert_eq!(first, caller.describe_pending());
        assert_eq!(caller.submissions().len(), queued);
        assert_eq!(caller.calls.len(), 1);
        assert_eq!(caller.calls[0].tries, 1);
        assert!(first.contains("step=0 call=Some(0)"), "{first}");
        assert!(first.contains("argv=[\"wait\", \"task\", \"3\", \"--until\", \"ended\"]"), "{first}");
        assert!(first.contains("waits_for=ctl response and exit: wait task 3 --until ended"), "{first}");
    }

    // A 20-second world, with the one-second diagnostic margin and a nonzero origin.
    const START: Time = Time::from_nanos(7);
    const EARLY: Time = Time::from_nanos(19_000_000_007);
    const LIMIT: Time = Time::from_nanos(20_000_000_007);

    fn armed_guard() -> Patience {
        let mut guard = Patience::new(Duration::from_secs(20));
        let _armed = guard.due(START);
        guard
    }

    #[test]
    fn a_starved_loop_leaves_both_deadline_moments_to_the_world() {
        let mut guard = armed_guard();
        assert!(guard.due(EARLY));
        assert!(guard.defer_if_starved(Some(Duration::from_secs(10)), true));
        for now in [EARLY, LIMIT] {
            assert!(!guard.due(now), "the world's patience owns the starved verdict");
            assert_eq!(guard.deadline(), None, "the early deadline cannot keep waking the loop");
            assert_eq!(guard.overdue(), None);
        }
    }

    #[test]
    fn idle_and_spinning_loops_keep_the_early_diagnostic() {
        for waited in [None, Some(Duration::ZERO), Some(Duration::from_nanos(9_999_999_999))] {
            let mut guard = armed_guard();
            assert!(guard.due(EARLY));
            assert!(!guard.defer_if_starved(waited, true));
            guard.capture("held service component".into());
            assert_eq!(guard.deadline(), Some(EARLY));
            assert!(guard.due(LIMIT));
            let judge = Judge { patience: guard };
            let error = std::panic::catch_unwind(|| fail_overdue::<Frozen, _>(&judge, EARLY))
                .expect_err("a non-starved loop still fails before patience");
            let message = error.downcast_ref::<String>().expect("formatted early diagnostic");
            assert!(message.contains("the referee failed the real loop before patience"), "{message}");
            assert!(message.contains("held service component"), "{message}");
        }
    }

    #[test]
    fn starvation_outside_nextest_keeps_the_early_deadline() {
        let mut guard = armed_guard();
        assert!(guard.due(EARLY));
        assert!(!guard.defer_if_starved(Some(Duration::from_secs(10)), false));
        assert_eq!(guard.deadline(), Some(EARLY));
        assert!(guard.due(LIMIT));
    }

    #[test]
    fn a_held_terminal_snapshot_is_bounded_nonsecret_and_does_not_load_a_machine() {
        use crate::world::Proc;
        let printed = b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let host = crate::terminal::compose(
            std::path::Path::new("/private-directory-must-not-appear"),
            temper::open::target(printed).expect("synthetic link"),
            Fd::new(3),
            Fd::new(0),
            Fd::new(1),
            Fd::new(2),
        );
        let caller = crate::calls::Caller::new(Vec::new(), std::path::PathBuf::new(), Fd::new(4));
        let mut procs = [Proc::Caller(Box::new(caller)), Proc::Terminal(Box::new(host))];
        let mut first = String::new();
        super::processes(&mut procs, Time::ZERO, &mut first);
        let mut second = String::new();
        super::processes(&mut procs, Time::ZERO, &mut second);
        assert_eq!(first, second, "snapshot observation changes no client or typist state");
        for expected in [
            "proc slot=1 terminal:",
            "loaded: false",
            "held: 0",
            "active: 0",
            "queued: 0",
            "attempts: 0",
            "deferred_reads: 0",
            "chat: None",
            "view_revision: None",
            "builds: 0",
            "terminal typist Some",
            "commands: 0",
            "pages: 0",
            "recent: []",
        ] {
            assert!(first.contains(expected), "missing {expected}: {first}");
        }
        for secret in ["private-directory-must-not-appear", "127.0.0.1", "AAAAAAAA", "Token("] {
            assert!(!first.contains(secret), "diagnostic leaked {secret}: {first}");
        }
        assert!(first.len() < 4096, "an unloaded snapshot has a fixed small envelope");
        assert!(procs[1].submissions().is_empty(), "no descriptor was driven or loaded");
    }

    #[test]
    fn the_guard_derives_its_wake_deadline_from_the_supplied_patience() {
        for seconds in [3, 20, 40] {
            let mut guard = Patience::new(Duration::from_secs(seconds));
            assert_eq!(guard.deadline(), None);
            let start = Time::from_nanos(7);
            assert!(!guard.due(start));
            let at = start.saturating_add(Duration::from_secs(seconds - 1));
            assert_eq!(guard.deadline(), Some(at));
            assert!(!guard.due(Time::from_nanos(at.as_nanos() - 1)));
            assert!(guard.overdue().is_none(), "no message or snapshot before it fires");
            assert!(guard.due(at));
        }
    }
}
