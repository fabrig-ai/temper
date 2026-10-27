# Journeys

Provisional, 2026-10-26, from the user's draft
(`../draft/look-recording.md`), revised the same day to its second
pass. A journey is a walk through the
browser's client (`browser.md`) recorded on a fresh `temper box
--fake-backends`, then replayed natively through the shipped host and
printed as the client's own pages, linked so that the same clicks walk
them. The look is tuned on those pages, and the committed journeys are
kept current by a test. They are not the mockups' journeys
(`mockups/README.md`, 4), which are worlds walked in the mockups.
Section 2 is the host on native, 3 the recording, 4 its replay, 5 the
print, 6 the tests, 7 editing the look, 8 a recording gone stale, 9 the
journeys, 10 what is open.

## 1. In one page

- **temper is the source of truth** for what it draws (the user,
  10-26). A page a journey records looks as its print does:
  `temper.css` over skein's sheet, tuned on the print. The mockups are
  reconciled to it gradually, and stay the design of what is not built
  yet (`mockups/README.md`, 1; `browser.md`, 2.5).
- **One seam, the host's.** The browser shell gives its host each
  report of a turn and carries out what the host asks back. The
  recorder keeps what crosses that seam; the replay gives it back to
  the same host, built natively (section 2).
- **Recorded in a browser,** in a bundle built for it alone, on a fresh
  fake box: the facts the host starts from, then turn by turn the
  clocks, each report and what the host asked after it, each act as the
  event the view makes of it, never its control's words or place, and a
  mark for each page the person saw (section 3).
- **Replayed, it must ask what it asked,** and a control on the page
  must give each recorded event. A client that asks otherwise would be
  answered what it never asked: the journey is stale, and is recorded
  again (sections 4 and 8). The view may rename, restyle or move a
  control and the journey holds. The first replay proves itself against
  the live page's prints.
- **Recorded again by a command** while its acts hold: `again NAME`
  walks them on a fresh fake box in headless Chromium, as the browser
  tier drives it, and adds the result; no person walks it (8.1).
- **Printed as static pages** by `temper-client-journeys`, under
  `docs/design/client/journeys/`: a page per mark, one tag per line, the
  acted control leading to the next page, the shell's stylesheets
  linked from the checkout, then an empty `overlay.css`; beside them the
  journey's `README.md`, its description. No browser, no box, the same
  bytes every time (section 5).
- **Kept current by a test** that reprints every committed journey and
  fails while a byte differs, naming the command that reprints, so a
  landing that changes a recorded page carries its new print. It also
  fails while `overlay.css` holds anything (section 6).
- **The look is tuned on the print:** rules in the overlay, markup in
  place, each request a commit on the person's branch `look-requests`,
  kept on `main` by `scripts/look-refresh.sh` and never landed. A
  landing that carries a request's name moves it into the shell's
  stylesheets and the view, and reprints (section 7).
- **Fake only.** A recording keeps what the page was given as it was,
  the fake box's dummies included; none is made against real accounts
  (3.5).

## 2. The host on native

- **A library on every target.** `temper-client-shell-web` is a
  `cdylib` and an `rlib`. Its `host`, `limits` and `records` modules
  build for native as for `wasm32-unknown-unknown`, their dependencies
  no longer under the wasm target; `lib`'s start, the DOM and the
  platform's parts stay wasm-only, as do `wasm-bindgen` and `web-sys`.
  `tests/host.rs` uses the library instead of including the modules by
  path. The replay runs this host, never a copy.
- **The `journey` module** (the file, the recorder, the replay) builds
  natively, for the program and the tests, and in the browser only with
  the `record` feature: the shipped bundle holds none of it.
- **The seam** is `host::Host`'s, and the shell's turn and the replay
  call it alike:
  - `clocks(now, wall)`, at the turn's start, as today;
  - `report(&mut self, report: Report) -> Duties`: one report given,
    and what the host asks after it. `Report` is `Start`, `Due`,
    `Act { node, act }`, `Place { path }`, `Heard(Heard)` (a fetch's
    report, through jig's `adapter::fetched`) and
    `Carried { reports, writes }` (skein's `transport::Carried`).
    `Duties` holds the `asked` (jig's `adapter::Asked`), the `writes`
    to answer, and the address to `navigate` to, to `replace` with, or
    to `show`, with whether it replaces. What `Shell::turn` does
    between a report and the platform today, `duties()` and the
    `navigate`, `replace` and `show` fields, moves behind it;
  - `draw(&mut self, page, ops)`: the turn's patches applied to the
    page, and its links' hrefs set (`browser.md`, 2.3), as the turn's
    end does today;
  - `deadline(&self)`, as today.
- **The page's look** (`Look` with `look::NAMES` and `look::ELEMENTS`,
  the prefix `temper`) is one function of the host module's,
  `page(bounds, primary) -> Page<'static>`, which the shell's start and
  the replay call.
