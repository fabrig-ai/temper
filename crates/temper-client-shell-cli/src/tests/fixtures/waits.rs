use super::*;
pub const LONG_LIVE: Duration = Duration::from_secs(31); // Exceeds the ordinary link deadline, below Hello's 60 seconds.
pub const BEFORE_DEADLINE: Duration = Duration::from_secs(29); // Proves the stopping wait cannot finish one second early.
impl Frame {
    pub fn socket_wait() -> Self {
        let mut frame = Self::read(&["task", "show", "41", "--until", "changed"]);
        frame.host.test_connections = Some(jig_client_shell_native::Limits {
            // The live stream and the two completed watches occupy all three sockets.
            exchanges: 2,
            ..crate::limits::native().connections
        });
        frame.sign_in_then_deliver_me();
        frame
    }
    pub fn exchange(&mut self, number: u64) {
        let exchange = skein_lib::Token::new((1 << 63) | number);
        assert!(self.probe.replace(exchange).is_none(), "one probe exchange at a time");
        let read = w::Read::Task(w::ReadTask::new(&w::CEILINGS, w::ReadTaskParts { task: 41 }).expect("read task"));
        let document = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: encode!(read) },
        )
        .expect("read document");
        self.host.transport.as_mut().expect("live memory transport").down(
            self.now,
            jig_client::Request::Exchange {
                exchange,
                message: jig_client::Message::Read(encode!(document)),
                credential: None,
            },
        );
        self.run();
    }
    pub(super) fn probe_reply(&mut self) {
        let Some(exchange) = self.probe else { return };
        let memory = self.host.transport.as_mut().expect("probe transport");
        let mut other = Vec::new();
        while let Some(event) = memory.events.pop() {
            match event {
                jig_client::Event::Reply { exchange: reply, message } if reply == exchange => {
                    self.probe_replies.push((reply.raw() & !(1 << 63), self.now, message));
                    self.probe = None;
                }
                jig_client::Event::Lost { exchange: lost } if lost == exchange => panic!("probe exchange lost"),
                event @ (jig_client::Event::Start { .. }
                | jig_client::Event::Authorize { .. }
                | jig_client::Event::StartUnlinked { .. }
                | jig_client::Event::SignIn { .. }
                | jig_client::Event::SignOut
                | jig_client::Event::SignOutLost
                | jig_client::Event::Watch { .. }
                | jig_client::Event::Unwatch { .. }
                | jig_client::Event::Read { .. }
                | jig_client::Event::Send { .. }
                | jig_client::Event::Carry { .. }
                | jig_client::Event::Reply { .. }
                | jig_client::Event::Lost { .. }
                | jig_client::Event::Streamed { .. }
                | jig_client::Event::StreamGone
                | jig_client::Event::Stored
                | jig_client::Event::NotStored
                | jig_client::Event::Up { .. }) => other.push(event),
            }
        }
        for event in other {
            memory.events.push(event);
        }
    }
    pub fn probe_replies(&self) -> Vec<(u64, Time)> {
        for (_, _, message) in &self.probe_replies {
            let answer =
                w::ReadAnswer::decode(&w::CEILINGS, &mut skein_lib::Reader::new(message)).expect("read answer");
            assert!(matches!(answer, w::ReadAnswer::Value(_)), "complete read response");
        }
        self.probe_replies.iter().map(|(number, now, _)| (*number, *now)).collect()
    }
    pub fn stream_live(&self) -> bool {
        self.stream.is_some_and(|fd| {
            self.sockets.get(&fd).is_some_and(|socket| socket.eof.is_none() && !socket.closed)
                && self
                    .held
                    .iter()
                    .any(|submit| matches!(submit.kind, Op::Recv { fd: receiving, .. } if receiving == fd))
        })
    }
    pub fn wait(words: &[&str]) -> Self {
        let mut frame = Self::read(words);
        frame.sign_in_then_deliver_me();
        frame
    }
    pub fn advance_live(&mut self, span: Duration) {
        let end = self.now.saturating_add(span);
        while self.now < end {
            self.sockets.get_mut(&self.stream.expect("stream")).expect("stream socket").reply.extend(b": alive\n\n");
            self.now = self.now.saturating_add(crate::limits::DEADLINE).min(end);
            self.run();
        }
    }
    pub fn engine_stopping(&mut self) {
        let event = w::StreamEvent::Ended(
            w::Ended::new(
                &w::CEILINGS,
                w::EndedParts { watch: self.wait_task.expect("wait watch"), why: w::WatchEnd::Stopping },
            )
            .expect("ended"),
        );
        self.sockets.get_mut(&self.stream.expect("open stream")).expect("stream socket").reply.extend(sse(&event));
        self.stopping = Some(());
        self.run();
        self.sockets.get_mut(&self.stream.expect("stream")).expect("stream socket").eof = Some(());
        self.run();
    }
    pub fn change_task(&mut self) {
        self.deliver_task(true);
        self.run();
    }
    pub fn refuse_wait(&mut self) {
        let event = w::StreamEvent::Ended(
            w::Ended::new(
                &w::CEILINGS,
                w::EndedParts { watch: self.wait_task.expect("wait watch"), why: w::WatchEnd::NotAllowed },
            )
            .expect("ended"),
        );
        self.sockets.get_mut(&self.stream.expect("open stream")).expect("stream socket").reply.extend(sse(&event));
        self.run();
    }
    pub(super) fn deliver_task(&mut self, changed: bool) {
        let mut parts = jig_client_test_records::task_record().into_parts();
        if changed {
            parts.project += 1;
        }
        let task = w::Task::new(&w::CEILINGS, parts).expect("task");
        let value = w::DeliveryValue::Task(Box::new(task));
        let document = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: encode!(value) },
        )
        .expect("task delivery");
        let event = w::StreamEvent::Delivery(
            w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch: self.wait_task.expect("wait watch"), document })
                .expect("delivery"),
        );
        self.sockets.get_mut(&self.stream.expect("open stream")).expect("stream socket").reply.extend(sse(&event));
    }
    pub fn no_directory(&self) -> bool {
        self.opened.is_empty() && self.sockets.is_empty() && self.locks == 0
    }
}
