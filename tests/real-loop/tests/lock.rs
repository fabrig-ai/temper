//! Two native acts serialize on the real directory lock.
use skein_world::domain::heap::Counting;
use temper_real_loop_world::{
    Exit, argv,
    calls::{D1, contended, join, probe, release, sign_in, start, task, until},
    fixtures, run, scripts,
};
#[global_allocator]
static HEAP: Counting = Counting;
// Two calls must contend together; a single call cannot expose overlapping ownership.
const ACTS: usize = 2;
// Sign-in is call zero, so the first competing act occupies the next call.
const FIRST: usize = 1;
// The second competing act follows the first, before either read.
const SECOND: usize = 2;
// Distinct keys ask for two tasks rather than replaying one act.
const FIRST_NAME: &str = "first";
// This key must differ from the first competing act's key.
const SECOND_NAME: &str = "second";
#[test]
fn two_acts_on_one_directory_take_its_lock_in_turn() {
    let ran = run(
        || scripts::chat_scripts(ACTS),
        || {
            vec![
                sign_in(D1),
                probe(D1),
                start(D1, argv!["chat", "start", "--project", fixtures::PROJECT, "--key", FIRST_NAME, scripts::WORDS]),
                start(D1, argv!["chat", "start", "--project", fixtures::PROJECT, "--key", SECOND_NAME, scripts::WORDS]),
                contended(&[FIRST, SECOND]),
                release(D1),
                join(FIRST),
                join(SECOND),
                until(D1, argv!["chat", "show", task(FIRST)], scripts::REPLY, scripts::EARLY),
                until(D1, argv!["chat", "show", task(SECOND)], scripts::REPLY, scripts::EARLY),
            ]
        },
    );
    let (one, two) = (&ran.calls()[FIRST], &ran.calls()[SECOND]);
    assert_eq!([one.exit, two.exit], [Some(Exit::Code(0)); ACTS], "{}", String::from_utf8_lossy(&two.stderr));
    let (a, b) = (one.locked.expect("one took the lock"), two.locked.expect("two took the lock"));
    let (x, y) = (one.released.expect("one closed its lock"), two.released.expect("two closed its lock"));
    assert!(x <= b || y <= a, "both held DIRECTORY's lock at once");
}
