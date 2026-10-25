//! Local-agent composition at the startup/store/wire boundaries (configuration.md, 7).
#![expect(clippy::disallowed_methods, clippy::disallowed_types, reason = "ordinary tests edit configuration fixtures")]
#![expect(clippy::disallowed_macros, reason = "ordinary tests format explicit scalar overrides")]
use super::*;
use crate::tests::fixtures;

fn document(engine: Option<u32>, worker: Option<u32>) -> Box<[u8]> {
    let mut text = core::str::from_utf8(&fixtures::document()).unwrap().to_string();
    for bound in [engine, worker] {
        let replacement = bound.map_or_else(String::new, |bytes| format!(",\"core.call_answer_bytes\":{bytes}"));
        text = text.replacen(",\"core.call_answer_bytes\":2048", &replacement, 1);
    }
    text.into_bytes().into_boxed_slice()
}

#[test]
fn omitted_call_answers_reach_the_core_store_and_both_wire_sides() {
    let directories = config::Directories::working();
    for (engine, worker, payload, expected_engine, expected_worker) in [
        (None, None, 4096, 32_768, 32_768),
        (None, None, 32_768, 32_768, 32_768),
        (None, None, 40_960, 40_960, 40_960),
        (Some(32_768), None, 40_960, 32_768, 40_960),
        (None, Some(32_768), 40_960, 40_960, 32_768),
        (Some(2048), Some(2048), 4096, 2048, 2048),
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
        let required = memory_requirements(
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
        assert_eq!(required, checked.requirements);
        let terms = memory_terms(
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
        assert_eq!(memory_sum(&terms.engine), Some(required.engine_bytes));
        assert_eq!(memory_sum(&terms.worker), Some(required.worker_bytes));
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
fn worker_only_runtime_and_memory_use_the_same_composed_limits() {
    let directories = config::Directories::working();
    for (explicit, expected) in [(false, 40_960), (true, 32_768)] {
        let input = fixtures::worker_alone_document();
        let replacement = if explicit { ",\"core.call_answer_bytes\":32768" } else { "" };
        let input = core::str::from_utf8(&input).unwrap().replace(",\"core.call_answer_bytes\":2048", replacement);
        let mut agent = fixtures::startup_agent();
        agent.tool_payload = 40_960;
        let part = worker_part(input.as_bytes(), &directories).unwrap();
        let service = worker_alone(part, &fixtures::files(), &agent, fixtures::worker_runtime(), 7).unwrap();
        let required = memory_requirements(
            input.as_bytes(),
            &directories,
            &fixtures::files(),
            &agent,
            &fixtures::seeds(),
            None,
            List::with_capacity(0),
            None,
        )
        .unwrap();
        assert_eq!(required.engine_bytes, 0);
        let terms = memory_terms(
            input.as_bytes(),
            &directories,
            &fixtures::files(),
            &agent,
            &fixtures::seeds(),
            None,
            List::with_capacity(0),
            None,
        )
        .unwrap();
        assert_eq!(memory_sum(&terms.worker), Some(required.worker_bytes));
        let mut expected_part = worker_part(input.as_bytes(), &directories).unwrap();
        expected_part.worker.jig.limits.core.call_answer_bytes = expected;
        assert_eq!(
            required.worker_bytes,
            crate::system::worker_alone_worst_case(&expected_part.worker, &agent, &expected_part.limits).unwrap()
        );
        assert!(service.engine.is_none());
        // A one-byte-short memory declaration refuses the same runtime envelope.
        let short = fixtures::with(
            input.as_bytes(),
            b"workers[0].memory_bytes",
            (required.worker_bytes - 1).to_string().as_bytes(),
        );
        let found = worker_alone(
            worker_part(&short, &directories).unwrap(),
            &fixtures::files(),
            &agent,
            fixtures::worker_runtime(),
            7,
        )
        .err()
        .unwrap();
        assert_eq!(found.problems.get(0).unwrap().what, What::Jig(config::What::Memory { over: 1 }));
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
