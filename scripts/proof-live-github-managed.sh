#!/bin/sh
# Owner-run managed GitHub sitting; implementers run fake only.
set -eu
umask 077
# Preserve the person's answers before the build wrapper can read stdin.
exec 3<&0
cd "$(dirname "$0")/.."
# The machine's build queue where it has one, plain cargo where it hasn't
# (agents' docs/design/build-infrastructure.md 12).
heavy=$(command -v heavy || true)
case ${1:-} in live|fake|fake-wide) mode=$1 ;; *) printf '%s\n' 'usage: proof-live-github-managed.sh {live|fake|fake-wide} [PORT [CONVERSION.json]]' >&2; exit 2 ;; esac
[ "$#" -le 3 ] || exit 2
port=${2:-8765}
case $port in ''|*[!0-9]*) exit 2 ;; esac
[ "$port" -ge 1 ] && [ "$port" -le 65535 ] || exit 2
export CARGO_NET_OFFLINE=true TEMPER_BUNDLE=none
export CARGO_TARGET_DIR=/home/free/src/rust/tmp/owners/github-managed/target NEXTEST_DOUBLE_SPAWN=0
scratch=$(readlink -f "$(mktemp -d "${TMPDIR:-/tmp}/temper-live-github-managed.XXXXXX")")
runner=
cleanup() {
    status=$?
    trap - EXIT INT TERM HUP
    : > "$scratch/stop-runner"
    [ -z "$runner" ] || wait "$runner" 2>/dev/null || true
    rm -rf -- "$scratch"
    printf '%s\n' 'scratch wiped'
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
if [ "$mode" = live ]; then
    evidence=${3:-/home/free/src/rust/tmp/owners/github-managed/reports/m3-conversion.json}
else
    evidence=${3:-$scratch.conversion.json}
fi
# create_new in the relay protects existing evidence, including symlinks.
if [ -e "$evidence" ] || [ -L "$evidence" ]; then
    printf '%s\n' 'conversion evidence path already exists' >&2
    exit 2
fi
printf '%s\n' 'building the sitting offline'
# shellcheck disable=SC2086
if ! $heavy cargo test --locked -p temper --test live_github_managed --no-run --message-format=json < /dev/null > "$scratch/build.jsonl" 2> "$scratch/build.log"; then
    # Compilation has no sitting credentials. Surface its diagnostics before cleanup.
    python3 - "$scratch/build.jsonl" <<'PYBUILD'
import json
import sys
for line in open(sys.argv[1]):
    if line.startswith('{'):
        row = json.loads(line)
        if row.get('reason') == 'compiler-message':
            print(row['message']['rendered'], file=sys.stderr)
PYBUILD
    tail -n 15 "$scratch/build.log" >&2
    exit 1
fi
if [ "$mode" = live ]; then
    # Keep TEMPER_BUNDLE=none; build a separate, explicit browser bundle in this scratch.
    printf '%s\n' 'building the browser bundle offline'
    # shellcheck disable=SC2086
    if ! $heavy cargo build --locked --profile web --target wasm32-unknown-unknown -p temper-client-shell-web < /dev/null > "$scratch/web-build.log" 2>&1; then
        tail -n 15 "$scratch/web-build.log" >&2
        exit 1
    fi
    # shellcheck disable=SC2086
    if ! $heavy cargo build --locked -p skein-ui-bundle < /dev/null > "$scratch/bundler-build.log" 2>&1; then
        tail -n 15 "$scratch/bundler-build.log" >&2
        exit 1
    fi
    "$CARGO_TARGET_DIR/debug/skein-ui-bundle" \
        --module "$CARGO_TARGET_DIR/wasm32-unknown-unknown/web/temper_client_shell_web.wasm" \
        --name temper_client_shell_web --prefix /bundle/ \
        --stylesheet crates/temper-client-shell-web/temper.css \
        --stylesheet crates/temper-client-shell-web/task-pages.css \
        --files crates/temper-client-shell-web/files \
        --noscript 'temper needs WebAssembly and scripts.' --out "$scratch/bundle" > "$scratch/bundle.log" 2>&1
fi
# A background job's stdin is /dev/null when job control is off (POSIX): keep the person's on fd 3.
python3 scripts/owned-process.py --stop-file "$scratch/stop-runner" -- \
    python3 scripts/proof-live-github-managed.py "$mode" "$port" "$scratch" "$evidence" <&3 3<&- &
runner=$!
wait "$runner"
runner=
