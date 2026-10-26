# Start a local temper box

Kind: guide

Run temper, an owned Forgejo and its runner on one machine, on
loopback, from a checkout of temper, then adopt a repository and ask
for work.

Temper supplies the stock charters, templates, the policy new projects
are made from and the rules. They are settings: read them with
`temper ctl DIR settings` after signing in as below. An owner changes
them with `temper ctl DIR change-settings`, or a member proposes a
change with `temper ctl DIR propose-settings`. Here DIR is your signed-in
client directory; `temper help settings` explains the change file and
the required revision and key.

## 1. Install temper

From a temper checkout, install the binary and put its directory on
PATH. Its build needs Rust's wasm32 target
(`rustup target add wasm32-unknown-unknown`). Have git 2.41 or later on
PATH too.

```sh
scripts/install.sh "$HOME/.local"
```

It prints the installed binary's path, `$HOME/.local/bin/temper`.

## 2. Start from empty directories

Choose C for configuration, D for data and K for cache; use absolute
paths in place of those letters throughout this walk. Keep this terminal
open:

```sh
temper box --config C --data D --cache K
```

An empty C runs `temper init` first. It prints `init: wrote PATH` for
each file, then `First run: wrote the configuration, PATH`. It writes
`temper.json` and `agent.json` in C, and private worker secrets and the
dormant playground's files in D. You do not make secrets by hand.

The configuration chooses free loopback ports, preferring 8367 for the
client, 8368 for Forgejo, 8369 and 8370 for the playground, and 8371 for
webhooks. If a preferred port is busy, it writes another fixed port.
The box prints its sign-in link at the client's address. Keep that
link private for the next step.

Forge 2 is GitHub, not yet connected; forge 3 is your own Forgejo,
started only when you ask. The playground stays dormant. When you ask
for forge 3, the box fetches Forgejo **17.0.0** into
`K/forgejo/17.0.0/forgejo` and runner **13.2.0** into
`K/forgejo-runner/13.2.0/forgejo-runner`, checking their pinned digests.
It reuses matching cached files. To supply installed binaries instead,
set `forgejo` and `runner.binary` in forge 3's owned setup to their
absolute paths before starting.

To edit configuration first, run `temper init --config C --data D
--cache K` separately, then start the box. For a shared GitHub App, set
its read-only mode before this start (`temper help github`, section 11).

## 3. Sign in through ctl

In another terminal, choose DIR as a private directory for ctl's local
sign-in and request store. Use that same directory for every ctl call
below; it is separate from C, D and K.

```sh
temper ctl DIR sign-in -
temper ctl DIR me
```

For the first command, paste the box's link on stdin, then press Enter
and Ctrl-D. Never put it on the command line. A link works once, within
five minutes; if it expires, use the fresh link the box prints. The
second command prints your party, projects and allowed acts.

## 4. Set up your Forgejo and first repository

```sh
temper ctl DIR forge-setup 3 --org work --repository hello --project 1
temper ctl DIR forges
```

The setup prints `setting up`, waits for the box to fetch, prepare and
start Forgejo and its runner, then prints the adoption of `work/hello`
into project 1, with CI on and landing branch `main`. The forge row says
connected and names the first repository. You may replace `work`,
`hello` and `1`: `--org` defaults to `work`, `--repository` to `hello`,
and omitting `--project` makes a new project named after the repository.
Use the project number printed by the adoption in the following steps.

To try the playground's suggested requests on your own Forgejo, set it
up with a copy of the playground's repository instead:

```sh
temper ctl DIR forge-setup 3 --playground
```

It makes `playground/widget`, with the playground's `src/main.rs`
beside the starter workflow, and adopts it as the home of a new project
named `widget`. `--playground` takes no `--org` or `--repository`. Ask
that project the playground's requests: "Add CSV export to widget",
"Add a dark theme to widget", "Fix widget's empty search state".

Every change is reviewed by an agent at its head before it lands:
`change N` shows the review, gate 1, pending, then approved.

CI jobs run on this machine as the box's user, with label
`temper-host`, without a container. Run code you trust there. The
starter workflow only echoes a success message, on pushes to `main`
and on pull requests. It proves the runner works; your repository's
real checks belong in its own workflow. Ask temper to add those checks
when you ask for work.

## 5. Give the model an account

Choose one path. On a machine without a browser, import a login from
another deployment, or add an API key from a private file.

To move a signed-in account from another running deployment, export it
there first (`temper help server`, section 10), then:

```sh
temper ctl DIR import FILE --key model-in
temper ctl DIR wait accounts --until usable
```

