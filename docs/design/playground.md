# The playground in the browser

Provisional, 2026-10-26, from the user's `draft/browser-playground.md`.
The box's playground served as static files and run whole in a
visitor's browser: temper's engine and worker, jig's store, smith's
agents and the fake LLM, forge and git on skein's simulator in a Web
Worker, and temper's web client on the page. skein's kit runs the world
in the worker and carries the client's transport to it (skein's
`playground.md`); this document is what temper composes on it. Its bar
is a desktop browser: nothing here is for phones, which wait until
desktop browsers are well supported (the user, 10-26).

## 1. In one page

- **The box's playground, with no box.** The playground a person meets
  in `temper box` ("Try the playground": the Scripted model, the
  Playground forge, `playground/widget`; `../protocol/setup.md`, 8.4)
  runs in the visitor's browser: nothing to install, no account, no
  server, no real model.
- **Product code on fakes, with no fake branch.** The engine, the
  worker, the store, the agents and the client are the shipped crates.
  Only the composition differs: the simulator's machine for the
  operating system, skein's visitor and bridge for the network, and
  documents of its own for the configuration (section 3).
- **One composition, two users.** `temper-playground` composes the
  world; the one-process world composes it with its harness, so the gate
  runs the world the browser runs (section 3).
- **The visitor signs in by the printed link,** which the world's
  service prints as `temper serve` does; the page puts it in the
  address and the client spends it as a person's browser does
  (section 6).
- **Stories are data** (section 5): the box's three requests first,
  from the box's own scripts, with a pace a person can follow; then the
  ladder's rungs as their live transcripts become scripts.
- **A world that lives while the page does,** paused and sped by the
  page's frame, reset by starting it again, and nothing saved (section
  6). A failure offers the session's log, which replays natively as a
  test (section 9).
- **Nothing leaves the browser.** Once loaded, the page fetches nothing:
  no model, no analytics, no telemetry (section 7).
- **Bounded by its documents** (section 8): capacities just big enough
  for its stories, the simulator's trace off, no captures.
- **Its bar for v0.1.0** (section 10): story A walked in desktop
  browsers, published with the release.

## 2. What the visitor sees

- **A loading line,** then the client as the box's person sees it after
  following the printed link: signed in as the deployment's owner, on
  the welcome. The visitor chooses "Try the playground", and the demo's
  start makes the project Playground, adds the Scripted model and
  adopts `playground/widget` (`../protocol/setup.md`, 8.4). From there,
  the box's playground: the banner, the three suggested requests, a
  change followed from a branch to `main`.
- **Only what the deployment can do is offered.** The playground's
  document configures the demo alone, so the client should offer no Set
  up temper, no Connect a forge and no Connect a model. That is the
  client's rule for every deployment that configures them out, never a
  branch for the playground (`../client/README.md`). Until it is built,
  the welcome offers Set up temper beside Try the playground, as on any
  deployment, and the playground's stories never press it (section
  11).
- **The frame around the client** is the page's own, not the client's:
  what this is ("temper's playground: a scripted model and forge,
  running in your browser"), the world's controls (pause, speed, reset,
  the story), the world's time, temper's version, the way to the real
  thing (the download), and the session's log. It has temper's look and
  a mockup of its own, which the user chooses as every page's.
- **A pace a person can follow.** A story's replies and CI take time,
  set in its documents (5.1), so a delegate's phases show at 1×; the
  speed control shortens them.
- **A reload starts over.**

## 3. The crates

| Crate | Where | What |
|---|---|---|
| `temper-playground` | `crates/` | the world's composition and its stories' documents; on wasm32, the worker's start |
| `temper-client-shell-web` | `crates/` | gains the feature `playground`: skein's worker platform for the page, the starter, the frame and the failure page |
| `temper-fake-git-hosted` | `crates/` | moved from `testing/`, its fixtures staying with the tests: the fake git program the world hosts ships in it; what the composition takes from `tests/fake-checkout` moves to it too |
| `temper-playground-world` | `tests/playground` | the stories walked natively, through the client's host (section 9) |

