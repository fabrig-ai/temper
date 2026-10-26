use super::*;
use jig_client::Record;
pub const ACT_WITH_STREAM: u32 = 2; // Both the request and stream predecessor survive a deadline.
const PENDING_PARTY: u64 = 73;
pub fn pending(unread: bool, stream: bool, json: bool) -> Frame {
    let mut records = vec![recovery::record(PENDING_PARTY)];
    if unread {
        records.push(Record { key: b"unread".as_slice().into(), bytes: b"unknown".as_slice().into() });
    }
    if stream {
        let kept = w::Kept::new(
            &w::CEILINGS,
            w::KeptParts { store_version: jig_client_domain::TABLE_VERSION, record: [1; 16].into() },
        )
        .expect("stream record");
        records.push(Record { key: jig_client::STREAM_KEY.into(), bytes: encode!(kept) });
    }
    Frame::act_words_with(if json { &[b"pending", b"--json"] } else { &[b"pending"] }, small_store(&records))
}
pub fn kept_close() -> Box<[u8]> {
    let limits = jig_client_protocol_link::Limits::new(
        crate::limits::native().root.domain.client.domain,
        crate::limits::EXCHANGES,
    )
    .expect("link limits");
    let me = jig_client_protocol_link::translate::documents::frame(recovery::chat_me(), &limits).expect("me");
    let ask = jig_client::shapes::close_chat(
        &me,
        3,
        7,
        &[],
        jig_client::contract::Text::new(b"".as_slice().into(), crate::limits::native().root.tools.text)
            .expect("reason"),
    )
    .expect("close");
    small_store(&[recovery::stored_request(7, jig_client::Ask::Jig(Box::new(ask)))])
}
impl Frame {
    pub(super) fn hello(&mut self, fd: Fd) {
        self.stream = Some(fd);
        self.sockets
            .get_mut(&fd)
            .expect("stream socket")
            .reply
            .extend(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
        let mut served = skein_lib::List::with_capacity(2);
        served
            .push(
                w::Served::new(
                    &w::CEILINGS,
                    w::ServedParts { owner: w::Owner::Jig, first: w::VERSION, last: w::VERSION },
                )
                .expect("served version"),
            )
            .expect("one family");
        let owner = crate::limits::native().root.domain.client.domain.owners[0];
        served
            .push(
                w::Served::new(
                    &w::CEILINGS,
                    w::ServedParts {
                        owner: w::Owner::Family(
                            w::Family::new(&w::CEILINGS, w::FamilyParts { number: owner.family }).expect("family"),
                        ),
                        first: owner.version,
                        last: owner.version,
                    },
                )
                .expect("forge version"),
            )
            .expect("second family");
        let hello = w::StreamEvent::Hello(
            w::Hello::new(&w::CEILINGS, w::HelloParts { stream: [1; 16], served, heartbeat: Duration::from_secs(60) })
                .expect("hello"),
        );
        self.sockets.get_mut(&fd).expect("stream socket").reply.extend(sse(&hello));
    }

    pub fn store_snapshot(&self) -> Box<[u8]> {
        self.stored(b"store").expect("saved store").into()
    }
    pub fn act_keys(&self) -> &[[u8; 16]] {
        &self.acts
    }
    pub(super) fn read_task(&mut self, fd: Fd) {
        let lim = jig_client_test_records::limits();
        let mut parts = jig_client_test_records::task_record().into_parts();
        parts.project = 3;
        let mut acts = Vec::new();
        for act in [w::ActKind::Stop, w::ActKind::Cancel] {
            let offered =
                w::Offered::new(&lim, w::OfferedParts { act, admission: w::Admission::Allowed, effect: None })
                    .expect("offer");
            let encoded = encode!(offered);
            acts.extend_from_slice(&u32::try_from(encoded.len()).expect("offer size").to_be_bytes());
            acts.extend_from_slice(&encoded);
        }
        parts.acts = acts.into_boxed_slice();
        let task = w::Task::new(&lim, parts).expect("task");
        let owned = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: encode!(w::ReadValue::Task(task)) },
        )
        .expect("task record");
        self.reply(fd, &encode!(w::ReadAnswer::Value(owned)));
    }
}
