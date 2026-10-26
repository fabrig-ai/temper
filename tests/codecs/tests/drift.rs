//! The committed codecs and golden bytes against their schemas (skein's codec.md, 6).
use skein_codegen::{generate, parse};
use std::fs;

fn schema_matches(crate_name: &str, family: &str, version: &str) {
    let root = skein_scratch::paths::workspace().join("crates").join(crate_name);
    let source =
        fs::read_to_string(root.join("schema").join(format!("{family}-{version}.schema"))).expect("family schema");
    let output = generate(&parse(&source).expect("schema valid"));
    let code = root.join("src/generated").join(format!("{version}.rs"));
    let goldens = root.join("golden");
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    let again = "run SKEIN_CODEGEN_WRITE=1 heavy cargo nextest run -p temper-codecs-world --test drift";
    if write {
        fs::write(&code, &output.rust).expect("write generated code");
        fs::create_dir_all(&goldens).expect("create golden directory");
    } else {
        assert_eq!(fs::read_to_string(&code).expect("generated code committed"), output.rust, "{again}");
    }
    for golden in output.goldens {
        let path = goldens.join(golden.name);
        if write {
            fs::write(&path, golden.bytes).expect("write golden bytes");
        } else {
            assert_eq!(fs::read(&path).expect("golden bytes committed"), golden.bytes, "{again}");
        }
    }
}

#[test]
fn worker_wire_schema_matches_generated_code_and_goldens() {
    schema_matches("temper-protocol-worker-wire", "worker-wire", "v1");
}

#[test]
fn client_wire_schema_matches_generated_code_and_goldens() {
    schema_matches("temper-client-protocol-wire", "client-wire", "v1");
}

#[test]
fn store_schema_matches_generated_code_and_goldens() {
    schema_matches("temper-protocol-store", "store", "v10");
}

#[test]
fn secrets_schema_matches_generated_code_and_goldens() {
    schema_matches("temper-protocol-records", "secrets", "v1");
}
