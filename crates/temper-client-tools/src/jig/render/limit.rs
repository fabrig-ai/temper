//! Limit names and remedies are ctl's words over typed facts; client/tools.md, 6.
use super::Text;
use crate::jig::Limits;
use jig_client::contract as c;

pub(super) fn limit(which: &c::Limit, limits: &Limits, out: &mut Text) {
    out.put(b"past ");
    match which {
        c::Limit::Admission { which, bound } => {
            let (name, next) = match bound {
                Some(c::LimitBound { key: Some(key), .. }) => configured(*key),
                Some(c::LimitBound { key: None, .. }) => fixed(*which),
                None => admission(*which),
            };
            out.put(name);
            if let Some(bound) = bound {
                amount(*bound, out);
            }
            out.put(b"; ");
            out.put(next);
            if let Some(c::LimitBound { key: Some(key), .. }) = bound {
                out.put(b", or raise ");
                out.put(key.name());
                out.put(b" (temper help configuration)");
            }
        }
        c::Limit::LiveTasks { project, limit } => {
            out.put(b"project ");
            out.number(u64::from(*project));
            out.put(b"'s live tasks");
            amount(c::LimitBound::fixed(u64::from(*limit), c::LimitUnit::Items), out);
            out.put(b"; end or wait for tasks, or raise tasks.project_tasks (temper help configuration)");
        }
        c::Limit::Inbox { task, limit } => {
            out.put(b"task ");
            out.number(*task);
            out.put(b"'s inbox");
            amount(c::LimitBound::fixed(u64::from(*limit), c::LimitUnit::Items), out);
            out.put(b"; read the inbox, or raise tasks.inbox_messages (temper help configuration)");
        }
        c::Limit::Words { limit } => {
            out.put(b"the words");
            amount(c::LimitBound::fixed(u64::from(*limit), c::LimitUnit::Bytes), out);
            out.put(b"; shorten them");
        }
        c::Limit::Fields => {
            out.put(b"the client's fields");
            amount(c::LimitBound::fixed(u64::from(limits.client.fields), c::LimitUnit::Items), out);
            out.put(b"; request fewer fields");
        }
        c::Limit::Records => {
            out.put(b"the client's records");
            amount(c::LimitBound::fixed(u64::from(limits.client.records), c::LimitUnit::Items), out);
            out.put(b"; read fewer records at a time");
        }
        c::Limit::Requests => {
            out.put(b"the client's pending requests");
            amount(c::LimitBound::fixed(u64::from(limits.client.requests.requests), c::LimitUnit::Items), out);
            out.put(b"; wait for a request to finish");
        }
        c::Limit::Reads => {
            out.put(b"the client's pending reads");
            amount(c::LimitBound::fixed(u64::from(limits.client.reads), c::LimitUnit::Items), out);
            out.put(b"; wait for a read to finish");
        }
        c::Limit::Watches => {
            out.put(b"the client's watches");
            amount(c::LimitBound::fixed(u64::from(limits.client.watches), c::LimitUnit::Items), out);
            out.put(b"; close a watch");
        }
        c::Limit::Objects => {
            out.put(b"the client's working objects");
            amount(c::LimitBound::fixed(u64::from(limits.client.handles), c::LimitUnit::Items), out);
            out.put(b"; close an unused object");
        }
    }
}
fn amount(bound: c::LimitBound, out: &mut Text) {
    out.put(b", ");
    grouped(bound.value, out);
    out.put(match bound.unit {
        c::LimitUnit::Bytes => b" bytes",
        c::LimitUnit::Items => b" items",
        c::LimitUnit::Milliseconds => b" milliseconds",
    });
}
// At most seven groups cover u64; this fixed stack buffer owns no retained table.
fn grouped(mut number: u64, out: &mut Text) {
    let mut bytes = [0_u8; 26];
    let mut start = bytes.len();
    for digit in 0..20_u8 {
        if digit > 0 && digit.is_multiple_of(3) {
            start = start.checked_sub(1).expect("separator slot");
            *bytes.get_mut(start).expect("separator slot") = b',';
        }
        start = start.checked_sub(1).expect("digit slot");
        *bytes.get_mut(start).expect("digit slot") =
            b'0'.checked_add(u8::try_from(number % 10).expect("digit fits")).expect("ASCII digit");
        number /= 10;
        if number == 0 {
            break;
        }
    }
    out.put(bytes.get(start..).expect("decimal range"));
}
fn fixed(which: c::AdmissionLimit) -> (&'static [u8], &'static [u8]) {
    if which == c::AdmissionLimit::Contract { (b"the result size", b"shorten the result") } else { admission(which) }
}
fn admission(which: c::AdmissionLimit) -> (&'static [u8], &'static [u8]) {
    match which {
        c::AdmissionLimit::Words => (b"the engine's request size", b"shorten it"),
        c::AdmissionLimit::Amendment => (b"the engine's amendment size", b"reduce the amendment"),
        c::AdmissionLimit::Key => (b"the account key size", b"use a shorter key"),
        c::AdmissionLimit::Name => (b"the name size", b"use a shorter name"),
        c::AdmissionLimit::Projects => (b"the deployment's projects", b"use an existing project"),
        c::AdmissionLimit::SettingsProposals => {
            (b"the pending settings proposals", b"decide or withdraw a settings proposal")
        }
        c::AdmissionLimit::Goals => (b"the project's live goals", b"end or wait for goals"),
        c::AdmissionLimit::Operations => (b"the operations per request", b"split the request"),
        c::AdmissionLimit::Grace => (b"the cancellation grace", b"use a shorter grace"),
        c::AdmissionLimit::Identity => (b"the identity rule", b"use a valid identity"),
        c::AdmissionLimit::Roles => (b"the project roles", b"choose valid roles and name each party once"),
        c::AdmissionLimit::Arithmetic => (b"the engine's counter", b"report the exhausted counter to the engine owner"),
        c::AdmissionLimit::Duplicate => (b"duplicate input", b"remove the duplicate"),
        c::AdmissionLimit::Empty => (b"empty input", b"provide at least one item"),
        c::AdmissionLimit::Batch => (b"the task batch", b"split the batch"),
        c::AdmissionLimit::ProjectLiveTasks => (b"the project's live tasks", b"end or wait for tasks"),
        c::AdmissionLimit::LiveTasks => (b"the engine's live tasks", b"end or wait for tasks"),
        c::AdmissionLimit::Tree => (b"the task tree", b"use a smaller task tree"),
        c::AdmissionLimit::Depth => (b"the task depth", b"use fewer delegation levels"),
        c::AdmissionLimit::Delegates => (b"the task's delegates", b"end or wait for delegates"),
        c::AdmissionLimit::Subscription => (b"the task's subscriptions", b"remove a subscription"),
        c::AdmissionLimit::Dependencies => (b"the task's dependencies", b"reduce the dependencies"),
        c::AdmissionLimit::Cycle => (b"a dependency cycle", b"remove the dependency cycle"),
        c::AdmissionLimit::Spec => (b"the task's specification", b"reduce the specification"),
        c::AdmissionLimit::Contract => (b"the result contract", b"reduce the result contract"),
        c::AdmissionLimit::Authority => (b"the authority shape", b"reduce the authority"),
        c::AdmissionLimit::Inputs => (b"the task's inputs", b"reduce the inputs"),
        c::AdmissionLimit::Attempts => (b"the task's attempt", b"read the task and use its current attempt"),
        c::AdmissionLimit::LiveDelegates => (b"the live delegates", b"end or wait for delegates"),
        c::AdmissionLimit::Restore => {
            (b"the engine's restore state", b"restore records before marking the engine ready")
        }
        c::AdmissionLimit::Read => (b"the task's read position", b"read the task and use its offered read position"),
        c::AdmissionLimit::Turn => (b"the task's turn", b"use the next turn and nondecreasing expense"),
        c::AdmissionLimit::HoldKind => (b"the resource hold kind", b"use a configured resource hold kind"),
        c::AdmissionLimit::HoldTaken => (b"the resource hold", b"wait for the resource, or release its hold"),
        c::AdmissionLimit::Holds => (b"the resource holds", b"release a hold or wait for room"),
        c::AdmissionLimit::InboxFull => (b"the task's inbox", b"read the inbox"),
        c::AdmissionLimit::Notes => (b"the notes", b"remove a note or reduce its size"),
        c::AdmissionLimit::Revision => (b"the revision", b"read the current revision"),
        c::AdmissionLimit::Question => (b"the question size", b"shorten the question"),
        c::AdmissionLimit::Priority => (b"the priority batch", b"reduce the priority batch"),
        c::AdmissionLimit::Policy => (b"the policy size", b"reduce the policy"),
        c::AdmissionLimit::Resource => (b"the resource size", b"reduce the resource"),
        c::AdmissionLimit::Service => (b"the service size", b"reduce the service"),
        c::AdmissionLimit::Accounts => (b"the deployment's accounts", b"remove an unused account"),
        c::AdmissionLimit::HolderAccounts => (b"the holder's accounts", b"remove an unused account"),
        c::AdmissionLimit::Sources => (b"the account sources", b"remove an unused source"),
        c::AdmissionLimit::SignIns => (b"the sign-ins in progress", b"finish or cancel a sign-in"),
        c::AdmissionLimit::PlacedHolds => (b"the placed holds", b"lift a placed hold"),
        c::AdmissionLimit::ProjectPlacedHolds => (b"the project's placed holds", b"lift a placed hold"),
        c::AdmissionLimit::PartyPlacedHolds => (b"the party's placed holds", b"lift a placed hold"),
        c::AdmissionLimit::Exemptions => (b"the hold exemptions", b"remove an exemption"),
    }
}

