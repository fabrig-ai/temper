# The welcome: github

A fresh ordinary `temper box --fake-backends`, driven by skein's hand.
The browser walks GitHub one-click: Connect GitHub, Create GitHub App, and GitHub's unaided create/install/return chain,
then “Use an API key”, pastes the exact bytes of the file captured from
`box: model key FILE`, and requires the Model step’s embedded “Ready” card and
“Added your OpenAI API key” status. It presses “Start a chat”, checks
that `widget` is the selected composer project, sends a first chat,
sees `Hello from the local fake model.`,
and reloads to see the reply again. hand admits only temper's origin and the fake GitHub origin captured from `box: fake github http://`.

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

```sh
$L validate lab/welcome-github --subject "$S"
$L run lab/welcome-github --subject "$S" --seed 1
$L run lab/welcome-github --subject "$S" --seed 2
$L run lab/welcome-github --subject "$S" --seed 3
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
