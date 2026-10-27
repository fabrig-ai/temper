//! Gregorian civil date from epoch days, with March-based months and 400-year eras.
use super::Text;
use skein_lib::Wall;
#[expect(
    clippy::arithmetic_side_effects,
    reason = "u64 nanoseconds bound epoch days to 213503; calendar intermediates fit u64"
)]
fn civil(wall: Wall) -> (u64, u64, u64) {
    let days = wall.as_secs() / 86_400 + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year + year / 4 - year / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = if march_month < 10 { march_month + 3 } else { march_month - 9 };
    (year + era * 400 + u64::from(month <= 2), month, day)
}
pub(super) fn deadline(wall: Wall, out: &mut Text) {
    let (year, month, day) = civil(wall);
    out.number(year);
    out.put(b"-");
    two_digits(month, out);
    out.put(b"-");
    two_digits(day, out);
    out.put(b" ");
    super::utc(wall, out);
    out.put(b" UTC");
}
pub(super) fn minute(wall: Wall, out: &mut Text) {
    let (year, month, day) = civil(wall);
    out.number(year);
    out.put(b"-");
    two_digits(month, out);
    out.put(b"-");
    two_digits(day, out);
    out.put(b"T");
    super::utc(wall, out);
    out.put(b"Z");
}
fn two_digits(number: u64, out: &mut Text) {
    if number < 10 {
        out.put(b"0");
    }
    out.number(number);
}
#[cfg(test)]
mod tests {
    use super::civil;
    use skein_lib::Wall;
    #[test]
    fn epoch_and_leap_year_boundaries_have_civil_dates() {
        for (seconds, expected) in [
            (0_u64, (1970, 1, 1)),
            (951_782_399, (2000, 2, 28)),
            (951_782_400, (2000, 2, 29)),
            (951_868_800, (2000, 3, 1)),
            (978_307_200, (2001, 1, 1)),
        ] {
            assert_eq!(civil(Wall::from_nanos(seconds.checked_mul(1_000_000_000).expect("wall fits"))), expected);
        }
    }
}
