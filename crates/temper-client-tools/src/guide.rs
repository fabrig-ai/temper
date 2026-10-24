//! The agents' guide. Call examples are checked against the table.
pub const GUIDE: &[u8] = br#"Agent tools

Sign in with the engine host's link on stdin:
  temper ctl DIRECTORY sign-in -
Read your party, projects, bounds and request retention:
  temper ctl DIRECTORY me
Wait, read what needs you, copy an act and its revisions, then act:
  temper ctl DIRECTORY wait me --until waiting
  temper ctl DIRECTORY inbox
  temper ctl DIRECTORY task 41 --tree
  temper ctl DIRECTORY message 41 investigating --key investigate-41
Loop by waiting again; use the matching read's digest with --since to wait for a change.
With --since and --until, both must hold. Read acts and their effects before sending.
A stopping engine's wait continues through reconnect and fails only past the link deadline.

Read deployment settings, then read a whole value as JSON, put its value member
in an items FILE, and edit it before one keyed decision:
  temper ctl DIRECTORY settings
  temper ctl DIRECTORY setting charter 3 --json
  temper ctl DIRECTORY change-settings FILE --against 5 --key settings-5
Use setting ... --json > FILE to save a value; FILE for change-settings is
{"items":[{"setting":{"kind":"charter","number":3},"set":VALUE}]}.
Running work stays unless --running moves. On stale, read settings again and retry.

Read accounts without values; model names and limits are supplied facts:
  temper ctl DIRECTORY accounts
  temper ctl DIRECTORY models --project shop
  temper ctl DIRECTORY wait accounts --until usable
models has its own joined digest; wait accounts uses the accounts read's digest.
Settled follows party-requested sign-ins; program checks and provider readiness await upstream facts.
  temper ctl DIRECTORY connect 1 --key connect-llm
  temper ctl DIRECTORY add-key 1 FILE --deployment --key add-llm
  temper ctl DIRECTORY replace-key 43 FILE --key replace-llm
  temper ctl DIRECTORY declare 1 claude_code claude-work --key declare-llm
  temper ctl DIRECTORY verify 43 --source program
ADAPTER must appear in the provider's programs in me; the declare request carries no adapter binding.
NAME is 1 to 32 lower-case letters, digits or hyphens.
declare prints temper program-login NAME to run on each host; the tools never run it.
verify accepts only a program source, answers once asked, then prints wait accounts --until settled.
Move a signed-in account between deployments:
  temper ctl DIRECTORY export 43 FILE
  temper ctl DIRECTORY import FILE --key move-in
export creates FILE new and private before sending, writes and syncs the record, then sends written.
It takes no --key. Until written, failures remove FILE; lost exports can be tried again after the move's deadline.
After written, FILE is kept unless the engine refuses cancelled. A later call resends a kept written.
import reads a private owned regular file, never a link: 1 to 131072 bytes whole, with no newline trimmed.
A newer JIGM form requires upgrading temper before sending. Other forms are the engine's to decide.
Moved in prints the account and removes FILE; a failed removal exits 1 and says its record is spent.
Refused or lost imports keep FILE: ask again with the same --key and FILE. No record is printed or stored.
Keys use private files, never stdin or arguments; 1 to 4096 bytes after stripping one final newline.
Value requests are sent once, never kept or automatically replayed. Unsent/lost exits 1;
rerun with the same --key and file to recover the original answer. Only engine refusals exit 2.
disconnect, forget, rank and retry resolve a unique --source sign_in|key|program from accounts.
forget without --source removes the whole account; rank without it moves the account.
connect prints a browser address and wait accounts --until settled, then exits once durable.

Connect an existing GitHub App with its private PEM file:
  temper ctl DIRECTORY forge-connect 3 github-app 4242 FILE --key app-3
  temper ctl DIRECTORY wait forges --until connected:3
