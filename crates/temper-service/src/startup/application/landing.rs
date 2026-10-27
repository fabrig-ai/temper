//! Application landing rules use jig's pattern and project vocabulary.
use super::{Problems, What, limits, read};
use alloc::boxed::Box;
use jig_protocol_config::{self as config, Path};
use skein_json::Document;
use skein_lib::{List, Map};
use temper_engine_domain::{self as engine, Approval, Freshness, Gate, LandingRule};
use temper_engine_domain_forge::{CHECK_NAME_BYTES, WORKFLOW_BYTES};

fn freshness(
    object: &mut config::read::Object,
    document: &Document,
    path: &Path,
    problems: &mut Problems,
) -> Option<Freshness> {
    let value = read::text(object, document, b"freshness", path, problems)?;
    match value.as_ref() {
        b"exact" => Some(Freshness::Exact),
        b"clean" => Some(Freshness::Clean),
        _ => {
            read::form(false, path.member(b"freshness"), problems);
            None
        }
    }
}
fn pattern(
    document: &Document,
    at: u32,
    path: &Path,
    limits: &jig_domain_authority::Limits,
    problems: &mut Problems,
) -> Option<jig_domain_authority::Pattern> {
    let mut object = read::object(document, at, path, problems)?;
    let ats = read::list(&mut object, document, b"segments", limits.segments, path, problems);
    let last = read::text(&mut object, document, b"last", path, problems);
    let value = read::text(&mut object, document, b"value", path, problems);
    read::finish(object, document, path, problems);
    let mut segments = List::with_capacity(limits.segments);
    if let Some(ats) = ats {
        for (index, at) in ats.iter().enumerate() {
            match config::read::text(document, *at, limits.segment_bytes) {
                Ok(segment) => segments.push(segment).expect("bounded segments"),
                Err(what) => problems.push(read::index(&path.member(b"segments"), index), What::Jig(what)),
            }
        }
    }
    let value = value?;
    if value.len() > usize::try_from(limits.segment_bytes).expect("segment limit") {
        problems.push(path.member(b"value"), What::Jig(config::What::Bound));
    }
    let last = match last?.as_ref() {
        b"exact" => jig_domain_authority::Last::Exact(value),
        b"open" => jig_domain_authority::Last::Open(value),
        _ => {
            read::form(false, path.member(b"last"), problems);
            return None;
        }
    };
    Some(jig_domain_authority::Pattern { segments: segments.into_boxed(), last })
}
fn rule(
    document: &Document,
    at: u32,
    path: &Path,
    jig: &config::Engine,
    limits: &limits::Limits,
    problems: &mut Problems,
) -> Option<LandingRule> {
    let mut object = read::object(document, at, path, problems)?;
    let connector = read::short(&mut object, document, b"connector", path, problems);
    let kind = read::short(&mut object, document, b"kind", path, problems);
    let pattern_at = read::required(&mut object, document, b"pattern", path, problems);
    let enforce = read::boolean(&mut object, document, b"enforce", path, problems);
    let ci = read::boolean(&mut object, document, b"ci", path, problems);
    let up_to_date = read::boolean(&mut object, document, b"up_to_date", path, problems);
    let gate_ats = read::list(&mut object, document, b"gates", limits.forge.change_policy.gates, path, problems);
    let approval_ats = read::list(&mut object, document, b"approvals", jig.limits.core.authority.roles, path, problems);
    read::finish(object, document, path, problems);
    if let Some(connector) = connector
        && !jig.connectors.contains(&connector)
    {
        problems.push(path.member(b"connector"), What::Jig(config::What::Refers));
    }
    if let Some(kind) = kind {
        read::form(kind == 4, path.member(b"kind"), problems);
    }
    let pattern = match pattern_at {
        Some(at) => pattern(document, at, &path.member(b"pattern"), &jig.limits.core.authority, problems),
        None => None,
    };
    let mut gates = List::with_capacity(limits.forge.change_policy.gates);
    if let Some(ats) = gate_ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"gates"), index);
            let Some(mut object) = read::object(document, *at, &path, problems) else { continue };
            let number = read::number(&mut object, document, b"number", &path, problems);
            let blocking = read::boolean(&mut object, document, b"blocking", &path, problems);
            let freshness = freshness(&mut object, document, &path, problems);
            read::finish(object, document, &path, problems);
            if let Some(number) = number
                && let Some(blocking) = blocking
                && let Some(freshness) = freshness
            {
                gates.push(Gate { number, blocking, freshness }).expect("bounded gates");
            }
        }
    }
    let mut approvals = List::with_capacity(jig.limits.core.authority.roles);
    if let Some(ats) = approval_ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"approvals"), index);
            let Some(mut object) = read::object(document, *at, &path, problems) else { continue };
            let role = read::number(&mut object, document, b"role", &path, problems);
            let people = read::number(&mut object, document, b"people", &path, problems);
            let freshness = freshness(&mut object, document, &path, problems);
            read::finish(object, document, &path, problems);
            if let Some(role) = role
                && let Some(people) = people
                && let Some(freshness) = freshness
            {
                approvals.push(Approval { role, people, freshness }).expect("bounded approvals");
            }
        }
    }
    Some(LandingRule {
        connector: connector?,
        kind: kind?,
        pattern: pattern?,
        enforce: enforce?,
        ci: ci?,
        up_to_date: up_to_date?,
        gates: gates.into_boxed(),
        approvals: approvals.into_boxed(),
    })
}
fn rules(
    document: &Document,
    ats: List<u32>,
    path: &Path,
    jig: &config::Engine,
    limits: &limits::Limits,
    project: Option<u32>,
    problems: &mut Problems,
) -> Box<[LandingRule]> {
    let mut rules = List::with_capacity(ats.len());
    for (index, at) in ats.iter().enumerate() {
        let path = read::index(path, index);
        if let Some(rule) = rule(document, *at, &path, jig, limits, problems) {
            for (index, approval) in rule.approvals.iter().enumerate() {
                for configured in &jig.projects {
                    if let Some(project) = project
                        && configured.number != project
                    {
                        continue;
                    }
                    let mut found = false;
                    for role in &configured.policy.roles {
                        if role.number == approval.role {
                            found = true;
                        }
                    }
                    if !found {
                        problems.push(
                            read::index(&path.member(b"approvals"), index).member(b"role"),
                            What::Jig(config::What::Refers),
                        );
                    }
                }
            }
            rules.push(rule).expect("bounded rules");
        }
    }
    rules.into_boxed()
}
pub(super) fn parse(
    document: &Document,
    at: u32,
    jig: &config::Engine,
    limits: &limits::Limits,
    forges: &[super::Forge],
    problems: &mut Problems,
) -> Option<(engine::LandingPolicy, Box<[u32]>)> {
    let path = Path::root().member(b"application").member(b"landing");
    let mut object = read::object(document, at, &path, problems)?;
    let deployment_ats =
        read::list(&mut object, document, b"deployment", jig.limits.core.authority.requirements, &path, problems);
    let project_ats =
        read::list(&mut object, document, b"projects", jig.limits.core.authority.projects, &path, problems);
    read::finish(object, document, &path, problems);
    let deployment = rules(document, deployment_ats?, &path.member(b"deployment"), jig, limits, None, problems);
    let mut projects = Map::with_capacity(jig.limits.core.authority.projects);
    let mut order = List::with_capacity(jig.limits.core.authority.projects);
    let mut landings = Map::with_capacity(jig.limits.core.authority.projects);
    if let Some(ats) = project_ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"projects"), index);
            let Some(mut object) = read::object(document, *at, &path, problems) else { continue };
            let project = read::number(&mut object, document, b"project", &path, problems);
            let ats =
                read::list(&mut object, document, b"rules", jig.limits.core.authority.requirements, &path, problems);
            let landing = project_landing(&mut object, document, &path, &limits.forge, forges, problems);
            read::finish(object, document, &path, problems);
            if let Some(project) = project
                && let Some(ats) = ats
            {
                let mut found = false;
                for configured in &jig.projects {
                    if configured.number == project {
                        found = true;
                    }
                }
                // domain/landing.md, 13.1: a turn can configure an adopted
                // project's restored identity; policy rules still name seeds.
                let restored_turn = match &landing {
                    Some(landing) => landing.by == engine::Way::Turn && ats.is_empty(),
                    None => false,
                };
                if !found && !restored_turn {
                    problems.push(path.member(b"project"), What::Jig(config::What::Refers));
                }
                if projects.contains_key(&project) {
                    problems.push(path.member(b"project"), What::Jig(config::What::Unique));
                }
                let rules = rules(document, ats, &path.member(b"rules"), jig, limits, Some(project), problems);
                projects.insert(project, rules).expect("bounded projects");
                if let Some(landing) = landing {
                    landings.insert(project, landing).expect("bounded projects");
                }
                order.push(project).expect("bounded projects");
            }
        }
    }
    Some((engine::LandingPolicy { deployment, projects, landings }, order.into_boxed()))
}

