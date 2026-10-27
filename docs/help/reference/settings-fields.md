# Settings fields

Kind: reference

Every member of a settings value: its type, its bound and what it does to work, as a settings file sets it and `settings show` prints it.

A settings file is a `file`, below; each section is one shape of JSON object, and each bullet one of its members, with its type, its bound and what it does. Read a value with `settings show`, given its kind and number, edit it, and set it whole. A bound here is what ctl sends; the deployment may still refuse a value, naming its fault: `shape`, `count`, `reference`, `rules`, `budget`, `grace` or `owner`. Spend is in the deployment's units, those of its models' prices. Hex is bytes in lower-case hexadecimal: `666f726765` is `forge`.

## file

A settings file, as `settings apply` and `settings propose` read it.

- `items`: list of at most 1,024, each `item`. The change: an item for each setting it touches, each setting once.

## item

One setting, and what becomes of it: `setting`, then exactly one of `set` and `edit`.

- `setting`: `setting`. The setting the item changes.
- `set`: the setting's kind's value: `charter`, `template`, `policy` or `rules`. Its new value, whole, which replaces the one before.
- `edit`: `"removed"` or `"reset"`. What becomes of the setting, in place of a value. `"removed"`: No longer offered, its default included; the rules cannot be removed. `"reset"`: Its change erased, so that it follows its default again.

## setting

Which setting: its kind and, but for the rules, its number.

- `kind`: `"charter"`, `"template"`, `"policy"` or `"rules"`. The setting's kind.
- `number`: integer, 0 to 4,294,967,295. The setting's number, from 1; left out for the rules.

## charter

How an agent's run works: its instructions, models, tools, hosts and budget. A task's runs read the charter of the settings revision the task was made in.

- `number`: integer, 0 to 4,294,967,295. The charter's number, the same as `setting.number`.
- `tools`: integer. The host families its runs are offered, where their task's authority holds them too
- `instructions`: text, at most 1,048,576 bytes. What the run's agent reads first, before its task's brief.
- `waiting`: nanoseconds. How long a run with nothing to do waits for words or news before it parks.
- `resume`: true or false. Whether a run resumes its task's transcript, when there is one, rather than start afresh.
- `turns`: integer, 0 to 4,294,967,295. The most model turns a run takes; more than `reserve_turns`, else refused as `budget`.
- `time`: nanoseconds. The longest a run lasts before it is cancelled; more than `reserve_time`, else refused as `budget`.
- `reserve_turns`: integer, 0 to 4,294,967,295. Of `turns`, those kept for the run to finish its work and report.
- `reserve_time`: nanoseconds. Of `time`, the part kept for the run to finish its work and report.
- `model`: `model`. The default model: a task that names none takes the first of `model` and `alternatives` whose provider has an account that can pay.
- `alternatives`: list of at most 16, each `model`. The other models the charter offers, in order after `model`, each named once; a run's sub-agents may use them too.
- `inspect`: true or false. Whether the run's agent may read, list and search its workspace.
- `modify`: true or false. Whether the run's agent may write and edit files in its workspace.
- `shell`: true or false. Whether the run's agent may run commands in its workspace.
- `agents`: true or false. Whether the run's agent may open sub-agents, each with no more tools than its own.
- `call_timeout`: nanoseconds. The longest one of the run's calls to temper's tools may take.
- `hosts`: `"workers"`, `"engine"` or `"both"`. Which hosts may run it. `"workers"`: The deployment's workers alone. `"engine"`: The engine's own slots alone. `"both"`: Either.
- `spend`: integer. A spend the charter must hold: at least `reserve_turns` plus one of the default model's completions at their costliest, else refused as `budget`. A run is given its task's funds left, at most `rules.maximum_run_spend`.

## model

One of the deployment's models, as a charter offers it. Every member but `effort` must equal the configured model's, else refused as `reference`: copy them from a charter's value.

- `dialect`: integer, 0 to 4,294,967,295. The protocol the model is spoken to in, as configured.
- `endpoint`: integer, 0 to 4,294,967,295. The number of the configured endpoint that serves it.
- `program`: text, at most 32 bytes, or null. The program that runs it, as configured, or null for none.
- `name`: text, at most 4,096 bytes. The model's name, as its provider knows it.
- `window`: integer, 0 to 4,294,967,295. The most tokens of input it reads.
- `output`: integer, 0 to 4,294,967,295. The most tokens it writes in one completion, thinking included.
- `input_price`: integer. What fresh input costs, in spend units per `price_unit` tokens.
- `cached_price`: integer. What cached input costs, in spend units per `price_unit` tokens.
- `cache_write_price`: integer, or null. What writing its cache costs, in spend units per `price_unit` tokens, or null to price it as fresh input.
- `output_price`: integer. What output costs, in spend units per `price_unit` tokens.
- `price_unit`: integer, 0 to 4,294,967,295. The number of tokens each price is for.
- `efforts`: list of at most 16, each text, at most 32 bytes. The efforts it takes, in order, as configured.
- `effort`: text, at most 32 bytes, or null. The effort a run asks of it, one of `efforts`, or null for its default.

