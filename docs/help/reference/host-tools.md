# Host tools

Kind: reference

Every tool temper adds to its runs' own: what it does, as a run reads it, whether it reads or writes, and the members of its input.

## change

Have one change written on a branch of a repository of this project, opened as a pull request and merged once its checks pass; you hear its result.

A write.

- `words`: text; required. What to change: the files, and the exact text where it matters. It is all the change's coder reads.
- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one you may write.

## delegate

Make a batch of direct delegates within your authority. tracked {priority} makes one a goal, the lower first. For a change to a repository, call change.

A write.

- `batch`: a list; required. The tasks: each with executor, spec, contract, authority; optionally deadline_seconds, tracked, dependencies, wake, llm.

## amend

Change a live delegate within your authority, naming only what changes and the spec revision you read; authority and instructions are whole.

A write.

- `target`: an integer of at least 1; required. The delegate's number.
- `revision`: an integer of at least 0; optional. The spec revision you last read. Leave it out only to change a tracked delegate's priority alone.
- `amendment`: an object; required. The reason, which its live run hears, and what changes: tracked, instructions, spec, authority, dependencies, wake, llm.

## message

Send words, a question, or the answer to a question, to a task you reference. Its run reads it at its next turn.

A write.

- `target`: an integer of at least 1; required. The task's number.
- `form`: one of `words`, `question` or `answer`; required. words; a question, which its run may answer; or an answer, naming the question.
- `words`: text; required. What you say.
- `question`: an integer of at least 1; optional. For an answer: the question's number.

## cancel

Cancel a live delegate and every task below it, with a reason.

A write.

- `target`: an integer of at least 1; required. The delegate's number.
- `reason`: text; required. Why it is cancelled.

## release

Lift a held delegate's hold, naming the hold's revision. For an effect held uncertain, say whether to make it again.

A write.

- `target`: an integer of at least 1; required. The held delegate's number.
- `revision`: an integer of at least 0; required. The hold's revision, as you last read it.
- `again`: true or false; optional. For an effect held uncertain alone: true makes it again, false does not.

## introduce

Give two tasks you reference a reference to each other, so that each can message the other.

A write.

- `left`: an integer of at least 1; required. One task's number.
- `right`: an integer of at least 1; required. The other task's number.

## subscribe

Hear of a task you reference, held or ended, or of a timer, once or periodic. Answers the subscription's number.

A write.

- `kind`: one of `task` or `timer`; required. task, to hear of a task; timer, to hear at a time.
- `target`: an integer of at least 1; optional. For task: the task's number.
- `held`: true or false; optional. For task: true to hear when it is held; false when absent.
- `result`: true or false; optional. For task: true to hear its result; true when absent.
- `at`: an integer of at least 0; optional. For timer: when, in wall-clock nanoseconds since the Unix epoch.
- `period`: an integer of at least 1; optional. For timer: again every this many nanoseconds; once when absent.

## unsubscribe

Remove one of your subscriptions.

A write.

- `subscription`: an integer of at least 1; required. The subscription's number, as subscribe answered it.

## decide

Decide a proposal waiting for you: accept it, reject it with a reason, or pass it up to the holder above.

A write.

- `proposer`: an integer of at least 1; required. The proposing task's number.
- `proposal`: an integer of at least 1; required. The proposal's number among its proposer's.
- `decision`: one of `accept`, `reject` or `pass`; required. accept, reject or pass.
- `reason`: text; optional. Why; required to reject.

## decide_escalation

Decide a held descendant's escalation: release it, reject it with a reason, or pass it up to the holder above.

A write.

- `task`: an integer of at least 1; required. The held task's number.
- `revision`: an integer of at least 1; required. Its hold's revision, as you last read it.
- `decision`: one of `release`, `reject` or `pass`; required. release, reject or pass.
- `reason`: text; optional. Why; required to reject.

## propose

Propose an act beyond your authority, with a reason, to the holder who may accept it: a batch, a forge effect, an amendment, a release or a widening.

