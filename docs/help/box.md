# Run a local box

Kind: walk

Run a whole deployment on one machine, on loopback, from its configuration to a change landed on its own Forgejo, then stop it without losing the model's account.

## Before you start

- **The binary:** `temper` on `PATH`, and git 2.41 or later. From a
  temper checkout, `scripts/install.sh "$HOME/.local"` installs it;
  its build needs Rust's `wasm32-unknown-unknown` target.
- **Real or fake backends,** chosen before step 2; a box never mixes
  them. Real: the model is a real account, and the box fetches
  Forgejo and its runner on first use. Fake: GitHub, the model, its
  sign-in and a Forgejo elsewhere are fakes on loopback, and no one's
  account is used; Forgejo on this machine stays real. A box on fake
  backends needs, from a temper checkout,
  `cargo build -p temper -p temper-fake-claude`, which puts the
  adapter `fake-claude` beside `temper`, and
  `scripts/bundle.sh --loopback BUNDLE`.
- **The directories:** `CONFIG`, `DATA` and `CACHE`, absolute and
  empty, and `DIR`, private to you. Nothing else may use them.
- **The model's account,** on real backends, one of: a login exported
  from another deployment to a private file; a browser on this
  machine, for a provider's sign-in; an API key in a private file.
- `BUNDLE`: the loopback bundle's directory, on fake backends.
- `REQUEST`: a file of the words that ask for the change.
- `CHANGE`: the change task's number, which step 6 reads.

## 1. Write the configuration

```
temper init --config CONFIG --data DATA --cache CACHE
```

Make every change the start needs now, while nothing runs: a shared
GitHub App's read-only mode is one (temper help github). On fake
backends, leave `CONFIG` as `temper init` wrote it.

Expect: `init: wrote DATA/worker.secret`, with `DATA` the path you
gave; the last line names the step 2 command for real backends.

If not: `temper init: CONFIG/temper.json: already exists` (exit 1):
`CONFIG` holds a configuration; start from empty directories.

## 2. Start the box

Real backends:

```
temper box --config CONFIG --data DATA --cache CACHE
```

Fake backends:

```
temper box --fake-backends --bundle BUNDLE --config CONFIG --data DATA --cache CACHE
```

Keep it running, in a terminal of its own or in the background with
its output in a file. It prints its sign-in link on a line of its own,
after `Open this link in a browser on this machine.`

Expect: real, `Logs: DATA/box/logs/`. Fake,
`box: effective configuration DATA/box/fake-backends/config`; among
the next lines, `box: model key FILE` and
`box: GitHub own App id 1, key file FILE`, the files later steps read.

If not: `missing or nonexecutable adapter`: build `fake-claude`
beside `temper`, as `Before you start` says. `not in current init
shape`: `CONFIG` was changed after `temper init`; start again from
empty directories. `retained documents must be private files`: an
effective document lost its mode; `chmod 600` it. A start refused for
its memory names by how many bytes the whole worst case passes
`engine.memory_bytes`, from any value, the box's two pipes included:
raise it by that once and start again. `--box: the box's pipes need ...`: the pipes lack what
the line names (memory, socket slots, or a nonzero
`application.limits.overrides.box.line_bytes`); give it. A start
whose stored state is refused prints, in place of a sign-in link,
`temper serve: restore KIND: REASON` or
`temper serve: restart refused during STEP`; the box starts serve
again a few times, each saying the same, then gives up and exits 1:
keep `DATA` as it is and report the line as a finding.

## 3. Sign in

Give the box's link on stdin, never as an argument:

```
temper ctl DIR sign-in -
temper ctl DIR me
```

Expect: `party 1 · owner · Owner`, the owner of a new box; `me`
then lists `1 · Project 1 · owner`.

If not: `temper ctl: the sign-in link was refused` (exit 1): the link
was spent or expired. The box then prints `A fresh sign-in link:` and
a new link: sign in with it.

## 4. Set up Forgejo on this machine

`PROJECT` is 1 on a new box, as `me` lists it.

```
temper ctl DIR forge setup 3 --org work --repository hello --project PROJECT --key KEY
```

Expect: `setting up`, then `project: PROJECT · NAME`, the project
and its name, then `Adopted work/hello on forge 3 as owned.` and
`It is the project's home.`

At each Forgejo start the box prints its API address and your
headers:

```
Forge 3 (Forgejo): api http://127.0.0.1:PORT/api/v1
Forge 3 (Forgejo): person LOGIN, read header PATH, write header PATH
```

Pass a header by path with `curl -H @PATH`, keeping its token
private. Your first password is in `person_password` beside them;
change it on Forgejo when you like, with no forced first password
change.

