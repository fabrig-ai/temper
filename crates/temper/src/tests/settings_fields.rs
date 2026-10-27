use jig_domain_core::settings::Value as StockValue;
use serde_json::Value as Json;
use settings_form::{
    Limits,
    fields::{self, Choice, Field, Shape, Value, Width},
};
use skein_json::writer::{Encoder, Limits as JsonLimits};

#[test]
fn the_page_is_what_fields_renders() {
    let checked_in = include_str!("../../../../docs/help/reference/settings-fields.md");
    let rendered = fields::page(&temper_client_tools::limits::form());
    let rendered = std::str::from_utf8(&rendered).expect("printable ASCII page");
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help/reference/settings-fields.md"),
            rendered,
        )
        .expect("write fields page");
        return;
    }
    if checked_in != rendered {
        let mut line = 1;
        for (left, right) in checked_in.lines().zip(rendered.lines()) {
            if left != right {
                break;
            }
            line += 1;
        }
        panic!("settings-fields differs at line {line}; the page should be:\n{rendered}");
    }
}

pub(super) fn llm() -> jig_protocol_config::Llm {
    jig_protocol_config::Llm {
        endpoints: Box::new([]),
        models: Box::new([jig_domain_core::Model {
            dialect: 1,
            endpoint: 1,
            program: None,
            name: Box::from(b"x".as_slice()),
            window: 8192,
            output: 1024,
            input_price: 1,
            cached_price: 1,
            cache_write_price: None,
            output_price: 1,
            price_unit: 1000,
            efforts: Box::new([]),
            effort: None,
        }]),
        descriptions: Box::new([None]),
        providers: Box::new([]),
    }
}
fn models(llm: &jig_protocol_config::Llm) -> Box<[jig_protocol_engine_clients::Model]> {
    let mut models = Vec::new();
    for model in &llm.models {
        models.push(jig_protocol_engine_clients::Model {
            name: model.name.clone(),
            provider: None,
            window: model.window,
            output: model.output,
            efforts: model.efforts.clone(),
            description: None,
            prices: jig_protocol_engine_clients::Prices {
                input: model.input_price,
                cached: model.cached_price,
                cache_write: model.cache_write_price,
                output: model.output_price,
                unit: model.price_unit,
            },
        });
    }
    models.into_boxed_slice()
}
fn encode(value: &settings_form::view::SettingValue, out: &mut Encoder) {
    use settings_form::{values, view::SettingValue};
    match value {
        SettingValue::Charter(value) => values::encode_charter(value, out),
        SettingValue::Template(value) => values::encode_template(value, out),
        SettingValue::Policy(value) => values::encode_setting_policy(value, out),
        SettingValue::Rules(value) => values::encode_rules(value, out),
    }
}
fn printed(value: &StockValue, models: &[jig_protocol_engine_clients::Model]) -> Json {
    let preset = temper_client_tools::limits::client().client;
    let link = jig_client_protocol_link::Limits::new(preset.domain, preset.exchanges).expect("ctl preset");
    let wire =
        jig_protocol_engine_clients::translate::settings::value(value, models, &link.wire).expect("stock fits wire");
    let client = jig_client_protocol_link::translate::settings::restore_setting_value(&wire, &link)
        .expect("stock reaches client");
    let value = temper_client_tools::jig::form_setting_value(&client);
    let bounds = JsonLimits {
        depth: 32,
        length: u32::try_from(temper_client_tools::limits::RESULT_BYTES).expect("result bound"),
    };
    let mut measure = Encoder::measure(&bounds);
    encode(&value, &mut measure);
    let mut out = Encoder::write(measure.measured().expect("stock JSON fits"), &bounds);
    encode(&value, &mut out);
    serde_json::from_slice(&out.finish()).expect("ctl JSON")
}
fn word(value: &Json, words: &[fields::Word]) {
    let text = value.as_str().expect("word");
    let mut found = false;
    for word in words {
        if word.word == text.as_bytes() {
            found = true;
            break;
        }
    }
    assert!(found, "known word {text}");
}
fn scalar(value: &Json, field: &Field, limits: &Limits, pending: &mut Vec<(Shape, Json)>) {
    match &field.value {
        Value::Shape(shape) => pending.push((*shape, value.clone())),
        Value::Setting => panic!("stock values do not contain file items"),
        Value::Integer(width) => {
            let number = value.as_u64().expect("unsigned integer");
            let max = match width {
                Width::U8 => u64::from(u8::MAX),
                Width::U16 => u64::from(u16::MAX),
                Width::U32 => u64::from(u32::MAX),
                Width::U64 => u64::MAX,
            };
            assert!(number <= max, "integer fits {width:?}");
        }
        Value::Nanoseconds | Value::Time => {
            value.as_u64().expect("nanoseconds");
        }
        Value::Boolean => {
            value.as_bool().expect("boolean");
        }
        Value::Text(bound) => {
            let text = value.as_str().expect("text");
            assert!(u64::try_from(text.len()).expect("text length") <= fields::most(*bound, limits), "text bound");
        }
        Value::Hex(bound) => {
            let text = value.as_str().expect("hexadecimal text");
            assert_eq!(text.len() % 2, 0, "even hexadecimal length");
            assert!(u64::try_from(text.len() / 2).expect("hex length") <= fields::most(*bound, limits), "hex bound");
            for byte in text.bytes() {
                assert!(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte), "lower-case hex");
            }
        }
        Value::Words(words) => word(value, words),
    }
}
fn walk(shape: Shape, value: Json) {
    let limits = temper_client_tools::limits::form();
    let mut pending = vec![(shape, value)];
    while let Some((shape, value)) = pending.pop() {
        if let Choice::One { words } = fields::form(shape).choice {
            if value.is_string() {
                word(&value, words);
                continue;
            }
            assert_eq!(value.as_object().expect("choice object").len(), 1, "one choice member");
        }
        for (name, value) in value.as_object().expect("shape object") {
            let mut row = None;
            for field in fields::members(shape) {
                if field.name == name.as_bytes() {
                    row = Some(field);
                    break;
                }
            }
            let Some(field) = row else {
                panic!("{shape:?}.{name} has a row");
            };
            if value.is_null() {
                assert!(field.null, "{shape:?}.{name} permits null");
                continue;
            }
            if let Some(bound) = field.list {
                let list = value.as_array().expect("list");
                assert!(u64::try_from(list.len()).expect("list length") <= fields::most(bound, &limits), "list bound");
                for value in list {
                    scalar(value, field, &limits, &mut pending);
                }
            } else {
                scalar(value, field, &limits, &mut pending);
            }
        }
    }
}

