//! The shipped init template's default model, used only by scripted workers.
pub fn box_model() -> temper_engine_domain::Model {
    let document: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../../../crates/temper-init/template/temper.json"))
            .expect("init document");
    let model = &document["engine"]["llm"]["models"][0];
    let mut value = temper_engine_domain_world::walking::models()[0].clone();
    value.dialect = u32::try_from(document["engine"]["llm"]["endpoints"][0]["dialect"].as_u64().expect("dialect"))
        .expect("bounded dialect");
    value.name = model["name"].as_str().expect("model name").as_bytes().into();
    value.window = u32::try_from(model["window"].as_u64().expect("window")).expect("bounded window");
    value.output = u32::try_from(model["output"].as_u64().expect("output")).expect("bounded output");
    value.input_price = model["prices"]["input"].as_u64().expect("input price");
    value.cached_price = model["prices"]["cached"].as_u64().expect("cache price");
    value.cache_write_price = model["prices"]["cache_write"].as_u64();
    value.output_price = model["prices"]["output"].as_u64().expect("output price");
    value.price_unit =
        u32::try_from(model["prices"]["unit"].as_u64().expect("price unit")).expect("bounded price unit");
    value
}

