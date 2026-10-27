# Run temper on a server

Kind: walk

Run a long-lived deployment on a server: the engine behind a TLS proxy with Google sign-in, an owned Forgejo beside it, the workers on a second host, and a redeploy that keeps the model's accounts.

## Before you start

- **Two hosts:** `engine`, which faces the world (the proxy, the
  engine, Forgejo), and `worker` (the worker, Forgejo's runner), joined
  by WireGuard. On `engine`, a service user `temper`, home
  `/var/lib/temper` (0700, no login shell): `CONFIG`, `DATA` and
  `CACHE` are its `config/`, `data/` and `cache/`.
- **Two names,** one for temper and one for Forgejo, here
  `temper.example.com` and `git.example.com`.
- **Google's client:** a project whose OAuth consent is in Testing,
  your members its test users, the scopes `openid`, `email` and
  `profile`; a Web application client whose one redirect URI is
  `https://temper.example.com/sign-in/google/return`.
- **The binary,** built at the commit you deploy, with its browser:
  `TEMPER_BUNDLE=embed cargo build --release --locked -p temper`,
  which needs Rust's `wasm32-unknown-unknown` target; copy
  `target/release/temper`, `docs/examples/owned.json` and
  `docs/examples/owned/` of the same commit to both hosts.

## 1. Write the engine's configuration

From `owned.json`, at `CONFIG/temper.json`, these members changed and
the rest kept:

- `engine.hosts`: `{"workers": [{"name": "w1", "digest":
  "secrets/w1.digest"}], "listener": [{"socket": "127.0.0.1:8369"}]}`;
  `workers` and `application.workers`: `[]`, an engine alone;
- `application.clients`: `listener` `{"socket": "127.0.0.1:8367"}`,
  `public` `"https://temper.example.com"`;
  `application.webhooks.listener`: `{"socket": "127.0.0.1:8368"}`;
- the owned forge: `api` `{"socket": "127.0.0.1:3000"}`, `git`
  `"http://127.0.0.1:3000"`; its setup's `forgejo` `"fetched"`, its
  `runner` `{"binary": "fetched", "labels":
  ["w1-jobs:docker://docker.io/library/debian:trixie-slim"],
  "capacity": 1, "path": "/usr/bin:/bin"}` or `null` for no CI,
  `listen` `{"socket": "127.0.0.1:3000"}`, `person` your Forgejo login
  and address, `public` `"https://git.example.com"`;
- `engine.providers` gains `{"number": 3, "kind": "application"}`,
  Google's; `engine.owners`: `[{"provider": 3, "subject":
  "sam@example.com"}]`;
- `application.sign_in`: `{"google": {"provider": 3, "client":
  "CLIENT_ID", "secret": "secrets/google-client.secret", "endpoints":
  null}, "members": [{"address": "sam@example.com", "login": "sam"}]}`.

Every owner of provider 3 is a member. Adding a member is a change of
this file and a restart.

Expect: the file, valid JSON; nothing runs yet.

If not: the start in step 3 names the member it refuses.

## 2. Keep the secrets

Google's client secret, typed by you as `temper` with nothing echoed,
never through a chat or a copied file:

```
sudo -u temper bash -c 'umask 077; mkdir -p DATA/secrets; IFS= read -rsp "Google client secret: " s; echo; printf %s "$s" > DATA/secrets/google-client.secret; unset s'
```

The worker's secret on `worker`, in its data's `secrets/w1.secret`, and
its digest, the SHA-256 of its digits without the line end, copied to
`engine`'s `DATA/secrets/w1.digest`:

```
umask 077; s=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n'); printf '%s\n' "$s" > secrets/w1.secret; printf %s "$s" | sha256sum | cut -d' ' -f1 > w1.digest; unset s
```

Expect: `stat -c '%U %a' FILE` prints `temper 600` for each.

If not: temper refuses to start on a secret's file that another user
may read, or that is a link: give it to `temper`, mode 0600.

## 3. Start Forgejo, then the engine

Units under `temper`, each with `UMask=0077`, `ProtectSystem=strict`,
`ReadWritePaths=/var/lib/temper`, `NoNewPrivileges=yes`. Forgejo's:

```
temper forge fetch --config CONFIG --data DATA --cache CACHE 1
temper forge prepare --config CONFIG --data DATA --cache CACHE 1
temper forge run --config CONFIG --data DATA --cache CACHE 1 forgejo
```

The engine's, after Forgejo's `http://127.0.0.1:3000/api/v1/version`
answers, since it takes Forgejo's handover at its start:

```
temper serve --config CONFIG --data DATA --cache CACHE
```

Expect: `fetch` and `prepare` print nothing and exit 0; `run` is
Forgejo's own output; `serve` prints its sign-in link,
`https://temper.example.com/jig/v1/sign-in/link?secret=` and its
secret, and on stderr `listening clients 127.0.0.1:8367`.

