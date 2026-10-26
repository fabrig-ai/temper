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
    let Some(repository) = repository else { return Box::from(&digits[..length]); };
    let number = skein_lib::Decimal::of(pull);
    let parts: [&[u8]; 6] = [b"pull request #", number.as_bytes(), b" of ", &repository.owner, b"/", &repository.name];
    let mut line_bytes = b" on ".len() + repository.host.len();
    for part in parts { line_bytes += part.len(); }
    // The descriptive line is atomic even when the contract shortens the id.
    if length < digits.len() || digits.len() + 1 + line_bytes > bound {
        return Box::from(&digits[..length]);
    }
    let mut address = None;
    if let Some((kind, mut web)) = web {
        while web.last() == Some(&b'/') { web = &web[..web.len() - 1]; }
        let route: &[u8] = match kind { forge::Kind::GitHub => b"/pull/", forge::Kind::Forgejo => b"/pulls/" };
        let address_bytes = web.len() + 1 + repository.owner.len() + 1 + repository.name.len() + route.len() + number.as_bytes().len();
        // Same address ceiling as the panel (client/forge.md, 9.2).
        const ADDRESS_BYTES: usize = 2048;
        if address_bytes <= ADDRESS_BYTES && digits.len() + 1 + line_bytes + 2 + address_bytes <= bound {
            address = Some((web, route, address_bytes));
        }
    }
    let mut bytes = digits.len() + 1 + line_bytes;
    if let Some((_, _, address_bytes)) = address { bytes += 2 + address_bytes; }
    let mut out = List::with_capacity(u32::try_from(bytes).expect("within contract bound"));
    append(&mut out, &digits);
    append(&mut out, b"\\n");
    for part in parts { append(&mut out, part); }
    append(&mut out, b" on ");
    append(&mut out, &repository.host);
    if let Some((web, route, _)) = address {
        for part in [b": ".as_slice(), web, b"/", &repository.owner, b"/", &repository.name, route, number.as_bytes()] {
            append(&mut out, part);
        }
    }
    out.into_boxed()
}
'''+s[b:]
old='''                words: landing_words(&merge, bound),'''
new='''                words: {
                    let row = domain.forge.change(task).expect("landed change");
                    let repository = domain.forge.repository(row.repository);
                    let forges = domain.forge.forges();
                    let mut web = None;
                    for forge in &forges.rows {
                        if forge.number == row.repository.forge {
                            if let Some(address) = &forge.web { web = Some((forge.kind, address.as_ref())); }
                            break;
                        }
                    }
                    landing_words(&merge, bound, repository, pull, web)
                },'''
assert old in s;s=s.replace(old,new);p.write_text(s)
p=Path('crates/temper-client-domain/src/pages/delegation.rs');s=p.read_text().replace('pub fn merge_commit(words: &[u8]) -> Option<&[u8]> {','''pub fn merge_commit(words: &[u8]) -> Option<&[u8]> {
    let words = words.split(|byte| *byte == b'\\n').next()?;''');# existing file uses closures? use bytes find to avoid closure
s=s.replace('let words = words.split(|byte| *byte == b\'\\n\').next()?;', '''let words = match skein_lib::bytes::find(words, b"\\n") {
        Some(end) => &words[..end],
        None => words,
    };''');p.write_text(s)
p=Path('crates/temper-client-tools/src/forge.rs');s=p.read_text().replace('''        out.words(commit);
    } else {''','''        out.words(commit);
        if let Some(start) = skein_lib::bytes::find(words.bytes(), b"\\n") {
            let line = &words.bytes()[start + 1..];
            if let Some(separator) = skein_lib::bytes::find(line, b": ") {
                out.put(b"\\npull request: ");
                out.words(&line[separator + 2..]);
            }
        }
    } else {''',1);p.write_text(s)
