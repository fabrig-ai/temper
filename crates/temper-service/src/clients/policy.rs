//! The clients' listener's page policy on every answer (client/browser.md, 2.2; jig's protocol/browser.md, 12.1).
//! Copied from jig's testing/jig-test-service/src/server/policy.rs.
use alloc::boxed::Box;
use skein_http::{Header, server::Response};
use skein_lib::{List, bytes};

/// jig's reference policy, on every answer of the clients' listener.
pub const FIELDS: [(&[u8], &[u8]); 5] = [
    (b"Content-Security-Policy", b"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"),
    (b"X-Content-Type-Options", b"nosniff"),
    (b"Cross-Origin-Opener-Policy", b"same-origin"),
    (b"Cross-Origin-Resource-Policy", b"same-origin"),
    (b"Referrer-Policy", b"no-referrer"),
];

/// Append each field the response does not hold, its name compared ignoring case.
pub(crate) fn add(response: &mut Response) {
    let capacity = u32::try_from(response.headers.len()).expect("bounded response headers");
    let mut headers = List::with_capacity(capacity.checked_add(5).expect("policy fields"));
    for header in core::mem::replace(&mut response.headers, Box::new([])) {
        headers.push(header).expect("existing fields");
    }
    for (name, value) in FIELDS {
        let mut present = false;
        for header in &headers {
            present |= header.name.eq_ignore_ascii_case(name);
        }
        if !present {
            headers.push(Header { name: bytes::copy_of(name), value: bytes::copy_of(value) }).expect("policy room");
        }
    }
    response.headers = headers.into_boxed();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::disallowed_macros, reason = "compare the static hosting template with the service's policy")]
    fn the_playground_headers_are_the_page_policy_with_workers() {
        use crate::clients::bundle::{FILE_CACHE_CONTROL, PAGE_CACHE_CONTROL};

        let template = include_str!("../../../temper-client-shell-web/playground/headers");
        let mut headers = List::with_capacity(7);
        for line in template.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (path, field) = line.split_once(' ').expect("PATH NAME: VALUE");
            let (name, value) = field.split_once(": ").expect("NAME: VALUE");
            headers.push((path, name.as_bytes(), value.as_bytes())).expect("only policy and cache fields");
        }
        assert_eq!(headers.len(), 7, "exactly five policy fields and two cache fields");
        for (at, (name, value)) in FIELDS.iter().enumerate() {
            let (path, actual_name, actual_value) = headers.get(u32::try_from(at).unwrap()).expect("policy field");
            assert_eq!(*path, "/*");
            assert_eq!(actual_name, name);
            if *name == b"Content-Security-Policy" {
                assert_eq!(actual_value.strip_suffix(b"; worker-src 'self'"), Some(*value));
            } else {
                assert_eq!(actual_value, value);
            }
        }
        assert_eq!(headers.get(5), Some(&("/*", b"Cache-Control".as_slice(), PAGE_CACHE_CONTROL)));
        assert_eq!(headers.get(6), Some(&("{bundle}*", b"Cache-Control".as_slice(), FILE_CACHE_CONTROL)));
    }
}