If not: a refused start names its member, file or address. A forge
not prepared: `serve`'s stderr says
`temper serve: forge 1: Not connected: prepare it with` the command;
run `prepare`, then start the engine again.

## 4. The TLS proxy

Caddy, the only listener facing the world, on 80 and 443:

```
git.example.com {
    reverse_proxy 127.0.0.1:3000
}
temper.example.com {
    @link path /jig/v1/sign-in/link /jig/v1/sign-in/link/*
    respond @link 403
    reverse_proxy 127.0.0.1:8367 {
        flush_interval -1
        transport http {
            read_timeout 10m
        }
    }
}
```

Keep the `Host` header as it comes: temper sets `Secure` on its
cookies by it. Add no retry.

Expect: `https://temper.example.com/` answers 200; a `POST` to
`/jig/v1/sign-in/link` answers 403; `/sign-in/google` answers 303 to
`accounts.google.com` with your redirect URI.

If not: a 502 is the engine down or its listener elsewhere: read its
unit's journal.

## 5. The workers' host

- WireGuard: `engine` at `10.88.0.1/32` on UDP 51820, `worker` at
  `10.88.0.5/32` dialling it; `AllowedIPs` the other's /32 alone.
- Forwarders, `systemd-socket-proxyd`: on `engine`, `10.88.0.1:8369`
  and `10.88.0.1:3000` to the same ports on 127.0.0.1, with
  `FreeBind=yes`, admitted from `10.88.0.5` alone; on `worker`,
  `127.0.0.1:8369` and `127.0.0.1:3000` to `10.88.0.1`.
- The worker alone: `temper serve` on `worker`'s own three
  directories, its document the example's worker part, `workers[0]`
  with `engine` `{"socket": "127.0.0.1:8369"}`, `name` `"w1"`,
  `secret` `"secrets/w1.secret"`, `agent` naming the binary and
  `agent.json` by absolute paths.
- The runner, Forgejo runner 13.2.0 under a user of its own, its
  `url` `http://127.0.0.1:3000`, its `uuid` and secret those of
  `DATA/forge-1/runner/`; its jobs in rootless podman.

```
temper serve --config CONFIG --data DATA --cache CACHE
```

Expect: on `worker`, the worker's stderr says `linked w1` and the
engine's address; Forgejo's site administration lists the runner
online.

If not: a start refused for its memory names by how many bytes the
worst case passes `memory_bytes`: raise it past that.

## 6. Sign in, through a tunnel

The link names the public origin, whose route the proxy refuses: give
it to ctl with the listener's origin in its place, on `engine`, as
`temper`:

```
journalctl -u temper -o cat | grep -o 'https://temper.example.com/jig/v1/sign-in/link?[^ ]*' | tail -1 | sed 's#^https://temper.example.com#http://127.0.0.1:8367#' | temper ctl DIR sign-in -
```

A person's browser opens the same link through `ssh -L
8367:127.0.0.1:8367 engine`.

Expect: `party 1 · sam · ` and the party's name: the first of
`engine.owners`.

If not: `temper ctl: the sign-in link was refused`: it was spent or
expired, and temper printed a fresh one: run the line again.

## 7. Redeploy from main

A redeploy starts the store afresh; Forgejo, the secrets and the
members stay. Before the stop, export each account whose source is
`sign_in` and `granted` to a file the reset keeps:

```
temper ctl DIR account list
temper ctl DIR account export ACCOUNT FILE
```

Then stop the units; install the new binary and documents; remove
`DATA` but `DATA/secrets/` and `DATA/forge-1/`, and on `worker` its
data but `secrets/`; start Forgejo, the engine, the worker and the
runner; sign in again (step 6); then:

```
temper ctl DIR account import FILE --key KEY
```

Expect: `moved out: account ACCOUNT to FILE`; after the start,
`moved in: account ACCOUNT`, and `FILE` removed.

If not: an export refused or lost: the engine kept the account, and
the store loses it; an owner signs in to it again. An import refused
keeps `FILE`: leave it as it is. A key's account does not cross: add
its key again. A worker's document from an older `temper init` stops the
worker's start at `agent.arguments`,
`"agent" is now "internal", "agent"`: write them
`["internal", "agent", PATH]`, `PATH` its `agent.configuration`.
A running build from before 10-27 answers `account list` with ctl's usage line (exit 1): before the stop, use its own names, `accounts` and `export`.

## Clean up

- `temper ctl DIR sign-out`, then remove `DIR`.
- Check an exported `FILE` by `stat` alone; never print it or copy
  it off `engine`.

## Next

- temper help ctl: the loop, keys and exit statuses for every call.
- temper help forgejo: a Forgejo temper does not run.