Import prints `moved in: account N` and removes the spent FILE. The
wait reports when an account is usable. A refreshing login has one
holder at a time: export relinquishes it, and import takes it. Do not
copy its record or run the same login in two deployments.

Or sign in with ChatGPT, provider 1 in the generated configuration:

```sh
temper ctl DIR connect 1 --key model-login
temper ctl DIR wait accounts --until usable
```

Connect prints a sign-in address; complete that sign-in in a browser.
Its callback is `127.0.0.1:1455` on the box's machine. If your browser
is elsewhere, forward it with `ssh -L 1455:127.0.0.1:1455 HOST`.

Or add an OpenAI API key, FILE a regular file owned by you, not a link,
with no group or other permissions:

```sh
chmod 600 FILE
temper ctl DIR add-key 1 FILE --deployment --key model-key
temper ctl DIR wait accounts --until usable
```

The key is sent without being printed. Read `accounts` to see the
account and its sources, never their values. Remove your key file once
the account is usable. A key account is added again on another
deployment; only a granted sign-in source moves by export.

## 6. Ask for a change and follow it to landing

Use your project number in place of 1. Supply your request on stdin,
then Ctrl-D; name the repository, the change and the checks to run.
For example: append a sentence to `work/hello`'s README, check it, and
take the change through CI and landing on `main`.

```sh
temper ctl DIR chat --project 1 - --key first-change
```

It prints the chat's task number. In this example it is 1; substitute
the number you received:

```sh
temper ctl DIR wait task 1 --until replied
temper ctl DIR task 1 --tree
```

The wait reports a reply; the tree names the change task the chat
delegated. Read the chat with `temper ctl DIR chat 1` for its words.
Use the change task's number, here 2, for the next calls:

```sh
temper ctl DIR change 2
temper ctl DIR wait change 2 --until landed
temper ctl DIR change 2
```

The panel names the repository, branch and head, the pull request's
number and address, CI at its head, its step and any hold. The change
waits for a reported CI pass at its head; without a reported check it
keeps waiting. Landed, it prints the merge commit. If a wait times out (exit 3),
read `change 2` and `task 2 --tree` for its step, hold and next acts;
follow the printed calls and wait again.

Read the pull request's diff and reviews, CI's runs and logs, and the
branches on the forge, at the ids and address `change 2` prints, with
your chosen tools. Compare what is there with temper's account. The
change's branch, pull request and landing belong to temper: change
them only through temper. Parts served by the forge say
"Shown on the forge."

## 7. Export the login, then stop

Before stopping a box whose signed-in account you want elsewhere,
read its account number and export it to a new FILE:

```sh
temper ctl DIR accounts --json
temper ctl DIR export 1 FILE
```

Replace 1 with the account whose `sign_in` source is granted. Export
prints `moved out: account N to FILE`, creating a private 0600 file.
Check that FILE is regular and nonempty by `stat`, never by printing
its contents. If export fails, keep the box running and follow its
message; do not discard the only holder of the login. After export
succeeds, stop the box immediately with Ctrl-C in its terminal, or send
that box SIGTERM. Import FILE on the destination deployment as in
section 5; a successful import removes it.

C keeps your configuration, D keeps the engine's state, worker secrets,
workspaces and `forge-3` with the repository, and K keeps the downloaded
binaries. Restart with the same three directories to resume. Keep DIR
private too; it holds ctl's sign-in. With the box stopped, back up C
and D together to keep your deployment.

## 8. With GitHub too

Forge 2 is already configured. For an existing App and installation,
follow `temper help github`, sections 11 and 12: set shared-App
read-only mode before starting, connect with the private key, accept
the installation and adopt its repository. No browser is needed for
that path. The box's Forgejo repository is already the project's home:
add `--no-home` when adopting GitHub into that project, or use a new
project to make GitHub its home. Use the adopted project's number and
name the GitHub repository in section 6's chat; follow its change
through CI and landing with the same calls.

## 9. Trying the welcome with nothing real

Use `--fake-backends` to click through the welcome without anyone's
accounts. GitHub, the ChatGPT sign-in, the model and a Forgejo elsewhere
are fakes on loopback. Forgejo here runs a real Forgejo and runner; a
program uses the real login mechanism with the local `fake-claude`
adapter. Once Forgejo here's binaries are cached, nothing in the walk
reaches the network.

From the checkout, build temper and its sibling adapter, then a browser
bundle that allows loopback navigation (the ordinary bundle does not).
Use fresh directories, independent of the walk above:

```sh
cargo build -p temper -p temper-fake-claude
scripts/bundle.sh --loopback "$PWD/welcome-bundle"
target/debug/temper box --fake-backends --bundle "$PWD/welcome-bundle" \
  --config "$PWD/welcome-config" --data "$PWD/welcome-data" \
  --cache "$PWD/welcome-cache"
```

