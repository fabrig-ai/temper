# temper ctl chat

Kind: reference

Chats: start one, read its conversation, close it.

## chat show

```
chat show TASK [--before CURSOR]
  TASK (task): a number; required. The chat's task number
  --before CURSOR: a cursor, as a read prints it; optional. The cursor a page printed, to read further back
Read a chat's conversation from its end
Prints: The conversation, oldest line first: each turn's words, each run's start and end, then `read on with: chat show TASK --before CURSOR` or `no more`
--json: the record chat.
Key: none.
Each page is older than the one before it. A run still writing
adds its words so far.
```

## chat start

```
chat start --project PROJECT TEXT [--model MODEL] [--effort EFFORT] [--template TEMPLATE] --key KEY
  --project PROJECT: a project's name or number; required. The project
  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. The opening words
  --model MODEL: text, at most 1048576 bytes, or - to read stdin; optional. A model the chat may use, as models lists it
  --effort EFFORT: text, at most 1048576 bytes, or - to read stdin; optional. An effort that model offers
  --template TEMPLATE: a number from 1 to 4294967295; optional. An offered conversation template; the project's chat template when absent
Start a chat in a project
Prints: `done: task N, chat`, or `proposed: PROPOSAL, to RECIPIENT` when another party must agree
--json: the record chat_create.
Key: --key KEY, required.
The chat's template chooses its model unless `--model` names one;
`--effort` needs `--model`, and a model that declares efforts needs
`--effort`. A chat past what your party may start, but within what
it may propose, is proposed (exit 0). `chat not offered` and `chat
exceeds offered bounds` (exit 1): your party may not start it.
Then task show TASK --until replied.
```

## chat close

```
chat close TASK [--move TASK]... [--reason TEXT] --key KEY
  TASK (task): a number; required. The chat's task number
  --move TASK: a number; repeated, at most 8. A live delegate to move to your party
  --reason TEXT: text, at most 1048576 bytes, or - to read stdin; optional. Why; empty when absent
Close a chat: move the delegates you name to your party, cancel the rest
Prints: `done: task N, close`
--json: the record close.
Key: --key KEY, required.
Every live delegate that `--move` does not name is cancelled with
the chat. A `--move` naming no live delegate fails, `move is not a
live delegate` (exit 1).
```
