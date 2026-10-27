use crate::{Conclusion, Fault, Job, Run, RunState, Verdict, verdict};
use alloc::boxed::Box;
const HEAD: [u8; 32] = [1; 32];
fn run(conclusion: Option<Conclusion>, jobs: Box<[Job]>) -> Run {
    Run { commit: HEAD, state: RunState::Completed, conclusion, jobs, cancelled_by_temper: false }
}
fn job(failed: Option<u32>) -> Job {
    Job { conclusion: Some(Conclusion::Failure), failed, steps: 4 }
}
#[test]
fn run_and_step_facts_distinguish_real_failures_from_runner_faults() {
    let rows = [
        (Run { commit: [2; 32], ..run(Some(Conclusion::Success), Box::new([])) }, Verdict::Fault(Fault::OtherCommit)),
        (Run { state: RunState::Waiting, ..run(None, Box::new([])) }, Verdict::Running),
        (Run { state: RunState::Running, ..run(None, Box::new([])) }, Verdict::Running),
        (run(Some(Conclusion::Success), Box::new([])), Verdict::Passed),
        (run(Some(Conclusion::Cancelled), Box::new([])), Verdict::Fault(Fault::Cancelled)),
        (Run { cancelled_by_temper: true, ..run(Some(Conclusion::Cancelled), Box::new([])) }, Verdict::Running),
        (run(Some(Conclusion::Failure), Box::new([job(Some(1))])), Verdict::Failed),
        (run(Some(Conclusion::Failure), Box::new([job(Some(2))])), Verdict::Failed),
        (run(Some(Conclusion::Failure), Box::new([job(Some(0))])), Verdict::Fault(Fault::SetUp)),
        (run(Some(Conclusion::Failure), Box::new([job(Some(3))])), Verdict::Fault(Fault::Completion)),
        (run(Some(Conclusion::Failure), Box::new([job(None)])), Verdict::Fault(Fault::NoStep)),
        (run(Some(Conclusion::Failure), Box::new([job(Some(0)), job(Some(1))])), Verdict::Failed),
        (run(Some(Conclusion::Failure), Box::new([job(Some(3)), job(Some(0))])), Verdict::Fault(Fault::Completion)),
        (run(Some(Conclusion::Failure), Box::new([])), Verdict::Fault(Fault::NoStep)),
        (run(Some(Conclusion::Skipped), Box::new([])), Verdict::Fault(Fault::NoStep)),
        (run(None, Box::new([])), Verdict::Fault(Fault::NoStep)),
        (
            run(Some(Conclusion::Failure), Box::new([Job { conclusion: Some(Conclusion::Success), ..job(Some(1)) }])),
            Verdict::Fault(Fault::NoStep),
        ),
        (run(Some(Conclusion::Failure), Box::new([Job { steps: 0, ..job(Some(0)) }])), Verdict::Fault(Fault::NoStep)),
        (run(Some(Conclusion::Failure), Box::new([Job { steps: 1, ..job(Some(0)) }])), Verdict::Fault(Fault::SetUp)),
        (run(Some(Conclusion::Failure), Box::new([job(Some(4))])), Verdict::Fault(Fault::NoStep)),
    ];
    for (index, (run, expected)) in rows.iter().enumerate() {
        let actual = verdict(run, HEAD);
        assert_eq!(&actual, expected, "run/step table row {index}");
        assert_ne!(actual, Verdict::Fault(Fault::Lost), "only the top classifies a lost run");
    }
}
