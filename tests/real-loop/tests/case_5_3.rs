//! Retained first-edition story (domain/testing.md, 5.3): A's amended instructions, waiting on jig for a deadline's hold.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn an_owners_amended_instructions_reach_its_next_run_without_a_deadlines_hold_or_the_amendments_history() {
    // Leaves out the deadline's hold: jig never enforces a budget deadline on
    // a running task (jig's crates/jig-domain-tasks/src/domain.rs:698 and
    // crates/jig-domain-core/src/run.rs:121), in jig's backlog.
    // Leaves out the amendment history's read: jig's History keeps the task's
    // settings override before and after (crates/jig-domain-tasks/src/control.rs:698,
    // save_history), so when the brief comes from the charter, as here, "before"
    // records no override, not the old text; its store serves Scope::History
    // (crates/jig-protocol-store/src/reads.rs:59), but no temper ctl command
    // reads it (crates/temper-client-tools/src/table.rs:8;
    // crates/temper-client-tools/src/jig/render.rs:55 maps ReadValue::History to
    // nothing). A ctl read alone wouldn't show what was replaced; this story
    // covers it only by what A was told before and after. No x3 is delegated, and the two hours remain words. Also leaves
    // out the retained first edition's charter run policy (domain/testing.md, 5.3):
    // a charter changes only in config, across a restart; this amends A's task.
    // Pruning A's notes into an archive is A's own work (domain/testing.md, 5.3).
    let ran = run_cluster_forge(
        &["ws"],
        &[],
        case::fixture_5_3(),
        case::scripts::step_5_3,
        || Box::new([]),
        case::acts::step_5_3,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_5_3);
    oracle::assert_met(&oracle::step_5_3(&observed));
}
