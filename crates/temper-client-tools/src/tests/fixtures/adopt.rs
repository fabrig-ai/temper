use super::*;
use temper_client_domain::forge;
use temper_client_protocol_wire::{self as wire, v1 as w};
/// One byte past the form bound must refuse a write rather than retain its default.
pub(crate) const FIELD_OVERFLOW: u32 = 1;
/// One restored adopt distinguishes recovery from a fresh call.
pub(crate) const ADOPT_RESTORES: u32 = 1;
/// All nine capabilities exercise the forge's specified output order.
const ADOPT_KINDS: [bool; 9] = [true; 9];
/// A nonzero repository identifier is preserved in the structured result.
const REPOSITORY: u32 = 7;
/// These inputs alter the ask while keeping its already held name.
pub(crate) const CHANGED_ADOPTION: &[&str] = &["--prefix", "other/"];
pub(crate) fn forge_asker() -> client::Asker {
    temper_client_domain::owner::Owner::Forge.asker()
}
pub(crate) fn adopt_root(extra: &[&str]) -> (Harness, u32) {
    let mut root = signed_in(PARTY);
    let words = extended(ADOPTION, extra);
    let call = root.submit(words.as_slice());
    (root, call)
}
pub(crate) fn adopted_with_unknown_protection() -> w::Outcome {
    let limits = forge::wire_limits(&limits_root().domain.forge).expect("forge limits");
    w::Outcome::Adopted(
        w::Adopted::new(
            &limits,
            w::AdoptedParts {
                forge: 1,
                repository: REPOSITORY,
                owner: Box::from(b"o".as_slice()),
                name: Box::from(b"n".as_slice()),
                role: w::Role::Owned,
                home: true,
                kinds: w::Kinds::new(&limits, w::KindsParts { skein_bools: ADOPT_KINDS }).expect("kinds"),
                protection: w::Protection::Unknown,
            },
        )
        .expect("adopted"),
    )
}
pub(crate) fn forge_refused(reason: w::ForgeReason) -> w::Outcome {
    let limits = forge::wire_limits(&limits_root().domain.forge).expect("forge limits");
    w::Outcome::ForgeRefused(w::ForgeRefused::new(&limits, w::ForgeRefusedParts { reason }).expect("forge refusal"))
}
pub(crate) fn family_answer(outcome: w::Outcome) -> client::Answered {
    let limits = forge::wire_limits(&limits_root().domain.forge).expect("forge limits");
    let answer = w::Answer::new(&limits, w::AnswerParts { outcome }).expect("adoption answer");
    client::Answered::Family(client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::answer(&answer),
    })
}
impl Harness {
    pub(crate) fn submit_call(&mut self, call: crate::jig::call::Call, json: bool) -> u32 {
        self.next = self.next.checked_add(1).expect("fixture correlation");
        self.event(root::Event::Call { number: self.next, call, json, value: None });
        self.next
    }
    pub(crate) fn adoption_answer(&mut self, call: u32, outcome: w::Outcome) {
        let (attempt, _, _) = self.sends.get(&call).expect("adopt sent");
        self.client(client::Event::Up {
            up: client::Up::Answer {
                attempt: *attempt,
                envelope: skein_lib::RequestEnvelope::Final,
                answer: Some(family_answer(outcome)),
                why: None,
            },
        });
    }
    pub(crate) fn sent_adoption(&self, call: u32) -> w::AdoptParts {
        let (_, _, client::Ask::Family { record, .. }) = self.sends.get(&call).expect("adopt sent") else {
            panic!("forge ask")
        };
        let limits = forge::wire_limits(&self.limits.domain.forge).expect("forge limits");
        (match w::Requesting::decode(&limits, &mut skein_lib::Reader::new(&record.bytes))
            .expect("valid adopt")
            .into_parts()
            .request
        {
            w::Request::Adopt(adopt) => adopt,
            w::Request::Return(_)
            | w::Request::SetUp(_)
            | w::Request::StartDemo
            | w::Request::Accept(_)
            | w::Request::Reachable(_)
            | w::Request::Connect(_)
            | w::Request::Install(_) => {
                panic!("expected adoption")
            }
        })
        .into_parts()
    }
    pub(crate) fn adoption_bytes(&self, call: u32) -> &client::Ask {
        &self.sends.get(&call).expect("adopt sent").2
    }
    pub(crate) fn nothing_sent(&self) -> bool {
        self.sends.is_empty() && self.saved.is_empty()
    }
    pub(crate) fn adopt_outcome(&mut self, call: u32, outcome: client::Outcome) {
        let mut out = root::act_fixture::forge_told(
            &mut self.root,
            &self.limits,
            client::Told::Answered {
                key: client::Key { asker: forge_asker(), number: (u64::from(call) << 32u32) | 1 },
                outcome,
            },
        );
        for _ in 0..out.len() {
            match out.pop().expect("counted output") {
                crate::jig::runner::Out::Printed { number, printed } => {
                    assert!(self.answers.insert(number, printed).is_none());
                }
                crate::jig::runner::Out::Kept { line } => self.kept_lines.push(line).expect("kept line"),
                crate::jig::runner::Out::Progress { line } => self.progress.push(line).expect("bounded progress lines"),
                crate::jig::runner::Out::Client { .. }
                | crate::jig::runner::Out::Record { .. }
                | crate::jig::runner::Out::Ready
                | crate::jig::runner::Out::DraftAmendment { .. } => panic!("answer output"),
            }
        }
    }
}
pub(crate) fn restored_adoption(party: u64) -> List<skein_lib::RequestRecord<client::Held>> {
    let (root, call) = adopt_root(&[]);
    let mut records = List::with_capacity(ADOPT_RESTORES);
    records
        .push(skein_lib::RequestRecord {
            key: root.sent_name(call).expect("named adopt"),
            scope: party,
            first_sent: None,
            request: client::Held {
                key: client::Key { asker: forge_asker(), number: (u64::from(RESTORED_PREFIX) << 32u32) | 1 },
                request: root.adoption_bytes(call).clone(),
            },
        })
        .expect("one adopt restore");
    records
}
pub(crate) fn part_read() -> (Harness, u32, c::Task) {
    let (root, call, mut task) = task_read();
    let mut path = List::with_capacity(FORGE_SEGMENTS);
    for segment in [b"forge".as_slice(), b"example.com", b"o", b"n"] {
        path.push(Box::from(segment)).expect("resource path");
    }
    task.resources = List::with_capacity(RESOURCES);
    task.resources
        .push(c::TaskResource::Owner(Box::new(c::OwnerResourceLine {
            part: c::OwnerPart {
                connector: wire::FORGE,
                kind: c::OwnerPartKind::Resource,
                key: c::OwnerPartKey::Path(path),
            },
            kind: 1,
            taken: false,
            holder: None,
            writer: None,
            writer_lost: false,
        })))
        .expect("one forge resource");
    (root, call, task)
}
/// Four segments distinguish a forge resource from an arbitrary path.
const FORGE_SEGMENTS: u32 = 4;
pub(crate) fn long_form_value(extra: u32) -> Box<[u8]> {
    let bound = limits_root().domain.forge.field_bytes.checked_add(extra).expect("fixture bound");
    let mut out = skein_lib::Writer::new(usize::try_from(bound).expect("fixture bytes"));
    for _ in 0..bound {
        out.put(b"x").expect("field bytes");
    }
    out.finish()
}

