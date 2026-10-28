# A stopped run's twin

`twin` turns the selected tasks' transcripts from a cleanly stopped deployment
into a `temper fake llm --document` document (domain/testing.md, 4.1).
It is a developer tool, never shipped or published.

```sh
heavy env TEMPER_BUNDLE=none cargo run -p temper-twin --bin twin -- \
  --data /absolute/data/deployment --base /absolute/base.json \
  --out /absolute/twin.json \
  --cue '1=Add CSV export to widget' \
  --cue '2=ask the chat for a green light first' \
  --replace 'OLD=NEW'
```

`--data` names Jig's deployment directory, the resolved
`engine.deployment.directory` under Temper's data directory. It is the directory
holding `transcripts/` and `active`, not a generation's `store/` or `secrets/`.

Each `--cue TASK=PHRASE` takes a task's completions in file order, across
resumptions, as one system-cued script. Smith puts the brief in system text,
a chat's request included. Pick distinct phrases of those briefs; the fake
chooses the earliest cue in a task's system text, so replay the result to
check selection. `asked` and every other base member are preserved; only
`cued` is replaced. The base and result are checked by
`temper_fake::llm::Document::decode` before any output is opened.

A completion's ordered text and calls become one turn. Calls finish
`tool_calls`, other completions `stop`. Output tokens are the recorded count,
or 1 when absent. Arguments are embedded as recorded JSON, preserving their
spacing and member order in the document; the fake subsequently applies its
normal compact JSON encoding when it serves them. `--replace OLD=NEW` applies
literal, non-overlapping byte substitutions to arguments alone, in option
order, counting occurrences per option. An empty OLD or a replacement that
produces invalid object JSON is refused. Reports list turns per task and
occurrences per replacement and opaque blocks dropped per task. A refusal
prints one line and exits 2; no output is opened on a conversion refusal.

The bounds are the fake's: sixteen scripts, sixteen turns per script, sixteen lines
per turn, 256-byte nonempty cues, 64 KiB answers, 256 KiB total scripts and
a 1 MiB document. The fake's decoder checks its detailed JSON and byte bounds.
Opaque blocks are dropped, keeping text and calls in order. Cut calls and
compactions are refused with task and turn; bad or partial frames include
task and byte offset. Versions must be those
of this build's Jig and Smith codecs. There are no upgrades and the store is
never opened. Jig's `protocol/store.md`, 8.1–8.4, owns frame layout and the
clean-stop guarantee: a complete checksummed frame does not independently
prove its commit. Supply only data from a successful drained stop, never a
crash or failed commit. The reader detects incomplete or corrupt tails but
does not inspect the map's committed-length records.

The small codec fixtures are actual Jig frames recorded from the original
hand-written green-light story before B0 replaced it. They contain synthetic
test data only, no credential files.

The current round trip records B0's generated twin, converts those five tasks'
completions, and replays the same acts and checks on a fresh deployment. It
checks converter stability through recording/replay and the story's outcome;
it does not independently check model judgement or changed prompts. Normal
tests never rewrite fixtures. Regenerate B0's document from its cleanly stopped
passing walk using the command named in `tests/real-loop/src/green_light.rs`.
Task, question and pull numbers and fake-git identities recur. The scripts'
forge reads infer their repository; no grant carries a listener address.

```sh
heavy env TEMPER_BUNDLE=none NEXTEST_DOUBLE_SPAWN=0 cargo nextest run -p temper-twin
heavy env TEMPER_BUNDLE=none NEXTEST_DOUBLE_SPAWN=0 cargo nextest run \
  -p temper-real-loop-world --profile fuzzy \
  --test fuzzy_twin_round_trip --test fuzzy_green_light
```
