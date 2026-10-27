# temper ctl account

Kind: reference

LLM accounts: read them, connect one, manage its keys and sources, move one in or out.

## account list

```
account list [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: usable, settled, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read the accounts and their sources, never their values
Prints: `current: true`, then each account, its sources, their state and readiness, and `digest: accounts`
--json: the record accounts.
Key: none.
The deployment's own accounts show beside your party's when your
party owns the deployment.
```

## account connect

```
account connect PROVIDER --key KEY
  PROVIDER (provider): a number; required. The provider's number, as me prints it
Start a sign-in to an LLM provider
Prints: `Open this address in a browser on the engine's machine:`, the address, then `account list --until settled`
--json: the record connect.
Key: --key KEY, required.
It returns once the sign-in starts. Open the address in a browser
on the engine's machine, where the provider sends the browser back,
then account list --until settled.
```

## account disconnect

```
account disconnect ACCOUNT [--source sign_in|key|program] [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  --source sign_in|key|program: one of sign_in, key or program; optional. The source by its kind, when the account has several
Sign an account's source out
Prints: `done: disconnect`; for a program, where it signed out; for a provider that keeps its grant, where to revoke it
--json: the record disconnect.
Key: --key KEY, optional.
Without `--source`, the account has one source. Where the
provider's grant cannot be revoked from here, the answer says where
to revoke it, in the provider's settings.
```

## account add-key

```
account add-key PROVIDER FILE [--account ACCOUNT] [--deployment] --key KEY
  PROVIDER (provider): a number; required. The provider's number, as me prints it
  FILE (file): a file; required. The private file holding the key
  --account ACCOUNT: a number; optional. Join this account instead of making one
  --deployment: a switch. Hold it for the deployment, not your party; an owner's
Add an API key from a private file
Prints: `done: add-key`
--json: the record add_key.
Key: --key KEY, required.
The file holds 1 to 4096 bytes after one final newline is trimmed.
ctl leaves it in place: remove it once the account is usable. Then
account list --until usable.
```

## account replace-key

```
account replace-key ACCOUNT FILE [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  FILE (file): a file; required. The private file holding the new key
Replace an account's key from a private file
Prints: `done: replace-key`
--json: the record replace_key.
Key: --key KEY, optional.
The account has one key source. The file's bounds are account
add-key's.
```

## account forget

```
account forget ACCOUNT [--source sign_in|key|program] [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  --source sign_in|key|program: one of sign_in, key or program; optional. The source by its kind, when the account has several
Remove an account, or one of its sources
Prints: `done: forget`
--json: the record forget.
Key: --key KEY, optional.
Without `--source`, the whole account goes. With it, a sign-in or
program source is signed out, and a key source is erased.
```

## account rank

```
account rank ACCOUNT POSITION [--source sign_in|key|program] [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  POSITION (position): a number; required. The place, from 1
  --source sign_in|key|program: one of sign_in, key or program; optional. The source by its kind, when the account has several
Move an account, or one of its sources, to a place in its order
Prints: `done: rank`
--json: the record rank.
Key: --key KEY, optional.
Without `--source`, the account moves among the accounts; with it,
the source moves among its account's sources.
```

## account retry

```
account retry ACCOUNT [--source sign_in|key|program] [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  --source sign_in|key|program: one of sign_in, key or program; optional. The source by its kind, when the account has several
Resume a source that reached a limit
Prints: `done: retry`
--json: the record retry.
Key: --key KEY, optional.
```

## account declare

```
account declare PROVIDER ADAPTER NAME [--account ACCOUNT] --key KEY
  PROVIDER (provider): a number; required. The provider's number, as me prints it
  ADAPTER (adapter): text, at most 1048576 bytes; required. A program the provider offers, as me prints it
  NAME (name): text, at most 1048576 bytes; required. The login's name: 1 to 32 lower-case letters, digits or hyphens
  --account ACCOUNT: a number; optional. Join this account instead of making one
Declare a program's login for a provider
Prints: `Program source S: NAME`, then the command to run on each host
--json: the record declare.
Key: --key KEY, required.
No program runs for you: run the printed command on each host that
runs the program, then account verify.
```

## account verify

```
account verify ACCOUNT [--source sign_in|key|program] [--key KEY]
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  --source sign_in|key|program: one of sign_in, key or program; optional. The source by its kind, when the account has several
Ask the hosts to check a program's login
Prints: `asked`, then `account list --until settled`
--json: the record verify.
Key: --key KEY, optional.
Only a program source has a login to check; a sign-in or key
source fails (exit 1).
```

## account export

```
account export ACCOUNT FILE
  ACCOUNT (account): a number; required. The account's number, as account list prints it
  FILE (file): a file; required. The new file; nothing may be there
Move a signed-in account out to a new private file
Prints: `moved out: account N to FILE`
--json: the record export.
Key: none.
It takes no --key. It creates FILE, mode 0600, before sending
anything, writes and syncs the account's record, and only then
tells the engine; the account then leaves this deployment. A
failure before that removes FILE, and the engine keeps the account
and cancels the move at its deadline: export again after it.
On the deployment it moves to, account import FILE, as it is.
```

## account import

```
account import FILE --key KEY
  FILE (file): a file; required. The private file account export wrote
Move an account in from the file account export wrote, then remove the file
Prints: `moved in: account N`
--json: the record import.
Key: --key KEY, required.
The file holds 1 to 131072 bytes, read whole. It is removed once
the engine holds the account, and kept otherwise: the same call
with the same key and file gets the first answer. A record of a
newer form than this binary reads asks for an upgrade.
```
