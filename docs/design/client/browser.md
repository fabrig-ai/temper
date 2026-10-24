# The browser

Provisional, 2026-10-05, revised 2026-10-23. temper's browser client: a
second shell of temper's client (`README.md`), beside the terminal, on
skein's browser parts. What its pages do is the client's (`README.md`;
`forge.md`), with the mockups' look (`mockups/`, reference material),
which this document takes as given; how a view runs in a browser is
skein's (skein's `ui-web.md`), and how a client of jig's does is jig's
(jig's `protocol/browser.md`). It is built in two milestones (section
6). What is still open is section 5.

## 1. In one page

- **A second shell of the same client.** temper's client root, its view
  and the forge's family (`README.md`, sections 3 to 5), compiled to
  WebAssembly under a browser shell of temper's,
  `temper-client-shell-web`, on skein's browser parts (`skein-ui-web`;
  skein's `ui-web.md`) and jig's client adapter and sign-in part
  (`jig-client-shell-web`; jig's `protocol/browser.md`, 5.1), with
  neither jig's look nor its reference UI. The adapter carries jig's
  default HTTP mapping over fetch (`jig-client-protocol-http`). temper
  writes no web wire and no web protocol of its own: the client's
  records cross as jig's wire carries them, binary records from a schema
  per owner, never JSON (jig's `protocol/client.md`, section 3).
- **The pages temper has,** in both shells: the inbox, the chats, a
  chat, a task's page and the adoption form (`README.md`, 3.5). A page a
  later plan adds to the view (the board, the changes, the accounts) is
  in the browser once it is in the view, with its look and, where it
  has one, its browser-only story (4.2).
- **No framework.** The client and its view are step code under the
  subset in a browser too (skein's `ui.md`, section 1).
- **The view tree is the fake DOM.** Below the browser, every world
  reads skein-ui's tree by role, name and text through skein's fake
  person (skein's `ui.md`, 17.1): no browser, no WebAssembly and no fake
  DOM library.
- **Served by temper's server, from one origin.** `temper-service` serves
  the page and the bundle on its clients' listener (skein's `ui-web.md`,
  4.4), beside jig's clients' component under `/jig` (`README.md`,
  section 8), from the bundle the binary holds or a directory its
  configuration names, with jig's example guards and the page's policy,
  which is temper's: a strict content security policy, no frames, no
  CORS (jig's `protocol/browser.md`, 12.1 and 12.2). A binary built
  without its browser serves the page for no bundle, and the terminal
  works as before (2.2).
- **The bundle is in the binary.** A plain `cargo build` (or `cargo
  build --release`, or `cargo install --git`) makes a `temper` that
  holds its bundle, so that running it is the whole install. The
  machine's builds, temper's gate and its agents, build without it
  (`TEMPER_BUNDLE=none`), as fast as before (2.6).
- **Signed in by the printed link,** opened in a browser on the engine's
  machine: jig's sign-in part spends it, the credential is the
  mapping's cookie, and a reload signs in again by it (2.4).
- **Browser tests only for what a browser alone shows.** temper's
  stories are the worlds' and the real loop's, through the terminal
  shell (`../domain/testing.md`, 2.1, 2.4 and 6.3). The browser tier proves
  the bundle under its policy, signing in by the link and the cookie,
  addresses, the composer's keys and jig's attack stories that need a
  browser, and nothing a world can show (section 4).
- **A real browser at the top, driven from Rust,** by `skein-browser`
  (section 4).
- **The gallery** prints the browser's pages in named states as static
  documents, beside the bundle's stylesheets, for screenshots held to
  the mockups: no server, no script, no network (section 7).

## 2. What temper brings to the browser

- **Its crates as they are.** The client root, the forge's side, the
  view and `temper-client-protocol-wire` are step crates under the
  subset, which build for the host and for wasm32 alike; the terminal
  and the agents' tools are not part of the bundle.
- **Its look:** after the mockups (`mockups/style.css`), as temper's
  style tokens and a stylesheet over skein's (skein's `ui-web.md`, 3.2
  and section 12): 2.5.
- **Its pages,** the client's (`README.md`, 3.5), each at an address of
  its own (2.3); and the forge's parts, from `forge.md`, as the forge's
  side reads them.
- **Signing in:** by the link `temper serve` prints (2.4). The forge as
  people's sign-in provider is temper's own routes (`README.md`,
  section 11).
- **The toolchain:** the standard library for `wasm32-unknown-unknown`,
  skein's bundler and the glue it names (skein's `ui-web.md`, section 4),
  and Chromium for the browser tier.

### 2.1 The shell

- **`temper-client-shell-web`** is a `cdylib` for
  `wasm32-unknown-unknown`, ordinary Rust as a shell is (skein's
  `ui.md`, section 2), copied from jig's reference browser shell,
  `jig-client-test-shell-web` (jig's `protocol/browser.md`, 3.1), with
  temper's client root, view and records in place of jig's test
  application's, and none of jig's look (`jig-client-shell-web`'s
  `look`, `jig.css`). Its modules:
  - `lib`: the start (skein's restart part; jig's sign-in part taking
    the link from the address before anything is sent; the small store;
    the page with temper's look; the DOM) and the turn of skein's
    `ui-web.md`, 5.2; and, as jig's reference shell has it, the test
    address `/test/panic`, at which the shell panics at start, for
    skein's `panic` story (4.1);
  - `host`: the client host: temper's client root and view, and the
    view's patch queue, reserved at start at the view's limit on
    patches and drained each turn (skein's `ui-web.md`, 5.2); the root's
    `Leave`, a sign-in's departure (`README.md`, 3.5, The tab's
    departure), as skein's transport's navigation away, in the turn
    that asked it (skein's `ui-web.md`, section 10); jig's
    mapping under `/jig` and its stream; the root's requests to jig's
    adapter (`jig-client-shell-web`'s `adapter`) and skein's transport,
    and their reports back to the root; and the wake, skein's timer
    set at the earlier of the root's deadline and the instant the
    view's words next change (`README.md`, 5.12);
  - `records`: the small store's records, `jig-client`'s alone; from the
    adoption form (section 6), a pending adoption is among them, a kept
    request whose request `temper-client-protocol-wire` encodes
    (`README.md`, 4.6);
  - `limits`: the client's limits in a browser, derived as the
    terminal's are (`README.md`, section 2), and their sum.
- **It decides nothing.** What a page shows and does is the domain's
  and the view's, step code under the subset; the shell orders the
  platform's work (jig's `protocol/browser.md`, 3.1).
- **The failure page:** past the restart part's limit, the page says
  "The application failed.", in temper's words, since it takes none of
  jig's look; it starts nothing and keeps the small store (jig's
  `protocol/browser.md`, 5.1).
- **The small store** is the browser's session storage, under the
  prefix `temper.`, holding `jig-client`'s records alone, as the
  terminal's file does (`README.md`, section 7); from the adoption form
  (section 6), a pending adoption is among them, a kept request
  (`README.md`, 4.6). Each load of the page is a client of its own, with
  its own stream (jig's `protocol/browser.md`, section 10). The root can
  step the client several times in one turn (a sign-in's answer saves its
  `Return` and erases the answered request): each write the client asks
  for (`Save`, `Erase`) is answered once, after the turn's whole storage
  has finished, and every one is refused if any write was refused.
- **What the terminal says in lines of its own** (`README.md`, 3.4 and
  6.6): another party's kept work and kept work dropped have no place
  in the browser, whose small store lives as long as its tab, of one
  person (section 5). A service's credential is never shown: temper's
  client makes no service (`README.md`, section 11).

### 2.2 Serving

- **The routes of the clients' listener,** temper's, in this order,
  over jig's mapping (jig's `protocol/browser.md`, section 12) and
  skein's serving of a bundle (skein's `ui-web.md`, 4.4):
  1. what jig's default mapping claims, under `/jig`, to jig's clients'
     component: every request there but a `GET` of the printed link's
     route, which the mapping leaves to the page, answered with the
     page's policy and `Referrer-Policy: no-referrer` (jig's
     `protocol/browser.md`, 12.1 and 12.2);
  2. with no bundle, the configuration naming none (`null`) and the
     binary built without its own (2.6), a `GET` of any other path but
     under `/bundle/` and `/robots.txt`, the printed link's route among
     them, answers `200` temper's page for no bundle, every other
     request not found, as `README.md`, section 8 says. The page is
     fixed HTML in the binary, under the page's policy, with no script,
     style or picture: the level-1 heading "temper", then "This temper
     was built without its browser client.", "Open the link in a
     terminal with temper open DIRECTORY LINK." and "To serve the
     browser, build temper with the wasm32-unknown-unknown target, or
     name a bundle's directory in application.clients.bundle, and
     restart temper." (`../protocol/setup.md`, 8.2);
  3. a request other than `GET`: `405`;
  4. `/bundle/<digest>/…`: the bundle's files, immutable; any other path
     under `/bundle/`: not found;
  5. `/robots.txt`, the bundle's one root file (skein's `ui-web.md`,
     3.3, step 5; jig's `protocol/browser.md`, section 12), disallowing
     every path;
  6. any other `GET`, the printed link's route among them: the page,
     with its ETag and `no-cache`, and `304` on a matching
     `If-None-Match` (skein's `ui-web.md`, 4.4).
- **The page's policy,** on every answer temper gives: jig's reference
  policy as it stands (jig's `protocol/browser.md`, 12.1): `default-src 'none';
  script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src
  'self'; font-src 'self'; connect-src 'self'; base-uri 'none';
  form-action 'none'; frame-ancestors 'none'`, with `nosniff`,
  `Cross-Origin-Opener-Policy: same-origin`,
  `Cross-Origin-Resource-Policy: same-origin` and `Referrer-Policy:
  no-referrer`. skein-http's own refusals, `400`, `413` and `431`, given
  before temper sees the request, carry none: they hold no page to frame
  or sniff. temper adds no origin: no font, picture or script from
  elsewhere. The mockups' optional web font is not used; the look takes
  the system's monospaced font.
- **jig's example guards, as they are** (`README.md`, section 8): the
  Host allow-list holding the listener's own address; a cookie without
  jig's browser header refused.
- **The bundle** is skein's bundler's output (skein's `ui-web.md`,
  section 4): the shell's module, skein's stylesheet and temper's
  (2.5). temper's build embeds it in the binary (2.6);
  `scripts/bundle.sh` builds one apart, from the module built for
  wasm32, into a directory it prints. `application.clients.bundle` is
  `null` for the binary's own, or names a directory, whose bundle is
  served in place of the binary's (`../protocol/configuration.md`,
  section 3). `temper serve` makes it, at start, whole, into memory,
  within the limits `clients.bundle_files` and `clients.bundle_bytes`
  (`../protocol/configuration.md`, section 7), and refuses the start
  past them, or when it is no bundle (its page and its digest's
  directory), by one code for both: a directory's files read, or the
  binary's, already in memory, copied (2.6). A new bundle is served
  after a restart; a tab on the old one is answered not found for its
  files, and a reload loads the new (skein's `ui-web.md`, 4.4). A page
  from the binary's own bundle always speaks its engine's records, so a
  client out of date (`README.md`, 5.1) comes only from a binary
  replaced under an open tab, or from a directory configured.

### 2.3 Addresses

- **Each page's address is a path,** by temper's address codec, a
  module of `temper-client-domain`, step code, total and bounded:

  | Page | Path |
  |---|---|
  | `Inbox { filter }` | `/`, and its filter's query |
  | `Chats { filter, start }` | `/chats`, and its filter's and start's query |
  | `Accounts` | `/accounts` |
  | `Chat { task }` | `/chats/<task>` |
  | `Task { task, filter }` | `/tasks/<task>`, and its plan's filter's query |
  | `Adopt { project }` | `/projects/<project>/adopt` |
  | `Welcome { step }` | `/welcome`, and its step's query |
  | `Forges` | `/forges` |
  | `ConnectForge { forge }` | `/forges/connect`, and its choice's query |
  | `ConnectModel { way }` | `/accounts/connect`, and its way's query |
  | `Authorize { query }` | `/oauth/authorize`, and the query as the address carried it, within jig's bound on an authorization's query (`../protocol/sign-in.md`, 9.3) |

  `<task>` and `<project>` are the task's and the project's numbers in
  decimal. A later page's path comes with it, in this shape
  (`/projects/<project>/board`, `/projects/<project>/changes`).
- **The inbox's filter is its query:** the fields it sets, in one
  order, `?project=<project>&kind=<kind>&goal=<goal>`, and none with
  every field unset, so that `/` is the inbox unfiltered and
  `/?kind=question&goal=9` one filtered by kind and goal. `<project>`
  and `<goal>` are numbers in decimal, as a path's; `<kind>` is one of
  `proposal`, `question`, `held` and `person-task`: the kind's button
  word (`README.md`, 3.5's inbox), in lower case, its space a hyphen.
  The codec reads the fields between `&`, each `name=value`, and drops
  one it doesn't know, one whose value doesn't read as its field's
  (`kind=reply`, `project=07`, a number past its type), and one named
  again after a first that read; the rest set the filter. So a stale
  or edited link opens the inbox less filtered, never refused, and its
  address is replaced with the printed one. It reads numbers, not the
  person's projects: a project the person isn't in shows an empty
  inbox, which "All projects" undoes. The inbox's filter, the chats
  list's and a goal's plan's (below), the welcome's step,
  `ConnectForge`'s choice, `ConnectModel`'s way (below) and
  `Authorize`'s query, kept as it came and never printed again, alone
  are in an address, and another page's query is dropped, `/chats/7?x=1`
  replaced with `/chats/7`.
- **The chats list's and a goal's plan's filters are their queries,**
  as the inbox's (the user's pick, 10-23): none for "All", so that
  `/chats` is the list whole; `?state=live` for "Live" and
  `?state=closed` for "Closed"; a goal's `/tasks/<task>?state=live`
  for its plan's "Live only" (`README.md`, 5.8). A `state` that
  doesn't read as one of the page's, `/tasks/9?state=closed` among
  them, or a field of another name, is dropped, and the address
  replaced with the printed one. A filter chosen is a move (below).
- **The chats' start is `?at=start`,** as the welcome's step is
  `?at=`: `/chats?at=start` is the Chats page
  with the focus in its start's text area ("Start a chat", or the
  demo's start's while the demo's start is drawn in its place), the
  address the inbox's "+ New chat" links to (`README.md`, 3.5's
  inbox); with a filter, `state` comes first,
  `/chats?state=live&at=start`. Every other `/chats` leaves the focus
  on its heading, as any move does.
- **The welcome's step is its query:** none for its first step,
  Welcome, so that `/welcome` is the welcome; `?at=forge`, `?at=model`
  and `?at=ready` for the others (`README.md`, 3.5, The welcome). As
  the inbox's filter, an `at` that doesn't read as a step, or a field
  of another name, is dropped, and the address replaced with the
  printed one: `/welcome?at=done` opens the welcome at `/welcome`.
  Its Forge step adds its choice, as `ConnectForge`'s (below):
  `/welcome?at=forge&forge=3`; its Model step its way, as
  `ConnectModel`'s: `/welcome?at=model&provider=1&way=key`.
- **`ConnectForge`'s choice is its query:** none at Choose, so that
  `/forges/connect` is the flow's first step; `?forge=N`, N a forge's
  number in decimal, or `?forge=elsewhere`, for a Forgejo elsewhere
  (`forge.md`, 5.4). A value that doesn't read as one, or a field of
  another name, is dropped, and the address replaced with the printed
  one. It reads a number, not the deployment's forges: a number that
  names no forge shows Choose.
- **`ConnectModel`'s way is its query:** none at Choose, so that
  `/accounts/connect` is the flow's first step; `?provider=N&way=W`,
  N an LLM provider's number in decimal and W `sign-in`, `program` or
  `key`, in that order, for the way of that kind its provider offers
  (`README.md`, 3.5, `ConnectModel`). The two go together: one without
  the other, a value that doesn't read as its field's, or a field of
  another name, is dropped, and the address replaced with the printed
  one. It reads a number and a kind, not the configured providers: a
  way they do not offer shows Choose. A sign-in's way may add its
  sign-in's source, `&source=S`, S a source's number in decimal, the
  address a sign-in returns to (`README.md`, 3.5, The tab's departure):
  `/accounts/connect?provider=1&way=sign-in&source=7`. A source on
  another way, or one that doesn't read as a number, is dropped as
  above.
- **A filter chosen is a move** (below), with its history entry: a
  link's, at the page's top with the focus on its heading, and back
  returns to the filter before. Like any move it folds the open
  disclosures (2.5), so the Kind menu closes on its choice. A move
  that stays on the chats list, or on one goal's page, changing its
  filter or its start alone, keeps the page: its composers' words, the
  closed chats and ended delegates it read, which it reads not again;
  the inbox's filter, a query the engine answers, reads its page anew.
- **A path that is no page** opens the inbox, and the address is
  replaced with `/`. The codec never prints a path under `/jig`,
  `/bundle/`, `/test/` or `/forges/N/github/`, temper's routes for
  GitHub's steps (`forge.md`, 9.1), nor `/robots.txt`.
- **Moving** is skein's (skein's `ui-web.md`, section 9): a link pressed
  is a move within the client, with a new history entry; back and
  forward are moves too, each entry keeping its scroll; a reload, or an
  address typed or opened, is a new client at that address (jig's
  `protocol/browser.md`, section 10). That a client opens at the inbox
  (`README.md`, 3.4) is the terminal's: in a browser the address is the
  browser's, and the client keeps no record of its own.

### 2.4 Signing in

- **The printed link,**
  `http://HOST:PORT/jig/v1/sign-in/link?secret=SECRET`, opened in a
  browser on the engine's machine (`README.md`, section 7): temper's
  server answers its `GET` with the page (2.2); jig's sign-in part takes
  the secret and replaces the address with `/welcome`, temper's home for
  a sign-in, before anything is sent; the client signs in with it
  (`SignIn { with: Link { secret, held: None } }`), and the mapping's
  answer sets its cookie, which the client never sees (jig's
  `protocol/browser.md`, section 11). The browser sends its cookie with
  the link's sign-in, as with any exchange: a person still signed in
  keeps their sign-in, its number and expiry, and the answer sets no
  cookie, so the browser's stays as it was (jig's `domain/people.md`,
  section 3); a cookie the engine does not know, or one ended or
  expired, signs in afresh, with a new cookie. The welcome follows,
  which moves on to the chats page once the first run is over
  (`README.md`, 3.5, The welcome).
- **A reload, or another tab,** signs in with `Transport`, by the cookie
  (jig's `client/domain/client.md`, section 12).
- **Signed out,** by the banner's "Sign out" (jig's sign-out clears the
  cookie) or by a link spent or expired: the main is the view's
  signed-out paragraph (`README.md`, 5.1), in the browser's words:
  "Signed out. To sign in, open a sign-in link `temper serve` printed."
  The view takes its shell's words at construction; the terminal's stay
  "Signed out." with its own line (`README.md`, 6.6).
  With a way of signing in configured, the main is the sign-in page
  instead: temper's mark, "Sign in" and "Continue with Google", the ways
  read from the page's `temper-sign-in` meta at start
  (`../protocol/sign-in.md`, section 7); the link leaves for Google's
  sign-in and comes back to the page's address.
- **A link's sign-in not taken,** answered full, busy or not ready: the
  link is not spent, and the main says why and that the link stays
  usable until it expires (`README.md`, 5.1), full with the deployment's
  room, its earliest end and the link's own. The retry is the person's: opening the
  printed link again sends its secret again, as the first open did, and
  signs in once there is room; opened past its expiry, it is refused,
  and `temper serve` prints a fresh one. The page never retries by
  itself, and keeps no secret: the address already says `/welcome`.
- **One person, on loopback,** as `README.md`, section 7 says. A browser
  on another machine is open (`README.md`, section 11).

### 2.5 The look

- **temper's style tokens,** under the owner's prefix `temper-`, on the
  nodes the mockups style: the frame's banner, navigation and main; the
  cards; the conversation's turns; the composers. The view sets them
  (`README.md`, 5.1); the terminal prints none.
- **The bundle's stylesheets:** skein's `skein-ui.css`, then
  `temper.css`, which takes its rules from the mockups' terminal sheets
  as they render (`terminal-chat.css` and `terminal-pages.css` over
  `style.css`); the mockups are not shipped. These are the bundle's three
  stylesheets; `jig.css` is not in it. `temper.css` holds the mockups'
  palette as variables under the prefix, their monospaced stack, compact
  top navigation and quiet borders. DM Sans, for the wordmark alone, is
  self-hosted in the bundle's files with its SIL OFL licence beside it;
  the page fetches nothing from another host. The mockups' icons are
  decorative masks drawn by `temper.css` from the bundle's SVG files.
  Every control keeps its words as its name. Nothing a person must know
  is said by colour or icon alone (skein's `ui.md`, section 8): a status
  is its word, and its colour only repeats it.
- **Connection flows:** the welcome steps use compact 14 px headings and
  9 px card corners; its opening introduction keeps its larger heading.
  Standalone connection cards use 5 px corners. The
  current step takes the accent. The view assigns `FLOW_PRIMARY` to
  filled terracotta acts and `FLOW_PAIR` to the organization/repository
  row, stacked below 600 px. Progress draws ✓, ●, ✕ and ○ for done,
  current, failed and pending; status words remain accessible in the DOM,
  visually hidden. Back, Later and the playground alternative are quiet
  underlined text controls.
- **The task pages' stylesheet,** `task-pages.css`, comes third, after
  `temper.css`, whose variables and rules it builds on: what the task
  pages add to the look, as `task.html` draws it in the terminal
  sheets (`README.md`, 5.8): the result a goal gives as an outlined
  tag at its card's top, a plan row's spend as a fourth, right-aligned
  cell, a row's dependency line in small muted words under its name,
  the list's name shown before its items, and the budget's card as the
  mockup's side card, its terms and descriptions as its key and value
  rows. `scripts/bundle.sh` passes the three to `skein-ui-bundle` in
  that order.
- **The navigation** sits in the banner after the projects, as in the
  mockups. At phone width, 760 px and under as in the mockups, the
  banner keeps temper's name and the view's "Menu" alone (`README.md`,
  5.1): its summary a ☰, drawn as a mask over its word, and its
  content the projects, the navigation and the corner (the link's
  state, the person, "Sign out") in a panel that opens under the
  banner's end, as `terminal-chat.css` draws the mockups' menu. Theirs
  holds the navigation alone, which leaves a phone no way to choose a
  project or sign out; temper's holds the corner and the projects too.
  Wider, `temper.css` draws the menu's content in the banner whether
  it is open or not, and its summary not at all, where the browser
  lets a closed disclosure's content be drawn (`::details-content`);
  elsewhere the ☰ is drawn at every width. Choosing an entry folds the
  menu: skein's page folds its open disclosures when its address moves
  (skein's `ui-web.md`, section 7).
- **Light only:** the mockups' palette is light, and `temper.css` sets
  `color-scheme: light`, where `skein-ui.css` follows the system's dark
  preference. A dark palette is open (section 5).
- **The composers** (the chats' and a chat's) are text areas with
  skein-ui's `SubmitsOnEnter`: Enter sends, Shift+Enter is a new line
  (skein's `ui-web.md`, section 7), which the view allows in a
  composer's words, counted within its bound (`README.md`, 5.1). A
  chat's shows its placeholder, "Message T6…", while it is empty, as
  `placeholder`, its name staying "Message" (`README.md`, 3.5). They
  grow with their words and shrink after sending by `temper.css` alone
  (`field-sizing: content`, from one line to a bound), with no
  component and no script of temper's; a browser without it shows an
  area of fixed height that scrolls.
- **A run's live text** is a patch like any other: the chat's
  provisional turn ("Writing…", `README.md`, 3.5) changes in place;
  skein's page keeps focus, caret and scroll across it, and the
  conversation, a log, keeps its end in view while the person has not
  scrolled away (skein's `ui-web.md`, section 7).

### 2.6 The bundle in the binary

The `temper` binary holds its bundle, so that `temper box` and `temper
serve` serve the browser with nothing beside the binary, and `cargo
build` in a checkout makes that binary. It is a person's convenience:
the machine's builds keep building the way that serves their work,
without it.

- **One file.** The binary holds the bundle: the shell's module, its
  glue, and the stylesheets and files skein's bundler puts beside them
  (2.2, 2.5). Running it is the whole install: no directory beside it,
  no `scripts/bundle.sh` step.
- **A plain `cargo build` makes it.** `cargo build`, then
  `./target/debug/temper box`, serves the browser; so do `cargo build
  --release` and `cargo install --git` of temper's repository.
- **A crate of its own embeds it,** `temper-client-bundle`, ordinary
  Rust with its own `[lints]` table, as tools have, which `temper`
  depends on. Its library is data: the bundle's files, each its path
  from the bundle's root (`index.html`, then `<digest>/…`) and its
  bytes by `include_bytes!`, in a directory read's order, and the
  digest, both written by its build script into `OUT_DIR`. A build
  without the embed has no files and no digest. The include sits in a
  small crate, not in `temper`'s, because a build script that runs
  makes its package build again, and `temper`'s is the slowest to
  build; `main.rs` gets no package of its own, since `temper` builds
  again after a client edit anyway (the terminal shell shares the
  client's crates), and its integration tests run the binary by
  `CARGO_BIN_EXE_temper`.
- **Its build script,** unless `TEMPER_BUNDLE` is `none`, takes
  `scripts/bundle.sh`'s steps with the cargo running it (`CARGO`), from
  the workspace's root, into the nested build's directory (below):
  1. `build --locked --target wasm32-unknown-unknown -p
     temper-client-shell-web`, with the module's profile (below);
  2. `build --locked --release -p skein-ui-bundle`: the bundler,
     always optimized, as skein's `ui-web.md`, 4.1 asks of a tool;
  3. the bundler, with `scripts/bundle.sh`'s inputs (the name, the
     prefix `/bundle/`, `temper.css`, `task-pages.css`, the shell's
     `files/` and the noscript words), into its `OUT_DIR`;
  4. the list of the bundle's files, written into `OUT_DIR` for the
     library's include.
  It has no build dependencies: the bundler is built by the nested
  cargo and run, never compiled into the script, so a build without the
  embed compiles a script of `std` alone, and nothing of the bundler is
  ever linked into `temper` (skein's `ui.md`: "never linked into a
  service or a module"). One `Cargo.lock` still locks the bundler's
  `wasm-bindgen-cli-support` with the module's macros (skein's
  `ui-web.md`, 4.1). The script's logic (the dep-info's read, the
  nested commands, the bundle's directory read into its list) is plain
  functions, which the crate's tests take on samples.
- **The nested build's directory** is `<target>/bundle/`, `<target>`
  the outer build's, found as the nearest ancestor of `OUT_DIR` holding
  cargo's `CACHEDIR.TAG`. The outer cargo holds its own directory's
  lock while build scripts run, so a nested build there would wait on
  it forever. It is one directory, not one under each `OUT_DIR`, so
  that a check, a clippy run, a build and a test build of `temper`
  reuse one module and one bundler, and a build after a change is
  incremental. Under `cargo clippy`, cargo builds a member's build
  script apart, so the first clippy run and the first build after a
  change each bundle once; the second finds the module built. A build
  without the embed never makes the directory.
- **None of the outer build's flags or wrappers:** the nested cargo
  runs without `CARGO_ENCODED_RUSTFLAGS` and `RUSTFLAGS` (the host's
  flags, not wasm32's), `RUSTC_WORKSPACE_WRAPPER` and `CLIPPY_ARGS`
  (which `cargo clippy` sets, and which would make a module of another
  fingerprint, built again at each switch between clippy and build),
  with `CARGO_TARGET_DIR` set anew. It keeps the jobserver
  (`CARGO_MAKEFLAGS`), so that its jobs count within the outer build's.
  Its stdout goes to the script's stderr, since cargo reads the
  script's stdout.
- **Rebuilt only when what it is made from changes,** each by
  `cargo:rerun-if-changed`: each source file the nested build's dep-info
  names (the `.d` cargo writes beside the module: every path package
  the module is built from, temper's client crates and, when a local
  jig or skein is patched in, theirs); each of those packages'
  `Cargo.toml`, the workspace's `Cargo.toml` and `Cargo.lock`, which
  pins the git dependencies; the shell's stylesheets and its `files/`;
  and `TEMPER_BUNDLE`, by `cargo:rerun-if-env-changed`. The first run
  has no dep-info yet, and runs regardless. An edit elsewhere never
  runs the script, and a build that does not include `temper` (a
  domain's tests, the client crates' own) never runs it.
- **The module's profile:** a dev build embeds a dev module, as
  `scripts/bundle.sh` makes; a release build embeds one built with
  `web`, the profile skein's `ui-web.md`, 4.1 names for an
  application's release module, which temper's workspace defines as
  skein's does. The script maps cargo's `PROFILE`, `debug` or
  `release`, to `dev` and `web`.
- **`TEMPER_BUNDLE`:** unset or `embed`, the embed; `none`, no nested
  build and no bundle; any other value fails the build, naming the two.
  `embed` is for a build whose environment says `none` and wants the
  browser this once. The machine's builds set `none`: the gate's and
  the agents' (`docs/development/workflow.md`), a machine without the
  target, an editor's own (rust-analyzer runs build scripts at its
  start). Such a binary serves the page for no bundle (2.2).
- **The embed needs the wasm32 target.** Before any nested build, the
  script asks rustc for the target's library directory
  (`--print target-libdir`); without it, the build fails with one
  message: `temper-client-bundle: the browser's bundle needs the
  wasm32-unknown-unknown target: add it (rustup target add
  wasm32-unknown-unknown), or build without the browser
  (TEMPER_BUNDLE=none)`. Never a binary without its browser in
  silence. temper ships no `rust-toolchain.toml` naming the target: it
  would set the channel for every rustup user, which temper does not
  set (`rust-version` alone).
- **Offline:** the nested build may download crates only wasm32 uses,
  as any first build of the target does; an offline build runs `cargo
  fetch` first, which fetches for every target.
- **Served as a directory's is.** At start, `application.clients.bundle`
  `null` takes the binary's bundle: its files, already in the binary,
  are made into `temper-service`'s `Bundle` (its page, its digest, its
  files by path with their types from the closed list, its `ETag`) by
  the same code as a directory's read, given files in memory in place
  of a directory, so the same checks hold: the closed list, each path's
  bound, one digest directory, and `clients.bundle_files` and
  `clients.bundle_bytes`, a start past them refused naming the limit
  (`application.clients.bundle: the binary's bundle: <path>: <why>`).
  The copy is held as a read one is, within what the profile's worst
  case already counts for the bundle, so the memory account does not
  change. `temper-service` adopts a bundle, wherever the binary got it.
  A directory configured is read as before, and the binary's is unused.
- **The fake-backend bundle is provisioned separately.** Build it with
  scripts/bundle.sh --loopback OUT and name OUT in application.clients.bundle
  before starting the fake-backend box (../protocol/configuration.md, 12.1).
  The ordinary embedded bundle remains loopback-free and is not used for
  that box. Missing provision is a startup refusal explaining the command
  and configuration member. Startup never builds or downloads the bundle.
  The second-embed prototype added 8,974,112 bytes in the measured dev build,
  exceeding its 5 MB limit; the provisioned path keeps that cost outside
  ordinary builds. TEMPER_BUNDLE=none runs no nested browser build.
- **`scripts/bundle.sh` stays,** for a bundle built apart: the browser
  tier's, which turns on `loopback-navigation` (4.3), and a
  developer's. The ordinary binary bundle never enables it. `scripts/install.sh PREFIX`
  installs the binary alone, built with `--release --locked`, and
  prints its path.
- **Tested in both builds.** Every suite passes with and without the
  embed, and the gate runs them under `none`. The build script's logic
  is tested on samples in its crate; the in-memory read on fixture
  files beside the directory's read, the same `Bundle` and the same
  refusals; and a start with `bundle` `null` expects what its build
  holds, which `temper`'s crate says (`temper::bundle::OWN`, the
  binary's digest or none): the page for no bundle under `none`,
  otherwise the bundle's page at `/` and its files under
  `/bundle/<digest>/` (`../domain/testing.md`, 2.6). The page for no
  bundle stays proved where it is built, in `temper-service`. The embed
  itself is a person's build, which fails loudly when its steps do; a
  change to it checks it with `TEMPER_BUNDLE=embed heavy cargo nextest
  run -p temper`, which the gate does not run.

## 4. The browser tier

- **The driver is skein's.** `skein-browser` (skein's `browser.md`) runs
  headless Chromium as a child and drives it over the DevTools protocol
  on a pipe, from the loop beside the engine.
- **The person's browser face is skein's** (`skein-fake-person-browser`;
  skein's `ui-web.md`, 14.2): the same queries and acts as on the tree,
  made over Chromium's accessibility tree, and the browser's own: the
  current address, read from the navigation history, and keys pressed
  with Shift.
- **What it runs against:** `temper serve`, a child of the harness's
  loop, on a scratch directory, from a configuration the test writes
  with the bundle named: no Forgejo, no runner. The first milestone's
  stories need no answer of the forge's or the LLM's, so its forge and
  its LLM endpoint are unreached, on closed loopback ports, as jig's
  tier serves (jig's `protocol/browser.md`, section 15). The fake
  forge, hosted on loopback in that loop by skein's forge peer
  (`../domain/testing.md`, 4.2), and skein's fake LLM peer, as the
  process tier has them (`../domain/testing.md`, 2.6), join with the
  second milestone's first story that needs them (4.2). The one
  exception is 4.4's test, for which the forge peer joins earlier
  serving only the fake's GitHub `web`. skein's fake issuer joins for
  `ConnectModel`'s round trip alone (4.2).
- **Only what a browser alone shows.** temper's stories (what a page
  shows, what an act sends, a chat answered, a proposal accepted, a
  repository adopted, a run's live text) are the joint world's
  (`tests/client`; `../domain/testing.md`, 6.3) and the real loop's, through
  the terminal shell (`../domain/testing.md`, 2.4); they never run again in a
  browser. A story is in this tier only when no world can show it: the
  bundle, the browser's platform (the address, history, the cookie,
  keys) or an attack that needs a browser. Each names what only the
  browser shows. What skein's browser parts prove for every application
  (the mapping of roles, a patch keeping focus, caret and scroll, the
  store, the bundler) and what jig's prove for every client of jig's
  (jig's `protocol/browser.md`, section 15) are not proved again.
- **Every story checks the policy too:** skein's harness fails a test
  on a policy violation, an exception or a native dialog (skein's
  `ui-web.md`, 14.2).

### 4.1 The first milestone's stories

1. **Signing in by the printed link:** the welcome shows, at
   `/welcome`, or, where the first run is over, the chats, the
   address reading `/chats` once they do (2.4); a reload on a chat's
   address shows that chat, signed in by the cookie; a link spent shows
   the browser's signed-out paragraph (2.4). That the secret leaves the
   address before the first request is held by the shell's start order
   (2.1), and is not tested.
2. **Addresses:** skein's `back_and_forward` (skein's `ui-web.md`,
   14.2), from the chats to a chat by its link and back; an address
   that is no page opens the inbox, and the address reads `/` once it
   shows (2.3).
3. **The composer:** in the chats, Enter starts a chat with the words
   once, and Shift+Enter makes a new line in them (2.5).
4. **A panic:** skein's `panic` (skein's `ui-web.md`, 14.2), which
   checks the shell's wiring of the restart part, at the test address
   `/test/panic` at which the shell panics at start, as jig's reference
   shell has it (2.1).
5. **jig's attack stories that need a browser,** from
   `jig-client-fake-person-browser`, against `temper serve`: a
   cross-site request from another port and from another host does
   nothing; a framed page shows nothing (jig's `protocol/browser.md`,
   section 15). The rebound Host, which needs no browser, stays in the
   process tier (`README.md`, section 10).

Not in the tier: skein's `typing_while_patched`, which takes a second
person, and `write_refused`, which takes a test build's bounded small
store; what they check is skein's page and store, which skein's tier
proves.

### 4.2 Later pages

A page gets a story here only for what of its own a browser alone
shows. The inbox, a task's page and the adoption form have none beyond
their addresses: one story opens each present page's address directly
and reads its heading. The task pages' story runs on `temper box`'s demo,
with its own fakes, from a procedure's plan to its agent and back by
the lineage bar, each by its address, and opens the agent's page fresh
by its address. The procedure is the demo's change, drawn from its
panel (`README.md`, 5.10); the story reads its heading and its plan's
agent, no more of it. The change's steps to landed, its gates, its
bounds and its ended tasks hold nothing a browser alone shows: the
client world's `change_panel.rs` reads them. A later page that has such
behaviour brings its story with it: `ConnectModel`'s sign-in, its tab
sent to the provider's address and back to the page it left
(`README.md`, 3.5).

**`ConnectModel`'s sign-in, the round trip.** What only a browser
shows: the page's own tab leaving for the provider's address, the
provider's redirect followed to the engine's listener, its `303` back
to the page, and the page come back, a new client, drawing the end.
- **Against:** `temper serve` with its OpenAI provider's issuer, its
  authorization, token and key addresses, skein's fake issuer
  (`skein_fake_peers::oauth::Peer`), hosted on loopback in the
  harness's loop, its one grant a success for a synthetic subject, and
  the provider's redirect port the kernel's (the listener's port
  varies). The fake has no TLS, so the tier's bundle is built with
  skein-ui-web's `loopback-navigation` (4.3).
- **The story:** a person on `ConnectModel`'s Choose presses "Continue
  with ChatGPT"; the browser follows the issuer's redirect and the
  listener's `303` alone, and the address reads
  `/accounts/connect?provider=1&way=sign-in&source=S`; the page shows
  Ready ("Signed in to ChatGPT"); the fake has taken one authorization
  and one code exchange. A reload then shows Ready again and the issuer
  has taken no second authorization: the departure is once.
- **Not here:** the departure's order, `Return` before `Leave`, its
  refusal and the terminal's root without one are the client world's
  (`tests/client`); the path's check, the engine's; the `303`, its
  absence and a return for no sign-in, skein's listener tier.

**`ConnectForge`'s App of your own, the pasted key,** has no story
here. Its field is a text field of the secret kind, which skein's page
maps to `<input type="password">` (skein's `ui-web.md`), its value read
as any text field's; HTML's value sanitization strips a password
input's line feeds, the one thing a browser alone would add, and the
client restores the PEM's armour from what is left (`forge.md`, 5.4).
The view's tests hold that the field is a text field of the secret
kind; the client domain's, that the stripped, the file's and the CRLF
forms reach one carried value with the armour's two line feeds, and
that the field is emptied; the service's, that `Paste` reads the fake's
real key in that restored form.

### 4.3 Its budget and the gate

- **A suite of its own,** outside the focused and fuzzy suites
  (`../domain/testing.md`, section 2), in its own crate
  (`tests/client-browser`, `temper-client-browser-world`), run by
  temper's gate with a nextest profile `browser`, one test at a time,
  on the bundle the gate builds once, with the dev profile.
- **The bundle the gate builds** turns on skein-ui-web's
  `loopback-navigation` (`scripts/bundle.sh --loopback`), so that the
  round trip (4.2) can send its tab to the fake issuer on plaintext
  loopback; the ordinary binary bundle never does (skein's `ui-web.md`,
  section 10). The tier names that bundle in its configuration, so it
  runs alike over a binary built with or without its own (2.6).
- **A guide: at most 12 tests, and about 120 seconds** (60 seconds before
  the user's doubling of 10-22, provisional) on the slowest of the machines
  that run temper's gate. Past it, a story moves to a world or goes, and the
  budget stays.
- **Chromium** is the machine's, at the path its owner configures
  (skein's `browser.md`), which skein's and jig's gates read from
  `SKEIN_TEST_CHROMIUM`, and temper's gate too. A missing Chromium or
  bundle fails the suite, and never skips it (skein's `ui-web.md`,
  14.1).
- **The client's crates for wasm32:** the gate runs clippy for
  `wasm32-unknown-unknown` over `temper-client-domain`,
  `temper-client-view`, `temper-client-protocol-wire` and
  `temper-client-shell-web`, so a change that stops them building for
  the browser fails the gate.

### 4.4 GitHub's manifest page

One test, the github plan's, counted in 4.3's guide: with the ten
tests 4.1's stories take and 4.2's story, the tier reaches its 12.

- **What only a browser shows:** the manifest page's form posted under
  the page's own policy, `form-action WEB` (`forge.md`, 9.1). The
  redirect and the setup address need no browser; the real loop's
  GitHub stories follow all three steps as plain requests
  (`../domain/testing.md`, 2.4).
- **What it runs against:** `temper serve` with a GitHub forge whose
  `api` and `web` are the fake's GitHub layer, hosted on loopback in the
  harness's loop by skein's forge peer (skein's `fake-forge.md`, section
  10), which joins the tier for this test alone, ahead of the second
  milestone (section 4): the test needs only its `web`. A GitHub forge needs a fixed
  clients' listener (`forge.md`, 9.1), so the tier reserves a loopback
  port the kernel assigns before `temper serve` starts and names it in
  the configuration.
- **The test:** GitHub's Connect asked through `temper ctl`, which
  prints the manifest page's link (`tools.md`); the browser opens
  `/forges/N/github/manifest?state=S`, which posts its form as it
  loads, with nothing pressed. The form posts to
  `WEB/settings/apps/new?state=S`, the fake's. The test
  ends once the fake's GitHub layer has taken the post, its `manifest`
  field the page's and its state S, with no policy violation. The post
  is answered `200` with a page holding one link, `Create GitHub App`
  (id `create`); the test does not follow it, and the later steps are
  the real loop's (`../domain/testing.md`, 2.4).
- **No github.com:** `web` is the fake's, and the page's policy lets
  its form post nowhere else. No test of the tier holds a real
  credential.

### 4.5 Forgejo's authorization

One test, the provider's (`../protocol/sign-in.md`, 9.8).

- **What only a browser shows:** jig's strict cookie, absent from a
  navigation that started on another site and present on the page's own
  exchanges, so that the `Authorize` place answers *go* for a person
  signed in.
- **What it runs against:** `temper serve` with jig's provider, its key
  and its client's secret made in the test's scratch directory, and one
  client whose redirect URI is a page of the harness's on the other
  loopback site (`localhost` against `127.0.0.1`), hosted in the
  harness's loop: a link to temper's `/oauth/authorize` with that
  client, and the callback, which records its query. No Forgejo: the
  process tier's story has the real one (`../domain/testing.md`, 2.6).
- **The test:** signed in by the printed link, the browser opens the
  harness's page and follows its link; temper's page sends `Authorize`,
  and the browser's address becomes the callback's, its `code` and the
  link's `state` there. Nothing of the token's is the browser's.

## 5. Open questions

- **Kept work in the browser** (`README.md`, 3.4 and 6.6): another
  party's kept work, and kept work dropped at start, which the terminal
  says in lines of its own. Default: not shown; the small store lives
  as long as its tab, of one person.
- **A bundle built apart, embedded:** `TEMPER_BUNDLE=DIR`, a bundle's
  directory embedded in place of building one, so that one wasm32
  build serves binaries for several hosts and a machine without the
  target can still make a whole binary. Default: none; it waits for
  the release pipeline (2.6).
- **Served from the binary's bytes,** with no copy at start: a `Bundle`
  that borrows. Default: copied (2.6), until a bundle whose copy
  matters.
- **The embed's steps, skein's for every application:** the nested
  build, its directory, its environment and what reruns it are nothing
  of temper's. Default: temper's crate, until a second application
  embeds its bundle (jig's reference application, or skein's example),
  when they move to a small crate of skein's that a build script
  depends on (skein's `ui-web.md`, section 15).
- **A binary to download:** where release binaries are built, for
  which targets and C library (a static musl build, or a floor of
  glibc), and where they are published. Default: none; temper's
  binary is whole (2.6), not shipped.
- **A switch to serve no browser** from a binary that has one, such as
  `clients.bundle` `false`. Default: none until a deployment asks: the
  browser is reached only by a printed link, as the terminal is.
- **A dark palette,** following the system's preference as
  `skein-ui.css` does. Default: light only (2.5).
- **Other browsers** than Chromium are `skein-browser`'s question
  (skein's `browser.md`).
- **The kit's stylesheet without a bundle:** the gallery takes
  `skein-ui.css` by running skein's bundler with no module (7.1).
  Default: so, until skein's bundler offers the stylesheet alone.

## 6. In two milestones

- **The first: one path end to end, sign-in, the chats, a chat.**
  Everything every page shares: the shell (2.1), serving and the policy
  (2.2), the address codec with every present page's path (2.3),
  signing in (2.4), the frame's look and the chats' and a chat's (2.5),
  the bundle and the gate's checks (4.3). A chat's own, drawn by
  `temper.css`: the composer's keys and growth, a run's live text, a
  proposal accepted or rejected in place. The tier's stories of 4.1.
  Upstream, in skein, before them: a click inside a button or a menu
  item finding it (skein's `ui-web.md`, section 6), since temper's pages
  click buttons; `back_and_forward` awaiting a page that holds its
  server's state before its first move (skein's `ui-web.md`, 14.2),
  since the first page signs in by the printed link; and the fake
  person reading the current address and pressing a key with Shift
  (skein's `ui-web.md`, 14.2). Done when temper's gate passes 4.1's
  stories.
- **The second: the other pages.** The inbox, a task's page (a goal's
  and an agent's task's) and the adoption form, each with its look
  (2.5), and 4.2's story; the forge peer and the fake LLM peer join the
  tier with its first story that needs them (4; the forge peer
  forge-kit's). Done when temper's gate passes it.
- **What it touches in temper:** `temper-service`'s clients' server and
  its configuration and limits (2.2); `temper-client-domain`'s address
  codec (2.3); the view's style tokens, its composers' `SubmitsOnEnter`
  and its signed-out words (2.4, 2.5); the new shell and the tier.

## 7. The gallery

`temper-client-gallery` (`tools/gallery`), a program of temper's
workspace, prints the browser's pages in named states into a
directory, as static documents, for screenshots beside the mockups
(`../draft/qa-web.md`, 3.2) and a person's eye. It is a tool, not a test of the pages: what
the pages do is the worlds' (section 4), and the gallery draws what the
view draws, never changing a view to suit a sample.

### 7.1 What it prints

- **One command:** `cargo run -p temper-client-gallery -- OUT`. It
  reads no configuration, environment, clock or network, and starts no
  server.
  `OUT` is replaced whole when it holds the gallery's `index.html`,
  made when it is missing, and refused otherwise.
- **Each state's page** as `OUT/NAME.html`: the view's tree for the
  state, made a page by skein's `Page` with the shell's look (`Look`
  with `look::NAMES`, primary input `Control`) and its prefix, its
  links given the hrefs the shell gives them (2.3), by the view's one
  href function, which the shell's host calls too. The page's root is
  the document's `#app`, as skein's `Dom` mounts it; dialogs carry
  `open`, since a static document has no top layer. The title is the
  shell's: the page's heading, then "temper".
- **The bundle's stylesheets and files,** under `OUT/bundle/`, made by
  skein's bundler from the stylesheets and files `scripts/bundle.sh`
  passes, in its order (2.5), with no module: each page links them as
  the bundle's page does, so `temper.css`'s files resolve as they do
  in the bundle. The bundle's own page and its empty module are unused.
- **An index,** `OUT/index.html`: each state's name, in the gallery's
  order, linking its page, with its mockup's address beside it where a
  mockup draws it.
- **Static:** no script runs, so focus, caret, scroll and a run's live
  text are not shown; what only the live page shows is a live scene's
  (`../draft/qa-web.md`, 3.3).

### 7.2 Its states

- **A state** is a world at a page's address, then what is done there:
  the client's root stepped from start by the documents a world
  delivers, then by the person's acts or the engine's answers that the
  state names (a close pressed, a reject's reason box, a refusal), as a
  page would be. No page state is set by hand.
- **The worlds are the mockups':** reference, solo and the first run at
  its steps, their words copied from `mockups/world-*.js` where temper's
  model holds them, so that a picture differs from its mockup by look,
  not content. A value the model has no place for is left out, never
  invented. The gallery's own documents carry what the domain's
  fixtures don't: the providers, accounts and their holders by name.
  A spend is the contract's whole number (jig's owner numbers), so a
  mockup's fraction is rounded to the nearest (0.8 is 1) until the
  contract holds one.
- **Its time** is each world's, fixed: the mockup's now, its
  `world.now`, at which the view builds, and each record's time the
  mockup's offset from it, so that a time reads as its mockup's
  ("12 min ago"). The program still reads no clock (7.1).
- **Its bounds** are the browser's shipped client limits as they are
  (`temper-client-shell-terminal`'s `native()`, which the browser's
  shell takes), never widened, and the view's bounds are derived from
  them as the shell derives its own: a page shows what a person's
  browser would, a window's rows and a chat's lines included (`README.md`,
  5.4), so that its outline is the live page's (`../draft/qa-web.md`,
  8.2). A world holds more than a window shows; the page shows the
  window, and the state's test row says what it shows.
- **Names,** binding once landed: a state a mockup draws is named by
  its address, `<page>-<world>[-<step>][-<task>]` (`chat-solo-T10` for
  `chat.html?world=solo&task=T10`, `chats-first-run-1` for
  `chats.html?world=first-run&step=1`); one no mockup draws takes a
  plain name (`signed-out`, `chats-empty`, `adoption-refused`). A name
  is never reused for another state, and one leaves only with the
  scenes that capture it.

| Name | The state |
|---|---|
| `chats-solo` | the solo world's chats |
| `chat-solo-T10` | the solo world's chat T10, its proposal in place |
| `inbox-solo` | the solo world's inbox: a proposal, a question |
| `task-solo-T3` | the solo world's goal T3, its plan and budget |
| `accounts-solo` | the solo world's accounts |
| `chats-first-run-1` | the first run's first screen: the demo's start |
| `welcome-first-run-0` | the first run's welcome: its two cards |
| `connect-forge-first-run-6` | the first run's `ConnectForge` at step 6: Forgejo here setting up, the box preparing |
| `forges-first-run-7` | the first run's Forges at step 7: Forgejo here connected |
| `welcome-forge-choose` | the welcome's Forge choices, before any forge is connected |
| `welcome-forge-details` | the welcome's Forgejo Details, with organization and repository paired |
| `welcome-model-choose` | the welcome's Model choices, with ChatGPT sign-in and an API key |
| `connect-forge-choose` | `ConnectForge`'s Choose: GitHub, Forgejo here and a Forgejo elsewhere |
| `chats-setup-strip` | the chats with no demo, no usable account and no forge: the setup strip |
| `adoption` | the adoption form, empty |
| `adoption-filled` | the adoption form, filled in |
| `adoption-refused` | the adoption form after a refusal, its alert |
| `chats-reference` | the reference world's chats |
| `chat-reference-T1` | the reference world's chat T1 |
| `chat-solo-T6` | the solo world's chat T6 |
| `chat-first-run-2-T1` | the first run's chat T1 at step 2, its change running |
| `chat-first-run-3-T1` | the first run's chat T1 at step 3, its change landed |
| `chat-first-run-4-T4` | the first run's chat T4 at step 4, ended |
| `inbox-reference` | the reference world's inbox |
| `accounts-reference` | the reference world's accounts |
| `accounts-first-run-1` | the first run's accounts at step 1: the demo's key |
| `signed-out` | signed out |
| `out-of-date` | a bundle older than the engine |
| `chats-empty` | the chats with none, outside the demo |
| `inbox-empty` | nothing needs the person |
| `chat-closing` | `chat-solo-T6` with Close pressed: its confirmation |
| `chat-rejecting` | `chat-solo-T10` with Reject pressed: the reason's box |
| `task-held` | a goal's plan with a held task, Release offered |
| `task-releasing` | `task-held` with Release pressed: its confirmation |
| `connect-model-choose` | the offered ways and the owner's holder choice |
| `connect-model-key-details` | a key's secret field and Add key |
| `connect-model-program-details` | a program's name, command and Declare |
| `connect-model-signing-in` | a sign-in started, its address an external link |
| `connect-model-key-ready` | a key kept, Ready with Start a chat |
| `connect-model-signin-ready` | a sign-in made an account, Ready |
| `connect-model-declined` | plan usage declined, with another sign-in and a key way |
| `connect-model-renewed` | the same account again, with another sign-in |
| `connect-model-refused` | another holder's account refused, with another sign-in |
| `connect-model-key-lost` | a key request's answer lost, its alert asking for the value again |
| `task-reference-T28` | the reference world's change T28, queued 2nd after T26 |
| `task-reference-T24` | the reference world's change T24, landed, its ended task T25 |
| `task-reference-T12` | the reference world's goal T12, its plan with its ended delegates T13, T14 and T24 |
| `task-solo-T9` | the solo world's agent T9, its run's committed calls and words within the shipped window |

### 7.3 Its tests

In `tools/gallery`'s own tests, in the view's world: each state's tree
read by skein's fake person (`../domain/testing.md`, 4.4), as the
gallery builds it.

- **The names:** the states' names, in order, are `tools/gallery/names.txt`,
  kept by hand, so a name gone, changed or added fails until the file
  says so; the scenes read the same file.
- **Each state holds what its name says:** a table of rows, each a
  name, its page's heading and the one part the state is about, found
  by role and name (the proposal in `chat-solo-T10`, the three
  suggested requests in `chats-first-run-1`, the alert in
  `adoption-refused`).
- **The print:** the program run into a scratch directory writes every
  name's page, the index linking each, and the bundle's stylesheets in
  `scripts/bundle.sh`'s order, read from that script.
- **Their cost:** together at most 1 s (0.5 s before the user's doubling of
  10-22, provisional) of the default suite; no fuzzy test.