//! UTF-8 projections fitted to forge byte bounds (domain/forge.md, 8.1 and 12).
use alloc::boxed::Box;
use skein_lib::Writer;

/// Keep UTF-8 words unchanged when they fit, otherwise append the supplied cut marker.
/// A bound smaller than the marker carries an ellipsis when it can fit one.
#[must_use]
pub fn fit(words: &[u8], bound: usize, marker: &[u8]) -> Box<[u8]> {
    if words.len() <= bound {
        return words.into();
    }
    let marker = if marker.len() <= bound {
        marker
    } else if bound >= 3 {
        "…".as_bytes()
    } else {
        return Box::new([]);
    };
    let mut end = bound.saturating_sub(marker.len()).min(words.len());
    while end > 0 {
        match words.get(end) {
            Some(byte) if byte & 0xc0 == 0x80 => end = end.checked_sub(1).expect("positive prefix length"),
            Some(_) | None => break,
        }
    }
    let mut out = Writer::new(end.checked_add(marker.len()).expect("bounded projection length"));
    out.put(words.get(..end).expect("fitted prefix length")).expect("fitted prefix");
    out.put(marker).expect("fitted marker");
    out.finish()
}
