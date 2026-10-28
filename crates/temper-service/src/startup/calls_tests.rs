//! Local-agent composition at the startup/store/wire boundaries (configuration.md, 7).
#![expect(clippy::disallowed_methods, clippy::disallowed_types, reason = "ordinary tests edit configuration fixtures")]
#![expect(clippy::disallowed_macros, reason = "ordinary tests format explicit scalar overrides")]
use super::*;
use crate::tests::fixtures;
use core::fmt::Write;

fn document(engine: Option<u32>, worker: Option<u32>) -> Box<[u8]> {
    let input = fixtures::document();
    let mut remaining = core::str::from_utf8(&input).unwrap();
    let mut text = String::new();
    for bound in [engine, worker] {
        let (before, after) = remaining.split_once(",\"core.call_answer_bytes\":2048").unwrap();
        text.push_str(before);
        if let Some(bytes) = bound {
            write!(text, ",\"core.call_answer_bytes\":{bytes}").unwrap();
        }
        remaining = after;
    }
    text.push_str(remaining);
    text.into_bytes().into_boxed_slice()
}

fn worker_document(bound: Option<u32>) -> Box<[u8]> {
    let input = fixtures::worker_alone_document();
    let replacement = bound.map_or_else(String::new, |bytes| format!(",\"core.call_answer_bytes\":{bytes}"));
    core::str::from_utf8(&input)
        .unwrap()
        .replace(",\"core.call_answer_bytes\":2048", &replacement)
        .into_bytes()
        .into_boxed_slice()
}

#[test]
fn omitted_call_answers_reach_the_core_store_and_both_wire_sides() {
    let directories = config::Directories::working();
    for (engine, worker, payload, expected_engine, expected_worker) in [
        (None, None, 4096, 32_768, 32_768),
        (None, None, 32_768, 32_768, 32_768),
        (None, None, 40_960, 40_960, 40_960),
        (Some(4096), None, 4096, 4096, 32_768),
        (None, Some(4096), 4096, 32_768, 4096),
        (Some(32_768), Some(32_768), 32_768, 32_768, 32_768),
        (Some(40_960), Some(40_960), 4096, 40_960, 40_960),
    ] {
        let input = document(engine, worker);
        let mut agent = fixtures::startup_agent();
        agent.tool_payload = payload;
        let checked = checked_start(
            &input,
            &directories,
            &fixtures::files(),
            Some(&agent),
            &fixtures::seeds(),
            None,
            fixtures::forge_runtime(),
            None,
        )
        .unwrap();
        assert_eq!(checked.engine.jig.limits.core.call_answer_bytes, expected_engine);
        let wire = crate::workers::limits(&checked.engine).unwrap();
        assert_eq!((wire.sizes.call_input, wire.sizes.answer), (expected_engine, expected_engine));
        let worker = checked.worker.as_ref().unwrap();
        let payload = crate::worker_settings::payload_limits(&checked.engine.limits).unwrap();
        let wire = crate::worker_settings::link_limits(&worker.jig, &payload).unwrap();
        assert_eq!((wire.sizes.call_input, wire.sizes.answer), (expected_worker, expected_worker));
        let service = startup(
            &input,
            &directories,
            &fixtures::files(),
            &agent,
            &fixtures::seeds(),
            None,
            fixtures::worker_runtime(),
            fixtures::forge_runtime(),
        )
        .unwrap();
        let engine = service.engine.as_ref().unwrap();
        assert_eq!(engine.env.limits.call_answer_bytes, expected_engine);
        let opening = store_limits(&input, &directories, Some(&agent)).unwrap();
        assert_eq!(opening.store, engine.store.opening_limits());
        assert_eq!(opening.store.core.call_answer_bytes, expected_engine);
        check(
            &input,
            &directories,
            &fixtures::files(),
            &agent,
            &fixtures::seeds(),
            None,
            fixtures::forge_runtime(),
            None,
        )
        .unwrap();
    }
}