## template

How a new task of one shape is filled: its executor, result, budget, deadline and flags. A task keeps the values it was made with; a change reaches the tasks made after it.

- `number`: integer, 0 to 4,294,967,295. The template's number, the same as `setting.number`; a chat is filled from template 1 and a goal from template 2.
- `name`: `text`. Its name, as clients show it.
- `executor`: `executor`. Who runs the task.
- `contract`: `contract`. The result the task must give.
- `shallower`: integer, 0 to 4,294,967,295. How many levels of delegation the task has below its creator's role: 1 is one level; 0 leaves the template unusable.
- `budget`: `budget`. What the task may spend.
- `deadline`: `deadline`. By when the task must end.
- `tracked`: integer, 0 to 4,294,967,295, or null. Its default priority as tracked work, lower first, or null for untracked.
- `flags`: `flags`. How the task behaves and is kept.
- `label`: integer, 0 to 4,294,967,295. A number that groups the task in the lists of ended tasks; temper reads nothing else from it.
- `grace`: nanoseconds, or null. How long a chat's close lets its runs finish before it cancels them, or null; past the deployment's limit, refused as `grace`.
- `llm`: `llm`, or null. The models the task asks for, or null for its charter's default.

## text

A text, whole.

- `bytes`: text, at most 32,768 bytes. The text.
- `cut`: true or false. Always false: a value is set whole.

## executor

Who runs a task: a word, or an object with one member.

- `"chosen"`: The creator chooses a charter or procedure their role may create with.
- `charter`: integer, 0 to 4,294,967,295. An agent under this charter, which must be offered, else refused as `reference`.

## contract

The result a task must give: an object with one of these members.

- `approval`: `approval`. A person's approval of an exact state; an agent given one can only fail.
- `report`: `report`. A report.
- `verdict`: `verdict`. One of a set of verdicts.
- `change`: `change`. A connector's result.

## approval

An approval of an exact state.

- `state`: hex, at most 32,768 bytes. The state approved, which the approval must give back byte for byte.
- `part`: `part`. What a client shows the approval with.

## part

A thing a connector owns, which a client shows.

- `connector`: integer, 0 to 65,535. The connector that owns it: 1 is the forge.
- `kind`: `"resource"`, `"pool"`, `"result"`, `"procedure"`, `"effect"`, `"outbox"` or `"hold"`. What kind of thing it is.
- `key`: `key`. Which one.

## key

Which thing: an object with one of these members.

- `path`: list of at most 9, each hex, at most 32,768 bytes. A resource's name, its segments as hex: `forge`, the host, the owner and the repository, then `branch` and the branch's segments.
- `number`: integer. A number the connector knows it by.

## report

A report in words.

- `words`: integer, 0 to 4,294,967,295. The most bytes the report may hold.

## verdict

A verdict: one of its choices, with the fields that choice asks for.

- `choices`: list of at most 1,048,576, each `choice`. The choices the task gives one of.
- `cut`: true or false. Always false: a value is set whole.

## choice

One verdict a task may give.

- `code`: integer, 0 to 4,294,967,295. Its number, unique among the choices.
- `words`: integer, 0 to 4,294,967,295. The most bytes of the verdict's explanation.
- `followups`: integer, 0 to 4,294,967,295. The most follow-up delegates the verdict may ask for.
- `fields`: list of at most 32, each `choice_field`. The fields the verdict must fill, each named once.

## choice_field

A field a verdict fills.

- `name`: hex, at most 32,768 bytes. Its name as hex, not empty.
- `max`: integer, 0 to 4,294,967,295. The most bytes of its value.

## change

A connector's result: for the forge, a change.

- `connector`: integer, 0 to 65,535. The connector that gives the result: 1 is the forge.
- `kind`: integer, 0 to 65,535. The kind of result: the forge's 1 is a landed change, its 2 a change made but not landed.
- `words`: integer, 0 to 4,294,967,295. The most bytes of the result's report, each of the change's title and body.

## budget

What a task may spend: a word, or an object with one member.

- `"given"`: The creator gives it, within their allotment.
- `amount`: integer. This much.
- `as_allowed`: `as_allowed`. As much as the creator has left when the task is made.

## as_allowed

