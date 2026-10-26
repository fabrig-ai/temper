from pathlib import Path
p=Path('crates/temper-engine-domain/src/boundary.rs');s=p.read_text().replace('    ReadForge {', '    ReadNamed { repository: Option<Box<[u8]>>, read: skein_forge::api::Read },\n    ReadForge {');p.write_text(s)
p=Path('crates/temper-engine-domain/src/translate.rs');s=p.read_text().replace('Tool::ReadForge { .. }','Tool::ReadNamed { .. } | Tool::ReadForge { .. }').replace('| CallAnswer::ChangeRefused { .. }','| CallAnswer::ReadRepositoryRefused { .. }\n        | CallAnswer::ChangeRefused { .. }');p.write_text(s)
p=Path('crates/temper-engine-domain-forge/src/domain.rs');s=p.read_text();at=s.index('    /// Adopted repositories in this project that still permit')
s=s[:at]+'''    /// All adopted repositories of this project on forges that have not ended.
    #[must_use]
    pub fn readable_repositories(&self, project: u32) -> Box<[&Repository]> {
        let mut repositories = List::with_capacity(self.repositories.capacity());
        for (_, repository) in &self.repositories {
            if repository.project == project && !self.forge_ended(repository.provider.forge) {
                repositories.push(repository).expect("bounded adopted repositories");
            }
        }
        repositories.into_boxed()
    }

'''+s[at:];p.write_text(s)
p=Path('crates/temper-engine-domain/src/route/forge_route.rs');s=p.read_text();at=s.index('/// Expand a chat\'s request;')
s=s[:at]+'''/// Resolve a run's repository name before checking authority on the read's resource.
pub(crate) fn named_repository(
    domain: &Domain,
    project: u32,
    named: Option<&[u8]>,
) -> Result<forge::Repository, tasks::Refusal> {
    let repositories = domain.forge.readable_repositories(project);
    let mut chosen = None;
    for repository in &repositories {
        let fits = match named {
            Some(named) => repository_name(repository).as_ref() == named,
            None => repositories.len() == 1,
        };
        if fits {
            if chosen.is_some() { return Err(tasks::Refusal::Unknown); }
            chosen = Some(*repository);
        }
    }
    match chosen {
        Some(repository) => Ok(repository.clone()),
        None => Err(tasks::Refusal::Unknown),
    }
}

pub(crate) fn read_repositories(domain: &Domain, project: u32) -> Box<[Box<[u8]>]> {
    let repositories = domain.forge.readable_repositories(project);
    let mut names = List::with_capacity(u32::try_from(repositories.len()).expect("bounded repositories"));
    for repository in &repositories {
        names.push(repository_name(repository)).expect("bounded repositories");
    }
    names.into_boxed()
}

'''+s[at:];p.write_text(s)
p=Path('crates/temper-engine-domain/src/route/tools.rs');s=p.read_text();s=s.replace('            match body.tool {','''            let tool = match body.tool {
                Tool::ReadNamed { repository, mut read } => {
                    let project = domain.core.tasks.delegation(key.task).map(|context| context.project);
                    let resolved = match project {
                        Some(project) => forge_route::named_repository(domain, project, repository.as_deref()),
                        None => Err(tasks::Refusal::Unknown),
                    };
                    let adopted = match resolved {
                        Ok(adopted) => adopted,
                        Err(_) => {
                            let repositories = match project {
                                Some(project) => forge_route::read_repositories(domain, project),
                                None => Box::new([]),
                            };
                            super::host_route::read(domain, reply_to, super::CallAnswer::ReadRepositoryRefused {
                                named: repository, repositories,
                            });
                            return;
                        }
                    };
                    match &mut read {
                        skein_forge::api::Read::File { max_bytes, .. }
                        | skein_forge::api::Read::Job { max_bytes, .. } if *max_bytes == 0 => {
                            *max_bytes = env.limits.forge.client.answer_bytes;
                        }
                        _ => {}
                    }
                    Tool::ReadForge { repository: adopted.provider, read }
                }
                tool => tool,
            };
            match tool {
                Tool::ReadNamed { .. } => unreachable!("named reads resolved before deciding"),''',1)
# no closure in production strict subset
s=s.replace('let project = domain.core.tasks.delegation(key.task).map(|context| context.project);','let project = match domain.core.tasks.delegation(key.task) { Some(context) => Some(context.project), None => None };')
p.write_text(s)
p=Path('crates/temper-engine-domain/src/store.rs');s=p.read_text();s=s.replace('    ChangeRefused {','    ReadRepositoryRefused { named: Option<Box<[u8]>>, repositories: Box<[Box<[u8]>]> },\n    ChangeRefused {',1).replace('CallAnswer::ForgeEffectRefused(_) | CallAnswer::ForgeRead(_)','CallAnswer::ForgeEffectRefused(_) | CallAnswer::ForgeRead(_) | CallAnswer::ReadRepositoryRefused { .. }')
s=s.replace('        CallAnswer::ChangeRefused { named, repositories, .. } => {','        CallAnswer::ReadRepositoryRefused { named, repositories } | CallAnswer::ChangeRefused { named, repositories, .. } => {');p.write_text(s)
p=Path('crates/temper-engine-smith/src/answers.rs');s=p.read_text().replace('        CallAnswer::ChangeRefused { named, repositories, .. } => {','        CallAnswer::ReadRepositoryRefused { named, repositories } | CallAnswer::ChangeRefused { named, repositories, .. } => {').replace('                text.add(b"this project has no repository to change")?;', '''                text.add(match answer {
                    CallAnswer::ReadRepositoryRefused { .. } => b"this project has no repository to read",
                    _ => b"this project has no repository to change",
                })?;''');# verify variable name
p.write_text(s)
p=Path('crates/temper-engine-smith/src/calls.rs');s=p.read_text().replace('forge::texts(paths)?','forge::texts(paths, b"paths")?');p.write_text(s)
p=Path('crates/temper-service/src/tools.rs');s=p.read_text().replace('invalid(to, Problem::Range)','invalid(to, Problem::Range(b"activation"))')
s=s.replace('let body: &[u8] = match problem {','let (body, member): (&[u8], Option<&[u8]>) = match problem {')
s=s.replace('Problem::Malformed => b"malformed host call",','Problem::Malformed => (b"malformed host call", None),').replace('Problem::TooLarge => b"host call too large",','Problem::TooLarge => (b"host call too large", None),').replace('Problem::UnknownTool => b"unknown tool",','Problem::UnknownTool => (b"unknown tool", None),')
s=s.replace('Problem::Missing => b"host call member missing",','Problem::Missing(member) => (b"host call member missing", Some(member)),').replace('Problem::Type => b"host call member of wrong type",','Problem::Type(member) => (b"host call member of wrong type", Some(member)),').replace('Problem::Range => b"host call out of range",','Problem::Range(member) => (b"host call out of range", Some(member)),')
s=s.replace('    engine::Event::Worker(engine::WorkerInput::RenderedHostCall {\n        to,\n        answer: core::SettledAnswer::Host { error: true, body: body.into() },','''    let body = match member {
        None => body.into(),
        Some(member) => {
            let mut writer = Writer::new(body.len() + 2 + member.len());
            writer.put(body).expect("measured error");
            writer.put(b": ").expect("measured separator");
            writer.put(member).expect("static member name");
            writer.finish()
        }
    };
    engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
        to,
        answer: core::SettledAnswer::Host { error: true, body },''');p.write_text(s)
