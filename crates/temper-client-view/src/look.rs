//! temper's style tokens and their names (client/browser.md, 2.5), as jig's `jig-client-shell-web` `look.rs`.
use crate::frame::owner;
use skein_ui::{Style, look::Names};
/// A folded account.
pub const ACCOUNT_ROW: Style = Style { owner: owner(), token: 124 };
/// The frame's banner.
pub const BANNER: Style = Style { owner: owner(), token: 0 };
/// The frame's navigation.
pub const NAVIGATION: Style = Style { owner: owner(), token: 1 };
/// Every page's main.
pub const MAIN: Style = Style { owner: owner(), token: 2 };
/// A chat's cards and departures.
pub const CARD: Style = Style { owner: owner(), token: 3 };
/// Each line of a chat's conversation.
pub const TURN: Style = Style { owner: owner(), token: 4 };
/// The chats' and a chat's composers.
pub const COMPOSER: Style = Style { owner: owner(), token: 5 };
/// The banner's wordmark.
pub const BRAND: Style = Style { owner: owner(), token: 6 };
/// The live link status.
pub const LIVE: Style = Style { owner: owner(), token: 7 };
/// The connecting link status.
pub const CONNECTING: Style = Style { owner: owner(), token: 8 };
/// The offline link status.
pub const OFFLINE: Style = Style { owner: owner(), token: 9 };
/// The link region's behind status.
pub const BEHIND: Style = Style { owner: owner(), token: 10 };
/// A chats section's list.
pub const ROWS: Style = Style { owner: owner(), token: 11 };
/// A chats row.
pub const ROW: Style = Style { owner: owner(), token: 12 };
/// The chats' composer box.
pub const ENTRY: Style = Style { owner: owner(), token: 13 };
/// A chat waiting for the reader.
pub const STATUS_YOURS: Style = Style { owner: owner(), token: 14 };
/// A held chat.
pub const STATUS_HELD: Style = Style { owner: owner(), token: 15 };
/// A retrying chat.
pub const STATUS_RETRYING: Style = Style { owner: owner(), token: 16 };
/// A chat starting or doing work.
pub const STATUS_ACTIVE: Style = Style { owner: owner(), token: 17 };
/// A chat waiting, parked, closing or ended.
pub const STATUS_QUIET: Style = Style { owner: owner(), token: 18 };
/// A chats row's absolute UTC time.
pub const TIME: Style = Style { owner: owner(), token: 19 };
/// The start composer's keyboard hint.
pub const HINT: Style = Style { owner: owner(), token: 20 };
/// The banner's corner: link state, person and sign out.
pub const CORNER: Style = Style { owner: owner(), token: 21 };
/// The accounts' banner: no usable account, its words and acts.
pub const ACCOUNTS: Style = Style { owner: owner(), token: 22 };
/// The composer's account and model line.
pub const ACCOUNT_LINE: Style = Style { owner: owner(), token: 23 };
/// The conversation page main.
pub const CHAT: Style = Style { owner: owner(), token: 24 };
/// The chat identity and controls.
pub const CHATBAR: Style = Style { owner: owner(), token: 25 };
/// The conversation and its tail.
pub const THREAD: Style = Style { owner: owner(), token: 26 };
/// One message in a conversation turn.
pub const MESSAGE: Style = Style { owner: owner(), token: 27 };
/// A message spoken by a person.
pub const BY_PERSON: Style = Style { owner: owner(), token: 28 };
/// A message spoken by the agent.
pub const BY_AGENT: Style = Style { owner: owner(), token: 29 };
/// A message spoken by a tool.
pub const BY_TOOL: Style = Style { owner: owner(), token: 30 };
/// A message containing instructions.
pub const BY_SYSTEM: Style = Style { owner: owner(), token: 31 };
/// The speaker and message state.
pub const WHO: Style = Style { owner: owner(), token: 32 };
/// The contents of a message.
pub const SAYS: Style = Style { owner: owner(), token: 33 };
/// A tool call or result line.
pub const WORK: Style = Style { owner: owner(), token: 34 };
/// A run or omission marker.
pub const MARKER: Style = Style { owner: owner(), token: 35 };
/// The chat proposal and departure list.
pub const CARDS: Style = Style { owner: owner(), token: 36 };
/// The chat composer and account area.
pub const COMPOSER_AREA: Style = Style { owner: owner(), token: 37 };
/// A list page main.
pub const PAGE: Style = Style { owner: owner(), token: 38 };
/// The page breadcrumb bar.
pub const PAGEBAR: Style = Style { owner: owner(), token: 39 };
/// A section heading row.
pub const HEADING: Style = Style { owner: owner(), token: 40 };
/// The heading row filters.
pub const FILTERS: Style = Style { owner: owner(), token: 41 };
/// The inbox updates list.
pub const UPDATES: Style = Style { owner: owner(), token: 42 };
/// A card kind and path.
pub const META: Style = Style { owner: owner(), token: 43 };
/// A card title.
pub const TITLE: Style = Style { owner: owner(), token: 44 };
/// A card supplied facts.
pub const DETAILS: Style = Style { owner: owner(), token: 45 };
/// An inbox proposal.
pub const KIND_PROPOSAL: Style = Style { owner: owner(), token: 46 };
/// An inbox question.
pub const KIND_QUESTION: Style = Style { owner: owner(), token: 47 };
/// An inbox held task.
pub const KIND_HELD: Style = Style { owner: owner(), token: 48 };
/// An inbox person task.
pub const KIND_PERSON_TASK: Style = Style { owner: owner(), token: 49 };
/// An inbox result.
pub const KIND_RESULT: Style = Style { owner: owner(), token: 50 };
/// An inbox reply.
pub const KIND_REPLY: Style = Style { owner: owner(), token: 51 };
/// The task page header region.
pub const TASK_HEADING: Style = Style { owner: owner(), token: 52 };
/// A task page card region.
pub const TASK_CARD: Style = Style { owner: owner(), token: 53 };
/// The plan tree.
pub const PLAN: Style = Style { owner: owner(), token: 54 };
/// One plan row's own parts.
pub const PLAN_ROW: Style = Style { owner: owner(), token: 55 };
/// The list around a held row card.
pub const PLAN_HOLD: Style = Style { owner: owner(), token: 56 };
/// The task activity list.
pub const EVENTS: Style = Style { owner: owner(), token: 57 };
/// A done task phase or a settled account outcome.
pub const STATUS_DONE: Style = Style { owner: owner(), token: 58 };
/// The next-run model choice block.
pub const CHOICE: Style = Style { owner: owner(), token: 59 };
/// The accounts page body, separate from the banner notice.
pub const ACCOUNTS_PAGE: Style = Style { owner: owner(), token: 60 };
/// The repository adoption page body.
pub const ADOPTION_PAGE: Style = Style { owner: owner(), token: 61 };
/// The demo heading, request, suggestions and script note.
pub const INVITATION: Style = Style { owner: owner(), token: 62 };
/// A supplied refusal, still announced as a status.
pub const REFUSAL: Style = Style { owner: owner(), token: 63 };
/// A change's steps on its delegation card.
pub const STEPS: Style = Style { owner: owner(), token: 64 };
/// One step on a change's delegation card.
pub const STEP: Style = Style { owner: owner(), token: 65 };
/// A completed step on a change's delegation card.
pub const STEP_DONE: Style = Style { owner: owner(), token: 66 };
/// The demo's project, repository and signed-in role.
pub const CONTEXT: Style = Style { owner: owner(), token: 67 };
/// The existing guidance when the application is unavailable.
pub const UNAVAILABLE: Style = Style { owner: owner(), token: 68 };
/// The result a goal gives.
pub const TASK_CONTRACT: Style = Style { owner: owner(), token: 69 };
/// A goal's budget and authority.
pub const TASK_BUDGET: Style = Style { owner: owner(), token: 70 };
/// A plan row's spend.
pub const PLAN_SPEND: Style = Style { owner: owner(), token: 71 };
/// A plan row's dependency line.
pub const DEPENDENCIES: Style = Style { owner: owner(), token: 72 };
/// A task waiting on its dependencies, delegates, an answer or news.
pub const STATUS_WAITING: Style = Style { owner: owner(), token: 73 };
/// An agent task's parent and own identity.
pub const AGENT_LINEAGE: Style = Style { owner: owner(), token: 74 };
/// An agent task's specification, requester and dependencies.
pub const AGENT_BRIEF: Style = Style { owner: owner(), token: 75 };
/// An agent task's current or last run heading.
pub const AGENT_RUN: Style = Style { owner: owner(), token: 76 };
/// An ended agent task's result.
pub const AGENT_RESULT: Style = Style { owner: owner(), token: 77 };
/// The demo context line's repository.
pub const REPOSITORY: Style = Style { owner: owner(), token: 78 };
/// A goal's offered acts (client/README.md, 5.11).
pub const TASK_ACTS: Style = Style { owner: owner(), token: 79 };
/// The agent task composer's steering note (client/README.md, 5.11).
pub const AGENT_STEER: Style = Style { owner: owner(), token: 80 };
/// The goal toolbar's Cancel button (client/README.md, 5.11).
pub const TASK_DANGER: Style = Style { owner: owner(), token: 81 };
/// The task page's amendment editor (client/README.md, 5.11).
pub const TASK_AMEND: Style = Style { owner: owner(), token: 82 };
/// A chat's end line (client/README.md, 3.5).
pub const ENDED: Style = Style { owner: owner(), token: 83 };
/// The day before a chat's turns (client/README.md, 5.12).
pub const DAY: Style = Style { owner: owner(), token: 84 };
/// The banner's menu, folded by the browser at phone width.
pub const MENU: Style = Style { owner: owner(), token: 85 };
/// A change's card on its task page (client/README.md, 5.10).
pub const CHANGE: Style = Style { owner: owner(), token: 86 };
/// The tag naming a change in its task page's header (5.10).
pub const CHANGE_TAG: Style = Style { owner: owner(), token: 87 };
/// The step a change is at (5.10).
pub const STEP_CURRENT: Style = Style { owner: owner(), token: 88 };
/// The notice of a held change (5.10).
pub const CHANGE_HOLD: Style = Style { owner: owner(), token: 89 };
/// One of a change's gates, CI's among them (5.10).
pub const GATE: Style = Style { owner: owner(), token: 90 };
/// An ended plan row (5.8).
pub const PLAN_DONE: Style = Style { owner: owner(), token: 91 };
/// The navigation inbox count.
pub const NAV_COUNT: Style = Style { owner: owner(), token: 92 };
/// Labelled facts in a proposal card.
pub const PROPOSAL_FACTS: Style = Style { owner: owner(), token: 93 };
/// An empty list's words, in a dashed box.
pub const EMPTY: Style = Style { owner: owner(), token: 94 };
/// The proposal type above its title.
pub const PROPOSAL_KICKER: Style = Style { owner: owner(), token: 95 };
/// Inbox presentation (client/README.md, 3.5).
pub const INBOX_TAG: Style = Style { owner: owner(), token: 96 };
/// Inbox presentation (client/README.md, 3.5).
pub const SUBTLE_COUNT: Style = Style { owner: owner(), token: 97 };
/// Inbox presentation (client/README.md, 3.5).
pub const KIND_MENU: Style = Style { owner: owner(), token: 98 };
/// Inbox presentation (client/README.md, 3.5).
pub const WAIT: Style = Style { owner: owner(), token: 99 };
/// Inbox presentation (client/README.md, 3.5).
pub const UPDATE_TIME: Style = Style { owner: owner(), token: 100 };
/// Centred first-run column.
pub const WELCOME: Style = Style { owner: owner(), token: 101 };
/// Four wizard steps.
pub const WELCOME_STEPS: Style = Style { owner: owner(), token: 102 };
/// One wizard step, marked when current.
pub const WELCOME_STEP: Style = Style { owner: owner(), token: 103 };
/// The playground and setup cards.
pub const WELCOME_CHOICES: Style = Style { owner: owner(), token: 104 };
/// One first-run choice.
pub const WELCOME_CARD: Style = Style { owner: owner(), token: 105 };
/// The step’s introduction.
pub const WELCOME_LEAD: Style = Style { owner: owner(), token: 106 };
/// A choice’s cost and time.
pub const WELCOME_MUTED: Style = Style { owner: owner(), token: 107 };
/// The closing invitation to connect real backends.
pub const WELCOME_AFTER: Style = Style { owner: owner(), token: 108 };
/// Back, Later and the playground alternative.
pub const WELCOME_FOOT: Style = Style { owner: owner(), token: 109 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_STEPS: Style = Style { owner: owner(), token: 110 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_STEP: Style = Style { owner: owner(), token: 111 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_DONE: Style = Style { owner: owner(), token: 112 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_CARD: Style = Style { owner: owner(), token: 113 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_MUTED: Style = Style { owner: owner(), token: 114 };
/// Shared flow presentation (client/README.md, 5.2).
pub const FLOW_PROGRESS: Style = Style { owner: owner(), token: 115 };
/// A navigation entry needs the person.
pub const NAV_NEEDS: Style = Style { owner: owner(), token: 116 };
/// A navigation entry is limited.
pub const NAV_LIMITED: Style = Style { owner: owner(), token: 117 };
/// The forge connection flow's main.
pub const CONNECT_FORGE: Style = Style { owner: owner(), token: 118 };
/// A configuration entry or terminal command.
pub const FLOW_CODE: Style = Style { owner: owner(), token: 119 };
/// The Forges page (client/forge.md, 5.4).
pub const FORGES: Style = Style { owner: owner(), token: 120 };
/// A forge's row on the Forges page (client/forge.md, 5.4).
pub const FORGE_ROW: Style = Style { owner: owner(), token: 121 };
/// The setup strip (client/README.md, 5.1, 5.6).
pub const SETUP_STRIP: Style = Style { owner: owner(), token: 122 };
/// Browser sign-in page (protocol/sign-in.md, 7).
pub const SIGN_IN: Style = Style { owner: owner(), token: 123 };
/// A connection flow's filled primary control.
pub const FLOW_PRIMARY: Style = Style { owner: owner(), token: 125 };
/// A failed line in a connection flow's progress.
pub const FLOW_FAILED: Style = Style { owner: owner(), token: 126 };
/// Organization and repository fields on one responsive row.
pub const FLOW_PAIR: Style = Style { owner: owner(), token: 127 };
/// The opening welcome keeps its introductory heading scale.
pub const WELCOME_INTRO: Style = Style { owner: owner(), token: 128 };
/// The navigation's project label when no project remains.
pub const PROJECT_EMPTY: Style = Style { owner: owner(), token: 129 };
/// A neutral small action.
pub const FLOW_SECONDARY: Style = Style { owner: owner(), token: 130 };
/// A quiet terracotta text link.
pub const FLOW_QUIET: Style = Style { owner: owner(), token: 131 };
/// The forge choices, three across on desktop.
pub const FLOW_CHOICES: Style = Style { owner: owner(), token: 132 };
/// A choice card with bottom-aligned actions.
pub const FLOW_CHOICE: Style = Style { owner: owner(), token: 133 };
/// A stacked choice action group.
pub const FLOW_ACTIONS: Style = Style { owner: owner(), token: 134 };
/// A small muted setup field label.
pub const FLOW_FIELD: Style = Style { owner: owner(), token: 135 };
/// The setup notes as muted bullet points.
pub const FLOW_FACTS: Style = Style { owner: owner(), token: 136 };
/// The model choices, two across on desktop.
pub const MODEL_CHOICES: Style = Style { owner: owner(), token: 137 };
/// A connection details card, separate from Model’s outer Choose card.
pub const FLOW_DETAILS: Style = Style { owner: owner(), token: 138 };
/// The API key card limits its contents to the reference's 420 px maximum.
pub const MODEL_KEY: Style = Style { owner: owner(), token: 139 };
/// The configured admin's first repository group.
pub const ADMIN_FIRST: Style = Style { owner: owner(), token: 140 };
/// The first repository group's legend.
pub const ADMIN_LEGEND: Style = Style { owner: owner(), token: 141 };
/// The Model ending’s definition list, scoped away from Accounts and Forges.
pub const MODEL_FACTS: Style = Style { owner: owner(), token: 142 };
/// The Model ending’s wrapping row of actions.
pub const MODEL_ACTIONS: Style = Style { owner: owner(), token: 143 };
/// A muted repository line tight beneath its setup heading.
pub const FLOW_SUBTITLE: Style = Style { owner: owner(), token: 144 };
/// The muted description of a Model way’s Details.
pub const MODEL_DESCRIPTION: Style = Style { owner: owner(), token: 145 };
/// Native terms and definitions inside the Model ending’s list.
pub const MODEL_FACT: Style = Style { owner: owner(), token: 146 };
/// The Model sign-in’s external address and folded paste.
pub const MODEL_SIGN_IN: Style = Style { owner: owner(), token: 147 };
/// Ready’s evenly spaced checklist.
pub const WELCOME_CHECKLIST: Style = Style { owner: owner(), token: 148 };
/// Ready’s sign-in ending aligned with the checklist edge.
pub const WELCOME_END: Style = Style { owner: owner(), token: 149 };
/// The small note below Ready’s disabled action.
pub const WELCOME_TINY: Style = Style { owner: owner(), token: 150 };
/// Model’s muted forge status above its heading.
pub const WELCOME_STATUS: Style = Style { owner: owner(), token: 151 };
/// A private repository's tag in the first-run picker.
pub const REPOSITORY_PRIVATE: Style = Style { owner: owner(), token: 152 };
/// A repository radio, its native label and optional privacy tag on one row.
pub const REPOSITORY_CHOICE: Style = Style { owner: owner(), token: 153 };
/// Ready's project button and optional Forgejo link.
pub const WELCOME_ACTIONS: Style = Style { owner: owner(), token: 154 };
/// The selected repository row in the first-run picker.
pub const REPOSITORY_SELECTED: Style = Style { owner: owner(), token: 155 };
/// The setup card's forge and model lines in one compact block.
pub const WELCOME_SETUP_DESCRIPTION: Style = Style { owner: owner(), token: 156 };
/// The opening welcome’s software factory tagline.
pub const WELCOME_TAGLINE: Style = Style { owner: owner(), token: 157 };
/// Native browser elements for the admin group and Model’s definition list.
pub const ELEMENTS: &[(skein_ui::Owner, skein_ui::Role, Style, &str)] = &[
    (owner(), skein_ui::Role::Region, ADMIN_FIRST, "fieldset"),
    (owner(), skein_ui::Role::Slot, MODEL_FACTS, "dl"),
    (owner(), skein_ui::Role::Term, MODEL_FACT, "dt"),
    (owner(), skein_ui::Role::Description, MODEL_FACT, "dd"),
    (owner(), skein_ui::Role::Heading { level: 3 }, ADMIN_LEGEND, "legend"),
];
/// The look's names (skein's ui.md, 8): styles by token, regions by number.
pub const NAMES: Names<'static> = Names {
    owner: owner(),
    prefix: b"temper",
    styles: &[
        b"temper-banner",
        b"temper-navigation",
        b"temper-main",
        b"temper-card",
        b"temper-turn",
        b"temper-composer",
        b"temper-brand",
        b"temper-live",
        b"temper-connecting",
        b"temper-offline",
        b"temper-behind",
        b"temper-rows",
        b"temper-row",
        b"temper-entry",
        b"temper-status-yours",
        b"temper-status-held",
        b"temper-status-retrying",
        b"temper-status-active",
        b"temper-status-quiet",
        b"temper-time",
        b"temper-hint",
        b"temper-corner",
        b"temper-accounts",
        b"temper-account-line",
        b"temper-chat",
        b"temper-chatbar",
        b"temper-thread",
        b"temper-message",
        b"temper-by-person",
        b"temper-by-agent",
        b"temper-by-tool",
        b"temper-by-system",
        b"temper-who",
        b"temper-says",
        b"temper-work",
        b"temper-marker",
        b"temper-cards",
        b"temper-composer-area",
        b"temper-page",
        b"temper-pagebar",
        b"temper-heading",
        b"temper-filters",
        b"temper-updates",
        b"temper-meta",
        b"temper-title",
        b"temper-details",
        b"temper-kind-proposal",
        b"temper-kind-question",
        b"temper-kind-held",
        b"temper-kind-person-task",
        b"temper-kind-result",
        b"temper-kind-reply",
        b"temper-task-heading",
        b"temper-task-card",
        b"temper-plan",
        b"temper-plan-row",
        b"temper-plan-hold",
        b"temper-events",
        b"temper-status-done",
        b"temper-choice",
        b"temper-accounts-page",
        b"temper-adoption-page",
        b"temper-invitation",
        b"temper-refusal",
        b"temper-steps",
        b"temper-step",
        b"temper-step-done",
        b"temper-context",
        b"temper-unavailable",
        b"temper-task-contract",
        b"temper-task-budget",
        b"temper-plan-spend",
        b"temper-dependencies",
        b"temper-status-waiting",
        b"temper-agent-lineage",
        b"temper-agent-brief",
        b"temper-agent-run",
        b"temper-agent-result",
        b"temper-repository",
        b"temper-task-acts",
        b"temper-agent-steer",
        b"temper-task-danger",
        b"temper-task-amend",
        b"temper-ended",
        b"temper-day",
        b"temper-menu",
        b"temper-change",
        b"temper-change-tag",
        b"temper-step-current",
        b"temper-change-hold",
        b"temper-gate",
        b"temper-plan-done",
        b"temper-nav-count",
        b"temper-proposal-facts",
        b"temper-empty",
        b"temper-proposal-kicker",
        b"temper-inbox-tag",
        b"temper-subtle-count",
        b"temper-kind-menu",
        b"temper-wait",
        b"temper-update-time",
        b"temper-welcome",
        b"temper-welcome-steps",
        b"temper-welcome-step",
        b"temper-welcome-choices",
        b"temper-welcome-card",
        b"temper-welcome-lead",
        b"temper-welcome-muted",
        b"temper-welcome-after",
        b"temper-welcome-foot",
        b"temper-flow-steps",
        b"temper-flow-step",
        b"temper-flow-done",
        b"temper-flow-card",
        b"temper-flow-muted",
        b"temper-flow-progress",
        b"temper-nav-needs",
        b"temper-nav-limited",
        b"temper-connect-forge",
        b"temper-flow-code",
        b"temper-forges",
        b"temper-forge-row",
        b"temper-setup-strip",
        b"temper-sign-in",
        b"temper-account-row",
        b"temper-flow-primary",
        b"temper-flow-failed",
        b"temper-flow-pair",
        b"temper-welcome-intro",
        b"temper-project-empty",
        b"temper-flow-secondary",
        b"temper-flow-quiet",
        b"temper-flow-choices",
        b"temper-flow-choice",
        b"temper-flow-actions",
        b"temper-flow-field",
        b"temper-flow-facts",
        b"temper-model-choices",
        b"temper-flow-details",
        b"temper-model-key",
        b"temper-admin-first",
        b"temper-admin-legend",
        b"temper-model-facts",
        b"temper-model-actions",
        b"temper-flow-subtitle",
        b"temper-model-description",
        b"temper-model-fact",
        b"temper-model-sign-in",
        b"temper-welcome-checklist",
        b"temper-welcome-end",
        b"temper-welcome-tiny",
        b"temper-welcome-status",
        b"temper-repository-private",
        b"temper-repository-choice",
        b"temper-welcome-actions",
        b"temper-repository-selected",
        b"temper-welcome-setup-description",
        b"temper-welcome-tagline",
    ],
    regions: &[b"temper-link"],
};