fn optional_list(
    object: &mut config::read::Object,
    document: &Document,
    name: &[u8],
    max: u32,
    path: &Path,
    problems: &mut Problems,
) -> Option<List<u32>> {
    let Some(at) = object.member(document, name) else { return Some(List::with_capacity(0)) };
    match config::read::list(document, at, max) {
        Ok(ats) => Some(ats),
        Err(what) => {
            problems.push(path.member(name), What::Jig(what));
            None
        }
    }
}
fn optional_word(
    object: &mut config::read::Object,
    document: &Document,
    name: &[u8],
    default: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    let Some(at) = object.member(document, name) else { return Some(default.into()) };
    match config::read::text(document, at, 1_048_576) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), What::Jig(what));
            None
        }
    }
}
fn checked_text(
    object: &mut config::read::Object,
    document: &Document,
    name: &[u8],
    max: u32,
    path: &Path,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    let at = read::required(object, document, name, path, problems)?;
    match config::read::text(document, at, max) {
        Ok(value) => {
            read::form(!value.is_empty(), path.member(name), problems);
            Some(value)
        }
        Err(what) => {
            problems.push(path.member(name), What::Jig(what));
            None
        }
    }
}
// Ignored paths are whole segments; no empty segment, dot or parent segment.
fn valid_ignored_path(value: &[u8]) -> bool {
    let mut length = 0_usize;
    let mut dots = true;
    for byte in value {
        if *byte == b'/' {
            if length == 0 || (dots && length <= 2) {
                return false;
            }
            length = 0;
            dots = true;
        } else {
            length = length.saturating_add(1);
            dots &= *byte == b'.';
        }
    }
    length > 0 && (!dots || length > 2)
}