fn configured(key: c::LimitKey) -> (&'static [u8], &'static [u8]) {
    match key {
        c::LimitKey::AuthorityGrants => (b"the policy grants", b"use fewer grants"),
        c::LimitKey::AuthoritySegments => (b"the policy grant path segments", b"shorten the grant paths"),
        c::LimitKey::AuthoritySegmentBytes => (b"the policy grant segment size", b"shorten the grant segments"),
        c::LimitKey::PeoplePending => {
            (b"the pending person-task operations", b"wait for a person-task operation to finish")
        }
        c::LimitKey::PeopleWords => (b"the engine's request size", b"shorten it"),
        c::LimitKey::PeopleAmendmentBytes => (b"the engine's amendment size", b"reduce the amendment"),
        c::LimitKey::PeopleHoldings => (b"the project's people", b"reduce the roster"),
        c::LimitKey::PeopleIdentityBytes => (b"the identity size", b"use a shorter identity"),
        c::LimitKey::PeopleBatch => (b"the requested task batch", b"split the batch"),
        c::LimitKey::PeopleOperations => (b"the operations per request", b"split the request"),
        c::LimitKey::PeopleProjects => (b"the project rosters", b"use an existing project"),
        c::LimitKey::AuthorityProjects => (b"the deployment's projects", b"use an existing project"),
        c::LimitKey::TasksTasks => (b"the engine's live tasks", b"end or wait for tasks"),
        c::LimitKey::TasksProjectTasks => (b"the project's live tasks", b"end or wait for tasks"),
        c::LimitKey::TasksProjectGoals => (b"the project's live goals", b"end or wait for goals"),
        c::LimitKey::TasksTreeTasks => (b"the task tree", b"use a smaller task tree"),
        c::LimitKey::TasksDepth => (b"the task depth", b"use fewer delegation levels"),
        c::LimitKey::TasksDelegates => (b"the task's delegates", b"end or wait for delegates"),
        c::LimitKey::TasksSubscriptions => (b"the task's subscriptions", b"remove a subscription"),
        c::LimitKey::TasksDependencies => (b"the task's dependencies", b"reduce the dependencies"),
        c::LimitKey::TasksSpecBytes => (b"the specification size", b"shorten the specification"),
        c::LimitKey::TasksParameters => (b"the specification parameters", b"use fewer parameters"),
        c::LimitKey::TasksContractChoices => (b"the result choices", b"use fewer choices"),
        c::LimitKey::TasksResultBytes => (b"the result size", b"shorten the result or reason"),
        c::LimitKey::TasksInputs => (b"the task's inputs", b"reduce the inputs"),
        c::LimitKey::TasksHoldings => (b"the resource holds", b"release a hold"),
        c::LimitKey::TasksHoldWaiters => (b"the resource waiters", b"wait for a resource waiter to finish"),
        c::LimitKey::TasksInboxMessages => (b"the task's inbox messages", b"read the inbox"),
        c::LimitKey::TasksInboxBytes => (b"the task's inbox size", b"read the inbox"),
        c::LimitKey::TasksMessageBytes => (b"the task's message size", b"shorten the message"),
        c::LimitKey::TasksGrace => (b"the cancellation grace", b"use a shorter grace"),
        c::LimitKey::TasksPlacedHolds => (b"the placed holds", b"lift a placed hold"),
        c::LimitKey::TasksProjectPlacedHolds => (b"the project's placed holds", b"lift a placed hold"),
        c::LimitKey::TasksPartyPlacedHolds => (b"the party's placed holds", b"lift a placed hold"),
        c::LimitKey::TasksExemptions => (b"the hold exemptions", b"remove an exemption"),
        c::LimitKey::TasksAuthorityGrants => (b"the authority grants", b"use fewer grants"),
        c::LimitKey::TasksAuthorityBytes => (b"the authority size", b"reduce the authority"),
        c::LimitKey::TasksAuthoritySegments => (b"the authority path segments", b"shorten the grant paths"),
        c::LimitKey::CoreTemplates => (b"the project templates", b"use fewer templates"),
        c::LimitKey::CorePolicyBytes => (b"the policy size", b"reduce the policy"),
        c::LimitKey::CoreSettingsProposals => {
            (b"the pending settings proposals", b"decide or withdraw a settings proposal")
        }
        c::LimitKey::NotesScopes => (b"the note scopes", b"use fewer note scopes"),
        c::LimitKey::NotesEntriesPerScope => (b"the notes in a scope", b"remove a note"),
        c::LimitKey::NotesPatternBytes => (b"the note resource pattern", b"shorten the pattern"),
        c::LimitKey::NotesDescriptionBytes => (b"the note description size", b"shorten the description"),
        c::LimitKey::NotesBodyBytes => (b"the note body size", b"shorten the body"),
        c::LimitKey::NotesReferences => (b"the note references", b"use fewer references"),
        c::LimitKey::AccountsAccounts => (b"the deployment's accounts", b"remove an unused account"),
        c::LimitKey::AccountsHolderAccounts => (b"the holder's accounts", b"remove an unused account"),
        c::LimitKey::AccountsSources => (b"the account sources", b"remove an unused source"),
        c::LimitKey::AccountsSignIns => (b"the sign-ins in progress", b"finish or cancel a sign-in"),
        c::LimitKey::AccountsProgramHosts => (b"the program host operations", b"wait for a host operation to finish"),
        c::LimitKey::TasksBatch => (b"the delegated task batch", b"split the batch"),
        c::LimitKey::CoreEscalationReasonBytes => (b"the escalation reason size", b"shorten the reason"),
        c::LimitKey::AccountsProgramNames => (b"the issued program names", b"reuse an issued program name"),
        c::LimitKey::AccountsCredentials => {
            (b"the pending credential operations", b"wait for a credential operation to finish")
        }
        c::LimitKey::TasksReferences => (b"the task references", b"remove an unused reference"),
        c::LimitKey::TasksHoldSegments => (b"the resource path segments", b"shorten the resource path"),
        c::LimitKey::TasksHoldBytes => (b"the resource path size", b"shorten the resource path"),
        c::LimitKey::TasksExecutorKinds => (b"the permitted executor kinds", b"use fewer executor kinds"),
        c::LimitKey::TasksResourceReports => (b"the pending resource reports", b"wait for a resource report to finish"),
        c::LimitKey::TasksFunders => (b"the task funders", b"end or wait for funded tasks"),
    }
}
