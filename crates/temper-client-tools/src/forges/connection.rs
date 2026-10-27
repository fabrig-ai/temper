//! One-shot pasted connections and durable installation requests (client/tools.md, 5.5).
use super::{ACCEPT, CONNECT, Forges, INSTALL, domain, finish, number_arg};
use crate::{
    jig::{
        self,
        call::{Project, Value, number, value},
        render::{self, Text},
        runner::{Called, Out},
    },
    root::Limits,
};
use alloc::boxed::Box;
use jig_client::{Answered, Ask, Family, contract as c};
use jig_client::{Event, Key, Outcome, Told};
use skein_json::writer::Encoder;
use skein_lib::{Env, Queue};
use temper_client_domain::owner::Owner;
use temper_client_protocol_wire::{self as wire, v1 as w};

#[derive(Debug)]
pub(super) struct Pending {
    forge: u16,
    install: bool,
    installation: Option<u64>,
    manifest: bool,
    first: Option<super::Intent>,
    called: Option<Called>,
}
pub(super) fn owns(call: &jig::call::Call) -> bool {
    let name = crate::table::TOOLS.get(usize::from(call.tool)).expect("parsed tool").name;
    name == CONNECT.name || name == INSTALL.name || name == ACCEPT.name
}
fn request(
    forge: u16,
    app: Option<u64>,
    installation: Option<u64>,
    manifest: bool,
    admin: Option<w::ForgejoAdmin>,
    limits: &Limits,
) -> Option<Ask> {
    let bounds = temper_client_domain::forge::wire_limits(&limits.domain.forge)?;
    let request = if let Some(installation) = installation {
        w::Request::Accept(w::Accept::new(&bounds, w::AcceptParts { forge, installation }).ok()?)
    } else if let Some(admin) = admin {
        w::Request::Connect(w::Connect::new(&bounds, w::ConnectParts { forge, how: w::How::ForgejoAdmin(admin) }).ok()?)
    } else if manifest {
        w::Request::Connect(
            w::Connect::new(
                &bounds,
                w::ConnectParts {
                    forge,
                    how: w::How::GithubManifest(
                        w::GitHubManifest::new(&bounds, w::GitHubManifestParts { back: None }).ok()?,
                    ),
                },
            )
            .ok()?,
        )
    } else {
        match app {
            Some(id) => w::Request::Connect(
                w::Connect::new(
                    &bounds,
                    w::ConnectParts {
                        forge,
                        how: w::How::GithubApp(w::GitHubApp::new(&bounds, w::GitHubAppParts { id }).ok()?),
                    },
                )
                .ok()?,
            ),
            None => w::Request::Install(w::Install::new(&bounds, w::InstallParts { forge, back: None }).ok()?),
        }
    };
    let record = w::Requesting::new(&bounds, w::RequestingParts { request }).ok()?;
    Some(Ask::Family {
        scope: c::Scope::Deployment,
        record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&record) },
    })
}
// The bundle's item is AdminToken (1), not AppKey (11). Preserve every byte.
fn admin_token(token: &[u8]) -> Option<skein_lib::Secret> {
    temper_client_domain::forge::forges::pasted_admin_token(token)
}
fn admin_how(
    call: &jig::call::Call,
    client: &jig_client::Client,
    limits: &Limits,
) -> Result<w::ForgejoAdmin, &'static [u8]> {
    let invalid = b"invalid first repository, project or runner label".as_slice();
    let bounds = temper_client_domain::forge::wire_limits(&limits.domain.forge).ok_or(invalid)?;
    let first = match value(call, 3) {
        None => {
            if value(call, 4).is_some() || value(call, 5).is_some() {
                return Err(b"--project and --label require --first");
            }
            None
        }
        Some(Value::Text(name)) => {
            let mut split = None;
            for (at, byte) in name.iter().enumerate() {
                if *byte == b'/' {
                    split = Some(at);
                    break;
                }
            }
            let split = split.ok_or(invalid)?;
            let org = name.get(..split).ok_or(invalid)?;
            let repository = name.get(split..).ok_or(invalid)?.strip_prefix(b"/").ok_or(invalid)?;
            if org.is_empty() || repository.is_empty() || repository.contains(&b'/') {
                return Err(invalid);
            }
            let label = match value(call, 5) {
                Some(Value::Text(label)) => label.as_ref(),
                _ => b"docker",
            };
            if label.is_empty() || label.len() > 64 {
                return Err(invalid);
            }
            for byte in label {
                if !byte.is_ascii_alphanumeric() && !b"._-".contains(byte) {
                    return Err(invalid);
                }
            }
            let project = match value(call, 4) {
                Some(Value::Project(Project::Number(project))) => w::SetUpProject::Existing(
                    w::ExistingProject::new(&bounds, w::ExistingProjectParts { project: *project })
                        .ok()
                        .ok_or(invalid)?,
                ),
                Some(Value::Project(Project::Name(name))) => {
                    let mut found = None;
                    if let Some(me) = crate::forge::me(client) {
                        for project in &me.projects {
                            if project.name.bytes() == name.as_ref() {
                                found = Some(project.project);
                                break;
                            }
                        }
                    }
                    match found {
                        Some(project) => w::SetUpProject::Existing(
                            w::ExistingProject::new(&bounds, w::ExistingProjectParts { project })
                                .ok()
                                .ok_or(invalid)?,
                        ),
                        None => w::SetUpProject::New(
                            w::NewProject::new(&bounds, w::NewProjectParts { name: name.clone() })
                                .ok()
                                .ok_or(invalid)?,
                        ),
                    }
                }
                _ => w::SetUpProject::New(
                    w::NewProject::new(&bounds, w::NewProjectParts { name: repository.into() }).ok().ok_or(invalid)?,
                ),
            };
            Some(
                w::FirstAsk::new(
                    &bounds,
                    w::FirstAskParts { org: org.into(), repository: repository.into(), project, label: label.into() },
                )
                .ok()
                .ok_or(invalid)?,
            )
        }
        _ => return Err(invalid),
    };
    w::ForgejoAdmin::new(&bounds, w::ForgejoAdminParts { first }).ok().ok_or(invalid)
}
#[expect(clippy::manual_map, reason = "the domain matches without closures")]
pub(super) fn send(
    state: &mut Forges,
    client: &jig_client::Client,
    env: &Env<Limits>,
    mut called: Called,
    out: &mut Queue<Out>,
) {
    if jig::argv::check(crate::table::TOOLS, &called.call, &env.limits.tools).is_err() {
        let printed = render::failure(b"invalid forge call", called.json, 1);
        finish(called, printed, out);
        return;
    }
    if crate::table::TOOLS.get(usize::from(called.call.tool)).expect("parsed tool").name == CONNECT.name
        && called.call.form == 2
    {
        let printed =
            render::failure(b"forgejo is not offered yet; use forge connect FORGE github --key KEY", called.json, 2);
        finish(called, printed, out);
        return;
    }
    let tool = crate::table::TOOLS.get(usize::from(called.call.tool)).expect("parsed tool").name;
    let installation = if tool == ACCEPT.name { number(&called.call, 1) } else { None };
    let install = tool == INSTALL.name || installation.is_some();
    let manifest = !install && called.call.form == 1;
    let forge = u16::try_from(number_arg(&called.call)).expect("checked forge number");
    let app = if install { None } else { number(&called.call, 2) };
    let admin = !install && called.call.form == 3;
    let first = admin && value(&called.call, 3).is_some();
    let admin_how = if admin {
        match admin_how(&called.call, client, &env.limits) {
            Ok(how) => Some(how),
            Err(why) => {
                let printed = render::failure(why, called.json, 1);
                finish(called, printed, out);
                return;
            }
        }
    } else {
        None
    };
    let first = if first {
        let first = admin_how.as_ref().expect("admin request").first().as_ref().expect("first repository");
        let project = match first.project() {
            w::SetUpProject::Existing(project) => domain::SetUpProject::Existing(project.project()),
            w::SetUpProject::New(project) => domain::SetUpProject::New(project.name().into()),
        };
        Some(super::Intent {
            playground: false,
            forge,
            org: first.org().into(),
            repository: first.repository().into(),
            project,
        })
    } else {
        None
    };
    let value = called.value.take();
    if value.is_some() != (!install && !manifest) {
        let printed =
            render::failure(b"forge connect requires a private key file; forge install takes no value", called.json, 1);
        finish(called, printed, out);
        return;
    }
    let Some(request) = request(forge, app, installation, manifest, admin_how, &env.limits) else {
        let printed = render::failure(b"forge request encoding failed", called.json, 1);
        finish(called, printed, out);
        return;
    };
    let key = Key { asker: Owner::Forge.asker(), number: (u64::from(called.call_number) << 32u32) | 5 };
    let name = match &called.call.key {
        Some(name) => Some(crate::key::engine(name)),
        None => None,
    };
    let event = match value {
        Some(pem) => {
            let bundled = if admin { admin_token(pem.expose()) } else { domain::pasted_app_key(pem.expose()) };
            let Some(value) = bundled else {
                let printed = render::failure(b"credential file must contain 1 to 4089 bytes", called.json, 1);
                finish(called, printed, out);
                return;
            };
            Event::Carry { key, name: name.expect("connect requires a key"), request, value: Some(value) }
        }
        None => Event::Send { key, name, request },
    };
    state.connections.insert(key, Pending { forge, install, installation, manifest, first, called: Some(called) });
    out.push(Out::Client { event });
}
pub(super) fn told(state: &mut Forges, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) -> bool {
    match told {
        Told::Kept { key, request: Ask::Family { scope: c::Scope::Deployment, record } }
            if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION =>
        {
            let Some(bounds) = temper_client_domain::forge::wire_limits(&env.limits.domain.forge) else { return false };
            let Some(request) = wire::decode_requesting(&record.bytes, &bounds) else { return false };
            let (forge, install, installation, manifest) = match request.into_parts().request {
                w::Request::Accept(value) => (value.forge(), true, Some(value.installation()), false),
                w::Request::Install(value) => (value.forge(), true, None, false),
                w::Request::Connect(value) => match value.how() {
                    w::How::GithubManifest(_) => (value.forge(), false, None, true),
                    w::How::GithubApp(_) | w::How::ForgejoAdmin(_) => return false,
                },
                w::Request::Reachable(_)
                | w::Request::Return(_)
                | w::Request::Adopt(_)
                | w::Request::SetUp(_)
                | w::Request::StartDemo => {
                    return false;
                }
            };
            state
                .connections
                .insert(*key, Pending { forge, install, installation, manifest, first: None, called: None });
            true
        }
        Told::Answered { key, outcome } => {
            let Some(mut pending) = state.connections.remove(key) else { return false };
            let json = match &pending.called {
                Some(called) => called.json,
                None => state.json,
            };
            let printed = print(&pending, outcome, json, &env.limits);
            if printed.status == 0
                && let Some(intent) = pending.first.take()
            {
                let called = pending.called.take().expect("one-shot connection");
                out.push(Out::Progress {
                    line: if called.json { render::outcome::newline(&printed.stdout) } else { printed.stdout },
                });
                super::watch_first(state, env, *key, called, intent, out);
                return true;
            }
            match pending.called {
                Some(called) => finish(called, printed, out),
                None => out.push(Out::Kept { line: if printed.status == 0 { printed.stdout } else { printed.stderr } }),
            }
            true
        }
        Told::Kept { .. }
        | Told::Link { .. }
        | Told::Dropped { .. }
        | Told::Pending { .. }
        | Told::Changed { .. }
        | Told::Ended { .. }
        | Told::Text { .. }
        | Told::Page { .. } => false,
    }
}
#[derive(Debug)]
enum Answer {
    Connecting,
    Accepted(u64),
    ForgeRefused(w::ForgeReason),
    Link(Box<[u8]>),
    Refused(w::ForgeRefusal),
    Entrance(c::EntranceRefusal),
    Unknown,
}
fn answer(value: &Answered, limits: &Limits) -> Option<Answer> {
    let record = match value {
        Answered::Family(record) => record,
        Answered::Jig(c::Answer::Refused { refusal, .. }) => {
            return Some(Answer::Entrance(c::EntranceRefusal::new(refusal.clone()).ok()?));
        }
        Answered::Jig(c::Answer::Done(_)) => return None,
    };
    if record.family != wire::FORGE || record.version != wire::CLIENT_VERSION {
        return None;
    }
    let bounds = temper_client_domain::forge::wire_limits(&limits.domain.forge)?;
    let mut reader = skein_lib::Reader::new(&record.bytes);
    let value = w::Answer::decode(&bounds, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    match value.into_parts().outcome {
        w::Outcome::Accepted(value) => Some(Answer::Accepted(value.installation())),
        w::Outcome::ForgeRefused(value) => Some(Answer::ForgeRefused(value.into_parts().reason)),
        w::Outcome::Connecting => Some(Answer::Connecting),
        w::Outcome::Link(value) => Some(Answer::Link(value.into_parts().address)),
        w::Outcome::ConnectRefused(value) => Some(Answer::Refused(value.into_parts().why)),
        w::Outcome::Unknown => Some(Answer::Unknown),
        w::Outcome::Page(_)
        | w::Outcome::Returning
        | w::Outcome::Adopted(_)
        | w::Outcome::CoreRefused(_)
        | w::Outcome::SettingUp
        | w::Outcome::SetUpRefused(_)
        | w::Outcome::Running
        | w::Outcome::StartRefused(_)
        | w::Outcome::StartFailed(_) => None,
    }
}
fn text(pending: &Pending, answer: &Answer, kept: bool, limits: &Limits, out: &mut Text) {
    let forge = pending.forge;
    let install = pending.install;
    let manifest = pending.manifest;
    if kept {
        out.put(if pending.installation.is_some() {
            b"kept: forge-accept "
        } else if install {
            b"kept: forge-install "
        } else {
            b"kept: forge-connect "
        });
    }
    match answer {
        Answer::Accepted(installation) => {
            out.put(b"accepted: installation ");
            out.number(*installation);
            out.put(b" on forge ");
            out.number(u64::from(forge));
            out.put(b"\ntemper ctl DIR forge repositories ");
            out.number(u64::from(forge));
            out.put(b" --installation ");
            out.number(*installation);
            out.put(b"\n");
        }
        Answer::ForgeRefused(why) => accept_refusal(pending, why, out),
        Answer::Connecting => {
            out.put(b"connecting\n");
            if !install && !manifest && pending.first.is_none() {
                out.put(b"temper ctl DIR forge list --until connected:");
                out.number(u64::from(forge));
                out.put(b"\n");
            }
        }
        Answer::Link(address) => {
            out.put(b"Open this address in a browser on the engine's machine:\n");
            out.put(address);
            out.put(b"\ntemper ctl DIR forge list --until settled\n");
        }
        Answer::Refused(why) => {
            out.put(domain::connect_refusal(why).1);
            out.put(b"\n");
        }
        Answer::Entrance(why) => {
            render::acts::refusal(why.refusal(), &render::document::phrasebook(limits.tools.text), out);
            out.put(b"\n");
        }
        Answer::Unknown => out.put(b"unknown\n"),
    }
}
fn json(answer: &Answer, kept: bool, pending: &Pending, out: &mut Encoder) {
    if kept {
        out.object_start();
        out.key(b"operation");
        out.string(if pending.installation.is_some() {
            b"forge-accept"
        } else if pending.install {
            b"forge-install"
        } else {
            b"forge-connect"
        });
        out.key(b"outcome");
    }
    out.object_start();
    match answer {
        Answer::Accepted(installation) => {
            out.key(b"forge");
            out.unsigned(u64::from(pending.forge));
            out.key(b"accepted");
            out.unsigned(*installation);
        }
        Answer::ForgeRefused(why) => {
            out.key(b"forge_refused");
            out.object_start();
            out.key(b"reason");
            out.string(forge_reason_name(why));
            out.object_end();
        }
        Answer::Connecting => {
            out.key(b"connecting");
            out.object_start();
            out.object_end();
        }
        Answer::Link(address) => {
            out.key(b"link");
            out.object_start();
            out.key(b"address");
            out.string(address);
            out.object_end();
        }
        Answer::Refused(why) => {
            out.key(b"refused");
            out.object_start();
            out.key(b"why");
            out.string(domain::connect_refusal(why).0);
            out.object_end();
        }
        Answer::Entrance(why) => {
            out.key(b"entrance_refused");
            out.object_start();
            out.key(b"refusal");
            render::document::refusal_json(why.refusal(), out);
            out.object_end();
        }
        Answer::Unknown => {
            out.key(b"unknown");
            out.object_start();
            out.object_end();
        }
    }
    out.object_end();
    if kept {
        out.object_end();
    }
}
fn print(pending: &Pending, outcome: &Outcome, structured: bool, limits: &Limits) -> jig::Printed {
    let forge = pending.forge;
    let install = pending.install;
    let kept = pending.called.is_none();
    let answer = match outcome {
        Outcome::Answered(value) => answer(value, limits),
        Outcome::Unknown => Some(Answer::Unknown),
        Outcome::Held | Outcome::Limit | Outcome::Unconfirmed | Outcome::NotKept | Outcome::Unsent | Outcome::Lost => {
            let mut operation = render::outcome::Operation::of(
                &request(forge, None, pending.installation, pending.manifest, None, limits)
                    .expect("checked forge limits"),
            );
            operation.word = if pending.installation.is_some() {
                b"forge-accept"
            } else if install {
                b"forge-install"
            } else {
                b"forge-connect"
            };
            if !install
                && !pending.manifest
                && let Some(called) = &pending.called
            {
                operation.value_name.clone_from(&called.call.key);
            }
            if kept {
                return jig::Printed {
                    file: jig::FileEnd::Keep,
                    status: 1,
                    stdout: Box::default(),
                    stderr: render::outcome::kept(&operation, outcome, structured, None, &limits.tools),
                };
            }
            return render::outcome::print(&operation, outcome, structured, None, &limits.tools);
        }
    };
    let Some(answer) = answer else { return render::failure(b"forge answer decoding failed", structured, 1) };
    let status = match &answer {
        Answer::Accepted(installation) if pending.installation != Some(*installation) => {
            return render::failure(b"forge answer does not fit accept", structured, 1);
        }
        Answer::ForgeRefused(_) if pending.installation.is_none() => {
            return render::failure(b"forge answer does not fit accept", structured, 1);
        }
        Answer::Connecting => u8::from(install || pending.manifest),
        Answer::Link(_) if pending.installation.is_some() || !install && !pending.manifest => {
            return render::failure(b"forge answer does not fit connect", structured, 1);
        }
        Answer::Accepted(_) | Answer::Link(_) => 0,
        Answer::ForgeRefused(_) | Answer::Refused(_) | Answer::Entrance(_) => 2,
        Answer::Unknown => 1,
    };
    let bytes = if structured {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        json(&answer, kept, pending, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut out = Encoder::write(size, &bounds);
        json(&answer, kept, pending, &mut out);
        let bytes = out.finish();
        if kept { render::outcome::newline(&bytes) } else { bytes }
    } else {
        let mut measure = Text::measure();
        text(pending, &answer, kept, limits, &mut measure);
        let mut out = Text::writing_exact(measure);
        text(pending, &answer, kept, limits, &mut out);
        out.finish()
    };
    if bytes.len() > usize::try_from(limits.tools.text).expect("text fits") {
        return render::failure(b"result limit", structured, 1);
    }
    if status == 0 {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: bytes, stderr: Box::default() }
    } else {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: Box::default(), stderr: bytes }
    }
}

