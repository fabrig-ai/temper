# The welcome with an App of your own and a program

A fresh ordinary `temper box --fake-backends`, driven by the lab's hand.
The Welcome phase opens `/welcome` and presses Set up temper. Forge
opens the own App’s Details at `/welcome?at=forge&forge=2` (GitHub is
forge 2 in the fake box), types App id 1 and the bytes from the
key file the box names, connects, installs on the fake GitHub and chooses
`fake-person/widget`. Model chooses the program way, labelled Use your
Claude subscription on the fake box’s Anthropic provider, backed by the
Claude Code adapter,
reads the name shown and runs the box's printed program-login command
through `--cache DIR`, with that name appended, then submits the program
account with Use your Claude subscription. The page then observes
`fake-person/widget is ready`. Ready captures that end separately.

This walk ends at Ready because jig cannot keep a chat open through a program yet (`jig-domain-core/src/run_choice.rs`).

The browser allows the fake GitHub origin captured from the box as its
peer. The checks require no hand trouble and ctl's named program source
usable, with its connected worker reporting that same login usable.
A missing step stops immediately with an outline and a shot. Each welcome
step is a separate `steps` phase so live scenes can capture between them.

Provision the executable siblings `temper` and `fake-claude`, and a USTAR
loopback bundle artifact `bundle`, as in
[welcome-chatgpt](../welcome-chatgpt/README.md). Use the lab's runner and hand
with `--peer` support:

```sh
$LAB validate lab/welcome-own-app --subject "$S"
$LAB run lab/welcome-own-app --subject "$S" --seed 1
$LAB run lab/welcome-own-app --subject "$S" --seed 2
$LAB run lab/welcome-own-app --subject "$S" --seed 3
```

The existing `temper-m1-calibration` allowances apply: ready 120 s,
command 240 s, stop 20 s. All backends and the program login are fake.
