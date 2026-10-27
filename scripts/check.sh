#!/bin/sh
# The gate (docs/development/workflow.md, section 1): formatting, lints as
# errors, then both test suites in full: the default one, then the fuzzy
# one. agents' `land` runs it through the machine's queue. Nothing slower:
# the browser tier is scripts/browser.sh, outside the gate (agents'
# docs/design/landing-flow.md, 4).
set -eu

cd "$(dirname "$0")/.."

# Each step's time, for the landing flow's reports (agents'
# docs/design/landing-flow.md, 10): `gate-step NAME SECONDS EXIT` on stderr.
step() {
    step_name=$1
    shift
    step_start=$(date +%s%N)
    step_exit=0
    "$@" || step_exit=$?
    step_ms=$((($(date +%s%N) - step_start) / 1000000))
    printf 'gate-step %s %d.%03d %d\n' "$step_name" $((step_ms / 1000)) $((step_ms % 1000)) "$step_exit" >&2
    return "$step_exit"
}
# The browser in the binary is a person's build; the gate's are without it (client/browser.md, 2.6).
export TEMPER_BUNDLE=none

# The lab is parked (docs/design/draft/qa-lab.md): its tests are outside the gate.
if grep -rnE '\b(task|watching|join|number|delegate|digest)\([0-9]+\)|calls\(\)\[[0-9]+\]' tests/real-loop/tests; then
    echo 'A setup written once, and rows carry data: name real-loop call positions.' >&2
    exit 1
fi

if grep -rnE 'assert_eq!\(\([a-z_]+\.exit, [a-z_]+\.exit|\.len\(\), [0-9]+[,)]' tests/real-loop/tests; then
    echo 'One behaviour per test: assert the act, and name real-loop sizes.' >&2
    exit 1
fi

# skein's programming-model.md, 2: every binary installs the fail-stop hook first.
step fail-stop python3 -I -B scripts/fail_stop.py
step scripts python3 -I -B -m unittest discover -s scripts -p 'test_*.py'
step fmt cargo fmt --check
step clippy cargo clippy --workspace --all-targets -- -D warnings
# The browser's client crates (client/browser.md, 4.3).
step wasm cargo clippy --target wasm32-unknown-unknown \
    -p temper-client-domain -p temper-client-view \
    -p temper-client-protocol-wire -p temper-client-shell-web -- -D warnings
step wasm cargo clippy --target wasm32-unknown-unknown \
    -p temper-client-domain -p temper-client-view \
    -p temper-client-protocol-wire -p temper-client-shell-web \
    --features loopback-navigation,record -- -D warnings
# skein's testing-strategy.md, section 8: a test is a process.
export NEXTEST_DOUBLE_SPAWN=0
step build cargo nextest run --workspace --no-run
step focused cargo nextest run --workspace --tool-config-file "gate:$PWD/.config/nextest-gate.toml"
step fuzzy cargo nextest run --workspace --profile fuzzy --tool-config-file "gate:$PWD/.config/nextest-gate.toml"
python3 -I -B scripts/gate_starved.py default fuzzy