- **An act's event,** `Host::event_of(&self, node, act) ->
  Option<Event>`, the event the view makes of an act on a node, without
  giving it: `temper_client_view::event(&View, node, act)` reads the
  view and changes nothing, so that the recorder and the replay may ask
  it of any node. The host's own act gives what it returns. With the view's tree, `Host::tree(&self) ->
  &Tree`, it is where an act is read and its node found again (3.3).

## 3. The recording

### 3.1 The recorder

- **In a bundle of its own:** `scripts/bundle.sh --loopback --record
  OUT` builds the shell with its `record` feature beside
  `loopback-navigation`; the shipped bundle has neither. The recorder
  adds nothing to the page: no node, no style, no request.
- **What it hears:** in `Shell::turn`, beside the host, the start's
  facts once, then each turn's clocks, each report given to
  `Host::report` with the duties it returned, and the turn's deadline.
  It keeps them in memory as the file's lines (3.2), through
  `journey::Recorder`, which is platform-independent and wraps the
  seam: `Recorder::report(host, page, report) -> Duties` gives the
  report to the host and keeps both, and `Duties::departs()` says when
  the duties leave the page. The shell and the host's tests call it
  alike; the wasm part only reads the keys and saves.
- **Two keys,** read by their `code` on the document before the page
  sees them, so that the page never does: Alt+Shift+M marks the page
  as it is drawn, with no act, for a state the page reaches by itself,
  such as a setup's progress; Alt+Shift+S saves the recording as it is
  then, closed by an end mark of the page as drawn, through the
  browser's download, as `journey.recording`, from a link never added to
  the document. The recorder goes on after a save, so that each save is
  a whole recording and a later one holds more; the person saves once,
  at the walk's end, and `again` saves as it goes (8.1). This departs
  from the draft, whose save ends the recording: what the person does is
  the same. A key's turn that gives the host nothing, a save's, is not
  kept.
- **One load of the page.** A recording starts with the page, at the
  printed link's first page, and is whole at each save. A departure ends
  it: when a report's duties ask to `navigate` or `replace`, the
  recorder keeps them, marks the end as the page is drawn then, and
  saves, before the shell carries the departure out; the rest of that
  turn is not kept, and the recorder keeps nothing more. A reload starts
  a new recording.

### 3.2 The file

Text, one record per line, its kind first, then its fields, each after
one space. A number is decimal and a word bare (a role, an act, `none`);
bytes are in double quotes, with `\\`, `\"`, and `\xHH` for a byte
outside printable ASCII (0x20 to 0x7e). There are no other lines: no
comment, no blank.

