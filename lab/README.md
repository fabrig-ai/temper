# M1 lab pieces

[chat-change-smoke/](chat-change-smoke/README.md) is S, the free NOTICE smoke
from `docs/design/qa.md`. It runs on skein's runner with no driver or model,
using the canonical [cluster/](cluster/) documents, a TLS fake LLM, two
remote workers and the subject's fake git.

The runner's private bootstrap pipes engine stdout into `ctl sign-in -`.
The seed adds the fake key, waits for accounts and forge, adopts `fake/widget`,
then runs the story through `tools/ctl`. The story asks for NOTICE, requires
the delegate's historical successful landing result and a complete root tree
with no live or held delegates. The root chat stays open. Public ctl calls
use exactly `http://HOST:PORT/` from the engine's clients readiness capture.

The live forge collection retains an observation snapshot and object replies;
its end-state check reads those files only. Stopped collections copy the
forge observations before the engine store, after confirmed exits and empty
trees; the promises check reads these copies and independent temper records.
See [checks/README.md](chat-change-smoke/checks/README.md).

With the owner's subject digest, run from this repository:

```sh
L=/path/to/skein/scripts/lab/lab
S=sha256:...
$L validate lab/chat-change-smoke --subject "$S"
$L run lab/chat-change-smoke --subject "$S" --seed 1
$L run lab/chat-change-smoke --subject "$S" --seed 2
$L run lab/chat-change-smoke --subject "$S" --seed 3
$L judge lab/chat-change-smoke --subject "$S"
```

The class stays `temper-m1-calibration`: ready 120 s, command 240 s, stop
20 s. Seed waits allow 200 s; the story has one 220 s deadline. Python's
standard library is the approved temporary lab-tooling exception. Tests run
through `heavy`:

```sh
heavy python3 -I -B -m unittest discover -s lab/tests/unit -v
heavy python3 -I -B -m unittest discover -s lab/tests/process -p 'test_ctl.py' -v
```

[chat-change/](chat-change/README.md) is H, the charter-driven NOTICE story;
[chat-change-killed/](chat-change-killed/) is P, its planted engine kill twin.
The scripts in `chat-change-rehearsal/` play H's correct drive and a false
completion through the real runner, with no model or login. Rehearse with
`$L run lab/chat-change --subject "$S" --seed 1 --rehearse lab/chat-change-rehearsal/correct.json`
and the same command with `wrong.json`: expect exit 0 and exit 1 respectively,
the latter with `check/end-state/not-landed/fake/widget/main`. Before H's
calibration sets `temper-m1`, validate and rehearse an external copy whose
only change is `cost` set to `temper-m1-calibration`.

[live-0a/](live-0a/README.md) is rung 0a of `docs/design/draft/qa-live.md`
(3.1), the ground on real backends with no driver, account or model: a
fresh `temper box` with one owned Forgejo from the lab's `forgejo` and
`forgejo-runner` programs on the kept `forgejo` port. Its seed signs in by
the box's link, runs `ctl forge-setup 1` and does the person's first act
with Forgejo's CLI. Its `ground` check judges the forge connected,
`work/hello` adopted into `hello` and the starter workflow's first run green
on Forgejo's Actions, from ctl's documents and the forge's API. The class is
`temper-live-0a`, 2× three calibration passes; it was judged pass on three
seeds, free (2026-10-24).

[live-0b/](live-0b/README.md) is rung 0b: 0a with the lab's held ChatGPT
record, `chatgpt`, moved in by `ctl import`, one chat turn whose reply the
`chat` check reads, and the record moved out by the release's `ctl export`
for the runner to give back. Its meter reads temper's spend. temper's run
cap, the charter's 280 units, is the stop while a turn runs. It was rehearsed
without a model on the real stack (a fake LLM, a junk held record); its paid
runs wait on the owner's go and a person's sign-in (2026-10-24).

[welcome-chatgpt/](welcome-chatgpt/README.md) walks the welcome’s ChatGPT
sign-in on the ordinary fake box, then checks a first conversation and
reload through hand and ctl. It has two processes and no driver.

[welcome-github/](welcome-github/README.md) walks GitHub one-click through
its create/install/return pages and adopts `fake-person/widget`.
[welcome-forgejo-here/](welcome-forgejo-here/README.md) sets up a real local
Forgejo and its first repository from staged pinned cache binaries.
Both paste the box's fake model key in the browser, require its embedded
Ready card, and follow “Start a chat” into the adopted project before
checking the first chat's reply and reload. Each welcome step
is a separate action, and each experiment has two processes and no driver.

# The look experiment

`look/` holds temper's pages to the mockups (`docs/design/draft/qa-web.md`,
sections 3 to 5): the gallery's states captured beside the mockups' at
1440×900 and 390×844, and checked. It has no driver and no process: a
seed action extracts the gallery printout, the collection is skein's
capture, and the checks are skein's look checks (skein's
`docs/design/lab-browser.md`, 4, 5 and 5.4). `look-planted/` is the same
with a fault (qa-web.md, 5.4).

## Its files

- `experiment.json`: `name` `look`, cost class `look`, one directory
  `page`, no processes, no driver, no faults;
  - the action `stage`: `/bin/sh {experiment}/stage {artifact.gallery}
    {dir.page}/gallery {experiment}`;
  - the collection `capture`: `{runner}/look/capture` with the settings'
    browser by its templates, `--mockups
    {source}/docs/design/client/mockups`, `--page {dir.page}/gallery`,
    `--out {collect.dir}/capture` and every scene;
  - the checks `look-structure`, `look-style` and `look-words`:
    `{runner}/look/check structure|style|words --capture
    {collected.capture}/capture` and every scene.