The usual `--config`, `--data` and `--cache` flags choose the source
directories. The private sandbox lives at `DATA/box/fake-backends`, with
its own `config` and `data`; the cache stays at the directory you gave.
An empty configuration is initialized there. The source configuration
is left untouched. Keep the box's terminal open: it prints these lines,
with your paths and addresses in place of `DIR`, `FILE` and `ADDR`:

```text
box: effective configuration DIR
box: effective data DIR
box: effective cache DIR
box: fake github http://ADDR
box: fake issuer http://ADDR
box: fake model http://ADDR
box: fake forgejo http://ADDR
box: model key FILE
box: Forgejo elsewhere admin token file FILE
box: program login: TEMPER program-login --config ... --data ... --cache ... NAME (the name the Model step shows)
box: GitHub own App id 1, key file FILE
```

The effective directories are the running box's, including for commands
in another terminal. The four addresses identify the local services;
the `fake forgejo` address identifies the **Forgejo at HOST** card.
The three files supply the values to paste below. The program-login
line gives the command already pointing at the sandbox.

Open the box's printed sign-in link in your browser and choose **Set up
temper**. On **Forge**, take one path:

- **Connect GitHub**: follow the fake's App creation and installation
  pages back to temper, then choose `fake-person/widget` to start in.
- **Use an App of your own**: give **App id** `1`, paste the key
  file's text into **Private key**, then choose **Connect the App**.
  Follow the installation link, then choose `fake-person/widget`.
- **Forgejo on this machine**: choose **Choose**, fill in the first
  repository's details, then **Set up a Forgejo here**. Its first use
  downloads Forgejo and its runner if they are not cached.
- **Forgejo at HOST**, matching the printed fake's address, is not on
  the welcome: open **Connect a forge** (`/forges/connect`, or
  **Forges** then **Connect a forge**), choose **Connect as its admin**
  on its card, paste the admin token file's text, fill in the first
  repository's details, and choose **Connect as its admin**. Then open
  `/welcome?at=model` for **Model**.

On **Model**, take one path:

- **Continue with ChatGPT**: the fake signs you in and returns you to
  temper without a real account.
- **Use an API key**: paste the model key file's text.
- **Use your Claude subscription** (the local Claude Code adapter): keep
  or edit **Name**, submit it,
  then run the printed program-login command in another terminal.
  Replace `NAME` with that name and omit the parenthesized reminder;
  keep the printed effective directory flags.

A program's account reaches **Ready**, but temper currently refuses
its first chat: "A chat can't stay open through a program yet." This is
temper's current behaviour, not a limit of the fake.

## 10. From the example, by hand

If you want to assemble the files yourself, use `docs/examples/owned.json`
and `docs/examples/owned/` from your checkout. This example uses forge 1,
`acme/widget`, and ports 3000, 7100 and 7101. Set EXAMPLES to that
checkout's `docs/examples`, TEMPER to your installed binary, and FORGEJO
and RUNNER to the installed Forgejo 17.0.0 and runner 13.2.0 binaries.

```sh
umask 077
mkdir -m 700 temper-box
cd temper-box
cp "$EXAMPLES/owned.json" temper.json
cp -R "$EXAMPLES/owned" .
sed -i -e "s|/home/me/temper|$PWD|g" \
  -e "s|/opt/forgejo/forgejo-runner|$RUNNER|g" \
  -e "s|/opt/forgejo/forgejo|$FORGEJO|g" \
  -e "s|/usr/local/bin/temper|$TEMPER|g" temper.json owned/agent.json
mkdir -m 700 -p secrets
secret=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
printf '%s\n' "$secret" > secrets/worker.secret
printf %s "$secret" | sha256sum | cut -d' ' -f1 > secrets/worker.digest
unset secret
"$TEMPER" box --config "$PWD" --data "$PWD" --cache "$PWD/cache"
```

The box prepares forge 1, starts it and its runner, and prints a sign-in
link. Sign in through ctl as in section 3. This example does not create
a repository: create `acme/widget` on the forge with your chosen tools,
initialized on `main`, with Actions enabled and a workflow using
`temper-host`. Your first Forgejo sign-in uses `forge-1/person_password`
and you can change it on Forgejo when you like. The landing rule covers only
`acme/widget`. This example starts on the binary's stock settings; no
charter instruction files are needed. Read or change the settings as
above, then adopt it:

```sh
temper ctl DIR adopt --project 1 --forge 1 --owner acme --name widget --landing main --key manual-adopt
```

The answer names the adoption. Continue at section 5 for the model and
section 6 for a change; section 7 exports and stops this box too.
