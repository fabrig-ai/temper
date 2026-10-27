//! The move's file is the shell's; only a durable write permits written (client/tools.md, 5.5).
use super::{Called, FileEnd, Out, Runner, acts, finish};
use crate::jig::{Limits, render};
use alloc::boxed::Box;
use jig_client::{Answered, Client, Outcome, contract as c};
use skein_lib::{Env, Queue};
#[derive(Debug)]
pub(super) struct Recording {
    called: Called,
    holder: c::AccountHolder,
    account: u32,
    number: u32,
}
pub(super) fn answered(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    sent: acts::Sent,
    outcome: &Outcome,
    out: &mut Queue<Out>,
) {
    if let Some(holder) = sent.holder {
        if let Outcome::Answered(Answered::Jig(c::Answer::Done(c::Outcome::Made {
            what: c::Made::MovingOut { account, number, record },
        }))) = outcome
        {
            out.push(Out::Record { number: sent.called.number, record: record.clone() });
            runner.recording.insert(
                sent.called.number,
                Recording { called: sent.called, holder, account: *account, number: *number },
            );
            return;
        }
        let printed = if let Outcome::Lost = outcome {
            render::failure(b"the record of the move was lost on its way; the engine keeps the account and cancels the move at its deadline; account export again after it", sent.called.json, 1)
        } else {
            render::outcome::print(&sent.operation, outcome, sent.called.json, acts::me(runner, client), &env.limits)
        };
        finish(sent.called, printed, out);
    } else {
        let mut printed =
            render::outcome::print(&sent.operation, outcome, sent.called.json, acts::me(runner, client), &env.limits);
        printed.file = if let Outcome::Answered(Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::AccountMove { why: c::AccountMoveWhy::Cancelled },
            ..
        })) = outcome
        {
            FileEnd::Remove
        } else if sent.dispatched || printed.status == 0 {
            FileEnd::Keep
        } else {
            FileEnd::Remove
        };
        out.push(Out::Printed { number: sent.called.number, printed });
    }
}
pub(super) fn recorded(
    runner: &mut Runner,
    client: &Client,
    env: &Env<Limits>,
    number: u32,
    result: Result<(), Box<[u8]>>,
    out: &mut Queue<Out>,
) {
    let Some(mut recording) = runner.recording.remove(&number) else { return };
    if let Err(why) = result {
        let Some(crate::jig::call::Value::File(file)) = crate::jig::call::value(&recording.called.call, 1) else {
            unreachable!("export file")
        };
        let parts: [&[u8]; 4] =
            [file, b" not written: ", &why, b"; the engine keeps the account and cancels the move at its deadline"];
        let mut measured = render::Text::measure();
        for part in parts {
            measured.put(part);
        }
        let mut write = render::Text::writing_exact(measured);
        for part in parts {
            write.put(part);
        }
        let printed = render::failure(&write.finish(), recording.called.json, 1);
        finish(recording.called, printed, out);
        return;
    }
    // The acknowledgment is a distinct durable request with a fresh drawn name.
    recording.called.call.key = None;
    acts::prepared(
        runner,
        client,
        env,
        recording.called,
        Ok((
            acts::Request::Written { holder: recording.holder, account: recording.account, number: recording.number },
            None,
        )),
        out,
    );
}
