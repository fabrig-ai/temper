//! Question words keep the engine's independent cut marker.
use crate::jig::runner::reads::Reading;
use jig_client::contract as c;
use skein_json::writer::Encoder;
pub(super) fn encode(reading: &Reading, out: &mut Encoder) {
    let mut present = false;
    for question in reading.questions.values() {
        present |= question.is_some();
    }
    if !present {
        return;
    }
    out.key(b"questions");
    out.array_start();
    for question in reading.questions.values() {
        let Some(question) = question else { continue };
        out.object_start();
        out.key(b"task");
        out.unsigned(question.task.task);
        out.key(b"message");
        out.unsigned(question.message);
        out.key(b"words");
        out.object_start();
        out.key(b"bytes");
        out.string(question.words.bytes());
        out.key(b"cut");
        out.boolean(question.words.cut);
        out.object_end();
        if question.words.cut {
            out.key(b"read_whole");
            let task = asking_task(question);
            let mut measured = super::Text::measure();
            whole(reading.command_prefix, task, &mut measured);
            let mut bytes = super::Text::writing_exact(measured);
            whole(reading.command_prefix, task, &mut bytes);
            out.string(&bytes.finish());
        }
        out.object_end();
    }
    out.array_end();
}
pub(crate) fn asking_task(question: &c::Question) -> u64 {
    match question.from {
        Some(c::OwnerRequester::Task(task)) => task,
        _ => question.task.task,
    }
}
fn whole(prefix: &[u8], task: u64, out: &mut super::Text) {
    out.put(prefix);
    out.put(b"task activity ");
    out.number(task);
}
