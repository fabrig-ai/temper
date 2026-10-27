//! Settings tools use the shared ctl vocabulary and byte offsets.
use super::{decided, decoded};
use crate::{Problem, settings_answer, settings_text};
use jig_client::contract as c;
use jig_domain_core::{SettingsAnswer, SettingsRead, documents as d, settings as s};
use skein_lib::List;
use temper_engine_domain as engine;

#[test]
fn every_settings_read_form_and_offset_decodes() {
    for (input, read, from) in [
        (br"{}".as_slice(), SettingsRead::Inventory, 0),
        (br#"{"from":7}"#, SettingsRead::Inventory, 7),
        (
            br#"{"setting":{"kind":"charter","number":2}}"#,
            SettingsRead::Setting { setting: s::Setting::Charter(2), revision: 0 },
            0,
        ),
        (
            br#"{"setting":{"kind":"charter","number":2},"revision":3,"from":8}"#,
            SettingsRead::Setting { setting: s::Setting::Charter(2), revision: 3 },
            8,
        ),
        (
            br#"{"setting":{"kind":"template","number":1}}"#,
            SettingsRead::Setting { setting: s::Setting::Template(1), revision: 0 },
            0,
        ),
        (
            br#"{"setting":{"kind":"policy","number":1}}"#,
            SettingsRead::Setting { setting: s::Setting::Policy(1), revision: 0 },
            0,
        ),
        (br#"{"setting":{"kind":"rules"}}"#, SettingsRead::Setting { setting: s::Setting::Rules, revision: 0 }, 0),
        (br#"{"revision":4,"from":9}"#, SettingsRead::Revision { number: 4 }, 9),
        (br#"{"history":{},"from":10}"#, SettingsRead::Revisions { before: None }, 10),
        (br#"{"history":{"before":4},"from":11}"#, SettingsRead::Revisions { before: Some(4) }, 11),
    ] {
        assert_eq!(decided(b"settings", input).expect("valid read").tool, engine::Tool::Settings { read, from });
    }
}
#[test]
fn settings_bad_fields_are_refused_by_name() {
    for (input, why) in [
        (br#"{"from":-1}"#.as_slice(), Problem::Range(b"from")),
        (br#"{"revision":0}"#, Problem::Range(b"revision")),
        (br#"{"setting":4}"#, Problem::Type(b"setting")),
        (br#"{"setting":{"kind":"charter"}}"#, Problem::Missing(b"number")),
        (br#"{"setting":{"kind":"rules","number":1}}"#, Problem::Range(b"number")),
        (br#"{"history":[],"from":0}"#, Problem::Type(b"history")),
        (br#"{"history":{"before":0}}"#, Problem::Range(b"before")),
        (br#"{"history":{},"revision":1}"#, Problem::Range(b"history")),
    ] {
        assert_eq!(decoded(b"settings", input).expect_err("invalid read"), why);
    }
    for (input, field) in [
        (br#"{"items":[],"reason":"why"}"#.as_slice(), b"against".as_slice()),
        (br#"{"against":1,"items":[]}"#, b"reason"),
    ] {
        assert_eq!(decoded(b"propose_settings", input).expect_err("missing proposal field"), Problem::Missing(field));
    }
    let crate::Decoded::Answered(answer) = decoded(
        b"propose_settings",
        br#"{"against":1,"reason":"why","items":[{"setting":{"kind":"rules"},"edit":"wrong"}]}"#,
    )
    .expect("bounded decode error") else {
        panic!("bad edit")
    };
    let (body, error) = answer.into_parts();
    assert!(error);
    let mut named = false;
    for word in body.windows(4) {
        named |= word == b"edit";
    }
    assert!(named);
}
#[test]
fn settings_proposal_items_use_the_file_decoder() {
    let body = decided(b"propose_settings", br#"{"against":1,"reason":"restore the default","items":[{"setting":{"kind":"charter","number":1},"edit":"reset"}]}"#).expect("proposal");
    assert_eq!(
        body.tool,
        engine::Tool::ProposeSettings {
            against: 1,
            reason: b"restore the default".as_slice().into(),
            items: Box::new([s::Item { setting: s::Setting::Charter(1), edit: s::Edit::Reset }])
        }
    );
}
fn inventory(count: u32) -> SettingsAnswer {
    let mut settings = List::with_capacity(count);
    for number in 1..=count {
        settings
            .push(d::SettingLine {
                setting: s::Setting::Charter(number),
                name: None,
                follows: d::Follows::Change,
                last: 5,
            })
            .expect("inventory count");
    }
    SettingsAnswer::Inventory(d::Settings {
        newest: 5,
        settings,
        held: List::with_capacity(0),
        pending: List::with_capacity(0),
    })
}
#[test]
fn settings_inventory_pages_continue_at_the_exact_offset_and_end() {
    let answer = inventory(80);
    let whole = settings_text(&answer, None, &[]).expect("inventory text");
    let mut from = 0;
    let mut combined = skein_lib::Writer::new(whole.len());
    let mut pages = 0_u32;
    loop {
        let (body, error) = settings_answer(&answer, from, None, &[]).into_parts();
        assert!(!error);
        assert!(body.len() <= usize::try_from(crate::DOC_ANSWER).expect("host bound"));
        let mut marker = None;
        for (index, bytes) in body.windows(6).enumerate() {
            if bytes == b"next: " {
                marker = Some(index);
            }
        }
        if let Some(marker) = marker {
            let page = body.get(..marker).expect("page before marker");
            assert!(page.ends_with(b"\n"));
            combined.put(page).expect("reconstructed page");
            let digits = body
                .get(marker.checked_add(6).expect("marker length")..body.len().checked_sub(1).expect("line end"))
                .expect("next offset");
            let next = u32::try_from(crate::decimal(digits, b"next").expect("offset")).expect("u32 offset");
            assert!(next > from);
            from = next;
            pages += 1;
        } else {
            assert!(body.ends_with(b"end\n"));
            combined
                .put(body.get(..body.len().checked_sub(4).expect("last marker")).expect("page"))
                .expect("last page");
            break;
        }
    }
    assert!(pages >= 1, "the long inventory requires a continuation");
    assert_eq!(combined.finish(), whole);
    assert_eq!(settings_answer(&answer, u32::MAX, None, &[]).into_parts().0.as_ref(), b"end\n");
}
#[test]
fn engine_and_ctl_render_the_same_settings_inventory() {
    let mut rows = List::with_capacity(2);
    for number in 1..=2 {
        rows.push(c::SettingLine {
            setting: c::Setting::Charter(c::SettingNumber { number }),
            name: None,
            follows: c::SettingFollows::Change,
            last: 5,
        })
        .expect("two rows");
    }
    let value = c::ReadValue::Settings(Box::new(c::SettingsDocument {
        newest: 5,
        settings: rows,
        held: List::with_capacity(0),
        pending: List::with_capacity(0),
    }));
    assert_eq!(
        settings_text(&inventory(2), None, &[]).expect("engine text"),
        temper_client_tools::jig::render::settings_text(&value, None, &temper_client_tools::limits::tools())
    );
}
#[test]
fn settings_inbox_outcomes_keep_the_revision_and_rejection_reason() {
    use jig_domain_tasks as tasks;
    for (outcome, expected) in [
        (tasks::SettingsOutcome::Accepted { revision: 7 }, "accepted: revision 7 made"),
        (tasks::SettingsOutcome::Rejected, "rejected: keep it"),
        (tasks::SettingsOutcome::Stale { newest: 8 }, "withdrawn as stale: the newest is 8"),
    ] {
        let word = tasks::Word {
            number: 1,
            from: tasks::Party::Deployment { project: 0 },
            kind: tasks::MessageKind::SettingsDecision { proposal: 1, outcome },
            words: b"keep it".as_slice().into(),
            at: skein_lib::Wall::EPOCH,
            hits: 1,
            eligible: true,
        };
        assert_eq!(engine::inbox_words(&word).as_ref(), expected.as_bytes());
    }
}

#[test]
fn settings_pages_price_escapes_cut_long_lines_and_keep_utf8_offsets() {
    let quoted = "\"".repeat(800);
    let first = crate::settings::page(quoted.as_bytes(), 0).expect("quoted text");
    assert_eq!(first.as_ref(), format!("{}\nnext: 768\n", "\"".repeat(768)).as_bytes());
    let last = crate::settings::page(quoted.as_bytes(), 768).expect("last quoted page");
    assert_eq!(last.as_ref(), format!("{}\nend\n", "\"".repeat(32)).as_bytes());
    let unicode = format!("{}λ", "x".repeat(1535));
    let page = crate::settings::page(unicode.as_bytes(), 0).expect("UTF-8 boundary cut");
    assert!(page.ends_with(b"next: 1535\n"));
    assert_eq!(
        crate::settings::page(unicode.as_bytes(), 1535).expect("last character").as_ref(),
        "λ\nend\n".as_bytes()
    );
    assert_eq!(crate::settings::page(unicode.as_bytes(), 1536), Err(Problem::Range(b"from")));
    let lines = format!("short\n{}", "q".repeat(1800));
    assert_eq!(crate::settings::page(lines.as_bytes(), 0).expect("line cut").as_ref(), b"short\nnext: 6\n");
}

#[test]
fn a_large_settings_decoder_error_is_bounded_host_feedback() {
    let key = "x".repeat(smith_domain_run::HostInput::CAPACITY.checked_sub(128).expect("input envelope room"));
    let input =
        format!("{{\"against\":1,\"reason\":\"why\",\"items\":[{{\"setting\":{{\"kind\":\"rules\"}},\"{key}\":0}}]}}");
    let crate::Decoded::Answered(answer) =
        decoded(b"propose_settings", input.as_bytes()).expect("bounded malformed input")
    else {
        panic!("unknown item field")
    };
    let (body, error) = answer.into_parts();
    assert!(error);
    assert!(body.len() <= smith_domain_run::HostAnswer::CAPACITY);
    assert!(body.starts_with(b"settings file item 1:"));
}
