# Accounts and models

Kind: concept

An account is how a deployment pays an LLM provider for its runs, through its sources: a sign-in, a key or a program's login; models are what each provider offers, as the configuration lists them.

## What it is

- **A provider** is an LLM provider in the configuration, by its
  number: `temper init` writes provider 1, OpenAI, and provider 3, the
  playground's scripted model. `me` lists the providers your party may
  use and how each signs in.
- **An account** is your party's, or the deployment's, held for its
  owners; it has its number, its provider and its order among the
  accounts.
- **Its sources,** each numbered, in an order of their own:
  - `sign_in`: a sign-in to the provider, made in a browser;
  - `key`: an API key, read from a private file;
  - `program`: a program's own login, such as Claude Code's, on each
    host that runs it.
- **A source's state,** as `account list` prints it: `moving`,
  `asking`, `granted`, `later` (a limit reached, until a time),
  `sign_in_again`, `misconfigured`, `unloaded` (with where and why),
  `cancelled` or `closing`; and whether it is `usable`, `expiring` or
  needs `attention`.
- **A model** is one of a provider's, from the configuration: its
  window, its output, its efforts and its prices. `model list` prints
  each project's, by provider. A charter names the models its runs may
  use (temper help charter); a run's spend is its usage at its model's
  prices (temper help budget).

## How it changes

- **Connected:** a provider's sign-in starts from ctl and finishes in
  a browser on the engine's machine, where the provider sends it back;
  a key is added from a file; a program's login is declared, made on
  each host with `temper program-login`, then verified.
- **Limited:** a source that reaches a provider's limit waits,
  `later`, until its time, or until retried.
- **Signed out or forgotten:** a source is signed out; an account, or
  one source, is forgotten: a key erased, a sign-in or a login signed
  out.
- **Moved:** a signed-in source moves to another deployment. Export
  writes its record to a new private file, and the source leaves this
  deployment once that file is written; import on the other one takes
  it in and removes the file. A key or a program's login does not move:
  add it again there.

## What temper guarantees

- **No secret is printed:** `account list` shows accounts and sources,
  never their values.
- **One account, one deployment:** a moved source leaves only once its
  file is written and synced; an export that fails before keeps it
  here, and the engine cancels the move at its deadline.
- **Which account pays:** a task that names an account uses it, and
  waits while it is not usable. Otherwise temper takes a usable account
  by who pays and by the accounts' order; within an account, its
  sources in their order.
- **A key stays where you put it:** ctl reads a key file and leaves it;
  only an import removes its file.

## Acting on it

- **ctl:** `account list` reads the accounts and waits for one usable
  or settled; `account connect`, `account add-key` and `account
  declare` add a source, and `account verify` checks a program's
  login; `account replace-key`, `account rank`, `account retry`,
  `account disconnect` and `account forget` change one; `account
  export` and `account import` move one; `model list` reads the
  models. Each verb's arguments are its help: `temper ctl help
  account`.
- **The binary:** `temper program-login` signs a program in on this
  host (temper help commands).
- **temper's runs** name a model, and an account where they choose
  one, in the tasks they delegate.

## Limits

- ctl opens no browser: a provider's sign-in calls back to the engine's
  machine, and a browser elsewhere reaches it only through a tunnel
  (temper help box).
- temper runs no program for you: a program's login is made on each
  host that runs it.
- `account verify` checks a program source alone; a sign-in or key
  source fails (exit 1).
- The providers and models are the configuration's: a new one needs a
  change of it and a restart (temper help deployment).
