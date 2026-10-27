//! Public configured offers and observed readiness, without inferring selection.
use super::{Cut, Limits, Text, accounts::Window};
use jig_client::contract as c;
use skein_json::writer::Encoder;
use skein_lib::List;

pub(super) fn fields(
    providers: &List<c::ProviderOffer>,
    providers_cut: bool,
    key_bytes: u32,
    out: &mut Encoder,
    cut: &mut Cut,
    window: &mut Window,
) {
    window.cut |= providers_cut;
    out.key(b"providers");
    out.array_start();
    for offer in providers {
        if !window.take() {
            break;
        }
        window.cut |= offer.name.cut || offer.programs_cut;
        out.object_start();
        out.key(b"provider");
        out.unsigned(u64::from(offer.provider));
        out.key(b"key_bytes");
        out.unsigned(u64::from(offer.key_bytes));
        out.key(b"name");
        super::encode_document_text(&offer.name, out, cut);
        for (key, flag) in [
            (b"sign_in".as_slice(), offer.sign_in),
            (b"key", offer.key),
            (b"none", offer.none),
            (b"signed_in_plan", offer.signed_in_plan),
            (b"programs_cut", offer.programs_cut),
        ] {
            out.key(key);
            out.boolean(flag);
        }
        out.key(b"programs");
        out.array_start();
        for program in &offer.programs {
            if !window.take() {
                break;
            }
            whole(program.bytes(), out, cut, window);
        }
        out.array_end();
        out.key(b"manage_usage");
        match &offer.manage_usage {
            Some(uri) => whole(uri, out, cut, window),
            None => out.null(),
        }
        out.object_end();
    }
    out.array_end();
    out.key(b"providers_cut");
    out.boolean(providers_cut);
    out.key(b"key_bytes");
    out.unsigned(u64::from(key_bytes));
}
pub(super) fn readiness(
    rows: &List<c::PayerReadiness>,
    lost: bool,
    limits: &Limits,
    out: &mut Encoder,
    window: &mut Window,
) {
    let mut readiness = Window { left: readiness_rows(limits), cut: lost };
    out.key(b"readiness");
    out.array_start();
    for row in rows {
        if !readiness.take() {
            break;
        }
        out.object_start();
        out.key(b"provider");
        out.unsigned(u64::from(row.provider));
        out.key(b"payer");
        out.object_start();
        match row.payer {
            c::OwnerAccountUse::Party(n) => {
                out.key(b"party");
                out.unsigned(n);
            }
            c::OwnerAccountUse::Project(n) => {
                out.key(b"project");
                out.unsigned(u64::from(n));
            }
        }
        out.object_end();
        out.key(b"readiness");
        out.string(match row.readiness {
            c::Readiness::Usable => b"usable",
            c::Readiness::Unavailable => b"unavailable",
            c::Readiness::Unknown => b"unknown",
        });
        out.key(b"settled");
        match row.settled {
            Some(flag) => out.boolean(flag),
            None => out.null(),
        }
        out.object_end();
    }
    out.array_end();
    out.key(b"readiness_cut");
    out.boolean(lost);
    window.cut |= readiness.cut;
}
// The contract caps each readiness list at rows. At 128 B per row (including
// 20-digit party numbers), both lists use at most half the available window.
pub(super) fn readiness_rows(limits: &Limits) -> u32 {
    limits.rows.min(limits.text.saturating_sub(128).saturating_sub(1024) / 4 / 128)
}
pub(super) fn me_window(limits: &Limits) -> Window {
    // Contract: rows providers × (1 header + rows programs). Each JSON row
    // costs at most 256 structural bytes; escaped text costs 6 × text bytes.
    // Reserve 1 KiB for the envelope; the result-byte ceiling bounds this window.
    let rows = limits.rows.saturating_mul(limits.rows.saturating_add(1));
    let room = limits.result_bytes.saturating_sub(1024).saturating_sub(u64::from(limits.text).saturating_mul(6));
    Window { left: u32::try_from(u64::from(rows).min(room / 256)).expect("contract row bound"), cut: false }
}
pub(super) fn whole(bytes: &[u8], out: &mut Encoder, cut: &mut Cut, window: &mut Window) {
    if bytes.len() <= cut.room {
        cut.words(bytes, out);
    } else {
        cut.left =
            cut.left.checked_add(u64::try_from(bytes.len()).expect("bounded identifier")).expect("bounded document");
        window.cut = true;
        out.null();
    }
}
fn whole_text(bytes: &[u8], out: &mut Text, window: &mut Window) {
    if bytes.len() <= out.room() {
        out.words(bytes);
    } else {
        out.omit(bytes.len());
        window.cut = true;
    }
}
fn flag(value: bool, out: &mut Text) {
    out.put(if value { b"true" } else { b"false" });
}
pub(super) fn text(me: &c::Me, limits: &Limits, out: &mut Text) {
    out.put(b"providers:\n");
    let mut window = me_window(limits);
    window.cut = me.cut || me.providers_cut;
    for offer in &me.providers {
        if !window.take() {
            break;
        }
        window.cut |= offer.name.cut || offer.programs_cut;
        out.put(b"  provider ");
        out.number(u64::from(offer.provider));
        out.put(b" name: ");
        out.words(offer.name.bytes());
        out.put(b" name_cut: ");
        flag(offer.name.cut, out);
        out.put(b" key_bytes: ");
        out.number(u64::from(offer.key_bytes));
        for (key, value) in [
            (b" sign_in: ".as_slice(), offer.sign_in),
            (b" key: ", offer.key),
            (b" none: ", offer.none),
            (b" signed_in_plan: ", offer.signed_in_plan),
            (b" programs_cut: ", offer.programs_cut),
        ] {
            out.put(key);
            flag(value, out);
        }
        out.put(b" programs:");
        for program in &offer.programs {
            if !window.take() {
                break;
            }
            out.put(b" ");
            whole_text(program.bytes(), out, &mut window);
        }
        out.put(b" manage_usage: ");
        match &offer.manage_usage {
            Some(uri) => whole_text(uri, out, &mut window),
            None => out.put(b"null"),
        }
        out.put(b"\n");
    }
    out.put(b"providers_cut: ");
    flag(me.providers_cut, out);
    out.put(b"\nkey_bytes: ");
    out.number(u64::from(me.key_bytes));
    out.put(b"\ncut: ");
    flag(window.cut || out.left() > 0, out);
    out.put(b"\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    const IDENTIFIERS: &[(&[u8], &[u8])] =
        &[(b"helper", b"\"helper\""), (b"https://provider.example/usage", b"\"https://provider.example/usage\"")];
    #[test]
    fn identifiers_are_whole_at_the_byte_bound() {
        for (bytes, json) in IDENTIFIERS {
            let (printed, cut) = json_whole(bytes, bytes.len());
            assert_eq!(printed.as_ref(), *json);
            assert!(!cut);
            let mut text = Text::with_room(bytes.len());
            let mut window = Window { left: 0, cut: false };
            whole_text(bytes, &mut text, &mut window);
            assert_eq!(text.written(), bytes.len());
            assert!(!window.cut);
        }
    }
    #[test]
    fn identifiers_are_absent_past_the_byte_bound() {
        for (bytes, _) in IDENTIFIERS {
            let room = bytes.len().checked_sub(1).expect("nonempty identifier");
            let (printed, cut) = json_whole(bytes, room);
            assert_eq!(printed.as_ref(), b"null");
            assert!(cut);
            let mut text = Text::with_room(room);
            let mut window = Window { left: 0, cut: false };
            whole_text(bytes, &mut text, &mut window);
            assert_eq!(text.written(), 0);
            assert_eq!(text.left(), bytes.len());
            assert!(window.cut);
        }
    }
    fn budget(room: usize) -> Cut {
        Cut { room, left: 0, turn: None, subject: false, digest: None }
    }
    fn json_whole(bytes: &[u8], room: usize) -> (Box<[u8]>, bool) {
        let limits = skein_json::writer::Limits { depth: 1, length: 4096 };
        let mut measure = Encoder::measure(&limits);
        whole(bytes, &mut measure, &mut budget(room), &mut Window { left: 0, cut: false });
        let mut out = Encoder::write(measure.measured().expect("identifier length"), &limits);
        let mut window = Window { left: 0, cut: false };
        whole(bytes, &mut out, &mut budget(room), &mut window);
        (out.finish(), window.cut)
    }
}
