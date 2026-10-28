#!/bin/sh
# Walk 8, after its successful drained stop; run from the workspace root.
set -eu
heavy env TEMPER_BUNDLE=none NEXTEST_DOUBLE_SPAWN=0 cargo run --locked -p temper-twin --bin twin -- --data "${1:-/home/free/src/rust/tmp/qa-focused/rung-a-8/data/deployment}" --base tests/real-loop/tests/fixtures/rung-a/llm-base.json --out tests/real-loop/tests/fixtures/rung-a/llm.json \
  --cue '3=Add CSV export to widget. Inspect the existing widget' \
  --cue '1=Add CSV export to widget' \
  --cue '4=Change: Add CSV export to widget' \
  --replace 8d4ecd4d47aea870eeec145c1dce13831d110039=975835de1c9756ce000000000000000200000000
