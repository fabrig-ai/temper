#!/bin/sh
# Build the browser bundle (client/browser.md, 4.3; playground.md, 7).
set -eu
profile=debug
flag=
features=
playground=false
while [ "$#" -gt 0 ]; do
    case "$1" in
    --release) profile=release; flag=--release ;;
    --loopback) features="${features:+$features,}loopback-navigation" ;;
    --record) features="${features:+$features,}record" ;;
    --playground) playground=true ;;
    *) break ;;
    esac
    shift
done
if [ "$#" -ne 1 ]; then
    echo "usage: scripts/bundle.sh [--release] [--loopback] [--record] [--playground] OUT" >&2
    exit 2
fi
if "$playground" && [ -n "$features" ]; then
    echo 'scripts/bundle.sh: --playground refuses --loopback and --record' >&2
    exit 2
fi
out=$1
cd "$(dirname "$0")/.."
if [ -e "$out" ] && [ ! -e "$out/index.html" ]; then
    echo "scripts/bundle.sh: $out exists and holds no index.html" >&2
    exit 1
fi
shell=crates/temper-client-shell-web
noscript='temper needs WebAssembly and scripts.'
if "$playground"; then
    unset TEMPER_COMMIT
    if [ -z "$(git status --porcelain)" ]; then
        TEMPER_COMMIT=$(git rev-parse HEAD)
        export TEMPER_COMMIT
    else
        echo "scripts/bundle.sh: dirty tree; leaving TEMPER_COMMIT unset" >&2
    fi
    profile=web
    cargo build --target wasm32-unknown-unknown --profile web \
        -p temper-client-shell-web --features playground
    cargo build --target wasm32-unknown-unknown --profile playground \
        -p temper-playground
    set -- --worker-module "target/wasm32-unknown-unknown/playground/temper_playground.wasm" \
        --worker-name temper_playground --page "$shell/playground/index.html" --stylesheet "$shell/playground.css"
    noscript="temper's playground needs WebAssembly and scripts."
else
    cargo build --target wasm32-unknown-unknown $flag ${features:+--features "$features"} -p temper-client-shell-web
    set --
fi
cargo build -p skein-ui-bundle
target/debug/skein-ui-bundle \
    --module "target/wasm32-unknown-unknown/$profile/temper_client_shell_web.wasm" \
    --name temper_client_shell_web --prefix /bundle/ \
    --stylesheet "$shell/temper.css" \
    --stylesheet "$shell/task-pages.css" \
    --files "$shell/files" \
    --noscript "$noscript" "$@" \
    --out "$out" >&2
if "$playground"; then
    bundle=$(sed -n 's|.*src="\(/bundle/[0-9a-f]\{16\}/\)boot.js".*|\1|p' "$out/index.html")
    if [ -z "$bundle" ]; then
        echo 'scripts/bundle.sh: playground page holds no bundle prefix' >&2
        exit 1
    fi
    sed "s|{bundle}|$bundle|g" "$shell/playground/headers" >"$out/headers"
fi
echo "$(cd "$out" && pwd)"
