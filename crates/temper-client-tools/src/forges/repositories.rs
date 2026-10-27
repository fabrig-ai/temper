//! The picker's one fresh page (client/tools.md, 5.5).
use super::{Forges, REPOSITORIES, domain, finish};
use crate::{
    jig::{
        self,
        call::number,
        render::{self, Text},
        runner::{Called, Out},
    },
    root::Limits,
};
use alloc::boxed::Box;
use jig_client::{Answered, Ask, Event, Family, Key, Outcome, Told, contract as c};
use skein_json::writer::Encoder;
use skein_lib::{Env, Queue};
use temper_client_domain::{
    forge::family::{self, Page},
    owner::Owner,
};
use temper_client_protocol_wire::{self as wire, v1 as w};

#[derive(Debug)]
pub(super) struct Pending {
    forge: u16,
    installation: Option<u64>,
    page: u32,
    called: Option<Called>,
}
pub(super) fn owns(call: &jig::call::Call) -> bool {
    crate::table::TOOLS.get(usize::from(call.tool)).expect("parsed tool").name == REPOSITORIES.name
}
fn request(forge: u16, installation: Option<u64>, page: u32, limits: &Limits) -> Option<Ask> {
    let bounds = temper_client_domain::forge::wire_limits(&limits.domain.forge)?;
    let request =
        w::Request::Reachable(w::Reachable::new(&bounds, w::ReachableParts { forge, installation, page }).ok()?);
    let request = w::Requesting::new(&bounds, w::RequestingParts { request }).ok()?;
    Some(Ask::Family {
        scope: c::Scope::Deployment,
        record: Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes: wire::requesting(&request) },
    })
}
pub(super) fn send(state: &mut Forges, env: &Env<Limits>, called: Called, out: &mut Queue<Out>) {
    if called.value.is_some() || jig::argv::check(crate::table::TOOLS, &called.call, &env.limits.tools).is_err() {
        let printed = render::failure(b"invalid repositories call", called.json, 1);
        finish(called, printed, out);
        return;
    }
    let forge = u16::try_from(number(&called.call, 0).expect("checked forge")).expect("checked forge");
    let installation = number(&called.call, 1);
    let page = u32::try_from(number(&called.call, 2).unwrap_or(1)).expect("checked page");
    let Some(request) = request(forge, installation, page, &env.limits) else {
        let printed = render::failure(b"repositories encoding failed", called.json, 1);
        finish(called, printed, out);
        return;
    };
    let key = Key { asker: Owner::Forge.asker(), number: (u64::from(called.call_number) << 32u32) | 6 };
    state.pages.insert(key, Pending { forge, installation, page, called: Some(called) });
    out.push(Out::Client { event: Event::Send { key, name: None, request } });
}
pub(super) fn told(state: &mut Forges, env: &Env<Limits>, told: &Told, out: &mut Queue<Out>) -> bool {
    match told {
        Told::Kept { key, request: Ask::Family { scope: c::Scope::Deployment, record } }
            if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION =>
        {
            let Some(bounds) = temper_client_domain::forge::wire_limits(&env.limits.domain.forge) else { return false };
            let Some(request) = wire::decode_requesting(&record.bytes, &bounds) else { return false };
            let w::Request::Reachable(request) = request.into_parts().request else { return false };
            let request = request.into_parts();
            state.pages.insert(
                *key,
                Pending { forge: request.forge, installation: request.installation, page: request.page, called: None },
            );
            true
        }
        Told::Answered { key, outcome } => {
            let Some(pending) = state.pages.remove(key) else { return false };
            let json = match &pending.called {
                Some(called) => called.json,
                None => state.json,
            };
            let printed = print(&pending, outcome, json, &env.limits);
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
fn text(pending: &Pending, page: &Page, out: &mut Text) {
    for row in &page.rows {
        out.put(&row.owner.login);
        out.put(b"/");
        out.put(&row.name);
        if row.private {
            out.put(b" private");
        }
        out.put(b" ");
        out.number(row.id);
        out.put(b"/");
        out.number(row.owner.id);
        out.put(b"\n");
    }
    if page.more {
        if let Some(next) = pending.page.checked_add(1) {
            out.put(b"temper ctl DIR forge repositories ");
            out.number(u64::from(pending.forge));
            if let Some(installation) = pending.installation {
                out.put(b" --installation ");
                out.number(installation);
            }
            out.put(b" --page ");
            out.number(u64::from(next));
            out.put(b"\n");
        } else {
            out.put(b"More follow; the page number limit was reached.\n");
        }
    } else {
        out.put(b"No more repositories follow.\n");
    }
}
fn json(page: &Page, out: &mut Encoder) {
    out.object_start();
    out.key(b"rows");
    out.array_start();
    for row in &page.rows {
        out.object_start();
        out.key(b"id");
        out.number(skein_lib::Decimal::of(row.id).as_bytes());
        out.key(b"owner");
        out.object_start();
        out.key(b"id");
        out.number(skein_lib::Decimal::of(row.owner.id).as_bytes());
        out.key(b"login");
        out.string(&row.owner.login);
        out.object_end();
        out.key(b"name");
        out.string(&row.name);
        out.key(b"private");
        out.boolean(row.private);
        out.object_end();
    }
    out.array_end();
    out.key(b"more");
    out.boolean(page.more);
    out.object_end();
}
fn print(pending: &Pending, outcome: &Outcome, structured: bool, limits: &Limits) -> jig::Printed {
    let value = match outcome {
        Outcome::Answered(Answered::Family(record))
            if record.family == wire::FORGE && record.version == wire::CLIENT_VERSION =>
        {
            let Some(bounds) = temper_client_domain::forge::wire_limits(&limits.domain.forge) else {
                return render::failure(b"repositories decoding failed", structured, 1);
            };
            let mut reader = skein_lib::Reader::new(&record.bytes);
            let Ok(value) = w::Answer::decode(&bounds, &mut reader) else {
                return render::failure(b"repositories decoding failed", structured, 1);
            };
            if !reader.is_empty() {
                return render::failure(b"repositories decoding failed", structured, 1);
            }
            value.into_parts().outcome
        }
        Outcome::Answered(Answered::Jig(c::Answer::Refused { refusal, .. })) => {
            let mut measure = Text::measure();
            let phrases = render::document::phrasebook(limits.tools.text);
            render::acts::refusal(refusal, &phrases, &mut measure);
            let mut out = Text::writing_exact(measure);
            render::acts::refusal(refusal, &phrases, &mut out);
            return render::failure(&out.finish(), structured, 2);
        }
        Outcome::Unknown => w::Outcome::Unknown,
        Outcome::Answered(_) => return render::failure(b"repositories decoding failed", structured, 1),
        Outcome::Held | Outcome::Limit | Outcome::Unconfirmed | Outcome::NotKept | Outcome::Unsent | Outcome::Lost => {
            let mut operation = render::outcome::Operation::of(
                &request(pending.forge, pending.installation, pending.page, limits).expect("checked bounds"),
            );
            operation.word = b"repositories";
            return render::outcome::print(&operation, outcome, structured, None, &limits.tools);
        }
    };
    let page = match value {
        w::Outcome::Page(page) => family::page(page),
        w::Outcome::ConnectRefused(why) => return render::failure(domain::connect_refusal(why.why()).1, structured, 2),
        w::Outcome::ForgeRefused(why) => {
            let why = match why.into_parts().reason {
                w::ForgeReason::NoSuchForge => b"no such forge".as_slice(),
                w::ForgeReason::NotFound => b"not found",
                w::ForgeReason::NoAccess => b"no access",
                w::ForgeReason::Refused => b"refused",
                w::ForgeReason::InFlight => b"in flight",
                w::ForgeReason::TooLarge => b"too large",
                w::ForgeReason::Unreachable => b"unreachable",
                w::ForgeReason::AnotherProject
                | w::ForgeReason::HomeHeld
                | w::ForgeReason::PrefixOccupied
                | w::ForgeReason::InvalidAnswer => b"invalid answer",
            };
            return render::failure(why, structured, 2);
        }
        w::Outcome::Unknown => return render::failure(b"unknown", structured, 1),
        w::Outcome::Accepted(_)
        | w::Outcome::Adopted(_)
        | w::Outcome::CoreRefused(_)
        | w::Outcome::Returning
        | w::Outcome::Connecting
        | w::Outcome::Link(_)
        | w::Outcome::SettingUp
        | w::Outcome::SetUpRefused(_)
        | w::Outcome::Running
        | w::Outcome::StartRefused(_)
        | w::Outcome::StartFailed(_) => return render::failure(b"repositories decoding failed", structured, 1),
    };
    let bytes = if structured {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        json(&page, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut out = Encoder::write(size, &bounds);
        json(&page, &mut out);
        out.finish()
    } else {
        let mut measure = Text::measure();
        text(pending, &page, &mut measure);
        if measure.structural() > usize::try_from(limits.tools.text).expect("text bound") {
            return render::failure(b"result limit", false, 1);
        }
        let mut out = Text::writing_exact(measure);
        text(pending, &page, &mut out);
        out.finish()
    };
    jig::Printed { file: jig::FileEnd::Keep, status: 0, stdout: bytes, stderr: Box::default() }
}
