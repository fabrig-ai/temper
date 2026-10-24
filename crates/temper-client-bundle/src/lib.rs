//! The browser's bundle as temper's build made it (client/browser.md, 2.6): data, no code but the include.
//! `FILES` holds each file's path from the bundle's root (`index.html`, then `<digest>/...`) and its bytes,
//! in a directory read's order; `DIGEST` names the digest directory. Built with `TEMPER_BUNDLE=none`, `FILES`
//! is empty and `DIGEST` is `None`.
#![no_std]

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));
