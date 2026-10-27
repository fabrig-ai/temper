//! Whole values for the process tier's one settings transaction.
use jig_domain_authority as authority;
use jig_domain_core::{Charter, ConfiguredPolicy, Template, settings::Defaults};
use serde_json::{Value, json};
fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("stock text")
}
fn model(value: &jig_domain_core::Model) -> Value {
    let mut efforts = Vec::new();
    for effort in &value.efforts {
        efforts.push(text(effort));
    }
    json!({"dialect":value.dialect,"endpoint":value.endpoint,"program":value.program.as_deref().map(text),
        "name":text(&value.name),"window":value.window,"output":value.output,"input_price":value.input_price,
        "cached_price":value.cached_price,"cache_write_price":value.cache_write_price,"output_price":value.output_price,
        "price_unit":value.price_unit,"efforts":efforts,"effort":value.effort.as_deref().map(text)})
}

/// Render these typed case defaults for the process owner's transaction.
#[must_use]
pub fn items(defaults: &Defaults) -> Value {
    assert_eq!(defaults.rules.implies.len(), 0, "case rules have no implications");
    let mut items = Vec::new();
    for value in &defaults.charters {
        items.push(crate::settings::item("charter", value.number, charter(value)));
    }
    items.push(crate::settings::item("template", 1, template(&defaults.templates[0])));
    items.push(crate::settings::item("policy", 1, policy(defaults.policies.get(&1).expect("case policy 1"))));
    assert!(defaults.rules.requirements.is_empty(), "requirements are joined from the process startup's landing rules");
    items.push(json!({"setting":{"kind":"rules"},"set":{
        "ceiling": ceiling(&defaults.rules.ceiling),
        "period_spend":defaults.rules.period_spend,
        "minimum_run_spend":defaults.rules.minimum_run_spend,
        "maximum_run_spend":defaults.rules.maximum_run_spend,
        "implies":[],"requirements":[]
    }}));
    items.into()
}
fn charter(c: &Charter) -> Value {
    let p = &c.policy;
    json!({"number":c.number,"instructions":text(&p.instructions),"model":model(&p.model),"alternatives":p.alternatives.iter().map(model).collect::<Vec<_>>(),
        "waiting":p.waiting.as_nanos(),"resume":p.resume,"turns":p.turns,"time":p.time.as_nanos(),"reserve_turns":p.reserve_turns,"reserve_time":p.reserve_time.as_nanos(),
        "inspect":p.inspect,"modify":p.modify,"shell":p.shell,"agents":p.agents,"tools":p.tools.0,"call_timeout":p.call_timeout.as_nanos(),"hosts":match c.hosts { jig_domain_fleet::Kinds::Workers=>"workers",jig_domain_fleet::Kinds::Engine=>"engine",jig_domain_fleet::Kinds::Both=>"both" },"spend":c.spend})
}
fn template(t: &Template) -> Value {
    let jig_domain_people::TemplateExecutor::Charter(executor) = t.executor else { panic!("stock chat executor") };
    let jig_domain_tasks::Contract::Report { words } = t.contract else { panic!("stock chat report") };
    let jig_domain_people::TemplateBudget::AsAllowed { cap } = t.budget else { panic!("stock chat budget") };
    assert_eq!(t.deadline, jig_domain_people::TemplateDeadline::None);
    assert!(t.llm.is_none());
    json!({"number":t.number,"name":{"bytes":text(&t.name),"cut":false},"executor":{"charter":executor},"contract":{"report":{"words":words}},"shallower":t.shallower,
        "budget":{"as_allowed":{"cap":cap}},"deadline":"none","tracked":t.tracked,"label":t.label,"grace":t.grace.map(skein_lib::Duration::as_nanos),"llm":null,
        "flags":{"conversation":t.flags.conversation,"listed":t.flags.listed,"retention":match t.flags.retention { jig_domain_tasks::Retention::Kept=>"kept",jig_domain_tasks::Retention::ForgottenOnceRead=>"forgotten_once_read" },
        "ended_by":match t.flags.ended_by { jig_domain_tasks::EndedBy::Requester=>"requester",jig_domain_tasks::EndedBy::Result=>"result" }}})
}
fn policy(p: &ConfiguredPolicy) -> Value {
    assert!(p.policy.requirements.is_empty());
    assert!(p.templates.is_empty());
    let mut roles = Vec::new();
    for role in &p.policy.roles {
        roles.push(json!({"number":role.number,"authority":ceiling(&role.authority),
            "period_spend":role.period_spend,"requests":role.requests.0,"decides":role.decides.0}));
    }
    let mut permissions = Vec::new();
    for permission in &p.permissions {
        permissions
            .push(json!({"connector":permission.connector,"permission":permission.permission,"role":permission.role}));
    }
    json!({"escalation_role":p.policy.escalation_role,"ceiling":ceiling(&p.policy.ceiling),
        "projections":grants(&p.policy.projections),"period_spend":p.policy.period_spend,
        "roles":roles,"permissions":permissions,"requirements":[],"templates":[]})
}
fn ceiling(value: &authority::Authority) -> Value {
    assert_eq!(value.models, authority::Models::ALL);
    assert!(value.note_resources.is_empty());
    let mut kinds = Vec::new();
    for kind in &value.delegation.kinds {
        kinds.push(match kind {
            authority::Executor::Charter(n) => json!({"charter":n}),
            authority::Executor::Procedure(n) => json!({"procedure":n}),
            authority::Executor::Role(n) => json!({"role":n}),
        });
    }
    json!({"models":"all","tools":value.tools.0,"grants":grants(&value.grants),
        "delegation":{"kinds":kinds,"tasks":value.delegation.tasks,"depth":value.delegation.depth},
        "spend":value.budget.spend,"deadline":value.budget.deadline.map(skein_lib::Wall::as_nanos),
        "notes":value.notes.0,"note_resources":[]})
}
fn grants(values: &[authority::Grant]) -> Value {
    let mut grants = Vec::new();
    for grant in values {
        let mut segments = Vec::new();
        for segment in &grant.pattern.segments {
            segments.push(hex(segment));
        }
        let last = match &grant.pattern.last {
            authority::Last::Exact(value) => json!({"exact":hex(value)}),
            authority::Last::Open(value) => json!({"open":hex(value)}),
        };
        grants.push(json!({"connector":grant.connector,"kind":grant.kind,"pattern":{"segments":segments,"last":last}}));
    }
    grants.into()
}
fn hex(value: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(value.len() * 2);
    for byte in value {
        write!(&mut text, "{byte:02x}").expect("hex bytes");
    }
    text
}