struct Check {
    name: Box<[u8]>,
    workflow: Box<[u8]>,
    ignored: Box<[Box<[u8]>]>,
}
fn check(
    object: &mut config::read::Object,
    document: &Document,
    path: &Path,
    limits: &temper_engine_domain_forge::Limits,
    problems: &mut Problems,
) -> Option<Check> {
    let name = checked_text(
        object,
        document,
        b"name",
        u32::try_from(CHECK_NAME_BYTES).expect("check name bound"),
        path,
        problems,
    );
    let workflow = checked_text(
        object,
        document,
        b"workflow",
        u32::try_from(WORKFLOW_BYTES).expect("workflow bound"),
        path,
        problems,
    );
    let ats = optional_list(object, document, b"ignored", limits.ignored_paths, path, problems);
    let mut ignored = List::with_capacity(limits.ignored_paths);
    if let Some(ats) = ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"ignored"), index);
            match config::read::text(document, *at, limits.name_bytes) {
                Ok(value) => {
                    read::form(valid_ignored_path(&value), path, problems);
                    ignored.push(value).expect("bounded ignored paths");
                }
                Err(what) => problems.push(path, What::Jig(what)),
            }
        }
    }
    let name = name?;
    for byte in &name {
        if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
            read::form(false, path.member(b"name"), problems);
            break;
        }
    }
    let workflow = workflow?;
    read::form(!workflow.contains(&b'/'), path.member(b"workflow"), problems);
    Some(Check { name, workflow, ignored: ignored.into_boxed() })
}
fn repository_checks(
    document: &Document,
    at: u32,
    path: &Path,
    limits: &temper_engine_domain_forge::Limits,
    problems: &mut Problems,
) -> Option<engine::RepositoryChecks> {
    let mut object = read::object(document, at, path, problems)?;
    let forge = read::short(&mut object, document, b"forge", path, problems);
    let repository = read::number(&mut object, document, b"repository", path, problems);
    let gate_ats = optional_list(&mut object, document, b"gates", limits.gates, path, problems);
    read::finish(object, document, path, problems);
    let mut gates = List::with_capacity(limits.gates);
    if let Some(ats) = gate_ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"gates"), index);
            let Some(mut object) = read::object(document, *at, &path, problems) else { continue };
            let check = check(&mut object, document, &path, limits, problems);
            let blocking = read::boolean(&mut object, document, b"blocking", &path, problems);
            read::finish(object, document, &path, problems);
            if let Some(check) = check
                && let Some(blocking) = blocking
            {
                for previous in &gates {
                    let previous: &engine::GateCheck = previous;
                    if previous.name == check.name {
                        problems.push(path.member(b"name"), What::Jig(config::What::Unique));
                    }
                }
                gates
                    .push(engine::GateCheck {
                        name: check.name,
                        workflow: check.workflow,
                        ignored: check.ignored,
                        blocking,
                    })
                    .expect("bounded gates");
            }
        }
    }
    Some(engine::RepositoryChecks {
        repository: skein_forge::api::Repository { forge: forge?, repository: repository? },
        gates: gates.into_boxed(),
    })
}
fn project_landing(
    object: &mut config::read::Object,
    document: &Document,
    path: &Path,
    limits: &temper_engine_domain_forge::Limits,
    forges: &[super::Forge],
    problems: &mut Problems,
) -> Option<engine::ProjectLanding> {
    let by = optional_word(object, document, b"by", b"queue", path, problems)?;
    let by = match by.as_ref() {
        b"queue" => engine::Way::Queue,
        b"turn" => engine::Way::Turn,
        _ => {
            read::form(false, path.member(b"by"), problems);
            return None;
        }
    };
    let ats = optional_list(object, document, b"gates", limits.repositories, path, problems)?;
    let mut repositories = List::with_capacity(limits.repositories);
    for (index, at) in ats.iter().enumerate() {
        let path = read::index(&path.member(b"gates"), index);
        if let Some(checks) = repository_checks(document, *at, &path, limits, problems) {
            let mut found = false;
            for forge in forges {
                if forge.number == checks.repository.forge {
                    found = true;
                    read::form(
                        by != engine::Way::Turn || forge.kind != crate::settings::Kind::GitHub,
                        path.member(b"repository"),
                        problems,
                    );
                }
            }
            if !found {
                problems.push(path.member(b"forge"), What::Jig(config::What::Refers));
            }
            for previous in &repositories {
                let previous: &engine::RepositoryChecks = previous;
                if previous.repository == checks.repository {
                    problems.push(path.member(b"repository"), What::Jig(config::What::Unique));
                }
            }
            repositories.push(checks).expect("bounded repositories");
        }
    }
    Some(engine::ProjectLanding { by, repositories: repositories.into_boxed() })
}
