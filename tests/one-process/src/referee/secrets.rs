//! Secrets are checked in root formatting, stored bytes and actual agent environments.
use crate::fixtures;
use std::fmt;

/// The issued value's name, never the value itself.
#[must_use]
pub fn in_bytes(bytes: &[u8]) -> Option<&'static str> {
    // The issued fixture tokens and their Debug representations are ASCII.
    // Lossy decoding preserves every ASCII run, even in binary store frames.
    let text = String::from_utf8_lossy(bytes);
    for (name, token) in [
        ("LLM access token", fixtures::LLM_TOKEN),
        ("git identity token", fixtures::GIT_TOKEN),
        ("forge API token", fixtures::API_TOKEN),
    ]
    .into_iter()
    .map(|(name, bytes)| (name, bytes.to_vec().into_boxed_slice()))
    .chain(crate::replay::issued())
    {
        let rendered = format!("{token:?}");
        if text.contains(std::str::from_utf8(&token).expect("ASCII fixture token")) || text.contains(&rendered) {
            return Some(name);
        }
    }
    None
}
pub(crate) struct Roots<'a>(pub &'a temper_playground::Service);
impl fmt::Debug for Roots<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_roots(formatter)
    }
}
pub(crate) fn require(bytes: &[u8], place: &str) {
    assert!(in_bytes(bytes).is_none(), "{} in {place}", in_bytes(bytes).unwrap_or("issued secret"));
}
pub(crate) fn require_formatted(text: &str, place: &str) {
    for (name, token) in [
        ("LLM access token", fixtures::LLM_TOKEN),
        ("git identity token", fixtures::GIT_TOKEN),
        ("forge API token", fixtures::API_TOKEN),
    ]
    .into_iter()
    .map(|(name, bytes)| (name, bytes.to_vec().into_boxed_slice()))
    .chain(crate::replay::issued())
    {
        assert!(
            !text.contains(std::str::from_utf8(&token).expect("fixture token"))
                && !text.contains(&format!("{token:?}")),
            "{name} in {place}"
        );
    }
}
