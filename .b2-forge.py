from pathlib import Path
import re,json
p=Path('crates/temper-engine-smith/src/forge.rs');s=p.read_text()
# Replace read defaults at the common read decoder used by flat names.
s=s.replace('value.required(b"since")?.number(b"since")?', 'optional_number(value, b"since", 0)?').replace('value.required(b"after")?.number(b"after")?', 'optional_number(value, b"after", 0)?')
s=s.replace('value.required(b"page")?.small(b"page")?', 'page(value)?').replace('value.required(b"max_bytes")?.small(b"max_bytes")?', 'max_bytes(value)?')
# All commits use their field's name.
s=re.sub(r'commit\(value.required\((b"[^"]+")\)\?\)',r'commit(value.required(\1)?, \1)',s)
s=s.replace('fn commit(value: &Value)', 'fn commit(value: &Value, member: &\'static [u8])')
a=s.index('fn commit(');b=s.index('fn optional_text',a)
s=s[:a]+'''fn commit(value: &Value, member: &'static [u8]) -> Result<api::Commit, Problem> {
    let text = value.text(member)?;
    if text.len() != 40 && text.len() != 64 {
        return Err(Problem::Range(member));
    }
    let mut commit = [0_u8; 32];
    for (pair, cell) in text.chunks_exact(2).zip(&mut commit) {
        *cell = high_hex(pair[0], member)? * 16 + high_hex(pair[1], member)?;
    }
    Ok(commit)
}

fn high_hex(value: u8, member: &'static [u8]) -> Result<u8, Problem> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(Problem::Range(member)),
    }
}

fn optional_number(value: &Value, member: &'static [u8], default: u64) -> Result<u64, Problem> {
    match value.field(member)? {
        Some(value) => value.number(member),
        None => Ok(default),
    }
}

fn page(value: &Value) -> Result<u32, Problem> {
    let page = optional_number(value, b"page", 1)?;
    if page == 0 { return Err(Problem::Range(b"page")); }
    u32::try_from(page).or(Err(Problem::Range(b"page")))
}

fn max_bytes(value: &Value) -> Result<u32, Problem> {
    // Zero denotes the root's configured read limit; explicit zero is invalid.
    match value.field(b"max_bytes")? {
        None => Ok(0),
        Some(value) => {
            let max = value.small(b"max_bytes")?;
            if max == 0 { Err(Problem::Range(b"max_bytes")) } else { Ok(max) }
        }
    }
}

'''+s[b:]
for field in ['kind','kind','item_kind','kind','kind','verdict','check','kind']:
 s=re.sub(r'Problem::Range(?![A-Za-z_(])',f'Problem::Range(b"{field}")',s,count=1)
s=s.replace('Problem::Type)', 'Problem::Type(b"input"))')
s=s.replace('fn optional_text(value: &Value, name: &[u8])','fn optional_text(value: &Value, name: &\'static [u8])').replace('value.text(b"kind")','value.text(name)') # last optional_text context verify
# Fix optional converter/list member names explicitly.
s=s.replace('Some(value) => Ok(Some(value.text(b"item_kind")?))','Some(value) => Ok(Some(value.text(name)?))')
s=s.replace('pub(crate) fn texts(value: &Value)', 'pub(crate) fn texts(value: &Value, member: &\'static [u8])')
s=s.replace('texts(value.required(b"segments")?)','texts(value.required(b"segments")?, b"segments")')
a=s.index('pub(crate) fn texts');b=s.index('fn numbers',a);sub=s[a:b];sub=re.sub(r'\.(array|text)\(b"[^"]+"\)',r'.\1(member)',sub);s=s[:a]+sub+s[b:]
a=s.index('fn numbers');s=s[:a]+re.sub(r'\.(array|number)\(b"[^"]+"\)',r'.\1(b"reviewers")',s[a:]);p.write_text(s)
p=Path('crates/temper-engine-smith/src/calls.rs');s=p.read_text();s=s.replace('engine::Tool::ReadForge {\n            repository: forge::repository(value.required(b"repository")?)?,\n            read: forge::read_named(tool, value.required(b"read")?)?,\n        }','engine::Tool::ReadNamed {\n            repository: match value.field(b"repository")? {\n                Some(value) => Some(value.text(b"repository")?),\n                None => None,\n            },\n            read: forge::read_named(tool, &value)?,\n        }')
s=s.replace('forge::texts(value.required(b"paths")?)','forge::texts(value.required(b"paths")?, b"paths")')
# Flat declarations, schemas independent of internal identifiers.
reads={'items':['since','page','item_kind'],'item':['number','after'],'pull':['number'],'pull_for':['head_branch','base'],'reviews':['number','page'],'statuses':['commit','page'],'remarks':['number','review','page'],'branch':['branch'],'branches':['prefix'],'pull_files':['number','head','page'],'compare':['before','after_commit'],'checks':['commit'],'file':['head','path','max_bytes'],'job':['head','run','job','attempt','max_bytes'],'protection':['branch'],'settings':[],'collaborators':['page'],'permission':['user']}
optional={'since','page','item_kind','after','max_bytes'}
desc={'repository':"owner/name of one of the project's repositories; leave out when it has one",'head_branch':"a branch name: the pull request's head",'base':"a branch name: the pull request's base",'head':'a commit id, 40 or 64 lowercase hex digits','commit':'a commit id, 40 or 64 lowercase hex digits','before':'a commit id, 40 or 64 lowercase hex digits','after_commit':'a commit id, 40 or 64 lowercase hex digits','branch':'a branch name','prefix':'a branch name prefix','path':'a file path in the repository','number':'an issue or pull request number','review':'a review id','run':'a CI run id','job':'a CI job id','attempt':'a CI attempt number','user':"a forge user's id",'since':'a time in nanoseconds; defaults to 0','after':'a comment id; defaults to 0','page':'a page number, from 1; defaults to 1','max_bytes':"the maximum bytes to read; defaults to the read's limit",'item_kind':'issue or pull; leave out for both'}
consts=[]
for read,fields in reads.items():
 props={};req=[]
 for f in ['repository']+fields:
  member=f.replace('head_branch','head').replace('after_commit','after');d={'type':'string' if f in ['repository','head_branch','base','head','commit','before','after_commit','branch','prefix','path','item_kind'] else 'integer','description':desc[f]}
  if d['type']=='integer': d['minimum']=0 if f in ['since','after'] else 1;d['maximum']=4294967295 if f in ['page','max_bytes','attempt'] else 18446744073709551615
  if f=='item_kind':d['enum']=['issue','pull']
  if f in ['since','after','page']:d['default']=1 if f=='page' else 0
  props[member]=d
  if f!='repository' and f not in optional:req.append(member)
 schema={'type':'object','additionalProperties':False,'properties':props,'required':req}
 const=read.upper()+'_SCHEMA';consts.append('const '+const+': &[u8] = br#"'+json.dumps(schema,separators=(',',':'))+'"#;')
 s=s.replace(f'name: b"read_{read}",',f'name: b"read_{read}",',1)
 s=re.sub(r'(Spec \{ name: b"read_'+read+r'",[^\n]+schema: )READ_SCHEMA',r'\g<1>'+const,s)
s=re.sub(r'const READ_SCHEMA:[^\n]+','\n'.join(consts),s)
p.write_text(s)
