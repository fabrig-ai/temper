# S: the free NOTICE smoke

S runs temper's NOTICE story on the lab's runner, with no driver or model.
The subject supplies `temper` and the same binary named `git`. The cluster is
forge → TLS fake LLM → engine → two remote workers; its documents equal
`../cluster/` byte for byte. The runtime LLM adapter adapts the cluster's NOTICE
root for S, H and P, keeping the root chat open after its delegate settles.

The runner sends the engine's private stdout directly to the `sign-in` seed
command, once, without retaining the link or any sign-in stream. Public
readiness still captures all three engine listeners. The next seed action
applies `settings.json` against revision 1, preserving the long chat wait,
coding shell and chat cap. The remaining seed adds
the fake model key, waits for usable accounts and the connected forge, and
adopts `fake/widget`. The story asks for NOTICE through `tools/ctl`, requires
the coding delegate's historical successful change result, and reads a
complete root tree with no held or live delegates. It has one 220 s deadline;
seed waits allow 200 s within the class's 240 s command bound.

After the story, the live `forge` collection retains observations and C/P
object responses; the file-only `end-state` check judges them. After all
process exits and empty trees are confirmed, stopped collections copy the
forge's observations, then the engine store. The stopped `promises` check
uses those copies and temper's independent decision records. See
[checks/README.md](checks/README.md).

With the owner's subject digest:

```sh
LAB=/path/to/lab
S=sha256:...
$LAB validate lab/chat-change-smoke --subject "$S"
$LAB run lab/chat-change-smoke --subject "$S" --seed 1
$LAB run lab/chat-change-smoke --subject "$S" --seed 2
$LAB run lab/chat-change-smoke --subject "$S" --seed 3
$LAB judge lab/chat-change-smoke --subject "$S"
```

The cost class stays `temper-m1-calibration`. Each changed experiment has a
new key; a pass needs three seeds. Records and raw evidence live under the
lab's `keys/chat-change-smoke/<key>/runs/<run>/`.
