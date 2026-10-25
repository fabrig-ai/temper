//! Admission of only protocol/git.md's builder vectors and whole environments.
use skein_io::kernel::Spawn;
use skein_lib::{Secret, base64};
use std::collections::BTreeMap;
use temper_worker_protocol_git::invocation::{self, Command, Source};
/// The name and email the commit's environment carries.
pub type Author = (Box<[u8]>, Box<[u8]>);
/// A login and token, kept solely on the fake's transport side.
pub type Credential = (Box<[u8]>, Secret);

/// An admitted invocation; credential debug output contains its length only.
#[derive(Debug)]
pub struct Checked {
    pub command: Command,
    pub at: Box<[u8]>,
    pub home: Box<[u8]>,
    pub committer: Author,
    pub author: Option<Author>,
    pub credential: Option<Credential>,
    /// The admitted Authorization header, forwarded verbatim on repository calls.
    pub authorization: Option<Secret>,
    /// commit-tree's stdin, filled by the hosted program after admission.
    pub message: Vec<u8>,
}
/// Usage is a normal git exit; a broken protocol rule fails the world.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Usage,
    Broken(&'static str),
}

/// Validate settings, exact argv, environment and remote safety.
/// # Errors
/// Returns Usage for unsupported argv, or Broken naming a contract violation.
pub fn check(spawn: &Spawn) -> Result<Checked, Refusal> {
    let args: Vec<_> = spawn.args.iter().map(AsRef::as_ref).collect();
    for (pair, setting) in args.chunks(2).take(invocation::SETTINGS.len()).zip(invocation::SETTINGS) {
        if pair != [b"-c".as_slice(), setting] {
            return Err(Refusal::Broken("git settings"));
        }
    }
    let rest = args.get(invocation::SETTINGS.len() * 2..).ok_or(Refusal::Broken("git settings"))?;
    let command = parse(rest).ok_or(Refusal::Usage)?;
    if invocation::arguments(&command) != spawn.args {
        return Err(Refusal::Usage);
    }
    if let Some(remote) = remote(&command) {
        check_remote(remote)?;
    }
    let mut env = BTreeMap::new();
    for entry in &spawn.env {
        let split = entry.iter().position(|b| *b == b'=').ok_or(Refusal::Broken("environment entry"))?;
        let (name, value) = entry.split_at(split);
        if env.insert(name, &value[1..]).is_some() {
            return Err(Refusal::Broken("duplicate environment name"));
        }
    }
    for (name, value) in [
        (b"LC_ALL".as_slice(), b"C".as_slice()),
        (b"TZ", b"UTC"),
        (b"GIT_CONFIG_NOSYSTEM", b"1"),
        (b"GIT_CONFIG_GLOBAL", b"/dev/null"),
        (b"GIT_TERMINAL_PROMPT", b"0"),
        (b"GIT_ASKPASS", b""),
    ] {
        if env.remove(name) != Some(value) {
            return Err(Refusal::Broken("fixed environment value"));
        }
    }
    let home = take(&mut env, b"HOME")?;
    if !home.starts_with(b"/") {
        return Err(Refusal::Broken("absolute HOME"));
    }
    let committer = (take(&mut env, b"GIT_COMMITTER_NAME")?, take(&mut env, b"GIT_COMMITTER_EMAIL")?);
    let author = if env.contains_key(b"GIT_AUTHOR_NAME".as_slice()) || env.contains_key(b"GIT_AUTHOR_EMAIL".as_slice())
    {
        match command {
            Command::Unmerged
            | Command::Add
            | Command::DiffIndex { .. }
            | Command::WriteTree
            | Command::CommitTree { .. } => {}
            Command::Version
            | Command::Clone { .. }
            | Command::Fetch { .. }
            | Command::Create { .. }
            | Command::CheckOut { .. }
            | Command::Clean
            | Command::Merge { .. }
            | Command::IsAncestor { .. }
            | Command::Push { .. } => return Err(Refusal::Broken("author outside commit")),
        }
        let author = (take(&mut env, b"GIT_AUTHOR_NAME")?, take(&mut env, b"GIT_AUTHOR_EMAIL")?);
        if author != committer {
            return Err(Refusal::Broken("author and committer differ"));
        }
        Some(author)
    } else {
        None
    };
    if matches!(command, Command::Add | Command::DiffIndex { .. } | Command::WriteTree | Command::CommitTree { .. })
        && author.is_none()
    {
        return Err(Refusal::Broken("commit author"));
    }
    let credential = if remote(&command).is_some()
        || (matches!(command, Command::IsAncestor { .. }) && env.contains_key(b"GIT_CONFIG_COUNT".as_slice()))
    {
        Some(read_credential(&mut env, spawn)?)
    } else {
        None
    };
    if !env.is_empty() {
        return Err(Refusal::Broken("unexpected environment name"));
    }
    if author.is_none()
        && credential.is_none()
        && (committer.0.as_ref() != b"temper" || committer.1.as_ref() != b"temper@temper.invalid")
    {
        return Err(Refusal::Broken("placeholder committer"));
    }
    let authorization = spawn
        .env
        .iter()
        .find_map(|e| e.strip_prefix(b"GIT_CONFIG_VALUE_0=Authorization: ").map(|value| Secret::new(value.into())));
    Ok(Checked {
        command,
        at: spawn.dir.clone(),
        home,
        committer,
        author,
        credential,
        authorization,
        message: Vec::new(),
    })
}
fn read_credential(env: &mut BTreeMap<&[u8], &[u8]>, spawn: &Spawn) -> Result<Credential, Refusal> {
    if env.remove(b"GIT_CONFIG_COUNT".as_slice()) != Some(b"1".as_slice())
        || env.remove(b"GIT_CONFIG_KEY_0".as_slice()) != Some(b"http.extraHeader".as_slice())
    {
        return Err(Refusal::Broken("credential configuration"));
    }
    let header = env.remove(b"GIT_CONFIG_VALUE_0".as_slice()).ok_or(Refusal::Broken("credential header"))?;
    let encoded = header.strip_prefix(b"Authorization: Basic ").ok_or(Refusal::Broken("credential scheme"))?;
    let raw = base64::decode(base64::Alphabet::Standard, encoded, u32::MAX)
        .map_err(|_| Refusal::Broken("credential base64"))?;
    let split = raw.iter().position(|b| *b == b':').ok_or(Refusal::Broken("credential login"))?;
    let token = &raw[split + 1..];
    if split == 0 || token.is_empty() {
        return Err(Refusal::Broken("empty credential"));
    }
    if spawn.args.iter().any(|arg| arg.windows(token.len()).any(|part| part == token))
        || spawn
            .env
            .iter()
            .filter(|e| !e.starts_with(b"GIT_CONFIG_VALUE_0="))
            .any(|e| e.windows(token.len()).any(|part| part == token))
    {
        return Err(Refusal::Broken("credential outside header"));
    }
    Ok((raw[..split].into(), Secret::new(token.into())))
}
fn take(env: &mut BTreeMap<&[u8], &[u8]>, name: &[u8]) -> Result<Box<[u8]>, Refusal> {
    env.remove(name).map(Into::into).ok_or(Refusal::Broken("missing environment name"))
}
fn check_remote(url: &[u8]) -> Result<(), Refusal> {
    let authority = if let Some(rest) = url.strip_prefix(b"https://") {
        rest
    } else if let Some(rest) = url.strip_prefix(b"http://") {
        let host = rest.split(|b| *b == b'/').next().expect("split always gives one item");
        let host = host.split(|b| *b == b':').next().expect("split always gives one item");
        if !matches!(host, b"localhost" | b"127.0.0.1") && !rest.starts_with(b"[::1]:") && !rest.starts_with(b"[::1]/")
        {
            return Err(Refusal::Broken("plaintext remote off loopback"));
        }
        rest
    } else {
        return Err(Refusal::Broken("remote scheme"));
    };
    let host = authority.split(|b| *b == b'/').next().expect("split always gives one item");
    if host.is_empty() || host.contains(&b'@') || url.iter().any(|b| *b <= 32 || *b == 127) {
        return Err(Refusal::Broken("remote authority"));
    }
    Ok(())
}
pub(crate) fn remote(command: &Command) -> Option<&[u8]> {
    match command {
        Command::Clone { remote, .. }
        | Command::Fetch { remote, .. }
        | Command::Create { remote, .. }
        | Command::Push { remote, .. } => Some(remote),
        Command::Version
        | Command::CheckOut { .. }
        | Command::Clean
        | Command::Merge { .. }
        | Command::Unmerged
        | Command::Add
        | Command::DiffIndex { .. }
        | Command::WriteTree
        | Command::CommitTree { .. }
        | Command::IsAncestor { .. } => None,
    }
}
fn parse(args: &[&[u8]]) -> Option<Command> {
    Some(match args {
        [b"version"] => Command::Version,
        [b"clone", b"--no-checkout", b"--no-tags", b"--origin", b"origin", b"--", url, dir] => {
            Command::Clone { remote: (*url).into(), directory: (*dir).into() }
        }
        [
            b"fetch",
            b"--porcelain",
            b"--verbose",
            b"--no-tags",
            b"--no-write-fetch-head",
            b"--no-recurse-submodules",
            b"--",
            url,
            spec,
        ] => {
            let from = spec.strip_prefix(b"+")?.strip_suffix(b":refs/temper/fetched")?;
            let source = if from == b"HEAD" {
                Source::Default
            } else if let Some(branch) = from.strip_prefix(b"refs/heads/") {
                Source::Branch(branch.into())
            } else {
                Source::Commit(invocation::object(from)?)
            };
            Command::Fetch { remote: (*url).into(), source }
        }
        [b"checkout", b"--force", b"--detach", commit] => Command::CheckOut { commit: invocation::object(commit)? },
        [b"clean", b"-ffdxq"] => Command::Clean,
        [b"merge", b"--no-commit", b"--no-ff", b"--", theirs] => Command::Merge { theirs: invocation::object(theirs)? },
        [b"diff-files", b"--name-only", b"-z", b"--diff-filter=U"] => Command::Unmerged,
        [b"add", b"--all"] => Command::Add,
        [b"diff-index", b"--cached", b"--quiet", parent, b"--"] => {
            Command::DiffIndex { parent: invocation::object(parent)? }
        }
        [b"write-tree"] => Command::WriteTree,
        [b"commit-tree", tree, b"-p", parent, b"-F", b"-"] => {
            Command::CommitTree { tree: invocation::object(tree)?, parent: invocation::object(parent)?, merging: None }
        }
        [b"commit-tree", tree, b"-p", parent, b"-p", second, b"-F", b"-"] => Command::CommitTree {
            tree: invocation::object(tree)?,
            parent: invocation::object(parent)?,
            merging: Some(invocation::object(second)?),
        },
        [b"merge-base", b"--is-ancestor", old, commit] => {
            Command::IsAncestor { expected: invocation::object(old)?, commit: invocation::object(commit)? }
        }
        [b"push", b"--porcelain", b"--", url, spec] => push(url, spec, None)?,
        [b"push", b"--porcelain", lease, b"--", url, spec] => push(url, spec, Some(lease))?,
        _ => return None,
    })
}
fn push(url: &[u8], spec: &[u8], lease: Option<&[u8]>) -> Option<Command> {
    let split = spec.windows(12).position(|part| part == b":refs/heads/")?;
    let commit = invocation::object(&spec[..split])?;
    let branch = &spec[split + 12..];
    let remote = url.into();
    if let Some(lease) = lease {
        let rest = lease.strip_prefix(b"--force-with-lease=refs/heads/")?;
        let expected = rest.strip_prefix(branch)?.strip_prefix(b":")?;
        if expected.is_empty() {
            return Some(Command::Create { remote, branch: branch.into(), commit });
        }
        return Some(Command::Push {
            remote,
            branch: branch.into(),
            commit,
            expected: Some(invocation::object(expected)?),
        });
    }
    Some(Command::Push { remote, branch: branch.into(), commit, expected: None })
}
