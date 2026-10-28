use crate::tests::fixtures as f;
use jig_client::contract as c;

fn refusal(which: c::Limit, expected: &str, json: &str) {
    for output_json in [false, true] {
        let args: &[&str] = if output_json {
            &["task", "message", "41", "hello", "--key", "limit", "--json"]
        } else {
            &["task", "message", "41", "hello", "--key", "limit"]
        };
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(args);
        root.serve(call, f::task_result(f::offered(&[c::ActKind::Message])));
        root.answered(call, c::Answer::Refused { refusal: c::Refusal::Limit { which }, at: None });
        let answer = root.answer(call).expect("limit answered");
        assert_eq!(answer.status, 2);
        if output_json {
            assert!(
                skein_lib::bytes::find(&answer.stderr, json.as_bytes()).is_some(),
                "{}",
                String::from_utf8_lossy(&answer.stderr)
            );
        } else {
            assert_eq!(answer.stderr.as_ref(), format!("task message: past {expected}\n").as_bytes());
        }
    }
}
#[test]
fn every_engine_limit_has_its_own_name_and_next_step_in_text_and_json() {
    for (which, expected, tag) in [
        (c::AdmissionLimit::Words, "the engine's request size; shorten it", "words"),
        (c::AdmissionLimit::Amendment, "the engine's amendment size; reduce the amendment", "amendment"),
        (c::AdmissionLimit::Key, "the account key size; use a shorter key", "key"),
        (c::AdmissionLimit::Name, "the name size; use a shorter name", "name"),
        (c::AdmissionLimit::Projects, "the deployment's projects; use an existing project", "projects"),
        (
            c::AdmissionLimit::SettingsProposals,
            "the pending settings proposals; decide or withdraw a settings proposal",
            "settings_proposals",
        ),
        (c::AdmissionLimit::Goals, "the project's live goals; end or wait for goals", "goals"),
        (c::AdmissionLimit::Operations, "the operations per request; split the request", "operations"),
        (c::AdmissionLimit::Grace, "the cancellation grace; use a shorter grace", "grace"),
        (c::AdmissionLimit::Identity, "the identity rule; use a valid identity", "identity"),
        (c::AdmissionLimit::Roles, "the project roles; choose valid roles and name each party once", "roles"),
        (
            c::AdmissionLimit::Arithmetic,
            "the engine's counter; report the exhausted counter to the engine owner",
            "arithmetic",
        ),
        (c::AdmissionLimit::Duplicate, "duplicate input; remove the duplicate", "duplicate"),
        (c::AdmissionLimit::Empty, "empty input; provide at least one item", "empty"),
        (c::AdmissionLimit::Batch, "the task batch; split the batch", "batch"),
        (c::AdmissionLimit::ProjectLiveTasks, "the project's live tasks; end or wait for tasks", "project_live_tasks"),
        (c::AdmissionLimit::LiveTasks, "the engine's live tasks; end or wait for tasks", "live_tasks"),
        (c::AdmissionLimit::Tree, "the task tree; use a smaller task tree", "tree"),
        (c::AdmissionLimit::Depth, "the task depth; use fewer delegation levels", "depth"),
        (c::AdmissionLimit::Delegates, "the task's delegates; end or wait for delegates", "delegates"),
        (c::AdmissionLimit::Subscription, "the task's subscriptions; remove a subscription", "subscription"),
        (c::AdmissionLimit::Dependencies, "the task's dependencies; reduce the dependencies", "dependencies"),
        (c::AdmissionLimit::Cycle, "a dependency cycle; remove the dependency cycle", "cycle"),
        (c::AdmissionLimit::Spec, "the task's specification; reduce the specification", "spec"),
        (c::AdmissionLimit::Contract, "the result contract; reduce the result contract", "contract"),
        (c::AdmissionLimit::Authority, "the authority shape; reduce the authority", "authority"),
        (c::AdmissionLimit::Inputs, "the task's inputs; reduce the inputs", "inputs"),
        (c::AdmissionLimit::Attempts, "the task's attempt; read the task and use its current attempt", "attempts"),
        (c::AdmissionLimit::LiveDelegates, "the live delegates; end or wait for delegates", "live_delegates"),
        (
            c::AdmissionLimit::Restore,
            "the engine's restore state; restore records before marking the engine ready",
            "restore",
        ),
        (c::AdmissionLimit::Read, "the task's read position; read the task and use its offered read position", "read"),
        (c::AdmissionLimit::Turn, "the task's turn; use the next turn and nondecreasing expense", "turn"),
        (c::AdmissionLimit::HoldKind, "the resource hold kind; use a configured resource hold kind", "hold_kind"),
        (c::AdmissionLimit::HoldTaken, "the resource hold; wait for the resource, or release its hold", "hold_taken"),
        (c::AdmissionLimit::Holds, "the resource holds; release a hold or wait for room", "holds"),
        (c::AdmissionLimit::InboxFull, "the task's inbox; read the inbox", "inbox_full"),
        (c::AdmissionLimit::Notes, "the notes; remove a note or reduce its size", "notes"),
        (c::AdmissionLimit::Revision, "the revision; read the current revision", "revision"),
        (c::AdmissionLimit::Question, "the question size; shorten the question", "question"),
        (c::AdmissionLimit::Priority, "the priority batch; reduce the priority batch", "priority"),
        (c::AdmissionLimit::Policy, "the policy size; reduce the policy", "policy"),
        (c::AdmissionLimit::Resource, "the resource size; reduce the resource", "resource"),
        (c::AdmissionLimit::Service, "the service size; reduce the service", "service"),
        (c::AdmissionLimit::Accounts, "the deployment's accounts; remove an unused account", "accounts"),
        (c::AdmissionLimit::HolderAccounts, "the holder's accounts; remove an unused account", "holder_accounts"),
        (c::AdmissionLimit::Sources, "the account sources; remove an unused source", "sources"),
        (c::AdmissionLimit::SignIns, "the sign-ins in progress; finish or cancel a sign-in", "sign_ins"),
        (c::AdmissionLimit::PlacedHolds, "the placed holds; lift a placed hold", "placed_holds"),
        (
            c::AdmissionLimit::ProjectPlacedHolds,
            "the project's placed holds; lift a placed hold",
            "project_placed_holds",
        ),
        (c::AdmissionLimit::PartyPlacedHolds, "the party's placed holds; lift a placed hold", "party_placed_holds"),
        (c::AdmissionLimit::Exemptions, "the hold exemptions; remove an exemption", "exemptions"),
    ] {
        refusal(
            c::Limit::Admission { which, bound: None },
            expected,
            &format!("\"admission\":{{\"which\":\"{tag}\",\"bound\":null}}"),
        );
    }
}
#[test]
fn configured_fixed_and_legacy_bounds_keep_their_values_units_and_remedies() {
    for (which, expected, json) in [
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Words,
                bound: Some(c::LimitBound::configured(2048, c::LimitKey::PeopleWords, c::LimitUnit::Bytes)),
            },
            "the engine's request size, 2,048 bytes; shorten it, or raise people.words (temper help configuration)",
            r#""bound":{"value":2048,"key":"people.words","unit":"bytes"}"#,
        ),
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Amendment,
                bound: Some(c::LimitBound::configured(8192, c::LimitKey::PeopleAmendmentBytes, c::LimitUnit::Bytes)),
            },
            "the engine's amendment size, 8,192 bytes; reduce the amendment, or raise people.amendment_bytes (temper help configuration)",
            r#""bound":{"value":8192,"key":"people.amendment_bytes","unit":"bytes"}"#,
        ),
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Name,
                bound: Some(c::LimitBound::fixed(64, c::LimitUnit::Bytes)),
            },
            "the name size, 64 bytes; use a shorter name",
            r#""bound":{"value":64,"key":null,"unit":"bytes"}"#,
        ),
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Key,
                bound: Some(c::LimitBound::fixed(256, c::LimitUnit::Bytes)),
            },
            "the account key size, 256 bytes; use a shorter key",
            r#""bound":{"value":256,"key":null,"unit":"bytes"}"#,
        ),
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Contract,
                bound: Some(c::LimitBound::fixed(3, c::LimitUnit::Bytes)),
            },
            "the result size, 3 bytes; shorten the result",
            r#""bound":{"value":3,"key":null,"unit":"bytes"}"#,
        ),
        (
            c::Limit::Admission {
                which: c::AdmissionLimit::Grace,
                bound: Some(c::LimitBound::configured(1000, c::LimitKey::TasksGrace, c::LimitUnit::Milliseconds)),
            },
            "the cancellation grace, 1,000 milliseconds; use a shorter grace, or raise tasks.grace_ms (temper help configuration)",
            r#""bound":{"value":1000,"key":"tasks.grace_ms","unit":"milliseconds"}"#,
        ),
        (
            c::Limit::LiveTasks { project: 3, limit: 7 },
            "project 3's live tasks, 7 items; end or wait for tasks, or raise tasks.project_tasks (temper help configuration)",
            r#""live_tasks":{"project":3,"limit":7}"#,
        ),
        (
            c::Limit::Inbox { task: 41, limit: 8 },
            "task 41's inbox, 8 items; read the inbox, or raise tasks.inbox_messages (temper help configuration)",
            r#""inbox":{"task":41,"limit":8}"#,
        ),
        (c::Limit::Words { limit: 9 }, "the words, 9 bytes; shorten them", r#""words":{"limit":9}"#),
        (c::Limit::Fields, "the client's fields, 16 items; request fewer fields", r#""which":"fields""#),
        (c::Limit::Records, "the client's records, 16 items; read fewer records at a time", r#""which":"records""#),
        (
            c::Limit::Requests,
            "the client's pending requests, 16 items; wait for a request to finish",
            r#""which":"requests""#,
        ),
        (c::Limit::Reads, "the client's pending reads, 16 items; wait for a read to finish", r#""which":"reads""#),
        (c::Limit::Watches, "the client's watches, 16 items; close a watch", r#""which":"watches""#),
        (c::Limit::Objects, "the client's working objects, 17 items; close an unused object", r#""which":"objects""#),
    ] {
        refusal(which, expected, json);
    }
}
#[test]
fn engine_words_are_checked_in_bytes_before_sending_and_json_keeps_the_refusal() {
    for (text, past) in [("hello", false), ("helloo", true), ("ééé", true)] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let mut me = f::me(f::PARTY);
            me.words_bytes = 5;
            root.deliver(c::Delivery::Me(Box::new(me)));
            let args: &[&str] = if json {
                &["task", "message", "41", text, "--key", "local-limit", "--json"]
            } else {
                &["task", "message", "41", text, "--key", "local-limit"]
            };
            let call = root.submit(args);
            root.serve(call, f::task_result(f::offered(&[c::ActKind::Message])));
            assert_eq!(root.sends.contains_key(&call), !past);
            if past {
                let answer = root.answer(call).expect("local refusal");
                assert_eq!(answer.status, 2);
                let expected: &[u8] = if json {
                    br#""bound":{"value":5,"key":"people.words","unit":"bytes"}"#
                } else {
                    b"task message: past the engine's request size, 5 bytes; shorten it, or raise people.words (temper help configuration)\n"
                };
                assert!(
                    skein_lib::bytes::find(&answer.stderr, expected).is_some(),
                    "{}",
                    String::from_utf8_lossy(&answer.stderr)
                );
            }
        }
    }
}

#[test]
fn the_configured_dimension_names_each_exact_limit_and_its_key_in_text_and_json() {
    for (key, name, next, spelling) in [
        (c::LimitKey::AuthorityGrants, "the policy grants", "use fewer grants", "authority.grants"),
        (
            c::LimitKey::AuthoritySegments,
            "the policy grant path segments",
            "shorten the grant paths",
            "authority.segments",
        ),
        (
            c::LimitKey::AuthoritySegmentBytes,
            "the policy grant segment size",
            "shorten the grant segments",
            "authority.segment_bytes",
        ),
        (c::LimitKey::AuthorityProjects, "the deployment's projects", "use an existing project", "authority.projects"),
        (
            c::LimitKey::PeoplePending,
            "the pending person-task operations",
            "wait for a person-task operation to finish",
            "people.pending",
        ),
        (c::LimitKey::PeopleWords, "the engine's request size", "shorten it", "people.words"),
        (
            c::LimitKey::PeopleAmendmentBytes,
            "the engine's amendment size",
            "reduce the amendment",
            "people.amendment_bytes",
        ),
        (c::LimitKey::PeopleHoldings, "the project's people", "reduce the roster", "people.holdings"),
        (c::LimitKey::PeopleIdentityBytes, "the identity size", "use a shorter identity", "people.identity_bytes"),
        (c::LimitKey::PeopleBatch, "the requested task batch", "split the batch", "people.batch"),
        (c::LimitKey::PeopleOperations, "the operations per request", "split the request", "people.operations"),
        (c::LimitKey::PeopleProjects, "the project rosters", "use an existing project", "people.projects"),
        (c::LimitKey::TasksTasks, "the engine's live tasks", "end or wait for tasks", "tasks.tasks"),
        (c::LimitKey::TasksProjectTasks, "the project's live tasks", "end or wait for tasks", "tasks.project_tasks"),
        (c::LimitKey::TasksProjectGoals, "the project's live goals", "end or wait for goals", "tasks.project_goals"),
        (c::LimitKey::TasksTreeTasks, "the task tree", "use a smaller task tree", "tasks.tree_tasks"),
        (c::LimitKey::TasksDepth, "the task depth", "use fewer delegation levels", "tasks.depth"),
        (c::LimitKey::TasksDelegates, "the task's delegates", "end or wait for delegates", "tasks.delegates"),
        (c::LimitKey::TasksSubscriptions, "the task's subscriptions", "remove a subscription", "tasks.subscriptions"),
        (c::LimitKey::TasksDependencies, "the task's dependencies", "reduce the dependencies", "tasks.dependencies"),
        (c::LimitKey::TasksSpecBytes, "the specification size", "shorten the specification", "tasks.spec_bytes"),
        (c::LimitKey::TasksParameters, "the specification parameters", "use fewer parameters", "tasks.parameters"),
        (c::LimitKey::TasksContractChoices, "the result choices", "use fewer choices", "tasks.contract_choices"),
        (c::LimitKey::TasksResultBytes, "the result size", "shorten the result or reason", "tasks.result_bytes"),
        (c::LimitKey::TasksInputs, "the task's inputs", "reduce the inputs", "tasks.inputs"),
        (c::LimitKey::TasksHoldings, "the resource holds", "release a hold", "tasks.holdings"),
        (
            c::LimitKey::TasksHoldWaiters,
            "the resource waiters",
            "wait for a resource waiter to finish",
            "tasks.hold_waiters",
        ),
        (c::LimitKey::TasksInboxMessages, "the task's inbox messages", "read the inbox", "tasks.inbox_messages"),
        (c::LimitKey::TasksInboxBytes, "the task's inbox size", "read the inbox", "tasks.inbox_bytes"),
        (c::LimitKey::TasksMessageBytes, "the task's message size", "shorten the message", "tasks.message_bytes"),
        (c::LimitKey::TasksGrace, "the cancellation grace", "use a shorter grace", "tasks.grace_ms"),
        (c::LimitKey::TasksPlacedHolds, "the placed holds", "lift a placed hold", "tasks.placed_holds"),
        (
            c::LimitKey::TasksProjectPlacedHolds,
            "the project's placed holds",
            "lift a placed hold",
            "tasks.project_placed_holds",
        ),
        (
            c::LimitKey::TasksPartyPlacedHolds,
            "the party's placed holds",
            "lift a placed hold",
            "tasks.party_placed_holds",
        ),
        (c::LimitKey::TasksExemptions, "the hold exemptions", "remove an exemption", "tasks.exemptions"),
        (c::LimitKey::TasksAuthorityGrants, "the authority grants", "use fewer grants", "tasks.authority_grants"),
        (c::LimitKey::TasksAuthorityBytes, "the authority size", "reduce the authority", "tasks.authority_bytes"),
        (
            c::LimitKey::TasksAuthoritySegments,
            "the authority path segments",
            "shorten the grant paths",
            "tasks.authority_segments",
        ),
        (c::LimitKey::CoreTemplates, "the project templates", "use fewer templates", "core.templates"),
        (c::LimitKey::CorePolicyBytes, "the policy size", "reduce the policy", "core.policy_bytes"),
        (
            c::LimitKey::CoreSettingsProposals,
            "the pending settings proposals",
            "decide or withdraw a settings proposal",
            "core.settings.proposals",
        ),
        (c::LimitKey::NotesScopes, "the note scopes", "use fewer note scopes", "notes.scopes"),
        (c::LimitKey::NotesEntriesPerScope, "the notes in a scope", "remove a note", "notes.entries_per_scope"),
        (c::LimitKey::NotesPatternBytes, "the note resource pattern", "shorten the pattern", "notes.pattern_bytes"),
        (
            c::LimitKey::NotesDescriptionBytes,
            "the note description size",
            "shorten the description",
            "notes.description_bytes",
        ),
        (c::LimitKey::NotesBodyBytes, "the note body size", "shorten the body", "notes.body_bytes"),
        (c::LimitKey::NotesReferences, "the note references", "use fewer references", "notes.references"),
        (c::LimitKey::AccountsAccounts, "the deployment's accounts", "remove an unused account", "accounts.accounts"),
        (
            c::LimitKey::AccountsHolderAccounts,
            "the holder's accounts",
            "remove an unused account",
            "accounts.holder_accounts",
        ),
        (c::LimitKey::AccountsSources, "the account sources", "remove an unused source", "accounts.sources"),
        (c::LimitKey::AccountsSignIns, "the sign-ins in progress", "finish or cancel a sign-in", "accounts.sign_ins"),
        (
            c::LimitKey::AccountsProgramHosts,
            "the program host operations",
            "wait for a host operation to finish",
            "accounts.program_hosts",
        ),
        (c::LimitKey::TasksBatch, "the delegated task batch", "split the batch", "tasks.batch"),
        (
            c::LimitKey::CoreEscalationReasonBytes,
            "the escalation reason size",
            "shorten the reason",
            "core.escalation_reason_bytes",
        ),
        (
            c::LimitKey::AccountsProgramNames,
            "the issued program names",
            "reuse an issued program name",
            "accounts.program_names",
        ),
        (
            c::LimitKey::AccountsCredentials,
            "the pending credential operations",
            "wait for a credential operation to finish",
            "accounts.credentials",
        ),
        (c::LimitKey::TasksReferences, "the task references", "remove an unused reference", "tasks.references"),
        (
            c::LimitKey::TasksHoldSegments,
            "the resource path segments",
            "shorten the resource path",
            "tasks.hold_segments",
        ),
        (c::LimitKey::TasksHoldBytes, "the resource path size", "shorten the resource path", "tasks.hold_bytes"),
        (
            c::LimitKey::TasksExecutorKinds,
            "the permitted executor kinds",
            "use fewer executor kinds",
            "tasks.executor_kinds",
        ),
        (
            c::LimitKey::TasksResourceReports,
            "the pending resource reports",
            "wait for a resource report to finish",
            "tasks.resource_reports",
        ),
        (c::LimitKey::TasksFunders, "the task funders", "end or wait for funded tasks", "tasks.funders"),
    ] {
        let expected = format!("{name}, 17 items; {next}, or raise {spelling} (temper help configuration)");
        let json = format!("\"bound\":{{\"value\":17,\"key\":\"{spelling}\",\"unit\":\"items\"}}");
        refusal(
            c::Limit::Admission {
                which: c::AdmissionLimit::Spec,
                bound: Some(c::LimitBound::configured(17, key, c::LimitUnit::Items)),
            },
            &expected,
            &json,
        );
    }
}
