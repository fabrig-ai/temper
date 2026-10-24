//! The browser's bundle as temper's build made it (client/browser.md, 2.6): data, no code but the include.
//! `FILES` holds each file's path from the bundle's root (`index.html`, then `<digest>/...`) and its bytes,
//! in a directory read's order; `DIGEST` names the digest directory. Built with `TEMPER_BUNDLE=none`, `FILES`
//! is empty and `DIGEST` is `None`.
//! `loopback` holds a separate bundle when the consumer enables `loopback-navigation` and the build
//! embeds it; otherwise its files are empty and its digest is `None` too.
#![no_std]

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));

/// The separately packaged browser for the fake-backend configuration (protocol/configuration.md, 12.1).
pub mod loopback {
    include!(concat!(env!("OUT_DIR"), "/loopback.rs"));
}
