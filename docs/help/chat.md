# Chats

Kind: concept

A chat is a conversation between a party and an agent in a project: an untracked task of the label chat, which replies, parks between messages, and asks for changes that other tasks make.

## What it is

- **A chat is a task** labelled `chat`, made from its project's chat
  template, template 1: its charter, its budget rule and its model.
  It is untracked, has no deadline, and keeps its transcript.
- **Its conversation** is what `chat show` reads: each turn's words, from
  the party and from the agent, and each run's start and end.
- **A reply is not a result.** The agent's words answer the party,
  and the chat goes on: it ends only when its party closes it.
- **A change is another task's.** Asked for a change, the chat's run
  asks for it with the host tool `change`: a change task, its
  delegate, writes it, opens its pull request and lands it, and the
  chat hears its result (temper help task).

## How it changes

- **Running, then parked:** a run answers, waits a while for more
  words, then parks; the chat waits for news, `Waiting for news`, and
  a message makes it due for its next run.
- **Closing, then closed:** closing moves the live direct delegates
  the party names to the party, cancels the others at once, then
  gives the chat's run its template's grace, 5 seconds in the stock
  template, and cancels it. A closed chat is not reopened.

## What temper guarantees

- **Words between turns:** a message sent while a run works reaches
  it at the next turn's boundary.
- **Each delegate decided at the close:** none is left without an
  owner; the ones not moved are cancelled with the chat.
- **The transcript stays** after the close, and `chat show` reads it.

## Acting on it

- **ctl:** `chat start` starts one in a project and `chat show` reads
  one; `task message` sends it words; `chat close` ends it; `task
  show` on its task with `--until replied` returns once it has
  answered or waits on someone; `task tree` shows the change tasks it
  started.
- **temper's runs** in a chat use the host tools `change`,
  `delegate` and `message`, and read these pages with `read_docs`.

## Limits

- temper does not let ctl choose a chat's template: `chat start` uses the
  project's chat template.
- temper does not promise a final summary before the close's grace
  ends.
- A chat whose request goes past what the party may start alone is
  proposed instead, and starts only once accepted.
