from pathlib import Path
p=Path('crates/temper-engine-domain/src/route/forge_route.rs');s=p.read_text();a=s.index('fn landing_words(');b=s.index('\nfn news_words',a)
s=s[:a]+'''fn landing_words(
    merge: &skein_forge::api::Commit,
    bound: u32,
    repository: Option<&forge::Repository>,
    pull: u64,
    web: Option<(forge::Kind, &[u8])>,
) -> Box<[u8]> {
    let digits = commit_text(merge);
    let bound = usize::try_from(bound).expect("u32 fits usize");
    let length = digits.len().min(bound);
    let id = digits.get(..length).expect("landing words fit merge id");
    let line = match repository {
        Some(repository) if length == digits.len() => match bound.checked_sub(length.saturating_add(1)) {
            Some(room) => landing_pull(repository, pull, web, room),
            None => None,
        },
        Some(_) | None => None,
    };
    let Some(line) = line else { return Box::from(id); };
    let bytes = length.checked_add(1).expect("id separator").checked_add(line.len()).expect("bounded line");
    let mut out = List::with_capacity(u32::try_from(bytes).expect("within contract bound"));
    append(&mut out, id);
    append(&mut out, b"\\n");
    append(&mut out, &line);
    out.into_boxed()
}

fn landing_pull(
    repository: &forge::Repository,
    pull: u64,
    web: Option<(forge::Kind, &[u8])>,
    bound: usize,
) -> Option<Box<[u8]>> {
    let number = skein_lib::Decimal::of(pull);
    let parts: [&[u8]; 8] = [b"pull request #", number.as_bytes(), b" of ", &repository.owner, b"/", &repository.name, b" on ", &repository.host];
    let mut bytes = 0_usize;
    for part in parts { bytes = bytes.checked_add(part.len())?; }
    // The descriptive line is atomic even when the contract shortens the id.
    if bytes > bound { return None; }
    let mut address = None;
    if let Some((kind, mut web)) = web {
        while web.last() == Some(&b'/') { web = web.get(..web.len().checked_sub(1)?)?; }
        let route: &[u8] = match kind { forge::Kind::GitHub => b"/pull/", forge::Kind::Forgejo => b"/pulls/" };
        let mut address_bytes = 0_usize;
        for part in [web, b"/", &repository.owner, b"/", &repository.name, route, number.as_bytes()] {
            address_bytes = address_bytes.checked_add(part.len())?;
        }
        // Same address ceiling as the panel (client/forge.md, 9.2).
        const ADDRESS_BYTES: usize = 2048;
        let total = bytes.checked_add(2)?.checked_add(address_bytes)?;
        if address_bytes <= ADDRESS_BYTES && total <= bound {
            address = Some((web, route));
            bytes = total;
        }
    }
    let mut out = List::with_capacity(u32::try_from(bytes).ok()?);
    for part in parts { append(&mut out, part); }
    if let Some((web, route)) = address {
        for part in [b": ".as_slice(), web, b"/", &repository.owner, b"/", &repository.name, route, number.as_bytes()] {
            append(&mut out, part);
        }
    }
    Some(out.into_boxed())
}
'''+s[b:];p.write_text(s)
p=Path('crates/temper-engine-domain/src/route/tools.rs');s=p.read_text().replace('let tool = match body.tool {\n                Tool::ReadNamed { repository, mut read } => {','let tool = if let Tool::ReadNamed { repository, mut read } = body.tool {')
s=s.replace('''                    match &mut read {
                        skein_forge::api::Read::File { max_bytes, .. }
                        | skein_forge::api::Read::Job { max_bytes, .. } if *max_bytes == 0 => {
                            *max_bytes = env.limits.forge.client.answer_bytes;
                        }
                        _ => {}
                    }
                    Tool::ReadForge { repository: adopted.provider, read }
                }
                tool => tool,
            };''','''                    if let skein_forge::api::Read::File { max_bytes, .. }
                        | skein_forge::api::Read::Job { max_bytes, .. } = &mut read
                        && *max_bytes == 0 {
                            *max_bytes = env.limits.forge.client.answer_bytes;
                    }
                    Tool::ReadForge { repository: adopted.provider, read }
                } else { body.tool };''')
p.write_text(s)
p=Path('crates/temper-engine-smith/src/forge.rs');s=p.read_text().replace('*cell = high_hex(pair[0], member)? * 16 + high_hex(pair[1], member)?;','''let high = high_hex(*pair.first().expect("hex pair"), member)?;
        let low = high_hex(*pair.get(1).expect("hex pair"), member)?;
        *cell = high.saturating_mul(16).saturating_add(low);''').replace("Ok(value - b'0')", "Ok(value.saturating_sub(b'0'))").replace("Ok(value - b'a' + 10)", "Ok(value.saturating_sub(b'a').saturating_add(10))")
# Schema-positive read numbers reject zero at decode.
a=s.index('pub(crate) fn read(');b=s.index('/// Bind one declared',a);sub=s[a:b]
for member in ['number','review','run','job','user']:
 sub=sub.replace(f'value.required(b"{member}")?.number(b"{member}")?',f'positive_number(value, b"{member}")?')
sub=sub.replace('value.required(b"attempt")?.small(b"attempt")?', 'u32::try_from(positive_number(value, b"attempt")?).or(Err(Problem::Range(b"attempt")))?')
s=s[:a]+sub+s[b:];at=s.index('fn optional_number(');s=s[:at]+'''fn positive_number(value: &Value, member: &'static [u8]) -> Result<u64, Problem> {
    let number = value.required(member)?.number(member)?;
    if number == 0 { Err(Problem::Range(member)) } else { Ok(number) }
}

'''+s[at:];p.write_text(s)
p=Path('crates/temper-engine-smith/src/answers.rs');s=p.read_text().replace('''                text.add(match answer {
                    CallAnswer::ReadRepositoryRefused { .. } => b"this project has no repository to read",
                    _ => b"this project has no repository to change",
                })?;''','''                if matches!(answer, CallAnswer::ReadRepositoryRefused { .. }) {
                    text.add(b"this project has no repository to read")?;
                } else {
                    text.add(b"this project has no repository to change")?;
                }''');p.write_text(s)
p=Path('crates/temper-client-domain/src/pages/delegation.rs');s=p.read_text().replace('&words[..end]', 'words.get(..end).expect("newline within words")');p.write_text(s)
p=Path('crates/temper-client-tools/src/forge.rs');s=p.read_text().replace('&words.bytes()[start + 1..]','words.bytes().get(start.checked_add(1).expect("newline separator")..).expect("newline within words")').replace('&line[separator + 2..]','line.get(separator.checked_add(2).expect("address separator")..).expect("separator within line")');p.write_text(s)
p=Path('crates/temper-service/src/tools.rs');s=p.read_text().replace('Writer::new(body.len() + 2 + member.len())','Writer::new(body.len().checked_add(2).expect("fixed error separator").checked_add(member.len()).expect("static error member"))');p.write_text(s)
