# The welcome's ChatGPT sign-in

A fresh ordinary `temper box --fake-backends` and skein's hand, no driver.
The box supervises its four fake peers. ctl signs in and connects forge 4,
the fake Forgejo elsewhere, making and adopting `work/hello` in `hello`.
The spent-link relink sequence is `cluster-web`'s. Before navigating, the
walk waits for the visible Sign out button so the printed link's page
has finished signing in. hand's sole peer is the
issuer origin captured from the box's stderr; every other destination
remains refused.

The `welcome` function in `steps walk` opens `/welcome`, the Model step and Continue with ChatGPT,
then requires `work/hello is ready` and opens `hello`. Its `chat` function sends a
first message, waits for the fake reply, and reloads to see it again.
A missed step records the outline and a shot and stops the story. There
are no sleeps. All waits use the existing lab allowances.

The checks require an empty hand trouble log, including sign-in, and the
first committed assistant message in ctl's chat document to equal
`Hello from the local fake model.` A title and a person's turn cannot
satisfy the reply check.

Provision subject artifacts `temper` and `fake-claude` as exact executable
siblings, plus the loopback bundle as a USTAR tree artifact `bundle` (use
`lab/look_pack.py`). Build hand from the skein branch carrying `--peer`
and name it in the lab's browser settings. Use that branch's runner:

```sh
$L validate lab/welcome-chatgpt --subject "$S"
$L run lab/welcome-chatgpt --subject "$S" --seed 1
$L run lab/welcome-chatgpt --subject "$S" --seed 2
$L run lab/welcome-chatgpt --subject "$S" --seed 3
```

The cost is the existing `temper-m1-calibration` class: ready 120 s,
command 240 s, stop 20 s. This fake world uses no real account or model.