fn accept_refusal(pending: &Pending, why: &w::ForgeReason, out: &mut Text) {
    match why {
        w::ForgeReason::NotFound | w::ForgeReason::NoAccess => {
            out.put(if *why == w::ForgeReason::NotFound {
                b"GitHub has no installation "
            } else {
                b"GitHub refused the App's read of installation "
            });
            out.number(pending.installation.expect("accept request"));
            out.put(if *why == w::ForgeReason::NotFound { b" of this App\n" } else { b"\n" });
        }
        w::ForgeReason::Unreachable => out.put(b"GitHub did not answer: try again\n"),
        // The refusal carries its reason alone, not the deployment's configured capacity.
        w::ForgeReason::TooLarge => out.put(b"temper's accepted installation set is full\n"),
        w::ForgeReason::AnotherProject
        | w::ForgeReason::HomeHeld
        | w::ForgeReason::PrefixOccupied
        | w::ForgeReason::InvalidAnswer => out.put(b"GitHub answered what temper did not expect\n"),
        w::ForgeReason::NoSuchForge => out.put(b"no such forge\n"),
        w::ForgeReason::Refused => out.put(b"refused\n"),
        w::ForgeReason::InFlight => out.put(b"in flight\n"),
    }
}

fn forge_reason_name(why: &w::ForgeReason) -> &'static [u8] {
    match why {
        w::ForgeReason::NoSuchForge => b"no_such_forge",
        w::ForgeReason::NotFound => b"not_found",
        w::ForgeReason::NoAccess => b"no_access",
        w::ForgeReason::Refused => b"refused",
        w::ForgeReason::InFlight => b"in_flight",
        w::ForgeReason::TooLarge => b"too_large",
        w::ForgeReason::Unreachable => b"unreachable",
        w::ForgeReason::InvalidAnswer => b"invalid_answer",
        w::ForgeReason::AnotherProject => b"another_project",
        w::ForgeReason::HomeHeld => b"home_held",
        w::ForgeReason::PrefixOccupied => b"prefix_occupied",
    }
}
