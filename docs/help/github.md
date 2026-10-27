# Connect GitHub

Kind: walk

Connect a box to GitHub through a GitHub App, accept or make its installation, and adopt one of its repositories, with no browser when the App and its installation exist already.

## Before you start

- **A box's directories,** as temper help box makes them in its step
  1. GitHub is forge 2 on a configuration `temper init` wrote.
- **The App,** one of:
  - an App another owns, such as a team's or one kept for tests:
    temper must leave it as it is, so it is made read-only before the
    start (step 1);
  - an App of your own: its id and a private key GitHub made for it;
  - temper's App, made from its manifest: a browser on the engine's
    machine makes and installs it.
- **The App's private key** in `FILE`: regular, owned by you, mode
  0600.
- `APP`: the App's id, from its page on GitHub. On fake backends, the
  box prints `box: GitHub own App id 1, key file FILE`: `APP` is 1 and
  `FILE` that file.
- `OWNER`: the repository's owner, as `forge repositories` prints it.
- `NAME`: the repository's name, as `forge repositories` prints it.
- `REQUEST`: a file of the words that ask for a change in it.
- `CHANGE`: the change task's number, as the chat's tree lists it.

## 1. Make a shared App read-only, before the start

For an App another owns alone. On real backends, before the box's
first start:

```
jq '(.application.forges[] | select(.kind == "github")) += {"app_read_only": true}' CONFIG/temper.json > CONFIG/new.json
mv CONFIG/new.json CONFIG/temper.json
```

On fake backends, the box refuses a `CONFIG` changed after `temper
init`: start the box once, stop it, make the same change in
`DATA/box/fake-backends/config/temper.json`, give it mode 0600 again,
then go on.

Read-only, temper reads the App's hook and leaves it, uninstalls no
installation it did not accept, and changes nothing of the App.

Expect: no output; GitHub's forge in the file holds
`"app_read_only": true`.

If not: a start that says `app_read_only: not in current init shape`
is a box on fake backends given the change in `CONFIG`: put it in the
effective configuration instead.

## 2. Start the box and sign in

```
temper box --config CONFIG --data DATA --cache CACHE
temper ctl DIR sign-in -
```

On fake backends, start the box as temper help box says in its step
2. Give the link on stdin.

Expect: `party 1 · owner · Owner`, the owner of a new box.

If not: as temper help box says in its steps 2 and 3.

## 3. Connect the App

An App that exists, its key read from `FILE`:

```
temper ctl DIR forge connect 2 github-app APP FILE --key KEY
temper ctl DIR forge list --until connected:2
```

temper's App, made from its manifest: open the address it prints in
a browser on the engine's machine, create the App, then install it:

```
temper ctl DIR forge connect 2 github --key KEY
temper ctl DIR forge list --until settled
```

Expect: `connecting`; the wait prints the forges, the first line
`forge 2 · GitHub · connected · ` and its state, its installations
below it, then `met`.

If not: `credential file must contain 1 to 4089 bytes` or a key
file's mode (exit 1): mend `FILE`, then the same call with the same
key. `connected already` (exit 2): forge 2 holds a key, which this
build cannot replace. An App short of a permission or an event
connects, and `forge list` names what it lacks: grant it in the App's
settings on GitHub.

## 4. Accept an installation

An installation made already, by its id as `forge list` lists it:

```
temper ctl DIR forge list
temper ctl DIR forge accept 2 INSTALLATION
```

Or a new one, on another account, through a browser on the engine's
machine; on fake backends, any HTTP client completes it:

```
temper ctl DIR forge install 2 --key KEY
temper ctl DIR forge list --until settled
```

Expect: `accepted: installation INSTALLATION on forge 2`, then the
`forge repositories` call to make; or the forges, the new installation
listed, then `met`.

If not: `GitHub has no installation INSTALLATION of this App` (exit
2): read `forge list` for its ids. On fake backends, the installation
listed right after the connect answers so: install through
`forge install` instead. Without read-only mode, an installation not
accepted is uninstalled at the next reconcile: accept it first.

## 5. Adopt a repository

```
temper ctl DIR forge repositories 2 --installation INSTALLATION
temper ctl DIR repository adopt --project PROJECT --forge 2 --owner OWNER --name NAME --landing main --no-home --key KEY
```

`--landing` names the branch changes land on. `--no-home` keeps the
home the project has, the box's Forgejo repository; without one,
leave it out, and the repository becomes the home.

Expect: a line per repository, `OWNER/NAME`, then `No more
repositories follow.`; then `Adopted OWNER/NAME on forge 2 as
owned.`

If not: `The project has a home already.` (exit 2): it prints the call
with `--no-home`; make it with a new key. `No repository OWNER/NAME on
forge 2 that temper's forge user can see.` (exit 2): the installation
does not reach it; grant it to the App on GitHub's installation page.

## 6. Work on the repository

Name `OWNER/NAME` in the request:

```
temper ctl DIR chat start --project PROJECT - --key KEY < REQUEST
temper ctl DIR change show CHANGE --until landed --timeout 3600
```

The chat's tree names the change task, as temper help box says in its
step 6.

Expect: `done: task TASK, chat`; then the change's panel, its pull
request on GitHub, and `met`.

If not: as temper help box says in its steps 6 and 7.

## Clean up

- Export a signed-in account, then stop the box, as temper help box
  says in its step 8.
- Delete the App's key file once forge 2 is connected: temper keeps
  the key, and GitHub makes a new one on its App page.
- Leave the installations you did not accept to their owners.

## Next

- temper help box: the box, and a change on its own Forgejo.
- temper help ctl: the loop, keys and exit statuses for every call.
