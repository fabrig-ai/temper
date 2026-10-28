#!/bin/sh
# Walk 5, after its successful drained stop; run from the workspace root.
set -eu
heavy env TEMPER_BUNDLE=none NEXTEST_DOUBLE_SPAWN=0 cargo run --locked -p temper-twin --bin twin -- --data "${1:-/home/free/src/rust/tmp/qa-focused/rung-a-5/data/deployment}" --base tests/real-loop/tests/fixtures/rung-a/llm-base.json --out tests/real-loop/tests/fixtures/rung-a/llm.json \
  --cue '4=Add CSV export to widget. Current src/main.rs' \
  --cue '6=Add a dark theme to widget. Current repository' \
  --cue '11=Fix widget'"'"'s empty search state. Current checkout' \
  --cue '1=Add CSV export to widget' \
  --cue '2=Add a dark theme to widget' \
  --cue '8=Fix widget'"'"'s empty search state' \
  --cue '7=Change: Add CSV export to widget' \
  --cue '9=Change: Add a dark theme to widget' \
  --cue '12=Change: Fix widget'"'"'s empty search state' \
  --cue '14=This review is asked again at the same head' \
  --cue '13=Resolve the merge conflict and push the merge commit' \
  --replace e2b40db18e86388eaac0065747288e5808a371f6=975835de1c9756ce000000000000000200000000 \
  --replace b17420a5a3f92b257793120c28486e54fbf900100=1d0b14e4db018fed000000000000000300000000 \
  --replace 2fe8ef1d2e955fea80efd753962aa8a050817f0e=63033b0ca389c35a000000000000000500000000