/// A different restored name advances prefixes without holding the new call's name.
pub(crate) fn adoption_prefix_restore() -> List<skein_lib::RequestRecord<client::Held>> {
    let mut records = restored_adoption(PARTY);
    records.get_mut(EMPTY).expect("one restore").key = named("previous-adoption");
    records
}
pub(crate) fn core_key_conflict() -> w::Outcome {
    let limits = forge::wire_limits(&limits_root().domain.forge).expect("forge limits");
    let refusal = jig_client_protocol_wire::v19::Refusal::KeyConflict;
    let mut bytes = skein_lib::Writer::new(usize::try_from(refusal.measure()).expect("refusal bytes"));
    refusal.encode(&mut bytes).expect("refusal encoding");
    w::Outcome::CoreRefused(
        w::CoreRefused::new(&limits, w::CoreRefusedParts { refusal: bytes.finish() }).expect("core refusal"),
    )
}
pub(crate) fn malformed_adoption_answer() -> client::Outcome {
    client::Outcome::Answered(client::Answered::Family(client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: Box::new([]),
    }))
}
/// The repository's input fields, rather than a new call, are varied for required checks.
pub(crate) const REQUIRED_FIELDS: &[usize] = &[2, 3, 4, 6];
pub(crate) fn blank_adoption(field: usize) -> crate::jig::call::Call {
    let mut call = adoption();
    *call.values.get_mut(field).expect("text field") =
        Some(crate::jig::call::Values::One(crate::jig::call::Value::Text(Box::new([]))));
    call
}
/// Half the output window in each of two segments crosses it once joined.
pub(crate) const BRANCH_BYTES: u32 = crate::limits::TEXT / 2;
/// Five identifying segments and two branch segments fit the client's nine-segment bound.
const BRANCH_SEGMENTS: u32 = 7;
/// A drawn engine key carries no name to reconstruct after a restart.
pub(crate) const DRAWN_ADOPTION_KEY: skein_lib::RequestKey = [9; 16];
pub(crate) fn branch_part_read() -> (Harness, u32, c::Task, Box<[u8]>) {
    let (root, call, mut task) = part_read();
    let mut segment = skein_lib::Writer::new(usize::try_from(BRANCH_BYTES).expect("branch bytes"));
    for _ in 0..BRANCH_BYTES {
        segment.put(b"x").expect("branch segment");
    }
    let segment = segment.finish();
    let mut path = List::with_capacity(BRANCH_SEGMENTS);
    for word in [b"forge".as_slice(), b"example.com", b"o", b"n", b"branch", &segment, &segment] {
        path.push(Box::from(word)).expect("bounded branch path");
    }
    let c::TaskResource::Owner(resource) = task.resources.get_mut(EMPTY).expect("one resource") else {
        panic!("owner resource")
    };
    resource.part.key = c::OwnerPartKey::Path(path);
    let length = segment.len().checked_mul(2).expect("two segments").checked_add(1).expect("separator");
    let mut expected = skein_lib::Writer::new(length);
    expected.put(&segment).expect("first segment");
    expected.put(b"/").expect("separator");
    expected.put(&segment).expect("second segment");
    (root, call, task, expected.finish())
}
