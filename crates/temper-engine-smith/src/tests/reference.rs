#![expect(clippy::disallowed_methods, reason = "step fixtures validate the renderer's UTF-8 output")]

use skein_lib::Writer;
use smith_domain_run::HostEffect;

use crate::{
    calls::{DESCRIPTION_BYTES, MEMBER_BYTES, SPECS, Spec},
    json::{self, Value},
    reference::{reference, render},
};

const HEAD: &[u8] = b"# Host tools\n\nKind: reference\n\nEvery tool temper adds to its runs' own: what it does, as a run reads it, whether it reads or writes, and the members of its input.\n";

fn fixture(schema: &'static [u8], effect: HostEffect) -> Box<[u8]> {
    let specs = [Spec { name: b"fixture_read", description: b"Read a fixture.", schema, effect }];
    let mut out = Writer::new(temper_docs::PAGE_BYTES);
    render(&specs, &mut out).expect("fixture fits page bound");
    let mut exact = Writer::new(out.written());
    render(&specs, &mut exact).expect("fixture fits measured length");
    exact.finish()
}

#[test]
fn a_tool_renders_its_description_effect_and_members() {
    let schema = br#"{
        "type":"object",
        "properties":{
            "number":{"type":"integer","minimum":1,"maximum":9,"description":"a number"},
            "page":{"type":"integer","minimum":1,"default":1},
            "kind":{"type":"string","enum":["issue","pull"],"description":"which"},
            "name":{"type":"string"},
            "limit":{"type":"integer","maximum":7},
            "flag":{"type":"boolean"},
            "shape":{"type":"object","properties":{"nested":{"type":"string"}}},
            "list":{"type":"array","items":{"type":"string"}},
            "any":{}
        },
        "required":["number","name"]
    }"#;
    let expected = b"\n## fixture_read\n\nRead a fixture.\n\nA read.\n\n\
- `number`: an integer from 1 to 9; required. a number\n\
- `page`: an integer of at least 1, 1 when absent; optional.\n\
- `kind`: one of `issue` or `pull`; optional. which\n\
- `name`: text; required.\n\
- `limit`: an integer of at most 7; optional.\n\
- `flag`: true or false; optional.\n\
- `shape`: an object; optional.\n\
- `list`: a list; optional.\n\
- `any`: any value; optional.\n";
    assert_eq!(fixture(schema, HostEffect::Read).as_ref(), [HEAD, expected].concat());
}

#[test]
fn a_tool_with_no_members_says_its_input_is_empty() {
    let expected = b"\n## fixture_read\n\nRead a fixture.\n\nA write.\n\nIts input is `{}`.\n";
    for schema in [br#"{"type":"object","properties":{}}"#.as_slice(), br#"{"type":"object"}"#] {
        assert_eq!(fixture(schema, HostEffect::Write).as_ref(), [HEAD, expected].concat());
    }
}

#[test]
fn the_page_holds_every_declared_tool_in_the_tables_order() {
    // Main's 37 host tools plus the run's two settings tools.
    const TOOLS: usize = 39;
    let mut out = Writer::new(temper_docs::PAGE_BYTES);
    reference(&mut out).expect("page fits PAGE_BYTES");
    let mut exact = Writer::new(out.written());
    reference(&mut exact).expect("page fits measured length");
    let page = exact.finish();
    let text = core::str::from_utf8(&page).expect("reference UTF-8");
    let mut headings = text.lines();
    for spec in &SPECS {
        let mut heading = None;
        for line in headings.by_ref() {
            if let Some(name) = line.strip_prefix("## ") {
                heading = Some(name.as_bytes());
                break;
            }
        }
        assert_eq!(heading, Some(spec.name));
    }
    for line in headings {
        assert!(!line.starts_with("## "), "no undeclared tool heading");
    }
    assert_eq!(SPECS.len(), TOOLS);
    assert!(page.len() <= temper_docs::PAGE_BYTES);
}

#[test]
fn member_types_keep_enum_order_and_json_defaults() {
    let schema = br#"{"properties":{
        "one":{"enum":["a"],"type":"integer"},
        "two":{"enum":["b","a"]},
        "three":{"enum":["a","b","c"]},
        "number":{"type":"number","minimum":-1.20e2,"maximum":4.5,"default":1e+2},
        "integer":{"type":"integer"}
    }}"#;
    let expected = br"
## fixture_read

Read a fixture.

A read.

- `one`: one of `a`; optional.
- `two`: one of `b` or `a`; optional.
- `three`: one of `a`, `b` or `c`; optional.
- `number`: a number from -1.20e2 to 4.5, 1e+2 when absent; optional.
- `integer`: an integer; optional.
";
    assert_eq!(fixture(schema, HostEffect::Read).as_ref(), [HEAD, expected].concat());
}

#[test]
fn every_description_is_within_its_bound() {
    for spec in &SPECS {
        let tool = core::str::from_utf8(spec.name).expect("tool name UTF-8");
        assert!(spec.description.len() <= DESCRIPTION_BYTES, "{tool}: description exceeds {DESCRIPTION_BYTES} bytes");
        let schema = json::parse(spec.schema).expect("declared schema");
        member_descriptions(&schema, tool, "input");
    }
}

fn member_descriptions(schema: &Value, tool: &str, member: &str) {
    match schema {
        Value::Object(fields) => {
            for (key, value) in fields {
                match key.as_ref() {
                    b"description" => {
                        let Value::String(text) = value else { panic!("{tool}.{member}: description is text") };
                        assert!(
                            text.len() <= MEMBER_BYTES,
                            "{tool}.{member}: description exceeds {MEMBER_BYTES} bytes"
                        );
                    }
                    b"properties" => {
                        let Value::Object(properties) = value else { panic!("declared properties are an object") };
                        for (name, property) in properties {
                            let name = core::str::from_utf8(name).expect("member name UTF-8");
                            member_descriptions(property, tool, &format!("{member}.{name}"));
                        }
                    }
                    b"items" => member_descriptions(value, tool, &format!("{member}[]")),
                    _ => member_descriptions(value, tool, member),
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                member_descriptions(value, tool, member);
            }
        }
        Value::String(_) | Value::Number(_) | Value::Boolean(_) | Value::Null => {}
    }
}