As much as fits.

- `cap`: integer, or null. The most it may be, or null for no cap.

## deadline

By when a task must end: a word, or an object with one member.

- `"none"`: No deadline.
- `"given"`: The creator gives one, or none.
- `span`: nanoseconds. This long after the task is made.

## flags

How a task behaves and is kept.

- `conversation`: true or false. Whether the task is a conversation: its requester's words are its turns, and it moves to the newest revision at its next run.
- `listed`: true or false. Whether it is listed, by project and label, once it has ended.
- `retention`: `"kept"` or `"forgotten_once_read"`. How long it is kept. `"kept"`: Until the deployment's retention horizon. `"forgotten_once_read"`: Until its requester has read it and no live task names it.
- `ended_by`: `"result"` or `"requester"`. What ends it. `"result"`: Its run's result. `"requester"`: Its requester alone; it needs `conversation` and a charter as `executor`, else refused as `shape`.

## llm

The models a task asks for.

- `account`: integer, 0 to 4,294,967,295, or null. The account to pay with, or null for any that can.
- `main`: `model_effort`. The main model and its effort.
- `subagents`: list of at most 64, each `model_effort`. The models its runs' sub-agents may use, each named once.

## model_effort

A model, by name, and its effort.

- `model`: text, at most 4,096 bytes. A configured model's name, one the task's charter offers.
- `effort`: text, at most 32 bytes, or null. One of the model's efforts, or null for a model that takes none.

## policy

What a project made from it starts with, as a copy it keeps. A change reaches the projects made after it, never an existing project's own policy.

- `escalation_role`: integer, 0 to 4,294,967,295, or null. The role an escalation reaches last, or null; that role must accept proposals and decide escalations.
- `ceiling`: `authority`. The most any task in the project may have; within the rules, else refused as `rules`.
- `projections`: list of at most 32, each `grant`. The grants under which temper writes goals' projections into the project's homes; each within `ceiling`.
- `period_spend`: integer. What the project may spend in a period; within `rules.period_spend`.
- `roles`: list of at most 32, each `role`. The project's roles: what a party in each may do.
- `requirements`: list of at most 32, each `requirement`. What an effect in the project needs, beside the rules' requirements.
- `permissions`: list of at most 32, each `permission`. The project role each forge collaborator's permission gives, at adoption.
- `templates`: list of at most 16, each `template_override`. The project's overrides of templates.

## role

A role in a project.

- `number`: integer, 0 to 4,294,967,295. Its number: 0 is owner, 1 maintainer, 2 member, 3 observer; others are the policy's own. Role 0 must be able to create tasks, else refused as `owner`.
- `authority`: `authority`. The most a party in it may give a task; within the policy's `ceiling`.
- `period_spend`: integer. What a party in it may allot in a period; within the policy's `period_spend`.
- `requests`: integer, 0 to 65,535. What a party in it may ask, a bit each: 1 create, 2 allot, 4 accept, 8 amend, 16 cancel, 32 release, 64 move, 128 watch, 256 change the policy.
- `decides`: integer, 0 to 255. The proposals a party in it may decide, a bit each: 1 a batch of tasks, 2 an effect, 4 a widening, 8 an amendment, 16 an escalation.

## template_override

A project's override of a template; a member left null keeps the template's own.

- `template`: integer, 0 to 4,294,967,295. The template's number; offered, and once in the policy.
- `executor`: `executor`, or null. Its executor, or null.
- `budget`: `budget`, or null. Its budget, or null.
- `deadline`: `deadline`, or null. Its deadline, or null.
- `priority`: integer, 0 to 4,294,967,295, or null. Its default priority, or null; an untracked template ignores it.

## permission

A forge collaborator's permission, and the project role it gives at adoption.

- `connector`: integer, 0 to 65,535. The connector: 1 is the forge.
- `permission`: integer, 0 to 65,535. The permission: 0 none, 1 read, 2 write, 3 admin.
- `role`: integer, 0 to 4,294,967,295. The role it gives, one of the policy's `roles`.

## rules

The deployment's ceilings: every project's policy is within them, and every decision reads them afresh. A narrowing reaches existing tasks; a widening never widens one.

- `ceiling`: `authority`. The most any project may have.
- `period_spend`: integer. The most a project's period spend may be.
- `minimum_run_spend`: integer. A run starts only with more than this to spend.
- `maximum_run_spend`: integer. The most a run is given of its task's funds; more than `minimum_run_spend`, and within `ceiling`'s spend.
- `implies`: list of at most 32, each `implication`. Kinds of a connector's whose grant grants other kinds too.
- `requirements`: list of at most 32, each `requirement`. What an effect needs, in every project.

## implication

