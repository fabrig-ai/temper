# Run temper on a server

Kind: walk

Run temper as a long-lived deployment on a server, behind a TLS proxy,
with Google sign-in, an owned Forgejo and its workers on another host.

This walk sets temper up as a long-lived deployment that people reach
by name: `temper serve` as an engine alone, with an owned Forgejo beside
it, both on loopback behind a TLS proxy; people signing in with Google;
the workers and Forgejo's runner on another host, over WireGuard. It is
redeployed from `main`, and each redeploy may start from an empty
store. Every name here is an example: temper at
`https://temper.example.com`, Forgejo at `https://git.example.com`, the
engine's host `engine` and the workers' host `worker`. The designs are
`docs/design/protocol/sign-in.md` (the address, the proxy, the members),
`docs/design/protocol/configuration.md`, section 2 (an engine alone, a
worker alone) and `docs/design/protocol/setup.md` (the owned forge).

## 1. What runs where

- **On `engine`,** under a service user of its own, `temper`, home
  `/var/lib/temper` (0700, no login shell), which holds `bin/temper`,
  `config/` (the document), `data/` (the store, the
  forge's directory `forge-1/`, `secrets/`) and `cache/` (the fetched
  Forgejo):
  - Forgejo, fetched, prepared and run by temper, at `127.0.0.1:3000`;
  - `temper serve`, an engine alone: the clients' listener at
    `127.0.0.1:8367`, the webhooks' at `127.0.0.1:8368`, the workers'
    at `127.0.0.1:8369`;
  - the TLS proxy, Caddy here, the only thing listening to the world,
    on 80 and 443.
- **On `worker`:** a worker alone, under its own service user, and
  Forgejo's runner, under another with rootless podman for its jobs.
  Each reaches `engine`'s loopback through WireGuard and a forwarder at
  each end, so that every listener and dial stays on loopback, as the
  cluster's shapes want.
- **The firewall on `engine`** admits 22, 80 and 443, and WireGuard's
  UDP port from `worker`'s address alone.

## 2. Build the binary

Build on a machine with room, not on `engine`, at the commit you deploy.
The binary holds its browser, so the build needs Rust's wasm32 target:

```sh
git checkout --detach origin/main
TEMPER_BUNDLE=embed cargo build --release --locked -p temper
```

Copy `target/release/temper` to `engine` and `worker`, with
`docs/examples/owned.json` and `docs/examples/owned/` of the same
commit: the configuration below is made from them, so it follows the
binary's format and figures.

## 3. The engine's configuration

Start from `owned.json` and change these members; keep the rest. The
document goes to `/var/lib/temper/config/temper.json`.

Temper supplies the stock charters, templates, the policy new projects
are made from and the rules. Read these settings after signing in with
`temper ctl me settings`, where `me` is your client directory. An owner
uses `temper ctl me change-settings` to change them; a member can ask
with `temper ctl me propose-settings`. Read `temper help settings` for
the change file, revision and key these commands need.

| Member | Value |
|---|---|
| `engine.hosts` | `{"workers": [{"name": "w1", "digest": "secrets/w1.digest"}], "listener": [{"socket": "127.0.0.1:8369"}]}` |
| `workers`, `application.workers` | `[]`: an engine alone runs no worker |
| `application.clients.listener` | `{"socket": "127.0.0.1:8367"}` |
| `application.clients.public` | `"https://temper.example.com"` |
| `application.webhooks.listener` | `{"socket": "127.0.0.1:8368"}` |
| `application.forges[0]` | `api` `{"socket": "127.0.0.1:3000"}`, `git` `"http://127.0.0.1:3000"` |
| the owned setup's `forgejo` | `"fetched"`: `temper forge fetch` downloads it into the cache |
| its `runner` | `{"binary": "fetched", "labels": ["w1-jobs:docker://docker.io/library/debian:trixie-slim"], "capacity": 1, "path": "/usr/bin:/bin"}`, or `null` for no Actions (section 7) |
| its `listen` | `{"socket": "127.0.0.1:3000"}` |
| its `person` | your Forgejo login and address |
| its `public` | `"https://git.example.com"`: Forgejo's `ROOT_URL`, for its clone URLs and redirects |
| `engine.providers` | add `{"number": 3, "kind": "application"}`, Google's sign-in |
| `engine.owners` | `[{"provider": 3, "subject": "sam@example.com"}]`: the first member (section 5) |
| `application.sign_in` | section 5 |

## 4. The units