| Record | Fields | Where |
|---|---|---|
| `journey` | `2`, the format's version | the first line |
| `seed` | the platform's seed | the start's facts, in this order |
| `clocks` | now and wall, in nanoseconds | |
| `primary` | `control` or `meta` | |
| `ways` | the `temper-sign-in` meta's content, the page's origin | when the page has the meta |
| `forge` | a `temper-forge-sign-in` meta's content | one per meta, in order |
| `path` | the address's path and query | |
| `secret` | the printed link's secret | when the address held one |
| `stored` | a small-store record's key and value | one per record read at start |
| `turn` | now and wall | each turn's clocks |
| `start`, `due`, `place`, `heard`, `carried`, `act` | the report | each report, in its turn's order |
| `exchange`, `open`, `close`, `save`, `erase`, `writes`, `navigate`, `replace`, `show` | what the host asked | after its report, in order |
| `deadline` | nanoseconds, or `none` | each turn's end |
| `mark` | the live page's html | before its act, or at the key, in a turn of its own (`turn`, `mark`, `deadline`) |
| `end` | the live page's html | the last line |

- **The reports:** `place PATH`; `heard answered EXCHANGE STATUS BODY`,
  `heard failed EXCHANGE`, `heard opened`, `heard refused STATUS`,
  `heard piece BYTES`, `heard ended`; `carried WRITES`, then one line
  per carried report, `written KEY`, `unwritten KEY` or `lost
  EXCHANGE`; `act ACT [VALUE] EVENT NAME` (3.3).
- **The asks:** `exchange ID BOUND METHOD PATH BODY`, then its headers
  as `NAME VALUE` pairs on the same line; `open PIECE PATH` and its
  headers; `close`; `save KEY VALUE`; `erase KEY`; `writes N`, when not
  zero; `navigate TO`; `replace TO`; `show PATH push|replace`.
- **A committed recording's marks hold no html:** `add` drops it once
  the first replay has proved itself (4.3).

### 3.3 An act's event

- **The act** is skein-ui's: `press`, `submit`, `toggle`, `input` with
  its value, or `drawing` with its bytes.
- **Its event** is the one the view makes of it, read through
  `Host::event_of` before the act is given: for a press, the client's
  action, such as a `Go` to an address, in the domain's own names. It is
  kept as the domain `Event`'s Debug print, compact, as bytes, which the
  replay compares and nothing parses. Not the control's role, words or
  place, so that the view may rename, restyle or move it and the
  recording holds; a change to the domain's names, or to how Rust prints
  them, stales it (section 8).
- **The control's accessible name follows,** for a reader: the node's
  name, or else its subtree's text, as skein's fake person reads it. It
  is never checked.
- **An act the view makes no event of** is given to the host, which
  does nothing with it, and kept nowhere: no line, no mark.
- The welcome's "Set up temper" is kept as `act press "Act { action:
  Pages(Go { address: ... }) }" "Set up temper"`; a typed secret's event
  shows its length alone (`AccountKey { value: Secret { len: 12 } }`),
  its value being the act's (3.5).

### 3.4 Marks

- **A mark is a page as the person saw it:** as drawn when an act came,
  before the act's turn changes it; as drawn when the mark key was
  pressed; and at the recording's end. Either way it is the page as the
  last turn's end drew it, so the replay takes a mark wherever it sits. Each keeps the live page's
  `Page::html()`, for the first replay's proof.
- **Typing marks nothing.** An `input` act is recorded and replayed but
  marks no page: a printed field takes typing as the live one does
  (5.3), and the next act's mark shows what was typed.
- **The page's own state:** skein's page draws one state that its host
  never hears of, an open menu (`Role::Menu`), and a mark taken with
  one open fails the first replay's proof. temper's view draws no such
  menu (its banner's "Menu" is a disclosure, the browser's own); one
  that does will need the page's events recorded too (section 10).

### 3.5 Fake only

