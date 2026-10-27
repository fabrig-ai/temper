# 0a: the ground on a real owned Forgejo

0a is rung 0a of `docs/design/draft/qa-live.md` (3.1): a fresh `temper box` on
the run's directories, with one owned Forgejo set up from the lab's programs,
and no driver, account or model. It is free. The subject supplies `temper`;
the lab's settings supply `forgejo` and `forgejo-runner` by path and digest and
the kept port `forgejo`, which only Forgejo listens on.

The one process, `box`, runs `temper box` on rendered files: `temper.json` is
the document `temper init` writes, shaped to owned forge 1 with
`"start": "asked"`, the lab's two binaries and port, no demo, ChatGPT's
provider and model, and clients and webhooks on `127.0.0.1:0`. Every host its documents
name is `127.0.0.1`: a run with no driver and no held file has loopback alone
(the lab's `lab.md`, 5.2), and serve resolves each endpoint's name as it starts, the
sign-in's among them, though 0a calls none. Nothing is
fetched, and `temper init` never runs. `TEMPER_ECHO=steps` puts serve's
listeners on stderr, where readiness captures `serve: listening clients `.
The box's stdout is serve's sign-in link alone; the runner sends it to the
`sign-in` seed action, once, as its private bootstrap.

The template's turn bounds are rewritten for the agent's largest turn,
1,590,046 bytes, by `docs/design/protocol/setup.md` 8.1's derivation, and
both memory figures are raised to fit: finding F1's workaround. Without the
demo's endpoint, init's own bounds fall short of its own ChatGPT model and
serve refuses the document. The worker's memory carries 16 MiB of slack for
the lab's longer paths.

The seed:

- signs in by the link;
- runs `ctl forge setup 1` (`seed/forge-setup`), which prepares the forge,
  makes the project `hello` and adopts `work/hello` as its home, and keeps
  the answer in the run's `ground` directory;
- does the person's own act at first sign-in (`seed/person`). Forgejo's CLI,
  on the run's work path, unsets `ada`'s must-change-password and makes her a
  `read:repository` token. The token is 0600 and never printed.

The live collections are `temper` (ctl's `forge list --json`, `me --json` and
forge 1's picker page) and `forge` (the forge's API with ada's token). The
forge collection polls `work/hello`'s Actions runs until the first has
finished, then keeps the runs, main's combined status and the repository.
It deletes the token once read, so no run keeps it, whatever the outcome.

The `ground` check judges those files alone (the lab's `lab.md`, 7.2). It fails
with one of these signatures:

- `forge/not-connected`: forge 1 is not owned and connected;
- `adoption/missing`: the forges row's first repository is not `work/hello`
  made in the new project `hello`, or forge-setup did not print it adopted as
  the project's home;
- `project/missing`: the party has no project `hello`;
- `repository/missing` or `repository/shape`: there is no `work/hello`, or it
  is not private on `main`;
- `actions/first-run/missing` or `actions/first-run/STATUS`: the starter
  workflow's first run is missing or not `success`;
- `status/STATE`: main's combined status is not `success`.

With the owner's subject digest, run from this repository:

```sh
LAB=/path/to/lab
S=sha256:...
$LAB validate lab/live-0a --subject "$S"
$LAB judge lab/live-0a --subject "$S"
```

The class is `temper-live-0a`: 2× the largest of three calibration passes
under `temper-live-0a-calibration` (240/480/60 s). The runner also bounds its
own unit's start by the class's readiness. Under heavy load a 1 s class can
refuse with `containment: timed out` before any run; retry it. The tests are
`lab/tests/unit/test_live_0a.py`.
