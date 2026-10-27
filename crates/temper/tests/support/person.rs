//! A real native client driven by jig's finite semantic-tree script.
use jig_client_protocol_link::Signing;
use skein_fake_person::{Doing, Face, Find, Found, Next, Step, Target, TreeFace};
use skein_io::kernel::{Complete, Done, Fd, Op, Submit};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use temper_client_domain::Address;
use temper_client_domain::{ChatsFilter, PlanFilter};
use temper_client_shell_terminal::client::{NativeClient, Settings, Start};
/// Resolve the process story's Reply and task title together on one Item.
/// Other semantic queries and interaction checks retain `TreeFace`'s contract.
pub struct ReplyFace<'a> {
    tree: &'a skein_ui::Tree,
}
impl<'a> ReplyFace<'a> {
    #[must_use]
    pub fn new(tree: &'a skein_ui::Tree) -> Self {
        Self { tree }
    }
}
impl Face for ReplyFace<'_> {
    fn find(&mut self, find: &Find) -> Found {
        if find.target == Find::named(skein_ui::Role::Heading { level: 3 }, b"replied").target
            && let [scope @ Target::Role { role: skein_ui::Role::Item, .. }] = find.within.as_ref()
        {
            let item = Find { within: Box::new([]), target: scope.clone(), nth: 0 };
            for node in self.tree.nodes().iter().filter(|node| node.role == skein_ui::Role::Item) {
                let Some(mut face) = TreeFace::subtree(self.tree, node.id) else { continue };
                // A descendant Item matching the title does not qualify this root.
                if face.find(&item) == Found::Node(node.id)
                    && let reply @ Found::Node(_) = face.find(find)
                {
                    return reply;
                }
            }
            return Found::Absent;
        }
        TreeFace::new(self.tree).find(find)
    }
    fn role(&self, node: skein_ui::NodeId) -> Option<skein_ui::Role> {
        TreeFace::new(self.tree).role(node)
    }
    fn allows(&self, node: skein_ui::NodeId, act: &skein_ui::Act) -> bool {
        TreeFace::new(self.tree).allows(node, act)
    }
}
const START_WITHIN: skein_lib::Duration = skein_lib::Duration::from_secs(40);
// Measured 21 s from Start to landed + one missed slow poll of 40 s = 61 s.
const DEMO_LANDS_WITHIN: skein_lib::Duration = skein_lib::Duration::from_secs(122);
const SIGN_IN_FIRST: skein_lib::Duration = skein_lib::Duration::from_millis(100);
const SIGN_IN_MOST: skein_lib::Duration = skein_lib::Duration::from_secs(2);
const PIPE: Token = Token::new(u64::MAX);
const CLOSE: Token = Token::new(u64::MAX - 1);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Begin {
    LinkWithKey,
    LinkWithOpenAiKey,
    Token,
    Link,
    Playground,
}
/// The script is separate from the client's production composition.
pub struct Script {
    steps: Box<[Step<Address>]>,
    begin: Begin,
}
#[must_use]
pub fn answered_chat(words: &[u8], answer: &[u8]) -> Script {
    Script {
        begin: Begin::LinkWithKey,
        steps: Box::new([
            Step::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
            Step::See { find: Find::named(skein_ui::Role::TextArea, b"Start a chat"), within: START_WITHIN },
            Step::Type { find: Find::named(skein_ui::Role::TextArea, b"Start a chat"), words: words.into() },
            Step::Press { find: Find::named(skein_ui::Role::Button, b"Start") },
            Step::See { find: Find::text(answer), within: START_WITHIN },
        ]),
    }
}
/// The configured first owner adds a deployment key through the accounts page.
#[must_use]
pub fn deployment_account() -> Script {
    use skein_ui::Role;
    Script {
        begin: Begin::LinkWithOpenAiKey,
        steps: Box::new([
            Step::Go { address: Address::Accounts },
            Step::See { find: Find::exactly_named(Role::Heading { level: 1 }, b"Accounts"), within: START_WITHIN },
            Step::See {
                find: Find::exactly_named(Role::Disclosure, b"OpenAI API key \xc2\xb7 held by the deployment"),
                within: START_WITHIN,
            },
        ]),
    }
}
/// The welcome's button leaves the tab; the next observation belongs to its come-back page.
#[must_use]
pub fn welcome_sign_in() -> Script {
    use skein_ui::Role;
    Script {
        begin: Begin::Link,
        steps: Box::new([
            Step::Go { address: Address::Welcome { step: temper_client_domain::Step::Model { way: None } } },
            Step::See { find: Find::exactly_named(Role::Button, b"Continue with ChatGPT"), within: START_WITHIN },
            Step::Press { find: Find::exactly_named(Role::Button, b"Continue with ChatGPT") },
            Step::See { find: Find::named(Role::Paragraph, b"Model: ChatGPT"), within: START_WITHIN },
        ]),
    }
}
/// The same account's source is still signed in after a healthy service restart.
#[must_use]
pub fn kept_chatgpt_account(heading: &[u8]) -> Script {
    use skein_ui::Role;
    Script {
        begin: Begin::Token,
        steps: Box::new([
            Step::Go { address: Address::Accounts },
            Step::See { find: Find::exactly_named(Role::Disclosure, heading), within: START_WITHIN },
            chatgpt_source_ready(heading),
        ]),
    }
}
/// Observe a ctl-started sign-in through the same person's served accounts page.
#[must_use]
pub fn signed_in_account(signing: &[u8], name: &[u8]) -> Script {
    use skein_ui::Role;
    // jig's translate.rs:124 leaves the signed-in display empty; follow it when jig carries one.
    let card = format!("ChatGPT · OpenAI · held by {}", String::from_utf8_lossy(name));
    Script {
        begin: Begin::Token,
        steps: Box::new([
            // A sign-in started from another client is drawn on Accounts (README 11).
            Step::Go { address: Address::Accounts },
            Step::See { find: Find::exactly_named(Role::Status, signing), within: START_WITHIN },
            Step::See { find: Find::exactly_named(Role::Disclosure, card.as_bytes()), within: START_WITHIN },
            chatgpt_source_ready(card.as_bytes()),
            Step::See { find: Find::exactly_named(Role::Status, b"Signed in to ChatGPT"), within: START_WITHIN },
        ]),
    }
}
/// Read the state in the sign-in source's own Item, inside the exact account card.
/// The source kind and state are separate nodes (client/README.md, 5.5).
#[must_use]
pub fn chatgpt_source_ready(heading: &[u8]) -> Step<Address> {
    use skein_ui::Role;
    // TreeFace reads disclosure contents even while the native shell folds them.
    Step::See {
        find: Find::exactly_named(Role::Status, b"Ready").within(Box::new([
            Target::ExactRole { role: Role::Disclosure, name: heading.into() },
            Target::Role { role: Role::Item, name: b"ChatGPT sign-in".as_slice().into() },
        ])),
        within: START_WITHIN,
    }
}
/// Signs in by the printed link; the demo's running start keeps the deployment key.
/// Asks `cue` in a new chat, task `task`, then sees its committed reply.
/// Chats are ended by their requester; the scripted text replies and waits.
#[must_use]
pub fn demo_chat(task: u64, cue: &[u8], text: &[u8]) -> Script {
    use skein_ui::Role;
    Script {
        begin: Begin::Link,
        steps: Box::new([
            Step::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
            Step::See { find: Find::named(Role::TextArea, b"Start a chat"), within: START_WITHIN },
            Step::See { find: Find::exactly_named(Role::Status, b"Ready"), within: START_WITHIN },
            Step::Gone { find: Find::exactly_named(Role::Status, b"Starting"), within: START_WITHIN },
            Step::Press { find: Find::exactly_named(Role::Button, b"Scripted model") },
            Step::Type { find: Find::named(Role::TextArea, b"Start a chat"), words: cue.into() },
            Step::Press { find: Find::named(Role::Button, b"Start") },
            Step::See {
                find: Find::exactly_named(Role::Paragraph, b"You")
                    .within(Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])),
                within: START_WITHIN,
            },
            Step::See {
                find: Find::exactly_named(Role::Paragraph, cue)
                    .within(Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])),
                within: START_WITHIN,
            },
            Step::See {
                find: Find::text(text)
                    .within(Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])),
                within: START_WITHIN,
            },
            Step::See { find: Find::named(Role::TextArea, b"Message"), within: START_WITHIN },
            Step::Go { address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL } },
            // Reading the reply in its chat marks it seen before the inbox opens.
            Step::Gone { find: Find::named(Role::Heading { level: 3 }, b"replied"), within: START_WITHIN },
            Step::Go { address: Address::Chat { task } },
            Step::See {
                find: Find::text(text)
                    .within(Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])),
                within: START_WITHIN,
            },
            Step::Gone {
                find: Find::text(b"Invalid call: unknown tool")
                    .within(Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])),
                within: START_WITHIN,
            },
        ]),
    }
}
/// The first run delegates and lands without choosing a model or answering its wait.
#[must_use]
pub fn demo_change() -> Script {
    use skein_ui::Role;
    // The empty first run's chat is task 1; its change is task 2.
    const CHANGE: u64 = 2;
    let conversation = || Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }]);
    let details = || Box::new([Target::Role { role: Role::Region, name: b"Task details".as_slice().into() }]);
    let card = || {
        Box::new([
            Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() },
            Target::Role { role: Role::Region, name: b"Task 2".as_slice().into() },
        ])
    };
    Script {
        begin: Begin::Playground,
        steps: playground_opening(conversation).into_iter().chain([
            Step::See {
                find: Find::text("CSV export is available from widget’s table. The change landed on Playground forge. This was a scripted change.".as_bytes()).within(conversation()),
                within: DEMO_LANDS_WITHIN,
            },
            // client/README.md, 5.4: the committed opening turn fits whole.
            Step::Gone { find: Find::exactly_named(Role::Status, b"Earlier text omitted").within(conversation()), within: START_WITHIN },
            Step::Gone { find: Find::exactly_named(Role::Paragraph, b"delegate").within(conversation()), within: START_WITHIN },
            // client/README.md, 5.7: the delegated change's card says it landed.
            Step::See { find: Find::text("T2 · CHANGE".as_bytes()).within(card()), within: START_WITHIN },
            Step::See { find: Find::exactly_named(Role::Status, b"Landed").within(card()), within: START_WITHIN },
            Step::See { find: Find::text("CI passed. Landed · merge commit ".as_bytes()).within(card()), within: START_WITHIN },
            Step::Gone { find: Find::exactly_named(Role::Status, b"Applied"), within: START_WITHIN },
            // The owner's chat log holds the result's words alone, not its ending.
            Step::Go { address: Address::Task { task: CHANGE, filter: PlanFilter::All } },
            Step::See {
                find: Find::exactly_named(Role::Heading { level: 1 }, b"Export widget's table as CSV").within(details()),
                within: START_WITHIN,
            },
            // Completed, not held: a change that fails to land is held; the clone check reads the merge.
            // client/README.md, 5.8: the header words an ended task by its owner's ending.
            Step::See { find: Find::exactly_named(Role::Status, b"Completed").within(details()), within: START_WITHIN },
        ]).collect(),
    }
}

