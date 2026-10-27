# 0b: the ground with a held ChatGPT account and one chat turn

0b is rung 0b of `docs/design/draft/qa-live.md` (3.1): 0a's box, owned
Forgejo, seed and `ground` check (`../live-0a/README.md`), plus the lab's
held ChatGPT record, `chatgpt` (4.3), moved into the box, one chat turn on
it, and the record moved out again. It has no driver. Its only spend is the
chat's turn on the account, which the meter reads.

The rendered files are 0a's, except for the model prices and window.
`temper.json` prices gpt-6-luna at 1,000 tokens a unit with its window at
131,072 and seeds no project. After sign-in, the `settings` action applies
`settings.json` against revision 1: both charters' spend and the rules'
`maximum_run_spend` become 280; policy 1's and the rules' spends and
`period_spend` become 600. The following `forge-setup` creates `hello`
from that policy, including its 600-unit ceiling. The charter's allowance
buys two of its model's largest turns:
2 × ceil((window + output) / 1000) units, which is 280 at this window. At
65,536 the agent's tool payload does not fit its messages. `agent.json`'s
model carries the same unit and window.

The seed inserts `settings` after 0a's sign-in and before its forge setup
and person actions, then adds:

- `import` (`seed/import`): `ctl account import {held.chatgpt} --key lab-import`,
  which moves the record in and removes the file. The account's number,
  from `moved in: account N`, is kept in the run's `ground/account`.
- `usable`: `ctl account list --until usable --timeout 200`.
- `chat` (`seed/chat`): in the project `hello`, one chat with the key
  `ground-chat`, asking for one short sentence and no delegation. It
  waits until the chat has replied, then keeps `ctl chat show N --json` in
  `ground/chat.json`.

The release, `export` (`release/export`), runs
`ctl account export ACCOUNT {held.chatgpt}` with the kept number, which writes the
record back for the runner to return (the lab's `lab.md`, 4.8). If no account
was imported, it runs nothing and fails, and the untouched file is given
back.

The meter (`meter/spend`) adds up `spent` over every project's top-level
tasks, live and ended, in units of 1,000 tokens. With no driver, it reads
once after the seed. The class's `subject_spend` fails a run that read more.
temper's own run cap, 280, is what stops a turn while it runs.

The checks are 0a's `ground` and `chat`. `chat` fails `no-reply` when no
assistant message in the conversation has a text block with words in it
(`task show --until replied` alone is also met by a failed run). It is undecided
when the conversation cannot be read.

The class is `temper-live-0b-calibration` (0a's calibration bounds,
`subject_spend` 60, `meter_seconds` 10), then `temper-live-0b`. Paid runs
spend the held account, so nobody runs this experiment without the owner's
go. It was rehearsed without a model on the real stack: the fake LLM in
place of the endpoint and `add-key` in place of `import`, once with a reply
(pass) and once with a blank one (`check/chat/no-reply`). It was also run
on a junk held record, which `import` refused and the release gave back.
