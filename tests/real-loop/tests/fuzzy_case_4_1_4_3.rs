//! Retained first-edition story (domain/testing.md, 5.3): opener followed by its timer check on three workers.
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
fn the_opener_and_the_two_hour_check_run_on_three_workers() {
    // Leaves out survey, instructions/pool, host kinds, Markdown CI's fast path and goal issue; pools/load/queue/disk, moving B and the plane: testing.md 5.3's 4.1 and 4.3 rows.
    let cluster = run_cluster_forge(
        WORKERS,
        &["s2"],
        case::fixture_check(),
        case::scripts::step_4_1_4_3,
        case::scripts::step_4_1_4_3_rules,
        acts::step_4_1_4_3,
    );
    for (at, called) in cluster.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    let observed = Observed::from_ran(&cluster, &acts::reads_4_1_4_3());
    let mut misses = oracle::step_4_1(&observed);
    misses.extend(oracle::step_4_3(&observed));
    oracle::assert_met(&misses);
    let queued = &observed.documents[&acts::Read::BenchmarkWaiting];
    assert_eq!(queued.pointer("/task/chip/phase").and_then(serde_json::Value::as_str), Some("due"));
    for name in WORKERS {
        let worker = cluster.worker(name);
        assert_eq!(worker.end(), Some(&End::Drained), "{name} drained");
    }
    assert_eq!(cluster.worker("ws").ran(), &[case::DIRECTOR, case::CHECK], "ws ran both chats");
    assert_eq!(cluster.worker("s2").ran(), &[case::BENCHMARK], "s2 ran only the benchmark");
    // A parks between its wakes, so its activations' count is the engine's; the tasks are not.
    let s1 = cluster.worker("s1").ran();
    let opener = [case::A, case::CODER, case::REVIEW];
    let (last, earlier) = s1.split_last().expect("s1 ran tasks");
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
    assert!(cluster.killed().is_empty(), "cancellation was cooperative");
}
