# Connect a Forgejo

Kind: walk

Connect a deployment to a Forgejo it does not run, as its site admin, so that temper makes and keeps its own bot there, then adopt a repository.

## Before you start

- **A deployment,** its directories made as temper help box says in
  its step 1, before its start.
- **The mode.** Managed, this walk: you give a site admin's token,
  and temper makes its bot, the bot's tokens, and at each adoption
  the bot's access and the repository's hook. Owned: temper runs the
  Forgejo itself; on a box, temper help box says how in its step 4,
  and on a server, temper help server. Connected, a bot's tokens given
  by hand, is not offered: `forge connect FORGE forgejo` is refused.
- **The admin's token** in `FILE`: regular, owned by you, mode 0600,
  made in a site admin's settings with the scopes `write:admin`,
  `read:user` and `write:repository`.
- `FORGE`: the forge's number in the configuration, kept for good: a
  forge that changes its number is a new forge.
- `ORG`: the organization of the first repository temper makes.
- `NAME`: that repository's name.
- `OWNER`: another repository's owner, as `forge repositories` prints it.
- `REPOSITORY`: that repository's name.
- **On fake backends,** the box's Forgejo elsewhere is forge 4,
  configured managed already: skip step 1. The box prints `box:
  Forgejo elsewhere admin token file FILE`.

## 1. Configure the forge, before the start

Add to `CONFIG/temper.json` the forge, its identity and its setup
entry, the address the Forgejo answers on in place of `ADDRESS`:

```
jq '.application.forges += [{"number": FORGE, "kind": "forgejo", "api": {"socket": "ADDRESS"}, "transport": "plaintext", "git": "http://ADDRESS", "web": null}]
  | .application.identities += [{"number": FORGE, "forge": FORGE, "login": "temper", "author": {"name": "temper", "email": "temper@localhost"}}]
  | .application.setup += [{"forge": FORGE, "bot": {"login": "temper", "email": "temper@localhost"}, "mode": "managed"}]' CONFIG/temper.json > CONFIG/new.json
mv CONFIG/new.json CONFIG/temper.json
```

A plaintext forge is on loopback; one elsewhere takes a TLS
`transport`, `{"tls": {"server_name": HOST, "trust": null}}`, and an
`https` git address.

Expect: no output; the three entries in the file.

If not: a start that names `application.setup` found no setup entry
for the forge: add it, then start again.

## 2. Start and sign in

```
temper box --config CONFIG --data DATA --cache CACHE
temper ctl DIR sign-in -
```

On fake backends, or with `temper serve`, start as temper help box or
temper help server says. Give the link on stdin.

Expect: `party 1 · owner · Owner` on a new deployment.

If not: as temper help box says in its steps 2 and 3.

## 3. Connect as its admin

```
temper ctl DIR forge connect FORGE forgejo-admin FILE --first ORG/NAME --key KEY
temper ctl DIR forge list --until made:FORGE
```

`--first` makes the first repository in a new project named after
it, and adopts it; without it, the forge connects with no repository.

Expect: `connecting`, then `project N`, the new project, then
`Adopted ORG/NAME on forge FORGE as owned.` and `It is the project's
home.`; the wait prints the forges, forge `FORGE` `managed`, then
`met`.

If not: `credential file must contain 1 to 4089 bytes`, or a key
file's mode (exit 1): mend `FILE`, then the same call with the same
key. A token without `write:repository` connects, and the adoption
says that scope is missing: give it, then connect again. `LOGIN is
not temper's bot` (in `forge list`): a user of that login exists; add
`temper:bot` to its full name on Forgejo, then connect again.

## 4. Adopt another repository

```
temper ctl DIR forge repositories FORGE
temper ctl DIR repository adopt --project PROJECT --forge FORGE --owner OWNER --name REPOSITORY --landing main --key KEY
```

`forge repositories` lists every repository the admin sees; `PROJECT` is
the project step 3 printed.

Expect: a line per repository, `OWNER/REPOSITORY`; then `Adopted
OWNER/REPOSITORY on forge FORGE as owned.`

If not: `temper's forge user may not read OWNER/REPOSITORY on forge
FORGE.` (exit 2): the bot has no access to it; make it a collaborator
with write on Forgejo, then adopt again with a new key. `The project
has a home already.` (exit 2): add `--no-home`, with a new key.

## Clean up

- Delete `FILE` once the forge is connected: temper keeps the token.
- temper keeps the admin's token while the forge is managed; ctl has
  no call to forget it.

## Next

- temper help box: a change asked and followed to its landing.
- temper help ctl: the loop, keys and exit statuses for every call.
