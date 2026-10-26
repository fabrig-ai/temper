//! Forge records and the bounded attempts owned by their routing (client/README.md, 4.6).
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm
)]
use crate::forge;
use alloc::boxed::Box;
use jig_client_protocol_link as link;
use skein_lib::{List, Reader, Writer};
use temper_client_protocol_wire::{self as wire, v1 as w};
#[must_use]
pub fn request(a: &forge::Adoption, limits: &w::Limits) -> Option<jig_client::Family> {
    let mut checks = List::with_capacity(limits.adopt_checks);
    for check in &a.checks {
        if checks.push(*check).is_err() {
            return None;
        }
    }
    let record = w::Adopt::new(
        limits,
        w::AdoptParts {
            project: a.project,
            home: a.home,
            forge: a.forge,
            owner: a.owner.clone(),
            name: a.name.clone(),
            prefix: a.prefix.clone(),
            role: role_to(a.role),
            landing: a.landing.clone(),
            ci: a.ci,
            checks,
        },
    )
    .ok()?;
    Some(jig_client::Family {
        family: wire::number(wire::Owner::Forge),
        version: wire::CLIENT_VERSION,
        bytes: wire::requesting(
            &w::Requesting::new(limits, w::RequestingParts { request: w::Request::Adopt(record) }).ok()?,
        ),
    })
}
#[must_use]
pub fn answer(record: &jig_client::Family, limits: &w::Limits, link: &link::Limits) -> Option<forge::Answer> {
    if record.family != wire::number(wire::Owner::Forge) {
        return None;
    }
    if record.version != wire::CLIENT_VERSION {
        return None;
    }
    let mut reader = Reader::new(&record.bytes);
    let a = w::Answer::decode(limits, &mut reader).ok()?.into_parts();
    if !reader.is_empty() {
        return None;
    }
    Some(match a.outcome {
        w::Outcome::Adopted(a) => {
            let a = a.into_parts();
            forge::Answer::Adopted(forge::Adopted {
                forge: a.forge,
                repository: a.repository,
                owner: a.owner,
                name: a.name,
                role: role_from(&a.role),
                home: a.home,
                kinds: forge::Kinds {
                    read: a.kinds.read(),
                    push: a.kinds.push(),
                    open: a.kinds.open(),
                    land: a.kinds.land(),
                    review: a.kinds.review(),
                    status: a.kinds.status(),
                    comment: a.kinds.comment(),
                    issue: a.kinds.issue(),
                    branch: a.kinds.branch(),
                },
                protection: match a.protection {
                    w::Protection::Unknown => forge::Protection::Unknown,
                    w::Protection::Absent => forge::Protection::Absent,
                    w::Protection::Rule(rule) => {
                        let rule = rule.into_parts();
                        forge::Protection::Rule(forge::Rule {
                            branch: rule.branch,
                            contexts: rule.contexts.into_boxed(),
                            approvals: rule.approvals,
                            dismiss_stale: rule.dismiss_stale,
                        })
                    }
                },
            })
        }
        w::Outcome::CoreRefused(a) => forge::Answer::CoreRefused(core_refusal(a.refusal(), link)?),
        w::Outcome::ForgeRefused(a) => forge::Answer::ForgeRefused(match a.into_parts().reason {
            w::ForgeReason::NoSuchForge => forge::Reason::NoSuchForge,
            w::ForgeReason::NotFound => forge::Reason::NotFound,
            w::ForgeReason::NoAccess => forge::Reason::NoAccess,
            w::ForgeReason::Refused => forge::Reason::Refused,
            w::ForgeReason::InFlight => forge::Reason::InFlight,
            w::ForgeReason::TooLarge => forge::Reason::TooLarge,
            w::ForgeReason::Unreachable => forge::Reason::Unreachable,
            w::ForgeReason::InvalidAnswer => forge::Reason::InvalidAnswer,
        }),
        w::Outcome::Unknown => forge::Answer::Unknown,
        w::Outcome::Page(_)
        | w::Outcome::Accepted(_)
        | w::Outcome::Returning
        | w::Outcome::Connecting
        | w::Outcome::Link(_)
        | w::Outcome::ConnectRefused(_)
        | w::Outcome::SettingUp
        | w::Outcome::SetUpRefused(_)
        | w::Outcome::Running
        | w::Outcome::StartRefused(_)
        | w::Outcome::StartFailed(_) => return None,
    })
}
#[must_use]
pub fn core_refusal(bytes: &[u8], limits: &link::Limits) -> Option<jig_client::contract::Refusal> {
    use jig_client_protocol_wire as j;
    let mut reader = Reader::new(bytes);
    let refusal = j::Refusal::decode(&limits.wire, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    let final_answer = j::Final::Refused(j::Refused::new(&limits.wire, j::RefusedParts { refusal, at: None }).ok()?);
    let mut writer = Writer::new(usize::try_from(final_answer.measure()).expect("measured record fits usize"));
    final_answer.encode(&mut writer).ok()?;
    let owned = j::Owned::new(
        &limits.wire,
        j::OwnedParts { owner: j::Owner::Jig, version: j::VERSION, record: writer.finish() },
    )
    .ok()?;
    match link::translate::answers::answer(j::KeyedAnswer::FinalAnswer(owned), limits).ok()? {
        jig_client::contract::Answer::Refused { refusal, .. } => Some(refusal),
        jig_client::contract::Answer::Done(_) => None,
    }
}
const fn role_to(role: forge::Role) -> w::Role {
    match role {
        forge::Role::Owned => w::Role::Owned,
        forge::Role::Adopted => w::Role::Adopted,
        forge::Role::Fork => w::Role::Fork,
        forge::Role::Context => w::Role::Context,
    }
}
const fn role_from(role: &w::Role) -> forge::Role {
    match role {
        w::Role::Owned => forge::Role::Owned,
        w::Role::Adopted => forge::Role::Adopted,
        w::Role::Fork => forge::Role::Fork,
        w::Role::Context => forge::Role::Context,
    }
}

/// Decode the demo's complete record, including its embedded family version.
#[must_use]
pub fn demo(record: &[u8], limits: &w::Limits) -> Option<forge::Demo> {
    let answer = wire::decode_demo_answer(record, limits)?.into_parts();
    Some(forge::Demo {
        state: match answer.state {
            w::DemoState::NotStarted => forge::demo::DemoState::NotStarted,
            w::DemoState::Running => forge::demo::DemoState::Running,
            w::DemoState::Ended => forge::demo::DemoState::Ended,
        },
        project: answer.project,
        forge: answer.forge,
        provider: answer.provider,
        owner: answer.owner,
        name: answer.name,
        cues: answer.cues.into_boxed(),
    })
}

#[must_use]
pub fn set_up(
    forge: u16,
    org: Box<[u8]>,
    repository: Box<[u8]>,
    project: forge::forges::SetUpProject,
    limits: &w::Limits,
) -> Option<jig_client::Family> {
    let project = match project {
        forge::forges::SetUpProject::Existing(project) => {
            w::SetUpProject::Existing(w::ExistingProject::new(limits, w::ExistingProjectParts { project }).ok()?)
        }
        forge::forges::SetUpProject::New(name) => {
            w::SetUpProject::New(w::NewProject::new(limits, w::NewProjectParts { name }).ok()?)
        }
    };
    let request = w::SetUp::new(limits, w::SetUpParts { forge, org, repository, project }).ok()?;
    let record = w::Requesting::new(limits, w::RequestingParts { request: w::Request::SetUp(request) }).ok()?;
    Some(jig_client::Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&record) })
}
#[must_use]
pub fn set_up_answer(record: &jig_client::Family, limits: &w::Limits) -> Option<forge::forges::SetUpOutcome> {
    use forge::forges::{SetUpOutcome as O, SetUpRefusal as R};
    if record.family != wire::FORGE || record.version != wire::CLIENT_VERSION {
        return None;
    }
    let mut reader = Reader::new(&record.bytes);
    let answer = w::Answer::decode(limits, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    Some(match answer.into_parts().outcome {
        w::Outcome::SettingUp => O::SettingUp,
        w::Outcome::SetUpRefused(why) => O::Refused(match why.into_parts().why {
            w::SetUpRefusal::NotOffered => R::NotOffered,
            w::SetUpRefusal::NotUnderTheBox => R::NotUnderTheBox,
            w::SetUpRefusal::Busy => R::Busy,
            w::SetUpRefusal::NewProject => R::NewProject,
            w::SetUpRefusal::Invalid => R::Invalid,
        }),
        w::Outcome::Unknown => O::Unknown,
        w::Outcome::Page(_)
        | w::Outcome::Accepted(_)
        | w::Outcome::Returning
        | w::Outcome::Connecting
        | w::Outcome::Link(_)
        | w::Outcome::ConnectRefused(_)
        | w::Outcome::Adopted(_)
        | w::Outcome::CoreRefused(_)
        | w::Outcome::ForgeRefused(_)
        | w::Outcome::Running
        | w::Outcome::StartRefused(_)
        | w::Outcome::StartFailed(_) => return None,
    })
}
#[must_use]
#[expect(clippy::manual_map, reason = "the translation uses explicit matches (programming-model.md, 10)")]
pub fn forges(bytes: &[u8], limits: &w::Limits) -> Option<Box<[forge::forges::Row]>> {
    use forge::forges::{Account, Installation, Kind, Mode, Offer, Row};
    let record = wire::decode_forges(bytes, limits)?.into_parts();
    let mut rows = List::with_capacity(record.forges.len());
    for row in record.forges.into_boxed() {
        let row = row.into_parts();
        let connection = connection_from(row.connection);
        let mut installations = List::with_capacity(row.installations.len());
        for installation in row.installations.into_boxed() {
            let installation = installation.into_parts();
            installations
                .push(Installation {
                    id: installation.id,
                    account: match installation.account {
                        Some(account) => {
                            let account = account.into_parts();
                            Some(Account { id: account.id, login: account.login })
                        }
                        None => None,
                    },
                    suspended: installation.suspended,
                    stray: installation.stray,
                })
                .ok()?;
        }
        let mut repositories = List::with_capacity(row.repositories.len());
        for adopted in row.repositories.into_boxed() {
            let adopted = adopted.into_parts();
            repositories
                .push(forge::forges::Adoption { owner: adopted.owner, name: adopted.name, project: adopted.project })
                .ok()?;
        }
        rows.push(Row {
            read_only: row.read_only,
            number: row.number,
            kind: match row.kind {
                w::Kind::Forgejo => Kind::Forgejo,
                w::Kind::Github => Kind::GitHub,
            },
            mode: match row.mode {
                w::Mode::Connected => Mode::Connected,
                w::Mode::Managed => Mode::Managed,
                w::Mode::Owned => Mode::Owned,
                w::Mode::Fake => Mode::Fake,
            },
            connection,
            offer: match row.offer {
                w::Offer::NotOffered => Offer::NotOffered,
                w::Offer::NotUnderTheBox => Offer::NotUnderTheBox,
                w::Offer::Offered => Offer::Offered,
            },
            first: match row.first {
                Some(first) => Some(first_from(first)),
                None => None,
            },
            installations: installations.into_boxed(),
            ended: match row.ended {
                Some(ended) => Some(ended.into_parts().why),
                None => None,
            },
            app: match row.app {
                Some(w::AppWay::Manifest) => Some(super::forges::AppWay::Manifest),
                Some(w::AppWay::Pasted) => Some(super::forges::AppWay::Pasted),
                None => None,
            },
            web: row.web,
            repositories: repositories.into_boxed(),
            owned: match row.owned {
                Some(owned) => Some(owned_from(owned)),
                None => None,
            },
            admin: row.admin,
            ended_stop: match row.ended_stop {
                Some(stop) => Some(admin_stop(stop)),
                None => None,
            },
        })
        .ok()?;
    }
    Some(rows.into_boxed())
}

fn connection_from(connection: w::Connection) -> forge::forges::Connection {
    use forge::forges::{Connection as C, Step};
    match connection {
        w::Connection::NotConnected => C::NotConnected,
        w::Connection::NotSetUp => C::NotSetUp,
        w::Connection::SettingUp(value) => C::SettingUp {
            step: match value.into_parts().step {
                Some(w::SetUpStep::Fetch) => Some(Step::Fetch),
                Some(w::SetUpStep::Prepare) => Some(Step::Prepare),
                Some(w::SetUpStep::Run) => Some(Step::Run),
                None => None,
            },
        },
        w::Connection::SetUpFailed(value) => {
            let value = value.into_parts();
            C::SetUpFailed {
                why: value.why,
                at: match value.at {
                    Some(w::FailedStep::Fetch) => Some(forge::forges::FailedStep::Fetch),
                    Some(w::FailedStep::Prepare) => Some(forge::forges::FailedStep::Prepare),
                    Some(w::FailedStep::Run) => Some(forge::forges::FailedStep::Run),
                    Some(w::FailedStep::Connect) => Some(forge::forges::FailedStep::Connect),
                    None => None,
                },
            }
        }
        w::Connection::Connecting => C::Connecting,
        w::Connection::Connected(value) => {
            let value = value.into_parts();
            C::Connected { id: value.id, login: value.login }
        }
        w::Connection::Stopped(value) => C::Stopped { reason: value.into_parts().reason },
    }
}

fn owned_from(owned: w::OwnedForge) -> forge::forges::Owned {
    let owned = owned.into_parts();
    forge::forges::Owned { forgejo: owned.forgejo, runner: owned.runner, password: owned.password }
}

fn first_from(first: w::FirstRepository) -> forge::forges::First {
    use forge::forges::{CreateStep, First, FirstState, SetUpProject};
    let first = first.into_parts();
    First {
        org: first.org,
        repository: first.repository,
        project: match first.project {
            w::SetUpProject::Existing(project) => SetUpProject::Existing(project.project()),
            w::SetUpProject::New(project) => SetUpProject::New(project.into_parts().name),
        },
        state: match first.state {
            w::FirstState::Making => FirstState::Making,
            w::FirstState::Made(value) => FirstState::Made { id: value.id() },
            w::FirstState::Failed(value) => FirstState::Failed {
                step: match value.step() {
                    w::CreateStep::Admin => CreateStep::Admin,
                    w::CreateStep::Org => CreateStep::Org,
                    w::CreateStep::Repository => CreateStep::Repository,
                    w::CreateStep::File => CreateStep::File,
                },
            },
        },
        label: first.label,
    }
}

/// One fresh picker page; it carries no credentials (client/README.md, 4.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub rows: Box<[Reached]>,
    pub more: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reached {
    pub id: u64,
    pub owner: forge::forges::Account,
    pub name: Box<[u8]>,
    pub private: bool,
}
#[must_use]
pub fn page(value: w::Page) -> Page {
    let value = value.into_parts();
    let mut rows = List::with_capacity(value.rows.len());
    for row in value.rows.into_boxed() {
        let row = row.into_parts();
        let owner = row.owner.into_parts();
        rows.push(Reached {
            id: row.id,
            owner: forge::forges::Account { id: owner.id, login: owner.login },
            name: row.name,
            private: row.private,
        })
        .expect("decoded page bound");
    }
    Page { rows: rows.into_boxed(), more: value.more }
}

fn admin_stop(stop: w::AdminStop) -> forge::forges::AdminStop {
    use forge::forges::{AdminStop, SetupStage};
    match stop {
        w::AdminStop::NotAdmin => AdminStop::NotAdmin,
        w::AdminStop::Unmarked => AdminStop::Unmarked,
        w::AdminStop::Other => AdminStop::Other,
        w::AdminStop::Refused(refused) => AdminStop::Refused {
            status: refused.status(),
            stage: match refused.stage() {
                w::SetupStage::Admin => SetupStage::Admin,
                w::SetupStage::Bot => SetupStage::Bot,
                w::SetupStage::Token => SetupStage::Token,
                w::SetupStage::Org => SetupStage::Org,
                w::SetupStage::Repository => SetupStage::Repository,
                w::SetupStage::File => SetupStage::File,
                w::SetupStage::App => SetupStage::App,
                w::SetupStage::Installation => SetupStage::Installation,
                w::SetupStage::State => SetupStage::State,
            },
        },
    }
}