One kind whose grant grants another.

- `connector`: integer, 0 to 65,535. The connector: 1 is the forge.
- `kind`: integer, 0 to 65,535. The kind whose grant also grants `implies`.
- `implies`: integer, 0 to 65,535. The kind it grants.

## authority

What a task, a role or a ceiling may do: its tools, models, grants, delegation, spend and deadline.

- `models`: `models`. The models its runs may use.
- `tools`: integer. The engine's tool families its runs may call, a bit each: 1 delegate, 2 message, 4 amend, cancel and release, 8 decide and withdraw, 16 propose, 32 subscribe, 64 effect, 128 note, 256 recall, 512 the forge's reads, 1024 settings.
- `grants`: list of at most 1,048,576, each `grant`. The effects and reads it may make, each a connector's kind on some resources.
- `delegation`: `delegation`. The delegates it may make.
- `spend`: integer. The most its tree may spend.
- `deadline`: nanoseconds since 1970 UTC, or null. The latest it may end, or null for no deadline.
- `notes`: integer, 0 to 255. The scopes it may write notes in, a bit each: 1 its goal, 2 its project, 4 the deployment.
- `note_resources`: list of at most 1,048,576, each `resource`. The resources it may write notes on.

## models

Which models: a word, or an object with one member.

- `"all"`: Every configured model, those configured later included.
- `named`: list of at most 64, each hex, at most 4,096 bytes. These models alone, each name as hex; an empty list allows none.

## grant

Leave to make one kind of a connector's effect or read, on some resources.

- `connector`: integer, 0 to 65,535. The connector: 1 is the forge.
- `kind`: integer, 0 to 65,535. The kind; the forge's are 1 read, 2 push, 3 open a pull request, 4 merge, 5 review, 6 set a status, 7 comment, 8 open an issue, 9 make a branch, 10 edit a pull request, 11 update it from its base, 12 close or reopen it, 13 edit an issue, 14 set reviewers, 15 delete a branch.
- `pattern`: `pattern`. The resources it covers.

## pattern

Resources by name: the exact `segments`, then `last`. A forge's repository is `forge`, the host, the owner and the repository; a branch adds `branch` and its segments.

- `segments`: list of at most 9, each hex, at most 32,768 bytes. The first segments, each as hex, each matched exactly.
- `last`: `last`. The segment after them.

## last

The last segment: an object with one of these members.

- `exact`: hex, at most 32,768 bytes. Exactly this segment, as hex, and nothing beneath it.
- `open`: hex, at most 32,768 bytes. Any segment that begins with this, as hex, and everything beneath it; empty covers every segment.

## delegation

The delegates a task may make.

- `kinds`: list of at most 12, each `delegate`. Who its delegates may be.
- `tasks`: integer, 0 to 4,294,967,295. How many tasks its tree may make over its life, not at once.
- `depth`: integer, 0 to 4,294,967,295. How far below it its tree may go; 0 allows no delegate.

## delegate

A kind of delegate: an object with one of these members.

- `charter`: integer, 0 to 4,294,967,295. An agent under this charter, which must be offered.
- `procedure`: integer, 0 to 4,294,967,295. This procedure: 2 is the forge's change.
- `role`: integer, 0 to 4,294,967,295. A person in this role.

## resource

Resources it may write notes on.

- `connector`: integer, 0 to 65,535. The connector: 1 is the forge.
- `pattern`: `pattern`. The resources.

## requirement

What one kind of a connector's effect needs on some resources, whatever any authority says.

- `connector`: integer, 0 to 65,535. The connector whose effect it constrains: 1 is the forge.
- `kind`: integer, 0 to 65,535. The kind of effect; implications do not apply.
- `pattern`: `pattern`. The resources it applies to.
- `judge`: `judge`. Who judges it.
- `guard`: `guard`. When it is judged.
- `must_be_guarded`: true or false. Whether the system that makes the effect must check it as it does; an effect that system cannot guard is refused.

## judge

The connector that judges a requirement, and its criterion.

- `connector`: integer, 0 to 65,535. The judging connector: 1 is the forge.
- `requirement`: integer, 0 to 65,535. Its criterion; the forge's are 1 CI passed at the head, 2 the head up to date with its base, 3 a gate, 4 approvals.
- `parameters`: integer, 0 to 4,294,967,295. A number the judging connector reads the criterion's terms by.

## guard

When a requirement is judged: a word, or an object with one member.

- `"guarded"`: As the effect is made, by the system that makes it.
- `observed`: `observed`. Before the effect is made, from facts read recently enough.

## observed

A requirement judged from facts read before the effect.

- `freshness`: nanoseconds. How old those facts may be.
