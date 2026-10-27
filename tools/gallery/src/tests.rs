use super::*;
use skein_fake_person::{Face, Find, Found, TreeFace};
use skein_scratch::Scratch;

#[test]
fn names_are_the_promised_ones() {
    let samples = samples::samples();
    let mut names = Vec::new();
    for sample in &samples {
        assert!(!names.contains(&sample.name), "duplicate {}", sample.name);
        names.push(sample.name);
    }
    let promised: Vec<&str> = include_str!("../names.txt").lines().collect();
    assert_eq!(names, promised);
}

#[test]
fn navigation_links_have_addresses_and_install_action_links_keep_their_binding() {
    for sample in samples::samples() {
        let mut view = view(&sample);
        let mut links = Vec::new();
        for node in view.tree().nodes() {
            if node.role == Role::Link {
                assert!(node.link.is_some() || node.external.is_some(), "{}: link {:?}", sample.name, node.name);
                links.push(node.id);
            }
        }
        for node in links {
            if let Some(address) = temper_client_view::href(&mut view, node) {
                assert!(!address.is_empty(), "{}: empty href for {node:?}", sample.name);
            } else if let Some(temper_client_domain::Action::Forge(temper_client_domain::forge::Act::Install {
                forge,
                ..
            })) = temper_client_view::act(&mut view, node, skein_ui::Act::Press)
            {
                // The existing picker link starts an asynchronous install request;
                // it has no static destination until the forge answers.
                assert_eq!(sample.name, "github-picker");
                assert_eq!(forge, 2);
            } else {
                panic!("{}: link {node:?} has neither a destination nor its install act", sample.name);
            }
        }
    }
}