Only github-app is offered; github, forgejo and forgejo-admin exit 2 as not offered yet.
The private file must be regular, owned by you, with no group or other permissions.
PEM is 1 to 4090 bytes after one final newline; the carried bundle is at most 4096 bytes.
It is sent once, never kept or replayed; use the same --key and file to recover a lost answer.
Install the connected App on another account by opening the printed address on the engine's machine:
  temper ctl DIRECTORY forge-install 3
  temper ctl DIRECTORY repositories 3 --page 1
  temper ctl DIRECTORY wait forges --until settled
The tools print links; they never open a browser. A stopped install prints connecting and exits 1.

Start the built-in playground, or finish a previous failed start:
  temper ctl DIRECTORY demo-start --key playground
It waits for running; not offered exits 2, a start failure exits 1 and names why.

Choose a chat's main model from models --project P; --effort is required when that model declares efforts:
  temper ctl DIRECTORY chat --project shop --model chosen-model --effort chosen-effort hello --key model-chat
  temper ctl DIRECTORY amend 41 --rev 7 --model chosen-model --effort chosen-effort because --key model-amend
amend replaces each named control and keeps the rest; --rev names the shown spec revision.
Goal model flags arrive with area 2's goal tool and its executor/budget/deadline inputs.
Without these flags the template's choice or engine default is sent unchanged.
A model change preserves the supplied account and sub-agent choices; there is no --account flag.
Unknown model or effort names fail locally (exit 1), without sending.

Name keys for chat, message, close, adopt, connect, add-key, declare, import, change-settings and forge-connect; export takes no --key and other acts accept an optional --key NAME.
Names are one to 64 lower-case letters, digits or hyphens.
Give each agent a distinct prefix, such as investigate-41; reads and waits need no key.
A repeat keeps its name, party and ask within retention. A changed template or bound in me
can make another ask: the engine then answers a key conflict. Never reuse a name for new work.
TEXT is an argument or - for stdin, used only once.
--call FILE|- reads one JSON call; TEXT - is forbidden under --call -.
--json gives one object in snake case; help TOOL --json gives its table record.

Exit statuses:
0: a read answered, an act done or proposed, or a wait met. A sized too-large turn stays
   exit 0; report it as a finding and read smaller parts where offered.
1: usage or client failure, including limits, Held, NotKept and unknown past retention.
   Held may name another party's parked request; it is a client failure.
2: an engine refusal alone, including an engine key conflict.
3: an unmet wait's timeout or watch end. A signal's status is the OS's, never 0.
Unconfirmed or a client-side key conflict is a tools bug (exit 1); report it as a finding.

Recovery notices go on stderr, for example:
kept: create done: task 41, chat
A kept JSON notice has only operation and outcome, one object per stderr line, never a request name or key.
pending reads only the store, with no lock, client or send. Each row gives operation,
party N and first sending in wall-time words, or not sent. For example:
chat, party 3, not sent
JSON: {"pending":[{"operation":"chat","party":3,"first_sent":null}],"unread":2}
2 kept records these tools cannot read; a call that sends drops them
Unsupported store versions or undeclared family owners/versions are counted, never listed (exit 0).
An invalid store fails: DIRECTORY's store cannot be read (exit 1).
A sending call reports dropped and parked work on stderr, then goes on:
dropped: 2 kept records these tools cannot read
JSON: {"dropped":2}
kept: 2 for party 3, unsent while it is not signed in here
JSON: {"party":3,"unsent":2}
It answers and erases only the signed-in party's kept requests; other parties' work stays parked.

Read forge changes with the forge API or git. Change the forge only through temper.
A part served there says: Shown on the forge.

Tools:
"#;

/// The local pending tool's result, without opening a client.
pub const PENDING: &[u8] = br#"Reads only the store: no lock, client or send.
Rows: operation, party N, first sending in wall-time words or not sent.
chat, party 3, not sent
{"pending":[{"operation":"chat","party":3,"first_sent":null}],"unread":2}
2 kept records these tools cannot read; a call that sends drops them
Unread records are counted, never listed; exit 0.
An invalid store: DIRECTORY's store cannot be read (exit 1).
"#;
