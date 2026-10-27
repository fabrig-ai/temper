# Proposal

Kind: concept

A request for an act beyond its proposer's authority, which waits for a party or task that may decide it: a batch of tasks, an effect, a widening, an amendment, or a change of settings.

## What it is

A proposal asks someone with more authority to authorize one act:

- a batch of delegate tasks;
- one effect, such as a write to the forge;
- a widening of a task's authority;
- an amendment that needs more authority than its task has;
- a change of settings, which only the deployment's owners decide
  (`temper help settings`).

A proposal is named by its proposer and its number: `t41/3` is task 41's
third, `p3/17` party 3's seventeenth. Every result that offers it prints
its name.

A proposal of settings carries its items, the revision it was made
against, and its reason. Its review shows, for each item, the value now
and the value it would be, each whole.

## How it changes

A proposal waits with a holder: a party or a task whose role decides its
kind and whose authority covers the act. It is then:

- accepted, which makes the act, funded by the accepter;
- rejected, with a reason its proposer hears;
- passed up, to the holder above;
- withdrawn by its proposer.

A pending proposal of settings is withdrawn as stale when another
change, or a start, makes a new revision.

## What temper guarantees

- A proposal grants nothing until it is accepted.
- A proposal is judged again when it is accepted, against the authority
  and policy that stand then.
- A holder decides only the kinds of proposal their role decides.
- A proposal of settings is accepted only by an owner of the deployment,
  with a running choice: running work stays on its revision, as when no
  choice is given, or moves.
- Its proposer reads the outcome with `proposal show`.
- A decision asked again under its key gets its first answer again. A
  decision of a proposal decided already, by anyone, answers `decided
  first:` with who decided it, how and when, and changes nothing.

## Acting on it

- `temper ctl`: `inbox` lists what waits on you, proposals among it;
  `proposal show` reads one, with its review for settings; `proposal
  decide` accepts, rejects or passes it up; `settings propose`
  proposes a change of settings.
- A run proposes with the host tool `propose` (a batch, an effect, an
  amendment, a widening or a release), decides with `decide`, and
  withdraws its own with `withdraw`.

## Limits

- The person's client shows a proposal of settings in its inbox without
  its values, and can reject it but not accept it: accept it with
  `temper ctl`.
- A party's proposal withdrawn as stale names the revision it was made
  against, not the newest: read `settings show`, and propose again.
- temper does not let ctl withdraw a proposal.
- A proposal of a batch names amounts: a budget of as much as fits is
  not proposable.