// Expected words are the shipped view's; available counts are the worlds'
// live/closed windows, combined in the named Your chats list.
const SOLO_LIVE: u32 = 2;
const SOLO_CLOSED: u32 = 1;
const REFERENCE_LIVE: u32 = 1;
const REFERENCE_CLOSED: u32 = 2;
const SOLO_PLAN_LIVE: u32 = 3;
struct Holds {
    name: &'static str,
    heading: &'static str,
    parts: &'static [Part],
}
enum Part {
    Named { role: Role, name: &'static str },
    Described { role: Role, name: &'static str, description: &'static str },
    Absent { role: Role, name: &'static str },
    Value { role: Role, name: &'static str, value: &'static str },
    Chats { live: u32, closed: u32 },
    Plan { live: u32, title: &'static str },
    PlanWithEnded { live: u32, ended: u32, before: &'static str, after: &'static str },
}
const HOLDS: [Holds; samples::N] = [
    Holds {
        name: "chat-first-run-7-T1",
        heading: "T1 · Add CSV export to widget",
        parts: &[Part::Named { role: Role::Status, name: "Closed 21:19" }],
    },
    Holds {
        name: "chats-first-run-5",
        heading: "Chats",
        parts: &[Part::Named { role: Role::Link, name: "Manage usage" }],
    },
    Holds {
        name: "welcome-paste-expired",
        heading: "Connect a model",
        parts: &[
            Part::Named { role: Role::Status, name: "This sign-in has ended." },
            Part::Named { role: Role::Button, name: "Continue with ChatGPT" },
            Part::Absent { role: Role::TextField, name: "Address of the page that couldn't be reached" },
        ],
    },
    Holds {
        name: "welcome-model-declined",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Button, name: "Continue with ChatGPT" }],
    },
    Holds {
        name: "welcome-model-renewed",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Button, name: "Continue with ChatGPT" }],
    },
    Holds {
        name: "welcome-model-refused",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Button, name: "Continue with ChatGPT" }],
    },
    Holds {
        name: "welcome-paste-waiting",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-written",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-too-long",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-sending",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-finishing",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Status, name: "Finishing your sign-in…" }],
    },
    Holds {
        name: "welcome-paste-address",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-state",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-too-many",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Button, name: "Continue with ChatGPT" }],
    },
    Holds {
        name: "welcome-paste-ended",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Button, name: "Continue with ChatGPT" }],
    },
    Holds {
        name: "welcome-paste-lost",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "welcome-paste-unsent",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Address of the page that couldn't be reached" }],
    },
    Holds {
        name: "forges-github-install",
        heading: "Forges",
        parts: &[Part::Named { role: Role::Button, name: "Install on GitHub" }],
    },
    Holds {
        name: "forge-admin-details",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Region, name: "Your first repository" },
            Part::Named { role: Role::Heading { level: 3 }, name: "Your first repository" },
            Part::Named { role: Role::TextField, name: "Runner label" },
        ],
    },
    Holds {
        name: "forge-setup-failed",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Status, name: "failed" },
            Part::Named { role: Role::List, name: "Each step" },
            Part::Named { role: Role::Button, name: "Set up again" },
        ],
    },
    Holds {
        name: "welcome-setup-failed",
        heading: "Almost ready",
        parts: &[
            Part::Named { role: Role::Status, name: "failed" },
            Part::Named { role: Role::List, name: "Each step" },
            Part::Named { role: Role::Button, name: "Set up again" },
        ],
    },
    Holds {
        name: "welcome-model-key-details",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Key (pasted once)" }],
    },
    Holds {
        name: "welcome-model-signing-in",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Heading { level: 2 }, name: "Signing in to ChatGPT" }],
    },
    Holds {
        name: "welcome-model-signin-ready",
        heading: "Connect a model",
        parts: &[Part::Named {
            role: Role::Slot,
            name: "ChatGPT · pat@example.com is on the Accounts page, with its details.",
        }],
    },
    Holds {
        name: "github-app-details",
        heading: "Connect a forge",
        parts: &[Part::Named { role: Role::Region, name: "An App of your own" }],
    },
    Holds {
        name: "github-install",
        heading: "Connect a forge",
        parts: &[Part::Named { role: Role::Button, name: "Install on GitHub" }],
    },
    Holds {
        name: "github-picker",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Choice, name: "sam/widget" },
            Part::Named { role: Role::Button, name: "Adopt sam/widget" },
        ],
    },
    Holds {
        name: "github-adopted",
        heading: "Connect a forge",
        parts: &[Part::Named { role: Role::Link, name: "Open widget" }],
    },
    Holds {
        name: "welcome-forge-choose",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Region, name: "GitHub" },
            Part::Named { role: Role::Region, name: "Forgejo on this machine" },
            Part::Named { role: Role::Region, name: "A Forgejo elsewhere" },
        ],
    },
    Holds {
        name: "welcome-forge-details",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::TextField, name: "Organization" },
            Part::Named { role: Role::TextField, name: "Repository" },
        ],
    },
    Holds {
        name: "welcome-model-choose",
        heading: "Connect a model",
        parts: &[
            Part::Named { role: Role::Button, name: "Continue with ChatGPT" },
            Part::Named { role: Role::Link, name: "Use an API key" },
        ],
    },
    Holds {
        name: "forge-details",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::TextField, name: "Organization" },
            Part::Named { role: Role::TextField, name: "Repository" },
            Part::Named { role: Role::TextField, name: "Into a new project, named" },
        ],
    },
    Holds {
        name: "forge-setting-up-middle",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Status, name: "under way" },
            Part::Named { role: Role::List, name: "Each step" },
        ],
    },
    Holds {
        name: "welcome-ready-under-way",
        heading: "Almost ready",
        parts: &[
            Part::Named { role: Role::Disclosure, name: "Each step" },
            Part::Named { role: Role::Paragraph, name: "Project Playground, with work/hello" },
            Part::Named { role: Role::Button, name: "Open Playground" },
            Part::Named { role: Role::Paragraph, name: "It opens once work/hello is made." },
        ],
    },
    Holds {
        name: "welcome-ready-adopted",
        heading: "work/hello is ready",
        parts: &[
            Part::Named { role: Role::Paragraph, name: "Forge: Forgejo on this machine, connected as temper" },
            Part::Named { role: Role::Paragraph, name: "Project hello, with work/hello" },
            Part::Named { role: Role::Link, name: "Open hello" },
            Part::Absent { role: Role::Disclosure, name: "Each step" },
        ],
    },
    Holds {
        name: "connect-forge-first-run-6",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Heading { level: 2 }, name: "Setting up Forgejo on this machine" },
            Part::Named { role: Role::Status, name: "under way" },
            Part::Named { role: Role::Paragraph, name: "work/hello, into Playground" },
        ],
    },
    Holds {
        name: "forges-first-run-7",
        heading: "Forges",
        parts: &[
            Part::Named { role: Role::Heading { level: 2 }, name: "Your forges 1" },
            Part::Described {
                role: Role::Disclosure,
                name: "Forgejo on this machine",
                description: "forge 3 · Forgejo · Connected as temper\nwork/hello · project hello",
            },
            Part::Absent { role: Role::Heading { level: 2 }, name: "The playground" },
        ],
    },
    Holds {
        name: "connect-forge-choose",
        heading: "Connect a forge",
        parts: &[
            Part::Named { role: Role::Region, name: "GitHub" },
            Part::Named { role: Role::Region, name: "Forgejo on this machine" },
            Part::Named { role: Role::Region, name: "A Forgejo elsewhere" },
        ],
    },
    Holds {
        name: "chats-setup-strip",
        heading: "Chats",
        parts: &[
            Part::Named { role: Role::Region, name: "Setup" },
            Part::Named { role: Role::Paragraph, name: "temper has no forge and no model yet." },
            Part::Named { role: Role::Link, name: "Continue setting up" },
        ],
    },
    Holds {
        name: "welcome-first-run-0",
        heading: "Welcome to temper",
        parts: &[
            Part::Named { role: Role::List, name: "Steps" },
            Part::Named { role: Role::Region, name: "Try the playground" },
            Part::Named { role: Role::Button, name: "Try the playground" },
            Part::Named { role: Role::Region, name: "Set up temper" },
            Part::Named { role: Role::Link, name: "Set up temper" },
        ],
    },
    Holds {
        name: "chats-solo",
        heading: "Chats",
        parts: &[
            Part::Named { role: Role::Paragraph, name: "Why is tally sum slow on big files?" },
            Part::Chats { live: SOLO_LIVE, closed: SOLO_CLOSED },
        ],
    },
    Holds {
        name: "chat-solo-T10",
        heading: "T10 · Add a --json flag to tally report",
        parts: &[
            Part::Named { role: Role::Button, name: "Accept" },
            Part::Named { role: Role::Heading { level: 2 }, name: "JSON output for tally report" },
            Part::Described { role: Role::Disclosure, name: "2 tool calls", description: "read ×2" },
        ],
    },
    Holds {
        name: "inbox-solo",
        heading: "Inbox",
        parts: &[
            Part::Named { role: Role::Button, name: "Answer" },
            Part::Named { role: Role::Heading { level: 3 }, name: "JSON output for tally report" },
            Part::Named { role: Role::Button, name: "Accept" },
            Part::Named { role: Role::Paragraph, name: "The old parser accepts" },
        ],
    },
    Holds {
        name: "task-solo-T3",
        heading: "CSV import",
        parts: &[
            Part::Named { role: Role::Status, name: "waiting on an answer from you" },
            Part::Plan { live: SOLO_PLAN_LIVE, title: "CSV import: the quoted-field parser" },
            Part::Named { role: Role::Region, name: "Budget and authority" },
            Part::Named { role: Role::Paragraph, name: "5 spent · 15 left of 20" },
            Part::Named { role: Role::Term, name: "Funded by" },
            Part::Named { role: Role::Description, name: "Your pool" },
        ],
    },
    Holds {
        name: "accounts-solo",
        heading: "Accounts",
        parts: &[
            Part::Named { role: Role::Heading { level: 3 }, name: "Claude" },
            Part::Named { role: Role::Item, name: "Claude Code login claude-robin · Ready" },
            Part::Named { role: Role::Item, name: "API key · Ready" },
            Part::Named { role: Role::Paragraph, name: "laptop" },
        ],
    },
    Holds {
        name: "chats-first-run-1",
        heading: "What shall we change?",
        parts: &[
            Part::Named { role: Role::List, name: "Suggested requests" },
            Part::Named { role: Role::Button, name: "Add CSV export to widget" },
            Part::Named { role: Role::Button, name: "Add a dark theme to widget" },
            Part::Named { role: Role::Button, name: "Fix widget’s empty search state" },
        ],
    },
    Holds {
        name: "adoption",
        heading: "Adopt a repository into factory",
        parts: &[Part::Value { role: Role::TextField, name: "Owner", value: "" }],
    },
    Holds {
        name: "adoption-filled",
        heading: "Adopt a repository into factory",
        parts: &[
            Part::Named { role: Role::Button, name: "Adopt" },
            Part::Value { role: Role::TextField, name: "Owner", value: "acme" },
        ],
    },
    Holds {
        name: "adoption-refused",
        heading: "Adopt a repository into factory",
        parts: &[Part::Named { role: Role::Alert, name: "" }],
    },
    Holds {
        name: "chats-reference",
        heading: "Chats",
        parts: &[
            Part::Named { role: Role::Paragraph, name: "Director" },
            Part::Chats { live: REFERENCE_LIVE, closed: REFERENCE_CLOSED },
        ],
    },
    Holds {
        name: "chat-reference-T1",
        heading: "T1 · Director",
        parts: &[Part::Named { role: Role::Paragraph, name: "Check · two-hour timer" }],
    },
    Holds {
        name: "chat-solo-T6",
        heading: "T6 · Why is tally sum slow on big files?",
        parts: &[Part::Named { role: Role::Paragraph, name: "T7 found it" }],
    },
    Holds {
        name: "chat-first-run-2-T1",
        heading: "T1 · Add CSV export to widget",
        parts: &[
            Part::Named { role: Role::Link, name: "Task 2" },
            Part::Described { role: Role::Disclosure, name: "1 tool call", description: "wait" },
        ],
    },
    Holds {
        name: "chat-first-run-3-T1",
        heading: "T1 · Add CSV export to widget",
        parts: &[Part::Named { role: Role::Paragraph, name: "CSV export is available" }],
    },
    Holds {
        name: "chat-first-run-4-T4",
        heading: "T4 · Add a dark theme to widget",
        parts: &[
            Part::Named { role: Role::Paragraph, name: "I would give widget" },
            Part::Named { role: Role::TextArea, name: "Message" },
            Part::Absent { role: Role::Disclosure, name: "1 tool call" },
        ],
    },
    Holds {
        name: "inbox-reference",
        heading: "Inbox",
        parts: &[Part::Named { role: Role::Link, name: "T1 Director replied" }],
    },
    Holds {
        name: "accounts-reference",
        heading: "Accounts",
        parts: &[Part::Named { role: Role::Heading { level: 3 }, name: "Claude" }],
    },
    Holds {
        name: "accounts-first-run-1",
        heading: "Accounts",
        parts: &[Part::Named { role: Role::Disclosure, name: "Scripted model · the playground’s" }],
    },
    Holds {
        name: "google-sign-in",
        heading: "Sign in",
        parts: &[Part::Named { role: Role::Link, name: "Continue with Google" }],
    },
    Holds {
        name: "signed-out",
        heading: "",
        parts: &[Part::Named {
            role: Role::Paragraph,
            name: "Signed out. To sign in, open a sign-in link temper serve printed.",
        }],
    },
    Holds {
        name: "out-of-date",
        heading: "",
        parts: &[Part::Named {
            role: Role::Paragraph,
            name: "This temper is out of date: run the temper the engine runs.",
        }],
    },
    Holds {
        name: "chats-empty",
        heading: "Chats",
        parts: &[
            Part::Named { role: Role::Heading { level: 2 }, name: "Your chats" },
            Part::Chats { live: 0, closed: 0 },
        ],
    },
    Holds {
        name: "inbox-empty",
        heading: "Inbox",
        parts: &[
            Part::Named { role: Role::Heading { level: 2 }, name: "Needs you" },
            Part::Absent { role: Role::List, name: "" },
        ],
    },
    Holds {
        name: "chat-closing",
        heading: "T6 · Why is tally sum slow on big files?",
        parts: &[Part::Named { role: Role::Dialog, name: "Close" }],
    },
    Holds {
        name: "chat-rejecting",
        heading: "T10 · Add a --json flag to tally report",
        parts: &[
            Part::Named { role: Role::TextArea, name: "Reason" },
            Part::Named { role: Role::Heading { level: 2 }, name: "JSON output for tally report" },
            Part::Named { role: Role::Button, name: "Accept" },
        ],
    },
    Holds { name: "task-held", heading: "CSV import", parts: &[Part::Named { role: Role::Button, name: "Release" }] },
    Holds {
        name: "task-releasing",
        heading: "CSV import",
        parts: &[Part::Named { role: Role::Dialog, name: "Release" }],
    },
    Holds {
        name: "connect-model-choose",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Region, name: "Choose" }],
    },
    Holds {
        name: "connect-model-key-details",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Key (pasted once)" }],
    },
    Holds {
        name: "connect-model-program-details",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::TextField, name: "Name" }],
    },
    Holds {
        name: "connect-model-key-ready",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Status, name: "Added your OpenAI API key" }],
    },
    Holds {
        name: "connect-model-signin-ready",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Heading { level: 2 }, name: "Signed in to ChatGPT as pat@example.com" }],
    },
    Holds {
        name: "connect-model-declined",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Alert, name: "You signed in without plan usage." }],
    },
    Holds {
        name: "connect-model-renewed",
        heading: "Connect a model",
        parts: &[Part::Named {
            role: Role::Status,
            name: "That’s pat@example.com again. To add another account, sign out of ChatGPT in your browser first, then Continue with ChatGPT.",
        }],
    },
    Holds {
        name: "connect-model-refused",
        heading: "Connect a model",
        parts: &[Part::Named {
            role: Role::Alert,
            name: "Another person here holds that ChatGPT account. To use one of yours, sign out of ChatGPT in your browser first, then Continue with ChatGPT.",
        }],
    },
    Holds {
        name: "connect-model-signing-in",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Link, name: "Open the sign-in page" }],
    },
    Holds {
        name: "connect-model-key-lost",
        heading: "Connect a model",
        parts: &[Part::Named { role: Role::Alert, name: "" }],
    },
    Holds {
        name: "task-reference-T28",
        heading: "Build path: one cache per lane",
        parts: &[
            Part::Named { role: Role::Item, name: "Queued · 2nd" },
            Part::Named { role: Role::Slot, name: "2nd in the queue for main, after Task 26." },
            Part::Named {
                role: Role::Text,
                name: "PR #216 · acme/kit · temper/28/build-path-one-cache-per-lane → main",
            },
            Part::Named { role: Role::Paragraph, name: "CI at its head" },
            Part::Named { role: Role::Paragraph, name: "Passed" },
            Part::Named { role: Role::Paragraph, name: "No other gate." },
            Part::Named { role: Role::Text, name: "Repairs 0 of 3 · resolutions 0 of 3 · updates 0 of 3" },
            Part::Named { role: Role::Link, name: "View on forge ↗" },
            Part::Named { role: Role::Tree, name: "Its tasks" },
            Part::Named { role: Role::Paragraph, name: "Produce T28's branch" },
        ],
    },
    Holds {
        name: "task-reference-T24",
        heading: "Fake server: replay fixtures",
        parts: &[
            Part::Named { role: Role::Item, name: "Landed" },
            Part::Named { role: Role::Slot, name: "Landed · merge commit 4e1a9c7" },
            Part::Named {
                role: Role::Text, name: "PR #212 · acme/kit · temper/24/fake-server-replay-fixtures → main"
            },
            Part::Named { role: Role::Tree, name: "Its tasks" },
            Part::Named { role: Role::Paragraph, name: "Produce T24's branch" },
            Part::Absent { role: Role::Paragraph, name: "More of its ended tasks are not shown." },
        ],
    },
    Holds {
        name: "task-reference-T12",
        heading: "Harness",
        parts: &[
            Part::Named { role: Role::Paragraph, name: "Its delegates: 6 live, 3 ended." },
            Part::Named { role: Role::Button, name: "All" },
            Part::Named { role: Role::Button, name: "Live only" },
            Part::PlanWithEnded {
                live: 6,
                ended: 3,
                before: "Advise: the fake server's API",
                after: "Fake server: typed errors",
            },
        ],
    },
    Holds {
        name: "task-solo-T9",
        heading: "T9 · Produce T8's branch",
        parts: &[
            Part::Named { role: Role::Log, name: "Conversation" },
            Part::Described { role: Role::Disclosure, name: "3 tool calls", description: "read ×3" },
            Part::Described { role: Role::Disclosure, name: "3 tool calls", description: "edit ×3" },
            Part::Named { role: Role::Paragraph, name: "Errors carry their line now" },
        ],
    },
];
const SPENDS: [(&str, Role, &str); 11] = [
    ("chat-first-run-7-T1", Role::Paragraph, "0 spent · of 20"),
    ("chat-solo-T10", Role::Paragraph, "0 spent · of 40"),
    ("chat-reference-T1", Role::Paragraph, "6 spent · of 100"),
    ("chat-solo-T6", Role::Paragraph, "1 spent · of 40"),
    ("chat-first-run-2-T1", Role::Paragraph, "0 spent · of 20"),
    ("chat-first-run-3-T1", Role::Paragraph, "0 spent · of 20"),
    ("chat-first-run-4-T4", Role::Paragraph, "0 spent · of 20"),
    ("chat-closing", Role::Paragraph, "1 spent · of 40"),
    ("chat-rejecting", Role::Paragraph, "0 spent · of 40"),
    ("chats-reference", Role::Paragraph, "6 spent"),
    ("chats-solo", Role::Paragraph, "1 spent"),
];
#[test]
fn each_chat_reads_its_allotment() {
    for sample in samples::samples() {
        if sample.name.starts_with("chat-") || matches!(sample.name, "chats-reference" | "chats-solo") {
            let (_, role, spend) = SPENDS.iter().find(|row| row.0 == sample.name).expect("sample's spend row");
            let view = view(&sample);
            find(view.tree(), *role, spend, sample.name);
        }
    }
}
#[test]
fn no_state_reads_of_zero() {
    for sample in samples::samples() {
        let view = view(&sample);
        for node in view.tree().nodes() {
            let mut text = Vec::new();
            for run in &node.text {
                text.extend_from_slice(&run.text);
            }
            for text in [node.name.as_deref(), Some(text.as_slice())].into_iter().flatten() {
                // A space or end after zero: times (of 08:00) and fractions (of 0.5) are not allotments.
                assert!(
                    !text.windows(b"of 0 ".len()).any(|part| part == b"of 0 ") && !text.ends_with(b"of 0"),
                    "{}: zero allotment in {:?}",
                    sample.name,
                    String::from_utf8_lossy(text)
                );
            }
        }
    }
}
fn in_main(role: Role, name: &str) -> Find {
    Find::named(role, name.as_bytes())
        .within(Box::new([skein_fake_person::Target::Role { role: Role::Main, name: Box::new([]) }]))
}
fn find(tree: &skein_ui::Tree, role: Role, name: &str, sample: &str) -> skein_ui::NodeId {
    let mut face = TreeFace::new(tree);
    match face.find(&Find::named(role, name.as_bytes())) {
        Found::Node(node) => node,
        Found::Absent | Found::Pending => panic!("{sample}: missing {role:?} {name:?}"),
    }
}
fn plan_with_ended(tree: &skein_ui::Tree, live: u32, ended: u32, before: &str, after: &str, sample: &str) {
    let plan = find(tree, Role::Tree, "Plan", sample);
    let nodes = tree.nodes();
    let start = nodes.iter().position(|node| node.id == plan).expect("plan node");
    let end = start + usize::try_from(nodes[start].size).expect("plan subtree");
    let rows: Vec<_> = (start..end).filter(|index| nodes[*index].role == Role::TreeItem).collect();
    let window = driver::limits().client.domain.rows;
    assert_eq!(
        rows.len(),
        usize::try_from(live.min(window) + ended.min(window)).expect("row count"),
        "{sample}: live and ended plan rows"
    );
    let [before, after] = [before, after].map(|title| {
        rows.iter()
            .position(|start| {
                let end = *start + usize::try_from(nodes[*start].size).expect("row subtree");
                nodes[*start..end]
                    .iter()
                    .any(|node| node.role == Role::Paragraph && node.name.as_deref() == Some(title.as_bytes()))
            })
            .unwrap_or_else(|| panic!("{sample}: missing plan row {title:?}"))
    });
    assert!(before < after, "{sample}: lower-numbered ended row before live row");
}
#[test]
fn each_state_holds_what_its_name_says() {
    for sample in samples::samples() {
        let mut row = None;
        for holds in &HOLDS {
            if holds.name == sample.name {
                assert!(row.is_none(), "duplicate holds row");
                row = Some(holds);
            }
        }
        let row = row.expect("every sample has a HOLDS row");
        let view = view(&sample);
        let tree = view.tree();
        let mut main = TreeFace::new(tree);
        let h1 = in_main(Role::Heading { level: 1 }, row.heading);
        if row.heading.is_empty() {
            assert_eq!(main.find(&h1), Found::Absent, "{} has no main heading", sample.name);
        } else {
            assert!(matches!(main.find(&h1), Found::Node(_)), "{}: heading {}", sample.name, row.heading);
        }
        // Count-only names may repeat; described parts follow conversation order.
        let mut described = tree.nodes().iter();
        for part in row.parts {
            match part {
                Part::Named { role, name } => {
                    find(tree, *role, name, sample.name);
                }
                Part::Absent { role, name } => {
                    assert_eq!(
                        main.find(&in_main(*role, name)),
                        Found::Absent,
                        "{}: absent {role:?} {name:?}",
                        sample.name
                    );
                }
                Part::Value { role, name, value } => {
                    let node = find(tree, *role, name, sample.name);
                    let value = value.as_bytes();
                    assert_eq!(
                        tree.find(node).expect("field").value.as_ref().expect("field value").text.as_ref(),
                        value,
                        "{}",
                        sample.name
                    );
                }
                Part::Described { role, name, description } => {
                    let node = described
                        .find(|node| node.role == *role && node.name.as_deref() == Some(name.as_bytes()))
                        .unwrap_or_else(|| panic!("{}: missing {role:?} {name:?}", sample.name));
                    assert_eq!(node.description.as_deref(), Some(description.as_bytes()), "{}", sample.name);
                }
                Part::Chats { live, closed } => {
                    let available = (*live).min(driver::limits().client.domain.rows)
                        + (*closed).min(driver::limits().client.domain.rows);
                    if available == 0 {
                        assert_eq!(main.find(&in_main(Role::List, "Your chats")), Found::Absent, "{}", sample.name);
                        continue;
                    }
                    let list = find(tree, Role::List, "Your chats", sample.name);
                    let nodes = tree.nodes();
                    let start = nodes.iter().position(|node| node.id == list).expect("chats list");
                    let end = start + usize::try_from(nodes[start].size).expect("chats subtree");
                    let mut count = 0;
                    for node in &nodes[start..end] {
                        if node.role == Role::Link
                            && let Some(name) = &node.name
                            && name
                                .strip_prefix(b"T")
                                .is_some_and(|number| !number.is_empty() && number.iter().all(u8::is_ascii_digit))
                        {
                            count += 1;
                        }
                    }
                    assert_eq!(count, available, "{}: windowed chat rows", sample.name);
                }
                Part::PlanWithEnded { live, ended, before, after } => {
                    plan_with_ended(tree, *live, *ended, before, after, sample.name);
                }
                Part::Plan { live, title } => {
                    let plan = find(tree, Role::Tree, "Plan", sample.name);
                    let row = Find::named(Role::Paragraph, title.as_bytes()).within(Box::new([
                        skein_fake_person::Target::Role { role: Role::Tree, name: Box::from(b"Plan".as_slice()) },
                    ]));
                    assert!(matches!(main.find(&row), Found::Node(_)), "{}: delegate title", sample.name);
                    let nodes = tree.nodes();
                    let start = nodes.iter().position(|node| node.id == plan).expect("plan node");
                    let end = start + usize::try_from(nodes[start].size).expect("plan subtree");
                    let count = nodes[start..end].iter().filter(|node| node.role == Role::TreeItem).count();
                    let window = driver::limits().client.domain.rows;
                    assert_eq!(
                        count,
                        usize::try_from((*live).min(window)).expect("row count"),
                        "{}: plan window",
                        sample.name
                    );
                }
            }
        }
    }
}
#[test]
fn print_writes_every_page_and_the_bundle() {
    let scratch = Scratch::new("temper-client-gallery-print");
    let out = scratch.path().join("gallery");
    print(&out).expect("print gallery");
    let index = fs::read_to_string(out.join("index.html")).expect("index");
    let bundle = out.join("bundle");
    let mut digest = None;
    for entry in fs::read_dir(&bundle).expect("bundle") {
        let entry = entry.expect("bundle entry");
        if entry.file_type().expect("file type").is_dir() {
            assert!(digest.is_none(), "one digest");
            digest = Some(entry.file_name().into_string().expect("digest ASCII"));
        }
    }
    let digest = digest.expect("digest directory");
    for file in ["brand.svg", "menu.svg", "send.svg"] {
        assert!(bundle.join(&digest).join("files").join(file).is_file(), "bundled decorative image {file}");
    }
    for file in ["skein-ui.css", "temper.css", "task-pages.css"] {
        assert!(bundle.join(&digest).join(file).is_file(), "bundled {file}");
    }
    let mut position = 0;
    for sample in samples::samples() {
        let link = format!("href=\"{}.html\"", sample.name);
        assert_eq!(index.matches(&link).count(), 1, "one link for {}", sample.name);
        let at = index.find(&link).expect("index link");
        assert!(at >= position, "table order");
        position = at + link.len();
        let page = fs::read_to_string(out.join(format!("{}.html", sample.name))).expect("sample page");
        if matches!(sample.address, temper_client_domain::Address::Welcome { .. }) {
            assert!(!page.contains("<summary>Menu</summary>"), "welcome has a bare frame");
        } else {
            assert!(page.contains("class=\"temper-menu\""), "{}: menu disclosure", sample.name);
            assert!(page.contains("<summary>Menu</summary>"), "{}: menu name", sample.name);
        }
        let mut position = 0;
        for file in ["skein-ui.css", "temper.css", "task-pages.css"] {
            let link = format!("href=\"bundle/{digest}/{file}\"");
            let at = page.find(&link).expect("stylesheet linked");
            assert!(at >= position, "stylesheet order");
            position = at + link.len();
        }
    }
    let signin = fs::read_to_string(out.join("connect-model-signing-in.html")).expect("sign-in page");
    assert!(signin.contains("href=\"https://issuer.example/visit/gallery-sign-in\""), "external sign-in href");
    let closing = fs::read_to_string(out.join("chat-closing.html")).expect("confirmation page");
    assert!(closing.contains("<dialog open "), "static confirmation opens");
    let chats = fs::read_to_string(out.join("chats-solo.html")).expect("chats page");
    assert!(chats.contains("<title>Chats · temper</title>"), "shell heading title");
    let signed_out = fs::read_to_string(out.join("signed-out.html")).expect("signed-out page");
    assert!(signed_out.contains("<title>temper</title>"), "shell title without heading");
    // Reprinting replaces the whole owned directory, including stale files.
    let stale = out.join("stale.html");
    fs::write(&stale, b"obsolete").expect("stale page");
    print(&out).expect("replace gallery");
    assert!(!stale.exists(), "old output removed");
    let mut expected = Vec::new();
    for path in STYLESHEETS {
        expected.push(Path::new(path).file_name().expect("stylesheet basename").to_str().expect("ASCII"));
    }
    let mut actual = Vec::new();
    for line in include_str!("../../../scripts/bundle.sh").lines() {
        let line = line.trim();
        if line.starts_with("--stylesheet ") {
            let (_, path) = line.split_once("$shell/").expect("shell stylesheet");
            actual.push(path.split('"').next().expect("basename"));
        }
    }
    assert_eq!(expected, actual);
}
#[test]
fn print_refuses_a_directory_not_its_own() {
    let scratch = Scratch::new("temper-client-gallery-refusal");
    let file = scratch.path().join("precious");
    fs::write(&file, b"keep me").expect("foreign file");
    match print(scratch.path()) {
        Err(Refusal::NotGallery { out }) => assert_eq!(out, scratch.path()),
        other => panic!("expected NotGallery, got {other:?}"),
    }
    assert_eq!(fs::read(&file).expect("file untouched"), b"keep me");
    assert_eq!(fs::read_dir(scratch.path()).expect("directory untouched").count(), 1);
}

