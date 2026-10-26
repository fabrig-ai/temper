//! Root committed call answers as bounded, UTF-8 Smith host feedback. The
//! root's typed record is authoritative; rendering has no effects or retry
//! policy (domain/agent.md, 4.3; domain/root.md, 5.2).

use alloc::boxed::Box;
use skein_forge::api;
use skein_lib::{Decimal, List, Writer, bytes};
use smith_domain_run::HostAnswer;
use temper_engine_domain::{CallAnswer, Changed};

use crate::Problem;

struct Text {
    parts: List<Box<[u8]>>,
    len: usize,
}

impl Text {
    fn new() -> Self {
        Self { parts: List::with_capacity(1024), len: 0 }
    }

    fn add(&mut self, text: &[u8]) -> Result<(), Problem> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut len = 0_usize;
        for byte in text {
            len = len
                .checked_add(if (*byte >= b' ' && *byte <= b'~') || *byte == b'\n' { 1 } else { 4 })
                .ok_or(Problem::TooLarge)?;
        }
        self.len = self.len.checked_add(len).ok_or(Problem::TooLarge)?;
        if self.len > HostAnswer::CAPACITY {
            return Err(Problem::TooLarge);
        }
        let mut writer = Writer::new(len);
        for byte in text {
            if (*byte >= b' ' && *byte <= b'~') || *byte == b'\n' {
                if writer.put(&[*byte]).is_err() {
                    return Err(Problem::TooLarge);
                }
            } else {
                let high = usize::from(byte.checked_div(16).ok_or(Problem::Range(b"answer"))?);
                let low = usize::from(byte.checked_rem(16).ok_or(Problem::Range(b"answer"))?);
                if writer
                    .put(&[
                        b'\\',
                        b'x',
                        *DIGITS.get(high).ok_or(Problem::Range(b"answer"))?,
                        *DIGITS.get(low).ok_or(Problem::Range(b"answer"))?,
                    ])
                    .is_err()
                {
                    return Err(Problem::TooLarge);
                }
            }
        }
        let Ok(()) = self.parts.push(writer.finish()) else { return Err(Problem::TooLarge) };
        Ok(())
    }

    fn number(&mut self, value: u64) -> Result<(), Problem> {
        self.add(Decimal::of(value).as_bytes())
    }

    fn commit(&mut self, commit: &api::Commit) -> Result<(), Problem> {
        self.add(&temper_engine_domain::commit_text(commit))
    }

    fn finish(self, error: bool) -> Result<HostAnswer, Problem> {
        let mut writer = Writer::new(self.len);
        for part in &self.parts {
            if writer.put(part).is_err() {
                return Err(Problem::TooLarge);
            }
        }
        HostAnswer::new(writer.finish(), error).ok_or(Problem::TooLarge)
    }
}

