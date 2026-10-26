//! The test's shell: linked sign-in played as platform reports, through bytes.
use jig_client_protocol_wire::{VERSION, v19 as w};
use jig_client_shell_web::adapter::{Asked, Heard};
use skein_lib::{
    Duration, List, Overflow, Reader, Time, Wall, Writer,
    base64::{self, Alphabet},
};
use skein_ui::{Act, Role};
use skein_ui_web::{mapping::Primary, page::Page, transport::Carried};
use temper_client_protocol_wire::{self as wire, v1 as f};
use temper_client_shell_web::{
    host::{Duties, Host, Report},
    journey::{Facts, Recorder},
};

const ADDRESS: &[u8] = b"http://127.0.0.1:9/authorize?state=test";
struct Walk {
    host: Host,
    page: Page<'static>,
    recorder: Option<Recorder>,
    recording: Option<Box<[u8]>>,
    now: Time,
    wall: Wall,
}
impl Walk {
    fn new() -> Self {
        let facts = Facts {
            seed: 11,
            now: Time::ZERO,
            wall: Wall::EPOCH,
            primary: Primary::Control,
            ways: None,
            forges: Vec::new(),
            path: b"/accounts/connect".as_slice().into(),
            secret: None,
            stored: Vec::new(),
        };
        let (host, page) = facts.start();
        Self { host, page, recorder: Some(Recorder::new(&facts)), recording: None, now: facts.now, wall: facts.wall }
    }
    fn turn(&mut self, reports: Vec<Report>) -> Vec<Asked> {
        self.now = self.now.saturating_add(Duration::from_millis(10));
        self.wall = Wall::from_nanos(self.wall.as_nanos() + Duration::from_millis(10).as_nanos());
        self.host.clocks(self.now, self.wall);
        self.recorder.as_mut().expect("recording until departure").turn(self.now, self.wall);
        let mut pending: std::collections::VecDeque<_> = reports.into();
        let mut asks = Vec::new();
        while let Some(report) = pending.pop_front() {
            let duties =
                self.recorder.as_mut().expect("recording until departure").report(&mut self.host, &self.page, report);
            if duties.departs() {
                self.recording = Some(self.recorder.take().expect("departure consumes recorder").end(&self.page));
                asks.extend(duties.asked);
                return asks;
            }
            let Duties { asked, writes, .. } = duties;
            let carried: Vec<_> = asked
                .iter()
                .filter_map(|ask| {
                    if let Asked::Save { key, .. } = ask { Some(Carried::Written { key: key.clone() }) } else { None }
                })
                .collect();
            asks.extend(asked);
            if writes != 0 || !carried.is_empty() {
                pending.push_back(Report::Carried { reports: carried, writes });
            }
        }
        self.host.draw(&mut self.page, &mut Vec::new());
        self.recorder.as_mut().expect("recording until departure").deadline(self.host.deadline());
        asks
    }
    fn press_heading(&mut self) {
        let heading = self
            .host
            .tree()
            .nodes()
            .iter()
            .find(|node| matches!(node.role, Role::Heading { .. }))
            .expect("a heading")
            .id;
        assert!(self.host.event_of(heading, Act::Press).is_none());
        assert!(self.turn(vec![Report::Act { node: heading, act: Act::Press }]).is_empty());
    }
    fn continue_with_chatgpt(&mut self) -> u32 {
        let button = self
            .host
            .tree()
            .nodes()
            .iter()
            .find(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Continue with ChatGPT"))
            .expect("sign-in button")
            .id;
        exchange(self.turn(vec![Report::Act { node: button, act: Act::Press }]))
    }
    fn streamed(&mut self, event: &w::StreamEvent) -> Vec<Asked> {
        let bytes = encoded(event.measure(), |writer| event.encode(writer));
        let size = base64::encoded_len(Alphabet::Standard, u32::try_from(bytes.len()).expect("fixture fits"))
            .expect("base64 fits");
        let mut writer = Writer::new(usize::try_from(size).expect("fixture fits"));
        base64::encode(Alphabet::Standard, &bytes, &mut writer).expect("measured base64");
        let mut piece = b"data: ".to_vec();
        piece.extend_from_slice(&writer.finish());
        piece.extend_from_slice(b"\n\n");
        self.turn(vec![Report::Heard(Heard::Piece { bytes: piece.into() })])
    }
    fn answered(&mut self, exchange: u32, answer: &w::KeyedAnswer) -> Vec<Asked> {
        self.turn(vec![Report::Heard(Heard::Answered {
            exchange,
            status: 200,
            body: encoded(answer.measure(), |writer| answer.encode(writer)),
        })])
    }
}
fn encoded(size: u32, write: impl FnOnce(&mut Writer) -> Result<(), Overflow>) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(size).expect("fixture fits"));
    write(&mut writer).expect("measured fixture");
    writer.finish()
}
fn owned(record: Box<[u8]>) -> w::Owned {
    w::Owned::new(&w::CEILINGS, w::OwnedParts { owner: w::Owner::Jig, version: VERSION, record })
        .expect("fixture envelope")
}
fn exchange(asks: Vec<Asked>) -> u32 {
    asks.into_iter()
        .find_map(|ask| {
            let Asked::Exchange { id, call, .. } = ask else {
                return None;
            };
            (call.path.as_ref() == b"/jig/v1/request").then_some(id)
        })
        .expect("keyed request left the linked host")
}
pub(super) fn walked() -> Box<[u8]> {
    walked_with_no_event(false)
}
pub(super) fn walked_with_no_event(no_event: bool) -> Box<[u8]> {
    walked_with_saves(no_event).recording
}
pub(super) struct Saves {
    pub recording: Box<[u8]>,
    pub before_press: Box<[u8]>,
    pub first: Box<[u8]>,
    pub later: Box<[u8]>,
}
pub(super) fn walked_with_saves(no_event: bool) -> Saves {
    let mut walk = Walk::new();
    walk.turn(vec![Report::Start]);
    walk.turn(vec![Report::Heard(Heard::StreamOpened)]);
    let mut served = List::with_capacity(2);
    for (owner, version) in [
        (w::Owner::Jig, VERSION),
        (
            w::Owner::Family(
                w::Family::new(&w::CEILINGS, w::FamilyParts { number: wire::FORGE }).expect("forge family"),
            ),
            wire::CLIENT_VERSION,
        ),
    ] {
        served
            .push(
                w::Served::new(&w::CEILINGS, w::ServedParts { owner, first: version, last: version })
                    .expect("served family"),
            )
            .expect("two families");
    }
    let asks = walk.streamed(&w::StreamEvent::Hello(
        w::Hello::new(&w::CEILINGS, w::HelloParts { stream: [7; 16], served, heartbeat: Duration::from_secs(30) })
            .expect("hello"),
    ));
    let watch = asks
        .into_iter()
        .find_map(|ask| {
            let Asked::Exchange { call, .. } = ask else {
                return None;
            };
            if call.path.as_ref() != b"/jig/v1/watch" {
                return None;
            }
            let open = w::Open::decode(&w::CEILINGS, &mut Reader::new(&call.body)).expect("watch envelope");
            let subject = w::Subject::decode(&w::CEILINGS, &mut Reader::new(open.subject().record())).expect("subject");
            matches!(subject, w::Subject::Me).then_some(open.watch())
        })
        .expect("me watch");
    let value = me_with_openai();
    walk.streamed(&w::StreamEvent::Delivery(
        w::Delivery::new(
            &w::CEILINGS,
            w::DeliveryParts { watch, document: owned(encoded(value.measure(), |writer| value.encode(writer))) },
        )
        .expect("me delivery"),
    ));
    if no_event {
        walk.press_heading();
    }
    let before_press = walk.recorder.as_ref().expect("recording").save(&walk.page);
    let signing = walk.continue_with_chatgpt();
    let first = walk.recorder.as_ref().expect("recording").save(&walk.page);
    let started = w::OwnerOutcome::AccountSignInStarted(
        w::OwnerOutcomeAccountSignInStarted::new(
            &w::CEILINGS,
            w::OwnerOutcomeAccountSignInStartedParts { source: 43, address: ADDRESS.into() },
        )
        .expect("sign-in started"),
    );
    let outcome = w::Outcome::Owner(
        w::OutcomeOwner::new(&w::CEILINGS, w::OutcomeOwnerParts { value: started }).expect("owner outcome"),
    );
    let answer = w::Final::Done(w::Done::new(&w::CEILINGS, w::DoneParts { outcome }).expect("done"));
    let answer = w::KeyedAnswer::FinalAnswer(owned(encoded(answer.measure(), |writer| answer.encode(writer))));
    let returning = exchange(walk.answered(signing, &answer));
    let later = walk.recorder.as_ref().expect("recording").save(&walk.page);
    let answer = f::Answer::new(&f::CEILINGS, f::AnswerParts { outcome: f::Outcome::Returning }).expect("returning");
    let record = w::Owned::new(
        &w::CEILINGS,
        w::OwnedParts {
            owner: w::Owner::Family(
                w::Family::new(&w::CEILINGS, w::FamilyParts { number: wire::FORGE }).expect("forge family"),
            ),
            version: wire::CLIENT_VERSION,
            record: encoded(answer.measure(), |writer| answer.encode(writer)),
        },
    )
    .expect("family answer");
    walk.answered(returning, &w::KeyedAnswer::FinalAnswer(record));
    Saves { recording: walk.recording.expect("departure ended the walk"), before_press, first, later }
}

fn me_with_openai() -> w::DeliveryValue {
    let mut me = jig_client_test_records::frame_record().into_parts();
    let provider = w::ProviderOffer::new(
        &w::CEILINGS,
        w::ProviderOfferParts {
            provider: 17,
            name: w::DocumentText::new(
                &w::CEILINGS,
                w::DocumentTextParts { bytes: b"openai".as_slice().into(), cut: false },
            )
            .expect("name"),
            skein_bools: [true, true, false, false, true],
            programs: List::with_capacity(0),
            manage_usage: None,
            key_bytes: 4096,
        },
    )
    .expect("provider");
    me.providers = encoded(provider.measure() + 4, |writer| {
        writer.put(&provider.measure().to_be_bytes())?;
        provider.encode(writer)
    });
    w::DeliveryValue::Me(w::Me::new(&w::CEILINGS, me).expect("me"))
}