Three on `engine`, each `User=temper`, `UMask=0077`,
`ProtectSystem=strict`, `ReadWritePaths=/var/lib/temper`,
`ProtectHome=yes`, `NoNewPrivileges=yes`, and the same three directories
on every command (`$D` below stands for `--config /var/lib/temper/config
--data /var/lib/temper/data --cache /var/lib/temper/cache`):

- **Forgejo:** `ExecStartPre=/var/lib/temper/bin/temper forge fetch $D
  1`, `ExecStartPre=... forge prepare $D 1`, `ExecStart=... forge run $D
  1 forgejo`. The preparation leaves the handover, `admin_token`, in the
  forge's directory whenever it is missing.
- **temper:** `Requires=` and `After=` Forgejo's unit; an
  `ExecStartPre=` that waits until `http://127.0.0.1:3000/api/v1/version`
  answers, since the engine takes the handover at its start;
  `ExecStart=/var/lib/temper/bin/temper serve $D`.
- Nothing on `engine` runs `temper forge run 1 runner`: the runner
  lives on `worker` (section 7).

## 5. Sign in with Google

Members sign in with Google; temper mints for a configured member alone
(`docs/design/protocol/sign-in.md`, section 3).

1. **Google's client.** In Google's console, a project with its OAuth
   consent in **Testing**, your members as its test users, and the
   scopes `openid`, `email` and `profile` alone. Then a client of type
   **Web application**, no JavaScript origin, and one redirect URI,
   `https://temper.example.com/sign-in/google/return`. Its id is not a
   secret; its secret goes only into the file of step 2.
2. **The secret's file,** by your own hand, on `engine`, never through
   a chat or a copied file. It survives every redeploy (section 8):

   ```sh
   sudo -u temper bash -c 'umask 077; mkdir -p /var/lib/temper/data/secrets; IFS= read -rsp "Google client secret: " s; echo; printf %s "$s" > /var/lib/temper/data/secrets/google-client.secret; unset s'
   sudo stat -c '%U %a %s' /var/lib/temper/data/secrets/google-client.secret
   ```

   temper refuses to start when the file is readable by anyone but its
   user, or reached through a link. `secret` is a data path: relative to
   the data directory, as below, or absolute, for a file kept outside it.
3. **The configuration:**

   ```
   "sign_in": {
     "google": {"provider": 3,
                "client": "1234-abc.apps.googleusercontent.com",
                "secret": "secrets/google-client.secret",
                "endpoints": null},
     "members": [{"address": "sam@example.com", "login": "sam"}]
   }
   ```

   Each of `engine.owners` of provider 3 must be a member, and there
   must be one at least, or the start is refused, naming
   `engine.owners`. The first is whom the printed link signs in
   (section 9).
4. **A member's first sign-in** binds their address to their Google
   account; another Google account with that address is refused
   afterwards. A redeploy from an empty store binds afresh. Adding a
   member is a change to the document and a restart.

## 6. The TLS proxy

Caddy obtains and renews the certificates and redirects HTTP to HTTPS
by itself. `/etc/caddy/Caddyfile`:

```
git.example.com {
    reverse_proxy 127.0.0.1:3000
}

temper.example.com {
    # The printed link's route is the host's alone.
    @link path /jig/v1/sign-in/link /jig/v1/sign-in/link/*
    respond @link 403
    reverse_proxy 127.0.0.1:8367 {
        # jig's stream, unbuffered, its read longer than its heartbeat.
        flush_interval -1
        transport http {
            read_timeout 10m
        }
    }
}
```

- **The `Host` goes through unchanged,** Caddy's default: temper admits
  its public host beside its listener's address, and sets `Secure` on
  its cookies by it. Never rewrite it to the listener's address: the
  cookies would lose `Secure`.
- **Nothing else in the transport:** `read_timeout 10m` outlasts jig's
  stream's heartbeat. Leave Caddy's upstream keepalive at its default,
  and add no retry (`lb_try_duration`): temper answers a new connection
  however long it was idle.
- **Check from outside:** `https://temper.example.com/` answers 200;
  `POST https://temper.example.com/jig/v1/sign-in/link` answers 403;
  `https://temper.example.com/sign-in/google` answers 303 to
  `accounts.google.com`, its `redirect_uri` the one you registered, its
  cookie `Secure`.

## 7. The workers' host over WireGuard

