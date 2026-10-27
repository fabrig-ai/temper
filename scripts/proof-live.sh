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
    rung-b0) test_name=rung_b0_a_person_agrees_a_plan_before_the_change_lands ;;
    forge-land-claude) test_name=a_person_asks_claude_code_to_land_a_change_on_the_owned_forge ;;
    *) printf '%s\n' 'usage: scripts/proof-live.sh [forge-land|forge-land-claude|rung-b0]' >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { printf '%s\n' 'usage: scripts/proof-live.sh [forge-land|forge-land-claude|rung-b0]' >&2; exit 2; }
# The held directory is opened without following a link and listed once.
# Only its metadata and entry names are inspected; no entry is opened.
claude_identity=
if [ "${1-}" = forge-land-claude ]; then
    [ -n "${TEMPER_LIVE_CLAUDE-}" ] || { printf '%s\n' 'TEMPER_LIVE_CLAUDE is required for forge-land-claude' >&2; exit 2; }
    [ -z "${TEMPER_LIVE_ACCOUNT-}" ] || { printf '%s\n' 'TEMPER_LIVE_ACCOUNT is for forge-land' >&2; exit 2; }
    while [ "${TEMPER_LIVE_CLAUDE%/}" != "$TEMPER_LIVE_CLAUDE" ]; do
        TEMPER_LIVE_CLAUDE=${TEMPER_LIVE_CLAUDE%/}
    done
    claude_identity=$(python3 -I - "$TEMPER_LIVE_CLAUDE" <<'PYTHON'
import os
import stat
import sys

def refuse(message):
    print("TEMPER_LIVE_CLAUDE " + message, file=sys.stderr)
    sys.exit(2)

path = sys.argv[1].rstrip("/")
try:
    before = os.stat(path, follow_symlinks=False)
except OSError:
    refuse("names no directory")
if not stat.S_ISDIR(before.st_mode):
    refuse("is not a directory or is a link")
if stat.S_IMODE(before.st_mode) != 0o700:
    refuse("is not 0700")
if before.st_uid != os.geteuid():
    refuse("is not owned by this user")
name = os.path.basename(path)
if not 1 <= len(name) <= 32 or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in name):
    refuse("has no valid source name (1 to 32 lowercase letters, digits or hyphens)")
try:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        held = os.fstat(fd)
        if (held.st_dev, held.st_ino, held.st_mode, held.st_uid) != (before.st_dev, before.st_ino, before.st_mode, before.st_uid):
            refuse("changed during admission")
        if not os.listdir(fd):
            refuse("is empty")
    finally:
        os.close(fd)
except OSError:
    refuse("cannot be opened as a directory without following a link")
print(f"{held.st_dev}:{held.st_ino}:{stat.S_IMODE(held.st_mode):o}:{held.st_uid}:directory")
PYTHON
    ) || exit 2
    TEMPER_LIVE_CLAUDE=$(readlink -f -- "$(dirname -- "$TEMPER_LIVE_CLAUDE")")/$(basename -- "$TEMPER_LIVE_CLAUDE")
    export TEMPER_LIVE_CLAUDE
elif [ -n "${TEMPER_LIVE_CLAUDE-}" ]; then
    printf '%s\n' 'TEMPER_LIVE_CLAUDE is for forge-land-claude' >&2
    exit 2
fi
store=
rung=false
rerun=false
if [ "${1-}" = forge-land ] || [ "${1-}" = forge-land-claude ] || [ "${1-}" = rung-b0 ]; then
    if [ "${1-}" = rung-b0 ]; then
        rung=true
        [ -n "${TEMPER_LIVE_ACCOUNT-}" ] || { printf '%s\n' 'TEMPER_LIVE_ACCOUNT is required for rung-b0' >&2; exit 2; }
        store=${TEMPER_LIVE_STORE:-$PWD/../rung-b0-$(date -u +%Y%m%dT%H%M%SZ)-$$}
        # Rung boxes and their retained evidence belong to this task directory.
        export TMPDIR=${TMPDIR:-$PWD/..}
        export TEMPER_LIVE_KEEP=${TEMPER_LIVE_KEEP:-$PWD/../live-evidence}
    else
        store=${TEMPER_LIVE_STORE:-$HOME/src/rust/tmp/live-store/$1}
    fi
    if [ -e "$store" ] || [ -L "$store" ]; then
        if [ ! -d "$store" ] || [ -n "$(ls -A -- "$store")" ]; then
            rerun=true
            printf 'rerunning on the kept deployment in %s; needs no person\n' "$store"
        fi
    fi
    store_parent=$(dirname -- "$store")
    mkdir -p -- "$store_parent"
    store=$(readlink -f -- "$store")
    export TEMPER_LIVE_STORE="$store"
    if [ -n "${TEMPER_LIVE_ACCOUNT-}" ]; then
        # A held record moved in by ctl account import (qa-live.md, 4.3), which removes it.
        [ "$rerun" = false ] || { printf '%s\n' 'a held account moves into a fresh store only' >&2; exit 2; }
        # Its path goes to ctl account import alone, never into this script's words.
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
    printf '%s\n' 'TEMPER_LIVE_ACCOUNT is for forge-land or rung-b0' >&2
    exit 2
