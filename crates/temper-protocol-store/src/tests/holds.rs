//! Every schema word reads bytes taken from a full fixture's stored encoding.
use super::{Bounds, api, w};
use crate::tests::fixtures;
use skein_forge::api as d;
use skein_lib::{List, Reader};

// The schema has eight answer-record words and four name-list words.
const ANSWER_WORDS: u32 = 8;
const NAME_WORDS: u32 = 4;
use temper_engine_domain_forge as forge;

// Ordinary test code: one framing loop, with each declaration's own decoder.
fn pieces<T>(bytes: &[u8], bounds: &Bounds, decode: fn(&Bounds, &mut Reader<'_>) -> T) -> Box<[T]> {
    let mut reader = Reader::new(bytes);
    let prefix_bytes = u32::try_from(size_of::<u32>()).expect("piece prefix size");
    let mut values = List::with_capacity(reader.remaining().checked_div(prefix_bytes).expect("nonzero prefix"));
    while let Some(bytes) = skein_codec::piece::split(&mut reader).expect("whole piece framing") {
        let mut item = Reader::new(&bytes);
        assert!(values.push(decode(bounds, &mut item)).is_ok(), "framing bounds the item count");
        assert!(item.is_empty(), "declaration consumes the whole piece");
    }
    assert!(reader.is_empty(), "pieces consume their whole holder");
    values.into_boxed()
}

fn stored(row: &forge::Stored, bounds: &Bounds) -> w::Value {
    let bytes = super::encode(row);
    w::Row::decode(bounds.limits(), &mut Reader::new(&bytes)).unwrap().into_parts().value
}

fn answer(fixture: &d::Answer, bounds: &Bounds) -> w::Answer {
    let row = fixtures::named_call(forge::CallPayload::Read(Box::new(Ok(fixture.clone()))));
    let w::Value::Call(call) = stored(&row, bounds) else { panic!("call fixture") };
    let w::CallPayload::Read(read) = call.into_parts().answer else { panic!("read fixture") };
    let w::ReadResult::Answer(value) = read.into_parts().result else { panic!("answer fixture") };
    value.into_parts().answer
}

#[derive(Debug, PartialEq, Eq)]
enum Items {
    Summaries(Box<[d::Summary]>),
    Comments(Box<[d::Comment]>),
    Reviews(Box<[d::Review]>),
    Statuses(Box<[d::Status]>),
    Remarks(Box<[d::Remark]>),
    Files(Box<[d::File]>),
}
impl Items {
    fn read(&self, bytes: &[u8], bounds: &Bounds) -> Self {
        match self {
            Self::Summaries(_) => Self::Summaries(pieces(bytes, bounds, |bounds, reader| {
                api::decode_summary(w::Summary::decode(bounds.limits(), reader).unwrap(), bounds).unwrap()
            })),
            Self::Comments(_) => Self::Comments(pieces(bytes, bounds, |bounds, reader| {
                api::decode_comment(w::Comment::decode(bounds.limits(), reader).unwrap())
            })),
            Self::Reviews(_) => Self::Reviews(pieces(bytes, bounds, |bounds, reader| {
                api::decode_review(w::Review::decode(bounds.limits(), reader).unwrap())
            })),
            Self::Statuses(_) => Self::Statuses(pieces(bytes, bounds, |bounds, reader| {
                api::decode_status(w::Status::decode(bounds.limits(), reader).unwrap())
            })),
            Self::Remarks(_) => Self::Remarks(pieces(bytes, bounds, |bounds, reader| {
                api::decode_remark(w::Remark::decode(bounds.limits(), reader).unwrap())
            })),
            Self::Files(_) => Self::Files(pieces(bytes, bounds, |bounds, reader| {
                api::decode_file(w::File::decode(bounds.limits(), reader).unwrap())
            })),
        }
    }
}
#[derive(Debug)]
struct ItemRow {
    path: &'static str,
    bytes: Box<[u8]>,
    expected: Items,
}

#[test]
fn answer_piece_words_read_their_full_fixtures() {
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).unwrap();
    let mut rows = List::with_capacity(ANSWER_WORDS);
    for fixture in fixtures::item_answers().into_iter().chain(fixtures::remaining_answers()) {
        let encoded = answer(&fixture, &bounds);
        let (path, bytes, expected) = match (fixture, encoded) {
            (d::Answer::Items { items, .. }, w::Answer::Items(value)) => {
                ("ItemsAnswer.items", value.into_parts().items, Items::Summaries(items))
            }
            (d::Answer::Item { comments, .. }, w::Answer::Item(value)) => {
                ("ItemAnswer.comments", value.into_parts().comments, Items::Comments(comments))
            }
            (d::Answer::Reviews { reviews, .. }, w::Answer::Reviews(value)) => {
                ("ReviewsAnswer.reviews", value.into_parts().reviews, Items::Reviews(reviews))
            }
            (d::Answer::Statuses { statuses, .. }, w::Answer::Statuses(value)) => {
                ("StatusesAnswer.statuses", value.into_parts().statuses, Items::Statuses(statuses))
            }
            (d::Answer::Remarks { remarks, .. }, w::Answer::Remarks(value)) => {
                ("RemarksAnswer.remarks", value.into_parts().remarks, Items::Remarks(remarks))
            }
            (d::Answer::PullFiles { files, .. }, w::Answer::PullFiles(value)) => {
                ("PullFilesAnswer.files", value.into_parts().files, Items::Files(files))
            }
            (d::Answer::Compare { files, .. }, w::Answer::Compare(value)) => {
                ("CompareAnswer.files", value.into_parts().files, Items::Files(files))
            }
            (d::Answer::Checks(statuses), w::Answer::Checks(value)) => {
                ("ChecksAnswer.statuses", value.into_parts().statuses, Items::Statuses(statuses))
            }
            (d::Answer::Collaborators { .. }, w::Answer::Collaborators(_)) => continue,
            _ => panic!("matching full answer fixtures"),
        };
        rows.push(ItemRow { path, bytes, expected }).expect("one row per answer word");
    }
    for row in &rows {
        assert_eq!(row.expected.read(&row.bytes, &bounds), row.expected, "{}", row.path);
    }
}

