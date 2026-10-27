//! Retained first-edition story (domain/testing.md, 5.3): one deployment repair before the waiting change.
use temper_real_loop_world::{Exit, case, run_cluster_forge};

#[test]
fn a_red_tip_pauses_kits_queue_for_one_repair_that_lands_before_a4_while_the_directors_interim_timer_waits_without_adopting_a4()
 {
    // A1 is delegated directly, tracked as a goal; A is omitted. One
    // repository stands for kit/app until 6.6. The person's red push stands
    // for app's slow tier, and a1/a2's landings are omitted. Adopting a4 is
    // design (temper-on-temper.md, 6.8): the deployment repairs separately.
    // Ninety minutes is represented by a far-future timer, read armed and
    // removed, never waited out. One repair restores CI; exhaustion of the
    // repairs bound belongs to the lower world. All waits follow events.
    // The product's first needs a4's head CI and blocking review passed:
    // release a4's review after the red push and before repair creation.
    // Hold the repair's coder while
    // the director arms its timer and the paused change is read.
    // Exact-head freshness asks another a4 review after its clean update;
    // head cues hold only the first review. Coders write and finish in one
    // response. Director/A1 delegation and the timer subscription share
    // completions with their waits, serviced serially by one call slot.
    // Coder progress uses ctl events; creation and held-writing reads stay
    // on exact document fields. Reuse repair reads instead of reading twice.
    // Timer acknowledgement precedes A1's report even within one query.
    // The armed activity prefix completes the ended journal page.
    // Owners/changes use 5.4's six grants and its change/hold helpers.
    // F9, the pause's news to waiting goals (forge.md, 9), is missing on main:
    // crates/temper-engine-domain/src/route/forge_route.rs:1958
    // start_queue_repair notifies no goal.
    // crates/temper-engine-domain-forge/src/boundary.rs:484 News has no pause event.
    // A1's actual a4 result is checked.
    // a4 writes its own file: its pre-push head keeps the green cue, and
    // updating onto the repaired main needs no conflict coder.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_4(),
        case::scripts::step_6_4,
        case::scripts::step_6_4_rules,
        case::acts::step_6_4,
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
    let observed = case::oracle::Observed::from_ran(&ran, case::acts::READS_6_4);
    case::oracle::assert_met(&case::oracle::step_6_4(&observed));
}