- `stage`: extracts the subject's tar into the page directory and requires
  exactly one `bundle/*/temper.css` (the bundle directory is named by its
  16-digit hex digest), failing loudly if zero or several match. When the
  experiment holds `planted.css`, it appends it to that stylesheet, the
  last word in the cascade for the pages' look.
- `scenes/<scene>.json`: `look-scene/1` files (below).
- **The planted twin,** `look-planted/`: `look/`'s files byte for byte,
  but `experiment.json`, which differs by `planted` alone, and
  `planted.css`. So with the stylesheet removed and `planted` cleared it
  is `look/`, tree and digest. The planted stylesheet changes one token
  and moves the chat's composer above its conversation. Its token is
  one whose regions agree with the mockup on the healthy page at both
  widths, so its signatures are new: `planted` lists
  `check/look-style/chat/<width>/style/<region>/<property>` and
  `check/look-structure/chat/<width>/order/composer` at each width.
  A third fault is the words': on the task's page alone (a rule under
  `:has(.temper-task-heading)`), the banner's connection line, "Live",
  is hidden, a line that is neither a control's nor a heading's name,
  so `look-structure` cannot see it and the healthy banner agrees with
  the mockup there: `planted` lists
  `check/look-words/task/desktop/words/banner/missing/Live` (at phone
  the line is behind the Menu on both sides). It draws nothing on the
  chat, so the blind judge's pictures, and its run, stand.

## The subject

`lab subject --repository TEMPER --commit C --exclude lab --artifact
gallery=<gallery printout tar>`. The artifact `gallery` is the gallery's
printout at C: `look-judge` builds the release gallery, runs it into fresh
scratch space, and packs the output as one deterministic, uncompressed
USTAR tar. Names are sorted, relative and slash-separated; only regular
files and directories are allowed. Times and numeric owners are zero, owner
names empty, file modes 0644 and directory modes 0755. Names outside USTAR's
limits are refused. Thus every page's and stylesheet's bytes are in the
subject's digest: the same commit C gives the same key. The mockups come from
the subject's source (`docs/design/client/mockups/`), so a changed
mockup is a new subject and key (qa-web.md, 5.3); the scenes are the
experiment's, so a changed scene is a new experiment and key. The
gallery printout is the subject's artifact; the executable is used only
to produce it before the subject is made.

## The scenes

Each at `desktop` (1440×900) and `phone` (390×844, mobile), each side
ready by its `main`. The page is the gallery's state by its name
(`docs/design/client/browser.md`, 7.2), whose names
`tools/gallery/names.txt` keeps.

| Scene | Mockup | Page | Regions beyond banner, navigation and main |
|---|---|---|---|
| `chats` | `/chats.html?world=solo&bare` | `/chats-solo.html` | the chats' list, the composer |
| `chat` | `/chat.html?world=solo&task=T10&bare` | `/chat-solo-T10.html` | the chatbar, the conversation, the proposal, the composer |
| `inbox` | `/inbox.html?world=solo&bare` | `/inbox-solo.html` | the first entry, the "Needs you" and "Updates" lists |
| `task` | `/task.html?world=solo&task=T3&bare` | `/task-solo-T3.html` | the task's heading, the plan, the budget's card |
| `accounts` | `/accounts.html?world=solo&bare` | `/accounts-solo.html` | the first account's card |
| `first-run` | `/chats.html?world=first-run&step=1&bare` | `/chats-first-run-1.html` | the suggested requests, the composer |
| `connect-model` | `/connect-model.html?world=reference&bare` | `/connect-model-choose.html` | Choose and its offered ways, inside main |
| `adoption` | none | `/adoption.html` | the form, its first field, its submit button |

The `connect-model` scene compares Choose. Its owner holder buttons and
configured Anthropic ways are departures citing client README 5.5,
ConnectModel, Choose and The ways; its named cards and Steps list cite
5.2, A flow's parts. The mockup's sign-in navigation is a request button
in the client (5.5, Its address). The gallery's other `connect-model-*`
pages hold Details, Signing in, Ready and the sign-in's endings. Their
mockup links preserve the way (`chatgpt`, `key`, or `program`); they are
comparisons to that way's drawing, not a claim that the mockup plays every
request outcome. `program` opens the existing subscription sketch at Details.

