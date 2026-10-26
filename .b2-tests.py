from pathlib import Path
import re,json
p=Path('crates/temper-engine-smith/src/tests.rs');s=p.read_text()
# Former nested forge inputs now flat; invalid defaultable forms need actual invalid members.
def flatten(m):
 try:o=json.loads(m[1])
 except ValueError:return m[0]
 if isinstance(o,dict) and 'read' in o and isinstance(o['read'],dict):
  o={'repository':'qa/pilot',**o['read']}
  if o=={'repository':'qa/pilot'}:o['page']=0 # invalid for formerly missing optional page, fix required-read tests separately
  return 'br#"'+json.dumps(o,separators=(',',':'))+'"#'
 return m[0]
s=re.sub(r'br#"(.*?)"#',flatten,s)
# Valid settings and collaborators omissions are valid; table has one valid and one invalid per read.
lines=s.splitlines()
for i,line in enumerate(lines):
 if '(b"read_settings",' in line:
  line=line.replace('br#"{"repository":"qa/pilot","page":0}"#','br#"{"repository":"qa/pilot"}"#',1)
 if '(b"read_items",' in line or '(b"read_collaborators",' in line:
  # Invalid input already page:0; valid inputs have page 1.
  pass
 lines[i]=line
s='\n'.join(lines)+'\n'
# Name error members in existing assertions, including tracked tables.
s=s.replace('("8", Problem::Type)', '("8", Problem::Type(b"tracked"))').replace('(r#""untracked""#, Problem::Type)', '(r#""untracked""#, Problem::Type(b"tracked"))').replace('(r#""tracked""#, Problem::Range)', '(r#""tracked""#, Problem::Range(b"tracked"))').replace('(r#"{"priority":"8"}"#, Problem::Type)', '(r#"{"priority":"8"}"#, Problem::Type(b"priority"))').replace('(r#"{"priority":4294967296}"#, Problem::Range)', '(r#"{"priority":4294967296}"#, Problem::Range(b"priority"))').replace('("{}", Problem::Missing)', '("{}", Problem::Missing(b"priority"))')
# Remaining expectation sites in source order.
fields=['target','revision','revision','revision','instructions','deadline_seconds','tools','kind','resource','revision','request','request','words','words','words','repository']
# actual remaining sequence inspect below; do not replace automatically yet
p.write_text(s)
p=Path('crates/temper-engine-smith/src/tests/docs.rs');s=p.read_text()
for line in s.splitlines():
 if 'Problem::' in line and 'br#' in line:
  member='from' if 'from' in line else ('search' if 'search' in line else 'topic')
  s=s.replace(line,re.sub(r'Problem::(Type|Range)(?!\()',lambda m:f'Problem::{m[1]}(b"{member}")',line))
p.write_text(s)
p=Path('crates/temper-engine-domain/src/route/forge_route/tests.rs');s=p.read_text().replace('landing_words(&merge, bound)','landing_words(&merge, bound, None, 1, None)')
s+='''
#[test]
fn landing_words_name_the_pull_and_use_the_forges_address_when_it_fits() {
    let repository = adopted(1);
    for (kind, web, line) in [
        (super::forge::Kind::GitHub, Some(b"https://github.example/".as_slice()), b"pull request #7 of org/repo on forge.example: https://github.example/org/repo/pull/7".as_slice()),
        (super::forge::Kind::Forgejo, Some(b"https://forge.example/base".as_slice()), b"pull request #7 of org/repo on forge.example: https://forge.example/base/org/repo/pulls/7".as_slice()),
        (super::forge::Kind::GitHub, None, b"pull request #7 of org/repo on forge.example".as_slice()),
    ] {
        let address = web.map(|web| (kind, web));
        let expected = [COMMITS[0].1, b"\\n", line].concat();
        assert_eq!(landing_words(&SHA1, 512, Some(&repository), 7, address).as_ref(), expected);
        let bound = u32::try_from(expected.len()).unwrap();
        assert_eq!(landing_words(&SHA1, bound, Some(&repository), 7, address).as_ref(), expected);
    }
    let line = b"pull request #7 of org/repo on forge.example";
    let bound = u32::try_from(COMMITS[0].1.len() + 1 + line.len()).unwrap();
    let expected = [COMMITS[0].1, b"\\n", line].concat();
    assert_eq!(landing_words(&SHA1, bound, Some(&repository), 7, Some((super::forge::Kind::GitHub, b"https://github.example"))).as_ref(), expected);
    assert_eq!(landing_words(&SHA1, bound - 1, Some(&repository), 7, None).as_ref(), COMMITS[0].1);
    assert_eq!(landing_words(&SHA1, 7, Some(&repository), 7, None).as_ref(), b"abcdef0");
    let long = [b'x'; 2048];
    assert_eq!(landing_words(&SHA1, 4096, Some(&repository), 7, Some((super::forge::Kind::Forgejo, &long))).as_ref(), expected);
}
''';p.write_text(s)
p=Path('crates/temper-client-domain/src/tests/pages/delegation.rs');s=p.read_text().replace('        (MERGE, Some(MERGE)),','        (MERGE, Some(MERGE)),\n        (b"abc1234\\npull request #7 of acme/app on github.com: https://github.com/acme/app/pull/7", Some(b"abc1234".as_slice())),');p.write_text(s)
