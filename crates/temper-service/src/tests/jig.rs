//! The deployment's two jig profiles, observed through startup's projections.
use super::fixtures;
use crate::{limits, settings, worker_settings, workers};
use jig_protocol_config::{self as config, overrides};
use limits::{JIG, Place};
use skein_lib::List;

#[derive(PartialEq, Eq, Debug)]
struct Engine {
    root: temper_engine_domain::Limits,
    channel: Option<jig_protocol_engine_workers::Limits>,
    driver: skein_oauth_accounts::Limits,
    clients: jig_protocol_engine_clients::Limits,
    resume_bytes: u32,
    tail_bytes: u32,
    hub_turns: u32,
    key_bytes: u32,
    redirect_bytes: u32,
}

fn engine_projection(engine: &mut settings::Engine, jig: &config::Limits) -> Engine {
    engine.jig.limits = *jig;
    let seeds = fixtures::seeds();
    let core = config::core(&engine.jig, seeds.deployment, seeds.seed);
    // Startup also installs the clients' effective wire bound here. Observe the
    // candidate even when an adjacent input is refused by envelope sizing.
    let _checked_root = crate::startup::engine_limits(&mut engine.jig.limits, &engine.limits);
    Engine {
        root: limits::engine_candidate(jig, &engine.limits),
        // A sizing refusal is a change in what the engine reads, too.
        channel: workers::limits(engine),
        // Startup passes these component limits through unchanged to their constructors.
        driver: engine.jig.limits.driver,
        clients: engine.jig.limits.clients,
        resume_bytes: core.settings.resume_bytes,
        tail_bytes: core.settings.tail_bytes,
        hub_turns: core.hub_turns,
        key_bytes: core.settings.key_bytes,
        redirect_bytes: core.settings.redirect_bytes,
    }
}

#[derive(PartialEq, Eq, Debug)]
struct Worker {
    root: temper_worker_domain::Limits,
    link: jig_protocol_worker_link::Limits,
    credential_bytes: u32,
}

fn worker_projection(
    worker: &mut settings::Worker,
    agent: &settings::Agent,
    own: &limits::Limits,
    jig: &config::Limits,
) -> Worker {
    worker.jig.limits = *jig;
    let payload = worker_settings::payload_limits(own).expect("the fixture's temper payload fits");
    Worker {
        root: worker_settings::root_limits(worker, agent, own),
        link: worker_settings::link_limits(&worker.jig, &payload)
            .expect("the changed profile has a worker link projection"),
        credential_bytes: worker_settings::credential_bytes(jig.token),
    }
}

fn changed(name: &str) -> config::Limits {
    let mut jig = config::profile(config::Profile::Small);
    let value = overrides::value(&jig, name.as_bytes()).expect("each jig name has a value");
    if let Some(next) = value.checked_add(1)
        && overrides::apply(&mut jig, name.as_bytes(), next).is_ok()
    {
        return jig;
    }
    jig = config::profile(config::Profile::Small);
    let Some(previous) = value.checked_sub(1) else {
        panic!("Question: no different value for {name}");
    };
    if let Err(why) = overrides::apply(&mut jig, name.as_bytes(), previous) {
        panic!("Question: neither adjacent value changes {name}: {why:?}");
    }
    jig
}

#[test]
fn jigs_names_each_have_one_row_in_order() {
    let mut names = List::with_capacity(u32::try_from(JIG.len()).unwrap());
    let mut expected = List::with_capacity(u32::try_from(config::names::NAMES.len()).unwrap());
    for row in JIG {
        names.push(row.name).unwrap();
    }
    for row in config::names::NAMES {
        expected.push(row.name).unwrap();
    }
    assert_eq!(names.as_slice(), expected.as_slice());
}

#[test]
fn each_place_is_where_temper_reads_the_name() {
    let small = config::profile(config::Profile::Small);
    let mut engine = fixtures::engine();
    let own = limits::profile(limits::Profile::Small);
    let input = fixtures::owned_worker_alone_document();
    let mut part = crate::startup::worker_part(&input, &config::Directories::working()).unwrap();
    let agent = fixtures::startup_agent();
    let engine_before = engine_projection(&mut engine, &small);
    let worker_before = worker_projection(&mut part.worker, &agent, &own, &small);
    let mut differences = List::with_capacity(u32::try_from(JIG.len()).unwrap());
    for row in JIG {
        let jig = changed(row.name);
        let engine_changed = engine_projection(&mut engine, &jig) != engine_before;
        let worker_changed = worker_projection(&mut part.worker, &agent, &own, &jig) != worker_before;
        let place = match (engine_changed, worker_changed) {
            (true, false) => Place::Engine,
            (false, true) => Place::Worker,
            (true, true) => Place::Both,
            (false, false) => Place::Neither,
        };
        if place != row.place {
            differences.push((row.name, place, row.place)).unwrap();
        }
    }
    assert_eq!(differences.as_slice(), &[], "(name, read by projections, table)");
}

#[test]
fn a_name_read_nowhere_says_what_temper_does() {
    // The owner's one-line note bound (the task's outer layer).
    const NOTE_BYTES: usize = 200;
    for row in JIG {
        if row.place == Place::Neither {
            assert!(!row.note.is_empty(), "Question: {} needs the owner's note", row.name);
        }
        if !row.note.is_empty() {
            assert!(row.note.len() <= NOTE_BYTES, "Question: {} has an overlong note", row.name);
            assert!(!row.note.contains(['\n', '\r']), "Question: {} has a multiline note", row.name);
            assert!(row.note.ends_with('.'), "Question: {} has an unfinished note", row.name);
        }
    }
}