#[test]
fn store_readers_require_local_agent_metadata_but_leave_remote_engines_configured() {
    let directories = config::Directories::working();
    let found = store_limits(&document(None, None), &directories, None).unwrap_err();
    assert_eq!(found.problems.get(0).unwrap().path, indexed(0).member(b"agent"));
    assert_eq!(found.problems.get(0).unwrap().what, What::Agent);
    let input = fixtures::engine_alone_document();
    let input = core::str::from_utf8(&input).unwrap().replace(",\"core.call_answer_bytes\":2048", "");
    let agent = fixtures::startup_agent();
    for metadata in [None, Some(&agent)] {
        assert_eq!(
            store_limits(input.as_bytes(), &directories, metadata).unwrap().store.core.call_answer_bytes,
            32_768
        );
    }
}

#[test]
fn worker_only_runtime_and_checks_use_the_same_composed_limits() {
    let directories = config::Directories::working();
    for (bound, expected) in [(None, 40_960), (Some(40_960), 40_960)] {
        let input = worker_document(bound);
        let mut agent = fixtures::startup_agent();
        agent.tool_payload = 40_960;
        let part = worker_part(&input, &directories).unwrap();
        let service = worker_alone(part, &fixtures::files(), &agent, fixtures::worker_runtime(), 7).unwrap();
        check(&input, &directories, &fixtures::files(), &agent, &fixtures::seeds(), None, List::with_capacity(0), None)
            .unwrap();
        assert!(service.engine.is_none());
        let mut part = worker_part(&input, &directories).unwrap();
        check_worker_part(&mut part, &fixtures::files(), &agent).unwrap();
        assert_eq!(part.worker.jig.limits.core.call_answer_bytes, expected);
        let payload = crate::worker_settings::payload_limits(&part.limits).unwrap();
        let wire = crate::worker_settings::link_limits(&part.worker.jig, &payload).unwrap();
        assert_eq!((wire.sizes.call_input, wire.sizes.answer), (expected, expected));
        assert_eq!(
            service.worker.as_ref().unwrap().env.limits,
            crate::worker_settings::root_limits(&part.worker, &agent, &part.limits)
        );
    }
}

fn call_input_lines(path: &str, bytes: u32, input: u32, ask: u32, reply: u32) -> String {
    format!(
        "{path}: {bytes} bytes, short of the agent's call input (profile.declared.tool_payload), {input} bytes; raise it to at least {input} bytes, or omit it to follow the agent's call input\n{path}: call input {ask} bytes; wire answer {bytes} bytes; smith host_reply_bytes {reply} bytes\n"
    )
}

fn answer_floor_line(path: &str, bytes: u32) -> String {
    format!("{path}: {bytes} bytes, short of temper's documentation answer (read_docs), 2048 bytes\n")
}

fn standalone_refusals(input: &[u8], agent: &settings::Agent) -> [Problems; 3] {
    let directories = config::Directories::working();
    [
        startup(
            input,
            &directories,
            &fixtures::files(),
            agent,
            &fixtures::seeds(),
            None,
            fixtures::worker_runtime(),
            fixtures::forge_runtime(),
        )
        .err()
        .unwrap(),
        store_limits(input, &directories, Some(agent)).unwrap_err(),
        check(
            input,
            &directories,
            &fixtures::files(),
            agent,
            &fixtures::seeds(),
            None,
            fixtures::forge_runtime(),
            None,
        )
        .unwrap_err(),
    ]
}

fn worker_refusals(input: &[u8], agent: &settings::Agent) -> [Problems; 2] {
    let directories = config::Directories::working();
    [
        worker_alone(
            worker_part(input, &directories).unwrap(),
            &fixtures::files(),
            agent,
            fixtures::worker_runtime(),
            7,
        )
        .err()
        .unwrap(),
        check(input, &directories, &fixtures::files(), agent, &fixtures::seeds(), None, List::with_capacity(0), None)
            .unwrap_err(),
    ]
}

