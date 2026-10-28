#!/bin/sh
# B1 walk 5, after its clean stop; run from the workspace root.
# As on the walk, the goal projects issue 1, then the first pull is 2.
# Question 2, pull reads and changes/2.md therefore recur unchanged.
# Rewrite only calls' arguments: repaired head, goal finish's merge and
# forge address. Recorded prose stays as it was.
# The address marker is bound to the kernel-assigned listener by repair::recipe.
set -eu
heavy env TEMPER_BUNDLE=none NEXTEST_DOUBLE_SPAWN=0 cargo run -p temper-twin -- \
  --data "${1:-/home/free/src/rust/tmp/qa-focused/rung-b1-5/data/deployment}" \
  --base tests/real-loop/tests/fixtures/b1/llm-base.json \
  --out tests/real-loop/tests/fixtures/b1/llm.json \
  --cue '1=[Requested by person 1]' \
  --cue '2=[Requested by task 1]' \
  --cue '3=You are an advisor: you read what your brief asks and report what you found; you change nothing and ask no one.' \
  --cue '5=Implement CSV export for the existing hard-coded TABLE data in src/main.rs' \
  --cue '6=Repair the failed CI on the change branch' \
  --cue '7=Review this change at the named head' \
  --replace 'c11d0f93cbd8254894b5af9863483d3cf5069d03=1d0b14e4db018fed000000000000000300000000' \
  --replace 'e77e8f5454a6ca32047ce064e25446dfbb22e8a4=6e73e372e2338aca000000000000000400000000' \
  --replace 'http://127.0.0.1:34357=http://B1-FORGE'
