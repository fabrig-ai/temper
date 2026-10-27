//! A change panel on the HTTP fake, with not-ready supplemental reads.
use super::*;
use temper_client_protocol_wire::{self as wire, v1 as f};
// jig-client-test-records' task chip is 7 in project 3.
const TASK: u64 = 7;
pub const TIMEOUT: Duration = Duration::from_secs(12);
impl Frame {
    pub fn change_wait(timeout: &str) -> Self {
        let mut frame =
            Self::read(&["change", "show", "7", "--project", "3", "--until", "landed", "--timeout", timeout]);
        frame.change = true;
        frame.sign_in_then_deliver_me();
        frame
    }
    pub(super) fn deliver_change(&mut self) {
        let used = f::Used::new(&f::CEILINGS, f::UsedParts { used: 0, limit: 3 }).expect("repair limit");
        let panel = f::ChangePanel::new(
            &f::CEILINGS,
            f::ChangePanelParts {
                task: TASK,
                forge: 3,
                owner: b"qa".as_slice().into(),
                name: b"restart".as_slice().into(),
                branch: b"temper/7".as_slice().into(),
                landing: b"main".as_slice().into(),
                head: None,
                pull: None,
                step: f::Step::Gating,
                hold: Some(f::Hold::Stalled),
                runs: None,
                ci: f::Ci::Pending,
                gates: skein_lib::List::with_capacity(0),
                repairs: used.clone(),
                resolutions: used.clone(),
                updates: used,
            },
        )
        .expect("held panel");
        let document = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts {
                owner: w::Owner::Family(
                    w::Family::new(&w::CEILINGS, w::FamilyParts { number: wire::FORGE }).expect("forge"),
                ),
                version: wire::CLIENT_VERSION,
                record: wire::change_panel(&panel),
            },
        )
        .expect("change document");
        let event = w::StreamEvent::Delivery(
            w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch: self.wait_task.expect("change watch"), document })
                .expect("delivery"),
        );
        self.sockets.get_mut(&self.stream.expect("stream")).expect("stream socket").reply.extend(sse(&event));
    }
    pub fn change_task_not_ready(&mut self) {
        self.read_unreachable = true;
        let event = w::StreamEvent::Ended(
            w::Ended::new(
                &w::CEILINGS,
                w::EndedParts { watch: self.wait_task.expect("change watch"), why: w::WatchEnd::Unknown },
            )
            .expect("ended"),
        );
        self.sockets.get_mut(&self.stream.expect("stream")).expect("stream socket").reply.extend(sse(&event));
        self.run();
    }
    pub fn change_task_ready(&mut self) {
        self.read_unreachable = false;
        self.advance_past(crate::limits::native().root.domain.client.domain.backoff.first);
    }
    pub fn engine_restarted(&mut self) {
        self.stopping = None;
        self.advance_past(crate::limits::native().root.domain.client.domain.backoff.first);
    }
}
