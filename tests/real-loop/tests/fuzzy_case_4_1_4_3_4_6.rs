//! The opener, the timer check, and a restarted workstation on the same three workers.
use temper_real_loop_world::{
    End, Exit,
    case::{
        self, acts,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

const WORKERS: &[&str] = &["ws", "s1", "s2"];

#[test]
fn the_opener_the_two_hour_check_and_a_restarted_workstation_run_on_three_workers() {
    // Leaves out survey, instructions/pool, host kinds, Markdown CI's fast path and goal issue; pools/load/queue/disk, moving B and the plane: testing.md 5.3's 4.1 and 4.3 rows; the hang past the grace, a fenced attempt still running, a stalled run's bound, three questions, no single point, the reboot's faults: testing.md 5.3's 4.6 row.
    let cluster = run_cluster_forge(
        WORKERS,
        &["s2"],
        case::fixture_4_6(),
        case::scripts::step_4_1_4_3_4_6,
        acts::step_4_1_4_3_4_6,
    );
    for (at, called) in cluster.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    let observed = Observed::from_ran(&cluster, &acts::reads_4_1_4_3_4_6());
    let queued = &observed.documents[&acts::Read::BenchmarkWaiting];
    assert_eq!(queued.pointer("/task/chip/phase").and_then(serde_json::Value::as_str), Some("due"));
    let mut misses = oracle::step_4_1(&observed);
    misses.extend(oracle::step_4_3(&observed));
    misses.extend(oracle::step_4_6(&observed));
    oracle::assert_met(&misses);
    for name in WORKERS {
        let worker = cluster.worker(name);
        assert_eq!(worker.end(), Some(&End::Drained), "{name} drained");
    }
    assert_eq!(cluster.worker("ws").ran(), &[case::DIRECTOR, case::CHECK, case::HANG], "ws ran the three chats");
    let s2: Vec<_> =
        cluster.worker("s2").ran().iter().copied().filter(|task| ![case::HANG, case::PHASE].contains(task)).collect();
    assert_eq!(s2, &[case::BENCHMARK], "s2 ran only the benchmark before the third part");
    // A parks between its wakes, so its activations' count is the engine's; the tasks are not.
    let s1 = cluster.worker("s1").ran();
    let opener = [case::A, case::CODER, case::REVIEW];
    let before_phase: Vec<_> = s1.iter().copied().filter(|task| ![case::HANG, case::PHASE].contains(task)).collect();
    let (last, earlier) = before_phase.split_last().expect("s1 ran tasks");
    assert_eq!(*last, case::B, "s1 ran B after the opener");
    assert!(earlier.iter().all(|task| opener.contains(task)), "s1 ran the opener's A and delegates: {s1:?}");
    assert!(opener.iter().all(|task| s1.contains(task)), "s1 ran each of A, its coder and reviewer: {s1:?}");
    let added = cluster.worker("s2").started_at().expect("late worker started");
    for name in ["ws", "s1"] {
        assert!(
            cluster.worker(name).assigned_at().expect("initial worker assigned") <= added,
            "{name} assigned before s2 started"
        );
    }
    let lives = cluster.worker("ws").lives();
    assert!(lives.iter().all(|life| life.drained), "both ws lives drained");
    assert!(lives.iter().all(|life| life.linked > 0), "both ws lives linked");
    assert!(lives.last().expect("restarted ws").ran.is_empty(), "the restarted ws ran no task");
    assert!(
        ["s1", "s2"].iter().any(|name| cluster.worker(name).ran().contains(&case::HANG)),
        "director resumed elsewhere"
    );
    assert_eq!(lives.len(), WORKSTATION_LIVES, "ws kept its root across two lives");
    assert_eq!(cluster.worker("ws").linked(), WORKSTATION_LIVES, "ws linked for each life");
    for life in lives {
        assert_eq!(life.linked, ONE, "each life linked once");
    }
    let other: Vec<_> =
        ["s1", "s2"].iter().flat_map(|name| cluster.worker(name).ran()).filter(|task| **task == case::HANG).collect();
    assert_eq!(other.len(), ONE, "the director had one activation away from ws");
    assert!(cluster.killed().is_empty(), "cancellation was cooperative");
}

const WORKSTATION_LIVES: usize = 2;
const ONE: usize = 1;
