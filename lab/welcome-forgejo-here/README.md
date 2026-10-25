# The welcome: forgejo-here

A fresh ordinary `temper box --fake-backends`, driven by skein's hand.
The browser walks Forgejo here: Choose, Set up a Forgejo here, and its first repository,
then “Use an API key”, pastes the exact bytes of the file captured from
`box: model key FILE`, and requires the Model step’s embedded “Ready” card and
“Added your OpenAI API key” status. It presses “Start a chat”, checks
that `hello` is the selected composer project, sends a first chat,
sees `Hello from the local fake model.`,
and reloads to see the reply again. hand needs no peer: the walk stays on temper's pages and opens no Forgejo page.

Owned setup continues during Model. Before following “Start a chat”, the
walk waits for the Model step’s adopted `work/hello` summary in `hello`.

The actions `welcome`, `forge`, `model`, `ready`, and `chat` invoke separate
`steps` phases so live scenes can capture between them. A missed call keeps
an outline and shot and stops the walk; a completion marker prevents later
actions from browsing after a failed phase; `call` is byte for byte the
`welcome-chatgpt` function. No sleep or retry is added. ctl only signs in
and collects the committed chat; all forge and model setup is clicked.
The spent-link relink sequence is `welcome-chatgpt`'s.

The checks require every hand trouble entry, including sign-in, to be absent,
and the first committed assistant message in ctl's chat document to equal
the fake reply. A title, user message, or later matching reply cannot pass.

Provision subject artifacts `temper` and `fake-claude` as exact executable
siblings, and `bundle` as the loopback bundle's USTAR tree
(`scripts/bundle.sh --loopback DIR`, packed with `lab/look_pack.py`).
Use the lab runner and hand from skein's `fake-backends/hand-peers` branch,
as [welcome-chatgpt](../welcome-chatgpt/README.md) does; the lab's browser
settings must name that hand.

Also provide a `forgejo-cache` subject tree artifact: deterministic,
uncompressed POSIX USTAR, preserving each binary's owner execute bit,
with the directory entries preceding these files:

- `forgejo/17.0.0/forgejo`
- `forgejo-runner/13.2.0/forgejo-runner`

The pinned binaries named by `TEMPER_FORGEJO` and `TEMPER_FORGEJO_RUNNER`
are suitable. The lab unpacks this tree at `{proc.box.dir}/cache` before
starting the box. The ordinary fetch checks the pinned digests there
(`protocol/setup.md`, 6.6); there is no fetch bypass or configuration override.
The box supervises the real Forgejo and runner, and uses real git.

```sh
$L validate lab/welcome-forgejo-here --subject "$S"
$L run lab/welcome-forgejo-here --subject "$S" --seed 1
$L run lab/welcome-forgejo-here --subject "$S" --seed 2
$L run lab/welcome-forgejo-here --subject "$S" --seed 3
```

The class is `temper-m1-calibration`: ready 120 s, command 240 s,
stop 20 s; page waits 120 s and the first reply 180 s, as the existing
welcome experiment. This world requires no real account or model.

Tests: `lab/tests/unit/test_welcome_keys.py` checks the manifests, evidence
scripts and committed reply/trouble decisions for both experiments, and
compares every `lab/welcome-*/steps`'s `call`. The process tests in
`lab/tests/process/test_welcome_keys.py` check both complete walks and
separate phases, exact pasted bytes, every missed wait, failed pastes,
and invalid chat addresses under the real POSIX shell.