#[test]
fn every_member_ctl_prints_for_the_stock_is_a_field() {
    // Stock has three charters, two templates, one policy and the rules.
    const STOCK_VALUES: usize = 7;
    let llm = llm();
    let models = models(&llm);
    let stock = temper_service::stock::defaults(&llm);
    let mut checked = 0;
    for value in stock.charters {
        walk(Shape::Charter, printed(&StockValue::Charter(Box::new(value)), &models));
        checked += 1;
    }
    for value in stock.templates {
        walk(Shape::Template, printed(&StockValue::Template(Box::new(value)), &models));
        checked += 1;
    }
    for (_, value) in &stock.policies {
        walk(Shape::Policy, printed(&StockValue::Policy(Box::new(value.clone())), &models));
        checked += 1;
    }
    walk(Shape::Rules, printed(&StockValue::Rules(Box::new(stock.rules)), &models));
    checked += 1;
    assert_eq!(checked, STOCK_VALUES, "every stock value checked");
}

fn setting_paths(text: &str) -> Vec<&str> {
    let mut paths = Vec::new();
    let mut fence = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let marker = if trimmed.starts_with("```") {
            Some(b'`')
        } else if trimmed.starts_with("~~~") {
            Some(b'~')
        } else {
            None
        };
        if let Some(marker) = marker {
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let mut start = None;
        for (at, byte) in line.bytes().enumerate() {
            if byte != b'`' {
                continue;
            }
            match start.take() {
                None => start = Some(at + 1),
                Some(from) => {
                    let span = &line[from..at];
                    let mut space = false;
                    for byte in span.bytes() {
                        if byte.is_ascii_whitespace() {
                            space = true;
                            break;
                        }
                    }
                    if !span.contains('.') || space {
                        continue;
                    }
                    for kind in ["charter.", "template.", "policy.", "rules."] {
                        if span.starts_with(kind) {
                            paths.push(span);
                            break;
                        }
                    }
                }
            }
        }
    }
    paths
}

#[test]
fn setting_paths_finds_the_kinds_paths_in_code_spans() {
    let text = r"`charter.model.name`
`charter` `engine.limits` `setting --json`
charter.number
```json
`template.number`
```
~~~text
`rules.maximum_run_spend`
~~~
`policy.roles[].number`
";
    assert_eq!(setting_paths(text), ["charter.model.name", "policy.roles[].number"]);
}

#[test]
fn every_setting_field_a_page_names_is_a_field() {
    for page in &temper_docs::PAGES {
        for path in setting_paths(page.text) {
            assert!(fields::resolve(path.as_bytes()).is_some(), "{}: unknown setting field {path}", page.name);
        }
    }
}
