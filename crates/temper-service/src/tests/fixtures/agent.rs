use crate::settings;
pub fn agent() -> settings::Agent {
    use skein_lib::Duration;
    let mut endpoints = skein_lib::List::with_capacity(1);
    endpoints
        .push(smith_protocol_channel::Endpoint { name: b"fake".as_slice().into(), number: 1, dialect: super::DIALECT })
        .unwrap();
    let host_limits = smith_host_domain::Limits {
        agents: 1,
        directories: 1,
        conflicts: 4,
        path_bytes: 255,
        name_bytes: 64,
        accounts: 2,
        charter_bytes: 65_536,
        transcript_bytes: 262_144,
        answered_bytes: 65_536,
        message_bytes: 65_536,
        messages: 2,
        calls: 1,
        call_bytes: 65_536,
        answer_bytes: smith_host_domain::Delivered::worst_case(),
        turns: 1,
        turn_bytes: 65_536,
        unacknowledged_bytes: 65_536,
        fact_bytes: 65_536,
        outcome_bytes: 4096,
        detail_bytes: 64,
        spawn_timeout: Duration::from_secs(1),
        no_progress: Duration::from_secs(120),
        silence: Duration::from_secs(60),
        long_span: Duration::from_secs(60),
        long_open: 1,
        wall_time: Duration::from_secs(300),
        cancel_grace: Duration::from_secs(5),
        exit_grace: Duration::from_secs(5),
        teardown: Duration::from_secs(1),
        kill_after: Duration::from_secs(2),
        facts: 1,
    };
    let mut limits = smith_host_protocol::Limits {
        bodies: smith_channel::Limits {
            turn_body: 65_536,
            accepted_result: 4096,
            host_ask_input: 4096,
            message_text: 65_536,
            host_reply_text: 65_536,
            start_charter: 65_536,
            start_transcript: 262_144,
            start_answered: 4,
            start_grants: 2,
            start_messages: 2,
            ..smith_channel::CEILINGS
        },
        charter: smith_charter::CEILINGS,
        transcript: smith_transcript::CEILINGS,
        endpoints: 1,
        channel: skein_channel::Limits {
            chunk: 64,
            credential: 1,
            skip: 1_048_576,
            output_bytes: 1_048_576,
            output_frames: 8,
            kinds: 32,
        },
        start_bytes: 1_048_576,
        calls: 1,
    };
    limits.channel.output_bytes = smith_host_protocol::output_bytes(&limits).expect("host frames");
    settings::Agent {
        tool_payload: 4096,
        programs: Box::new([]),
        endpoints: smith_protocol_channel::Endpoints::new(endpoints),
        limits,
        host_limits,
        bounds: jig_domain_host::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([jig_domain_host::ModelBound {
                name: b"small".as_slice().into(),
                window: 8192,
                output: 1024,
            }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(300),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 65_536,
            inbox: 2,
            unacknowledged: 1,
        },
    }
}
