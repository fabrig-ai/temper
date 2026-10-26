use super::*;
use jig_client_protocol_link::{Limits, translate};
use skein_lib::{List, Reader, RequestRecord};
pub const KEPT_KEY: [u8; 16] = [9; 16];
const PARTY: u64 = 7;
const PROJECT: u32 = 3;
const BUDGET: u64 = 7; // Fits the shared authority's spend, exposing an admitted chat.
const ONE: u32 = 1; // One template and one membership are sufficient to offer chat.
use jig_client_domain::TABLE_VERSION;
fn link_limits() -> Limits {
    let limits = crate::limits::native().root.domain.client;
    Limits::new(limits.domain, limits.exchanges).expect("native link limits")
}
fn piece(bytes: &[u8]) -> Box<[u8]> {
    let mut out = u32::try_from(bytes.len()).expect("piece length").to_be_bytes().to_vec();
    out.extend_from_slice(bytes);
    out.into_boxed_slice()
}
pub fn chat_me() -> w::Me {
    let lim = jig_client_test_records::limits();
    let text = jig_client_test_records::wire_text;
    let template = w::Template::new(
        &lim,
        w::TemplateParts {
            number: jig_client::shapes::CHAT,
            name: text(b"chat"),
            executor: w::TemplateExecutor::Charter(
                w::TemplateCharter::new(&lim, w::TemplateCharterParts { value: 1 }).expect("charter"),
            ),
            contract: w::TaskContractDocument::Report(
                w::TaskContractDocumentReport::new(&lim, w::TaskContractDocumentReportParts { words: 32 })
                    .expect("report"),
            ),
            shallower: 1,
            budget: w::TemplateBudget::Amount(
                w::TemplateAmount::new(&lim, w::TemplateAmountParts { value: BUDGET }).expect("amount"),
            ),
            deadline: w::TemplateDeadline::None,
            tracked: None,
            flags: w::TaskFlags::new(
                &lim,
                w::TaskFlagsParts {
                    ended_by: w::EndedBy::Result,
                    conversation: true,
                    listed: false,
                    retention: w::Retention::Kept,
                },
            )
            .expect("flags"),
            label: jig_client::shapes::CHAT,
            grace: None,
            llm: None,
        },
    )
    .expect("chat template");
    let mut charters = List::with_capacity(ONE);
    charters.push(1).expect("one charter");
    let creating = w::CreatingBounds::new(
        &lim,
        w::CreatingBoundsParts {
            within: w::BoundsCreate::new(
                &lim,
                w::BoundsCreateParts {
                    authority: jig_client_test_records::wire_owner_authority_document(),
                    pool_left: BUDGET,
                    charters,
                    procedures: List::with_capacity(0),
                    latest: None,
                    budget: BUDGET,
                    maximum_budget: BUDGET,
                    maximum_deadline: None,
                },
            )
            .expect("bounds"),
            beyond: w::Beyond::Proposed(
                w::BeyondProposed::new(
                    &lim,
                    w::BeyondProposedParts {
                        to: w::Recipient::Party(
                            w::RecipientParty::new(&lim, w::RecipientPartyParts { value: PARTY }).expect("recipient"),
                        ),
                    },
                )
                .expect("beyond"),
            ),
        },
    )
    .expect("creating");
    let project = w::MeProject::new(
        &lim,
        w::MeProjectParts {
            project: PROJECT,
            name: text(b"shop"),
            role: w::HeldRole::Owner,
            pool: None,
            templates: piece(&encode!(template)),
            creating: Some(creating),
            models: List::with_capacity(0),
        },
    )
    .expect("membership");
    let mut parts = jig_client_test_records::frame_record().into_parts();
    parts.projects = piece(&encode!(project));
    parts.retention = Duration::from_secs(3600);
    w::Me::new(&lim, parts).expect("chat frame")
}
pub(super) fn record(party: u64) -> jig_client::Record {
    let lim = link_limits();
    let me = translate::documents::frame(chat_me(), &lim).expect("chat me");
    let request = jig_client::shapes::chat(
        &me,
        PROJECT,
        jig_client::contract::Text::new(b"hello".as_slice().into(), crate::limits::native().root.tools.text)
            .expect("words"),
    )
    .expect("chat shape");
    stored_request(party, jig_client::Ask::Jig(Box::new(request)))
}
pub(super) fn stored_request(party: u64, request: jig_client::Ask) -> jig_client::Record {
    let lim = link_limits();
    let table = translate::store::table(
        &RequestRecord {
            key: KEPT_KEY,
            scope: party,
            first_sent: None,
            request: jig_client::Held {
                key: jig_client::Key { asker: temper_client_tools::root::RUNNER.asker(), number: 9 << 32 | 1 },
                request,
            },
        },
        &lim,
    )
    .expect("kept table");
    let kept = w::Kept::new(&lim.wire, w::KeptParts { store_version: TABLE_VERSION, record: encode!(table) })
        .expect("kept record");
    jig_client::Record { key: KEPT_KEY.into(), bytes: encode!(kept) }
}
pub fn small_store(records: &[jig_client::Record]) -> Box<[u8]> {
    let mut bytes = u32::try_from(records.len()).expect("record count").to_be_bytes().to_vec();
    for record in records {
        bytes.extend_from_slice(&piece(&record.key));
        bytes.extend_from_slice(&piece(&record.bytes));
    }
    bytes.into_boxed_slice()
}
pub fn kept_chat() -> Box<[u8]> {
    small_store(&[record(PARTY)])
}
pub fn other_party() -> Box<[u8]> {
    small_store(&[record(PARTY + 1)])
}
pub fn dropped() -> Box<[u8]> {
    small_store(&[jig_client::Record { key: KEPT_KEY.into(), bytes: b"unread".as_slice().into() }])
}
impl Frame {
    pub fn act_with(store: Box<[u8]>) -> Self {
        Self::act_words_with(CHAT, store)
    }
    pub fn act_words_with(words: &[&[u8]], store: Box<[u8]>) -> Self {
        let mut frame = Self::act(words);
        frame.store = Some(store);
        frame.offers = Some(());
        frame
    }
    pub fn answer_kept(&mut self) {
        self.answer_kept = Some(());
        self.sign_in_then_deliver_me();
    }
    pub fn answer_new_act(&mut self) {
        self.answer_new = Some(());
        self.sign_in_then_deliver_me();
    }
    pub fn store_erase(&mut self) {
        self.hold_stores = None;
        self.run();
    }
    pub fn fail_erase(&mut self) {
        self.fail_store = Some(());
        self.store_erase();
    }
    pub fn root_kept(&self) -> u32 {
        self.host.root.kept()
    }
    pub fn act_sent(&self) -> bool {
        self.acts.iter().any(|key| !self.is_kept(key))
    }
    pub fn store_count(&self) -> u32 {
        self.store_writes
    }
    pub fn kept_records(&self) -> u32 {
        let bytes = self.stored(b"store").or(self.store.as_deref()).expect("store");
        Reader::new(bytes).u32().expect("small store")
    }
    pub(super) fn act_reply(&mut self, fd: Fd, body: &[u8]) {
        let keyed = w::Keyed::decode(&w::CEILINGS, &mut Reader::new(body)).expect("keyed request");
        self.acts.push(*keyed.key());
        if let Some(value) = keyed.value() {
            self.carried_keys.push(value.clone());
        }
        if self.move_mode && matches!(keyed.owner(), w::Owner::Jig) {
            self.move_reply(fd, &keyed);
            return;
        }
        if !if self.is_kept(keyed.key()) { self.answer_kept.is_some() } else { self.answer_new.is_some() } {
            return;
        }
        self.hold_stores = Some(());
        if matches!(keyed.owner(), w::Owner::Family(_)) {
            self.adopt_reply(fd);
            return;
        }
        let value = w::OwnerOutcome::Created(
            w::OwnerOutcomeCreated::new(&w::CEILINGS, w::OwnerOutcomeCreatedParts { first: 41, last: 41, given: None })
                .expect("created"),
        );
        let outcome = w::Outcome::Owner(
            w::OutcomeOwner::new(&w::CEILINGS, w::OutcomeOwnerParts { value }).expect("owner outcome"),
        );
        let answer = w::Final::Done(w::Done::new(&w::CEILINGS, w::DoneParts { outcome }).expect("done"));
        let owned = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts { owner: w::Owner::Jig, version: w::VERSION, record: encode!(answer) },
        )
        .expect("answer");
        let answer = w::KeyedAnswer::FinalAnswer(owned);
        self.reply(fd, &encode!(answer));
    }
    pub(super) fn reply(&mut self, fd: Fd, body: &[u8]) {
        let reply = &mut self.sockets.get_mut(&fd).expect("socket").reply;
        reply.extend(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).bytes());
        reply.extend(body.iter().copied());
    }
}

