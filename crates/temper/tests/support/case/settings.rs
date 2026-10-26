//! Adapt the shared case fixture's edits to one owner settings transaction.
use super::{Ended, Value, ctl, json, read, text, write};
use std::path::Path;

/// The legacy fixture edits a value in memory; none of these members reaches startup.
pub(super) fn base(value: &mut Value, file: &Path) {
    let channel: Value =
        serde_json::from_slice(&jig_channel_world::fixtures::engine_document()).expect("case charter fixture");
    value["engine"]["charters"] = channel["engine"]["charters"].clone();
    value["engine"]["charters"][0]["waiting_ms"] = 1.into();
    value["application"]["charter"] = 1.into();
    let cluster = read(file);
    let mut template = cluster["items"]
        .as_array()
        .expect("cluster settings items")
        .iter()
        .find(|item| item["setting"] == json!({"kind":"template","number":1}))
        .expect("cluster chat template")["set"]
        .clone();
    template["name"] = template["name"]["bytes"].clone();
    template["budget"] = json!({"as_allowed":template["budget"]["as_allowed"]["cap"]});
    template["grace_ms"] = (template["grace"].as_u64().expect("chat grace") / 1_000_000).into();
    let flags = template.as_object_mut().expect("template").remove("flags").expect("flags");
    for (name, flag) in flags.as_object().expect("template flags") {
        template[name] = flag.clone();
    }
    template.as_object_mut().expect("template").remove("grace");
    template["ended_by"] = "result".into();
    value["engine"]["templates"] = json!([template]);
    // Landing's fixture supplies its ceiling; these other values are the stock's.
    value["engine"]["rules"] = json!({"ceiling":null,"period_spend":100,
        "minimum_run_spend":1,"maximum_run_spend":10,"implies":[],"requirements":[]});
}

/// Retain only the study's settings; start on the stock with no over-stock seed.
pub(super) fn take(value: &mut Value) -> Value {
    let engine = &mut value["engine"];
    let study = json!({
        "charters":engine["charters"], "template":engine["templates"][0],
        "rules":engine["rules"], "project":engine["projects"][0]
    });
    super::super::configuration::use_stock(value);
    value["engine"]["projects"] = json!([]);
    // Even the one-charter story starts with the stock's two charters.
    let limit = &mut value["engine"]["limits"]["overrides"]["tasks.charters"];
    *limit = limit.as_u64().expect("charter capacity").max(2).into();
    // This fixture has one project. Deployment landing rules cover it when MakeProject
    // creates it; a project-specific startup override requires an existing seed.
    let projects = value["application"]["landing"]["projects"].take();
    for project in projects.as_array().expect("landing projects") {
        assert_eq!(project["project"], 1);
        value["application"]["landing"]["deployment"]
            .as_array_mut()
            .expect("deployment landing")
            .extend(project["rules"].as_array().expect("landing rules").iter().cloned());
    }
    value["application"]["landing"]["projects"] = json!([]);
    study
}

