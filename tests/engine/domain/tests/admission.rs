//! Completion admission is independent of the backlog watermark.
use jig_domain_core::documents::{Watched, Window};
use jig_domain_tasks as tasks;
use skein_lib::Token;
use temper_engine_domain as engine;
use temper_engine_domain_world::{direct::Driver, protocol};

#[test]
fn completions_are_taken_at_the_watermark_and_new_work_waits() {
    let (mut driver, run) = Driver::chat();
    // On this base the watermark is the journal's commit count, not a
    // separate default that could hide the old three-commit behaviour.
    driver.env.limits.journal.commits = 3;
    let watcher = Token::new(980);
    driver.send(engine::Event::Party(engine::PartyInput::Watch {
        watcher,
        sign_in: driver.session(),
        subject: Watched::Frame,
        window: Window { rows: 1, after: None },
    }));
    driver.settle();
    driver.fill_to_watermark();
    let returned = driver.returned;
    for event in [
        protocol::turn(Token::new(7), run.task, run.attempt, 1, 3, None, b"turn"),
        engine::Event::Worker(engine::WorkerInput::ResultRead {
            payload: Token::new(9999),
            end: tasks::End::Finished {
                result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
                cancel_delegates: false,
            },
        }),
        protocol::lost(Token::new(9999)),
        engine::Event::Party(engine::PartyInput::Delivered { watcher }),
    ] {
        driver.send(event);
        assert_eq!(driver.returned, returned, "a completion is taken at the watermark");
    }
    let call = driver.protocol.undecodable(Token::new(7), run.task, run.attempt, 981);
    driver.send(call);
    assert_eq!(driver.returned, returned + 1, "a worker call is new work");
    driver.send(engine::Event::Party(engine::PartyInput::Watch {
        watcher: Token::new(982),
        sign_in: driver.session(),
        subject: Watched::Frame,
        window: Window { rows: 1, after: None },
    }));
    assert_eq!(driver.returned, returned + 2, "a watch is new work");
    driver.settle();
    assert!(!driver.stopped);
}
