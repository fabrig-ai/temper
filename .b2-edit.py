from pathlib import Path
import re
root=Path('crates/temper-engine-smith/src')
p=root/'lib.rs'
s=p.read_text().replace('fn decimal(bytes: &[u8])','fn decimal(bytes: &[u8], member: &\'static [u8])')
for v in ['Missing','Type','Range']:
 s=s.replace(f'    {v},', f'    {v}(&\'static [u8]),')
s=s.replace('Problem::Range','Problem::Range(member)')
p.write_text(s)
for p in root.glob('*.rs'):
 if p.name in ['lib.rs','start.rs','answers.rs','finish.rs','json.rs']: continue
 s=p.read_text()
 s=re.sub(r'\.required\((b"[^"]+")\)\?\.(text|number|boolean|array|small|narrow)\(\)',r'.required(\1)?.\2(\1)',s)
 p.write_text(s)
p=root/'json.rs';s=p.read_text()
s=s.replace('name: &[u8]', 'name: &\'static [u8]')
s=s.replace('Problem::Missing)', 'Problem::Missing(name))')
for method in ['text','number','boolean','array','small','narrow']:
 s=s.replace(f'fn {method}(&self)',f'fn {method}(&self, member: &\'static [u8])')
s=s.replace('crate::decimal(text)','crate::decimal(text, member)').replace('self.number()?','self.number(member)?')
# Root object errors name input; object lookup errors name requested member.
s=s.replace('Err(Problem::Type)', 'Err(Problem::Type(member))').replace('Err(Problem::Range)', 'Err(Problem::Range(member))')
a=s.index('impl Value');s=s[:a].replace('Problem::Type(member)','Problem::Type(b"input")')+s[a:]
a=s.index('pub(crate) fn required');s=s[:a].replace('Problem::Type(member)','Problem::Type(name)')+s[a:]
p.write_text(s)