fn core_answer_floor(engine: bool) {
    let agent = fixtures::startup_agent();
    let input = if engine { document(Some(2047), Some(2048)) } else { document(Some(2048), Some(2047)) };
    let path = if engine {
        "engine.limits.overrides.core.call_answer_bytes"
    } else {
        "workers[0].limits.overrides.core.call_answer_bytes"
    };
    let expected = call_input_lines(path, 2047, 2048, 4096, 65_536) + &answer_floor_line(path, 2047);
    for found in standalone_refusals(&input, &agent) {
        assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
    }
    let service = startup(
        &document(Some(2048), Some(2048)),
        &config::Directories::working(),
        &fixtures::files(),
        &agent,
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
    .unwrap();
    assert_eq!(service.engine.as_ref().unwrap().env.limits.call_answer_bytes, 2048);
}

#[test]
fn the_engines_call_answer_bound_must_hold_the_documentation_answer() {
    core_answer_floor(true);
}

#[test]
fn a_workers_call_answer_bound_must_hold_the_documentation_answer() {
    core_answer_floor(false);
    let agent = fixtures::startup_agent();
    let expected = call_input_lines("workers[0].limits.overrides.core.call_answer_bytes", 2047, 2048, 4096, 65_536)
        + &answer_floor_line("workers[0].limits.overrides.core.call_answer_bytes", 2047);
    for found in worker_refusals(&worker_document(Some(2047)), &agent) {
        assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
    }
    let service = worker_alone(
        worker_part(&worker_document(Some(2048)), &config::Directories::working()).unwrap(),
        &fixtures::files(),
        &agent,
        fixtures::worker_runtime(),
        7,
    )
    .unwrap();
    assert!(service.engine.is_none());
}

#[test]
fn a_local_agents_tool_payload_must_hold_the_documentation_answer() {
    let mut agent = fixtures::startup_agent();
    agent.tool_payload = 2047;
    let expected = answer_floor_line("workers[0].agent.profile.declared.tool_payload", 2047);
    for found in standalone_refusals(&document(Some(2048), Some(2048)), &agent) {
        assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
    }
    for found in worker_refusals(&worker_document(Some(2048)), &agent) {
        assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
    }
    agent.tool_payload = 2048;
    let directories = config::Directories::working();
    startup(
        &document(Some(2048), Some(2048)),
        &directories,
        &fixtures::files(),
        &agent,
        &fixtures::seeds(),
        None,
        fixtures::worker_runtime(),
        fixtures::forge_runtime(),
    )
    .unwrap();
    worker_alone(
        worker_part(&worker_document(Some(2048)), &directories).unwrap(),
        &fixtures::files(),
        &agent,
        fixtures::worker_runtime(),
        7,
    )
    .unwrap();
}

#[test]
fn an_engine_without_a_local_agent_still_needs_the_documentation_answer_bound() {
    let input = fixtures::engine_alone_document();
    let short =
        core::str::from_utf8(&input).unwrap().replace("core.call_answer_bytes\":2048", "core.call_answer_bytes\":2047");
    let mut agent = fixtures::startup_agent();
    agent.tool_payload = 1;
    let expected = answer_floor_line("engine.limits.overrides.core.call_answer_bytes", 2047);
    for found in standalone_refusals(short.as_bytes(), &agent) {
        assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
    }
    let found = store_limits(short.as_bytes(), &config::Directories::working(), None).unwrap_err();
    assert_eq!(core::str::from_utf8(&lines(&found)).unwrap(), expected);
}

#[test]
fn explicit_short_call_inputs_refuse_every_standalone_entrance() {
    let directories = config::Directories::working();
    // The old M2 explicit cases remain refusals, alongside P - 1 at each override.
    for (engine, worker, payload, paths, bound) in [
        (Some(4095), None, 4096, vec!["engine"], 4095),
        (None, Some(4095), 4096, vec!["workers[0]"], 4095),
        (Some(2048), Some(2048), 4096, vec!["engine", "workers[0]"], 2048),
        (Some(32_768), None, 40_960, vec!["engine"], 32_768),
        (None, Some(32_768), 40_960, vec!["workers[0]"], 32_768),
    ] {
        let input = document(engine, worker);
        let mut agent = fixtures::startup_agent();
        agent.tool_payload = payload;
        let mut expected = String::new();
        for path in paths {
            expected.push_str(&call_input_lines(
                &format!("{path}.limits.overrides.core.call_answer_bytes"),
                bound,
                payload,
                4096,
                65_536,
            ));
        }
        let found = [
            startup(
                &input,
                &directories,
                &fixtures::files(),
                &agent,
                &fixtures::seeds(),
                None,
                fixtures::worker_runtime(),
                fixtures::forge_runtime(),
            )
            .err()
            .unwrap(),
            store_limits(&input, &directories, Some(&agent)).unwrap_err(),
            check(
                &input,
                &directories,
                &fixtures::files(),
                &agent,
                &fixtures::seeds(),
                None,
                fixtures::forge_runtime(),
                None,
            )
            .unwrap_err(),
        ];
        for problems in found {
            assert_eq!(core::str::from_utf8(&lines(&problems)).unwrap(), expected);
            assert_eq!(problems.problems.len(), if engine.is_some() && worker.is_some() { 2 } else { 1 });
        }
        // A refusal preserves each configured value, rather than silently deriving it.
        let mut parsed = parse(&input, &directories).unwrap();
        parsed.compose(&agent).unwrap_err();
        if let Some(bytes) = engine {
            assert_eq!(parsed.engine.limits.core.call_answer_bytes, bytes);
        }
        if let Some(bytes) = worker {
            assert_eq!(parsed.workers[0].limits.core.call_answer_bytes, bytes);
        }
    }
}

#[test]
fn explicit_short_call_inputs_refuse_every_worker_only_entrance() {
    let directories = config::Directories::working();
    for (bound, payload) in [(4095, 4096), (32_768, 40_960)] {
        let input = worker_document(Some(bound));
        let mut agent = fixtures::startup_agent();
        agent.tool_payload = payload;
        let expected =
            call_input_lines("workers[0].limits.overrides.core.call_answer_bytes", bound, payload, 4096, 65_536);
        let found = [
            worker_alone(
                worker_part(&input, &directories).unwrap(),
                &fixtures::files(),
                &agent,
                fixtures::worker_runtime(),
                7,
            )
            .err()
            .unwrap(),
            check(
                &input,
                &directories,
                &fixtures::files(),
                &agent,
                &fixtures::seeds(),
                None,
                List::with_capacity(0),
                None,
            )
            .unwrap_err(),
        ];
        for problems in found {
            assert_eq!(core::str::from_utf8(&lines(&problems)).unwrap(), expected);
            assert_eq!(problems.problems.len(), 1);
        }
    }
}

#[test]
fn an_engine_without_a_local_agent_keeps_its_explicit_call_bound() {
    let input = fixtures::engine_alone_document();
    let directories = config::Directories::working();
    let mut agent = fixtures::startup_agent();
    for payload in [1, 2047, 2048, 4096, u32::MAX] {
        agent.tool_payload = payload;
        let checked = checked_start(
            &input,
            &directories,
            &fixtures::files(),
            Some(&agent),
            &fixtures::seeds(),
            None,
            fixtures::forge_runtime(),
            None,
        )
        .unwrap();
        assert_eq!(checked.engine.jig.limits.core.call_answer_bytes, 2048);
        assert_eq!(store_limits(&input, &directories, Some(&agent)).unwrap().store.core.call_answer_bytes, 2048);
    }
}

#[test]
fn per_kind_terms_reject_call_and_reply_mismatches_even_with_larger_other_frames() {
    use jig_protocol_worker_wire as wire;
    use skein_channel::{Role, Term};
    let worker = fixtures::worker_settings();
    let own = fixtures::worker_limits();
    let payload = crate::worker_settings::payload_limits(&own).unwrap();
    let mut link = crate::worker_settings::link_limits(&worker.jig, &payload).unwrap();
    link.sizes.call_input = 40_960;
    link.sizes.answer = 40_960;
    let larger = wire::schema(&wire::sizes::limits(&link.sizes).unwrap()).unwrap();
    link.sizes.call_input = 32_768;
    link.sizes.answer = 32_768;
    let smaller = wire::schema(&wire::sizes::limits(&link.sizes).unwrap()).unwrap();
    for (role, kind) in [(Role::Initiator, 0x0102), (Role::Responder, 0x0182)] {
        for short in [true, false] {
            let mut terms = List::with_capacity(u32::try_from(wire::KINDS.len()).unwrap());
            for entry in &larger.version(wire::VERSION).unwrap().kinds {
                let largest = if short && entry.kind == kind {
                    smaller.version(wire::VERSION).unwrap().kind(kind).unwrap().largest
                } else {
                    entry.largest
                };
                terms.push(Term { kind: entry.kind, largest }).unwrap();
            }
            assert_eq!(wire::terms_gap(&larger, role, &terms), if short { Some(kind) } else { None });
        }
    }
}