pub const ADOPT: &[&[u8]] = &[
    b"adopt",
    b"--project",
    b"shop",
    b"--owner",
    b"o",
    b"--name",
    b"n",
    b"--landing",
    b"main",
    b"--key",
    b"adopt-repo",
];
pub fn unanswered_adopt() -> Box<[u8]> {
    let mut first = Frame::act_words_with(ADOPT, small_store(&[]));
    first.sign_in_then_deliver_me();
    first.store_snapshot()
}
impl Frame {
    fn is_kept(&self, key: &[u8; 16]) -> bool {
        self.initial_records().iter().any(|record| record.key.as_ref() == key)
    }
    pub fn initial_records(&self) -> Vec<jig_client::Record> {
        let Some(bytes) = &self.store else { return Vec::new() };
        let count = Reader::new(bytes).u32().expect("store count");
        let mut rest = &bytes[4..];
        let mut records = Vec::new();
        for _ in 0..count {
            let length = Reader::new(rest).u32().expect("key length") as usize;
            let key = rest[4..4 + length].into();
            rest = &rest[4 + length..];
            let length = Reader::new(rest).u32().expect("record length") as usize;
            let bytes = rest[4..4 + length].into();
            rest = &rest[4 + length..];
            records.push(jig_client::Record { key, bytes });
        }
        records
    }
    pub fn answer_restored(&mut self) {
        self.answer_kept = Some(());
        let mut requests = Vec::new();
        for (fd, socket) in &self.sockets {
            if !socket.closed
                && socket.reply.is_empty()
                && let Some(end) = skein_lib::bytes::find(&socket.request, b"\r\n\r\n")
                && socket.request.starts_with(b"POST /jig/v1/request ")
            {
                requests.push((*fd, socket.request[end + 4..].to_vec()));
            }
        }
        for (fd, bytes) in requests {
            self.act_reply(fd, &bytes);
        }
        self.run();
    }
    fn adopt_reply(&mut self, fd: Fd) {
        use temper_client_protocol_wire::{self as wire, v1 as f};
        let answer = f::Answer::new(
            &f::CEILINGS,
            f::AnswerParts {
                outcome: f::Outcome::ForgeRefused(
                    f::ForgeRefused::new(&f::CEILINGS, f::ForgeRefusedParts { reason: f::ForgeReason::NotFound })
                        .expect("refusal"),
                ),
            },
        )
        .expect("answer");
        let owned = w::Owned::new(
            &w::CEILINGS,
            w::OwnedParts {
                owner: w::Owner::Family(
                    w::Family::new(&w::CEILINGS, w::FamilyParts { number: wire::FORGE }).expect("family"),
                ),
                version: wire::CLIENT_VERSION,
                record: wire::answer(&answer),
            },
        )
        .expect("owned answer");
        self.reply(fd, &encode!(w::KeyedAnswer::FinalAnswer(owned)));
    }
}