Those states have three further documented differences, outside this
Choose capture: Signing in keeps an external link until tab navigation
and the listener's return exist (client README 5.5, ConnectModel, Signing
in); every chosen way, including Ready and endings, has “‹ All ways”
(5.5, Its address); and no distinct ineligible, port-taken or provider
plan-declined ending is invented without a jig fact (README 11, “A
sign-in's endings jig gives no fact for”). Consent's `declined`, renewed
and refused-as-held have jig facts and their own gallery samples. The
lost key sample retains the former account request alert's meaning.

- **Regions** are found on temper's side by role and name, as a person
  finds them, and on the mockup's by role and name or, where its part
  has no role of its own, by selector. The view's names: the navigation
  `navigation "Main navigation"`; the chats' list by its heading; the
  chatbar `region "T10 · TITLE"`; the proposal `listitem "Proposal from
  T10"` (an `article` of that name in the mockup); the inbox's entries
  `listitem "KIND from TN"` and its lists by their headings. The
  mockup's side keeps a selector where its part has no role or name of
  its own (its chatbar, its inbox entries and sections), and for the
  navigation, which the phone mockups hide. The account card and the
  adoption form have no name yet: their regions stand on a named part
  inside them (the account's heading, the form's field and button).
- **Names** are the frame's and the controls' words (the navigation's
  links, Sign out, the composers' Send, a card's acts, the lists'
  headings), never content.
- **Words** (skein's `lab-browser.md`, 4 and 5.4): the regions whose
  every word is held to the mockup's, a `words` entry a region and
  width. The gallery uses the mockups' words where temper's model can
  hold them (qa-web.md, 3.2), so a word difference is a finding
  (`world`), a departure citing the section that means the page's way,
  or a question. They are the frame and the parts whose text the model
  holds whole: the banner in each scene with a mockup, at both widths;
  the first run's suggested requests; the account's heading. A region
  whose part the tree leaves out (an ignored element the mockup's
  selector finds, such as its chatbar) records no words, so it has
  none. Content a run draws (the conversation, the lists' entries, the
  proposal's fields) stays with the structure and the eye.
- **The data rule:** a region's `data` lists its patterns for what a
  run or the model draws rather than the design (a count, a time, a
  title), declared in the scene and cut to `#` on both sides, never
  inferred from what a page lacks; a datum's span never holds a space,
  so a word beside it still signs, and a pattern that matches nothing
  shows on the check's stderr. A region whose words the model holds as
  the mockup's needs none.
- **A disclosure's words** (the phone's Menu, the chat's model
  chooser) are out of the tree while it is closed, and judged by eye
  until the capture can press it open (skein's `lab-browser.md`, 5.4).
- **Expectations and departures** are the judge's (below). Expectations
  are few, since the base is the mockup as captured; `adoption`, with
  no mockup, is held to them alone. Each departure cites the section of
  the design text that says the page's way is meant.
- **Standing departures:** with the network closed, the mockups'
  Google Fonts stylesheet is refused and their families fall back to
  the lab's DejaVu, while temper draws its bundled faces; each such
  difference in `faces` or `font-family`, and the mockups' refused
  request in their trouble, is a departure citing qa-web.md, 11, until
  the mockups' fonts sit beside them. Nothing in `client/mockups/`
  changes.

## Judging

The judge (qa-web.md, 5.2) is the lab owner's step of a campaign, by
hand and outside the run: Claude reads each scene's three views at both
widths beside the design text of its page (`client/README.md`, the
page's section; `client/browser.md`, 2.5 and 7; `client/mockups/README.md`;
the gap tables its owners give). A sub-agent may draft a scene's
judgement; the judge checks it against the pictures and the text. Each
difference gets a tag (`layout`, `navigation`, `content`, `naming`,
`style`, `behaviour`) and a place:

- **the mockup's way:** temper's page is wrong, a finding (qa-web.md,
  9), classed `world` (words, order, a missing part or name) or `look`
  (appearance);
- **the page's way:** the mockup is behind. Departures at the
  difference's width, by signature or by outline entry, each citing the
  section that says the page's way is meant, and a note for the user.
  Without such a section the difference is a finding or a question,
  never a departure;
- **the text silent:** a question for the user, with its options and a
  recommendation. Its signatures go on failing, undecided.

### When the judge writes an expectation

The base is the mockup as captured, so a difference the mockup's way
already fails, and needs none. The judge writes one only where the base
is not the value meant:
- **a scene without a mockup** (`adoption`): its first judged capture's
  outline entries and the regions' values the judge holds right, so
  that it is held to itself (qa-web.md, 3.1);
- **a mockup's value not drawn as meant,** bound to its invented
  content or its refused fonts, where the text gives the value meant:
  the page is held to that rather than to the captured mockup;
- **a defect the pictures show that no signature names:** a region's
  value or an outline entry that fails on it, so that it signs.

### The judgements

`~/src/rust/tmp/lab/judgements/look/<scene>.json`, beside the lab's
keys: one file a scene, written by the judge, replaced when the scene is
judged again, and never read by the runner. `look-judgements/1`,
canonical JSON (skein's `lab.md`, 2.2), shown spread out:

```json
{"format": "look-judgements/1",
 "scene": "chat",
 "widths": [{"name": "desktop",
             "pictures": [{"digest": "sha256:…", "seen": "judged"}]},
            {"name": "phone",
             "pictures": [{"digest": "sha256:…", "seen": "judged"},
                          {"digest": "sha256:…",
                           "seen": "6 px, largest channel delta 3: corners"}]}],
 "differences": [
   {"id": "composer-stack", "width": "desktop",
    "signatures": ["style/composer/font-family"],
    "tag": "style", "place": "mockup",
    "cites": "client/mockups/README.md, Where each value comes from",
    "seen": "temper's monospace stack adds Liberation Mono",
    "finding": {"class": "look", "title": "…", "owner": "o/web-look"}},
   {"id": "fonts-phone", "width": "phone",
    "signatures": ["style/banner/faces"],
    "tag": "style", "place": "page", "cites": "qa-web.md, 11",
    "seen": "…", "departures": ["google-fonts-banner-faces-phone"]},
   {"id": "goal-deadline", "width": "desktop", "signatures": ["…"],
    "tag": "content", "place": "silent",
    "cites": "client/README.md, 11, A goal's page against its mockup",
    "seen": "…",
    "question": {"options": ["…", "…"], "recommendation": "…"}}]}
```

- **`pictures`:** the picture digests the width's judgement stands for.
  A width's picture digest is `sha256:` and the hex of the canonical
  JSON list of its shots' digests as the capture's record names them:
  the mockup's viewport (null without a mockup), the page's, then each
  region's mockup crop and page crop in the scene's order, null for one
  not taken. A list, since Chromium's raster is not exact from one run
  to the next: the antialiasing of a rounded corner may differ by a few
  levels, so one state may draw under several digests. The judge adds a
  digest once it has checked that nothing in the pictures moved, and its
  `seen` records the check: the pixels that differ and the largest
  channel delta. A scene whose pictures moved is judged again; one whose
  pictures stand is not (qa-web.md, 5.2).
- **A difference:** `id`, a name unique in the file; `width`; its
  `signatures`, the tails after `<scene>/<width>/` as the checks print
  them, each in at most one difference of its width; `tag`; `place`,
  `mockup`, `page` or `silent`; `cites`, the text it is judged by;
  `seen`, what the pictures show, in a line. Then by place: `mockup`, a
  `finding` (`class`, `title` and `owner`, the page's owner), classed
  `world` (words, order, a missing part or name), `look` (appearance) or
  `experiment` (the scene's or the gallery's, qa-web.md, 9);
  `page`, its `departures` and `expectations`, the scene's ids,
  at least one; `silent`, a `question`, at least two `options` and a
  `recommendation`.
- **Findings and questions are named** `<scene>/<id>`. The findings go
  to the lab owner's parent in batches as they are made, grouped by the
  page's owner, and it routes them through the director (qa-web.md, 9):
  a finding is filed once routed, and the lab's candidates of its
  signatures stay as they are. The questions and the notes go to the
  user in one batch at the end of the judging. `python3 -I lab/look_bar.py batch` prints the three
  lists from the files, each with its scene, width, signatures, the
  pair's paths (the page's own pictures for a scene without a mockup)
  and its citation.

A `mockup` or `silent` difference may also list the `departures` and
`expectations` written for the part of its line judged the page's way:
the line keeps failing by its other part, and is judged by its finding
or question.

A `mockup` difference may list no signature: a finding no look line signs,
seen in the pictures or the page's tree (words the look doesn't compare,
or a role or name a scene's finder holds), whose test belongs to the
page's own tiers (qa-web.md, 9). It judges no line.

### The bar

`lab/look_bar.py`, beside `look_pack.py`, reads a key's records in the
lab and the judgements, and prints the key's bar; `look-judge` prints it
after each key's verdict.
`look_bar.py bar` exits 0 when the bar holds, 1 when it does not, 2 on
input it cannot read.

- **Each scene's verdict on each judged run:** `pass`, or `fail` and its
  signatures' count, and whether the runs' signatures are the same.
- **With `planted`** (the planted twin), given the healthy key: each
  judged run's planted signatures, those present of all; and its
  signatures beside the healthy key's run in the same place of its three
  (the look draws nothing from a seed), as two sets, planted − healthy
  and healthy − planted. The judge confirms that each line of the first
  is the token's reach or the composer's move, and that the second is
  empty or explained. The first holds more than the planted six: the
  token also draws the banner's bottom border in every other scene, the
  chatbar's, and the cards' borders of the chats, the inbox and the
  task, and the moved composer moves the chatbar and the conversation
  too (at temper e35fcec58, 53: 46 border colours and 7 lines of the
  composer's move). Its pictures
  differ by design, so it has no unjudged list: the blind judge (below)
  judges its chat.
- **Without `planted`, the unjudged list.** A judged run's failing
  signature is judged when it is a look check's, `<scene>/<width>/<tail>`,
  its scene's judgements hold that width with the run's picture digest,
  and a difference of that width names `<tail>` as a finding or a
  question. Every other is listed once, with the runs it failed in and
  why: `departure-gone`, always; `not a look check` (the runner's own);
  `not judged` (no judgements for the scene or the width); `pictures
  moved`; `no difference`; `still failing` (judged the page's way, its
  departure or expectation not in the scene). The `undecided` lines are
  listed too: a look check prints one, and the run records it, for a
  part it cannot judge (a record's file missing or changed, a call it
  needs refused, a region's mockup part not found: skein's
  `lab-browser.md`, 5.4). A check that prints both exits 1, and the runner
  records its failures and its undecided alike (skein's
  `scripts/lab/runner/checks.py`, `command`): the bar lists both, and
  neither hides the other. `unjudged: none` when nothing is.

### The planted chat's blind judge

The judge must name both planted differences (qa-web.md, 5.4) without
being told them:
- **What it sees:** copies of a planted run's chat pictures, the pair
  and the regions' pairs at both widths, in a fresh directory outside
  `~/src/rust` with no `CLAUDE.md` on the path above it, under neutral
  names (the width and the region): no path it sees holds `planted`, a
  key, the experiment's name or a run's id. It sees neither
  `planted.css`, nor the planted signatures, nor the healthy run's
  chat.
- **How it runs:** `claude -p --restricted --tools Read`, a fresh
  session with auto memory off, from that directory. Its prompt lists
  the files and asks it to compare each mockup with temper's page and
  to name every difference it sees; it says nothing of a fault.
- **What is kept,** in `judgements/look/blind/`: the prompt verbatim,
  its words, and its transcript (`--output-format stream-json`).
- **Shown blind by a command:** `python3 -I lab/look_bar.py blind --dir
  DIR --transcript FILE --forbid WORD...` fails on a file under the
  directory whose path holds a forbidden word, a tool other than Read
  in the transcript, or a path the transcript names outside the
  directory, and prints the paths it read.
- **The bar holds** when its words name the changed token and the
  composer's move.

## Running it

```sh
lab/look-judge SKEIN EXPERIMENT...
```

From a clean temper checkout at its commit C: builds the gallery
through `heavy` (release), prints and packs its output in temporary space
removed after making the subject, and runs skein's `lab
judge` on each experiment named (`lab/look`, `lab/look-planted`):
three seeds a key (skein's `lab.md`, section 8), printing each key and
verdict, then the key's bar (above). More runs of a seed are `lab run
--seed N`. The lab is `~/src/rust/tmp/lab`, whose `settings.json` names
the browser and the `look` cost class. It runs on demand, never in the
gate; its shots are evidence, never goldens. Given both experiments,
the planted key's bar is beside the healthy key's. `python3 -I
lab/look_bar.py bar --key KEY [--healthy KEY] EXPERIMENT` prints a
key's bar again without a run.

## Its tests

`lab/tests/unit/test_look.py`, in the gate, fast and with no browser:
each scene canonical `look-scene/1` with the members above, its page a
name of `tools/gallery/names.txt`, its mockup's file in
`client/mockups/`, its `words` entries naming its regions, none in a
scene without a mockup; `experiment.json` naming every scene in its
capture and the three checks; and `look-planted/` byte for byte `look/` but for
`planted.css` and `experiment.json`'s `planted`. `test_look_pack.py` checks
identical tar bytes and digests despite changed disk times and modes,
fixed archive metadata, and refusal of links, special files and names
outside USTAR's limits. `test_look_bar.py` checks the bar on records and
judgements written as fixtures: the picture digest; a signature judged
by a finding and by a question; each reason of the unjudged list; a
scene's verdicts the same on three runs and not; the planted signatures
counted, and the planted and healthy sets' two differences; a
judgements file refused for each rule above; the batch; and the blind
check on a transcript that stays in its directory and on ones that
leave it, use another tool or see a forbidden name.
The checks' own decisions are skein's, tested there on capture
fixtures.

# The cluster in the browser

`cluster-web/` is `cluster/`'s cluster with temper's browser bundle
served and a person's browser beside it, the hand
(`docs/design/draft/qa-web.md`, section 6; skein's
`docs/design/lab-browser.md`, section 8): the browser signed in as the
deployment's owner, and a scripted story, the smoke, driving it
through `browse`. It has no driver: it is the free smoke W2's later
experiments stand on, and it costs no model call.

## Its files

- **`cluster/`'s, copied:** `llm.json`, `chat.txt`, `agent.json`,
  `w1.*`, `w2.*` and `forge-1.*` byte for byte; `engine.json` but for
  `application.clients.bundle`, `"{proc.engine.dir}/bundle"` where
  `cluster/`'s is null. `experiment.json`'s processes `llm`, `engine`,
  `w1` and `w2` are `cluster/`'s member for member, but for `engine`'s
  `files`, which add the bundle's tree, `{"path": "bundle", "tree":
  "bundle"}` (skein's `lab.md`, 4.3), and `llm`'s `after`: `cluster/`'s
  `forge` member, which only the process tier's case stories reach
  (`docs/design/domain/testing.md`, 2.6), is not the browser's, so its
  `llm` starts first. Copies, not names: an
  experiment's digest is its own directory's tree (skein's `lab.md`,
  2.3), so an experiment that named another's files would run with
  bytes its key does not hold, and the runner would need a new
  identity rule; a test holds the copies equal instead (below), and
  `cluster/` and its Rust reader stay as they are, with no Chromium.
- **`browser`,** after `engine`: `{browser.program} hand --chromium
  {browser.chromium} --fonts {browser.fonts} --scratch
  {proc.browser.dir} --out {dir.hand} --origin
  http://{proc.engine.ready.clients} --size 1440x900 --listen
  127.0.0.1:0`, `env` `{}`, `memory_bytes` null, ready by `listening `
  on its stdout, captured as `address`. At a desktop width: at 760 px
  and under, the banner's "Sign out" and the navigation sit in a closed
  disclosure, "Menu".
- **`dirs`:** sorted as `client`, `hand`, `spent`: `client`, the seed's
  `temper ctl` directory; `hand`, the browser's `--out` (its
  `trouble.jsonl` and its shots); `spent`, the directory of the `ctl`
  that presents the spent link.
- **`cost`:** `temper-m1-calibration`, `cluster/`'s (`qa.md`, 10.1): 120 s (60 s before the user's doubling of 10-22, provisional)
  to be ready, 240 s (120 s before the user's doubling of 10-22, provisional) a command,
  20 s (10 s before the user's doubling of 10-22, provisional) to stop.
- **`relink`, `smoke` and `checks/trouble`,** below.

## The seed

Each `{artifact.temper} ctl` action is `qa.md`'s, 5.1. In order:

| Action | Its argv | Its stdin | For |
|---|---|---|---|
| `sign-in` | `{artifact.temper} ctl {dir.client} sign-in -` | `http://{proc.engine.ready.link}` | the owner signs in by serve's printed link, which spends it |
| `add-key` | `{artifact.temper} ctl {dir.client} add-key 1 {proc.llm.dir}/llm.key --deployment --key cluster-web-llm` | | the fake model's key |
| `usable` | `{artifact.temper} ctl {dir.client} wait accounts --until usable --timeout 200` | | the account usable |
| `relink` | `/bin/sh {experiment}/relink {artifact.temper} {dir.spent}` | the spent link again | serve refuses it and prints a fresh one, the action's awaited line: `{"process": "engine", "stream": "stdout", "prefix": "http://", "capture": "link"}` (skein's `lab.md`, 4.5) |
| `browser-sign-in` | `{runner}/hand/sign-in {proc.browser.ready.address}` | `http://{action.relink.link}` | the browser opens the fresh link |
| `signed-in` | `{runner}/hand/browse {proc.browser.ready.address} wait heading Chats --exact --timeout 120` | | the chats page, signed in |
| `smoke` | `/bin/sh {experiment}/smoke {runner}/hand/browse {proc.browser.ready.address}` | | the story (below) |

- **Environments:** `browser-sign-in`, `signed-in` and `smoke` have
  `PATH=/usr/bin:/bin` for the hand launchers' `python3` shebangs; all
  other actions have empty environments.
- **A second `ctl` presents the spent link,** not the browser. The
  browser then signs in once, by the fresh link, so its trouble holds
  nothing the seed caused but its `sign-in` call's, which the entries'
  `during` names and the check leaves out (skein's `lab-browser.md`,
  8.7). A browser shown
  the spent link would show the signed-out page, and what that page
  logs after the call's load comes between calls, which the log cannot
  tell from the pages' own. It is also the product's way to a fresh link
  (`docs/design/client/README.md`, 11, "A fresh link from the client").
  The browser's page for a spent link is a story of its own, for the
  browser tier (`docs/design/client/browser.md`, 2.4).
- **`relink`,** POSIX sh: runs `TEMPER ctl DIRECTORY sign-in -` with
  its own stdin, and exits 0 when that exits 1 with `the sign-in link
  was refused` on its stderr; otherwise it exits 1, saying what it saw.
  The link is never an argument: it reaches `ctl` on stdin alone, and
  the browser through `hand/sign-in`'s.
- **The links in the record:** the first is `engine`'s ready capture,
  the second `relink`'s awaited capture, both in `run.json`, and both
  are spent before the smoke starts. What shows a run (a report, a
  finding's evidence) shows each query as `<link>` (skein's
  `lab-browser.md`, 8.6).

## The smoke

`smoke BROWSE ADDRESS`, POSIX sh: each step one `browse` call, its
output printed, and the story ends at the first that does not exit 0,
with its status, which fails the action (`action/smoke/exit:1`, skein's
`lab.md`, 7.1). Each expectation is a `wait`, so a missed one exits 1
(lab-browser.md, 8.8), 120 seconds (60 seconds before the user's doubling of 10-22, provisional) at most
but the first reply's, 180 seconds (90 seconds before the user's doubling of 10-22, provisional). The roles are the fake
person's; the names are o/web-look's words for the pages:

1. `type "text area" "Start a chat"`, its text `cluster-first-chat`,
   the fake model's cue (`cluster/llm.json`); `press button Start
   --exact`.
2. `wait log Conversation`, the chat's page; `wait text "First worker
   replied." --in log Conversation --timeout 180`: its reply, once its
   turn commits. A run is shown by its activity, and its words as each
   turn commits (`docs/design/protocol/llm.md`, 7; an agent process
   gives no provisional text, `client/tools.md`). The cue's turn writes
   the reply, then calls `wait`, which ends at the charter's
   `waiting_ms`, 120 s (`cluster/engine.json`): the reply shows 120 s
   after Start, most of a run's time, and the wait's 180 s (90 s before the user's doubling of 10-22, provisional) leave it a
   margin. A shorter fixture or `waiting_ms` would only hide it.
3. `address`: the chat's, `/chats/<task>` on the origin, kept.
4. `go /chats`, `wait heading Chats --exact`; `go` the chat's path,
   `wait text "First worker replied." --in log Conversation`: its page
   by its address.
5. `reload`, the same wait; `shot`.
6. `back`, `wait heading Chats --exact`; `address`, which must be
   `/chats` on the origin, or the story exits 1.

## The browser's trouble

The check `trouble` (`qa-web.md`, 8.1): `/usr/bin/python3 -I -B -S
{experiment}/checks/trouble {collected.hand}/trouble.jsonl`, after the
live collection `hand`, `/bin/cp -R -- {dir.hand}/. {collect.dir}/.`.

- **Each entry fails `browser/<kind>/<page>`:** its kind as the hand
  logs it (an exception, a console error, a policy violation, a dialog,
  a request off the origin: skein's `lab-browser.md`, 8.7), and its
  page by its address's route, by temper's address codec
  (`docs/design/client/browser.md`, 2.3): `inbox`, `chats`, `accounts`,
  `chat`, `task`, `adopt`; `sign-in` for the link's route; `blank` for
  `about:blank`; `other` for any other path, and `off-origin` for an
  address off serve's origin. Never a task's or a project's number.
- **One line a signature,** its summary the entries' count and the first
  one's text, as the hand redacted it.
- **Left out:** the entries whose `during` is `sign-in`, the seed's
  calls.
- **Statuses** (skein's `lab.md`, 7.2): 0 with no entry left; 1 with
  one, an unreadable line's `undecided` printed beside it; 2, undecided,
  when the log is missing, or a line is unreadable and no entry is left.
- **The codec's table is copied** into the check, which names its
  source: `docs/design/client/browser.md`, 2.3, and the codec,
  `crates/temper-client-domain/src/address.rs` (`Address`, its `path`
  and its parse), each page by its variant's name in lower case. A page
  the codec gains names itself `other` until the table follows it.

## The subject and the command

```sh
lab/cluster-web-judge SKEIN
```

From a clean temper checkout at its commit C, as `look-judge` (above):
builds `temper` (release) and the browser bundle
(`scripts/bundle.sh --release`) through `heavy`, packs the bundle's
directory as `look_pack.py` packs the gallery, makes the subject
(`lab subject --repository TEMPER --commit C --exclude lab --artifact
temper=… --artifact bundle=…`), validates `cluster-web`, and runs
skein's `lab judge` on it: three seeds, printing the key and the
verdict. The lab's `settings.json` names the browser and the cost
class. It runs on demand, never in the gate; its transcripts are
evidence, never goldens.

## Its tests

- **`lab/tests/unit/test_cluster_web.py`,** in the gate, pure: the
  copies equal `cluster/`'s, `engine.json` but for its bundle's line,
  and the four processes but for `engine`'s tree; `browser` and the
  seed as above; the check's decisions: the page of each route and of
  the others, the seed's sign-in left out, one line a signature, the
  statuses, an unreadable line.
- **`lab/tests/process/test_cluster_web.py`,** on demand: `relink` and
  `smoke` under the real shell against stand-ins for `temper ctl` and
  `browse`: the refusal taken, every other exit refused, the smoke's
  calls in order, and a missed wait or a wrong address ending it with
  1.

# The live scenes

`look-live/` takes the look's scenes for what the gallery cannot show
(`docs/design/draft/qa-web.md`, 3.3) on the hand's page in
`cluster-web/`'s cluster: focus after a send, the page after a reload,
a dialog in the top layer. Each is captured beside its mockup as the
look's scenes are, and checked by the look's two checks; and the
gallery's outline of each such page is held to the live one (qa-web.md,
8.2). skein's `docs/design/lab-browser.md` designs the capture's live
side (5.1) and the hand's look reads (8.10). It has no driver: the
seed makes each state through `browse`, and the judge is the lab
owner, as for `look/`. No scene expects a run's live text, which
temper's agent processes do not give (`docs/design/protocol/llm.md`,
7).

## Its files

- **`cluster-web/`'s, copied** byte for byte, as `cluster-web/`
  copies `cluster/`'s (above), but two:
  - **`llm.json`** adds one cue, `look-live-chat`: a reply and `wait`,
    a second reply and `wait`, then final text when Close asks the Open
    run to finish. The person's message wakes the first wait and the
    first reply commits; the second turn then waits uncommitted, a quiet
    page whose composer stays. `cluster-first-chat`'s second turn is
    already its final text.
  - **`engine.json`'s chat charter** waits 300 s (`waiting_ms`), its
    time and the policy's bounds widened to hold it, as temper's
    startup checks require: the quiet window, about ten times the
    30 s its steps and captures take. No turn commits inside it, so
    the chatbar holds still (its status "waiting on your words", its
    spend). When it closes, the second reply commits, the spend moves,
    and the chat closes under its existing grace, taking its composer: each live
    scene is ready by the composer (`textbox "Message"`; the dialog in
    `chat-closing`), so a capture after the window is `missed`, never
    a wrong picture.
  `experiment.json`'s processes are `cluster-web/`'s member for
  member; the browser is at 1440×900, and the capture changes its
  viewport for each width (lab-browser.md, 8.10).
- **`stage`,** `look/`'s byte for byte: the gallery printout extracted
  into `{dir.page}/gallery`.
- **`steps`,** POSIX sh, the seed's `browse` steps (below), one phase an
  invocation, each call's output printed, ending at the first that
  does not exit 0, with its status.
- **`scenes/`:** the live scenes, `look-scene/1` with a live page
  (lab-browser.md, section 4). **`gallery/`:** the gallery's states
  that stand for their pages, served scenes with no mockup, captured
  alone for the gallery's check.
- **`checks/gallery`:** the gallery's check (below).
- **`dirs`:** `cluster-web/`'s `client`, `hand` and `spent`; `look`,
  each live capture's `--out` under it by its scene's name; `page`, the
  gallery staged.
- **`cost`:** `temper-m1-calibration`, the cluster's: 120 s a command
  holds a capture of one scene at two widths.

## The scenes

Each at `desktop` (1440×900) and `phone` (390×844, mobile), on the
chat page `/chats/1`, the cluster's first task. Its regions are the
frame's, none of the conversation's, whose turns carry their times:
the banner, the navigation, the chatbar and the composer, as `look/`'s
`chat` finds them (the composer's mockup, T6's), and in `chat-closing`
the dialog alone, `dialog "Close"`: shown modal, it leaves the page
behind it inert, out of the accessibility tree.
The names are `look/`'s `chat`'s, but the proposal's words (Accept,
Reject with reason, Accept goal), which a chat at rest has none of.

| Scene | Mockup | State | Focus |
|---|---|---|---|
| `chat-sent` | `/chat.html?world=solo&task=T6&bare` | a message sent from the chat's composer by Enter, the first reply in | `textbox "Message"` |
| `chat-reloaded` | the same | `chat-sent`'s page reloaded, the first reply in | none: a reload is a fresh document, not "Going to another page" (skein's `ui-web.md`, 7), so its focus is the platform's, the document's |
| `chat-closing` | none | Close pressed: its confirmation, a dialog in the top layer | the dialog's first button (shown modal: skein's `ui-web.md`, 7) |

- **The mockup is the solo world's chat at rest** (`chat.html?task=T6`,
  "A chat at rest, and its delegate's card"): the nearest to the live
  chat, whose content differs (its delegate's card, its turns), which
  the judge accepts as departures citing qa-web.md, 2 ("real data
  only").
- **`chat-closing` has no mockup:** no mockup draws a confirmation, so
  its first judged capture is its expectations (qa-web.md, 3.1).
- **The phone's width is the same page made narrow,** its state made
  at the desktop's: the banner's navigation and Sign out then sit in
  the closed "Menu" (`docs/design/client/README.md`), which no scene
  opens.

## The seed

`cluster-web/`'s actions through `signed-in`, then, each `steps`
phase `/bin/sh {experiment}/steps PHASE {runner}/hand/browse
{proc.browser.ready.address}`:

| Action | What | For |
|---|---|---|
| `drain` | `browse trouble` | the sign-in's trouble read, so each capture's is its own scene's (lab-browser.md, 5.1) |
| `stage` | `look/`'s stage of the gallery into `{dir.page}/gallery` | the gallery's scenes |
| `send` | `steps send`: `type "text area" "Start a chat"` (`look-live-chat`), `press button Start --exact`, `wait log Conversation`; `type "text area" Message` (`look-live-sent`), `key Enter`, `wait text "First reply." --in log Conversation` | `chat-sent`'s state |
| `capture-sent` | the live capture of `scenes/chat-sent.json`, `--out {dir.look}/chat-sent` | |
| `reload` | `steps reload`: `reload`, the same wait | `chat-reloaded`'s |
| `capture-reloaded` | the live capture of `scenes/chat-reloaded.json` | |
| `close` | `steps close`: `press button Close --exact`, `wait dialog Close --exact` | `chat-closing`'s |
| `capture-closing` | the live capture of `scenes/chat-closing.json` | |

A live capture is `{runner}/look/capture` with the settings' browser by
its templates, `--mockups {source}/docs/design/client/mockups --live
{proc.browser.ready.address} --live-out {dir.hand} --ready-seconds 40`.
The three captures and their steps take about 30 s, inside the second
turn's quiet five minutes.

## The checks

- **The collections:** `look`, `{dir.look}` copied; `hand`, as
  `cluster-web/`'s; `gallery`, the served capture of `gallery/`'s
  scenes, `--page {dir.page}/gallery`, no mockups.
- **`look-structure` and `look-style`** over the three live scenes,
  one `--capture` each (lab-browser.md, 5.4): the outline, readiness,
  trouble and order, the focus (`focus/page`), and the regions'
  values, as `look/`'s.
- **`gallery`** (qa-web.md, 8.2): `/usr/bin/python3 -I -B -S
  {experiment}/checks/gallery --live {collected.look} --gallery
  {collected.gallery}/capture --scenes {experiment}/scenes
  chat-sent=chat chat-reloaded=chat chat-closing=chat-closing:modal`,
  each pair a live scene and the gallery
  scene of `gallery/` that stands for its page: `chat`,
  `/chat-solo-T6.html`, the chat at rest; `chat-closing`,
  `/chat-closing.html`, its Close pressed. A gallery scene's names are
  its live scene's.
  - **What agrees: the frame, line for line.** At each width, each
    page side's outline is made from its tree as lab-browser.md 5.3
    makes it, with the live scene's names, but that a name is its
    words before a trailing count (a space, then digits alone): the
    gallery's "Inbox 2" and the live "Inbox" are both `link "Inbox"`,
    while a renamed link still fails. The outlines are cut to the frame:
    the `banner`'s subtree, the chatbar's (`main`'s first `region`),
    each `dialog`'s, and the composer's lines (`textbox "Message"`,
    `button "Send"`) wherever they sit under `main`. A pair declared
    modal (`chat-closing=chat-closing:modal`) is cut to its dialog's
    lines alone, on both sides: a modal leaves the page behind it inert,
    out of the live tree, and skein's outline prints no `dialog` line,
    so the lines outside every landmark are the dialog's. A page that
    lost its banner fails every other pair. Every line there
    is compared as the outline prints it, with its landmarks
    (`banner > navigation "Main navigation" > link "Chats"`), counted;
    the outline prints only the scene's names, so a title, a project's
    name or a count of the page's data is never compared. The banner's
    unnamed buttons are its projects, the person's data as the
    conversation is: they count as one, whatever their number, so two
    projects agree with one, and a banner without any still fails. Everything
    else under `main`, the conversation whole (its turns, folded
    calls, delegates' cards), is what only a live page has, and is left
    out, whatever its names.
  - **Each disagreement fails** `<scene>/<width>/gallery/missing/<role>[/<name>]`
    (fewer on the live page than in the gallery) or
    `.../gallery/extra/...` (more), its role and name signed as
    skein's look checks sign them; its summary names both counts. A
    disagreement is the gallery's finding, classed `experiment`
    (qa-web.md, 9), unless the judge finds the live page wrong.
  - **Statuses** (skein's `lab.md`, 7.2): 0 agreeing, 1 with a line,
    2 when a record or an outline is missing or unreadable, or a pair
    names a scene no record holds.

## Judging and the bar

As `look/`'s (above), its judgements in
`~/src/rust/tmp/lab/judgements/look-live/`, one file a live scene, by
the same format, and its bar by `look_bar.py`. A live width's picture
digest stands for its regions' crops alone, its viewports' shots
listed as null: the live page's viewport shows its turns' times and
the day, which move with the clock, so its digest moves with every
run. A width is live when its record's page side holds `focus`
(lab-browser.md, 5.2), which no served side has: `look/`'s digests and
bar stay as they are. The judge still reads the pair. The planted
twin and the blind judge are `look/`'s alone.

## Running it

```sh
lab/look-live-judge SKEIN
```

From a clean temper checkout at its commit C: builds `temper`, the
bundle and the gallery (release) through `heavy`, packs the bundle
and the gallery's printout as `look_pack.py` packs them, makes the
subject (`--artifact temper=… --artifact bundle=… --artifact
gallery=…`), validates `look-live`, runs skein's `lab judge` on it
(three seeds), and prints the key, the verdict and the bar. It runs on
demand, never in the gate.

## Its tests

- **`lab/tests/unit/test_look_live.py`,** in the gate, pure: the
  copies equal `cluster-web/`'s and `look/`'s (`llm.json` but for its
  cue), the processes `cluster-web/`'s; each scene canonical, live,
  its names `look/`'s `chat`'s less the proposal's, its mockup's file
  in `client/mockups/`; each gallery scene's page a name of
  `tools/gallery/names.txt` and its names its live scene's; the seed's
  order and the checks' captures; and the gallery check's decisions on
  outlines and trees written as fixtures: agreeing, an entry missing
  and one extra, counts, landmarks apart, the statuses, and the
  conversation left out whole: a content control that differs between
  the sides (a folded call, a delegate's card, one named by a scene's
  name) fails nothing; the projects' number and the Inbox's count
  agree, a banner without projects and a renamed link fail.
- **`lab/tests/unit/test_look_bar.py`:** `look_bar.py` is `look/`'s
  too, so its live rule is tested both ways: a width with `focus` on
  its page side digests its crops alone, and a `look/` record's
  digests and bar are unchanged.
- **`lab/tests/process/test_look_live.py`,** on demand: `steps` under
  the real shell against a stand-in for `browse`: each phase's calls
  in order, and a missed wait ending it with 1.

The provisional lab waits are 200 s (100 s before the user's doubling of 10-22, provisional) for an
account, 120 s (60 s before the user's doubling of 10-22, provisional) for a page, and 40 s
(20 s before the user's doubling of 10-22, provisional) for capture readiness.