A recording is made on a fresh `temper box --fake-backends`, started
as the help's `box` page starts one on fake backends (`temper help
box`), its printed sign-in link opened in the browser, and keeps what
the page was given as it was: the printed link's secret, a
typed value, the fakes' loopback addresses, all of them the fake box's
dummies, spent or gone with the box. Recording against real accounts
is not designed (section 10).

## 4. Replay

### 4.1 Through the shipped host

`journey::Replay`, native, starts the host from the recording's facts
as the shell's start does (`Host::new`, `forge_sign_in`, `start`) and
the page by `host::page`. For each turn it sets the clocks, gives each
report through `Host::report`, an act to the first node, in tree order,
on the page at that point of which the view makes the same event
(3.3), and draws at the turn's end. At each mark it yields the page
and, for an act's mark, every node on it that gives the act's event.

### 4.2 The check

- **Each report's duties,** written as their records, are the recorded
  lines, and each turn's deadline is the recorded one. The recording's
  answers are to those requests: a client that asks otherwise (a new
  read, another request's bytes, another deadline) would be answered
  what it never asked.
- **Stale** is where the replay stops: at the first duty or deadline
  that differs, a departure asked or not asked among them, or at an
  act whose event no control on the page gives. It names the turn, the
  recorded line and what the host asked instead.
- **Patches are not compared:** they are what the print shows.

### 4.3 The first replay's proof

`add` replays with the live prints: at each mark the replay's
`Page::html()` is the recorded one, or the recording is refused,
naming the mark and the first byte that differs. Such a replay is not
faithful, and the defect is fixed in the seam or the replay, never
recorded around. Once every mark matches, the live prints are dropped:
the print is the replay's from then on.

## 5. The print

### 5.1 The program

`temper-client-journeys` (`tools/journeys`), a program of temper's
workspace beside the gallery (`browser.md`, section 7):

- `cargo run -p temper-client-journeys -- add NAME FILE` takes a saved
  recording as the journey NAME, replacing one of that name, proves it
  (4.3) and prints it. A NAME is `[a-z][a-z0-9-]*`.
- `cargo run -p temper-client-journeys` reprints every journey from its
  recording. A stale one is named with its turn and line, its print
  left as it was, and the program exits 1 once the others are printed.
- `cargo run -p temper-client-journeys -- again NAME` records the
  journey NAME again while its acts hold (8.1).
- `add` and the reprint read no configuration, environment, clock or
  network, and start no server; `again` alone does. Its paths are its
  manifest's, as the gallery's are.

### 5.2 What it writes

Under `docs/design/client/journeys/`:

- **A journey's directory,** `NAME/`: its `recording`; its
  `README.md`, the journey's description, what it walks and what each
  mark shows, which `add` and the reprint leave as they find it; and a
  page for each mark, `01.html` onwards, in the marks' order, replaced
  whole at each print.
- **Beside the journeys:** `skein-ui.css`, skein-ui-bundle's own
  (`skein_ui_bundle::STYLESHEET`, skein's `ui-web.md`, section 12), and
  `journey.js`, the program's, both written at each print; and
  `overlay.css`, the person's, made empty when missing and never written
  otherwise.
- **Generated:** the pages, `skein-ui.css` and `journey.js` are
  generated files, owned by `scripts/regenerate.sh`, which rewrites them
  by the reprint, so that a rebase that conflicts in them is resolved
  by reprinting (agents' `landing-flow.md`, 3). A recording, a README
  and the overlay are sources, never regenerated. A stale journey fails the
  reprint, and so the regeneration: such a landing stops until the
  journey is recorded again (section 8).

### 5.3 A page

- **The host's page at its mark:** `Page::html()` with its root the
  document's `#app` and its dialogs `open`, under the shell's title,
  the main heading then "temper", as the gallery prints a state
  (`browser.md`, 7.1).
- **The document:** `<!doctype html>`, `html` with `lang="en"`; in its
  head the charset, the viewport, the title, the four stylesheets and
  `../journey.js`; in its body the page.
- **For a diff:** one tag per line, the break inside the tag before its
  `>`, so that nothing drawn gains a space; and no `data-node`, whose
  numbers follow the order the nodes appeared in, not what they are.
  skein's `dom::print` escapes `<`, `>`, `"` and `&` in text and
  attribute values alike, so the tags of `Page::html()` are found
  exactly.
