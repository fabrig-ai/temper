//! Draft task controls through the client domain's amendment editor (client/tools.md, 5.5).
use crate::jig::{
    Limits,
    call::{Call, Value, number, value},
    runner::acts::{Failed, Prepared, Request, choice, words},
};
use alloc::boxed::Box;
use jig_client::contract as c;
use skein_lib::{List, Writer};

#[expect(clippy::manual_map, reason = "step code keeps the optional shown revision explicit")]
pub(crate) fn prepare(call: &Call, task: &c::Task, me: &c::Me, limits: &Limits) -> Prepared {
    let mut draft = temper_client_domain::amend::Draft::of(task);
    let shown = match number(call, 1) {
        Some(value) => Some(c::Revision { value }),
        None => None,
    };
    if let Some(shown) = shown
        && draft.revision() != shown
    {
        return Err(amend_error(
            task,
            b" moved: its spec revision is ",
            Some((draft.revision().value, shown.value)),
            b"",
        ));
    }
    controls(&mut draft, call, task, me, limits)?;
    let Some(drafted) = draft.amendment() else {
        return Err(amend_error(task, b"", None, b"amend changes nothing on task "));
    };
    if drafted.revision.is_some() && shown.is_none() {
        return Err(Failed::Fixed(b"task amend needs --rev REV"));
    }
    let revision = shown.or(drafted.revision);
    let change = Box::new(drafted.change);
    let mut admission = None;
    for offer in &task.acts {
        if offer.act == c::ActKind::Amend {
            admission = Some(c::admission::amendment(&offer.admission, &change, &task.authority));
        }
    }
    let reason = words(call, 12, limits)?;
    let (ask, to) = match admission {
        Some(c::Admission::Allowed) => {
            (c::Ask::Amend { project: task.project, task: task.chip.task, revision, change, reason }, None)
        }
        Some(c::Admission::Proposed { to }) => (
            c::Ask::Propose {
                project: task.project,
                action: c::ProposeAction::Amendment {
                    task: task.chip.task,
                    revision: revision.unwrap_or(draft.revision()),
                    change,
                },
                reason,
            },
            Some(to),
        ),
        Some(c::Admission::Bounded { .. }) => return Err(Failed::Fixed(b"amendment has no bounded draft")),
        Some(c::Admission::Refused(_)) | None => return Err(Failed::Fixed(b"amendment not offered")),
    };
    Ok((Request::Ask(ask), to))
}

fn amend_error(task: &c::Task, suffix: &[u8], moved: Option<(u64, u64)>, prefix: &[u8]) -> Failed {
    let prefix = if prefix.is_empty() { b"task ".as_slice() } else { prefix };
    let task_number = skein_lib::Decimal::of(task.chip.task);
    let mut length = prefix
        .len()
        .checked_add(task_number.as_bytes().len())
        .expect("error length")
        .checked_add(suffix.len())
        .expect("error length");
    if let Some((now, shown)) = moved {
        length = length
            .checked_add(skein_lib::Decimal::of(now).as_bytes().len())
            .expect("revision length")
            .checked_add(b", not ".len())
            .expect("separator length")
            .checked_add(skein_lib::Decimal::of(shown).as_bytes().len())
            .expect("shown revision length")
            .checked_add(b"; read again: task ".len())
            .expect("retry length")
            .checked_add(task_number.as_bytes().len())
            .expect("task length");
    }
    let mut out = Writer::new(length);
    out.put(prefix).expect("error prefix");
    out.put(skein_lib::Decimal::of(task.chip.task).as_bytes()).expect("task number");
    out.put(suffix).expect("error suffix");
    if let Some((now, shown)) = moved {
        out.put(skein_lib::Decimal::of(now).as_bytes()).expect("revision");
        out.put(b", not ").expect("separator");
        out.put(skein_lib::Decimal::of(shown).as_bytes()).expect("shown revision");
        out.put(b"; read again: task ").expect("retry");
        out.put(skein_lib::Decimal::of(task.chip.task).as_bytes()).expect("task number");
    }
    Failed::Detail(out.finish())
}

fn controls(
    draft: &mut temper_client_domain::amend::Draft,
    call: &Call,
    task: &c::Task,
    me: &c::Me,
    limits: &Limits,
) -> Result<(), Failed> {
    if let Some(priority) = number(call, 2) {
        draft.track(c::Tracking::Tracked { priority: u32::try_from(priority).expect("checked priority") });
    }
    if value(call, 3).is_some() {
        draft.track(c::Tracking::Untracked);
    }
    if let Some(Value::Text(text)) = value(call, 4) {
        whole(draft.deadline(Some(crate::jig::argv::deadline(text).expect("checked deadline"))), task)?;
    } else if value(call, 5).is_some() {
        whole(draft.deadline(None), task)?;
    }
    if value(call, 6).is_some() {
        draft.instructions(words(call, 6, limits)?);
    }
    if value(call, 7).is_some() {
        whole(draft.words(words(call, 7, limits)?), task)?;
    }
    if let Some(Some(crate::jig::call::Values::Many(values))) = call.values.get(8) {
        let mut dependencies = List::with_capacity(u32::try_from(values.len()).expect("parsed dependencies"));
        for value in values {
            let Value::Number(number) = value else { return Err(Failed::Fixed(b"dependency not loaded")) };
            dependencies.push(*number).expect("same dependency count");
        }
        draft.dependencies(dependencies);
    } else if value(call, 9).is_some() {
        draft.dependencies(List::with_capacity(0));
    }
    if value(call, 10).is_some() {
        draft.llm(choice::llm(call, 10, 11, me, task.project, task.llm.as_ref())?.expect("model supplied"));
    }
    Ok(())
}
fn whole(result: Result<(), temper_client_domain::amend::Cut>, task: &c::Task) -> Result<(), Failed> {
    use temper_client_domain::amend::Cut;
    let suffix = match result {
        Ok(()) => return Ok(()),
        Err(Cut::Authority) => {
            b"'s authority is cut in this read; amend its deadline from a client with wider bounds".as_slice()
        }
        Err(Cut::Spec) => b"'s spec is cut in this read; amend its words from a client with wider bounds".as_slice(),
    };
    Err(amend_error(task, suffix, None, b""))
}
