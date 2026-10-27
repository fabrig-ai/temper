//! Pending protocol events and attempts, without their content or credentials.
use super::{Worker, hub, root};
use crate::diagnostics::{self, LinkEvent, WorkerEvent};
use skein_lib::{List, Time};

impl Worker {
    pub(crate) fn pending(&self, now: Time) -> diagnostics::Worker {
        let hosting = self.root.hosting();
        let mut attempts =
            List::with_capacity(hosting.len().checked_add(self.root.kept_answers()).expect("host attempts"));
        for attempt in &hosting {
            attempts.push(*attempt).expect("room for hosting and retained answers");
        }
        for index in 0..self.root.kept_answers() {
            if let Some(hub::LinkUp::Answer { run, attempt, .. }) = self.root.host().answer_at(index) {
                attempts
                    .push(hub::Hosting { run, attempt, phase: hub::Phase::Answered })
                    .expect("room for hosting and retained answers");
            }
        }
        let mut events = List::with_capacity(self.events.len());
        for event in &self.events {
            events.push(event_tag(event)).expect("one tag per queued event");
        }
        diagnostics::Worker {
            work_pending: self.work_pending(now),
            started: self.started,
            aborted: self.aborted.is_some(),
            root_ready: self.root.is_ready(),
            events,
            process_events: self.process_events.len(),
            io: self.io.len(),
            submitted: self.submitted.len(),
            outputs: self.out.len(),
            link_outputs: self.link_out.len(),
            retained_turns: self.root.retained_turns(),
            kept_answers: self.root.kept_answers(),
            next_deadline: self.next_deadline(),
            attempts,
        }
    }
}

fn event_tag(event: &root::Event) -> WorkerEvent {
    match event {
        root::Event::SourceAnswered { .. } => WorkerEvent::SourceAnswered,
        root::Event::Assign { .. } => WorkerEvent::Assign,
        root::Event::Link(link) => match link {
            hub::LinkDown::Sources { .. } => WorkerEvent::Link(LinkEvent::Sources),
            hub::LinkDown::Check { .. } => WorkerEvent::Link(LinkEvent::Check),
            hub::LinkDown::SignOut { .. } => WorkerEvent::Link(LinkEvent::SignOut),
            hub::LinkDown::Assign { .. } => WorkerEvent::Link(LinkEvent::Assign),
            hub::LinkDown::Message { .. } => WorkerEvent::Link(LinkEvent::Message),
            hub::LinkDown::Relayed { .. } => WorkerEvent::Link(LinkEvent::Relayed),
            hub::LinkDown::TurnKept { .. } => WorkerEvent::Link(LinkEvent::TurnKept),
            hub::LinkDown::Acknowledge { .. } => WorkerEvent::Link(LinkEvent::Acknowledge),
            hub::LinkDown::Cancel { .. } => WorkerEvent::Link(LinkEvent::Cancel),
            hub::LinkDown::Grant { .. } => WorkerEvent::Link(LinkEvent::Grant),
        },
        root::Event::Ready => WorkerEvent::Ready,
        root::Event::Lost => WorkerEvent::Lost,
        root::Event::GracePassed => WorkerEvent::GracePassed,
        root::Event::Resend { .. } => WorkerEvent::Resend,
        root::Event::RelayCancelled { .. } => WorkerEvent::RelayCancelled,
        root::Event::Process(_) => WorkerEvent::Process,
        root::Event::Live { .. } => WorkerEvent::Live,
        root::Event::Decoded { .. } => WorkerEvent::Decoded,
        root::Event::Encoded { .. } => WorkerEvent::Encoded,
        root::Event::Activity { .. } => WorkerEvent::Activity,
        root::Event::Done { .. } => WorkerEvent::Done,
        root::Event::Shutdown => WorkerEvent::Shutdown,
    }
}
