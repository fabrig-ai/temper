#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "rendered calls are checked with ordinary test collections; testing-strategy.md, step tests"
)]
use super::fixtures as f;
use crate::jig::{call::Parsed, render::outcome};
use jig_client::{Answered, Ask, Outcome, contract as c};
use skein_lib::List;
use temper_client_domain::forge::forges::{Connection, Installation, Kind};
use temper_client_protocol_wire::v1 as w;

#[test]
fn every_printed_call_parses() {
    let task = f::offered(&[
        c::ActKind::Message,
        c::ActKind::Answer,
        c::ActKind::Stop,
        c::ActKind::Cancel,
        c::ActKind::Release,
        c::ActKind::Accept,
        c::ActKind::Reject,
        c::ActKind::PassUp,
        c::ActKind::LeaveHeld,
        c::ActKind::Adopt,
        c::ActKind::Amend,
    ]);
    let mut forge = super::forges::row(Connection::Connected { id: 42, login: b"my-app".as_slice().into() });
    forge.kind = Kind::GitHub;
    forge.installations = Box::new([Installation { id: 22, account: None, suspended: true, stray: true }]);
    let revisions = c::ReadResult::Value(c::ReadValue::Revisions(Box::new(c::RevisionsPage {
        rows: List::with_capacity(0),
        next: Some(4),
        forgotten: false,
    })));
    let operation = outcome::Operation::of(&Ask::Jig(Box::new(c::Ask::AccountChange {
        holder: c::AccountHolder::Own,
        change: c::AccountChange::CheckProgram { source: f::SOURCE },
    })));
    let verify = outcome::print(
        &operation,
        &Outcome::Answered(Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountChanged)))),
        false,
        None,
        &f::limits_root().tools,
    );
    let (mut root, call) = f::message();
    root.answered(
        call,
        c::Answer::Refused {
            refusal: c::Refusal::Moved { now: c::Revision { value: f::HOLD }, what: c::Moved::Spec },
            at: None,
        },
    );
    let moved = root.answer(call).expect("moved refusal");
    let docs: [(&str, crate::jig::Printed, usize, u8); 11] = [
        ("task acts", f::read_answer(f::TASK_CALL, f::task_result(task)), 11, 0),
        ("activity cut", f::read_answer(f::LIMITED_ACTIVITY_CALL, f::activity_words()), 1, 0),
        ("activity turn", f::read_answer(f::ACTIVITY_CALL, f::activity_too_large()), 1, 0),
        ("turn cut", f::read_answer(f::TURN_CALL, f::turn_text(f::LONG_PART)), 1, 0),
        ("tasks page", f::read_answer(f::TASKS_CALL, f::task_page_with_cursor()), 1, 0),
        ("inbox page", f::inbox_answer(f::PAGE_ROWS, f::PAGE_ROWS, true), 1, 0),
        ("revisions page", f::read_answer(&["revision", "list"], revisions), 1, 0),
        ("forges installation", crate::forges::print_rows(Some(&[forge]), false, None, &f::limits_root()), 1, 0),
        (
            "change",
            crate::change::print(
                Some(&super::change::panel(w::Step::Checking)),
                false,
                None,
                b"",
                &f::limits_root(),
                None,
            ),
            1,
            0,
        ),
        ("verify follow-up", verify, 1, 0),
        ("moved refusal", moved, 1, 2),
    ];
    let mut failures = Vec::new();
    for (document, printed, expected, status) in docs {
        assert_eq!(printed.status, status, "{document}: {:?}", printed.stderr);
        let bytes = [printed.stdout.as_ref(), printed.stderr.as_ref()].concat();
        let text = core::str::from_utf8(&bytes).expect("rendered UTF-8");
        let mut count = 0_usize;
        let mut acts = false;
        for line in text.lines() {
            let call = if acts && line.starts_with("  ") {
                Some(line.trim_start())
            } else {
                acts = line == "acts:";
                marked_call(line)
            };
            if let Some(call) = call {
                count = count.checked_add(1).expect("fixture call count");
                let words = substitute(call);
                let argv: Vec<&str> = words.iter().map(String::as_str).collect();
                let parsed = f::parse(&argv);
                match &parsed {
                    Ok(Parsed::Call { .. }) => {}
                    Ok(Parsed::File { .. }) | Err(_) => failures.push((document, String::from(call), parsed)),
                }
            }
        }
        assert_eq!(count, expected, "{document}: all printed calls must be exercised\n{text}");
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn marked_call(line: &str) -> Option<&str> {
    for marker in ["read on with: ", "read with: ", "read again: ", "read on: ", "next: temper ctl DIR "] {
        if let Some((_, call)) = line.split_once(marker) {
            return Some(call);
        }
    }
    if let Some(at) = line.find(" · forge accept ") {
        return line.get(at..).expect("call suffix").strip_prefix(" · ");
    }
    line.strip_prefix("temper ctl DIR ").or_else(|| line.starts_with("account list --until ").then_some(line))
}

fn substitute(call: &str) -> Vec<String> {
    let mut words = Vec::new();
    for word in call.split_whitespace() {
        // CHANGE... stands for one or more named amendment controls.
        if word == "CHANGE..." {
            words.extend([String::from("--priority"), String::from("1")]);
        } else if word == "TEXT" || word == "REASON" {
            words.push(String::from("word"));
        } else if word.bytes().all(|byte| byte.is_ascii_uppercase()) {
            words.push(String::from("1"));
        } else if word.starts_with('t') && word.ends_with("/K") {
            words.push(word.replace("/K", "/1"));
        } else {
            words.push(word.into());
        }
    }
    words
}