#[derive(Debug)]
struct NameRow {
    path: &'static str,
    bytes: Box<[u8]>,
    expected: Box<[Box<[u8]>]>,
    decode: fn(&Bounds, &mut Reader<'_>) -> Box<[u8]>,
}
fn segment(bounds: &Bounds, reader: &mut Reader<'_>) -> Box<[u8]> {
    w::Segment::decode(bounds.limits(), reader).unwrap().into_parts().name
}
fn answer_name(bounds: &Bounds, reader: &mut Reader<'_>) -> Box<[u8]> {
    w::AnswerName::decode(bounds.limits(), reader).unwrap().into_parts().name
}

#[test]
fn name_piece_words_read_their_full_fixtures() {
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).unwrap();
    let name = fixtures::named_branch();
    let row = forge::Stored::Hold(forge::Hold { name: name.clone(), task: 1, writer: None, drift: None });
    let w::Value::Hold(held) = stored(&row, &bounds) else { panic!("hold fixture") };
    let w::What::Branch(segments) = held.into_parts().name.into_parts().what else { panic!("branch fixture") };
    let forge::What::Branch(expected) = name.what else { panic!("branch fixture") };
    let mut rows = List::with_capacity(NAME_WORDS);
    rows.push(NameRow { path: "Segments.segments", bytes: segments.into_parts().segments, expected, decode: segment })
        .expect("one row per name word");
    for fixture in fixtures::answers_without_item_pieces().into_iter().chain(fixtures::item_answers()) {
        let encoded = answer(&fixture, &bounds);
        match (fixture, encoded) {
            (d::Answer::Branches(expected), w::Answer::Branches(value)) => rows
                .push(NameRow {
                    path: "BranchesAnswer.branches",
                    bytes: value.into_parts().branches,
                    expected,
                    decode: answer_name,
                })
                .expect("one row per name word"),
            (d::Answer::Protection(Some(expected)), w::Answer::Protection(value)) => rows
                .push(NameRow {
                    path: "Protection.contexts",
                    bytes: value.into_parts().protection.unwrap().into_parts().contexts,
                    expected: expected.contexts,
                    decode: answer_name,
                })
                .expect("one row per name word"),
            (d::Answer::Item { item, .. }, w::Answer::Item(value)) => rows
                .push(NameRow {
                    path: "Summary.labels",
                    bytes: value.into_parts().item.into_parts().labels,
                    expected: item.labels,
                    decode: answer_name,
                })
                .expect("one row per name word"),
            _ => (),
        }
    }
    for row in &rows {
        assert_eq!(pieces(&row.bytes, &bounds, row.decode), row.expected, "{}", row.path);
    }
}

#[test]
fn held_refusal_word_reads_the_full_application_fixture() {
    use jig_domain_people as p;
    use jig_protocol_store::people_v13 as people;
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).unwrap();
    // The full list fills the held declaration's largest payload at temper's limits.
    let mut tasks = List::with_capacity(people::CEILINGS.refusal_circle_tasks);
    for task in 0..people::CEILINGS.refusal_circle_tasks {
        tasks.push(u64::from(task)).expect("full refusal list");
    }
    let fixture = p::Refusal::Circle { tasks: tasks.into_boxed(), cut: true };
    let row = forge::Stored::Application {
        token: 1,
        until: skein_lib::Wall::from_nanos(2),
        outcome: forge::ApplicationOutcome::CoreRefused(fixture.clone()),
    };
    let w::Value::Application(application) = stored(&row, &bounds) else { panic!("application fixture") };
    let w::ApplicationOutcome::CoreRefused(value) = application.into_parts().outcome else { panic!("refusal fixture") };
    let rows = [("CoreRefused.refusal", value.into_parts().refusal, fixture)];
    for (path, bytes, expected) in rows {
        let mut reader = Reader::new(&bytes);
        let people::Refusal::Circle(value) = people::Refusal::decode(&people::CEILINGS, &mut reader).unwrap() else {
            panic!("circle fixture")
        };
        let value = value.into_parts();
        assert_eq!(p::Refusal::Circle { tasks: value.tasks.into_boxed(), cut: value.cut }, expected, "{path}");
        assert!(reader.is_empty(), "held declaration consumes its whole holder");
    }
}
