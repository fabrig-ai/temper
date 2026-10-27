# Changes

Kind: concept

A change is a task that writes one repository's branch and lands it: its pull request, CI at its head, its gates, the landing queue, its repairs and the merge, each step as change show prints it.

## What it is

- **A change task** is a delegate a run asks for with the host tool
  `change`, giving the words and, where its project has several
  repositories it may write, which one. Its coder writes the change
  from those words alone (temper help task).
- **Its panel,** as `change show` prints it: `change TASK ·
  OWNER/NAME on forge FORGE · BRANCH into BASE`, its pull request and
  its address, `step:` and the step, `way: turn · landing task N`
  where its project lands by turn, the CI at its head, its gates, and
  how many repairs, resolutions and updates it has used of its limits.
- **Its result** is the change landed: its merge commit, its pull
  request with its address, then what the landing passed as the change
  saw it: `checks at HEAD:` each check at the landed head with its
  conclusion, `repairs: N:` why each repair was asked, and `reviews:`
  each verdict at the head it judged, cut from the end and marked when
  long. It names no check or test the change didn't see. Or it fails,
  or is cancelled, as any task.

## How it changes

Its steps, as `step:` prints them:

- `producing`: the coder writes the change in its files, and temper
  pushes its work as the change's branch once it finishes;
- `opening`: temper opens its pull request;
- `checking`: the forge's CI runs at its head;
- `gating`: its blocking gates give their verdicts (temper help
  landing-rule);
- `queued`: it waits its turn in its landing branch's queue, `queued:
  place N`;
- `first`: at the front, temper checks CI and the blocking gates again;
- `updating`: a branch behind its landing branch is updated by the
  forge, which merges the base in;
- `resolving`: an agent resolves a conflict with the base and pushes
  the merge commit;
- `repairing`: an agent repairs it, after CI failed at its head (a
  review still running there is cancelled first), a gate asked for
  changes, or the base broke it, its brief holding the failed check
  and the end of its log; the change is checked again;
- `landing`, then `landed`: temper merges it, and `change show` names
  the merge commit.

Where its project lands by turn (temper help landing), no CI runs at
its head and it waits in no queue: `gating` follows `opening`, then:

- `ready`: its gates hold at its head, and its landing, `way: turn ·
  landing task N`, proves its candidate and waits for the turn;
- `preparing`: the forge rebases its branch onto main for its
  landing, and it is `ready` again at the new head;
- `repairing`: a gate failed at its candidate, given that run's
  output, and its new head makes it ready again;
- `landed`: main was fast-forwarded to its candidate.

After a release, `reopening` reopens its closed pull request and
`recreating` pushes its missing branch again.

**Held,** `held: REASON at STEP`, it makes no progress: `its wait
stalled`, `its repairs are spent`, `its resolutions are spent`, `its
updates are spent`, `its branch was moved on the forge`, `its branch is
missing on the forge`, `its pull request was closed on the forge`, `its
pull request was retargeted on the forge`, `a delegate or a write
failed for good`, or `a person rejected a gate`. A release resets its
counts and goes on from where it was held. Its panel also names a held
task, the change's own or its writer's or reviewer's, `held: task N
(REASON)`, with that task's way out.

## What temper guarantees

- **Landed at the head that passed:** temper merges naming the head
  whose CI and gates passed; a moved head or base is checked again
  first.
- **Nothing overwritten:** a branch or pull request changed by someone
  else on the forge holds the change.
- **Gates decide:** a blocking gate that asks for changes starts a
  repair; a person's rejection holds it; an advisory gate never blocks.
  A reviewer that gives no verdict is asked once more, then the change
  is held.
- **Bounded work:** past its limit of repairs, resolutions or updates,
  the change holds rather than trying again.
- **The parent hears its result,** landed or not.

## Acting on it

- **ctl:** `change show` reads its panel and waits for its pull
  request, its CI, a hold or its landing (temper help ctl); a wait for
  its landing ends at a hold, `not met: held` (exit 3); `task show`
  reads its task and its acts, `task release` among them; `task tree`
  shows its delegates; `landing show` reads its landing, where its
  project lands by turn.
- **temper's runs** ask for one with `change`, and hear landings on a
  branch with `subscribe_forge`.
- **The forge** holds its diff, its reviews and CI's logs: read them
  there, at the pull request `change show` prints.

## Limits

- A change writes one repository, and lands on its landing branch.
- temper does not let ctl start a change: a run asks for one, as a
  chat's does when asked (temper help chat).
- `no change panel for task N` (exit 2): task N is no change.
- A release does not retarget back a pull request retargeted on the
  forge.