/// Render a durable root answer for Smith's LLM. Any unrenderable payload
/// becomes a bounded host error, so no decided call is left unanswered.
#[must_use]
pub fn answer(answer: &CallAnswer) -> HostAnswer {
    match rendered(answer) {
        Ok(answer) => answer,
        Err(Problem::TooLarge) => HostAnswer::new(
            bytes::copy_of(b"The recorded answer exceeded the host text limit; ask a narrower read."),
            true,
        )
        .expect("fixed host feedback fits"),
        Err(Problem::Malformed | Problem::Missing(_) | Problem::Type(_) | Problem::Range(_) | Problem::UnknownTool) => {
            HostAnswer::new(bytes::copy_of(b"The recorded answer cannot be rendered as text."), true)
                .expect("fixed host feedback fits")
        }
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive durable answer vocabulary")]
fn rendered(answer: &CallAnswer) -> Result<HostAnswer, Problem> {
    let mut text = Text::new();
    let error = match answer {
        CallAnswer::SettingsRefused(why) => {
            settings_refused(&mut text, why)?;
            true
        }
        CallAnswer::ForgeEffect { entry, outcome, .. } => {
            text.add(b"Forge effect ")?;
            text.number(*entry)?;
            match outcome {
                Some(outcome) => {
                    text.add(b": ")?;
                    forge_outcome(&mut text, outcome)?;
                }
                None => text.add(b" committed and pending")?,
            }
            false
        }
        CallAnswer::ReadRepositoryRefused { named, repositories }
        | CallAnswer::ChangeRefused { named, repositories, .. } => {
            if repositories.is_empty() {
                if let CallAnswer::ReadRepositoryRefused { .. } = answer {
                    text.add(b"this project has no repository to read")?;
                } else {
                    text.add(b"this project has no repository to change")?;
                }
            } else {
                match named {
                    Some(named) => {
                        text.add(b"no repository ")?;
                        text.add(named)?;
                        text.add(b" here; this project has: ")?;
                    }
                    None => text.add(b"name the repository: ")?,
                }
                for (at, repository) in repositories.iter().enumerate() {
                    if at != 0 {
                        text.add(b", ")?;
                    }
                    text.add(repository)?;
                }
            }
            true
        }
        CallAnswer::ForgeEffectRefused(refusal) => {
            text.add(b"Forge effect refused: ")?;
            forge_error(&mut text, *refusal)?;
            true
        }
        value @ (CallAnswer::ForgeEffectDenied { answer, findings }
        | CallAnswer::ToolDenied { answer, findings }
        | CallAnswer::DelegationDenied { answer, findings }) => {
            text.add(b"Authority ")?;
            authority(&mut text, *answer)?;
            text.add(b": ")?;
            let mut change_hint = false;
            for finding in findings {
                finding_text(&mut text, *finding)?;
                text.add(b"; ")?;
                if let CallAnswer::DelegationDenied { .. } = value
                    && let jig_domain_authority::Finding::Executor { .. } | jig_domain_authority::Finding::Grant { .. } =
                        finding
                {
                    change_hint = true;
                }
            }
            if change_hint {
                text.add(b"For a change to a repository, call change with what to change.")?;
            }
            true
        }
        CallAnswer::ForgeRead(read) => match read.as_ref() {
            Ok(read) => {
                forge_answer(&mut text, read)?;
                false
            }
            Err(refusal) => {
                text.add(b"Forge read failed: ")?;
                forge_error(&mut text, *refusal)?;
                true
            }
        },
        CallAnswer::EscalationDecided { task, revision, .. } => {
            text.add(b"Escalation for task ")?;
            text.number(*task)?;
            text.add(b" revision ")?;
            text.number(*revision)?;
            text.add(b" decided")?;
            false
        }
        CallAnswer::EscalationRefused(problem)
        | CallAnswer::ProposalRefused(problem)
        | CallAnswer::EffectRefused(problem)
        | CallAnswer::ControlRefused(problem)
        | CallAnswer::MessageRefused(problem)
        | CallAnswer::SubscriptionRefused(problem)
        | CallAnswer::DelegationRefused(problem) => {
            text.add(b"Refused: ")?;
            task_problem(&mut text, problem)?;
            true
        }
        CallAnswer::Proposed { proposal, holder } => {
            if let jig_domain_tasks::ProposalHolder::Policy {
                project: 0,
                kind: jig_domain_tasks::ProposalKind::Settings,
            } = holder
            {
                text.add(b"proposed as proposal ")?;
                text.number(*proposal)?;
                text.add(b" to the deployment's owners")?;
            } else {
                text.add(b"Proposed as ")?;
                text.number(*proposal)?;
            }
            false
        }
        CallAnswer::ProposalDecided { proposal, .. } => {
            text.add(b"Proposal ")?;
            text.number(*proposal)?;
            text.add(b" decided")?;
            false
        }
        CallAnswer::Again { task, required } => {
            text.add(b"Refused: task ")?;
            text.number(*task)?;
            text.add(if *required {
                b" holds an effect whose outcome is uncertain; say again: true or false"
            } else {
                b" holds no uncertain effect; leave again out"
            })?;
            true
        }
        CallAnswer::PassedUp { task } => {
            text.add(b"Beyond your authority: task ")?;
            text.number(*task)?;
            text.add(b"'s escalation was passed up")?;
            false
        }
        CallAnswer::Amended { task, revision, changed } => {
            text.add(b"Done: task ")?;
            text.number(*task)?;
            text.add(b" is at spec revision ")?;
            text.number(*revision)?;
            let mut separator = b"; changed: ".as_slice();
            match changed.tracked {
                Some(jig_domain_tasks::Tracking::Tracked { priority }) => {
                    text.add(separator)?;
                    text.add(b"priority ")?;
                    text.number(u64::from(priority))?;
                    separator = b", ";
                }
                Some(jig_domain_tasks::Tracking::Untracked) => {
                    text.add(separator)?;
                    text.add(b"untracked")?;
                    separator = b", ";
                }
                None => {}
            }
            for (bit, name) in [
                (Changed::SPEC, b"spec".as_slice()),
                (Changed::WAKE, b"wake"),
                (Changed::DEPENDENCIES, b"dependencies"),
                (Changed::AUTHORITY, b"authority"),
                (Changed::INSTRUCTIONS, b"instructions"),
                (Changed::LLM, b"llm"),
            ] {
                if changed.fields & bit != 0 {
                    text.add(separator)?;
                    text.add(name)?;
                    separator = b", ";
                }
            }
            false
        }
        CallAnswer::Revised { task, revision } => {
            text.add(b"Done: task ")?;
            text.number(*task)?;
            text.add(b" is at spec revision ")?;
            text.number(*revision)?;
            false
        }
        CallAnswer::Moved { task, now } => {
            moved(&mut text, *task, now)?;
            true
        }
        CallAnswer::Controlled => {
            text.add(b"Done")?;
            false
        }
        CallAnswer::ControlDenied { answer } => {
            text.add(b"Control denied: ")?;
            authority(&mut text, *answer)?;
            true
        }
        CallAnswer::Sent { message } => {
            text.add(b"Message ")?;
            text.number(*message)?;
            text.add(b" sent")?;
            false
        }
        CallAnswer::Introduced => {
            text.add(b"Tasks introduced")?;
            false
        }
        CallAnswer::Subscribed { subscription } => {
            text.add(b"Subscription ")?;
            text.number(*subscription)?;
            text.add(b" added")?;
            false
        }
        CallAnswer::Unsubscribed => {
            text.add(b"Subscription removed")?;
            false
        }
        CallAnswer::Delegated(ids) => {
            // The client's chat cards read this line (temper-client-domain pages/delegation.rs `delegated`).
            text.add(b"Delegated tasks: ")?;
            for (at, id) in ids.iter().enumerate() {
                if at > 0 {
                    text.add(b", ")?;
                }
                text.number(id.task)?;
            }
            false
        }
        CallAnswer::NoteWritten { name, revision } => {
            text.add(b"Note ")?;
            text.number(*name)?;
            text.add(b" revision ")?;
            text.number(u64::from(*revision))?;
            text.add(b" written")?;
            false
        }
        CallAnswer::NoteRecalled { entries, more } => {
            for entry in entries {
                text.add(b"Note ")?;
                text.number(entry.name)?;
                text.add(b" revision ")?;
                text.number(u64::from(entry.revision))?;
                text.add(b": ")?;
                text.add(&entry.description)?;
                text.add(b"\n")?;
                text.add(&entry.body)?;
                text.add(b"\n")?;
            }
            if *more {
                text.add(b"More notes are available.")?;
            }
            false
        }
        CallAnswer::NoteRefused(why) => {
            text.add(b"Note refused: ")?;
            text.add(match why {
                jig_domain_notes::Refusal::Busy => b"busy",
                jig_domain_notes::Refusal::Oversized => b"oversized",
                jig_domain_notes::Refusal::Full => b"full",
                jig_domain_notes::Refusal::Exists => b"already exists",
                jig_domain_notes::Refusal::Missing => b"missing",
                jig_domain_notes::Refusal::Moved { .. } => b"revision moved",
                jig_domain_notes::Refusal::RevisionExhausted => b"revision exhausted",
            })?;
            true
        }
        CallAnswer::Unavailable => {
            text.add(b"This engine tool is unavailable.")?;
            true
        }
    };
    text.finish(error)
}

fn forge_outcome(text: &mut Text, outcome: &skein_forge_client::Outcome) -> Result<(), Problem> {
    match outcome {
        skein_forge_client::Outcome::Made { made, found } => {
            if *found {
                text.add(b"found ")?;
            } else {
                text.add(b"made ")?;
            }
            made_text(text, *made)
        }
        skein_forge_client::Outcome::Failed(failure) => forge_error(text, *failure),
        skein_forge_client::Outcome::Raced { made, why } => {
            made_text(text, *made)?;
            text.add(b" raced: ")?;
            forge_error(text, *why)
        }
        skein_forge_client::Outcome::Queued => text.add(b"queued"),
        skein_forge_client::Outcome::Uncertain => text.add(b"uncertain; connector will find it"),
        skein_forge_client::Outcome::Held => text.add(b"uncertain; task held for a person"),
        skein_forge_client::Outcome::Withdrawn => text.add(b"withdrawn before write"),
    }
}

fn made_text(text: &mut Text, made: skein_forge_client::Made) -> Result<(), Problem> {
    match made {
        skein_forge_client::Made::Created(number)
        | skein_forge_client::Made::Commented(number)
        | skein_forge_client::Made::Reviewed(number) => text.number(number),
        skein_forge_client::Made::Merged(commit)
        | skein_forge_client::Made::Updated(commit)
        | skein_forge_client::Made::Branch(commit) => text.commit(&commit),
        skein_forge_client::Made::Set => text.add(b"state set"),
    }
}

fn authority(text: &mut Text, answer: jig_domain_authority::Answer) -> Result<(), Problem> {
    match answer {
        jig_domain_authority::Answer::Allow => text.add(b"allowed"),
        jig_domain_authority::Answer::Wait => text.add(b"waiting"),
        jig_domain_authority::Answer::Propose => text.add(b"needs proposal"),
        jig_domain_authority::Answer::Refuse => text.add(b"refused"),
    }
}

fn finding_text(text: &mut Text, finding: jig_domain_authority::Finding) -> Result<(), Problem> {
    use jig_domain_authority::Finding;
    match finding {
        Finding::Oversized => text.add(b"too large"),
        Finding::UnknownProject => text.add(b"unknown project"),
        Finding::UnknownRole => text.add(b"unknown role"),
        Finding::Authority { .. } => text.add(b"authority ceiling"),
        Finding::Executor { .. } => text.add(b"executor permission"),
        Finding::Tasks { .. } => text.add(b"task allowance"),
        Finding::Spend { .. } => text.add(b"spend allowance"),
        Finding::Price { .. } => text.add(b"effect price"),
        Finding::Arithmetic => text.add(b"arithmetic limit"),
        Finding::RunBudget => text.add(b"run budget"),
        Finding::RunCap => text.add(b"run cap"),
        Finding::Deadline => text.add(b"deadline"),
        Finding::Account => text.add(b"account unavailable"),
        Finding::Writer => text.add(b"writer held"),
        Finding::Tool => text.add(b"tool family"),
        Finding::Grant { .. } => text.add(b"resource grant"),
        Finding::ResourceUnavailable => text.add(b"resource unavailable"),
        Finding::ResourceAccess => text.add(b"resource write unavailable"),
        Finding::Reference => text.add(b"task reference"),
        Finding::Scope { .. } => text.add(b"note scope"),
        Finding::Required { .. } => text.add(b"required fact"),
        Finding::Failed { .. } => text.add(b"failed fact"),
        Finding::Unpermitted => text.add(b"request not permitted"),
        Finding::Undecidable => text.add(b"proposal decision not permitted"),
        Finding::PeriodSpend => text.add(b"period spend ceiling"),
        Finding::Unguarded { .. } => text.add(b"required guard unavailable"),
    }
}

// These refusals name their tasks, matching jig-protocol-tools' render vocabulary.
fn dependency_problem(text: &mut Text, problem: &jig_domain_tasks::Problem) -> Result<(), Problem> {
    use jig_domain_tasks::Refusal;
    let blocked = problem.blocked_by.as_deref().unwrap_or(&[]);
    if let Refusal::Circle { cut } = problem.why {
        text.add(b"these tasks would wait for one another")?;
        for (index, task) in blocked.iter().enumerate() {
            text.add(if index == 0 { b": T" } else { b", T" })?;
            text.number(*task)?;
        }
        if cut && !blocked.is_empty() {
            text.add(b"; some task lists are incomplete")?;
        }
        return Ok(());
    }
    if let Refusal::DependencyEnded { how } = problem.why {
        let Some(task) = blocked.first() else { return text.add(b"a dependency has ended") };
        if how != jig_domain_tasks::Status::Done {
            text.add(b"dependency ")?;
        }
        text.add(b"T")?;
        text.number(*task)?;
        return text.add(match how {
            jig_domain_tasks::Status::Failed => b" failed",
            jig_domain_tasks::Status::Cancelled => b" was cancelled",
            jig_domain_tasks::Status::Done => b" is done",
        });
    }
    if blocked.is_empty() {
        return text.add(b"kept by its dependencies");
    }
    for (index, task) in blocked.iter().enumerate() {
        text.add(if index == 0 { b"kept until T" } else { b", kept until T" })?;
        text.number(*task)?;
        text.add(b" is done")?;
    }
    Ok(())
}

fn task_problem(text: &mut Text, problem: &jig_domain_tasks::Problem) -> Result<(), Problem> {
    use jig_domain_tasks::Refusal;
    if let Refusal::Circle { .. } | Refusal::DependencyEnded { .. } | Refusal::Kept = problem.why {
        return dependency_problem(text, problem);
    }
    if let Some(task) = problem.task {
        text.add(b"task ")?;
        text.number(task)?;
        text.add(b" ")?;
    }
    match problem.why {
        Refusal::Account { account, why } => {
            text.add(b"account ")?;
            text.number(u64::from(account))?;
            text.add(b": ")?;
            text.add(match why {
                jig_domain_tasks::AccountWhy::Unknown => b"not found",
                jig_domain_tasks::AccountWhy::Provider => b"belongs to another provider",
                jig_domain_tasks::AccountWhy::Uses => b"does not serve the payer",
                jig_domain_tasks::AccountWhy::Limited => b"is limited",
            })
        }

        Refusal::Placed { .. } => text.add(b"held by a placed hold"),
        Refusal::HoldOwner { .. } => text.add(b"only the placed hold's owner may release it"),
        Refusal::HoldRevision { .. } => text.add(b"placed hold revision changed"),
        Refusal::NotReady => text.add(b"not ready"),
        Refusal::Unknown => text.add(b"unknown"),
        Refusal::Busy => text.add(b"busy"),
        Refusal::Duplicate => text.add(b"duplicate"),
        Refusal::Empty => text.add(b"empty batch"),
        Refusal::Batch => text.add(b"batch limit"),
        Refusal::Grace => text.add(b"finish grace"),
        Refusal::Goals => text.add(b"goal limit"),
        Refusal::Live => text.add(b"live task limit"),
        Refusal::Project => text.add(b"project"),
        Refusal::Tree => text.add(b"task tree limit"),
        Refusal::Depth => text.add(b"depth limit"),
        Refusal::Delegates => text.add(b"delegate limit"),
        Refusal::Reference => text.add(b"no reference"),
        Refusal::Subscription => text.add(b"subscription"),
        Refusal::Dependencies => text.add(b"dependencies"),
        Refusal::Circle { .. } => text.add(b"circle"),
        Refusal::DependencyEnded { .. } => text.add(b"dependency"),
        Refusal::Kept => text.add(b"kept"),
        Refusal::Executor => text.add(b"executor"),
        Refusal::Spec => text.add(b"specification"),
        Refusal::Contract => text.add(b"result contract"),
        Refusal::AuthorityShape => text.add(b"authority shape"),
        Refusal::Inputs => text.add(b"historical inputs"),
        Refusal::State => text.add(b"task state"),
        Refusal::Attempt => text.add(b"attempt"),
        Refusal::LiveDelegates => text.add(b"live delegates"),
        Refusal::Restore => text.add(b"restoration"),
        Refusal::Read => text.add(b"read fence"),
        Refusal::Turn => text.add(b"turn order"),
        Refusal::Funding => text.add(b"funding"),
        Refusal::HoldKind => text.add(b"resource hold kind"),
        Refusal::HoldTaken => text.add(b"resource held by"),
        Refusal::ResourceUnavailable => text.add(b"resource unavailable"),
        Refusal::Holds => text.add(b"resource hold limit"),
        Refusal::Unproposable => text.add(b"no one may accept this proposal"),
        Refusal::PersonRevision { .. } => text.add(b"older person task revision"),
        // As jig-protocol-tools' render names each fault.
        Refusal::Llm { why, .. } => text.add(match why {
            jig_domain_tasks::LlmFault::Unconfigured => b"model not configured",
            jig_domain_tasks::LlmFault::Effort => b"effort not declared",
            jig_domain_tasks::LlmFault::Charter => b"model outside the charter",
            jig_domain_tasks::LlmFault::Authority => b"model outside authority",
            jig_domain_tasks::LlmFault::Procedure => b"no model runs this task",
            jig_domain_tasks::LlmFault::Repeated => b"model repeated",
        }),
    }?;
    if let Some(blocked_by) = &problem.blocked_by {
        for task in blocked_by {
            text.add(b" ")?;
            text.number(*task)?;
        }
    }
    Ok(())
}

fn forge_error(text: &mut Text, error: api::Error) -> Result<(), Problem> {
    match error {
        api::Error::Unavailable => text.add(b"unavailable"),
        api::Error::Timeout => text.add(b"timed out"),
        api::Error::RateLimited { .. } => text.add(b"rate limited"),
        api::Error::Forbidden => text.add(b"forbidden"),
        api::Error::Missing => text.add(b"missing"),
        api::Error::MissingJob => text.add(b"job missing"),
        api::Error::TooLarge => text.add(b"too large"),
        api::Error::Empty => text.add(b"empty"),
        api::Error::Full => text.add(b"full"),
        api::Error::Exists => text.add(b"already exists"),
        api::Error::NothingToMerge => text.add(b"nothing to merge"),
        api::Error::Closed => text.add(b"closed"),
        api::Error::Stale => text.add(b"stale head"),
        api::Error::Conflict => text.add(b"conflict"),
        api::Error::Protected => text.add(b"protected"),
        api::Error::Refused => text.add(b"refused"),
        api::Error::InvalidAnswer => text.add(b"invalid provider answer"),
        api::Error::Busy => text.add(b"busy"),
        api::Error::NotInstalled => text.add(b"not installed"),
        api::Error::NotConnected => text.add(b"not connected"),
    }
}

#[expect(clippy::too_many_lines, reason = "one exhaustive forge answer vocabulary")]
fn forge_answer(text: &mut Text, answer: &api::Answer) -> Result<(), Problem> {
    match answer {
        api::Answer::Located { repository } => {
            text.add(b"Repository ")?;
            text.number(u64::from(*repository))?;
        }
        api::Answer::Items { items, more, .. } => {
            text.add(b"Items:\n")?;
            for item in items {
                summary(text, item)?;
            }
            if *more {
                text.add(b"More pages available.\n")?;
            }
        }
        api::Answer::Item { item, comments, more } => {
            summary(text, item)?;
            for comment in comments {
                text.add(b"Comment ")?;
                text.number(comment.id)?;
                text.add(b": ")?;
                text.add(&comment.body)?;
                text.add(b"\n")?;
            }
            if *more {
                text.add(b"More comments available.\n")?;
            }
        }
        api::Answer::Pull(pull) => {
            text.add(b"Pull ")?;
            text.number(pull.number)?;
            text.add(b" head ")?;
            text.add(&pull.head)?;
            text.add(b" commit ")?;
            text.commit(&pull.commit)?;
            text.add(b" base ")?;
            text.add(&pull.base)?;
            text.add(b" state ")?;
            match pull.state {
                api::State::Open => text.add(b"open")?,
                api::State::Closed => text.add(b"closed")?,
            }
            if let Some(merge) = &pull.merged {
                text.add(b" merged commit ")?;
                text.commit(merge)?;
            }
            text.add(b" CI ")?;
            ci(text, pull.ci)?;
        }
        api::Answer::Reviews { reviews, more } => {
            for review in reviews {
                text.add(b"Review ")?;
                text.number(review.id)?;
                text.add(b" by ")?;
                text.number(review.author)?;
                text.add(b": ")?;
                text.add(&review.body)?;
                text.add(b"\n")?;
            }
            if *more {
                text.add(b"More reviews available.\n")?;
            }
        }
        api::Answer::Statuses { ci: status, statuses, more } => {
            text.add(b"CI ")?;
            ci(text, *status)?;
            text.add(b"\n")?;
            for status in statuses {
                status_text(text, status)?;
            }
            if *more {
                text.add(b"More statuses available.\n")?;
            }
        }
        api::Answer::Remarks { remarks, more } => {
            for remark in remarks {
                text.add(b"Remark on ")?;
                text.add(&remark.path)?;
                text.add(b": ")?;
                text.add(&remark.body)?;
                text.add(b"\n")?;
            }
            if *more {
                text.add(b"More remarks available.\n")?;
            }
        }
        api::Answer::Commit(commit) | api::Answer::Merged(commit) => text.commit(commit)?,
        api::Answer::Branches(branches) => {
            for branch in branches {
                text.add(branch)?;
                text.add(b"\n")?;
            }
        }
        api::Answer::PullFiles { files, more, .. } => {
            for file in files {
                file_text(text, file)?;
            }
            if *more {
                text.add(b"More files available.\n")?;
            }
        }
        api::Answer::Compare { files, commits, contains_before, .. } => {
            text.add(b"Contains prior head: ")?;
            text.add(if *contains_before { b"yes\n" } else { b"no\n" })?;
            for commit in commits {
                text.commit(commit)?;
                text.add(b"\n")?;
            }
            for file in files {
                file_text(text, file)?;
            }
        }
        api::Answer::Checks(statuses) => {
            for status in statuses {
                status_text(text, status)?;
            }
        }
        api::Answer::Job { attempt, step, log, truncated } => {
            text.add(b"Job ")?;
            text.number(attempt.job)?;
            text.add(b" run ")?;
            text.number(attempt.run)?;
            if let Some(step) = step {
                text.add(b", failed step ")?;
                text.number(u64::from(step.number))?;
                if !step.name.is_empty() {
                    text.add(b" ")?;
                    text.add(&step.name)?;
                }
            } else {
                text.add(b", no step failed")?;
            }
            text.add(b":\n")?;
            if *truncated {
                text.add(b"[earlier log bytes cut]\n")?;
            }
            text.add(log)?;
        }
        api::Answer::File { head, path, bytes, truncated } => {
            text.add(path)?;
            text.add(b" at ")?;
            text.commit(head)?;
            text.add(b":\n")?;
            text.add(bytes)?;
            if *truncated {
                text.add(b"\n[file truncated]")?;
            }
        }
        api::Answer::Protection(protection) => match protection {
            Some(rule) => {
                text.add(b"Protected branch ")?;
                text.add(&rule.branch)?;
                text.add(b" approvals ")?;
                text.number(u64::from(rule.approvals))?;
            }
            None => text.add(b"No protection rule")?,
        },
        api::Answer::Settings(settings) => {
            text.add(b"Default branch ")?;
            text.add(&settings.default_branch)?;
            text.add(b"; merge ")?;
            text.add(if settings.merge { b"yes" } else { b"no" })?;
        }
        api::Answer::Collaborators { collaborators, more } => {
            for collaborator in collaborators {
                text.add(b"User ")?;
                text.number(collaborator.user)?;
                text.add(b"\n")?;
            }
            if *more {
                text.add(b"More collaborators available.\n")?;
            }
        }
        api::Answer::Permission(permission) => match permission {
            api::Permission::None => text.add(b"No permission")?,
            api::Permission::Read => text.add(b"Read")?,
            api::Permission::Write => text.add(b"Write")?,
            api::Permission::Admin => text.add(b"Admin")?,
        },
        api::Answer::Created(number) | api::Answer::Commented(number) | api::Answer::Reviewed(number) => {
            text.number(*number)?;
        }
        api::Answer::Branch(branch) => match branch {
            api::BranchCreation::Created => text.add(b"Branch created")?,
            api::BranchCreation::Exists => text.add(b"Branch already exists")?,
        },
        api::Answer::Queued => unreachable!("temper-12 gives merge queue answers meaning"),
        api::Answer::Done => text.add(b"Done")?,
    }
    Ok(())
}

fn summary(text: &mut Text, item: &api::Summary) -> Result<(), Problem> {
    text.add(b"#")?;
    text.number(item.number)?;
    text.add(b" ")?;
    text.add(&item.title)?;
    text.add(b"\n")?;
    text.add(&item.body)?;
    text.add(b"\n")
}

fn ci(text: &mut Text, state: api::Ci) -> Result<(), Problem> {
    match state {
        api::Ci::None => text.add(b"none"),
        api::Ci::Pending => text.add(b"pending"),
        api::Ci::Passed => text.add(b"passed"),
        api::Ci::Failed => text.add(b"failed"),
    }
}

fn status_text(text: &mut Text, status: &api::Status) -> Result<(), Problem> {
    text.add(&status.context)?;
    text.add(b": ")?;
    match status.check {
        api::Check::Pending => text.add(b"pending")?,
        api::Check::Passed => text.add(b"passed")?,
        api::Check::Failed => text.add(b"failed")?,
    }
    text.add(b" ")?;
    text.add(&status.description)?;
    text.add(b" ")?;
    text.add(&status.url)?;
    text.add(b"\n")
}

fn file_text(text: &mut Text, file: &api::File) -> Result<(), Problem> {
    text.add(&file.path)?;
    text.add(b"\n")?;
    match &file.before {
        Some(before) => text.add(before)?,
        None => text.add(b"[absent]")?,
    }
    text.add(b"\n->\n")?;
    match &file.after {
        Some(after) => text.add(after)?,
        None => text.add(b"[absent]")?,
    }
    text.add(b"\n")
}

fn moved(text: &mut Text, task: u64, now: &jig_domain_people::Moved) -> Result<(), Problem> {
    use jig_domain_people::Moved;
    text.add(b"Moved: task ")?;
    text.number(task)?;
    let (kind, revision) = match now {
        Moved::Spec { revision } => (b" is at spec revision ".as_slice(), *revision),
        Moved::Person { revision } => (b" is at person revision ".as_slice(), *revision),
        Moved::Note { revision } => (b" is at note revision ".as_slice(), u64::from(*revision)),
        Moved::Hold { revision, .. } => (b" is held at hold revision ".as_slice(), *revision),
    };
    text.add(kind)?;
    text.number(revision)?;
    text.add(b" now")?;
    if let Moved::Hold { reason, .. } = now {
        text.add(b", ")?;
        text.add(hold_reason(reason))?;
    }
    Ok(())
}

fn hold_reason(reason: &jig_domain_people::HoldReason) -> &'static [u8] {
    use jig_domain_people::{FailureClass, HoldReason};
    match reason {
        HoldReason::Failures { class } => match class {
            FailureClass::Transient => b"for transient failures",
            FailureClass::Permanent => b"for permanent failures",
            FailureClass::Run => b"for run failures",
            FailureClass::Agent => b"for agent failures",
            FailureClass::Lost => b"for lost runs",
            FailureClass::Invalid => b"for invalid results",
        },
        HoldReason::Exceeded(_) => b"for its configured model allowance",
        HoldReason::Circle => b"for a circle of waits",
        HoldReason::Stalled => b"because it stalled",
        HoldReason::Dependency { .. } => b"for its dependency",
        HoldReason::Stopped => b"because it was stopped",
        HoldReason::Placed { .. } => b"because a placed hold holds it",
        HoldReason::Drift => b"for drift",
        HoldReason::Effects => b"for its effects",
        HoldReason::EffectFailed => b"for a failed effect",
        HoldReason::Uncertain { .. } => b"for an uncertain effect",
        HoldReason::Budget => b"for its budget",
        HoldReason::Deadline => b"for its deadline",
        HoldReason::HoldsWaited => b"for waiting on holds",
        HoldReason::ResourceUnavailable => b"for an unavailable resource",
        HoldReason::ResourceAccess => b"for resource access",
        HoldReason::PoolLost { .. } => b"for a lost pool",
        HoldReason::Procedure => b"for its procedure",
    }
}

fn settings_refused(text: &mut Text, why: &jig_domain_people::Refusal) -> Result<(), Problem> {
    text.add(&crate::settings::refusal(why))
}