A write.

- `action`: one of `batch`, `effect`, `amend`, `widen` or `release`; required. The act; each takes the members named for it below.
- `task`: an integer of at least 1; optional. For amend, widen and release: the task's number.
- `revision`: an integer of at least 0; optional. For amend: its spec revision, as you last read it.
- `batch`: a list; optional. For batch: the tasks, as delegate's batch.
- `amendment`: an object; optional. For amend: what changes, as amend's amendment.
- `authority`: an object; optional. For widen: the authority wanted, whole.
- `reason`: text; required. Why; the holder reads it.
- `as_holder`: true or false; optional. For batch: true makes the tasks the accepter's delegates rather than yours.
- `repository`: an object; optional. For effect: as effect_forge's repository.
- `resource`: an object; optional. For effect: as effect_forge's resource.
- `write`: an object; optional. For effect: as effect_forge's write.

## withdraw

Withdraw one of your pending proposals.

A write.

- `proposal`: an integer of at least 1; required. The proposal's number, as propose answered it.

## note

Write a note in a scope for later runs to recall, or revise one, naming the revision you recalled.

A write.

- `scope`: an object; required. {kind}: deployment; project, with project; goal, with project and goal; resources, with project, connector, pattern.
- `description`: text; required. What the note is about: a search reads it.
- `body`: text; required. The note's text.
- `references`: a list; optional. The numbers of the tasks it refers to.
- `name`: an integer of at least 1; optional. To revise a note: its name. Leave it out for a new note.
- `recalled`: an integer of at least 1; optional. To revise a note: its revision, as recall answered it.

## recall

Recall a note by its name, or search the descriptions of the notes in some scopes, a page at a time.

A read.

- `kind`: one of `name` or `search`; required. name, by its name; search, by words in scopes.
- `name`: an integer of at least 1; optional. For name: the note's name.
- `scopes`: a list; optional. For search: the scopes, each as note's scope.
- `query`: text; optional. For search: the words.
- `page`: an integer of at least 0; optional. The page, from 0; 0 when absent.

## subscribe_forge

Hear forge news: landings on a branch, CI on a head, a pull request's state, or the comments and reviews on an issue or pull request.

A write.

- `topic`: an object; required. {kind, repository: {forge, repository}}: landings with branch, ci with head, pull or participation with number.
- `own_change`: an integer of at least 1; optional. For landings: your change task's number; its own landing does not wake you.
- `paths`: a list; optional. For landings: a landing wakes you when it changes one of these paths, or a file below one.

## effect_forge

Make one write on an adopted repository through the forge's connector: an issue, a comment or a status, among the forge's writes.

A write.

- `repository`: an object; required. The repository, as {forge, repository}: the forge's number and the repository's.
- `resource`: an object; required. What it writes on: {kind: repository}, {kind: pull, number}, {kind: issue, number} or {kind: branch, segments}.
- `write`: an object; required. {kind: create_issue, key, title, body}, {kind: post, number, key, body} or {kind: status, commit, context, check}.

## settings

Read the deployment's settings: the inventory with no member, or a setting, a revision's record or a page of the history; a long answer goes on from an offset.

A read.

- `setting`: an object; optional. One setting's whole value, named as a settings file names it.
- `revision`: an integer from 1 to 4294967295; optional. Alone, that revision's record: its items and who decided it. With a charter, the charter as that held revision has it.
- `history`: an object; optional. A page of the revisions, the newest first.
- `from`: an integer from 0 to 4294967295; optional. Where to go on in the answer's text: the offset after next: on its last line; 0 when absent.

## propose_settings

Propose a change of the deployment's settings to its owners, with a reason. You hear in your messages whether it was accepted, rejected or went stale.

A write.

- `against`: an integer from 1 to 4294967295; required. The newest revision you read. A proposal against an older one is stale, and you hear the newest.
- `reason`: text; required. Why; the owners read it.
- `items`: a list; required. The changes, at most one per setting.

