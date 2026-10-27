//! Retained first-edition story (domain/testing.md, 5.3): Stop holds B and its delegates; one release resumes their work.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn a_stopped_owner_holds_its_delegates_and_its_release_resumes_them_and_its_news_waits_for_its_release_without_a_park_or_a_retired_charter()
 {
    // Leaves out the director parking B, temper-on-temper.md 6.2's design gap:
    // an agent's task tools delegate, amend, message, cancel and release, but
    // none stops a task (crates/temper-engine-smith/src/calls.rs:42-52), so the
    // director proposes and the person stops. Leaves out retiring the helper's
    // charter, 6.8 (b), missing: no charter is withdrawn while a deployment
    // runs, and across a restart a live task on a charter no longer configured
    // refuses the restore (jig's crates/jig-domain-tasks/src/stored.rs:656), so
    // the helper's cancel stands for it. Leaves out a second helper. A held
    // review stands for h1 in flight,
    // not already ready in kit's queue: this fixture's CI passes at once.
    // Nothing acts on the results until B's one release; the person's tour of
    // every owner's work is left out (domain/testing.md, 5.3's 5.2 row).
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_5_2(),
        case::scripts::step_5_2,
        case::scripts::step_5_2_rules,
        case::acts::step_5_2,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(
            called.exit,
            Some(Exit::Code(0)),
            "call {at}: {}\n{}",
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&called.stdout)
        );
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_5_2);
    oracle::assert_met(&oracle::step_5_2(&observed));
}
