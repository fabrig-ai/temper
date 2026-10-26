# H: the NOTICE chat change

H asks a chat in project 1 for NOTICE through the provisioned `ctl` tool and
follows the coding change to landing. Its charter is draft/qa-lab.md, 6.1 verbatim.
It shares S's documents, processes, six seed actions, collections and checks;
the scripted story is replaced by the driver. The runtime LLM adapter adapts
the cluster's NOTICE root for S, H and P, keeping the root chat open after
its delegate settles. No smoke story is included.

`../chat-change-killed/` is P: the same files, with only `faults` and `planted`
changed in `experiment.json`. It kills the engine after the first driver
action and expects `stop/engine/signal:SIGKILL`. Clearing both arrays gives
H's exact bytes. Its name, cost and about stay H's. Do not run P until the
runner supports faults.

The scripts in `../chat-change-rehearsal/` are outside the experiment's key.
`correct.json` asks for NOTICE and reads its change's result; `wrong.json`
only reads `me` and claims done, so the independent end-state check fails
`check/end-state/not-landed/fake/widget/main`. Both use the real cluster,
provisioning and checks with a scripted driver endpoint and no model or login.

With the owner's subject digest and configured `temper-m1` class:

```sh
LAB=/path/to/lab
S=sha256:...
$LAB validate lab/chat-change --subject "$S"
$LAB run lab/chat-change --subject "$S" --seed 1 --rehearse lab/chat-change-rehearsal/correct.json
$LAB run lab/chat-change --subject "$S" --seed 1 --rehearse lab/chat-change-rehearsal/wrong.json
```

Before calibration sets `temper-m1`, copy H outside the repository, change
only `cost` to `temper-m1-calibration`, and validate and rehearse that copy.
Remake it whenever H changes. See `../README.md` and draft/qa-lab.md, 10.1.
