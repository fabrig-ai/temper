//! Drain before closing children, close the store last (protocol/README.md, 6).
use crate::{End, Stop, engine::Engine};
use jig_protocol_config::{Path, Problems, What};
use temper_engine_domain as root;

pub(crate) struct Drain {
    pub(crate) closing: bool,
    pub(crate) requested: bool,
    stopped: Option<Stop>,
    pub(crate) unsettled: skein_lib::List<root::Unsettled>,
}
/// A second call aborts, ending at once with the reason a failure's drain
/// carries (protocol/README.md, section 4).
pub(crate) fn shutdown(engine: &mut Engine) {
    if let Some(drain) = &mut engine.drain {
        engine.end = Some(match drain.stopped.take() {
            Some(why) => End::Stopped(why),
            None => End::Aborted,
        });
        return;
    }
    engine.drain = Some(Drain {
        closing: false,
        requested: false,
        stopped: None,
        unsettled: skein_lib::List::with_capacity(engine.env.limits.forge.entries),
    });
    engine.workers.down(engine.env.now, jig_protocol_engine_workers::Event::Stop, &mut engine.events);
    engine.clients.down(jig_protocol_engine_clients::Event::Stop);
    advance(engine);
}
pub(crate) fn drained(engine: &mut Engine) {
    let Some(drain) = &mut engine.drain else {
        return;
    };
    if drain.closing {
        return;
    }
    drain.closing = true;
    engine.setup.close();
    engine.forge.down(skein_forge_connection::Request::Close);
    engine.clients.down(jig_protocol_engine_clients::Event::Close);
    engine.workers.close(engine.env.now, &mut engine.events);
    if engine.accounts.is_some() {
        engine.step(root::Event::Account(jig_domain_accounts::Event::Close));
    }
}
pub(crate) fn advance(engine: &mut Engine) {
    if engine.end.is_some() {
        return;
    }
    let Some(drain) = &mut engine.drain else {
        return;
    };
    if !drain.requested {
        // Finish public restore and the mark before closing the core's children.
        // In particular, Switch must still see the accounts in Wanted.
        if engine.domain.ready() && engine.events.room() > 0 {
            drain.requested = true;
            engine.events.push(root::Event::Stop);
        }
        return;
    }
    if !drain.closing {
        return;
    }
    engine.setup.close();
    let closed = match &engine.accounts {
        Some(accounts) => accounts.is_empty(),
        None => true,
    };
    if closed
        && engine.setup.closed
        && engine.setup.io.is_empty()
        && engine.forge.closed
        && engine.forge.io.is_empty()
        && engine.workers.closed()
        && engine.clients.closed
        && engine.clients.io.is_empty()
        && engine.store.idle()
        && engine.events.is_empty()
        && engine.requests.is_empty()
    {
        engine.store.close();
        if engine.store.closed() {
            engine.end = Some(match drain.stopped.take() {
                Some(why) => End::Stopped(why),
                None => End::Drained,
            });
        }
    }
}
pub(crate) fn frame_too_large(engine: &mut Engine, number: u64, task: u64, bytes: u32, limit: u32) {
    engine.step(root::Event::Store(root::StoreInput::Uncommitted { number }));
    engine.end = Some(End::Stopped(Stop::FrameTooLarge { number, task, bytes, limit }));
}
pub(crate) fn failed(engine: &mut Engine, number: u64) {
    engine.step(root::Event::Store(root::StoreInput::Uncommitted { number }));
    engine.end = Some(End::Stopped(Stop::CommitFailed { number }));
}
pub(crate) fn open(engine: &mut Engine, why: &jig_protocol_store::OpenRefusal) {
    engine.end = Some(End::Stopped(Stop::StoreOpen { why: why.clone() }));
}
pub(crate) fn unavailable(engine: &mut Engine, member: &[u8]) {
    let mut problems = Problems::empty();
    problems.push(Path::root().member(member), What::Refers);
    engine.end = Some(End::Stopped(Stop::Start(problems)));
}

/// Losing the webhook listener drains the engine (protocol/forge.md, 2.3).
pub(crate) fn hooks(engine: &mut Engine) {
    if engine.drain.is_none() {
        shutdown(engine);
    }
    if let Some(drain) = &mut engine.drain
        && drain.stopped.is_none()
    {
        drain.stopped = Some(Stop::Hooks);
    }
}

