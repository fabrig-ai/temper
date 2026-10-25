# The mockups' status

Revised 2026-10-25. How far temper's client is from its mockups
(`README.md`), a group of pages at a time. A group under the rules has
a row for each thing its main drawing shows that the client doesn't
draw yet (a lag), and for each place the client does otherwise (a
divergence); the rest of its main drawing is built. Its future pieces
are marked in the drawing, each with its need, and not repeated here.
Section 2 lists the pages not under the rules yet, with the notes the
README held for them.

## 1. Under the rules

### 1.1 The welcome journey

`welcome.html` and `github.html`, at the states their worlds list:
`world-first-run.js` at step 0, `world-first-run-real.js`,
`world-first-run-github.js` and `world-first-run-elsewhere.js`. Audited
2026-10-25, at temper 3273eae0d (first-run-ux-6's walk of its 64 rows,
desktop and phone, and its fixes); under the rules since 2026-10-25,
with the user. The Chats, Forges, Accounts and
Connect pages the journey passes through are other groups'.

What the client doesn't draw yet:

- **GitHub's target details:** Model and Ready use the durable adoption,
  and the asking tab names its new project. The durable row does not
  retain whether the project was new, so after reload Model says
  "in PROJECT". No repository web address reaches the client for
  Ready's "Open on GitHub ↗".
- **A sign-in's port taken on Ready:** declined, held and misconfigured
  now draw their words and acts. "Try later" still needs the listener's
  port-taken fact in the client (M-j); `OwnerAccounts.last_end` does
  not carry it.

Where the client does otherwise:

- **Ready opened before a GitHub repository is chosen,** at
  `/welcome?at=ready`, draws "Almost ready" with its forge and model
  lines, as the client README's 3.5 allows; the mockup goes back to
  the picker.

Not yet known: whether GitHub keeps the state through its
installation's Configure and Save, and whether the picker reads the
repositories again on the way back (`../forge.md`, 9.1).

## 2. Not under the rules yet

Every page. What one draws may need what the engine lacks, marked
`ahead` or `gap` or not at all, and the client documents' passages
comparing a page with its mockup are disagreements still to settle.
The notes below are the README's, as they stood on 2026-10-24, until
their group comes under the rules.

### 2.1 Where each value comes from

What a task waits on is jig's, computed by the engine: a party, its delegates, news. Where it is news, the page narrows it from the task's wake policy, mechanically: *your words* when a party's words wake a task that party requested (a chat at rest), *T12's end* for a subscription to another task's state, *until 09:05* for a timer alone. *Waiting on your words* is not *needs you*: nothing is held up, and only an entry (a question, a proposal, an escalation, a person task) is in the inbox, where its task waits on *an answer from you*. temper's client words these statuses its own way ("Waiting for you", "Parked"; the client README, 3.5).

Anything else on a page is the UI's own words, and those name no workflow: the director, an owner or a check appear only in what was written or configured. The inbox, for example, says "Nothing needs you." and stops; "Next check at 09:10" appears only where the director wrote it, in its reply. Its updates are derived, as jig's inbox derives them: replies in the person's chats, and results of the tasks the person requested.

### 2.2 Drawn ahead of temper

What temper does not have yet, drawn as temper-on-temper.md's section 6 wants it and marked with its gap while **Annotations** is on:

- **6.3, plan usage:** a program's 5-hour and weekly windows on the accounts page, the week with its pace; the director's check reads them. A ChatGPT plan shows none: its usage is ChatGPT's settings' to show (jig's `domain/providers.md`, 2.6), so its row shows its limit's state and "Manage usage ↗", which opens them.
- **6.4, worker health:** the ops strip on the workers page, an ops application's report of load and disk.
- **6.5, open findings:** a finding on a goal's page, the board and the changes page, where a failure that matches an open finding attaches to it and leaves the queue running.
- **6.7, fleet occupancy:** a worker's free slots, and the director's read of them.

Marked "ahead" (slate, with the annotations): what the owned Forgejo’s rows and the first run draw before temper has a mission for it. **B4 M4:** the set-up’s first repository, and "New repository", made by temper with the admin’s token once temper-15 moves that token into the engine. **demo’s end:** retirement after the first personal forge connects, with no mission yet. **sign-in:** "Open Forgejo" signing the person in through temper, as Forgejo’s OpenID provider (`../../draft/sign-in.md`, section 5); until then, the person signs in with the password the preparation wrote. **display:** an account's email, "ChatGPT · sam@example.com"; jig carries no display yet, and the client README's 5.5 drops it from the heading until it does. **words:** a sign-in's endings that 5.5 has no words for: a subject another person holds, and the port taken. **Google:** signing in to temper with Google, through jig's `Mint`, and the return route's pages (`../../draft/sign-in.md`, section 4). **provider:** temper as Forgejo's OpenID provider: its authorization, and "Open Forgejo" on a managed Forgejo that signs in through temper (section 5). **sign-out:** signing out of temper leaves Forgejo signed in (section 6). **sketch:** Claude's subscription in Connect a model, in outline. **limited chat:** where a chat shows its account's limit.

Two pages follow jig's pages, which temper's client leaves out (the client README, 5.3 and 11): **Tasks**, every task of the project as one tree (jig's `client/domain/pages.md`, 6.3), and **Workers**, the deployment's hosts (7).

Not drawn: two ideas that would change temper's engine, not only its pages. A charter or template would name the pages where its tasks lead, so the board could open with the director's line; an agent would set a short status line on its task with a tool, shown as written beside its phase. They wait for a design of their own.

### 2.3 The first run

`world-first-run.js` is the deployment `temper init` writes (`../../protocol/setup.md`, 8.2 and 8.3): its project 1, "Project 1", which View → Future draws as no project, made by the playground's start or the first repository (`../../draft/first-run.md`, 2.4), the charters `chat` and `coding`, one worker in temper's process, three forges (the demo's fake, GitHub not connected, a Forgejo here not set up), and no account, repository or forge started yet. Every word an agent says is the demo's script (8.4). [first-run.html](first-run.html), the address the documents cite, opens it at its first step. Its steps:

- **0. The link opened:** the welcome offers Try the playground or Set up temper (section 1.1). Chats behind it has the setup strip, no model and a disabled send button.
- **1. The demo chosen:** the engine asks temper box for the demo’s fakes, makes the project Playground, adds the Scripted model’s account and adopts playground/widget into it. The chats page is the demo's start (client README, 5.6): the banner under the navigation, as on every page; a chatbar with no task and no number; the context line; the three suggested requests.
- **2. A request sent:** "Add CSV export to widget" starts the chat T1; its agent delegates the change T2, whose agent T3 edits `src/main.rs`. T2's card says Running with no step ticked (5.7).
- **3. The change landed:** T2's card says Landed, every step ticked, "CI passed. Landed · merge commit e3b8d41"; T1 says so in its script's third turn.
- **4. A text-only request:** "Add a dark theme to widget", the chat T4, answered in words; its script's `finish` ends only the turn (setup.md 8.4), so T4 waits at rest for Sam's next words, its composer open, as T1 does.
- **4a. Connect a model, from another computer:** `step=model-choose` draws Choose, with Continue with ChatGPT and the secondary button “This browser isn't on temper's computer ›”.
- **4b. Signing in, with the browser elsewhere:** `step=model-elsewhere`, `way=chatgpt`, draws Signing in, its external sign-in link in another tab and the paste unfolded. Accounts draws that sign-in with the paste folded.
- **4c. The whole address pasted:** `step=model-pasted`, `way=chatgpt`, draws the secret field written, hidden, and Finish connecting enabled; the button leads to step 5. Accounts has the same paste, folded. No address is sent or stored by a mockup.
- **5. A model connected:** Sam follows the banner's Connect a model, chooses ChatGPT and finishes by pasting from the browser on another computer; the flow ends on Ready, with Start a chat and Manage usage. The Scripted model leaves Accounts, where ChatGPT · sam@example.com is the one row. The model's half of the demo ends (4.8): the banner says a playground forge alone, the chats page has no demo's start, and the composers' line reads "Using ChatGPT plan · gpt-6-luna · Manage usage". The galleries below Connect a model, Accounts and Chats draw the rest of the ChatGPT experience (below).
- **6. Setting up a Forgejo:** chosen in the flow, its defaults kept (work/hello, a new project hello); the box has fetched and is preparing. The flow shows each step, and the banner says where it is.
- **7. A Forgejo connected:** temper connected as its bot, made work/hello and adopted it into the new project hello; its first CI run passed. The flow ends on the new project, opened with no chat yet. The demo is retired: forge 1 stops and disappears, T1 and T4 end at 21:19 with the reason, and playground/widget leaves Playground’s repository lists. Its landing and transcripts stay readable; Recent landings names it “playground/widget · the playground’s, ended”. The banner is gone.

### 2.4 The first run's other notes

The welcome journey's own notes are its drawing's, under the rules (section 1.1). What stays here is the first run's, beyond it.

The person meets the demo as **the playground** (with the user, 2026-10-23): "Try the playground", the banner "This is the playground: a scripted model and a playground forge.", Scripted model, Playground forge and playground/widget; the scripts say "Playground forge" and "a scripted change". The design keeps the demo as its concept's name (`application.demo`, the demo's halves); setup.md 8.4 holds the scripts' words, and the client README 5.6 the page's (`../../draft/first-run.md`, 2.3).

The demo’s explicit states are `not started`, `running`, and `retired`; an absent state retains the older worlds’ running behavior. Its model and forge halves and context draw only while running. The first usable account from another provider ends the model half and removes the Scripted model’s account; the first connected forge of the person’s retires the demo’s forge and repository and ends its open chats, keeping history.

While the demo is not started and a forge or model is missing, the setup strip takes the demo strip’s place. It states what is missing, shows any forge setup status with View, and links to Continue setting up and (until a personal forge connects) Try the playground. Welcome, Accounts, Forges, Connect a forge and Connect a model omit it. With no account, a composer says “No model yet · Connect a model” and cannot send.

Where the documents disagree, or leave a word open, the mockup takes one side and says so here:

- **T1's next run takes the new account** (step 5), where temper's stays on the demo model. The draft says the demo model "stays the demo project's" (`../../draft/first-run.md`, 2.2). setup.md 8.3 says a task with no model chosen takes the first of its charter's models that has a usable account, and the demo model is last in both charters. jig resolves that default once, at the task's Create, and records it, so a new chat's first run takes gpt-6-luna on the ChatGPT plan while T1, created before it, keeps the demo model. The mockup draws T1 on gpt-6-luna too: re-resolving a task's default for each run is jig's to decide.
- **The coding agent's title is the change's** (T3, "Export widget's table as CSV"): setup.md 8.4 makes the change's title its agent's brief, and a chip is the spec's first line. The other worlds title that agent "Produce TN's branch", a stand-in for a title temper's procedure does not word yet.
- **The dark theme's words** are a stand-in. Their content is the demo's follow-up (`../../draft/first-run.md`, section 8). The empty search and the fallback are not drawn: they take step 4's shape.
- **The forges page** shows only the person’s forges (connecting, setting up, set up failed, connected, stopped), each folded until opened, and the playground’s. A forge not acted on is a choice in the “Connect a forge” flow: Choose → Details → Setting up → Ready for a Forgejo, and Choose → Connecting → Ready for GitHub (below), ending on the new project. The banner shows the set-up’s status and a link to it; the Forges nav item’s dot is amber while setting up or connecting, red when a forge needs the person. The page and the flow are forge.md 5.4's. Mockup-only galleries on both pages compare the flow’s other paths and the other forge rows, including managed and connected Forgejo. The flow’s “A Forgejo elsewhere” includes the setup entry that connecting as its admin needs.
- **Accounts and Connect a model** follow the forges' shape (with the user, 2026-10-23). The page stays Accounts; the flow and every entry point say "Connect a model": the banner, the composer's "No model yet", the page's "+ Connect a model". The flow is Choose → Signing in → Ready: Continue with ChatGPT opens its sign-in in the same tab, and the engine's listener sends the browser back to the page it left; the secondary button keeps Signing in here for a browser on another computer (B, below). A sign-in drawn under way offers the paste, folded on the engine’s machine and in Accounts, unfolded after the secondary button. A key has Details in Signing in’s place. ChatGPT is drawn whole; an API key's form and Claude's subscription are there for the context, not designed. Each account is a folded row: its name and holder, its deciding source's state, and a line of its models, what it pays for and its tasks; opened, the state's words and acts from 5.5's table, its sources in order, Pays for, Used by, Waiting for it, and its acts. The Accounts nav item's dot is amber while an account is limited, red while one needs the person. Mockup-only galleries draw, below Connect a model, each other ending, back in temper; below Accounts, the ChatGPT row in each state of 5.5's table, and a key's and a Claude subscription's rows; below Chats, a plan's first-use card and a chat whose plan is limited.
- **A plan pays for its holder's own work, and for projects where the holder is the only member;** a key or a program's login, with no uses given, for its holder's own work. A limited plan's tasks wait for it; where the account also holds a key, "Use the API key first" reorders its sources, and never moves a task to another account.
- **B is built:** a browser on another computer finishes by pasting the whole address of the page that could not be reached into temper; the tunnel still works (`ssh -L 1455:127.0.0.1:1455 HOST`, `../../../guide/box.md`, step 4). The first run’s 4a–4c draw it; Connect a model’s gallery draws every paste state (below), and Accounts draws the same paste folded. Words and controls follow temper-client-view's `accounts/words.rs`, `pages/connect_model.rs` and `pages/accounts.rs` as built.
- **A key reads Ready until a run is refused:** nothing checks it when it is added, so a wrong key shows only at its first request. The key's form says so, as a note.

The paste’s gallery is below [Connect a model](connect-model.html?world=first-run&step=5#the-paste). Its slate headings link to each state as the main card: `connect-model.html?world=first-run&step=5&way=chatgpt&paste=STATE&elsewhere=1`. The query names a mock state only, never a pasted address. Omit `elsewhere=1` for the folded paste under a local sign-in; `accounts.html?world=first-run&step=5&paste=waiting` shows it in the account’s row, also drawn in Accounts’ gallery at `#other-states-signing`. The external sign-in links use a fake provider, never ChatGPT’s live sign-in.

| State (`paste=STATE`) | Gallery anchor | What is drawn |
|---|---|---|
| `waiting` | `#paste-waiting`; `#paste-local` without `elsewhere=1` | The paste unfolded or folded; empty secret field, Finish connecting disabled |
| `written` | `#paste-written` | Hidden written field; Finish connecting enabled |
| `sending` | `#paste-sending` | Field disabled, button “Finishing…” |
| `finishing` | `#paste-finishing` | “Finishing your sign-in…”; field and finish button gone |
| `address`, `state` | `#paste-address`, `#paste-state` | Refusal above the empty field |
| `too-many` | `#paste-too-many` | Refusal and Continue with ChatGPT; field gone, as built |
| `too-long` | `#paste-too-long` | Bound’s refusal above the field; Finish connecting disabled |
| `not-waiting`, `expired` | `#paste-not-waiting`, `#paste-expired` | “This sign-in has ended.” and Continue with ChatGPT |
| `lost`, `unsent` | `#paste-lost`, `#paste-unsent` | “That address wasn't sent. Paste it again.”; empty field |
| `done` | `#paste-done` | Ready, “Signed in to ChatGPT”, using the existing return’s drawing |

The earlier first-run mockup, a story of its own, also drew "Reset demo", a turn's "Just now", the composer's placeholder, "Landed into main" and the pull request's link on the card. The client leaves each out (client README, section 11), and so does this world.


### 2.5 Chats, a chat, Accounts and the inbox

`chats.html`, `chat.html`, `inbox.html` and `accounts.html`, surveyed
2026-10-25 at temper ff1dac865c (the branch's base), then compared
with the gallery at 390 and 1440 px. The branch builds their composer,
chat rows and ended bars, inbox menus and cards, and account rows.
The two additional gallery states are the first plan's Chats at step 5
and the retired playground's T1 at step 7. This group remains here:
its drawings still include the future pieces and missing facts below,
unmarked in their main drawing.

Where the client's recorded picks stand against the drawing:

- **Accept**, not “Accept goal”. The chatbar carries status and own
  spend, “N spent · of B”, and no model, template, charter, host or
  budget mode; the composer's line names the next run. Its chooser
  stays behind the folded “Change model”.
- **Closed** for every ended chat's bar and list row, with the end's
  clock in the bar when supplied. The conversation's Failed and
  Cancelled words and reasons stay. Chats is one list, newest first;
  a row's spend is its own, with live delegates counted separately.
- **Acts beside each account and source**, including the move
  controls. A person's own account shows neither Used by nor Waiting
  for it; the deployment's accounts can show both. No Tasks or
  Workers in the navigation.
- **Mixed key and program offers** do not establish MODEL's endpoint.
  The composer's line keeps “Using MODEL” rather than naming an
  account from its login. Known provider and plan names elsewhere
  use OpenAI/ChatGPT and Anthropic/Claude.

What is not built here, though the main drawings show it:

- A chats row's or update's reply preview; a question's choice
  buttons; a chats row's ended delegates; fractional spend.
- A program's usage windows (6.3); an account's email display; the
  composer's account where the provider offers both key and program
  endpoints.
- A proposal's template and charter names (`me` has no charter names,
  and a proposed task names no template); a card's lineage beyond
  its own task (the inbox's `Card` carries no ancestry).
- **Board and Changes** in the navigation are not built.

Other facts the comparison found absent:

- **A timer's next clock or interval.** The chat's wake policy says
  timers can wake it, but carries neither “until 09:10” nor “every
  2 h”. The client keeps its existing news-wait words.
- **The request's own timestamp.** The task gives its specification
  but no separate request clock. The conversation uses its first
  activity line's clock for the request, as client README 3.5 says;
  that differs from mockups which timestamp the request separately.
- **An update's title before its task document arrives.** The entry
  has its task number and kind but no title. Its heading-link says
  “Task N” until the task document supplies the title, preserving
  the same OpenResult/Go act and seen behavior.
- **A plan's first-use card.** No delivered account or task fact says
  whether the person has seen that explanation; the mockup-only
  sample below Chats is not drawn as part of the client's page.
- **Older transcript and calls beyond the receiving bounds.** The
  reference T1 drawing has 142 older turns and nine calls; the fake
  client's delivered window has no such total and carries three
  calls. The gallery keeps the browser's shipped bounds (browser.md
  7.2), reports omitted text, and offers the existing older-page act
  where supplied, rather than inventing history or widening limits.

Where a word or shape was otherwise open, the client keeps its
existing pattern: jig's executor and specification words and resource
and grant words in proposals (numeric charters until names arrive);
Kind's folded disclosure for all inbox menus; the folded rejection
editor for “Answer in words”; the forge row's native disclosure and
summary for account rows; the existing dashed empty block for “No
model of yours yet.”. Confirmation keeps each supplied funding fact
in its Budget row, as the existing decision referee reads it. After
retirement the banner selects the surviving project, hello, as client
README 4.8 requires; the history remains the retired task's. Other-state
galleries below mockup pages remain
examples of separate states, not extra content appended to a page.