- **`temper-playground` depends on product crates alone,** on every
  target: what the one-process world takes today from test kits (jig's
  channel world's documents, smith's process world's profile, limits and
  error pipe, skein's LLM world) becomes the playground's own documents,
  or is taken from the product crates that hold it (skein's simulated
  world's pipe capture). Its build for wasm32 proves it.
- **The composition,** moved out of `tests/one-process`: the service,
  the engine and one worker in one `iterate`, started from configuration
  bytes as `temper serve` starts them; jig's store on skein-kv over the
  fake machine's disk; smith's agents hosted (smith's `shell.md`, 2.5);
  the fake LLM and fake forge peers on simulated sockets; the fake git
  program; skein's visitor (skein's `playground.md`, 4). Its process
  type and its program registry are the composition's; the harness's
  processes (the fake person, the terminal client) join them in the
  one-process world.
- **What stays in `tests/one-process`:** its referee, its fake person in
  front of the terminal client, its captures, its faults, its kills and
  power losses, its fixtures' scenarios, all over the composition.
- **A library on every target and a module on wasm32:** natively,
  `temper-playground` is the composition the one-process world, the
  stories' tests and a log's replay build; on wasm32 its start, the
  worker's module's entry, makes the world from the start message's
  story, seed and start, and hands it to skein's slices.

## 4. The world

- **What runs** is the one-process world's composition
  (`domain/testing.md`, 2.5) without its harness: no referee, no fake
  person, no terminal client, no captures and no fault schedule.
- **No box.** The fakes are the world's processes, as for a deployment
  without the box (`../protocol/setup.md`, 7.4): the forge a `fake`
  setup entry whose handover the composition lays on the disk, the
  model the demo's provider. The demo's start asks nothing of a pipe.
- **Calm at every layer:** no fault in the simulator, the fake LLM or the
  fake forge.
- **One seed:** every seed of the world (the simulator's, the
  service's, the agents', the fakes') derives from the visit's one
  seed, which the page draws and the log keeps.
- **The printed link** reaches the page by the service's standard
  output: the composition's host of the service drains its links each
  iteration and writes each as a line, as `temper serve`'s host does,
  to a pipe the world captures (skein's `world.md`, 2), whose lines the
  bridge brings (skein's `playground.md`, 5).
- **The clients' listener** is at the address the deployment's document
  names, and the visitor sends that address as its Host, so jig's guards
  hold as on a deployment.

## 5. Stories

### 5.1 What a story is

- **Data, no code:** the deployment's document, which is the demo's
  part with the forge, the provider and the project; the fake LLM's and
  the fake forge's documents, in `temper-fake`'s form of a document of
  its own (`../protocol/setup.md`, 8.4), which hold the scripts and the
  seeded repositories with their files, CI and cues; its pace; and a
  description of what the visitor is expected to do, which the frame
  shows.
- **Its pace** is the fakes' latencies, which their documents gain: the
  fake LLM's before each reply's pieces, the fake forge's CI's before
  its verdict, each a fixed duration, so that a delegate's phases show
  at 1×. The box's fakes keep none.
- **One world per story.** A story's scripts rely on its repository's
  state, so a story is chosen in the frame, and a reset starts the same
  story again on a fresh world.
- **Compiled into the worker's module:** nothing is fetched after the
  load.
- **Checked as data** in the gate, as `temper-init`'s scripts are: each
  cue matched apart, within the fakes' bounds; and walked whole
  (section 9).

### 5.2 The first: A

- **The box's playground** (`../protocol/setup.md`, 8.4): three
  suggested requests. "Add CSV export to widget" is delegated, edited,
  pushed, passed by CI and landed; the dark theme and the empty search
  are answered in text.
- **One source for its scripts:** `temper-init`'s template, which the
  box writes. The story is derived from it, its `otherwise` worded for
  the browser (5.4), so the box and the browser tell one story, and the
  ladder's rung A walks the same requests on real backends.

### 5.3 From the ladder

- **B0 to B2 become stories as their rungs pass,** each from its passing
  live transcript, which serves as the rung's fake twin, the
  playground's story and the guide's example at once. A story is added
  as data, with no code.
- **A visitor's answer** in B0 is free text: the delegate's script goes
  on whatever it says, or branches on cue words by the fake's turn rules
  (`inputs`).

### 5.4 Off-script

- **The scripted model says what it is:** the playground's, answering
  only the story's requests, which it names again, and that a real
  model comes with the download. Agents never go off-script: each is
  cued by the brief a script gave it.

## 6. The page

- **Built from the shipped shell,** `temper-client-shell-web` with its
  feature `playground`, which holds skein's worker platform where the
  shipped shell holds the browser's (skein's `ui-web.md`, 18): the
  shell's platform is a type the feature chooses, and nothing else of
  its loop changes.
- **The starter:** it shows the frame and the loading line, starts the
  worker with the story, the seed it draws and the browser's time, waits
  for the printed link among the lines the world prints, puts the
  link's path and query in the address, and starts the client, whose
  sign-in part spends it as a person's browser does (jig's
  `protocol/browser.md`, section 11). No credential is made by hand.
- **The frame** wires its controls to the bridge's pace, the worker's
  restart and the story's choice, shows the world's time from the
  platform's clock, and offers the session's log by a key.
- **One life per page.** A reload, a reset or a new story ends the
  worker and starts a fresh world; nothing is saved.
- **A failure** in either module stops the playground: the page says it
  failed, offers the log and a start over, and does not reload by
  itself.
- **The playground forge's links,** a pull request's among them, are
  shown as text for a forge that serves no pages.

## 7. Served

- **Static files:** the page, its starter, the two modules and their
  glue, the stylesheets; each but the page under its digest's name and
  cached for good, the page with a short lifetime (skein's `ui-web.md`,
  4.4). Any path answers with the page, which always starts at the
  printed link.
- **At the site's playground address,** which the site's design names.
- **The page's policy** is temper's (`client/browser.md`, 2.2) with
  `worker-src 'self'`, and no `connect-src` beyond `'self'`: after the
  load, nothing is fetched.
- **Built by `scripts/bundle.sh --playground OUT`:** the web shell with
  `playground` and `temper-playground` for wasm32, in the `web`
  profile, bundled by skein's bundler with the worker's module (skein's
  `playground.md`, 7).
- **Released with temper,** built at each release tag from the commit
  the download is built from, and linked from the site and the guide.
  Between releases, a preview of `main` is private, for the user and QA.
- **Nothing is collected.** A log reaches us only as a file a visitor
  attaches to an issue.

## 8. Bounded

- **Its documents bound it:** the engine's and the worker's memory, jig's
  capacities, smith's windows and the fakes' limits, each just big
  enough for its stories, every story at its high water. The trace is
  off (skein's `world.md`, 7) and nothing is captured.
- **Measured first:** what reserves the world's memory at its first step
  and through each story, and which of it the documents control.
- **Its figures,** the world's memory at its high water and the download
  compressed, are measured by the browser tier and reported. The
  draft's defaults, 256 MiB and 6 MB, were set for a phone; with phones
  out, the bars for a desktop browser are the user's (section 11).
- **The first paint does not wait for the world:** the frame and the
  loading line come first, and the world's module compiles as it
  streams, in the worker.

## 9. Testing

- **Natively, in the gate,** `tests/playground`
  (`temper-playground-world`): each story walked whole in the
  playground's world through the web client's host built natively,
  driven by skein's fake person on its tree as a journey's replay drives
  it (`client/journeys.md`, section 2), the host's duties carried to the
  world by skein's bridge in memory (skein's `playground.md`, 5): story
  A from the welcome to the CSV change landed and the other two
  requests answered. Within a world's budget (`domain/testing.md`,
  6.1).
- **The documents' memory bounds** are checked at start as any
  document's are, so a story past them fails its walk.
- **The wasm32 builds,** in the gate: `temper-playground`, and the web
  shell with `playground`, with clippy's lints as errors for
  `wasm32-unknown-unknown`.
- **A log replayed:** a session log of story A, kept as a fixture,
  replays natively under the one-process world's referee to the same
  end. A change that makes it differ is a new log, recorded again, or a
  finding.
- **In a browser, outside the gate:** the browser tier walks story A on
  the built playground in headless Chromium (`client/browser.md`,
  section 4), and measures the download, the memory's high water and a
  slice's latency, reported against their bars.
- **By hand, before the release:** Firefox and Safari on a desktop,
  walking story A.

## 10. Its bar for v0.1.0

- **Story A walks in the browser,** from the welcome to the CSV change
  landed and the other two requests answered: in the browser tier, and
  by hand on Firefox and Safari on a desktop.
- **Within its bars,** once the user sets them for a desktop browser
  (section 8), and a page that answers while the world steps.
- **A failure offers its log,** and a log replays natively.
- **Published** at the site's playground address with the release.
- **B0 to B2 are not in the bar.**

## 11. Open questions

- **What the welcome offers on a deployment that can set nothing up:**
  the client tells nothing of it today, and the welcome is the user's
  page. Default: the welcome as it is, Set up temper unpressed, until
  the user says what replaces it.
- **The memory and download bars for a desktop browser:** the draft's
  256 MiB and 6 MB were a phone's; measured first, then the user's.
- **The welcome, or straight in.** Default: the welcome, the product's
  path.
- **The address,** and the crate's name `temper-playground`: the site's
  design decides the address; the name is provisional.
- **The frame's mockup,** the user's.
- **A hidden tab:** paused (skein's `ui-web.md`, 18), the default.

## 12. Later

- **Pull the plug:** a button that kills the service or cuts the power
  at a chosen moment, then shows temper recover on the fake disk: the
  one-process world's kill and power loss (skein's `world.md`, 3).
- **Saving across a reload,** in IndexedDB.
- **The playground forge's pages:** a pull request's diff in the page,
  shared with the box's playground.
- **The reference case's acts as stories,** once rung C passes; **the
  box's playground taking every story;** **journeys recorded on the
  browser playground,** with no box behind it.
- **Phones,** once desktop browsers are well supported.