fi
export NEXTEST_DOUBLE_SPAWN=0
export TEMPER_BUNDLE=${TEMPER_BUNDLE:-none}
scratch=$(readlink -f "$(mktemp -d "${TMPDIR:-/tmp}/temper-live.XXXXXX")")
# A kept deployment may already have processes. Only this sitting's new
# processes belong to its cleanup, even when its box refuses a busy store.
# Every process whose cwd is in this newly created private scratch was started
# by this sitting. As in jig's live-sign-in.sh, stop those owned descendants
# before keeping or erasing their stores, including after an interruption.
# Called by the EXIT trap below.
# shellcheck disable=SC2317
stop_owned() {
    [ "$launched" = true ] || return 0
    [ -d "$scratch" ] || return 0
    owned=
    for process in /proc/[0-9]*; do
        cwd=$(readlink "$process/cwd" 2>/dev/null || true)
        case $cwd in
            "$scratch"/*) owned="$owned ${process#/proc/}" ;;
            *)
                if [ -n "$store" ]; then
                    case $cwd in
                        "$store"|"$store"/*)
                            case " $store_before " in
                                *" ${process#/proc/} "*) ;;
                                *) owned="$owned ${process#/proc/}" ;;
                            esac
                            ;;
                    esac
                fi
                ;;
        esac
    done
    for process in $owned; do kill -KILL "$process" 2>/dev/null || true; done
    attempt=0
    left=
    while [ "$attempt" -lt 100 ]; do
        left=
        for process in $owned; do
            if kill -0 "$process" 2>/dev/null; then left="$left $process"; fi
        done
        [ -n "$left" ] || break
        owned=$left
        attempt=$((attempt + 1))
        sleep 0.1
    done
    [ -z "$left" ]
}
notice=$scratch/notice
: > "$notice"
chmod 600 "$notice"
export TEMPER_LIVE_NOTICE="$notice"
launched=false
runner=
follower=
# shellcheck disable=SC2317
finish() {
    status=$?
    trap - EXIT INT TERM HUP
    set +e
    # The rung installs this before account import. Return while its box is
    # still alive, including when nextest or this script received a signal.
    # A failed return leaves the deployment alive for the owner's recovery.
    if [ "$rung" = true ] && [ -x "$store/give-back" ]; then
        if ! timeout 60 "$store/give-back" >> "$notice" 2>&1; then
            printf '%s\n' 'LOGIN NOT BACK: export/stat failed; box retained for recovery' >&2
            printf 'return hook %s/give-back\n' "$store" >&2
            exit 1
        fi
    fi
    # This also covers a failed build or a signal before import/hook creation.
    if [ "$rung" = true ]; then
        if [ "$(LC_ALL=C stat -c %F -- "$TEMPER_LIVE_ACCOUNT" 2>/dev/null)" != 'regular file' ] ||
           [ "$(stat -c %a -- "$TEMPER_LIVE_ACCOUNT" 2>/dev/null)" != 600 ] ||
           ! [ "$(stat -c %s -- "$TEMPER_LIVE_ACCOUNT" 2>/dev/null)" -gt 0 ]; then
            printf '%s\n' 'LOGIN NOT BACK: final stat failed; box retained for recovery' >&2
            exit 1
        fi
        stat -c 'LOGIN BACK: %F mode=%a bytes=%s owner=%u' -- "$TEMPER_LIVE_ACCOUNT" >> "$notice"
    fi
    if [ -n "$runner" ]; then
        kill -TERM "$runner" 2>/dev/null
        wait "$runner" 2>/dev/null
    fi
    if [ -n "$follower" ]; then
        kill "$follower" 2>/dev/null
        wait "$follower" 2>/dev/null
    fi
    stopped=true
    stop_owned || { stopped=false; status=1; }
    if [ -n "$claude_identity" ]; then
        returned=$(LC_ALL=C stat -c '%d:%i:%a:%u:%F' -- "$TEMPER_LIVE_CLAUDE" 2>/dev/null)
        if [ "$stopped" = true ] && [ "$returned" = "$claude_identity" ]; then
            printf '%s\n' 'CLAUDE LOGIN BACK: box stopped; same private directory, inode and owner; used in place' >> "$notice"
        else
            printf '%s\n' 'CLAUDE LOGIN NOT BACK: deployment did not stop or held directory metadata changed' >> "$notice"
            status=1
        fi
    fi
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
# A build failure or a signal also returns through finish, after stopping the box.
# shellcheck disable=SC2086
$heavy cargo nextest run -p temper --profile live --no-run
if [ "$rerun" = false ] && [ -z "${TEMPER_LIVE_ACCOUNT-}" ] && [ -z "$claude_identity" ]; then
    printf '%s\n' 'Approve the printed ChatGPT address in a browser that reaches this machine.' \
        'For a remote browser: ssh -L 1455:127.0.0.1:1455 -L <client port>:127.0.0.1:<client port> HOST'
fi
store_before=
if [ -n "$store" ] && [ -d "$store" ]; then
    for process in /proc/[0-9]*; do
        case $(readlink "$process/cwd" 2>/dev/null || true) in
            "$store"|"$store"/*) store_before="$store_before ${process#/proc/}" ;;
        esac
    done
fi
launched=true
# shellcheck disable=SC2086
TMPDIR=$scratch $heavy cargo nextest run -p temper --profile live --run-ignored only --no-capture \
    -E "test(=$test_name)" &
runner=$!
tail -n +1 -f --sleep-interval=0.2 --pid="$runner" "$notice" &
follower=$!
status=0
wait "$runner" || status=$?
runner=
# Let tail flush notices appended just before the run returned.
wait "$follower" || true
follower=
exit "$status"