/// Preserve jig's named settings failure in the ordinary startup diagnostics.
pub(crate) fn root(engine: &mut Engine) {
    use jig_domain_core::RestartRequest;
    if let Some(failure) = engine.domain.restore_load_failure() {
        engine.end = Some(End::Stopped(Stop::RestoreLoad(failure)));
        return;
    }
    if engine.domain.restore_failure().is_none()
        && let Some(why) = engine.domain.forge_restore_failure()
    {
        engine.end = Some(End::Stopped(Stop::ForgeUnresolved { records: why.records }));
        return;
    }
    let problems = match engine.domain.restore_failure() {
        Some(RestartRequest::Settings { failure, changed }) => settings_problem(failure, changed),
        Some(RestartRequest::Unresolved { child, records }) => {
            engine.end = Some(End::Stopped(Stop::Unresolved { child, records }));
            return;
        }
        Some(RestartRequest::ProjectPolicy { project }) => project_policy_problem(project),
        Some(RestartRequest::HeldPastLimit { which, held, limit }) => held_limit_problem(which, held, limit),
        Some(RestartRequest::UnheldRevision { task, revision }) => unheld_revision_problem(task, revision),
        Some(RestartRequest::Refused { step }) => {
            engine.end = Some(End::Stopped(Stop::RestoreStep { step }));
            return;
        }
        Some(RestartRequest::Step(_) | RestartRequest::Idle) | None => {
            if let Some(step) = engine.domain.restart_failure() {
                engine.end = Some(End::Stopped(Stop::RestoreStep { step }));
                return;
            }
            unavailable(engine, b"deployment");
            return;
        }
    };
    engine.end = Some(End::Stopped(Stop::Start(problems)));
}
fn held_limit_problem(which: jig_domain_core::HeldLimit, held: u32, limit: u32) -> Problems {
    use jig_domain_core::HeldLimit;
    let name: &[u8] = match which {
        HeldLimit::Revisions => b"revisions",
        HeldLimit::Versions => b"versions",
    };
    let path = Path::root()
        .member(b"settings")
        .member(name)
        .member(b"held")
        .child(jig_protocol_config::Segment::Index(held))
        .member(b"allowed")
        .child(jig_protocol_config::Segment::Index(limit));
    let mut problems = Problems::empty();
    problems.push(path, What::Bound);
    problems
}
fn unheld_revision_problem(task: u64, revision: u32) -> Problems {
    let path = Path::root()
        .member(b"tasks")
        .member(skein_lib::Decimal::of(task).as_bytes())
        .member(b"settings")
        .member(b"revision")
        .child(jig_protocol_config::Segment::Index(revision));
    let mut problems = Problems::empty();
    problems.push(path, What::Refers);
    problems
}
fn project_policy_problem(project: u32) -> Problems {
    let path = Path::root()
        .member(b"projects")
        .child(jig_protocol_config::Segment::Index(project))
        .member(b"policy")
        .member(b"bytes");
    let mut problems = Problems::empty();
    problems.push(path, What::Bound);
    problems
}
fn settings_problem(failure: jig_domain_core::settings::Failure, changed: bool) -> Problems {
    use jig_domain_core::settings::{Fault, Setting};
    let origin: &[u8] = if changed { b"owner's stored change" } else { b"binary's default" };
    let path = Path::root().member(b"settings").member(origin);
    let path = match failure.setting {
        Setting::Charter(number) => path.member(b"charter").child(jig_protocol_config::Segment::Index(number)),
        Setting::Template(number) => path.member(b"template").child(jig_protocol_config::Segment::Index(number)),
        Setting::Policy(number) => path.member(b"policy").child(jig_protocol_config::Segment::Index(number)),
        Setting::Rules => path.member(b"rules"),
    };
    let fault: &[u8] = match failure.fault {
        Fault::Shape => b"shape",
        Fault::Count => b"count",
        Fault::Reference => b"reference",
        Fault::Rules => b"rules",
        Fault::Budget => b"budget",
        Fault::Grace => b"grace",
        Fault::Owner => b"owner",
    };
    let mut problems = Problems::empty();
    problems.push(path.member(fault), What::Form);
    problems
}

pub(crate) fn requested(engine: &Engine) -> bool {
    match &engine.drain {
        Some(drain) => drain.requested,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_settings_past_a_lowered_limit_name_the_limit_and_both_counts() {
        use jig_domain_core::HeldLimit;
        for (which, expected) in [
            (HeldLimit::Revisions, b"settings.revisions.held[3].allowed[2]: past its bound\n".as_slice()),
            (HeldLimit::Versions, b"settings.versions.held[3].allowed[2]: past its bound\n".as_slice()),
        ] {
            let problems = crate::startup::Problems::jig(held_limit_problem(which, 3, 2), &Path::root());
            assert_eq!(crate::startup::lines(&problems).as_ref(), expected);
        }
    }

    #[test]
    fn an_unheld_settings_revision_names_the_task_and_revision() {
        let problems = crate::startup::Problems::jig(unheld_revision_problem(4_294_967_297, 7), &Path::root());
        assert_eq!(
            crate::startup::lines(&problems).as_ref(),
            b"tasks.4294967297.settings.revision[7]: unknown reference\n"
        );
    }

    #[test]
    fn an_oversized_stored_project_policy_names_its_project_and_byte_bound() {
        let problems = crate::startup::Problems::jig(project_policy_problem(7), &Path::root());
        assert_eq!(crate::startup::lines(&problems).as_ref(), b"projects[7].policy.bytes: past its bound\n");
    }

    #[test]
    fn settings_start_refusals_name_the_setting_fault_and_source() {
        use jig_domain_core::settings::{Failure, Fault, Setting};
        for (changed, origin) in [(true, b"owner's stored change".as_slice()), (false, b"binary's default".as_slice())]
        {
            let problems = settings_problem(Failure { setting: Setting::Template(1), fault: Fault::Grace }, changed);
            let problem = problems.problems.get(0).unwrap();
            let path = &problem.path.segments;
            assert_eq!(path.get(0), Some(&jig_protocol_config::Segment::Member(b"settings".as_slice().into())));
            assert_eq!(path.get(1), Some(&jig_protocol_config::Segment::Member(origin.into())));
            assert_eq!(path.get(2), Some(&jig_protocol_config::Segment::Member(b"template".as_slice().into())));
            assert_eq!(path.get(3), Some(&jig_protocol_config::Segment::Index(1)));
            assert_eq!(path.get(4), Some(&jig_protocol_config::Segment::Member(b"grace".as_slice().into())));
        }
    }
}
