# temper ctl project

Kind: reference

Projects: read their periods and pools, set their spend per period.

## project show

```
project show PROJECT
  PROJECT (project): a project's name or number; required. The project
Read a project, its roles and spends, its open period and each party's pool
Prints: The project, its homes and spend per period, each role, its open period and each pool
--json: the record project.
Key: none.
Amounts are in the deployment's unit. Spent includes what ended tasks spent. A missing next start or open period says so.
```

## project fund

```
project fund PROJECT AMOUNT [--role ROLE] --key KEY
  PROJECT (project): a project's name or number; required. The project
  AMOUNT (amount): a number from 0 to 18446744073709551615; required. The spend per period, in the deployment's unit
  --role ROLE: a number from 0 to 4294967295; optional. Set this role's spend per period
Set a project's spend per period, or a role's, resizing the open period at once
Prints: The spend edit as answered; then now: the open period's budget and what is left, with --role its pools too
--json: the record project_fund.
Key: --key KEY, required.
An owner's chats are funded from their pool, and pools reserve from the project's period. Raise both with two calls: project fund PROJECT AMOUNT first, then project fund PROJECT AMOUNT --role ROLE. A role's raise past the period's room is refused, exit 2, naming what the period has left. A cut keeps each budget at what it has used. A key replay prints the original edit, then today's balances marked now:. JSON has answer (project, optional role, period_spend) and now (the project read).
```

## project hold

```
project hold PROJECT --stop REASON [--key KEY]
  PROJECT (project): a project's name or number; required. The project's number
  --stop: a switch. Stop its runs at once
  REASON (reason): text, at most 1048576 bytes, or - to read stdin; required. Why, in words the hold keeps
Place a hold of yours over a whole project
Prints: `done: hold N placed, rev REV`
--json: the record hold.
Key: --key KEY, optional.
Every task of the project is held, those made later
included, but the chats you or another person started, which
go on; each live run in scope is cancelled at once, and a run
asks for no delegate while it stands. task release releases
one task under it, which then runs; hold lift lifts it. A
project's owner or a maintainer places one.
```
