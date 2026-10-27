# temper ctl forge

Kind: reference

Forges: read them, set one up, connect it, accept an installation, read what it reaches.

## forge list

```
forge list [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: one of settled, changed, connected:N or made:N with N from 1 to 65535; optional. Wait until it holds: settled, connected:FORGE, made:FORGE, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read the deployment's forges
Prints: A line per forge, `forge N`, its kind, mode and connection, its App and installations, then `digest: forges`
--json: the record forges.
Key: none.
An installation not yet accepted prints the `forge accept FORGE
INSTALLATION` that accepts it.
```

## forge setup

```
forge setup FORGE [--org ORG] [--repository REPOSITORY] [--project PROJECT] [--playground] [--key KEY]
  FORGE (forge): a number from 1 to 65535; required. The owned Forgejo's forge number
  --org ORG: text, at most 1048576 bytes; optional. The organization; work when absent
  --repository REPOSITORY: text, at most 1048576 bytes; optional. The first repository; hello when absent
  --project PROJECT: a project's name or number; optional. Adopt into this project; a new project named after the repository when absent
  --playground: a switch. Make the playground's repository, playground/widget, instead; no --org or --repository
Set up the owned Forgejo on this machine, with a first repository
Prints: `setting up`, then, once Forgejo runs, the adoption of its first repository
--json: the record forge-setup.
Key: --key KEY, optional.
It waits up to 300 seconds while the box fetches, prepares and
starts Forgejo and its runner, then adopts the repository as the
project's home, landing on main, with CI. Past the wait, it prints
the forges and `not met: timeout` (exit 3): wait with forge list
--until made:FORGE, then read it. `refused: not under the box` (exit
2): the deployment is no box, and the refusal names the commands to
run instead.
```

## forge connect

```
forge connect FORGE github-app APP FILE --key KEY
  FORGE (forge): a number from 1 to 65535; required. GitHub's forge number
  github-app (way): one of github-app; required. An App of your own, or one another owns
  APP (id): a number from 1 to 18446744073709551615; required. The App's id
  FILE (file): a file; required. The private file of the App's PEM key
forge connect FORGE github --key KEY
  FORGE (forge): a number from 1 to 65535; required. GitHub's forge number
  github (way): one of github; required. temper's App, made in a browser from its manifest
forge connect FORGE forgejo FILE --key KEY
  FORGE (forge): a number from 1 to 65535; required. The Forgejo's forge number
  forgejo (way): one of forgejo; required. Not offered yet
  FILE (file): text, at most 1048576 bytes; required. Not offered yet; no file is read
forge connect FORGE forgejo-admin FILE [--first ORG/REPOSITORY] [--project PROJECT] [--label LABEL] --key KEY
  FORGE (forge): a number from 1 to 65535; required. The Forgejo's forge number
  forgejo-admin (way): one of forgejo-admin; required. As the Forgejo's site admin
  FILE (file): a file; required. The private file of the admin's token
  --first ORG/REPOSITORY: text, at most 1048576 bytes; optional. Make this first repository, ORG/NAME
  --project PROJECT: a project's name or number; optional. Adopt it into this project; a new one named after it when absent
  --label LABEL: text, at most 1048576 bytes; optional. The runner's label; docker when absent
Connect GitHub through an App, or a Forgejo as its site admin
Prints: `connecting`, then the wait to make; for github, the address to open in a browser
--json: the record forge-connect.
Key: --key KEY, required.
The key or token file holds 1 to 4089 bytes after one final
newline is trimmed. An App another owns is set read-only in the
configuration before the start (temper help github). Then forge
list --until connected:FORGE, or, after a browser, --until settled.
With `--first`, it goes on to make and adopt that repository, and
prints its project. `connected already` (exit 2): the forge holds a
key, which this build cannot replace.
```

## forge install

```
forge install FORGE [--key KEY]
  FORGE (forge): a number from 1 to 65535; required. GitHub's forge number
Print a fresh link that installs GitHub's App on another account
Prints: `Open this address in a browser on the engine's machine:`, the address, then `forge list --until settled`
--json: the record forge-install.
Key: --key KEY, optional.
An installation made through this link is accepted. One made on
GitHub without it is accepted with forge accept.
```

## forge repositories

```
forge repositories FORGE [--installation INSTALLATION] [--page PAGE]
  FORGE (forge): a number from 1 to 65535; required. The forge's number
  --installation INSTALLATION: a number from 1 to 18446744073709551615; optional. Only this accepted installation, by its id
  --page PAGE: a number from 1 to 4294967295; optional. The page, from 1; 1 when absent
Read a page of the repositories a forge reaches
Prints: A line per repository, `OWNER/NAME`, whether private, its id and its owner's id, then the next page's call or `No more repositories follow.`
--json: the record repositories.
Key: none.
On GitHub, name the installation whose repositories you want: forge
list prints its id.
```

## forge accept

```
forge accept FORGE INSTALLATION [--key KEY]
  FORGE (forge): a number from 1 to 65535; required. GitHub's forge number
  INSTALLATION (installation): a number from 1 to 18446744073709551615; required. The installation's id, as forges prints it
Accept a GitHub App installation made already
Prints: `accepted: installation N on forge N`, then the forge repositories call to make
--json: the record forge-accept.
Key: --key KEY, optional.
It needs no browser, and an installation accepted already answers
the same. `GitHub has no installation I of this App` (exit 2): read
forge list for the ids.
```