fn playground_opening(conversation: impl Fn() -> Box<[Target; 1]>) -> [Step<Address>; 12] {
    use skein_ui::Role;
    [
        Step::Go { address: Address::Welcome { step: temper_client_domain::Step::Welcome } },
        Step::See { find: Find::exactly_named(Role::Button, b"Try the playground"), within: START_WITHIN },
        Step::Press { find: Find::exactly_named(Role::Button, b"Try the playground") },
        Step::See { find: Find::exactly_named(Role::Status, b"Starting"), within: START_WITHIN },
        Step::See { find: Find::exactly_named(Role::Status, b"Ready"), within: START_WITHIN },
        Step::See { find: Find::exactly_named(Role::Paragraph, "Using Scripted model · Default".as_bytes()), within: START_WITHIN },
        Step::See { find: Find::exactly_named(Role::Disclosure, b"Change model"), within: START_WITHIN },
        // client/README.md, 4.8: a suggested request starts and opens its chat.
        Step::See { find: Find::named(Role::Button, b"Add CSV export to widget"), within: START_WITHIN },
        Step::Press { find: Find::named(Role::Button, b"Add CSV export to widget") },
        Step::See { find: Find::exactly_named(Role::Paragraph, b"You").within(conversation()), within: START_WITHIN },
        Step::See { find: Find::exactly_named(Role::Paragraph, b"Add CSV export to widget").within(conversation()), within: START_WITHIN },
        Step::See {
            find: Find::text("I’ll delegate this change in playground/widget. It will push a branch, open a pull request, check CI and land into main on Playground forge.".as_bytes()).within(conversation()),
            within: START_WITHIN,
        },
    ]
}
/// Signs in to the first run and sees the welcome without starting the playground.
#[must_use]
pub fn welcome_open() -> Script {
    Script {
        begin: Begin::Playground,
        steps: Box::new([
            Step::Go { address: Address::Welcome { step: temper_client_domain::Step::Welcome } },
            Step::See {
                find: Find::exactly_named(skein_ui::Role::Button, b"Try the playground"),
                within: START_WITHIN,
            },
        ]),
    }
}
/// Starts the first run's playground and sees Ready and its chat's first answer.
#[must_use]
pub fn demo_start_answered() -> Script {
    let conversation =
        || Box::new([Target::Role { role: skein_ui::Role::Log, name: b"Conversation".as_slice().into() }]);
    Script { begin: Begin::Playground, steps: Box::new(playground_opening(conversation)) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectSelection {
    Pending,
    Selected,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Departure {
    Disabled,
    Present,
    Left,
}
/// Stdout and the client share one host; the link comes only from the binary.
pub struct Person {
    pub client: Option<NativeClient>,
    root: Option<Fd>,
    actor: skein_fake_person::Person<Address>,
    input: Option<Fd>,
    output: Option<Fd>,
    reading: bool,
    closing: u32,
    bytes: Vec<u8>,
    wake: Option<Time>,
    signing_until: Option<Time>,
    signing_retry: Option<Time>,
    signing_backoff: skein_lib::Duration,
    pub finished: bool,
    project: ProjectSelection,
    pub failed: Option<String>,
    adopting: bool,
    begin: Begin,
    directory: Box<[u8]>,
    seed: u64,
    departure: Departure,
    leave: Option<Box<[u8]>>,
    return_address: Option<Address>,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl Person {
    /// Every byte read from the binary's stdout, including the printed link.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn with_link(root: Fd, input: Fd, output: Fd, script: Script, link: &[u8]) -> Self {
        let mut person = Self::new(root, input, output, script);
        person.bytes.extend_from_slice(link);
        person.bytes.push(b'\n');
        person
    }
    #[must_use]
    pub fn new(root: Fd, input: Fd, output: Fd, script: Script) -> Self {
        Self::build(root, Some(input), Some(output), script)
    }
    #[must_use]
    pub fn departing(root: Fd, input: Fd, output: Fd, script: Script) -> Self {
        let mut person = Self::new(root, input, output, script);
        person.departure = Departure::Present;
        person.project = ProjectSelection::Selected;
        person
    }
    pub fn leave(&mut self) -> Option<Box<[u8]>> {
        self.leave.take()
    }
    /// Reopen the departed tab on its kept sign-in, once the native transport has drained.
    pub fn come_back(&mut self, root: Fd, path: &[u8]) {
        assert!(self.departure == Departure::Left, "the tab departed");
        assert!(self.client.as_ref().is_some_and(Host::is_empty), "departing client drained");
        let temper_client_domain::Place::Address(address) = temper_client_domain::address::parse(path) else {
            panic!("the listener returned a client address");
        };
        self.client = None;
        self.root = Some(root);
        self.begin = Begin::Token;
        self.seed = self.seed.checked_add(1).expect("fresh client seed");
        self.return_address = Some(address);
        self.departure = Departure::Present;
        self.wake = Some(Time::ZERO);
    }
    /// Wait for ctl to finish a sign-in in its own directory; load it through the native shell.
    #[must_use]
    pub fn waiting_for_ctl(root: Fd, script: Script) -> Self {
        let mut person = Self::build(root, None, None, Script { begin: Begin::Token, ..script });
        person.directory = b"client".as_slice().into();
        person
    }
    /// Keep the script's key-entry steps, then load ctl's completed sign-in.
    #[must_use]
    pub fn waiting_for_ctl_with_key(root: Fd, script: Script) -> Self {
        let mut person = Self::build(root, None, None, script);
        person.begin = Begin::Token;
        person
    }
    /// Hold the native observer until ctl supplies its identity and the expected page words.
    #[must_use]
    pub fn waiting_for_observation(root: Fd) -> Self {
        let mut person = Self::waiting_for_ctl(root, Script { begin: Begin::Token, steps: Box::new([]) });
        // Accounts belongs to the person and needs no project selection.
        person.project = ProjectSelection::Selected;
        person
    }
    pub fn observe(&mut self, link: &[u8], script: Script) {
        assert!(self.client.is_none(), "observation starts once");
        self.actor = skein_fake_person::Person::new(script.steps);
        self.ctl_ready(link);
    }
    #[must_use]
    pub fn steps_seen(&self) -> u32 {
        self.actor.at()
    }
    pub fn ctl_ready(&mut self, link: &[u8]) {
        assert!(self.bytes.is_empty(), "one handover from ctl");
        self.bytes.extend_from_slice(link);
        self.bytes.push(b'\n');
    }
    /// The welcome has answered and the playground button has not yet been pressed.
    #[must_use]
    pub fn choosing_playground(&self) -> bool {
        self.begin == Begin::Playground && self.actor.at() == 2
    }
    fn build(root: Fd, input: Option<Fd>, output: Option<Fd>, script: Script) -> Self {
        let steps = if matches!(script.begin, Begin::LinkWithKey | Begin::LinkWithOpenAiKey) {
            use skein_ui::Role;
            let key_way: &[u8] =
                if script.begin == Begin::LinkWithOpenAiKey { b"Use an OpenAI API key" } else { b"Use an API key" };
            let mut setup = vec![
                Step::Go { address: Address::ConnectModel { way: None } },
                Step::See {
                    find: Find::exactly_named(Role::Heading { level: 1 }, b"Connect a model"),
                    within: START_WITHIN,
                },
                Step::See { find: Find::named(Role::Button, b"Use deployment accounts"), within: START_WITHIN },
                Step::Press { find: Find::named(Role::Button, b"Use deployment accounts") },
                Step::Press { find: Find::named(Role::Link, key_way) },
                Step::Type {
                    find: Find::named(Role::TextField, b"Key (pasted once)"),
                    words: jig_channel_world::fixtures::KEY.into(),
                },
                Step::Press { find: Find::named(Role::Button, b"Add key") },
                Step::See { find: Find::text(b"Added your"), within: START_WITHIN },
            ];
            setup.extend(script.steps);
            setup.into_boxed_slice()
        } else {
            script.steps
        };
        Self {
            client: None,
            root: Some(root),
            actor: skein_fake_person::Person::new(steps),
            input,
            output,
            reading: false,
            closing: 0,
            bytes: Vec::new(),
            wake: None,
            signing_until: None,
            signing_retry: None,
            signing_backoff: SIGN_IN_FIRST,
            finished: false,
            project: if script.begin == Begin::Playground {
                ProjectSelection::Selected
            } else {
                ProjectSelection::Pending
            },
            failed: None,
            adopting: false,
            begin: script.begin,
            directory: b"private".as_slice().into(),
            // Fresh demo clients must not reuse the same request names when
            // signing the same owner in again on the same deployment.
            seed: if matches!(script.begin, Begin::Link | Begin::Playground) {
                let seed = skein_shell::crypto_seed().expect("demo client's fresh seed");
                u64::from_le_bytes(seed.expose()[..8].try_into().expect("eight seed bytes"))
            } else {
                7
            },
            departure: Departure::Disabled,
            leave: None,
            return_address: None,
            completions: Queue::with_capacity(512),
            submissions: Queue::with_capacity(512),
        }
    }
    fn close(&mut self, fd: Fd) {
        self.closing += 1;
        self.submissions.push(Submit { op: CLOSE, kind: Op::Close { fd } });
    }
    fn completed(&mut self, complete: Complete) {
        if complete.op == CLOSE {
            self.closing -= 1;
            complete.result.expect("pipe closed");
        } else if complete.op == PIPE {
            self.reading = false;
            let Op::PipeRead { buf, .. } = complete.kind else {
                panic!("stdout read");
            };
            let Done::Count(count) = complete.result.expect("stdout read completes") else {
                panic!("read count");
            };
            self.bytes.extend_from_slice(&buf[..usize::try_from(count).expect("bounded read")]);
            assert!(self.bytes.len() <= 65_536, "bounded printed links");
            if count == 0 {
                let fd = self.output.take().expect("stdout");
                self.close(fd);
            }
        } else {
            self.client.as_mut().expect("native client").completions().push(complete);
        }
    }
    fn start(&mut self, now: Time) {
        if self.client.is_none()
            && let Some(link) = super::printed_link(&self.bytes)
        {
            let address = std::str::from_utf8(link)
                .expect("printed URL")
                .strip_prefix("http://")
                .expect("loopback URL")
                .split('/')
                .next()
                .expect("URL address")
                .parse()
                .expect("socket address");
            self.signing_until = Some(now.saturating_add(START_WITHIN));
            let make_client =
                if self.departure == Departure::Disabled { NativeClient::new } else { NativeClient::departing };
            self.client = Some(
                make_client(
                    Settings {
                        engine: skein_io::Addr::Ip(address),
                        directory: self.directory.clone(),
                        limits: temper_client_shell_terminal::limits::native(),
                    },
                    self.root.take().expect("client root"),
                    skein_shell::effective_user(),
                    self.seed,
                    if self.begin == Begin::Token { Start::Token } else { Start::Link(link.into()) },
                )
                .expect("printed sign-in link"),
            );
        }
    }
    fn notices(&mut self) {
        let client = self.client.as_mut().expect("native client");
        if self.adopting
            && let Some(refusal) = adoption_refusal(client.tree())
        {
            self.failed = Some(format!("native adoption refused: {refusal}"));
        }
        while let Some(notice) = client.notices() {
            if let temper_client_shell_terminal::client::Shown::Notice(
                temper_client_shell_terminal::route::Notice::Leave { to },
            ) = notice
            {
                assert!(self.departure == Departure::Present, "one tab departure");
                self.departure = Departure::Left;
                self.leave = Some(to);
                client.close();
                continue;
            }
            assert!(
                !matches!(
                    notice,
                    temper_client_shell_terminal::client::Shown::Notice(
                        temper_client_shell_terminal::route::Notice::Fact(_)
                    )
                ),
                "native person step {} protocol fact: {notice:?}",
                self.actor.at()
            );
            assert!(
                !matches!(
                    notice,
                    temper_client_shell_terminal::client::Shown::Terminal(
                        temper_client_shell_terminal::terminal::Notice::DirectoryRefused
                    )
                ),
                "private client directory refused"
            );
        }
    }
    fn drive(&mut self, now: Time, wall: Wall) {
        let Some(client) = &mut self.client else {
            return;
        };
        client.iterate(now, wall);
        while let Some(signing) = client.signing() {
            match signing {
                Signing::SignedIn { .. } => {
                    self.signing_retry = None;
                    self.signing_until = None;
                }
                Signing::Again | Signing::Busy => {
                    let until = self.signing_until.expect("sign-in deadline");
                    assert!(now < until, "native signing failed: {signing:?}");
                    self.signing_retry = Some(now.saturating_add(self.signing_backoff).min(until));
                    self.signing_backoff = self.signing_backoff.saturating_mul(2).min(SIGN_IN_MOST);
                }
                other @ (Signing::Refused | Signing::Lost | Signing::SignedOut | Signing::Full { .. }) => {
                    panic!("native signing failed: {other:?}");
                }
            }
        }
        self.notices();
        let client = self.client.as_mut().expect("native client");
        self.wake = None;
        if self.signing_backoff != SIGN_IN_FIRST
            && let Some(until) = self.signing_until
        {
            assert!(now < until, "native signing failed: Again");
            self.wake = Some(until);
        }
        if let Some(retry) = self.signing_retry {
            if now >= retry {
                self.signing_retry = None;
                // Start::Token installs a held credential locally; only link sign-ins have replies.
                client
                    .sign_in(super::printed_link(&self.bytes).expect("printed sign-in link"))
                    .expect("printed sign-in link");
            } else {
                self.wake = Some(self.wake.map_or(retry, |at| at.min(retry)));
            }
        }
        if client.started() && self.project == ProjectSelection::Pending && select_project(client) {
            self.project = ProjectSelection::Selected;
            self.wake = Some(now);
        }
        let signing_wake = self.wake;
        if client.started()
            && let Some(address) = self.return_address.take()
        {
            client.go(address);
        }
        if client.started()
            && self.departure != Departure::Left
            && self.project == ProjectSelection::Selected
            && !self.finished
            && self.failed.is_none()
        {
            let before = self.actor.at();
            match self.actor.next(&mut ReplyFace::new(client.tree()), now) {
                Next::Do(Doing::Act { node, act }) => {
                    self.adopting |= client.tree().nodes().iter().any(|item| {
                        item.id == node && item.role == skein_ui::Role::Button && item.name.as_deref() == Some(b"Adopt")
                    });
                    assert!(client.act(node, act), "offered person act");
                }
                Next::Do(Doing::Go { address }) => {
                    self.adopting = false;
                    client.go(address);
                }
                Next::Do(Doing::Reload) => panic!("chat script has no reload"),
                Next::Wait { until } => self.wake = until,
                Next::Done => {
                    self.finished = true;
                    client.close();
                }
                Next::Failed { step, why } => {
                    self.failed = Some(format!("native person step {step}: {why:?}"));
                }
            }
            if self.begin == Begin::Playground && before == 3 && self.actor.at() == 4 {
                let mut face = TreeFace::new(client.tree());
                let Found::Node(cue) =
                    face.find(&Find::exactly_named(skein_ui::Role::Button, b"Add CSV export to widget"))
                else {
                    panic!("starting playground has its suggested request");
                };
                assert!(!face.allows(cue, &skein_ui::Act::Press), "suggestions wait for the start's answer");
            }
            if self.actor.at() != before {
                self.wake = Some(now);
            }
        }
        self.wake = [self.wake, signing_wake].into_iter().flatten().min();
        while let Some(submit) = client.submissions().pop() {
            self.submissions.push(submit);
        }
    }
}
fn adoption_refusal(tree: &skein_ui::Tree) -> Option<String> {
    let nodes = tree.nodes();
    let (at, alert) = nodes.iter().enumerate().find(|(_, node)| node.role == skein_ui::Role::Alert)?;
    let end = at.checked_add(usize::try_from(alert.size).expect("bounded alert subtree")).expect("alert end");
    let mut reason = String::new();
    for node in &nodes[at..end] {
        for run in &node.text {
            reason.push_str(&String::from_utf8_lossy(&run.text));
        }
    }
    Some(reason)
}

#[test]
fn a_native_adoption_refusal_names_the_alert_without_a_wait() {
    let (view, _) = temper_client_shell_terminal::limits::view(&temper_client_shell_terminal::limits::native())
        .expect("native view bounds");
    let mut builder = skein_ui::Builder::new(&view.tree);
    builder.open(skein_ui::Role::Page);
    builder.open(skein_ui::Role::Alert);
    builder.text(skein_ui::Run::Plain, b"Forge 1 did not answer; nothing was adopted.");
    builder.close();
    builder.close();
    assert_eq!(adoption_refusal(&builder.finish()).as_deref(), Some("Forge 1 did not answer; nothing was adopted."));
}
/// The banner's button for a project `me` leaves unnamed, "Project N"
/// (client/README.md, 5.1).
fn an_unnamed_project(name: Option<&[u8]>) -> bool {
    let Some(number) = name.and_then(|name| name.strip_prefix(b"Project ".as_slice())) else { return false };
    !number.is_empty() && number.iter().all(u8::is_ascii_digit)
}
// Project zero is absent from me; the sole configured project may already be selected.
fn select_project(client: &mut NativeClient) -> bool {
    let node = client
        .tree()
        .nodes()
        .iter()
        .find(|node| node.role == skein_ui::Role::Button && an_unnamed_project(node.name.as_deref()))
        .map(|node| node.id);
    let Some(node) = node else { return false };
    assert!(client.act(node, skein_ui::Act::Press), "offered project selection");
    true
}
impl Host for Person {
    fn iterate(&mut self, now: Time, wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            self.completed(complete);
        }
        if let Some(fd) = self.input.take() {
            self.close(fd);
        }
        self.start(now);
        if !self.reading
            && let Some(fd) = self.output
        {
            self.reading = true;
            self.submissions
                .push(Submit { op: PIPE, kind: Op::PipeRead { fd, buf: vec![0; 1024].into_boxed_slice() } });
        }
        self.drive(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        !self.completions.is_empty()
            || self.input.is_some()
            || (!self.reading && self.output.is_some())
            || self.wake.is_some_and(|at| at <= now)
            || self.client.as_ref().is_some_and(|client| client.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.client.as_ref().and_then(Host::next_deadline), self.wake].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        [self.client.as_ref().and_then(Host::next_policy_deadline), self.wake].into_iter().flatten().min()
    }
    fn is_empty(&self) -> bool {
        self.output.is_none() && self.closing == 0 && self.client.as_ref().is_some_and(Host::is_empty)
    }
    fn worst_case(&self) -> u64 {
        self.client.as_ref().map_or(131_072, |client| client.worst_case() + 131_072)
    }
    fn operations(&self) -> u32 {
        512
    }
}

#[must_use]
pub fn adopt_and_change(_forgejo: &super::forgejo::Throwaway) -> Script {
    use skein_ui::Role;
    let mut steps = answered_chat(b"Hello", super::ANSWER).steps.into_vec();
    steps.extend([
        Step::Go { address: Address::Forges },
        // The Account slot of this row is rendered only for Connected.
        Step::See {
            find: Find::named(Role::Description, b"temper \xc2\xb7 user ").within(Box::new([Target::ExactRole {
                role: Role::Disclosure,
                name: b"Forgejo on this machine".as_slice().into(),
            }])),
            within: super::WITHIN,
        },
        Step::Go {
            address: Address::Inbox {
                filter: temper_client_domain::address::Filter {
                    project: Some(1),
                    ..temper_client_domain::address::Filter::ALL
                },
            },
        },
        Step::See { find: Find::named(Role::Link, b"Adopt a repository"), within: super::WITHIN },
        Step::Press { find: Find::named(Role::Link, b"Adopt a repository") },
        Step::See { find: Find::named(Role::TextField, b"Owner"), within: super::WITHIN },
        Step::Type { find: Find::named(Role::TextField, b"Owner"), words: b"acme".as_slice().into() },
        Step::Type { find: Find::named(Role::TextField, b"Name"), words: b"widget".as_slice().into() },
        Step::Type { find: Find::named(Role::TextField, b"Landing branch"), words: b"main".as_slice().into() },
        Step::Press { find: Find::named(Role::Button, b"Adopt") },
        Step::See { find: Find::text(b"Adopted acme/widget on forge 1 as owned."), within: super::WITHIN },
    ]);
    steps.extend([
        Step::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        Step::See { find: Find::named(Role::TextArea, b"Start a chat"), within: super::WITHIN },
        Step::Type {
            find: Find::named(Role::TextArea, b"Start a chat"),
            words: b"@forge_chat Land a small change".as_slice().into(),
        },
        Step::Press { find: Find::named(Role::Button, b"Start") },
        Step::Go { address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL } },
        inbox_reply(FORGE_CHAT_REPLY),
    ]);
    Script { steps: steps.into_boxed_slice(), begin: Begin::LinkWithKey }
}
/// Observe the new chat's acknowledgement without assuming its task number.
#[must_use]
pub fn adopt_and_change_with_tier(forgejo: &super::forgejo::Throwaway) -> Script {
    let mut script = adopt_and_change(forgejo);
    let last = script.steps.len() - 1;
    script.steps[last - 1] = Step::See { find: Find::text(b"Change requested"), within: super::WITHIN };
    script.steps[last] = Step::Go { address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL } };
    script
}

/// A person's live sitting keeps the native stories, allowing real completions their budget.
#[must_use]
pub fn live_adopt_and_change(forge: &super::forgejo::Throwaway) -> Script {
    let mut script = adopt_and_change(forge);
    for step in &mut script.steps {
        if let Step::See { within, .. } = step {
            *within = skein_lib::Duration::from_secs(1_740);
        }
    }
    script
}

#[must_use]
pub fn found() -> Script {
    Script {
        begin: Begin::Token,
        steps: Box::new([
            Step::Go { address: Address::Inbox { filter: temper_client_domain::address::Filter::ALL } },
            inbox_reply(FORGE_CHAT_REPLY),
        ]),
    }
}
#[must_use]
pub fn found_from(task: u64) -> Script {
    let mut script = found();
    *script.steps.last_mut().expect("reply step") = inbox_reply(format!("Reply from T{task}").as_bytes());
    script
}

/// The forge chat's reply in the inbox: the chat is task 2, after "Hello", and
/// the view names each inbox entry by its kind and its source task.
const FORGE_CHAT_REPLY: &[u8] = b"Reply from T2";
#[must_use]
pub fn inbox_reply(entry: &[u8]) -> Step<Address> {
    Step::See {
        find: Find::named(skein_ui::Role::Heading { level: 3 }, b"replied")
            .within(Box::new([Target::Role { role: skein_ui::Role::Item, name: entry.into() }])),
        within: super::WITHIN,
    }
}
