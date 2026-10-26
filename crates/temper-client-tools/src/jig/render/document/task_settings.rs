//! A task's settings revision (client/tools.md, 6.1).
use super::Text;
use jig_client::contract::TaskSettings;
use skein_json::writer::Encoder;

pub(super) fn text(settings: Option<&TaskSettings>, out: &mut Text) {
    let Some(settings) = settings else { return };
    out.put(b"\nsettings: revision ");
    out.number(u64::from(settings.task));
    if let Some(run) = settings.run {
        out.put(" · its run on ".as_bytes());
        out.number(u64::from(run));
    }
    if settings.newest != settings.task {
        out.put(" · newest ".as_bytes());
        out.number(u64::from(settings.newest));
    }
}

pub(super) fn encode(settings: Option<&TaskSettings>, out: &mut Encoder) {
    out.key(b"settings");
    match settings {
        Some(settings) => {
            out.object_start();
            out.key(b"task");
            out.unsigned(u64::from(settings.task));
            out.key(b"run");
            match settings.run {
                Some(run) => out.unsigned(u64::from(run)),
                None => out.null(),
            }
            out.key(b"newest");
            out.unsigned(u64::from(settings.newest));
            out.object_end();
        }
        None => out.null(),
    }
}
