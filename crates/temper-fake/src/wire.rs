//! Typed wire limits for the configurations owned by temper-fake (protocol/setup.md, 8.4).
use crate::{limits as l, llm};
use alloc::boxed::Box;
use skein_fake_forge_protocol as forgejo;
use skein_fake_llm_protocol::{documents, provider};
use skein_llm::{Credential, Scheme};

pub(crate) fn forge_limits() -> forgejo::Limits {
    forgejo::Limits {
        calls: l::CONNECTIONS,
        connections: l::CONNECTIONS,
        raw_bytes: l::FORGE_RAW_BYTES,
        http: skein_http::server::Limits {
            head: l::HEAD,
            headers: l::HEADERS,
            body: u64::from(l::FORGE_BODY),
            read: l::READ,
            response: l::HEAD,
            send: l::SEND,
        },
        documents: skein_forge::forgejo::limits::Limits {
            json: skein_json::tokenizer::Limits {
                chunk: l::READ,
                string: l::FORGE_BODY,
                number: l::JSON_NUMBER,
                depth: l::JSON_DEPTH,
                length: l::FORGE_BODY,
            },
            writer: skein_json::writer::Limits { depth: l::JSON_DEPTH, length: l::FORGE_BODY },
            string_bytes: l::FORGE_STRING_BYTES,
            body_bytes: l::FORGE_BODY,
            inner: l::FORGE_INNER,
            target_bytes: l::PATH_BYTES,
        },
    }
}
pub(crate) fn llm_limits() -> provider::Limits {
    let native = skein_llm::openai::Limits {
        calls_per_response: 1,
        request: l::LLM_REQUEST_BYTES,
        retained: l::LLM_REQUEST_BYTES,
        strings: l::LLM_REQUEST_BYTES,
        depth: l::JSON_DEPTH,
        tokens: l::LLM_TOKENS,
        output_items: crate::ENTRIES,
        tools: l::LLM_TOOLS,
        history_items: l::LLM_HISTORY,
        metadata: l::LLM_METADATA_BYTES,
        receiving: l::LLM_REQUEST_BYTES,
        skip: l::LLM_REQUEST_BYTES,
        input: l::LLM_REQUEST_BYTES,
        reasoning: crate::ANSWER_BYTES,
        answer: crate::ANSWER_BYTES,
        detail_bytes: l::LLM_DETAIL_BYTES,
    };
    provider::Limits {
        calls: l::CONNECTIONS,
        http: skein_http::server::Limits {
            head: l::HEAD,
            headers: l::HEADERS,
            body: u64::from(l::LLM_REQUEST_BYTES),
            read: l::READ,
            response: l::HEAD,
            send: l::SEND,
        },
        sse: skein_http::sse::writer::Limits { event: l::LLM_REQUEST_BYTES, chunk: l::SEND },
        documents: documents::Limits { anthropic: native, openai: native, model_ceiling: l::MODEL_CEILING },
    }
}
pub(crate) fn llm(
    configuration: llm::Configuration,
    recording: bool,
) -> Result<skein_fake_peers::llm::Peer, skein_fake_peers::Error> {
    let mut limits = l::peer();
    if recording {
        limits.observations = l::LLM_OBSERVATIONS;
        limits.observation_bytes = l::LLM_OBSERVATION_BYTES;
    }
    let mut peer = skein_fake_peers::llm::Peer::new(
        configuration.listen,
        if configuration.tls { skein_fake_peers::Transport::Tls } else { skein_fake_peers::Transport::Plaintext },
        limits,
        provider::Config {
            without_credential: false,
            echo: skein_llm::openai::Echo::NONE,
            usage_fields: documents::UsageFields::ALL,
            provider: documents::Provider::OpenAi,
            path: llm::ROUTE.into(),
            headers: Box::new([]),
        },
        Credential { scheme: Scheme::Key, value: configuration.key.expose().into(), account_id: Box::new([]) },
        llm_limits(),
        configuration.domain,
        llm::LIMITS,
    )?;
    for key in configuration.added_keys {
        peer.accept(Credential::key(key.expose().into()))?;
    }
    if recording { Ok(peer) } else { Ok(peer.without_records()) }
}
