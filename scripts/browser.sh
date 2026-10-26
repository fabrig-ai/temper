#!/bin/sh
# The browser tier (client/browser.md, 4.3), outside the gate (agents'
# docs/design/landing-flow.md, 4): the bundle and temper, then Chromium
# against temper serve on that bundle. Its arguments go to nextest, so that
# one test runs alone: scripts/browser.sh -E 'test(=NAME)'.
set -eu

cd "$(dirname "$0")/.."
# The browser in the binary is a person's build; the gate's are without it (client/browser.md, 2.6).
export TEMPER_BUNDLE=none
# skein's testing-strategy.md, section 8: a test is a process.
export NEXTEST_DOUBLE_SPAWN=0

scripts/bundle.sh --loopback target/temper-bundle >/dev/null
scripts/bundle.sh --playground target/playground-bundle >/dev/null
cargo build -p temper
TEMPER_TEST_BUNDLE="$(pwd)/target/temper-bundle"
TEMPER_TEST_PLAYGROUND="$(pwd)/target/playground-bundle"
export TEMPER_TEST_BUNDLE TEMPER_TEST_PLAYGROUND
cargo nextest run --workspace --profile browser "$@"
