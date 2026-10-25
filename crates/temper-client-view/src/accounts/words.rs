//! Account English has its own phrase IDs (client/README.md, 5.5).
use crate::words::Words;
use alloc::boxed::Box;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{Form, Hole, HoleKind, HoleShape, Phrase, PhraseId, PhraseShape, PluralClass, Segment};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Word {
    UnusableOn,
    Models,
    LimitedBy,
    ResetUntil,
    ProgramNeedsSignIn,
    UsedBy,
    WaitingFor,
    NoUsers,
    NoOwn,
    Connect,
    Playground,
    PlaygroundAccount,
    PlaygroundWords,
    AddApiKey,
    Replace,
    Remove,
    ProgramSignOut,
    KeyFirst,
    CheckAgain,
    PlanLimited,
    SignedOutPlan,
    Command,
    PlanRuns,
    KeyAdded,
    KeyTitle,
    KeyTitleA,
    SignInDescription,
    PlanPays,
    ProgramDeclared,
    DeploymentPays,
    OwnPays,
    MisconfiguredPlan,
    HeldPlan,
    DeclinedPlan,
    SteppedAside,
    PaysFor,
    ReadyModel,
    ReadyAccount,
    StartChat,
    ProgramRun,
    KeyRuns,
    ProgramNote,
    ProgramDescription,
    Pasted,
    KeyDescription,
    BrowserMinute,
    ChatDescription,
    AllWays,
    SigningIn,
    Details,
    Choose,
    ConnectAdd,
    Accounts,
    Own,
    Deployment,
    Unknown,
    Empty,
    Provider,
    Account,
    Source,
    Place,
    Party,
    Project,
    DefaultUses,
    Uses,
    Plan,
    Key,
    SignIn,
    Program,
    None,
    Ready,
    Granted,
    Asking,
    Limited,
    NeedsSignIn,
    Misconfigured,
    Cancelled,
    Closing,
    Expiring,
    Attention,
    Spent,
    Cut,
    Settled,
    Signing,
    LastEnd,
    Made,
    Renewed,
    Declined,
    Later,
    Scope,
    Directory,
    File,
    Unloaded,
    Unreadable,
    Refused,
    Failed,
    Stalled,
    TooLarge,
    NotLimited,
    UsesRefused,
    KeyLong,
    NotStored,
    LimitAccounts,
    LimitHolder,
    LimitSources,
    LimitSignIns,
    Held,
    ProviderRefused,
    Subject,
    UnknownNumber,
    Numbers,
    Kind,
    NotStarted,
    Address,
    Lost,
    Unsent,
    InFlight,
    Confirmed,
    Unconfirmed,
    Saved,
    Changed,
    KeyEmpty,
    Selection,
    Rank,
    ValueAgain,
    ProviderField,
    AccountField,
    SourceField,
    KeyField,
    OwnChoice,
    DeploymentChoice,
    SignInAction,
    AddKey,
    ReplaceKey,
    SignOut,
    RemoveKey,
    RemoveAccount,
    Resume,
    UpAccount,
    DownAccount,
    UpSource,
    DownSource,
    Confirm,
    Cancel,
    ConfirmSignOut,
    ConfirmKey,
    ConfirmAccount,
    OmittedDisplay,
    UsageUntil,
    Removed,
    RemovedUnconfirmed,
    Number,
    Until,
    After,
    AccountsOmitted,
    SourcesOmitted,
    UsesOmitted,
    SignInsOmitted,
    OpenSignIn,
    UnsafeAddress,
    AddressLong,
    OwnHolder,
    DeploymentHolder,
    UnknownAccount,
    Worker,
    AwaitingLogins,
    NotInContact,
    SignedInOtherwise,
    SignedOut,
    Cleanup,
    Missing,
    Deadline,
    Exited,
    Unconfigured,
    ProgramsOmitted,
    AddAccount,
    AddKeyToAccount,
    ProgramField,
    Declare,
    ProgramInvalid,
    HeldDeployment,
    Separator,
    Continue,
    SignInBrand,
    SubscriptionWay,
    KeyWay,
    KeyWayOnly,
    KeyWayAn,
    Subscription,
    ApiKeyBrand,
    HeldBy,
    PlanSignIn,
    CodeLogin,
    ProgramName,
    ProgramCommand,
    MadeNamed,
    RenewedNamed,
    MadePlan,
    NavNeeds,
    NavLimited,
    Moving,
    NotSignedIn,
    NotOurs,
    MoveCancelled,
    Elsewhere,
    PasteDisclosure,
    PasteHistory,
    PasteInstructions,
    PasteField,
    PasteFinish,
    PasteSending,
    PasteFinishing,
    PasteLong,
    PasteAddress,
    PasteState,
    PasteTooMany,
    PasteEnded,
    PasteAgain,
    OpenPlanSignIn,
}
const ROWS: &[(Word, &[u8])] = &[
    (Word::UnusableOn, b"Can't be used on "),
    (Word::Models, b"Models"),
    (Word::LimitedBy, b"Limited by "),
    (Word::UsedBy, b"Used by"),
    (Word::WaitingFor, b"Waiting for it"),
    (Word::NoUsers, b"no task uses it now"),
    (Word::NoOwn, b"No model of yours yet."),
    (Word::Connect, b"Connect a model"),
    (Word::Playground, b"The playground"),
    (Word::PlaygroundAccount, "Scripted model · the playground’s, until a model of yours connects · held by the deployment".as_bytes()),
    (Word::PlaygroundWords, "It plays the playground’s scripts: it answers the suggested requests, and nothing else. It costs nothing, and nothing leaves this machine.".as_bytes()),
    (Word::AddApiKey, b"Add an API key"),
    (Word::Replace, b"Replace"),
    (Word::Remove, b"Remove"),
    (Word::ProgramSignOut, b"Sign out on every machine"),
    (Word::KeyFirst, b"Use the API key first"),
    (Word::CheckAgain, b"Check again"),
    (Word::PlanLimited, b"temper reached its usage limit on your ChatGPT plan. Tasks on it wait."),
    (Word::SignedOutPlan, b"Signed out of ChatGPT."),
    (Word::DeploymentPays, "Deployment work".as_bytes()),
    (Word::OwnPays, "Your own work".as_bytes()),
    (Word::PlanPays, "Your own work, and projects where you’re the only member".as_bytes()),
    (Word::MisconfiguredPlan, "OpenAI refused temper’s sign-in. Check the configuration’s registration.".as_bytes()),
    (Word::HeldPlan, "Another person here holds that ChatGPT account. To use one of yours, sign out of ChatGPT in your browser first, then Continue with ChatGPT.".as_bytes()),
    (Word::DeclinedPlan, "You signed in without plan usage.".as_bytes()),
    (Word::SteppedAside, "The playground’s scripted model stepped aside.".as_bytes()),
    (Word::PaysFor, "Pays for".as_bytes()),
    (Word::ReadyModel, "Model".as_bytes()),
    (Word::ReadyAccount, "Account".as_bytes()),
    (Word::StartChat, "Start a chat".as_bytes()),
    (Word::ProgramRun, "Run this on each host temper’s agents run on, in a terminal there:".as_bytes()),
    (Word::KeyRuns, "temper’s agents now run on it.".as_bytes()),
    (Word::ProgramNote, "Claude Code asks you to sign in. temper never reads its login.".as_bytes()),
    (Word::ProgramDescription, "temper runs Claude Code, signed in to your Claude subscription on each host its agents run on. Name the login, then sign in on each host, in a terminal there.".as_bytes()),
    (Word::Pasted, "Pasted here · at once".as_bytes()),
    (Word::BrowserMinute, "In a browser on the engine’s machine · about a minute".as_bytes()),
    (Word::AllWays, "‹ All ways".as_bytes()),
    (Word::SigningIn, "Signing in".as_bytes()),
    (Word::Details, "Details".as_bytes()),
    (Word::Choose, "Choose".as_bytes()),
    (Word::ConnectAdd, "+ Connect a model".as_bytes()),
    (Word::AccountsOmitted, b"Accounts left out: "),
    (Word::SourcesOmitted, b"Sources left out: "),
    (Word::UsesOmitted, b"Uses left out: "),
    (Word::SignInsOmitted, b"Sign-ins left out: "),
    (Word::OpenSignIn, b"Open the sign-in page"),
    (Word::UnsafeAddress, b"Sign-in address is not an http(s) address."),
    (Word::OwnHolder, b"Your own accounts"),
    (Word::DeploymentHolder, b"The deployment's accounts"),
    (Word::UnknownAccount, b"Account not shown"),
    (Word::Removed, b"Account removed; confirmed."),
    (
        Word::RemovedUnconfirmed,
        b"Account removed; revocation was not confirmed. Disconnect temper in the provider's settings.",
    ),
    (Word::Accounts, b"Accounts"),
    (Word::Own, b"Your accounts"),
    (Word::Deployment, b"Deployment accounts"),
    (Word::Unknown, b"Waiting for current account facts."),
    (Word::Empty, b"No accounts shown."),
    (Word::Provider, b"LLM provider "),
    (Word::Account, b"Account "),
    (Word::Source, b"Source "),
    (Word::Place, b"Order "),
    (Word::Party, b"Person "),
    (Word::Project, b"Project "),
    (Word::DefaultUses, b"Default uses"),
    (Word::Uses, b"Uses: "),
    (Word::Plan, b"Signed-in plan"),
    (Word::Key, b"API key"),
    (Word::SignIn, b"Sign-in"),
    (Word::Program, b"Program login"),
    (Word::None, b"none"),
    (Word::Ready, b"Ready"),
    (Word::Granted, b"Granted"),
    (Word::Asking, b"Checking"),
    (Word::Limited, b"Limited. Tasks on it wait."),
    (Word::NeedsSignIn, b"Needs sign-in"),
    (Word::Misconfigured, b"The provider refused temper's sign-in. Check the configuration's registration."),
    (Word::Cancelled, b"Cancelled"),
    (Word::Closing, b"Closing"),
    (Word::Expiring, b"Expiring"),
    (Word::Attention, b"Needs attention"),
    (Word::Spent, b"Usage limit reached"),
    (Word::Cut, b"Some account facts were omitted."),
    (Word::Settled, b"No sign-ins in progress."),
    (Word::Signing, b"Sign-in in progress: source "),
    (Word::LastEnd, b"Last sign-in: source "),
    (Word::Made, b"Signed in; account "),
    (Word::Renewed, b"Signed in to the same account again; account "),
    (Word::Declined, b"You signed in without plan usage."),
    (Word::Later, b"Try later"),
    (Word::Scope, b"Missing required scope "),
    (Word::Directory, b"Directory"),
    (Word::File, b"File"),
    (Word::Unloaded, b"Cannot load "),
    (Word::Unreadable, b"Unreadable"),
    (Word::Refused, b"Refused"),
    (Word::Failed, b"Failed"),
    (Word::Stalled, b"Stalled"),
    (Word::TooLarge, b"Too large"),
    (Word::NotLimited, b"This source is not limited."),
    (Word::UsesRefused, b"The provider does not permit these uses."),
    (Word::KeyLong, b"The key is too long."),
    (Word::NotStored, b"The key could not be stored."),
    (Word::LimitAccounts, b"Account limit reached."),
    (Word::LimitHolder, b"Holder account limit reached."),
    (Word::LimitSources, b"Source limit reached."),
    (Word::LimitSignIns, b"Sign-in limit reached."),
    (Word::Held, b"Account already held for LLM provider "),
    (Word::ProviderRefused, b"Unknown or unavailable LLM provider "),
    (Word::Subject, b"This provider subject cannot be joined."),
    (Word::UnknownNumber, b"Unknown account or source."),
    (Word::Numbers, b"No account or source numbers remain."),
    (Word::Kind, b"Source kind refused: "),
    (Word::NotStarted, b"Sign-in has not started."),
    (Word::Address, b"Open this address in a browser on the engine's machine:"),
    (Word::Lost, b"This act may have been applied. Re-enter the same act, or Cancel to abandon it."),
    (Word::Unsent, b"This act was not sent. Re-enter it, or Cancel to abandon it."),
    (Word::InFlight, b"Account request in progress."),
    (Word::Confirmed, b"Signed out; revocation confirmed."),
    (
        Word::Unconfirmed,
        b"temper removed its tokens, but the provider did not confirm. Disconnect temper in the provider's settings.",
    ),
    (Word::Saved, b"Key kept as source "),
    (Word::Changed, b"Account changed."),
    (Word::KeyEmpty, b"Enter a key."),
    (Word::Selection, b"This account cannot receive that act."),
    (Word::Rank, b"Choose a positive position."),
    (Word::ValueAgain, b"Enter the key again for the same act, or Cancel."),
    (Word::ProviderField, b"Provider number"),
    (Word::AccountField, b"Account number"),
    (Word::SourceField, b"Source number"),
    (Word::KeyField, b"Key (pasted once)"),
    (Word::OwnChoice, b"Use your accounts"),
    (Word::DeploymentChoice, b"Use deployment accounts"),
    (Word::SignInAction, b"Sign in with provider"),
    (Word::AddKey, b"Add key"),
    (Word::ReplaceKey, b"Replace key"),
    (Word::SignOut, b"Sign out"),
    (Word::RemoveKey, b"Remove key"),
    (Word::RemoveAccount, b"Remove account"),
    (Word::Resume, b"Resume"),
    (Word::UpAccount, b"Move account up"),
    (Word::DownAccount, b"Move account down"),
    (Word::UpSource, b"Move source up"),
    (Word::DownSource, b"Move source down"),
    (Word::Confirm, b"Confirm"),
    (Word::Cancel, b"Cancel"),
    (Word::ConfirmSignOut, b"Sign this source out?"),
    (Word::ConfirmKey, b"Remove this key?"),
    (Word::ConfirmAccount, b"Remove this account and all its sources?"),
    (Word::OmittedDisplay, b"Display name omitted."),
    (Word::UsageUntil, b"Usage limited until "),
    (Word::Worker, b"Worker "),
    (Word::AwaitingLogins, b"Waiting for login observations"),
    (Word::NotInContact, b"Not in contact"),
    (Word::SignedInOtherwise, b"Signed in by another method: "),
    (Word::SignedOut, b"Signed out"),
    (Word::Cleanup, b"Cleanup requested"),
    (Word::Missing, b"Program missing"),
    (Word::Deadline, b"Check deadline elapsed"),
    (Word::Exited, b"Program exited"),
    (Word::Unconfigured, b"Program not configured"),
    (Word::ProgramsOmitted, b"Program observations left out: "),
    (Word::AddAccount, b"Add an account"),
    (Word::AddKeyToAccount, b"Add key to account"),
    (Word::ProgramField, b"Program name"),
    (Word::Declare, b"Declare"),
    (Word::ProgramName, b"Claude Code"),
    (Word::ProgramInvalid, b"Use 1 to 32 lowercase letters, digits or hyphens for the program name."),
    (Word::HeldDeployment, b"held by the deployment"),
    (Word::Separator, " · ".as_bytes()),
    (Word::KeyWayOnly, b"Use an API key"),
    (Word::NavNeeds, b"an account needs you"),
    (Word::NavLimited, b"an account is limited"),
    (Word::Moving, b"Moving to another deployment"),
    (Word::NotSignedIn, b"Not signed in"),
    (Word::NotOurs, b"Not this application's record"),
    (Word::MoveCancelled, b"The move was cancelled"),
    (Word::Elsewhere, "This browser isn't on temper's computer ›".as_bytes()),
    (Word::PasteDisclosure, b"Signed in, but the page couldn't be reached?"),
    (Word::PasteHistory, b"Go forward to that page, or find it in your history, and copy its address."),
    (Word::PasteField, b"Address of the page that couldn't be reached"),
    (Word::PasteFinish, b"Finish connecting"),
    (Word::PasteSending, "Finishing…".as_bytes()),
    (Word::PasteFinishing, "Finishing your sign-in…".as_bytes()),
    (Word::PasteLong, b"That's longer than a sign-in's address. Copy the address bar alone."),
    (Word::PasteAddress, b"That isn't this sign-in's address. Copy the whole address of the page that couldn't be reached, from the address bar."),
    (Word::PasteState, b"That address is from an earlier sign-in. Paste the one from this sign-in's tab, or start again."),
    (Word::PasteEnded, b"This sign-in has ended."),
    (Word::PasteAgain, b"That address wasn't sent. Paste it again."),
];
// Both names are borrowed; no configured name is replaced by a guessed brand.
pub(crate) fn provider_words(name: &[u8]) -> (&[u8], &[u8]) {
    match name {
        b"openai" => (b"OpenAI", b"ChatGPT"),
        b"anthropic" => (b"Anthropic", b"Claude"),
        name => (name, name),
    }
}
pub(crate) fn join(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).expect("admitted account words fit");
    }
    let mut out = skein_lib::Writer::new(length);
    for part in parts {
        out.put(part).expect("measured words fit");
    }
    out.finish()
}
const NAMED: &[(Word, &[u8], &[u8])] = &[
    (Word::ProgramNeedsSignIn, b"Claude Code isn't signed in for '", b"' on "),
    (Word::Continue, b"Continue with ", b""),
    (Word::SignInBrand, b"Sign in to ", b""),
    (Word::SubscriptionWay, b"Use your ", b" subscription"),
    (Word::KeyWay, b"Use a ", b" API key"),
    (Word::KeyWayAn, b"Use an ", b" API key"),
    (Word::Subscription, b"", b" subscription"),
    (Word::ApiKeyBrand, b"", b" API key"),
    (Word::HeldBy, b"held by ", b""),
    (Word::PlanSignIn, b"", b" sign-in"),
    (Word::CodeLogin, b"Claude Code login ", b""),
    (Word::ProgramCommand, b"On each host, run: temper program-login ", b""),
    (Word::KeyTitle, "An ".as_bytes(), " API key".as_bytes()),
    (Word::KeyAdded, "Added your ".as_bytes(), " API key".as_bytes()),
    (Word::PlanRuns, "temper’s agents now run on your ".as_bytes(), " plan.".as_bytes()),
    (Word::Command, "temper program-login ".as_bytes(), "".as_bytes()),
    (Word::OpenPlanSignIn, b"Open ", "'s sign-in ›".as_bytes()),
    (Word::PasteInstructions, b"When ", b" is done, its tab shows a page that can't be reached. Copy that page's whole address from the address bar, come back here, and paste it. If a page opens instead, such as another program's sign-in, close that program's sign-in and start again."),
    (Word::PasteTooMany, b"That's too many addresses from other sign-ins. Continue with ", b" to start again."),
];
// PLAN is hole 0 and DISPLAY hole 1; hole 2 repeats PLAN in a renewal.
struct EndWords {
    literals: &'static [&'static [u8]],
    indices: &'static [u16],
}
const ENDS: &[EndWords] = &[
    EndWords { literals: &[b"Signed in to ", b" as ", b""], indices: &[0, 1] },
    EndWords {
        literals: &[
            "That’s ".as_bytes(),
            b" again. To add another account, sign out of ",
            b" in your browser first, then Continue with ",
            b".",
        ],
        indices: &[1, 0, 2],
    },
    EndWords { literals: &[b"Signed in to ", b""], indices: &[0] },
    EndWords { literals: &[b"Declared your ", " subscription · ".as_bytes(), b""], indices: &[0, 1] },
    EndWords { literals: &[b"A ", b" API key"], indices: &[0] },
    EndWords {
        literals: &[
            b"A key from ",
            "’s platform. What temper’s agents use is billed to the key’s organization.".as_bytes(),
        ],
        indices: &[0],
    },
    EndWords {
        literals: &[
            b"Your ",
            b" plan. Sign in with your ",
            " account, and temper’s agents use your plan’s included usage.".as_bytes(),
        ],
        indices: &[0, 1],
    },
    EndWords { literals: &[b"Sign in with your ", b" account."], indices: &[0] },
];
fn id(number: u16) -> PhraseId {
    PhraseId { owner: super::owner(), number }
}
pub(crate) fn say(words: &Words, word: Word, holes: &[Hole]) -> Box<[u8]> {
    let number = if word == Word::ResetUntil {
        228
    } else if word == Word::Number {
        200
    } else if word == Word::Until {
        201
    } else if word == Word::After {
        202
    } else if word == Word::UsageUntil {
        203
    } else if word == Word::AddressLong {
        204
    } else if word == Word::MadeNamed {
        220
    } else if word == Word::RenewedNamed {
        221
    } else if word == Word::KeyTitleA {
        224
    } else if word == Word::KeyDescription {
        225
    } else if word == Word::ChatDescription {
        226
    } else if word == Word::SignInDescription {
        227
    } else if word == Word::ProgramDeclared {
        223
    } else if word == Word::MadePlan {
        222
    } else {
        let mut number = None;
        for (index, (candidate, _, _)) in NAMED.iter().enumerate() {
            if *candidate == word {
                number =
                    Some(300_u16.checked_add(u16::try_from(index).expect("named words fit")).expect("phrase IDs fit"));
            }
        }
        match number {
            Some(number) => number,
            None => literal_number(word),
        }
    };
    words.book.say(words.language, id(number), holes).expect("checked account phrase").text
}
pub(crate) fn extend(shapes: &mut List<PhraseShape>, phrases: &mut List<Phrase>, name_bytes: u32, text_bytes: u32) {
    for (index, (_, literal)) in ROWS.iter().enumerate() {
        let id = id(u16::try_from(index).expect("account words fit"));
        shapes.push(PhraseShape { id, holes: Box::new([]), plural: None }).expect("account shapes fit");
        phrases
            .push(Phrase {
                id,
                forms: Box::new([Form {
                    class: PluralClass::Other,
                    segments: Box::new([Segment::Text(copy_of(literal))]),
                }]),
            })
            .expect("account phrases fit");
    }
    for (index, (_, prefix, suffix)) in NAMED.iter().enumerate() {
        let id =
            id(300_u16.checked_add(u16::try_from(index).expect("named account words fit")).expect("phrase IDs fit"));
        shapes
            .push(PhraseShape {
                id,
                holes: Box::new([HoleShape { kind: HoleKind::Name, bytes: name_bytes }]),
                plural: None,
            })
            .expect("account shape fits");
        phrases
            .push(Phrase {
                id,
                forms: Box::new([Form {
                    class: PluralClass::Other,
                    segments: Box::new([
                        Segment::Text(copy_of(prefix)),
                        Segment::Hole { index: 0, kind: HoleKind::Name },
                        Segment::Text(copy_of(suffix)),
                    ]),
                }]),
            })
            .expect("account phrase fits");
    }
    for (index, end) in ENDS.iter().enumerate() {
        let id = id(220_u16.checked_add(u16::try_from(index).expect("end words fit")).expect("phrase IDs fit"));
        let hole = HoleShape { kind: HoleKind::Name, bytes: text_bytes.max(7) };
        let mut holes = List::with_capacity(u32::try_from(end.indices.len()).expect("end holes fit"));
        for _ in end.indices {
            holes.push(hole).expect("end holes fit");
        }
        shapes.push(PhraseShape { id, holes: holes.into_boxed(), plural: None }).expect("end shapes fit");
        let mut segments = List::with_capacity(7);
        for (at, literal) in end.literals.iter().enumerate() {
            segments.push(Segment::Text(copy_of(literal))).expect("end segments fit");
            if let Some(index) = end.indices.get(at) {
                segments.push(Segment::Hole { index: *index, kind: HoleKind::Name }).expect("end segments fit");
            }
        }
        phrases
            .push(Phrase { id, forms: Box::new([Form { class: PluralClass::Other, segments: segments.into_boxed() }]) })
            .expect("end phrases fit");
    }
    for (number, prefix, kind, suffix) in [
        (228, b" until ".as_slice(), HoleKind::Moment, b". Tasks on it wait.".as_slice()),
        (200, b"".as_slice(), HoleKind::Number, b"".as_slice()),
        (201, b"Limited until ".as_slice(), HoleKind::Moment, b" UTC".as_slice()),
        (202, b"Try again after ".as_slice(), HoleKind::Span, b"".as_slice()),
        (203, b"Usage limit until ".as_slice(), HoleKind::Moment, b" UTC".as_slice()),
        (
            204,
            b"Sign-in address is longer than this client shows; bytes: ".as_slice(),
            HoleKind::Number,
            b"".as_slice(),
        ),
    ] {
        let id = id(number);
        shapes
            .push(PhraseShape { id, holes: Box::new([HoleShape { kind, bytes: 0 }]), plural: None })
            .expect("account shape fits");
        phrases
            .push(Phrase {
                id,
                forms: Box::new([Form {
                    class: PluralClass::Other,
                    segments: Box::new([
                        Segment::Text(copy_of(prefix)),
                        Segment::Hole { index: 0, kind },
                        Segment::Text(copy_of(suffix)),
                    ]),
                }]),
            })
            .expect("account phrase fits");
    }
}
pub(crate) fn capacity(words: &Words) -> Option<u32> {
    let mut most = 0;
    for number in 0..u16::try_from(ROWS.len()).ok()? {
        most = most.max(words.book.capacity(id(number))?);
    }
    for number in [200, 201, 202, 203, 204, 220, 221, 222, 223, 224, 225, 226, 227, 228] {
        most = most.max(words.book.capacity(id(number))?);
    }
    for index in 0..NAMED.len() {
        most = most.max(words.book.capacity(id(300_u16.checked_add(u16::try_from(index).ok()?)?))?);
    }
    Some(most)
}
fn literal_number(word: Word) -> u16 {
    for (index, (candidate, _)) in ROWS.iter().enumerate() {
        if *candidate == word {
            return u16::try_from(index).expect("account words fit");
        }
    }
    unreachable!("closed account words")
}

