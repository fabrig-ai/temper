# Rung A, walk 5

`llm.json` is tools/twin's output from the cleanly stopped walk at
96532b9493 (gpt-6.1-sol/high). `person.json` keeps the client's three
requests as data, shared with the future process-profile driver.
`forge.json` seeds the owned playground files and passing starter CI.
`pace.json` names the dark review's final completion and its release condition;
`temper_real_loop_world::llm::ReplyGate` can drive the same pacing in a process
transport. The fuzzy transport pauses delivery until CSV's green merge is
observed. This preserves the walk's landing order despite kernel scheduling;
elapsed delays alone let dark theme land first. The model text and calls are
unchanged.

From the workspace root, regenerate only through the converter:

```sh
sh tests/real-loop/tests/fixtures/rung-a/regenerate.sh [DEPLOYMENT]
```

The producer cues precede the chat cues: each producer's own specification
starts with the person's request, so their matches have the same starting
position. The longer producer cue wins that tie by document order. Review
cues include their own change title; the retry's introduction precedes it.
The story checks the selected scripts against their own requesters and
requires all 74 recorded completions, with no fallback turns.

The only substitutions replace recorded commit IDs with the fresh fake
forge's identities. The converter reports each count. No deployment file
or raw transcript is copied into this fixture.

The fuzzy story keeps the two other changes held at resolution and review,
as in the walk. Its final cancellations close their pulls to drain cleanly;
the observer checks the open pulls before that cleanup. The forge has room
for three retained pulls and current reads, and the model's 100 ms pacing
scales the walk down. A 40-second story guard covers its eleven agents; the
fuzzy profile's two-minute suite bound stays unchanged.