fn grant(source: &Value) -> Value {
    let pattern = &source["pattern"];
    let segments: Vec<_> = pattern["segments"].as_array().expect("grant segments").iter().map(hex).collect();
    let last = pattern["last"].as_str().expect("pattern ending");
    json!({"connector":source["connector"],"kind":source["kind"],
        "pattern":{"segments":segments,"last":{last:hex(&pattern["value"])}}})
}
fn hex(value: &Value) -> String {
    let mut out = String::new();
    for byte in value.as_str().expect("fixture bytes").bytes() {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").expect("hex");
    }
    out
}
fn grants(source: &Value) -> Value {
    Value::Array(source.as_array().expect("fixture grants").iter().map(grant).collect())
}
fn authority(source: &Value) -> Value {
    assert_eq!(source["note_resources"], json!([]), "these cases use project notes");
    let kinds: Vec<_> = source["delegation"]["kinds"]
        .as_array()
        .expect("delegation kinds")
        .iter()
        .map(|kind| json!({kind["kind"].as_str().expect("executor kind"):kind["number"]}))
        .collect();
    let deadline = source["budget"]["deadline_unix_seconds"]
        .as_u64()
        .map(|seconds| seconds.checked_mul(1_000_000_000).expect("fixture deadline"));
    json!({"models":"all","tools":source["tools"],"grants":grants(&source["grants"]),
        "delegation":{"kinds":kinds,"tasks":source["delegation"]["tasks"],"depth":source["delegation"]["depth"]},
        "spend":source["budget"]["spend"],"deadline":deadline,"notes":source["notes"],"note_resources":[]})
}
fn charter(source: &Value, stock: &Value) -> Value {
    let mut value = stock.clone();
    for name in ["number", "hosts", "resume", "turns", "reserve_turns", "spend", "inspect", "modify", "shell", "agents"]
    {
        if let Some(field) = source.get(name) {
            value[name] = field.clone();
        }
    }
    for name in ["waiting", "time", "reserve_time", "call_timeout"] {
        if let Some(millis) = source.get(format!("{name}_ms")) {
            value[name] = millis.as_u64().expect("fixture duration").checked_mul(1_000_000).expect("duration").into();
        }
    }
    let files = jig_channel_world::fixtures::files(false);
    let instructions = files
        .entries
        .into_boxed()
        .into_vec()
        .into_iter()
        .find(|(name, _)| name.as_ref() == source["instructions"].as_str().expect("fixture instructions").as_bytes())
        .expect("channel instructions")
        .1;
    value["instructions"] = text(instructions.expose()).into();
    let mut models = vec![stock["model"].clone()];
    models.extend(stock["alternatives"].as_array().expect("configured alternatives").iter().cloned());
    value["model"] = models.iter().find(|model| model["name"] == source["model"]).expect("fixture model").clone();
    let alternatives: Vec<_> = source["alternatives"]
        .as_array()
        .expect("fixture alternatives")
        .iter()
        .map(|name| models.iter().find(|model| model["name"] == *name).expect("fixture alternative").clone())
        .collect();
    value["alternatives"] = alternatives.into();
    value
}

/// Fold the cluster's cap and the cases' overrides into one change against revision 1.
pub(super) fn change(study: &Value, reads: &[Ended], file: &Path) -> ctl::Call {
    let [chat, template, rules, policy] = reads else { panic!("four whole stock setting reads") };
    let chat = ctl::json(chat)["value"].clone();
    let mut template = ctl::json(template)["value"].clone();
    let source = &study["template"];
    for name in ["executor", "contract", "shallower", "label", "llm"] {
        template[name] = source[name].clone();
    }
    template["budget"] = json!({"as_allowed":{"cap":source["budget"]["as_allowed"]}});
    for name in ["conversation", "ended_by", "listed", "retention"] {
        template["flags"][name] = source[name].clone();
    }
    let mut rules = ctl::json(rules)["value"].clone();
    rules["ceiling"] = authority(&study["rules"]["ceiling"]);
    // Preserve the deployment landing requirements that startup attached to the rules.
    assert_eq!(study["rules"]["implies"], json!([]));
    let mut policy = ctl::json(policy)["value"].clone();
    let project = &study["project"];
    let source = &project["policy"];
    for name in ["escalation_role", "period_spend", "requirements", "templates"] {
        policy[name] = source[name].clone();
    }
    policy["ceiling"] = authority(&source["ceiling"]);
    policy["projections"] = grants(&source["projections"]);
    policy["permissions"] = project["roles"].clone();
    let configured_roles: Vec<_> = source["roles"]
        .as_array()
        .expect("project roles")
        .iter()
        .map(|source| {
            let mut role = source.clone();
            role["authority"] = authority(&source["authority"]);
            role
        })
        .collect();
    policy["roles"] = configured_roles.into();
    let mut items: Vec<_> = study["charters"]
        .as_array()
        .expect("case charters")
        .iter()
        .map(|source| json!({"setting":{"kind":"charter","number":source["number"]},"set":charter(source,&chat)}))
        .collect();
    items.extend([
        json!({"setting":{"kind":"template","number":1},"set":template}),
        json!({"setting":{"kind":"rules"},"set":rules}),
        json!({"setting":{"kind":"policy","number":1},"set":policy}),
    ]);
    write(file, &json!({"items":items}));
    ctl::Call::new(&[
        "change-settings",
        file.to_str().expect("case settings FILE"),
        "--against",
        "1",
        "--key",
        "case-settings",
    ])
}
