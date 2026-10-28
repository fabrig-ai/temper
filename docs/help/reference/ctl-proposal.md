# temper ctl proposal

Kind: reference

Proposals: read one, decide it.

## proposal decide

```
proposal decide PROPOSAL accept|reject|pass-up [REASON] [--running stays|moves] [--key KEY]
  PROPOSAL (proposal): a proposal, tN/K or pN/K; required. The proposal, as the inbox prints it
  accept|reject|pass-up (outcome): one of accept, reject or pass-up; required. The decision
  REASON (reason): text, up to the deployment's people.words, 4,096 bytes on temper init's box, or - to read stdin; optional. Why; sent with reject alone
  --running stays|moves: one of stays or moves; optional. For a settings proposal: running work stays on its revision, or moves to the new one; stays when absent
Decide a proposal
Prints: `done: decide`, or `decided first:`, by whom, how and when, if it was decided already
--json: the record decide.
Key: --key KEY, optional.
A decision asked again under its key prints its first answer.
A decision of a proposal decided already, by your party or
another, answers `decided first:` with exit 0 and changes nothing.
```

## proposal show

```
proposal show PROPOSAL
  PROPOSAL (proposal): a proposal, tN/K or pN/K; required. The proposal, tN/K or pN/K, as a read prints it
Read a proposal; for settings, what each item would change
Prints: `proposal P/K` and its action, its reason, and, for settings, each item's value now and as it would be, whole
--json: the record proposal.
Key: none.
A decided proposal prints how it was decided. Decide a live one
with proposal decide.
```
