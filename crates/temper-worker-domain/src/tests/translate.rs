//! Every smith limit keeps its configured value and its classification.
use jig_domain_host as host;
use smith_host_domain as agent;

#[test]
fn every_smith_limit_names_a_hold_or_stays_a_model_failure() {
    use agent::{Cap, Limit};
    use host::Bound;
    let rows = [
        ("reasoning", Limit::Path(Cap::Reasoning), Some(Bound::Reasoning)),
        ("metadata", Limit::Path(Cap::Metadata), Some(Bound::Metadata)),
        ("output items", Limit::Path(Cap::OutputItems), Some(Bound::Items)),
        ("answer", Limit::Path(Cap::Answer), Some(Bound::Answer)),
        ("tools", Limit::Path(Cap::Tools), Some(Bound::Tools)),
        ("history items", Limit::Path(Cap::HistoryItems), Some(Bound::HistoryItems)),
        ("output", Limit::Path(Cap::Output), Some(Bound::Output)),
        ("history", Limit::History, Some(Bound::History)),
        ("window", Limit::Window, Some(Bound::Window)),
        ("pool", Limit::Pool { reservation: 4096 }, None),
        ("call", Limit::Call, None),
        ("message", Limit::Message, None),
        ("request", Limit::Path(Cap::Request), None),
        ("request head", Limit::Path(Cap::RequestHead), None),
        ("response head", Limit::Path(Cap::ResponseHead), None),
        ("response fields", Limit::Path(Cap::ResponseFields), None),
        ("skip", Limit::Path(Cap::Skip), None),
        ("field", Limit::Path(Cap::Field), None),
        ("retained", Limit::Path(Cap::Retained), None),
        ("tokens", Limit::Path(Cap::Tokens), None),
        ("depth", Limit::Path(Cap::Depth), None),
        ("strings", Limit::Path(Cap::Strings), None),
        ("number", Limit::Path(Cap::Number), None),
        ("error body", Limit::Path(Cap::ErrorBody), None),
        ("access token", Limit::Path(Cap::AccessToken), None),
        ("account id", Limit::Path(Cap::AccountId), None),
    ];
    for (name, which, bound) in rows {
        let failure = agent::RunFailure::Model(agent::ModelFault::Completion {
            failure: agent::CompletionFailure::Limit { which, bound: 2048 },
            evidence: agent::CompletionEvidence::Response { status: Some(200) },
            model: 1,
            attempts: 1,
            report: agent::Report::none(),
        });
        let expected = match bound {
            Some(bound) => host::RunFailure::Exceeded(host::Exceeded { bound, value: 2048 }),
            None => host::RunFailure::Model,
        };
        assert_eq!(crate::translate::run_failure(&failure), expected, "{name}");
    }
}

#[test]
fn a_host_part_start_refusal_keeps_smiths_bound() {
    for bound in [1, 33419, u64::MAX] {
        assert_eq!(
            crate::translate::start_cause(agent::RunInvalid::HostPart { bound }),
            host::StartCause::HostPart { bound }
        );
    }
}