- **Linked:** each control on a mark's page of which the view makes the
  act's event carries the next page's name,
  `data-journey-next="02.html"`, and, for an act other than a press, the
  act, `data-journey-act="submit"` or `"toggle"`: the act on any of them
  would have led there. `journey.js` follows a press's on a click, a
  submit's on its form's submit (Enter in its field), and a toggle's on
  its change; a click into a field to type in it never follows. Any other press, link or submit shows a note that the journey
  goes no further there, as the mockups' acts that lead to no drawn
  state do. What the browser does alone works as on the live page:
  hover, focus, a disclosure, typing, a narrower window.
- **Its neighbours:** each page names the pages before and after it in
  its head (`<link rel="prev">`, `<link rel="next">`), and `journey.js`
  follows them on `p` and `n` outside a field. A page whose mark had no
  act, the mark key's, leads on by these alone.
- **The stylesheets,** by paths relative to the page:
  `../skein-ui.css`, then `temper.css` and `task-pages.css` from
  `crates/temper-client-shell-web/` in the checkout, in
  `scripts/bundle.sh`'s order, then `../overlay.css`. The bundler copies
  stylesheets and their `files/` as they are, so `temper.css`'s images
  and fonts resolve beside it in the checkout as in the bundle, and a
  change to it shows on the next reload, with no print.

### 5.4 Opening them

As files: a journey's `01.html` opened in the browser from the
checkout. Nothing on them is fetched under CORS, which a `file://` page
fails, since `temper.css` draws its icons as background images, never
masks (`browser.md`, 2.5). A server from the repository's root draws
them the same:

```sh
python3 -m http.server 8766
# http://localhost:8766/docs/design/client/journeys/
```

## 6. Its tests

