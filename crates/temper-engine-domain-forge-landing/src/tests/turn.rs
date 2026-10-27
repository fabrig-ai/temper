use crate::{Proved, first};
use skein_lib::Wall;
#[test]
fn the_first_proved_takes_the_turn() {
    let proved = [
        Proved { task: 1, at: Wall::from_nanos(9) },
        Proved { task: 3, at: Wall::from_nanos(4) },
        Proved { task: 2, at: Wall::from_nanos(5) },
    ];
    assert_eq!(first(&proved), Some(3));
}
#[test]
fn a_tie_goes_to_the_lower_task() {
    let mut proved = [Proved { task: 9, at: Wall::from_nanos(4) }, Proved { task: 2, at: Wall::from_nanos(4) }];
    assert_eq!(first(&proved), Some(2));
    proved.reverse();
    assert_eq!(first(&proved), Some(2));
}
#[test]
fn none_proved_gives_none() {
    assert_eq!(first(&[]), None);
}
