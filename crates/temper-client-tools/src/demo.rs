//! The playground's start answer (client/tools.md, 5.5, area 6).
use crate::{
    jig::{
        self,
        render::{self, Text},
    },
    root::Limits,
};
use alloc::boxed::Box;
use jig_client::Outcome;
use skein_json::writer::Encoder;
use temper_client_domain::forge::{
    demo::{self, StartAnswer, StartFailure},
    words,
};

fn text(answer: &StartAnswer, kept: bool, limits: &Limits, out: &mut Text) {
    if kept {
        out.put(b"kept: demo-start ");
    }
    match answer {
        StartAnswer::EntranceRefused(why) => {
            render::acts::refusal(why.refusal(), &render::document::phrasebook(limits.tools.text), out);
            out.put(b"\n");
        }
        StartAnswer::Running => out.put(b"running\n"),
        StartAnswer::NotOffered => out.put(b"refused: not offered\n"),
        StartAnswer::Unknown => out.put(b"unknown\n"),
        StartAnswer::Failed(why) => {
            match why {
                StartFailure::Group(why) => {
                    out.put(b"the fakes did not start (");
                    out.put(&words::lossy(why));
                    out.put(b")");
                }
                StartFailure::Account => out.put(b"the account refused"),
                StartFailure::Adoption => out.put(b"the adoption refused"),
                StartFailure::NotConnected => out.put(b"the forge not connected in time"),
            }
            out.put(b"\n");
        }
    }
}
fn json(answer: &StartAnswer, kept: bool, out: &mut Encoder) {
    if kept {
        out.object_start();
        out.key(b"operation");
        out.string(b"demo-start");
        out.key(b"outcome");
    }
    out.object_start();
    out.key(match answer {
        StartAnswer::EntranceRefused(_) => b"entrance_refused",
        StartAnswer::Running => b"running",
        StartAnswer::NotOffered => b"start_refused",
        StartAnswer::Failed(_) => b"start_failed",
        StartAnswer::Unknown => b"unknown",
    });
    out.object_start();
    match answer {
        StartAnswer::EntranceRefused(why) => {
            out.key(b"refusal");
            render::document::refusal_json(why.refusal(), out);
        }
        StartAnswer::NotOffered => {
            out.key(b"why");
            out.string(b"not_offered");
        }
        StartAnswer::Failed(why) => {
            out.key(b"why");
            match why {
                StartFailure::Group(why) => {
                    out.object_start();
                    out.key(b"group");
                    out.object_start();
                    out.key(b"why");
                    out.string(&words::lossy(why));
                    out.object_end();
                    out.object_end();
                }
                StartFailure::Account => out.string(b"account"),
                StartFailure::Adoption => out.string(b"adoption"),
                StartFailure::NotConnected => out.string(b"not_connected"),
            }
        }
        StartAnswer::Running | StartAnswer::Unknown => {}
    }
    out.object_end();
    out.object_end();
    if kept {
        out.object_end();
    }
}
pub(crate) fn print(outcome: &Outcome, as_json: bool, kept: bool, limits: &Limits) -> jig::Printed {
    let answer = match outcome {
        Outcome::Answered(answer) => demo::answer(answer, &limits.domain.forge),
        Outcome::Unknown => Some(StartAnswer::Unknown),
        Outcome::Held
        | Outcome::Limit
        | Outcome::Full { .. }
        | Outcome::Unconfirmed
        | Outcome::NotKept
        | Outcome::Unsent
        | Outcome::Lost => {
            let mut operation =
                render::outcome::Operation::of(&demo::ask(&limits.domain.forge).expect("checked limits"));
            operation.word = b"demo-start";
            if kept {
                return jig::Printed {
                    file: jig::FileEnd::Keep,
                    status: 1,
                    stdout: Box::default(),
                    stderr: render::outcome::kept(&operation, outcome, as_json, None, &limits.tools),
                };
            }
            return render::outcome::print(&operation, outcome, as_json, None, &limits.tools);
        }
    };
    let Some(answer) = answer else { return render::failure(b"demo start answer decoding failed", as_json, 1) };
    let status = match answer {
        StartAnswer::Running => 0,
        StartAnswer::NotOffered | StartAnswer::EntranceRefused(_) => 2,
        StartAnswer::Failed(_) | StartAnswer::Unknown => 1,
    };
    let bytes = if as_json {
        let bounds = skein_json::writer::Limits { depth: 8, length: limits.tools.text };
        let mut measure = Encoder::measure(&bounds);
        json(&answer, kept, &mut measure);
        let Ok(size) = measure.measured() else { return render::failure(b"result limit", true, 1) };
        let mut out = Encoder::write(size, &bounds);
        json(&answer, kept, &mut out);
        let bytes = out.finish();
        if kept { render::outcome::newline(&bytes) } else { bytes }
    } else {
        let mut measure = Text::measure();
        text(&answer, kept, limits, &mut measure);
        let mut out = Text::writing_exact(measure);
        text(&answer, kept, limits, &mut out);
        out.finish()
    };
    if bytes.len() > usize::try_from(limits.tools.text).expect("text fits") {
        return render::failure(b"result limit", as_json, 1);
    }
    if status == 0 {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: bytes, stderr: Box::default() }
    } else {
        jig::Printed { file: jig::FileEnd::Keep, status, stdout: Box::default(), stderr: bytes }
    }
}
