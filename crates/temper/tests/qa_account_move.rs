//! QA-A: interrupted and refused account moves; client/tools.md 5.5, 6.3, 15;
//! jig's domain/accounts.md 6 and protocol/llm.md 5.6. All records are fake.
pub mod support;

use std::{fs, thread, time::Duration};
use support::account_move_qa::{Boundary, DEADLINE_MS, Fixture, refused, success};

#[test]
fn before_written_signal_and_lost_reply_restore_while_retry_is_moving() {
    let fixture = Fixture::new("qa-move-before-written");
    let number = fixture.number();
    let relay = fixture.a().relay();
    let gate = relay.arm(1, Boundary::AfterReply);
    let export = fixture.a().spawn(&["account", "export", &number, fixture.path()]);
    gate.wait();
    assert!(fixture.file.exists(), "FILE created before move_out");
    assert_eq!(fs::metadata(&fixture.file).expect("FILE").len(), 0, "record has not reached ctl");
    export.terminate();
    let ended = export.finish();
    assert_eq!(ended.status.code(), Some(143));
    gate.release(false);
    assert!(!fixture.file.exists(), "signal before written removes FILE");
    let retry = fixture.a().call(&["account", "export", &number, fixture.path()], None);
    refused(&retry, "moving");
    assert!(!fixture.file.exists());
    let account = fixture.a().account_state("granted");
    assert_eq!(account["number"].as_u64().expect("number").to_string(), number);
    assert_eq!(account["sources"][0]["usable"], true);
    lost_move_out_answer(&fixture, &relay, &number);
}

#[test]
fn after_written_file_returns_to_source_but_dead_copies_and_other_holders_are_refused() {
    let mut fixture = Fixture::new("qa-move-after-written");
    let number = fixture.number();
    let relay = fixture.a().relay();
    let gate = relay.arm(2, Boundary::AfterReply);
    let export = fixture.a().spawn(&["account", "export", &number, fixture.path()]);
    gate.wait();
    assert!(support::private(&fixture.file));
    assert!(fs::metadata(&fixture.file).expect("synced FILE").len() > 0);
    export.terminate();
    assert_eq!(export.finish().status.code(), Some(143));
    gate.release(false);
    assert!(fixture.file.exists(), "written sent: FILE stays");
    fixture.a().direct();
    drop(relay);
    fixture.a().empty(); // Resends kept written before reading accounts (tools.md 15).
    success(&fixture.a().call(&["account", "import", fixture.path(), "--key", "qa-return"], None));
    assert!(!fixture.file.exists());
    let account = fixture.a().account_state("granted");
    assert_ne!(
        account["number"].as_u64().expect("number").to_string(),
        number,
        "erased account numbers are never reused"
    );
    dead_copy_and_another_holder(&mut fixture);
}

#[test]
fn restart_while_moving_cancels_and_the_left_copy_needs_sign_in() {
    let mut fixture = Fixture::new("qa-move-restart");
    let number = fixture.number();
    let relay = fixture.a().relay();
    let gate = relay.arm(2, Boundary::BeforeRequest);
    let export = fixture.a().spawn(&["account", "export", &number, fixture.path()]);
    gate.wait();
    export.terminate();
    assert_eq!(export.finish().status.code(), Some(143));
    gate.release(false);
    drop(relay);
    fixture.restart_a();
    let account = fixture.a().account_state("granted");
    assert_eq!(account["number"].as_u64().expect("number").to_string(), number);
    assert!(fixture.file.exists(), "later cancelled written leaves FILE (tools.md 15)");
    success(&fixture.b.call(&["account", "import", fixture.path(), "--key", "qa-restart-dead"], None));
    let _ = fixture.b.account_state("sign_in_again");
    let _ = fixture.a().account_state("granted");
}

fn lost_move_out_answer(fixture: &Fixture, relay: &support::account_move_qa::Relay, number: &str) {
    let gate = relay.arm(1, Boundary::AfterReply);
    let export = fixture.a().spawn(&["account", "export", number, fixture.path()]);
    gate.wait();
    gate.release(false);
    let ended = export.finish();
    assert_eq!(ended.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ended.stderr).contains("the record of the move was lost on its way"));
    assert!(!fixture.file.exists());
    let _ = fixture.a().account_state("granted");
}

#[test]
fn written_past_the_short_deadline_is_cancelled_and_removes_file() {
    let fixture = Fixture::new("qa-move-deadline");
    let number = fixture.number();
    let relay = fixture.a().relay();
    let gate = relay.arm(2, Boundary::BeforeRequest);
    let export = fixture.a().spawn(&["account", "export", &number, fixture.path()]);
    gate.wait();
    thread::sleep(Duration::from_millis(DEADLINE_MS + 100));
    gate.release(true);
    refused(&export.finish(), "cancelled");
    assert!(!fixture.file.exists());
    let _ = fixture.a().account_state("granted");
}

fn dead_copy_and_another_holder(fixture: &mut Fixture) {
    success(&fixture.a().call(&["account", "export", &fixture.number(), fixture.path()], None));
    let copy = fixture.copy("stale.move");
    let held = fixture.copy("held.move");
    fixture.a().empty();
    success(&fixture.b.call(&["account", "import", fixture.path(), "--key", "qa-fresh"], None));
    let _ = fixture.b.account_state("granted");
    // tools.md 15 and jig's protocol/llm.md 5.6: adoption succeeds;
    // the dead refresh token is refused at the source's first refresh.
    success(&fixture.a().call(&["account", "import", copy.to_str().expect("copy path"), "--key", "qa-stale"], None));
    let _ = fixture.a().account_state("sign_in_again");
    assert!(!copy.exists(), "adopted file is removed even if first refresh refuses it");
    let _ = fixture.b.account_state("granted");
    fixture.another_holder_on_b();
    let output = fixture.b.call(&["account", "import", held.to_str().expect("held path"), "--key", "qa-held"], None);
    assert_eq!(output.status.code(), Some(2), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stderr, b"import: held by another holder\n");
    assert!(held.exists(), "a refusal keeps the only file the caller holds");
}
