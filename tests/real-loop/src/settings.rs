//! Whole stock charter and template values in ctl's JSON vocabulary.
use serde_json::{Value, json};
fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("stock text")
}
fn model(value: &jig_domain_core::Model) -> Value {
    json!({"dialect":value.dialect,"endpoint":value.endpoint,"program":value.program.as_deref().map(text),
        "name":text(&value.name),"window":value.window,"output":value.output,"input_price":value.input_price,
        "cached_price":value.cached_price,"cache_write_price":value.cache_write_price,"output_price":value.output_price,
        "price_unit":value.price_unit,"efforts":value.efforts.iter().map(|effort| text(effort)).collect::<Vec<_>>(),"effort":value.effort.as_deref().map(text)})
}
/// Use the binary's actual defaults, with this deployment's models.
#[must_use]
pub fn stock(deployment: &Value) -> Value {
    let mut value = deployment["engine"].clone();
    for member in ["charters", "templates", "policies", "rules"] {
        value.as_object_mut().expect("engine").remove(member);
    }
    value["projects"] = json!([]);
    let bytes = serde_json::to_vec(&value).expect("engine JSON");
    let document = jig_protocol_config::document(
        &bytes,
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 65_536, depth: 32 },
    )
    .expect("engine document");
    let engine = jig_protocol_config::engine(
        &document,
        0,
        &jig_protocol_config::Directories::working(),
        jig_protocol_config::Defaults::Passed,
    )
    .expect("configured models");
    let defaults = temper_service::stock::defaults(&engine.llm);
    let c = &defaults.charters[0];
    let p = &c.policy;
    let charter = json!({"number":c.number,"instructions":text(&p.instructions),"model":model(&p.model),"alternatives":p.alternatives.iter().map(model).collect::<Vec<_>>(),
        "waiting":p.waiting.as_nanos(),"resume":p.resume,"turns":p.turns,"time":p.time.as_nanos(),"reserve_turns":p.reserve_turns,"reserve_time":p.reserve_time.as_nanos(),
        "inspect":p.inspect,"modify":p.modify,"shell":p.shell,"agents":p.agents,"call_timeout":p.call_timeout.as_nanos(),"hosts":match c.hosts { jig_domain_fleet::Kinds::Workers=>"workers",jig_domain_fleet::Kinds::Engine=>"engine",jig_domain_fleet::Kinds::Both=>"both" },"spend":c.spend});
    let t = &defaults.templates[0];
    let jig_domain_people::TemplateExecutor::Charter(executor) = t.executor else { panic!("stock chat executor") };
    let jig_domain_tasks::Contract::Report { words } = t.contract else { panic!("stock chat report") };
    let jig_domain_people::TemplateBudget::AsAllowed { cap } = t.budget else { panic!("stock chat budget") };
    assert_eq!(t.deadline, jig_domain_people::TemplateDeadline::None);
    assert!(t.llm.is_none());
    let template = json!({"number":t.number,"name":{"bytes":text(&t.name),"cut":false},"executor":{"charter":executor},"contract":{"report":{"words":words}},"shallower":t.shallower,
        "budget":{"as_allowed":{"cap":cap}},"deadline":"none","tracked":t.tracked,"label":t.label,"grace":t.grace.map(skein_lib::Duration::as_nanos),"llm":null,
        "flags":{"conversation":t.flags.conversation,"listed":t.flags.listed,"retention":match t.flags.retention { jig_domain_tasks::Retention::Kept=>"kept",jig_domain_tasks::Retention::ForgottenOnceRead=>"forgotten_once_read" },
        "ended_by":match t.flags.ended_by { jig_domain_tasks::EndedBy::Requester=>"requester",jig_domain_tasks::EndedBy::Result=>"result" }}});
    json!({"charter":charter,"template":template})
}
#[must_use]
pub fn item(kind: &str, number: u32, value: Value) -> Value {
    let mut item = json!({"setting":{"kind":kind,"number":number}});
    item["set"] = value;
    item
}

/// Parse only startup configuration; the caller supplies typed defaults afterwards.
#[must_use]
pub fn configuration(deployment: &Value) -> jig_protocol_config::Engine {
    let mut value = deployment["engine"].clone();
    for member in ["charters", "templates", "policies", "rules"] {
        value.as_object_mut().expect("engine").remove(member);
    }
    let bytes = serde_json::to_vec(&value).expect("engine JSON");
    let document = jig_protocol_config::document(
        &bytes,
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 65_536, depth: 32 },
    )
    .expect("engine document");
    jig_protocol_config::engine(
        &document,
        0,
        &jig_protocol_config::Directories::working(),
        jig_protocol_config::Defaults::Passed,
    )
    .expect("startup configuration")
}