#[test]
fn times_read_against_each_mockup_worlds_fixed_now() {
    for (name, expected) in [
        ("chats-solo", "1 h 28 min ago"),
        ("chats-reference", "50 min ago"),
        ("inbox-solo", "Waiting 12 min"),
        ("inbox-solo", "Waiting 18 min"),
        ("chat-solo-T10", "waiting 12 min"),
        ("chat-reference-T1", "Today · Thursday 22 October"),
        ("chat-solo-T10", "Today · Tuesday 27 October"),
        ("chat-first-run-4-T4", "21:05"),
    ] {
        let sample = samples::samples().into_iter().find(|sample| sample.name == name).expect("named sample");
        let view = view(&sample);
        assert!(
            view.tree()
                .nodes()
                .iter()
                .any(|node| node.text.iter().any(|run| String::from_utf8_lossy(&run.text).contains(expected))),
            "{name}: {expected}"
        );
    }
}

#[test]
fn gallery_chat_clocks_are_separate_leaves_under_the_speakers() {
    for (name, clock) in [("chat-reference-T1", "06:56"), ("chat-solo-T10", "09:18")] {
        let sample = samples::samples().into_iter().find(|sample| sample.name == name).expect("sample");
        let view = view(&sample);
        let tree = view.tree();
        let who = tree
            .nodes()
            .iter()
            .enumerate()
            .find(|(_, node)| node.styles.as_slice().contains(&temper_client_view::look::WHO))
            .expect("speaker")
            .0;
        assert_eq!(tree.nodes()[who + 1].name.as_deref(), Some(b"You".as_slice()));
        let clock_at = who + 1 + usize::try_from(tree.nodes()[who + 1].size).expect("speaker size");
        assert_eq!(tree.nodes()[clock_at].role, Role::Time);
        assert_eq!(tree.nodes()[clock_at].name.as_deref(), Some(clock.as_bytes()));
    }
}

