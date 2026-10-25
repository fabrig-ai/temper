# The checkout's io: git

Provisional, 2026-10-14, revised 2026-10-20. How the worker's protocol
layer runs the checkout's operations (`../domain/worker.md`, section 5):
the git component, `temper-worker-protocol-git`. For each operation of
`temper-worker-domain-checkout`'s `git::Op`, the fixed sequence of git
invocations it runs, their arguments and environment, how its terminal is
read from what git says, and how its failures are classified; then the
processes, deadlines and cancel, the limits, and how it is tested. The
mechanics are skein's `programming-model.md` (sections 4 and 9) and io's
processes (skein's `io.md`, section 6). The credentials it hands git are
credentials.md's (section 4). What is open is listed in section 10.

## 1. In one page

- **One component, every operation.** The worker's root asks for a
  `git::Op` with a deadline (`Request::Io`) and may cancel it
  (`Request::CancelIo`); the git component runs it and answers with
  exactly one `Event::Done`. It is step code, a child of the worker's
  protocol layer, with its own limits and worst case (README.md,
  section 5).
- **A fixed sequence per operation:** at most four git invocations, each
  its own process in a process group of its own, all under the
  operation's one deadline. Which invocation comes next depends only on
  what the last one said, never on a retry: the component never retries
  (README.md, section 5).
- **git as it ships, told exactly what to do.** git is run by an absolute
  path: the one a repository's item names, a fake forge's program
  (channel.md, section 4), or else `workers[].git` (configuration.md,
  section 4); with neither, the invocation fails, naming the forge's host.
  It runs in an environment given whole, with settings on the command line
  that turn off what could run another program or outlive the invocation:
  hooks, credential helpers, the file system monitor, background
  maintenance, every transport but HTTPS and HTTP.
- **Credentials in the environment only.** An operation that reaches the
  forge gets its git identity's token as an `http.extraHeader` set
  through `GIT_CONFIG_*`, and a commit its author through `GIT_AUTHOR_*`
  and `GIT_COMMITTER_*`; never in an argument, a URL, a repository's
  configuration or a file (credentials.md, section 4).
- **The remote is a URL on the command line.** The engine's routing
  makes each repository's remote from its forge's `git` base, its owner
  and its name (channel.md, section 4); every invocation that reaches the
  forge names that URL itself, so a repository's own configuration never
  says where git connects.
- **Read by exit status first,** then by git's machine output: porcelain
  lines, NUL-separated paths and object ids. Standard error only
  classifies a failure, read in the C locale, and its tail is kept as the
  run's diagnostic (`PushDiagnostic`).
- **Never forced.** A push moves a branch only as a fast-forward; an
  expected head adds a compare-and-swap; a branch is created only where
  none exists.
- **Commits on the parents the domain names,** by git's plumbing
  (`write-tree`, `commit-tree`), so a merge always records both parents;
  HEAD stays where the checkout put it.
- **Make is file operations:** the old workspace renamed aside and a new
  one made; a sweep empties what was set aside, one file operation at a
  time.
- **A deadline or a cancel kills the invocation's process group,** and
  the operation ends only once its process has closed, so nothing of it
  touches the workspace afterwards.
- **Tested** by step tests against transcripts of real git, the git
  component's machine world, the fake git program checked against the
  same transcripts, and real git in the process tier (section 9).

## 2. Where it sits

```
temper-worker-domain            the worker's root: the checkout's Request::Io, Request::CancelIo
  ▲ git::Op and its deadline; Event::Done
the routing                     hands each operation over, with its identity's value from the identities' table
  ▲
temper-worker-protocol-git      the operations in flight: their invocations, pipes, deadlines; the sweep
  ▲ spawn, pipe reads and writes, signal, close; file operations
io                              processes (skein's `io.md`, section 6) and files (skein's `io.md`, section 5)
  ▲
the worker's root directory     workers[].root
├── workspaces/<token>/         a workspace, named by the checkout's token in decimal
│   └── <repository>/           a repository's working tree and its .git, side by side with the others
├── trash/<n>/                  what Make set aside, emptied by the sweep
└── home/                       git's HOME: empty, private
```

- **The root's requests reach it through the routing,** which keeps no
  state. For an operation that names an identity, the routing reads the
  identity's newest value from the worker's identities' table and hands
  over one token, its login and the author with the operation
  (credentials.md, section 4), with the program its repository's item
  named, if one did (section 3): Forgejo's one token, or on GitHub the
  entry whose account id is the repository's owner's, which its
  workspace item carries (channel.md, sections 4 and 6). So the
  component holds a value only while that operation is in flight. An
  operation whose identity has no value in the table fails `Broken`
  without running git: grants arrive with the assignment, so this is a
  fault of the worker's side. One whose GitHub value has no entry for
  the owner's account fails `Refused` without running git, named
  "temper's App is not installed on the account owning <name>"
  (channel.md, section 6).
- **Each operation ends exactly once:** `Done` with its own terminal, or
  `Failed`/`FailedWithOutput` with a fault, and `Cancelled` only if the
  cancel won the race (section 7). Its `owner` token is echoed.
- **The worker's root directory** is opened once at start by the
  worker's service and is the root every spawn and file operation
  resolves beneath (skein's `io.md`, section 5). At every start the
  component renames `workspaces` and `home` into `trash` if they exist,
  and makes them afresh, private (0o700), with `trash` if it is absent:
  the worker keeps nothing durable (`../domain/worker.md`, section 1),
  and git's `HOME` must be empty.
- **What it depends on:** `skein-lib`, `skein-io` and the checkout's
  vocabulary. It never decodes a channel record or reads configuration.

## 3. Invocations

Every invocation has this shape:

```
program    the repository's item's git, else workers[].git: an absolute path (configuration.md, section 4)
argv       git -c <setting> … <command> <options> [--] <operands>
directory  beneath the worker's root: the repository's directory, or the workspace's for Clone
environment  given whole (section 5)
pipes      stdin (written, then closed), stdout, stderr
```

**The settings,** on every invocation, in this order, each with `-c`:

| Setting | Why |
|---|---|
| `core.hooksPath=/dev/null` | no hook runs, whatever the repository holds |
| `core.fsmonitor=false` | no file system monitor program or daemon |
| `protocol.version=2` | fetch by any object id (gitprotocol-v2: wants are not limited to advertised refs) |
| `protocol.allow=never` | no transport by default: not `file`, `ssh` or `ext` |
| `protocol.https.allow=always` | the forge's transport |
| `protocol.http.allow=always` | the forge's transport where configuration allows plaintext, on loopback only (configuration.md, section 3) |
| `credential.helper=` | the empty value clears the list: no credential helper runs |
| `http.sslVerify=true` | the forge's certificate is always checked |
| `gc.auto=0` | no automatic garbage collection, which detaches and outlives the invocation |
| `maintenance.auto=false` | no automatic maintenance, likewise |
| `rerere.enabled=false` | no recorded resolution changes a merge |
| `merge.conflictStyle=merge` | conflicts carry the three markers section 4.6 reads |
| `submodule.recurse=false` | no submodule is touched |

- **Command-line settings win** over the repository's own configuration
  for the keys they name. The rest is the repository's own, which `clone`
  wrote; standalone trusts it until contained trees keep agents out of
  the git directory (section 10).
- **The working directory is the spawn's,** resolved by io beneath the
  worker's root, never `-C`: a repository's directory is
  `workspaces/<token>/<repository>`, with the workspace's token in
  decimal and the repository's name, one path component
  (`../domain/worker.md`, 4.1).
- **Operands.** Object ids are hex the component writes; branches appear
  only inside refspecs the component composes (`refs/heads/<branch>`);
  the remote URL and Clone's directory come after `--`. So no operand can
  be read as an option.
- **A renamed repository.** The remote is made from the owner and name
  the engine's root holds (channel.md, section 4). git follows Forgejo's
  redirect from a renamed repository's old path for its first request
  (`http.followRedirects`, whose default is `initial`), as
  `tools/git-transcripts` records (fact
  `git-follows-a-renamed-repository-redirect`); a remote that changes
  misses the checkout's cache, which clones afresh.
- **Object ids** cross the boundary as the checkout's 32-byte `Commit`.
  From git: exactly 40 or 64 lowercase hex digits, anything else is
  `Broken`; a SHA-1 id becomes its 20 bytes followed by twelve zero
  bytes, a SHA-256 id its 32 bytes. To git: the first 20 bytes as 40
  digits when the last twelve are zero, else all 32 as 64 digits. (A
  SHA-256 id whose last twelve bytes are zero, one in 2^96, is named by
  its first 40 digits, which git reads locally as an unambiguous
  abbreviation.)
- **The program,** for each operation, is the one its repository's
  item names (channel.md, section 4), the fake forge's, which the
  domain carries from the item to each `git::Op` on the repository and
  never reads; or, where the item names none, `workers[].git`. Every
  invocation of an operation runs the one program.
- **No program.** With `workers[].git` `null` and an item naming none,
  nothing is spawned: each invocation fails as git's fatal exit would,
  status 128, its stderr `fatal: no git on this machine for HOST`, HOST
  the remote's host with its port, and an operation without a remote
  (`CheckOut`, `Merge`, `Commit`) names no host, `fatal: no git on this
  machine`; so the operation ends `FailedWithOutput { Broken, diagnostic }`
  (section 6), and the forge's row warns (`../client/forge.md`, 5.4).
- **The version.** At the worker's start the component runs `git
  version` once with `workers[].git`, unless `null`, in the environment
  of section 5, and refuses the start, naming the program, when it fails
  or names a version below 2.41, the first with `fetch --porcelain`. A
  program an item names is checked so at its first use, before the
  operation's first invocation: one that fails it fails the operation
  `Broken`, naming the program, without running it. The component keeps
  the last such program it checked and its answer, one at a time, and
  checks a different one afresh. The component is tested against git
  2.53 (section 9).

## 4. Each operation

| Operation | Invocations | Ends in |
|---|---|---|
| `Make` | none: two file operations | `Succeeded` |
| `Clone` | 1 | `Succeeded` |
| `Fetch` | 1 | `Fetched { commit }` |
| `Create` | 1 | `Succeeded`, `Exists` |
| `CheckOut` | 2 | `Succeeded` |
| `Merge` | 1 or 2 | `Merged`, `Conflicted { files }` |
| `Commit` | 1 to 4, and a scan of files for a merge | `Committed { commit }`, `Unchanged`, `Conflicted { files }` |
| `Push` | 1, or 2 with an expected head | `Succeeded`, `Rejected` |
| `IsAncestor` | 1 | `IsAncestor { contains }` |

Each may also end failed, with the fault of section 6. Below, `<url>` is
the operation's `remote`, `<dir>` its repository's name, and ids are as
section 3 writes them. The settings of section 3 precede each command.

### 4.1 Make

`Make { workspace }`: the workspace's directory, empty.

1. `Rename` `workspaces/<token>` to `trash/<n>`, `<n>` the component's
   next serial number in decimal, counted from a number the component
   draws from its seed at each start, so a name an earlier start left in
   `trash` is not taken again. A workspace that does not exist is
   nothing to move.
2. `MakeDirectory` `workspaces/<token>`, mode 0o700.

`Succeeded` once both are done; a failed file operation is `Broken`. So
what the cache evicted, or a worker left behind before a restart, never
fails a prepare (`../domain/worker.md`, section 9), and Make takes two
file operations however large the old tree was.

**The sweep** empties `trash`, which io cannot do in one operation (io
never removes a tree: skein's `io.md`, section 5). While `trash` is not
empty and the worker is not shutting down, the component runs one file
operation at a time:

- it lists a page of `trash` (`Scan`, within `sweep_entries` and
  `sweep_bytes`) and takes the first entry;
- it tries to remove the entry as a file; one that is a directory it
  lists a page of, and each child it removes as a file, or, if the child
  is a directory, renames to `trash/<n>`;
- it removes a directory once its listing is empty.

Every directory it lists is at most two levels below the worker's root,
so the sweep holds one page, whatever the depth of the tree.
Nothing is followed through a link. What is left at shutdown is swept
after the next start. A sweep's failure is counted and the entry is left
for the next pass, which starts at the next `Make` or the next start.

### 4.2 Clone

`Clone { at, remote, identity }`, in the workspace's directory:

```
git clone --no-checkout --no-tags --origin origin -- <url> <dir>
```

- **Exit 0:** `Succeeded`, an empty repository included (git warns and
  succeeds).
- **Otherwise:** the fault of section 6; `Missing { Repository }` when
  the forge has no such repository.
- **Full history, no checkout.** No `--depth` and no `--filter`: a partial
  clone would fetch missing objects later, from `CheckOut` and `Merge`,
  which carry no identity. The working tree is made by `CheckOut`.

### 4.3 Fetch

`Fetch { at, remote, want, identity }`:

```
git fetch --porcelain --verbose --no-tags --no-write-fetch-head --no-recurse-submodules -- <url> +<source>:refs/temper/fetched
```

where `<source>` is `refs/heads/<branch>` for `Want::Branch`, the commit's
full id for `Want::Commit`, and `HEAD`, the forge's default branch, for
`Want::Default`.

- **Exit 0 and one porcelain line** `<flag> <old-id> <new-id>
  refs/temper/fetched`, its flag one of space, `+`, `*` or `=`:
  `Fetched { commit: <new-id> }`. `--verbose` makes git print the line
  when the reference was already there.
- **`refs/temper/fetched`** is the one reference the component writes in
  a repository: it keeps what was fetched reachable, and is moved freely
  (`+`) by the next fetch.
- **Missing:** `Branch` for a branch, or a default branch, the forge does
  not have; `Commit` for a commit it does not have (section 6). A full id
  is required: git looks an abbreviation up as a reference name.
- **A push that ended ambiguously** is verified by the domain with a
  fetch of its branch (`../domain/worker.md`, section 5): no other
  invocation is needed for it.

### 4.4 Create

`Create { at, remote, branch, commit, identity }`: the branch, only if
the forge has none.

```
git push --porcelain --force-with-lease=refs/heads/<branch>: -- <url> <commit>:refs/heads/<branch>
```

The lease with an empty expected value requires that the branch not
exist (git-push(1)); the forge's receive applies it as a compare-and-swap
on the old value. The status line of `refs/heads/<branch>` decides:

| Flag and summary | Ends in |
|---|---|
| `*` `[new branch]` | `Succeeded` |
| `=` `[up to date]`: it exists, at `commit` | `Exists` |
| `!` `[rejected] (stale info)`: it exists elsewhere | `Exists` |
| `!` `[remote rejected] (…)`, its reason ending in `declined` | the fault `Refused`, with its diagnostic |
| any other, or no status line | the fault of section 6 |

### 4.5 CheckOut

`CheckOut { at, commit }`: the working tree exactly `commit`'s tree.

```
git checkout --force --detach <commit>
git clean -ffdxq
```

- **The first** detaches HEAD at the commit, overwrites what is in the
  way, tracked or not, and clears a merge's state (`MERGE_HEAD` and the
  rest). **The second** removes every untracked and ignored file and
  directory, nested repositories included.
- **Exit 0 from both:** `Succeeded`; any other exit is `Broken`.

### 4.6 Merge

`Merge { at, theirs }`: merge fetched `theirs` into the checked-out
commit, without committing, leaving conflicts and the merge's state on
disk.

```
git merge --no-commit --no-ff -- <theirs>
git diff-files --name-only -z --diff-filter=U          only after exit 1
```

- **Exit 0:** `Merged`, a commit already contained in HEAD ("Already up
  to date") included: the next commit still records both parents
  (section 4.7).
- **Exit 1, and a list that is not empty:** `Conflicted { files }`, the
  unmerged paths, NUL-separated, relative to the repository's top, in
  git's order. More than the checkout's `conflicts`, or one longer than
  its `path_bytes`, is `Broken`.
- **Exit 1 with an empty list,** as for a commit git cannot merge, or any
  other exit: `Broken`.
- **`diff-files`,** the plumbing command, lists the index's unmerged
  entries whatever the repository's diff settings say.
- **The committer** git requires even with `--no-commit` is the
  placeholder of section 5: Merge names no identity, and makes no commit.

### 4.7 Commit

`Commit { at, parent, merging, title, body, identity }`: the working tree
exactly as it is, on the parents the domain names. The message, given on
stdin, is `title`, then, when `body` is not empty, a blank line and
`body`, then a newline. `.gitignore` is honoured: an ignored file is not
part of the tree.

**Without a second parent** (`merging` is `None`):

```
git add --all
git diff-index --cached --quiet <parent> --            exit 0: Unchanged, and nothing more runs
git write-tree
git commit-tree <tree> -p <parent> -F -
```

**With a second parent** (`merging` is `Some(m)`):

```
git diff-files --name-only -z --diff-filter=U          the originally conflicted paths still unmerged in the index
                                                       then the scan of those files, below
git add --all
git write-tree
git commit-tree <tree> -p <parent> -p <m> -F -
```

- **`Committed { commit }`:** `commit-tree`'s stdout, one id.
- **`diff-index` exit 1** means the tree changed; any exit but 0 and 1 is
  `Broken`.
- **The scan.** Before anything is staged, each listed path is read
  beneath the repository's directory, without following a link, in
  chunks of `read_bytes`. A path holds a marker when one of its lines,
  split at newlines with one trailing carriage return dropped, begins
  with seven `<` and a space, is exactly seven `=`, or begins with seven
  `>` and a space. Paths with markers end the operation `Conflicted {
  files }`, in the list's order, and nothing is staged, so the index
  still knows them at the next attempt. A path that is gone, a directory
  or a link holds no marker: deleting a conflicted file resolves it, as
  `git add --all` then records.
- **Why plumbing.** The domain always records both parents of a merge,
  also when git found `theirs` already merged and wrote no `MERGE_HEAD`,
  and git refuses to write that pseudo-reference through `update-ref`.
  `commit-tree` records exactly the parents asked, signs nothing and runs
  no hook.
- **HEAD is not moved,** nor the merge's state cleared, by a commit: HEAD
  and the merge's state are `CheckOut`'s. Each commit compares and
  records against the parent the domain names, never against HEAD, so
  the next commit and push are right whatever HEAD says.
- **The author and committer** are the identity's author (section 5); the
  date is the clock's, in UTC.

### 4.8 Push

`Push { at, remote, commit, branch, expected, identity }`: `commit` to
the branch as a fast-forward, never forced.

Without an expected head:

```
git push --porcelain -- <url> <commit>:refs/heads/<branch>
```

With `expected: Some(e)`:

```
git merge-base --is-ancestor <e> <commit>              exit 1: Rejected, and nothing is pushed
git push --porcelain --force-with-lease=refs/heads/<branch>:<e> -- <url> <commit>:refs/heads/<branch>
```

- **Why both.** A lease alone overrides the fast-forward rule: with the
  remote at `e`, git would replace `e` with a commit that does not
  descend from it. The ancestry check makes the update a fast-forward
  first; the lease then makes the forge apply it only if the branch is
  still at `e`. `--force` and a `+` refspec are never used.
- **`merge-base` exits** 0 (go on), 1 (`Rejected`), anything else
  `Broken`.

The push's status line decides:

| Flag and summary | Ends in |
|---|---|
| space (a fast-forward), `*` `[new branch]`, `=` `[up to date]` | `Succeeded`: the branch is at `commit` |
| `!` `[rejected] (non-fast-forward)`, `(fetch first)`, `(stale info)` | `Rejected` |
| `!` `[remote rejected] (…)`, its reason ending in `declined` | the fault `Refused`, with its diagnostic: a protected branch, a hook |
| any other, `+` included, or no status line | the fault of section 6 |

Forgejo refuses a push to a protected branch with that `[remote
rejected]` line, its reason ending in `declined`, as
`tools/git-transcripts` records (fact
`protected-push-is-remote-rejected-declined`).

A push that fails in a way that may have landed (`Unreachable`, `Broken`,
`TimedOut`, `Cancelled`) is verified by the domain (section 4.3).

### 4.9 IsAncestor

`IsAncestor { at, ancestor, commit }` asks whether fetched `ancestor` is
contained in fetched `commit`, including equality. It is local, changes no
file or ref, and needs no forge credential:

```
git merge-base --is-ancestor <ancestor> <commit>
```

Exit 0 ends `IsAncestor { contains: true }`, exit 1 ends it with false,
and other exits fail as `Broken`. The same command inside Push retains
section 4.8's separate contract: exit 1 rejects that push. The checkout
uses this operation to compare an existing producer head with the landing
tip frozen by its prepare; the comparison alone is never a push receipt.

## 5. Credentials and environment

The environment is given whole: what is below, and nothing else. No
variable of the worker's own process reaches git.

| Variable | Value | For |
|---|---|---|
| `HOME` | the absolute path of `home` under the worker's root, an empty private directory | every invocation: no `.gitconfig`, `.netrc` or other file of a user's reaches git |
| `LC_ALL` | `C` | every invocation: the messages section 6 reads |
| `TZ` | `UTC` | every invocation: a commit's dates carry no host's zone |
| `GIT_CONFIG_NOSYSTEM` | `1` | every invocation: no system configuration |
| `GIT_CONFIG_GLOBAL` | `/dev/null` | every invocation: no user configuration |
| `GIT_TERMINAL_PROMPT` | `0` | every invocation: git never asks |
| `GIT_ASKPASS` | empty | every invocation: no askpass program, whatever a repository's `core.askPass` says |
| `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` | the identity's author; or `temper` and `temper@temper.invalid` for an operation that names no identity | every invocation: the committer of a commit, or of a reference's log entry |
| `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL` | the identity's author | `Commit` |
| `GIT_CONFIG_COUNT` | `1` | `Clone`, `Fetch`, `Create`, `Push` |
| `GIT_CONFIG_KEY_0` | `http.extraHeader` | likewise |
| `GIT_CONFIG_VALUE_0` | `Authorization: Basic ` and the base64 of `<login>:<token>` | likewise |

- **No `PATH`.** git is run by its absolute path and starts its own
  helpers (`git-remote-https`, `index-pack`) from its build's exec path;
  nothing in these invocations runs a program by name, so none can be
  found in a working tree.
- **The identity's value** is the one channel.md, section 6 encodes, as
  the routing hands it over (section 2): one token, Forgejo's or the
  GitHub entry it picked, with the login and the author. The header's
  value is base64 and a fixed prefix, so no byte of the token can break
  it.
- **Never elsewhere:** no credential in an argument, a URL, a
  repository's configuration or a file; no `GIT_TRACE*` or
  `GIT_CURL_VERBOSE`, so git never prints the header, and the diagnostic
  tail a run sees (section 6) holds none.
- **What standalone accepts:** an agent run as the same user could read a
  git process's environment while it runs (credentials.md, section 8).
  Contained trees close it (section 10).

## 6. Output and failures

**What is kept** of each invocation:

- **stdout,** up to `stdout_bytes`, which is enough for every output
  section 4 reads. Past it, the bytes are read and dropped, counted, so
  git is never blocked on its pipe, and an operation whose output passed
  the cap ends `Broken`.
- **stderr,** its last `stderr_bytes`, with the count of bytes dropped
  before them. It is read only to classify a failure, and its last
  `PushDiagnostic::CAPACITY` bytes, with that count, are the diagnostic
  (`PushDiagnostic::new`).

**How an operation ends,** in this order:

1. **The deadline passed or the cancel won** (section 7): `TimedOut` or
   `Cancelled`.
2. **The spawn failed,** or git ended by a signal: `Broken`.
3. **The exit status and stdout give the outcome** of section 4.
4. **Otherwise, stderr's tail classifies it.** The component reads the
   tail's lines that begin `fatal: ` or `error: `, from the last to the
   first; the first that matches a row below decides, and none matching
   is `Broken`.

| A line of stderr (C locale) | Fault |
|---|---|
| `fatal: repository '…' not found` | `Missing { Repository }` |
| `fatal: couldn't find remote ref …` (Fetch) | `Missing { Branch }` |
| `fatal: remote error: upload-pack: not our ref …` (Fetch) | `Missing { Commit }` |
| `fatal: could not read Username for …`, `fatal: Authentication failed for …` | `Refused`: the forge did not take the token |
| `fatal: unable to access '…': The requested URL returned error: 401` or `403` | `Refused` |
| `fatal: unable to access '…': The requested URL returned error: 404` | `Missing { Repository }` |
| `fatal: unable to access '…': ` and anything else: another status, a refused connection, a name not resolved, a TLS failure, a timeout | `Unreachable` |
| `fatal: the remote end hung up unexpectedly`, `fatal: early EOF`, `error: RPC failed…` | `Unreachable` |

- **Only operations that reach the forge** (`Clone`, `Fetch`, `Create`,
  `Push`) are classified by the table. A local operation that fails is
  `Broken`.
- **The faults** are the checkout's (`git::Fault`): `Missing` where the
  forge lacks what was asked, `Refused` where it refused the identity or
  the update, `Unreachable` where it could not be reached or failed,
  `Broken` for the worker's side and anything git said that the table
  does not name, `TimedOut` and `Cancelled` as section 7 says.
- **With output or without.** A failure after git ran ends
  `FailedWithOutput { fault, diagnostic }`, the diagnostic from the
  invocation that failed; one before any invocation ran (a missing
  identity's value, a failed spawn of the first, a named program that
  fails its version, Make's file operations) ends `Failed { fault }`,
  but for no program, whose diagnostic names the host (section 3).
- **The messages are git 2.53's** in the C locale, as the transcripts of
  section 9 record them; a version that words them otherwise classifies
  them `Broken`, which the domain retries as transient and the process
  tier shows.

## 7. Processes, deadlines and cancel

- **One invocation at a time per operation,** each spawned in a process
  group of its own (skein's `io.md`, section 6); contained process trees
  are for hosts apart (jig's `protocol/README.md`, section 10; section
  10). git's own children (its remote helper, `index-pack`,
  `pack-objects`) stay in that group. The settings of section 3 leave
  git nothing that would leave the group: no detached maintenance, no
  credential daemon, no file system monitor.
- **An invocation ends** once git has exited, stdout and stderr have
  reached their ends, and io has closed the child, which kills whatever
  is left of its group (skein's `io.md`, section 6). The next invocation
  starts only then.
- **One deadline per operation:** the domain's, from the checkout's
  `remote_timeout` or `local_timeout`. It covers the whole sequence,
  Commit's scan included. When it passes, the component signals `Kill` to
  the current invocation's group and closes it; no further invocation
  starts; the operation ends `TimedOut` once the child has closed.
- **Cancel** does the same and ends `Cancelled`. An operation whose last
  invocation had already exited, its output read, when the cancel came
  ends with its own terminal: the outcome that won the race (the
  checkout's boundary, `Request::Cancel`). A cancel between two
  invocations starts no further one.
- **Kill at once.** A killed git may leave a lock file or half an index;
  the checkout does not trust a workspace whose operation timed out, was
  cancelled or broke, and makes it again from an empty directory
  (`../domain/worker.md`, section 5).
- **stdin** is written whole (the message, for `commit-tree`; nothing
  otherwise) and closed; stdout and stderr are read as they come, both at
  once, so git never waits on a full pipe.
- **At the worker's end** the service closes every child, killing their
  groups: in temper's one process, the service ends the worker at once
  after the engine has drained (README.md, section 8; jig's
  `protocol/service.md`, section 10). Operations in flight end as io
  closes them.

## 8. Limits and the worst case

The git component's `Limits`, set by temper's profile `small` and
overridable under `git_component.` (configuration.md, section 7):

| Limit | What it bounds | Rule |
|---|---|---|
| `operations` | operations in flight at once | at least the checkout's `workspaces`, since each hold runs one at a time |
| `stdout_bytes` | stdout kept per invocation | at least the largest output section 4 reads under the checkout's limits: the unmerged list (`conflicts` paths of `path_bytes`, each with its NUL), a push's status lines |
| `stderr_bytes` | the stderr tail kept per invocation | at least `PushDiagnostic::CAPACITY` |
| `read_bytes` | one read of Commit's scan | above 0 |
| `sweep_entries`, `sweep_bytes` | one page of the sweep's listing: entries, and bytes of names | above 0; `sweep_bytes` at least 255, the longest name io lists (skein's `io.md`, section 5) |

The start is refused, naming the limit, when a rule does not hold. The
component has no deadline of its own: an operation's is the domain's,
and io's close timeout bounds the end of a killed invocation.

- **Memory:** for each operation in flight, its program (an absolute
  path of at most 4,096 bytes, channel.md, section 8), its argument
  vector (the settings, a URL and a refspec within the checkout's
  `name_bytes`), its
  environment (an identity's value within the channel's bound,
  channel.md, section 8), its message (the checkout's `message_bytes`),
  `stdout_bytes`, `stderr_bytes`, `read_bytes` and the paths it parsed
  (`conflicts` × `path_bytes`); plus one page of the sweep, and the
  table of operations. `worst_case` counts them, the containers'
  bookkeeping included.
- **Processes:** at most `operations` git process groups at once.
- **Time:** an operation ends within its deadline and io's close timeout.
- **Most outputs per entry point:** a constant the component exports; an
  event starts at most one invocation, or one file operation.
- **Disk:** the workspaces are the cache's (`../domain/worker.md`, section
  5); `trash` holds no more than the workspaces it took, until the sweep
  has emptied it.

## 9. Testing

The tiers, their budgets and the fakes are `../domain/testing.md`'s. What
checks this document's contract:

- **Step tests** in `temper-worker-protocol-git`:
  - **the builders:** for each operation and case (each `Want`, a push
    with and without an expected head, a commit with and without a second
    parent), the exact argument vector and environment of sections 3 to
    5; that no value of an identity appears in an argument; the message's
    bytes;
  - **the parsers:** fetch's porcelain line, push's status lines, NUL
    lists, ids of 40 and 64 digits and malformed ones, the conversions
    both ways, and the classifier of section 6, each against transcripts
    and against their mutations;
  - **the scan's rule** on lines cut across reads, carriage returns, and
    markers of six and eight characters; **the sweep's step** over a
    listing;
  - **the program:** an item's spawned in place of `workers[].git`, its
    `git version` at first use, again for a different one and never for
    the same, a version below 2.41 failing the operation unrun; and no
    program, each invocation failing 128, naming the host, with nothing
    spawned.
- **Transcripts** of real git 2.53, captured once, outside the suites, by
  `tools/git-transcripts`: every invocation of section 4, and every
  failure of section 6 (a missing repository, branch and commit, a wrong
  token, no permission, a protected branch, a refused connection),
  against skein's throwaway Forgejo (`fake-forge.md`, section 6;
  `../domain/testing.md`, 2.6). Each keeps the argument vector,
  exit status, stdout and stderr, masked: object ids, ports, paths and
  times. No credential is ever written. The facts this document relies
  on, named in sections 3 and 4.8, are in `tools/git-transcripts/facts.txt`
  with git's and Forgejo's versions. They are captured again when an
  invocation changes, or to move to another git.
- **The git component's machine world,** `tests/worker/protocol-git`:
  the component between scripted operations above and scripted io below:
  a spawn refused; a process's exit before and after its pipes end;
  output cut at random, past each cap, and empty; an exit by a signal;
  the deadline at every invocation of every sequence; a cancel at every
  invocation and racing an exit; a missing identity's value; file
  operations failing; the sweep over deep and wide trees in pages of
  two. Its referee: each operation ends exactly once, with the outcome
  its scripted outputs imply; no invocation starts after a deadline or a
  cancel; every child is closed before its operation's terminal; no
  credential in an argument; memory within `worst_case`. Its scripted
  outputs are the transcripts.
- **The fake git program,** its core `crates/temper-fake-git` and its
  hosted half `testing/temper-fake-git-hosted` (`../domain/testing.md`,
  4.3), answers this document's invocations as real
  git does, and is checked against the same transcripts: each
  transcript's arguments and environment, run on the fake over the
  equivalent fixture, give the same exit status, the same stdout once
  masked, and stderr lines that classify alike.
- **The process tier** runs the machine's git, 2.41 or later, against
  its owned Forgejo: clone, fetch, create, check out, commit, push, in
  its story (`../domain/testing.md`, 2.6).

## 10. Open questions

- **Containment.** In standalone, git and agents run as one user in
  process groups: an agent could read a running git's environment, or
  write a repository's configuration (a URL rewrite, a proxy) before the
  next invocation. Default: accepted on one person's machine
  (credentials.md, section 8); the command-line settings of section 3
  and the URL on the command line cover what they name. Contained trees
  (skein's `draft/process.md`), with the git directory unwritable by an
  agent's tree, close both.
- **HEAD after a commit.** Default: not moved (section 4.7), keeping each
  commit to four invocations; an agent that reads git sees the work
  since its start as uncommitted. Moving it costs a fifth invocation
  (`git reset -q <commit>`).
- **A forge whose certificate needs its own trust for git.** Default: the
  machine's roots; configuration gives a trust file for the API only
  (configuration.md, section 3). Between hosts, the trust would come with
  the workspace's items.
- **Git LFS.** Default: not supported; no filter is configured, so a
  repository's large files stay pointer files in its working tree.
- **Another git's messages.** Default: classified against 2.53's, from
  2.41 on; a version that words a failure otherwise reads `Broken` until
  its transcripts are captured.
