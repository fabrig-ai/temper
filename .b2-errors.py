from pathlib import Path
import re
root=Path('crates/temper-engine-smith/src')
# Named primitive conversions. Optional lookups in match arms keep the lookup's name.
for name in ['calls.rs','nested.rs','notes.rs','docs.rs','forge.rs']:
 p=root/name;s=p.read_text()
 # Conversions on bare value identifiers in match arms: nearest preceding field, overridable below.
 lines=s.splitlines(); field='input'
 for i,line in enumerate(lines):
  m=re.search(r'\.field\(b"([^"]+)"\)',line)
  if m: field=m[1]
  if re.search(r'\.(text|number|boolean|array|small|narrow)\(\)',line):
   lines[i]=re.sub(r'\.(text|number|boolean|array|small|narrow)\(\)',lambda m:f'.{m[1]}(b"{field}")',line)
 s='\n'.join(lines)+'\n'
 p.write_text(s)
# Use member argument for shared list/number helpers.
p=root/'calls.rs';s=p.read_text().replace('field: &[u8]','field: &\'static [u8]').replace('.number(b"period")\n}', '.number(field)\n}').replace('value.required(field)?.number(b"period")','value.required(field)?.number(field)').replace('value.boolean(b"period")','value.boolean(field)')
# Every explicit range/missing at known sites.
fields=['words','repository','repository','repository','revision','form','decision','decision','kind','action']
for field in fields:
 s=re.sub(r'Problem::(Range|Missing)(?![A-Za-z_(])',lambda m:f'Problem::{m[1]}(b"{field}")',s,count=1)
s=s.replace('Err(Problem::Range)', 'Err(Problem::Range(field))')
p.write_text(s)
p=root/'nested.rs';s=p.read_text()
for field in ['deadline_seconds','kind','tracked','kind','kind','kind','kind','notes','terminal','terminal','results','kind','model','effort']:
 s=re.sub(r'Problem::Range(?![A-Za-z_(])',f'Problem::Range(b"{field}")',s,count=1)
# shared list helpers retain the incoming collection's name.
s=s.replace('fn texts(value: &Value)', 'fn texts(value: &Value, member: &\'static [u8])').replace('fn numbers(value: &Value)', 'fn numbers(value: &Value, member: &\'static [u8])')
s=s.replace('texts(pattern.required(b"segments")?)','texts(pattern.required(b"segments")?, b"segments")').replace('texts(grant.required(b"segments")?)','texts(grant.required(b"segments")?, b"segments")').replace('numbers(value)?','numbers(value, b"dependencies")?').replace('numbers(items)?','numbers(items, b"inputs")?')
# Repair helper primitive contexts, plus functions whose first operation is an array.
s=s.replace('let items = value.array(b"tracked")?;', 'let items = value.array(b"batch")?;',1)
a=s.index('fn numbers('); b=s.index('// Model indices',a);sub=s[a:b];sub=re.sub(r'\.(array|number|text)\(b"[^"]+"\)',r'.\1(member)',sub);s=s[:a]+sub+s[b:]
s=s.replace('let scopes = value.array(b"note_resources")?', 'let scopes = value.array(b"note_resources")?')
s=s.replace('let grants = value.array(b"note_resources")?', 'let grants = value.array(b"grants")?')
p.write_text(s)
p=root/'notes.rs';s=p.read_text()
for field in ['name','name','kind','terminal','kind','references']:
 s=re.sub(r'Problem::Range(?![A-Za-z_(])',f'Problem::Range(b"{field}")',s,count=1)
s=s.replace('segment.text(b"scopes")','segment.text(b"segments")')
a=s.index('fn numbers(');s=s[:a]+re.sub(r'\.(array|number)\(b"[^"]+"\)',r'.\1(b"references")',s[a:]);p.write_text(s)
p=root/'answers.rs';s=p.read_text().replace('Problem::Range','Problem::Range(b"answer")');s=s.replace('Problem::Missing | Problem::Type | Problem::Range(b"answer")','Problem::Missing(_) | Problem::Type(_) | Problem::Range(_)');p.write_text(s)
p=root/'finish.rs';s=p.read_text().replace('crate::decimal(&verdict.name)','crate::decimal(&verdict.name, b"name")');s=s.replace('Problem::Range','Problem::Range(b"name")');
for field in ['request','items','fields']:
 s=re.sub(r'Problem::Type(?![A-Za-z_(])',f'Problem::Type(b"{field}")',s,count=1)
for field in ['resource','body']:
 s=re.sub(r'Problem::Missing(?![A-Za-z_(])',f'Problem::Missing(b"{field}")',s,count=1)
p.write_text(s)
