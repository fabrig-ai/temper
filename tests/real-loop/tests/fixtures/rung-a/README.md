# Rung A's landing path, walk 8

`llm.json` is tools/twin's output from the cleanly stopped walk at
19ce18cfec5da23688dec999d9803196bdf27116 (gpt-6-luna/medium).
`person.json` keeps its CSV request. `forge.json` seeds the owned playground
files and passing starter CI. `pace.json` keeps the final-answer close and
100 ms model latency. No reply gate or fabricated follow-up is needed.

From the workspace root, regenerate only through the converter:

```sh
sh tests/real-loop/tests/fixtures/rung-a/regenerate.sh [DEPLOYMENT]
```

The writer cue precedes the chat cue so its longer own specification wins
the shared-prefix tie. The review cue names its change. The story checks
each selected script's own requester and all twenty recorded completions:
writer 3 (8), chat 1 (3), reviewer 4 (9). Conversion drops four opaque blocks.
Only the recorded CSV head in arguments is substituted for the fresh fake
identity; the converter reports six replacements. No deployment file or raw
transcript is copied here. The script count remains eleven, sized by walk 5;
each script admits sixteen turns, matching the stock coding/reviewer runs.

CSV change 2 lands through passing CI and blocking agent review. The oracle
checks its head/merge identities, one merge, no open pulls and the exact
resulting source through the outside forge API. The chat closes after its
final answer, as in the walk. Dark theme and empty search return with the
next walk's whole twin: walk 8's bare chat-to-advisor calls are now refused
on main (1d9f72bebf). This fixture covers the landing path alone. Its
40-second guard fits under the unchanged two-minute fuzzy suite budget.