/// The navigation reads only supplied own-account facts (client/README.md, 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Needs,
    Limited,
}
pub(crate) fn mark(root: &temper_client_domain::Domain) -> Option<Mark> {
    use jig_client::contract::{OwnerAccountHolder, OwnerAccountState};
    let document = root.pages().accounts_document(root.client())?;
    let party = root.pages().me(root.client())?.party.number;
    let mut demo = None;
    if let Some(answer) = root.forge().demo() {
        demo = Some(answer.provider);
    }
    let mut limited = false;
    for account in &document.accounts {
        if account.holder != OwnerAccountHolder::Party(party) || (demo.is_some() && account.provider == demo) {
            continue;
        }
        // The deciding source is the first supplied in order (client/README.md, 5.3).
        // The accounts page also reads source.state directly; cuts add no inferred state.
        let Some(source) = account.sources.get(0) else { continue };
        if source.attention {
            return Some(Mark::Needs);
        }
        limited |= source.spent_until.is_some();
        match source.state {
            OwnerAccountState::SignInAgain | OwnerAccountState::Misconfigured | OwnerAccountState::Unloaded { .. } => {
                return Some(Mark::Needs);
            }
            OwnerAccountState::Later { .. } => limited = true,
            OwnerAccountState::Moving
            | OwnerAccountState::Asking
            | OwnerAccountState::Granted
            | OwnerAccountState::Cancelled
            | OwnerAccountState::Closing => {}
        }
    }
    if limited { Some(Mark::Limited) } else { None }
}
