use super::*;

const HEAD: Commit = [3; 32];
const BOUND: u32 = 512;
fn check(context: &[u8], check: Check) -> Status {
    Status {
        at: skein_lib::Time::from_nanos(0),
        context: context.into(),
        check,
        description: Box::new([]),
        url: Box::new([]),
        job: None,
    }
}
#[test]
fn a_landed_result_names_two_checks_one_ci_repair_and_its_approved_review() {
    let mut result = ChangeResult::empty();
    result.checked([2; 32], &[check(b"release note", Check::Failed)], BOUND);
    result.repaired(change::Repair::Ci, Some([2; 32]), BOUND);
    result.checked(HEAD, &[check(b"build", Check::Passed), check(b"release note", Check::Passed)], BOUND);
    result.reviewed(&HEAD, b"approved", BOUND);
    let expected = "checks at 0303030303030303030303030303030303030303030303030303030303030303: build passed; release note passed\nrepairs: 1: CI failed: release note\nreviews: approved at 0303030303030303030303030303030303030303030303030303030303030303";
    assert_eq!(result.words(&HEAD, BOUND).fitted(BOUND).unwrap().as_ref(), expected.as_bytes());
}
#[test]
fn a_landed_result_with_no_repairs_names_no_unread_checks() {
    let mut result = ChangeResult::empty();
    result.checked([2; 32], &[check(b"old tests", Check::Passed)], BOUND);
    let expected =
        "checks at 0303030303030303030303030303030303030303030303030303030303030303: none\nrepairs: 0\nreviews: none";
    assert_eq!(result.words(&HEAD, BOUND).fitted(BOUND).unwrap().as_ref(), expected.as_bytes());
}
#[test]
fn the_result_marks_the_exact_cut_including_history_omitted_before_landing() {
    let mut result = ChangeResult::empty();
    result.reviewed(&HEAD, b"changes requested", 40);
    result.repaired(change::Repair::Gate(1), Some(HEAD), BOUND);
    result.reviewed(&HEAD, b"approved", 40);
    let whole = b"checks at 0303030303030303030303030303030303030303030303030303030303030303: none\nrepairs: 1: review asked for changes\nreviews: changes requested at 0303030303030303030303030303030303030303030303030303030303030303; approved at 0303030303030303030303030303030303030303030303030303030303030303";
    for bound in [100, 160, 512] {
        let words = result.words(&HEAD, bound).fitted(bound).unwrap();
        let mut kept = None;
        for (at, pair) in words.windows(2).enumerate() {
            if pair == b"\n[" {
                kept = Some(at);
            }
        }
        let kept = kept.expect("cut marker");
        assert!(whole.starts_with(&words[..kept]));
        let lost = Decimal::of(u64::try_from(whole.len() - kept).unwrap());
        assert_eq!(&words[kept + 2..words.len() - b" bytes cut]\n".len()], lost.as_bytes());
        assert!(words.ends_with(b" bytes cut]\n"));
        assert!(words.len() <= usize::try_from(bound).unwrap());
    }
}
#[test]
fn every_review_verdict_keeps_its_own_head_and_order() {
    let mut result = ChangeResult::empty();
    for (head, verdict) in
        [(HEAD, b"failed".as_slice()), ([2; 32], b"cancelled"), (HEAD, b"changes requested"), ([2; 32], b"approved")]
    {
        result.reviewed(&head, verdict, BOUND);
    }
    assert_eq!(result.reviews.bytes.as_ref(), b"failed at 0303030303030303030303030303030303030303030303030303030303030303; cancelled at 0202020202020202020202020202020202020202020202020202020202020202; changes requested at 0303030303030303030303030303030303030303030303030303030303030303; approved at 0202020202020202020202020202020202020202020202020202020202020202");
}

#[test]
fn an_unread_failed_head_never_borrows_a_context_from_an_older_head() {
    let mut result = ChangeResult::empty();
    result.checked([2; 32], &[check(b"old check", Check::Failed)], BOUND);
    result.repaired(change::Repair::Ci, Some(HEAD), BOUND);
    assert_eq!(result.repairs.bytes.as_ref(), b"CI failed");
}
