#!/bin/sh
# A person's sitting (domain/testing.md, 2.7): fresh ChatGPT sign-in and real stories.
set -eu
umask 077
cd "$(dirname "$0")/.."
# The machine's build queue where it has one, plain cargo where it hasn't
# (agents' docs/design/build-infrastructure.md 12).
heavy=$(command -v heavy || true)
case ${1-} in
    '') test_name=a_real_model_answers_a_chat_and_pushes_a_small_change ;;
    forge-land) test_name=a_person_asks_chatgpt_to_land_a_change_on_the_owned_forge ;;
    *) printf '%s\n' 'usage: scripts/proof-live.sh [forge-land]' >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { printf '%s\n' 'usage: scripts/proof-live.sh [forge-land]' >&2; exit 2; }
store=
rerun=false
if [ "${1-}" = forge-land ]; then
    store=${TEMPER_LIVE_STORE:-$HOME/src/rust/tmp/live-store/forge-land}
    if [ -e "$store" ] || [ -L "$store" ]; then
        if [ ! -d "$store" ] || [ -n "$(ls -A -- "$store")" ]; then
            rerun=true
            printf 'rerunning on the kept ChatGPT sign-in in %s; needs no person\n' "$store"
        fi
    fi
    store_parent=$(dirname -- "$store")
    mkdir -p -- "$store_parent"
    store=$(readlink -f -- "$store")
    export TEMPER_LIVE_STORE=$store
    if [ -n "${TEMPER_LIVE_ACCOUNT-}" ]; then
        # A held record moved in by ctl import (qa-live.md, 4.3), which removes it.
        [ "$rerun" = false ] || { printf '%s\n' 'a held account moves into a fresh store only' >&2; exit 2; }
        # Its path goes to ctl import alone, never into this script's words.
        if [ -L "$TEMPER_LIVE_ACCOUNT" ] || [ ! -f "$TEMPER_LIVE_ACCOUNT" ]; then
            printf '%s\n' 'TEMPER_LIVE_ACCOUNT names no regular file' >&2
            exit 2
        fi
        [ "$(stat -c %a -- "$TEMPER_LIVE_ACCOUNT")" = 600 ] || { printf '%s\n' 'TEMPER_LIVE_ACCOUNT is not 0600' >&2; exit 2; }
        TEMPER_LIVE_ACCOUNT=$(readlink -f -- "$TEMPER_LIVE_ACCOUNT")
        export TEMPER_LIVE_ACCOUNT
        printf '%s\n' 'moving the held ChatGPT account in; needs no person'
    fi
elif [ -n "${TEMPER_LIVE_ACCOUNT-}" ]; then
    printf '%s\n' 'TEMPER_LIVE_ACCOUNT is for forge-land' >&2
    exit 2
fi
export NEXTEST_DOUBLE_SPAWN=0
# shellcheck disable=SC2086
$heavy cargo nextest run -p temper --profile live --no-run
scratch=$(readlink -f "$(mktemp -d "${TMPDIR:-/tmp}/temper-live.XXXXXX")")
notice=$scratch/notice
: > "$notice"
chmod 600 "$notice"
export TEMPER_LIVE_NOTICE=$notice
runner=
follower=
finish() {
    status=$?
    trap - EXIT INT TERM HUP
    set +e
    : > "$scratch/stop-runner"
    [ -z "$runner" ] || wait "$runner" 2>/dev/null
    : > "$scratch/stop-follower"
    [ -z "$follower" ] || wait "$follower" 2>/dev/null
    printf '%s\n' 'Last live notices:'
    tail -n 30 "$notice"
    keep=${TEMPER_LIVE_KEEP:-$HOME/src/rust/tmp/live-evidence}
    tag=$(sed -n 's/^TAG \([^; ]*\).*$/\1/p' "$notice" | head -n 1 | tr -cd 'A-Za-z0-9_.:-')
    [ -n "$tag" ] || tag=$(date -u +%Y-%m-%dT%H:%M:%SZ)-$$
    if mkdir -p -- "$keep" && chmod 700 "$keep"; then
        # The kept notice never carries the transient CLIENT bearer.
        kept=$keep/$tag.notice
        if sed 's/secret=[^&[:space:]]*/secret=…/g' "$notice" > "$kept" && chmod 600 "$kept"; then
            printf 'notice %s\n' "$kept"
        else
            printf 'could not retain notice in %s\n' "$keep" >&2
            status=1
        fi
    else
        printf 'could not create private evidence directory %s\n' "$keep" >&2
        status=1
    fi
    if [ "$status" -eq 0 ]; then
        rm -rf -- "$scratch"
    else
        # Retain scratch evidence, and redact the transient link there too.
        sed 's/secret=[^&[:space:]]*/secret=…/g' "$notice" > "$scratch/notice.redacted"
        mv -- "$scratch/notice.redacted" "$notice"
        printf 'kept %s; erase it with rm -rf -- %s once read\n' "$scratch" "$scratch"
    fi
    [ -z "$store" ] || printf 'kept store %s\n' "$store"
    exit "$status"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
if [ "$rerun" = false ] && [ -z "${TEMPER_LIVE_ACCOUNT-}" ]; then
    printf '%s\n' 'Approve the printed ChatGPT address in a browser that reaches this machine.' \
        'For a remote browser: ssh -L 1455:127.0.0.1:1455 -L <client port>:127.0.0.1:<client port> HOST'
fi
# shellcheck disable=SC2086
TMPDIR=$scratch python3 scripts/owned-process.py --stop-file "$scratch/stop-runner" -- $heavy cargo nextest run -p temper --profile live --run-ignored only --no-capture \
    -E "test(=$test_name)" &
runner=$!
python3 scripts/owned-process.py --stop-file "$scratch/stop-follower" -- \
    tail -n +1 -f --sleep-interval=0.2 "$notice" &
follower=$!
status=0
wait "$runner" || status=$?
runner=
# The runner has drained its tree; stop the separate notice follower by request.
: > "$scratch/stop-follower"
wait "$follower" || true
follower=
exit "$status"