1. **The tunnel:** WireGuard between the two hosts, `engine` at
   `10.88.0.1/32` listening on UDP 51820, `worker` at `10.88.0.5/32`
   dialling it with a keepalive; each host's private key made on that
   host, only the public keys exchanged; `AllowedIPs` the other's /32
   alone, nothing routed.
2. **The forwarders,** systemd's `systemd-socket-proxyd` behind a socket
   unit, `DynamicUser=yes`:
   - on `engine`: `10.88.0.1:8369` to `127.0.0.1:8369` (the engine's
     workers' listener) and `10.88.0.1:3000` to `127.0.0.1:3000`
     (Forgejo), each socket with `FreeBind=yes`, bound before the
     tunnel has its address; the firewall admits them from `10.88.0.5`
     on the tunnel's interface alone;
   - on `worker`: `127.0.0.1:8369` to `10.88.0.1:8369`, and
     `127.0.0.1:3000` to `10.88.0.1:3000`.
3. **The worker's secret and its digest,** as step 3 of `temper help box`
   makes them: the secret in the worker's data,
   `secrets/w1.secret`; its digest in the engine's,
   `/var/lib/temper/data/secrets/w1.digest`.
4. **The worker alone,** a unit on `worker` running `temper serve` on
   its own three directories, whose document is the example's worker
   part alone, and `application`'s `workers` and `limits` alone:
   `workers[0]` with `engine` `{"socket": "127.0.0.1:8369"}`, `name`
   `"w1"`, `secret` `"secrets/w1.secret"`, and `agent` naming
   temper's binary and its `agent.json` by absolute paths. The example's
   worker `memory_bytes`, with its headroom, also holds a worker alone's
   worst case with its strings at their bounds. It is a bound the start
   checks, and reserves nothing. If a changed configuration's start is
   refused, it names by how many bytes the worst case passes the bound:
   raise `memory_bytes` past that.
5. **The runner,** when the owned setup has one: Forgejo's preparation
   on `engine` registers it and writes `runner/secret` and
   `runner/config.yaml` in the forge's directory; it draws a new secret
   only when the runner is added or its version moves. On `worker`,
   Forgejo runner 13.2.0's daemon runs under its own user, its
   connection `url` `http://127.0.0.1:3000` (the forwarder), its `uuid`
   that of `engine`'s `runner/config.yaml`, its `token_url` a file
   holding a copy of `runner/secret`. Its jobs run in rootless podman
   (`DOCKER_HOST` the user's podman socket), with
   `container.docker_host: "-"`, `valid_volumes: []`, no privileges and
   the cache off; the label's `docker://` image is the job's. Never run
   `forgejo-runner exec` on a host with public addresses: it opens its
   cache server on them, whatever its configuration says.

## 8. Redeploy from main

A redeploy starts temper from an empty store; Forgejo is kept unless
you reset it too.

**First, the families.** Until temper's first release, a family keeps
its version number and its golden bytes change in place (skein's
`docs/design/codec.md`, section 5): nothing an older build wrote is
translated. Before a redeploy, list the families whose golden bytes
differ between the running build's commit and the new one: in temper,
and, for each of jig, skein and smith whose pin in `Cargo.lock` moved,
in that repository between its two pins.

```sh
git diff --stat LIVE NEW -- ':(glob)crates/*/golden*/**'
```

- **The store's** need nothing: the store starts afresh at each
  redeploy (step 4).
- **The clients' and the workers' wires** move with the build: the page
  is embedded in the binary, and the workers are redeployed from the
  same one.
- **The file `temper ctl export` writes** (section 10) is what crosses a
  redeploy. Its two families have no golden directory: jig's moved
  form (`crates/jig-protocol-accounts/src/moved.rs`) and skein's OAuth
  record inside it (`crates/skein-oauth/src/record.rs`), so diff those
  between the pins too. If either changed, the import after the start
  refuses the old file, and an owner signs in to the account again from
  **Accounts**. Let the refusal stand: an old file is never read as a
  new one.

1. **Build** at `main` (section 2), and remake both documents from the
   new commit's example (sections 3 and 7).
2. **Export** the LLM account from the running engine (section 10),
   then **stop** temper's and Forgejo's units on `engine`, and the
   worker and runner on `worker`.
3. **Install** the binary and the documents.
4. **Reset** what may be lost: on `engine`, everything in `data/` but
   `secrets/` and the forge's directory; on `worker`, everything in the
   worker's `data/` but `secrets/`. With the forge's directory removed
   too, Forgejo starts empty, and its runner's secret is new: copy it to
   `worker` again.
5. **Start** Forgejo, then temper; then the worker and the runner. The
   preparation leaves a new handover; the engine takes it over,
   reconnects to the kept Forgejo and remakes its bot's tokens. Once
   temper answers, **import** the account (section 10).
6. **Check:** both names over HTTPS, the link's route refused from
   outside, Google's start (section 6), the worker linked
   (`journalctl -u` its unit shows `linked`), the runner online in
   Forgejo's site administration.

What survives: Forgejo and its repositories, the secrets' files, and
the members, which are configuration. What is lost: the store, so every
sign-in, chat and task, the members' bindings, and every LLM account
but the signed-in ones exported and imported (section 10).

## 9. The printed link, for recovery

temper prints a sign-in link at each start, in its unit's journal
(`journalctl -u` temper's unit), valid five minutes and once. It is a
credential: never paste it anywhere but your browser. It signs in the
first of `engine.owners`, the first member. With `clients.public` set,
the link names the public origin, whose route the proxy refuses: open
it through a tunnel to the listener instead, with the same port on your
side, so that the `Host` is one temper admits:

```sh
ssh -L 8367:127.0.0.1:8367 -o ExitOnForwardFailure=yes engine
```

and in your browser the link with `https://temper.example.com` replaced
by `http://127.0.0.1:8367`. You are then signed in at that address, not
at the public name. A restart of temper's unit prints a fresh link and
keeps the store.

## 10. The LLM account across a redeploy

The LLM accounts are in the store, which starts afresh at each redeploy
(section 8). A signed-in account crosses it by `temper ctl`
(`docs/design/client/tools.md`, 5.5, area 5): exported from the running
engine last before the stop, imported into the new one once it answers.
A key's account does not cross: add the key again from **Accounts**.
Run `temper ctl` on `engine` as temper's user (`sudo -u temper
/var/lib/temper/bin/temper ctl`, written `temper ctl` below), with a
directory of its own, `0700`, made empty each time (`DIR`):

1. **Sign in** by the link temper printed (section 9), its origin
   swapped for the listener's, on stdin, so that it is never an
   argument or shown:

   ```sh
   sudo journalctl -u temper -o cat | grep -o 'https://temper.example.com/jig/v1/sign-in/link?[^ ]*' \
       | tail -1 | sed 's#^https://temper.example.com#http://127.0.0.1:8367#' \
       | sudo -u temper /var/lib/temper/bin/temper ctl DIR sign-in -
   ```

   It signs in the first of `engine.owners`. A link spent or expired is
   refused, and temper prints a fresh one: run it again.
2. **Before the stop,** `temper ctl DIR accounts --json` names each
   account and its sources. For each with a granted `sign_in` source,
   `temper ctl DIR export N FILE`, FILE a new path in
   `/var/lib/temper/data/secrets/`, which the reset keeps: it prints
   `moved out: account N to FILE`, FILE `0600`, and the running engine
   has let the account go. Stop then, with nothing in between.

   Read `accounts --json` once before the redeploy begins, too: it is
   the dry run. Only an account with a granted `sign_in` source crosses.
   A connect not finished is a row of `signing` (its `source`, no
   number), and an account whose sign-in is not granted stays behind
   too. If the account you expect is not there granted, finish its
   sign-in from **Accounts** first. Don't wait for `temper ctl DIR wait
   accounts --until settled`: a connect in progress keeps the accounts
   unsettled until it ends.
3. **After the start,** sign in again by the new engine's link (step
   1), then `temper ctl DIR import FILE --key NAME`, NAME a word of `a`
   to `z`, `0` to `9` and `-`: it prints `moved in: account N` and
   removes FILE. Then `temper ctl DIR sign-out`, and remove `DIR`.

FILE is a credential, the account's only holder between the two steps:
check it by `stat` alone, and never print it, copy it off `engine` or
commit it. Nothing here stops a redeploy; each of these ends is said, and
an owner signs in to the account again from **Accounts**:

- **the running build has no `export`** (`temper ctl help export`
  fails): nothing crosses;
- **an export refused or its record lost on its way:** the engine kept
  the account, and the store loses it; FILE is removed, or, if `temper
  ctl` kept it, import it;
- **an import refused** ("not ours", "held", or the file's family
  changed: section 8): FILE is kept. Leave it as it is, and never edit
  or translate it.

ChatGPT's sign-in returns to `127.0.0.1:1455` on `engine`: from your
browser's machine, keep `ssh -L 1455:127.0.0.1:1455 engine` open while
you sign in.
