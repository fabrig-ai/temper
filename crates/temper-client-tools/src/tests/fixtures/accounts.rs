//! Account/source/provider identities differ; none is a secret.
use super::*;
pub(crate) const ACCOUNT: u32 = 43;
pub(crate) const SOURCE: u32 = 71;
pub(crate) const PROVIDER: u32 = 9;
pub(crate) const ACCOUNT_WATCH: client::Watchable = client::Watchable::Jig(c::Subject::Accounts);
pub(crate) const DEPLOYMENT_WATCH: client::Watchable = client::Watchable::Jig(c::Subject::Deployment);
pub(crate) fn accounts() -> c::OwnerAccounts {
    let mut sources = List::with_capacity(1);
    sources
        .push(c::OwnerAccountSource {
            name: None,
            place: 1,
            number: SOURCE,
            kind: c::OwnerAccountKind::Key,
            state: c::OwnerAccountState::Later { until: Some(Wall::from_nanos(100)) },
            usable: false,
            expiring: true,
            attention: true,
            spent_until: Some(Wall::from_nanos(100)),
        })
        .expect("one source");
    let mut uses = List::with_capacity(1);
    uses.push(c::OwnerAccountUse::Project(PROJECT)).expect("one payer");
    let mut accounts = List::with_capacity(1);
    accounts
        .push(c::OwnerAccountLine {
            number: ACCOUNT,
            place: 1,
            uses: Some(uses),
            plan: false,
            provider: Some(PROVIDER),
            provider_name: Some(document_text(b"Scripted")),
            manage_usage: Some(Box::from(b"https://provider.example/usage".as_slice())),
            display: document_text(b"Work"),
            holder: c::OwnerAccountHolder::Party(PARTY),
            sources,
            cut: false,
        })
        .expect("one account");
    c::OwnerAccounts {
        program_hosts: List::with_capacity(0),
        readiness: List::with_capacity(0),
        readiness_cut: false,
        accounts,
        signing: List::with_capacity(0),
        last_end: None,
        usable: false,
        settled: true,
        cut: false,
    }
}
pub(crate) fn account_root(owner: bool) -> Harness {
    let mut root = signed_in(PARTY);
    let mut me = me(PARTY);
    if owner {
        me.deployment_role = Some(c::HeldRole::Owner);
    }
    root.deliver(c::Delivery::Me(Box::new(me)));
    root
}
pub(crate) fn deployment() -> c::OwnerDeployment {
    let limits = crate::limits::client().client;
    let limits = jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).expect("tools limits");
    let mut deployment = jig_client_protocol_link::translate::documents::owner_deployment(
        jig_client_test_records::owner_deployment_record(),
        &limits,
    )
    .expect("deployment fixture");
    deployment.cut = false;
    let row = accounts().accounts.get(0).expect("account row").clone();
    deployment.accounts = List::with_capacity(1);
    deployment
        .accounts
        .push(c::OwnerAccount {
            number: row.number,
            place: row.place,
            uses: row.uses,
            plan: row.plan,
            provider: row.provider,
            provider_name: row.provider_name,
            manage_usage: row.manage_usage,
            display: row.display,
            holder: c::OwnerAccountHolder::Deployment,
            sources: row.sources,
            generation: Some(1),
            state: c::OwnerAccountState::Granted,
            usable: true,
            expiring: false,
            attention: false,
            spent_until: None,
            runs: List::with_capacity(0),
            waiting: List::with_capacity(0),
            cut: false,
        })
        .expect("one deployment account");
    deployment
}
pub(crate) fn deliver_accounts(root: &mut Harness, value: c::OwnerAccounts) {
    root.deliver_subject(&ACCOUNT_WATCH, c::Delivery::OwnerAccounts(Box::new(value)));
}
pub(crate) fn deliver_deployment(root: &mut Harness, value: c::OwnerDeployment) {
    root.deliver_subject(&DEPLOYMENT_WATCH, c::Delivery::OwnerDeployment(Box::new(value)));
}
pub(crate) fn account_read(
    root: &mut Harness,
    value: c::OwnerAccounts,
    deployment: Option<c::OwnerDeployment>,
) -> crate::jig::Printed {
    let call = root.submit(&["account", "list", "--json"]);
    deliver_accounts(root, value);
    if let Some(deployment) = deployment {
        deliver_deployment(root, deployment);
    }
    root.answer(call).expect("first deliveries complete read")
}

pub(crate) fn provider_offers() -> List<c::ProviderOffer> {
    let mut programs = List::with_capacity(1);
    programs.push(c::Text::new(b"helper".as_slice().into(), 32).expect("program")).expect("program room");
    let mut offers = List::with_capacity(1);
    offers
        .push(c::ProviderOffer {
            provider: PROVIDER,
            key_bytes: 256,
            name: document_text(b"Scripted"),
            sign_in: true,
            key: true,
            programs,
            programs_cut: false,
            none: false,
            signed_in_plan: false,
            manage_usage: Some(b"https://provider.example/usage".as_slice().into()),
        })
        .expect("offer room");
    offers
}
pub(crate) fn payer_readiness() -> List<c::PayerReadiness> {
    let mut rows = List::with_capacity(1);
    rows.push(c::PayerReadiness {
        provider: PROVIDER,
        payer: c::OwnerAccountUse::Party(PARTY),
        readiness: c::Readiness::Unknown,
        settled: None,
    })
    .expect("readiness room");
    rows
}