- **The seam, in `temper-client-shell-web`,** natively, in the host's
  world of `tests/host.rs` (the platform's reports given by the test):
  a walk recorded by `journey::Recorder` and replayed; the file's lines
  read back to the records written, each escape among them, and another
  version unreadable; stale at a changed ask, a changed deadline and an
  event no control gives; a control's recorded name changed, holding;
  an act with no event, kept nowhere; and the proof refusing a mark
  whose html differs.
- **The print, in `tools/journeys`' own tests:** every committed
  journey reprinted into a scratch directory equals its directory and
  the files beside it, byte for byte; one that differs or is stale
  fails, naming `cargo run -p temper-client-journeys`. A reprint keeps a
  journey's README.
- **No request lands as it is:** the same tests fail while
  `overlay.css` holds any byte, and for a journey whose `README.md` is
  missing or empty (section 7).
- **The links by act,** in the print's unit tests: a press's control,
  a submit's field and a toggle's box, each with its attributes.
- **`again`'s walk, natively,** in the journey module's tests and the
  host's world, with no browser: a recording's walk read as its steps
  (each typed act one, a drawing and a moving key mark refused); a save
  replayed to its end finding the control of a step's event by its
  role, name and order; a still page told from a moving one by two
  saves, an exchange or a write in flight among them; and the check
  refusing a new walk whose acts, values, events or key marks differ.
- **`again`'s browser half has no story of its own,** as the recorder's
  keys have none: its first run, a journey recorded again, proves it,
  and skein's browser tier proves the kit's letter with Alt and its
  downloads (skein's `browser.md`, section 9). No default test starts a
  browser or a box.
- **The recording bundle stays built:** the gate's browser-target
  check builds the shell with `loopback-navigation` and `record` too.
- **The recorder's keys and download** have no browser story: the first
  journey's recording proves them, and the browser tier proves only
  what a browser alone shows (`browser.md`, section 4). That a save
  leaves the recording going on is the recorder's, tested natively: two
  saves of one walk, the first a whole recording that the second
  extends.
- **Their cost:** the journeys' reprint together at most 1 s of the
  default suite, as the gallery's states (`browser.md`, 7.3); no fuzzy
  test.

## 7. Editing the look

- **On the print:** the person serves the checkout, walks a journey, and
  writes rules in `overlay.css`, with the browser's tools (a DevTools
  workspace mapped to the checkout saves the Styles pane's edits to the
  file) or in an editor, then reloads. The markup is edited in place: a
  word, an order, a part moved or gone.
- **Selectors the view can carry:** the overlay selects by the `temper-`
  names, roles and regions, which the view sets, never by a part's
  position among its siblings.
- **The diff is the request:** the overlay's rules and the pages'
  changes, under `docs/design/client/journeys/`.
- **Requests stack** on a branch of the person's own, `look-requests`,
  one commit each, named by a `Look-request: NAME` trailer (NAME
  `[a-z0-9][a-z0-9-]*`), which a rebase keeps. A later request is drawn
  on the pages the earlier ones ask for. They are done in order, each
  read by its name on the branch as it is then, as its own commit's
  diff.
- **Kept on `main`:** `scripts/look-refresh.sh`, run by the person in
  their checkout with `look-requests` checked out and clean, fetches
  `main` alone, rebases the branch on `origin/main`, drops each request
  whose name a commit on `main` carries in that trailer, and pushes it
  with `--force-with-lease`, its lease the branch as the checkout last
  knew it, so that a branch moved from another checkout is refused,
  never overwritten. It says, for each request, whether it was kept or
  dropped and by which commit. Nothing is built on the branch, so
  moving it breaks no one, and its one name leaves no copies behind. A
  conflict is a line of a request's that `main` reprinted since: the
  script stops with the rebase in progress, naming the request; the
  person settles it and finishes the rebase (`git rebase --continue`),
  then runs the script again, which refuses while a rebase is in
  progress.
- **Never landed:** the journeys' test refuses a request's pages, which
  differ from the reprint, and its overlay (section 6), so `main`'s
  overlay stays empty.
- **Done** is a landing, carrying the request's trailer in commits of
  its own (never the request's commit itself), that moves the overlay's
  rules into `temper.css` or `task-pages.css` and the markup's changes
  into the view, and reprints. The reprint then equals the request's
  pages, compared as text, not as pictures; where it does not, the
  landing says why, and the later requests take it as it is. What the
  edits imply beyond the pages recorded (the same part in a state no
  journey reaches) is the view's and `temper.css`'s for every state, and
  the mockups follow (`mockups/README.md`, 1).

## 8. A recording gone stale

A recording goes stale only when the client's exchange with the engine
changes, or what the client can do on a page (an event no control
gives); never for a change to the view or the look alone, which a
reprint carries, since its acts are events, not controls.

- **While its acts hold,** `again NAME` records it again (8.1). No
  person walks it.
- **When the walk itself changes** (a step gone, another added), the
  journey is walked anew in a browser on a fresh fake box (3.5), from
  its `README.md` and the change that moved it, then added (`add NAME
  FILE`). The README changes with it, and its new pages are read as new.
- **A new recording reads anew:** its times are the new walk's, and so
  are the fake box's loopback ports wherever a page shows one, so the
  print's diff holds those as well as what changed (section 10).

### 8.1 Again

`cargo run -p temper-client-journeys -- again NAME`, in
`tools/journeys`. Its walk's logic is the journey module's, beside the
recorder and the replay, natively and without io: it takes each save's
bytes and answers what to do next. The program reads the saves from the
person's downloads and drives the box and the browser, as ordinary Rust
over skein's harness.

- **The walk** is the journey's recording read as its steps, in order:
  each act, with its value and event, each key's mark, and the end. Each
  act is a step, an `input` among them (typing is given an act at a
  time, so that the new walk asks what the old one asked after each). A
  `drawing` act is no person's to give, and a key's mark that caught the
  page moving (a report other than a due within half a second after it)
  is no state a still page shows: such a journey is refused before
  anything starts, and walked anew.
- **A fresh fake box** (3.5): `temper box --fake-backends`, built from
  the checkout with the recording bundle (`scripts/bundle.sh --loopback
  --record`), every directory fresh in a scratch directory of its own;
  its printed sign-in link opened.
- **Headless Chromium, as the browser tier drives it** (`browser.md`,
  4): skein's browser kit through the fake person's browser face and
  harness (skein's `ui-web.md`, 14.2), Chromium from
  `SKEIN_TEST_CHROMIUM`, the person's downloads into the scratch
  directory.
- **The client first:** a page's load comes before the client in it
  starts, so the first save waits for the client's main region, which
  it draws once its recorder hears the keys.
- **Each step once the page is still.** A save (Alt+Shift+S) is the walk
  so far, a whole recording. The page is still when a save half a second
  after another holds no report the other lacks but dues, no exchange
  asked but not heard, and no write asked but not carried. A page not
  still within 30 seconds stops `again`, naming the step. Then:
  - **an act:** the still save is replayed natively to its end
    (section 4); the first node of which the view makes the step's event
    names its control by its role, its accessible name and its order,
    from 0, among the page's controls with both, and the face finds that
    control on the live page (the fake person's n-th match, skein's
    `ui.md` 17.1) and acts on it as a person: a press, a submit, a
    toggle, or the field's value selected and the act's value typed;
  - **a key's mark:** Alt+Shift+M;
  - **the end:** the still save is the new recording.

  A step whose event no control gives, in a still save, within 10
  seconds stops `again`: its acts no longer hold.
- **The check, then `add`:** the new recording's walk is the old one's,
  the same acts with their values and events in order and the key marks
  in their places, or `again` refuses, naming the first step that
  differs, and the journey is walked anew. The page may change between
  a still save and the act given; an act that then lands on a control
  making another event fails the check, and one that makes the same
  event is the same act. Then `add NAME FILE`, its first replay proving
  it (4.3), its README left as it was.
- **What it reads and starts:** the environment's Chromium, the clock
  and the loopback network; the box and Chromium, both stopped at its
  end, its scratch directory removed unless it failed, which it then
  names with what failed.

## 9. The journeys

| Name | The walk | Its pages |
|---|---|---|
| `welcome-set-up` | on a fresh `temper box --fake-backends` (3.5): the printed sign-in link opened, the welcome drawn, "Set up temper" pressed, then "Choose" on "Forgejo on this machine" | `01.html`, the welcome as it was when "Set up temper" was pressed, which leads to `02.html`, the Forge step's choices as they were when "Choose" was pressed, which leads to `03.html`, Forgejo on this machine's details as they were when the recording ended; its `README.md` says so |
| `welcome-playground` | on a fresh `temper box --fake-backends` (3.5): the printed sign-in link opened, the welcome drawn, "Try the playground" pressed | `01.html`, the welcome as it was when "Try the playground" was pressed, which leads to `02.html`, the chats page in the Playground project, the demo started, as it was when the recording ended; its `README.md` says so |

## 10. Open questions

- **Typing:** the draft has every act mark its page; the design marks
  none for an `input` (3.4), since a page per keystroke would make a
  walk of typing. The user's word.
- **The gallery's states,** set from hand-written worlds: whether each
  moves to a journey once the fake box reaches it, its answers then the
  engine's own (`browser.md`, section 7).
- **The page's own state at a mark** (3.4): where a journey meets a
  skein menu, the page's events would be recorded and replayed beside
  the host's reports.
- **A new recording's noise:** fixed loopback ports for the fake box,
  which must not collide when several record at once, and a fixed start
  for its clock and the browser's, so that a recording made again
  differs from the last only where the client does.
- **A longer walk for `again`:** skein's harness ends its whole run at
  40 seconds, which holds the box's start and every step's saves;
  `welcome-set-up`'s fits in about 10. A journey that doesn't would need
  the harness's horizon set by its setup, skein's.
- **Later, if wanted:** the one-tag-per-line print as skein-ui-web's,
  for another application's journeys.
- **Real accounts:** a recording against a box with real accounts would
  need its secrets kept out (the printed link's, a typed key's); not
  designed.