#[test]
fn model_samples_link_the_mockup_state_that_draws_them() {
    let rows = [
        ("connect-model-choose", Some("connect-model.html?world=reference&bare")),
        ("connect-model-key-details", Some("connect-model.html?world=reference&way=key&bare")),
        ("connect-model-program-details", Some("connect-model.html?world=reference&way=program&bare")),
        ("connect-model-signing-in", None),
        ("connect-model-key-ready", None),
        ("connect-model-signin-ready", Some("connect-model.html?world=first-run&step=5&way=chatgpt&bare")),
        (
            "connect-model-declined",
            Some("connect-model.html?world=first-run&step=5&way=chatgpt&bare#other-states-declined"),
        ),
        (
            "connect-model-renewed",
            Some("connect-model.html?world=first-run&step=5&way=chatgpt&bare#other-states-renewed"),
        ),
        (
            "connect-model-refused",
            Some("connect-model.html?world=first-run&step=5&way=chatgpt&bare#other-states-refused"),
        ),
        ("connect-model-key-lost", None),
    ];
    let models: Vec<_> =
        samples::samples().into_iter().filter(|sample| sample.name.starts_with("connect-model-")).collect();
    assert_eq!(models.len(), rows.len(), "every model sample has a row");
    for (sample, (name, expected)) in models.iter().zip(rows) {
        assert_eq!(sample.name, name);
        assert_eq!(samples::mockup(sample).as_deref(), expected, "{name}");
    }
}

