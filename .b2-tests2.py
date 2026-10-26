from pathlib import Path
import re
p=Path('crates/temper-engine-smith/src/tests.rs');s=p.read_text()
fields=['target','revision','revision','revision','instructions','deadline_seconds','revision','kind','resource','revision','request','request','words','words','words','repository']
for field in fields:
 s=re.sub(r'Problem::(Type|Range|Missing)(?![A-Za-z_(])',lambda m:f'Problem::{m[1]}(b"{field}")',s,count=1)
s+='''
#[test]
fn flat_forge_reads_decode_the_repository_name_and_name_bad_members() {
    assert_eq!(decided(b"read_branch", br#"{"branch":"main"}"#).unwrap().tool,
        engine::Tool::ReadNamed { repository: None, read: skein_forge::api::Read::Branch { branch: b"main".as_slice().into() } });
    assert_eq!(decided(b"read_pull_for", br#"{"repository":"qa/pilot","head":"temper/1","base":"main"}"#).unwrap().tool,
        engine::Tool::ReadNamed { repository: Some(b"qa/pilot".as_slice().into()), read: skein_forge::api::Read::PullFor { head: b"temper/1".as_slice().into(), base: b"main".as_slice().into() } });
    for (tool, input, problem) in [
        (b"read_branch".as_slice(), b"{}".as_slice(), Problem::Missing(b"branch")),
        (b"read_branch", br#"{"branch":1}"#, Problem::Type(b"branch")),
        (b"read_reviews", br#"{"number":1,"page":4294967296}"#, Problem::Range(b"page")),
        (b"read_reviews", br#"{"number":1,"page":0}"#, Problem::Range(b"page")),
        (b"read_checks", br#"{"commit":"bad"}"#, Problem::Range(b"commit")),
    ] { assert_eq!(decided(tool, input), Err(problem)); }
    assert!(matches!(decided(b"read_items", b"{}").unwrap().tool,
        engine::Tool::ReadNamed { read: skein_forge::api::Read::Items { since: skein_lib::Time::ZERO, page: 1, kind: None }, .. }));
    assert!(matches!(decided(b"read_checks", br#"{"commit":"abcdef0123456789abcdef0123456789abcdef01"}"#).unwrap().tool,
        engine::Tool::ReadNamed { read: skein_forge::api::Read::Checks { commit }, .. } if commit[20..] == [0; 12]));
}

#[test]
fn every_forge_read_declares_its_flat_members_as_json() {
    let members: &[(&[u8], &[&[u8]])] = &[
        (b"read_items", &[b"since", b"page", b"item_kind"]),
        (b"read_item", &[b"number", b"after"]),
        (b"read_pull", &[b"number"]),
        (b"read_pull_for", &[b"head", b"base"]),
        (b"read_reviews", &[b"number", b"page"]),
        (b"read_statuses", &[b"commit", b"page"]),
        (b"read_remarks", &[b"number", b"review", b"page"]),
        (b"read_branch", &[b"branch"]),
        (b"read_branches", &[b"prefix"]),
        (b"read_pull_files", &[b"number", b"head", b"page"]),
        (b"read_compare", &[b"before", b"after"]),
        (b"read_checks", &[b"commit"]),
        (b"read_file", &[b"head", b"path", b"max_bytes"]),
        (b"read_job", &[b"head", b"run", b"job", b"attempt", b"max_bytes"]),
        (b"read_protection", &[b"branch"]),
        (b"read_settings", &[]),
        (b"read_collaborators", &[b"page"]),
        (b"read_permission", &[b"user"]),
    ];
    let mut count = 0;
    for tool in tools() {
        let Some((_, names)) = members.iter().find(|(name, _)| *name == tool.name.as_ref()) else { continue; };
        count += 1;
        let schema = crate::json::parse(&tool.schema).expect("JSON schema");
        assert!(!schema.required(b"additionalProperties").unwrap().boolean(b"additionalProperties").unwrap());
        let properties = schema.required(b"properties").unwrap();
        let crate::json::Value::Object(fields) = properties else { panic!("properties object"); };
        assert_eq!(fields.len(), names.len() + 1);
        assert_eq!(properties.required(b"repository").unwrap().required(b"type").unwrap().text(b"type").unwrap().as_ref(), b"string");
        for name in *names {
            let property = properties.required(name).unwrap();
            assert!(!property.required(b"description").unwrap().text(b"description").unwrap().is_empty());
            assert!(property.field(b"type").unwrap().is_some());
        }
    }
    assert_eq!(count, members.len());
}
'''
p.write_text(s)
p=Path('crates/temper-engine-smith/src/json.rs');s=p.read_text();at=s.index('    pub(crate) fn field(');s=s[:at]+'''    pub(crate) fn object(&self, member: &'static [u8]) -> Result<&Self, Problem> {
        match self {
            Value::Object(_) => Ok(self),
            Value::Array(_) | Value::String(_) | Value::Number(_) | Value::Boolean(_) | Value::Null => Err(Problem::Type(member)),
        }
    }

'''+s[at:];p.write_text(s)
p=Path('crates/temper-engine-smith/src/nested.rs');s=p.read_text();s=s.replace('Some(value) => Some(value.required(b"priority")?', 'Some(value) => Some(value.object(b"tracked")?.required(b"priority")?').replace('priority: value.required(b"priority")?', 'priority: value.object(b"tracked")?.required(b"priority")?')
for name,member in [('delegate','batch'),('amendment','amendment'),('executor','executor'),('spec','spec'),('contract','contract'),('authority','authority'),('wake_policy','wake'),('wake_rule','kind'),('model_effort','main')]:
 s=re.sub(r'(fn '+name+r'\([^\n]+\{\n)',r'\1    let value = value.object(b"'+member+r'")?;\n',s)
p.write_text(s)
# exact service regression expectations and invalid flat branch
p=Path('crates/temper-service/src/tests/tools.rs');s=p.read_text().replace('b"host call member missing"','b"host call member missing: target"') # inspect context later
p.write_text(s)