## read_items

Read a page of a repository's issues and pull requests, those updated least recently first, from a time on.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `since`: an integer from 0 to 18446744073709551615, 0 when absent; optional. Only items updated at or after this time, in nanoseconds.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.
- `item_kind`: one of `issue` or `pull`; optional. Issues or pull requests alone; both when absent.

## read_item

Read an issue or a pull request with its comments.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `number`: an integer from 1 to 18446744073709551615; required. The issue's or pull request's number.
- `after`: an integer from 0 to 18446744073709551615, 0 when absent; optional. Only the comments after this comment's id.

## read_pull

Read a pull request.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `number`: an integer from 1 to 18446744073709551615; required. The pull request's number.

## read_pull_for

Find the pull request from one branch into another.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `head`: text; required. The branch it merges from.
- `base`: text; required. The branch it merges into.

## read_reviews

Read a page of a pull request's reviews.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `number`: an integer from 1 to 18446744073709551615; required. The pull request's number.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.

## read_statuses

Read a page of the statuses posted on a commit.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `commit`: text; required. The commit's id: 40 or 64 lowercase hexadecimal digits.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.

## read_remarks

Read a page of one review's remarks on a pull request.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `number`: an integer from 1 to 18446744073709551615; required. The pull request's number.
- `review`: an integer from 1 to 18446744073709551615; required. The review's id, as read_reviews answers it.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.

## read_branch

Read a branch's head.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `branch`: text; required. The branch's name.

## read_branches

List the branches whose names start with a prefix.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `prefix`: text; required. The names' prefix.

## read_pull_files

Read a page of the files a pull request changes at one head.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `number`: an integer from 1 to 18446744073709551615; required. The pull request's number.
- `head`: text; required. The head's commit id: 40 or 64 lowercase hexadecimal digits.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.

## read_compare

Compare two commits.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `before`: text; required. The earlier commit's id: 40 or 64 lowercase hexadecimal digits.
- `after`: text; required. The later commit's id: 40 or 64 lowercase hexadecimal digits.

## read_checks

Read a commit's combined checks.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `commit`: text; required. The commit's id: 40 or 64 lowercase hexadecimal digits.

## read_file

Read a file at one head, cut at a byte limit.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `head`: text; required. The commit's id: 40 or 64 lowercase hexadecimal digits.
- `path`: text; required. The file's path in the repository.
- `max_bytes`: an integer from 1 to 4294967295; optional. At most this many bytes; the read's limit when absent.

## read_job

Read one CI job's log, cut at a byte limit.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `head`: text; required. The commit's id the CI ran on: 40 or 64 lowercase hexadecimal digits.
- `run`: an integer from 1 to 18446744073709551615; required. The CI run's id.
- `job`: an integer from 0 to 18446744073709551615; required. The CI job's id, which may be 0 on Forgejo.
- `attempt`: an integer from 0 to 4294967295; required. The attempt's number, which may be 0 on Forgejo.
- `max_bytes`: an integer from 1 to 4294967295; optional. At most this many bytes; the read's limit when absent.

## read_protection

Read a branch's protection.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `branch`: text; required. The branch's name.

## read_settings

Read the repository's settings.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.

## read_collaborators

Read a page of the repository's collaborators.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `page`: an integer from 1 to 4294967295, 1 when absent; optional. The page.

## read_permission

Read a forge user's permission on the repository.

A read.

- `repository`: text; optional. The repository, as owner/name; leave it out when the project has one.
- `user`: an integer from 1 to 18446744073709551615; required. The forge user's id.

## read_docs

Read temper's pages: {topic, from} gives a page from an offset, then next: OFFSET or end; {search} the sections holding every word, a line each.

A read.

- `topic`: text; optional. A page's name, such as index.
- `from`: an integer of at least 0; optional. The offset to read from, as next: gave it; 0 when absent.
- `search`: text; optional. The words a section must hold, every one: at most 8.