If not: past 300 seconds it prints the forges and `not met: timeout`
(exit 3) while the box still fetches Forgejo: run `temper ctl DIR
forge list --until made:3`, then go on. A failed fetch or start
shows in `temper ctl DIR forge list`, with its reason.

## 5. Give the model an account

Choose one way. Provider 1 is OpenAI on a configuration `temper init`
wrote; `me` lists the providers.

A login exported from another deployment:

```
temper ctl DIR account import FILE --key KEY
```

A provider's sign-in, opened in a browser on this machine; on fake
backends, any HTTP client completes it:

```
temper ctl DIR account connect PROVIDER --key KEY
```

An API key from a private file; on fake backends, the model key file
the box printed:

```
temper ctl DIR account add-key PROVIDER FILE --deployment --key KEY
```

Then:

```
temper ctl DIR account list --until usable
```

Expect: `moved in: account N`, the address to open, or `done:
add-key`; the wait ends with `met`.

If not: `key file mode 0o644 permits group or other access` (exit 1):
`chmod 600 FILE`, then the same call with the same key. The person's
browser is on another machine: provider 1's sign-in calls back to
`127.0.0.1:1455` on this one, which that browser reaches only through
`ssh -L 1455:127.0.0.1:1455 HOST`, HOST this machine; give the person
that line with the address. A wait that ends `not met: timeout`
(exit 3) after a sign-in: the browser did not finish it; open the
address again, then wait again.

## 6. Ask for a change

```
temper ctl DIR chat start --project PROJECT - --key KEY < REQUEST
temper ctl DIR task show TASK --until replied
temper ctl DIR task show TASK --tree
```

`TASK` is the chat's number, which the first call prints. The tree
lists the change task the chat started: that is `CHANGE`.

Expect: `done: task TASK, chat`; the wait prints the task, its first
line `task TASK · chat · ` and its phase, then `met`.

If not: on fake backends, the model answers every request with
`Hello from the local fake model.` and starts no change: go to step 8.
A chat refused with `chat exceeds offered bounds` (exit 1) asks for
more than the project allows your party: read `me`.

## 7. Follow the change to its landing

```
temper ctl DIR change show CHANGE --until landed --timeout 3600
temper ctl DIR change show CHANGE
```

The change waits for CI to pass at its head; the starter workflow
passes. Read its diff, reviews, CI runs and logs on the forge, at the
pull request `change show` prints.

Expect: the change's panel, its first line `change CHANGE ·
work/hello on forge 3 · `, its branch and `into main`, then `met`;
`change show` then names its step, `landed`, and its merge commit.

If not: `not met: timeout` (exit 3): read `temper ctl DIR change
show CHANGE` and `temper ctl DIR task show CHANGE --tree` for its step, its
hold and its acts, act on them, and wait again. `not met: ended:
cancelled` or `not met: ended: failed` (exit 3), at once: the change
ended without landing; its tree says why. `held: task N (REASON)` in
the panel: task `N`, its writer or its reviewer, is held; `temper ctl
DIR task show N` prints its way out. `held before registration:
REASON` (exit 2): the change was held before its pull request was
made; `temper ctl DIR task show CHANGE` prints its way out.

## 8. Export the account, then stop

On real backends, move a signed-in account out before the stop:

```
temper ctl DIR account list
temper ctl DIR account export ACCOUNT FILE
```

`ACCOUNT` is the account whose source is `sign_in`, `granted`;
`FILE` must not exist. Then stop the box with `SIGINT` (Ctrl-C) or
`SIGTERM`.

Expect: `moved out: account ACCOUNT to FILE`; the box's last line
ends `Stopped`.

If not: `account export: not signed in` (exit 2): the account holds no
sign-in to move; a key is added again where it is needed.
`temper ctl: the engine holds its N answers of the last day; room again
at TIME` (exit 2): the engine has no room for the move's answers, and
nothing moved: keep the box running and account export again after
TIME. `temper ctl: unsent` (exit 1): the engine was busy, and nothing
moved: account export again later. Any other failure:
keep the box running; the engine keeps the account and cancels the
move at its deadline: account export again after it.

## Clean up

- `CONFIG`, `DATA` and `CACHE` hold the deployment: start the box
  again with the same three to resume. On fake backends, removing
  `DATA/box/fake-backends` resets it.
- Keep `FILE` from step 8 private until an import on the next
  deployment removes it.
- Remove a key file once its account is usable, and `DIR` once you
  are done.

## Next

- temper help ctl: the loop, keys and exit statuses for every call.
- temper help github: GitHub beside this box's Forgejo.