#[test]
fn connection_samples_assign_priority_and_progress_to_the_bundled_nodes() {
    use temper_client_view::look;
    for (name, expected) in [
        ("welcome-forge-choose", vec!["Connect GitHub"]),
        ("welcome-forge-details", vec!["Set up a Forgejo here"]),
        ("welcome-model-choose", vec!["Continue with ChatGPT"]),
        ("github-app-details", vec!["Connect the App"]),
        ("github-install", vec!["Install on GitHub"]),
        ("github-picker", vec!["Adopt sam/widget"]),
        ("github-adopted", vec!["Open widget"]),
        ("forges-github-install", vec!["Install on GitHub"]),
    ] {
        let sample = samples::samples().into_iter().find(|sample| sample.name == name).expect("sample");
        let view = view(&sample);
        let primary: Vec<_> = view
            .tree()
            .nodes()
            .iter()
            .filter(|node| node.styles.as_slice().contains(&look::FLOW_PRIMARY))
            .map(|node| std::str::from_utf8(node.name.as_deref().expect("primary name")).expect("UTF-8"))
            .collect();
        assert_eq!(primary, expected, "{name}");
        if name == "welcome-forge-details" {
            assert_eq!(
                view.tree().nodes().iter().filter(|node| node.styles.as_slice().contains(&look::FLOW_PAIR)).count(),
                2
            );
        }
    }
    let sample =
        samples::samples().into_iter().find(|sample| sample.name == "forge-setting-up-middle").expect("sample");
    let view = view(&sample);
    let progress = view
        .tree()
        .nodes()
        .iter()
        .position(|node| node.styles.as_slice().contains(&look::FLOW_PROGRESS))
        .expect("progress");
    let nodes = view.tree().nodes();
    let end = progress + usize::try_from(nodes[progress].size).expect("subtree size");
    let items: Vec<_> = nodes[progress..end].iter().filter(|node| node.role == Role::Item).collect();
    assert_eq!(items.len(), 5);
    assert!(items[0].styles.as_slice().contains(&look::FLOW_DONE));
    assert!(items[1].states.as_slice().contains(&skein_ui::State::Current));
    assert!(items[2].styles.is_empty());
    assert!(items[2].states.is_empty());
}
